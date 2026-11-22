//! Keyword expansions are the ability's text when it is put on the stack (405.4).
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Target, Zone};
use game::simulation::Simulation;
use game::state::Decision;

fn definitions(names: &[&str]) -> Vec<CardDef> {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    names.iter().map(|name| {
        let def = CardDef::from_faces(&catalog[*name]).unwrap();
        game::play::check(&def).unwrap();
        def
    }).collect()
}

#[test]
fn r405_4_firebending_has_its_amount_and_retention_text_on_the_stack_for_both_seats() {
    // 702.189a: add N {R}, retaining that mana until end of combat. Student's
    // pinned Oracle defines N as its power; Fire Sages has a fixed amount.
    for name in ["Firebending Student", "Fire Sages"] {
        let mut g = Game::new(definitions(&[name]), 2, 20);
        let source = g.put(0, 0, Zone::Battlefield);
        while g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
        g.act(0, Move::Attack(vec![(source, Target::Player(1))])).unwrap();
        assert_eq!(g.stack.len(), 1);
        let top = g.stack[0];
        let mut sim = Simulation::new(g);
        let mut texts = Vec::new();
        for seat in 0..2 {
            let prompt = sim.prompt(seat).unwrap();
            let text = prompt.entities[&top].occurrence.as_ref().unwrap().text.as_deref()
                .unwrap_or_else(|| panic!("{name}'s firebending ability must have text"));
            cite!(["405.4", "702.189a"]; assert!(text.contains("{R}") && text.contains("Until end of combat")
                && text.contains("don't lose this mana"), "{name}: {text}"));
            if name == "Firebending Student" {
                cite!(["405.4", "702.189a"]; assert!(text.contains("power"), "{text}"));
            }
            texts.push(text.to_owned());
        }
        cite!(["405.4"]; assert_eq!(texts[0], texts[1]));
    }
}

#[test]
fn r405_4_partner_with_waiting_and_stacked_text_names_the_actual_partner() {
    let mut g = Game::new(definitions(&["Alisaie Leveilleur", "Alphinaud Leveilleur"]), 2, 20);
    let source = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colorless = 2;
    g.players[0].pool.colors[ir::Color::White as usize] = 1;
    g.act(0, paid("CCW", Move::Cast {card:source, targets:vec![]})).unwrap();
    for player in 0..2 { g.act(player, Move::Pass).unwrap(); }
    let mut sim = Simulation::new(g);
    let prompt = sim.prompt(0).unwrap();
    let waiting = prompt.decision.as_ref().unwrap().occurrence.as_ref().unwrap().text.as_deref().unwrap();
    cite!(["603.3d", "702.124j"]; assert!(waiting.contains("Alphinaud Leveilleur") && waiting.contains("search")
        && waiting.contains("shuffle"), "{waiting}"));
    let request = sim.request().unwrap().unwrap().clone();
    sim.respond(request.id, request.player, Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap();
    let prompt = sim.prompt(1).unwrap();
    let top = *prompt.view.stack.last().unwrap();
    let stacked = prompt.entities[&top].occurrence.as_ref().unwrap().text.as_deref().unwrap();
    cite!(["405.4", "702.124j"]; assert_eq!(stacked, waiting));
}

fn cast_copy_keyword(name: &str, pay: bool) -> Simulation {
    let mut g = Game::new(definitions(&[name, "Ornithopter", "Grizzly Bears"]), 2, 20);
    let target = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast {card:target, targets:vec![]}).unwrap();
    let target = *g.stack.last().unwrap();
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colorless = 2;
    g.players[0].pool.colors[ir::Color::Blue as usize] = 1;
    let action = Move::Cast {card, targets:vec![vec![Target::Object(target)]]};
    let (action, mana) = if name == "Make Disappear" {
        let action = if pay {
            let fodder = g.put(0, 2, Zone::Battlefield);
            Move::WithOptional(true, Box::new(Move::WithCosts(vec![vec![fodder]], Box::new(action))))
        } else {action};
        (action, "CU")
    } else {
        (Move::WithReplicate(if pay {2} else {0}, Box::new(action)), if pay {"CCU"} else {"U"})
    };
    g.act(0, paid(mana, action)).unwrap();
    Simulation::new(g)
}

#[test]
fn r405_4_casualty_and_replicate_keep_printed_costs_and_their_own_copy_text() {
    for (name, keyword, rule) in [("Make Disappear", "Casualty 1", "702.153a"),
                                  ("Consign to Memory", "Replicate {1}", "702.56a")] {
        for pay in [false, true] {
            let mut sim = cast_copy_keyword(name, pay);
            let prompt = sim.prompt(0).unwrap();
            cite!([rule]; assert_eq!(prompt.view.stack.len(), if pay {3} else {2}, "{name}: optional cost paid={pay}"));
            let spell = prompt.entities.values().find(|entity| entity.text.iter().any(|row| row == keyword)).unwrap();
            cite!([rule]; assert!(spell.text.iter().any(|row| row == keyword)));
            if !pay {continue;}
            let top = *prompt.view.stack.last().unwrap();
            let text = prompt.entities[&top].occurrence.as_ref().unwrap().text.as_deref()
                .unwrap_or_else(|| panic!("{name}'s cast trigger must have text"));
            cite!(["405.4", rule]; assert!(text.contains("copy") && text.contains("new targets") && text.contains("paid"), "{text}"));
            if name == "Make Disappear" {
                cite!(["405.4", "702.153a"]; assert!(text.contains("casualty cost"), "{text}"));
            } else {
                cite!(["405.4", "702.56a"]; assert!(text.contains("replicate cost") && text.contains("time"), "{text}"));
            }
            let expected = text.to_owned();
            let other = sim.prompt(1).unwrap();
            cite!(["405.4", rule]; assert_eq!(other.entities[&top].occurrence.as_ref().unwrap().text.as_deref(), Some(expected.as_str())));
        }
    }
}

#[test]
fn r113_7a_a_transform_during_resolution_keeps_waiting_order_and_target_text() {
    let mut defs = definitions(&["Smoldering Werewolf", "Warstorm Surge"]);
    // A controlled procedure isolates 608.2c's instruction order. The triggered
    // sources are full pinned cards; this fixture is not a card-coverage claim.
    let effect = CardDef::new("Recovery procedure", "{0}", "Instant",
        "Put target creature card from your graveyard onto the battlefield. Transform it.", None).unwrap();
    game::play::check(&effect).unwrap();
    defs.push(effect);
    let mut g = Game::new(defs, 2, 20);
    let wolf = g.put(0, 0, Zone::Graveyard);
    g.put(0, 1, Zone::Battlefield);
    let effect = g.put(0, 2, Zone::Hand);
    g.act(0, Move::Cast {card:effect, targets:vec![vec![Target::Object(wolf)]]}).unwrap();
    for player in 0..2 { g.act(player, Move::Pass).unwrap(); }
    let wolf = *g.battlefield.iter().find(|id| g.def(**id).is_some_and(|d| d.name() == "Erupting Dreadwolf")).unwrap();
    let Some(Decision::Order {abilities,..}) = &g.decision else {panic!("both entry triggers must wait for order")};
    let first = abilities.iter().position(|t| t.source == Some(wolf)).unwrap();
    let order = std::iter::once(first).chain((0..abilities.len()).filter(|i| *i != first)).collect();
    let mut sim = Simulation::new(g);
    let prompt = sim.prompt(0).unwrap();
    let ordered = &prompt.decision.as_ref().unwrap().ordered;
    let text = ordered.iter().find(|o| o.source == Some(wolf)).unwrap().text.as_deref().unwrap();
    cite!(["113.7a", "603.3b", "608.2c"]; assert!(text.contains("enters") && text.contains("1 damage")
        && text.contains("two target creatures"), "{text}"));
    let expected = text.to_owned();
    let request = sim.request().unwrap().unwrap().clone();
    sim.respond(request.id, request.player, Move::Order(order)).unwrap();
    for _ in 0..2 {
        let prompt = sim.prompt(0).unwrap();
        let occurrence = prompt.decision.as_ref().unwrap().occurrence.as_ref().unwrap();
        let targets = if occurrence.source == Some(wolf) {
            cite!(["113.7a", "603.3d"]; assert_eq!(occurrence.text.as_deref(), Some(expected.as_str())));
            vec![vec![]] // Smoldering Werewolf targets up to two creatures.
        } else { vec![vec![Target::Player(1)]] };
        let request = sim.request().unwrap().unwrap().clone();
        sim.respond(request.id, request.player, Move::ChooseTargets(targets)).unwrap();
    }
    for seat in 0..2 {
        let prompt = sim.prompt(seat).unwrap();
        let occurrence = prompt.view.stack.iter().find_map(|id| prompt.entities[id].occurrence.as_ref()
            .filter(|o| o.source == Some(wolf))).unwrap();
        cite!(["113.7a", "405.4"]; assert_eq!(occurrence.text.as_deref(), Some(expected.as_str())));
    }
}
