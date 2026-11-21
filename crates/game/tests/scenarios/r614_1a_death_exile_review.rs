//! Checks of event substitution, replacement ownership and no-event cases.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use game::state::{Answer, Choice, Decision};
use ir::Part;

const MARK: usize = 0;
const CHOSEN: usize = 1;
const SWEEP: usize = 2;
const KILL: usize = 3;
const BEAR: usize = 4;
const WATCH: usize = 5;
const TAP_MARK: usize = 6;
const WALKER: usize = 7;
const BOLT: usize = 8;
const TOUGH: usize = 9;
const LEADER: usize = 10;
const STEAL: usize = 11;

fn game(players: usize) -> Game {
    let c = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).unwrap();
    let defs = vec![
        c("Mark", "Sorcery", "Mark deals 1 damage to target creature. If that creature would die this turn, exile it instead.", None),
        c("Chosen", "Creature — Elf", "If this creature would die, put it on the bottom of its owner's library instead.", Some((2, 3))),
        c("Sweep", "Sorcery", "Destroy all creatures.", None),
        c("Kill", "Sorcery", "Destroy target creature.", None),
        c("Bear", "Creature — Bear", "", Some((2, 2))),
        c("Watcher", "Enchantment", "Whenever a creature is exiled, you gain 1 life.", None),
        c("Tap mark", "Sorcery", "Tap target permanent. If that permanent would die this turn, exile it instead.", None),
        CardDef::planeswalker("Walker", "{0}", "Legendary Planeswalker — Jace", "+1: You gain 1 life.", 3).unwrap(),
        c("Bolt", "Sorcery", "Bolt deals 3 damage to any target.", None),
        c("Tough", "Creature — Bear", "Indestructible", Some((4, 4))),
        c("Leader", "Legendary Creature — Elf", "If this creature would die, put it on the bottom of its owner's library instead.", Some((2, 3))),
        c("Steal", "Sorcery", "Gain control of target creature until end of turn.", None),
        c("Island", "Basic Land — Island", "", None),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, players, 20);
    for p in 0..players { for _ in 0..5 { g.put(p, 12, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn resolve(g: &mut Game) {
    for _ in 0..40 {
        if g.stack.is_empty() || g.decision.is_some() { return; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("stack did not settle");
}
fn cast(g: &mut Game, def: usize, target: Option<ObjId>) {
    let card = g.put(0, def, Zone::Hand);
    g.act(0, Move::Cast { card, targets: target.map_or(vec![], |o| vec![vec![Target::Object(o)]]) }).unwrap();
    resolve(g);
}
fn in_zone(g: &Game, def: usize, zone: Zone) -> bool {
    g.exile.iter().chain(&g.command).chain(g.players.iter().flat_map(|p| p.library.iter().chain(&p.graveyard)))
        .any(|o| g.obj(*o).card == Some(def) && g.obj(*o).zone == zone)
}

#[test]
fn r616_1_simultaneous_death_replacement_choices_follow_apnap() {
    // Rules: 616.1, 101.4
    let mut g = game(3);
    let p2 = g.put(2, CHOSEN, Zone::Battlefield);
    let p1 = g.put(1, CHOSEN, Zone::Battlefield);
    let p0 = g.put(0, CHOSEN, Zone::Battlefield);
    for o in [p2, p1, p0] { cast(&mut g, MARK, Some(o)); }
    cast(&mut g, SWEEP, None);
    for (player, expected) in [(0, p0), (1, p1), (2, p2)] {
        let Some(Decision::Choose { player: chooser, choice: Choice::Replacement { on: Target::Object(on), among, .. } }) = g.decision.clone() else { panic!("{:?}", g.decision) };
        cite!(["616.1", "101.4"]; case: format!("player {player}"); assert_eq!((chooser, on), (player, expected), "allocation order cannot choose the first player"));
        let exile = among.into_iter().find(|r| r.source != on).unwrap();
        g.act(player, Move::Choose(Answer::Replacement(exile))).unwrap();
    }
    assert_eq!(g.exile.iter().filter(|o| g.obj(**o).card == Some(CHOSEN)).count(), 3);
}

#[test]
fn r614_6_a_death_replaced_by_exile_is_an_exile_event() {
    // Rules: 614.6, 701.13a, 603.2
    let mut g = game(2);
    g.put(0, WATCH, Zone::Battlefield);
    let bear = g.put(1, BEAR, Zone::Battlefield);
    cast(&mut g, MARK, Some(bear));
    cast(&mut g, KILL, Some(bear));
    resolve(&mut g);
    cite!(["614.1a"]; assert!(in_zone(&g, BEAR, Zone::Exile)));
    cite!(["614.6"]; assert_eq!(g.players[0].life, 21, "the replacement event can trigger an exile watcher"));
}

#[test]
fn r614_7_prohibited_destruction_has_no_replacement_question() {
    // Rules: 614.7, 702.12b
    let mut g = game(2);
    let tough = g.put(1, TOUGH, Zone::Battlefield);
    cast(&mut g, MARK, Some(tough));
    cast(&mut g, KILL, Some(tough));
    cite!(["614.7"]; assert!(g.decision.is_none()));
    cite!(["702.12b"]; assert!(g.battlefield.contains(&tough)));
    assert!(!in_zone(&g, TOUGH, Zone::Exile));
}

#[test]
fn r704_5i_the_zero_loyalty_departure_is_replaced() {
    // Rules: 704.5i, 614.1a, 700.4
    let mut g = game(2);
    let walker = g.put(1, WALKER, Zone::Battlefield);
    g.put_counters(walker, "loyalty", 3);
    cast(&mut g, TAP_MARK, Some(walker));
    cast(&mut g, BOLT, Some(walker));
    cite!(["704.5i", "614.1a"]; assert!(in_zone(&g, WALKER, Zone::Exile)));
    cite!(["614.6"]; assert!(!in_zone(&g, WALKER, Zone::Graveyard)));
}

#[test]
fn r903_9a_exile_replacement_precedes_the_owners_commander_sba_choice() {
    // Rules: 616.1, 903.9a-b
    for return_it in [false, true] {
        let mut g = game(2);
        let leader = g.put(1, LEADER, Zone::Command);
        g.designate_commander(leader).unwrap();
        g.move_object(leader, Zone::Battlefield);
        let leader = *g.battlefield.iter().find(|o| g.is_commander(**o)).unwrap();
        cast(&mut g, STEAL, Some(leader));
        assert_eq!(g.controller(leader), 0);
        cast(&mut g, MARK, Some(leader));
        cast(&mut g, KILL, Some(leader));
        let Some(Decision::Choose { player: 0, choice: Choice::Replacement { among, .. } }) = g.decision.clone() else { panic!("{:?}", g.decision) };
        let exile = among.into_iter().find(|r| r.source != leader).unwrap();
        g.act(0, Move::Choose(Answer::Replacement(exile))).unwrap();
        let Some(Decision::Choose { player: 1, choice: Choice::CommandZone { object } }) = g.decision else { panic!("{:?}", g.decision) };
        cite!(["903.9a"]; assert_eq!(g.obj(object).zone, Zone::Exile, "exile happens before the SBA choice"));
        cite!(["614.6"]; assert!(!g.log.iter().any(|e| matches!(e, Event::ZoneChanged { was, to: Zone::Graveyard, .. } if *was == leader))));
        g.act(1, Move::Choose(if return_it { Answer::Yes } else { Answer::No })).unwrap();
        cite!(["903.9a"]; case: format!("return: {return_it}"); assert!(in_zone(&g, LEADER, if return_it { Zone::Command } else { Zone::Exile })));
    }
}

#[test]
fn r614_6_impossible_replacement_exile_keeps_independent_life_gain() {
    let c = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).unwrap();
    let defs = vec![
        c("Crystal", "Artifact", "If a nontoken creature an opponent controls would die, instead exile it and you gain 2 life.", None),
        c("Ban", "Enchantment", "Creatures can't be exiled.", None),
        c("Bear", "Creature — Bear", "", Some((2, 2))),
        c("Kill", "Instant", "Destroy target creature.", None),
        c("Watcher", "Enchantment", "Whenever a creature is exiled, you gain 1 life.", None),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    g.put(0, 0, Zone::Battlefield); g.put(0, 1, Zone::Battlefield); g.put(0, 4, Zone::Battlefield);
    let bear = g.put(1, 2, Zone::Battlefield);
    cast(&mut g, 3, Some(bear));
    cite!(["614.6", "609.3"]; assert!(g.battlefield.contains(&bear)));
    cite!(["614.6"]; assert_eq!(g.players[0].life, 22));
    cite!(["614.6", "603.2"]; assert!(!g.log.iter().any(|e| matches!(e, Event::Exiled { .. }))));
    cite!(["614.6"]; assert!(!g.log.iter().any(|e| matches!(e, Event::ZoneChanged { to: Zone::Exile, .. }))));
    cite!(["603.2"]; assert!(g.stack.is_empty()));
    cite!(["614.6"]; assert!(g.exile.is_empty()));
    g.exile(bear);
    cite!(["101.2", "609.3"]; assert!(g.battlefield.contains(&bear)));
    cite!(["701.13a"]; assert!(g.exile.is_empty()));
    cite!(["603.2"]; assert_eq!(g.players[0].life, 22));
}

#[test]
fn active_exile_prohibition_remains_explicitly_unsupported() {
    let d = CardDef::new("Active ban", "{0}", "Enchantment", "Players can't exile cards.", None).unwrap();
    // Uncited (admission): an active actor exile ban is outside the passive-object prohibition.
    crate::support::refused(game::play::check(&d).map_err(|e|e.0),"prohibition: Exile");
}

#[test]
fn r603_12_impossible_replacement_exile_does_not_trigger_when_you_do() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let hunt = CardDef::from_face(faces.iter().find(|f| f.name == "Head of the Hunt").unwrap()).unwrap();
    let c = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).unwrap();
    let defs = vec![hunt, c("Ban", "Enchantment", "Creatures can't be exiled.", None),
        c("Bear", "Creature — Bear", "", Some((2, 2))),
        c("Kill", "Instant", "Destroy target creature.", None)];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    g.put(0, 0, Zone::Battlefield); g.put(0, 1, Zone::Battlefield);
    let bear = g.put(1, 2, Zone::Battlefield); cast(&mut g, 3, Some(bear));
    cite!(["614.6", "609.3"]; assert!(g.battlefield.contains(&bear)));
    cite!(["603.12"]; assert!(g.stack.is_empty()));
    cite!(["603.12"]; assert!(!g.battlefield.iter().any(|o| g.obj(*o).token())));
}
