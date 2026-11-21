//! Color identity is printed setup information, not an effective characteristic (903.4).
use crate::support::cite;
use game::CardDef;
use ir::Color::{self, *};
use std::sync::OnceLock;

fn oracle(name: &str) -> CardDef {
    static FACES: OnceLock<Vec<oracle::corpus::Face>> = OnceLock::new();
    let faces = FACES.get_or_init(|| oracle::corpus::load(&oracle::corpus::default_path()).unwrap());
    CardDef::from_faces(CardDef::catalog(faces).get(name).unwrap()).unwrap()
}
fn identity(name: &str, expected: &[Color]) { assert_eq!(oracle(name).color_identity(), expected, "{name}"); }

#[test]
fn r903_4_cost_rules_symbols_and_quoted_abilities_contribute_but_color_words_do_not() {
    let d = CardDef::new("Witness", "{W/U}{B/P}{2/G}", "Artifact", "{T}: Add {R}.", None).unwrap();
    cite!(["903.4"]; assert_eq!(d.color_identity(), vec![White, Blue, Black, Red, Green]));
    let d = CardDef::new("Grant", "{0}", "Sorcery", "Until end of turn, target creature gains \"{G}, {T}: Add {U}{U}.\"", None).unwrap();
    cite!(["903.4"]; assert_eq!(d.color_identity(), vec![Blue, Green]));
    let d = CardDef::new("Witness", "{0}", "Artifact", "{T}: Add one mana of any color.\nProtection from red", None).unwrap();
    cite!(["903.4"]; assert!(d.color_identity().is_empty()));
    identity("Arcane Signet", &[]);
    identity("Command Tower", &[]);
}

#[test]
fn r903_4c_reminder_symbols_do_not_count_but_nested_quoted_rules_symbols_do() {
    // The hybrid extort payment is reminder text, not a printed mana symbol in rules text.
    identity("Basilica Screecher", &[Black]);
    identity("Blind Obedience", &[White]);
    let d = CardDef::new("Witness", "{U}", "Artifact", "{T}: Add {C}. (Pay {R} or {G}.)", None).unwrap();
    cite!(["903.4c"]; assert_eq!(d.color_identity(), vec![Blue]));
}

#[test]
fn r903_4_cdas_add_colors_without_erasing_mana_symbol_colors() {
    cite!(["903.4"]; assert_eq!(oracle("Transguild Courier").color_identity(), [White, Blue, Black, Red, Green], "Transguild Courier"));
    identity("Ghostfire", &[Red]);
    identity("Kozilek's Return", &[Red]);
}

#[test]
fn r903_4d_and_e_all_faces_and_alternative_characteristics_contribute() {
    cite!(["903.4d"]; assert_eq!(oracle("Civilized Scholar").color_identity(), [Blue, Red], "Civilized Scholar"));
    identity("Wear // Tear", &[White, Red]);
    cite!(["903.4e"]; assert_eq!(oracle("Heartflame Duelist").color_identity(), [White, Red], "Heartflame Duelist"));
}

#[test]
fn r903_5d_basic_land_types_are_a_separate_deck_constraint() {
    // An Island's implied blue mana ability is not printed rules text or a color indicator.
    cite!(["903.4"]; assert_eq!(oracle("Island").color_identity(), [] as [Color; 0], "Island"));
    identity("Hallowed Fountain", &[]);
    identity("Azorius Guildgate", &[White, Blue]);
}

#[test]
fn r604_3a_self_color_definitions_work_in_all_zones_before_ordinary_color_effects() {
    use game::{Game, Zone};
    let courier = oracle("Transguild Courier");
    let blue = CardDef::new("Blue", "{0}", "Enchantment", "All creatures are blue.", None).unwrap();
    assert!(blue.color_identity().is_empty(), "other-object color changes are not a CDA");
    let conditional = CardDef::new("Conditional", "{0}", "Artifact", "As long as you control a creature, this artifact is blue.", None).unwrap();
    assert!(conditional.color_identity().is_empty(), "conditional color changes are not a CDA");
    let mut g = Game::new(vec![courier, blue, oracle("Ghostfire")], 2, 20);
    let effect = g.put(0, 1, Zone::Battlefield);
    for zone in [Zone::Library, Zone::Hand, Zone::Graveyard, Zone::Exile, Zone::Command, Zone::Stack] {
        let card = g.put(0, 0, zone);
        cite!(["113.6a"]; case: format!("{zone:?}"); assert_eq!(g.seen(card, None).colors.len(), 5, "{zone:?}"));
    }
    let card = g.put(0, 0, Zone::Battlefield);
    cite!(["613.3"]; assert_eq!(g.seen(card, None).colors, vec![Blue], "ordinary color effects follow CDAs even with an earlier timestamp"));
    g.move_object(effect, Zone::Graveyard);
    assert_eq!(g.seen(card, None).colors.len(), 5);
    let ghostfire = g.put(0, 2, Zone::Hand);
    cite!(["604.3", "113.6a"]; assert!(g.seen(ghostfire, None).colors.is_empty()));
}
