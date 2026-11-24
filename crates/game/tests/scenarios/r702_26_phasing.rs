//! Phasing (702.26). A permanent that phases out is treated as though it does not exist
//! (702.26b) and phases in, the same permanent, before its controller untaps during their
//! next untap step (502.1): phasing is no zone change (702.26d). What is attached to it phases
//! out and in with it (702.26g). Every card is read from its Oracle text.
use crate::support;
use game::state::Decision;
use ir::Ability;
use support::*;

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Teferi's Drake", "Blink Dog", "Haystack", "Teferi's Curse", "Shock", "Act of Treason", "Banishing Light", "Clever Concealment", "Grizzly Bears", "Lightning Bolt"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

/// The index of its one activated ability.
fn activated(t: &T, label: &str) -> usize {
    let o = t.id(label);
    t.g.abilities(o).iter().position(|a| matches!(&**a, Ability::Activated { .. })).expect("an activated ability")
}

fn phased_out(t: &T, o: ObjId) -> bool {
    t.g.obj(o).zone == Zone::PhasedOut && t.g.phased_out.contains(&o) && !t.g.battlefield.contains(&o)
}

#[test]
fn r702_26a_a_permanent_with_phasing_phases_out_and_in_before_its_controller_untaps() {
    // Teferi's Drake: "Flying / Phasing"
    let mut t = Board::new(2).fill(6).tapped(0, &["Teferi's Drake"]).build();
    let drake = t.find_in_battlefield("Teferi's Drake");
    t.g.set_initial_counters(drake, "+1/+1", 1);
    // another player's untap step does not phase it
    t.to_no_combat(1, Part::PrecombatMain);
    cite!(["702.26a"]; assert!(t.g.battlefield.contains(&drake)));
    // its controller's does, before they untap: it stays tapped
    t.to_no_combat(0, Part::PrecombatMain);
    cite!(["702.26a", "502.1"]; assert!(phased_out(&t, drake) && t.g.obj(drake).tapped));
    t.to_no_combat(1, Part::PrecombatMain);
    cite!(["702.26a"]; assert!(phased_out(&t, drake)));
    // and in again at their next one, the same permanent, before they untap it
    t.to_no_combat(0, Part::PrecombatMain);
    cite!(["702.26a", "502.1", "502.3"]; assert!(t.g.battlefield.contains(&drake) && t.g.obj(drake).zone == Zone::Battlefield && !t.g.obj(drake).tapped));
    // its counters remain, and it has been its controller's since their turn began (702.26d)
    cite!(["702.26d"]; assert_eq!(t.g.obj(drake).counters.get("+1/+1"), Some(&1)));
    cite!(["702.26d", "302.6"]; assert!(!t.g.summoning_sick(drake)));
    // no zone changed
    cite!(["702.26d"]; assert!(!t.g.log.iter().any(|e| matches!(e, game::Event::ZoneChanged { was, .. } if *was == drake))));
}

#[test]
fn r702_26b_a_phased_out_permanent_can_not_be_affected_and_its_spell_loses_its_target() {
    // Blink Dog: "Teleport — {3}{W}: This creature phases out."
    let mut t = Board::new(2).named(0, Zone::Battlefield, "dog", "Blink Dog").hand(0, &["Shock", "Lightning Bolt"]).build();
    let (dog, teleport) = (t.id("dog"), activated(&t, "dog"));
    let shock = t.find_in(0, Zone::Hand, "Shock");
    t.mana(0, "{R}{3}{W}");
    t.g.act(0, paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Object(dog)]] })).unwrap();
    // in response it phases out
    t.g.act(0, paid("{3}{W}", Move::Activate { source: dog, ability: teleport, targets: vec![], paying: vec![] })).unwrap();
    t.resolve();
    cite!(["702.26b"]; assert!(phased_out(&t, dog)));
    t.resolve();
    // the spell's one target is illegal: it does not resolve (608.2b)
    cite!(["702.26b", "608.2b"]; assert!(t.g.log.iter().any(|e| matches!(e, game::Event::Fizzled { .. }))));
    cite!(["702.26b"]; assert_eq!(t.g.obj(dog).damage, 0));
    // nor can it be targeted
    let bolt = t.find_in(0, Zone::Hand, "Lightning Bolt");
    t.mana(0, "{R}");
    cite!(["702.26b"]; refused("an illegal target, Blink Dog: it isn't a player, or a creature or a planeswalker or a battle on the battlefield"): t.g.act(0, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Object(dog)]] })));
}

#[test]
fn r506_4_an_attacking_creature_that_phases_out_is_removed_from_combat() {
    let mut t = Board::new(2).fill(4).named(0, Zone::Battlefield, "dog", "Blink Dog").build();
    let (dog, teleport) = (t.id("dog"), activated(&t, "dog"));
    while !matches!(t.g.decision, Some(Decision::Attackers { .. })) { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.g.act(0, Move::Attack(vec![(dog, Target::Player(1))])).unwrap();
    cite!(["508.1"]; assert!(t.g.combat.attackers.iter().any(|(a, _)| *a == dog)));
    t.mana(0, "{3}{W}");
    t.g.act(0, paid("{3}{W}", Move::Activate { source: dog, ability: teleport, targets: vec![], paying: vec![] })).unwrap();
    t.resolve();
    cite!(["506.4", "702.26b"]; assert!(phased_out(&t, dog) && !t.g.combat.attackers.iter().any(|(a, _)| *a == dog)));
    while t.g.part != Part::PostcombatMain {
        match t.g.decision.clone() {
            Some(Decision::Blockers { player }) => t.g.act(player, Move::Block(vec![])).unwrap(),
            _ => t.g.act(t.g.priority, Move::Pass).unwrap(),
        }
    }
    cite!(["506.4"]; assert_eq!(t.g.players[1].life, 20));
}

#[test]
fn r702_26g_an_aura_phases_out_and_in_with_the_permanent_it_enchants() {
    // Teferi's Curse: "Enchant artifact or creature / Enchanted permanent has phasing."
    let mut t = Board::new(2).fill(6).named(0, Zone::Battlefield, "bears", "Grizzly Bears").hand(0, &["Teferi's Curse"]).build();
    let bears = t.id("bears");
    t.mana(0, "{1}{U}");
    t.cast(0, "Teferi's Curse", vec![Tgt::Object(bears)], mana_pool("{1}{U}")).unwrap();
    t.resolve();
    let curse = t.find_in_battlefield("Teferi's Curse");
    cite!(["303.4", "702.26a"]; assert!(t.g.attached_to(curse) == Some(Target::Object(bears)) && t.g.has(bears, "Phasing")));
    t.to_no_combat(1, Part::PrecombatMain);
    t.to_no_combat(0, Part::PrecombatMain);
    // the Aura phases out with it, indirectly
    cite!(["702.26a", "702.26g"]; assert!(phased_out(&t, bears) && phased_out(&t, curse)));
    cite!(["702.26g"]; assert!(t.g.obj(curse).phased.is_some_and(|p| p.indirect) && t.g.obj(bears).phased.is_some_and(|p| !p.indirect)));
    t.to_no_combat(1, Part::PrecombatMain);
    t.to_no_combat(0, Part::PrecombatMain);
    // and in with it, still attached to it
    cite!(["702.26g"]; assert!(t.g.battlefield.contains(&bears) && t.g.battlefield.contains(&curse)));
    cite!(["702.26g"]; assert_eq!(t.g.attached_to(curse), Some(Target::Object(bears))));
    // it alternates: the next untap step it phases out again
    t.to_no_combat(1, Part::PrecombatMain);
    t.to_no_combat(0, Part::PrecombatMain);
    cite!(["702.26a"]; assert!(phased_out(&t, bears) && phased_out(&t, curse)));
}

#[test]
fn r514_2_damage_marked_on_a_phased_out_permanent_is_removed_in_cleanup() {
    // Haystack: "{2}, {T}: Target creature you control phases out."
    let mut t = Board::new(2).fill(4).named(0, Zone::Battlefield, "bears", "Grizzly Bears").named(0, Zone::Battlefield, "haystack", "Haystack").build();
    let (bears, haystack) = (t.id("bears"), t.id("haystack"));
    t.g.set_initial_damage(bears, 1).unwrap();
    t.mana(0, "{2}");
    t.g.act(0, paid("{2}", Move::Activate { source: haystack, ability: 0, targets: vec![vec![Target::Object(bears)]], paying: vec![] })).unwrap();
    t.resolve();
    cite!(["702.26b"]; assert!(phased_out(&t, bears) && t.g.obj(bears).damage == 1));
    t.to_no_combat(1, Part::PrecombatMain);
    cite!(["514.2"]; assert!(phased_out(&t, bears) && t.g.obj(bears).damage == 0));
}

#[test]
fn r702_26d_what_an_enchantment_exiled_until_it_leaves_stays_exiled_while_it_is_phased_out() {
    // Banishing Light: "When this enchantment enters, exile target nonland permanent an
    // opponent controls until this enchantment leaves the battlefield."
    // Clever Concealment: "Any number of target nonland permanents you control phase out."
    let mut t = Board::new(2).fill(6).named(1, Zone::Battlefield, "bears", "Grizzly Bears")
        .hand(0, &["Banishing Light", "Clever Concealment"]).build();
    let bears = t.id("bears");
    t.mana(0, "{2}{W}{2}{W}{W}");
    t.cast(0, "Banishing Light", vec![], mana_pool("{2}{W}")).unwrap();
    let asked = t.settle();
    assert!(matches!(asked, Boundary::Question(0, Asked::Targets { .. })), "the trigger's target is asked: {asked:?}");
    t.g.act(0, Move::ChooseTargets(vec![vec![Target::Object(bears)]])).unwrap();
    t.resolve();
    let exiled = t.find_in(1, Zone::Exile, "Grizzly Bears");
    let light = t.find_in_battlefield("Banishing Light");
    t.cast(0, "Clever Concealment", vec![Tgt::Object(light)], mana_pool("{2}{W}{W}")).unwrap();
    t.resolve();
    // phasing out is not leaving the battlefield (702.26d)
    cite!(["702.26d", "610.3"]; assert!(phased_out(&t, light) && t.g.obj(exiled).zone == Zone::Exile));
    t.to_no_combat(1, Part::PrecombatMain);
    t.to_no_combat(0, Part::PrecombatMain);
    cite!(["702.26d", "610.3"]; assert!(t.g.battlefield.contains(&light) && t.g.obj(exiled).zone == Zone::Exile));
}

/// Act of Treason: "Gain control of target creature until end of turn. Untap that creature.
/// It gains haste until end of turn." Player 1 takes player 0's Blink Dog and phases it out.
fn taken_and_phased_out(players: usize) -> (T, ObjId) {
    let mut t = Board::new(players).fill(8).active(1).named(0, Zone::Battlefield, "dog", "Blink Dog").hand(1, &["Act of Treason"]).build();
    let (dog, teleport) = (t.id("dog"), activated(&t, "dog"));
    t.mana(1, "{2}{R}{3}{W}");
    t.cast(1, "Act of Treason", vec![Tgt::Object(dog)], mana_pool("{2}{R}")).unwrap();
    t.resolve();
    cite!(["613.2"]; assert_eq!(t.g.controller(dog), 1));
    t.g.act(1, paid("{3}{W}", Move::Activate { source: dog, ability: teleport, targets: vec![], paying: vec![] })).unwrap();
    t.resolve();
    cite!(["702.26b"]; assert!(phased_out(&t, dog) && t.g.obj(dog).phased.is_some_and(|p| p.under == 1)));
    (t, dog)
}

#[test]
fn r502_1_it_phases_in_in_the_untap_step_of_the_player_who_controlled_it_as_it_phased_out() {
    let (mut t, dog) = taken_and_phased_out(2);
    // the control effect has ended meanwhile (702.26f); its owner's untap step does not phase it in
    t.to_no_combat(0, Part::PrecombatMain);
    cite!(["502.1"]; assert!(phased_out(&t, dog)));
    t.to_no_combat(1, Part::PrecombatMain);
    cite!(["502.1"]; assert!(t.g.battlefield.contains(&dog)));
    cite!(["702.26f", "611.2a"]; assert_eq!(t.g.controller(dog), 0));
}

#[test]
fn r702_26n_what_phased_out_under_a_player_who_left_phases_in_after_their_next_turn_would_have_begun() {
    let (mut t, dog) = taken_and_phased_out(3);
    t.to_no_combat(2, Part::PrecombatMain);
    // player 1 leaves the game in player 2's turn; their next turn would have begun after
    // player 0's, and player 2's untap step is the next after it
    t.expect_losses(&[1]);
    t.g.act(1, Move::Concede).unwrap();
    cite!(["702.26n", "800.4a"]; assert!(phased_out(&t, dog)));
    t.to_no_combat(0, Part::PrecombatMain);
    cite!(["702.26n"]; assert!(phased_out(&t, dog)));
    t.to_no_combat(2, Part::PrecombatMain);
    cite!(["702.26n"]; assert!(t.g.battlefield.contains(&dog) && t.g.controller(dog) == 0));
}

#[test]
fn r702_26k_a_phased_out_permanent_leaves_the_game_with_its_owner() {
    let mut t = Board::new(3).fill(6).battlefield(2, &["Teferi's Drake"]).build();
    let drake = t.find_in_battlefield("Teferi's Drake");
    t.to_no_combat(2, Part::PrecombatMain);
    cite!(["702.26a"]; assert!(phased_out(&t, drake)));
    t.to_no_combat(0, Part::PrecombatMain);
    t.expect_losses(&[2]);
    t.g.act(2, Move::Concede).unwrap();
    cite!(["702.26k", "800.4a"]; assert!(t.g.obj(drake).zone == Zone::Gone && t.g.phased_out.is_empty()));
    // it leaves without being exiled
    cite!(["702.26k"]; assert!(!t.g.log.iter().any(|e| matches!(e, game::Event::Exiled { .. }))));
}
