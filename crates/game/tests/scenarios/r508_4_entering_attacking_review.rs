//! Creatures put onto the battlefield attacking (508.4, 506.3b,
//! 508.4d), and battle cry (702.91) composing with them.
use crate::support::cite;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const HERO: usize = 0;
const WALKER: usize = 1;
const RALLY: usize = 2;
const BEAR: usize = 3;
const WATCHER: usize = 4;

fn game(players: usize) -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let hero = CardDef::from_face(faces.iter().find(|f| f.name == "Hero of Bladehold").unwrap()).unwrap();
    let defs = vec![
        hero,
        CardDef::planeswalker("Walker", "{0}", "Legendary Planeswalker — Garruk", "+1: You gain 1 life.", 5).unwrap(),
        CardDef::new("Rally", "{0}", "Instant", "Create a 1/1 white Soldier creature token that's tapped and attacking.", None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Watcher", "{0}", "Enchantment", "Whenever a creature you control attacks, you gain 1 life.", None).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, players, 20);
    for p in 0..players { for _ in 0..5 { g.put(p, 5, Zone::Library); } }
    g
}
fn soldiers(g: &Game) -> Vec<ObjId> { g.battlefield.iter().copied().filter(|o| g.obj(*o).token()).collect() }
fn to_attackers(g: &mut Game) {
    while g.decision != Some(Decision::Attackers { player: 0 }) { g.act(g.priority, Move::Pass).unwrap(); }
}
/// Pass until `part`: trigger order as listed (or reversed), the first offered defender, no blocks.
fn until(g: &mut Game, part: Part, reversed: bool) -> Vec<Vec<Target>> {
    let mut offered = Vec::new();
    while g.part != part {
        match g.decision.clone() {
            Some(Decision::Order { player, abilities }) => {
                let mut order: Vec<usize> = (0..abilities.len()).collect();
                if reversed { order.reverse(); }
                g.act(player, Move::Order(order)).unwrap();
            }
            Some(Decision::Choose { player, choice: Choice::Defender { among, .. } }) => {
                g.act(player, Move::Choose(Answer::Defender(among[0]))).unwrap();
                offered.push(among);
            }
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            Some(d) => panic!("{d:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    offered
}

#[test]
fn r702_91a_the_trigger_order_decides_whether_the_tokens_get_battle_crys_bonus() {
    // battle cry resolving after the tokens are created pumps them (they are attacking);
    // resolving before, it does not (they were not on the battlefield yet)
    let mut powers = Vec::new();
    for reversed in [false, true] {
        let mut g = game(2);
        let hero = g.put(0, HERO, Zone::Battlefield);
        to_attackers(&mut g);
        g.act(0, Move::Attack(vec![(hero, Target::Player(1))])).unwrap();
        until(&mut g, Part::DeclareBlockers, reversed);
        let s = soldiers(&g);
        assert_eq!(s.len(), 2);
        cite!(["702.91a"]; assert!(s.iter().all(|t| g.combat.attacking(*t))));
        powers.push(g.power(s[0]).unwrap());
        cite!(["702.91a"]; assert_eq!(g.power(s[0]), g.power(s[1])));
        cite!(["702.91a"]; assert_eq!(g.power(hero), Some(3), "battle cry is 'each other'"));
    }
    powers.sort();
    cite!(["702.91a"]; assert_eq!(powers, [1, 2]));
}

#[test]
fn r508_4_tokens_entering_attacking_never_attacked() {
    let mut g = game(2);
    let hero = g.put(0, HERO, Zone::Battlefield);
    g.put(0, WATCHER, Zone::Battlefield);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(hero, Target::Player(1))])).unwrap();
    until(&mut g, Part::DeclareBlockers, false);
    assert_eq!(soldiers(&g).len(), 2);
    cite!(["508.4"]; assert_eq!(g.players[0].life, 21, "only the hero attacked"));
}

#[test]
fn r508_4_the_attacking_players_own_planeswalker_is_never_offered() {
    let mut g = game(2);
    let hero = g.put(0, HERO, Zone::Battlefield);
    let mine = g.put(0, WALKER, Zone::Battlefield);
    g.put_counters(mine, "loyalty", 5);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(hero, Target::Player(1))])).unwrap();
    let offered = until(&mut g, Part::DeclareBlockers, false);
    cite!(["508.4"]; assert!(offered.is_empty(), "one defender only: {offered:?}"));
    cite!(["508.4"]; assert!(soldiers(&g).iter().all(|s| g.combat.attackers.contains(&(*s, Target::Player(1))))));
}

#[test]
fn r508_4_each_opponent_may_be_attacked_in_multiplayer() {
    let mut g = game(3);
    let hero = g.put(0, HERO, Zone::Battlefield);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(hero, Target::Player(1))])).unwrap();
    let offered = until(&mut g, Part::DeclareBlockers, false);
    cite!(["508.4"]; assert_eq!(offered.len(), 2, "one question per token"));
    cite!(["508.4"]; assert!(offered.iter().all(|o| o.contains(&Target::Player(1)) && o.contains(&Target::Player(2)) && !o.contains(&Target::Player(0))), "{offered:?}"));
}

#[test]
fn r508_4d_after_blockers_are_declared_a_token_enters_unblocked() {
    let mut g = game(2);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    g.put(1, BEAR, Zone::Battlefield);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(bear, Target::Player(1))])).unwrap();
    until(&mut g, Part::DeclareBlockers, false);
    while g.decision.is_some() { let Some(Decision::Blockers { player }) = g.decision.clone() else { panic!() }; g.act(player, Move::Block(vec![])).unwrap(); }
    let card = g.put(0, RALLY, Zone::Hand);
    while g.priority != 0 { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    let [soldier] = soldiers(&g)[..] else { panic!() };
    cite!(["508.4d"]; assert!(g.combat.attacking(soldier)));
    cite!(["508.4d"]; assert!(!g.combat.blocked.contains(&soldier)));
    until(&mut g, Part::EndOfCombat, false);
    cite!(["508.4d"]; assert_eq!(g.players[1].life, 20 - 2 - 1, "it dealt combat damage"));
}

#[test]
fn r508_4_before_blockers_a_token_entering_attacking_can_be_blocked() {
    let mut g = game(2);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let wall = g.put(1, BEAR, Zone::Battlefield);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(bear, Target::Player(1))])).unwrap();
    // in the declare attackers step, player 0 creates the token
    while g.priority != 0 || g.part != Part::DeclareAttackers { g.act(g.priority, Move::Pass).unwrap(); }
    let card = g.put(0, RALLY, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    let [soldier] = soldiers(&g)[..] else { panic!() };
    while g.decision != Some(Decision::Blockers { player: 1 }) { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(1, Move::Block(vec![(wall, soldier)])).unwrap();
    cite!(["508.4"]; assert!(g.combat.blocked.contains(&soldier)));
    until(&mut g, Part::EndOfCombat, false);
    cite!(["508.4"]; assert_eq!(g.players[1].life, 18, "only the bear connected"));
    cite!(["508.4"]; assert!(!g.battlefield.contains(&soldier), "the 1/1 died to the 2/2"));
}

#[test]
fn r508_4_attacking_ends_with_the_combat_phase() {
    let mut g = game(2);
    let hero = g.put(0, HERO, Zone::Battlefield);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(hero, Target::Player(1))])).unwrap();
    until(&mut g, Part::PostcombatMain, false);
    cite!(["508.4"]; assert!(soldiers(&g).iter().all(|s| !g.combat.attacking(*s))));
    assert_eq!(soldiers(&g).len(), 2);
}
