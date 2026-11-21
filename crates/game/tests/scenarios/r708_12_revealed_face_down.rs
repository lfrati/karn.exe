//! What an effect that reveals a face-down permanent asks of it, it asks of the card, ignoring
//! continuous effects (708.12): printed types, power, mana value, a chosen type. A description
//! the printed card can't answer is refused rather than asked of the face-down 2/2.
use crate::support::*;

fn reveal_then(text: &str, creature: &str) -> T {
    let procedure = CardDef::new("Procedure", "{0}", "Instant", text, None).unwrap();
    game::play::check(&procedure).unwrap_or_else(|e| panic!("{text}: {}", e.0));
    let mut t = Board::new(2).battlefield(1, &[creature]).fill(8).build();
    let down = t.find_in_battlefield(creature);
    assert!(t.g.turn_face_down(down));
    let def = t.g.defs.len();
    std::sync::Arc::make_mut(&mut t.g.defs).push(procedure);
    let card = t.g.put(0, def, Zone::Hand);
    t.g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(down)]] }).unwrap();
    t.resolve();
    t
}

#[test]
fn r708_12_a_revealed_card_is_asked_its_printed_power() {
    let text = "Reveal target face-down permanent. If it's a creature card with power 4 or greater, you gain 3 life.";
    cite!(["708.12"]; assert_eq!(reveal_then(text, "Shivan Dragon").g.players[0].life, 23, "Shivan Dragon is printed 5/5"));
    cite!(["708.12"]; assert_eq!(reveal_then(text, "Grizzly Bears").g.players[0].life, 20, "Grizzly Bears is printed 2/2, as is the face-down 2/2"));
}

#[test]
fn r708_12_a_card_condition_beyond_printed_characteristics_is_refused() {
    let d = CardDef::new("Procedure", "{0}", "Instant",
        "Reveal target face-down permanent. If it's a creature card with flying, you gain 3 life.", None).unwrap();
    let refused = game::play::check(&d).map_err(|e| e.0).expect_err("abilities are not asked of the card as printed here");
    assert!(refused.contains("printed"), "{refused}");
}
