use super::*;
use crate::state::PlayerId;

fn card(name: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    CardDef::new(name, "", types, text, pt).expect(name)
}

fn creature(name: &str, text: &str) -> CardDef {
    card(name, "Creature — Bear", text, Some((2, 2)))
}

/// What the definition keeps of its one triggered ability.
fn template(def: &CardDef) -> &Template {
    let [registered] = def.triggers.as_slice() else { panic!("{}: one triggered ability", def.name) };
    registered.result.as_ref().expect("lowered")
}

/// This permanent entering, under the control of the player who controls it.
fn enters(game: &Game, object: ObjId) -> Fact {
    Fact::new(Verb::Enter, None, vec![(Role::What, Target::Object(object)), (Role::Under, Target::Player(game.controller(object)))])
}

fn deals_damage(source: ObjId, to: PlayerId, combat: bool) -> Fact {
    Fact { combat: Some(combat), amount: Some(2), ..Fact::new(Verb::DealDamage, Some(Target::Object(source)), vec![(Role::To, Target::Player(to))]) }
}

#[test]
fn r603_1_a_definition_keeps_each_trigger_lowered_or_its_refusal() {
    // a triggered ability is kept under the index of its ability; what is not one is not kept
    let warden = creature("Warden", "Flying\nWhenever another creature enters, you gain 1 life.");
    assert_eq!(warden.triggers.iter().map(|r| r.ability).collect::<Vec<_>>(), vec![1]);
    assert_eq!((warden.trigger_ok(0), warden.trigger_ok(1), warden.trigger_ok(2)), (None, Some(Ok(())), None));
    assert!(creature("Bear", "").triggers.is_empty());

    // the same definition lowers to the same template: nothing of a game is in it
    let again = creature("Warden", "Flying\nWhenever another creature enters, you gain 1 life.");
    assert_eq!(template(&warden), template(&again));

    // what the engine does not watch for is refused once, as the definition is loaded, and
    // the definition keeps the refusal
    let alone = creature("Unblocked", "Whenever this creature attacks and isn't blocked, it gets +2/+0 until end of turn.");
    let refusal = alone.trigger_ok(0).expect("a triggered ability").unwrap_err();
    assert!(refusal.0.starts_with("trigger: "), "{}", refusal.0);
    assert_eq!(alone.triggers[0].result.as_ref().unwrap_err(), &refusal);
    assert_eq!(watched(&alone).count(), 0);
}

#[test]
fn r603_2_a_kept_trigger_is_asked_of_the_permanent_whose_ability_it_is() {
    let warden = creature("Warden", "Whenever another creature enters, you gain 1 life.");
    let yours = creature("Yours", "Whenever a creature you control enters, you gain 1 life.");
    let mut game = Game::new(vec![warden, yours, creature("Bear", ""), card("Land", "Land", "", None)], 2, 20);
    let warden = game.put(0, 0, Zone::Battlefield);
    let yours = game.put(1, 1, Zone::Battlefield);
    let bear = game.put(1, 2, Zone::Battlefield);
    let land = game.put(0, 3, Zone::Battlefield);
    let ask = |game: &Game, source: ObjId, f: &Fact| {
        let controller = game.controller(source);
        template(game.def(source).unwrap()).watch.matches(game, &Ctx::new(controller, source).with_ability(0), f)
    };

    // "another creature": a creature, whoever controls it, and not the permanent itself
    let matched = ask(&game, warden, &enters(&game, bear)).expect("a creature entered");
    assert_eq!(matched.subject, Some(Target::Object(bear)));
    assert!(ask(&game, warden, &enters(&game, warden)).is_none());
    assert!(ask(&game, warden, &enters(&game, land)).is_none());
    // "you control": of the one template, whoever controls the permanent as it is asked
    assert!(ask(&game, yours, &enters(&game, bear)).is_some());
    assert!(ask(&game, yours, &enters(&game, warden)).is_none());
    // another event is not it
    assert!(ask(&game, warden, &Fact::new(Verb::Die, None, vec![(Role::What, Target::Object(bear))])).is_none());
}

#[test]
fn r510_3a_the_kind_of_damage_and_whom_it_is_dealt_to_are_part_of_the_kept_trigger() {
    let thief = creature("Thief", "Whenever this creature deals combat damage to a player, draw a card.");
    let mut game = Game::new(vec![thief], 2, 20);
    let thief = game.put(0, 0, Zone::Battlefield);
    let other = game.put(0, 0, Zone::Battlefield);
    let ctx = Ctx::new(0, thief).with_ability(0);
    let watch = &template(game.def(thief).unwrap()).watch;

    let matched = watch.matches(&game, &ctx, &deals_damage(thief, 1, true)).expect("combat damage to a player");
    assert_eq!((matched.subject, matched.participants.first()), (Some(Target::Object(thief)), Some(&Some(Target::Player(1)))));
    assert!(watch.matches(&game, &ctx, &deals_damage(thief, 1, false)).is_none());
    assert!(watch.matches(&game, &ctx, &deals_damage(other, 1, true)).is_none());
}

#[test]
fn r500_6_a_step_trigger_begins_in_a_turn_of_the_player_it_says() {
    let yours = creature("Yours", "At the beginning of your upkeep, you gain 1 life.");
    let each = creature("Each", "At the beginning of each upkeep, you gain 1 life.");
    let mut game = Game::new(vec![yours, each], 2, 20);
    let yours = game.put(0, 0, Zone::Battlefield);
    let each = game.put(0, 1, Zone::Battlefield);
    let begins = |game: &Game, source: ObjId, part: Part| {
        template(game.def(source).unwrap()).watch.begins(game, &Ctx::new(0, source).with_ability(0), part)
    };
    let upkeep = *Part::ORDER.iter().find(|p| p.named(Step::Upkeep)).expect("an upkeep step");
    let other = *Part::ORDER.iter().find(|p| !p.named(Step::Upkeep)).expect("another part");

    game.active = 0;
    assert!(begins(&game, yours, upkeep) && begins(&game, each, upkeep));
    assert!(!begins(&game, yours, other) && !begins(&game, each, other));
    game.active = 1;
    assert!(!begins(&game, yours, upkeep) && begins(&game, each, upkeep));
    // a step is not an event of the log
    assert!(template(game.def(yours).unwrap()).watch.matches(&game, &Ctx::new(0, yours), &enters(&game, each)).is_none());
}

#[test]
fn r603_4_an_intervening_if_is_kept_with_the_trigger_and_asked_of_the_game_as_it_is() {
    let patient = creature("Patient", "At the beginning of your upkeep, if you control three or more creatures, you gain 1 life.");
    let mut game = Game::new(vec![patient, creature("Bear", "")], 2, 20);
    let patient = game.put(0, 0, Zone::Battlefield);
    let ctx = Ctx::new(0, patient).with_ability(0);
    game.put(0, 1, Zone::Battlefield);
    assert!(template(game.def(patient).unwrap()).intervening.is_some());
    assert!(!template(game.def(patient).unwrap()).holds(&game, &ctx));
    game.put(0, 1, Zone::Battlefield);
    assert!(template(game.def(patient).unwrap()).holds(&game, &ctx));
    // one without an "if" always does
    let plain = creature("Plain", "At the beginning of your upkeep, you gain 1 life.");
    assert!(template(&plain).intervening.is_none() && template(&plain).holds(&game, &ctx));
}

#[test]
fn r603_7c_a_delayed_occurrence_is_the_same_actions_of_the_objects_it_is_given() {
    // the pattern an effect would make a delayed trigger of, read here from a printed one
    let mortal = creature("Mortal", "When this creature dies, draw a card.");
    let Ability::Triggered { trigger: Trigger::Event { pattern, .. }, .. } = &mortal.abilities[0] else { panic!("an event trigger") };
    let support = Support::definition(&mortal, None);
    let occurrence = Occurrence::compile(pattern, &support).unwrap();
    let Watch::Event(kept) = &template(&mortal).watch else { panic!("an event") };
    assert_eq!(occurrence.0, kept.actions);

    let mut game = Game::new(vec![mortal.clone()], 2, 20);
    let (a, b) = (game.put(0, 0, Zone::Battlefield), game.put(0, 0, Zone::Battlefield));
    let ctx = Ctx::new(0, a);
    let dies = |o: ObjId| Fact::new(Verb::Die, None, vec![(Role::What, Target::Object(o))]);
    assert!(occurrence.happened_to(&game, &ctx, &dies(a), &[Target::Object(a)]));
    assert!(!occurrence.happened_to(&game, &ctx, &dies(b), &[Target::Object(a)]));
    assert!(!occurrence.happened_to(&game, &ctx, &enters(&game, a), &[Target::Object(a)]));

    // what the engine does not watch for is refused as the occurrence is lowered
    let mut ordinal = pattern.clone();
    ordinal.nth_each_turn = Some(1);
    assert!(Occurrence::compile(&ordinal, &support).unwrap_err().0.contains("an ordinal, of a delayed ability"));
}

#[test]
fn r400_7e_public_successors_survive_event_history_and_do_not_follow_later_moves() {
    let mut game = Game::new(vec![creature("Bear", "")], 2, 20);
    for zone in [Zone::Battlefield, Zone::Graveyard, Zone::Exile, Zone::Stack, Zone::Hand, Zone::Library] {
        let was = game.put(0, 0, Zone::Hand);
        assert_eq!(game.became(was), None);
        let now = game.move_to(was, zone, 0).unwrap();
        let expected = matches!(zone, Zone::Battlefield | Zone::Graveyard | Zone::Exile | Zone::Stack).then_some(now);
        assert_eq!(game.became(was), expected);
        game.check_triggers();
        game.log.clear();
        game.seen = game.log.end();
        let mut fork = game.clone();
        assert_eq!(fork.became(was), expected);
        let later = fork.move_to(now, Zone::Exile, 0).unwrap();
        assert_eq!(fork.became(now), Some(later));
        assert_eq!(fork.became(was), expected, "an exception finds one successor, not a chain");
        assert_eq!(game.obj(now).zone, zone, "clones keep independent incarnations");
    }
}

#[test]
fn r400_7_graveyard_arrival_separates_earlier_and_later_step_triggers() {
    let returns = creature("Returns", "At the beginning of your upkeep, you may return this card from your graveyard to your hand.");
    let mut game = Game::new(vec![returns], 2, 20);
    let upkeep = *Part::ORDER.iter().find(|p| p.named(Step::Upkeep)).unwrap();
    game.log.push(Event::Began { part: upkeep });
    let was = game.put(0, 0, Zone::Hand);
    let arrived = game.move_to(was, Zone::Graveyard, 0).unwrap();
    game.log.push(Event::Began { part: upkeep });
    // Setup cards have no arrival event even if the fixture inserts them after events.
    let setup = game.put(0, 0, Zone::Graveyard);
    assert_eq!(game.obj(setup).arrived, None);
    for mut branch in [game.clone(), game] {
        branch.check_triggers();
        assert_eq!(branch.triggered.iter().filter(|t| t.source == Some(arrived)).count(), 1);
        assert_eq!(branch.triggered.iter().filter(|t| t.source == Some(setup)).count(), 2);
    }
}

#[test]
fn r400_7_graveyard_arrival_does_not_require_the_zone_change_payload() {
    let returns = creature("Returns", "At the beginning of your upkeep, you may return this card from your graveyard to your hand.");
    let mut game = Game::new(vec![returns], 2, 20);
    let upkeep = *Part::ORDER.iter().find(|p| p.named(Step::Upkeep)).unwrap();
    game.log.push(Event::Began { part: upkeep });
    let was = game.put(0, 0, Zone::Hand);
    let arrived = game.move_to(was, Zone::Graveyard, 0).unwrap();
    let arrival = game.obj(arrived).arrived.unwrap();
    // Keep sequence positions while removing the historical payload: the pending
    // earlier upkeep must not become visible to this incarnation as a result.
    let index = arrival.0 - game.log.start().0;
    game.log[index] = Event::LifeGained { player: 1, amount: 1 };
    game.log.push(Event::Began { part: upkeep });
    game.check_triggers();
    assert_eq!(game.triggered.iter().map(|t| t.source).collect::<Vec<_>>(), vec![Some(arrived)]);
}

#[test]
fn r400_7_graveyard_triggers_do_not_watch_their_own_arrival_death() {
    let returns = creature("Returns", "Whenever a creature dies, return this card from your graveyard to your hand.");
    assert_eq!(returns.scope_ok(0), Some(Ok(())));
    let mut game = Game::new(vec![returns, creature("Bear", "")], 2, 20);
    let was = game.put(0, 0, Zone::Battlefield);
    let bear = game.put(0, 1, Zone::Battlefield);
    let arrived = game.move_to(was, Zone::Graveyard, 0).unwrap();
    game.move_to(bear, Zone::Graveyard, 0);
    game.check_triggers();
    assert_eq!(game.triggered.iter().map(|t| t.source).collect::<Vec<_>>(), vec![Some(arrived)]);
}

#[test]
fn r603_7b_a_delayed_ability_without_a_duration_triggers_for_the_first_of_the_events_checked_together() {
    let mortal = creature("Mortal", "When this creature dies, draw a card.");
    let Ability::Triggered { trigger: Trigger::Event { pattern, .. }, .. } = &mortal.abilities[0] else { panic!("an event trigger") };
    let occurrence = Occurrence::compile(pattern, &Support::definition(&mortal, None)).unwrap();
    let mut game = Game::new(vec![mortal.clone(), creature("Bear", "")], 2, 20);
    let source = game.put(0, 0, Zone::Battlefield);
    let (a, b) = (game.put(0, 1, Zone::Battlefield), game.put(1, 1, Zone::Battlefield));
    let captured = crate::program::capture(&game, Ctx::new(0, source).with_ability(0)).unwrap();
    let delayed = |subjects: &[ObjId], lasts| crate::state::Delayed { source, controller: 0, pattern: DelayedPattern::event(occurrence.clone()),
        subjects: subjects.iter().map(|o| Target::Object(*o)).collect(), captured: captured.clone(), bound: Bound::default(), once: lasts == crate::state::Lasts::Indefinite, lasts };
    use crate::state::Lasts;
    game.delayed = vec![delayed(&[a, b], Lasts::Indefinite), delayed(&[a, b], Lasts::ThisTurn), delayed(&[b], Lasts::Indefinite), delayed(&[source], Lasts::Indefinite)];
    // both die before anything is checked: two events of one check
    game.move_to(a, Zone::Graveyard, 0);
    game.move_to(b, Zone::Graveyard, 1);
    for mut branch in [game.clone(), game] {
        branch.check_triggers();
        let found: Vec<_> = branch.triggered.iter().filter(|t| t.delayed.is_some()).collect();
        // the first for a's death only, the second for both, the third for b's
        assert_eq!(found.len(), 4, "{found:?}");
        let moved: Vec<_> = found.iter().map(|t| t.bound.became.first().map(|(was, _)| *was)).collect();
        assert_eq!(moved, vec![Some(a), Some(a), Some(b), Some(b)], "in the order of the events, each as they were made");
        // what triggered once is gone, by its own place; what did not is kept
        assert_eq!(branch.delayed.iter().map(|d| (d.subjects.clone(), d.lasts)).collect::<Vec<_>>(),
            vec![(vec![Target::Object(a), Target::Object(b)], Lasts::ThisTurn), (vec![Target::Object(source)], Lasts::Indefinite)]);
        // and nothing found is found again
        branch.check_triggers();
        assert_eq!(branch.triggered.iter().filter(|t| t.delayed.is_some()).count(), 4);
    }
}
