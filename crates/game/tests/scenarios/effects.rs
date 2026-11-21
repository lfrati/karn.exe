//! Effects, rule by rule: static abilities (CR 604), the steps of following a spell's or an
//! ability's instructions and the information they use (608.2), continuous effects (611), the
//! order they are applied in within a layer (613.4, 613.7, 613.8), and replacement effects
//! (614). Every card is read from its printed text.

use crate::support::{cite, paid};
use game::card::CardDef;
use game::play::check;
use game::state::Decision;
use game::{Event, Game, Move, ObjId, Target, Zone};

const FOREST: usize = 0;
const PLAINS: usize = 1;
const SWAMP: usize = 2;
const MOUNTAIN: usize = 3;
const ISLAND: usize = 4;
const BEARS: usize = 5;
const ANTHEM: usize = 6;
const KNIGHTHOOD: usize = 7;
const ORB: usize = 8;
const GROWTH: usize = 9;
const OVERRUN: usize = 10;
const RAID: usize = 11;
const SCOURGER: usize = 12;
const SCHOLAR: usize = 13;
const SOL_RING: usize = 14;
const ARCHON: usize = 15;
const GIGANTOMANCER: usize = 16;
const KWENDE: usize = 17;
const UNICORN: usize = 18;
const WATCHDOG: usize = 19;
const GRUMGULLY: usize = 20;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        card("Plains", "", "Basic Land — Plains", "({T}: Add {W}.)", None),
        card("Swamp", "", "Basic Land — Swamp", "({T}: Add {B}.)", None),
        card("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None),
        card("Island", "", "Basic Land — Island", "({T}: Add {U}.)", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Glorious Anthem", "{1}{W}{W}", "Enchantment", "Creatures you control get +1/+1.", None),
        card("Knighthood", "{2}{W}", "Enchantment", "Creatures you control have first strike.", None),
        card("Orb of Dreams", "{3}", "Artifact", "Permanents enter tapped.", None),
        card("Giant Growth", "{G}", "Instant", "Target creature gets +3/+3 until end of turn.", None),
        card(
            "Overrun",
            "{2}{G}{G}{G}",
            "Sorcery",
            "Creatures you control get +3/+3 and gain trample until end of turn. (Each of those creatures can deal excess combat damage to the player or planeswalker it's attacking.)",
            None,
        ),
        card("Massive Raid", "{1}{R}{R}", "Instant", "Massive Raid deals damage to any target equal to the number of creatures you control.", None),
        card(
            "Gruesome Scourger",
            "{3}{B}{B}",
            "Creature — Orc Warrior",
            "When this creature enters, it deals damage to target opponent or planeswalker equal to the number of creatures you control.",
            Some((3, 3)),
        ),
        card(
            "Scholar of Stars",
            "{3}{U}",
            "Creature — Human Artificer",
            "When this creature enters, if you control an artifact, draw a card.",
            Some((3, 2)),
        ),
        card("Sol Ring", "{1}", "Artifact", "{T}: Add {C}{C}.", None),
        card(
            "Harmonious Archon",
            "{4}{W}{W}",
            "Creature — Archon",
            "Flying\nNon-Archon creatures have base power and toughness 3/3.\nWhen this creature enters, create two 1/1 white Human creature tokens.",
            Some((4, 5)),
        ),
        card(
            "Gigantomancer",
            "{7}{G}",
            "Creature — Human Shaman",
            "{1}: Target creature you control has base power and toughness 7/7 until end of turn.",
            Some((1, 1)),
        ),
        card(
            "Kwende, Pride of Femeref",
            "{3}{W}",
            "Legendary Creature — Human Knight",
            "Double strike\nCreatures you control with first strike have double strike.",
            Some((2, 2)),
        ),
        card(
            "Good-Fortune Unicorn",
            "{1}{G}{W}",
            "Creature — Unicorn",
            "Whenever another creature you control enters, put a +1/+1 counter on that creature.",
            Some((2, 2)),
        ),
        card(
            "Faithful Watchdog",
            "{G}{W}",
            "Creature — Dog",
            "Vigilance\nThis creature enters with three +1/+1 counters on it.",
            Some((0, 0)),
        ),
        card(
            "Grumgully, the Generous",
            "{1}{R}{G}",
            "Legendary Creature — Goblin Shaman",
            "Each other non-Human creature you control enters with an additional +1/+1 counter on it.",
            Some((3, 3)),
        ),
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

/// Put lands onto the battlefield for player 0 and tap them for mana.
fn mana(g: &mut Game, land: usize, n: usize) {
    for _ in 0..n {
        let l = g.put(0, land, Zone::Battlefield);
        g.act(0, Move::Activate { source: l, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
}

/// Player 0 casts this card from hand with this mana: the spell, on the stack.
fn cast(g: &mut Game, def: usize, land: usize, lands: usize, targets: Vec<Vec<Target>>, spending: &str) -> ObjId {
    let card = g.put(0, def, Zone::Hand);
    mana(g, land, lands);
    g.act(0, paid(spending, Move::Cast { card, targets })).unwrap();
    *g.stack.last().unwrap()
}

/// A triggered ability waiting for its target as it goes on the stack gets this one.
fn target_trigger(g: &mut Game, t: Target) {
    if let Some(Decision::Targets { player, .. }) = g.decision {
        g.act(player, Move::ChooseTargets(vec![vec![t]])).unwrap();
    }
}

/// Player 0 activates Gigantomancer's ability at this creature, paying with a Forest; the
/// ability is on the stack.
fn seven(g: &mut Game, mancer: ObjId, at: ObjId) {
    mana(g, FOREST, 1);
    g.act(0, paid("G", Move::Activate { source: mancer, ability: 0, targets: vec![vec![Target::Object(at)]], paying: vec![] })).unwrap();
}

fn pt(g: &Game, o: ObjId) -> (i32, i32) {
    (g.power(o).unwrap(), g.toughness(o).unwrap())
}

fn permanent(g: &Game, name: &str) -> ObjId {
    g.battlefield.iter().copied().find(|o| g.obj(*o).chars.name == name).unwrap_or_else(|| panic!("{name} is on the battlefield"))
}

/// The source of the last damage dealt, what it was dealt to and how much.
fn last_damage(g: &Game) -> Option<(ObjId, Target, u32)> {
    g.log.iter().rev().find_map(|e| match e {
        Event::DamageDealt { source, to, amount, .. } => Some((*source, *to, *amount)),
        _ => None,
    })
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r604_1_a_static_ability_does_something_all_the_time_without_being_activated_or_triggered() {
    // "Creatures you control get +1/+1.", "Creatures you control have first strike.": the
    // moment each is on the battlefield it is simply true, with nothing put on the stack and
    // nothing done by a player
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    cast(&mut g, ANTHEM, PLAINS, 3, vec![], "WWW");
    all_pass(&mut g);
    cite!(["604.1"]; assert_eq!((pt(&g, bears), g.stack.len(), g.decision.clone()), ((3, 3), 0, None)));
    let before = g.log.len();
    g.put(0, KNIGHTHOOD, Zone::Battlefield);
    cite!(["604.1"]; assert!(g.has(bears, "First strike")));
    cite!(["604.1"]; assert_eq!((g.stack.len(), g.log.len()), (0, before), "nothing happened: it is true"));
    // and stays true as the game goes on, asked of whatever comes
    let late = g.put(0, BEARS, Zone::Battlefield);
    cite!(["611.3a"]; assert_eq!((pt(&g, late), g.has(late, "First strike")), ((3, 3), true)));
}

#[test]
fn r604_2_a_static_abilitys_effects_are_active_while_its_permanent_is_on_the_battlefield() {
    // "Permanents enter tapped.": a replacement effect of a static ability, in force while the
    // Orb is on the battlefield, and not after
    let mut g = game();
    let orb = g.put(1, ORB, Zone::Battlefield);
    let land = g.put(0, FOREST, Zone::Hand);
    g.act(0, Move::PlayLand(land)).unwrap();
    cite!(["604.2"]; assert!(g.obj(permanent(&g, "Forest")).tapped));
    g.destroy(orb);
    cast(&mut g, BEARS, FOREST, 2, vec![], "GG");
    all_pass(&mut g);
    cite!(["604.2"]; assert!(!g.obj(permanent(&g, "Grizzly Bears")).tapped, "the Orb has left the battlefield"));
    // "Creatures you control get +1/+1.": likewise
    let bears = permanent(&g, "Grizzly Bears");
    let anthem = g.put(0, ANTHEM, Zone::Battlefield);
    cite!(["604.2"]; assert_eq!(pt(&g, bears), (3, 3)));
    g.destroy(anthem);
    cite!(["604.2"]; assert_eq!(pt(&g, bears), (2, 2)));
}

#[test]
fn r608_2_the_targets_are_checked_first_and_the_spell_is_put_into_the_graveyard_last() {
    // "Massive Raid deals damage to any target equal to the number of creatures you control."
    let mut g = game();
    g.put(0, BEARS, Zone::Battlefield);
    let theirs = g.put(1, BEARS, Zone::Battlefield);
    let raid = cast(&mut g, RAID, MOUNTAIN, 3, vec![vec![Target::Object(theirs)]], "RRR");
    all_pass(&mut g);
    let at = |g: &Game, p: &dyn Fn(&Event) -> bool| g.log.iter().position(p).expect("logged");
    let damage = at(&g, &|e| matches!(e, Event::DamageDealt { to: Target::Object(o), amount: 1, .. } if *o == theirs));
    let graveyard = at(&g, &|e| matches!(e, Event::ZoneChanged { was, from: Zone::Stack, to: Zone::Graveyard, .. } if *was == raid));
    cite!(["608.2n"]; assert!(damage < graveyard, "its instruction is followed, then it is put into its owner's graveyard (608.2n)"));
    assert_eq!((g.stack.len(), g.players[0].graveyard.len()), (0, 1));
    // its only target gone, it does not resolve: no instruction is followed (608.2b)
    let mut g = game();
    g.put(0, BEARS, Zone::Battlefield);
    let theirs = g.put(1, BEARS, Zone::Battlefield);
    let raid = cast(&mut g, RAID, MOUNTAIN, 3, vec![vec![Target::Object(theirs)]], "RRR");
    g.destroy(theirs);
    all_pass(&mut g);
    assert!(g.log.contains(&Event::Fizzled { spell: raid }));
    cite!(["608.2b"]; assert!(!g.log.iter().any(|e| matches!(e, Event::DamageDealt { .. }))));
    cite!(["608.2b"]; assert_eq!((g.stack.len(), g.players[0].graveyard.len()), (0, 1), "it is put into its owner's graveyard"));
}

#[test]
fn r608_2a_an_intervening_if_is_checked_again_as_the_ability_resolves() {
    // "When this creature enters, if you control an artifact, draw a card."
    let entered = || {
        let mut g = game();
        g.put(0, BEARS, Zone::Library);
        let ring = g.put(0, SOL_RING, Zone::Battlefield);
        cast(&mut g, SCHOLAR, ISLAND, 4, vec![], "UUUU");
        all_pass(&mut g);
        cite!(["603.4"]; assert_eq!(g.stack.len(), 1, "it triggered"));
        (g, ring)
    };
    // the condition isn't true: the ability is removed from the stack and does nothing
    let (mut g, ring) = entered();
    let ability = *g.stack.last().unwrap();
    g.destroy(ring);
    all_pass(&mut g);
    assert!(g.log.contains(&Event::Fizzled { spell: ability }));
    cite!(["608.2a"]; assert_eq!((g.stack.len(), g.players[0].hand.len()), (0, 0)));
    // what is asked is the condition, of the game as it is: another artifact makes it true
    let (mut g, ring) = entered();
    g.destroy(ring);
    g.put(0, SOL_RING, Zone::Battlefield);
    all_pass(&mut g);
    cite!(["608.2a"]; assert_eq!((g.stack.len(), g.players[0].hand.len()), (0, 1), "it continued to resolve"));
}

#[test]
fn r608_2h_information_from_the_game_is_determined_when_the_effect_is_applied() {
    // "When this creature enters, it deals damage to target opponent or planeswalker equal to
    // the number of creatures you control."
    let triggered = || {
        let mut g = game();
        g.put(0, BEARS, Zone::Battlefield);
        cast(&mut g, SCOURGER, SWAMP, 5, vec![], "BBBBB");
        all_pass(&mut g);
        target_trigger(&mut g, Target::Player(1));
        assert_eq!(g.stack.len(), 1, "it triggered and is on the stack");
        let scourger = permanent(&g, "Gruesome Scourger");
        (g, scourger)
    };
    // two creatures as it triggered, four as it resolves: the answer is the one then
    let (mut g, scourger) = triggered();
    g.put(0, BEARS, Zone::Battlefield);
    g.put(0, BEARS, Zone::Battlefield);
    all_pass(&mut g);
    cite!(["608.2h"]; assert_eq!(last_damage(&g), Some((scourger, Target::Player(1), 4))));
    assert_eq!(g.players[1].life, 16);
    // "it deals": the creature does, as it most recently existed, though it has left the
    // battlefield; and it is no longer among the creatures counted
    let (mut g, scourger) = triggered();
    g.destroy(scourger);
    all_pass(&mut g);
    cite!(["608.2h"]; assert_eq!(last_damage(&g), Some((scourger, Target::Player(1), 1))));
    assert_eq!(g.players[1].life, 19);
}

#[test]
fn r608_2i_an_effect_that_looks_back_further_than_the_history_keeps_is_not_run_yet() {
    // what happened this turn is asked (r608_2i_turn_history.rs); of the turn before, only the
    // spells cast are kept: the cards that need more are refused rather than run as if nothing
    // had happened
    let goes_on = CardDef::new("Life Goes On", "{G}", "Instant", "You gain 4 life. If a creature died this turn, you gain 8 life instead.", None)
        .unwrap();
    check(&goes_on).unwrap();
    let attacked = CardDef::new("Charge", "{R}", "Instant", "If you attacked this turn, you gain 4 life.", None).unwrap();
    check(&attacked).unwrap();
    let died = CardDef::new("Rue", "{B}", "Instant", "If a creature died last turn, you gain 4 life.", None).unwrap();
    assert!(check(&died).unwrap_err().0.contains("happened"), "{:?}", check(&died));
}

#[test]
fn r611_2_the_resolution_of_a_spell_may_generate_a_continuous_effect() {
    // "Creatures you control get +3/+3 and gain trample until end of turn."
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let theirs = g.put(1, BEARS, Zone::Battlefield);
    assert!(g.continuous.is_empty());
    cast(&mut g, OVERRUN, FOREST, 5, vec![], "GGGGG");
    all_pass(&mut g);
    // the spell is gone; what it began goes on
    assert_eq!((g.stack.len(), g.players[0].graveyard.len()), (0, 1));
    cite!(["611.2"]; assert!(!g.continuous.is_empty()));
    cite!(["611.2a"]; assert!(g.continuous.iter().all(|c| c.affected == vec![bears] && c.lifetime == game::layers::Lifetime::Turn)));
    cite!(["611.2"]; assert_eq!((pt(&g, bears), g.has(bears, "Trample")), ((5, 5), true)));
    assert_eq!((pt(&g, theirs), g.has(theirs, "Trample")), ((2, 2), false));
}

#[test]
fn r611_3_a_static_ability_generates_a_continuous_effect() {
    // "Creatures you control get +1/+1.": no spell or ability resolved, and yet an effect
    // applies, for as long as the Anthem is there
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    g.put(0, ANTHEM, Zone::Battlefield);
    assert!(g.continuous.is_empty(), "nothing resolved");
    cite!(["611.3"]; assert_eq!(pt(&g, bears), (3, 3)));
    assert_eq!(g.obj(bears).chars.pt, Some((2, 2)), "what is printed is unchanged");
    // it and an effect of a resolved spell apply together
    cast(&mut g, GROWTH, FOREST, 1, vec![vec![Target::Object(bears)]], "G");
    all_pass(&mut g);
    cite!(["611.3", "613.4c"]; assert_eq!(pt(&g, bears), (6, 6)));
}

#[test]
fn r613_4_within_layer_7_the_sublayers_come_first_and_timestamps_within_each() {
    // "+3/+3" (7c) has the earlier timestamp, "Non-Archon creatures have base power and
    // toughness 3/3." (7b) the later: 7b is still applied first
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let mancer = g.put(0, GIGANTOMANCER, Zone::Battlefield);
    cast(&mut g, GROWTH, FOREST, 1, vec![vec![Target::Object(bears)]], "G");
    all_pass(&mut g);
    assert_eq!(pt(&g, bears), (5, 5));
    g.put(1, ARCHON, Zone::Battlefield);
    cite!(["613.4"]; assert_eq!(pt(&g, bears), (6, 6), "3/3, then +3/+3"));
    // within 7b, the later 7/7 after the 3/3
    seven(&mut g, mancer, bears);
    all_pass(&mut g);
    cite!(["613.4", "613.7"]; assert_eq!(pt(&g, bears), (10, 10), "3/3, then 7/7, then +3/+3"));
}

#[test]
fn r613_7a_a_static_abilitys_effect_has_the_timestamp_of_its_object() {
    // "Non-Archon creatures have base power and toughness 3/3." against "has base power and
    // toughness 7/7 until end of turn": the later of the two is applied last
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let mancer = g.put(0, GIGANTOMANCER, Zone::Battlefield);
    let archon = g.put(1, ARCHON, Zone::Battlefield);
    seven(&mut g, mancer, bears);
    all_pass(&mut g);
    assert!(g.obj(archon).stamp < g.continuous[0].stamp);
    cite!(["613.7a"]; assert_eq!(pt(&g, bears), (7, 7)));
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let mancer = g.put(0, GIGANTOMANCER, Zone::Battlefield);
    seven(&mut g, mancer, bears);
    all_pass(&mut g);
    let archon = g.put(1, ARCHON, Zone::Battlefield);
    assert!(g.obj(archon).stamp > g.continuous[0].stamp);
    cite!(["613.7a"]; assert_eq!(pt(&g, bears), (3, 3), "the Archon came later: its effect is the later one"));
}

#[test]
fn r613_7b_a_resolved_effect_receives_its_timestamp_when_it_is_created() {
    // the ability is activated before the Archon arrives and resolves after: its effect is
    // created as it resolves, so it is the later one
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let mancer = g.put(0, GIGANTOMANCER, Zone::Battlefield);
    seven(&mut g, mancer, bears);
    let archon = g.put(1, ARCHON, Zone::Battlefield);
    assert_eq!(pt(&g, bears), (3, 3));
    all_pass(&mut g);
    cite!(["613.7b"]; assert!(g.continuous[0].stamp > g.obj(archon).stamp));
    cite!(["613.7b"]; assert_eq!(pt(&g, bears), (7, 7)));
}

#[test]
fn r613_7d_an_object_receives_a_timestamp_when_it_enters_a_zone() {
    // the Archon card is in the hand before the 7/7 begins and enters the battlefield after:
    // its timestamp is the battlefield's, so its 3/3 is the later effect
    let mut g = game();
    g.put(0, BEARS, Zone::Library);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let mancer = g.put(0, GIGANTOMANCER, Zone::Battlefield);
    let in_hand = g.put(0, ARCHON, Zone::Hand);
    seven(&mut g, mancer, bears);
    all_pass(&mut g);
    assert_eq!(pt(&g, bears), (7, 7));
    mana(&mut g, PLAINS, 6);
    g.act(0, paid("WWWWWW", Move::Cast { card: in_hand, targets: vec![] })).unwrap();
    let on_stack = *g.stack.last().unwrap();
    all_pass(&mut g);
    let archon = permanent(&g, "Harmonious Archon");
    cite!(["613.7d"]; assert!(g.obj(in_hand).stamp < g.obj(on_stack).stamp && g.obj(on_stack).stamp < g.obj(archon).stamp, "a new one in each zone"));
    assert!(g.obj(archon).stamp > g.continuous[0].stamp);
    cite!(["613.7d", "613.7a"]; assert_eq!(pt(&g, bears), (3, 3)));
}

#[test]
fn r613_8_an_effect_that_depends_on_another_is_applied_after_it() {
    // "Creatures you control with first strike have double strike." depends on "Creatures you
    // control have first strike.": applying the Knighthood's changes what Kwende's applies to
    // (613.8a), so it is applied after it, whatever their timestamps
    for knighthood_first in [true, false] {
        let mut g = game();
        if knighthood_first {
            g.put(0, KNIGHTHOOD, Zone::Battlefield);
            g.put(0, KWENDE, Zone::Battlefield);
        } else {
            g.put(0, KWENDE, Zone::Battlefield);
            g.put(0, KNIGHTHOOD, Zone::Battlefield);
        }
        let bears = g.put(0, BEARS, Zone::Battlefield);
        assert!(g.has(bears, "First strike"));
        cite!(["613.8", "613.8a"]; case: format!("{knighthood_first}"); assert!(g.has(bears, "Double strike"), "Knighthood first: {knighthood_first}"));
    }
}

#[test]
fn r614_1_a_replacement_effect_applies_as_the_event_happens_not_locked_in_ahead() {
    // "Permanents enter tapped.": the Orb arrives while the Bears is on the stack, and the
    // Bears enters tapped
    let mut g = game();
    cast(&mut g, BEARS, FOREST, 2, vec![], "GG");
    g.put(1, ORB, Zone::Battlefield);
    all_pass(&mut g);
    cite!(["614.1"]; assert!(g.obj(permanent(&g, "Grizzly Bears")).tapped));
    // the Orb leaves while it is on the stack, and it enters untapped
    let mut g = game();
    let orb = g.put(1, ORB, Zone::Battlefield);
    cast(&mut g, BEARS, FOREST, 2, vec![], "GG");
    g.destroy(orb);
    all_pass(&mut g);
    cite!(["614.1"]; assert!(!g.obj(permanent(&g, "Grizzly Bears")).tapped));
}

#[test]
fn r614_5_a_replacement_effect_gets_only_one_opportunity_to_affect_an_event() {
    // "Each other non-Human creature you control enters with an additional +1/+1 counter on
    // it.": the Watchdog's entering with three counters is modified to four, and that event
    // is not modified again
    let mut g = game();
    g.put(0, GRUMGULLY, Zone::Battlefield);
    let card = g.put(0, WATCHDOG, Zone::Hand);
    mana(&mut g, FOREST, 1);
    mana(&mut g, PLAINS, 1);
    g.act(0, paid("WG", Move::Cast { card, targets: vec![] })).unwrap();
    all_pass(&mut g);
    let Some(Decision::Choose { player, choice: game::state::Choice::Replacement { among, .. } }) = g.decision.clone() else {
        panic!("the affected controller orders the two entry effects");
    };
    g.act(player, Move::Choose(game::state::Answer::Replacement(among[0].clone()))).unwrap();
    let dog = permanent(&g, "Faithful Watchdog");
    cite!(["614.5"]; assert_eq!(g.obj(dog).counters.get("+1/+1"), Some(&4)));
    // a creature that enters with none gets one
    cast(&mut g, BEARS, FOREST, 2, vec![], "GG");
    all_pass(&mut g);
    assert_eq!(g.obj(permanent(&g, "Grizzly Bears")).counters.get("+1/+1"), Some(&1));
}

#[test]
fn r614_6_a_replaced_event_never_happens_and_the_modified_one_may_trigger_abilities() {
    // "Permanents enter tapped." and "Whenever another creature you control enters, put a
    // +1/+1 counter on that creature."
    let mut g = game();
    g.put(1, ORB, Zone::Battlefield);
    g.put(0, UNICORN, Zone::Battlefield);
    cast(&mut g, BEARS, FOREST, 2, vec![], "GG");
    all_pass(&mut g);
    let bears = permanent(&g, "Grizzly Bears");
    assert!(g.obj(bears).tapped);
    cite!(["614.6"]; assert!(!g.log.iter().any(|e| matches!(e, Event::Tapped { object } if *object == bears)), "it never entered untapped"));
    // its entering tapped is an entering: the Unicorn's ability triggered
    cite!(["614.6"]; assert_eq!(g.stack.len(), 1));
    all_pass(&mut g);
    assert_eq!((pt(&g, bears), g.obj(bears).tapped), ((3, 3), true));
}
