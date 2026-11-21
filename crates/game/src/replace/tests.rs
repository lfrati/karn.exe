use super::*;
use crate::state::Zone;

fn card(name: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    CardDef::new(name, "", types, text, pt).expect(name)
}

fn creature(name: &str, text: &str) -> CardDef {
    card(name, "Creature — Bear", text, Some((2, 2)))
}

fn land(name: &str, text: &str) -> CardDef {
    card(name, "Land", text, None)
}

/// What the definition keeps of its one replacement effect.
fn replacement(def: &CardDef) -> &Replacement {
    let [registered] = def.replacements.as_slice() else { panic!("{}: one replacement effect", def.name) };
    registered.result.as_ref().expect("lowered")
}

/// A permanent seen as it would exist on the battlefield while it is not yet a source there
/// (614.12): the view an entry event is asked in.
fn entering(game: &mut Game, player: PlayerId, def: usize) -> ObjId {
    let object = game.put(player, def, Zone::Battlefield);
    game.battlefield.retain(|o| *o != object);
    object
}

const DOUBLE: &str = "If you would gain life, you gain twice that much life instead.";
const SCALES: &str = "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.";
const SHOCK: &str = "As this land enters, you may pay 2 life. If you don't, it enters tapped.";

#[test]
fn r614_1_a_definition_keeps_each_replacement_lowered_or_its_refusal() {
    // a replacement effect is kept under the index of its ability; what is not one is not kept
    let doubler = creature("Doubler", &format!("Lifelink\n{DOUBLE}"));
    assert_eq!(doubler.replacements.iter().map(|r| r.ability).collect::<Vec<_>>(), vec![1]);
    assert_eq!((doubler.replacement_ok(0), doubler.replacement_ok(1), doubler.replacement_ok(2)), (None, Some(Ok(())), None));
    assert!(creature("Bear", "").replacements.is_empty());
    assert!(creature("Warden", "Whenever another creature enters, you gain 1 life.").replacements.is_empty());

    // each kind lowers to its own form, with what it says counted or described already
    assert!(matches!(replacement(&doubler), Replacement::LifeGain { who: Noun::You, factor: 2 }));
    assert!(matches!(
        replacement(&creature("Scales", SCALES)),
        Replacement::Counters { on: Noun::Described(_), kinds: super::CounterScope::Named(kind), change: CounterChange::Plus(1), .. } if kind == "+1/+1"
    ));
    assert!(matches!(
        replacement(&land("Tapland", "This land enters tapped.")),
        Replacement::Arrives { subject: Said::Source, acts } if matches!(acts.as_slice(), [(When::Always, enters)] if enters == &[Enters::Tapped])
    ));
    assert!(matches!(
        replacement(&land("Checkland", "This land enters tapped unless you control a basic land.")),
        Replacement::Arrives { subject: Said::Source, acts } if matches!(acts.as_slice(), [(When::Unless(_), _)])
    ));
    assert!(matches!(
        replacement(&creature("Grown", "This creature enters with two +1/+1 counters on it.")),
        Replacement::Arrives { subject: Said::Source, acts }
            if matches!(acts.as_slice(), [(When::Always, enters)] if matches!(enters.as_slice(), [Enters::Counters { kind, .. }] if kind == "+1/+1"))
    ));
    assert!(matches!(replacement(&creature("Frost", "Creatures your opponents control enter tapped.")), Replacement::Arrives { subject: Said::Described(_), .. }));
    assert!(matches!(
        replacement(&land("Shock", SHOCK)),
        Replacement::AsEnters(Asked::Life { otherwise, .. }) if otherwise == &[Enters::Tapped]
    ));
    assert!(matches!(replacement(&card("Lens", "Artifact", "As this artifact enters, choose a color.", None)), Replacement::AsEnters(Asked::Value(_))));

    // the same definition lowers to the same replacement: nothing of a game is in it
    assert_eq!(replacement(&creature("Scales", SCALES)), replacement(&creature("Scales", SCALES)));
}

#[test]
fn r614_1_what_the_engine_does_not_apply_is_refused_as_the_definition_is_loaded() {
    // a counter replacement outside the currently admitted family
    let wither = creature("Wither", "If one or more -1/-1 counters would be put on a creature you control, that many plus one -1/-1 counters are put on it instead.");
    let refusal = wither.replacement_ok(0).expect("a replacement effect").unwrap_err();
    assert!(refusal.0.starts_with("counter replacement: "), "{}", refusal.0);
    assert_eq!(wither.replacements[0].result.as_ref().unwrap_err(), &refusal);

    // a refused replacement is never applied
    let mut game = Game::new(vec![wither, creature("Bear", "")], 2, 20);
    let wither = game.put(0, 0, Zone::Battlefield);
    let bear = game.put(0, 1, Zone::Battlefield);
    assert_eq!(game.replacements(wither).count(), 0);
    assert!(game.counter_replacements(bear, 0, &[("-1/-1".into(),1)], true, &[]).is_empty());
}

#[test]
fn r119_10_a_kept_multiplier_is_asked_of_the_permanent_whose_ability_it_is() {
    let mut game = Game::new(vec![creature("Doubler", DOUBLE), creature("Twice", &format!("{DOUBLE}\n{DOUBLE}"))], 2, 20);
    assert_eq!(game.replace_life_gain(0, 3), Ok(3));
    game.put(0, 0, Zone::Battlefield);
    // "you" is the player who controls the permanent, not the player gaining
    assert_eq!((game.replace_life_gain(0, 3), game.replace_life_gain(1, 3)), (Ok(6), Ok(3)));
    // each ability of each permanent applies once (614.5)
    game.put(0, 1, Zone::Battlefield);
    assert_eq!((game.replace_life_gain(0, 3), game.replace_life_gain(1, 3)), (Ok(24), Ok(3)));
    game.put(1, 0, Zone::Battlefield);
    assert_eq!(game.replace_life_gain(1, 3), Ok(6));
    // a modified gain this engine cannot represent is refused, never wrapped (107.1b)
    assert_eq!(game.replace_life_gain(0, u32::MAX / 8 + 1).unwrap_err().0,
        "life gain amount exceeds the supported numerical range (107.1b)");
    assert_eq!(game.replace_life_gain(0, u32::MAX / 8), Ok(u32::MAX / 8 * 8));
}

#[test]
fn r614_1a_a_kept_counter_replacement_is_found_for_the_permanents_it_describes() {
    let mut game = Game::new(vec![creature("Scales", SCALES), creature("Bear", ""), land("Land", "")], 2, 20);
    let scales = game.put(0, 0, Zone::Battlefield);
    let mine = game.put(0, 1, Zone::Battlefield);
    let theirs = game.put(1, 1, Zone::Battlefield);
    let land = game.put(0, 2, Zone::Battlefield);
    let id = ReplacementId::from_ability(scales, crate::binding::AbilityKey::printed(0), 0);

    assert_eq!(game.counter_replacements(mine, 0, &[("+1/+1".into(),1)], true, &[]), vec![(id.clone(), (super::CounterScope::Named("+1/+1".into()),CounterChange::Plus(1)))]);
    // itself is a creature its controller controls
    assert_eq!(game.counter_replacements(scales, 0, &[("+1/+1".into(),2)], true, &[]), vec![(id.clone(), (super::CounterScope::Named("+1/+1".into()),CounterChange::Plus(1)))]);
    // not what it does not describe, another kind, no counters, or once it has applied (614.5)
    assert!(game.counter_replacements(theirs, 0, &[("+1/+1".into(),1)], true, &[]).is_empty());
    assert!(game.counter_replacements(land, 0, &[("+1/+1".into(),1)], true, &[]).is_empty());
    assert!(game.counter_replacements(mine, 0, &[("charge".into(),1)], true, &[]).is_empty());
    assert!(game.counter_replacements(mine, 0, &[("+1/+1".into(),0)], true, &[]).is_empty());
    assert!(game.counter_replacements(mine, 0, &[("+1/+1".into(),1)], true, &[id]).is_empty());
}

#[test]
fn r614_12_kept_arrivals_apply_to_the_entering_permanent_alone_or_as_described() {
    let defs = vec![
        creature("Frost", "Creatures your opponents control enter tapped."),
        creature("Grown", "This creature enters with two +1/+1 counters on it."),
        land("Tapland", "This land enters tapped."),
        land("Checkland", "This land enters tapped unless you control a basic land."),
        card("Plains", "Basic Land — Plains", "", None),
    ];
    let mut game = Game::new(defs, 2, 20);
    let frost = game.put(0, 0, Zone::Battlefield);
    let frozen = ReplacementId::from_ability(frost, crate::binding::AbilityKey::printed(0), 0);

    // its own text is about it alone; another's text is about what that describes
    let grown = entering(&mut game, 1, 1);
    let own = ReplacementId::from_ability(grown, crate::binding::AbilityKey::printed(0), 0);
    assert_eq!(
        game.arrival_replacements(grown, &[]).unwrap(),
        vec![(own.clone(), false, false, vec![("+1/+1".to_string(), 2)]), (frozen.clone(), true, false, Vec::new())]
    );
    // each applies once to the event (614.5)
    assert_eq!(game.arrival_replacements(grown, &[own.clone()]).unwrap(), vec![(frozen.clone(), true, false, Vec::new())]);
    assert!(game.arrival_replacements(grown, &[own, frozen]).unwrap().is_empty());
    // "your opponents" are the opponents of the player who controls the source
    let mine = entering(&mut game, 0, 1);
    assert_eq!(game.arrival_replacements(mine, &[]).unwrap().len(), 1);
    // a land is not a creature
    let tapland = entering(&mut game, 1, 2);
    assert_eq!(game.arrival_replacements(tapland, &[]).unwrap(), vec![(ReplacementId::from_ability(tapland, crate::binding::AbilityKey::printed(0), 0), true, false, Vec::new())]);

    // a condition is asked as the permanent enters, not as its definition is loaded
    let check = entering(&mut game, 1, 3);
    assert_eq!(game.arrival_replacements(check, &[]).unwrap().len(), 1);
    game.put(1, 4, Zone::Battlefield);
    assert!(game.arrival_replacements(check, &[]).unwrap().is_empty());
}

#[test]
fn r614_12a_a_kept_entry_choice_is_asked_with_its_number_counted() {
    let lens = card("Lens", "Artifact", "Flash\nAs this artifact enters, choose a color.", None);
    let mut game = Game::new(vec![land("Shock", SHOCK), lens, land("Land", "")], 2, 20);
    let shock = game.put(0, 0, Zone::Hand);
    let lens = game.put(0, 1, Zone::Hand);
    let plain = game.put(0, 2, Zone::Hand);

    let asking = game.enters_asking(shock, 0, &crate::replace::Choices::default()).expect("asks");
    assert_eq!(asking.asks, Asks::Life { life: 2, otherwise: vec![Enters::Tapped] });
    // the value chosen is linked to the ability that asks for it (607.2d)
    let asking = game.enters_asking(lens, 0, &crate::replace::Choices::default()).expect("asks");
    assert!(matches!(asking.asks, Asks::Value { link, .. } if link == LinkId(AbilityKey::printed(1))));
    assert!(game.enters_asking(plain, 0, &crate::replace::Choices::default()).is_none());
}

#[test]
fn r614_1a_draw_replacements_compile_once_and_refuse_unimplemented_qualifiers() {
    let reflection = card("Thought Reflection", "Enchantment", "If you would draw a card, draw two cards instead.", None);
    assert!(matches!(replacement(&reflection), Replacement::Draw { who: Noun::You, count: 2 }));
    crate::play::check(&reflection).unwrap();
    let explicit = card("Explicit", "Enchantment", "If you would draw a card, you draw three cards instead.", None);
    assert!(matches!(replacement(&explicit), Replacement::Draw { who: Noun::You, count: 3 }));
    crate::play::check(&explicit).unwrap();
    for text in [
        "If you would draw a card while you control a creature, draw two cards instead.",
        "If you would draw a card, you may draw two cards instead.",
        "If you would draw a card, draw two cards and gain 1 life instead.",
        "If you would draw a card, target opponent draws two cards instead.",
    ] {
        let def = CardDef::new("Unsupported", "", "Enchantment", text, None).unwrap_or_else(|e| panic!("{text}: {e:?}"));
        assert!(super::draw_replacement(&def.abilities[0]).is_none(), "the exact unsupported draw shape: {text}");
        assert_eq!(crate::play::check(&def).unwrap_err().0,
            "ability on a permanent: sentence: Replacement", "{text}");
    }
}

/// CR107.1b has no fixed-width counter limit; refuse an unrepresentable result.
#[test]
fn r107_1b_counter_replacement_does_not_silently_clamp_the_rule_result() {
    let actual=super::CounterChange::Times(2).apply(2_147_483_648);
    assert!(actual.unwrap_err().0.contains("107.1b"));
}
