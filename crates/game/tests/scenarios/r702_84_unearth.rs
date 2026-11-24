//! Unearth (702.84a): "[Cost]: Return this card from your graveyard to the battlefield. It
//! gains haste. Exile it at the beginning of the next end step. If it would leave the
//! battlefield, exile it instead of putting it anywhere else. Activate only as a sorcery."
use crate::support::{cite, mana_pool, Board, T};
use game::{Game, Move, ObjId, Target, Zone};
use ir::Part;

/// The card's unearth ability among its abilities: printed last on each card here.
fn unearth_row(g: &Game, card: ObjId) -> usize {
    g.abilities(card).iter().rposition(|a| matches!(&**a, ir::Ability::Activated { .. })).expect("an unearth ability")
}

/// Activate the unearth ability of this card in `p`'s graveyard, paying `mana`.
fn unearth(t: &mut T, p: usize, name: &str, mana: &str) -> Result<(), String> {
    let card = t.find_in(p, Zone::Graveyard, name);
    let ability = unearth_row(&t.g, card);
    t.g.players[p].pool = mana_pool(mana);
    t.g.act(p, Move::Paying(mana_pool(mana), Box::new(Move::Activate { source: card, ability, targets: vec![], paying: vec![] })))
        .map(|_| ()).map_err(|e| e.to_string())
}

/// Player 0 has unearthed this card for `mana`, holding `hand`.
fn unearthed(name: &str, mana: &str, hand: &[&str]) -> T {
    let mut t = Board::new(2).fill(5).at(Part::PrecombatMain).graveyard(0, &[name]).hand(0, hand).build();
    unearth(&mut t, 0, name, mana).expect("unearthed");
    t.resolve();
    t
}

/// Pass priority, with no attack or block, until the stack is empty in player 0's end step.
fn to_end_step(t: &mut T) {
    t.to_no_combat(0, Part::End);
}

#[test]
fn r702_84a_it_returns_with_haste_and_is_exiled_at_the_beginning_of_the_next_end_step() {
    let mut t = Board::new(2).fill(5).at(Part::PrecombatMain).graveyard(0, &["Dregscape Zombie"]).build();
    cite!(["702.84a", "602.2"]; ok: unearth(&mut t, 0, "Dregscape Zombie", "B"));
    // the ability is on the stack, the card is still in the graveyard (602.2a)
    cite!(["702.84a", "602.2a"]; assert_eq!(t.g.stack.len(), 1));
    t.resolve();
    let zombie = t.find_in_battlefield("Dregscape Zombie");
    cite!(["702.84a", "702.10a"]; assert!(t.g.has(zombie, "Haste")));
    // it can attack the turn it came under its controller's control (302.6, 702.10b)
    while t.g.part != Part::DeclareAttackers { let p = t.g.priority; t.g.act(p, Move::Pass).unwrap(); }
    cite!(["702.84a", "702.10b"]; ok: t.g.act(0, Move::Attack(vec![(zombie, Target::Player(1))])));
    // no block; at the end step the delayed trigger exiles it
    to_end_step(&mut t);
    t.expect().rules(["702.84a", "603.7a", "510.2"]).life(1, 18)
        .zone(0, Zone::Exile, &["Dregscape Zombie"]).zone(0, Zone::Battlefield, &[]).zone(0, Zone::Graveyard, &[]).check();
}

#[test]
fn r702_84a_unearth_is_activated_only_from_a_graveyard_and_only_as_a_sorcery() {
    // on the opponent's turn
    let mut t = Board::new(2).fill(5).active(1).at(Part::PrecombatMain).graveyard(0, &["Dregscape Zombie"]).build();
    t.g.act(1, Move::Pass).unwrap();
    cite!(["702.84a", "602.5d"]; refused("that ability is activated only as a sorcery: in a main phase of your own turn with the stack empty (602.5d)"): unearth(&mut t, 0, "Dregscape Zombie", "B"));
    // with the stack not empty
    let mut t = Board::new(2).fill(5).at(Part::PrecombatMain).graveyard(0, &["Dregscape Zombie"]).hand(0, &["Unsummon"])
        .named(1, Zone::Battlefield, "bears", "Grizzly Bears").build();
    t.g.players[0].pool = mana_pool("U");
    t.cast(0, "Unsummon", vec![crate::support::on(1, Zone::Battlefield, "Grizzly Bears")], mana_pool("U")).unwrap();
    cite!(["702.84a", "307.1"]; refused("that ability is activated only as a sorcery: in a main phase of your own turn with the stack empty (602.5d)"): unearth(&mut t, 0, "Dregscape Zombie", "B"));
    // it is an ability of the card in a graveyard, not of the permanent (113.6m)
    let mut t = Board::new(2).fill(5).at(Part::PrecombatMain).battlefield(0, &["Dregscape Zombie"]).build();
    let zombie = t.find_in_battlefield("Dregscape Zombie");
    let ability = unearth_row(&t.g, zombie);
    t.g.players[0].pool = mana_pool("B");
    cite!(["702.84a", "113.6m"]; refused("that ability functions only in the zone it moves its card out of (113.6m)"): t.g.act(0, Move::Paying(mana_pool("B"), Box::new(Move::Activate { source: zombie, ability, targets: vec![], paying: vec![] }))));
}

#[test]
fn r702_84a_destroyed_it_is_exiled_instead_and_still_leaves_the_battlefield() {
    // "When this creature leaves the battlefield, you gain 2 life."
    let mut t = unearthed("Tocasia's Onulet", "WWWW", &["Murder"]);
    let onulet = t.find_in_battlefield("Tocasia's Onulet");
    t.g.players[0].pool = mana_pool("BBB");
    t.cast(0, "Murder", vec![crate::support::on(0, Zone::Battlefield, "Tocasia's Onulet")], mana_pool("BBB")).unwrap();
    t.resolve();
    cite!(["614.1a", "702.84a"]; assert!(!t.g.battlefield.contains(&onulet)));
    t.expect().rules(["702.84a", "614.1a", "603.6c"]).life(0, 22)
        .zone(0, Zone::Exile, &["Tocasia's Onulet"]).zone(0, Zone::Graveyard, &["Murder"]).check();
}

#[test]
fn r702_84a_returned_to_a_hand_or_put_into_a_library_it_is_exiled_instead() {
    let mut t = unearthed("Dregscape Zombie", "B", &["Unsummon"]);
    t.g.players[0].pool = mana_pool("U");
    t.cast(0, "Unsummon", vec![crate::support::on(0, Zone::Battlefield, "Dregscape Zombie")], mana_pool("U")).unwrap();
    t.resolve();
    t.expect().rules(["702.84a", "614.1a"]).zone(0, Zone::Exile, &["Dregscape Zombie"]).zone(0, Zone::Hand, &[]).check();

    let mut t = unearthed("Dregscape Zombie", "B", &["Griptide"]);
    let library = t.g.players[0].library.len();
    t.g.players[0].pool = mana_pool("UUUU");
    t.cast(0, "Griptide", vec![crate::support::on(0, Zone::Battlefield, "Dregscape Zombie")], mana_pool("UUUU")).unwrap();
    t.resolve();
    cite!(["702.84a", "614.1a"]; assert_eq!(t.g.players[0].library.len(), library));
    t.expect().rules(["702.84a", "614.1a"]).zone(0, Zone::Exile, &["Dregscape Zombie"]).check();
}

#[test]
fn r702_84a_sacrificed_it_is_exiled_instead() {
    // "{2}, Sacrifice this creature: Draw a card."
    let mut t = unearthed("Combat Courier", "U", &[]);
    let hand = t.g.players[0].hand.len();
    t.g.players[0].pool = mana_pool("BB");
    cite!(["602.2", "702.84a"]; ok: t.activate(0, "Combat Courier", 0, vec![], mana_pool("BB")));
    t.resolve();
    cite!(["701.21a"]; assert_eq!(t.g.players[0].hand.len(), hand + 1));
    t.expect().rules(["702.84a", "614.1a", "701.21a"]).zone(0, Zone::Exile, &["Combat Courier"]).zone(0, Zone::Graveyard, &[]).check();
}

#[test]
fn r702_84a_once_it_has_left_and_returned_the_new_object_is_not_unearthed() {
    // "Exile target creature you control, then return that card to the battlefield under your
    // control."
    let mut t = unearthed("Dregscape Zombie", "B", &["Cloudshift", "Murder"]);
    t.g.players[0].pool = mana_pool("W");
    t.cast(0, "Cloudshift", vec![crate::support::on(0, Zone::Battlefield, "Dregscape Zombie")], mana_pool("W")).unwrap();
    t.resolve();
    let zombie = t.find_in_battlefield("Dregscape Zombie");
    // a new object (400.7): neither the delayed exile nor the replacement applies to it
    to_end_step(&mut t);
    cite!(["400.7", "702.84a"]; assert!(t.g.battlefield.contains(&zombie)));
    t.to_no_combat(1, Part::PrecombatMain);
    t.to_no_combat(0, Part::PrecombatMain);
    t.g.players[0].pool = mana_pool("BBB");
    t.cast(0, "Murder", vec![crate::support::on(0, Zone::Battlefield, "Dregscape Zombie")], mana_pool("BBB")).unwrap();
    t.resolve();
    t.expect().rules(["400.7", "702.84a"]).contains(0, Zone::Graveyard, "Dregscape Zombie").zone(0, Zone::Exile, &[]).check();
}

#[test]
fn r702_84a_unearth_does_nothing_if_the_card_left_the_graveyard() {
    let mut t = Board::new(2).fill(5).at(Part::PrecombatMain).graveyard(0, &["Dregscape Zombie"]).build();
    unearth(&mut t, 0, "Dregscape Zombie", "B").unwrap();
    let card = t.find_in(0, Zone::Graveyard, "Dregscape Zombie");
    // put elsewhere in response, by whatever means
    t.g.move_object(card, Zone::Exile);
    t.resolve();
    t.expect().rules(["702.84a", "608.2b"]).zone(0, Zone::Battlefield, &[]).zone(0, Zone::Exile, &["Dregscape Zombie"]).check();
}


#[test]
fn r614_1a_a_creature_returned_by_whip_of_erebos_is_exiled_instead_of_dying() {
    // "{2}{B}{B}, {T}: Return target creature card from your graveyard to the battlefield. It
    // gains haste. Exile it at the beginning of the next end step. If it would leave the
    // battlefield, exile it instead of putting it anywhere else. Activate only as a sorcery."
    let mut t = Board::new(2).fill(5).at(Part::PrecombatMain).battlefield(0, &["Whip of Erebos"])
        .graveyard(0, &["Grizzly Bears"]).hand(0, &["Murder"]).build();
    t.g.players[0].pool = mana_pool("BBBB");
    cite!(["602.2"]; ok: t.activate(0, "Whip of Erebos", 1, vec![crate::support::on(0, Zone::Graveyard, "Grizzly Bears")], mana_pool("BBBB")));
    t.resolve();
    let bears = t.find_in_battlefield("Grizzly Bears");
    // "it" is the permanent the card became (400.7j)
    cite!(["400.7j", "702.10a"]; assert!(t.g.has(bears, "Haste")));
    t.g.players[0].pool = mana_pool("BBB");
    t.cast(0, "Murder", vec![crate::support::on(0, Zone::Battlefield, "Grizzly Bears")], mana_pool("BBB")).unwrap();
    t.resolve();
    t.expect().rules(["614.1a", "400.7j"]).zone(0, Zone::Exile, &["Grizzly Bears"]).zone(0, Zone::Graveyard, &["Murder"]).check();
}

#[test]
fn r614_1a_a_creature_put_onto_the_battlefield_by_gruesome_encore_is_exiled_instead_of_returning_to_hand() {
    // "Put target creature card from an opponent's graveyard onto the battlefield under your
    // control. It gains haste. Exile it at the beginning of the next end step. If that creature
    // would leave the battlefield, exile it instead of putting it anywhere else."
    let mut t = Board::new(2).fill(5).at(Part::PrecombatMain).hand(0, &["Gruesome Encore", "Unsummon"])
        .graveyard(1, &["Grizzly Bears"]).build();
    t.g.players[0].pool = mana_pool("BBB");
    cite!(["601.2"]; ok: t.cast(0, "Gruesome Encore", vec![crate::support::on(1, Zone::Graveyard, "Grizzly Bears")], mana_pool("BBB")));
    t.resolve();
    let bears = t.find_in_battlefield("Grizzly Bears");
    cite!(["400.7j", "702.10a"]; assert!(t.g.has(bears, "Haste") && t.g.controller(bears) == 0));
    t.g.players[0].pool = mana_pool("U");
    t.cast(0, "Unsummon", vec![crate::support::on(0, Zone::Battlefield, "Grizzly Bears")], mana_pool("U")).unwrap();
    t.resolve();
    // exiled, which its owner's hand would have been
    t.expect().rules(["614.1a", "400.7j"]).zone(1, Zone::Exile, &["Grizzly Bears"]).zone(1, Zone::Hand, &[]).check();
}

#[test]
fn r702_84a_an_unearth_cost_that_is_not_mana_is_paid() {
    // "Unearth—Pay eight {E}." (Salvation Colossus)
    let mut t = Board::new(2).fill(5).at(Part::PrecombatMain).graveyard(0, &["Salvation Colossus"]).build();
    t.g.put_counters(Target::Player(0), "energy", 7);
    cite!(["702.84a", "107.14", "118.3"]; refused("player 0 can't have 8 energy counters removed to pay this cost (118.3, 118.10)"): unearth(&mut t, 0, "Salvation Colossus", ""));
    t.g.put_counters(Target::Player(0), "energy", 2);
    cite!(["702.84a", "107.14"]; ok: unearth(&mut t, 0, "Salvation Colossus", ""));
    t.resolve();
    let colossus = t.find_in_battlefield("Salvation Colossus");
    cite!(["702.84a", "122.1"]; assert!(t.g.has(colossus, "Haste") && t.g.counters(Target::Player(0)).get("energy") == Some(&1)));
}
