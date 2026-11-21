//! Manifest (701.40): the top card of a library enters face down as a 2/2 creature (708.2a),
//! its controller may look at it (708.5), and it is turned face up by the special action for
//! its card's mana cost (701.40b, 116.2b) or by an effect without paying (Break Open). A
//! manifested instant or sorcery is revealed instead and stays face down (701.40g).
use crate::support::cite;
use crate::support;
use game::face::{Concealment, Way};
use game::observation::Kind;
use game::Event;
use support::*;

/// The face-down permanents on the battlefield, oldest first.
fn face_down(t: &T) -> Vec<ObjId> {
    t.g.battlefield.iter().copied().filter(|o| t.g.face_down(*o)).collect()
}
fn only_face_down(t: &T) -> ObjId {
    let down = face_down(t);
    assert_eq!(down.len(), 1, "one face-down permanent: {down:?}");
    down[0]
}
/// Player `p` casts Soul Summons with mana given for it, and it resolves.
fn summon(t: &mut T, p: usize) -> ObjId {
    let before = face_down(t);
    t.mana(p, "{1}{W}");
    t.cast(p, "Soul Summons", vec![], crate::support::mana_pool("CW")).unwrap();
    t.resolve();
    let now: Vec<ObjId> = face_down(t).into_iter().filter(|o| !before.contains(o)).collect();
    assert_eq!(now.len(), 1, "one card manifested");
    now[0]
}
/// The listed special action that turns this permanent face up, if any.

fn turned_up_events(g: &Game, from: game::event::Seq) -> usize {
    g.log.since(from).iter().filter(|e| matches!(e, Event::TurnedFaceUp { .. })).count()
}

#[test]
fn r701_40a_a_manifested_card_is_a_nameless_2_2_creature_with_no_text() {
    let mut t = Board::new(2).hand(0, &["Soul Summons"]).library(0, &["Shivan Dragon"]).build();
    let o = summon(&mut t, 0);
    let g = &t.g;
    assert!(g.players[0].library.is_empty(), "the top card was manifested");
    cite!(["701.40a"]; assert_eq!(g.face(o), game::face::Face::Down(game::face::Down { by: Concealment::Manifested })));
    let seen = g.seen(o, None);
    cite!(["701.40a"]; assert_eq!(g.obj(o).chars.name, ""));
    cite!(["701.40a"]; assert_eq!(seen.types, vec![ir::CardType::Creature]));
    assert!(seen.subtypes.is_empty() && seen.supertypes.is_empty());
    assert!(seen.colors.is_empty(), "colorless: it has no mana cost and no color indicator");
    assert_eq!(g.obj(o).chars.mana_value, 0);
    assert!(g.def(o).is_some_and(|d| d.cost().is_none()), "no mana cost");
    cite!(["701.40a"]; assert_eq!((g.power(o), g.toughness(o)), (Some(2), Some(2))));
    cite!(["701.40a"]; assert!(g.abilities(o).is_empty(), "no text: Shivan Dragon's flying and firebreathing are not there"));
    assert!(!g.has(o, "Flying"));
    assert_eq!(g.controller(o), 0);
    assert_eq!(g.obj(o).owner, 0);
}

#[test]
fn r701_40b_the_special_action_pays_the_cards_mana_cost_and_turns_it_up_with_a_nonempty_stack() {
    let mut t = Board::new(2).hand(0, &["Soul Summons"]).hand(1, &["Lightning Bolt"]).library(0, &["Grizzly Bears"]).build();
    let o = summon(&mut t, 0);
    t.g.put_counters(o, "+1/+1", 1);
    t.g.tap(o);
    // the opponent bolts it; with Lightning Bolt on the stack its controller turns it face up
    t.g.act(0, Move::Pass).unwrap();
    t.mana(1, "{R}");
    t.cast(1, "Lightning Bolt", vec![Tgt::Object(o)], crate::support::mana_pool("R")).unwrap();
    t.g.act(1, Move::Pass).unwrap();
    assert_eq!(t.g.priority, 0);
    let before=format!("{:?}",t.g);
    cite!(["701.40b","118.3a"]; refused("that mana is not in your mana pool"): t.g.act(0, crate::support::paid("CG", Move::TurnFaceUp(o, Way::Manifested))));
    assert_eq!(format!("{:?}",t.g),before);
    t.mana(0, "{1}{G}");
    let from = t.mark();
    t.g.act(0, crate::support::paid("CG", Move::TurnFaceUp(o, Way::Manifested))).unwrap();
    cite!(["701.40b"]; assert_eq!(t.g.stack.len(), 1, "a special action does not use the stack"));
    assert_eq!(t.g.priority, 0, "its controller keeps priority");
    cite!(["701.40b"]; assert_eq!(t.g.players[0].pool.total(), 0, "{{1}}{{G}} was paid"));
    cite!(["708.8"]; assert!(!t.g.face_down(o) && t.g.battlefield.contains(&o), "the same object, face up (708.8)"));
    assert_eq!(t.g.obj(o).chars.name, "Grizzly Bears");
    cite!(["708.8"]; assert_eq!((t.g.power(o), t.g.toughness(o)), (Some(3), Some(3)), "its counter still applies"));
    assert!(t.g.obj(o).tapped, "still tapped");
    assert_eq!(turned_up_events(&t.g, from), 1);
    // the bolt still targets it and kills it
    t.resolve();
    assert!(t.g.players[0].graveyard.iter().any(|c| t.g.obj(*c).chars.name == "Grizzly Bears"));
}

#[test]
fn r701_40b_announcing_reveals_once() {
    let mut t = Board::new(2).hand(0, &["Soul Summons"]).library(0, &["Grizzly Bears"]).build();
    let o = summon(&mut t, 0);
    for p in 0..2 { t.g.take_observations(p); }
    t.mana(0, "{1}{G}");
    assert!(t.g.observations(1).is_empty(), "the card remains private until announcement");
    t.g.act(0, crate::support::paid("CG", Move::TurnFaceUp(o, Way::Manifested))).unwrap();
    for p in 0..2 {
        let shown = t.g.observations(p);
        cite!(["701.40b"]; case: p; assert_eq!(shown.len(), 1, "seat {p}: one disclosure"));
        cite!(["701.40b"]; case: p; assert_eq!(shown[0].kind, Kind::Revealed));
        cite!(["701.40b"]; assert_eq!(shown[0].by, 0, "shown by its controller (701.40b)"));
        assert_eq!(shown[0].cards[0].name, "Grizzly Bears");
        assert_eq!(shown[0].cards[0].object, o);
    }
}

/// Soul Summons in hand, Llanowar Elves on top of the library, and a mana creature whose
/// mana ability asks which color it adds as it resolves, so a payment with it waits.
fn suspending() -> (T, ObjId) {
    let mana = CardDef::new("Mana creature", "{0}", "Creature", "{T}: Put a +1/+1 counter on this creature. Add X mana of any one color, where X is the number of +1/+1 counters on this creature.", Some((1, 1))).unwrap();
    game::play::check(&mana).unwrap();
    let mut g = Game::new(vec![oracle("Soul Summons"), oracle("Llanowar Elves"), mana], 2, 20);
    g.put(0, 1, Zone::Library);
    g.put(0, 0, Zone::Hand);
    let source = g.put(0, 2, Zone::Battlefield);
    (T::new(g), source)
}

#[test]
fn r701_40b_the_disclosure_survives_a_suspended_payment_and_a_clone() {
    let (mut t, source) = suspending();
    let o = summon(&mut t, 0);
    for p in 0..2 { t.g.take_observations(p); }
    let plan = vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source, ability: 0, paying: vec![], adding: vec![] }];
    t.g.act(0, crate::support::paid("G", Move::WithMana(plan, Box::new(Move::TurnFaceUp(o, Way::Manifested))))).unwrap();
    let shown = |g: &Game, p: usize| g.observations(p).iter().filter(|d| d.cards.iter().any(|c| c.name == "Llanowar Elves")).count();
    assert!(t.g.decision.is_some(), "waiting for the color");
    cite!(["701.40b"]; assert!(t.g.face_down(o), "not turned up before it is paid for"));
    cite!(["701.40b"]; assert_eq!(shown(&t.g, 1), 1, "announced before the payment question"));
    let mut copy = t.g.clone();
    for g in [&mut t.g, &mut copy] {
        g.act(0, Move::Choose(Answer::Mana(vec![ir::mana::ManaSymbol::Color(ir::Color::Green)]))).unwrap();
        assert!(!g.face_down(o));
        cite!(["701.40b"]; assert_eq!(g.obj(o).chars.name, "Llanowar Elves"));
        assert_eq!((shown(g, 0), shown(g, 1)), (1, 1), "recorded once, not replayed on resuming");
    }
}

#[test]
fn r733_1_a_reversed_turn_up_keeps_what_was_shown() {
    let (mut t, source) = suspending();
    let o = summon(&mut t, 0);
    for p in 0..2 { t.g.take_observations(p); }
    let plan = vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source, ability: 0, paying: vec![], adding: vec![] }];
    t.g.act(0, crate::support::paid("G", Move::WithMana(plan, Box::new(Move::TurnFaceUp(o, Way::Manifested))))).unwrap();
    // blue does not pay {G}: the payment cannot finish, and its controller reverses it
    t.g.act(0, Move::Choose(Answer::Mana(vec![ir::mana::ManaSymbol::Color(ir::Color::Blue)]))).unwrap();
    assert!(matches!(t.g.decision, Some(Decision::Payment { .. })), "{:?}", t.g.decision);
    t.g.act(0, Move::ReversePayment).unwrap();
    cite!(["733.1"]; assert!(t.g.face_down(o), "not turned up"));
    assert_eq!(t.g.observations(1).len(), 1, "what was shown stays shown");
    assert_eq!(t.g.observations(1)[0].cards[0].name, "Llanowar Elves");
}

#[test]
fn r701_40b_not_offered_for_a_land_or_a_card_with_no_mana_cost_or_to_another_player() {
    for (top, refusal) in [
        ("Forest", Some("only a creature card is turned face up for its mana cost (701.40b, 701.58b)")),
        ("Dryad Arbor", Some("a card with no mana cost can't be turned face up for it (701.40b, 701.58b)")),
        ("Ornithopter", None),
    ] {
        let mut t = Board::new(2).hand(0, &["Soul Summons"]).library(0, &[top]).build();
        let o = summon(&mut t, 0);
        t.mana(0, "{5}");
        if let Some(why) = refusal {
            let before=format!("{:?}",t.g);
            cite!(["701.40b"]; case: top; refused(why): t.g.act(0, Move::TurnFaceUp(o, Way::Manifested)));
            assert_eq!(format!("{:?}",t.g),before);
        } else {
            cite!(["701.40b"]; case: top; ok: t.g.act(0, Move::TurnFaceUp(o, Way::Manifested)));
            cite!(["708.8"]; assert!(!t.g.face_down(o) && t.g.obj(o).chars.name=="Ornithopter"));
        }
    }
    // the other player has priority, and does not control it
    let mut t = Board::new(2).hand(0, &["Soul Summons"]).library(0, &["Grizzly Bears"]).build();
    let o = summon(&mut t, 0);
    t.g.act(0, Move::Pass).unwrap();
    t.mana(1, "{5}{G}");
    let before=format!("{:?}",t.g);
    cite!(["701.40b", "116.2b"]; refused("only its controller turns a face-down permanent face up (116.2b)"): t.g.act(1,crate::support::paid("CG", Move::TurnFaceUp(o, Way::Manifested))));
    assert_eq!(format!("{:?}",t.g),before);
}

#[test]
fn r701_40b_offered_after_it_loses_all_abilities_and_whatever_type_it_has_now() {
    // Lignify: a 0/4 Treefolk with no abilities; the card is still a creature card
    let mut t = Board::new(2).hand(0, &["Soul Summons", "Lignify"]).library(0, &["Grizzly Bears"]).build();
    let o = summon(&mut t, 0);
    t.mana(0, "{1}{G}");
    t.cast(0, "Lignify", vec![Tgt::Object(o)], crate::support::mana_pool("CG")).unwrap();
    t.resolve();
    assert_eq!(t.g.toughness(o), Some(4));
    t.mana(0, "{1}{G}");
    t.g.act(0, crate::support::paid("CG", Move::TurnFaceUp(o, Way::Manifested))).unwrap();
    cite!(["701.40b"]; assert_eq!(t.g.obj(o).chars.name, "Grizzly Bears"));
    cite!(["708.8"]; assert_eq!((t.g.power(o), t.g.toughness(o)), (Some(0), Some(4)), "Lignify still applies (708.8)"));
}

#[test]
fn r701_40b_x_is_zero() {
    let x = CardDef::new("Xling", "{X}{G}", "Creature — Elf", "", Some((1, 1))).unwrap();
    game::play::check(&x).unwrap();
    let mut g = Game::new(vec![oracle("Soul Summons"), x], 2, 20);
    g.put(0, 1, Zone::Library);
    g.put(0, 0, Zone::Hand);
    let mut t = T::new(g);
    let o = summon(&mut t, 0);
    let before=format!("{:?}",t.g);
    cite!(["701.40b","118.3a"]; refused("that mana is not in your mana pool"): t.g.act(0,crate::support::paid("G", Move::TurnFaceUp(o, Way::Manifested))));
    assert_eq!(format!("{:?}",t.g),before);
    t.mana(0, "{G}");
    t.g.act(0, crate::support::paid("G", Move::TurnFaceUp(o, Way::Manifested))).unwrap();
    cite!(["701.40b", "107.3g"]; assert_eq!(t.g.obj(o).chars.name, "Xling"));
    cite!(["107.3g"]; assert_eq!(t.g.players[0].pool.total(), 0));
}

#[test]
fn r708_8_turning_face_up_is_not_entering() {
    // Elvish Visionary draws as it enters; Soul Warden watches creatures enter
    let mut t = Board::new(2).hand(0, &["Soul Summons"]).battlefield(0, &["Soul Warden"]).library(0, &["Elvish Visionary", "Forest"]).build();
    let o = summon(&mut t, 0);
    cite!(["603.6a"]; assert_eq!(t.g.players[0].life, 21, "Soul Warden saw a creature enter"));
    cite!(["708.3"]; assert_eq!(t.g.players[0].hand.len(), 0, "its own enters ability does not exist face down (708.3)"));
    t.mana(0, "{1}{G}");
    t.g.act(0, crate::support::paid("CG", Move::TurnFaceUp(o, Way::Manifested))).unwrap();
    t.resolve();
    cite!(["708.8"]; assert_eq!(t.g.players[0].life, 21, "nothing entered"));
    cite!(["708.8"]; assert_eq!(t.g.players[0].hand.len(), 0, "no enters trigger on turning face up (708.8)"));
}

#[test]
fn r708_3_its_own_enters_trigger_does_not_fire_and_another_sees_a_nameless_2_2() {
    let mut t = Board::new(2).hand(0, &["Soul Summons"]).battlefield(1, &["Soul Warden"]).library(0, &["Elvish Visionary", "Forest"]).build();
    summon(&mut t, 0);
    cite!(["603.6a"]; assert_eq!(t.g.players[1].life, 21, "an opponent's Soul Warden sees a creature enter"));
    cite!(["708.3"]; assert!(t.g.players[0].hand.is_empty()));
}

#[test]
fn r701_40f_a_prohibited_manifest_leaves_the_card_on_top() {
    // Grafdigger's Cage: creature cards in libraries can't enter the battlefield. The face-down
    // object is a creature card wherever its card is.
    let mut t = Board::new(2).hand(0, &["Soul Summons"]).battlefield(1, &["Grafdigger's Cage"]).library(0, &["Forest", "Wastes"]).build();
    t.mana(0, "{1}{W}");
    t.cast(0, "Soul Summons", vec![], crate::support::mana_pool("CW")).unwrap();
    t.resolve();
    cite!(["701.40f"]; assert!(face_down(&t).is_empty(), "nothing manifested"));
    t.expect().zone_ordered(0, Zone::Library, &["Forest", "Wastes"]).check();
    cite!(["701.40f"]; assert!(t.g.obj(t.g.players[0].library[1]).face == game::face::Face::Up));
}

#[test]
fn break_open_turns_up_a_manifested_creature_without_paying() {
    let mut t = Board::new(2).hand(0, &["Soul Summons"]).hand(1, &["Break Open"]).library(0, &["Shivan Dragon"]).build();
    let o = summon(&mut t, 0);
    t.g.act(0, Move::Pass).unwrap();
    t.mana(1, "{1}{R}");
    let from = t.mark();
    t.cast(1, "Break Open", vec![Tgt::Object(o)], crate::support::mana_pool("CR")).unwrap();
    t.resolve();
    assert!(!t.g.face_down(o));
    cite!(["708.8"]; assert_eq!(t.g.obj(o).chars.name, "Shivan Dragon"));
    cite!(["708.8"]; assert!(t.g.has(o, "Flying")));
    assert_eq!(turned_up_events(&t.g, from), 1);
    let shown: Vec<_> = t.g.observations(1).iter().filter(|d| d.kind == Kind::Revealed).collect();
    assert!(shown.iter().any(|d| d.cards[0].name == "Shivan Dragon" && d.by == 0), "revealed as it turned up: {shown:?}");
}

#[test]
fn break_open_turns_a_manifested_land_into_a_noncreature_land() {
    let mut t = Board::new(2).hand(0, &["Soul Summons"]).hand(1, &["Break Open"]).library(0, &["Forest"]).build();
    let o = summon(&mut t, 0);
    t.g.act(0, Move::Pass).unwrap();
    t.mana(1, "{1}{R}");
    t.cast(1, "Break Open", vec![Tgt::Object(o)], crate::support::mana_pool("CR")).unwrap();
    t.resolve();
    assert!(!t.g.face_down(o));
    cite!(["708.8"]; assert_eq!(t.g.seen(o, None).types, vec![ir::CardType::Land]));
    cite!(["708.8"]; assert_eq!(t.g.power(o), None));
}

#[test]
fn r701_40g_break_open_reveals_a_manifested_instant_which_stays_face_down() {
    let mut t = Board::new(2).hand(0, &["Soul Summons"]).hand(1, &["Break Open"]).library(0, &["Opt"]).build();
    let o = summon(&mut t, 0);
    for p in 0..2 { t.g.take_observations(p); }
    t.g.act(0, Move::Pass).unwrap();
    t.mana(1, "{1}{R}");
    let from = t.mark();
    t.cast(1, "Break Open", vec![Tgt::Object(o)], crate::support::mana_pool("CR")).unwrap();
    t.resolve();
    cite!(["701.40g"]; assert!(t.g.face_down(o), "it stays face down"));
    assert_eq!(t.g.obj(o).chars.name, "");
    cite!(["701.40g"]; assert_eq!(turned_up_events(&t.g, from), 0, "it was not turned face up"));
    for p in 0..2 {
        let shown = t.g.observations(p);
        assert_eq!(shown.len(), 1, "seat {p}: {shown:?}");
        cite!(["701.40g"]; case: p; assert_eq!((shown[0].kind, shown[0].by, shown[0].cards[0].name.as_str()), (Kind::Revealed, 0, "Opt"), "its controller reveals it"));
    }
    assert!(t.g.log.since(from).iter().any(|e| matches!(e, Event::Revealed { player: 0, cards } if cards == &vec![o])));
}

#[test]
fn break_open_cannot_target_your_own_face_down_creature_or_a_face_up_one() {
    let mut t = Board::new(2).hand(0, &["Soul Summons", "Break Open"]).battlefield(1, &["Grizzly Bears"]).library(0, &["Shivan Dragon"]).build();
    let o = summon(&mut t, 0);
    t.mana(0, "{1}{R}");
    cite!(["115.1"]; refused("an illegal target, an object with no name: the target is one an opponent controls, and you control it (115.1)"): t.cast(0, "Break Open", vec![Tgt::Object(o)], crate::support::mana_pool("CR")));
    let bears = t.find_in(1, Zone::Battlefield, "Grizzly Bears");
    cite!(["115.1"]; refused("an illegal target, Grizzly Bears: it isn't face down (115.1)"): t.cast(0, "Break Open", vec![Tgt::Object(bears)], crate::support::mana_pool("CR")));
}

#[test]
fn r613_7f_formless_nurturing_puts_its_counter_on_the_manifested_permanent() {
    let mut t = Board::new(2).hand(0, &["Formless Nurturing"]).library(0, &["Grizzly Bears"]).build();
    t.mana(0, "{3}{G}");
    t.cast(0, "Formless Nurturing", vec![], crate::support::mana_pool("CCCG")).unwrap();
    t.resolve();
    let o = only_face_down(&t);
    cite!(["608.2c"]; assert_eq!(t.g.obj(o).counters.get("+1/+1").copied(), Some(1)));
    cite!(["122.1a"]; assert_eq!((t.g.power(o), t.g.toughness(o)), (Some(3), Some(3))));
}

#[test]
fn reality_shift_its_controller_manifests_and_the_caster_does_not_see_it() {
    let mut t = Board::new(2).hand(0, &["Reality Shift"]).battlefield(1, &["Grizzly Bears"]).library(1, &["Shivan Dragon"]).build();
    t.mana(0, "{1}{U}");
    t.cast(0, "Reality Shift", vec![on(1, Zone::Battlefield, "Grizzly Bears")], crate::support::mana_pool("CU")).unwrap();
    t.resolve();
    let o = only_face_down(&t);
    cite!(["608.2h", "110.2a"]; assert_eq!(t.g.controller(o), 1, "the exiled creature's controller manifests"));
    assert!(t.g.exile.iter().any(|c| t.g.obj(*c).chars.name == "Grizzly Bears"));
    cite!(["708.5"]; assert!(t.g.sees(1, o) && !t.g.sees(0, o)));
    assert!(t.g.perceives(0, o));
    cite!(["708.5"]; assert!(t.g.observations(0).iter().all(|d| d.cards.iter().all(|c| c.name != "Shivan Dragon")), "the caster learned nothing"));
    cite!(["708.5"]; assert!(t.g.observations(1).iter().any(|d| d.kind == Kind::Looked && d.cards[0].name == "Shivan Dragon")));
}
