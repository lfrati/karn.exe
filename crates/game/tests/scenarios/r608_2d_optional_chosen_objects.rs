//! "You may exile a card from your graveyard", "you may put a card an opponent owns from exile
//! into that player's graveyard": whether and which are one choice of the player who follows
//! the instruction (608.2d), among the cards that fit as it is followed. "If you do" asks
//! whether they were chosen; with too few cards that fit, nothing is asked and it wasn't done.
//! A card exiled face down that player may not look at is chosen as its pile, and the card
//! put away is one of that pile at random (406.4).
use crate::support::{Board, Boundary, T, Tgt, cite, mana_pool, paid};
use game::legal::Asked;
use game::state::Answer;
use game::{Move, ObjId, Zone};
use ir::Part;

/// Player 0 casts this card from their hand, paying `mana`, until a question or priority.
fn cast(t: &mut T, name: &str, targets: Vec<Tgt>, mana: &str) -> Boundary {
    t.mana(0, mana).cast(0, name, targets, mana_pool(mana)).unwrap();
    t.settle()
}

/// What player 0 is offered to choose, `count` of them or none.
fn offered(boundary: Boundary, count: usize) -> Vec<ObjId> {
    match boundary {
        Boundary::Question(0, Asked::OptionalObjects { among, count: n, .. }) if n == count => among,
        other => panic!("expected an optional choice of {count}: {other:?}"),
    }
}

fn argument(graveyard: &[&str]) -> T {
    Board::new(2).fill(5).at(Part::PrecombatMain).hand(0, &["Heated Argument"]).graveyard(0, graveyard).battlefield(1, &["Serra Angel"]).build()
}

#[test]
fn r608_2d_whether_and_which_card_are_one_choice_and_if_you_do_asks_whether_one_was_chosen() {
    let mut t = argument(&["Mountain"]);
    let mountain = t.find_in(0, Zone::Graveyard, "Mountain");
    let among = offered(cast(&mut t, "Heated Argument", vec![Tgt::In(1, Zone::Battlefield, "Serra Angel")], "{4}{R}"), 1);
    cite!(["608.2d"]; assert_eq!(among, vec![mountain]));
    t.answer(0, Answer::OptionalObjects(Some(vec![mountain]))).unwrap().resolve();
    t.expect().rules(["608.2d", "608.2c", "120.3"]).zone(0, Zone::Exile, &["Mountain"]).zone(1, Zone::Graveyard, &["Serra Angel"]).life(1, 18).check();

    // declined, it is not done
    let mut t = argument(&["Mountain"]);
    offered(cast(&mut t, "Heated Argument", vec![Tgt::In(1, Zone::Battlefield, "Serra Angel")], "{4}{R}"), 1);
    t.answer(0, Answer::OptionalObjects(None)).unwrap().resolve();
    t.expect().rules(["608.2d", "608.2c"]).zone(0, Zone::Graveyard, &["Heated Argument", "Mountain"]).life(1, 20).check();

    // with no card to exile, nothing is asked
    let mut t = argument(&[]);
    let boundary = cast(&mut t, "Heated Argument", vec![Tgt::In(1, Zone::Battlefield, "Serra Angel")], "{4}{R}");
    cite!(["608.2d"]; assert!(matches!(boundary, Boundary::Priority(_)), "{boundary:?}"));
    t.expect().rules(["608.2c"]).zone(1, Zone::Graveyard, &["Serra Angel"]).life(1, 20).check();
}

fn skaab(graveyard: &[&str]) -> T {
    Board::new(2).fill(5).at(Part::PrecombatMain).hand(0, &["Skywarp Skaab"]).graveyard(0, graveyard).build()
}

#[test]
fn r608_2d_two_cards_or_none_are_chosen_and_with_one_that_fits_nothing_is_done() {
    let mut t = skaab(&["Grizzly Bears", "Hill Giant", "Mountain"]);
    let (bears, giant) = (t.find_in(0, Zone::Graveyard, "Grizzly Bears"), t.find_in(0, Zone::Graveyard, "Hill Giant"));
    let mut among = offered(cast(&mut t, "Skywarp Skaab", vec![], "{3}{U}{U}"), 2);
    among.sort();
    let mut creatures = vec![bears, giant];
    creatures.sort();
    cite!(["608.2d"]; assert_eq!(among, creatures));
    cite!(["608.2d"]; refused("accept with exactly 2 distinct offered objects (608.2d)"): t.g.act(0, Move::Choose(Answer::OptionalObjects(Some(vec![bears])))));
    t.answer(0, Answer::OptionalObjects(Some(vec![bears, giant]))).unwrap().resolve();
    t.expect().rules(["608.2d", "608.2c", "121.1"]).zone(0, Zone::Exile, &["Grizzly Bears", "Hill Giant"]).zone(0, Zone::Hand, &["Wastes"]).check();

    let mut t = skaab(&["Grizzly Bears", "Mountain"]);
    let boundary = cast(&mut t, "Skywarp Skaab", vec![], "{3}{U}{U}");
    cite!(["608.2d"]; assert!(matches!(boundary, Boundary::Priority(_)), "{boundary:?}"));
    t.expect().rules(["608.2c"]).zone(0, Zone::Graveyard, &["Grizzly Bears", "Mountain"]).zone(0, Zone::Hand, &[]).check();
}

#[test]
fn r406_3_a_card_an_opponent_owns_is_put_from_exile_into_their_graveyard() {
    let mut t = Board::new(2).fill(5).at(Part::PrecombatMain).hand(0, &["Mind Raker"]).exile(0, &["Shock"])
        .exile(1, &["Lightning Bolt"]).hand(1, &["Grizzly Bears"]).build();
    let bolt = t.find_in(1, Zone::Exile, "Lightning Bolt");
    let among = offered(cast(&mut t, "Mind Raker", vec![], "{3}{B}"), 1);
    // a card its own controller owns is not one an opponent owns
    cite!(["108.3", "608.2d"]; assert_eq!(among, vec![bolt]));
    t.answer(0, Answer::OptionalObjects(Some(vec![bolt]))).unwrap().resolve();
    t.expect().rules(["404.2", "701.9a"]).zone(1, Zone::Graveyard, &["Lightning Bolt", "Grizzly Bears"]).zone(0, Zone::Exile, &["Shock"]).check();
}

#[test]
fn r406_4_a_card_exiled_face_down_is_chosen_as_its_pile() {
    let mut t = Board::new(2).fill(5).active(1).at(Part::PrecombatMain).hand(0, &["Mind Raker"]).hand(1, &["Demon Bolt"]).build();
    let card = t.find_in(1, Zone::Hand, "Demon Bolt");
    t.mana(1, "{2}");
    t.g.act(1, paid("CC", Move::Foretell(card))).unwrap();
    let foretold = *t.g.exile.last().expect("the foretold card");
    t.to_no_combat(0, Part::PrecombatMain);
    cite!(["406.3"]; assert!(!t.g.sees(0, foretold)));
    // the one card of its pile stands for the pile
    let among = offered(cast(&mut t, "Mind Raker", vec![], "{3}{B}"), 1);
    cite!(["406.4"]; assert_eq!(among, vec![foretold]));
    t.answer(0, Answer::OptionalObjects(Some(vec![foretold]))).unwrap().resolve();
    t.expect().rules(["406.4", "400.7"]).zone(1, Zone::Graveyard, &["Demon Bolt"]).zone(1, Zone::Exile, &[]).check();
}

#[test]
fn r607_2a_a_creature_card_exiled_with_this_creature_is_chosen_among_the_cards_it_exiled() {
    use game::Target;
    let mut t = Board::new(2).fill(5).at(Part::PrecombatMain).battlefield(0, &["Emperor of Bones"])
        .graveyard(1, &["Grizzly Bears"]).exile(1, &["Hill Giant"]).build();
    let (bears, giant) = (t.find_in(1, Zone::Graveyard, "Grizzly Bears"), t.find_in(1, Zone::Exile, "Hill Giant"));
    // at the beginning of combat it exiles the bears
    t.g.act(0, Move::Pass).unwrap();
    t.g.act(1, Move::Pass).unwrap();
    let Some((0, Asked::Targets { .. })) = t.asked() else { panic!("expected the exile trigger's target: {:?}", t.asked()) };
    t.g.act(0, Move::ChooseTargets(vec![vec![Target::Object(bears)]])).unwrap();
    t.resolve();
    let exiled = t.find_in(1, Zone::Exile, "Grizzly Bears");
    // adapting, it puts a creature card it exiled onto the battlefield: not one another
    // effect exiled (607.2a)
    t.mana(0, "{1}{B}").activate(0, "Emperor of Bones", 1, vec![], mana_pool("{1}{B}")).unwrap();
    match t.settle() {
        Boundary::Priority(_) => {}
        Boundary::Question(0, Asked::Objects { among, .. }) => {
            cite!(["607.2a"]; assert_eq!(among, vec![exiled]));
            t.answer(0, Answer::Objects(vec![exiled])).unwrap().resolve();
        }
        other => panic!("unexpected {other:?}"),
    }
    t.expect().rules(["607.2a", "608.2d"]).zone(0, Zone::Battlefield, &["Emperor of Bones", "Grizzly Bears"]).zone(1, Zone::Exile, &["Hill Giant"])
        .counters("Grizzly Bears", "finality", 1).check();
    cite!(["607.2a"]; assert!(t.g.exile.contains(&giant)));
}
