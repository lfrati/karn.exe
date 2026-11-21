//! Mayhem (702.187b).
//!
//! 702.187b: "Mayhem [cost]" means "As long as you discarded this card this turn, you may cast
//! it from your graveyard by paying [cost] rather than paying its mana cost."
//! Ruling (2025-09-19): "You must follow all normal timing rules when casting a spell with
//! mayhem. For example, you can't cast a creature card with mayhem you couldn't normally cast
//! during an opponent's turn, even if you discard that card during an opponent's turn."
//! 514.1: the active player discards down to their maximum hand size in the cleanup step;
//! 514.3a: a player gets priority in the cleanup step only if a state-based action is performed
//! or an ability triggers then.
//!
//! Cards read from their Oracle text: Electro's Bolt (sorcery, mayhem {1}{R}), Sadistic Slash
//! (instant, -5/-5, mayhem {1}{B}), Swarm, Being of Bees (flash creature, mayhem {B}),
//! Spider-Islanders (creature, mayhem {1}{R}).
use crate::support::cite;
use game::announcement::CastCost;
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const NAMES: [&str; 7] = ["Swamp", "Electro's Bolt", "Sadistic Slash", "Swarm, Being of Bees", "Spider-Islanders", "Grizzly Bears", "Sandman's Quicksand"];
const SWAMP: usize = 0;
const BOLT: usize = 1;
const SLASH: usize = 2;
const SWARM: usize = 3;
const ISLANDERS: usize = 4;
const BEARS: usize = 5;
const QUICKSAND: usize = 6;
const OUTLET: usize = 7;
const MIND_ROT: usize = 8;
const WATCHER: usize = 9;
const FORK: usize = 10;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let mut defs: Vec<_> = NAMES.map(|n| CardDef::from_faces(&catalog[n]).unwrap()).to_vec();
    defs.push(CardDef::new("Outlet", "{0}", "Artifact", "Discard a card: You gain 1 life.", None).unwrap());
    defs.push(CardDef::new("Quick Rot", "{0}", "Instant", "Target player discards a card.", None).unwrap());
    defs.push(CardDef::new("Watcher", "{0}", "Enchantment", "Whenever you discard a card, you gain 1 life.", None).unwrap());
    defs.push(CardDef::new("Fork", "{0}", "Instant", "Copy target spell.", None).unwrap());
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, SWAMP, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}

fn mayhem(g: &mut Game, card: ObjId, targets: Vec<Vec<Target>>, spending: &str) -> Result<(), String> {
    let row = match g.obj(card).card { Some(BOLT) => 1, Some(SWARM) => 2, Some(QUICKSAND) => 0, Some(SLASH) => 1, Some(ISLANDERS) => 0, other => panic!("no authored Mayhem row for {other:?}") };
    g.act(0, crate::support::paid(spending,Move::CastFor(CastCost::Alternative(row), Box::new(Move::Cast { card, targets }))))
}
fn mana(g: &mut Game, colorless: u32, red: u32, black: u32) { let p = &mut g.players[0].pool; p.colorless = colorless; p.colors[3] = red; p.colors[2] = black; }
fn in_graveyard(g: &Game, def: usize) -> ObjId { *g.players[0].graveyard.iter().find(|c| g.obj(**c).card == Some(def)).unwrap() }
/// Authored empty combat progression; callers must supply any discard themselves.
fn step(g: &mut Game) {
    match g.decision.clone() {
        Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
        Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
        Some(d) => panic!("unexpected decision {d:?}"),
        None => g.act(g.priority, Move::Pass).unwrap(),
    }
}
fn resolve(g: &mut Game) { while !g.stack.is_empty() { step(g); } }

// Rules: 702.187b, 701.9a
// Given: in player 0's main phase, player 1 casts an instant: "Target player discards a card."
// targeting player 0, whose only card in hand is Electro's Bolt.
// Expect: player 0 discarded it, though an opponent's effect made them; Electro's Bolt is cast
// for its mayhem cost {1}{R} in player 0's main phase with the stack empty.
// Why: the implementer's tests discard only with their own cost; "you discarded" is the player
// who discards (701.9a), whoever controls the effect.
#[test]
fn r702_187b_a_discard_an_opponents_effect_made_is_yours() {
    let mut g = game();
    let bears = g.put(1, BEARS, Zone::Battlefield);
    let held = g.put(0, BOLT, Zone::Hand);
    let rot = g.put(1, MIND_ROT, Zone::Hand);
    g.act(0, Move::Pass).unwrap();
    assert_eq!(g.priority, 1);
    g.act(1, Move::Cast { card: rot, targets: vec![vec![Target::Player(0)]] }).unwrap();
    resolve(&mut g);
    assert_eq!(g.obj(held).zone, Zone::Gone);
    let bolt = in_graveyard(&g, BOLT);
    assert_eq!((g.part, g.priority, g.active), (Part::PrecombatMain, 0, 0));
    mana(&mut g, 1, 1, 0);
    cite!(["702.187b"]; ok: mayhem(&mut g, bolt, vec![vec![Target::Object(bears)]], "CR"));
    resolve(&mut g);
    cite!(["702.187b"]; assert_eq!(g.obj(bears).zone, Zone::Gone, "4 damage"));
}

// Rules: 702.187b, 117.1a, 702.8a
// Given: on player 1's turn, player 0 discards Sadistic Slash (an instant) and Spider-Islanders
// (a creature without flash) with an instant-speed cost.
// Expect: Sadistic Slash is cast for its mayhem cost on that turn; Spider-Islanders is not
// (normal timing still applies, the 2025-09-19 ruling); on player 0's own turn after, neither,
// discarded in another turn.
#[test]
fn r702_187b_on_an_opponents_turn_an_instant_but_not_a_creature() {
    let mut g = game();
    let outlet = g.put(0, OUTLET, Zone::Battlefield);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    let slash = g.put(0, SLASH, Zone::Hand);
    let islanders = g.put(0, ISLANDERS, Zone::Hand);
    while !(g.active == 1 && g.part == Part::PrecombatMain && g.priority == 0) { step(&mut g); }
    for card in [slash, islanders] {
        g.act(0, Move::Activate { source: outlet, ability: 0, targets: vec![], paying: vec![vec![card]] }).unwrap();
        resolve(&mut g);
        while g.priority != 0 { step(&mut g); }
    }
    assert_eq!(g.active, 1);
    let (slash, islanders) = (in_graveyard(&g, SLASH), in_graveyard(&g, ISLANDERS));
    mana(&mut g, 1, 1, 1);
    cite!(["702.187b", "117.1a"]; refused("that is cast in a main phase of your own turn with the stack empty"): unchanged(&mut g, |g| mayhem(g, islanders, vec![], "CR")));
    mana(&mut g, 1, 1, 1);
    let mut later = g.clone();
    mana(&mut g, 1, 0, 1);
    cite!(["702.187b"]; ok: mayhem(&mut g, slash, vec![vec![Target::Object(bears)]], "CB"));
    cite!(["702.187b"]; assert_eq!(g.players[0].pool.total(), 0, "{{1}}{{B}}"));
    resolve(&mut g);
    cite!(["702.187b"]; assert_eq!(g.obj(bears).zone, Zone::Gone));
    // the next turn, player 0's own: discarded in another turn
    while !(later.active == 0 && later.part == Part::PrecombatMain && later.stack.is_empty() && later.decision.is_none()) { step(&mut later); }
    mana(&mut later, 1, 1, 1);
    cite!(["702.187b"]; refused("that card is not in your hand: it is in a graveyard, and no rule or effect lets you cast it from there"): unchanged(&mut later, |g| mayhem(g, islanders, vec![], "CR")));
    cite!(["702.187b"]; refused("that card is not in your hand: it is in a graveyard, and no rule or effect lets you cast it from there"): unchanged(&mut later, |g| mayhem(g, slash, vec![vec![Target::Object(bears)]], "CB")));
}

// Rules: 702.187b, 514.1, 514.3a
// Given: player 0 ends their turn with nine cards in hand, among them Swarm, Being of Bees
// (flash) and Sadistic Slash, and discards both to the maximum hand size in the cleanup step.
// A permanent triggers on the discard ("Whenever you discard a card, you gain 1 life").
// Expect: the trigger gives priority in that cleanup step (514.3a), and Swarm is cast for its
// mayhem cost then. Without the trigger no player gets priority, and in the next turn (an
// opponent's) Sadistic Slash, an instant, is not offered: discarded in another turn.
#[test]
fn r702_187b_a_cleanup_discard_is_cast_only_if_a_player_gets_priority_then() {
    for watched in [true, false] {
        let mut g = game();
        let watcher = watched.then(|| g.put(0, WATCHER, Zone::Battlefield));
        let bears=g.put(1, BEARS, Zone::Battlefield);
        let swarm = g.put(0, SWARM, Zone::Hand);
        let slash = g.put(0, SLASH, Zone::Hand);
        for _ in 0..7 { g.put(0, SWAMP, Zone::Hand); }
        // the hand put Swarm and Sadistic Slash first: the cleanup discard takes them
        assert_eq!(&g.players[0].hand[..2], &[swarm, slash]);
        while g.part != Part::Cleanup { step(&mut g); }
        assert_eq!(g.decision, Some(Decision::Discard { player: 0, count: 2 }));
        g.act(0, Move::Discard(vec![swarm, slash])).unwrap();
        // CR404.3: explicit owner order for these cost/departure identities.
        assert_eq!(g.decision,Some(game::state::Decision::Choose{player:0,choice:game::state::Choice::OrderCards{to:game::state::Placement::Zone(Zone::Graveyard),cards:vec![swarm,slash]}}));
        g.act(0,Move::Choose(game::state::Answer::Objects(vec![slash,swarm]))).unwrap();

        for _ in 0..50 {
            if g.active != 0 || (g.part == Part::Cleanup && g.decision.is_none() && !g.stack.is_empty()) { break; }
            if watched && matches!(g.decision, Some(Decision::Order { .. })) { break; }
            step(&mut g);
        }
        if watched {
            // Both cleanup discards trigger the same Watcher, but its controller
            // orders the two occurrences before cleanup priority (603.3b/514.3a).
            let Some(Decision::Order { player: 0, abilities }) = g.decision.clone() else { panic!("expected cleanup trigger order: {:?}", g.decision) };
            cite!(["603.3b", "514.3a"]; assert_eq!(abilities.len(), 2));
            assert!(abilities.iter().all(|ability| ability.source == watcher && ability.controller == 0));
            g.act(0, Move::Order(vec![0, 1])).unwrap();
        }
        let (swarm, slash) = (in_graveyard(&g, SWARM), in_graveyard(&g, SLASH));
        if watched {
            cite!(["514.3a"]; case: "a trigger"; assert_eq!((g.part, g.active, g.priority), (Part::Cleanup, 0, 0)));
            assert_eq!(g.stack.len(), 2, "two discards, two triggers");
            mana(&mut g, 0, 0, 1);
            cite!(["702.187b", "514.3a"]; case: "a trigger"; ok: mayhem(&mut g, swarm, vec![], "B"));
            resolve(&mut g);
            cite!(["702.187b"]; case: "a trigger"; assert!(g.battlefield.iter().any(|o| g.obj(*o).card == Some(SWARM))));
        } else {
            cite!(["514.3a"]; case: "no trigger"; assert_eq!(g.active, 1, "no priority in the cleanup step"));
            while g.priority != 0 || g.decision.is_some() { step(&mut g); }
            mana(&mut g, 1, 0, 1);
            cite!(["702.187b"]; refused("that card is not in your hand: it is in a graveyard, and no rule or effect lets you cast it from there"): unchanged(&mut g, |g| mayhem(g, slash, vec![vec![Target::Object(bears)]], "CB")));
            cite!(["702.187b"]; refused("that card is not in your hand: it is in a graveyard, and no rule or effect lets you cast it from there"): unchanged(&mut g, |g| mayhem(g, swarm, vec![], "B")));
        }
    }
}

// Rules: 707.10, 702.187b, 601.2b
// Given: Sandman's Quicksand ("All creatures get -2/-2 until end of turn. If this spell's
// mayhem cost was paid, creatures your opponents control get -2/-2 until end of turn
// instead.") cast for its mayhem cost; Fork copies it.
// Expect: the copy copies the alternative cost paid (707.10): both give only the opponent's
// creatures -2/-2, so player 0's 2/2 Bears survive and player 1's die.
// Why: the same question as a copy of a warped spell: a copy has no cast record of its own.
#[test]
fn r707_10_a_copy_of_a_spell_cast_for_its_mayhem_cost_was_paid_for_too() {
    let mut g = game();
    let outlet = g.put(0, OUTLET, Zone::Battlefield);
    let mine = g.put(0, BEARS, Zone::Battlefield);
    let theirs = g.put(1, BEARS, Zone::Battlefield);
    let held = g.put(0, QUICKSAND, Zone::Hand);
    g.act(0, Move::Activate { source: outlet, ability: 0, targets: vec![], paying: vec![vec![held]] }).unwrap();
    resolve(&mut g);
    let quicksand = in_graveyard(&g, QUICKSAND);
    mana(&mut g, 3, 0, 1);
    mayhem(&mut g, quicksand, vec![], "CCCB").unwrap();
    let spell = g.stack[0];
    let fork = g.put(0, FORK, Zone::Hand);
    g.act(0, Move::Cast { card: fork, targets: vec![vec![Target::Object(spell)]] }).unwrap();
    // Fork, then the copy: player 0's Bears are 2/2 between the copy and the original
    for _ in 0..2 { g.act(0, Move::Pass).unwrap(); g.act(1, Move::Pass).unwrap(); }
    assert_eq!(g.stack, vec![spell], "the copy resolved");
    cite!(["707.10", "702.187b"]; assert_eq!(g.obj(mine).zone, Zone::Battlefield, "the copy's mayhem cost was paid"));
    resolve(&mut g);
    cite!(["707.10", "702.187b"]; assert_eq!((g.obj(mine).zone == Zone::Battlefield, g.obj(theirs).zone == Zone::Battlefield), (true, false)));
}

#[track_caller]
fn unchanged(g: &mut Game, action: impl FnOnce(&mut Game) -> Result<(), String>) -> Result<(), String> {
    let before=format!("{g:?}");
    let result=action(g);
    assert_eq!(format!("{g:?}"),before);
    result
}
