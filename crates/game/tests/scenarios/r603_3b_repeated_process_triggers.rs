//! What a repeated process causes waits for it. Countryside Crusher puts each land it
//! reveals into its controller's graveyard (608.2c); every ability that triggers on those
//! moves, its own or other permanents', its controller's or an opponent's, or a card's in a
//! graveyard, triggers once for each land and is put on the stack only once the upkeep
//! ability has finished resolving (603.3, 603.3b, ruling 78f6e66c). What then happens as those
//! triggers resolve does not run the process again.
use game::event::Event;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Zone};
use ir::Part;
use crate::support::{cite, mana_pool};

fn pinned(name: &str) -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    CardDef::from_face(faces.iter().find(|f| f.face_name == name).unwrap_or_else(|| panic!("{name} pinned"))).unwrap()
}

const CRUSHER: usize = 0;
const FOREST: usize = 1;
const MOUNTAIN: usize = 2;
const BOLT: usize = 3;
/// The first card of `defs(more)` after these.
const MORE: usize = 4;

fn defs(more: Vec<CardDef>) -> Vec<CardDef> {
    let mut defs = vec![
        pinned("Countryside Crusher"),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Mountain", "", "Basic Land — Mountain", "", None).unwrap(),
        CardDef::new("Lightning Bolt", "{R}", "Instant", "Lightning Bolt deals 3 damage to any target.", None).unwrap(),
    ];
    defs.extend(more);
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    defs
}

fn pinned_too(names: &[&str]) -> Vec<CardDef> { defs(names.iter().map(|n| pinned(n)).collect()) }

/// A game whose player 0 has these cards in their library, the first on top.
fn game(defs: Vec<CardDef>, library: &[usize]) -> Game {
    let mut g = Game::new(defs, 2, 20);
    for &card in library.iter().rev() { g.put(0, card, Zone::Library); }
    for _ in 0..5 { g.put(1, FOREST, Zone::Library); }
    g
}

fn cards(g: &Game, objects: &[ObjId]) -> Vec<usize> { objects.iter().map(|o| g.obj(*o).card.unwrap()).collect() }
fn library(g: &Game) -> Vec<usize> { g.players[0].library.iter().rev().map(|o| g.obj(*o).card.unwrap()).collect() }
fn graveyard(g: &Game) -> Vec<usize> { cards(g, &g.players[0].graveyard) }
fn revealed(g: &Game) -> Vec<Vec<usize>> {
    g.log.iter().filter_map(|e| match e { Event::Revealed { player: 0, cards: shown } => Some(cards(g, shown)), _ => None }).collect()
}
fn plus(g: &Game, o: ObjId) -> u32 { g.counters(o).get("+1/+1").copied().unwrap_or(0) }
/// Of the abilities on the stack, bottom first: whose ability and who controls it.
fn stack(g: &Game) -> Vec<(Option<ObjId>, usize)> {
    g.stack.iter().map(|o| (g.obj(*o).ability.expect("an ability").0, g.obj(*o).controller)).collect()
}

/// Order triggers as they are listed, if asked.
fn order(g: &mut Game) {
    if let Some(Decision::Order { player, abilities }) = g.decision.clone() {
        g.act(player, Move::Order((0..abilities.len()).collect())).unwrap();
    }
}

/// From player 0's untap step until its upkeep triggers are on the stack.
fn to_upkeep_trigger(g: &mut Game) {
    g.turn = 3;
    g.part = Part::Untap;
    for _ in 0..10 {
        order(g);
        if !g.stack.is_empty() { return }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("no upkeep trigger");
}

/// Everyone passes: the top of the stack resolves, and what it caused waits to be ordered.
fn resolve_top(g: &mut Game) {
    let top = *g.stack.last().expect("something to resolve");
    while g.stack.contains(&top) && g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
}

/// Resolve the whole stack: triggers ordered as listed, each "may" declined.
fn settle(g: &mut Game) {
    for _ in 0..200 {
        match g.decision.clone() {
            Some(Decision::Order { .. }) => { order(g); continue }
            Some(Decision::Choose { player, choice: Choice::May }) => { g.act(player, Move::Choose(Answer::No)).unwrap(); continue }
            Some(other) => panic!("asked {other:?}"),
            None => {}
        }
        if g.stack.is_empty() { return }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("the stack did not empty");
}
/// Skola Grovedancer's and the Crusher's triggers for every land are put on the stack
/// together, in the order their controller chooses, once the process is done (603.3b).
#[test]
fn r603_3b_the_controller_orders_every_trigger_of_every_land_after_the_process() {
    let mut g = game(pinned_too(&["Skola Grovedancer"]), &[FOREST, MOUNTAIN, BOLT]);
    let crusher = g.put(0, CRUSHER, Zone::Battlefield);
    g.put(0, MORE, Zone::Battlefield);
    to_upkeep_trigger(&mut g);
    resolve_top(&mut g);
    let Some(Decision::Order { player: 0, abilities }) = g.decision.clone() else { panic!("asked {:?}", g.decision) };
    cite!(["603.3b"]; assert_eq!(abilities.len(), 4, "two triggers for each of two lands"));
    cite!(["603.3"]; assert_eq!(graveyard(&g), [FOREST, MOUNTAIN], "asked once the process is done"));
    assert!(g.stack.is_empty());
    settle(&mut g);
    assert_eq!(g.players[0].life, 22);
    assert_eq!(plus(&g, crusher), 2);
}

/// Vulturous Zombie's triggers are its controller's; theirs go on the stack after the
/// active player's (APNAP, 101.4), so they resolve first.
#[test]
fn r603_3b_an_opponents_triggers_for_the_same_lands_go_on_the_stack_last() {
    let mut g = game(pinned_too(&["Vulturous Zombie"]), &[FOREST, MOUNTAIN, BOLT]);
    let crusher = g.put(0, CRUSHER, Zone::Battlefield);
    let zombie = g.put(1, MORE, Zone::Battlefield);
    to_upkeep_trigger(&mut g);
    resolve_top(&mut g);
    for (controller,source) in [(0,crusher),(1,zombie)] {
        let Some(Decision::Order{player,abilities})=g.decision.clone() else {panic!("expected APNAP ordering: {:?}",g.decision)};
        cite!(["603.3b","101.4"]; assert_eq!((player,abilities.len()),(controller,2)));
        assert!(abilities.iter().all(|ability|ability.source==Some(source)));
        // Distinct departed-land subjects remain separate; each controller chooses
        // offered order, with the nonactive player's pair above the active pair.
        g.act(controller,Move::Order(vec![0,1])).unwrap();
    }
    cite!(["603.3b", "101.4"]; assert_eq!(stack(&g), [(Some(crusher), 0), (Some(crusher), 0), (Some(zombie), 1), (Some(zombie), 1)]));
    resolve_top(&mut g);
    assert_eq!((plus(&g, crusher), plus(&g, zombie)), (0, 1));
    settle(&mut g);
    assert_eq!((plus(&g, crusher), plus(&g, zombie)), (2, 2));
}

/// Centaur Vinecrasher in a graveyard triggers for each land too; each trigger offers its
/// {G}{G} when it resolves, after the process.
#[test]
fn r603_3b_a_graveyard_card_triggers_for_each_land_and_each_offers_its_payment() {
    let mut g = game(pinned_too(&["Centaur Vinecrasher"]), &[FOREST, MOUNTAIN, BOLT]);
    let crusher = g.put(0, CRUSHER, Zone::Battlefield);
    let vinecrasher = g.put(0, MORE, Zone::Graveyard);
    to_upkeep_trigger(&mut g);
    resolve_top(&mut g);
    let Some(Decision::Order { player: 0, abilities }) = g.decision.clone() else { panic!("asked {:?}", g.decision) };
    cite!(["603.3b"]; assert_eq!(abilities.len(), 4));
    g.players[0].pool.colors = [0, 0, 0, 0, 4];
    let mut offered = 0;
    for _ in 0..100 {
        match g.decision.clone() {
            Some(Decision::Order { .. }) => order(&mut g),
            Some(Decision::Choose { player: 0, .. }) => {
                offered += 1;
                g.act(0, Move::Choose(if offered == 2 { Answer::Pay {plan:vec![],with:Some(mana_pool("GG")),tapped:vec![]} } else { Answer::No })).unwrap();
            }
            Some(other) => panic!("asked {other:?}"),
            None if g.stack.is_empty() => break,
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    assert_eq!(offered, 2, "one offer for each land");
    cite!(["118.12"]; assert_eq!(g.obj(vinecrasher).zone, Zone::Gone, "returned by the second payment"));
    assert_eq!(g.players[0].hand.iter().filter(|o| g.obj(**o).card == Some(MORE)).count(), 1);
    assert_eq!(plus(&g, crusher), 2);
}

/// After the process, each counter trigger meets Hardened Scales and Corpsejack Menace; the
/// Crusher's controller orders them (616.1, ruling 4e46b671): Scales first makes 4, Menace
/// first 3. Each choice made from a copy of the game at that question is its own, and none
/// runs the upkeep process again.
#[test]
fn r616_1_counter_replacements_order_after_the_process_on_independent_copies() {
    let mut g = game(pinned_too(&["Hardened Scales", "Corpsejack Menace"]), &[FOREST, BOLT]);
    let crusher = g.put(0, CRUSHER, Zone::Battlefield);
    let scales = g.put(0, MORE, Zone::Battlefield);
    let menace = g.put(0, MORE + 1, Zone::Battlefield);
    to_upkeep_trigger(&mut g);
    resolve_top(&mut g);
    assert_eq!(g.stack.len(), 1);
    resolve_top(&mut g);
    let Some(Decision::Choose { player: 0, choice: Choice::Replacement { among, count: 1, .. } }) = g.decision.clone() else { panic!("asked {:?}", g.decision) };
    let (before, shown, top) = (graveyard(&g), revealed(&g), library(&g));
    assert_eq!(shown, [vec![FOREST], vec![BOLT]]);
    for (first, expected) in [(scales, 4), (menace, 3)] {
        let mut copy = g.clone();
        let effect = among.iter().find(|id| id.source == first).unwrap().clone();
        copy.act(0, Move::Choose(Answer::Replacement(effect))).unwrap();
        settle(&mut copy);
        cite!(["616.1"]; case: format!("{expected}"); assert_eq!(plus(&copy, crusher), expected));
        assert_eq!((graveyard(&copy), revealed(&copy), library(&copy)), (before.clone(), shown.clone(), top.clone()));
        assert!(copy.stack.is_empty());
    }
    assert_eq!(plus(&g, crusher), 0, "the original is still asking");
}

