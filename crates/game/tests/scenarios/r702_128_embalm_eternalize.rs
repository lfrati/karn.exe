//! Embalm (702.128a): "[Cost], Exile this card from your graveyard: Create a token that's a
//! copy of this card, except it's white, it has no mana cost, and it's a Zombie in addition to
//! its other types. Activate only as a sorcery." Eternalize (702.129a) is the same, except the
//! token is black and 4/4. Every card is read from its Oracle text.
use crate::support;
use support::*;
use ir::{CardType, Color};

/// Activate the first activated ability of the card at `source`: its embalm or eternalize.
fn activate(t: &mut T, source: ObjId, mana: &str) -> Result<(), String> {
    let ability = t.g.abilities(source).iter().position(|a| matches!(&**a, ir::Ability::Activated { .. })).expect("embalm or eternalize");
    t.g.act(0, paid(mana, Move::Activate { source, ability, targets: vec![], paying: vec![] }))
}

/// The one token on player 0's battlefield.
fn token(t: &T) -> ObjId {
    let tokens: Vec<_> = t.g.battlefield.iter().copied().filter(|o| t.g.obj(*o).token()).collect();
    let [token] = tokens[..] else { panic!("one token: {tokens:?}") };
    token
}

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Sacred Cat", "Proven Combatant"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

#[test]
fn r702_128a_embalm_makes_a_white_zombie_copy_with_no_mana_cost() {
    // Sacred Cat: {W}, 1/1 Cat, "Lifelink / Embalm {W}"
    let mut t = Board::new(2).named(0, Zone::Graveyard, "cat", "Sacred Cat").build();
    let cat = t.id("cat");
    t.mana(0, "{W}");
    cite!(["702.128a", "602.2"]; ok: activate(&mut t, cat, "W"));
    // exiling it is part of the cost (602.2b)
    t.expect().rules(["702.128a", "602.2b"]).zone_exact(0, Zone::Exile, &["Sacred Cat"]).zone_exact(0, Zone::Graveyard, &[]).check();
    t.resolve();
    let token = token(&t);
    let (seen, chars) = (t.g.seen(token, None), &t.g.obj(token).chars);
    // Ruling: "The token is a Zombie in addition to its other types and is white instead of
    // its other colors. It has no mana cost, and thus its mana value is 0."
    cite!(["702.128a", "707.9b"]; assert_eq!((chars.name.as_str(), seen.colors.clone(), chars.mana_value, seen.pt), ("Sacred Cat", vec![Color::White], 0, Some((1, 1)))));
    cite!(["702.128a", "707.9b"]; assert!(seen.types.contains(&CardType::Creature) && ["Zombie", "Cat"].iter().all(|s| seen.subtypes.iter().any(|t| t.name() == *s)), "{:?}", seen.subtypes));
    cite!(["702.128a", "707.2"]; assert!(t.g.has(token, "Lifelink")));
}

#[test]
fn r702_129a_eternalize_makes_a_black_4_4_zombie_copy_with_no_mana_cost() {
    // Proven Combatant: {U}, 1/1 Human Warrior, "Eternalize {4}{U}{U}"
    let mut t = Board::new(2).named(0, Zone::Graveyard, "combatant", "Proven Combatant").build();
    let combatant = t.id("combatant");
    t.mana(0, "{4}{U}{U}");
    cite!(["702.129a", "602.2"]; ok: activate(&mut t, combatant, "CCCCUU"));
    t.resolve();
    let token = token(&t);
    let (seen, chars) = (t.g.seen(token, None), &t.g.obj(token).chars);
    // Ruling: "The token is a Zombie in addition to its other types and is black instead of
    // its other colors. Its base power and toughness are 4/4. It has no mana cost."
    cite!(["702.129a", "707.9b"]; assert_eq!((chars.name.as_str(), seen.colors.clone(), chars.mana_value, seen.pt), ("Proven Combatant", vec![Color::Black], 0, Some((4, 4)))));
    cite!(["702.129a", "707.9b"]; assert!(["Zombie", "Human", "Warrior"].iter().all(|s| seen.subtypes.iter().any(|t| t.name() == *s)), "{:?}", seen.subtypes));
}

#[test]
fn r702_128a_it_is_activated_only_as_a_sorcery_from_a_graveyard() {
    let mut t = Board::new(2).fill(5).named(0, Zone::Graveyard, "cat", "Sacred Cat").named(0, Zone::Hand, "held", "Sacred Cat").build();
    let (cat, held) = (t.id("cat"), t.id("held"));
    t.mana(0, "{W}");
    cite!(["702.128a", "113.6m"]; refused("functions only in the zone"): activate(&mut t, held, "W"));
    // in another player's main phase, with that player passing to it and the stack empty
    t.to_no_combat(1, Part::PrecombatMain);
    t.g.act(1, Move::Pass).unwrap();
    t.mana(0, "{W}");
    cite!(["702.128a", "602.5d"]; refused("only as a sorcery"): activate(&mut t, cat, "W"));
}
