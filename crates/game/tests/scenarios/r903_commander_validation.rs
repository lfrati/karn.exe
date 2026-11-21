//! The Commander foundation, through real game flows.
//! 603.10a/603.6c: a commander's "when this dies" ability triggers on the death itself and
//! resolves whether or not its owner then puts the card into the command zone (704.6d).
//! 701.20d, 401.4: a commander in a library that is looked at and reordered becomes a new
//! object, still that physical commander (903.3). 903.10a: only combat damage dealt to a
//! player counts, through the ordinary combat damage step, not damage to a planeswalker.
use crate::support::cite;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const LEADER: usize = 0;
const DESTROY: usize = 1;
const INDEX: usize = 2;
const ISLAND: usize = 3;
const TITAN: usize = 4;
const WALKER: usize = 5;

fn game() -> Game {
    let card = |name: &str, cost: &str, types: &str, text: &str, pt: Option<(i32, i32)>| {
        let d = CardDef::new(name, cost, types, text, pt).unwrap();
        game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        d
    };
    let walker = CardDef::planeswalker("Walker", "{0}", "Planeswalker — Jace", "+1: You gain 1 life.", 20).unwrap();
    let defs = vec![
        card("Leader", "{2}", "Legendary Creature — Human", "When this creature dies, you gain 2 life.", Some((2, 2))),
        card("Destroy", "{0}", "Instant", "Destroy target creature.", None),
        card("Index", "{0}", "Sorcery", "Look at the top three cards of your library, then put them back in any order.", None),
        card("Island", "", "Basic Land — Island", "", None),
        card("Titan", "{0}", "Legendary Creature — Giant", "", Some((11, 11))),
        walker,
    ];
    let mut g = Game::new(defs, 2, 40);
    for p in 0..2 { for _ in 0..20 { g.put(p, ISLAND, Zone::Library); } }
    g
}
/// A designated commander of player 0, moved from the command zone to `zone`.
fn commander(g: &mut Game, def: usize, zone: Zone) -> ObjId {
    let c = g.put(0, def, Zone::Command);
    g.designate_commander(c).unwrap();
    g.move_object(c, zone);
    *match zone { Zone::Battlefield => &g.battlefield, Zone::Library => &g.players[0].library, _ => unreachable!() }.last().unwrap()
}
fn resolve(g: &mut Game) { while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); } }

#[test]
fn r603_10a_a_commanders_dies_trigger_resolves_whether_or_not_it_returns() {
    for to_command in [true, false] {
        let mut g = game();
        let leader = commander(&mut g, LEADER, Zone::Battlefield);
        let destroy = g.put(0, DESTROY, Zone::Hand);
        g.act(0, Move::Cast { card: destroy, targets: vec![vec![Target::Object(leader)]] }).unwrap();
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        let Some(Decision::Choose { player: 0, choice: Choice::CommandZone { object } }) = g.decision.clone() else { panic!("{:?}", g.decision) };
        cite!(["704.6d"]; case: to_command; assert_eq!(g.obj(object).zone, Zone::Graveyard, "it died first"));
        g.act(0, Move::Choose(if to_command { Answer::Yes } else { Answer::No })).unwrap();
        resolve(&mut g);
        cite!(["603.10a", "603.6c"]; case: to_command; assert_eq!(g.players[0].life, 42, "the dies trigger resolved (to command zone: {to_command})"));
        let home = if to_command { &g.command } else { &g.players[0].graveyard };
        cite!(["903.3"]; case: to_command; assert!(home.iter().any(|o| g.is_commander(*o)), "still the commander (to command zone: {to_command})"));
    }
}

#[test]
fn r701_20d_a_commander_reordered_in_its_library_is_a_new_object_and_still_the_commander() {
    let mut g = game();
    let leader = commander(&mut g, LEADER, Zone::Library);
    assert_eq!(g.players[0].library.last(), Some(&leader), "on top");
    let top=g.players[0].library.len();
    let order=g.players[0].library[top-3..].to_vec(); // the two Islands above the commander
    let index = g.put(0, INDEX, Zone::Hand);
    g.act(0, Move::Cast { card: index, targets: vec![] }).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    let Some(Decision::Choose { player: 0, choice: Choice::OrderCards { cards: among, .. } }) = g.decision.clone() else { panic!("{:?}", g.decision) };
    assert_eq!(among.len(),3);
    g.act(0, Move::Choose(Answer::Objects(order))).unwrap();
    resolve(&mut g);
    let now = *g.players[0].library.iter().find(|o| g.obj(**o).card == Some(LEADER)).unwrap();
    cite!(["701.20d"]; assert_ne!(now, leader, "a new object (701.20d)"));
    cite!(["903.3"]; assert!(g.is_commander(now) && g.clone().is_commander(now)));
    // the old handle is a departed incarnation, not the card in the library
    assert_eq!(g.obj(leader).zone, Zone::Gone);
}

/// Pass to player 0's next declare-attackers decision on a later turn, declining others'.
fn next_attack(g: &mut Game) {
    let turn = g.turn;
    loop {
        match g.decision.clone() {
            Some(Decision::Attackers { player }) if player == 0 && g.turn > turn => return,
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            _ => g.act(g.decision_maker(g.priority), Move::Pass).unwrap(),
        }
    }
}
/// Through blockers and combat damage, to the end of combat.
fn finish_combat(g: &mut Game) {
    let turn = g.turn;
    while g.turn == turn && g.part != ir::Part::PostcombatMain {
        if g.finished() { return; }
        match g.decision.clone() {
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            _ => g.act(g.decision_maker(g.priority), Move::Pass).unwrap(),
        }
    }
}

#[test]
fn r903_10a_combat_damage_through_combat_counts_only_when_dealt_to_the_player() {
    let mut g = game();
    let titan = commander(&mut g, TITAN, Zone::Battlefield);
    let walker = g.put(1, WALKER, Zone::Battlefield);
    g.as_entered(walker);
    // an attack on the planeswalker: 11 combat damage to it, none to its controller
    next_attack(&mut g);
    g.act(0, Move::Attack(vec![(titan, Target::Object(walker))])).unwrap();
    finish_combat(&mut g);
    cite!(["306.8"]; assert_eq!(g.players[1].life, 40));
    // two attacks on the player: 11, then 22 combat damage from this commander
    next_attack(&mut g);
    g.act(0, Move::Attack(vec![(titan, Target::Player(1))])).unwrap();
    finish_combat(&mut g);
    cite!(["903.10a"]; assert_eq!((g.players[1].life, g.players[1].lost), (29, false), "11, and the planeswalker's 11 did not count"));
    next_attack(&mut g);
    g.act(0, Move::Attack(vec![(titan, Target::Player(1))])).unwrap();
    finish_combat(&mut g);
    cite!(["704.6c"]; assert!(g.players[1].lost, "22 combat damage from one commander (704.6c)"));
    assert_eq!(g.players[1].life, 18, "lost to commander damage, not life");
}
