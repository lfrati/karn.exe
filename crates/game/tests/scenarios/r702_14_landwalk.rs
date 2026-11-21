//! Landwalk (CR 702.14), through real cards' text.
//!
//! 702.14a "Landwalk is a generic term that appears within an object's rules text as
//! "[type]walk," where [type] is usually a land type, but it can also be the card type land
//! plus any combination of land types, card types, and/or supertypes."
//! 702.14c "A creature with landwalk can't be blocked as long as the defending player controls
//! at least one land with the specified land type (as in "islandwalk"), with the specified
//! type or supertype (as in "artifact landwalk"), without the specified type or supertype (as
//! in "nonbasic landwalk"), or with both the specified type or supertype and the specified
//! subtype (as in "snow swampwalk")."
//! 509.1b: a declaration of blockers that disobeys a restriction is illegal; "A restriction may
//! be created by an evasion ability" (702.14b: landwalk is one).
//! 802.2a: the defending player an attacking creature's ability refers to is "the player that
//! creature is attacking, the controller of the planeswalker that creature is attacking".

use crate::support::cite;
use game::card::CardDef;
use game::legal::Asked;
use game::play::check;
use game::state::Decision;
use game::{Game, Move, ObjId, Target, Zone};
use ir::Part;

const PALE_BEARS: usize = 0;
const RIME_DRYAD: usize = 1;
const SOPHISTICATE: usize = 2;
const AYUMI: usize = 3;
const AQUITECTS: usize = 4;
const MERFOLK: usize = 5;
const BEARS: usize = 6;
const ONES: usize = 7;
const ISLAND: usize = 8;
const FOREST: usize = 9;
const SNOW_FOREST: usize = 10;
const SNOW_ISLAND: usize = 11;
const TROPICAL: usize = 12;
const MIKOKORO: usize = 13;
const JACE: usize = 14;
const LEGIONS: usize = 15;
const SNOW_SWAMP: usize = 16;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Pale Bears", "{2}{G}", "Creature — Bear",
            "Islandwalk (This creature can't be blocked as long as defending player controls an Island.)", Some((2, 2))),
        card("Rime Dryad", "{G}", "Creature — Dryad",
            "Snow forestwalk (This creature can't be blocked as long as defending player controls a snow Forest.)", Some((1, 2))),
        card("Dryad Sophisticate", "{1}{G}", "Creature — Dryad",
            "Nonbasic landwalk (This creature can't be blocked as long as defending player controls a nonbasic land.)", Some((2, 1))),
        card("Ayumi, the Last Visitor", "{3}{G}{G}", "Legendary Creature — Spirit",
            "Legendary landwalk (This creature can't be blocked as long as defending player controls a legendary land.)", Some((7, 3))),
        card("Streambed Aquitects", "{1}{U}{U}", "Creature — Merfolk Scout",
            "{T}: Target Merfolk creature gets +1/+1 and gains islandwalk until end of turn. (It can't be blocked as long as defending player controls an Island.)\n{T}: Target land becomes an Island until end of turn.",
            Some((2, 3))),
        card("Merfolk of the Pearl Trident", "{U}", "Creature — Merfolk", "", Some((1, 1))),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Wandering Ones", "{U}", "Creature — Spirit", "", Some((1, 1))),
        card("Island", "", "Basic Land — Island", "({T}: Add {U}.)", None),
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        card("Snow-Covered Forest", "", "Basic Snow Land — Forest", "({T}: Add {G}.)", None),
        card("Snow-Covered Island", "", "Basic Snow Land — Island", "({T}: Add {U}.)", None),
        card("Tropical Island", "", "Land — Forest Island", "({T}: Add {G} or {U}.)", None),
        card("Mikokoro, Center of the Sea", "", "Legendary Land", "{T}: Add {C}.\n{2}, {T}: Each player draws a card.", None),
        card("Jace Beleren", "{1}{U}{U}", "Legendary Planeswalker — Jace",
            "+2: Each player draws a card.\n−1: Target player draws a card.\n−10: Target player mills twenty cards.", None),
        card("Legions of Lim-Dûl", "{1}{B}{B}", "Creature — Zombie",
            "Snow swampwalk (This creature can't be blocked as long as defending player controls a snow Swamp.)", Some((2, 3))),
        card("Snow-Covered Swamp", "", "Basic Snow Land — Swamp", "({T}: Add {B}.)", None),
    ]
}

fn game(players: usize) -> Game {
    Game::new(defs(), players, 20)
}

fn all_pass(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}

/// Player 0 attacks as said from its first main phase; then all pass, to the declaration of
/// blockers.
fn attack(g: &mut Game, attacks: &[(ObjId, Target)]) {
    all_pass(g);
    all_pass(g);
    assert_eq!(g.part, Part::DeclareAttackers);
    g.act(0, Move::Attack(attacks.to_vec())).unwrap();
    all_pass(g);
    assert_eq!(g.part, Part::DeclareBlockers);
}

fn at(player: usize) -> Target {
    Target::Player(player)
}

/// The attacking creatures each creature of this defending player is offered to block (509.1a),
/// as legal-move discovery has them.
fn offered(g: &mut Game, player: usize) -> Vec<(ObjId, Vec<ObjId>)> {
    assert_eq!(g.decision, Some(Decision::Blockers { player }));
    match g.asked() {
        Some((p, Asked::Blockers { able, .. })) if p == player => able,
        other => panic!("expected player {player}'s blockers, got {other:?}"),
    }
}

/// Is `blocker` offered to block `attacker`?
fn may_block(g: &mut Game, player: usize, blocker: ObjId, attacker: ObjId) -> bool {
    offered(g, player).iter().any(|(b, attackers)| *b == blocker && attackers.contains(&attacker))
}

/// The defending player (1) controls `land` and a Grizzly Bears; player 0 attacks with
/// `walker` and with a Wandering Ones, so that player 1 has a block to declare either way. Is
/// the walker offered to the Bears, and is a submitted block of it accepted? The two answers
/// must agree: one restriction decides both (509.1b).
fn blockable_with(walker: usize, land: usize) -> bool {
    blockable_with_lands(walker, &[land])
}
fn blockable_with_lands(walker: usize, lands: &[usize]) -> bool {
    let mut g = game(2);
    let attacker = g.put(0, walker, Zone::Battlefield);
    let ones = g.put(0, ONES, Zone::Battlefield);
    for land in lands { g.put(1, *land, Zone::Battlefield); }
    let blocker = g.put(1, BEARS, Zone::Battlefield);
    attack(&mut g, &[(attacker, at(1)), (ones, at(1))]);
    let offered = may_block(&mut g, 1, blocker, attacker);
    assert!(may_block(&mut g, 1, blocker, ones), "the creature without landwalk can always be blocked");
    let submitted = g.clone().act(1, Move::Block(vec![(blocker, attacker)]));
    if let Err(e) = &submitted {
        let name = &g.obj(attacker).chars.name;
        cite!(["702.14c", "509.1b"]; assert_eq!(e, &format!("Grizzly Bears can't block {name}: {name} has landwalk, and the defending player controls a land of that kind (702.14c, 509.1b)")));
    }
    assert_eq!(offered, submitted.is_ok(), "discovery and validation disagree");
    offered
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        // loyalty abilities are not activated yet: Jace is put onto the battlefield here
        if d.name() == "Jace Beleren" {
            continue;
        }
        check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r702_14c_islandwalk_cant_be_blocked_while_the_defending_player_controls_an_island() {
    cite!(["702.14c"]; assert!(blockable_with(PALE_BEARS, FOREST), "no Island: it can be blocked"));
    cite!(["702.14c"]; assert!(!blockable_with(PALE_BEARS, ISLAND)));
    // the land type, whatever else the land is: a snow Island, a nonbasic Forest Island
    assert!(!blockable_with(PALE_BEARS, SNOW_ISLAND));
    cite!(["702.14c"]; assert!(!blockable_with(PALE_BEARS, TROPICAL)));
}

#[test]
fn r702_14c_with_no_other_block_to_declare_the_defending_player_is_not_asked() {
    let mut g = game(2);
    let bears = g.put(0, PALE_BEARS, Zone::Battlefield);
    g.put(1, ISLAND, Zone::Battlefield);
    g.put(1, BEARS, Zone::Battlefield);
    attack(&mut g, &[(bears, at(1))]);
    cite!(["509.1"]; assert_eq!(g.decision, None, "no creature could block it: nobody is asked (509.1)"));
    all_pass(&mut g);
    cite!(["510.1a"]; assert_eq!(g.players[1].life, 18, "unblocked, it deals its damage to the defending player"));
}

#[test]
fn r702_14c_snow_forestwalk_needs_both_the_supertype_and_the_subtype() {
    assert!(blockable_with(RIME_DRYAD, FOREST), "a Forest that isn't snow");
    assert!(blockable_with(RIME_DRYAD, SNOW_ISLAND), "a snow land that isn't a Forest");
    cite!(["702.14c"]; assert!(!blockable_with(RIME_DRYAD, SNOW_FOREST)));
    cite!(["702.14c"]; assert!(blockable_with_lands(RIME_DRYAD, &[SNOW_ISLAND, FOREST]), "both qualities must belong to the same land"));
    // "snow swampwalk", the rule's own example
    assert!(blockable_with(LEGIONS, SNOW_FOREST));
    cite!(["702.14c"]; assert!(!blockable_with(LEGIONS, SNOW_SWAMP)));
}

#[test]
fn r702_14c_nonbasic_landwalk_is_a_land_without_the_supertype() {
    cite!(["702.14c"]; assert!(blockable_with(SOPHISTICATE, ISLAND), "a basic land"));
    assert!(blockable_with(SOPHISTICATE, SNOW_FOREST), "a basic snow land");
    cite!(["702.14c"]; assert!(!blockable_with(SOPHISTICATE, TROPICAL)));
    assert!(!blockable_with(SOPHISTICATE, MIKOKORO));
}

#[test]
fn r702_14c_legendary_landwalk_is_a_land_with_the_supertype() {
    cite!(["702.14c"]; assert!(blockable_with(AYUMI, TROPICAL), "a nonbasic land that isn't legendary"));
    cite!(["702.14c"]; assert!(!blockable_with(AYUMI, MIKOKORO)));
}

/// "Target land becomes an Island until end of turn": the land's types as they are now decide
/// (613.1d). Its land types are replaced (305.7): a Forest that becomes an Island is no longer
/// a Forest.
#[test]
fn r702_14c_a_land_whose_land_type_changes_is_asked_as_it_is_now() {
    // a Forest becomes an Island: islandwalk now applies
    let mut g = game(2);
    let bears = g.put(0, PALE_BEARS, Zone::Battlefield);
    let ones = g.put(0, ONES, Zone::Battlefield);
    let aquitects = g.put(0, AQUITECTS, Zone::Battlefield);
    let forest = g.put(1, FOREST, Zone::Battlefield);
    let blocker = g.put(1, BEARS, Zone::Battlefield);
    g.act(0, Move::Activate { source: aquitects, ability: 1, targets: vec![vec![Target::Object(forest)]], paying: vec![] }).unwrap();
    all_pass(&mut g);
    assert!(g.seen(forest, None).subtypes.iter().any(|s| s.name() == "Island"));
    attack(&mut g, &[(bears, at(1)), (ones, at(1))]);
    cite!(["702.14c"]; assert!(!may_block(&mut g, 1, blocker, bears)));
    cite!(["702.14c", "509.1b"]; refused("Grizzly Bears can't block Pale Bears: Pale Bears has landwalk, and the defending player controls a land of that kind (702.14c, 509.1b)"): g.act(1, Move::Block(vec![(blocker, bears)])));
    g.act(1, Move::Block(vec![(blocker, ones)])).unwrap();

    // a snow Forest becomes an Island: snow forestwalk no longer applies
    let mut g = game(2);
    let dryad = g.put(0, RIME_DRYAD, Zone::Battlefield);
    let ones = g.put(0, ONES, Zone::Battlefield);
    let aquitects = g.put(0, AQUITECTS, Zone::Battlefield);
    let snow_forest = g.put(1, SNOW_FOREST, Zone::Battlefield);
    let blocker = g.put(1, BEARS, Zone::Battlefield);
    g.act(0, Move::Activate { source: aquitects, ability: 1, targets: vec![vec![Target::Object(snow_forest)]], paying: vec![] }).unwrap();
    all_pass(&mut g);
    attack(&mut g, &[(dryad, at(1)), (ones, at(1))]);
    cite!(["305.7", "702.14c"]; assert!(may_block(&mut g, 1, blocker, dryad)));
    g.act(1, Move::Block(vec![(blocker, dryad)])).unwrap();
}

/// "Target Merfolk creature … gains islandwalk until end of turn": the granted ability keeps
/// its type (613.1f) and restricts blocks as the printed one does.
#[test]
fn r702_14c_granted_islandwalk_keeps_its_land_type() {
    let mut g = game(2);
    let merfolk = g.put(0, MERFOLK, Zone::Battlefield);
    let ones = g.put(0, ONES, Zone::Battlefield);
    let aquitects = g.put(0, AQUITECTS, Zone::Battlefield);
    g.put(1, ISLAND, Zone::Battlefield);
    let blocker = g.put(1, BEARS, Zone::Battlefield);
    g.act(0, Move::Activate { source: aquitects, ability: 0, targets: vec![vec![Target::Object(merfolk)]], paying: vec![] }).unwrap();
    all_pass(&mut g);
    assert_eq!(g.power(merfolk), Some(2));
    attack(&mut g, &[(merfolk, at(1)), (ones, at(1))]);
    cite!(["702.14c"]; assert!(!may_block(&mut g, 1, blocker, merfolk)));
    cite!(["702.14c", "509.1b"]; refused("Grizzly Bears can't block Merfolk of the Pearl Trident: Merfolk of the Pearl Trident has landwalk, and the defending player controls a land of that kind (702.14c, 509.1b)"): g.act(1, Move::Block(vec![(blocker, merfolk)])));

    // without an Island, the gained islandwalk restricts nothing
    let mut g = game(2);
    let merfolk = g.put(0, MERFOLK, Zone::Battlefield);
    let ones = g.put(0, ONES, Zone::Battlefield);
    let aquitects = g.put(0, AQUITECTS, Zone::Battlefield);
    g.put(1, FOREST, Zone::Battlefield);
    let blocker = g.put(1, BEARS, Zone::Battlefield);
    g.act(0, Move::Activate { source: aquitects, ability: 0, targets: vec![vec![Target::Object(merfolk)]], paying: vec![] }).unwrap();
    all_pass(&mut g);
    attack(&mut g, &[(merfolk, at(1)), (ones, at(1))]);
    cite!(["702.14c"]; assert!(may_block(&mut g, 1, blocker, merfolk)));
    g.act(1, Move::Block(vec![(blocker, merfolk)])).unwrap();
}

/// 802.2a, 802.4b: in a multiplayer game each attacking creature's defending player is the one
/// it attacks; another player's Island doesn't keep a creature attacking someone else from
/// being blocked.
#[test]
fn r802_2a_only_the_lands_of_the_player_it_attacks_matter() {
    let mut g = game(3);
    let at_one = g.put(0, PALE_BEARS, Zone::Battlefield);
    let at_two = g.put(0, PALE_BEARS, Zone::Battlefield);
    let ones = g.put(0, ONES, Zone::Battlefield);
    g.put(1, FOREST, Zone::Battlefield);
    let blocker_one = g.put(1, BEARS, Zone::Battlefield);
    g.put(2, ISLAND, Zone::Battlefield);
    let blocker_two = g.put(2, BEARS, Zone::Battlefield);
    attack(&mut g, &[(at_one, at(1)), (at_two, at(2)), (ones, at(2))]);
    // player 1 declares first (802.4, 101.4): player 2's Island is not theirs
    cite!(["101.4", "802.4"]; assert_eq!(offered(&mut g, 1), vec![(blocker_one, vec![at_one])]));
    g.act(1, Move::Block(vec![(blocker_one, at_one)])).unwrap();
    // player 2 controls an Island: the Pale Bears attacking them can't be blocked
    cite!(["802.2a", "702.14c"]; assert_eq!(offered(&mut g, 2), vec![(blocker_two, vec![ones])]));
    cite!(["702.14c", "509.1b"]; refused("Grizzly Bears can't block Pale Bears: Pale Bears has landwalk, and the defending player controls a land of that kind (702.14c, 509.1b)"): g.act(2, Move::Block(vec![(blocker_two, at_two)])));
    g.act(2, Move::Block(vec![(blocker_two, ones)])).unwrap();
    assert_eq!(g.combat.blockers, vec![(blocker_one, at_one), (blocker_two, ones)]);
}

/// 506.2, 802.2a: a creature attacking a planeswalker has that planeswalker's controller as its
/// defending player.
#[test]
fn r802_2a_attacking_a_planeswalker_its_controller_is_the_defending_player() {
    let jace = |g: &mut Game, player| {
        let jace = g.put(player, JACE, Zone::Battlefield);
        g.put_counters(jace, "loyalty", 3);
        jace
    };
    // two players: Jace's controller controls an Island
    let mut g = game(2);
    let bears = g.put(0, PALE_BEARS, Zone::Battlefield);
    let ones = g.put(0, ONES, Zone::Battlefield);
    let walker = jace(&mut g, 1);
    g.put(1, ISLAND, Zone::Battlefield);
    let blocker = g.put(1, BEARS, Zone::Battlefield);
    attack(&mut g, &[(bears, Target::Object(walker)), (ones, at(1))]);
    cite!(["802.2a", "702.14c"]; assert!(!may_block(&mut g, 1, blocker, bears)));
    cite!(["702.14c", "509.1b"]; refused("Grizzly Bears can't block Pale Bears: Pale Bears has landwalk, and the defending player controls a land of that kind (702.14c, 509.1b)"): g.act(1, Move::Block(vec![(blocker, bears)])));

    // three players: the Island is another player's, not Jace's controller's
    let mut g = game(3);
    let bears = g.put(0, PALE_BEARS, Zone::Battlefield);
    let walker = jace(&mut g, 1);
    g.put(1, FOREST, Zone::Battlefield);
    let blocker = g.put(1, BEARS, Zone::Battlefield);
    g.put(2, ISLAND, Zone::Battlefield);
    g.put(2, BEARS, Zone::Battlefield);
    attack(&mut g, &[(bears, Target::Object(walker))]);
    cite!(["802.2a"]; assert!(may_block(&mut g, 1, blocker, bears)));
    g.act(1, Move::Block(vec![(blocker, bears)])).unwrap();
}
