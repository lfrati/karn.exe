//! A token "with" a keyword that is more than a flag: a triggered keyword (prowess), or a
//! parameter (mountainwalk). It has that ability as a card printed with it would (111.3), so
//! it triggers and restricts as one.
use crate::support::{cite, paid};
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Zone};

fn real(name: &str) -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let d = CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
    d
}
fn def(name: &str, cost: &str, types: &str, pt: Option<(i32, i32)>) -> CardDef { CardDef::new(name, cost, types, "", pt).unwrap() }
fn resolve(g: &mut Game) {
    for _ in 0..20 {
        if g.stack.is_empty() && g.decision.is_none() { return; }
        match g.decision.clone() {
            None => g.act(g.priority, Move::Pass).unwrap(),
            Some(Decision::Order { player, abilities }) => g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(),
            Some(d) => panic!("{d:?}"),
        }
    }
}
fn tokens(g: &Game, name: &str) -> Vec<ObjId> { g.battlefield.iter().copied().filter(|o| g.obj(*o).card.is_none() && g.obj(*o).chars.name == name).collect() }

/// Monastery Mentor: "Whenever you cast a noncreature spell, create a 1/1 white Monk creature
/// token with prowess." The Monk's prowess triggers on the next noncreature spell (702.108a).
#[test]
fn r702_108a_a_monk_token_with_prowess_triggers_on_the_next_noncreature_spell() {
    let mut g = Game::new(vec![real("Monastery Mentor"), def("Ornament", "{0}", "Artifact", None), def("Bear", "{0}", "Creature — Bear", Some((2, 2)))], 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, 2, Zone::Library); } }
    g.put(0, 0, Zone::Battlefield);
    let card = g.put(0, 2, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    resolve(&mut g);
    assert!(tokens(&g, "Monk Token").is_empty(), "a creature spell");
    let card = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    resolve(&mut g);
    let [monk] = tokens(&g, "Monk Token")[..] else { panic!() };
    cite!(["111.3"]; assert!(g.has(monk, "Prowess")));
    assert_eq!(g.power(monk), Some(1));
    let card = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    let Some(Decision::Order { abilities, .. }) = &g.decision else { panic!("{:?}", g.decision) };
    cite!(["702.108a"]; assert_eq!(abilities.len(), 3, "the Mentor's two triggers and the Monk's prowess"));
    resolve(&mut g);
    assert_eq!(tokens(&g, "Monk Token").len(), 2);
    cite!(["702.108a"]; assert_eq!((g.power(monk), g.toughness(monk)), (Some(2), Some(2))));
}

/// Goblin Scouts: "Create three 1/1 red Goblin Scout creature tokens with mountainwalk."
/// Next turn, a defending player with a Mountain can't block them (702.14c, 509.1b).
#[test]
fn r702_14c_goblin_tokens_with_mountainwalk_cant_be_blocked_by_a_mountain_player() {
    let mut g = Game::new(vec![real("Goblin Scouts"), def("Mountain", "", "Basic Land — Mountain", None), def("Bear", "{0}", "Creature — Bear", Some((2, 2)))], 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, 1, Zone::Library); } }
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors = [5; 5];
    g.players[0].pool.colorless = 5;
    g.act(0, paid("CCCRR", Move::Cast { card, targets: vec![] })).unwrap();
    resolve(&mut g);
    let goblins = tokens(&g, "Goblin Scout Token");
    assert_eq!(goblins.len(), 3);
    let mountain = g.put(1, 1, Zone::Battlefield);
    let bear = g.put(1, 2, Zone::Battlefield);
    // to player 0's next turn's attack
    let turn = g.turn;
    for _ in 0..200 {
        if g.turn == turn + 2 && g.decision == Some(Decision::Attackers { player: 0 }) { break; }
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            None => g.act(g.priority, Move::Pass).unwrap(),
            Some(d) => panic!("{d:?}"),
        }
    }
    assert_eq!((g.turn, g.decision.clone()), (turn + 2, Some(Decision::Attackers { player: 0 })));
    let blocks = |mut g: Game| {
        g.act(0, Move::Attack(vec![(goblins[0], game::Target::Player(1))])).unwrap();
        while g.part != ir::Part::DeclareBlockers { g.act(g.priority, Move::Pass).unwrap(); }
        if g.decision != Some(Decision::Blockers { player: 1 }) { return false; }
        g.act(1, Move::Block(vec![(bear, goblins[0])])).is_ok()
    };
    cite!(["702.14c", "509.1b"]; assert!(!blocks(g.clone()), "nothing the defender has can block it"));
    g.destroy(mountain);
    cite!(["702.14c"]; assert!(blocks(g), "without a Mountain, the Bear blocks it"));
}
