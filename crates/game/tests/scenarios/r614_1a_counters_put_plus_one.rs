//! Winding Constrictor: "If one or more counters would be put on an artifact or creature you
//! control, that many plus one of each of those kinds of counters are put on that permanent
//! instead. If you would get one or more counters, you get that many plus one of each of those
//! kinds of counters instead." A replacement effect of the counters an event would put (614.1a),
//! on a permanent or on a player, who can have counters too (122.1). It is of what its
//! controller controls and of its controller alone.
use crate::support;
use support::*;

/// Pass until the stack is empty.
fn resolve(t: &mut T) {
    for _ in 0..8 {
        if t.g.stack.is_empty() { return; }
        t.pass_all();
    }
}

fn plus_one(t: &T, o: ObjId) -> u32 { t.g.obj(o).counters.get("+1/+1").copied().unwrap_or(0) }

fn energy(t: &T, p: usize) -> u32 { t.g.players[p].counters.get("energy").copied().unwrap_or(0) }

// Rules: 614.1a, 122.6
// Given: player 0 controls Winding Constrictor; each player controls a Grizzly Bears.
// When: player 0 casts Battlegrowth ("Put a +1/+1 counter on target creature.") on their own
// Bears, or on player 1's.
// Expect: their own Bears gets two +1/+1 counters; player 1's gets one.
#[test]
fn r614_1a_winding_constrictor_puts_one_more_counter_on_a_creature_you_control() {
    for (on, expected) in [(0, 2), (1, 1)] {
        let mut t = Board::new(2).battlefield(0, &["Winding Constrictor", "Grizzly Bears"]).battlefield(1, &["Grizzly Bears"]).hand(0, &["Battlegrowth"]).build();
        let bears = t.find_in(on, Zone::Battlefield, "Grizzly Bears");
        t.mana(0, "{G}").cast(0, "Battlegrowth", vec![Tgt::Object(bears)], mana_pool("G")).unwrap();
        resolve(&mut t);
        cite!(["614.1a", "122.6"]; case: format!("player {on}'s Bears"); assert_eq!(plus_one(&t, bears), expected));
    }
}

// Rules: 614.1a, 122.1
// Given: Winding Constrictor under player 0's control, under player 1's, or none.
// When: player 0 plays Aether Hub ("When Aether Hub enters, you get {E}.") and its ability
// resolves.
// Expect: player 0 gets two energy counters with their own Constrictor, and one otherwise.
#[test]
fn r614_1a_winding_constrictor_gives_you_one_more_counter() {
    for (controller, expected) in [(Some(0), 2), (Some(1), 1), (None, 1)] {
        let mut board = Board::new(2).hand(0, &["Aether Hub"]);
        if let Some(p) = controller { board = board.battlefield(p, &["Winding Constrictor"]); }
        let mut t = board.build();
        let hub = t.find_in(0, Zone::Hand, "Aether Hub");
        t.g.act(0, Move::PlayLand(hub)).unwrap();
        resolve(&mut t);
        cite!(["614.1a", "122.1"]; case: format!("{controller:?}"); assert_eq!((energy(&t, 0), energy(&t, 1)), (expected, 0)));
    }
}
