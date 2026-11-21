//! Class levels: Wizard and Ranger Class, a level kept while abilities are
//! lost (716.2b), a copy of a leveled Class (716.2b, 716.2d) and bars out of order (716.2a).
use crate::support::{cite, paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::{Ability, Part};

fn real(name: &str) -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let d = CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
    d
}
fn c(name: &str, cost: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    let d = CardDef::new(name, cost, types, text, pt).expect(name);
    game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
    d
}
/// `defs`, then an Island at the end; player 0's precombat main phase.
fn game(mut defs: Vec<CardDef>) -> Game {
    defs.push(CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap());
    let island = defs.len() - 1;
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..12 { g.put(p, island, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn mana(g: &mut Game) { g.players[0].pool.colors = [9; 5]; g.players[0].pool.colorless = 9; }
/// Resolve the stack, aiming any target at `at`.
fn resolve(g: &mut Game, at: Option<ObjId>) {
    for _ in 0..40 {
        match g.decision.clone() {
            None if g.stack.is_empty() => return,
            None => g.act(g.priority, Move::Pass).unwrap(),
            Some(Decision::Order { player, abilities }) => g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(),
            Some(Decision::Targets { player, .. }) => g.act(player, Move::ChooseTargets(vec![vec![Target::Object(at.expect("a target"))]])).unwrap(),
            Some(Decision::Choose { player, choice: Choice::May }) => g.act(player, Move::Choose(Answer::Yes)).unwrap(),
            Some(d) => panic!("{d:?}"),
        }
    }
}
/// Its level bars, by the activated abilities they are, in order: level 2's, then level 3's.
fn bars(g: &Game, o: ObjId) -> Vec<usize> {
    g.abilities(o).iter().enumerate().filter(|(_, a)| matches!(&***a, Ability::Activated { .. })).map(|(i, _)| i).collect()
}
fn level_up(g: &mut Game, o: ObjId, level: usize, units: &str) -> Result<(), String> {
    let Some(&ability) = bars(g, o).get(level - 2) else { return Err(format!("no bar for level {level}")) };
    g.act(0, paid(units, Move::Activate { source: o, ability, targets: vec![], paying: vec![] }))
}
fn enter(g: &mut Game, def: usize, units: &str) -> ObjId {
    let card = g.put(0, def, Zone::Hand);
    mana(g);
    g.act(0, paid(units, Move::Cast { card, targets: vec![] })).unwrap();
    resolve(g, None);
    *g.battlefield.iter().rev().find(|o| g.obj(**o).card == Some(def)).unwrap()
}

/// Wizard Class: level 2 draws two as it becomes level 2; level 3 puts a counter on a
/// creature whenever its controller draws. A bar already gained can't be gained again, and
/// level 3's can't be gained before level 2's (716.2a, ruling 0748b4e3).
#[test]
fn r716_2a_wizard_class_draws_two_at_level_two_and_counts_draws_at_level_three() {
    let mut g = game(vec![real("Wizard Class"), c("Bear", "{0}", "Creature — Bear", "", Some((2, 2))), c("Peek", "{0}", "Sorcery", "Draw a card.", None)]);
    let bear = g.put(0, 1, Zone::Battlefield);
    let class = enter(&mut g, 0, "U");
    cite!(["716.2a"]; refused("this Class becomes level 3 only from level 2 (716.2a)"): level_up(&mut g, class, 3, "CCCCU"));
    let hand = g.players[0].hand.len();
    mana(&mut g);
    level_up(&mut g, class, 2, "CCU").unwrap();
    resolve(&mut g, None);
    cite!(["716.2a"]; assert_eq!(g.obj(class).level, Some(2)));
    cite!(["716.2a"]; assert_eq!(g.players[0].hand.len(), hand + 2, "When this Class becomes level 2, draw two cards"));
    mana(&mut g);
    cite!(["716.2a"]; refused("this Class becomes level 2 only from level 1 (716.2a)"): level_up(&mut g, class, 2, "CCU"));
    level_up(&mut g, class, 3, "CCCCU").unwrap();
    resolve(&mut g, None);
    assert_eq!(g.obj(class).level, Some(3));
    let peek = g.put(0, 2, Zone::Hand);
    g.act(0, Move::Cast { card: peek, targets: vec![] }).unwrap();
    resolve(&mut g, Some(bear));
    cite!(["716.2a"]; assert_eq!(g.power(bear), Some(3), "a +1/+1 counter for the card drawn"));
}

/// A Class whose abilities are lost keeps its level (716.2b): with "lose all abilities" gone,
/// its level 3 bar can be gained, and level 2's "becomes level 2" doesn't trigger again.
#[test]
fn r716_2b_a_class_that_loses_its_abilities_keeps_its_level() {
    let mut g = game(vec![real("Wizard Class"), c("Hush", "{0}", "Enchantment", "Other enchantments lose all abilities.", None)]);
    let class = enter(&mut g, 0, "U");
    mana(&mut g);
    level_up(&mut g, class, 2, "CCU").unwrap();
    resolve(&mut g, None);
    let hush = g.put(0, 1, Zone::Battlefield);
    cite!(["613.1f"]; assert!(bars(&g, class).is_empty(), "no bars while it has no abilities"));
    cite!(["716.2b"]; assert_eq!(g.obj(class).level, Some(2)));
    g.destroy(hush);
    let hand = g.players[0].hand.len();
    mana(&mut g);
    cite!(["716.2a"]; refused("this Class becomes level 2 only from level 1 (716.2a)"): level_up(&mut g, class, 2, "CCU")); // still level 2
    level_up(&mut g, class, 3, "CCCCU").unwrap();
    resolve(&mut g, None);
    cite!(["716.2b"]; assert_eq!(g.obj(class).level, Some(3)));
    assert_eq!(g.players[0].hand.len(), hand, "no level 2 trigger");
}

/// Copy Enchantment entering as a copy of a level 3 Talent copies no level (716.2b): it is
/// level 1 (716.2d), so it has its level 2 bar and not level 3's ability.
#[test]
fn r716_2b_a_copy_of_a_leveled_class_is_level_one() {
    let mut g = game(vec![real("Stormchaser's Talent"), real("Copy Enchantment"), c("Ping", "{0}", "Instant", "Ping deals 1 damage to any target.", None)]);
    let talent = enter(&mut g, 0, "U");
    for level in [2, 3] {
        mana(&mut g);
        level_up(&mut g, talent, level, if level == 2 { "CCCU" } else { "CCCCCU" }).unwrap();
        resolve(&mut g, None);
    }
    assert_eq!(g.obj(talent).level, Some(3));
    let card = g.put(0, 1, Zone::Hand);
    mana(&mut g);
    g.act(0, paid("CCU", Move::Cast { card, targets: vec![] })).unwrap();
    for _ in 0..20 {
        match g.decision.clone() {
            Some(Decision::Choose { player, choice: Choice::Objects { .. } }) => g.act(player, Move::Choose(Answer::Objects(vec![talent]))).unwrap(),
            Some(Decision::Choose { player, .. }) => g.act(player, Move::Choose(Answer::Objects(vec![talent]))).unwrap(),
            None if g.stack.is_empty() => break,
            None => g.act(g.priority, Move::Pass).unwrap(),
            Some(Decision::Order { player, abilities }) => g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(),
            Some(d) => panic!("{d:?}"),
        }
    }
    let copy = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(1)).expect("the copy");
    assert_eq!(g.obj(copy).chars.name, "Stormchaser's Talent");
    cite!(["716.2b"]; assert_eq!(g.obj(copy).level, None, "levels are not copiable"));
    let otters = |g: &Game| g.battlefield.iter().filter(|o| g.obj(**o).chars.name == "Otter Token").count();
    let before = otters(&g);
    let ping = g.put(0, 2, Zone::Hand);
    g.act(0, Move::Cast { card: ping, targets: vec![vec![Target::Player(1)]] }).unwrap();
    resolve(&mut g, None);
    cite!(["716.2d"]; assert_eq!(otters(&g), before + 1, "only the original's level 3 ability makes an Otter"));
    mana(&mut g);
    cite!(["716.2d", "716.2a"]; ok: level_up(&mut g, copy, 2, "CCCU"));
}

/// Ranger Class level 3: "You may cast creature spells from the top of your library."
#[test]
fn r716_2a_ranger_class_level_three_casts_creatures_from_the_top() {
    let mut g = game(vec![real("Ranger Class"), c("Bear", "{0}", "Creature — Bear", "", Some((2, 2)))]);
    let class = enter(&mut g, 0, "CG");
    let top = g.put(0, 1, Zone::Library);
    assert_eq!(g.players[0].library.last(), Some(&top), "the Bear is on top");
    cite!(["601.3"]; refused("that card is not in your hand: it is in a library, and no rule or effect lets you cast it from there (601.3)"): g.act(0, Move::Cast { card: top, targets: vec![] })); // not before level 3
    for level in [2, 3] {
        mana(&mut g);
        level_up(&mut g, class, level, if level == 2 { "CG" } else { "CCCG" }).unwrap();
        resolve(&mut g, None);
    }
    let top = *g.players[0].library.last().unwrap();
    assert_eq!(g.obj(top).chars.name, "Bear");
    // Five base rows, then the level-two attack trigger and level-three look/cast grants.
    cite!(["716.2a", "601.3"]; ok: g.act(0, Move::CastVia(game::announcement::CastRoute::Permission{source:class,ability:7},Box::new(Move::Cast { card: top, targets: vec![] }))));
    resolve(&mut g, None);
    assert!(g.battlefield.iter().any(|o| g.obj(*o).chars.name == "Bear"));
}
