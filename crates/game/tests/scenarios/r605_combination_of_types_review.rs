//! Vivi Ornitier's "{0}: Add X mana in any combination of {U} and/or {R}, where X is
//! Vivi Ornitier's power. Activate only during your turn and only once each turn.": at power 0
//! it adds nothing (107.1b reads no negative X, 106.4); its mana pays for a spell as it is cast
//! (605.3a, 601.2g); a Vivi that left and came back is a new object, not yet activated (400.7).
use crate::support::{cite,paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Color;
use ir::Part;
use ir::mana::ManaSymbol;

const VIVI: usize = 0;
const BOLT: usize = 1;
const FLICKER: usize = 2;
const ISLAND: usize = 3;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_faces(&CardDef::catalog(&faces)[name]).unwrap();
    let defs = vec![real("Vivi Ornitier"), real("Lightning Bolt"),
        CardDef::new("Flicker", "{0}", "Instant", "Exile target creature, then return it to the battlefield under its owner's control.", None).unwrap(),
        real("Island")];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn r() -> ManaSymbol { ManaSymbol::Color(Color::Red) }
fn add(g: &mut Game, vivi: ObjId, adding: Vec<Vec<ManaSymbol>>) -> Result<(), String> {
    g.act(0, Move::Adding(adding, Box::new(Move::Activate { source: vivi, ability: 0, targets: vec![], paying: vec![] })))
}
fn resolve(g: &mut Game) {
    for _ in 0..30 {
        if g.stack.is_empty() && g.decision.is_none() { return; }
        g.act(g.priority, Move::Pass).unwrap();
    }
}

/// Power 0: X is 0, and the ability adds no mana.
#[test]
fn at_power_zero_it_adds_nothing() {
    let mut g = game();
    let vivi = g.put(0, VIVI, Zone::Battlefield);
    let before = g.players[0].pool.clone();
    cite!(["107.3c"]; ok: add(&mut g, vivi, vec![]));
    cite!(["107.3c"]; assert_eq!(g.players[0].pool, before));
}

/// With power1 and no other mana, the authored plan activates Vivi forR; casting
/// Lightning Bolt that way leaves Vivi spent for the turn.
#[test]
fn its_mana_pays_for_a_spell_as_it_is_cast() {
    let mut g = game();
    let vivi = g.put(0, VIVI, Zone::Battlefield);
    g.put_counters(vivi, "+1/+1", 1);
    let bolt = g.put(0, BOLT, Zone::Hand);
    let plan = vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source: vivi, ability: 0, paying: vec![], adding: vec![vec![r()]], chose: vec![], cost_branches: None }];
    cite!(["605.3a", "601.2g"]; ok: g.act(0, Move::WithMana(plan, Box::new(paid("R",Move::Cast { card: bolt, targets: vec![vec![Target::Player(1)]] })))));
    let before = format!("{g:?}");
    cite!(["602.5"]; refused("that ability is activated only once each turn"): add(&mut g, vivi, vec![vec![r()]]));
    assert_eq!(format!("{g:?}"), before);
    resolve(&mut g);
    assert_eq!(g.players[1].life, 17 - 1, "3 from the Bolt and 1 from Vivi's trigger");
}

/// Activated, then flickered: the Vivi that returned is a new object and may activate this
/// turn (400.7, 602.5b).
#[test]
fn a_vivi_that_returned_may_activate_again() {
    let mut g = game();
    let vivi = g.put(0, VIVI, Zone::Battlefield);
    g.put_counters(vivi, "+1/+1", 1);
    add(&mut g, vivi, vec![vec![r()]]).unwrap();
    let flicker = g.put(0, FLICKER, Zone::Hand);
    g.act(0, Move::Cast { card: flicker, targets: vec![vec![Target::Object(vivi)]] }).unwrap();
    resolve(&mut g);
    let back = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(VIVI)).unwrap();
    assert_ne!(back, vivi);
    // Flicker is a noncreature spell: the old Vivi got a counter, the new one has none
    g.put_counters(back, "+1/+1", 1);
    add(&mut g, back, vec![vec![r()]]).unwrap();
}

fn u() -> ManaSymbol { ManaSymbol::Color(Color::Blue) }

/// Activated on its own, Vivi's mana said in another order is the offered mana (106.4).
#[test]
fn activated_alone_its_mana_may_be_said_in_any_order() {
    let mut g = game();
    let vivi = g.put(0, VIVI, Zone::Battlefield);
    g.put_counters(vivi, "+1/+1", 2);
    add(&mut g, vivi, vec![vec![r(), u()]]).unwrap();
    // Uncited (card text): "Add X mana in any combination of {U} and/or {R}": a combination has no order, so {R}{U} is one blue and one red.
    assert_eq!((g.players[0].pool.colors[1], g.players[0].pool.colors[3]), (1, 1));
}

/// At power 20 each mana is asked in turn: a plan with one green among them refuses the cast,
/// committing nothing (605.3b).
#[test]
fn a_plan_with_one_unoffered_mana_among_many_is_refused() {
    let mut g = game();
    let vivi = g.put(0, VIVI, Zone::Battlefield);
    g.put_counters(vivi, "+1/+1", 20);
    let bolt = g.put(0, BOLT, Zone::Hand);
    let mut adding = vec![vec![r()]; 19];
    adding.push(vec![ManaSymbol::Color(Color::Green)]);
    let cast = |adding: Vec<Vec<ManaSymbol>>| Move::WithMana(
        vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source: vivi, ability: 0, paying: vec![], adding }],
        Box::new(paid("R",Move::Cast { card: bolt, targets: vec![vec![Target::Player(1)]] })));
    let logged = g.log.len();
    cite!(["608.2d"]; refused("that mana ability offers no such choice of mana (608.2d)"): g.act(0, cast(adding)));
    assert_eq!((g.log.len(), g.obj(bolt).zone), (logged, Zone::Hand));
    assert!(g.decision.is_none());
    g.act(0, cast(vec![vec![r()]; 20])).unwrap();
    cite!(["106.4"]; assert_eq!(g.players[0].pool.colors[3], 19, "one {{R}} paid the Bolt"));
}
