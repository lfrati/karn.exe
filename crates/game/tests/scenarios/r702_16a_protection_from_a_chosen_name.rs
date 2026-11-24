//! Protection from a card name (702.16a): Runed Halo, "As this enchantment enters, choose a
//! card name. You have protection from the chosen card name." Its controller can't be
//! targeted by a spell with that name (702.16b) and isn't dealt damage by a source with that
//! name (702.16e); other names are not stopped.
use crate::support::{cite, paid};
use game::binding::Value;
use game::select::Ctx;
use game::state::Answer;
use game::{CardDef, Game, Move, Target, Zone};
use ir::Part;

const HALO: usize = 0;
const BOLT: usize = 1;
const SHOCK: usize = 2;
const BEARS: usize = 3;
const GIANT: usize = 4;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let defs: Vec<_> = ["Runed Halo", "Lightning Bolt", "Shock", "Grizzly Bears", "Hill Giant", "Wastes"].iter().map(|n| {
        let d = CardDef::from_faces(&catalog[*n]).unwrap_or_else(|e| panic!("{n}: {e:?}"));
        game::play::check(&d).unwrap_or_else(|e| panic!("{n}: {}", e.0));
        d
    }).collect();
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, 5, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn mana(g: &mut Game, p: usize) { g.players[p].pool.colors = [9; 5]; g.players[p].pool.colorless = 9; }
fn settle(g: &mut Game) {
    for _ in 0..30 {
        if g.stack.is_empty() && g.decision.is_none() { return; }
        assert!(g.decision.is_none(), "{:?}", g.decision);
        g.act(g.priority, Move::Pass).unwrap();
    }
}
/// Player 0 casts Runed Halo and names `name` as it enters.
fn halo(g: &mut Game, name: &str) {
    let card = g.put(0, HALO, Zone::Hand);
    mana(g, 0);
    g.act(0, paid("WW", Move::Cast { card, targets: vec![] })).unwrap();
    while g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Choose(Answer::Value(Value::CardName(name.into())))).unwrap();
    settle(g);
}

/// Naming Lightning Bolt: an opponent's Lightning Bolt can't target the Halo's controller, and
/// their Shock can.
#[test]
fn r702_16b_runed_halo_stops_spells_with_the_chosen_name_from_targeting_its_controller() {
    let mut g = game();
    halo(&mut g, "Lightning Bolt");
    let (bolt, shock) = (g.put(1, BOLT, Zone::Hand), g.put(1, SHOCK, Zone::Hand));
    cite!(["702.16a", "702.16b"]; assert!(!g.can_target(&Ctx::new(1, bolt), Target::Player(0))));
    cite!(["702.16b"]; assert!(g.can_target(&Ctx::new(1, bolt), Target::Player(1)), "the opponent has no protection"));
    cite!(["702.16b"]; assert!(g.can_target(&Ctx::new(1, shock), Target::Player(0))));
    g.act(0, Move::Pass).unwrap();
    mana(&mut g, 1);
    cite!(["702.16a", "702.16b"]; refused("an illegal target, that player: it has protection from that (702.16b)"):
        g.act(1, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Player(0)]] })));
    g.act(1, paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Player(0)]] })).unwrap();
    settle(&mut g);
    cite!(["702.16b"]; assert_eq!(g.players[0].life, 18));
}

/// Naming Grizzly Bears: damage an opponent's Grizzly Bears would deal the Halo's controller is
/// prevented; their Hill Giant's is not.
#[test]
fn r702_16e_runed_halo_prevents_damage_from_sources_with_the_chosen_name() {
    let mut g = game();
    halo(&mut g, "Grizzly Bears");
    let (bears, giant) = (g.put(1, BEARS, Zone::Battlefield), g.put(1, GIANT, Zone::Battlefield));
    g.damage(bears, Target::Player(0), 2);
    cite!(["702.16a", "702.16e"]; assert_eq!(g.players[0].life, 20));
    g.damage(giant, Target::Player(0), 3);
    cite!(["702.16e"]; assert_eq!(g.players[0].life, 17));
}
