//! One physical split card, chosen characteristics on the stack, and fuse (709, 702.102).
use crate::support::{cite, paid};
use game::card::CastAs;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::{CardType, Color};

fn card(name: &str, cost: &str, kind: &str, text: &str) -> CardDef {
    CardDef::new(name, cost, kind, text, None).unwrap()
}
fn toil_trouble() -> CardDef {
    CardDef::split([
        card("Toil", "{2}{B}", "Sorcery", "Target player draws two cards and loses 2 life.\nFuse"),
        card("Trouble", "{2}{R}", "Sorcery", "Trouble deals damage to target player equal to the number of cards in that player's hand.\nFuse"),
    ]).unwrap()
}
fn game(split: CardDef) -> Game {
    game::play::check(&split).unwrap();
    Game::new(vec![split, card("Forest", "", "Basic Land — Forest", "")], 2, 20)
}
fn cast(card: ObjId, choice: CastAs, targets: Vec<Vec<Target>>) -> Move {
    Move::CastAs(choice, Box::new(Move::Cast { card, targets }))
}
fn pass(g: &mut Game) {
    g.act(0, Move::Pass).unwrap(); g.act(1, Move::Pass).unwrap();
}
#[test]
fn r709_2_and_4_a_split_card_is_one_card_with_combined_characteristics_off_the_stack() {
    let mut g = game(toil_trouble());
    let c = g.put(0, 0, Zone::Library);
    assert_eq!(g.players[0].library, [c]);
    cite!(["709.4"]; assert_eq!(g.obj(c).chars.mana_value, 6));
    cite!(["709.4"]; assert_eq!(g.seen(c, None).colors, [Color::Black, Color::Red]));
    g.draw(0, 1);
    let hand = g.players[0].hand[0];
    cite!(["709.2"]; assert_eq!(g.players[0].hand, [hand]));
    assert_eq!(g.def(hand).unwrap().name(), "Toil // Trouble");
    assert_eq!(g.obj(hand).chars.mana_value, 6);
}

#[test]
fn r709_3_only_the_chosen_half_has_a_cost_and_characteristics_on_the_stack() {
    for (half, color, name) in [(0, Color::Black, "Toil"), (1, Color::Red, "Trouble")] {
        let mut g = game(toil_trouble());
        let c = g.put(0, 0, Zone::Hand);
        g.players[0].pool.colors[if half == 0 { 2 } else { 3 }] = 1;
        g.players[0].pool.colorless = 2;
        for _ in 0..3 { g.put(1, 1, Zone::Library); }
        g.act(0, paid(if half == 0 { "CCB" } else { "CCR" }, cast(c, CastAs::Half(half), vec![vec![Target::Player(1)]]))).unwrap();
        let spell = g.stack[0];
        assert_eq!(g.stack.len(), 1);
        cite!(["709.3b"]; case: name; assert_eq!(g.obj(spell).chars.name, name));
        cite!(["709.3b"]; case: name; assert_eq!(g.obj(spell).chars.mana_value, 3));
        cite!(["709.3b"]; case: name; assert_eq!(g.seen(spell, None).colors, [color]));
        assert_eq!(g.players[0].pool.colorless, 0);
        pass(&mut g);
        assert_eq!(g.players[0].graveyard.len(), 1);
        let grave = g.players[0].graveyard[0];
        cite!(["709.4"]; case: name; assert_eq!(g.obj(grave).chars.mana_value, 6));
        assert_eq!(g.obj(grave).chars.name, "Toil // Trouble");
        assert_eq!(g.seen(grave, None).colors, [Color::Black, Color::Red]);
        // The former spell keeps its last identity for events (400.7).
        assert_eq!(g.def(spell).unwrap().name(), name);
    }
}

#[test]
fn r702_102_a_fused_spell_pays_both_costs_then_resolves_left_before_right() {
    let mut g = game(toil_trouble());
    let c = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors = [0, 0, 1, 1, 0];
    g.players[0].pool.colorless = 3;
    let targets = vec![vec![Target::Player(1)], vec![Target::Player(1)]];
    let before = format!("{g:?}");
    cite!(["702.102c", "118.3a"]; refused("that mana is not in your mana pool (118.3a)"): g.act(0, paid("CCCCBR", cast(c, CastAs::Fused, targets.clone()))));
    assert_eq!(g.obj(c).zone, Zone::Hand);
    assert_eq!(format!("{g:?}"), before);
    g.players[0].pool.colorless = 4;
    for _ in 0..3 { g.put(1, 1, Zone::Hand); g.put(1, 1, Zone::Library); }
    g.act(0, paid("CCCCBR", cast(c, CastAs::Fused, targets))).unwrap();
    let spell = g.stack[0];
    cite!(["702.102b"]; assert_eq!(g.obj(spell).chars.mana_value, 6));
    assert_eq!(g.audit_game_objects()["objects"][spell.0.to_string()]["cast_as"],"fused");
    cite!(["702.102b"]; assert_eq!(g.seen(spell, None).colors, [Color::Black, Color::Red]));
    cite!(["702.102a"]; assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Cast { .. })).count(), 1));
    pass(&mut g);
    assert_eq!(g.players[1].hand.len(), 5);
    cite!(["702.102d"]; assert_eq!(g.players[1].life, 13)); // two lost to Toil, then five damage from Trouble
    assert_eq!(g.players[0].graveyard.len(), 1);
}

#[test]
fn r709_3a_timing_and_target_legality_come_from_the_chosen_half() {

    let split = CardDef::split([
        card("Quick", "{0}", "Instant", "You gain 1 life.\nFuse"),
        card("Slow", "{0}", "Sorcery", "Destroy target creature.\nFuse"),
    ]).unwrap();
    let mut g = game(split);
    std::sync::Arc::make_mut(&mut g.defs).push(CardDef::new("Bear","{0}","Creature — Bear","",Some((2,2))).unwrap());
    let target=g.put(0,2,Zone::Battlefield);
    let c = g.put(0, 0, Zone::Hand);
    cite!(["601.2c"]; refused("that has 1 target group(s), 0 given"): unchanged(&mut g,cast(c, CastAs::Half(1), vec![])));
    g.act(0, cast(c, CastAs::Half(0), vec![])).unwrap();
    assert!(g.seen(g.stack[0], None).is(CardType::Instant));
    cite!(["709.3b"]; assert!(!g.seen(g.stack[0], None).is(CardType::Sorcery)));
    let other = g.put(0, 0, Zone::Hand);
    cite!(["702.102b", "117.1a"]; refused("that is cast in a main phase of your own turn with the stack empty (117.1a)"): unchanged(&mut g,cast(other, CastAs::Fused, vec![vec![Target::Object(target)]])));
    pass(&mut g); assert_eq!(g.players[0].life,21); assert!(g.stack.is_empty());
}

#[test]
fn r702_102a_fuse_is_a_permission_of_a_split_card_in_hand() {

    let plain = CardDef::split([
        card("Left", "{0}", "Instant", "You gain 1 life."),
        card("Right", "{0}", "Instant", "You gain 2 life."),
    ]).unwrap();
    let mut g = game(plain);
    let c = g.put(0, 0, Zone::Hand);
    cite!(["702.102a"]; refused("choose one split half, or both with fuse (709.3, 702.102)"): unchanged(&mut g,cast(c, CastAs::Fused, vec![])));
    cite!(["709.3"]; refused("choose one split half, or both with fuse (709.3, 702.102)"): unchanged(&mut g,Move::Cast { card: c, targets: vec![] }));
    cite!(["709.3"]; refused("no such split half (709.3)"): unchanged(&mut g,cast(c, CastAs::Half(2), vec![])));
    crate::support::refused(game::play::check(&card("Half", "{0}", "Instant", "You gain 1 life.\nFuse")).map_err(|e|e.0),"fuse requires a whole split card (702.102a)");
    let mut g = game(toil_trouble());
    let c = g.put(0, 0, Zone::Graveyard);
    g.players[0].pool=crate::support::mana_pool("CCCCBR");
    cite!(["601.3"]; refused("that card is not in your hand: it is in a graveyard, and no rule or effect lets you cast it from there (601.3)"): unchanged(&mut g,paid("CCCCBR",cast(c, CastAs::Fused, vec![vec![Target::Player(0)],vec![Target::Player(1)]]))));
}

#[test]
fn r608_2b_fuse_is_one_spell_when_some_or_all_targets_become_illegal() {
    for both in [false, true] {
        let split = CardDef::split([
            card("Left", "{0}", "Instant", "Destroy target creature.\nFuse"),
            card("Right", "{0}", "Instant", "Destroy target creature.\nFuse"),
        ]).unwrap();
        let mut g = game(split);
        std::sync::Arc::make_mut(&mut g.defs).push(CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap());
        let a = g.put(1, 2, Zone::Battlefield);
        let b = g.put(1, 2, Zone::Battlefield);
        let c = g.put(0, 0, Zone::Hand);
        g.act(0, cast(c, CastAs::Fused, vec![vec![Target::Object(a)], vec![Target::Object(b)]])).unwrap();
        g.exile(a);
        if both { g.exile(b); }
        pass(&mut g);
        cite!(["608.2b"]; case: both; assert_eq!(g.players[1].graveyard.len(), if both { 0 } else { 1 }));
        cite!(["608.2b"]; case: both; assert_eq!(g.log.iter().any(|e| matches!(e, Event::Fizzled { .. })), both));
    }
}

#[test]
fn r106_6_restricted_mana_checks_the_chosen_half_not_the_combined_card() {
    let split = CardDef::split([
        card("Black", "{1}", "Sorcery", "You gain 1 life."),
        card("Red", "{R}", "Sorcery", "You gain 2 life."),
    ]).unwrap();
    let mut g = game(split);
    std::sync::Arc::make_mut(&mut g.defs).push(card("Source", "", "Land", "{T}: Add {R}. Spend this mana only to cast a red spell."));
    game::play::check(&g.defs[2]).unwrap();
    let source = g.put(0, 2, Zone::Battlefield);
    let c = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Activate { source: source, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    let selected = game::state::Pool { restricted: vec![g.players[0].pool.restricted[0].clone()], ..Default::default() };
    let before = format!("{g:?}");
    cite!(["106.6"]; refused("that mana can't be spent on that: its text restricts how it can be spent (106.6)"): g.act(0, Move::Paying(selected.clone(), Box::new(cast(c, CastAs::Half(0), vec![])))));
    assert_eq!(format!("{g:?}"), before);
    g.act(0, Move::Paying(selected, Box::new(cast(c, CastAs::Half(1), vec![])))).unwrap();
    pass(&mut g);
    assert_eq!(g.players[0].life, 22);
}

#[test]
fn r709_2_the_corpus_loader_requires_both_halves_in_printed_order() {

    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let pair = &catalog["Toil // Trouble"];
    assert_eq!(pair.len(), 2);
    crate::support::refused(CardDef::from_face(pair[0]),"a split card requires both halves (709.2)");
    let mut reversed = pair.clone(); reversed.reverse();
    let d = CardDef::from_faces(&reversed).unwrap();
    game::play::check(&d).unwrap();
    assert_eq!(d.halves()[0].name(), "Toil");
    assert_eq!(d.halves()[1].name(), "Trouble");
    assert_eq!(CardDef::from_faces(&catalog["Trouble"]).unwrap().name(), d.name());
}

#[test]
fn r709_3a_a_sorcery_half_prevents_fuse_at_instant_timing() {
    let split = CardDef::split([
        card("Quick", "{0}", "Instant", "You gain 1 life.\nFuse"),
        card("Slow", "{0}", "Sorcery", "You gain 2 life.\nFuse"),
    ]).unwrap();
    let mut g = game(split);
    let c = g.put(0, 0, Zone::Hand);
    g.act(0, cast(c, CastAs::Half(0), vec![])).unwrap();
    let next = g.put(0, 0, Zone::Hand);
    cite!(["702.102b", "117.1a"]; refused("that is cast in a main phase of your own turn with the stack empty (117.1a)"): g.act(0, cast(next, CastAs::Fused, vec![])));
    cite!(["709.3a", "117.1a"]; refused("that is cast in a main phase of your own turn with the stack empty (117.1a)"): g.act(0, cast(next, CastAs::Half(1), vec![])));
    g.act(0, cast(next, CastAs::Half(0), vec![])).unwrap();
}

#[test]
fn r702_102a_flashback_of_a_half_does_not_allow_fuse_from_the_graveyard() {
    let split = CardDef::split([
        card("Recall", "{B}", "Sorcery", "You gain 1 life.\nFlashback {0}\nFuse"),
        card("Rest", "{R}", "Sorcery", "You gain 2 life.\nFuse"),
    ]).unwrap();
    let mut g = game(split);
    let c = g.put(0, 0, Zone::Graveyard);
    g.players[0].pool=crate::support::mana_pool("BR");
    cite!(["702.102a", "601.3"]; refused("that card is not in your hand: it is in a graveyard, and no rule or effect lets you cast it from there (601.3)"): unchanged(&mut g,paid("BR",cast(c, CastAs::Fused, vec![]))));
    cite!(["601.3"]; refused("that card is not in your hand: it is in a graveyard, and no rule or effect lets you cast it from there (601.3)"): unchanged(&mut g,paid("R",cast(c, CastAs::Half(1), vec![]))));
    g.act(0, paid("",Move::CastVia(game::announcement::CastRoute::Flashback, Box::new(cast(c, CastAs::Half(0), vec![]))))).unwrap();
    pass(&mut g);
    assert_eq!(g.players[0].life, 21);
    assert_eq!(g.players[0].pool,crate::support::mana_pool("BR"));
    assert!(g.players[0].graveyard.is_empty());
    cite!(["702.34a"]; assert_eq!(g.exile.len(), 1));
    cite!(["709.4"]; assert_eq!(g.obj(g.exile[0]).chars.mana_value, 2));
}

#[track_caller]
fn unchanged(g:&mut Game,action:Move)->Result<(),String>{let before=format!("{g:?}");let result=g.act(0,action);assert_eq!(format!("{g:?}"),before);result}
