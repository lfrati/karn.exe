//! A token that is a copy of "that card", "that creature" or "it": the object the anaphor names.
//!
//! Séance: "At the beginning of each upkeep, you may exile target creature card from your
//! graveyard. If you do, create a token that's a copy of that card, except it's a Spirit in
//! addition to its other types. Exile it at the beginning of the next end step." The copy is
//! of the exiled card (the anaphor "that card"), with the exception applied (707.9b), and
//! the delayed trigger exiles the token, not the card (603.7).
use crate::support::{cite, paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::{CardType, Part};

const SEANCE: usize = 0;
const BEAR: usize = 1;
const ISLAND: usize = 2;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let seance = CardDef::from_face(faces.iter().find(|f| f.name == "Séance").unwrap()).unwrap();
    let defs = vec![
        seance,
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..6 { g.put(p, ISLAND, Zone::Library); } }
    g
}
/// Play on to `until`, answering Séance's target with `target` and its "may" with `may`.
fn play_to(g: &mut Game, until: Part, target: Option<ObjId>, may: bool) {
    for _ in 0..200 {
        match g.decision.clone() {
            Some(Decision::Targets { player, .. }) => {
                let t = target.map(|t| vec![Target::Object(t)]).unwrap_or_default();
                g.act(player, Move::ChooseTargets(vec![t])).unwrap();
            }
            Some(Decision::Choose { player, choice: Choice::May { possible: true } }) =>
                g.act(player, Move::Choose(if may { Answer::Yes } else { Answer::No })).unwrap(),
            Some(d) => panic!("{d:?}"),
            None if g.part == until && g.stack.is_empty() => return,
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("never reached {until:?}");
}
/// From the first main phase, on to the next turn's: its upkeep is the next upkeep.
fn next_upkeep(g: &mut Game, target: Option<ObjId>, may: bool) {
    play_to(g, Part::End, None, may);
    play_to(g, Part::PrecombatMain, target, may);
}
/// The new object a card became in exile (400.7).
fn exiled(g: &Game, card: ObjId) -> Option<ObjId> {
    g.log.iter().find_map(|e| match e {
        Event::ZoneChanged { was, now, to: Zone::Exile, .. } if *was == card => *now,
        _ => None,
    })
}
fn token(g: &Game) -> Option<ObjId> {
    g.log.iter().find_map(|e| if let Event::Created { token } = e { Some(*token) } else { None })
}

#[test]
fn seance_copies_the_exiled_card_as_a_spirit_until_the_next_end_step() {
    let mut g = game();
    g.put(0, SEANCE, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Graveyard);
    next_upkeep(&mut g, Some(bear), true);
    let card = exiled(&g, bear).expect("the card was exiled");
    let copy = token(&g).expect("a token copy");
    assert_eq!(g.obj(copy).zone, Zone::Battlefield);
    let seen = g.seen(copy, None);
    assert!(seen.is(CardType::Creature));
    assert_eq!(seen.pt, Some((2, 2)));
    let subtypes: Vec<_> = seen.subtypes.iter().map(|s| s.name()).collect();
    cite!(["707.9b"]; assert!(subtypes.contains(&"Bear") && subtypes.contains(&"Spirit"), "{subtypes:?}"));
    // the delayed trigger exiles the token; the card stays in exile
    play_to(&mut g, Part::End, None, true);
    cite!(["603.7c"]; assert_ne!(g.obj(copy).zone, Zone::Battlefield, "exiled at the next end step"));
    assert_eq!(g.obj(card).zone, Zone::Exile);
}

#[test]
fn seance_declined_does_nothing() {
    let mut g = game();
    g.put(0, SEANCE, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Graveyard);
    next_upkeep(&mut g, Some(bear), false);
    assert_eq!(g.obj(bear).zone, Zone::Graveyard);
    cite!(["118.12"]; assert!(token(&g).is_none()));
}

#[test]
fn seance_only_targets_a_creature_card_in_your_graveyard() {
    let mut g = game();
    g.put(0, SEANCE, Zone::Battlefield);
    let theirs = g.put(1, BEAR, Zone::Graveyard);
    let land = g.put(0, ISLAND, Zone::Graveyard);
    let bear = g.put(0, BEAR, Zone::Graveyard);
    for _ in 0..50 {
        if g.decision.is_some() { break; }
        // it triggers in the next upkeep: this game begins in the first main phase
        g.act(g.priority, Move::Pass).unwrap();
    }
    let Some(Decision::Targets { player: 0, .. }) = g.decision.clone() else { panic!("{:?}", g.decision) };
    for (wrong, why) in [(theirs, "an illegal target, Bear: it is in an opponent's graveyard, not yours (115.1)"), (land, "an illegal target, Island: it isn't a creature (115.1)")] {
        cite!(["115.1"]; case: why; refused(why): g.act(0, Move::ChooseTargets(vec![vec![Target::Object(wrong)]])));
    }
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(bear)]])).unwrap();
}

/// Supplant Form: "Return target creature to its owner's hand. You create a token that's a copy
/// of that creature." The copy is of the creature as it last existed on the battlefield
/// (608.2h): a Clone that copied a Bear makes a Bear, not a 0/0 Clone.
#[test]
fn supplant_form_copies_the_creature_as_it_last_existed() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let supplant = CardDef::from_face(faces.iter().find(|f| f.name == "Supplant Form").unwrap()).unwrap();
    let defs = vec![
        supplant,
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Clone", "{3}{U}", "Creature — Shapeshifter", "You may have this creature enter as a copy of any creature on the battlefield.", Some((0, 0))).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    let bear = g.put(1, 1, Zone::Battlefield);
    let clone = g.put(0, 2, Zone::Hand);
    g.players[0].pool.colorless = 3;
    g.players[0].pool.colors[1] = 1;
    g.act(0, paid("CCCU", Move::Cast { card: clone, targets: vec![] })).unwrap();
    for _ in 0..20 {
        match g.decision.clone() {
            Some(Decision::Choose { player, choice: Choice::May { possible: true } }) => g.act(player, Move::Choose(Answer::Yes)).unwrap(),
            Some(Decision::Choose { player, .. }) => g.act(player, Move::Choose(Answer::Objects(vec![bear]))).unwrap(),
            Some(d) => panic!("{d:?}"),
            None if g.stack.is_empty() => break,
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    let copy = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(2)).expect("the clone entered");
    assert_eq!(g.seen(copy, None).pt, Some((2, 2)), "it copied the bear");
    let spell = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colorless = 4;
    g.players[0].pool.colors[1] = 2;
    g.act(0, paid("CCCCUU", Move::Cast { card: spell, targets: vec![vec![Target::Object(copy)]] })).unwrap();
    for _ in 0..4 { if g.stack.is_empty() { break; } g.act(g.priority, Move::Pass).unwrap(); }
    let token = token(&g).expect("a token");
    let seen = g.seen(token, None);
    cite!(["608.2h", "707.2"]; assert_eq!(seen.pt, Some((2, 2))));
    cite!(["608.2h", "707.2"]; assert!(seen.subtypes.iter().any(|s| s.name() == "Bear")));
}
