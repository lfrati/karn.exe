//! Losing for life or failed draws waits for the next priority check (104.3b–c, 704.4).
use crate::support::cite;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, Target, Zone};

fn spell(text: &str) -> Game {
    let defs = vec![
        CardDef::new("Trial", "{0}", "Instant", text, None).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0)); }
    Game::new(defs, 3, 20)
}

fn cast(g: &mut Game, targets: Vec<Vec<Target>>) {
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets }).unwrap();
    for _ in 0..3 { g.act(g.priority, Move::Pass).unwrap(); }
}

#[test]
fn r104_3b_a_player_at_zero_or_negative_life_may_recover_before_resolution_finishes() {
    for amount in [20, 21] {
        for recover in [true, false] {
            let mut g = spell(&format!("You lose {amount} life. You may gain {amount} life."));
            cast(&mut g, vec![]);
            assert_eq!(g.players[0].life, 20 - amount);
            cite!(["104.3b"]; case: format!("{amount} {recover}"); assert!(!g.players[0].lost));
            assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May }));
            g.act(0, Move::Choose(if recover { Answer::Yes } else { Answer::No })).unwrap();
            cite!(["104.3b", "704.5a"]; case: format!("{amount} {recover}"); assert_eq!(g.players[0].lost, !recover));
            assert_eq!(g.players[0].life, if recover { 20 } else { 20 - amount });
            assert!(!g.finished(), "other players continue after a loss");
            assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Lost { player: 0 })).count(), usize::from(!recover));
        }
    }
}

#[test]
fn r104_3c_refilling_a_library_does_not_undo_an_earlier_failed_draw() {
    let mut g = spell("You draw two cards. Put target creature card from your graveyard on top of your library. You may gain 1 life.");
    g.put(0, 1, Zone::Library);
    let bear = g.put(0, 2, Zone::Graveyard);
    cast(&mut g, vec![vec![Target::Object(bear)]]);
    cite!(["104.3c"]; assert_eq!(g.players[0].hand.len(), 1, "draw as many cards as were available"));
    assert_eq!(g.players[0].library.len(), 1, "the later instruction refilled the library");
    assert_eq!(g.obj(g.players[0].library[0]).chars.name, "Bear");
    assert!(!g.players[0].lost, "resolution has not finished");
    assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May }));
    g.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["104.3c", "704.5b"]; assert!(g.players[0].lost));
    assert_eq!(g.priority, 1);
    assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Drew { player: 0, .. })).count(), 1);
}

#[test]
fn r104_3c_an_empty_library_without_a_failed_draw_does_not_cause_a_loss() {
    let mut g = spell("You draw zero cards.");
    assert!(g.players[0].library.is_empty());
    cast(&mut g, vec![]);
    cite!(["704.5b"]; assert!(!g.players[0].lost));
    assert!(!g.log.iter().any(|e| matches!(e, Event::Drew { .. } | Event::Lost { .. })));
}

#[test]
fn r104_3b_paying_all_life_can_be_recovered_by_a_mana_ability_but_not_a_stack_ability() {
    for mana in [true, false] {
        let text = if mana { "{T}, Pay 20 life: Add {G}. You gain 1 life." } else { "{T}, Pay 20 life: You gain 1 life." };
        let def = CardDef::new("Relic", "{0}", "Artifact", text, None).unwrap();
        game::play::check(&def).unwrap();
        let mut g = Game::new(vec![def], 3, 20);
        let source = g.put(0, 0, Zone::Battlefield);
        g.act(0, Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }).unwrap();
        cite!(["104.3b", "605.3b"]; case: format!("mana ability: {mana}"); assert_eq!(g.players[0].lost, !mana));
        assert_eq!(g.players[0].life, if mana { 1 } else { 0 });
        assert_eq!(g.players[0].pool.total(), u32::from(mana));
        assert!(g.stack.is_empty(), "a departed player's waiting ability cannot recover them");
    }
}
