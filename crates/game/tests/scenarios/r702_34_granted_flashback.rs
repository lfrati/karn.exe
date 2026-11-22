//! Snapcaster's recipient gains flashback for its actual mana cost (702.34a), independently
//! of the granting source, until the effect ends or the recipient changes zones (400.7).
use crate::support::{Board, T, cite, mana_pool, oracle, paid};
use game::{Move, ObjId, Target, Zone};

fn setup() -> T {
    setup_card("Lightning Bolt")
}
fn setup_card(name: &str) -> T {
    Board::new(2).hand(0, &["Snapcaster Mage", "Unsummon"])
        .graveyard(0, &[name]).hand(1, &["Counterspell"]).fill(8).build()
}
fn announce_grant(t: &mut T) -> (ObjId, ObjId) {
    announce_grant_for(t, "Lightning Bolt")
}
fn announce_grant_for(t: &mut T, name: &str) -> (ObjId, ObjId) {
    let mage = t.find_in(0, Zone::Hand, "Snapcaster Mage");
    let card = t.find_in(0, Zone::Graveyard, name);
    t.g.players[0].pool = mana_pool("UU");
    t.g.act(0, paid("UU", Move::Cast {card: mage, targets: vec![]})).unwrap();
    t.pass_all();
    t.g.act(0, Move::ChooseTargets(vec![vec![card.into()]])).unwrap();
    (t.find_in(0, Zone::Battlefield, "Snapcaster Mage"), card)
}
fn flashback(card: ObjId) -> Move {
    Move::CastVia(game::announcement::CastRoute::Flashback,
        Box::new(Move::Cast {card, targets: vec![vec![Target::Player(1)]]}))
}

#[test]
fn r702_34a_snapcaster_grants_the_cards_colored_mana_cost_and_exiles_after_resolution() {
    let mut t = setup();
    let (_, card) = announce_grant(&mut t);
    t.pass_all();
    cite!(["702.34a"]; assert!(t.g.has(card, "Flashback")));
    t.g.players[0].pool = mana_pool("C");
    let before = format!("{:?}", t.g);
    // {R} is the card's cost; mana value 1 does not turn it into a generic {1} cost.
    cite!(["702.34a", "601.2f", "118.3a"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"):
        t.g.act(0, paid("C", flashback(card))));
    assert_eq!(format!("{:?}", t.g), before);
    t.g.players[0].pool = mana_pool("R");
    t.g.act(0, paid("R", flashback(card))).unwrap();
    let mut branch = T::new(t.g.clone());
    for t in [&mut t, &mut branch] {
        t.pass_all();
        cite!(["702.34a", "120.3a"]; assert_eq!(t.g.players[1].life, 17));
        cite!(["702.34a"]; assert_eq!(t.g.obj(t.g.exile[0]).chars.name, "Lightning Bolt"));
        assert!(t.g.players[0].graveyard.is_empty());
    }
}

#[test]
fn r113_7a_the_grant_survives_its_source_departing_and_countering_still_exiles_the_spell() {
    let mut t = setup();
    let (source, card) = announce_grant(&mut t);
    let bounce = t.find_in(0, Zone::Hand, "Unsummon");
    t.g.players[0].pool = mana_pool("U");
    t.g.act(0, paid("U", Move::Cast {card: bounce, targets: vec![vec![source.into()]]})).unwrap();
    t.pass_all();
    cite!(["400.7"]; assert_eq!(t.g.obj(source).zone, Zone::Gone));
    t.pass_all();
    cite!(["113.7a", "611.2a"]; assert!(t.g.has(card, "Flashback")));
    t.g.players[0].pool = mana_pool("R");
    t.g.act(0, paid("R", flashback(card))).unwrap();
    let spell = *t.g.stack.last().unwrap();
    t.g.act(0, Move::Pass).unwrap();
    let counter = t.find_in(1, Zone::Hand, "Counterspell");
    t.g.players[1].pool = mana_pool("UU");
    t.g.act(1, paid("UU", Move::Cast {card: counter, targets: vec![vec![spell.into()]]})).unwrap();
    t.pass_all();
    cite!(["702.34a", "701.6a"]; assert_eq!(t.g.players[1].life, 20));
    cite!(["702.34a"]; assert_eq!(t.g.obj(t.g.exile[0]).chars.name, "Lightning Bolt"));
}

#[test]
fn r400_7_a_graveyard_card_that_leaves_and_returns_does_not_keep_the_grant() {
    let mut t = setup();
    let (_, card) = announce_grant(&mut t);
    t.pass_all();
    cite!(["702.34a"]; assert!(t.g.has(card, "Flashback")));
    t.g.move_object(card, Zone::Exile);
    let exiled = t.g.exile[0];
    t.g.move_object(exiled, Zone::Graveyard);
    let returned = t.find_in(0, Zone::Graveyard, "Lightning Bolt");
    cite!(["400.7"]; assert_ne!(returned, card));
    cite!(["400.7", "611.2a"]; assert!(!t.g.has(returned, "Flashback")));
}

#[test]
fn r107_3a_a_granted_flashback_cost_keeps_x_in_the_cards_mana_cost() {
    let mut t = setup_card("Blaze");
    let (_, card) = announce_grant_for(&mut t, "Blaze");
    t.pass_all();
    t.g.players[0].pool = mana_pool("CCCR");
    t.g.act(0, Move::WithX(3, Box::new(paid("CCCR", flashback(card))))).unwrap();
    t.pass_all();
    // Pinned ruling 31c9a4cd8dd28835: X is still chosen, not frozen to graveyard mana value.
    cite!(["107.3a", "702.34a"]; assert_eq!(t.g.players[1].life, 17));
    assert_eq!(t.g.obj(t.g.exile[0]).chars.name, "Blaze");
}

#[test]
fn r202_1b_a_card_without_a_mana_cost_gains_flashback_but_cannot_pay_it() {
    // Authored no-cost recipient; the full Snapcaster is the granting witness. Ancestral
    // Vision itself still refuses for unsupported suspend, so it is not a partial fixture.
    let blank = game::CardDef::new("No price", "", "Instant", "You gain 1 life.", None).unwrap();
    let mut g = game::Game::new(vec![oracle("Snapcaster Mage"), blank], 2, 20);
    for def in g.defs.iter() { game::play::check(def).unwrap(); }
    let mage = g.put(0, 0, Zone::Hand);
    let card = g.put(0, 1, Zone::Graveyard);
    g.players[0].pool = mana_pool("UU");
    g.act(0, paid("UU", Move::Cast {card: mage, targets: vec![]})).unwrap();
    let mut t = T::new(g);
    t.pass_all();
    t.g.act(0, Move::ChooseTargets(vec![vec![card.into()]])).unwrap();
    t.pass_all();
    cite!(["702.34a"]; assert!(t.g.has(card, "Flashback")));
    let before = format!("{:?}", t.g);
    cite!(["202.1b", "118.6"]; refused("that card has no mana cost (202.1b)"):
        t.g.act(0, Move::CastVia(game::announcement::CastRoute::Flashback, Box::new(Move::Cast {card, targets: vec![]}))));
    assert_eq!(format!("{:?}", t.g), before);
}

#[test]
fn r113_2c_printed_and_granted_costs_are_distinct_choices_without_autoselection() {
    let mut t = setup_card("Think Twice");
    let (_, card) = announce_grant_for(&mut t, "Think Twice");
    t.pass_all();
    let cast = Move::CastVia(game::announcement::CastRoute::Flashback, Box::new(Move::Cast {card, targets: vec![]}));
    let before = format!("{:?}", t.g);
    cite!(["113.2c", "601.2b"]; refused("multiple flashback costs are available; say which flashback cost is paid (113.2c, 601.2b)"):
        t.g.act(0, cast.clone()));
    assert_eq!(format!("{:?}", t.g), before);
    cite!(["113.2c", "601.2b"]; refused("that flashback cost is unavailable (702.34a)"):
        t.g.act(0, Move::CastFor(game::announcement::CastCost::Flashback(2), Box::new(cast.clone()))));
    assert_eq!(format!("{:?}", t.g), before);
    // Think Twice prints {2}{U}; Snapcaster grants {1}{U}, its mana cost.
    for (index, spend) in [(0, "UUU"), (1, "UU")] {
        let mut branch = T::new(t.g.clone());
        branch.g.players[0].pool = mana_pool(spend);
        let hand = branch.g.players[0].hand.len();
        branch.g.act(0, Move::CastFor(game::announcement::CastCost::Flashback(index), Box::new(paid(spend, cast.clone())))).unwrap();
        branch.pass_all();
        cite!(["702.34a", "113.2c"]; assert_eq!(branch.g.players[0].hand.len(), hand + 1));
        assert_eq!(branch.g.obj(branch.g.exile[0]).chars.name, "Think Twice");
    }
}

#[test]
fn r118_8_granted_flashback_still_pays_mandatory_additional_costs() {
    let mut t = setup_card("Village Rites");
    let (mage, card) = announce_grant_for(&mut t, "Village Rites");
    t.pass_all();
    t.g.players[0].pool = mana_pool("B");
    let hand = t.g.players[0].hand.len();
    let cast = Move::CastVia(game::announcement::CastRoute::Flashback, Box::new(Move::Cast {card, targets: vec![]}));
    t.g.act(0, Move::WithCosts(vec![vec![mage]], Box::new(paid("B", cast)))).unwrap();
    cite!(["118.8", "601.2h"]; assert_eq!(t.g.obj(mage).zone, Zone::Gone));
    t.pass_all();
    cite!(["118.8", "121.1"]; assert_eq!(t.g.players[0].hand.len(), hand + 2));
    cite!(["702.34a"]; assert_eq!(t.g.obj(t.g.exile[0]).chars.name, "Village Rites"));
}

#[test]
fn r514_2_the_resolved_flashback_grant_ends_during_cleanup() {
    let mut t = setup();
    let (_, card) = announce_grant(&mut t);
    t.pass_all();
    t.to_no_combat(1, ir::Part::PrecombatMain);
    cite!(["514.2", "611.2a"]; assert!(!t.g.has(card, "Flashback")));
    t.g.players[0].pool = mana_pool("R");
    t.g.act(1, Move::Pass).unwrap();
    let before = format!("{:?}", t.g);
    cite!(["702.34a"]; refused("that card cannot be cast using flashback (702.34a)"):
        t.g.act(0, paid("R", flashback(card))));
    assert_eq!(format!("{:?}", t.g), before);
}

#[test]
fn r709_3b_the_granted_cost_is_the_chosen_split_half_not_the_combined_mana_cost() {
    let mut t = setup_card("Fire // Ice");
    let (mage, card) = announce_grant_for(&mut t, "Fire // Ice");
    t.pass_all();
    t.g.players[0].pool = mana_pool("UU");
    let hand = t.g.players[0].hand.len();
    let cast = Move::CastAs(game::card::CastAs::Half(1), Box::new(
        Move::CastVia(game::announcement::CastRoute::Flashback, Box::new(
            Move::Cast {card, targets: vec![vec![mage.into()]]}))));
    t.g.act(0, paid("UU", cast)).unwrap();
    cite!(["709.3b", "702.34a"]; assert_eq!(t.g.obj(t.g.stack[0]).chars.mana_value, 2));
    t.pass_all();
    cite!(["608.2c", "121.1"]; assert_eq!(t.g.players[0].hand.len(), hand + 1));
    cite!(["702.34a", "709.4"]; assert_eq!(t.g.obj(t.g.exile[0]).chars.name, "Fire // Ice"));
}

#[test]
fn r118_9a_a_granted_flashback_cost_cannot_combine_with_a_printed_alternative() {
    let mut t = setup_card("Force of Will");
    let (_, card) = announce_grant_for(&mut t, "Force of Will");
    t.pass_all();
    let unsummon = t.find_in(0, Zone::Hand, "Unsummon");
    let mage = t.find_in(0, Zone::Battlefield, "Snapcaster Mage");
    t.g.players[0].pool = mana_pool("U");
    t.g.act(0, paid("U", Move::Cast {card: unsummon, targets: vec![vec![mage.into()]]})).unwrap();
    let before = format!("{:?}", t.g);
    let cast = Move::CastFor(game::announcement::CastCost::Alternative(0), Box::new(
        Move::CastVia(game::announcement::CastRoute::Flashback, Box::new(
            Move::Cast {card, targets: vec![vec![t.g.stack[0].into()]]}))));
    cite!(["118.9a", "702.34a"]; refused("only one alternative cost is paid for a spell (118.9a)"): t.g.act(0, cast));
    assert_eq!(format!("{:?}", t.g), before);
}

#[test]
fn r611_2c_past_in_flames_grants_each_present_recipient_but_not_a_later_arrival() {
    let mut t = Board::new(2).hand(0, &["Past in Flames", "Shock"])
        .graveyard(0, &["Lightning Bolt", "Think Twice"]).fill(8).build();
    let flames = t.find_in(0, Zone::Hand, "Past in Flames");
    let bolt = t.find_in(0, Zone::Graveyard, "Lightning Bolt");
    let twice = t.find_in(0, Zone::Graveyard, "Think Twice");
    t.g.players[0].pool = mana_pool("RRRR");
    t.g.act(0, paid("RRRR", Move::Cast {card: flames, targets: vec![]})).unwrap();
    t.pass_all();
    cite!(["611.2c", "702.34a"]; assert!(t.g.has(bolt, "Flashback") && t.g.has(twice, "Flashback")));
    let shock = t.find_in(0, Zone::Hand, "Shock");
    t.g.players[0].pool = mana_pool("R");
    t.g.act(0, paid("R", Move::Cast {card: shock, targets: vec![vec![Target::Player(1)]]})).unwrap();
    t.pass_all();
    let shock = t.find_in(0, Zone::Graveyard, "Shock");
    cite!(["611.2c"]; assert!(!t.g.has(shock, "Flashback")));
    // The resolving Past in Flames was on the stack when it fixed the recipients;
    // only its printed {4}{R} flashback remains, not a newly granted {3}{R} instance.
    let flames = t.find_in(0, Zone::Graveyard, "Past in Flames");
    t.g.players[0].pool = mana_pool("RRRR");
    let before = format!("{:?}", t.g);
    cite!(["611.2c", "702.34a"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"):
        t.g.act(0, paid("RRRR", Move::CastVia(game::announcement::CastRoute::Flashback, Box::new(Move::Cast {card: flames, targets: vec![]})))));
    assert_eq!(format!("{:?}", t.g), before);
}

#[test]
fn r702_34a_sphinx_binds_that_cards_mana_cost_to_its_target() {
    let mut t = Board::new(2).battlefield(0, &["Sphinx of Forgotten Lore"])
        .graveyard(0, &["Lightning Bolt"]).fill(8).build();
    t.to_no_combat(0, ir::Part::BeginningOfCombat);
    for _ in 0..10 {
        if matches!(t.g.asked(), Some((0, game::legal::Asked::Attackers(_)))) { break; }
        t.g.act(t.g.priority, Move::Pass).unwrap();
    }
    let sphinx = t.find_in(0, Zone::Battlefield, "Sphinx of Forgotten Lore");
    let card = t.find_in(0, Zone::Graveyard, "Lightning Bolt");
    t.g.act(0, Move::Attack(vec![(sphinx, Target::Player(1))])).unwrap();
    let _ = t.g.asked();
    t.g.act(0, Move::ChooseTargets(vec![vec![card.into()]])).unwrap();
    t.pass_all();
    cite!(["702.34a"]; assert!(t.g.has(card, "Flashback")));
    t.g.players[0].pool = mana_pool("R");
    t.g.act(0, paid("R", flashback(card))).unwrap();
    t.pass_all();
    cite!(["702.34a", "120.3a"]; assert_eq!(t.g.players[1].life, 17));
}
