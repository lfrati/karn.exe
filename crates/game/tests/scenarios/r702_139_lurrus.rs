use crate::support::cite;
use game::{CardDef, Game, Move, Zone};
fn lurrus() -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let face = faces.iter().find(|f| f.name == "Lurrus of the Dream-Den").unwrap();
    let def = CardDef::from_face(face).unwrap();
    game::play::check(&def).unwrap();
    def
}
fn pass(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.decision_maker(g.priority), Move::Pass).unwrap();
    }
}
fn bear() -> CardDef {
    CardDef::new("Bear", "{2}", "Creature", "", Some((2, 2))).unwrap()
}
#[test]
fn r702_139_complete_card_companion_condition_and_once_per_turn_cast() {
    let mut g = Game::new(vec![lurrus(), bear()], 3, 20);
    let l = g.put(0, 0, Zone::Battlefield);
    assert!(g.has(l, "Lifelink"));
    let a = g.put(0, 1, Zone::Graveyard);
    let b = g.put(0, 1, Zone::Graveyard);
    g.players[0].pool.colorless = 4;
    let before = format!("{g:?}");
    cite!(["601.3"]; refused("that card is not in your hand: it is in a graveyard, and no rule or effect lets you cast it from there"): unchanged(&mut g, |g| g.act(0, crate::support::paid("CC", Move::Cast { card: a, targets: vec![] }))));
    assert_eq!(format!("{g:?}"), before);
    cite!(["601.3"]; ok: g.act(0, crate::support::paid("CC", permitted(l, a))));
    pass(&mut g);
    cite!(["601.3"]; refused("that casting permission is unavailable (601.3)"): unchanged(&mut g, |g| g.act(0, crate::support::paid("CC", permitted(l, b)))));
    let decks = vec![vec![1; 10]; 3];
    cite!(["702.139a"]; assert!(Game::start_with_companions(vec![lurrus(), bear()], &decks, &[Some(0), None, Some(0)], 20, 0, 1).is_ok()));
    let mut decks = decks;
    decks[0].push(0);
    cite!(["702.139a"]; refused("starting deck does not meet companion condition: Lurrus of the Dream-Den"): Game::start_with_companions(vec![lurrus(), bear()], &decks, &[Some(0), None, None], 20, 0, 1));
    decks[0].pop();
    decks[0].push(2);
    let large = CardDef::new("Large Bear", "{3}", "Creature", "", Some((3, 3))).unwrap();
    cite!(["702.139a"]; refused("starting deck does not meet companion condition: Large Bear"): Game::start_with_companions(vec![lurrus(), bear(), large], &decks, &[Some(0), None, None], 20, 0, 1));
}
fn permitted(source: game::ObjId, card: game::ObjId) -> Move {
    Move::CastVia(game::announcement::CastRoute::Permission { source, ability: 2 },
        Box::new(Move::Cast { card, targets: vec![] }))
}
use game::announcement::CastRoute;
use game::state::Decision;
use game::{ObjId, Target};
fn next(g: &mut Game) {
    let turn = g.turn;
    for _ in 0..160 {
        if g.turn != turn {
            return;
        }
        let m = match &g.decision {
            Some(Decision::Attackers { .. }) => Move::Attack(vec![]),
            Some(Decision::Blockers { .. }) => Move::Block(vec![]),
            Some(Decision::Discard { player, count }) => Move::Discard(g.players[*player].hand[..*count].to_vec()),
            None => Move::Pass,
            x => panic!("{x:?}"),
        };
        g.act(g.decision_maker(g.decision_actor()), m).unwrap();
    }
    panic!("turn stuck");
}
fn main(g: &mut Game) {
    while g.part != ir::Part::PrecombatMain {
        g.act(g.decision_maker(g.decision_actor()), Move::Pass).unwrap();
    }
}
fn setup() -> Game {
    let mut g = Game::new(vec![lurrus(), bear()], 3, 20);
    for p in 0..3 {
        for _ in 0..30 {
            g.put(p, 1, Zone::Library);
        }
    }
    g
}
fn add(g: &mut Game, d: CardDef) -> usize {
    game::play::check(&d).unwrap();
    let n = g.defs.len();
    std::sync::Arc::make_mut(&mut g.defs).push(d);
    n
}
fn cast_lurrus(g: &mut Game) -> ObjId {
    let c = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[0] = 3;
    g.act(0, crate::support::paid("WWW", Move::Cast { card: c, targets: vec![] })).unwrap();
    pass(g);
    *g.battlefield.last().unwrap()
}
#[test]
fn r601_3_permission_preserves_costs_timing_ownership_and_announced_mana_value() {
    let mut g = setup();
    let l = cast_lurrus(&mut g);
    let a = g.put(0, 1, Zone::Graveyard);
    let other = g.put(1, 1, Zone::Graveyard);
    let big = g.put(0, 0, Zone::Graveyard);
    let instant = add(&mut g, CardDef::new("Instant", "{0}", "Instant", "You gain 1 life.", None).unwrap());
    let instant = g.put(0, instant, Zone::Graveyard);
    let land = add(&mut g, CardDef::new("Land", "", "Land", "{T}: Add {C}.", None).unwrap());
    let land = g.put(0, land, Zone::Graveyard);
    // the permission keeps the cost: unpaid, it is refused
    cite!(["118.3a"]; refused("your mana pool cannot pay that cost (118.3a): {2}"): g.act(0, permitted(l, a)));
    for paid in [false, true] {
        g.players[0].pool.colorless = if paid { 8 } else { 0 };
        // another player's card, a greater mana value, a nonpermanent spell, a land
        for (card, why) in [(other, "that casting permission is unavailable (601.3)"), (big, "that casting permission is unavailable (601.3)"), (instant, "that casting permission is unavailable (601.3)"), (land, "a land is played, not cast (305.9)")] {
            cite!(["601.3"]; case: format!("{card:?} paid={paid}"); refused(why): g.act(0, permitted(l, card)));
        }
    }
    g.players[0].pool.colorless = 8;
    let x = add(&mut g, CardDef::new("Variable", "{X}", "Artifact", "", None).unwrap());
    let x = g.put(0, x, Zone::Graveyard);
    cite!(["601.3"]; refused("that casting permission is unavailable (601.3)"): g.act(0, Move::WithX(3, Box::new(permitted(l, x)))));
    cite!(["601.3", "202.3e"]; ok: g.act(
        0,
        crate::support::paid("CC", Move::WithX(
            2,
            Box::new(Move::CastVia(
                CastRoute::Permission { source: l, ability: 2 },
                Box::new(Move::Cast { card: x, targets: vec![] }),
            )),
        )),
    ));
    assert_eq!(g.players[0].pool.colorless, 6);
    cite!(["601.3"]; refused("that casting permission is unavailable (601.3)"): g.act(0, permitted(l, a)));
    pass(&mut g);
    cite!(["601.3"]; refused("that card is not in your hand: it is in a graveyard, and no rule or effect lets you cast it from there (601.3)"): g.act(0, Move::CastVia(CastRoute::Normal, Box::new(Move::Cast { card: a, targets: vec![] }))));
    next(&mut g);
    main(&mut g);
    g.act(1, Move::Pass).unwrap();
    g.act(2, Move::Pass).unwrap();
    cite!(["601.3"]; refused("that casting permission is unavailable (601.3)"): g.act(0, permitted(l, a)));
}
#[test]
fn r601_2i_countered_spell_consumes_permission_failed_announcement_does_not() {
    let mut g = setup();
    let l = cast_lurrus(&mut g);
    let a = g.put(0, 1, Zone::Graveyard);
    cite!(["118.3a"]; refused("your mana pool cannot pay that cost (118.3a): {2}"): g.act(0, permitted(l, a)));
    g.players[0].pool.colorless = 4;
    cite!(["601.3"]; ok: g.act(0, crate::support::paid("CC", permitted(l, a))));
    let spell = g.stack[0];
    let counter = add(&mut g, CardDef::new("Counter", "{0}", "Instant", "Counter target spell.", None).unwrap());
    let counter = g.put(1, counter, Zone::Hand);
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Cast { card: counter, targets: vec![vec![Target::Object(spell)]] }).unwrap();
    pass(&mut g);
    assert!(g.stack.is_empty());
    let back = *g.players[0].graveyard.last().unwrap();
    cite!(["601.3"]; refused("that casting permission is unavailable (601.3)"): g.act(0, permitted(l, back)));
    for _ in 0..3 {
        next(&mut g);
    }
    main(&mut g);
    g.players[0].pool.colorless = 2;
    cite!(["601.3"]; ok: g.act(0, crate::support::paid("CC", permitted(l, back))));
    pass(&mut g);
    assert_eq!(g.battlefield.len(), 2);
}
#[test]
fn r611_3_permission_tracks_effective_ability_controller_and_new_incarnation() {
    let mut g = setup();
    let l = cast_lurrus(&mut g);
    let a = g.put(0, 1, Zone::Graveyard);
    g.players[0].pool.colorless = 6;
    g.act(0, crate::support::paid("CC", permitted(l, a))).unwrap();
    pass(&mut g);
    g.move_object(l, Zone::Hand);
    let card = *g.players[0].hand.last().unwrap();
    g.players[0].pool.colors[0] = 3;
    g.act(0, crate::support::paid("WWW", Move::Cast { card, targets: vec![] })).unwrap();
    pass(&mut g);
    let l = *g.battlefield.last().unwrap();
    let b = g.put(0, 1, Zone::Graveyard);
    let null = add(
        &mut g,
        CardDef::new("Null", "{0}", "Instant", "Target creature loses all abilities until end of turn.", None).unwrap(),
    );
    let null = g.put(0, null, Zone::Hand);
    g.act(0, Move::Cast { card: null, targets: vec![vec![Target::Object(l)]] }).unwrap();
    pass(&mut g);
    cite!(["601.3"]; refused("that casting permission is unavailable (601.3)"): g.act(0, permitted(l, b)));
    next(&mut g);
    main(&mut g);
    let take = add(&mut g, CardDef::new("Take", "{0}", "Sorcery", "Gain control of target creature.", None).unwrap());
    let take = g.put(1, take, Zone::Hand);
    g.act(1, Move::Cast { card: take, targets: vec![vec![Target::Object(l)]] }).unwrap();
    pass(&mut g);
    let theirs = g.put(1, 1, Zone::Graveyard);
    g.players[1].pool.colorless = 2;
    cite!(["601.3"]; ok: g.act(1, crate::support::paid("CC", permitted(l, theirs))));
    pass(&mut g);
    assert_eq!(g.controller(l), 1);
    let another = g.put(1, 1, Zone::Graveyard);
    g.players[1].pool.colorless = 2;
    cite!(["601.3"]; refused("that casting permission is unavailable (601.3)"): g.act(1, permitted(l, another)));
}
#[test]
fn r702_139_companion_special_action_is_paid_once_and_card_then_follows_normal_zones() {
    let mut g =
        Game::start_with_companions(vec![lurrus(), bear()], &vec![vec![1; 30]; 3], &[Some(0), None, Some(0)], 20, 0, 4)
            .unwrap();
    for p in 0..3 {
        g.act(p, Move::Keep).unwrap();
    }
    main(&mut g);
    let hand = g.players[0].hand.len();
    cite!(["116.2g"]; refused("your mana pool cannot pay that cost (118.3a): {3}"): g.act(0, Move::Companion));
    assert!(!g.companion(0).unwrap().taken);
    g.players[0].pool.colorless = 3;
    let mut copy = g.clone();
    for g in [&mut g, &mut copy] {
        cite!(["116.2g"]; ok: g.act(0, crate::support::paid("CCC", Move::Companion)));
        cite!(["116.2g"]; assert_eq!(g.players[0].pool.total(), 0));
        cite!(["116.2g"]; assert_eq!(g.players[0].hand.len(), hand + 1));
        assert!(g.companion(0).unwrap().taken);
        cite!(["702.139a"]; assert!(g.stack.is_empty()));
        assert_eq!(g.priority, 0);
        g.players[0].pool.colors[0] = 3;
        let card = *g.players[0].hand.last().unwrap();
        g.act(0, crate::support::paid("WWW", Move::Cast { card, targets: vec![] })).unwrap();
        pass(g);
        let l = *g.battlefield.last().unwrap();
        assert!(g.has(l, "Lifelink"));
        g.move_object(l, Zone::Graveyard);
        g.players[0].pool.colorless = 3;
        cite!(["702.139a"]; refused("no untaken chosen companion (702.139a)"): g.act(0, Move::Companion));
    }
    assert_eq!(format!("{g:?}"), format!("{copy:?}"));
}
#[test]
fn r702_139_starting_deck_ignores_nonpermanents_and_designation_is_optional() {
    let spell = CardDef::new("Expensive spell", "{9}", "Sorcery", "Draw a card.", None).unwrap();
    let defs = vec![lurrus(), spell];
    cite!(["702.139a"]; assert!(Game::start_with_companions(defs.clone(), &vec![vec![1; 20]; 2], &[Some(0), None], 20, 0, 1).is_ok()));
    cite!(["702.139a"]; assert!(Game::start_with_companions(defs.clone(), &vec![vec![0; 20]; 2], &[None, None], 20, 0, 1).is_ok()));
    // one companion choice for a game of two players describes no game
    assert_eq!(Game::start_with_companions(defs.clone(), &vec![vec![1; 20]; 2], &[Some(0)], 20, 0, 1).map(|_| ()).unwrap_err(),
        game::start::Refused { class: game::simulation::Class::Malformed, why: "one optional companion per player and a valid starting player".into() });
    cite!(["702.139a"]; refused("that card has no companion condition"): Game::start_with_companions(defs, &vec![vec![1; 20]; 2], &[Some(1), None], 20, 0, 1));
}
#[test]
fn r116_2g_companion_payment_suspends_clones_and_reverses_without_using_the_designation() {
    use game::state::{Answer, Choice};
    use ir::Color;
    use ir::mana::ManaSymbol;
    for enough in [false, true] {
        let mut g = Game::start_with_companions(
            vec![lurrus(), bear()],
            &vec![vec![1; 30]; 3],
            &[Some(0), None, None],
            20,
            0,
            7,
        )
        .unwrap();
        for p in 0..3 {
            g.act(p, Move::Keep).unwrap();
        }
        main(&mut g);
        let mana=add(&mut g,CardDef::new("Mana","{0}","Artifact","{T}: Put a charge counter on this artifact. Add X mana of any one color, where X is the number of charge counters on this artifact.",None).unwrap());
        let mana = g.put(0, mana, Zone::Battlefield);
        if enough {
            g.put_counters(mana, "charge", 2);
        }
        let hand = g.players[0].hand.len();
        let steps = vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source: mana, ability: 0, paying: vec![], adding: vec![] }];
        g.act(0, Move::WithMana(steps, Box::new(Move::Companion))).unwrap();
        assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::Mana { .. }, .. })));
        assert!(!g.companion(0).unwrap().taken);
        let mut copy = g.clone();
        for g in [&mut g, &mut copy] {
            g.act(0, Move::Choose(Answer::Mana(vec![ManaSymbol::Color(Color::Blue); if enough { 3 } else { 1 }])))
                .unwrap();
            if enough {
                g.act(0, Move::FinishPayment { plan: vec![], paying: vec![], with: Some(crate::support::mana_pool("UUU")) }).unwrap();
                cite!(["116.2g"]; assert_eq!(g.players[0].hand.len(), hand + 1));
                cite!(["116.2g"]; assert!(g.companion(0).unwrap().taken));
            } else {
                assert!(matches!(g.decision, Some(Decision::Payment { .. })));
                g.act(0, Move::ReversePayment).unwrap();
                assert!(!g.companion(0).unwrap().taken);
                assert_eq!(g.players[0].hand.len(), hand);
                assert_eq!(g.players[0].pool.total(), 1);
                assert_eq!(g.obj(mana).counters["charge"], 1);
            }
        }
        assert_eq!(format!("{g:?}"), format!("{copy:?}"));
    }
}
#[test]
fn r601_2h_permission_survives_sacrificing_its_source_to_pay_for_the_cast() {
    use game::simulation::Simulation;
    let mut g = setup();
    let l = cast_lurrus(&mut g);
    let altar =
        add(&mut g, CardDef::new("Altar", "{0}", "Artifact", "Sacrifice a creature: Add {C}{C}.", None).unwrap());
    let altar = g.put(0, altar, Zone::Battlefield);
    let a = g.put(0, 1, Zone::Graveyard);
    let mut sim = Simulation::new(g);
    let r = sim.request().unwrap().unwrap().clone();
    sim.respond(
        r.id,
        r.player,
        Move::WithMana(
            vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source: altar, ability: 0, paying: vec![vec![l]], adding: vec![] }],
            Box::new(crate::support::paid("CC", permitted(l, a))),
        ),
    )
    .unwrap();
    let fork = sim.clone();
    for mut sim in [sim, fork] {
        for _ in 0..3 {
            let r = sim.request().unwrap().unwrap().clone();
            sim.respond(r.id, r.player, Move::Pass).unwrap();
        }
        let g = sim.game().unwrap();
        cite!(["601.3"]; assert!(g.battlefield.iter().any(|o| g.def(*o).unwrap().name() == "Bear")));
        assert!(!g.battlefield.contains(&l));
        assert_eq!(g.players[0].graveyard.len(), 1);
        let turn = g.turn;
        for _ in 0..180 {
            if sim.game().unwrap().turn > turn {
                break;
            }
            let m = match &sim.game().unwrap().decision {
                Some(Decision::Attackers { .. }) => Move::Attack(vec![]),
                Some(Decision::Discard { player, count }) => {
                    Move::Discard(sim.game().unwrap().players[*player].hand[..*count].to_vec())
                }
                None => Move::Pass,
                x => panic!("{x:?}"),
            };
            let r = sim.request().unwrap().unwrap().clone();
            sim.respond(r.id, r.player, m).unwrap();
        }
        assert!(sim.game().unwrap().get_object(l).is_none());
    }
}
#[test]
fn r611_3_blink_gives_a_new_permission_and_two_sources_offer_distinct_routes() {
    let mut g = setup();
    let l = cast_lurrus(&mut g);
    let a = g.put(0, 1, Zone::Graveyard);
    g.players[0].pool.colorless = 6;
    g.act(0, crate::support::paid("CC", permitted(l, a))).unwrap();
    pass(&mut g);
    g.move_object(l, Zone::Hand);
    let card = *g.players[0].hand.last().unwrap();
    g.players[0].pool.colors[0] = 3;
    g.act(0, crate::support::paid("WWW", Move::Cast { card, targets: vec![] })).unwrap();
    pass(&mut g);
    let l = *g.battlefield.last().unwrap();
    let b = g.put(0, 1, Zone::Graveyard);
    cite!(["400.7", "601.3"]; ok: g.act(0, crate::support::paid("CC", permitted(l, b))));
    pass(&mut g);
    let permit=add(&mut g,CardDef::new("Second permission","{0}","Artifact","Once during each of your turns, you may cast a permanent spell with mana value 2 or less from your graveyard.",None).unwrap());
    let source = g.put(0, permit, Zone::Battlefield);
    let c = g.put(0, 1, Zone::Graveyard);
    g.players[0].pool.colorless = 2;
    cite!(["601.3"]; ok: g.act(
        0,
        crate::support::paid("CC", Move::CastVia(CastRoute::Permission { source, ability: 0 }, Box::new(Move::Cast { card: c, targets: vec![] }))),
    ));
    pass(&mut g);
}
#[test]
fn r702_139_managed_start_and_companion_use_resume_through_normal_requests() {
    use game::simulation::Simulation;
    fn respond(s: &mut Simulation, m: Move) {
        let r = s.request().unwrap().unwrap().clone();
        s.respond(r.id, r.player, m).unwrap();
    }
    let rock = CardDef::new("Rock", "{0}", "Artifact", "{T}: Add {C}{C}{C}.", None).unwrap();
    let g = Game::start_with_companions(
        vec![lurrus(), bear(), rock],
        &vec![vec![2; 30]; 3],
        &[Some(0), None, None],
        20,
        0,
        1,
    )
    .unwrap();
    let mut s = Simulation::new(g);
    for _ in 0..3 {
        respond(&mut s, Move::Keep);
    }
    while s.game().unwrap().part != ir::Part::PrecombatMain {
        respond(&mut s, Move::Pass);
    }
    let card = s.game().unwrap().players[0].hand[0];
    respond(&mut s, Move::Cast { card, targets: vec![] });
    for _ in 0..3 {
        respond(&mut s, Move::Pass);
    }
    let rock = s.game().unwrap().battlefield[0];
    let fork = s.clone();
    for mut s in [s, fork] {
        respond(
            &mut s,
            Move::WithMana(
                vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source: rock, ability: 0, paying: vec![], adding: vec![] }],
                Box::new(crate::support::paid("CCC", Move::Companion)),
            ),
        );
        assert!(s.game().unwrap().companion(0).unwrap().taken);
        cite!(["116.2g"]; assert_eq!(s.game().unwrap().players[0].hand.len(), 8));
        cite!(["702.139a"]; assert!(s.game().unwrap().stack.is_empty()));
        let turn = s.game().unwrap().turn;
        for _ in 0..180 {
            if s.game().unwrap().turn > turn {
                break;
            }
            let m = match &s.game().unwrap().decision {
                Some(Decision::Attackers { .. }) => Move::Attack(vec![]),
                Some(Decision::Discard { player, count }) => {
                    Move::Discard(s.game().unwrap().players[*player].hand[..*count].to_vec())
                }
                None => Move::Pass,
                x => panic!("{x:?}"),
            };
            respond(&mut s, m);
        }
        assert!(s.game().unwrap().companion(0).unwrap().taken);
    }
}
#[test]
fn r601_3_flash_still_works_only_during_the_controllers_turn_and_cage_overrides_permission() {
    let mut g = setup();
    let l = cast_lurrus(&mut g);
    let flash = add(&mut g, CardDef::new("Flash permanent", "{0}", "Creature", "Flash", Some((1, 1))).unwrap());
    let flash = g.put(0, flash, Zone::Graveyard);
    g.part = ir::Part::Upkeep;
    let mut allowed = g.clone();
    cite!(["702.8a", "601.3"]; ok: allowed.act(0, permitted(l, flash)));
    pass(&mut allowed);
    assert_eq!(allowed.battlefield.len(), 2);
    next(&mut g);
    main(&mut g);
    g.act(1, Move::Pass).unwrap();
    g.act(2, Move::Pass).unwrap();
    cite!(["601.3"]; refused("that casting permission is unavailable (601.3)"): g.act(0, permitted(l, flash)));
    let mut g = setup();
    cast_lurrus(&mut g);
    let bear = g.put(0, 1, Zone::Graveyard);
    g.players[0].pool.colorless = 2;
    let cage = add(
        &mut g,
        CardDef::new("Cage", "{0}", "Artifact", "Players can't cast spells from graveyards or libraries.", None)
            .unwrap(),
    );
    g.put(0, cage, Zone::Battlefield);
    cite!(["601.3"]; refused("an effect prohibits casting that spell (601.3)"): g.act(0, Move::Cast { card: bear, targets: vec![] }));

}
#[test]
fn r702_139_unsupported_companion_and_limited_permission_forms_remain_refused() {
    for text in [
        "Companion — Each permanent card in your starting deck has flying.",
        "Once each turn, you may cast a permanent spell from your graveyard without paying its mana cost.",
        "Once each turn, you may cast a permanent spell from your hand.",
        "Once each turn, you may play a land from your graveyard.",
    ] {
        let d = CardDef::new("Boundary", "{0}", "Creature", text, Some((1, 1))).unwrap();
        let reason = if text.starts_with("Companion") { "companion: starting-deck printed characteristic predicate" }
            else if text.ends_with("from your hand.") { "limited casting permission: your graveyard" }
            else { "limited casting permission: ordinary casting costs" };
        crate::support::refused(game::play::check(&d).map_err(|e|e.0), reason);
    }
    let other = CardDef::new(
        "Different restriction",
        "{0}",
        "Creature",
        "Companion — Each permanent card in your starting deck has mana value 3 or less.",
        Some((1, 1)),
    )
    .unwrap();
    game::play::check(&other).unwrap();
    cite!(["702.139a"]; assert!(
        Game::start_with_companions(vec![other, lurrus()], &vec![vec![1; 20]; 2], &[Some(0), None], 20, 0, 0).is_ok()
    ));
}
#[test]
fn r116_2g_companion_action_requires_sorcery_timing_for_the_designated_player() {
    let mut g =
        Game::start_with_companions(vec![lurrus(), bear()], &vec![vec![1; 30]; 3], &[Some(0), None, Some(0)], 20, 0, 3)
            .unwrap();
    for p in 0..3 {
        g.act(p, Move::Keep).unwrap();
    }
    main(&mut g);
    g.players[0].pool.colorless = 5;
    let card = g.players[0].hand[0];
    g.act(0, crate::support::paid("CC", Move::Cast { card, targets: vec![] })).unwrap();
    cite!(["116.2g"]; refused("companion action requires your main phase and an empty stack (116.2g)"): g.act(0, Move::Companion));
    pass(&mut g);
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    g.players[2].pool.colorless = 3;
    cite!(["116.2g"]; refused("companion action requires your main phase and an empty stack (116.2g)"): g.act(2, Move::Companion));
    g.act(2, Move::Pass).unwrap();
    while g.part != ir::Part::BeginningOfCombat {
        g.act(g.priority, Move::Pass).unwrap();
    }
    g.players[0].pool.colorless = 3;
    cite!(["116.2g"]; refused("companion action requires your main phase and an empty stack (116.2g)"): g.act(0, Move::Companion));
    for _ in 0..2 {
        next(&mut g);
    }
    main(&mut g);
    assert_eq!(g.active, 2);
    g.players[2].pool.colorless = 3;
    cite!(["116.2g"]; ok: g.act(2, crate::support::paid("CCC", Move::Companion)));
    cite!(["116.2g"]; assert!(g.companion(2).unwrap().taken));
    assert!(!g.companion(0).unwrap().taken);
}
#[test]
fn r601_2b_permission_uses_the_announced_x() {
    let permission = CardDef::new(
        "Permission",
        "{0}",
        "Artifact",
        "Once each turn, you may cast an artifact spell with mana value 2 or greater from your graveyard.",
        None,
    )
    .unwrap();
    game::play::check(&permission).unwrap();
    let variable = CardDef::new("Variable", "{X}", "Artifact", "", None).unwrap();
    let mut g = Game::new(vec![permission, variable], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let card = g.put(0, 1, Zone::Graveyard);
    g.players[0].pool.colorless = 2;
    cite!(["601.3"]; refused("that casting permission is unavailable (601.3)"): g.act(0, Move::WithX(1, Box::new(Move::CastVia(CastRoute::Permission { source, ability: 0 }, Box::new(Move::Cast { card, targets: vec![] }))))));
    cite!(["202.3e"]; ok: g.act(0, crate::support::paid("CC", Move::WithX(2, Box::new(Move::CastVia(CastRoute::Permission { source, ability: 0 }, Box::new(Move::Cast { card, targets: vec![] })))))));
    pass(&mut g);
    assert_eq!(g.battlefield.len(), 2);
}
#[test]
fn r514_3a_repeated_cleanup_does_not_reset_the_casting_allowance() {
    let mut g = setup();
    let l = cast_lurrus(&mut g);
    let flash = add(&mut g, CardDef::new("Quick creature", "{0}", "Creature", "Flash", Some((1, 1))).unwrap());
    let card = g.put(0, flash, Zone::Graveyard);
    g.act(0, permitted(l, card)).unwrap();
    pass(&mut g);
    let second = g.put(0, flash, Zone::Graveyard);
    let watcher = add(
        &mut g,
        CardDef::new("Watcher", "{0}", "Artifact", "Whenever you discard a card, you gain 1 life.", None).unwrap(),
    );
    g.put(0, watcher, Zone::Battlefield);
    for _ in 0..8 {
        g.put(0, 1, Zone::Hand);
    }
    let turn = g.turn;
    for _ in 0..100 {
        if matches!(g.decision, Some(Decision::Discard { .. })) {
            break;
        }
        let m = if matches!(g.decision, Some(Decision::Attackers { .. })) { Move::Attack(vec![]) } else { Move::Pass };
        g.act(g.priority, m).unwrap();
    }
    let Some(Decision::Discard { player, count }) = g.decision else { panic!("discard") };
    let cards = g.players[player].hand[..count].to_vec();
    g.act(player, Move::Discard(cards)).unwrap();
    assert_eq!(g.turn, turn);
    assert_eq!(g.part, ir::Part::Cleanup);
    assert!(!g.stack.is_empty());
    cite!(["601.3"]; refused("that casting permission is unavailable (601.3)"): g.act(0, permitted(l, second)));
    pass(&mut g);
    cite!(["601.3"]; refused("that casting permission is unavailable (601.3)"): g.act(0, permitted(l, second)));
}

#[track_caller]
fn unchanged(g: &mut Game, action: impl FnOnce(&mut Game) -> Result<(), String>) -> Result<(), String> {
    let before=format!("{g:?}");
    let result=action(g);
    assert_eq!(format!("{g:?}"),before);
    result
}
