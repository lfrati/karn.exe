//! Draw ordinals belong to the completed draw, not the eventual trigger-delivery state.
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, Zone};
use crate::support::cite;

fn setup() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let defs = vec![
        CardDef::from_face(faces.iter().find(|f| f.face_name == "Sneaky Snacker").unwrap()).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Draw four", "{0}", "Instant", "Draw four cards.", None).unwrap(),
        CardDef::new("Draw one", "{0}", "Instant", "Draw a card.", None).unwrap(),
        CardDef::new("Dredger", "{0}", "Creature", "Dredge 1", Some((1, 1))).unwrap(),
        CardDef::new("Late discard", "{0}", "Instant", "Draw three cards. Discard a card.", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|e| panic!("{}: {e:?}", d.name()));
    }
    let mut g = Game::new(defs, 3, 20);
    for p in 0..3 {
        for _ in 0..20 {
            g.put(p, 1, Zone::Library);
        }
    }
    g
}
fn passes(g: &mut Game) {
    for _ in 0..3 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn cast(g: &mut Game, def: usize) {
    while g.priority != 0 {
        g.act(g.priority, Move::Pass).unwrap();
    }
    let card = g.put(0, def, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    passes(g);
}
fn returned(g: &Game) -> bool {
    g.battlefield.iter().any(|&o| g.obj(o).card == Some(0))
}
#[test]
fn r121_2_four_draws_trigger_the_third_once_even_before_trigger_delivery() {
    let mut g = setup();
    let snack = g.put(0, 0, Zone::Hand);
    g.move_object(snack, Zone::Graveyard);
    cast(&mut g, 2);
    cite!(["121.2"]; assert_eq!(g.stack.len(), 1));
    assert!(!returned(&g));
    passes(&mut g);
    assert!(returned(&g));
    let snack = *g.battlefield.last().unwrap();
    assert!(g.obj(snack).tapped);
    g.move_object(snack, Zone::Graveyard);
    cast(&mut g, 3);
    assert!(!returned(&g));
    assert!(g.stack.is_empty());
}
#[test]
fn r603_2_draws_are_counted_before_the_watcher_arrives_and_per_player() {
    let mut g = setup();
    g.draw(0, 2);
    g.draw(1, 3);
    let snack = g.put(0, 0, Zone::Hand);
    g.move_object(snack, Zone::Graveyard);
    cast(&mut g, 3);
    cite!(["603.2"]; assert_eq!(g.stack.len(), 1));
    passes(&mut g);
    assert!(returned(&g));
}
#[test]
fn r113_6m_a_later_graveyard_arrival_does_not_react_to_earlier_draws() {
    let mut g = setup();
    let snack = g.put(0, 0, Zone::Hand);
    cast(&mut g, 5);
    assert!(g.decision.is_some());
    g.act(0, Move::Choose(Answer::Objects(vec![snack]))).unwrap();
    cite!(["113.6m"]; assert!(g.stack.is_empty()));
    assert!(!returned(&g));
}
#[test]
fn r614_6_replaced_draws_do_not_count_and_a_suspended_clone_keeps_its_ordinal() {
    let mut g = setup();
    g.draw(0, 2);
    let snack = g.put(0, 0, Zone::Hand);
    g.move_object(snack, Zone::Graveyard);
    let dredger = g.put(0, 4, Zone::Graveyard);
    cast(&mut g, 3);
    let Some(Decision::Choose { choice: Choice::Replacement { among, .. }, .. }) = &g.decision else {
        panic!("{:?}", g.decision)
    };
    let replacement = among.iter().find(|r| r.source == dredger).unwrap().clone();
    let mut other = g.clone();
    g.act(0, Move::Choose(Answer::Replacement(replacement))).unwrap();
    cite!(["614.6"]; assert!(g.stack.is_empty()));
    other.act(0, Move::Choose(Answer::No)).unwrap();
    assert_eq!(other.stack.len(), 1);
    passes(&mut other);
    assert!(returned(&other));
    cast(&mut g, 3);
    cite!(["614.6"]; assert_eq!(g.stack.len(), 1));
    passes(&mut g);
    assert!(returned(&g));
}
#[test]
fn r500_1_draw_counts_expire_on_turn_transition() {
    let mut g = setup();
    g.draw(0, 3);
    let snack = g.put(0, 0, Zone::Hand);
    g.move_object(snack, Zone::Graveyard);
    while g.active == 0 {
        g.act(g.priority, Move::Pass).unwrap();
    }
    while g.priority != 0 {
        g.act(g.priority, Move::Pass).unwrap();
    }
    cast(&mut g, 3);
    // Uncited (card text): "When you draw your third card in a turn": the previous turn's draws don't count.
    assert!(g.stack.is_empty());
    cast(&mut g, 3);
    // Uncited (card text): "your third card in a turn": this is only the second.
    assert!(g.stack.is_empty());
    cast(&mut g, 3);
    // Uncited (card text): "your third card in a turn".
    assert_eq!(g.stack.len(), 1);
    passes(&mut g);
    assert!(returned(&g));
}

/// "your third creature card": the third draw of a creature card this turn, not the third draw
/// (603.2, 121.2). Drawn one at a time, a land between them.
#[test]
fn r603_2_a_filtered_draw_ordinal_counts_only_the_cards_it_describes() {
    let defs = vec![
        CardDef::new("Filtered", "{0}", "Artifact", "Whenever you draw your third creature card each turn, you gain 1 life.", None).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Draw one", "{0}", "Instant", "Draw a card.", None).unwrap(),
    ];
    game::play::check(&defs[0]).unwrap();
    let mut g = Game::new(defs, 2, 20);
    for def in [2, 1, 2, 1, 2, 2] { g.put(0, def, Zone::Library); }
    for _ in 0..4 { g.put(1, 1, Zone::Library); }
    g.put(0, 0, Zone::Battlefield);
    let mut life = vec![];
    for _ in 0..6 {
        let card = g.put(0, 3, Zone::Hand);
        g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
        let creatures = g.players[0].hand.iter().filter(|o| g.obj(**o).card == Some(2)).count();
        life.push((creatures, g.players[0].life));
    }
    cite!(["603.2"]; assert_eq!(life.iter().map(|(_, l)| *l).collect::<Vec<_>>(), life.iter().map(|(c, _)| 20 + i32::from(*c >= 3)).collect::<Vec<_>>(), "{life:?}"));
    assert_eq!(g.players[0].life, 21, "once: the fourth creature card is not the third");
}
