//! Mayhem (702.187).
//!
//! 702.187a: "Mayhem is a static ability that functions while the card with mayhem is in a
//! player's graveyard."
//!
//! 702.187b: "“Mayhem [cost]” means “As long as you discarded this card this turn, you may cast
//! it from your graveyard by paying [cost] rather than paying its mana cost.” Casting a spell
//! using its mayhem ability follows the rules for paying alternative costs in rules 601.2b and
//! 601.2f–h."
//!
//! 400.7: an object that moves from one zone to another becomes a new object with no memory
//! of its previous existence.
//!
//! The cards are read from their Oracle text, but for the discard outlet.
use crate::support::cite;
use game::announcement::CastCost;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const NAMES: [&str; 5] = ["Swamp", "Electro's Bolt", "Sandman's Quicksand", "Swarm, Being of Bees", "Grizzly Bears"];
const SWAMP: usize = 0;
const BOLT: usize = 1;
const QUICKSAND: usize = 2;
const SWARM: usize = 3;
const BEARS: usize = 4;
const OUTLET: usize = 5;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let mut defs: Vec<_> = NAMES.map(|n| CardDef::from_faces(&catalog[n]).unwrap()).to_vec();
    defs.push(CardDef::new("Outlet", "{0}", "Artifact", "Discard a card: You gain 1 life.", None).unwrap());
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, SWAMP, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
/// Player 0 discards this card with the outlet: the card it is in the graveyard (400.7).
fn discard(g: &mut Game, outlet: ObjId, card: ObjId) -> ObjId {
    let p = g.priority;
    g.act(p, Move::Activate { source: outlet, ability: 0, targets: vec![], paying: vec![vec![card]] }).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    *g.players[g.obj(outlet).controller].graveyard.last().unwrap()
}


fn mayhem(g: &mut Game, card: ObjId, targets: Vec<Vec<Target>>, spending: &str) -> Result<(), String> {
    // each of these cards' row of its mayhem keyword
    let row = match g.obj(card).card { Some(BOLT) => 1, Some(SWARM) => 2, Some(QUICKSAND) => 0, other => panic!("no authored Mayhem row for {other:?}") };
    g.act(0, crate::support::paid(spending,Move::CastFor(CastCost::Alternative(row), Box::new(Move::Cast { card, targets }))))
}
fn mana(g: &mut Game, colorless: u32, red: u32, black: u32) { let p = &mut g.players[0].pool; p.colorless = colorless; p.colors[3] = red; p.colors[2] = black; }

#[test]
fn r702_187b_cast_from_the_graveyard_for_its_cost_only_after_discarding_it() {
    // Electro's Bolt {2}{R}, a sorcery: "Electro's Bolt deals 4 damage to target creature.",
    // "Mayhem {1}{R}"
    let mut g = game();
    let outlet = g.put(0, OUTLET, Zone::Battlefield);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    let target = vec![vec![Target::Object(bears)]];
    // put there, not discarded
    let put = g.put(0, BOLT, Zone::Graveyard);
    mana(&mut g, 1, 1, 0);
    cite!(["702.187b"]; refused("that card is not in your hand: it is in a graveyard, and no rule or effect lets you cast it from there"): unchanged(&mut g, |g| mayhem(g, put, target.clone(), "CR")));
    // from a hand, the mayhem cost is not offered nor taken
    let held = g.put(0, BOLT, Zone::Hand);
    cite!(["702.187b"]; refused("a mayhem cost is paid only for a card cast from a graveyard"): unchanged(&mut g, |g| mayhem(g, held, target.clone(), "CR")));
    let bolt = discard(&mut g, outlet, held);
    assert_eq!(g.obj(bolt).zone, Zone::Graveyard);
    mana(&mut g, 2, 1, 0);
    cite!(["702.187b"]; refused("that card is not in your hand: it is in a graveyard"): unchanged(&mut g, |g| g.act(0, crate::support::paid("CCR", Move::Cast { card: bolt, targets: target.clone() }))));
    mana(&mut g, 1, 1, 0);
    mayhem(&mut g, bolt, target, "CR").unwrap();
    cite!(["702.187b"]; assert_eq!(g.players[0].pool.total(), 0, "{{1}}{{R}}"));
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["702.187b"]; assert_eq!(g.obj(bears).zone, Zone::Gone));
    let again = *g.players[0].graveyard.iter().find(|c| g.obj(**c).card == Some(BOLT) && **c != put).unwrap();
    let live_target = g.put(1, BEARS, Zone::Battlefield);
    mana(&mut g, 1, 1, 0);
    cite!(["702.187b", "400.7"]; refused("that card is not in your hand: it is in a graveyard, and no rule or effect lets you cast it from there"): unchanged(&mut g, |g| mayhem(g, again, vec![vec![Target::Object(live_target)]], "CR")));
}

#[test]
fn r702_187b_only_this_turn_and_at_the_cards_timing() {
    let mut g = game();
    let outlet = g.put(0, OUTLET, Zone::Battlefield);
    let bears=g.put(1, BEARS, Zone::Battlefield);
    let held = g.put(0, BOLT, Zone::Hand);
    // Discarded during the end step: no sorcery timing, then permission expires.
    while g.part != Part::End { g.act(g.priority, Move::Pass).unwrap(); if let Some(game::state::Decision::Attackers { player }) = g.decision { g.act(player, Move::Attack(vec![])).unwrap(); } }
    let bolt = discard(&mut g, outlet, held);
    mana(&mut g, 1, 1, 0);
    cite!(["702.187b"]; refused("that is cast in a main phase of your own turn with the stack empty"): unchanged(&mut g, |g| mayhem(g, bolt, vec![vec![Target::Object(bears)]], "CR")));
    // the next turn: player 1's, then player 0's again
    while !(g.active == 0 && g.part == Part::PrecombatMain) {
        match g.decision.clone() {
            Some(game::state::Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(other) => panic!("unexpected turn-progression choice: {other:?}"),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    mana(&mut g, 1, 1, 0);
    cite!(["702.187b"]; refused("that card is not in your hand: it is in a graveyard, and no rule or effect lets you cast it from there"): unchanged(&mut g, |g| mayhem(g, bolt, vec![vec![Target::Object(bears)]], "CR")));
}

#[test]
fn r702_187b_a_permanent_card_with_flash_at_an_instants_timing() {
    // Swarm, Being of Bees {2}{B}: "Flash", "Flying", "Mayhem {B}"
    let mut g = game();
    let outlet = g.put(0, OUTLET, Zone::Battlefield);
    let held = g.put(0, SWARM, Zone::Hand);
    // discarded and cast in the end step
    while g.part != Part::End { g.act(g.priority, Move::Pass).unwrap(); if let Some(game::state::Decision::Attackers { player }) = g.decision { g.act(player, Move::Attack(vec![])).unwrap(); } }
    let swarm = discard(&mut g, outlet, held);
    mana(&mut g, 0, 0, 1);
    mayhem(&mut g, swarm, vec![], "B").unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["702.187b"]; assert!(g.battlefield.iter().any(|o| g.obj(*o).card == Some(SWARM))));
}

#[test]
fn r702_187_if_this_spells_mayhem_cost_was_paid() {
    // Sandman's Quicksand {1}{B}{B}: "Mayhem {3}{B}", "All creatures get -2/-2 until end of turn.
    // If this spell's mayhem cost was paid, creatures your opponents control get -2/-2 until end
    // of turn instead."
    for paid in [false, true] {
        let mut g = game();
        let outlet = g.put(0, OUTLET, Zone::Battlefield);
        let mine = g.put(0, BEARS, Zone::Battlefield);
        let theirs = g.put(1, BEARS, Zone::Battlefield);
        let held = g.put(0, QUICKSAND, Zone::Hand);
        if paid {
            let quicksand = discard(&mut g, outlet, held);
            mana(&mut g, 3, 0, 1);
            mayhem(&mut g, quicksand, vec![], "CCCB").unwrap();
        } else {
            mana(&mut g, 1, 0, 2);
            g.act(0, crate::support::paid("CBB",Move::Cast { card: held, targets: vec![] })).unwrap();
        }
        while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
        cite!(["702.187b", "601.2b"]; case: paid; assert_eq!((g.obj(mine).zone == Zone::Battlefield, g.obj(theirs).zone == Zone::Battlefield), (paid, false)));
    }
}

#[track_caller]
fn unchanged(g: &mut Game, action: impl FnOnce(&mut Game) -> Result<(), String>) -> Result<(), String> {
    let before=format!("{g:?}");
    let result=action(g);
    assert_eq!(format!("{g:?}"),before);
    result
}
