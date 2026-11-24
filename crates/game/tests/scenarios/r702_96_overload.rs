//! Overload (702.96): "You may choose to pay [cost] rather than pay this spell's mana cost" and
//! "If you chose to pay this spell's overload cost, change its text by replacing all instances
//! of the word 'target' with the word 'each'" (702.96a). Such a spell has no targets (702.96b).
use crate::support::{cite, mana_pool, player, Board, Boundary, Tgt, T};
use game::announcement::CastCost;
use game::state::Answer;
use game::{Game, Move, ObjId, Target, Zone};
use ir::Part;

/// The row of the card's overload ability: the alternative cost it is cast for (702.96a).
fn overload_row(g: &Game, card: ObjId) -> usize {
    g.def(card).unwrap().abilities().iter().position(|a| matches!(a, ir::Ability::Keywords(items)
        if items.iter().any(|i| i.keyword.name() == "Overload"))).expect("an overload ability")
}

/// Cast this card from `p`'s hand for its overload cost, paying `mana`.
fn overloaded(t: &mut T, p: usize, name: &str, targets: Vec<Vec<Target>>, mana: &str) -> Result<(), String> {
    let card = t.find_in(p, Zone::Hand, name);
    let row = overload_row(&t.g, card);
    let cast = Move::CastFor(CastCost::Alternative(row), Box::new(Move::Cast { card, targets }));
    t.g.players[p].pool = mana_pool(mana);
    t.g.act(p, Move::Paying(mana_pool(mana), Box::new(cast))).map(|_| ()).map_err(|e| e.to_string())
}

fn on_stack(t: &T, name: &str) -> ObjId {
    *t.g.stack.iter().find(|o| t.g.def(**o).is_some_and(|d| d.name() == name)).unwrap_or_else(|| panic!("no {name} on the stack"))
}

/// Player 0 holds `hand`; each player controls a Llanowar Elves (1/1) and a Grizzly Bears (2/2).
fn board(hand: &[&str]) -> T {
    Board::new(2).fill(5).at(Part::PrecombatMain).hand(0, hand)
        .named(0, Zone::Battlefield, "my elves", "Llanowar Elves")
        .named(0, Zone::Battlefield, "my bears", "Grizzly Bears")
        .named(1, Zone::Battlefield, "elves", "Llanowar Elves")
        .named(1, Zone::Battlefield, "bears", "Grizzly Bears")
        .build()
}

#[test]
fn r702_96a_cast_for_its_overload_cost_it_affects_each_and_has_no_targets() {
    // "Electrickery deals 1 damage to target creature you don't control." Overload {1}{R}
    let mut t = board(&["Electrickery"]);
    cite!(["702.96a", "601.2b"]; ok: overloaded(&mut t, 0, "Electrickery", vec![], "RR"));
    let spell = on_stack(&t, "Electrickery");
    cite!(["702.96b", "601.2c"]; assert!(t.g.obj(spell).targets.iter().all(|slot| slot.is_empty())));
    t.resolve();
    t.expect().rules(["702.96a", "702.96b", "120.3"])
        .zone(1, Zone::Battlefield, &["Grizzly Bears"]).contains(1, Zone::Graveyard, "Llanowar Elves")
        .contains(0, Zone::Battlefield, "Llanowar Elves").contains(0, Zone::Battlefield, "Grizzly Bears").check();
}

#[test]
fn r702_96b_an_overloaded_spell_announces_no_target() {
    let mut t = board(&["Electrickery"]);
    let elves = Target::Object(t.find_in(1, Zone::Battlefield, "Llanowar Elves"));
    cite!(["702.96b", "601.2c"]; refused("that has 0 target group(s), 1 given"): overloaded(&mut t, 0, "Electrickery", vec![vec![elves]], "RR"));
    // its overload cost is the whole cost: {R} alone does not pay it (601.2f)
    cite!(["702.96a", "601.2f"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): overloaded(&mut t, 0, "Electrickery", vec![], "R"));
}

#[test]
fn r702_96a_cast_for_its_mana_cost_it_has_its_one_target() {
    let mut t = board(&["Electrickery"]);
    t.g.players[0].pool = mana_pool("R");
    cite!(["702.96a", "601.2c"]; ok: t.cast(0, "Electrickery", vec![crate::support::on(1, Zone::Battlefield, "Llanowar Elves")], mana_pool("R")));
    t.resolve();
    t.expect().rules(["702.96a", "115.1a"]).zone(1, Zone::Battlefield, &["Grizzly Bears"])
        .contains(0, Zone::Battlefield, "Llanowar Elves").check();
}

#[test]
fn r702_96b_it_affects_what_could_not_be_targeted() {
    // "Return target nonland permanent you don't control to its owner's hand." Overload {6}{U}
    let mut t = Board::new(2).fill(5).at(Part::PrecombatMain).hand(0, &["Cyclonic Rift"])
        .named(0, Zone::Battlefield, "mine", "Grizzly Bears")
        .named(1, Zone::Battlefield, "scout", "Gladecover Scout")
        .named(1, Zone::Battlefield, "bears", "Grizzly Bears")
        .named(1, Zone::Battlefield, "forest", "Forest")
        .build();
    let hand = t.g.players[1].hand.len();
    cite!(["702.96a"]; ok: overloaded(&mut t, 0, "Cyclonic Rift", vec![], "UUUUUUU"));
    t.resolve();
    // the hexproof Scout is returned too (702.11b does not apply: nothing targets it)
    cite!(["702.96b", "702.11b"]; assert_eq!(t.g.players[1].hand.len(), hand + 2));
    t.expect().rules(["702.96a", "702.96b"]).zone(1, Zone::Battlefield, &["Forest"])
        .contains(0, Zone::Battlefield, "Grizzly Bears").check();
}

#[test]
fn r702_96a_target_player_becomes_each_player() {
    // "Target player discards two cards." Overload {1}{B}; overloaded, it affects all players,
    // its caster too (Mind Rake ruling)
    let mut t = Board::new(2).fill(5).at(Part::PrecombatMain).hand(0, &["Mind Rake", "Forest", "Forest"])
        .hand(1, &["Forest", "Forest", "Forest"]).build();
    cite!(["702.96a"]; ok: overloaded(&mut t, 0, "Mind Rake", vec![], "BB"));
    // each player discards two cards of their choice
    while let Boundary::Question(p, _) = t.settle() {
        let two = t.g.players[p].hand[..2].to_vec();
        t.answer(p, Answer::Objects(two)).unwrap();
    }
    cite!(["702.96a", "701.9a"]; assert_eq!((t.g.players[0].hand.len(), t.g.players[1].hand.len()), (0, 1)));
}

#[test]
fn r702_96a_counterflux_overloaded_counters_each_spell_you_dont_control() {
    // "Counter target spell you don't control." Overload {1}{U}{U}{R}
    let mut t = Board::new(2).fill(5).at(Part::PrecombatMain).hand(0, &["Lightning Bolt", "Counterflux"])
        .hand(1, &["Lightning Bolt"]).build();
    t.g.players[0].pool = mana_pool("R");
    t.cast(0, "Lightning Bolt", vec![player(1)], mana_pool("R")).unwrap();
    t.g.act(0, Move::Pass).unwrap();
    t.g.players[1].pool = mana_pool("R");
    t.cast(1, "Lightning Bolt", vec![player(0)], mana_pool("R")).unwrap();
    t.g.act(1, Move::Pass).unwrap();
    cite!(["702.96a"]; ok: overloaded(&mut t, 0, "Counterflux", vec![], "UUUR"));
    t.resolve();
    t.expect().rules(["702.96a", "701.6a"]).life(0, 20).life(1, 17)
        .contains(1, Zone::Graveyard, "Lightning Bolt").check();
}

#[test]
fn r702_96a_overload_cost_with_x() {
    // "Street Spasm deals X damage to target creature without flying you don't control."
    // Overload {X}{X}{R}{R}
    let mut t = board(&["Street Spasm"]);
    let card = t.find_in(0, Zone::Hand, "Street Spasm");
    let row = overload_row(&t.g, card);
    t.g.players[0].pool = mana_pool("RRRRRR");
    let cast = Move::WithX(2, Box::new(Move::CastFor(CastCost::Alternative(row), Box::new(Move::Cast { card, targets: vec![] }))));
    cite!(["702.96a", "107.3a", "601.2f"]; ok: t.g.act(0, Move::Paying(mana_pool("RRRRRR"), Box::new(cast))));
    // both of the opponent's creatures die at once; their owner orders them (405.6)
    while let Boundary::Question(p, game::legal::Asked::OrderCards { cards, .. }) = t.settle() {
        t.answer(p, Answer::Objects(cards)).unwrap();
    }
    t.expect().rules(["702.96a", "107.3a"]).zone(1, Zone::Battlefield, &[])
        .contains(0, Zone::Battlefield, "Grizzly Bears").check();
}

#[test]
fn r707_10_a_copy_of_an_overloaded_spell_is_overloaded() {
    // "Copy target instant or sorcery spell. You may choose new targets for the copy."
    let mut t = board(&["Electrickery", "Twincast"]);
    overloaded(&mut t, 0, "Electrickery", vec![], "RR").unwrap();
    let spell = on_stack(&t, "Electrickery");
    t.g.players[0].pool = mana_pool("UU");
    t.cast(0, "Twincast", vec![Tgt::Object(spell)], mana_pool("UU")).unwrap();
    t.resolve();
    // the copy deals 1 damage to each, then the spell: the Bears have 2 (Overload ruling, 707.10)
    t.expect().rules(["707.10", "702.96a"]).zone(1, Zone::Battlefield, &[])
        .contains(0, Zone::Battlefield, "Grizzly Bears").check();
}
