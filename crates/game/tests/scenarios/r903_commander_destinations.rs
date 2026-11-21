//! Commander destination replacement composes with retained movement, draws and dredge.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use game::state::{Answer, Choice, Decision};

fn setup(effect: &str, leader_text: &str) -> Game {
    let defs = vec![
        CardDef::new("Leader", "{0}", "Legendary Creature — Human", leader_text, Some((2, 2))).unwrap(),
        CardDef::new("Effect", "{0}", "Sorcery", effect, None).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
        CardDef::new("Other", "{0}", "Creature — Human", "", Some((1, 1))).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap(); }
    let mut g = Game::new(defs, 3, 40);
    for p in 0..3 { for _ in 0..12 { g.put(p, 2, Zone::Library); } }
    g
}
fn leader(g: &mut Game, owner: usize, zone: Zone) -> ObjId {
    let card = g.put(owner, 0, Zone::Command);
    g.designate_commander(card).unwrap();
    g.move_object(card, zone);
    *match zone {
        Zone::Battlefield => &g.battlefield, Zone::Library => &g.players[owner].library,
        Zone::Graveyard => &g.players[owner].graveyard, _ => unreachable!(),
    }.last().unwrap()
}
fn resolve(g: &mut Game, targets: Vec<Vec<Target>>) {
    let spell = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets }).unwrap();
    for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); }
}
fn ask(g: &Game, owner: usize, card: ObjId) {
    assert_eq!(g.decision, Some(Decision::Choose { player: owner, choice: Choice::CommandZone { object: card } }));
}
fn answer(g: &mut Game, owner: usize, yes: bool) {
    g.act(owner, Move::Choose(if yes { Answer::Yes } else { Answer::No })).unwrap();
}

#[test]
fn r903_9b_hand_replacement_asks_owner_before_departure_and_resumes_following_instruction_once() {
    let mut g = setup("Return target creature to its owner's hand. You gain 2 life.", "");
    let card = leader(&mut g, 1, Zone::Battlefield);
    resolve(&mut g, vec![vec![Target::Object(card)]]);
    ask(&g, 1, card);
    assert_eq!(g.obj(card).zone, Zone::Battlefield);
    assert_eq!(g.players[0].life, 40);
    cite!(["903.9b"]; refused("that decision is another player's"): g.act(0, Move::Choose(Answer::Yes)));
    cite!(["903.9b"]; refused("a decision is pending: whether the commander goes to the command zone instead (903.9b)"): g.act(1, Move::Pass));
    let mut declined = g.clone();
    answer(&mut g, 1, true);
    answer(&mut declined, 1, false);
    cite!(["903.9b"]; assert!(g.command.iter().any(|c| g.is_commander(*c))));
    assert!(g.players[1].hand.is_empty());
    cite!(["903.9b"]; assert!(declined.players[1].hand.iter().any(|c| declined.is_commander(*c))));
    for state in [&g, &declined] {
        cite!(["608.2c"]; assert_eq!(state.players[0].life, 42));
        assert!(state.stack.is_empty());
        assert!(state.decision.is_none());
    }
}

#[test]
fn r903_9b_simultaneous_returns_collect_apnap_answers_before_committing() {
    let mut g = setup("Return all creatures to their owners' hands.", "");
    let c2 = leader(&mut g, 2, Zone::Battlefield);
    let c0 = leader(&mut g, 0, Zone::Battlefield);
    let c1 = leader(&mut g, 1, Zone::Battlefield);
    resolve(&mut g, vec![]);
    for (p, c, yes) in [(0, c0, true), (1, c1, false), (2, c2, true)] {
        ask(&g, p, c);
        assert_eq!(g.battlefield.len(), 3);
        cite!(["101.4"]; case: p; assert_eq!(g.public_choices().count(), p, "earlier public answers are visible to later choosers"));
        answer(&mut g, p, yes);
    }
    cite!(["101.4"]; assert!(g.battlefield.is_empty()));
    cite!(["903.9b"]; assert_eq!(g.command.len(), 2));
    cite!(["903.9b"]; assert_eq!(g.players[1].hand.len(), 1));
}

#[test]
fn r903_9b_library_destinations_exclude_redirected_cards_from_private_ordering() {
    let mut g = setup("Put all creatures on the bottom of their owners' libraries.", "");
    let c = leader(&mut g, 0, Zone::Battlefield);
    let a = g.put(0, 3, Zone::Battlefield);
    let b = g.put(0, 3, Zone::Battlefield);
    let n = g.players[0].library.len();
    resolve(&mut g, vec![]);
    ask(&g, 0, c);
    answer(&mut g, 0, true);
    let Some(Decision::Choose { player: 0, choice: Choice::OrderCards { to: game::state::Placement::Library(ir::action::LibraryPosition::Bottom), cards: among } }) = g.decision.clone() else { panic!("{:?}", g.decision) };
    assert_eq!(among.len(), 2);
    cite!(["401.4", "903.9b"]; assert_eq!(among, vec![a,b]));
    assert_eq!(g.battlefield.len(), 3, "all choices precede the move");
    g.act(0, Move::Choose(Answer::Objects(vec![b,a]))).unwrap();
    assert_eq!(g.players[0].library.len(), n + 2);
    cite!(["903.9b"]; assert_eq!(g.command.len(), 1));
}

#[test]
fn r903_9b_revealed_library_selection_can_return_to_command_instead_of_hand() {
    let mut g = setup("Reveal the top card of your library. Put that card into your hand. You gain 2 life.", "");
    let c = leader(&mut g, 0, Zone::Library);
    resolve(&mut g, vec![]);
    ask(&g, 0, c);
    assert_eq!(g.obj(c).zone, Zone::Library);
    answer(&mut g, 0, true);
    cite!(["903.9b"]; assert_eq!(g.command.len(), 1));
    assert_eq!(g.players[0].life, 42);
    cite!(["903.9b"]; assert!(g.players[0].hand.is_empty()));
}

#[test]
fn r121_1_replacing_a_draws_move_is_not_a_draw_and_later_draws_are_not_replayed() {
    for yes in [false, true] {
        let mut g = setup("Draw two cards. You gain 2 life.", "");
        let c = leader(&mut g, 0, Zone::Library);
        let n = g.players[0].library.len();
        resolve(&mut g, vec![]);
        ask(&g, 0, c);
        assert_eq!(g.players[0].library.len(), n);
        let mut copied = g.clone();
        for state in [&mut g, &mut copied] {
            answer(state, 0, yes);
            assert_eq!(state.players[0].library.len(), n - 2);
            cite!(["903.9b"]; case: yes; assert_eq!(state.players[0].hand.len(), if yes { 1 } else { 2 }));
            cite!(["614.6", "121.1"]; case: yes; assert_eq!(state.log.iter().filter(|e| matches!(e, Event::Drew { player: 0, .. })).count(), if yes { 1 } else { 2 }));
            assert_eq!(state.players[0].life, 42);
        }
    }
}

#[test]
fn r702_52_dredge_mills_once_before_its_commander_return_can_pause() {
    let mut g = setup("Draw two cards.", "Dredge 2");
    let c = leader(&mut g, 0, Zone::Graveyard);
    assert!(g.state_based_actions());
    answer(&mut g, 0, false); // keep the commander in the graveyard for dredge
    let n = g.players[0].library.len();
    let milled = g.players[0].library.iter().rev().take(2).copied().collect::<Vec<_>>();
    resolve(&mut g, vec![]);
    let Some(Decision::Choose { choice: Choice::Replacement { among, .. }, .. }) = g.decision.clone() else { panic!("{:?}", g.decision) };
    g.act(0, Move::Choose(Answer::Replacement(among[0].clone()))).unwrap();
    assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::OrderCards { to: game::state::Placement::Zone(Zone::Graveyard), cards: milled.clone() } }));
    g.act(0, Move::Choose(Answer::Objects(vec![milled[1], milled[0]]))).unwrap();
    ask(&g, 0, c);
    assert_eq!(g.players[0].library.len(), n - 2);
    let mut copied = g.clone();
    for state in [&mut g, &mut copied] {
        answer(state, 0, true);
        assert_eq!(state.players[0].library.len(), n - 3);
        cite!(["702.52a"]; assert_eq!(state.players[0].graveyard.len(), 3, "two milled cards and the resolved spell"));
        assert_eq!(state.players[0].hand.len(), 1);
        cite!(["903.9b"]; assert_eq!(state.command.len(), 1));
    }
}

#[test]
fn r616_2_death_to_library_makes_commander_destination_replacement_applicable() {
    for yes in [false,true] {
        let mut g=setup("Destroy target creature.","If this creature would die, put it on the bottom of its owner's library instead.");
        let c=leader(&mut g,0,Zone::Battlefield);
        let n=g.players[0].library.len();
        resolve(&mut g,vec![vec![Target::Object(c)]]);
        ask(&g,0,c);
        assert_eq!(g.obj(c).zone,Zone::Battlefield);
        answer(&mut g,0,yes);
        cite!(["616.2", "903.9b"]; case: yes; assert_eq!(g.command.len(),usize::from(yes)));
        cite!(["616.2"]; case: yes; assert_eq!(g.players[0].library.len(),n+usize::from(!yes)));
        cite!(["614.6"]; case: yes; assert!(!g.log.iter().any(|e|matches!(e,Event::ZoneChanged{was,from:Zone::Battlefield,to:Zone::Graveyard,..}if *was==c))));
        assert!(g.decision.is_none());
    }
}
