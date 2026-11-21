//! What an object choice says it is for (608.2d) in more forms than
//! r608_2d_object_choice_verbs has, and the refusals of a wrong answer.
use crate::support::cite;
use game::state::{Answer, Choice, Decision, Placement};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::{Part, Verb};

fn game(texts: &[&str]) -> Game {
    let mut defs: Vec<CardDef> = texts.iter().enumerate().map(|(i, t)| CardDef::new(&format!("Spell{i}"), "{0}", "Sorcery", t, None).unwrap()).collect();
    defs.push(CardDef::new("Shock", "{R}", "Instant", "Shock deals 2 damage to any target.", None).unwrap());
    defs.push(CardDef::new("Swamp", "", "Basic Land — Swamp", "", None).unwrap());
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    let swamp = texts.len() + 1;
    for p in 0..2 { for _ in 0..5 { g.put(p, swamp, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn asked(g: &mut Game) -> (usize, Verb, Option<Placement>, Vec<ObjId>) {
    for _ in 0..20 {
        match g.decision.clone() {
            Some(Decision::Choose { player, choice: Choice::Objects { verb, to, among, .. } }) => return (player, verb, to, among),
            Some(Decision::Targets { player, .. }) => { g.act(player, Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap(); }
            Some(d) => panic!("{d:?}"),
            None => { g.act(g.priority, Move::Pass).unwrap(); }
        }
    }
    panic!("no object choice");
}
fn hand_of_shocks(g: &mut Game, shock: usize) -> Vec<ObjId> { (0..3).map(|_| g.put(1, shock, Zone::Hand)).collect() }

#[test]
fn review_chosen_then_exiled_says_exile() {
    let mut g = game(&["Target opponent reveals their hand. You choose a nonland card from it. Exile that card."]);
    let shocks = hand_of_shocks(&mut g, 1);
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Player(1)]] }).unwrap();
    let (player, verb, to, among) = asked(&mut g);
    cite!(["608.2d"]; assert_eq!(player, 0));
    cite!(["608.2d"]; assert_eq!(among, shocks));
    println!("exile form: verb={verb:?} to={to:?}");
    assert!(verb == Verb::Exile || to == Some(Placement::Zone(Zone::Exile)), "chosen to be exiled, said {verb:?} {to:?}");
}

#[test]
fn review_chosen_then_put_on_bottom_says_library_bottom() {
    let mut g = game(&["Target opponent reveals their hand. You choose a nonland card from it. That player puts that card on the bottom of their library."]);
    hand_of_shocks(&mut g, 1);
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Player(1)]] }).unwrap();
    let (_, verb, to, _) = asked(&mut g);
    assert_eq!((verb, to), (Verb::Move, Some(Placement::Library(ir::action::LibraryPosition::Bottom))));
}

#[test]
fn review_refusals_name_the_mistake() {
    let mut g = game(&["Target opponent reveals their hand. You choose a nonland card from it. That player discards that card."]);
    let shocks = hand_of_shocks(&mut g, 1);
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Player(1)]] }).unwrap();
    let (player, verb, _, _) = asked(&mut g);
    assert_eq!(verb, Verb::Discard);
    // an unoffered object, a repeat, too many, none
    cite!(["608.2d"]; refused("a chosen object is not among the offered objects to discard (608.2d)"): g.act(player, Move::Choose(Answer::Objects(vec![spell]))));
    cite!(["608.2d"]; refused("choose each object to discard once (608.2d)"): g.act(player, Move::Choose(Answer::Objects(vec![shocks[0], shocks[0]]))));
    cite!(["608.2d"]; refused("choose exactly 1 of them to discard (608.2d)"): g.act(player, Move::Choose(Answer::Objects(vec![shocks[0], shocks[1]]))));
    cite!(["608.2d"]; refused("choose exactly 1 of them to discard (608.2d)"): g.act(player, Move::Choose(Answer::Objects(vec![]))));
    crate::support::refused(g.act(1, Move::Choose(Answer::Objects(vec![shocks[0]]))), "that decision is another player's");
    g.act(player, Move::Choose(Answer::Objects(vec![shocks[1]]))).unwrap();
    cite!(["701.9b"]; assert!(g.players[1].graveyard.contains(&shocks[1]) || g.players[1].graveyard.iter().any(|o| g.obj(*o).card == Some(1))));
}
