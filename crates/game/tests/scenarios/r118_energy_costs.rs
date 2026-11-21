//! Energy is paid by removing the payer's counters, once (107.14, 118.3/10, 605.3b).
use crate::support::{cite, paid};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use game::legal::Step;
fn step(source: ObjId, ability: usize) -> Step { Step { plan: vec![], x: None, life: vec![], with: None, source, ability, paying: vec![], adding: vec![], chose: vec![], cost_branches: None } }
use game::state::{Answer, Choice, Decision};
use ir::{Color, mana::ManaSymbol};
fn card(name: &str, cost: &str, text: &str) -> CardDef {
    let d = CardDef::new(name, cost, "Artifact", text, None).unwrap();
    game::play::check(&d).unwrap(); d
}
fn spell(cost: &str, text: &str) -> CardDef {
    let d = CardDef::new("Spell", cost, "Instant", text, None).unwrap();
    game::play::check(&d).unwrap(); d
}
fn hub() -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let d = CardDef::from_faces(CardDef::catalog(&faces).get("Aether Hub").unwrap()).unwrap();
    game::play::check(&d).unwrap(); d
}
fn activate(source: ObjId, ability: usize) -> Move { Move::Activate { source, ability, targets: vec![], paying: vec![] } }
fn removals(g: &Game, player: usize) -> Vec<u32> {
    g.log.iter().filter_map(|e| match e { Event::CountersRemoved { from: Target::Player(p), kind, count } if *p == player && kind == "energy" => Some(*count), _ => None }).collect()
}
fn resolve(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
#[test]
fn full_hub_entry_colorless_colored_and_exhaustion() {
    let mut g = Game::new(vec![hub()], 2, 20);
    let hand = g.put(0, 0, Zone::Hand);
    g.act(0, Move::PlayLand(hand)).unwrap();
    resolve(&mut g);
    let source = g.battlefield[0];
    assert_eq!(g.counters(Target::Player(0))["energy"], 1);
    g.act(0, Move::Adding(vec![vec![ManaSymbol::Color(Color::Green)]], Box::new(activate(source, 2)))).unwrap();
    cite!(["107.14"]; assert_eq!(removals(&g, 0), vec![1]));
    cite!(["605.3b"]; assert!(g.stack.is_empty()));
    g.untap(source);
    let before = format!("{g:?}");
    cite!(["118.3"]; refused("player 0 can't have 1 energy counters removed to pay this cost (118.3, 118.10)"): g.act(0, Move::Adding(vec![vec![ManaSymbol::Color(Color::Blue)]], Box::new(activate(source, 2)))));
    assert_eq!(format!("{g:?}"), before);
    g.act(0, activate(source, 1)).unwrap();
    assert_eq!(g.players[0].pool.total(), 2);
}
#[test]
fn whole_cost_reserves_duplicate_energy_and_only_payer_counters() {
    for cost in ["Pay {E}{E}", "Pay {E}, Pay {E}"] {
        let mut g = Game::new(vec![card("Consumer", "{0}", &format!("{cost}: You gain 1 life."))], 2, 20);
        let source = g.put(1, 0, Zone::Battlefield);
        g.put_counters(Target::Player(0), "energy", 9);
        g.put_counters(Target::Player(1), "energy", 1);
        g.priority = 1;
        let before = format!("{g:?}");
        let need = if cost == "Pay {E}{E}" { 2 } else { 1 };
        cite!(["118.3", "118.10"]; case: cost; refused(&format!("player 1 can't have {need} energy counters removed to pay this cost (118.3, 118.10)")): g.act(1, activate(source, 0)));
        assert_eq!(format!("{g:?}"), before);
        g.put_counters(Target::Player(1), "energy", 1);
        g.act(1, activate(source, 0)).unwrap();
        cite!(["107.14"]; case: cost; assert_eq!(removals(&g, 1).iter().sum::<u32>(), 2));
        cite!(["107.14"]; case: cost; assert_eq!(g.counters(Target::Player(0))["energy"], 9));
    }
}
#[test]
fn authored_mana_steps_reserve_energy_for_the_final_additional_cost() {
    let resource = card("Resource", "{0}", "Pay {E}{E}: Add {G}.\nPay {E}: Add {G}.");
    let spell = spell("{G}{G}", "As an additional cost to cast this spell, pay {E}.\nYou gain 1 life.");
    let mut g = Game::new(vec![resource, spell], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let spell = g.put(0, 1, Zone::Hand);
    g.put_counters(Target::Player(0), "energy", 2);
    let plan = vec![step(source, 1), step(source, 1)];
    let before = format!("{g:?}");
    cite!(["118.3", "118.10"]; refused("player 0 can't have 1 energy counters removed to pay this cost"): g.act(0, paid("GG", Move::WithMana(plan.clone(), Box::new(Move::Cast { card: spell, targets: vec![] })))));
    assert_eq!(format!("{g:?}"), before);
    g.put_counters(Target::Player(0), "energy", 1);
    g.act(0, paid("GG", Move::WithMana(plan, Box::new(Move::Cast { card: spell, targets: vec![] })))).unwrap();
    cite!(["107.14"]; assert_eq!(removals(&g, 0), vec![1, 1, 1]));
}
#[test]
fn two_hubs_share_one_energy_pool() {
    let spell = spell("{G}{G}", "You gain 1 life.");
    let mut g = Game::new(vec![hub(), spell], 2, 20);
    let hubs = [g.put(0, 0, Zone::Battlefield), g.put(0, 0, Zone::Battlefield)];
    let spell = g.put(0, 1, Zone::Hand);
    g.put_counters(Target::Player(0), "energy", 1);
    let plan: Vec<_> = hubs.into_iter().map(|source| Step { plan: vec![], x: None, life: vec![], with: None, adding: vec![vec![ManaSymbol::Color(Color::Green)]], ..step(source, 2) }).collect();
    let before = format!("{g:?}");
    cite!(["118.3", "118.10"]; refused("player 0 can't have 1 energy counters removed to pay this cost"): g.act(0, paid("GG", Move::WithMana(plan.clone(), Box::new(Move::Cast { card: spell, targets: vec![] })))));
    assert_eq!(format!("{g:?}"), before);
    g.put_counters(Target::Player(0), "energy", 1);
    g.act(0, paid("GG", Move::WithMana(plan, Box::new(Move::Cast { card: spell, targets: vec![] })))).unwrap();
    cite!(["107.14"]; assert_eq!(removals(&g, 0), vec![1, 1]));
}
#[test]
fn suspended_mana_child_clone_and_parent_reversal_do_not_repay_energy() {
    let defs = vec![
        CardDef::new("Resource", "{0}", "Creature", "Pay {E}: Put a +1/+1 counter on this creature. Add {G}.", Some((1,1))).unwrap(),
        CardDef::new("Bear", "{0}", "Creature", "", Some((2,2))).unwrap(),
        card("Scales", "{0}", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead."),
        card("Evolution", "{0}", "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead."),
        spell("{G}{G}", "You gain 1 life."),
    ];
    for def in &defs { game::play::check(def).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    g.put(0, 2, Zone::Battlefield);
    let evolution = g.put(0, 3, Zone::Battlefield);
    let spell = g.put(0, 4, Zone::Hand);
    g.put_counters(Target::Player(0), "energy", 1);
    g.act(0, paid("GG", Move::WithMana(vec![Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source, ability: 0, paying: vec![], adding: vec![] }], Box::new(Move::Cast { card: spell, targets: vec![] })))).unwrap();
    cite!(["107.14"]; assert_eq!(removals(&g, 0), vec![1]));
    let Some(Decision::Choose { choice: Choice::Replacement { among, .. }, .. }) = &g.decision else { panic!("{:?}", g.decision) };
    let effect = among.iter().find(|e| e.source == evolution).unwrap().clone();
    let mut clone = g.clone();
    for game in [&mut g, &mut clone] {
        game.act(0, Move::Choose(Answer::Replacement(effect.clone()))).unwrap();
        assert!(matches!(game.decision, Some(Decision::Payment { .. })));
        game.act(0, Move::ReversePayment).unwrap();
        assert_eq!(removals(game, 0), vec![1]);
        assert_eq!(game.players[0].pool.total(), 1);
        cite!(["733.1"]; assert_eq!(game.obj(spell).zone, Zone::Hand));
        assert_eq!(game.obj(source).counters["+1/+1"], 3);
    }
}
