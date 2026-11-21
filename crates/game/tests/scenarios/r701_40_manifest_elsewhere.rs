//! Manifesting from other places than the top card of a library (701.40a): a card from a
//! hand, chosen by the player who manifests it; and several cards of a library, one at a time
//! (701.40e). Revealing Wind lets its caster look at face-down attackers and blockers.
use crate::support;
use game::face::{Concealment, Down, Face, Way};
use game::observation::Kind;
use support::*;

fn face_down(t: &T) -> Vec<ObjId> {
    t.g.battlefield.iter().copied().filter(|o| t.g.face_down(*o)).collect()
}

#[test]
fn scroll_of_fate_manifests_a_card_chosen_from_the_hand_unseen_by_the_opponent() {
    let mut t = Board::new(2).battlefield(0, &["Scroll of Fate"]).hand(0, &["Grizzly Bears", "Forest"]).build();
    let (bears, forest) = (t.find_in(0, Zone::Hand, "Grizzly Bears"), t.find_in(0, Zone::Hand, "Forest"));
    for p in 0..2 { t.g.take_observations(p); }
    t.activate(0, "Scroll of Fate", 0, vec![], crate::support::mana_pool("")).unwrap();
    while t.g.decision.is_none() && !t.g.stack.is_empty() { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    let Some(Decision::Choose { player: 0, choice: Choice::Objects { among, count: 1, .. } }) = t.g.decision.clone() else { panic!("{:?}", t.g.decision) };
    let mut among = among;
    among.sort();
    let mut hand = vec![bears, forest];
    hand.sort();
    cite!(["608.2d"]; assert_eq!(among, hand, "any card in the hand"));
    t.g.act(0, Move::Choose(Answer::Objects(vec![bears]))).unwrap();
    t.resolve();
    let down = face_down(&t);
    assert_eq!(down.len(), 1);
    let o = down[0];
    cite!(["701.40a"]; assert_eq!(t.g.face(o), Face::Down(Down { by: Concealment::Manifested })));
    cite!(["701.40a"]; assert_eq!(t.g.physical_card(o).unwrap().1.name(), "Grizzly Bears"));
    assert_eq!(t.g.players[0].hand, vec![forest]);
    cite!(["708.5"]; assert!(t.g.observations(1).is_empty(), "the opponent is shown nothing"));
    cite!(["708.5"]; assert!(game::snapshot::Snapshot::of(&t.g, Some(1)).objects.into_iter().find(|s| s.id == o).unwrap().face_down.unwrap().card.is_none()));
    // turned face up for its mana cost (701.40b)
    t.mana(0, "{1}{G}");
    t.g.act(0, crate::support::paid("CG", Move::TurnFaceUp(o, Way::Manifested))).unwrap();
    cite!(["701.40b"]; assert_eq!(t.g.obj(o).chars.name, "Grizzly Bears"));
}

#[test]
fn r701_40e_ethereal_ambush_manifests_the_top_two_cards_one_at_a_time() {
    let mut t = Board::new(2).hand(0, &["Ethereal Ambush"]).library(0, &["Grizzly Bears", "Shivan Dragon", "Forest"]).build();
    let from = t.mark();
    t.mana(0, "{3}{G}{U}");
    t.cast(0, "Ethereal Ambush", vec![], crate::support::mana_pool("CCCGU")).unwrap();
    t.resolve();
    let down = face_down(&t);
    cite!(["701.40a"]; assert_eq!(down.len(), 2));
    let mut cards: Vec<_> = down.iter().map(|o| t.g.physical_card(*o).unwrap().1.name().to_owned()).collect();
    cards.sort();
    cite!(["701.40a"]; assert_eq!(cards, ["Grizzly Bears", "Shivan Dragon"]));
    t.expect().zone_ordered(0, Zone::Library, &["Forest"]).check();
    // each card entered on its own
    let entries = t.g.log.since(from).iter().filter(|e| matches!(e, game::Event::ZoneChanged { to: Zone::Battlefield, .. })).count();
    cite!(["701.40e"]; assert_eq!(entries, 2));
}

#[test]
fn revealing_wind_its_caster_looks_at_each_face_down_attacker() {
    let mut t = Board::new(2).battlefield(0, &["Grizzly Bears", "Llanowar Elves"]).hand(1, &["Revealing Wind"]).build();
    let (bears, elves) = (t.find_in_battlefield("Grizzly Bears"), t.find_in_battlefield("Llanowar Elves"));
    assert!(t.g.turn_face_down(bears));
    for _ in 0..60 {
        if t.g.decision.is_some() { break }
        t.g.act(t.g.priority, Move::Pass).unwrap();
    }
    t.g.act(0, Move::Attack(vec![(bears, Target::Player(1)), (elves, Target::Player(1))])).unwrap();
    for p in 0..2 { t.g.take_observations(p); }
    while t.g.priority != 1 { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.mana(1, "{2}{G}");
    t.cast(1, "Revealing Wind", vec![], crate::support::mana_pool("CCG")).unwrap();
    while t.g.decision.is_none() && !t.g.stack.is_empty() { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    assert!(matches!(t.g.asked(), Some((1, Asked::May))), "{:?}", t.g.asked());
    t.g.act(1, Move::Choose(Answer::Yes)).unwrap();
    while !t.g.stack.is_empty() && t.g.decision.is_none() { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    let shown: Vec<_> = t.g.observations(1).iter().flat_map(|d| d.cards.iter().map(|c| (d.kind, c.object, c.name.clone()))).collect();
    cite!(["701.20e"]; assert_eq!(shown, vec![(Kind::Looked, bears, "Grizzly Bears".to_owned())], "only the face-down attacker, to its caster alone"));
    assert!(t.g.observations(0).is_empty(), "nothing new for the attacker's controller");
    cite!(["701.20b", "701.20e"]; assert!(t.g.face_down(bears), "it stays face down"));
}
