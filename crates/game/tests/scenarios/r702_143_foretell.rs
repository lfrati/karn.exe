//! Foretell (702.143): during their own turn, with any stack, a player may pay {2} and exile a
//! card with foretell from their hand face down, a special action (116.2h, 702.143b). Only they
//! may look at it while it stays exiled (702.143a, 406.3). After that turn they may cast it for
//! its foretell cost, an alternative cost (118.9); it is turned face up just before it is
//! announced (406.3a). Foretold cards are revealed when their owner leaves the game and at the
//! end of the game (702.143f).
use crate::support;
use game::foretell::Foretold;
use game::snapshot::Snapshot;
use game::Event;
use support::*;

type Text = (&'static str, &'static str, &'static str, &'static str, Option<(i32, i32)>);
const WATCHER: Text = ("Foretell watcher", "{0}", "Artifact", "Whenever you foretell a card, you gain 1 life.", None);

/// Oracle cards by name, then test texts, each at its index among the definitions; each
/// library holds ten Wastes.
fn game(players: usize, names: &[&str], texts: &[Text]) -> T {
    let mut defs: Vec<CardDef> = ["Wastes"].iter().chain(names).map(|n| oracle(n)).collect();
    for (name, cost, types, text, pt) in texts {
        let d = CardDef::new(name, cost, types, text, *pt).unwrap();
        game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
        defs.push(d);
    }
    let mut g = Game::new(defs, players, 20);
    for p in 0..players { for _ in 0..10 { g.put(p, 0, Zone::Library); } }
    T::new(g)
}
fn put(t: &mut T, p: usize, name: &str, zone: Zone) -> ObjId {
    let index = t.g.defs.iter().position(|d| d.name() == name).unwrap_or_else(|| panic!("no {name}"));
    t.g.put(p, index, zone)
}
fn view(g: &Game, viewer: Option<usize>, o: ObjId) -> Option<game::snapshot::Object> {
    Snapshot::of(g, viewer).objects.into_iter().find(|x| x.id == o)
}
/// Player `p` pays {2} from their pool and foretells this hand card: what it is in exile.
fn foretell(t: &mut T, p: usize, card: ObjId) -> ObjId {
    let before = t.g.exile.clone();
    t.mana(p, "{2}");
    t.g.act(p, paid("CC", Move::Foretell(card))).unwrap();
    let new: Vec<ObjId> = t.g.exile.iter().copied().filter(|o| !before.contains(o)).collect();
    assert_eq!(new.len(), 1, "one card was exiled");
    new[0]
}
/// To player 1's precombat main phase, where player 1 passes so player 0 has priority.
fn to_their_main(t: &mut T) {
    t.to_no_combat(1, Part::PrecombatMain);
    t.g.act(1, Move::Pass).unwrap();
    assert_eq!(t.g.priority, 0);
}
fn cast(t: &mut T, p: usize, card: ObjId, targets: Vec<Target>) -> Result<(), String> {
    t.g.act(p, paid("R", Move::Cast { card, targets: targets.into_iter().map(|t| vec![t]).collect() }))
}
fn foretold_cast(t: &mut T, p: usize, card: ObjId, targets: Vec<Target>, mana: &str) -> Result<(), String> {
    t.g.act(p, paid(mana, Move::CastVia(game::announcement::CastRoute::Foretell, Box::new(Move::Cast { card, targets: targets.into_iter().map(|t| vec![t]).collect() }))))
}

#[test]
fn r702_143a_foretelling_pays_two_and_exiles_the_card_face_down_without_the_stack() {
    let mut t = game(2, &["Demon Bolt"], &[WATCHER]);
    put(&mut t, 0, "Foretell watcher", Zone::Battlefield);
    let bolt = put(&mut t, 0, "Demon Bolt", Zone::Hand);
    let card = foretell(&mut t, 0, bolt);
    cite!(["702.143a"]; assert_eq!(t.g.players[0].pool.total(), 0, "{{2}} was paid"));
    assert!(!t.g.players[0].hand.contains(&bolt));
    assert_eq!(t.g.foretold(card), Some(Foretold { by: 0, turn: t.g.turn }));
    cite!(["702.143a"]; assert_eq!(t.g.hidden(card).map(|h| h.lookers.clone()), Some(vec![0]), "only its player may look at it"));
    cite!(["702.143b"]; assert!(!t.g.log.iter().any(|e| matches!(e, Event::Cast { .. } | Event::Activated { .. })), "a special action, not a spell or ability (702.143b)"));
    assert!(t.g.log.iter().any(|e| matches!(e, Event::Foretold { player: 0, card: c } if *c == card)));
    // its trigger is put on the stack; the special action itself never was
    cite!(["116.3"]; assert_eq!(t.g.priority, 0));
    t.resolve();
    cite!(["702.143c"]; assert_eq!(t.g.players[0].life, 21, "\"whenever you foretell a card\" triggered"));
}

#[test]
fn r702_143a_its_player_alone_sees_it_with_no_look_decision_and_the_designation_is_public() {
    let mut t = game(2, &["Demon Bolt"], &[]);
    let bolt = put(&mut t, 0, "Demon Bolt", Zone::Hand);
    let card = foretell(&mut t, 0, bolt);
    assert!(matches!(t.g.asked(), Some((0, Asked::Priority))), "nobody is asked whether to look");
    cite!(["702.143a"]; assert!(t.g.sees(0, card) && !t.g.sees(1, card)));
    let turn = t.g.turn;
    let mine = view(&t.g, Some(0), card).unwrap();
    let theirs = view(&t.g, Some(1), card).unwrap();
    assert_eq!(mine.hidden.and_then(|h| h.card).map(|c| c.name).as_deref(), Some("Demon Bolt"));
    assert_eq!(theirs.hidden.as_ref().and_then(|h| h.card.clone()), None);
    cite!(["406.3a"]; assert_eq!((theirs.name.as_str(), theirs.types.is_empty()), ("", true), "no characteristics (406.3a)"));
    cite!(["702.143e"]; assert_eq!((mine.foretold, theirs.foretold), (Some(turn), Some(turn)), "which card was foretold when is public (702.143e)"));
}

#[test]
fn r116_2h_foretelling_is_done_with_a_spell_on_the_stack_but_only_during_your_turn() {
    let mut t = game(2, &["Demon Bolt", "Lightning Bolt"], &[]);
    let first = put(&mut t, 0, "Demon Bolt", Zone::Hand);
    let second = put(&mut t, 0, "Demon Bolt", Zone::Hand);
    let shock = put(&mut t, 0, "Lightning Bolt", Zone::Hand);
    t.mana(0, "{R}");
    cast(&mut t, 0, shock, vec![Target::Player(1)]).unwrap();
    assert_eq!(t.g.stack.len(), 1);
    foretell(&mut t, 0, first);
    cite!(["702.143a", "702.170a"]; assert_eq!(t.g.stack.len(), 1, "unlike plot, any stack (702.143a, 702.170a)"));
    t.resolve();
    to_their_main(&mut t);
    t.mana(0, "{2}");
    cite!(["702.143a"]; refused("foretell only during your own turn (702.143a)"): t.g.act(0, Move::Foretell(second)));
    assert!(t.g.players[0].hand.contains(&second));
    assert_eq!(t.g.players[0].pool.total(), 2, "nothing was paid");
}

#[test]
fn r702_143a_a_foretold_card_is_cast_for_its_foretell_cost_only_after_that_turn() {
    let mut t = game(2, &["Demon Bolt", "Grizzly Bears"], &[]);
    let bears = put(&mut t, 1, "Grizzly Bears", Zone::Battlefield);
    let bolt = put(&mut t, 0, "Demon Bolt", Zone::Hand);
    let card = foretell(&mut t, 0, bolt);
    t.mana(0, "{2}{R}");
    cite!(["702.143a"]; refused("only a card you foretold is cast for its foretell cost, after the turn it was foretold (702.143a)"): foretold_cast(&mut t, 0, card, vec![Target::Object(bears)], "R"));
    assert_eq!((t.g.foretold(card).is_some(), t.g.hidden(card).is_some()), (true, true), "a refused cast leaves it foretold, face down");
    assert!(!t.g.sees(1, card));
    to_their_main(&mut t);
    t.mana(0, "{R}");
    foretold_cast(&mut t, 0, card, vec![Target::Object(bears)], "R").unwrap();
    cite!(["702.143a"]; assert_eq!(t.g.players[0].pool.total(), 0, "its foretell cost {{R}}, not its mana cost {{2}}{{R}}"));
    let spell = *t.g.stack.last().unwrap();
    cite!(["406.3a"]; assert_eq!(t.g.obj(spell).chars.name, "Demon Bolt", "turned face up as it was cast (406.3a)"));
    assert!(t.g.sees(1, spell));
    t.resolve();
    assert_eq!(t.g.obj(bears).zone, Zone::Gone);
    assert!(t.g.players[0].graveyard.iter().any(|o| t.g.obj(*o).chars.name == "Demon Bolt"));
}

#[test]
fn r406_3a_a_refused_foretold_cast_turns_nothing_face_up() {
    let mut t = game(2, &["Demon Bolt", "Grizzly Bears"], &[]);
    let bears = put(&mut t, 1, "Grizzly Bears", Zone::Battlefield);
    let bolt = put(&mut t, 0, "Demon Bolt", Zone::Hand);
    let card = foretell(&mut t, 0, bolt);
    to_their_main(&mut t);
    let before = t.g.log.end();
    cite!(["118.3a"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): foretold_cast(&mut t, 0, card, vec![Target::Object(bears)], ""));
    cite!(["601.2c", "115.1"]; refused("an illegal target, that player: a player isn't a permanent (115.1)"): foretold_cast(&mut t, 0, card, vec![Target::Player(1)], ""));
    assert_eq!(t.g.hidden(card).map(|h| h.lookers.clone()), Some(vec![0]));
    assert_eq!(t.g.obj(card).chars.name, "");
    assert_eq!(t.g.foretold(card).map(|f| f.by), Some(0));
    assert!(t.g.log.since(before).is_empty(), "nothing happened");
    assert_eq!(view(&t.g, Some(1), card).and_then(|o| o.hidden).and_then(|h| h.card), None);
}

#[test]
fn r702_143a_only_its_foretell_cost_and_ordinary_timing_apply() {
    let mut t = game(2, &["Augury Raven"], &[]);
    let raven = put(&mut t, 0, "Augury Raven", Zone::Hand);
    let card = foretell(&mut t, 0, raven);
    to_their_main(&mut t);
    t.mana(0, "{1}{U}");
    cite!(["117.1a"]; refused("that is cast in a main phase of your own turn with the stack empty (117.1a)"): foretold_cast(&mut t, 0, card, vec![], "CU"));
    t.to_no_combat(0, Part::PrecombatMain);
    t.mana(0, "{3}{U}");
    // through its own route: not as a card cast from exile for its mana cost
    let normal = Move::CastVia(game::announcement::CastRoute::Normal, Box::new(Move::Cast { card, targets: vec![] }));
    cite!(["601.3", "702.143a"]; refused("that card is not in your hand: it is in exile, and no rule or effect lets you cast it from there (601.3)"): t.g.act(0, normal));
    foretold_cast(&mut t, 0, card, vec![], "CU").unwrap();
    cite!(["702.143a"]; assert_eq!(t.g.players[0].pool.total(), 2, "{{1}}{{U}} paid"));
    t.resolve();
    let raven = t.find_in(0, Zone::Battlefield, "Augury Raven");
    assert_eq!((t.g.power(raven), t.g.toughness(raven)), (Some(3), Some(3)));
}

#[test]
fn r601_2f_sage_of_the_beyond_reduces_a_foretold_spell_cast_from_exile() {
    let mut t = game(2, &["Augury Raven", "Sage of the Beyond"], &[]);
    put(&mut t, 0, "Sage of the Beyond", Zone::Battlefield);
    let raven = put(&mut t, 0, "Augury Raven", Zone::Hand);
    let card = foretell(&mut t, 0, raven);
    to_their_main(&mut t);
    t.to_no_combat(0, Part::PrecombatMain);
    t.mana(0, "{U}");
    cite!(["118.9d"]; ok: foretold_cast(&mut t, 0, card, vec![], "U"));
    t.resolve();
    t.find_in(0, Zone::Battlefield, "Augury Raven");
}

#[test]
fn r702_143a_a_foretold_spell_was_cast_from_exile() {
    let mut t = game(2, &["Ultimate Magic: Holy", "Lightning Bolt"], &[]);
    let holy = put(&mut t, 0, "Ultimate Magic: Holy", Zone::Hand);
    let shock = put(&mut t, 1, "Lightning Bolt", Zone::Hand);
    let card = foretell(&mut t, 0, holy);
    to_their_main(&mut t);
    t.mana(0, "{2}{W}");
    foretold_cast(&mut t, 0, card, vec![], "CCW").unwrap();
    t.resolve();
    t.mana(1, "{R}");
    cast(&mut t, 1, shock, vec![Target::Player(0)]).unwrap();
    t.resolve();
    cite!(["702.143a"]; assert_eq!(t.g.players[0].life, 20, "cast from exile, so damage to its player is prevented"));
}

#[test]
fn r116_2h_foretelling_is_your_turn_only_but_later_instant_casting_is_not() {
    let mut t = game(2, &["Demon Bolt", "Grizzly Bears"], &[]);
    let target = put(&mut t, 1, "Grizzly Bears", Zone::Battlefield);
    let bolt = put(&mut t, 0, "Demon Bolt", Zone::Hand);
    t.mana(0, "{2}");
    cite!(["116.2h"]; ok: t.g.act(0, paid("CC", Move::Foretell(bolt))));
    let card = *t.g.exile.last().unwrap();
    t.mana(0, "{R}");
    cite!(["702.143a"]; refused("only a card you foretold is cast for its foretell cost, after the turn it was foretold"): unchanged(&mut t.g, |g| g.act(0, paid("R", Move::CastVia(game::announcement::CastRoute::Foretell, Box::new(Move::Cast { card, targets: vec![vec![Target::Object(target)]] }))))));
    to_their_main(&mut t);
    t.mana(0, "{R}");
    let other = put(&mut t, 0, "Demon Bolt", Zone::Hand);
    t.mana(0, "{2}");
    cite!(["116.2h"]; refused("foretell only during your own turn"): unchanged(&mut t.g, |g| g.act(0, paid("CC", Move::Foretell(other)))));
    cite!(["702.143a"]; ok: t.g.act(0, paid("R", Move::CastVia(game::announcement::CastRoute::Foretell, Box::new(Move::Cast { card, targets: vec![vec![Target::Object(target)]] })))));
}

#[test]
fn r400_7_a_foretold_card_that_leaves_exile_is_a_new_face_up_object_not_foretold() {
    let mut t = game(2, &["Demon Bolt"], &[]);
    let bolt = put(&mut t, 0, "Demon Bolt", Zone::Hand);
    let card = foretell(&mut t, 0, bolt);
    t.g.move_object(card, Zone::Hand);
    let back = *t.g.players[0].hand.last().unwrap();
    cite!(["400.7"]; assert_eq!((t.g.foretold(back), t.g.hidden(back).is_some()), (None, false)));
    t.g.exile(back);
    let again = *t.g.exile.last().unwrap();
    cite!(["400.7"]; assert_eq!(t.g.foretold(again), None));
    to_their_main(&mut t);
    t.mana(0, "{R}");
    cite!(["400.7"]; refused("only a card you foretold is cast for its foretell cost, after the turn it was foretold (702.143a)"): t.g.act(0, Move::CastVia(game::announcement::CastRoute::Foretell, Box::new(Move::Cast { card: again, targets: vec![] }))));
}

#[test]
fn r702_143f_foretold_cards_are_revealed_at_the_end_of_the_game() {
    let mut t = game(2, &["Demon Bolt", "Lightning Bolt"], &[]);
    t.g.players[1].life = 3;
    let bolt = put(&mut t, 0, "Demon Bolt", Zone::Hand);
    let shock = put(&mut t, 0, "Lightning Bolt", Zone::Hand);
    let card = foretell(&mut t, 0, bolt);
    assert!(!t.g.log.iter().any(|e| matches!(e, Event::Revealed { .. })));
    t.mana(0, "{R}");
    cast(&mut t, 0, shock, vec![Target::Player(1)]).unwrap();
    t.expect_losses(&[1]);
    t.settle();
    assert!(t.g.finished());
    cite!(["702.143f"]; assert!(t.g.log.iter().any(|e| matches!(e, Event::Revealed { player: 0, cards } if cards.contains(&card)))));
}

#[test]
fn r702_143f_a_player_who_leaves_the_game_reveals_their_foretold_cards() {
    let mut t = game(3, &["Demon Bolt", "Lightning Bolt"], &[]);
    let bolt = put(&mut t, 0, "Demon Bolt", Zone::Hand);
    let shock = put(&mut t, 0, "Lightning Bolt", Zone::Hand);
    let card = foretell(&mut t, 0, bolt);
    t.g.players[0].life = 3;
    t.mana(0, "{R}");
    cast(&mut t, 0, shock, vec![Target::Player(0)]).unwrap();
    t.expect_losses(&[0]);
    t.settle();
    assert!(t.g.players[0].lost && !t.g.finished());
    cite!(["702.143f"]; assert!(t.g.log.iter().any(|e| matches!(e, Event::Revealed { player: 0, cards } if cards.contains(&card)))));
}

#[test]
fn r733_1_a_reversed_foretold_cast_puts_it_back_foretold_and_face_down() {
    const MANA: Text = ("Mana creature", "{0}", "Creature", "{T}: Put a +1/+1 counter on this creature. Add X mana of any one color, where X is the number of +1/+1 counters on this creature.", Some((1, 1)));
    let mut t = game(2, &["Depart the Realm", "Grizzly Bears"], &[MANA]);
    let source = put(&mut t, 0, "Mana creature", Zone::Battlefield);
    let bears = put(&mut t, 1, "Grizzly Bears", Zone::Battlefield);
    let depart = put(&mut t, 0, "Depart the Realm", Zone::Hand);
    let card = foretell(&mut t, 0, depart);
    to_their_main(&mut t);
    let plan = vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source, ability: 0, paying: vec![], adding: vec![] }];
    t.g.act(0, Move::WithMana(plan, Box::new(Move::CastVia(game::announcement::CastRoute::Foretell, Box::new(Move::Cast { card, targets: vec![vec![Target::Object(bears)]] }))))).unwrap();
    // red does not pay its foretell cost {U}: the payment cannot finish, and it is reversed
    t.g.act(0, Move::Choose(Answer::Mana(vec![ir::mana::ManaSymbol::Color(ir::Color::Red)]))).unwrap();
    assert!(matches!(t.g.decision, Some(Decision::Payment { .. })), "{:?}", t.g.decision);
    t.g.act(0, Move::ReversePayment).unwrap();
    assert!(t.g.exile.contains(&card) && t.g.stack.is_empty());
    assert_eq!(t.g.foretold(card).map(|f| f.by), Some(0), "still foretold");
    assert_eq!(t.g.hidden(card).map(|h| h.lookers.clone()), Some(vec![0]), "face down again");
    assert_eq!(t.g.obj(card).chars.name, "");
    assert!(!t.g.sees(1, card));
    // and it may still be cast for its foretell cost
    t.mana(0, "{U}");
    foretold_cast(&mut t, 0, card, vec![Target::Object(bears)], "U").unwrap();
    t.resolve();
    assert!(t.g.players[1].hand.iter().any(|o| t.g.obj(*o).chars.name == "Grizzly Bears"));
}

#[track_caller]
fn unchanged(g: &mut Game, action: impl FnOnce(&mut Game) -> Result<(), String>) -> Result<(), String> {
    let before=format!("{g:?}");
    let result=action(g);
    assert_eq!(format!("{g:?}"),before);
    result
}
