//! Casting owns mana children and resumes once (601.2g-h, 605.3b, 616.1).
use crate::support::{cite, mana_pool, paid};
use game::{CardDef, Event, Game, Move, ObjId, Zone};
use game::legal::Step;
use game::state::{Answer, Choice, Decision};
use game::simulation::{Simulation, ResponseError};
use ir::mana::ManaSymbol;

fn setup(cost: &str) -> (Game, ObjId, ObjId, ObjId, ObjId) {
    let defs = vec![
        CardDef::new("Mana creature", "{0}", "Creature — Elf", "{T}: You gain 2 life. Put a +1/+1 counter on this creature. Add X mana of any one color, where X is the number of +1/+1 counters on this creature.", Some((1, 1))).unwrap(),
        CardDef::new("Hardened Scales", "{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.", None).unwrap(),
        CardDef::new("Branching Evolution", "{2}{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.", None).unwrap(),
        CardDef::new("Spell", cost, "Sorcery", "You gain 3 life.\nFlashback {4}", None).unwrap(),
        CardDef::new("Land", "", "Land", "{T}: Add {G}.", None).unwrap(),
        CardDef::new("Consumer", "{0}", "Artifact", &format!("{cost}, {{T}}: You gain 3 life. Activate only once each turn."), None).unwrap(),
        CardDef::new("Watcher", "{0}", "Enchantment", "Whenever you gain life, this enchantment deals 1 damage to each opponent.\nWhenever you cast a spell, you gain 5 life.", None).unwrap(),
        CardDef::new("Filter", "{0}", "Artifact", "{1}, {T}: Add {W}.", None).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let scales = g.put(0, 1, Zone::Battlefield);
    let evolution = g.put(0, 2, Zone::Battlefield);
    let card = g.put(0, 3, Zone::Hand);
    (g, source, card, scales, evolution)
}
fn step(source: ObjId) -> Step {  Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source, ability: 0, paying: vec![], adding: vec![vec![ManaSymbol::Color(ir::Color::Green); 3]] } }
fn begin(g: &mut Game, source: ObjId, card: ObjId, spending: &str) {
    g.act(0, paid(spending, Move::WithMana(vec![step(source)], Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
}
fn choose(g: &mut Game, source: ObjId) {
    let Some(Decision::Choose { choice: Choice::Replacement { among, .. }, .. }) = &g.decision else { panic!("replacement question: {:?}", g.decision) };
    let effect = among.iter().find(|e| e.source == source).unwrap().clone();
    g.act(0, Move::Choose(Answer::Replacement(effect))).unwrap();
}
#[test]
fn r601_2g_casting_owns_a_mana_child_through_replacement_and_mana_questions() {
    let (mut g, source, card, scales, evolution) = setup("{3}");
    let before_payment = g.layer_revision();
    begin(&mut g, source, card, "GGG");
    assert!(g.layer_revision() > before_payment);
    assert_eq!(g.stack.len(), 1);
    assert_eq!(g.players[0].life, 22);
    assert!(g.obj(source).tapped);
    cite!(["601.2i"]; assert!(!g.log.iter().any(|e| matches!(e, Event::Cast { .. }))));
    for player in 0..2 { crate::support::refused(g.act(player, Move::Pass), "a decision is pending: a choice of the effect being applied (608.2d)"); }
    let paused_revision = g.layer_revision();
    let mut other = g.clone();
    choose(&mut other, evolution);
    assert!(other.layer_revision() > paused_revision);
    assert_eq!(g.layer_revision(), paused_revision);
    assert!(other.decision.is_none());
    cite!(["616.1"]; assert_eq!(other.players[0].pool.total(), 0));
    choose(&mut g, scales);
    assert!(g.layer_revision() > paused_revision);
    assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::Mana { .. }, .. })));
    crate::support::refused(g.act(1, Move::Choose(Answer::Mana(vec![ManaSymbol::Color(ir::Color::Green); 4]))), "that decision is another player's");
    g.act(0, Move::Choose(Answer::Mana(vec![ManaSymbol::Color(ir::Color::Green); 4]))).unwrap();
    cite!(["616.1"]; assert_eq!(g.players[0].pool.total(), 1));
    for (case, game) in [("scales first", &g), ("evolution first", &other)] {
        assert!(game.decision.is_none() && game.resolving().is_none());
        assert_eq!(game.stack.len(), 1);
        assert_eq!(game.players[0].life, 22);
        cite!(["601.2i"]; case: case; assert_eq!(game.log.iter().filter(|e| matches!(e, Event::Cast { .. })).count(), 1));
        assert_eq!(game.log.iter().filter(|e| matches!(e, Event::Activated { .. })).count(), 1);
        assert_eq!(game.log.iter().filter(|e| matches!(e, Event::LifeGained { .. })).count(), 1);
        assert_eq!(game.log.iter().filter(|e| matches!(e, Event::TappedForMana { .. })).count(), 1);
    }
}
#[test]
fn r601_2h_payment_can_be_corrected_after_an_order_produces_less_mana() {
    let (mut g, source, card, _, evolution) = setup("{4}");
    let land = g.put(0, 4, Zone::Battlefield);
    begin(&mut g, source, card, "GGGG");
    choose(&mut g, evolution);
    assert!(matches!(g.decision, Some(Decision::Payment { player: 0, .. })));
    assert_eq!(g.players[0].pool.total(), 3);
    let spell=g.stack[0];
    let audit=g.audit_game_objects();
    cite!(["601.2h","601.2i"]; assert_eq!(audit["objects"][spell.0.to_string()]["announced"]["cast"]["completed"],false));
    assert!(audit["objects"][spell.0.to_string()]["announced"]["cast"]["payment"].is_null());
    let before = format!("{g:?}");
    cite!(["601.2h"]; refused("your mana pool cannot pay that cost (118.3a): {4}"): g.act(0, Move::FinishPayment { plan: vec![], paying: vec![], with: None }));
    assert_eq!(format!("{g:?}"), before);
    crate::support::refused(g.act(0, Move::Pass), "a decision is pending: complete the announced payment (601.2g-h)");
    cite!(["605.3a"]; ok: g.act(0, Move::FinishPayment { plan: vec![Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source: land, ability: 0, paying: vec![], adding: vec![] }], paying: vec![], with: Some(mana_pool("GGGG")) }));
    assert!(g.decision.is_none());
    cite!(["601.2h"]; assert_eq!(g.players[0].pool.total(), 0));
    assert_eq!(g.players[0].life, 22);
    cite!(["601.2i"]; assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Cast { .. })).count(), 1));
}
#[test]
fn r601_2g_simulation_correlates_nested_payment_questions() {
    let (g, source, card, _, _) = setup("{3}");
    let mut sim = Simulation::new(g);
    let first = sim.request().unwrap().unwrap().clone();
    sim.respond(first.id, first.player, paid("GGG", Move::WithMana(vec![step(source)], Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    let question = sim.request().unwrap().unwrap().clone();
    assert_ne!(question.id, first.id);
    let Some(Decision::Choose { choice: Choice::Replacement { among, .. }, .. }) = &sim.game().unwrap().decision else { panic!("replacement question") };
    let answer = Move::Choose(Answer::Replacement(among[0].clone()));
    assert!(matches!(sim.respond(first.id, 0, answer.clone()), Err(ResponseError::Stale { .. })));
    crate::support::refused(sim.respond(question.id, 0, Move::Pass), "a decision is pending: a choice of the effect being applied (608.2d)");
    sim.respond(question.id, 0, answer).unwrap();
    assert!(sim.fault().is_none());
}

#[test]
fn r602_2b_activation_uses_the_same_suspended_payment_parent() {
    let (mut g, source, _, _, evolution) = setup("{3}");
    let consumer = g.put(0, 5, Zone::Battlefield);
    g.act(0, paid("GGG", Move::WithMana(vec![step(source)], Box::new(Move::Activate {
        source: consumer, ability: 0, targets: vec![], paying: vec![] })))).unwrap();
    assert_eq!(g.stack.len(), 1);
    cite!(["602.2b", "601.2g"]; assert!(!g.obj(consumer).tapped, "the outer activation has not paid yet"));
    crate::support::refused(g.act(1, Move::Pass), "a decision is pending: a choice of the effect being applied (608.2d)");
    choose(&mut g, evolution);
    assert!(g.decision.is_none());
    cite!(["602.2b", "601.2h"]; assert!(g.obj(consumer).tapped));
    assert_eq!(g.players[0].pool.total(), 0);
    assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Activated { .. })).count(), 2);
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    assert_eq!(g.players[0].life, 25);
}

#[test]
fn r605_3b_multiple_payment_children_each_resume_once() {
    let (mut g, first, card, _, evolution) = setup("{6}");
    let second = g.put(0, 0, Zone::Battlefield);
    g.act(0, paid("GGGGGG", Move::WithMana(vec![step(first), step(second)], Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    choose(&mut g, evolution);
    cite!(["605.3b"]; assert_eq!(g.players[0].life, 24));
    assert_eq!(g.players[0].pool.total(), 3);
    assert!(!g.log.iter().any(|e| matches!(e, Event::Cast { .. })));
    choose(&mut g, evolution);
    assert!(g.decision.is_none());
    assert_eq!(g.players[0].pool.total(), 0);
    assert_eq!(g.players[0].life, 24);
    assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Activated { .. })).count(), 2);
    cite!(["601.2i"]; assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Cast { .. })).count(), 1));
}

#[test]
fn r601_2g_an_invalid_later_mana_step_asks_for_correction_without_losing_its_prefix() {
    let (mut g, source, card, _, evolution) = setup("{3}");
    g.act(0, paid("GGG", Move::WithMana(vec![step(source), step(source)], Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    choose(&mut g, evolution);
    assert!(matches!(g.decision, Some(Decision::Payment { .. })));
    assert_eq!(g.players[0].pool.total(), 3);
    g.act(0, Move::FinishPayment { plan: vec![], paying: vec![], with: Some(mana_pool("GGG")) }).unwrap();
    cite!(["601.2h"]; assert_eq!(g.players[0].pool.total(), 0));
    assert_eq!(g.players[0].life, 22);
    cite!(["118.3"]; assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Activated { .. })).count(), 1));
}

#[test]
fn r733_1_reverse_failed_cast_keeps_legal_mana_and_original_card_identity() {
    for graveyard in [false, true] {
        let (mut g, source, hand, _, evolution) = setup("{4}");
        let card = if graveyard { g.put(0, 3, Zone::Graveyard) } else { hand };
        let cast = paid("GGGG", Move::WithMana(vec![step(source)], Box::new(Move::Cast {card,targets:vec![]})));
        g.act(0, if graveyard {Move::CastVia(game::announcement::CastRoute::Flashback, Box::new(cast))} else {cast}).unwrap();
        let abandoned = g.stack[0];
        choose(&mut g, evolution);
        let end = g.log.end();
        let mut clone = g.clone();
        crate::support::refused(g.act(1, Move::ReversePayment), "that decision is another player's");
        let before_reverse = g.layer_revision();
        g.act(0, Move::ReversePayment).unwrap();
        assert!(g.layer_revision() > before_reverse);
        let case = if graveyard { "graveyard" } else { "hand" };
        cite!(["733.1"]; case: case; assert!(g.stack.is_empty() && g.decision.is_none()));
        cite!(["733.2"]; case: case; assert_eq!(g.priority, 0));
        assert_eq!(g.players[0].pool.total(), 3);
        assert_eq!(g.players[0].life, 22);
        assert!(g.obj(source).tapped);
        cite!(["733.1"]; case: case; assert_eq!(g.obj(card).zone, if graveyard { Zone::Graveyard } else { Zone::Hand }));
        assert_eq!(g.obj(abandoned).zone, Zone::Gone);
        assert!(g.log.end() > end);
        assert!(!g.log.since(end).iter().any(|e| matches!(e, Event::ZoneChanged { .. })));
        assert!(matches!(clone.decision, Some(Decision::Payment { .. })));
        let land = g.put(0, 4, Zone::Battlefield);
        let cast=paid("GGGG", Move::WithMana(vec![Step { plan: vec![], x: None, life: vec![], with: None,cost_branches:None,chose:vec![],source:land,ability:0,paying:vec![],adding:vec![]}],Box::new(Move::Cast{card,targets:vec![]})));
        g.act(0,if graveyard {Move::CastVia(game::announcement::CastRoute::Flashback,Box::new(cast))} else {cast}).unwrap();
        assert_eq!(g.stack.len(), 1);
        assert_ne!(g.stack[0], abandoned, "reversal never reuses an exposed incarnation");
        assert_eq!(g.players[0].pool.total(), 0);
        clone.act(0, Move::ReversePayment).unwrap();
    }
}

#[test]
fn r733_1_reversing_activation_releases_only_its_own_activation_limit() {
    let (mut g, source, _, _, evolution) = setup("{4}");
    let used = g.put(0, 5, Zone::Battlefield);
    let used_activation = Move::Activate { source: used, ability: 0, targets: vec![], paying: vec![] };
    g.players[0].pool.colorless = 4;
    g.act(0, paid("CCCC", used_activation.clone())).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    g.untap(used); // otherwise a tap-cost failure would hide a cleared activation limit
    let consumer = g.put(0, 5, Zone::Battlefield);
    let activate = Move::Activate { source: consumer, ability: 0, targets: vec![], paying: vec![] };
    g.act(0, paid("GGGG", Move::WithMana(vec![step(source)], Box::new(activate.clone())))).unwrap();
    choose(&mut g, evolution);
    g.act(0, Move::ReversePayment).unwrap();
    cite!(["733.1"]; assert!(!g.obj(consumer).tapped));
    assert!(g.obj(source).tapped);
    let land = g.put(0, 4, Zone::Battlefield);
    cite!(["733.1", "602.1b"]; ok: g.act(0, paid("GGGG", Move::WithMana(vec![Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source: land, ability: 0, paying: vec![], adding: vec![] }], Box::new(activate)))));
    assert!(g.obj(consumer).tapped);
    assert_eq!(g.stack.len(), 1);
    assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Activated { source: s, .. } if *s == consumer)).count(), 1);
    g.players[0].pool.colorless = 4;
    let before = format!("{g:?}");
    cite!(["602.1b"]; refused("that ability is activated only once each turn (602.1b)"): g.act(0, used_activation));
    assert_eq!(format!("{g:?}"), before);
}

#[test]
fn r733_1_reversed_spell_does_not_cast_but_keeps_its_mana_ability_triggers() {
    let (mut g, source, card, _, evolution) = setup("{4}");
    g.put(0, 6, Zone::Battlefield);
    begin(&mut g, source, card, "GGGG");
    choose(&mut g, evolution);
    cite!(["603.3"]; assert_eq!(g.stack.len(), 1, "mana triggers wait until the announcement ends"));
    g.act(0, Move::ReversePayment).unwrap();
    cite!(["733.1"]; assert_eq!(g.stack.len(), 1, "only the life-gain trigger remains"));
    cite!(["733.1"]; assert!(!g.log.iter().any(|e| matches!(e, Event::Cast { .. }))));
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    assert_eq!(g.players[0].life, 22);
    assert_eq!(g.players[1].life, 19);
    assert!(g.stack.is_empty());
}

#[test]
fn r605_3b_a_complete_payment_refuses_extra_mana_answers_before_any_question() {
    let defs = vec![
        CardDef::new("Source", "{0}", "Artifact", "{T}: Add X mana of any one color, where X is the number of charge counters on this artifact.", None).unwrap(),
        CardDef::new("Spell", "{0}", "Sorcery", "You gain 1 life.", None).unwrap(),
    ];
    let mut g = Game::new(defs, 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let card = g.put(0, 1, Zone::Hand);
    let before = format!("{g:?}");
    let invalid = Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source, ability: 0, paying: vec![], adding: vec![vec![ManaSymbol::Color(ir::Color::Green)]] };
    cite!(["608.2d"]; refused("that mana ability offers no such choice of mana (608.2d)"): g.act(0, Move::WithMana(vec![invalid], Box::new(Move::Cast { card, targets: vec![] }))));
    assert_eq!(format!("{g:?}"), before);
}

#[test]
fn r605_3a_recursive_parent_retains_its_child_through_questions_and_clone() {
    let (mut g, source, card, scales, evolution) = setup("{W}");
    let filter = g.put(0, 7, Zone::Battlefield);
    let nested = Step {plan:vec![step(source)],with:Some(mana_pool("G")),adding:vec![],source:filter,..step(filter)};
    g.act(0,paid("W",Move::WithMana(vec![nested],Box::new(Move::Cast{card,targets:vec![]})))).unwrap();
    assert!(g.obj(source).tapped);
    assert!(!g.obj(filter).tapped,"the parent's cost waits for its child's resolution (605.3b)");
    let mut clone = g.clone();
    choose(&mut clone,evolution);
    choose(&mut g,scales);
    g.act(0,Move::Choose(Answer::Mana(vec![ManaSymbol::Color(ir::Color::Green);4]))).unwrap();
    for (case,game,left) in [("scales first",&g,3),("evolution first",&clone,2)] {
        cite!(["601.2h","605.3b","616.1"];case:case;assert_eq!(game.players[0].pool.total(),left));
        assert_eq!(game.players[0].life,22,"the child's gain executes once");
        assert!(game.obj(filter).tapped && game.obj(source).tapped);
        assert!(game.decision.is_none());
        assert_eq!(game.log.iter().filter(|e|matches!(e,Event::Activated{..})).count(),2);
        assert_eq!(game.log.iter().filter(|e|matches!(e,Event::Cast{..})).count(),1);
    }
}
