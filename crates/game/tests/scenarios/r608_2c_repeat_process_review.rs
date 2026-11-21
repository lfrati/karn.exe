//! Countryside Crusher's "reveal the top card of your
//! library. If it's a land card, put it into your graveyard and repeat this process" (608.2c),
//! with expectations derived from the rules and the card's rulings.
//! Each time through names the card then on top and asks the condition of it; the
//! first nonland card stays on top (ruling 948c4fe5); what the moves trigger waits for the
//! whole resolution (603.3, ruling 78f6e66c); every reveal is shown to every player and stops
//! being revealed once the effect is done (701.20a); the ability resolves without its source
//! (113.7a); emptying the library loses nothing until a draw from it (704.5b); a double-faced
//! card in a library has only its front face's characteristics (712.8a). Processes not known
//! to stop, or doing something else each time, are refused with a named reason (104.4b).
use crate::support::{cite, paid};
use game::event::Event;
use game::observation::Kind;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

fn pinned(name: &str) -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    CardDef::from_face(faces.iter().find(|f| f.face_name == name).unwrap_or_else(|| panic!("{name} pinned"))).unwrap()
}

const CRUSHER: usize = 0;
const FOREST: usize = 1;
const MOUNTAIN: usize = 2;
const BOLT: usize = 3;
const SKOLA: usize = 4;
/// The first card of `defs(more)` after these.
const MORE: usize = 5;

fn upkeep(rest: &str) -> String { format!("At the beginning of your upkeep, reveal the top card of your library. {rest}") }

fn defs(more: Vec<CardDef>) -> Vec<CardDef> {
    let mut defs = vec![
        pinned("Countryside Crusher"),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Mountain", "", "Basic Land — Mountain", "", None).unwrap(),
        CardDef::new("Lightning Bolt", "{R}", "Instant", "Lightning Bolt deals 3 damage to any target.", None).unwrap(),
        pinned("Skola Grovedancer"),
    ];
    defs.extend(more);
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    defs
}

/// A game whose player `who` has these cards in their library, the first on top; the other
/// player has five Forests.
fn game_for(defs: Vec<CardDef>, who: usize, library: &[usize]) -> Game {
    let mut g = Game::new(defs, 2, 20);
    for &card in library.iter().rev() { g.put(who, card, Zone::Library); }
    for _ in 0..5 { g.put(1 - who, FOREST, Zone::Library); }
    g
}
fn game(defs: Vec<CardDef>, library: &[usize]) -> Game { game_for(defs, 0, library) }

fn cards(g: &Game, objects: &[ObjId]) -> Vec<usize> { objects.iter().map(|o| g.obj(*o).card.unwrap()).collect() }
fn library_of(g: &Game, p: usize) -> Vec<usize> { g.players[p].library.iter().rev().map(|o| g.obj(*o).card.unwrap()).collect() }
fn library(g: &Game) -> Vec<usize> { library_of(g, 0) }
fn graveyard_of(g: &Game, p: usize) -> Vec<usize> { cards(g, &g.players[p].graveyard) }
fn graveyard(g: &Game) -> Vec<usize> { graveyard_of(g, 0) }
fn revealed_by(g: &Game, p: usize) -> Vec<Vec<usize>> {
    g.log.iter().filter_map(|e| match e { Event::Revealed { player, cards: shown } if *player == p => Some(cards(g, shown)), _ => None }).collect()
}
fn revealed(g: &Game) -> Vec<Vec<usize>> { revealed_by(g, 0) }
fn plus(g: &Game, o: ObjId) -> u32 { g.counters(o).get("+1/+1").copied().unwrap_or(0) }
/// Of the abilities on the stack, bottom first: whose ability.
fn sources(g: &Game) -> Vec<Option<ObjId>> { g.stack.iter().map(|o| g.obj(*o).ability.expect("an ability").0).collect() }

/// Order triggers as they are listed, if asked.
fn order(g: &mut Game) {
    if let Some(Decision::Order { player, abilities }) = g.decision.clone() {
        g.act(player, Move::Order((0..abilities.len()).collect())).unwrap();
    }
}

/// From the active player's untap step until its upkeep triggers are on the stack.
fn to_upkeep_trigger(g: &mut Game) {
    g.turn = 3;
    g.part = Part::Untap;
    for _ in 0..10 {
        order(g);
        if !g.stack.is_empty() { return }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("no upkeep trigger");
}

/// Everyone passes until the top of the stack has resolved; nothing may be asked meanwhile
/// except ordering what it caused once it is done.
fn resolve_top(g: &mut Game) {
    let top = *g.stack.last().expect("something to resolve");
    while g.stack.contains(&top) {
        assert!(g.decision.is_none(), "asked {:?} while the process resolves", g.decision);
        g.act(g.priority, Move::Pass).unwrap();
    }
}

/// Resolve the whole stack: triggers ordered as listed, each "may" declined.
fn settle(g: &mut Game) {
    for _ in 0..400 {
        match g.decision.clone() {
            Some(Decision::Order { .. }) => { order(g); continue }
            Some(Decision::Choose { player, choice: Choice::May }) => { g.act(player, Move::Choose(Answer::No)).unwrap(); continue }
            Some(other) => panic!("asked {other:?}"),
            None => {}
        }
        if g.stack.is_empty() { return }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("the stack did not empty");
}

/// The process stops at the first nonland card: lands under it stay where they are, and only
/// the run above it is put into the graveyard, each moved land triggering every "land card
/// put into your graveyard" ability once (603.2c: one event each).
#[test]
fn r608_2c_review_only_the_run_of_lands_above_the_first_nonland_is_moved() {
    let mut g = game(defs(Vec::new()), &[MOUNTAIN, FOREST, FOREST, BOLT, FOREST, MOUNTAIN, BOLT, FOREST]);
    let crusher = g.put(0, CRUSHER, Zone::Battlefield);
    g.put(0, SKOLA, Zone::Battlefield);
    to_upkeep_trigger(&mut g);
    resolve_top(&mut g);
    cite!(["608.2c"]; assert_eq!(revealed(&g), [vec![MOUNTAIN], vec![FOREST], vec![FOREST], vec![BOLT]]));
    cite!(["608.2c"]; assert_eq!(graveyard(&g), [MOUNTAIN, FOREST, FOREST]));
    cite!(["701.20b"]; assert_eq!(library(&g), [BOLT, FOREST, MOUNTAIN, BOLT, FOREST]));
    let Some(Decision::Order { player: 0, abilities }) = g.decision.clone() else { panic!("asked {:?}", g.decision) };
    cite!(["603.2c"]; assert_eq!(abilities.len(), 6, "a Crusher and a Grovedancer trigger for each of three lands"));
    settle(&mut g);
    assert_eq!((plus(&g, crusher), g.players[0].life), (3, 23));
    assert_eq!(revealed(&g).len(), 4, "nothing runs the process again");
}

/// With every card of the library a land, the library is emptied; that loses nothing by
/// itself, but the draw step's draw from it does (704.5b, 121.3).
#[test]
fn r704_5b_review_a_library_emptied_by_the_process_loses_at_the_next_draw() {
    let mut g = game(defs(Vec::new()), &[FOREST, FOREST, MOUNTAIN]);
    let crusher = g.put(0, CRUSHER, Zone::Battlefield);
    to_upkeep_trigger(&mut g);
    resolve_top(&mut g);
    assert!(library(&g).is_empty());
    settle(&mut g);
    assert_eq!(plus(&g, crusher), 3);
    cite!(["704.5b"]; assert!(!g.players[0].lost && g.winner().is_none(), "revealing and milling are not drawing"));
    for _ in 0..20 {
        if g.players[0].lost || g.part != Part::Upkeep { break }
        g.act(g.priority, Move::Pass).unwrap();
    }
    cite!(["704.5b"]; assert!(g.players[0].lost, "the draw step's draw from an empty library loses (704.5b), at {:?}", g.part));
    assert_eq!(g.winner(), Some(1));
}

/// The Crusher is bolted in response: its upkeep ability still does all it says (113.7a),
/// and the lands it moves trigger the Grovedancer for each, but not the gone Crusher.
#[test]
fn r113_7a_review_without_its_source_the_process_still_triggers_other_permanents() {
    let mut g = game(defs(Vec::new()), &[FOREST, MOUNTAIN, FOREST, BOLT]);
    let crusher = g.put(0, CRUSHER, Zone::Battlefield);
    let skola = g.put(0, SKOLA, Zone::Battlefield);
    let bolt = g.put(0, BOLT, Zone::Hand);
    to_upkeep_trigger(&mut g);
    g.players[0].pool.colors = [0, 0, 0, 1, 0];
    g.act(0, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Object(crusher)]] })).unwrap();
    resolve_top(&mut g);
    assert_ne!(g.obj(crusher).zone, Zone::Battlefield);
    let gone = graveyard(&g).len();
    resolve_top(&mut g);
    order(&mut g);
    cite!(["113.7a"]; assert_eq!(revealed(&g), [vec![FOREST], vec![MOUNTAIN], vec![FOREST], vec![BOLT]]));
    cite!(["113.7a"]; assert_eq!(graveyard(&g).len(), gone + 3));
    assert_eq!(library(&g), [BOLT]);
    cite!(["113.6"]; assert_eq!(sources(&g), [Some(skola); 3], "only the Grovedancer triggers"));
    settle(&mut g);
    assert_eq!(g.players[0].life, 23);
}

/// Every reveal is shown to every player, one card at a time in the order revealed
/// (701.20a); once the ability is done, the nonland card left on top is no longer revealed,
/// so the opponent may not look at it, though what they were shown stays theirs.
#[test]
fn r701_20a_review_each_reveal_is_public_and_ends_with_the_effect() {
    let mut g = game(defs(Vec::new()), &[FOREST, MOUNTAIN, BOLT, FOREST]);
    g.put(0, CRUSHER, Zone::Battlefield);
    to_upkeep_trigger(&mut g);
    g.take_observations(0);
    g.take_observations(1);
    resolve_top(&mut g);
    for seat in 0..2 {
        let shown: Vec<(Kind, usize, Vec<Option<usize>>)> = g.observations(seat).iter()
            .map(|d| (d.kind, d.by, d.cards.iter().map(|c| c.card).collect())).collect();
        cite!(["701.20a"]; case: format!("seat {seat}"); assert_eq!(shown, [
            (Kind::Revealed, 0, vec![Some(FOREST)]), (Kind::Revealed, 0, vec![Some(MOUNTAIN)]), (Kind::Revealed, 0, vec![Some(BOLT)]),
        ], "seat {seat}"));
        let seqs: Vec<u64> = g.observations(seat).iter().map(|d| d.seq).collect();
        assert!(seqs.windows(2).all(|w| w[0] < w[1]), "in the order shown");
        assert!(g.observations(seat).iter().all(|d| d.cards.iter().all(|c| c.zone == Zone::Library)), "shown as they were, in the library");
    }
    settle(&mut g);
    let top = *g.players[0].library.last().unwrap();
    assert_eq!(g.obj(top).card, Some(BOLT));
    cite!(["701.20a"]; assert!(!g.sees(1, top), "the card left on top stopped being revealed with the effect"));
}

/// The condition is asked of the card each time reveals, whatever it describes: a process
/// that goes on while the card is a creature card stops at a land card. A land creature card
/// is a land card (205.2a), and a basic land condition stops at a nonbasic land.
#[test]
fn r608_2c_review_the_condition_is_asked_of_each_new_top_card() {
    let all = || defs(vec![
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Arbor", "", "Land Creature — Forest Dryad", "", Some((1, 1))).unwrap(),
        CardDef::new("Wastes Field", "", "Land", "", None).unwrap(),
        CardDef::new("Eater", "{0}", "Creature — Giant", &upkeep("If it's a creature card, put it into your graveyard and repeat this process."), Some((1, 1))).unwrap(),
        CardDef::new("Basic Eater", "{0}", "Creature — Giant", &upkeep("If it's a basic land card, put it into your graveyard and repeat this process."), Some((1, 1))).unwrap(),
    ]);
    let (bear, arbor, wastes, eater, basic) = (MORE, MORE + 1, MORE + 2, MORE + 3, MORE + 4);

    // a creature card each time, then a land: two creatures go
    let mut g = game(all(), &[bear, bear, FOREST, bear]);
    g.put(0, eater, Zone::Battlefield);
    to_upkeep_trigger(&mut g);
    resolve_top(&mut g);
    cite!(["608.2c"]; assert_eq!(graveyard(&g), [bear, bear]));
    assert_eq!(library(&g), [FOREST, bear]);

    // a land creature card is both: the Eater and the Crusher each take it
    let mut g = game(all(), &[arbor, bear, MOUNTAIN]);
    g.put(0, eater, Zone::Battlefield);
    to_upkeep_trigger(&mut g);
    resolve_top(&mut g);
    cite!(["608.2c", "205.2a"]; assert_eq!(graveyard(&g), [arbor, bear]));
    assert_eq!(library(&g), [MOUNTAIN]);
    let mut g = game(all(), &[FOREST, arbor, bear]);
    let crusher = g.put(0, CRUSHER, Zone::Battlefield);
    to_upkeep_trigger(&mut g);
    resolve_top(&mut g);
    cite!(["608.2c", "205.2a"]; assert_eq!(graveyard(&g), [FOREST, arbor]));
    assert_eq!(library(&g), [bear]);
    settle(&mut g);
    assert_eq!(plus(&g, crusher), 2);

    // basic: a nonbasic land stops it, though the Crusher would take it
    let mut g = game(all(), &[FOREST, wastes, MOUNTAIN]);
    g.put(0, basic, Zone::Battlefield);
    to_upkeep_trigger(&mut g);
    resolve_top(&mut g);
    cite!(["608.2c"]; assert_eq!(graveyard(&g), [FOREST]));
    assert_eq!(library(&g), [wastes, MOUNTAIN]);
}

/// In a library, a modal double-faced card has only its front face's characteristics
/// (712.8a): a spell with a land back face is not a land card and stops the process; a card
/// whose front face is a land is one.
#[test]
fn r712_8a_review_a_modal_card_is_a_land_card_in_a_library_only_by_its_front() {
    let spell_land = CardDef::modal(
        CardDef::new("Front Spell", "{1}{G}", "Sorcery", "You gain 1 life.", None).unwrap(),
        CardDef::new("Back Land", "", "Land", "{T}: Add {G}.", None).unwrap()).unwrap();
    let land_spell = CardDef::modal(
        CardDef::new("Front Land", "", "Land", "{T}: Add {G}.", None).unwrap(),
        CardDef::new("Back Spell", "{1}{G}", "Sorcery", "You gain 1 life.", None).unwrap()).unwrap();
    let (spell_land_card, land_spell_card) = (MORE, MORE + 1);
    let mut g = game(defs(vec![spell_land, land_spell]), &[land_spell_card, FOREST, spell_land_card, MOUNTAIN]);
    let crusher = g.put(0, CRUSHER, Zone::Battlefield);
    to_upkeep_trigger(&mut g);
    resolve_top(&mut g);
    cite!(["712.8a"]; assert_eq!(graveyard(&g), [land_spell_card, FOREST]));
    cite!(["712.8a"]; assert_eq!(library(&g), [spell_land_card, MOUNTAIN], "a spell with a land back is not a land card here"));
    settle(&mut g);
    assert_eq!(plus(&g, crusher), 2);
}

/// "Your library" and "your graveyard" are the ability's controller's: an opponent's Crusher
/// in its controller's upkeep reveals and moves that player's cards, and the other player's
/// Crusher does not trigger in that upkeep.
#[test]
fn r608_2c_review_the_process_is_its_controllers_in_their_own_upkeep() {
    let mut g = game_for(defs(Vec::new()), 1, &[MOUNTAIN, FOREST, BOLT]);
    for _ in 0..3 { g.put(0, FOREST, Zone::Library); }
    let mine = g.put(0, CRUSHER, Zone::Battlefield);
    let theirs = g.put(1, CRUSHER, Zone::Battlefield);
    g.active = 1;
    g.priority = 1;
    to_upkeep_trigger(&mut g);
    assert_eq!(sources(&g), [Some(theirs)], "only the active player's Crusher triggers");
    resolve_top(&mut g);
    cite!(["109.5"]; assert_eq!(revealed_by(&g, 1), [vec![MOUNTAIN], vec![FOREST], vec![BOLT]]));
    assert!(revealed_by(&g, 0).is_empty());
    cite!(["109.5"]; assert_eq!(graveyard_of(&g, 1), [MOUNTAIN, FOREST]));
    assert_eq!(library_of(&g, 1), [BOLT]);
    assert!(graveyard_of(&g, 0).is_empty());
    assert_eq!(library_of(&g, 0).len(), 8);
    settle(&mut g);
    cite!(["109.5"]; assert_eq!((plus(&g, theirs), plus(&g, mine)), (2, 0), "a land put into another player's graveyard is not one put into yours"));
}

/// Two Crushers and a library of lands only: the first empties it, and the second reveals
/// an empty library, which shows nothing and is not as described (each Crusher still counts
/// every land).
#[test]
fn r608_2c_review_a_second_crusher_after_the_first_emptied_the_library() {
    let mut g = game(defs(Vec::new()), &[FOREST, MOUNTAIN]);
    let first = g.put(0, CRUSHER, Zone::Battlefield);
    let second = g.put(0, CRUSHER, Zone::Battlefield);
    to_upkeep_trigger(&mut g);
    resolve_top(&mut g);
    order(&mut g);
    cite!(["603.3"]; assert_eq!(g.stack.len(), 1 + 4));
    settle(&mut g);
    cite!(["608.2c"]; assert_eq!(revealed(&g), [vec![FOREST], vec![MOUNTAIN]], "the second reveals nothing"));
    cite!(["603.2"]; assert_eq!((plus(&g, first), plus(&g, second)), (2, 2)));
}

/// A copy of the game taken where its controller orders what the process caused goes on by
/// itself: any order gives the same counters and life, the original still asks, and neither
/// copy runs the process again.
#[test]
fn r603_3b_review_copies_at_the_order_question_are_independent() {
    let mut g = game(defs(Vec::new()), &[FOREST, MOUNTAIN, FOREST, BOLT]);
    let crusher = g.put(0, CRUSHER, Zone::Battlefield);
    g.put(0, SKOLA, Zone::Battlefield);
    to_upkeep_trigger(&mut g);
    resolve_top(&mut g);
    let Some(Decision::Order { player: 0, abilities }) = g.decision.clone() else { panic!("asked {:?}", g.decision) };
    assert_eq!(abilities.len(), 6);
    let n = abilities.len();
    for ordering in [(0..n).collect::<Vec<_>>(), (0..n).rev().collect()] {
        let mut copy = g.clone();
        copy.act(0, Move::Order(ordering)).unwrap();
        settle(&mut copy);
        cite!(["603.3b"]; assert_eq!((plus(&copy, crusher), copy.players[0].life), (3, 23)));
        assert_eq!(revealed(&copy).len(), 4);
        assert_eq!(library(&copy), [BOLT]);
    }
    assert!(matches!(g.decision, Some(Decision::Order { .. })), "the original still asks");
    assert_eq!((plus(&g, crusher), g.players[0].life), (0, 20));
}

/// Wording that is not a process known to stop, or that does more than reveal and move each
/// time, is refused with a reason naming "repeat this process"; one whose condition is of
/// the revealed card is run whatever it describes.
#[test]
fn r104_4b_review_repetitions_outside_the_stopping_shape_are_refused() {
    let check = |text: &str| {
        let def = CardDef::new("P", "{0}", "Creature — Giant", text, Some((1, 1))).unwrap();
        game::play::check(&def).map_err(|e| e.0)
    };
    for rest in [
        // the card stays in the library: with only lands, a mandatory loop (104.4b)
        "If it's a land card, put it on the bottom of your library and repeat this process.",
        // optional each time
        "If it's a land card, you may put it into your graveyard. If you do, repeat this process.",
        "If it's a land card, you may put it into your graveyard and repeat this process.",
        // something else done each time, or elsewhere
        "If it's a land card, put it into your graveyard, you gain 1 life, and repeat this process.",
        "If it's a land card, put it onto the battlefield and repeat this process.",
        "If it's a land card, exile it and repeat this process.",
        // not of the revealed card
        "If you control a Mountain, put it into your graveyard and repeat this process.",
        "If it isn't a land card, put it into your graveyard and repeat this process.",
    ] {
        let text = upkeep(rest);
        let error = check(&text).expect_err(&text);
        assert!(error.starts_with("repeat this process"), "{text}: {error}");
    }
    // These syntax-valid variations cross distinct current lowering boundaries.
    // They are support refusals, not claims that the instructions are illegal Magic.
    let stopping_shape = "repeat this process: only a revealed top card put into a graveyard while it is as described";
    for (text, reason) in [
        ("At the beginning of your upkeep, reveal the top card of your library. If it's a land card, shuffle it into your library and repeat this process.", stopping_shape),
        ("At the beginning of your upkeep, reveal the top card of target player's library. If it's a land card, put it into that player's graveyard and repeat this process.", "repeat this process: targets are chosen once, not each time"),
        ("At the beginning of your upkeep, each player reveals the top card of their library. If it's a land card, that player puts it into their graveyard and repeats this process.", "program: instruction subject not lowered"),
        ("At the beginning of your upkeep, reveal the top two cards of your library. If they're land cards, put them into your graveyard and repeat this process.", "condition: is, of \"Definite\""),
        ("At the beginning of your upkeep, mill a card. If it's a land card, repeat this process.", "input reference: nothing before it supplies the required participant/cardinality"),
    ] {
        let error = check(text).expect_err(text);
        assert_eq!(error, reason, "{text}");
    }
    // "nonland" is a description of the card: it stops at a land, and an empty reveal
    let eater = CardDef::new("Nonland Eater", "{0}", "Creature — Giant", &upkeep("If it's a nonland card, put it into your graveyard and repeat this process."), Some((1, 1))).unwrap();
    let mut g = game(defs(vec![eater]), &[BOLT, BOLT, FOREST, BOLT]);
    g.put(0, MORE, Zone::Battlefield);
    to_upkeep_trigger(&mut g);
    resolve_top(&mut g);
    cite!(["608.2c"]; assert_eq!(graveyard(&g), [BOLT, BOLT]));
    assert_eq!(library(&g), [FOREST, BOLT]);
    let mut g = game(defs(vec![CardDef::new("Nonland Eater", "{0}", "Creature — Giant", &upkeep("If it's a nonland card, put it into your graveyard and repeat this process."), Some((1, 1))).unwrap()]), &[BOLT, BOLT]);
    g.put(0, MORE, Zone::Battlefield);
    to_upkeep_trigger(&mut g);
    resolve_top(&mut g);
    assert_eq!(graveyard(&g), [BOLT, BOLT]);
    cite!(["608.2c"]; assert!(library(&g).is_empty(), "an empty reveal is not a nonland card: it stops"));
}
