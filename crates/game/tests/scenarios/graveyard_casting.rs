//! A card's own zone permission changes where it can be cast, not timing or costs (113.6f, 601.3).
use crate::support::{cite,paid};
use game::{CardDef, Game, Move, Zone};

fn returning(types: &str, text: &str) -> CardDef {
    CardDef::new("Returning", "{0}", types, text, types.contains("Creature").then_some((2, 2))).unwrap()
}
fn game() -> Game {
    let defs = vec![returning("Creature — Spirit", "You may cast this card from your graveyard.")];
    game::play::check(&defs[0]).unwrap();
    Game::new(defs, 3, 20)
}
fn pass(g: &mut Game) {
    for _ in 0..3 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
#[test]
fn r113_6f_self_casting_permission_works_from_the_owners_graveyard_and_normal_hand() {
    for zone in [Zone::Hand, Zone::Graveyard] {
        let mut g = game();
        let card = g.put(0, 0, zone);
        cite!(["113.6f"]; case: format!("{zone:?}"); assert!(g.clone().act(g.priority, Move::Cast { card: card, targets: vec![] }).is_ok()));
        cite!(["113.6f"]; case: format!("{zone:?}"); ok: g.act(0, Move::Cast { card, targets: vec![] }));
        assert_eq!(g.obj(card).zone, Zone::Gone);
        let spell = g.stack[0];
        assert_eq!(g.obj(spell).owner, 0);
        assert_eq!(g.obj(spell).controller, 0);
        pass(&mut g);
        assert_eq!(g.battlefield.len(), 1);
    }
}
#[test]
fn r113_6f_self_casting_permission_does_not_authorize_other_players_or_zones() {
    for (zone, place) in [(Zone::Library, "in a library"), (Zone::Exile, "in exile"), (Zone::Battlefield, "on the battlefield")] {
        let mut g = game();
        let card = g.put(0, 0, zone);
        cite!(["601.3"]; case: place; assert!(!g.clone().act(g.priority, Move::Cast { card: card, targets: vec![] }).is_ok()));
        let before = format!("{g:?}");
        let why = format!("it is {place}, and no rule or effect lets you cast it from there (601.3)");
        cite!(["601.3"]; refused(&why): g.act(0, Move::Cast { card, targets: vec![] }));
        assert_eq!(format!("{g:?}"), before);
    }
    let mut g = game();
    let card = g.put(0, 0, Zone::Graveyard);
    g.active = 1;
    g.priority = 1;
    cite!(["601.3", "109.5"]; assert!(!g.clone().act(g.priority, Move::Cast { card: card, targets: vec![] }).is_ok()));
    let before = format!("{g:?}");
    cite!(["601.3"]; refused("it is in a graveyard, and no rule or effect lets you cast it from there (601.3)"): g.act(1, Move::Cast { card, targets: vec![] }));
    assert_eq!(format!("{g:?}"), before);
}
#[test]
fn r601_3_graveyard_permission_preserves_normal_casting_timing() {
    let mut g = game();
    let card = g.put(0, 0, Zone::Graveyard);
    g.part = ir::Part::Upkeep;
    cite!(["117.1a"]; assert!(!g.clone().act(g.priority, Move::Cast { card: card, targets: vec![] }).is_ok()));
    cite!(["117.1a"]; refused("cast in a main phase of your own turn with the stack empty (117.1a)"): g.act(0, Move::Cast { card, targets: vec![] }));
    g.part = ir::Part::PrecombatMain;
    g.put(1, 0, Zone::Stack);
    cite!(["117.1a"]; assert!(!g.clone().act(g.priority, Move::Cast { card: card, targets: vec![] }).is_ok()));
    cite!(["117.1a"]; refused("cast in a main phase of your own turn with the stack empty (117.1a)"): g.act(0, Move::Cast { card, targets: vec![] }));
}
#[test]
fn r113_3a_graveyard_permission_is_not_a_resolving_spell_instruction() {
    let d = returning("Instant", "You may cast this card from your graveyard.\nYou gain 2 life.");
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d], 3, 20);
    let mut card = g.put(0, 0, Zone::Graveyard);
    for expected in [22, 24] {
        g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
        pass(&mut g);
        cite!(["113.3a"]; case: expected; assert_eq!(g.players[0].life, expected));
        assert!(g.decision.is_none());
        assert!(g.exile.is_empty(), "ordinary graveyard permission is not flashback");
        card = *g.players[0].graveyard.last().unwrap();
    }
}
#[test]
fn r701_6_graveyard_casting_permission_does_not_replace_countering_with_exile() {
    let mut g = game();
    let card = g.put(0, 0, Zone::Graveyard);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    let spell = g.stack[0];
    assert!(g.counter(spell));
    cite!(["701.6a"]; assert!(g.exile.is_empty()));
    let returned = *g.players[0].graveyard.last().unwrap();
    assert_ne!(card, returned);
    cite!(["113.6f"]; assert!(g.clone().act(g.priority, Move::Cast { card: returned, targets: vec![] }).is_ok()));
    cite!(["400.7"]; assert!(!g.clone().act(g.priority, Move::Cast { card: card, targets: vec![] }).is_ok()));
}
#[test]
fn r601_2h_skaab_ruinator_pays_normal_and_additional_costs_from_either_zone() {
    let defs = vec![
        CardDef::new("Skaab Ruinator", "{1}{U}{U}", "Creature — Zombie Horror", "As an additional cost to cast this spell, exile three creature cards from your graveyard.\nFlying\nYou may cast this card from your graveyard.", Some((5,6))).unwrap(),
        CardDef::new("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2,2))).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap();
    }
    for zone in [Zone::Hand, Zone::Graveyard] {
        let mut g = Game::new(defs.clone(), 3, 20);
        let card = g.put(0, 0, zone);
        let islands: Vec<_> = (0..3).map(|_| g.put(0, 2, Zone::Battlefield)).collect();
        let mut food = vec![g.put(0, 1, Zone::Graveyard), g.put(0, 1, Zone::Graveyard)];
        cite!(["601.2a"]; case: format!("{zone:?}"); assert!(!g.clone().act(g.priority, Move::Cast { card: card, targets: vec![] }).is_ok(), "the announced spell cannot exile itself as a third card"));
        food.push(g.put(0, 1, Zone::Graveyard));
        let plan: Vec<_> = islands.iter().copied().map(|source| game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source, ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None }).collect();
        let cast = |chosen| {
            Move::WithMana(
                plan.clone(),
                Box::new(Move::WithCosts(vec![chosen], Box::new(paid("UUU",Move::Cast { card, targets: vec![] })))),
            )
        };
        let before = format!("{g:?}");
        cite!(["601.2a"]; refused("Skaab Ruinator is not what that cost asks for"): g.act(0, cast(vec![food[0], food[1], card])));
        assert_eq!(format!("{g:?}"), before);
        cite!(["601.2h"]; case: format!("{zone:?}"); ok: g.act(0, cast(food)));
        assert_eq!(g.exile.len(), 3);
        assert_eq!(g.players[0].pool.total(), 0);
        pass(&mut g);
        let creature = *g.battlefield.last().unwrap();
        assert_eq!(g.seen(creature, None).pt, Some((5, 6)));
        assert!(g.destroy(creature));
        assert_eq!(g.players[0].graveyard.len(), 1);
        let returned = g.players[0].graveyard[0];
        cite!(["118.3"]; case: format!("{zone:?}"); assert!(!g.clone().act(g.priority, Move::Cast { card: returned, targets: vec![] }).is_ok(), "another cast needs fresh payment resources"));
    }
}

#[test]
fn r601_3_graveyard_permission_composes_with_flash_on_an_opponents_turn() {
    let d = returning("Creature — Spirit", "Flash\nYou may cast this card from your graveyard.");
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d], 3, 20);
    g.active = 1;
    let card = g.put(0, 0, Zone::Graveyard);
    cite!(["702.8a", "113.6f"]; assert!(g.clone().act(g.priority, Move::Cast { card: card, targets: vec![] }).is_ok()));
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    pass(&mut g);
    let creature = g.battlefield[0];
    assert_eq!(g.obj(creature).owner, 0);
    assert_eq!(g.obj(creature).controller, 0);
}
#[test]
fn r109_5_you_on_a_graveyard_card_means_its_owner_in_multiplayer() {
    let d = returning("Instant", "You may cast this card from your graveyard.\nYou gain 2 life.");
    let mut g = Game::new(vec![d], 4, 20);
    g.priority = 2;
    let card = g.put(2, 0, Zone::Graveyard);
    cite!(["109.5"]; assert!(g.clone().act(g.priority, Move::Cast { card: card, targets: vec![] }).is_ok()));
    g.act(2, Move::Cast { card, targets: vec![] }).unwrap();
    let spell = g.stack[0];
    assert_eq!(g.obj(spell).owner, 2);
    assert_eq!(g.obj(spell).controller, 2);
    for _ in 0..4 {
        g.act(g.priority, Move::Pass).unwrap();
    }
    cite!(["109.5"]; assert_eq!(g.players.iter().map(|p| p.life).collect::<Vec<_>>(), vec![20, 20, 22, 20]));
    assert_eq!(g.players[2].graveyard.len(), 1);
}
#[test]
fn r113_6f_unmodeled_casting_permissions_are_refused_instead_of_losing_their_restrictions() {
    for (text, expected) in [
        ("You may cast this card from exile.", "ability on a permanent: statement: Cast"),
        ("You may cast this card from your graveyard without paying its mana cost.", "ability on a permanent: statement: Cast"),
        ("You may cast this card from your graveyard this turn.", "ability on a permanent: Cast with modifier Duration"),
        ("As long as you control a Zombie, you may cast this card from your graveyard.", "ability on a permanent: lead: Modifier"),
        ("You may cast creature cards from your graveyard.", "ability on a permanent: static: PlayFrom"),
        ("You may play this card from your graveyard.", "ability on a permanent: statement: Cast"),
    ] {
        let d = returning("Creature — Spirit", text);
        let reason=game::play::check(&d).expect_err("unsupported static permission").0;
        assert_eq!(reason,expected, "{text}");
    }
}
#[test]
fn r118_9_ordinary_graveyard_permission_and_flashback_have_distinct_casting_routes() {
    let d = returning("Instant", "You may cast this card from your graveyard.\nYou gain 2 life.\nFlashback {1}");
    game::play::check(&d).unwrap();
    for route in [game::announcement::CastRoute::Normal, game::announcement::CastRoute::Flashback] {
        let mut g = Game::new(vec![d.clone()], 2, 20);
        let card = g.put(0, 0, Zone::Graveyard);
        g.players[0].pool.colorless = 1;
        g.act(0, paid(if route==game::announcement::CastRoute::Flashback {"C"}else{""},Move::CastVia(route, Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
        for _ in 0..2 { g.act(g.priority,Move::Pass).unwrap(); }
        assert_eq!(g.players[0].life, 22);
        cite!(["702.34a"]; case: format!("{route:?}"); assert_eq!(g.exile.len(), usize::from(route == game::announcement::CastRoute::Flashback)));
        cite!(["118.9"]; case: format!("{route:?}"); assert_eq!(g.players[0].pool.total(), u32::from(route == game::announcement::CastRoute::Normal)));
    }
}
