//! Each trigger occurrence keeps the quantity of the event it refers to (603.2, 608.2h).
use crate::support::cite;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

fn game() -> Game {
    let defs = vec![
        CardDef::new("Firedrinker Satyr", "{R}", "Creature — Satyr Shaman", "Whenever this creature is dealt damage, it deals that much damage to you.\n{1}{R}: This creature gets +1/+0 until end of turn and deals 1 damage to you.", Some((2, 1))).unwrap(),
        CardDef::new("Mirror", "{0}", "Creature — Wall", "Whenever this creature is dealt damage, it deals that much damage to you.", Some((0, 8))).unwrap(),
        CardDef::new("Flame", "{0}", "Instant", "Flame deals 3 damage to target creature.", None).unwrap(),
        CardDef::new("Spark", "{0}", "Instant", "Spark deals 1 damage to target creature.", None).unwrap(),
        CardDef::new("Well", "{0}", "Artifact", "Whenever you gain life, draw that many cards.", None).unwrap(),
        CardDef::new("Gift", "{0}", "Instant", "You gain 3 life.", None).unwrap(),
        CardDef::new("Memory", "{0}", "Creature — Wall", "Whenever this creature is dealt damage, you may draw that many cards.", Some((0, 8))).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Bond", "{0}", "Enchantment", "Whenever you lose life, target opponent loses that much life.", None).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap_or_else(|u| panic!("{}: {}", def.name(), u.0)); }
    let mut g = Game::new(defs, 2, 20);
    for player in 0..2 { for _ in 0..10 { g.put(player, 7, Zone::Library); } }
    g
}

fn cast(g: &mut Game, def: usize, target: Option<ObjId>) {
    let card = g.put(g.priority, def, Zone::Hand);
    g.act(g.priority, Move::Cast { card, targets: target.map(|t| vec![vec![Target::Object(t)]]).unwrap_or_default() }).unwrap();
}

fn pass(g: &mut Game) {
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
}

#[test]
fn r113_7a_a_lethally_damaged_satyr_still_deals_the_recorded_damage_after_it_dies() {
    let mut g = game();
    let satyr = g.put(1, 0, Zone::Battlefield);
    cast(&mut g, 2, Some(satyr));
    pass(&mut g);
    cite!(["704.5g"]; assert_eq!(g.obj(satyr).zone, Zone::Gone));
    assert_eq!(g.stack.len(), 1);
    pass(&mut g);
    cite!(["113.7a"]; assert_eq!(g.players[1].life, 17, "the original 3 damage, even though toughness was only 1"));
    assert_eq!(g.players[0].life, 20);
}

#[test]
fn r603_2c_separate_damage_events_keep_separate_amounts_while_both_triggers_wait() {
    let mut g = game();
    let mirror = g.put(1, 1, Zone::Battlefield);
    cast(&mut g, 2, Some(mirror));
    pass(&mut g);
    cast(&mut g, 3, Some(mirror));
    pass(&mut g);
    cite!(["603.2c"]; assert_eq!(g.stack.len(), 2));
    assert_eq!(g.obj(mirror).damage, 4);
    pass(&mut g);
    // Uncited (ruling 35ec345a61f0154c): "that much" is the trigger event's own amount (Firedrinker Satyr dealt 7 deals 7); each event triggers separately (603.2c, Sanguine Bond 26d01553be641c0b)
    assert_eq!(g.players[1].life, 19, "the latest trigger remembers 1, not accumulated damage");
    pass(&mut g);
    // Uncited (ruling 35ec345a61f0154c): "that much" is the trigger event's own amount (Firedrinker Satyr dealt 7 deals 7); each event triggers separately (603.2c, Sanguine Bond 26d01553be641c0b)
    assert_eq!(g.players[1].life, 16, "the earlier trigger still remembers 3");
}

#[test]
fn r120_8_zero_damage_creates_neither_a_damage_event_nor_a_trigger() {
    let mut g = game();
    let mirror = g.put(1, 1, Zone::Battlefield);
    g.damage(mirror, Target::Object(mirror), 0);
    g.act(0, Move::Pass).unwrap();
    cite!(["120.8"]; assert!(g.stack.is_empty()));
    assert_eq!(g.obj(mirror).damage, 0);
}

#[test]
fn r608_2h_a_life_gain_event_binds_its_amount_not_the_players_life_total() {
    let mut g = game();
    g.put(0, 4, Zone::Battlefield);
    cast(&mut g, 5, None);
    pass(&mut g);
    assert_eq!(g.players[0].life, 23);
    g.lose_life(0, 7).unwrap();
    pass(&mut g);
    // Uncited (ruling 35ec345a61f0154c): "that much" is the trigger event's own amount (Firedrinker Satyr dealt 7 deals 7); each event triggers separately (603.2c, Sanguine Bond 26d01553be641c0b)
    assert_eq!(g.players[0].hand.len(), 3);
    assert_eq!(g.players[0].library.len(), 7);
}

#[test]
fn r603_2_a_life_loss_trigger_keeps_the_amount_while_its_target_is_chosen() {
    let mut g = game();
    g.put(0, 8, Zone::Battlefield);
    g.lose_life(0, 4).unwrap();
    g.act(0, Move::Pass).unwrap();
    cite!(["603.3d"]; assert!(matches!(g.decision, Some(Decision::Targets { player: 0, .. }))));
    g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap();
    pass(&mut g);
    // Uncited (ruling 35ec345a61f0154c): "that much" is the trigger event's own amount (Firedrinker Satyr dealt 7 deals 7); each event triggers separately (603.2c, Sanguine Bond 26d01553be641c0b)
    assert_eq!((g.players[0].life, g.players[1].life), (16, 16));
}

#[test]
fn r608_2d_a_paused_and_cloned_optional_trigger_keeps_its_event_amount() {
    let mut g = game();
    let memory = g.put(0, 6, Zone::Battlefield);
    cast(&mut g, 2, Some(memory));
    pass(&mut g);
    pass(&mut g);
    cite!(["603.5"]; assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::May { possible: true }, .. }))));
    let mut declined = g.clone();
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    declined.act(0, Move::Choose(Answer::No)).unwrap();
    // Uncited (ruling 35ec345a61f0154c): "that much" is the trigger event's own amount (Firedrinker Satyr dealt 7 deals 7); each event triggers separately (603.2c, Sanguine Bond 26d01553be641c0b)
    assert_eq!(g.players[0].hand.len(), 3);
    cite!(["603.5"]; assert!(declined.players[0].hand.is_empty()));
}

#[test]
fn r603_2_an_event_without_a_quantity_does_not_bind_that_much() {
    for (types, text, pt) in [
        ("Instant", "You gain that much life.", None),
        ("Creature — Bear", "Whenever this creature attacks, you gain that much life.", Some((2, 2))),
    ] {
        let def = CardDef::new("Unbound", "{0}", types, text, pt).unwrap();
        assert!(game::play::check(&def).unwrap_err().0.contains("without a quantitative trigger event"));
    }
}
