//! Orcish Bowmasters, its Oracle text: "When this creature enters and whenever an opponent
//! draws a card except the first one they draw in each of their draw steps, this creature
//! deals 1 damage to any target. Then amass Orcs 1."
//!
//! Amass is what 701.47a says (create an Orc Army only if its controller has no Army, choose
//! an Army, put the counters on it, and it becomes an Orc). The exception counts each
//! opponent's completed draws within each of their own draw steps (504.1, 121.2, 614.11):
//! not their first draw of the turn, and not a draw that was replaced.
use crate::support::cite;
use game::legal::Asked;
use game::simulation::{ResponseError, Simulation};
use game::state::Answer;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::{Color, Part, mana::ManaSymbol};
use std::sync::OnceLock;

const BOWMASTERS: usize = 0;
const ISLAND: usize = 1;
const DRAW_ONE: usize = 2;
const DREDGER: usize = 3;
const REFLECTION: usize = 4;
const ZOMBIE_ARMY: usize = 5;
const SCALES: usize = 6;
const BEARS: usize = 7;
const OPPONENTS_DRAW: usize = 8;

fn defs() -> Vec<CardDef> {
    static FACES: OnceLock<Vec<oracle::corpus::Face>> = OnceLock::new();
    let faces = FACES.get_or_init(|| oracle::corpus::load(&oracle::corpus::default_path()).unwrap());
    let card = |name: &str, cost: &str, types: &str, text: &str, pt: Option<(i32, i32)>| {
        let def = CardDef::new(name, cost, types, text, pt).unwrap();
        game::play::check(&def).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        def
    };
    let bowmasters = CardDef::from_faces(CardDef::catalog(faces).get("Orcish Bowmasters").unwrap()).unwrap();
    game::play::check(&bowmasters).unwrap();
    vec![
        bowmasters,
        card("Island", "", "Basic Land — Island", "", None),
        card("Draw one", "{0}", "Instant", "Draw a card.", None),
        card("Dredger", "{0}", "Creature — Zombie", "Dredge 1", Some((1, 1))),
        card("Thought Reflection", "{4}{U}{U}{U}", "Enchantment", "If you would draw a card, draw two cards instead.", None),
        card("Zombie Army", "{0}", "Creature — Zombie Army", "", Some((1, 1))),
        card("Hardened Scales", "{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Opponents draw", "{0}", "Instant", "Each opponent draws two cards.", None),
    ]
}

fn game() -> Game {
    let mut g = Game::new(defs(), 2, 20);
    for p in 0..2 { for _ in 0..20 { g.put(p, ISLAND, Zone::Library); } }
    g
}

/// Pass until it is this player's turn and this part of it, nothing on the stack.
fn to(g: &mut Game, active: usize, part: Part) {
    while !(g.active == active && g.part == part && g.stack.is_empty()) { g.act(g.priority, Move::Pass).unwrap(); }
}
fn pass_all(g: &mut Game) { while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); } }
/// Player `p` casts a {0} spell with priority (the others pass to them first).
fn cast(g: &mut Game, p: usize, def: usize) {
    while g.priority != p { g.act(g.priority, Move::Pass).unwrap(); }
    let card = g.put(p, def, Zone::Hand);
    g.act(p, Move::Cast { card, targets: vec![] }).unwrap();
}
/// Answer each pending Bowmasters target question with `target`: how many triggered.
fn shots(g: &mut Game, target: Target) -> usize {
    let mut n = 0;
    while let Some((0, Asked::Targets { .. })) = g.asked() {
        g.act(0, Move::ChooseTargets(vec![vec![target]])).unwrap();
        n += 1;
    }
    n
}
/// Bowmasters enters under player 0's control with flash, its trigger aimed at `target`.
fn enter(g: &mut Game, target: Target) -> ObjId {
    while g.priority != 0 { g.act(g.priority, Move::Pass).unwrap(); }
    let card = g.put(0, BOWMASTERS, Zone::Hand);
    g.add_mana(0, ManaSymbol::Color(Color::Black));
    g.add_mana(0, ManaSymbol::Colorless);
    g.act(0, crate::support::paid("BC", Move::Cast { card, targets: vec![] })).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    assert_eq!(shots(g, target), 1, "its enters trigger");
    *g.battlefield.iter().find(|o| g.obj(**o).card == Some(BOWMASTERS)).unwrap()
}
fn armies(g: &Game, p: usize) -> Vec<ObjId> {
    let army = ir::Subtype::from_name("Army").unwrap();
    g.battlefield.iter().copied().filter(|o| g.controller(*o) == p && g.seen(*o, None).subtypes.contains(&army)).collect()
}
fn is_orc(g: &Game, o: ObjId) -> bool { g.seen(o, None).subtypes.contains(&ir::Subtype::from_name("Orc").unwrap()) }
fn plus(g: &Game, o: ObjId) -> u32 { g.obj(o).counters.get("+1/+1").copied().unwrap_or(0) }

// ---------------------------------------------------------------- enters, amass

#[test]
fn r701_47a_enters_deals_one_then_creates_a_black_orc_army_with_a_counter() {
    let mut g = game();
    enter(&mut g, Target::Player(1));
    pass_all(&mut g);
    cite!(["120.3a"]; assert_eq!(g.players[1].life, 19));
    let [army] = armies(&g, 0)[..] else { panic!("{:?}", armies(&g, 0)) };
    let seen = g.seen(army, None);
    cite!(["701.47a"]; assert!(g.obj(army).card.is_none(), "a token"));
    cite!(["701.47a"]; assert_eq!(seen.colors, vec![Color::Black]));
    assert!(is_orc(&g, army));
    cite!(["701.47a"]; assert_eq!((plus(&g, army), g.power(army), g.toughness(army)), (1, Some(1), Some(1)), "0/0 with a +1/+1 counter"));
    // a second one amasses onto the Army there is: no second token
    enter(&mut g, Target::Player(1));
    pass_all(&mut g);
    cite!(["701.47a"]; assert_eq!(armies(&g, 0), vec![army]));
    cite!(["701.47a"]; assert_eq!(plus(&g, army), 2));
}

#[test]
fn r701_47a_a_non_orc_army_gets_the_counters_and_becomes_an_orc() {
    let mut g = game();
    let zombie = g.put(0, ZOMBIE_ARMY, Zone::Battlefield);
    assert!(!is_orc(&g, zombie));
    enter(&mut g, Target::Player(1));
    pass_all(&mut g);
    cite!(["701.47a"]; assert_eq!(armies(&g, 0), vec![zombie], "no token while an Army is controlled"));
    cite!(["701.47a"]; assert_eq!(plus(&g, zombie), 1));
    let zombie_type = ir::Subtype::from_name("Zombie").unwrap();
    cite!(["701.47a"]; assert!(is_orc(&g, zombie) && g.seen(zombie, None).subtypes.contains(&zombie_type), "an Orc in addition to its other types"));
}

#[test]
fn r701_47a_with_several_armies_the_controller_chooses_one() {
    let mut g = game();
    let a = g.put(0, ZOMBIE_ARMY, Zone::Battlefield);
    let b = g.put(0, ZOMBIE_ARMY, Zone::Battlefield);
    // the opponent's Army is not one to choose
    g.put(1, ZOMBIE_ARMY, Zone::Battlefield);
    enter(&mut g, Target::Player(1));
    while g.asked().is_some_and(|(_, q)| matches!(q, Asked::Priority)) { g.act(g.priority, Move::Pass).unwrap(); }
    let Some((0, Asked::Objects { among, count: 1, .. })) = g.asked() else { panic!("{:?}", g.asked()) };
    let mut sorted = among.clone();
    sorted.sort();
    cite!(["701.47a"]; assert_eq!(sorted, { let mut v = vec![a, b]; v.sort(); v }));
    cite!(["701.47a"]; refused("that decision is another player's"): g.act(1, Move::Choose(Answer::Objects(vec![b]))));
    let mut fork = g.clone();
    g.act(0, Move::Choose(Answer::Objects(vec![b]))).unwrap();
    fork.act(0, Move::Choose(Answer::Objects(vec![a]))).unwrap();
    pass_all(&mut g);
    pass_all(&mut fork);
    cite!(["701.47a"]; assert_eq!((plus(&g, a), plus(&g, b), is_orc(&g, a), is_orc(&g, b)), (0, 1, false, true)));
    cite!(["701.47a"]; assert_eq!((plus(&fork, a), plus(&fork, b), is_orc(&fork, a), is_orc(&fork, b)), (1, 0, true, false), "a clone resumes at the choice"));
}

#[test]
fn r614_1_counter_replacement_applies_to_the_amassed_counters() {
    let mut g = game();
    g.put(0, SCALES, Zone::Battlefield);
    enter(&mut g, Target::Player(1));
    pass_all(&mut g);
    let [army] = armies(&g, 0)[..] else { panic!() };
    cite!(["614.1a"]; assert_eq!(plus(&g, army), 2, "one, plus one"));
}

#[test]
fn r608_2b_an_illegal_target_does_not_amass_but_a_creature_target_dealt_damage_does() {
    // a creature target: 1 damage marked, and the Army
    let mut g = game();
    let bears = g.put(1, BEARS, Zone::Battlefield);
    enter(&mut g, Target::Object(bears));
    pass_all(&mut g);
    cite!(["608.2b"]; assert_eq!(g.obj(bears).damage, 1));
    cite!(["701.47a"]; assert_eq!(armies(&g, 0).len(), 1));
    // its only target gone: it doesn't resolve, and nothing is amassed (608.2b)
    let mut g = game();
    let bears = g.put(1, BEARS, Zone::Battlefield);
    enter(&mut g, Target::Object(bears));
    g.move_object(bears, Zone::Graveyard);
    pass_all(&mut g);
    cite!(["608.2b"]; assert!(armies(&g, 0).is_empty()));
}

// ---------------------------------------------------------------- the draw exception

#[test]
fn r504_1_an_opponents_draws_outside_their_draw_step_each_trigger() {
    let mut g = game();
    let bowmasters = enter(&mut g, Target::Player(1));
    pass_all(&mut g);
    // on its controller's turn: "Each opponent draws two cards."
    cast(&mut g, 0, OPPONENTS_DRAW);
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    // Each draw is a distinct occurrence, even from the same Bowmasters. Its
    // controller orders both before choosing their targets (603.3b-d).
    let Some((0, Asked::Order(abilities))) = g.asked() else { panic!("expected two draw triggers to order: {:?}", g.asked()) };
    cite!(["603.3b"]; assert_eq!(abilities.len(), 2));
    assert!(abilities.iter().all(|(source, _)| *source == Some(bowmasters)));
    g.act(0, Move::Order(vec![0, 1])).unwrap();
    cite!(["121.2", "603.2"]; assert_eq!(shots(&mut g, Target::Player(1)), 2));
    pass_all(&mut g);
    assert_eq!(g.players[1].life, 20 - 3);
    // its controller's own draws do not
    cast(&mut g, 0, DRAW_ONE);
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["603.2"]; assert_eq!(shots(&mut g, Target::Player(1)), 0));
}

#[test]
fn r504_1_the_first_draw_of_their_draw_step_is_excepted_not_their_first_of_the_turn() {
    let mut g = game();
    enter(&mut g, Target::Player(1));
    pass_all(&mut g);
    // in their upkeep, their first draw of the turn: it triggers
    to(&mut g, 1, Part::Upkeep);
    cast(&mut g, 1, DRAW_ONE);
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["603.2"]; assert_eq!(shots(&mut g, Target::Player(1)), 1));
    pass_all(&mut g);
    // the draw step's draw is the first of that step: no trigger
    to(&mut g, 1, Part::Draw);
    cite!(["603.2"]; assert_eq!(shots(&mut g, Target::Player(1)), 0));
    assert!(g.stack.is_empty());
    // a second draw in that step: it triggers
    cast(&mut g, 1, DRAW_ONE);
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["603.2"]; assert_eq!(shots(&mut g, Target::Player(1)), 1));
}

#[test]
fn r614_11_a_replaced_draw_is_not_the_first_one_they_draw() {
    // Thought Reflection: the step's draw becomes two draws; the first of them is the step's
    // first, the second triggers
    let mut g = game();
    g.put(1, REFLECTION, Zone::Battlefield);
    enter(&mut g, Target::Player(0));
    pass_all(&mut g);
    to(&mut g, 1, Part::Draw);
    cite!(["614.1a", "121.2"]; assert_eq!(shots(&mut g, Target::Player(0)), 1));

    // dredge: the step's draw is replaced by no draw at all; the next card they draw in that
    // step is the first one
    let mut g = game();
    let dredger = g.put(1, DREDGER, Zone::Hand);
    g.move_object(dredger, Zone::Graveyard);
    enter(&mut g, Target::Player(0));
    pass_all(&mut g);
    // the step's draw asks whether to dredge instead (702.52a)
    let mut asked = false;
    while !(g.active == 1 && g.part == Part::Draw && matches!(g.asked(), Some((_, Asked::Priority)))) {
        match g.asked() {
            Some((1, Asked::Replacement { among, .. })) => { asked = true; g.act(1, Move::Choose(Answer::Replacement(among[0].clone()))).unwrap() }
            Some((1, Asked::May)) => { asked = true; g.act(1, Move::Choose(Answer::Yes)).unwrap() }
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    cite!(["702.52a"]; assert!(asked, "dredge was offered"));
    assert!(g.players[1].hand.iter().all(|o| g.obj(*o).card != Some(ISLAND)), "no card was drawn");
    cite!(["702.52a"]; assert!(g.players[1].hand.iter().any(|o| g.obj(*o).card == Some(DREDGER)), "dredged back"));
    cite!(["614.1a"]; assert_eq!(shots(&mut g, Target::Player(0)), 0));
    cast(&mut g, 1, DRAW_ONE);
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["614.1a"]; assert_eq!(shots(&mut g, Target::Player(0)), 0, "the first card they draw in this step"));
    pass_all(&mut g);
    cast(&mut g, 1, DRAW_ONE);
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["603.2"]; assert_eq!(shots(&mut g, Target::Player(0)), 1));
}

// ---------------------------------------------------------------- managed Simulation

#[test]
fn simulation_asks_the_controller_for_the_target_and_resumes_to_amass() {
    let mut g = game();
    let card = g.put(0, BOWMASTERS, Zone::Hand);
    g.add_mana(0, ManaSymbol::Color(Color::Black));
    g.add_mana(0, ManaSymbol::Colorless);
    let mut sim = Simulation::new(g);
    let r = sim.request().unwrap().unwrap().clone();
    sim.respond(r.id, 0, crate::support::paid("BC", Move::Cast { card, targets: vec![] })).unwrap();
    let r = loop {
        let r = sim.request().unwrap().unwrap().clone();
        if !matches!(r.question, Asked::Priority) { break r; }
        sim.respond(r.id, r.player, Move::Pass).unwrap();
    };
    cite!(["603.3d"]; assert_eq!(r.player, 0));
    cite!(["603.3d"]; assert!(matches!(r.question, Asked::Targets { .. }), "{:?}", r.question));
    assert_eq!(sim.respond(r.id, 1, Move::ChooseTargets(vec![vec![Target::Player(0)]])), Err(ResponseError::WrongPlayer { expected: 0, received: 1 }));
    sim.respond(r.id, 0, Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap();
    let copy = sim.observe(|g| g.clone()).unwrap();
    loop {
        let r = sim.request().unwrap().unwrap().clone();
        if !matches!(r.question, Asked::Priority) { panic!("{:?}", r.question); }
        if sim.observe(|g| g.stack.is_empty()).unwrap() { break; }
        sim.respond(r.id, r.player, Move::Pass).unwrap();
    }
    assert!(sim.fault().is_none());
    let (life, armies_now) = sim.observe(|g| (g.players[1].life, armies(g, 0).len())).unwrap();
    cite!(["608.2c", "701.47a"]; assert_eq!((life, armies_now), (19, 1)));
    // the copy taken with the trigger on the stack resolves the same way
    let mut copy = copy;
    pass_all(&mut copy);
    assert_eq!((copy.players[1].life, armies(&copy, 0).len()), (19, 1));
}
