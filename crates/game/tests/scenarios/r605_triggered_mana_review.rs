//! Triggered mana abilities (605.1b, 605.4a): several on one tap, their answers in
//! order and a plan that counts them all; Overgrowth's two; a land tapped for an
//! ability that isn't a mana ability triggers none; a direct activation's leftover answers
//! go to the trigger; a supplied plan with many lands and an any-color trigger.
use crate::support::{cite,paid};
use game::state::{Answer, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::mana::ManaSymbol;
use ir::Color;

const FOREST: usize = 0;
const WILD: usize = 1;
const FERTILE: usize = 2;
const BLOOM: usize = 3;
const OVERGROWTH: usize = 4;
const SPRING: usize = 5;
const BIG: usize = 6;
const HUGE: usize = 7;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let real = |name: &str| CardDef::from_faces(&catalog[name]).unwrap();
    let defs = vec![real("Forest"), real("Wild Growth"), real("Fertile Ground"), real("Vernal Bloom"), real("Overgrowth"),
        CardDef::new("Spring", "", "Land", "{T}: Add {G}.\n{T}: You gain 1 life.", None).unwrap(),
        CardDef::new("Big", "{G}{G}{G}{U}", "Sorcery", "You gain 1 life.", None).unwrap(),
        CardDef::new("Huge", "{8}{G}{U}", "Sorcery", "You gain 1 life.", None).unwrap()];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, FOREST, Zone::Library); } }
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn g_() -> ManaSymbol { ManaSymbol::Color(Color::Green) }
fn u_() -> ManaSymbol { ManaSymbol::Color(Color::Blue) }
/// Player 0 casts this Aura on `land`, paid from a pool given for it; a color asked is blue.
fn enchant(g: &mut Game, aura: usize, land: ObjId) {
    let card = g.put(0, aura, Zone::Hand);
    g.players[0].pool.colors[4] += 3;
    g.players[0].pool.colorless += 3;
    g.act(0, paid(match aura { WILD=>"G", FERTILE=>"CG", OVERGROWTH=>"CCG", _=>panic!("unauthored aura cost") },Move::Cast { card, targets: vec![vec![Target::Object(land)]] })).unwrap();
    while !g.stack.is_empty() || g.decision.is_some() {
        match g.decision.clone() {
            Some(Decision::Choose { player, .. }) => g.act(player, Move::Choose(Answer::Value(game::binding::Value::Color(Color::Blue)))).unwrap(),
            Some(d) => panic!("{d:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    g.players[0].pool = Default::default();
}
fn pool(g: &Game) -> (u32, u32, u32) { (g.players[0].pool.colors[4], g.players[0].pool.colors[1], g.players[0].pool.total()) }

/// Wild Growth, Fertile Ground and Vernal Bloom on one Forest: tapped, it adds {G}, {G}, {G}
/// and one asked color, nothing on the stack.
#[test]
fn several_triggers_on_one_tap() {
    let mut g = game();
    let forest = g.put(0, FOREST, Zone::Battlefield);
    g.put(0, BLOOM, Zone::Battlefield);
    enchant(&mut g, WILD, forest);
    enchant(&mut g, FERTILE, forest);
    assert_eq!(g.players[0].pool.total(), 0);
    g.act(0, Move::Activate { source: forest, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    let mut asked = 0;
    while let Some((0, game::legal::Asked::Mana { .. })) = g.asked() {
        asked += 1;
        g.act(0, Move::Choose(Answer::Mana(vec![u_()]))).unwrap();
    }
    // how many questions the client is asked is the answer protocol, not a rule: uncited
    assert_eq!(asked, 1, "only Fertile Ground asks");
    cite!(["106.12a", "605.1b"]; assert_eq!(pool(&g), (3, 1, 4), "each trigger added its mana"));
    cite!(["605.4a"]; assert!(g.stack.is_empty(), "triggered mana abilities don't use the stack"));
}

/// The same Forest pays for {G}{G}{G}{U} alone: the supplied plan taps it once and casting
/// with it leaves the pool empty.
#[test]
fn a_plan_counts_every_trigger() {
    let mut g = game();
    let forest = g.put(0, FOREST, Zone::Battlefield);
    g.put(0, BLOOM, Zone::Battlefield);
    enchant(&mut g, WILD, forest);
    enchant(&mut g, FERTILE, forest);
    let big = g.put(0, BIG, Zone::Hand);
    let plan = vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source: forest, ability: 0, paying: vec![], adding: vec![vec![u_()]], chose: vec![], cost_branches: None }];
    cite!(["601.2g", "605.4a"]; ok: g.act(0, Move::WithMana(plan, Box::new(paid("GGGU",Move::Cast { card: big, targets: vec![] })))));
    cite!(["601.2h"]; assert_eq!(g.players[0].pool.total(), 0, "all four mana paid {{G}}{{G}}{{G}}{{U}}"));
    cite!(["605.4a"]; assert!(g.stack.len() == 1 && g.decision.is_none(), "only the spell is on the stack"));
}

/// Activated directly: the Forest's {G} asks nothing, so the one answer given is Fertile
/// Ground's color; two answers are one too many; none, and the color is asked.
#[test]
fn a_direct_activations_answer_goes_to_the_trigger() {
    let mut g = game();
    let forest = g.put(0, FOREST, Zone::Battlefield);
    enchant(&mut g, FERTILE, forest);
    let activate = || Box::new(Move::Activate { source: forest, ability: 0, targets: vec![], paying: vec![] });
    // which answers a move may carry is the answer protocol, not a rule: uncited
    crate::support::refused(g.clone().act(0, Move::Adding(vec![vec![g_()], vec![u_()]], activate())), "that ability offers no such choice of mana (608.2d)");
    let mut asked = g.clone();
    asked.act(0, *activate()).unwrap();
    // the trigger asks its color at once, not after a pass from the stack
    cite!(["605.4a"]; assert!(matches!(asked.asked(), Some((0, game::legal::Asked::Mana { .. }))), "{:?}", asked.asked()));
    g.act(0, Move::Adding(vec![vec![u_()]], activate())).unwrap();
    assert!(!matches!(g.asked(), Some((_, game::legal::Asked::Mana { .. }))));
    cite!(["106.12a", "605.1b"]; assert_eq!(pool(&g), (1, 1, 2), "the Forest's {{G}} and the trigger's {{U}}"));
}

/// Overgrowth: "adds an additional {G}{G}", without a question.
#[test]
fn overgrowth_adds_two() {
    let mut g = game();
    let forest = g.put(0, FOREST, Zone::Battlefield);
    enchant(&mut g, OVERGROWTH, forest);
    g.act(0, Move::Activate { source: forest, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    // that nothing is asked is the answer protocol (the mana is fixed): uncited
    assert!(!matches!(g.asked(), Some((_, game::legal::Asked::Mana { .. }))), "{:?}", g.asked());
    cite!(["106.12a", "605.1b"]; assert_eq!(pool(&g), (3, 0, 3), "{{G}} and the additional {{G}}{{G}}"));
}

/// Wild Growth on a land tapped for its life ability: not tapped for mana (106.12), so no {G}
/// (106.12a; 605.1b "from the activation or resolution of an activated mana ability").
#[test]
fn tapped_for_another_ability_it_triggers_nothing() {
    let mut g = game();
    let spring = g.put(0, SPRING, Zone::Battlefield);
    enchant(&mut g, WILD, spring);
    g.act(0, Move::Activate { source: spring, ability: 1, targets: vec![], paying: vec![] }).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    // the guard that the life ability was activated and resolved: uncited
    assert_eq!(g.players[0].life, 21);
    cite!(["106.12", "106.12a"]; assert_eq!(g.players[0].pool.total(), 0, "not tapped for mana"));
}

/// Twelve Forests, one with Fertile Ground: the authored plan pays {8}{G}{U}.
#[test]
fn an_explicit_many_land_payment_includes_the_any_color_trigger() {
    let mut g = game();
    let forest = g.put(0, FOREST, Zone::Battlefield);
    enchant(&mut g, FERTILE, forest);
    let others: Vec<_> = (0..11).map(|_| g.put(0, FOREST, Zone::Battlefield)).collect();
    let huge = g.put(0, HUGE, Zone::Hand);
    let step = |source| game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source, ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None };
    let mut plan = vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, adding: vec![vec![u_()]], ..step(forest) }];
    plan.extend(others[..8].iter().copied().map(step));
    cite!(["601.2g", "601.2h"]; ok: g.act(0, Move::WithMana(plan, Box::new(paid("GGGGGGGGGU",Move::Cast { card: huge, targets: vec![] })))));
    assert_eq!(g.stack.len(), 1);
}
