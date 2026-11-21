//! Permanent status belongs to battlefield objects, with explicit last-known snapshots (110.5).
use crate::support::cite;
use game::select::Ctx;
use game::{CardDef, Game, Move, Target, Zone};
use grammar::kernel::Outcome;

fn condition(text: &str) -> ir::Condition {
    let normalized = oracle::normalize::normalize(text, "Witness", "Witness", "Creature — Bear");
    let Outcome::Accepted { value, .. } =
        grammar::condition::conditions().condition.parse_all(&oracle::token::tokenize(&normalized.rows[0]))
    else {
        panic!("{text}")
    };
    value
}
fn status(g: &Game, source: game::ObjId, text: &str) -> bool {
    game::condition::holds(g, &Ctx::new(g.obj(source).controller, source), &condition(text)).unwrap()
}
fn pass(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
#[test]
fn r110_5d_cards_outside_the_battlefield_are_neither_tapped_nor_untapped() {
    let d = CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap();
    let mut g = Game::new(vec![d], 2, 20);
    for zone in [Zone::Hand, Zone::Library, Zone::Graveyard, Zone::Exile, Zone::Stack] {
        let card = g.put(0, 0, zone);
        cite!(["110.5d"]; case: format!("{zone:?}"); assert!(!status(&g, card, "this card is tapped"), "{zone:?}"));
        cite!(["110.5d"]; case: format!("{zone:?}"); assert!(!status(&g, card, "this card is untapped"), "{zone:?}"));
    }
    let permanent = g.put(0, 0, Zone::Battlefield);
    assert!(status(&g, permanent, "this creature is untapped"));
    assert!(!status(&g, permanent, "this creature is tapped"));
    g.tap(permanent);
    assert!(!status(&g, permanent, "this creature is untapped"));
    assert!(status(&g, permanent, "this creature is tapped"));
}
#[test]
fn r110_5d_a_spell_does_not_gain_untapped_status_from_its_storage_default() {
    let d = CardDef::new("Query", "{0}", "Instant", "If this spell is untapped, you gain 3 life.", None).unwrap();
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d], 2, 20);
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    pass(&mut g);
    cite!(["110.5d"]; assert_eq!(g.players[0].life, 20));
}
#[test]
fn r603_10a_a_death_trigger_reads_last_known_tapped_status_not_the_new_cards_status() {
    let defs = vec![
        CardDef::new("Watcher", "{0}", "Enchantment", "Whenever a tapped creature dies, you gain 1 life.", None)
            .unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap();
    }
    let mut g = Game::new(defs, 2, 20);
    g.put(0, 0, Zone::Battlefield);
    for tapped in [false, true] {
        let bear = g.put(0, 1, Zone::Battlefield);
        if tapped {
            g.tap(bear);
        }
        g.destroy(bear);
        let card = *g.players[0].graveyard.last().unwrap();
        assert!(!status(&g, card, "this card is tapped"));
        assert!(!status(&g, card, "this card is untapped"));
        g.act(g.priority, Move::Pass).unwrap();
        cite!(["603.10a"]; case: tapped; assert_eq!(g.stack.len(), usize::from(tapped)));
        if tapped {
            pass(&mut g);
        }
    }
    assert_eq!(g.players[0].life, 21);
}
#[test]
fn r110_5c_changing_a_creatures_types_preserves_its_tapped_status() {
    let defs = vec![
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Reshape", "{0}", "Instant", "Target creature becomes an artifact until end of turn.", None)
            .unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap();
    }
    let mut g = Game::new(defs, 2, 20);
    let bear = g.put(0, 0, Zone::Battlefield);
    g.tap(bear);
    let spell = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(bear)]] }).unwrap();
    pass(&mut g);
    assert!(g.is_a(bear, ir::CardType::Artifact));
    assert!(!g.is_a(bear, ir::CardType::Creature));
    cite!(["110.5c"]; assert!(status(&g, bear, "this permanent is tapped")));
    assert!(!status(&g, bear, "this permanent is untapped"));
}

#[test]
fn r608_2h_a_departed_ability_source_retains_its_last_battlefield_status() {
    for tapped in [false, true] {
        let d = CardDef::new(
            "Memory",
            "{0}",
            "Creature — Human",
            "{0}: If this creature is tapped, you gain 1 life.",
            Some((2, 2)),
        )
        .unwrap();
        game::play::check(&d).unwrap();
        let mut g = Game::new(vec![d], 2, 20);
        let source = g.put(0, 0, Zone::Battlefield);
        if tapped {
            g.tap(source);
        }
        g.act(0, Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }).unwrap();
        g.destroy(source);
        pass(&mut g);
        cite!(["608.2h"]; case: tapped; assert_eq!(g.players[0].life, 20 + i32::from(tapped)));
    }
}
