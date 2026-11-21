//! What was dealt damage, dealt it, attacked or blocked this turn, as a description of an
//! object or player.
use crate::support::{cite, paid};
use crate::support::*;
use game::state::Decision;

// Rules: 120.1, 608.2i, 115.1
// Given: Fatal Blow ("Destroy target creature that was dealt damage this turn"); the
// opponent's Hill Giant and Grizzly Bears.
// When: Fatal Blow is cast at the Giant, then Shock hits the Giant and Fatal Blow is cast again.
// Expect: before Shock no target is legal; after it the Giant is, the Bears still is not.
// Why: the description looks back at the turn's damage, whatever its source.
#[test]
fn r120_1_a_creature_dealt_damage_this_turn_is_a_target() {
    let mut t = Board::new(2).battlefield(1, &["Hill Giant", "Grizzly Bears"]).hand(0, &["Shock", "Fatal Blow"]).build();
    let (giant, bears) = (t.find_in_battlefield("Hill Giant"), t.find_in_battlefield("Grizzly Bears"));
    t.mana(0, "{B}");
    cite!(["115.1"]; refused("an illegal target, Hill Giant: it doesn't match the target's description (115.1)"): t.g.clone().act(0, paid("B", Move::Cast { card: t.find_in(0, Zone::Hand, "Fatal Blow"), targets: vec![vec![Target::Object(giant)]] })));
    t.mana(0, "{R}").cast(0, "Shock", vec![Tgt::Object(giant)], crate::support::mana_pool("R")).unwrap();
    t.resolve();
    assert_eq!(t.g.obj(giant).damage, 2);
    cite!(["115.1"]; refused("an illegal target, Grizzly Bears: it doesn't match the target's description (115.1)"): t.g.clone().act(0, paid("B", Move::Cast { card: t.find_in(0, Zone::Hand, "Fatal Blow"), targets: vec![vec![Target::Object(bears)]] })));
    t.cast(0, "Fatal Blow", vec![Tgt::Object(giant)], crate::support::mana_pool("B")).unwrap();
    t.resolve();
    cite!(["608.2i"]; assert!(!t.g.battlefield.contains(&giant) && t.g.battlefield.contains(&bears)));
}

// Rules: 120.1, 510.2, 608.2i
// Given: Spear of Heliod ("Destroy target creature that dealt damage to you this turn").
// When: the Bears attack unblocked; the Hill Giant attacks and the Elves block it.
// Expect: the Spear may target the Bears, not the Hill Giant.
// Why: the Giant dealt damage this turn, but to the Elves, not to the Spear's controller.
#[test]
fn r120_1_a_creature_that_dealt_damage_to_you_this_turn() {
    let mut t = Board::new(2).battlefield(0, &["Grizzly Bears", "Hill Giant"]).battlefield(1, &["Spear of Heliod", "Llanowar Elves"]).build();
    let (bears, giant, elves) = (t.find_in_battlefield("Grizzly Bears"), t.find_in_battlefield("Hill Giant"), t.find_in_battlefield("Llanowar Elves"));
    let spear = t.find_in_battlefield("Spear of Heliod");
    while t.g.decision != Some(Decision::Attackers { player: 0 }) { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.g.act(0, Move::Attack(vec![(bears, Target::Player(1)), (giant, Target::Player(1))])).unwrap();
    while t.g.decision != Some(Decision::Blockers { player: 1 }) { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.g.act(1, Move::Block(vec![(elves, giant)])).unwrap();
    while t.g.players[1].life == 20 || t.g.priority != 1 { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    assert_eq!(t.g.players[1].life, 18);
    t.mana(1, "{1}{W}{W}");
    let activate = |o| paid("CWW", Move::Activate { source: spear, ability: 1, targets: vec![vec![Target::Object(o)]], paying: vec![] });
    cite!(["115.1"]; refused("an illegal target, Hill Giant: it doesn't match the target's description (115.1)"): t.g.clone().act(1, activate(giant)));
    t.g.act(1, activate(bears)).unwrap();
    t.resolve();
    cite!(["608.2i"]; assert!(!t.g.battlefield.contains(&bears) && t.g.battlefield.contains(&giant)));
}

// Rules: 120.1, 608.2i
// Given: Furious Spinesplitter ("a +1/+1 counter for each opponent who was dealt damage this
// turn") and three opponents.
// When: Shock hits one opponent; the end step trigger resolves.
// Expect: one counter.
// Why: players are described by the damage dealt to them this turn as objects are.
#[test]
fn r120_1_each_opponent_who_was_dealt_damage_this_turn() {
    let mut t = Board::new(4).battlefield(0, &["Furious Spinesplitter"]).hand(0, &["Shock"]).build();
    t.mana(0, "{R}").cast(0, "Shock", vec![Tgt::Player(2)], crate::support::mana_pool("R")).unwrap();
    t.resolve();
    t.to_no_combat(0, Part::End);
    t.resolve();
    t.expect().rules(["120.1", "608.2i"]).counters("Furious Spinesplitter", "+1/+1", 1).check();
}

// Rules: 120.1, 510.2, 608.2i
// Given: Moonshae Pixie ("draw cards equal to the number of opponents who were dealt combat
// damage this turn") and two opponents.
// When: Shock hits one opponent, the Bears hit the other in combat; the Pixie enters.
// Expect: one card drawn.
// Why: only combat damage (510.2) is combat damage; Shock's is not.
#[test]
fn r120_1_only_combat_damage_is_combat_damage_this_turn() {
    let mut t = Board::new(3).fill(5).battlefield(0, &["Grizzly Bears"]).hand(0, &["Shock", "Moonshae Pixie"]).build();
    let bears = t.find_in_battlefield("Grizzly Bears");
    t.mana(0, "{R}").cast(0, "Shock", vec![Tgt::Player(2)], crate::support::mana_pool("R")).unwrap();
    t.resolve();
    while t.g.decision != Some(Decision::Attackers { player: 0 }) { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.g.act(0, Move::Attack(vec![(bears, Target::Player(1))])).unwrap();
    t.to_no_combat(0, Part::PostcombatMain);
    assert_eq!((t.g.players[1].life, t.g.players[2].life), (18, 18));
    let hand = t.g.players[0].hand.len();
    t.mana(0, "{3}{U}").cast(0, "Moonshae Pixie", vec![], crate::support::mana_pool("CCCU")).unwrap();
    t.resolve();
    t.resolve();
    cite!(["510.2"]; assert_eq!(t.g.players[0].hand.len(), hand - 1 + 1, "one card drawn"));
}

// Rules: 508.1, 509.1, 608.2i
// Given: a Hill Giant and a tapped Grizzly Bears; the opponent's Wall of Stone and Craw Wurm.
// When: the Giant attacks and the Wall blocks; after combat Relentless Assault ("Untap all
// creatures that attacked this turn"), Sizzling Barrage ("target creature that blocked this
// turn") and Fire and Brimstone ("target player who attacked this turn") are cast.
// Expect: only the Giant untaps; the Barrage may target the Wall, not the Wurm; Fire and
// Brimstone may target the attacking player, not the opponent.
// Why: what attacked or blocked is what was declared so (508.1, 509.1), and the player who
// attacked is the one who declared attackers.
#[test]
fn r508_1_what_attacked_and_blocked_this_turn() {
    let mut t = Board::new(2).battlefield(0, &["Hill Giant"]).tapped(0, &["Grizzly Bears"]).battlefield(1, &["Wall of Stone", "Craw Wurm"])
        .hand(0, &["Relentless Assault", "Sizzling Barrage", "Fire and Brimstone"]).build();
    let (giant, bears, wall, wurm) = (t.find_in_battlefield("Hill Giant"), t.find_in_battlefield("Grizzly Bears"), t.find_in_battlefield("Wall of Stone"), t.find_in_battlefield("Craw Wurm"));
    while t.g.decision != Some(Decision::Attackers { player: 0 }) { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.g.act(0, Move::Attack(vec![(giant, Target::Player(1))])).unwrap();
    while t.g.decision != Some(Decision::Blockers { player: 1 }) { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.g.act(1, Move::Block(vec![(wall, giant)])).unwrap();
    t.to_no_combat(0, Part::PostcombatMain);
    assert!(t.g.obj(giant).tapped && t.g.obj(bears).tapped);
    t.mana(0, "{2}{R}{R}").cast(0, "Relentless Assault", vec![], crate::support::mana_pool("CCRR")).unwrap();
    t.resolve();
    cite!(["508.1"]; assert!(!t.g.obj(giant).tapped && t.g.obj(bears).tapped, "only what attacked untaps"));
    let card = |t: &T, name| t.find_in(0, Zone::Hand, name);
    t.mana(0, "{1}{R}");
    cite!(["115.1"]; refused("an illegal target, Craw Wurm: it doesn't match the target's description (115.1)"): t.g.clone().act(0, paid("CR", Move::Cast { card: card(&t, "Sizzling Barrage"), targets: vec![vec![Target::Object(wurm)]] })));
    t.cast(0, "Sizzling Barrage", vec![Tgt::Object(wall)], crate::support::mana_pool("CR")).unwrap();
    t.resolve();
    cite!(["509.1"]; assert_eq!(t.g.obj(wall).damage, 3 + 4, "the Hill Giant's and the Barrage's"));
    t.mana(0, "{3}{W}{W}");
    cite!(["115.1"]; refused("an illegal target, that player: that player isn't one the target describes (115.1)"): t.g.clone().act(0, paid("CCCWW", Move::Cast { card: card(&t, "Fire and Brimstone"), targets: vec![vec![Target::Player(1)]] })));
    t.cast(0, "Fire and Brimstone", vec![Tgt::Player(0)], crate::support::mana_pool("CCCWW")).unwrap();
    t.resolve();
    cite!(["508.1"]; assert_eq!(t.g.players[0].life, 12));
}

// Rules: 608.2i, 508.1, 601.2f
// Given: Rowdy Research ("costs {1} less to cast for each creature that attacked this turn",
// {6}{U}); the Grizzly Bears and the Hill Giant attack, and the Bears die blocked by a Craw Wurm.
// When: Rowdy Research is cast after combat.
// Expect: it costs {4}{U}.
// Why: a look back counts the creatures that attacked even if they are gone now; the effect
// takes no action on them (608.2i).
#[test]
fn r608_2i_a_creature_that_attacked_and_died_still_counts() {
    let mut t = Board::new(2).battlefield(0, &["Grizzly Bears", "Hill Giant"]).battlefield(1, &["Craw Wurm"]).hand(0, &["Rowdy Research"]).build();
    let (bears, giant, wurm) = (t.find_in_battlefield("Grizzly Bears"), t.find_in_battlefield("Hill Giant"), t.find_in_battlefield("Craw Wurm"));
    while t.g.decision != Some(Decision::Attackers { player: 0 }) { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.g.act(0, Move::Attack(vec![(bears, Target::Player(1)), (giant, Target::Player(1))])).unwrap();
    while t.g.decision != Some(Decision::Blockers { player: 1 }) { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.g.act(1, Move::Block(vec![(wurm, bears)])).unwrap();
    t.to_no_combat(0, Part::PostcombatMain);
    assert!(!t.g.battlefield.contains(&bears));
    let card = t.find_in(0, Zone::Hand, "Rowdy Research");
    t.mana(0, "{3}{U}");
    cite!(["601.2f"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): t.g.clone().act(0, paid("CCCU", Move::Cast { card, targets: vec![] })));
    t.mana(0, "{1}");
    cite!(["608.2i", "601.2f"]; ok: t.g.act(0, paid("CCCCU", Move::Cast { card, targets: vec![] })));
}

// Rules: 509.1h, 509.1, 115.1
// Given: the Hill Giant attacks and the opponent's Wall of Stone blocks it.
// When: Sizzling Barrage ("target creature that blocked this turn") is cast.
// Expect: the Wall is a legal target, the blocked Giant is not.
// Why: a blocked attacker became blocked (509.1h); it did not block.
#[test]
fn r509_1_a_blocked_attacker_did_not_block() {
    let mut t = Board::new(2).battlefield(0, &["Hill Giant"]).battlefield(1, &["Wall of Stone"]).hand(0, &["Sizzling Barrage"]).build();
    let (giant, wall) = (t.find_in_battlefield("Hill Giant"), t.find_in_battlefield("Wall of Stone"));
    while t.g.decision != Some(Decision::Attackers { player: 0 }) { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.g.act(0, Move::Attack(vec![(giant, Target::Player(1))])).unwrap();
    while t.g.decision != Some(Decision::Blockers { player: 1 }) { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.g.act(1, Move::Block(vec![(wall, giant)])).unwrap();
    t.to_no_combat(0, Part::PostcombatMain);
    t.mana(0, "{1}{R}");
    let barrage = t.find_in(0, Zone::Hand, "Sizzling Barrage");
    cite!(["509.1h", "115.1"]; refused("an illegal target, Hill Giant: it doesn't match the target's description (115.1)"): t.g.clone().act(0, paid("CR", Move::Cast { card: barrage, targets: vec![vec![Target::Object(giant)]] })));
    cite!(["509.1"]; ok: t.g.act(0, paid("CR", Move::Cast { card: barrage, targets: vec![vec![Target::Object(wall)]] })));
}

// Rules: 506.4, 701.19a, 508.1, 608.2i
// Given: Drudge Skeletons ("{B}: Regenerate this creature") attack and the opponent's Craw
// Wurm blocks them; Relentless Assault ("Untap all creatures that attacked this turn").
// When: the Skeletons regenerate from the Wurm's damage, then Relentless Assault resolves.
// Expect: the Skeletons untap.
// Why: regeneration taps the creature and removes it from combat (701.19a, 506.4), but it was
// declared as an attacker, so it attacked this turn (508.1).
#[test]
fn r506_4_a_creature_removed_from_combat_still_attacked() {
    let mut t = Board::new(2).battlefield(0, &["Drudge Skeletons"]).battlefield(1, &["Craw Wurm"]).hand(0, &["Relentless Assault"]).build();
    let (skeletons, wurm) = (t.find_in_battlefield("Drudge Skeletons"), t.find_in_battlefield("Craw Wurm"));
    while t.g.decision != Some(Decision::Attackers { player: 0 }) { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.g.act(0, Move::Attack(vec![(skeletons, Target::Player(1))])).unwrap();
    while t.g.decision != Some(Decision::Blockers { player: 1 }) { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.g.act(1, Move::Block(vec![(wurm, skeletons)])).unwrap();
    while t.g.priority != 0 { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.mana(0, "{B}");
    t.g.act(0, paid("B", Move::Activate { source: skeletons, ability: 0, targets: vec![], paying: vec![] })).unwrap();
    t.resolve();
    t.to_no_combat(0, Part::PostcombatMain);
    cite!(["701.19a"]; assert!(t.g.battlefield.contains(&skeletons) && t.g.obj(skeletons).tapped, "regenerated: tapped, removed from combat"));
    t.mana(0, "{2}{R}{R}").cast(0, "Relentless Assault", vec![], crate::support::mana_pool("CCRR")).unwrap();
    t.resolve();
    cite!(["508.1", "506.4"]; assert!(!t.g.obj(skeletons).tapped, "it attacked this turn"));
}

// Rules: 508.4, 508.1, 601.2f
// Given: Hero of Bladehold ("Whenever this creature attacks, create two 1/1 white Soldier
// creature tokens that are tapped and attacking"); Rowdy Research in hand.
// When: the Hero attacks alone; Rowdy Research is cast after combat.
// Expect: it costs {1} less, not {3}.
// Why: creatures put onto the battlefield attacking never attacked (508.4).
#[test]
fn r508_4_tokens_put_onto_the_battlefield_attacking_never_attacked() {
    let mut t = Board::new(2).battlefield(0, &["Hero of Bladehold"]).hand(0, &["Rowdy Research"]).build();
    let hero = t.find_in_battlefield("Hero of Bladehold");
    while t.g.decision != Some(Decision::Attackers { player: 0 }) { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.g.act(0, Move::Attack(vec![(hero, Target::Player(1))])).unwrap();
    // the token trigger first on the stack, so battle cry resolves after the tokens exist
    if let Some(Decision::Order { player, abilities }) = t.g.decision.clone() { t.g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(); }
    t.to_no_combat(0, Part::PostcombatMain);
    assert_eq!(t.g.battlefield.len(), 3, "the Hero and two Soldiers");
    let card = t.find_in(0, Zone::Hand, "Rowdy Research");
    t.mana(0, "{4}{U}");
    cite!(["508.4", "601.2f"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): t.g.clone().act(0, paid("CCCCU", Move::Cast { card, targets: vec![] })));
    t.mana(0, "{1}");
    cite!(["508.1", "601.2f"]; ok: t.g.act(0, paid("CCCCCU", Move::Cast { card, targets: vec![] })));
}

// Rules: 615.1a, 120.1, 115.1
// Given: Fog ("Prevent all combat damage that would be dealt this turn"); the opponent's
// Spear of Heliod.
// When: Fog resolves, then the Grizzly Bears attack unblocked.
// Expect: the Spear can't target the Bears.
// Why: prevented damage is not dealt (615.1a), so the Bears dealt no damage this turn.
#[test]
fn r615_1a_prevented_damage_was_not_dealt() {
    let mut t = Board::new(2).battlefield(0, &["Grizzly Bears"]).hand(0, &["Fog"]).battlefield(1, &["Spear of Heliod"]).build();
    let (bears, spear) = (t.find_in_battlefield("Grizzly Bears"), t.find_in_battlefield("Spear of Heliod"));
    t.mana(0, "{G}").cast(0, "Fog", vec![], crate::support::mana_pool("G")).unwrap();
    t.resolve();
    while t.g.decision != Some(Decision::Attackers { player: 0 }) { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.g.act(0, Move::Attack(vec![(bears, Target::Player(1))])).unwrap();
    t.to_no_combat(0, Part::PostcombatMain);
    assert_eq!(t.g.players[1].life, 20);
    while t.g.priority != 1 { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.mana(1, "{1}{W}{W}");
    cite!(["615.1a", "115.1"]; refused("an illegal target, Grizzly Bears: it doesn't match the target's description (115.1)"):
        t.g.act(1, paid("CWW", Move::Activate { source: spear, ability: 1, targets: vec![vec![Target::Object(bears)]], paying: vec![] })));
}

// Rules: 400.7, 120.1, 115.1
// Given: Shock, Cloudshift and Fatal Blow; a Hill Giant.
// When: Shock deals 2 damage to the Giant, Cloudshift blinks it, Fatal Blow is cast.
// Expect: the returned Giant is not a legal target.
// Why: it is a new object (400.7); the damage was dealt to its previous existence.
#[test]
fn r400_7_a_blinked_creature_was_not_dealt_damage_this_turn() {
    let mut t = Board::new(2).battlefield(0, &["Hill Giant"]).hand(0, &["Shock", "Cloudshift", "Fatal Blow"]).build();
    let giant = t.find_in_battlefield("Hill Giant");
    t.mana(0, "{R}").cast(0, "Shock", vec![Tgt::Object(giant)], crate::support::mana_pool("R")).unwrap();
    t.resolve();
    t.mana(0, "{W}").cast(0, "Cloudshift", vec![Tgt::Object(giant)], crate::support::mana_pool("W")).unwrap();
    t.resolve();
    let again = t.find_in_battlefield("Hill Giant");
    assert!(again != giant && t.g.obj(again).damage == 0);
    t.mana(0, "{B}");
    cite!(["400.7", "115.1"]; refused("an illegal target, Hill Giant: it doesn't match the target's description (115.1)"):
        t.g.act(0, paid("B", Move::Cast { card: t.find_in(0, Zone::Hand, "Fatal Blow"), targets: vec![vec![Target::Object(again)]] })));
}

// Rules: 120.1, 500.4, 115.1
// Given: Shock and Fatal Blow; the opponent's Hill Giant.
// When: Shock hits the Giant; in the opponent's next upkeep Fatal Blow is cast at it.
// Expect: the Giant is no longer a legal target.
// Why: "this turn" looks back at the current turn only.
#[test]
fn r120_1_damage_dealt_last_turn_is_not_this_turn() {
    let mut t = Board::new(2).battlefield(1, &["Hill Giant"]).hand(0, &["Shock", "Fatal Blow"]).build();
    let giant = t.find_in_battlefield("Hill Giant");
    t.mana(0, "{R}").cast(0, "Shock", vec![Tgt::Object(giant)], crate::support::mana_pool("R")).unwrap();
    t.resolve();
    t.to_no_combat(1, Part::Upkeep);
    while t.g.priority != 0 { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.mana(0, "{B}");
    cite!(["120.1", "115.1"]; refused("an illegal target, Hill Giant: it doesn't match the target's description (115.1)"):
        t.g.act(0, paid("B", Move::Cast { card: t.find_in(0, Zone::Hand, "Fatal Blow"), targets: vec![vec![Target::Object(giant)]] })));
}
