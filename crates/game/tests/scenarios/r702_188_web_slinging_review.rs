//! Web-slinging (702.188).
//!
//! 702.188a: "Web-slinging [cost]" means "You may cast this spell by paying [cost] and
//! returning a tapped creature you control to its owner's hand rather than paying its mana
//! cost."
use crate::support::cite;
use game::announcement::CastCost;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const PLAINS: usize = 0;
const SPIDER_MAN: usize = 1;
const BEARS: usize = 2;
const STEAL: usize = 3;
const ROCK: usize = 4;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let mut defs: Vec<_> = ["Plains", "Spider-Man, Web-Slinger", "Grizzly Bears"].map(|n| CardDef::from_faces(&catalog[n]).unwrap()).to_vec();
    defs.push(CardDef::new("Steal", "{0}", "Sorcery", "Gain control of target creature until end of turn.", None).unwrap());
    defs.push(CardDef::new("Rock", "{0}", "Artifact", "", None).unwrap());
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, PLAINS, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn sling(g: &mut Game, card: ObjId, returned: ObjId) -> Result<(), String> {
    g.act(0, crate::support::paid("W",Move::CastFor(CastCost::Alternative(0), Box::new(Move::WithCosts(vec![vec![returned]], Box::new(Move::Cast { card, targets: vec![] }))))))
}

// Rules: 702.188a, 108.3
// Given: player 0 controls a tapped Bears that player 1 owns, and a tapped noncreature artifact.
// When: player 0 casts Spider-Man for its web-slinging cost returning each.
// Expect: the artifact is refused (not a creature); the stolen Bears goes to player 1's hand.
// Why: the cost returns "a tapped creature you control to its owner's hand"; its owner is
// player 1 (108.3), not its controller.
#[test]
fn r702_188a_a_stolen_creature_returns_to_its_owners_hand() {
    let mut g = game();
    let spider = g.put(0, SPIDER_MAN, Zone::Hand);
    let theirs = g.put(1, BEARS, Zone::Battlefield);
    let rock = g.put(0, ROCK, Zone::Battlefield);
    let steal = g.put(0, STEAL, Zone::Hand);
    g.act(0, Move::Cast { card: steal, targets: vec![vec![Target::Object(theirs)]] }).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    assert_eq!(g.controller(theirs), 0, "setup: stolen");
    g.tap(theirs);
    g.tap(rock);
    g.players[0].pool.colors[0] = 1;
    cite!(["702.188a"]; refused("Rock is not what that cost asks for or cannot pay it"): unchanged(&mut g, |g| sling(g, spider, rock)));
    sling(&mut g, spider, theirs).unwrap();
    cite!(["702.188a", "108.3"]; assert_eq!(g.players[1].hand.iter().filter(|c| g.obj(**c).card == Some(BEARS)).count(), 1, "its owner's hand"));
    cite!(["702.188a"]; assert!(!g.players[0].hand.iter().any(|c| g.obj(*c).card == Some(BEARS))));
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    assert!(g.battlefield.iter().any(|o| g.obj(*o).card == Some(SPIDER_MAN)));
}

#[track_caller]
fn unchanged(g: &mut Game, action: impl FnOnce(&mut Game) -> Result<(), String>) -> Result<(), String> {
    let before=format!("{g:?}");
    let result=action(g);
    assert_eq!(format!("{g:?}"),before);
    result
}
