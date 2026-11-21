//! Countering (701.6) removes a stack object without resolving it or refunding its costs.
use crate::support::{cite, paid};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::{Color, mana::ManaSymbol};

fn game() -> Game {
    let defs = vec![
        CardDef::new("Gift", "{U}", "Instant", "You gain 3 life.", None).unwrap(),
        CardDef::new("Denial", "{0}", "Instant", "Counter target spell.", None).unwrap(),
        CardDef::new("Bear", "{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Memory", "{0}", "Instant", "You gain 4 life.\nFlashback {0}", None).unwrap(),
        CardDef::new("Recall", "{0}", "Instant", "Return target spell to its owner's hand.", None).unwrap(),
        CardDef::new("Vessel", "{0}", "Artifact", "Sacrifice this artifact: You gain 7 life.", None).unwrap(),
        CardDef::new("Stifle", "{0}", "Instant", "Counter target activated or triggered ability.", None).unwrap(),
        CardDef::new("Nullify", "{0}", "Instant", "Counter target spell or ability.", None).unwrap(),
        CardDef::new("Healer", "{0}", "Creature — Human", "When this creature enters, you gain 2 life.", Some((1, 1))).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap_or_else(|u| panic!("{}: {}", def.name(), u.0)); }
    Game::new(defs, 2, 20)
}

fn cast(g: &mut Game, def: usize, targets: Vec<Vec<Target>>, spending: &str) -> ObjId {
    let card = g.put(g.priority, def, Zone::Hand);
    g.act(g.priority, paid(spending, Move::Cast { card, targets })).unwrap();
    *g.stack.last().unwrap()
}

fn pass(g: &mut Game) {
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
}

fn counter(g: &mut Game, def: usize, target: ObjId) {
    g.act(g.priority, Move::Pass).unwrap();
    cast(g, def, vec![vec![Target::Object(target)]], "");
    pass(g);
}

#[test]
fn r701_6a_countered_instant_does_not_resolve_and_goes_to_its_owners_graveyard() {
    let mut g = game();
    g.add_mana(0, ManaSymbol::Color(Color::Blue));
    let spell = cast(&mut g, 0, vec![], "U");
    counter(&mut g, 1, spell);
    assert!(g.stack.is_empty());
    cite!(["701.6a"]; assert_eq!(g.players[0].life, 20));
    cite!(["701.6a"]; assert_eq!(g.obj(g.players[0].graveyard[0]).chars.name, "Gift"));
    assert!(g.log.contains(&Event::Countered { object: spell }));
    cite!(["701.6a"]; assert!(!g.log.contains(&Event::Resolved { spell })));
}

#[test]
fn r701_6b_countering_does_not_refund_mana_or_sacrificed_costs() {
    let mut g = game();
    g.add_mana(0, ManaSymbol::Color(Color::Green));
    let spell = cast(&mut g, 2, vec![], "G");
    counter(&mut g, 1, spell);
    cite!(["701.6b"]; assert_eq!(g.players[0].pool.total(), 0));
    assert!(g.battlefield.is_empty());
    let vessel = g.put(0, 5, Zone::Battlefield);
    g.act(0, Move::Activate { source: vessel, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    let ability = g.stack[0];
    counter(&mut g, 6, ability);
    assert_eq!(g.obj(ability).zone, Zone::Gone);
    cite!(["701.6a"]; assert_eq!(g.players[0].life, 20));
    cite!(["701.6b"]; assert_eq!(g.players[0].graveyard.len(), 2, "the sacrificed Vessel stays there; no card is created for its ability"));
}

#[test]
fn r702_34a_countering_or_returning_a_flashback_spell_exiles_it() {
    for response in [1, 4] {
        let mut g = game();
        let card = g.put(0, 3, Zone::Graveyard);
        g.act(0, Move::CastVia(game::announcement::CastRoute::Flashback, Box::new(Move::Cast { card, targets: vec![] }))).unwrap();
        let spell = g.stack[0];
        counter(&mut g, response, spell);
        assert!(g.stack.is_empty());
        assert!(g.players[0].graveyard.is_empty());
        assert!(g.players[0].hand.is_empty());
        cite!(["702.34a"]; case: format!("{response}"); assert_eq!(g.obj(g.exile[0]).chars.name, "Memory"));
        assert_eq!(g.players[0].life, 20);
    }
}

#[test]
fn r701_6a_countering_a_triggered_ability_does_not_remove_its_source() {
    let mut g = game();
    cast(&mut g, 8, vec![], "");
    pass(&mut g);
    let creature = g.battlefield[0];
    let trigger = g.stack[0];
    counter(&mut g, 6, trigger);
    cite!(["701.6a"]; assert_eq!(g.battlefield, [creature]));
    assert!(g.stack.is_empty());
    cite!(["701.6a"]; assert_eq!(g.players[0].life, 20));
}

#[test]
fn r608_2b_a_counterspell_with_a_departed_target_does_not_resolve() {
    let mut g = game();
    let target = cast(&mut g, 3, vec![], "");
    let first = cast(&mut g, 1, vec![vec![Target::Object(target)]], "");
    cast(&mut g, 1, vec![vec![Target::Object(target)]], "");
    pass(&mut g);
    cite!(["701.6a"]; assert_eq!(g.stack, [first]));
    pass(&mut g);
    cite!(["608.2b"]; assert!(g.log.contains(&Event::Fizzled { spell: first })));
    assert_eq!(g.players[0].life, 20);
}

#[test]
fn r112_1_a_counterspell_cannot_target_a_card_still_in_hand() {
    let mut g = game();
    let card = g.put(0, 1, Zone::Hand);
    cite!(["115.1", "112.1"]; refused("an illegal target, Denial: it isn't a spell (115.1)"): g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(card)]] }));
    assert_eq!(g.players[0].hand, [card]);
    cite!(["112.1"]; assert!(g.stack.is_empty()));

}

#[test]
fn r701_6a_countering_refuses_non_stack_objects_in_text_and_execution() {
    for text in ["Counter target creature.", "Counter target player.", "Counter target land."] {
        assert_eq!(CardDef::new("Invalid","{0}","Instant",text,None).unwrap_err(),"text the grammar does not read");
    }
    let mut g = game();
    let permanent = g.put(0, 2, Zone::Battlefield);
    let card = g.put(0, 7, Zone::Hand);
    let before=format!("{g:?}");
    cite!(["115.1"]; refused("an illegal target, Bear: it is none of what the target allows: it isn't a spell; it isn't an ability (115.1)"): g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(permanent)]] }));
    assert_eq!(format!("{g:?}"),before);
    assert_eq!(g.battlefield, [permanent]);
}
