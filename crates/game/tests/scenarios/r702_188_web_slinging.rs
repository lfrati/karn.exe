//! Web-slinging (702.188).
//!
//! 702.188a: "Web-slinging is a static ability that functions while the spell with
//! web-slinging is on the stack. “Web-slinging [cost]” means “You may cast this spell by paying
//! [cost] and returning a tapped creature you control to its owner's hand rather than paying
//! its mana cost.” Casting a spell using its web-slinging ability follows the rules for paying
//! alternative costs in rules 601.2b and 601.2f–h."
//!
//! Every card is read from its Oracle text.
use crate::support::cite;
use game::announcement::CastCost;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const NAMES: [&str; 5] = ["Plains", "Spider-Man, Web-Slinger", "Spider-Sense", "Grizzly Bears", "Shock"];
const PLAINS: usize = 0;
const SPIDER_MAN: usize = 1;
const SENSE: usize = 2;
const BEARS: usize = 3;
const SHOCK: usize = 4;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let defs: Vec<_> = NAMES.map(|n| CardDef::from_faces(&catalog[n]).unwrap()).to_vec();
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, PLAINS, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
/// The listed web-slinging casts of this card: the creatures each may return.

fn sling(g: &mut Game, player: usize, card: ObjId, returned: ObjId, targets: Vec<Vec<Target>>, spending: &str) -> Result<(), String> {
    // each of these cards' first row is its web-slinging keyword
    g.act(player, crate::support::paid(spending,Move::CastFor(CastCost::Alternative(0), Box::new(Move::WithCosts(vec![vec![returned]], Box::new(Move::Cast { card, targets }))))))
}
fn white(g: &mut Game, n: u32) { g.players[0].pool.colors[0] = n; }

#[test]
fn r702_188a_cast_for_its_cost_returning_a_tapped_creature_you_control() {
    // Spider-Man, Web-Slinger {2}{W}: "Web-slinging {W}"
    let mut g = game();
    let spider = g.put(0, SPIDER_MAN, Zone::Hand);
    let untapped = g.put(0, BEARS, Zone::Battlefield);
    let theirs = g.put(1, BEARS, Zone::Battlefield);
    g.tap(theirs);
    white(&mut g, 1);
    cite!(["702.188a"]; refused("Grizzly Bears is not what that cost asks for or cannot pay it"): unchanged(&mut g, |g| sling(g, 0, spider, untapped, vec![], "W")));
    cite!(["702.188a"]; refused("Grizzly Bears is not what that cost asks for or cannot pay it"): unchanged(&mut g, |g| sling(g, 0, spider, theirs, vec![], "W")));
    let tapped = g.put(0, BEARS, Zone::Battlefield);
    g.tap(tapped);
    sling(&mut g, 0, spider, tapped, vec![], "W").unwrap();
    cite!(["702.188a"]; assert_eq!(g.obj(tapped).zone, Zone::Gone, "returned to its owner's hand"));
    assert_eq!(g.players[0].hand.iter().filter(|c| g.obj(**c).card == Some(BEARS)).count(), 1);
    assert_eq!(g.players[0].pool.total(), 0, "{{W}} paid, not {{2}}{{W}}");
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["702.188a"]; assert!(g.battlefield.iter().any(|o| g.obj(*o).card == Some(SPIDER_MAN))));
}

#[test]
fn r702_188a_the_spells_timing_is_its_own() {
    // a creature spell cast for its web-slinging cost is still cast at a sorcery's timing (307.1);
    // Spider-Sense, an instant: "Web-slinging {U}", "Counter target instant spell, sorcery
    // spell, or triggered ability."
    let mut g = game();
    let spider = g.put(0, SPIDER_MAN, Zone::Hand);
    let sense = g.put(0, SENSE, Zone::Hand);
    let tapped = g.put(0, BEARS, Zone::Battlefield);
    g.tap(tapped);
    let shock = g.put(1, SHOCK, Zone::Hand);
    // player 1 casts Shock at the tapped Bears in player 0's main phase
    g.act(0, Move::Pass).unwrap();
    g.players[1].pool.colors[3] = 1;
    g.act(1, crate::support::paid("R",Move::Cast { card: shock, targets: vec![vec![Target::Object(tapped)]] })).unwrap();
    g.act(1, Move::Pass).unwrap();
    assert_eq!(g.priority, 0);
    white(&mut g, 1);
    cite!(["702.188a", "307.1"]; refused("that is cast in a main phase of your own turn with the stack empty"): unchanged(&mut g, |g| sling(g, 0, spider, tapped, vec![], "W")));
    g.players[0].pool.colors[1] = 1;
    let shock_on_stack = *g.stack.last().unwrap();
    sling(&mut g, 0, sense, tapped, vec![vec![Target::Object(shock_on_stack)]], "U").unwrap();
    cite!(["702.188a"]; assert_eq!(g.obj(tapped).zone, Zone::Gone, "the Bears saved by returning them"));
    // Resolve only Spider-Sense. Shock must leave now, before it could resolve
    // with the returned Bear as an illegal target (608.2b).
    assert_eq!(g.stack.len(), 2);
    assert!(g.decision.is_none());
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    cite!(["701.6a", "702.188a"]; assert!(g.stack.is_empty(), "Spider-Sense countered the authored Shock"));
    assert_eq!(g.obj(shock_on_stack).zone, Zone::Gone);
    cite!(["701.6a"]; assert!(g.players[1].graveyard.iter().any(|c| g.obj(*c).card == Some(SHOCK)), "Shock countered"));
}

#[track_caller]
fn unchanged(g: &mut Game, action: impl FnOnce(&mut Game) -> Result<(), String>) -> Result<(), String> {
    let before=format!("{g:?}");
    let result=action(g);
    assert_eq!(format!("{g:?}"),before);
    result
}
