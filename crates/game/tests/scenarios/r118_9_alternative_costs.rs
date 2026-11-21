//! An alternative cost a card prints (CR 118.9, 601.2b, 601.2f-h): "You may sacrifice two
//! Mountains rather than pay this spell's mana cost." Which cost the spell is cast for is
//! chosen as it is announced, listed as an option of its own, said in the move, and paid
//! with the ordinary cost plan. Every card is its printed Oracle text unless named otherwise.
use crate::support::{cite, paid};
use game::announcement::CastCost;
use game::card::CardDef;
use game::play::check;
use game::{Game, Move, ObjId, Target, Zone};
use ir::mana::ManaSymbol;
use ir::Color;

const FIREBLAST: usize = 0;
const MOUNTAIN: usize = 1;
const TAIGA: usize = 2;
const FOREST: usize = 3;
const DENY: usize = 4;
const TAX: usize = 5;
const DISCOUNT: usize = 6;
const HASTY: usize = 7;
const KICKED: usize = 8;
const BEARS: usize = 9;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Fireblast", "{4}{R}{R}", "Instant", "You may sacrifice two Mountains rather than pay this spell's mana cost.\nFireblast deals 4 damage to any target.", None),
        card("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None),
        card("Taiga", "", "Land — Mountain Forest", "({T}: Add {R} or {G}.)", None),
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        card("Deny", "{0}", "Instant", "Counter target spell.", None),
        // cost changes apply to the cost the spell is cast for, alternative or not (601.2f)
        card("Tax", "{0}", "Enchantment", "Noncreature spells cost {1} more to cast.", None),
        card("Discount", "{0}", "Enchantment", "Instant spells you cast cost {1} less to cast.", None),
        // test cards: an alternative cost beside an additional cost, and beside kicker (118.9d)
        card("Hasty Blast", "{2}{R}", "Instant", "As an additional cost to cast this spell, discard a card.\nYou may sacrifice a Mountain rather than pay this spell's mana cost.\nThis spell deals 2 damage to any target.", None),
        card("Kicked Blast", "{2}{R}", "Instant", "Kicker {1}\nYou may sacrifice a Mountain rather than pay this spell's mana cost.\nThis spell deals 2 damage to any target. If this spell was kicked, it deals 4 damage instead.", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
    ]
}

/// The alternative cost's row: the first row of the card's text.
const SACRIFICE: CastCost = CastCost::Alternative(0);

fn game() -> Game {
    Game::new(defs(), 2, 20)
}

fn resolve(g: &mut Game) {
    for _ in 0..20 {
        if g.stack.is_empty() { return; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("stack did not resolve within 20 passes");
}

fn alternative(g: &mut Game, card: ObjId, sacrificed: &[ObjId], target: Target, spending: &str) -> Result<(), String> {
    g.act(0, paid(spending, Move::CastFor(SACRIFICE, Box::new(Move::WithCosts(vec![sacrificed.to_vec()], Box::new(Move::Cast { card, targets: vec![vec![target]] }))))))
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    // A conditional alternative uses the shared predicate and cost plan.
    let conditional = CardDef::new("Probe", "{2}{R}", "Instant", "If you control a Mountain, you may sacrifice a Mountain rather than pay this spell's mana cost.\nThis spell deals 2 damage to any target.", None).unwrap();
    check(&conditional).unwrap();
}

#[test]
fn r118_9_supplied_mana_and_sacrifice_costs_use_their_own_resources() {
    for (mana, mountains, nonbasic) in [(6, 2, false), (0, 2, true), (0, 1, false)] {
        let mut g = game();
        let blast = g.put(0, FIREBLAST, Zone::Hand);
        let resources: Vec<_> = (0..mountains).map(|i| g.put(0, if nonbasic && i == 1 { TAIGA } else { MOUNTAIN }, Zone::Battlefield)).collect();
        g.players[0].pool.colors[Color::Red as usize] = mana;
        let mut normal = g.clone();
        cite!(["118.9", "601.2b"]; case: format!("mana={mana}, mountains={mountains}"); assert_eq!(normal.act(0, paid("RRRRRR", Move::Cast { card: blast, targets: vec![vec![Target::Player(1)]] })).is_ok(), mana == 6));
        cite!(["118.9", "205.3i", "118.3"]; case: format!("mana={mana}, mountains={mountains}"); assert_eq!(alternative(&mut g, blast, &resources, Target::Player(1), "").is_ok(), mountains == 2));
    }
}

#[test]
fn r601_2h_the_mountains_are_sacrificed_as_it_is_cast_and_it_goes_to_the_graveyard() {
    for second in [MOUNTAIN, TAIGA] {
        let mut g = game();
        let blast = g.put(0, FIREBLAST, Zone::Hand);
        let a = g.put(0, MOUNTAIN, Zone::Battlefield);
        let b = g.put(0, second, Zone::Battlefield);
        alternative(&mut g, blast, &[a, b], Target::Player(1), "").expect("cast for the alternative cost");
        // CR404.3: explicit owner order for these cost/departure identities.
        assert_eq!(g.decision,Some(game::state::Decision::Choose{player:0,choice:game::state::Choice::OrderCards{to:game::state::Placement::Zone(Zone::Graveyard),cards:vec![a,b]}}));
        g.act(0,Move::Choose(game::state::Answer::Objects(vec![b,a]))).unwrap();
        let spell=*g.stack.last().unwrap();
        let audit=g.audit_game_objects();
        let alternative=&audit["objects"][spell.0.to_string()]["announced"]["costs"]["alternative"];
        cite!(["118.9", "601.2b"]; assert_eq!(alternative["paragraph"], 1));
        assert_eq!(alternative["source"], spell.0);
        assert_eq!(alternative["definition"], "Fireblast");
        // paid as it is cast, before it resolves (601.2h)
        cite!(["601.2h"]; assert!(!g.battlefield.contains(&a) && !g.battlefield.contains(&b)));
        assert_eq!(g.stack.len(), 1);
        assert_eq!(g.players[0].life, 20);
        resolve(&mut g);
        cite!(["118.9"]; case: format!("{second}"); assert_eq!(g.players[1].life, 16));
        // an alternative cost changes nothing of where it goes (unlike flashback, 702.34a)
        cite!(["118.9"]; case: format!("{second}"); assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).chars.name == "Fireblast")));
    }
}

#[test]
fn r601_2h_a_payment_or_target_refused_changes_nothing() {
    let mut g = game();
    let blast = g.put(0, FIREBLAST, Zone::Hand);
    let a = g.put(0, MOUNTAIN, Zone::Battlefield);
    let b = g.put(0, MOUNTAIN, Zone::Battlefield);
    let forest = g.put(0, FOREST, Zone::Battlefield);
    let land = g.put(1, FOREST, Zone::Battlefield);
    let before = (g.players[0].hand.clone(), g.battlefield.clone(), g.stack.clone(), g.players[0].graveyard.clone());
    for (paid, target, rules, why) in [
        (vec![a, forest], Target::Player(1), ["601.2h"], "Forest is not what that cost asks for"),   // a Forest is not a Mountain
        (vec![a], Target::Player(1), ["601.2h"], "which asks for exactly 2 permanent(s): 1 chosen"),     // one is not two
        (vec![a, a], Target::Player(1), ["601.2h"], "a card is chosen once for that cost (601.2h)"), // the same one twice
        (vec![a, b], Target::Object(land), ["115.1"], "an illegal target, Forest"),                 // a land is no legal target
    ] {
        cite!([rules[0]]; case: format!("{paid:?} {target:?}"); refused(why): alternative(&mut g, blast, &paid, target, ""));
        assert_eq!((g.players[0].hand.clone(), g.battlefield.clone(), g.stack.clone(), g.players[0].graveyard.clone()), before);
    }
    // the alternative, said without sacrificing anything, is refused too
    cite!(["601.2h"]; refused("which asks for exactly 2 permanent(s): 0 chosen"): g.act(0, Move::CastFor(SACRIFICE, Box::new(Move::Cast { card: blast, targets: vec![vec![Target::Player(1)]] }))));
    // a row that is no alternative cost of it
    cite!(["118.9"]; refused("that card prints no such alternative cost (118.9)"): g.act(0, Move::CastFor(CastCost::Alternative(1), Box::new(Move::WithCosts(vec![vec![a, b]], Box::new(Move::Cast { card: blast, targets: vec![vec![Target::Player(1)]] }))))));
    assert_eq!((g.players[0].hand.clone(), g.battlefield.clone(), g.stack.clone(), g.players[0].graveyard.clone()), before);
}

#[test]
fn r601_2f_cost_increases_and_reductions_apply_to_the_alternative_cost() {
    // "{1} more": the alternative costs {1} besides the Mountains
    let mut g = game();
    g.put(0, TAX, Zone::Battlefield);
    let blast = g.put(0, FIREBLAST, Zone::Hand);
    let a = g.put(0, MOUNTAIN, Zone::Battlefield);
    let b = g.put(0, MOUNTAIN, Zone::Battlefield);
    cite!(["601.2f"]; refused("that mana is not in your mana pool (118.3a)"): alternative(&mut g, blast, &[a, b], Target::Player(1), "C")); // the {1} is unpaid
    assert!(g.battlefield.contains(&a));
    g.add_mana(0, ManaSymbol::Colorless);
    cite!(["118.9d", "601.2f"]; ok: alternative(&mut g, blast, &[a, b], Target::Player(1), "C"));
    // CR404.3: explicit owner order for these cost/departure identities.
    assert_eq!(g.decision,Some(game::state::Decision::Choose{player:0,choice:game::state::Choice::OrderCards{to:game::state::Placement::Zone(Zone::Graveyard),cards:vec![a,b]}}));
    g.act(0,Move::Choose(game::state::Answer::Objects(vec![b,a]))).unwrap();
    assert_eq!(g.players[0].pool.total(), 0);
    resolve(&mut g);
    assert_eq!(g.players[1].life, 16);
    // "{1} less": nothing generic to reduce; the Mountains are still sacrificed
    let mut g = game();
    g.put(0, DISCOUNT, Zone::Battlefield);
    let blast = g.put(0, FIREBLAST, Zone::Hand);
    let a = g.put(0, MOUNTAIN, Zone::Battlefield);
    let b = g.put(0, MOUNTAIN, Zone::Battlefield);
    cite!(["118.9d", "601.2f"]; ok: alternative(&mut g, blast, &[a, b], Target::Player(1), ""));
    // CR404.3: explicit owner order for these cost/departure identities.
    assert_eq!(g.decision,Some(game::state::Decision::Choose{player:0,choice:game::state::Choice::OrderCards{to:game::state::Placement::Zone(Zone::Graveyard),cards:vec![a,b]}}));
    g.act(0,Move::Choose(game::state::Answer::Objects(vec![b,a]))).unwrap();
    assert!(!g.battlefield.contains(&a) && !g.battlefield.contains(&b));
}

#[test]
fn r118_9d_additional_costs_and_kicker_are_paid_with_the_alternative_cost() {
    // the alternative cost is named by the row stating it: here the second, after the
    // additional cost (or the kicker)
    let second = CastCost::Alternative(1);
    // an additional cost: the discard and the sacrifice, each its own choice
    let mut g = game();
    let blast = g.put(0, HASTY, Zone::Hand);
    let discarded = g.put(0, BEARS, Zone::Hand);
    let mountain = g.put(0, MOUNTAIN, Zone::Battlefield);
    let paying = |a: ObjId, b: ObjId| vec![vec![a], vec![b]];
    let cast = |g: &mut Game, groups: Vec<Vec<ObjId>>| g.act(0, Move::CastFor(second, Box::new(Move::WithCosts(groups, Box::new(Move::Cast { card: blast, targets: vec![vec![Target::Player(1)]] })))));
    // The command groups the alternative-cost resources before additional costs.
    cast(&mut g, paying(mountain, discarded)).expect("both costs paid");
    assert!(!g.battlefield.contains(&mountain));
    cite!(["118.9d"]; assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).chars.name == "Grizzly Bears")));
    resolve(&mut g);
    assert_eq!(g.players[1].life, 18);
    // kicker with an alternative cost: the kicker {1} is paid besides the Mountain
    let mut g = game();
    let blast = g.put(0, KICKED, Zone::Hand);
    let mountain = g.put(0, MOUNTAIN, Zone::Battlefield);
    g.add_mana(0, ManaSymbol::Colorless);
    g.act(0, paid("C", Move::CastFor(second, Box::new(Move::WithKicks(1, Box::new(Move::WithCosts(vec![vec![mountain]], Box::new(Move::Cast { card: blast, targets: vec![vec![Target::Player(1)]] })))))))).expect("kicked for its alternative cost");
    cite!(["118.9d"]; assert_eq!(g.players[0].pool.total(), 0));
    resolve(&mut g);
    cite!(["118.9d", "702.33d"]; assert_eq!(g.players[1].life, 16));
}

#[test]
fn r118_9_foil_discards_an_island_card_and_another_card_two_distinct_cards() {
    // "discard an Island card and another card": two choices, each its own card (118.10); the
    // second may be any card but the first, and the Island card is a card with that subtype
    let mut defs = defs();
    defs.push(CardDef::new("Foil", "{2}{U}{U}", "Instant", "You may discard an Island card and another card rather than pay this spell's mana cost.\nCounter target spell.", None).unwrap());
    defs.push(CardDef::new("Island", "", "Basic Land — Island", "({T}: Add {U}.)", None).unwrap());
    let (foil, island_def) = (defs.len() - 2, defs.len() - 1);
    check(&defs[foil]).unwrap();
    // a creature spell of its own on the stack to counter; Foil, an Island card and a Mountain
    // card in hand
    let setup = || {
        let mut g = Game::new(defs.clone(), 2, 20);
        let bears = g.put(0, BEARS, Zone::Hand);
        g.add_mana(0, ManaSymbol::Color(Color::Green));
        g.add_mana(0, ManaSymbol::Colorless);
        g.act(0, paid("CG", Move::Cast { card: bears, targets: vec![] })).unwrap();
        let spell = *g.stack.last().unwrap();
        let card = g.put(0, foil, Zone::Hand);
        let island = g.put(0, island_def, Zone::Hand);
        let other = g.put(0, MOUNTAIN, Zone::Hand);
        (g, card, spell, island, other)
    };
    let cast = |g: &mut Game, card: ObjId, spell: ObjId, groups: Vec<Vec<ObjId>>| g.act(0, Move::CastFor(SACRIFICE, Box::new(Move::WithCosts(groups,
        Box::new(Move::Cast { card, targets: vec![vec![Target::Object(spell)]] })))));
    // listed for its alternative cost
    // the Island card for both, and two non-Island cards (the Mountain twice): refused, nothing done
    for same_island in [true, false] {
        let (mut g, card, spell, island, other) = setup();
        let paid = if same_island { island } else { other };
        let before = (g.players[0].hand.clone(), g.stack.clone());
        // the Island card cannot pay both parts (118.10); a Mountain is no Island card
        let why = if same_island { "Island pays for one thing (118.10)" } else { "Mountain is not what that cost asks for" };
        cite!([if same_island { "118.10" } else { "601.2h" }]; refused(why): cast(&mut g, card, spell, vec![vec![paid], vec![paid]]));
        assert_eq!((g.players[0].hand.clone(), g.stack.clone()), before);
    }
    // the Mountain where the Island card is asked for: refused
    let (mut g, card, spell, island, other) = setup();
    cite!(["601.2h"]; refused("Mountain is not what that cost asks for"): cast(&mut g, card, spell, vec![vec![other], vec![island]]));
    // the Island card and another: both discarded, the spell countered
    cast(&mut g, card, spell, vec![vec![island], vec![other]]).expect("an Island card and another card");
    // CR404.3: explicit owner order for these cost/departure identities.
    assert_eq!(g.decision,Some(game::state::Decision::Choose{player:0,choice:game::state::Choice::OrderCards{to:game::state::Placement::Zone(Zone::Graveyard),cards:vec![island,other]}}));
    g.act(0,Move::Choose(game::state::Answer::Objects(vec![other,island]))).unwrap();
    cite!(["118.10"]; assert!(g.players[0].hand.iter().all(|o| *o != island && *o != other)));
    let graveyard: Vec<_> = g.players[0].graveyard.iter().map(|o| g.obj(*o).chars.name.clone()).collect();
    assert!(graveyard.contains(&"Island".to_string()) && graveyard.contains(&"Mountain".to_string()), "{graveyard:?}");
    resolve(&mut g);
    assert!(g.battlefield.iter().all(|o| g.obj(*o).chars.name != "Grizzly Bears"), "countered");
}

#[test]
fn r701_6_countered_it_goes_to_its_owners_graveyard() {
    let mut g = game();
    let blast = g.put(0, FIREBLAST, Zone::Hand);
    let a = g.put(0, MOUNTAIN, Zone::Battlefield);
    let b = g.put(0, MOUNTAIN, Zone::Battlefield);
    alternative(&mut g, blast, &[a, b], Target::Player(1), "").unwrap();
    // CR404.3: explicit owner order for these cost/departure identities.
    assert_eq!(g.decision,Some(game::state::Decision::Choose{player:0,choice:game::state::Choice::OrderCards{to:game::state::Placement::Zone(Zone::Graveyard),cards:vec![a,b]}}));
    g.act(0,Move::Choose(game::state::Answer::Objects(vec![b,a]))).unwrap();
    let spell = *g.stack.last().unwrap();
    g.act(0, Move::Pass).unwrap();
    let deny = g.put(1, DENY, Zone::Hand);
    g.act(1, Move::Cast { card: deny, targets: vec![vec![Target::Object(spell)]] }).unwrap();
    resolve(&mut g);
    assert_eq!(g.players[1].life, 20);
    cite!(["701.6a"]; assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).chars.name == "Fireblast")));
    assert!(g.exile.is_empty());
}
