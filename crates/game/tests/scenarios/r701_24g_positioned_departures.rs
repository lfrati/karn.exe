//! One event can both shuffle a library and put cards at specified positions in it. "Destroy
//! all creatures" destroys them at once (608.2f), and each creature's own replacement gives
//! its modified event (614.6): Progenitus's "reveal Progenitus and shuffle it into its owner's
//! library instead" shuffles the library that Gravebane Zombie's "put it on top of its owner's
//! library instead" and Nissa's Chosen's "put it on the bottom of its owner's library instead"
//! put them in. The result is that library shuffled except that each of those is where it was
//! put (701.24g). Every card is read from its Oracle text.
use crate::support::{cite, oracle, paid};
use game::simulation::{ResponseError, Simulation};
use game::start::ShuffleOrder;
use game::{Event, Game, Move, ObjId, Zone};

const WRATH: usize = 0;
const PROGENITUS: usize = 1;
const GRAVEBANE: usize = 2;
const CHOSEN: usize = 3;
const PLAINS: usize = 4;
const ISLAND: usize = 5;
const FOREST: usize = 6;

/// Player 0 controls these creatures and has cast Wrath of God {2}{W}{W} ("Destroy all
/// creatures. They can't be regenerated."), and has passed; their library is a Plains, an
/// Island and a Forest, the Plains on top. The creatures and the library cards, in those orders.
fn game(creatures: &[usize]) -> (Game, Vec<ObjId>, Vec<ObjId>) {
    let defs = Vec::from(["Wrath of God", "Progenitus", "Gravebane Zombie", "Nissa's Chosen", "Plains", "Island", "Forest"].map(oracle));
    let mut g = Game::new(defs, 2, 20);
    let mut library: Vec<ObjId> = [FOREST, ISLAND, PLAINS].iter().map(|def| g.put(0, *def, Zone::Library)).collect();
    library.reverse();
    let creatures = creatures.iter().map(|def| g.put(0, *def, Zone::Battlefield)).collect();
    for _ in 0..4 {
        let land = g.put(0, PLAINS, Zone::Battlefield);
        g.act(0, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
    let wrath = g.put(0, WRATH, Zone::Hand);
    g.act(0, paid("WWWW", Move::Cast { card: wrath, targets: vec![] })).unwrap();
    g.act(0, Move::Pass).unwrap();
    (g, creatures, library)
}

/// Player 1 passes and Wrath of God resolves, the shuffles of player 0's library having these
/// outcomes, each the library top first.
fn resolved(g: &Game, outcomes: Vec<Vec<ObjId>>) -> (Simulation, Result<(), ResponseError>) {
    let mut sim = Simulation::new(g.clone());
    let request = sim.request().unwrap().unwrap().clone();
    assert_eq!(request.player, 1);
    let orders = outcomes.into_iter().map(|order| ShuffleOrder { player: 0, order }).collect();
    let result = sim.respond_with_shuffles(request.id, 1, Move::Pass, orders);
    (sim, result)
}

/// Player 0's library by name, the top card first.
fn library(g: &Game) -> Vec<String> { g.players[0].library.iter().rev().map(|c| g.obj(*c).chars.name.clone()).collect() }
fn graveyard(g: &Game) -> Vec<String> { g.players[0].graveyard.iter().map(|c| g.obj(*c).chars.name.clone()).collect() }
fn shuffles(g: &Game) -> usize { g.log.iter().filter(|e| matches!(e, Event::Shuffled { player: 0, .. })).count() }
fn revealed(g: &Game, card: ObjId) -> bool { g.log.iter().any(|e| matches!(e, Event::Revealed { cards, .. } if cards.contains(&card))) }

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    let _ = game(&[]);
}

#[test]
fn r701_24g_cards_put_on_top_and_on_the_bottom_by_their_own_replacements_stay_there_as_progenitus_is_shuffled_in() {
    // the positioned cards are destroyed before Progenitus in the event's own order
    let (g, creatures, cards) = game(&[GRAVEBANE, CHOSEN, PROGENITUS]);
    let [gravebane, chosen, progenitus] = creatures[..] else { unreachable!() };
    let [plains, island, forest] = cards[..] else { unreachable!() };
    let before = format!("{g:?}");

    // An outcome that moves the Gravebane Zombie off the top is not one this shuffle can
    // have (701.24g), and is refused whole.
    let (sim, result) = resolved(&g, vec![vec![island, gravebane, progenitus, plains, forest, chosen]]);
    cite!(["701.24g"]; assert!(matches!(&result, Err(ResponseError::ReplayInput(why)) if why.0.contains("set-aside")), "{result:?}"));
    assert_eq!(format!("{:?}", sim.game().unwrap()), before);
    // nor one that moves Nissa's Chosen off the bottom
    let (_, result) = resolved(&g, vec![vec![gravebane, island, progenitus, plains, chosen, forest]]);
    cite!(["701.24g"]; assert!(matches!(&result, Err(ResponseError::ReplayInput(_))), "{result:?}"));

    // the other four cards, Progenitus among them, in the order the shuffle gives them
    let (sim, result) = resolved(&g, vec![vec![island, progenitus, plains, forest]]);
    result.unwrap();
    let after = sim.game().unwrap();
    assert!(after.battlefield.iter().all(|o| after.obj(*o).chars.name == "Plains"), "every creature is gone");
    // Progenitus can be destroyed by an effect that doesn't target it (its 2009-02-01 ruling)
    cite!(["614.6"]; assert_eq!(graveyard(after), ["Wrath of God"], "none of them was put into a graveyard"));
    cite!(["701.24a"]; assert!(revealed(after, progenitus) && shuffles(after) == 1));
    cite!(["701.24g"]; assert_eq!(library(after), ["Gravebane Zombie", "Island", "Progenitus", "Plains", "Forest", "Nissa's Chosen"]));

    // the outcome may also name them, where they are
    let (sim, result) = resolved(&g, vec![vec![gravebane, forest, plains, progenitus, island, chosen]]);
    result.unwrap();
    cite!(["701.24g"]; assert_eq!(library(sim.game().unwrap()), ["Gravebane Zombie", "Forest", "Plains", "Progenitus", "Island", "Nissa's Chosen"]));
}

#[test]
fn r701_24g_with_the_games_own_shuffle_the_positioned_cards_stay_whatever_order_the_creatures_die_in() {
    for order in [[GRAVEBANE, CHOSEN, PROGENITUS], [PROGENITUS, GRAVEBANE, CHOSEN], [CHOSEN, PROGENITUS, GRAVEBANE]] {
        let (mut g, _, _) = game(&order);
        g.act(1, Move::Pass).unwrap();
        let after = library(&g);
        cite!(["701.24g"]; case: format!("{order:?}"); assert_eq!((after[0].as_str(), after[5].as_str()), ("Gravebane Zombie", "Nissa's Chosen")));
        let mut rest = after[1..5].to_vec();
        rest.sort();
        assert_eq!(rest, ["Forest", "Island", "Plains", "Progenitus"]);
        cite!(["701.24a"]; case: format!("{order:?}"); assert_eq!(shuffles(&g), 1));
    }
}
