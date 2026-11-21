//! Secret Identity ("Choose one — • Conceal — Until end of turn, target
//! creature you control becomes a Citizen with base power and toughness 1/1 and gains
//! hexproof. • Reveal — … becomes a Hero with base power and toughness 3/4 and gains flying
//! and vigilance.") and Elusive Otter ("Prowess. Creatures with power less than this
//! creature's power can't block it.").
//! A set subtype replaces the creature types (205.1a, 613.1d); setting base power and toughness is
//! layer 7b, under counters and modifications in 7c whatever their timestamps (613.4b–c).
use crate::support::{self, cite};
use support::*;
use ir::Part;

const CONCEAL: usize = 0;
const REVEAL: usize = 1;

fn identity(t: &mut T, label: &str, mode: usize, target: ObjId) -> Result<(), String> {
    let card = t.id(label);
    t.mana(0, "{U}");
    t.g.act(0, paid("U", Move::Modes(vec![mode], Box::new(Move::Cast { card, targets: vec![vec![Target::Object(target)]] }))))?;
    t.resolve();
    Ok(())
}
fn subtypes(t: &T, o: ObjId) -> Vec<String> { t.g.seen(o, None).subtypes.iter().map(|s| s.name().to_string()).collect() }

#[test]
fn r205_1a_conceal_replaces_the_creature_types() {
    let mut t = Board::new(2).battlefield(0, &["Grizzly Bears"]).named(0, Zone::Hand, "si", "Secret Identity").build();
    let bears = t.find_in_battlefield("Grizzly Bears");
    identity(&mut t, "si", CONCEAL, bears).unwrap();
    cite!(["205.1a", "613.1d"]; assert_eq!(subtypes(&t, bears), vec!["Citizen"], "no longer a Bear"));
}

#[test]
fn r613_4c_counters_and_an_earlier_pump_apply_over_the_set_base() {
    let mut t = Board::new(2).battlefield(0, &["Grizzly Bears"]).counters("Grizzly Bears", "+1/+1", 1)
        .named(0, Zone::Hand, "si", "Secret Identity").named(0, Zone::Hand, "gg", "Giant Growth").build();
    let bears = t.find_in_battlefield("Grizzly Bears");
    let growth = t.id("gg");
    t.mana(0, "{G}").g.act(0, paid("G", Move::Cast { card: growth, targets: vec![vec![Target::Object(bears)]] })).unwrap();
    t.resolve();
    t.expect().rules(["613.4c"]).pt("Grizzly Bears", 6, 6).check();
    identity(&mut t, "si", REVEAL, bears).unwrap();
    // base 3/4, +3/+3 from the earlier Giant Growth, +1/+1 from the counter
    t.expect().rules(["613.4b", "613.4c"]).pt("Grizzly Bears", 7, 8).check();
}

/// Conceal then Reveal on one creature: the later effect sets the subtype and base P/T; the
/// abilities each grants are both kept.
#[test]
fn r613_7_the_later_identity_wins_and_abilities_accumulate() {
    let mut t = Board::new(2).battlefield(0, &["Grizzly Bears"])
        .named(0, Zone::Hand, "a", "Secret Identity").named(0, Zone::Hand, "b", "Secret Identity").build();
    let bears = t.find_in_battlefield("Grizzly Bears");
    identity(&mut t, "a", CONCEAL, bears).unwrap();
    identity(&mut t, "b", REVEAL, bears).unwrap();
    t.expect().rules(["613.4b", "613.7"]).pt("Grizzly Bears", 3, 4).check();
    cite!(["613.1d", "613.7"]; assert_eq!(subtypes(&t, bears), vec!["Hero"]));
    cite!(["613.1f"]; assert!(t.g.has(bears, "Hexproof") && t.g.has(bears, "Flying") && t.g.has(bears, "Vigilance")));
}

/// "Target creature you control": not an opponent's. Concealed, an opponent can't target it.
#[test]
fn r702_11b_concealed_it_has_hexproof_from_the_opponent() {
    let mut t = Board::new(2).battlefield(0, &["Grizzly Bears"]).battlefield(1, &["Llanowar Elves"])
        .named(0, Zone::Hand, "si", "Secret Identity").named(1, Zone::Hand, "shock", "Shock").build();
    let (bears, elves) = (t.find_in_battlefield("Grizzly Bears"), t.find_in_battlefield("Llanowar Elves"));
    cite!(["115.1"]; refused("an illegal target, Llanowar Elves: the target is one you control, and another player controls it (115.1)"): identity(&mut t, "si", CONCEAL, elves));
    t.g.players[0].pool = Default::default();
    identity(&mut t, "si", CONCEAL, bears).unwrap();
    t.g.act(0, Move::Pass).unwrap();
    let shock = t.id("shock");
    t.mana(1, "{R}");
    cite!(["702.11b"]; refused("an illegal target, Grizzly Bears: it has hexproof, and an opponent of its controller is targeting it (702.11b)"): t.g.act(1, paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Object(bears)]] })));
}

/// Until end of turn: the next turn it is a 2/2 Bear again with none of the abilities.
#[test]
fn r514_2_it_ends_at_cleanup() {
    let mut t = Board::new(2).fill(10).battlefield(0, &["Grizzly Bears"]).named(0, Zone::Hand, "si", "Secret Identity").build();
    let bears = t.find_in_battlefield("Grizzly Bears");
    identity(&mut t, "si", REVEAL, bears).unwrap();
    t.to_no_combat(1, Part::PrecombatMain);
    t.expect().rules(["514.2"]).pt("Grizzly Bears", 2, 2).check();
    cite!(["514.2"]; assert_eq!(subtypes(&t, bears), vec!["Bear"]));
    cite!(["514.2"]; assert!(!t.g.has(bears, "Flying") && !t.g.has(bears, "Vigilance")));
}

/// Elusive Otter's creature face in combat: prowess from a noncreature spell makes it 2/2,
/// so a power-1 creature can't block it and a power-2 one can (509.1b, 702.108a).
#[test]
fn r509_1b_prowess_raises_what_cant_block_the_otter() {
    let defs = vec![
        oracle("Elusive Otter"),
        oracle("Llanowar Elves"), oracle("Grizzly Bears"), oracle("Shock"), oracle("Island"),
    ];
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, 4, Zone::Library); } }
    let otter = g.put(0, 0, Zone::Battlefield);
    let (elves, bears) = (g.put(1, 1, Zone::Battlefield), g.put(1, 2, Zone::Battlefield));
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["509.1b"]; assert!(!g.cant_block(elves, otter, &[elves]), "1 is not less than 1"));
    let shock = g.put(0, 3, Zone::Hand);
    g.players[0].pool.colors[3] = 1;
    g.act(0, paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Player(1)]] })).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["702.108a"]; assert_eq!((g.power(otter), g.toughness(otter)), (Some(2), Some(2)), "prowess"));
    while !matches!(g.decision, Some(game::state::Decision::Attackers { .. })) { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Attack(vec![(otter, Target::Player(1))])).unwrap();
    while !matches!(g.decision, Some(game::state::Decision::Blockers { .. })) { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["509.1b"]; refused("Llanowar Elves can't block Elusive Otter: Elusive Otter says it can't, or not that way (509.1b)"): g.clone().act(1, Move::Block(vec![(elves, otter)])));
    cite!(["509.1b"]; ok: g.act(1, Move::Block(vec![(bears, otter)])));  // 2 is not less than 2
}
