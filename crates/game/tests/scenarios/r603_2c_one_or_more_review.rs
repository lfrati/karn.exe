//! Independent composition checks for simultaneous groups.
use crate::support::{cite, mana_pool};
use game::{CardDef, Game, Move, Target, Zone};
use ir::Part;

fn resolve(g: &mut Game) {
    for _ in 0..40 {
        if g.stack.is_empty() { return; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("stack did not empty");
}

fn main(g: &mut Game) {
    for _ in 0..30 {
        if g.part == Part::PrecombatMain { return; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("never reached main phase");
}

#[test]
fn r603_7b_a_duration_bounded_delayed_group_triggers_once_per_event() {
    // Rules: 603.2c, 603.7b, 704.3
    let watcher = CardDef::new("Time watcher", "{0}", "Artifact",
        "{0}: Whenever one or more creatures you control die this turn, you gain 1 life.", None).unwrap();
    game::play::check(&watcher).unwrap();
    let bear = CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap();
    let burn = CardDef::new("Burn", "{0}", "Sorcery", "Burn deals 2 damage to each creature.", None).unwrap();
    game::play::check(&burn).unwrap();
    let mut g = Game::new(vec![watcher, bear, burn], 2, 20);
    let w = g.put(0, 0, Zone::Battlefield);
    let a=g.put(0, 1, Zone::Battlefield);
    let second=g.put(0, 1, Zone::Battlefield);
    let b = g.put(0, 2, Zone::Hand);
    main(&mut g);
    g.act(0, Move::Activate { source: w, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    resolve(&mut g);
    g.act(0, Move::Cast { card: b, targets: vec![] }).unwrap();
    for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
    cite!(["404.3"]; ok: g.act(0,Move::Choose(game::state::Answer::Objects(vec![a,second]))));
    resolve(&mut g);
    cite!(["603.2c", "704.3"]; assert_eq!(g.players[0].life, 21, "two deaths in one state-based check are one delayed trigger event"));
}

#[test]
fn r509_1h_one_or_more_creatures_become_blocked_once_for_the_declaration() {
    // Rules: 603.2c, 509.1h-i
    let watcher = CardDef::new("Watcher", "{0}", "Enchantment",
        "Whenever one or more creatures you control become blocked, you gain 1 life.", None).unwrap();
    game::play::check(&watcher).unwrap();
    let bear = CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap();
    let mut g = Game::new(vec![watcher, bear], 2, 20);
    g.put(0, 0, Zone::Battlefield);
    let a = g.put(0, 1, Zone::Battlefield);
    let b = g.put(0, 1, Zone::Battlefield);
    let x = g.put(1, 1, Zone::Battlefield);
    let y = g.put(1, 1, Zone::Battlefield);
    for _ in 0..40 {
        if matches!(g.decision, Some(game::state::Decision::Attackers { .. })) { break; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    g.act(0, Move::Attack(vec![(a, Target::Player(1)), (b, Target::Player(1))])).unwrap();
    for _ in 0..40 {
        if matches!(g.decision, Some(game::state::Decision::Blockers { .. })) { break; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    g.act(1, Move::Block(vec![(x, a), (y, b)])).unwrap();
    resolve(&mut g);
    cite!(["603.2c", "509.1h"]; assert_eq!(g.players[0].life, 21, "two became blocked in one declaration"));
}

#[test]
fn r400_7e_a_grouped_death_trigger_follows_each_card_to_its_new_incarnation() {
    // Rules: 603.2c, 400.7e, 603.6c
    let watcher = CardDef::new("Watcher", "{0}", "Enchantment",
        "Whenever one or more creatures you control die, return those cards to your hand.", None).unwrap();
    game::play::check(&watcher).unwrap();
    let bear = CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap();
    let burn = CardDef::new("Burn", "{0}", "Sorcery", "Burn deals 2 damage to each creature.", None).unwrap();
    let mut g = Game::new(vec![watcher, bear, burn], 2, 20);
    g.put(0, 0, Zone::Battlefield);
    let a=g.put(0, 1, Zone::Battlefield);
    let second=g.put(0, 1, Zone::Battlefield);
    let b = g.put(0, 2, Zone::Hand);
    main(&mut g);
    g.act(0, Move::Cast { card: b, targets: vec![] }).unwrap();
    for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
    cite!(["404.3"]; ok: g.act(0,Move::Choose(game::state::Answer::Objects(vec![a,second]))));
    resolve(&mut g);
    cite!(["400.7e"]; assert_eq!(g.players[0].hand.len(), 2, "each departed creature supplies its graveyard card"));
}

#[test]
fn r603_2d_additional_occurrences_each_keep_the_whole_subject_group() {
    // Rules: 603.2c-d, 508.1m, 608.2k
    let watcher = CardDef::new("Watcher", "{0}", "Creature — Human",
        "Whenever one or more creatures you control attack, put a +1/+1 counter on each of those creatures.", Some((1, 3))).unwrap();
    let doubler = CardDef::new("Doubler", "{0}", "Enchantment",
        "If a triggered ability of a creature you control triggers, it triggers an additional time.", None).unwrap();
    let bear = CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap();
    for d in [&watcher, &doubler, &bear] { game::play::check(d).unwrap(); }
    let mut g = Game::new(vec![watcher, doubler, bear], 2, 20);
    let w = g.put(0, 0, Zone::Battlefield);
    g.put(0, 1, Zone::Battlefield);
    let a = g.put(0, 2, Zone::Battlefield);
    let b = g.put(0, 2, Zone::Battlefield);
    for _ in 0..40 {
        if matches!(g.decision, Some(game::state::Decision::Attackers { .. })) { break; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    g.act(0, Move::Attack(vec![(a, Target::Player(1)), (b, Target::Player(1))])).unwrap();
    resolve(&mut g);
    cite!(["603.2d"]; assert_eq!((g.power(a), g.power(b), g.power(w)), (Some(4), Some(4), Some(1))));
}

#[test]
fn r704_3_a_later_state_based_death_check_is_a_second_batch() {
    // Rules: 704.3, 603.2c
    let watcher = CardDef::new("Watcher", "{0}", "Enchantment",
        "Whenever one or more creatures you control die, you gain 1 life.", None).unwrap();
    let lord = CardDef::new("Lord", "{0}", "Creature — Human",
        "Other creatures you control get +1/+1.", Some((1, 1))).unwrap();
    let fragile = CardDef::new("Fragile", "{0}", "Creature — Bear", "", Some((0, 0))).unwrap();
    let removal = CardDef::new("Removal", "{0}", "Sorcery", "Destroy target creature.", None).unwrap();
    for d in [&watcher, &lord, &fragile, &removal] { game::play::check(d).unwrap(); }
    let mut g = Game::new(vec![watcher, lord, fragile, removal], 2, 20);
    g.put(0, 0, Zone::Battlefield);
    let l = g.put(0, 1, Zone::Battlefield);
    let a=g.put(0, 2, Zone::Battlefield);
    let b=g.put(0, 2, Zone::Battlefield);
    let s = g.put(0, 3, Zone::Hand);
    main(&mut g);
    g.act(0, Move::Cast { card: s, targets: vec![vec![Target::Object(l)]] }).unwrap();
    g.act(g.priority,Move::Pass).unwrap();g.act(g.priority,Move::Pass).unwrap();
    cite!(["404.3"]; ok: g.act(0,Move::Choose(game::state::Answer::Objects(vec![a,b]))));
    let Some(game::state::Decision::Order{player:0,abilities})=g.decision.clone() else {panic!("expected the two death-batch triggers: {:?}",g.decision)};
    cite!(["603.3b","704.3"]; assert_eq!(abilities.len(),2));
    // The two checks produced distinct subjects; both gain one life, in either order.
    g.act(0,Move::Order(vec![0,1])).unwrap();
    resolve(&mut g);
    cite!(["704.3", "603.2c"]; assert_eq!(g.players[0].life, 22, "Lord dies, then the two 0/0s die in a later event"));
}

#[test]
fn r508_1f_declaration_taps_the_chosen_attackers_as_one_event() {
    // Rules: 508.1f, 508.2b, 603.2c
    let watcher = CardDef::new("Watcher", "{0}", "Enchantment",
        "Whenever one or more creatures you control become tapped, you gain 1 life.", None).unwrap();
    game::play::check(&watcher).unwrap();
    let bear = CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap();
    let mut g = Game::new(vec![watcher, bear], 2, 20);
    g.put(0, 0, Zone::Battlefield);
    let a = g.put(0, 1, Zone::Battlefield);
    let b = g.put(0, 1, Zone::Battlefield);
    for _ in 0..40 {
        if matches!(g.decision, Some(game::state::Decision::Attackers { .. })) { break; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    g.act(0, Move::Attack(vec![(a, Target::Player(1)), (b, Target::Player(1))])).unwrap();
    resolve(&mut g);
    cite!(["508.1f"]; assert_eq!(g.players[0].life, 21, "chosen attackers are tapped together in 508.1f"));
}

#[test]
fn r733_1_reversing_declaration_taps_restores_the_trigger_turn_use() {
    // Rules: 733.1, 508.1h-j, 508.2b, 603.2c
    let watcher = CardDef::new("Watcher", "{0}", "Enchantment",
        "Whenever one or more creatures you control become tapped, you gain 1 life. This ability triggers only once each turn.", None).unwrap();
    let prison = CardDef::new("Prison", "{0}", "Enchantment",
        "Creatures can't attack you unless their controller pays {2} for each creature they control that's attacking you.", None).unwrap();
    let bear = CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap();
    for d in [&watcher, &prison, &bear] { game::play::check(d).unwrap(); }
    let mut g = Game::new(vec![watcher, prison, bear], 2, 20);
    g.put(0, 0, Zone::Battlefield);
    g.put(1, 1, Zone::Battlefield);
    let a = g.put(0, 2, Zone::Battlefield);
    let b = g.put(0, 2, Zone::Battlefield);
    for _ in 0..40 {
        if matches!(g.decision, Some(game::state::Decision::Attackers { .. })) { break; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    let attack = Move::Attack(vec![(a, Target::Player(1)), (b, Target::Player(1))]);
    g.act(0, attack.clone()).unwrap();
    g.act(0, Move::Choose(game::state::Answer::No)).unwrap();
    assert!(!g.obj(a).tapped && !g.obj(b).tapped);
    cite!(["733.1"]; assert_eq!(g.players[0].life, 20));
    g.players[0].pool.colorless = 4;
    g.act(0, attack).unwrap();
    g.act(0, Move::Choose(game::state::Answer::Pay {plan:vec![],with:Some(mana_pool("CCCC")),tapped:vec![]})).unwrap();
    resolve(&mut g);
    cite!(["733.1"]; assert_eq!(g.players[0].life, 21, "the reversed declaration cannot consume the turn's trigger use"));
}

#[test]
fn r733_1_reversal_keeps_a_real_mana_taps_once_per_turn_trigger() {
    // Rules: 733.1, 508.1i, 603.2c
    reversal_keeps_mana_trigger(false);
}

#[test]
fn r603_2_reversal_uses_the_mana_tap_subjects_event_time_power() {
    // Rules: 603.2, 733.1, 508.1i
    reversal_keeps_mana_trigger(true);
}

fn reversal_keeps_mana_trigger(filtered: bool) {
    let text = if filtered {
        "Whenever one or more creatures you control with power 1 or less become tapped, you gain 1 life. This ability triggers only once each turn."
    } else {
        "Whenever one or more creatures you control become tapped, you gain 1 life. This ability triggers only once each turn."
    };
    let watcher = CardDef::new("Watcher", "{0}", "Enchantment", text, None).unwrap();
    let prison = CardDef::new("Prison", "{0}", "Enchantment",
        "Creatures can't attack you unless their controller pays {2} for each creature they control that's attacking you.", None).unwrap();
    let bear = CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((1, 1))).unwrap();
    let elf = CardDef::new("Mana elf", "{0}", "Creature — Elf", "{T}: Put a +1/+1 counter on this creature. Add one mana of any color.", Some((1, 1))).unwrap();
    let scales = CardDef::new("Scales", "{0}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.", None).unwrap();
    let evolution = CardDef::new("Evolution", "{0}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.", None).unwrap();
    for d in [&watcher, &prison, &bear, &elf, &scales, &evolution] { game::play::check(d).unwrap(); }
    let mut g = Game::new(vec![watcher, prison, bear, elf, scales, evolution], 2, 20);
    g.put(0, 0, Zone::Battlefield);
    g.put(1, 1, Zone::Battlefield);
    let a = g.put(0, 2, Zone::Battlefield);
    let b = g.put(0, 2, Zone::Battlefield);
    let e = g.put(0, 3, Zone::Battlefield);
    let scales = g.put(0, 4, Zone::Battlefield);
    g.put(0, 5, Zone::Battlefield);
    for _ in 0..40 {
        if matches!(g.decision, Some(game::state::Decision::Attackers { .. })) { break; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    g.act(0, Move::Attack(vec![(a, Target::Player(1)), (b, Target::Player(1))])).unwrap();
    let step = game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![], source: e, ability: 0, paying: vec![], adding: vec![vec![ir::mana::ManaSymbol::Color(ir::Color::Green)]] };
    g.act(0, Move::Choose(game::state::Answer::Pay { plan: vec![step], with: Some(mana_pool("CCCG")), tapped: vec![] })).unwrap();
    let replacement = game::operation::ReplacementId::from_ability(scales, game::binding::AbilityKey::printed(0), 0);
    // The same retained mana action with enough mana completes the declaration:
    // its earlier tap event consumes the one turn use, not an additional mana tap.
    let mut paid = g.clone();
    paid.players[0].pool.colorless = 3;
    paid.act(0, Move::Choose(game::state::Answer::Replacement(replacement.clone()))).unwrap();
    resolve(&mut paid);
    assert_eq!(paid.players[0].life, 21);
    assert!(paid.combat.attacking(a) && paid.combat.attacking(b));
    g.act(0, Move::Choose(game::state::Answer::Replacement(replacement))).unwrap();
    assert!(matches!(g.decision, Some(game::state::Decision::Payment { .. })));
    g.act(0, Move::ReversePayment).unwrap();
    assert!(!g.obj(a).tapped && !g.obj(b).tapped);
    assert!(g.obj(e).tapped, "the real mana action is retained");
    assert_eq!(g.players[0].pool.colors[4], 1);
    g.act(0, Move::Attack(vec![])).unwrap();
    resolve(&mut g);
    cite!(["733.1"]; assert_eq!(g.players[0].life, 21, "the real mana tap triggers after the tentative tap event is undone"));
}

#[test]
fn r603_2c_reversal_keeps_the_whole_suppressed_mana_tap_group() {
    // Rules: 603.2c, 733.1, 508.1i
    let watcher = CardDef::new("Watcher", "{0}", "Enchantment",
        "Whenever one or more creatures you control become tapped, untap those creatures. This ability triggers only once each turn.", None).unwrap();
    let prison = CardDef::new("Prison", "{0}", "Enchantment",
        "Creatures can't attack you unless their controller pays {2} for each creature they control that's attacking you.", None).unwrap();
    let bear = CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((1, 1))).unwrap();
    let elf = CardDef::new("Mana elf", "{0}", "Creature — Elf",
        "Tap two untapped creatures you control: Put a +1/+1 counter on this creature. Add one mana of any color.", Some((1, 1))).unwrap();
    let scales = CardDef::new("Scales", "{0}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.", None).unwrap();
    let evolution = CardDef::new("Evolution", "{0}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.", None).unwrap();
    for d in [&watcher, &prison, &bear, &elf, &scales, &evolution] { game::play::check(d).unwrap(); }
    let mut g = Game::new(vec![watcher, prison, bear, elf, scales, evolution], 2, 20);
    g.put(0, 0, Zone::Battlefield);
    g.put(1, 1, Zone::Battlefield);
    let a = g.put(0, 2, Zone::Battlefield);
    let b = g.put(0, 2, Zone::Battlefield);
    let e = g.put(0, 3, Zone::Battlefield);
    let other = g.put(0, 2, Zone::Battlefield);
    let scales = g.put(0, 4, Zone::Battlefield);
    g.put(0, 5, Zone::Battlefield);
    for _ in 0..40 {
        if matches!(g.decision, Some(game::state::Decision::Attackers { .. })) { break; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    g.act(0, Move::Attack(vec![(a, Target::Player(1)), (b, Target::Player(1))])).unwrap();
    let step = game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![], source: e, ability: 0, paying: vec![vec![e, other]], adding: vec![vec![ir::mana::ManaSymbol::Color(ir::Color::Green)]] };
    g.act(0, Move::Choose(game::state::Answer::Pay { plan: vec![step], with: Some(mana_pool("CCCG")), tapped: vec![] })).unwrap();
    let replacement = game::operation::ReplacementId::from_ability(scales, game::binding::AbilityKey::printed(0), 0);
    g.act(0, Move::Choose(game::state::Answer::Replacement(replacement))).unwrap();
    assert!(matches!(g.decision, Some(game::state::Decision::Payment { .. })));
    g.act(0, Move::ReversePayment).unwrap();
    assert!(!g.obj(a).tapped && !g.obj(b).tapped);
    assert!(g.obj(e).tapped && g.obj(other).tapped, "the real mana cost stays paid");
    g.act(0, Move::Attack(vec![])).unwrap();
    resolve(&mut g);
    cite!(["603.2c", "733.1"]; assert_eq!((g.obj(e).tapped, g.obj(other).tapped), (false, false), "both members of the retained event are the trigger's subject"));
    assert_eq!(g.obj(e).counters["+1/+1"], 4);
    assert_eq!(g.players[0].pool.colors[4], 1);
}

#[test]
fn r118_3_a_single_selected_tap_cost_is_one_trigger_event() {
    // Rules: 118.3, 601.2h, 700.1, 603.2c
    let watcher = CardDef::new("Watcher", "{0}", "Enchantment",
        "Whenever one or more creatures you control become tapped, untap those creatures. This ability triggers only once each turn.", None).unwrap();
    let elf = CardDef::new("Mana elf", "{0}", "Creature — Elf",
        "Tap two untapped creatures you control: Add {G}.", Some((1, 1))).unwrap();
    let bear = CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((1, 1))).unwrap();
    for d in [&watcher, &elf, &bear] { game::play::check(d).unwrap(); }
    let mut g = Game::new(vec![watcher, elf, bear], 2, 20);
    g.put(0, 0, Zone::Battlefield);
    let e = g.put(0, 1, Zone::Battlefield);
    let other = g.put(0, 2, Zone::Battlefield);
    main(&mut g);
    g.act(0, Move::Activate { source: e, ability: 0, targets: vec![], paying: vec![vec![e, other]] }).unwrap();
    assert!(g.obj(e).tapped && g.obj(other).tapped);
    resolve(&mut g);
    cite!(["603.2c"]; assert_eq!((g.obj(e).tapped, g.obj(other).tapped), (false, false), "the cost is one event of both chosen creatures"));
    assert_eq!(g.players[0].pool.colors[4], 1);
}
