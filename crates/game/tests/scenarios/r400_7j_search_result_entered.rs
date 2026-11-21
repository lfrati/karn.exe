//! 400.7j: "If an effect causes an object to move to a public zone, other parts of that effect
//! can find that object." A card a search found and the same effect put onto the battlefield
//! is found there by the effect's later "that land" (701.23a): Fabled Passage. Were this
//! rule not followed for a search's result, "untap that land" would be done to the card as it
//! had been in the library, and the land would stay tapped.
use crate::support::{Board, T, cite};
use game::state::{Answer, Choice, Decision, Pool};
use game::Zone;

/// Activate Fabled Passage and find the Forest of the library among its basic land cards,
/// returning the Forest's permanent once the ability has resolved.
fn fetch(t: &mut T) -> game::ObjId {
    let forest = t.find_in(0, Zone::Library, "Forest");
    // every card of the declared libraries is a basic land card (205.4c)
    let mut basics = t.g.players[0].library.clone();
    basics.sort();
    t.activate(0, "Fabled Passage", 0, vec![], Pool::default()).unwrap();
    t.pass_all();
    let Some(Decision::Choose { player: 0, choice: Choice::Search { mut among, .. } }) = t.g.decision.clone() else { panic!("{:?}", t.g.decision) };
    among.sort();
    cite!(["701.23a"]; assert_eq!(among, basics));
    let mark = t.mark();
    t.answer(0, Answer::Objects(vec![forest])).unwrap();
    assert!(t.g.decision.is_none() && t.g.stack.is_empty(), "{:?}", t.g.decision);
    t.entered_since(mark, 0, Zone::Battlefield, "Forest")
}

/// Rules: 400.7j, 701.23a, 608.2c
/// Given: Fabled Passage and three other lands, one of them tapped; a Forest in the library.
/// When: Passage is sacrificed to find the Forest, which enters tapped: four lands.
/// Expect: the Forest is untapped; the land tapped before stays tapped.
/// Why: "that land" is the land the effect put onto the battlefield, found there (400.7j).
#[test]
fn r400_7j_fabled_passage_untaps_the_land_it_put_onto_the_battlefield() {
    let mut t = Board::new(2).battlefield(0, &["Fabled Passage", "Plains", "Island"]).tapped(0, &["Swamp"])
        .library(0, &["Forest", "Mountain"]).battlefield(1, &["Plains", "Plains"]).build();
    let swamp = t.find_in(0, Zone::Battlefield, "Swamp");
    let land = fetch(&mut t);
    cite!(["701.21a"]; assert!(t.g.players[0].graveyard.iter().any(|o| t.g.obj(*o).chars.name == "Fabled Passage")));
    cite!(["400.7j", "608.2c"]; assert!(!t.g.obj(land).tapped, "four lands: that land is untapped"));
    assert!(t.g.obj(swamp).tapped, "only that land is untapped");
}

/// Rules: 400.7j, 608.2c
/// Given: Fabled Passage and two other lands; an opponent's two lands do not count.
/// When: Passage is sacrificed to find the Forest: three lands you control.
/// Expect: the Forest stays tapped as it entered.
#[test]
fn r608_2c_fabled_passage_leaves_the_land_tapped_with_fewer_than_four_lands() {
    let mut t = Board::new(2).battlefield(0, &["Fabled Passage", "Plains", "Island"])
        .library(0, &["Forest"]).battlefield(1, &["Plains", "Plains"]).build();
    let land = fetch(&mut t);
    cite!(["608.2c"]; assert!(t.g.obj(land).tapped, "three lands: it stays tapped"));
}
