//! Instructions that wait on a player in the middle of what they do (CR 608.2), through the
//! game's own moves: what was done before the question is done once, the game that waits is
//! a value that can be copied and gone on with either way, and what several players choose
//! is done only when all have chosen. Every card is read from its printed text.
//!
//! 608.2c: "The controller of the spell or ability follows its instructions in the order
//! written."
//! 608.2e: "… the players make their choices in APNAP order … Then the actions happen
//! simultaneously."
//! 614.12a: "If a replacement effect that modifies how a permanent enters the battlefield
//! requires a choice, that choice is made before the permanent enters the battlefield."

use crate::support::{cite, paid};
use game::binding::{Kind, Value};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, PlayerId, Target, Zone};
use ir::mana::ManaSymbol;
use ir::{Color, Subtype, Verb};

const BEARS: usize = 0;
const GIANT: usize = 1;
const LOOTER: usize = 2;
const INNOCENT_BLOOD: usize = 3;
const BRASS_HERALD: usize = 4;
const CLOUDSHIFT: usize = 5;
const RAIDERS: usize = 6;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Hill Giant", "{3}{R}", "Creature — Giant", "", Some((3, 3))),
        card("Merfolk Looter", "{1}{U}", "Creature — Merfolk Rogue", "{T}: Draw a card, then discard a card.", Some((1, 1))),
        card("Innocent Blood", "{B}", "Sorcery", "Each player sacrifices a creature of their choice.", None),
        card(
            "Brass Herald",
            "{6}",
            "Artifact Creature — Golem",
            "As this creature enters, choose a creature type.\nWhen this creature enters, reveal the top four cards of your library. Put all creature cards of the chosen type revealed this way into your hand and the rest on the bottom of your library in any order.\nCreatures of the chosen type get +1/+1.",
            Some((2, 2)),
        ),
        card(
            "Cloudshift",
            "{W}",
            "Instant",
            "Exile target creature you control, then return that card to the battlefield under your control.",
            None,
        ),
        card("Mons's Goblin Raiders", "{R}", "Creature — Goblin", "", Some((1, 1))),
    ]
}

/// The cards a test uses must be ones the engine admits; the test fails here otherwise.
fn game(players: usize, cards: &[usize]) -> Game {
    let ds = defs();
    for &c in cards {
        game::play::check(&ds[c]).unwrap_or_else(|u| panic!("{} is not admitted: {}", ds[c].name(), u.0));
    }
    Game::new(ds, players, 20)
}

/// Every player passes in succession, from whoever has priority (405.5).
fn pass_all(g: &mut Game) {
    for _ in 0..g.players.len() {
        let p = g.priority;
        g.act(p, Move::Pass).unwrap_or_else(|e| panic!("player {p} passes: {e}"));
    }
}

/// The player casts this card from their hand with this mana; the spell on the stack.
fn cast(g: &mut Game, player: PlayerId, def: usize, mana: &[ManaSymbol], targets: Vec<Vec<Target>>, spending: &str) -> ObjId {
    for s in mana {
        g.add_mana(player, s.clone());
    }
    let card = g.put(player, def, Zone::Hand);
    g.act(player, paid(spending, Move::Cast { card, targets })).unwrap_or_else(|e| panic!("cast {}: {e}", g.defs[def].name()));
    *g.stack.last().expect("the spell is on the stack")
}

fn which(player: PlayerId, verb: Verb, among: Vec<ObjId>, count: usize) -> Option<Decision> {
    Some(Decision::Choose { player, choice: Choice::Objects { verb, to: None, among, count } })
}

fn choose(g: &mut Game, player: PlayerId, objects: &[ObjId]) -> Result<(), String> {
    g.act(player, Move::Choose(Answer::Objects(objects.to_vec())))
}

fn creature_type(name: &str) -> Value {
    Value::CreatureType(Subtype::from_name(name).unwrap_or_else(|| panic!("{name} is a subtype")))
}

fn count(g: &Game, of: impl Fn(&Event) -> bool) -> usize {
    g.log.iter().filter(|e| of(e)).count()
}

/// While the game waits for a choice nobody can pass, and a refused pass changes nothing.
/// `entry`: the choice is of how a permanent enters (614.12), not of the effect being applied.
fn nobody_can_pass(g: &mut Game, entry: bool) {
    let before = format!("{g:?}");
    for p in 0..g.players.len() {
        if entry {
            cite!(["614.12"]; refused("a decision is pending: a choice of how the permanent enters the battlefield (614.12)"): g.act(p, Move::Pass));
        } else {
            cite!(["608.2d"]; refused("a decision is pending: a choice of the effect being applied (608.2d)"): g.act(p, Move::Pass));
        }
        assert_eq!(format!("{g:?}"), before, "a refused pass by player {p} changed the game");
    }
}

#[test]
fn r608_2c_what_was_done_before_a_discard_choice_is_not_done_again() {
    // "Draw a card, then discard a card."
    let mut g = game(2, &[LOOTER]);
    let looter = g.put(0, LOOTER, Zone::Battlefield);
    let held = g.put(0, BEARS, Zone::Hand);
    for def in [GIANT, RAIDERS, GIANT] {
        g.put(0, def, Zone::Library);
    }
    g.act(0, Move::Activate { source: looter, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    pass_all(&mut g);

    // the draw has happened and the discard waits, with the ability still on the stack
    let Some(Decision::Choose { player: 0, choice: Choice::Objects { verb: Verb::Discard, among, count: 1, .. } }) = g.decision.clone() else {
        panic!("the discard waits for a choice: {:?}", g.decision)
    };
    cite!(["701.9b"]; assert_eq!(among, g.players[0].hand));
    assert_eq!((among.len(), g.players[0].library.len(), g.stack.len()), (2, 2, 1));
    let drew = |g: &Game| count(g, |e| matches!(e, Event::Drew { player: 0, .. }));
    let discarded = |g: &Game| count(g, |e| matches!(e, Event::Discarded { player: 0, .. }));
    cite!(["608.2c"]; assert_eq!((drew(&g), discarded(&g)), (1, 0)));
    let drawn = *among.iter().find(|c| **c != held).expect("the card drawn is one to choose from");

    // only that choice, by that player, of what is offered; nothing refused changes anything
    nobody_can_pass(&mut g, false);
    cite!(["701.9b"]; refused("that decision is another player's"): choose(&mut g, 1, &[held]));
    cite!(["608.2d"]; refused("choose exactly 1 of them to discard (608.2d)"): choose(&mut g, 0, &[held, drawn]));
    cite!(["701.9a"]; refused("a chosen object is not among the offered objects to discard (608.2d)"): choose(&mut g, 0, &[looter]));
    assert_eq!((drew(&g), g.players[0].library.len(), g.players[0].hand.len()), (1, 2, 2));

    // the waiting game is a value: copied, each goes on its own way from the same draw
    let mut other = g.clone();
    choose(&mut g, 0, &[drawn]).unwrap();
    choose(&mut other, 0, &[held]).unwrap();
    for (g, kept) in [(&g, held), (&other, drawn)] {
        cite!(["701.9b"]; assert_eq!(g.players[0].hand, vec![kept]));
        cite!(["608.2c"]; assert_eq!((drew(g), discarded(g)), (1, 1), "drawn once, discarded once"));
        assert_eq!((g.players[0].library.len(), g.players[0].graveyard.len()), (2, 1));
        assert_eq!((g.decision.clone(), g.stack.len()), (None, 0));
    }
    assert_eq!(g.obj(g.players[0].graveyard[0]).chars.name, "Hill Giant");
    assert_eq!(other.obj(other.players[0].graveyard[0]).chars.name, "Grizzly Bears");
    // an answer when nothing is asked is refused
    crate::support::refused(choose(&mut g, 0, &[held]), "no decision is pending");
}

#[test]
fn r608_2e_every_player_chooses_before_any_permanent_is_sacrificed() {
    // "Each player sacrifices a creature of their choice."
    let mut g = game(3, &[INNOCENT_BLOOD]);
    let mine = vec![g.put(0, BEARS, Zone::Battlefield), g.put(0, GIANT, Zone::Battlefield)];
    let only = g.put(1, RAIDERS, Zone::Battlefield);
    let theirs = vec![g.put(2, BEARS, Zone::Battlefield), g.put(2, GIANT, Zone::Battlefield)];
    let spell = cast(&mut g, 0, INNOCENT_BLOOD, &[ManaSymbol::Color(Color::Black)], vec![], "B");
    pass_all(&mut g);
    let sacrificed = |g: &Game| -> Vec<(PlayerId, ObjId)> {
        g.log.iter().filter_map(|e| if let Event::Sacrificed { player, object } = e { Some((*player, *object)) } else { None }).collect()
    };

    // the active player first (101.4), among what they control (701.21a)
    cite!(["101.4", "701.21a"]; assert_eq!(g.decision, which(0, Verb::Sacrifice, mine.clone(), 1)));
    nobody_can_pass(&mut g, false);
    cite!(["701.21a"]; refused("a chosen object is not among the offered objects to sacrifice (608.2d)"): choose(&mut g, 0, &theirs[..1]));
    choose(&mut g, 0, &mine[..1]).unwrap();
    // player 1 has one creature and nothing to choose; player 2 is asked next
    cite!(["608.2e", "101.4"]; assert_eq!(g.decision, which(2, Verb::Sacrifice, theirs.clone(), 1)));
    cite!(["608.2e"]; assert!(sacrificed(&g).is_empty(), "nothing is sacrificed until every player has chosen"));
    assert!([mine[0], only].iter().all(|o| g.battlefield.contains(o)));
    assert!(g.stack.contains(&spell), "the spell is on the stack while its instruction waits");
    nobody_can_pass(&mut g, false);
    cite!(["101.4"]; refused("that decision is another player's"): choose(&mut g, 0, &mine[1..]));

    // the game that waits for the last choice goes on either way; the first is not asked again
    let mut other = g.clone();
    choose(&mut g, 2, &theirs[..1]).unwrap();
    choose(&mut other, 2, &theirs[1..]).unwrap();
    for (g, gone, kept) in [(&g, theirs[0], theirs[1]), (&other, theirs[1], theirs[0])] {
        cite!(["608.2e"]; assert_eq!(sacrificed(g), vec![(0, mine[0]), (1, only), (2, gone)], "each once, in turn order"));
        assert!(g.battlefield.contains(&mine[1]) && g.battlefield.contains(&kept));
        assert!(!g.battlefield.contains(&mine[0]) && !g.battlefield.contains(&only) && !g.battlefield.contains(&gone));
        assert_eq!((g.decision.clone(), g.stack.len()), (None, 0));
        assert!(g.log.contains(&Event::Resolved { spell }));
        // each owner's graveyard: the creature, and the caster's spell (608.2n)
        cite!(["608.2n"]; assert_eq!(g.players.iter().map(|p| p.graveyard.len()).collect::<Vec<_>>(), vec![2, 1, 1]));
    }
}

#[test]
fn r614_12a_a_return_waits_for_its_entry_choice_after_the_exile_that_is_done_once() {
    // "Exile target creature you control, then return that card to the battlefield under
    // your control.", of a creature that enters with a choice
    let mut g = game(2, &[BRASS_HERALD, CLOUDSHIFT]);
    let herald = g.put(0, BRASS_HERALD, Zone::Battlefield);
    let goblin = g.put(1, RAIDERS, Zone::Battlefield);
    let bear = g.put(1, BEARS, Zone::Battlefield);
    let shift = cast(&mut g, 0, CLOUDSHIFT, &[ManaSymbol::Color(Color::White)], vec![vec![Target::Object(herald)]], "W");
    pass_all(&mut g);

    // the exile is done; the return waits for the choice made before the card enters
    cite!(["614.12a"]; assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::Value(Kind::CreatureType) })));
    let exiled = |g: &Game| count(g, |e| matches!(e, Event::ZoneChanged { to: Zone::Exile, .. }));
    let entered = |g: &Game| count(g, |e| matches!(e, Event::ZoneChanged { from: Zone::Exile, to: Zone::Battlefield, .. }));
    assert_eq!((exiled(&g), entered(&g)), (1, 0));
    cite!(["400.7"]; assert_eq!(g.obj(herald).zone, Zone::Gone, "the creature that was exiled is gone (400.7)"));
    let card = g.entering_card().expect("the entry question identifies its card");
    assert_eq!((g.obj(card).zone, g.obj(card).chars.name.as_str()), (Zone::Exile, "Brass Herald"));
    cite!(["614.12a"]; assert!(g.battlefield.iter().all(|o| g.obj(*o).chars.name != "Brass Herald"), "nothing has entered"));
    assert_eq!(g.stack.last(), Some(&shift));
    assert!(!g.log.contains(&Event::Resolved { spell: shift }));

    // no pass while it is pending, and only its chooser answers, with a creature type
    nobody_can_pass(&mut g, true);
    crate::support::refused(g.act(1, Move::Choose(Answer::Value(creature_type("Bear")))), "that decision is another player's");
    cite!(["205.3m"]; refused("choose a creature type (205.3m)"): g.act(0, Move::Choose(Answer::Yes)));
    assert_eq!((exiled(&g), entered(&g)), (1, 0));

    // copied while it waits, each game goes on from the one exile with its own answer
    let mut other = g.clone();
    g.act(0, Move::Choose(Answer::Value(creature_type("Bear")))).unwrap();
    other.act(0, Move::Choose(Answer::Value(creature_type("Goblin")))).unwrap();
    for (g, ty, powers) in [(&g, "Bear", (Some(1), Some(3))), (&other, "Goblin", (Some(2), Some(2)))] {
        cite!(["608.2c"]; assert_eq!((exiled(g), entered(g)), (1, 1), "exiled once, entered once"));
        let [new] = g.battlefield.iter().copied().filter(|o| g.obj(*o).chars.name == "Brass Herald").collect::<Vec<_>>()[..] else {
            panic!("one Brass Herald returned")
        };
        cite!(["400.7"]; assert!(new != herald && new != card, "a new object (400.7)"));
        assert_eq!(g.obj(new).controller, 0);
        let [record] = &g.obj(new).choices[..] else { panic!("one choice record") };
        assert_eq!((record.chooser, record.value.clone()), (0, creature_type(ty)));
        assert_eq!((g.power(goblin), g.power(bear)), powers);
        assert!(g.log.contains(&Event::Resolved { spell: shift }));
        assert_eq!(g.entering_card(), None);
        assert_eq!((g.decision.clone(), g.stack.len()), (None, 1), "its enters ability triggered");
    }
}

#[test]
fn r608_2c_a_search_between_committed_actions_keeps_its_result_and_never_repeats_the_prefix() {
    let defs = vec![
        CardDef::new("Search", "{0}", "Sorcery", "You gain 2 life. Search your library for a card, put it into your hand, then shuffle. You draw a card.", None).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
    ];
    game::play::check(&defs[0]).unwrap();
    let mut g = Game::new(defs, 3, 20);
    let cards: Vec<_> = (0..3).map(|_| g.put(0, 1, Zone::Library)).collect();
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
    pass_all(&mut g);
    assert_eq!(g.players[0].life, 22);
    nobody_can_pass(&mut g, false);
    let mut clone = g.clone();
    for (game, picked) in [(&mut g, cards[0]), (&mut clone, cards[1])] {
        game.act(0, Move::Choose(Answer::Objects(vec![picked]))).unwrap();
        let picked_move = game.log.iter().position(|e| matches!(e, Event::ZoneChanged { was, from: Zone::Library, to: Zone::Hand, .. } if *was == picked)).expect("the selected card moved into hand");
        let draw = game.log.iter().position(|e| matches!(e, Event::Drew { .. })).unwrap();
        cite!(["608.2c"]; assert!(picked_move < draw, "selection must happen before the separate draw"));
        assert_eq!(game.players[0].life, 22);
        assert_eq!(game.players[0].hand.len(), 2);
        assert_eq!(game.players[0].library.len(), 1);
        assert_eq!(game.log.iter().filter(|e| matches!(e, Event::Drew { .. })).count(), 1);
        cite!(["608.2c"]; assert_eq!(game.log.iter().filter(|e| matches!(e, Event::LifeGained { .. })).count(), 1));
        assert_eq!(game.log.iter().filter(|e| matches!(e, Event::Shuffled { .. })).count(), 1);
    }
}
