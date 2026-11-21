//! Behold (701.4a): "Reveal a [quality] card from your hand or choose a [quality] permanent you
//! control on the battlefield." As a resolving effect's optional cost answered by "if you do"
//! (118.12): Sarkhan, Dragon Ascendant and Elven Passage. As a permanent enters (614.1c,
//! 614.12a): Theorist's Sanctum. As a cost of several at once: Kindle the Inner Flame's
//! flashback (601.2h). A revealed card is shown to every player and stays in the hand
//! (701.20a-b); "if a Dragon was beheld" reads the moment it was beheld (701.4b).
use crate::support::{Board, T, Tgt, cite, mana_pool, refused};
use game::observation::Kind;
use game::simulation::Simulation;
use game::state::{Answer, Choice, Decision, Pool};
use game::{Event, Move, ObjId, Target, Zone};
use ir::Part;

fn subtype(name: &str) -> ir::Subtype { ir::Subtype::from_name(name).unwrap() }
fn treasures(t: &T) -> usize {
    t.g.battlefield.iter().filter(|o| t.g.obj(**o).token() && t.g.obj(**o).chars.subtypes.contains(&subtype("Treasure"))).count()
}
/// Each permanent chosen to be beheld, as every player is told of it.
fn beheld(t: &T) -> Vec<(usize, Vec<ObjId>)> {
    t.g.log.iter().filter_map(|e| match e { Event::Beheld { player, permanents } => Some((*player, permanents.clone())), _ => None }).collect()
}
/// The cards this seat has been shown revealed.
fn revealed(t: &T, seat: usize) -> Vec<ObjId> {
    t.g.observations(seat).iter().filter(|o| o.kind == Kind::Revealed).flat_map(|o| o.cards.iter().map(|c| c.object)).collect()
}
/// The same disclosures, as the seat's own view of the game delivers them: (object, name).
fn delivered(t: &T, seat: usize) -> Vec<(ObjId, String)> {
    let mut sim = Simulation::new(t.g.clone());
    sim.take_observations(seat).unwrap().iter().filter(|o| o.kind == Kind::Revealed && o.by == 0)
        .flat_map(|o| o.cards.iter().map(|c| (c.object, c.name.clone()))).collect()
}
fn sorted(mut v: Vec<ObjId>) -> Vec<ObjId> { v.sort(); v }

/// Cast Sarkhan, let it resolve, then let its enter trigger resolve to its first question.
fn sarkhan(t: &mut T) {
    t.mana(0, "{1}{R}").cast(0, "Sarkhan, Dragon Ascendant", vec![], mana_pool("CR")).unwrap().pass_all();
    cite!(["603.3"]; assert_eq!(t.g.stack.len(), 1, "the enter trigger is on the stack"));
    t.pass_all();
}

/// Is its controller asked whether to behold?
fn asks_may(t: &T) -> bool {
    matches!(t.g.decision, Some(Decision::Choose { player: 0, choice: Choice::May }))
}

/// "you may behold a Dragon. If you do, create a Treasure token." A Dragon card in the hand is
/// revealed: every player is shown it, and it stays in the hand.
#[test]
fn r701_4a_sarkhan_beholds_a_dragon_card_revealed_to_every_seat() {
    let mut t = Board::new(2).hand(0, &["Sarkhan, Dragon Ascendant", "Rapacious Dragon", "Iron-Shield Elf"]).build();
    let dragon = t.find_in(0, Zone::Hand, "Rapacious Dragon");
    sarkhan(&mut t);
    // the choice is made as the ability resolves, by its controller (118.12, 608.2d)
    cite!(["118.12", "608.2d"]; assert!(asks_may(&t), "{:?}", t.g.decision));
    refused(t.answer(1, Answer::Yes), "that decision is another player's");
    assert!(revealed(&t, 1).is_empty() && treasures(&t) == 0, "nothing is beheld before the choice");
    t.answer(0, Answer::Yes).unwrap();
    // the only Dragon card or permanent is what is beheld
    assert!(t.g.decision.is_none() && t.g.stack.is_empty(), "{:?}", t.g.decision);
    cite!(["118.12"]; assert_eq!(treasures(&t), 1));
    cite!(["701.20b"]; assert_eq!(t.g.obj(dragon).zone, Zone::Hand));
    for seat in 0..2 {
        cite!(["701.4a", "701.20a"]; case: format!("seat {seat}"); assert_eq!(revealed(&t, seat), vec![dragon]));
        cite!(["701.20a"]; case: format!("seat {seat}"); assert_eq!(delivered(&t, seat), vec![(dragon, "Rapacious Dragon".to_string())]));
    }
    assert!(beheld(&t).is_empty(), "a card was revealed, not a permanent chosen");
    assert!(!t.g.sees(1, dragon), "the reveal grants no ongoing look at the hand");
}

/// A Dragon permanent you control may be chosen instead; nothing is done to it. An opponent's
/// Dragon, a Dragon card in an opponent's hand, and a card of another type are not offered.
#[test]
fn r701_4a_a_dragon_permanent_you_control_is_chosen_and_left_as_it_is() {
    let mut t = Board::new(2).hand(0, &["Sarkhan, Dragon Ascendant", "Rapacious Dragon", "Iron-Shield Elf"])
        .battlefield(0, &["Firespitter Whelp"]).battlefield(1, &["Rapacious Dragon"]).hand(1, &["Rapacious Dragon"]).build();
    let (card, mine) = (t.find_in(0, Zone::Hand, "Rapacious Dragon"), t.find_in(0, Zone::Battlefield, "Firespitter Whelp"));
    let (theirs, their_card, elf) = (t.find_in(1, Zone::Battlefield, "Rapacious Dragon"), t.find_in(1, Zone::Hand, "Rapacious Dragon"), t.find_in(0, Zone::Hand, "Iron-Shield Elf"));
    sarkhan(&mut t);
    t.answer(0, Answer::Yes).unwrap();
    let Some(Decision::Choose { player: 0, choice: Choice::Objects { verb: ir::Verb::Behold, to: None, among, count: 1 } }) = t.g.decision.clone() else { panic!("{:?}", t.g.decision) };
    cite!(["701.4a"]; assert_eq!(sorted(among), sorted(vec![card, mine])));
    for other in [theirs, their_card, elf] {
        cite!(["701.4a"]; refused("a chosen object is not among the offered objects to behold (608.2d)"): t.answer(0, Answer::Objects(vec![other])));
    }
    refused(t.answer(0, Answer::Objects(vec![])), "choose exactly 1 of them to behold (608.2d)");
    t.answer(0, Answer::Objects(vec![mine])).unwrap();
    cite!(["701.4a"]; assert_eq!(beheld(&t), vec![(0, vec![mine])]));
    cite!(["701.4a"]; assert!(t.g.battlefield.contains(&mine) && !t.g.obj(mine).tapped));
    assert!(revealed(&t, 1).is_empty() && delivered(&t, 1).is_empty(), "no card was revealed");
    cite!(["118.12"]; assert_eq!(treasures(&t), 1));
}

/// "If you do" is false when the player declines, and when there is nothing they could behold:
/// a cost can't be paid without the resources to pay it (118.3).
#[test]
fn r118_12_declining_or_having_nothing_to_behold_creates_no_treasure() {
    let mut t = Board::new(2).hand(0, &["Sarkhan, Dragon Ascendant", "Rapacious Dragon"]).build();
    sarkhan(&mut t);
    assert!(asks_may(&t), "{:?}", t.g.decision);
    t.answer(0, Answer::No).unwrap();
    cite!(["118.12"]; assert_eq!(treasures(&t), 0));
    assert!(revealed(&t, 1).is_empty() && beheld(&t).is_empty() && t.g.stack.is_empty());

    // a Dragon card in your graveyard, an opponent's Dragon and Dragon card: none of them yours
    // to behold. Sarkhan himself is a Human Druid.
    let mut t = Board::new(2).hand(0, &["Sarkhan, Dragon Ascendant", "Iron-Shield Elf"]).graveyard(0, &["Rapacious Dragon"])
        .battlefield(1, &["Rapacious Dragon"]).hand(1, &["Rapacious Dragon"]).build();
    sarkhan(&mut t);
    cite!(["118.3", "701.4a"]; assert!(t.g.decision.is_none() && t.g.stack.is_empty(), "{:?}", t.g.decision));
    cite!(["118.12"]; assert_eq!(treasures(&t), 0));
    assert!(revealed(&t, 1).is_empty() && beheld(&t).is_empty());
}

/// Rules: 603.2, 122.1a, 205.1b, 613.1f, 514.2
/// Given: Sarkhan on the battlefield; each player has a Shivan Dragon in hand.
/// When: the opponent's Dragon enters on their turn; then Sarkhan's controller's on theirs.
/// Expect: only the Dragon you control triggers "Whenever a Dragon you control enters, put a
/// +1/+1 counter on Sarkhan. Until end of turn, Sarkhan becomes a Dragon in addition to his
/// other types and gains flying."; the counter stays after the turn, the type and flying end.
#[test]
fn r603_2_sarkhan_grows_and_is_a_flying_dragon_until_end_of_turn_when_a_dragon_you_control_enters() {
    let mut t = Board::new(2).active(1).battlefield(0, &["Sarkhan, Dragon Ascendant"]).hand(0, &["Shivan Dragon"])
        .hand(1, &["Shivan Dragon"]).fill(5).build();
    let sarkhan = t.find_in(0, Zone::Battlefield, "Sarkhan, Dragon Ascendant");
    let dragon = |t: &T| t.g.seen(sarkhan, None).subtypes.contains(&subtype("Dragon"));
    t.mana(1, "{4}{R}{R}").cast(1, "Shivan Dragon", vec![], mana_pool("CCCCRR")).unwrap().pass_all();
    cite!(["603.2"]; case: "an opponent's Dragon"; assert!(t.g.stack.is_empty() && t.g.obj(sarkhan).counters.is_empty() && !dragon(&t)));
    t.to_no_combat(0, Part::PrecombatMain);
    t.mana(0, "{4}{R}{R}").cast(0, "Shivan Dragon", vec![], mana_pool("CCCCRR")).unwrap().pass_all();
    cite!(["603.2", "603.3"]; assert_eq!(t.g.stack.len(), 1, "Sarkhan's trigger"));
    t.pass_all();
    cite!(["122.1a", "613.4c"]; assert_eq!((t.g.power(sarkhan), t.g.toughness(sarkhan)), (Some(3), Some(3))));
    let seen = t.g.seen(sarkhan, None);
    cite!(["205.1b"]; assert!(["Dragon", "Human", "Druid"].iter().all(|s| seen.subtypes.contains(&subtype(s))), "{:?}", seen.subtypes));
    cite!(["613.1f"]; assert!(t.g.has(sarkhan, "Flying")));
    t.to_no_combat(1, Part::PrecombatMain);
    cite!(["514.2"]; assert!(!dragon(&t) && !t.g.has(sarkhan, "Flying")));
    cite!(["122.1a", "514.2"]; assert_eq!((t.g.power(sarkhan), t.g.toughness(sarkhan)), (Some(3), Some(3))));
}

/// Elven Passage: "{T}, Pay 1 life, Sacrifice this land: Search your library for a basic land
/// card, put it onto the battlefield tapped, then shuffle. You may behold an Elf. If you do,
/// untap that land." "That land" is the land put onto the battlefield; the behold is offered
/// whether or not one was found, and with no Elf to behold it is not done.
#[test]
fn r701_4a_elven_passage_untaps_the_land_it_found_only_if_an_elf_is_beheld() {
    #[derive(Debug, Clone, Copy, PartialEq)]
    enum Case { HandElf, ElfPermanent, Declined, NoElf, NotFound }
    for case in [Case::HandElf, Case::ElfPermanent, Case::Declined, Case::NoElf, Case::NotFound] {
        let named = format!("{case:?}");
        let board = Board::new(2).battlefield(0, &["Elven Passage"]).library(0, &["Forest", "Iron-Shield Elf"]).hand(1, &["Iron-Shield Elf"]);
        let mut t = match case {
            Case::ElfPermanent => board.battlefield(0, &["Dawn's Light Archer"]),
            Case::NoElf => board.hand(0, &["Firespitter Whelp"]).battlefield(1, &["Dawn's Light Archer"]),
            _ => board.hand(0, &["Iron-Shield Elf"]),
        }.build();
        let forest = t.find_in(0, Zone::Library, "Forest");
        t.activate(0, "Elven Passage", 0, vec![], Pool::default()).unwrap();
        cite!(["602.2b", "118.3b"]; case: named; assert_eq!((t.g.players[0].life, t.g.players[0].graveyard.len()), (19, 1)));
        t.pass_all();
        let Some(Decision::Choose { player: 0, choice: Choice::Search { among, .. } }) = t.g.decision.clone() else { panic!("{:?}", t.g.decision) };
        cite!(["701.23a"]; case: named; assert_eq!(among, vec![forest]));
        // a player searching a hidden zone for a stated quality need not find it (701.23b)
        t.answer(0, Answer::Objects(if case == Case::NotFound { vec![] } else { vec![forest] })).unwrap();
        let land = (case != Case::NotFound).then(|| t.find_in(0, Zone::Battlefield, "Forest"));
        if let Some(land) = land { cite!(["701.26a"]; case: named; assert!(t.g.obj(land).tapped, "put onto the battlefield tapped")); }
        match case {
            Case::NoElf => cite!(["118.3", "701.4a"]; assert!(t.g.decision.is_none(), "{:?}", t.g.decision)),
            Case::Declined => { cite!(["608.2c", "118.12"]; case: named; assert!(asks_may(&t), "{:?}", t.g.decision)); t.answer(0, Answer::No).unwrap(); }
            _ => { cite!(["608.2c", "118.12"]; case: named; assert!(asks_may(&t), "{:?}", t.g.decision)); t.answer(0, Answer::Yes).unwrap(); }
        }
        assert!(t.g.decision.is_none() && t.g.stack.is_empty(), "{named}: {:?}", t.g.decision);
        match case {
            Case::HandElf | Case::NotFound => {
                let elf = t.find_in(0, Zone::Hand, "Iron-Shield Elf");
                for seat in 0..2 { cite!(["701.4a", "701.20a"]; case: format!("{named} seat {seat}"); assert_eq!(revealed(&t, seat), vec![elf])); }
                cite!(["701.20b"]; case: named; assert_eq!(t.g.obj(elf).zone, Zone::Hand));
            }
            Case::ElfPermanent => {
                let archer = t.find_in(0, Zone::Battlefield, "Dawn's Light Archer");
                cite!(["701.4a"]; assert_eq!(beheld(&t), vec![(0, vec![archer])]));
                assert!(!t.g.obj(archer).tapped && revealed(&t, 1).is_empty());
            }
            Case::Declined | Case::NoElf => assert!(revealed(&t, 1).is_empty() && beheld(&t).is_empty(), "{named}"),
        }
        match land {
            Some(land) => cite!(["118.12", "701.26b"]; case: named; assert_eq!(t.g.obj(land).tapped, !matches!(case, Case::HandElf | Case::ElfPermanent))),
            None => cite!(["701.23b"]; assert!(t.g.battlefield.iter().all(|o| t.g.controller(*o) != 0), "nothing was found to put onto the battlefield")),
        }
    }
}

fn play(t: &mut T, name: &str) { let o = t.find_in(0, Zone::Hand, name); t.g.act(0, Move::PlayLand(o)).unwrap(); }
/// The entry question: at most one of these to behold (614.12a).
fn offered(t: &T) -> Vec<ObjId> {
    let Some(Decision::Choose { player: 0, choice: Choice::Select { among, min: 0, max: 1 } }) = &t.g.decision else { panic!("{:?}", t.g.decision) };
    among.clone()
}
const SANCTUM: &str = "Theorist's Sanctum";

/// Theorist's Sanctum: "As this land enters, you may behold a Jace. If you don't, this land
/// enters tapped." The choice is made before it enters (614.12a). A Jace card revealed from
/// the hand is shown to every player; Eye of Jace is not a Jace card, and an opponent's Jace
/// card or Jace planeswalker is not the player's to behold.
#[test]
fn r614_12a_theorists_sanctum_beholds_a_jace_card_before_it_enters_untapped() {
    let mut t = Board::new(2).hand(0, &[SANCTUM, "Jace Beleren", "Eye of Jace"]).hand(1, &["Jace Beleren"])
        .battlefield(1, &["Jace Beleren"]).counters("Jace Beleren", "loyalty", 3).build();
    let jace = t.find_in(0, Zone::Hand, "Jace Beleren");
    play(&mut t, SANCTUM);
    cite!(["701.4a"]; assert_eq!(offered(&t), vec![jace]));
    cite!(["614.12a"]; assert!(t.g.battlefield.iter().all(|o| t.g.obj(*o).chars.name != SANCTUM)));
    refused(t.answer(1, Answer::Objects(vec![jace])), "that decision is another player's");
    let (eye, theirs) = (t.find_in(0, Zone::Hand, "Eye of Jace"), t.find_in(1, Zone::Battlefield, "Jace Beleren"));
    for other in [eye, theirs, t.find_in(1, Zone::Hand, "Jace Beleren")] {
        refused(t.answer(0, Answer::Objects(vec![other])), "a chosen object is not among the offered objects (608.2d)");
    }
    assert!(revealed(&t, 1).is_empty());
    t.answer(0, Answer::Objects(vec![jace])).unwrap();
    cite!(["614.12a"]; assert!(!t.g.obj(t.find_in_battlefield(SANCTUM)).tapped));
    cite!(["701.20b"]; assert_eq!(t.g.obj(jace).zone, Zone::Hand));
    for seat in 0..2 {
        cite!(["701.4a", "701.20a"]; case: format!("seat {seat}"); assert_eq!(revealed(&t, seat), vec![jace]));
        cite!(["701.20a"]; case: format!("seat {seat}"); assert_eq!(delivered(&t, seat), vec![(jace, "Jace Beleren".to_string())]));
    }
    assert!(beheld(&t).is_empty() && !t.g.sees(1, jace));
}

/// A Jace permanent you control is chosen instead, here the Jace planeswalker token another
/// Sanctum's "Empower Jace 2" creates (701.71a); nothing is done to it.
#[test]
fn r614_12a_theorists_sanctum_beholds_the_jace_token_another_sanctum_empowered() {
    let mut t = Board::new(2).named(0, Zone::Battlefield, "first", SANCTUM).hand(0, &[SANCTUM]).build();
    t.mana(0, "{2}{U}").activate(0, "first", 1, vec![], mana_pool("CCU")).unwrap().pass_all();
    let token = t.find_in(0, Zone::Battlefield, "Jace Token");
    cite!(["701.71a"]; assert_eq!(t.g.obj(token).counters.get("loyalty").copied(), Some(2)));
    let mark = t.mark();
    play(&mut t, SANCTUM);
    cite!(["701.4a"]; assert_eq!(offered(&t), vec![token]));
    t.answer(0, Answer::Objects(vec![token])).unwrap();
    let second = t.entered_since(mark, 0, Zone::Battlefield, SANCTUM);
    cite!(["614.12a"]; assert!(!t.g.obj(second).tapped));
    cite!(["701.4a"]; assert_eq!(beheld(&t), vec![(0, vec![token])]));
    assert_eq!(t.g.obj(token).counters.get("loyalty").copied(), Some(2), "nothing is done to what is beheld");
    assert!(revealed(&t, 1).is_empty());
}

/// "If you don't, this land enters tapped": declined, or with no Jace to behold, when no
/// question is asked (118.3).
#[test]
fn r614_1c_theorists_sanctum_enters_tapped_unless_a_jace_is_beheld() {
    let mut t = Board::new(2).hand(0, &[SANCTUM, "Jace Beleren"]).build();
    play(&mut t, SANCTUM);
    assert_eq!(offered(&t).len(), 1);
    t.answer(0, Answer::Objects(vec![])).unwrap();
    cite!(["614.1c", "614.1d"]; assert!(t.g.obj(t.find_in_battlefield(SANCTUM)).tapped));
    assert!(revealed(&t, 1).is_empty() && beheld(&t).is_empty());

    let mut t = Board::new(2).hand(0, &[SANCTUM, "Eye of Jace"]).hand(1, &["Jace Beleren"])
        .battlefield(1, &["Jace Beleren"]).counters("Jace Beleren", "loyalty", 3).build();
    play(&mut t, SANCTUM);
    cite!(["118.3", "614.12a"]; assert!(t.g.decision.is_none(), "{:?}", t.g.decision));
    cite!(["614.1c", "614.1d"]; assert!(t.g.obj(t.find_in_battlefield(SANCTUM)).tapped));
    assert!(revealed(&t, 1).is_empty() && beheld(&t).is_empty());
}

/// "If a Dragon was beheld" is about the moment of beholding (701.4b): a Dragon beheld to cast
/// Piercing Exhale that becomes a Frog before the spell resolves was still beheld. Without the
/// behold, no surveil.
#[test]
fn r701_4b_a_dragon_beheld_for_piercing_exhale_was_beheld_after_it_becomes_a_frog() {
    for behold in [true, false] {
        let mut t = Board::new(2).hand(0, &["Piercing Exhale"]).battlefield(0, &["Grizzly Bears", "Rapacious Dragon"])
            .library(0, &["Forest", "Forest", "Forest"]).hand(1, &["Turn to Frog"]).battlefield(1, &["Dawn's Light Archer"]).build();
        let (card, bears, dragon, archer) = (t.find_in(0, Zone::Hand, "Piercing Exhale"), t.find_in(0, Zone::Battlefield, "Grizzly Bears"),
            t.find_in(0, Zone::Battlefield, "Rapacious Dragon"), t.find_in(1, Zone::Battlefield, "Dawn's Light Archer"));
        let cast = Move::Cast { card, targets: vec![vec![Target::Object(bears)], vec![Target::Object(archer)]] };
        let cast = Move::WithOptional(behold, Box::new(if behold { Move::WithCosts(vec![vec![dragon]], Box::new(cast)) } else { cast }));
        t.mana(0, "{1}{G}");
        t.g.act(0, Move::Paying(mana_pool("CG"), Box::new(cast))).unwrap();
        cite!(["701.4a", "601.2h"]; case: behold; assert_eq!(beheld(&t), if behold { vec![(0, vec![dragon])] } else { vec![] }));
        t.g.act(0, Move::Pass).unwrap();
        t.mana(1, "{1}{U}").cast(1, "Turn to Frog", vec![Tgt::Object(dragon)], mana_pool("CU")).unwrap().pass_all();
        let seen = t.g.seen(dragon, None);
        cite!(["205.1a", "613.1d"]; case: behold; assert!(!seen.subtypes.contains(&subtype("Dragon")) && seen.subtypes.contains(&subtype("Frog"))));
        assert_eq!(t.g.stack.len(), 1, "Piercing Exhale is still on the stack");
        t.pass_all();
        cite!(["120.3"]; case: behold; assert_eq!(t.g.obj(archer).damage, 2));
        let surveil = matches!(t.g.decision, Some(Decision::Choose { player: 0, choice: Choice::Arrange { verb: ir::Verb::Surveil, .. } }));
        cite!(["701.4b", "701.25a"]; case: behold; assert_eq!(surveil, behold, "{:?}", t.g.decision));
    }
}

const KINDLE: &str = "Kindle the Inner Flame";
fn kindle_flashback(card: ObjId, beheld: Vec<ObjId>, target: ObjId) -> Move {
    Move::Paying(mana_pool("CR"), Box::new(Move::CastVia(game::announcement::CastRoute::Flashback,
        Box::new(Move::WithCosts(vec![beheld], Box::new(Move::Cast { card, targets: vec![vec![Target::Object(target)]] }))))))
}

/// Rules: 701.4a, 702.34a, 601.2h, 701.20a, 308.2, 707.9b
/// Given: Kindle the Inner Flame in the graveyard ("Flashback—{1}{R}, Behold three
/// Elementals."); in hand an Elemental creature card, another Kindle (a Kindred Elemental
/// card, 308.2) and a Bear card; an Elemental you control; an opponent's Elemental; an
/// Elemental card in the graveyard.
/// Expect: three different Elementals of the hand and the battlefield together pay the cost
/// (ruling 2025-11-17: "reveal and choose any combination that adds up to three. You can't
/// behold the same object more than once"); fewer, the same one twice, a non-Elemental, an
/// opponent's Elemental or a graveyard card do not. The revealed cards are shown to every
/// player and stay in the hand; the copy has haste and is sacrificed at the end step; the
/// flashback card is exiled.
#[test]
fn r701_4a_kindle_the_inner_flame_beholds_three_elementals_of_the_hand_and_the_battlefield() {
    let mut t = Board::new(2).named(0, Zone::Graveyard, "cast", KINDLE).graveyard(0, &["Earth Elemental"])
        .named(0, Zone::Hand, "kindred", KINDLE).hand(0, &["Fire Elemental", "Grizzly Bears"])
        .battlefield(0, &["Air Elemental"]).battlefield(1, &["Earth Elemental"]).fill(3).build();
    let (card, kindred) = (t.id("cast"), t.id("kindred"));
    let (fire, bears, air) = (t.find_in(0, Zone::Hand, "Fire Elemental"), t.find_in(0, Zone::Hand, "Grizzly Bears"), t.find_in(0, Zone::Battlefield, "Air Elemental"));
    let (theirs, buried) = (t.find_in(1, Zone::Battlefield, "Earth Elemental"), t.find_in(0, Zone::Graveyard, "Earth Elemental"));
    t.mana(0, "{1}{R}");
    cite!(["701.4a", "601.2h"]; refused("which asks for exactly 3 card(s): 2 chosen"): t.g.act(0, kindle_flashback(card, vec![fire, kindred], air)));
    cite!(["601.2h"]; refused("a card is chosen once for that cost (601.2h)"): t.g.act(0, kindle_flashback(card, vec![fire, kindred, fire], air)));
    for other in [bears, theirs, buried] {
        let name = t.g.obj(other).chars.name.clone();
        cite!(["701.4a"]; case: name.clone(); refused(&format!("{name} is not what that cost asks for")): t.g.act(0, kindle_flashback(card, vec![fire, kindred, other], air)));
    }
    assert!(t.g.stack.is_empty() && revealed(&t, 1).is_empty() && beheld(&t).is_empty(), "a refused cast pays nothing");
    t.g.act(0, kindle_flashback(card, vec![fire, kindred, air], air)).unwrap();
    for seat in 0..2 {
        cite!(["701.4a", "701.20a", "308.2"]; case: format!("seat {seat}"); assert_eq!(sorted(revealed(&t, seat)), sorted(vec![fire, kindred])));
    }
    cite!(["701.4a"]; assert_eq!(beheld(&t), vec![(0, vec![air])]));
    cite!(["701.20b"]; assert!(t.g.obj(fire).zone == Zone::Hand && t.g.obj(kindred).zone == Zone::Hand));
    let mark = t.mark();
    t.pass_all();
    let [token] = t.created_since(mark)[..] else { panic!("one token: {:?}", t.created_since(mark)) };
    cite!(["707.2", "707.9b"]; assert!(t.g.obj(token).chars.name == "Air Elemental" && t.g.has(token, "Flying") && t.g.has(token, "Haste")));
    cite!(["702.34a"]; assert!(t.g.exile.iter().any(|o| t.g.obj(*o).chars.name == KINDLE && t.g.obj(*o).owner == 0)));
    assert!(t.g.obj(air).zone == Zone::Battlefield && !t.g.obj(air).tapped, "nothing is done to the beheld permanent");
    t.to_no_combat(0, Part::End);
    cite!(["707.9b", "603.2", "513.1"]; assert!(!t.g.battlefield.contains(&token), "sacrificed at the beginning of the end step"));
    assert!(t.g.battlefield.contains(&air));
}
