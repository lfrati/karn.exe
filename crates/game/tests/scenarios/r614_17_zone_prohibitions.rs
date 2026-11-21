//! CR 101.2, 601.3 and 614.17: source-zone restrictions override permissions and entry.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Target, Zone};
use game::simulation::Simulation;

const CAGE: &str = "Creature cards in graveyards and libraries can't enter the battlefield.\nPlayers can't cast spells from graveyards or libraries.";
fn defs() -> Vec<CardDef> {
    let defs = vec![
        CardDef::new("Grafdigger's Cage", "{1}", "Artifact", CAGE, None).unwrap(),
        CardDef::new("Flashback witness", "{1}", "Instant", "You gain 1 life.\nFlashback {1}", None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Return witness", "{0}", "Instant", "Return target creature card from your graveyard to the battlefield.", None).unwrap(),
        CardDef::new("Remove witness", "{0}", "Instant", "Destroy target artifact.", None).unwrap(),
        CardDef::new("Mass return witness", "{0}", "Sorcery", "Return all artifact and creature cards from all graveyards to the battlefield.", None).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap_or_else(|e| panic!("{}: {e:?}", def.name())); }
    defs
}
fn cast(sim: &mut Simulation, card: game::ObjId, targets: Vec<Vec<Target>>, spending: &str) {
    let r = sim.request().unwrap().unwrap().clone();
    sim.respond(r.id, r.player, paid(spending, Move::Cast { card, targets })).unwrap();
}
fn resolve(sim: &mut Simulation) {
    for _ in 0..sim.game().unwrap().players.len() {
        let r = sim.request().unwrap().unwrap().clone();
        sim.respond(r.id, r.player, Move::Pass).unwrap();
    }
}
#[test]
fn r601_3_origin_prohibition_overrides_flashback_for_every_player_and_preserves_the_command() {
    for player in 0..3 {
        let mut game = Game::new(defs(), 3, 20);
        game.priority = player;
        game.players[player].pool.colorless = 2;
        let cage = game.put((player + 1) % 3, 0, Zone::Battlefield);
        let grave = game.put(player, 1, Zone::Graveyard);
        let hand = game.put(player, 1, Zone::Hand);
        let remove = game.put(player, 4, Zone::Hand);
        let mut sim = Simulation::new(game);
        let r = sim.request().unwrap().unwrap().clone();
        let before = format!("{:?}", sim.game().unwrap());
        cite!(["601.3"]; refused("an effect prohibits casting that spell (601.3)"): sim.respond(r.id, player, paid("C",Move::CastVia(game::announcement::CastRoute::Flashback,Box::new(Move::Cast{card:grave,targets:vec![]})))));
        assert_eq!(format!("{:?}", sim.game().unwrap()), before);
        cast(&mut sim, hand, vec![], "C");
        resolve(&mut sim);
        assert_eq!(sim.game().unwrap().players[player].life, 21);
        // Remove the source through ordinary commands: the permission becomes usable again.
        // Resolution returns priority to the active player, so give the actor priority.
        while sim.request().unwrap().unwrap().player != player {
            let r = sim.request().unwrap().unwrap().clone();
            sim.respond(r.id, r.player, Move::Pass).unwrap();
        }
        cast(&mut sim, remove, vec![vec![Target::Object(cage)]], "");
        resolve(&mut sim);
        while sim.request().unwrap().unwrap().player != player {
            let r = sim.request().unwrap().unwrap().clone();
            sim.respond(r.id, r.player, Move::Pass).unwrap();
        }
        let r = sim.request().unwrap().unwrap().clone();
        sim.respond(r.id, player, paid("C",Move::CastVia(game::announcement::CastRoute::Flashback,Box::new(Move::Cast{card:grave,targets:vec![]})))).unwrap();
        resolve(&mut sim);
        assert_eq!(sim.game().unwrap().players[player].life, 22);
    }
}
#[test]
fn r614_17_a_legal_return_target_stays_in_its_graveyard_without_entry_events() {
    let mut game = Game::new(defs(), 2, 20);
    game.put(1, 0, Zone::Battlefield);
    let bear = game.put(0, 2, Zone::Graveyard);
    let spell = game.put(0, 3, Zone::Hand);
    let normal = game.put(0, 2, Zone::Hand);
    let mut sim = Simulation::new(game);
    cast(&mut sim, spell, vec![vec![Target::Object(bear)]], "");
    resolve(&mut sim);
    cite!(["101.2", "614.17"]; assert_eq!(sim.game().unwrap().obj(bear).zone, Zone::Graveyard));
    assert!(sim.game().unwrap().players[0].graveyard.contains(&bear));
    assert_eq!(sim.game().unwrap().battlefield.len(), 1);
    cast(&mut sim, normal, vec![], "");
    resolve(&mut sim);
    assert_eq!(sim.game().unwrap().battlefield.len(), 2, "ordinary creature spell resolves from stack");
}
#[test]
fn r614_17a_a_simultaneously_entering_prohibition_does_not_stop_its_batch() {
    let mut game = Game::new(defs(), 3, 20);
    let cage = game.put(1, 0, Zone::Graveyard);
    let a = game.put(0, 2, Zone::Graveyard);
    let b = game.put(2, 2, Zone::Graveyard);
    let spell = game.put(0, 5, Zone::Hand);
    let mut sim = Simulation::new(game);
    cast(&mut sim, spell, vec![], "");
    resolve(&mut sim);
    let game = sim.game().unwrap();
    cite!(["614.17a"]; assert_eq!(game.battlefield.len(), 3));
    for (owner, def) in [(1, 0), (0, 2), (2, 2)] {
        assert!(game.battlefield.iter().any(|o| game.obj(*o).owner == owner && game.obj(*o).card == Some(def)));
    }
    assert!(!game.players[0].graveyard.contains(&a));
    assert!(!game.players[1].graveyard.contains(&cage));
    assert!(!game.players[2].graveyard.contains(&b));
}
#[test]
fn r614_17_a_present_prohibition_filters_simultaneous_returns_by_source_characteristics() {
    let mut game = Game::new(defs(), 3, 20);
    game.put(0, 0, Zone::Battlefield);
    let artifact = game.put(2, 0, Zone::Graveyard);
    let a = game.put(0, 2, Zone::Graveyard);
    let b = game.put(1, 2, Zone::Graveyard);
    let spell = game.put(0, 5, Zone::Hand);
    let mut sim = Simulation::new(game);
    cast(&mut sim, spell, vec![], "");
    resolve(&mut sim);
    let game = sim.game().unwrap();
    cite!(["101.2", "614.17"]; assert_eq!(game.battlefield.len(), 2));
    cite!(["101.2", "614.17"]; assert!(game.players[0].graveyard.contains(&a)));
    assert!(game.players[1].graveyard.contains(&b));
    assert!(!game.players[2].graveyard.contains(&artifact));
}
#[test]
fn unsupported_prospective_entry_and_cast_limit_restrictions_remain_refused() {
    // "Players can't cast creature spells." is supported (r201_4a_named_characteristics)
    for text in ["Creatures can't enter the battlefield.", "Players can't cast more than one spell each turn."] {
        let def = CardDef::new("Unsupported witness", "{0}", "Artifact", text, None).unwrap();
        let reason=if text.starts_with("Creatures") {"entry prohibition: only cards described in their source zones"}
            else {"prohibition: a window of its own"};
        crate::support::refused(game::play::check(&def).map_err(|e|e.0),reason);
    }
    let def = CardDef::new("Unsupported witness", "{0}", "Creature — Bear", "This creature can't enter the battlefield.", Some((2, 2))).unwrap();
    crate::support::refused(game::play::check(&def).map_err(|e|e.0),"entry prohibition: only cards described in their source zones");
}

#[test]
fn r614_17_a_searched_creature_stays_in_its_library_across_managed_resumption() {
    let mut definitions = defs();
    definitions.push(CardDef::new("Search witness", "{0}", "Sorcery", "Search your library for a creature card, put it onto the battlefield, then shuffle.", None).unwrap());
    game::play::check(definitions.last().unwrap()).unwrap();
    let mut game = Game::new(definitions, 2, 20);
    game.put(1, 0, Zone::Battlefield);
    let bear = game.put(0, 2, Zone::Library);
    let spell = game.put(0, 6, Zone::Hand);
    let mut sim = Simulation::new(game);
    cast(&mut sim, spell, vec![], "");
    resolve(&mut sim);
    let request = sim.request().unwrap().unwrap().clone();
    let mut branch = sim.clone();
    for sim in [&mut sim, &mut branch] {
        sim.respond(request.id, request.player, Move::Choose(game::state::Answer::Objects(vec![bear]))).unwrap();
        let game = sim.game().unwrap();
        // Shuffling renews hidden library identities; the selected card remains in
        // that zone and never becomes a permanent.
        let [remaining] = game.players[0].library[..] else { panic!("one card remains in the library"); };
        assert_eq!(game.obj(remaining).card, Some(2));
        cite!(["101.2", "614.17"]; assert_eq!(game.obj(remaining).zone, Zone::Library));
        assert_eq!(game.battlefield.len(), 1);
        assert!(!game.sees(1, remaining), "the prohibition does not expose the chosen card");
    }
}
