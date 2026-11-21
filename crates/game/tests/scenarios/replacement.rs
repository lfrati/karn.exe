//! Replacement effects (CR 614), rule by rule: how a permanent enters the battlefield, and
//! the counters it enters with (122). Every card is read from its printed text.

use crate::support::{cite, paid};
use game::card::CardDef;
use game::play::check;
use game::state::{Answer, Choice, Decision};
use game::{Event, Game, Move, ObjId, Target, Zone};
use ir::{Color, Part};

const FOREST: usize = 0;
const SWAMP: usize = 1;
const PLAINS: usize = 2;
const BEARS: usize = 3;
const GHOUL: usize = 4;
const AGNA: usize = 5;
const VERDURE: usize = 6;
const WATCHDOG: usize = 7;
const DIAMOND: usize = 8;
const FLIGHT: usize = 9;
const ORB: usize = 10;
const SOVEREIGN: usize = 11;
const ALARM: usize = 12;
const GRUMGULLY: usize = 13;
const MOUNTAIN: usize = 14;
const SKELETONS: usize = 15;
const WARD: usize = 16;
const MURDER: usize = 17;
const TERROR: usize = 18;
const WRATH: usize = 19;
const SHOCK: usize = 20;
const GIANT: usize = 21;
const TOMB: usize = 22;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        card("Swamp", "", "Basic Land — Swamp", "({T}: Add {B}.)", None),
        card("Plains", "", "Basic Land — Plains", "({T}: Add {W}.)", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Shambling Ghoul", "{1}{B}", "Creature — Zombie", "This creature enters tapped.", Some((2, 3))),
        card(
            "Agna Qel'a",
            "",
            "Land",
            "This land enters tapped unless you control a basic land.\n{T}: Add {U}.\n{2}{U}, {T}: Draw a card, then discard a card.",
            None,
        ),
        card(
            "Sodden Verdure",
            "",
            "Land — Forest Island",
            "({T}: Add {G} or {U}.)\nThis land enters tapped unless you control two or more basic lands.",
            None,
        ),
        card(
            "Faithful Watchdog",
            "{G}{W}",
            "Creature — Dog",
            "Vigilance\nThis creature enters with three +1/+1 counters on it.",
            Some((0, 0)),
        ),
        card("Moss Diamond", "{2}", "Artifact", "This artifact enters tapped.\n{T}: Add {G}.", None),
        card("Spontaneous Flight", "{2}{W}", "Instant", "Target creature gets +2/+2 until end of turn. Put a flying counter on it.", None),
        card("Orb of Dreams", "{3}", "Artifact", "Permanents enter tapped.", None),
        card("Imposing Sovereign", "{1}{W}", "Creature — Human Noble", "Creatures your opponents control enter tapped.", Some((2, 1))),
        card("Raise the Alarm", "{1}{W}", "Instant", "Create two 1/1 white Soldier creature tokens.", None),
        card(
            "Grumgully, the Generous",
            "{1}{R}{G}",
            "Legendary Creature — Goblin Shaman",
            "Each other non-Human creature you control enters with an additional +1/+1 counter on it.",
            Some((3, 3)),
        ),
        card("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None),
        card(
            "Drudge Skeletons",
            "{1}{B}",
            "Creature — Skeleton",
            "{B}: Regenerate this creature. (The next time this creature would be destroyed this turn, instead tap it, remove it from combat, and heal all damage on it.)",
            Some((1, 1)),
        ),
        card("Death Ward", "{W}", "Instant", "Regenerate target creature.", None),
        card("Murder", "{1}{B}{B}", "Instant", "Destroy target creature.", None),
        card("Terror", "{1}{B}", "Instant", "Destroy target nonartifact, nonblack creature. It can't be regenerated.", None),
        card("Wrath of God", "{2}{W}{W}", "Sorcery", "Destroy all creatures. They can't be regenerated.", None),
        card("Shock", "{R}", "Instant", "Shock deals 2 damage to any target.", None),
        card("Hill Giant", "{3}{R}", "Creature — Giant", "", Some((3, 3))),
        card(
            "Overgrown Tomb",
            "",
            "Land — Swamp Forest",
            "({T}: Add {B} or {G}.)\nAs this land enters, you may pay 2 life. If you don't, it enters tapped.",
            None,
        ),
    ]
}

fn game() -> Game {
    Game::new(defs(), 2, 20)
}

fn pass_both(g: &mut Game) {
    let first = g.priority;
    g.act(first, Move::Pass).unwrap();
    g.act(1 - first, Move::Pass).unwrap();
}

/// Cast a card with the mana of these lands, and let it resolve.
fn cast(g: &mut Game, def: usize, lands: &[usize], targets: Vec<Vec<Target>>, spending: &str) {
    for land in lands {
        let land = g.put(0, *land, Zone::Battlefield);
        g.act(0, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
    let card = g.put(0, def, Zone::Hand);
    g.act(0, paid(spending, Move::Cast { card, targets })).unwrap();
    pass_both(g);
}

fn permanent(g: &Game, name: &str) -> ObjId {
    g.battlefield.iter().copied().find(|o| g.obj(*o).chars.name == name).unwrap_or_else(|| panic!("{name} is on the battlefield"))
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r614_1d_a_permanent_that_enters_tapped_was_never_untapped_on_the_battlefield() {
    // "This creature enters tapped.": the event of entering untapped is replaced (614.6), so
    // nothing became tapped
    let mut g = game();
    cast(&mut g, GHOUL, &[SWAMP, SWAMP], vec![], "BB");
    let ghoul = permanent(&g, "Shambling Ghoul");
    cite!(["614.1d"]; assert!(g.obj(ghoul).tapped));
    cite!(["614.6"]; assert!(!g.log.iter().any(|e| matches!(e, Event::Tapped { object } if *object == ghoul)), "it did not become tapped"));
    // a land that is played, likewise: it can't be tapped for mana this turn (107.5)
    let mut g = game();
    let agna = g.put(0, AGNA, Zone::Hand);
    g.act(0, Move::PlayLand(agna)).unwrap();
    let agna = permanent(&g, "Agna Qel'a");
    cite!(["614.1d"]; assert!(g.obj(agna).tapped));
    cite!(["107.5"]; refused("Agna Qel'a can't pay {T} as it is (107.5, 118.3)"): g.act(0, Move::Activate { source: agna, ability: 1, targets: vec![], paying: vec![] }));
    // and an artifact, whose mana is for the next turn
    let mut g = game();
    cast(&mut g, DIAMOND, &[FOREST, FOREST], vec![], "GG");
    cite!(["614.1d"]; assert!(g.obj(permanent(&g, "Moss Diamond")).tapped));
}

#[test]
fn r614_12a_a_choice_of_how_a_permanent_enters_is_made_before_it_enters() {
    // "As this land enters, you may pay 2 life. If you don't, it enters tapped."
    let asked = |g: &Game| matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May }));
    // paid: 2 life, and it enters untapped
    let mut g = game();
    let tomb = g.put(0, TOMB, Zone::Hand);
    g.act(0, Move::PlayLand(tomb)).unwrap();
    assert!(asked(&g));
    cite!(["614.12a"]; assert!(g.battlefield.is_empty(), "the choice is made before it enters"));
    crate::support::refused(g.act(0, Move::Pass), "a decision is pending: a choice of how the permanent enters the battlefield (614.12)");
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    let tomb = permanent(&g, "Overgrown Tomb");
    cite!(["119.4"]; assert_eq!((g.players[0].life, g.obj(tomb).tapped), (18, false)));
    assert!(g.log.iter().any(|e| matches!(e, Event::LandPlayed { by: 0, .. })));
    assert!(g.players[0].land_played);
    // a Swamp Forest: it has the mana ability of each (305.6), after the one its text gives
    let mut other = g.clone();
    g.act(0, Move::Activate { source: tomb, ability: 1, targets: vec![], paying: vec![] }).unwrap();
    other.act(0, Move::Activate { source: tomb, ability: 2, targets: vec![], paying: vec![] }).unwrap();
    let (black, green) = (Color::Black as usize, Color::Green as usize);
    let pools = [g.players[0].pool.colors, other.players[0].pool.colors];
    cite!(["305.6"]; assert_eq!(pools.iter().map(|p| p[black]).sum::<u32>(), 1));
    cite!(["305.6"]; assert_eq!(pools.iter().map(|p| p[green]).sum::<u32>(), 1));
    // not paid: tapped from the first, and nothing became tapped
    let mut g = game();
    let tomb = g.put(0, TOMB, Zone::Hand);
    g.act(0, Move::PlayLand(tomb)).unwrap();
    g.act(0, Move::Choose(Answer::No)).unwrap();
    let tomb = permanent(&g, "Overgrown Tomb");
    cite!(["614.1c"]; assert_eq!((g.players[0].life, g.obj(tomb).tapped), (20, true)));
    assert!(!g.log.iter().any(|e| matches!(e, Event::Tapped { .. })));
    // a player with less life than that can't pay it (119.4): there is no choice to make
    let mut g = Game::new(defs(), 2, 1);
    let tomb = g.put(0, TOMB, Zone::Hand);
    g.act(0, Move::PlayLand(tomb)).unwrap();
    cite!(["119.4"]; assert!(g.decision.is_none()));
    assert!(g.obj(permanent(&g, "Overgrown Tomb")).tapped);
}

#[test]
fn r614_12_how_a_permanent_enters_is_asked_as_it_would_enter() {
    // "This land enters tapped unless you control a basic land.": with a Forest it enters
    // untapped
    let mut g = game();
    g.put(0, FOREST, Zone::Battlefield);
    let agna = g.put(0, AGNA, Zone::Hand);
    g.act(0, Move::PlayLand(agna)).unwrap();
    let agna = permanent(&g, "Agna Qel'a");
    cite!(["614.12"]; assert!(!g.obj(agna).tapped));
    g.act(0, Move::Activate { source: agna, ability: 1, targets: vec![], paying: vec![] }).unwrap();
    // "… unless you control two or more basic lands.": one is not enough, and the opponent's
    // are not the player's
    for (mine, tapped) in [(1, true), (2, false)] {
        let mut g = game();
        for _ in 0..mine {
            g.put(0, FOREST, Zone::Battlefield);
        }
        g.put(1, PLAINS, Zone::Battlefield);
        g.put(1, PLAINS, Zone::Battlefield);
        let verdure = g.put(0, VERDURE, Zone::Hand);
        g.act(0, Move::PlayLand(verdure)).unwrap();
        cite!(["614.12"]; case: format!("{mine} basic"); assert_eq!(g.obj(permanent(&g, "Sodden Verdure")).tapped, tapped, "with {mine} basic land(s)"));
    }
}

#[test]
fn r614_12_an_effect_about_permanents_of_a_kind_is_asked_of_the_one_entering_as_it_would_exist() {
    // "Creatures your opponents control enter tapped.": the opponent's creature does, their
    // land does not, and the creatures of the Sovereign's controller do not
    let mut g = game();
    g.put(1, SOVEREIGN, Zone::Battlefield);
    cast(&mut g, BEARS, &[FOREST, FOREST], vec![], "GG");
    cite!(["614.12"]; assert!(g.obj(permanent(&g, "Grizzly Bears")).tapped));
    let swamp = g.put(0, SWAMP, Zone::Hand);
    g.act(0, Move::PlayLand(swamp)).unwrap();
    assert!(!g.obj(permanent(&g, "Swamp")).tapped);
    let mut g = game();
    g.put(0, SOVEREIGN, Zone::Battlefield);
    cast(&mut g, BEARS, &[FOREST, FOREST], vec![], "GG");
    cite!(["614.12"]; assert!(!g.obj(permanent(&g, "Grizzly Bears")).tapped));
    // "Permanents enter tapped." is not about the Orb itself as it enters: such an effect
    // comes from the permanent itself only if it affects only that permanent
    let mut g = game();
    cast(&mut g, ORB, &[FOREST, FOREST, FOREST], vec![], "GGG");
    cite!(["614.12"]; assert!(!g.obj(permanent(&g, "Orb of Dreams")).tapped));
    let swamp = g.put(0, SWAMP, Zone::Hand);
    g.act(0, Move::PlayLand(swamp)).unwrap();
    cite!(["614.1d"]; assert!(g.obj(permanent(&g, "Swamp")).tapped, "what enters after it"));
}

#[test]
fn r701_7a_a_token_enters_as_the_effects_that_modify_how_permanents_enter_have_it() {
    // to create a token is to put it onto the battlefield: "Permanents enter tapped."
    let mut g = game();
    g.put(0, ORB, Zone::Battlefield);
    cast(&mut g, ALARM, &[PLAINS, PLAINS], vec![], "WW");
    let soldiers: Vec<ObjId> = g.battlefield.iter().copied().filter(|o| g.obj(*o).chars.name == "Soldier Token").collect();
    assert_eq!(soldiers.len(), 2);
    cite!(["701.7a"]; assert!(soldiers.iter().all(|s| g.obj(*s).tapped)));
}

#[test]
fn r616_1_every_effect_that_modifies_how_a_permanent_enters_is_applied() {
    // "Each other non-Human creature you control enters with an additional +1/+1 counter on
    // it." and the Watchdog's own three; with "Permanents enter tapped." it is also tapped.
    // The controller explicitly orders the effects (616.1), even when they commute.
    let mut g = game();
    g.put(0, GRUMGULLY, Zone::Battlefield);
    g.put(1, ORB, Zone::Battlefield);
    cast(&mut g, WATCHDOG, &[FOREST, PLAINS], vec![], "GW");
    for count in [3, 2] {
        let Some(Decision::Choose { player, choice: Choice::Replacement { among, .. } }) = g.decision.clone() else {
            panic!("the controller orders three entry effects before any entry");
        };
        cite!(["616.1"]; case: format!("{count} left"); assert_eq!(player, 0, "the entrant's controller orders replacements, not the Orb's"));
        assert_eq!(among.len(), count);
        assert!(!g.battlefield.iter().any(|o| g.obj(*o).card == Some(WATCHDOG)));
        g.act(player, Move::Choose(Answer::Replacement(among[0].clone()))).unwrap();
    }
    let dog = permanent(&g, "Faithful Watchdog");
    cite!(["616.1f"]; assert_eq!((g.power(dog), g.obj(dog).tapped), (Some(4), true)));
    assert!(g.decision.is_none());
    // a Human gets none ("non-Human"), nor does Grumgully itself ("other")
    cast(&mut g, SOVEREIGN, &[PLAINS, PLAINS], vec![], "WW");
    assert_eq!(g.power(permanent(&g, "Imposing Sovereign")), Some(2));
    let mut g = game();
    cast(&mut g, GRUMGULLY, &[MOUNTAIN, FOREST, FOREST], vec![], "RGG");
    assert_eq!(g.power(permanent(&g, "Grumgully, the Generous")), Some(3), "it is not on the battlefield as it enters");
}

#[test]
fn r614_1c_a_permanent_enters_with_its_counters_on_it() {
    // "This creature enters with three +1/+1 counters on it.": a 0/0 that is 3/3 from the
    // first, so no state-based action finds it with toughness 0 (704.5f)
    let mut g = game();
    cast(&mut g, WATCHDOG, &[FOREST, PLAINS], vec![], "GW");
    let dog = permanent(&g, "Faithful Watchdog");
    cite!(["614.1c"]; assert_eq!((g.power(dog), g.toughness(dog)), (Some(3), Some(3))));
    assert_eq!(g.obj(dog).counters.get("+1/+1"), Some(&3));
}

#[test]
fn r122_6_an_object_given_counters_as_it_enters_has_had_counters_put_on_it() {
    let mut g = game();
    cast(&mut g, WATCHDOG, &[FOREST, PLAINS], vec![], "GW");
    let dog = permanent(&g, "Faithful Watchdog");
    cite!(["122.6"]; assert!(g.log.iter().any(|e| matches!(e, Event::CountersPut { on: Target::Object(on), kind, count: 3, .. } if *on == dog && kind == "+1/+1"))));
}

#[test]
fn r122_1b_a_keyword_counter_causes_the_permanent_to_gain_that_keyword() {
    // "Target creature gets +2/+2 until end of turn. Put a flying counter on it.": the
    // counter stays when the turn ends
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    assert!(!g.has(bears, "Flying"));
    cast(&mut g, FLIGHT, &[PLAINS, PLAINS, PLAINS], vec![vec![Target::Object(bears)]], "WWW");
    cite!(["122.1b"]; assert!(g.has(bears, "Flying")));
    assert_eq!(g.power(bears), Some(4));
    while g.turn == 1 {
        match g.decision {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    cite!(["122.1b"]; assert!(g.has(bears, "Flying"), "the counter is still there"));
    cite!(["514.2"]; assert_eq!(g.power(bears), Some(2), "the +2/+2 ended"));
}

/// Player 0 pays {B} for the Skeletons' "Regenerate this creature." and it resolves: a
/// regeneration shield.
fn shield(g: &mut Game, skeletons: ObjId) {
    let swamp = g.put(0, SWAMP, Zone::Battlefield);
    g.act(0, Move::Activate { source: swamp, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    g.act(0, paid("B", Move::Activate { source: skeletons, ability: 0, targets: vec![], paying: vec![] })).unwrap();
    pass_both(g);
}

#[test]
fn r701_19a_regenerate_replaces_the_next_time_the_permanent_would_be_destroyed_this_turn() {
    let mut g = game();
    let skeletons = g.put(0, SKELETONS, Zone::Battlefield);
    shield(&mut g, skeletons);
    assert!(!g.log.iter().any(|e| matches!(e, Event::Regenerated { .. })), "a shield is not a regeneration (701.19c)");
    // "Destroy target creature.": instead, its controller taps it
    cast(&mut g, MURDER, &[SWAMP, SWAMP, SWAMP], vec![vec![Target::Object(skeletons)]], "BBB");
    cite!(["701.19a"]; assert_eq!((g.obj(skeletons).zone, g.obj(skeletons).tapped), (Zone::Battlefield, true)));
    assert!(g.log.contains(&Event::Regenerated { object: skeletons }));
    cite!(["614.6"]; assert!(!g.log.iter().any(|e| matches!(e, Event::Destroyed { .. })), "the destruction never happened (614.6)"));
    // the next time only: the shield has been used
    cast(&mut g, MURDER, &[SWAMP, SWAMP, SWAMP], vec![vec![Target::Object(skeletons)]], "BBB");
    cite!(["701.19a"]; assert_eq!(g.obj(skeletons).zone, Zone::Gone));
    // this turn only: a shield that was not used is gone when the turn has ended
    let mut g = game();
    for player in [0, 1, 0, 1] {
        g.put(player, FOREST, Zone::Library);
    }
    let skeletons = g.put(0, SKELETONS, Zone::Battlefield);
    shield(&mut g, skeletons);
    while !(g.turn == 3 && g.part == Part::PrecombatMain) {
        match g.decision {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    cast(&mut g, MURDER, &[SWAMP, SWAMP, SWAMP], vec![vec![Target::Object(skeletons)]], "BBB");
    cite!(["701.19a"]; assert_eq!(g.obj(skeletons).zone, Zone::Gone));
}

#[test]
fn r701_19a_a_regenerated_creature_has_its_damage_removed_and_is_removed_from_combat() {
    // destroyed by lethal damage (704.5g): the 1/1 blocked by a 3/3
    let mut g = game();
    let skeletons = g.put(0, SKELETONS, Zone::Battlefield);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    while g.decision.is_none() {
        g.act(g.priority, Move::Pass).unwrap();
    }
    g.act(0, Move::Attack(vec![(skeletons, Target::Player(1))])).unwrap();
    pass_both(&mut g);
    g.act(1, Move::Block(vec![(giant, skeletons)])).unwrap();
    shield(&mut g, skeletons);
    pass_both(&mut g);
    assert_eq!(g.part, Part::CombatDamage);
    let o = g.obj(skeletons);
    cite!(["701.19a"]; assert_eq!((o.zone, o.damage, o.tapped), (Zone::Battlefield, 0, true)));
    cite!(["506.4"]; assert!(!g.combat.attacking(skeletons), "removed from combat (506.4)"));
    assert_eq!(g.obj(giant).damage, 1, "it had dealt its damage");
    // and by lethal damage from a spell
    let mut g = game();
    let skeletons = g.put(0, SKELETONS, Zone::Battlefield);
    shield(&mut g, skeletons);
    cast(&mut g, SHOCK, &[MOUNTAIN], vec![vec![Target::Object(skeletons)]], "R");
    let o = g.obj(skeletons);
    cite!(["701.19a"]; assert_eq!((o.zone, o.damage, o.tapped), (Zone::Battlefield, 0, true)));
}

#[test]
fn r701_19c_what_cant_be_regenerated_is_destroyed_whatever_shields_it_has() {
    // "Regenerate target creature." can be cast all the same; "Destroy target nonartifact,
    // nonblack creature. It can't be regenerated." does not apply the shield
    let mut g = game();
    let giant = g.put(0, GIANT, Zone::Battlefield);
    cast(&mut g, WARD, &[PLAINS], vec![vec![Target::Object(giant)]], "W");
    cast(&mut g, TERROR, &[SWAMP, SWAMP], vec![vec![Target::Object(giant)]], "BB");
    cite!(["701.19c"]; assert_eq!(g.obj(giant).zone, Zone::Gone));
    assert!(!g.log.iter().any(|e| matches!(e, Event::Regenerated { .. })));
    // "Destroy all creatures. They can't be regenerated."
    let mut g = game();
    let skeletons = g.put(0, SKELETONS, Zone::Battlefield);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    shield(&mut g, skeletons);
    cast(&mut g, WRATH, &[PLAINS, PLAINS, PLAINS, PLAINS], vec![], "WWWW");
    cite!(["701.19c"]; assert_eq!((g.obj(skeletons).zone, g.obj(giant).zone), (Zone::Gone, Zone::Gone)));
    // with a shield and no such sentence, the same creature stays
    let mut g = game();
    let giant = g.put(0, GIANT, Zone::Battlefield);
    cast(&mut g, WARD, &[PLAINS], vec![vec![Target::Object(giant)]], "W");
    cast(&mut g, MURDER, &[SWAMP, SWAMP, SWAMP], vec![vec![Target::Object(giant)]], "BBB");
    cite!(["701.19a"]; assert_eq!(g.obj(giant).zone, Zone::Battlefield));
}

#[test]
fn r122_1b_a_two_word_keyword_counter_is_not_run_yet() {
    // read by the grammar (the kinds first strike and double strike), not run by the engine
    let reflexes = CardDef::new(
        "Heightened Reflexes",
        "{R}",
        "Instant",
        "Target creature gets +1/+0 until end of turn. Put a first strike counter on it.",
        None,
    )
    .unwrap();
    assert!(check(&reflexes).unwrap_err().0.contains("keyword counter"));
}

#[test]
fn r122_1_a_counter_the_engine_does_not_give_its_rule_is_not_run() {
    // Shield counters still require their damage/destroy replacements (122.1c).
    // Stun counters now replace untapping through the shared untap operation (122.1d).
    let protector = CardDef::new(
        "Swooping Protector",
        "{3}{W}",
        "Creature — Bird Citizen",
        "Flash\nFlying\nThis creature enters with a shield counter on it. (If it would be dealt damage or destroyed, remove a shield counter from it instead.)",
        Some((2, 1)),
    )
    .unwrap();
    assert!(check(&protector).unwrap_err().0.contains("shield"));
    let patrol = CardDef::new(
        "Shipwreck Patrol",
        "{3}{U}",
        "Creature — Merfolk Warrior",
        "When this creature enters, tap target creature an opponent controls and put a stun counter on it. (If a permanent with a stun counter would become untapped, remove one from it instead.)",
        Some((3, 3)),
    )
    .unwrap();
    check(&patrol).unwrap();
}
