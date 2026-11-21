//! Permission to play an exiled card until a deadline: "Exile the top card of your library.
//! You may play it this turn." (Chandra, Pyromaster). The permission is the effect; playing
//! the card follows every other rule of playing it (305.2, 601.3, 117.1a), and it is about
//! the card as it is in exile (400.7).
use crate::support::{cite,paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const CHANDRA: usize = 0;
const MOUNTAIN: usize = 1;
const BEAR: usize = 2;
const BOLT: usize = 3;
const CASTER: usize = 4;

fn game() -> Game {
    let defs = vec![
        CardDef::planeswalker("Chandra", "{2}{R}{R}", "Legendary Planeswalker — Chandra", "0: Exile the top card of your library. You may play it this turn.", 4).unwrap(),
        CardDef::new("Mountain", "", "Basic Land — Mountain", "", None).unwrap(),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Lightning Bolt", "{R}", "Instant", "Lightning Bolt deals 3 damage to any target.", None).unwrap(),
        CardDef::new("Caster", "{R}", "Sorcery", "Exile the top card of your library. You may cast it this turn.", None).unwrap(),
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
/// Chandra's ability has resolved with this card on top of the library. Returns the card as
/// it is in exile.
fn exiled(top: usize) -> (Game, ObjId) {
    let mut g = game();
    g.put(0, top, Zone::Library);
    let chandra = g.put(0, CHANDRA, Zone::Battlefield);
    g.set_initial_counters(chandra, "loyalty", 4);
    g.act(0, Move::Activate { source: chandra, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    pass_all(&mut g);
    assert!(g.asked().is_some_and(|(p, a)| p == 0 && matches!(a, game::legal::Asked::Priority)), "nothing is asked as it resolves");
    let card = *g.exile.iter().find(|o| g.obj(**o).card == Some(top)).expect("the top card was exiled");
    (g, card)
}
#[test]
fn a_land_is_played_from_exile_as_the_land_for_the_turn() {
    let (mut g, land) = exiled(MOUNTAIN);
    cite!(["116.2a"]; assert_eq!((g.clone().act(0, Move::PlayLand(land)).is_ok(), g.clone().act(0, Move::Cast { card: land, targets: vec![] }).is_ok()), (true, false)));
    crate::support::refused(g.act(1, Move::PlayLand(land)), "that player does not have priority");
    // with priority, the other player is still not the one the effect lets play it
    let mut o = g.clone();
    o.act(0, Move::Pass).unwrap();
    cite!(["305.1"]; refused("an effect lets another player play that card, not you (305.1)"): o.act(1, Move::PlayLand(land)));
    g.act(0, Move::PlayLand(land)).unwrap();
    assert!(g.exile.is_empty());
    cite!(["305.1"]; assert!(g.battlefield.iter().any(|o| g.obj(*o).card == Some(MOUNTAIN))));
    cite!(["305.2"]; assert!(g.players[0].land_played, "it is the land played this turn (305.2)"));

    // not after a land was played from the hand
    let (mut g, land) = exiled(MOUNTAIN);
    let hand = g.put(0, MOUNTAIN, Zone::Hand);
    g.act(0, Move::PlayLand(hand)).unwrap();
    cite!(["305.2"]; refused("you have played a land this turn (305.2)"): g.act(0, Move::PlayLand(land)));
    cite!(["305.2b"]; assert_eq!((g.clone().act(0, Move::PlayLand(land)).is_ok(), g.clone().act(0, Move::Cast { card: land, targets: vec![] }).is_ok()), (false, false)));
}

#[test]
fn a_spell_is_cast_from_exile_for_its_cost_at_its_timing() {
    let (mut g, bear) = exiled(BEAR);
    assert_eq!((g.clone().act(0, Move::PlayLand(bear)).is_ok(), g.clone().act(0, paid("GG",Move::Cast { card: bear, targets: vec![] })).is_ok()), (false, false), "its cost cannot be paid");
    cite!(["118.3a"]; refused("your mana pool cannot pay that cost (118.3a): {1}{G}"): g.act(0, Move::Cast { card: bear, targets: vec![] }));
    assert!(g.exile.contains(&bear), "a cast that cannot be paid leaves it in exile");
    g.players[0].pool.colors[4] = 2;
    cite!(["601.3"]; assert_eq!((g.clone().act(0, Move::PlayLand(bear)).is_ok(), g.clone().act(0, paid("GG",Move::Cast { card: bear, targets: vec![] })).is_ok()), (false, true)));
    crate::support::refused(g.act(1, paid("GG",Move::Cast { card: bear, targets: vec![] })), "that player does not have priority");
    let mut o = g.clone();
    o.act(0, Move::Pass).unwrap();
    cite!(["601.3"]; refused("an effect lets another player cast that card, not you (601.3)"): o.act(1, paid("GG",Move::Cast { card: bear, targets: vec![] })));
    g.act(0, paid("GG",Move::Cast { card: bear, targets: vec![] })).unwrap();
    assert!(g.exile.is_empty());
    pass_all(&mut g);
    assert!(g.battlefield.iter().any(|o| g.obj(*o).card == Some(BEAR)));

    // a creature spell only at sorcery timing (117.1a); an instant whenever it has priority
    let (mut g, bear) = exiled(BEAR);
    let bolt = g.put(0, BOLT, Zone::Hand);
    g.players[0].pool.colors[3] = 1;
    g.players[0].pool.colors[4] = 2;
    g.act(0, paid("R",Move::Cast { card: bolt, targets: vec![vec![Target::Player(1)]] })).unwrap();
    cite!(["117.1a"]; refused("that is cast in a main phase of your own turn with the stack empty (117.1a)"): g.act(0, paid("GG",Move::Cast { card: bear, targets: vec![] })));
    let (mut g, bolt) = exiled(BOLT);
    let other = g.put(0, BOLT, Zone::Hand);
    g.players[0].pool.colors[3] = 2;
    g.act(0, paid("R",Move::Cast { card: other, targets: vec![vec![Target::Player(1)]] })).unwrap();
    cite!(["117.1a", "601.3"]; ok: g.act(0, paid("R",Move::Cast { card: bolt, targets: vec![vec![Target::Player(1)]] })));
    pass_all(&mut g);
    pass_all(&mut g);
    assert_eq!(g.players[1].life, 14);
    cite!(["608.2n"]; assert!(g.players[0].graveyard.iter().filter(|o| g.obj(**o).card == Some(BOLT)).count() == 2, "it goes where a resolved spell goes"));
}

#[test]
fn the_permission_ends_with_the_turn_and_with_the_object() {
    let (mut g, bear) = exiled(BEAR);
    let later = g.clone();
    g.put(0, MOUNTAIN, Zone::Library);
    g.put(1, MOUNTAIN, Zone::Library);
    // a main phase of its player's next turn: a Bear in hand could be cast
    let hand = g.put(0, BEAR, Zone::Hand);
    for _ in 0..100 {
        if g.turn == 3 && g.part == ir::Part::PrecombatMain && g.priority == 0 { break; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    assert_eq!((g.turn, g.part, g.priority), (3, ir::Part::PrecombatMain, 0));
    g.players[0].pool.colors[4] = 2;
    assert!((g.clone().act(0, Move::PlayLand(hand)).is_ok(), g.clone().act(0, paid("GG",Move::Cast { card: hand, targets: vec![] })).is_ok()).1, "the same creature in hand can be cast");
    assert!(g.exile.contains(&bear));
    cite!(["601.3"]; refused("that card is not in your hand: it is in exile, and no rule or effect lets you cast it from there (601.3)"): g.act(0, paid("GG",Move::Cast { card: bear, targets: vec![] })));
    cite!(["611.2a"]; assert_eq!((g.clone().act(0, Move::PlayLand(bear)).is_ok(), g.clone().act(0, paid("GG",Move::Cast { card: bear, targets: vec![] })).is_ok()), (false, false)));

    // a copy of the game made while the permission lasts carries it
    let mut g = later;
    g.players[0].pool.colors[4] = 2;
    cite!(["611.2a"]; assert_eq!((g.clone().act(0, Move::PlayLand(bear)).is_ok(), g.clone().act(0, paid("GG",Move::Cast { card: bear, targets: vec![] })).is_ok()), (false, true)));
    g.act(0, paid("GG",Move::Cast { card: bear, targets: vec![] })).unwrap();
}

#[test]
fn permission_to_cast_is_not_permission_to_play_a_land() {
    let mut g = game();
    g.put(0, MOUNTAIN, Zone::Library);
    let caster = g.put(0, CASTER, Zone::Hand);
    g.players[0].pool.colors[3] = 1;
    g.act(0, paid("R",Move::Cast { card: caster, targets: vec![] })).unwrap();
    pass_all(&mut g);
    let land = *g.exile.first().expect("a Mountain was exiled");
    cite!(["305.9"]; refused("an effect lets you cast that card, not play it as a land"): g.act(0, Move::PlayLand(land)));
    cite!(["305.9"]; refused("a land is played, not cast (305.9)"): g.act(0, Move::Cast { card: land, targets: vec![] }));
    cite!(["305.9"]; assert_eq!((g.clone().act(0, Move::PlayLand(land)).is_ok(), g.clone().act(0, Move::Cast { card: land, targets: vec![] }).is_ok()), (false, false)));
}
