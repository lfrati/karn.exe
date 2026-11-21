//! +1/+1 counters a permanent enters with, and the replacement effects said of counters
//! being put (122.6, 614.12, 616.1).
//!
//! 122.6: "Some spells and abilities refer to counters being put on an object. This refers to
//! putting counters on that object while it's on the battlefield and also to an object that's
//! given counters as it enters the battlefield."
//! 614.1c, 603.6d: "[This permanent] enters with …" is a replacement effect of a static
//! ability, "whose effect occurs as part of the event that puts the permanent onto the
//! battlefield". It is not a self-replacement effect (614.15: those are effects of a
//! resolving spell or ability), so 616.1a does not put it first.
//! 614.12: which effects apply is checked of "the permanent as it would exist on the
//! battlefield, taking into account replacement effects that have already modified how it
//! enters", and "continuous effects that already exist": a permanent entering at the same
//! time is not yet a source of one (Grumgully's ruling: "Any other non-Human creatures that
//! enter the battlefield at the same time as Grumgully won't get a +1/+1 counter").
//! 614.12a: a choice is "made before the permanent enters the battlefield".
//! 616.1, 616.1e, 616.1f: the controller of the affected object chooses one applicable
//! effect, applies it, and repeats with those now applicable. 616.2: an effect "can become
//! applicable to an event as the result of another replacement … effect that modifies the
//! event". 614.5: each "gets only one opportunity".
//!
//! What follows for the order. "If one or more +1/+1 counters would be put on a creature"
//! (Hardened Scales, Branching Evolution) does not apply to an entry with no counter, and
//! comes to apply once an "enters with" effect has given it one. Where a permanent has its
//! own "enters with" and Grumgully also applies, both apply from the start and the multiplier
//! may be chosen between them: Faithful Watchdog (three) under Grumgully and Branching
//! Evolution enters with 8 (three, one more, doubled), 7 (three, doubled, one more) or 5
//! (one, doubled, three more) as its controller orders them. The counters "enters with"
//! effects add CANNOT be summed before a multiplier is chosen.
//!
//! Every definition below passes admission; fixture placement only prepares the starting
//! state. The tested spells and choices run through `Game::act`.
use crate::support::{cite, paid};
use game::legal::Asked;
use game::operation::ReplacementId;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, PlayerId, Target, Zone};

const SCALES: usize = 0;
const EVOLUTION: usize = 1;
const SERVANT: usize = 2;
const GRUMGULLY: usize = 3;
const REANIMATE: usize = 4;
const RISE: usize = 5;
const ALARM: usize = 6;
const BEAR: usize = 7;
const WATCHDOG: usize = 8;

fn game() -> Game {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).unwrap();
    let defs = vec![
        card("Hardened Scales", "{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.", None),
        card("Branching Evolution", "{2}{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.", None),
        card("Servant of the Scale", "{G}", "Creature — Human Soldier", "This creature enters with a +1/+1 counter on it.\nWhen this creature dies, put X +1/+1 counters on target creature you control, where X is the number of +1/+1 counters on this creature.", Some((0, 0))),
        card("Grumgully, the Generous", "{1}{R}{G}", "Legendary Creature — Goblin Shaman", "Each other non-Human creature you control enters with an additional +1/+1 counter on it.", Some((3, 3))),
        card("Reanimate", "{B}", "Sorcery", "Put target creature card from a graveyard onto the battlefield under your control. You lose life equal to that card's mana value.", None),
        card("Rise of the Dark Realms", "{7}{B}{B}", "Sorcery", "Put all creature cards from all graveyards onto the battlefield under your control.", None),
        card("Raise the Alarm", "{1}{W}", "Instant", "Create two 1/1 white Soldier creature tokens.", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Faithful Watchdog", "{G}{W}", "Creature — Dog", "Vigilance\nThis creature enters with three +1/+1 counters on it.", Some((0, 0))),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0));
    }
    let mut g = Game::new(defs, 2, 20);
    g.players[0].pool.colors = [10; 5];
    g
}

/// Everyone passes until the game asks something or the stack is empty.
fn pass(g: &mut Game) {
    for _ in 0..20 {
        if g.decision.is_some() || (g.stack.is_empty() && g.resolving().is_none()) {
            return;
        }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("the stack does not empty");
}

/// Cast this card from the first player's hand and have everyone pass.
fn cast(g: &mut Game, def: usize, targets: Vec<Vec<Target>>, spending: &str) {
    let card = g.put(0, def, Zone::Hand);
    g.act(0, paid(spending, Move::Cast { card, targets })).unwrap_or_else(|e| panic!("cast {}: {e}", g.defs[def].name()));
    pass(g);
}

/// The replacement question now asked, if one is: of whom, how many +1/+1 counters the
/// event has come to, and the sources of the effects offered.
fn question(g: &Game) -> Option<(PlayerId, u32, Vec<ReplacementId>)> {
    match &g.decision {
        Some(Decision::Choose { player, choice: Choice::Replacement { on: Target::Object(on), kind, count, among, optional: false, .. } }) => {
            assert_eq!(kind, "+1/+1");
            assert!(!g.battlefield.contains(on), "asked of a permanent before it enters (614.12a)");
            Some((*player, *count, among.clone()))
        }
        None => None,
        other => panic!("another question: {other:?}"),
    }
}

fn sources(among: &[ReplacementId]) -> Vec<ObjId> {
    let mut sources: Vec<ObjId> = among.iter().map(|r| r.source).collect();
    sources.sort();
    sources
}

fn sorted(mut objects: Vec<ObjId>) -> Vec<ObjId> {
    objects.sort();
    objects
}

/// Which effect to apply next.
#[derive(Clone, Copy, Debug)]
enum Pick {
    /// The effect of this permanent on the battlefield.
    Of(ObjId),
    /// The entering permanent's own "enters with": an effect of none of the permanents the
    /// test put on the battlefield.
    Own,
}

/// Answer every replacement question in this order of preference: of the effects offered,
/// the first of `order`. `known`: the permanents on the battlefield whose effects are in
/// play. How many questions were asked.
fn settle(g: &mut Game, known: &[ObjId], order: &[Pick]) -> usize {
    let mut asked = 0;
    while let Some((player, _, among)) = question(g) {
        asked += 1;
        let chosen = order
            .iter()
            .find_map(|pick| among.iter().find(|r| match pick {
                Pick::Of(source) => r.source == *source,
                Pick::Own => !known.contains(&r.source),
            }))
            .unwrap_or_else(|| panic!("none of {order:?} is offered: {among:?}"));
        g.act(player, Move::Choose(Answer::Replacement(chosen.clone()))).unwrap();
        pass(g);
        assert!(asked < 20, "the questions do not end (614.5)");
    }
    asked
}

fn counters(g: &Game, o: ObjId) -> u32 {
    g.obj(o).counters.get("+1/+1").copied().unwrap_or(0)
}

fn named(g: &Game, name: &str) -> Vec<ObjId> {
    g.battlefield.iter().copied().filter(|o| g.obj(*o).chars.name == name).collect()
}

/// The one permanent of this name.
fn one(g: &Game, name: &str) -> ObjId {
    match named(g, name)[..] {
        [one] => one,
        ref other => panic!("{} permanents named {name}", other.len()),
    }
}

/// The +1/+1 counters on each permanent of this name, fewest first.
fn counts(g: &Game, name: &str) -> Vec<u32> {
    let mut counts: Vec<u32> = named(g, name).into_iter().map(|o| counters(g, o)).collect();
    counts.sort();
    counts
}

fn tokens(g: &Game, from: usize) -> Vec<ObjId> {
    g.log[from..].iter().filter_map(|e| match e { Event::Created { token } => Some(*token), _ => None }).collect()
}

/// While a question about an entry is asked, nothing of that entry has happened: nothing
/// has entered or been created, no counter has been put, and no player has priority.
fn nothing_entered(g: &mut Game, from: usize) {
    let early: Vec<&Event> = g.log[from..]
        .iter()
        .filter(|e| matches!(e, Event::ZoneChanged { to: Zone::Battlefield, .. } | Event::Created { .. } | Event::CountersPut { .. } | Event::Resolved { .. }))
        .collect();
    assert!(early.is_empty(), "before the choice is made (614.12a): {early:?}");
    assert!(!matches!(g.asked(), Some((_, Asked::Priority))), "no player has priority during a resolution (117.1)");
    for p in 0..g.players.len() {
        crate::support::refused(g.clone().act(p, Move::Pass), "a decision is pending: a choice of how the permanent enters the battlefield (614.12)");
    }
}

// ---- one permanent, cast ----

#[test]
fn r122_6_a_counter_a_creature_enters_with_is_put_on_it_plus_one() {
    // one effect applies once the Servant's own has given it a counter: no question
    let mut g = game();
    g.put(0, SCALES, Zone::Battlefield);
    cast(&mut g, SERVANT, vec![], "G");
    assert_eq!(g.decision, None);
    let servant = one(&g, "Servant of the Scale");
    cite!(["122.6"]; assert_eq!((counters(&g, servant), g.power(servant)), (2, Some(2))));
}

#[test]
fn r122_6_an_opponents_replacement_is_not_said_of_this_creature() {
    // "a creature you control": the opponent's Hardened Scales does nothing
    let mut g = game();
    g.put(1, SCALES, Zone::Battlefield);
    cast(&mut g, SERVANT, vec![], "G");
    assert_eq!(g.decision, None);
    cite!(["109.5"]; assert_eq!(counts(&g, "Servant of the Scale"), [1]));
}

#[test]
fn r616_1_the_counters_a_cast_creature_enters_with_ask_which_replacement_first() {
    for (first, expected) in [(SCALES, 4), (EVOLUTION, 3)] {
        let mut g = game();
        let (scales, evolution) = (g.put(0, SCALES, Zone::Battlefield), g.put(0, EVOLUTION, Zone::Battlefield));
        let from = g.log.len();
        cast(&mut g, SERVANT, vec![], "G");
        // its own counter is there before either is said of it (616.2): one counter is asked about
        let (player, count, among) = question(&g).expect("two effects apply to one counter");
        cite!(["616.1", "616.2"]; case: format!("first {first}"); assert_eq!((player, count, sources(&among)), (0, 1, sorted(vec![scales, evolution]))));
        cite!(["614.12a"]; case: format!("first {first}"); assert!(named(&g, "Servant of the Scale").is_empty(), "it has not entered (614.12a)"));
        let entered = |e: &Event| matches!(e, Event::ZoneChanged { to: Zone::Battlefield, .. } | Event::CountersPut { .. });
        assert!(!g.log[from..].iter().any(entered), "nothing of the entry has happened");
        assert!(!matches!(g.asked(), Some((_, Asked::Priority))));
        let chosen = among.iter().find(|r| r.source == if first == SCALES { scales } else { evolution }).unwrap().clone();
        // its controller chooses
        cite!(["616.1"]; refused("that decision is another player's"): g.clone().act(1, Move::Choose(Answer::Replacement(chosen.clone()))));
        g.act(0, Move::Choose(Answer::Replacement(chosen))).unwrap();
        pass(&mut g);
        assert_eq!(g.decision, None, "the effect left applies without a question (614.5)");
        let servant = one(&g, "Servant of the Scale");
        cite!(["616.1f"]; case: format!("first {first}"); assert_eq!((counters(&g, servant), g.power(servant)), (expected, Some(expected as i32))));
    }
}

#[test]
fn r614_12_grumgully_gives_no_counter_to_a_human() {
    // "Each other non-Human creature you control": Servant of the Scale is a Human Soldier
    let mut g = game();
    g.put(0, GRUMGULLY, Zone::Battlefield);
    cast(&mut g, SERVANT, vec![], "G");
    assert_eq!(g.decision, None);
    cite!(["614.12"]; assert_eq!(counts(&g, "Servant of the Scale"), [1]));
}

#[test]
fn r616_2_an_additional_counter_makes_the_counter_replacements_apply() {
    // a Bear has no counter of its own: only Grumgully's effect applies, then both of the
    // others apply to its one counter
    for (first, expected) in [(SCALES, 4), (EVOLUTION, 3)] {
        let mut g = game();
        let grumgully = g.put(0, GRUMGULLY, Zone::Battlefield);
        let (scales, evolution) = (g.put(0, SCALES, Zone::Battlefield), g.put(0, EVOLUTION, Zone::Battlefield));
        cast(&mut g, BEAR, vec![], "WG");
        let (player, count, among) = question(&g).expect("two effects apply to Grumgully's counter");
        cite!(["616.2"]; case: format!("first {first}"); assert_eq!((player, count, sources(&among)), (0, 1, sorted(vec![scales, evolution])), "Grumgully's has applied: nothing else did"));
        let order = if first == SCALES { [Pick::Of(scales), Pick::Of(evolution)] } else { [Pick::Of(evolution), Pick::Of(scales)] };
        assert_eq!(settle(&mut g, &[grumgully, scales, evolution], &order), 1);
        let bear = one(&g, "Grizzly Bears");
        cite!(["616.1f"]; case: format!("first {first}"); assert_eq!((counters(&g, bear), g.power(bear)), (expected, Some(2 + expected as i32))));
        assert_eq!(counters(&g, grumgully), 0);
    }
}

#[test]
fn r616_2_one_counter_replacement_after_grumgully_asks_nothing() {
    let mut g = game();
    g.put(0, GRUMGULLY, Zone::Battlefield);
    g.put(0, SCALES, Zone::Battlefield);
    cast(&mut g, BEAR, vec![], "WG");
    assert_eq!(g.decision, None);
    cite!(["616.2"]; assert_eq!(counts(&g, "Grizzly Bears"), [2]));
}

// ---- a creature's own counters, Grumgully's, and a multiplier: the order is chosen ----

#[test]
fn r616_1e_own_counters_and_grumgully_without_a_multiplier_come_to_their_sum() {
    // whichever is applied first, and whether or not that is asked: three and one
    let mut g = game();
    let grumgully = g.put(0, GRUMGULLY, Zone::Battlefield);
    cast(&mut g, WATCHDOG, vec![], "GW");
    settle(&mut g, &[grumgully], &[Pick::Own, Pick::Of(grumgully)]);
    cite!(["616.1e"]; assert_eq!(counts(&g, "Faithful Watchdog"), [4]));
}

#[test]
fn r616_1e_a_multiplier_may_be_chosen_between_own_counters_and_grumgully() {
    // Faithful Watchdog (three) under Grumgully (one more) and Branching Evolution (twice):
    // the counters the two "enters with" effects add are not summed before the doubling
    let mut g = game();
    let grumgully = g.put(0, GRUMGULLY, Zone::Battlefield);
    let evolution = g.put(0, EVOLUTION, Zone::Battlefield);
    let known = [grumgully, evolution];
    let (own, more, twice) = (Pick::Own, Pick::Of(grumgully), Pick::Of(evolution));
    // the doubling is not offered while the entry has no counter (616.2), so the first of
    // these that is offered is taken each time
    for (order, expected) in [
        ([own, more, twice], 8),  // 3, 4, 8
        ([more, own, twice], 8),  // 1, 4, 8
        ([own, twice, more], 7),  // 3, 6, 7
        ([more, twice, own], 5),  // 1, 2, 5
    ] {
        let mut g = g.clone();
        let from = g.log.len();
        cast(&mut g, WATCHDOG, vec![], "GW");
        let (player, count, among) = question(&g).expect("its own effect and Grumgully's both apply");
        assert_eq!((player, count), (0, 0), "no counter yet");
        cite!(["616.2"]; assert!(!sources(&among).contains(&evolution), "nothing to double yet (616.2)"));
        assert!(sources(&among).contains(&grumgully) && among.len() == 2, "{among:?}");
        nothing_entered(&mut g, from);
        settle(&mut g, &known, &order);
        let watchdog = one(&g, "Faithful Watchdog");
        cite!(["616.1e", "616.1f"]; case: format!("{order:?}"); assert_eq!((counters(&g, watchdog), g.power(watchdog)), (expected, Some(expected as i32)), "{order:?}"));
    }
}

#[test]
fn r616_1f_own_counters_grumgully_and_two_multipliers_each_apply_once() {
    // every effect has one opportunity (614.5); the greatest and the least orderings
    let mut g = game();
    let grumgully = g.put(0, GRUMGULLY, Zone::Battlefield);
    let (scales, evolution) = (g.put(0, SCALES, Zone::Battlefield), g.put(0, EVOLUTION, Zone::Battlefield));
    let known = [grumgully, scales, evolution];
    let (own, more, plus, twice) = (Pick::Own, Pick::Of(grumgully), Pick::Of(scales), Pick::Of(evolution));
    for (order, expected) in [
        ([own, more, plus, twice], 10), // 3, 4, 5, 10
        ([own, plus, twice, more], 9),  // 3, 4, 8, 9
        ([more, twice, plus, own], 6),  // 1, 2, 3, 6
        ([more, plus, twice, own], 7),  // 1, 2, 4, 7
    ] {
        let mut g = g.clone();
        cast(&mut g, WATCHDOG, vec![], "GW");
        settle(&mut g, &known, &order);
        cite!(["614.5", "616.1f"]; case: format!("{order:?}"); assert_eq!(counts(&g, "Faithful Watchdog"), [expected], "{order:?}"));
    }
}

// ---- put onto the battlefield under another player's control ----

#[test]
fn r616_1_a_reanimated_creature_is_asked_about_by_the_player_it_enters_under() {
    // player 1's Servant, in player 1's graveyard, enters under player 0: player 0's
    // effects apply and player 0 chooses; player 1's Hardened Scales is not said of it
    let mut g = game();
    let (scales, evolution) = (g.put(0, SCALES, Zone::Battlefield), g.put(0, EVOLUTION, Zone::Battlefield));
    g.put(1, SCALES, Zone::Battlefield);
    let card = g.put(1, SERVANT, Zone::Graveyard);
    let from = g.log.len();
    cast(&mut g, REANIMATE, vec![vec![Target::Object(card)]], "B");
    let (player, count, among) = question(&g).expect("player 0's two effects apply");
    cite!(["616.1"]; assert_eq!((player, count, sources(&among)), (0, 1, sorted(vec![scales, evolution]))));
    cite!(["614.12a"]; assert_eq!(g.obj(card).zone, Zone::Graveyard, "it has not left the graveyard"));
    assert_eq!(g.players[0].life, 20, "nothing after the entry has happened");
    nothing_entered(&mut g, from);
    let twice = among.iter().find(|r| r.source == evolution).unwrap().clone();
    cite!(["616.1"]; refused("that decision is another player's"): g.clone().act(1, Move::Choose(Answer::Replacement(twice.clone()))));
    g.act(0, Move::Choose(Answer::Replacement(twice))).unwrap();
    pass(&mut g);
    assert_eq!(g.decision, None);
    let servant = one(&g, "Servant of the Scale");
    cite!(["616.1f"]; assert_eq!((g.obj(servant).controller, g.obj(servant).owner, counters(&g, servant)), (0, 1, 3), "doubled, then one more"));
    assert_eq!(g.players[0].life, 19, "mana value 1");
}

#[test]
fn r122_6_only_the_new_controllers_replacements_apply_to_a_reanimated_creature() {
    // its owner controls both replacements, the player it enters under controls none
    let mut g = game();
    g.put(1, SCALES, Zone::Battlefield);
    g.put(1, EVOLUTION, Zone::Battlefield);
    let card = g.put(1, SERVANT, Zone::Graveyard);
    cast(&mut g, REANIMATE, vec![vec![Target::Object(card)]], "B");
    assert_eq!(g.decision, None);
    let servant = one(&g, "Servant of the Scale");
    cite!(["614.12"]; assert_eq!((g.obj(servant).controller, counters(&g, servant)), (0, 1)));
}

// ---- several permanents at the same time ----

#[test]
fn r614_12a_simultaneous_returns_are_all_asked_about_before_any_enters() {
    // two Servants and a Bear return together under player 0, who controls both
    // replacements: a question for each Servant, and nothing enters until both are answered
    let mut g = game();
    let (scales, evolution) = (g.put(0, SCALES, Zone::Battlefield), g.put(0, EVOLUTION, Zone::Battlefield));
    let cards = [g.put(0, SERVANT, Zone::Graveyard), g.put(1, SERVANT, Zone::Graveyard), g.put(1, BEAR, Zone::Graveyard)];
    let from = g.log.len();
    cast(&mut g, RISE, vec![], "WWWWWWWBB");
    for first in [scales, evolution] {
        let (player, count, among) = question(&g).expect("a question for each Servant");
        assert_eq!((player, count, sources(&among)), (0, 1, sorted(vec![scales, evolution])));
        cite!(["614.12a"]; case: format!("first {first:?}"); assert!(cards.iter().all(|c| g.obj(*c).zone == Zone::Graveyard), "none has left its graveyard"));
        assert!(named(&g, "Servant of the Scale").is_empty() && named(&g, "Grizzly Bears").is_empty());
        nothing_entered(&mut g, from);
        let chosen = among.iter().find(|r| r.source == first).unwrap().clone();
        g.act(0, Move::Choose(Answer::Replacement(chosen))).unwrap();
        pass(&mut g);
    }
    assert_eq!(g.decision, None, "the effect left applies to each without a question (614.5)");
    // one Scales first (1, 2, 4), one Evolution first (1, 2, 3); each chosen for itself
    cite!(["616.1"]; assert_eq!(counts(&g, "Servant of the Scale"), [3, 4]));
    assert_eq!(counts(&g, "Grizzly Bears"), [0]);
    assert!(named(&g, "Servant of the Scale").into_iter().chain(named(&g, "Grizzly Bears")).all(|o| g.obj(o).controller == 0));
    let arrivals = g.log[from..].iter().filter(|e| matches!(e, Event::ZoneChanged { to: Zone::Battlefield, from: Zone::Graveyard, .. })).count();
    assert_eq!(arrivals, 3);
}

#[test]
fn r614_12_a_grumgully_arriving_with_others_gives_them_no_counter() {
    // Grumgully, a Bear and a Watchdog return together: its effect does not exist yet as
    // they enter, so there is no additional counter, and Hardened Scales has nothing to add
    // to on the Bear
    let mut g = game();
    g.put(0, SCALES, Zone::Battlefield);
    g.put(0, GRUMGULLY, Zone::Graveyard);
    g.put(1, BEAR, Zone::Graveyard);
    g.put(0, WATCHDOG, Zone::Graveyard);
    cast(&mut g, RISE, vec![], "WWWWWWWBB");
    assert_eq!(g.decision, None, "one effect applies to the Watchdog's own counters, none to the others");
    cite!(["614.12"]; assert_eq!(counts(&g, "Grizzly Bears"), [0]));
    cite!(["614.12"]; assert_eq!(counts(&g, "Faithful Watchdog"), [4], "three, plus one"));
    assert_eq!(counts(&g, "Grumgully, the Generous"), [0]);
}

#[test]
fn r614_12_a_grumgully_already_there_gives_each_returning_non_human_a_counter() {
    // the same return under a Grumgully on the battlefield: the Bear gets one, the Human none
    let mut g = game();
    let grumgully = g.put(0, GRUMGULLY, Zone::Battlefield);
    g.put(1, BEAR, Zone::Graveyard);
    g.put(1, BEAR, Zone::Graveyard);
    g.put(0, SERVANT, Zone::Graveyard);
    cast(&mut g, RISE, vec![], "WWWWWWWBB");
    assert_eq!(g.decision, None);
    cite!(["614.12"]; assert_eq!(counts(&g, "Grizzly Bears"), [1, 1]));
    assert_eq!(counts(&g, "Servant of the Scale"), [1]);
    assert_eq!(counters(&g, grumgully), 0);
}

#[test]
fn r614_12_an_opponents_grumgully_gives_nothing_to_what_enters_under_another_player() {
    let mut g = game();
    g.put(1, GRUMGULLY, Zone::Battlefield);
    g.put(1, BEAR, Zone::Graveyard);
    cast(&mut g, RISE, vec![], "WWWWWWWBB");
    assert_eq!(g.decision, None);
    cite!(["614.12"]; assert_eq!(counts(&g, "Grizzly Bears"), [0], "it enters under player 0's control"));
}

// ---- tokens ----

#[test]
fn r614_12_tokens_created_under_grumgully_enter_with_a_counter() {
    // Soldier tokens are not Humans
    let mut g = game();
    let grumgully = g.put(0, GRUMGULLY, Zone::Battlefield);
    let from = g.log.len();
    cast(&mut g, ALARM, vec![], "WW");
    assert_eq!(g.decision, None);
    let soldiers = tokens(&g, from);
    assert_eq!(soldiers.len(), 2);
    for s in soldiers {
        cite!(["614.1d"]; assert_eq!((counters(&g, s), g.power(s), g.obj(s).zone), (1, Some(2), Zone::Battlefield)));
    }
    assert_eq!(counters(&g, grumgully), 0);
}

#[test]
fn r616_2_tokens_created_under_grumgully_and_one_counter_replacement() {
    let mut g = game();
    g.put(0, GRUMGULLY, Zone::Battlefield);
    g.put(0, SCALES, Zone::Battlefield);
    let from = g.log.len();
    cast(&mut g, ALARM, vec![], "WW");
    assert_eq!(g.decision, None);
    let soldiers = tokens(&g, from);
    cite!(["616.2"]; assert_eq!(soldiers.iter().map(|s| counters(&g, *s)).collect::<Vec<_>>(), [2, 2]));
}

#[test]
fn r616_1_each_token_created_under_grumgully_is_asked_about_before_any_is_created() {
    // both tokens are created at the same time: a question for each, answered differently,
    // and no token exists until both are answered
    let mut g = game();
    g.put(0, GRUMGULLY, Zone::Battlefield);
    let (scales, evolution) = (g.put(0, SCALES, Zone::Battlefield), g.put(0, EVOLUTION, Zone::Battlefield));
    let from = g.log.len();
    cast(&mut g, ALARM, vec![], "WW");
    for first in [scales, evolution] {
        let (player, count, among) = question(&g).expect("a question for each token");
        assert_eq!((player, count, sources(&among)), (0, 1, sorted(vec![scales, evolution])));
        nothing_entered(&mut g, from);
        let chosen = among.iter().find(|r| r.source == first).unwrap().clone();
        g.act(0, Move::Choose(Answer::Replacement(chosen))).unwrap();
        pass(&mut g);
    }
    assert_eq!(g.decision, None);
    let mut put: Vec<u32> = tokens(&g, from).iter().map(|s| counters(&g, *s)).collect();
    put.sort();
    cite!(["616.1"]; assert_eq!(put, [3, 4]));
}

#[test]
fn r614_12_tokens_created_without_grumgully_have_no_counter_to_replace() {
    let mut g = game();
    g.put(0, SCALES, Zone::Battlefield);
    g.put(0, EVOLUTION, Zone::Battlefield);
    let from = g.log.len();
    cast(&mut g, ALARM, vec![], "WW");
    assert_eq!(g.decision, None);
    cite!(["614.7"]; assert_eq!(tokens(&g, from).iter().map(|s| counters(&g, *s)).collect::<Vec<_>>(), [0, 0]));
}

#[test]
fn r614_6_several_entry_effects_commit_one_counter_placement() {
    let mut g = game();
    let grumgully = g.put(0, GRUMGULLY, Zone::Battlefield);
    cast(&mut g, WATCHDOG, vec![], "GW");
    settle(&mut g, &[grumgully], &[Pick::Own, Pick::Of(grumgully)]);
    let watchdog = one(&g, "Faithful Watchdog");
    let placed: Vec<_> = g.log.iter().filter_map(|e| match e {
        Event::CountersPut { on: Target::Object(on), kind, count, .. } if *on == watchdog => Some((kind.as_str(), *count)),
        _ => None,
    }).collect();
    cite!(["614.6"]; assert_eq!(placed, [("+1/+1", 4)]));
}

#[test]
fn r614_12a_an_entry_order_can_be_cloned_and_bad_answers_do_not_enter_anything() {
    let mut g = game();
    let scales = g.put(0, SCALES, Zone::Battlefield);
    let evolution = g.put(0, EVOLUTION, Zone::Battlefield);
    let from = g.log.len();
    cast(&mut g, SERVANT, vec![], "G");
    let decision = g.decision.clone();
    let log = g.log.clone();
    cite!(["616.1"]; refused("choose one of the replacement effects that apply (616.1)"): g.act(0, Move::Choose(Answer::Replacement(ReplacementId::from_ability(scales, game::binding::AbilityKey::printed(17), 0)))));
    assert_eq!(g.decision, decision);
    assert_eq!(g.log, log);
    nothing_entered(&mut g, from);
    let mut alternate = g.clone();
    settle(&mut g, &[scales, evolution], &[Pick::Of(scales), Pick::Of(evolution)]);
    settle(&mut alternate, &[scales, evolution], &[Pick::Of(evolution), Pick::Of(scales)]);
    cite!(["616.1"]; assert_eq!(counts(&g, "Servant of the Scale"), [4]));
    cite!(["616.1"]; assert_eq!(counts(&alternate, "Servant of the Scale"), [3]));
    assert_eq!(named(&g, "Servant of the Scale").len(), 1);
}

fn add_definition(g: &mut Game, def: CardDef) -> usize {
    game::play::check(&def).unwrap_or_else(|u| panic!("{}: {}", def.name(), u.0));
    let index = g.defs.len();
    std::sync::Arc::make_mut(&mut g.defs).push(def);
    index
}

#[test]
fn r305_1_playing_a_creature_land_waits_for_entry_replacements_then_counts_as_played() {
    let mut g = game();
    let arbor = add_definition(&mut g, CardDef::new("Dryad Arbor", "", "Land Creature — Forest Dryad", "", Some((1, 1))).unwrap());
    let grumgully = g.put(0, GRUMGULLY, Zone::Battlefield);
    let scales = g.put(0, SCALES, Zone::Battlefield);
    let evolution = g.put(0, EVOLUTION, Zone::Battlefield);
    let card = g.put(0, arbor, Zone::Hand);
    let from = g.log.len();
    g.act(0, Move::PlayLand(card)).unwrap();
    assert!(question(&g).is_some());
    assert!(!g.players[0].land_played);
    cite!(["614.12a"]; assert_eq!(g.obj(card).zone, Zone::Hand));
    nothing_entered(&mut g, from);
    settle(&mut g, &[grumgully, scales, evolution], &[Pick::Of(scales), Pick::Of(evolution)]);
    cite!(["616.1f"]; assert_eq!(counts(&g, "Dryad Arbor"), [4]));
    assert!(g.players[0].land_played);
    assert_eq!(g.log[from..].iter().filter(|e| matches!(e, Event::LandPlayed { .. })).count(), 1);
}

#[test]
fn r610_3_return_counter_choices_finish_before_the_next_instruction() {
    let mut g = game();
    let prison_def = add_definition(&mut g, CardDef::new("Prison fixture", "{0}", "Enchantment",
        "{0}: Exile target permanent until this enchantment leaves the battlefield.", None).unwrap());
    let release_def = add_definition(&mut g, CardDef::new("Release fixture", "{0}", "Instant",
        "Destroy target enchantment. You gain 2 life.", None).unwrap());
    let scales = g.put(0, SCALES, Zone::Battlefield);
    let evolution = g.put(0, EVOLUTION, Zone::Battlefield);
    let servant = g.put(0, SERVANT, Zone::Battlefield);
    g.put_counters(servant, "+1/+1", 1);
    let prison = g.put(0, prison_def, Zone::Battlefield);
    g.act(0, Move::Activate { source: prison, ability: 0, targets: vec![vec![Target::Object(servant)]], paying: vec![] }).unwrap();
    pass(&mut g);
    let exiled = g.exile[0];
    cast(&mut g, release_def, vec![vec![Target::Object(prison)]], "");
    assert!(question(&g).is_some());
    cite!(["614.12a"]; assert_eq!(g.obj(exiled).zone, Zone::Exile));
    cite!(["610.3", "608.2c"]; assert_eq!(g.players[0].life, 20));
    settle(&mut g, &[scales, evolution], &[Pick::Of(evolution), Pick::Of(scales)]);
    assert_eq!(counts(&g, "Servant of the Scale"), [3]);
    assert_eq!(g.players[0].life, 22);
    assert!(g.exile.is_empty());
}

#[test]
fn r101_4_entry_replacement_choices_follow_active_player_order_in_three_player_games() {
    let base = game();
    let mut g = Game::new((*base.defs).clone(), 3, 20);
    let fixture = add_definition(&mut g, CardDef::new("Artifact entry fixture", "{1}", "Artifact Creature — Construct",
        "This creature enters with a +1/+1 counter on it.", Some((1, 1))).unwrap());
    let vaults = add_definition(&mut g, CardDef::new("Open the Vaults", "{4}{W}{W}", "Sorcery",
        "Return all artifact and enchantment cards from all graveyards to the battlefield under their owners' control.", None).unwrap());
    let mut effects = Vec::new();
    for player in 0..3 {
        effects.push((g.put(player, SCALES, Zone::Battlefield), g.put(player, EVOLUTION, Zone::Battlefield)));
    }
    let cards: Vec<_> = [0, 2, 1].into_iter().map(|p| g.put(p, fixture, Zone::Graveyard)).collect();
    g.active = 1;
    g.priority = 1;
    g.players[1].pool.colors = [10; 5];
    let spell = g.put(1, vaults, Zone::Hand);
    g.act(1, paid("WWWWWW", Move::Cast { card: spell, targets: vec![] })).unwrap();
    pass(&mut g);
    for (i, player) in [1, 2, 0].into_iter().enumerate() {
        let (asked, count, among) = question(&g).expect("one order per owner");
        cite!(["616.1", "101.4"]; case: format!("player {player}"); assert_eq!((asked, count), (player, 1)));
        assert_eq!(g.public_choices().count(), i);
        assert!(cards.iter().all(|c| g.obj(*c).zone == Zone::Graveyard));
        assert_eq!(sources(&among), sorted(vec![effects[player].0, effects[player].1]));
        let effect = among.iter().find(|id| id.source == effects[player].0).unwrap().clone();
        g.act(player, Move::Choose(Answer::Replacement(effect))).unwrap();
        pass(&mut g);
    }
    assert_eq!(counts(&g, "Artifact entry fixture"), [4, 4, 4]);
    assert!(g.public_choices().next().is_none());
}

#[test]
fn r614_12_a_creatures_general_replacement_is_not_active_during_its_own_entry() {
    let mut g = game();
    g.defs = std::sync::Arc::new({
        let mut defs = g.defs.as_ref().clone();
        defs.push(CardDef::new("Corpsejack Menace", "{2}{B}{G}", "Creature — Fungus",
            "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on it instead.", Some((4, 4))).unwrap());
        defs
    });
    let corpsejack = g.defs.len() - 1;
    game::play::check(&g.defs[corpsejack]).unwrap();
    g.put(0, GRUMGULLY, Zone::Battlefield);
    cast(&mut g, corpsejack, vec![], "WWBG");
    let arrived = *g.battlefield.last().unwrap();
    assert_eq!(g.obj(arrived).card, Some(corpsejack));
    cite!(["614.12"]; assert_eq!(g.obj(arrived).counters.get("+1/+1"), Some(&1)));
    cast(&mut g, BEAR, vec![], "WG");
    let bear = *g.battlefield.last().unwrap();
    cite!(["616.2"]; assert_eq!(g.obj(bear).counters.get("+1/+1"), Some(&2)));
}
