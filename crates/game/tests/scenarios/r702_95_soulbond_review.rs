//! Review of the soulbond cards whose pair gives each of the two creatures a whole ability
//! (702.95b): the ability is the partner's own, its "this creature" that creature.
use crate::support;
use support::*;

const BEARS: &str = "Grizzly Bears";

fn enter(t: &mut T, name: &str, cost: &str, pool: &str) {
    t.mana(0, cost).cast(0, name, vec![], mana_pool(pool)).unwrap();
    t.pass_all();
}

fn pair(t: &mut T) {
    let boundary = t.settle();
    cite!(["702.95a"]; assert!(matches!(boundary, Boundary::Question(0, Asked::May)), "the controller may pair them: {boundary:?}"));
    t.answer(0, Answer::Yes).unwrap();
    t.resolve();
}

/// From player 0's main phase, attack player 1 with these and let the combat be fought with
/// these blocks, up to the postcombat main phase with an empty stack.
fn combat(t: &mut T, attackers: &[ObjId], blocks: Vec<(ObjId, ObjId)>) {
    let mut blocks = Some(blocks);
    for _ in 0..200 {
        if t.g.part == Part::PostcombatMain && t.g.stack.is_empty() { return; }
        match t.g.decision.clone() {
            Some(Decision::Attackers { player }) => t.g.act(player, Move::Attack(attackers.iter().map(|a| (*a, Target::Player(1))).collect())).unwrap(),
            Some(Decision::Blockers { player }) => t.g.act(player, Move::Block(blocks.take().unwrap_or_default())).unwrap(),
            Some(Decision::Choose { player, choice: Choice::OrderCards { cards, .. } }) => t.g.act(player, Move::Choose(Answer::Objects(cards))).unwrap(),
            _ => match t.asked() {
                Some((q, Asked::Order(abilities))) => t.g.act(q, Move::Order((0..abilities.len()).collect())).unwrap(),
                _ => t.g.act(t.g.decision_maker(t.g.priority), Move::Pass).unwrap(),
            },
        }
    }
    panic!("combat never ended");
}

#[test]
fn tandem_lookout_partner_draws_as_it_deals_damage_to_an_opponent() {
    // "each of those creatures has "Whenever this creature deals damage to an opponent, draw a card.""
    let mut t = Board::new(2).fill(5).hand(0, &["Tandem Lookout"]).battlefield(0, &[BEARS]).build();
    enter(&mut t, "Tandem Lookout", "{2}{U}", "CCU");
    pair(&mut t);
    let bears = t.find_in_battlefield(BEARS);
    let hand = t.g.players[0].hand.len();
    combat(&mut t, &[bears], vec![]);
    t.expect().rules(["702.95b", "510.2", "603.2"]).life(1, 18).check();
    cite!(["702.95b"]; assert_eq!(t.g.players[0].hand.len(), hand + 1, "the Bears' trigger drew a card"));
}

#[test]
fn imperious_mindbreaker_partner_mills_each_opponent_its_own_toughness() {
    // "each of those creatures has "Whenever this creature attacks, each opponent mills cards
    // equal to its toughness."": its is the attacking creature's
    let mut t = Board::new(2).fill(10).hand(0, &["Imperious Mindbreaker"]).battlefield(0, &["Hill Giant"]).build();
    enter(&mut t, "Imperious Mindbreaker", "{1}{U}{U}", "CUU");
    pair(&mut t);
    let giant = t.find_in_battlefield("Hill Giant");
    combat(&mut t, &[giant], vec![]);
    cite!(["702.95b", "508.1m"]; assert_eq!(t.g.players[1].graveyard.len(), 3, "the Giant's toughness, not the Mindbreaker's 4"));
}

#[test]
fn galvanic_alchemist_partner_untaps_itself() {
    // "each of those creatures has "{2}{U}: Untap this creature.""
    let mut t = Board::new(2).hand(0, &["Galvanic Alchemist"]).tapped(0, &[BEARS]).build();
    enter(&mut t, "Galvanic Alchemist", "{2}{U}", "CCU");
    pair(&mut t);
    let (bears, alchemist) = (t.find_in_battlefield(BEARS), t.find_in_battlefield("Galvanic Alchemist"));
    let untap = t.g.abilities(bears).iter().position(|a| matches!(&**a, ir::Ability::Activated { .. })).expect("the Bears have it");
    t.mana(0, "{2}{U}");
    cite!(["602.2"]; ok: t.g.act(0, paid("CCU", Move::Activate { source: bears, ability: untap, targets: vec![], paying: vec![] })));
    t.resolve();
    cite!(["702.95b"]; assert!(!t.g.obj(bears).tapped));
    // the Alchemist's own copy untaps the Alchemist
    t.g.set_tapped(alchemist, true);
    t.g.set_tapped(bears, true);
    let own = t.g.abilities(alchemist).iter().position(|a| matches!(&**a, ir::Ability::Activated { .. })).expect("the Alchemist has it");
    t.mana(0, "{2}{U}");
    cite!(["602.2"]; ok: t.g.act(0, paid("CCU", Move::Activate { source: alchemist, ability: own, targets: vec![], paying: vec![] })));
    t.resolve();
    cite!(["702.95b"]; assert!(!t.g.obj(alchemist).tapped && t.g.obj(bears).tapped));
}

#[test]
fn doom_weaver_draws_the_last_known_power_of_the_creature_that_died() {
    // "When this creature dies, draw cards equal to its power": its power as it last existed
    for weaver_dies in [false, true] {
        let mut t = Board::new(2).fill(8).hand(0, &["Doom Weaver", "Murder"]).battlefield(0, &[BEARS]).counters(BEARS, "+1/+1", 2).build();
        enter(&mut t, "Doom Weaver", "{4}{B}{B}", "CCCCBB");
        pair(&mut t);
        let victim = t.find_in_battlefield(if weaver_dies { "Doom Weaver" } else { BEARS });
        let hand = t.g.players[0].hand.len() - 1;
        t.mana(0, "{1}{B}{B}").cast(0, "Murder", vec![Tgt::Object(victim)], mana_pool("CBB")).unwrap();
        t.resolve();
        let drawn = if weaver_dies { 1 } else { 4 };
        cite!(["603.10a", "608.2h"]; case: weaver_dies; assert_eq!(t.g.players[0].hand.len(), hand + drawn));
    }
}

#[test]
fn diregraf_escort_gives_both_protection_from_zombies() {
    // "both creatures have protection from Zombies"; ruling: it can be paired with a Zombie
    for partner in [BEARS, "Walking Corpse"] {
        let mut t = Board::new(2).hand(0, &["Diregraf Escort"]).battlefield(0, &[partner]).battlefield(1, &["Walking Corpse"]).build();
        enter(&mut t, "Diregraf Escort", "{G}", "G");
        pair(&mut t);
        let attacker = t.find_in(0, Zone::Battlefield, partner);
        let zombie = t.find_in(1, Zone::Battlefield, "Walking Corpse");
        // the defender is asked even with no legal block, so that the Zombie's is refused
        t.g.set_ask_forced(true);
        let mut blocked = None;
        for _ in 0..50 {
            match t.g.decision.clone() {
                Some(Decision::Attackers { player: 0 }) => t.g.act(0, Move::Attack(vec![(attacker, Target::Player(1))])).unwrap(),
                Some(Decision::Blockers { player: 1 }) => { blocked = Some(t.g.act(1, Move::Block(vec![(zombie, attacker)]))); break; }
                _ => t.g.act(t.g.decision_maker(t.g.priority), Move::Pass).unwrap(),
            }
        }
        cite!(["702.16e", "702.95b"]; case: partner; refused("Walking Corpse can't block"): blocked.expect("the defender is asked"));
    }
}

#[test]
fn deadeye_navigator_blinking_itself_pairs_again_as_it_returns() {
    let mut t = Board::new(2).hand(0, &["Deadeye Navigator"]).battlefield(0, &[BEARS]).build();
    enter(&mut t, "Deadeye Navigator", "{4}{U}{U}", "CCCCUU");
    pair(&mut t);
    let navigator = t.find_in_battlefield("Deadeye Navigator");
    let blink = t.g.abilities(navigator).iter().position(|a| matches!(&**a, ir::Ability::Activated { .. })).expect("it has the ability");
    t.mana(0, "{1}{U}");
    cite!(["602.2"]; ok: t.g.act(0, paid("CU", Move::Activate { source: navigator, ability: blink, targets: vec![], paying: vec![] })));
    t.pass_all();
    let returned = t.find_in_battlefield("Deadeye Navigator");
    let bears = t.find_in_battlefield(BEARS);
    cite!(["400.7", "702.95e"]; assert!(returned != navigator && !t.g.abilities(bears).iter().any(|a| matches!(&**a, ir::Ability::Activated { .. }))));
    cite!(["702.95a"]; assert_eq!(t.g.controller(returned), 0));
    pair(&mut t);
    cite!(["702.95a"]; assert!(t.g.abilities(bears).iter().any(|a| matches!(&**a, ir::Ability::Activated { .. }))));
}

#[test]
fn stonewright_partner_pumps_itself() {
    // "each of those creatures has "{R}: This creature gets +1/+0 until end of turn.""
    let mut t = Board::new(2).hand(0, &["Stonewright"]).battlefield(0, &[BEARS]).build();
    enter(&mut t, "Stonewright", "{R}", "R");
    pair(&mut t);
    let bears = t.find_in_battlefield(BEARS);
    let pump = t.g.abilities(bears).iter().position(|a| matches!(&**a, ir::Ability::Activated { .. })).expect("the Bears have it");
    t.mana(0, "{R}");
    cite!(["602.2"]; ok: t.g.act(0, paid("R", Move::Activate { source: bears, ability: pump, targets: vec![], paying: vec![] })));
    t.resolve();
    t.expect().rules(["702.95b", "613.4c"]).pt(BEARS, 3, 2).pt("Stonewright", 1, 1).check();
}

#[test]
fn thundering_mightmare_partner_gets_the_counter() {
    // "each of those creatures has "Whenever an opponent casts a spell, put a +1/+1 counter on
    // this creature."": each on itself
    let mut t = Board::new(2).hand(0, &["Thundering Mightmare"]).battlefield(0, &[BEARS]).hand(1, &["Lightning Bolt"]).build();
    enter(&mut t, "Thundering Mightmare", "{4}{G}", "CCCCG");
    pair(&mut t);
    t.g.act(0, Move::Pass).unwrap();
    t.mana(1, "{R}").cast(1, "Lightning Bolt", vec![Tgt::Player(0)], mana_pool("R")).unwrap();
    for _ in 0..20 {
        if t.g.stack.is_empty() { break; }
        match t.asked() {
            Some((q, Asked::Order(abilities))) => t.g.act(q, Move::Order((0..abilities.len()).collect())).unwrap(),
            _ => t.g.act(t.g.decision_maker(t.g.priority), Move::Pass).unwrap(),
        }
    }
    t.expect().rules(["702.95b", "603.2"]).counters(BEARS, "+1/+1", 1).counters("Thundering Mightmare", "+1/+1", 1).life(0, 17).check();
}

#[test]
fn stern_mentor_partner_taps_to_mill_a_target_player() {
    // "each of those creatures has "{T}: Target player mills two cards.""
    let mut t = Board::new(2).fill(5).hand(0, &["Stern Mentor"]).battlefield(0, &[BEARS]).build();
    enter(&mut t, "Stern Mentor", "{3}{U}", "CCCU");
    pair(&mut t);
    let bears = t.find_in_battlefield(BEARS);
    let mill = t.g.abilities(bears).iter().position(|a| matches!(&**a, ir::Ability::Activated { .. })).expect("the Bears have it");
    cite!(["602.2"]; ok: t.g.act(0, Move::Activate { source: bears, ability: mill, targets: vec![vec![Target::Player(1)]], paying: vec![] }));
    while let Boundary::Question(p, _) = t.settle() {
        let Some(Decision::Choose { choice: Choice::OrderCards { cards, .. }, .. }) = t.g.decision.clone() else { panic!("{:?}", t.g.decision) };
        t.answer(p, Answer::Objects(cards)).unwrap();
    }
    cite!(["702.95b", "701.17a"]; assert!(t.g.obj(bears).tapped && t.g.players[1].graveyard.len() == 2));
}
