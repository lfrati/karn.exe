//! Instructions another player follows (608.2d) and searches of other zones (701.23):
//! what each player sees of hidden cards, one shuffle per search (701.23h, 701.24a), searches
//! made at once (701.23i, 608.2e), an attack trigger whose creature is gone (113.7a, 508.5) and
//! "that many" counting tokens (Settle the Wreckage rulings). Every card is read from its
//! Oracle text.
use crate::support::{cite, paid};
use game::binding::Value;
use game::legal::Asked;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, Target, Zone};
use ir::Part;

const NEGOTIATION: usize = 0;
const VENDETTA: usize = 1;
const BOLT: usize = 2;
const ISLAND: usize = 3;
const CRUSHER: usize = 4;
const FOREST: usize = 5;
const FIELD: usize = 6;
const PLAINS: usize = 7;
const BEARS: usize = 8;
const SETTLE: usize = 9;
const BETRAYAL: usize = 10;

fn game(players: usize) -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let defs: Vec<_> = ["Ruthless Negotiation", "Ancient Vendetta", "Lightning Bolt", "Island", "Ulamog's Crusher", "Forest", "Field of Ruin", "Plains", "Grizzly Bears", "Settle the Wreckage", "Strategic Betrayal"]
        .map(|n| CardDef::from_faces(&catalog[n]).unwrap()).to_vec();
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, players, 20);
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}

fn pass(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }

fn shuffled(g: &Game) -> Vec<usize> {
    g.log.iter().filter_map(|e| match e { Event::Shuffled { player, .. } => Some(*player), _ => None }).collect()
}

/// Player 0 casts this spell at player 1 with plenty of mana; it resolves until asked.
fn cast_at_opponent(g: &mut Game, spell: usize, spending: &str) {
    let c = g.put(0, spell, Zone::Hand);
    g.players[0].pool.colors = [3, 3, 3, 3, 3];
    g.act(0, paid(spending, Move::Cast { card: c, targets: vec![vec![Target::Player(1)]] })).unwrap();
    pass(g);
}

#[test]
fn ruthless_negotiation_the_caster_sees_neither_the_hand_nor_the_choice_before_the_exile() {
    let mut g = game(2);
    let hand = [g.put(1, BOLT, Zone::Hand), g.put(1, ISLAND, Zone::Hand)];
    cast_at_opponent(&mut g, NEGOTIATION, "B");
    let Some(Decision::Choose { player, choice: Choice::Objects { among, count, .. } }) = g.decision.clone() else { panic!("{:?}", g.decision) };
    cite!(["608.2d"]; assert_eq!((player, count), (1, 1), "the opponent chooses"));
    assert_eq!(among, hand.to_vec());
    // their hand stays hidden from the caster while they choose (400.2)
    cite!(["400.2"]; assert!(hand.iter().all(|c| !g.sees(0, *c) && g.sees(1, *c))));
    g.act(1, Move::Choose(Answer::Objects(vec![hand[1]]))).unwrap();
    // the card exiled is public; the one kept is still hidden
    let exiled = *g.exile.last().unwrap();
    cite!(["406.3"]; assert!(g.sees(0, exiled) && g.obj(exiled).chars.name == "Island"));
    assert!(!g.sees(0, hand[0]));
}

#[test]
fn ancient_vendetta_the_searcher_sees_the_hidden_zones_and_the_owner_shuffles_once() {
    // "Choose a card name. Search target opponent's graveyard, hand, and library for up to
    // four cards with that name and exile them. Then that player shuffles."
    let mut g = game(2);
    let library = [g.put(1, BOLT, Zone::Library), g.put(1, ISLAND, Zone::Library)];
    let hand = g.put(1, BOLT, Zone::Hand);
    let grave = g.put(1, BOLT, Zone::Graveyard);
    cast_at_opponent(&mut g, VENDETTA, "WBBB");
    g.act(0, Move::Choose(Answer::Value(Value::CardName("Lightning Bolt".into())))).unwrap();
    let Some(Decision::Choose { player: 0, choice: Choice::Search { cards, among, .. } }) = g.decision.clone() else { panic!("{:?}", g.decision) };
    cite!(["701.23a"]; assert!([grave, hand, library[0], library[1]].iter().all(|c| cards.contains(c)), "every card of the zones searched"));
    assert_eq!(among.len(), 3);
    // the searcher looks at the hidden cards; their owner doesn't look through their library
    cite!(["701.23a"]; assert!(library.iter().all(|c| g.sees(0, *c)) && g.sees(0, hand)));
    cite!(["701.23a"]; assert!(library.iter().all(|c| !g.sees(1, *c))));
    g.act(0, Move::Choose(Answer::Objects(vec![library[0], hand]))).unwrap();
    while !g.stack.is_empty() && g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["701.23h", "701.24a"]; assert_eq!(shuffled(&g), vec![1], "one shuffle, of that player's library"));
    assert_eq!(g.obj(grave).zone, Zone::Graveyard, "not chosen, not exiled");
    // what moved is a new object there (400.7): one Bolt from the library, one from the hand
    assert_eq!((g.exile.len(), g.players[1].hand.len(), g.players[1].library.len()), (2, 0, 1));
    assert!(g.exile.iter().all(|o| g.obj(*o).chars.name == "Lightning Bolt"));
}

#[test]
fn annihilator_its_creature_gone_before_the_trigger_resolves_still_names_the_player() {
    // 113.7a: the trigger resolves without its source; the defending player is the one the
    // Crusher was attacking (508.5)
    let mut g = game(3);
    let one = g.put(1, FOREST, Zone::Battlefield);
    let two = [g.put(2, FOREST, Zone::Battlefield), g.put(2, FOREST, Zone::Battlefield), g.put(2, FOREST, Zone::Battlefield)];
    let crusher = g.put(0, CRUSHER, Zone::Battlefield);
    while g.part != Part::DeclareAttackers { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Attack(vec![(crusher, Target::Player(2))])).unwrap();
    assert_eq!(g.stack.len(), 1, "the annihilator trigger");
    g.move_object(crusher, Zone::Graveyard);
    while !g.stack.is_empty() && g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
    let Some(Decision::Choose { player, choice: Choice::Objects { among, count, .. } }) = g.decision.clone() else { panic!("{:?}", g.decision) };
    cite!(["508.5", "113.7a"]; assert_eq!((player, count, among), (2, 2, two.to_vec())));
    g.act(2, Move::Choose(Answer::Objects(two[..2].to_vec()))).unwrap();
    assert!(g.battlefield.contains(&one) && g.battlefield.contains(&two[2]));
}

#[test]
fn field_of_ruin_what_the_first_player_found_stays_in_their_library_while_the_next_chooses() {
    // 608.2e, 701.23i: the choices are made in APNAP order, then the cards are put onto the
    // battlefield at once
    let mut g = game(2);
    let plains = g.put(0, PLAINS, Zone::Library);
    let forest = g.put(1, FOREST, Zone::Library);
    let field = g.put(0, FIELD, Zone::Battlefield);
    let target = g.put(1, FIELD, Zone::Battlefield);
    g.players[0].pool.colors = [0, 0, 0, 0, 2];
    g.act(0, paid("GG", Move::Activate { source: field, ability: 1, targets: vec![vec![Target::Object(target)]], paying: vec![] })).unwrap();
    while !g.stack.is_empty() && g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
    let Some((0, Asked::Search { .. })) = g.asked() else { panic!("{:?}", g.asked()) };
    g.act(0, Move::Choose(Answer::Objects(vec![plains]))).unwrap();
    let Some((1, Asked::Search { .. })) = g.asked() else { panic!("{:?}", g.asked()) };
    cite!(["608.2e", "701.23i"]; assert_eq!(g.obj(plains).zone, Zone::Library, "not yet put onto the battlefield"));
    assert!(!g.sees(1, plains));
    g.act(1, Move::Choose(Answer::Objects(vec![forest]))).unwrap();
    let on = |name: &str, p: usize| g.battlefield.iter().filter(|o| g.obj(**o).chars.name == name && g.controller(**o) == p).count();
    assert_eq!((on("Plains", 0), on("Forest", 1)), (1, 1));
}

#[test]
fn settle_the_wreckage_counts_an_attacking_token_exiled() {
    // ruling: the number of lands is the number of attacking creatures exiled, tokens too
    // a token made on player 0's first turn attacks on their next
    let mut g = game(2);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let token = g.create_token(0, g.obj(bears).chars.clone(), false);
    assert!(g.obj(token).token());
    for _ in 0..12 { g.put(0, ISLAND, Zone::Library); g.put(1, ISLAND, Zone::Library); }
    while g.part != Part::PrecombatMain || g.active != 0 || g.turn < 3 {
        let p = g.priority;
        if matches!(g.asked(), Some((_, Asked::Attackers(_) | Asked::Blockers { .. }))) { g.act(p, Move::Attack(vec![])).or_else(|_| g.act(p, Move::Block(vec![]))).unwrap(); continue; }
        g.act(p, Move::Pass).unwrap();
    }
    let lands = [g.put(0, PLAINS, Zone::Library), g.put(0, PLAINS, Zone::Library), g.put(0, PLAINS, Zone::Library)];
    while g.part != Part::DeclareAttackers { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Attack(vec![(bears, Target::Player(1)), (token, Target::Player(1))])).unwrap();
    g.act(0, Move::Pass).unwrap();
    let settle = g.put(1, SETTLE, Zone::Hand);
    g.players[1].pool.colors = [4, 0, 0, 0, 0];
    g.act(1, paid("WWWW", Move::Cast { card: settle, targets: vec![vec![Target::Player(0)]] })).unwrap();
    while !g.stack.is_empty() && g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
    if matches!(g.decision, Some(Decision::Choose { choice: Choice::May { possible: true }, .. })) { g.act(0, Move::Choose(Answer::Yes)).unwrap(); }
    let Some(Decision::Choose { player, choice: Choice::Search { max, .. } }) = g.decision.clone() else { panic!("{:?}", g.decision) };
    cite!(["608.2c"]; assert_eq!((player, max), (0, 2), "two attacking creatures exiled, one a token"));
    // the caster doesn't see the library searched by its owner
    cite!(["701.23a"]; assert!(lands.iter().all(|c| !g.sees(1, *c))));
}

#[test]
fn strategic_betrayal_with_no_creature_still_exiles_their_graveyard() {
    // "Target opponent exiles a creature they control and their graveyard." (609.3)
    let mut g = game(2);
    let grave = [g.put(1, BOLT, Zone::Graveyard), g.put(1, BEARS, Zone::Graveyard)];
    cast_at_opponent(&mut g, BETRAYAL, "BB");
    while !g.stack.is_empty() && g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
    // each a new object in exile (400.7)
    cite!(["609.3"]; assert!(grave.iter().all(|c| g.obj(*c).zone != Zone::Graveyard) && g.players[1].graveyard.is_empty(), "{:?}", g.decision));
    assert_eq!(g.exile.iter().filter(|o| g.obj(**o).owner == 1).count(), 2);
}


/// A split card's halves are each a spell of their own (709.3): Consecrate's two paragraphs
/// are one text, and Consume's is another; neither is read with the other's.
#[test]
fn consecrate_and_consume_each_half_resolves_alone() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let defs: Vec<_> = ["Consecrate // Consume", "Grizzly Bears", "Island"].map(|n| CardDef::from_faces(&catalog[n]).unwrap()).to_vec();
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    for half in [0, 1] {
        let mut g = Game::new(defs.clone(), 2, 20);
        for p in 0..2 { for _ in 0..5 { g.put(p, 2, Zone::Library); } }
        while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
        let dead = g.put(1, 1, Zone::Graveyard);
        let bears = g.put(1, 1, Zone::Battlefield);
        let card = g.put(0, 0, Zone::Hand);
        g.players[0].pool.colors = [3, 3, 3, 3, 3];
        let target = if half == 0 { Target::Object(dead) } else { Target::Player(1) };
        let hand = g.players[0].hand.len();
        g.act(0, paid(if half == 0 { "WW" } else { "WWWB" }, Move::CastAs(game::card::CastAs::Half(half), Box::new(Move::Cast { card, targets: vec![vec![target]] })))).unwrap();
        while !g.stack.is_empty() && g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
        assert!(g.stack.is_empty() && g.decision.is_none(), "half {half}: {:?}", g.decision);
        if half == 0 {
            // "Exile target card from a graveyard. / Draw a card."
            cite!(["709.3"]; assert_eq!((g.obj(dead).zone != Zone::Graveyard, g.players[0].hand.len()), (true, hand), "exiled, and a card drawn for the one cast"));
        } else {
            // "Target player sacrifices a creature with the greatest power among creatures they
            // control. You gain life equal to its power."
            cite!(["709.3"]; assert_eq!((g.battlefield.contains(&bears), g.players[0].life), (false, 22)));
        }
    }
}
