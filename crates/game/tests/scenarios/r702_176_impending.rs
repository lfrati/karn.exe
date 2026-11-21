//! Impending (702.176a): "Impending N—[cost]" is an alternative cost of casting the spell
//! (601.2b, 118.9); if it was paid the permanent enters with N time counters, is not a
//! creature as long as it has one (613.1d), and at the beginning of its controller's end step
//! a time counter is removed from it. Each of the last three asks whether the impending cost
//! was paid, which a new object never was (400.7) and a copy of the spell was (707.10).
use crate::support::cite;
use game::announcement::CastCost;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::{CardType, Part};

const MISTMOORS: usize = 0;
const PROBE: usize = 1;
const PROLIFERATE: usize = 2;
const REMOVE: usize = 3;
const PUT: usize = 4;
const FLICKER: usize = 5;
const DESTROY: usize = 6;
const COPY_SPELL: usize = 7;
const COPY_TOKEN: usize = 8;
const EXILE: usize = 9;
const ISLAND: usize = 10;
const BALEMURK: usize = 11;
const WATCHER: usize = 12;
// the row of each card's impending keyword
const IMPENDING_ROW: usize = 0;

fn defs() -> Vec<CardDef> {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap();
    let c = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        real("Overlord of the Mistmoors"),
        c("Probe Horror", "{5}", "Enchantment Creature — Horror", "Impending 1—{0}", Some((3, 3))),
        c("Spread Probe", "{0}", "Instant", "Proliferate.", None),
        c("Unwind Probe", "{0}", "Instant", "Remove a time counter from target permanent.", None),
        c("Wind Probe", "{0}", "Instant", "Put a time counter on target permanent.", None),
        c("Blink Probe", "{0}", "Instant", "Exile target permanent you control, then return that card to the battlefield under its owner's control.", None),
        c("Kill Probe", "{0}", "Instant", "Destroy target creature.", None),
        c("Echo Probe", "{0}", "Instant", "Copy target creature spell.", None),
        c("Mirror Probe", "{0}", "Instant", "Create a token that's a copy of target permanent.", None),
        c("Banish Probe", "{0}", "Instant", "Exile target permanent.", None),
        c("Island", "", "Basic Land — Island", "", None),
        real("Overlord of the Balemurk"),
        c("Watcher Probe", "{0}", "Enchantment", "Whenever a creature you control enters, you gain 1 life.", None),
    ]
}

fn game() -> Game {
    let defs = defs();
    for d in defs.iter().filter(|d| d.name() != "Overlord of the Balemurk") { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..20 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
/// The alternative costs the legal casts of this card offer, by row.

/// Answer what is asked and pass until the stack is empty: triggers in the order listed, a
/// proliferate choice of every holder offered.
fn settle(g: &mut Game) {
    for _ in 0..100 {
        match g.decision.clone() {
            Some(Decision::Order { player, abilities }) => g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(),
            Some(Decision::Choose { player, choice: Choice::Holders { among, .. } }) => g.act(player, Move::Choose(Answer::Holders(among))).unwrap(),
            Some(d) => panic!("unexpected decision {d:?}"),
            None if g.stack.is_empty() => return,
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("did not settle");
}
/// Cast the card for its impending cost, or for its mana cost, with what pays either, and
/// resolve it: the permanent it becomes.
fn cast(g: &mut Game, def: usize, impending: bool, mana: &str) -> ObjId {
    let card = g.put(0, def, Zone::Hand);
    g.players[0].pool.colors[0] = 2;
    g.players[0].pool.colorless = 5;
    let cast = Move::Cast { card, targets: vec![] };
    g.act(0, crate::support::paid(mana, if impending { Move::CastFor(CastCost::Alternative(IMPENDING_ROW), Box::new(cast)) } else { cast })).unwrap();
    g.players[0].pool = Default::default();
    settle(g);
    *g.battlefield.iter().rev().find(|o| g.obj(**o).card == Some(def)).expect("it resolved")
}
fn spell(g: &mut Game, by: usize, def: usize, target: Option<ObjId>) {
    while g.priority != by { g.act(g.priority, Move::Pass).unwrap(); }
    let card = g.put(by, def, Zone::Hand);
    g.act(by, Move::Cast { card, targets: target.map(|t| vec![vec![Target::Object(t)]]).unwrap_or_default() }).unwrap();
}
fn time(g: &Game, o: ObjId) -> u32 { g.obj(o).counters.get("time").copied().unwrap_or(0) }
fn creature(g: &Game, o: ObjId) -> bool { g.seen(o, None).types.contains(&CardType::Creature) }
fn insects(g: &Game) -> usize { g.battlefield.iter().filter(|o| g.obj(**o).card.is_none() && g.seen(**o, None).pt == Some((2, 1))).count() }
/// Pass to the beginning of this player's next end step, as its triggers are put on the stack.
fn end_step(g: &mut Game, player: usize) {
    let (turn, at_end) = (g.turn, g.part == Part::End);
    loop {
        if g.part == Part::End && g.active == player && !(at_end && g.turn == turn) { return; }
        match g.decision.clone() {
            Some(Decision::Order { player, abilities }) => g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(),
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(d) => panic!("unexpected decision {d:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
}
/// The triggered abilities on the stack whose source is this permanent, other than its own
/// enters trigger.
fn triggers_of(g: &Game, source: ObjId) -> usize {
    g.stack.iter().filter(|o| matches!(g.obj(**o).ability, Some((Some(s), _)) if s == source)).count()
}

#[test]
fn r702_176a_the_impending_cost_is_offered_beside_the_mana_cost() {
    let mut g = game();
    let card = g.put(0, MISTMOORS, Zone::Hand);
    g.players[0].pool.colors[0] = 2;
    g.players[0].pool.colorless = 5;
    cite!(["702.176a", "601.2b", "118.9"]; ok: g.clone().act(0, crate::support::paid("CCCCCWW", Move::Cast { card, targets: vec![] })));
    cite!(["702.176a", "601.2b", "118.9"]; ok: g.clone().act(0, crate::support::paid("CCWW", Move::CastFor(CastCost::Alternative(IMPENDING_ROW), Box::new(Move::Cast { card, targets: vec![] })))));
    g.players[0].pool.colorless = 2;
    cite!(["601.2h"]; refused("the supplied mana does not pay that cost, all of it and no more"): unchanged(&mut g, |g| g.act(0, crate::support::paid("CCWW", Move::Cast { card, targets: vec![] }))));
    g.act(0, crate::support::paid("CCWW", Move::CastFor(CastCost::Alternative(IMPENDING_ROW), Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    cite!(["118.9", "601.2h"]; assert_eq!(g.players[0].pool.total(), 0, "{{2}}{{W}}{{W}} paid"));
    // it is still a creature spell on the stack
    let on_stack = *g.stack.last().unwrap();
    cite!(["702.176a"]; case: "a creature spell"; assert!(creature(&g, on_stack)));
}

#[test]
fn r702_176a_cast_for_its_impending_cost_it_enters_with_time_counters_and_is_not_a_creature() {
    let mut g = game();
    let o = cast(&mut g, MISTMOORS, true, "CCWW");
    cite!(["702.176a"]; assert_eq!(time(&g, o), 4));
    let seen = g.seen(o, None);
    cite!(["702.176a", "613.1d"]; assert_eq!(seen.types, vec![CardType::Enchantment]));
    // its creature types are removed with the creature type they correlate with
    cite!(["205.1a"]; assert!(seen.subtypes.is_empty(), "{:?}", seen.subtypes));
    // its enters trigger is not the impending ability's: the Insects are created
    cite!(["603.2"]; assert_eq!(insects(&g), 2));
}

#[test]
fn r702_176a_cast_for_its_mana_cost_it_is_a_creature_without_counters() {
    let mut g = game();
    let o = cast(&mut g, MISTMOORS, false, "CCCCCWW");
    cite!(["702.176a"]; assert_eq!(time(&g, o), 0));
    cite!(["702.176a"]; assert!(creature(&g, o)));
    assert_eq!(insects(&g), 2);
    // no impending cost was paid: no end-step trigger
    end_step(&mut g, 0);
    cite!(["702.176a", "603.4"]; assert_eq!(triggers_of(&g, o), 0));
}

#[test]
fn r702_176a_a_time_counter_is_removed_at_its_controllers_end_step_until_it_is_a_creature() {
    let mut g = game();
    let o = cast(&mut g, MISTMOORS, true, "CCWW");
    for left in (0..4).rev() {
        end_step(&mut g, 0);
        cite!(["702.176a", "603.2"]; case: format!("{left} left"); assert_eq!(triggers_of(&g, o), 1));
        settle(&mut g);
        cite!(["702.176a"]; case: format!("{left} left"); assert_eq!(time(&g, o), left));
        cite!(["702.176a", "613.1d"]; case: format!("{left} left"); assert_eq!(creature(&g, o), left == 0));
        // not at the opponent's end step
        end_step(&mut g, 1);
        cite!(["702.176a"]; case: format!("{left} left, opponent"); assert_eq!(triggers_of(&g, o), 0));
    }
    let seen = g.seen(o, None);
    cite!(["702.176a", "205.1a"]; assert!(seen.subtypes.iter().any(|s| s.name() == "Avatar"), "{:?}", seen.subtypes));
    // no time counter: it no longer triggers (603.4)
    end_step(&mut g, 0);
    cite!(["702.176a", "603.4"]; assert_eq!(triggers_of(&g, o), 0));
}

#[test]
fn r702_176a_the_end_step_trigger_is_an_ability_of_the_permanent_and_leaves_with_it() {
    let mut g = game();
    let o = cast(&mut g, MISTMOORS, true, "CCWW");
    end_step(&mut g, 0);
    assert_eq!(triggers_of(&g, o), 1);
    // exiled in response: the trigger still resolves, and does nothing to the new object
    spell(&mut g, 0, EXILE, Some(o));
    settle(&mut g);
    let exiled = *g.exile.iter().find(|x| g.obj(**x).card == Some(MISTMOORS)).expect("exiled");
    cite!(["603.4", "400.7"]; assert_eq!(time(&g, exiled), 0));
    cite!(["113.7a"]; assert!(g.stack.is_empty()));
}

#[test]
fn r702_176a_proliferate_adds_a_time_counter_and_delays_it() {
    let mut g = game();
    let o = cast(&mut g, MISTMOORS, true, "CCWW");
    spell(&mut g, 0, PROLIFERATE, None);
    settle(&mut g);
    cite!(["701.34a", "702.176a"]; assert_eq!(time(&g, o), 5));
    assert!(!creature(&g, o));
}

#[test]
fn r702_176a_a_time_counter_on_one_cast_for_its_mana_cost_does_nothing() {
    let mut g = game();
    let o = cast(&mut g, MISTMOORS, false, "CCCCCWW");
    spell(&mut g, 0, PUT, Some(o));
    settle(&mut g);
    assert_eq!(time(&g, o), 1);
    cite!(["702.176a"]; case: "still a creature"; assert!(creature(&g, o)));
    end_step(&mut g, 0);
    cite!(["702.176a", "603.4"]; case: "no removal"; assert_eq!(triggers_of(&g, o), 0));
}

#[test]
fn r702_176a_it_is_a_creature_to_target_only_once_its_last_time_counter_is_removed() {
    let mut g = game();
    // Probe and the two authored instants create no choice or trigger ordering.
    let settle = |g: &mut Game| {
        while !g.stack.is_empty() {
            assert!(g.decision.is_none(), "unexpected question: {:?}", g.decision);
            g.act(g.priority, Move::Pass).unwrap();
        }
    };
    let card = g.put(0, PROBE, Zone::Hand);
    g.act(0, Move::CastFor(CastCost::Alternative(IMPENDING_ROW), Box::new(Move::Cast { card, targets: vec![] }))).unwrap();
    settle(&mut g);
    let o = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(PROBE)).unwrap();
    assert_eq!(time(&g, o), 1);
    let destroy = g.put(1, DESTROY, Zone::Hand);
    while g.priority != 1 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["702.176a", "115.1"]; case: "with a time counter"; refused("an illegal target, Probe Horror"): unchanged(&mut g, |g| g.act(1, Move::Cast { card: destroy, targets: vec![vec![Target::Object(o)]] })));
    // another effect removes the last counter: a creature again at once
    spell(&mut g, 1, REMOVE, Some(o));
    settle(&mut g);
    assert_eq!(time(&g, o), 0);
    cite!(["702.176a", "613.1d"]; assert!(creature(&g, o)));
    spell(&mut g, 1, DESTROY, Some(o));
    settle(&mut g);
    cite!(["702.176a", "115.1"]; case: "without a time counter"; assert!(!g.battlefield.contains(&o)));
}

#[test]
fn r400_7_a_flickered_impending_permanent_returns_as_a_creature_without_counters() {
    let mut g = game();
    let o = cast(&mut g, MISTMOORS, true, "CCWW");
    spell(&mut g, 0, FLICKER, Some(o));
    settle(&mut g);
    let back = *g.battlefield.iter().find(|x| g.obj(**x).card == Some(MISTMOORS)).expect("returned");
    cite!(["400.7"]; assert_ne!(back, o));
    cite!(["400.7", "702.176a"]; assert_eq!(time(&g, back), 0));
    cite!(["400.7", "702.176a"]; assert!(creature(&g, back)));
    cite!(["603.2"]; assert_eq!(insects(&g), 4, "it entered again"));
}

#[test]
fn r707_10_a_copy_of_a_spell_cast_for_its_impending_cost_enters_with_time_counters() {
    let mut g = game();
    let card = g.put(0, MISTMOORS, Zone::Hand);
    g.players[0].pool.colors[0] = 2;
    g.players[0].pool.colorless = 2;
    g.act(0, crate::support::paid("CCWW", Move::CastFor(CastCost::Alternative(IMPENDING_ROW), Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    let original = *g.stack.last().unwrap();
    let copier = g.put(0, COPY_SPELL, Zone::Hand);
    g.act(0, Move::Cast { card: copier, targets: vec![vec![Target::Object(original)]] }).unwrap();
    settle(&mut g);
    let token = *g.battlefield.iter().find(|x| g.obj(**x).card.is_none() && g.seen(**x, None).types.contains(&CardType::Enchantment)).expect("the copy resolved");
    cite!(["707.10", "702.176a"]; assert_eq!(time(&g, token), 4));
    cite!(["707.10", "702.176a"]; assert!(!creature(&g, token)));
}

#[test]
fn r702_176a_a_token_copy_of_an_impending_permanent_is_a_creature_without_counters() {
    let mut g = game();
    let o = cast(&mut g, MISTMOORS, true, "CCWW");
    spell(&mut g, 0, COPY_TOKEN, Some(o));
    settle(&mut g);
    let token = *g.battlefield.iter().find(|x| g.obj(**x).card.is_none() && g.seen(**x, None).types.contains(&CardType::Enchantment)).expect("the token");
    // the copy wasn't cast, so its impending cost wasn't paid; counters aren't copied (707.2)
    cite!(["707.2", "702.176a"]; assert_eq!(time(&g, token), 0));
    cite!(["707.2", "702.176a"]; assert!(creature(&g, token)));
}

#[test]
fn r702_176a_balemurk_is_refused_by_its_return_not_by_impending() {
    let defs = defs();
    cite!(["702.176a"]; assert_eq!(game::play::check(&defs[BALEMURK]).unwrap_err().0, "selected object: one description"));
}

#[test]
fn r702_176a_cast_for_its_impending_cost_it_does_not_enter_as_a_creature() {
    for impending in [true, false] {
        let mut g = game();
        g.put(0, WATCHER, Zone::Battlefield);
        cast(&mut g, PROBE, impending, if impending { "" } else { "CCCCC" });
        cite!(["603.6a", "702.176a"]; case: format!("impending {impending}"); assert_eq!(g.players[0].life, if impending { 20 } else { 21 }));
    }
}

#[track_caller]
fn unchanged(g: &mut Game, action: impl FnOnce(&mut Game) -> Result<(), String>) -> Result<(), String> {
    let before=format!("{g:?}");
    let result=action(g);
    assert_eq!(format!("{g:?}"),before);
    result
}
