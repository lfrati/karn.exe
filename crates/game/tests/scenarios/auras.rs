//! Auras, enchant and attachment (303.4, 702.5, 701.3), from card text.
use crate::support::cite;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

fn game(text: &str) -> Game {
    let defs = vec![
        CardDef::new("Aura", "{0}", "Enchantment — Aura", text, None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Mountain", "", "Basic Land — Mountain", "", None).unwrap(),
        CardDef::new(
            "Reclaim",
            "{0}",
            "Sorcery",
            "Return target enchantment card from your graveyard to the battlefield.",
            None,
        )
        .unwrap(),
        CardDef::new("Change", "{0}", "Instant", "Target permanent becomes an artifact until end of turn.", None)
            .unwrap(),
        CardDef::new("Move Aura", "{0}", "Instant", "Attach target Aura to target creature.", None).unwrap(),
        CardDef::new("Myr", "{0}", "Artifact Creature — Myr", "", Some((2, 2))).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0));
    }
    Game::new(defs, 2, 20)
}
fn pass(g: &mut Game) {
    g.act(g.priority, Move::Pass).unwrap();
    g.act(g.priority, Move::Pass).unwrap();
}
fn cast(g: &mut Game, card: ObjId, to: Target) {
    g.act(0, Move::Cast { card, targets: vec![vec![to]] }).unwrap();
}
fn aura(g: &mut Game, to: Target) -> ObjId {
    let card = g.put(0, 0, Zone::Hand);
    cast(g, card, to);
    pass(g);
    *g.battlefield.last().unwrap()
}

#[test]
fn r303_4a_enchant_defines_the_spell_target_and_the_attachment() {
    let mut g = game("Enchant Mountain you control\nEnchanted land has indestructible.");
    let mine = g.put(0, 2, Zone::Battlefield);
    let theirs = g.put(1, 2, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand);
    cite!(["303.4a"]; refused("an illegal target, Mountain: the target is one you control, and another player controls it (115.1)"): g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(theirs)]] }));
    crate::support::refused(g.act(0, Move::Cast { card, targets: vec![] }), "that has 1 target group(s), 0 given");
    cast(&mut g, card, Target::Object(mine));
    pass(&mut g);
    let a = *g.battlefield.last().unwrap();
    cite!(["608.3c"]; assert_eq!(g.attached_to(a), Some(Target::Object(mine))));
    assert!(g.has(mine, "Indestructible") && !g.has(theirs, "Indestructible"));
    assert_eq!(g.obj(a).controller, 0);
}

#[test]
fn r702_5c_all_enchant_abilities_restrict_one_target() {
    let mut g = game("Enchant creature\nEnchant artifact\nEnchanted creature gets +1/+1.");
    let ordinary = g.put(0, 1, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand);
    cite!(["702.5c"]; refused("an illegal target, Bear: it isn't an artifact (115.1)"): g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(ordinary)]] }));
    let myr = g.put(0, 6, Zone::Battlefield);
    cast(&mut g, card, Target::Object(myr));
    pass(&mut g);
    cite!(["702.5c"]; assert_eq!(g.power(myr), Some(3)));
}

#[test]
fn r608_3b_an_aura_with_an_illegal_target_does_not_enter() {
    let mut g = game("Enchant creature\nEnchanted creature gets +1/+1.");
    let bear = g.put(1, 1, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand);
    cast(&mut g, card, Target::Object(bear));
    g.destroy(bear);
    pass(&mut g);
    cite!(["608.3b"]; assert!(g.battlefield.is_empty()));
    cite!(["608.3b"]; assert_eq!(g.players[0].graveyard.len(), 1));
}

#[test]
fn r303_4c_an_illegal_attachment_goes_to_the_graveyard_without_destruction() {
    let mut g = game("Enchant creature\nIndestructible\nEnchanted creature gets +1/+1.");
    let bear = g.put(1, 1, Zone::Battlefield);
    let a = aura(&mut g, Target::Object(bear));
    let change = g.put(0, 4, Zone::Hand);
    cast(&mut g, change, Target::Object(bear));
    pass(&mut g);
    cite!(["303.4c", "704.5m"]; assert_eq!(g.obj(a).zone, Zone::Gone));
    cite!(["303.4c", "704.5m"]; assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).card == Some(0))));
}

#[test]
fn r303_4f_an_aura_returned_without_being_cast_chooses_its_attachment_as_it_enters() {
    let mut g = game("Enchant creature\nEnchanted creature gets +1/+1.");
    let bear = g.put(1, 1, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Graveyard);
    let reclaim = g.put(0, 3, Zone::Hand);
    cast(&mut g, reclaim, Target::Object(card));
    pass(&mut g);
    cite!(["303.4f"]; assert_eq!(
        g.decision,
        Some(Decision::Choose { player: 0, choice: Choice::Attach { aura: card, among: vec![Target::Object(bear)] } })
    ));
    assert_eq!(g.obj(card).zone, Zone::Graveyard);
    cite!(["303.4f"]; refused("choose a legal attachment (303.4f)"): g.act(0, Move::Choose(Answer::Attach(Target::Player(1)))));
    cite!(["303.4f"]; ok: g.act(0, Move::Choose(Answer::Attach(Target::Object(bear)))));
    assert_eq!(g.power(bear), Some(3));
    cite!(["303.4f"]; assert_eq!(g.attached_to(*g.battlefield.last().unwrap()), Some(Target::Object(bear))));
}

#[test]
fn r303_4g_an_aura_with_nothing_to_enchant_stays_in_its_zone() {
    let mut g = game("Enchant creature");
    let card = g.put(0, 0, Zone::Graveyard);
    let reclaim = g.put(0, 3, Zone::Hand);
    cast(&mut g, reclaim, Target::Object(card));
    pass(&mut g);
    cite!(["303.4g"]; assert_eq!(g.obj(card).zone, Zone::Graveyard));
    cite!(["303.4g"]; assert!(g.battlefield.is_empty() && g.decision.is_none()));
}

#[test]
fn r701_3b_c_attachment_moves_the_effect_and_changes_its_timestamp_only_when_legal() {
    let mut g = game("Enchant creature\nEnchanted creature gets +1/+1.");
    let one = g.put(0, 1, Zone::Battlefield);
    let two = g.put(1, 1, Zone::Battlefield);
    let a = aura(&mut g, Target::Object(one));
    let stamp = g.obj(a).stamp;
    cite!(["701.3b"]; assert!(!g.attach(a, Target::Object(one)) && !g.attach(a, Target::Object(a))));
    cite!(["701.3b"]; assert!(!g.attach(a, Target::Player(1))));
    cite!(["701.3b"]; assert_eq!(g.obj(a).stamp, stamp));
    let spell = g.put(0, 5, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(a)], vec![Target::Object(two)]] }).unwrap();
    pass(&mut g);
    cite!(["701.3c", "613.7e"]; assert!(g.obj(a).stamp > stamp));
    cite!(["701.3a"]; assert_eq!((g.power(one), g.power(two)), (Some(2), Some(3))));
    cite!(["303.4e"]; assert_eq!((g.obj(a).controller, g.obj(two).controller), (0, 1)));
}

#[test]
fn r702_5d_an_aura_can_enchant_a_player() {
    let mut g = game("Enchant player\nAt the beginning of your upkeep, enchanted player loses 1 life.");
    let a = aura(&mut g, Target::Player(1));
    cite!(["702.5d"]; assert_eq!(g.attached_to(a), Some(Target::Player(1))));
    for player in 0..2 {
        for _ in 0..3 {
            g.put(player, 2, Zone::Library);
        }
    }
    while g.turn < 3 {
        pass(&mut g);
    }
    while !g.stack.is_empty() {
        pass(&mut g);
    }
    assert_eq!(g.players[1].life, 19);
}

#[test]
fn r702_5d_an_aura_that_enchants_players_can_enchant_its_controller() {
    let mut g = game("Enchant player\nAt the beginning of your upkeep, enchanted player loses 1 life.");
    let a = aura(&mut g, Target::Player(0));
    cite!(["702.5d"]; assert_eq!(g.attached_to(a), Some(Target::Player(0))));
    cite!(["702.5d", "701.3a"]; assert!(g.attach(a, Target::Player(1)) && g.attach(a, Target::Player(0))));
}

#[test]
fn r704_5p_a_permanent_that_stops_being_an_aura_becomes_unattached() {
    let mut g = game("Enchant creature\nEnchanted creature gets +1/+1.");
    let bear = g.put(0, 1, Zone::Battlefield);
    let a = aura(&mut g, Target::Object(bear));
    let change = g.put(0, 4, Zone::Hand);
    cast(&mut g, change, Target::Object(a));
    pass(&mut g);
    cite!(["704.5p"]; assert_eq!(g.obj(a).zone, Zone::Battlefield));
    cite!(["704.5p"]; assert_eq!(g.attached_to(a), None));
    assert_eq!(g.power(bear), Some(2));
}

#[test]
fn r603_10a_an_aura_sees_its_enchanted_creature_die() {
    let mut g = game("Enchant creature\nWhen enchanted creature dies, you gain 3 life.");
    let bear = g.put(1, 1, Zone::Battlefield);
    let a = aura(&mut g, Target::Object(bear));
    g.destroy(bear);
    g.act(0, Move::Pass).unwrap();
    assert_eq!(g.obj(a).zone, Zone::Gone);
    cite!(["603.10a"]; assert_eq!(g.stack.len(), 1));
    pass(&mut g);
    cite!(["603.10a"]; assert_eq!(g.players[0].life, 23));
}
