//! Review of described casts (608.2g, 601.3e): "you may cast a spell with mana value 3 or less
//! from your hand", "you may cast a spell from among them". Each test checks a bound the
//! mechanism's own tests don't, where the uncast cards go, or a card read from a group named
//! another way. Every card is read from its Oracle text.
use crate::support;
use game::state::Decision;
use support::*;

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Aetherworks Marvel", "Herald of Amity", "Yahenni's Expertise", "Rishkar's Expertise", "Sram's Expertise", "Electrodominance",
        "Velomachus Lorehold", "Cosmic Cube", "Buster Sword", "Perception Bobblehead", "In Search of Greatness", "Silent-Blade Oni", "Holy Strength",
        "Pacifism", "Grizzly Bears", "Serra Angel", "Lightning Bolt", "Divination", "Centaur Courser", "Hill Giant", "Savannah Lions", "Craw Wurm",
        "Air Elemental", "Lava Spike", "Overwhelming Stampede", "Mass Calcify", "Giant Growth"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

fn names(t: &T, cards: &[ObjId]) -> Vec<String> {
    let mut names: Vec<String> = cards.iter().map(|o| t.g.obj(*o).chars.name.clone()).collect();
    names.sort();
    names
}

/// The names of a player's library, top first.
fn top_down(t: &T, p: usize) -> Vec<String> { t.g.players[p].library.iter().rev().map(|o| t.g.obj(*o).chars.name.clone()).collect() }

/// The cards an effect offers its controller to cast.
fn offered(t: &mut T) -> Vec<ObjId> {
    match t.g.asked() { Some((0, Asked::Cast { cards })) => cards, other => panic!("not an offer to cast: {other:?}") }
}

fn named(t: &T, cards: &[ObjId], name: &str) -> ObjId { *cards.iter().find(|o| t.g.obj(**o).chars.name == name).expect(name) }

/// Pass until the stack is empty, answering how cards are ordered into a graveyard.
fn drain(t: &mut T) {
    for _ in 0..30 {
        match t.g.asked() {
            Some((p, Asked::OrderCards { cards, .. })) => { t.answer(p, Answer::Objects(cards)).unwrap(); }
            Some((p, Asked::Priority)) if !t.g.stack.is_empty() => { t.g.act(p, Move::Pass).unwrap(); }
            _ => return,
        }
    }
}

fn to_attackers(t: &mut T) {
    for _ in 0..40 {
        if matches!(t.g.decision, Some(Decision::Attackers { .. })) { return; }
        let p = t.g.decision_maker(t.g.priority);
        t.g.act(p, Move::Pass).unwrap();
    }
    panic!("no declare attackers step");
}

/// Pass until an offer to cast is asked, declaring no blocks.
fn to_offer(t: &mut T) {
    for _ in 0..40 {
        if matches!(t.g.asked(), Some((_, Asked::Cast { .. }))) { return; }
        match t.g.decision.clone() {
            Some(Decision::Blockers { player }) => t.g.act(player, Move::Block(vec![])).unwrap(),
            _ => { let p = t.g.decision_maker(t.g.priority); t.g.act(p, Move::Pass).unwrap() }
        }
    }
    panic!("no offer to cast: {:?}", t.g.decision);
}

const MARVEL_LIBRARY: [&str; 8] = ["Mountain", "Grizzly Bears", "Serra Angel", "Lightning Bolt", "Island", "Divination", "Forest", "Plains"];

// Rules: 608.2g, 305.9, 401.4
// Given: Aetherworks Marvel and six energy; player 0's library is Mountain, Grizzly Bears,
// Serra Angel, Lightning Bolt, Island, Divination, then Forest and Plains.
// When: its second ability resolves; player 0 casts Serra Angel, or declines.
// Expect: the four nonland cards of the six are offered; cast, the Angel is on the stack and
// the other five go under Forest and Plains; declined, all six do.
#[test]
fn r608_2g_aetherworks_marvel_puts_the_cards_not_cast_on_the_bottom() {
    // Aetherworks Marvel: "{T}, Pay six {E}: Look at the top six cards of your library. You may
    // cast a spell from among them without paying its mana cost. Put the rest on the bottom of
    // your library in a random order."
    for decline in [false, true] {
        let mut t = Board::new(2).named(0, Zone::Battlefield, "marvel", "Aetherworks Marvel").library(0, &MARVEL_LIBRARY).build();
        let marvel = t.id("marvel");
        t.g.put_counters(Target::Player(0), "energy", 6);
        t.g.act(0, Move::Activate { source: marvel, ability: 1, targets: vec![], paying: vec![] }).unwrap();
        t.pass_all();
        let cards = offered(&mut t);
        cite!(["608.2g", "305.9"]; case: format!("declined {decline}"); assert_eq!(names(&t, &cards), ["Divination", "Grizzly Bears", "Lightning Bolt", "Serra Angel"]));
        if decline {
            t.g.act(0, Move::Choose(Answer::No)).unwrap();
        } else {
            let angel = named(&t, &cards, "Serra Angel");
            t.g.act(0, Move::Cast { card: angel, targets: vec![] }).unwrap();
            cite!(["608.2g"]; assert_eq!(names(&t, &t.g.stack.clone()), ["Serra Angel"]));
        }
        let library = top_down(&t, 0);
        cite!(["401.4"]; case: format!("declined {decline}"); assert_eq!(library[..2], ["Forest", "Plains"]));
        cite!(["401.4"]; case: format!("declined {decline}"); assert_eq!(library.len(), if decline { 8 } else { 7 }));
    }
}

// Rules: 608.2g, 406.1, 401.4, 303.4a
// Given: Herald of Amity; player 0's library is Holy Strength, Grizzly Bears, Pacifism, Mountain,
// Island, Lightning Bolt, Divination, Swamp, then Forest and Plains.
// When: its enters trigger resolves and player 0 casts Holy Strength on it.
// Expect: the top eight are exiled and only the two Auras offered; Holy Strength is cast from
// exile; the seven other exiled cards go under Forest and Plains, leaving none in exile; once it
// resolves the Herald is 3/4.
#[test]
fn r608_2g_herald_of_amity_casts_an_aura_from_exile_and_puts_the_rest_on_the_bottom() {
    // Herald of Amity: "When this creature enters, exile the top eight cards of your library.
    // You may cast an Aura spell from among them without paying its mana cost. Then put the rest
    // on the bottom of your library in a random order."
    let mut t = Board::new(2).hand(0, &["Herald of Amity"])
        .library(0, &["Holy Strength", "Grizzly Bears", "Pacifism", "Mountain", "Island", "Lightning Bolt", "Divination", "Swamp", "Forest", "Plains"]).build();
    t.mana(0, "{3}{W}").cast(0, "Herald of Amity", vec![], mana_pool("CCCW")).unwrap();
    t.pass_all();
    t.pass_all();
    cite!(["406.1"]; assert_eq!(t.g.exile.len(), 8));
    let cards = offered(&mut t);
    cite!(["608.2g"]; assert_eq!(names(&t, &cards), ["Holy Strength", "Pacifism"]));
    let herald = t.find_in_battlefield("Herald of Amity");
    let strength = named(&t, &cards, "Holy Strength");
    t.g.act(0, Move::Cast { card: strength, targets: vec![vec![Target::Object(herald)]] }).unwrap();
    cite!(["608.2g"]; assert_eq!(names(&t, &t.g.stack.clone()), ["Holy Strength"]));
    let library = top_down(&t, 0);
    cite!(["401.4"]; assert!(t.g.exile.is_empty() && library.len() == 9 && library[..2] == ["Forest", "Plains"]));
    drain(&mut t);
    cite!(["303.4a", "613.4c"]; assert_eq!((t.g.power(herald), t.g.toughness(herald)), (Some(3), Some(4))));
}

// Rules: 611.2c, 608.2g, 704.5f
// Given: player 0's Savannah Lions, player 1's Hill Giant; player 0 holds Yahenni's Expertise,
// Grizzly Bears and Hill Giant.
// When: Yahenni's Expertise resolves and player 0 casts Grizzly Bears.
// Expect: only the Bears are offered (the Giant has mana value 4); the creatures it shrank die;
// the Bears, cast after the effect began, enter 2/2.
#[test]
fn r611_2c_a_creature_cast_with_yahenni_s_expertise_is_not_shrunk_by_it() {
    // Yahenni's Expertise: "All creatures get -3/-3 until end of turn. / You may cast a spell
    // with mana value 3 or less from your hand without paying its mana cost."
    let mut t = Board::new(2).fill(4).hand(0, &["Yahenni's Expertise", "Grizzly Bears", "Hill Giant"])
        .battlefield(0, &["Savannah Lions"]).battlefield(1, &["Hill Giant"]).build();
    t.mana(0, "{2}{B}{B}").cast(0, "Yahenni's Expertise", vec![], mana_pool("CCBB")).unwrap();
    t.pass_all();
    let cards = offered(&mut t);
    cite!(["608.2g", "202.3"]; assert_eq!(names(&t, &cards), ["Grizzly Bears"]));
    t.g.act(0, Move::Cast { card: cards[0], targets: vec![] }).unwrap();
    drain(&mut t);
    cite!(["704.5f"]; assert_eq!(names(&t, &t.g.battlefield.clone()), ["Grizzly Bears"]));
    let bears = t.find_in_battlefield("Grizzly Bears");
    cite!(["611.2c"]; assert_eq!((t.g.power(bears), t.g.toughness(bears)), (Some(2), Some(2))));
}

// Rules: 608.2g, 608.2h
// Given: player 0's Hill Giant (power 3); player 0 holds Rishkar's Expertise and Craw Wurm; the
// top three cards of their library are Air Elemental, Grizzly Bears and Craw Wurm.
// When: Rishkar's Expertise resolves.
// Expect: it draws three, and offers the drawn Air Elemental (mana value 5) and Grizzly Bears;
// neither Craw Wurm (6) is offered.
#[test]
fn r608_2g_rishkar_s_expertise_offers_the_cards_it_drew() {
    // Rishkar's Expertise: "Draw cards equal to the greatest power among creatures you control.
    // / You may cast a spell with mana value 5 or less from your hand without paying its mana
    // cost." Its ruling: a card drawn by the first effect may be cast by the second.
    let mut t = Board::new(2).hand(0, &["Rishkar's Expertise", "Craw Wurm"]).battlefield(0, &["Hill Giant"])
        .library(0, &["Air Elemental", "Grizzly Bears", "Craw Wurm", "Forest"]).build();
    t.mana(0, "{4}{G}{G}").cast(0, "Rishkar's Expertise", vec![], mana_pool("CCCCGG")).unwrap();
    t.pass_all();
    cite!(["608.2h"]; assert_eq!(t.g.players[0].hand.len(), 4));
    let cards = offered(&mut t);
    cite!(["608.2g", "202.3"]; assert_eq!(names(&t, &cards), ["Air Elemental", "Grizzly Bears"]));
}

// Rules: 608.2g, 202.3, 305.9
// Given: player 0 holds Sram's Expertise, Centaur Courser (mana value 3), Hill Giant (4),
// Mountain and Lava Spike (1).
// When: Sram's Expertise resolves.
// Expect: Centaur Courser and Lava Spike are offered: 3 is "3 or less"; Hill Giant and the land
// are not.
#[test]
fn r608_2g_sram_s_expertise_offers_a_spell_of_mana_value_3_exactly() {
    let mut t = Board::new(2).fill(4).hand(0, &["Sram's Expertise", "Centaur Courser", "Hill Giant", "Mountain", "Lava Spike"]).build();
    t.mana(0, "{2}{W}{W}").cast(0, "Sram's Expertise", vec![], mana_pool("CCWW")).unwrap();
    t.pass_all();
    let cards = offered(&mut t);
    cite!(["608.2g", "202.3", "305.9"]; assert_eq!(names(&t, &cards), ["Centaur Courser", "Lava Spike"]));
}

// Rules: 608.2g, 307.1, 302.1
// Given: player 1's turn; player 0 holds Electrodominance, Grizzly Bears, Lava Spike and
// Centaur Courser.
// When: player 0 casts Electrodominance with X 2 at player 1, and casts Grizzly Bears as it
// resolves.
// Expect: Grizzly Bears and Lava Spike are offered, not the Courser (mana value 3); the Bears
// are cast in player 1's turn, as a cast during resolution ignores timing (its ruling), and
// enter; player 1 is at 18.
#[test]
fn r608_2g_electrodominance_casts_a_creature_in_another_player_s_turn() {
    let mut t = Board::new(2).fill(6).active(1).at(Part::PrecombatMain).hand(0, &["Electrodominance", "Grizzly Bears", "Lava Spike", "Centaur Courser"]).build();
    t.g.act(1, Move::Pass).unwrap();
    let card = t.find_in(0, Zone::Hand, "Electrodominance");
    t.mana(0, "{2}{R}{R}");
    t.g.act(0, paid("CCRR", Move::WithX(2, Box::new(Move::Cast { card, targets: vec![vec![Target::Player(1)]] })))).unwrap();
    t.pass_all();
    let cards = offered(&mut t);
    cite!(["608.2g", "202.3"]; assert_eq!(names(&t, &cards), ["Grizzly Bears", "Lava Spike"]));
    let bears = named(&t, &cards, "Grizzly Bears");
    cite!(["608.2g", "307.1"]; ok: t.g.act(0, Move::Cast { card: bears, targets: vec![] }));
    drain(&mut t);
    cite!(["608.3"]; assert!(t.g.battlefield.iter().any(|o| t.g.obj(*o).chars.name == "Grizzly Bears" && t.g.controller(*o) == 0)));
    cite!(["120.3"]; assert_eq!(t.g.players[1].life, 18));
}

// Rules: 608.2g, 608.2h, 401.4
// Given: Velomachus Lorehold (5/5), grown by Giant Growth to 8/8 or not; player 0's library is
// Lightning Bolt, Overwhelming Stampede (mana value 5), Mass Calcify (7), Grizzly Bears,
// Divination, Island, Lava Spike, then Forest and Plains.
// When: it attacks and its trigger resolves; player 0 declines.
// Expect: the instants and sorceries of the seven are offered, of mana value at most its power
// as the trigger resolves: Mass Calcify only at 8. Declined, all seven go under Forest and Plains.
#[test]
fn r608_2h_velomachus_lorehold_reads_its_power_as_its_trigger_resolves() {
    // Velomachus Lorehold: "Whenever Velomachus Lorehold attacks, look at the top seven cards of
    // your library. You may cast an instant or sorcery spell with mana value less than or equal
    // to Velomachus Lorehold's power from among them without paying its mana cost. Put the rest
    // on the bottom of your library in a random order."
    for grown in [false, true] {
        let mut t = Board::new(2).named(0, Zone::Battlefield, "velo", "Velomachus Lorehold").named(0, Zone::Battlefield, "forest", "Forest")
            .hand(0, &["Giant Growth"])
            .library(0, &["Lightning Bolt", "Overwhelming Stampede", "Mass Calcify", "Grizzly Bears", "Divination", "Island", "Lava Spike", "Forest", "Plains"]).build();
        let velo = t.id("velo");
        if grown {
            t.mana(0, "{G}").cast(0, "Giant Growth", vec![Tgt::Object(velo)], mana_pool("G")).unwrap();
            t.resolve();
        }
        to_attackers(&mut t);
        t.g.act(0, Move::Attack(vec![(velo, Target::Player(1))])).unwrap();
        t.pass_all();
        let cards = offered(&mut t);
        let mut expected = vec!["Divination", "Lava Spike", "Lightning Bolt", "Overwhelming Stampede"];
        if grown { expected.insert(3, "Mass Calcify"); }
        cite!(["608.2h", "608.2g"]; case: format!("grown {grown}"); assert_eq!(names(&t, &cards), expected));
        t.g.act(0, Move::Choose(Answer::No)).unwrap();
        let library = top_down(&t, 0);
        cite!(["401.4"]; case: format!("grown {grown}"); assert!(library.len() == 9 && library[..2] == ["Forest", "Plains"]));
    }
}

// Rules: 608.2h, 508.1
// Given: Cosmic Cube, Grizzly Bears, Hill Giant and Craw Wurm; player 0's library is Lightning
// Bolt, Centaur Courser, Hill Giant, Mountain, Air Elemental, Divination, Forest.
// When: the Bears and the Giant attack, and the Cube's trigger resolves.
// Expect: the greatest power among the attacking creatures is 3, not the Wurm's 6: Lightning
// Bolt, Centaur Courser and Divination are offered.
#[test]
fn r608_2h_cosmic_cube_reads_only_the_attacking_creatures() {
    // Cosmic Cube: "Whenever you attack, look at the top six cards of your library. You may
    // cast a spell from among them with mana value less than or equal to the greatest power
    // among attacking creatures you control without paying its mana cost. Put the rest on the
    // bottom of your library in a random order."
    let mut t = Board::new(2).battlefield(0, &["Cosmic Cube", "Craw Wurm"]).named(0, Zone::Battlefield, "bears", "Grizzly Bears")
        .named(0, Zone::Battlefield, "giant", "Hill Giant")
        .library(0, &["Lightning Bolt", "Centaur Courser", "Hill Giant", "Mountain", "Air Elemental", "Divination", "Forest"]).build();
    let (bears, giant) = (t.id("bears"), t.id("giant"));
    to_attackers(&mut t);
    t.g.act(0, Move::Attack(vec![(bears, Target::Player(1)), (giant, Target::Player(1))])).unwrap();
    t.pass_all();
    let cards = offered(&mut t);
    cite!(["608.2h", "508.1"]; assert_eq!(names(&t, &cards), ["Centaur Courser", "Divination", "Lightning Bolt"]));
}

// Rules: 608.2h, 510.2
// Given: Buster Sword on player 0's Grizzly Bears (5/4); player 0 holds Craw Wurm (mana value
// 6), Air Elemental (5) and Lava Spike.
// When: the Bears deal 5 combat damage to player 1 and the Sword's trigger resolves.
// Expect: player 0 draws, then Air Elemental and Lava Spike are offered, not the Wurm.
#[test]
fn r608_2h_buster_sword_offers_a_spell_of_mana_value_at_most_the_damage() {
    // Buster Sword: "Equipped creature gets +3/+2. / Whenever equipped creature deals combat
    // damage to a player, draw a card, then you may cast a spell from your hand with mana value
    // less than or equal to that damage without paying its mana cost."
    let mut t = Board::new(2).fill(4).named(0, Zone::Battlefield, "bears", "Grizzly Bears").named(0, Zone::Battlefield, "sword", "Buster Sword")
        .hand(0, &["Craw Wurm", "Air Elemental", "Lava Spike"]).build();
    let (bears, sword) = (t.id("bears"), t.id("sword"));
    cite!(["301.5"]; assert!(t.g.attach(sword, Target::Object(bears))));
    to_attackers(&mut t);
    t.g.act(0, Move::Attack(vec![(bears, Target::Player(1))])).unwrap();
    to_offer(&mut t);
    cite!(["510.2"]; assert_eq!(t.g.players[1].life, 15));
    cite!(["121.1"]; assert_eq!(t.g.players[0].hand.len(), 4));
    let cards = offered(&mut t);
    cite!(["608.2h"]; assert_eq!(names(&t, &cards), ["Air Elemental", "Lava Spike"]));
}

// Rules: 608.2g, 608.2h, 401.4
// Given: two Perception Bobbleheads; player 0's library is Hill Giant, Grizzly Bears, Centaur
// Courser, Forest.
// When: one's "{3}, {T}" ability resolves; player 0 declines.
// Expect: the top two cards are looked at, as two Bobbleheads are controlled; only the Bears
// are offered (mana value 3 or less); declined, both go to the bottom, so the Courser is on top.
#[test]
fn r608_2g_perception_bobblehead_looks_at_one_card_for_each_bobblehead() {
    // Perception Bobblehead: "{3}, {T}: Look at the top X cards of your library, where X is the
    // number of Bobbleheads you control. You may cast a spell with mana value 3 or less from
    // among them without paying its mana cost. Put the rest on the bottom of your library in a
    // random order."
    let mut t = Board::new(2).named(0, Zone::Battlefield, "bob", "Perception Bobblehead").battlefield(0, &["Perception Bobblehead"])
        .library(0, &["Hill Giant", "Grizzly Bears", "Centaur Courser", "Forest"]).build();
    let bob = t.id("bob");
    t.mana(0, "{3}");
    t.g.act(0, paid("CCC", Move::Activate { source: bob, ability: 1, targets: vec![], paying: vec![] })).unwrap();
    t.pass_all();
    let cards = offered(&mut t);
    cite!(["608.2h", "608.2g"]; assert_eq!(names(&t, &cards), ["Grizzly Bears"]));
    t.g.act(0, Move::Choose(Answer::No)).unwrap();
    let library = top_down(&t, 0);
    cite!(["401.4"]; assert!(library[..2] == ["Centaur Courser", "Forest"] && names(&t, &t.g.players[0].library[..2]) == ["Grizzly Bears", "Hill Giant"]));
}

/// In Search of Greatness, Grizzly Bears and a Mountain on player 0's battlefield, Centaur
/// Courser, Hill Giant and Lightning Bolt in their hand: brought to their upkeep trigger's offer.
fn greatness() -> (T, Vec<ObjId>) {
    let mut t = Board::new(2).fill(4).battlefield(0, &["In Search of Greatness", "Grizzly Bears", "Mountain"])
        .hand(0, &["Centaur Courser", "Hill Giant", "Lightning Bolt"]).build();
    t.to_no_combat(1, Part::PrecombatMain);
    while !(t.g.active == 0 && t.g.part == Part::Upkeep) { let p = t.g.decision_maker(t.g.priority); t.g.act(p, Move::Pass).unwrap(); }
    t.pass_all();
    let cards = offered(&mut t);
    (t, cards)
}

// Rules: 608.2g, 608.2c, 701.22a
// Given: In Search of Greatness; the greatest mana value among player 0's other permanents is
// the Bears' 2.
// When: its upkeep trigger resolves; player 0 casts Centaur Courser, or declines.
// Expect: only a permanent spell of mana value 3 is offered: the Courser, not Hill Giant (4) or
// Lightning Bolt; cast, nothing is scried; declined, player 0 scries 1.
#[test]
fn r608_2g_in_search_of_greatness_offers_one_more_and_scries_if_nothing_is_cast() {
    // In Search of Greatness: "At the beginning of your upkeep, you may cast a permanent spell
    // from your hand with mana value equal to 1 plus the greatest mana value among other
    // permanents you control without paying its mana cost. If you don't, scry 1."
    let (mut t, cards) = greatness();
    cite!(["608.2g", "202.3"]; assert_eq!(names(&t, &cards), ["Centaur Courser"]));
    let mut declined = t.g.clone();
    declined.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["608.2c", "701.22a"]; assert!(matches!(declined.asked(), Some((0, Asked::Arrange { .. })))));
    t.g.act(0, Move::Cast { card: cards[0], targets: vec![] }).unwrap();
    cite!(["608.2c"]; assert!(matches!(t.g.asked(), Some((0, Asked::Priority)))));
    cite!(["608.2g"]; assert_eq!(names(&t, &t.g.stack.clone()), ["Centaur Courser"]));
}

// Rules: 608.2g, 110.2, 108.3
// Given: Silent-Blade Oni unblocked; player 1 holds Grizzly Bears, Lightning Bolt, a Mountain
// and Craw Wurm.
// When: it deals combat damage to player 1, and player 0 casts the Wurm from their hand.
// Expect: the three nonland cards are offered; the Wurm enters under player 0's control,
// still player 1's card; the other cards stay in player 1's hand.
#[test]
fn r608_2g_silent_blade_oni_casts_a_card_from_another_player_s_hand() {
    // Silent-Blade Oni: "Whenever this creature deals combat damage to a player, look at that
    // player's hand. You may cast a spell from among those cards without paying its mana cost."
    let mut t = Board::new(2).fill(4).named(0, Zone::Battlefield, "oni", "Silent-Blade Oni").hand(1, &["Grizzly Bears", "Lightning Bolt", "Mountain", "Craw Wurm"]).build();
    let oni = t.id("oni");
    to_attackers(&mut t);
    t.g.act(0, Move::Attack(vec![(oni, Target::Player(1))])).unwrap();
    to_offer(&mut t);
    let cards = offered(&mut t);
    cite!(["608.2g", "305.9"]; assert_eq!(names(&t, &cards), ["Craw Wurm", "Grizzly Bears", "Lightning Bolt"]));
    let wurm = named(&t, &cards, "Craw Wurm");
    t.g.act(0, Move::Cast { card: wurm, targets: vec![] }).unwrap();
    drain(&mut t);
    let wurm = t.find_in_battlefield("Craw Wurm");
    cite!(["110.2", "108.3"]; assert_eq!((t.g.controller(wurm), t.g.obj(wurm).owner), (0, 1)));
    cite!(["608.2g"]; assert_eq!(names(&t, &t.g.players[1].hand.clone()), ["Grizzly Bears", "Lightning Bolt", "Mountain"]));
}
