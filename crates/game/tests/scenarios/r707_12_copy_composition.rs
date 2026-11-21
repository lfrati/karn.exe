//! Cast card copies compose with permanent entry, copying, split halves, and double faces.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use game::state::{Answer, Choice, Decision};
use game::card::CastAs;

const DUPLICATE: &str = "Exile the top card of your library. Choose a card exiled this way and copy it. You may cast the copy without paying its mana cost.";
fn card(name: &str, cost: &str, kind: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    CardDef::new(name, cost, kind, text, pt).unwrap()
}
fn game(model: CardDef) -> Game {
    let defs = vec![card("Copy a card", "{0}", "Instant", DUPLICATE, None), model];
    for def in &defs { game::play::check(def).unwrap_or_else(|e| panic!("{}: {}", def.name(), e.0)); }
    Game::new(defs, 2, 20)
}
fn pass(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }
fn offered(g: &mut Game) -> ObjId {
    g.put(0, 1, Zone::Library);
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap(); pass(g);
    let Some(Decision::Choose { player: 0, choice: Choice::Cast { cards } }) = &g.decision else { panic!("{:?}", g.decision) };
    assert_eq!(cards.len(), 1);
    let copy = cards[0];
    assert!(!g.obj(copy).token() && g.obj(copy).card.is_none());
    assert!(!g.is(Target::Object(copy), ir::Sort::Card));
    copy
}
fn cast_copy(g: &mut Game, copy: ObjId, face: CastAs) {
    let action = Move::Cast { card: copy, targets: vec![] };
    g.act(0, if face == CastAs::Whole { action } else { Move::CastAs(face, Box::new(action)) }).unwrap();
    let spell = *g.stack.last().unwrap();
    assert!(g.is(Target::Object(spell), ir::Sort::Spell));
    assert!(!g.obj(spell).token() && !g.is(Target::Object(spell), ir::Sort::Card));
}
fn permanent(g: &Game) -> ObjId { *g.battlefield.iter().find(|o| g.obj(**o).token()).unwrap() }
fn activate(g: &mut Game, source: ObjId, ability: usize) {
    g.act(0, Move::Activate { source, ability, targets: vec![], paying: vec![] }).unwrap(); pass(g);
}

#[test]
fn r111_13_copied_permanent_spell_enters_as_a_token_with_replacements_triggers_and_abilities() {
    let model = card("Model", "{1}{G}", "Creature — Bear", "This creature enters with two +1/+1 counters on it.\nWhen this creature enters, you gain 3 life.\n{0}: You gain 5 life.", Some((1, 1)));
    let mut g = game(model); let copy = offered(&mut g); let clone = g.clone();
    for mut g in [g, clone] {
        cast_copy(&mut g, copy, CastAs::Whole); pass(&mut g);
        let token = permanent(&g);
        cite!(["111.13", "608.3f"]; assert_eq!(g.def(token).unwrap().name(), "Model"));
        assert_eq!(g.obj(token).chars.mana_value, 2);
        cite!(["614.1c"]; assert_eq!(g.obj(token).counters.get("+1/+1"), Some(&2)));
        assert_eq!(g.power(token), Some(3));
        cite!(["603.6a"]; assert_eq!(g.stack.len(), 1)); pass(&mut g); assert_eq!(g.players[0].life, 23);
        activate(&mut g, token, 2); assert_eq!(g.players[0].life, 28);
        cite!(["111.13"]; assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Created { .. })).count(), 0, "111.13: resolving a copy does not create a token"));
        assert_eq!(g.exile.iter().filter(|o| g.obj(**o).card == Some(1)).count(), 1);
        g.move_object(token, Zone::Graveyard);
        let departed = *g.players[0].graveyard.last().unwrap();
        assert_eq!(g.def(departed).unwrap().name(), "Model");
    }
}

#[test]
fn r614_12_copied_permanent_spell_entry_choice_suspends_and_resumes_without_replay() {
    let model = card("Chooser", "{2}{G}", "Creature — Elf", "As this creature enters, choose a color.\nWhen this creature enters, you gain 3 life.", Some((2, 2)));
    let mut g = game(model); let copy = offered(&mut g); cast_copy(&mut g, copy, CastAs::Whole); pass(&mut g);
    cite!(["614.12a"]; assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::Value(game::binding::Kind::Color), .. }))));
    cite!(["614.12a"]; assert!(g.battlefield.is_empty()));
    let clone = g.clone();
    for mut g in [g, clone] {
        g.act(0, Move::Choose(Answer::Value(game::binding::Value::Color(ir::Color::Red)))).unwrap();
        let token = permanent(&g);
        assert_eq!(g.def(token).unwrap().name(), "Chooser"); assert_eq!(g.battlefield.len(), 1);
        cite!(["603.6a"]; assert_eq!(g.stack.len(), 1)); pass(&mut g); assert_eq!(g.players[0].life, 23);
        assert!(g.stack.is_empty() && g.decision.is_none());
    }
}

#[test]
fn r707_5_copied_clone_spell_retains_its_entry_copy_overlay_and_own_token_baseline() {
    let clone = card("Clone", "{3}{U}", "Creature — Shapeshifter", "You may have this creature enter as a copy of any creature on the battlefield.", Some((0, 0)));
    let mut g = game(clone);
    std::sync::Arc::make_mut(&mut g.defs).push(card("Model", "{1}{G}", "Creature — Bear", "{0}: You gain 5 life.", Some((2, 2))));
    let source = g.put(0, 2, Zone::Battlefield);
    let copy = offered(&mut g); cast_copy(&mut g, copy, CastAs::Whole); pass(&mut g);
    g.act(0, Move::Choose(Answer::Objects(vec![source]))).unwrap();
    let token = permanent(&g);
    cite!(["707.5"]; assert_eq!(g.obj(token).chars.name, "Model")); assert_eq!(g.def(token).unwrap().name(), "Model");
    activate(&mut g, token, 0); assert_eq!(g.players[0].life, 25);
    g.move_object(token, Zone::Graveyard);
    let departed = *g.players[0].graveyard.last().unwrap();
    assert_eq!(g.def(departed).unwrap().name(), "Clone", "the entry copying effect ends; its creating baseline remains");
}

#[test]
fn r707_10g_cast_copy_of_double_faced_card_becomes_a_transforming_token() {
    let front = card("Front", "{3}{G}", "Creature — Human", "{0}: Transform this creature.", Some((2, 2)));
    let back = card("Back", "", "Creature — Werewolf", "{0}: You gain 7 life.", Some((4, 4)));
    let mut g = game(CardDef::transforming(front, back).unwrap());
    let copy = offered(&mut g); cast_copy(&mut g, copy, CastAs::Whole); pass(&mut g);
    let token = permanent(&g); assert_eq!(g.obj(token).chars.name, "Front");
    let clone = g.clone();
    for mut g in [g, clone] {
        activate(&mut g, token, 0);
        cite!(["707.10g"]; assert!(g.obj(token).transformed));
        cite!(["712.8e"]; assert_eq!(g.obj(token).chars.name, "Back")); assert_eq!(g.def(token).unwrap().name(), "Back");
        cite!(["712.8e"]; assert_eq!(g.obj(token).chars.mana_value, 4)); assert_eq!(g.power(token), Some(4));
        activate(&mut g, token, 0); assert_eq!(g.players[0].life, 27);
        g.move_object(token, Zone::Graveyard);
        let departed = *g.players[0].graveyard.last().unwrap();
        assert_eq!(g.def(departed).unwrap().name(), "Front");
    }
}

#[test]
fn r709_3c_split_card_copy_casts_only_the_chosen_half_with_its_own_mana_value() {
    for half in 0..2 {
        let left = card("Up", "{1}{W}", "Instant", "You gain 2 life.", None);
        let right = card("Down", "{3}{B}", "Sorcery", "You lose 3 life.", None);
        let mut g = game(CardDef::split([left, right]).unwrap());
        let copy = offered(&mut g);
        cite!(["709.3c", "709.4b"]; case: format!("half {half}"); assert_eq!(g.obj(copy).chars.mana_value, 6));
        cite!(["709.3"]; refused("choose one split half, or both with fuse (709.3, 702.102)"): g.act(0, Move::Cast { card: copy, targets: vec![] }));
        cast_copy(&mut g, copy, CastAs::Half(half));
        let spell = *g.stack.last().unwrap();
        cite!(["709.3b"]; case: format!("half {half}"); assert_eq!(g.def(spell).unwrap().name(), if half == 0 { "Up" } else { "Down" }));
        cite!(["709.3b"]; case: format!("half {half}"); assert_eq!(g.obj(spell).chars.mana_value, if half == 0 { 2 } else { 4 }));
        let clone = g.clone();
        for mut g in [g, clone] {
            pass(&mut g);
            assert_eq!(g.players[0].life, if half == 0 { 22 } else { 17 });
            assert!(g.stack.is_empty());
            assert!(g.players[0].graveyard.iter().all(|o| g.obj(*o).card.is_some()));
        }
    }
}

#[test]
fn r608_2c_empty_explicit_choice_does_not_copy_the_earlier_singleton_exile_result() {
    let text = DUPLICATE.replace("Choose a card", "Choose a creature card");
    let chooser = card("Copy a creature card", "{0}", "Instant", &text, None);
    game::play::check(&chooser).unwrap();
    let mut g = Game::new(vec![chooser, card("Reward", "{W}", "Instant", "You gain 2 life.", None)], 2, 20);
    g.put(0, 1, Zone::Library);
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap(); pass(&mut g);
    cite!(["101.3"]; assert!(g.stack.is_empty() && g.decision.is_none()));
    // the card's instruction exiles one; 101.3 only lets the rest still happen
    cite!(["101.3"]; assert_eq!(g.exile.len(), 1));
    // Uncited (card text): "Exile the top card of your library" exiles the Reward card itself.
    assert_eq!(g.obj(g.exile[0]).card, Some(1));
}
