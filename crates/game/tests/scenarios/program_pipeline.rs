//! Compilation, admission and ordinary game decisions share one resolution path.
use crate::support::cite;
use game::{Game, Move, Zone, Event, ObjId};
use game::card::CardDef;
use game::play::check;
use game::state::{Answer, Choice, Decision};

const SALVAGE: &str = "Reveal the top five cards of your library. You may put a creature or land card from among them into your hand. Put the rest into your graveyard.";
const PICK: &str = "Reveal the top four cards of your library. Put a creature card from among them into your hand and the rest on the bottom of your library.";
fn def(name: &str, kind: &str, text: &str) -> CardDef {
    CardDef::new(name, "{0}", kind, text, kind.contains("Creature").then_some((1,1))).unwrap()
}
fn setup(text: &str) -> Game {
    let defs = vec![def("Probe", "Instant", text), def("Bear", "Creature — Bear", ""), def("Elf", "Creature — Elf", ""), def("Stone", "Artifact", "")];
    for d in &defs { check(d).unwrap(); }
    let mut g = Game::new(defs, 3, 20);
    for id in [3, 1, 2, 3] { g.put(0, id, Zone::Library); }
    g
}
fn cast(g: &mut Game, id: usize) {
    let card = g.put(0, id, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    for player in 0..3 { g.act(player, Move::Pass).unwrap(); }
}
fn among(g: &Game) -> Vec<ObjId> {
    match g.decision.as_ref().unwrap() {
        Decision::Choose { player: 0, choice: Choice::Objects { among, .. } } => among.clone(),
        d => panic!("unexpected {d:?}"),
    }
}
fn state(g: &Game) -> String { format!("{g:?}") }
fn reveals(g: &Game) -> usize { g.log.iter().filter(|e| matches!(e, Event::Revealed { .. })).count() }

#[test]
fn r608_2d_compiled_resolution_rejects_wrong_answers_and_resumes_clones_without_repeating_actions() {
    let mut g = setup(SALVAGE);
    let original = g.players[0].library.clone();
    cast(&mut g, 0);
    let frame = g.resolving().unwrap().program_frame().expect("real cast uses compiled program");
    let (program, pc) = (frame.program_id(), frame.pc());
    let before = state(&g);
    crate::support::refused(g.act(1, Move::Choose(Answer::Yes)), "that decision is another player's");
    cite!(["608.2d"]; refused("the choice is whether to do it: yes or no (608.2d)"): g.act(0, Move::Choose(Answer::Objects(vec![]))));
    assert_eq!(state(&g), before);
    assert_eq!(reveals(&g), 1);
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    let frame = g.resolving().unwrap().program_frame().unwrap();
    assert_eq!(frame.program_id(), program);
    assert!(frame.pc() > pc);
    let cards = among(&g);
    let before = state(&g);
    // the previous question's answer is stale
    cite!(["608.2d"]; refused("choose exactly 1 of the offered objects to put (608.2d)"): g.act(0, Move::Choose(Answer::Yes)));
    crate::support::refused(g.act(0, Move::Choose(Answer::Objects(vec![ObjId(u64::MAX)]))), "an object handle is stale or unknown");
    assert_eq!(state(&g), before);
    let mut clone = g.clone();
    for game in [&mut g, &mut clone] {
        game.act(0, Move::Choose(Answer::Objects(vec![cards[0]]))).unwrap();
        let remainder = original.iter().copied().filter(|o| *o != cards[0]).collect();
        cite!(["404.3"]; ok: game.act(0, Move::Choose(Answer::Objects(remainder))));
    }
    assert_eq!(state(&g), state(&clone));
    assert_eq!(reveals(&g), 1);
    cite!(["608.2d"]; assert_eq!(g.players[0].hand.len(), 1));
    assert_eq!(g.players[0].graveyard.len(), 4); // three unselected cards + the spell
    let before = state(&g);
    crate::support::refused(g.act(0, Move::Choose(Answer::Objects(vec![cards[0]]))), "no decision is pending");
    assert_eq!(state(&g), before);
}

#[test]
fn r401_4_second_composition_uses_the_same_frame_and_shared_library_ordering() {
    let mut g = Game::new(vec![def("Probe", "Instant", PICK), def("Bear", "Creature — Bear", ""),
        def("Elf", "Creature — Elf", ""), def("Stone", "Artifact", ""), def("Rock", "Artifact", "")], 3, 20);
    for id in [3, 1, 2, 4] { g.put(0, id, Zone::Library); }
    let old = g.players[0].library.clone();
    cast(&mut g, 0);
    let id = g.resolving().unwrap().program_frame().unwrap().program_id();
    let picked = among(&g)[0];
    g.act(0, Move::Choose(Answer::Objects(vec![picked]))).unwrap();
    assert_eq!(g.resolving().unwrap().program_frame().unwrap().program_id(), id);
    assert_eq!(picked, old[2]);
    let order = vec![old[0], old[1], old[3]];
    let expected = vec!["Stone", "Bear", "Rock"];
    let before = state(&g);
    let mut clone = g.clone();
    crate::support::refused(g.act(2, Move::Choose(Answer::Objects(order.clone()))), "that decision is another player's");
    assert_eq!(state(&g), before);
    for game in [&mut g, &mut clone] { game.act(0, Move::Choose(Answer::Objects(order.clone()))).unwrap(); }
    assert_eq!(state(&g), state(&clone));
    let actual: Vec<_> = g.players[0].library.iter().rev().map(|c| g.obj(*c).chars.name.clone()).collect();
    cite!(["401.4"]; assert_eq!(actual, expected));
    cite!(["701.20d"]; assert!(old.iter().all(|o| !g.players[0].library.contains(o)), "revealed reordered objects have new identities"));
    assert_eq!(reveals(&g), 1);
}

#[test]
fn r400_2_private_inspection_is_admitted_now_that_what_was_seen_is_delivered_without_a_later_choice() {
    // delivery itself: tests/r701_observations.rs
    for text in ["Look at the top card of your library.", "Look at the top four cards of your library. Put the rest on the bottom of your library in a random order."] {
        check(&def("Inspection","Instant",text)).unwrap_or_else(|e| panic!("{text}: {e:?}"));
    }
    // A targeted library is also observed by the caster, with no following decision.
    let inspection=def("Inspection","Instant","Look at the top card of target player's library.");
    check(&inspection).unwrap();
    let land=CardDef::new("Forest","","Basic Land — Forest","",None).unwrap();
    let mut g=Game::new(vec![inspection,land],3,20);
    let seen=g.put(1,1,Zone::Library);let spell=g.put(0,0,Zone::Hand);
    g.act(0,Move::Cast{card:spell,targets:vec![vec![game::Target::Player(1)]]}).unwrap();
    for _ in 0..3 {g.act(g.priority,Move::Pass).unwrap();}
    assert!(g.decision.is_none());assert_eq!(g.players[1].library,vec![seen]);
    assert_eq!(g.observations(0)[0].cards[0].object,seen);
    assert!(g.observations(1).is_empty());assert!(g.observations(2).is_empty());
}

#[test]
fn r113_7a_trigger_frame_survives_its_source_and_retains_its_controller() {
    let mut g = setup(SALVAGE);
    let satyr = CardDef::new("Satyr Wayfinder", "{0}", "Creature — Satyr", "When this creature enters, reveal the top four cards of your library. You may put a land card from among them into your hand. Put the rest into your graveyard.", Some((1,1))).unwrap();
    check(&satyr).unwrap();
    let land = CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap();
    let reanimate = def("Return witness", "Sorcery", "Return target creature card from a graveyard to the battlefield under your control.");
    check(&reanimate).unwrap();
    std::sync::Arc::make_mut(&mut g.defs).extend([satyr,land,reanimate]);
    g.put(0,5,Zone::Library); g.put(0,5,Zone::Library);
    let target = g.put(1,4,Zone::Graveyard);
    let spell = g.put(0,6,Zone::Hand);
    g.act(0,Move::Cast { card:spell,targets:vec![vec![game::Target::Object(target)]] }).unwrap();
    for player in 0..3 { g.act(player,Move::Pass).unwrap(); }
    let source = *g.battlefield.last().unwrap();
    assert_eq!((g.obj(source).owner, g.controller(source)), (1, 0));
    g.move_object(source, Zone::Graveyard);
    for player in 0..3 { g.act(player,Move::Pass).unwrap(); }
    cite!(["113.7a"]; assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May })));
    assert!(g.resolving().unwrap().program_frame().is_some());
    g.act(0,Move::Choose(Answer::Yes)).unwrap();
    let cards=among(&g);
    g.act(0,Move::Choose(Answer::Objects(vec![cards[0]]))).unwrap();
    assert_eq!(g.players[0].hand.len(),1);
    assert_eq!(g.obj(g.players[0].hand[0]).chars.name,"Forest");
    assert_eq!(reveals(&g),1);
}

#[test]
fn r608_2c_compilation_checks_bindings_and_operands_even_when_runtime_groups_would_be_empty() {
    for text in [
        "Put the rest into your graveyard.",
        "Reveal the top four cards of your library. Reveal the top five cards of your library. Put the rest into your graveyard.",
        "Reveal the top four cards of your library. Put all cards of the chosen color revealed this way into your hand and the rest into your graveyard.",
    ] {
        let d=def("Invalid", "Instant",text);
        let reason=if text.starts_with("Put") {"no earlier instruction produces what it names"}
            else if text.contains("chosen color") {"linked choice: no Color producer (607.2d)"}
            else {"more than one earlier group could be meant"};
        crate::support::refused(check(&d).map_err(|e|e.0),reason);
    }
}

#[test]
fn r608_2c_mixed_abilities_are_checked_and_run_by_their_own_authority() {
    let text = format!("{SALVAGE}\nYou gain 3 life.");
    let mut g=setup(&text);
    let remainder = g.players[0].library.clone();
    cast(&mut g,0);
    assert!(g.resolving().unwrap().program_frame().is_some());
    g.act(0,Move::Choose(Answer::No)).unwrap();
    cite!(["404.3"]; ok: g.act(0, Move::Choose(Answer::Objects(remainder))));
    cite!(["608.2c"]; assert_eq!(g.players[0].life,23));
    assert_eq!(reveals(&g),1);
    let bad=def("Mixed bad","Instant",&format!("{SALVAGE}\nYou get an emblem with \"{{0}}: You gain 1 life.\""));
    crate::support::refused(check(&bad).map_err(|e|e.0),"emblem: only a single triggered or supported static ability is implemented (114.4)");
}

#[test]
fn r608_2c_mixed_program_creates_a_live_passive_static_emblem() {
    let text = format!("{SALVAGE}\nYou get an emblem with \"Creatures you control have flying.\"");
    let mut g = setup(&text);
    let remainder = g.players[0].library.clone();
    let own = g.put(0, 1, Zone::Battlefield);
    let opponent = g.put(1, 1, Zone::Battlefield);
    cast(&mut g, 0);
    assert!(!g.has(own, "Flying"));
    g.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["404.3"]; ok: g.act(0, Move::Choose(Answer::Objects(remainder))));
    assert_eq!(reveals(&g), 1);
    cite!(["114.2"]; assert_eq!(g.command.len(), 1));
    cite!(["114.4"]; assert!(g.has(own, "Flying")));
    assert!(!g.has(opponent, "Flying"));
    let later = g.put(0, 2, Zone::Battlefield);
    assert!(g.has(later, "Flying"));
}

#[test]
fn r608_2c_games_share_precompiled_definitions_without_recompiling_at_cast_or_resume() {
    let mut first = setup(SALVAGE);
    let defs = first.defs.as_ref().clone(); // CardDef cloning shares the compiled program
    cast(&mut first, 0);
    let id = first.resolving().unwrap().program_frame().unwrap().program_id();
    let mut second = Game::new(defs, 3, 20);
    let remainder: Vec<_> = [1,2,3].into_iter().map(|card| second.put(0,card,Zone::Library)).collect();
    cast(&mut second,0);
    assert_eq!(second.resolving().unwrap().program_frame().unwrap().program_id(), id);
    // An unanswered game cannot affect another game's continuation or bindings.
    let waiting = state(&first);
    second.act(0,Move::Choose(Answer::No)).unwrap();
    cite!(["404.3"]; ok: second.act(0, Move::Choose(Answer::Objects(remainder))));
    assert_eq!(state(&first),waiting);
    assert!(second.resolving().is_none());
}

const EXILE_PICK: &str = "Exile the top four cards of your library. Put a creature card exiled this way into your hand and the rest into your graveyard.";

#[test]
fn r400_7j_r608_2d_exile_results_hold_new_identities_and_resume_without_repeating_moves() {
    let mut g = setup(EXILE_PICK);
    let old = g.players[0].library.clone();
    cast(&mut g, 0);
    assert!(g.resolving().unwrap().program_frame().is_some());
    let offered = among(&g);
    cite!(["400.7j"]; assert_eq!(offered.len(), 2));
    assert_eq!(g.exile.len(), 4);
    assert!(old.iter().all(|o| g.obj(*o).zone == Zone::Gone));
    cite!(["400.7"]; assert!(offered.iter().all(|o| g.obj(*o).zone == Zone::Exile && !old.contains(o))));
    let before = state(&g);
    crate::support::refused(g.act(1, Move::Choose(Answer::Objects(vec![offered[0]]))), "that decision is another player's");
    cite!(["400.7", "608.2d"]; refused("a chosen object is not among the offered objects to return (608.2d)"): g.act(0, Move::Choose(Answer::Objects(vec![old[0]]))));
    assert_eq!(state(&g), before);
    let picked_name = g.obj(offered[0]).chars.name.clone();
    let remainder: Vec<_> = g.exile.iter().copied().filter(|o| *o != offered[0]).collect();
    let mut clone = g.clone();
    for game in [&mut g, &mut clone] {
        game.act(0, Move::Choose(Answer::Objects(vec![offered[0]]))).unwrap();
        cite!(["404.3"]; ok: game.act(0, Move::Choose(Answer::Objects(remainder.clone()))));
        assert!(game.resolving().is_none());
        assert!(game.exile.is_empty());
        assert_eq!(game.players[0].hand.len(), 1);
        assert_eq!(game.obj(game.players[0].hand[0]).chars.name, picked_name);
        assert_ne!(game.players[0].hand[0], offered[0], "a second zone change creates another identity");
        assert_eq!(game.players[0].graveyard.len(), 4); // remainder and resolved spell
        assert_eq!(game.log.iter().filter(|e| matches!(e, Event::Exiled { .. })).count(), 4);
        assert_eq!(reveals(game), 0, "exile is a public move, not a reveal event");
    }
    assert_eq!(state(&g), state(&clone));
}

#[test]
fn r400_7j_r609_3_empty_and_partial_exile_results_are_values() {
    let defs = setup(EXILE_PICK).defs.as_ref().clone();
    for library in [vec![], vec![3], vec![1], vec![3, 1]] {
        let mut g = Game::new(defs.clone(), 3, 20);
        for d in &library { g.put(0, *d, Zone::Library); }
        cast(&mut g, 0);
        assert!(g.resolving().is_none());
        assert!(g.exile.is_empty());
        let picked = usize::from(library.contains(&1));
        cite!(["609.3"]; case: format!("{library:?}"); assert_eq!(g.players[0].hand.len(), picked));
        assert_eq!(g.players[0].graveyard.len(), library.len() - picked + 1);
        assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Exiled { .. })).count(), library.len());
    }
}

#[test]
fn r400_7j_r608_2c_action_qualifiers_bind_distinct_public_move_and_reveal_results() {
    let text = "Exile the top two cards of your library. Reveal the top two cards of your library. Put all creature cards exiled this way into your hand. Put all creature cards revealed this way into your hand.";
    let mut g = setup(text);
    cast(&mut g, 0);
    assert!(g.resolving().is_none());
    let mut names: Vec<_> = g.players[0].hand.iter().map(|o| g.obj(*o).chars.name.as_str()).collect();
    names.sort();
    cite!(["400.7j"]; assert_eq!(names, vec!["Bear", "Elf"]));
    assert_eq!(g.exile.len(), 1);
    assert_eq!(g.players[0].library.len(), 1);
    assert_eq!(reveals(&g), 1);
}

#[test]
fn r608_2c_exile_references_require_one_matching_producer() {
    for (text, why) in [
        ("Reveal the top four cards of your library. Put all creature cards exiled this way into your hand.", "no earlier instruction produces"),
        ("Exile the top two cards of your library. Reveal the top two cards of your library. Put the rest into your graveyard.", "more than one"),
    ] {
        let d = def("Ambiguous", "Instant", text);
        let error = check(&d).unwrap_err();
        assert!(error.0.contains(why), "{text}: {error:?}");
    }
    // "exiled this way" after two exiles names both: one group of their results (608.2c)
    let both = def("Both", "Instant", "Exile the top two cards of your library. Exile the top two cards of your library. Put all creature cards exiled this way into your hand.");
    check(&both).unwrap();
}
