//! Compiled ability programs: real card
//! text compiled once, references bound to producers at compile time, and frames run directly.
//! These tests use hand-built frames over compiled programs: intermediate evidence for the
//! mechanism. The routed witnesses (a card cast and resolved through `Game::act`) come with the
//! routing step.

use crate::support::cite;
use game::card::CardDef;
use game::program::{compile, run, Audience, Frame, Instr, Program, Value, Who};
use game::select::Ctx;
use game::state::{Answer, Choice, Decision};
use game::{Game, ObjId, Zone};
use ir::Ability;
use std::sync::Arc;

const FOREST: usize = 0;
const BEARS: usize = 1;
const SHOCK: usize = 2;

fn card(name: &str, text: &str) -> CardDef {
    CardDef::new(name, "", "Sorcery", text, None).expect(name)
}

fn defs() -> Vec<CardDef> {
    let c = |n, c, t, x, pt| CardDef::new(n, c, t, x, pt).expect(n);
    vec![
        c("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        c("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        c("Shock", "{R}", "Instant", "Shock deals 2 damage to any target.", None),
    ]
}

/// The effect of a card's first ability, a spell's text or a trigger's instructions.
fn effect_of(def: &CardDef) -> &ir::Effect {
    match &def.abilities()[0] {
        Ability::Text(e) | Ability::Triggered { effect: e, .. } => e,
        other => panic!("not text or trigger: {other:?}"),
    }
}

fn program(text: &str) -> Result<Program, String> {
    let def = card("T", text);
    let mut game = Game::new(vec![def.clone()], 2, 20);
    let ctx = Ctx::new(0, game.put(0, 0, Zone::Stack));
    compile(effect_of(&def), &game, &ctx).map_err(|u| u.0)
}

fn names(g: &Game, cards: &[ObjId]) -> Vec<String> {
    let mut n: Vec<String> = cards.iter().map(|c| g.obj(*c).chars.name.clone()).collect();
    n.sort();
    n
}

/// Player 0's library is these cards, the last of them on top; a Forest is on the battlefield
/// as the source.
fn setup(library: &[usize]) -> (Game, Ctx<'static>) {
    let mut g = Game::new(defs(), 2, 20);
    for d in library {
        g.put(0, *d, Zone::Library);
    }
    let source = g.put(0, FOREST, Zone::Battlefield);
    (g, Ctx::new(0, source))
}

fn frame_of(text: &str, ctx: Ctx<'static>) -> Frame {
    Frame::new(Arc::new(program(text).unwrap()), ctx)
}

const MULCH: &str = "Reveal the top four cards of your library. Put all land cards revealed this way into your hand and the rest into your graveyard.";
const SALVAGE: &str = "Reveal the top five cards of your library. You may put a creature or land card from among them into your hand. Put the rest into your graveyard.";
const SATYR: &str = "When this creature enters, reveal the top four cards of your library. You may put a land card from among them into your hand. Put the rest into your graveyard.";
const BOTTOM: &str = "Reveal the top four cards of your library. Put a creature card from among them into your hand and the rest on the bottom of your library in a random order.";

#[test]
fn compiled_once_from_real_text_the_witnesses_and_the_reveal_to_bottom_example() {
    for text in [MULCH, SALVAGE, SATYR, BOTTOM] {
        let p = program(text).unwrap_or_else(|e| panic!("not compiled: {text}: {e}"));
        assert!(p.instrs().iter().any(|i| matches!(i, Instr::TakeTop { .. })));
        assert!(p.instrs().iter().any(|i| matches!(i, Instr::Remainder { .. })), "{p}");
    }
}

#[test]
fn unsupported_instructions_are_compilation_errors_with_no_fallback() {
    for (text, why) in [
        ("Put that card into your hand.", "input reference: nothing before it supplies the required participant/cardinality"),
        ("Search your graveyard for a card.", "search: only the controller searching their own library is implemented"),
    ] {
        crate::support::refused(program(text), why);
    }
    for text in [
        "Reveal the top card of your library and put that card into your hand.",
        "Reveal the top card of your library. Shuffle. You may put that card into your hand.",
        "Draw a card.",
        "Reveal the top four cards of your library. Put all land cards revealed this way into your hand and the rest into your graveyard. You gain 3 life.",
    ] {
        assert!(program(text).is_ok(), "{text}");
    }
}

#[test]
fn a_reference_with_no_producer_does_not_compile_which_is_not_an_empty_group() {
    let e = program("Put the rest into your graveyard.").unwrap_err();
    assert_eq!(e, "reference Remainder ([]): no earlier instruction produces what it names");
    let e = program("Put all land cards revealed this way into your hand.").unwrap_err();
    assert_eq!(e, "reference ActionResult ([ThisWay(Reveal)]): no earlier instruction produces what it names");
}

#[test]
fn a_reference_two_groups_could_mean_is_ambiguous_and_does_not_compile() {
    // no "most recent" fallback: either reveal could be the one the rest is of
    let e = program("Reveal the top four cards of your library. Reveal the top two cards of your library. Put the rest into your graveyard.").unwrap_err();
    assert_eq!(e, "reference Remainder ([]): more than one earlier group could be meant");
}

#[test]
fn slots_read_before_they_are_produced_or_of_the_wrong_kind_are_refused_by_construction() {
    use game::program::ActSpan;
    let acts = |end| vec![ActSpan { end, moves: false, keeps_x: false }];
    let e = Program::new(vec![Instr::Remainder { of: 3, minus: vec![], still_in: Zone::Library, out: 0 }], acts(1)).unwrap_err();
    assert_eq!(e.0, "slot 3 is read before any instruction produces it");
    let take = Instr::TakeTop { n: 1.into(), whose: game::program::action::Operand::Controller, out: 0 };
    let e = Program::new(vec![take.clone(), take], acts(2)).unwrap_err();
    assert_eq!(e.0, "slot 0 is produced twice");
}

// A retained departure child belongs to the game procedure driver; standalone
// Frame::answer cannot answer its owner-order decision. Route these movement
// witnesses through the same compiled instruction body so children return normally.
fn routed(text: &str, library: &[usize]) -> Game {
    let (mut g, _) = setup(library);
    let body = text.strip_prefix("When this creature enters, ").map(|body| {
        let mut chars=body.chars(); chars.next().unwrap().to_uppercase().collect::<String>()+chars.as_str()
    }).unwrap_or_else(||text.to_owned());
    let def=CardDef::new("Routed program witness", "{0}", "Sorcery", &body, None).unwrap();
    // A zero-cost spell runs the exact compiled body under the retained driver.
    game::play::check(&def).unwrap();
    let index=g.defs.len(); std::sync::Arc::make_mut(&mut g.defs).push(def);
    let spell=g.put(0,index,Zone::Hand);
    g.act(0,game::Move::Cast {card:spell,targets:vec![]}).unwrap();
    for _ in 0..2 {g.act(g.priority,game::Move::Pass).unwrap();}
    g
}

#[test]
fn r701_20b_mulch_an_empty_or_partial_library_runs_and_an_empty_group_is_a_value() {
    let mut g=routed(MULCH,&[FOREST,SHOCK,FOREST,BEARS,FOREST]);
    let shock=ObjId(1); let bears=ObjId(3);
    assert_eq!(g.decision,Some(Decision::Choose{player:0,choice:Choice::OrderCards{to:game::state::Placement::Zone(Zone::Graveyard),cards:vec![bears,shock]}}));
    g.act(0,game::Move::Choose(Answer::Objects(vec![shock,bears]))).unwrap();
    cite!(["608.2c"]; assert_eq!(names(&g,&g.players[0].hand),["Forest","Forest"]));
    cite!(["608.2c"]; assert_eq!(names(&g,&g.players[0].graveyard).iter().filter(|name|name.as_str()!="Routed program witness").cloned().collect::<Vec<_>>(),["Grizzly Bears","Shock"]));
    assert_eq!(g.players[0].graveyard.len(),3); // two remainder cards and the resolved witness
    assert_eq!(g.players[0].library.len(),1); assert!(g.resolving().is_none());
    // The no-child empty case still checks the low-level produced empty slot.
    let (mut g,ctx)=setup(&[]);let mut f=frame_of(MULCH,ctx);
    assert!(run(&mut g,&mut f).unwrap().is_none());assert!(f.finished());
    assert_eq!(f.slot(0),Some(&Value::Cards(vec![])),"{}",f.dump());
    assert!(g.log.iter().all(|e|!matches!(e,game::Event::Revealed{..})),"nothing was shown");
}

#[test]
fn r608_2d_salvage_asks_whether_then_which_and_a_refusal_keeps_everything_in_place() {
    let library=[FOREST,SHOCK,BEARS,FOREST,SHOCK,BEARS];
    let mut g=routed(SALVAGE,&library);
    assert_eq!(g.decision,Some(Decision::Choose{player:0,choice:Choice::May}));
    g.act(0,game::Move::Choose(Answer::Yes)).unwrap();
    let Some(Decision::Choose{choice:Choice::Objects{among,count:1,..},..})=g.decision.clone() else {panic!("{:?}",g.decision)};
    cite!(["608.2d"]; assert_eq!(names(&g,&among),["Forest","Grizzly Bears","Grizzly Bears"]));
    assert_eq!(among[0],ObjId(5));
    g.act(0,game::Move::Choose(Answer::Objects(vec![among[0]]))).unwrap();
    assert_eq!(g.decision,Some(Decision::Choose{player:0,choice:Choice::OrderCards{to:game::state::Placement::Zone(Zone::Graveyard),cards:vec![ObjId(4),ObjId(3),ObjId(2),ObjId(1)]}}));
    g.act(0,game::Move::Choose(Answer::Objects(vec![ObjId(1),ObjId(2),ObjId(3),ObjId(4)]))).unwrap();
    assert_eq!(g.players[0].hand.len(),1);assert_eq!(g.players[0].graveyard.len(),5); // four remainder cards plus resolved witness
    let mut g=routed(SALVAGE,&library);g.act(0,game::Move::Choose(Answer::No)).unwrap();
    assert_eq!(g.decision,Some(Decision::Choose{player:0,choice:Choice::OrderCards{to:game::state::Placement::Zone(Zone::Graveyard),cards:vec![ObjId(5),ObjId(4),ObjId(3),ObjId(2),ObjId(1)]}}));
    g.act(0,game::Move::Choose(Answer::Objects(vec![ObjId(1),ObjId(2),ObjId(3),ObjId(4),ObjId(5)]))).unwrap();
    assert!(g.players[0].hand.is_empty());assert_eq!(g.players[0].graveyard.len(),6);
    let mut g=routed(SALVAGE,&[SHOCK,SHOCK,SHOCK,SHOCK,SHOCK]);
    cite!(["608.2d"]; assert_eq!(g.decision,Some(Decision::Choose{player:0,choice:Choice::OrderCards{to:game::state::Placement::Zone(Zone::Graveyard),cards:vec![ObjId(4),ObjId(3),ObjId(2),ObjId(1),ObjId(0)]}})));
    g.act(0,game::Move::Choose(Answer::Objects(vec![ObjId(0),ObjId(1),ObjId(2),ObjId(3),ObjId(4)]))).unwrap();
    assert_eq!(g.players[0].graveyard.len(),6);assert!(g.resolving().is_none());
}

#[test]
fn ask_before_mutate_running_the_waiting_instruction_again_changes_nothing_and_clones_agree() {
    // Preserve direct-frame idempotence at its own May and selection decisions.
    let (mut direct,ctx)=setup(&[FOREST,FOREST,SHOCK,BEARS,FOREST]);let mut f=frame_of(SATYR,ctx);
    run(&mut direct,&mut f).unwrap();let before=(format!("{direct:?}"),f.clone());
    assert!(run(&mut direct,&mut f).unwrap().is_some());assert_eq!((format!("{direct:?}"),f.clone()),before);
    f.answer(Answer::Yes);let Some(Decision::Choose{choice:Choice::Objects{among,..},..})=run(&mut direct,&mut f).unwrap() else {panic!("{}",f.dump())};
    let (mut direct_copy,mut frame_copy)=(direct.clone(),f.clone());let before=(format!("{direct:?}"),f.clone());
    assert!(run(&mut direct,&mut f).unwrap().is_some());assert_eq!((format!("{direct:?}"),f.clone()),before);
    f.answer(Answer::Objects(vec![among[0]]));frame_copy.answer(Answer::Objects(vec![among[0]]));
    let decision=run(&mut direct,&mut f).unwrap();assert_eq!(decision,run(&mut direct_copy,&mut frame_copy).unwrap());
    assert_eq!(format!("{direct:?}"),format!("{direct_copy:?}"));assert_eq!(f,frame_copy);
    assert_eq!(decision,Some(Decision::Choose{player:0,choice:Choice::OrderCards{to:game::state::Placement::Zone(Zone::Graveyard),cards:vec![ObjId(3),ObjId(2),ObjId(1)]}}));
    // Actual completion uses the retained driver, in each cloned branch.
    let mut g=routed(SATYR,&[FOREST,FOREST,SHOCK,BEARS,FOREST]);g.act(0,game::Move::Choose(Answer::Yes)).unwrap();
    g.act(0,game::Move::Choose(Answer::Objects(vec![ObjId(4)]))).unwrap();let mut copied=g.clone();
    for state in [&mut g,&mut copied] {
        assert_eq!(state.decision,decision);
        state.act(0,game::Move::Choose(Answer::Objects(vec![ObjId(1),ObjId(2),ObjId(3)]))).unwrap();
        assert!(state.resolving().is_none());assert!(state.decision.is_none());
        assert_eq!(names(state,&state.players[0].hand),["Forest"]);
    }
    assert_eq!(format!("{g:?}"),format!("{copied:?}"));
}

#[test]
fn the_remainder_is_the_group_minus_what_was_taken_and_still_where_it_was() {
    let mut g=routed(SATYR,&[SHOCK,FOREST,BEARS,FOREST,BEARS]);
    let departed=ObjId(4);g.move_object(departed,Zone::Exile);
    g.act(0,game::Move::Choose(Answer::Yes)).unwrap();
    let Some(Decision::Choose{choice:Choice::Objects{among,..},..})=g.decision.clone() else {panic!("{:?}",g.decision)};
    assert_eq!(among,vec![ObjId(3),ObjId(1)]);
    g.act(0,game::Move::Choose(Answer::Objects(vec![among[1]]))).unwrap();
    assert_eq!(g.decision,Some(Decision::Choose{player:0,choice:Choice::OrderCards{to:game::state::Placement::Zone(Zone::Graveyard),cards:vec![ObjId(3),ObjId(2)]}}));
    g.act(0,game::Move::Choose(Answer::Objects(vec![ObjId(2),ObjId(3)]))).unwrap();
    assert_eq!(names(&g,&g.players[0].hand),["Forest"]);
    assert_eq!(names(&g,&g.players[0].graveyard).iter().filter(|name|name.as_str()!="Routed program witness").cloned().collect::<Vec<_>>(),["Forest","Grizzly Bears"]);
    assert_eq!(g.players[0].graveyard.len(),3);
    assert_eq!(names(&g,&g.exile),["Grizzly Bears"]);assert_eq!(g.players[0].library.len(),1);
}

#[test]
fn r400_7_the_old_ids_of_moved_cards_are_gone_and_the_slots_keep_the_ids_taken() {
    let (mut g, ctx) = setup(&[BEARS, FOREST]);
    let (bears, forest) = (g.players[0].library[0], g.players[0].library[1]);
    let mut f = frame_of(MULCH, ctx);
    run(&mut g, &mut f).unwrap();
    assert_eq!(f.slot(0), Some(&Value::Cards(vec![forest, bears])), "{}", f.dump());
    cite!(["400.7"]; assert_eq!(g.obj(forest).zone, Zone::Gone));
    cite!(["400.7"]; assert_eq!(g.obj(bears).zone, Zone::Gone));
}

#[test]
fn r701_20a_a_group_is_seen_by_its_audience_only_for_as_long_as_the_frame_lives() {
    let (mut g, ctx) = setup(&[BEARS, FOREST]);
    let top = g.players[0].library[1];
    let mut f = frame_of("Reveal the top card of your library.", ctx);
    assert!(!f.shown_to(&g, 0, top) && !f.shown_to(&g, 1, top));
    run(&mut g, &mut f).unwrap();
    cite!(["701.20a"]; assert!(f.shown_to(&g, 0, top) && f.shown_to(&g, 1, top), "revealed to everyone"));
    // the same instructions with one player as audience: looking, not revealing (hand-built)
    let (mut g, ctx) = setup(&[BEARS, FOREST]);
    let top = g.players[0].library[1];
    let look = Program::new(
        vec![
            Instr::TakeTop { n: 1.into(), whose: game::program::action::Operand::Controller, out: 0 },
            Instr::Show { from: 0, by: Who::Controller, to: Audience::Only(Who::Controller), out: 1 },
        ],
        vec![game::program::ActSpan { end: 2, moves: false, keeps_x: false }],
    )
    .unwrap();
    let mut f = Frame::new(Arc::new(look), ctx);
    run(&mut g, &mut f).unwrap();
    assert!(f.shown_to(&g, 0, top) && !f.shown_to(&g, 1, top));
    assert!(g.log.iter().all(|e| !matches!(e, game::Event::Revealed { .. })), "looking is not revealing (701.20)");
}

#[test]
fn r401_4_random_bottom_placement_leaves_unrevealed_cards_above_the_remainder() {
    // "the rest on the bottom of your library in a random order" asks no one
    let (mut g, ctx) = setup(&[FOREST, BEARS, BEARS, FOREST, BEARS]);
    let unrevealed = g.players[0].library[0];
    let mut f = frame_of(BOTTOM, ctx);
    let d = run(&mut g, &mut f).unwrap();
    // Three creatures fit and one is taken: chosen first.
    let Some(Decision::Choose { choice: Choice::Objects { among, count: 1, .. }, .. }) = d else { panic!("{}", f.dump()) };
    assert_eq!(among.len(), 3);
    f.answer(Answer::Objects(vec![among[0]]));
    assert!(run(&mut g, &mut f).unwrap().is_none(), "{}", f.dump());
    assert_eq!(g.players[0].hand.len(), 1);
    assert_eq!(g.players[0].library.len(), 4, "the rest are back in the library, at the bottom");
    assert_eq!(g.players[0].library.last(), Some(&unrevealed));
    assert!(g.players[0].graveyard.is_empty());
}

#[test]
fn r113_7a_the_frame_carries_its_own_context_and_never_looks_the_source_up() {
    let (mut g, _) = setup(&[FOREST]);
    let source = g.put(1, BEARS, Zone::Battlefield);
    let other_library = g.put(1, SHOCK, Zone::Library);
    let ctx = Ctx::new(0, source);
    let mut f = frame_of(MULCH, ctx);
    // The retained controller differs from the departed source's owner.
    g.move_object(source, Zone::Graveyard);
    assert!(run(&mut g, &mut f).unwrap().is_none());
    cite!(["113.7a"]; assert_eq!(names(&g, &g.players[0].hand), ["Forest"]));
    assert!(g.players[0].library.is_empty());
    assert!(g.players[1].hand.is_empty());
    assert_eq!(g.players[1].library, vec![other_library]);
}

#[test]
fn the_chooser_is_a_role_of_the_frame_not_read_from_the_player_that_answers() {
    // not a pipeline test: a hand-built frame, to show the decision is addressed to the chooser
    let (mut g, ctx) = setup(&[FOREST, FOREST, FOREST]);
    g.active = 1;
    g.priority = 1;
    let mut f = frame_of("Reveal the top three cards of your library. Put two land cards from among them into your hand and the rest into your graveyard.", ctx.clone());
    let Some(Decision::Choose { player, .. }) = run(&mut g, &mut f).unwrap() else { panic!() };
    assert_eq!(player, ctx.controller);
}

#[test]
fn the_debug_dump_names_the_position_and_the_bindings() {
    let (mut g, ctx) = setup(&[FOREST, BEARS]);
    let mut f = frame_of(MULCH, ctx);
    run(&mut g, &mut f).unwrap();
    let d = f.dump();
    assert!(d.contains("s3 = select All of s1") && d.contains("remainder of s1 minus [s3]"), "{d}");
    assert!(d.contains("s1 = [") && !d.contains("not produced"), "{d}");
}
