//! Static player hexproof
//! (702.11c) and shroud (702.18a) in the cases r702_player_targeting_scope leaves out.
use crate::support::cite;
use game::select::Ctx;
use game::state::{Answer, Decision};
use game::{CardDef, Game, Move, Target, Zone};

fn def(name: &str, kind: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    let d = CardDef::new(name, "{0}", kind, text, pt).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{text}: {}", e.0));
    d
}
fn resolve(g: &mut Game) {
    for _ in 0..20 { if g.decision.is_none() && g.stack.is_empty() { return } let p = g.priority; g.act(p, Move::Pass).unwrap(); }
}
/// May a spell of `by`'s target player `at`? The source object is only the spell's identity.
fn may(g: &Game, by: usize, at: usize) -> bool { g.can_target(&Ctx::for_spell(by, g.battlefield[0]), Target::Player(at)) }

/// The ability's controller is who activated it, not its source's current controller
/// (113.8), so the holder's own ability stays legal after an opponent steals the source.
#[test]
fn a_holders_ability_targeting_itself_stays_legal_when_the_source_is_stolen() {
    let defs = vec![
        def("Pinger", "Creature — Human", "{0}: This creature deals 1 damage to any target.", Some((1, 1))),
        def("Ward", "Enchantment", "You have hexproof.", None),
        def("Steal", "Instant", "Gain control of target creature.", None),
    ];
    let mut g = Game::new(defs, 2, 20);
    let pinger = g.put(1, 0, Zone::Battlefield);
    g.put(1, 1, Zone::Battlefield);
    (g.priority, g.active) = (1, 1);
    g.act(1, Move::Activate { source: pinger, ability: 0, targets: vec![vec![Target::Player(1)]], paying: vec![] }).unwrap();
    let steal = g.put(0, 2, Zone::Hand);
    g.act(1, Move::Pass).unwrap();
    g.act(0, Move::Cast { card: steal, targets: vec![vec![Target::Object(pinger)]] }).unwrap();
    resolve(&mut g);
    assert_eq!(g.controller(pinger), 0);
    cite!(["113.8"]; assert_eq!(g.players[1].life, 19));
}

#[test]
fn every_opponent_is_blocked_in_multiplayer_and_only_the_holder_is_protected() {
    let mut g = Game::new(vec![def("Ward", "Enchantment", "You have hexproof.", None)], 3, 20);
    g.put(1, 0, Zone::Battlefield);
    cite!(["702.11c"]; assert!(!may(&g, 0, 1)));
    cite!(["702.11c"]; assert!(!may(&g, 2, 1)));
    cite!(["702.11c"]; assert!(may(&g, 1, 1)));
    assert!(may(&g, 2, 0));
}

/// Admitted forms that reach the same restriction: a quoted grant on a creature, a copy,
/// and existing static conditions, leading or trailing.
#[test]
fn quoted_grants_copies_and_conditions_are_enforced() {
    let defs = vec![
        def("Quoted", "Enchantment", "Creatures you control have \"You have shroud.\"", None),
        def("Bear", "Creature — Bear", "", Some((2, 2))),
        def("Clone", "Creature — Shapeshifter", "You may have this creature enter as a copy of any creature on the battlefield.", Some((0, 0))),
        def("Believer", "Creature — Human", "You have shroud.", Some((2, 2))),
        def("Turn", "Enchantment", "During your turn, you have hexproof.", None),
        def("Human", "Enchantment", "You have hexproof as long as you control a Human.", None),
    ];
    let mut g = Game::new(defs.clone(), 2, 20);
    g.put(1, 0, Zone::Battlefield);
    assert!(may(&g, 0, 1), "no creature holds the quoted ability yet");
    g.put(1, 1, Zone::Battlefield);
    cite!(["702.18a"]; assert!(!may(&g, 0, 1) && !may(&g, 1, 1)));

    // P0's copy of P1's True Believer gives P0 shroud as well.
    let mut g = Game::new(defs.clone(), 2, 20);
    let believer = g.put(1, 3, Zone::Battlefield);
    let clone = g.put(0, 2, Zone::Hand);
    g.act(0, Move::Cast { card: clone, targets: vec![] }).unwrap();
    while !matches!(g.decision, Some(Decision::Choose { .. })) { let p = g.priority; g.act(p, Move::Pass).unwrap(); }
    g.act(0, Move::Choose(Answer::Objects(vec![believer]))).unwrap();
    resolve(&mut g);
    cite!(["707.2", "702.18a"]; assert!(!may(&g, 0, 0) && !may(&g, 1, 0) && !may(&g, 0, 1)));

    let mut g = Game::new(defs.clone(), 2, 20);
    g.put(1, 4, Zone::Battlefield);
    g.active = 0;
    assert!(may(&g, 0, 1), "not P1's turn");
    g.active = 1;
    cite!(["702.11c"]; assert!(!may(&g, 0, 1)));

    let mut g = Game::new(defs, 2, 20);
    g.put(1, 5, Zone::Battlefield);
    assert!(may(&g, 0, 1), "no Human");
    g.put(1, 3, Zone::Battlefield);
    cite!(["702.11c"]; assert!(!may(&g, 0, 1)));
}

#[test]
fn compound_parameterized_and_opponent_player_grants_refuse() {
    for text in ["You have hexproof and shroud.", "You have hexproof from blue.", "Your opponents have hexproof."] {
        let d = CardDef::new("Scope", "{0}", "Enchantment", text, None).unwrap();
        let reason=match text {
            "You have hexproof and shroud."=>"player keyword: only hexproof, shroud or protection is implemented",
            "You have hexproof from blue."=>"player keyword: only unparameterized hexproof or shroud, or protection, is implemented",
            _=>"player characteristic: only static you-have hexproof/shroud is implemented",
        };
        crate::support::refused(game::play::check(&d).map_err(|e|e.0),reason);
    }
}
