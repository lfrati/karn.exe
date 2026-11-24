//! Review of moving counters (122.5) and Graft (702.58a), for what their own tests don't check:
//! that Graft watches every other creature, of any player, and no other permanent; that it
//! doesn't target; that it checks its counter again as it resolves (603.4); and that a move is
//! a removal and a putting, so that nothing is removed when the counter can't be put, and what
//! watches counters being put sees it. Every card is read from its Oracle text.
use crate::support::*;
use game::state::Decision;

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Simic Initiate", "Llanowar Reborn", "Daghatar the Adamant", "Weapon Rack", "Cytoplast Root-Kin", "Afiya Grove", "Explorer's Cache",
        "Cytoplast Manipulator", "Fate Transfer", "Gatewatch Beacon", "Melira's Keepers", "Herd Baloth", "Fathom Mage", "Hardened Scales",
        "Gladecover Scout", "Raise the Alarm", "Jace Beleren", "Ambush Viper", "Shock", "Lightning Bolt", "Grizzly Bears", "Ornithopter", "Sol Ring"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

fn plus(t: &T, o: ObjId) -> u32 { t.g.counters(o).get("+1/+1").copied().unwrap_or(0) }

/// The printed activated abilities of this permanent, by index.
fn activated(t: &T, o: ObjId) -> Vec<usize> {
    t.g.abilities(o).iter().enumerate().filter(|(_, a)| matches!(&***a, ir::Ability::Activated { .. })).map(|(i, _)| i).collect()
}

/// Player `p` casts this card from their hand, paying `cost`; everyone passes until it is
/// resolved and any trigger is on the stack.
fn cast(t: &mut T, p: usize, name: &str, cost: &str) {
    let pool: String = cost.split('{').filter(|s| !s.is_empty()).map(|s| s.trim_end_matches('}'))
        .map(|s| s.parse::<usize>().map_or(s.to_string(), |n| "C".repeat(n))).collect();
    t.mana(p, cost).cast(p, name, vec![], mana_pool(&pool)).unwrap().pass_all();
}

/// Resolve the stack, answering each "you may" with `yes`.
fn resolve(t: &mut T, yes: bool) {
    for _ in 0..20 {
        if let Some(Decision::Choose { player, .. }) = t.g.decision.clone() {
            t.g.act(player, Move::Choose(if yes { Answer::Yes } else { Answer::No })).unwrap();
            continue;
        }
        if t.g.stack.is_empty() { return; }
        t.pass_all();
    }
    panic!("the stack did not resolve: {:?}", t.g.decision);
}

// Rules: 702.58a, 603.2, 115.1
// Given: player 0's Simic Initiate, with its +1/+1 counter.
// When: player 0 casts Sol Ring; then, in player 1's turn, player 1 casts Gladecover Scout
// (hexproof).
// Expect: an artifact entering is no creature and Graft doesn't trigger; another player's
// creature is "another creature", and Graft, which doesn't target, moves the counter onto the
// Scout though it has hexproof. The Initiate, a 0/0 left with none, dies.
#[test]
fn r702_58a_graft_watches_every_other_creature_and_targets_none() {
    let mut t = Board::new(2).fill(8).hand(0, &["Simic Initiate", "Sol Ring"]).hand(1, &["Gladecover Scout"]).build();
    cast(&mut t, 0, "Simic Initiate", "{G}");
    let initiate = t.find_in_battlefield("Simic Initiate");
    cast(&mut t, 0, "Sol Ring", "{1}");
    cite!(["702.58a"]; assert!(t.g.stack.is_empty(), "no creature entered"));
    t.to_no_combat(1, Part::PrecombatMain);
    cast(&mut t, 1, "Gladecover Scout", "{G}");
    cite!(["702.58a", "603.2"]; assert_eq!(t.g.stack.len(), 1));
    resolve(&mut t, true);
    let scout = t.find_in_battlefield("Gladecover Scout");
    cite!(["702.58a", "115.1"]; assert_eq!((plus(&t, scout), t.g.controller(scout)), (1, 1)));
    cite!(["704.5f"]; assert!(!t.g.battlefield.contains(&initiate)));
}

// Rules: 122.5, 702.58a, 608.2h
// Given: player 0's Simic Initiate with its counter.
// When: player 0 casts Grizzly Bears, and Shocks it with Graft's trigger on the stack.
// Expect: the creature the counter would move onto is gone, so nothing moves: the Initiate keeps
// its counter and lives.
#[test]
fn r122_5_graft_moves_nothing_onto_a_creature_that_has_left() {
    let mut t = Board::new(2).fill(8).hand(0, &["Simic Initiate", "Grizzly Bears", "Shock"]).build();
    cast(&mut t, 0, "Simic Initiate", "{G}");
    let initiate = t.find_in_battlefield("Simic Initiate");
    cast(&mut t, 0, "Grizzly Bears", "{1}{G}");
    let bears = t.find_in_battlefield("Grizzly Bears");
    t.mana(0, "{R}").cast(0, "Shock", vec![Tgt::Object(bears)], mana_pool("R")).unwrap();
    resolve(&mut t, true);
    t.expect().rules(["122.5"]).zone(0, Zone::Graveyard, &["Grizzly Bears", "Shock"]).check();
    cite!(["122.5"]; assert_eq!(plus(&t, initiate), 1));
}

// Rules: 603.4, 702.58a, 122.5
// Given: player 0's Llanowar Reborn, a land with Graft 1, with its counter.
// When: player 0 casts Grizzly Bears; with its Graft trigger on the stack, casts Ambush Viper
// (flash), and moves the counter onto the Viper; then casts Ornithopter.
// Expect: the trigger for the Bears finds no counter as it resolves and does nothing; with none,
// Graft doesn't trigger for the Ornithopter at all.
#[test]
fn r603_4_graft_checks_its_counter_as_it_triggers_and_as_it_resolves() {
    let mut t = Board::new(2).fill(8).hand(0, &["Grizzly Bears", "Ambush Viper", "Ornithopter"]).battlefield(0, &["Llanowar Reborn"]).build();
    let reborn = t.find_in_battlefield("Llanowar Reborn");
    t.g.put_counters(Target::Object(reborn), "+1/+1", 1);
    cast(&mut t, 0, "Grizzly Bears", "{1}{G}");
    cite!(["702.58a", "603.2"]; assert_eq!(t.g.stack.len(), 1));
    cast(&mut t, 0, "Ambush Viper", "{1}{G}");
    cite!(["702.58a", "603.4"]; assert_eq!(t.g.stack.len(), 2));
    resolve(&mut t, true);
    let (bears, viper) = (t.find_in_battlefield("Grizzly Bears"), t.find_in_battlefield("Ambush Viper"));
    cite!(["603.4", "122.5"]; assert_eq!((plus(&t, reborn), plus(&t, viper), plus(&t, bears)), (0, 1, 0)));
    cast(&mut t, 0, "Ornithopter", "");
    cite!(["603.4"]; assert!(t.g.stack.is_empty()));
}

// Rules: 603.2c, 603.3b, 702.58a, 122.5
// Given: player 0's Simic Initiate with its counter.
// When: player 0 casts Raise the Alarm, which creates two Soldier tokens at once.
// Expect: Graft triggers for each; the first moves the counter, the second finds none.
#[test]
fn r603_2c_graft_triggers_once_for_each_creature_entering_together() {
    // Raise the Alarm: "Create two 1/1 white Soldier creature tokens."
    let mut t = Board::new(2).fill(8).hand(0, &["Simic Initiate", "Raise the Alarm"]).build();
    cast(&mut t, 0, "Simic Initiate", "{G}");
    cast(&mut t, 0, "Raise the Alarm", "{1}{W}");
    // its controller puts the two on the stack in an order they choose (603.3b)
    let Some(Decision::Order { player: 0, abilities }) = t.g.decision.clone() else { panic!("{:?}", t.g.decision) };
    cite!(["603.2c", "702.58a"]; assert_eq!(abilities.len(), 2));
    t.g.act(0, Move::Order(vec![0, 1])).unwrap();
    resolve(&mut t, true);
    let soldiers: Vec<u32> = t.g.battlefield.iter().filter(|o| t.g.obj(**o).chars.name.contains("Soldier")).map(|o| plus(&t, *o)).collect();
    cite!(["603.4", "122.5"]; assert_eq!({ let mut s = soldiers; s.sort(); s }, [0, 1]));
}

// Rules: 122.5, 702.58a
// Given: player 0's Simic Initiate with its counter, Daghatar the Adamant with four, and Weapon
// Rack with three.
// When: player 0 casts Melira's Keepers ("This creature can't have counters put on it.") and
// says yes to Graft; then moves a counter from Daghatar onto the Keepers, and taps Weapon Rack
// to move one onto it.
// Expect: none can be put onto the Keepers, so none is removed from the Initiate, Daghatar or
// the Rack.
#[test]
fn r122_5_nothing_is_removed_when_the_counter_cant_be_put() {
    let mut t = Board::new(2).fill(8).hand(0, &["Simic Initiate", "Melira's Keepers"]).battlefield(0, &["Daghatar the Adamant", "Weapon Rack"]).build();
    let (daghatar, rack) = (t.find_in_battlefield("Daghatar the Adamant"), t.find_in_battlefield("Weapon Rack"));
    t.g.put_counters(Target::Object(daghatar), "+1/+1", 4);
    t.g.put_counters(Target::Object(rack), "+1/+1", 3);
    cast(&mut t, 0, "Simic Initiate", "{G}");
    let initiate = t.find_in_battlefield("Simic Initiate");
    cast(&mut t, 0, "Melira's Keepers", "{4}{G}");
    resolve(&mut t, true);
    let keepers = t.find_in_battlefield("Melira's Keepers");
    cite!(["122.5", "702.58a"]; assert_eq!((plus(&t, initiate), plus(&t, keepers)), (1, 0)));
    t.mana(0, "{1}{G}{G}");
    t.g.act(0, paid("CGG", Move::Activate { source: daghatar, ability: activated(&t, daghatar)[0],
        targets: vec![vec![Target::Object(daghatar)], vec![Target::Object(keepers)]], paying: vec![] })).unwrap();
    resolve(&mut t, true);
    cite!(["122.5"]; assert_eq!((plus(&t, daghatar), plus(&t, keepers)), (4, 0)));
    // Weapon Rack's ruling: "If the creature ... can't have a +1/+1 counter put onto it for some
    // other reason, you won't remove a +1/+1 counter from Weapon Rack."
    t.g.act(0, Move::Activate { source: rack, ability: activated(&t, rack)[0], targets: vec![vec![Target::Object(keepers)]], paying: vec![] }).unwrap();
    resolve(&mut t, true);
    cite!(["122.5"]; assert_eq!((plus(&t, rack), plus(&t, keepers)), (3, 0)));
}

// Rules: 122.5, 122.6, 614.1a
// Given: player 0's two Weapon Racks with three counters each, Hardened Scales, Herd Baloth and
// Fathom Mage; player 1's Ornithopter.
// When: player 0 moves a counter from one Rack onto the Baloth, and from the other onto the
// Ornithopter.
// Expect: a moved counter is put onto the second creature (rulings of Daghatar the Adamant and
// Explorer's Cache): Hardened Scales makes it two on the Baloth, and "Whenever one or more +1/+1
// counters are put on this creature, you may create a 4/4 green Beast creature token"
// triggers; the Ornithopter isn't player 0's, so it gets one.
#[test]
fn r122_5_a_moved_counter_is_put_on_for_what_watches_and_replaces_that() {
    let mut t = Board::new(2).fill(8).named(0, Zone::Battlefield, "rack", "Weapon Rack").named(0, Zone::Battlefield, "other rack", "Weapon Rack")
        .battlefield(0, &["Hardened Scales", "Herd Baloth"]).battlefield(1, &["Ornithopter"]).build();
    let (rack, other) = (t.id("rack"), t.id("other rack"));
    let (baloth, thopter) = (t.find_in_battlefield("Herd Baloth"), t.find_in_battlefield("Ornithopter"));
    for r in [rack, other] { t.g.put_counters(Target::Object(r), "+1/+1", 3); }
    t.g.act(0, Move::Activate { source: rack, ability: activated(&t, rack)[0], targets: vec![vec![Target::Object(baloth)]], paying: vec![] }).unwrap();
    resolve(&mut t, true);
    cite!(["122.5", "614.1a"]; assert_eq!((plus(&t, rack), plus(&t, baloth)), (2, 2)));
    t.expect().rules(["122.5", "122.6"]).zone(0, Zone::Battlefield, &["Weapon Rack", "Weapon Rack", "Hardened Scales", "Herd Baloth", "Beast Token"]).check();
    t.g.act(0, Move::Activate { source: other, ability: activated(&t, other)[0], targets: vec![vec![Target::Object(thopter)]], paying: vec![] }).unwrap();
    resolve(&mut t, true);
    cite!(["122.5", "614.1a"]; assert_eq!((plus(&t, other), plus(&t, thopter)), (2, 1)));
}

// Rules: 122.5, 603.2
// Given: player 0's Weapon Rack with three counters and Fathom Mage ("Whenever a +1/+1 counter is
// put on this creature, you may draw a card.").
// When: player 0 moves a counter from the Rack onto the Mage, and says yes.
// Expect: it is put on: player 0 draws a card.
#[test]
fn r122_5_a_counter_moved_onto_fathom_mage_draws_a_card() {
    let mut t = Board::new(2).fill(8).battlefield(0, &["Weapon Rack", "Fathom Mage"]).build();
    let (rack, mage) = (t.find_in_battlefield("Weapon Rack"), t.find_in_battlefield("Fathom Mage"));
    t.g.put_counters(Target::Object(rack), "+1/+1", 3);
    let hand = t.g.players[0].hand.len();
    t.g.act(0, Move::Activate { source: rack, ability: activated(&t, rack)[0], targets: vec![vec![Target::Object(mage)]], paying: vec![] }).unwrap();
    resolve(&mut t, true);
    cite!(["122.5", "603.2"]; assert_eq!((plus(&t, mage), t.g.players[0].hand.len()), (1, hand + 1)));
}

// Rules: 603.10a, 122.5
// Given: player 0's Explorer's Cache with two counters and Grizzly Bears.
// When: player 0 moves a counter from the Cache onto the Bears, then casts Lightning Bolt at the
// Bears.
// Expect: "Whenever a creature you control with a +1/+1 counter on it dies, put a +1/+1 counter
// on this artifact" looks back at the Bears as it last was, with its counter: the Cache gets it
// back.
#[test]
fn r603_10a_explorers_cache_gets_back_the_counter_of_a_creature_that_dies_with_it() {
    let mut t = Board::new(2).fill(8).battlefield(0, &["Explorer's Cache", "Grizzly Bears"]).hand(0, &["Lightning Bolt"]).build();
    let (cache, bears) = (t.find_in_battlefield("Explorer's Cache"), t.find_in_battlefield("Grizzly Bears"));
    t.g.put_counters(Target::Object(cache), "+1/+1", 2);
    t.g.act(0, Move::Activate { source: cache, ability: activated(&t, cache)[0], targets: vec![vec![Target::Object(bears)]], paying: vec![] }).unwrap();
    resolve(&mut t, true);
    cite!(["122.5"]; assert_eq!((plus(&t, cache), plus(&t, bears)), (1, 1)));
    t.mana(0, "{R}").cast(0, "Lightning Bolt", vec![Tgt::Object(bears)], mana_pool("R")).unwrap();
    resolve(&mut t, true);
    t.expect().rules(["603.10a"]).zone(0, Zone::Graveyard, &["Grizzly Bears", "Lightning Bolt"]).check();
    cite!(["603.10a"]; assert_eq!(plus(&t, cache), 2));
}

// Rules: 122.5
// Given: player 0's Cytoplast Root-Kin with four counters and Grizzly Bears with one.
// When: player 0 activates "{2}: Move a +1/+1 counter from target creature you control onto this
// creature." at the Root-Kin itself, then at the Bears.
// Expect: a counter can't move from a permanent onto itself, so the Root-Kin keeps four; from
// the Bears one moves onto it.
#[test]
fn r122_5_a_counter_doesnt_move_from_a_permanent_onto_itself() {
    let mut t = Board::new(2).fill(8).battlefield(0, &["Cytoplast Root-Kin", "Grizzly Bears"]).build();
    let (root_kin, bears) = (t.find_in_battlefield("Cytoplast Root-Kin"), t.find_in_battlefield("Grizzly Bears"));
    t.g.put_counters(Target::Object(root_kin), "+1/+1", 4);
    t.g.put_counters(Target::Object(bears), "+1/+1", 1);
    let ability = activated(&t, root_kin)[0];
    for from in [root_kin, bears] {
        t.mana(0, "{2}");
        t.g.act(0, paid("CC", Move::Activate { source: root_kin, ability, targets: vec![vec![Target::Object(from)]], paying: vec![] })).unwrap();
        resolve(&mut t, true);
    }
    cite!(["122.5"]; assert_eq!((plus(&t, root_kin), plus(&t, bears)), (5, 0)));
}

// Rules: 122.5, 603.3d, 603.8
// Given: player 0's Afiya Grove with one counter left; player 1's Grizzly Bears, or no creature.
// When: player 0's upkeep begins.
// Expect: its trigger must target a creature, even an opponent's (its ruling), and moves its
// last counter onto the Bears; then with none it is sacrificed. With no creature to target, no
// counter is removed and it stays.
#[test]
fn r122_5_afiya_grove_moves_its_last_counter_and_is_sacrificed() {
    // Afiya Grove: "This enchantment enters with three +1/+1 counters on it. / At the beginning of
    // your upkeep, move a +1/+1 counter from this enchantment onto target creature. / When this
    // enchantment has no +1/+1 counters on it, sacrifice it."
    for creature in [true, false] {
        let theirs: &[&str] = if creature { &["Grizzly Bears"] } else { &[] };
        let mut t = Board::new(2).fill(8).active(1).at(Part::End).battlefield(0, &["Afiya Grove"]).battlefield(1, theirs).build();
        let grove = t.find_in_battlefield("Afiya Grove");
        t.g.put_counters(Target::Object(grove), "+1/+1", 1);
        for _ in 0..30 {
            if matches!(t.g.decision, Some(Decision::Targets { player: 0, .. })) || (t.g.active == 0 && t.g.part == Part::Draw) { break; }
            let p = t.g.decision_maker(t.g.priority);
            t.g.act(p, Move::Pass).unwrap();
        }
        if creature {
            let bears = t.find_in_battlefield("Grizzly Bears");
            cite!(["603.3d"]; assert!(matches!(t.g.decision, Some(Decision::Targets { player: 0, .. }))));
            t.g.act(0, Move::ChooseTargets(vec![vec![Target::Object(bears)]])).unwrap();
            resolve(&mut t, true);
            cite!(["122.5"]; assert_eq!(plus(&t, bears), 1));
            t.expect().rules(["603.8"]).zone(0, Zone::Graveyard, &["Afiya Grove"]).check();
        } else {
            cite!(["603.3d", "122.5"]; assert_eq!((t.g.battlefield.contains(&grove), plus(&t, grove)), (true, 1)));
        }
    }
}

// Rules: 611.2b, 122.5
// Given: player 0's Cytoplast Manipulator; player 1's Grizzly Bears with a +1/+1 counter and
// Ornithopter with none.
// When: player 0 activates "{U}, {T}: Gain control of target creature with a +1/+1 counter on it
// for as long as Cytoplast Manipulator remains on the battlefield." at the Ornithopter, then at
// the Bears; then casts Fate Transfer to move the Bears' counter onto the Ornithopter.
// Expect: the Ornithopter can't be its target; player 0 gains the Bears and keeps it once it has
// no counter (its ruling): the duration is the Manipulator's, not the counter's.
#[test]
fn r611_2b_cytoplast_manipulators_control_outlasts_the_counter() {
    let mut t = Board::new(2).fill(8).battlefield(0, &["Cytoplast Manipulator"]).battlefield(1, &["Grizzly Bears", "Ornithopter"]).hand(0, &["Fate Transfer"]).build();
    let (manipulator, bears, thopter) = (t.find_in_battlefield("Cytoplast Manipulator"), t.find_in_battlefield("Grizzly Bears"), t.find_in_battlefield("Ornithopter"));
    t.g.put_counters(Target::Object(manipulator), "+1/+1", 2);
    t.g.put_counters(Target::Object(bears), "+1/+1", 1);
    let ability = activated(&t, manipulator)[0];
    let gain = |t: &mut T, o: ObjId| t.g.act(0, paid("U", Move::Activate { source: manipulator, ability, targets: vec![vec![Target::Object(o)]], paying: vec![] }));
    t.mana(0, "{U}");
    cite!(["115.1"]; refused("an illegal target, Ornithopter"): gain(&mut t.clone(), thopter));
    gain(&mut t, bears).unwrap();
    resolve(&mut t, true);
    cite!(["611.2b"]; assert_eq!(t.g.controller(bears), 0));
    t.mana(0, "{1}{U}");
    t.g.act(0, paid("CU", Move::Cast { card: t.find_in(0, Zone::Hand, "Fate Transfer"), targets: vec![vec![Target::Object(bears)], vec![Target::Object(thopter)]] })).unwrap();
    resolve(&mut t, true);
    cite!(["122.5"]; assert_eq!((plus(&t, bears), plus(&t, thopter)), (0, 1)));
    cite!(["611.2b"]; assert_eq!(t.g.controller(bears), 0));
}

// Rules: 122.5, 306.5b
// Given: player 0's Gatewatch Beacon with three loyalty counters.
// When: player 0 casts Jace Beleren (loyalty 3) and says yes.
// Expect: "Whenever a planeswalker you control enters, if this artifact has loyalty counters on
// it, you may move a loyalty counter from this artifact onto that planeswalker": Jace has four,
// the Beacon two.
#[test]
fn r122_5_gatewatch_beacon_moves_a_loyalty_counter_onto_a_planeswalker() {
    let mut t = Board::new(2).fill(8).battlefield(0, &["Gatewatch Beacon"]).hand(0, &["Jace Beleren"]).build();
    let beacon = t.find_in_battlefield("Gatewatch Beacon");
    t.g.put_counters(Target::Object(beacon), "loyalty", 3);
    cast(&mut t, 0, "Jace Beleren", "{1}{U}{U}");
    resolve(&mut t, true);
    let jace = t.find_in_battlefield("Jace Beleren");
    let loyalty = |t: &T, o: ObjId| t.g.counters(o).get("loyalty").copied().unwrap_or(0);
    cite!(["122.5", "306.5b"]; assert_eq!((loyalty(&t, jace), loyalty(&t, beacon)), (4, 2)));
}
