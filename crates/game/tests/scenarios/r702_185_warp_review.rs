//! Warp (702.185) and warp given to cards in a hand (613.1f).
//!
//! 702.185a: "Warp [cost]" means "You may cast this card from your hand by paying [cost]
//! rather than its mana cost" and "If this spell's warp cost was paid, exile the permanent this
//! spell becomes at the beginning of the next end step. Its owner may cast this card after the
//! current turn has ended for as long as it remains exiled."
//!
//! 707.10: a copy of a spell isn't cast; it copies the decisions made for it, including
//! alternative costs.
//!
//! Tannuk, Steadfast Second: "Other creatures you control have haste. Artifact cards and red
//! creature cards in your hand have warp {2}{R}."
//! Full Bore: "Target creature you control gets +3/+2 until end of turn. If that creature was
//! cast for its warp cost, it also gains trample and haste until end of turn."
//! "Boltbender": a red 3/3 creature card {3}{R} with only "Disguise {1}{R}" (702.168a), the real
//! card's turned-face-up trigger left out (unsupported).
use crate::support::cite;
use game::announcement::CastCost;
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const WARPER: usize = 0;
const FOREST: usize = 1;
const TANNUK: usize = 2;
const GIANT: usize = 3;
const RELIC: usize = 4;
const BORE: usize = 5;
const FORK: usize = 6;
const BOLTBENDER: usize = 7;
const SLINGER: usize = 8;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap();
    let c = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    let defs = vec![
        c("Warper", "{3}{G}", "Creature — Beast", "Warp {G}", Some((2, 2))),
        c("Forest", "", "Basic Land — Forest", "", None),
        real("Tannuk, Steadfast Second"),
        c("Giant", "{3}{R}", "Creature — Giant", "", Some((3, 3))),
        c("Relic", "{4}", "Artifact", "", None),
        real("Full Bore"),
        c("Fork", "{0}", "Instant", "Copy target spell.", None),
        c("Boltbender", "{3}{R}", "Creature — Goblin Wizard", "Disguise {1}{R}", Some((3, 3))),
        c("Slinger", "{3}{R}", "Creature — Spider", "Web-slinging {R}", Some((3, 3))),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, FOREST, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}

fn alt(card: ObjId, row: usize) -> Move {
    Move::CastFor(CastCost::Alternative(row), Box::new(Move::Cast { card, targets: vec![] }))
}
fn mana(g: &mut Game, green: u32, red: u32, generic: u32) {
    let p = &mut g.players[0].pool;
    p.colors[4] = green; p.colors[3] = red; p.colorless = generic;
}
fn step(g: &mut Game, target: Option<ObjId>) {
    match g.decision.clone() {
        Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
        Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
        Some(Decision::Targets { player, .. }) => g.act(player, Move::ChooseTargets(vec![target.map(Target::Object).into_iter().collect()])).unwrap(),
        Some(d) => panic!("unexpected decision {d:?}"),
        None => g.act(g.priority, Move::Pass).unwrap(),
    }
}
fn settle(g: &mut Game) {
    for _ in 0..30 {
        if g.decision.is_none() && g.stack.is_empty() { return; }
        step(g, None);
    }
    panic!("the stack did not settle");
}
/// To the next end step's priority (its delayed triggers on the stack), then resolve them.
fn through_end_step(g: &mut Game) {
    for _ in 0..200 {
        if g.part == Part::End && g.decision.is_none() { settle(g); return; }
        step(g, None);
    }
    panic!("no end step");
}
fn next_own_main(g: &mut Game) {
    let turn = g.turn;
    for _ in 0..400 {
        if g.turn != turn && g.active == 0 && g.part == Part::PrecombatMain && g.decision.is_none() && g.stack.is_empty() { return; }
        step(g, None);
    }
    panic!("no next main phase");
}
const NAMES: [&str; 9] = ["Warper", "Forest", "Tannuk, Steadfast Second", "Giant", "Relic", "Full Bore", "Fork", "Boltbender", "Slinger"];
/// The permanents of this card, a token copy of it among them, by its name.
fn on_battlefield(g: &Game, def: usize) -> Vec<ObjId> { g.battlefield.iter().copied().filter(|o| g.obj(*o).chars.name == NAMES[def]).collect() }
fn in_exile(g: &Game, def: usize) -> Option<ObjId> { g.exile.iter().copied().find(|o| g.obj(*o).card == Some(def)) }
/// Full Bore on this creature: whether it gained trample.
fn full_bore_tramples(g: &mut Game, creature: ObjId) -> bool {
    let bore = g.put(0, BORE, Zone::Hand);
    g.players[0].pool.colors[3] = 1;
    g.act(0, crate::support::paid("R",Move::Cast { card: bore, targets: vec![vec![Target::Object(creature)]] })).unwrap();
    settle(g);
    g.seen(creature, None).has(ir::Keyword::from_name("Trample").unwrap())
}

// Rules: 702.185a, 707.10
// Given: Warper cast for its warp cost {G}; Fork copies the spell on the stack.
// Expect: the copy copies the alternative cost paid (707.10), so the token it becomes is
// exiled at the next end step as well as the card (the token then ceases to exist, 111.8).
// Why: a plausible wrong behavior reads "warp cost was paid" off the card's cast record, which
// a copy has none of, leaving a permanent token copy of a warped creature.
#[test]
fn r707_10_a_copy_of_a_warped_creature_spell_is_exiled_too() {
    let mut g = game();
    let card = g.put(0, WARPER, Zone::Hand);
    mana(&mut g, 1, 0, 0);
    let row = 0; // Warper prints Warp as its first ability.
    g.act(0, crate::support::paid("G",alt(card, row))).unwrap();
    let spell = g.stack[0];
    let fork = g.put(0, FORK, Zone::Hand);
    g.act(0, Move::Cast { card: fork, targets: vec![vec![Target::Object(spell)]] }).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } // Fork resolves; the copy is now on the stack.
    assert_eq!(g.stack.len(), 2);
    let copy_spell = *g.stack.last().unwrap();
    assert_ne!(copy_spell, spell);
    settle(&mut g);
    let beasts = on_battlefield(&g, WARPER);
    assert_eq!(beasts.len(), 2, "the card and the token copy");
    assert!(beasts.iter().any(|o| g.obj(*o).token()));
    // Both known permanents have a delayed warp trigger; author their order here.
    for _ in 0..80 {
        if matches!(g.decision, Some(Decision::Order { .. })) { break; }
        step(&mut g, None);
    }
    let Some(Decision::Order { player: 0, abilities }) = &g.decision else { panic!("the two warp triggers need an order: {:?}", g.decision) };
    assert_eq!(abilities.len(), 2);
    assert_eq!(abilities.iter().filter_map(|a| a.source).collect::<std::collections::BTreeSet<_>>(), [spell, copy_spell].into_iter().collect());
    g.act(0, Move::Order(vec![0, 1])).unwrap();
    through_end_step(&mut g);
    cite!(["702.185a", "707.10"]; assert!(on_battlefield(&g, WARPER).is_empty(), "both exiled: {:?}", on_battlefield(&g, WARPER)));
    assert!(in_exile(&g, WARPER).is_some());
}

// Rules: 707.10, 601.2b
// Given: the token copy of a Warper spell cast for its warp cost.
// Expect: Full Bore gives it +3/+2 but not trample: a copy of a spell isn't cast (707.10), so
// the token was not "cast for its warp cost"; the warped card itself still gains trample.
#[test]
fn r707_10_full_bore_a_copy_of_a_warped_spell_was_not_cast() {
    let mut g = game();
    let card = g.put(0, WARPER, Zone::Hand);
    mana(&mut g, 1, 0, 0);
    let row = 0; // Warper prints Warp as its first ability.
    g.act(0, crate::support::paid("G",alt(card, row))).unwrap();
    let spell = g.stack[0];
    let fork = g.put(0, FORK, Zone::Hand);
    g.act(0, Move::Cast { card: fork, targets: vec![vec![Target::Object(spell)]] }).unwrap();
    settle(&mut g);
    let beasts = on_battlefield(&g, WARPER);
    let token = *beasts.iter().find(|o| g.obj(**o).token()).unwrap();
    let real = *beasts.iter().find(|o| !g.obj(**o).token()).unwrap();
    cite!(["707.10", "601.2b"]; assert!(!full_bore_tramples(&mut g, token), "the copy was not cast"));
    assert_eq!(g.power(token), Some(5));
    cite!(["601.2b", "702.185a"]; assert!(full_bore_tramples(&mut g, real)));
}

// Rules: 702.168a, 708.2, 613.1f, 702.185a
// Given: Tannuk on the battlefield; Boltbender, a red creature card with disguise, in hand.
// Expect: offered both its disguise cost (face down) and the given warp {2}{R} (face up). Cast
// face down for {3}, the 2/2 is not exiled at the end step and Full Bore gives it no trample;
// cast face up for the given warp, it is exiled.
// Why: the given warp row is "past the card's printed rows"; a face-down spell's own
// definition prints no rows, like a morph's.
#[test]
fn r702_168a_a_disguised_spell_beside_a_given_warp_is_not_warped() {
    let mut g = game();
    g.put(0, TANNUK, Zone::Battlefield);
    let card = g.put(0, BOLTBENDER, Zone::Hand);
    mana(&mut g, 0, 1, 2);
    // The fixture prints Disguise at row 0; Tannuk's grant follows at row 1.
    let (down, warp) = (0, 1);
    mana(&mut g, 0, 0, 3);
    g.act(0, crate::support::paid("CCC",alt(card, down))).unwrap();
    settle(&mut g);
    let face_down = *g.battlefield.last().unwrap();
    cite!(["702.168a", "708.2"]; assert_eq!((g.power(face_down), g.toughness(face_down)), (Some(2), Some(2))));
    cite!(["702.168a", "601.2b"]; assert!(!full_bore_tramples(&mut g, face_down), "not cast for a warp cost"));
    through_end_step(&mut g);
    cite!(["702.168a", "702.185a"]; assert!(g.battlefield.contains(&face_down) && g.exile.is_empty()));
    // the control: face up, for the given warp {2}{R}
    let mut g = game();
    g.put(0, TANNUK, Zone::Battlefield);
    let card = g.put(0, BOLTBENDER, Zone::Hand);
    mana(&mut g, 0, 1, 2);
    g.act(0, crate::support::paid("CCR",alt(card, warp))).unwrap();
    cite!(["702.185a"]; assert_eq!(g.players[0].pool.total(), 0));
    settle(&mut g);
    let up = *g.battlefield.last().unwrap();
    assert_eq!(g.obj(up).card, Some(BOLTBENDER));
    through_end_step(&mut g);
    cite!(["613.1f", "702.185a"]; assert!(!g.battlefield.contains(&up) && in_exile(&g, BOLTBENDER).is_some()));
}

// Rules: 613.1f, 702.188a, 702.185a
// Given: Tannuk; a red creature card printing a non-warp alternative cost, "Web-slinging {R}"
// (pay {R} and return a tapped creature you control to its owner's hand), in hand; a tapped
// creature of its caster's.
// Expect: both the printed alternative and the given warp are offered as separate rows; paid
// for the printed one it is not exiled and Full Bore gives no trample; for the given warp it is.
// Why: the given warp is a row numbered past the printed ones; a printed non-warp alternative
// must not be read as the given warp, nor the reverse.
#[test]
fn r613_1f_a_given_warp_beside_a_printed_alternative_cost() {
    let mut g = game();
    g.put(0, TANNUK, Zone::Battlefield);
    let tapped = g.put(0, GIANT, Zone::Battlefield);
    g.tap(tapped);
    let card = g.put(0, SLINGER, Zone::Hand);
    mana(&mut g, 0, 1, 2);
    // Printed Web-slinging and the granted Warp retain separate ability rows.
    let (printed, given) = (0, 1);
    mana(&mut g, 0, 1, 0);
    g.act(0, crate::support::paid("R",Move::CastFor(CastCost::Alternative(printed), Box::new(Move::WithCosts(vec![vec![tapped]], Box::new(Move::Cast { card, targets: vec![] })))))).unwrap();
    cite!(["702.188a"]; assert_eq!(g.obj(tapped).zone, Zone::Gone, "returned to its owner's hand"));
    settle(&mut g);
    let slinger = *g.battlefield.last().unwrap();
    assert_eq!(g.obj(slinger).card, Some(SLINGER));
    cite!(["601.2b"]; assert!(!full_bore_tramples(&mut g, slinger)));
    through_end_step(&mut g);
    cite!(["702.185a"]; assert!(g.battlefield.contains(&slinger), "not cast for its warp cost"));
    let mut g = game();
    g.put(0, TANNUK, Zone::Battlefield);
    let tapped = g.put(0, GIANT, Zone::Battlefield);
    g.tap(tapped);
    let card = g.put(0, SLINGER, Zone::Hand);
    mana(&mut g, 0, 1, 2);
    g.act(0, crate::support::paid("CCR",alt(card, given))).unwrap();
    cite!(["702.185a"]; assert_eq!(g.players[0].pool.total(), 0, "{{2}}{{R}}"));
    assert!(g.battlefield.contains(&tapped), "no creature returned for a warp cost");
    settle(&mut g);
    let slinger = *g.battlefield.last().unwrap();
    cite!(["601.2b", "613.1f"]; assert!(full_bore_tramples(&mut g, slinger)));
    through_end_step(&mut g);
    cite!(["702.185a"]; assert!(!g.battlefield.contains(&slinger) && in_exile(&g, SLINGER).is_some()));
}

// Rules: 603.7c, 702.185a, 613.1f
// Given: a Giant cast for the warp Tannuk gives; once it has resolved, Tannuk is destroyed.
// Expect: still exiled at the next end step: the delayed trigger exists once the spell has
// resolved and does not depend on the source of the given warp. An artifact card cast for it
// (a noncreature permanent) is exiled the same way.
#[test]
fn r603_7c_a_given_warp_still_exiles_after_its_source_leaves() {
    let mut g = game();
    let tannuk = g.put(0, TANNUK, Zone::Battlefield);
    let giant = g.put(0, GIANT, Zone::Hand);
    let relic = g.put(0, RELIC, Zone::Hand);
    mana(&mut g, 0, 1, 2);
    let row = 0; // The blank Giant receives its first ability.
    g.act(0, crate::support::paid("CCR",alt(giant, row))).unwrap();
    let giant_spell = *g.stack.last().unwrap();
    settle(&mut g);
    mana(&mut g, 0, 1, 2);
    let row = 0; // The blank Relic receives its first ability.
    g.act(0, crate::support::paid("CCR",alt(relic, row))).unwrap();
    let relic_spell = *g.stack.last().unwrap();
    cite!(["702.185a"]; assert_eq!(g.players[0].pool.total(), 0, "{{2}}{{R}} rather than {{4}}"));
    settle(&mut g);
    let permanents = (on_battlefield(&g, GIANT)[0], on_battlefield(&g, RELIC)[0]);
    assert!(g.destroy(tannuk));
    for _ in 0..80 {
        if matches!(g.decision, Some(Decision::Order { .. })) { break; }
        step(&mut g, None);
    }
    let Some(Decision::Order { player: 0, abilities }) = &g.decision else { panic!("two declared warp triggers: {:?}", g.decision) };
    assert_eq!(abilities.len(), 2);
    assert_eq!(abilities.iter().filter_map(|a| a.source).collect::<std::collections::BTreeSet<_>>(), [giant_spell, relic_spell].into_iter().collect());
    g.act(0, Move::Order(vec![0, 1])).unwrap();
    through_end_step(&mut g);
    cite!(["603.7c", "702.185a"]; assert!(!g.battlefield.contains(&permanents.0), "the Giant"));
    cite!(["603.7c", "702.185a"]; assert!(!g.battlefield.contains(&permanents.1), "the Relic, an artifact"));
    assert!(in_exile(&g, GIANT).is_some() && in_exile(&g, RELIC).is_some());
}

// Rules: 702.185a, 702.185b, 400.7
// Given: Warper cast for its warp cost, then exiled by something other than the warp trigger
// before the end step.
// Expect: its owner can't cast it from exile on a later turn: only the delayed triggered
// ability's exile gives that permission (702.185a; EOE ruling: "If it goes to exile some other
// way, its owner won't be able to cast it on a future turn").
#[test]
fn r702_185b_a_warped_permanent_exiled_some_other_way_is_not_castable() {
    let mut g = game();
    let card = g.put(0, WARPER, Zone::Hand);
    mana(&mut g, 1, 0, 0);
    let row = 0; // Warper prints Warp as its first ability.
    g.act(0, crate::support::paid("G",alt(card, row))).unwrap();
    settle(&mut g);
    let beast = on_battlefield(&g, WARPER)[0];
    g.move_object(beast, Zone::Exile);
    let exiled = in_exile(&g, WARPER).unwrap();
    through_end_step(&mut g);
    next_own_main(&mut g);
    cite!(["702.185b"]; assert_eq!(g.warped(exiled), None));
    mana(&mut g, 1, 0, 3);
    cite!(["702.185a"]; refused("that card is not in your hand: it is in exile"): unchanged(&mut g, |g| g.act(0, crate::support::paid("CCCG", Move::Cast { card: exiled, targets: vec![] }))));
}

#[track_caller]
fn unchanged(g: &mut Game, action: impl FnOnce(&mut Game) -> Result<(), String>) -> Result<(), String> {
    let before=format!("{g:?}");
    let result=action(g);
    assert_eq!(format!("{g:?}"),before);
    result
}
