//! The chain spells: as one resolves, the player it names, or the controller of the permanent
//! it named as that permanent last existed (608.2h), may pay a cost (118.12). If that player
//! does, they may copy the spell. The copy is theirs (707.10) and they may give it a new
//! target (707.10c). "That player or that permanent's controller" is one player, whichever
//! kind of thing the one target is (115.4).
use crate::support;
use support::*;
use game::Event;

fn copies(t: &T) -> Vec<(ObjId, usize)> {
    t.g.log.iter().filter_map(|e| match e { Event::SpellCopied { copy, by, .. } => Some((*copy, *by)), _ => None }).collect()
}
fn casts(t: &T) -> usize { t.g.log.iter().filter(|e| matches!(e, Event::Cast { .. })).count() }
fn pay(t: &mut T, p: usize, units: &str) {
    let Some((asked, Asked::Pay { .. })) = t.asked() else { panic!("a payment is asked: {:?}", t.asked()) };
    assert_eq!(asked, p, "the payment is asked of player {p}");
    t.answer(p, Answer::Pay { plan: vec![], with: Some(mana_pool(units)), tapped: vec![] }).unwrap();
}
fn may(t: &mut T, p: usize, yes: bool) {
    let Some((asked, Asked::May)) = t.asked() else { panic!("a yes or no is asked: {:?}", t.asked()) };
    assert_eq!(asked, p, "the choice is player {p}'s");
    t.answer(p, if yes { Answer::Yes } else { Answer::No }).unwrap();
}
/// "That player may sacrifice a land / discard a card": whether, then which (608.2d).
fn optional(t: &mut T, p: usize, among: &[&str], chosen: Option<&str>) {
    may(t, p, chosen.is_some());
    let Some(chosen) = chosen else { return };
    let Some((asked, Asked::Objects { among: offered, count: 1, .. })) = t.asked() else { panic!("which one is asked: {:?}", t.asked()) };
    assert_eq!(asked, p);
    let want: Vec<ObjId> = among.iter().map(|n| t.find(n)).collect();
    cite!(["608.2d"]; assert_eq!(offered, want));
    t.choose(p, &[chosen]).unwrap();
}
/// Two cards discarded at once: their owner orders them in the graveyard (404.3).
fn discard_two(t: &mut T, p: usize, cards: [&str; 2]) {
    let Some((asked, Asked::Objects { count: 2, .. })) = t.asked() else { panic!("two discards are asked: {:?}", t.asked()) };
    assert_eq!(asked, p, "player {p} discards");
    t.choose(p, &cards).unwrap();
    let Some((asked, Asked::OrderCards { .. })) = t.asked() else { panic!("the graveyard order is asked: {:?}", t.asked()) };
    assert_eq!(asked, p);
    cite!(["404.3"]; ok: t.choose(p, &cards));
}
/// The copy's new target, asked of the player who copied it; None keeps the old one.
fn retarget(t: &mut T, p: usize, to: Option<Target>) -> ObjId {
    let Some((asked, Asked::Retarget { copy, .. })) = t.asked() else { panic!("the copy's targets are asked: {:?}", t.asked()) };
    assert_eq!(asked, p, "the copy's targets are chosen by player {p}");
    t.answer(p, match to { Some(to) => Answer::Targets(vec![vec![to]]), None => Answer::No }).unwrap();
    copy
}

// Rules: 118.12, 707.10, 707.10c
// Given: Chain Lightning aimed at the opponent, who has {R}{R}.
// When: the opponent pays, copies it and aims the copy at the caster, who then declines to pay.
// Expect: 3 damage to each player; the copy was the opponent's and was not cast.
#[test]
fn r707_10_chain_lightning_the_targeted_player_pays_copies_and_aims_the_copy_back() {
    let mut t = Board::new(2).hand(0, &["Chain Lightning"]).build();
    t.mana(0, "{R}").cast(0, "Chain Lightning", vec![player(1)], mana_pool("R")).unwrap();
    t.mana(1, "{R}{R}");
    t.pass_all();
    cite!(["118.12"]; assert_eq!(t.g.players[1].life, 17));
    pay(&mut t, 1, "RR");
    may(&mut t, 1, true);
    let copy = retarget(&mut t, 1, Some(Target::Player(0)));
    cite!(["707.10"]; assert_eq!(copies(&t), vec![(copy, 1)]));
    cite!(["707.10"]; assert_eq!(t.g.controller(copy), 1));
    cite!(["707.10"]; assert_eq!(t.g.obj(copy).owner, 1));
    cite!(["707.10c"]; assert_eq!(t.g.obj(copy).targets, vec![vec![Target::Player(0)]]));
    cite!(["707.10"]; assert_eq!(casts(&t), 1));
    // The original has finished resolving: only the copy is on the stack.
    t.expect().rules(["608.2n"]).stack(&["Chain Lightning"]).zone_exact(0, Zone::Graveyard, &["Chain Lightning"]).check();
    t.pass_all();
    // The copy names its own target: the caster may pay to copy it again.
    let Some((0, Asked::Pay { .. })) = t.asked() else { panic!("the copy's target is asked: {:?}", t.asked()) };
    t.answer(0, Answer::No).unwrap();
    t.resolve();
    t.expect().rules(["120.3a", "707.10"]).life(0, 17).life(1, 17).stack(&[])
        .zone_exact(0, Zone::Graveyard, &["Chain Lightning"]).zone_exact(1, Zone::Graveyard, &[]).check();
    assert_eq!(copies(&t).len(), 1);
}

// Rules: 115.4, 118.12, 707.10c
// Given: Chain Lightning aimed at the opponent's Grizzly Bears.
// When: it resolves.
// Expect: the Bears' controller, not the caster, is asked to pay; the copy keeps its target
// when its controller changes nothing, and the second 3 damage goes to the dead Bears' target.
#[test]
fn r115_4_chain_lightning_asks_the_controller_of_the_permanent_it_hit() {
    let mut t = Board::new(2).hand(0, &["Chain Lightning"]).battlefield(1, &["Grizzly Bears", "Mountain"]).build();
    t.mana(0, "{R}").cast(0, "Chain Lightning", vec!["Grizzly Bears".into()], mana_pool("R")).unwrap();
    t.pass_all();
    let Some((1, Asked::Pay { .. })) = t.asked() else { panic!("the Bears' controller is asked: {:?}", t.asked()) };
    t.answer(1, Answer::No).unwrap();
    t.resolve();
    cite!(["118.12"]; assert!(copies(&t).is_empty()));
    t.expect().rules(["120.3a", "704.5g"]).zone_exact(1, Zone::Graveyard, &["Grizzly Bears"]).life(1, 20).life(0, 20)
        .zone_exact(0, Zone::Graveyard, &["Chain Lightning"]).stack(&[]).check();
}

// Rules: 118.12, 608.2d
// Given: the opponent pays {R}{R} for Chain Lightning.
// When: they then choose not to copy it.
// Expect: the mana is spent and there is no copy.
#[test]
fn r118_12_paying_does_not_oblige_the_player_to_copy() {
    let mut t = Board::new(2).hand(0, &["Chain Lightning"]).build();
    t.mana(0, "{R}").cast(0, "Chain Lightning", vec![player(1)], mana_pool("R")).unwrap();
    t.mana(1, "{R}{R}");
    t.pass_all();
    pay(&mut t, 1, "RR");
    may(&mut t, 1, false);
    t.resolve();
    cite!(["608.2d"]; assert!(copies(&t).is_empty()));
    cite!(["118.12"]; assert_eq!(t.g.players[1].pool.total(), 0));
    t.expect().life(1, 17).stack(&[]).zone_exact(0, Zone::Graveyard, &["Chain Lightning"]).check();
}

// Rules: 608.2b
// Given: Chain Lightning aimed at Grizzly Bears, and a Lightning Bolt in response.
// When: the Bolt kills the Bears first.
// Expect: Chain Lightning doesn't resolve; no one is asked to pay or copy.
#[test]
fn r608_2b_an_illegal_target_leaves_nothing_to_copy() {
    let mut t = Board::new(2).hand(0, &["Chain Lightning", "Lightning Bolt"]).battlefield(1, &["Grizzly Bears"]).build();
    t.mana(0, "{R}").cast(0, "Chain Lightning", vec!["Grizzly Bears".into()], mana_pool("R")).unwrap();
    t.mana(0, "{R}").cast(0, "Lightning Bolt", vec!["Grizzly Bears".into()], mana_pool("R")).unwrap();
    t.resolve();
    // Uncited (ruling): "If the targeted player or permanent is an illegal target as Chain Lightning tries to resolve ... It can't be copied."
    assert!(copies(&t).is_empty());
    t.expect().rules(["608.2b"]).zone_exact(0, Zone::Graveyard, &["Chain Lightning", "Lightning Bolt"])
        .zone_exact(1, Zone::Graveyard, &["Grizzly Bears"]).stack(&[]).check();
}

// Rules: 608.2h, 707.10, 707.10c
// Given: Chain of Vapor aimed at the opponent's Grizzly Bears; the opponent has an Island.
// When: the opponent sacrifices the Island, copies the spell and aims it at the caster's
// Ornithopter; the caster then sacrifices nothing.
// Expect: both creatures in their owners' hands; the Bears' controller was asked although
// the Bears had left the battlefield.
#[test]
fn r608_2h_chain_of_vapor_asks_the_controller_the_bounced_permanent_last_had() {
    let mut t = Board::new(2).hand(0, &["Chain of Vapor"]).battlefield(0, &["Ornithopter", "Plains"])
        .battlefield(1, &["Grizzly Bears", "Island", "Swamp"]).build();
    t.mana(0, "{U}").cast(0, "Chain of Vapor", vec!["Grizzly Bears".into()], mana_pool("U")).unwrap();
    t.pass_all();
    cite!(["608.2h"]; assert!(matches!(t.asked(), Some((1, Asked::May)))));
    optional(&mut t, 1, &["Island", "Swamp"], Some("Island"));
    may(&mut t, 1, true);
    let ornithopter = Target::Object(t.find_in(0, Zone::Battlefield, "Ornithopter"));
    let copy = retarget(&mut t, 1, Some(ornithopter));
    cite!(["707.10"]; assert_eq!(copies(&t), vec![(copy, 1)]));
    t.expect().rules(["701.21a"]).zone_exact(1, Zone::Hand, &["Grizzly Bears"]).zone_exact(1, Zone::Graveyard, &["Island"]).check();
    t.pass_all();
    optional(&mut t, 0, &["Plains"], None);
    t.resolve();
    t.expect().zone_exact(0, Zone::Hand, &["Ornithopter"]).zone_exact(1, Zone::Hand, &["Grizzly Bears"])
        .zone_exact(0, Zone::Battlefield, &["Plains"]).zone_exact(1, Zone::Battlefield, &["Swamp"]).zone_exact(0, Zone::Graveyard, &["Chain of Vapor"]).stack(&[]).check();
    assert_eq!(copies(&t).len(), 1);
}

// Rules: 608.2h, 707.10, 707.10c
// Given: Chain of Acid aimed at the opponent's Sol Ring.
// When: the opponent copies it and aims the copy at the caster's Mind Stone.
// Expect: both artifacts destroyed; the caster is offered the next copy and declines.
#[test]
fn r707_10_chain_of_acid_the_destroyed_permanents_controller_copies() {
    let mut t = Board::new(2).hand(0, &["Chain of Acid"]).battlefield(0, &["Mind Stone"]).battlefield(1, &["Sol Ring"]).build();
    t.mana(0, "{3}{G}").cast(0, "Chain of Acid", vec!["Sol Ring".into()], mana_pool("CCCG")).unwrap();
    t.pass_all();
    // The Sol Ring is in the graveyard: its controller as it last existed is asked.
    cite!(["608.2h"]; assert!(matches!(t.asked(), Some((1, Asked::May)))));
    may(&mut t, 1, true);
    let stone = Target::Object(t.find_in(0, Zone::Battlefield, "Mind Stone"));
    let copy = retarget(&mut t, 1, Some(stone));
    cite!(["707.10"]; assert_eq!(copies(&t), vec![(copy, 1)]));
    t.pass_all();
    may(&mut t, 0, false);
    t.resolve();
    t.expect().rules(["701.8a"]).zone_exact(0, Zone::Graveyard, &["Mind Stone", "Chain of Acid"])
        .zone_exact(1, Zone::Graveyard, &["Sol Ring"]).stack(&[]).check();
}

// Rules: 707.10, 707.10c, 701.9a
// Given: Chain of Smog aimed at the opponent, who holds three cards.
// When: the opponent discards two, copies the spell and aims it at the caster.
// Expect: the caster discards two cards too, then declines to copy.
#[test]
fn r707_10c_chain_of_smog_the_discarding_player_aims_the_copy_at_the_caster() {
    let mut t = Board::new(2).hand(0, &["Chain of Smog", "Forest", "Swamp", "Plains"]).hand(1, &["Island", "Mountain", "Wastes"]).build();
    t.mana(0, "{1}{B}").cast(0, "Chain of Smog", vec![player(1)], mana_pool("CB")).unwrap();
    t.pass_all();
    discard_two(&mut t, 1, ["Island", "Mountain"]);
    may(&mut t, 1, true);
    retarget(&mut t, 1, Some(Target::Player(0)));
    t.pass_all();
    discard_two(&mut t, 0, ["Forest", "Swamp"]);
    may(&mut t, 0, false);
    t.resolve();
    t.expect().rules(["701.9a", "707.10c"]).zone_exact(0, Zone::Hand, &["Plains"]).zone_exact(0, Zone::Graveyard, &["Forest", "Swamp", "Chain of Smog"])
        .zone_exact(1, Zone::Hand, &["Wastes"]).zone_exact(1, Zone::Graveyard, &["Island", "Mountain"]).stack(&[]).check();
}

// Rules: 118.12, 707.10
// Given: String of Disappearances aimed at the opponent's Grizzly Bears; they have {U}{U}.
// When: they pay, copy and aim it at the caster's Ornithopter; the caster has no mana.
// Expect: both creatures returned; the caster is asked once and declines.
#[test]
fn r118_12_string_of_disappearances_the_creatures_controller_pays_to_copy() {
    let mut t = Board::new(2).hand(0, &["String of Disappearances"]).battlefield(0, &["Ornithopter"])
        .battlefield(1, &["Grizzly Bears"]).build();
    t.mana(0, "{U}").cast(0, "String of Disappearances", vec!["Grizzly Bears".into()], mana_pool("U")).unwrap();
    t.mana(1, "{U}{U}");
    t.pass_all();
    pay(&mut t, 1, "UU");
    may(&mut t, 1, true);
    let ornithopter = Target::Object(t.find_in(0, Zone::Battlefield, "Ornithopter"));
    retarget(&mut t, 1, Some(ornithopter));
    t.pass_all();
    let Some((0, Asked::Pay { .. })) = t.asked() else { panic!("the Ornithopter's controller is asked: {:?}", t.asked()) };
    t.answer(0, Answer::No).unwrap();
    t.resolve();
    cite!(["707.10"]; assert_eq!(copies(&t).len(), 1));
    t.expect().zone_exact(0, Zone::Hand, &["Ornithopter"]).zone_exact(1, Zone::Hand, &["Grizzly Bears"]).stack(&[]).check();
}

// Rules: 118.12, 707.10c
// Given: Chain of Plasma aimed at the opponent, who holds a card.
// When: they discard it, copy the spell and leave its target unchanged.
// Expect: 6 damage to the opponent, who then declines to discard for a third.
#[test]
fn r707_10c_chain_of_plasma_a_copy_may_keep_its_target() {
    let mut t = Board::new(2).hand(0, &["Chain of Plasma"]).hand(1, &["Island", "Mountain"]).build();
    t.mana(0, "{1}{R}").cast(0, "Chain of Plasma", vec![player(1)], mana_pool("CR")).unwrap();
    t.pass_all();
    optional(&mut t, 1, &["Island", "Mountain"], Some("Island"));
    may(&mut t, 1, true);
    retarget(&mut t, 1, None);
    t.pass_all();
    optional(&mut t, 1, &["Mountain"], None);
    t.resolve();
    cite!(["707.10c"]; assert_eq!(t.g.players[1].life, 14));
    t.expect().rules(["118.12"]).zone_exact(1, Zone::Hand, &["Mountain"]).zone_exact(1, Zone::Graveyard, &["Island"])
        .zone_exact(0, Zone::Graveyard, &["Chain of Plasma"]).stack(&[]).check();
}

// Rules: 118.12, 707.10
// Given: Chain of Silence aimed at the opponent's Grizzly Bears.
// When: the opponent sacrifices a Forest and copies it onto the caster's Ornithopter.
// Expect: the copy is the opponent's; the caster is offered the next copy and keeps the land.
#[test]
fn r707_10_chain_of_silence_the_creatures_controller_sacrifices_a_land_to_copy() {
    let mut t = Board::new(2).hand(0, &["Chain of Silence"]).battlefield(0, &["Ornithopter", "Plains"])
        .battlefield(1, &["Grizzly Bears", "Forest", "Mountain"]).build();
    t.mana(0, "{1}{W}").cast(0, "Chain of Silence", vec!["Grizzly Bears".into()], mana_pool("CW")).unwrap();
    t.pass_all();
    optional(&mut t, 1, &["Forest", "Mountain"], Some("Forest"));
    may(&mut t, 1, true);
    let ornithopter = Target::Object(t.find_in(0, Zone::Battlefield, "Ornithopter"));
    let copy = retarget(&mut t, 1, Some(ornithopter));
    cite!(["707.10"]; assert_eq!(t.g.controller(copy), 1));
    t.pass_all();
    optional(&mut t, 0, &["Plains"], None);
    t.resolve();
    t.expect().zone_exact(1, Zone::Graveyard, &["Forest"]).zone_exact(1, Zone::Battlefield, &["Grizzly Bears", "Mountain"])
        .zone_exact(0, Zone::Battlefield, &["Ornithopter", "Plains"]).stack(&[]).check();
    assert_eq!(copies(&t).len(), 1);
}

// Rules: 702.34a, 707.10, 707.10c
// Given: Sevinne's Reclamation in the graveyard, with two cards to return.
// When: it is cast with flashback; its controller copies it onto the other card.
// Expect: both permanents return; the copy, which was not cast from a graveyard, offers no
// further copy.
#[test]
fn r702_34a_sevinnes_reclamation_cast_from_a_graveyard_copies_itself_once() {
    let mut t = Board::new(2).graveyard(0, &["Sevinne's Reclamation", "Ornithopter", "Mind Stone"]).build();
    let card = t.find_in(0, Zone::Graveyard, "Sevinne's Reclamation");
    let thopter = Target::Object(t.find_in(0, Zone::Graveyard, "Ornithopter"));
    let stone = Target::Object(t.find_in(0, Zone::Graveyard, "Mind Stone"));
    t.mana(0, "{4}{W}");
    cite!(["702.34a"]; ok: t.g.act(0, paid("CCCCW", Move::CastVia(game::announcement::CastRoute::Flashback, Box::new(Move::Cast { card, targets: vec![vec![thopter]] })))));
    t.pass_all();
    may(&mut t, 0, true);
    let copy = retarget(&mut t, 0, Some(stone));
    cite!(["707.10"]; assert_eq!(copies(&t), vec![(copy, 0)]));
    // Uncited (ruling): "The copy wasn't cast from a graveyard, so it won't make another copy of itself."
    t.resolve();
    t.expect().rules(["702.34a"]).zone_exact(0, Zone::Battlefield, &["Ornithopter", "Mind Stone"]).zone_exact(0, Zone::Exile, &["Sevinne's Reclamation"])
        .zone_exact(0, Zone::Graveyard, &[]).stack(&[]).check();
}

// Rules: 707.10
// Given: Sevinne's Reclamation cast from the hand.
// When: it resolves.
// Expect: the card returns and nothing is offered.
#[test]
fn r707_10_sevinnes_reclamation_cast_from_the_hand_offers_no_copy() {
    let mut t = Board::new(2).hand(0, &["Sevinne's Reclamation"]).graveyard(0, &["Ornithopter"]).build();
    t.mana(0, "{2}{W}").cast(0, "Sevinne's Reclamation", vec![on(0, Zone::Graveyard, "Ornithopter")], mana_pool("CCW")).unwrap();
    t.resolve();
    // Uncited (card text): "If this spell was cast from a graveyard, you may copy this spell".
    assert!(copies(&t).is_empty());
    t.expect().zone_exact(0, Zone::Battlefield, &["Ornithopter"]).zone_exact(0, Zone::Graveyard, &["Sevinne's Reclamation"]).check();
}

// Rules: 115.4, 701.9a
// Given: Blightning aimed at the opponent's planeswalker, then at the opponent.
// When: each resolves.
// Expect: the opponent discards two cards each time.
#[test]
fn r115_4_blightning_the_planeswalkers_controller_or_the_player_discards() {
    for walker in [true, false] {
        let mut t = Board::new(2).hand(0, &["Blightning"]).hand(1, &["Island", "Mountain", "Forest"])
            .battlefield(1, &["Jace Beleren"]).counters("Jace Beleren", "loyalty", 5).build();
        let target = if walker { Tgt::Card("Jace Beleren") } else { player(1) };
        t.mana(0, "{1}{B}{R}").cast(0, "Blightning", vec![target], mana_pool("CBR")).unwrap();
        t.pass_all();
        discard_two(&mut t, 1, ["Island", "Mountain"]);
        t.resolve();
        t.expect().rules(["701.9a", "115.4"]).zone_exact(1, Zone::Hand, &["Forest"]).zone_exact(1, Zone::Graveyard, &["Island", "Mountain"])
            .life(1, if walker { 20 } else { 17 }).check();
        if walker { t.expect().rules(["120.3c"]).counters("Jace Beleren", "loyalty", 2).check(); }
    }
}

// Rules: 115.4, 608.2h
// Given: Acorn Catapult aimed at the opponent's Grizzly Bears, then at the opponent.
// When: the ability resolves.
// Expect: the opponent gets the Squirrel each time.
#[test]
fn r115_4_acorn_catapult_the_permanents_controller_or_the_player_gets_the_squirrel() {
    for creature in [true, false] {
        let mut t = Board::new(2).battlefield(0, &["Acorn Catapult"]).battlefield(1, &["Grizzly Bears"]).build();
        let target = if creature { Tgt::Card("Grizzly Bears") } else { player(1) };
        t.mana(0, "{1}").activate(0, "Acorn Catapult", 0, vec![target], mana_pool("C")).unwrap();
        t.resolve();
        t.expect().rules(["111.2"]).zone_exact(1, Zone::Battlefield, &["Grizzly Bears", "Squirrel Token"])
            .zone_exact(0, Zone::Battlefield, &["Acorn Catapult"]).life(1, if creature { 20 } else { 19 }).check();
    }
}

// Rules: 115.4
// Given: Rakdos's Return, whose target is "target opponent or planeswalker".
// Expect: refused: its "that player" names a qualified player, which the shared reading does
// not cover yet.
#[test]
fn r115_4_a_qualified_player_alternative_is_refused() {
    let def = oracle_read("Rakdos's Return");
    // Uncited (admission): the qualified alternative is not read as one player yet.
    support::refused(game::play::check(&def).map_err(|e| e.0), "either player: the player or the controller of one named target");
}
