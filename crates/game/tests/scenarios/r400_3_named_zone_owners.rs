//! Zones named by whose they are: "target opponent puts a card from their hand on top of their
//! library", "put that card into their graveyard", "on the bottom of that player's library",
//! "exile all cards from target player's library", "shuffle your graveyard into your library".
//! A card put into a hand, library or graveyard goes to its owner's (400.3).
use crate::support::{cite, paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const RATS: usize = 0;
const EYE_SPY: usize = 1;
const JACE: usize = 2;
const CANE: usize = 3;
const BEAR: usize = 5;
const ISLAND: usize = 6;
const SPELL: usize = 7;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap();
    let c = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).expect(name);
    let defs = vec![
        real("Chittering Rats"),
        real("Eye Spy"),
        real("Jace, the Mind Sculptor"),
        real("Feldon's Cane"),
        c("Unused", "Sorcery", "You gain 1 life.", None),
        c("Bear", "Creature — Bear", "", Some((2, 2))),
        c("Island", "Basic Land — Island", "", None),
        c("Spell", "Instant", "You gain 1 life.", None),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn mana(g: &mut Game) { g.players[0].pool.colors = [9; 5]; g.players[0].pool.colorless = 9; }
/// Pass priority until the stack is empty or a decision other than targets is asked;
/// a trigger's target is player 1.
fn settle(g: &mut Game) {
    loop {
        match &g.decision {
            Some(Decision::Targets { .. }) => g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap(),
            Some(_) => return,
            None if g.stack.is_empty() => return,
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
}
fn cast(g: &mut Game, card: usize, targets: Vec<Vec<Target>>, spending: &str) {
    let spell = g.put(0, card, Zone::Hand);
    mana(g);
    g.act(0, paid(spending, Move::Cast { card: spell, targets })).unwrap();
    settle(g);
}
// a card that changes zones is a new object (400.7): compare what card it is
fn card(g: &Game, o: ObjId) -> usize { g.obj(o).card.unwrap() }
fn top(g: &Game, p: usize) -> usize { card(g, *g.players[p].library.last().unwrap()) }
fn bottom(g: &Game, p: usize) -> usize { card(g, g.players[p].library[0]) }
fn has(g: &Game, zone: &[ObjId], c: usize) -> usize { zone.iter().filter(|o| g.obj(**o).card == Some(c)).count() }

#[test]
fn r400_3_the_opponent_chooses_a_card_from_their_hand_for_the_top_of_their_library() {
    let mut g = game();
    g.put(1, BEAR, Zone::Hand);
    let b = g.put(1, SPELL, Zone::Hand);
    cast(&mut g, RATS, vec![], "BBB");
    let Some(Decision::Choose { player: 1, choice: Choice::Objects { among, count: 1, .. } }) = g.decision.clone() else { panic!("{:?}", g.decision) };
    assert_eq!(among.len(), 2);
    crate::support::refused(g.act(0, Move::Choose(Answer::Objects(vec![b]))), "that decision is another player's");
    g.act(1, Move::Choose(Answer::Objects(vec![b]))).unwrap();
    settle(&mut g);
    cite!(["400.3"]; assert_eq!(top(&g, 1), SPELL));
    assert_eq!(g.players[1].hand.iter().map(|o| card(&g, *o)).collect::<Vec<_>>(), vec![BEAR]);
}

#[test]
fn r400_3_an_empty_hand_puts_nothing_back() {
    let mut g = game();
    let library = g.players[1].library.clone();
    cast(&mut g, RATS, vec![], "BBB");
    assert!(g.decision.is_none() && g.stack.is_empty());
    cite!(["609.3"]; assert_eq!(g.players[1].library, library));
}

#[test]
fn r400_3_the_looked_at_card_goes_to_its_owners_graveyard() {
    for yes in [true, false] {
        let mut g = game();
        g.put(1, SPELL, Zone::Library);
        cast(&mut g, EYE_SPY, vec![vec![Target::Player(1)]], "U");
        assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May { possible: true } })), "{:?}", g.decision);
        g.act(0, Move::Choose(if yes { Answer::Yes } else { Answer::No })).unwrap();
        settle(&mut g);
        cite!(["400.3"]; case: format!("yes {yes}"); assert_eq!(has(&g, &g.players[1].graveyard, SPELL), yes as usize));
        assert_eq!(top(&g, 1) == SPELL, !yes);
    }
}

fn jace(g: &mut Game, loyalty: u32) -> ObjId {
    let jace = g.put(0, JACE, Zone::Battlefield);
    g.put_counters(jace, "loyalty", loyalty);
    jace
}

#[test]
fn r400_3_jace_puts_the_top_card_on_the_bottom_of_that_players_library() {
    let mut g = game();
    let j = jace(&mut g, 3);
    g.put(1, SPELL, Zone::Library);
    g.act(0, Move::Activate { source: j, ability: 0, targets: vec![vec![Target::Player(1)]], paying: vec![] }).unwrap();
    settle(&mut g);
    assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May { possible: true } })), "{:?}", g.decision);
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    settle(&mut g);
    cite!(["400.3"]; assert_eq!(bottom(&g, 1), SPELL));
    assert_eq!(g.players[1].library.len(), 6);
}

#[test]
fn r400_3_jace_exiles_a_library_and_that_player_shuffles_their_hand_into_it() {
    let mut g = game();
    let j = jace(&mut g, 12);
    let library = g.players[1].library.len();
    g.put(1, BEAR, Zone::Hand);
    g.put(1, SPELL, Zone::Hand);
    let mine = g.players[0].library.len();
    g.act(0, Move::Activate { source: j, ability: 3, targets: vec![vec![Target::Player(1)]], paying: vec![] }).unwrap();
    settle(&mut g);
    assert_eq!(has(&g, &g.exile, ISLAND), library);
    let mut now: Vec<_> = g.players[1].library.iter().map(|o| card(&g, *o)).collect();
    now.sort();
    cite!(["400.3"]; assert_eq!(now, vec![BEAR, SPELL]));
    assert!(g.players[1].hand.is_empty());
    assert_eq!(g.players[0].library.len(), mine);
}

#[test]
fn r400_3_shuffle_your_graveyard_into_your_library() {
    let mut g = game();
    let cane = g.put(0, CANE, Zone::Battlefield);
    g.put(0, BEAR, Zone::Graveyard);
    g.put(0, SPELL, Zone::Graveyard);
    g.put(1, BEAR, Zone::Graveyard);
    let from = g.log.len();
    g.act(0, Move::Activate { source: cane, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    settle(&mut g);
    // the owner of the cards is the player named: one shuffle (701.24a)
    cite!(["701.24a"]; assert_eq!(g.log[from..].iter().filter(|e| matches!(e, game::Event::Shuffled { player: 0, .. })).count(), 1));
    assert_eq!((has(&g, &g.players[0].graveyard, BEAR), has(&g, &g.players[0].graveyard, SPELL)), (0, 0));
    cite!(["400.3"]; assert_eq!((has(&g, &g.players[0].library, BEAR), has(&g, &g.players[0].library, SPELL)), (1, 1)));
    assert_eq!(has(&g, &g.players[1].graveyard, BEAR), 1);
    assert!(g.exile.iter().any(|o| g.obj(*o).card == Some(CANE)));
}
