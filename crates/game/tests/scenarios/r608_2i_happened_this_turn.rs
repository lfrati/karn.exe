//! Conditions that look back in the turn (608.2i): "if a creature died this turn", "if you've
//! cast another spell this turn". What happened is noted as it happens and asked about for
//! the rest of that turn only, however long ago its events were delivered.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const DIED: usize = 0;
const TWO: usize = 1;
const NONE: usize = 2;
const ANOTHER: usize = 3;
const THEIRS: usize = 4;
const THREE: usize = 5;
const MURDER: usize = 6;
const BEAR: usize = 7;
const SCAVENGER: usize = 8;
const LAND: usize = 9;
const RELIC: usize = 10;
const SHATTER: usize = 11;
const EXILE: usize = 12;

fn game() -> Game {
    let defs = vec![
        CardDef::new("Died", "{B}", "Instant", "If a creature died this turn, you gain 3 life.", None).unwrap(),
        CardDef::new("Two", "{B}", "Instant", "If two or more creatures died this turn, you gain 3 life.", None).unwrap(),
        CardDef::new("None", "{B}", "Instant", "If no creatures died this turn, you gain 3 life.", None).unwrap(),
        CardDef::new("Another", "{B}", "Instant", "If you've cast another spell this turn, you gain 3 life.", None).unwrap(),
        CardDef::new("Theirs", "{B}", "Instant", "If an opponent cast two or more spells this turn, you gain 3 life.", None).unwrap(),
        CardDef::new("Three", "{B}", "Instant", "If you cast three or more spells this turn, you gain 3 life.", None).unwrap(),
        CardDef::new("Death probe", "{B}", "Instant", "Destroy target creature.", None).unwrap(),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Hollowhenge Scavenger", "{3}{G}{G}", "Creature — Elemental", "Morbid — When this creature enters, if a creature died this turn, you gain 5 life.", Some((4, 5))).unwrap(),
        CardDef::new("Wastes", "", "Land", "", None).unwrap(),
        CardDef::new("Relic", "{1}", "Artifact", "", None).unwrap(),
        CardDef::new("Shatter", "{B}", "Instant", "Destroy target artifact.", None).unwrap(),
        CardDef::new("Exile", "{B}", "Instant", "Exile target creature.", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    let mut g = Game::new(defs, 2, 20);
    for player in 0..2 { for _ in 0..8 { g.put(player, LAND, Zone::Library); } }
    g
}
fn pass_all(g: &mut Game) {
    for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); }
}
/// This player casts this card with mana given for it, and it resolves.
fn resolve(g: &mut Game, player: usize, card: usize, targets: Vec<Vec<Target>>, spending: &str) -> ObjId {
    let spell = g.put(player, card, Zone::Hand);
    g.players[player].pool.colors = [0, 0, 5, 0, 5];
    while g.priority != player { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(player, paid(spending, Move::Cast { card: spell, targets })).unwrap();
    pass_all(g);
    g.players[player].pool = Default::default();
    spell
}
fn kill(g: &mut Game, by: usize, whose: usize) {
    let bear = g.put(whose, BEAR, Zone::Battlefield);
    resolve(g, by, MURDER, vec![vec![Target::Object(bear)]], "B");
    assert!(!g.battlefield.contains(&bear));
}
fn next_turn(g: &mut Game) {
    let turn = g.turn;
    while g.turn == turn { g.act(g.priority, Move::Pass).unwrap(); }
}
fn gains(g: &mut Game, card: usize) -> i32 {
    let life = g.players[0].life;
    resolve(g, 0, card, vec![], "B");
    g.players[0].life - life
}

#[test]
fn a_creature_died_this_turn_whoever_controlled_it_and_however_long_ago() {
    let mut g = game();
    assert_eq!(gains(&mut g, DIED), 0);
    kill(&mut g, 0, 1);
    cite!(["608.2i"]; assert_eq!(gains(&mut g, DIED), 3));
    // delivered events are retired and the dead creature reclaimed: it still died this turn
    g.collect_departed();
    cite!(["608.2i"]; assert_eq!(gains(&mut g, DIED), 3));
    assert_eq!(gains(&mut g, TWO), 0);
    assert_eq!(gains(&mut g, NONE), 0);
    kill(&mut g, 0, 0);
    cite!(["608.2i"]; assert_eq!(gains(&mut g, TWO), 3));
    // another turn: nothing has died in it
    next_turn(&mut g);
    cite!(["608.2i"]; assert_eq!(gains(&mut g, DIED), 0));
    assert_eq!(gains(&mut g, NONE), 3);
}

#[test]
fn only_a_creature_that_dies_counts() {
    // an artifact put into a graveyard did not die; a creature exiled did not die (700.4)
    let mut g = game();
    let relic = g.put(1, RELIC, Zone::Battlefield);
    resolve(&mut g, 0, SHATTER, vec![vec![Target::Object(relic)]], "B");
    assert!(!g.battlefield.contains(&relic));
    cite!(["700.4"]; assert_eq!(gains(&mut g, DIED), 0));
    let bear = g.put(1, BEAR, Zone::Battlefield);
    resolve(&mut g, 0, EXILE, vec![vec![Target::Object(bear)]], "B");
    assert!(!g.battlefield.contains(&bear));
    cite!(["700.4"]; assert_eq!(gains(&mut g, DIED), 0));
    assert_eq!(gains(&mut g, NONE), 3);
}

#[test]
fn morbid_is_asked_as_the_creature_enters_and_as_its_ability_resolves() {
    let mut g = game();
    let life = g.players[0].life;
    resolve(&mut g, 0, SCAVENGER, vec![], "GGGGG");
    cite!(["603.4"]; assert!(g.stack.is_empty(), "no creature died: the ability did not trigger (603.4)"));
    assert_eq!(g.players[0].life, life);
    kill(&mut g, 0, 1);
    resolve(&mut g, 0, SCAVENGER, vec![], "GGGGG");
    cite!(["603.4", "608.2i"]; assert_eq!(g.stack.len(), 1, "it triggered"));
    pass_all(&mut g);
    cite!(["603.4"]; assert_eq!(g.players[0].life, life + 5));
}

#[test]
fn spells_cast_this_turn_are_counted_for_the_player_who_cast_them() {
    let mut g = game();
    // the spell asking is itself a spell cast this turn, but not another one
    assert_eq!(gains(&mut g, ANOTHER), 0);
    cite!(["608.2i"]; assert_eq!(gains(&mut g, ANOTHER), 3, "the first one was another"));
    cite!(["608.2i"]; assert_eq!(gains(&mut g, THREE), 3, "this is the third"));
    assert_eq!(gains(&mut g, THEIRS), 0, "an opponent cast none");
    resolve(&mut g, 1, DIED, vec![], "B");
    assert_eq!(gains(&mut g, THEIRS), 0, "one");
    resolve(&mut g, 1, DIED, vec![], "B");
    cite!(["608.2i"]; assert_eq!(gains(&mut g, THEIRS), 3, "two"));
    next_turn(&mut g);
    cite!(["608.2i"]; assert_eq!(gains(&mut g, ANOTHER), 0));
    assert_eq!(gains(&mut g, THREE), 0);
}

#[test]
fn a_copy_of_the_game_remembers_the_same() {
    let mut g = game();
    kill(&mut g, 0, 1);
    let mut copy = g.clone();
    cite!(["608.2i"]; assert_eq!(gains(&mut g, DIED), 3));
    cite!(["608.2i"]; assert_eq!(gains(&mut copy, DIED), 3));
    assert_eq!(format!("{g:?}"), format!("{copy:?}"));
}
