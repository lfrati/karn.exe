//! Triggered abilities played through the engine with real card text: what a card's trigger
//! watches for and what the engine logs are the same actions of the lexicon.

use crate::support::{cite, paid};
use game::card::CardDef;
use game::play::check;
use game::state::Decision;
use game::{Event, Game, Move, ObjId, Target, Zone};

const FOREST: usize = 0;
const MOUNTAIN: usize = 1;
const BEARS: usize = 2;
const VISIONARY: usize = 3;
const WARDEN: usize = 4;
const PRIDEMATE: usize = 5;
const ARTIST: usize = 6;
const THALLID: usize = 7;
const BABOONS: usize = 8;
const GUTTERSNIPE: usize = 9;
const SHOCK: usize = 10;
const BOLT: usize = 11;
const TEMPLE: usize = 12;
const ALARM: usize = 13;
const PLAINS: usize = 14;
const PATRON: usize = 15;
const PYROCLASM: usize = 16;
const DRAKE: usize = 17;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        card("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Elvish Visionary", "{1}{G}", "Creature — Elf Shaman", "When this creature enters, draw a card.", Some((1, 1))),
        card("Soul Warden", "{W}", "Creature — Human Cleric", "Whenever another creature enters, you gain 1 life.", Some((1, 1))),
        card(
            "Ajani's Pridemate",
            "{1}{W}",
            "Creature — Cat Soldier",
            "Whenever you gain life, put a +1/+1 counter on this creature.",
            Some((2, 2)),
        ),
        card(
            "Blood Artist",
            "{1}{B}",
            "Creature — Vampire",
            "Whenever this creature or another creature dies, target player loses 1 life and you gain 1 life.",
            Some((0, 1)),
        ),
        card(
            "Tukatongue Thallid",
            "{G}",
            "Creature — Fungus",
            "When this creature dies, create a 1/1 green Saproling creature token.",
            Some((1, 1)),
        ),
        card(
            "Ravenous Baboons",
            "{3}{R}",
            "Creature — Monkey",
            "When this creature enters, destroy target nonbasic land.",
            Some((2, 2)),
        ),
        card(
            "Guttersnipe",
            "{2}{R}",
            "Creature — Goblin Shaman",
            "Whenever you cast an instant or sorcery spell, this creature deals 2 damage to each opponent.",
            Some((2, 2)),
        ),
        card("Shock", "{R}", "Instant", "Shock deals 2 damage to any target.", None),
        card("Lightning Bolt", "{R}", "Instant", "Lightning Bolt deals 3 damage to any target.", None),
        card("Ancient Tomb", "", "Land", "{T}: Add {C}{C}. This land deals 2 damage to you.", None),
        card("Raise the Alarm", "{1}{W}", "Instant", "Create two 1/1 white Soldier creature tokens.", None),
        card("Plains", "", "Basic Land — Plains", "({T}: Add {W}.)", None),
        card(
            "Patron of the Vein",
            "{4}{B}{B}",
            "Creature — Vampire Shaman",
            "Flying\nWhen this creature enters, destroy target creature an opponent controls.\nWhenever a creature an opponent controls dies, exile it and put a +1/+1 counter on each Vampire you control.",
            Some((4, 4)),
        ),
        card("Pyroclasm", "{1}{R}", "Sorcery", "Pyroclasm deals 2 damage to each creature.", None),
        card(
            "Scavenger Drake",
            "{3}{B}",
            "Creature — Drake",
            "Flying\nWhenever another creature dies, you may put a +1/+1 counter on this creature.",
            Some((1, 1)),
        ),
    ]
}

fn game() -> Game {
    Game::new(defs(), 2, 20)
}

fn mana(g: &mut Game, player: usize, lands: &[usize]) {
    for land in lands {
        let l = g.put(player, *land, Zone::Battlefield);
        g.act(player, Move::Activate { source: l, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
}

/// Both players pass: the top of the stack resolves.
fn pass_both(g: &mut Game) {
    let first = g.priority;
    g.act(first, Move::Pass).unwrap();
    g.act(1 - first, Move::Pass).unwrap();
}

fn on_battlefield(g: &Game, name: &str) -> Option<ObjId> {
    g.battlefield.iter().copied().find(|o| g.obj(*o).chars.name == name)
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r603_3_a_triggered_ability_goes_on_the_stack_the_next_time_a_player_would_receive_priority() {
    let mut g = game();
    g.put(0, FOREST, Zone::Library);
    mana(&mut g, 0, &[FOREST, FOREST]);
    let visionary = g.put(0, VISIONARY, Zone::Hand);
    g.act(0, paid("GG", Move::Cast { card: visionary, targets: vec![] })).unwrap();
    pass_both(&mut g);
    assert!(on_battlefield(&g, "Elvish Visionary").is_some());
    cite!(["603.3"]; assert_eq!(g.stack.len(), 1, "its ability triggered and is on the stack (603.3)"));
    assert!(g.obj(g.stack[0]).ability.is_some());
    cite!(["603.3"]; assert_eq!(g.players[0].hand.len(), 0, "not resolved yet"));
    pass_both(&mut g);
    assert_eq!(g.players[0].hand.len(), 1);
    assert!(g.stack.is_empty());
    assert!(g.players[0].graveyard.is_empty(), "an ability is no card: it is in no graveyard");
}

#[test]
fn r603_2_an_ability_triggers_whenever_an_event_matches_its_trigger_event() {
    let mut g = game();
    g.put(0, WARDEN, Zone::Battlefield);
    let pridemate = g.put(0, PRIDEMATE, Zone::Battlefield);
    mana(&mut g, 0, &[FOREST, FOREST]);
    let bears = g.put(0, BEARS, Zone::Hand);
    g.act(0, paid("GG", Move::Cast { card: bears, targets: vec![] })).unwrap();
    pass_both(&mut g); // the Bears enter: Soul Warden triggers
    cite!(["603.2"]; assert_eq!(g.stack.len(), 1));
    pass_both(&mut g); // gain 1 life: the Pridemate triggers
    assert_eq!(g.players[0].life, 21);
    cite!(["603.2"]; assert_eq!(g.stack.len(), 1));
    pass_both(&mut g);
    assert_eq!((g.power(pridemate), g.toughness(pridemate)), (Some(3), Some(3)));
    assert!(g.stack.is_empty());
}

#[test]
fn r603_3d_its_controller_chooses_its_targets_as_it_goes_on_the_stack() {
    let mut g = game();
    g.put(0, ARTIST, Zone::Battlefield);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    mana(&mut g, 0, &[MOUNTAIN]);
    let bolt = g.put(0, BOLT, Zone::Hand);
    g.act(0, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Object(bears)]] })).unwrap();
    pass_both(&mut g); // the Bears die: Blood Artist triggers and needs a target
    cite!(["603.3d"]; assert!(matches!(g.decision, Some(Decision::Targets { player: 0, groups: 1, .. }))));
    cite!(["603.3d"]; refused("a decision is pending: choose the targets of the triggered ability"): g.act(0, Move::Pass));
    crate::support::refused(g.act(1, Move::ChooseTargets(vec![vec![Target::Player(0)]])), "that decision is another player's");
    cite!(["115.1"]; refused("an illegal target, Grizzly Bears: it isn't a player (115.1)"): g.act(0, Move::ChooseTargets(vec![vec![Target::Object(bears)]])));
    cite!(["603.3d"]; ok: g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)]])));
    assert!(g.decision.is_none());
    assert_eq!(g.stack.len(), 1);
    pass_both(&mut g);
    assert_eq!((g.players[0].life, g.players[1].life), (21, 19));
}

#[test]
fn r603_10a_a_leaves_the_battlefield_ability_looks_back_in_time() {
    let mut g = game();
    let thallid = g.put(1, THALLID, Zone::Battlefield);
    mana(&mut g, 0, &[MOUNTAIN]);
    let shock = g.put(0, SHOCK, Zone::Hand);
    g.act(0, paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Object(thallid)]] })).unwrap();
    pass_both(&mut g);
    assert_eq!(g.obj(thallid).zone, Zone::Gone);
    cite!(["603.10a"]; assert_eq!(g.stack.len(), 1, "the ability of the creature that died (603.10a)"));
    cite!(["603.3a"]; assert_eq!(g.obj(g.stack[0]).controller, 1));
    pass_both(&mut g);
    let saproling = on_battlefield(&g, "Saproling Token").expect("a Saproling token");
    assert_eq!(g.obj(saproling).controller, 1);
    assert!(g.obj(saproling).token());
}

/// A Bolt at an opponent's Bears, resolved: the Bears have died.
fn bolt_the_bears(g: &mut Game) {
    let bears = g.put(1, BEARS, Zone::Battlefield);
    mana(g, 0, &[MOUNTAIN]);
    let bolt = g.put(0, BOLT, Zone::Hand);
    g.act(0, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Object(bears)]] })).unwrap();
    pass_both(g);
    assert_eq!(g.players[1].graveyard.len(), 1);
}

fn exiled(g: &Game) -> Vec<&str> {
    g.exile.iter().map(|o| g.obj(*o).chars.name.as_str()).collect()
}

#[test]
fn r400_7e_an_ability_that_triggers_on_a_zone_change_finds_the_object_it_became() {
    // "Whenever a creature an opponent controls dies, exile it …": the creature is named as
    // it last was on the battlefield (603.10a), and what is exiled is the card it became in
    // the graveyard
    let mut g = game();
    let patron = g.put(0, PATRON, Zone::Battlefield);
    bolt_the_bears(&mut g);
    assert_eq!(g.stack.len(), 1, "the Patron's ability");
    pass_both(&mut g);
    cite!(["400.7e"]; assert_eq!(exiled(&g), ["Grizzly Bears"]));
    cite!(["400.7e"]; assert!(g.players[1].graveyard.is_empty()));
    assert_eq!((g.power(patron), g.toughness(patron)), (Some(5), Some(5)), "and the rest of its effect");
}

#[test]
fn r603_6c_what_left_the_battlefield_is_looked_for_only_in_the_first_zone_it_went_to() {
    // two Patrons trigger for one death: the first ability exiles the card from the
    // graveyard; the second no longer finds it there and exiles nothing, the card in exile
    // being another object (400.7), and still does the rest
    let mut g = game();
    let patrons = [g.put(0, PATRON, Zone::Battlefield), g.put(0, PATRON, Zone::Battlefield)];
    bolt_the_bears(&mut g);
    // the abilities of two permanents: their controller says in which order (603.3b)
    g.act(0, Move::Order(vec![0, 1])).unwrap();
    assert_eq!(g.stack.len(), 2);
    pass_both(&mut g);
    pass_both(&mut g);
    assert_eq!(exiled(&g), ["Grizzly Bears"]);
    cite!(["603.6c"]; assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Exiled { .. })).count(), 1, "exiled once"));
    for patron in patrons {
        cite!(["101.3"]; assert_eq!(g.power(patron), Some(6), "a counter from each ability"));
    }
}

#[test]
fn r603_10a_a_permanent_sees_what_left_the_battlefield_at_the_same_time_as_it() {
    // "Whenever another creature dies, …" on a creature destroyed by the same state-based
    // action check as the other (704.3: simultaneously): it looks back, and triggers
    let mut g = game();
    g.put(0, BEARS, Zone::Battlefield);
    let drake = g.put(1, DRAKE, Zone::Battlefield);
    mana(&mut g, 0, &[MOUNTAIN, MOUNTAIN]);
    let pyroclasm = g.put(0, PYROCLASM, Zone::Hand);
    g.act(0, paid("RR", Move::Cast { card: pyroclasm, targets: vec![] })).unwrap();
    pass_both(&mut g);
    assert_eq!(g.obj(drake).zone, Zone::Gone, "both died");
    cite!(["603.10a"]; assert_eq!(g.stack.len(), 1, "the Drake's ability, for the Bears"));
    assert_eq!((g.obj(g.stack[0]).ability, g.obj(g.stack[0]).controller), (Some((Some(drake), 1)), 1));
    // its own death is not "another creature" dying: alone, nothing triggers
    let mut g = game();
    let drake = g.put(1, DRAKE, Zone::Battlefield);
    mana(&mut g, 0, &[MOUNTAIN]);
    let shock = g.put(0, SHOCK, Zone::Hand);
    g.act(0, paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Object(drake)]] })).unwrap();
    pass_both(&mut g);
    // Uncited (card text): "Whenever another creature dies" excludes the Drake's own death.
    assert!(g.stack.is_empty() && g.decision.is_none());
}

#[test]
fn r603_3b_a_player_puts_their_triggered_abilities_on_the_stack_in_the_order_they_choose() {
    // the Visionary enters: its own "draw a card" and the Warden's "you gain 1 life" both
    // triggered, both player 0's
    let entered = || {
        let mut g = game();
        g.put(0, FOREST, Zone::Library);
        let warden = g.put(0, WARDEN, Zone::Battlefield);
        mana(&mut g, 0, &[FOREST, FOREST]);
        let visionary = g.put(0, VISIONARY, Zone::Hand);
        g.act(0, paid("GG", Move::Cast { card: visionary, targets: vec![] })).unwrap();
        pass_both(&mut g);
        (g, warden)
    };
    let (mut g, warden) = entered();
    let Some(Decision::Order { player: 0, abilities }) = g.decision.clone() else { panic!("an order to choose: {:?}", g.decision) };
    assert_eq!(abilities.len(), 2);
    cite!(["603.3b"]; refused("a decision is pending: the order of the abilities that triggered (603.3b)"): g.act(0, Move::Pass));
    crate::support::refused(g.act(1, Move::Order(vec![0, 1])), "that decision is another player's");
    cite!(["603.3b"]; refused("an order is each of the 2 abilities once (603.3b)"): g.act(0, Move::Order(vec![0, 0])));
    // the last put on the stack is on top, and resolves first
    let first = abilities.iter().position(|t| t.source == Some(warden)).unwrap();
    g.act(0, Move::Order(vec![first, 1 - first])).unwrap();
    let sources = |g: &Game| g.stack.iter().map(|o| g.obj(*o).ability.unwrap().0.unwrap()).collect::<Vec<_>>();
    cite!(["603.3b"]; assert_eq!(sources(&g)[0], warden, "the Warden's first, under the Visionary's"));
    pass_both(&mut g);
    cite!(["405.5"]; assert_eq!((g.players[0].hand.len(), g.players[0].life), (1, 20), "the card is drawn first"));
    // and the other way round
    let (mut g, warden) = entered();
    g.act(0, Move::Order(vec![1 - first, first])).unwrap();
    cite!(["603.3b"]; assert_eq!(sources(&g)[1], warden));
    pass_both(&mut g);
    cite!(["405.5"]; assert_eq!((g.players[0].hand.len(), g.players[0].life), (0, 21), "the life is gained first"));
}

#[test]
fn r603_3d_with_no_legal_choice_for_a_target_the_ability_is_removed() {
    let mut g = game();
    mana(&mut g, 0, &[MOUNTAIN, MOUNTAIN, MOUNTAIN, MOUNTAIN]);
    let baboons = g.put(0, BABOONS, Zone::Hand);
    g.act(0, paid("RRRR", Move::Cast { card: baboons, targets: vec![] })).unwrap();
    pass_both(&mut g);
    assert!(on_battlefield(&g, "Ravenous Baboons").is_some());
    cite!(["603.3d"]; assert!(g.decision.is_none() && g.stack.is_empty(), "only basic lands: no target, no ability (603.3d)"));
}

#[test]
fn r603_6a_an_enters_the_battlefield_ability_triggers_when_a_permanent_enters() {
    let mut g = game();
    let temple = g.put(1, TEMPLE, Zone::Battlefield);
    mana(&mut g, 0, &[MOUNTAIN, MOUNTAIN, MOUNTAIN, MOUNTAIN]);
    let baboons = g.put(0, BABOONS, Zone::Hand);
    g.act(0, paid("RRRR", Move::Cast { card: baboons, targets: vec![] })).unwrap();
    pass_both(&mut g);
    cite!(["603.6a"]; assert!(matches!(g.decision, Some(Decision::Targets { player: 0, .. }))));
    let mountain = g.battlefield[1];
    cite!(["115.1"]; refused("an illegal target, Mountain: it is basic (115.1)"): g.act(0, Move::ChooseTargets(vec![vec![Target::Object(mountain)]])));
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(temple)]])).unwrap();
    pass_both(&mut g);
    assert_eq!(g.obj(temple).zone, Zone::Gone);
    assert!(g.log.contains(&Event::Destroyed { object: temple }));
}

#[test]
fn r701_7a_a_created_token_is_put_onto_the_battlefield_and_so_enters() {
    // "Whenever another creature enters, you gain 1 life." sees each token that is created
    let mut g = game();
    let warden = g.put(0, WARDEN, Zone::Battlefield);
    mana(&mut g, 0, &[PLAINS, PLAINS]);
    let alarm = g.put(0, ALARM, Zone::Hand);
    g.act(0, paid("WW", Move::Cast { card: alarm, targets: vec![] })).unwrap();
    pass_both(&mut g);
    // Each Soldier creates a separate Warden occurrence; its controller chooses
    // their stack order even though source/text and eventual life gains agree.
    let Some(Decision::Order { player: 0, abilities }) = g.decision.clone() else { panic!("expected two Warden triggers: {:?}", g.decision) };
    cite!(["603.3b"]; assert_eq!(abilities.len(), 2));
    assert!(abilities.iter().all(|ability| ability.source == Some(warden) && ability.controller == 0));
    g.act(0, Move::Order(vec![0, 1])).unwrap();
    cite!(["701.7a", "603.6a"]; assert_eq!(g.stack.len(), 2, "one trigger for each of the two Soldiers"));
    pass_both(&mut g);
    pass_both(&mut g);
    assert_eq!(g.players[0].life, 22);
}

#[test]
fn r405_5_the_last_added_object_on_the_stack_resolves_first() {
    let mut g = game();
    g.put(0, GUTTERSNIPE, Zone::Battlefield);
    mana(&mut g, 0, &[MOUNTAIN]);
    let shock = g.put(0, SHOCK, Zone::Hand);
    g.act(0, paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Player(1)]] })).unwrap();
    cite!(["603.3"]; assert_eq!(g.stack.len(), 2, "the trigger is above the spell"));
    pass_both(&mut g);
    cite!(["405.5"]; assert_eq!(g.players[1].life, 18, "Guttersnipe's 2"));
    assert_eq!(g.stack.len(), 1);
    pass_both(&mut g);
    cite!(["405.5"]; assert_eq!(g.players[1].life, 16, "then the Shock"));
}
