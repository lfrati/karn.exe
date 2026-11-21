//! Exhaust's per-incarnation limit and the completed activation event are distinct.
use crate::support::cite;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use game::legal::Asked;
use std::sync::OnceLock;

fn definitions() -> Vec<CardDef> {
    static DEFS: OnceLock<Vec<CardDef>> = OnceLock::new();
    DEFS.get_or_init(|| {
        let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
        let catalog = CardDef::catalog(&faces);
        let mut defs: Vec<_> = ["Afterburner Expert", "Draconautics Engineer", "Camera Launcher", "Loot, the Pathfinder", "Rangers' Refueler"]
            .iter().map(|name| CardDef::from_faces(catalog.get(name).unwrap()).unwrap()).collect();
        defs.extend([
            CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
            CardDef::new("Mirror", "{0}", "Instant", "Create a token that's a copy of target creature.", None).unwrap(),
            CardDef::new("Steal", "{0}", "Instant", "You gain control of target creature.", None).unwrap(),
            CardDef::from_faces(catalog.get("Brave Brawler").unwrap()).unwrap(),
            CardDef::from_faces(catalog.get("Abomination, Terrifying Titan").unwrap()).unwrap(),
            CardDef::from_faces(catalog.get("Elvish Refueler").unwrap()).unwrap(),
            CardDef::from_faces(catalog.get("Boom Scholar").unwrap()).unwrap(),
            CardDef::from_faces(catalog.get("Wonder Man, Hollywood Hero").unwrap()).unwrap(),
            CardDef::from_faces(catalog.get("Hulk, Gamma Goliath").unwrap()).unwrap(),
        ]);
        for def in &defs { game::play::check(def).unwrap_or_else(|e| panic!("{}: {e:?}", def.name())); }
        defs
    }).clone()
}
fn game() -> Game {
    let mut g = Game::new(definitions(), 3, 20);
    for p in 0..3 { for _ in 0..12 { g.put(p, 5, Zone::Library); } }
    g
}
fn activate(source: ObjId, ability: usize) -> Move {
    Move::Activate { source, ability, targets: vec![], paying: vec![] }
}
fn pass(g: &mut Game) { for _ in 0..3 { g.act(g.priority, Move::Pass).unwrap(); } }
fn settle(g: &mut Game) {
    for _ in 0..20 {
        if g.stack.is_empty() { return; }
        assert!(matches!(g.asked(), Some((_, Asked::Priority))));
        pass(g);
    }
    panic!("unsettled stack");
}

// Rules: 602.5b, 702.177a, 613.1f
// Given: Engineer's first exhaust ability has been used, but its second has not.
// When: another player gains control and a layer-six effect hides then restores both.
// Expect: neither transition refreshes the first ability or consumes the second.
#[test]
fn r602_5b_control_change_and_ability_loss_preserve_each_exhaust_limit() {
    let mut g = game();
    let source = g.put(0, 1, Zone::Battlefield);
    g.players[0].pool.colors[ir::Color::Red as usize] = 1;
    g.act(0, crate::support::paid("R", activate(source, 0))).unwrap(); settle(&mut g);
    while g.priority != 1 { g.act(g.priority, Move::Pass).unwrap(); }
    let steal = g.put(1, 7, Zone::Hand);
    g.act(1, Move::Cast { card: steal, targets: vec![vec![Target::Object(source)]] }).unwrap();
    settle(&mut g);
    cite!(["602.5b"]; assert_eq!(g.controller(source), 1));
    while g.priority != 1 { g.act(g.priority, Move::Pass).unwrap(); }
    g.players[1].pool.colors[ir::Color::Red as usize] = 2;
    g.players[1].pool.colorless = 3;
    cite!(["602.5b", "702.177a"]; refused("only once"): g.act(1, activate(source, 0)));

    // Synthetic mechanism fixture: removing this continuous effect restores the
    // printed abilities of the same permanent, rather than granting new instances.
    let strip = CardDef::new("Suppressor", "{0}", "Enchantment", "All creatures lose all abilities.", None).unwrap();
    game::play::check(&strip).unwrap();
    let index = g.defs.len(); std::sync::Arc::make_mut(&mut g.defs).push(strip);
    let suppressor = g.put(1, index, Zone::Battlefield);
    cite!(["613.1f"]; assert!(g.abilities(source).is_empty()));
    g.move_object(suppressor, Zone::Graveyard);
    cite!(["613.1f"]; assert_eq!(g.abilities(source).len(), 2));
    cite!(["602.5b", "702.177a"]; refused("only once"): g.act(1, activate(source, 0)));
    cite!(["702.177a"]; ok: g.act(1, crate::support::paid("CCCR", activate(source, 1))));
    settle(&mut g);
    while g.priority != 1 { g.act(g.priority, Move::Pass).unwrap(); }
    g.players[1].pool.colorless = 3;
    cite!(["702.177a"]; refused("only once"): g.act(1, activate(source, 1)));
}

// Rules: 602.5b, 602.5c, 702.177a, 514.2
// Given: a used printed Engineer ability and two successive acquisitions of its text.
// When: a temporary acquisition hides a permanent acquisition, then expires.
// Expect: each new acquisition gets its own limit; restoring the older acquisition
// preserves its used limit. Likeness Looter's pinned repeat-copy ruling independently
// confirms that copying the same card again creates a new limited-ability instance.
#[test]
fn r602_5c_new_copy_acquisitions_are_independent_but_restored_acquisitions_are_not_new() {
    let mut g = game();
    let source = g.put(0, 1, Zone::Battlefield);
    let model = g.put(0, 1, Zone::Battlefield);
    let copy = CardDef::new("Copy again", "{0}", "Instant", "Target creature becomes a copy of target creature.", None).unwrap();
    let temporary = CardDef::new("Temporary copy", "{0}", "Instant", "Target creature becomes a copy of target creature until end of turn.", None).unwrap();
    for def in [&copy, &temporary] { game::play::check(def).unwrap(); }
    let index = g.defs.len(); std::sync::Arc::make_mut(&mut g.defs).extend([copy, temporary]);
    let exhaust = |g: &mut Game| {
        while g.priority != 0 { g.act(g.priority, Move::Pass).unwrap(); }
        g.players[0].pool.colors[ir::Color::Red as usize] = 1;
        g.act(0, crate::support::paid("R", activate(source, 0))).unwrap(); settle(g);
    };
    let acquire = |g: &mut Game, definition| {
        while g.priority != 0 { g.act(g.priority, Move::Pass).unwrap(); }
        let spell = g.put(0, definition, Zone::Hand);
        g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(source)], vec![Target::Object(model)]] }).unwrap();
        settle(g);
    };
    exhaust(&mut g);
    let mut printed = g.clone();
    acquire(&mut printed, index + 1); exhaust(&mut printed);
    let advance = |g: &mut Game| {
        let turn = g.turn;
        for _ in 0..100 {
            if g.turn != turn { break; }
            let (player, asked) = g.asked().unwrap();
            g.act(player, match asked { Asked::Priority => Move::Pass, Asked::Attackers(_) => Move::Attack(vec![]), other => panic!("{other:?}") }).unwrap();
        }
        assert_ne!(g.turn, turn);
        while g.priority != 0 { g.act(g.priority, Move::Pass).unwrap(); }
    };
    advance(&mut printed);
    printed.players[0].pool.colors[ir::Color::Red as usize] = 1;
    cite!(["514.2", "602.5b", "702.177a"]; refused("only once"): printed.act(0, activate(source, 0)));
    acquire(&mut g, index); exhaust(&mut g);
    acquire(&mut g, index + 1); exhaust(&mut g);
    cite!(["602.5c", "702.177a"]; assert_eq!(g.obj(source).counters.get("+1/+1"), Some(&3)));
    advance(&mut g);
    g.players[0].pool.colors[ir::Color::Red as usize] = 1;
    cite!(["514.2", "602.5b", "702.177a"]; refused("only once"): g.act(0, activate(source, 0)));
    acquire(&mut g, index); exhaust(&mut g);
    cite!(["602.5c", "702.177a"]; assert_eq!(g.obj(source).counters.get("+1/+1"), Some(&4)));
}

// Rules: 702.193a, 707.2, 118.7a, 118.7b, 118.7d
// Given: an Engineer entering this turn copies Brawler, whose current mana cost is 1W.
// When: its copied Power-up is announced with three colorless mana available.
// Expect: the current copied 1W, rather than the physical Engineer's 1R, reduces 4W to 3.
// Additional synthetic mechanism fixtures isolate required C and excess C reduction.
#[test]
fn r702_193a_current_copied_mana_cost_and_colorless_reduction_are_distinct() {
    let mut g = game();
    let in_hand = g.put(0, 1, Zone::Hand);
    g.move_object(in_hand, Zone::Battlefield);
    let source = *g.battlefield.last().unwrap();
    let model = g.put(0, 8, Zone::Battlefield);
    let copy = CardDef::new("Current cost", "{0}", "Instant", "Target creature becomes a copy of target creature.", None).unwrap();
    game::play::check(&copy).unwrap();
    let index = g.defs.len(); std::sync::Arc::make_mut(&mut g.defs).push(copy);
    let spell = g.put(0, index, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(source)], vec![Target::Object(model)]] }).unwrap(); settle(&mut g);
    g.players[0].pool.colorless = 3;
    cite!(["702.193a", "707.2"]; ok: g.act(0, crate::support::paid("CCC", activate(source, 1)))); settle(&mut g);
    cite!(["702.193a"]; assert_eq!(g.players[0].pool.colorless, 0));
    cite!(["702.193a"]; assert_eq!(g.obj(source).counters.get("+1/+1"), Some(&2)));

    let required = CardDef::new("Required colorless", "{2}", "Creature", "Power-up — {C}: You gain 1 life.", Some((2,2))).unwrap();
    let excess = CardDef::new("Excess colorless", "{C}", "Creature", "Power-up — {2}: You gain 1 life.", Some((2,2))).unwrap();
    for def in [&required, &excess] { game::play::check(def).unwrap(); }
    let index = g.defs.len(); std::sync::Arc::make_mut(&mut g.defs).extend([required, excess]);
    let in_hand = g.put(0, index, Zone::Hand);
    g.move_object(in_hand, Zone::Battlefield);
    let required = *g.battlefield.last().unwrap();
    g.players[0].pool.colors[ir::Color::Green as usize] = 1;
    cite!(["118.7a", "702.193a"]; refused("mana pool"): g.act(0, activate(required, 0)));
    g.players[0].pool.colorless = 1;
    cite!(["118.7a"]; ok: g.act(0, crate::support::paid("C", activate(required, 0)))); settle(&mut g);
    let in_hand = g.put(0, index + 1, Zone::Hand);
    g.move_object(in_hand, Zone::Battlefield);
    let excess = *g.battlefield.last().unwrap();
    cite!(["118.7b", "118.7d", "702.193a"]; ok: g.act(0, crate::support::paid("G", activate(excess, 0)))); settle(&mut g);
    cite!(["118.7b", "118.7d"]; assert_eq!(g.players[0].pool.colors[ir::Color::Green as usize], 0));
    assert_eq!(g.players[0].life, 22);
}

// Rules: 702.177a, 602.5b, 118.3, 733.1
// Given: Engineer has two exhaust abilities and enough mana for either after a failed attempt.
// When: each ability is activated, including before its first occurrence resolves.
// Expect: failure spends neither mana nor use; each distinct ability is allowed once only.
#[test]
fn r702_177a_independent_abilities_reserve_their_own_limit_and_failed_payment_spends_none() {
    let mut g = game();
    let source = g.put(0, 1, Zone::Battlefield);
    cite!(["118.3", "733.1"]; refused("mana pool"): g.act(0, activate(source, 0)));
    g.players[0].pool.colors[ir::Color::Red as usize] = 3;
    g.players[0].pool.colorless = 6;
    cite!(["702.177a"]; ok: g.act(0, crate::support::paid("R", activate(source, 0))));
    cite!(["702.177a", "602.5b"]; refused("only once"): g.act(0, activate(source, 0)));
    cite!(["702.177a"]; ok: g.act(0, crate::support::paid("CCCR", activate(source, 1))));
    cite!(["702.177a"]; assert_eq!(g.stack.len(), 2));
    settle(&mut g);
    cite!(["702.177a", "602.5b"]; refused("only once"): g.act(0, activate(source, 1)));
    cite!(["702.177a"]; assert_eq!(g.obj(source).counters.get("+1/+1"), Some(&1)));
}

// Rules: 702.177a, 602.5b, 400.7, 707.2
// Given: an Expert has already exhausted; a new token copies it and it later leaves/reenters.
// When: the token and the returned card activate their copied/printed exhaust abilities.
// Expect: new incarnations have fresh limits; existing object use remains spent across turns.
#[test]
fn r702_177a_copy_and_reentry_are_new_uses_but_next_turn_does_not_reset_the_old_one() {
    let mut g = game();
    let source = g.put(0, 0, Zone::Battlefield);
    g.players[0].pool.colorless = 2;
    g.players[0].pool.colors[ir::Color::Green as usize] = 2;
    g.act(0, crate::support::paid("CCGG", activate(source, 0))).unwrap(); settle(&mut g);
    let mirror = g.put(0, 6, Zone::Hand);
    g.act(0, Move::Cast { card: mirror, targets: vec![vec![Target::Object(source)]] }).unwrap(); settle(&mut g);
    let copy = *g.battlefield.iter().find(|id| **id != source && g.obj(**id).chars.name == "Afterburner Expert").unwrap();
    g.players[0].pool.colorless = 2;
    g.players[0].pool.colors[ir::Color::Green as usize] = 2;
    cite!(["707.2", "702.177a"]; ok: g.act(0, crate::support::paid("CCGG", activate(copy, 0)))); settle(&mut g);
    let turn = g.turn;
    for _ in 0..100 {
        if g.turn != turn { break; }
        let (player, asked) = g.asked().unwrap();
        g.act(player, match asked { Asked::Priority => Move::Pass, Asked::Attackers(_) => Move::Attack(vec![]), other => panic!("{other:?}") }).unwrap();
    }
    assert_ne!(g.turn, turn);
    g.players[0].pool.colorless = 2;
    g.players[0].pool.colors[ir::Color::Green as usize] = 2;
    // Seat 0 regains priority after the active player's pass.
    while g.priority != 0 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["602.5b", "702.177a"]; refused("only once"): g.act(0, activate(source, 0)));
    g.move_object(source, Zone::Hand);
    let in_hand = *g.players[0].hand.last().unwrap();
    g.move_object(in_hand, Zone::Battlefield);
    let returned = *g.battlefield.last().unwrap();
    cite!(["400.7", "702.177a"]; ok: g.act(0, crate::support::paid("CCGG", activate(returned, 0))));
}

// Rules: 702.177a, 602.5b, 603.2, 113.6m
// Given: Refueler watches exhaust activations and an Expert is in its owner's graveyard.
// When: the other permanent's exhaust ability is paid and activated.
// Expect: Refueler's draw and Expert's graveyard return trigger above that activation.
#[test]
fn r702_177a_completed_activation_triggers_graveyard_return_and_draw() {
    let mut g = game();
    let source = g.put(0, 2, Zone::Battlefield);
    g.put(0, 4, Zone::Battlefield);
    g.put(0, 0, Zone::Graveyard);
    g.players[0].pool.colorless = 3;
    g.act(0, crate::support::paid("CCC", activate(source, 0))).unwrap();
    let Some((player, Asked::Order(abilities))) = g.asked() else { panic!("two watchers must order their triggers"); };
    cite!(["603.2", "113.6m"]; assert_eq!((player, abilities.len()), (0, 2)));
    g.act(player, Move::Order(vec![0, 1])).unwrap(); settle(&mut g);
    cite!(["603.2", "113.6m"]; assert!(g.players[0].graveyard.is_empty()));
    cite!(["603.2"]; assert_eq!(g.players[0].hand.len(), 1));
    cite!(["702.177a"]; assert_eq!(g.obj(source).counters.get("+1/+1"), Some(&1)));
}

// Rules: 702.177a, 605.3b, 603.2
// Given: Loot's haste permits its mana exhaust activation immediately, with a Refueler watching.
// When: its {G},{T} ability adds three green mana.
// Expect: no mana ability on stack; one completed exhaust event triggers the Refueler's draw.
#[test]
fn r702_177a_mana_exhaust_is_a_completed_activation_without_a_stack_ability() {
    let mut g = game();
    let source = g.put(0, 3, Zone::Battlefield);
    g.put(0, 4, Zone::Battlefield);
    g.players[0].pool.colors[ir::Color::Green as usize] = 1;
    let mv = Move::Adding(vec![vec![ir::mana::ManaSymbol::Color(ir::Color::Green); 3]], Box::new(activate(source, 1)));
    cite!(["605.3b", "702.177a"]; ok: g.act(0, crate::support::paid("G", mv)));
    cite!(["605.3b"]; assert_eq!(g.players[0].pool.colors[ir::Color::Green as usize], 3));
    cite!(["603.2"]; assert_eq!(g.stack.len(), 1));
    pass(&mut g);
    cite!(["603.2"]; assert_eq!(g.players[0].hand.len(), 1));
}


// Rules: 702.193a, 118.7a, 707.2, 400.7
// Given: Brawler's printed {1}{W} reduces its {4}{W} power-up to {3} on entry.
// Expect: a copied new permanent retains that declaration and gets its own reduction/use.
#[test]
fn r702_193a_entry_reduction_and_copy_preserve_the_keyword_declaration() {
    let mut g = game();
    let source = g.put(0, 8, Zone::Hand);
    g.move_object(source, Zone::Battlefield);
    let source = *g.battlefield.last().unwrap();
    assert_eq!(g.ability_text(Some(source), game::binding::AbilityKey::printed(1)).as_deref(), Some("Power-up — {4}{W}: Put two +1/+1 counters on Brave Brawler."));
    g.players[0].pool.colorless = 3;
    cite!(["702.193a", "118.7a"]; ok: g.act(0, crate::support::paid("CCC", activate(source, 1))));
    cite!(["118.7a"]; assert_eq!(g.players[0].pool.total(), 0));
    settle(&mut g);
    cite!(["702.193a"]; assert_eq!(g.obj(source).counters["+1/+1"], 2));
    let mirror = g.put(0, 6, Zone::Hand);
    g.act(0, Move::Cast { card: mirror, targets: vec![vec![Target::Object(source)]] }).unwrap();
    settle(&mut g);
    let copy = *g.battlefield.last().unwrap();
    g.players[0].pool.colorless = 3;
    cite!(["707.2", "702.193a"]; ok: g.act(0, crate::support::paid("CCC", activate(copy, 1))));
    cite!(["118.7a"]; assert_eq!(g.players[0].pool.total(), 0));
    settle(&mut g);
    cite!(["707.2"]; assert_eq!(g.obj(copy).counters["+1/+1"], 2));
    g.players[0].pool.colorless = 4;
    g.players[0].pool.colors[ir::Color::White as usize] = 1;
    cite!(["602.5b", "702.193a"]; refused("only once"): g.act(0, activate(copy, 1)));
}

// Rules: 702.193b, 118.7e, 118.7b
// Given: Abomination entered this turn; choose the red half of its reduction.
// Expect: {5}{R/G}{R/G} less {3}{R} costs {2} and one hybrid symbol.
#[test]
fn r702_193b_hybrid_reduction_half_is_explicit_and_remaining_hybrid_can_be_green() {
    let mut g = game();
    let source = g.put(0, 9, Zone::Hand);
    g.move_object(source, Zone::Battlefield);
    let source = *g.battlefield.last().unwrap();
    g.players[0].pool.colorless = 2;
    g.players[0].pool.colors[ir::Color::Green as usize] = 1;
    let audit=g.audit_game_objects();
    let rows=&audit["objects"][source.0.to_string()]["cost_branch_choices"];
    cite!(["118.7e","702.193b"]; assert_eq!(rows[0][0]["branch"],serde_json::json!({"kind":"hybrid_reduction","symbol":1,"half":"R"})));
    assert_eq!(rows[0][1]["branch"],serde_json::json!({"kind":"hybrid_reduction","symbol":1,"half":"G"}));
    assert_eq!(rows[0][0]["ability"]["paragraph"],2);
    let selected:game::BranchSelection=serde_json::from_value(rows[0][0].clone()).unwrap();
    let mv = Move::WithCostBranches(vec![selected], Box::new(Move::Activate { source, ability: 1, targets: vec![vec![]], paying: vec![] }));
    cite!(["118.7e", "702.193b"]; ok: g.act(0, crate::support::paid("CCG", mv)));
    cite!(["118.7b"]; assert_eq!(g.players[0].pool.total(), 0));
    settle(&mut g);
    cite!(["702.193a"]; assert_eq!(g.obj(source).counters["+1/+1"], 1));
}

// Rules: 707.2, 702.177a, 603.2
// Given: an Expert is copied and Refueler watches the new permanent's activation.
// Expect: the copy's exhaust family survives recompilation and triggers the draw.
#[test]
fn r702_177a_copy_retains_completed_exhaust_family() {
    let mut g = game();
    let source = g.put(0, 0, Zone::Battlefield);
    g.put(0, 4, Zone::Battlefield);
    let mirror = g.put(0, 6, Zone::Hand);
    g.act(0, Move::Cast { card: mirror, targets: vec![vec![Target::Object(source)]] }).unwrap();
    settle(&mut g);
    let copy = *g.battlefield.last().unwrap();
    g.players[0].pool.colorless = 2;
    g.players[0].pool.colors[ir::Color::Green as usize] = 2;
    g.act(0, crate::support::paid("CCGG", activate(copy, 0))).unwrap();
    cite!(["707.2", "603.2"]; assert_eq!(g.stack.len(), 2));
    settle(&mut g);
    cite!(["603.2"]; assert_eq!(g.players[0].hand.len(), 1));
}

fn suspended(exhaust_mana: bool) -> (Game, ObjId, ObjId) {
    let prefix = if exhaust_mana { "Exhaust — " } else { "" };
    let defs = vec![
        CardDef::new("Mana", "{0}", "Creature — Elf", &format!("{prefix}{{T}}: Put a +1/+1 counter on this creature. Add X mana of any one color, where X is the number of +1/+1 counters on this creature. If you haven't activated an exhaust ability this turn, you gain 3 life."), Some((1, 1))).unwrap(),
        CardDef::new("Scales", "{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.", None).unwrap(),
        CardDef::new("Evolution", "{2}{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.", None).unwrap(),
        CardDef::new("Consumer", "{0}", "Artifact", "Exhaust — {4}: You gain 3 life.", None).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    let mana = g.put(0, 0, Zone::Battlefield);
    g.turn = 2;
    g.put(0, 1, Zone::Battlefield);
    let evolution = g.put(0, 2, Zone::Battlefield);
    let outer = g.put(0, 3, Zone::Battlefield);
    let step = game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, source: mana, ability: 0, chose: vec![], paying: vec![], adding: vec![] };
    g.act(0, Move::WithMana(vec![step], Box::new(activate(outer, 0)))).unwrap();
    let Some(game::state::Decision::Choose { choice: game::state::Choice::Replacement { among, .. }, .. }) = &g.decision else { panic!("replacement question"); };
    let replacement = among.iter().find(|r| r.source == evolution).unwrap().clone();
    g.act(0, Move::Choose(game::state::Answer::Replacement(replacement))).unwrap();
    g.act(0, Move::Choose(game::state::Answer::Mana(vec![ir::mana::ManaSymbol::Color(ir::Color::Green); 3]))).unwrap();
    assert!(matches!(g.decision, Some(game::state::Decision::Payment { .. })));
    (g, mana, outer)
}

// Rules: 702.177b, 733.1
// Given: an exhaust activation is open while an ordinary mana ability resolves in payment.
// Expect: its conditional text sees the begun exhaust; reversal releases the outer use.
#[test]
fn r702_177b_begun_history_is_visible_before_payment_and_reversal_releases_the_use() {
    let (mut g, _, outer) = suspended(false);
    cite!(["702.177b"]; assert_eq!(g.players[0].life, 20));
    assert!(!g.log.iter().any(|e| matches!(e, game::Event::Activated { source, .. } if *source == outer)));
    g.act(0, Move::ReversePayment).unwrap();
    g.players[0].pool.colorless = 4;
    cite!(["733.1"]; ok: g.act(0, crate::support::paid("CCCC", activate(outer, 0))));
}

// Rules: 733.1, 702.177a
// Given: a legal inner mana exhaust completed before an unaffordable outer exhaust is reversed.
// Expect: only the outer use is released; the completed inner use remains consumed.
#[test]
fn r733_1_reversal_keeps_the_completed_inner_mana_exhaust_use() {
    let (mut g, mana, outer) = suspended(true);
    g.act(0, Move::ReversePayment).unwrap();
    g.untap(mana);
    cite!(["733.1", "702.177a"]; refused("only once"): g.act(0, activate(mana, 0)));
    g.players[0].pool.colorless = 4;
    cite!(["733.1"]; ok: g.act(0, crate::support::paid("CCCC", activate(outer, 0))));
}


fn advance_to_main(g: &mut Game, active: usize, later_than: u32) {
    for _ in 0..400 {
        if g.turn > later_than && g.active == active && g.priority == active && g.part == ir::Part::PrecombatMain { return; }
        let (player, asked) = g.asked().unwrap();
        let mv = match asked {
            Asked::Priority => Move::Pass,
            Asked::Attackers(_) => Move::Attack(vec![]),
            other => panic!("unexpected question: {other:?}"),
        };
        g.act(player, mv).unwrap();
    }
    panic!("main phase not reached");
}

// Rules: 702.177b, 602.5b, 733.1
// Given: Engineer and Loot's mana Exhaust were used on a previous turn; Refueler is present.
// When: on your next turn, Engineer begins under Refueler and tries to use the used Loot
// mana Exhaust during payment. Then retry with mana already in the pool.
// Expect: beginning Engineer disables Refueler's permission for Loot; reversing the illegal
// attempt restores the permission, so Engineer can be retried once, never twice that turn.
#[test]
fn r702_177b_refueler_permission_ends_when_outer_exhaust_begins_and_failed_attempt_restores_it() {
    let mut g = game();
    let engineer = g.put(0, 1, Zone::Battlefield);
    let loot = g.put(0, 3, Zone::Battlefield);
    g.put(0, 10, Zone::Battlefield);
    g.players[0].pool.colors[ir::Color::Red as usize] = 1;
    g.players[0].pool.colors[ir::Color::Green as usize] = 1;
    g.act(0, crate::support::paid("R", activate(engineer, 0))).unwrap();
    let mut unused_mana = g.clone();
    let red = ir::mana::ManaSymbol::Color(ir::Color::Red);
    g.act(0, crate::support::paid("G", Move::Adding(vec![vec![red; 3]], Box::new(activate(loot, 1))))).unwrap();
    settle(&mut g);
    let previous = g.turn;
    settle(&mut unused_mana);
    advance_to_main(&mut unused_mana, 0, previous);
    unused_mana.players[0].pool.colors[ir::Color::Green as usize] = 1;
    let fresh = game::legal::Step { plan: vec![], x: None, life: vec![], with: Some(crate::support::mana_pool("G")), cost_branches: None, source: loot, ability: 1, chose: vec![], paying: vec![], adding: vec![vec![red; 3]] };
    cite!(["702.177a", "702.177b"]; ok: unused_mana.act(0, crate::support::paid("R", Move::WithMana(vec![fresh], Box::new(activate(engineer, 0))))));
    cite!(["605.3b"]; assert!(unused_mana.obj(loot).tapped));
    cite!(["702.177b"]; assert_eq!(unused_mana.players[0].pool.colors[ir::Color::Red as usize], 2));
    advance_to_main(&mut g, 1, previous);
    while g.priority != 0 { g.act(g.priority, Move::Pass).unwrap(); }
    g.players[0].pool.colors[ir::Color::Red as usize] = 1;
    cite!(["702.177b", "602.5b"]; refused("only once"): g.act(0, activate(engineer, 0)));
    advance_to_main(&mut g, 0, previous);
    g.players[0].pool.colors[ir::Color::Green as usize] = 1;
    let step = game::legal::Step { plan: vec![], x: None, life: vec![], with: Some(crate::support::mana_pool("G")), cost_branches: None, source: loot, ability: 1, chose: vec![], paying: vec![], adding: vec![vec![red; 3]] };
    cite!(["702.177b"]; refused("only once"): g.act(0, Move::WithMana(vec![step], Box::new(activate(engineer, 0)))));
    cite!(["733.1"]; assert!(!g.obj(loot).tapped));
    g.players[0].pool.colors[ir::Color::Red as usize] = 1;
    cite!(["733.1", "702.177b"]; ok: g.act(0, crate::support::paid("R", activate(engineer, 0))));
    g.players[0].pool.colors[ir::Color::Red as usize] = 1;
    cite!(["702.177b"]; refused("only once"): g.act(0, activate(engineer, 0)));
}


// Rules: 118.7a, 602.2b
// Given: Scholar reduces OTHER permanents' Exhaust generic costs by two per instance.
// Expect: it doesn't reduce its own cost, a second Scholar does; stacked reductions can
// make Camera's cost zero but cannot replace Expert's required green mana.
#[test]
fn r118_7a_exhaust_source_modifiers_exclude_self_stack_and_preserve_typed_mana() {
    let mut g = game();
    let scholar = g.put(0, 11, Zone::Battlefield);
    g.players[0].pool.colorless = 2;
    g.players[0].pool.colors[ir::Color::Red as usize] = 1;
    g.players[0].pool.colors[ir::Color::Green as usize] = 1;
    cite!(["118.7a"]; refused("mana pool"): g.act(0, activate(scholar, 1)));
    g.put(0, 11, Zone::Battlefield);
    cite!(["118.7a", "602.2b"]; ok: g.act(0, crate::support::paid("CCRG", activate(scholar, 1))));
    cite!(["118.7a"]; assert_eq!(g.players[0].pool.total(), 0));
    settle(&mut g);
    let camera = g.put(0, 2, Zone::Battlefield);
    cite!(["118.7a"]; ok: g.act(0, activate(camera, 0)));
    settle(&mut g);
    cite!(["118.7a"]; assert_eq!(g.obj(camera).counters["+1/+1"], 1));
    let expert = g.put(0, 0, Zone::Battlefield);
    g.players[0].pool.colors[ir::Color::Green as usize] = 1;
    cite!(["118.7a"]; refused("mana pool"): g.act(0, activate(expert, 0)));
    g.players[0].pool.colors[ir::Color::Green as usize] = 2;
    cite!(["118.7a"]; ok: g.act(0, crate::support::paid("GG", activate(expert, 0))));
    cite!(["118.7a"]; assert_eq!(g.players[0].pool.total(), 0));
}

// Rules: 602.5b, 702.193a, 611.3b
// Given: Wonder Man permits each controlled Power-up an additional activation.
// Expect: Brawler may activate twice; losing and regaining that static permission does
// not erase earlier uses, and source counters are the results of both resolutions.
#[test]
fn r702_193a_additional_lifetime_permission_is_live_and_does_not_reset_uses() {
    let mut g = game();
    let wonder = g.put(0, 12, Zone::Battlefield);
    let brawler = g.put(0, 8, Zone::Battlefield);
    g.players[0].pool.colorless = 8;
    g.players[0].pool.colors[ir::Color::White as usize] = 2;
    g.act(0, crate::support::paid("CCCCW", activate(brawler, 1))).unwrap();
    let mut lost = g.clone();
    lost.move_object(wonder, Zone::Graveyard);
    cite!(["611.3b", "602.5b"]; refused("only once"): lost.act(0, activate(brawler, 1)));
    cite!(["702.193a"]; ok: g.act(0, crate::support::paid("CCCCW", activate(brawler, 1))));
    g.players[0].pool.colorless = 4;
    g.players[0].pool.colors[ir::Color::White as usize] = 1;
    cite!(["602.5b", "702.193a"]; refused("already been activated 2 times"): g.act(0, activate(brawler, 1)));
    settle(&mut g);
    cite!(["608.2c"]; assert_eq!(g.obj(brawler).counters["+1/+1"], 4));
    g.move_object(wonder, Zone::Hand);
    let in_hand = *g.players[0].hand.last().unwrap();
    g.move_object(in_hand, Zone::Battlefield);
    cite!(["602.5b", "611.3b"]; refused("already been activated 2 times"): g.act(0, activate(brawler, 1)));
}


// Rules: 601.2f, 602.2b
// Given: a {1} activation reduces itself by {3}; another permanent increases it by {2}.
// Expect: all increases precede all reductions, so (1+2)-3 is zero, never two.
#[test]
fn r601_2f_registered_increases_precede_announcement_reductions() {
    let defs = vec![
        CardDef::new("Source", "{0}", "Creature — Elf", "{1}: You gain 1 life. This ability costs {3} less to activate.", Some((1, 1))).unwrap(),
        CardDef::new("Tax", "{0}", "Enchantment", "Activated abilities of creatures you control cost {2} more to activate.", None).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    g.put(0, 1, Zone::Battlefield);
    cite!(["601.2f", "602.2b"]; ok: g.act(0, activate(source, 0)));
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["119.3"]; assert_eq!(g.players[0].life, 21));
}

// Rules: 118.7a, 702.193b, 601.2f
// Given: Hulk reduces other creatures' power-up costs, but not his own; entry discounts
// also use each source's current mana cost. Brave Brawler is a creature; Camera Launcher
// has an Exhaust ability instead, so Hulk does not reduce that activation.
#[test]
fn r118_7a_power_up_source_modifiers_filter_family_and_exclude_the_grant() {
    let mut g = game();
    let hulk = g.put(0, 13, Zone::Hand);
    g.move_object(hulk, Zone::Battlefield);
    let hulk = *g.battlefield.last().unwrap();
    let index = (0..g.abilities(hulk).len()).find(|i| g.ability_text(Some(hulk), game::binding::AbilityKey::printed(*i)).is_some_and(|text| text.starts_with("Power-up —"))).unwrap();
    g.players[0].pool.colorless = 2;
    cite!(["118.7a", "702.193b"]; refused("mana pool"): g.act(0, activate(hulk, index)));
    g.players[0].pool.colorless = 3;
    cite!(["118.7a", "702.193b"]; ok: g.act(0, crate::support::paid("CCC", activate(hulk, index))));
    settle(&mut g);
    cite!(["608.2c"]; assert_eq!(g.obj(hulk).counters["+1/+1"], 5));
    let entered = g.put(0, 8, Zone::Hand);
    g.move_object(entered, Zone::Battlefield);
    let entered = *g.battlefield.last().unwrap();
    cite!(["601.2f", "702.193b"]; ok: g.act(0, activate(entered, 1)));
    settle(&mut g);
    let old = g.put(0, 8, Zone::Battlefield);
    g.players[0].pool.colorless = 1;
    cite!(["118.7a"]; refused("mana pool"): g.act(0, activate(old, 1)));
    g.players[0].pool.colors[ir::Color::White as usize] = 1;
    cite!(["118.7a"]; ok: g.act(0, crate::support::paid("CW", activate(old, 1))));
    settle(&mut g);
    let exhaust = g.put(0, 2, Zone::Battlefield);
    cite!(["118.7a"]; refused("mana pool"): g.act(0, activate(exhaust, 0)));
}

// Rules: 109.5, 702.177a/b, 602.5b
// Given: each player controls a Refueler; p0's Engineer was used on a previous turn.
// When: p1 activates its unused Engineer during p0's new turn, then p0 reuses its own.
// Expect: p1's history does not consume p0's exception. P0's subsequent activation does;
// neither p0's own-turn permission nor its controller's history grants p1 a repeat.
#[test]
fn r702_177b_refueler_history_and_own_turn_permission_belong_to_each_player() {
    let mut g = game();
    let own = g.put(0, 1, Zone::Battlefield);
    let opponent = g.put(1, 1, Zone::Battlefield);
    g.put(0, 10, Zone::Battlefield);
    g.put(1, 10, Zone::Battlefield);
    g.players[0].pool.colors[ir::Color::Red as usize] = 1;
    g.act(0, crate::support::paid("R", activate(own, 0))).unwrap(); settle(&mut g);
    let previous = g.turn;
    advance_to_main(&mut g, 0, previous);
    while g.priority != 1 { g.act(g.priority, Move::Pass).unwrap(); }
    g.players[1].pool.colors[ir::Color::Red as usize] = 1;
    cite!(["702.177a"]; ok: g.act(1, crate::support::paid("R", activate(opponent, 0))));
    settle(&mut g);
    cite!(["109.5", "702.177b"]; assert_eq!(g.active, 0));
    assert_eq!(g.obj(opponent).counters.get("+1/+1"), Some(&1));
    while g.priority != 0 { g.act(g.priority, Move::Pass).unwrap(); }
    g.players[0].pool.colors[ir::Color::Red as usize] = 1;
    cite!(["109.5", "702.177b", "602.5b"]; ok: g.act(0, crate::support::paid("R", activate(own, 0))));
    settle(&mut g);
    cite!(["702.177b"]; assert_eq!(g.obj(own).counters.get("+1/+1"), Some(&2)));
    g.players[0].pool.colors[ir::Color::Red as usize] = 1;
    cite!(["702.177a", "702.177b"]; refused("only once"): g.act(0, activate(own, 0)));
    while g.priority != 1 { g.act(g.priority, Move::Pass).unwrap(); }
    g.players[1].pool.colors[ir::Color::Red as usize] = 1;
    cite!(["109.5", "702.177b"]; refused("only once"): g.act(1, activate(opponent, 0)));
    assert_eq!(g.players[0].pool.colors[ir::Color::Red as usize], 1);
    assert_eq!(g.players[1].pool.colors[ir::Color::Red as usize], 1);
}
