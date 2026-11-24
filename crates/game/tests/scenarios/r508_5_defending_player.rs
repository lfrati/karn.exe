//! "Defending player" said by an ability that refers to an attacking creature (508.5): the
//! player that creature is attacking, determined for each creature (508.5a), whether the
//! ability is the creature's own or another permanent's.
use crate::support::*;

const BEARS: &str = "Grizzly Bears";

/// From player 0's main phase, attack with these, each at its player, with these blocks
/// (blocker, attacker) by the first defending player asked, up to the postcombat main phase
/// with an empty stack.
fn combat(t: &mut T, attacks: &[(ObjId, usize)], blocks: Vec<(ObjId, ObjId)>) {
    let mut blocks = Some(blocks);
    for _ in 0..300 {
        if t.g.part == Part::PostcombatMain && t.g.stack.is_empty() { return; }
        match t.g.decision.clone() {
            Some(Decision::Attackers { player }) => t.g.act(player, Move::Attack(attacks.iter().map(|(a, p)| (*a, Target::Player(*p))).collect())).unwrap(),
            Some(Decision::Blockers { player }) => t.g.act(player, Move::Block(blocks.take().unwrap_or_default())).unwrap(),
            _ => match t.asked() {
                Some((q, Asked::Order(abilities))) => t.g.act(q, Move::Order((0..abilities.len()).collect())).unwrap(),
                _ => t.g.act(t.g.decision_maker(t.g.priority), Move::Pass).unwrap(),
            },
        }
    }
    panic!("combat never ended");
}

#[test]
fn r508_5a_campaign_of_vengeance_drains_the_player_each_creature_attacks() {
    // "Whenever a creature you control attacks, defending player loses 1 life and you gain 1
    // life.": the enchantment is not attacking; each trigger's defending player is the one its
    // creature attacks
    let mut t = Board::new(3).battlefield(0, &["Campaign of Vengeance", BEARS, "Hill Giant", "Llanowar Elves"]).build();
    let (bears, giant, elves) = (t.find_in_battlefield(BEARS), t.find_in_battlefield("Hill Giant"), t.find_in_battlefield("Llanowar Elves"));
    combat(&mut t, &[(bears, 1), (giant, 2), (elves, 2)], vec![]);
    t.expect().rules(["508.5", "508.5a", "603.2"])
        .life(1, 20 - 1 - 2).life(2, 20 - 2 - 3 - 1).life(0, 20 + 3).check();
}

#[test]
fn r508_5_tormentors_helm_deals_its_damage_to_the_player_the_equipped_creature_attacks() {
    // "Whenever equipped creature becomes blocked, it deals 1 damage to defending player."
    let mut t = Board::new(2).battlefield(0, &["Tormentor's Helm", BEARS]).battlefield(1, &["Hill Giant"]).build();
    let (helm, bears, giant) = (t.find_in_battlefield("Tormentor's Helm"), t.find_in(0, Zone::Battlefield, BEARS), t.find_in(1, Zone::Battlefield, "Hill Giant"));
    assert!(t.g.attach(helm, Target::Object(bears)));
    combat(&mut t, &[(bears, 1)], vec![(giant, bears)]);
    t.expect().rules(["508.5", "509.3c"]).life(1, 19).life(0, 20).check();
}

#[test]
fn r508_5_leeching_sliver_drains_for_another_sliver_attacking() {
    // "Whenever a Sliver you control attacks, defending player loses 1 life."
    let mut t = Board::new(2).battlefield(0, &["Leeching Sliver", "Muscle Sliver"]).build();
    let muscle = t.find_in_battlefield("Muscle Sliver");
    // Muscle Sliver, a 1/1, gets +1/+1 from itself
    combat(&mut t, &[(muscle, 1)], vec![]);
    t.expect().rules(["508.5", "603.2"]).life(1, 20 - 1 - 2).check();
}

#[test]
fn r508_5_reapers_talisman_drains_the_player_the_lone_equipped_creature_attacks() {
    // "Whenever equipped creature attacks alone, defending player loses 2 life and you gain 2 life."
    let mut t = Board::new(2).battlefield(0, &["Reaper's Talisman", BEARS]).build();
    let (talisman, bears) = (t.find_in_battlefield("Reaper's Talisman"), t.find_in_battlefield(BEARS));
    assert!(t.g.attach(talisman, Target::Object(bears)));
    combat(&mut t, &[(bears, 1)], vec![]);
    t.expect().rules(["508.5", "506.5"]).life(1, 20 - 2 - 2).life(0, 22).check();
}
