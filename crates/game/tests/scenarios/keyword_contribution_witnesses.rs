//! Full-card witnesses for separate keyword contributions (113.6): a hand activation feeding a
//! hand replacement, a graveyard route after a hand activation, and a granted stack destination
//! beside a printed flashback route.
use crate::support::{cite, paid};
use game::legal::Asked;
use game::state::{Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

fn full(name: &str) -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let d = CardDef::from_faces(&CardDef::catalog(&faces)[name]).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
    d
}
fn land() -> CardDef { CardDef::new("Land", "", "Basic Land — Island", "", None).unwrap() }
fn bear() -> CardDef { CardDef::new("Bear", "{G}", "Creature — Bear", "", Some((2, 2))).unwrap() }
/// Card 0 under test, card 1 a land stocking every library.
fn stocked(defs: Vec<CardDef>) -> Game {
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..12 { g.put(p, 1, Zone::Library); } }
    g
}
fn pass(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }
fn activated(g: &Game, o: ObjId) -> usize {
    g.abilities(o).iter().position(|a| matches!(&**a, ir::Ability::Activated { .. })).expect("an activated keyword ability")
}
fn upkeep(g: &mut Game) {
    let turn = g.turn;
    for _ in 0..200 {
        if g.turn > turn && g.active == 0 && g.part == Part::Upkeep { return; }
        match g.decision {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("no upkeep");
}
fn of(g: &Game, zone: &[ObjId]) -> Vec<ObjId> { zone.iter().copied().filter(|o| g.obj(*o).card == Some(0)).collect() }

/// Ichor Slick: cycling (702.29a) discards it from a hand, where madness's replacement
/// functions (702.35a), so it is exiled and its trigger goes above the cycling ability
/// (603.3); cast for its madness cost, it resolves first, then cycling draws.
#[test]
fn ichor_slick_cycled_is_discarded_into_exile_and_cast_for_madness() {
    let mut g = stocked(vec![full("Ichor Slick"), land(), bear()]);
    let bear = g.put(1, 2, Zone::Battlefield);
    let slick = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors = [10; 5];
    let cycling = activated(&g, slick);
    g.act(0, paid("WW",Move::Activate { source: slick, ability: cycling, targets: vec![], paying: vec![] })).unwrap();
    let exiled = of(&g, &g.exile.clone());
    cite!(["702.35a"]; assert_eq!(exiled.len(), 1, "discarded into exile, not the graveyard"));
    assert!(of(&g, &g.players[0].graveyard.clone()).is_empty());
    cite!(["702.35a", "603.3"]; assert_eq!(g.stack.len(), 2, "madness trigger above the cycling ability"));
    assert_eq!(g.obj(g.stack[1]).ability.and_then(|(s, _)| s), Some(exiled[0]));
    pass(&mut g);
    let Some((0, Asked::Cast { cards })) = g.asked() else { panic!("no madness offer: {:?}", g.asked()) };
    assert!(cards.contains(&exiled[0]));
    let cost = game::announcement::CastCost::Alternative(2); // Ichor Slick's madness row.
    let hand = g.players[0].hand.len();
    g.act(0, paid("BBBB",Move::CastFor(cost, Box::new(Move::Cast { card: exiled[0], targets: vec![vec![Target::Object(bear)]] })))).unwrap();
    pass(&mut g);
    cite!(["702.35a", "405.5"]; assert_eq!(g.obj(bear).zone, Zone::Gone, "-3/-3 resolved before the cycling draw"));
    assert_eq!(g.players[0].hand.len(), hand);
    pass(&mut g);
    cite!(["702.29a"]; assert_eq!(g.players[0].hand.len(), hand + 1, "cycling then draws"));
    assert_eq!(of(&g, &g.players[0].graveyard.clone()).len(), 1, "a madness spell resolves to the graveyard");
}

/// Marshaling Cry: cycling from a hand (702.29a) puts it into the graveyard, where its separate
/// flashback route functions (702.34a); cast that way it is exiled as it resolves.
#[test]
fn marshaling_cry_cycled_then_flashed_back_is_exiled() {
    let mut g = stocked(vec![full("Marshaling Cry"), land(), bear()]);
    let bear = g.put(0, 2, Zone::Battlefield);
    let cry = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors = [10; 5];
    let cycling = activated(&g, cry);
    let hand = g.players[0].hand.len();
    g.act(0, paid("WW",Move::Activate { source: cry, ability: cycling, targets: vec![], paying: vec![] })).unwrap();
    pass(&mut g);
    cite!(["702.29a"]; assert_eq!(g.players[0].hand.len(), hand, "discarded one, drew one"));
    let [grave] = of(&g, &g.players[0].graveyard.clone())[..] else { panic!("cycled into the graveyard") };
    g.players[0].pool.colors = [10; 5];
    g.act(0,paid("WWWW",Move::CastVia(game::announcement::CastRoute::Flashback,Box::new(Move::Cast{card:grave,targets:vec![]})))).unwrap();
    pass(&mut g);
    assert_eq!(g.power(bear), Some(3));
    assert!(g.has(bear, "Vigilance"));
    cite!(["702.34a"]; assert_eq!(of(&g, &g.exile.clone()).len(), 1, "flashback exiles it (702.34a)"));
    assert!(of(&g, &g.players[0].graveyard.clone()).is_empty());
}

/// Cast Through Time grants rebound to stack spells. Cast from a hand, Marshaling Cry rebounds
/// (702.88a); cast from exile it goes to the graveyard (neither rebound nor an unpaid
/// flashback applies); flashed back from there it is exiled with no further rebound.
#[test]
fn granted_rebound_and_printed_flashback_apply_only_to_their_own_casts() {
    let mut g = stocked(vec![full("Marshaling Cry"), land(), full("Cast Through Time")]);
    g.put(0, 2, Zone::Battlefield);
    let cry = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors = [10; 5];
    g.act(0, paid("WWW",Move::Cast { card: cry, targets: vec![] })).unwrap();
    pass(&mut g);
    let [exiled] = of(&g, &g.exile.clone())[..] else { panic!("rebound exile: stack={:?} grave={:?} exile={:?} decision={:?}", g.stack, of(&g, &g.players[0].graveyard.clone()), g.exile, g.decision) };
    upkeep(&mut g);
    cite!(["702.88a"]; assert_eq!(g.stack.len(), 1, "rebound's delayed trigger"));
    pass(&mut g);
    cite!(["702.88a"]; assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::Cast { .. } }))));
    g.act(0, Move::Cast { card: exiled, targets: vec![] }).unwrap();
    pass(&mut g);
    cite!(["702.88a", "702.34a"]; assert!(of(&g, &g.exile.clone()).is_empty(), "cast from exile: no rebound, no flashback exile"));
    let [grave] = of(&g, &g.players[0].graveyard.clone())[..] else { panic!("resolved to the graveyard") };
    while g.part != Part::PrecombatMain || !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    g.players[0].pool.colors = [10; 5];
    g.act(0,paid("WWWW",Move::CastVia(game::announcement::CastRoute::Flashback,Box::new(Move::Cast{card:grave,targets:vec![]})))).unwrap();
    assert!(g.has(g.stack[0], "Rebound"), "the grant applies to the flashback spell too");
    pass(&mut g);
    cite!(["702.34a"]; assert_eq!(of(&g, &g.exile.clone()).len(), 1, "flashback exiles it"));
    upkeep(&mut g);
    cite!(["702.88a"]; assert!(g.stack.is_empty(), "not cast from a hand: no rebound trigger"));
}

/// CR 122.1b has a finite keyword-counter list: banding and soulbond counters
/// are named markers. A raw fixture can retain an unimplemented decayed keyword
/// without inventing executable abilities; admitting its placement remains refused.
#[test]
fn unsupported_keyword_counter_does_not_panic_reading_abilities() {
    for kind in ["flying", "banding", "soulbond", "decayed"] {
        let mut g = stocked(vec![bear(), land()]);
        let bear = g.put(0, 0, Zone::Battlefield);
        g.put_counters(bear, kind, 1);
        let name = kind[..1].to_uppercase() + &kind[1..];
        // Any panic fails this ordinary assertion; no exception is accepted as a refusal.
        let keyword_counter = matches!(kind, "flying" | "decayed");
        cite!(["122.1b"]; case: kind; assert_eq!(g.has(bear, &name), keyword_counter));
        assert!(g.abilities(bear).is_empty(), "no executable ability is invented");
        let cloned = g.clone();
        assert_eq!(cloned.has(bear, &name), keyword_counter, "cloning preserves the counter's actual meaning");
        let placement = CardDef::new("Counter placement", "{0}", "Instant",
            &format!("Put a {kind} counter on target creature."), None).unwrap();
        if kind == "decayed" {
            crate::support::refused(game::play::check(&placement).map_err(|e| e.0), "keyword counter: Decayed");
        } else {
            // Uncited (admission): supported flying and cosmetic named counters compile.
            game::play::check(&placement).unwrap();
        }
        if kind != "flying" {
            let printed = CardDef::new("Unsupported", "{0}", "Creature — Bear", &name, Some((2, 2))).unwrap();
            crate::support::refused(game::play::check(&printed).map_err(|e|e.0),&format!("keyword: {name}"));
            let text = format!("Target creature gains {} until end of turn.", kind);
            let grant = CardDef::new("Unsupported grant", "{0}", "Instant", &text, None).unwrap();
            crate::support::refused(game::play::check(&grant).map_err(|e|e.0),&format!("keyword: {name}"));
        }
    }
}
