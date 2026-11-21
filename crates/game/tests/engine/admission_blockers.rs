//! Every refusal of a card, for reports (`game-gate --all-blockers`): admission goes on past
//! the first refusal, which stays the one `check` gives, wherever what follows can still be
//! analyzed; where it cannot, the refusal says what it left unanalyzed, and only then.

use crate::support::cite;
use game::card::CardDef;
use game::play::{Blocker, Stage, blockers, check};
use std::sync::LazyLock;

static FACES: LazyLock<Vec<oracle::corpus::Face>> = LazyLock::new(|| oracle::corpus::load(&oracle::corpus::default_path()).unwrap());

fn reasons(all: &[Blocker]) -> Vec<&str> { all.iter().map(|b| b.reason.as_str()).collect() }

fn face(type_line: &str, text: &str, pt: Option<(&str, &str)>) -> oracle::corpus::Face {
    serde_json::from_value(serde_json::json!({
        "oracle_id": "fixture", "name": "Gizmo Frobber", "face": 0, "face_name": "Gizmo Frobber", "layout": "normal",
        "mana_cost": "{2}", "type_line": type_line, "oracle_text": text,
        "power": pt.map(|p| p.0), "toughness": pt.map(|p| p.1),
        "loyalty": null, "defense": null, "colors": [], "color_indicator": null
    })).unwrap()
}

#[test]
fn every_refusal_of_a_card_is_listed_after_the_first() {
    // a battle has defense (310.4), and a creature without printed numbers no power and
    // toughness the engine can use: two independent refusals of one face, of characteristics,
    // which hide nothing
    let d = CardDef::new("Odd", "{2}", "Creature Battle — Siege", "Flying", None).expect("the grammar reads it");
    let all = blockers(&d);
    assert_eq!(reasons(&all), ["defense", "power and toughness that are not numbers"]);
    assert!(all.iter().all(|b| b.stage == Stage::Engine && b.unanalyzed.is_empty()), "{all:?}");
    assert_eq!(check(&d).unwrap_err().0, all[0].reason, "the first blocker is the refusal check gives");
}

#[test]
fn r113_2c_each_paragraph_is_its_own_ability_and_each_refused_one_is_a_blocker() {
    // Fear (702.36) and "tap or untap" are both unimplemented: each paragraph is a separate
    // ability (113.2c), lowered on its own, so the second is analyzed past the first's refusal.
    // Neither hides anything: the first paragraph is the one keyword, and the second's one
    // instruction is the one refused
    let d = CardDef::new("Odd", "{2}", "Creature — Bear", "Fear\n{T}: Tap or untap target permanent.", Some((2, 2))).expect("the grammar reads it");
    let all = blockers(&d);
    cite!(["113.2c"]; assert_eq!(reasons(&all), ["keyword: Fear", "program: action not lowered: TapOrUntap"]));
    assert_eq!(check(&d).unwrap_err().0, all[0].reason);
    assert!(all.iter().all(|b| b.unanalyzed.is_empty()), "{all:?}");
}

#[test]
fn r113_2c_each_keyword_strung_on_one_line_is_its_own_ability_and_blocker() {
    // keyword abilities strung together on one line are each an ability (113.2c): fear
    // (702.36) and shadow (702.28) are both refused, each on its own, and neither hides the other
    let d = CardDef::new("Odd", "{2}", "Creature — Bear", "Fear, shadow", Some((2, 2))).expect("the grammar reads it");
    let all = blockers(&d);
    cite!(["113.2c"]; assert_eq!(reasons(&all), ["keyword: Fear", "keyword: Shadow"]));
    assert_eq!(check(&d).unwrap_err().0, all[0].reason);
    assert!(all.iter().all(|b| b.unanalyzed.is_empty()), "{all:?}");
}

#[test]
fn r608_2c_a_refused_instruction_leaves_those_written_after_it_unanalyzed() {
    // instructions are followed in the order written (608.2c), and lowered so: past a refused
    // one, lowering of that ability stops, so "Draw a card." written after it is not analyzed
    let d = CardDef::new("Odd", "{2}", "Creature — Bear", "{T}: Tap or untap target permanent. Draw a card.", Some((2, 2))).expect("the grammar reads it");
    let all = blockers(&d);
    cite!(["608.2c"]; assert_eq!(reasons(&all), ["program: action not lowered: TapOrUntap"]));
    assert_eq!(all[0].unanalyzed, ["Odd, ability 1, 1 instruction its lowering did not reach: it stops at its first refusal"]);
    // written before it, the same instruction was lowered: the refused one is the last, and
    // hides nothing
    let d = CardDef::new("Odd", "{2}", "Creature — Bear", "{T}: Draw a card. Tap or untap target permanent.", Some((2, 2))).expect("the grammar reads it");
    let all = blockers(&d);
    assert_eq!(reasons(&all), ["program: action not lowered: TapOrUntap"]);
    assert!(all[0].unanalyzed.is_empty(), "{all:?}");
}

#[test]
fn r700_2_a_refused_mode_leaves_the_modes_after_it_unanalyzed() {
    // each mode is its own instructions (700.2), lowered in printed order: past a refused one
    // the modes after it are not analyzed; refused last, it hides nothing
    let d = CardDef::new("Odd", "{2}", "Sorcery", "Choose one —\n• Tap or untap target permanent.\n• Draw a card.", None).expect("the grammar reads it");
    let all = blockers(&d);
    cite!(["700.2"]; assert_eq!(reasons(&all), ["program: action not lowered: TapOrUntap"]));
    assert_eq!(all[0].unanalyzed, ["Odd, ability 1, 1 instruction its lowering did not reach: it stops at its first refusal"]);
    let d = CardDef::new("Odd", "{2}", "Sorcery", "Choose one —\n• Draw a card.\n• Tap or untap target permanent.", None).expect("the grammar reads it");
    let all = blockers(&d);
    assert_eq!(reasons(&all), ["program: action not lowered: TapOrUntap"]);
    assert!(all[0].unanalyzed.is_empty(), "{all:?}");
}

#[test]
fn r714_2_a_refusal_that_skips_an_ability_says_so_and_the_next_ability_is_still_checked() {
    // a chapter ability is a Saga's (714.2): on another permanent the engine skips the rest of
    // that ability's checks, and says it did; the next paragraph (113.2c) is still checked
    let d = CardDef::new("Odd", "{2}", "Enchantment", "I — Draw a card.\nFear", None).expect("the grammar reads it");
    let all = blockers(&d);
    cite!(["714.2", "113.2c"]; assert_eq!(reasons(&all), ["chapter ability of a permanent that is not a Saga", "keyword: Fear"]));
    assert_eq!(all[0].unanalyzed, ["the rest of Odd, ability 1: its checks are skipped"]);
}

#[test]
fn r113_3a_a_triggered_ability_of_a_sorcery_does_not_hide_its_spell_abilities() {
    // of an instant or sorcery, text other than a triggered, activated or stack-functioning
    // static ability is a spell ability (113.3a), and each paragraph is its own ability
    // (113.2c): Killian's Confidence's graveyard trigger (113.6k) is refused, its own checks
    // skipped, and its spell ability is still checked: "Draw a card." hides nothing more
    let trigger = "Whenever one or more creatures you control deal combat damage to a player, you may pay {W/B}. If you do, return this card from your graveyard to your hand.";
    let d = CardDef::new("Odd", "{W}{B}", "Sorcery", &format!("Target creature gets +1/+1 until end of turn. Draw a card.\n{trigger}"), None).expect("the grammar reads it");
    let all = blockers(&d);
    cite!(["113.3a", "113.2c"]; assert_eq!(reasons(&all), ["ability on a spell: Triggered"]));
    assert_eq!(check(&d).unwrap_err().0, all[0].reason);
    assert_eq!(all[0].unanalyzed, ["the rest of Odd, ability 2: its checks are skipped"]);
    // its spell ability refused too, that is a blocker of its own, after the one admission gives
    let d = CardDef::new("Odd", "{W}{B}", "Sorcery", &format!("Tap or untap target permanent.\n{trigger}"), None).expect("the grammar reads it");
    let all = blockers(&d);
    assert_eq!(reasons(&all), ["ability on a spell: Triggered", "program: action not lowered: TapOrUntap"]);
    assert_eq!(check(&d).unwrap_err().0, all[0].reason);
    assert_eq!(all[0].unanalyzed, ["the rest of Odd, ability 2: its checks are skipped"]);
    assert!(all[1].unanalyzed.is_empty(), "{all:?}");
}

#[test]
fn r702_42a_a_refused_mode_leaves_whether_an_entwine_spell_is_modal_unanalyzed() {
    // entwine is a static ability of modal spells (702.42a): whether the spell is modal is read
    // of its bodies, so with a mode refused, that check, like the modes after it (700.2), is
    // not reached
    let d = CardDef::new("Odd", "{2}", "Sorcery", "Choose one —\n• Tap or untap target permanent.\n• Draw a card.\nEntwine {2}", None).expect("the grammar reads it");
    let all = blockers(&d);
    cite!(["702.42a", "700.2"]; assert_eq!(reasons(&all), ["program: action not lowered: TapOrUntap"]));
    assert_eq!(check(&d).unwrap_err().0, all[0].reason);
    assert_eq!(all[0].unanalyzed, [
        "Odd, its spell's modes, whether it is modal, as entwine requires (702.42): checked of its bodies",
        "Odd, ability 1, 1 instruction its lowering did not reach: it stops at its first refusal",
    ]);
}

#[test]
fn r113_2c_each_row_the_grammar_does_not_read_is_a_blocker_with_where_it_stopped() {
    // no rule reads "Zorp" or "frobnicate": the grammar declines the first row at its first
    // word, reads the second, and declines the third where its trigger condition ends. Each
    // row is read on its own (113.2c); what follows each stop is the row's unanalyzed rest
    let f = face("Creature — Bear", "Zorp the blarg.\nFlying\nWhenever this creature attacks, frobnicate the gizmo.", Some(("2", "2")));
    let read = CardDef::from_face(&f).map(|_| ()).unwrap_err();
    let all = CardDef::unread_face(&f);
    cite!(["113.2c"]; assert_eq!(reasons(&all), ["text the grammar does not read", "text the grammar does not read"]));
    assert_eq!(read, all[0].reason, "the first blocker is the error reading gives");
    assert!(all.iter().all(|b| b.stage == Stage::Parse));
    let rows: Vec<_> = all.iter().map(|b| b.row.as_ref().expect("a text row")).collect();
    assert_eq!(rows.iter().map(|r| (r.index, r.mode)).collect::<Vec<_>>(), [(0, false), (2, false)]);
    let stops: Vec<_> = rows.iter().map(|r| r.stop.as_ref().expect("declined, not ambiguous")).collect();
    assert_eq!((stops[0].read.as_str(), stops[0].at.as_str(), stops[0].rest.as_str()), ("", "Zorp", "Zorp the blarg."));
    assert_eq!((stops[1].read.as_str(), stops[1].at.as_str(), stops[1].rest.as_str()), ("Whenever ~ attacks,", "frobnicate", "frobnicate the gizmo."));
    assert_eq!(all[1].label(), "text the grammar does not read: at \"frobnicate\"");
    assert_eq!(all[0].unanalyzed, ["the rest of row 0: \"Zorp the blarg.\""]);
    assert_eq!(all[1].unanalyzed, ["the rest of row 2: \"frobnicate the gizmo.\""]);
}

#[test]
fn r208_2a_a_face_not_read_names_what_only_its_text_could_tell() {
    // a printed "*" is a number only by a characteristic-defining ability of the text (208.2a):
    // with the text not read, the power is left unanalyzed, not refused. The type line is read
    // apart from the text, and its own refusal kept
    let f = face("Conspiracy", "Zorp the blarg.", Some(("*", "2")));
    let read = CardDef::from_face(&f).map(|_| ()).unwrap_err();
    let all = CardDef::unread_face(&f);
    cite!(["208.2a"]; assert_eq!(reasons(&all), ["text the grammar does not read", "type Conspiracy"]));
    assert_eq!(read, all[0].reason);
    assert_eq!(all[0].unanalyzed, ["the rest of row 0: \"Zorp the blarg.\"", "the printed power \"*\": read with the abilities of the text (208.2a, 721.2b)"]);
    assert!(all[1].row.is_none() && all[1].unanalyzed.is_empty());
    // read, the same "*" with no ability defining it is refused as before
    let f = face("Creature — Bear", "Flying", Some(("*", "2")));
    assert_eq!(reasons(&CardDef::unread_face(&f)), ["power *"]);
    assert_eq!(CardDef::from_face(&f).map(|_| ()).unwrap_err(), "power *");
}

#[test]
fn an_admitted_card_has_no_blockers() {
    let d = CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).expect("the grammar reads it");
    assert!(check(&d).is_ok());
    assert!(blockers(&d).is_empty());
    let f = face("Creature — Bear", "Flying", Some(("2", "2")));
    assert!(CardDef::unread_face(&f).is_empty());
}

#[test]
fn r722_2_a_prepare_spell_refusal_is_a_blocker_of_its_card() {
    // a preparation card is one card whose prepare spell is part of it (722.2, 722.2c): the
    // card is refused for its prepare spell's refusal, and lists it
    let normal = CardDef::new("Chaser", "{1}{R}", "Creature — Elf", "", Some((2, 2))).expect("the grammar reads it");
    let spell = CardDef::new("Arc", "{R}", "Sorcery", "Tap or untap target permanent.", None).expect("the grammar reads it");
    let d = CardDef::preparation(normal, spell).expect("a preparation card");
    cite!(["722.2"]; assert_eq!(check(&d).unwrap_err().0, "program: action not lowered: TapOrUntap"));
    assert_eq!(reasons(&blockers(&d)), ["program: action not lowered: TapOrUntap"]);
}

#[test]
fn the_blockers_of_every_card_read_whole_begin_with_its_refusal() {
    // of each card whose faces are joined (split, adventure, omen, prepare, double-faced),
    // the collecting checks meet what admission meets, first: none when admitted
    let catalog = CardDef::catalog(&FACES);
    let mut seen = std::collections::BTreeSet::new();
    for card in catalog.values().filter(|c| c.len() > 1) {
        if !seen.insert(&card[0].oracle_id) || !matches!(card[0].layout.as_str(), "split" | "adventure" | "prepare" | "transform" | "modal_dfc") { continue; }
        match CardDef::from_faces(card) {
            Ok(d) => match check(&d) {
                Ok(()) => assert!(blockers(&d).is_empty(), "{}", card[0].name),
                Err(e) => assert_eq!(blockers(&d).first().map(|b| b.reason.as_str()), Some(e.0.as_str()), "{}", card[0].name),
            },
            Err(e) => {
                let unread: Vec<_> = card.iter().flat_map(|f| CardDef::unread_face(f)).collect();
                assert_eq!(unread.first().map(|b| b.reason.as_str()), Some(e.as_str()), "{}", card[0].name);
            }
        }
    }
}
