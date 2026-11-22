//! "It" where the text has named both its own source and something else (608.2k). The
//! antecedent is the one the reference's own sentence named, else the one named later; an
//! act that names both names the source as what acts, and "it" afterwards is the other.
//! Neither the source nor what the text named is preferred as such, and the answer does not
//! depend on which branch is taken.

use crate::support::cite;
use game::card::CardDef;
use game::play::check;
use game::state::{Answer, Choice, Decision};
use game::{Event, Game, Move, ObjId, Target, Zone};

const BEARS: usize = 0;
const ROCK: usize = 1;
const CARD: usize = 2;

const DEMON: &str = "Flying\nFirst strike\nAt the beginning of your upkeep, you may sacrifice an artifact. If you don't, tap this creature and it deals 2 damage to you.";
const ARCHDEMON: &str = "Flying, trample\nAt the beginning of your upkeep, sacrifice a Human. If you can't, tap this creature and it deals 9 damage to you.";

/// Grizzly Bears, an artifact, then the creature under test; libraries of Bears.
fn game(text: &str) -> Game {
    let card = |name: &str, types, text, pt| {
        let def = CardDef::new(name, "{0}", types, text, pt).expect(name);
        check(&def).unwrap_or_else(|e| panic!("{name}: {}", e.0));
        def
    };
    let defs = vec![card("Grizzly Bears", "Creature — Bear", "", Some((2, 2))), card("Rock", "Artifact", "", None), card("Tested", "Creature — Demon", text, Some((6, 6)))];
    let mut g = Game::new(defs, 2, 20);
    for player in 0..2 {
        for _ in 0..10 {
            g.put(player, BEARS, Zone::Library);
        }
    }
    g
}

/// Answers what is asked, sacrificing where `accept`, until nothing is asked and the stack
/// is empty.
fn settle(g: &mut Game, accept: bool) {
    for _ in 0..40 {
        match g.decision.clone() {
            Some(Decision::Choose { choice: Choice::May { possible: true }, .. }) => g.act(0, Move::Choose(if accept { Answer::Yes } else { Answer::No })).unwrap(),
            Some(Decision::Choose { choice: Choice::Objects { among, .. }, .. }) => g.act(0, Move::Choose(Answer::Objects(vec![among[0]]))).unwrap(),
            Some(other) => panic!("not asked here: {other:?}"),
            None if g.stack.is_empty() => return,
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("the stack does not empty");
}

/// The first activated ability of the creature under test, on the battlefield beside an
/// artifact where `rock`.
fn activated(text: &str, rock: bool, accept: bool, targets: impl Fn(ObjId, ObjId) -> Vec<Vec<Target>>) -> (Game, ObjId, ObjId) {
    let mut g = game(text);
    let source = g.put(0, CARD, Zone::Battlefield);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    if rock {
        g.put(0, ROCK, Zone::Battlefield);
    }
    g.act(0, Move::Activate { source, ability: 0, targets: targets(source, bears), paying: vec![] }).unwrap();
    settle(&mut g, accept);
    (g, source, bears)
}

/// The upkeep trigger of the creature under test, on the second turn of its controller.
fn upkeep(text: &str, rock: bool, accept: bool) -> (Game, ObjId) {
    let mut g = game(text);
    let source = g.put(0, CARD, Zone::Battlefield);
    if rock {
        g.put(0, ROCK, Zone::Battlefield);
    }
    let from = g.log.len();
    for _ in 0..200 {
        if g.log[from..].iter().any(|e| matches!(e, Event::Began { part: ir::Part::Upkeep })) && g.active == 0 && (g.decision.is_some() || !g.stack.is_empty()) {
            settle(&mut g, accept);
            return (g, source);
        }
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(Decision::Discard { player, count }) => {
                let cards = g.players[player].hand[..count].to_vec();
                g.act(player, Move::Discard(cards)).unwrap()
            }
            Some(other) => panic!("not asked here: {other:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("no upkeep of the controller came");
}

/// What dealt damage, to whom and how much, in the order it was dealt.
fn damage(g: &Game) -> Vec<(ObjId, Target, u32)> {
    g.log.iter().filter_map(|e| match e { Event::DamageDealt { source, to, amount, .. } => Some((*source, *to, *amount)), _ => None }).collect()
}

fn rocks(g: &Game) -> usize {
    g.battlefield.iter().filter(|o| g.obj(**o).card == Some(ROCK)).count()
}

#[test]
fn r608_2k_it_is_the_creature_tapped_in_its_own_sentence_when_nothing_was_sacrificed() {
    // Yawgmoth Demon: no artifact to sacrifice, and an artifact kept
    for rock in [false, true] {
        let (g, demon) = upkeep(DEMON, rock, false);
        cite!(["608.2c"]; case: format!("rock={rock}"); assert!(g.obj(demon).tapped, "artifact {rock}"));
        cite!(["608.2c"]; case: format!("rock={rock}"); assert_eq!(damage(&g), [(demon, Target::Player(0), 2)], "artifact {rock}"));
        cite!(["120.3a"]; case: format!("rock={rock}"); assert_eq!(g.players[0].life, 18));
        assert_eq!(rocks(&g), usize::from(rock));
    }
}

#[test]
fn r608_2k_a_sacrifice_made_leaves_the_other_branch_undone() {
    let (g, demon) = upkeep(DEMON, true, true);
    cite!(["608.2c"]; assert!(!g.obj(demon).tapped));
    cite!(["608.2c"]; assert_eq!(damage(&g), []));
    assert_eq!(g.players[0].life, 20);
    cite!(["701.21a"]; assert_eq!(rocks(&g), 0));
}

#[test]
fn r608_2k_it_is_that_creature_after_a_sacrifice_that_could_not_be_made() {
    // Archdemon of Greed: the sacrifice is not optional and there is no Human
    let (g, demon) = upkeep(ARCHDEMON, false, false);
    cite!(["608.2c"]; assert!(g.obj(demon).tapped));
    cite!(["608.2c"]; assert_eq!(damage(&g), [(demon, Target::Player(0), 9)]));
    cite!(["120.3a"]; assert_eq!(g.players[0].life, 11));
}

#[test]
fn r608_2k_it_is_that_creature_also_where_the_sacrifice_was_made() {
    // the same sentence after "If you do": the sacrificed artifact exists and is not "it"
    let text = "{0}: You may sacrifice an artifact. If you do, tap this creature and it deals 2 damage to you.";
    let (g, source, _) = activated(text, true, true, |_, _| vec![]);
    cite!(["608.2c"]; assert!(g.obj(source).tapped));
    cite!(["608.2c"]; assert_eq!(damage(&g), [(source, Target::Player(0), 2)]));
    cite!(["701.21a"]; assert_eq!(rocks(&g), 0));

    let (g, source, _) = activated(text, true, false, |_, _| vec![]);
    cite!(["608.2c"]; assert!(!g.obj(source).tapped));
    cite!(["608.2c"]; assert_eq!(damage(&g), []));
    assert_eq!(rocks(&g), 1);
}

#[test]
fn r608_2k_the_source_named_after_a_sacrifice_in_an_earlier_sentence_is_it() {
    let text = "{0}: Sacrifice an artifact. Tap this creature. It deals 2 damage to you.";
    let (g, source, _) = activated(text, true, true, |_, _| vec![]);
    cite!(["608.2c"]; assert_eq!(damage(&g), [(source, Target::Player(0), 2)]));
    cite!(["701.21a"]; assert_eq!(rocks(&g), 0));
}

#[test]
fn r608_2k_something_named_after_the_source_is_it() {
    // the target is named in a later sentence than "this creature"
    let text = "{0}: Tap this creature. Target creature gets +2/+0 until end of turn. It deals 1 damage to you.";
    let (g, source, bears) = activated(text, false, true, |_, bears| vec![vec![Target::Object(bears)]]);
    cite!(["701.26a"]; assert!(g.obj(source).tapped));
    cite!(["608.2c"]; assert_eq!(damage(&g), [(bears, Target::Player(0), 1)]));
}

#[test]
fn r608_2k_what_the_source_acted_on_is_it_in_the_next_sentence() {
    // one act names both: the source as what deals the damage, the target as what takes it
    let text = "{0}: This creature deals 1 damage to target creature. It gets +2/+0 until end of turn.";
    let (g, source, bears) = activated(text, false, true, |_, bears| vec![vec![Target::Object(bears)]]);
    cite!(["608.2c"]; assert_eq!(damage(&g), [(source, Target::Object(bears), 1)]));
    cite!(["608.2c", "613.4c"]; assert_eq!(g.seen(bears, None).pt, Some((4, 2))));
    cite!(["608.2c"]; assert_eq!(g.seen(source, None).pt, Some((6, 6))));
}
