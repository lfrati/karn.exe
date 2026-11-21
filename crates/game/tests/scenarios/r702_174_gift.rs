//! Gift (702.174): "Gift a tapped Fish" is "As an additional cost to cast this spell, you may
//! choose an opponent" and, of an instant or sorcery, "If this spell's gift cost was paid, the
//! chosen player creates a tapped 1/1 blue Fish creature token", which happens before any other
//! of its abilities (702.174j). Targets of a part that applies only if the gift was promised are
//! chosen only then (702.174m). Sazacap's Brew is its Oracle text; the others are test cards.
use crate::support::cite;
use game::card::CardDef;
use game::event::Event;
use game::legal::Asked;
use game::play::check;
use game::state::{Answer, Choice, Decision};
use game::{Game, Move, ObjId, Target, Zone};
use ir::mana::ManaSymbol;
use ir::Color;

const BREW: usize = 0;
const BEARS: usize = 1;
const DENY: usize = 2;
const MOUNTAIN: usize = 3;
const BOOST: usize = 4;
const SHOCK: usize = 5;
const PRESENT: usize = 6;
const DREDGER: usize = 7;

const BREW_TEXT: &str = "Gift a tapped Fish\nAs an additional cost to cast this spell, discard a card.\nTarget player draws two cards. If the gift was promised, target creature you control gets +2/+0 until end of turn.";

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Sazacap's Brew", "{1}{R}", "Instant", BREW_TEXT, None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Deny", "{0}", "Instant", "Counter target spell.", None),
        card("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None),
        // a gift with one target, which can leave before the spell resolves
        card("Fish Boost", "{0}", "Instant", "Gift a tapped Fish\nTarget creature gets +2/+0 until end of turn.", None),
        card("Shock", "{0}", "Instant", "Shock deals 2 damage to any target.", None),
        // a gift whose effect asks a question: the chosen player's draw, replaced by dredge
        card("Present", "{0}", "Instant", "Gift a card\nYou gain 3 life.", None),
        card("Small dredger", "{0}", "Creature", "Dredge 1", Some((1, 1))),
    ]
}

fn game() -> Game {
    let mut g = Game::new(defs(), 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, MOUNTAIN, Zone::Library); } }
    g
}

fn mana(g: &mut Game) {
    g.add_mana(0, ManaSymbol::Color(Color::Red));
    g.add_mana(0, ManaSymbol::Colorless);
}

fn resolve(g: &mut Game) {
    while !g.stack.is_empty() && g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
}

fn fish(g: &Game) -> Vec<ObjId> {
    g.battlefield.iter().copied().filter(|o| g.obj(*o).token() && g.obj(*o).chars.subtypes.iter().any(|s| s.name() == "Fish")).collect()
}

/// Brew in hand with a card to discard and a creature: (brew, fodder, creature).
fn setup(g: &mut Game) -> (ObjId, ObjId, ObjId) {
    let brew = g.put(0, BREW, Zone::Hand);
    let fodder = g.put(0, MOUNTAIN, Zone::Hand);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    mana(g);
    (brew, fodder, bears)
}

fn brew(card: ObjId, fodder: ObjId, gift: Option<usize>, targets: Vec<Vec<Target>>) -> Move {
    let cast = Move::WithCosts(vec![vec![fodder]], Box::new(Move::Cast { card, targets }));
    crate::support::paid("CR",match gift { Some(opponent) => Move::GiftTo(opponent, Box::new(cast)), None => cast })
}

#[test]
fn sazacaps_brew_is_its_oracle_card_and_unsupported_gifts_are_refused() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let real = CardDef::from_faces(catalog.get("Sazacap's Brew").unwrap()).unwrap();
    check(&real).unwrap();
    assert_eq!(real.abilities(), defs()[BREW].abilities());
    for d in defs() { check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0)); }
    // a permanent's gift is an enters trigger (702.174b); an extra turn is not given
    let refused = [
        CardDef::new("Gift creature", "{2}", "Creature — Bird", "Gift a card\nFlying", Some((1, 1))).unwrap(),
        CardDef::new("Gift turn", "{2}", "Instant", "Gift an extra turn\nYou gain 3 life.", None).unwrap(),
    ];
    for d in refused {
        let reason = if d.name()=="Gift creature" { "gift on a permanent: its enters trigger (702.174b)" } else { "keyword: Gift" };
        crate::support::refused(check(&d).map_err(|e|e.0),reason);
    }
}

#[test]
fn r702_174a_each_gift_announcement_requires_its_own_targets() {
    let mut g = game();
    let (card, fodder, bears) = setup(&mut g);
    cite!(["702.174m"]; ok: g.clone().act(0, brew(card, fodder, None, vec![vec![Target::Player(0)], vec![]])));
    cite!(["702.174m"]; ok: g.clone().act(0, brew(card, fodder, Some(1), vec![vec![Target::Player(0)], vec![Target::Object(bears)]])));
    cite!(["702.174a"]; refused("a gift is promised to an opponent"): unchanged(&mut g, |g| g.act(0, brew(card, fodder, Some(0), vec![vec![Target::Player(0)], vec![Target::Object(bears)]]))));
    g.destroy(bears);
    cite!(["702.174m"]; refused("there is no legal target for it (601.2c)"): unchanged(&mut g, |g| g.act(0, brew(card, fodder, Some(1), vec![vec![Target::Player(0)], vec![]]))));
}

#[test]
fn r702_174j_promised_the_opponent_gets_a_tapped_fish_before_the_rest_of_the_spell() {
    let mut g = game();
    let (brew_card, fodder, bears) = setup(&mut g);
    let hand = g.players[0].hand.len();
    g.act(0, brew(brew_card, fodder, Some(1), vec![vec![Target::Player(0)], vec![Target::Object(bears)]])).unwrap();
    // the discard is paid as it is cast
    assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).chars.name == "Mountain"));
    resolve(&mut g);
    let fish = fish(&g);
    assert_eq!(fish.len(), 1);
    let token = g.obj(fish[0]);
    cite!(["702.174f"]; assert!(token.tapped));
    cite!(["702.174f"]; assert_eq!((g.controller(fish[0]), g.power(fish[0]), token.chars.colors.clone()), (1, Some(1), vec![Color::Blue])));
    assert_eq!(g.players[0].hand.len(), hand - 2 + 2, "two drawn, the Brew and the discard gone");
    cite!(["702.174k"]; assert_eq!(g.power(bears), Some(4)));
    // the Fish was made before the cards were drawn (702.174j)
    let made = g.log.iter().position(|e| matches!(e, Event::Created { token } if fish.contains(token))).unwrap();
    let drawn = g.log.iter().position(|e| matches!(e, Event::Drew { .. })).unwrap();
    cite!(["702.174j"]; assert!(made < drawn));
}

#[test]
fn r702_174a_declined_no_fish_and_no_creature_target() {
    let mut g = game();
    let (brew_card, fodder, bears) = setup(&mut g);
    g.act(0, brew(brew_card, fodder, None, vec![vec![Target::Player(0)], vec![]])).unwrap();
    resolve(&mut g);
    cite!(["702.174b"]; assert!(fish(&g).is_empty()));
    cite!(["702.174k"]; assert_eq!(g.power(bears), Some(2)));
    assert_eq!(g.players[0].hand.len(), 2);
}

#[test]
fn r601_2c_an_illegal_gift_or_targets_or_cost_changes_nothing() {
    let mut g = game();
    let (brew_card, fodder, bears) = setup(&mut g);
    let deny = g.put(0, DENY, Zone::Hand);
    // A zero-cost Shock supplies a legal otherwise counterable spell for Deny.
    let shock = g.put(0, SHOCK, Zone::Hand);
    g.act(0, Move::Cast { card: shock, targets: vec![vec![Target::Player(1)]] }).unwrap();
    let counter_target = *g.stack.last().unwrap();
    let before = format!("{g:?}");
    let opponent = "a gift is promised to an opponent (702.174a)";
    for (mv, rule, why) in [
        // to its own controller
        (brew(brew_card, fodder, Some(0), vec![vec![Target::Player(0)], vec![Target::Object(bears)]]), "702.174a", opponent),
        // promised, without the creature it then targets
        (brew(brew_card, fodder, Some(1), vec![vec![Target::Player(0)], vec![]]), "702.174m", "wrong number of targets: 0 chosen, it takes 1 (601.2c)"),
        // not promised, with a creature it then does not target
        (brew(brew_card, fodder, None, vec![vec![Target::Player(0)], vec![Target::Object(bears)]]), "702.174m", "wrong number of targets: 1 chosen, it takes 0 (601.2c)"),
        // without the discard it costs
        (crate::support::paid("CR", Move::GiftTo(1, Box::new(Move::Cast { card: brew_card, targets: vec![vec![Target::Player(0)], vec![Target::Object(bears)]] }))), "601.2h",
            "that selection does not satisfy the cost, which asks for exactly 1 card(s): 0 chosen (601.2h, 701.9a)"),
        // a spell with no gift
        (Move::GiftTo(1, Box::new(Move::Cast { card: deny, targets: vec![vec![Target::Object(counter_target)]] })), "702.174a", "that spell has no gift to promise (702.174a)"),
        // said twice
        (Move::GiftTo(1, Box::new(brew(brew_card, fodder, Some(1), vec![vec![Target::Player(0)], vec![Target::Object(bears)]]))), "702.174a",
            "promise a spell's gift to one opponent, once (702.174a)"),
    ] {
        cite!([rule]; case: why; refused(why): g.act(0, mv.clone()));
        assert_eq!(format!("{g:?}"), before);
    }
    // to no player: a player is one of the people in the game (102.1), and this game of two
    // has no player 5, so the promise names no one; it is refused before the rules are asked
    let nobody = g.attempt(0, brew(brew_card, fodder, Some(5), vec![vec![Target::Player(0)], vec![Target::Object(bears)]]));
    cite!(["102.1"]; assert!(matches!(nobody, Err(game::play::ActionError::Unreferable(Target::Player(5)))), "{nobody:?}"));
    assert_eq!(format!("{g:?}"), before);
    // the discard is required: with nothing else in hand there is nothing to cast it with
    let mut g = game();
    let brew_card = g.put(0, BREW, Zone::Hand);
    g.put(0, BEARS, Zone::Battlefield);
    mana(&mut g);
    cite!(["601.2h"]; refused("that selection does not satisfy the cost, which asks for exactly 1 card(s): 0 chosen"): unchanged(&mut g, |g| g.act(0, crate::support::paid("CR", Move::Cast { card: brew_card, targets: vec![vec![Target::Player(0)], vec![]] }))));
}

#[test]
fn r702_174j_countered_or_fizzled_no_gift() {
    // countered: the gift's effect never happens
    let mut g = game();
    let (brew_card, fodder, bears) = setup(&mut g);
    g.act(0, brew(brew_card, fodder, Some(1), vec![vec![Target::Player(0)], vec![Target::Object(bears)]])).unwrap();
    let spell = *g.stack.last().unwrap();
    g.act(0, Move::Pass).unwrap();
    let deny = g.put(1, DENY, Zone::Hand);
    g.act(1, Move::Cast { card: deny, targets: vec![vec![Target::Object(spell)]] }).unwrap();
    resolve(&mut g);
    cite!(["702.174j"]; assert!(fish(&g).is_empty()));
    assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).chars.name == "Sazacap's Brew"));
    // every target gone: it does not resolve, and gives nothing (608.2b)
    let mut g = game();
    let boost = g.put(0, BOOST, Zone::Hand);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    g.act(0, Move::GiftTo(1, Box::new(Move::Cast { card: boost, targets: vec![vec![Target::Object(bears)]] }))).unwrap();
    let shock = g.put(0, SHOCK, Zone::Hand);
    g.act(0, Move::Cast { card: shock, targets: vec![vec![Target::Object(bears)]] }).unwrap();
    resolve(&mut g);
    assert!(!g.battlefield.contains(&bears));
    cite!(["608.2b", "702.174j"]; assert!(fish(&g).is_empty()));
    // a target left: it resolves, with its gift
    let mut g = game();
    let (brew_card, fodder, bears) = setup(&mut g);
    g.act(0, brew(brew_card, fodder, Some(1), vec![vec![Target::Player(0)], vec![Target::Object(bears)]])).unwrap();
    let shock = g.put(0, SHOCK, Zone::Hand);
    g.act(0, Move::Cast { card: shock, targets: vec![vec![Target::Object(bears)]] }).unwrap();
    resolve(&mut g);
    cite!(["608.2b"]; assert_eq!(fish(&g).len(), 1));
    assert_eq!(g.players[0].hand.len(), 2);
}

#[test]
fn r702_174j_a_gift_that_asks_a_question_suspends_the_spell_and_a_copy_continues_the_same() {
    let mut g = game();
    g.put(1, DREDGER, Zone::Graveyard);
    let present = g.put(0, PRESENT, Zone::Hand);
    g.act(0, Move::GiftTo(1, Box::new(Move::Cast { card: present, targets: vec![] }))).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    // the chosen player's draw may be replaced by dredge: they are asked, before the rest
    assert!(matches!(g.decision, Some(Decision::Choose { player: 1, choice: Choice::Replacement { optional: true, .. } })), "{:?}", g.decision);
    cite!(["702.174j"]; assert_eq!(g.players[0].life, 20, "the gift happens first (702.174j)"));
    assert!(matches!(g.asked(), Some((1, Asked::Replacement { .. }))));
    let mut copy = g.clone();
    for g in [&mut g, &mut copy] {
        g.act(1, Move::Choose(Answer::No)).unwrap();
    }
    assert_eq!(format!("{g:?}"), format!("{copy:?}"));
    assert_eq!(g.players[0].life, 23);
    cite!(["702.174e"]; assert_eq!(g.players[1].hand.len(), 1, "the opponent drew a card"));
}

#[track_caller]
fn unchanged(g: &mut Game, action: impl FnOnce(&mut Game) -> Result<(), String>) -> Result<(), String> {
    let before=format!("{g:?}");
    let result=action(g);
    assert_eq!(format!("{g:?}"),before);
    result
}
