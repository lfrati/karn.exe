//! "Repeat this process" (608.2c): Countryside Crusher reveals the top card of its controller's
//! library and, while it is a land card, puts it into their graveyard and does it again. Each
//! time names the card that time revealed, and the first nonland card stays on top (ruling
//! 948c4fe5). Its other ability triggers once for each land put there; those triggers, and
//! every other one the process caused, are put on the stack only after the upkeep ability has
//! finished resolving (603.3, ruling 78f6e66c). A loop that could not stop would be a draw
//! (104.4b), so only a process each time through which takes a card out of the library is run.
//! Other abilities triggering on the same moves: `r603_3b_repeated_process_triggers`.
use crate::support::{cite, paid};
use game::event::Event;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

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

const UPKEEP: &str = "At the beginning of your upkeep, reveal the top card of your library. If it's a land card, put it into your graveyard and repeat this process.";

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

#[test]
fn r608_2c_crusher_puts_each_land_into_the_graveyard_and_stops_at_the_first_nonland() {
    let mut g = game(defs(Vec::new()), &[FOREST, MOUNTAIN, BOLT, FOREST]);
    let crusher = g.put(0, CRUSHER, Zone::Battlefield);
    to_upkeep_trigger(&mut g);
    resolve_top(&mut g);
    cite!(["608.2c"]; assert_eq!(revealed(&g), [vec![FOREST], vec![MOUNTAIN], vec![BOLT]], "each time reveals the card then on top"));
    cite!(["608.2c"]; assert_eq!(graveyard(&g), [FOREST, MOUNTAIN]));
    cite!(["701.20b"]; assert_eq!(library(&g), [BOLT, FOREST], "the nonland card stays on top"));
    let Some(Decision::Order{player:0,abilities})=g.decision.clone() else {panic!("expected the completed process's counter triggers: {:?}",g.decision)};
    cite!(["603.3","603.3b"]; assert_eq!(abilities.len(),2));
    assert!(abilities.iter().all(|ability|ability.source==Some(crusher)));
    // Both land subjects are retained; choose offered order only after the process.
    g.act(0,Move::Order(vec![0,1])).unwrap();
    // one trigger for each land, both waiting for the whole ability (603.3)
    cite!(["603.3"]; assert_eq!(stack(&g), [(Some(crusher), 0), (Some(crusher), 0)]));
    assert_eq!(plus(&g, crusher), 0);
    settle(&mut g);
    assert_eq!(plus(&g, crusher), 2);
    assert_eq!(revealed(&g).len(), 3, "the counter triggers do not run the process again");
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    assert_eq!(cards(&g, &g.players[0].hand), [BOLT], "the revealed nonland card is the next draw");
}

#[test]
fn r608_2c_crusher_reveals_nothing_from_an_empty_library_and_loses_no_game() {
    let mut g = game(defs(Vec::new()), &[]);
    let crusher = g.put(0, CRUSHER, Zone::Battlefield);
    to_upkeep_trigger(&mut g);
    resolve_top(&mut g);
    assert!(revealed(&g).is_empty());
    assert!(g.stack.is_empty() && g.decision.is_none());
    assert!(graveyard(&g).is_empty());
    assert_eq!(plus(&g, crusher), 0);
    cite!(["704.5b"]; assert!(g.winner().is_none(), "revealing an empty library is not drawing from it"));
}

#[test]
fn r608_2c_a_nonland_card_on_top_stops_the_process_at_once() {
    let mut g = game(defs(Vec::new()), &[BOLT, FOREST]);
    let crusher = g.put(0, CRUSHER, Zone::Battlefield);
    to_upkeep_trigger(&mut g);
    resolve_top(&mut g);
    cite!(["608.2c"]; assert_eq!(revealed(&g), [vec![BOLT]]));
    cite!(["701.20b"]; assert_eq!(library(&g), [BOLT, FOREST]));
    assert!(graveyard(&g).is_empty() && g.stack.is_empty());
    assert_eq!(plus(&g, crusher), 0);
}

#[test]
fn r608_2c_a_library_of_lands_is_put_into_the_graveyard_until_none_is_left() {
    let mut g = game(defs(Vec::new()), &[FOREST, MOUNTAIN, FOREST, FOREST, MOUNTAIN]);
    let crusher = g.put(0, CRUSHER, Zone::Battlefield);
    to_upkeep_trigger(&mut g);
    resolve_top(&mut g);
    assert_eq!(revealed(&g), [vec![FOREST], vec![MOUNTAIN], vec![FOREST], vec![FOREST], vec![MOUNTAIN]]);
    cite!(["608.2c"]; assert_eq!(graveyard(&g), [FOREST, MOUNTAIN, FOREST, FOREST, MOUNTAIN], "in the order they were revealed"));
    assert!(library(&g).is_empty());
    let Some(Decision::Order{player:0,abilities})=g.decision.clone() else {panic!("expected five distinct land-subject triggers: {:?}",g.decision)};
    cite!(["603.3b"]; assert_eq!(abilities.len(),5));
    assert!(abilities.iter().all(|ability|ability.source==Some(crusher)));
    // Every trigger adds one counter; its controller explicitly chooses offered order.
    g.act(0,Move::Order(vec![0,1,2,3,4])).unwrap();
    cite!(["603.3"]; assert_eq!(g.stack.len(), 5));
    settle(&mut g);
    assert_eq!(plus(&g, crusher), 5);
    assert!(g.winner().is_none() && g.players.iter().all(|p| !p.lost));
}

/// The upkeep ability does what it says for the player who controlled it, even when the
/// Crusher has left; a land put into the graveyard then triggers nothing of it.
#[test]
fn r608_2c_the_process_goes_on_after_its_source_leaves_the_battlefield() {
    let mut g = game(defs(Vec::new()), &[FOREST, MOUNTAIN]);
    let crusher = g.put(0, CRUSHER, Zone::Battlefield);
    let bolt = g.put(0, BOLT, Zone::Hand);
    to_upkeep_trigger(&mut g);
    g.players[0].pool.colors = [0, 0, 0, 1, 0];
    g.act(0, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Object(crusher)]] })).unwrap();
    resolve_top(&mut g);
    assert_ne!(g.obj(crusher).zone, Zone::Battlefield);
    resolve_top(&mut g);
    cite!(["113.7a", "608.2c"]; assert_eq!(revealed(&g), [vec![FOREST], vec![MOUNTAIN]]));
    cite!(["113.7a", "608.2c"]; assert_eq!(cards(&g, &g.players[0].graveyard).iter().filter(|c| **c == FOREST || **c == MOUNTAIN).count(), 2));
    assert!(library(&g).is_empty());
    assert!(g.stack.is_empty() && g.decision.is_none(), "a Crusher that has left gets no counters");
}

#[test]
fn r608_2c_a_land_put_into_the_graveyard_otherwise_triggers_the_crusher_once() {
    let raze = CardDef::new("Raze", "{0}", "Sorcery", "Destroy target land.", None).unwrap();
    let mut g = game(defs(vec![raze]), &[]);
    g.part = Part::PrecombatMain;
    let crusher = g.put(0, CRUSHER, Zone::Battlefield);
    let forest = g.put(0, FOREST, Zone::Battlefield);
    let raze = g.put(0, MORE, Zone::Hand);
    g.act(0, Move::Cast { card: raze, targets: vec![vec![Target::Object(forest)]] }).unwrap();
    settle(&mut g);
    cite!(["603.2"]; assert_eq!(plus(&g, crusher), 1, "from anywhere: the battlefield too"));
    assert!(revealed(&g).is_empty());
}

#[test]
fn r608_2c_two_crushers_both_count_each_land_and_the_second_reveals_the_new_top() {
    let mut g = game(defs(Vec::new()), &[FOREST, MOUNTAIN, BOLT]);
    let first = g.put(0, CRUSHER, Zone::Battlefield);
    let second = g.put(0, CRUSHER, Zone::Battlefield);
    to_upkeep_trigger(&mut g);
    assert_eq!(g.stack.len(), 2);
    resolve_top(&mut g);
    order(&mut g);
    // the second upkeep ability waits under the four counter triggers
    cite!(["603.3"]; assert_eq!(g.stack.len(), 5));
    assert_eq!(revealed(&g), [vec![FOREST], vec![MOUNTAIN], vec![BOLT]]);
    settle(&mut g);
    cite!(["603.2"]; assert_eq!((plus(&g, first), plus(&g, second)), (2, 2)));
    cite!(["608.2c"]; assert_eq!(revealed(&g), [vec![FOREST], vec![MOUNTAIN], vec![BOLT], vec![BOLT]], "the second sees the card now on top"));
    assert_eq!(library(&g), [BOLT]);
}

#[test]
fn r608_2c_an_instruction_after_the_process_is_followed_once() {
    let grower = CardDef::new("Grower", "{0}", "Creature — Giant", &format!("{UPKEEP} You gain 1 life."), Some((1, 1))).unwrap();
    for library in [&[FOREST, MOUNTAIN, FOREST, BOLT][..], &[BOLT], &[]] {
        let mut g = game(defs(vec![grower.clone()]), library);
        g.put(0, MORE, Zone::Battlefield);
        to_upkeep_trigger(&mut g);
        resolve_top(&mut g);
        cite!(["608.2c"]; case: format!("{library:?}"); assert_eq!(g.players[0].life, 21, "{library:?}"));
        assert_eq!(graveyard(&g).len(), library.iter().take_while(|c| **c != BOLT).count());
    }
}

/// Text that names what one time through made, after the process, has no one card to name;
/// processes that could go on forever, or that do something else each time, are refused.
#[test]
fn r608_2c_repetitions_not_known_to_stop_or_naming_one_time_after_are_refused() {
    let refused = |text: String, reason: &str| {
        let def = CardDef::new("P", "{0}", "Creature — Giant", &text, Some((1, 1))).unwrap();
        let error = game::play::check(&def).expect_err(&text).0;
        assert!(error.contains(reason), "{text}: {error}");
    };
    refused(format!("{UPKEEP} You gain life equal to its mana value."), "not known after it");
    let process = "only a revealed top card put into a graveyard while it is as described";
    for (said, reason) in [
        ("reveal the top card of your library and put it into your graveyard. Repeat this process.", "at the end of one condition's block"),
        ("reveal the top card of your library. If it isn't a land card, put it into your graveyard and repeat this process.", process),
        ("reveal the top card of your library. If you control a Forest, put it into your graveyard and repeat this process.", process),
        ("reveal the top card of your library. If it's a land card, put it on the bottom of your library and repeat this process.", process),
        ("reveal the top card of your library. If it's a land card, put it into your hand and repeat this process.", process),
        ("reveal the top card of your library. If it's a land card, you may put it into your graveyard. If you do, repeat this process.", process),
        ("you gain 1 life. Reveal the top card of your library. If it's a land card, put it into your graveyard and repeat this process.", process),
        ("reveal the top card of your library. If it's a land card, put it into your graveyard and repeat this process once.", "a number of times"),
    ] {
        refused(format!("At the beginning of your upkeep, {said}"), reason);
    }
    for (name, reason) in [
        ("Primal Surge", process), ("Cultivator Colossus", process),
        ("Claim Jumper", "a number of times"), ("Zimone and Dina", "a number of times"), ("Ad Nauseam", "a number of times"),
    ] {
        let error = game::play::check(&pinned(name)).expect_err(name).0;
        assert!(error.contains(reason), "{name}: {error}");
    }
}
