//! Exile is a cost action too (118.3, 601.2h); paid objects can be found in public zones (400.7j).
use crate::support::{cite,paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

fn game(text: &str, creature: bool) -> Game {
    let defs = vec![
        CardDef::new("Source", "{1}", if creature { "Creature — Spirit" } else { "Artifact" }, text, creature.then_some((3, 3))).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Denial", "{0}", "Instant", "Counter target activated ability.", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0)); }
    Game::new(defs, 3, 20)
}

fn activate(g: &mut Game, source: ObjId, targets: Vec<Vec<Target>>, paying: Vec<Vec<ObjId>>) {
    g.act(0, Move::Activate { source, ability: 0, targets, paying }).unwrap();
}

fn resolve(g: &mut Game) { for _ in 0..3 { g.act(g.priority, Move::Pass).unwrap(); } }

#[test]
fn r118_3_exiling_a_source_pays_the_cost_before_its_ability_resolves() {
    let mut g = game("{4}, {T}, Exile this artifact: Exile target creature.", false);
    let source = g.put(0, 0, Zone::Battlefield);
    let bear = g.put(1, 1, Zone::Battlefield);
    g.players[0].pool.colorless = 3;
    let before = format!("{g:?}");
    cite!(["601.2h", "118.3"]; refused("your mana pool cannot pay that cost (118.3a): {4}"): g.act(0, Move::Activate { source, ability: 0, targets: vec![vec![Target::Object(bear)]], paying: vec![] }));
    assert_eq!(format!("{g:?}"), before, "an unpaid cost makes no partial payment");
    g.players[0].pool.colorless = 4;
    g.act(0,paid("CCCC",Move::Activate {source,ability:0,targets:vec![vec![Target::Object(bear)]],paying:vec![]})).unwrap();
    cite!(["601.2h"]; assert_eq!(g.obj(source).zone, Zone::Gone));
    assert_eq!(g.exile.len(), 1);
    cite!(["601.2h"]; assert_eq!(g.obj(bear).zone, Zone::Battlefield));
    assert_eq!(g.players[0].pool.total(), 0);
    assert_eq!(g.stack.len(), 1);
    resolve(&mut g);
    cite!(["113.7a"]; assert_eq!(g.obj(bear).zone, Zone::Gone));
    assert_eq!(g.exile.len(), 2);
}

#[test]
fn r701_6b_countering_an_ability_does_not_return_its_exiled_cost() {
    let mut g = game("Exile this artifact: Exile target creature.", false);
    let source = g.put(0, 0, Zone::Battlefield);
    let bear = g.put(1, 1, Zone::Battlefield);
    activate(&mut g, source, vec![vec![Target::Object(bear)]], vec![]);
    let ability = g.stack[0];
    let denial = g.put(0, 2, Zone::Hand);
    g.act(0, Move::Cast { card: denial, targets: vec![vec![Target::Object(ability)]] }).unwrap();
    resolve(&mut g);
    assert!(g.stack.is_empty());
    cite!(["701.6b"]; assert_eq!(g.exile.len(), 1));
    assert_eq!(g.obj(bear).zone, Zone::Battlefield);
}

#[test]
fn r118_10_the_same_object_cannot_pay_exile_and_sacrifice_costs_together() {
    let mut g = game("Exile this artifact, Sacrifice this artifact: You gain 1 life.", false);
    let source = g.put(0, 0, Zone::Battlefield);
    let before = format!("{g:?}");
    cite!(["118.10"]; refused("Source pays for one thing (118.10)"): g.act(0, Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }));
    assert_eq!(format!("{g:?}"), before);
}

#[test]
fn r601_2h_chosen_exile_costs_use_the_same_membership_and_payment_checks() {
    let mut g = game("Exile a creature you control: You gain 1 life.", false);
    let source = g.put(0, 0, Zone::Battlefield);
    let mine = g.put(0, 1, Zone::Battlefield);
    let theirs = g.put(1, 1, Zone::Battlefield);
    let before = format!("{g:?}");
    cite!(["601.2h"]; refused("Bear is not what that cost asks for"): g.act(0, Move::Activate { source, ability: 0, targets: vec![], paying: vec![vec![theirs]] }));
    assert_eq!(format!("{g:?}"), before);
    activate(&mut g, source, vec![], vec![vec![mine]]);
    cite!(["601.2h"]; assert_eq!((g.obj(mine).zone, g.obj(theirs).zone), (Zone::Gone, Zone::Battlefield)));
    assert_eq!(g.players[0].life, 20);
    resolve(&mut g);
    assert_eq!(g.players[0].life, 21);
}

#[test]
fn r400_7j_an_ability_can_find_the_new_object_moved_to_a_public_zone_by_its_cost() {
    for cost in ["Exile", "Sacrifice"] {
        for moved_again in [false, true] {
            let mut g = game(&format!("{cost} this artifact: Return it to the battlefield."), false);
            let source = g.put(0, 0, Zone::Battlefield);
            activate(&mut g, source, vec![], vec![]);
            let paid = if cost == "Exile" { g.exile[0] } else { g.players[0].graveyard[0] };
            assert_ne!(paid, source);
            if moved_again {
                g.move_object(paid, Zone::Hand);
                let hand = g.players[0].hand[0];
                g.move_object(hand, if cost == "Exile" { Zone::Exile } else { Zone::Graveyard });
            }
            resolve(&mut g);
            cite!(["400.7j", "400.7"]; case: format!("{cost}, moved again: {moved_again}"); assert_eq!(g.battlefield.len(), usize::from(!moved_again)));
            if !moved_again { assert_ne!(g.battlefield[0], paid); }
        }
    }
}

#[test]
fn r608_2h_exiling_the_damage_source_as_a_cost_preserves_its_last_known_power() {
    let mut g = game("Exile this creature: It deals damage equal to its power to target player.", true);
    let source = g.put(0, 0, Zone::Battlefield);
    g.put_counters(source, "+1/+1", 1);
    activate(&mut g, source, vec![vec![Target::Player(1)]], vec![]);
    assert_eq!(g.power(g.exile[0]), Some(3), "the new card has printed power, without the old counter");
    resolve(&mut g);
    cite!(["608.2h"]; assert_eq!(g.players[1].life, 16, "use the old permanent's four power"));
}

#[test]
fn r118_3_exile_costs_from_hand_use_owned_candidates_and_library_selection_remains_refused() {
    let mut g = game("Exile a card from your hand: You gain 1 life.", false);
    let source = g.put(0, 0, Zone::Battlefield);
    let mine = g.put(0, 1, Zone::Hand); let theirs = g.put(1, 1, Zone::Hand);
    let before = format!("{g:?}");
    cite!(["601.2h"]; refused("Bear is not what that cost asks for"): g.act(0, Move::Activate {source,ability:0,targets:vec![],paying:vec![vec![theirs]]}));
    assert_eq!(format!("{g:?}"),before);
    activate(&mut g,source,vec![],vec![vec![mine]]);resolve(&mut g);
    cite!(["118.3"]; assert_eq!(g.players[0].life,21)); assert_eq!(g.exile.len(),1);assert_eq!(g.players[1].hand,vec![theirs]);
    let d=CardDef::new("Library cost","{0}","Artifact","Exile a card from your library: You gain 1 life.",None).unwrap();
    assert!(game::play::check(&d).unwrap_err().0.contains("library"));
}

#[test]
fn r400_7j_a_mana_ability_finds_the_card_exiled_by_its_cost_without_using_the_stack() {
    let mut g = game("Exile this artifact: Add {G}. Return it to the battlefield.", false);
    let source = g.put(0, 0, Zone::Battlefield);
    activate(&mut g, source, vec![], vec![]);
    cite!(["605.3b"]; assert!(g.stack.is_empty()));
    assert!(g.exile.is_empty());
    assert_eq!(g.players[0].pool.total(), 1);
    cite!(["400.7j"]; assert_eq!(g.battlefield.len(), 1));
    assert_ne!(g.battlefield[0], source);
}
