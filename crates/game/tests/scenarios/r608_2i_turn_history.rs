//! What happened earlier in the turn (608.2i), asked of the turn's history: whether you
//! attacked, how many creatures died, what entered the battlefield and whether it is the
//! object that entered (400.7), and which time of the turn an event is (603.2). Each is asked
//! as it is now, at resolution, of what happened as it happened; a new turn, an extra one
//! included, begins with nothing (500.1).

use crate::support::{cite, paid};
use game::card::CardDef;
use game::state::Decision;
use game::{Game, Move, ObjId, Target, Zone};
use ir::Part;

const BEAR: usize = 0;
const ISLAND: usize = 1;
const KILL: usize = 2;
const CHARGE: usize = 3;
const WALK: usize = 4;
const HEAL: usize = 5;
const SPY: usize = 6;
const EMBRACE: usize = 7;
const SELL_SWORD: usize = 8;
const OUTRIDER: usize = 9;
const SERAPH: usize = 10;
const AURELIA: usize = 11;
const GATHERING: usize = 12;
const FLICKER: usize = 13;
const NAVIGATOR: usize = 14;
const MERFOLK: usize = 15;

fn defs() -> Vec<CardDef> {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let real = |name: &str| CardDef::from_faces(catalog.get(name).expect(name)).expect(name);
    let c = |name: &str, cost: &str, types: &str, text: &str, pt| CardDef::new(name, cost, types, text, pt).unwrap();
    vec![
        c("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        c("Island", "", "Basic Land — Island", "({T}: Add {U}.)", None),
        c("Kill", "{0}", "Instant", "Destroy target creature.", None),
        c("Charge", "{0}", "Instant", "If you attacked this turn, you gain 4 life.", None),
        c("Walk", "{0}", "Sorcery", "Take an extra turn after this one.", None),
        c("Heal", "{0}", "Instant", "You gain 1 life.", None),
        real("Storm Fleet Spy"),
        real("Deadly Embrace"),
        real("Callous Sell-Sword"),
        real("Bristlebane Outrider"),
        real("Vanguard Seraph"),
        real("Aurelia, the Warleader"),
        real("Gathering Place"),
        c("Flicker", "{0}", "Instant", "Exile target creature you control, then return that card to the battlefield under its owner's control.", None),
        real("Deepway Navigator"),
        c("Merfolk", "{U}", "Creature — Merfolk", "", Some((1, 1))),
    ]
}

/// Player 0's first precombat main phase, with cards in both libraries.
fn game() -> Game {
    let mut g = Game::new(defs(), 2, 20);
    for p in 0..2 { for _ in 0..20 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}

/// Both players pass until the stack is empty, answering nothing.
fn settle(g: &mut Game) {
    for _ in 0..20 {
        if g.stack.is_empty() || g.decision.is_some() { return; }
        g.act(g.priority, Move::Pass).unwrap();
    }
}

/// This player casts this card from their hand, with these targets, mana given, and it resolves.
fn cast(g: &mut Game, player: usize, def: usize, targets: Vec<Vec<Target>>, mana: ([u32; 5], u32)) -> ObjId {
    while g.priority != player { g.act(g.priority, Move::Pass).unwrap(); }
    g.players[player].pool.colors = mana.0;
    g.players[player].pool.colorless = mana.1;
    let card = g.put(player, def, Zone::Hand);
    g.act(player, Move::Paying(game::state::Pool { colors: mana.0, colorless: mana.1, ..Default::default() }, Box::new(Move::Cast { card, targets }))).unwrap();
    settle(g);
    card
}

fn free() -> ([u32; 5], u32) { ([0; 5], 0) }
fn colors(color: usize, n: u32, generic: u32) -> ([u32; 5], u32) { let mut c = [0; 5]; c[color] = n; (c, generic) }
const U: usize = 1;
const B: usize = 2;
const G: usize = 4;

/// To the next declaration of attackers of this player.
fn to_attackers(g: &mut Game, player: usize) {
    for _ in 0..200 {
        if g.decision == Some(Decision::Attackers { player }) { return; }
        match g.decision { Some(Decision::Attackers { player: p }) => { g.act(p, Move::Attack(vec![])).unwrap(); } _ => { g.act(g.priority, Move::Pass).unwrap(); } }
    }
    panic!("no declaration of attackers: {:?} {:?}", g.part, g.decision);
}

/// To this player's next postcombat main phase, with priority.
fn to_main_after_combat(g: &mut Game, player: usize) {
    for _ in 0..200 {
        if g.part == Part::PostcombatMain && g.active == player && g.priority == player && g.stack.is_empty() { return; }
        match g.decision { Some(Decision::Attackers { player: p }) => { g.act(p, Move::Attack(vec![])).unwrap(); } Some(Decision::Blockers { player: p }) => { g.act(p, Move::Block(vec![])).unwrap(); } _ => { g.act(g.priority, Move::Pass).unwrap(); } }
    }
    panic!("not there: {:?}", g.part);
}

/// To the next turn's precombat main phase, whoever's it is.
fn next_turn(g: &mut Game) {
    let turn = g.turn;
    for _ in 0..400 {
        if g.turn != turn && g.part == Part::PrecombatMain && g.stack.is_empty() && g.priority == g.active { return; }
        match g.decision { Some(Decision::Attackers { player: p }) => { g.act(p, Move::Attack(vec![])).unwrap(); } Some(Decision::Blockers { player: p }) => { g.act(p, Move::Block(vec![])).unwrap(); } Some(_) => panic!("{:?}", g.decision), None => { g.act(g.priority, Move::Pass).unwrap(); } }
    }
    panic!("no next turn");
}

fn hand(g: &Game, player: usize) -> usize { g.players[player].hand.len() }

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for def in defs() {
        assert_eq!(game::play::check(&def).err().map(|u| u.0), None, "{}", def.name());
    }
}

/// Raid: "if you attacked this turn" (508.1): you declared an attack, whatever became of the
/// attacker since; another player's attack is not yours.
#[test]
fn you_attacked_this_turn_with_an_attacker_since_destroyed() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let n = hand(&g, 0);
    cast(&mut g, 0, SPY, vec![], colors(U, 1, 2));
    cite!(["508.1", "603.4"]; assert_eq!(hand(&g, 0), n, "not attacked yet: no draw"));
    to_attackers(&mut g, 0);
    g.act(0, Move::Attack(vec![(bear, Target::Player(1))])).unwrap();
    to_main_after_combat(&mut g, 0);
    cast(&mut g, 0, KILL, vec![vec![Target::Object(bear)]], free());
    assert!(!g.battlefield.contains(&bear));
    let n = hand(&g, 0);
    cast(&mut g, 0, SPY, vec![], colors(U, 1, 2));
    cite!(["608.2i", "508.1"]; assert_eq!(hand(&g, 0), n + 1, "you attacked, with what is gone now"));

    // the opponent's turn: they attack; player 0 did not
    next_turn(&mut g);
    assert_eq!(g.active, 1);
    let theirs = g.put(1, BEAR, Zone::Battlefield);
    to_attackers(&mut g, 1);
    g.act(1, Move::Attack(vec![(theirs, Target::Player(0))])).unwrap();
    for _ in 0..10 { if g.priority == 0 && g.part != Part::DeclareAttackers { break; } if let Some(Decision::Blockers { player }) = g.decision { g.act(player, Move::Block(vec![])).unwrap(); } else { g.act(g.priority, Move::Pass).unwrap(); } }
    let life = g.players[0].life;
    cast(&mut g, 0, CHARGE, vec![], free());
    cite!(["608.2i"]; assert_eq!(g.players[0].life, life, "a player attacked, not you"));
    let life = g.players[1].life;
    cast(&mut g, 1, CHARGE, vec![], free());
    cite!(["608.2i"]; assert_eq!(g.players[1].life, life + 4));
}

/// "draw a card for each creature that died this turn": counted as the spell resolves, its own
/// destruction and one in response included, of any controller (608.2i, 608.2h).
#[test]
fn the_creatures_that_died_this_turn_are_counted_at_resolution() {
    let mut g = game();
    let mine = g.put(0, BEAR, Zone::Battlefield);
    let a = g.put(1, BEAR, Zone::Battlefield);
    let b = g.put(1, BEAR, Zone::Battlefield);
    cast(&mut g, 0, KILL, vec![vec![Target::Object(mine)]], free());
    // Deadly Embrace on A; Kill on B in response
    g.players[0].pool.colors = colors(B, 2, 3).0;
    g.players[0].pool.colorless = 3;
    let embrace = g.put(0, EMBRACE, Zone::Hand);
    g.act(0, paid("CCCBB", Move::Cast { card: embrace, targets: vec![vec![Target::Object(a)]] })).unwrap();
    let kill = g.put(0, KILL, Zone::Hand);
    g.act(0, Move::Cast { card: kill, targets: vec![vec![Target::Object(b)]] }).unwrap();
    let n = hand(&g, 0);
    settle(&mut g);
    cite!(["608.2i", "608.2h"]; assert_eq!(hand(&g, 0), n + 3, "mine, B in response, then A"));

    // a turn later, only that turn's
    next_turn(&mut g);
    next_turn(&mut g);
    assert_eq!(g.active, 0);
    let c = g.put(1, BEAR, Zone::Battlefield);
    let n = hand(&g, 0);
    cast(&mut g, 0, EMBRACE, vec![vec![Target::Object(c)]], colors(B, 2, 3));
    cite!(["500.1", "608.2i"]; assert_eq!(hand(&g, 0), n + 1));
}

/// An extra turn is a turn: it begins with nothing having happened in it (500.7, 500.1).
#[test]
fn an_extra_turn_begins_with_nothing_having_died() {
    let mut g = game();
    let mine = g.put(0, BEAR, Zone::Battlefield);
    cast(&mut g, 0, KILL, vec![vec![Target::Object(mine)]], free());
    cast(&mut g, 0, WALK, vec![], free());
    next_turn(&mut g);
    cite!(["500.7"]; assert_eq!(g.active, 0, "the extra turn"));
    let c = g.put(1, BEAR, Zone::Battlefield);
    let n = hand(&g, 0);
    cast(&mut g, 0, EMBRACE, vec![vec![Target::Object(c)]], colors(B, 2, 3));
    cite!(["500.7", "608.2i"]; assert_eq!(hand(&g, 0), n + 1, "only the one destroyed in this turn"));
}

/// "for each creature that died under your control this turn": as it died, whose it was.
#[test]
fn a_death_under_your_control_is_counted_by_its_controller_then() {
    let mut g = game();
    for p in [0, 0, 1] {
        let bear = g.put(p, BEAR, Zone::Battlefield);
        cast(&mut g, 0, KILL, vec![vec![Target::Object(bear)]], free());
    }
    let sword = cast(&mut g, 0, SELL_SWORD, vec![], colors(B, 1, 1));
    let sword = g.battlefield.iter().copied().find(|o| g.obj(*o).chars.name == "Callous Sell-Sword").unwrap_or(sword);
    cite!(["608.2i", "614.1c"]; assert_eq!(g.power(sword), Some(4), "two +1/+1 counters: player 1's bear isn't counted"));
}

/// "As long as another creature entered the battlefield under your control this turn": one
/// that entered and left since still did (608.2i); this creature's own return is its own
/// entry, not another's (400.7).
#[test]
fn another_creature_entered_this_turn_and_a_flickered_self_is_not_another() {
    let mut g = game();
    let outrider = g.put(0, OUTRIDER, Zone::Battlefield);
    next_turn(&mut g);
    next_turn(&mut g);
    assert_eq!(g.active, 0);
    cite!(["608.2i"]; assert_eq!(g.power(outrider), Some(3)));
    // it leaves and returns: a new object (400.7), whose entry is its own
    cast(&mut g, 0, FLICKER, vec![vec![Target::Object(outrider)]], free());
    let outrider = g.battlefield.iter().copied().find(|o| g.obj(*o).chars.name == "Bristlebane Outrider").unwrap();
    cite!(["400.7", "608.2i"]; assert_eq!(g.power(outrider), Some(3), "its own entry is not another creature's"));
    let bear = cast(&mut g, 0, BEAR, vec![], colors(G, 1, 1));
    let bear = g.battlefield.iter().copied().find(|o| g.obj(*o).card == Some(BEAR)).unwrap_or(bear);
    assert_eq!(g.power(outrider), Some(5));
    cast(&mut g, 0, KILL, vec![vec![Target::Object(bear)]], free());
    cite!(["608.2i"]; assert_eq!(g.power(outrider), Some(5), "it entered, though it is gone now"));
    next_turn(&mut g);
    cite!(["500.1"]; assert_eq!(g.power(outrider), Some(3), "a new turn"));
}

/// "Activate only if this land entered this turn": this object did. A land on the battlefield
/// since an earlier turn did not; the same card returned is a new object, which did (400.7).
#[test]
fn this_land_entered_this_turn_is_of_the_object_it_now_is() {
    let mut g = game();
    let card = g.put(0, GATHERING, Zone::Hand);
    g.act(0, Move::PlayLand(card)).unwrap();
    let place = *g.battlefield.last().unwrap();
    let colored = |g: &Game, o: ObjId| {
        let mut copy = g.clone();
        let green = vec![vec![ir::mana::ManaSymbol::Color(ir::Color::Green)]];
        copy.act(0, Move::Adding(green, Box::new(Move::Activate { source: o, ability: 1, targets: vec![], paying: vec![] }))).is_ok()
    };
    while g.priority != 0 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["608.2i"]; assert!(colored(&g, place), "it entered this turn"));
    next_turn(&mut g);
    next_turn(&mut g);
    assert!(!colored(&g, place), "it entered in an earlier turn");
    g.move_object(place, Zone::Exile);
    let card = *g.exile.last().unwrap();
    g.move_object(card, Zone::Battlefield);
    let place = *g.battlefield.last().unwrap();
    cite!(["400.7", "608.2i"]; assert!(colored(&g, place), "a new object, that entered this turn"));
}

/// "Whenever you gain life for the first time each turn": of your life gains of the turn, the
/// first; another player's doesn't count for you; each turn has its own first (603.2).
#[test]
fn the_first_life_gain_of_yours_each_turn() {
    let mut g = game();
    g.put(0, SERAPH, Zone::Battlefield);
    let stack = |g: &mut Game, player: usize| {
        while g.priority != player { g.act(g.priority, Move::Pass).unwrap(); }
        let card = g.put(player, HEAL, Zone::Hand);
        g.act(player, Move::Cast { card, targets: vec![] }).unwrap();
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        let triggered = g.stack.len();
        while !g.stack.is_empty() || g.decision.is_some() {
            if let Some(Decision::Choose { player, choice: game::state::Choice::Arrange { cards, .. } }) = g.decision.clone() { g.act(player, Move::Choose(game::state::Answer::Arrange { top: cards, away: vec![] })).unwrap(); } else { g.act(g.priority, Move::Pass).unwrap(); }
        }
        triggered
    };
    cite!(["603.2"]; assert_eq!(stack(&mut g, 1), 0, "an opponent's gain"));
    cite!(["603.2"]; assert_eq!(stack(&mut g, 0), 1, "the first"));
    cite!(["603.2"]; assert_eq!(stack(&mut g, 0), 0, "the second"));
    next_turn(&mut g);
    cite!(["603.2", "500.1"]; assert_eq!(stack(&mut g, 0), 1, "the first of another turn"));
}

/// "Whenever Aurelia attacks for the first time each turn, untap all creatures you control.
/// After this phase, there is an additional combat phase." In that combat, it attacks for the
/// second time: no trigger (603.2, 506.1).
#[test]
fn aurelia_attacks_a_second_time_in_her_additional_combat_without_triggering() {
    let mut g = game();
    let aurelia = g.put(0, AURELIA, Zone::Battlefield);
    to_attackers(&mut g, 0);
    g.act(0, Move::Attack(vec![(aurelia, Target::Player(1))])).unwrap();
    cite!(["603.2"]; assert_eq!(g.stack.len(), 1, "the first time"));
    for _ in 0..200 {
        if g.decision == Some(Decision::Attackers { player: 0 }) { break; }
        match g.decision { Some(Decision::Blockers { player }) => { g.act(player, Move::Block(vec![])).unwrap(); } _ => { g.act(g.priority, Move::Pass).unwrap(); } }
    }
    assert_eq!(g.decision, Some(Decision::Attackers { player: 0 }), "the additional combat");
    assert!(!g.obj(aurelia).tapped, "untapped by the trigger (vigilance anyway)");
    let life = g.players[1].life;
    g.act(0, Move::Attack(vec![(aurelia, Target::Player(1))])).unwrap();
    cite!(["603.2"]; assert!(g.stack.is_empty(), "the second time: no trigger"));
    assert!(life < 20, "the first combat's damage was dealt");
}

/// "As long as you attacked with three or more Merfolk this turn": the Merfolk declared as
/// attackers, as they were then, whatever became of them since; two are not three (508.1).
#[test]
fn you_attacked_with_three_merfolk_this_turn_though_one_is_gone() {
    for n in [2, 3] {
        let mut g = game();
        let navigator = g.put(0, NAVIGATOR, Zone::Battlefield);
        let merfolk: Vec<_> = (0..n).map(|_| g.put(0, MERFOLK, Zone::Battlefield)).collect();
        assert_eq!(g.power(navigator), Some(2));
        to_attackers(&mut g, 0);
        g.act(0, Move::Attack(merfolk.iter().map(|m| (*m, Target::Player(1))).collect())).unwrap();
        to_main_after_combat(&mut g, 0);
        cast(&mut g, 0, KILL, vec![vec![Target::Object(merfolk[0])]], free());
        cite!(["508.1", "608.2i"]; case: n; assert_eq!(g.power(navigator), Some(2 + i32::from(n == 3)), "the Navigator didn't attack; it is a Merfolk"));
        next_turn(&mut g);
        cite!(["500.1"]; case: n; assert_eq!(g.power(navigator), Some(2)));
    }
}

/// Tunnel Tipster: "At the beginning of your end step, if a face-down creature entered the
/// battlefield under your control this turn, put a +1/+1 counter on this creature." The
/// creature entered face down; that it was turned face up since doesn't change what entered
/// (608.2i; ruling 2024-02-02).
#[test]
fn a_face_down_creature_entered_though_it_is_face_up_now() {
    use crate::scenarios::r708_8_turned_face_up::{face_down, game as faces, put, turn_up};
    for turned in [false, true] {
        let mut t = faces(&["Tunnel Tipster", "Dog Walker"], &[]);
        while t.g.part != Part::PrecombatMain { t.g.act(t.g.priority, Move::Pass).unwrap(); }
        let tipster = put(&mut t, 0, "Tunnel Tipster", Zone::Battlefield);
        let walker = face_down(&mut t, 0, "Dog Walker");
        if turned { turn_up(&mut t, walker, game::face::Way::Disguise, "{R}{W}"); t.resolve(); }
        while t.g.part != Part::End { match t.g.decision { Some(Decision::Attackers { player }) => { t.g.act(player, Move::Attack(vec![])).unwrap(); } _ => { t.g.act(t.g.priority, Move::Pass).unwrap(); } } }
        t.resolve();
        cite!(["608.2i"]; case: turned; assert_eq!(t.g.power(tipster), Some(2), "a 1/1 with a counter: it entered face down (turned up: {turned})"));
    }
}


/// What entered face up and is face down now didn't enter face down (608.2i): Pine Walker,
/// cast, then turned face down by Backslide.
#[test]
fn a_creature_that_entered_face_up_and_was_turned_face_down_did_not_enter_face_down() {
    use crate::scenarios::r708_8_turned_face_up::{game as faces, put};
    let mut t = faces(&["Tunnel Tipster", "Pine Walker", "Backslide"], &[]);
    while t.g.part != Part::PrecombatMain { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    let tipster = put(&mut t, 0, "Tunnel Tipster", Zone::Battlefield);
    let walker = put(&mut t, 0, "Pine Walker", Zone::Hand);
    t.mana(0, "{3}{G}{G}");
    t.g.act(0, paid("CCCGG", Move::Cast { card: walker, targets: vec![] })).unwrap();
    t.resolve();
    let walker = t.find("Pine Walker");
    let backslide = put(&mut t, 0, "Backslide", Zone::Hand);
    t.mana(0, "{1}{U}");
    t.g.act(0, paid("CU", Move::Cast { card: backslide, targets: vec![vec![Target::Object(walker)]] })).unwrap();
    t.resolve();
    assert!(t.g.face_down(walker), "Backslide turned it face down");
    while t.g.part != Part::End { match t.g.decision { Some(Decision::Attackers { player }) => { t.g.act(player, Move::Attack(vec![])).unwrap(); } _ => { t.g.act(t.g.priority, Move::Pass).unwrap(); } } }
    t.resolve();
    cite!(["608.2i"]; assert_eq!(t.g.power(tipster), Some(1), "no face-down creature entered"));
}
