//! Checks of player/1: information rights and allocation-independent labels.
use crate::support::cite;
use game::{CardDef, Game, Move, Target, Zone};
use game::legal::Asked;
use game::simulation::Simulation;

fn card(name: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    let d = CardDef::new(name, "{0}", types, text, pt).unwrap();
    game::play::check(&d).unwrap();
    d
}

fn revealed_numbers(reverse_allocation: bool) -> Vec<u64> {
    let spell = card("Inspection", "Instant", "Reveal the top three cards of your library. You may gain 1 life.", None);
    let a = card("Alpha", "Creature — Bear", "", Some((2, 2)));
    let b = card("Beta", "Creature — Bear", "", Some((2, 2)));
    let c = card("Gamma", "Creature — Bear", "", Some((2, 2)));
    let mut g = Game::new(vec![spell, a, b, c], 2, 20);
    let order = if reverse_allocation { [3, 2, 1] } else { [1, 2, 3] };
    let ids: Vec<_> = order.map(|def| g.put(0, def, Zone::Library)).into();
    // Same observable library order: Alpha, Beta, Gamma. Only hidden allocation differs.
    let alpha = ids[if reverse_allocation { 2 } else { 0 }];
    let beta = ids[1];
    let gamma = ids[if reverse_allocation { 0 } else { 2 }];
    g.players[0].library = vec![gamma, beta, alpha];
    let spell = g.put(0, 0, Zone::Hand);
    let mut s = Simulation::new(g);
    let r = s.request().unwrap().unwrap().clone();
    s.answer(0, r.id, Move::Cast { card: spell, targets: vec![] }).unwrap();
    loop {
        let r = s.request().unwrap().unwrap().clone();
        if r.question != Asked::Priority { break; }
        s.answer(r.player, r.id, Move::Pass).unwrap();
    }
    let prompt = s.prompt(1).unwrap();
    cite!(["701.20a"]; assert!(prompt.entities.contains_key(&alpha) && prompt.entities.contains_key(&beta)));
    [alpha, beta, gamma].map(|id| s.labels(1).unwrap().number(id).unwrap()).into()
}

#[test]
fn newly_revealed_library_labels_do_not_disclose_hidden_allocation_order() {
    // 400.2 / 401.2: a library's hidden creation order is not public information.
    assert_eq!(revealed_numbers(false), revealed_numbers(true));
}

#[test]
fn a_face_down_permanent_has_only_public_text_for_an_opponent() {
    let g = Game::new(vec![card("Secret", "Creature — Bear", "Flying", Some((3, 3))),
        card("Cloak", "Sorcery", "Cloak the top card of your library.", None)], 2, 20);
    let mut g = g;
    g.put(0, 0, Zone::Library);
    let cloak = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card: cloak, targets: vec![] }).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    let down = g.battlefield[0];
    let mut s = Simulation::new(g);
    let mine = s.prompt(0).unwrap();
    let theirs = s.prompt(1).unwrap();
    assert_eq!(mine.entities[&down].card.as_ref().unwrap(), &["Flying"]);
    cite!(["708.5"]; assert!(theirs.entities[&down].card.is_none()));
    cite!(["701.58a"]; assert_eq!(theirs.entities[&down].text, ["Ward {2}"]));
    cite!(["701.58a"]; assert!(theirs.entities[&down].mana_cost.is_none()));
}

#[test]
fn direct_prompt_answers_enforce_target_cardinality_without_listing_moves() {
    let bolt = card("Bolt", "Instant", "This spell deals 3 damage to any target.", None);
    let mut g = Game::new(vec![bolt], 2, 20);
    let bolt = g.put(0, 0, Zone::Hand);
    let mut s = Simulation::new(g);
    // three refusals of one priority request: none submits an action for the player
    let r = s.prompt(0).unwrap().request.unwrap();
    // "any target" is one target: no group, an empty group, or two are refused for it
    for (targets, why) in [
        (vec![], "that has 1 target group(s), 0 given"),
        (vec![vec![]], "wrong number of targets: 0 chosen, it takes 1 (601.2c)"),
        (vec![vec![Target::Player(0), Target::Player(1)]], "wrong number of targets: 2 chosen, it takes 1 (601.2c)"),
    ] {
        crate::support::refused(s.answer(0, r, Move::Cast { card: bolt, targets }), why);
        assert_eq!(s.prompt(0).unwrap().request, Some(r));
    }
    s.answer(0, r, Move::Cast { card: bolt, targets: vec![vec![Target::Player(1)]] }).unwrap();
}

#[test]
fn typed_prompts_describe_modal_cards_and_pending_modal_triggers() {
    let bell = card("Bell", "Artifact", "Whenever a creature enters, choose one —\n• You gain 1 life.\n• Each opponent loses 1 life.", None);
    let bear = card("Bear", "Creature — Bear", "", Some((2, 2)));
    let mut g = Game::new(vec![bell, bear], 2, 20);
    let bell = g.put(0, 0, Zone::Battlefield);
    let bear = g.put(0, 1, Zone::Hand);
    let mut s = Simulation::new(g);
    let prompt = s.prompt(0).unwrap();
    let text = prompt.entities[&bell].text.join("\n");
    assert!(text.contains("You gain 1 life.") && text.contains("Each opponent loses 1 life."), "{text}");
    s.answer(0, prompt.request.unwrap(), Move::Cast { card: bear, targets: vec![] }).unwrap();
    while s.request().unwrap().unwrap().question == Asked::Priority {
        let r = s.request().unwrap().unwrap().clone();
        s.answer(r.player, r.id, Move::Pass).unwrap();
    }
    let decision = s.prompt(0).unwrap().decision.unwrap();
    cite!(["700.2b"]; assert!(matches!(decision.question, Asked::Modes { .. })));
    let text = decision.occurrence.unwrap().text.unwrap();
    assert!(text.contains("You gain 1 life.") && text.contains("Each opponent loses 1 life."), "{text}");
}

fn revealed_hand_numbers(reverse_allocation: bool) -> Vec<u64> {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let kaito = CardDef::from_face(faces.iter().find(|f| f.face_name == "Kaito, Bane of Nightmares").unwrap()).unwrap();
    game::play::check(&kaito).unwrap();
    let bear = card("Bear", "Creature — Bear", "", Some((2, 2)));
    let mut g = Game::new(vec![kaito, bear], 2, 20);
    let first = g.put(0, 0, Zone::Hand);
    let second = g.put(0, 0, Zone::Hand);
    let (alpha, beta) = if reverse_allocation { (second, first) } else { (first, second) };
    g.players[0].hand = vec![alpha, beta];
    let attackers = [g.put(0, 1, Zone::Battlefield), g.put(0, 1, Zone::Battlefield)];
    let pass = |g: &mut Game| for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); };
    pass(&mut g);
    pass(&mut g);
    g.act(0, Move::Attack(attackers.map(|a| (a, Target::Player(1))).into())).unwrap();
    pass(&mut g);
    while let Some(game::state::Decision::Blockers { player }) = g.decision {
        g.act(player, Move::Block(vec![])).unwrap();
    }
    for (ninja, attacker) in [alpha, beta].into_iter().zip(attackers) {
        g.players[0].pool.colors = [0, 1, 1, 0, 0];
        g.players[0].pool.colorless = 1;
        g.act(0, crate::support::paid("CUB", Move::Activate { source: ninja, ability: 5, targets: vec![], paying: vec![vec![attacker]] })).unwrap();
    }
    let mut s = Simulation::new(g);
    let p = s.prompt(1).unwrap();
    cite!(["702.49b"]; assert!(p.view.publicly_revealed.contains(&alpha) && p.view.publicly_revealed.contains(&beta)));
    [alpha, beta].map(|id| s.labels(1).unwrap().number(id).unwrap()).into()
}

#[test]
fn multiple_ninjutsu_reveals_do_not_number_hand_cards_by_hidden_allocation() {
    // 602.2a / 702.49b: both cards are revealed by announced abilities. The observer
    // first sees both together; the same public sequence must give the same labels.
    assert_eq!(revealed_hand_numbers(false), revealed_hand_numbers(true));
}

#[test]
fn an_occurrences_text_survives_its_source_transforming_in_response() {
    // 113.7a / 701.27: the source remains one permanent, but the activated ability
    // on the stack retains the instructions of the face from which it was activated.
    let front = card("Day", "Creature — Human", "{0}: You may gain 1 life.", Some((2, 2)));
    let back = CardDef::new("Night", "", "Creature — Werewolf", "Flying", Some((3, 3))).unwrap();
    let source = CardDef::transforming(front, back).unwrap();
    game::play::check(&source).unwrap();
    let turner = card("Turner", "Instant", "Transform target creature.", None);
    let mut g = Game::new(vec![source, turner], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let turner = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    let ability = g.stack[0];
    g.act(0, Move::Cast { card: turner, targets: vec![vec![Target::Object(source)]] }).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    assert!(g.obj(source).transformed);
    let mut s = Simulation::new(g);
    let prompt = s.prompt(0).unwrap();
    let old = prompt.entities[&ability].occurrence.as_ref().unwrap();
    assert_eq!(old.source_name, "Day");
    cite!(["113.7a"]; assert_eq!(old.text.as_deref(), Some("{0}: You may gain 1 life.")));
    while s.request().unwrap().unwrap().question == Asked::Priority {
        let r = s.request().unwrap().unwrap().clone();
        s.answer(r.player, r.id, Move::Pass).unwrap();
    }
    let decision = s.prompt(0).unwrap().decision.unwrap();
    assert_eq!(decision.question, Asked::May);
    assert_eq!(decision.occurrence.unwrap().text.as_deref(), Some("{0}: You may gain 1 life."));
}

#[test]
fn a_delayed_occurrence_keeps_its_creator_text_after_the_source_transforms() {
    let creator = "{0}: This creature deals 2 damage to target creature. When that creature dies this turn, you may gain 1 life.";
    let front = card("Day", "Creature — Human", creator, Some((2, 2)));
    let back = CardDef::new("Night", "", "Creature — Werewolf", "Flying", Some((3, 3))).unwrap();
    let source = CardDef::transforming(front, back).unwrap();
    game::play::check(&source).unwrap();
    let turner = card("Turner", "Instant", "Transform target creature.", None);
    let bear = card("Bear", "Creature — Bear", "", Some((2, 2)));
    let mut g = Game::new(vec![source, turner, bear], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let bear = g.put(1, 2, Zone::Battlefield);
    let turner = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Activate { source, ability: 0, targets: vec![vec![Target::Object(bear)]], paying: vec![] }).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    assert_eq!(g.obj(bear).zone, Zone::Gone);
    g.act(0, Move::Cast { card: turner, targets: vec![vec![Target::Object(source)]] }).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    let mut s = Simulation::new(g);
    let p = s.prompt(0).unwrap();
    let occurrence = p.entities[p.view.stack.last().unwrap()].occurrence.as_ref().unwrap();
    assert_eq!(occurrence.kind, game::prompt::OccurrenceKind::Delayed);
    let text = occurrence.created_by.as_deref().unwrap();
    assert!(text.contains("2 damage to target creature") && text.contains("gain 1 life"), "{text}");
}
