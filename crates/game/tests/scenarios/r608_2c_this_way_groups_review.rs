//! "This way" result groups (608.2c): what an earlier instruction did, as a later
//! instruction of the same resolution, or a delayed trigger it created (603.7), names it.
//! Expectations come from the rules and the cards' rulings: a member that changed zones is a
//! new object (400.7, 603.7c); a replaced event still happened when the replacement only
//! changes where the card goes (701.8, 701.17c, 701.9c; Fumigate's and madness's rulings);
//! a regenerated creature was not destroyed (701.8c, 701.19a); a reference names every
//! producer its words name, whether or not each did something (608.2c, 609.3).
use crate::support::{cite, paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const LAND: usize = 0;
const BEAR: usize = 1;
const PEACE: usize = 2;
const FUMIGATE: usize = 3;
const HIDE: usize = 4;
const FANATIC: usize = 5;
const TEMPER: usize = 6;
const MILL_PICK: usize = 7;
const MILL_COUNT: usize = 8;
const MILL_IF: usize = 9;
const EXILE_THEN_BURY: usize = 10;
const TWICE: usize = 11;
const NOTHING_FIRST: usize = 12;
const NOTHING_COUNT: usize = 13;
const RELIC: usize = 14;

fn defs() -> Vec<CardDef> {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let real = |name: &str| CardDef::from_faces(catalog.get(name).unwrap()).unwrap();
    let c = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).expect(name);
    vec![
        c("Plain Land", "Land", "", None),
        c("Bear", "Creature — Bear", "", Some((2, 2))),
        real("Rest in Peace"),
        real("Fumigate"),
        real("Hide on the Ceiling"),
        real("Fanatic of the Harrowing"),
        real("Fiery Temper"),
        c("Mill Pick", "Sorcery", "Mill three cards. Put a land card from among the milled cards into your hand.", None),
        c("Mill Count", "Sorcery", "Mill three cards. You gain 2 life for each land card milled this way.", None),
        c("Mill If", "Sorcery", "Mill three cards. If a land card was milled this way, draw a card.", None),
        c("Exile Then Bury", "Sorcery", "Exile the top three cards of your library. Put all land cards exiled this way into your graveyard. You may put a land card from among the exiled cards into your hand.", None),
        c("Twice", "Sorcery", "Exile the top two cards of your library. Exile the top two cards of your library. Put all creature cards exiled this way into your hand.", None),
        c("Nothing First", "Sorcery", "Exile all artifacts. Exile the top card of your library. You may put a land card from among the exiled cards into your hand.", None),
        c("Nothing Count", "Sorcery", "Exile all artifacts. Exile the top card of your library. You gain 2 life for each card exiled this way.", None),
        c("Relic", "Artifact", "", None),
    ]
}

fn game() -> Game {
    let defs = defs();
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}

/// Put these cards into a library, the last one on top.
fn library(g: &mut Game, player: usize, cards: &[usize]) {
    for d in cards { g.put(player, *d, Zone::Library); }
}

fn mana(g: &mut Game) {
    g.players[0].pool.colorless = 6;
    for c in 0..5 { g.players[0].pool.colors[c] = 3; }
}

fn cast(g: &mut Game, def: usize, targets: Vec<Target>, spending: &str) -> ObjId {
    let card = g.put(0, def, Zone::Hand);
    mana(g);
    g.act(0, paid(spending, Move::Cast { card, targets: if targets.is_empty() { vec![] } else { vec![targets] } })).unwrap();
    card
}

fn pass(g: &mut Game) {
    for _ in 0..40 {
        if g.decision.is_some() || g.stack.is_empty() { return; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("did not settle");
}

fn offered(g: &Game) -> Option<Vec<ObjId>> {
    match &g.decision {
        Some(Decision::Choose { choice: Choice::Objects { among, .. }, .. }) => Some(among.clone()),
        _ => None,
    }
}

fn choose(g: &mut Game, answer: Answer) {
    let Some(Decision::Choose { player, .. }) = g.decision else { panic!("no choice asked: {:?}", g.decision) };
    g.act(player, Move::Choose(answer)).unwrap();
}

/// Answer "may" with yes and take the first object of every object choice, until the stack
/// is empty and nothing is asked; returns every object offered on the way.
fn settle(g: &mut Game) -> Vec<ObjId> {
    let mut seen = Vec::new();
    for _ in 0..20 {
        pass(g);
        match &g.decision {
            None => return seen,
            Some(Decision::Choose { choice: Choice::May, .. }) => choose(g, Answer::Yes),
            Some(Decision::Choose { choice: Choice::Objects { among, .. }, .. }) => {
                seen.extend(among.iter().copied());
                let first = among[0];
                choose(g, Answer::Objects(vec![first]));
            }
            other => panic!("unexpected decision {other:?}"),
        }
    }
    panic!("did not settle")
}

fn count(g: &Game, list: &[ObjId], def: usize) -> usize { list.iter().filter(|o| g.obj(**o).card == Some(def)).count() }

// Probe 1: a member that changed zones between the producer and the reader is a new object.

#[test]
fn review_cards_exiled_then_moved_on_are_no_longer_among_the_exiled_cards() {
    let mut g = game();
    library(&mut g, 0, &[LAND, BEAR, LAND]);
    cast(&mut g, EXILE_THEN_BURY, vec![], "");
    // The two lands have entered exile as new objects before the bury instruction.
    pass(&mut g);
    let lands: Vec<_> = g.exile.iter().copied().filter(|o|g.obj(*o).card==Some(LAND)).collect();
    assert_eq!(lands.len(),2);
    assert_eq!(g.decision,Some(Decision::Choose{player:0,choice:Choice::OrderCards{to:game::state::Placement::Zone(Zone::Graveyard),cards:lands.clone()}}));
    g.act(0,Move::Choose(Answer::Objects(vec![lands[1],lands[0]]))).unwrap();
    let seen = settle(&mut g);
    // the two lands left exile for the graveyard: new objects, not "the exiled cards" (400.7)
    cite!(["400.7", "608.2c"]; assert!(seen.iter().all(|o| g.obj(*o).card != Some(LAND) || g.obj(*o).zone == Zone::Exile), "no land offered from outside exile"));
    cite!(["400.7", "608.2c"]; assert_eq!(g.players[0].hand.len(), 0));
    assert_eq!(count(&g, &g.players[0].graveyard, LAND), 2);
    assert_eq!(count(&g, &g.exile, BEAR), 1);
}

#[test]
fn review_controller_change_between_producer_and_reader_is_not_expressible() {
    // The same object that changed controller stays in its group (400.7 names only zone
    // changes); no text that moves a group, changes its controller, then reads it is run yet.
    for text in ["Return target creature card from your graveyard to the battlefield. Gain control of it until end of turn. If a creature was returned this way, you gain 3 life.",
        "Return all creature cards from your graveyard to the battlefield. Target opponent gains control of all creatures you control. You gain 1 life for each creature returned this way."] {
        let def = CardDef::new("Limit", "{0}", "Sorcery", text, None).unwrap();
        // Uncited (admission): distinct unsupported control paths, not a zone-identity rule.
        let reason=if text.starts_with("Return target") {"gain control: only permanents, not spell occurrences"}
            else {"gain control: a different player or a repeated quantity"};
        crate::support::refused(game::play::check(&def).map_err(|e|e.0),reason);
    }
}

#[test]
fn r603_7c_leading_step_delay_keeps_the_returned_group_through_control_but_not_zone_changes() {
    let make=|name,types,text,pt|CardDef::new(name,"{0}",types,text,pt).unwrap();
    let definitions=vec![
        make("Bear","Creature — Bear","",Some((2,2))),
        make("Temporary return","Sorcery","Return up to two target creature cards from your graveyard to the battlefield. At the beginning of the next end step, exile the creatures returned this way.",None),
        make("Steal","Instant","You gain control of target creature.",None),
        make("Blink","Instant","Exile target creature. Return that card to the battlefield under its owner's control.",None),
    ];
    for definition in &definitions {game::play::check(definition).unwrap();}
    let mut g=Game::new(definitions,2,20);
    let unrelated=g.put(0,0,Zone::Battlefield);
    let one=g.put(0,0,Zone::Graveyard);let two=g.put(0,0,Zone::Graveyard);
    let spell=g.put(0,1,Zone::Hand);
    g.act(0,Move::Cast{card:spell,targets:vec![vec![Target::Object(one),Target::Object(two)]]}).unwrap();
    pass(&mut g);
    let returned:Vec<_>=g.battlefield.iter().copied().filter(|object|*object!=unrelated).collect();
    assert_eq!(returned.len(),2);
    let physical=g.physical_card_identity(returned[0]);
    let blink=g.put(0,3,Zone::Hand);
    g.act(0,Move::Cast{card:blink,targets:vec![vec![Target::Object(returned[0])]]}).unwrap();
    pass(&mut g);
    let reentered=*g.battlefield.iter().find(|object|g.physical_card_identity(**object)==physical).unwrap();
    cite!(["400.7"]; assert_ne!(reentered,returned[0]));
    g.act(0,Move::Pass).unwrap();
    let steal=g.put(1,2,Zone::Hand);
    g.act(1,Move::Cast{card:steal,targets:vec![vec![Target::Object(returned[1])]]}).unwrap();
    pass(&mut g);
    cite!(["400.7","611.2c"]; assert_eq!(g.controller(returned[1]),1));
    for _ in 0..80 {
        if g.part==Part::End && !g.stack.is_empty() {break;}
        match g.decision.clone() {
            Some(Decision::Attackers{player})=>g.act(player,Move::Attack(vec![])).unwrap(),
            Some(Decision::Blockers{player})=>g.act(player,Move::Block(vec![])).unwrap(),
            None=>g.act(g.priority,Move::Pass).unwrap(),
            other=>panic!("unexpected decision {other:?}"),
        }
    }
    assert_eq!(g.part,Part::End);assert_eq!(g.stack.len(),1);
    pass(&mut g);
    cite!(["603.7c","608.2c"]; assert!(g.battlefield.contains(&unrelated)));
    cite!(["603.7c","400.7"]; assert!(g.battlefield.contains(&reentered)));
    assert_eq!(g.battlefield.len(),2);
    cite!(["603.7c","400.7"]; assert_eq!(g.exile.len(),1));
    assert_eq!(g.physical_card_identity(g.exile[0]),g.physical_card_identity(returned[1]));
}

// Probe 2: replaced producers.

#[test]
fn review_fumigate_counts_creatures_destroyed_into_exile_but_not_regenerated_ones() {
    for (bears, regenerated) in [(3, false), (2, true)] {
        let mut g = game();
        g.put(1, PEACE, Zone::Battlefield);
        for i in 0..bears { g.put(i % 2, BEAR, Zone::Battlefield); }
        if regenerated { let r = g.put(1, BEAR, Zone::Battlefield); g.regenerate(r); }
        cast(&mut g, FUMIGATE, vec![], "CCCWW");
        settle(&mut g);
        // destroyed, then exiled instead of put into a graveyard: still destroyed (701.8a, 614.1a)
        cite!(["608.2c", "701.8a", "614.1a"]; case: bears; assert_eq!(g.players[0].life, 20 + bears as i32));
        cite!(["701.8c", "701.19a"]; case: bears; assert_eq!(g.battlefield.iter().filter(|o| g.obj(**o).card == Some(BEAR)).count(), usize::from(regenerated)));
        assert_eq!(count(&g, &g.exile, BEAR), bears);
    }
}

#[test]
fn review_mill_replaced_to_exile_still_counts_the_milled_cards() {
    let mut g = game();
    g.put(1, PEACE, Zone::Battlefield);
    library(&mut g, 0, &[LAND, BEAR, LAND]);
    cast(&mut g, MILL_COUNT, vec![], "");
    settle(&mut g);
    cite!(["701.17c", "608.2c"]; assert_eq!(g.players[0].life, 24, "two land cards milled this way, found in exile"));
    assert_eq!(g.exile.iter().filter(|o| g.obj(**o).owner == 0).count(), 4, "three milled cards and the spell");
}

#[test]
fn review_mill_replaced_to_exile_still_answers_if_a_land_was_milled() {
    let mut g = game();
    g.put(1, PEACE, Zone::Battlefield);
    library(&mut g, 0, &[BEAR, LAND, BEAR, BEAR]);
    cast(&mut g, MILL_IF, vec![], "");
    settle(&mut g);
    cite!(["701.17c", "608.2c"]; assert_eq!(count(&g, &g.players[0].hand, BEAR), 1, "a land was milled this way: draw"));
}

#[test]
fn review_mill_replaced_to_exile_finds_the_milled_cards_in_exile() {
    let mut g = game();
    g.put(1, PEACE, Zone::Battlefield);
    library(&mut g, 0, &[LAND, BEAR, BEAR]);
    cast(&mut g, MILL_PICK, vec![], "");
    settle(&mut g);
    // "An effect that refers to a milled card can find that card in the zone it moved to
    // from the library, as long as that zone is a public zone." (701.17c)
    cite!(["701.17c", "608.2c"]; assert_eq!(count(&g, &g.players[0].hand, LAND), 1));
}

#[test]
fn review_a_discard_into_exile_is_still_a_discard_this_way() {
    let mut g = game();
    g.put(1, PEACE, Zone::Battlefield);
    library(&mut g, 0, &[LAND]);
    g.put(0, BEAR, Zone::Hand);
    g.put(1, BEAR, Zone::Hand);
    cast(&mut g, FANATIC, vec![], "CCCB");
    settle(&mut g);
    cite!(["701.9a", "614.1a", "608.2c"]; assert_eq!(count(&g, &g.players[0].hand, LAND), 1, "you discarded a card this way: draw"));
    assert_eq!(count(&g, &g.exile, BEAR), 2);
}

#[test]
fn review_a_madness_discard_is_still_a_discard_this_way() {
    let mut g = game();
    library(&mut g, 0, &[LAND]);
    g.put(0, TEMPER, Zone::Hand);
    g.put(1, BEAR, Zone::Hand);
    cast(&mut g, FANATIC, vec![], "CCCB");
    pass(&mut g);
    for _ in 0..6 {
        match offered(&g) { Some(among) => { choose(&mut g, Answer::Objects(vec![among[0]])); pass(&mut g); } None => break }
    }
    // the draw comes in the Fanatic's resolution, before the madness trigger resolves
    cite!(["702.35a", "701.9a", "608.2c"]; assert_eq!(count(&g, &g.players[0].hand, LAND), 1, "you discarded a card this way: draw"));
    assert_eq!(count(&g, &g.players[0].graveyard, TEMPER), 0, "discarded into exile, not the graveyard");
    assert_eq!(count(&g, &g.exile, TEMPER), 1, "madness: discarded into exile");
}

// Probe 3: a delayed trigger reads the group; a member that left and came back is new.

#[test]
fn review_hide_on_the_ceiling_does_not_return_a_card_that_left_exile_and_came_back() {
    let mut g = game();
    let mine = g.put(0, BEAR, Zone::Battlefield);
    let theirs = g.put(1, BEAR, Zone::Battlefield);
    let card = g.put(0, HIDE, Zone::Hand);
    mana(&mut g);
    g.act(0, paid("CCU", Move::WithX(2, Box::new(Move::Cast { card, targets: vec![vec![Target::Object(mine), Target::Object(theirs)]] })))).unwrap();
    pass(&mut g);
    assert_eq!(g.exile.len(), 2);
    let gone = *g.exile.iter().find(|o| g.obj(**o).owner == 1).unwrap();
    g.move_object(gone, Zone::Graveyard);
    let back_in_graveyard = *g.players[1].graveyard.iter().find(|o| g.obj(**o).card == Some(BEAR)).unwrap();
    g.move_object(back_in_graveyard, Zone::Exile);
    assert_eq!(g.exile.len(), 2);
    while g.part != Part::End || !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); if g.part == Part::End { pass(&mut g); } }
    let back: Vec<usize> = g.battlefield.iter().map(|o| g.obj(*o).owner).collect();
    cite!(["603.7c", "400.7"]; assert_eq!(back, vec![0]));
    cite!(["603.7c"]; assert_eq!(g.exile.iter().filter(|o| g.obj(**o).owner == 1).count(), 1, "the new object stays in exile"));
}

// Probe 4: the union of two producers when only one did something.

#[test]
fn review_two_exiles_when_the_second_has_nothing_left_to_exile() {
    for (lib, bears) in [(vec![BEAR, LAND], 1), (vec![BEAR, LAND, BEAR], 2), (vec![LAND, LAND], 0)] {
        let mut g = game();
        library(&mut g, 0, &lib);
        cast(&mut g, TWICE, vec![], "");
        settle(&mut g);
        cite!(["608.2c", "609.3"]; case: format!("{lib:?}"); assert_eq!(count(&g, &g.players[0].hand, BEAR), bears));
        assert_eq!(count(&g, &g.exile, LAND), count_of(&lib, LAND));
    }
}

fn count_of(list: &[usize], def: usize) -> usize { list.iter().filter(|d| **d == def).count() }

#[test]
fn review_the_first_producer_exiled_nothing_the_second_still_counts() {
    for relics in [0, 1] {
        let mut g = game();
        library(&mut g, 0, &[LAND]);
        for _ in 0..relics { g.put(1, RELIC, Zone::Battlefield); }
        cast(&mut g, NOTHING_COUNT, vec![], "");
        settle(&mut g);
        cite!(["608.2c", "609.3"]; case: relics; assert_eq!(g.players[0].life, 22 + 2 * relics as i32));
    }
    for relics in [0, 1] {
        let mut g = game();
        library(&mut g, 0, &[LAND]);
        for _ in 0..relics { g.put(1, RELIC, Zone::Battlefield); }
        cast(&mut g, NOTHING_FIRST, vec![], "");
        settle(&mut g);
        cite!(["608.2c", "609.3"]; case: relics; assert_eq!(count(&g, &g.players[0].hand, LAND), 1));
    }
}

// Probe 5: "if you discarded a card this way" is about the controller's own discard.

#[test]
fn review_fanatic_draws_nothing_when_only_the_opponent_discarded() {
    for opponent_cards in [0usize, 2] {
        let mut g = game();
        library(&mut g, 0, &[LAND]);
        for _ in 0..opponent_cards { g.put(1, BEAR, Zone::Hand); }
        cast(&mut g, FANATIC, vec![], "CCCB");
        settle(&mut g);
        cite!(["608.2c", "701.9a"]; case: opponent_cards; assert_eq!(g.players[0].hand.len(), 0));
        assert_eq!(g.players[0].library.len(), 1);
        assert_eq!(g.players[1].hand.len(), opponent_cards.saturating_sub(1));
    }
}
