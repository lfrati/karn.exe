use crate::support::cite;
use game::{CardDef, Game, ObjId, Zone};
use ir::Keyword;

fn setup(creature: &str, effects: &[&str]) -> (Game, ObjId) {
    let mut defs = vec![CardDef::new("Subject", "{0}", "Creature", creature, Some((2, 2))).unwrap()];
    defs.extend(effects.iter().enumerate().map(|(i, text)|
        CardDef::new(&format!("Effect {i}"), "{0}", "Enchantment", text, None).unwrap()));
    for def in &defs { game::play::check(def).unwrap(); }
    let mut game = Game::new(defs, 2, 20);
    let subject = game.put(0, 0, Zone::Battlefield);
    for i in 1..=effects.len() { game.put(0, i, Zone::Battlefield); }
    (game, subject)
}
fn has(game: &Game, on: ObjId, name: &str) -> bool {
    game.seen(on, None).has(Keyword::from_name(name).unwrap())
}

#[test]
fn r613_8b_keyword_dependency_overrides_both_timestamp_orders() {
    for texts in [
        ["Creatures with flying have vigilance.", "Creatures with vigilance have trample."],
        ["Creatures with vigilance have trample.", "Creatures with flying have vigilance."],
    ] {
        let (game, subject) = setup("Flying", &texts);
        assert!(has(&game, subject, "vigilance"));
        cite!(["613.8b"]; case: texts[0]; assert!(has(&game, subject, "trample")));
        assert_eq!(game.seen(subject, None), game.clone().seen(subject, None));
    }
}

#[test]
fn r613_8b_negative_keyword_dependency_does_not_keep_an_earlier_grant() {
    let (game, subject) = setup("", &[
        "Creatures without flying have vigilance.",
        "Creatures have flying.",
    ]);
    assert!(has(&game, subject, "flying"));
    cite!(["613.8b"]; assert!(!has(&game, subject, "vigilance")));
}

#[test]
fn r613_8b_dependency_cycle_uses_timestamp_instead_of_saturation() {
    for (texts, yes, no) in [
        (["Creatures without flying have vigilance.", "Creatures without vigilance have flying."], "vigilance", "flying"),
        (["Creatures without vigilance have flying.", "Creatures without flying have vigilance."], "flying", "vigilance"),
    ] {
        let (game, subject) = setup("", &texts);
        cite!(["613.8b"]; case: yes; assert!(has(&game, subject, yes)));
        cite!(["613.8b"]; case: yes; assert!(!has(&game, subject, no)));
    }
}

#[test]
fn r613_8a_global_condition_reads_other_objects_from_the_same_partial_world() {
    let (game, subject) = setup("Flying", &[
        "Creatures have trample as long as you control a creature with vigilance.",
        "Creatures with flying have vigilance.",
    ]);
    cite!(["613.8a"]; assert!(has(&game, subject, "trample")));
}

#[test]
fn r613_8a_keyword_counter_is_visible_to_dependent_statics() {
    let (mut game, subject) = setup("", &["Creatures with flying have vigilance."]);
    game.put_counters(subject, "flying", 1);
    cite!(["613.1f"]; assert!(has(&game, subject, "vigilance")));
}

#[test]
fn r613_9_color_setters_are_independent_even_when_their_results_overwrite() {
    use game::layers::{Change, Continuous};
    use ir::Color;
    for (first, last) in [(Color::Red, Color::Blue), (Color::Blue, Color::Red)] {
        let (mut game, subject) = setup("", &[]);
        for (stamp, color) in [(1, first), (2, last)] {
            game.continuous.push(Continuous { producer:None, change: Change::Colors { colors: vec![color], added: false },
                affected: vec![subject], stamp, part: 0, lifetime: game::layers::Lifetime::Forever });
        }
        cite!(["613.7"]; case: format!("{last:?} last"); assert_eq!(game.seen(subject, None).colors, [last]));
    }
}

#[test]
fn r613_8a_condition_observes_a_different_creature_modified_in_this_layer() {
    let defs = vec![
        CardDef::new("Subject", "{0}", "Creature", "", Some((2, 2))).unwrap(),
        CardDef::new("Other", "{0}", "Creature", "Vigilance", Some((2, 2))).unwrap(),
        CardDef::new("First", "{0}", "Enchantment", "Creatures have trample as long as you control a creature with flying.", None).unwrap(),
        CardDef::new("Second", "{0}", "Enchantment", "Creatures with vigilance have flying.", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap(); }
    let mut game = Game::new(defs, 2, 20);
    let subject = game.put(0, 0, Zone::Battlefield);
    game.put(0, 1, Zone::Battlefield);
    game.put(0, 2, Zone::Battlefield);
    game.put(0, 3, Zone::Battlefield);
    assert!(!has(&game, subject, "flying"));
    cite!(["613.8a"]; assert!(has(&game, subject, "trample")));
}

#[test]
fn r208_3_partial_view_does_not_give_a_noncreature_printed_power() {
    use game::layers::{Change, Continuous, Types};
    use ir::CardType;
    let (mut game, subject) = setup("", &[
        "This enchantment has flying as long as you control a permanent with power 2 or greater.",
    ]);
    let effect = game.battlefield[1];
    assert!(has(&game, effect, "flying"));
    game.continuous.push(Continuous { producer:None, change: Change::Types(Types { types: vec![CardType::Artifact],
        subtypes: vec![], every_creature_type: false, keeps: false }), affected: vec![subject],
        stamp: 10, part: 0, lifetime: game::layers::Lifetime::Forever });
    cite!(["208.3"]; assert_eq!(game.seen(subject, None).pt, None));
    cite!(["208.3"]; assert!(!has(&game, effect, "flying")));
}

#[test]
fn r613_6_an_animation_keeps_its_recipients_for_its_later_pt_component() {
    let defs = vec![
        CardDef::new("Animation", "{0}", "Enchantment", "Noncreature artifacts become 3/3 artifact creatures.", None).unwrap(),
        CardDef::new("Artifact", "{0}", "Artifact", "", None).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap(); }
    let mut game = Game::new(defs, 2, 20);
    game.put(0, 0, Zone::Battlefield);
    let artifact = game.put(0, 1, Zone::Battlefield);
    let seen = game.seen(artifact, None);
    assert!(seen.is(ir::CardType::Artifact) && seen.is(ir::CardType::Creature));
    cite!(["613.6"]; assert_eq!(seen.pt, Some((3, 3))));
}

#[test]
fn r613_8c_only_remaining_effects_are_reconsidered_after_a_counter_applies() {
    let (mut game, subject) = setup("", &[
        "Creatures have trample as long as you control a creature with flying.",
        "Creatures with vigilance have flying.",
    ]);
    let other = game.put(0, 0, Zone::Battlefield);
    game.put_counters(other, "vigilance", 1);
    // Initially only the flying effect depends on the counter. The oldest trample
    // effect applies empty; 613.8c does not rerun an already applied effect.
    cite!(["613.8c"]; assert!(has(&game, other, "flying")));
    cite!(["613.8c"]; assert!(!has(&game, subject, "trample")));
}
