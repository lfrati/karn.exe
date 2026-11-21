//! Losing abilities (CR 613.1f, 113.10): an effect that says an object loses all abilities,
//! or a named keyword, takes them from what the object has, in timestamp order with what
//! gives abilities (613.7). What it no longer has it neither activates, triggers with, nor
//! applies; what is already on the stack is what it was (113.7a). Every card is read from
//! its text, and each test first shows the ability at work before it is taken away.

use crate::support::cite;
use game::card::CardDef;
use game::legal::Asked;
use game::play::check;
use game::state::Decision;
use game::{Game, Move, ObjId, Target, Zone};

const BEARS: usize = 0;
const HEALER: usize = 1;
const MOURNER: usize = 2;
const RAIDER: usize = 3;
const CAPTAIN: usize = 4;
const WARDEN: usize = 5;
const HAWK: usize = 6;
const STRIP: usize = 7;
const GROUND: usize = 8;
const LIFT: usize = 9;
const FEATHER: usize = 10;
const TEACHER: usize = 11;
const HUMILITY: usize = 12;
const ALTAR: usize = 13;
const FOREST: usize = 14;
const DOUBLER: usize = 15;
const GROWTH: usize = 16;

fn defs() -> Vec<CardDef> {
    let card = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).expect(name);
    vec![
        card("Bears", "Creature — Bear", "", Some((2, 2))),
        card("Healer", "Creature — Cleric", "{0}: You gain 1 life.", Some((1, 1))),
        card("Mourner", "Creature — Cleric", "When this creature dies, you gain 3 life.", Some((1, 1))),
        card("Raider", "Creature — Warrior", "Whenever this creature attacks, you gain 1 life.", Some((2, 2))),
        card("Captain", "Creature — Soldier", "Other creatures you control get +1/+1.", Some((2, 2))),
        card("Warden", "Creature — Wall", "Creatures can't attack.", Some((0, 4))),
        card("Hawk", "Creature — Bird", "Flying, first strike", Some((1, 1))),
        card("Strip", "Enchantment", "{0}: Target permanent loses all abilities until end of turn.", None),
        card("Ground", "Enchantment", "{0}: Target creature loses flying until end of turn.", None),
        card("Lift", "Enchantment", "{0}: Target creature gains flying until end of turn.", None),
        card("Feather", "Enchantment", "{0}: Put a flying counter on target creature.", None),
        card("Teacher", "Enchantment", "{0}: Until end of turn, target creature gains \"{0}: You gain 1 life.\"", None),
        card("Humility", "Enchantment", "All creatures lose all abilities and have base power and toughness 1/1.", None),
        card("Altar", "Artifact", "Sacrifice a creature: Add {C}{C}.", None),
        card("Forest", "Basic Land — Forest", "", None),
        card(
            "Doubler",
            "Artifact",
            "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.",
            None,
        ),
        card("Growth", "Instant", "Put a +1/+1 counter on target creature.", None),
    ]
}

/// Both libraries have cards, so turns can go by.
fn game() -> Game {
    let mut g = Game::new(defs(), 2, 20);
    for player in 0..2 {
        for _ in 0..5 {
            g.put(player, FOREST, Zone::Library);
        }
    }
    g
}

fn all_pass(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}

fn object(o: ObjId) -> Vec<Vec<Target>> {
    vec![vec![Target::Object(o)]]
}

/// Player 0 activates the first ability of `source`, with these targets.
fn activate(g: &mut Game, source: ObjId, targets: Vec<Vec<Target>>) -> Result<(), String> {
    g.act(0, Move::Activate { source, ability: 0, targets, paying: vec![] })
}

/// Player 0 activates the one ability of this enchantment at `target`, and it resolves.
fn apply(g: &mut Game, enchantment: ObjId, target: ObjId) {
    activate(g, enchantment, object(target)).unwrap_or_else(|e| panic!("{}: {e}", g.obj(enchantment).chars.name));
    all_pass(g);
}

fn strip(g: &mut Game, target: ObjId) {
    let strip = g.put(0, STRIP, Zone::Battlefield);
    apply(g, strip, target);
}

/// The least each player can do until this holds.
fn go_until(g: &mut Game, done: impl Fn(&Game) -> bool) {
    for _ in 0..300 {
        if done(g) {
            return;
        }
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            Some(Decision::Discard { player, count }) => {
                let cards = g.players[player].hand[..count].to_vec();
                g.act(player, Move::Discard(cards)).unwrap()
            }
            Some(other) => panic!("unexpected decision: {other:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("the game never got there");
}

/// Player 0's next turn, at its first priority.
fn my_next_turn(g: &mut Game) {
    let turn = g.turn;
    go_until(g, |g| g.turn > turn && g.active == 0 && g.decision.is_none() && g.priority == 0 && g.stack.is_empty());
}

/// Player 0 is asked to declare attackers this turn.
fn to_attackers(g: &mut Game) {
    for _ in 0..50 {
        if matches!(g.decision, Some(Decision::Attackers { player: 0 })) {
            return;
        }
        assert!(g.decision.is_none(), "unexpected decision: {:?}", g.decision);
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("attackers were never asked for");
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r613_1f_an_activated_ability_lost_is_neither_offered_nor_activated_and_returns_at_cleanup() {
    let mut g = game();
    let healer = g.put(0, HEALER, Zone::Battlefield);
    assert_eq!(g.abilities(healer).len(), 1);
    strip(&mut g, healer);
    cite!(["613.1f"]; assert!(g.abilities(healer).is_empty()));
    let before = format!("{g:?}");
    cite!(["613.1f"]; refused("that is not an activated ability of that permanent (602.1c)"): activate(&mut g, healer, vec![]));
    assert_eq!(format!("{g:?}"), before);
    // "until end of turn" ends at cleanup (514.2): it has its ability again
    my_next_turn(&mut g);
    cite!(["514.2"]; assert_eq!(g.abilities(healer).len(), 1));
    activate(&mut g, healer, vec![]).unwrap();
    all_pass(&mut g);
    assert_eq!(g.players[0].life, 21);
}

#[test]
fn r113_7a_an_ability_on_the_stack_resolves_though_its_source_has_since_lost_it() {
    let mut g = game();
    let healer = g.put(0, HEALER, Zone::Battlefield);
    let strip = g.put(0, STRIP, Zone::Battlefield);
    activate(&mut g, healer, vec![]).unwrap();
    // in response, the healer loses all abilities
    activate(&mut g, strip, object(healer)).unwrap();
    assert_eq!(g.stack.len(), 2);
    all_pass(&mut g);
    assert!(g.abilities(healer).is_empty());
    assert_eq!((g.stack.len(), g.players[0].life), (1, 20));
    // the activation made before exists independently of its source
    all_pass(&mut g);
    cite!(["113.7a"]; assert_eq!((g.stack.len(), g.players[0].life), (0, 21)));
    cite!(["613.1f"]; refused("that is not an activated ability of that permanent (602.1c)"): activate(&mut g, healer, vec![]));
}

#[test]
fn r603_2_an_attack_trigger_lost_does_not_trigger_but_one_that_triggered_resolves() {
    // lost before the attack: nothing triggers
    let mut g = game();
    let raider = g.put(0, RAIDER, Zone::Battlefield);
    my_next_turn(&mut g);
    strip(&mut g, raider);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(raider, Target::Player(1))])).unwrap();
    cite!(["603.2"]; assert!(g.stack.is_empty(), "an ability it does not have does not trigger"));
    assert_eq!(g.players[0].life, 20);

    // lost after it triggered: the triggered ability on the stack is unaffected (113.7a)
    let mut g = game();
    let raider = g.put(0, RAIDER, Zone::Battlefield);
    let strip = g.put(0, STRIP, Zone::Battlefield);
    my_next_turn(&mut g);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(raider, Target::Player(1))])).unwrap();
    assert_eq!(g.stack.len(), 1, "the attack triggers");
    activate(&mut g, strip, object(raider)).unwrap();
    all_pass(&mut g);
    assert!(g.abilities(raider).is_empty());
    assert_eq!((g.stack.len(), g.players[0].life), (1, 20));
    all_pass(&mut g);
    cite!(["113.7a"]; assert_eq!((g.stack.len(), g.players[0].life), (0, 21)));
}

#[test]
fn r603_10a_a_creature_that_died_without_its_ability_does_not_trigger_from_how_it_last_was() {
    // with it: it triggers as it dies
    let mut g = game();
    let mourner = g.put(0, MOURNER, Zone::Battlefield);
    let altar = g.put(0, ALTAR, Zone::Battlefield);
    g.act(0, Move::Activate { source: altar, ability: 0, targets: vec![], paying: vec![vec![mourner]] }).unwrap();
    assert_eq!(g.stack.len(), 1);
    all_pass(&mut g);
    assert_eq!(g.players[0].life, 23);

    // having lost it: as it last existed it had no such ability, and its card's text is not
    // looked at again
    let mut g = game();
    let mourner = g.put(0, MOURNER, Zone::Battlefield);
    let altar = g.put(0, ALTAR, Zone::Battlefield);
    strip(&mut g, mourner);
    g.act(0, Move::Activate { source: altar, ability: 0, targets: vec![], paying: vec![vec![mourner]] }).unwrap();
    assert_eq!(g.obj(mourner).zone, Zone::Gone);
    cite!(["603.10a"]; assert!(g.stack.is_empty(), "nothing triggered"));
    assert_eq!(g.players[0].life, 20);

    // the same under a static effect that is still there as it dies
    let mut g = game();
    let mourner = g.put(0, MOURNER, Zone::Battlefield);
    let altar = g.put(0, ALTAR, Zone::Battlefield);
    g.put(1, HUMILITY, Zone::Battlefield);
    g.act(0, Move::Activate { source: altar, ability: 0, targets: vec![], paying: vec![vec![mourner]] }).unwrap();
    cite!(["603.10a"]; assert!(g.stack.is_empty()));
    assert_eq!(g.players[0].life, 20);
}

#[test]
fn r613_1f_a_static_restriction_of_a_creature_that_lost_it_no_longer_applies() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let warden = g.put(1, WARDEN, Zone::Battlefield);
    my_next_turn(&mut g);
    // no creature can attack, so no attackers are asked for (508.8) and none is accepted
    let mut held = g.clone();
    let turn = held.turn;
    for _ in 0..50 {
        if held.turn > turn { break; }
        assert!(!matches!(held.decision, Some(Decision::Attackers { .. })), "creatures can't attack");
        match held.decision.clone() {
            Some(Decision::Discard { player, count }) => { let cards = held.players[player].hand[..count].to_vec(); held.act(player, Move::Discard(cards)).unwrap() }
            Some(other) => panic!("unexpected decision: {other:?}"),
            None => held.act(held.priority, Move::Pass).unwrap(),
        }
    }
    assert!(held.turn > turn);

    strip(&mut g, warden);
    to_attackers(&mut g);
    let Some((0, Asked::Attackers(able))) = g.asked() else { panic!("attackers") };
    cite!(["613.1f"]; assert!(able.iter().any(|a| a.creature == bears && a.attacks == [Target::Player(1)])));
    g.act(0, Move::Attack(vec![(bears, Target::Player(1))])).unwrap();
}

#[test]
fn r613_4_an_effect_of_a_lost_static_ability_no_longer_applies_in_a_later_layer() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let captain = g.put(0, CAPTAIN, Zone::Battlefield);
    assert_eq!((g.power(bears), g.toughness(bears)), (Some(3), Some(3)));
    strip(&mut g, captain);
    cite!(["613.1f"]; assert_eq!((g.power(bears), g.toughness(bears)), (Some(2), Some(2))));
    assert_eq!(g.power(captain), Some(2));
    // and it is back when the effect ends
    my_next_turn(&mut g);
    cite!(["514.2"]; assert_eq!(g.power(bears), Some(3)));

    // Humility: abilities go in layer 6, so the +1/+1 of layer 7c is never there, whichever
    // came first (613.1f, 613.4)
    for humility_first in [true, false] {
        let mut g = game();
        if humility_first {
            g.put(1, HUMILITY, Zone::Battlefield);
        }
        let bears = g.put(0, BEARS, Zone::Battlefield);
        let captain = g.put(0, CAPTAIN, Zone::Battlefield);
        let hawk = g.put(0, HAWK, Zone::Battlefield);
        if !humility_first {
            g.put(1, HUMILITY, Zone::Battlefield);
        }
        for creature in [bears, captain, hawk] {
            cite!(["613.1f", "613.4"]; case: format!("humility first: {humility_first}"); assert_eq!((g.power(creature), g.toughness(creature)), (Some(1), Some(1)), "humility first: {humility_first}"));
            assert!(g.abilities(creature).is_empty());
        }
        cite!(["613.1f"]; case: format!("humility first: {humility_first}"); assert!(!g.has(hawk, "flying") && !g.has(hawk, "first strike")));
    }
}

#[test]
fn r613_7_an_ability_given_before_is_lost_and_one_given_after_is_had() {
    // given, then all abilities lost: the older gift goes with the rest
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let teacher = g.put(0, TEACHER, Zone::Battlefield);
    apply(&mut g, teacher, bears);
    assert_eq!(g.abilities(bears).len(), 1);
    strip(&mut g, bears);
    cite!(["613.7"]; assert!(g.abilities(bears).is_empty()));
    cite!(["613.1f"]; refused("that is not an activated ability of that permanent (602.1c)"): activate(&mut g, bears, vec![]));

    // all abilities lost, then given: distinguish the gift from the printed ability
    let mut ds = defs();
    ds[TEACHER] = CardDef::new("Teacher", "{0}", "Enchantment",
        "{0}: Until end of turn, target creature gains \"{0}: You gain 2 life.\"", None).unwrap();
    check(&ds[TEACHER]).unwrap();
    let mut g = Game::new(ds, 2, 20);
    for player in 0..2 { g.put(player, FOREST, Zone::Library); }
    let healer = g.put(0, HEALER, Zone::Battlefield);
    let teacher = g.put(0, TEACHER, Zone::Battlefield);
    strip(&mut g, healer);
    assert!(g.abilities(healer).is_empty());
    apply(&mut g, teacher, healer);
    cite!(["613.7"]; assert_eq!(g.abilities(healer).len(), 1, "its own is still lost; the given one is not"));
    activate(&mut g, healer, vec![]).unwrap();
    all_pass(&mut g);
    assert_eq!(g.players[0].life, 22);
    assert!(activate(&mut g, healer, vec![]).is_ok(), "{{0}}: it can be activated again");
    all_pass(&mut g);
    // at cleanup both effects end: its own ability again, and no given one
    my_next_turn(&mut g);
    assert_eq!(g.abilities(healer).len(), 1);
    assert_eq!(g.players[0].life, 24);
    activate(&mut g, healer, vec![]).unwrap();
    all_pass(&mut g);
    cite!(["514.2"]; assert_eq!(g.players[0].life, 25, "only the printed one-life ability remains"));
}

#[test]
fn r613_7_a_keyword_granted_and_all_abilities_lost_apply_in_timestamp_order() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let lift = g.put(0, LIFT, Zone::Battlefield);
    apply(&mut g, lift, bears);
    assert!(g.has(bears, "flying"));
    strip(&mut g, bears);
    cite!(["613.7"]; assert!(!g.has(bears, "flying"), "granted first, then lost"));

    let mut g = game();
    let hawk = g.put(0, HAWK, Zone::Battlefield);
    let lift = g.put(0, LIFT, Zone::Battlefield);
    strip(&mut g, hawk);
    assert!(!g.has(hawk, "flying") && !g.has(hawk, "first strike"));
    apply(&mut g, lift, hawk);
    cite!(["613.7"]; assert!(g.has(hawk, "flying"), "lost first, then granted"));
    cite!(["613.7"]; assert!(!g.has(hawk, "first strike"), "what it had of its own is still lost"));
}

#[test]
fn r613_7_a_keyword_counter_and_all_abilities_lost_apply_in_timestamp_order() {
    // 122.1b: a keyword counter gives the keyword; it is timestamped as it is put (613.7)
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let feather = g.put(0, FEATHER, Zone::Battlefield);
    apply(&mut g, feather, bears);
    assert!(g.has(bears, "flying"));
    strip(&mut g, bears);
    cite!(["613.7c"]; assert!(!g.has(bears, "flying"), "the counter was put before the abilities were lost"));
    assert_eq!(g.obj(bears).counters.get("flying"), Some(&1), "the counter itself stays");
    // when the loss ends the counter gives flying again
    my_next_turn(&mut g);
    assert!(g.has(bears, "flying"));

    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let feather = g.put(0, FEATHER, Zone::Battlefield);
    strip(&mut g, bears);
    apply(&mut g, feather, bears);
    cite!(["613.7c"]; assert!(g.has(bears, "flying"), "the counter was put after the abilities were lost"));
}

#[test]
fn r613_1f_losing_a_named_keyword_takes_that_keyword_alone_in_timestamp_order() {
    let mut g = game();
    let hawk = g.put(0, HAWK, Zone::Battlefield);
    let ground = g.put(0, GROUND, Zone::Battlefield);
    let lift = g.put(0, LIFT, Zone::Battlefield);
    assert!(g.has(hawk, "flying") && g.has(hawk, "first strike"));
    apply(&mut g, ground, hawk);
    cite!(["613.1f"]; assert!(!g.has(hawk, "flying")));
    cite!(["113.10"]; assert!(g.has(hawk, "first strike"), "another keyword is not lost"));
    // granted afterwards, it has flying; lost again afterwards, it does not
    apply(&mut g, lift, hawk);
    cite!(["613.7"]; assert!(g.has(hawk, "flying")));
    apply(&mut g, ground, hawk);
    cite!(["613.7"]; assert!(!g.has(hawk, "flying")));
    my_next_turn(&mut g);
    assert!(g.has(hawk, "flying") && g.has(hawk, "first strike"));
}

#[test]
fn r305_6_a_land_that_lost_all_abilities_has_no_mana_ability_of_its_type() {
    let mut g = game();
    let forest = g.put(0, FOREST, Zone::Battlefield);
    let other = g.put(0, FOREST, Zone::Battlefield);
    assert_eq!(g.abilities(forest).len(), 1);
    strip(&mut g, forest);
    cite!(["305.6", "613.1f"]; assert!(g.abilities(forest).is_empty()));
    cite!(["305.6", "613.1f"]; refused("that is not an activated ability of that permanent (602.1c)"): g.act(0, Move::Activate { source: forest, ability: 0, targets: vec![], paying: vec![] }));
    assert!(!g.obj(forest).tapped);
    assert_eq!(g.players[0].pool.total(), 0);
    // another Forest is untouched
    g.act(0, Move::Activate { source: other, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    assert_eq!(g.players[0].pool.total(), 1);
}

#[test]
fn r614_1_a_replacement_effect_of_a_permanent_that_lost_it_does_not_replace() {
    let grow = |g: &mut Game, creature: ObjId| {
        let growth = g.put(0, GROWTH, Zone::Hand);
        g.act(0, Move::Cast { card: growth, targets: object(creature) }).unwrap();
        all_pass(g);
    };
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let doubler = g.put(0, DOUBLER, Zone::Battlefield);
    grow(&mut g, bears);
    cite!(["614.1a"]; assert_eq!(g.obj(bears).counters.get("+1/+1"), Some(&2)));
    strip(&mut g, doubler);
    grow(&mut g, bears);
    cite!(["613.1f"]; assert_eq!(g.obj(bears).counters.get("+1/+1"), Some(&3), "one more, not two"));
    my_next_turn(&mut g);
    grow(&mut g, bears);
    cite!(["614.1a"]; assert_eq!(g.obj(bears).counters.get("+1/+1"), Some(&5)));
}

#[test]
fn r613_6_a_coordinated_effect_finishes_after_removing_its_own_ability() {
    let mut definitions = defs();
    let animate = definitions.len();
    definitions.push(CardDef::new("Animate", "{0}", "Instant",
        "Target noncreature permanent becomes a 3/3 creature until end of turn.", None).unwrap());
    for d in &definitions { check(d).unwrap(); }
    let mut g = Game::new(definitions, 2, 20);
    let humility = g.put(0, HUMILITY, Zone::Battlefield);
    let bear = g.put(0, BEARS, Zone::Battlefield);
    let spell = g.put(0, animate, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: object(humility) }).unwrap();
    all_pass(&mut g);
    assert!(g.abilities(humility).is_empty());
    // The newer animation wins the 7b timestamp comparison on its target.
    cite!(["613.7"]; assert_eq!(g.seen(humility, None).pt, Some((3, 3))));
    cite!(["613.6"]; assert_eq!(g.seen(bear, None).pt, Some((1, 1))));
}

#[test]
fn unsupported_removal_scopes_and_partial_keyword_families_are_refused() {
    for (types, text) in [
        ("Enchantment", "Cards in graveyards lose all abilities."),
        ("Instant", "Target creature loses protection from red until end of turn."),
    ] {
        let d = CardDef::new("Unsupported", "{0}", types, text, None).unwrap();
        let reason=if types=="Enchantment" {"loses: ability removal outside a permanent scope"}
            else {"loses: a parameterized keyword"};
        crate::support::refused(check(&d).map_err(|e|e.0),reason);
    }
}

#[test]
fn r613_7c_another_counter_of_the_same_kind_refreshes_their_shared_timestamp() {
    let mut g = game();
    let creature = g.put(0, BEARS, Zone::Battlefield);
    g.put_counters(creature, "flying", 1);
    strip(&mut g, creature);
    assert!(!g.seen(creature, None).has(ir::Keyword::from_name("Flying").unwrap()));
    g.put_counters(creature, "flying", 1);
    assert_eq!(g.obj(creature).counters["flying"], 2);
    cite!(["613.7c"]; assert!(g.seen(creature, None).has(ir::Keyword::from_name("Flying").unwrap())));
}
