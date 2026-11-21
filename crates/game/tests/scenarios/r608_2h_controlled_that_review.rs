//! The other "If you controlled that ..." cards (608.2h). Gleeful Demolition
//! asks of a destroyed artifact; Kellan's "up to one target" may have none, and then no one
//! controlled "that permanent".
use crate::support::{cite, paid};
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const DEMOLITION: usize = 0;
const ROCK: usize = 1;
const KELLAN: usize = 2;
const ISLAND: usize = 3;

fn setup() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let defs = vec![
        CardDef::from_face(faces.iter().find(|f| f.face_name == "Gleeful Demolition").unwrap()).unwrap(),
        CardDef::new("Rock", "{0}", "Artifact", "", None).unwrap(),
        // Kellan, Inquisitive Prodigy's creature face (its adventure is not supported)
        CardDef::new("Kellan, Inquisitive Prodigy", "{0}", "Legendary Creature — Faerie Detective", "Flying, vigilance\nWhenever Kellan attacks, destroy up to one target artifact. If you controlled that permanent, draw a card.", Some((3, 4))).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {e:?}", d.name())); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn goblins(g: &Game) -> usize { g.battlefield.iter().filter(|o| g.obj(**o).card.is_none() && g.controller(**o) == 0).count() }
fn demolish(g: &mut Game, at: ObjId) {
    let card = g.put(0, DEMOLITION, Zone::Hand);
    g.players[0].pool.colors = [9; 5];
    g.act(0, paid("R", Move::Cast { card, targets: vec![vec![Target::Object(at)]] })).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
}

#[test]
fn gleeful_demolition_of_your_own_artifact_makes_three_goblins() {
    let mut g = setup();
    let rock = g.put(0, ROCK, Zone::Battlefield);
    demolish(&mut g, rock);
    cite!(["608.2h"]; assert_eq!(goblins(&g), 3));
}

#[test]
fn gleeful_demolition_of_an_opponents_artifact_makes_none() {
    let mut g = setup();
    let rock = g.put(1, ROCK, Zone::Battlefield);
    demolish(&mut g, rock);
    assert_eq!(g.obj(rock).zone, Zone::Gone);
    cite!(["608.2h"]; assert_eq!(goblins(&g), 0));
}

/// "destroy up to one target artifact. If you controlled that permanent, draw a card": with
/// no target chosen, nothing was controlled.
#[test]
fn kellan_with_no_target_draws_nothing() {
    for aim in [false, true] {
        let mut g = setup();
        let kellan = g.put(0, KELLAN, Zone::Battlefield);
        let rock = g.put(0, ROCK, Zone::Battlefield);
        while g.part != Part::DeclareAttackers { g.act(g.priority, Move::Pass).unwrap(); }
        let hand = g.players[0].hand.len();
        g.act(0, Move::Attack(vec![(kellan, Target::Player(1))])).unwrap();
        let Some(Decision::Targets { .. }) = g.decision else { panic!("{:?}", g.decision) };
        g.act(0, Move::ChooseTargets(vec![if aim { vec![Target::Object(rock)] } else { vec![] }])).unwrap();
        while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
        assert_eq!(g.obj(rock).zone == Zone::Battlefield, !aim);
        cite!(["608.2h"]; case: format!("aim={aim}"); assert_eq!(g.players[0].hand.len(), hand + usize::from(aim), "aimed: {aim}"));
    }
}
