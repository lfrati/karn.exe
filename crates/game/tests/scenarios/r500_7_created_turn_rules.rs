//! Created-turn rules are attached to occurrences, not next-turn player anchors (500.7).
use crate::support::{cite, oracle, paid};
use game::state::Decision;
use game::turn::Occurrence;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
fn card(name: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    let d = CardDef::new(name, "{0}", types, text, pt).unwrap();
    game::play::check(&d).unwrap();
    d
}
fn setup(n: usize) -> Game {
    let ds = vec![
        oracle("Kang the Conqueror"),
        card("Battery", "Creature", "Power-up — {0}: You gain 1 life.", Some((2, 2))),
        card("Extra", "Instant", "Target player takes an extra turn after this one.", None),
        card("Skip", "Instant", "Target player skips their next turn.", None),
        card("Double Skip", "Instant", "Target player skips their next two turns.", None),
        card("Stone", "Artifact", "", None),
        card("Kill", "Instant", "Destroy target creature.", None),
        card(
            "Control",
            "Instant",
            "You gain control of target opponent during that player's next turn. After that turn, that player takes an extra turn.",
            None,
        ),
        card(
            "Scoped",
            "Instant",
            "Take an extra turn after this one. During that turn, power-up abilities can't be activated.",
            None,
        ),
        card("Steal", "Instant", "You gain control of target creature.", None),
    ];
    let mut g = Game::new(ds, n, 20);
    for p in 0..n {
        for _ in 0..30 {
            g.put(p, 5, Zone::Library);
        }
    }
    g
}
fn passes(g: &mut Game) {
    for _ in 0..g.players.iter().filter(|p| !p.lost).count() {
        g.act(g.decision_maker(g.priority), Move::Pass).unwrap();
    }
}
fn cast(g: &mut Game, p: usize, d: usize, targets: Vec<Vec<Target>>) {
    while g.priority != p {
        g.act(g.decision_maker(g.priority), Move::Pass).unwrap();
    }
    let c = g.put(p, d, Zone::Hand);
    g.act(g.decision_maker(p), Move::Cast { card: c, targets }).unwrap();
    passes(g);
}
fn next(g: &mut Game) {
    let old = g.turn;
    for _ in 0..100 {
        if g.turn != old {
            return;
        }
        let action = match &g.decision {
            Some(Decision::Attackers { .. }) => Move::Attack(vec![]),
            Some(Decision::Discard { player, count }) => Move::Discard(g.players[*player].hand[..*count].to_vec()),
            None => Move::Pass,
            x => panic!("unexpected {x:?}"),
        };
        g.act(g.decision_maker(g.decision_actor()), action).unwrap();
    }
    panic!("no turn began");
}
fn try_power(g: &mut Game, p: usize, s: ObjId) -> Result<(), String> {
    while g.priority != p {
        g.act(g.decision_maker(g.priority), Move::Pass).unwrap();
    }
    g.act(g.decision_maker(p), Move::Activate { source: s, ability: 0, targets: vec![], paying: vec![] })
}
#[test]
fn r500_7_native_kang_rule_applies_only_to_its_created_turn_for_both_players() {
    let mut g = setup(2);
    let kang = g.put(0, 0, Zone::Battlefield);
    let a = g.put(0, 1, Zone::Battlefield);
    let b = g.put(1, 1, Zone::Battlefield);
    let k = (0..g.abilities(kang).len())
        .find(|i| {
            g.ability_text(Some(kang), game::binding::AbilityKey::printed(*i))
                .is_some_and(|s| s.starts_with("Power-up —"))
        })
        .unwrap();
    g.players[0].pool.colors[1] = 3;
    g.players[0].pool.colorless = 5;
    g.act(0, paid("CCCCCUUU",Move::Activate { source: kang, ability: k, targets: vec![], paying: vec![] })).unwrap();
    passes(&mut g);
    cast(&mut g, 0, 6, vec![vec![Target::Object(kang)]]);
    cite!(["611.2a"];assert_eq!(g.obj(kang).zone,Zone::Gone));
    cast(&mut g, 0, 2, vec![vec![Target::Player(0)]]);
    next(&mut g);
    cite!(["500.7"];assert_eq!(g.active,0));
    cite!(["500.7","602.5"];assert!(try_power(&mut g,0,a).is_ok()));
    passes(&mut g);
    let a2 = g.put(0, 1, Zone::Battlefield);
    next(&mut g);
    cite!(["500.7","602.5"];refused("prohibits activating"):try_power(&mut g,0,a2));
    cite!(["602.5"];refused("prohibits activating"):try_power(&mut g,1,b));
    next(&mut g);
    cite!(["611.2a","602.5"]; assert_eq!(g.active,1));
    cite!(["611.2a","602.5"]; assert!(try_power(&mut g,1,b).is_ok()));
}
#[test]
fn r611_2a_resolved_created_turn_rule_survives_source_departure_and_game_clone() {
    let mut g = setup(2);
    cast(&mut g, 0, 8, vec![]);
    let source = g.put(0, 1, Zone::Battlefield);
    // The resolving spell is now gone; the rule is owned by its created turn, not that spell.
    cite!(["611.2a"];assert!(g.players[0].graveyard.iter().any(|c|g.obj(*c).chars.name=="Scoped")));
    for mut fork in [g.clone(), g] {
        next(&mut fork);
        cite!(["611.2a","602.5"];refused("prohibits activating"):try_power(&mut fork,0,source));
    }
}
#[test]
fn r614_10_skipping_the_designated_extra_retires_only_its_rule() {
    let mut g = setup(2);
    cast(&mut g, 0, 8, vec![]);
    cast(&mut g, 0, 3, vec![vec![Target::Player(0)]]);
    let source = g.put(0, 1, Zone::Battlefield);
    let start = g.log.len();
    next(&mut g);
    cite!(["500.11","614.10"]; assert_eq!(g.active,1));
    cite!(["500.11","614.10"]; assert!(g.log[start..].iter().any(|e|matches!(e,Event::Skipped{player:0,what:Occurrence::Turn,..}))));
    cite!(["614.10a"];assert!(!g.log[start..].iter().any(|e|matches!(e,Event::TurnBegan{player:0}))));
    next(&mut g);
    cite!(["614.10a","602.5"]; assert_eq!(g.active,0));
    cite!(["614.10a","602.5"]; assert!(try_power(&mut g,0,source).is_ok()));
}
#[test]
fn r614_10a_two_next_turn_skips_consume_separately_before_beginning_actions() {
    for counted in [false, true] {
        let mut g = setup(2);
        if counted {
            cast(&mut g, 0, 4, vec![vec![Target::Player(1)]]);
        } else {
            cast(&mut g, 0, 3, vec![vec![Target::Player(1)]]);
            cast(&mut g, 0, 3, vec![vec![Target::Player(1)]]);
        }
        cast(&mut g, 0, 2, vec![vec![Target::Player(1)]]);
        let stone = g.put(1, 5, Zone::Battlefield);
        g.tap(stone);
        let old = g.players[1].library.len();
        let start = g.log.len();
        next(&mut g);
        cite!(["500.7","614.10a"]; assert_eq!(g.active,0));
        cite!(["500.7","614.10a"]; assert_eq!(g.log[start..].iter().filter(|e|matches!(e,Event::Skipped{player:1,what:Occurrence::Turn,..})).count(),2));
        cite!(["614.10a"]; assert!(g.skips.is_empty()));
        cite!(["614.10a"]; assert_eq!(g.players[1].library.len(),old));
        cite!(["614.10a"]; assert!(g.obj(stone).tapped));
        cite!(["614.10a"]; assert_eq!(g.players[1].turns_taken,0));
        cite!(["500.11","614.10a"];assert!(!g.log[start..].iter().any(|e|matches!(e,Event::TurnBegan{player:1}))));
        next(&mut g);
        cite!(["614.10a","502.2"]; assert_eq!(g.active,1));
        cite!(["614.10a","502.2"]; assert!(!g.obj(stone).tapped));
        cite!(["614.10a","502.2"]; assert_eq!(g.players[1].library.len(),old));
        passes(&mut g);
        cite!(["504.1"]; assert_eq!(g.part,ir::Part::Draw));
        cite!(["504.1"]; assert_eq!(g.players[1].library.len(),old-1));
    }
}
#[test]
fn r614_10a_control_next_turn_anchor_waits_through_whole_turn_skip() {
    let mut g = setup(3);
    cast(&mut g, 0, 7, vec![vec![Target::Player(1)]]);
    cast(&mut g, 0, 3, vec![vec![Target::Player(1)]]);
    next(&mut g);
    cite!(["614.10a","723.1"];assert_eq!((g.active,g.decision_maker(1)),(2,1)));
    next(&mut g);
    next(&mut g);
    cite!(["614.10a","723.1"];assert_eq!((g.active,g.decision_maker(1)),(1,0)));
    next(&mut g);
    cite!(["723.1","500.7"];assert_eq!((g.active,g.decision_maker(1)),(1,1)));
}
#[test]
fn r614_10b_skip_followup_and_ambiguous_created_turn_reference_remain_refused() {
    for text in [
        "Skip your next turn. Draw a card.",
        "Take two extra turns after this one. During that turn, power-up abilities can't be activated.",
        "Each player takes an extra turn after this one. During that turn, power-up abilities can't be activated.",
        "Take an extra turn after this one. During that turn, copy target instant spell.",
        "Take an extra turn after this one. At the beginning of the next end step, during that turn, power-up abilities can't be activated.",
    ] {
        let d = CardDef::new("Unsupported", "{0}", "Instant", text, None).unwrap();
        let reason=if text.starts_with("Skip") {"whole-turn skip followed by another action (614.10b)"}
            else if text.starts_with("Take two") || text.starts_with("Each") {"during that turn: one preceding created turn is required"}
            else if text.contains("copy target") {"during a created turn: only a retained activation-family rule is implemented"}
            else {"during that turn: one preceding created turn is required"};
        crate::support::refused(game::play::check(&d).map_err(|e|e.0),reason);
    }
}

#[test]
fn r500_7_native_kang_control_change_during_resolution_does_not_change_turn_recipient() {
    let mut g = setup(2);
    let kang = g.put(0, 0, Zone::Battlefield);
    let a = g.put(0, 1, Zone::Battlefield);
    let b = g.put(1, 1, Zone::Battlefield);
    let k = (0..g.abilities(kang).len())
        .find(|i| {
            g.ability_text(Some(kang), game::binding::AbilityKey::printed(*i))
                .is_some_and(|s| s.starts_with("Power-up —"))
        })
        .unwrap();
    g.players[0].pool.colors[1] = 3;
    g.players[0].pool.colorless = 5;
    g.act(0, paid("CCCCCUUU",Move::Activate { source: kang, ability: k, targets: vec![], paying: vec![] })).unwrap();
    cast(&mut g, 1, 9, vec![vec![Target::Object(kang)]]);
    cite!(["113.7a","611.2a"];assert_eq!(g.controller(kang),1));
    passes(&mut g);
    next(&mut g);
    cite!(["113.7a","500.7"];assert_eq!(g.active,0));
    cite!(["602.5"];refused("prohibits activating"):try_power(&mut g,0,a));
    cite!(["602.5"];refused("prohibits activating"):try_power(&mut g,1,b));
}
#[test]
fn r800_4_eliminated_created_turn_owner_does_not_leak_its_rule() {
    let mut g = setup(3);
    cast(&mut g, 0, 8, vec![]);
    let source = g.put(1, 1, Zone::Battlefield);
    g.players[0].life = 0;
    while g.state_based_actions() {}
    cite!(["704.5a","800.4a"];assert!(g.players[0].lost));
    next(&mut g);
    cite!(["800.4j","602.5"]; assert_eq!(g.active,1));
    cite!(["800.4j","602.5"]; assert!(try_power(&mut g,1,source).is_ok()));
}
#[test]
fn r500_7_declined_optional_creation_leaves_empty_occurrence_not_an_older_turn() {
    for prior in [false, true] {
        let mut g = setup(2);
        if prior {
            cast(&mut g, 0, 2, vec![vec![Target::Player(0)]]);
        }
        let d = card(
            "Optional",
            "Instant",
            "You may take an extra turn after this one. During that turn, power-up abilities can't be activated.",
            None,
        );
        let index = g.defs.len();
        std::sync::Arc::make_mut(&mut g.defs).push(d);
        let spell = g.put(0, index, Zone::Hand);
        g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
        passes(&mut g);
        g.act(0, Move::Choose(game::state::Answer::No)).unwrap();
        cite!(["608.2d","500.7"];assert!(g.stack.is_empty()));
        next(&mut g);
        cite!(["500.7"];assert_eq!(g.active,if prior {0}else{1}));
        let active = g.active;
        let source = g.put(active, 1, Zone::Battlefield);
        cite!(["608.2d","602.5"];ok:try_power(&mut g,active,source));
    }
}
