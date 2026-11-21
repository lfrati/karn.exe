//! Discard-to-entry replacements keep the causing occurrence and compose with entry (614, 701.9).
use crate::support::{cite, paid};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use game::state::{Answer, Choice, Decision};
use game::binding::{Kind, Value};
const REPLACE: &str = "If a spell or ability an opponent controls causes you to discard this card, put it onto the battlefield instead of putting it into your graveyard.";
fn card(name: &str, text: &str) -> CardDef {
    let d = CardDef::new(name, "{1}{G}{W}", "Creature — Elephant Soldier", text, Some((4,4))).unwrap();
    game::play::check(&d).unwrap(); d
}
fn fixture(discard: &str, extra: &str) -> Game {
    let defs = vec![
        card("Loxodon Smiter", &format!("This spell can't be countered.\n{REPLACE}{extra}")),
        CardDef::new("Discard", "{0}", "Sorcery", discard, None).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Captor", "{0}", "Creature — Human", "{0}: Target player discards a card.", Some((1,1))).unwrap(),
        CardDef::new("Looter", "{0}", "Creature — Human", "Discard a card: Draw a card.", Some((1,1))).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap(); }
    Game::new(defs, 3, 20)
}
fn resolve(g: &mut Game) { for _ in 0..3 { g.act(g.priority, Move::Pass).unwrap(); } }
fn cast(g: &mut Game, target: usize) {
    let c=g.put(0,1,Zone::Hand);
    g.act(0, Move::Cast { card:c, targets:vec![vec![Target::Player(target)]] }).unwrap();
    resolve(g);
}
fn discards(g: &Game) -> Vec<ObjId> { g.log.iter().filter_map(|e| match e { Event::Discarded { card, .. } => Some(*card), _=>None }).collect() }
#[test]
fn r614_1a_opponents_spell_changes_destination_but_still_discards() {
    let mut g=fixture("Target player discards a card.", "");
    let old=g.put(1,0,Zone::Hand);
    cast(&mut g,1);
    let now=g.battlefield[0];
    assert_eq!((g.obj(now).owner,g.obj(now).controller),(1,1));
    cite!(["614.1a"]; assert!(g.players[1].hand.is_empty() && g.players[1].graveyard.is_empty()));
    assert_eq!(g.obj(old).zone,Zone::Gone);
    assert_eq!(discards(&g),vec![now]);
    assert!(!g.log.iter().any(|e| matches!(e, Event::ZoneChanged { was, to:Zone::Graveyard, .. } if *was==old)));
}
#[test]
fn r614_1a_self_caused_and_rule_discards_go_to_the_graveyard() {
    let mut g=fixture("Target player discards a card.", "");
    g.put(0,0,Zone::Hand); cast(&mut g,0);
    assert!(g.battlefield.is_empty());
    cite!(["701.9a"]; assert_eq!(g.players[0].graveyard.len(),2));
    let c=g.put(1,0,Zone::Hand);g.discard(1,c);
    cite!(["701.9a"]; assert_eq!(g.players[1].graveyard.len(),1));
}
#[test]
fn r118_11_discard_as_activation_cost_is_not_caused_by_an_opponents_effect() {
    let mut g=fixture("Target player discards a card.", "");
    let c=g.put(0,0,Zone::Hand);let source=g.put(0,4,Zone::Battlefield);
    g.put(0,2,Zone::Library);
    g.act(0,Move::Activate{source,ability:0,targets:vec![],paying:vec![vec![c]]}).unwrap();
    resolve(&mut g);
    cite!(["701.9a"]; assert_eq!(g.players[0].graveyard.len(),1));
    assert_eq!(g.players[0].hand.len(),1);
    assert_eq!(g.battlefield,vec![source]);
}
#[test]
fn r113_7a_departed_ability_source_keeps_its_controller_as_discard_cause() {
    let mut g=fixture("Target player discards a card.", "");
    g.put(2,0,Zone::Hand);let source=g.put(0,3,Zone::Battlefield);
    g.act(0,Move::Activate{source,ability:0,targets:vec![vec![Target::Player(2)]],paying:vec![]}).unwrap();
    g.destroy(source);resolve(&mut g);
    cite!(["113.7a"]; assert_eq!(g.battlefield.len(),1));
    assert_eq!(g.obj(g.battlefield[0]).controller,2);
}
#[test]
fn r701_9a_mixed_simultaneous_discards_keep_each_destination_and_emit_once() {
    let mut g=fixture("Target player discards two cards.", "");
    g.put(1,0,Zone::Hand);g.put(1,2,Zone::Hand);cast(&mut g,1);
    cite!(["614.1a"]; assert_eq!(g.battlefield.len(),1));
    cite!(["701.9a"]; assert_eq!(g.players[1].graveyard.len(),1));
    assert_eq!(discards(&g).len(),2);
}
#[test]
fn r614_12a_discard_parent_waits_for_entry_choice_and_clones_do_not_replay() {
    let mut g=fixture("Target player discards two cards. You lose 2 life.", "\nAs this creature enters, choose a color.");
    let discarded=g.put(1,0,Zone::Hand);let land=g.put(1,2,Zone::Hand);
    cast(&mut g,1);
    cite!(["614.12a"]; assert_eq!(g.decision,Some(Decision::Choose{player:1,choice:Choice::Value(Kind::Color)})));
    assert_eq!(g.players[0].life,20);
    cite!(["614.12a"]; assert_eq!(g.players[1].hand,vec![discarded,land],"prepare every destination before committing the batch"));
    assert!(discards(&g).is_empty());
    for p in 0..3 { crate::support::refused(g.act(p,Move::Pass), "a decision is pending: a choice of how the permanent enters the battlefield (614.12)"); }
    let copy=g.clone();
    for (mut g,color) in [(g,ir::Color::Blue),(copy,ir::Color::Red)] {
        crate::support::refused(g.act(0,Move::Choose(Answer::Value(Value::Color(color)))), "that decision is another player's");
        g.act(1,Move::Choose(Answer::Value(Value::Color(color)))).unwrap();
        assert!(g.stack.is_empty() && g.decision.is_none());
        cite!(["608.2c"]; case: format!("{color:?}"); assert_eq!(g.players[0].life,18));
        assert_eq!(g.battlefield.len(),1);
        assert_eq!(g.players[1].graveyard.len(),1);
        assert_eq!(discards(&g).len(),2);
    }
}
#[test]
fn r608_2c_revealed_selection_feeds_discard_replacement_before_life_loss() {
    let mut g=fixture("Target player reveals their hand. You choose a nonland card from it. That player discards that card. You lose 2 life.", "");
    let smiter=g.put(1,0,Zone::Hand);g.put(1,3,Zone::Hand);
    cast(&mut g,1);
    g.act(0,Move::Choose(Answer::Objects(vec![smiter]))).unwrap();
    cite!(["614.1a"]; assert_eq!(g.battlefield.len(),1));
    cite!(["608.2c"]; assert_eq!(g.players[0].life,18));
    assert!(g.players[1].graveyard.is_empty());
    assert_eq!(g.players[1].hand.len(),1);
}

#[test]
fn r113_6g_unrestricted_cast_is_uncounterable_and_enters_normally() {
    let mut g=fixture("Target player discards a card.", "");
    let c=g.put(0,0,Zone::Hand);
    for mana in [ir::mana::ManaSymbol::Colorless,ir::mana::ManaSymbol::Color(ir::Color::Green),ir::mana::ManaSymbol::Color(ir::Color::White)] { g.add_mana(0,mana); }
    g.act(0,paid("CGW",Move::Cast{card:c,targets:vec![]})).unwrap();
    let spell=g.stack[0];
    cite!(["113.6g"]; assert!(!g.counter(spell)));
    resolve(&mut g);
    assert_eq!(g.battlefield.len(),1);
    assert!(discards(&g).is_empty());
}

#[test]
fn r614_1a_unsupported_substitute_instructions_are_not_silently_omitted() {
    for text in [
        REPLACE.replace("put it onto the battlefield", "put it onto the battlefield tapped"),
        REPLACE.replace("your graveyard.", "your graveyard. Draw a card."),
    ] {
        let d=CardDef::new("Unimplemented", "{1}{G}", "Creature — Elf", &text, Some((2,2))).unwrap();
        // Uncited (admission): only the complete supported substitution is registered.
        crate::support::refused(game::play::check(&d).map_err(|e|e.0),"ability on a permanent: sentence: Replacement");
    }
}
