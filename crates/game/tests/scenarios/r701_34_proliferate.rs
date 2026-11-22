//! 701.34a: choose a set of existing holders; add one of every kind in one placement batch.
use crate::support::cite;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, Target, Zone};
use std::sync::OnceLock;
fn pinned(name: &str) -> CardDef {
    static FACES: OnceLock<Vec<oracle::corpus::Face>> = OnceLock::new();
    let faces = FACES.get_or_init(|| oracle::corpus::load(&oracle::corpus::default_path()).unwrap());
    CardDef::from_faces(CardDef::catalog(faces).get(name).unwrap()).unwrap()
}
fn def(name: &str, kind: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    CardDef::new(name, "{0}", kind, text, pt).unwrap()
}
fn game(text: &str) -> Game {
    let cards = vec![
        def("Probe", "Sorcery", text, None),
        def("Bear", "Creature — Bear", "Shroud", Some((2, 2))),
        def(
            "Scale",
            "Enchantment",
            "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.",
            None,
        ),
        def(
            "Double",
            "Enchantment",
            "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.",
            None,
        ),
    ];
    for d in &cards {
        game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0));
    }
    Game::new(cards, 2, 20)
}
fn resolve(g: &mut Game) {
    for _ in 0..30 {
        assert!(!matches!(g.decision, Some(Decision::Order { .. })),
            "these authored single-trigger sequences ask no ordering choice: {:?}", g.decision);
        if g.decision.is_some() || g.stack.is_empty() {
            return;
        }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("resolution stalled");
}
fn cast(g: &mut Game, def: usize, targets: Vec<Vec<Target>>, spending: &str) {
    let card = g.put(0, def, Zone::Hand);
    g.act(0, crate::support::paid(spending, Move::Cast { card, targets })).unwrap();
    resolve(g);
}
fn offered(g: &Game) -> (usize, Vec<Target>) {
    let Some(Decision::Choose { player, choice: Choice::Holders { verb: ir::Verb::Proliferate, among } }) = &g.decision
    else {
        panic!("{:?}", g.decision)
    };
    (*player, among.clone())
}
fn choose(g: &mut Game, chosen: Vec<Target>) {
    let (player, _) = offered(g);
    g.act(player, Move::Choose(Answer::Holders(chosen))).unwrap();
    resolve(g);
}
fn puts(g: &Game, since: usize) -> Vec<(Target, String, u32)> {
    g.log
        .iter()
        .skip(since)
        .filter_map(|e| match e {
            Event::CountersPut { on, kind, count, .. } => Some((*on, kind.clone(), *count)),
            _ => None,
        })
        .collect()
}
#[test]
fn mixed_kinds_non_targeting_and_invalid_answers_preserve_the_choice() {
    let mut g = game("Proliferate.");
    let mine = g.put(0, 1, Zone::Battlefield);
    let other = g.put(1, 1, Zone::Battlefield);
    let empty = g.put(0, 1, Zone::Battlefield);
    let off = g.put(0, 1, Zone::Graveyard);
    g.put_counters(mine, "+1/+1", 2);
    g.put_counters(mine, "charge", 3);
    g.put_counters(other, "flying", 1);
    g.put_counters(Target::Player(1), "poison", 2);
    let mark = g.log.len();
    cast(&mut g, 0, vec![], "");
    cite!(["701.34a"]; assert_eq!(offered(&g).1, vec![Target::Player(1), mine.into(), other.into()]));
    let decision = g.decision.clone();
    let subset = "choose a distinct subset of the offered counter holders (701.34a)";
    for (p, a, why) in [
        (1, Answer::Holders(vec![]), None),
        (0, Answer::Objects(vec![mine]), Some(subset)),
        (0, Answer::Holders(vec![mine.into(), mine.into()]), Some(subset)),
        (0, Answer::Holders(vec![off.into()]), Some(subset)),
    ] {
        match why {
            Some(why) => cite!(["701.34a"]; case: format!("{a:?}"); refused(why): g.act(p, Move::Choose(a))),
            None => crate::support::refused(g.act(p, Move::Choose(a)), "that decision is another player's"),
        }
        assert_eq!(g.decision, decision);
        assert!(puts(&g, mark).is_empty());
    }
    // a player is one of the people in the game (102.1): this game has no player 999, so the
    // answer names no one, and is refused before the rules are asked
    let nobody = g.attempt(0, Move::Choose(Answer::Holders(vec![Target::Player(999)])));
    cite!(["102.1"]; assert!(matches!(nobody, Err(game::play::ActionError::Unreferable(Target::Player(999)))), "{nobody:?}"));
    assert_eq!(g.decision, decision);
    assert!(puts(&g, mark).is_empty());
    choose(&mut g, vec![other.into(), mine.into(), Target::Player(1)]);
    assert_eq!(
        puts(&g, mark),
        vec![
            (Target::Player(1), "poison".into(), 1),
            (mine.into(), "+1/+1".into(), 1),
            (mine.into(), "charge".into(), 1),
            (other.into(), "flying".into(), 1)
        ]
    );
    cite!(["701.34a"]; assert_eq!(g.counters(mine)["+1/+1"], 3));
    cite!(["701.34a"]; assert_eq!(g.counters(mine)["charge"], 4));
    assert_eq!(g.counters(other)["flying"], 2);
    cite!(["701.34a"]; assert_eq!(g.counters(Target::Player(1))["poison"], 3));
    assert!(g.counters(empty).is_empty());
}
#[test]
fn empty_selection_empty_eligibility_and_repeated_instructions_complete_once() {
    let mut g = game("Proliferate. You gain 1 life.");
    cast(&mut g, 0, vec![], "");
    cite!(["701.34a"]; assert!(g.decision.is_none()));
    cite!(["608.2c"]; assert_eq!(g.players[0].life, 21));
    let mut g = game("Proliferate. Proliferate. You gain 1 life.");
    let bear = g.put(0, 1, Zone::Battlefield);
    g.put_counters(bear, "charge", 1);
    cast(&mut g, 0, vec![], "");
    choose(&mut g, vec![]);
    cite!(["701.34a"]; assert_eq!(g.counters(bear)["charge"], 1));
    cite!(["608.2c"]; assert_eq!(g.players[0].life, 20));
    choose(&mut g, vec![bear.into()]);
    assert!(g.decision.is_none());
    cite!(["701.34a"]; assert_eq!(g.counters(bear)["charge"], 2));
    assert_eq!(g.players[0].life, 21);
}
#[test]
fn replacement_batch_ownership_clones_and_answer_order_are_canonical() {
    let mut g = game("Proliferate. You gain 1 life.");
    let bear = g.put(0, 1, Zone::Battlefield);
    g.put_counters(bear, "+1/+1", 1);
    g.put_counters(bear, "charge", 1);
    g.put_counters(Target::Player(1), "energy", 1);
    g.put(0, 2, Zone::Battlefield);
    g.put(0, 3, Zone::Battlefield);
    let mark = g.log.len();
    cast(&mut g, 0, vec![], "");
    let mut reordered = g.clone();
    choose(&mut g, vec![bear.into(), Target::Player(1)]);
    choose(&mut reordered, vec![Target::Player(1), bear.into()]);
    assert_eq!(g.decision, reordered.decision);
    assert_eq!(g.log, reordered.log);
    assert!(puts(&g, mark).is_empty());
    let Some(Decision::Choose { player, choice: Choice::Replacement { on, among, .. } }) = g.decision.clone() else {
        panic!("no replacement")
    };
    cite!(["616.1"]; assert_eq!(player, 0));
    cite!(["616.1"]; assert_eq!(on, bear.into()));
    let mut different = g.clone();
    g.act(0, Move::Choose(Answer::Replacement(among[0].clone()))).unwrap();
    different.act(0, Move::Choose(Answer::Replacement(among[1].clone()))).unwrap();
    resolve(&mut g);
    resolve(&mut different);
    assert_eq!(g.players[0].life, 21);
    assert_eq!(different.players[0].life, 21);
    assert_eq!(g.counters(Target::Player(1))["energy"], 2);
    let amounts = [g.counters(bear)["+1/+1"], different.counters(bear)["+1/+1"]];
    let expected = if g.obj(among[0].source).chars.name == "Scale" { [5, 4] } else { [4, 5] };
    cite!(["616.1e"]; assert_eq!(amounts, expected));
    assert_eq!(puts(&g, mark).len(), 3);
    assert_eq!(puts(&different, mark).len(), 3);
}
#[test]
fn poison_threshold_is_checked_after_the_combined_commit() {
    let mut g = game("Proliferate.");
    let bear = g.put(0, 1, Zone::Battlefield);
    g.put_counters(Target::Player(1), "poison", 9);
    g.put_counters(bear, "charge", 1);
    cast(&mut g, 0, vec![], "");
    choose(&mut g, vec![Target::Player(1), bear.into()]);
    cite!(["704.5c"]; assert!(g.players[1].lost));
    cite!(["701.34a"]; assert_eq!(g.counters(bear)["charge"], 2));
}
#[test]
fn full_gambit_draws_once_and_full_clasp_enters_and_activates() {
    let mut g = Game::new(
        vec![
            pinned("Tezzeret's Gambit"),
            pinned("Contagion Clasp"),
            def("Holder", "Creature", "", Some((3, 3))),
            def("Land", "Basic Land — Island", "", None),
        ],
        2,
        20,
    );
    for d in g.defs.iter() {
        game::play::check(d).unwrap();
    }
    g.players[0].pool.colors = [10; 5];
    let bear = g.put(1, 2, Zone::Battlefield);
    g.put_counters(bear, "charge", 1);
    for _ in 0..4 {
        g.put(0, 3, Zone::Library);
    }
    cast(&mut g, 0, vec![], "WWWU");
    assert_eq!(g.players[0].hand.len(), 2);
    let mut clone = g.clone();
    choose(&mut g, vec![bear.into()]);
    choose(&mut clone, vec![]);
    cite!(["608.2c"]; assert_eq!(g.players[0].hand.len(), 2));
    assert_eq!(clone.players[0].hand.len(), 2);
    cast(&mut g, 1, vec![], "WW");
    let Some(Decision::Targets { player, .. }) = g.decision.clone() else { panic!("no ETB target") };
    g.act(player, Move::ChooseTargets(vec![vec![bear.into()]])).unwrap();
    resolve(&mut g);
    assert_eq!(g.counters(bear)["-1/-1"], 1);
    let clasp = *g.battlefield.iter().find(|o| g.obj(**o).chars.name.as_str() == "Contagion Clasp").unwrap();
    g.act(0, crate::support::paid("WWWW", Move::Activate { source: clasp, ability: 1, targets: vec![], paying: vec![] })).unwrap();
    resolve(&mut g);
    choose(&mut g, vec![bear.into()]);
    cite!(["701.34a"]; assert_eq!(g.counters(bear)["-1/-1"], 2));
    cite!(["701.34a"]; assert_eq!(g.counters(bear)["charge"], 3));
}
#[test]
fn full_volt_charge_does_not_offer_a_planeswalker_whose_last_loyalty_was_removed() {
    let mut g = Game::new(
        vec![
            pinned("Volt Charge"),
            def("Walker", "Planeswalker — Jace", "+1: You gain 1 life.", None),
            def("Bear", "Creature", "", Some((2, 2))),
        ],
        2,
        20,
    );
    game::play::check(&g.defs[0]).unwrap();
    g.players[0].pool.colors = [10; 5];
    let walker = g.put(1, 1, Zone::Battlefield);
    g.put_counters(walker, "loyalty", 3);
    let bear = g.put(0, 2, Zone::Battlefield);
    g.put_counters(bear, "charge", 1);
    cast(&mut g, 0, vec![vec![walker.into()]], "WWR");
    cite!(["701.34a"]; assert_eq!(offered(&g).1, vec![bear.into()]));
    assert_eq!(g.obj(walker).zone, Zone::Battlefield);
    assert!(g.counters(walker).is_empty());
    choose(&mut g, vec![]);
    cite!(["704.5i"]; assert_eq!(g.obj(walker).zone, Zone::Gone));
}
#[test]
fn full_anatomist_if_you_do_and_saga_chapter() {
    for accept in [false, true] {
        let mut g = Game::new(
            vec![
                pinned("Gitaxian Anatomist"),
                def("Probe", "Sorcery", "Proliferate.", None),
                def(
                    "Saga",
                    "Enchantment — Saga",
                    "I — You gain 1 life.\nII — You gain 2 life.\nIII — You gain 3 life.",
                    None,
                ),
            ],
            2,
            20,
        );
        for d in g.defs.iter() {
            game::play::check(d).unwrap();
        }
        g.players[0].pool.colors = [10; 5];
        let saga = g.put(0, 2, Zone::Battlefield);
        g.put_counters(saga, "lore", 1);
        resolve(&mut g);
        cast(&mut g, 0, vec![], "WWWU");
        assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::May { possible: true }, .. })));
        g.act(0, Move::Choose(if accept { Answer::Yes } else { Answer::No })).unwrap();
        resolve(&mut g);
        if accept {
            choose(&mut g, vec![saga.into()]);
            cite!(["701.34a"]; assert_eq!(g.counters(saga)["lore"], 2));
            cite!(["714.2b"]; assert_eq!(g.players[0].life, 23));
        } else {
            cite!(["608.2c"]; assert!(g.decision.is_none()));
            cite!(["608.2c"]; assert_eq!(g.counters(saga)["lore"], 1));
            assert_eq!(g.players[0].life, 21);
        }
    }
}
#[test]
fn full_thrummingbird_and_times_boundary() {
    let bird = pinned("Thrummingbird");
    game::play::check(&bird).unwrap();
    let mut g = Game::new(vec![bird], 2, 20);
    let bird = g.put(0, 0, Zone::Battlefield);
    g.turn = 2;
    g.part = ir::Part::BeginningOfCombat;
    for _ in 0..2 {
        g.act(g.priority, Move::Pass).unwrap();
    }
    g.put_counters(Target::Player(1), "poison", 1);
    g.act(0, Move::Attack(vec![(bird, Target::Player(1))])).unwrap();
    for _ in 0..20 {
        if matches!(g.decision, Some(Decision::Choose { choice: Choice::Holders { .. }, .. })) {
            break;
        }
        if let Some(Decision::Blockers { player, .. }) = g.decision.clone() {
            g.act(player, Move::Block(vec![])).unwrap();
        } else {
            g.act(g.priority, Move::Pass).unwrap();
        }
    }
    cite!(["701.34a"]; assert_eq!(offered(&g).1, vec![Target::Player(1)]));
    choose(&mut g, vec![Target::Player(1)]);
    assert_eq!(g.players[1].life, 19);
    cite!(["701.34a"]; assert_eq!(g.counters(Target::Player(1))["poison"], 2));
    crate::support::refused(game::play::check(&pinned("Contagion Engine")).map_err(|e|e.0), "modifier: Times of Keyword");
    for text in ["Proliferate twice.", "Proliferate X times."] {
        crate::support::refused(game::play::check(&def("No", "Sorcery", text, None)).map_err(|e|e.0),"modifier: Times of Keyword");
    }
}

#[test]
fn instructed_player_chooses_holders_but_affected_player_orders_replacements() {
    let mut g = Game::new(
        vec![
            def("Probe", "Sorcery", "Target player proliferates. You gain 1 life.", None),
            def(
                "Scale",
                "Enchantment",
                "If one or more poison counters would be put on a player, that many plus one poison counters are put on that player instead.",
                None,
            ),
            def(
                "Double",
                "Enchantment",
                "If one or more poison counters would be put on a player, twice that many poison counters are put on that player instead.",
                None,
            ),
        ],
        3,
        20,
    );
    for d in g.defs.iter() {
        game::play::check(d).unwrap();
    }
    g.put_counters(Target::Player(2), "poison", 1);
    g.put(0, 1, Zone::Battlefield);
    g.put(0, 2, Zone::Battlefield);
    cast(&mut g, 0, vec![vec![Target::Player(1)]], "");
    cite!(["701.34a"]; assert_eq!(offered(&g).0, 1));
    choose(&mut g, vec![Target::Player(2)]);
    let Some(Decision::Choose { player, choice: Choice::Replacement { on, among, .. } }) = g.decision.clone() else {
        panic!("replacement")
    };
    cite!(["616.1"]; assert_eq!(player, 2));
    assert_eq!(on, Target::Player(2));
    assert_eq!(g.counters(on)["poison"], 1);
    cite!(["616.1"]; refused("that decision is another player's"): g.act(1, Move::Choose(Answer::Replacement(among[0].clone()))));
    g.act(2, Move::Choose(Answer::Replacement(among[0].clone()))).unwrap();
    resolve(&mut g);
    cite!(["616.1"]; assert_eq!(g.counters(on)["poison"], 5));
    assert_eq!(g.players[0].life, 21);
}
