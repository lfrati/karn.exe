//! Cumulative upkeep (702.24a): an age counter, then the cost paid once for each age counter
//! as one payment, or the permanent is sacrificed. Every card is read from its Oracle text.
use crate::support::*;
use crate::support::{cite, mana_pool, oracle, oracle_read};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Zone};
use ir::Part;

fn fixture() -> (Game, ObjId) {
    let mut g = Game::new(vec![oracle("Mystic Remora"), oracle("Island"), CardDef::new("Counter ban", "{0}", "Enchantment", "Enchantments can't have counters put on them.", None).unwrap()], 2, 20);
    for p in 0..2 { for _ in 0..20 { g.put(p, 1, Zone::Library); } }
    let remora = g.put(0, 0, Zone::Battlefield);
    (g, remora)
}
fn next_upkeep(g: &mut Game) {
    let turn = g.turn;
    for _ in 0..200 {
        if g.turn > turn && g.active == 0 && g.part == Part::Upkeep { return; }
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            None => g.act(g.priority, Move::Pass).unwrap(),
            other => panic!("unexpected question {other:?}"),
        }
    }
    panic!("no upkeep");
}
fn resolve(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
fn offered(g: &Game) -> u32 {
    let Some(Decision::Choose { player: 0, choice: Choice::Pay { cost, .. } }) = &g.decision else { panic!("{:?}", g.decision) };
    let [ir::mana::ManaSymbol::Generic(n)] = cost.0.as_slice() else { panic!("{cost:?}") };
    *n
}
#[test]
fn r702_24a_age_accumulates_and_partial_payment_does_not_count() {
    let (mut g, remora) = fixture();
    next_upkeep(&mut g); resolve(&mut g);
    cite!(["702.24a"]; assert_eq!((g.obj(remora).counters.get("age"), offered(&g)), (Some(&1), 1)));
    g.players[0].pool = mana_pool("C");
    g.act(0, Move::Choose(Answer::Pay { plan: vec![], with: Some(mana_pool("C")), tapped: vec![] })).unwrap();
    cite!(["702.24a"]; assert!(g.battlefield.contains(&remora)));
    next_upkeep(&mut g); resolve(&mut g);
    cite!(["702.24a"]; assert_eq!((g.obj(remora).counters.get("age"), offered(&g)), (Some(&2), 2)));
    g.players[0].pool = mana_pool("CC");
    cite!(["702.24a", "118.3a"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0, Move::Choose(Answer::Pay { plan: vec![], with: Some(mana_pool("C")), tapped: vec![] })));
    g.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["702.24a"]; assert!(!g.battlefield.contains(&remora)));
}
#[test]
fn r702_24a_no_age_counters_allows_zero_payment_and_departed_source_has_no_payment() {
    let (mut g, remora) = fixture();
    g.put(0, 2, Zone::Battlefield);
    next_upkeep(&mut g); resolve(&mut g);
    cite!(["702.24a", "101.2"]; assert_eq!(offered(&g), 0));
    g.act(0, Move::Choose(Answer::Pay { plan: vec![], with: Some(mana_pool("")), tapped: vec![] })).unwrap();
    cite!(["702.24a"]; assert!(g.battlefield.contains(&remora)));
    let (mut g, remora) = fixture();
    next_upkeep(&mut g);
    g.destroy(remora);
    resolve(&mut g);
    cite!(["603.4", "702.24a"]; assert!(g.decision.is_none()));
}
#[test]
fn repeated_costs_of_other_actions_remain_refused() {
    // Uncited (admission): a repeated payment of counters, draws and other actions has no lowering yet.
    for name in ["Aboroth", "Psychic Vortex"] {
        crate::support::refused(game::play::check(&oracle_read(name)).map_err(|e| e.0), "keyword: Cumulative Upkeep");
    }
}

/// Player 0's next upkeep, its cumulative upkeep abilities resolved up to the first question
/// they ask (702.24a), or with nothing asked.
fn upkeep(t: &mut T) -> Boundary {
    let turn = t.g.turn;
    for _ in 0..2000 {
        if t.g.turn > turn && t.g.active == 0 && t.g.part == Part::Upkeep && !t.g.stack.is_empty() { return t.settle(); }
        match t.g.decision.clone() {
            Some(Decision::Attackers { player }) => { t.g.act(player, Move::Attack(vec![])).unwrap(); }
            Some(Decision::Blockers { player }) => { t.g.act(player, Move::Block(vec![])).unwrap(); }
            _ => { let p = t.g.decision_maker(t.g.priority); t.g.act(p, Move::Pass).unwrap(); }
        }
    }
    panic!("no upkeep of player 0 with its cumulative upkeep on the stack");
}
/// The mana and life it asks to be paid, its symbols sorted.
fn owed(boundary: Boundary) -> (Vec<String>, u32) {
    let Boundary::Question(0, Asked::Pay { cost, life, .. }) = boundary else { panic!("expected a payment, got {boundary:?}") };
    let mut symbols: Vec<String> = cost.0.iter().map(|s| s.text()).collect();
    symbols.sort();
    (symbols, life)
}
fn pay(t: &mut T, mana: &str) -> Result<(), String> {
    t.g.players[0].pool = mana_pool(mana);
    t.g.act(0, Move::Choose(Answer::Pay { plan: vec![], with: Some(mana_pool(mana)), tapped: vec![] })).map(|_| ())
}
fn s(symbols: &[&str]) -> Vec<String> { symbols.iter().map(|s| s.to_string()).collect() }
/// "You may [sacrifice or discard] …": yes, and the objects it then asks for (608.2d).
fn agree(t: &mut T, boundary: Boundary) -> Boundary {
    let Boundary::Question(0, Asked::May) = boundary else { return boundary };
    t.answer(0, Answer::Yes).unwrap();
    t.settle()
}

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Mystic Remora", "Uktabi Efreet", "Firestorm Hellkite", "Gallowbraid", "Inner Sanctum", "Jötun Owl Keeper",
        "Arctic Nishoba", "Phyrexian Soulgorger", "Polar Kraken", "Vexing Sphinx"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

#[test]
fn r702_24a_colored_mana_is_paid_once_for_each_age_counter() {
    // Firestorm Hellkite: "Cumulative upkeep {U}{R}"
    let mut t = Board::new(2).fill(10).battlefield(0, &["Firestorm Hellkite"]).build();
    let first = owed(upkeep(&mut t));
    cite!(["702.24a"]; assert_eq!(first, (s(&["R", "U"]), 0)));
    cite!(["702.24a"]; ok: pay(&mut t, "UR"));
    t.expect().rules(["702.24a"]).counters("Firestorm Hellkite", "age", 1).check();
    let second = owed(upkeep(&mut t));
    cite!(["702.24a"]; assert_eq!(second, (s(&["R", "R", "U", "U"]), 0)));
    // {U}{R} once is a partial payment, and none is made (702.24a, 118.3a)
    cite!(["702.24a", "118.3a"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): pay(&mut t, "UR"));
    cite!(["702.24a"]; ok: pay(&mut t, "UURR"));
    t.expect().rules(["702.24a"]).counters("Firestorm Hellkite", "age", 2).zone_exact(0, Zone::Battlefield, &["Firestorm Hellkite"]).check();
}

#[test]
fn r702_24a_unpaid_colored_upkeep_sacrifices_it() {
    // Uktabi Efreet: "Cumulative upkeep {G}"
    let mut t = Board::new(2).fill(10).battlefield(0, &["Uktabi Efreet"]).build();
    upkeep(&mut t);
    cite!(["702.24a"]; ok: pay(&mut t, "G"));
    let second = owed(upkeep(&mut t));
    cite!(["702.24a"]; assert_eq!(second, (s(&["G", "G"]), 0)));
    cite!(["702.24a", "118.3a"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): pay(&mut t, "CG"));
    t.answer(0, Answer::No).unwrap();
    t.expect().rules(["702.24a", "701.21a"]).zone_exact(0, Zone::Battlefield, &[]).zone_exact(0, Zone::Graveyard, &["Uktabi Efreet"]).check();
}

#[test]
fn r702_24a_life_is_paid_once_for_each_age_counter() {
    // Inner Sanctum: "Cumulative upkeep—Pay 2 life."
    let mut t = Board::new(2).fill(10).battlefield(0, &["Inner Sanctum"]).build();
    let first = owed(upkeep(&mut t));
    cite!(["702.24a"]; assert_eq!(first, (vec![], 2)));
    cite!(["702.24a", "119.4"]; ok: pay(&mut t, ""));
    t.expect().rules(["702.24a", "119.4"]).life(0, 18).check();
    let second = owed(upkeep(&mut t));
    cite!(["702.24a"]; assert_eq!(second, (vec![], 4)));
    cite!(["702.24a", "119.4"]; ok: pay(&mut t, ""));
    t.expect().rules(["702.24a", "119.4"]).life(0, 14).counters("Inner Sanctum", "age", 2).check();
}

#[test]
fn r119_4_a_player_with_too_little_life_cannot_pay_it() {
    // Gallowbraid: "Cumulative upkeep—Pay 1 life." Its second upkeep asks 2 life of a
    // player with 1: they can't pay it (119.4), so it is sacrificed (702.24a).
    let mut t = Board::new(2).fill(10).battlefield(0, &["Gallowbraid"]).build();
    upkeep(&mut t);
    cite!(["702.24a"]; ok: pay(&mut t, ""));
    t.g.players[0].life = 1;
    let asked = upkeep(&mut t);
    cite!(["119.4", "118.3"]; assert!(matches!(asked, Boundary::Priority(_)), "nothing to pay with: {asked:?}"));
    t.expect().rules(["702.24a", "119.4"]).life(0, 1).zone_exact(0, Zone::Graveyard, &["Gallowbraid"]).check();
}

#[test]
fn r702_24a_one_of_two_colors_is_chosen_for_each_age_counter() {
    // Jötun Owl Keeper: "Cumulative upkeep {W} or {U}. When this creature dies, create a 1/1
    // white Bird creature token with flying for each age counter on it."
    let mut t = Board::new(2).fill(10).battlefield(0, &["Jötun Owl Keeper"]).build();
    upkeep(&mut t);
    cite!(["702.24a"]; ok: pay(&mut t, "U"));
    upkeep(&mut t);
    // a choice for each age counter: {W} for one, {U} for the other
    cite!(["702.24a"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): pay(&mut t, "UG"));
    cite!(["702.24a"]; ok: pay(&mut t, "WU"));
    upkeep(&mut t);
    cite!(["702.24a"]; ok: pay(&mut t, "WWW"));
    t.expect().rules(["702.24a"]).counters("Jötun Owl Keeper", "age", 3).check();
    upkeep(&mut t);
    t.answer(0, Answer::No).unwrap();
    // it was sacrificed with four age counters on it (603.10a)
    t.resolve();
    t.expect().rules(["702.24a", "603.10a"]).zone_exact(0, Zone::Graveyard, &["Jötun Owl Keeper"])
        .holds(|g| g.battlefield.iter().filter(|o| g.obj(**o).token()).count() == 4, "four Bird tokens").check();
}

#[test]
fn r702_24a_a_different_creature_is_sacrificed_for_each_age_counter() {
    // Phyrexian Soulgorger: "Cumulative upkeep—Sacrifice a creature." Ruling: "you must either
    // sacrifice a different creature for each age counter on Phyrexian Soulgorger or sacrifice
    // it. You may also sacrifice it as part of its own upkeep payment."
    let mut t = Board::new(2).fill(10).battlefield(0, &["Phyrexian Soulgorger", "Grizzly Bears", "Llanowar Elves", "Savannah Lions", "Forest"]).build();
    let first = upkeep(&mut t);
    let first = agree(&mut t, first);
    let Boundary::Question(0, Asked::Objects { count: 1, among, .. }) = first else { panic!("{first:?}") };
    // a creature: not the Forest; the Soulgorger itself is one
    cite!(["702.24a", "701.21a"]; assert_eq!(among.len(), 4));
    cite!(["702.24a"]; ok: t.choose(0, &["Grizzly Bears"]).map(|_| ()));
    t.expect().rules(["702.24a", "701.21a"]).zone_exact(0, Zone::Graveyard, &["Grizzly Bears"]).counters("Phyrexian Soulgorger", "age", 1).check();
    let second = upkeep(&mut t);
    let second = agree(&mut t, second);
    cite!(["702.24a"]; assert!(matches!(second, Boundary::Question(0, Asked::Objects { count: 2, .. })), "two creatures: {second:?}"));
    // one creature is a partial payment
    cite!(["702.24a", "118.3a"]; refused("choose exactly 2"): t.choose(0, &["Llanowar Elves"]).map(|_| ()));
    // its upkeep trigger on the stack shares its name: the permanent is chosen by its object
    let (elves, gorger) = (t.find_in_battlefield("Llanowar Elves"), t.find_in_battlefield("Phyrexian Soulgorger"));
    cite!(["702.24a"]; ok: t.answer(0, Answer::Objects(vec![elves, gorger])).map(|_| ()));
    // the two go to the graveyard at once, in the order their owner chooses (701.21a)
    if let Boundary::Question(0, Asked::OrderCards { cards, .. }) = t.settle() { t.answer(0, Answer::Objects(cards)).unwrap(); }
    t.expect().rules(["702.24a", "701.21a"]).zone_exact(0, Zone::Battlefield, &["Forest", "Savannah Lions"])
        .zone_exact(0, Zone::Graveyard, &["Grizzly Bears", "Llanowar Elves", "Phyrexian Soulgorger"]).check();
}

#[test]
fn r702_24a_without_enough_lands_to_sacrifice_it_is_sacrificed() {
    // Polar Kraken: "Cumulative upkeep—Sacrifice a land." Its second upkeep asks two lands of
    // a player with one: they can't pay it (118.3).
    let mut t = Board::new(2).fill(10).battlefield(0, &["Polar Kraken"]).named(0, Zone::Battlefield, "one", "Island")
        .named(0, Zone::Battlefield, "other", "Island").build();
    let first = upkeep(&mut t);
    let first = agree(&mut t, first);
    cite!(["702.24a"]; assert!(matches!(first, Boundary::Question(0, Asked::Objects { count: 1, .. })), "one land: {first:?}"));
    let one = t.id("one");
    cite!(["702.24a"]; ok: t.answer(0, Answer::Objects(vec![one])).map(|_| ()));
    let second = upkeep(&mut t);
    cite!(["118.3", "702.24a"]; assert!(matches!(second, Boundary::Priority(_)), "nothing to pay with: {second:?}"));
    t.expect().rules(["702.24a", "118.3"]).zone_exact(0, Zone::Battlefield, &["Island"]).zone_exact(0, Zone::Graveyard, &["Island", "Polar Kraken"]).check();
}

#[test]
fn r702_24a_a_card_is_discarded_for_each_age_counter() {
    // Vexing Sphinx: "Cumulative upkeep—Discard a card. When this creature dies, draw a card for
    // each age counter on it." Ruling: "if Vexing Sphinx has three age counters on it when its
    // cumulative upkeep ability triggers, it gets another age counter and then its controller
    // chooses to either discard four cards or sacrifice it."
    let mut t = Board::new(2).fill(10).hand(0, &["Island", "Island", "Island"]).battlefield(0, &["Vexing Sphinx"]).build();
    let first = upkeep(&mut t);
    let first = agree(&mut t, first);
    cite!(["702.24a"]; assert!(matches!(first, Boundary::Question(0, Asked::Objects { count: 1, .. })), "one card: {first:?}"));
    let hand = t.g.players[0].hand.clone();
    t.answer(0, Answer::Objects(vec![hand[0]])).unwrap();
    t.expect().rules(["702.24a", "701.9a"]).holds(|g| g.players[0].graveyard.len() == 1, "one card discarded").check();
    let second = upkeep(&mut t);
    cite!(["702.24a"]; assert!(matches!(second, Boundary::Question(0, Asked::May)), "pay or not: {second:?}"));
    t.answer(0, Answer::No).unwrap();
    let hand = t.g.players[0].hand.len();
    t.resolve();
    // it died with two age counters on it (603.10a)
    t.expect().rules(["702.24a", "603.10a"]).zone_exact(0, Zone::Battlefield, &[])
        .holds(|g| g.players[0].hand.len() == hand + 2, "two cards drawn").check();
}
