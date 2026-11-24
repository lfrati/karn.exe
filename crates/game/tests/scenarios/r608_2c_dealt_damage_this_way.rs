//! "A creature dealt damage this way", "creatures destroyed this way": what the instruction
//! before did, its result fixed as it was done (608.2c). "If a creature dealt damage this way
//! would die this turn, exile it instead" is a replacement of the creatures that were dealt
//! damage (614.1a), however they would die later in the turn; "can't be regenerated" leaves
//! their regeneration shields unapplied, though shields are still made (701.19c).
use crate::support::{Board, Boundary, T, Tgt, cite, mana_pool, player};
use game::announcement::CastCost;
use game::legal::Asked;
use game::state::Answer;
use game::{Event, Move, Zone};
use ir::Part;

fn board(hand: &[&str], theirs: &[&str]) -> T {
    Board::new(2).fill(5).at(Part::PrecombatMain).hand(0, hand).battlefield(1, theirs).build()
}

/// Player 0 casts this card from their hand, paying `mana`, and it resolves.
fn cast(t: &mut T, name: &str, targets: Vec<Tgt>, mana: &str) {
    t.mana(0, mana).cast(0, name, targets, mana_pool(mana)).unwrap().resolve();
}

#[test]
fn r614_1a_a_creature_dealt_damage_this_way_is_exiled_whenever_it_would_die_this_turn() {
    let mut t = board(&["Anger of the Gods", "Shock", "Lightning Bolt", "Grizzly Bears"], &["Grizzly Bears", "Serra Angel"]);
    let angel = t.find_in(1, Zone::Battlefield, "Serra Angel");
    cast(&mut t, "Anger of the Gods", vec![], "{1}{R}{R}");
    // the 2/2 dies of its damage (704.5g) and is exiled instead; the 4/4 is not destroyed
    t.expect().rules(["614.1a", "608.2c"]).zone(1, Zone::Exile, &["Grizzly Bears"]).zone(1, Zone::Graveyard, &[]).zone(1, Zone::Battlefield, &["Serra Angel"]).check();
    cite!(["120.3"]; assert_eq!(t.g.obj(angel).damage, 3));
    // a creature that is not one Anger of the Gods dealt damage to dies as ever
    cast(&mut t, "Grizzly Bears", vec![], "{G}{1}");
    cast(&mut t, "Shock", vec![Tgt::In(0, Zone::Battlefield, "Grizzly Bears")], "{R}");
    t.expect().rules(["614.1a"]).zone(0, Zone::Graveyard, &["Anger of the Gods", "Shock", "Grizzly Bears"]).zone(0, Zone::Exile, &[]).check();
    // the angel was: dying later in the turn of other damage, it is exiled
    cast(&mut t, "Lightning Bolt", vec![Tgt::Object(angel)], "{R}");
    t.expect().rules(["614.1a", "608.2c"]).zone(1, Zone::Exile, &["Grizzly Bears", "Serra Angel"]).zone(1, Zone::Graveyard, &[]).check();
}

#[test]
fn r608_2c_damage_dealt_to_a_player_this_way_is_dealt_to_no_creature() {
    let mut t = board(&["Pillar of Flame", "Shock"], &["Grizzly Bears"]);
    cast(&mut t, "Pillar of Flame", vec![player(1)], "{R}");
    cite!(["120.3"]; assert_eq!(t.g.players[1].life, 18));
    cast(&mut t, "Shock", vec![Tgt::In(1, Zone::Battlefield, "Grizzly Bears")], "{R}");
    t.expect().rules(["608.2c", "614.1a"]).zone(1, Zone::Graveyard, &["Grizzly Bears"]).zone(1, Zone::Exile, &[]).check();
}

#[test]
fn r701_19c_a_creature_dealt_damage_this_way_is_not_regenerated_this_turn() {
    // with a shield made before Incinerate, the skeletons die of its damage
    let mut t = board(&["Incinerate"], &["Drudge Skeletons"]);
    let skeletons = t.find_in(1, Zone::Battlefield, "Drudge Skeletons");
    t.g.regenerate(skeletons);
    cast(&mut t, "Incinerate", vec![Tgt::Object(skeletons)], "{R}{1}");
    t.expect().rules(["701.19c", "704.5g"]).zone(1, Zone::Graveyard, &["Drudge Skeletons"]).check();
    cite!(["701.19c"]; assert!(!t.g.log.contains(&Event::Regenerated { object: skeletons })));

    // Shock leaves the shield to do what it does
    let mut t = board(&["Shock"], &["Drudge Skeletons"]);
    let skeletons = t.find_in(1, Zone::Battlefield, "Drudge Skeletons");
    t.g.regenerate(skeletons);
    cast(&mut t, "Shock", vec![Tgt::Object(skeletons)], "{R}");
    t.expect().rules(["701.19a"]).zone(1, Zone::Battlefield, &["Drudge Skeletons"]).tapped("Drudge Skeletons", true).check();

    // a shield made after Incinerate is made, and is not applied this turn
    let mut t = board(&["Incinerate", "Shock"], &["Serra Angel"]);
    let angel = t.find_in(1, Zone::Battlefield, "Serra Angel");
    cast(&mut t, "Incinerate", vec![Tgt::Object(angel)], "{R}{1}");
    t.g.regenerate(angel);
    cast(&mut t, "Shock", vec![Tgt::Object(angel)], "{R}");
    t.expect().rules(["701.19c"]).zone(1, Zone::Graveyard, &["Serra Angel"]).check();
}

#[test]
fn r701_19c_a_creature_destroyed_this_way_is_not_regenerated() {
    let mut t = board(&["Damn"], &["Drudge Skeletons"]);
    let skeletons = t.find_in(1, Zone::Battlefield, "Drudge Skeletons");
    t.g.regenerate(skeletons);
    cast(&mut t, "Damn", vec![Tgt::Object(skeletons)], "{B}{B}");
    t.expect().rules(["701.19c", "701.8a"]).zone(1, Zone::Graveyard, &["Drudge Skeletons"]).check();

    // overloaded, it destroys each creature (702.96a), and none is regenerated
    let mut t = board(&["Damn"], &["Drudge Skeletons", "Grizzly Bears"]);
    let skeletons = t.find_in(1, Zone::Battlefield, "Drudge Skeletons");
    t.g.regenerate(skeletons);
    let card = t.find_in(0, Zone::Hand, "Damn");
    let row = t.g.def(card).unwrap().abilities().iter().position(|a| matches!(a, ir::Ability::Keywords(items)
        if items.iter().any(|i| i.keyword.name() == "Overload"))).expect("an overload ability");
    t.mana(0, "{2}{W}{W}");
    t.g.act(0, Move::Paying(mana_pool("WWCC"), Box::new(Move::CastFor(CastCost::Alternative(row), Box::new(Move::Cast { card, targets: vec![] }))))).unwrap();
    // both are put into their owner's graveyard at once, in the order the owner chooses (404.3)
    let Boundary::Question(1, Asked::OrderCards { cards, .. }) = t.settle() else { panic!("expected the owner to order the cards") };
    t.g.act(1, Move::Choose(Answer::Objects(cards))).unwrap();
    t.resolve();
    t.expect().rules(["701.19c", "702.96a"]).zone(1, Zone::Graveyard, &["Drudge Skeletons", "Grizzly Bears"]).check();
}

#[test]
fn r614_1a_a_creature_that_would_die_this_turn_is_exiled_one_there_then_or_not() {
    let mut t = board(&["Malicious Malfunction", "Grizzly Bears", "Shock"], &["Grizzly Bears", "Hill Giant"]);
    cast(&mut t, "Malicious Malfunction", vec![], "{1}{B}{B}");
    // the 2/2 dies of -2/-2 (704.5f) and is exiled; the 3/3 is a 1/1
    t.expect().rules(["614.1a", "704.5f"]).zone(1, Zone::Exile, &["Grizzly Bears"]).zone(1, Zone::Battlefield, &["Hill Giant"]).pt("Hill Giant", 1, 1).check();
    // a creature that enters later this turn, and dies, is exiled too (611.2c)
    cast(&mut t, "Grizzly Bears", vec![], "{1}{G}");
    cast(&mut t, "Shock", vec![Tgt::In(0, Zone::Battlefield, "Grizzly Bears")], "{R}");
    t.expect().rules(["614.1a", "611.2c"]).zone(0, Zone::Exile, &["Grizzly Bears"]).zone(0, Zone::Graveyard, &["Malicious Malfunction", "Shock"]).check();
}

#[test]
fn r614_1a_only_a_creature_an_opponent_controls_is_exiled_as_it_would_die() {
    let mut t = Board::new(2).fill(5).at(Part::PrecombatMain).hand(0, &["Malicious Eclipse"])
        .battlefield(0, &["Grizzly Bears"]).battlefield(1, &["Grizzly Bears"]).build();
    cast(&mut t, "Malicious Eclipse", vec![], "{1}{B}{B}");
    t.expect().rules(["614.1a", "109.4"]).zone(1, Zone::Exile, &["Grizzly Bears"]).zone(0, Zone::Graveyard, &["Malicious Eclipse", "Grizzly Bears"]).check();
}
