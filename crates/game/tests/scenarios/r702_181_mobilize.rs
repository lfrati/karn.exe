//! Mobilize (702.181).
//!
//! 702.181a: "Mobilize is a triggered ability. “Mobilize N” means “Whenever this creature
//! attacks, create N 1/1 red Warrior creature tokens. Those tokens enter tapped and attacking.
//! Sacrifice them at the beginning of the next end step.”"
//!
//! 506.3a: a creature put onto the battlefield attacking was never declared as an attacker.
//!
//! Every card is read from its Oracle text, but for the granting one.
use crate::support::cite;
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const NAMES: [&str; 5] = ["Plains", "Shock Brigade", "Dalkovan Packbeasts", "Avenger of the Fallen", "Grizzly Bears"];
const PLAINS: usize = 0;
const BRIGADE: usize = 1;
const PACKBEASTS: usize = 2;
const AVENGER: usize = 3;
const BEARS: usize = 4;
const BANNER: usize = 5;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let mut defs: Vec<_> = NAMES.map(|n| CardDef::from_faces(&catalog[n]).unwrap()).to_vec();
    defs.push(CardDef::new("Banner", "{0}", "Artifact", "{T}: Target creature you control gains mobilize 2 until end of turn.", None).unwrap());
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, PLAINS, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
/// Player 0 attacks player 1 with these; the triggers resolve.
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
/// To this part with no blocks.
fn to(g: &mut Game, part: Part) {
    while g.part != part {
        match g.decision.clone() {
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            Some(d) => panic!("{d:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
}
fn warriors(g: &Game) -> Vec<ObjId> {
    g.battlefield.iter().copied().filter(|o| g.obj(*o).token() && g.obj(*o).chars.name.contains("Warrior")).collect()
}

#[test]
fn r702_181a_tokens_enter_tapped_and_attacking_and_are_sacrificed_at_the_next_end_step() {
    // Shock Brigade 1/3: "Mobilize 1"
    let mut g = game();
    let brigade = g.put(0, BRIGADE, Zone::Battlefield);
    attack(&mut g, &[brigade]);
    let tokens = warriors(&g);
    cite!(["702.181a"]; assert_eq!(tokens.len(), 1));
    let token = tokens[0];
    cite!(["702.181a"]; assert_eq!((g.power(token), g.toughness(token)), (Some(1), Some(1))));
    cite!(["702.181a"]; assert_eq!(g.obj(token).chars.colors, vec![ir::Color::Red]));
    cite!(["702.181a"]; assert!(g.is_a(token, ir::CardType::Creature) && format!("{:?}", g.obj(token).chars.subtypes).contains("Warrior")));
    cite!(["702.181a", "506.3a"]; assert!(g.obj(token).tapped && g.combat.attackers.contains(&(token, Target::Player(1)))));
    to(&mut g, Part::PostcombatMain);
    cite!(["510.1"]; assert_eq!(g.players[1].life, 18, "the Brigade's 1 and the token's 1"));
    to(&mut g, Part::End);
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["702.181a"]; assert_eq!(g.obj(token).zone, Zone::Gone, "sacrificed at the beginning of the end step"));
    assert_eq!(g.obj(brigade).zone, Zone::Battlefield);
}

#[test]
fn r702_181a_a_creature_with_zero_power_mobilizes_its_n() {
    // Dalkovan Packbeasts 0/4: "Vigilance", "Mobilize 3"
    let mut g = game();
    let beasts = g.put(0, PACKBEASTS, Zone::Battlefield);
    attack(&mut g, &[beasts]);
    cite!(["702.181a"]; assert_eq!(warriors(&g).len(), 3, "N, not the creature's power"));
    to(&mut g, Part::PostcombatMain);
    cite!(["510.1"]; assert_eq!(g.players[1].life, 17));
}

#[test]
fn r702_181a_mobilize_x_counts_as_it_resolves_and_zero_creates_none() {
    // Avenger of the Fallen: "Mobilize X, where X is the number of creature cards in your
    // graveyard."
    for dead in [0, 2] {
        let mut g = game();
        let avenger = g.put(0, AVENGER, Zone::Battlefield);
        for _ in 0..dead { g.put(0, BEARS, Zone::Graveyard); }
        g.put(0, PLAINS, Zone::Graveyard);
        attack(&mut g, &[avenger]);
        cite!(["702.181a"]; case: dead; assert_eq!(warriors(&g).len(), dead));
    }
}

#[test]
fn r613_1f_a_creature_given_mobilize_triggers() {
    // "{T}: Target creature you control gains mobilize 2 until end of turn."
    let mut g = game();
    let banner = g.put(0, BANNER, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let other = g.put(0, BEARS, Zone::Battlefield);
    g.act(0, Move::Activate { source: banner, ability: 0, targets: vec![vec![Target::Object(bears)]], paying: vec![] }).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    attack(&mut g, &[bears, other]);
    cite!(["613.1f", "702.181a"]; assert_eq!(warriors(&g).len(), 2, "the given two, none from the other Bears"));
}
