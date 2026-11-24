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

/// Play on to player `p`'s next declaration of attackers, declaring none and passing.
fn to_attackers(t: &mut T, p: usize) {
    let turn = t.g.turn;
    for _ in 0..2000 {
        match t.g.decision.clone() {
            Some(Decision::Attackers { player }) if player == p && t.g.turn > turn => return,
            Some(Decision::Attackers { player }) => { t.g.act(player, Move::Attack(vec![])).unwrap(); }
            Some(Decision::Blockers { player }) => { t.g.act(player, Move::Block(vec![])).unwrap(); }
            _ => { t.g.act(t.g.decision_maker(t.g.priority), Move::Pass).unwrap(); }
        }
    }
    panic!("never reached player {p}'s attackers");
}

/// True-Name Nemesis entered for player 0 with player 1 chosen, in a three-player game where
/// players 1 and 2 each control Grizzly Bears; it attacks `defender` on its controller's next
/// turn, and the game is past the declaration of attackers.
fn nemesis_attacks(defender: usize) -> (T, ObjId) {
    let mut t = Board::new(3).hand(0, &["True-Name Nemesis"]).battlefield(1, &["Grizzly Bears"])
        .battlefield(2, &["Grizzly Bears"]).fill(5).build();
    enters_choosing(&mut t, "True-Name Nemesis", "{1}{U}{U}", "CUU", vec![0, 1, 2], 1);
    to_attackers(&mut t, 0);
    let nemesis = t.find_in_battlefield("True-Name Nemesis");
    t.g.act(0, Move::Attack(vec![(nemesis, Target::Player(defender))])).unwrap();
    (t, nemesis)
}

/// Pass priority until `done` holds.
fn pass_until(t: &mut T, done: impl Fn(&Game) -> bool) {
    for _ in 0..20 {
        if done(&t.g) { return; }
        t.g.act(t.g.decision_maker(t.g.priority), Move::Pass).unwrap();
    }
    panic!("never reached");
}

// Rules: 607.2d, 702.16f, 510.1a
// Given: True-Name Nemesis (3/1), with player 1 chosen, attacks player 1, who controls
// Grizzly Bears.
// When: combat goes on.
// Expect: the Bears can't block it, a creature the chosen player controls; it is unblocked
// and deals 3 damage to player 1.
#[test]
fn r702_16f_the_chosen_players_creatures_cant_block_true_name_nemesis() {
    let (mut t, nemesis) = nemesis_attacks(1);
    let bears = t.find_in(1, Zone::Battlefield, "Grizzly Bears");
    cite!(["702.16f"]; assert!(t.g.cant_block(bears, nemesis, &[bears])));
    pass_until(&mut t, |g| matches!(g.decision, Some(Decision::Blockers { .. })) || g.players[1].life < 20);
    if let Some(Decision::Blockers { player }) = t.g.decision {
        cite!(["702.16f"]; refused("can't block"): t.g.act(player, Move::Block(vec![(bears, nemesis)])));
        t.g.act(player, Move::Block(vec![])).unwrap();
        pass_until(&mut t, |g| g.players[1].life < 20);
    }
    t.expect().rules(["510.1a"]).life(1, 17).zone(1, Zone::Battlefield, &["Grizzly Bears"]).check();
}

// Rules: 607.2d, 702.16f, 510.2
// Given: True-Name Nemesis (3/1), with player 1 chosen, attacks player 2, who controls
// Grizzly Bears (2/2).
// When: player 2 blocks with the Bears and combat damage is dealt.
// Expect: the block stands: protection is from player 1 only; each deals its damage to the
// other, and both die.
#[test]
fn r702_16f_another_players_creature_blocks_true_name_nemesis() {
    let (mut t, nemesis) = nemesis_attacks(2);
    let bears = t.find_in(2, Zone::Battlefield, "Grizzly Bears");
    pass_until(&mut t, |g| matches!(g.decision, Some(Decision::Blockers { .. })));
    assert_eq!(t.g.decision, Some(Decision::Blockers { player: 2 }));
    cite!(["702.16f"]; ok: t.g.act(2, Move::Block(vec![(bears, nemesis)])));
    for _ in 0..10 {
        if !t.g.battlefield.contains(&nemesis) { break; }
        t.g.act(t.g.decision_maker(t.g.priority), Move::Pass).unwrap();
    }
    t.expect().rules(["510.2", "704.5g"]).zone(0, Zone::Graveyard, &["True-Name Nemesis"])
        .zone(2, Zone::Graveyard, &["Grizzly Bears"]).zone(1, Zone::Battlefield, &["Grizzly Bears"]).check();
}

// Rules: 607.2d, 702.16c, 704.5m
// Given: True-Name Nemesis, with player 1 chosen, enchanted by its controller's Holy Strength.
// When: player 1 casts Kefnet's Last Word to gain control of Holy Strength.
// Expect: an Aura the chosen player controls can't enchant it: Holy Strength is put into its
// owner's graveyard as a state-based action, and the Nemesis is 3/1 again.
#[test]
fn r702_16c_an_aura_the_chosen_player_controls_falls_off_true_name_nemesis() {
    let mut t = Board::new(2).hand(0, &["True-Name Nemesis", "Holy Strength"]).hand(1, &["Kefnet's Last Word"]).fill(5).build();
    enters_choosing(&mut t, "True-Name Nemesis", "{1}{U}{U}", "CUU", vec![0, 1], 1);
    t.mana(0, "{W}").cast(0, "Holy Strength", vec!["True-Name Nemesis".into()], mana_pool("W")).unwrap();
    t.resolve();
    t.expect().rules(["303.4"]).pt("True-Name Nemesis", 4, 3).check();
    t.to_no_combat(1, Part::PrecombatMain);
    t.mana(1, "{2}{U}{U}").cast(1, "Kefnet's Last Word", vec!["Holy Strength".into()], mana_pool("CCUU")).unwrap();
    t.resolve();
    t.expect().rules(["702.16c", "704.5m"]).zone(0, Zone::Graveyard, &["Holy Strength"])
        .zone(1, Zone::Battlefield, &[]).pt("True-Name Nemesis", 3, 1).check();
}

// Rules: 607.2d, 702.16d, 704.5n
// Given: True-Name Nemesis, with player 1 chosen, equipped with its controller's Bonesplitter.
// When: player 1 casts Kefnet's Last Word to gain control of Bonesplitter.
// Expect: Equipment the chosen player controls can't equip it: Bonesplitter becomes
// unattached as a state-based action and stays on the battlefield under player 1's control,
// and the Nemesis is 3/1 again.
#[test]
fn r702_16d_equipment_the_chosen_player_controls_becomes_unattached_from_true_name_nemesis() {
    let mut t = Board::new(2).hand(0, &["True-Name Nemesis"]).battlefield(0, &["Bonesplitter"]).hand(1, &["Kefnet's Last Word"]).fill(5).build();
    enters_choosing(&mut t, "True-Name Nemesis", "{1}{U}{U}", "CUU", vec![0, 1], 1);
    let splitter = t.find_in_battlefield("Bonesplitter");
    let equip = t.g.abilities(splitter).iter().position(|a| matches!(&**a, ir::Ability::Activated { .. })).expect("equip");
    t.mana(0, "{1}").activate(0, "Bonesplitter", equip, vec!["True-Name Nemesis".into()], mana_pool("C")).unwrap();
    t.resolve();
    t.expect().rules(["702.6a"]).pt("True-Name Nemesis", 5, 1).check();
    t.to_no_combat(1, Part::PrecombatMain);
    t.mana(1, "{2}{U}{U}").cast(1, "Kefnet's Last Word", vec!["Bonesplitter".into()], mana_pool("CCUU")).unwrap();
    t.resolve();
    let splitter = t.find_in_battlefield("Bonesplitter");
    cite!(["702.16d", "704.5n"]; assert_eq!((t.g.controller(splitter), t.g.attached_to(splitter)), (1, None)));
    t.expect().rules(["702.16d"]).pt("True-Name Nemesis", 3, 1).check();
}
