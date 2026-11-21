//! Firebending (702.189) and mana kept "until end of turn".
//!
//! 702.189a: "“Firebending N” means “Whenever this creature attacks, add N {R}. Until end of
//! combat, you don't lose this mana as steps and phases end.”" 106.4: mana pools empty at the
//! end of each step and phase. Savage Ventmaw: "Whenever this creature attacks, add
//! {R}{R}{R}{G}{G}{G}. Until end of turn, you don't lose this mana as steps and phases end."
use crate::support::cite;
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use game::state::Pool;
use ir::Part;

const NAMES: [&str; 6] = ["Mountain", "Fire Sages", "Savage Ventmaw", "Brazen Collector", "Mana Leak", "Sakura-Tribe Springcaller"];
const MOUNTAIN: usize = 0;
const SAGES: usize = 1;
const VENTMAW: usize = 2;
const COLLECTOR: usize = 3;
const LEAK: usize = 4;
const SPRINGCALLER: usize = 5;
const SAPLING: usize = 6;
const CUB: usize = 7;
const TRINKET: usize = 8;
const VAULT: usize = 9;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let mut defs: Vec<_> = NAMES.map(|n| CardDef::from_faces(&catalog[n]).unwrap()).to_vec();
    defs.push(CardDef::new("Sapling", "{G}", "Creature — Plant", "", Some((1, 1))).unwrap());
    defs.push(CardDef::new("Cub", "{1}", "Creature — Bear", "", Some((1, 1))).unwrap());
    defs.push(CardDef::new("Trinket", "{1}", "Artifact", "", None).unwrap());
    defs.push(CardDef::new("Vault", "{0}", "Artifact", "{T}: Add {C}. Spend this mana only to cast artifact spells. Until end of turn, you don't lose this mana as steps and phases end.", None).unwrap());
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, MOUNTAIN, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn count(g: &Game, color: ir::Color) -> u32 {
    let pool = &g.players[0].pool;
    pool.colors[color as usize] + pool.restricted.iter().filter(|m| m.mana == ir::mana::ManaSymbol::Color(color)).count() as u32
}
fn attack(g: &mut Game, attackers: &[ObjId]) {
    while g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Attack(attackers.iter().map(|a| (*a, Target::Player(1))).collect())).unwrap();
    while !g.stack.is_empty() || matches!(g.decision, Some(Decision::Order { .. })) {
        match g.decision.clone() {
            Some(Decision::Order { player, abilities }) => g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
}
/// Passes (no blocks, no attacks) until this part of the turn.
fn to(g: &mut Game, part: Part) {
    while g.part != part {
        match g.decision.clone() {
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(d) => panic!("{d:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
}

// Rules: 702.189a, 106.4, 602.2
// Given: Fire Sages ("Firebending 1"; "{1}{R}{R}: Put a +1/+1 counter on this creature.") attacks.
// When: in the declare blockers step, with {R}{R} added then, its ability is activated.
// Expect: activated, all three mana spent (the kept {R} among them); the counter is put.
// Why: the kept mana is ordinary {R} that may be spent on anything, also an activated ability.
#[test]
fn r702_189a_the_kept_mana_pays_an_activated_ability() {
    let mut g = game();
    let sages = g.put(0, SAGES, Zone::Battlefield);
    attack(&mut g, &[sages]);
    to(&mut g, Part::DeclareBlockers);
    assert_eq!(g.priority, 0);
    cite!(["702.189a"]; assert_eq!(count(&g, ir::Color::Red), 1, "kept into the declare blockers step"));
    g.players[0].pool.colors[3] += 2;
    let ability = g.abilities(sages).iter().position(|a| matches!(&**a, ir::Ability::Activated { .. })).unwrap();
    let mut selected = crate::support::mana_pool("RR");
    selected.restricted.push(g.players[0].pool.restricted[0].clone());
    cite!(["702.189a", "602.2"]; ok: g.act(0, Move::Paying(selected, Box::new(Move::Activate { source: sages, ability, targets: vec![], paying: vec![] }))));
    cite!(["702.189a"]; assert_eq!(g.players[0].pool.total(), 0));
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    assert_eq!(g.obj(sages).counters.get("+1/+1").copied().unwrap_or(0), 1);
}

// Rules: 106.4, 500.5, 514.2
// Given: Savage Ventmaw attacks: "add {R}{R}{R}{G}{G}{G}. Until end of turn, you don't lose
// this mana as steps and phases end."
// When: the turn goes on to the postcombat main phase and the end step, then ends.
// Expect: all six mana in the pool at the postcombat main phase and the end step; none in the
// next turn.
// Why: the effect keeps it through step and phase ends until the end of the turn, past the end
// of combat (unlike firebending).
#[test]
fn r106_4_savage_ventmaw_mana_is_kept_until_end_of_turn() {
    let mut g = game();
    let ventmaw = g.put(0, VENTMAW, Zone::Battlefield);
    attack(&mut g, &[ventmaw]);
    cite!(["106.4"]; assert_eq!((count(&g, ir::Color::Red), count(&g, ir::Color::Green)), (3, 3)));
    for part in [Part::PostcombatMain, Part::End] {
        to(&mut g, part);
        cite!(["106.4", "500.5"]; case: format!("{part:?}"); assert_eq!((count(&g, ir::Color::Red), count(&g, ir::Color::Green)), (3, 3)));
    }
    while g.active == 0 { g.act(g.decision_maker(g.priority), Move::Pass).unwrap(); }
    cite!(["514.2", "106.4"]; assert_eq!(g.players[0].pool.total(), 0));
}

// Rules: 106.4, 118.12
// Given: Brazen Collector ("Whenever this creature attacks, add {R}. Until end of turn, you
// don't lose this mana as steps and phases end.") attacked. In the postcombat main phase its
// controller casts Sapling {G} with {R}{R} more in the pool; the opponent responds with Mana
// Leak ("Counter target spell unless its controller pays {3}.").
// When: Mana Leak resolves and its controller answers yes to the found payment.
// Expect: the {3} is paid with the kept {R} and the {R}{R}; Sapling resolves.
// Why: mana kept until end of turn is spent like any other, also on a payment an effect asks.
#[test]
fn r118_12_kept_mana_pays_an_unless_cost() {
    let mut g = game();
    let collector = g.put(0, COLLECTOR, Zone::Battlefield);
    let leak = g.put(1, LEAK, Zone::Hand);
    let sapling = g.put(0, SAPLING, Zone::Hand);
    attack(&mut g, &[collector]);
    to(&mut g, Part::PostcombatMain);
    cite!(["106.4"]; assert_eq!(count(&g, ir::Color::Red), 1, "kept past the end of combat"));
    g.players[0].pool.colors[3] += 2;
    g.players[0].pool.colors[4] += 1;
    g.act(0, crate::support::paid("G",Move::Cast { card: sapling, targets: vec![] })).unwrap();
    let spell = *g.stack.last().unwrap();
    g.act(0, Move::Pass).unwrap();
    g.players[1].pool.colors[1] = 1;
    g.players[1].pool.colorless = 1;
    g.act(1, crate::support::paid("CU",Move::Cast { card: leak, targets: vec![vec![Target::Object(spell)]] })).unwrap();
    while !matches!(g.decision, Some(Decision::Choose { .. })) { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["118.12"]; assert!(matches!(&g.decision, Some(Decision::Choose { player: 0, choice: game::state::Choice::Pay { .. } })), "{:?}", g.decision));
    cite!(["118.12", "106.4"]; ok: g.act(0, Move::Choose(game::state::Answer::Pay { plan: vec![], with: Some({let mut selected = crate::support::mana_pool("RR"); selected.restricted.push(g.players[0].pool.restricted[0].clone()); selected}), tapped: vec![] })));
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["118.12", "106.4"]; assert_eq!(g.players[0].pool.total(), 0));
    cite!(["118.12"]; assert!(g.battlefield.iter().any(|o| g.obj(*o).chars.name == "Sapling")));
}

/// From player 0's first main phase to their next upkeep, its triggers resolved.
fn next_upkeep(g: &mut Game) {
    while g.active == 0 { to_any(g); }
    while !(g.active == 0 && g.part == Part::Upkeep) { to_any(g); }
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
}
fn to_any(g: &mut Game) {
    match g.decision.clone() {
        Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
        Some(Decision::Discard { player, count, .. }) => { let cards = g.players[player].hand[..count].to_vec(); g.act(player, Move::Discard(cards)).unwrap() },
        Some(d) => panic!("{d:?}"),
        None => g.act(g.decision_maker(g.priority), Move::Pass).unwrap(),
    }
}

// Rules: 106.4, 500.5
// Given: Sakura-Tribe Springcaller ("At the beginning of your upkeep, add {G}. Until end of
// turn, you don't lose this mana as steps and phases end.").
// When: its upkeep trigger resolves and the turn goes on to the precombat main phase.
// Expect: the {G} is kept through the upkeep and draw steps and pays for Cub {1}.
// Why: kept until end of turn as steps and phases end, from the turn's first steps.
#[test]
fn r106_4_upkeep_mana_kept_until_end_of_turn() {
    let mut g = game();
    g.put(0, SPRINGCALLER, Zone::Battlefield);
    let cub = g.put(0, CUB, Zone::Hand);
    next_upkeep(&mut g);
    cite!(["106.4"]; assert_eq!(count(&g, ir::Color::Green), 1));
    to(&mut g, Part::PrecombatMain);
    cite!(["106.4", "500.5"]; assert_eq!(count(&g, ir::Color::Green), 1, "kept through upkeep and draw"));
    cite!(["106.4"]; ok: g.act(0, Move::Paying(Pool { restricted: vec![g.players[0].pool.restricted[0].clone()], ..Pool::default() },Box::new(Move::Cast { card: cub, targets: vec![] }))));
    assert_eq!(g.players[0].pool.total(), 0);
}

// Rules: 106.4, 106.6
// Given: "{T}: Add {C}. Spend this mana only to cast artifact spells. Until end of turn, you
// don't lose this mana as steps and phases end." tapped in the precombat main phase.
// When: the turn goes on, with no attack, to the postcombat main phase.
// Expect: the {C} is still there; it can't pay for Cub {1} (a creature spell) but pays for
// Trinket {1} (an artifact spell).
// Why: kept mana keeps its spending restriction (106.6) as phases end.
#[test]
fn r106_6_kept_restricted_mana_keeps_its_restriction() {
    let mut g = game();
    let vault = g.put(0, VAULT, Zone::Battlefield);
    let cub = g.put(0, CUB, Zone::Hand);
    let trinket = g.put(0, TRINKET, Zone::Hand);
    g.act(0, Move::Activate { source: vault, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    to(&mut g, Part::PostcombatMain);
    cite!(["106.4"]; assert_eq!(g.players[0].pool.total(), 1, "kept as phases end"));
    cite!(["106.6"]; refused("that mana can't be spent on that: its text restricts how it can be spent"): unchanged(&mut g, |g| g.act(0, Move::Paying(Pool { restricted: vec![g.players[0].pool.restricted[0].clone()], ..Pool::default() },Box::new(Move::Cast { card: cub, targets: vec![] })))));
    cite!(["106.6", "106.4"]; ok: g.act(0, Move::Paying(Pool { restricted: vec![g.players[0].pool.restricted[0].clone()], ..Pool::default() },Box::new(Move::Cast { card: trinket, targets: vec![] }))));
    assert_eq!(g.players[0].pool.total(), 0);
}

#[track_caller]
fn unchanged(g: &mut Game, action: impl FnOnce(&mut Game) -> Result<(), String>) -> Result<(), String> {
    let before=format!("{g:?}");
    let result=action(g);
    assert_eq!(format!("{g:?}"),before);
    result
}
