//! Batched distributive dealers. The member and refusal cases are in
//! distributive_recheck_review and distributive_runtime_review. A distributive
//! dealer with a controller-proven earlier-target "other" is refused, and plain plural object
//! subjects are distributive. Run with `engine::distributive_batch_review::`.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};

fn creature(name: &str, p: i32, t: i32) -> CardDef { CardDef::new(name, "{0}", "Creature — Beast", "", Some((p, t))).unwrap() }
fn spell(text: &str) -> CardDef { CardDef::new("Probe", "{0}", "Sorcery", text, None).unwrap() }
fn admitted(text: &str) -> Result<(), String> { game::play::check(&spell(text)).map_err(|e| e.0) }

/// Mine: Big 3/5 and Small 2/9. Theirs: Wall 1/20. Cast without targets.
fn run(text: &str) -> (Vec<(ObjId, Target, u32)>, ObjId, ObjId, ObjId, Vec<i32>) {
    let mut g = Game::new(vec![spell(text), creature("Big", 3, 5), creature("Small", 2, 9), creature("Wall", 1, 20)], 2, 20);
    let a = g.put(0, 1, Zone::Battlefield);
    let b = g.put(0, 2, Zone::Battlefield);
    let w = g.put(1, 3, Zone::Battlefield);
    let c = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: c, targets: vec![] }).unwrap();
    for _ in 0..3 { if g.stack.is_empty() { break; } g.act(g.priority, Move::Pass).unwrap(); }
    let mut dealt: Vec<_> = g.log.iter().filter_map(|e| match e {
        Event::DamageDealt { source, to, amount, .. } => Some((*source, *to, *amount)),
        _ => None,
    }).collect();
    dealt.sort_by_key(|(s, t, _)| (*s, format!("{t:?}")));
    (dealt, a, b, w, g.players.iter().map(|p| p.life).collect())
}

/// The singular form: one named actor with a proven target keeps one exclusion.
#[test]
fn singular_actor_with_proven_target_is_unchanged() {
    admitted("Target creature an opponent controls deals 1 damage to each other creature that player controls.").unwrap();
    admitted("Tap target creature an opponent controls. Each other creature that player controls gets -2/-0 until end of turn.").unwrap();
}

/// Fixed and counted quantities from a plain plural subject keep their values, per dealer.
#[test]
fn plain_plural_fixed_and_counted_quantities() {
    for (text, each) in [("Creatures you control deal 1 damage to each creature your opponents control.", 1),
                         ("Creatures you control deal damage equal to the number of creatures you control to each creature your opponents control.", 2)] {
        admitted(text).unwrap();
        let (dealt, a, b, w, _) = run(text);
        cite!(["120.1", "608.2h"]; case: text; assert_eq!(dealt, vec![(a, Target::Object(w), each), (b, Target::Object(w), each)], "{text}"));
    }
    let text = "Creatures you control deal 1 damage to each opponent.";
    admitted(text).unwrap();
    let (dealt, a, b, _, life) = run(text);
    cite!(["120.1", "608.2h"]; assert_eq!(dealt, vec![(a, Target::Player(1), 1), (b, Target::Player(1), 1)]));
    cite!(["120.3a"]; assert_eq!(life, vec![20, 18]));
}
