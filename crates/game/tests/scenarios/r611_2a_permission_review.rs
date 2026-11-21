//! Independent boundary/composition checks for 611.2a, 514.2 and 500.7.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use game::state::Decision;
use ir::Part;

const LAND: usize = 0;
const BOLT: usize = 1;
const TURN: usize = 2;
const NEXT: usize = 3;
const EXTRA: usize = 4;
const GROWTH: usize = 5;
const BEAR: usize = 6;
const UPKEEP: usize = 7;
const OVERLAP: usize = 8;
const END: usize = 9;
const OVERLAP_UPKEEP: usize = 10;
const SKIP_UPKEEP: usize = 11;

fn game(players: usize) -> Game {
    let d = |name, ty, text| CardDef::new(name, "{0}", ty, text, None).unwrap();
    let defs = vec![
        CardDef::new("Mountain", "", "Basic Land — Mountain", "", None).unwrap(),
        CardDef::new("Bolt", "{R}", "Instant", "Bolt deals 3 damage to any target.", None).unwrap(),
        d("Turn", "Instant", "Exile the top card of your library. Until end of turn, you may play that card."),
        d("Next", "Instant", "Exile the top card of your library. Until the end of your next turn, you may play that card."),
        d("Extra", "Instant", "Target player takes an extra turn after this one."),
        d("Growth", "Instant", "Target creature gets +3/+3 until end of turn."),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        d("Upkeep", "Instant", "Exile the top card of your library. Until the beginning of your next upkeep, you may play that card."),
        d("Overlap", "Instant", "Exile the top card of your library. Until end of turn, you may cast that card. Until the end of your next turn, you may play that card."),
        d("End", "Instant", "Exile the top card of your library. Until your next end step, you may play that card."),
        d("Overlap Upkeep", "Instant", "Exile the top card of your library. Until the end of your next turn, you may play that card. Until the beginning of your next upkeep, you may play that card."),
        d("Skip Upkeep", "Instant", "Target player skips their next upkeep step."),
    ];
    for def in &defs { game::play::check(def).unwrap_or_else(|e| panic!("{}: {}", def.name(), e.0)); }
    let mut g = Game::new(defs, players, 20);
    for player in 0..players { for _ in 0..16 { g.put(player, LAND, Zone::Library); } }
    g
}

fn go(g: &mut Game) {
    let (actor, m) = match g.decision.clone() {
        Some(Decision::Attackers { player }) => (player, Move::Attack(vec![])),
        Some(Decision::Discard { player, count }) => (player, Move::Discard(g.players[player].hand[..count].to_vec())),
        None => (g.priority, Move::Pass),
        other => panic!("unexpected {other:?}"),
    };
    g.act(actor, m).unwrap();
}

fn to(g: &mut Game, turn: u32, part: Part, player: usize) {
    for _ in 0..200 {
        if (g.turn, g.part, g.priority) == (turn, part, player) && g.stack.is_empty() && g.decision.is_none() { return; }
        go(g);
    }
    panic!("never reached {turn} {part:?} p{player}");
}

fn resolve(g: &mut Game, card: usize, targets: Vec<Vec<Target>>) {
    let player = g.priority;
    let object = g.put(player, card, Zone::Hand);
    g.act(player, Move::Cast { card: object, targets }).unwrap();
    for _ in 0..g.players.len() { go(g); }
    assert!(g.stack.is_empty());
}

fn grant(g: &mut Game, kind: usize, card: usize) -> ObjId {
    let player = g.priority;
    g.put(player, card, Zone::Library);
    resolve(g, kind, vec![]);
    *g.exile.last().unwrap()
}

#[test]
fn r514_2_a_permission_is_gone_before_priority_in_cleanup() {
    // Rules: 611.2a, 514.2, 514.3a, 704.5f
    let mut g = game(2);
    let exiled = grant(&mut g, TURN, BOLT);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    resolve(&mut g, GROWTH, vec![vec![Target::Object(bear)]]);
    g.put_counters(bear, "-1/-1", 2);
    to(&mut g, 1, Part::Cleanup, 0);
    cite!(["514.2", "514.3a", "704.5f"]; assert!(!g.battlefield.contains(&bear)));
    assert!(g.exile.contains(&exiled));
    cite!(["611.2a", "514.2"]; assert_eq!(g.permission(exiled), None));
    g.players[0].pool.colors[3] = 1;
    cite!(["514.2"]; refused("that card is not in your hand: it is in exile, and no rule or effect lets you cast it from there (601.3)"): g.act(0, paid("R", Move::Cast { card: exiled, targets: vec![vec![Target::Player(1)]] })));
}

#[test]
fn r611_2a_the_next_actual_turn_can_be_an_extra_turn() {
    // Rules: 611.2a, 500.7, 514.2
    let mut g = game(3);
    let exiled = grant(&mut g, NEXT, BOLT);
    resolve(&mut g, EXTRA, vec![vec![Target::Player(0)]]);
    to(&mut g, 2, Part::End, 0);
    cite!(["500.7"]; assert_eq!(g.active, 0, "an extra turn, before opponents' turns"));
    cite!(["611.2a"]; assert_eq!(g.permission(exiled), Some((0, true))));
    assert_eq!(g.permission_until(exiled), Some("end_of_turn"));
    to(&mut g, 3, Part::Upkeep, 1);
    assert!(g.exile.contains(&exiled));
    cite!(["611.2a", "514.2"]; assert_eq!(g.permission(exiled), None, "the extra turn's cleanup ended it"));
}

#[test]
fn r611_2a_a_grant_during_upkeep_waits_until_a_later_upkeep_begins() {
    // Rules: 611.2a, 503.1
    let mut g = game(2);
    to(&mut g, 3, Part::Upkeep, 0);
    let exiled = grant(&mut g, UPKEEP, BOLT);
    assert_eq!(g.permission(exiled), Some((0, true)));
    to(&mut g, 3, Part::PrecombatMain, 0);
    cite!(["611.2a"]; assert_eq!(g.permission(exiled), Some((0, true))));
    to(&mut g, 4, Part::Upkeep, 1);
    cite!(["611.2a"]; assert_eq!(g.permission(exiled), Some((0, true))));
    to(&mut g, 5, Part::Upkeep, 0);
    cite!(["611.2a"]; assert_eq!(g.permission(exiled), None));
}

#[test]
fn r611_2a_a_grant_during_end_step_waits_until_a_later_end_step_begins() {
    // Rules: 611.2a, 513.1
    let mut g = game(2);
    to(&mut g, 1, Part::End, 0);
    let exiled = grant(&mut g, END, BOLT);
    assert_eq!(g.permission(exiled), Some((0, true)));
    to(&mut g, 2, Part::End, 1);
    cite!(["611.2a"]; assert_eq!(g.permission(exiled), Some((0, true))));
    to(&mut g, 3, Part::End, 0);
    cite!(["611.2a"]; assert_eq!(g.permission(exiled), None));
}

#[test]
fn r611_2a_the_visible_land_permission_uses_its_own_deadline() {
    // Rules: 611.2a, 305.2
    // These two grants affect the same incarnation, but only the longer one permits lands.
    let mut g = game(2);
    let exiled = grant(&mut g, OVERLAP, LAND);
    assert_eq!(g.permission(exiled), Some((0, true)));
    // grants that end differently are each reported, the land right with its own deadline
    assert_eq!(g.permission_until(exiled), None);
    assert!(g.permission_grants(exiled).contains(&(0, true, Some("end_of_their_next_turn"))));
    to(&mut g, 3, Part::PrecombatMain, 0);
    cite!(["611.2a", "116.2a"]; ok: g.act(0, Move::PlayLand(exiled)));
}

#[test]
fn r611_2a_skipping_an_upkeep_changes_the_order_of_two_permission_deadlines() {
    // Rules: 611.2a, 614.10, 514.2
    let mut g = game(2);
    let exiled = grant(&mut g, OVERLAP_UPKEEP, BOLT);
    resolve(&mut g, SKIP_UPKEEP, vec![vec![Target::Player(0)]]);
    let reported_last = g.permission_until(exiled);
    // The next own upkeep was skipped on turn 3; its grant outlasts turn 3's cleanup.
    to(&mut g, 4, Part::Upkeep, 0);
    cite!(["611.2a", "614.10a"]; assert_eq!(g.permission(exiled), Some((0, true))));
    assert_eq!(g.permission_until(exiled), Some("their_next_upkeep"));
    g.players[0].pool.colors[3] = 1;
    cite!(["611.2a", "614.10a"]; ok: g.act(0, paid("R", Move::Cast { card: exiled, targets: vec![vec![Target::Player(1)]] })));
    for _ in 0..2 { go(&mut g); }
    assert_eq!(g.players[1].life, 17);
    assert_ne!(reported_last, Some("end_of_their_next_turn"), "the published last deadline ended while a grant survived it");
}
