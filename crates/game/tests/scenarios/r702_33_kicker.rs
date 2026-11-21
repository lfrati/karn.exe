//! Ordinary kicker reuses announcement, payment and linked cast facts (702.33a,d,e).
use crate::support::cite;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use game::state::Answer;

fn gatekeeper() -> CardDef {
    CardDef::new("Gatekeeper of Malakir", "{B}{B}", "Creature — Vampire Warrior", "Kicker {B}\nWhen this creature enters, if it was kicked, target player sacrifices a creature of their choice.", Some((2,2))).unwrap()
}
fn game() -> Game {
    let defs = vec![gatekeeper(), CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2,2))).unwrap()];
    for d in &defs { game::play::check(d).unwrap(); }
    Game::new(defs, 2, 20)
}
fn cast(g: &mut Game, card: ObjId, kicks: u32, spending: &str) -> Result<(), String> {
    g.act(0, crate::support::paid(spending,Move::WithKicks(kicks, Box::new(Move::Cast { card, targets: vec![] }))))
}
fn pass(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
fn black(g: &mut Game, n: u32) { g.players[0].pool.colors[ir::Color::Black as usize] = n; }

#[test]
fn r702_33a_kicker_is_optional_and_paid_once_as_an_additional_cost() {
    let mut g = game();
    let card = g.put(0, 0, Zone::Hand);
    black(&mut g, 2);
    cite!(["702.33a"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): cast(&mut g, card, 1, "BB"));
    assert_eq!(g.obj(card).zone, Zone::Hand);
    assert_eq!(g.players[0].pool.total(), 2);
    black(&mut g, 5);
    cite!(["702.33a"]; refused("a kicker cost can be paid only once (702.33a)"): cast(&mut g, card, 2, "BBB"));
    cast(&mut g, card, 0, "BB").unwrap();
    cite!(["702.33a"]; assert_eq!(g.players[0].pool.total(), 3));
    pass(&mut g);
    cite!(["603.4"]; assert!(g.stack.is_empty(), "without kicker the intervening-if entry trigger does not trigger"));
    assert!(g.decision.is_none(), "there must not be a trigger waiting for its target");
}

#[test]
fn r702_33e_kicked_entry_trigger_keeps_the_fact_and_asks_the_target_to_sacrifice() {
    let mut g = game();
    let bear = g.put(1, 1, Zone::Battlefield);
    g.put(1, 1, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand);
    black(&mut g, 3);
    cast(&mut g, card, 1, "BBB").unwrap();
    let spell = *g.stack.last().unwrap();
    cite!(["702.33d", "118.8d"]; assert_eq!((g.obj(spell).kicked, g.obj(spell).chars.mana_value), (1,2)));
    pass(&mut g);
    crate::support::refused(g.act(1, Move::ChooseTargets(vec![vec![Target::Player(1)]])), "that decision is another player's");
    g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap();
    pass(&mut g);
    let mut copy = g.clone();
    for game in [&mut g, &mut copy] {
        crate::support::refused(game.act(0, Move::Choose(Answer::Objects(vec![bear]))), "that decision is another player's");
        game.act(1, Move::Choose(Answer::Objects(vec![bear]))).unwrap();
        cite!(["702.33e"]; assert!(!game.battlefield.contains(&bear)));
        assert_eq!(game.players[0].pool.total(), 0, "resumption does not repay the kicker");
        assert_eq!(game.players[1].graveyard.len(), 1);
    }
}

#[test]
fn r601_2b_kicker_announcement_requires_its_full_payment() {
    for (mana, kicks, accepted) in [(3, 0, true), (3, 1, true), (2, 0, true), (2, 1, false)] {
        let mut g = game();
        let card = g.put(0, 0, Zone::Hand);
        black(&mut g, mana);
        let result = cast(&mut g, card, kicks, if kicks == 0 || mana == 2 { "BB" } else { "BBB" });
        cite!(["702.33a", "601.2h"]; assert_eq!(result.is_ok(), accepted, "mana={mana}, kicks={kicks}: {result:?}"));
        if accepted { assert_eq!(g.players[0].pool.total(), mana - 2 - kicks); }
        else { assert_eq!(g.obj(card).zone, Zone::Hand); assert_eq!(g.players[0].pool.total(), mana); }
    }
}

#[test]
fn r400_7_kicker_is_not_carried_through_a_later_zone_change() {
    let d=CardDef::new("Kicked Creature", "{0}", "Creature — Bear", "Kicker {1}\nWhen this creature enters, if it was kicked, you gain 1 life.", Some((2,2))).unwrap();
    game::play::check(&d).unwrap();
    let mut g=Game::new(vec![d],2,20);
    let card=g.put(0,0,Zone::Hand);
    g.players[0].pool.colorless=1;
    cast(&mut g,card,1,"C").unwrap();
    pass(&mut g); pass(&mut g);
    cite!(["702.33e"]; assert_eq!(g.players[0].life,21));
    let permanent=g.battlefield[0];
    g.move_object(permanent,Zone::Hand);
    let card=g.players[0].hand[0];
    cast(&mut g,card,0,"").unwrap(); pass(&mut g);
    assert!(g.stack.is_empty());
    cite!(["400.7"]; assert_eq!(g.players[0].life,21));
}

#[test]
fn r702_33f_multiple_kicker_costs_are_refused_until_individually_announced() {
    let d=CardDef::new("Double Kicker", "{0}", "Creature — Bear", "Kicker {1}\nKicker {2}", Some((2,2))).unwrap();
    assert!(game::play::check(&d).unwrap_err().0.contains("multiple kicker costs"));
}
