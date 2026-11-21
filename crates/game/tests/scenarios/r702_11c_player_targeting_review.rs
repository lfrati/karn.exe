//! A static "You have hexproof/shroud"
//! must mean exactly the existing explicit restriction (702.11c, 702.18a). That restriction
//! already enforces announcement, the resolution recheck (608.2b), control change and ability
//! loss. Run with `r702::r702_11c_player_targeting_review::`.
use crate::support::cite;
use game::{CardDef, Game, Move, Target, Zone};

const HEXPROOF: &str = "You can't be the target of spells or abilities your opponents control.";
const SHROUD: &str = "You can't be the target of spells or abilities.";

/// Index 0 Bolt, 1 Ward (the tested static), 2 Mogg, 3 Fates, 4 Bear, 5 Steal, 6 Artifact, 7 Jest.
fn game(ward: &str) -> Game {
    let defs = vec![
        CardDef::new("Bolt", "{0}", "Instant", "This spell deals 3 damage to any target.", None).unwrap(),
        CardDef::new("Ward", "{0}", "Creature — Human", ward, Some((2, 2))).unwrap(),
        CardDef::new("Mogg", "{0}", "Sorcery", "Destroy all creatures target player controls. For each creature that died this way, that player creates two 1/1 red Goblin creature tokens.", None).unwrap(),
        CardDef::new("Fates", "{0}", "Sorcery", "Destroy target nonland permanent. Put a +1/+1 counter on each creature target player controls.", None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Steal", "{0}", "Instant", "Gain control of target permanent.", None).unwrap(),
        CardDef::new("Artifact", "{0}", "Artifact", "", None).unwrap(),
        CardDef::new("Jest", "{0}", "Instant", "Until end of turn, each creature target player controls loses all abilities and becomes a blue Frog with base power and toughness 1/1.", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    g.players[0].pool.colors = [10; 5];
    g.players[1].pool.colors = [10; 5];
    g
}
fn resolve(g: &mut Game) {
    for _ in 0..20 { if g.decision.is_none() && g.stack.is_empty() { return } let p = g.priority; g.act(p, Move::Pass).unwrap(); }
}
fn bolt_ok(g: &mut Game, by: usize, at: usize) -> bool {
    let card = g.put(by, 0, Zone::Hand);
    (g.priority, g.active) = (by, by);
    g.act(by, Move::Cast { card, targets: vec![vec![Target::Player(at)]] }).is_ok()
}

/// Everything the explicit and keyword forms must agree on. The source, a creature, is P1's.
fn contract(hexproof: &str, shroud: &str) {
    // Hexproof: an opponent can't target; the holder can. Shroud: neither can.
    let mut g = game(hexproof); g.put(1, 1, Zone::Battlefield);
    cite!(["702.11c"]; assert!(!bolt_ok(&mut g, 0, 1), "{hexproof}: opponent targeted the holder"));
    let mut g = game(hexproof); g.put(1, 1, Zone::Battlefield);
    cite!(["702.11c"]; assert!(bolt_ok(&mut g, 1, 1), "{hexproof}: holder could not target itself"));
    let mut g = game(shroud); g.put(1, 1, Zone::Battlefield);
    cite!(["702.18a"]; assert!(!bolt_ok(&mut g, 1, 1), "{shroud}: holder targeted itself"));

    // Gained before resolution (608.2b): Mogg's only target is illegal, so it does nothing.
    let mut g = game(hexproof); let bear = g.put(1, 4, Zone::Battlefield);
    let card = g.put(0, 2, Zone::Hand); g.act(0, Move::Cast { card, targets: vec![vec![Target::Player(1)]] }).unwrap();
    g.put(1, 1, Zone::Battlefield); resolve(&mut g);
    cite!(["608.2b"]; assert!(g.battlefield.contains(&bear), "{hexproof}: Mogg resolved against an illegal player"));
    assert_eq!(g.battlefield.iter().filter(|o| g.obj(**o).token()).count(), 0);
    // Twisted Fates keeps its legal permanent target but puts no counters for the illegal player.
    let mut g = game(hexproof); let bear = g.put(1, 4, Zone::Battlefield); let artifact = g.put(1, 6, Zone::Battlefield);
    let card = g.put(0, 3, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(artifact)], vec![Target::Player(1)]] }).unwrap();
    g.put(1, 1, Zone::Battlefield); resolve(&mut g);
    assert!(!g.battlefield.contains(&artifact));
    cite!(["608.2b"]; assert_eq!(g.power(bear), Some(2), "{hexproof}: counters for an illegal player target"));

    // The source's current controller holds it: after P0 steals it, P0 is protected and P1 isn't.
    let mut g = game(hexproof); let ward = g.put(1, 1, Zone::Battlefield);
    let card = g.put(0, 5, Zone::Hand); g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(ward)]] }).unwrap(); resolve(&mut g);
    cite!(["702.11c"]; assert!(bolt_ok(&mut g.clone(), 0, 1), "{hexproof}: old controller still protected"));
    cite!(["702.11c"]; assert!(!bolt_ok(&mut g, 1, 0), "{hexproof}: new controller not protected"));

    // A source that loses all abilities no longer protects anyone.
    let mut g = game(hexproof); g.put(1, 1, Zone::Battlefield);
    let card = g.put(1, 7, Zone::Hand); (g.priority, g.active) = (1, 1);
    g.act(1, Move::Cast { card, targets: vec![vec![Target::Player(1)]] }).unwrap(); resolve(&mut g);
    cite!(["604.2"]; assert!(bolt_ok(&mut g, 0, 1), "{hexproof}: protection survived ability loss"));
}

/// The explicit restriction route has the whole contract.
#[test]
fn explicit_player_targeting_restrictions_already_enforce_the_contract() {
    contract(HEXPROOF, SHROUD);
}

/// The keyword forms must not be admitted as no-ops: they are identical to the
/// explicit restriction above. Nor may they be refused: 7 pinned cards depend on the
/// "You have hexproof/shroud." form.
#[test]
fn static_you_have_hexproof_and_shroud_match_the_explicit_restriction() {
    contract("You have hexproof.", "You have shroud.");
}
