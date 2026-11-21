//! Creation from a completed destruction's result. These cover the nested target-player
//! selector, optional destruction as an attempt, and ordinary Create amounts per player.
//! Run with `r701::r701_7_result_creation_runtime_review::`.
use crate::support::cite;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

/// After resolution: (P1 artifact alive, tapped objects, life, token controllers).
/// Objects: 0 = P1 artifact, 1 and 2 = P1 Bears, 3 = P0 Bear, 4 and 5 = P1 Mountains.
fn run(text: &str, kind: &str, targets: &dyn Fn(&[ObjId]) -> Vec<Vec<Target>>, consent: bool) -> (bool, Vec<usize>, Vec<i32>, Vec<usize>) {
    let pt = kind.starts_with("Creature").then_some((2, 2));
    let probe = CardDef::new("Probe", "{0}", kind, text, pt).unwrap();
    game::play::check(&probe).unwrap_or_else(|e| panic!("{text}: {}", e.0));
    let mut g = Game::new(vec![probe, CardDef::new("Artifact", "{0}", "Artifact", "", None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Mountain", "", "Basic Land — Mountain", "", None).unwrap()], 2, 20);
    g.players[0].pool.colors = [10; 5];
    let ids = vec![g.put(1, 1, Zone::Battlefield), g.put(1, 2, Zone::Battlefield), g.put(1, 2, Zone::Battlefield),
        g.put(0, 2, Zone::Battlefield), g.put(1, 3, Zone::Battlefield), g.put(1, 3, Zone::Battlefield)];
    if kind.starts_with("Creature") {
        let source = g.put(0, 0, Zone::Battlefield);
        g.act(0, Move::Activate { source, ability: 0, targets: targets(&ids), paying: vec![] }).unwrap();
    } else {
        let card = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card, targets: targets(&ids) }).unwrap();
    }
    for _ in 0..30 {
        match &g.decision {
            Some(Decision::Choose { player, choice: Choice::May, .. }) => { let p = *player; g.act(p, Move::Choose(if consent { Answer::Yes } else { Answer::No })).unwrap(); }
            Some(Decision::Choose { player: 1, choice: Choice::OrderCards { to: game::state::Placement::Zone(Zone::Graveyard), cards } }) => {
                assert!(text.starts_with("Destroy all creatures target player controls."));
                assert_eq!(cards, &vec![ids[1],ids[2]]);
                g.act(1,Move::Choose(Answer::Objects(vec![ids[2],ids[1]]))).unwrap();
            }
            Some(d) => panic!("{text}: unexpected {d:?}"),
            None if g.stack.is_empty() => break,
            None => { let p = g.priority; g.act(p, Move::Pass).unwrap(); }
        }
    }
    let tapped = (0..ids.len()).filter(|i| g.obj(ids[*i]).tapped).collect();
    let tokens = g.battlefield.iter().filter(|o| g.obj(**o).token()).map(|o| g.controller(*o)).collect();
    (g.battlefield.contains(&ids[0]), tapped, g.players.iter().map(|p| p.life).collect(), tokens)
}

/// Nested "target player" selectors are admitted and read as follows.
/// - Each "target player" mention is its own announced slot, so it may be a different player
///   (115.1).
/// - "That player" and a shared clause reuse the captured target. Nothing is targeted twice.
#[test]
fn nested_target_players_are_separate_slots_and_that_player_retains_the_capture() {
    let both = |_: &[ObjId]| vec![vec![Target::Player(1)], vec![Target::Player(0)]];
    let one = |_: &[ObjId]| vec![vec![Target::Player(1)]];
    // P1's creatures are tapped; P0's (untapped) lands are untapped.
    cite!(["115.3"]; assert_eq!(run("Tap all creatures target player controls. Untap all lands target player controls.", "Sorcery", &both, true).1, vec![1, 2]));
    // The first slot gains life (P1), the second slot's creatures are tapped (P0's Bear).
    let (_, tapped, life, _) = run("Target player gains 2 life. Tap all creatures target player controls.", "Sorcery", &both, true);
    cite!(["115.3"]; assert_eq!((tapped, life), (vec![3], vec![20, 22])));
    cite!(["608.2c"]; assert_eq!(run("Tap all creatures target player controls and all lands that player controls.", "Sorcery", &one, true).1, vec![1, 2, 4, 5]));
    // P1's two Bears die; that player gets one token and two life.
    cite!(["608.2c", "701.7a"]; assert_eq!(run("Destroy all creatures target player controls. That player creates a 1/1 white Soldier creature token.", "Sorcery", &one, true).3, vec![1]));
    cite!(["608.2c", "700.4"]; assert_eq!(run("Destroy all creatures target player controls. That player gains 1 life for each creature that died this way.", "Sorcery", &one, true).2, vec![20, 22]));
}

/// Optional destruction as an attempt gives "if you do" and "if you don't" their
/// consent outcomes.
#[test]
fn optional_destroy_attempt_supplies_if_you_do_outcomes() {
    let artifact = |ids: &[ObjId]| vec![vec![Target::Object(ids[0])]];
    let c = "Creature — Bear";
    cite!(["608.2d"]; assert_eq!(run("{T}: You may destroy target artifact. If you do, you gain 3 life.", c, &artifact, true).0, false));
    cite!(["608.2c"]; assert_eq!(run("{T}: You may destroy target artifact. If you do, you gain 3 life.", c, &artifact, true).2, vec![23, 20]));
    cite!(["608.2c"]; assert_eq!(run("{T}: You may destroy target artifact. If you do, you gain 3 life.", c, &artifact, false).2, vec![20, 20]));
    cite!(["608.2c"]; assert_eq!(run("{T}: You may destroy target artifact. If you don't, you gain 3 life.", c, &artifact, false).2, vec![23, 20]));
}

/// Create's amount is evaluated per player, but
/// ordinary amounts don't change: "you" is still the controller.
#[test]
fn ordinary_create_amounts_are_unchanged_by_per_player_evaluation() {
    let none = |_: &[ObjId]| vec![];
    let one = |_: &[ObjId]| vec![vec![Target::Player(1)]];
    // P0 controls one creature: each player creates one token.
    cite!(["109.5", "701.7a"]; assert_eq!(run("Each player creates a 1/1 white Soldier creature token for each creature you control.", "Sorcery", &none, true).3.len(), 2));
    cite!(["701.7a"]; assert_eq!(run("Each player creates two 1/1 white Soldier creature tokens.", "Sorcery", &none, true).3, vec![0, 0, 1, 1]));
    cite!(["701.7a"]; assert_eq!(run("Target player creates two Treasure tokens.", "Sorcery", &one, true).3, vec![1, 1]));
}
