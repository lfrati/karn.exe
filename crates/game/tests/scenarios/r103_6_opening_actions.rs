//! Opening-hand actions (103.6): once the mulligans are done, the starting player and then
//! each other player in turn order may take the actions their cards allow, in any order.
//! Gemstone Caverns begins the game on the battlefield, for a player who is not the starting
//! player, with a luck counter, and has them exile a card from their hand (103.6a); Devourer of
//! Destiny is revealed (103.6b) and has its player look at the top four cards of their library
//! at the beginning of their first upkeep. The witnesses are their Oracle text.
use crate::support::{cite, paid};
use game::card::CardDef;
use game::event::Event;
use game::legal::{Asked};
use game::play::check;
use game::simulation::Simulation;
use game::state::Answer;
use game::{Game, Move, ObjId, Target, Zone};
use ir::mana::ManaSymbol;
use ir::{Color, Part};
use std::sync::OnceLock;

const CAVERNS: usize = 0;
const DEVOURER: usize = 1;
const WASTES: usize = 2;
const BAUBLE: usize = 3;
const LEYLINE: usize = 4;
const BEARS: usize = 5;

fn defs() -> Vec<CardDef> {
    static FACES: OnceLock<Vec<oracle::corpus::Face>> = OnceLock::new();
    let faces = FACES.get_or_init(|| oracle::corpus::load(&oracle::corpus::default_path()).unwrap());
    let oracle = |name: &str| CardDef::from_faces(CardDef::catalog(faces).get(name).unwrap()).unwrap();
    vec![
        oracle("Gemstone Caverns"),
        oracle("Devourer of Destiny"),
        CardDef::new("Wastes", "", "Basic Land", "{T}: Add {C}.", None).unwrap(),
        CardDef::new("Bauble", "{0}", "Artifact", "", None).unwrap(),
        oracle("Leyline of Sanctity"),
        CardDef::new("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
    ]
}

/// A game started from these seven-card decks (each a whole opening hand), `first` starting,
/// every player keeping. The opening draw consumes each explicitly seven-card library.
fn start(decks: &[Vec<usize>], first: usize) -> Game {
    let mut g = Game::start(defs(), decks, 20, first, 7);
    while let Some((p, Asked::Mulligan)) = g.asked() { g.act(p, Move::Keep).unwrap(); }
    g
}

fn hand_of(g: &Game, p: usize, def: usize) -> Vec<ObjId> {
    g.players[p].hand.iter().copied().filter(|o| g.obj(*o).card == Some(def)).collect()
}

/// Who is asked to choose an opening-hand action now, and among what.
fn offered(g: &mut Game) -> Option<(usize, Vec<ObjId>)> {
    match g.asked() { Some((p, Asked::Select { among, min: 0, max: 1 })) if !g.begun() => Some((p, among)), _ => None }
}

#[test]
fn r103_6_the_witnesses_are_admitted() {
    for d in defs() { check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0)); }
}

#[test]
fn r103_6a_caverns_begins_on_the_battlefield_only_for_a_player_who_is_not_starting() {
    let mut g = start(&[vec![CAVERNS, BAUBLE, BAUBLE, BAUBLE, BAUBLE, BAUBLE, BAUBLE], vec![CAVERNS, BAUBLE, BAUBLE, BAUBLE, BAUBLE, BAUBLE, BAUBLE]], 0);
    // the starting player is not offered it: the other player is asked first
    let (player, among) = offered(&mut g).expect("an opening action");
    cite!(["103.6"]; assert_eq!(player, 1));
    let theirs = hand_of(&g, 1, CAVERNS)[0];
    let baubles = hand_of(&g, 1, BAUBLE);
    assert_eq!(baubles.len(), 6);
    let exile_card = baubles[2]; // authored from the declared opening hand, before the exile question
    cite!(["103.6"]; assert_eq!(among, vec![theirs]));
    cite!(["103.6"]; refused("a decision is pending: an opening-hand action (103.6)"): g.act(1, Move::Pass));
    g.act(1, Move::Choose(Answer::Objects(vec![theirs]))).unwrap();
    // with a luck counter on it; then "exile a card from your hand"
    let caverns = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(CAVERNS)).unwrap();
    cite!(["103.6a"]; assert_eq!(g.obj(caverns).counters.get("luck"), Some(&1)));
    let Some((1, Asked::Objects { among, count: 1, .. })) = g.asked() else { panic!("{:?}", g.asked()) };
    assert_eq!(among, baubles, "exactly the six declared Baubles remain eligible");
    // a copy taken at the question resumes the same; nothing else may be done meanwhile
    let mut copy = g.clone();
    cite!(["103.6"]; refused("a decision is pending: an opening-hand action (103.6)"): g.act(0, Move::Pass));
    for g in [&mut g, &mut copy] {
        g.act(1, Move::Choose(Answer::Objects(vec![exile_card]))).unwrap();
        assert!(!g.players[1].hand.contains(&exile_card));
        assert_eq!(g.exile.len(), 1);
        let exiled = g.exile[0];
        assert_ne!(exiled, exile_card, "400.7: a new object in exile");
        assert_eq!((g.obj(exiled).card, g.obj(exiled).owner), (Some(BAUBLE), 1));
    }
    assert_eq!(format!("{g:?}"), format!("{copy:?}"));
    assert_eq!(g.exile.len(), 1);
    assert_eq!(g.players[1].hand.len(), 5);
    // nothing more to do: the game begins, the starting player's Caverns still in hand
    assert!(g.begun());
    assert_eq!((g.active, g.turn), (0, 1), "the starting player's first turn");
    assert_eq!(hand_of(&g, 0, CAVERNS).len(), 1);
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    // with luck: one mana of any color; without (played as a land): {C}
    let mine = hand_of(&g, 0, CAVERNS)[0];
    g.act(0, Move::PlayLand(mine)).unwrap();
    let mine = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(CAVERNS) && g.controller(**o) == 0).unwrap();
    // its second row: "{T}: Add {C}. If ~ has a luck counter on it, instead add one mana of any color."
    let tap = |o: ObjId, mana: ManaSymbol| Move::Adding(vec![vec![mana]], Box::new(Move::Activate { source: o, ability: 1, targets: vec![], paying: vec![] }));
    // CR 605.3b/608.2c: without luck the replacement instruction does not
    // apply, so there is no color choice. The old test supplied unused Red
    // anyway; it checked output but missed the incorrect offered question.
    let before = format!("{g:?}");
    cite!(["605.3b", "608.2c"]; refused("that ability offers no such choice of mana"): g.act(0, tap(mine, ManaSymbol::Color(Color::Red))));
    assert_eq!(format!("{g:?}"), before);
    g.act(0, Move::Activate { source: mine, ability: 1, targets: vec![], paying: vec![] }).unwrap();
    cite!(["614.15"]; assert_eq!((g.players[0].pool.total(), g.players[0].pool.colors.iter().sum::<u32>()), (1, 0), "no luck counter: {{C}}"));
    g.act(0, Move::Pass).unwrap();
    g.act(1, tap(caverns, ManaSymbol::Color(Color::Red))).unwrap();
    cite!(["614.15"]; assert_eq!((g.players[1].pool.total(), g.players[1].pool.colors[Color::Red as usize]), (1, 1), "a luck counter: instead {{R}}"));
}

#[test]
fn r103_6b_devourer_is_revealed_once_and_looks_at_the_top_four_at_its_players_first_upkeep() {
    for first in [0, 1] {
        let mut g = start(&[vec![DEVOURER, WASTES, WASTES, WASTES, WASTES, WASTES, WASTES], vec![BAUBLE; 7]], first);
        let devourer = hand_of(&g, 0, DEVOURER)[0];
        cite!(["103.6"]; case: format!("first {first}"); assert_eq!(offered(&mut g), Some((0, vec![devourer]))));
        g.act(0, Move::Choose(Answer::Objects(vec![devourer]))).unwrap();
        cite!(["103.6b"]; case: format!("first {first}"); assert!(g.log.iter().any(|e| matches!(e, Event::Revealed { player: 0, cards } if cards == &vec![devourer]))));
        // revealed once: not offered again, and the game begins
        cite!(["103.6b"]; case: format!("first {first}"); assert!(offered(&mut g).is_none() && g.begun()));
        // four different cards on top (the last of the library is its top)
        for def in [CAVERNS, LEYLINE, BEARS, BAUBLE] { g.put(0, def, Zone::Library); }
        // a draw before the upkeep would take one: the starting player skips their first (103.8a)
        let top: Vec<ObjId> = g.players[0].library.iter().rev().take(4).copied().collect();
        let mut asked = None;
        for _ in 0..80 {
            match g.asked() {
                // "You may put one of those cards back": yes, then which
                Some((0, Asked::May)) => g.act(0, Move::Choose(Answer::Yes)).unwrap(),
                Some((0, Asked::Select { among, .. } | Asked::Objects { among, .. })) => { asked = Some(among); break; }
                Some((p, Asked::Order(a))) => g.act(p, Move::Order((0..a.len()).collect())).unwrap(),
                _ => g.act(g.priority, Move::Pass).unwrap(),
            }
        }
        let among = asked.expect("the first-upkeep look");
        assert_eq!(g.part, Part::Upkeep);
        assert_eq!(g.active, 0, "player 0's own first upkeep");
        assert_eq!(g.turn, if first == 0 { 1 } else { 2 });
        let mut seen = among.clone();
        seen.sort();
        let mut expected = top.clone();
        expected.sort();
        assert_eq!(seen, expected);
        // the Bears back on top, the other three exiled
        let bears = *top.iter().find(|o| g.obj(**o).card == Some(BEARS)).unwrap();
        g.act(0, Move::Choose(Answer::Objects(vec![bears]))).unwrap();
        while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
        let on_top = *g.players[0].library.last().unwrap();
        assert_eq!(g.obj(on_top).card, Some(BEARS), "put back on top");
        let mut exiled: Vec<usize> = g.exile.iter().filter_map(|o| g.obj(*o).card).collect();
        exiled.sort();
        assert_eq!(exiled, vec![CAVERNS, BAUBLE, LEYLINE], "the rest exiled");
        assert!(hand_of(&g, 0, DEVOURER).len() == 1, "the revealed card stays in hand");
    }
}

#[test]
fn r601_2g_caverns_without_luck_pays_colorless_with_an_explicit_mana_step() {
    // Without a luck counter its mana ability produces only {C}.
    let mut defs = defs();
    defs.push(CardDef::new("One-Drop", "{1}", "Artifact", "", None).unwrap());
    let one = defs.len() - 1;
    let mut g = Game::new(defs, 2, 20);
    let caverns = g.put(0, CAVERNS, Zone::Battlefield);
    let card = g.put(0, one, Zone::Hand);
    let plan = vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source:caverns, ability:1, paying:vec![], adding:vec![], chose:vec![], cost_branches:None }];
    g.act(0, paid("C",Move::WithMana(plan, Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    cite!(["601.2g"]; assert!(g.obj(caverns).tapped));
    cite!(["107.4b"]; assert_eq!(g.stack.len(), 1));
}

#[test]
fn r103_6_multiplayer_order_and_several_actions_of_one_player() {
    // player 1 starts: then player 2, then player 0, each taking what they will in any order
    let mut g = start(&[vec![CAVERNS, LEYLINE, BAUBLE, BAUBLE, BAUBLE, BAUBLE, BAUBLE], vec![BAUBLE; 7], vec![DEVOURER, BAUBLE, BAUBLE, BAUBLE, BAUBLE, BAUBLE, BAUBLE]], 1);
    let (player, among) = offered(&mut g).unwrap();
    cite!(["103.6"]; assert_eq!((player, among.len()), (2, 1), "the starting player has none; the next in turn order"));
    g.act(2, Move::Choose(Answer::Objects(among))).unwrap();
    let (player, among) = offered(&mut g).unwrap();
    cite!(["103.6"]; assert_eq!((player, among.len()), (0, 2), "Caverns and Leyline, in the order player 0 likes"));
    let leyline = hand_of(&g, 0, LEYLINE)[0];
    g.act(0, Move::Choose(Answer::Objects(vec![leyline]))).unwrap();
    let (player, among) = offered(&mut g).unwrap();
    cite!(["103.6"]; assert_eq!((player, among.clone()), (0, hand_of(&g, 0, CAVERNS))));
    g.act(0, Move::Choose(Answer::Objects(among))).unwrap();
    let Some((0, Asked::Objects { among, count: 1, .. })) = g.asked() else { panic!("{:?}", g.asked()) };
    g.act(0, Move::Choose(Answer::Objects(vec![among[0]]))).unwrap();
    assert!(offered(&mut g).is_none() && g.begun());
    cite!(["103.6a"]; assert_eq!(g.battlefield.iter().filter(|o| g.controller(**o) == 0).count(), 2));
    assert_eq!(g.active, 1);
}

#[test]
fn r103_6_declining_takes_nothing_and_the_game_begins() {
    let mut g = start(&[vec![BAUBLE; 7], vec![CAVERNS, BAUBLE, BAUBLE, BAUBLE, BAUBLE, BAUBLE, BAUBLE]], 0);
    cite!(["103.6"]; assert_eq!(offered(&mut g).map(|(p, _)| p), Some(1)));
    g.act(1, Move::Choose(Answer::Objects(vec![]))).unwrap();
    cite!(["103.6"]; assert!(g.begun() && g.battlefield.is_empty() && g.exile.is_empty()));
}

#[test]
fn r603_devourers_cast_trigger_exiles_a_colored_permanent() {
    let mut g = Game::new(defs(), 2, 20);
    let devourer = g.put(0, DEVOURER, Zone::Hand);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    let bauble = g.put(1, BAUBLE, Zone::Battlefield);
    for _ in 0..7 { g.add_mana(0, ManaSymbol::Colorless); }
    g.act(0, paid("CCCCCCC",Move::Cast { card: devourer, targets: vec![] })).unwrap();
    // "exile target permanent that's one or more colors": the Bears, its one legal target,
    // not the colorless Bauble; the trigger above the spell
    if let Some((0, Asked::Targets { .. })) = g.asked() { g.act(0, Move::ChooseTargets(vec![vec![Target::Object(bears)]])).unwrap(); }
    cite!(["603.3"]; assert_eq!(g.stack.len(), 2));
    cite!(["603.3d"]; assert_eq!(g.obj(g.stack[1]).targets, vec![vec![Target::Object(bears)]]));
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    assert!(g.exile.iter().any(|o| g.obj(*o).card == Some(BEARS)));
    assert!(g.battlefield.contains(&bauble));
    assert!(g.battlefield.iter().any(|o| g.obj(*o).card == Some(DEVOURER)));
}

#[test]
fn simulation_offers_opening_actions_to_the_right_player() {
    let g = start(&[vec![BAUBLE; 7], vec![DEVOURER, BAUBLE, BAUBLE, BAUBLE, BAUBLE, BAUBLE, BAUBLE]], 0);
    let mut sim = Simulation::new(g);
    let r = sim.request().unwrap().unwrap().clone();
    cite!(["103.6"]; assert_eq!(r.player, 1));
    let Asked::Select { among, .. } = &r.question else { panic!("{:?}", r.question) };
    assert_eq!(sim.respond(r.id, 0, Move::Choose(Answer::Objects(among.clone()))), Err(game::simulation::ResponseError::WrongPlayer { expected: 1, received: 0 }), "only its player answers");
    sim.respond(r.id, 1, Move::Choose(Answer::Objects(among.clone()))).unwrap();
    let r = sim.request().unwrap().unwrap().clone();
    assert!(matches!(r.question, Asked::Priority), "{:?}", r.question);
    assert!(sim.fault().is_none());
}

/// The question an opening-hand action asks is for that action: Gemstone Caverns' ability,
/// with its text, followed with no stack (103.6a), and not a spell.
#[test]
fn r103_6a_the_caverns_exile_is_asked_for_its_opening_action_not_a_spell() {
    use game::prompt::{OccurrenceKind, Reason};
    let g = start(&[vec![BAUBLE; 7], vec![CAVERNS, BAUBLE, BAUBLE, BAUBLE, BAUBLE, BAUBLE, BAUBLE]], 0);
    let mut sim = Simulation::new(g);
    let r = sim.request().unwrap().unwrap().clone();
    let Asked::Select { among, .. } = &r.question else { panic!("{:?}", r.question) };
    sim.respond(r.id, 1, Move::Choose(Answer::Objects(among.clone()))).unwrap();
    let prompt = sim.prompt(1).unwrap();
    let decision = prompt.decision.expect("the exile is asked of seat 1");
    assert!(matches!(decision.question, Asked::Objects { count: 1, .. }), "{:?}", decision.question);
    assert_eq!(decision.reason, Reason::Resolution);
    let occurrence = decision.occurrence.expect("asked for the action");
    // the permanent the card became (400.7), whose ability it is
    let g = sim.game().unwrap();
    let caverns = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(CAVERNS)).unwrap();
    cite!(["103.6a"]; assert_eq!((occurrence.kind, occurrence.object, occurrence.source, occurrence.source_name.as_str()),
               (OccurrenceKind::Opening, None, Some(caverns), "Gemstone Caverns")));
    assert!(occurrence.text.as_deref().is_some_and(|t| t.ends_with("If you do, exile a card from your hand.")), "{:?}", occurrence.text);
}
