//! What a root "other" is other than. With no comparison, explicit actor or earlier
//! target's player to bind it, "other" should exclude the sole compatible announced target
//! (608.2c). Run with `engine::phalanx_other_review::`.
use game::state::Decision;
use game::{CardDef, Game, Move, Target, Zone};

fn pinned(name: &str) -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    CardDef::from_faces(CardDef::catalog(&faces).get(name).unwrap()).unwrap()
}
fn creature(name: &str, p: i32, t: i32) -> CardDef { CardDef::new(name, "{0}", "Creature — Beast", "", Some((p, t))).unwrap() }
fn resolve(g: &mut Game, target: Target) {
    for _ in 0..6 {
        if let Some(Decision::Targets { .. }) = g.decision { g.act(0, Move::ChooseTargets(vec![vec![target]])).unwrap(); continue; }
        if g.stack.is_empty() { break; }
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn pt(g: &Game, o: game::ObjId) -> (Option<i32>, Option<i32>) { (g.power(o), g.toughness(o)) }

/// Regression: the target is one of "other creatures you control", so it gets the anthem too.
#[test]
fn pinned_anthems_exclude_their_target() {
    for (name, target, other) in [("Exhilarating Elocution", (4, 4), (3, 3)), ("Sigil Blessing", (5, 5), (3, 3))] {
        let mut g = Game::new(vec![pinned(name), creature("Bear", 2, 2)], 2, 20);
        let t = g.put(0, 1, Zone::Battlefield);
        let o = g.put(0, 1, Zone::Battlefield);
        g.players[0].pool.colors = [10; 5];
        let c = g.put(0, 0, Zone::Hand);
        g.act(0, crate::support::paid(if name == "Exhilarating Elocution" { "WWWB" } else { "GW" }, Move::Cast { card: c, targets: vec![vec![Target::Object(t)]] })).unwrap();
        resolve(&mut g, Target::Object(t));
        // Uncited (card text): Exhilarating Elocution's "Other creatures you control get +1/+1", Sigil Blessing's "other creatures you control get +1/+1": other than the target
        assert_eq!((pt(&g, t), pt(&g, o)), ((Some(target.0), Some(target.1)), (Some(other.0), Some(other.1))), "{name}");
    }
}

/// Regression: "if that opponent controls no other creatures". The condition should count the
/// opponent's creatures other than the target, not the controller's creatures other than
/// Skulking Killer: read that way, an extra opponent creature is ignored and an extra one of
/// mine blocks the effect.
#[test]
fn skulking_killer_counts_that_opponents_other_creatures() {
    for (theirs, mine, shrunk) in [(0, 0, true), (1, 0, false), (0, 1, true), (1, 1, false)] {
        let mut g = Game::new(vec![pinned("Skulking Killer"), creature("Bear", 3, 3)], 2, 20);
        let t = g.put(1, 1, Zone::Battlefield);
        for _ in 0..theirs { g.put(1, 1, Zone::Battlefield); }
        for _ in 0..mine { g.put(0, 1, Zone::Battlefield); }
        g.players[0].pool.colors = [10; 5];
        let c = g.put(0, 0, Zone::Hand);
        g.act(0, crate::support::paid("WWWB", Move::Cast { card: c, targets: vec![] })).unwrap();
        resolve(&mut g, Target::Object(t));
        let expected = if shrunk { (Some(1), Some(1)) } else { (Some(3), Some(3)) };
        // Uncited (card text): Skulking Killer's "if that opponent controls no other creatures": the target's controller's creatures other than the target
        assert_eq!(pt(&g, t), expected, "opponent's others {theirs}, mine {mine}");
    }
}

/// Synthetic regression: one plural target group is a valid complement. The Set evaluator already
/// excludes every `other_than` member. All chosen members must be excluded.
#[test]
fn single_plural_target_group_is_excluded() {
    let text = "Two target creatures you control each get +2/+1 until end of turn. Each other creature you control gets +1/+1 until end of turn.";
    let d = CardDef::new("Probe", "{0}", "Sorcery", text, None).unwrap();
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d, creature("Bear", 2, 2)], 2, 20);
    let a = g.put(0, 1, Zone::Battlefield);
    let b = g.put(0, 1, Zone::Battlefield);
    let o = g.put(0, 1, Zone::Battlefield);
    let c = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: c, targets: vec![vec![Target::Object(a), Target::Object(b)]] }).unwrap();
    resolve(&mut g, Target::Object(a));
    // Uncited (card text): "Two target creatures you control each get +2/+1 ... Each other creature you control gets +1/+1": other than both targets
    assert_eq!([pt(&g, a), pt(&g, b), pt(&g, o)], [(Some(4), Some(3)), (Some(4), Some(3)), (Some(3), Some(3))]);
}

/// Boundaries that must hold.
#[test]
fn scope_contrasts() {
    let check = |t: &str| game::play::check(&CardDef::new("Probe", "{0}", "Sorcery", t, None).unwrap()).map_err(|e| e.0);
    // A conditional branch is still the root of its sentence: the earlier target is bound.
    let d = CardDef::new("Probe", "{0}", "Sorcery", "Tap target creature an opponent controls. If you control a Wizard, each other creature that player controls gets -2/-0 until end of turn.", None).unwrap();
    assert!(format!("{d:?}").contains("other_than: Some"));
    // Two distinct compatible groups (no union) must not silently pick one; nothing excluded today.
    let two = "Target creature you control gets +2/+1 until end of turn. Target creature an opponent controls gets -1/-0 until end of turn. Each other creature gets +1/+1 until end of turn.";
    crate::support::refused(check(two), "other: ambiguous announced target slots");
    // A mode without its own target can't borrow another mode's target.
    let modal = "Choose one —\n• Target creature you control gets +2/+1 until end of turn.\n• Each other creature you control gets +1/+1 until end of turn.";
    check(modal).unwrap();
    assert!(!format!("{:?}", CardDef::new("Probe", "{0}", "Sorcery", modal, None).unwrap()).contains("other_than: Some"));
}
