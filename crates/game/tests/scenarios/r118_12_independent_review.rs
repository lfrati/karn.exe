//! Costs paid on resolution (118.3, 118.12a), on pinned cards.
use crate::support::{cite, mana_pool, paid};
use game::card::CardDef;
use game::state::{Answer, Choice, Decision};
use game::{Game, Move, Target, Zone};

fn pinned(names: &[&str]) -> Vec<CardDef> {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    names
        .iter()
        .map(|name| {
            let def = CardDef::from_faces(catalog.get(name).unwrap_or_else(|| panic!("{name} is pinned"))).unwrap();
            game::play::check(&def).unwrap_or_else(|e| panic!("{name}: {}", e.0));
            def
        })
        .collect()
}

/// Libraries of the last card for both players.
fn game(names: &[&str]) -> Game {
    let mut g = Game::new(pinned(names), 2, 20);
    for player in 0..2 {
        for _ in 0..10 {
            g.put(player, names.len() - 1, Zone::Library);
        }
    }
    g
}

fn pass(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}

fn answer(g: &mut Game, player: usize, answer: Answer) {
    g.act(player, Move::Choose(answer)).unwrap();
}

#[test]
fn r118_3_hecatomb_requires_four_creatures_not_a_partial_sacrifice() {
    for n in [0, 3, 4, 5] {
        for pay in [false, true] {
            let mut g = game(&["Hecatomb", "Grizzly Bears", "Swamp"]);
            let creatures: Vec<_> = (0..n).map(|_| g.put(0, 1, Zone::Battlefield)).collect();
            g.put(1, 1, Zone::Battlefield);
            let card = g.put(0, 0, Zone::Hand);
            g.players[0].pool.colors[ir::Color::Black as usize] = 3;
            g.act(0, paid("BBB", Move::Cast { card, targets: vec![] })).unwrap();
            pass(&mut g);
            pass(&mut g);
            if n >= 4 {
                cite!(["118.12a"]; case: format!("{n} creatures, pay {pay}"); assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May { possible: true } }))));
                answer(&mut g, 0, if pay { Answer::Yes } else { Answer::No });
                if pay && n > 4 {
                    answer(&mut g, 0, Answer::Objects(creatures[..4].to_vec()));
                }
                if pay { cite!(["404.3"]; ok: g.act(0, Move::Choose(Answer::Objects(creatures[..4].to_vec())))); }
            }
            assert!(g.decision.is_none());
            cite!(["118.3"]; case: format!("{n} creatures, pay {pay}"); assert_eq!(
                creatures.iter().filter(|o| g.battlefield.contains(o)).count(),
                if n >= 4 && pay { n - 4 } else { n }
            ));
            cite!(["118.12a"]; case: format!("{n} creatures, pay {pay}"); assert_eq!(g.battlefield.iter().any(|&o| g.obj(o).card == Some(0)), n >= 4 && pay));
        }
    }
}

#[test]
fn r118_12a_reality_smasher_asks_the_spell_controller_not_its_own() {
    for pay in [false, true] {
        let mut g = game(&["Reality Smasher", "Lightning Bolt", "Forest"]);
        let smasher = g.put(0, 0, Zone::Battlefield);
        g.put(0, 2, Zone::Hand);
        let extras = [g.put(1, 2, Zone::Hand), g.put(1, 2, Zone::Hand)];
        g.act(0, Move::Pass).unwrap();
        let bolt = g.put(1, 1, Zone::Hand);
        g.players[1].pool.colors[ir::Color::Red as usize] = 1;
        g.act(1, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Object(smasher)]] })).unwrap();
        pass(&mut g);
        cite!(["118.12a"]; case: format!("pay {pay}"); assert!(matches!(g.decision, Some(Decision::Choose { player: 1, choice: Choice::May { possible: true } })), "{:?}", g.decision));
        cite!(["118.12a"]; refused("that decision is another player's"): g.act(0, Move::Choose(Answer::Yes)));
        answer(&mut g, 1, if pay { Answer::Yes } else { Answer::No });
        if pay {
            answer(&mut g, 1, Answer::Objects(vec![extras[1]]));
        }
        cite!(["118.12a"]; case: format!("pay {pay}"); assert_eq!(g.stack.len(), usize::from(pay)));
        assert_eq!(g.players[1].hand.len(), if pay { 1 } else { 2 });
        assert_eq!(g.players[0].hand.len(), 1);
        if pay {
            pass(&mut g);
            assert_eq!(g.obj(smasher).damage, 3);
        }
    }
}

#[test]
fn r118_12a_named_sacrifice_payment_keeps_the_prior_target_for_later_text() {
    // The payment names a different permanent; it must not steal the body's "it".
    for pay in [false, true] {
        let d = CardDef::new(
            "Procedure",
            "{0}",
            "Instant",
            "Destroy target creature unless you sacrifice an artifact. Tap it.",
            None,
        )
        .unwrap();
        game::play::check(&d).unwrap();
        let mut defs = pinned(&["Grizzly Bears", "Ornithopter", "Forest"]);
        defs.push(d);
        let mut g = Game::new(defs, 2, 20);
        let bear = g.put(1, 0, Zone::Battlefield);
        let artifact = g.put(0, 1, Zone::Battlefield);
        let procedure = g.put(0, 3, Zone::Hand);
        g.act(0, Move::Cast { card: procedure, targets: vec![vec![Target::Object(bear)]] }).unwrap();
        pass(&mut g);
        answer(&mut g, 0, if pay { Answer::Yes } else { Answer::No });
        assert!(g.decision.is_none());
        cite!(["118.12a"]; case: format!("pay {pay}"); assert_eq!(g.battlefield.contains(&bear), pay));
        assert_eq!(g.battlefield.contains(&artifact), !pay);
        if pay {
            // Uncited (card text): "Destroy target creature unless you sacrifice an artifact. Tap it.": "it" is the target creature, not the sacrificed artifact
            assert!(g.obj(bear).tapped);
        }
    }
}

#[test]
fn r118_12a_sacrificed_source_power_sets_x_from_lki_before_payment() {
    for pay in [false, true] {
        let mut g = game(&["Mausoleum Wanderer", "Giant Growth", "Opt", "Island"]);
        let wanderer = g.put(0, 0, Zone::Battlefield);
        let growth = g.put(0, 1, Zone::Hand);
        g.players[0].pool.colors[ir::Color::Green as usize] = 1;
        g.act(0, paid("G", Move::Cast { card: growth, targets: vec![vec![Target::Object(wanderer)]] })).unwrap();
        pass(&mut g);
        g.act(0, Move::Pass).unwrap();
        let opt = g.put(1, 2, Zone::Hand);
        g.players[1].pool.colors[ir::Color::Blue as usize] = 5;
        g.act(1, paid("U", Move::Cast { card: opt, targets: vec![] })).unwrap();
        let target = *g.stack.last().unwrap();
        g.act(1, Move::Pass).unwrap();
        g.act(
            0,
            Move::Activate {
                source: wanderer,
                ability: 2,
                targets: vec![vec![Target::Object(target)]],
                paying: vec![],
            },
        )
        .unwrap();
        pass(&mut g);
        cite!(["118.12a"]; case: format!("pay {pay}"); assert!(
            matches!(g.decision, Some(Decision::Choose { player: 1, choice: Choice::Pay { .. } })),
            "{:?}",
            g.decision
        ));
        answer(&mut g, 1, if pay { Answer::Pay{plan:vec![],with:Some(mana_pool("UUUU")),tapped:vec![]} } else { Answer::No });
        cite!(["118.12a"]; case: format!("pay {pay}"); assert_eq!(g.stack.len(), usize::from(pay)));
        cite!(["608.2h", "113.7a"]; case: format!("pay {pay}"); assert_eq!(
            g.players[1].pool.colors[ir::Color::Blue as usize],
            if pay { 0 } else { 4 },
            "X is last-known power 4"
        ));
    }
}

#[test]
fn r118_12a_return_payment_sends_a_borrowed_island_to_its_owner() {
    let control = CardDef::new("Control witness", "{0}", "Instant", "Gain control of target permanent.", None).unwrap();
    let mut defs = pinned(&["Waterspout Djinn", "Island", "Forest"]);
    defs.push(control);
    let mut g = Game::new(defs, 2, 20);
    for player in 0..2 {
        for _ in 0..10 {
            g.put(player, 2, Zone::Library);
        }
    }
    let djinn = g.put(0, 0, Zone::Battlefield);
    let island = g.put(1, 1, Zone::Battlefield);
    let control = g.put(0, 3, Zone::Hand);
    g.act(0, Move::Cast { card: control, targets: vec![vec![Target::Object(island)]] }).unwrap();
    pass(&mut g);
    assert_eq!(g.controller(island), 0);
    for _ in 0..300 {
        if matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May { possible: true } })) {
            break;
        }
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            None => g.act(g.priority, Move::Pass).unwrap(),
            other => panic!("unexpected {other:?}"),
        }
    }
    answer(&mut g, 0, Answer::Yes);
    cite!(["118.12a"]; assert!(g.battlefield.contains(&djinn)));
    assert!(!g.battlefield.contains(&island));
    cite!(["400.3"]; assert!(g.players[1].hand.iter().any(|&o| g.obj(o).card == Some(1))));
    assert!(!g.players[0].hand.iter().any(|&o| g.obj(o).card == Some(1)));
}

#[test]
fn r118_12a_skipped_sacrifice_result_cannot_invent_a_creature() {
    let admitted = CardDef::new(
        "Procedure",
        "{0}",
        "Sorcery",
        "Sacrifice a creature. You gain life equal to that creature's power.",
        None,
    )
    .unwrap();
    game::play::check(&admitted).unwrap();
    let d = CardDef::new(
        "Procedure",
        "{0}",
        "Sorcery",
        "Sacrifice a creature unless you pay {1}. You gain life equal to that creature's power.",
        None,
    )
    .unwrap();
    assert_eq!(game::play::check(&d).expect_err("skipped sacrifice cannot bind its creature").0, "slot 0 is read before any instruction produces it");
}
