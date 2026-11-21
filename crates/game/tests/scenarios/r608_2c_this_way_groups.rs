//! "This way" result groups (608.2c): a later instruction refers to what an earlier one of
//! the same resolution did, "the exiled cards", "creatures destroyed this way", "a card milled
//! this way", "if you put a Town card into your hand this way". The group is what was done,
//! over every producer the words name ("exiled this way" after two exiles is both); a member
//! that has since left the zone it was put into is a new object no longer in it (400.7); a
//! group nothing was put into is empty and the instructions that use it do nothing (609.3).
//! It stays readable by a delayed (603.7) or reflexive (603.12) trigger of that resolution.
use crate::support::{cite, paid, mana_pool};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const LAND: usize = 0;
const BEAR: usize = 1;
const TOWN: usize = 2;
const FUMIGATE: usize = 3;
const MILL_PICK: usize = 4;
const MILL_THEN_EXILE: usize = 5;
const BOTH_EXILES: usize = 6;
const BOTH_COUNT: usize = 7;
const DELAYED: usize = 8;
const REFLEXIVE: usize = 9;
const TOWN_GREETER: usize = 10;
const NO_PUT: usize = 11;
const FANATIC: usize = 12;
const ARCHWAY: usize = 13;
const DESERT: usize = 14;
const PIXIE: usize = 15;
const PEACE: usize = 16;
const DISCARD_PICK: usize = 17;

fn defs() -> Vec<CardDef> {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap();
    let c = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        c("Plain Land", "", "Land", "", None),
        c("Bear", "{G}", "Creature — Bear", "", Some((2, 2))),
        c("Town Square", "", "Land — Town", "", None),
        real("Fumigate"),
        c("Mill Pick", "{0}", "Sorcery", "Mill three cards. Put a land card from among the milled cards into your hand.", None),
        c("Mill Then Exile", "{0}", "Sorcery", "Mill three cards. Exile all land cards from your graveyard. Put a land card from among the milled cards into your hand.", None),
        c("Both Exiles", "{0}", "Sorcery", "Exile the top card of your library. Exile the top card of target opponent's library. You may put a land card from among the exiled cards into your hand.", None),
        c("Both Count", "{0}", "Sorcery", "Exile the top card of your library, then exile the top card of target opponent's library. You gain 2 life for each land card exiled this way.", None),
        c("Delayed Return", "{0}", "Instant", "Exile up to two target creatures. Return the exiled cards to the battlefield under their owners' control at the beginning of the next end step.", None),
        c("Reflexive Mill", "{0}", "Sorcery", "Mill three cards. You may pay {1}. When you do, return a creature card milled this way to your hand.", None),
        real("Town Greeter"),
        c("No Put", "{0}", "Sorcery", "Mill three cards. You may put a creature card from among them into your hand. If you put no cards into your hand this way, draw a card.", None),
        real("Fanatic of the Harrowing"),
        real("Arid Archway"),
        c("Other Desert", "", "Land — Desert", "", None),
        real("Nurturing Pixie"),
        real("Rest in Peace"),
        c("Discard Pick", "{0}", "Sorcery", "Discard two cards. Put a land card from among the discarded cards into your hand.", None),
    ]
}

fn game() -> Game {
    let defs = defs();
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}

fn library(g: &mut Game, player: usize, cards: &[usize]) -> Vec<ObjId> {
    cards.iter().map(|d| g.put(player, *d, Zone::Library)).collect()
}

/// Cast player 0's card with the stated targets and caller-authored mana selection.
fn cast(g: &mut Game, def: usize, targets: Vec<Target>, spending: &str) -> ObjId {
    let card = g.put(0, def, Zone::Hand);
    g.players[0].pool.colorless = 4;
    for c in 0..5 { g.players[0].pool.colors[c] = 2; }
    g.act(0, paid(spending, Move::Cast { card, targets: if targets.is_empty() { vec![] } else { vec![targets] } })).unwrap();
    card
}

/// Pass priority until a decision is asked or the stack is empty.
fn pass(g: &mut Game) {
    for _ in 0..40 {
        if g.decision.is_some() || g.stack.is_empty() { return; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("did not settle");
}

/// The objects a pending choice offers, if one is asked.
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

/// The cards of this player in this zone made from this definition.
fn cards(g: &Game, player: usize, zone: Zone, def: usize) -> usize {
    let list = match zone { Zone::Hand => &g.players[player].hand, Zone::Graveyard => &g.players[player].graveyard,
        Zone::Library => &g.players[player].library, _ => unreachable!() };
    list.iter().filter(|o| g.obj(**o).card == Some(def)).count()
}

#[test]
fn one_producer_several_results_offers_each_milled_card_as_it_is_now() {
    let mut g = game();
    let incoming = library(&mut g, 0, &[LAND, BEAR, TOWN]);
    cast(&mut g, MILL_PICK, vec![], "");
    pass(&mut g);
    cite!(["404.3"]; ok: g.act(0,Move::Choose(Answer::Objects(incoming))));
    let among = offered(&g).expect("a land card milled this way is chosen");
    cite!(["608.2c", "701.17a"]; assert_eq!(among.len(), 2));
    cite!(["400.7"]; assert!(among.iter().all(|o| g.obj(*o).zone == Zone::Graveyard)));
    let town = *among.iter().find(|o| g.obj(**o).card == Some(TOWN)).unwrap();
    choose(&mut g, Answer::Objects(vec![town]));
    pass(&mut g);
    assert_eq!(cards(&g, 0, Zone::Hand, TOWN), 1);
    assert_eq!(g.players[0].graveyard.len(), 3, "the other two milled cards and the spell");
}

/// Milled or discarded into exile instead (Rest in Peace, 614.1a): the cards are found where
/// they went, a public zone (701.17c, 400.7j). One that leaves that zone after is not.
#[test]
fn a_member_a_replacement_put_into_another_public_zone_is_found_there() {
    let mut g = game();
    g.put(1, PEACE, Zone::Battlefield);
    library(&mut g, 0, &[LAND, BEAR, TOWN]);
    cast(&mut g, MILL_PICK, vec![], "");
    pass(&mut g);
    let among = offered(&g).expect("the land cards milled into exile");
    cite!(["701.17c", "608.2c"]; assert_eq!(among.len(), 2));
    assert!(among.iter().all(|o| g.obj(*o).zone == Zone::Exile));
    let town = *among.iter().find(|o| g.obj(**o).card == Some(TOWN)).unwrap();
    choose(&mut g, Answer::Objects(vec![town]));
    pass(&mut g);
    assert_eq!(cards(&g, 0, Zone::Hand, TOWN), 1);

    let mut g = game();
    g.put(1, PEACE, Zone::Battlefield);
    g.put(0, LAND, Zone::Hand);
    g.put(0, BEAR, Zone::Hand);
    cast(&mut g, DISCARD_PICK, vec![], "");
    pass(&mut g);
    for _ in 0..3 {
        match &g.decision {
            Some(Decision::Choose { choice: Choice::Objects { among, .. }, .. }) => { let all = among.clone(); choose(&mut g, Answer::Objects(all)); pass(&mut g); }
            _ => break,
        }
    }
    cite!(["400.7j", "608.2c"]; assert_eq!(cards(&g, 0, Zone::Hand, LAND), 1, "the land discarded into exile, put into the hand"));
    assert_eq!(g.exile.iter().filter(|o| g.obj(**o).card == Some(BEAR)).count(), 1);
}

#[test]
fn every_creature_destroyed_counts_and_none_destroyed_gains_nothing() {
    for (n, regenerated) in [(0, false), (3, false), (2, true)] {
        let mut g = game();
        let bodies:Vec<_>=(0..n).map(|i|g.put(i%2,BEAR,Zone::Battlefield)).collect();
        if regenerated { let r = g.put(1, BEAR, Zone::Battlefield); g.regenerate(r); }
        cast(&mut g, FUMIGATE, vec![], "CCCWW");
        pass(&mut g);
        if n == 3 {cite!(["404.3"]; ok: g.act(0,Move::Choose(Answer::Objects(vec![bodies[0],bodies[2]]))));}
        cite!(["608.2c", "701.8a"]; case: n; assert_eq!(g.players[0].life, 20 + n as i32));
        cite!(["701.19a"]; case: n; assert_eq!(g.battlefield.len(), usize::from(regenerated)));
    }
}

#[test]
fn two_producers_in_one_ability_make_one_group() {
    let mut g = game();
    library(&mut g, 0, &[LAND]);
    library(&mut g, 1, &[TOWN]);
    cast(&mut g, BOTH_EXILES, vec![Target::Player(1)], "");
    pass(&mut g);
    choose(&mut g, Answer::Yes);
    let among = offered(&g).expect("a land card from among the exiled cards");
    cite!(["608.2c"]; assert_eq!(among.len(), 2, "both exiles' cards"));
    assert!(among.iter().all(|o| g.obj(*o).zone == Zone::Exile));
    let theirs = *among.iter().find(|o| g.obj(**o).owner == 1).unwrap();
    choose(&mut g, Answer::Objects(vec![theirs]));
    pass(&mut g);
    cite!(["400.3"]; assert_eq!(cards(&g, 1, Zone::Hand, TOWN), 1));

    for (mine, life) in [(LAND, 24), (BEAR, 22)] {
        let mut g = game();
        library(&mut g, 0, &[mine]);
        library(&mut g, 1, &[TOWN]);
        cast(&mut g, BOTH_COUNT, vec![Target::Player(1)], "");
        pass(&mut g);
        cite!(["608.2c"]; case: mine; assert_eq!(g.players[0].life, life));
    }
}

#[test]
fn a_member_that_left_its_zone_is_a_new_object_not_in_the_group() {
    let mut g = game();
    let incoming = library(&mut g, 0, &[LAND, BEAR, TOWN]);
    cast(&mut g, MILL_THEN_EXILE, vec![], "");
    pass(&mut g);
    cite!(["404.3"]; ok: g.act(0,Move::Choose(Answer::Objects(incoming))));
    cite!(["400.7", "608.2c"]; assert_eq!(g.decision, None, "both milled lands were exiled after"));
    assert_eq!(g.players[0].hand.len(), 0);
    assert_eq!(g.exile.len(), 2);
}

#[test]
fn a_delayed_trigger_returns_the_exiled_cards_still_in_exile() {
    for leaves in [false, true] {
        let mut g = game();
        let mine = g.put(0, BEAR, Zone::Battlefield);
        let theirs = g.put(1, BEAR, Zone::Battlefield);
        cast(&mut g, DELAYED, vec![Target::Object(mine), Target::Object(theirs)], "");
        pass(&mut g);
        assert_eq!(g.exile.len(), 2);
        if leaves { let gone = *g.exile.iter().find(|o| g.obj(**o).owner == 1).unwrap(); g.move_object(gone, Zone::Graveyard); }
        while g.part != Part::End || !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); if g.part == Part::End { pass(&mut g); } }
        let back: Vec<usize> = g.battlefield.iter().map(|o| g.controller(*o)).collect();
        cite!(["603.7", "400.7"]; case: leaves; assert_eq!(back, if leaves { vec![0] } else { vec![0, 1] }));
        cite!(["610.3"]; case: leaves; assert!(g.exile.is_empty()));
    }
}

#[test]
fn a_reflexive_trigger_selects_from_what_was_milled() {
    for (cards_milled, pay) in [([LAND, BEAR, TOWN], true), ([LAND, BEAR, TOWN], false), ([LAND, TOWN, LAND], true)] {
        let mut g = game();
        let incoming = library(&mut g, 0, &cards_milled);
        cast(&mut g, REFLEXIVE, vec![], "");
        pass(&mut g);
        cite!(["404.3"]; ok: g.act(0,Move::Choose(Answer::Objects(incoming))));
        choose(&mut g, if pay { Answer::Pay {plan:vec![],with:Some(mana_pool("C")),tapped:vec![]} } else { Answer::No });
        pass(&mut g);
        if let Some(among) = offered(&g) {
            cite!(["603.12", "608.2c"]; case: format!("{pay}"); assert_eq!(among.len(), 1));
            choose(&mut g, Answer::Objects(among));
            pass(&mut g);
        }
        let bear = cards_milled.contains(&BEAR) && pay;
        cite!(["603.12"]; case: format!("{cards_milled:?} {pay}"); assert_eq!(cards(&g, 0, Zone::Hand, BEAR), usize::from(bear)));
        assert!(g.decision.is_none() && g.stack.is_empty());
    }
}

#[test]
fn put_into_your_hand_this_way_asks_where_the_chosen_card_went() {
    for pick in [Some(TOWN), Some(LAND), None] {
        let mut g = game();
        let incoming = library(&mut g, 0, &[LAND, BEAR, TOWN, LAND]);
        cast(&mut g, TOWN_GREETER, vec![], "CG");
        pass(&mut g);
        pass(&mut g);
        cite!(["404.3"]; ok: g.act(0,Move::Choose(Answer::Objects(incoming))));
        choose(&mut g, if pick.is_some() { Answer::Yes } else { Answer::No });
        if let Some(def) = pick {
            let among = offered(&g).unwrap();
            let one = *among.iter().find(|o| g.obj(**o).card == Some(def)).unwrap();
            choose(&mut g, Answer::Objects(vec![one]));
        }
        pass(&mut g);
        cite!(["608.2c"]; case: format!("{pick:?}"); assert_eq!(g.players[0].life, if pick == Some(TOWN) { 22 } else { 20 }));
    }
}

#[test]
fn put_no_cards_this_way_is_the_empty_group() {
    // the top three are milled; the bottom card is drawn when nothing is put into the hand
    for (top, put) in [([LAND, TOWN, LAND], false), ([LAND, BEAR, TOWN], true), ([LAND, BEAR, TOWN], false)] {
        let mut g = game();
        library(&mut g, 0, &[LAND]);
        let incoming = library(&mut g, 0, &top);
        cast(&mut g, NO_PUT, vec![], "");
        pass(&mut g);
        cite!(["404.3"]; ok: g.act(0,Move::Choose(Answer::Objects(incoming))));
        if g.decision == Some(Decision::Choose { player: 0, choice: Choice::May }) {
            choose(&mut g, if put { Answer::Yes } else { Answer::No });
            // a single creature card milled is the one put, with nothing left to choose
            if let Some(among) = offered(&g) { choose(&mut g, Answer::Objects(among)); }
        }
        pass(&mut g);
        cite!(["608.2c", "609.3"]; case: format!("{top:?} {put}"); assert_eq!(g.players[0].hand.len(), 1));
        assert_eq!(cards(&g, 0, Zone::Hand, BEAR), usize::from(put));
        assert_eq!(g.players[0].library.len(), usize::from(put));
    }
}

#[test]
fn you_discarded_this_way_is_only_what_the_controller_discarded() {
    for own in [true, false] {
        let mut g = game();
        library(&mut g, 0, &[LAND]);
        if own { g.put(0, BEAR, Zone::Hand); }
        g.put(1, BEAR, Zone::Hand);
        cast(&mut g, FANATIC, vec![], "CCCB");
        pass(&mut g);
        pass(&mut g);
        for _ in 0..4 {
            match offered(&g) { Some(among) => choose(&mut g, Answer::Objects(vec![among[0]])), None => break }
            pass(&mut g);
        }
        cite!(["608.2c", "701.9a"]; case: own; assert_eq!(g.players[0].hand.len(), usize::from(own)));
        assert_eq!(g.players[1].hand.len(), 0);
    }
}

#[test]
fn another_desert_returned_this_way_is_not_the_source_itself() {
    for (bounce_self, other) in [(true, DESERT), (false, DESERT), (false, LAND)] {
        let mut g = game();
        library(&mut g, 0, &[LAND]);
        let back = g.put(0, other, Zone::Battlefield);
        let archway = g.put(0, ARCHWAY, Zone::Hand);
        g.act(0, Move::PlayLand(archway)).unwrap();
        pass(&mut g);
        let here = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(ARCHWAY)).unwrap();
        choose(&mut g, Answer::Objects(vec![if bounce_self { here } else { back }]));
        pass(&mut g);
        let surveil = matches!(g.decision, Some(Decision::Choose { choice: Choice::Arrange { .. }, .. }));
        cite!(["608.2c", "400.7"]; case: format!("{bounce_self} {other}"); assert_eq!(surveil, !bounce_self && other == DESERT));
    }
}

#[test]
fn up_to_one_returned_this_way_and_none() {
    for chosen in [true, false] {
        let mut g = game();
        let bear = g.put(0, BEAR, Zone::Battlefield);
        cast(&mut g, PIXIE, vec![], "W");
        pass(&mut g);
        // the enters trigger targets up to one
        let Some(Decision::Targets { player, .. }) = g.decision else { panic!("{:?}", g.decision) };
        g.act(player, Move::ChooseTargets(vec![if chosen { vec![Target::Object(bear)] } else { vec![] }])).unwrap();
        pass(&mut g);
        let pixie = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(PIXIE)).unwrap();
        cite!(["608.2c"]; case: chosen; assert_eq!(g.obj(pixie).counters.get("+1/+1").copied().unwrap_or(0), u32::from(chosen)));
        assert_eq!(cards(&g, 0, Zone::Hand, BEAR), usize::from(chosen));
    }
}

#[test]
fn a_result_no_earlier_instruction_produced_is_refused() {
    for text in ["Put a land card from among the milled cards into your hand.",
        "You gain 1 life for each creature destroyed this way.",
        "Destroy all creatures. If you put a card into your hand this way, draw a card."] {
        let def = CardDef::new("Refused", "{0}", "Sorcery", text, None).unwrap();
        // Uncited (admission): every row names an absent earlier result producer.
        crate::support::refused(game::play::check(&def).map_err(|e|e.0), "no earlier instruction");
    }
}
