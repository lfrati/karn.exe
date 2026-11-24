//! Review of "a statement and its rules" (604.1): Auras and Equipment whose one static ability
//! both changes the creature and says a rule for it. Each test checks a kind of rule the
//! mechanism's own tests don't: a blocking restriction, an extra block, not untapping, can't be
//! blocked with shroud, can't attack or block, a must-attack, and the conditional ones (during
//! your turn, as long as it is a color). Every card is read from its Oracle text.
use crate::support;
use game::state::Decision;
use ir::Ability;
use support::*;

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Treetop Bracers", "Iona's Blessing", "Vulshok Gauntlets", "Whispersilk Cloak", "Protective Bubble", "Cagemail", "Bilbo's Ring",
        "Steel of the Godhead", "Infiltrator's Magemark", "Holy Strength", "Vorrac Battlehorns", "Rope", "Saddle of the Cavalier", "Spectral Shield",
        "Copper Carapace", "Tormentor's Trident", "Grizzly Bears", "Savannah Lions", "Hill Giant", "Craw Wurm", "Giant Spider", "Wind Drake",
        "Wall of Stone", "Prodigal Sorcerer", "Merfolk of the Pearl Trident", "Azorius Guildmage", "Shock", "Giant Growth"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

/// Pass until this decision is asked.
fn to(t: &mut T, step: fn(&Decision) -> bool) {
    for _ in 0..40 {
        if t.g.decision.as_ref().is_some_and(step) { return; }
        let p = t.g.decision_maker(t.g.priority);
        t.g.act(p, Move::Pass).unwrap();
    }
    panic!("not reached: {:?}", t.g.decision);
}
fn attackers(d: &Decision) -> bool { matches!(d, Decision::Attackers { .. }) }
fn blockers(d: &Decision) -> bool { matches!(d, Decision::Blockers { .. }) }

/// Pass to the postcombat main phase, declaring no blocks and dividing nothing.
fn through_combat(t: &mut T) {
    for _ in 0..40 {
        if t.g.part == Part::PostcombatMain { return; }
        match t.g.decision.clone() {
            Some(Decision::Blockers { player }) => t.g.act(player, Move::Block(vec![])).unwrap(),
            _ => { let p = t.g.decision_maker(t.g.priority); t.g.act(p, Move::Pass).unwrap() }
        }
    }
    panic!("combat did not end: {:?}", t.g.decision);
}

/// Its last activated ability: equip for a creature you control.
fn equip(t: &mut T, equipment: ObjId, creature: ObjId, units: &str) {
    let ability = t.g.abilities(equipment).iter().rposition(|a| matches!(&**a, Ability::Activated { .. })).expect("equip");
    t.mana(0, &"{1}".repeat(units.len()));
    t.g.act(0, paid(units, Move::Activate { source: equipment, ability, targets: vec![vec![Target::Object(creature)]], paying: vec![] })).unwrap();
    t.resolve();
}

/// Player 0 casts this Aura on `creature`.
fn enchant(t: &mut T, aura: &str, cost: &str, pool: &str, creature: ObjId) {
    let card = t.g.players[0].hand.iter().copied().find(|o| t.g.obj(*o).chars.name == aura).expect("the Aura in hand");
    t.mana(0, cost);
    t.g.act(0, Move::Paying(mana_pool(pool), Box::new(Move::Cast { card, targets: vec![vec![Target::Object(creature)]] }))).unwrap();
    t.resolve();
}

fn pt(t: &T, o: ObjId) -> (Option<i32>, Option<i32>) { (t.g.power(o), t.g.toughness(o)) }

/// Prodigal Sorcerer's "{T}: This creature deals 1 damage to any target.", aimed at `target`.
fn ping(t: &T, pro: ObjId, target: ObjId) -> Move {
    let ability = t.g.abilities(pro).iter().position(|a| matches!(&**a, Ability::Activated { .. })).expect("its ability");
    Move::Activate { source: pro, ability, targets: vec![vec![Target::Object(target)]], paying: vec![] }
}

// Rules: 604.1, 509.1b, 702.9b, 702.17b
// Given: Treetop Bracers ("Enchanted creature gets +1/+1 and can't be blocked except by
// creatures with flying.") on player 0's Grizzly Bears; player 1 has Grizzly Bears, Giant
// Spider (reach) and Wind Drake (flying).
// When: the Bears attack and player 1 declares blockers.
// Expect: the Bears are 3/3; neither the Bears nor the Spider may block them, as reach isn't
// flying (the card's ruling); the Drake may.
#[test]
fn r604_1_treetop_bracers_lets_only_a_flier_block() {
    let mut t = Board::new(2).fill(6).named(0, Zone::Battlefield, "bears", "Grizzly Bears").hand(0, &["Treetop Bracers"])
        .named(1, Zone::Battlefield, "b2", "Grizzly Bears").named(1, Zone::Battlefield, "spider", "Giant Spider").named(1, Zone::Battlefield, "drake", "Wind Drake").build();
    let (bears, b2, spider, drake) = (t.id("bears"), t.id("b2"), t.id("spider"), t.id("drake"));
    enchant(&mut t, "Treetop Bracers", "{1}{G}", "CG", bears);
    cite!(["613.4c"]; assert_eq!(pt(&t, bears), (Some(3), Some(3))));
    to(&mut t, attackers);
    t.g.act(0, Move::Attack(vec![(bears, Target::Player(1))])).unwrap();
    to(&mut t, blockers);
    cite!(["509.1b", "604.1"]; refused("Treetop Bracers says it can't"): t.g.clone().act(1, Move::Block(vec![(b2, bears)])));
    cite!(["509.1b", "702.17b"]; refused("Treetop Bracers says it can't"): t.g.clone().act(1, Move::Block(vec![(spider, bears)])));
    cite!(["509.1b", "702.9b"]; ok: t.g.act(1, Move::Block(vec![(drake, bears)])));
}

// Rules: 604.1, 509.1a, 702.20b
// Given: Iona's Blessing ("Enchanted creature gets +2/+2, has vigilance, and can block an
// additional creature each combat.") on player 0's Grizzly Bears, once or twice.
// When: the Bears attack; then player 1 attacks with Grizzly Bears, Savannah Lions and Hill
// Giant.
// Expect: with one Blessing the Bears are 4/4, attack without tapping, and may block two of
// the attackers but not three; with two they are 6/6 and may block all three (the card's
// ruling: it is cumulative).
#[test]
fn r604_1_iona_s_blessing_lets_its_creature_block_one_more_each_time() {
    for blessings in [1, 2] {
        let mut t = Board::new(2).fill(8).named(0, Zone::Battlefield, "bears", "Grizzly Bears").hand(0, &["Iona's Blessing", "Iona's Blessing"])
            .named(1, Zone::Battlefield, "a1", "Grizzly Bears").named(1, Zone::Battlefield, "a2", "Savannah Lions").named(1, Zone::Battlefield, "a3", "Hill Giant").build();
        let (bears, a1, a2, a3) = (t.id("bears"), t.id("a1"), t.id("a2"), t.id("a3"));
        for _ in 0..blessings { enchant(&mut t, "Iona's Blessing", "{3}{W}", "CCCW", bears); }
        let grown = 2 + 2 * blessings;
        cite!(["613.4c"]; case: format!("{blessings} Blessings"); assert_eq!(pt(&t, bears), (Some(grown), Some(grown))));
        to(&mut t, attackers);
        t.g.act(0, Move::Attack(vec![(bears, Target::Player(1))])).unwrap();
        cite!(["702.20b"]; assert!(!t.g.obj(bears).tapped));
        through_combat(&mut t);
        t.to_no_combat(1, Part::PrecombatMain);
        to(&mut t, attackers);
        t.g.act(1, Move::Attack(vec![(a1, Target::Player(0)), (a2, Target::Player(0)), (a3, Target::Player(0))])).unwrap();
        to(&mut t, blockers);
        cite!(["509.1a", "604.1"]; ok: t.g.clone().act(0, Move::Block(vec![(bears, a1), (bears, a2)])));
        let three = t.g.clone().act(0, Move::Block(vec![(bears, a1), (bears, a2), (bears, a3)]));
        if blessings == 1 {
            cite!(["509.1a"]; refused("cannot block that many attacking creatures"): three);
        } else {
            cite!(["509.1a"]; ok: three);
        }
    }
}

// Rules: 604.1, 502.3, 301.5
// Given: Vulshok Gauntlets ("Equipped creature gets +4/+2 and doesn't untap during its
// controller's untap step.") equipped to player 0's Grizzly Bears; Savannah Lions beside it.
// When: both attack; player 0's next turn comes; the Gauntlets move to the Lions, which
// attack; player 0's next turn comes.
// Expect: the Bears are 6/4 and deal 6; they stay tapped through the untap step while the
// Lions untap. Once the Gauntlets move, the Bears untap and the Lions stay tapped.
#[test]
fn r604_1_vulshok_gauntlets_keeps_only_the_equipped_creature_tapped() {
    let mut t = Board::new(2).fill(8).named(0, Zone::Battlefield, "bears", "Grizzly Bears").named(0, Zone::Battlefield, "g", "Vulshok Gauntlets")
        .named(0, Zone::Battlefield, "lions", "Savannah Lions").build();
    let (bears, gauntlets, lions) = (t.id("bears"), t.id("g"), t.id("lions"));
    equip(&mut t, gauntlets, bears, "CCC");
    cite!(["613.4c"]; assert_eq!(pt(&t, bears), (Some(6), Some(4))));
    to(&mut t, attackers);
    t.g.act(0, Move::Attack(vec![(bears, Target::Player(1)), (lions, Target::Player(1))])).unwrap();
    through_combat(&mut t);
    cite!(["510.2"]; assert_eq!(t.g.players[1].life, 12));
    t.to_no_combat(1, Part::PrecombatMain);
    t.to_no_combat(0, Part::PrecombatMain);
    cite!(["502.3", "604.1"]; assert!(t.g.obj(bears).tapped && !t.g.obj(lions).tapped));
    equip(&mut t, gauntlets, lions, "CCC");
    to(&mut t, attackers);
    t.g.act(0, Move::Attack(vec![(lions, Target::Player(1))])).unwrap();
    through_combat(&mut t);
    t.to_no_combat(1, Part::PrecombatMain);
    t.to_no_combat(0, Part::PrecombatMain);
    cite!(["502.3", "301.5"]; assert!(!t.g.obj(bears).tapped && t.g.obj(lions).tapped));
}

// Rules: 604.1, 509.1b, 702.18a
// Given: player 0's Grizzly Bears, equipped with Whispersilk Cloak ("Equipped creature can't be
// blocked and has shroud.") or enchanted with Protective Bubble (the same words), and
// Savannah Lions; player 1's Grizzly Bears, Prodigal Sorcerer and Shock.
// When: player 0 aims Giant Growth at the Bears; both attack; player 1 declares blockers, then
// aims Shock and the Sorcerer at the Bears.
// Expect: no one may target the Bears, not even their controller; the Bears can't be blocked
// and the Lions can; unblocked, both deal their damage.
#[test]
fn r604_1_whispersilk_cloak_and_protective_bubble_make_a_creature_unblockable_and_untargetable() {
    for card in ["Whispersilk Cloak", "Protective Bubble"] {
        let mut t = Board::new(2).fill(8).named(0, Zone::Battlefield, "bears", "Grizzly Bears").named(0, Zone::Battlefield, "lions", "Savannah Lions")
            .named(0, Zone::Battlefield, "cloak", "Whispersilk Cloak").hand(0, &["Protective Bubble", "Giant Growth"])
            .named(1, Zone::Battlefield, "b2", "Grizzly Bears").named(1, Zone::Battlefield, "pro", "Prodigal Sorcerer").hand(1, &["Shock"]).build();
        let (bears, lions, cloak, b2, pro) = (t.id("bears"), t.id("lions"), t.id("cloak"), t.id("b2"), t.id("pro"));
        if card == "Whispersilk Cloak" { equip(&mut t, cloak, bears, "CC") } else { enchant(&mut t, card, "{3}{U}", "CCCU", bears) }
        cite!(["702.18a"]; case: card; assert!(t.g.has(bears, "Shroud")));
        let growth = t.find_in(0, Zone::Hand, "Giant Growth");
        t.mana(0, "{G}");
        cite!(["702.18a"]; case: card; refused("it has shroud"): t.g.clone().act(0, paid("G", Move::Cast { card: growth, targets: vec![vec![Target::Object(bears)]] })));
        to(&mut t, attackers);
        t.g.act(0, Move::Attack(vec![(bears, Target::Player(1)), (lions, Target::Player(1))])).unwrap();
        to(&mut t, blockers);
        cite!(["509.1b", "604.1"]; case: card; refused("says it can't"): t.g.clone().act(1, Move::Block(vec![(b2, bears)])));
        cite!(["509.1b"]; case: card; ok: t.g.clone().act(1, Move::Block(vec![(b2, lions)])));
        t.g.act(1, Move::Block(vec![])).unwrap();
        t.g.act(0, Move::Pass).unwrap();
        let shock = t.find_in(1, Zone::Hand, "Shock");
        t.mana(1, "{R}");
        cite!(["702.18a"]; case: card; refused("it has shroud"): t.g.clone().act(1, paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Object(bears)]] })));
        cite!(["702.18a"]; case: card; refused("it has shroud"): t.g.clone().act(1, ping(&t, pro, bears)));
        through_combat(&mut t);
        cite!(["510.2"]; case: card; assert_eq!(t.g.players[1].life, 16));
    }
}

// Rules: 604.1, 508.1c, 509.1a
// Given: Cagemail ("Enchanted creature gets +2/+2 and can't attack.") on player 0's Grizzly
// Bears; Savannah Lions beside them; player 1's Grizzly Bears.
// When: player 0 declares attackers; then player 1 attacks with their Bears.
// Expect: the enchanted Bears are 4/4 and can't attack while the Lions can; they may block.
#[test]
fn r604_1_cagemail_keeps_its_creature_from_attacking_but_not_from_blocking() {
    let mut t = Board::new(2).fill(8).named(0, Zone::Battlefield, "bears", "Grizzly Bears").named(0, Zone::Battlefield, "lions", "Savannah Lions")
        .hand(0, &["Cagemail"]).named(1, Zone::Battlefield, "b2", "Grizzly Bears").build();
    let (bears, lions, b2) = (t.id("bears"), t.id("lions"), t.id("b2"));
    enchant(&mut t, "Cagemail", "{1}{W}", "CW", bears);
    cite!(["613.4c"]; assert_eq!(pt(&t, bears), (Some(4), Some(4))));
    to(&mut t, attackers);
    cite!(["508.1c", "604.1"]; refused("can't attack"): t.g.clone().act(0, Move::Attack(vec![(bears, Target::Player(1))])));
    cite!(["508.1c"]; ok: t.g.clone().act(0, Move::Attack(vec![(lions, Target::Player(1))])));
    t.g.act(0, Move::Attack(vec![])).unwrap();
    t.to_no_combat(1, Part::PrecombatMain);
    to(&mut t, attackers);
    t.g.act(1, Move::Attack(vec![(b2, Target::Player(0))])).unwrap();
    to(&mut t, blockers);
    cite!(["509.1a"]; ok: t.g.act(0, Move::Block(vec![(bears, b2)])));
}

// Rules: 604.1, 611.3a, 702.11b, 509.1b, 603.2
// Given: Bilbo's Ring ("During your turn, equipped creature has hexproof and can't be blocked.
// / Whenever equipped creature attacks alone, you draw a card and you lose 1 life.") on player
// 0's Grizzly Bears; Savannah Lions beside them; player 1's Grizzly Bears, Prodigal Sorcerer
// and Hill Giant.
// When: on player 0's turn, both attack and player 1 declares blockers; on player 1's turn,
// the Sorcerer aims at the Bears and the Giant attacks.
// Expect: on player 0's turn the Bears have hexproof and can't be blocked, the Lions can; they
// didn't attack alone, so nothing is drawn. On player 1's turn the Bears have no hexproof, may
// be targeted and may block.
#[test]
fn r604_1_bilbo_s_ring_hides_and_frees_its_creature_only_during_its_controller_s_turn() {
    let mut t = Board::new(2).fill(8).named(0, Zone::Battlefield, "bears", "Grizzly Bears").named(0, Zone::Battlefield, "lions", "Savannah Lions")
        .named(0, Zone::Battlefield, "ring", "Bilbo's Ring").named(1, Zone::Battlefield, "b2", "Grizzly Bears")
        .named(1, Zone::Battlefield, "pro", "Prodigal Sorcerer").named(1, Zone::Battlefield, "giant", "Hill Giant").build();
    let (bears, lions, ring, b2, pro, giant) = (t.id("bears"), t.id("lions"), t.id("ring"), t.id("b2"), t.id("pro"), t.id("giant"));
    equip(&mut t, ring, bears, "CCCC");
    cite!(["611.3a", "702.11a"]; assert!(t.g.has(bears, "Hexproof")));
    t.g.act(0, Move::Pass).unwrap();
    cite!(["702.11b"]; refused("it has hexproof"): t.g.clone().act(1, ping(&t, pro, bears)));
    let hand = t.g.players[0].hand.len();
    to(&mut t, attackers);
    t.g.act(0, Move::Attack(vec![(bears, Target::Player(1)), (lions, Target::Player(1))])).unwrap();
    cite!(["603.2"]; assert!(t.g.stack.is_empty(), "it didn't attack alone"));
    to(&mut t, blockers);
    cite!(["509.1b", "604.1"]; refused("Bilbo's Ring says it can't"): t.g.clone().act(1, Move::Block(vec![(b2, bears)])));
    cite!(["509.1b"]; ok: t.g.clone().act(1, Move::Block(vec![(b2, lions)])));
    through_combat(&mut t);
    cite!(["510.2"]; assert_eq!((t.g.players[1].life, t.g.players[0].hand.len()), (16, hand)));
    t.to_no_combat(1, Part::PrecombatMain);
    cite!(["611.3a"]; assert!(!t.g.has(bears, "Hexproof")));
    cite!(["611.3a", "115.1"]; ok: t.g.clone().act(1, ping(&t, pro, bears)));
    // the Bears attacked last turn; they untap only in player 0's untap step
    t.g.set_tapped(bears, false);
    to(&mut t, attackers);
    t.g.act(1, Move::Attack(vec![(giant, Target::Player(0))])).unwrap();
    to(&mut t, blockers);
    cite!(["509.1a"]; ok: t.g.act(0, Move::Block(vec![(bears, giant)])));
}

// Rules: 604.1, 611.3a, 105.2, 702.15a
// Given: Steel of the Godhead ("As long as enchanted creature is white, it gets +1/+1 and has
// lifelink. / As long as enchanted creature is blue, it gets +1/+1 and can't be blocked.") on
// a blue, a white, a green and a white-and-blue creature of player 0; player 1's Grizzly Bears.
// When: the creature attacks beside Hill Giant, and player 1 declares blockers.
// Expect: blue Merfolk of the Pearl Trident is 2/2 and can't be blocked; white Savannah Lions
// is 3/2 with lifelink and can be; green Grizzly Bears is 2/2, neither; white and blue Azorius
// Guildmage gets both (the card's ruling): 4/4, lifelink, can't be blocked.
#[test]
fn r604_1_steel_of_the_godhead_reads_each_color_of_its_creature() {
    for (host, grown, lifelink, unblockable) in [
        ("Merfolk of the Pearl Trident", (2, 2), false, true),
        ("Savannah Lions", (3, 2), true, false),
        ("Grizzly Bears", (2, 2), false, false),
        ("Azorius Guildmage", (4, 4), true, true),
    ] {
        let mut t = Board::new(2).fill(8).named(0, Zone::Battlefield, "c", host).named(0, Zone::Battlefield, "giant", "Hill Giant")
            .hand(0, &["Steel of the Godhead"]).named(1, Zone::Battlefield, "b2", "Grizzly Bears").build();
        let (c, giant, b2) = (t.id("c"), t.id("giant"), t.id("b2"));
        enchant(&mut t, "Steel of the Godhead", "{2}{W}", "CCW", c);
        cite!(["613.4c", "611.3a"]; case: host; assert_eq!(pt(&t, c), (Some(grown.0), Some(grown.1))));
        cite!(["611.3a", "613.1f"]; case: host; assert_eq!(t.g.has(c, "Lifelink"), lifelink));
        to(&mut t, attackers);
        t.g.act(0, Move::Attack(vec![(c, Target::Player(1)), (giant, Target::Player(1))])).unwrap();
        to(&mut t, blockers);
        let block = t.g.clone().act(1, Move::Block(vec![(b2, c)]));
        if unblockable {
            cite!(["509.1b", "604.1"]; case: host; refused("Steel of the Godhead says it can't"): block);
        } else {
            cite!(["509.1b"]; case: host; ok: block);
        }
    }
}

// Rules: 604.1, 613.4c, 509.1b, 702.3b
// Given: Infiltrator's Magemark ("Creatures you control that are enchanted get +1/+1 and can't
// be blocked except by creatures with defender.") on player 0's Grizzly Bears; Holy Strength
// ("Enchanted creature gets +1/+2.") on their Savannah Lions; their Hill Giant unenchanted.
// Player 1 has Grizzly Bears and Wall of Stone (defender).
// When: all three attack and player 1 declares blockers.
// Expect: each enchanted creature gets +1/+1, whichever Aura enchants it: Bears 3/3, Lions
// 4/4; the Giant stays 3/3. Only the Wall may block an enchanted one; the Bears may block the
// Giant.
#[test]
fn r604_1_infiltrator_s_magemark_reads_every_enchanted_creature_of_its_controller() {
    let mut t = Board::new(2).fill(8).named(0, Zone::Battlefield, "a", "Grizzly Bears").named(0, Zone::Battlefield, "b", "Savannah Lions")
        .named(0, Zone::Battlefield, "c", "Hill Giant").hand(0, &["Infiltrator's Magemark", "Holy Strength"])
        .named(1, Zone::Battlefield, "b2", "Grizzly Bears").named(1, Zone::Battlefield, "wall", "Wall of Stone").build();
    let (a, b, c, b2, wall) = (t.id("a"), t.id("b"), t.id("c"), t.id("b2"), t.id("wall"));
    enchant(&mut t, "Infiltrator's Magemark", "{2}{U}", "CCU", a);
    enchant(&mut t, "Holy Strength", "{W}", "W", b);
    cite!(["613.4c"]; assert_eq!((pt(&t, a), pt(&t, b), pt(&t, c)), ((Some(3), Some(3)), (Some(4), Some(4)), (Some(3), Some(3)))));
    to(&mut t, attackers);
    t.g.act(0, Move::Attack(vec![(a, Target::Player(1)), (b, Target::Player(1)), (c, Target::Player(1))])).unwrap();
    to(&mut t, blockers);
    cite!(["509.1b", "604.1"]; refused("Infiltrator's Magemark says it can't"): t.g.clone().act(1, Move::Block(vec![(b2, a)])));
    cite!(["509.1b", "604.1"]; refused("Infiltrator's Magemark says it can't"): t.g.clone().act(1, Move::Block(vec![(b2, b)])));
    cite!(["509.1b", "702.3b"]; ok: t.g.clone().act(1, Move::Block(vec![(wall, a)])));
    cite!(["509.1b"]; ok: t.g.clone().act(1, Move::Block(vec![(b2, c)])));
}

// Rules: 604.1, 509.1b, 509.1c
// Given: player 0's Hill Giant equipped with Vorrac Battlehorns ("Equipped creature has
// trample and can't be blocked by more than one creature.") or Rope ("Equipped creature gets
// +1/+2, has reach, and can't be blocked by more than one creature."); player 1's Grizzly Bears
// and Savannah Lions.
// When: the Giant attacks and player 1 declares blockers.
// Expect: it has trample, or is 4/5 with reach; one blocker is legal and two are not.
#[test]
fn r604_1_vorrac_battlehorns_and_rope_allow_one_blocker() {
    for (card, units) in [("Vorrac Battlehorns", "C"), ("Rope", "CCC")] {
        let mut t = Board::new(2).fill(8).named(0, Zone::Battlefield, "giant", "Hill Giant").named(0, Zone::Battlefield, "eq", card)
            .named(1, Zone::Battlefield, "b2", "Grizzly Bears").named(1, Zone::Battlefield, "lions", "Savannah Lions").build();
        let (giant, eq, b2, lions) = (t.id("giant"), t.id("eq"), t.id("b2"), t.id("lions"));
        equip(&mut t, eq, giant, units);
        if card == "Rope" {
            cite!(["613.4c", "613.1f"]; assert!(pt(&t, giant) == (Some(4), Some(5)) && t.g.has(giant, "Reach")));
        } else {
            cite!(["613.1f"]; assert!(t.g.has(giant, "Trample")));
        }
        to(&mut t, attackers);
        t.g.act(0, Move::Attack(vec![(giant, Target::Player(1))])).unwrap();
        to(&mut t, blockers);
        cite!(["509.1b"]; case: card; ok: t.g.clone().act(1, Move::Block(vec![(b2, giant)])));
        cite!(["509.1c", "604.1"]; case: card; refused("says it can't"): t.g.clone().act(1, Move::Block(vec![(b2, giant), (lions, giant)])));
    }
}

// Rules: 604.1, 509.1b
// Given: Saddle of the Cavalier ("Equipped creature gets +3/+3 and can't be blocked by
// creatures with power 3 or less.") on player 0's Grizzly Bears; player 1's Hill Giant (3/3)
// and Craw Wurm (6/4).
// When: the Bears attack and player 1 declares blockers.
// Expect: the Bears are 5/5; the Giant can't block them and the Wurm can.
#[test]
fn r604_1_saddle_of_the_cavalier_reads_the_blocker_s_power() {
    let mut t = Board::new(2).fill(8).named(0, Zone::Battlefield, "bears", "Grizzly Bears").named(0, Zone::Battlefield, "eq", "Saddle of the Cavalier")
        .named(1, Zone::Battlefield, "giant", "Hill Giant").named(1, Zone::Battlefield, "wurm", "Craw Wurm").build();
    let (bears, eq, giant, wurm) = (t.id("bears"), t.id("eq"), t.id("giant"), t.id("wurm"));
    equip(&mut t, eq, bears, "CCC");
    cite!(["613.4c"]; assert_eq!(pt(&t, bears), (Some(5), Some(5))));
    to(&mut t, attackers);
    t.g.act(0, Move::Attack(vec![(bears, Target::Player(1))])).unwrap();
    to(&mut t, blockers);
    cite!(["509.1b", "604.1"]; refused("Saddle of the Cavalier says it can't"): t.g.clone().act(1, Move::Block(vec![(giant, bears)])));
    cite!(["509.1b"]; ok: t.g.act(1, Move::Block(vec![(wurm, bears)])));
}

// Rules: 604.1, 115.1
// Given: Spectral Shield ("Enchanted creature gets +0/+2 and can't be the target of spells.")
// on player 0's Grizzly Bears; player 1's Shock and Prodigal Sorcerer.
// When: on player 1's turn, Shock and the Sorcerer aim at the Bears.
// Expect: the Bears are 2/4; Shock can't target them, and the Sorcerer's ability can (the
// card's ruling).
#[test]
fn r604_1_spectral_shield_stops_spells_but_not_abilities() {
    let mut t = Board::new(2).fill(8).named(0, Zone::Battlefield, "bears", "Grizzly Bears").hand(0, &["Spectral Shield"])
        .hand(1, &["Shock"]).named(1, Zone::Battlefield, "pro", "Prodigal Sorcerer").build();
    let (bears, pro) = (t.id("bears"), t.id("pro"));
    enchant(&mut t, "Spectral Shield", "{1}{W}{U}", "CWU", bears);
    cite!(["613.4c"]; assert_eq!(pt(&t, bears), (Some(2), Some(4))));
    t.to_no_combat(1, Part::PrecombatMain);
    let shock = t.find_in(1, Zone::Hand, "Shock");
    t.mana(1, "{R}");
    cite!(["115.1", "604.1"]; refused("Spectral Shield says it can't be the target of that"): t.g.clone().act(1, paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Object(bears)]] })));
    cite!(["115.1"]; ok: t.g.act(1, ping(&t, pro, bears)));
}

// Rules: 604.1, 508.1d, 509.1a
// Given: Tormentor's Trident ("Equipped creature gets +3/+0 and attacks each combat if able.")
// on player 0's Grizzly Bears; Copper Carapace ("Equipped creature gets +2/+2 and can't
// block.") on player 1's Grizzly Bears, beside their Savannah Lions.
// When: player 0 declares attackers, then player 1 declares blockers.
// Expect: the Trident's Bears are 5/2 and declaring no attackers is refused; the Carapace's
// Bears are 4/4 and can't block, while the Lions can.
#[test]
fn r604_1_tormentor_s_trident_makes_its_creature_attack_and_copper_carapace_keeps_one_from_blocking() {
    let mut t = Board::new(2).fill(8).named(0, Zone::Battlefield, "bears", "Grizzly Bears").named(0, Zone::Battlefield, "trident", "Tormentor's Trident")
        .named(1, Zone::Battlefield, "b2", "Grizzly Bears").named(1, Zone::Battlefield, "carapace", "Copper Carapace").named(1, Zone::Battlefield, "lions", "Savannah Lions").build();
    let (bears, trident, b2, carapace, lions) = (t.id("bears"), t.id("trident"), t.id("b2"), t.id("carapace"), t.id("lions"));
    equip(&mut t, trident, bears, "CCC");
    // player 1's equip is sorcery-speed on their own turn; the Carapace is attached as it was set up
    cite!(["301.5"]; assert!(t.g.attach(carapace, Target::Object(b2))));
    cite!(["613.4c"]; assert_eq!((pt(&t, bears), pt(&t, b2)), ((Some(5), Some(2)), (Some(4), Some(4)))));
    to(&mut t, attackers);
    cite!(["508.1d", "604.1"]; refused("Grizzly Bears attacks if able (508.1d)"): t.g.clone().act(0, Move::Attack(vec![])));
    t.g.act(0, Move::Attack(vec![(bears, Target::Player(1))])).unwrap();
    to(&mut t, blockers);
    cite!(["509.1a", "604.1"]; refused("Copper Carapace says it can't"): t.g.clone().act(1, Move::Block(vec![(b2, bears)])));
    cite!(["509.1a"]; ok: t.g.act(1, Move::Block(vec![(lions, bears)])));
}
