//! Awaken (702.113a): "Awaken N—[cost]" means "You may pay [cost] rather than pay this spell's
//! mana cost as you cast this spell" and "If this spell's awaken cost was paid, put N +1/+1
//! counters on target land you control. That land becomes a 0/0 Elemental creature with
//! haste. It's still a land." Its target is chosen only if that cost is paid (702.113b). Every
//! card is read from its Oracle text.
use crate::support::{cite, mana_pool, oracle, paid};
use game::announcement::CastCost;
use game::{Game, Move, ObjId, Target, Zone};
use ir::{CardType, Part};

const DISCOVERY: usize = 0;
const CLUTCH: usize = 1;
const OUTBURST: usize = 2;
const BEARS: usize = 3;
const FOREST: usize = 4;

/// Coastal Discovery ({3}{U}, "Draw two cards. / Awaken 4—{5}{U}"), Clutch of Currents ({U},
/// "Return target creature to its owner's hand. / Awaken 3—{4}{U}"), Planar Outburst
/// ({3}{W}{W}, "Destroy all nonland creatures. / Awaken 4—{5}{W}{W}{W}"), Grizzly Bears and a
/// Forest player 0 controls, in player 0's precombat main phase.
fn setup() -> (Game, ObjId) {
    let defs: Vec<_> = ["Coastal Discovery", "Clutch of Currents", "Planar Outburst", "Grizzly Bears", "Forest"]
        .into_iter().map(oracle).collect();
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..8 { g.put(p, FOREST, Zone::Library); } }
    let forest = g.put(0, FOREST, Zone::Battlefield);
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    (g, forest)
}
/// Player 0 casts this card from their hand with this mana, for its awaken cost (its second
/// row) or its mana cost.
fn cast(g: &mut Game, card: usize, mana: &str, awaken: bool, targets: Vec<Vec<Target>>) -> Result<ObjId, String> {
    let object = g.put(0, card, Zone::Hand);
    g.players[0].pool = mana_pool(mana);
    let cast = Move::Cast { card: object, targets };
    let cast = if awaken { Move::CastFor(CastCost::Alternative(1), Box::new(cast)) } else { cast };
    g.act(0, paid(mana, cast)).map(|_| object).map_err(|e| e.to_string())
}
fn resolve(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
fn plus(g: &Game, o: ObjId) -> u32 { g.counters(o).get("+1/+1").copied().unwrap_or(0) }
/// A land creature and an Elemental, as its characteristics are now (613.1d).
fn awakened(g: &Game, land: ObjId) -> bool {
    let seen = g.seen(land, None);
    g.is_a(land, CardType::Creature) && g.is_a(land, CardType::Land) && seen.subtypes.iter().any(|s| s.name() == "Elemental")
}

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Coastal Discovery", "Clutch of Currents", "Planar Outburst", "Ruinous Path", "Part the Waterveil", "Mire's Malice", "Rush of Ice"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

#[test]
fn r702_113a_cast_for_its_awaken_cost_a_land_becomes_an_elemental_with_counters() {
    let (mut g, forest) = setup();
    let hand = g.players[0].hand.len();
    cast(&mut g, DISCOVERY, "UUUUUU", true, vec![vec![Target::Object(forest)]]).unwrap();
    resolve(&mut g);
    cite!(["702.113a"]; assert_eq!(g.players[0].hand.len(), hand + 2));
    cite!(["702.113a", "122.1a"]; assert_eq!(plus(&g, forest), 4));
    cite!(["702.113a"]; assert!(awakened(&g, forest)));
    // a 0/0 with four +1/+1 counters, with haste
    cite!(["702.113a", "613.4b", "613.4c"]; assert_eq!((g.power(forest), g.toughness(forest)), (Some(4), Some(4))));
    cite!(["702.113a"]; assert!(g.has(forest, "Haste")));
    // Ruling: "The land will retain any other types, subtypes, or supertypes it previously had."
    cite!(["702.113a", "205.1b"]; assert!(g.seen(forest, None).subtypes.iter().any(|s| s.name() == "Forest")));
}

#[test]
fn r702_113a_cast_for_its_mana_cost_it_has_only_its_first_effect() {
    // Ruling: "You can cast a spell with awaken for its mana cost and get only its first effect."
    let (mut g, forest) = setup();
    let hand = g.players[0].hand.len();
    // its awaken target group is announced empty
    cast(&mut g, DISCOVERY, "UUUU", false, vec![vec![]]).unwrap();
    resolve(&mut g);
    cite!(["702.113a"]; assert_eq!(g.players[0].hand.len(), hand + 2));
    cite!(["702.113a"]; assert!(!awakened(&g, forest) && plus(&g, forest) == 0));
}

#[test]
fn r702_113b_the_land_is_targeted_only_as_the_awaken_cost_is_paid() {
    let (mut g, forest) = setup();
    // its mana cost paid, the spell is cast as if it didn't have that target
    let said = cast(&mut g, DISCOVERY, "UUUU", false, vec![vec![Target::Object(forest)]]);
    cite!(["702.113b", "601.2c"]; assert_eq!(said, Err("wrong number of targets: 1 chosen, it takes 0 (601.2c)".to_string())));
    // its awaken cost paid, it needs a land its controller controls to target
    let (mut g, _) = setup();
    let said = cast(&mut g, DISCOVERY, "UUUUUU", true, vec![vec![]]);
    cite!(["702.113a", "601.2c"]; assert_eq!(said, Err("wrong number of targets: 0 chosen, it takes 1 (601.2c)".to_string())));
    let (mut g, _) = setup();
    let theirs = g.put(1, FOREST, Zone::Battlefield);
    let said = cast(&mut g, DISCOVERY, "UUUUUU", true, vec![vec![Target::Object(theirs)]]);
    cite!(["702.113a", "115.1"]; assert_eq!(said, Err("an illegal target, Forest: the target is one you control, and another player controls it (115.1)".to_string())));
}

#[test]
fn r608_2b_with_no_other_target_it_does_not_resolve_once_the_land_is_gone() {
    // Ruling: "If the non-awaken part of the spell doesn't require a target and you cast the
    // spell for its awaken cost, then the spell won't resolve if the target land you control
    // becomes illegal before the spell resolves"
    let (mut g, forest) = setup();
    let hand = g.players[0].hand.len();
    cast(&mut g, DISCOVERY, "UUUUUU", true, vec![vec![Target::Object(forest)]]).unwrap();
    g.destroy(forest);
    resolve(&mut g);
    cite!(["608.2b", "702.113a"]; assert_eq!(g.players[0].hand.len(), hand));
}

#[test]
fn r608_2b_with_another_target_the_rest_of_the_spell_still_happens() {
    // Ruling: "If a spell with awaken has multiple targets (including the land you control),
    // and some but not all of those targets become illegal by the time the spell tries to
    // resolve, the spell won't affect the illegal targets in any way."
    let (mut g, forest) = setup();
    let bears = g.put(1, BEARS, Zone::Battlefield);
    let other = g.put(0, FOREST, Zone::Battlefield);
    cast(&mut g, CLUTCH, "UUUUU", true, vec![vec![Target::Object(bears)], vec![Target::Object(forest)]]).unwrap();
    g.destroy(forest);
    resolve(&mut g);
    cite!(["608.2b", "702.113a"]; assert!(!g.battlefield.contains(&bears)
        && g.players[1].hand.iter().any(|o| g.obj(*o).card == Some(BEARS))));
    cite!(["608.2b"]; assert!(!awakened(&g, other) && plus(&g, other) == 0));
}

#[test]
fn r702_113a_the_land_awakens_after_the_spell_first_effect() {
    // Planar Outburst destroys all nonland creatures, then the awakened land becomes a
    // creature; an earlier awakened land is a land and survives too (ruling: "A 'nonland
    // creature' is a creature that isn't also a land.")
    let (mut g, forest) = setup();
    let bears = g.put(1, BEARS, Zone::Battlefield);
    let earlier = g.put(0, FOREST, Zone::Battlefield);
    cast(&mut g, DISCOVERY, "UUUUUU", true, vec![vec![Target::Object(earlier)]]).unwrap();
    resolve(&mut g);
    cast(&mut g, OUTBURST, "WWWWWWWW", true, vec![vec![Target::Object(forest)]]).unwrap();
    resolve(&mut g);
    cite!(["702.113a", "608.2c"]; assert!(!g.battlefield.contains(&bears)));
    cite!(["702.113a", "608.2c"]; assert!(g.battlefield.contains(&forest) && awakened(&g, forest) && plus(&g, forest) == 4));
    cite!(["702.113a", "205.4a"]; assert!(g.battlefield.contains(&earlier) && awakened(&g, earlier)));
}
