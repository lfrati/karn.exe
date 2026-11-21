//! Damage to each member of an announced target group (115.4, 601.2c, 608.2b).
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Target, Zone};
fn setup() -> Game {
    let charm=CardDef::new("Prismari Charm","{U}{R}","Instant","Choose one —\n• Surveil 2, then draw a card.\n• Prismari Charm deals 1 damage to each of one or two targets.\n• Return target nonland permanent to its owner's hand.",None).unwrap();
    game::play::check(&charm).unwrap();
    let bear = CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap();
    let land = CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap();
    let mut g = Game::new(vec![charm, bear, land], 2, 20);
    g.players[0].pool.colors[ir::Color::Blue as usize] = 1;
    g.players[0].pool.colors[ir::Color::Red as usize] = 1;
    g
}
fn resolve(g: &mut Game) {
    for _ in 0..2 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
#[test]
fn r115_4_each_of_two_targets_is_one_group_with_damage_appropriate_candidates() {
    let mut g = setup();
    let bear = g.put(1, 1, Zone::Battlefield);
    let land = g.put(1, 2, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand);
    let cast = |targets| paid("UR", Move::Modes(vec![1], Box::new(Move::Cast { card, targets: vec![targets] })));
    for (bad, why) in [
        (vec![], "wrong number of targets: 0 chosen, it takes 1 to 2 (601.2c)"),
        (vec![Target::Player(1); 2], "the same target twice for one \"target\" (115.3)"),
        (vec![Target::Object(land)], "an illegal target, Island: it isn't a creature or a planeswalker or a battle or a player (115.1)"),
        (vec![Target::Player(0), Target::Player(1), Target::Object(bear)], "wrong number of targets: 3 chosen, it takes 1 to 2 (601.2c)"),
    ] {
        cite!(["115.4"]; case: why; refused(why): g.act(0, cast(bad)));
    }
    g.act(0, cast(vec![Target::Player(1), Target::Object(bear)])).unwrap();
    resolve(&mut g);
    cite!(["120.3a"]; assert_eq!(g.players[1].life, 19));
    cite!(["120.3e"]; assert_eq!(g.obj(bear).damage, 1));
}
#[test]
fn r608_2b_each_surviving_target_receives_the_full_stated_damage() {
    let mut g = setup();
    let bear = g.put(1, 1, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand);
    g.act(
        0,
        paid("UR", Move::Modes(
            vec![1],
            Box::new(Move::Cast { card, targets: vec![vec![Target::Player(1), Target::Object(bear)]] }),
        )),
    )
    .unwrap();
    g.move_object(bear, Zone::Graveyard);
    resolve(&mut g);
    cite!(["608.2b"]; assert_eq!(g.players[1].life, 19));
}
#[test]
fn r700_2_the_other_mode_retains_its_own_target_group() {
    let mut g = setup();
    let bear = g.put(1, 1, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand);
    cite!(["700.2c"]; ok: g.act(0, paid("UR", Move::Modes(vec![2], Box::new(Move::Cast { card, targets: vec![vec![Target::Object(bear)]] })))));
    resolve(&mut g);
    assert!(!g.battlefield.contains(&bear));
    assert_eq!(g.players[1].hand.len(), 1);
}

#[test]
fn r701_25_the_surveil_mode_resumes_into_its_draw_once() {
    use game::state::Answer;
    let mut g = setup();
    let lower = g.put(0, 1, Zone::Library);
    let upper = g.put(0, 2, Zone::Library);
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, paid("UR", Move::Modes(vec![0], Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    resolve(&mut g);
    cite!(["608.2c"]; assert!(g.players[0].hand.is_empty()));
    crate::support::refused(g.act(1, Move::Choose(Answer::Arrange { top: vec![lower], away: vec![upper] })), "that decision is another player's");
    let mut copy = g.clone();
    for (g, run) in [(&mut g, "original"), (&mut copy, "clone")] {
        g.act(0, Move::Choose(Answer::Arrange { top: vec![lower], away: vec![upper] })).unwrap();
        assert_eq!(g.players[0].hand.len(), 1);
        cite!(["701.25a"]; case: run; assert_eq!(g.obj(g.players[0].hand[0]).card, Some(1)));
        assert!(g.stack.is_empty());
    }
}
