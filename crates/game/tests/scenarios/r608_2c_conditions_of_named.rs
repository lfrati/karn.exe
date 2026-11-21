//! A condition said of something the text names (608.2c, 608.2h): "Counter target spell if
//! it's blue" asks of the target, as the spell resolves, whatever it was when chosen; "If the
//! discarded card wasn't a land card" asks of the card paid for this spell's own cost, as it
//! last was. Each is Oracle text as printed.
use crate::support::{cite, paid as spend};
use game::legal::Asked;
use game::state::Answer;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::mana::ManaSymbol;
use ir::Color;

fn card(name: &str, cost: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    let d = CardDef::new(name, cost, types, text, pt).unwrap_or_else(|e| panic!("{name}: {e}"));
    game::play::check(&d).unwrap_or_else(|u| panic!("{name}: {}", u.0));
    d
}

const PYROBLAST: usize = 0;
const GRAB: usize = 1;
const BLUE_BEAR: usize = 2;
const RED_BEAR: usize = 3;
const SHOCK: usize = 4;
const PAINT: usize = 5;
const ISLAND: usize = 6;
const LOOTING: usize = 7;
const ARISTOCRAT: usize = 8;
const HUMAN: usize = 9;
const ORACLE: usize = 10;
const STARFALL: usize = 11;
const CHARM: usize = 12;

fn game() -> Game {
    let defs = vec![
        card("Pyroblast", "{R}", "Instant", "Choose one —\n• Counter target spell if it's blue.\n• Destroy target permanent if it's blue.", None),
        card("Grab the Prize", "{1}{R}", "Sorcery", "As an additional cost to cast this spell, discard a card.\nDraw two cards. If the discarded card wasn't a land card, Grab the Prize deals 2 damage to each opponent.", None),
        card("Blue Bear", "{1}{U}", "Creature — Bear", "", Some((2, 2))),
        card("Red Bear", "{1}{R}", "Creature — Bear", "", Some((2, 2))),
        card("Shock", "{R}", "Instant", "Shock deals 2 damage to any target.", None),
        card("Paint Blue", "{0}", "Instant", "Target creature becomes blue until end of turn.", None),
        card("Island", "", "Basic Land — Island", "({T}: Add {U}.)", None),
        card("Faithless Looting", "{R}", "Sorcery", "Draw two cards, then discard two cards.", None),
        card("Falkenrath Aristocrat", "{2}{B}{R}", "Creature — Vampire Noble", "Flying, haste\nSacrifice a creature: This creature gains indestructible until end of turn. If the sacrificed creature was a Human, put a +1/+1 counter on this creature.", Some((4, 1))),
        card("Villager", "{W}", "Creature — Human", "", Some((1, 1))),
        card("Coiling Oracle", "{G}{U}", "Creature — Snake Elf Druid", "When this creature enters, reveal the top card of your library. If it's a land card, put it onto the battlefield. Otherwise, put that card into your hand.", Some((1, 1))),
        card("Starfall", "{4}{R}", "Instant", "Starfall deals 3 damage to target creature. If that creature is an enchantment, Starfall deals 3 damage to that creature's controller.", None),
        card("Charm Bear", "{1}{G}", "Enchantment Creature — Bear", "", Some((2, 2))),
    ];
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..6 { g.put(p, ISLAND, Zone::Library); } }
    g
}

fn mana(g: &mut Game, player: usize, symbols: &[ManaSymbol]) { for s in symbols { g.add_mana(player, *s); } }
const R: ManaSymbol = ManaSymbol::Color(Color::Red);
const U: ManaSymbol = ManaSymbol::Color(Color::Blue);
const C: ManaSymbol = ManaSymbol::Colorless;

fn resolve(g: &mut Game) {
    for _ in 0..40 {
        match g.asked() {
            Some((_, Asked::Priority)) if !g.stack.is_empty() => { g.act(g.priority, Move::Pass).unwrap(); }
            _ => return,
        }
    }
}

fn pyroblast(g: &mut Game, mode: usize, target: ObjId) -> Result<(), String> {
    mana(g, 0, &[R]);
    let card = g.put(0, PYROBLAST, Zone::Hand);
    g.act(0, spend("R", Move::Modes(vec![mode], Box::new(Move::Cast { card, targets: vec![vec![Target::Object(target)]] }))))
}

#[test]
fn r608_2c_pyroblast_targets_any_spell_and_counters_it_only_if_it_is_blue() {
    for (spell, countered, spending) in [(BLUE_BEAR, true, "CU"), (SHOCK, false, "R")] {
        let mut g = game();
        mana(&mut g, 0, &[U, R, C]);
        let card = g.put(0, spell, Zone::Hand);
        let targets = if spell == SHOCK { vec![vec![Target::Player(1)]] } else { vec![] };
        g.act(0, spend(spending, Move::Cast { card, targets })).unwrap();
        let on_stack = *g.stack.last().unwrap();
        pyroblast(&mut g, 0, on_stack).expect("any spell is a legal target");
        resolve(&mut g);
        let countered_events = g.log.iter().filter(|e| matches!(e, Event::Countered { .. })).count();
        cite!(["608.2c"]; case: &g.obj(card).chars.name; assert_eq!(countered_events == 1, countered, "{}", g.obj(card).chars.name));
        if spell == SHOCK { assert_eq!(g.players[1].life, 18, "the red spell resolves"); }
    }
}

#[test]
fn r608_2h_pyroblast_destroys_a_permanent_that_is_blue_as_it_resolves() {
    // blue when chosen; red when chosen; red when chosen and made blue before it resolves
    for (bear, paint, destroyed) in [(BLUE_BEAR, false, true), (RED_BEAR, false, false), (RED_BEAR, true, true)] {
        let mut g = game();
        let target = g.put(1, bear, Zone::Battlefield);
        pyroblast(&mut g, 1, target).expect("any permanent is a legal target");
        if paint {
            let card = g.put(0, PAINT, Zone::Hand);
            g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(target)]] }).unwrap();
        }
        resolve(&mut g);
        cite!(["608.2h"]; case: format!("{} painted {paint}", g.obj(target).chars.name); assert_eq!(g.obj(target).zone != Zone::Battlefield, destroyed, "{} painted {paint}", g.obj(target).chars.name));
    }
}

fn grab(g: &mut Game, discard: ObjId) {
    mana(g, 0, &[R, C]);
    let card = g.put(0, GRAB, Zone::Hand);
    g.act(0, spend("CR", Move::WithCosts(vec![vec![discard]], Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    resolve(g);
}

#[test]
fn r608_2h_grab_the_prize_asks_of_the_card_discarded_for_it() {
    for (discarded, damage) in [(ISLAND, false), (SHOCK, true)] {
        let mut g = game();
        let paid = g.put(0, discarded, Zone::Hand);
        let hand = g.players[0].hand.len();
        grab(&mut g, paid);
        assert_eq!(g.players[0].hand.len(), hand - 1 + 2, "discarded one, drew two");
        cite!(["608.2h"]; case: format!("damage={damage}"); assert_eq!(g.players[1].life, if damage { 18 } else { 20 }, "discarded {}", if damage { "Shock" } else { "Island" }));
    }
}

#[test]
fn r608_2h_grab_the_prize_asks_of_its_own_discard_not_an_earlier_one() {
    // a nonland card discarded earlier this turn does not make a land paid now count
    for (earlier, paid, damage) in [(SHOCK, ISLAND, false), (ISLAND, SHOCK, true)] {
        let mut g = game();
        let looting = g.put(0, LOOTING, Zone::Hand);
        let first = g.put(0, earlier, Zone::Hand);
        let second = g.put(0, earlier, Zone::Hand);
        mana(&mut g, 0, &[R]);
        g.act(0, spend("R", Move::Cast { card: looting, targets: vec![] })).unwrap();
        resolve(&mut g);
        let Some((0, Asked::Objects { .. })) = g.asked() else { panic!("Looting asks for its discards: {:?}", g.asked()) };
        g.act(0, Move::Choose(Answer::Objects(vec![first, second]))).unwrap();
        cite!(["404.3"]; ok: g.act(0,Move::Choose(Answer::Objects(vec![first,second]))));
        resolve(&mut g);
        let pay = g.put(0, paid, Zone::Hand);
        grab(&mut g, pay);
        cite!(["608.2h"]; case: format!("damage={damage}"); assert_eq!(g.players[1].life, if damage { 18 } else { 20 }));
    }
}

#[test]
fn r608_2h_the_sacrificed_creature_is_asked_of_as_it_last_was() {
    for (fodder, counter) in [(HUMAN, true), (RED_BEAR, false)] {
        let mut g = game();
        let aristocrat = g.put(0, ARISTOCRAT, Zone::Battlefield);
        let model = g.put(0, fodder, Zone::Hand);
        let sacrificed = g.create_token(0, g.obj(model).chars.clone(), false);
        let ability = g.abilities(aristocrat).iter().position(|a| matches!(&**a, ir::Ability::Activated { .. })).expect("its activated ability");
        g.act(0, Move::Activate { source: aristocrat, ability, targets: vec![], paying: vec![vec![sacrificed]] }).unwrap();
        resolve(&mut g);
        cite!(["608.2h"]; case: format!("counter={counter}"); assert_eq!(g.obj(aristocrat).counters.get("+1/+1").copied().unwrap_or(0), u32::from(counter)));
    }
}

#[test]
fn r608_2c_it_is_the_card_revealed_and_that_creature_the_target() {
    // Coiling Oracle: the top card is a land (put onto the battlefield) or not (into hand)
    let mut g = game();
    mana(&mut g, 0, &[ManaSymbol::Color(Color::Green), U]);
    let oracle = g.put(0, ORACLE, Zone::Hand);
    let lands = g.battlefield.len();
    g.act(0, spend("GU", Move::Cast { card: oracle, targets: vec![] })).unwrap();
    resolve(&mut g);
    cite!(["608.2c"]; assert_eq!(g.battlefield.len(), lands + 2, "the Oracle and the Island it revealed"));
    // Starfall: the target is an enchantment, or not
    for (bear, controller_hit) in [(BLUE_BEAR, false), (CHARM_BEAR, true)] {
        let mut g = game();
        let target = g.put(1, bear, Zone::Battlefield);
        mana(&mut g, 0, &[R, C, C, C, C]);
        let card = g.put(0, STARFALL, Zone::Hand);
        g.act(0, spend("CCCCR", Move::Cast { card, targets: vec![vec![Target::Object(target)]] })).unwrap();
        resolve(&mut g);
        cite!(["608.2c"]; case: format!("controller_hit={controller_hit}"); assert_eq!(g.players[1].life, if controller_hit { 17 } else { 20 }));
    }
}
const CHARM_BEAR: usize = CHARM;

#[test]
fn r608_2h_only_a_paid_object_is_asked_of_in_the_past() {
    // what a target was is history the engine does not keep: still refused, not guessed
    let d = CardDef::new("Probe", "{1}{B}", "Instant", "Destroy target creature. If it was a Human, you gain 2 life.", None).unwrap();
    let refused = game::play::check(&d).expect_err("a past condition of a target");
    assert!(refused.0.contains("past condition"), "{}", refused.0);
}
