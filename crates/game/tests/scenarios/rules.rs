//! The model, rule by rule: each test is named by the Comprehensive Rules number it checks.
//!
//! What an action can be done to is declared once, in the lexicon ([`ir::lexicon`]), in the
//! rules' own sorts. The grammar asks that declaration whether a noun phrase can name such a
//! thing; the engine asks it whether a thing is one now. These tests ask both.

use crate::support::cite;
use game::card::CardDef;
use game::{Event, Game, ObjId, Target, Zone};
use ir::{CardType, Role, Sort, Verb};

const MOUNTAIN: usize = 0;
const BEARS: usize = 1;
const BOLT: usize = 2;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Lightning Bolt", "{R}", "Instant", "Lightning Bolt deals 3 damage to any target.", None),
    ]
}

fn game() -> Game {
    Game::new(defs(), 2, 20)
}

fn object(o: ObjId) -> Target {
    Target::Object(o)
}

/// Does the grammar read this as the text of an instant?
fn reads(text: &str) -> bool {
    CardDef::new("Test", "{R}", "Instant", text, None).is_ok()
}

#[test]
fn r110_1_a_card_is_a_permanent_only_on_the_battlefield() {
    let mut g = game();
    let in_hand = g.put(0, BEARS, Zone::Hand);
    let on_battlefield = g.put(0, BEARS, Zone::Battlefield);
    assert!(g.is(object(in_hand), Sort::Card));
    cite!(["110.1"]; assert!(!g.is(object(in_hand), Sort::Permanent)));
    cite!(["110.1"]; assert!(g.is(object(on_battlefield), Sort::Permanent)));
    assert!(g.is(object(on_battlefield), Sort::Of(CardType::Creature)));
    assert!(g.is(object(on_battlefield), Sort::Card), "being a permanent does not remove card backing (108.2)");
}

#[test]
fn r112_1_a_card_on_the_stack_is_a_spell() {
    let mut g = game();
    let bolt = g.put(0, BOLT, Zone::Stack);
    cite!(["112.1"]; assert!(g.is(object(bolt), Sort::Spell)));
    cite!(["108.2"]; assert!(g.is(object(bolt), Sort::Card), "a spell represented by a card remains a card (108.2)"));
    // what can be countered (701.6a) is what is on the stack
    cite!(["701.6a"]; assert_eq!(g.all_that(Verb::Counter, Role::What), vec![object(bolt)]));
}

#[test]
fn r120_1_damage_is_dealt_to_battles_creatures_planeswalkers_and_players() {
    let mut g = game();
    let land = g.put(1, MOUNTAIN, Zone::Battlefield);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    let source = g.put(0, BOLT, Zone::Stack);
    // asked with no card in hand: what can be dealt damage?
    assert_eq!(g.all_that(Verb::DealDamage, Role::To), vec![Target::Player(0), Target::Player(1), object(bears)]);
    // a land that is not a creature is not dealt damage: nothing happens, nothing is logged
    g.damage(source, object(land), 3);
    assert!(g.log.is_empty());
    assert_eq!(g.obj(land).damage, 0);
    g.damage(source, object(bears), 1);
    assert_eq!(g.obj(bears).damage, 1);
    cite!(["120.1"]; assert_eq!(g.log.to_vec(), vec![Event::DamageDealt { source, to: object(bears), amount: 1, combat: false }]));
    // and the grammar reads text by the same declaration: a spell is never dealt damage
    assert!(reads("Test deals 3 damage to target creature."));
    assert!(reads("Test deals 3 damage to any target."));
    assert!(!reads("Test deals 3 damage to target spell."));
}

#[test]
fn r701_8a_only_a_permanent_is_destroyed() {
    let mut g = game();
    let in_hand = g.put(0, BEARS, Zone::Hand);
    let land = g.put(1, MOUNTAIN, Zone::Battlefield);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    assert_eq!(g.all_that(Verb::Destroy, Role::What), vec![object(land), object(bears)]);
    assert!(!g.takes(Verb::Destroy, Role::What, Target::Player(1)));
    g.destroy(in_hand);
    assert!(g.log.is_empty());
    cite!(["701.8a"]; assert_eq!(g.players[0].hand, vec![in_hand]));
    g.destroy(bears);
    cite!(["701.8a"]; assert_eq!(g.players[1].graveyard.len(), 1));
    assert!(reads("Destroy target creature."));
    assert!(!reads("Destroy target player."));
    assert!(!reads("Destroy any target."));
}

#[test]
fn r701_13a_an_object_is_exiled_from_wherever_it_is() {
    let mut g = game();
    let in_hand = g.put(0, BEARS, Zone::Hand);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    g.exile(in_hand);
    g.exile(bears);
    cite!(["701.13a"]; assert_eq!(g.exile.len(), 2));
    assert!(!g.takes(Verb::Exile, Role::What, Target::Player(0)));
    assert!(!reads("Exile target player."));
}

#[test]
fn r701_26a_only_an_untapped_permanent_is_tapped() {
    let mut g = game();
    let land = g.put(0, MOUNTAIN, Zone::Battlefield);
    let in_hand = g.put(0, MOUNTAIN, Zone::Hand);
    g.tap(in_hand);
    cite!(["701.26a"]; assert!(g.log.is_empty()));
    g.tap(land);
    cite!(["118.3"]; assert!(!g.can(Verb::Tap, 0, object(land)), "it is tapped: it cannot pay a cost of {{T}} either (118.3)"));
    g.tap(land);
    assert_eq!(g.log.to_vec(), vec![Event::Tapped { object: land }]);
    assert!(!reads("Tap target player."));
    // a noun phrase that says its permanents are tapped names nothing that can be
    assert!(reads("Tap target untapped creature."));
    assert!(!reads("Tap target tapped creature."));
}

#[test]
fn r701_26b_only_a_tapped_permanent_is_untapped() {
    let mut g = game();
    let land = g.put(0, MOUNTAIN, Zone::Battlefield);
    assert!(!g.can(Verb::Untap, 0, object(land)));
    g.untap(land);
    cite!(["701.26b"]; assert!(g.log.is_empty()));
    g.tap(land);
    g.untap(land);
    assert_eq!(g.log.last(), Some(&Event::Untapped { object: land }));
    assert!(reads("Untap all tapped creatures."));
    assert!(!reads("Untap all untapped creatures."));
}

#[test]
fn r701_21a_a_player_sacrifices_only_a_permanent_they_control() {
    let mut g = game();
    let mine = g.put(0, BEARS, Zone::Battlefield);
    let theirs = g.put(1, BEARS, Zone::Battlefield);
    let in_hand = g.put(0, BEARS, Zone::Hand);
    assert!(g.can(Verb::Sacrifice, 0, object(mine)));
    assert!(!g.can(Verb::Sacrifice, 0, object(theirs)), "a permanent they don't control");
    assert!(!g.can(Verb::Sacrifice, 0, object(in_hand)), "something that isn't a permanent");
    g.sacrifice(0, theirs);
    g.sacrifice(0, in_hand);
    cite!(["701.21a"]; assert!(g.log.is_empty()));
    g.sacrifice(0, mine);
    assert_eq!(g.log[0], Event::Sacrificed { player: 0, object: mine });
    cite!(["701.21a"]; assert_eq!((g.battlefield.clone(), g.players[0].graveyard.len()), (vec![theirs], 1)));
    // not destruction (701.21a)
    cite!(["701.21a"]; assert!(!g.log.iter().any(|e| matches!(e, Event::Destroyed { .. }))));
    assert!(!reads("Sacrifice target player."));
}

#[test]
fn r701_9a_a_card_is_discarded_from_its_owners_hand() {
    let mut g = game();
    let mine = g.put(0, BEARS, Zone::Hand);
    let theirs = g.put(1, BEARS, Zone::Hand);
    let permanent = g.put(0, BEARS, Zone::Battlefield);
    assert!(g.can(Verb::Discard, 0, object(mine)));
    assert!(!g.can(Verb::Discard, 0, object(theirs)));
    assert!(!g.can(Verb::Discard, 0, object(permanent)));
    g.discard(0, theirs);
    g.discard(0, permanent);
    cite!(["701.9a"]; assert!(g.log.is_empty()));
    g.discard(0, mine);
    cite!(["701.9a"]; assert_eq!(g.players[0].graveyard.len(), 1));
}

#[test]
fn r119_4_a_player_pays_life_only_with_a_life_total_at_least_that() {
    let mut g = Game::new(defs(), 2, 2);
    assert!(g.can_pay_life(0, 2));
    cite!(["119.4"]; assert!(!g.can_pay_life(0, 3)));
    assert!(!g.pay_life(0, 3).unwrap());
    assert_eq!(g.players[0].life, 2);
    assert!(g.pay_life(0, 2).unwrap(), "paying life is losing that much");
    cite!(["119.4"]; assert_eq!((g.players[0].life, g.log.last()), (0, Some(&Event::LifeLost { player: 0, amount: 2 }))));
    cite!(["119.4b"]; assert!(g.can_pay_life(0, 0), "0 life can always be paid (119.4b)"));
}

#[test]
fn r506_3_only_a_creature_attacks_or_blocks_and_only_a_player_planeswalker_or_battle_is_attacked() {
    let mut g = game();
    let land = g.put(1, MOUNTAIN, Zone::Battlefield);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    // what can be attacked: the players; a land or a creature cannot
    cite!(["506.3"]; assert_eq!(g.all_that(Verb::Attack, Role::To), vec![Target::Player(0), Target::Player(1)]));
    // what can be blocked, and what a creature is blocked by: creatures
    cite!(["506.3"]; assert_eq!(g.all_that(Verb::Block, Role::What), vec![object(bears)]));
    assert!(!g.takes(Verb::Block, Role::By, object(land)));
    // "a Wall" may be a creature; "a noncreature permanent" never is
    let permanent = |text: &str| CardDef::new("Test", "{1}{G}", "Creature — Bear", text, Some((2, 2))).is_ok();
    assert!(permanent("Test can't be blocked by Walls."));
    assert!(!permanent("Test can't be blocked by noncreature permanents."));
}
