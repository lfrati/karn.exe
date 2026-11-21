//! Bilbo, Thief in the Night, its Oracle text: "Spells you cast from anywhere other than your
//! hand cost {1} less to cast. / Whenever Bilbo attacks, you may cast an artifact, instant, or
//! sorcery spell from your graveyard. If an instant or sorcery spell cast this way would be put
//! into your graveyard, exile it instead."
//!
//! The reduction reads where the spell was cast from (601.2a, 601.2f). The attack trigger
//! offers one cast from its controller's own graveyard as it resolves, for the card's ordinary
//! costs, ignoring timing (608.2g); an instant or sorcery cast so is exiled instead of being
//! put into its owner's graveyard, wherever else it would go (614.1a), and only while it is
//! that spell (400.7).
use crate::support::cite;
use game::legal::Asked;
use game::simulation::{ResponseError, Simulation};
use crate::support::refused;
use game::state::Answer;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::{Color, mana::ManaSymbol};
use std::sync::OnceLock;

const BILBO: usize = 0;
const ISLAND: usize = 1;
const DIVINATION: usize = 2;
const ZAP: usize = 3;
const TRINKET: usize = 4;
const BEAR: usize = 5;
const COUNTER: usize = 6;
const BOUNCE: usize = 7;

fn defs() -> Vec<CardDef> {
    static FACES: OnceLock<Vec<oracle::corpus::Face>> = OnceLock::new();
    let faces = FACES.get_or_init(|| oracle::corpus::load(&oracle::corpus::default_path()).unwrap());
    let card = |name: &str, cost: &str, types: &str, text: &str, pt: Option<(i32, i32)>| {
        let def = CardDef::new(name, cost, types, text, pt).unwrap();
        game::play::check(&def).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        def
    };
    let bilbo = CardDef::from_faces(CardDef::catalog(faces).get("Bilbo, Thief in the Night").unwrap()).unwrap();
    game::play::check(&bilbo).unwrap();
    vec![
        bilbo,
        card("Island", "", "Basic Land — Island", "", None),
        card("Divination", "{2}{U}", "Sorcery", "Draw two cards.", None),
        card("Zap", "{1}{R}", "Instant", "Zap deals 2 damage to any target.", None),
        card("Trinket", "{2}", "Artifact", "", None),
        card("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Counterspell", "{0}", "Instant", "Counter target spell.", None),
        card("Unsummon Spell", "{0}", "Instant", "Return target spell to its owner's hand.", None),
    ]
}

fn game() -> Game {
    let mut g = Game::new(defs(), 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    g
}
fn pass_all(g: &mut Game) { while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); } }
fn pool(g: &mut Game, p: usize, mana: &[ManaSymbol]) { for m in mana { g.add_mana(p, *m); } }
const U: ManaSymbol = ManaSymbol::Color(Color::Blue);
const R: ManaSymbol = ManaSymbol::Color(Color::Red);
const C: ManaSymbol = ManaSymbol::Colorless;

/// Player `p`'s Bilbo attacks player `1 - p`; the game stops at its trigger's offer, or
/// wherever the trigger leaves it.
fn attack(g: &mut Game, p: usize, bilbo: ObjId) {
    while !(g.active == p && matches!(g.asked(), Some((_, Asked::Attackers(_))))) {
        match g.asked() {
            Some((q, Asked::Attackers(_))) => g.act(q, Move::Attack(vec![])).unwrap(),
            Some((q, Asked::Blockers { .. })) => g.act(q, Move::Block(vec![])).unwrap(),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    g.act(p, Move::Attack(vec![(bilbo, Target::Player(1 - p))])).unwrap();
    while matches!(g.asked(), Some((_, Asked::Priority))) && !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
}
fn offer(g: &mut Game, p: usize) -> Vec<ObjId> {
    match g.asked() { Some((q, Asked::Cast { cards })) if q == p => cards, other => panic!("not an offer to cast: {other:?}") }
}
fn in_graveyard(g: &Game, p: usize, card: usize) -> ObjId { *g.players[p].graveyard.iter().find(|o| g.obj(**o).card == Some(card)).unwrap() }
fn cards(g: &Game, of: &[ObjId]) -> Vec<usize> { let mut v: Vec<usize> = of.iter().map(|o| g.obj(*o).card.unwrap()).collect(); v.sort(); v }
fn exiled(g: &Game, card: usize) -> bool { g.exile.iter().any(|o| g.obj(*o).card == Some(card)) }
fn graveyard_has(g: &Game, p: usize, card: usize) -> bool { g.players[p].graveyard.iter().any(|o| g.obj(*o).card == Some(card)) }

// ---------------------------------------------------------------- the reduction

#[test]
fn r601_2f_spells_cast_from_anywhere_but_the_hand_cost_one_less() {
    // from the hand: full cost, {2}{U} with only {1}{U} is not castable
    let mut g = game();
    g.put(0, BILBO, Zone::Battlefield);
    let div = g.put(0, DIVINATION, Zone::Hand);
    pool(&mut g, 0, &[C, U]);
    cite!(["118.3a"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0, crate::support::paid("CU", Move::Cast { card: div, targets: vec![] })));
    pool(&mut g, 0, &[C]);
    g.act(0, crate::support::paid("CCU", Move::Cast { card: div, targets: vec![] })).unwrap();
    assert_eq!(g.players[0].pool.total(), 0);
    pass_all(&mut g);

    // from the graveyard through the attack trigger: {1}{U}
    let mut g = game();
    let bilbo = g.put(0, BILBO, Zone::Battlefield);
    g.put(0, DIVINATION, Zone::Graveyard);
    attack(&mut g, 0, bilbo);
    let div = in_graveyard(&g, 0, DIVINATION);
    pool(&mut g, 0, &[U]);
    cite!(["118.3a", "601.2f"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0, crate::support::paid("U", Move::Cast { card: div, targets: vec![] })));
    pool(&mut g, 0, &[C]);
    let hand = g.players[0].hand.len();
    g.act(0, crate::support::paid("CU", Move::Cast { card: div, targets: vec![] })).unwrap();
    cite!(["601.2f"]; assert_eq!(g.players[0].pool.total(), 0, "{{1}}{{U}} paid"));
    pass_all(&mut g);
    assert_eq!(g.players[0].hand.len(), hand + 2);
    cite!(["614.1a"]; assert!(exiled(&g, DIVINATION) && !graveyard_has(&g, 0, DIVINATION), "a sorcery cast this way is exiled"));
}

// ---------------------------------------------------------------- the offer

#[test]
fn r608_2g_the_offer_is_of_one_own_artifact_instant_or_sorcery_and_can_be_declined() {

    let mut g = game();
    let bilbo = g.put(0, BILBO, Zone::Battlefield);
    let declared: Vec<_> = [DIVINATION, ZAP, TRINKET, BEAR, ISLAND].into_iter().map(|c|(g.put(0,c,Zone::Graveyard),c)).collect();
    // the opponent's graveyard is not theirs
    g.put(1, ZAP, Zone::Graveyard);
    attack(&mut g, 0, bilbo);
    let offered = offer(&mut g, 0);
    cite!(["608.2g"]; assert_eq!(cards(&g, &offered), vec![DIVINATION, ZAP, TRINKET], "not a creature or a land, not another's"));
    for &(card,definition) in &declared[..3] {
        let targets = if definition == ZAP { vec![vec![Target::Player(1)]] } else { vec![] };
        cite!(["118.3a"]; refused("the supplied mana does not pay that cost, all of it and no more"): unchanged(&mut g,0,crate::support::paid("",Move::Cast { card, targets })));
    }
    let theirs = in_graveyard(&g, 1, ZAP);
    pool(&mut g, 0, &[R]);
    cite!(["608.2g"]; refused("the effect offers other cards to cast (608.2g)"): unchanged(&mut g,0,crate::support::paid("R",Move::Cast { card: theirs, targets: vec![vec![Target::Player(1)]] })));
    crate::support::refused(unchanged(&mut g,1,Move::Choose(Answer::No)), "that decision is another player's");
    cite!(["608.2g"]; refused("a decision is pending: cast one of the cards offered, or decline (608.2g)"): unchanged(&mut g,0,Move::Pass));
    let mut declined = g.clone();
    declined.act(0, Move::Choose(Answer::No)).unwrap();
    assert!(declined.stack.is_empty());
    assert_eq!(declined.players[0].graveyard.len(), 5, "nothing was cast");
    assert_eq!(declined.players[0].pool,crate::support::mana_pool("R"));
    // one cast, and the offer is over
    let zap = in_graveyard(&g, 0, ZAP);
    g.act(0, crate::support::paid("R", Move::Cast { card: zap, targets: vec![vec![Target::Player(1)]] })).unwrap();
    cite!(["608.2g"]; assert_eq!(g.stack.len(), 1));
    assert!(matches!(g.asked(), Some((0, Asked::Priority))), "{:?}", g.asked());
    pass_all(&mut g);
    cite!(["608.2c"]; assert_eq!(g.players[1].life,18));
    assert!(g.stack.is_empty() && exiled(&g,ZAP));
}

#[track_caller]
fn unchanged(g:&mut Game,player:usize,action:Move)->Result<(),String>{let before=format!("{g:?}");let result=g.act(player,action);assert_eq!(format!("{g:?}"),before);result}

#[test]
fn r614_1a_an_instant_resolved_or_countered_is_exiled_and_an_artifact_stays() {
    // resolves: exiled
    let mut g = game();
    let bilbo = g.put(0, BILBO, Zone::Battlefield);
    g.put(0, ZAP, Zone::Graveyard);
    attack(&mut g, 0, bilbo);
    let zap = in_graveyard(&g, 0, ZAP);
    pool(&mut g, 0, &[R]);
    g.act(0, crate::support::paid("R", Move::Cast { card: zap, targets: vec![vec![Target::Player(1)]] })).unwrap();
    pass_all(&mut g);
    assert_eq!(g.players[1].life, 18);
    cite!(["614.1a"]; assert!(exiled(&g, ZAP) && !graveyard_has(&g, 0, ZAP)));

    // countered: exiled
    let mut g = game();
    let bilbo = g.put(0, BILBO, Zone::Battlefield);
    g.put(0, ZAP, Zone::Graveyard);
    attack(&mut g, 0, bilbo);
    let zap = in_graveyard(&g, 0, ZAP);
    pool(&mut g, 0, &[R]);
    g.act(0, crate::support::paid("R", Move::Cast { card: zap, targets: vec![vec![Target::Player(1)]] })).unwrap();
    let spell = g.stack[0];
    g.act(0, Move::Pass).unwrap();
    let counter = g.put(1, COUNTER, Zone::Hand);
    g.act(1, Move::Cast { card: counter, targets: vec![vec![Target::Object(spell)]] }).unwrap();
    pass_all(&mut g);
    assert_eq!(g.players[1].life, 20);
    cite!(["614.1a", "701.6a"]; assert!(exiled(&g, ZAP) && !graveyard_has(&g, 0, ZAP), "countered into exile"));
    assert!(graveyard_has(&g, 1, COUNTER), "the counterspell itself is not cast this way");

    // an artifact spell: onto the battlefield, as normal
    let mut g = game();
    let bilbo = g.put(0, BILBO, Zone::Battlefield);
    g.put(0, TRINKET, Zone::Graveyard);
    attack(&mut g, 0, bilbo);
    let trinket = in_graveyard(&g, 0, TRINKET);
    pool(&mut g, 0, &[C]);
    g.act(0, crate::support::paid("C", Move::Cast { card: trinket, targets: vec![] })).unwrap();
    assert_eq!(g.players[0].pool.total(), 0, "{{2}} less {{1}}");
    pass_all(&mut g);
    assert!(g.battlefield.iter().any(|o| g.obj(*o).card == Some(TRINKET)));
}

#[test]
fn r400_7_a_bounced_spell_is_not_exiled_and_its_next_incarnation_carries_nothing() {
    let mut g = game();
    let bilbo = g.put(0, BILBO, Zone::Battlefield);
    g.put(0, ZAP, Zone::Graveyard);
    attack(&mut g, 0, bilbo);
    let zap = in_graveyard(&g, 0, ZAP);
    pool(&mut g, 0, &[R]);
    g.act(0, crate::support::paid("R", Move::Cast { card: zap, targets: vec![vec![Target::Player(1)]] })).unwrap();
    let spell = g.stack[0];
    g.act(0, Move::Pass).unwrap();
    let bounce = g.put(1, BOUNCE, Zone::Hand);
    g.act(1, Move::Cast { card: bounce, targets: vec![vec![Target::Object(spell)]] }).unwrap();
    pass_all(&mut g);
    // to its owner's hand: only the graveyard destination is replaced
    let back = *g.players[0].hand.iter().find(|o| g.obj(**o).card == Some(ZAP)).expect("in hand");
    cite!(["614.1a"]; assert!(!exiled(&g, ZAP)));
    // cast again from the hand, at full cost: a new object, not cast this way
    while !(g.active == 0 && g.part.main() && g.stack.is_empty()) { match g.asked() {
        Some((q, Asked::Blockers { .. })) => g.act(q, Move::Block(vec![])).unwrap(),
        _ => g.act(g.priority, Move::Pass).unwrap(),
    } }
    pool(&mut g, 0, &[R]);
    cite!(["118.3a"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0, crate::support::paid("R", Move::Cast { card: back, targets: vec![vec![Target::Player(1)]] })));
    pool(&mut g, 0, &[C]);
    g.act(0, crate::support::paid("CR", Move::Cast { card: back, targets: vec![vec![Target::Player(1)]] })).unwrap();
    pass_all(&mut g);
    cite!(["400.7"]; assert!(graveyard_has(&g, 0, ZAP) && !exiled(&g, ZAP), "to the graveyard as normal"));
}

#[test]
fn two_players_bilbos_reduce_and_offer_only_their_own() {
    let mut g = game();
    g.put(0, BILBO, Zone::Battlefield);
    let theirs = g.put(1, BILBO, Zone::Battlefield);
    g.put(0, ZAP, Zone::Graveyard);
    g.put(1, DIVINATION, Zone::Graveyard);
    // player 1's turn: their Bilbo attacks, offering their own graveyard
    while g.active != 1 { match g.asked() {
        Some((q, Asked::Attackers(_))) => g.act(q, Move::Attack(vec![])).unwrap(),
        Some((q, Asked::Blockers { .. })) => g.act(q, Move::Block(vec![])).unwrap(),
        _ => g.act(g.priority, Move::Pass).unwrap(),
    } }
    attack(&mut g, 1, theirs);
    let offered = offer(&mut g, 1);
    cite!(["109.5"]; assert_eq!(cards(&g, &offered), vec![DIVINATION]));
    let div = in_graveyard(&g, 1, DIVINATION);
    // its controller's Bilbo reduces it once: {1}{U}, not {U}
    pool(&mut g, 1, &[U]);
    cite!(["118.3a", "601.2f"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(1, crate::support::paid("U", Move::Cast { card: div, targets: vec![] })));
    pool(&mut g, 1, &[C]);
    g.act(1, crate::support::paid("CU", Move::Cast { card: div, targets: vec![] })).unwrap();
    pass_all(&mut g);
    cite!(["614.1a"]; assert!(exiled(&g, DIVINATION)));
    assert!(graveyard_has(&g, 0, ZAP), "the other player's card untouched");
}

// ---------------------------------------------------------------- managed Simulation

#[test]
fn simulation_suspends_at_the_offer_refuses_illegal_answers_and_clones() {
    let mut g = game();
    let bilbo = g.put(0, BILBO, Zone::Battlefield);
    g.put(0, ZAP, Zone::Graveyard);
    g.put(0, BEAR, Zone::Graveyard);
    attack(&mut g, 0, bilbo);
    pool(&mut g, 0, &[R]);
    let zap = in_graveyard(&g, 0, ZAP);
    let bear = in_graveyard(&g, 0, BEAR);
    let mut sim = Simulation::new(g);
    let r = sim.request().unwrap().unwrap().clone();
    assert_eq!(r.player, 0);
    let Asked::Cast { cards } = r.question.clone() else { panic!("{:?}", r.question) };
    assert_eq!(cards, vec![zap]);
    assert_eq!(sim.respond(r.id, 1, Move::Choose(Answer::No)), Err(ResponseError::WrongPlayer { expected: 0, received: 1 }), "not the opponent's offer");
    // a creature card is not offered
    refused(sim.respond(r.id, 0, Move::Cast { card: bear, targets: vec![] }), "the effect offers other cards to cast (608.2g)");
    let mut copy = sim.observe(|g| g.clone()).unwrap();
    sim.respond(r.id, 0, crate::support::paid("R", Move::Cast { card: zap, targets: vec![vec![Target::Player(1)]] })).unwrap();
    loop {
        let r = sim.request().unwrap().unwrap().clone();
        if sim.observe(|g| g.stack.is_empty()).unwrap() { break; }
        sim.respond(r.id, r.player, Move::Pass).unwrap();
    }
    assert!(sim.fault().is_none());
    assert_eq!(sim.observe(|g| (g.players[1].life, exiled(g, ZAP))).unwrap(), (18, true));
    // the copy, declined, still has the card in the graveyard
    copy.act(0, Move::Choose(Answer::No)).unwrap();
    assert!(graveyard_has(&copy, 0, ZAP) && copy.players[1].life == 20);
}
