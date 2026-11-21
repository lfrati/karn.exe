//! Sagas (714): chapter abilities are triggered abilities (714.2b) of lore counters put on the
//! Saga; it enters with one (714.3a), its controller adds one as their precombat main phase
//! begins (714.3c), and it is sacrificed once it has read its final chapter and no chapter
//! ability of it is still to resolve (714.4). Urza's Saga is its Oracle text; the others are
//! test cards.
use crate::support::{cite, paid};
use game::card::CardDef;
use game::event::Event;
use game::legal::Asked;
use game::play::check;
use game::simulation::Simulation;
use game::state::Answer;
use game::{Game, Move, ObjId, Target, Zone};
use ir::Part;
use std::sync::OnceLock;

const URZA: usize = 0;
const CHRONICLE: usize = 1;
const TWIN: usize = 2;
const LORE: usize = 3;
const STIFLE: usize = 4;
const BAUBLE: usize = 5;
const ONE: usize = 6;
const WHITE: usize = 7;
const TWO: usize = 8;
const WASTES: usize = 9;
const ONE_LORE: usize = 10;

fn faces() -> &'static Vec<oracle::corpus::Face> {
    static FACES: OnceLock<Vec<oracle::corpus::Face>> = OnceLock::new();
    FACES.get_or_init(|| oracle::corpus::load(&oracle::corpus::default_path()).unwrap())
}

fn urza() -> CardDef {
    CardDef::from_faces(CardDef::catalog(faces()).get("Urza's Saga").unwrap()).unwrap()
}

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text| CardDef::new(name, cost, types, text, None).expect(name);
    vec![
        urza(),
        card("Chronicle", "{0}", "Enchantment — Saga", "I — You gain 1 life.\nII — You gain 2 life.\nIII — You gain 3 life."),
        card("Twin Chronicle", "{0}", "Enchantment — Saga", "I, II — You gain 1 life.\nIII — You gain 5 life."),
        card("Lore", "{0}", "Instant", "Put two lore counters on target Saga you control."),
        card("Stifle", "{0}", "Instant", "Counter target activated or triggered ability."),
        card("Bauble", "{0}", "Artifact", ""),
        card("One-Drop", "{1}", "Artifact", ""),
        card("White One", "{W}", "Artifact", ""),
        card("Two-Drop", "{2}", "Artifact", ""),
        CardDef::new("Wastes", "", "Basic Land", "{T}: Add {C}.", None).unwrap(),
        card("One Lore", "{0}", "Instant", "Put a lore counter on target Saga you control."),
    ]
}

/// Cast a lore-counter spell of player 0's at this Saga and let everything resolve.
fn add_lore(g: &mut Game, saga: ObjId, def: usize) {
    let spell = g.put(0, def, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(saga)]] }).unwrap();
    resolve(g);
}

fn game() -> Game {
    let mut g = Game::new(defs(), 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, WASTES, Zone::Library); } }
    g
}

fn lore(g: &Game, o: ObjId) -> u32 { g.obj(o).counters.get("lore").copied().unwrap_or(0) }

/// Pass priority until the stack is empty, answering nothing else.
fn resolve(g: &mut Game) {
    for _ in 0..40 {
        if g.stack.is_empty() && g.decision.is_none() { return; }
        if let Some((p, Asked::Order(abilities))) = g.asked() { g.act(p, Move::Order((0..abilities.len()).collect())).unwrap(); continue; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("did not settle: {:?}", g.asked());
}

/// Pass until player 0's next precombat main phase has begun.
fn next_main(g: &mut Game) {
    let turn = g.turn;
    for _ in 0..200 {
        if g.turn > turn && g.active == 0 && g.part == Part::PrecombatMain { return; }
        match g.asked() {
            Some((p, Asked::Order(abilities))) => g.act(p, Move::Order((0..abilities.len()).collect())).unwrap(),
            Some((p, Asked::Discard { count, .. })) => { let cards = g.players[p].hand[..count].to_vec(); g.act(p, Move::Discard(cards)).unwrap() }
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("no next main phase");
}

fn put(g: &mut Game, def: usize) -> ObjId { g.put(0, def, Zone::Battlefield) }

#[test]
fn r714_urzas_saga_is_admitted_and_its_chapters_round_trip() {

    let urza = urza();
    check(&urza).unwrap();
    for d in defs() { check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0)); }
    // each chapter prints back as the Oracle line it was read from
    let face = faces().iter().find(|f| f.face_name == "Urza's Saga").unwrap();
    let rule = &grammar::ability::abilities().ability;
    let normalized = oracle::normalize::normalize(&face.oracle_text, &face.name, &face.face_name, &face.type_line);
    let rows: Vec<&String> = normalized.rows.iter().filter(|r| !r.is_empty()).collect();
    assert_eq!((urza.abilities().len(), rows.len()), (3, 3));
    for (a, row) in urza.abilities().iter().zip(rows) {
        assert_eq!(rule.print_text(a, true).as_ref(), Some(row));
    }
    // a chapter ability of a permanent that is no Saga, and read ahead, are refused
    for (types, text) in [("Enchantment", "I — You gain 1 life."), ("Enchantment — Saga", "Read ahead\nI — You gain 1 life.")] {
        let d=CardDef::new("Probe", "{0}", types, text, None).unwrap();
        crate::support::refused(check(&d).map_err(|e|e.0),if types=="Enchantment" {"chapter ability of a permanent that is not a Saga"} else {"keyword: Read Ahead"});
    }
    // nested quotations read (an Oracle quote inside a quote is single-quoted)
    let nested: Vec<_> = faces().iter().filter(|f| f.oracle_text.split('"').enumerate().any(|(i, p)| i % 2 == 1 && p.contains(" '"))).collect();
    let parsed = nested.iter().filter(|f| CardDef::from_face(f).is_ok()).count();
    assert!(nested.len() >= 13 && parsed >= 11, "{parsed} of {}", nested.len());
}

#[test]
fn r714_3_urzas_saga_from_land_drop_to_sacrifice() {
    let mut g = game();
    let saga = g.put(0, URZA, Zone::Hand);
    let sources: Vec<_> = (0..2).map(|_| put(&mut g, WASTES)).collect();
    let library: Vec<ObjId> = [BAUBLE, ONE, WHITE, TWO].map(|d| g.put(0, d, Zone::Library)).to_vec();
    g.act(0, Move::PlayLand(saga)).unwrap();
    let saga = *g.battlefield.iter().find(|o| g.obj(**o).chars.name == "Urza's Saga").unwrap();
    // it entered with a lore counter: chapter I triggered (714.3a)
    cite!(["714.3a"]; assert_eq!(lore(&g, saga), 1));
    assert_eq!(g.stack.len(), 1);
    resolve(&mut g);
    // I — it has "{T}: Add {C}."
    let granted = |game:&Game| game.audit_game_objects()["objects"][saga.0.to_string()]["abilities"].as_array().unwrap()
        .iter().filter(|row|row["origin"]["kind"]=="given")
        .map(|row|row["origin"]["producer"].clone()).collect::<Vec<_>>();
    // The resolving chapter grants the ability (611.2a); its source paragraph
    // remains chapter I, not paragraph 1 of an anonymous granted template.
    cite!(["611.2a", "714.2b"]; assert_eq!(granted(&g).iter().map(|row|row["paragraph"].as_u64().unwrap()).collect::<Vec<_>>(),vec![1]));
    cite!(["113.7"]; assert_eq!(granted(&g)[0]["source"],saga.0));
    g.act(0, Move::Activate { source: saga, ability: 3, targets: vec![], paying: vec![] }).unwrap();
    assert_eq!(g.players[0].pool.total(), 1);
    // its controller's next precombat main phase: a lore counter before anyone has priority
    next_main(&mut g);
    // the first priority of the phase: the counter is already there, its chapter on the stack
    assert_eq!(g.priority, 0);
    cite!(["714.3c"]; assert_eq!(lore(&g, saga), 2, "the turn-based action happened as the phase began"));
    assert_eq!(g.stack.len(), 1, "chapter II waits on the stack");
    resolve(&mut g);
    // II — "{2}, {T}: Create a 0/0 colorless Construct artifact creature token with …"
    cite!(["611.2a", "714.2b"]; assert_eq!(granted(&g).iter().map(|row|row["paragraph"].as_u64().unwrap()).collect::<Vec<_>>(),vec![1,2]));
    let plan = sources.into_iter().map(|source| game::legal::Step { plan: vec![], x: None, life: vec![], with: None,
        source, ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None,
    }).collect();
    g.act(0, paid("CC", Move::WithMana(plan, Box::new(Move::Activate { source: saga, ability: 4, targets: vec![], paying: vec![] })))).unwrap();
    resolve(&mut g);
    let construct = *g.battlefield.iter().find(|o| g.obj(**o).chars.name == "Construct Token").unwrap();
    assert_eq!(g.power(construct), Some(1), "itself is an artifact");
    // III — search for an artifact card with mana cost {0} or {1}, not mana value 1
    next_main(&mut g);
    assert_eq!(lore(&g, saga), 3);
    for _ in 0..10 {
        if let Some((0, Asked::Search { among, .. })) = g.asked() {
            let mut offered = among.clone();
            offered.sort();
            assert_eq!(offered, vec![library[0], library[1]], "{{0}} and {{1}}, not {{W}} or {{2}}");
            // the final chapter is on the stack: the Saga is still there (714.4)
            cite!(["714.4"]; assert!(g.battlefield.contains(&saga)));
            g.act(0, Move::Choose(Answer::Objects(vec![library[1]]))).unwrap();
            break;
        }
        g.act(g.priority, Move::Pass).unwrap();
    }
    resolve(&mut g);
    assert!(g.battlefield.iter().any(|o| g.obj(*o).chars.name == "One-Drop"));
    // the chapter ability has left the stack: sacrificed
    cite!(["714.4"]; assert!(!g.battlefield.contains(&saga)));
    assert!(g.log.iter().any(|e| matches!(e, Event::Sacrificed { player: 0, object } if *object == saga)));
    assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).chars.name == "Urza's Saga"));
    assert_eq!(g.power(construct), Some(2), "the Construct counts the One-Drop");
}

#[test]
fn r714_2c_a_counter_jump_triggers_each_chapter_reached_and_its_controller_orders_them() {
    let mut g = game();
    let saga = g.put(0, TWIN, Zone::Hand);
    g.act(0, Move::Cast { card: saga, targets: vec![] }).unwrap();
    resolve(&mut g);
    let saga = *g.battlefield.iter().find(|o| g.obj(**o).chars.name == "Twin Chronicle").unwrap();
    assert_eq!((lore(&g, saga), g.players[0].life), (1, 21), "I");
    // two counters at once: II (of "I, II") and III, both triggered, its controller orders
    let lore_spell = g.put(0, LORE, Zone::Hand);
    g.act(0, Move::Cast { card: lore_spell, targets: vec![vec![Target::Object(saga)]] }).unwrap();
    for _ in 0..6 {
        if let Some((0, Asked::Order(abilities))) = g.asked() { assert_eq!(abilities.len(), 2); break; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    cite!(["714.2c", "603.3b"]; assert!(matches!(g.asked(), Some((0, Asked::Order(_)))), "{:?}", g.asked()));
    resolve(&mut g);
    cite!(["714.2b", "714.2c"]; assert_eq!(g.players[0].life, 21 + 1 + 5));
    cite!(["714.4"]; assert!(!g.battlefield.contains(&saga), "read its final chapter"));
    // removing a counter triggers nothing; putting one back reaches the chapter again
    let mut g = game();
    let saga = put(&mut g, CHRONICLE);
    add_lore(&mut g, saga, ONE_LORE);
    assert_eq!(g.players[0].life, 21);
    g.remove_counters(saga, "lore", 1);
    assert_eq!(lore(&g, saga), 0);
    // the removal triggered nothing: the next counter makes chapter I trigger once more, alone
    add_lore(&mut g, saga, ONE_LORE);
    cite!(["714.2b"]; assert_eq!(g.players[0].life, 22, "chapter I again"));
    assert!(g.battlefield.contains(&saga));
}

#[test]
fn r714_4_the_final_chapter_on_the_stack_keeps_the_saga_and_countering_it_lets_it_go() {
    for countered in [false, true] {
        let mut g = game();
        let saga = put(&mut g, CHRONICLE);
        add_lore(&mut g, saga, LORE);
        assert_eq!(g.players[0].life, 20 + 1 + 2);
        // III triggers; while it waits on the stack the Saga stays
        let one = g.put(0, ONE_LORE, Zone::Hand);
        g.act(0, Move::Cast { card: one, targets: vec![vec![Target::Object(saga)]] }).unwrap();
        g.act(0, Move::Pass).unwrap();
        g.act(1, Move::Pass).unwrap();
        assert_eq!(lore(&g, saga), 3);
        assert_eq!(g.stack.len(), 1);
        cite!(["714.4"]; assert!(g.battlefield.contains(&saga), "714.4: its chapter ability has not left the stack"));
        if countered {
            let stifle = g.put(1, STIFLE, Zone::Hand);
            let ability = g.stack[0];
            g.act(0, Move::Pass).unwrap();
            g.act(1, Move::Cast { card: stifle, targets: vec![vec![Target::Object(ability)]] }).unwrap();
            resolve(&mut g);
            assert_eq!(g.players[0].life, 20 + 1 + 2, "III never resolved");
        } else {
            resolve(&mut g);
            assert_eq!(g.players[0].life, 20 + 1 + 2 + 3);
        }
        cite!(["714.4"]; case: countered; assert!(!g.battlefield.contains(&saga)));
    }
}

#[test]
fn r714_3c_each_saga_gets_a_counter_and_a_copy_continues_the_same() {
    let mut g = game();
    let first = g.put(0, CHRONICLE, Zone::Hand);
    g.act(0, Move::Cast { card: first, targets: vec![] }).unwrap();
    resolve(&mut g);
    let second = g.put(0, CHRONICLE, Zone::Hand);
    g.act(0, Move::Cast { card: second, targets: vec![] }).unwrap();
    resolve(&mut g);
    let sagas: Vec<ObjId> = g.battlefield.iter().copied().filter(|o| g.obj(*o).chars.name == "Chronicle").collect();
    assert_eq!(g.players[0].life, 22);
    // an opponent's Saga gets its counter in their precombat main phase, not in ours
    let theirs = g.put(1, CHRONICLE, Zone::Battlefield);
    next_main(&mut g);
    cite!(["714.3c"]; assert_eq!(lore(&g, theirs), 1));
    cite!(["714.3c"]; assert!(sagas.iter().all(|s| lore(&g, *s) == 2)));
    // two chapter IIs, both its controller's: they order them before anyone has priority
    cite!(["603.3b"]; assert!(matches!(g.asked(), Some((0, Asked::Order(a))) if a.len() == 2), "{:?}", g.asked()));
    let mut copy = g.clone();
    for g in [&mut g, &mut copy] { resolve(g); }
    assert_eq!(format!("{g:?}"), format!("{copy:?}"));
    assert_eq!(g.players[0].life, 22 + 4);
}

#[test]
fn r111_3_urzas_constructs_share_one_definition() {
    let mut g = game();
    let saga = put(&mut g, URZA);
    // Chapters I and II at once: II resolves first and grants the Construct
    // ability before I grants its mana ability (603.3b, 405.2).
    add_lore(&mut g, saga, LORE);
    let sources: Vec<_> = (0..4).map(|_| put(&mut g, WASTES)).collect();
    for batch in 0..2 {
        g.untap(saga);
        let plan = sources[batch*2..batch*2+2].iter().map(|source| game::legal::Step { plan: vec![], x: None, life: vec![], with: None,
            source: *source, ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None,
        }).collect();
        g.act(0, paid("CC", Move::WithMana(plan, Box::new(Move::Activate { source: saga, ability: 3, targets: vec![], paying: vec![] })))).unwrap();
        resolve(&mut g);
    }
    let constructs: Vec<ObjId> = g.battlefield.iter().copied().filter(|o| g.obj(*o).chars.name == "Construct Token").collect();
    assert_eq!(constructs.len(), 2);
    assert!(std::ptr::eq(g.def(constructs[0]).unwrap(), g.def(constructs[1]).unwrap()), "one compiled definition");
    assert!(constructs.iter().all(|c| g.power(*c) == Some(2)));
}

#[test]
fn simulation_plays_urzas_saga_and_resolves_chapter_one() {
    let mut g = game();
    let saga = g.put(0, URZA, Zone::Hand);
    let mut sim = Simulation::new(g);
    let r = sim.request().unwrap().unwrap().clone();
    sim.respond(r.id, r.player, Move::PlayLand(saga)).unwrap();
    for _ in 0..6 {
        let r = sim.request().unwrap().unwrap().clone();
        if sim.game().unwrap().stack.is_empty() { break; }
        sim.respond(r.id, r.player, Move::Pass).unwrap();
    }
    let g = sim.game().unwrap();
    assert!(sim.fault().is_none());
    let on = *g.battlefield.iter().find(|o| g.obj(**o).chars.name == "Urza's Saga").unwrap();
    cite!(["714.3a"]; assert_eq!(lore(g, on), 1));
    let r = sim.request().unwrap().unwrap().clone();
    sim.respond(r.id, r.player, Move::Activate { source: on, ability: 3, targets: vec![], paying: vec![] }).unwrap();
    cite!(["714.2b", "605.1a"]; assert_eq!(sim.game().unwrap().players[0].pool.total(), 1));
}

#[test]
fn r714_4_pending_chapter_keeps_its_identity_when_the_source_becomes_another_saga() {
    let card = |name, kind, text| {let d=CardDef::new(name,"{0}",kind,text,None).unwrap();check(&d).unwrap();d};
    let mut g=Game::new(vec![
        card("Original","Enchantment — Saga","I — You gain 3 life."),
        card("Model","Enchantment — Saga","This enchantment can't be countered.\nI — You gain 7 life."),
        card("Change","Instant","Target enchantment becomes a copy of target enchantment until end of turn."),
    ],2,20);
    let model=g.put(0,1,Zone::Battlefield);
    let saga=g.put(0,0,Zone::Hand);g.act(0,Move::Cast{card:saga,targets:vec![]}).unwrap();
    for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
    let saga=*g.battlefield.iter().find(|o|**o!=model).unwrap();
    assert_eq!(g.stack.len(),1,"original final chapter pending");
    let change=g.put(0,2,Zone::Hand);g.act(0,Move::Cast{card:change,targets:vec![vec![Target::Object(saga)],vec![Target::Object(model)]]}).unwrap();
    for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
    cite!(["714.4"]; assert!(g.battlefield.contains(&saga),"captured chapter remains a chapter after its source changes definition"));
    assert_eq!(g.def(saga).unwrap().name(),"Model");
    for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
    cite!(["113.7a"]; assert_eq!(g.players[0].life,23,"original captured chapter resolves"));
    cite!(["714.4"]; assert!(!g.battlefield.contains(&saga),"sacrifice waits until original chapter leaves the stack"));
}
