//! An unpayable total cost is named with the changes that made
//! it (601.2f); a loss that ends the game keeps everything where it is (104.2a).
use crate::support::cite;
use game::{CardDef, Game, Move, Target, Zone};
use ir::Part;

fn game(players: usize, defs: Vec<CardDef>) -> Game {
    for d in &defs { game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0)); }
    let mut g = Game::new(defs, players, 20);
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn refusal(g: &mut Game, card: usize) -> String {
    let c = g.put(0, card, Zone::Hand);
    format!("{}", g.act(0, Move::Cast { card: c, targets: vec![] }).unwrap_err())
}

#[test]
fn review_an_increase_is_named_with_the_total() {
    let mut g = game(2, vec![
        CardDef::new("Taxer", "{0}", "Creature — Human", "Noncreature spells cost {1} more to cast.", Some((2, 1))).unwrap(),
        CardDef::new("Zap", "{1}{R}", "Instant", "You gain 1 life.", None).unwrap(),
    ]);
    g.put(1, 0, Zone::Battlefield);
    g.players[0].pool.colors[3] = 2;
    let e = refusal(&mut g, 1);
    println!("{e}");
    cite!(["601.2f"]; assert_eq!(e, "your mana pool cannot pay that cost (118.3a): {2}{R} in total, {1} more than it was (601.2f)"));
}

#[test]
fn review_a_reduction_that_reduces_nothing_is_not_said_to() {
    let mut g = game(2, vec![
        CardDef::new("Helper", "{0}", "Creature — Human", "Instant spells you cast cost {1} less to cast.", Some((2, 1))).unwrap(),
        CardDef::new("Zap", "{R}", "Instant", "You gain 1 life.", None).unwrap(),
    ]);
    g.put(0, 0, Zone::Battlefield);
    let e = refusal(&mut g, 1);
    println!("{e}");
    cite!(["118.3a"]; assert_eq!(e, "your mana pool cannot pay that cost (118.3a): {R}"));
    cite!(["118.7a"]; assert!(!e.contains("less"), "{{1}} less reduces nothing of {{R}} (118.7a): {e}"));
}

#[test]
fn review_a_simultaneous_double_loss_in_three_players_ends_the_game_keeping_objects() {
    let mut g = game(3, vec![
        CardDef::new("Drain", "{0}", "Sorcery", "Each opponent loses 20 life.", None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
    ]);
    let bears = [g.put(1, 1, Zone::Battlefield), g.put(2, 1, Zone::Battlefield)];
    let drain = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: drain, targets: vec![] }).unwrap();
    while !g.finished() { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["704.5a", "104.2a"]; assert_eq!(g.winner(), Some(0)));
    for b in bears { cite!(["104.2a"]; assert_eq!(g.obj(b).zone, Zone::Battlefield, "104.2a")); }
    let _ = Target::Player(0);
}
