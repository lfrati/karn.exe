//! Bounded untap: "Untap up to three lands."
//! is a choice made as the instruction is carried out (608.2d) among the lands on the
//! battlefield, any player's, token or card, by the controller of the spell; a spell whose
//! only target is illegal does not resolve (608.2b), so nothing is chosen or untapped.
use crate::support::cite;
use game::legal::Asked;
use game::state::Answer;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::{Color, mana::ManaSymbol};

const FRANTIC: usize = 0;
const REWIND: usize = 1;
const ISLAND: usize = 2;
const SEED: usize = 3;
const WITNESS: usize = 4;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let oracle = |name: &str| {
        let d = CardDef::from_faces(CardDef::catalog(&faces).get(name).unwrap()).unwrap();
        game::play::check(&d).unwrap();
        d
    };
    let card = |name: &str, cost: &str, types: &str, text: &str| {
        let d = CardDef::new(name, cost, types, text, None).unwrap();
        game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        d
    };
    let defs = vec![
        oracle("Frantic Search"),
        oracle("Rewind"),
        card("Island", "", "Basic Land — Island", ""),
        card("Seed", "{0}", "Sorcery", "Create a tapped Forest land token."),
        card("Witness", "{0}", "Instant", "You gain 1 life."),
    ];
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    g
}
fn resolve_top(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }
fn tapped_land(g: &mut Game, p: usize) -> ObjId { let o = g.put(p, ISLAND, Zone::Battlefield); g.set_tapped(o, true); o }

#[test]
fn r608_2d_a_land_token_is_among_the_lands_and_can_be_untapped() {
    let mut g = game();
    let seed = g.put(0, SEED, Zone::Hand);
    g.act(0, Move::Cast { card: seed, targets: vec![] }).unwrap();
    resolve_top(&mut g);
    let token = *g.battlefield.iter().find(|o| g.obj(**o).card.is_none()).expect("a Forest token");
    assert!(g.obj(token).tapped);
    let card_land = tapped_land(&mut g, 1);
    let drawn:Vec<_>=g.players[0].library.iter().rev().take(2).copied().collect();
    let frantic = g.put(0, FRANTIC, Zone::Hand);
    for _ in 0..3 { g.add_mana(0, ManaSymbol::Color(Color::Blue)); }
    g.act(0, crate::support::paid("UUU", Move::Cast { card: frantic, targets: vec![] })).unwrap();
    resolve_top(&mut g);
    // The two draws have new identities in hand; their definitions and order originate in the fixed library.
    assert_eq!(drawn.len(),2);
    let discarded=g.players[0].hand.clone();
    cite!(["404.3"]; ok: g.act(0,Move::Choose(Answer::Objects(discarded))));
    // the hand is the two cards drawn: they are discarded without a selection
    let Some((0, Asked::Select { among, min: 0, max: 2 })) = g.asked() else { panic!("{:?}", g.asked()) };
    cite!(["608.2d"]; assert!(among.contains(&token) && among.contains(&card_land), "{among:?}"));
    g.act(0, Move::Choose(Answer::Objects(vec![token, card_land]))).unwrap();
    cite!(["701.26b"]; assert!(!g.obj(token).tapped && !g.obj(card_land).tapped));
}

#[test]
fn r608_2b_rewind_whose_target_left_the_stack_untaps_nothing() {
    let mut g = game();
    let lands: Vec<ObjId> = (0..4).map(|_| tapped_land(&mut g, 0)).collect();
    let witness = g.put(1, WITNESS, Zone::Stack);
    let rewind = g.put(0, REWIND, Zone::Hand);
    g.players[0].pool.colors[1] = 4;
    g.act(0, crate::support::paid("UUUU", Move::Cast { card: rewind, targets: vec![vec![Target::Object(witness)]] })).unwrap();
    // the target leaves the stack before Rewind resolves
    g.move_object(witness, Zone::Graveyard);
    resolve_top(&mut g);
    cite!(["608.2b"]; assert!(!matches!(g.asked(), Some((_, Asked::Select { .. }))), "no untap question: {:?}", g.asked()));
    cite!(["608.2b"]; assert!(lands.iter().all(|o| g.obj(*o).tapped), "nothing untapped"));
    cite!(["608.2b"]; assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).card == Some(REWIND))));
}
