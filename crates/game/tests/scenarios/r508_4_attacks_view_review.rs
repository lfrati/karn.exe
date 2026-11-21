//! What every seat sees of each attack (506.4, 508.4), and which
//! entering creature a defender choice is for.
use crate::support::cite;
use game::snapshot::Snapshot;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const BEAR: usize = 0;
const RALLY: usize = 1;
const WALKER: usize = 2;
const GAIN: usize = 3;

fn game() -> Game {
    let defs = vec![
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Rally", "{0}", "Instant", "Create two 1/1 white Soldier creature tokens that are tapped and attacking.", None).unwrap(),
        CardDef::planeswalker("Walker", "{0}", "Legendary Planeswalker — Garruk", "+1: You gain 1 life.", 5).unwrap(),
        CardDef::new("Gain", "{0}", "Instant", "You gain 1 life.", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    Game::new(defs, 3, 20)
}
fn attacks(g: &Game, seat: usize) -> Vec<(ObjId, Target)> { Snapshot::of(g, Some(seat)).attacks }
fn to_attackers(g: &mut Game) {
    while g.decision != Some(Decision::Attackers { player: 0 }) { g.act(g.priority, Move::Pass).unwrap(); }
}

#[test]
fn r506_4_every_seat_sees_each_declared_attack_and_entered_attack_until_combat_ends() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let walker = g.put(2, WALKER, Zone::Battlefield);
    g.put_counters(walker, "loyalty", 5);
    let rally = g.put(0, RALLY, Zone::Hand);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(bear, Target::Object(walker))])).unwrap();
    for seat in 0..3 { assert_eq!(attacks(&g, seat), vec![(bear, Target::Object(walker))], "seat {seat}"); }
    while g.decision.is_some() || g.priority != 0 { match g.decision.clone() { Some(Decision::Order { player, abilities }) => g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(), _ => g.act(g.priority, Move::Pass).unwrap() } }
    g.act(0, Move::Cast { card: rally, targets: vec![] }).unwrap();
    let mut asked = Vec::new();
    let defenders = [Target::Player(2), Target::Player(1)];
    loop {
        match g.decision.clone() {
            Some(Decision::Choose { player, choice: Choice::Defender { among, nth, of } }) => {
                cite!(["508.4"]; assert!(among.contains(&defenders[nth - 1])));
                asked.push((nth, of));
                g.act(player, Move::Choose(Answer::Defender(defenders[nth - 1]))).unwrap();
            }
            Some(d) => panic!("{d:?}"),
            None if g.stack.is_empty() => break,
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    cite!(["508.4"]; assert_eq!(asked, vec![(1, 2), (2, 2)]));
    let tokens: Vec<ObjId> = g.battlefield.iter().copied().filter(|o| g.obj(*o).token()).collect();
    for seat in 0..3 {
        let seen = attacks(&g, seat);
        assert_eq!(seen.len(), 3, "seat {seat}");
        let targets: Vec<Target> = tokens.iter().map(|t| seen.iter().find(|(a, _)| a == t).unwrap().1).collect();
        cite!(["508.4"]; assert_eq!(targets, defenders.to_vec(), "seat {seat}: each token's own defender, in the order chosen"));
    }
    // the walker leaves: removed from combat, it is attacked no longer (506.4, 506.4c)
    g.remove_counters(walker, "loyalty", 5);
    let hand = g.put(0, GAIN, Zone::Hand);
    g.act(0, Move::Cast { card: hand, targets: vec![] }).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    assert_eq!(g.obj(walker).zone, Zone::Gone);
    for seat in 0..3 {
        // still attacking, but attacking no player, planeswalker or battle (506.4c)
        cite!(["506.4c"]; assert!(Snapshot::of(&g, Some(seat)).attacking.contains(&bear)));
        cite!(["506.4c"]; assert!(!attacks(&g, seat).contains(&(bear, Target::Object(walker))), "seat {seat}: {:?}", attacks(&g, seat)));
    }
    while g.part != ir::Part::PostcombatMain { match g.decision.clone() { Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(), _ => g.act(g.priority, Move::Pass).unwrap() } }
    for seat in 0..3 { cite!(["511.3"]; assert!(attacks(&g, seat).is_empty(), "combat over (511.3)")); }
}
