//! Ordinary instructions compose in one compiled body, with retained child operations.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, Target, Zone};
use game::state::{Answer, Choice, Decision};
use game::operation::ReplacementId;

fn card(name: &str, kind: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    CardDef::new(name, "{0}", kind, text, pt).unwrap()
}
fn resolve(g: &mut Game) {
    for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); }
}

#[test]
fn r608_2c_draw_life_damage_tap_counters_and_creation_share_one_compiled_body() {
    let defs = vec![
        card("Sequence", "Sorcery", "Draw a card. You gain 3 life. You lose 1 life. Sequence deals 2 damage to target player. Tap target creature. Put a +1/+1 counter on target creature. Create a 1/1 white Soldier creature token.", None),
        card("Bear", "Creature — Bear", "", Some((2, 2))),
    ];
    game::play::check(&defs[0]).unwrap();
    let mut g = Game::new(defs, 2, 20);
    let drawn = g.put(0, 1, Zone::Library);
    let bear = g.put(1, 1, Zone::Battlefield);
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Player(1)], vec![Target::Object(bear)], vec![Target::Object(bear)]] }).unwrap();
    resolve(&mut g);
    cite!(["608.2c"]; assert_eq!(g.players[0].life, 22));
    cite!(["120.3a"]; assert_eq!(g.players[1].life, 18));
    assert_eq!(g.obj(drawn).zone, Zone::Gone);
    assert_eq!(g.players[0].hand.len(), 1);
    assert!(g.obj(bear).tapped);
    assert_eq!(g.obj(bear).counters["+1/+1"], 1);
    assert_eq!(g.battlefield.iter().filter(|o| g.obj(**o).token()).count(), 1);
    assert!(g.resolving().is_none());
}

#[test]
fn r616_1_child_resume_never_repeats_earlier_draw_or_life_and_clones_keep_the_request() {
    let defs = vec![
        card("Sequence", "Sorcery", "Draw a card. You gain 3 life. Put a +1/+1 counter on target creature. You gain 2 life.", None),
        card("Bear", "Creature — Bear", "", Some((2, 2))),
        card("Hardened Scales", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.", None),
        card("Branching Evolution", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.", None),
    ];
    for def in &defs { game::play::check(def).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    g.put(0, 1, Zone::Library);
    g.put(0, 1, Zone::Library);
    let bear = g.put(0, 1, Zone::Battlefield);
    let scales = g.put(0, 2, Zone::Battlefield);
    let evolution = g.put(0, 3, Zone::Battlefield);
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(bear)]] }).unwrap();
    resolve(&mut g);
    cite!(["616.1"]; assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::Replacement { .. }, .. }))));
    assert_eq!(g.players[0].life, 23);
    assert_eq!(g.players[0].hand.len(), 1);
    crate::support::refused(g.act(g.priority, Move::Pass), "a decision is pending: a choice of the effect being applied (608.2d)");
    let mut other_order = g.clone();
    for (game, first, count) in [(&mut g, scales, 4), (&mut other_order, evolution, 3)] {
        game.act(0, Move::Choose(Answer::Replacement(ReplacementId::from_ability(first, game::binding::AbilityKey::printed(0), 0)))).unwrap();
        assert_eq!(game.players[0].life, 25);
        assert_eq!(game.players[0].hand.len(), 1);
        assert_eq!(game.players[0].library.len(), 1);
        cite!(["616.1f"]; assert_eq!(game.obj(bear).counters["+1/+1"], count));
        assert_eq!(game.log.iter().filter(|e| matches!(e, Event::CountersPut { .. })).count(), 1);
        assert!(game.resolving().is_none());
    }
}

#[test]
fn r701_8_mass_destruction_and_its_rider_compose_with_later_instructions() {
    for (text, survives) in [
        ("Destroy all creatures. You gain 1 life.", true),
        ("Destroy all creatures. They can't be regenerated. You gain 1 life.", false),
    ] {
        let defs = vec![
            card("Sweep", "Sorcery", text, None),
            card("Bear", "Creature — Bear", "", Some((2, 2))),
            card("Survivor", "Creature — Golem", "Indestructible", Some((3, 3))),
        ];
        for def in &defs { game::play::check(def).unwrap(); }
        let mut g = Game::new(defs, 2, 20);
        let shielded = g.put(0, 1, Zone::Battlefield);
        let bear = g.put(1, 1, Zone::Battlefield);
        let survivor = g.put(1, 2, Zone::Battlefield);
        g.regenerate(shielded);
        let spell = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
        resolve(&mut g);
        cite!(["701.19a", "701.19c"]; case: text; assert_eq!(g.obj(shielded).zone == Zone::Battlefield, survives));
        assert_eq!(g.obj(bear).zone, Zone::Gone);
        cite!(["702.12b"]; case: text; assert_eq!(g.obj(survivor).zone, Zone::Battlefield));
        assert_eq!(g.players[0].life, 21);
    }
}

#[test]
fn r614_12_created_result_is_delivered_after_entry_choices_without_replaying_creation() {
    let defs = vec![
        card("Create and tap", "Sorcery", "You gain 2 life. Create a 1/1 white Soldier creature token. Tap it.", None),
        card("Grumgully, the Generous", "Legendary Creature — Goblin Shaman", "Each other non-Human creature you control enters with an additional +1/+1 counter on it.", Some((3, 3))),
        card("Hardened Scales", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.", None),
        card("Branching Evolution", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.", None),
    ];
    for d in &defs { game::play::check(d).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    g.put(0, 1, Zone::Battlefield);
    let scales = g.put(0, 2, Zone::Battlefield);
    let evolution = g.put(0, 3, Zone::Battlefield);
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
    resolve(&mut g);
    assert_eq!(g.players[0].life, 22);
    assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::Replacement { .. }, .. })));
    assert!(!g.battlefield.iter().any(|o| g.obj(*o).token()));
    cite!(["614.12"]; refused("a decision is pending: a choice of how the permanent enters the battlefield (614.12)"): g.act(0, Move::Pass));
    let mut other = g.clone();
    for (game, first, count) in [(&mut g, scales, 4), (&mut other, evolution, 3)] {
        game.act(0, Move::Choose(Answer::Replacement(ReplacementId::from_ability(first, game::binding::AbilityKey::printed(0), 0)))).unwrap();
        let tokens: Vec<_> = game.battlefield.iter().filter(|o| game.obj(**o).token()).copied().collect();
        assert_eq!(tokens.len(), 1);
        // Uncited (card text): "Create a 1/1 white Soldier creature token. Tap it.": "it" is the token just created; 400.7j covers objects moved to a public zone, not created ones
        assert!(game.obj(tokens[0]).tapped, "the later instruction reads the completed creation's result");
        cite!(["614.1c", "616.1f"]; assert_eq!(game.obj(tokens[0]).counters["+1/+1"], count));
        assert_eq!(game.players[0].life, 22);
        assert_eq!(game.log.iter().filter(|e| matches!(e, Event::Created { .. })).count(), 1);
        assert!(game.resolving().is_none());
    }
}
