//! Planetarium's resolution-only library cast and limited action (608.2g, 603.2h).
use crate::support::{cite, paid};
use game::legal::Asked;
use game::state::Answer;
use game::{CardDef, Game, Move, ObjId, Zone};
use std::sync::LazyLock;
static FACES: LazyLock<Vec<oracle::corpus::Face>> = LazyLock::new(|| oracle::corpus::load(&oracle::corpus::default_path()).unwrap());
fn game() -> (Game, ObjId) {
    let real = CardDef::from_faces(&CardDef::catalog(&FACES)["Planetarium of Wan Shi Tong"]).unwrap();
    let defs = vec![real,
        CardDef::new("Ponder once", "{0}", "Instant", "Scry 1.", None).unwrap(),
        CardDef::new("Ponder twice", "{0}", "Instant", "Scry 1. Scry 1.", None).unwrap(),
        CardDef::new("Bear", "{2}{G}", "Creature — Bear", "", Some((2,2))).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Offering", "{4}{G}", "Creature — Bear", "As an additional cost to cast this spell, sacrifice a creature.", Some((3,3))).unwrap(),
        CardDef::new("Variable", "{X}{G}", "Sorcery", "You gain X life.", None).unwrap(),
        CardDef::new("Library mirror", "{0}", "Instant", "Look at the top card of your library. Copy that card. You may cast the copy without paying its mana cost.", None).unwrap(),
        CardDef::new("Counter", "{0}", "Instant", "Counter target spell.", None).unwrap(),
        CardDef::from_faces(&CardDef::catalog(&FACES)["Shiko, Paragon of the Way"]).unwrap(),
        CardDef::from_faces(&CardDef::catalog(&FACES)["Reenact the Crime"]).unwrap(),
        CardDef::new("Exile prohibition", "{0}", "Artifact", "Cards in your graveyard can't be exiled.", None).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap_or_else(|error| panic!("{}: {}", def.name(), error.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..8 { g.put(p, 4, Zone::Library); } }
    let source = g.put(0, 0, Zone::Battlefield);
    (g, source)
}
fn scry(g: &mut Game, twice: bool) {
    let card = g.put(0, if twice { 2 } else { 1 }, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
}
fn drive(g: &mut Game, keep: &[ObjId], order: Option<(ObjId, &[usize])>) {
    for _ in 0..60 {
        match g.asked() {
            Some((player, Asked::Arrange { cards, verb: ir::Verb::Scry, .. })) => {
                assert_eq!(player,0); assert_eq!(cards,keep);
                g.act(0,Move::Choose(Answer::Arrange{top:keep.to_vec(),away:vec![]})).unwrap();
            },
            Some((player, Asked::Order(abilities))) => {
                let (source,order)=order.expect("the caller authored a trigger order");
                assert_eq!(player,0);
                assert_eq!(abilities,vec![(Some(source),game::binding::AbilityKey::printed(1));2]);
                assert_eq!(order,&[0,1]);
                g.act(0,Move::Order(order.to_vec())).unwrap();
            },
            Some((player, Asked::Priority)) if !g.stack.is_empty() => g.act(player, Move::Pass).unwrap(),
            Some((_,Asked::Cast { .. } | Asked::Targets { .. } | Asked::Priority)) => return,
            other => panic!("unexpected library resolution question: {other:?}"),
        }
    }
    panic!("resolution did not finish");
}
fn offer(g: &mut Game, card: ObjId) {
    assert!(matches!(g.asked(), Some((0, Asked::Cast { cards, .. })) if cards == vec![card]), "{:?}", g.asked());
}
fn cast(g: &mut Game, card: ObjId) { g.act(0, Move::Cast { card, targets: vec![] }).unwrap(); }
#[test]
fn r608_2g_actual_planetarium_casts_creature_off_turn_and_look_is_private() {
    let (mut g, source) = game();
    while g.active == 0 || g.priority != 0 { g.act(g.priority, Move::Pass).unwrap(); }
    let top = g.put(0, 3, Zone::Library);
    scry(&mut g, false);
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    let Some((player, Asked::Arrange { cards, .. })) = g.asked() else { panic!("expected scry"); };
    assert_eq!(player,0); assert_eq!(cards,vec![top]);
    g.act(0, Move::Choose(Answer::Arrange { top: vec![top], away: vec![] })).unwrap();
    assert_eq!(g.stack.len(), 1, "Planetarium trigger is already on the stack");
    // Removing the source does not remove its already-triggered instructions (113.7a).
    g.move_object(source, Zone::Graveyard);
    drive(&mut g, &[], None);
    offer(&mut g, top);
    cite!(["701.20e", "401.2"]; assert!(g.sees(0, top) && !g.sees(1, top)));
    assert_eq!(g.active, 1);
    cast(&mut g, top);
    cite!(["608.2g", "601.2i"]; assert_eq!(g.stack.len(), 1));
    assert!(!g.players[0].library.contains(&top));
    assert!(g.players[0].pool.colorless == 0 && g.players[0].pool.colors == [0; 5]);
    drive(&mut g, &[], None);
    assert!(g.battlefield.iter().any(|id| g.obj(*id).chars.name == "Bear"));
}
#[test]
fn r603_2h_two_queued_instances_decline_preserves_action_success_consumes_it() {
    for (decline_first, counter) in [(false, false), (true, false), (false, true)] {
        let (mut g, planetarium) = game();
        let lower = g.put(0, 3, Zone::Library);
        let top = g.put(0, 3, Zone::Library);
        scry(&mut g, true); drive(&mut g, &[top], Some((planetarium,&[0,1]))); offer(&mut g, top);
        if decline_first {
            g.act(0, Move::Choose(Answer::No)).unwrap(); drive(&mut g, &[], None); offer(&mut g, top);
        }
        cast(&mut g, top);
        if counter {
            let spell = *g.stack.last().unwrap();
            g.act(g.priority, Move::Pass).unwrap();
            let answer = g.put(1, 8, Zone::Hand);
            g.act(1, Move::Cast { card: answer, targets: vec![vec![game::Target::Object(spell)]] }).unwrap();
        }
        drive(&mut g, &[], None);
        cite!(["603.2h", "608.2g"]; assert!(g.stack.is_empty() && g.decision.is_none()));
        assert!(g.players[0].library.contains(&lower));
        assert_eq!(g.battlefield.iter().filter(|id| g.obj(**id).chars.name == "Bear").count(), if counter { 0 } else { 1 });
        if counter { assert!(g.players[0].graveyard.iter().any(|id| g.obj(*id).chars.name == "Bear")); }
        scry(&mut g, false); drive(&mut g, &[lower], None);
        assert!(g.stack.is_empty() && g.decision.is_none(), "successful cast also stops later triggers");
    }
}
#[test]
fn r603_2h_land_top_does_not_consume_action_and_declined_look_does_not_persist() {
    let (mut g, _) = game();
    let land=*g.players[0].library.last().unwrap(); // last declared Forest in game()
    scry(&mut g, false); drive(&mut g, &[land], None);
    cite!(["305.9", "603.2h"]; assert!(g.stack.is_empty() && g.decision.is_none()));
    let top = g.put(0, 3, Zone::Library);
    scry(&mut g, false); drive(&mut g, &[top], None); offer(&mut g, top);
    g.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["701.20a", "701.20e"]; assert!(!g.sees(0, top) && !g.sees(1, top)));
    assert!(g.players[0].library.contains(&top));
    cite!(["608.2g","601.3"]; refused("it is in a library, and no rule or effect lets you cast it from there"): act_unchanged(&mut g,Move::Cast { card: top, targets: vec![] }));
    scry(&mut g, false); drive(&mut g, &[top], None); offer(&mut g, top);
}
#[test]
fn r733_1_failed_additional_cost_keeps_offer_and_once_action_available() {
    let (mut g, _) = game();
    let top = g.put(0, 5, Zone::Library);
    scry(&mut g, false); drive(&mut g, &[top], None); offer(&mut g, top);
    let before = format!("{g:?}");
    cite!(["118.9d", "733.1"]; refused("that selection does not satisfy the cost, which asks for exactly 1 card(s): 0 chosen"): g.act(0, Move::Cast { card: top, targets: vec![] }));
    assert_eq!(format!("{g:?}"), before);
    g.act(0, Move::Choose(Answer::No)).unwrap();
    let bear = g.put(0, 3, Zone::Battlefield);
    scry(&mut g, false); drive(&mut g, &[top], None); offer(&mut g, top);
    g.act(0, Move::WithCosts(vec![vec![bear]], Box::new(Move::Cast { card: top, targets: vec![] }))).unwrap();
    drive(&mut g, &[], None);
    assert!(!g.battlefield.contains(&bear));
    assert!(g.battlefield.iter().any(|id| g.obj(*id).chars.name == "Offering"));
}
#[test]
fn r107_3b_library_free_cast_has_zero_x() {
    let (mut g, _) = game();
    let top = g.put(0, 6, Zone::Library);
    scry(&mut g, false); drive(&mut g, &[top], None); offer(&mut g, top);
    cast(&mut g, top); drive(&mut g, &[], None);
    cite!(["107.3b", "118.9a"]; assert_eq!(g.players[0].life, 20));
    assert!(g.players[0].graveyard.iter().any(|id| g.obj(*id).chars.name == "Variable"));
}

#[test]
fn r707_12_a_card_copy_inherits_library_origin_and_does_not_cast_the_original() {
    let (mut g, _) = game();
    let top = g.put(0, 3, Zone::Library);
    let mirror = g.put(0, 7, Zone::Hand);
    cast(&mut g, mirror); drive(&mut g, &[], None);
    let Some((0, Asked::Cast { cards, .. })) = g.asked() else { panic!("expected card-copy cast"); };
    assert_eq!(cards.len(), 1);
    let copy = cards[0];
    cite!(["707.12"]; assert_ne!(copy, top));
    assert_eq!(g.obj(copy).zone, Zone::Library);
    assert!(!g.sees(1, top));
    cast(&mut g, copy); drive(&mut g, &[], None);
    assert!(g.players[0].library.contains(&top));
    assert!(g.battlefield.iter().any(|id| g.obj(*id).chars.name == "Bear"));
}
#[test]
fn r400_7_a_hidden_return_cannot_replace_the_exact_inspected_incarnation() {
    let (mut g, _) = game();
    let top = g.put(0, 3, Zone::Library);
    scry(&mut g, false); drive(&mut g, &[top], None); offer(&mut g, top);
    // Unit-level transition injection exercises permission identity independently of
    // legal response windows; there is no priority in this resolution (608.2g).
    g.move_object(top, Zone::Graveyard);
    let departed = *g.players[0].graveyard.last().unwrap();
    g.move_object(departed, Zone::Library);
    let returned = *g.players[0].library.last().unwrap();
    assert_ne!(top, returned);
    // The returned card is a new object; the old permission does not let us cast it.
    // Check that specific refusal: an unrelated error must not make this test pass.
    cite!(["400.7", "608.2g"]; refused("the effect offers other cards to cast (608.2g)"):
        g.act(0, Move::Cast { card: returned, targets: vec![] }));
    g.act(0, Move::Choose(Answer::No)).unwrap();
    assert!(g.players[0].library.contains(&returned));
    assert!(g.stack.is_empty());
}

#[test]
fn r400_7j_unproved_hidden_movement_and_occurrence_inputs_refuse_cast_origins() {
    for text in [
        "Return target creature to its owner's hand. You may cast that card without paying its mana cost.",
        "Whenever a creature dies, you may cast that card without paying its mana cost.",
        "Exile target card from a graveyard. Return that card to its owner's hand. You may cast that card without paying its mana cost.",
    ] {
        let def = CardDef::new("Unproved cast", "{0}", if text.starts_with("Whenever") { "Artifact" } else { "Instant" }, text, None).unwrap();
        let error = game::play::check(&def).unwrap_err();
        assert!(error.0.contains("no fixed source zone supplied by its card producer"), "{error:?}");
    }
}

#[test]
fn r707_10_copying_a_spell_does_not_produce_a_card_to_cast() {
    let def = CardDef::new("Spell mirror", "{0}", "Instant", "Copy target instant spell. You may cast the copy without paying its mana cost.", None).unwrap();
    let error = game::play::check(&def).unwrap_err();
    assert!(error.0.contains("no fixed source zone supplied by its card producer"), "{error:?}");
}

#[test]
fn r707_12_real_exiled_target_copy_consumers_cast_only_the_created_copy() {
    for reenact in [false, true] {
        let (mut g, _) = game();
        let target = g.put(0, 3, Zone::Hand);
        g.move_object(target, Zone::Graveyard);
        let target = *g.players[0].graveyard.last().unwrap();
        let spell = g.put(0, if reenact { 10 } else { 9 }, Zone::Hand);
        // Pinned full costs: Shiko {2}{U}{R}{W}; Reenact {1}{U}{U}{U}.
        g.players[0].pool.colorless = if reenact { 1 } else { 2 };
        g.players[0].pool.colors = if reenact { [0, 3, 0, 0, 0] } else { [1, 1, 0, 1, 0] };
        g.act(0, paid(if reenact { "CUUU" } else { "CCURW" }, Move::Cast { card: spell, targets: if reenact { vec![vec![game::Target::Object(target)]] } else { vec![] } })).unwrap();
        drive(&mut g, &[], None);
        if !reenact {
            assert!(matches!(g.asked(), Some((0, Asked::Targets { .. }))));
            g.act(0, Move::ChooseTargets(vec![vec![game::Target::Object(target)]])).unwrap();
            drive(&mut g, &[], None);
        }
        let Some((0, Asked::Cast { cards, .. })) = g.asked() else { panic!("expected copied-card cast"); };
        assert_eq!(cards.len(), 1);
        let copy = cards[0];
        cite!(["400.7j", "707.12"]; assert_ne!(copy, target));
        assert_eq!(g.obj(copy).zone, Zone::Exile);
        assert_eq!(g.exile.iter().filter(|card| g.obj(**card).chars.name == "Bear").count(), 2);
        cast(&mut g, copy); drive(&mut g, &[], None);
        assert_eq!(g.exile.iter().filter(|card| g.obj(**card).chars.name == "Bear").count(), 1);
        assert_eq!(g.battlefield.iter().filter(|card| g.obj(**card).chars.name == "Bear").count(), 1);
    }
}

#[test]
fn singular_card_copy_cannot_pick_an_unannounced_member_of_a_multiple_card_result() {
    let def = CardDef::new("Ambiguous mirror", "{0}", "Instant", "Look at the top two cards of your library. Copy that card. You may cast the copy without paying its mana cost.", None).unwrap();
    crate::support::refused(game::play::check(&def).map_err(|e|e.0),"copy: only stack spells or abilities (707.10)");
}

#[test]
fn r609_3_prevented_exile_still_copies_the_named_card_in_its_actual_graveyard() {
    for reenact in [false, true] {
        let (mut g, _) = game();
        g.put(0, 11, Zone::Battlefield);
        let target = g.put(0, 3, Zone::Hand);
        g.move_object(target, Zone::Graveyard);
        let target = *g.players[0].graveyard.last().unwrap();
        let spell = g.put(0, if reenact { 10 } else { 9 }, Zone::Hand);
        g.players[0].pool.colorless = if reenact { 1 } else { 2 };
        g.players[0].pool.colors = if reenact { [0, 3, 0, 0, 0] } else { [1, 1, 0, 1, 0] };
        g.act(0, paid(if reenact { "CUUU" } else { "CCURW" }, Move::Cast { card: spell, targets: if reenact { vec![vec![game::Target::Object(target)]] } else { vec![] } })).unwrap();
        drive(&mut g, &[], None);
        if !reenact {
            g.act(0, Move::ChooseTargets(vec![vec![game::Target::Object(target)]])).unwrap();
            drive(&mut g, &[], None);
        }
        let Some((0, Asked::Cast { cards, .. })) = g.asked() else { panic!("expected copied-card cast"); };
        assert_eq!(cards.len(), 1);
        let copy = cards[0];
        cite!(["609.3", "707.12"]; assert!(g.players[0].graveyard.contains(&target)));
        assert_ne!(copy, target);
        assert_eq!(g.obj(copy).zone, Zone::Graveyard, "copy is created where the original actually remains");
        assert!(g.exile.is_empty());
        assert!(!g.log.iter().any(|event| matches!(event, game::Event::Exiled { .. })));
        cast(&mut g, copy); drive(&mut g, &[], None);
        assert!(g.players[0].graveyard.contains(&target));
        assert_eq!(g.battlefield.iter().filter(|card| g.obj(**card).chars.name == "Bear").count(), 1);
    }
}

#[test]
fn r609_3_failed_exile_preserves_literal_reference_but_not_success_or_zone_predicate() {
    for (text, retained) in [
        ("Exile target nonland card from your graveyard. You may cast it without paying its mana cost.", true),
        ("Exile target nonland card from your graveyard. You may cast the card exiled this way without paying its mana cost.", false),
        ("Exile target nonland card from your graveyard. You may cast that card from exile without paying its mana cost.", false),
    ] {
        let defs = vec![CardDef::new("Offer", "{0}", "Instant", text, None).unwrap(),
            CardDef::new("Bear", "{2}{G}", "Creature — Bear", "", Some((2,2))).unwrap(),
            CardDef::new("Exile prohibition", "{0}", "Artifact", "Cards in your graveyard can't be exiled.", None).unwrap()];
        for def in &defs { game::play::check(def).unwrap_or_else(|error| panic!("{text}: {}", error.0)); }
        let mut g = Game::new(defs, 2, 20);
        for p in 0..2 { for _ in 0..8 { g.put(p, 1, Zone::Library); } }
        g.put(0, 2, Zone::Battlefield);
        let original = g.put(0, 1, Zone::Graveyard);
        let spell = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card: spell, targets: vec![vec![game::Target::Object(original)]] }).unwrap();
        drive(&mut g, &[], None);
        assert_eq!(g.obj(original).zone, Zone::Graveyard);
        assert!(g.exile.is_empty());
        if retained {
            offer(&mut g, original);
            cast(&mut g, original); drive(&mut g, &[], None);
            cite!(["609.3", "608.2g"]; assert!(g.battlefield.iter().any(|id| g.obj(*id).chars.name == "Bear")));
        } else {
            cite!(["609.3", "400.7j"]; assert!(g.stack.is_empty() && g.decision.is_none(), "{text}: {:?}", g.asked()));
            assert!(!g.battlefield.iter().any(|id| g.obj(*id).chars.name == "Bear"));
        }
    }
}

#[test]
fn r400_7_retained_offer_cannot_follow_an_external_exile_reincarnation() {
    let defs = vec![CardDef::new("Offer", "{0}", "Instant",
        "Exile target card from a graveyard. You may cast that card without paying its mana cost.", None).unwrap(),
        CardDef::new("Bear", "{2}{G}", "Creature — Bear", "", Some((2,2))).unwrap()];
    for def in &defs { game::play::check(def).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..8 { g.put(p, 1, Zone::Library); } }
    let original = g.put(1, 1, Zone::Graveyard);
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![vec![game::Target::Object(original)]] }).unwrap();
    drive(&mut g, &[], None);
    let offered = g.exile[0]; offer(&mut g, offered);
    // An external effect's moves do not belong to this resolution's continuity.
    g.move_object(offered, Zone::Graveyard);
    let grave = *g.players[1].graveyard.last().unwrap();
    g.move_object(grave, Zone::Exile);
    let returned = g.exile[0];
    assert_ne!(offered, returned);
    cite!(["400.7", "400.7j"]; refused("the effect offers other cards to cast"): act_unchanged(&mut g,Move::Cast { card: returned, targets: vec![] }));
    g.act(0, Move::Choose(Answer::No)).unwrap();
    assert_eq!(g.obj(returned).zone, Zone::Exile);
    assert!(g.stack.is_empty());
}

#[test]
fn completed_cast_predicates_never_fall_back_to_literal_discard_or_mill_references() {
    for text in [
        "Discard a card. You may cast the card discarded this way without paying its mana cost.",
        "Mill a card. You may cast the card milled this way without paying its mana cost.",
    ] {
        let def = CardDef::new("Unmodeled result", "{0}", "Instant", text, None).unwrap();
        let error = game::play::check(&def).unwrap_err();
        assert!(error.0.contains("cast: completed-card result predicate not modeled"), "{text}: {error:?}");
    }
}

#[test]
fn r701_22d_empty_scry_still_triggers_but_cannot_consume_planetariums_cast_action() {
    let (mut g, source) = game();
    for card in g.players[0].library.clone() { g.move_object(card, Zone::Graveyard); }
    scry(&mut g, false);
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    // An empty arrangement has only one legal outcome; the interface may omit it.
    if let Some((player, Asked::Arrange { cards, .. })) = g.asked() {
        assert!(cards.is_empty());
        g.act(player, Move::Choose(Answer::Arrange { top: vec![], away: vec![] })).unwrap();
    }
    cite!(["701.22d", "603.2"]; assert_eq!(g.stack.len(), 1));
    assert_eq!(g.obj(g.stack[0]).ability.unwrap().0, Some(source));
    drive(&mut g, &[], None);
    cite!(["608.2g", "609.3"]; assert!(g.stack.is_empty() && g.decision.is_none()));
    assert!(g.players[0].library.is_empty());
    let top = g.put(0, 3, Zone::Library);
    scry(&mut g, false); drive(&mut g, &[top], None);
    cite!(["603.2h"]; assert!(matches!(g.asked(), Some((0, Asked::Cast { cards, .. })) if cards == vec![top])));
}

#[track_caller]
fn act_unchanged(g:&mut Game,action:Move)->Result<(),String> {
 let before=format!("{g:?}");let result=g.act(0,action);assert_eq!(format!("{g:?}"),before);result
}
