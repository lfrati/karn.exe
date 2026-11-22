//! Explore (701.44): reveal the top card; a land card goes to its player's hand, otherwise
//! the exploring permanent gets a +1/+1 counter and the card may go to the graveyard. It has
//! explored even where some of that couldn't be done (701.44b), uses last known information
//! once gone (701.44c), and several explore one at a time, each controller choosing the order
//! of their own (701.44d).
use crate::support::cite;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};

const PROBE: usize = 0;
const BEAR: usize = 1;
const LAND: usize = 2;
const SPELL: usize = 3;
const WALKER: usize = 4;
const ALL: usize = 5;
const BOUNCE: usize = 6;
const SCOUT: usize = 7;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap();
    let c = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).expect(name);
    let defs = vec![
        c("Probe", "Instant", "Target creature you control explores.", None),
        c("Bear", "Creature — Bear", "", Some((2, 2))),
        c("Land", "Basic Land — Forest", "", None),
        c("Spell", "Instant", "You gain 1 life.", None),
        real("Wildgrowth Walker"),
        c("Expedition", "Sorcery", "Each creature you control explores.", None),
        c("Bounce", "Instant", "Return target creature to its owner's hand.", None),
        c("Scout", "Creature — Scout", "When this creature enters, it explores.\nWhenever this creature explores, you gain 2 life.", Some((1, 1))),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    Game::new(defs, 2, 20)
}
fn pass(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }
fn cast(g: &mut Game, card: usize, targets: Vec<Vec<Target>>) {
    let spell = g.put(0, card, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets }).unwrap();
}
fn explored(g: &Game) -> usize { g.log.iter().filter(|e| matches!(e, Event::Explored { .. })).count() }
fn plus(g: &Game, o: ObjId) -> u32 { g.obj(o).counters.get("+1/+1").copied().unwrap_or(0) }

#[test]
fn r701_44a_a_land_goes_to_hand_without_a_counter() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let land = g.put(0, LAND, Zone::Library);
    cast(&mut g, PROBE, vec![vec![Target::Object(bear)]]);
    pass(&mut g);
    cite!(["701.44a"]; assert!(g.players[0].hand.iter().any(|o| g.obj(*o).card == Some(LAND)), "{land:?}"));
    cite!(["701.44a"]; assert_eq!((plus(&g, bear), explored(&g)), (0, 1)));
    cite!(["701.44a"]; assert!(g.log.iter().any(|e| matches!(e, Event::Revealed { player: 0, .. }))));
}

#[test]
fn r701_44a_a_nonland_gives_a_counter_and_may_be_milled() {
    for mill in [false, true] {
        let mut g = game();
        let bear = g.put(0, BEAR, Zone::Battlefield);
        let card = g.put(0, SPELL, Zone::Library);
        cast(&mut g, PROBE, vec![vec![Target::Object(bear)]]);
        pass(&mut g);
        assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May { possible: true } })), "{:?}", g.decision);
        cite!(["701.44a"]; refused("that decision is another player's"): g.act(1, Move::Choose(Answer::Yes)));
        g.act(0, Move::Choose(if mill { Answer::Yes } else { Answer::No })).unwrap();
        cite!(["701.44a"]; case: format!("mill={mill}"); assert_eq!(plus(&g, bear), 1));
        cite!(["701.44a"]; case: format!("mill={mill}"); assert_eq!(g.players[0].library.last() == Some(&card), !mill));
        cite!(["701.44a"]; case: format!("mill={mill}"); assert_eq!(g.players[0].graveyard.iter().any(|o| g.obj(*o).card == Some(SPELL)), mill));
        assert_eq!(explored(&g), 1);
    }
}

#[test]
fn r701_44b_an_empty_library_still_explores_with_a_counter() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    g.put(0, WALKER, Zone::Battlefield);
    cast(&mut g, PROBE, vec![vec![Target::Object(bear)]]);
    pass(&mut g);
    cite!(["701.44b"]; assert_eq!((plus(&g, bear), explored(&g)), (1, 1)));
    assert!(g.decision.is_none());
    // "whenever a creature you control explores" sees it
    cite!(["701.44b", "603.2"]; assert_eq!(g.stack.len(), 1));
    pass(&mut g);
    assert_eq!(g.players[0].life, 23);
}

#[test]
fn r701_44c_a_permanent_gone_still_explores_and_its_own_trigger_sees_it() {
    let mut g = game();
    g.put(0, SPELL, Zone::Library);
    let card = g.put(0, SCOUT, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    pass(&mut g);
    let scout = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(SCOUT)).unwrap();
    // its enters trigger waits; it is returned to hand before the trigger resolves
    assert_eq!(g.stack.len(), 1);
    cast(&mut g, BOUNCE, vec![vec![Target::Object(scout)]]);
    pass(&mut g);
    assert!(!g.battlefield.contains(&scout));
    pass(&mut g);
    // it explored as it last existed: the card was revealed, no counter could be put
    cite!(["701.44c"]; assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May { possible: true } })), "{:?}", g.decision));
    g.act(0, Move::Choose(Answer::No)).unwrap();
    assert_eq!(explored(&g), 1);
    cite!(["701.44c"]; assert!(g.log.iter().any(|e| matches!(e, Event::Explored { object, player: 0 } if *object == scout))));
    // "whenever this creature explores" triggers from where it left
    assert_eq!(g.stack.len(), 1);
    pass(&mut g);
    assert_eq!(g.players[0].life, 22);
}

#[test]
fn r701_44d_several_explore_one_at_a_time_in_the_order_chosen() {
    let mut g = game();
    let (a, b) = (g.put(0, BEAR, Zone::Battlefield), g.put(0, BEAR, Zone::Battlefield));
    g.put(0, LAND, Zone::Library);
    g.put(0, SPELL, Zone::Library);
    cast(&mut g, ALL, vec![]);
    pass(&mut g);
    let Some(Decision::Choose { player: 0, choice: Choice::Objects { verb: ir::Verb::Explore, among, count: 1, .. } }) = g.decision.clone() else { panic!("{:?}", g.decision) };
    cite!(["701.44d"]; assert_eq!(among.len(), 2));
    // b first: it reveals the Spell (the top card), a then the Land beneath
    g.act(0, Move::Choose(Answer::Objects(vec![b]))).unwrap();
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    cite!(["701.44d"]; assert_eq!((plus(&g, a), plus(&g, b)), (0, 1)));
    cite!(["701.44d"]; assert!(g.players[0].hand.iter().any(|o| g.obj(*o).card == Some(LAND))));
    assert_eq!(explored(&g), 2);
}
