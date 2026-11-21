//! 608.2d resolution-time token alternatives retain one chosen creation, not announcement modes.
use crate::support::{cite, paid};
use game::state::{ActionOptionBody, Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Zone};
use std::sync::OnceLock;
fn full(name: &str) -> CardDef {
    static F: OnceLock<Vec<oracle::corpus::Face>> = OnceLock::new();
    let faces = F.get_or_init(|| oracle::corpus::load(&oracle::corpus::default_path()).unwrap());
    let d = CardDef::from_faces(&CardDef::catalog(faces)[name]).unwrap();
    game::play::check(&d).unwrap();
    d
}
fn def(text: &str) -> CardDef {
    CardDef::new("Instruction", "{0}", "Sorcery", text, None).unwrap()
}
fn passes(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn setup(text: &str) -> Game {
    let d = def(text);
    game::play::check(&d).unwrap();
    Game::new(vec![d, full("Root Maze"), full("Orb of Dreams"), full("Kismet")], 2, 20)
}
fn cast(g: &mut Game) {
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    passes(g);
}
fn ask(g: &Game, p: usize) {
    let Some(Decision::Choose { player, choice: Choice::Alternative { among, optional: false } }) = &g.decision else {
        panic!("{:?}", g.decision)
    };
    assert_eq!(*player, p);
    assert_eq!(among.len(), 2);
    for (i, name) in [ir::Predefined::Food, ir::Predefined::Treasure].into_iter().enumerate() {
        assert_eq!(among[i].index, i);
        assert_eq!(among[i].body, ActionOptionBody::Create { tokens: vec![(name, 1)] });
    }
}
fn tokens(g: &Game) -> Vec<(ObjId, String, bool)> {
    g.battlefield
        .iter()
        .filter(|o| g.obj(**o).token())
        .map(|o| (*o, g.obj(*o).chars.name.clone(), g.obj(*o).tapped))
        .collect()
}
#[test]
fn both_choices_are_atomic_and_copies_finish_the_selected_body_once() {
    let mut g = setup("Create a Food token or a Treasure token. You gain 1 life.");
    cast(&mut g);
    ask(&g, 0);
    const OFFERED: &str = "choose one of the offered actions, or decline if optional (608.2d)";
    for (p, m, rule, why) in [
        (1, Move::Choose(Answer::Alternative(0)), None, "that decision is another player's"),
        (0, Move::Choose(Answer::Alternative(2)), Some("608.2d"), OFFERED),
        (0, Move::Choose(Answer::No), Some("608.2d"), OFFERED),
        (0, Move::Pass, None, "a decision is pending: a choice of the effect being applied (608.2d)"),
    ] {
        let before = format!("{g:?}");
        match rule {
            Some(rule) => cite!([rule]; case: format!("{m:?}"); refused(why): g.act(p, m)),
            None => crate::support::refused(g.act(p, m), why),
        }
        assert_eq!(format!("{g:?}"), before);
        ask(&g, 0);
    }
    let mut other = g.clone();
    for (branch, index, name) in [(&mut g, 0, "Food Token"), (&mut other, 1, "Treasure Token")] {
        branch.act(0, Move::Choose(Answer::Alternative(index))).unwrap();
        cite!(["608.2d"]; case: name; assert_eq!(tokens(branch).iter().map(|(_, n, _)| n.as_str()).collect::<Vec<_>>(), [name]));
        assert_eq!(branch.players[0].life, 21);
        assert!(branch.stack.is_empty() && branch.decision.is_none());
    }
}
#[test]
fn actual_entry_replacement_children_resume_without_reasking_and_clone_once() {
    for index in 0..2 {
        let mut g = setup("Create a Food token or a Treasure token. You gain 1 life.");
        g.put(0, 1, Zone::Battlefield);
        g.put(0, 2, Zone::Battlefield);
        cast(&mut g);
        ask(&g, 0);
        g.act(0, Move::Choose(Answer::Alternative(index))).unwrap();
        let Some(Decision::Choose { player: 0, choice: Choice::Replacement { among, .. } }) = g.decision.clone() else {
            panic!("{:?}", g.decision)
        };
        cite!(["616.1"]; case: format!("alternative {index}"); assert_eq!(among.len(), 2));
        assert!(
            matches!(g.public_choices().collect::<Vec<_>>().as_slice(), [game::state::PublicChoice::Alternative {player:0,index:i,..}] if *i==index)
        );
        assert!(tokens(&g).is_empty());
        assert_eq!(g.players[0].life, 20);
        let mut copy = g.clone();
        for (branch, effect) in [(&mut g, among[0].clone()), (&mut copy, among[1].clone())] {
            branch.act(0, Move::Choose(Answer::Replacement(effect))).unwrap();
            assert_eq!(tokens(branch).len(), 1);
            assert!(tokens(branch)[0].2);
            assert_eq!(branch.players[0].life, 21);
            assert!(branch.stack.is_empty() && branch.decision.is_none());
        }
    }
    let mut g = setup("Create a Food token or a Treasure token.");
    g.put(1, 3, Zone::Battlefield);
    cast(&mut g);
    g.act(0, Move::Choose(Answer::Alternative(1))).unwrap();
    assert_eq!(tokens(&g).len(), 1);
    assert!(tokens(&g)[0].2);
    assert!(g.decision.is_none());
}
#[test]
fn full_ant_man_and_provisioner_trigger_with_independent_choices_and_departed_source() {
    for name in ["Ant-Man's Army", "Tireless Provisioner"] {
        let mut g = Game::new(vec![full(name), full("Forest")], 2, 20);
        g.players[0].pool.colors[ir::Color::Green as usize] = 3;
        let card = g.put(0, 0, Zone::Hand);
        g.act(0, paid("GGG", Move::Cast { card, targets: vec![] })).unwrap();
        passes(&mut g);
        let source = *g.battlefield.iter().find(|o| g.def(**o).unwrap().name() == name).unwrap();
        if name == "Tireless Provisioner" {
            let land = g.put(0, 1, Zone::Hand);
            g.act(0, Move::PlayLand(land)).unwrap();
        }
        assert_eq!(g.stack.len(), 1);
        g.exile(source);
        passes(&mut g);
        ask(&g, 0);
        g.act(0, Move::Choose(Answer::Alternative(0))).unwrap();
        cite!(["113.7a"]; case: name; assert_eq!(tokens(&g)[0].1, "Food Token"));
    }
    let lands = def("Create two 0/1 green Forest land creature tokens.");
    game::play::check(&lands).unwrap();
    let mut g = Game::new(vec![lands, full("Tireless Provisioner")], 2, 20);
    let provisioner = g.put(0, 1, Zone::Battlefield);
    cast(&mut g);
    let Some(Decision::Order { player, abilities }) = &g.decision else { panic!("expected trigger order: {:?}", g.decision) };
    cite!(["603.3b"]; assert_eq!((*player, abilities.len()), (0, 2)));
    assert!(abilities.iter().all(|trigger| trigger.controller == 0 && trigger.source == Some(provisioner)));
    g.act(0, Move::Order(vec![0, 1])).unwrap();
    cite!(["603.2c"]; assert_eq!(g.stack.len(), 2));
    for index in 0..2 {
        passes(&mut g);
        ask(&g, 0);
        g.act(0, Move::Choose(Answer::Alternative(index))).unwrap();
    }
    cite!(["608.2d"]; assert_eq!(
        tokens(&g)
            .iter()
            .filter(|(_, n, _)| n == "Food Token" || n == "Treasure Token")
            .map(|(_, n, _)| n.as_str())
            .collect::<Vec<_>>(),
        ["Food Token", "Treasure Token"]
    ));
}
#[test]
fn selected_tokens_have_their_real_food_and_treasure_abilities() {
    for index in 0..2 {
        let mut g = setup("Create a Food token or a Treasure token.");
        cast(&mut g);
        g.act(0, Move::Choose(Answer::Alternative(index))).unwrap();
        let t = tokens(&g)[0].0;
        g.players[0].pool.colorless = 2;
        let activate = Move::Activate { source: t, ability: 0, targets: vec![], paying: vec![] };
        if index == 0 {
            g.act(0, paid("CC", activate)).unwrap();
            passes(&mut g);
            cite!(["111.10b"]; assert_eq!(g.players[0].life, 23));
        } else {
            g.act(0, Move::Adding(vec![vec![ir::ManaSymbol::Color(ir::Color::Green)]], Box::new(activate))).unwrap();
            cite!(["111.10a"]; assert_eq!(g.players[0].pool.colors[ir::Color::Green as usize], 1));
        }
        assert!(tokens(&g).is_empty());
    }
}
#[test]
fn result_consumers_refuse_instead_of_source_or_old_producer_fallback() {
    for text in [
        "Create a Food token or a Treasure token. Sacrifice that token.",
        "Create a Food token or a Treasure token. Exile it.",
        "Create a Food token or a Treasure token. Exile them.",
        "Create a Food token. Create a Food token or a Treasure token. Sacrifice that token.",
        "Create a Food token or a Treasure token. You gain life equal to its mana value.",
        "If you control a creature, create a Food token or a Treasure token. Exile it.",
        "You may create a Food token or a Treasure token.",
    ] {
        let d = def(text);
        let err = game::play::check(&d).expect_err(text);
        assert!(err.0.contains("alternative"), "{text}: {}", err.0);
    }
    let mut g = setup("Create a Food token and a Treasure token.");
    cast(&mut g);
    assert!(g.decision.is_none());
    assert_eq!(tokens(&g).len(), 2);
}

#[test]
fn captured_controller_chooses_after_an_opponents_turn_entry_and_source_departure() {
    let mut g = Game::new(vec![full("Ant-Man's Army"), full("Leyline of Anticipation")], 2, 20);
    g.put(1, 1, Zone::Battlefield);
    g.act(0, Move::Pass).unwrap();
    g.players[1].pool.colors[ir::Color::Green as usize] = 3;
    let card = g.put(1, 0, Zone::Hand);
    g.act(1, paid("GGG", Move::Cast { card, targets: vec![] })).unwrap();
    passes(&mut g);
    let source = *g.battlefield.iter().find(|o| g.obj(**o).chars.name == "Ant-Man's Army").unwrap();
    assert_eq!(g.active, 0);
    g.exile(source);
    passes(&mut g);
    ask(&g, 1);
    g.act(1, Move::Choose(Answer::Alternative(1))).unwrap();
    let token = tokens(&g)[0].0;
    cite!(["603.3a", "111.2"]; assert_eq!(g.controller(token), 1));
    cite!(["111.2"]; assert_eq!(g.obj(token).owner, 1));
}
