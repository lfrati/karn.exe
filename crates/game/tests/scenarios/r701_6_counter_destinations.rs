//! Counter destination self-replacements apply during the counter (614.15), before other
//! replacements; a failed counter does not suppress later unconditional instructions.
use crate::support::{Board, T, cite, mana_pool, paid};
use game::state::{Answer, Choice, Decision};
use game::{Event, Move, ObjId, Target, Zone};

fn setup(counter: &str, target: &str, replacement: bool) -> T {
    let mut board = Board::new(2).hand(0, &[target]).hand(1, &[counter])
        .library(0, &["Island"]).library(1, &["Forest"]);
    if replacement { board = board.battlefield(0, &["Rest in Peace"]); }
    board.build()
}
fn announce(t: &mut T, counter: &str, target: &str, spending: &str) -> (ObjId, ObjId) {
    let card = t.find_in(0, Zone::Hand, target);
    t.g.players[0].pool = mana_pool(spending);
    t.g.act(0, paid(spending, Move::Cast {card, targets: vec![]})).unwrap();
    let target = *t.g.stack.last().unwrap();
    t.g.act(0, Move::Pass).unwrap();
    let card = t.find_in(1, Zone::Hand, counter);
    let spending = match counter {"Lapse of Certainty" => "WWW", "Spell Crumple" => "UUU", _ => "UU"};
    t.g.players[1].pool = mana_pool(spending);
    t.g.act(1, paid(spending, Move::Cast {card, targets: vec![vec![Target::Object(target)]]})).unwrap();
    (target, *t.g.stack.last().unwrap())
}

#[test]
fn r701_6a_remand_returns_the_countered_spell_and_draws_a_card() {
    let mut t = setup("Remand", "Grizzly Bears", false);
    let (target, _) = announce(&mut t, "Remand", "Grizzly Bears", "GG");
    let mut fork = T::new(t.g.clone());
    for t in [&mut t, &mut fork] {
        t.pass_all();
        cite!(["701.6a"]; assert!(t.g.stack.is_empty()));
        cite!(["614.15", "400.7"]; assert_eq!(t.g.obj(t.g.players[0].hand[0]).chars.name, "Grizzly Bears"));
        cite!(["608.2c", "121.1"]; assert_eq!(t.g.obj(t.g.players[1].hand[0]).chars.name, "Forest"));
        cite!(["701.6a"]; assert!(t.g.log.contains(&Event::Countered {object: target})));
        assert!(!t.g.log.contains(&Event::Resolved {spell: target}));
    }
}

#[test]
fn r614_15_memory_lapse_puts_the_countered_spell_above_the_existing_library() {
    let mut t = setup("Memory Lapse", "Grizzly Bears", false);
    announce(&mut t, "Memory Lapse", "Grizzly Bears", "GG");
    t.pass_all();
    cite!(["701.6a"]; assert!(t.g.stack.is_empty()));
    cite!(["614.15", "401.2"]; assert_eq!(t.g.obj(*t.g.players[0].library.last().unwrap()).chars.name, "Grizzly Bears"));
    assert_eq!(t.g.players[0].library.len(), 2);
    assert!(t.g.players[0].graveyard.is_empty());
    assert!(t.g.players[1].hand.is_empty());
}

#[test]
fn r614_15_lapse_of_certainty_uses_the_same_top_destination() {
    let mut t = setup("Lapse of Certainty", "Grizzly Bears", false);
    announce(&mut t, "Lapse of Certainty", "Grizzly Bears", "GG");
    t.pass_all();
    cite!(["614.15", "401.2"]; assert_eq!(t.g.obj(*t.g.players[0].library.last().unwrap()).chars.name, "Grizzly Bears"));
    assert!(t.g.stack.is_empty() && t.g.players[0].graveyard.is_empty());
}

#[test]
fn r608_2c_spell_crumple_puts_itself_on_the_bottom_even_if_countering_is_prohibited() {
    for (target, spending, protected) in [("Grizzly Bears", "GG", false), ("Supreme Verdict", "WWUU", true)] {
        let mut t = setup("Spell Crumple", target, false);
        let (target, _) = announce(&mut t, "Spell Crumple", target, spending);
        t.pass_all();
        cite!(["608.2c", "401.2"]; assert_eq!(t.g.obj(t.g.players[1].library[0]).chars.name, "Spell Crumple"));
        cite!(["701.6a", "101.2"]; assert_eq!(t.g.stack, if protected {vec![target]} else {vec![]}));
        if !protected {
            cite!(["614.15", "401.2"]; assert_eq!(t.g.obj(t.g.players[0].library[0]).chars.name, "Grizzly Bears"));
        }
    }
}

#[test]
fn r101_2_uncounterable_spells_stay_on_stack_but_remands_draw_still_happens() {
    for counter in ["Remand", "Memory Lapse"] {
        let mut t = setup(counter, "Supreme Verdict", false);
        let (target, _) = announce(&mut t, counter, "Supreme Verdict", "WWUU");
        t.pass_all();
        // Pinned Remand ruling 91146a225347f5c9: legal target, failed counter, still draw.
        cite!(["101.2", "701.6a"]; assert_eq!(t.g.stack, vec![target]));
        cite!(["608.2c"]; assert_eq!(t.g.players[1].hand.len(), usize::from(counter == "Remand")));
        assert!(!t.g.log.contains(&Event::Countered {object: target}));
        t.pass_all();
        cite!(["614.15"]; assert_eq!(t.g.obj(t.g.players[0].graveyard[0]).chars.name, "Supreme Verdict"));
        assert!(t.g.players[0].hand.is_empty());
    }
}

#[test]
fn r616_1a_counter_self_replacements_apply_before_rest_in_peace() {
    for counter in ["Remand", "Memory Lapse"] {
        let mut t = setup(counter, "Grizzly Bears", true);
        announce(&mut t, counter, "Grizzly Bears", "GG");
        t.pass_all();
        cite!(["614.15", "616.1a"]; assert!(t.g.decision.is_none()));
        let destination = if counter == "Remand" {&t.g.players[0].hand} else {&t.g.players[0].library};
        cite!(["614.15", "616.1a"]; assert_eq!(t.g.obj(*destination.last().unwrap()).chars.name, "Grizzly Bears"));
        // Rest in Peace still replaces the counterspell's own normal completion.
        assert_eq!(t.g.exile.len(), 1);
        assert_eq!(t.g.obj(t.g.exile[0]).chars.name, counter);
    }
}

#[test]
fn r608_2b_remand_does_not_draw_when_its_only_target_is_already_gone() {
    let mut t = Board::new(2).hand(0, &["Grizzly Bears", "Counterspell"])
        .hand(1, &["Remand"]).library(1, &["Forest"]).build();
    let (target, remand) = announce(&mut t, "Remand", "Grizzly Bears", "GG");
    t.g.act(1, Move::Pass).unwrap();
    let card = t.find_in(0, Zone::Hand, "Counterspell");
    t.g.players[0].pool = mana_pool("UU");
    t.g.act(0, paid("UU", Move::Cast {card, targets: vec![vec![target.into()]]})).unwrap();
    t.pass_all();
    assert_eq!(t.g.stack, vec![remand]);
    t.pass_all();
    cite!(["608.2b"]; assert!(t.g.log.contains(&Event::Fizzled {spell: remand})));
    cite!(["608.2b"]; assert!(t.g.players[1].hand.is_empty()));
    assert_eq!(t.g.players[1].library.len(), 1);
}

#[test]
fn r702_34a_flashback_still_exiles_after_a_counter_self_replaces_its_destination() {
    for counter in ["Remand", "Memory Lapse"] {
        let mut t = Board::new(2).graveyard(0, &["Think Twice"]).hand(1, &[counter])
            .library(0, &["Island"]).library(1, &["Forest"]).build();
        let card = t.find_in(0, Zone::Graveyard, "Think Twice");
        t.g.players[0].pool = mana_pool("UUU");
        t.g.act(0, paid("UUU", Move::CastVia(game::announcement::CastRoute::Flashback,
            Box::new(Move::Cast {card, targets: vec![]})))).unwrap();
        let target = *t.g.stack.last().unwrap();
        t.g.act(0, Move::Pass).unwrap();
        let card = t.find_in(1, Zone::Hand, counter);
        t.g.players[1].pool = mana_pool("UU");
        t.g.act(1, paid("UU", Move::Cast {card, targets: vec![vec![target.into()]]})).unwrap();
        t.pass_all();
        // Pinned rulings: Remand65b0b9669c44c2c2, Memory Lapse ae77bd7628fe8ba1.
        cite!(["614.15", "702.34a"]; assert_eq!(t.g.obj(t.g.exile[0]).chars.name, "Think Twice"));
        assert!(t.g.stack.is_empty() && t.g.players[0].hand.is_empty());
        assert_eq!(t.g.players[0].library.len(), 1);
        cite!(["608.2c"]; assert_eq!(t.g.players[1].hand.len(), usize::from(counter == "Remand")));
    }
}

#[test]
fn r903_9b_counter_destination_and_result_survive_a_cloned_commander_choice() {
    for counter in ["Remand", "Memory Lapse"] {
        let mut t = Board::new(2).commander(0, "Isamaru, Hound of Konda").hand(1, &[counter])
            .library(0, &["Island"]).library(1, &["Forest"]).build();
        let commander = t.find_in(0, Zone::Command, "Isamaru, Hound of Konda");
        t.g.players[0].pool = mana_pool("W");
        t.g.act(0, paid("W", Move::Cast {card: commander, targets: vec![]})).unwrap();
        let target = *t.g.stack.last().unwrap();
        t.g.act(0, Move::Pass).unwrap();
        let card = t.find_in(1, Zone::Hand, counter);
        t.g.players[1].pool = mana_pool("UU");
        t.g.act(1, paid("UU", Move::Cast {card, targets: vec![vec![target.into()]]})).unwrap();
        t.pass_all();
        cite!(["903.9b"]; assert!(matches!(t.g.decision,
            Some(Decision::Choose {player: 0, choice: Choice::CommandZone {object}}) if object == target)));
        assert!(t.g.players[1].hand.is_empty()); // Remand's draw is still waiting.
        for home in [false, true] {
            let mut branch = t.g.clone();
            let before = format!("{branch:?}");
            cite!(["903.9b"]; refused("that decision is another player's"):
                branch.act(1, Move::Choose(if home {Answer::Yes} else {Answer::No})));
            assert_eq!(format!("{branch:?}"), before);
            branch.act(0, Move::Choose(if home {Answer::Yes} else {Answer::No})).unwrap();
            let destination = if home {&branch.command} else if counter == "Remand" {&branch.players[0].hand} else {&branch.players[0].library};
            let card = *destination.last().unwrap();
            cite!(["903.9b", "400.7"]; assert!(branch.is_commander(card) && branch.obj(card).chars.name == "Isamaru, Hound of Konda"));
            cite!(["608.2c"]; assert_eq!(branch.players[1].hand.len(), usize::from(counter == "Remand")));
            assert!(branch.stack.is_empty());
            cite!(["701.6a"]; assert_eq!(branch.log.iter().filter(|e| matches!(e, Event::Countered {object} if *object == target)).count(), 1));
        }
    }
}

#[test]
fn r701_6a_counter_outcomes_bind_to_their_own_input_rather_than_the_latest_counter() {
    // Authored boundary: two different producers, one successful and one protected.
    for instant_protected in [false, true] {
        let target = |name, kind, protected| game::CardDef::new(name, "{0}", kind,
            if protected {"This spell can't be countered.\nYou gain 3 life."} else {"You gain 3 life."}, None).unwrap();
        let defs = vec![target("First", "Instant", instant_protected), target("Second", "Sorcery", !instant_protected),
            game::CardDef::new("Two counters", "{0}", "Instant",
                "Counter target instant spell. Counter target sorcery spell. If that instant spell is countered this way, you gain 1 life.", None).unwrap()];
        for def in &defs { game::play::check(def).unwrap(); }
        let mut g = game::Game::new(defs, 2, 20);
        let instant = g.put(1, 0, Zone::Stack);
        let sorcery = g.put(1, 1, Zone::Stack);
        let card = g.put(0, 2, Zone::Hand);
        g.act(0, Move::Cast {card, targets: vec![vec![instant.into()], vec![sorcery.into()]]}).unwrap();
        let mut t = T::new(g);
        t.pass_all();
        cite!(["701.6a", "608.2c"]; assert_eq!(t.g.players[0].life, if instant_protected {20} else {21}));
        cite!(["701.6a", "101.2"]; assert_eq!(t.g.stack, vec![if instant_protected {instant} else {sorcery}]));
    }
}

#[test]
fn r115_1_a_counter_destination_spell_refuses_a_hand_target_without_state_changes() {
    let mut t = setup("Memory Lapse", "Grizzly Bears", false);
    t.g.act(0, Move::Pass).unwrap();
    let card = t.find_in(1, Zone::Hand, "Memory Lapse");
    let target = t.find_in(0, Zone::Hand, "Grizzly Bears");
    t.g.players[1].pool = mana_pool("UU");
    let before = format!("{:?}", t.g);
    cite!(["115.1"]; refused("an illegal target, Grizzly Bears: it isn't a spell (115.1)"):
        t.g.act(1, paid("UU", Move::Cast {card, targets: vec![vec![target.into()]]})));
    assert_eq!(format!("{:?}", t.g), before);
}

#[test]
fn r707_10a_countering_a_spell_copy_never_returns_the_original_card() {
    for counter in ["Remand", "Memory Lapse"] {
        let mut t = Board::new(2).hand(0, &["Shock", "Twincast"]).hand(1, &[counter])
            .library(0, &["Island"]).library(1, &["Forest"]).build();
        let card = t.find_in(0, Zone::Hand, "Shock");
        t.g.players[0].pool = mana_pool("RUU");
        t.g.act(0, paid("R", Move::Cast {card, targets: vec![vec![Target::Player(1)]]})).unwrap();
        let original = t.g.stack[0];
        let card = t.find_in(0, Zone::Hand, "Twincast");
        t.g.act(0, paid("UU", Move::Cast {card, targets: vec![vec![original.into()]]})).unwrap();
        t.pass_all();
        t.g.act(0, Move::Choose(Answer::Targets(vec![vec![Target::Player(1)]]))).unwrap();
        let copy = t.g.stack[1];
        t.g.act(0, Move::Pass).unwrap();
        let card = t.find_in(1, Zone::Hand, counter);
        t.g.players[1].pool = mana_pool("UU");
        t.g.act(1, paid("UU", Move::Cast {card, targets: vec![vec![copy.into()]]})).unwrap();
        t.pass_all();
        cite!(["707.10a", "701.6a"]; assert_eq!(t.g.stack, vec![original]));
        cite!(["704.5e"]; assert!(t.g.players[0].hand.is_empty() && t.g.players[0].library.len() == 1));
        cite!(["608.2c"]; assert_eq!(t.g.players[1].hand.len(), usize::from(counter == "Remand")));
        t.pass_all();
        cite!(["707.10a", "120.3a"]; assert_eq!(t.g.players[1].life, 18));
    }
}

#[test]
fn r608_2c_an_unsupported_counter_else_branch_is_refused_rather_than_dropped() {
    // Authored boundary: fusing the destination cannot erase a meaningful alternative.
    let def = game::CardDef::new("Counter alternative", "{0}", "Instant",
        "Counter target spell. If that spell is countered this way, put it into its owner's hand instead of into that player's graveyard. Otherwise, draw a card.", None).unwrap();
    cite!(["608.2c"]; assert_eq!(game::play::check(&def).unwrap_err().0, "modifier: InsteadOf of Move"));
}

#[test]
fn r608_2c_a_skipped_counter_supplies_an_empty_outcome_to_the_following_condition() {
    let defs = vec![
        game::CardDef::new("Guarded counter", "{0}", "Instant",
            "If it's your turn, counter target spell. If that spell is countered this way, you gain 1 life.", None).unwrap(),
        game::CardDef::new("Target", "{0}", "Instant", "You gain 3 life.", None).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap(); }
    for own_turn in [false, true] {
        let mut g = game::Game::new(defs.clone(), 2, 20);
        g.active = usize::from(!own_turn);
        let target = g.put(1, 1, Zone::Stack);
        let card = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast {card, targets: vec![vec![target.into()]]}).unwrap();
        let mut t = T::new(g);
        t.pass_all();
        cite!(["608.2c", "701.6a"]; assert_eq!(t.g.players[0].life, if own_turn {21} else {20}));
        cite!(["608.2c"]; assert_eq!(t.g.stack.is_empty(), own_turn));
    }
}
