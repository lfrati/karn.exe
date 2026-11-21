//! "You may remove N counters.
//! If you do, …": a player can't choose an impossible option (608.2d), so with fewer than N
//! counters, or with the holder gone, the player hasn't done it. Nazar ruling
//! ac24c41510cfe349: "If Nazar isn't on the battlefield as its last ability resolves, you
//! can't remove three feeding counters from it … You won't draw cards or lose life."
//! Impossible removal must skip the optional action without paying a partial amount.
use crate::support::cite;
use game::state::{Answer, Decision};
use game::{CardDef, Game, Move, Target, Zone};
use ir::Part;

const SUN_DROPLET: &str = "Whenever you're dealt damage, put that many charge counters on this artifact.\nAt the beginning of each upkeep, you may remove a charge counter from this artifact. If you do, you gain 1 life.";
const MAGMASAUR: &str = "This creature enters with five +1/+1 counters on it.\nAt the beginning of your upkeep, you may remove a +1/+1 counter from this creature. If you don't, sacrifice this creature and it deals damage equal to the number of +1/+1 counters on it to each creature without flying and each player.";
/// Nazar's attack-trigger body, activated so a test can control when it resolves.
const NAZAR: &str = "{0}: You may remove three feeding counters from this creature. If you do, you draw three cards and you lose 3 life.";

fn game(defs: Vec<CardDef>) -> Game {
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let land = defs.len() - 1;
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, land, Zone::Library); } }
    g
}
fn island() -> CardDef { CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap() }
/// An authored response to the optional counter-removal question. Impossible
/// removal is skipped by the procedure (608.2d), never retried with another answer.
fn say(g: &mut Game, yes: bool) {
    let Some(Decision::Choose { player: 0, choice: game::state::Choice::May }) = g.decision else {
        panic!("expected player 0's optional removal: {:?}", g.decision);
    };
    g.act(0, Move::Choose(if yes { Answer::Yes } else { Answer::No })).unwrap();
}

fn resolve(g: &mut Game) {
    for _ in 0..30 {
        if matches!(g.decision, Some(Decision::Choose { .. })) { say(g, true); continue }
        if g.stack.is_empty() { return }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("resolution did not reach an empty stack");
}
/// From P0's untap step to its draw step or, on the first turn, its main phase (103.8a),
/// answering each "may" with `yes`.
fn upkeep(g: &mut Game, yes: bool) {
    g.part = Part::Untap;
    for _ in 0..20 {
        if matches!(g.part, Part::Draw | Part::PrecombatMain) { return }
        if matches!(g.decision, Some(Decision::Choose { .. })) { say(g, yes); continue }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("upkeep did not complete");
}

#[test]
fn sun_droplet_with_a_charge_counter_gains_life() {
    let mut g = game(vec![CardDef::new("Sun Droplet", "{2}", "Artifact", SUN_DROPLET, None).unwrap(), island()]);
    let droplet = g.put(0, 0, Zone::Battlefield);
    g.put_counters(droplet, "charge", 1);
    upkeep(&mut g, true);
    cite!(["118.12"]; assert_eq!(g.players[0].life, 21));
    assert_eq!(g.counters(droplet).get("charge"), None);
}

/// With no charge counter to remove, no life is gained (20, not 21).
#[test]
fn sun_droplet_without_a_charge_counter_gains_no_life() {
    let mut g = game(vec![CardDef::new("Sun Droplet", "{2}", "Artifact", SUN_DROPLET, None).unwrap(), island()]);
    g.put(0, 0, Zone::Battlefield);
    upkeep(&mut g, true);
    cite!(["608.2d"]; assert_eq!(g.players[0].life, 20, "gained life without removing a charge counter"));
}

fn nazar(feeding: u32) -> (Game, game::ObjId) {
    let mut g = game(vec![
        CardDef::new("Nazar", "{0}", "Creature — Vampire", NAZAR, Some((3, 3))).unwrap(),
        CardDef::new("Exile", "{0}", "Instant", "Exile target creature.", None).unwrap(),
        island(),
    ]);
    let nazar = g.put(0, 0, Zone::Battlefield);
    g.put_counters(nazar, "feeding", feeding);
    g.act(0, Move::Activate { source: nazar, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    (g, nazar)
}

#[test]
fn nazar_with_three_feeding_counters_draws_three() {
    let (mut g, nazar) = nazar(3);
    resolve(&mut g);
    cite!(["118.12"]; assert_eq!((g.players[0].life, g.players[0].hand.len()), (17, 3)));
    assert_eq!(g.counters(nazar).get("feeding"), None);
}

/// It must not remove the two, draw three and lose 3 life.
#[test]
fn nazar_with_two_feeding_counters_cannot_remove_three() {
    let (mut g, nazar) = nazar(2);
    resolve(&mut g);
    cite!(["608.2d"]; assert_eq!((g.players[0].life, g.players[0].hand.len()), (20, 0), "drew for an impossible removal"));
    cite!(["608.2d"]; assert_eq!(g.counters(nazar).get("feeding"), Some(&2), "an impossible option can't be chosen (608.2d)"));
}

/// It must not draw three and lose 3 life (ruling ac24c41510cfe349).
#[test]
fn nazar_gone_before_resolution_draws_nothing() {
    let (mut g, nazar) = nazar(3);
    g.act(0, Move::Pass).unwrap();
    let exile = g.put(1, 1, Zone::Hand);
    g.act(1, Move::Cast { card: exile, targets: vec![vec![Target::Object(nazar)]] }).unwrap();
    resolve(&mut g);
    cite!(["608.2d"]; assert_eq!((g.players[0].life, g.players[0].hand.len()), (20, 0)));
}

fn magmasaur(counters: u32) -> (Game, game::ObjId) {
    let mut g = game(vec![
        CardDef::new("Magmasaur", "{0}", "Creature — Elemental Dinosaur", MAGMASAUR, Some((0, 0))).unwrap(),
        CardDef::new("Anthem", "{0}", "Enchantment", "Creatures you control get +1/+1.", None).unwrap(),
        island(),
    ]);
    g.put(0, 1, Zone::Battlefield);
    let magmasaur = g.put(0, 0, Zone::Battlefield);
    g.remove_counters(magmasaur, "+1/+1", 5);
    g.put_counters(magmasaur, "+1/+1", counters);
    (g, magmasaur)
}

/// Declining is "If you don't": it is sacrificed and deals its one counter's damage.
#[test]
fn magmasaur_declined_is_sacrificed() {
    let (mut g, magmasaur) = magmasaur(1);
    upkeep(&mut g, false);
    resolve(&mut g);
    assert!(!g.battlefield.contains(&magmasaur));
    cite!(["118.12", "608.2h"]; assert_eq!((g.players[0].life, g.players[1].life), (19, 19)));
}

/// With no +1/+1 counter Magmasaur can't remove one, so "If you don't"
/// holds and it is sacrificed. The anthem keeps the 0/0 alive until then.
#[test]
fn magmasaur_without_a_counter_is_sacrificed() {
    let (mut g, magmasaur) = magmasaur(0);
    upkeep(&mut g, true);
    resolve(&mut g);
    cite!(["608.2d"]; assert!(!g.battlefield.contains(&magmasaur), "kept by an impossible removal"));
}
