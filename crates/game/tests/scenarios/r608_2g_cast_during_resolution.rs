//! Casting a spell while an effect is applied (608.2g): "You may cast that card without
//! paying its mana cost." The card is cast then or not at all, ignoring the timing its type
//! gives it; its mana cost is not paid, its other costs are (118.9), and X is 0 (107.3b). The
//! spell goes on the stack above what is resolving, which goes on to its end before any
//! player receives priority.
use crate::support::{cite, paid};
use game::legal::{Asked, Step};
use game::state::{Answer, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const GIFT: usize = 0;
const MOUNTAIN: usize = 1;
const BEAR: usize = 2;
const BOLT: usize = 3;
const BLAZE: usize = 4;
const ALTAR: usize = 5;
const WATCHER: usize = 6;
const TWIN: usize = 7;
const FOUNT: usize = 8;

fn game() -> Game {
    let defs = vec![
        CardDef::new("Gift", "{R}", "Instant", "Exile the top card of your library. You may cast that card without paying its mana cost. You gain 1 life.", None).unwrap(),
        CardDef::new("Mountain", "", "Basic Land — Mountain", "", None).unwrap(),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Lightning Bolt", "{R}", "Instant", "Lightning Bolt deals 3 damage to any target.", None).unwrap(),
        CardDef::new("Blaze", "{X}{R}", "Sorcery", "Blaze deals X damage to any target.", None).unwrap(),
        CardDef::new("Altar Bolt", "{4}{R}", "Sorcery", "As an additional cost to cast this spell, sacrifice a creature.\nAltar Bolt deals 5 damage to any target.", None).unwrap(),
        CardDef::new("Watcher", "{W}", "Enchantment", "Whenever you cast a spell, you gain 2 life.", None).unwrap(),
        CardDef::new("Twin Gift", "{R}", "Instant", "Exile the top two cards of your library. You may cast those cards without paying their mana costs.", None).unwrap(),
        CardDef::new("Fount", "{0}", "Artifact Creature — Construct", "{T}: Add {G}. Exile this creature. You may cast it without paying its mana cost.", Some((0, 1))).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    Game::new(defs, 2, 20)
}
fn pass_all(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
/// Gift is resolving with this card exiled from the top of the library, in the opponent's
/// turn when `theirs`. Returns the exiled card, if the offer is made.
fn offered(top: usize, theirs: bool, setup: impl FnOnce(&mut Game)) -> (Game, Option<ObjId>) {
    let mut g = game();
    for player in 0..2 { for _ in 0..5 { g.put(player, MOUNTAIN, Zone::Library); } }
    if theirs { while g.active == 0 { g.act(g.priority, Move::Pass).unwrap(); } while g.priority != 0 { g.act(g.priority, Move::Pass).unwrap(); } }
    g.put(0, top, Zone::Library);
    setup(&mut g);
    let gift = g.put(0, GIFT, Zone::Hand);
    g.players[0].pool.colors[3] = 1;
    g.act(0, paid("R", Move::Cast { card: gift, targets: vec![] })).unwrap();
    while g.asked().is_some_and(|(_, a)| a == Asked::Priority) && !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    let card = g.exile.iter().copied().find(|o| g.obj(*o).card == Some(top));
    (g, card)
}
fn asked_cast(g: &mut Game) -> Vec<ObjId> {
    match g.asked() { Some((0, Asked::Cast { cards })) => cards, other => panic!("not an offer to cast: {other:?}") }
}

#[test]
fn a_creature_is_cast_for_nothing_in_the_opponents_turn_and_the_effect_goes_on() {
    let (mut g, bear) = offered(BEAR, true, |_| {});
    let bear = bear.unwrap();
    let cards = asked_cast(&mut g);
    assert_eq!(cards, vec![bear]);
    crate::support::refused(g.act(1, Move::Cast { card: bear, targets: vec![] }), "that decision is another player's");
    cite!(["608.2g"]; refused("a decision is pending: cast one of the cards offered, or decline (608.2g)"): g.act(0, Move::Pass));
    g.act(0, Move::Cast { card: bear, targets: vec![] }).unwrap();
    // the rest of Gift happened before anyone could respond, and the Bear is a spell
    cite!(["608.2g"]; assert_eq!(g.players[0].life, 21));
    cite!(["608.2g"]; assert_eq!(g.stack.len(), 1));
    assert!(g.exile.is_empty());
    assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).card == Some(GIFT)));
    cite!(["608.2g", "117.3b"]; assert_eq!(g.asked().map(|(_, a)| a), Some(Asked::Priority)));
    pass_all(&mut g);
    assert!(g.battlefield.iter().any(|o| g.obj(*o).card == Some(BEAR)));
}

#[test]
fn declined_it_stays_exiled_and_cannot_be_cast_later() {
    let (mut g, bear) = offered(BEAR, false, |_| {});
    let bear = bear.unwrap();
    g.act(0, Move::Choose(Answer::No)).unwrap();
    assert_eq!(g.players[0].life, 21);
    assert!(g.exile.contains(&bear));
    g.players[0].pool.colors[4] = 2;
    cite!(["601.3"]; refused("that card is not in your hand: it is in exile, and no rule or effect lets you cast it from there (601.3)"): g.act(0, Move::Cast { card: bear, targets: vec![] }));
}

#[test]
fn a_land_is_not_offered() {
    let (g, land) = offered(MOUNTAIN, false, |_| {});
    assert!(land.is_some());
    assert_eq!(g.players[0].life, 21, "nothing was asked");
}

#[test]
fn targets_are_chosen_and_an_illegal_cast_changes_nothing() {
    let (mut g, bolt) = offered(BOLT, false, |_| {});
    let bolt = bolt.unwrap();
    let before = format!("{g:?}");
    cite!(["601.2c"]; refused("that has 1 target group(s), 0 given"): g.act(0, Move::Cast { card: bolt, targets: vec![] }));
    let other = g.players[0].library[0];
    cite!(["608.2g"]; refused("the effect offers other cards to cast (608.2g)"): g.act(0, Move::Cast { card: other, targets: vec![] }));
    assert_eq!(format!("{g:?}"), before);
    cite!(["608.2g", "601.2c"]; ok: g.act(0, Move::Cast { card: bolt, targets: vec![vec![Target::Player(1)]] }));
    assert_eq!(g.stack.len(), 1);
    pass_all(&mut g);
    assert_eq!(g.players[1].life, 17);
}

#[test]
fn x_is_zero_and_additional_costs_are_paid() {
    let (mut g, blaze) = offered(BLAZE, false, |_| {});
    let blaze = blaze.unwrap();
    let cast = Move::Cast { card: blaze, targets: vec![vec![Target::Player(1)]] };
    cite!(["107.3b"]; refused("X is 0 for a spell cast without paying its mana cost (107.3b)"): g.act(0, Move::WithX(2, Box::new(cast.clone()))));
    cite!(["107.3b"]; ok: g.act(0, cast));
    pass_all(&mut g);
    cite!(["107.3b"]; assert_eq!(g.players[1].life, 20));

    // with no creature to sacrifice it is not offered a cast; with one, the creature is paid
    let (mut g, altar) = offered(ALTAR, false, |_| {});
    let cards = asked_cast(&mut g);
    cite!(["118.9d", "601.2h"]; assert_eq!(cards, vec![altar.unwrap()]));
    cite!(["601.2h"]; refused("that selection does not satisfy the cost, which asks for exactly 1 card(s): 0 chosen (601.2h, 701.21a)"): g.act(0, Move::Cast { card: altar.unwrap(), targets: vec![vec![Target::Player(1)]] }));
    g.act(0, Move::Choose(Answer::No)).unwrap();

    let mut creature = None;
    let (mut g, altar) = offered(ALTAR, false, |g| creature = Some(g.put(0, BEAR, Zone::Battlefield)));
    let (altar, creature) = (altar.unwrap(), creature.unwrap());
    g.act(0, Move::WithCosts(vec![vec![creature]], Box::new(Move::Cast { card: altar, targets: vec![vec![Target::Player(1)]] }))).unwrap();
    cite!(["118.9d"]; assert!(!g.battlefield.contains(&creature)));
    pass_all(&mut g);
    assert_eq!(g.players[1].life, 15);
}

#[test]
fn it_was_cast_and_a_copy_of_the_game_resumes_the_same() {
    let (mut g, bear) = offered(BEAR, false, |g| { g.put(0, WATCHER, Zone::Battlefield); });
    let bear = bear.unwrap();
    // Gift itself was cast: its trigger has resolved or waits below; note the life now
    let life = g.players[0].life;
    let mut copy = g.clone();
    g.act(0, Move::Cast { card: bear, targets: vec![] }).unwrap();
    copy.act(0, Move::Cast { card: bear, targets: vec![] }).unwrap();
    assert_eq!(format!("{g:?}"), format!("{copy:?}"));
    // Gift finished (+1) and the cast trigger waits on the stack above the Bear spell
    cite!(["608.2g"]; assert_eq!(g.players[0].life, life + 1));
    cite!(["608.2g", "603.3"]; assert_eq!(g.stack.len(), 2));
    pass_all(&mut g);
    assert_eq!(g.players[0].life, life + 3);
}

/// This effect is resolving with these cards on top of the library, the last topmost, and a
/// Fount on the battlefield: a mana ability that itself offers a cast.
fn nested(effect: usize, library: &[usize]) -> (Game, ObjId) {
    let mut g = game();
    for player in 0..2 { for _ in 0..5 { g.put(player, MOUNTAIN, Zone::Library); } }
    for card in library { g.put(0, *card, Zone::Library); }
    let fount = g.put(0, FOUNT, Zone::Battlefield);
    let spell = g.put(0, effect, Zone::Hand);
    g.players[0].pool.colors[3] = 1;
    g.act(0, paid("R", Move::Cast { card: spell, targets: vec![] })).unwrap();
    while g.asked().is_some_and(|(_, a)| a == Asked::Priority) && !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    (g, fount)
}
fn tap(fount: ObjId, mv: Move) -> Move {
    Move::WithMana(vec![Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source: fount, ability: 0, paying: vec![], adding: vec![] }], Box::new(mv))
}
fn exiled(g: &Game, card: usize) -> Vec<ObjId> { g.exile.iter().copied().filter(|o| g.obj(*o).card == Some(card)).collect() }

#[test]
fn an_offer_made_while_a_cast_is_paid_for_is_of_its_own_cards_and_ends_alone() {
    let (mut g, fount) = nested(TWIN, &[BLAZE, BOLT]);
    let (bolt, blaze) = (exiled(&g, BOLT)[0], exiled(&g, BLAZE)[0]);
    let mut cards = asked_cast(&mut g);
    cards.sort();
    assert_eq!(cards, { let mut both = vec![bolt, blaze]; both.sort(); both });
    // the Bolt is cast with mana of the Fount, whose ability exiles it and offers it
    g.act(0, tap(fount, Move::Cast { card: bolt, targets: vec![vec![Target::Player(1)]] })).unwrap();
    let fount = exiled(&g, FOUNT)[0];
    cite!(["608.2g"]; assert_eq!(asked_cast(&mut g), vec![fount], "the Blaze is another effect's to offer"));
    cite!(["608.2g"]; refused("the effect offers other cards to cast (608.2g)"): g.act(0, Move::Cast { card: blaze, targets: vec![vec![Target::Player(1)]] }));
    g.act(0, Move::Choose(Answer::No)).unwrap();
    // the Fount's ability is over, the Bolt is cast, and Twin Gift still offers the Blaze
    cite!(["608.2g"]; assert_eq!(asked_cast(&mut g), vec![blaze]));
    cite!(["608.2g"]; refused("the effect offers other cards to cast (608.2g)"): g.act(0, Move::Cast { card: fount, targets: vec![] }));
    g.act(0, Move::Cast { card: blaze, targets: vec![vec![Target::Player(1)]] }).unwrap();
    assert_eq!(g.asked().map(|(p, a)| (p, a == Asked::Priority)), Some((0, true)));
    assert_eq!((g.stack.len(), g.players[0].pool.colors[4]), (2, 1));
    assert_eq!(g.exile, vec![fount]);
    pass_all(&mut g);
    pass_all(&mut g);
    assert_eq!(g.players[1].life, 17);
}

#[test]
fn r733_1_a_cast_abandoned_as_it_is_paid_for_is_offered_again() {
    let (mut g, creature) = nested(GIFT, &[ALTAR]);
    let (altar, life) = (exiled(&g, ALTAR)[0], g.players[0].life);
    assert_eq!(asked_cast(&mut g), vec![altar]);
    // the Fount is to be sacrificed for it, and is tapped for mana first: its ability exiles it
    let cast = Move::WithCosts(vec![vec![creature]], Box::new(Move::Cast { card: altar, targets: vec![vec![Target::Player(1)]] }));
    g.act(0, tap(creature, cast)).unwrap();
    let fount = exiled(&g, FOUNT)[0];
    assert_eq!(asked_cast(&mut g), vec![fount]);
    g.act(0, Move::Choose(Answer::No)).unwrap();
    // the cast cannot be paid for and is reversed (733.1)
    assert!(matches!(g.decision, Some(Decision::Payment { .. })), "{:?}", g.decision);
    let copy = g.clone();
    crate::support::refused(g.act(1, Move::ReversePayment), "that decision is another player's");
    g.act(0, Move::ReversePayment).unwrap();
    cite!(["733.1"]; assert_eq!(g.obj(altar).zone, Zone::Exile));
    assert!(!g.stack.iter().any(|o| g.obj(*o).card == Some(ALTAR)));
    assert_eq!(g.players[0].pool.colors[4], 1, "mana of a legal mana ability stays");
    // Gift offers it again: no creature is left to pay with, and it is declined
    let cards = asked_cast(&mut g);
    assert_eq!(cards, vec![altar]);
    cite!(["601.2h"]; refused("that selection does not satisfy the cost, which asks for exactly 1 card(s): 0 chosen (601.2h, 701.21a)"): g.act(0, Move::Cast { card: altar, targets: vec![vec![Target::Player(1)]] }));
    cite!(["608.2g"]; refused("the effect offers other cards to cast (608.2g)"): g.act(0, Move::Cast { card: fount, targets: vec![] }));
    g.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["608.2g"]; assert_eq!(g.players[0].life, life + 1, "the effect went on"));
    assert_eq!(g.asked().map(|(p, a)| (p, a == Asked::Priority)), Some((0, true)));
    assert_eq!((g.exile.len(), g.stack.len()), (2, 0));
    cite!(["601.3"]; refused("that card is not in your hand: it is in exile, and no rule or effect lets you cast it from there (601.3)"): g.act(0, Move::Cast { card: altar, targets: vec![vec![Target::Player(1)]] }));
    // a copy of the game reverses and resumes the same
    let mut copy = copy;
    copy.act(0, Move::ReversePayment).unwrap();
    copy.act(0, Move::Choose(Answer::No)).unwrap();
    assert_eq!(format!("{g:?}"), format!("{copy:?}"));
}
