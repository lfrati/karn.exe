//! Adventure completion is a rule replacement (715.3d, 608.2n), competing with
//! graveyard replacements under 616.1. Only its own exile grants the permission.
use crate::support::{Board, T, cite, mana_pool, paid};
use game::card::CastAs;
use game::state::{Answer, Choice, Decision, DestinationEffect, PublicChoice};
use game::{Move, ObjId, Zone};

const GIANT: &str = "Grabby Giant // That's Mine";

fn cast(t: &mut T) -> ObjId {
    let card = t.find_in(0, Zone::Hand, "adventurer");
    t.g.players[0].pool = mana_pool("CR");
    t.g.act(0, paid("CR", Move::CastAs(CastAs::Adventure, Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    let spell = t.g.stack[0];
    t.pass_all();
    spell
}

fn board(replacement: Option<(usize, &str)>) -> T {
    let mut b = Board::new(2).named(0, Zone::Hand, "adventurer", GIANT).hand(1, &["Counterspell"]);
    if let Some((player, source)) = replacement { b = b.battlefield(player, &[source]); }
    b.build()
}

#[test]
fn r715_3d_the_controller_chooses_adventure_or_a_competing_graveyard_replacement() {
    for source in [(0, "Rest in Peace"), (1, "Leyline of the Void"), (1, "Dryad Militant")] {
        let mut t = board(Some(source));
        let spell = cast(&mut t);
        let Some(Decision::Choose { player: 0, choice: Choice::SpellDestination { on, among } }) = t.g.decision.clone()
            else { panic!("{:?}", t.g.decision) };
        cite!(["616.1", "608.2n", "715.3d"]; assert_eq!((on, among.len()), (spell, 2)));
        let adventure = *among.iter().find(|e| e.name() == "adventure").expect("Adventure is a competing rule replacement");
        let ordinary = *among.iter().find(|e| matches!(e, DestinationEffect::GraveyardReplacement(_))).unwrap();
        let mut rip = T::new(t.g.clone());
        for (t, effect, allowed) in [(&mut t, adventure, true), (&mut rip, ordinary, false)] {
            let before = format!("{:?}", t.g);
            cite!(["616.1"]; refused("that decision is another player's"):
                t.answer(1, Answer::SpellDestination { on: spell, effect }));
            cite!(["616.1"]; refused("choose an offered destination for this resolving spell (616.1)"):
                t.answer(0, Answer::SpellDestination { on: spell, effect: DestinationEffect::Buyback }));
            assert_eq!(format!("{:?}", t.g), before);
            t.answer(0, Answer::SpellDestination { on: spell, effect }).unwrap();
            let exiled = t.g.exile[0];
            cite!(["715.3d"]; assert_eq!(t.g.permission(exiled), allowed.then_some((0, true))));
            cite!(["616.1"]; assert!(game::snapshot::Snapshot::of(&t.g, Some(1)).choices_made.contains(
                &PublicChoice::SpellDestination { player: 0, on: spell, effect })));
            assert_eq!(t.g.battlefield.iter().filter(|o| t.g.obj(**o).token()).count(), 1, "resolution is not replayed");
            t.g.players[0].pool = mana_pool("CCCR");
            let action = paid("CCCR", Move::Cast { card: exiled, targets: vec![] });
            if allowed {
                cite!(["715.3d"]; ok: t.g.act(0, action));
                assert!(game::snapshot::Snapshot::of(&t.g, Some(1)).choices_made.is_empty(), "the next accepted move clears completed output");
            } else {
                cite!(["601.3", "715.3d"]; refused("that card is not in your hand: it is in exile, and no rule or effect lets you cast it from there (601.3)"):
                    t.g.act(0, action));
                assert!(game::snapshot::Snapshot::of(&t.g, Some(1)).choices_made.contains(
                    &PublicChoice::SpellDestination { player: 0, on: spell, effect }), "a refusal preserves public output");
                t.g.act(0, Move::Pass).unwrap();
                assert!(game::snapshot::Snapshot::of(&t.g, Some(1)).choices_made.is_empty());
            }
        }
    }
}

#[test]
fn r715_3d_without_a_competing_replacement_adventure_grants_permission_without_a_question() {
    let mut t = board(None);
    cast(&mut t);
    cite!(["715.3d"]; assert!(t.g.decision.is_none() && t.g.stack.is_empty()));
    cite!(["715.3d"]; assert_eq!(t.g.permission(t.g.exile[0]), Some((0, true))));
    cite!(["616.1"]; assert!(game::snapshot::Snapshot::of(&t.g, Some(1)).choices_made.is_empty()));
}

#[test]
fn r715_3d_a_countered_adventure_under_rest_in_peace_grants_no_permission() {
    let mut t = board(Some((0, "Rest in Peace")));
    let card = t.find_in(0, Zone::Hand, "adventurer");
    t.g.players[0].pool = mana_pool("CR");
    t.g.act(0, paid("CR", Move::CastAs(CastAs::Adventure, Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    let spell = t.g.stack[0];
    t.g.act(0, Move::Pass).unwrap();
    let counter = t.find_in(1, Zone::Hand, "Counterspell");
    t.g.players[1].pool = mana_pool("UU");
    t.g.act(1, paid("UU", Move::Cast { card: counter, targets: vec![vec![game::Target::Object(spell)]] })).unwrap();
    t.pass_all();
    cite!(["701.6a", "715.3d"]; assert!(t.g.stack.is_empty() && t.g.decision.is_none()));
    cite!(["715.3d"]; assert!(t.g.exile.iter().all(|o| t.g.permission(*o).is_none())));
    assert_eq!(t.g.exile.len(), 2, "Rest in Peace exiles both spells");
    assert_eq!(t.g.battlefield.iter().filter(|o| t.g.obj(**o).token()).count(), 0, "the Adventure did not resolve");
}

// Authored mechanism fixture: an Adventure alternative explicitly prints flashback.
// Both orders exile, but only choosing the Adventure replacement grants permission.
#[test]
fn r715_3d_adventure_and_flashback_compete_even_though_both_orders_exile() {
    let main = game::CardDef::new("Traveler", "{0}", "Creature — Human", "", Some((1, 1))).unwrap();
    let alternative = game::CardDef::new("Trip", "{0}", "Instant — Adventure", "Flashback {0}\nYou gain 1 life.", None).unwrap();
    let def = game::CardDef::adventurer(main, alternative).unwrap();
    game::play::check(&def).unwrap();
    for effect in [DestinationEffect::Adventure, DestinationEffect::Flashback] {
        let mut g = game::Game::new(vec![def.clone()], 2, 20);
        let card = g.put(0, 0, Zone::Graveyard);
        g.act(0, Move::CastVia(game::announcement::CastRoute::Flashback,
            Box::new(Move::CastAs(CastAs::Adventure, Box::new(Move::Cast { card, targets: vec![] }))))).unwrap();
        let spell = g.stack[0];
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        cite!(["616.1", "715.3d", "702.34a"]; assert!(matches!(&g.decision,
            Some(Decision::Choose { player: 0, choice: Choice::SpellDestination { among, .. } })
                if *among == vec![DestinationEffect::Adventure, DestinationEffect::Flashback])));
        g.act(0, Move::Choose(Answer::SpellDestination { on: spell, effect })).unwrap();
        cite!(["715.3d", "702.34a"]; assert_eq!(g.permission(g.exile[0]),
            (effect == DestinationEffect::Adventure).then_some((0, true))));
        assert_eq!(g.players[0].life, 21, "the Adventure body resolves once");
    }
}
