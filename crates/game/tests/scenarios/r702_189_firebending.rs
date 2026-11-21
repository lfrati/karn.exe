//! Firebending (702.189).
//!
//! 702.189a: "Firebending is a triggered ability. "Firebending N" means "Whenever this creature
//! attacks, add N {R}. Until end of combat, you don't lose this mana as steps and phases end.""
//!
//! 106.4: "When an effect instructs a player to add mana, that mana goes into a mana pool. …
//! Each player's mana pool empties at the end of each step and phase, and the player is said to
//! lose this mana." 511.3: "As the end of combat step ends, the combat phase ends and all
//! effects that last "until end of combat" end."
//!
//! Every card is read from its Oracle text, but for the "until end of turn" keeper.
use crate::support::cite;
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const NAMES: [&str; 7] = ["Mountain", "Fire Sages", "Firebending Student", "Fire Nation Palace", "Fire Nation Cadets", "Lightning Bolt", "Grizzly Bears"];
const MOUNTAIN: usize = 0;
const SAGES: usize = 1;
const STUDENT: usize = 2;
const PALACE: usize = 3;
const CADETS: usize = 4;
const BOLT: usize = 5;
const BEARS: usize = 6;
const KEEPER: usize = 7;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let mut defs: Vec<_> = NAMES.map(|n| CardDef::from_faces(&catalog[n]).unwrap()).to_vec();
    defs.push(CardDef::new("Keeper", "{0}", "Artifact", "{T}: Add {G}. Until end of turn, you don't lose this mana as steps and phases end.", None).unwrap());
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, MOUNTAIN, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn red(g: &Game) -> u32 {
    let pool = &g.players[0].pool;
    pool.colors[3] + pool.restricted.iter().filter(|m| m.mana == ir::mana::ManaSymbol::Color(ir::Color::Red)).count() as u32
}
/// Player 0 attacks player 1 with these; the firebending triggers resolve.
fn attack(g: &mut Game, attackers: &[ObjId]) {
    while g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Attack(attackers.iter().map(|a| (*a, Target::Player(1))).collect())).unwrap();
    while !g.stack.is_empty() || matches!(g.decision, Some(Decision::Order { .. })) {
        match g.decision.clone() {
            Some(Decision::Order { player, abilities }) => g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    assert_eq!(g.part, Part::DeclareAttackers);
}
/// To the start of the next part with no blocks.
fn next_part(g: &mut Game) {
    let part = g.part;
    while g.part == part || matches!(g.decision, Some(Decision::Blockers { .. })) {
        match g.decision.clone() {
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            Some(d) => panic!("{d:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
}

#[test]
fn r702_189a_the_mana_lasts_until_end_of_combat() {
    // Fire Sages: "Firebending 1"
    let mut g = game();
    let sages = g.put(0, SAGES, Zone::Battlefield);
    attack(&mut g, &[sages]);
    cite!(["702.189a"]; assert_eq!(red(&g), 1));
    for part in [Part::DeclareBlockers, Part::CombatDamage, Part::EndOfCombat] {
        next_part(&mut g);
        assert_eq!(g.part, part);
        cite!(["702.189a", "106.4"]; case: format!("{part:?}"); assert_eq!(red(&g), 1, "kept as steps end"));
    }
    next_part(&mut g);
    assert_eq!(g.part, Part::PostcombatMain);
    cite!(["511.3", "106.4"]; assert_eq!(red(&g), 0, "lost as the combat phase ends"));
}

#[test]
fn r702_189a_the_kept_mana_is_spent_like_any() {
    let mut g = game();
    let sages = g.put(0, SAGES, Zone::Battlefield);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    let bolt = g.put(0, BOLT, Zone::Hand);
    attack(&mut g, &[sages]);
    next_part(&mut g);
    assert_eq!((g.part, g.priority), (Part::DeclareBlockers, 0));
    g.act(0, Move::Paying(game::state::Pool { restricted: vec![g.players[0].pool.restricted[0].clone()], ..Default::default() },Box::new(Move::Cast { card: bolt, targets: vec![vec![Target::Object(bears)]] }))).unwrap();
    cite!(["702.189a"]; assert_eq!(red(&g), 0));
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    assert_eq!(g.obj(bears).zone, Zone::Gone);
}

#[test]
fn r702_189a_firebending_x_is_counted_as_it_resolves() {
    // Firebending Student: "Firebending X, where X is this creature's power." 1/2
    for (counters, mana) in [(0, 1), (2, 3)] {
        let mut g = game();
        let student = g.put(0, STUDENT, Zone::Battlefield);
        g.put_counters(student, "+1/+1", counters);
        attack(&mut g, &[student]);
        cite!(["702.189a"]; case: counters; assert_eq!(red(&g), mana));
    }
}

#[test]
fn r613_1f_a_granted_firebending_triggers_and_a_conditional_one_only_while_it_has_it() {
    // Fire Nation Palace: "{1}{R}, {T}: Target creature you control gains firebending 4 until
    // end of turn."
    let mut g = game();
    let palace = g.put(0, PALACE, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    // Fire Nation Cadets: "This creature has firebending 2 as long as there's a Lesson card in
    // your graveyard." There is none.
    let cadets = g.put(0, CADETS, Zone::Battlefield);
    g.players[0].pool.colors[3] = 2;
    let ability = g.def(palace).unwrap().abilities().iter().rposition(|a| matches!(a, ir::Ability::Activated { .. })).unwrap();
    g.act(0, crate::support::paid("RR",Move::Activate { source: palace, ability, targets: vec![vec![Target::Object(bears)]], paying: vec![] })).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    attack(&mut g, &[bears, cadets]);
    cite!(["613.1f", "702.189a"]; assert_eq!(red(&g), 4, "the Bears' four, none of the Cadets'"));
}

#[test]
fn r106_4_mana_kept_until_end_of_turn_is_lost_as_the_turn_ends() {
    // "{T}: Add {G}. Until end of turn, you don't lose this mana as steps and phases end."
    let mut g = game();
    let keeper = g.put(0, KEEPER, Zone::Battlefield);
    g.act(0, Move::Activate { source: keeper, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    let green = |g: &Game| g.players[0].pool.total();
    assert_eq!(green(&g), 1);
    while g.part != Part::End {
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
        cite!(["106.4"]; assert_eq!(green(&g), 1, "{:?}", g.part));
    }
    while g.active == 0 { g.act(g.decision_maker(g.priority), Move::Pass).unwrap(); }
    cite!(["514.2", "106.4"]; assert_eq!(green(&g), 0));
}
