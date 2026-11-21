//! 707.8a token creation copies both faces; 707.8 entry copying still takes one face.
//! Mana value follows the copied front cost (202.3b, 712.8e).
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use game::state::{Answer, Choice, Decision};

const MODEL: usize = 0;
const COPY: usize = 1;
const CLONE: usize = 2;
const PUMP: usize = 3;
const STRIP: usize = 4;
const COPY_ENTRY: &str = "You may have this creature enter as a copy of any creature on the battlefield.";
const TURN: &str = "{0}: Transform this creature.";
const MULTIPLY: &str = "{0}: Create a token that's a copy of this creature. You gain 1 life.";
fn card(name: &str, cost: &str, kind: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    CardDef::new(name, cost, kind, text, pt).unwrap()
}
fn model(front_text: &str, back_text: &str) -> CardDef {
    CardDef::transforming(
        card("Front", "{3}{G}", "Creature — Human", front_text, Some((2, 2))),
        card("Back", "", "Creature — Werewolf", back_text, Some((4, 4))),
    ).unwrap()
}
fn setup(model: CardDef) -> Game {
    let defs = vec![model,
        card("Duplicate", "{0}", "Instant", "Create a token that's a copy of target creature.", None),
        card("Clone", "{0}", "Creature — Shapeshifter", COPY_ENTRY, Some((0, 0))),
        card("Pump", "{0}", "Instant", "Target creature gets +5/+5 until end of turn.", None),
        card("Strip", "{0}", "Instant", "Target creature loses all abilities until end of turn.", None),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    Game::new(defs, 2, 20)
}
fn game() -> Game { setup(model(&format!("{TURN}\n{MULTIPLY}"), &format!("{TURN}\n{MULTIPLY}\nTrample"))) }
fn pass(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }
fn cast(g: &mut Game, def: usize, target: Option<ObjId>) {
    let card = g.put(0, def, Zone::Hand);
    let targets = target.map(|o| vec![vec![Target::Object(o)]]).unwrap_or_default();
    g.act(0, Move::Cast { card, targets }).unwrap();
    pass(g);
}
fn token(g: &Game) -> ObjId { *g.battlefield.iter().rev().find(|o| g.obj(**o).token()).unwrap() }
fn duplicate(g: &mut Game, source: ObjId) -> ObjId { cast(g, COPY, Some(source)); token(g) }
fn activate(g: &mut Game, source: ObjId, ability: usize) {
    g.act(0, Move::Activate { source, ability, targets: vec![], paying: vec![] }).unwrap();
}
fn face(g: &Game, object: ObjId, back: bool) {
    assert_eq!(g.obj(object).transformed, back);
    assert_eq!(g.obj(object).chars.name, if back { "Back" } else { "Front" });
    assert_eq!(g.def(object).unwrap().name(), if back { "Back" } else { "Front" });
    assert_eq!(g.power(object), Some(if back { 4 } else { 2 }));
    assert_eq!(g.obj(object).chars.mana_value, 4);
}

#[test]
fn r707_8a_created_token_copies_both_faces_and_enters_with_the_same_face_up() {
    for back in [false, true] {
        let mut g = game();
        let source = g.put(0, MODEL, Zone::Battlefield);
        if back { assert!(g.transform(source, None)); }
        let copy = duplicate(&mut g, source);
        face(&g, copy, back);
        assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Created { .. })).count(), 1);
        cite!(["707.8a"]; case: back; assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Transformed { object } if *object == copy)).count(), 0, "entering back up is not transforming"));
        activate(&mut g, copy, 0); pass(&mut g);
        face(&g, copy, !back);
        assert!(g.battlefield.contains(&copy), "transformation keeps its identity");
        cite!(["707.8a", "701.27a"]; case: back; assert_eq!(g.has(copy, "Trample"), !back));
        activate(&mut g, copy, 0); pass(&mut g);
        face(&g, copy, back);
    }
}

#[test]
fn r707_8a_copy_of_a_transforming_token_retains_both_faces_independently_of_sources() {
    let mut g = game();
    let original = g.put(0, MODEL, Zone::Battlefield);
    let first = duplicate(&mut g, original);
    assert!(g.transform(first, None));
    let second = duplicate(&mut g, first);
    face(&g, second, true);
    g.move_object(first, Zone::Graveyard);
    g.move_object(original, Zone::Hand);
    g.act(0, Move::Pass).unwrap();
    g.retire_events(g.log.end()); g.collect_departed();
    let clone = g.clone();
    for mut g in [g, clone] {
        cite!(["707.8a"]; assert!(g.transform(second, None))); face(&g, second, false);
        assert!(g.transform(second, None)); face(&g, second, true);
    }
}

#[test]
fn r608_2h_departed_double_faced_source_supplies_its_last_face_and_both_baselines() {
    let mut g = game();
    let source = g.put(0, MODEL, Zone::Battlefield);
    assert!(g.transform(source, None));
    activate(&mut g, source, 1);
    g.move_object(source, Zone::Hand);
    pass(&mut g);
    let copy = token(&g);
    face(&g, copy, true);
    assert_eq!(g.players[0].life, 21);
    cite!(["707.8a"]; assert!(g.transform(copy, None))); face(&g, copy, false);
}

#[test]
fn r707_2_copy_keeps_face_baselines_without_counters_status_or_later_layers() {
    let mut g = game();
    let source = g.put(0, MODEL, Zone::Battlefield);
    g.put_counters(source, "+1/+1", 3); g.tap(source); g.damage(source, Target::Object(source), 1);
    cast(&mut g, PUMP, Some(source)); cast(&mut g, STRIP, Some(source));
    assert_eq!(g.power(source), Some(10));
    let copy = duplicate(&mut g, source);
    face(&g, copy, false);
    cite!(["707.2"]; assert!(g.obj(copy).counters.is_empty() && !g.obj(copy).tapped && g.obj(copy).damage == 0));
    activate(&mut g, copy, 0); pass(&mut g); face(&g, copy, true);
    assert_eq!(g.power(source), Some(10));
}

#[test]
fn r707_6_token_entry_uses_face_up_choices_and_triggers_and_resumes_clones_once() {
    let front = format!("As this creature enters, choose a color.\nWhen this creature enters, you gain 2 life.\n{MULTIPLY}");
    let back = format!("As this creature enters, choose a creature type.\nWhen this creature enters, you gain 3 life.\n{MULTIPLY}");
    for back_up in [false, true] {
        let mut g = setup(model(&front, &back));
        let source = g.put(0, MODEL, Zone::Battlefield);
        if back_up { assert!(g.transform(source, None)); }
        activate(&mut g, source, 2); pass(&mut g);
        let expected = if back_up { game::binding::Kind::CreatureType } else { game::binding::Kind::Color };
        cite!(["707.6"]; case: back_up; assert!(matches!(&g.decision, Some(Decision::Choose { player: 0, choice: Choice::Value(kind) }) if *kind == expected)));
        assert_eq!(g.battlefield.len(), 1); assert_eq!(g.players[0].life, 20);
        let clone = g.clone();
        for mut g in [g, clone] {
            let value = if back_up { game::binding::Value::CreatureType(ir::Subtype::from_name("Elf").unwrap()) }
                else { game::binding::Value::Color(ir::Color::Blue) };
            g.act(0, Move::Choose(Answer::Value(value))).unwrap();
            let copy = token(&g); face(&g, copy, back_up);
            assert_eq!(g.players[0].life, 21, "the creator's next instruction executes once");
            cite!(["707.6"]; assert_eq!(g.stack.len(), 1, "only the entering face's entry trigger"));
            pass(&mut g);
            assert_eq!(g.players[0].life, if back_up { 24 } else { 23 });
            assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Created { .. })).count(), 1);
        }
    }
}

#[test]
fn r707_8_single_faced_clone_of_a_token_back_still_has_one_face_and_zero_mana_value() {
    let mut g = game();
    let source = g.put(0, MODEL, Zone::Battlefield);
    assert!(g.transform(source, None));
    let transformed_token = duplicate(&mut g, source);
    cast(&mut g, CLONE, None);
    g.act(0, Move::Choose(Answer::Objects(vec![transformed_token]))).unwrap();
    let clone = *g.battlefield.last().unwrap();
    cite!(["707.8"]; assert_eq!(g.obj(clone).chars.name, "Back"));
    cite!(["202.3b", "712.8e"]; assert_eq!(g.obj(clone).chars.mana_value, 0));
    cite!(["707.8", "701.27a"]; assert!(!g.transform(clone, None)));
    let clone_token = duplicate(&mut g, clone);
    assert_eq!(g.obj(clone_token).chars.name, "Back");
    cite!(["202.3b"]; assert_eq!(g.obj(clone_token).chars.mana_value, 0));
    assert!(!g.transform(clone_token, None));
    face(&g, transformed_token, true);
}

#[test]
fn r707_8a_an_entry_copy_replacement_does_not_give_a_token_new_physical_faces() {
    for double in [false, true] {
        let copier = card("Copier", "{2}{U}", "Creature — Shapeshifter", COPY_ENTRY, Some((2, 2)));
        let copier = if double { CardDef::transforming(copier, card("Reverse", "", "Creature", "", Some((1, 1)))).unwrap() } else { copier };
        let mut g = setup(copier);
        let witness = model(TURN, TURN);
        std::sync::Arc::make_mut(&mut g.defs).push(witness);
        let target = g.put(0, g.defs.len() - 1, Zone::Battlefield);
        assert!(g.transform(target, None));
        let source = g.put(0, MODEL, Zone::Battlefield);
        cast(&mut g, COPY, Some(source));
        g.act(0, Move::Choose(Answer::Objects(vec![target]))).unwrap();
        let copy = token(&g);
        assert_eq!(g.obj(copy).chars.name, "Back"); assert_eq!(g.obj(copy).chars.mana_value, 0);
        cite!(["701.27a"]; case: double; assert_eq!(g.transform(copy, None), double));
        assert_eq!(g.obj(copy).chars.name, "Back", "turning physical faces does not end a copy effect");
        let descendant = duplicate(&mut g, copy);
        assert_eq!(g.obj(descendant).chars.name, "Back");
        cite!(["202.3b"]; case: double; assert_eq!(g.obj(descendant).chars.mana_value, 0));
        cite!(["707.8a"]; case: double; assert_eq!(g.transform(descendant, None), double, "copy effects modify both copied token faces"));
        assert_eq!(g.obj(descendant).chars.name, "Back");
    }
}

#[test]
fn r712_8a_departing_transforming_token_has_its_front_characteristics_until_cessation() {
    let mut g = game();
    let source = g.put(0, MODEL, Zone::Battlefield);
    assert!(g.transform(source, None));
    let copy = duplicate(&mut g, source);
    g.move_object(copy, Zone::Graveyard);
    let departed = *g.players[0].graveyard.last().unwrap();
    face(&g, departed, false);
    assert_eq!(g.obj(copy).chars.name, "Back", "departed battlefield identity keeps LKI");
    g.move_object(departed, Zone::Battlefield);
    cite!(["111.8"]; assert_eq!(g.obj(departed).zone, Zone::Graveyard, "a departed token cannot return"));
    g.act(0, Move::Pass).unwrap();
    cite!(["704.5d"]; assert!(!g.players[0].graveyard.contains(&departed)));
}

#[test]
fn r701_27f_transforming_token_obeys_the_captured_self_ability_timestamp() {
    let mut g = game();
    let source = g.put(0, MODEL, Zone::Battlefield);
    let copy = duplicate(&mut g, source);
    activate(&mut g, copy, 0); activate(&mut g, copy, 0);
    let clone = g.clone();
    for mut g in [g, clone] {
        pass(&mut g); pass(&mut g);
        face(&g, copy, true);
        cite!(["701.27f"]; assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Transformed { object } if *object == copy)).count(), 1));
    }
}

/// A token that copies a transforming permanent has both faces (707.8a), each with its own
/// characteristics (712.8). Its front face's "{0}: Transform this creature." activated twice
/// waits on the stack while the first transforms it; that ability "exists on the stack
/// independently of its source" (113.7a) and "has the text of the ability that created it"
/// (405.4): the front's line, not the back's "{1}: Transform this creature." now up. The
/// prompt says "this creature" by the name its source had as the ability was put on the
/// stack, as for any ability (201.5): "{0}: Transform Front.".
#[test]
fn r405_4_a_transformed_tokens_waiting_ability_keeps_its_front_faces_text() {
    let mut g = setup(model(TURN, "{1}: Transform this creature.\nTrample"));
    let source = g.put(0, MODEL, Zone::Battlefield);
    let copy = duplicate(&mut g, source);
    activate(&mut g, copy, 0); activate(&mut g, copy, 0);
    let waiting = g.stack[0];
    pass(&mut g);
    face(&g, copy, true);
    assert_eq!(g.stack, [waiting]);
    for seat in 0..g.players.len() {
        let p = game::simulation::Simulation::new(g.clone()).prompt(seat).unwrap();
        let occurrence = p.entities[&waiting].occurrence.as_ref().unwrap();
        cite!(["113.7a", "405.4", "707.8a"]; assert_eq!(occurrence.text.as_deref(), Some("{0}: Transform Front."), "p{seat}"));
        cite!(["602.2a"]; assert_eq!(occurrence.created_by, None, "p{seat}: an activated ability, which no ability created"));
    }
}

#[test]
fn r707_8a_full_huntmaster_token_uses_both_faces_and_back_up_creation_is_not_transformation() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let huntmaster = CardDef::from_faces(&catalog["Huntmaster of the Fells"]).unwrap();
    let mut g = setup(huntmaster);
    std::sync::Arc::make_mut(&mut g.defs).push(card("Turner", "{0}", "Instant", "Transform target creature.", None));
    let turner = g.defs.len() - 1;
    let source = g.put(0, MODEL, Zone::Battlefield);
    let copy = duplicate(&mut g, source);
    assert_eq!(g.stack.len(), 1); pass(&mut g);
    assert_eq!(g.players[0].life, 22);
    cast(&mut g, turner, Some(copy));
    assert!(matches!(g.decision, Some(Decision::Targets { player: 0, .. })));
    g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)], vec![]])).unwrap();
    let clone = g.clone();
    for mut g in [g, clone] {
        pass(&mut g);
        assert_eq!(g.players[1].life, 18);
        assert_eq!(g.obj(copy).chars.name, "Ravager of the Fells");
        cite!(["712.8e"]; assert_eq!(g.obj(copy).chars.mana_value, 4));
        let back_copy = duplicate(&mut g, copy);
        cite!(["707.8a"]; assert!(g.stack.is_empty(), "copying a back-up permanent is neither a transform nor a front-face entry"));
        cite!(["707.8a"]; assert_eq!(g.obj(back_copy).chars.name, "Ravager of the Fells"));
        assert_eq!(g.players[1].life, 18);
        cast(&mut g, turner, Some(back_copy)); pass(&mut g);
        assert_eq!(g.obj(back_copy).chars.name, "Huntmaster of the Fells");
        assert_eq!(g.players[0].life, 24);
        assert_eq!(g.battlefield.iter().filter(|o| g.obj(**o).chars.name == "Wolf Token").count(), 2);
    }
}
