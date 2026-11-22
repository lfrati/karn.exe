//! Soulshift is a targeted optional return, including noncreature Spirit cards (702.46a).
use crate::support::{cite, oracle};
use game::{Game, Move, Target, Zone};
use game::state::{Answer, Choice, Decision};

fn pass(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }
fn fixture() -> (Game, game::ObjId, game::ObjId) {
    let mut g = Game::new(vec![oracle("Venerable Kumo"), oracle("Crib Swap"), oracle("Grizzly Bears")], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let spirit = g.put(0, 1, Zone::Graveyard);
    (g, source, spirit)
}
fn die(g: &mut Game, source: game::ObjId) {
    assert!(g.destroy(source));
    g.act(g.priority, Move::Pass).unwrap();
    assert!(matches!(g.decision, Some(Decision::Targets { player: 0, .. })));
}
#[test]
fn r702_46a_targets_only_own_spirits_within_mana_value_then_offers_the_return() {
    let (mut g, source, spirit) = fixture();
    let nonspirit = g.put(0, 2, Zone::Graveyard);
    let opponent = g.put(1, 1, Zone::Graveyard);
    die(&mut g, source);
    let too_large = *g.players[0].graveyard.last().unwrap();
    for wrong in [nonspirit, opponent, too_large] {
        cite!(["702.46a", "115.1d"]; refused("an illegal target"): g.act(0, Move::ChooseTargets(vec![vec![Target::Object(wrong)]])));
    }
    cite!(["702.46a", "702.73a"]; ok: g.act(0, Move::ChooseTargets(vec![vec![Target::Object(spirit)]])));
    pass(&mut g);
    assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May { possible: true } }));
    for (mut branch, yes) in [(g.clone(), true), (g, false)] {
        branch.act(0, Move::Choose(if yes { Answer::Yes } else { Answer::No })).unwrap();
        cite!(["702.46a"]; assert_eq!(branch.players[0].hand.len(), usize::from(yes)));
        if yes { assert_eq!(branch.obj(branch.players[0].hand[0]).card, Some(1)); }
    }
}
#[test]
fn r702_46a_a_returned_graveyard_incarnation_is_no_longer_the_target() {
    let (mut g, source, spirit) = fixture();
    die(&mut g, source);
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(spirit)]])).unwrap();
    g.exile(spirit);
    let exile = g.exile[0];
    g.move_object(exile, Zone::Graveyard);
    pass(&mut g);
    cite!(["400.7", "608.2b"]; assert!(g.players[0].hand.is_empty() && g.stack.is_empty() && g.decision.is_none()));
}
#[test]
fn r702_46b_forked_branch_garami_has_two_independent_soulshift_triggers() {
    let mut g = Game::new(vec![oracle("Forked-Branch Garami"), oracle("Crib Swap")], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let spirit = g.put(0, 1, Zone::Graveyard);
    g.destroy(source); g.act(g.priority, Move::Pass).unwrap();
    assert!(matches!(g.decision, Some(Decision::Order { player: 0, .. })));
    g.act(0, Move::Order(vec![0, 1])).unwrap();
    for _ in 0..2 { g.act(0, Move::ChooseTargets(vec![vec![Target::Object(spirit)]])).unwrap(); }
    cite!(["702.46b"]; assert_eq!(g.stack.len(), 2));
    pass(&mut g); g.act(0, Move::Choose(Answer::Yes)).unwrap();
    pass(&mut g);
    cite!(["702.46b", "608.2b"]; assert_eq!(g.players[0].hand.len(), 1));
    assert!(g.stack.is_empty());
}
