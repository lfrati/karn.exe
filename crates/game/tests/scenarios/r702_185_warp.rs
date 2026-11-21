//! Warp (702.185a): "You may cast this card from your hand by paying [cost] rather than its
//! mana cost" and "If this spell's warp cost was paid, exile the permanent this spell becomes
//! at the beginning of the next end step. Its owner may cast this card after the current turn
//! has ended for as long as it remains exiled." The exile is a delayed trigger the resolving
//! spell creates, not an enters trigger; the later cast uses normal timing and normal costs
//! (EOE release notes).
//!
//! 613.1f: an effect that gives cards in a hand warp ("Artifact cards and red creature cards
//! in your hand have warp {2}{R}") gives them that alternative cost, cast for as a printed one.
use crate::support::cite;
use game::announcement::CastCost;
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const WARPER: usize = 0;
const FLASHER: usize = 1;
const GRAVE: usize = 2;
const HELLKITE: usize = 3;
const BEAR: usize = 4;
const FOREST: usize = 5;
const TANNUK: usize = 6;
const GIANT: usize = 7;
const RED_WARPER: usize = 8;
const RELIC: usize = 9;
const BORE: usize = 10;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap();
    let c = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    let defs = vec![
        c("Warper", "{3}{G}", "Creature — Beast", "Warp {G}", Some((2, 2))),
        c("Flasher", "{3}{G}", "Creature — Beast", "Flash\nWarp {G}", Some((2, 2))),
        c("Grave Warper", "{3}{G}", "Creature — Beast", "You may cast this card from your graveyard.\nWarp {G}", Some((2, 2))),
        real("Nova Hellkite"),
        c("Bear", "{0}", "Creature — Bear", "", Some((2, 2))),
        c("Forest", "", "Basic Land — Forest", "", None),
        real("Tannuk, Steadfast Second"),
        c("Giant", "{3}{R}", "Creature — Giant", "", Some((3, 3))),
        c("Red Warper", "{3}{R}", "Creature — Beast", "Warp {R}", Some((2, 2))),
        c("Relic", "{4}", "Artifact", "", None),
        real("Full Bore"),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, FOREST, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
/// The fixture's printed Warp row, independent of payment or timing.
fn warp_row(g: &Game, card: ObjId) -> usize {
    g.def(card).unwrap().abilities().iter().position(|a| matches!(a,
        ir::Ability::Keywords(items) if items.iter().any(|i| i.keyword.name() == "Warp")))
        .expect("the fixture's printed Warp row")
}

fn warp(card: ObjId, row: usize, mana: &str) -> Move {
    crate::support::paid(mana, Move::CastFor(CastCost::Alternative(row), Box::new(Move::Cast { card, targets: vec![] })))
}
fn green(g: &mut Game, n: u32) { g.players[0].pool.colors[4] = n; }
/// One step of the game: no attacks or blocks; triggered abilities target `target`.
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
    for _ in 0..20 {
        if g.decision.is_none() && g.stack.is_empty() { return; }
        step(g, None);
    }
    panic!("the stack did not settle");
}
/// Until the beginning of the end step of this turn, or of the next turn if it is under way,
/// with priority given and any trigger of its beginning on the stack.
fn end_step(g: &mut Game) {
    while g.part == Part::End { step(g, None); }
    for _ in 0..200 {
        if g.part == Part::End && g.decision.is_none() { return; }
        step(g, None);
    }
    panic!("no end step");
}
/// Until player 0's next precombat main phase.
fn next_own_main(g: &mut Game) {
    let turn = g.turn;
    for _ in 0..400 {
        if g.turn != turn && g.active == 0 && g.part == Part::PrecombatMain && g.decision.is_none() && g.stack.is_empty() { return; }
        step(g, None);
    }
    panic!("no next main phase");
}
fn in_exile(g: &Game, def: usize) -> Option<ObjId> { g.exile.iter().copied().find(|o| g.obj(*o).card == Some(def)) }

#[test]
fn r702_185a_the_warped_permanent_is_exiled_at_the_next_end_step() {
    let mut g = game();
    let card = g.put(0, WARPER, Zone::Hand);
    green(&mut g, 1);
    let row = warp_row(&g, card);
    g.act(0, warp(card, row, "G")).unwrap();
    cite!(["702.185a"]; assert_eq!(g.players[0].pool.total(), 0, "{{G}} rather than {{3}}{{G}}"));
    settle(&mut g);
    let beast = *g.battlefield.last().unwrap();
    assert_eq!(g.obj(beast).card, Some(WARPER));
    assert!(g.stack.is_empty(), "no enters trigger: the exile waits for the end step");
    let turn = g.turn;
    end_step(&mut g);
    cite!(["702.185a"]; assert_eq!(g.stack.len(), 1, "the delayed trigger, as the end step begins"));
    settle(&mut g);
    cite!(["702.185a"]; assert!(!g.battlefield.contains(&beast)));
    let exiled = in_exile(&g, WARPER).expect("exiled");
    cite!(["702.185b"]; assert_eq!(g.warped(exiled), Some(turn), "a warped card (702.185b)"));
}

#[test]
fn r702_185a_its_owner_casts_it_from_exile_after_this_turn_for_its_normal_cost_and_timing() {
    let mut g = game();
    let card = g.put(0, WARPER, Zone::Hand);
    green(&mut g, 1);
    let row = warp_row(&g, card);
    g.act(0, warp(card, row, "G")).unwrap();
    settle(&mut g);
    end_step(&mut g);
    settle(&mut g);
    let exiled = in_exile(&g, WARPER).unwrap();
    // not during the turn it was exiled
    green(&mut g, 4);
    cite!(["702.185a"]; refused("the effect that lets you cast that card doesn't yet (601.3)"): g.act(0, Move::Cast { card: exiled, targets: vec![] }));
    // nor on the next turn, an opponent's, without flash
    while g.active == 0 { step(&mut g, None); }
    while g.priority != 0 { step(&mut g, None); }
    green(&mut g, 4);
    cite!(["117.1a"]; refused("that is cast in a main phase of your own turn with the stack empty (117.1a)"): g.act(0, Move::Cast { card: exiled, targets: vec![] }));
    // in its owner's main phase: for its mana cost, never its warp cost (from a hand only)
    next_own_main(&mut g);
    assert!(g.warped(exiled).is_some_and(|t| t < g.turn), "still a warped card, from an earlier turn");
    green(&mut g, 1);
    cite!(["702.185a"]; refused("that alternative cost is paid only for a spell cast from a hand (702.185a)"): g.act(0, warp(exiled, row, "G")));
    cite!(["118.3a"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0, crate::support::paid("G",Move::Cast { card: exiled, targets: vec![] })));
    green(&mut g, 4);
    cite!(["702.185a"]; ok: g.act(0, crate::support::paid("GGGG", Move::Cast { card: exiled, targets: vec![] })));
    assert_eq!(g.players[0].pool.total(), 0);
    settle(&mut g);
    let beast = *g.battlefield.last().unwrap();
    assert_eq!(g.obj(beast).card, Some(WARPER));
    // cast for its mana cost, it stays
    end_step(&mut g);
    settle(&mut g);
    cite!(["702.185a"]; assert!(g.battlefield.contains(&beast)));
}

#[test]
fn r702_185a_cast_for_its_mana_cost_it_is_not_exiled() {
    let mut g = game();
    let card = g.put(0, WARPER, Zone::Hand);
    green(&mut g, 4);
    g.act(0, crate::support::paid("GGGG", Move::Cast { card, targets: vec![] })).unwrap();
    settle(&mut g);
    let beast = *g.battlefield.last().unwrap();
    end_step(&mut g);
    cite!(["702.185a"]; assert!(g.stack.is_empty()));
    settle(&mut g);
    assert!(g.battlefield.contains(&beast));
    cite!(["702.185a"]; assert!(g.exile.is_empty()));
}

#[test]
fn r702_185a_a_warp_cost_is_paid_only_for_a_spell_cast_from_a_hand() {
    let mut g = game();
    let card = g.put(0, GRAVE, Zone::Graveyard);
    green(&mut g, 4);
    let row = warp_row(&g, card);
    let before = format!("{g:?}");
    cite!(["702.185a"]; refused("that alternative cost is paid only for a spell cast from a hand (702.185a)"): g.act(0, warp(card, row, "G")));
    assert_eq!(format!("{g:?}"), before);
    // its own permission still lets it be cast from there for its mana cost
    g.act(0, crate::support::paid("GGGG", Move::Cast { card, targets: vec![] })).unwrap();
    assert_eq!(g.players[0].pool.total(), 0);
}

#[test]
fn r400_7_a_permanent_that_left_and_returned_is_a_new_object_and_stays() {
    let mut g = game();
    let card = g.put(0, WARPER, Zone::Hand);
    green(&mut g, 1);
    let row = warp_row(&g, card);
    g.act(0, warp(card, row, "G")).unwrap();
    settle(&mut g);
    let beast = *g.battlefield.last().unwrap();
    // flickered: exiled and returned, a new object the delayed trigger does not name
    g.move_object(beast, Zone::Exile);
    let gone = *g.exile.last().unwrap();
    g.move_object(gone, Zone::Battlefield);
    let back = *g.battlefield.last().unwrap();
    end_step(&mut g);
    settle(&mut g);
    cite!(["603.7c", "400.7"]; assert!(g.battlefield.contains(&back), "only the permanent the spell became is exiled"));
    assert!(g.exile.is_empty());
    cite!(["702.185b"]; assert_eq!(g.warped(back), None));
    // returned to the hand, it stays there, and is not a warped card
    let mut g = game();
    let card = g.put(0, WARPER, Zone::Hand);
    green(&mut g, 1);
    g.act(0, warp(card, row, "G")).unwrap();
    settle(&mut g);
    let beast = *g.battlefield.last().unwrap();
    g.move_object(beast, Zone::Hand);
    end_step(&mut g);
    settle(&mut g);
    cite!(["400.7"]; assert!(g.exile.is_empty()));
    assert_eq!(g.players[0].hand.iter().filter(|o| g.obj(**o).card == Some(WARPER)).count(), 1);
}

#[test]
fn r603_7_cast_during_an_end_step_it_is_exiled_at_the_next_one() {
    let mut g = game();
    let card = g.put(0, FLASHER, Zone::Hand);
    end_step(&mut g);
    green(&mut g, 1);
    let row = warp_row(&g, card);
    g.act(0, warp(card, row, "G")).unwrap();
    settle(&mut g);
    let beast = *g.battlefield.last().unwrap();
    let turn = g.turn;
    // this end step began before the trigger existed: the next one is the opponent's turn's
    end_step(&mut g);
    assert_eq!(g.turn, turn + 1);
    cite!(["603.7a"]; assert!(g.battlefield.contains(&beast), "not yet"));
    cite!(["603.7b"]; assert_eq!(g.stack.len(), 1));
    settle(&mut g);
    let exiled = in_exile(&g, FLASHER).expect("exiled at the opponent's end step");
    // after that turn has ended, its owner may cast it again: with flash, at once
    cite!(["702.185b"]; assert_eq!(g.warped(exiled), Some(turn + 1)));
    // its owner with priority in that same turn: not during the turn it was exiled
    while g.priority != 0 || g.decision.is_some() { step(&mut g, None); }
    assert_eq!(g.turn, turn + 1);
    green(&mut g, 4);
    cite!(["702.185a"]; refused("the effect that lets you cast that card doesn't yet (601.3)"): g.act(0, Move::Cast { card: exiled, targets: vec![] }));
    while g.turn == turn + 1 { step(&mut g, None); }
    while g.priority != 0 || g.decision.is_some() { step(&mut g, None); }
    green(&mut g, 4);
    cite!(["702.185a"]; ok: g.act(0, crate::support::paid("GGGG", Move::Cast { card: exiled, targets: vec![] })));
}

#[test]
fn r702_185a_nova_hellkite_for_its_warp_cost() {
    let mut g = game();
    let card = g.put(0, HELLKITE, Zone::Hand);
    let bear = g.put(1, BEAR, Zone::Battlefield);
    g.players[0].pool.colors[3] = 3;
    let row = warp_row(&g, card);
    g.act(0, warp(card, row, "RRR")).unwrap();
    cite!(["702.185a"]; assert_eq!(g.players[0].pool.total(), 0));
    // its enters trigger, which deals 1 damage to a creature an opponent controls
    for _ in 0..20 {
        if g.decision.is_none() && g.stack.is_empty() { break; }
        step(&mut g, Some(bear));
    }
    assert_eq!(g.obj(bear).damage, 1);
    let dragon = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(HELLKITE)).unwrap();
    end_step(&mut g);
    settle(&mut g);
    assert!(!g.battlefield.contains(&dragon));
    cite!(["702.185a"]; assert!(in_exile(&g, HELLKITE).is_some()));
}

/// The rows the card is offered for alternative costs, in order.

fn red_and_generic(g: &mut Game, red: u32, generic: u32) { g.players[0].pool.colors[3] = red; g.players[0].pool.colorless = generic; }

/// Warp is two static abilities, one of which may create a delayed triggered ability
/// (702.185a); a warp an effect gave the card (Tannuk: "Artifact cards and red creature cards
/// in your hand have warp {2}{R}", 613.1f) is the one that created it, as a printed one is
/// (603.7a). On the stack the delayed ability has its own text (603.3, 405.4), and the prompt
/// says "Warp {2}{R}" created it, though no definition prints it and Tannuk has since left
/// (113.7a, 603.7c).
#[test]
fn r702_185a_a_given_warps_delayed_ability_says_the_warp_that_created_it() {
    let mut g = game();
    let tannuk = g.put(0, TANNUK, Zone::Battlefield);
    let giant = g.put(0, GIANT, Zone::Hand);
    red_and_generic(&mut g, 1, 2);
    // These blank printed fixtures have their first granted ability at row 0.
    g.act(0, warp(giant, 0, "CCR")).unwrap();
    settle(&mut g);
    assert!(g.destroy(tannuk));
    settle(&mut g);
    end_step(&mut g);
    let exile = *g.stack.last().expect("the warp's delayed ability");
    let text = "At the beginning of the next end step, exile the permanent this spell becomes. Its owner may cast this card after the current turn has ended for as long as it remains exiled.";
    for seat in 0..g.players.len() {
        let p = game::simulation::Simulation::new(g.clone()).prompt(seat).unwrap();
        let occurrence = p.entities[&exile].occurrence.as_ref().unwrap();
        cite!(["603.3", "702.185a"]; assert_eq!(occurrence.text.as_deref(), Some(text), "p{seat}"));
        cite!(["603.7a", "702.185a", "613.1f"]; assert_eq!(occurrence.created_by.as_deref(), Some("Warp {2}{R}"), "p{seat}"));
    }
}

#[test]
fn r613_1f_tannuk_gives_artifact_and_red_creature_cards_in_a_hand_warp() {
    // Tannuk, Steadfast Second: "Artifact cards and red creature cards in your hand have warp {2}{R}."
    let mut g = game();
    let giant = g.put(0, GIANT, Zone::Hand);
    let relic = g.put(0, RELIC, Zone::Hand);
    let bear = g.put(0, BEAR, Zone::Hand);
    red_and_generic(&mut g, 1, 2);
    cite!(["702.185a"]; refused("that card prints no such alternative cost"): unchanged(&mut g, |g| g.act(0, warp(giant, 0, "CCR"))));
    let tannuk = g.put(0, TANNUK, Zone::Battlefield);
    // These blank printed fixtures have their first granted ability at row 0.
    let given = 0;
    cite!(["613.1f", "702.185a"]; ok: g.clone().act(0, warp(relic, given, "CCR")));
    cite!(["613.1f"]; refused("that card prints no such alternative cost"): unchanged(&mut g, |g| g.act(0, warp(bear, given, "CCR"))));
    // in a graveyard it has no warp: the effect gives it to cards in a hand
    let buried = g.put(0, GIANT, Zone::Graveyard);
    cite!(["613.1f", "702.185a"]; refused("that card is not in your hand: it is in a graveyard"): unchanged(&mut g, |g| g.act(0, warp(buried, given, "CCR"))));
    g.act(0, warp(giant, given, "CCR")).unwrap();
    cite!(["702.185a"]; assert_eq!(g.players[0].pool.total(), 0, "{{2}}{{R}} rather than {{3}}{{R}}"));
    settle(&mut g);
    let permanent = *g.battlefield.last().unwrap();
    assert_eq!(g.obj(permanent).card, Some(GIANT));
    let turn = g.turn;
    end_step(&mut g);
    settle(&mut g);
    // its warp cost was paid, given or not: exiled at the next end step, a warped card
    cite!(["702.185a"]; assert!(!g.battlefield.contains(&permanent)));
    let exiled = in_exile(&g, GIANT).expect("exiled");
    cite!(["702.185b"]; assert_eq!(g.warped(exiled), Some(turn)));
    // the exiled card is cast later for its mana cost, Tannuk's warp being of hand cards
    next_own_main(&mut g);
    red_and_generic(&mut g, 1, 3);
    cite!(["702.185a"]; refused("that card prints no such alternative cost"): unchanged(&mut g, |g| g.act(0, warp(exiled, given, "CCR"))));
    g.act(0, crate::support::paid("CCCR", Move::Cast { card: exiled, targets: vec![] })).unwrap();
    cite!(["702.185a"]; assert_eq!(g.players[0].pool.total(), 0, "{{3}}{{R}}"));
    settle(&mut g);
    // Tannuk gone, cards in a hand have no warp
    let card = g.put(0, GIANT, Zone::Hand);
    g.destroy(tannuk);
    red_and_generic(&mut g, 1, 2);
    cite!(["613.1f"]; refused("that card prints no such alternative cost"): unchanged(&mut g, |g| g.act(0, warp(card, given, "CCR"))));
}

#[test]
fn r613_1f_a_given_warp_beside_a_printed_one_and_a_cast_for_the_mana_cost() {
    let mut g = game();
    g.put(0, TANNUK, Zone::Battlefield);
    // Red Warper: "Warp {R}", and warp {2}{R} given: two warp costs, each its own row
    let card = g.put(0, RED_WARPER, Zone::Hand);
    red_and_generic(&mut g, 1, 2);
    // Row 0 is printed Warp {R}; the grant follows it at row 1.
    cite!(["613.1f", "702.185a"]; ok: g.clone().act(0, warp(card, 0, "R")));
    let given = 1;
    g.act(0, warp(card, given, "CCR")).unwrap();
    cite!(["702.185a"]; assert_eq!(g.players[0].pool.total(), 0, "{{2}}{{R}}"));
    settle(&mut g);
    end_step(&mut g);
    settle(&mut g);
    cite!(["702.185a"]; assert!(in_exile(&g, RED_WARPER).is_some()));
    // cast for its mana cost, a card Tannuk gives warp is not exiled
    let mut g = game();
    g.put(0, TANNUK, Zone::Battlefield);
    let giant = g.put(0, GIANT, Zone::Hand);
    red_and_generic(&mut g, 1, 3);
    g.act(0, crate::support::paid("CCCR", Move::Cast { card: giant, targets: vec![] })).unwrap();
    settle(&mut g);
    end_step(&mut g);
    settle(&mut g);
    cite!(["702.185a"]; assert!(g.battlefield.iter().any(|o| g.obj(*o).card == Some(GIANT))));
}

/// Full Bore on this creature, cast by player 0 with {R} and resolved: its power and
/// toughness, and whether it has trample and haste.
fn full_bore(g: &mut Game, creature: ObjId) -> ((i32, i32), bool, bool) {
    let bore = g.put(0, BORE, Zone::Hand);
    g.players[0].pool.colors[3] = 1;
    g.act(0, crate::support::paid("R", Move::Cast { card: bore, targets: vec![vec![Target::Object(creature)]] })).unwrap();
    settle(g);
    let has = |g: &Game, name: &str| g.seen(creature, None).has(ir::Keyword::from_name(name).unwrap());
    ((g.power(creature).unwrap(), g.toughness(creature).unwrap()), has(g, "Trample"), has(g, "Haste"))
}

#[test]
fn r601_2b_full_bore_asks_whether_that_creature_was_cast_for_its_warp_cost() {
    // Full Bore: "Target creature you control gets +3/+2 until end of turn. If that creature
    // was cast for its warp cost, it also gains trample and haste until end of turn."
    // cast for its warp cost: trample and haste
    let mut g = game();
    let card = g.put(0, WARPER, Zone::Hand);
    green(&mut g, 1);
    let row = warp_row(&g, card);
    g.act(0, warp(card, row, "G")).unwrap();
    settle(&mut g);
    let beast = *g.battlefield.last().unwrap();
    cite!(["601.2b", "702.185a"]; assert_eq!(full_bore(&mut g, beast), ((5, 4), true, true)));
    // for a warp cost Tannuk gave it
    let mut g = game();
    g.put(0, TANNUK, Zone::Battlefield);
    let giant = g.put(0, GIANT, Zone::Hand);
    red_and_generic(&mut g, 1, 2);
    let row = 0; // The blank Giant receives its first ability from Tannuk.
    g.act(0, warp(giant, row, "CCR")).unwrap();
    settle(&mut g);
    let giant = *g.battlefield.last().unwrap();
    // (haste Tannuk gives as well: trample says it)
    cite!(["601.2b", "613.1f"]; assert_eq!(full_bore(&mut g, giant), ((6, 5), true, true)));
    // cast for its mana cost, or never cast: only +3/+2
    let mut g = game();
    let card = g.put(0, WARPER, Zone::Hand);
    green(&mut g, 1);
    g.players[0].pool.colorless = 3;
    g.act(0, crate::support::paid("CCCG", Move::Cast { card, targets: vec![] })).unwrap();
    settle(&mut g);
    let cast = *g.battlefield.last().unwrap();
    let put = g.put(0, BEAR, Zone::Battlefield);
    cite!(["601.2b"]; assert_eq!(full_bore(&mut g, cast), ((5, 4), false, false)));
    cite!(["601.2b"]; assert_eq!(full_bore(&mut g, put), ((5, 4), false, false)));
}

#[test]
fn r702_37c_a_face_down_spell_cast_for_its_morph_cost_is_not_warped() {
    // Exalted Angel: "Flying", "Whenever this creature deals damage, you gain that much life.",
    // "Morph {2}{W}{W}": the face-down spell's definition prints no abilities, and its
    // alternative cost is the morph row of its card, not a warp cost an effect gave it
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let angel = CardDef::from_face(faces.iter().find(|f| f.name == "Exalted Angel").unwrap()).unwrap();
    let mut g = Game::new(vec![angel, CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap()], 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, 1, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colorless = 3;
    g.act(0, crate::support::paid("CCC", Move::CastFor(CastCost::Alternative(2), Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    settle(&mut g);
    let permanent = *g.battlefield.last().unwrap();
    end_step(&mut g);
    cite!(["702.37c", "702.185a"]; assert!(g.stack.is_empty(), "no warp exile"));
    settle(&mut g);
    cite!(["702.37c", "702.185a"]; assert!(g.battlefield.contains(&permanent) && g.exile.is_empty()));
}

#[track_caller]
fn unchanged(g: &mut Game, action: impl FnOnce(&mut Game) -> Result<(), String>) -> Result<(), String> {
    let before=format!("{g:?}");
    let result=action(g);
    assert_eq!(format!("{g:?}"),before);
    result
}
