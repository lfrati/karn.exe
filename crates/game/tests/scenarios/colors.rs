//! The five colors, color sets, color indicators and layer-5 changes (105, 204.2).
use crate::support::cite;
use game::{CardDef, Game, Move, Target, Zone};
use ir::Color;

fn face(cost: Option<&str>, indicator: Option<Vec<&str>>) -> oracle::corpus::Face {
    serde_json::from_value(serde_json::json!({
        "oracle_id": "fixture", "name": "Prism", "face": 0, "face_name": "Prism", "layout": "normal",
        "mana_cost": cost, "type_line": "Creature — Spirit", "oracle_text": "", "power": "2", "toughness": "2",
        "loyalty": null, "defense": null, "colors": ["U"], "color_indicator": indicator
    })).unwrap()
}

#[test]
fn r204_2_a_color_indicator_defines_colors_in_every_zone_and_overrides_mana_colors() {
    for cost in [None, Some("{U}")] {
        let d = CardDef::from_face(&face(cost, Some(vec!["R", "G"]))).unwrap();
        game::play::check(&d).unwrap();
        cite!(["204.2"]; case: format!("{cost:?}"); assert_eq!(d.colors(), vec![Color::Red, Color::Green]));
        let mut g = Game::new(vec![d], 2, 20);
        for zone in [Zone::Library, Zone::Hand, Zone::Battlefield, Zone::Graveyard, Zone::Exile, Zone::Stack] {
            let o = g.put(0, 0, zone);
            cite!(["204.2"]; case: format!("{cost:?} {zone:?}"); assert_eq!(g.seen(o, None).colors, vec![Color::Red, Color::Green], "{zone:?}"));
        }
    }
}

#[test]
fn r105_1_indicators_use_the_five_colors_and_refuse_invalid_metadata() {
    for (letter, color) in [("W", Color::White), ("U", Color::Blue), ("B", Color::Black), ("R", Color::Red), ("G", Color::Green)] {
        let d = CardDef::from_face(&face(None, Some(vec![letter, letter]))).unwrap();
        cite!(["105.1", "204.2"]; case: letter.to_string(); assert_eq!(d.colors(), vec![color], "repetition does not invent a second color"));
    }
    for invalid in [vec![], vec!["C"], vec!["S"], vec!["R/G"], vec!["white"], vec!["R", "unknown"]] {
        let reason=if invalid.is_empty(){"a color indicator denotes one or more colors"}else{"invalid color indicator"};
        crate::support::refused(CardDef::from_face(&face(None,Some(invalid.clone()))),reason);
    }
    let absent = CardDef::from_face(&face(None, None)).unwrap();
    cite!(["105.2"]; assert!(absent.colors().is_empty(), "computed corpus colors cannot replace unimplemented rules"));
}

#[test]
fn r105_2a_b_c_color_classification_uses_distinct_colors_for_every_subset() {
    let letters = ["W", "U", "B", "R", "G"];
    let mut defs = Vec::new();
    for bits in 0u32..32 {
        let cost: String = letters.iter().enumerate().filter(|(i, _)| bits & (1 << i) != 0)
            .map(|(_, l)| format!("{{{l}}}{{{l}}}")).collect();
        defs.push(CardDef::new("Prism", if cost.is_empty() { "{0}" } else { &cost }, "Creature — Spirit", "", Some((2, 2))).unwrap());
    }
    for quality in ["monocolored", "multicolored", "colorless"] {
        defs.push(CardDef::new("Test", "{0}", "Instant", &format!("Destroy target {quality} creature."), None).unwrap());
    }
    for d in &defs { game::play::check(d).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    let creatures: Vec<_> = (0..32).map(|i| g.put(0, i, Zone::Battlefield)).collect();
    let spells: Vec<_> = (32..35).map(|i| g.put(0, i, Zone::Hand)).collect();
    for (which, spell) in spells.into_iter().enumerate() {
        for (bits, creature) in creatures.iter().enumerate() {
            let allowed = match which { 0 => bits.count_ones() == 1, 1 => bits.count_ones() >= 2, _ => bits == 0 };
            let mut attempt = g.clone();
            let result = attempt.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(*creature)]] });
            cite!(["105.2a", "105.2b", "105.2c"]; case: format!("spell {which}, colors {bits}"); assert_eq!(result.is_ok(), allowed));
        }
    }
}

#[test]
fn r105_3_new_colors_replace_or_add_and_colorless_removes_all_colors() {
    let texts = ["Target creature becomes blue until end of turn.",
        "Target creature becomes red in addition to its other colors until end of turn.",
        "Target creature becomes colorless until end of turn."];
    let mut defs = vec![CardDef::new("Prism", "{G}{W}", "Creature — Spirit", "", Some((2, 2))).unwrap()];
    for text in texts { defs.push(CardDef::new("Change", "{0}", "Instant", text, None).unwrap()); }
    for d in &defs { game::play::check(d).unwrap_or_else(|u| panic!("{}", u.0)); }
    let mut g = Game::new(defs, 2, 20);
    let creature = g.put(0, 0, Zone::Battlefield);
    for (def, expected) in [(1, vec![Color::Blue]), (2, vec![Color::Blue, Color::Red]), (2, vec![Color::Blue, Color::Red]), (3, vec![]), (2, vec![Color::Red])] {
        let card = g.put(0, def, Zone::Hand);
        g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(creature)]] }).unwrap();
        g.act(0, Move::Pass).unwrap(); g.act(1, Move::Pass).unwrap();
        cite!(["105.3"]; case: format!("{def} {expected:?}"); assert_eq!(g.seen(creature, None).colors, expected));
    }
    g.move_object(creature, Zone::Hand);
    let returned = *g.players[0].hand.last().unwrap();
    cite!(["400.7"]; assert_eq!(g.seen(returned, None).colors, vec![Color::Green, Color::White], "zone changes discard the old effects"));
}

#[test]
fn r105_2_color_changes_are_said_of_objects_not_players() {
    for text in ["Target player becomes blue.", "You become colorless.", "Target creature becomes multicolored."] {
        assert_eq!(CardDef::new("Bad color","{0}","Instant",text,None).unwrap_err(),"text the grammar does not read");
    }
}

#[test]
fn r105_3_color_changes_apply_to_spells_and_graveyard_cards_and_preserve_last_known_colors() {
    let defs = vec![
        CardDef::new("Prism", "{U}", "Instant", "You draw zero cards.", None).unwrap(),
        CardDef::new("Tint", "{0}", "Instant", "Target spell becomes red.", None).unwrap(),
        CardDef::new("Stain", "{0}", "Instant", "Target instant card in your graveyard becomes green.", None).unwrap(),
        CardDef::new("Denial", "{0}", "Instant", "Counter target blue spell.", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    let spell = g.put(0, 0, Zone::Stack);
    let tint = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card: tint, targets: vec![vec![Target::Object(spell)]] }).unwrap();
    g.act(0, Move::Pass).unwrap(); g.act(1, Move::Pass).unwrap();
    cite!(["105.3"]; assert_eq!(g.seen(spell, None).colors, vec![Color::Red]));
    let denial = g.put(0, 3, Zone::Hand);
    let before = format!("{g:?}");
    cite!(["105.2", "115.1"]; refused("an illegal target, Prism: it isn't blue (115.1)"): g.act(0, Move::Cast { card: denial, targets: vec![vec![Target::Object(spell)]] }));
    assert_eq!(format!("{g:?}"), before, "color-based targeting queries the changed color");
    g.counter(spell);
    let card = *g.players[0].graveyard.last().unwrap();
    // Uncited (glossary): "Last Known Information" captures the departed object's last existence in its zone; no effect asks, so 608.2h/113.7a don't apply
    assert_eq!(g.seen(spell, None).colors, vec![Color::Red], "the old spell retains its last known color");
    cite!(["400.7"]; assert_eq!(g.seen(card, None).colors, vec![Color::Blue], "the new card regains its printed color"));
    let stain = g.put(0, 2, Zone::Hand);
    g.act(0, Move::Cast { card: stain, targets: vec![vec![Target::Object(card)]] }).unwrap();
    g.act(0, Move::Pass).unwrap(); g.act(1, Move::Pass).unwrap();
    cite!(["105.3"]; assert_eq!(g.seen(card, None).colors, vec![Color::Green]));
    g.move_object(card, Zone::Hand);
    // Uncited (glossary): as above, "Last Known Information" of the departed graveyard card
    assert_eq!(g.seen(card, None).colors, vec![Color::Green]);
    cite!(["400.7"]; assert_eq!(g.seen(*g.players[0].hand.last().unwrap(), None).colors, vec![Color::Blue]));
}

#[test]
fn r105_3_color_changes_end_with_their_duration() {
    let defs = vec![CardDef::new("Prism", "{G}", "Creature — Spirit", "", Some((2, 2))).unwrap(),
        CardDef::new("Tint", "{0}", "Instant", "Target creature becomes blue until end of turn.", None).unwrap()];
    let mut g = Game::new(defs, 2, 20);
    let creature = g.put(0, 0, Zone::Battlefield);
    let tint = g.put(0, 1, Zone::Hand);
    for p in 0..2 { g.put(p, 0, Zone::Library); }
    g.act(0, Move::Cast { card: tint, targets: vec![vec![Target::Object(creature)]] }).unwrap();
    g.act(0, Move::Pass).unwrap(); g.act(1, Move::Pass).unwrap();
    cite!(["105.3"]; assert_eq!(g.seen(creature, None).colors, vec![Color::Blue]));
    for _ in 0..40 {
        if g.turn > 1 { break; }
        let mv = if matches!(g.decision, Some(game::state::Decision::Attackers { .. })) { Move::Attack(vec![]) } else { Move::Pass };
        g.act(g.priority, mv).unwrap();
    }
    assert_eq!(g.turn, 2);
    cite!(["514.2"]; assert_eq!(g.seen(creature, None).colors, vec![Color::Green]));
}
