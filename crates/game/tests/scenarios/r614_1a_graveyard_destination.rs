//! Static replacements of where something would be put instead of a graveyard (614.1a):
//! "If a card or token would be put into a graveyard from anywhere, exile it instead." (Rest in
//! Peace), its opponent-only and instant-or-sorcery forms, the self form that functions in
//! every zone (Darksteel Colossus, 113.6), and the class-wide death form with a reflexive
//! follow-up (Head of the Hunt, 603.12). The replaced event never happens (614.6); where two
//! with different outcomes apply, the affected object's controller, or its owner, chooses
//! (616.1).
use crate::support::cite;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};

const PEACE: usize = 0;
const LEYLINE: usize = 1;
const DRYAD: usize = 2;
const COLOSSUS: usize = 3;
const HUNT: usize = 4;
const BEAR: usize = 5;
const MURDER: usize = 6;
const EFFECT: usize = 7;
const WATCHER: usize = 8;
const ISLAND: usize = 9;
const ALTAR: usize = 10;
const FIRST: usize = 11;
const GRIZZLY: usize = 12;

fn game(effect: &str) -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap();
    let c = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).expect(name);
    let defs = vec![
        real("Rest in Peace"),
        real("Leyline of the Void"),
        real("Dryad Militant"),
        real("Darksteel Colossus"),
        real("Head of the Hunt"),
        c("Bear", "Creature — Bear", "When this creature dies, you gain 3 life.", Some((2, 2))),
        c("Murder", "Instant", "Destroy target creature.", None),
        c("Effect", "Instant", effect, None),
        c("Watcher", "Enchantment", "Whenever a creature dies, you gain 1 life.", None),
        c("Island", "Basic Land — Island", "", None),
        c("Altar", "Artifact", "Sacrifice a creature: You gain 1 life.", None),
        c("First", "Instant", "You gain 1 life.", None),
        c("Grizzly", "Creature — Bear", "", Some((2, 2))),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..6 { g.put(p, ISLAND, Zone::Library); } }
    g
}
fn pass(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }
fn cast(g: &mut Game, def: usize, targets: Vec<Vec<Target>>) -> ObjId {
    let spell = g.put(g.priority, def, Zone::Hand);
    g.act(g.priority, Move::Cast { card: spell, targets }).unwrap();
    spell
}
fn murder(g: &mut Game, creature: ObjId) -> ObjId { let spell = cast(g, MURDER, vec![vec![Target::Object(creature)]]); pass(g); spell }
/// Where this object is now, and the object it became by its zone changes (400.7).
fn now(g: &Game, was: ObjId) -> (Zone, Option<ObjId>) {
    let step = |was: ObjId| g.log.iter().rev().find_map(|e| match e { Event::ZoneChanged { was: w, now, to, .. } if *w == was => Some((*to, *now)), _ => None });
    let mut at = step(was).expect("it moved");
    while let (_, Some(next)) = at && let Some(further) = step(next) { at = further; }
    at
}
fn to_graveyard(g: &Game, was: ObjId) -> bool {
    g.log.iter().any(|e| matches!(e, Event::ZoneChanged { was: w, to: Zone::Graveyard, .. } if *w == was))
}
fn asked(g: &Game) -> (u32, String, Vec<game::operation::ReplacementId>) {
    match &g.decision {
        Some(Decision::Choose { player, choice: Choice::Replacement { kind, among, optional: false, .. } }) => (*player as u32, kind.clone(), among.clone()),
        other => panic!("expected a choice of replacement, got {other:?}"),
    }
}

#[test]
fn r614_1a_rest_in_peace_exiles_a_destroyed_creature_without_the_stack_or_its_dies_triggers() {
    let mut g = game("You gain 1 life.");
    g.put(0, PEACE, Zone::Battlefield);
    g.put(0, WATCHER, Zone::Battlefield);
    let bear = g.put(1, BEAR, Zone::Battlefield);
    let spell = murder(&mut g, bear);
    // nothing to respond to: a replacement effect does not use the stack (614.1a)
    cite!(["614.1a"]; assert!(g.stack.is_empty() && g.decision.is_none()));
    let (zone, card) = now(&g, bear);
    cite!(["614.6"]; assert_eq!(zone, Zone::Exile));
    cite!(["614.6"]; assert!(!to_graveyard(&g, bear), "it was never put into a graveyard"));
    // neither its own "when this dies" nor "whenever a creature dies" looks back to a death
    // that never happened (603.10a, 614.6)
    cite!(["614.6", "603.10a"]; assert_eq!((g.players[0].life, g.players[1].life), (20, 20)));
    cite!(["400.7"]; assert_ne!(card, Some(bear)));
    assert!(g.log.iter().any(|e| matches!(e, Event::Exiled { object } if *object == bear)));
    // the spell is a card put into a graveyard too
    cite!(["614.1a"]; assert_eq!(now(&g, spell).0, Zone::Exile));
    assert!(g.players.iter().all(|p| p.graveyard.is_empty()));
}

#[test]
fn r614_1a_rest_in_peace_replaces_discard_mill_countering_and_a_sacrifice_cost() {
    // discard (701.9a): still discarded, found in exile
    let mut g = game("Discard a card.");
    g.put(0, PEACE, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Hand);
    cast(&mut g, EFFECT, vec![]);
    pass(&mut g);
    if g.decision.is_some() { g.act(0, Move::Choose(Answer::Objects(vec![bear]))).unwrap(); }
    let (zone, card) = now(&g, bear);
    cite!(["614.1a", "701.9a"]; assert_eq!(zone, Zone::Exile));
    cite!(["701.9a"]; assert!(g.log.iter().any(|e| matches!(e, Event::Discarded { card: c, .. } if Some(*c) == card))));

    // mill (701.17a): still milled, found in exile (701.17c)
    let mut g = game("Mill two cards.");
    g.put(0, PEACE, Zone::Battlefield);
    let top = g.top_cards(0, 2);
    cast(&mut g, EFFECT, vec![]);
    pass(&mut g);
    for card in top {
        cite!(["614.1a", "701.17a"]; assert_eq!(now(&g, card).0, Zone::Exile));
        cite!(["701.17c"]; assert!(g.log.iter().any(|e| matches!(e, Event::Milled { player: 0, card: c } if Some(*c) == now(&g, card).1))));
    }
    assert_eq!(g.players[0].library.len(), 4);

    // countered (701.6a): the countered spell and the counterspell are both exiled
    let mut g = game("Counter target spell.");
    g.put(0, PEACE, Zone::Battlefield);
    cast(&mut g, FIRST, vec![]);
    let first = *g.stack.last().unwrap();
    let counter = cast(&mut g, EFFECT, vec![vec![Target::Object(first)]]);
    pass(&mut g);
    cite!(["614.1a", "701.6a"]; assert_eq!(now(&g, first).0, Zone::Exile));
    assert_eq!(g.players[0].life, 20);
    assert_eq!(now(&g, counter).0, Zone::Exile);

    // sacrificed as a cost (118.3): the cost is still paid
    let mut g = game("You gain 1 life.");
    g.put(0, PEACE, Zone::Battlefield);
    let altar = g.put(0, ALTAR, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    g.act(0, Move::Activate { source: altar, ability: 0, targets: vec![], paying: vec![vec![bear]] }).unwrap();
    cite!(["614.1a"]; assert_eq!(now(&g, bear).0, Zone::Exile));
    cite!(["118.11"]; assert!(g.log.iter().any(|e| matches!(e, Event::Sacrificed { object, .. } if *object == bear))));
    pass(&mut g);
    cite!(["614.6"]; assert_eq!(g.players[0].life, 21, "the altar's ability, not the bear's death trigger"));
}

#[test]
fn r704_5d_a_token_exiled_instead_still_ceases_to_exist() {
    let mut g = game("Create a 1/1 white Soldier creature token.");
    g.put(0, PEACE, Zone::Battlefield);
    cast(&mut g, EFFECT, vec![]);
    pass(&mut g);
    let token = *g.battlefield.iter().find(|o| g.obj(**o).token()).unwrap();
    murder(&mut g, token);
    let (zone, now) = now(&g, token);
    cite!(["614.1a"]; assert_eq!(zone, Zone::Exile));
    g.state_based_actions();
    cite!(["704.5d", "111.7"]; assert_eq!(g.obj(now.unwrap()).zone, Zone::Gone));
}

#[test]
fn r614_1a_leyline_exiles_only_cards_put_into_an_opponents_graveyard() {
    let mut g = game("Create a 1/1 white Soldier creature token.");
    g.put(0, LEYLINE, Zone::Battlefield);
    g.put(0, WATCHER, Zone::Battlefield);
    // already there: a replacement changes an event as it would happen, not a past one (614.1)
    let there = g.put(1, BEAR, Zone::Graveyard);
    let theirs = g.put(1, GRIZZLY, Zone::Battlefield);
    let mine = g.put(0, GRIZZLY, Zone::Battlefield);
    murder(&mut g, theirs);
    cite!(["614.1a"]; assert_eq!(now(&g, theirs).0, Zone::Exile));
    cite!(["614.1"]; assert_eq!(g.players[1].graveyard, vec![there]));
    assert_eq!(g.players[0].life, 20);
    murder(&mut g, mine);
    cite!(["614.1a"]; case: "your own graveyard"; assert_eq!(now(&g, mine).0, Zone::Graveyard));
    pass(&mut g);
    assert_eq!(g.players[0].life, 21, "your bear died");
    // a token is not a card: it dies (111.1)
    let mut g = game("Create a 1/1 white Soldier creature token.");
    g.put(0, LEYLINE, Zone::Battlefield);
    g.put(0, WATCHER, Zone::Battlefield);
    g.priority = 1;
    cast(&mut g, EFFECT, vec![]);
    pass(&mut g);
    let token = *g.battlefield.iter().find(|o| g.obj(**o).token()).unwrap();
    g.priority = 0;
    murder(&mut g, token);
    cite!(["111.1", "614.1a"]; case: "a token"; assert_eq!(now(&g, token).0, Zone::Graveyard));
    pass(&mut g);
    assert_eq!(g.players[0].life, 21);
}

#[test]
fn r614_1a_dryad_militant_exiles_instants_and_sorceries_unless_it_left_first() {
    let mut g = game("You gain 1 life.");
    let dryad = g.put(0, DRYAD, Zone::Battlefield);
    let bear = g.put(1, BEAR, Zone::Battlefield);
    let spell = murder(&mut g, bear);
    cite!(["614.1a"]; case: "a creature card"; assert_eq!(now(&g, bear).0, Zone::Graveyard));
    cite!(["614.1a"]; case: "an instant card"; assert_eq!(now(&g, spell).0, Zone::Exile));
    pass(&mut g);
    // destroyed by the spell, it is gone before the spell is put anywhere (608.2n)
    let spell = murder(&mut g, dryad);
    cite!(["608.2n"]; assert_eq!(now(&g, spell).0, Zone::Graveyard));
}

#[test]
fn r614_1a_rest_in_peace_destroyed_by_a_spell_is_exiled_and_the_spell_then_goes_to_the_graveyard() {
    let mut g = game("Destroy target enchantment.");
    let peace = g.put(0, PEACE, Zone::Battlefield);
    let spell = cast(&mut g, EFFECT, vec![vec![Target::Object(peace)]]);
    pass(&mut g);
    // it applies to its own move, as it would happen (614.1a)
    cite!(["614.1a"]; assert_eq!(now(&g, peace).0, Zone::Exile));
    cite!(["608.2n"]; assert_eq!(now(&g, spell).0, Zone::Graveyard));
    // and to everything destroyed with it
    let mut g = game("Destroy all nonland permanents.");
    let peace = g.put(0, PEACE, Zone::Battlefield);
    let bear = g.put(1, BEAR, Zone::Battlefield);
    cast(&mut g, EFFECT, vec![]);
    pass(&mut g);
    for object in [peace, bear] { cite!(["614.1a"]; assert_eq!(now(&g, object).0, Zone::Exile)); }
}

#[test]
fn r113_6_darksteel_colossus_is_revealed_and_shuffled_into_its_library_from_anywhere() {
    for (from, effect) in [(Zone::Battlefield, "You gain 1 life."), (Zone::Hand, "Discard a card."), (Zone::Library, "Mill a card.")] {
        let mut g = game(effect);
        let colossus = g.put(0, COLOSSUS, from);
        if from == Zone::Battlefield {
            let altar = g.put(0, ALTAR, Zone::Battlefield);
            g.act(0, Move::Activate { source: altar, ability: 0, targets: vec![], paying: vec![vec![colossus]] }).unwrap();
        } else {
            cast(&mut g, EFFECT, vec![]);
            pass(&mut g);
        }
        let (zone, _) = now(&g, colossus);
        cite!(["113.6", "614.1a"]; case: format!("{from:?}"); assert_eq!(zone, Zone::Library));
        cite!(["614.6"]; case: format!("{from:?}"); assert!(!to_graveyard(&g, colossus)));
        // revealed, it is shuffled to where nobody knows (701.24a)
        assert_eq!(g.players[0].library.iter().filter(|c| g.obj(**c).card == Some(COLOSSUS)).count(), 1);
        assert!(g.log.iter().any(|e| matches!(e, Event::Revealed { cards, .. } if cards.contains(&colossus))));
        cite!(["701.24a"]; case: format!("{from:?}"); assert!(g.log.iter().any(|e| matches!(e, Event::Shuffled { player: 0, .. }))));
    }
}

#[test]
fn r616_1_the_affected_creatures_controller_chooses_among_its_own_and_its_opponents_replacements() {
    let mut g = game("Each player sacrifices a creature.");
    g.put(0, PEACE, Zone::Battlefield);
    g.put(0, LEYLINE, Zone::Battlefield);
    let colossus = g.put(1, COLOSSUS, Zone::Battlefield);
    cast(&mut g, EFFECT, vec![]);
    pass(&mut g);
    let (player, kind, among) = asked(&g);
    cite!(["616.1"]; assert_eq!(player, 1, "the sacrificed creature's controller"));
    assert_eq!(kind, "death");
    cite!(["616.1"]; assert_eq!(among.len(), 3, "its own, Rest in Peace's and Leyline's"));
    crate::support::refused(g.act(0, Move::Choose(Answer::Replacement(among[1].clone()))), "that decision is another player's");
    let mut own = g.clone();
    g.act(1, Move::Choose(Answer::Replacement(among[1].clone()))).unwrap();
    // once one has applied it is no longer put into a graveyard: the other two do not apply
    cite!(["616.1f"]; assert_eq!(now(&g, colossus).0, Zone::Exile));
    assert!(!g.log.iter().any(|e| matches!(e, Event::Shuffled { .. })));
    own.act(1, Move::Choose(Answer::Replacement(among[0].clone()))).unwrap();
    cite!(["616.1f"]; case: "its own"; assert_eq!(now(&own, colossus).0, Zone::Library));
}

#[test]
fn r616_1_a_discarded_card_two_replacements_apply_to_is_chosen_for_by_its_owner() {
    let mut g = game("Each player discards a card.");
    let peace = g.put(1, PEACE, Zone::Battlefield);
    let colossus = g.put(0, COLOSSUS, Zone::Hand);
    cast(&mut g, EFFECT, vec![]);
    pass(&mut g);
    let own = game::operation::ReplacementId::from_ability(colossus, game::binding::AbilityKey::printed(2), 0);
    let exile = game::operation::ReplacementId::from_ability(peace, game::binding::AbilityKey::printed(1), 0);
    cite!(["616.1"]; assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::Replacement {
        on: Target::Object(colossus), kind: "discard".into(), count: 1, counters: None, tokens: None,
        among: vec![own.clone(), exile.clone()], optional: false,
    } }), "its owner, whose hand it is in, chooses before the discard"));
    assert!(g.players[0].hand.contains(&colossus));
    assert!(!g.log.iter().any(|event| matches!(event, Event::Discarded { .. })));
    let copy = g.clone();
    for (mut branch, choice, destination) in [(g, own, Zone::Library), (copy, exile, Zone::Exile)] {
        branch.act(0, Move::Choose(Answer::Replacement(choice))).unwrap();
        cite!(["616.1f"]; assert_eq!(now(&branch, colossus).0, destination));
        cite!(["614.6"]; assert!(!to_graveyard(&branch, colossus)));
        cite!(["701.9a"]; assert_eq!(branch.log.iter().filter(|e| matches!(e, Event::Discarded { player: 0, .. })).count(), 1));
        assert!(branch.players[0].hand.is_empty());
        assert!(branch.decision.is_none());
    }
}

#[test]
fn r603_12_head_of_the_hunt_exiles_an_opponents_dying_creature_and_then_makes_a_wolf() {
    let mut g = game("Destroy all creatures.");
    let hunt = g.put(0, HUNT, Zone::Battlefield);
    let theirs = g.put(1, BEAR, Zone::Battlefield);
    murder(&mut g, theirs);
    cite!(["614.1a"]; assert_eq!(now(&g, theirs).0, Zone::Exile));
    // "When you do": it triggers as the replacement is applied (603.12)
    cite!(["603.12"]; assert_eq!(g.stack.len(), 1));
    pass(&mut g);
    let wolves = |g: &Game| g.battlefield.iter().filter(|o| g.obj(**o).token() && g.controller(**o) == 0).count();
    cite!(["603.12"]; assert_eq!(wolves(&g), 1));
    cite!(["614.6"]; assert_eq!((g.players[0].life, g.players[1].life), (20, 20)));
    // your own creature dies
    let mine = g.put(0, BEAR, Zone::Battlefield);
    murder(&mut g, mine);
    cite!(["614.1a"]; case: "your creature"; assert_eq!(now(&g, mine).0, Zone::Graveyard));
    // leaving at the same time, it still applies to those that die with it
    let theirs = g.put(1, BEAR, Zone::Battlefield);
    while !g.stack.is_empty() { pass(&mut g); }
    cast(&mut g, EFFECT, vec![]);
    pass(&mut g);
    cite!(["614.1a"]; case: "with it"; assert_eq!(now(&g, theirs).0, Zone::Exile));
    assert_eq!(now(&g, hunt).0, Zone::Graveyard);
}
