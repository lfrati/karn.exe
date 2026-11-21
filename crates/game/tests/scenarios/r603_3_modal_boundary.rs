//! CR 603.3c–d: choose legal modes before their targets; no priority during announcement.
//! Synthetic trigger text isolates targeted, untargeted, optional and divided modes.
use crate::support::cite;
use game::{CardDef, Game, Move, Target, Zone};
use game::legal::Asked;
use game::state::Decision;

fn setup(text: &str) -> Game {
    let mut g = Game::new(vec![CardDef::new("Watcher", "{0}", "Creature", text, Some((2, 2))).unwrap()], 2, 20);
    for p in 0..2 { for _ in 0..4 { g.put(p, 0, Zone::Library); } }
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    assert!(matches!(g.decision, Some(Decision::Modes { player: 0, .. })));
    g
}
fn resolve(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }

#[test]
fn r603_3c_d_modes_then_targets_preserve_choice_and_refuse_priority() {
    let mut g = setup("When this creature enters, choose one —\n• Target player loses 3 life.\n• You gain 2 life.");
    let before = format!("{g:?}");
    cite!(["603.3c"]; refused("that decision is another player's"): g.act(1, Move::ChooseModes(vec![0])));
    cite!(["700.2a"]; refused("choose 1 to 1 modes of 2"): g.act(0, Move::ChooseModes(vec![0, 1])));
    assert_eq!(format!("{g:?}"), before);
    g.act(0, Move::ChooseModes(vec![0])).unwrap();
    cite!(["603.3c", "603.3d", "700.2c"]; assert!(matches!(&g.decision, Some(Decision::Targets { modes, groups: 1, .. }) if modes == &[0])));
    assert!(matches!(g.asked(), Some((0, Asked::Targets { slots, .. })) if slots.len() == 1));
    let before = format!("{g:?}");
    cite!(["603.3d"]; refused("a decision is pending: choose the targets of the triggered ability"): g.act(0, Move::Pass));
    cite!(["603.3c"]; refused("the triggered ability's modes have already been chosen"): g.act(0, Move::ChooseModes(vec![1])));
    cite!(["603.3d"]; refused("that has 1 target group(s), 0 given"): g.act(0, Move::ChooseTargets(vec![])));
    assert_eq!(format!("{g:?}"), before);
    let mut cloned = g.clone();
    for game in [&mut g, &mut cloned] {
        game.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap();
        let trigger = game.stack[0];
        cite!(["603.3d"]; assert_eq!(game.obj(trigger).modes, vec![0]));
        assert_eq!(game.obj(trigger).targets, vec![vec![Target::Player(1)]]);
        resolve(game);
        cite!(["608.2c"]; assert_eq!(game.players[1].life, 17));
    }
}

#[test]
fn r603_3c_target_free_mode_needs_no_target_answer() {
    let mut g = setup("When this creature enters, choose one —\n• Target player loses 3 life.\n• You gain 2 life.");
    g.act(0, Move::ChooseModes(vec![1])).unwrap();
    cite!(["603.3c", "700.2c"]; assert!(g.decision.is_none()));
    assert_eq!(g.obj(g.stack[0]).modes, vec![1]);
    resolve(&mut g);
    assert_eq!(g.players[0].life, 22);
    assert_eq!(g.players[1].life, 20);
}

#[test]
fn r603_3c_unavailable_mode_refuses_and_optional_no_mode_removes_trigger() {
    let text = "When this creature enters, choose up to one —\n• Destroy target artifact.\n• You gain 2 life.";
    let mut g = setup(text);
    let before = format!("{g:?}");
    cite!(["603.3c"]; refused("the chosen modes have no legal target selection (603.3c)"): g.act(0, Move::ChooseModes(vec![0])));
    assert_eq!(format!("{g:?}"), before);
    g.act(0, Move::ChooseModes(vec![])).unwrap();
    cite!(["603.3c"]; assert!(g.stack.is_empty() && g.decision.is_none()));
    assert_eq!(g.players[0].life, 20);
}

#[test]
fn r603_3d_division_follows_chosen_modes_with_targets() {
    let mut g = setup("When this creature enters, choose one —\n• This creature deals 3 damage divided as you choose among one, two, or three targets.\n• You gain 2 life.");
    g.act(0, Move::ChooseModes(vec![0])).unwrap();
    cite!(["603.3d", "601.2d"]; assert!(matches!(g.asked(), Some((0, Asked::Targets { slots, .. })) if slots[0].divide == Some(3))));
    let targets = vec![vec![Target::Player(0), Target::Player(1)]];
    let before = format!("{g:?}");
    cite!(["601.2d"]; refused("divide 3 among the 2 targets: say what each gets (601.2d)"): g.act(0, Move::ChooseTargets(targets.clone())));
    assert_eq!(format!("{g:?}"), before);
    g.act(0, Move::WithDivision(vec![vec![1, 2]], Box::new(Move::ChooseTargets(targets)))).unwrap();
    assert_eq!(g.obj(g.stack[0]).division, vec![vec![1, 2]]);
    resolve(&mut g);
    cite!(["601.2d", "608.2c"]; assert_eq!((g.players[0].life, g.players[1].life), (19, 18)));
}
