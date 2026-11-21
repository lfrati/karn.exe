//! Populate (701.36): create a token that's a copy of a creature token you control, chosen as
//! the instruction is followed (701.36a, 608.2d). With no creature token, nothing is created
//! (701.36b).
use crate::support::cite;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, Target, Zone};

const POPULATE: usize = 0;
const SOLDIER: usize = 1;
const BEAR: usize = 2;
const SPIRIT: usize = 3;
const STEAL: usize = 4;
const TROSTANI: usize = 5;
const GIFT: usize = 6;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap();
    let c = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).expect(name);
    let defs = vec![
        c("Populate", "Sorcery", "Populate.", None),
        c("Soldiers", "Sorcery", "Create a 1/1 white Soldier creature token.", None),
        c("Bear", "Creature — Bear", "", Some((2, 2))),
        c("Spirits", "Sorcery", "Create a 2/2 white Spirit creature token with flying.", None),
        c("Steal", "Sorcery", "Gain control of target creature.", None),
        real("Trostani's Judgment"),
        c("Gift", "Sorcery", "Each opponent creates a 1/1 white Soldier creature token.", None),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    Game::new(defs, 2, 20)
}
fn cast(g: &mut Game, player: usize, card: usize, targets: Vec<Vec<Target>>) {
    let spell = g.put(player, card, Zone::Hand);
    g.players[player].pool.colors = [9; 5];
    g.players[player].pool.colorless = 9;
    g.act(player, crate::support::paid(if card == TROSTANI { "CCCCCW" } else { "" }, Move::Cast { card: spell, targets })).unwrap();
    while !g.stack.is_empty() && g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
}
fn tokens(g: &Game, player: usize) -> Vec<String> {
    let mut names: Vec<_> = g.battlefield.iter().filter(|o| g.obj(**o).token() && g.controller(**o) == player)
        .map(|o| g.obj(*o).chars.name.clone()).collect();
    names.sort();
    names
}

#[test]
fn r701_36a_the_only_creature_token_is_copied() {
    let mut g = game();
    cast(&mut g, 0, SOLDIER, vec![]);
    g.put(0, BEAR, Zone::Battlefield);
    cast(&mut g, 0, POPULATE, vec![]);
    assert!(g.decision.is_none());
    cite!(["701.36a"]; assert_eq!(tokens(&g, 0), ["Soldier Token", "Soldier Token"]));
}

#[test]
fn r701_36a_its_controller_chooses_which_token() {
    let mut g = game();
    cast(&mut g, 0, SOLDIER, vec![]);
    cast(&mut g, 0, SPIRIT, vec![]);
    cast(&mut g, 0, POPULATE, vec![]);
    let Some(Decision::Choose { player: 0, choice: Choice::Objects { verb: ir::Verb::CreateCopy, among, count: 1, .. } }) = g.decision.clone() else { panic!("{:?}", g.decision) };
    cite!(["701.36a"]; assert_eq!(among.len(), 2));
    let spirit = *among.iter().find(|o| g.obj(**o).chars.name.starts_with("Spirit")).unwrap();
    cite!(["701.36a"]; refused("that decision is another player's"): g.act(1, Move::Choose(Answer::Objects(vec![spirit]))));
    g.act(0, Move::Choose(Answer::Objects(vec![spirit]))).unwrap();
    cite!(["701.36a"]; assert_eq!(tokens(&g, 0), ["Soldier Token", "Spirit Token", "Spirit Token"]));
    let copy = *g.battlefield.iter().rev().find(|o| g.obj(**o).chars.name.starts_with("Spirit")).unwrap();
    assert_ne!(copy, spirit);
    cite!(["111.2"]; assert_eq!(g.obj(copy).owner, 0));
}

#[test]
fn r701_36b_no_creature_token_creates_nothing() {
    let mut g = game();
    g.put(0, BEAR, Zone::Battlefield);
    // an opponent's token is not one you control
    cast(&mut g, 0, GIFT, vec![]);
    cast(&mut g, 0, POPULATE, vec![]);
    assert!(g.decision.is_none());
    cite!(["701.36b"]; assert_eq!(tokens(&g, 0), Vec::<String>::new()));
    cite!(["701.36a"]; assert_eq!(tokens(&g, 1), ["Soldier Token"]));
}

#[test]
fn r701_36a_a_controlled_token_owned_by_another_is_copied_for_you() {
    let mut g = game();
    cast(&mut g, 0, GIFT, vec![]);
    let theirs = *g.battlefield.iter().find(|o| g.obj(**o).token()).unwrap();
    cast(&mut g, 0, STEAL, vec![vec![Target::Object(theirs)]]);
    cast(&mut g, 0, POPULATE, vec![]);
    cite!(["701.36a"]; assert_eq!(tokens(&g, 0), ["Soldier Token", "Soldier Token"]));
    let copy = *g.battlefield.iter().find(|o| **o != theirs && g.obj(**o).token()).unwrap();
    cite!(["111.2"]; assert_eq!(g.obj(copy).owner, 0));
}

#[test]
fn r701_36a_trostanis_judgment_exiles_then_populates() {
    let mut g = game();
    cast(&mut g, 0, SOLDIER, vec![]);
    let bear = g.put(1, BEAR, Zone::Battlefield);
    cast(&mut g, 0, TROSTANI, vec![vec![Target::Object(bear)]]);
    cite!(["608.2c"]; assert!(g.exile.contains(&bear) || g.obj(bear).zone == Zone::Gone || !g.battlefield.contains(&bear)));
    cite!(["701.36a"]; assert_eq!(tokens(&g, 0), ["Soldier Token", "Soldier Token"]));
}
