//! The first Pauper target's Blood tokens use ordinary definitions, costs and programs.
use crate::support::{cite, paid};
use game::{CardDef, Event, Game, Move, ObjId, Zone};
use ir::{Color, mana::ManaSymbol};

fn pass(g: &mut Game) {
    for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); }
}

fn setup() -> (Game, ObjId) {
    let epicure = CardDef::new("Voldaren Epicure", "{R}", "Creature — Vampire",
        "When this creature enters, it deals 1 damage to each opponent. Create a Blood token. (It's an artifact with \"{1}, {T}, Discard a card, Sacrifice this token: Draw a card.\")", Some((1, 1))).unwrap();
    game::play::check(&epicure).unwrap();
    let island = CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap();
    let mut g = Game::new(vec![epicure, island], 3, 20);
    let card = g.put(0, 0, Zone::Hand);
    g.add_mana(0, ManaSymbol::Color(Color::Red));
    g.act(0, paid("R", Move::Cast { card, targets: vec![] })).unwrap();
    pass(&mut g);
    pass(&mut g);
    assert_eq!(g.players.iter().map(|p| p.life).collect::<Vec<_>>(), vec![20, 19, 19]);
    let blood = g.log.iter().find_map(|e| if let Event::Created { token } = e { Some(*token) } else { None }).unwrap();
    assert_eq!(g.obj(blood).chars.predefined, Some(ir::Predefined::Blood));
    (g, blood)
}

fn activate(blood: ObjId, discard: ObjId) -> Move {
    paid("C", Move::Activate { source: blood, ability: 0, targets: vec![], paying: vec![vec![discard]] })
}
#[test]
fn r111_10g_blood_pays_discards_and_sacrifices_before_drawing_on_resolution() {
    let (mut g, blood) = setup();
    g.put(0, 1, Zone::Library); // The one Island drawn by this activation.
    let discard = g.put(0, 1, Zone::Hand);
    g.add_mana(0, ManaSymbol::Colorless);
    g.act(0, activate(blood, discard)).unwrap();
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.players[0].pool.total(), 0);
    cite!(["602.2b"]; assert_eq!(g.obj(blood).zone, Zone::Gone));
    assert_eq!(g.players[0].graveyard.len(), 1);
    cite!(["602.2a"]; assert_eq!(g.stack.len(), 1, "drawing uses the stack, unlike a mana ability"));
    let mut other = g.clone();
    pass(&mut other);
    assert_eq!(other.players[0].hand.len(), 1);
    assert!(g.players[0].hand.is_empty());
    pass(&mut g);
    cite!(["111.10g"]; assert_eq!(g.players[0].hand.len(), 1));
    assert_eq!(g.players[0].graveyard.len(), 1, "resolving does not repeat payment");
}

#[test]
fn r118_3_blood_requires_mana_untapped_source_and_an_owned_hand_card() {
    let (mut g, blood) = setup();
    g.add_mana(0, ManaSymbol::Colorless);
    let wrong = g.put(1, 1, Zone::Hand);
    cite!(["701.9a"]; refused("Island is not what that cost asks for or cannot pay it (701.9a, 118.3)"): g.act(0, activate(blood, wrong)));
    assert_eq!(g.players[0].pool.total(), 1);
    assert_eq!(g.obj(blood).zone, Zone::Battlefield);
    assert!(!g.obj(blood).tapped);
    let discard = g.put(0, 1, Zone::Hand);
    let mut tapped = g.clone();
    tapped.tap(blood);
    cite!(["107.5"]; refused("Blood Token can't pay {T} as it is (107.5, 118.3)"): tapped.act(0, activate(blood, discard)));
    assert_eq!(tapped.players[0].hand, vec![discard]);
    let mut no_mana = g.clone();
    no_mana.players[0].pool = Default::default();
    cite!(["118.3"]; refused("that mana is not in your mana pool (118.3a)"): no_mana.act(0, activate(blood, discard)));
    assert!(!no_mana.obj(blood).tapped);
    assert_eq!(no_mana.players[0].hand, vec![discard]);
}
