//! "As this creature enters, choose a player": a player chosen as the permanent enters
//! (614.12a), which the ability linked to that choice reads as "the chosen player" (607.2d):
//! protection from that player (702.16), a count of that player's hand, damage dealt to that
//! player, and that player's upkeep.
use crate::support;
use support::*;

/// Cast `name` and choose `chosen` among `among` as it enters.
fn enters_choosing(t: &mut T, name: &str, cost: &str, spend: &str, among: Vec<usize>, chosen: usize) {
    t.mana(0, cost).cast(0, name, vec![], mana_pool(spend)).unwrap();
    t.pass_all();
    cite!(["614.12a"]; assert_eq!(t.asked(), Some((0, Asked::Player { among }))));
    t.answer(0, Answer::Player(chosen)).unwrap();
    t.resolve();
    t.find_in_battlefield(name);
}

// Rules: 607.2d, 702.16b
// Given: True-Name Nemesis entered with player 1 chosen; player 1 holds Lightning Bolt.
// When: player 1 casts Lightning Bolt aimed at it.
// Expect: any player could be chosen; the Nemesis can't be the target of a spell player 1
// controls.
#[test]
fn r702_16b_true_name_nemesis_cant_be_targeted_by_the_chosen_players_spells() {
    let mut t = Board::new(2).hand(0, &["True-Name Nemesis"]).hand(1, &["Lightning Bolt"]).build();
    enters_choosing(&mut t, "True-Name Nemesis", "{1}{U}{U}", "CUU", vec![0, 1], 1);
    t.g.act(0, Move::Pass).unwrap();
    t.mana(1, "{R}");
    let bolt = t.cast(1, "Lightning Bolt", vec!["True-Name Nemesis".into()], mana_pool("R")).map(|_| ());
    cite!(["702.16b"]; refused("protection"): bolt);
}

// Rules: 607.2d, 702.16b
// Given: True-Name Nemesis entered with player 1 chosen; its controller holds Lightning Bolt.
// When: its controller aims Lightning Bolt at it.
// Expect: the bolt is cast and destroys it: protection is from the chosen player only.
#[test]
fn r702_16b_true_name_nemesis_can_be_targeted_by_a_player_not_chosen() {
    let mut t = Board::new(2).hand(0, &["True-Name Nemesis", "Lightning Bolt"]).build();
    enters_choosing(&mut t, "True-Name Nemesis", "{1}{U}{U}", "CUU", vec![0, 1], 1);
    t.mana(0, "{R}");
    cite!(["702.16b"]; ok: t.cast(0, "Lightning Bolt", vec!["True-Name Nemesis".into()], mana_pool("R")));
    t.resolve();
    t.expect().rules(["120.3e", "704.5g"]).zone(0, Zone::Graveyard, &["Lightning Bolt", "True-Name Nemesis"]).check();
}

// Rules: 607.2d, 702.16e
// Given: True-Name Nemesis (3/1) entered with player 1 chosen, beside its controller's
// Grizzly Bears.
// When: player 1 casts Fiery Cannonade (2 damage to each creature that isn't a Pirate).
// Expect: the damage to the Nemesis is prevented; the Bears die.
#[test]
fn r702_16e_damage_from_the_chosen_players_spell_is_prevented() {
    let mut t = Board::new(2).hand(0, &["True-Name Nemesis"]).battlefield(0, &["Grizzly Bears"]).hand(1, &["Fiery Cannonade"]).build();
    enters_choosing(&mut t, "True-Name Nemesis", "{1}{U}{U}", "CUU", vec![0, 1], 1);
    t.g.act(0, Move::Pass).unwrap();
    t.mana(1, "{2}{R}").cast(1, "Fiery Cannonade", vec![], mana_pool("CCR")).unwrap();
    t.resolve();
    t.expect().rules(["702.16e"]).zone(0, Zone::Battlefield, &["True-Name Nemesis"]).zone(0, Zone::Graveyard, &["Grizzly Bears"]).check();
}

// Rules: 607.2d, 611.3a
// Given: Nyxathid (7/7) entered with its controller's opponent chosen; that opponent holds
// three cards.
// When: the opponent then draws a card.
// Expect: it is 4/4, then 3/3: -1/-1 for each card in the chosen player's hand.
#[test]
fn r607_2d_nyxathid_shrinks_with_the_chosen_players_hand() {
    let mut t = Board::new(2).hand(0, &["Nyxathid"]).hand(1, &["Lightning Bolt", "Giant Growth", "Mountain"])
        .library(1, &["Forest"]).build();
    enters_choosing(&mut t, "Nyxathid", "{1}{B}{B}", "CBB", vec![1], 1);
    t.expect().rules(["607.2d"]).pt("Nyxathid", 4, 4).check();
    t.g.draw(1, 1);
    t.expect().rules(["611.3a"]).pt("Nyxathid", 3, 3).check();
}

// Rules: 607.2d, 603.2
// Given: Sewer Nemesis entered with player 1 chosen; player 1's graveyard holds two cards.
// When: player 1 casts Lightning Bolt at its controller.
// Expect: it is 2/2; player 1 mills a card as the bolt waits, and once the bolt is in their
// graveyard too it is 4/4.
#[test]
fn r607_2d_sewer_nemesis_mills_the_chosen_player_who_casts_a_spell() {
    let mut t = Board::new(2).hand(0, &["Sewer Nemesis"]).hand(1, &["Lightning Bolt"])
        .graveyard(1, &["Mountain", "Forest"]).library(1, &["Island"]).build();
    enters_choosing(&mut t, "Sewer Nemesis", "{3}{B}", "CCCB", vec![0, 1], 1);
    t.expect().rules(["604.3", "607.2d"]).pt("Sewer Nemesis", 2, 2).check();
    t.g.act(0, Move::Pass).unwrap();
    t.mana(1, "{R}").cast(1, "Lightning Bolt", vec![Tgt::Player(0)], mana_pool("R")).unwrap();
    t.resolve();
    t.expect().rules(["603.2", "607.2d"]).zone(1, Zone::Graveyard, &["Forest", "Island", "Lightning Bolt", "Mountain"])
        .life(0, 17).pt("Sewer Nemesis", 4, 4).check();
}

// Rules: 607.2d, 503.1a
// Given: Black Vise entered with its controller's opponent chosen; that opponent holds six
// cards.
// When: the opponent's turn begins.
// Expect: at the beginning of their upkeep it deals them 2 damage (six cards minus 4).
#[test]
fn r607_2d_black_vise_triggers_at_the_chosen_players_upkeep() {
    let mut t = Board::new(2).hand(0, &["Black Vise"])
        .hand(1, &["Lightning Bolt", "Giant Growth", "Mountain", "Forest", "Island", "Swamp"]).fill(5).build();
    enters_choosing(&mut t, "Black Vise", "{1}", "C", vec![1], 1);
    t.to_no_combat(1, Part::Upkeep);
    t.expect().rules(["503.1a", "607.2d"]).life(1, 18).check();
}
