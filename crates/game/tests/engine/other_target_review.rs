//! Which object a root "other" set is other than
//! once an earlier target exists (608.2c). Run with `engine::other_target_review::`.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};

fn pinned(name: &str) -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    CardDef::from_faces(CardDef::catalog(&faces).get(name).unwrap()).unwrap()
}
fn spell(text: &str) -> CardDef {
    let d = CardDef::new("Probe", "{0}", "Sorcery", text, None).unwrap();
    game::play::check(&d).map_err(|e| e.0).expect("admitted");
    d
}
fn creature(name: &str, p: i32, t: i32, text: &str) -> CardDef { CardDef::new(name, "{0}", "Creature — Beast", text, Some((p, t))).unwrap() }
fn pass(g: &mut Game) { for _ in 0..3 { if g.stack.is_empty() || g.decision.is_some() { return; } g.act(g.priority, Move::Pass).unwrap(); } }
fn cast(g: &mut Game, card: usize, targets: Vec<Vec<Target>>, spending: &str) {
    g.players[0].pool.colors = [10; 5];
    let c = g.put(0, card, Zone::Hand);
    g.act(0, crate::support::paid(spending, Move::Cast { card: c, targets })).unwrap();
    pass(g);
}
fn to(g: &Game, o: ObjId) -> u32 {
    g.log.iter().filter_map(|e| match e {
        Event::DamageDealt { to, amount, .. } if *to == Target::Object(o) => Some(*amount),
        _ => None,
    }).sum()
}

/// Root-only provenance: the root's own filter, a later sentence, and a deferred condition all
/// keep the target excluded; a negated relation keeps the source default.
#[test]
fn target_exclusion_survives_filters_repeats_and_conditions() {
    for (text, target, same, other_side) in [
        ("Tap target creature an opponent controls. Each other creature that player controls with power 3 or greater gets -2/-0 until end of turn.", 3, 1, 3),
        ("Tap target creature an opponent controls. Each other creature that player controls gets -2/-0 until end of turn. Each other creature that player controls gets -1/-0 until end of turn.", 3, 0, 3),
        ("Tap target creature an opponent controls. Each other creature that player controls gets -2/-0 until end of turn if that player controls two or more creatures.", 3, 1, 3),
        ("Tap target creature an opponent controls. Each other creature that player doesn't control gets -2/-0 until end of turn.", 3, 3, 1),
    ] {
        let mut g = Game::new(vec![spell(text), creature("Bear", 3, 3, "")], 2, 20);
        let t = g.put(1, 1, Zone::Battlefield);
        let s = g.put(1, 1, Zone::Battlefield);
        let o = g.put(0, 1, Zone::Battlefield);
        cast(&mut g, 0, vec![vec![Target::Object(t)]], "");
        // Uncited (card text): each row's "Each other creature that player controls/doesn't control ...": other than the tapped target
        assert_eq!((g.power(t), g.power(s), g.power(o)), (Some(target), Some(same), Some(other_side)), "{text}");
    }
}

/// "Up to one" with no target chosen: there is no "that player", so nothing is affected.
#[test]
fn no_chosen_target_means_no_player_and_no_affected_creatures() {
    let text = "Tap up to one target creature an opponent controls. Each other creature that player controls gets -2/-0 until end of turn.";
    let mut g = Game::new(vec![spell(text), creature("Bear", 3, 3, "")], 2, 20);
    let theirs = g.put(1, 1, Zone::Battlefield);
    let mine = g.put(0, 1, Zone::Battlefield);
    cast(&mut g, 0, vec![vec![]], "");
    // Uncited (ruling 47800b8824082b95): Azog, "If no target is chosen …, "its controller" is undefined and no player amasses": an unchosen target's player is no player
    assert_eq!((g.power(theirs), g.power(mine)), (Some(3), Some(3)));
}

/// "its controller" / "that creature's controller" of a target that may be either player's.
#[test]
fn target_controller_possessive_follows_either_controller() {
    for text in ["Destroy target creature. Each other creature its controller controls gets -2/-0 until end of turn.",
                 "Destroy target creature. Each other creature that creature's controller controls gets -2/-0 until end of turn."] {
        for owner in [0, 1] {
            let mut g = Game::new(vec![spell(text), creature("Iron", 3, 5, "Indestructible"), creature("Bear", 3, 3, "")], 2, 20);
            let t = g.put(owner, 1, Zone::Battlefield);
            let s = g.put(owner, 2, Zone::Battlefield);
            let o = g.put(1 - owner, 2, Zone::Battlefield);
            cast(&mut g, 0, vec![vec![Target::Object(t)]], "");
            // Uncited (card text): "Each other creature its controller controls" / "that creature's controller controls": other than the target, among its controller's
            assert_eq!((g.power(t), g.power(s), g.power(o)), (Some(3), Some(1), Some(3)), "{text} owner {owner}");
        }
    }
}

/// Explicit "with the same name as that creature" comparisons: the target is hit once, and a
/// same-named source is "other" than the target, so it is hit too.
#[test]
fn same_name_comparisons_count_the_target_once() {
    let mut g = Game::new(vec![pinned("Homing Lightning"), creature("Bear", 2, 9, ""), creature("Ox", 2, 9, "")], 2, 20);
    let t = g.put(1, 1, Zone::Battlefield);
    let same = g.put(1, 1, Zone::Battlefield);
    let mine = g.put(0, 1, Zone::Battlefield);
    let ox = g.put(1, 2, Zone::Battlefield);
    cast(&mut g, 0, vec![vec![Target::Object(t)]], "WWRR");
    cite!(["201.2a"]; assert_eq!((to(&g, t), to(&g, same), to(&g, mine), to(&g, ox)), (4, 4, 4, 0)));

    let mut g = Game::new(vec![pinned("Echoing Courage"), creature("Bear", 2, 9, "")], 2, 20);
    let t = g.put(1, 1, Zone::Battlefield);
    let same = g.put(1, 1, Zone::Battlefield);
    cast(&mut g, 0, vec![vec![Target::Object(t)]], "WG");
    cite!(["201.2a"]; assert_eq!((g.power(t), g.power(same)), (Some(4), Some(4))));

    let mut g = Game::new(vec![pinned("Izzet Staticaster"), creature("Bear", 2, 9, "")], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let t = g.put(1, 0, Zone::Battlefield);
    let same = g.put(1, 0, Zone::Battlefield);
    let bear = g.put(1, 1, Zone::Battlefield);
    let ability = (0..4).find(|&a| g.clone().act(0, Move::Activate { source, ability: a, targets: vec![vec![Target::Object(t)]], paying: vec![] }).is_ok()).unwrap();
    g.act(0, Move::Activate { source, ability, targets: vec![vec![Target::Object(t)]], paying: vec![] }).unwrap();
    pass(&mut g);
    cite!(["201.2a"]; assert_eq!((to(&g, t), to(&g, same), to(&g, source), to(&g, bear)), (1, 1, 1, 0)));
}

// Phalanx Tactics in full: the target gets only its own pump.
#[test]
fn phalanx_tactics_target_gets_only_its_own_pump() {
    let mut g = Game::new(vec![pinned("Phalanx Tactics"), creature("Bear", 2, 2, "")], 2, 20);
    let target = g.put(0, 1, Zone::Battlefield);
    let other = g.put(0, 1, Zone::Battlefield);
    cast(&mut g, 0, vec![vec![Target::Object(target)]], "WW");
    // Uncited (card text): Phalanx Tactics' "Each other creature you control gets +1/+1": other than the target
    assert_eq!((g.power(other), g.toughness(other)), (Some(3), Some(3)));
    // Uncited (card text): "Target creature you control gets +2/+1", and it is not "each other creature"
    assert_eq!((g.power(target), g.toughness(target)), (Some(4), Some(3)));
}
