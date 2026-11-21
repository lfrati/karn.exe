//! Phyrexian mana (107.4f): each such symbol of a cost is paid with one mana of its color or
//! with 2 life. Which is said as the spell or ability is proposed and locked in (118.13a,
//! 601.2b); life is paid only by a player who has that much (119.4).
use crate::support::{cite,paid as spend,mana_pool};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const DISMEMBER: usize = 0;
const BEAR: usize = 1;
const SWAMP: usize = 2;
const ENGINE: usize = 3;
const FOREST: usize = 4;
const MIXED: usize = 5;

fn game() -> Game {
    let defs = vec![
        CardDef::new("Dismember", "{1}{B/P}{B/P}", "Instant", "({B/P} can be paid with either {B} or 2 life.)\nTarget creature gets -5/-5 until end of turn.", None).unwrap(),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Swamp", "", "Basic Land — Swamp", "", None).unwrap(),
        CardDef::new("Engine", "{3}{G/P}", "Artifact", "({G/P} can be paid with either {G} or 2 life.)\n{1}{G/P}, {T}: You gain 1 life.", None).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Mixed", "{W/P}{U/P}", "Sorcery", "You gain 1 life.", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    Game::new(defs, 2, 20)
}
fn pass_all(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn with_life(life: &[bool], mv: Move) -> Move { Move::WithLife(life.to_vec(), Box::new(mv)) }
/// Dismember in hand and an opposing Bear to target.
fn dismember() -> (Game, ObjId, Move) {
    let mut g = game();
    let bear = g.put(1, BEAR, Zone::Battlefield);
    let card = g.put(0, DISMEMBER, Zone::Hand);
    (g, card, Move::Cast { card, targets: vec![vec![Target::Object(bear)]] })
}
#[test]
fn each_symbol_is_paid_with_mana_of_its_color_or_two_life() {
    // mana for both: {1}{B}{B}, said or not
    for said in [None, Some([false, false])] {
        let (mut g, _, cast) = dismember();
        g.players[0].pool.colors[2] = 3;
        g.act(0, spend("BBB",said.map_or(cast.clone(), |s| with_life(&s, cast)))).unwrap();
        cite!(["107.4f"]; case: format!("{said:?}"); assert_eq!((g.players[0].life, g.players[0].pool.colors[2]), (20, 0)));
        pass_all(&mut g);
        assert!(g.battlefield.is_empty(), "the Bear died");
    }
    // life for one: {1}{B} and 2 life
    let (mut g, _, cast) = dismember();
    g.players[0].pool.colors[2] = 2;
    let before = format!("{g:?}");
    cite!(["107.4f"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0, spend("BB",cast.clone())));
    assert_eq!(format!("{g:?}"),before);
    g.act(0, spend("BB",with_life(&[true, false], cast))).unwrap();
    cite!(["107.4f"]; assert_eq!((g.players[0].life, g.players[0].pool.colors[2]), (18, 0)));
    // life for both: {1} of any mana and 4 life
    let (mut g, _, cast) = dismember();
    g.players[0].pool.colors[4] = 1;
    g.act(0, spend("G",with_life(&[true, true], cast))).unwrap();
    cite!(["107.4f"]; assert_eq!((g.players[0].life, g.players[0].pool.colors[4]), (16, 0)));
    pass_all(&mut g);
    assert!(g.battlefield.is_empty());
    // the mana is of the symbol's color: green does not pay {B/P}
    let (mut g, _, cast) = dismember();
    g.players[0].pool.colors[4] = 3;
    let before = format!("{g:?}");
    cite!(["107.4f"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0, spend("GGG",cast)));
    assert_eq!(format!("{g:?}"),before);
}

#[test]
fn what_is_said_is_checked_and_a_refusal_changes_nothing() {
    let (mut g, _, cast) = dismember();
    g.players[0].pool.colors[2] = 3;
    let before = format!("{g:?}");
    cite!(["107.4f", "601.2b"]; refused("that cost has 2 Phyrexian mana symbols: say for each whether life is paid (107.4f, 601.2b)"): g.act(0, with_life(&[true], cast.clone())));
    assert_eq!(format!("{g:?}"), before, "wrong answer count must not alter the state");
    cite!(["601.2b"]; refused("say how Phyrexian mana is paid only once (601.2b)"): g.act(0, with_life(&[true, true], with_life(&[false, false], cast.clone()))));
    assert_eq!(format!("{g:?}"), before, "duplicate announcements must not alter the state");
    // life a player does not have cannot be paid (119.4)
    g.players[0].life = 3;
    let before_low = format!("{g:?}");
    cite!(["119.4"]; refused("you can't pay 4 life with a life total of 3 (119.4)"): g.act(0, with_life(&[true, true], cast.clone())));
    assert_eq!(format!("{g:?}"), before_low);
    g.act(0, spend("BB",with_life(&[true, false], cast))).unwrap();
    cite!(["119.4"]; assert_eq!(g.players[0].life, 1));
    assert_ne!(format!("{g:?}"), before);
    // a cost without such symbols takes no such answer
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Hand);
    g.players[0].pool.colors[4] = 2;
    cite!(["107.4f"]; refused("that cost has 0 Phyrexian mana symbols: say for each whether life is paid (107.4f, 601.2b)"): g.act(0, with_life(&[true], Move::Cast { card: bear, targets: vec![] })));
}

#[test]
fn supplied_phyrexian_announcements_obey_mana_and_life_resources() {
    for (black, life) in [(0, 20), (1, 20), (2, 20), (3, 20), (3, 3)] {
        for paid in [[false, false], [true, false], [false, true], [true, true]] {
            let (mut g, _, cast) = dismember();
            g.players[0].pool.colors[2] = black;
            g.players[0].life = life;
            let count = paid.iter().filter(|p| **p).count() as u32;
            let expected = black >= 3 - count && life >= 2 * count as i32;
            cite!(["107.4f", "119.4"]; case: format!("{black}/{life}/{paid:?}"); assert_eq!(g.act(0, spend(&"B".repeat((3-count)as usize),with_life(&paid, cast))).is_ok(), expected));
        }
    }
    // a listed plan taps for what is still mana
    let mut g = game();
    let bear = g.put(1, BEAR, Zone::Battlefield);
    let card = g.put(0, DISMEMBER, Zone::Hand);
    let swamp = g.put(0, SWAMP, Zone::Battlefield);
    let plan = vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source: swamp, ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None }];
    g.act(0, spend("B",with_life(&[true, true], Move::WithMana(plan, Box::new(Move::Cast { card, targets: vec![vec![Target::Object(bear)]] }))))).unwrap();
    assert!(g.obj(swamp).tapped);
    assert_eq!(g.players[0].life, 16);
    // The white and blue symbols each have their own explicit life decision.
    for blue in [0, 1] {
        for paid in [[false, false], [true, false], [false, true], [true, true]] {
            let mut g = game();
            let card = g.put(0, MIXED, Zone::Hand);
            g.players[0].pool.colors[0] = 1;
            g.players[0].pool.colors[1] = blue;
            cite!(["107.4f"]; case: format!("{blue}/{paid:?}"); assert_eq!(g.act(0, Move::Paying(mana_pool(&format!("{}{}",if paid[0]{""}else{"W"},if paid[1]{""}else{"U"})),Box::new(with_life(&paid, Move::Cast { card, targets: vec![] })))).is_ok(), blue == 1 || paid[1]));
        }
    }
}

#[test]
fn an_activation_cost_is_paid_the_same_way() {
    let mut g = game();
    let engine = g.put(0, ENGINE, Zone::Battlefield);
    let forest = g.put(0, FOREST, Zone::Battlefield);
    let activate = Move::Activate { source: engine, ability: 0, targets: vec![], paying: vec![] };
    let before = format!("{g:?}");
    cite!(["107.4f"]; refused("that mana is not in your mana pool (118.3a)"): g.act(0, spend("GG",activate.clone())));
    assert_eq!(format!("{g:?}"),before);
    let tap = vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source: forest, ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None }];
    g.act(0, spend("G",with_life(&[true], Move::WithMana(tap, Box::new(activate))))).unwrap();
    assert!(g.obj(forest).tapped && g.obj(engine).tapped);
    cite!(["107.4f"]; assert_eq!(g.players[0].life, 18));
    pass_all(&mut g);
    assert_eq!(g.players[0].life, 19);

    // the spell itself: {3} and 2 life
    let mut g = game();
    let card = g.put(0, ENGINE, Zone::Hand);
    g.players[0].pool.colors[2] = 3;
    g.act(0, spend("BBB",with_life(&[true], Move::Cast { card, targets: vec![] }))).unwrap();
    cite!(["107.4f"]; assert_eq!(g.players[0].life, 18));
    let spell = *g.stack.last().unwrap();
    cite!(["202.3"]; assert_eq!(g.obj(spell).chars.mana_value, 4, "a Phyrexian symbol counts 1 however it is paid (202.3)"));
}
