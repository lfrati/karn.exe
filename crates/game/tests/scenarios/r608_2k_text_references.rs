//! What "it", "them", "that creature" and "those creatures" are said of when the text itself
//! named it earlier (608.2k): the permanents an earlier sentence acted on, what a player
//! sacrificed, the cards milled, the source the text called "this creature". Decided as the
//! text is lowered, by how each was named; every card here passes admission.

use crate::support::{cite, paid};
use game::card::CardDef;
use game::play::check;
use game::state::{Answer, Choice, Decision};
use game::{Game, Move, ObjId, Target, Zone};

const BEARS: usize = 0;
const WALL: usize = 1;
const FOREST: usize = 2;
const ROCK: usize = 3;
const CARD: usize = 4;

/// Grizzly Bears, a 0/20 Wall, a Forest, an artifact, then the cards under test.
fn game(cards: &[(&str, &str, &str, Option<(i32, i32)>)]) -> Game {
    let card = |name, types, text, pt| {
        // free, but for the one white spell
        let def = CardDef::new(name, if name == "Prayer" { "{W}" } else { "{0}" }, types, text, pt).expect(name);
        check(&def).unwrap_or_else(|e| panic!("{name}: {}", e.0));
        def
    };
    let mut defs = vec![
        card("Grizzly Bears", "Creature — Bear", "", Some((2, 2))),
        card("Wall", "Creature — Wall", "", Some((0, 20))),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        card("Rock", "Artifact", "", None),
    ];
    defs.extend(cards.iter().map(|(name, types, text, pt)| card(name, types, text, *pt)));
    let mut g = Game::new(defs, 2, 20);
    for player in 0..2 {
        for _ in 0..10 {
            g.put(player, BEARS, Zone::Library);
        }
    }
    g
}

fn refused(types: &str, text: &str) -> String {
    let def = CardDef::new("Refused", "{0}", types, text, Some((1, 1))).expect(text);
    check(&def).expect_err(text).0
}

/// Both players pass until the stack is empty or a decision is asked.
fn resolve(g: &mut Game) {
    for _ in 0..20 {
        if g.decision.is_some() || g.stack.is_empty() {
            return;
        }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("the stack does not empty");
}

fn cast(g: &mut Game, def: usize, targets: Vec<Vec<Target>>, spending: &str) {
    let card = g.put(0, def, Zone::Hand);
    g.act(0, paid(spending, Move::Cast { card, targets })).unwrap();
    resolve(g);
}

fn activate(g: &mut Game, source: ObjId, targets: Vec<Vec<Target>>, spending: &str) {
    g.act(0, paid(spending, Move::Activate { source, ability: 0, targets, paying: vec![] })).unwrap();
    resolve(g);
}

fn tapped(g: &mut Game, player: usize, def: usize) -> ObjId {
    let o = g.put(player, def, Zone::Battlefield);
    g.tap(o);
    o
}

/// Passes until that player's next turn has begun and its untap step is over.
fn next_turn_of(g: &mut Game, player: usize) {
    let turn = g.turn;
    for _ in 0..200 {
        if g.turn > turn && g.active == player {
            return;
        }
        match &g.decision {
            Some(Decision::Attackers { player }) => { let p = *player; g.act(p, Move::Attack(vec![])).unwrap() }
            Some(Decision::Discard { player, count }) => {
                let (p, hand) = (*player, g.players[*player].hand[..*count].to_vec());
                g.act(p, Move::Discard(hand)).unwrap()
            }
            Some(other) => panic!("unexpected decision: {other:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("the turn does not come");
}

#[test]
fn r608_2k_those_creatures_are_the_ones_the_earlier_sentence_acted_on() {
    for text in [
        "Creatures you control get +2/+1 until end of turn. Untap those creatures.",
        "Creatures you control get +1/+1 until end of turn. Untap them.",
    ] {
        let mut g = game(&[("Flare", "Instant", text, None)]);
        let mine = [tapped(&mut g, 0, BEARS), tapped(&mut g, 0, WALL)];
        let theirs = tapped(&mut g, 1, BEARS);
        let rock = tapped(&mut g, 0, ROCK);
        cast(&mut g, CARD, vec![], "");
        cite!(["608.2c"]; case: text; assert!(mine.iter().all(|o| !g.obj(*o).tapped), "{text}"));
        cite!(["608.2c"]; case: text; assert!(g.obj(theirs).tapped && g.obj(rock).tapped, "{text}"));
        cite!(["613.4c"]; case: text; assert!(g.power(mine[0]) > Some(2) && g.power(theirs) == Some(2), "{text}"));
    }
}

#[test]
fn r608_2k_they_are_each_permanent_the_counters_were_put_on() {
    let mut g = game(&[
        ("Ardbert", "Legendary Creature — Spirit Warrior", "Whenever you cast a white spell, put a +1/+1 counter on each legendary creature you control. They gain vigilance until end of turn.", Some((2, 2))),
        ("Prayer", "Instant", "You gain 1 life.", None),
    ]);
    let white = CARD + 1;
    let ardbert = g.put(0, CARD, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    g.add_mana(0, ir::ManaSymbol::Color(ir::Color::White));
    cast(&mut g, white, vec![], "W");
    resolve(&mut g);
    cite!(["613.4c"]; assert_eq!(g.power(ardbert), Some(3)));
    cite!(["608.2c"]; assert!(g.has(ardbert, "Vigilance")));
    assert_eq!(g.power(bears), Some(2));
    cite!(["608.2c"]; assert!(!g.has(bears, "Vigilance")));
}

#[test]
fn r608_2h_that_creature_is_the_one_sacrificed_as_it_last_was() {
    for (text, gains) in [
        ("Target player sacrifices a creature of their choice, then gains life equal to that creature's toughness.", 1),
        ("Target opponent sacrifices a creature of their choice. You gain life equal to that creature's toughness.", 0),
    ] {
        let mut g = game(&[("Hunger", "Instant", text, None)]);
        let bears = g.put(1, BEARS, Zone::Battlefield);
        let wall = g.put(1, WALL, Zone::Battlefield);
        let card = g.put(0, CARD, Zone::Hand);
        g.act(0, Move::Cast { card, targets: vec![vec![Target::Player(1)]] }).unwrap();
        resolve(&mut g);
        cite!(["701.21a"]; case: text; assert!(matches!(&g.decision, Some(Decision::Choose { player: 1, choice: Choice::Objects { among, count: 1, .. } }) if among.len() == 2), "{:?}", g.decision));
        g.act(1, Move::Choose(Answer::Objects(vec![wall]))).unwrap();
        resolve(&mut g);
        assert_eq!(g.obj(bears).zone, Zone::Battlefield);
        cite!(["701.21a"]; case: text; assert_ne!(g.obj(wall).zone, Zone::Battlefield));
        cite!(["608.2h"]; case: text; assert_eq!(g.players[gains].life, 40, "{text}"));
        assert_eq!(g.players[1 - gains].life, 20, "{text}");
    }
}

#[test]
fn r608_2h_nothing_sacrificed_has_no_toughness_to_gain() {
    let mut g = game(&[("Hunger", "Instant", "Target opponent sacrifices a creature of their choice. You gain life equal to that creature's toughness.", None)]);
    cast(&mut g, CARD, vec![vec![Target::Player(1)]], "");
    assert!(g.decision.is_none() && g.stack.is_empty());
    cite!(["608.2h"]; assert_eq!(g.players[0].life, 20));
}

#[test]
fn r608_2k_it_is_the_source_the_text_called_this_creature() {
    let mut g = game(&[
        ("Sentinel", "Creature — Skeleton", "{3}: Tap this creature. It gains indestructible until end of turn.", Some((2, 1))),
        ("Warrior", "Creature — Homarid", "{U}: This creature gains shroud until end of turn and doesn't untap during your next untap step. Tap it.", Some((3, 3))),
    ]);
    let sentinel = g.put(0, CARD, Zone::Battlefield);
    let warrior = g.put(0, CARD + 1, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    for _ in 0..3 { g.add_mana(0, ir::ManaSymbol::Colorless); }
    activate(&mut g, sentinel, vec![], "CCC");
    cite!(["608.2c"]; assert!(g.obj(sentinel).tapped && g.has(sentinel, "Indestructible")));
    g.add_mana(0, ir::ManaSymbol::Color(ir::Color::Blue));
    activate(&mut g, warrior, vec![], "U");
    cite!(["608.2c"]; assert!(g.obj(warrior).tapped && g.has(warrior, "Shroud")));
    cite!(["608.2c"]; assert!(!g.obj(bears).tapped && !g.has(bears, "Shroud") && !g.has(bears, "Indestructible")));
}

#[test]
fn r608_2k_it_is_not_the_source_where_the_event_has_something_else_to_mean() {
    // what the same act is done to is not the source it names
    check(&CardDef::new("Harness", "{0}", "Artifact — Equipment", "Whenever a creature enters, attach this Equipment to it.", None).unwrap()).unwrap();
    assert!(refused("Artifact — Equipment", "{0}: Attach this Equipment to it.").contains("input reference"));
    // Naming the source in the body establishes the following sentence's antecedent,
    // even though a different creature supplied the trigger event.
    let mut g = game(&[("Watcher", "Creature — Spirit", "Whenever another creature enters, tap this creature. It gains flying until end of turn.", Some((1, 1)))]);
    let source = g.put(0, CARD, Zone::Battlefield);
    cast(&mut g, BEARS, vec![], "");
    let other = *g.battlefield.iter().find(|o| **o != source).unwrap();
    cite!(["608.2c"]; assert!(g.obj(source).tapped && g.has(source, "Flying")));
    cite!(["608.2c"]; assert!(!g.obj(other).tapped && !g.has(other, "Flying")));
}

#[test]
fn r608_2k_that_lands_controller_is_whoever_controlled_the_destroyed_land() {
    let mut g = game(&[("Terrain", "Sorcery", "Destroy target land, then Terrain deals damage to that land's controller equal to the number of land cards in that player's graveyard.", None)]);
    let land = g.put(1, FOREST, Zone::Battlefield);
    g.put(1, FOREST, Zone::Graveyard);
    g.put(1, BEARS, Zone::Graveyard);
    g.put(0, FOREST, Zone::Graveyard);
    cast(&mut g, CARD, vec![vec![Target::Object(land)]], "");
    cite!(["701.8a"]; assert_ne!(g.obj(land).zone, Zone::Battlefield));
    // the destroyed land is in that graveyard by then
    cite!(["608.2h", "608.2c"]; assert_eq!(g.players[1].life, 18));
    cite!(["608.2h"]; assert_eq!(g.players[0].life, 20));
}

#[test]
fn r701_17_cards_from_among_them_are_only_the_cards_milled() {
    let mut g = game(&[("Tilling", "Instant", "Mill four cards, then you may return a permanent card from among them to your hand.", None)]);
    let old = g.put(0, BEARS, Zone::Graveyard);
    let top: Vec<ObjId> = g.players[0].library.iter().rev().take(4).copied().collect();
    cast(&mut g, CARD, vec![], "");
    // CR404.3: explicitly authored owner order, separate from selection.
    assert_eq!(g.decision, Some(game::state::Decision::Choose { player: 0, choice: game::state::Choice::OrderCards { to: game::state::Placement::Zone(Zone::Graveyard), cards: top.clone() } }));
    g.act(0, Move::Choose(game::state::Answer::Objects(vec![top[3],top[2],top[1],top[0]]))).unwrap();
    assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May { possible: true } }));
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    let Some(Decision::Choose { player: 0, choice: Choice::Objects { among, .. } | Choice::Search { among, .. } }) = g.decision.clone() else { panic!("{:?}", g.decision) };
    cite!(["701.17a"]; assert_eq!(among.len(), 4));
    cite!(["400.7"]; assert!(!among.contains(&old) && !top.iter().any(|c| among.contains(c)), "milled cards are new objects (400.7)"));
    cite!(["701.17a"]; assert!(among.iter().all(|c| g.obj(*c).zone == Zone::Graveyard)));
    g.act(0, Move::Choose(Answer::Objects(vec![among[0]]))).unwrap();
    resolve(&mut g);
    cite!(["608.2c"]; assert_eq!(g.players[0].hand.len(), 1));
    assert_eq!(g.players[0].graveyard.iter().filter(|c| g.obj(**c).card == Some(BEARS)).count(), 4);
}

#[test]
fn r608_2k_that_creature_is_the_other_one_not_the_source_called_it() {
    let mut g = game(&[("Trygon", "Creature — Tyranid", "Whenever this creature attacks, put a +1/+1 counter on it and a +1/+1 counter on up to one other target attacking creature. That creature can't be blocked this turn.", Some((3, 3)))]);
    let trygon = g.put(0, CARD, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let blocker = g.put(1, WALL, Zone::Battlefield);
    for _ in 0..60 {
        if g.decision.is_some() { break }
        g.act(g.priority, Move::Pass).unwrap();
    }
    g.act(0, Move::Attack(vec![(trygon, Target::Player(1)), (bears, Target::Player(1))])).unwrap();
    assert!(matches!(g.decision, Some(Decision::Targets { player: 0, .. })), "{:?}", g.decision);
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(bears)]])).unwrap();
    resolve(&mut g);
    cite!(["122.1a", "613.4c"]; assert_eq!((g.power(trygon), g.power(bears)), (Some(4), Some(3))));
    for _ in 0..60 {
        if g.decision.is_some() { break }
        g.act(g.priority, Move::Pass).unwrap();
    }
    cite!(["509.1b"]; refused("Wall can't block Grizzly Bears: Trygon says it can't, or not that way (509.1b)"): g.clone().act(1, Move::Block(vec![(blocker, bears)])));
    g.act(1, Move::Block(vec![(blocker, trygon)])).unwrap();
}

#[test]
fn r608_2k_that_land_is_the_land_and_that_creature_is_of_its_own_sentence() {
    for (text, first, second) in [
        ("Revolution deals 4 damage to target creature. Tap target land. That land doesn't untap during its controller's next untap step.", WALL, FOREST),
        ("Put a +1/+1 counter on up to one target creature you control. Tap up to one target creature you don't control, and that creature doesn't untap during its controller's next untap step.", BEARS, WALL),
    ] {
        let mut g = game(&[("Revolution", "Sorcery", text, None)]);
        let a =g.put(if first == BEARS { 0 } else { 1 }, first, Zone::Battlefield);
        let b = g.put(1, second, Zone::Battlefield);
        let other = tapped(&mut g, 1, ROCK);
        cast(&mut g, CARD, vec![vec![Target::Object(a)], vec![Target::Object(b)]], "");
        cite!(["608.2c"]; case: text; assert!(!g.obj(a).tapped && g.obj(b).tapped, "{text}"));
        next_turn_of(&mut g, 1);
        cite!(["502.3"]; case: text; assert!(g.obj(b).tapped, "{text}"));
        cite!(["502.3"]; case: text; assert!(!g.obj(other).tapped, "{text}"));
    }
}

#[test]
fn r400_7_what_was_exiled_returns_as_the_new_objects_it_became() {
    let mut g = game(&[
        ("Spirit", "Creature — Spirit", "{3}{W}: Exile this creature, then return it to the battlefield under its owner's control.", Some((2, 2))),
        ("Solvers", "Legendary Creature — Turtle", "At the beginning of your end step, exile up to one target artifact you control and up to one target creature you control. Then return them to the battlefield under their owners' control.", Some((3, 3))),
    ]);
    let spirit = tapped(&mut g, 0, CARD);
    for _ in 0..3 { g.add_mana(0, ir::ManaSymbol::Colorless); }
    g.add_mana(0, ir::ManaSymbol::Color(ir::Color::White));
    activate(&mut g, spirit, vec![], "CCCW");
    assert_eq!(g.obj(spirit).zone, Zone::Gone);
    let back: Vec<ObjId> = g.battlefield.iter().copied().filter(|o| g.obj(*o).card == Some(CARD)).collect();
    cite!(["400.7", "110.5b"]; assert!(matches!(back.as_slice(), [one] if *one != spirit && !g.obj(*one).tapped)));

    g.put(0, CARD + 1, Zone::Battlefield);
    let rock = tapped(&mut g, 0, ROCK);
    let bears = tapped(&mut g, 0, BEARS);
    let kept = tapped(&mut g, 0, WALL);
    for _ in 0..80 {
        match &g.decision {
            Some(Decision::Targets { .. }) => break,
            Some(Decision::Attackers { .. }) => g.act(0, Move::Attack(vec![])).unwrap(),
            Some(other) => panic!("{other:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(rock)], vec![Target::Object(bears)]])).unwrap();
    resolve(&mut g);
    assert_eq!((g.obj(rock).zone, g.obj(bears).zone), (Zone::Gone, Zone::Gone));
    for def in [ROCK, BEARS] {
        let now: Vec<ObjId> = g.battlefield.iter().copied().filter(|o| g.obj(*o).card == Some(def)).collect();
        cite!(["400.7", "110.5b"]; case: def; assert!(matches!(now.as_slice(), [one] if !g.obj(*one).tapped && g.obj(*one).controller == 0), "{def}"));
    }
    cite!(["608.2c"]; assert!(g.obj(kept).tapped));
}
