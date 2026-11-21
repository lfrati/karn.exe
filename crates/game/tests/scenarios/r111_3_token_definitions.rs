//! A token's text is what the effect that creates it says (111.3, 111.4): "a 0/1 colorless
//! Eldrazi Spawn creature token with \"Sacrifice this token: Add {C}.\"" makes a token whose
//! abilities are that quoted text, activated, paid for, triggered, applied and copied as any
//! object's are. It is no card (111.6), and in a zone other than the battlefield it ceases to
//! exist (111.7). The witnesses are their Oracle text; the others are test cards.
use crate::support::{cite, paid};
use game::card::CardDef;
use game::legal::Asked;
use game::play::check;
use game::simulation::Simulation;
use game::state::Answer;
use game::{Game, Move, ObjId, Target, Zone};
use ir::mana::ManaSymbol;
use ir::Color;
use std::sync::OnceLock;

const COMMAND: usize = 0;
const RUMBLE: usize = 1;
const FLESHRAKER: usize = 2;
const CIRCLE: usize = 3;
const BAUBLE: usize = 4;
const GROW: usize = 5;
const ASSEMBLE: usize = 6;
const DUPLICATE: usize = 7;
const MEND: usize = 8;
const TWIN: usize = 9;
const BOTH: usize = 10;
const OPT: usize = 11;
const MOUNTAIN: usize = 12;

fn oracle(name: &str) -> CardDef {
    static FACES: OnceLock<Vec<oracle::corpus::Face>> = OnceLock::new();
    let faces = FACES.get_or_init(|| oracle::corpus::load(&oracle::corpus::default_path()).unwrap());
    CardDef::from_faces(CardDef::catalog(faces).get(name).unwrap()).unwrap()
}

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text| CardDef::new(name, cost, types, text, None).expect(name);
    vec![
        oracle("Kozilek's Command"),
        oracle("Malevolent Rumble"),
        oracle("Glaring Fleshraker"),
        oracle("Circle of Power"),
        card("Bauble", "{0}", "Artifact", ""),
        card("Grow", "{0}", "Sorcery", "Create a 0/1 green Plant creature token with \"Sacrifice this token: Add {G}.\""),
        // the Construct Urza's Saga's second chapter makes
        card("Assemble", "{0}", "Sorcery", "Create a 0/0 colorless Construct artifact creature token with \"This token gets +1/+1 for each artifact you control.\""),
        card("Duplicate", "{0}", "Sorcery", "Create a token that's a copy of target creature you control."),
        card("Mend", "{C}{G}", "Instant", "You gain 1 life."),
        card("Twin", "{C}{C}", "Instant", "You gain 1 life."),
        card("Both", "{G}{G}", "Instant", "You gain 1 life."),
        card("Opt", "{0}", "Instant", "You gain 1 life."),
        CardDef::new("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None).unwrap(),
    ]
}

fn game() -> Game {
    Game::new(defs(), 2, 20)
}

/// Pass until the stack is empty, answering what resolving asks with nothing optional.
fn resolve(g: &mut Game) { resolve_with_orders(g, &[]); }
fn resolve_with_orders(g: &mut Game, orders: &[(usize,Vec<ObjId>)]) {
    let mut orders=orders.iter();
    for _ in 0..40 {
        match g.asked() {
            Some((p, Asked::Select { min: 0, .. })) => g.act(p, Move::Choose(Answer::Objects(vec![]))).unwrap(),
            Some((p, Asked::May)) => g.act(p, Move::Choose(Answer::No)).unwrap(),
            // scry: everything back on top, in the order shown
            Some((p, Asked::Arrange { cards, .. })) => g.act(p, Move::Choose(Answer::Arrange { top: cards, away: vec![] })).unwrap(),
            Some((p, Asked::OrderCards { to: game::state::Placement::Zone(Zone::Graveyard), cards })) => {
                let (owner,permutation)=orders.next().expect("caller-authored graveyard order");
                assert_eq!(p,*owner); assert_eq!(cards.len(),permutation.len());assert!(cards.iter().all(|o|permutation.contains(o)));
                g.act(p,Move::Choose(Answer::Objects(permutation.clone()))).unwrap();
            }
            _ if g.stack.is_empty() => {assert!(orders.next().is_none());return},
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("the stack did not empty: {:?}", g.asked());
}

fn tokens(g: &Game, name: &str) -> Vec<ObjId> {
    g.battlefield.iter().copied().filter(|o| g.obj(*o).token() && g.obj(*o).chars.name == name).collect()
}

/// Kozilek's Command for X=2: two Spawns for player 0, and no cards exiled.
fn command_spawns(g: &mut Game) -> Vec<ObjId> {
    let command = g.put(0, COMMAND, Zone::Hand);
    for _ in 0..4 { g.add_mana(0, ManaSymbol::Colorless); }
    let cast = paid("CCCC", Move::Cast { card: command, targets: vec![vec![Target::Player(0)], vec![]] });
    g.act(0, Move::WithX(2, Box::new(Move::Modes(vec![0, 3], Box::new(cast))))).unwrap();
    resolve(g);
    tokens(g, "Eldrazi Spawn Token")
}

fn cast(g: &mut Game, def: usize) {
    let card = g.put(0, def, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    resolve(g);
}

#[test]
fn r111_3_the_witnesses_are_admitted_and_unsupported_quoted_text_is_refused() {
    for d in defs() { check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0)); }
    // a token whose quoted ability the engine does not run is refused, not made without it
    let text = "Create a 1/1 white Soldier creature token with \"When this token dies, surveil 2. When you do, draw a card.\"";
    let refused = CardDef::new("Refused", "{0}", "Sorcery", text, None).unwrap();
    let why = check(&refused).unwrap_err().0;
    assert!(why.contains("token"), "{why}");
}

#[test]
fn r111_3_a_spawn_is_no_card_and_its_quoted_mana_ability_is_activated_as_any() {
    let mut g = game();
    let spawns = command_spawns(&mut g);
    assert_eq!(spawns.len(), 2);
    let spawn = g.obj(spawns[0]);
    cite!(["111.6"]; assert!(spawn.card.is_none(), "no card (111.6)"));
    cite!(["111.3"]; assert_eq!((spawn.chars.colors.clone(), g.power(spawns[0]), g.toughness(spawns[0])), (vec![], Some(0), Some(1))));
    // a copy of the game goes on the same way
    let mut copy = g.clone();
    for g in [&mut g, &mut copy] {
        g.act(0, Move::Activate { source: spawns[0], ability: 0, targets: vec![], paying: vec![] }).unwrap();
        assert_eq!(g.players[0].pool.total(), 1, "{{C}}");
        g.act(0, Move::Pass).unwrap();
    }
    assert_eq!(format!("{g:?}"), format!("{copy:?}"));
    // sacrificed, it went to the graveyard and ceased to exist there (111.7)
    cite!(["111.7"]; assert_eq!(tokens(&g, "Eldrazi Spawn Token"), vec![spawns[1]]));
    assert!(g.players[0].graveyard.iter().all(|o| !g.obj(*o).token()), "only the Command is there");
    assert_eq!(g.obj(spawns[0]).zone, Zone::Gone);
}

#[test]
fn r106_tokens_of_two_definitions_are_two_sources_of_mana() {
    let mut g = game();
    // These authored modes and spells ask no resolving choice. Reject an
    // unexpected question instead of using the optional-answer helper above.
    let resolve_priority = |g: &mut Game| {
        for _ in 0..12 {
            if g.stack.is_empty() { return; }
            assert!(matches!(g.asked(), Some((_, Asked::Priority))), "{:?}", g.asked());
            g.act(g.priority, Move::Pass).unwrap();
        }
        panic!("the authored stack did not resolve");
    };
    let command = g.put(0, COMMAND, Zone::Hand);
    for _ in 0..4 { g.add_mana(0, ManaSymbol::Colorless); }
    g.act(0, Move::WithX(2, Box::new(Move::Modes(vec![0, 3], Box::new(paid("CCCC",
        Move::Cast { card: command, targets: vec![vec![Target::Player(0)], vec![]] })))))).unwrap();
    resolve_priority(&mut g);
    let spawns = tokens(&g, "Eldrazi Spawn Token");
    let grow = g.put(0, GROW, Zone::Hand);
    g.act(0, Move::Cast { card: grow, targets: vec![] }).unwrap();
    resolve_priority(&mut g);
    let plant = tokens(&g, "Plant Token");
    assert_eq!((spawns.len(), plant.len()), (2, 1));
    let [mend, twin, both] = [MEND, TWIN, BOTH].map(|d| g.put(0, d, Zone::Hand));
    let step = |source| game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source, ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None };
    let cast = |card, plan, mana: &str| paid(mana,Move::WithMana(plan, Box::new(Move::Cast { card, targets: vec![] })));
    cite!(["111.3"]; ok: g.clone().act(0, cast(twin, spawns.iter().copied().map(step).collect(),"CC")));
    let before = format!("{g:?}");
    cite!(["601.2h","118.3a"]; refused("that mana is not in your mana pool"): g.act(0, cast(both, vec![step(plant[0]), step(spawns[0])],"GG")));
    assert_eq!(format!("{g:?}"), before);
    let plan = vec![step(spawns[0]), step(plant[0])];
    g.act(0, paid("CG",Move::WithMana(plan, Box::new(Move::Cast { card: mend, targets: vec![] })))).unwrap();
    resolve_priority(&mut g);
    assert_eq!(g.players[0].life, 21);
    assert_eq!((tokens(&g, "Eldrazi Spawn Token").len(), tokens(&g, "Plant Token").len()), (1, 0));
}

#[test]
fn r604_the_construct_has_its_static_ability() {
    let mut g = game();
    cast(&mut g, ASSEMBLE);
    // "for each artifact you control": itself
    let construct = tokens(&g, "Construct Token");
    assert_eq!(construct.len(), 1, "a 0/0 that the static keeps alive");
    cite!(["111.3", "604.1"]; assert_eq!((g.power(construct[0]), g.toughness(construct[0])), (Some(1), Some(1))));
    g.put(0, BAUBLE, Zone::Battlefield);
    g.put(0, BAUBLE, Zone::Battlefield);
    cite!(["613.4c"]; assert_eq!(g.power(construct[0]), Some(3)));
    // a second Construct counts the first, and is counted
    cast(&mut g, ASSEMBLE);
    let constructs = tokens(&g, "Construct Token");
    assert_eq!(constructs.len(), 2, "both Constructs survive");
    for object in constructs {
        assert_eq!((g.power(object), g.toughness(object)), (Some(4), Some(4)));
    }
}

#[test]
fn r603_the_wizard_has_its_triggered_ability_and_the_spell_its_effects() {
    let mut g = game();
    for _ in 0..2 { g.put(0, MOUNTAIN, Zone::Library); } // Circle draws exactly these cards.
    let circle = g.put(0, CIRCLE, Zone::Hand);
    g.add_mana(0, ManaSymbol::Color(Color::Black));
    for _ in 0..3 { g.add_mana(0, ManaSymbol::Colorless); }
    g.act(0, paid("CCCB", Move::Cast { card: circle, targets: vec![] })).unwrap();
    resolve(&mut g);
    assert_eq!((g.players[0].hand.len(), g.players[0].life), (2, 18));
    let wizard = tokens(&g, "Wizard Token");
    assert_eq!(wizard.len(), 1);
    cite!(["111.3"]; assert_eq!((g.obj(wizard[0]).chars.colors.clone(), g.power(wizard[0])), (vec![Color::Black], Some(1)), "+1/+0 until end of turn"));
    // a noncreature spell: the token deals 1 damage to the opponent, with lifelink
    cast(&mut g, OPT);
    cite!(["111.3"]; assert_eq!(g.players[1].life, 19));
    cite!(["702.15b"]; assert_eq!(g.players[0].life, 18 + 1 + 1, "Opt's life, and the Wizard's lifelink"));
}

#[test]
fn r603_fleshraker_makes_a_spawn_whose_entering_it_sees() {
    let mut g = game();
    g.put(0, FLESHRAKER, Zone::Battlefield);
    // a colorless spell: a Spawn, a colorless creature entering: 1 damage to each opponent
    cast(&mut g, BAUBLE);
    assert_eq!(tokens(&g, "Eldrazi Spawn Token").len(), 1);
    cite!(["603.6a"]; assert_eq!(g.players[1].life, 19));
    // Malevolent Rumble (green) makes one too, and the Fleshraker sees it enter
    let remainder: Vec<_>=(0..4).map(|_| g.put(0,MOUNTAIN,Zone::Library)).collect();
    let rumble = g.put(0, RUMBLE, Zone::Hand);
    g.add_mana(0, ManaSymbol::Color(Color::Green));
    g.add_mana(0, ManaSymbol::Colorless);
    g.act(0, paid("CG", Move::Cast { card: rumble, targets: vec![] })).unwrap();
    resolve_with_orders(&mut g, &[(0,vec![remainder[0],remainder[1],remainder[2],remainder[3]])]);
    assert_eq!(tokens(&g, "Eldrazi Spawn Token").len(), 2);
    cite!(["603.6a"]; assert_eq!(g.players[1].life, 18));
}

#[test]
fn kozileks_command_its_other_modes() {
    // scry X then draw, and exile a creature with mana value X or less: a Spawn (0), not the
    // Fleshraker (3)
    let mut g = game();
    let spawns = command_spawns(&mut g);
    for _ in 0..2 { g.put(0, MOUNTAIN, Zone::Library); } // Scry two, then draw one.
    let fleshraker = g.put(1, FLESHRAKER, Zone::Battlefield);
    let hand = g.players[0].hand.len();
    let command = g.put(0, COMMAND, Zone::Hand);
    for _ in 0..4 { g.add_mana(0, ManaSymbol::Colorless); }
    let with = |target: ObjId| Move::WithX(2, Box::new(Move::Modes(vec![1, 2], Box::new(paid("CCCC", Move::Cast { card: command,
        targets: vec![vec![Target::Player(0)], vec![Target::Object(target)]] })))));
    cite!(["107.3", "115.1"]; refused("an illegal target, Glaring Fleshraker: its mana value is 3, and the target says mana value 2 or less (115.1)"): g.act(0, with(fleshraker)));
    g.act(0, with(spawns[0])).unwrap();
    resolve(&mut g);
    assert_eq!(g.players[0].hand.len(), hand + 1, "drew a card after scrying");
    assert!(!g.battlefield.contains(&spawns[0]));
    // exile up to X target cards from graveyards
    let mut g = game();
    let dead = [g.put(1, BAUBLE, Zone::Graveyard), g.put(0, BAUBLE, Zone::Graveyard)];
    let command = g.put(0, COMMAND, Zone::Hand);
    for _ in 0..4 { g.add_mana(0, ManaSymbol::Colorless); }
    g.act(0, Move::WithX(2, Box::new(Move::Modes(vec![0, 3], Box::new(paid("CCCC", Move::Cast { card: command,
        targets: vec![vec![Target::Player(1)], dead.iter().map(|o| Target::Object(*o)).collect()] })))))).unwrap();
    resolve(&mut g);
    assert!(g.players.iter().all(|p| p.graveyard.iter().all(|o| g.obj(*o).chars.name != "Bauble")));
    cite!(["111.2"]; assert_eq!(g.battlefield.iter().filter(|o| g.controller(**o) == 1 && g.obj(**o).token()).count(), 2, "the target player's Spawns"));
}

#[test]
fn r707_2_a_copy_of_a_spawn_has_its_ability() {
    let mut g = game();
    let spawns = command_spawns(&mut g);
    let duplicate = g.put(0, DUPLICATE, Zone::Hand);
    g.act(0, Move::Cast { card: duplicate, targets: vec![vec![Target::Object(spawns[0])]] }).unwrap();
    resolve(&mut g);
    let all = tokens(&g, "Eldrazi Spawn Token");
    assert_eq!(all.len(), 3);
    let copy = *all.iter().find(|o| !spawns.contains(o)).unwrap();
    g.act(0, Move::Activate { source: copy, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    cite!(["707.2", "111.3"]; assert_eq!(g.players[0].pool.total(), 1));
}

#[test]
fn simulation_lists_and_activates_a_spawn_across_retirement() {
    let mut g = game();
    let spawns = command_spawns(&mut g);
    let twin = g.put(0, TWIN, Zone::Hand);
    let mut sim = Simulation::new(g);
    let request = sim.request().unwrap().unwrap().clone();
    let plan = spawns.iter().copied().map(|source| game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source, ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None }).collect();
    sim.respond(request.id, 0, Move::WithMana(plan, Box::new(paid("CC", Move::Cast { card: twin, targets: vec![] })))).unwrap();
    for _ in 0..4 {
        let Some(r) = sim.request().unwrap().cloned() else { break };
        if sim.game().unwrap().stack.is_empty() { break; }
        sim.respond(r.id, r.player, Move::Pass).unwrap();
    }
    let g = sim.game().unwrap();
    // Uncited (implementation): the Simulation keeps running, no fault, when the sacrificed Spawns are retired from its object table
    assert!(sim.fault().is_none());
    // Uncited (card text): Twin's "You gain 1 life."
    assert_eq!(g.players[0].life, 21);
    // sacrificed to pay (701.21a), each is in the graveyard or has ceased to exist there (111.7)
    cite!(["701.21a", "111.7"]; assert!(spawns.iter().all(|s| g.get_object(*s).is_none_or(|o| o.zone != Zone::Battlefield))));
}
