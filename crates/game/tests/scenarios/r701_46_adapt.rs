use crate::support::cite;
use game::{CardDef, Game, Move, Zone};
use game::state::{Answer, Choice, Decision};
fn card(name: &str, kind: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    let d = CardDef::new(name, "{0}", kind, text, pt).unwrap();
    game::play::check(&d).unwrap(); d
}
fn setup() -> Game {
    let mut g = Game::new(vec![
        card("Adapter", "Creature", "{0}: Adapt 2. You gain 1 life.", Some((2,2))),
        card("Scale", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.", None),
        card("Double", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.", None),
    ], 2, 20);
    g.put(0, 0, Zone::Battlefield); g
}
fn activate(g: &mut Game) {
    let source=g.battlefield[0];
    g.act(0, Move::Activate { source, ability:0, targets:vec![], paying:vec![] }).unwrap();
}
fn resolve(g: &mut Game) { for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();} }
#[test]
fn r701_46a_adapt_checks_on_resolution_and_skips_only_its_counter_instruction() {
    let mut g=setup();let source=g.battlefield[0];activate(&mut g);resolve(&mut g);
    cite!(["701.46a"]; assert_eq!(g.obj(source).counters.get("+1/+1"),Some(&2)));assert_eq!(g.players[0].life,21);
    activate(&mut g);resolve(&mut g);
    cite!(["701.46a"]; assert_eq!(g.obj(source).counters.get("+1/+1"),Some(&2)));cite!(["608.2c"]; assert_eq!(g.players[0].life,22));
    activate(&mut g);g.remove_counters(source,"+1/+1",99);resolve(&mut g);
    cite!(["701.46a"]; assert_eq!(g.obj(source).counters.get("+1/+1"),Some(&2)));
}
#[test]
fn r701_46a_counters_added_in_response_stop_adapt_but_other_counter_kinds_do_not() {
    let mut g=setup();let source=g.battlefield[0];activate(&mut g);
    g.put_counters(source,"+1/+1",1);resolve(&mut g);
    cite!(["701.46a"]; assert_eq!(g.obj(source).counters.get("+1/+1"),Some(&1)));
    g.remove_counters(source,"+1/+1",99);g.put_counters(source,"charge",1);
    activate(&mut g);resolve(&mut g);cite!(["701.46a"]; assert_eq!(g.obj(source).counters.get("+1/+1"),Some(&2)));
}
#[test]
fn r701_46a_departed_source_does_not_move_counters_to_its_new_incarnation() {
    let mut g=setup();let source=g.battlefield[0];activate(&mut g);
    g.move_object(source,Zone::Graveyard);resolve(&mut g);
    cite!(["400.7"]; assert!(g.obj(g.players[0].graveyard[0]).counters.is_empty()));assert_eq!(g.players[0].life,21);
}
#[test]
fn r616_1_adapt_uses_the_shared_replacement_question_and_resumes_clones() {
    let mut g=setup();let source=g.battlefield[0];g.put(0,1,Zone::Battlefield);g.put(0,2,Zone::Battlefield);
    activate(&mut g);resolve(&mut g);
    cite!(["616.1"]; assert!(matches!(g.decision,Some(Decision::Choose {choice:Choice::Replacement {..},..}))));
    assert_eq!(g.players[0].life,20);crate::support::refused(g.act(0,Move::Pass), "a decision is pending: a choice of the effect being applied (608.2d)");
    let mut clone=g.clone();
    for branch in [&mut g,&mut clone] {
        while let Some(Decision::Choose {player,choice:Choice::Replacement {among,..}})=branch.decision.clone() {
            branch.act(player,Move::Choose(Answer::Replacement(among[0].clone()))).unwrap();
        }
        cite!(["616.1", "616.1e"]; assert_eq!(branch.obj(source).counters.get("+1/+1"),Some(&6)));assert_eq!(branch.players[0].life,21);
    }
}
