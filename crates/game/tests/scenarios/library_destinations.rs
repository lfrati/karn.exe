//! Resolution destination choices retain the owner, objects and continuation.
//! CR 111.4/111.10f: an unnamed predefined Clue has the name "Clue Token".
use crate::support::{cite, oracle, paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::action::LibraryPosition;

fn setup(name: &str, players: usize) -> Game {
    let defs = vec![oracle(name), oracle("Grizzly Bears"), oracle("Island"),
        CardDef::new("Restore", "{0}", "Instant", "Put target creature card from a graveyard onto the battlefield under your control.", None).unwrap(),
        oracle("Shore Up")];
    let mut g = Game::new(defs, players, 20);
    g.part = ir::Part::PrecombatMain;
    g
}
fn cast(g: &mut Game, player: usize, def: usize, target: ObjId, spending: &str) {
    g.players[player].pool.colors = [9; 5];
    let card = g.put(player, def, Zone::Hand);
    g.act(player, paid(spending, Move::Cast { card, targets: vec![vec![Target::Object(target)]] })).unwrap();
}
fn resolve(g: &mut Game) {
    for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); }
}
fn names(g: &Game, player: usize) -> Vec<String> {
    g.players[player].library.iter().rev().map(|o| g.obj(*o).chars.name.clone()).collect()
}
fn destination(g: &Game, player: usize, card: ObjId, positions: &[LibraryPosition]) {
    // Uncited (protocol): the engine exposes the semantic destination and correct chooser.
    assert!(matches!(&g.decision, Some(Decision::Choose { player: chooser,
        choice: Choice::LibraryDestination { cards, positions: offered } })
        if *chooser == player && cards == &[card] && offered == positions), "{:?}", g.decision);
}

// Rules: 400.3, 400.7, 401.7, 608.2c, 608.2d
#[test]
fn r401_7_native_destinations_cover_both_alternatives_and_short_libraries() {
    for (name, first, spending) in [("Misleading Motes", LibraryPosition::Top, "UUUU"),
        ("Temporal Cleansing", LibraryPosition::FromTop(2), "UUUU"),
        ("Lost Days", LibraryPosition::FromTop(2), "UUUUU"), ("Vanish from Sight", LibraryPosition::Top, "UUUU")] {
        for count in [0, 1, 3] {
            for position in [first, LibraryPosition::Bottom] {
                let mut g = setup(name, 2);
                for _ in 0..count { g.put(1, 2, Zone::Library); }
                let bear = g.put(1, 1, Zone::Battlefield);
                if name == "Temporal Cleansing" && count == 3 && position == LibraryPosition::FromTop(2) {
                    // CR 702.51a: this green creature pays one generic mana, even
                    // without haste; the pool can pay only the remaining {2}{U}.
                    let convoker = g.put(0, 1, Zone::Battlefield);
                    let spell = g.put(0, 0, Zone::Hand);
                    g.players[0].pool.colors[ir::Color::Blue as usize] = 1;
                    g.players[0].pool.colorless = 2;
                    cite!(["702.51a"]; ok: g.act(0, Move::WithConvoke(vec![None], Box::new(Move::WithCosts(
                        vec![vec![convoker]], Box::new(paid("CCU", Move::Cast { card: spell, targets: vec![vec![Target::Object(bear)]] })))))));
                    cite!(["702.51a"]; assert!(g.obj(convoker).tapped));
                } else {
                    cast(&mut g, 0, 0, bear, spending);
                }
                resolve(&mut g);
                destination(&g, 1, bear, &[first, LibraryPosition::Bottom]);
                cite!(["608.2d"]; assert_eq!(g.obj(bear).zone, Zone::Battlefield));
                cite!(["608.2c"]; assert_eq!(g.stack.len(), 1));
                cite!(["608.2d"]; ok: g.act(1, Move::Choose(Answer::LibraryPosition(position))));
                let index = match position { LibraryPosition::Top => 0, LibraryPosition::Bottom => count,
                    LibraryPosition::FromTop(n) => ((n - 1) as usize).min(count) };
                let mut expected = vec!["Island".to_owned(); count]; expected.insert(index, "Grizzly Bears".into());
                cite!(["400.3", "401.7"]; assert_eq!(names(&g, 1), expected));
                cite!(["400.7"]; assert_eq!(g.obj(bear).zone, Zone::Gone));
                let moved = g.players[1].library.iter().find(|o| g.obj(**o).chars.name == "Grizzly Bears").unwrap();
                cite!(["400.7"]; assert_ne!(*moved, bear));
                cite!(["401.2"]; assert!((0..2).all(|viewer| !g.sees(viewer, *moved))));
                cite!(["608.2c"]; assert!(g.stack.is_empty()));
                cite!(["608.2c"]; assert_eq!(g.battlefield.iter().filter(|o| g.obj(**o).chars.name == "Clue Token").count(), usize::from(name == "Lost Days")));
                cite!(["400.7"]; assert_eq!(g.log.iter().filter(|e| matches!(e, Event::ZoneChanged { was, .. } if *was == bear)).count(), 1));
            }
        }
    }
}

// Rules: 400.3, 608.2c, 608.2d
#[test]
fn r608_2d_owner_chooses_with_different_controller_and_caster_and_bad_answers_rollback() {
    let mut g = setup("Lost Days", 3);
    let old = g.put(1, 1, Zone::Graveyard);
    g.priority = 2;
    cast(&mut g, 2, 3, old, ""); resolve(&mut g);
    let bear = g.battlefield[0];
    cite!(["400.3"]; assert_eq!((g.obj(bear).owner, g.controller(bear)), (1, 2)));
    // Ordinary priority passes return priority to player 0 before casting.
    while g.priority != 0 { g.act(g.priority, Move::Pass).unwrap(); }
    cast(&mut g, 0, 0, bear, "UUUUU"); resolve(&mut g);
    destination(&g, 1, bear, &[LibraryPosition::FromTop(2), LibraryPosition::Bottom]);
    let before = format!("{g:?}");
    // Uncited (protocol): wrong-seat commands must not consume the retained choice.
    crate::support::refused(g.act(2, Move::Choose(Answer::LibraryPosition(LibraryPosition::Bottom))),"that decision is another player's");
    assert_eq!(format!("{g:?}"), before);
    cite!(["608.2d"]; refused("offered library positions"): g.act(1, Move::Choose(Answer::LibraryPosition(LibraryPosition::Top))));
    cite!(["608.2d"]; refused("offered library positions"): g.act(1, Move::Choose(Answer::Yes)));
    assert_eq!(format!("{g:?}"), before);
    cite!(["608.2c"]; assert!(!g.battlefield.iter().any(|o| g.obj(*o).chars.name == "Clue Token")));
    cite!(["608.2d"]; ok: g.act(1, Move::Choose(Answer::LibraryPosition(LibraryPosition::Bottom))));
    cite!(["400.3"]; assert_eq!(names(&g, 1), ["Grizzly Bears"]));
    cite!(["608.2c"]; assert_eq!(g.battlefield.iter().filter(|o| g.obj(**o).chars.name == "Clue Token" && g.controller(**o) == 0).count(), 1));
}

// Rules: 608.2b, 608.2c, 702.11b
#[test]
fn r608_2b_all_illegal_targets_prevent_destination_and_subsequent_effects() {
    for (name, spending) in [("Vanish from Sight", "UUUU"), ("Lost Days", "UUUUU")] {
        for leaves in [true, false] {
            let mut g = setup(name, 2);
            let bear = g.put(1, 1, Zone::Battlefield);
            let top = g.put(0, 2, Zone::Library);
            cast(&mut g, 0, 0, bear, spending);
            if leaves {
                // Explicit raw movement isolates target incarnation loss at resolution.
                g.move_object(bear, Zone::Hand);
            } else {
                g.act(0, Move::Pass).unwrap();
                cast(&mut g, 1, 4, bear, "U"); resolve(&mut g);
            }
            resolve(&mut g);
            cite!(["608.2b", "702.11b"]; assert!(g.decision.is_none()));
            cite!(["608.2b"]; assert!(g.stack.is_empty()));
            cite!(["608.2b"]; assert_eq!(g.players[0].library, [top]));
            cite!(["608.2b"]; assert!(!g.battlefield.iter().any(|o| g.obj(*o).chars.name == "Clue Token")));
        }
    }
}

// Rules: 608.2c, 608.2d, 701.25a, 400.7
#[test]
fn r608_2c_destination_then_caster_surveils_once_without_moving_the_target_twice() {
    let mut g = setup("Vanish from Sight", 2);
    let top = g.put(0, 2, Zone::Library);
    let bear = g.put(1, 1, Zone::Battlefield);
    cast(&mut g, 0, 0, bear, "UUUU"); resolve(&mut g);
    destination(&g, 1, bear, &[LibraryPosition::Top, LibraryPosition::Bottom]);
    cite!(["608.2d"]; ok: g.act(1, Move::Choose(Answer::LibraryPosition(LibraryPosition::Top))));
    cite!(["608.2c", "701.25a"]; assert!(matches!(&g.decision, Some(Decision::Choose { player: 0, choice: Choice::Arrange { verb: ir::Verb::Surveil, cards } }) if cards == &[top])));
    cite!(["400.7"]; assert_eq!(g.obj(bear).zone, Zone::Gone));
    cite!(["608.2c"]; assert_eq!(names(&g, 1), ["Grizzly Bears"]));
    cite!(["701.25a"]; ok: g.act(0, Move::Choose(Answer::Arrange { top: vec![], away: vec![top] })));
    cite!(["608.2c"]; assert!(g.decision.is_none() && g.stack.is_empty()));
    cite!(["701.25a"]; assert_eq!(g.players[0].graveyard.iter().filter(|o| g.obj(**o).chars.name == "Island").count(), 1));
    cite!(["400.7"]; assert_eq!(g.log.iter().filter(|e| matches!(e, Event::ZoneChanged { was, .. } if *was == bear)).count(), 1));
}

// Rules: 400.3, 401.4, 608.2d
#[test]
fn r401_4_one_specified_destination_precedes_private_owner_ordering_and_all_movement() {
    let def = CardDef::new("Group", "{0}", "Instant", "Put all creatures on your choice of the top or bottom of their owners' libraries.", None).unwrap();
    // Uncited (admission): this direct composition isolates shared movement, not a new native consumer.
    game::play::check(&def).unwrap();
    let mut g = Game::new(vec![def, oracle("Grizzly Bears"), oracle("Aegis Turtle")], 2, 20);
    let cards: Vec<_> = (0..2).map(|player| vec![g.put(player, 1, Zone::Battlefield), g.put(player, 2, Zone::Battlefield)]).collect();
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap(); resolve(&mut g);
    cite!(["608.2d"]; assert!(matches!(&g.decision, Some(Decision::Choose { player: 0,
        choice: Choice::LibraryDestination { cards, .. } }) if cards.len() == 4)));
    cite!(["608.2d"]; ok: g.act(0, Move::Choose(Answer::LibraryPosition(LibraryPosition::Top))));
    for player in 0..2 {
        cite!(["401.4"]; assert!(matches!(&g.decision, Some(Decision::Choose { player: chooser,
            choice: Choice::OrderCards { cards, .. } }) if *chooser == player && cards.len() == 2)));
        cite!(["401.4"]; assert_eq!(g.battlefield.len(), 4));
        cite!(["401.4"]; ok: g.act(player, Move::Choose(Answer::Objects(cards[player].clone()))));
    }
    for player in 0..2 {
        cite!(["400.3", "401.4"]; assert_eq!(names(&g, player), ["Grizzly Bears", "Aegis Turtle"]));
    }
}
