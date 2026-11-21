//! Counter cancellation (704.5q) beside the deaths of its batch, with full pinned undying and
//! persist cards. A permanent that dies in a state-based batch keeps, as last known
//! information, the counters it had before any of that batch's actions (704.8, its Young Wolf
//! example); one that stays is left with N fewer of each kind (704.5q). Cancelling before
//! the departures would make the dying permanents return; the counters of those that
//! stay cancel in the same batch.
use crate::support::{cite, paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};

const WOLF: usize = 0;
const FINKS: usize = 1;
const COURAGE: usize = 2;
const BOLT: usize = 3;
const REGEN: usize = 4;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let mut defs: Vec<CardDef> = ["Young Wolf", "Kitchen Finks", "Courage in Crisis", "Lightning Bolt"]
        .iter()
        .map(|n| CardDef::from_faces(catalog.get(n).unwrap_or_else(|| panic!("{n}"))).unwrap())
        .collect();
    defs.push(CardDef::new("Regen", "{0}", "Creature — Ape", "", Some((2, 2))).unwrap());
    for d in &defs {
        game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0));
    }
    let mut g = Game::new(defs, 2, 20);
    g.players[0].pool.colors = [20; 5];
    g
}
/// Resolve everything: triggers in listed order, every "may" yes, no proliferate holder.
fn settle(g: &mut Game) {
    g.state_based_actions();
    for _ in 0..40 {
        match g.decision.clone() {
            Some(Decision::Order { player, abilities }) => g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(),
            Some(Decision::Choose { player, choice: Choice::Holders { .. } }) => g.act(player, Move::Choose(Answer::Holders(vec![]))).unwrap(),
            Some(Decision::Choose { player, .. }) => g.act(player, Move::Choose(Answer::Yes)).unwrap(),
            Some(d) => panic!("{d:?}"),
            None if g.stack.is_empty() => return,
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("the stack did not settle")
}
fn cast(g: &mut Game, def: usize, targets: Vec<Vec<Target>>, units: &str) {
    let card = g.put(0, def, Zone::Hand);
    g.act(0, paid(units, Move::Cast { card, targets })).unwrap();
    settle(g);
}
fn on_battlefield(g: &Game, def: usize) -> Vec<ObjId> {
    g.battlefield.iter().copied().filter(|o| g.obj(*o).card == Some(def)).collect()
}
fn counts(g: &Game, o: ObjId) -> (u32, u32) {
    let c = &g.obj(o).counters;
    (c.get("+1/+1").copied().unwrap_or(0), c.get("-1/-1").copied().unwrap_or(0))
}

/// 704.8's own example: one +1/+1 and three -1/-1, so it dies, and its last known
/// information has the +1/+1 counter, so undying does not trigger.
#[test]
fn r704_8_young_wolf_dying_with_both_kinds_keeps_its_plus_counter_and_does_not_return() {
    let mut g = game();
    let wolf = g.put(0, WOLF, Zone::Battlefield);
    g.put_counters(wolf, "+1/+1", 1);
    g.put_counters(wolf, "-1/-1", 3);
    settle(&mut g);
    cite!(["704.5f"]; assert_eq!(g.obj(wolf).zone, Zone::Gone));
    cite!(["704.8"]; assert_eq!(counts(&g, wolf), (1, 3), "last known counters are from before the batch"));
    cite!(["704.8", "702.93a"]; assert!(on_battlefield(&g, WOLF).is_empty(), "undying returned it"));
}

/// Destroyed by lethal damage in the batch that would also cancel its counters: it had a
/// -1/-1 counter when it was last on the battlefield, so persist does not trigger.
#[test]
fn r704_8_kitchen_finks_destroyed_with_both_kinds_does_not_persist() {
    let mut g = game();
    let finks = g.put(0, FINKS, Zone::Battlefield);
    g.put_counters(finks, "+1/+1", 1);
    g.put_counters(finks, "-1/-1", 1);
    g.damage(finks, Target::Object(finks), 2);
    settle(&mut g);
    cite!(["704.5g", "704.8"]; assert_eq!((g.obj(finks).zone, counts(&g, finks)), (Zone::Gone, (1, 1))));
    cite!(["704.8", "702.79a"]; assert!(on_battlefield(&g, FINKS).is_empty(), "persist returned it"));
}

/// Persisted, then given a +1/+1 counter: the two cancel (704.5q), so it can persist again.
/// Were both counters to stay, the second death would not persist.
#[test]
fn r704_5q_a_persisted_kitchen_finks_given_a_plus_counter_persists_again() {
    let mut g = game();
    let finks = g.put(0, FINKS, Zone::Battlefield);
    cast(&mut g, BOLT, vec![vec![Target::Object(finks)]], "R");
    let [back] = on_battlefield(&g, FINKS)[..] else { panic!("persist") };
    cite!(["702.79a"]; assert_eq!(counts(&g, back), (0, 1)));
    cast(&mut g, COURAGE, vec![vec![Target::Object(back)]], "GGG");
    cite!(["704.5q"]; assert_eq!(counts(&g, back), (0, 0), "704.5q"));
    assert_eq!(g.seen(back, None).pt, Some((3, 2)));
    cast(&mut g, BOLT, vec![vec![Target::Object(back)]], "R");
    cite!(["704.5q", "702.79a"]; assert_eq!(on_battlefield(&g, FINKS).len(), 1, "it had no -1/-1 counter, so it persists"));
}

/// A regeneration survivor of the same batch is a retained battlefield incarnation: it is
/// regenerated and its counters cancel, as one batch.
#[test]
fn r704_5q_a_regenerated_creature_also_cancels_in_the_same_batch() {
    let mut g = game();
    let ape = g.put(0, REGEN, Zone::Battlefield);
    g.put_counters(ape, "+1/+1", 2);
    g.put_counters(ape, "-1/-1", 1);
    g.regenerate(ape);
    g.damage(ape, Target::Object(ape), 3);
    let from = g.log.iter().count();
    assert!(g.state_based_actions());
    cite!(["701.19a"]; assert_eq!(g.obj(ape).zone, Zone::Battlefield));
    cite!(["701.19a"]; assert!(g.obj(ape).tapped && g.obj(ape).damage == 0));
    cite!(["704.5q", "704.3"]; assert_eq!(counts(&g, ape), (1, 0)));
    let events: Vec<_> = g.log.iter().skip(from).filter(|e| matches!(e, Event::Regenerated { .. } | Event::CountersRemoved { .. })).collect();
    assert_eq!(events.len(), 3, "{events:?}");
}

/// One batch: the Wolf dies with both kinds (no undying), the persisted Finks survives and
/// cancels. Only survivors are cancelled, and the Wolf's last known counters are untouched.
#[test]
fn r704_5q_survivors_cancel_while_a_dying_permanent_keeps_both_kinds() {
    let mut g = game();
    let wolf = g.put(0, WOLF, Zone::Battlefield);
    let finks = g.put(0, FINKS, Zone::Battlefield);
    g.put_counters(wolf, "+1/+1", 1);
    g.put_counters(wolf, "-1/-1", 3);
    g.put_counters(finks, "+1/+1", 2);
    g.put_counters(finks, "-1/-1", 1);
    settle(&mut g);
    cite!(["704.8"]; assert_eq!((g.obj(wolf).zone, counts(&g, wolf)), (Zone::Gone, (1, 3))));
    cite!(["704.8", "702.93a"]; assert!(on_battlefield(&g, WOLF).is_empty()));
    cite!(["704.5q"]; assert_eq!((g.obj(finks).zone, counts(&g, finks)), (Zone::Battlefield, (1, 0))));
}
