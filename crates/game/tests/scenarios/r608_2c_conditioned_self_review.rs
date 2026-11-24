//! "If this … , it …" opening an ability: "it" is the ability's own
//! object. Each admitted card: the condition checked as intervening if (603.4) or on
//! resolution (608.2c), and the act done to that object and nothing else.
use crate::support::{cite, paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::{CardType, Part};

const OPAL: usize = 0;
const PREDATORS: usize = 1;
const VAULT: usize = 2;
const GUARDIAN: usize = 3;
const ARMOR: usize = 4;
const BEAR: usize = 5;
const OGRE: usize = 6;
const LAND: usize = 7;
const JACKALS: usize = 8;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap();
    let defs = vec![
        real("Opal Avenger"), real("Hidden Predators"), real("Mana Vault"), real("Guardian of the Ages"), real("Iron Man Armor"),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Ogre", "{0}", "Creature — Ogre", "", Some((4, 4))).unwrap(),
        CardDef::new("Land", "", "Basic Land — Forest", "", None).unwrap(),
        real("Lurking Jackals"),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..8 { g.put(p, LAND, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
/// Pass until the stack is empty and nothing is asked, at most `n` resolutions.
fn settle(g: &mut Game, n: usize) -> usize {
    let mut resolved = 0;
    for _ in 0..4 * n + 8 {
        match g.decision.clone() {
            Some(Decision::Choose { player, choice: Choice::May { possible: true } }) => g.act(player, Move::Choose(Answer::No)).unwrap(),
            Some(d) => panic!("{d:?}"),
            None if g.stack.is_empty() => return resolved,
            None => { if g.stack.len() == 1 { resolved += 1; } g.act(g.priority, Move::Pass).unwrap(); }
        }
        if resolved > n { return resolved; }
    }
    resolved
}
/// A player receives priority after an act: state triggers are looked at (603.8).
fn poke(g: &mut Game) -> usize {
    let bear = g.put(0, BEAR, Zone::Hand);
    g.act(0, Move::Cast { card: bear, targets: vec![] }).unwrap();
    settle(g, 3)
}
fn creature(g: &Game, o: ObjId) -> bool { g.is_a(o, CardType::Creature) }

#[test]
fn r603_8_opal_avenger_becomes_a_3_5_once_life_is_10_or_less() {
    let mut g = game();
    let opal = g.put(0, OPAL, Zone::Battlefield);
    g.lose_life(0, 9).unwrap();
    poke(&mut g);
    cite!(["603.8"]; assert!(!creature(&g, opal), "11 life: not yet"));
    g.lose_life(0, 1).unwrap();
    let resolved = poke(&mut g);
    // "becomes a 3/5 Soldier creature" replaces its card types (205.1a): no longer an
    // enchantment, so the intervening if is false from then on and it does not trigger again
    cite!(["205.1a"]; assert!(creature(&g, opal) && !g.is_a(opal, CardType::Enchantment)));
    assert_eq!((g.power(opal), g.toughness(opal)), (Some(3), Some(5)));
    cite!(["603.8", "603.4"]; assert_eq!(resolved, 2, "the creature spell, then the trigger once"));
}

#[test]
fn r603_4_hidden_predators_watches_an_opponents_creature_not_mine() {
    let mut g = game();
    let hp = g.put(0, PREDATORS, Zone::Battlefield);
    g.put(0, OGRE, Zone::Battlefield);
    poke(&mut g);
    cite!(["603.8"]; assert!(!creature(&g, hp), "my own 4-power creature is not an opponent's"));
    g.put(1, OGRE, Zone::Battlefield);
    poke(&mut g);
    cite!(["603.8"]; assert!(creature(&g, hp)));
    assert_eq!((g.power(hp), g.toughness(hp)), (Some(4), Some(4)));
}

#[test]
fn r603_4_mana_vault_deals_1_to_its_controller_only_if_tapped() {
    for tapped in [false, true] {
        let mut g = game();
        let vault = g.put(0, VAULT, Zone::Battlefield);
        // to my next draw step: through the opponent's turn
        while g.active == 0 { g.act(g.priority, Move::Pass).unwrap(); }
        while g.active == 1 { match g.decision.clone() { Some(Decision::Choose { player, .. }) => g.act(player, Move::Choose(Answer::No)).unwrap(), _ => g.act(g.priority, Move::Pass).unwrap() } }
        if tapped { g.tap(vault); }
        while g.part != Part::Draw || !g.stack.is_empty() || g.decision.is_some() {
            match g.decision.clone() { Some(Decision::Choose { player, .. }) => g.act(player, Move::Choose(Answer::No)).unwrap(), _ => g.act(g.priority, Move::Pass).unwrap() }
            if g.part == Part::Draw && g.stack.is_empty() && g.decision.is_none() { break; }
        }
        settle(&mut g, 2);
        cite!(["603.4"]; case: format!("tapped={tapped}"); assert_eq!((g.players[0].life, g.players[1].life), (if tapped { 19 } else { 20 }, 20), "tapped {tapped}"));
    }
}

#[test]
fn r603_4_guardian_of_the_ages_loses_defender_not_the_attacker() {
    let mut g = game();
    let guardian = g.put(1, GUARDIAN, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    assert!(g.has(guardian, "defender"));
    while g.part != Part::DeclareAttackers { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Attack(vec![(bear, Target::Player(1))])).unwrap();
    settle(&mut g, 2);
    cite!(["603.4"]; assert!(!g.has(guardian, "defender") && g.has(guardian, "trample")));
    cite!(["608.2c"]; assert!(!g.has(bear, "trample"), "it is the Guardian, not the attacking creature"));
}

#[test]
fn r301_5c_iron_man_armor_becomes_a_creature_and_falls_off_until_end_of_turn() {
    let mut g = game();
    let armor = g.put(0, ARMOR, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    g.players[0].pool.colorless += 4;
    assert!(g.attach(armor, Target::Object(bear)));
    assert_eq!(g.attached_to(armor), Some(Target::Object(bear)), "equipped");
    g.act(0, paid("CC", Move::Activate { source: armor, ability: 2, targets: vec![], paying: vec![] })).unwrap();
    settle(&mut g, 2);
    // "… artifact creature" keeps its other types and subtypes (205.1b): still an Equipment
    cite!(["205.1b"]; assert!(creature(&g, armor) && g.is_a(armor, CardType::Artifact)));
    cite!(["301.5c"]; assert_eq!(g.attached_to(armor), None, "an Equipment that is a creature is unattached (301.5c)"));
    assert_eq!((g.power(armor), g.toughness(armor)), (Some(1), Some(1)), "0/0 +1/+1 for itself");
    assert!(g.has(armor, "flying"));
    while g.active == 0 {
        match g.decision { Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(), _ => g.act(g.priority, Move::Pass).unwrap() }
    }
    cite!(["514.2"]; assert!(!creature(&g, armor), "until end of turn"));
}

#[test]
fn lurking_jackals_is_admitted_and_watches_an_opponent_s_life() {
    let mut g = game();
    let j = g.put(0, JACKALS, Zone::Battlefield);
    g.lose_life(0, 15).unwrap();
    poke(&mut g);
    cite!(["603.8"]; assert!(!creature(&g, j), "my own life"));
    g.lose_life(1, 10).unwrap();
    poke(&mut g);
    cite!(["603.8"]; assert_eq!((g.power(j), g.toughness(j)), (Some(3), Some(2))));
}

#[test]
fn it_after_another_named_object_is_not_this() {
    // Uncited (admission): "it" is the target creature the condition names, which a condition
    // does not introduce yet.
    let def = CardDef::new("X", "{0}", "Creature — Bear", "{1}: If target creature is tapped, it gets +2/+2 until end of turn.", Some((1, 1))).unwrap();
    crate::support::refused(game::play::check(&def).map_err(|e|e.0), "condition: is, of \"Target\"");
}

#[test]
fn r608_2c_it_after_a_condition_naming_this_creature_in_a_later_sentence_is_this() {
    // "Draw a card. If this creature is tapped, it gets +1/+1": the condition names this
    // creature, so "it" is this creature, not the card drawn before
    for tapped in [true, false] {
        let def = CardDef::new("X", "{0}", "Creature — Bear", "{1}: Draw a card. If this creature is tapped, it gets +1/+1 until end of turn.", Some((1, 1))).unwrap();
        let land = CardDef::new("Land", "", "Basic Land — Forest", "", None).unwrap();
        let mut g = Game::new(vec![def, land], 2, 20);
        for p in 0..2 { for _ in 0..8 { g.put(p, 1, Zone::Library); } }
        while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
        let x = g.put(0, 0, Zone::Battlefield);
        g.set_tapped(x, tapped);
        let hand = g.players[0].hand.len();
        g.players[0].pool.colorless += 1;
        g.act(0, paid("C", Move::Activate { source: x, ability: 0, targets: vec![], paying: vec![] })).unwrap();
        settle(&mut g, 1);
        cite!(["608.2c"]; case: tapped; assert_eq!(g.power(x), Some(if tapped { 2 } else { 1 })));
        cite!(["121.1"]; case: tapped; assert_eq!(g.players[0].hand.len(), hand + 1));
    }
}
