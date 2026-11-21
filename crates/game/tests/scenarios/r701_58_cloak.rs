//! Cloak (701.58): a card is put onto the battlefield face down as a 2/2 creature with ward {2}
//! (701.58a), turned face up by the special action for the mana cost of the creature card it
//! represents (701.58b), or for its morph or disguise cost when it has one (701.58c/d). A
//! cloaked instant or sorcery is revealed instead and stays face down (701.58g).
use crate::support::cite;
use crate::support;
use game::face::{Concealment, Down, Face, Way};
use game::observation::Kind;
use game::Event;
use support::*;

const TOP: &str = "Cloak the top card of your library.";
const FROM_HAND: &str = "Cloak a card from your hand.";

/// Player 0 holds a sorcery with this text; each player's library is these cards, top first,
/// over ten Wastes.
fn game(text: &str, library: &[&str]) -> T { game_with_hand(text, library, &[]) }
/// [`game`], and player 0's hand holds these cards.
fn game_with_hand(text: &str, library: &[&str], hand: &[&str]) -> T {
    let spell = CardDef::new("Cloaker", "{0}", "Sorcery", text, None).unwrap();
    game::play::check(&spell).unwrap();
    let mut defs = vec![spell, CardDef::new("Wastes", "", "Basic Land", "", None).unwrap()];
    defs.extend(["Lightning Bolt", "Break Open"].iter().map(|n| oracle(n)));
    let first = defs.len();
    defs.extend(library.iter().chain(hand).map(|n| oracle(n)));
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 {
        for _ in 0..10 { g.put(p, 1, Zone::Library); }
        for i in (0..library.len()).rev() { g.put(p, first + i, Zone::Library); }
    }
    for i in 0..hand.len() { g.put(0, first + library.len() + i, Zone::Hand); }
    T::new(g)
}
fn face_down(t: &T) -> Vec<ObjId> {
    t.g.battlefield.iter().copied().filter(|o| t.g.face_down(*o)).collect()
}
/// Player 0 casts the sorcery and it resolves.
fn cast(t: &mut T) {
    let card = t.g.put(0, 0, Zone::Hand);
    t.g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    while t.g.decision.is_none() && !t.g.stack.is_empty() { t.g.act(t.g.priority, Move::Pass).unwrap(); }
}
fn cloak(t: &mut T) -> ObjId {
    cast(t);
    t.resolve();
    let down = face_down(t);
    assert_eq!(down.len(), 1, "one card cloaked");
    down[0]
}

/// Player 1 bolts this permanent with this mana; returns how many objects went on the stack.
fn bolt(t: &mut T, target: ObjId, mana: &str) -> usize {
    while t.g.priority != 1 { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    let bolt = t.g.put(1, 2, Zone::Hand);
    t.mana(1, mana);
    t.g.act(1, crate::support::paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Object(target)]] })).unwrap();
    let on_stack = t.g.stack.len();
    while t.g.decision.is_none() && !t.g.stack.is_empty() { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    on_stack
}

#[test]
fn r701_58a_a_cloaked_card_is_a_nameless_2_2_with_ward_2() {
    let mut t = game(TOP, &["Shivan Dragon"]);
    let o = cloak(&mut t);
    let g = &t.g;
    cite!(["701.58a"]; assert_eq!(g.face(o), Face::Down(Down { by: Concealment::Cloaked })));
    cite!(["701.58a"]; assert_eq!(g.obj(o).chars.name, ""));
    assert_eq!(g.seen(o, None).types, vec![ir::CardType::Creature]);
    assert!(g.seen(o, None).subtypes.is_empty() && g.seen(o, None).colors.is_empty());
    assert_eq!(g.obj(o).chars.mana_value, 0);
    cite!(["701.58a"]; assert_eq!((g.power(o), g.toughness(o)), (Some(2), Some(2))));
    cite!(["701.58a"]; assert_eq!(g.abilities(o).len(), 2, "ward {{2}}: the keyword and its trigger, and no flying or firebreathing"));
    assert!(!g.has(o, "Flying"));
    assert_eq!(g.controller(o), 0);
    let theirs = game::snapshot::Snapshot::of(g, Some(1)).objects.into_iter().find(|s| s.id == o).unwrap();
    assert_eq!(theirs.face_down.map(|d| (d.by, d.card.is_none())), Some((Concealment::Cloaked, true)), "the opponent sees a cloaked 2/2");
    let mine = game::snapshot::Snapshot::of(g, Some(0)).objects.into_iter().find(|s| s.id == o).unwrap();
    cite!(["708.5"]; assert_eq!(mine.face_down.and_then(|d| d.card).map(|c| c.name), Some("Shivan Dragon".to_owned()), "its controller may look at it (708.5)"));
}

#[test]
fn r701_58a_its_ward_counters_an_opponents_spell_unless_they_pay_2() {
    let mut t = game(TOP, &["Shivan Dragon"]);
    let o = cloak(&mut t);
    assert_eq!(bolt(&mut t, o, "{R}"), 2, "the bolt and the ward trigger");
    t.g.act(1, Move::Choose(Answer::No)).unwrap();
    cite!(["701.58a", "702.21a"]; assert!(t.g.stack.is_empty() && t.g.battlefield.contains(&o), "the bolt was countered"));
}

#[test]
fn r701_58b_turned_face_up_for_its_cards_mana_cost_it_loses_ward() {
    let mut t = game(TOP, &["Shivan Dragon"]);
    let o = cloak(&mut t);
    t.mana(0, "{4}{R}");
    let before=format!("{:?}",t.g);
    cite!(["701.58b","118.3a"]; refused("that mana is not in your mana pool"): t.g.act(0,crate::support::paid("CCCCRR", Move::TurnFaceUp(o, Way::Cloaked))));
    assert_eq!(format!("{:?}",t.g),before);
    t.mana(0, "{R}");
    let before=format!("{:?}",t.g);
    cite!(["702.37e"]; refused("that permanent would have no such cost if it were face up (702.37e)"): t.g.act(0, Move::TurnFaceUp(o, Way::Morph)));
    assert_eq!(format!("{:?}",t.g),before);
    cite!(["701.40b"]; refused("only a manifested permanent is turned face up for its card's mana cost (701.40b)"): t.g.act(0, Move::TurnFaceUp(o, Way::Manifested)));
    let from = t.mark();
    t.g.act(0, crate::support::paid("CCCCRR", Move::TurnFaceUp(o, Way::Cloaked))).unwrap();
    cite!(["701.58b", "116.2b"]; assert!(t.g.stack.is_empty(), "a special action"));
    assert_eq!(t.g.players[0].pool.total(), 0);
    cite!(["701.58b"]; assert_eq!(t.g.obj(o).chars.name, "Shivan Dragon"));
    assert!(t.g.has(o, "Flying"));
    assert_eq!(t.g.log.since(from).iter().filter(|e| matches!(e, Event::TurnedFaceUp { .. })).count(), 1);
    cite!(["701.58b"]; assert_eq!(bolt(&mut t, o, "{R}"), 1, "no ward trigger now"));
}

#[test]
fn r701_58b_a_cloaked_land_has_no_way_up_and_a_morph_card_has_two() {
    let mut t = game(TOP, &["Forest"]);
    let o = cloak(&mut t);
    t.mana(0, "{5}{G}{G}");
    let before=format!("{:?}",t.g);
    cite!(["701.58b"]; refused("only a creature card is turned face up for its mana cost (701.40b, 701.58b)"): t.g.act(0, Move::TurnFaceUp(o, Way::Cloaked)));
    assert_eq!(format!("{:?}",t.g),before);
    // 701.58c: Glacial Stalker ({5}{U}, morph {4}{U}) either way
    let mut t = game(TOP, &["Glacial Stalker"]);
    let o = cloak(&mut t);
    t.mana(0, "{5}{U}");
    cite!(["701.58c"]; ok: t.g.clone().act(0, crate::support::paid("CCCCCU", Move::TurnFaceUp(o, Way::Cloaked))));
    t.g.act(0, crate::support::paid("CCCCU", Move::TurnFaceUp(o, Way::Morph))).unwrap();
    assert_eq!(t.g.obj(o).chars.name, "Glacial Stalker");
    cite!(["701.58c", "702.37e"]; assert_eq!(t.g.players[0].pool.total(), 1, "paid {{4}}{{U}}"));
}

#[test]
fn r701_58g_a_cloaked_instant_turned_up_by_an_effect_is_revealed_and_stays_face_down() {
    let mut t = game(TOP, &["Lightning Bolt"]);
    let o = cloak(&mut t);
    let before=format!("{:?}",t.g);
    cite!(["701.58b"]; refused("only a creature card is turned face up for its mana cost (701.40b, 701.58b)"): t.g.act(0, Move::TurnFaceUp(o, Way::Cloaked)));
    assert_eq!(format!("{:?}",t.g),before);
    for p in 0..2 { t.g.take_observations(p); }
    while t.g.priority != 1 { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    let open = t.g.put(1, 3, Zone::Hand);
    t.mana(1, "{3}{R}");
    let from = t.mark();
    t.g.act(1, crate::support::paid("CR", Move::Cast { card: open, targets: vec![vec![Target::Object(o)]] })).unwrap();
    // its ward triggers: Break Open's caster pays {2}
    while t.g.decision.is_none() && t.g.stack.len() > 1 { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    assert!(matches!(t.g.asked(), Some((1, Asked::Pay { .. }))), "{:?}", t.g.asked());
    t.g.act(1, Move::Choose(Answer::Pay { plan: vec![], with: Some(crate::support::mana_pool("CC")), tapped: vec![] })).unwrap();
    t.resolve();
    cite!(["701.58g"]; assert_eq!(t.g.face(o), Face::Down(Down { by: Concealment::Cloaked }), "left face down"));
    cite!(["701.58g"]; assert_eq!(t.g.log.since(from).iter().filter(|e| matches!(e, Event::TurnedFaceUp { .. })).count(), 0));
    let shown: Vec<_> = t.g.observations(1).iter().flat_map(|d| d.cards.iter().map(|c| (d.kind, d.by, c.name.clone()))).collect();
    cite!(["701.58g"]; assert_eq!(shown, vec![(Kind::Revealed, 0, "Lightning Bolt".to_owned())], "revealed by its controller"));
}

#[test]
fn r701_58a_cloak_a_card_from_your_hand_the_opponent_learns_nothing() {
    let mut t = game_with_hand(FROM_HAND, &[], &["Grizzly Bears", "Forest"]);
    let (bears, forest) = (t.find_in(0, Zone::Hand, "Grizzly Bears"), t.find_in(0, Zone::Hand, "Forest"));
    for p in 0..2 { t.g.take_observations(p); }
    cast(&mut t);
    let Some(Decision::Choose { player: 0, choice: Choice::Objects { among, count: 1, .. } }) = t.g.decision.clone() else { panic!("{:?}", t.g.decision) };
    cite!(["608.2d"]; assert_eq!(among.len(), 2, "either card in the hand"));
    t.g.act(0, Move::Choose(Answer::Objects(vec![bears]))).unwrap();
    t.resolve();
    let down = face_down(&t);
    assert_eq!(down.len(), 1);
    cite!(["701.58a"]; assert_eq!(t.g.face(down[0]), Face::Down(Down { by: Concealment::Cloaked })));
    assert_eq!(t.g.physical_card(down[0]).map(|(_, d)| d.name().to_owned()), Some("Grizzly Bears".to_owned()));
    assert_eq!(t.g.players[0].hand, vec![forest]);
    cite!(["708.5"]; assert!(t.g.observations(1).is_empty(), "the opponent is shown nothing"));
    let theirs = game::snapshot::Snapshot::of(&t.g, Some(1)).objects.into_iter().find(|s| s.id == down[0]).unwrap();
    cite!(["708.5"]; assert!(theirs.face_down.unwrap().card.is_none()));
}

#[test]
fn r701_58a_cloak_a_card_from_an_empty_hand_does_nothing() {
    let mut t = game(FROM_HAND, &[]);
    cast(&mut t);
    t.resolve();
    cite!(["609.3"]; assert!(face_down(&t).is_empty()));
}

#[test]
fn r701_58e_hide_in_plain_sight_cloaks_two_of_five_one_at_a_time() {
    let library = ["Grizzly Bears", "Shivan Dragon", "Forest", "Island", "Mountain", "Plains"];
    let mut g = Game::new(std::iter::once("Hide in Plain Sight").chain(library).map(oracle).collect::<Vec<_>>(), 2, 20);
    for i in (1..=library.len()).rev() { g.put(0, i, Zone::Library); }
    let spell = g.put(0, 0, Zone::Hand);
    let mut t = T::new(g);
    t.mana(0, "{3}{G}");
    t.g.act(0, crate::support::paid("CCCG", Move::Cast { card: spell, targets: vec![] })).unwrap();
    while t.g.decision.is_none() && !t.g.stack.is_empty() { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    let mut chosen = Vec::new();
    for (pick, left) in [("Shivan Dragon", 5), ("Grizzly Bears", 4)] {
        let Some(Decision::Choose { player: 0, choice: Choice::Objects { among, count: 1, .. } }) = t.g.decision.clone() else { panic!("{:?}", t.g.decision) };
        cite!(["701.58e"]; assert_eq!(among.len(), left, "one at a time, from those still in the library"));
        let card = *among.iter().find(|o| t.g.obj(**o).chars.name == pick).unwrap();
        assert!(!t.g.sees(1, card));
        t.g.act(0, Move::Choose(Answer::Objects(vec![card]))).unwrap();
        chosen.push(card);
    }
    t.resolve();
    let down = face_down(&t);
    assert_eq!(down.len(), 2);
    cite!(["701.58a"]; assert!(down.iter().all(|o| t.g.face(*o) == Face::Down(Down { by: Concealment::Cloaked }))));
    let names: Vec<_> = t.g.players[0].library.iter().rev().map(|c| t.g.obj(*c).chars.name.clone()).collect();
    assert_eq!(names[0], "Plains", "the sixth card is now on top");
    let mut rest = names[1..].to_vec();
    rest.sort();
    cite!(["608.2c"]; assert_eq!(rest, ["Forest", "Island", "Mountain"], "the rest on the bottom"));
}

#[test]
fn r701_58a_you_may_cloak_asks_and_does_nothing_when_declined() {
    for accept in [false, true] {
        let mut t = game("You may cloak the top card of your library.", &["Grizzly Bears"]);
        cast(&mut t);
        assert!(matches!(t.g.asked(), Some((0, Asked::May))), "{:?}", t.g.asked());
        t.g.act(0, Move::Choose(if accept { Answer::Yes } else { Answer::No })).unwrap();
        t.resolve();
        cite!(["701.58a"]; assert_eq!(face_down(&t).len(), usize::from(accept)));
        assert_eq!(t.g.players[0].library.len(), if accept { 10 } else { 11 });
    }
}

#[test]
fn veiled_ascension_a_cloaked_creature_enters_with_a_flying_counter() {
    let mut t = game(TOP, &["Grizzly Bears"]);
    std::sync::Arc::make_mut(&mut t.g.defs).push(oracle("Veiled Ascension"));
    let ascension = t.g.defs.len() - 1;
    t.g.put(0, ascension, Zone::Battlefield);
    let o = cloak(&mut t);
    cite!(["614.1c"]; assert_eq!(t.g.counters(Target::Object(o)).get("flying").copied(), Some(1)));
    cite!(["122.1b"]; assert!(t.g.has(o, "Flying")));
}

#[test]
fn r707_2_a_clone_of_a_cloaked_permanent_is_a_face_up_nameless_2_2_with_ward_2() {
    let mut t = game_with_hand(TOP, &["Shivan Dragon"], &["Clone"]);
    let o = cloak(&mut t);
    let clone = t.find_in(0, Zone::Hand, "Clone");
    t.mana(0, "{3}{U}");
    t.g.act(0, crate::support::paid("CCCU", Move::Cast { card: clone, targets: vec![] })).unwrap();
    let boundary = t.settle();
    let Boundary::Question(0, _) = boundary else { panic!("Clone asks what to copy: {boundary:?}") };
    t.answer(0, Answer::Objects(vec![o])).unwrap();
    t.resolve();
    let copy = *t.g.battlefield.iter().find(|c| **c != o && t.g.is_a(**c, ir::CardType::Creature)).unwrap();
    cite!(["707.2"]; assert!(!t.g.face_down(copy), "face-down status is not copied"));
    cite!(["707.2", "708.2a"]; assert_eq!((t.g.obj(copy).chars.name.as_str(), t.g.power(copy), t.g.toughness(copy)), ("", Some(2), Some(2))));
    cite!(["701.58a", "707.2"]; assert_eq!(t.g.abilities(copy).len(), 2, "ward {{2}} is one of the copiable values cloaking gives (701.58a, 707.2)"));
    assert_eq!(bolt(&mut t, copy, "{R}"), 2, "the bolt and the copy's ward trigger");
    t.g.act(1, Move::Choose(Answer::No)).unwrap();
    t.mana(0, "{4}{U}{U}");
    assert_eq!(t.g.priority,0);
    let before=format!("{:?}",t.g);
    cite!(["707.2","116.2b"]; refused("that permanent is face up (116.2b)"): t.g.act(0, Move::TurnFaceUp(copy, Way::Cloaked)));
    assert_eq!(format!("{:?}",t.g),before);
}

#[test]
fn r701_58f_a_chosen_card_that_cannot_be_cloaked_is_not_chosen_again_and_goes_with_the_rest() {
    // the face-down object would be a creature card entering from a library: Grafdigger's Cage
    // makes cloaking any of them impossible (Cage ruling c58ea9abb49eb78d)
    let library = ["Grizzly Bears", "Shivan Dragon", "Forest", "Island", "Mountain", "Plains"];
    let defs = ["Hide in Plain Sight", "Grafdigger's Cage"].into_iter().chain(library).map(oracle).collect::<Vec<_>>();
    let mut g = Game::new(defs, 2, 20);
    for i in (2..2 + library.len()).rev() { g.put(0, i, Zone::Library); }
    g.put(1, 1, Zone::Battlefield);
    let spell = g.put(0, 0, Zone::Hand);
    let mut t = T::new(g);
    t.mana(0, "{3}{G}");
    t.g.act(0, crate::support::paid("CCCG", Move::Cast { card: spell, targets: vec![] })).unwrap();
    while t.g.decision.is_none() && !t.g.stack.is_empty() { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    let mut chosen = Vec::new();
    for pick in ["Grizzly Bears", "Forest"] {
        let Some(Decision::Choose { player: 0, choice: Choice::Objects { among, count: 1, .. } }) = t.g.decision.clone() else { panic!("{:?}", t.g.decision) };
        cite!(["608.2d"]; assert!(chosen.iter().all(|c| !among.contains(c)), "608.2d: two distinct cards of the five"));
        let card = *among.iter().find(|o| t.g.obj(**o).chars.name == pick).unwrap();
        t.g.act(0, Move::Choose(Answer::Objects(vec![card]))).unwrap();
        chosen.push(card);
    }
    t.resolve();
    let down = face_down(&t);
    cite!(["701.58f"]; assert!(down.is_empty(), "Grafdigger's Cage keeps both cards in the library"));
    let names: Vec<_> = t.g.players[0].library.iter().rev().map(|c| t.g.obj(*c).chars.name.clone()).collect();
    assert_eq!(names[0], "Plains", "the sixth card is now on top");
    let mut rest = names[1..].to_vec();
    rest.sort();
    cite!(["701.58f"]; assert_eq!(rest, ["Forest", "Grizzly Bears", "Island", "Mountain", "Shivan Dragon"], "701.58f: uncloaked cards stay among the rest, on the bottom"));
}
