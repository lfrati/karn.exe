//! Abilities an effect gives in quotes (CR 113.2c, 613.1f): what has one activates it and
//! triggers with it, each giving is its own instance, and what was given is still known of
//! an object that is no more (603.10a, 113.7a). Every card is read from its text.

use crate::support::cite;
use game::card::CardDef;
use game::play::check;
use game::{Game, Move, ObjId, Target, Zone};

const BEARS: usize = 0;
const PINGER_LORD: usize = 1;
const MOURNER_LORD: usize = 2;
const ALTAR: usize = 3;
const WRATH: usize = 4;
const BLESSER: usize = 5;
const ANTHEM_LORD: usize = 6;
const PACIFIST_LORD: usize = 7;
const SPAWNER: usize = 8;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card(
            "Pinger Lord",
            "{2}{R}",
            "Creature — Wizard",
            "Other creatures you control have \"{T}: This creature deals 1 damage to any target.\"\nSacrifice this creature: Add {C}.",
            Some((1, 1)),
        ),
        card(
            "Mourner Lord",
            "{2}{W}",
            "Creature — Cleric",
            "Other creatures you control have \"When this creature dies, you gain 2 life.\"\nSacrifice this creature: Add {C}.",
            Some((1, 1)),
        ),
        card("Ashnod's Altar", "{3}", "Artifact", "Sacrifice a creature: Add {C}{C}.", None),
        card("Wrath", "{0}", "Sorcery", "Destroy all creatures.", None),
        card(
            "Blesser",
            "{W}",
            "Enchantment",
            "{0}: Until end of turn, target creature gains \"When this creature dies, you gain 2 life.\"",
            None,
        ),
        card("Anthem Lord", "{2}{W}", "Creature — Soldier", "Creatures you control have \"Other creatures you control get +1/+1.\"", Some((1, 1))),
        card("Pacifist Lord", "{2}{W}", "Creature — Monk", "Other creatures you control have \"This creature can't block.\"", Some((1, 1))),
        card("Spawner", "{2}", "Artifact", "{0}: Create a 0/1 colorless Eldrazi Spawn creature token. It has \"Sacrifice this token: Add {C}.\"", None),
    ]
}

fn game() -> Game {
    Game::new(defs(), 2, 20)
}

fn all_pass(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}

fn activate(g: &mut Game, source: ObjId, ability: usize, targets: Vec<Vec<Target>>, paying: Vec<Vec<ObjId>>) -> Result<(), String> {
    g.act(0, Move::Activate { source, ability, targets, paying })
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r113_7a_a_given_activated_ability_resolves_as_what_it_was_after_it_is_no_longer_given() {
    let mut g = game();
    let lord = g.put(0, PINGER_LORD, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    // the bears have the ability the lord gives them, after their own (none); the lord has
    // only its own
    cite!(["113.2c"]; assert_eq!((g.abilities(bears).len(), g.abilities(lord).len()), (1, 2)));
    activate(&mut g, bears, 0, vec![vec![Target::Player(1)]], vec![]).unwrap();
    assert!(g.obj(bears).tapped && g.stack.len() == 1);
    // what gave it leaves: the bears no longer have it
    activate(&mut g, lord, 1, vec![], vec![]).unwrap();
    assert_eq!(g.obj(lord).zone, Zone::Gone);
    cite!(["611.3b"]; assert!(g.abilities(bears).is_empty()));
    // and the ability on the stack is still what it was (113.7a)
    all_pass(&mut g);
    cite!(["113.7a"]; assert_eq!(g.players[1].life, 19));
    assert!(g.stack.is_empty());
    // nothing of the bears is activated now (602.1)
    cite!(["611.3a"]; refused("that is not an activated ability of that permanent"): activate(&mut g, bears, 0, vec![vec![Target::Player(1)]], vec![]));
}

#[test]
fn r603_10a_a_given_death_trigger_is_an_ability_of_what_died() {
    let mut g = game();
    let _lord = g.put(0, MOURNER_LORD, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let altar = g.put(0, ALTAR, Zone::Battlefield);
    activate(&mut g, altar, 0, vec![], vec![vec![bears]]).unwrap();
    // the bears are no more; what they had as they last existed triggers
    cite!(["603.10a"]; assert_eq!(g.stack.len(), 1));
    all_pass(&mut g);
    cite!(["603.10a"]; assert_eq!(g.players[0].life, 22));
}

#[test]
fn r603_10a_a_given_death_trigger_triggers_when_what_gave_it_dies_with_it() {
    let mut g = game();
    let _lord = g.put(0, MOURNER_LORD, Zone::Battlefield);
    let (first, second) = (g.put(0, BEARS, Zone::Battlefield), g.put(0, BEARS, Zone::Battlefield));
    let wrath = g.put(0, WRATH, Zone::Hand);
    g.act(0, Move::Cast { card: wrath, targets: vec![] }).unwrap();
    all_pass(&mut g);
    // CR404.3: the owner explicitly orders these simultaneous graveyard arrivals.
    assert_eq!(g.decision, Some(game::state::Decision::Choose { player: 0, choice: game::state::Choice::OrderCards { to: game::state::Placement::Zone(Zone::Graveyard), cards: vec![_lord, first, second] } }));
    g.act(0, Move::Choose(game::state::Answer::Objects(vec![second, _lord, first]))).unwrap();
    assert!(g.obj(first).zone == Zone::Gone && g.obj(second).zone == Zone::Gone);
    // each of the bears had it as it last existed, though nothing gives it now; the lord,
    // which gave it to the others, had not
    while !g.stack.is_empty() || g.decision.is_some() {
        if g.decision.is_some() { g.act(0, Move::Order(vec![0, 1])).unwrap(); } else { all_pass(&mut g); }
    }
    cite!(["603.10a"]; assert_eq!(g.players[0].life, 24));
}

#[test]
fn r113_2c_two_givings_of_the_same_words_are_two_abilities() {
    let mut g = game();
    let blesser = g.put(0, BLESSER, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let altar = g.put(0, ALTAR, Zone::Battlefield);
    for given in 1..=2 {
        activate(&mut g, blesser, 0, vec![vec![Target::Object(bears)]], vec![]).unwrap();
        all_pass(&mut g);
        cite!(["113.2c"]; case: format!("giving {given}"); assert_eq!(g.abilities(bears).len(), given));
    }
    activate(&mut g, altar, 0, vec![], vec![vec![bears]]).unwrap();
    // each triggers (603.2): two abilities wait to be ordered or are on the stack
    while !g.stack.is_empty() || g.decision.is_some() {
        if g.decision.is_some() { g.act(0, Move::Order(vec![0, 1])).unwrap(); } else { all_pass(&mut g); }
    }
    cite!(["113.2c"]; assert_eq!(g.players[0].life, 24));
}

#[test]
fn r113_2c_one_giving_ending_leaves_the_other() {
    let mut g = game();
    let (first, _second) = (g.put(0, MOURNER_LORD, Zone::Battlefield), g.put(0, MOURNER_LORD, Zone::Battlefield));
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let altar = g.put(0, ALTAR, Zone::Battlefield);
    cite!(["113.2c"]; assert_eq!(g.abilities(bears).len(), 2));
    // one lord leaves by its own mana ability: the other lord's ability triggers for it, and
    // the bears keep what the other gives
    activate(&mut g, first, 1, vec![], vec![]).unwrap();
    while !g.stack.is_empty() { all_pass(&mut g); }
    cite!(["113.2c"]; assert_eq!((g.players[0].life, g.abilities(bears).len()), (22, 1)));
    activate(&mut g, altar, 0, vec![], vec![vec![bears]]).unwrap();
    while !g.stack.is_empty() { all_pass(&mut g); }
    cite!(["113.2c"]; assert_eq!(g.players[0].life, 24));
}

#[test]
fn r613_1f_a_given_static_ability_changes_power_and_toughness_after_it_is_given() {
    let mut g = game();
    let lord = g.put(0, ANTHEM_LORD, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let theirs = g.put(1, BEARS, Zone::Battlefield);
    // each has the ability, and each ability changes the other creature
    let pt = |g: &Game, o| (g.power(o).unwrap(), g.toughness(o).unwrap());
    cite!(["613.1f", "613.4c"]; assert_eq!((pt(&g, lord), pt(&g, bears), pt(&g, theirs)), ((2, 2), (3, 3), (2, 2))));
    let more = g.put(0, BEARS, Zone::Battlefield);
    cite!(["613.1f", "613.4c"]; assert_eq!((pt(&g, lord), pt(&g, bears), pt(&g, more)), ((3, 3), (4, 4), (4, 4))));
}

#[test]
fn r613_1f_what_a_given_static_ability_could_not_be_applied_for_is_refused() {
    let refused = |text: &str| check(&CardDef::new("Lord", "{1}", "Creature — Wizard", text, Some((1, 1))).unwrap()).unwrap_err().0;
    // Passive keyword statics now participate in the shared layer-6 dependency trials.
    check(&CardDef::new("Lord", "{1}", "Creature — Wizard",
        "Creatures you control have \"Creatures you control have flying.\"", Some((1, 1))).unwrap()).unwrap();
    assert!(refused("Creatures you control have \"Creatures you control lose all abilities.\"").contains("613.1d-f"));
    // what a replacement effect is known by names an ability of its source's own definition
    assert!(refused("Creatures you control have \"If you would gain life, you gain twice that much life instead.\"").contains("replacement"));
}

#[test]
fn r604_1_a_given_restriction_is_in_force_for_what_has_it() {
    let mut g = game();
    let lord = g.put(0, PACIFIST_LORD, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    assert_eq!((g.abilities(bears).len(), g.abilities(lord).len()), (1, 1));
    let attacker = g.put(1, BEARS, Zone::Battlefield);
    assert!(!g.cant_block(lord, attacker, &[lord]));
    cite!(["604.1"]; assert!(g.cant_block(bears, attacker, &[bears])));
}

#[test]
fn r113_2c_a_token_made_with_an_ability_in_quotes_has_it_while_it_exists() {
    let mut g = game();
    let spawner = g.put(0, SPAWNER, Zone::Battlefield);
    activate(&mut g, spawner, 0, vec![], vec![]).unwrap();
    all_pass(&mut g);
    let [_, token] = g.battlefield.as_slice() else { panic!("the artifact and its token") };
    let token = *token;
    cite!(["113.2c"]; assert_eq!(g.abilities(token).len(), 1));
    // its cost names the token itself, and it is a mana ability (605.1a)
    activate(&mut g, token, 0, vec![], vec![]).unwrap();
    assert_eq!(g.obj(token).zone, Zone::Gone);
    assert_eq!(g.players[0].pool.colorless, 1);
    cite!(["605.3b"]; assert!(g.stack.is_empty()));
}
