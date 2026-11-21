//! Bilbo's attack-trigger offer (608.2g) beside Lurrus's
//! once-each-turn graveyard permission (601.3) and Bilbo's own cost reduction (601.2f).
//!
//! Lurrus: "Once during each of your turns, you may cast a permanent spell with mana value 2 or
//! less from your graveyard." Bilbo: "Whenever Bilbo attacks, you may cast an artifact,
//! instant, or sorcery spell from your graveyard." A spell cast while Bilbo's trigger resolves
//! is cast because that effect allows it (608.2g); Lurrus's permission is not what lets it be
//! cast then, and its one use this turn stays available.
use crate::support::cite;
use game::announcement::CastRoute;
use game::legal::Asked;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::{Color, mana::ManaSymbol};

const BILBO: usize = 0;
const LURRUS: usize = 1;
const TRINKET: usize = 2;
const BEAR: usize = 3;
const ISLAND: usize = 4;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let oracle = |name: &str| {
        let d = CardDef::from_faces(CardDef::catalog(&faces).get(name).unwrap()).unwrap();
        game::play::check(&d).unwrap();
        d
    };
    let defs = vec![
        oracle("Bilbo, Thief in the Night"),
        oracle("Lurrus of the Dream-Den"),
        CardDef::new("Trinket", "{2}", "Artifact", "", None).unwrap(),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
    ];
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    g
}
fn attack(g: &mut Game, bilbo: ObjId) {
    while !matches!(g.asked(), Some((0, Asked::Attackers(_)))) { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Attack(vec![(bilbo, Target::Player(1))])).unwrap();
    while matches!(g.asked(), Some((_, Asked::Priority))) && !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
}
fn resolve(g: &mut Game) { while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); } }
/// On to player 0's postcombat main phase with an empty stack.
fn second_main(g: &mut Game) {
    while !(g.active == 0 && g.part == ir::Part::PostcombatMain && g.stack.is_empty()) {
        match g.asked() {
            Some((p, Asked::Blockers { .. })) => g.act(p, Move::Block(vec![])).unwrap(),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
}
fn graveyard(g: &Game, card: usize) -> ObjId { *g.players[0].graveyard.iter().find(|o| g.obj(**o).card == Some(card)).unwrap() }
fn on_battlefield(g: &Game, card: usize) -> bool { g.battlefield.iter().any(|o| g.obj(*o).card == Some(card)) }

#[test]
fn r608_2g_a_cast_offered_by_bilbo_does_not_spend_lurrus_once_each_turn_permission() {
    let mut g = game();
    let bilbo = g.put(0, BILBO, Zone::Battlefield);
    let lurrus = g.put(0, LURRUS, Zone::Battlefield);
    g.put(0, TRINKET, Zone::Graveyard);
    g.put(0, BEAR, Zone::Graveyard);
    attack(&mut g, bilbo);
    let Some((0, Asked::Cast { .. })) = g.asked() else { panic!("{:?}", g.asked()) };
    let trinket = graveyard(&g, TRINKET);
    // {2} less Bilbo's {1}, cast as the offer is answered, with no route said
    g.add_mana(0, ManaSymbol::Colorless);
    cite!(["608.2g", "601.2f"]; ok: g.act(0, crate::support::paid("C", Move::Cast { card: trinket, targets: vec![] })));
    resolve(&mut g);
    cite!(["608.2g"]; assert!(on_battlefield(&g, TRINKET)));
    // later this turn, Lurrus's one cast from the graveyard is still there
    second_main(&mut g);
    let bear = graveyard(&g, BEAR);
    g.add_mana(0, ManaSymbol::Color(Color::Green));
    g.act(0, crate::support::paid("G", Move::CastVia(CastRoute::Permission { source: lurrus, ability: 2 }, Box::new(Move::Cast { card: bear, targets: vec![] }))))
        .expect("Lurrus's permission was not used by the cast Bilbo's trigger offered");
    resolve(&mut g);
    assert!(on_battlefield(&g, BEAR));
}

#[test]
fn r608_2g_bilbos_resolution_refuses_a_lurrus_permission_route() {

    let mut g = game();
    let bilbo = g.put(0, BILBO, Zone::Battlefield);
    let lurrus = g.put(0, LURRUS, Zone::Battlefield);
    let trinket = g.put(0, TRINKET, Zone::Graveyard);
    attack(&mut g, bilbo);
    g.add_mana(0, ManaSymbol::Colorless);
    let Some((0, Asked::Cast { cards })) = g.asked() else { panic!("{:?}", g.asked()) };
    assert_eq!(cards,vec![trinket]);
    let command = crate::support::paid("C", Move::Cast { card: trinket, targets: vec![] });
    let before=format!("{g:?}");
    cite!(["608.2g"]; refused("that casting permission is unavailable (601.3)"): g.act(0, Move::CastVia(CastRoute::Permission { source: lurrus, ability: 2 }, Box::new(command.clone()))));
    assert_eq!(format!("{g:?}"),before);
    cite!(["608.2g"]; ok: g.act(0, command));
    assert_eq!(g.players[0].pool.total(),0);
    resolve(&mut g);
    assert!(g.stack.is_empty() && on_battlefield(&g,TRINKET));
}

#[test]
fn positive_controls_lurrus_with_bilbo_reduction_and_bilbo_after_lurrus_is_spent() {
    // Lurrus's permission in the main phase: {1}{G} less Bilbo's {1} is {G}
    let mut g = game();
    let bilbo = g.put(0, BILBO, Zone::Battlefield);
    let lurrus = g.put(0, LURRUS, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Graveyard);
    g.put(0, TRINKET, Zone::Graveyard);
    g.add_mana(0, ManaSymbol::Color(Color::Green));
    g.act(0, crate::support::paid("G", Move::CastVia(CastRoute::Permission { source: lurrus, ability: 2 }, Box::new(Move::Cast { card: bear, targets: vec![] })))).unwrap();
    cite!(["601.2f"]; assert_eq!(g.players[0].pool.total(), 0));
    resolve(&mut g);
    assert!(on_battlefield(&g, BEAR));
    // Lurrus's use is spent; Bilbo's offer still casts the Trinket
    attack(&mut g, bilbo);
    let trinket = graveyard(&g, TRINKET);
    g.add_mana(0, ManaSymbol::Colorless);
    g.act(0, crate::support::paid("C", Move::Cast { card: trinket, targets: vec![] })).unwrap();
    resolve(&mut g);
    cite!(["608.2g"]; assert!(on_battlefield(&g, TRINKET)));
}

#[test]
fn r608_2g_explicit_static_permission_cannot_borrow_an_effects_timing() {
    let mut g = game();
    let bilbo = g.put(0, BILBO, Zone::Battlefield);
    let lurrus = g.put(0, LURRUS, Zone::Battlefield);
    g.put(0, TRINKET, Zone::Graveyard);
    attack(&mut g, bilbo);
    let trinket = graveyard(&g, TRINKET);
    g.add_mana(0, ManaSymbol::Colorless);
    let before = g.decision.clone();
    cite!(["601.3", "608.2g"]; refused("that casting permission is unavailable (601.3)"): g.act(0, Move::CastVia(
        CastRoute::Permission { source: lurrus, ability: 2 },
        Box::new(crate::support::paid("C", Move::Cast { card: trinket, targets: vec![] })),
    )));
    assert_eq!(g.decision, before);
    assert_eq!(g.players[0].pool.total(), 1);
    assert_eq!(g.obj(trinket).zone, Zone::Graveyard);
    // Rejection leaves the effect's valid offer available, with no paid or replayed action.
    g.act(0, crate::support::paid("C", Move::Cast { card: trinket, targets: vec![] })).unwrap();
    resolve(&mut g);
    cite!(["608.2g"]; assert!(on_battlefield(&g, TRINKET)));
}

/// The fix excludes offered cards from Lurrus's route. A graveyard card Bilbo does not offer
/// (a creature) but Lurrus would allow is still not castable mid-resolution with Lurrus's
/// route: Lurrus follows normal timing (ruling 2020-04-17). Declining the offer keeps Lurrus's
/// one use for the main phase.
#[test]
fn r608_2g_an_unoffered_lurrus_card_waits_for_normal_timing_and_declining_keeps_the_use() {
    let mut g = game();
    let bilbo = g.put(0, BILBO, Zone::Battlefield);
    let lurrus = g.put(0, LURRUS, Zone::Battlefield);
    g.put(0, TRINKET, Zone::Graveyard);
    g.put(0, BEAR, Zone::Graveyard);
    attack(&mut g, bilbo);
    let Some((0, Asked::Cast { cards, .. })) = g.asked() else { panic!("{:?}", g.asked()) };
    let bear = graveyard(&g, BEAR);
    cite!(["608.2g"]; assert!(!cards.contains(&bear), "a creature card is not Bilbo's to offer"));
    g.add_mana(0, ManaSymbol::Color(Color::Green));
    for mv in [
        crate::support::paid("G", Move::CastVia(CastRoute::Permission { source: lurrus, ability: 2 }, Box::new(Move::Cast { card: bear, targets: vec![] }))),
        Move::Cast { card: bear, targets: vec![] },
    ] {
        cite!(["608.2g"]; refused("the effect offers other cards to cast (608.2g)"): g.act(0, mv));
        assert_eq!(g.obj(bear).zone, Zone::Graveyard);
    }
    g.act(0, Move::Choose(game::state::Answer::No)).unwrap();
    second_main(&mut g);
    // mana empties between steps (500.4): {1}{G} less Bilbo's {1}
    g.add_mana(0, ManaSymbol::Color(Color::Green));
    g.act(0, crate::support::paid("G", Move::CastVia(CastRoute::Permission { source: lurrus, ability: 2 }, Box::new(Move::Cast { card: bear, targets: vec![] })))).expect("Lurrus's use is still there after declining");
    resolve(&mut g);
    assert!(on_battlefield(&g, BEAR));
}
