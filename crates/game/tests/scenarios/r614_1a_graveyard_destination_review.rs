//! The graveyard-destination replacements: expectations
//! from 614.1a, 614.6, 616.1, 603.12 and the rulings of Head of the Hunt, Dryad Militant,
//! Rest in Peace and Leyline of the Void.
use crate::support::{cite, paid};
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
const ISLAND: usize = 8;
const ALTAR: usize = 9;
const SHOCK: usize = 10;
const SHRINK: usize = 11;
const TEMPER: usize = 12;

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
        c("Destroy probe", "Instant", "Destroy target creature.", None),
        c("Effect", "Instant", effect, None),
        c("Island", "Basic Land — Island", "", None),
        c("Altar", "Artifact", "Sacrifice a creature: You gain 1 life.", None),
        c("Shock", "Instant", "Shock deals 2 damage to any target.", None),
        c("Shrink", "Instant", "Target creature gets -20/-20 until end of turn.", None),
        c("Fiery Temper", "Instant", "Fiery Temper deals 3 damage to any target.\nMadness {R}", None),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..6 { g.put(p, ISLAND, Zone::Library); } }
    g
}
fn pass(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }
fn settle(g: &mut Game) { while !g.stack.is_empty() && g.decision.is_none() { pass(g); } }
fn cast(g: &mut Game, def: usize, targets: Vec<Vec<Target>>, spending: &str) -> ObjId {
    let spell = g.put(g.priority, def, Zone::Hand);
    g.act(g.priority, paid(spending, Move::Cast { card: spell, targets })).unwrap();
    spell
}
fn at(object: ObjId) -> Vec<Vec<Target>> { vec![vec![Target::Object(object)]] }
/// Where this object is now, following its zone changes (400.7).
fn now(g: &Game, was: ObjId) -> Zone {
    let step = |was: ObjId| g.log.iter().rev().find_map(|e| match e { Event::ZoneChanged { was: w, now, to, .. } if *w == was => Some((*to, *now)), _ => None });
    let mut at = step(was).expect("it moved");
    while let (_, Some(next)) = at && let Some(further) = step(next) { at = further; }
    at.0
}
fn wolves(g: &Game, player: usize) -> usize {
    g.battlefield.iter().filter(|o| g.obj(**o).token() && g.controller(**o) == player).count()
}
fn asked(g: &Game) -> Option<(usize, String, Vec<game::operation::ReplacementId>)> {
    match &g.decision {
        Some(Decision::Choose { player, choice: Choice::Replacement { kind, among, .. } }) => Some((*player, kind.clone(), among.clone())),
        _ => None,
    }
}

/// The dying creature's controller chooses between its own Rest in Peace and the opponent's
/// Head of the Hunt (616.1); only Head of the Hunt's "When you do" then triggers (603.12).
#[test]
fn r616_1_head_of_the_hunt_against_the_dying_creatures_controllers_rest_in_peace() {
    let mut g = game("You gain 1 life.");
    let hunt = g.put(0, HUNT, Zone::Battlefield);
    let peace = g.put(1, PEACE, Zone::Battlefield);
    let theirs = g.put(1, BEAR, Zone::Battlefield);
    cast(&mut g, MURDER, at(theirs), "");
    pass(&mut g);
    let (player, kind, among) = asked(&g).expect("a choice of replacement");
    cite!(["616.1"]; assert_eq!(player, 1, "the dying creature's controller"));
    assert_eq!(kind, "death");
    let by = |source| among.iter().find(|r| r.source == source).unwrap().clone();
    let mut other = g.clone();
    g.act(1, Move::Choose(Answer::Replacement(by(peace)))).unwrap();
    settle(&mut g);
    cite!(["616.1", "614.1a"]; case: "Rest in Peace"; assert_eq!(now(&g, theirs), Zone::Exile));
    cite!(["603.12"]; case: "Rest in Peace"; assert_eq!(wolves(&g, 0), 0, "Head of the Hunt's replacement was not applied"));
    other.act(1, Move::Choose(Answer::Replacement(by(hunt)))).unwrap();
    settle(&mut other);
    cite!(["616.1", "614.1a"]; case: "Head of the Hunt"; assert_eq!(now(&other, theirs), Zone::Exile));
    cite!(["603.12"]; case: "Head of the Hunt"; assert_eq!(wolves(&other, 0), 1));
    // it did not die either way (700.4): its dies trigger never triggers
    cite!(["614.6"]; assert_eq!((g.players[1].life, other.players[1].life), (20, 20)));
}

/// Ruling: if Head of the Hunt leaves at the same time as creatures an opponent controls
/// would die, those are still exiled; its "When you do" still triggers for its controller.
#[test]
fn r603_12_head_of_the_hunt_dying_with_them_still_makes_a_wolf_for_each() {
    let mut g = game("Destroy all creatures.");
    g.put(0, HUNT, Zone::Battlefield);
    let a = g.put(1, BEAR, Zone::Battlefield);
    let b = g.put(1, BEAR, Zone::Battlefield);
    cast(&mut g, EFFECT, vec![], "");
    pass(&mut g);
    for bear in [a, b] { cite!(["614.1a"]; assert_eq!(now(&g, bear), Zone::Exile)); }
    cite!(["603.12"]; assert_eq!(g.stack.len(), 2, "one reflexive trigger per replaced death"));
    settle(&mut g);
    cite!(["603.12", "603.10a"]; assert_eq!(wolves(&g, 0), 2));
    assert_eq!(g.players[1].life, 20);
}

/// Ruling: cards an opponent discards or mills still go to their graveyard; only dying is
/// replaced (700.4).
#[test]
fn r700_4_head_of_the_hunt_does_not_replace_a_discard_or_a_mill() {
    for effect in ["Target player discards a card.", "Target player mills a card."] {
        let mut g = game(effect);
        g.put(0, HUNT, Zone::Battlefield);
        let card = if effect.contains("discards") { g.put(1, BEAR, Zone::Hand) } else { g.top_cards(1, 1)[0] };
        cast(&mut g, EFFECT, vec![vec![Target::Player(1)]], "");
        settle(&mut g);
        cite!(["700.4", "614.1a"]; case: effect; assert_eq!(now(&g, card), Zone::Graveyard));
        assert_eq!(wolves(&g, 0), 0);
    }
}

/// Lethal damage: the state-based death is replaced (704.5g, 614.1a) and the wolf made.
#[test]
fn r704_5g_head_of_the_hunt_replaces_a_state_based_death() {
    let mut g = game("You gain 1 life.");
    g.put(0, HUNT, Zone::Battlefield);
    let theirs = g.put(1, BEAR, Zone::Battlefield);
    cast(&mut g, SHOCK, at(theirs), "");
    settle(&mut g);
    cite!(["704.5g", "614.1a"]; assert_eq!(now(&g, theirs), Zone::Exile));
    cite!(["603.12"]; assert_eq!(wolves(&g, 0), 1));
    // toughness 0 or less (704.5f)
    let theirs = g.put(1, BEAR, Zone::Battlefield);
    cast(&mut g, SHRINK, at(theirs), "");
    settle(&mut g);
    cite!(["704.5f", "614.1a"]; assert_eq!(now(&g, theirs), Zone::Exile));
    cite!(["603.12"]; assert_eq!(wolves(&g, 0), 2));
    assert_eq!(g.players[1].life, 20);
}

/// The state-based death of a creature two replacements with different outcomes apply to:
/// its controller chooses (616.1)./// It is not put into a graveyard and its dies trigger does not trigger.
#[test]
fn r616_1_a_state_based_death_with_two_replacements_is_not_a_death() {
    let mut g = game("You gain 1 life.");
    g.put(0, HUNT, Zone::Battlefield);
    g.put(1, PEACE, Zone::Battlefield);
    let theirs = g.put(1, BEAR, Zone::Battlefield);
    cast(&mut g, SHOCK, at(theirs), "");
    pass(&mut g);
    let (player, _, among) = asked(&g).expect("a choice of replacement");
    cite!(["616.1"]; assert_eq!(player, 1));
    g.act(1, Move::Choose(Answer::Replacement(among[0].clone()))).unwrap();
    settle(&mut g);
    cite!(["614.1a"]; assert_eq!(now(&g, theirs), Zone::Exile));
    assert_eq!(g.players[1].life, 20);
}

/// Sacrificed as a cost by a player whose own Rest in Peace and an opponent's Head of the
/// Hunt both apply: the cost is paid, the creature exiled, never dies (614.1a, 616.1).
#[test]
fn r616_1_a_creature_sacrificed_as_a_cost_with_two_replacements() {
    let mut g = game("You gain 1 life.");
    g.put(0, HUNT, Zone::Battlefield);
    g.put(1, PEACE, Zone::Battlefield);
    let altar = g.put(1, ALTAR, Zone::Battlefield);
    let theirs = g.put(1, BEAR, Zone::Battlefield);
    g.priority = 1;
    g.act(1, Move::Activate { source: altar, ability: 0, targets: vec![], paying: vec![vec![theirs]] }).unwrap();
    let (player, _, among) = asked(&g).expect("a choice of replacement");
    cite!(["616.1"]; assert_eq!(player, 1));
    g.act(1, Move::Choose(Answer::Replacement(among[0].clone()))).unwrap();
    cite!(["614.1a"]; assert_eq!(now(&g, theirs), Zone::Exile));
    settle(&mut g);
    cite!(["614.6"]; assert_eq!(g.players[1].life, 21, "the altar's ability, not the bear's dies trigger"));
}

/// Ruling: an instant that deals lethal damage to Dryad Militant is exiled; Dryad Militant
/// dies only at the next state-based check, after the instant left the stack (608.2n).
#[test]
fn r608_2n_an_instant_dealing_lethal_damage_to_dryad_militant_is_exiled() {
    let mut g = game("You gain 1 life.");
    let dryad = g.put(1, DRYAD, Zone::Battlefield);
    let shock = cast(&mut g, SHOCK, at(dryad), "");
    settle(&mut g);
    cite!(["608.2n", "614.1a"]; assert_eq!(now(&g, shock), Zone::Exile));
    cite!(["704.5g"]; assert_eq!(now(&g, dryad), Zone::Graveyard));
}

/// A countered Darksteel Colossus spell (701.6a): its controller chooses between its own
/// replacement and Rest in Peace's (616.1); each answer is followed.
#[test]
fn r616_1_a_countered_colossus_spell_is_chosen_for_by_its_controller() {
    let mut g = game("Counter target spell.");
    g.put(1, PEACE, Zone::Battlefield);
    g.players[0].pool.colorless = 11;
    let colossus = cast(&mut g, COLOSSUS, vec![], "CCCCCCCCCCC");
    let spell = *g.stack.last().unwrap();
    g.act(0, Move::Pass).unwrap();
    cast(&mut g, EFFECT, at(spell), "");
    pass(&mut g);
    let (player, kind, among) = asked(&g).expect("a choice of replacement");
    cite!(["616.1"]; assert_eq!(player, 0, "the countered spell's controller"));
    assert_eq!(kind, "graveyard");
    let mut other = g.clone();
    g.act(0, Move::Choose(Answer::Replacement(among.iter().cloned().find(|r| r.source == spell).unwrap()))).unwrap();
    cite!(["616.1", "113.6"]; case: "its own"; assert_eq!(now(&g, colossus), Zone::Library));
    let peace = among.iter().cloned().find(|r| r.source != spell).unwrap();
    other.act(0, Move::Choose(Answer::Replacement(peace))).unwrap();
    cite!(["616.1"]; case: "Rest in Peace"; assert_eq!(now(&other, colossus), Zone::Exile));
}

/// Milled Colossus with an opponent's Leyline: its owner chooses (616.1). Milled cards that
/// only exiling replacements apply to are exiled without a question.
#[test]
fn r616_1_mill_asks_only_where_outcomes_differ() {
    let mut g = game("Target player mills two cards.");
    g.put(0, PEACE, Zone::Battlefield);
    g.put(0, LEYLINE, Zone::Battlefield);
    let top = g.top_cards(1, 2);
    cast(&mut g, EFFECT, vec![vec![Target::Player(1)]], "");
    pass(&mut g);
    cite!(["616.1"]; case: "the same outcome"; assert!(asked(&g).is_none(), "{:?}", g.decision));
    for card in top { cite!(["614.1a", "701.17a"]; assert_eq!(now(&g, card), Zone::Exile)); }

    let mut g = game("Target player mills a card.");
    g.put(0, LEYLINE, Zone::Battlefield);
    let colossus = g.put(1, COLOSSUS, Zone::Library);
    cast(&mut g, EFFECT, vec![vec![Target::Player(1)]], "");
    pass(&mut g);
    let (player, kind, among) = asked(&g).expect("a choice of replacement");
    cite!(["616.1"]; assert_eq!(player, 1, "its owner"));
    assert_eq!(kind, "graveyard");
    let leyline = among.iter().cloned().find(|r| r.source != colossus).unwrap();
    g.act(1, Move::Choose(Answer::Replacement(leyline))).unwrap();
    cite!(["616.1"]; assert_eq!(now(&g, colossus), Zone::Exile));
    cite!(["701.17c"]; assert!(g.log.iter().any(|e| matches!(e, Event::Milled { player: 1, .. }))));
}

/// Counterspell with only exiling replacements applying: no question.
#[test]
fn r616_1_countering_asks_nothing_when_only_exile_applies() {
    let mut g = game("Counter target spell.");
    g.put(0, PEACE, Zone::Battlefield);
    g.put(0, LEYLINE, Zone::Battlefield);
    g.put(0, DRYAD, Zone::Battlefield);
    g.priority = 1;
    let shock = cast(&mut g, SHOCK, vec![vec![Target::Player(0)]], "");
    let spell = *g.stack.last().unwrap();
    g.act(1, Move::Pass).unwrap();
    cast(&mut g, EFFECT, at(spell), "");
    pass(&mut g);
    cite!(["616.1"]; assert!(asked(&g).is_none(), "{:?}", g.decision));
    cite!(["614.1a", "701.6a"]; assert_eq!(now(&g, shock), Zone::Exile));
    assert_eq!(g.players[0].life, 20);
}

/// Cleanup retains the selected discard until its owner chooses Colossus or Rest in Peace
/// (514.1, 616.1). Either explicit branch still discards the card, then cleanup continues.
#[test]
fn r514_1_a_colossus_discarded_to_hand_size_goes_to_no_graveyard() {
    let mut g = game("You gain 1 life.");
    let peace = g.put(1, PEACE, Zone::Battlefield);
    let colossus = g.put(0, COLOSSUS, Zone::Hand);
    for _ in 0..7 { g.put(0, ISLAND, Zone::Hand); }
    while !matches!(g.decision, Some(Decision::Discard { .. })) { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Discard(vec![colossus])).unwrap();
    let own = game::operation::ReplacementId::from_ability(colossus, game::binding::AbilityKey::printed(2), 0);
    let exile = game::operation::ReplacementId::from_ability(peace, game::binding::AbilityKey::printed(1), 0);
    cite!(["514.1", "616.1"]; assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::Replacement {
        on: Target::Object(colossus), kind: "discard".into(), count: 1, counters: None, tokens: None,
        among: vec![own.clone(), exile.clone()], optional: false,
    } })));
    assert_eq!(g.players[0].hand.len(), 8);
    assert!(!g.log.iter().any(|event| matches!(event, Event::ZoneChanged { was, .. } if *was == colossus)));
    assert!(!g.log.iter().any(|event| matches!(event, Event::Discarded { .. })));
    let copy = g.clone();
    for (mut branch, choice, destination) in [(g, own, Zone::Library), (copy, exile, Zone::Exile)] {
        branch.act(0, Move::Choose(Answer::Replacement(choice))).unwrap();
        cite!(["514.1", "614.1a", "616.1f"]; assert_eq!(now(&branch, colossus), destination));
        assert!(!branch.log.iter().any(|event| matches!(event, Event::ZoneChanged { was, to: Zone::Graveyard, .. } if *was == colossus)));
        cite!(["701.9a"]; assert_eq!(branch.log.iter().filter(|e| matches!(e, Event::Discarded { player: 0, .. })).count(), 1));
        assert_eq!(branch.players[0].hand.len(), 7);
        assert_eq!(branch.active, 1, "cleanup continued once after the replacement choice");
    }
}

/// The RIP/Leyline discard ruling preserves madness. The owner explicitly chooses its
/// replacement before either effect puts the discarded card elsewhere (702.35a, 616.1f).
#[test]
fn r702_35a_madness_still_works_under_rest_in_peace_and_leyline() {
    for (replacement_def, case) in [(PEACE, "Rest in Peace"), (LEYLINE, "Leyline of the Void")] {
        let mut g = game("Target player discards a card.");
        let replacement = g.put(0, replacement_def, Zone::Battlefield);
        let temper = g.put(1, TEMPER, Zone::Hand);
        cast(&mut g, EFFECT, vec![vec![Target::Player(1)]], "");
        pass(&mut g);
        let madness = game::operation::ReplacementId::from_ability(temper, game::binding::AbilityKey::printed(1), 0);
        let other = game::operation::ReplacementId::from_ability(replacement, game::binding::AbilityKey::printed(1), 0);
        cite!(["616.1"]; assert_eq!(g.decision, Some(Decision::Choose { player: 1, choice: Choice::Replacement {
            on: Target::Object(temper), kind: "discard".into(), count: 1, counters: None, tokens: None,
            among: vec![madness.clone(), other], optional: false,
        } }), "the discarded card's owner chooses"));
        assert!(!g.log.iter().any(|event| matches!(event, Event::Discarded { .. })));
        g.act(1, Move::Choose(Answer::Replacement(madness))).unwrap();
        cite!(["702.35a", "614.1a"]; case: case; assert_eq!(now(&g, temper), Zone::Exile));
        cite!(["702.35a"]; case: case; assert_eq!(g.stack.len(), 1, "its madness trigger"));
        cite!(["701.9a"]; case: case; assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Discarded { player: 1, .. })).count(), 1));
    }
}

/// Head of the Hunt's "When you do" is a reflexive triggered ability its replacement effect
/// creates as it is applied (603.12, 603.7a): on the stack it has its own text (603.3, 405.4),
/// and the prompt says the replacement ability that created it, its whole line.
#[test]
fn r603_12_head_of_the_hunts_reflexive_ability_says_its_replacement_created_it() {
    let mut g = game("You gain 1 life.");
    g.put(0, HUNT, Zone::Battlefield);
    let theirs = g.put(1, BEAR, Zone::Battlefield);
    cast(&mut g, MURDER, at(theirs), "");
    pass(&mut g);
    cite!(["614.1a"]; assert_eq!(now(&g, theirs), Zone::Exile));
    let wolf = *g.stack.last().expect("its \"When you do\" on the stack");
    assert!(g.obj(wolf).ability.is_some());
    for seat in 0..g.players.len() {
        let p = game::simulation::Simulation::new(g.clone()).prompt(seat).unwrap();
        let occurrence = p.entities[&wolf].occurrence.as_ref().unwrap();
        cite!(["603.12", "603.3"]; assert_eq!(occurrence.text.as_deref(), Some("When you do, create a 2/2 green Wolf creature token."), "p{seat}"));
        cite!(["603.12", "603.7a"]; assert_eq!(occurrence.created_by.as_deref(),
            Some("If a creature an opponent controls would die, exile it instead. When you do, create a 2/2 green Wolf creature token."), "p{seat}"));
    }
}
