//! The player-facing prompt (`game::prompt`): what a player is told, with labels and
//! readable source/effect context, and how the answers are checked.

use crate::support::cite;
use game::legal::Asked;
use game::prompt::{Label, OccurrenceKind, Prompt, Reason};
use game::simulation::{ResponseError, Simulation};
use game::state::Answer;
use game::{CardDef, Game, Move, ObjId, Target, Zone};

fn card(name: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    let def = CardDef::new(name, "{0}", types, text, pt).unwrap();
    game::play::check(&def).unwrap_or_else(|e| panic!("{name}: {e:?}"));
    def
}

fn request(s: &mut Simulation) -> (game::simulation::RequestId, usize) {
    let r = s.request().unwrap().unwrap();
    (r.id, r.player)
}

/// Both players pass until the question is no longer priority.
fn pass_until_asked(s: &mut Simulation) {
    while s.request().unwrap().unwrap().question == Asked::Priority {
        let (id, player) = request(s);
        s.respond(id, player, Move::Pass).unwrap();
    }
}

fn text(prompt: &Prompt, o: ObjId) -> &[String] { &prompt.entities[&o].text }

#[test]
fn a_prompt_is_the_seats_view_with_text_and_asks_only_the_seat_that_answers() {
    let bolt = card("Bolt", "Instant", "This spell deals 3 damage to any target.", None);
    let bear = card("Bear", "Creature — Bear", "", Some((2, 2)));
    let mut game = Game::new(vec![bolt, bear], 2, 20);
    let mine = game.put(0, 0, Zone::Hand);
    let theirs = game.put(1, 0, Zone::Hand);
    let (a, b) = (game.put(1, 1, Zone::Battlefield), game.put(1, 1, Zone::Battlefield));
    let mut s = Simulation::new(game);
    let me = s.prompt(0).unwrap();
    assert_eq!(me.api, game::prompt::API);
    assert_eq!((me.seat, me.waiting_for), (0, Some(0)));
    let decision = me.decision.as_ref().expect("the seat with priority is asked");
    assert_eq!((decision.question.clone(), decision.reason, decision.chooser), (Asked::Priority, Reason::Priority, 0));
    // 402.3: the opponent's hand is counted, not shown, and its card has no entity
    cite!(["402.3"]; assert!(me.entities.contains_key(&mine) && !me.entities.contains_key(&theirs)));
    assert_eq!(text(&me, mine), ["Bolt deals 3 damage to any target."]);
    // two cards with one name are two entities, told apart by their labels
    let names = s.labels(0).unwrap();
    assert_ne!(names.number(a), names.number(b));
    assert_eq!(me.view.objects.iter().filter(|o| o.name == "Bear").count(), 2);
    // the seat's names number what it was shown, the battlefield before its hand
    assert_eq!([a, b, mine].map(|o| names.number(o)), [Some(1), Some(2), Some(3)]);
    assert_eq!(names.number(theirs), None);
    let them = s.prompt(1).unwrap();
    assert!(them.decision.is_none() && them.waiting_for == Some(0), "another seat is told whom the game waits for");
    cite!(["402.3"]; assert!(them.entities.contains_key(&theirs) && !them.entities.contains_key(&mine)));
    // prompting again while nothing changed gives the same view
    assert!(std::sync::Arc::ptr_eq(&me.view, &s.prompt(0).unwrap().view));
}

#[test]
fn each_activated_ability_of_one_source_has_its_own_label_and_a_label_answer_activates_it() {
    let shrine = card("Shrine", "Artifact", "{T}: You gain 1 life.\n{1}: You gain 2 life.\n{0}: You gain 3 life.", None);
    let mut game = Game::new(vec![shrine], 2, 20);
    let one = game.put(0, 0, Zone::Battlefield);
    let two = game.put(0, 0, Zone::Battlefield);
    let mut s = Simulation::new(game);
    let prompt = s.prompt(0).unwrap();
    for o in [one, two] {
        let abilities = &prompt.entities[&o].abilities;
        assert_eq!(abilities.iter().map(|a| a.text.as_str()).collect::<Vec<_>>(),
            ["{T}: You gain 1 life.", "{1}: You gain 2 life.", "{0}: You gain 3 life."]);
        assert_eq!(abilities.iter().map(|a| a.index).collect::<Vec<_>>(), [0, 1, 2]);
    }
    let Some(Label::Ability(n, index)) = Label::parse(&Label::Ability(s.labels(0).unwrap().number(two).unwrap(), 2).to_string()) else { panic!() };
    let source = s.labels(0).unwrap().object(n).unwrap();
    let id = prompt.request.unwrap();
    s.answer(0, id, Move::Activate { source, ability: index, targets: vec![], paying: vec![] }).unwrap();
    let stacked = s.prompt(0).unwrap();
    let top = *stacked.view.stack.last().unwrap();
    let occurrence = stacked.entities[&top].occurrence.as_ref().unwrap();
    assert_eq!((occurrence.kind, occurrence.source, occurrence.text.as_deref()), (OccurrenceKind::Activated, Some(two), Some("{0}: You gain 3 life.")));
    pass_until_resolved(&mut s);
    assert_eq!(s.game().unwrap().players[0].life, 23);
}

fn pass_until_resolved(s: &mut Simulation) {
    while !s.game().unwrap().stack.is_empty() {
        let (id, player) = request(s);
        s.respond(id, player, Move::Pass).unwrap();
    }
}

#[test]
fn an_answer_naming_what_the_seat_may_not_see_is_refused_like_one_naming_nothing() {
    let bolt = card("Bolt", "Instant", "This spell deals 3 damage to any target.", None);
    let mut game = Game::new(vec![bolt], 2, 20);
    let mine = game.put(0, 0, Zone::Hand);
    let theirs = game.put(1, 0, Zone::Hand);
    let library = game.put(1, 0, Zone::Library);
    let mut s = Simulation::new(game);
    // six refusals of one priority request: none submits an action for the player
    let (id, _) = request(&mut s);
    let before = format!("{:?}", s.game().unwrap());
    for hidden in [theirs, library, ObjId(9_999)] {
        let cast = Move::Cast { card: mine, targets: vec![vec![Target::Object(hidden)]] };
        assert_eq!(s.answer(0, id, cast), Err(ResponseError::Unreferable(Target::Object(hidden))));
        assert_eq!(s.answer(0, id, Move::Cast { card: hidden, targets: vec![vec![Target::Player(1)]] }), Err(ResponseError::Unreferable(Target::Object(hidden))));
    }
    assert_eq!(format!("{:?}", s.game().unwrap()), before, "nothing was asked of the game");
    assert_eq!(request(&mut s).0, id);
    // the seat's own answers still go through the game's checks: another seat is refused,
    // a stale request is refused, and a visible card is cast
    assert!(matches!(s.answer(1, id, Move::Pass), Err(ResponseError::WrongPlayer { .. })));
    s.answer(0, id, Move::Cast { card: mine, targets: vec![vec![Target::Player(1)]] }).unwrap();
    assert!(matches!(s.answer(0, id, Move::Pass), Err(ResponseError::Stale { .. })));
}

#[test]
fn simultaneous_triggers_are_ordered_by_source_and_text() {
    let bell = card("Bell", "Enchantment", "Whenever you cast a spell, you gain 1 life.", None);
    let horn = card("Horn", "Enchantment", "Whenever you cast a spell, each opponent loses 1 life.", None);
    let spell = card("Spell", "Instant", "You gain 1 life.", None);
    let mut game = Game::new(vec![bell, horn, spell], 2, 20);
    let bell = game.put(0, 0, Zone::Battlefield);
    let horn = game.put(0, 1, Zone::Battlefield);
    let spell = game.put(0, 2, Zone::Hand);
    let mut s = Simulation::new(game);
    let (id, _) = request(&mut s);
    s.answer(0, id, Move::Cast { card: spell, targets: vec![] }).unwrap();
    let prompt = s.prompt(0).unwrap();
    let decision = prompt.decision.unwrap();
    cite!(["603.3b"]; assert!(matches!(decision.question, Asked::Order(_))));
    assert_eq!(decision.reason, Reason::TriggerOrder);
    let told: Vec<_> = decision.ordered.iter().map(|o| (o.source, o.source_name.as_str(), o.text.as_deref(), o.kind)).collect();
    assert_eq!(told, [
        (Some(bell), "Bell", Some("Whenever you cast a spell, you gain 1 life."), OccurrenceKind::Triggered),
        (Some(horn), "Horn", Some("Whenever you cast a spell, each opponent loses 1 life."), OccurrenceKind::Triggered),
    ]);
    // the opponent learns only that the game waits for this player
    assert!(s.prompt(1).unwrap().decision.is_none());
}

#[test]
fn an_effects_question_says_which_resolving_spell_asks_it() {
    let spell = card("Offer", "Instant", "You may gain 1 life.", None);
    let mut game = Game::new(vec![spell], 2, 20);
    let offer = game.put(0, 0, Zone::Hand);
    let mut s = Simulation::new(game);
    let (id, _) = request(&mut s);
    s.answer(0, id, Move::Cast { card: offer, targets: vec![] }).unwrap();
    let on_stack = s.game().unwrap().stack[0];
    pass_until_asked(&mut s);
    let decision = s.prompt(0).unwrap().decision.unwrap();
    cite!(["608.2d"]; assert_eq!((decision.question.clone(), decision.reason), (Asked::May, Reason::Resolution)));
    let spell = decision.occurrence.unwrap();
    assert_eq!((spell.kind, spell.object, spell.source_name.as_str()), (OccurrenceKind::Spell, Some(on_stack), "Offer"));
    assert_eq!(s.prompt(0).unwrap().entities[&on_stack].text, ["You may gain 1 life."]);
    s.answer(0, decision.id, Move::Choose(Answer::Yes)).unwrap();
    assert_eq!(s.game().unwrap().players[0].life, 21);
}

#[test]
fn a_trigger_whose_source_has_left_keeps_its_name_and_text() {
    let martyr = card("Martyr", "Creature — Human", "When this creature dies, it deals 1 damage to any target.", Some((1, 1)));
    let kill = card("Kill", "Instant", "Destroy target creature.", None);
    let mut game = Game::new(vec![martyr, kill], 2, 20);
    let martyr = game.put(1, 0, Zone::Battlefield);
    let kill = game.put(0, 1, Zone::Hand);
    let mut s = Simulation::new(game);
    let (id, _) = request(&mut s);
    s.answer(0, id, Move::Cast { card: kill, targets: vec![vec![Target::Object(martyr)]] }).unwrap();
    pass_until_asked(&mut s);
    let prompt = s.prompt(1).unwrap();
    let decision = prompt.decision.unwrap();
    assert_eq!(decision.reason, Reason::Trigger);
    cite!(["603.3d"]; assert!(matches!(decision.question, Asked::Targets { .. })));
    let trigger = decision.occurrence.unwrap();
    assert_eq!((trigger.source, trigger.source_left, trigger.source_name.as_str()), (Some(martyr), true, "Martyr"));
    assert_eq!(trigger.text.as_deref(), Some("When Martyr dies, it deals 1 damage to any target."));
    assert!(!prompt.view.battlefield.contains(&martyr), "it is described as it was, not looked for where it is");
    s.answer(1, decision.id, Move::ChooseTargets(vec![vec![Target::Player(0)]])).unwrap();
    let stacked = s.prompt(0).unwrap();
    let top = *stacked.view.stack.last().unwrap();
    let on_stack = stacked.entities[&top].occurrence.as_ref().unwrap();
    cite!(["603.3a"]; assert_eq!((on_stack.kind, on_stack.source, on_stack.source_left, on_stack.controller), (OccurrenceKind::Triggered, Some(martyr), true, 1)));
    assert_eq!(on_stack.text.as_deref(), Some("When Martyr dies, it deals 1 damage to any target."));
}

#[test]
fn a_granted_ability_is_labelled_on_the_permanent_that_has_it_and_says_what_gives_it() {
    let banner = card("Banner", "Enchantment", "Creatures you control have \"{0}: You gain 1 life.\"", None);
    let bear = card("Bear", "Creature — Bear", "", Some((2, 2)));
    let mut game = Game::new(vec![banner, bear], 2, 20);
    let banner = game.put(0, 0, Zone::Battlefield);
    let bear = game.put(0, 1, Zone::Battlefield);
    let mut s = Simulation::new(game);
    let prompt = s.prompt(0).unwrap();
    let granted = &prompt.entities[&bear].abilities;
    assert_eq!(granted.len(), 1);
    assert_eq!((granted[0].text.as_str(), granted[0].granted), ("{0}: You gain 1 life.", Some(Some(banner))));
    assert_eq!(prompt.entities[&bear].text, ["{0}: You gain 1 life."]);
    s.answer(0, prompt.request.unwrap(), Move::Activate { source: bear, ability: granted[0].index, targets: vec![], paying: vec![] }).unwrap();
    let top = *s.game().unwrap().stack.last().unwrap();
    let occurrence = s.prompt(1).unwrap().entities[&top].occurrence.clone().unwrap();
    assert!(occurrence.granted && occurrence.text.as_deref() == Some("{0}: You gain 1 life."));
}

#[test]
fn a_private_search_offers_its_chooser_library_cards_no_other_seat_can_name() {
    let tutor = card("Tutor", "Sorcery", "Search your library for a card, put it into your hand, then shuffle.", None);
    let bear = card("Bear", "Creature — Bear", "", Some((2, 2)));
    let mut game = Game::new(vec![tutor, bear], 2, 20);
    let tutor = game.put(0, 0, Zone::Hand);
    let wanted = game.put(0, 1, Zone::Library);
    game.put(0, 1, Zone::Library);
    let mut s = Simulation::new(game);
    let (id, _) = request(&mut s);
    s.answer(0, id, Move::Cast { card: tutor, targets: vec![] }).unwrap();
    pass_until_asked(&mut s);
    let prompt = s.prompt(0).unwrap();
    let decision = prompt.decision.unwrap();
    cite!(["701.23a"]; assert!(matches!(&decision.question, Asked::Search { cards, .. } if cards.contains(&wanted))));
    assert_eq!(decision.reason, Reason::Resolution);
    assert!(prompt.entities.contains_key(&wanted), "the chooser reads the cards searched");
    assert!(!s.prompt(1).unwrap().entities.contains_key(&wanted), "the other seat does not");
    s.answer(0, decision.id, Move::Choose(Answer::Objects(vec![wanted]))).unwrap();
    assert_eq!(s.game().unwrap().players[0].hand.len(), 1);
}

#[test]
fn a_seats_labels_say_nothing_of_how_objects_are_stored_or_what_it_was_not_shown() {
    let bolt = card("Bolt", "Instant", "This spell deals 3 damage to any target.", None);
    let mut game = Game::new(vec![bolt], 2, 20);
    let first = game.put(0, 0, Zone::Hand);
    let hidden: Vec<ObjId> = (0..5).map(|_| game.put(1, 0, Zone::Library)).collect();
    let second = game.put(0, 0, Zone::Hand);
    let mut s = Simulation::new(game);
    s.prompt(0).unwrap();
    // five cards the seat cannot see came between the two it can: its names do not skip them
    assert_eq!((s.labels(0).unwrap().number(first), s.labels(0).unwrap().number(second)), (Some(1), Some(2)));
    assert!(hidden.iter().all(|o| s.labels(0).unwrap().number(*o).is_none()));
    let (id, _) = request(&mut s);
    s.answer(0, id, Move::Cast { card: first, targets: vec![vec![Target::Player(1)]] }).unwrap();
    let spell = s.game().unwrap().stack[0];
    s.prompt(0).unwrap();
    s.prompt(1).unwrap();
    // the spell is a new object with the next name each seat has, however it is stored
    assert_eq!(s.labels(0).unwrap().number(spell), Some(3));
    assert_eq!(s.labels(1).unwrap().number(spell), Some(1));
    // a copy goes on naming by itself
    let mut branch = s.clone();
    assert_eq!(branch.labels(0), s.labels(0));
    branch.labels_mut(0).name(ObjId(77));
    assert_ne!(branch.labels(0), s.labels(0));
}

#[test]
fn r700_2_a_modal_spell_reads_with_its_modes_and_on_the_stack_says_those_chosen() {
    let charm = card("Charm", "Instant", "Choose one —\n• You gain 2 life.\n• Each opponent loses 2 life.", None);
    let mut game = Game::new(vec![charm], 2, 20);
    let charm = game.put(0, 0, Zone::Hand);
    let mut s = Simulation::new(game);
    let prompt = s.prompt(0).unwrap();
    assert_eq!(text(&prompt, charm), ["Choose one —\n• You gain 2 life.\n• Each opponent loses 2 life."]);
    s.answer(0, prompt.request.unwrap(), Move::Modes(vec![1], Box::new(Move::Cast { card: charm, targets: vec![] }))).unwrap();
    let them = s.prompt(1).unwrap();
    let spell = them.view.stack[0];
    let occurrence = them.entities[&spell].occurrence.clone().unwrap();
    // 700.2a: the modes are announced; every player knows them
    cite!(["700.2a"]; assert_eq!(occurrence.modes, [(1, "• Each opponent loses 2 life.".to_string())]));
    assert_eq!(text(&them, spell), ["Choose one —\n• You gain 2 life.\n• Each opponent loses 2 life."]);
}

#[test]
fn r700_2b_a_modal_trigger_reads_with_its_modes() {
    let altar = card("Altar", "Enchantment", "Whenever you cast a spell, choose one —\n• You gain 1 life.\n• Each opponent loses 1 life.", None);
    let spell = card("Spell", "Instant", "You gain 1 life.", None);
    let mut game = Game::new(vec![altar, spell], 2, 20);
    let altar = game.put(0, 0, Zone::Battlefield);
    let spell = game.put(0, 1, Zone::Hand);
    let mut s = Simulation::new(game);
    let full = "Whenever you cast a spell, choose one —\n• You gain 1 life.\n• Each opponent loses 1 life.";
    assert_eq!(text(&s.prompt(0).unwrap(), altar), [full]);
    let (id, _) = request(&mut s);
    s.answer(0, id, Move::Cast { card: spell, targets: vec![] }).unwrap();
    let decision = s.prompt(0).unwrap().decision.unwrap();
    assert_eq!(decision.reason, Reason::Trigger);
    assert_eq!(decision.occurrence.unwrap().text.as_deref(), Some(full));
}

/// A library from the bottom: the same three cards in the same places, made in either order.
fn revealing(made_backwards: bool) -> Simulation {
    let show = card("Show", "Sorcery", "Reveal the top three cards of your library. You may draw a card.", None);
    let named = |n: &str| card(n, "Creature — Bear", "", Some((2, 2)));
    let mut game = Game::new(vec![show, named("Alpha"), named("Beta"), named("Gamma")], 2, 20);
    let order: Vec<usize> = if made_backwards { vec![3, 2, 1] } else { vec![1, 2, 3] };
    for card in order { game.put(0, card, Zone::Library); }
    if made_backwards { game.players[0].library.reverse(); }
    let show = game.put(0, 0, Zone::Hand);
    let mut s = Simulation::new(game);
    s.prompt(1).unwrap();
    let (id, _) = request(&mut s);
    s.answer(0, id, Move::Cast { card: show, targets: vec![] }).unwrap();
    pass_until_asked(&mut s);
    s
}

#[test]
fn r401_2_cards_shown_together_are_named_by_where_they_are_not_how_they_were_made() {
    let names = |made_backwards| {
        let mut s = revealing(made_backwards);
        let prompt = s.prompt(1).unwrap();
        let labels = s.labels(1).unwrap();
        let mut shown: Vec<(u64, String)> = prompt.view.objects.iter().filter(|o| ["Alpha", "Beta", "Gamma"].contains(&o.name.as_str()))
            .map(|o| (labels.number(o.id).unwrap(), o.name.clone())).collect();
        shown.sort();
        shown
    };
    let forwards = names(false);
    cite!(["701.20a"]; assert_eq!(forwards.len(), 3, "the reveal shows the other seat all three: {forwards:?}"));
    assert_eq!(forwards, names(true));
}
