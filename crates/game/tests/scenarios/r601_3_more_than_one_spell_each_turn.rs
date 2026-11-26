//! "Each player can't cast more than one spell each turn.", "Each player can't cast more than
//! one noncreature spell each turn.": a prohibition (101.2) on casting a spell (601.3) once
//! that player has cast that many such spells this turn, counting the spells they cast before
//! it applied and those that were countered (Arcane Laboratory, Deafening Silence, Archon of
//! Emeria and High Noon rulings). And "This creature can't be blocked this turn except by
//! creatures with haste.": a prohibition that lasts the turn the phrase names (611.2a).
use crate::support;
use support::*;

const REFUSED: &str = "an effect prohibits casting that spell (601.3)";

// Rules: 101.2, 601.3
// Given: player 0 controls Arcane Laboratory ("Each player can't cast more than one spell each
// turn.") and holds Shock and Lightning Bolt; player 1 holds Shock and Lightning Bolt too.
// When: each player casts a spell, then tries another; then player 0 casts again on player 1's
// turn.
// Expect: each player's second spell of the turn is refused; on the next turn player 0 may cast
// one again.
#[test]
fn r601_3_arcane_laboratory_allows_each_player_one_spell_each_turn() {
    let mut t = Board::new(2).battlefield(0, &["Arcane Laboratory"]).hand(0, &["Shock", "Lightning Bolt"])
        .hand(1, &["Shock", "Lightning Bolt"]).fill(10).build();
    t.to_no_combat(0, Part::PrecombatMain);
    t.mana(0, "{R}").cast(0, "Shock", vec![Tgt::Player(1)], mana_pool("R")).unwrap();
    t.resolve();
    t.mana(0, "{R}");
    cite!(["101.2", "601.3"]; refused(REFUSED): t.cast(0, "Lightning Bolt", vec![Tgt::Player(1)], mana_pool("R")));
    // player 0 passes; player 1 has priority in the same step
    t.g.act(0, Move::Pass).unwrap();
    t.mana(1, "{R}").cast(1, "Shock", vec![Tgt::Player(0)], mana_pool("R")).unwrap();
    t.resolve();
    // the active player has priority after it resolves (117.3b)
    t.g.act(0, Move::Pass).unwrap();
    t.mana(1, "{R}");
    cite!(["101.2", "601.3"]; refused(REFUSED): t.cast(1, "Lightning Bolt", vec![Tgt::Player(0)], mana_pool("R")));
    t.g.players[1].pool = mana_pool("");
    t.to_no_combat(1, Part::Upkeep);
    t.g.act(1, Move::Pass).unwrap();
    t.mana(0, "{R}").cast(0, "Lightning Bolt", vec![Tgt::Player(1)], mana_pool("R")).unwrap();
    t.resolve();
    cite!(["601.3"]; assert_eq!(t.g.players[1].life, 15));
}

// Rules: 101.2, 601.3
// Given: player 0 holds Shock, Deafening Silence ("Each player can't cast more than one
// noncreature spell each turn."), Lightning Bolt, Grizzly Bears and Llanowar Elves.
// When: player 0 casts Shock, then Deafening Silence, then tries Lightning Bolt, then casts the
// creatures.
// Expect: Deafening Silence may be cast, as it isn't on the battlefield yet; once it is, Bolt is
// refused, since Shock and Deafening Silence were cast this turn; the creature spells are not
// counted nor refused.
#[test]
fn r601_3_deafening_silence_counts_noncreature_spells_cast_before_it_applied() {
    let mut t = Board::new(2).hand(0, &["Shock", "Deafening Silence", "Lightning Bolt", "Grizzly Bears", "Llanowar Elves"]).fill(10).build();
    t.to_no_combat(0, Part::PrecombatMain);
    t.mana(0, "{R}").cast(0, "Shock", vec![Tgt::Player(1)], mana_pool("R")).unwrap();
    t.resolve();
    t.mana(0, "{W}").cast(0, "Deafening Silence", vec![], mana_pool("W")).unwrap();
    t.resolve();
    t.mana(0, "{R}");
    cite!(["101.2", "601.3"]; refused(REFUSED): t.cast(0, "Lightning Bolt", vec![Tgt::Player(1)], mana_pool("R")));
    t.g.players[0].pool = mana_pool("");
    t.mana(0, "{1}{G}").cast(0, "Grizzly Bears", vec![], mana_pool("CG")).unwrap();
    t.resolve();
    t.mana(0, "{G}").cast(0, "Llanowar Elves", vec![], mana_pool("G")).unwrap();
    t.resolve();
    t.expect().rules(["601.3"]).contains(0, Zone::Battlefield, "Grizzly Bears").contains(0, Zone::Battlefield, "Llanowar Elves").check();
}

// Rules: 101.2, 601.3
// Given: player 0 holds Archon of Emeria ("Each player can't cast more than one spell each
// turn.") and Shock.
// When: player 0 casts the Archon and it resolves, then tries Shock.
// Expect: Shock is refused: the Archon was this turn's spell.
#[test]
fn r601_3_archon_of_emeria_counts_itself() {
    let mut t = Board::new(2).hand(0, &["Archon of Emeria", "Shock"]).fill(10).build();
    t.to_no_combat(0, Part::PrecombatMain);
    t.mana(0, "{2}{W}").cast(0, "Archon of Emeria", vec![], mana_pool("CCW")).unwrap();
    t.resolve();
    t.mana(0, "{R}");
    cite!(["101.2", "601.3"]; refused(REFUSED): t.cast(0, "Shock", vec![Tgt::Player(1)], mana_pool("R")));
}

// Rules: 101.2, 601.2i, 701.6a
// Given: player 0 controls High Noon ("Each player can't cast more than one spell each turn.")
// and holds Shock and Lightning Bolt; player 1 holds Counterspell.
// When: player 0 casts Shock, player 1 counters it, and player 0 tries Lightning Bolt.
// Expect: the countered Shock was cast all the same, so Bolt is refused.
#[test]
fn r601_2i_high_noon_counts_a_countered_spell() {
    let mut t = Board::new(2).battlefield(0, &["High Noon"]).hand(0, &["Shock", "Lightning Bolt"]).hand(1, &["Counterspell"]).fill(10).build();
    t.to_no_combat(0, Part::PrecombatMain);
    t.mana(0, "{R}").cast(0, "Shock", vec![Tgt::Player(1)], mana_pool("R")).unwrap();
    let shock = *t.g.stack.last().unwrap();
    t.g.act(0, Move::Pass).unwrap();
    t.mana(1, "{U}{U}").cast(1, "Counterspell", vec![Tgt::Object(shock)], mana_pool("UU")).unwrap();
    t.resolve();
    t.expect().rules(["701.6a"]).contains(0, Zone::Graveyard, "Shock").check();
    cite!(["701.6a"]; assert_eq!(t.g.players[1].life, 20));
    t.mana(0, "{R}");
    cite!(["101.2", "601.2i"]; refused(REFUSED): t.cast(0, "Lightning Bolt", vec![Tgt::Player(1)], mana_pool("R")));
}

// Rules: 509.1b, 611.2a, 514.2
// Given: player 0 controls Gingerbrute ("{1}: This creature can't be blocked this turn except by
// creatures with haste."); player 1 controls Grizzly Bears and Raging Goblin (haste).
// When: player 0 activates the ability.
// Expect: before it, the Bears could block Gingerbrute; after it, only the Goblin may; on the
// next turn the Bears may again.
#[test]
fn r509_1b_gingerbrute_cant_be_blocked_this_turn_except_by_creatures_with_haste() {
    let mut t = Board::new(2).battlefield(0, &["Gingerbrute"]).battlefield(1, &["Grizzly Bears", "Raging Goblin"]).fill(10).build();
    let (ginger, bears, goblin) = (t.find_in_battlefield("Gingerbrute"), t.find_in_battlefield("Grizzly Bears"), t.find_in_battlefield("Raging Goblin"));
    t.to_no_combat(0, Part::PrecombatMain);
    cite!(["509.1b"]; assert!(!t.g.cant_block(bears, ginger, &[bears])));
    let ability = t.g.def(ginger).unwrap().abilities().iter().position(|a| matches!(a, ir::Ability::Activated { .. })).unwrap();
    t.mana(0, "{1}").activate(0, "Gingerbrute", ability, vec![], mana_pool("C")).unwrap();
    t.resolve();
    cite!(["509.1b", "611.2a"]; assert!(t.g.cant_block(bears, ginger, &[bears]), "the Bears has no haste"));
    cite!(["509.1b"]; assert!(!t.g.cant_block(goblin, ginger, &[goblin]), "the Goblin has haste"));
    t.to_no_combat(1, Part::Upkeep);
    cite!(["514.2"]; assert!(!t.g.cant_block(bears, ginger, &[bears]), "it lasted only that turn"));
}
