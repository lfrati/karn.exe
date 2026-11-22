//! An activated or triggered ability on the stack "has the text of the ability that created
//! it and no other characteristics" (405.4, 602.2a, 603.3). Every viewer, its controller
//! included, is shown that text and, of its source, only the name it had when the ability was
//! put on the stack: a name the harness shows by its convention, no characteristic of the
//! ability. Nothing else of its source's card or characteristics is shown: no mana cost,
//! types, subtypes, supertypes, colors, power, toughness, loyalty, inset, card text or
//! abilities. So a face-down permanent's ward trigger tells no one what the card is, which a
//! player may not look at while another player controls it (708.5).
use crate::support::{cite, paid};
use crate::support;
use game::announcement::CastCost;
use game::face::Way;
use game::prompt::{OccurrenceKind, Prompt};
use game::simulation::Simulation;
use game::snapshot::{Object, Snapshot};
use support::*;

/// What this seat is prompted with, the game as it is now.
fn prompt(g: &Game, seat: usize) -> Prompt { Simulation::new(g.clone()).prompt(seat).unwrap() }

fn object(objects: &[Object], o: ObjId) -> &Object { objects.iter().find(|x| x.id == o).expect("the viewer sees it") }

/// An ability on the stack as a viewer is shown it: none of the characteristics checked here
/// (109.3, 405.4), and its source's name as the ability was put on the stack with it, which
/// the harness shows by its convention and which is no characteristic of the ability.
#[track_caller]
fn no_characteristics(o: &Object, name: &str, viewer: &str) {
    cite!(["405.4"]; assert!(o.types.is_empty() && o.subtypes.is_empty() && o.supertypes.is_empty(), "{viewer}: {o:?}"));
    cite!(["405.4"]; assert!(o.colors.is_empty(), "{viewer}: {o:?}"));
    cite!(["405.4"]; assert_eq!((o.pt, o.loyalty), (None, None), "{viewer}: {o:?}"));
    assert_eq!(o.name, name, "{viewer}");
    assert!(o.on_stack.as_ref().is_some_and(|s| s.ability), "{viewer}: {o:?}");
    assert!(o.permanent.is_none() && o.face_down.is_none() && o.hidden.is_none() && o.emblem.is_none(), "{viewer}: {o:?}");
}

/// The ability on the stack as every viewer is shown it: each seat's prompt, the entity and
/// the object of its view, and the harness's snapshot. Each has the one line of text of the
/// ability that created it, the same for every seat, and nothing else (405.4). Returns that
/// text and the prompts.
#[track_caller]
fn only_its_text(g: &Game, ability: ObjId, name: &str, rule: &str) -> (Vec<String>, Vec<Prompt>) {
    assert!(g.obj(ability).ability.is_some(), "an ability on the stack");
    let prompts: Vec<Prompt> = (0..g.players.len()).map(|seat| prompt(g, seat)).collect();
    for (seat, p) in prompts.iter().enumerate() {
        let viewer = format!("p{seat}");
        let entity = &p.entities[&ability];
        let occurrence = entity.occurrence.as_ref().expect("an object on the stack says how it occurred");
        cite!(["405.4", rule]; assert_eq!(entity.text, occurrence.text.iter().cloned().collect::<Vec<_>>(), "{viewer}: its own text"));
        cite!(["405.4", rule]; assert_eq!(entity.text, prompts[0].entities[&ability].text, "{viewer}: as every seat reads it"));
        cite!(["405.4", rule]; assert_eq!(entity.mana_cost, None, "{viewer}"));
        cite!(["405.4", rule]; assert!(entity.abilities.is_empty(), "{viewer}: {:?}", entity.abilities));
        cite!(["405.4", rule]; assert_eq!((&entity.card, &entity.inset), (&None, &None), "{viewer}"));
        no_characteristics(object(&p.view.objects, ability), name, &viewer);
    }
    no_characteristics(object(&Snapshot::of(g, None).objects, ability), name, "the harness");
    (prompts[0].entities[&ability].text.clone(), prompts)
}

/// Pass priority until something other than priority is asked or the stack is empty.
fn settle(t: &mut T) {
    while t.g.decision.is_none() && !t.g.stack.is_empty() { t.g.act(t.g.priority, Move::Pass).unwrap(); }
}

/// Player 1 casts Shock (definition `shock`) at this permanent, from priority with an empty
/// stack; returns the object its ward trigger is.
fn shock(t: &mut T, shock: usize, target: ObjId) -> ObjId {
    assert!(t.g.stack.is_empty());
    while t.g.priority != 1 { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    let card = t.g.put(1, shock, Zone::Hand);
    t.mana(1, "{R}");
    t.g.act(1, paid("R", Move::Cast { card, targets: vec![vec![Target::Object(target)]] })).unwrap();
    cite!(["702.21a", "603.3"]; assert_eq!(t.g.stack.len(), 2, "Shock and the ward trigger above it"));
    *t.g.stack.last().unwrap()
}

/// Player 1 declines to pay for ward, and Shock is countered.
fn decline_ward(t: &mut T) {
    settle(t);
    assert!(matches!(t.g.asked(), Some((1, Asked::Pay { .. }))), "{:?}", t.g.asked());
    t.g.act(1, Move::Choose(Answer::No)).unwrap();
    t.resolve();
}

/// A ward trigger's text, as 702.21a says ward: "Whenever this permanent becomes the target of
/// a spell or ability an opponent controls, counter that spell or ability unless that player
/// pays [cost]."
#[track_caller]
fn ward_text(text: &[String]) {
    let [line] = text else { panic!("one line: {text:?}") };
    cite!(["702.21a"]; assert!(line.contains("becomes the target of a spell or ability an opponent controls")
        && line.contains("counter that spell or ability unless") && line.contains("{2}"), "{line}"));
}

/// Player 0 has cloaked Elusive Otter // Grove's Bounty (definition 1) and Ajani Resolute
/// (definition 2) with Hide in Plain Sight; Shock is definition 3.
fn cloaked() -> T {
    let mut defs: Vec<CardDef> = ["Hide in Plain Sight", "Elusive Otter // Grove's Bounty", "Ajani Resolute", "Shock"].into_iter().map(oracle).collect();
    defs.push(CardDef::new("Wastes", "", "Basic Land", "", None).unwrap());
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, 4, Zone::Library); } }
    // the top five cards of player 0's library, from the top: Elusive Otter, Ajani Resolute
    // and three Wastes
    for def in [4, 4, 4, 2, 1] { g.put(0, def, Zone::Library); }
    let spell = g.put(0, 0, Zone::Hand);
    let mut t = T::new(g);
    t.mana(0, "{3}{G}");
    t.g.act(0, paid("CCCG", Move::Cast { card: spell, targets: vec![] })).unwrap();
    settle(&mut t);
    for pick in ["Elusive Otter", "Ajani Resolute"] {
        let Some(Decision::Choose { player: 0, choice: Choice::Objects { among, .. } }) = t.g.decision.clone() else { panic!("{:?}", t.g.decision) };
        let card = *among.iter().find(|o| t.g.obj(**o).chars.name == pick).unwrap();
        t.g.act(0, Move::Choose(Answer::Objects(vec![card]))).unwrap();
    }
    t.resolve();
    t
}

/// The face-down permanent player 0's card of this definition is.
fn face_down(t: &T, def: usize) -> ObjId {
    *t.g.battlefield.iter().find(|o| t.g.face_down(**o) && t.g.physical_card(**o).is_some_and(|(card, _)| card == def)).expect("face down")
}

#[test]
fn r708_5_a_cloaked_permanents_ward_trigger_shows_no_viewer_its_card() {
    let mut t = cloaked();
    let cloaked = face_down;
    for (def, name) in [(1, "Elusive Otter"), (2, "Ajani Resolute")] {
        let permanent = cloaked(&t, def);
        let ward = shock(&mut t, 3, permanent);
        let (text, prompts) = only_its_text(&t.g, ward, "", "603.3");
        ward_text(&text);
        for (seat, p) in prompts.iter().enumerate() {
            let occurrence = p.entities[&ward].occurrence.as_ref().unwrap();
            // its stack facts stay: what it is, its controller and its source, which both
            // players see as a face-down permanent (708.2)
            cite!(["113.7", "603.3"]; assert_eq!((occurrence.kind, occurrence.controller, occurrence.source), (OccurrenceKind::Triggered, 0, Some(permanent)), "p{seat}"));
            cite!(["708.2a"]; assert_eq!(occurrence.source_name, "", "p{seat}: its source has no name"));
            // the face-down permanent itself: its controller may look at it, the opponent may not
            let shown = object(&p.view.objects, permanent).face_down.as_ref().and_then(|d| d.card.as_ref()).map(|c| c.name.as_str());
            cite!(["708.5"]; assert_eq!(shown, (seat == 0).then_some(name), "p{seat}"));
        }
        decline_ward(&mut t);
        cite!(["702.21a"]; assert!(t.g.battlefield.contains(&permanent) && t.g.stack.is_empty()));
    }
}

/// Player 0 has cast Defenestrated Phantom (definition 0) face down with disguise, and it
/// resolved; Shock is definition 1. Returns the disguised permanent.
fn disguised() -> (T, ObjId) {
    let mut defs: Vec<CardDef> = ["Defenestrated Phantom", "Shock"].into_iter().map(oracle).collect();
    defs.push(CardDef::new("Wastes", "", "Basic Land", "", None).unwrap());
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, 2, Zone::Library); } }
    let card = g.put(0, 0, Zone::Hand);
    let mut t = T::new(g);
    // cast face down for {3} as a 2/2 creature with ward {2} and no name (702.168a)
    t.mana(0, "{3}");
    t.g.act(0, paid("CCC", Move::CastFor(CastCost::Alternative(1), Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    t.resolve();
    let permanent = face_down(&t, 0);
    (t, permanent)
}

#[test]
fn r702_168a_a_disguised_permanents_ward_trigger_shows_no_viewer_its_card() {
    let (mut t, permanent) = disguised();
    let ward = shock(&mut t, 1, permanent);
    // nothing of Defenestrated Phantom: not {4}{W}{W}, white, a Spirit, 4/3, "Flying" or
    // "Disguise {4}{W}"
    let (text, prompts) = only_its_text(&t.g, ward, "", "603.3");
    ward_text(&text);
    for (seat, p) in prompts.iter().enumerate() {
        let shown = object(&p.view.objects, permanent).face_down.as_ref().and_then(|d| d.card.as_ref()).map(|c| c.name.as_str());
        cite!(["708.5"]; assert_eq!(shown, (seat == 0).then_some("Defenestrated Phantom"), "p{seat}"));
    }
    decline_ward(&mut t);
}

/// Its controller turns the face-down permanent face up while its ward trigger waits on the
/// stack: for its card's mana cost (701.58b) or its disguise cost (702.168d), "This is a
/// special action". It "regains its normal characteristics", but the trigger "exists on the
/// stack independently of its source" (113.7a) and "has the text of the ability that created
/// it" (405.4, 603.3): ward's, not the line its card now prints in that place.
#[test]
fn r113_7a_a_ward_trigger_keeps_wards_text_when_its_source_is_turned_face_up() {
    let mut cloak = cloaked();
    let otter = face_down(&cloak, 1);
    let mut disguise = disguised();
    for (t, permanent, shock_def, way, cost, units, rule) in [
        (&mut cloak, otter, 3, Way::Cloaked, "{U}", "U", "701.58b"),
        (&mut disguise.0, disguise.1, 1, Way::Disguise, "{4}{W}", "CCCCW", "702.168d"),
    ] {
        let ward = shock(t, shock_def, permanent);
        let (before, _) = only_its_text(&t.g, ward, "", "603.3");
        ward_text(&before);
        // player 1 cast Shock and passes; player 0 has priority with the trigger on the stack
        t.g.act(1, Move::Pass).unwrap();
        assert_eq!((t.g.priority, t.g.stack.last()), (0, Some(&ward)));
        t.mana(0, cost);
        t.g.act(0, paid(units, Move::TurnFaceUp(permanent, way))).unwrap();
        cite!([rule]; assert!(!t.g.face_down(permanent) && !t.g.obj(permanent).chars.name.is_empty(), "turned face up"));
        cite!([rule, "116.2b"]; assert_eq!(t.g.stack.last(), Some(&ward), "a special action, with the trigger still waiting"));
        // the same text, for every seat, the harness and the question to pay
        let (after, _) = only_its_text(&t.g, ward, "", "603.3");
        cite!(["113.7a", "405.4", "603.3"]; assert_eq!(after, before, "{rule}"));
        settle(t);
        assert!(matches!(t.g.asked(), Some((1, Asked::Pay { .. }))), "{:?}", t.g.asked());
        let asked = prompt(&t.g, 1).decision.and_then(|d| d.occurrence).and_then(|o| o.text);
        cite!(["113.7a", "405.4"]; assert_eq!(asked.as_slice(), before.as_slice(), "{rule}: the payment is asked for ward"));
        t.g.act(1, Move::Choose(Answer::No)).unwrap();
        t.resolve();
        cite!(["702.21a"]; assert!(t.g.stack.is_empty() && t.g.battlefield.contains(&permanent), "{rule}: Shock was countered"));
    }
}

#[test]
fn r603_3_a_face_up_creatures_prowess_trigger_has_no_power_inset_types_or_colors() {
    let defs: Vec<CardDef> = ["Elusive Otter // Grove's Bounty", "Shock"].into_iter().map(oracle).collect();
    let mut g = Game::new(defs, 2, 20);
    let otter = g.put(0, 0, Zone::Battlefield);
    let card = g.put(0, 1, Zone::Hand);
    let mut t = T::new(g);
    t.mana(0, "{R}");
    t.g.act(0, paid("R", Move::Cast { card, targets: vec![vec![Target::Player(1)]] })).unwrap();
    cite!(["702.108a"]; assert_eq!(t.g.stack.len(), 2, "Shock and the prowess trigger above it"));
    let prowess = *t.g.stack.last().unwrap();
    let (text, prompts) = only_its_text(&t.g, prowess, "Elusive Otter", "603.3");
    let [line] = text.as_slice() else { panic!("one line: {text:?}") };
    cite!(["702.108a"]; assert!(line.contains("Whenever you cast a noncreature spell") && line.contains("+1/+1 until end of turn"), "{line}"));
    for p in &prompts {
        let occurrence = p.entities[&prowess].occurrence.as_ref().unwrap();
        cite!(["113.7"]; assert_eq!((occurrence.kind, occurrence.source), (OccurrenceKind::Triggered, Some(otter))));
        // its source keeps its own characteristics: a blue 1/1 Otter creature with an Adventure
        let source = object(&p.view.objects, otter);
        cite!(["208.1", "105.2"]; assert_eq!((source.pt, source.colors.as_slice(), source.types.as_slice()), (Some((1, 1)), &[ir::Color::Blue][..], &[ir::CardType::Creature][..])));
        cite!(["715.2a"]; assert_eq!(p.entities[&otter].inset.as_ref().map(|i| i.name.as_str()), Some("Grove's Bounty")));
        cite!(["202.1"]; assert_eq!(p.entities[&otter].mana_cost.as_deref(), Some("{U}")));
    }
}

#[test]
fn r602_2a_a_loyalty_ability_has_no_loyalty_and_its_planeswalker_has_its_counters() {
    let mut g = Game::new(vec![oracle("Ajani Resolute")], 2, 20);
    let card = g.put(0, 0, Zone::Hand);
    let mut t = T::new(g);
    t.mana(0, "{1}{W}");
    t.g.act(0, paid("CW", Move::Cast { card, targets: vec![] })).unwrap();
    t.resolve();
    let ajani = *t.g.battlefield.last().expect("Ajani resolved");
    let loyalty = |t: &T, seat: usize| {
        let o = object(&prompt(&t.g, seat).view.objects, ajani).clone();
        (o.loyalty, o.counters.get("loyalty").copied())
    };
    // it entered with its printed loyalty in loyalty counters (306.5b), and its loyalty is them
    cite!(["306.5b", "306.5c"]; assert_eq!(loyalty(&t, 1), (None, Some(2))));
    let zero = prompt(&t.g, 0).entities[&ajani].abilities.iter().find(|a| a.text.contains("You gain 1 life")).expect("its 0 ability").index;
    t.g.act(0, Move::Activate { source: ajani, ability: zero, targets: vec![], paying: vec![] }).unwrap();
    let activated = *t.g.stack.last().unwrap();
    // not Ajani's {1}{W}, white, Legendary Planeswalker — Ajani, or loyalty 2
    let (text, prompts) = only_its_text(&t.g, activated, "Ajani Resolute", "602.2a");
    let [line] = text.as_slice() else { panic!("one line: {text:?}") };
    cite!(["602.2a", "606.4"]; assert!(line.contains("You gain 1 life") && !line.contains("loyalty counter"), "{line}"));
    for p in &prompts {
        let occurrence = p.entities[&activated].occurrence.as_ref().unwrap();
        cite!(["602.2a"]; assert_eq!((occurrence.kind, occurrence.source), (OccurrenceKind::Activated, Some(ajani))));
    }
    // its controller gains life, and Ajani's first ability triggers
    t.g.act(0, Move::Pass).unwrap();
    t.g.act(1, Move::Pass).unwrap();
    assert_eq!(t.g.players[0].life, 21);
    let triggered = *t.g.stack.last().expect("Whenever you gain life");
    let (text, _) = only_its_text(&t.g, triggered, "Ajani Resolute", "603.3");
    let [line] = text.as_slice() else { panic!("one line: {text:?}") };
    cite!(["603.3"]; assert!(line.contains("Whenever you gain life") && line.contains("loyalty counter"), "{line}"));
    t.resolve();
    // on the battlefield its loyalty is the loyalty counters on it, now three
    cite!(["306.5c"]; assert_eq!((loyalty(&t, 0), loyalty(&t, 1)), ((None, Some(3)), (None, Some(3)))));
}

#[test]
fn r603_7_a_delayed_triggered_ability_has_none_of_its_sources_characteristics() {
    let creator = "{0}: This creature deals 2 damage to target creature. When that creature dies this turn, you may gain 1 life.";
    let day = CardDef::new("Day", "{2}{W}", "Creature — Human", creator, Some((2, 2))).unwrap();
    let bear = CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap();
    for d in [&day, &bear] { game::play::check(d).unwrap_or_else(|e| panic!("{}: {e:?}", d.name())); }
    let mut g = Game::new(vec![day, bear], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let bear = g.put(1, 1, Zone::Battlefield);
    g.act(0, Move::Activate { source, ability: 0, targets: vec![vec![Target::Object(bear)]], paying: vec![] }).unwrap();
    // the activated ability: not a white 2/2 Human creature costing {2}{W}
    let activated = *g.stack.last().unwrap();
    only_its_text(&g, activated, "Day", "602.2a");
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    assert_eq!(g.obj(bear).zone, Zone::Gone);
    let delayed = *g.stack.last().expect("When that creature dies this turn");
    let (text, prompts) = only_its_text(&g, delayed, "Day", "603.7");
    // the ability that triggered is the delayed one, whose text contains "when" (603.7): once
    // on the stack it "has the text of the ability that created it" (603.3, 405.4), that
    // sentence alone, and none of its source's or its creator's other text
    cite!(["603.3", "405.4", "603.7"]; assert_eq!(text, ["When that creature dies this turn, you may gain 1 life."]));
    for p in &prompts {
        let occurrence = p.entities[&delayed].occurrence.as_ref().unwrap();
        assert_eq!(occurrence.kind, OccurrenceKind::Delayed);
        let made_by = occurrence.created_by.as_deref().expect("the ability that created it");
        cite!(["603.7"]; assert!(made_by.contains("deals 2 damage to target creature")
            && made_by.ends_with("When that creature dies this turn, you may gain 1 life."), "{made_by}"));
    }
}

/// A reflexive triggered ability follows the rules for delayed ones (603.12), and one that
/// does something "at" a step is a delayed one (603.7, 603.7a): each, once it triggers, has its
/// own text (603.3, 405.4), the sentence that says it, not the spell's other sentences.
#[test]
fn r603_12_reflexive_and_step_delayed_abilities_have_their_own_sentence_as_text() {
    for (spell, own) in [
        ("Create a 1/1 white Soldier creature token. You may pay {0}. When you do, you gain 3 life.", "When you do, you gain 3 life."),
        ("Create a 1/1 white Soldier creature token. At the beginning of the next end step, you gain 2 life.", "At the beginning of the next end step, you gain 2 life."),
        ("Create a 1/1 white Soldier creature token. You gain 2 life at the beginning of the next end step.", "You gain 2 life at the beginning of the next end step."),
    ] {
        let def = CardDef::new("Parent", "{0}", "Instant", spell, None).unwrap();
        game::play::check(&def).unwrap_or_else(|e| panic!("{spell}: {e:?}"));
        let mut g = Game::new(vec![def], 2, 20);
        let card = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
        // pass, pay for "you may pay {0}" when asked, until the ability it made is on the stack
        for _ in 0..40 {
            if g.stack.last().is_some_and(|o| g.obj(*o).ability.is_some()) { break; }
            match g.asked() {
                Some((p, Asked::Pay { .. })) => g.act(p, Move::Choose(Answer::Pay { plan: vec![], with: None, tapped: vec![] })).unwrap(),
                _ => g.act(g.priority, Move::Pass).unwrap(),
            };
        }
        let ability = *g.stack.last().expect("the ability the spell made");
        assert!(g.obj(ability).ability.is_some(), "{spell}: {:?}", g.stack);
        let (text, prompts) = only_its_text(&g, ability, "Parent", "603.3");
        cite!(["603.3", "405.4", "603.7", "603.12"]; assert_eq!(text, [own], "{spell}"));
        for p in &prompts {
            let occurrence = p.entities[&ability].occurrence.as_ref().unwrap();
            cite!(["603.7"]; assert_eq!(occurrence.kind, OccurrenceKind::Delayed));
            // the spell that created it is said beside it, whole
            cite!(["603.7"]; assert_eq!(occurrence.created_by.as_deref(), Some(spell)));
        }
    }
}

#[test]
fn r707_10_a_copy_of_a_triggered_ability_has_only_its_text() {
    let copier = CardDef::new("Copier", "{0}", "Artifact", "{0}: Copy target activated or triggered ability you control.", None).unwrap();
    game::play::check(&copier).unwrap_or_else(|e| panic!("Copier: {e:?}"));
    let mut defs: Vec<CardDef> = ["Elusive Otter // Grove's Bounty", "Shock"].into_iter().map(oracle).collect();
    defs.push(copier);
    let mut g = Game::new(defs, 2, 20);
    g.put(0, 0, Zone::Battlefield);
    let copier = g.put(0, 2, Zone::Battlefield);
    let card = g.put(0, 1, Zone::Hand);
    let mut t = T::new(g);
    t.mana(0, "{R}");
    t.g.act(0, paid("R", Move::Cast { card, targets: vec![vec![Target::Player(1)]] })).unwrap();
    let prowess = *t.g.stack.last().unwrap();
    let (original, _) = only_its_text(&t.g, prowess, "Elusive Otter", "603.3");
    t.g.act(0, Move::Activate { source: copier, ability: 0, targets: vec![vec![Target::Object(prowess)]], paying: vec![] }).unwrap();
    only_its_text(&t.g, *t.g.stack.last().unwrap(), "Copier", "602.2a");
    t.g.act(0, Move::Pass).unwrap();
    t.g.act(1, Move::Pass).unwrap();
    let copy = *t.g.stack.last().unwrap();
    assert!(copy != prowess && t.g.stack.contains(&prowess), "the copy is above the original");
    // a copy of an ability copies its characteristics (707.10): its text, and nothing more
    let (text, _) = only_its_text(&t.g, copy, "Elusive Otter", "707.10");
    cite!(["707.10"]; assert_eq!(text, original));
}

/// A rule's inherent triggered ability "has no source" (702.179d speed, 725.2 the monarch):
/// on the stack it has its text and nothing else, so no source and no source's name either.
#[test]
fn r702_179d_a_rules_ability_has_no_source_and_no_sources_name() {
    // speed: a player with speed whose opponent loses life during their turn
    let defs = vec![
        CardDef::new("Engine", "{0}", "Artifact", "Start your engines!", None).unwrap(),
        CardDef::new("Blank", "", "Land", "", None).unwrap(),
        CardDef::new("Drain", "{0}", "Instant", "Target player loses 1 life.", None).unwrap(),
    ];
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..12 { g.put(p, 1, Zone::Library); } }
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g.put(0, 0, Zone::Battlefield);
    let drain = g.put(0, 2, Zone::Hand);
    g.act(0, Move::Cast { card: drain, targets: vec![vec![Target::Player(1)]] }).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    let speed = *g.stack.iter().find(|id| g.obj(**id).ability.is_some_and(|(source, _)| source.is_none())).expect("the speed rule on the stack");
    // monarch: the monarch's end step
    let mut m = Game::new(vec![CardDef::new("Blank", "", "Land", "", None).unwrap()], 2, 20);
    m.monarch = Some(0);
    for p in 0..2 { for _ in 0..12 { m.put(p, 0, Zone::Library); } }
    while m.stack.is_empty() { m.act(m.priority, Move::Pass).unwrap(); }
    let monarch = *m.stack.last().unwrap();
    assert_eq!(m.obj(monarch).ability, Some((None, 0)), "the monarch's end-step draw");
    // their text is the rule's: 702.179d "That ability is "Whenever one or more opponents lose
    // life during your turn, if your speed is less than 4, your speed increases by 1. This
    // ability triggers only once each turn."", 725.2 "The full texts of these abilities are
    // "At the beginning of the monarch's end step, that player draws a card" and …"
    let speed_text = "Whenever one or more opponents lose life during your turn, if your speed is less than 4, your speed increases by 1. This ability triggers only once each turn.";
    let monarch_text = "At the beginning of the monarch's end step, that player draws a card.";
    for (g, ability, rule, says) in [(&g, speed, "702.179d", speed_text), (&m, monarch, "725.2", monarch_text)] {
        let (text, prompts) = only_its_text(g, ability, "", rule);
        cite!([rule, "405.4", "603.3"]; assert_eq!(text, [says]));
        for (seat, p) in prompts.iter().enumerate() {
            let occurrence = p.entities[&ability].occurrence.as_ref().unwrap();
            cite!([rule]; assert_eq!((occurrence.source, occurrence.source_name.as_str()), (None, ""), "p{seat}"));
            cite!([rule]; assert_eq!(occurrence.created_by, None, "p{seat}: a rule's own triggered ability, which no ability created"));
        }
    }
}

/// The monarch's other inherent ability (725.2): "Whenever a creature deals combat damage to
/// the monarch, its controller becomes the monarch." On the stack it has that text (603.3,
/// 405.4), as every seat reads it, and no source.
#[test]
fn r725_2_the_monarchs_combat_damage_ability_has_the_rules_text() {
    let defs = vec![
        CardDef::new("Blank", "", "Land", "", None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
    ];
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..12 { g.put(p, 0, Zone::Library); } }
    g.monarch = Some(1);
    // set up with the game, it has been under its controller's control since before the turn (302.6)
    let bear = g.put(0, 1, Zone::Battlefield);
    while !matches!(g.decision, Some(game::state::Decision::Attackers { .. })) { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Attack(vec![(bear, Target::Player(1))])).unwrap();
    for _ in 0..20 {
        if !g.stack.is_empty() { break; }
        match g.decision {
            Some(game::state::Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    let ability = *g.stack.last().expect("the monarch's combat damage ability");
    assert_eq!(g.obj(ability).ability.map(|(source, _)| source), Some(None));
    let (text, prompts) = only_its_text(&g, ability, "", "725.2");
    cite!(["725.2", "405.4", "603.3"]; assert_eq!(text, ["Whenever a creature deals combat damage to the monarch, its controller becomes the monarch."]));
    for (seat, p) in prompts.iter().enumerate() {
        let occurrence = p.entities[&ability].occurrence.as_ref().unwrap();
        cite!(["725.2"]; assert_eq!((occurrence.source, occurrence.created_by.as_deref()), (None, None), "p{seat}"));
    }
}

/// An emblem has "the abilities defined by the effect that created it" and no other
/// characteristics (114.3); the ability that triggers from it, on the stack, has that
/// ability's text (603.3, 405.4): "Whenever you cast a spell, exile target permanent."
#[test]
fn r114_3_an_emblems_triggered_ability_has_the_emblems_ability_as_text() {
    let venser = "+2: Exile target permanent you own. Return it to the battlefield under your control at the beginning of the next end step.\n−1: Creatures can't be blocked this turn.\n−8: You get an emblem with \"Whenever you cast a spell, exile target permanent.\"";
    let defs = vec![
        CardDef::planeswalker("Venser, the Sojourner", "{3}{W}{U}", "Legendary Planeswalker — Venser", venser, 3).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Blessing", "{0}", "Instant", "You gain 1 life.", None).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap_or_else(|e| panic!("{}: {}", def.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    g.set_initial_counters(source, "loyalty", 8);
    g.act(0, Move::Activate { source, ability: 2, targets: vec![], paying: vec![] }).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    let emblem = *g.command.last().expect("the emblem");
    let bear = g.put(1, 1, Zone::Battlefield);
    let card = g.put(0, 2, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(bear)]])).unwrap();
    let ability = *g.stack.last().unwrap();
    assert_eq!(g.obj(ability).ability.and_then(|(s, _)| s), Some(emblem));
    let (text, _) = only_its_text(&g, ability, &g.obj(emblem).chars.name.clone(), "603.3");
    cite!(["114.3", "603.3", "405.4"]; assert_eq!(text, ["Whenever you cast a spell, exile target permanent."]));
}
