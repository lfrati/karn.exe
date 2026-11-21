//! Elusive Otter: "Creatures with power less than this creature's power can't block it."
//! The restriction names the Otter as what can't be blocked, and compares the blocker's power
//! with the Otter's as each is when blockers are declared (509.1b).
use crate::support::cite;
use game::{CardDef, Game, Move, Target, Zone};

fn game() -> Game {
    let defs = vec![
        CardDef::new("Otter", "{U}", "Creature — Otter", "Creatures with power less than this creature's power can't block it.", Some((1, 1))).unwrap(),
        CardDef::new("Wall", "{0}", "Creature — Wall", "", Some((0, 4))).unwrap(),
        CardDef::new("Squire", "{0}", "Creature — Human", "", Some((1, 2))).unwrap(),
        CardDef::new("Bears", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Growth", "{0}", "Instant", "Target creature gets +2/+0 until end of turn.", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {e:?}", d.name())); }
    Game::new(defs, 2, 20)
}

#[test]
fn r509_1b_a_creature_with_less_power_cant_block_it() {
    let mut g = game();
    let otter = g.put(0, 0, Zone::Battlefield);
    let (wall, squire, bears) = (g.put(1, 1, Zone::Battlefield), g.put(1, 2, Zone::Battlefield), g.put(1, 3, Zone::Battlefield));
    cite!(["509.1b"]; assert!(g.cant_block(wall, otter, &[wall]), "power 0 is less than 1"));
    cite!(["509.1b"]; assert!(!g.cant_block(squire, otter, &[squire]), "equal power can block"));
    assert!(!g.cant_block(bears, otter, &[bears]));
    // it restricts what blocks the Otter, not what the Otter blocks
    cite!(["509.1b"]; assert!(!g.cant_block(otter, wall, &[otter])));
}

#[test]
fn r509_1b_the_otters_power_is_compared_as_it_is_now() {
    let mut g = game();
    let otter = g.put(0, 0, Zone::Battlefield);
    let (squire, bears) = (g.put(1, 2, Zone::Battlefield), g.put(1, 3, Zone::Battlefield));
    let growth = g.put(0, 4, Zone::Hand);
    g.act(0, Move::Cast { card: growth, targets: vec![vec![Target::Object(otter)]] }).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["509.1b"]; assert!(g.cant_block(squire, otter, &[squire]), "1 is less than 3"));
    cite!(["509.1b"]; assert!(g.cant_block(bears, otter, &[bears]), "2 is less than 3"));
}
