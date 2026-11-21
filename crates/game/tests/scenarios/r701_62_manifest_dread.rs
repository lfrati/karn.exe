//! Manifest dread (701.62a): look at the top two cards of your library, manifest one of them,
//! then put the cards you looked at that were not manifested into your graveyard. Only the
//! player who manifests dread sees the two cards; the one they keep is a face-down 2/2
//! (701.40a) and the other is put into the graveyard for all to see.
use crate::support;
use game::face::{Concealment, Down, Face, Way};
use game::observation::Kind;
use support::*;

/// The cards of player 0's library, top first, over ten Wastes; player 1's library is ten
/// Wastes. `cards`: the other cards of the game, put nowhere.
fn game(cards: &[&str], library: &[&str]) -> T {
    let mut defs = vec![CardDef::new("Wastes", "", "Basic Land", "", None).unwrap()];
    defs.extend(cards.iter().chain(library).map(|n| oracle(n)));
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, 0, Zone::Library); } }
    for i in (0..library.len()).rev() { g.put(0, 1 + cards.len() + i, Zone::Library); }
    T::new(g)
}
fn face_down(t: &T) -> Vec<ObjId> {
    t.g.battlefield.iter().copied().filter(|o| t.g.face_down(*o)).collect()
}
/// Player `p` casts this card from their hand with this mana and targets, and priority passes
/// until something is asked or the stack is empty.
fn cast(t: &mut T, p: usize, name: &str, mana: &str, targets: Vec<Vec<Target>>) {
    let index = t.g.defs.iter().position(|d| d.name() == name).unwrap();
    let card = t.g.put(p, index, Zone::Hand);
    t.mana(p, mana);
    t.g.act(p, crate::support::paid(mana, Move::Cast { card, targets })).unwrap();
    while t.g.decision.is_none() && !t.g.stack.is_empty() { t.g.act(t.g.priority, Move::Pass).unwrap(); }
}
/// The cards player `p` is asked to choose one of now.
fn asked_one(t: &T, p: usize) -> Vec<ObjId> {
    match t.g.decision.clone() {
        Some(Decision::Choose { player, choice: Choice::Objects { among, count: 1, .. } }) if player == p => among,
        other => panic!("expected player {p} to choose one card: {other:?}"),
    }
}
fn pick(t: &mut T, p: usize, among: &[ObjId], name: &str) {
    let card = *among.iter().find(|o| t.g.obj(**o).chars.name == name).unwrap_or_else(|| panic!("{name} offered"));
    t.g.act(p, Move::Choose(Answer::Objects(vec![card]))).unwrap();
}
fn names(t: &T, cards: &[ObjId]) -> Vec<String> {
    let mut names: Vec<_> = cards.iter().map(|c| t.g.obj(*c).chars.name.clone()).collect();
    names.sort();
    names
}
fn graveyard(t: &T, p: usize) -> Vec<String> {
    names(t, &t.g.players[p].graveyard.iter().copied().filter(|c| t.g.obj(*c).chars.name != "Manifest Dread" && t.g.obj(*c).chars.name != "Unwanted Remake").collect::<Vec<_>>())
}

#[test]
fn r701_62a_one_of_the_top_two_is_manifested_and_the_other_put_into_the_graveyard() {
    let mut t = game(&["Manifest Dread"], &["Grizzly Bears", "Shivan Dragon", "Forest"]);
    for p in 0..2 { t.g.take_observations(p); }
    cast(&mut t, 0, "Manifest Dread", "{1}{G}", vec![]);
    let among = asked_one(&t, 0);
    cite!(["701.62a"]; assert_eq!(names(&t, &among), ["Grizzly Bears", "Shivan Dragon"], "the top two"));
    // the player who manifests dread looks at both; the opponent sees neither
    let looked: Vec<_> = t.g.observations(0).iter().map(|d| (d.kind, d.cards.len())).collect();
    cite!(["701.62a"]; assert_eq!(looked, vec![(Kind::Looked, 2)]));
    cite!(["701.62a"]; assert!(t.g.observations(1).is_empty()));
    for &card in &among { assert!(t.g.sees(0, card) && !t.g.sees(1, card)); }
    pick(&mut t, 0, &among, "Shivan Dragon");
    t.resolve();
    let down = face_down(&t);
    assert_eq!(down.len(), 1);
    let o = down[0];
    cite!(["701.40a"]; assert_eq!(t.g.face(o), Face::Down(Down { by: Concealment::Manifested })));
    cite!(["701.40a"]; assert_eq!((t.g.power(o), t.g.toughness(o), t.g.obj(o).chars.name.as_str()), (Some(2), Some(2), "")));
    assert_eq!(t.g.physical_card(o).map(|(_, d)| d.name().to_owned()), Some("Shivan Dragon".to_owned()));
    cite!(["701.62a"]; assert_eq!(graveyard(&t, 0), ["Grizzly Bears"]));
    assert_eq!(t.g.obj(*t.g.players[0].library.last().unwrap()).chars.name, "Forest", "the third card stays on top");
    // the manifested card is turned face up as any manifested permanent (701.40b)
    t.mana(0, "{4}{R}{R}");
    t.g.act(0, crate::support::paid("CCCCRR", Move::TurnFaceUp(o, Way::Manifested))).unwrap();
    cite!(["701.40b"]; assert_eq!(t.g.obj(o).chars.name, "Shivan Dragon"));
}

#[test]
fn r701_62a_with_one_card_in_the_library_it_is_manifested_and_nothing_is_put_into_the_graveyard() {
    let mut defs = vec![oracle("Manifest Dread"), oracle("Grizzly Bears")];
    defs.push(CardDef::new("Wastes", "", "Basic Land", "", None).unwrap());
    let mut g = Game::new(defs, 2, 20);
    g.put(0, 1, Zone::Library);
    for _ in 0..10 { g.put(1, 2, Zone::Library); }
    let mut t = T::new(g);
    cast(&mut t, 0, "Manifest Dread", "{1}{G}", vec![]);
    if t.g.decision.is_some() {
        let among = asked_one(&t, 0);
        assert_eq!(among.len(), 1);
        pick(&mut t, 0, &among, "Grizzly Bears");
    }
    t.resolve();
    cite!(["701.62a"]; assert_eq!(face_down(&t).len(), 1));
    assert!(t.g.players[0].library.is_empty());
    cite!(["701.62a"]; assert_eq!(graveyard(&t, 0), Vec::<String>::new()));
}

#[test]
fn r701_62a_r701_40f_a_card_that_cannot_be_manifested_goes_to_the_graveyard_too() {
    // Grafdigger's Cage: creature cards in libraries can't enter the battlefield, and the
    // face-down object would be a creature card. Neither card is manifested, so both are
    // "the cards you looked at that were not manifested this way".
    let mut t = game(&["Manifest Dread", "Grafdigger's Cage"], &["Grizzly Bears", "Forest", "Island"]);
    let cage = t.g.defs.iter().position(|d| d.name() == "Grafdigger's Cage").unwrap();
    t.g.put(1, cage, Zone::Battlefield);
    cast(&mut t, 0, "Manifest Dread", "{1}{G}", vec![]);
    let among = asked_one(&t, 0);
    pick(&mut t, 0, &among, "Grizzly Bears");
    let grave_order=vec![among.iter().copied().find(|o|t.g.obj(*o).chars.name=="Grizzly Bears").unwrap(),
        among.iter().copied().find(|o|t.g.obj(*o).chars.name=="Forest").unwrap()];
    cite!(["404.3"]; ok: t.g.act(0,Move::Choose(Answer::Objects(grave_order))));
    t.resolve();
    cite!(["701.40f"]; assert!(face_down(&t).is_empty(), "nothing manifested"));
    cite!(["701.62a", "701.40f"]; assert_eq!(graveyard(&t, 0), ["Forest", "Grizzly Bears"]));
    assert_eq!(t.g.obj(*t.g.players[0].library.last().unwrap()).chars.name, "Island");
}

#[test]
fn unwanted_remake_its_controller_manifests_dread_and_the_caster_sees_nothing() {
    let mut t = game(&["Unwanted Remake", "Grizzly Bears"], &[]);
    let bears = t.g.defs.iter().position(|d| d.name() == "Grizzly Bears").unwrap();
    let victim = t.g.put(1, bears, Zone::Battlefield);
    // player 1's library: Shivan Dragon over Forest over their Wastes
    let (shivan, forest) = (t.g.defs.len(), t.g.defs.len() + 1);
    std::sync::Arc::make_mut(&mut t.g.defs).extend([oracle("Shivan Dragon"), oracle("Forest")]);
    t.g.put(1, forest, Zone::Library);
    t.g.put(1, shivan, Zone::Library);
    for p in 0..2 { t.g.take_observations(p); }
    cast(&mut t, 0, "Unwanted Remake", "{W}", vec![vec![Target::Object(victim)]]);
    let among = asked_one(&t, 1);
    cite!(["608.2c", "701.62a"]; assert_eq!(names(&t, &among), ["Forest", "Shivan Dragon"], "the top two of the destroyed creature's controller's library"));
    cite!(["701.62a"]; assert!(t.g.observations(0).is_empty(), "the caster is shown nothing"));
    pick(&mut t, 1, &among, "Shivan Dragon");
    t.resolve();
    let down = face_down(&t);
    assert_eq!(down.len(), 1);
    cite!(["608.2c"]; assert_eq!(t.g.controller(down[0]), 1, "its controller controls the manifested card"));
    cite!(["701.62a"]; assert_eq!(graveyard(&t, 1), ["Forest", "Grizzly Bears"]));
    cite!(["708.5"]; assert!(t.g.observations(0).iter().all(|d| d.cards.iter().all(|c| c.name != "Shivan Dragon"))));
}

#[test]
fn they_came_from_the_pipes_manifests_dread_twice_one_after_the_other() {
    let mut t = game(&["They Came from the Pipes"], &["Grizzly Bears", "Forest", "Shivan Dragon", "Island", "Plains"]);
    cast(&mut t, 0, "They Came from the Pipes", "{4}{U}", vec![]);
    while t.g.decision.is_none() && !t.g.stack.is_empty() { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    let first = asked_one(&t, 0);
    cite!(["701.62a"]; assert_eq!(names(&t, &first), ["Forest", "Grizzly Bears"]));
    pick(&mut t, 0, &first, "Grizzly Bears");
    let second = asked_one(&t, 0);
    cite!(["608.2c"]; assert_eq!(names(&t, &second), ["Island", "Shivan Dragon"], "the next two, after the first is done"));
    pick(&mut t, 0, &second, "Shivan Dragon");
    // Both manifest instructions finish before the two face-down-entry draws
    // are stacked; identical source/text still leaves their controller a choice.
    let source = t.find_in_battlefield("They Came from the Pipes");
    let Some(Decision::Order { player: 0, abilities }) = t.g.decision.clone() else { panic!("expected two entry triggers: {:?}", t.g.decision) };
    cite!(["603.3b"]; assert_eq!(abilities.len(), 2));
    assert!(abilities.iter().all(|ability| ability.source == Some(source) && ability.controller == 0));
    t.g.act(0, Move::Order(vec![0, 1])).unwrap();
    t.resolve();
    let down = face_down(&t);
    let mut manifested: Vec<_> = down.iter().map(|o| t.g.physical_card(*o).unwrap().1.name().to_owned()).collect();
    manifested.sort();
    cite!(["701.62a"]; assert_eq!(manifested, ["Grizzly Bears", "Shivan Dragon"]));
    cite!(["701.62a"]; assert_eq!(graveyard(&t, 0), ["Forest", "Island"]));
    // "whenever a face-down creature you control enters, draw a card": once for each, after
    // both were manifested
    cite!(["603.2"]; assert_eq!(names(&t, &t.g.players[0].hand), ["Plains", "Wastes"]));
}

#[test]
fn conductive_machete_is_attached_to_the_creature_manifested_dread() {
    let mut t = game(&["Conductive Machete"], &["Grizzly Bears", "Forest"]);
    cast(&mut t, 0, "Conductive Machete", "{4}", vec![]);
    while t.g.decision.is_none() && !t.g.stack.is_empty() { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    let among = asked_one(&t, 0);
    pick(&mut t, 0, &among, "Grizzly Bears");
    t.resolve();
    let down = face_down(&t);
    assert_eq!(down.len(), 1);
    let machete = t.find_in_battlefield("Conductive Machete");
    cite!(["608.2c"]; assert_eq!(t.g.obj(machete).attached_to, Some(Target::Object(down[0])), "attached to that creature"));
    cite!(["701.62a"]; assert_eq!(graveyard(&t, 0), ["Forest"]));
}

#[test]
fn r701_62a_you_may_manifest_dread_does_nothing_when_declined() {
    for accept in [false, true] {
        let spell = CardDef::new("Maybe Dread", "{0}", "Sorcery", "You may manifest dread.", None).unwrap();
        game::play::check(&spell).unwrap();
        let mut t = game(&[], &["Grizzly Bears", "Forest"]);
        std::sync::Arc::make_mut(&mut t.g.defs).push(spell);
        cast(&mut t, 0, "Maybe Dread", "", vec![]);
        assert!(matches!(t.g.asked(), Some((0, Asked::May))), "{:?}", t.g.asked());
        t.g.act(0, Move::Choose(if accept { Answer::Yes } else { Answer::No })).unwrap();
        if accept {
            let among = asked_one(&t, 0);
            pick(&mut t, 0, &among, "Grizzly Bears");
        }
        t.resolve();
        cite!(["701.62a"]; case: format!("accept={accept}"); assert_eq!(face_down(&t).len(), usize::from(accept)));
        assert_eq!(t.g.players[0].library.len(), if accept { 10 } else { 12 });
    }
}

/// Hauntwoods Shrieker: "Reveal target face-down permanent. If it's a creature card, you may
/// turn it face up." What the condition asks of is the card revealed, not the face-down 2/2
/// (708.2): a land card is revealed and not turned up, nor is its controller's opponent asked.
#[test]
fn hauntwoods_shrieker_asks_to_turn_up_only_a_creature_card() {
    for (card, creature) in [("Forest", false), ("Grizzly Bears", true)] {
        let mut t = game(&["Hauntwoods Shrieker", "Forest", "Grizzly Bears"], &[]);
        let shrieker = t.g.put(0, 1, Zone::Battlefield);
        let index = t.g.defs.iter().position(|d| d.name() == card).unwrap();
        let theirs = t.g.put(1, index, Zone::Battlefield);
        assert!(t.g.turn_face_down(theirs));
        t.mana(0, "{1}{G}");
        t.g.act(0, crate::support::paid("CG", Move::Activate { source: shrieker, ability: 1, targets: vec![vec![Target::Object(theirs)]], paying: vec![] })).unwrap();
        while t.g.decision.is_none() && !t.g.stack.is_empty() { t.g.act(t.g.priority, Move::Pass).unwrap(); }
        if creature {
            cite!(["608.2c"]; case: card; assert!(matches!(t.g.decision, Some(Decision::Choose { player: 0, choice: Choice::May })), "{:?}", t.g.decision));
            t.g.act(0, Move::Choose(Answer::Yes)).unwrap();
            cite!(["608.2c"]; case: card; assert_eq!(t.g.obj(theirs).chars.name, "Grizzly Bears"));
        } else {
            cite!(["608.2c"]; case: card; assert!(t.g.decision.is_none(), "{card}: {:?}", t.g.decision));
            assert!(t.g.face_down(theirs));
        }
    }
}
