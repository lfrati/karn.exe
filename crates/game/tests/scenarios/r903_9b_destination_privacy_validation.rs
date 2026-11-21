//! 903.9b's destination choice and hidden information.
//! A player who draws their commander may put it into the command zone instead (903.9b). If
//! they decline, it is an ordinary card drawn into a hidden hand (402.3, 121.1): no other
//! player learns that the card drawn was the commander. Choosing the command zone is public,
//! because the command zone is (400.2).
use crate::support::cite;
use game::snapshot::Snapshot;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Zone};

fn setup() -> (Game, ObjId) {
    let defs = vec![
        CardDef::new("Leader", "{0}", "Legendary Creature — Human", "", Some((2, 2))).unwrap(),
        CardDef::new("Peek", "{0}", "Sorcery", "Draw a card.", None).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap(); }
    let mut g = Game::new(defs, 2, 40);
    for p in 0..2 { for _ in 0..12 { g.put(p, 2, Zone::Library); } }
    let card = g.put(0, 0, Zone::Command);
    g.designate_commander(card).unwrap();
    g.move_object(card, Zone::Library);
    let top = *g.players[0].library.last().unwrap();
    (g, top)
}
/// Player 0 casts "Draw a card." with their commander on top, up to the 903.9b question.
fn draw_the_commander() -> (Game, ObjId) {
    let (mut g, top) = setup();
    let spell = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::CommandZone { object: top } }));
    (g, top)
}

#[test]
fn r402_3_a_declined_commander_draw_tells_the_opponent_nothing() {
    // what the opponent is told while the owner decides: no card, no choice made
    let (mut g, top) = draw_the_commander();
    let seen = Snapshot::of(&g, Some(1));
    assert!(!seen.objects.iter().any(|o| o.id == top), "the card being drawn is hidden from the opponent");
    g.act(0, Move::Choose(Answer::No)).unwrap();
    // the draw completes into the hidden hand
    let drawn = *g.players[0].hand.iter().find(|o| g.obj(**o).card == Some(0)).expect("drawn");
    let seen = Snapshot::of(&g, Some(1));
    cite!(["402.3"]; assert!(!seen.objects.iter().any(|o| o.id == drawn), "the drawn card is hidden"));
    cite!(["402.3"]; assert!(seen.choices_made.is_empty(), "a declined return publishes nothing: {:?}", seen.choices_made));
}

#[test]
fn r402_3_a_declined_return_stays_private_while_the_same_draw_asks_again() {
    // "Draw two cards." with the commander on top and a dredge card in the graveyard: the
    // second draw asks about dredge while the first draw's declined return is still part of
    // the same operation
    let (mut g, top) = setup();
    let defs = std::sync::Arc::make_mut(&mut g.defs);
    defs.push(CardDef::new("Two", "{0}", "Sorcery", "Draw two cards.", None).unwrap());
    defs.push(CardDef::new("Dredger", "{0}", "Creature — Zombie", "Dredge 1", Some((1, 1))).unwrap());
    let (two, dredger) = (defs.len() - 2, defs.len() - 1);
    g.put(0, dredger, Zone::Graveyard);
    let spell = g.put(0, two, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    // the first draw may be dredged too: decline it, then decline the return
    while !matches!(g.decision, Some(Decision::Choose { choice: Choice::CommandZone { .. }, .. })) {
        g.act(0, Move::Choose(Answer::No)).unwrap();
    }
    assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::CommandZone { object: top } }));
    g.act(0, Move::Choose(Answer::No)).unwrap();
    assert!(g.decision.is_some(), "the second draw asks about dredge: {:?}", g.decision);
    let seen = Snapshot::of(&g, Some(1));
    cite!(["402.3"]; assert!(seen.choices_made.is_empty(), "while the draw is pending, the opponent is told: {:?}", seen.choices_made));
}

#[test]
fn r400_2_choosing_the_command_zone_is_public() {
    let (mut g, _) = draw_the_commander();
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    let leader = *g.command.iter().find(|o| g.obj(**o).card == Some(0)).expect("in the command zone");
    cite!(["903.9b", "903.3"]; assert!(g.is_commander(leader)));
    cite!(["400.2"]; assert!(Snapshot::of(&g, Some(1)).objects.iter().any(|o| o.id == leader), "the command zone is public"));
}

#[test]
fn r402_3_declining_a_hidden_hand_to_library_return_does_not_publish_the_hand() {
    let (mut g, top) = setup();
    g.move_object(top, Zone::Hand);
    let commander = g.players[0].hand[0];
    let other = g.put(0, 2, Zone::Hand);
    let defs = std::sync::Arc::make_mut(&mut g.defs);
    defs.push(CardDef::new("Arrange hand", "{0}", "Sorcery", "Put two cards from your hand on top of your library in any order.", None).unwrap());
    let definition = defs.len()-1;
    game::play::check(&defs[definition]).unwrap();
    let spell = g.put(0, definition, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    // With exactly two cards the selection may be automatic.
    if !matches!(g.decision, Some(Decision::Choose { choice: Choice::CommandZone { .. }, .. })) {
        g.act(0, Move::Choose(Answer::Objects(vec![commander,other]))).unwrap();
    }
    cite!(["903.9b"]; assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::CommandZone { .. }, .. }))));
    g.act(0, Move::Choose(Answer::No)).unwrap();
    assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::OrderCards { ref cards, .. }, .. }) if cards.len() == 2));
    let seen = Snapshot::of(&g, Some(1));
    cite!(["402.3"]; assert!(seen.choices_made.is_empty()));
    cite!(["402.3"]; assert!(!seen.objects.iter().any(|o|o.id==commander || o.id==other)));
    g.act(0, Move::Choose(Answer::Objects(vec![other,commander]))).unwrap();
    assert!(g.players[0].hand.is_empty());
}
