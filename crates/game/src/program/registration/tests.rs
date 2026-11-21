use super::*;
use crate::state::Zone;

fn creature(name: &str, text: &str) -> CardDef {
    CardDef::new(name, "{1}", "Creature — Wall", text, Some((0, 4))).unwrap()
}

#[test]
fn r604_1_a_definition_keeps_each_static_restriction_lowered_or_its_refusal() {
    // one registration per statement, by the index of its printed ability
    let def = creature("Wall", "Flying\nThis creature can't block.\nCreatures you control attack each combat if able.");
    assert_eq!(def.restrictions.iter().map(|r| r.ability).collect::<Vec<_>>(), [1, 2]);
    assert_eq!(def.restriction_ok(0), None, "a keyword line is not a statement");
    assert_eq!(def.restriction_ok(1), Some(Ok(())));
    assert_eq!(def.restriction_ok(2), Some(Ok(())));
    assert_eq!(def.restriction_ok(3), None);
    let Ok(r) = &def.restrictions[0].result else { panic!("it is held in force") };
    assert_eq!((r.scope, &r.subject, r.acts.len()), (Scope::In(Zone::Battlefield), &Subject::Source, 1));
    let Ok(r) = &def.restrictions[1].result else { panic!("it is held in force") };
    assert!(matches!(r.subject, Subject::Described(_)));

    // what the engine does not hold in force is kept as its refusal, not dropped
    let def = creature("Wall", "This creature doesn't untap during its controller's next untap step.");
    assert_eq!(def.restrictions.len(), 1);
    assert_eq!(def.restriction_ok(0), Some(unsupported("a static ability about a next untap step")));
}

#[test]
fn r611_3a_a_kept_restriction_is_asked_of_the_game_as_it_is() {
    let wall = creature("Wall", "Creatures you control can't block.");
    let moat = creature("Moat", "This creature can't attack unless you control an Island.");
    let island = CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap();
    assert_eq!(moat.restriction_ok(0), Some(Ok(())));
    let mut game = Game::new(vec![wall, moat, island], 2, 20);
    let wall = game.put(0, 0, Zone::Battlefield);
    let theirs = game.put(1, 1, Zone::Battlefield);
    assert!(game.cant_block(wall, theirs, &[wall]));
    assert!(!game.cant_block(theirs, wall, &[theirs]));
    // a creature that arrives later is one of them (611.3a)
    let later = game.put(0, 1, Zone::Battlefield);
    assert!(game.cant_block(later, theirs, &[later]));

    // the condition is asked each time, of the permanent whose ability it is
    assert!(game.cant_attack(later, Target::Player(1)));
    assert!(game.cant_attack(theirs, Target::Player(0)));
    game.put(0, 2, Zone::Battlefield);
    assert!(!game.cant_attack(later, Target::Player(1)));
    assert!(game.cant_attack(theirs, Target::Player(0)));
}

#[test]
fn r702_a_keyword_whose_rule_is_a_sentence_is_lowered_once() {
    for name in ["Defender", "Flying", "Menace", "Indestructible", "Hexproof", "Shroud"] {
        let k = Keyword::from_name(name).unwrap();
        let first = keyword(k).unwrap_or_else(|| panic!("{name} states a restriction"));
        assert_eq!(first.len(), 1, "{name} retains its singleton rule");
        assert!(first.iter().all(Result::is_ok), "{name}: {first:?}");
        assert!(std::ptr::eq(first, keyword(k).unwrap()), "{name} is lowered once");
    }
    // asked where a rule of the game names it; no sentence, so no registration
    assert!(keyword(Keyword::from_name("Haste").unwrap()).is_none());
}

#[test]
fn r611_3_a_definition_keeps_what_each_static_ability_says_its_subject_is_or_its_refusal() {
    let def = creature("Wall", "Creatures you control get +1/+1.\nThis creature can't block.\nThis creature gets +1/+1 for each other creature you control.");
    assert_eq!(def.characteristics.iter().map(|r| r.ability).collect::<Vec<_>>(), [0, 2]);
    assert_eq!(def.characteristic_ok(0), Some(Ok(())));
    assert_eq!(def.characteristic_ok(1), None, "a restriction says nothing of what its subject is");
    assert_eq!(def.characteristic_ok(2), Some(Ok(())));
    assert_eq!(def.restriction_ok(0), None);
    let Ok(all) = &def.characteristics[0].result else { panic!("it is applied") };
    assert!(matches!(all.subject, Subject::Described(_)));
    // the layer a change is of is known without the game (613.1)
    assert!(all.changes_in(Layer::ChangePt) && !all.changes_in(Layer::Abilities) && !all.changes_in(Layer::SetPt));
    let Ok(own) = &def.characteristics[1].result else { panic!("it is applied") };
    assert_eq!(own.subject, Subject::Source);
    assert!(matches!(own.acts.as_slice(), [(When::Always, Scale::ForEach(_), changes)] if changes.len() == 1));

    // The whole ability layer uses live dependency ordering, not a separate dependent pass.
    let def = creature("Wall", "Creatures you control with flying have trample.");
    let Ok(dependent) = &def.characteristics[0].result else { panic!("it is applied") };
    assert!(dependent.changes_in(Layer::Abilities));

    // what the engine does not apply is kept as its refusal, not dropped
    let def = creature("Wall", "This creature gets +1/+1 for each creature you control with power 4 or greater.");
    assert_eq!(def.characteristic_ok(0), Some(unsupported("a static ability whose number is counted from power or toughness")));
}

#[test]
fn r611_3a_what_a_kept_statement_counts_is_counted_each_time_it_is_asked() {
    let each = creature("Each", "This creature gets +1/+1 for each other creature you control.");
    let hand = creature("Hand", "This creature gets +X/+X, where X is the number of cards in your hand.");
    let lord = creature("Lord", "Other creatures you control get +1/+1 as long as you control an Island.");
    let island = CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap();
    assert_eq!(hand.characteristic_ok(0), Some(Ok(())));
    let mut game = Game::new(vec![each, hand, lord, island], 2, 20);
    let each = game.put(0, 0, Zone::Battlefield);
    let hand = game.put(0, 1, Zone::Battlefield);
    assert_eq!(game.seen(each, None).pt, Some((1, 5)));
    assert_eq!(game.seen(hand, None).pt, Some((0, 4)));

    // the number is not locked in: a card drawn, a creature arriving
    game.put(0, 3, Zone::Hand);
    game.put(0, 3, Zone::Hand);
    assert_eq!(game.seen(hand, None).pt, Some((2, 6)));
    let lord = game.put(0, 2, Zone::Battlefield);
    assert_eq!(game.seen(each, None).pt, Some((2, 6)));
    // nor is the condition, asked of the permanent whose ability it is
    game.put(1, 3, Zone::Battlefield);
    assert_eq!(game.seen(lord, None).pt, Some((0, 4)));
    assert_eq!(game.seen(each, None).pt, Some((2, 6)));
    game.put(0, 3, Zone::Battlefield);
    assert_eq!(game.seen(each, None).pt, Some((3, 7)));
    assert_eq!(game.seen(hand, None).pt, Some((3, 7)));
    assert_eq!(game.seen(lord, None).pt, Some((0, 4)), "other creatures");
}

#[test]
fn r113_2c_composed_granting_acts_retain_distinct_compiled_instances() {
    // Compose two known IR actions before immutable definition compilation. The surface
    // grammar normally combines adjacent grants into one list; component identity must
    // also hold when a programmatically generated ability contains distinct acts.
    let def = creature("Lord", "Creatures have \"{T}: Add {G}.\"");
    let mut ability = def.abilities()[0].clone();
    let Ability::Text(ir::Effect(sentences)) = &mut ability else { panic!("statement") };
    let ir::Sentence::Do(body) = &mut sentences[0] else { panic!("statement body") };
    body.first.rest.push((ir::effect::Join::And, body.first.first.clone()));
    let source = CardDef::of_ability(ability, vec![]);
    let mut game = Game::new(vec![source, creature("Subject", "")], 2, 20);
    game.put(0, 0, Zone::Battlefield);
    let subject = game.put(0, 1, Zone::Battlefield);
    let grants = game.seen(subject, None).given;
    assert_eq!(grants.len(), 2);
    assert_eq!(grants[0].1, grants[1].1);
    assert_ne!(grants[0].0, grants[1].0);
}

#[test]
fn r702_61_split_second_registers_both_subjects_at_the_stack_scope() {
    let k = Keyword::from_name("Split second").unwrap();
    let rules = keyword(k).unwrap();
    assert_eq!(rules.len(), 2);
    assert!(rules.iter().all(|r| r.as_ref().is_ok_and(|r| r.scope == Scope::In(Zone::Stack))));
    assert!(matches!(rules[0].as_ref().unwrap().subject, Subject::Described(_)));
    assert!(matches!(rules[1].as_ref().unwrap().subject, Subject::SourceFilter(_)));
    assert!(std::ptr::eq(rules, keyword(k).unwrap()));
}

#[test]
fn r702_keyword_rule_compile_failure_is_retained_beside_a_valid_contribution() {
    static NOT_A_RULE: Ability = Ability::Keywords(Vec::new());
    let def = creature("Rules", "");
    let support = Support::definition(&def, None);
    let declared = crate::keyword::Declared { needs: vec![], gives: vec![
        (Scope::In(Zone::Battlefield), crate::keyword::Gives::Rule(
            crate::keyword::meaning(Keyword::from_name("Defender").unwrap()).unwrap())),
        (Scope::In(Zone::Stack), crate::keyword::Gives::Rule(&NOT_A_RULE)),
    ] };
    let rules = keyword_rules(&declared, &support);
    assert_eq!(rules.len(), 2, "the failed contribution must not disappear");
    assert!(rules[0].is_ok());
    assert_eq!(rules[1].as_ref().unwrap_err().0, "keyword rule: no restriction");
}
