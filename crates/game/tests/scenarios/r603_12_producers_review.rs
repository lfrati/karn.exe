//! Acts that produce "if you do / if you can't / when you do"
//! (118.12, 603.12): a mandatory act on several objects is a cost done wholly or not at all
//! (118.3; Egon, God of Death ruling 2021-02-05), the reflexive ability's window, controller
//! and last known information (603.3, 603.10), repetition within one resolution (603.12a),
//! "if you don't" and "if you can't" after declining or being unable, a partial mill
//! (701.17b; Wick's Patrol ruling 2024-07-26) and an energy payment (107.14).
use crate::scenarios::r708_8_turned_face_up::{game, put};
use crate::support::cite;
use crate::support::*;

type Text<'a> = (&'a str, &'a str, &'a str, &'a str, Option<(i32, i32)>);

fn spell(text: &str) -> Text<'_> { ("Parent", "{0}", "Instant", text, None) }

/// Settle the stack, answering each "may" with `yes`, each object choice with the first
/// objects offered, each target question with `targets`: the questions asked, and by whom.
fn run(t: &mut T, yes: bool, targets: &[Target], orders: &[(usize, Vec<ObjId>)]) -> Vec<(usize, &'static str)> {
    let mut asked = Vec::new();
    let mut orders = orders.iter();
    for _ in 0..200 {
        let Some((p, question)) = t.g.asked() else { break };
        match question {
            Asked::Priority if t.g.stack.is_empty() => break,
            Asked::Priority => { t.g.act(p, Move::Pass).unwrap(); }
            Asked::May => { asked.push((p, "may")); t.g.act(p, Move::Choose(if yes { Answer::Yes } else { Answer::No })).unwrap(); }
            Asked::Targets { .. } => { asked.push((p, "targets")); t.g.act(p, Move::ChooseTargets(targets.iter().map(|t| vec![*t]).collect())).unwrap(); }
            Asked::Objects { among, count, .. } | Asked::Discard { among, count } => {
                asked.push((p, "objects"));
                t.g.act(p, Move::Choose(Answer::Objects(among[..count.min(among.len())].to_vec()))).unwrap();
            }
            Asked::Search { among, min, .. } | Asked::Select { among, min, .. } => {
                asked.push((p, "select"));
                t.g.act(p, Move::Choose(Answer::Objects(among[..min.max(1).min(among.len())].to_vec()))).unwrap();
            }
            Asked::OrderCards { to: game::state::Placement::Zone(Zone::Graveyard), cards } => {
                // CR404.3: every caller authors exact identities; none are inferred here.
                let (owner, permutation) = orders.next().expect("an authored graveyard order");
                assert_eq!(p, *owner);
                assert_eq!(cards.len(), permutation.len());
                assert!(cards.iter().all(|card| permutation.contains(card)));
                t.g.act(p, Move::Choose(Answer::Objects(permutation.clone()))).unwrap();
            }
            Asked::Arrange { cards, .. } => { asked.push((p, "arrange")); t.g.act(p, Move::Choose(Answer::Arrange { top: cards, away: vec![] })).unwrap(); }
            Asked::Order(list) => { asked.push((p, "order")); t.g.act(p, Move::Order((0..list.len()).collect())).unwrap(); }
            Asked::Attackers(_) => { t.g.act(p, Move::Attack(vec![])).unwrap(); }
            Asked::Blockers { .. } => { t.g.act(p, Move::Block(vec![])).unwrap(); }
            other => panic!("unexpected question {other:?}"),
        }
    }
    assert!(orders.next().is_none(), "every authored order was consumed");
    asked
}

fn said(asked: &[(usize, &'static str)], what: &str) -> bool { asked.iter().any(|(_, a)| *a == what) }

fn main_phase(t: &mut T) { while t.g.part != Part::PrecombatMain { t.g.act(t.g.priority, Move::Pass).unwrap(); } }

/// Player 0 casts the test spell, which resolves; the questions asked until the stack is empty.
fn cast(t: &mut T, yes: bool, targets: &[Target], orders: &[(usize, Vec<ObjId>)]) -> Vec<(usize, &'static str)> {
    let card = put(t, 0, "Parent", Zone::Hand);
    t.g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    run(t, yes, targets, orders)
}

fn creatures(t: &T, p: usize) -> usize { t.g.battlefield.iter().filter(|o| t.g.controller(**o) == p).count() }

fn energy(t: &T, p: usize) -> u32 { t.g.counters(Target::Player(p)).get("energy").copied().unwrap_or(0) }

/// "Sacrifice two creatures. If you can't, …": a cost done wholly or not at all (118.12,
/// 118.3), as Egon's ruling reads its "exile two cards": with one creature, it stays.
#[test]
fn a_mandatory_sacrifice_of_two_with_one_creature_sacrifices_nothing() {
    for (text, outcome) in [("Sacrifice two creatures. If you can't, you lose 3 life.", -3), ("Sacrifice two creatures. If you do, you gain 3 life.", 3)] {
        for had in [0, 1, 2] {
            let mut t = game(&["Grizzly Bears"], &[spell(text)]);
            main_phase(&mut t);
            let arrivals: Vec<_> = (0..had).map(|_| put(&mut t,0,"Grizzly Bears",Zone::Battlefield)).collect();
            let orders = if had == 2 { vec![(0, vec![arrivals[1],arrivals[0]])] } else { vec![] };
            cast(&mut t, true, &[], &orders);
            let paid = had == 2;
            cite!(["118.12", "118.3"]; case: format!("{text} {had}"); assert_eq!(creatures(&t, 0), if paid { 0 } else { had }));
            let life = 20 + match (outcome > 0, paid) { (true, true) | (false, false) => outcome, _ => 0 };
            assert_eq!(t.g.players[0].life, life);
        }
    }
}

/// The same for "Discard two cards. If you can't, …" with one card in hand (118.12, 118.3).
#[test]
fn a_mandatory_discard_of_two_with_one_card_discards_nothing() {
    for (text, outcome) in [("Discard two cards. If you can't, you lose 3 life.", -3), ("Discard two cards. If you do, you gain 3 life.", 3)] {
        for had in [0, 1, 2] {
            let mut t = game(&["Wastes"], &[spell(text)]);
            main_phase(&mut t);
            let arrivals: Vec<_> = (0..had).map(|_| put(&mut t,0,"Wastes",Zone::Hand)).collect();
            let orders = if had == 2 { vec![(0, vec![arrivals[1],arrivals[0]])] } else { vec![] };
            cast(&mut t, true, &[], &orders);
            let paid = had == 2;
            cite!(["118.12", "118.3"]; case: format!("{text} {had}"); assert_eq!(t.g.players[0].hand.len(), if paid { 0 } else { had }));
            let life = 20 + match (outcome > 0, paid) { (true, true) | (false, false) => outcome, _ => 0 };
            assert_eq!(t.g.players[0].life, life);
        }
    }
}

/// "Mill three cards. If you can't, …" with two cards in the library: the act is a cost
/// (118.12) and "the player can't pay a cost that includes milling a number of cards greater
/// than the number of cards in their library" (701.17b).
#[test]
fn a_mill_of_three_as_a_cost_with_two_cards_mills_nothing() {
    for library in [2, 3] {
        let mut t = game(&[], &[spell("Mill three cards. If you can't, you lose 3 life.")]);
        main_phase(&mut t);
        while t.g.players[0].library.len() > library { let top = t.g.players[0].library[0]; t.g.move_object(top, Zone::Exile); }
        let orders = if library == 3 { vec![(0, t.g.players[0].library.clone())] } else { vec![] };
        cast(&mut t, true, &[], &orders);
        let paid = library >= 3;
        cite!(["118.12", "701.17b"]; case: library; assert_eq!(t.g.players[0].life, if paid { 20 } else { 17 }));
        assert_eq!(t.g.players[0].library.len(), if paid { 0 } else { library }, "nothing milled when it can't be paid");
    }
}

/// "When you do" isn't 118.12's "if": the reflexive ability triggers only if the act was
/// done as it says (603.12). Whether the one object goes is left open here.
#[test]
fn a_two_object_act_done_in_part_does_not_trigger_when_you_do() {
    for (text, setup) in [("Sacrifice two creatures. When you do, you gain 3 life.", "Grizzly Bears"), ("Discard two cards. When you do, you gain 3 life.", "Wastes")] {
        for had in [1, 2] {
            let mut t = game(&["Grizzly Bears"], &[spell(text)]);
            main_phase(&mut t);
            let zone = if setup == "Wastes" { Zone::Hand } else { Zone::Battlefield };
            let arrivals: Vec<_> = (0..had).map(|_| put(&mut t,0,setup,zone)).collect();
            let orders = if had == 2 { vec![(0, vec![arrivals[1],arrivals[0]])] } else { vec![] };
            cast(&mut t, true, &[], &orders);
            cite!(["603.12"]; case: format!("{text} {had}"); assert_eq!(t.g.players[0].life, if had == 2 { 23 } else { 20 }));
        }
    }
}

/// Wick's Patrol: "When this creature enters, mill three cards. When you do, target creature an
/// opponent controls gets -X/-X until end of turn, …". Ruling 2024-07-26: "If your library has
/// two or fewer cards in it when Wick's Patrol's ability resolves, you'll mill as many cards
/// as you can, but the reflexive ability won't trigger." 701.17b: "If instructed to do so,
/// they mill as many as possible."
#[test]
fn wicks_patrol_mills_what_it_can_and_triggers_only_on_three() {
    for library in [2, 3] {
        let mut t = game(&["Wick's Patrol", "Grizzly Bears"], &[]);
        main_phase(&mut t);
        let theirs = put(&mut t, 1, "Grizzly Bears", Zone::Battlefield);
        // a library of Grizzly Bears: milled, X is 2
        let library_cards: Vec<ObjId> = t.g.players[0].library.clone();
        for o in library_cards { t.g.move_object(o, Zone::Exile); }
        let milled: Vec<_> = (0..library).map(|_| put(&mut t,0,"Grizzly Bears",Zone::Library)).collect();
        let patrol = put(&mut t, 0, "Wick's Patrol", Zone::Hand);
        t.mana(0, "{4}{B}{B}");
        t.g.act(0, paid("CCCCBB", Move::Cast { card: patrol, targets: vec![] })).unwrap();
        let asked = run(&mut t, true, &[Target::Object(theirs)], &[(0,milled)]);
        let triggered = library >= 3;
        cite!(["603.12", "701.17b"]; case: library; assert_eq!(said(&asked, "targets"), triggered, "{asked:?}"));
        assert_eq!(t.g.players[0].graveyard.len(), library, "it mills as many as possible");
        assert_eq!(t.g.obj(theirs).zone == Zone::Battlefield, !triggered, "-2/-2 kills the 2/2");
    }
}

/// Tether Technician: "When this creature enters, you may discard a card. When you do, this
/// creature deals 2 damage to any target." The reflexive ability goes on the stack after the
/// resolution, and the opponent may respond to it (603.3, 603.12); the Technician gone in
/// response, it still deals the damage, as it last existed (113.7a).
#[test]
fn tether_technician_can_be_responded_to_and_deals_damage_gone() {
    let mut t = game(&["Tether Technician", "Wastes"], &[]);
    main_phase(&mut t);
    put(&mut t, 0, "Wastes", Zone::Hand);
    let tech = put(&mut t, 0, "Tether Technician", Zone::Hand);
    t.mana(0, "{4}{R}");
    t.g.act(0, paid("CCCCR", Move::Cast { card: tech, targets: vec![] })).unwrap();
    t.g.act(0, Move::Pass).unwrap();
    t.g.act(1, Move::Pass).unwrap();
    // the enters trigger
    t.g.act(0, Move::Pass).unwrap();
    t.g.act(1, Move::Pass).unwrap();
    assert!(matches!(t.g.asked(), Some((0, Asked::May))), "{:?}", t.g.asked());
    t.g.act(0, Move::Choose(Answer::Yes)).unwrap();
    if let Some((_, Asked::Objects { among, .. } | Asked::Discard { among, .. })) = t.g.asked() { t.g.act(0, Move::Choose(Answer::Objects(vec![among[0]]))).unwrap(); }
    assert!(matches!(t.g.asked(), Some((0, Asked::Targets { .. }))), "{:?}", t.g.asked());
    t.g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap();
    let ability = *t.g.stack.last().unwrap();
    cite!(["603.12", "603.3"]; assert_eq!(t.g.stack.len(), 1));
    assert_eq!(t.g.controller(ability), 0);
    t.g.act(0, Move::Pass).unwrap();
    // the opponent has priority with it on the stack: they may respond (117.3d)
    assert!(matches!(t.g.asked(), Some((1, Asked::Priority))), "{:?}", t.g.asked());
    let on = t.find_in_battlefield("Tether Technician");
    t.g.move_object(on, Zone::Hand);
    run(&mut t, true, &[], &[]);
    cite!(["603.12", "113.7a"]; assert_eq!(t.g.players[1].life, 18));
}

/// Tether Technician with an empty hand: nothing can be discarded (118.3), so it doesn't
/// trigger; nor when it is declined.
#[test]
fn tether_technician_with_an_empty_hand_or_declined_does_not_trigger() {
    for (cards, yes) in [(0, true), (1, false)] {
        let mut t = game(&["Tether Technician", "Wastes"], &[]);
        main_phase(&mut t);
        for _ in 0..cards { put(&mut t, 0, "Wastes", Zone::Hand); }
        let tech = put(&mut t, 0, "Tether Technician", Zone::Hand);
        t.mana(0, "{4}{R}");
        t.g.act(0, paid("CCCCR", Move::Cast { card: tech, targets: vec![] })).unwrap();
        let asked = run(&mut t, yes, &[Target::Player(1)], &[]);
        cite!(["603.12", "118.3"]; case: format!("{cards} {yes}"); assert!(!said(&asked, "targets"), "{asked:?}"));
        assert_eq!(t.g.players[1].life, 20);
        assert_eq!(t.g.players[0].hand.len(), cards);
    }
}

/// The Fire Nation Drill: "When The Fire Nation Drill enters, you may tap it. When you do,
/// destroy target creature with power 4 or less." The Drill leaving in response doesn't stop
/// the reflexive ability, which has triggered (603.12).
#[test]
fn the_fire_nation_drill_destroys_even_if_the_drill_leaves_in_response() {
    for gone in [false, true] {
        let mut t = game(&["The Fire Nation Drill", "Grizzly Bears"], &[]);
        main_phase(&mut t);
        let theirs = put(&mut t, 1, "Grizzly Bears", Zone::Battlefield);
        let drill = put(&mut t, 0, "The Fire Nation Drill", Zone::Hand);
        t.mana(0, "{2}{B}{B}");
        t.g.act(0, paid("CCBB", Move::Cast { card: drill, targets: vec![] })).unwrap();
        for _ in 0..4 { t.g.act(t.g.priority, Move::Pass).unwrap(); }
        t.g.act(0, Move::Choose(Answer::Yes)).unwrap();
        assert!(matches!(t.g.asked(), Some((0, Asked::Targets { .. }))), "{:?}", t.g.asked());
        t.g.act(0, Move::ChooseTargets(vec![vec![Target::Object(theirs)]])).unwrap();
        let on = t.find_in_battlefield("The Fire Nation Drill");
        cite!(["603.12"]; case: gone; assert!(t.g.obj(on).tapped));
        if gone { t.g.move_object(on, Zone::Graveyard); }
        run(&mut t, true, &[], &[]);
        assert_ne!(t.g.obj(theirs).zone, Zone::Battlefield);
    }
}

/// The reflexive ability is controlled by whoever controlled the resolving ability (603.12,
/// 603.7d by way of 603.12): player 1 activates their own permanent's ability on player 0's
/// turn, chooses its targets and gains the life.
#[test]
fn the_reflexive_ability_is_controlled_by_the_resolving_abilitys_controller() {
    let texts = [("Spark", "{0}", "Creature — Elemental", "{0}: You may discard a card. When you do, you gain 3 life and this creature deals 2 damage to target player.", Some((1, 1)))];
    let mut t = game(&["Wastes"], &texts);
    main_phase(&mut t);
    let spark = put(&mut t, 1, "Spark", Zone::Battlefield);
    put(&mut t, 1, "Wastes", Zone::Hand);
    t.g.act(0, Move::Pass).unwrap();
    t.g.act(1, Move::Activate { source: spark, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    t.g.act(1, Move::Pass).unwrap();
    t.g.act(0, Move::Pass).unwrap();
    assert!(matches!(t.g.asked(), Some((1, Asked::May))), "{:?}", t.g.asked());
    t.g.act(1, Move::Choose(Answer::Yes)).unwrap();
    if let Some((_, Asked::Objects { among, .. } | Asked::Discard { among, .. })) = t.g.asked() { t.g.act(1, Move::Choose(Answer::Objects(vec![among[0]]))).unwrap(); }
    cite!(["603.12"]; assert!(matches!(t.g.asked(), Some((1, Asked::Targets { .. }))), "{:?}", t.g.asked()));
    t.g.act(1, Move::ChooseTargets(vec![vec![Target::Player(0)]])).unwrap();
    assert_eq!(t.g.controller(*t.g.stack.last().unwrap()), 1);
    run(&mut t, true, &[], &[]);
    assert_eq!((t.g.players[0].life, t.g.players[1].life), (18, 23));
}

/// 603.12a: "if the trigger event or events occur multiple times during the resolution of
/// the spell or ability that created it, the reflexive triggered ability will trigger once
/// for each of those times." Two nonland cards discarded at once are two occurrences of
/// "you discard a nonland card" (603.2c).
#[test]
fn two_nonland_cards_discarded_this_way_trigger_it_twice() {
    let text = "Draw two cards, then discard two cards. When you discard a nonland card this way, you gain 3 life.";
    let d = CardDef::new("Parent", "{0}", "Instant", text, None).unwrap();
    if let Err(e) = game::play::check(&d) { panic!("refused: {}", e.0); }
    for nonland in [0, 1, 2] {
        let mut t = game(&["Grizzly Bears"], &[spell(text)]);
        main_phase(&mut t);
        // the library's Wastes are drawn; the hand's Bears are discarded first
        for _ in 0..nonland { put(&mut t, 0, "Grizzly Bears", Zone::Hand); }
        let card = put(&mut t, 0, "Parent", Zone::Hand);
        t.g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
        t.g.act(0, Move::Pass).unwrap();
        t.g.act(1, Move::Pass).unwrap();
        // discard the Bears and then Wastes
        let mut discarded = t.g.players[0].hand.clone();
        if let Some((_, Asked::Objects { among, count, .. } | Asked::Discard { among, count })) = t.g.asked() {
            let mut pick: Vec<ObjId> = among.iter().copied().filter(|o| t.g.obj(*o).chars.name == "Grizzly Bears").collect();
            pick.extend(among.iter().copied().filter(|o| t.g.obj(*o).chars.name == "Wastes"));
            pick.truncate(count);
            discarded = pick.clone();
            t.g.act(0, Move::Choose(Answer::Objects(pick))).unwrap();
        }
        run(&mut t, true, &[], &[(0,vec![discarded[1],discarded[0]])]);
        cite!(["603.12a", "603.2c"]; case: nonland; assert_eq!(t.g.players[0].life, 20 + 3 * nonland as i32));
    }
}

/// An optional act done once for each player would repeat one producer in one resolution
/// (603.12a), which the engine doesn't do: each such text is refused outright, whether
/// "if you do" or "when you do", optional or not.
#[test]
fn an_act_for_each_player_with_an_outcome_is_refused_outright() {
    for text in ["For each player, you may sacrifice a creature. When you do, you gain 1 life.",
                 "For each opponent, you may pay {E}. If you do, you gain 1 life.",
                 "For each opponent, mill a card. When you do, you gain 1 life.",
                 "For each opponent, discard a card. When you do, you gain 1 life.",
                 "For each opponent, create a 1/1 white Soldier creature token. When you do, you gain 1 life.",
                 "For each opponent, you may tap target creature. When you do, you gain 1 life.",
                 "Each player may discard a card. When you do, you gain 3 life."] {
        let d = CardDef::new("Parent", "{0}", "Instant", text, None).unwrap();
        // Uncited (admission): per-player outcome producers are outside the retained scope.
        let reason = if text.starts_with("Each") { "reflexive: producer must be this resolution’s controller" }
            else if text.starts_with("Search") { "reflexive: not one instruction it is said of, done whatever happens" }
            else { "for each player: an act the engine does not do for each of them alike" };
        crate::support::refused(game::play::check(&d).map_err(|e|e.0), reason);
    }
    // two connives or two searches: one instruction "this way" is said of, or refused
    for text in ["Search your library for a basic land card, put it into your hand, then shuffle. Search your library for a card, put it into your hand, then shuffle. When you search your library this way, you gain 3 life."] {
        let d = CardDef::new("Parent", "{0}", "Instant", text, None).unwrap();
        // Uncited (admission): per-player outcome producers are outside the retained scope.
        let reason = if text.starts_with("Each") { "reflexive: producer must be this resolution’s controller" }
            else if text.starts_with("Search") { "reflexive: not one instruction it is said of, done whatever happens" }
            else { "for each player: an act the engine does not do for each of them alike" };
        crate::support::refused(game::play::check(&d).map_err(|e|e.0), reason);
    }
}

/// 118.12: "If [that player] [does, doesn't, or can't]" checks "whether the player chose to
/// pay an optional cost or started to pay a mandatory cost". Declining isn't being unable:
/// "if you can't" follows only an impossible act, "if you don't" both.
#[test]
fn if_you_dont_and_if_you_cant_after_declining_or_being_unable() {
    // (text, cards in hand, yes): life lost
    let cases = [("You may discard a card. If you don't, you lose 3 life.", 1, true, 0),
                 ("You may discard a card. If you don't, you lose 3 life.", 1, false, 3),
                 ("You may discard a card. If you don't, you lose 3 life.", 0, true, 3),
                 ("You may discard a card. If you can't, you lose 3 life.", 1, false, 0),
                 ("You may discard a card. If you can't, you lose 3 life.", 0, true, 3),
                 ("You may discard a card. If you can't, you lose 3 life.", 1, true, 0),
                 ("Discard a card. If you don't, you lose 3 life.", 0, true, 3),
                 ("Discard a card. If you don't, you lose 3 life.", 1, true, 0)];
    for (text, cards, yes, lost) in cases {
        let mut t = game(&["Wastes"], &[spell(text)]);
        main_phase(&mut t);
        for _ in 0..cards { put(&mut t, 0, "Wastes", Zone::Hand); }
        cast(&mut t, yes, &[], &[]);
        cite!(["118.12"]; case: format!("{text} {cards} {yes}"); assert_eq!(t.g.players[0].life, 20 - lost));
    }
}

/// Paying {E}{E} with one energy: it can't be paid (118.3, 107.14: "To pay {E}, a player
/// removes one energy counter from themselves"), so nothing is removed and nothing triggers.
#[test]
fn two_energy_with_one_is_not_paid_and_nothing_is_removed() {
    for text in ["Pay {E}{E}. When you do, you gain 3 life.", "You may pay {E}{E}. When you do, you gain 3 life.", "Pay {E}{E}. If you do, you gain 3 life."] {
        for had in [1, 2] {
            let mut t = game(&[], &[spell(text)]);
            main_phase(&mut t);
            t.g.put_counters(Target::Player(0), "energy", had);
            cast(&mut t, true, &[], &[]);
            cite!(["118.3", "107.14", "603.12"]; case: format!("{text} {had}"); assert_eq!(energy(&t, 0), if had == 2 { 0 } else { 1 }));
            assert_eq!(t.g.players[0].life, if had == 2 { 23 } else { 20 });
        }
    }
    // "Pay {E}. If you can't": with none, the other instruction
    let mut t = game(&[], &[spell("Pay {E}. If you can't, you lose 3 life.")]);
    main_phase(&mut t);
    cast(&mut t, true, &[], &[]);
    cite!(["118.12", "107.14"]; assert_eq!(t.g.players[0].life, 17));
}

/// Riveteers Overlook: "When this land enters, sacrifice it. When you do, search your library
/// for a basic Swamp, Mountain, or Forest card, put it onto the battlefield tapped, then
/// shuffle and you gain 1 life." Its search finding nothing, it still gains the life: that is
/// the reflexive ability's, which triggered on the sacrifice (603.12); the search finds what
/// there is (701.23a).
#[test]
fn riveteers_overlook_with_no_basic_land_still_gains_the_life() {
    let mut t = game(&["Riveteers Overlook"], &[]);
    main_phase(&mut t);
    let overlook = put(&mut t, 0, "Riveteers Overlook", Zone::Hand);
    t.g.act(0, Move::PlayLand(overlook)).unwrap();
    run(&mut t, true, &[], &[]);
    cite!(["603.12", "701.23a"]; assert_eq!(t.g.players[0].life, 21));
    assert!(t.g.battlefield.is_empty());
}


/// Narset of the Ancient Way: "−2: Draw a card, then you may discard a card. When you discard
/// a nonland card this way, Narset deals damage equal to that card's mana value to target
/// creature or planeswalker." "That card" is the card discarded this way (603.12), not the
/// target: Grizzly Bears deals 2 to Colossal Dreadmaw (mana value 6).
#[test]
fn narset_deals_damage_equal_to_the_discarded_cards_mana_value() {
    for (discard, damage) in [("Grizzly Bears", 2), ("Hill Giant", 4), ("Wastes", 0)] {
        let mut t = game(&["Narset of the Ancient Way", "Hill Giant", "Grizzly Bears", "Colossal Dreadmaw"], &[]);
        main_phase(&mut t);
        let theirs = put(&mut t, 1, "Colossal Dreadmaw", Zone::Battlefield);
        let narset = put(&mut t, 0, "Narset of the Ancient Way", Zone::Battlefield);
        t.g.set_initial_counters(narset, "loyalty", 5);
        let card = put(&mut t, 0, discard, Zone::Hand);
        t.g.act(0, Move::Activate { source: narset, ability: 1, targets: vec![], paying: vec![] }).unwrap();
        t.g.act(0, Move::Pass).unwrap();
        t.g.act(1, Move::Pass).unwrap();
        t.g.act(0, Move::Choose(Answer::Yes)).unwrap();
        t.g.act(0, Move::Choose(Answer::Objects(vec![card]))).unwrap();
        let asked = run(&mut t, true, &[Target::Object(theirs)], &[]);
        assert_eq!(said(&asked, "targets"), damage > 0, "{asked:?}");
        cite!(["603.12"]; case: discard; assert_eq!(t.g.obj(theirs).damage, damage));
    }
}

/// Flamethrower Sonata (the back face of Torrent Sculptor, as a card of its own): "Discard a
/// card, then draw a card. When you discard an instant or sorcery card this way, Flamethrower
/// Sonata deals damage equal to that card's mana value to target creature or planeswalker you
/// don't control." Lightning Bolt discarded: 1 damage, not the Sonata's 2.
#[test]
fn flamethrower_sonata_deals_damage_equal_to_the_discarded_cards_mana_value() {
    let text = "Discard a card, then draw a card. When you discard an instant or sorcery card this way, Flamethrower Sonata deals damage equal to that card's mana value to target creature or planeswalker you don't control.";
    let mut t = game(&["Lightning Bolt", "Colossal Dreadmaw"], &[("Flamethrower Sonata", "{1}{R}", "Sorcery", text, None)]);
    main_phase(&mut t);
    let theirs = put(&mut t, 1, "Colossal Dreadmaw", Zone::Battlefield);
    put(&mut t, 0, "Lightning Bolt", Zone::Hand);
    let sonata = put(&mut t, 0, "Flamethrower Sonata", Zone::Hand);
    t.mana(0, "{1}{R}");
    t.g.act(0, paid("CR", Move::Cast { card: sonata, targets: vec![] })).unwrap();
    let asked = run(&mut t, true, &[Target::Object(theirs)], &[]);
    assert!(said(&asked, "targets"), "{asked:?}");
    cite!(["603.12"]; assert_eq!(t.g.obj(theirs).damage, 1));
}

/// Cait Sith, Fortune Teller: "At the beginning of combat on your turn, scry 1, then exile the
/// top card of your library. You may play that card this turn. When you exile a card this
/// way, target creature you control gets +X/+0 until end of turn, where X is that card's mana
/// value." Hill Giant exiled (mana value 4): Grizzly Bears (mana value 2) becomes 6/2.
#[test]
fn cait_sith_gives_the_exiled_cards_mana_value() {
    let mut t = game(&["Cait Sith, Fortune Teller", "Hill Giant", "Grizzly Bears"], &[]);
    main_phase(&mut t);
    put(&mut t, 0, "Cait Sith, Fortune Teller", Zone::Battlefield);
    let bears = put(&mut t, 0, "Grizzly Bears", Zone::Battlefield);
    let library: Vec<ObjId> = t.g.players[0].library.clone();
    for o in library { t.g.move_object(o, Zone::Exile); }
    put(&mut t, 0, "Hill Giant", Zone::Library);
    let top = t.g.players[0].library[0];
    assert_eq!(t.g.obj(top).chars.name, "Hill Giant", "put on top");
    while t.g.part != Part::BeginningOfCombat { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    let asked = run(&mut t, true, &[Target::Object(bears)], &[]);
    assert!(said(&asked, "targets"), "{asked:?}");
    cite!(["603.12"]; assert_eq!(t.g.power(bears), Some(6)));
}

/// Wick's Patrol's X: "the greatest mana value among cards in your graveyard", counted as the
/// reflexive ability resolves (ruling 2024-07-26): three Hill Giants milled (mana value 4),
/// Colossal Dreadmaw (mana value 6) becomes 2/2.
#[test]
fn wicks_patrol_x_is_the_greatest_mana_value_in_the_graveyard() {
    let mut t = game(&["Wick's Patrol", "Hill Giant", "Colossal Dreadmaw"], &[]);
    main_phase(&mut t);
    let theirs = put(&mut t, 1, "Colossal Dreadmaw", Zone::Battlefield);
    let library: Vec<ObjId> = t.g.players[0].library.clone();
    for o in library { t.g.move_object(o, Zone::Exile); }
    let milled: Vec<_> = (0..3).map(|_| put(&mut t,0,"Hill Giant",Zone::Library)).collect();
    let patrol = put(&mut t, 0, "Wick's Patrol", Zone::Hand);
    t.mana(0, "{4}{B}{B}");
    t.g.act(0, paid("CCCCBB", Move::Cast { card: patrol, targets: vec![] })).unwrap();
    let asked = run(&mut t, true, &[Target::Object(theirs)], &[(0,milled)]);
    assert!(said(&asked, "targets"), "{asked:?}");
    cite!(["603.12"]; assert_eq!((t.g.power(theirs), t.g.toughness(theirs)), (Some(2), Some(2))));
}

/// The Ancient One: "{2}{U}{B}: Draw a card, then discard a card. When you discard a card this
/// way, target player mills cards equal to its mana value." "Its" is the discarded card's
/// (603.12), not The Ancient One's own mana value 2: Hill Giant discarded mills four.
#[test]
fn the_ancient_one_mills_the_discarded_cards_mana_value() {
    for (discard, milled) in [("Hill Giant", 4), ("Wastes", 0)] {
        let mut t = game(&["The Ancient One", "Hill Giant"], &[]);
        main_phase(&mut t);
        let one = put(&mut t, 0, "The Ancient One", Zone::Battlefield);
        // the drawn card is a Wastes: discard the chosen card, which is first in hand
        while t.g.players[0].hand.len() > 0 { let c = t.g.players[0].hand[0]; t.g.move_object(c, Zone::Exile); }
        put(&mut t, 0, discard, Zone::Hand);
        let before = t.g.players[1].library.len();
        let orders = if milled > 1 { vec![(1,t.g.players[1].library.iter().rev().take(milled).copied().collect())] } else { vec![] };
        t.mana(0, "{2}{U}{B}");
        let ability = t.g.def(one).unwrap().abilities().iter().position(|a| matches!(a, ir::Ability::Activated { .. })).unwrap();
        t.g.act(0, paid("CCUB", Move::Activate { source: one, ability, targets: vec![], paying: vec![] })).unwrap();
        let asked = run(&mut t, true, &[Target::Player(1)], &orders);
        assert_eq!(said(&asked, "targets"), true, "{asked:?}");
        cite!(["603.12"]; case: discard; assert_eq!(before - t.g.players[1].library.len(), milled));
    }
}

/// "You may mill three cards. If you do, you gain 3 life." with two cards in the library: the
/// mill is a cost (118.12) that can't be paid (701.17b), so nothing is milled and no life.
#[test]
fn an_optional_mill_as_a_cost_with_too_few_cards_does_nothing() {
    for library in [2, 3] {
        let mut t = game(&[], &[spell("You may mill three cards. If you do, you gain 3 life.")]);
        main_phase(&mut t);
        while t.g.players[0].library.len() > library { let top = t.g.players[0].library[0]; t.g.move_object(top, Zone::Exile); }
        let orders = if library == 3 { vec![(0, t.g.players[0].library.clone())] } else { vec![] };
        cast(&mut t, true, &[], &orders);
        let paid = library >= 3;
        cite!(["118.12", "701.17b"]; case: library; assert_eq!((t.g.players[0].life, t.g.players[0].library.len()), if paid { (23, 0) } else { (20, library) }));
    }
}
