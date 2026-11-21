//! "Whenever you cast your second spell each turn" and "whenever you cast a spell, if it's the
//! second creature spell you cast this turn" (603.2, 608.2i): the event is the n-th of its
//! kind its player has done in the turn, counted among the casts noted as they happen.
use crate::support::{cite, paid};
use game::state::Answer;
use game::{CardDef, Game, Move, ObjId, Zone};

const VINE: usize = 0;
const BEAR: usize = 1;
const SIP: usize = 2;
const SECOND: usize = 3;
const LAND: usize = 4;

fn game() -> Game {
    let defs = vec![
        CardDef::new("Vengevine", "{2}{G}{G}", "Creature — Elemental", "Haste\nWhenever you cast a spell, if it's the second creature spell you cast this turn, you may return this card from your graveyard to the battlefield.", Some((4, 3))).unwrap(),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "Flash", Some((2, 2))).unwrap(),
        CardDef::new("Sip", "{G}", "Instant", "You gain 1 life.", None).unwrap(),
        CardDef::new("Second", "{1}", "Artifact", "Whenever you cast your second spell each turn, you lose 1 life.", None).unwrap(),
        CardDef::new("Wastes", "", "Land", "", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    let mut g = Game::new(defs, 2, 20);
    for player in 0..2 { for _ in 0..8 { g.put(player, LAND, Zone::Library); } }
    g
}
/// This player casts this card with mana given for it; everything on the stack resolves,
/// each "you may" answered as said.
fn resolve(g: &mut Game, player: usize, card: usize, may: Answer, spending: &str) {
    let spell = g.put(player, card, Zone::Hand);
    g.players[player].pool.colors = [9, 9, 9, 9, 9];
    while g.priority != player { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(player, paid(spending, Move::Cast { card: spell, targets: vec![] })).unwrap();
    while !g.stack.is_empty() {
        match g.asked() {
            Some((who, game::legal::Asked::Priority)) => g.act(who, Move::Pass).unwrap(),
            Some((who, game::legal::Asked::Order(abilities))) => {
                // The duplicate-Vengevine case has two interchangeable triggers;
                // explicitly put the first listed below the second (603.3b).
                assert_eq!(who, player);
                assert_eq!(abilities.len(), 2);
                assert!(abilities.iter().all(|(source, _)| g.obj(source.expect("Vengevine source")).chars.name == "Vengevine"));
                g.act(player, Move::Order(vec![0, 1])).unwrap();
            }
            Some((who, game::legal::Asked::May)) => {
                assert_eq!(who, player);
                g.act(who, Move::Choose(may.clone())).unwrap();
            }
            other => panic!("unexpected spell-resolution boundary: {other:?}"),
        }
    }
    g.players[player].pool = Default::default();
}
fn on_battlefield(g: &Game, name: &str) -> Vec<ObjId> {
    g.battlefield.iter().copied().filter(|o| g.obj(*o).chars.name == name).collect()
}
fn next_turn(g: &mut Game) {
    let turn = g.turn;
    while g.turn == turn {
        match g.asked() {
            Some((player, game::legal::Asked::Priority)) => g.act(player, Move::Pass).unwrap(),
            Some((player, game::legal::Asked::Attackers(_))) => g.act(player, Move::Attack(vec![])).unwrap(),
            other => panic!("unexpected next-turn boundary: {other:?}"),
        }
    }
}

#[test]
fn vengevine_returns_as_the_second_creature_spell_of_the_turn_is_cast() {
    let mut g = game();
    g.put(0, VINE, Zone::Graveyard);
    resolve(&mut g, 0, BEAR, Answer::Yes, "GG");
    assert!(on_battlefield(&g, "Vengevine").is_empty(), "the first creature spell");
    resolve(&mut g, 0, SIP, Answer::Yes, "G");
    cite!(["603.4"]; assert!(on_battlefield(&g, "Vengevine").is_empty(), "the second spell, and no creature spell"));
    resolve(&mut g, 1, BEAR, Answer::Yes, "GG");
    assert!(on_battlefield(&g, "Vengevine").is_empty(), "another player's creature spell is not one you cast");
    resolve(&mut g, 0, BEAR, Answer::Yes, "GG");
    cite!(["603.4", "113.6k"]; assert_eq!(on_battlefield(&g, "Vengevine").len(), 1, "the second creature spell you cast this turn"));
    assert!(g.players[0].graveyard.iter().all(|o| g.obj(*o).chars.name != "Vengevine"));
}

#[test]
fn only_the_second_is_the_second_and_only_in_its_turn() {
    let mut g = game();
    g.put(0, VINE, Zone::Graveyard);
    resolve(&mut g, 0, BEAR, Answer::Yes, "GG");
    // it may stay where it is
    resolve(&mut g, 0, BEAR, Answer::No, "GG");
    cite!(["603.5"]; assert!(on_battlefield(&g, "Vengevine").is_empty()));
    // the third is not the second
    resolve(&mut g, 0, BEAR, Answer::Yes, "GG");
    cite!(["603.4"]; assert!(on_battlefield(&g, "Vengevine").is_empty()));
    // the count is of one turn: in the next, the first is the first again
    next_turn(&mut g);
    resolve(&mut g, 0, BEAR, Answer::Yes, "GG");
    assert!(on_battlefield(&g, "Vengevine").is_empty());
    resolve(&mut g, 0, BEAR, Answer::Yes, "GG");
    assert_eq!(on_battlefield(&g, "Vengevine").len(), 1);
}

#[test]
fn each_vengevine_in_the_graveyard_triggers() {
    let mut g = game();
    g.put(0, VINE, Zone::Graveyard);
    g.put(0, VINE, Zone::Graveyard);
    g.put(1, VINE, Zone::Graveyard);
    resolve(&mut g, 0, BEAR, Answer::Yes, "GG");
    resolve(&mut g, 0, BEAR, Answer::Yes, "GG");
    cite!(["113.6k"]; assert_eq!(on_battlefield(&g, "Vengevine").len(), 2));
    assert_eq!(g.players[1].graveyard.len(), 1, "its owner cast nothing");
}

#[test]
fn your_second_spell_each_turn_is_counted_for_you_alone() {
    let mut g = game();
    g.put(0, SECOND, Zone::Battlefield);
    resolve(&mut g, 0, SIP, Answer::Yes, "G");
    assert_eq!(g.players[0].life, 21);
    resolve(&mut g, 1, SIP, Answer::Yes, "G");
    resolve(&mut g, 1, SIP, Answer::Yes, "G");
    assert_eq!(g.players[0].life, 21, "their second, not yours");
    resolve(&mut g, 0, SIP, Answer::Yes, "G");
    cite!(["603.2"]; assert_eq!(g.players[0].life, 21, "yours: 1 gained, 1 lost"));
    resolve(&mut g, 0, SIP, Answer::Yes, "G");
    cite!(["603.2"]; assert_eq!(g.players[0].life, 22, "the third"));
}
