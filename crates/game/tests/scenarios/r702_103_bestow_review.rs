//! Review of the bestow cards that run with more than a pump and keywords (702.103): what an
//! Aura's "this creature or enchanted creature" and "this permanent" are while it is bestowed,
//! and while it is a creature.
use crate::support::*;
use game::announcement::CastCost;
use game::legal::Asked;

const BEARS: &str = "Grizzly Bears";

/// The row of the card's bestow ability: the alternative cost it is cast for (702.103a).
fn bestow_row(g: &Game, card: ObjId) -> usize {
    g.def(card).unwrap().abilities().iter().position(|a| matches!(a, ir::Ability::Keywords(items)
        if items.iter().any(|i| i.keyword.name() == "Bestow"))).expect("a bestow ability")
}

/// Cast this card from player 0's hand bestowed on `on`, paying `mana`, and let it resolve.
fn bestow(t: &mut T, name: &str, on: ObjId, mana: &str) -> ObjId {
    let card = t.find_in(0, Zone::Hand, name);
    let row = bestow_row(&t.g, card);
    let cast = Move::CastFor(CastCost::Alternative(row), Box::new(Move::Cast { card, targets: vec![vec![Target::Object(on)]] }));
    t.g.players[0].pool = mana_pool(mana);
    cite!(["702.103a", "601.2"]; ok: t.g.act(0, Move::Paying(mana_pool(mana), Box::new(cast))));
    t.resolve();
    let aura = t.find_in_battlefield(name);
    cite!(["702.103b", "303.4a"]; assert_eq!(t.g.attached_to(aura), Some(Target::Object(on))));
    aura
}

/// From player 0's main phase, attack player 1 with these, unblocked, answering each trigger's
/// target with `target`, up to the postcombat main phase with an empty stack.
fn attack(t: &mut T, attackers: &[ObjId], target: Option<Target>) {
    for _ in 0..200 {
        if t.g.part == Part::PostcombatMain && t.g.stack.is_empty() { return; }
        match t.g.decision.clone() {
            Some(Decision::Attackers { player }) => t.g.act(player, Move::Attack(attackers.iter().map(|a| (*a, Target::Player(1))).collect())).unwrap(),
            Some(Decision::Blockers { player }) => t.g.act(player, Move::Block(vec![])).unwrap(),
            _ => match t.asked() {
                Some((q, Asked::Targets { .. })) => t.g.act(q, Move::ChooseTargets(vec![vec![target.expect("a target")]])).unwrap(),
                Some((q, Asked::Order(abilities))) => t.g.act(q, Move::Order((0..abilities.len()).collect())).unwrap(),
                _ => t.g.act(t.g.decision_maker(t.g.priority), Move::Pass).unwrap(),
            },
        }
    }
    panic!("combat never ended");
}

#[test]
fn spiteful_returned_triggers_on_the_attack_of_itself_and_of_the_creature_it_enchants() {
    // "Whenever this creature or enchanted creature attacks, defending player loses 2 life."
    for bestowed in [false, true] {
        let board = Board::new(2).fill(5).battlefield(0, &[BEARS]);
        let mut t = if bestowed { board.hand(0, &["Spiteful Returned"]) } else { board.battlefield(0, &["Spiteful Returned"]) }.build();
        let bears = t.find_in_battlefield(BEARS);
        let attacker = if bestowed { bestow(&mut t, "Spiteful Returned", bears, "CCCB"); bears }
            else { t.find_in_battlefield("Spiteful Returned") };
        attack(&mut t, &[attacker], None);
        // the Bears are 3/3 enchanted; Spiteful Returned a 1/1
        cite!(["702.103b", "508.1m", "603.2"]; case: bestowed; assert_eq!(t.g.players[1].life, if bestowed { 20 - 2 - 3 } else { 20 - 2 - 1 }));
    }
}

#[test]
fn heliods_emissary_bestowed_taps_a_creature_as_the_enchanted_creature_attacks() {
    // "Whenever this creature or enchanted creature attacks, tap target creature an opponent controls."
    let mut t = Board::new(2).fill(5).hand(0, &["Heliod's Emissary"]).battlefield(0, &[BEARS]).battlefield(1, &["Hill Giant"]).build();
    let bears = t.find_in(0, Zone::Battlefield, BEARS);
    let giant = t.find_in(1, Zone::Battlefield, "Hill Giant");
    bestow(&mut t, "Heliod's Emissary", bears, "CCCCCCW");
    attack(&mut t, &[bears], Some(Target::Object(giant)));
    cite!(["702.103b", "508.1m"]; assert!(t.g.obj(giant).tapped));
    t.expect().rules(["702.103b", "611.3a"]).life(1, 20 - 5).check();
}

#[test]
fn thassas_emissary_bestowed_draws_as_the_enchanted_creature_deals_combat_damage() {
    // "Whenever this creature or enchanted creature deals combat damage to a player, draw a card."
    let mut t = Board::new(2).fill(5).hand(0, &["Thassa's Emissary"]).battlefield(0, &[BEARS]).build();
    let bears = t.find_in_battlefield(BEARS);
    bestow(&mut t, "Thassa's Emissary", bears, "CCCCCU");
    let hand = t.g.players[0].hand.len();
    attack(&mut t, &[bears], None);
    cite!(["702.103b", "510.3a", "603.2"]; assert_eq!(t.g.players[0].hand.len(), hand + 1));
    t.expect().rules(["611.3a"]).life(1, 20 - 5).check();
}

#[test]
fn kestia_draws_as_the_creature_it_enchants_attacks() {
    // "Whenever an enchanted creature or enchantment creature you control attacks, draw a card."
    let mut t = Board::new(2).fill(5).hand(0, &["Kestia, the Cultivator"]).battlefield(0, &[BEARS]).build();
    let bears = t.find_in_battlefield(BEARS);
    bestow(&mut t, "Kestia, the Cultivator", bears, "CCCGWU");
    let hand = t.g.players[0].hand.len();
    attack(&mut t, &[bears], None);
    cite!(["702.103b", "508.1m"]; assert_eq!(t.g.players[0].hand.len(), hand + 1));
    t.expect().rules(["611.3a"]).life(1, 20 - 6).check();
}

#[test]
fn glyph_elemental_bestowed_puts_its_landfall_counters_on_itself_and_pumps_the_creature() {
    // "Whenever a land you control enters, put a +1/+1 counter on this permanent. Enchanted
    // creature gets +1/+1 for each +1/+1 counter on this Aura."
    let mut t = Board::new(2).fill(5).hand(0, &["Glyph Elemental", "Plains"]).battlefield(0, &[BEARS]).build();
    let bears = t.find_in_battlefield(BEARS);
    let glyph = bestow(&mut t, "Glyph Elemental", bears, "CW");
    let plains = t.find_in(0, Zone::Hand, "Plains");
    cite!(["305.1"]; ok: t.g.act(0, Move::PlayLand(plains)));
    t.resolve();
    cite!(["702.103b", "122.1a"]; assert_eq!(t.g.obj(glyph).counters.get("+1/+1").copied(), Some(1)));
    // +1/+1 for its one counter; the Aura has no power (208.3)
    cite!(["702.103b", "613.4c"]; assert_eq!((t.g.power(bears), t.g.power(glyph)), (Some(3), None)));
}

#[test]
fn tricksters_elk_bestowed_makes_the_creature_a_vanilla_3_3_elk() {
    // "Enchanted creature loses all abilities and is a green Elk creature with base power and toughness 3/3."
    let mut t = Board::new(2).fill(5).hand(0, &["Trickster's Elk"]).battlefield(0, &["Serra Angel"]).build();
    let angel = t.find_in_battlefield("Serra Angel");
    bestow(&mut t, "Trickster's Elk", angel, "CG");
    let seen = t.g.seen(angel, None);
    cite!(["613.1f", "613.4b"]; assert!(seen.keywords.is_empty() && seen.subtypes.iter().any(|s| s.name() == "Elk") && !seen.subtypes.iter().any(|s| s.name() == "Angel")));
    t.expect().rules(["613.4b"]).pt("Serra Angel", 3, 3).check();
}

#[test]
fn crystalline_nautilus_sacrifices_the_permanent_that_becomes_a_target() {
    // "When this permanent becomes the target of a spell or ability, sacrifice it. Enchanted
    // creature gets +4/+4 and has "When this creature becomes the target of a spell or ability,
    // sacrifice it.""
    for aura_targeted in [true, false] {
        let mut t = Board::new(2).fill(5).hand(0, &["Crystalline Nautilus"]).battlefield(0, &[BEARS])
            .hand(1, &["Disenchant", "Unsummon"]).build();
        let bears = t.find_in_battlefield(BEARS);
        let nautilus = bestow(&mut t, "Crystalline Nautilus", bears, "CCCUU");
        t.g.act(0, Move::Pass).unwrap();
        let (spell, target, mana) = if aura_targeted { ("Disenchant", nautilus, "{1}{W}") } else { ("Unsummon", bears, "{U}") };
        t.mana(1, mana).cast(1, spell, vec![Tgt::Object(target)], mana_pool(mana)).unwrap();
        t.resolve();
        let graveyard = &t.g.players[0].graveyard;
        let named = |name: &str| graveyard.iter().any(|o| t.g.def(*o).is_some_and(|d| d.name() == name));
        if aura_targeted {
            // the Aura is sacrificed before Disenchant resolves; the Bears stay, a 2/2 again
            cite!(["603.2", "702.103b"]; case: aura_targeted; assert!(named("Crystalline Nautilus") && !named(BEARS)));
            t.expect().rules(["603.2"]).pt(BEARS, 2, 2).check();
        } else {
            // the Bears are sacrificed; the Aura, unattached, is a creature again (702.103f)
            cite!(["603.2", "702.103f"]; case: aura_targeted; assert!(named(BEARS) && !named("Crystalline Nautilus")));
            t.expect().rules(["702.103f"]).pt("Crystalline Nautilus", 4, 4).check();
        }
    }
}

#[test]
fn herald_of_torment_bestowed_still_costs_its_controller_life_each_upkeep() {
    // "At the beginning of your upkeep, you lose 1 life."
    let mut t = Board::new(2).fill(5).hand(0, &["Herald of Torment"]).battlefield(0, &[BEARS]).build();
    let bears = t.find_in_battlefield(BEARS);
    bestow(&mut t, "Herald of Torment", bears, "CCCBB");
    t.to_no_combat(1, Part::PrecombatMain).to_no_combat(0, Part::PrecombatMain);
    cite!(["702.103b", "503.1a"]; assert_eq!(t.g.players[0].life, 19));
    t.expect().rules(["611.3a"]).pt(BEARS, 5, 5).check();
}
