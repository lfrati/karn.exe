//! 708.2/708.12 composition checks.
use crate::support::cite;
use crate::support::*;

#[test]
fn r708_2_an_unrevealed_face_down_land_is_a_creature_card() {
    let procedure = CardDef::new(
        "Procedure",
        "{0}",
        "Instant",
        "Tap target face-down permanent. If it's a creature card, you gain 3 life.",
        None,
    )
    .unwrap();
    game::play::check(&procedure).unwrap();
    let mut t = Board::new(2).battlefield(1, &["Forest"]).fill(8).build();
    let land = t.find_in_battlefield("Forest");
    assert!(t.g.turn_face_down(land));
    let def = t.g.defs.len();
    std::sync::Arc::make_mut(&mut t.g.defs).push(procedure);
    let card = t.g.put(0, def, Zone::Hand);
    t.g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(land)]] }).unwrap();
    t.resolve();
    assert!(t.g.obj(land).tapped);
    cite!(["708.2"]; assert_eq!(
        t.g.players[0].life, 23,
        "708.2: no reveal instruction invokes 708.12; the face-down card is a creature"
    ));
}

#[test]
fn r708_12_a_revealed_card_qualifier_reads_its_printed_mana_value() {
    let procedure = CardDef::new(
        "Procedure",
        "{0}",
        "Instant",
        "Reveal target face-down permanent. If it's a creature card with mana value 2 or greater, you gain 3 life.",
        None,
    )
    .unwrap();
    game::play::check(&procedure).unwrap();
    let mut t = Board::new(2).battlefield(1, &["Grizzly Bears"]).fill(8).build();
    let bear = t.find_in_battlefield("Grizzly Bears");
    assert!(t.g.turn_face_down(bear));
    let def = t.g.defs.len();
    std::sync::Arc::make_mut(&mut t.g.defs).push(procedure);
    let card = t.g.put(0, def, Zone::Hand);
    t.g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(bear)]] }).unwrap();
    t.resolve();
    cite!(["708.12"]; assert_eq!(t.g.players[0].life, 23, "708.12: the revealed card has mana value 2, not the face-down 0"));
}
