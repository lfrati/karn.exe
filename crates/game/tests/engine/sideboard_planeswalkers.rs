//! The sideboard planeswalkers as the corpus prints them, each ability run to its end:
//! Garruk Wildspeaker's last ability, Garruk, Primal Hunter, Nissa Revane, and the first
//! ability of Chandra, Pyromaster.
use crate::support::cite;
use game::legal::Asked;
use game::state::{Answer, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const WILDSPEAKER: usize = 0;
const HUNTER: usize = 1;
const NISSA: usize = 2;
const CHANDRA: usize = 3;
const CHOSEN: usize = 4;
const FOREST: usize = 5;
const BEAR: usize = 6;
const GIANT: usize = 7;
const ELF: usize = 8;
const JACE: usize = 9;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let real = |name: &str| CardDef::from_faces(catalog.get(name).unwrap_or_else(|| panic!("{name} is in the corpus"))).unwrap();
    let defs = vec![
        real("Garruk Wildspeaker"),
        real("Garruk, Primal Hunter"),
        real("Nissa Revane"),
        real("Chandra, Pyromaster"),
        real("Nissa's Chosen"),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Giant", "{3}{R}", "Creature — Giant", "", Some((5, 3))).unwrap(),
        CardDef::new("Elf", "{G}", "Creature — Elf Druid", "", Some((1, 1))).unwrap(),
        real("Jace Beleren"),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    let mut g = Game::new(defs, 2, 20);
    for player in 0..2 { for _ in 0..12 { g.put(player, FOREST, Zone::Library); } }
    g
}
fn loyalty(g: &Game, o: ObjId) -> u32 { g.obj(o).counters.get("loyalty").copied().unwrap_or(0) }
/// The planeswalker on the battlefield under player 0's control with this much loyalty.
fn walker(g: &mut Game, def: usize, counters: u32) -> ObjId {
    let walker = g.put(0, def, Zone::Battlefield);
    let has = loyalty(g, walker);
    g.put_counters(walker, "loyalty", counters - has);
    walker
}
fn activate(g: &mut Game, source: ObjId, ability: usize, targets: Vec<Vec<Target>>) -> Result<(), String> {
    g.act(0, Move::Activate { source, ability, targets, paying: vec![] })
}
/// All pass while the stack has something and nothing else is asked.
fn resolve(g: &mut Game) {
    while g.asked().is_some_and(|(_, a)| a == Asked::Priority) && !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
}
fn mine(g: &Game, def: usize) -> Vec<ObjId> {
    g.battlefield.iter().copied().filter(|o| g.obj(*o).card == Some(def) && g.obj(*o).controller == 0).collect()
}
fn tokens(g: &Game, name: &str) -> Vec<ObjId> {
    g.battlefield.iter().copied().filter(|o| g.obj(*o).token() && g.obj(*o).chars.name == format!("{name} Token")).collect()
}
/// Pass, attacking with nothing and answering anything else with its default, until this
/// player's turn has begun.
fn to_turn_of(g: &mut Game, player: usize) {
    for _ in 0..200 {
        if g.active == player { return; }
        match g.decision.clone() {
            Some(Decision::Attackers { player: p }) => g.act(p, Move::Attack(vec![])).unwrap(),
            _ => match g.asked().expect("the game goes on") {
                (p, Asked::Priority) => g.act(p, Move::Pass).unwrap(),
                (_, asked) => panic!("unexpected decision while advancing this fixture: {asked:?}"),
            },
        }
    }
    panic!("no turn of player {player}");
}
/// Pass, attacking with nothing and answering anything else with its default, until this
/// player's declaration of attackers.
fn to_attackers(g: &mut Game, player: usize) {
    for _ in 0..200 {
        match g.decision.clone() {
            Some(Decision::Attackers { player: p }) if p == player => return,
            Some(Decision::Attackers { player: p }) => g.act(p, Move::Attack(vec![])).unwrap(),
            _ => match g.asked().expect("the game goes on") {
                (p, Asked::Priority) => g.act(p, Move::Pass).unwrap(),
                (_, asked) => panic!("unexpected decision while advancing this fixture: {asked:?}"),
            },
        }
    }
    panic!("no declaration of attackers by player {player}");
}

// ------------------------------------------------------------------ Garruk Wildspeaker

#[test]
fn wildspeaker_overrun_is_of_the_creatures_controlled_as_it_resolves_until_end_of_turn() {
    let mut g = game();
    let garruk = walker(&mut g, WILDSPEAKER, 3);
    let (bear, theirs) = (g.put(0, BEAR, Zone::Battlefield), g.put(1, BEAR, Zone::Battlefield));
    // three counters cannot pay four (606.6)
    cite!(["606.6"]; refused("Garruk Wildspeaker can't have 4 loyalty counters removed to pay this cost (606.6, 118.3, 118.10)"): activate(&mut g, garruk, 2, vec![]));
    g.put_counters(garruk, "loyalty", 1);
    activate(&mut g, garruk, 2, vec![]).unwrap();
    assert_eq!(loyalty(&g, garruk), 0);
    // the cost is paid, so the ability resolves though Garruk is gone (113.7a)
    cite!(["113.7a"]; assert_eq!(g.obj(garruk).zone, Zone::Gone));
    resolve(&mut g);
    assert_eq!((g.power(bear), g.toughness(bear)), (Some(5), Some(5)));
    assert!(g.has(bear, "Trample"));
    assert_eq!((g.power(theirs), g.has(theirs, "Trample")), (Some(2), false));
    // the set of creatures was fixed as the ability resolved (611.2c)
    let late = g.put(0, BEAR, Zone::Battlefield);
    cite!(["611.2c"]; assert_eq!((g.power(late), g.has(late, "Trample")), (Some(2), false)));
    // 5 trampling over a 2/2 blocker: 3 to the player
    to_attackers(&mut g, 0);
    g.act(0, Move::Attack(vec![(bear, Target::Player(1))])).unwrap();
    while g.decision != Some(Decision::Blockers { player: 1 }) { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(1, Move::Block(vec![(theirs, bear)])).unwrap();
    for _ in 0..40 {
        if g.players[1].life != 20 || g.obj(bear).zone != Zone::Battlefield { break; }
        let (p, asked) = g.asked().unwrap();
        match asked {
            Asked::Priority => g.act(p, Move::Pass).unwrap(),
            Asked::Divide { among, .. } => g.act(p, Move::Divide(vec![(among[0], 2), (Target::Player(1), 3)])).unwrap(),
            other => panic!("unexpected: {other:?}"),
        }
    }
    cite!(["702.19b"]; assert_eq!(g.players[1].life, 17));
    assert_eq!(g.obj(theirs).zone, Zone::Gone);
    // until end of turn (514.2)
    to_turn_of(&mut g, 1);
    cite!(["514.2"]; assert_eq!((g.power(bear), g.toughness(bear), g.has(bear, "Trample")), (Some(2), Some(2), false)));
}

// ------------------------------------------------------------------ Garruk, Primal Hunter

#[test]
fn primal_hunter_makes_a_beast() {
    let mut g = game();
    let garruk = walker(&mut g, HUNTER, 3);
    activate(&mut g, garruk, 0, vec![]).unwrap();
    resolve(&mut g);
    cite!(["606.4"]; assert_eq!(loyalty(&g, garruk), 4));
    let [beast] = tokens(&g, "Beast")[..] else { panic!("one Beast") };
    let o = g.obj(beast);
    assert_eq!((g.power(beast), g.toughness(beast), o.controller), (Some(3), Some(3), 0));
    assert!(o.is(ir::CardType::Creature) && o.chars.colors == vec![ir::Color::Green]);
    // one loyalty ability a turn (606.3)
    cite!(["606.3"]; refused("a loyalty ability of that permanent has already been activated this turn (606.3)"): activate(&mut g, garruk, 1, vec![]));
}

#[test]
fn primal_hunter_draws_for_the_greatest_power_among_its_controllers_creatures() {
    let mut g = game();
    let garruk = walker(&mut g, HUNTER, 3);
    g.put(0, BEAR, Zone::Battlefield);
    let beast_maker = g.put(0, ELF, Zone::Battlefield);
    g.put_counters(beast_maker, "+1/+1", 3);
    // an opponent's greater creature is not counted
    g.put(1, GIANT, Zone::Battlefield);
    let before = g.players[0].hand.len();
    // read as the ability resolves (608.2h): a creature that arrives in response counts
    let mut grown = g.clone();
    activate(&mut g, garruk, 1, vec![]).unwrap();
    activate(&mut grown, garruk, 1, vec![]).unwrap();
    grown.put(0, GIANT, Zone::Battlefield);
    resolve(&mut g);
    resolve(&mut grown);
    assert_eq!(loyalty(&g, garruk), 0);
    cite!(["608.2h"]; assert_eq!(g.players[0].hand.len() - before, 4, "the 1/1 with three counters is the greatest: 4"));
    cite!(["608.2h"]; assert_eq!(grown.players[0].hand.len() - before, 5));
}

#[test]
fn primal_hunter_draws_nothing_with_no_creature() {
    let mut g = game();
    let garruk = walker(&mut g, HUNTER, 3);
    g.put(1, GIANT, Zone::Battlefield);
    activate(&mut g, garruk, 1, vec![]).unwrap();
    resolve(&mut g);
    // "the greatest power among creatures you control" with none can't be determined: 0 (107.2)
    cite!(["107.2"]; assert!(g.players[0].hand.is_empty()));
    cite!(["107.2"]; assert_eq!(g.players[0].library.len(), 12));
    // Uncited (protocol): the ability is done and its controller gets priority, with no question left
    assert_eq!(g.asked().map(|(_, a)| a), Some(Asked::Priority));
}

#[test]
fn primal_hunter_makes_a_wurm_for_each_land_its_controller_controls() {
    let mut g = game();
    let garruk = walker(&mut g, HUNTER, 5);
    for _ in 0..3 { g.put(0, FOREST, Zone::Battlefield); }
    for _ in 0..2 { g.put(1, FOREST, Zone::Battlefield); }
    cite!(["606.6"]; refused("Garruk, Primal Hunter can't have 6 loyalty counters removed to pay this cost (606.6, 118.3, 118.10)"): activate(&mut g, garruk, 2, vec![]));
    g.put_counters(garruk, "loyalty", 1);
    activate(&mut g, garruk, 2, vec![]).unwrap();
    // counted as the ability resolves: a land that arrives in response is one more
    g.put(0, FOREST, Zone::Battlefield);
    resolve(&mut g);
    let wurms = tokens(&g, "Wurm");
    cite!(["608.2h"]; assert_eq!(wurms.len(), 4));
    for wurm in wurms {
        let o = g.obj(wurm);
        assert_eq!((g.power(wurm), g.toughness(wurm), o.controller), (Some(6), Some(6), 0));
        assert!(o.is(ir::CardType::Creature) && o.chars.colors == vec![ir::Color::Green]);
    }
}

#[test]
fn primal_hunter_makes_no_wurm_with_no_land() {
    let mut g = game();
    let garruk = walker(&mut g, HUNTER, 6);
    g.put(1, FOREST, Zone::Battlefield);
    activate(&mut g, garruk, 2, vec![]).unwrap();
    resolve(&mut g);
    assert!(tokens(&g, "Wurm").is_empty());
}

// ------------------------------------------------------------------ Nissa Revane

#[test]
fn nissa_finds_her_chosen_and_nothing_else() {
    let mut g = game();
    let nissa = walker(&mut g, NISSA, 2);
    let chosen = [g.put(0, CHOSEN, Zone::Library), g.put(0, CHOSEN, Zone::Library)];
    let elf = g.put(0, ELF, Zone::Library);
    g.put(1, CHOSEN, Zone::Library);
    let from = g.log.end();
    activate(&mut g, nissa, 0, vec![]).unwrap();
    resolve(&mut g);
    assert_eq!(loyalty(&g, nissa), 3);
    let Some((0, Asked::Search { cards, among, min, max })) = g.asked() else { panic!("a search: {:?}", g.asked()) };
    assert_eq!((cards.len(), min, max), (15, 0, 1));
    cite!(["701.23a"]; assert_eq!(among, chosen.to_vec(), "only her own library's cards of that name"));
    // another card, two cards, another player's answer: refused, and the question stands
    let question = g.asked();
    cite!(["701.23a"]; refused("a chosen card is not one the search can find, in the searched zone and matching its description (701.23a): choose between 0 and 1 distinct matching objects"): g.act(0, Move::Choose(Answer::Objects(vec![elf]))));
    cite!(["608.2d"]; refused("2 chosen (608.2d): choose between 0 and 1 distinct matching objects"): g.act(0, Move::Choose(Answer::Objects(chosen.to_vec()))));
    crate::support::refused(g.act(1, Move::Choose(Answer::Objects(vec![chosen[0]]))), "that decision is another player's");
    assert_eq!(g.asked(), question);
    // a card of a stated name need not be found (701.23b): a copy of the game finds none
    let mut none = g.clone();
    g.act(0, Move::Choose(Answer::Objects(vec![chosen[1]]))).unwrap();
    none.act(0, Move::Choose(Answer::Objects(vec![]))).unwrap();
    assert_eq!(mine(&g, CHOSEN).len(), 1);
    assert_eq!((g.players[0].library.len(), none.players[0].library.len()), (14, 15));
    cite!(["701.23b"]; assert!(mine(&none, CHOSEN).is_empty()));
    for game in [&mut g, &mut none] {
        let shuffled = game.log.since(from).iter().filter(|e| matches!(e, game::Event::Shuffled { player: 0, .. })).count();
        // Uncited (card text): Nissa Revane's "Search your library for a card named Nissa's Chosen, put it onto the battlefield, then shuffle"
        assert_eq!(shuffled, 1, "the library is shuffled either way (701.23)");
        assert_eq!(game.asked().map(|(_, a)| a), Some(Asked::Priority));
    }
}

#[test]
fn nissa_gains_two_life_for_each_elf_her_controller_controls() {
    let mut g = game();
    let nissa = walker(&mut g, NISSA, 2);
    g.put(0, ELF, Zone::Battlefield);
    g.put(0, CHOSEN, Zone::Battlefield);
    g.put(0, BEAR, Zone::Battlefield);
    g.put(1, ELF, Zone::Battlefield);
    let mut none = g.clone();
    activate(&mut g, nissa, 1, vec![]).unwrap();
    // counted as the ability resolves
    g.put(0, ELF, Zone::Battlefield);
    resolve(&mut g);
    cite!(["608.2h"]; assert_eq!((g.players[0].life, g.players[1].life, loyalty(&g, nissa)), (26, 20, 3)));
    for elf in mine(&none, ELF).into_iter().chain(mine(&none, CHOSEN)) { none.move_object(elf, Zone::Exile); }
    activate(&mut none, nissa, 1, vec![]).unwrap();
    resolve(&mut none);
    assert_eq!(none.players[0].life, 20);
}

#[test]
fn nissa_puts_any_number_of_elf_creature_cards_onto_the_battlefield() {
    let mut g = game();
    let nissa = walker(&mut g, NISSA, 6);
    let elves = [g.put(0, ELF, Zone::Library), g.put(0, CHOSEN, Zone::Library), g.put(0, ELF, Zone::Library)];
    let bear = g.put(0, BEAR, Zone::Library);
    cite!(["606.6"]; refused("Nissa Revane can't have 7 loyalty counters removed to pay this cost (606.6, 118.3, 118.10)"): activate(&mut g, nissa, 2, vec![]));
    g.put_counters(nissa, "loyalty", 1);
    activate(&mut g, nissa, 2, vec![]).unwrap();
    resolve(&mut g);
    let Some((0, Asked::Search { among, min, max, .. })) = g.asked() else { panic!("a search: {:?}", g.asked()) };
    assert_eq!((among.clone(), min, max), (elves.to_vec(), 0, 3));
    cite!(["701.23a"]; refused("a chosen card is not one the search can find, in the searched zone and matching its description (701.23a): choose between 0 and 3 distinct matching objects"): g.act(0, Move::Choose(Answer::Objects(vec![elves[0], bear]))));
    cite!(["608.2d"]; refused("each object is chosen once (608.2d): choose between 0 and 3 distinct matching objects"): g.act(0, Move::Choose(Answer::Objects(vec![elves[0], elves[0]]))));
    // copies of the game take all, some and none
    let (mut some, mut none) = (g.clone(), g.clone());
    g.act(0, Move::Choose(Answer::Objects(elves.to_vec()))).unwrap();
    some.act(0, Move::Choose(Answer::Objects(vec![elves[1]]))).unwrap();
    none.act(0, Move::Choose(Answer::Objects(vec![]))).unwrap();
    let elves_of = |game: &Game| mine(game, ELF).len() + mine(game, CHOSEN).len();
    cite!(["701.23b"]; assert_eq!((elves_of(&g), elves_of(&some), elves_of(&none)), (3, 1, 0)));
    assert_eq!((g.players[0].library.len(), some.players[0].library.len(), none.players[0].library.len()), (13, 15, 16));
    assert_eq!(mine(&some, CHOSEN).len(), 1);
    // they entered at once, untapped, under her controller's control
    assert!(mine(&g, ELF).iter().all(|o| !g.obj(*o).tapped && g.obj(*o).controller == 0));
    cite!(["704.5i"]; assert_eq!(g.obj(nissa).zone, Zone::Gone));
    assert_eq!(g.asked().map(|(_, a)| a), Some(Asked::Priority));
}

// ------------------------------------------------------------------ Chandra, Pyromaster

#[test]
fn chandra_pings_a_player_and_a_creature_of_theirs_which_cannot_block() {
    let mut g = game();
    let chandra = walker(&mut g, CHANDRA, 4);
    let (theirs, other, own) = (g.put(1, GIANT, Zone::Battlefield), g.put(1, BEAR, Zone::Battlefield), g.put(0, BEAR, Zone::Battlefield));
    // the creature is one the targeted player controls (115.1, 601.2c)
    cite!(["115.1"]; refused("an illegal target, Bear: the target is one the player it names controls, and you control it (115.1)"): activate(&mut g, chandra, 0, vec![vec![Target::Player(1)], vec![Target::Object(own)]]));
    cite!(["115.1"]; refused("an illegal target, Giant: the target is one the player it names controls, and another player controls it (115.1)"): activate(&mut g, chandra, 0, vec![vec![Target::Player(0)], vec![Target::Object(theirs)]]));
    cite!(["115.1"]; refused("an illegal target, Giant: it isn't a planeswalker (115.1)"): activate(&mut g, chandra, 0, vec![vec![Target::Object(theirs)], vec![]]));
    cite!(["601.2c"]; refused("wrong number of targets: 2 chosen, it takes 0 to 1 (601.2c)"): activate(&mut g, chandra, 0, vec![vec![Target::Player(1)], vec![Target::Object(theirs), Target::Object(other)]]));
    assert_eq!(loyalty(&g, chandra), 4, "nothing was paid for what was refused");
    // "up to one": no creature at all, in a copy of the game
    let mut alone = g.clone();
    cite!(["601.2c"]; ok: activate(&mut alone, chandra, 0, vec![vec![Target::Player(1)], vec![]]));
    resolve(&mut alone);
    assert_eq!((alone.players[1].life, alone.obj(theirs).damage, alone.obj(other).damage), (19, 0, 0));
    activate(&mut g, chandra, 0, vec![vec![Target::Player(1)], vec![Target::Object(theirs)]]).unwrap();
    resolve(&mut g);
    cite!(["120.3a", "120.3e"]; assert_eq!((g.players[1].life, g.obj(theirs).damage, g.obj(other).damage, loyalty(&g, chandra)), (19, 1, 0, 5)));
    // that creature can't block this turn; the other can
    to_attackers(&mut g, 0);
    g.act(0, Move::Attack(vec![(own, Target::Player(1))])).unwrap();
    while g.decision != Some(Decision::Blockers { player: 1 }) { g.act(g.priority, Move::Pass).unwrap(); }
    let Some((1, Asked::Blockers { able, .. })) = g.asked() else { panic!("blockers: {:?}", g.asked()) };
    cite!(["509.1b"]; assert_eq!(able, vec![(theirs, vec![]), (other, vec![own])], "the listing gives the Giant nothing it could block"));
    cite!(["509.1b"]; refused("Giant can't block Bear: Chandra, Pyromaster says it can't, or not that way (509.1b)"): g.act(1, Move::Block(vec![(theirs, own)])));
    g.act(1, Move::Block(vec![])).unwrap();
    // "this turn": it can block in its controller's next combat as the defender again
    to_attackers(&mut g, 1);
    g.act(1, Move::Attack(vec![])).unwrap();
    to_attackers(&mut g, 0);
    let own = mine(&g, BEAR)[0];
    g.act(0, Move::Attack(vec![(own, Target::Player(1))])).unwrap();
    while g.decision != Some(Decision::Blockers { player: 1 }) { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["514.2"]; ok: g.act(1, Move::Block(vec![(theirs, own)])));
}

#[test]
fn chandra_pings_a_planeswalker_and_a_creature_of_its_controller() {
    let mut g = game();
    let chandra = walker(&mut g, CHANDRA, 4);
    let jace = g.put(1, JACE, Zone::Battlefield);
    g.put_counters(jace, "loyalty", 3);
    let before = loyalty(&g, jace);
    let (theirs, own) = (g.put(1, BEAR, Zone::Battlefield), g.put(0, BEAR, Zone::Battlefield));
    cite!(["115.1"]; refused("an illegal target, Bear: the target is one the player it names controls, and you control it (115.1)"): activate(&mut g, chandra, 0, vec![vec![Target::Object(jace)], vec![Target::Object(own)]]));
    activate(&mut g, chandra, 0, vec![vec![Target::Object(jace)], vec![Target::Object(theirs)]]).unwrap();
    // the creature leaves in response: the rest still happens (608.2b), and nothing is
    // said of a creature that is no longer there
    let mut gone = g.clone();
    gone.move_object(theirs, Zone::Graveyard);
    resolve(&mut g);
    resolve(&mut gone);
    cite!(["306.8"]; assert_eq!((loyalty(&g, jace), g.obj(theirs).damage, g.players[1].life), (before - 1, 1, 20), "damage to a planeswalker removes loyalty (306.8)"));
    cite!(["608.2b"]; assert_eq!((loyalty(&gone, jace), gone.players[1].life), (before - 1, 20)));
    // both targets gone: the ability does nothing (608.2b)
    let mut fizzled = game();
    let chandra = walker(&mut fizzled, CHANDRA, 4);
    let jace = fizzled.put(1, JACE, Zone::Battlefield);
    fizzled.put_counters(jace, "loyalty", 3);
    activate(&mut fizzled, chandra, 0, vec![vec![Target::Object(jace)], vec![]]).unwrap();
    fizzled.move_object(jace, Zone::Graveyard);
    resolve(&mut fizzled);
    cite!(["608.2b"]; assert_eq!((fizzled.players[1].life, loyalty(&fizzled, chandra)), (20, 5)));
}
