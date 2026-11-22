//! Replay supplies random outcomes; the engine still performs the shuffle and every
//! following instruction (701.24a, 608.2c), including drawing from the top (121.1).
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Zone};
use game::simulation::Simulation;
use game::start::{CapturedRandom, ShuffleOrder, RandomKind, RandomOutcome};
use game::simulation::ResponseError;
use ir::action::LibraryPosition;

fn fixture(text: &str) -> (Simulation, Vec<ObjId>) {
    let defs = vec![
        CardDef::new("Tape spell", "{0}", "Instant", text, None).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    let cards = vec![g.put(0, 1, Zone::Library), g.put(0, 1, Zone::Library), g.put(0, 2, Zone::Library)];
    g.put(1, 1, Zone::Library);
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
    g.act(g.priority, Move::Pass).unwrap();
    (Simulation::new(g), cards)
}

fn resolve(sim: &mut Simulation, orders: Vec<ShuffleOrder>) -> Result<(), game::simulation::ResponseError> {
    let request = sim.request().unwrap().unwrap().clone();
    sim.respond_with_shuffles(request.id, request.player, Move::Pass, orders)
}

#[test]
fn r608_2c_tape_shuffle_is_applied_before_the_following_draws() {
    let (mut sim, cards) = fixture("Shuffle. Draw two cards.");
    let other = sim.game().unwrap().players[1].library.clone();
    let order = vec![cards[1], cards[2], cards[0]];
    resolve(&mut sim, vec![ShuffleOrder { player: 0, order }]).unwrap();
    let g = sim.game().unwrap();
    cite!(["608.2c", "121.1"]; assert_eq!(g.players[0].library, vec![cards[0]]));
    let names: Vec<_> = g.players[0].hand.iter().map(|id| g.obj(*id).chars.name.as_str()).collect();
    assert_eq!(names, vec!["Forest", "Island"]);
    let departed: Vec<_> = g.log.iter().filter_map(|e| match e {
        Event::ZoneChanged { from: Zone::Library, was, .. } => Some(*was), _ => None,
    }).collect();
    assert_eq!(departed, vec![cards[1], cards[2]], "identical copies keep distinct identities");
    assert_eq!(g.players[1].library, other);
    cite!(["701.24a"]; assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Shuffled { player: 0, .. })).count(), 1));
}

#[test]
fn r603_2c_tape_preserves_two_shuffle_occurrences_in_one_resolution() {
    let (mut sim, cards) = fixture("Shuffle. Shuffle. Draw a card.");
    resolve(&mut sim, vec![
        ShuffleOrder { player: 0, order: cards.clone() },
        ShuffleOrder { player: 0, order: vec![cards[2], cards[0], cards[1]] },
    ]).unwrap();
    let g = sim.game().unwrap();
    assert_eq!(g.obj(g.players[0].hand[0]).chars.name, "Island");
    assert_eq!(g.top_cards(0, 2), vec![cards[0], cards[1]]);
    cite!(["603.2c"]; assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Shuffled { .. })).count(), 2));
}

#[test]
fn r701_20d_recorded_physical_identity_survives_revealed_card_becoming_hidden() {
    let (mut sim, cards) = fixture("Reveal the top card of your library. Shuffle. Draw a card.");
    resolve(&mut sim, vec![ShuffleOrder { player: 0, order: cards.clone() }]).unwrap();
    let g = sim.game().unwrap();
    assert_eq!(g.top_cards(0, 2)[0], cards[1]);
    let hidden = g.top_cards(0, 2)[1];
    cite!(["701.20d"]; assert_ne!(hidden, cards[2]));
    assert_eq!(g.obj(hidden).chars.name, "Island");
    assert!(!g.sees(1, hidden));
}

#[test]
fn r400_7_tape_can_name_cards_that_enter_the_library_during_resolution() {
    let mut g = Game::new(vec![
        CardDef::new("Return", "{0}", "Instant", "Shuffle your graveyard into your library. Draw a card.", None).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
    ], 2, 20);
    let forest = g.put(0, 1, Zone::Library);
    let island = g.put(0, 2, Zone::Graveyard);
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
    g.act(g.priority, Move::Pass).unwrap();
    let mut sim = Simulation::new(g);
    resolve(&mut sim, vec![ShuffleOrder { player: 0, order: vec![island, forest] }]).unwrap();
    let g = sim.game().unwrap();
    cite!(["400.7", "121.1"]; assert_eq!(g.obj(g.players[0].hand[0]).chars.name, "Island"));
    assert_ne!(g.players[0].hand[0], island);
    assert_eq!(g.top_cards(0, 1), vec![forest]);
}

#[test]
fn r701_24e_empty_and_single_card_shuffles_consume_their_recorded_outcomes() {
    for count in 0..=1 {
        let mut g = Game::new(vec![
            CardDef::new("Mix", "{0}", "Instant", "Shuffle.", None).unwrap(),
            CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        ], 2, 20);
        let order = (0..count).map(|_| g.put(0, 1, Zone::Library)).collect::<Vec<_>>();
        let spell = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
        g.act(g.priority, Move::Pass).unwrap();
        let mut sim = Simulation::new(g);
        resolve(&mut sim, vec![ShuffleOrder { player: 0, order: order.clone() }]).unwrap();
        let g = sim.game().unwrap();
        assert_eq!(g.top_cards(0, count), order);
        cite!(["701.24e"]; assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Shuffled { .. })).count(), 1));
    }
}

#[test]
fn r701_24b_tape_cannot_shuffle_the_cards_set_aside_by_a_search() {
    for move_found in [false, true] {
        let (mut sim, cards) = fixture("Search your library for a card, then shuffle. You may put that card into your hand.");
        resolve(&mut sim, vec![]).unwrap(); // resolution stops for the search choice
        let request = sim.request().unwrap().unwrap().clone();
        let order = if move_found { vec![cards[1], cards[0], cards[2]] } else { cards.clone() };
        let result = sim.respond_with_shuffles(request.id, request.player,
            Move::Choose(game::state::Answer::Objects(vec![cards[1]])),
            vec![ShuffleOrder { player: 0, order }]);
        if move_found {
            assert!(matches!(result, Err(ResponseError::ReplayInput(_))));
            assert!(sim.fault().is_none());
            assert_eq!(sim.request().unwrap().unwrap(), &request);
        } else {
            result.unwrap();
            cite!(["701.24b"]; assert_eq!(sim.game().unwrap().top_cards(0, 3), cards));
        }
    }
}

#[test]
fn r701_24b_tape_may_leave_out_the_cards_set_aside_by_a_search() {
    // a record written after the search lists the library without the card it found: the
    // set-aside card keeps its position and the order names the rest
    for (rest, expected) in [(vec![0, 2], vec![0, 1, 2]), (vec![2, 0], vec![2, 1, 0])] {
        let (mut sim, cards) = fixture("Search your library for a card, then shuffle. You may put that card into your hand.");
        resolve(&mut sim, vec![]).unwrap();
        let request = sim.request().unwrap().unwrap().clone();
        let order = rest.iter().map(|i| cards[*i]).collect();
        sim.respond_with_shuffles(request.id, request.player,
            Move::Choose(game::state::Answer::Objects(vec![cards[1]])),
            vec![ShuffleOrder { player: 0, order }]).unwrap();
        let expected: Vec<ObjId> = expected.iter().map(|i| cards[*i]).collect();
        cite!(["701.24b"]; assert_eq!(sim.game().unwrap().top_cards(0, 3), expected));
    }
}

#[test]
fn tape_rejects_missing_extra_wrong_player_and_wrong_membership_outcomes() {
    for problem in 0..4 {
        let (mut sim, cards) = fixture("Shuffle.");
        let correct = ShuffleOrder { player: 0, order: cards.clone() };
        let orders = match problem {
            0 => vec![],
            1 => vec![correct.clone(), correct],
            2 => vec![ShuffleOrder { player: 1, order: sim.game().unwrap().players[1].library.clone() }],
            _ => vec![ShuffleOrder { player: 0, order: cards[..2].to_vec() }],
        };
        let request = sim.request().unwrap().unwrap().clone();
        let before = format!("{:?}", sim.game().unwrap());
        assert!(matches!(resolve(&mut sim, orders), Err(ResponseError::ReplayInput(_))));
        assert!(sim.fault().is_none(), "bad replay input leaves the request pending");
        assert_eq!(sim.request().unwrap().unwrap(), &request);
        assert_eq!(format!("{:?}", sim.game().unwrap()), before);
        resolve(&mut sim, vec![ShuffleOrder { player: 0, order: cards.clone() }]).unwrap();
    assert_eq!(sim.game().unwrap().top_cards(0,3),cards);
    assert_eq!(sim.game().unwrap().log.iter().filter(|e|matches!(e,Event::Shuffled{player:0,..})).count(),1);
    }
}

#[test]
fn tape_rejects_duplicate_outcomes_without_mutating_the_game() {

    let (mut sim, cards) = fixture("Shuffle.");
    let before = format!("{:?}", sim.game().unwrap());
    assert_eq!(resolve(&mut sim, vec![ShuffleOrder { player: 0, order: vec![cards[0]; 3] }]),Err(ResponseError::ReplayInput(game::start::ReplayInput("random outcome repeats a card".into()))));
    assert!(sim.fault().is_none());
    assert_eq!(format!("{:?}", sim.game().unwrap()), before);
    resolve(&mut sim, vec![ShuffleOrder { player: 0, order: cards }]).unwrap();
}

#[test]
fn fixture_library_order_is_atomic_and_refreshes_cached_observations() {

    let (mut sim, cards) = fixture("Shuffle.");
    let before = sim.snapshot(None).unwrap();
    let request = sim.request().unwrap().unwrap().clone();
    let foreign = sim.game().unwrap().players[1].library[0];
    // A fixture's order is no game action: one that is not this library's permutation is
    // malformed input, refused, not a move the rules forbid.
    for bad in [vec![], vec![cards[0]; 3], vec![cards[0], cards[1], foreign], vec![ObjId(u64::MAX); 3]] {
        assert_eq!(sim.set_library_order(request.id, 0, &bad),Err(ResponseError::Malformed("library order must be an exact permutation of its current objects".into())));
        assert_eq!(sim.snapshot(None).unwrap(), before);
        assert_eq!(sim.request().unwrap().unwrap().id, request.id);
    }
    sim.set_library_order(request.id, 0, &cards).unwrap();
    assert_eq!(sim.game().unwrap().top_cards(0, 3), cards);
    assert_ne!(sim.snapshot(None).unwrap(), before);
    assert_ne!(sim.request().unwrap().unwrap().id, request.id);
    let current=sim.request().unwrap().unwrap().id;
    assert_eq!(sim.respond(request.id, request.player, Move::Pass),Err(ResponseError::Stale{expected:current,received:request.id}));
    assert!(!sim.game().unwrap().log.iter().any(|e| matches!(e, Event::Shuffled { .. })));
}

#[test]
fn r608_2c_tape_forbids_unrecorded_random_bottom_order() {
    // Shimmer of Possibility's instructions require random order for the rest. The
    // shuffle-only receipt list must refuse this separate random operation instead of
    // silently choosing an order from the seeded generator.
    // CR 608.2c requires following the instructions; the refusal itself is a harness contract.
    let text = "Look at the top four cards of your library. Put one of them into your hand and the rest on the bottom of your library in a random order.";
    for recorded in [false, true] {
        let (mut sim, cards) = fixture(text);
        resolve(&mut sim, vec![]).unwrap(); // stops before a card is selected, without randomness
        let request = sim.request().unwrap().unwrap().clone();
        let action = Move::Choose(game::state::Answer::Objects(vec![cards[0]]));
        if recorded {
            let failure = sim.respond_with_shuffles(request.id, request.player, action, vec![]).unwrap_err();
            assert!(failure.to_string().contains("unrecorded random library order"), "{failure}");
            assert!(matches!(failure, ResponseError::ReplayInput(_)));
            assert!(sim.fault().is_none());
            assert_eq!(sim.request().unwrap().unwrap(), &request);
        } else {
            // Ordinary simulation still executes the required random placement. The
            // expected membership is independent of whichever random order it chose.
            sim.respond(request.id, request.player, action).unwrap();
            let g = sim.game().unwrap();
            let hand: Vec<_> = g.players[0].hand.iter().map(|id| g.obj(*id).chars.name.as_str()).collect();
            cite!(["608.2c", "609.3"]; assert_eq!(hand, vec!["Forest"]));
            let mut library: Vec<_> = g.players[0].library.iter().map(|id| g.obj(*id).chars.name.as_str()).collect();
            library.sort();
            assert_eq!(library, vec!["Forest", "Island"]);
        }
    }
}

#[test]
fn harness_records_shuffle_order_before_following_draws() {
    // Uncited (protocol): capture must report the intermediate permutation, including
    // cards drawn before this response ends, rather than reconstructing from final state.
    let (mut sim, cards) = fixture("Shuffle. Draw two cards.");
    let physical: Vec<_> = cards.iter().map(|id|sim.game().unwrap().physical_card_identity(*id).unwrap()).collect();
    let request = sim.request().unwrap().unwrap().clone();
    let outcomes = sim.respond_capturing_random(request.id,request.player,Move::Pass).unwrap();
    assert_eq!(outcomes.len(),1);
    let CapturedRandom::Order {player,order,..} = &outcomes[0] else { panic!("a captured shuffle") };
    assert_eq!(*player,0);
    assert_eq!(order.len(),3);
    let mut captured=order.clone();captured.sort();
    let mut expected=physical;expected.sort();assert_eq!(captured,expected);
    let g=sim.game().unwrap();
    let drawn: Vec<_> = g.players[0].hand.iter().map(|id|g.physical_card_identity(*id).unwrap()).collect();
    assert_eq!(drawn,order[..2]);
    assert_eq!(g.players[0].library.len(),1);
}


fn bottom_receipt(order: Vec<ObjId>) -> RandomOutcome {
    RandomOutcome::RandomOrder { player: None, order }
}

#[test]
fn r608_2c_random_bottom_receipt_names_exact_group_and_nearest_top_order() {
    // Shimmer of Possibility: the selected card goes to hand, the remaining looked-at
    // cards go to the bottom in a random order. No full-library shuffle occurs.
    let text = "Look at the top four cards of your library. Put one of them into your hand and the rest on the bottom of your library in a random order.";
    let (mut sim, cards) = fixture(text);
    resolve(&mut sim, vec![]).unwrap();
    let request = sim.request().unwrap().unwrap().clone();
    let action = Move::Choose(game::state::Answer::Objects(vec![cards[0]]));
    let physical: Vec<_> = cards.iter().map(|id| sim.game().unwrap().physical_card_identity(*id).unwrap()).collect();
    let unused = sim.respond_with_random(request.id, request.player, action, vec![bottom_receipt(vec![cards[1], cards[2]])], false).unwrap();
    assert!(unused.is_empty());
    let g = sim.game().unwrap();
    let library: Vec<_> = g.top_cards(0, 3).iter().map(|id| g.physical_card_identity(*id).unwrap()).collect();
    cite!(["608.2c", "401.4"]; assert_eq!(library, vec![physical[1], physical[2]]));
    assert_eq!(g.physical_card_identity(g.players[0].hand[0]), Some(physical[0]));
    assert!(!g.log.iter().any(|event| matches!(event, Event::Shuffled { .. })));
}

#[test]
fn random_bottom_bad_receipt_is_atomic_and_corrected_receipt_can_resume() {
    let text = "Look at the top four cards of your library. Put one of them into your hand and the rest on the bottom of your library in a random order.";
    for problem in 0..7 {
        let (mut sim, cards) = fixture(text);
        resolve(&mut sim, vec![]).unwrap();
        let request = sim.request().unwrap().unwrap().clone();
        let action = Move::Choose(game::state::Answer::Objects(vec![cards[0]]));
        let right = bottom_receipt(vec![cards[2], cards[1]]);
        let receipts = match problem {
            0 => vec![],
            1 => vec![bottom_receipt(vec![cards[1]])],
            2 => vec![bottom_receipt(vec![cards[1], cards[1]])],
            3 => vec![bottom_receipt(vec![cards[0], cards[1]])],
            4 => vec![RandomOutcome::Shuffle { player: 0, order: vec![cards[2], cards[1]] }],
            5 => vec![RandomOutcome::RandomOrder { player: Some(1), order: vec![cards[2], cards[1]] }],
            _ => vec![right.clone(), right.clone()],
        };
        let before = format!("{:?}", sim.game().unwrap());
        let snapshot = sim.snapshot(None).unwrap();
        let error = sim.respond_with_random(request.id, request.player, action.clone(), receipts, false).unwrap_err();
        assert!(matches!(error, ResponseError::ReplayInput(_)), "{error}");
        assert!(sim.fault().is_none());
        assert_eq!(sim.request().unwrap().unwrap(), &request);
        assert_eq!(format!("{:?}", sim.game().unwrap()), before);
        assert_eq!(sim.snapshot(None).unwrap(), snapshot);
        sim.respond_with_random(request.id, request.player, action, vec![right], false).unwrap();
    }
}

#[test]
fn r701_57a_discover_captures_and_replays_random_bottom_with_physical_provenance() {
    // Top-down: expensive nonland, land, eligible Bear, untouched land. CR701.57a
    // exiles three cards, offers only Bear, then returns just the first two at bottom.
    let defs = vec![
        CardDef::new("Finder", "{0}", "Instant", "Discover 3.", None).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Bear", "{2}", "Creature — Bear", "", Some((2,2))).unwrap(),
        CardDef::new("Huge", "{6}", "Creature — Wurm", "", Some((6,6))).unwrap(),
    ];
    let mut g = Game::new(defs, 2, 20);
    let cards: Vec<_> = [1, 2, 1, 3].iter().map(|def| g.put(0, *def, Zone::Library)).collect();
    let physical: Vec<_> = cards.iter().map(|id| g.physical_card_identity(*id).unwrap()).collect();
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
    g.act(g.priority, Move::Pass).unwrap();
    let mut sim = Simulation::new(g);
    resolve(&mut sim, vec![]).unwrap();
    let request = sim.request().unwrap().unwrap().clone();
    let mut replay = sim.clone();
    let captured = sim.respond_capturing_random(request.id, request.player, Move::Choose(game::state::Answer::No)).unwrap();
    assert_eq!(captured.len(), 1);
    let CapturedRandom::Order {kind,player,position,order: physical_order} = &captured[0] else { panic!("a captured order") };
    assert_eq!(*kind, RandomKind::RandomOrder);
    assert_eq!(*player, 0);
    assert_eq!(*position, Some(LibraryPosition::Bottom));
    let mut members = physical_order.clone(); members.sort();
    let mut expected = vec![physical[2], physical[3]]; expected.sort();
    assert_eq!(members, expected);
    let replay_g = replay.game().unwrap();
    let order = physical_order.iter().map(|physical| replay_g.exile.iter().copied()
        .find(|id| replay_g.physical_card_identity(*id) == Some(*physical)).unwrap()).collect();
    replay.respond_with_random(request.id, request.player, Move::Choose(game::state::Answer::No), vec![bottom_receipt(order)], false).unwrap();
    for sim in [&sim, &replay] {
        let g = sim.game().unwrap();
        let library: Vec<_> = g.top_cards(0, 4).iter().map(|id| g.physical_card_identity(*id).unwrap()).collect();
        cite!(["701.57a", "401.4"]; assert_eq!(library, [vec![physical[0]], physical_order.clone()].concat()));
        cite!(["701.57a"]; assert_eq!(g.physical_card_identity(g.players[0].hand[0]), Some(physical[1])));
        assert_eq!(g.obj(g.players[0].hand[0]).chars.name, "Bear");
        assert!(!g.log.iter().any(|event| matches!(event, Event::Shuffled { .. })));
    }
}

#[test]
fn random_outcomes_keep_unused_rebinds_cards_after_zone_changes() {
    let (mut sim, cards) = fixture("Shuffle. Draw a card.");
    let request = sim.request().unwrap().unwrap().clone();
    let kept = sim.respond_with_random(request.id, request.player, Move::Pass,
        vec![bottom_receipt(cards.clone()), ShuffleOrder { player: 0, order: cards.clone() }.into(), bottom_receipt(vec![cards[0]])], true).unwrap();
    assert_eq!(kept.len(), 2);
    assert_eq!(kept.iter().map(|unused|unused.input_index).collect::<Vec<_>>(), [0, 2]);
    let RandomOutcome::RandomOrder {order: first,..} = &kept[0].outcome else { panic!("an unused random order") };
    let RandomOutcome::RandomOrder {order: second,..} = &kept[1].outcome else { panic!("an unused random order") };
    assert_eq!(first[1..], cards[1..]);
    let g = sim.game().unwrap();
    assert_eq!(first[0], g.players[0].hand[0]);
    assert_ne!(first[0], cards[0]);
    assert_eq!(*second, vec![g.players[0].hand[0]]);
}

#[test]
fn r701_24d_shuffle_into_has_one_random_event_and_no_insertion_order_choice() {
    // CR701.24c-d: insertion and the library shuffle form the printed shuffle-into
    // instruction. Its internal insertion order creates no separate random event.
    let mut g = Game::new(vec![
        CardDef::new("Return", "{0}", "Instant", "Shuffle your graveyard into your library. Draw two cards.", None).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
    ], 2, 20);
    let library = g.put(0, 1, Zone::Library);
    let forest = g.put(0, 1, Zone::Graveyard);
    let island = g.put(0, 2, Zone::Graveyard);
    let physical: Vec<_> = [library, forest, island].iter().map(|id| g.physical_card_identity(*id).unwrap()).collect();
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
    g.act(g.priority, Move::Pass).unwrap();
    let mut sim = Simulation::new(g);
    let request = sim.request().unwrap().unwrap().clone();
    let mut recording = sim.clone();
    let outcomes = recording.respond_capturing_random(request.id, request.player, Move::Pass).unwrap();
    assert_eq!(outcomes.len(), 1, "insertion must neither ask for owner order nor capture a phantom random order");
    let CapturedRandom::Order {kind,order,..} = &outcomes[0] else { panic!("a captured shuffle") };
    assert_eq!(*kind, RandomKind::Shuffle);
    let mut actual = order.clone(); actual.sort();
    let mut expected = physical.clone(); expected.sort();
    assert_eq!(actual, expected);
    resolve(&mut sim, vec![ShuffleOrder { player: 0, order: vec![island, library, forest] }]).unwrap();
    let game = sim.game().unwrap();
    let drawn: Vec<_> = game.players[0].hand.iter().map(|id| game.physical_card_identity(*id).unwrap()).collect();
    cite!(["701.24d", "608.2c", "121.1"]; assert_eq!(drawn, vec![physical[2], physical[0]]));
    assert_eq!(game.physical_card_identity(game.top_cards(0, 1)[0]), Some(physical[1]));
    assert_eq!(game.log.iter().filter(|event| matches!(event, Event::Shuffled { player: 0, .. })).count(), 1);
}

#[test]
fn r401_4_single_selected_card_random_bottom_needs_no_random_outcome() {
    // The two cards looked at are Island then Forest. Selecting Island leaves one
    // Forest: its only permutation is placed below the untouched bottom Forest.
    let (mut sim, cards) = fixture("Look at the top two cards of your library. Put one of them into your hand and the rest on the bottom of your library in a random order.");
    let physical: Vec<_> = cards.iter().map(|id| sim.game().unwrap().physical_card_identity(*id).unwrap()).collect();
    resolve(&mut sim, vec![]).unwrap();
    let request = sim.request().unwrap().unwrap().clone();
    let action = Move::Choose(game::state::Answer::Objects(vec![cards[2]]));
    let mut recording = sim.clone();
    assert!(recording.respond_capturing_random(request.id, request.player, action.clone()).unwrap().is_empty());
    sim.respond_with_random(request.id, request.player, action, vec![], false).unwrap();
    for sim in [&sim, &recording] {
        let game = sim.game().unwrap();
        let remaining: Vec<_> = game.top_cards(0, 2).iter().map(|id| game.physical_card_identity(*id).unwrap()).collect();
        cite!(["401.4", "608.2c"]; assert_eq!(remaining, vec![physical[0], physical[1]]));
        assert_eq!(game.physical_card_identity(game.players[0].hand[0]), Some(physical[2]));
        assert!(!game.log.iter().any(|event| matches!(event, Event::Shuffled { .. })));
    }
}
