//! Activated abilities (CR 602), mana abilities (605) and costs (118), rule by rule. Every
//! card is read from its printed text.

use crate::support::{cite, mana_pool, paid};
use game::card::CardDef;
use game::play::check;
use game::state::{Answer, Choice, Decision, Pool};
use game::{Event, Game, Move, ObjId, Target, Zone};
use ir::mana::ManaSymbol;
use ir::{Color, Part, Sort};

const FOREST: usize = 0;
const BEARS: usize = 1;
const PYROMANCER: usize = 2;
const ELVES: usize = 3;
const SOL_RING: usize = 4;
const GREED: usize = 5;
const SWAMP: usize = 6;
const RUSALKA: usize = 7;
const RIDER: usize = 8;
const TRANSLATOR: usize = 9;
const WITNESS: usize = 10;
const PLAINS: usize = 11;
const ALTAR: usize = 12;
const EGG: usize = 13;
const OASIS: usize = 14;
const BIRDS: usize = 15;
const SENTINEL: usize = 16;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Prodigal Pyromancer", "{2}{R}", "Creature — Human Wizard", "{T}: This creature deals 1 damage to any target.", Some((1, 1))),
        card("Llanowar Elves", "{G}", "Creature — Elf Druid", "{T}: Add {G}.", Some((1, 1))),
        card("Sol Ring", "{1}", "Artifact", "{T}: Add {C}{C}.", None),
        card("Greed", "{3}{B}", "Enchantment", "{B}, Pay 2 life: Draw a card.", None),
        card("Swamp", "", "Basic Land — Swamp", "({T}: Add {B}.)", None),
        card("Starved Rusalka", "{G}", "Creature — Spirit", "{G}, Sacrifice a creature: You gain 1 life.", Some((1, 1))),
        card(
            "Westfold Rider",
            "{1}{W}",
            "Creature — Human Knight",
            "Sacrifice this creature: Destroy target artifact or enchantment. Activate only as a sorcery.",
            Some((3, 1)),
        ),
        card(
            "Kozilek's Translator",
            "{4}{B}",
            "Creature — Eldrazi Drone",
            "Devoid (This card has no color.)\nPay 1 life: Add {C}. Activate only once each turn.",
            Some((3, 5)),
        ),
        card(
            "Devout Witness",
            "{2}{W}",
            "Creature — Human Spellshaper",
            "{1}{W}, {T}, Discard a card: Destroy target artifact or enchantment.",
            Some((2, 2)),
        ),
        card("Plains", "", "Basic Land — Plains", "({T}: Add {W}.)", None),
        card("Ashnod's Altar", "{3}", "Artifact", "Sacrifice a creature: Add {C}{C}.", None),
        card(
            "Darkwater Egg",
            "{1}",
            "Artifact",
            "{2}, {T}, Sacrifice this artifact: Add {U}{B}. Draw a card. (Activate only as an instant.)",
            None,
        ),
        card("Shivan Oasis", "", "Land", "This land enters tapped.\n{T}: Add {R} or {G}.", None),
        card("Birds of Paradise", "{G}", "Creature — Bird", "Flying\n{T}: Add one mana of any color.", Some((0, 1))),
        card("Quirion Sentinel", "{1}{G}", "Creature — Elf Druid", "When this creature enters, add one mana of any color.", Some((2, 1))),
    ]
}

fn game() -> Game {
    Game::new(defs(), 2, 20)
}

fn object(o: ObjId) -> Target {
    Target::Object(o)
}

fn all_pass(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}

/// Activate ability zero with the caller's targets, nonmana choices and exact mana selection.
fn activate(g: &mut Game, player: usize, source: ObjId, targets: Vec<Vec<Target>>, paying: Vec<Vec<ObjId>>, with: Pool) -> Result<(), String> {
    g.act(player, Move::Paying(with, Box::new(Move::Activate { source, ability: 0, targets, paying })))
}

fn green(g: &Game, player: usize) -> u32 {
    g.players[player].pool.colors[Color::Green as usize]
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        // devoid is not given its meaning yet; the card is put onto the battlefield here
        if d.name() != "Kozilek's Translator" {
            check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
        }
    }
}

#[test]
fn r602_2_activating_puts_the_ability_on_the_stack_and_pays_its_cost() {
    let mut g = game();
    let pyromancer = g.put(0, PYROMANCER, Zone::Battlefield);
    activate(&mut g, 0, pyromancer, vec![vec![Target::Player(1)]], vec![], mana_pool("")).unwrap();
    // the cost is paid, and the ability is on the stack as an object that is not a card
    cite!(["602.2"]; assert!(g.obj(pyromancer).tapped));
    let [ability] = g.stack.as_slice() else { panic!("one object on the stack") };
    cite!(["602.2a"]; assert!(g.is(object(*ability), Sort::Ability)));
    assert!(g.log.contains(&Event::Activated { source: pyromancer, by: 0, ability: Some(*ability), keyword: None }));
    // nothing has happened yet, and its controller has priority again (117.3c)
    cite!(["117.3c"]; assert_eq!((g.players[1].life, g.priority), (20, 0)));
    all_pass(&mut g);
    assert_eq!(g.players[1].life, 19);
    assert!(g.stack.is_empty());
    // {T} can't be paid by a permanent that is already tapped (107.5)
    cite!(["107.5"]; refused("Prodigal Pyromancer can't pay {T} as it is (107.5, 118.3)"): activate(&mut g, 0, pyromancer, vec![vec![Target::Player(1)]], vec![], mana_pool("")));
}

#[test]
fn r602_2_only_its_controller_activates_an_objects_ability() {
    let mut g = game();
    let pyromancer = g.put(0, PYROMANCER, Zone::Battlefield);
    let in_hand = g.put(0, PYROMANCER, Zone::Hand);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    // an ability functions on the battlefield (113.6)
    cite!(["113.6"]; refused("an ability of a permanent on the battlefield is activated (113.6)"): activate(&mut g, 0, in_hand, vec![vec![Target::Player(1)]], vec![], mana_pool("")));
    // only an activated ability is activated (602.1c)
    cite!(["602.1c"]; refused("that is not an activated ability of that permanent (602.1c)"): activate(&mut g, 0, bears, vec![], vec![], mana_pool("")));
    // its targets are chosen as it is activated (601.2c)
    cite!(["601.2c", "602.2b"]; refused("that has 1 target group(s), 0 given"): activate(&mut g, 0, pyromancer, vec![], vec![], mana_pool("")));
    g.act(0, Move::Pass).unwrap();
    cite!(["602.2"]; refused("only an object's controller can activate its ability (602.2)"): activate(&mut g, 1, pyromancer, vec![vec![Target::Player(0)]], vec![], mana_pool("")));
    assert!(g.stack.is_empty() && !g.obj(pyromancer).tapped, "nothing has happened");
}

#[test]
fn r602_5a_a_creatures_tap_ability_waits_until_it_has_been_its_controllers_since_their_turn_began() {
    let mut g = game();
    let forests: Vec<ObjId> = (0..2).map(|_| g.put(0, FOREST, Zone::Battlefield)).collect();
    let (elves, ring) = (g.put(0, ELVES, Zone::Hand), g.put(0, SOL_RING, Zone::Hand));
    for (card, forest) in [(elves, forests[0]), (ring, forests[1])] {
        g.act(0, Move::Activate { source: forest, ability: 0, targets: vec![], paying: vec![] }).unwrap();
        g.act(0, paid("G", Move::Cast { card, targets: vec![] })).unwrap();
        all_pass(&mut g);
    }
    let [.., elves, ring] = *g.battlefield.as_slice() else { panic!("both arrived") };
    cite!(["602.5a"]; refused("Llanowar Elves has not been under your control since your turn began (602.5a)"): g.act(0, Move::Activate { source: elves, ability: 0, targets: vec![], paying: vec![] }));
    // the rule is a creature's: an artifact that came this turn pays {T}
    g.act(0, Move::Activate { source: ring, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    cite!(["602.5a"]; assert_eq!(g.players[0].pool.colorless, 2));
    // a creature that was there as the turn began does too
    let old = g.put(0, ELVES, Zone::Battlefield);
    g.act(0, Move::Activate { source: old, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    cite!(["602.5a"]; assert_eq!(green(&g, 0), 1));
}

#[test]
fn r605_3b_a_mana_ability_resolves_at_once_and_does_not_use_the_stack() {
    let mut g = game();
    let elves = g.put(0, ELVES, Zone::Battlefield);
    activate(&mut g, 0, elves, vec![], vec![], mana_pool("")).unwrap();
    cite!(["605.3b"]; assert_eq!((green(&g, 0), g.stack.len()), (1, 0)));
    assert!(g.log.contains(&Event::Activated { source: elves, by: 0, ability: None, keyword: None }));
    // one that has no {T} in its cost, and asks for a choice to pay it
    let altar = g.put(0, ALTAR, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    activate(&mut g, 0, altar, vec![], vec![vec![bears]], mana_pool("")).unwrap();
    cite!(["605.3b"]; assert_eq!((g.players[0].pool.colorless, g.stack.len()), (2, 0)));
    assert!(g.log.contains(&Event::Sacrificed { player: 0, object: bears }));

}

#[test]
fn r605_1a_an_ability_that_moves_a_card_from_a_library_is_not_a_mana_ability() {
    // "{2}, {T}, Sacrifice this artifact: Add {U}{B}. Draw a card.": to draw is to put the top
    // card of a library into a hand (121.1), so the ability uses the stack and its mana is
    // added when it resolves
    let mut g = game();
    let egg = g.put(0, EGG, Zone::Battlefield);
    g.put(0, BEARS, Zone::Library);
    for _ in 0..2 {
        let forest = g.put(0, FOREST, Zone::Battlefield);
        g.act(0, Move::Activate { source: forest, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
    activate(&mut g, 0, egg, vec![], vec![], mana_pool("GG")).unwrap();
    cite!(["605.1a"]; assert_eq!((g.stack.len(), g.players[0].pool.total(), g.players[0].hand.len()), (1, 0, 0), "on the stack, nothing added or drawn yet"));
    all_pass(&mut g);
    let pool = &g.players[0].pool;
    assert_eq!((pool.colors[Color::Blue as usize], pool.colors[Color::Black as usize], g.players[0].hand.len()), (1, 1, 1));
}

/// One mana of this color.
fn one(color: Color) -> Vec<ManaSymbol> {
    vec![ManaSymbol::Color(color)]
}

#[test]
fn r605_3b_the_mana_a_mana_ability_adds_is_chosen_as_it_is_activated() {
    // "{T}: Add {R} or {G}.": the ability resolves as it is activated, so which of the two is
    // said with the activation, and is one of the two
    let mut g = game();
    let oasis = g.put(0, OASIS, Zone::Battlefield);
    // (its first ability is "This land enters tapped.")
    let tap = Move::Activate { source: oasis, ability: 1, targets: vec![], paying: vec![] };
    cite!(["608.2d"]; refused("that ability offers a choice of mana: say which mana it adds (608.2d)"): g.act(0, tap.clone()));
    cite!(["608.2d"]; refused("that ability doesn't offer the mana said for it: say one of what it offers (608.2d)"): g.act(0, Move::Adding(vec![one(Color::White)], Box::new(tap.clone()))));
    assert!(!g.obj(oasis).tapped, "nothing was paid");
    g.act(0, Move::Adding(vec![one(Color::Green)], Box::new(tap))).unwrap();
    cite!(["605.3b"]; assert_eq!((green(&g, 0), g.players[0].pool.total(), g.stack.len()), (1, 1, 0)));
    // an ability that offers no choice is not told one
    let mut g = game();
    let elves = g.put(0, ELVES, Zone::Battlefield);
    let tap = Move::Activate { source: elves, ability: 0, targets: vec![], paying: vec![] };
    cite!(["608.2d"]; refused("that ability offers no such choice of mana (608.2d)"): g.act(0, Move::Adding(vec![one(Color::Green)], Box::new(tap))));
}

#[test]
fn r106_1a_one_mana_of_any_color_is_one_of_the_five_colors_of_mana() {
    // "{T}: Add one mana of any color.": white, blue, black, red or green, and not colorless,
    // which is a type of mana and no color (106.1b)
    for color in [Color::White, Color::Blue, Color::Black, Color::Red, Color::Green] {
        let mut g = game();
        let birds = g.put(0, BIRDS, Zone::Battlefield);
        let tap = Move::Activate { source: birds, ability: 1, targets: vec![], paying: vec![] };
        g.act(0, Move::Adding(vec![one(color)], Box::new(tap))).unwrap();
        cite!(["106.1a"]; case: format!("{color:?}"); assert_eq!(g.players[0].pool.colors[color as usize], 1));
    }
    let mut g = game();
    let birds = g.put(0, BIRDS, Zone::Battlefield);
    let tap = Move::Activate { source: birds, ability: 1, targets: vec![], paying: vec![] };
    cite!(["106.1a"]; refused("that ability doesn't offer the mana said for it: say one of what it offers (608.2d)"): g.act(0, Move::Adding(vec![vec![ManaSymbol::Colorless]], Box::new(tap))));
}

#[test]
fn r608_2d_a_choice_of_mana_is_made_while_the_effect_is_applied() {
    // "When this creature enters, add one mana of any color.": a triggered ability on the
    // stack, which waits for the choice as it resolves
    let mut g = game();
    for _ in 0..2 {
        let forest = g.put(0, FOREST, Zone::Battlefield);
        g.act(0, Move::Activate { source: forest, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
    let sentinel = g.put(0, SENTINEL, Zone::Hand);
    g.act(0, paid("GG", Move::Cast { card: sentinel, targets: vec![] })).unwrap();
    all_pass(&mut g);
    assert_eq!(g.stack.len(), 1, "its ability triggered");
    all_pass(&mut g);
    let options: Vec<Vec<ManaSymbol>> = [Color::White, Color::Blue, Color::Black, Color::Red, Color::Green].map(one).to_vec();
    cite!(["608.2d"]; assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::Mana { options } })));
    cite!(["608.2d"]; refused("the choice is which of the mana offered to add (608.2d)"): g.act(0, Move::Choose(Answer::Mana(vec![ManaSymbol::Colorless]))));
    g.act(0, Move::Choose(Answer::Mana(one(Color::Blue)))).unwrap();
    cite!(["608.2d"]; assert_eq!((g.players[0].pool.colors[Color::Blue as usize], g.stack.len()), (1, 0)));
}

#[test]
fn r305_6_a_land_with_a_basic_land_type_has_the_ability_to_tap_for_its_mana() {
    let mut g = game();
    let forest = g.put(0, FOREST, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    // no text gives it: the reminder text in parentheses is not an ability
    assert!(g.defs[FOREST].abilities().is_empty());
    cite!(["305.6"]; assert_eq!((g.abilities(forest).len(), g.abilities(bears).len()), (1, 0)));
    activate(&mut g, 0, forest, vec![], vec![], mana_pool("")).unwrap();
    cite!(["305.6"]; assert_eq!(green(&g, 0), 1));
    cite!(["107.5"]; refused("Forest can't pay {T} as it is (107.5, 118.3)"): g.act(0, Move::Activate { source: forest, ability: 0, targets: vec![], paying: vec![] }));
}

#[test]
fn r118_3_a_cost_is_paid_in_full_or_not_at_all() {
    // a player with 1 life can't pay 2 life (119.4)
    let mut g = Game::new(defs(), 2, 1);
    let greed = g.put(0, GREED, Zone::Battlefield);
    let swamp = g.put(0, SWAMP, Zone::Battlefield);
    g.act(0, Move::Activate { source: swamp, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    cite!(["119.4"]; refused("you can't pay 2 life with a life total of 1 (119.4)"): activate(&mut g, 0, greed, vec![], vec![], mana_pool("B")));
    assert_eq!((g.players[0].pool.total(), g.players[0].life, g.stack.len()), (1, 1, 0), "nothing was paid");

    let mut g = game();
    let greed = g.put(0, GREED, Zone::Battlefield);
    let swamp = g.put(0, SWAMP, Zone::Battlefield);
    g.put(0, BEARS, Zone::Library);
    // nor without the mana (118.3a)
    cite!(["118.3a"]; refused("that mana is not in your mana pool (118.3a)"): activate(&mut g, 0, greed, vec![], vec![], mana_pool("B")));
    assert_eq!(g.players[0].life, 20);
    g.act(0, Move::Activate { source: swamp, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    activate(&mut g, 0, greed, vec![], vec![], mana_pool("B")).unwrap();
    cite!(["118.3a", "119.4"]; assert_eq!((g.players[0].pool.total(), g.players[0].life), (0, 18)));
    assert_eq!(g.players[0].hand.len(), 0, "the effect waits on the stack");
    all_pass(&mut g);
    assert_eq!(g.players[0].hand.len(), 1);
}

/// A pool of this much green, black and white mana.
fn mana(green: u32, black: u32, white: u32) -> Pool {
    let mut pool = Pool::default();
    pool.colors[Color::Green as usize] = green;
    pool.colors[Color::Black as usize] = black;
    pool.colors[Color::White as usize] = white;
    pool
}

#[test]
fn r107_4b_generic_mana_in_a_cost_is_paid_with_any_type_of_mana_the_player_chooses() {
    // {1}{G} with {G}{G}{B} in the pool: the {1} is paid with {G} or with {B}, as its player says
    let pool = |g: &mut Game| {
        let bears = g.put(0, BEARS, Zone::Hand);
        for land in [FOREST, FOREST, SWAMP] {
            let land = g.put(0, land, Zone::Battlefield);
            g.act(0, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
        }
        Move::Cast { card: bears, targets: vec![] }
    };
    let mut g = game();
    let cast = pool(&mut g);
    g.act(0, Move::Paying(mana(2, 0, 0), Box::new(cast))).unwrap();
    cite!(["118.3a"]; assert_eq!(g.players[0].pool, mana(0, 1, 0), "the black mana is still there (118.3a)"));
    let mut g = game();
    let cast = pool(&mut g);
    // mana that is not in the pool, that does not pay the cost, or that is more than the cost
    cite!(["118.3a"]; refused("that mana is not in your mana pool (118.3a)"): g.act(0, Move::Paying(mana(1, 0, 1), Box::new(cast.clone()))));
    cite!(["601.2h"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0, Move::Paying(mana(0, 1, 0), Box::new(cast.clone()))));
    cite!(["601.2h"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0, Move::Paying(mana(2, 1, 0), Box::new(cast.clone()))));
    assert_eq!((g.players[0].pool.clone(), g.stack.len()), (mana(2, 1, 0), 0), "nothing was paid");
    g.act(0, Move::Paying(mana(1, 1, 0), Box::new(cast))).unwrap();
    cite!(["107.4b"]; assert_eq!(g.players[0].pool, mana(1, 0, 0)));
    // the cost of an ability likewise: "{2}, {T}, Sacrifice this artifact: …"
    let mut g = game();
    let egg = g.put(0, EGG, Zone::Battlefield);
    for land in [FOREST, SWAMP, PLAINS] {
        let land = g.put(0, land, Zone::Battlefield);
        g.act(0, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
    let activation = Move::Activate { source: egg, ability: 0, targets: vec![], paying: vec![] };
    g.act(0, Move::Paying(mana(1, 0, 1), Box::new(activation))).unwrap();
    cite!(["107.4b"]; assert_eq!(g.players[0].pool, mana(0, 1, 0)));
    // only a spell or an ability has a cost that mana pays
    crate::support::refused(g.act(0, Move::Paying(mana(0, 0, 0), Box::new(Move::Pass))), "mana pays the cost of a spell or an ability, is chosen for a mana ability, and modes are chosen for a spell or ability (601.2b, 601.2h, 605.3b)");
}

#[test]
fn r601_2h_a_cost_that_asks_for_a_choice_is_paid_with_what_the_player_chooses() {
    let mut g = game();
    let rusalka = g.put(0, RUSALKA, Zone::Battlefield);
    let forest = g.put(0, FOREST, Zone::Battlefield);
    let mine = g.put(0, BEARS, Zone::Battlefield);
    let theirs = g.put(1, BEARS, Zone::Battlefield);
    g.act(0, Move::Activate { source: forest, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    cite!(["601.2h"]; refused("that selection does not satisfy the cost, which asks for exactly 1 permanent(s): 0 chosen (601.2h, 701.21a)"): activate(&mut g, 0, rusalka, vec![], vec![], mana_pool("G")));
    cite!(["118.3"]; refused("Forest is not what that cost asks for or cannot pay it (701.21a, 118.3)"): activate(&mut g, 0, rusalka, vec![], vec![vec![forest]], mana_pool("G")));
    // a player sacrifices only a permanent they control (701.21a)
    cite!(["701.21a"]; refused("Grizzly Bears is not what that cost asks for or cannot pay it (701.21a, 118.3)"): activate(&mut g, 0, rusalka, vec![], vec![vec![theirs]], mana_pool("G")));
    cite!(["601.2h"]; refused("that selection does not satisfy the cost, which asks for exactly 1 permanent(s): 2 chosen (601.2h, 701.21a)"): activate(&mut g, 0, rusalka, vec![], vec![vec![mine, rusalka]], mana_pool("G")));
    assert_eq!((green(&g, 0), g.battlefield.len()), (1, 4), "nothing was paid");
    activate(&mut g, 0, rusalka, vec![], vec![vec![mine]], mana_pool("G")).unwrap();
    cite!(["601.2h", "701.21a"]; assert!(g.log.contains(&Event::Sacrificed { player: 0, object: mine })));
    assert_eq!((green(&g, 0), g.players[0].life), (0, 20));
    all_pass(&mut g);
    assert_eq!(g.players[0].life, 21);
}

#[test]
fn r701_9a_a_cost_to_discard_is_paid_with_a_card_from_the_players_hand() {
    let mut g = game();
    let witness = g.put(0, WITNESS, Zone::Battlefield);
    let plains: Vec<ObjId> = (0..2).map(|_| g.put(0, PLAINS, Zone::Battlefield)).collect();
    let in_hand = g.put(0, BEARS, Zone::Hand);
    let on_battlefield = g.put(0, BEARS, Zone::Battlefield);
    let their_card = g.put(1, BEARS, Zone::Hand);
    let ring = g.put(1, SOL_RING, Zone::Battlefield);
    for p in plains {
        g.act(0, Move::Activate { source: p, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
    let target = vec![vec![object(ring)]];
    cite!(["701.9a"]; refused("Grizzly Bears is not what that cost asks for or cannot pay it (701.9a, 118.3)"): activate(&mut g, 0, witness, target.clone(), vec![vec![on_battlefield]], mana_pool("WW")));
    cite!(["701.9a"]; refused("Grizzly Bears is not what that cost asks for or cannot pay it (701.9a, 118.3)"): activate(&mut g, 0, witness, target.clone(), vec![vec![their_card]], mana_pool("WW")));
    assert!(!g.obj(witness).tapped, "nothing was paid");
    activate(&mut g, 0, witness, target, vec![vec![in_hand]], mana_pool("WW")).unwrap();
    assert!(g.obj(witness).tapped);
    cite!(["701.9a"]; assert_eq!((g.players[0].hand.len(), g.players[0].graveyard.len(), g.players[0].pool.total()), (0, 1, 0)));
    all_pass(&mut g);
    assert!(g.log.contains(&Event::Destroyed { object: ring }));
}

#[test]
fn r113_7a_an_ability_on_the_stack_exists_independently_of_its_source() {
    let mut g = game();
    let rider = g.put(0, RIDER, Zone::Battlefield);
    let ring = g.put(1, SOL_RING, Zone::Battlefield);
    activate(&mut g, 0, rider, vec![vec![object(ring)]], vec![], mana_pool("")).unwrap();
    // its source was sacrificed to pay for it
    assert!(!g.battlefield.contains(&rider));
    assert_eq!(g.stack.len(), 1);
    all_pass(&mut g);
    cite!(["113.7a"]; assert!(g.log.contains(&Event::Destroyed { object: ring })));
    assert_eq!(g.battlefield, vec![]);
}

#[test]
fn r602_5d_only_as_a_sorcery_is_in_a_main_phase_of_ones_own_turn_with_the_stack_empty() {
    let mut g = game();
    let rider = g.put(0, RIDER, Zone::Battlefield);
    let theirs = g.put(1, RIDER, Zone::Battlefield);
    let pyromancer = g.put(0, PYROMANCER, Zone::Battlefield);
    let ring = g.put(1, SOL_RING, Zone::Battlefield);
    let target = vec![vec![object(ring)]];
    // not with something on the stack
    activate(&mut g, 0, pyromancer, vec![vec![Target::Player(1)]], vec![], mana_pool("")).unwrap();
    cite!(["602.5d"]; refused("that ability is activated only as a sorcery: in a main phase of your own turn with the stack empty (602.5d)"): activate(&mut g, 0, rider, target.clone(), vec![], mana_pool("")));
    all_pass(&mut g); // Resolve the pinger: a nonempty stack must not mask the timing check.
    assert!(g.stack.is_empty());
    // nor in another player's turn
    g.act(0, Move::Pass).unwrap();
    cite!(["602.5d"]; refused("that ability is activated only as a sorcery: in a main phase of your own turn with the stack empty (602.5d)"): activate(&mut g, 1, theirs, vec![vec![object(ring)]], vec![], mana_pool("")));
    g.act(1, Move::Pass).unwrap();
    // nor outside a main phase
    assert_eq!(g.part, Part::BeginningOfCombat);
    cite!(["602.5d"]; refused("that ability is activated only as a sorcery: in a main phase of your own turn with the stack empty (602.5d)"): activate(&mut g, 0, rider, target, vec![], mana_pool("")));
    assert!(g.battlefield.contains(&rider), "nothing was paid");
}

#[test]
fn r602_5b_only_once_each_turn_is_of_that_objects_ability() {
    let mut g = game();
    let translator = g.put(0, TRANSLATOR, Zone::Battlefield);
    let other = g.put(0, TRANSLATOR, Zone::Battlefield);
    // its second ability: the first is devoid
    let pay_life = |source| Move::Activate { source, ability: 1, targets: vec![], paying: vec![] };
    cite!(["602.1c"]; refused("that is not an activated ability of that permanent (602.1c)"): activate(&mut g, 0, translator, vec![], vec![], mana_pool("")));
    g.act(0, pay_life(translator)).unwrap();
    assert_eq!((g.players[0].pool.colorless, g.players[0].life), (1, 19));
    cite!(["602.1b"]; refused("that ability is activated only once each turn (602.1b)"): g.act(0, pay_life(translator)));
    assert_eq!(g.players[0].life, 19, "nothing was paid");
    // another object with the same ability is another ability
    g.act(0, pay_life(other)).unwrap();
    assert_eq!((g.players[0].pool.colorless, g.players[0].life), (2, 18));
}

#[test]
fn r113_6j_an_ability_whose_cost_is_paid_from_the_hand_functions_there() {
    // "Channel — {1}{B}, Discard this card: …": its cost can't be paid from the battlefield, so
    // it functions in the hand and resolves through an ordinary captured activation.
    let shinen = CardDef::new(
        "Shinen of Fear's Chill",
        "{4}{B}",
        "Creature — Spirit",
        "This creature can't block.\nChannel — {1}{B}, Discard this card: Target creature can't block this turn.",
        Some((3, 2)),
    )
    .expect("the grammar reads it");
    check(&shinen).unwrap();
    let bear = CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap();
    let mut g = Game::new(vec![shinen, bear], 2, 20);
    let card = g.put(0, 0, Zone::Hand);
    let target = g.put(1, 1, Zone::Battlefield);
    let attacker = g.put(0, 1, Zone::Battlefield);
    assert!(!g.cant_block(target, attacker, &[target]));
    g.players[0].pool.colorless = 1;
    g.players[0].pool.colors[2] = 1;
    g.act(0, paid("CB", Move::Activate { source: card, ability: 1, targets: vec![vec![Target::Object(target)]], paying: vec![] })).unwrap();
    cite!(["113.6j"]; assert_eq!(g.stack.len(), 1, "Channel waits on the stack"));
    assert!(!g.cant_block(target, attacker, &[target]), "the effect has not resolved");
    g.act(0, Move::Pass).unwrap(); g.act(1, Move::Pass).unwrap();
    assert!(g.cant_block(target, attacker, &[target]), "the resolving Channel forbids blocking");
    assert_eq!(g.players[0].graveyard.len(), 1);
    assert!(g.stack.is_empty());
}
