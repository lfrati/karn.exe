//! Existing targets keep announced positions and original spell/ability context (115.7).
use crate::support::{cite, paid};
use game::announcement::CastCost;
use game::state::{Answer, Choice, Decision, TargetPolicy};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
const CHANGERS: &[&str] = &[
    "Ricochet Trap",
    "Untimely Malfunction",
    "Imp's Mischief",
    "Reroute",
    "Bolt Bend",
    "Misdirection",
    "Swerve",
    "Shunt",
    "Deflection",
    "Redirect",
    "Insidious Will",
    "Wild Ricochet",
    "Deflecting Swat",
];
fn setup(extra: &[&str]) -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let names = CHANGERS.iter().copied().chain(extra.iter().copied()).collect::<Vec<_>>();
    let defs = names
        .iter()
        .map(|name| {
            let d = CardDef::from_faces(&catalog[*name]).unwrap();
            game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
            d
        })
        .collect::<Vec<_>>();
    Game::new(defs, 2, 20)
}
fn put(g: &mut Game, p: usize, name: &str, z: Zone) -> ObjId {
    let d = g.defs.iter().position(|d| d.name() == name).unwrap();
    g.put(p, d, z)
}
fn mana(g: &mut Game, p: usize) {
    g.players[p].pool.colors = [20; 5];
}
fn cast(g: &mut Game, p: usize, name: &str, targets: Vec<Vec<Target>>, modes: &[usize], spending: &str) -> ObjId {
    let card = put(g, p, name, Zone::Hand);
    mana(g, p);
    let mv = Move::Cast { card, targets };
    let mv = if modes.is_empty() { mv } else { Move::Modes(modes.to_vec(), Box::new(mv)) };
    g.act(p, paid(spending, mv)).unwrap_or_else(|e| panic!("{name}: {e}"));
    *g.stack.last().unwrap()
}
fn pass(g: &mut Game) {
    for _ in 0..2 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn to_other(g: &mut Game) {
    g.act(g.priority, Move::Pass).unwrap();
}
fn change(g: &mut Game, targets: Vec<Vec<Target>>) {
    let Some(Decision::Choose { player, choice: Choice::ChangeTargets { .. } }) = g.decision.clone() else {
        panic!("{:?}", g.decision)
    };
    g.act(player, Move::Choose(Answer::Targets(targets))).unwrap();
}
fn pending(g: &Game) -> (ObjId, Vec<Vec<Target>>, TargetPolicy) {
    match &g.decision {
        Some(Decision::Choose { choice: Choice::ChangeTargets { object, original, policy, .. }, .. }) => {
            (*object, original.clone(), *policy)
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn every_simple_required_changer_executes_its_full_body_at_normal_cost() {
    for (name, spending) in [
        ("Ricochet Trap", "WWWR"),
        ("Untimely Malfunction", "WR"),
        ("Imp's Mischief", "WB"),
        ("Bolt Bend", "WWWR"),
        ("Misdirection", "WWWUU"),
        ("Swerve", "UR"),
        ("Shunt", "WRR"),
        ("Deflection", "WWWU"),
    ] {
        let mut g = setup(&["Lightning Bolt", "Island"]);
        for _ in 0..4 {
            put(&mut g, 1, "Island", Zone::Library);
        }
        let bolt = cast(&mut g, 0, "Lightning Bolt", vec![vec![Target::Player(1)]], &[], "R");
        to_other(&mut g);
        let changer = cast(
            &mut g,
            1,
            name,
            vec![vec![Target::Object(bolt)]],
            if name == "Untimely Malfunction" { &[1] } else { &[] },
        spending);
        pass(&mut g);
        let paid = match name {
            "Ricochet Trap" | "Bolt Bend" | "Deflection" => 4,
            "Untimely Malfunction" | "Imp's Mischief" | "Swerve" => 2,
            "Misdirection" => 5,
            "Shunt" => 3,
            _ => unreachable!(),
        };
        assert_eq!(g.players[1].pool.total(), 100 - paid);
        assert_eq!(pending(&g), (bolt, vec![vec![Target::Player(1)]], TargetPolicy::RequiredSingle));
        // declined, the same target, the spell itself
        for (answer, why) in [
            (Answer::No, "choose targets according to the offered target-change policy (115.7)"),
            (Answer::Targets(vec![vec![Target::Player(1)]]), "change the single target to an offered different target (115.7a)"),
            (Answer::Targets(vec![vec![Target::Object(bolt)]]), "choose an offered legal alternative (115.7a)"),
        ] {
            let before = g.obj(bolt).targets.clone();
            cite!(["115.7a"]; case: format!("{name} {answer:?}"); refused(why): g.act(1, Move::Choose(answer)));
            assert_eq!(g.obj(bolt).targets, before);
        }
        let mut clone = g.clone();
        for branch in [&mut g, &mut clone] {
            change(branch, vec![vec![Target::Player(0)]]);
            assert!(!branch.stack.contains(&changer));
            cite!(["115.7a"]; case: name; assert_eq!(branch.obj(bolt).targets, [vec![Target::Player(0)]]));
            assert_eq!(branch.players[1].life, if name == "Imp's Mischief" { 19 } else { 20 });
            pass(branch);
            assert_eq!(branch.players[0].life, 17);
        }
    }
}

#[test]
fn counterspell_can_target_resolving_trap_but_not_itself() {
    let mut g = setup(&["Lightning Bolt", "Counterspell"]);
    let bolt = cast(&mut g, 0, "Lightning Bolt", vec![vec![Target::Player(1)]], &[], "R");
    to_other(&mut g);
    let counter = cast(&mut g, 1, "Counterspell", vec![vec![Target::Object(bolt)]], &[], "UU");
    to_other(&mut g);
    let trap = cast(&mut g, 0, "Ricochet Trap", vec![vec![Target::Object(counter)]], &[], "WWWR");
    pass(&mut g);
    let Some(Decision::Choose { choice: Choice::ChangeTargets { slots, .. }, .. }) = &g.decision else { panic!() };
    cite!(["115.5"]; assert!(!slots[0].among.contains(&Target::Object(counter))));
    change(&mut g, vec![vec![Target::Object(trap)]]);
    pass(&mut g);
    cite!(["608.2b"]; assert!(g.log.iter().any(|e| matches!(e,Event::Fizzled{spell} if *spell==counter))));
    pass(&mut g);
    assert_eq!(g.players[1].life, 17);
}

#[test]
fn optional_final_swap_keeps_positions_and_does_not_retrigger_unchanged_subjects() {
    let mut g = setup(&["Arc Trail", "Craw Wurm"]);
    let a = put(&mut g, 0, "Craw Wurm", Zone::Battlefield);
    let b = put(&mut g, 1, "Craw Wurm", Zone::Battlefield);
    let arc = cast(&mut g, 0, "Arc Trail", vec![vec![Target::Object(a)], vec![Target::Object(b)]], &[], "WR");
    to_other(&mut g);
    cast(&mut g, 1, "Redirect", vec![vec![Target::Object(arc)]], &[], "UU");
    pass(&mut g);
    cite!(["115.7d"]; assert_eq!(pending(&g).2, TargetPolicy::ChooseNew));
    cite!(["115.7d"]; refused("new targets must preserve distinct target groups (115.7d)"): g.act(1, Move::Choose(Answer::Targets(vec![vec![Target::Object(a)], vec![Target::Object(a)]]))));
    let from = g.log.len();
    change(&mut g, vec![vec![Target::Object(b)], vec![Target::Object(a)]]);
    // only the final set of targets is evaluated (115.7e), and both were already targets of Arc Trail, so neither becomes one (603.2e: not "if that state already exists"); the rulings on a target changed to a new object (Angelic Cub, valiant) don't cover this
    cite!(["115.7e", "603.2e"]; assert!(!g.log.iter().skip(from).any(|e| matches!(e,Event::BecameTarget{of,..} if *of==arc))));
    pass(&mut g);
    cite!(["115.7d"]; assert_eq!((g.obj(a).damage, g.obj(b).damage), (1, 2)));
}

#[test]
fn target_count_counts_occurrences_and_aura_uses_enchant_spec() {
    let mut g = setup(&["Seeds of Strength", "Grizzly Bears", "Pacifism"]);
    let bear = put(&mut g, 0, "Grizzly Bears", Zone::Battlefield);
    let seeds = cast(&mut g, 0, "Seeds of Strength", vec![vec![Target::Object(bear)]; 3], &[], "GW");
    to_other(&mut g);
    let trap = put(&mut g, 1, "Ricochet Trap", Zone::Hand);
    mana(&mut g, 1);
    cite!(["115.1"]; refused("an illegal target, Seeds of Strength: it doesn't match the target's description (115.1)"): g.act(1, Move::Cast { card: trap, targets: vec![vec![Target::Object(seeds)]] }));
    let mut g = setup(&["Pacifism", "Grizzly Bears"]);
    let a = put(&mut g, 0, "Grizzly Bears", Zone::Battlefield);
    let b = put(&mut g, 1, "Grizzly Bears", Zone::Battlefield);
    let aura = cast(&mut g, 0, "Pacifism", vec![vec![Target::Object(a)]], &[], "WW");
    to_other(&mut g);
    cast(&mut g, 1, "Swerve", vec![vec![Target::Object(aura)]], &[], "UR");
    pass(&mut g);
    change(&mut g, vec![vec![Target::Object(b)]]);
    pass(&mut g);
    let aura = *g.battlefield.iter().find(|o| g.def(**o).is_some_and(|d| d.name() == "Pacifism")).unwrap();
    cite!(["608.3c"]; assert_eq!(g.obj(aura).attached_to, Some(Target::Object(b))));
}

#[test]
fn optional_changer_modes_and_wild_copy_keep_separate_questions() {
    for (name, modes, spending) in
        [("Redirect", vec![], "UU"), ("Insidious Will", vec![1], "WWUU"), ("Deflecting Swat", vec![], "WWR"), ("Wild Ricochet", vec![], "WWRR")]
    {
        let mut g = setup(&["Lightning Bolt"]);
        let bolt = cast(&mut g, 0, "Lightning Bolt", vec![vec![Target::Player(1)]], &[], "R");
        to_other(&mut g);
        cast(&mut g, 1, name, vec![vec![Target::Object(bolt)]], &modes, spending);
        pass(&mut g);
        change(&mut g, vec![vec![Target::Player(0)]]);
        if name == "Wild Ricochet" {
            cite!(["707.10c"]; assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::Retarget { .. }, .. }))));
            g.act(1, Move::Choose(Answer::Targets(vec![vec![Target::Player(1)]]))).unwrap();
            pass(&mut g);
            assert_eq!(g.players[1].life, 17);
        }
        pass(&mut g);
        cite!(["115.7d"]; case: name; assert_eq!(g.players[0].life, 17));
    }
}

#[test]
fn activated_ability_retarget_keeps_lki_and_reroute_draws() {
    let mut g = setup(&["Prodigal Pyromancer", "Island"]);
    let source = put(&mut g, 0, "Prodigal Pyromancer", Zone::Battlefield);
    g.turn = 2;
    for _ in 0..4 {
        put(&mut g, 1, "Island", Zone::Library);
    }
    g.act(0, Move::Activate { source, ability: 0, targets: vec![vec![Target::Player(1)]], paying: vec![] }).unwrap();
    let ability = *g.stack.last().unwrap();
    to_other(&mut g);
    cast(&mut g, 1, "Reroute", vec![vec![Target::Object(ability)]], &[], "WR");
    pass(&mut g);
    change(&mut g, vec![vec![Target::Player(0)]]);
    assert_eq!(g.players[1].hand.len(), 1);
    pass(&mut g);
    cite!(["115.7a"]; assert_eq!(g.players[0].life, 19));
    let mut g = setup(&["Mogg Fanatic", "Paladin en-Vec"]);
    let source = put(&mut g, 0, "Mogg Fanatic", Zone::Battlefield);
    let paladin = put(&mut g, 0, "Paladin en-Vec", Zone::Battlefield);
    g.act(0, Move::Activate { source, ability: 0, targets: vec![vec![Target::Player(1)]], paying: vec![] }).unwrap();
    let ability = *g.stack.last().unwrap();
    assert_eq!(g.obj(source).zone, Zone::Gone);
    to_other(&mut g);
    cast(&mut g, 1, "Untimely Malfunction", vec![vec![Target::Object(ability)]], &[1], "WR");
    pass(&mut g);
    let Some(Decision::Choose { choice: Choice::ChangeTargets { slots, .. }, .. }) = &g.decision else { panic!() };
    cite!(["113.7a", "702.16b"]; assert!(!slots[0].among.contains(&Target::Object(paladin)), "departed red source still determines protection"));
    change(&mut g, vec![vec![Target::Player(0)]]);
    pass(&mut g);
    assert_eq!(g.players[0].life, 19);
}

#[test]
fn changed_target_ward_uses_the_original_controller() {
    for holder in [0, 1] {
        let mut g = setup(&["Lightning Bolt", "Tolarian Terror"]);
        let terror = put(&mut g, holder, "Tolarian Terror", Zone::Battlefield);
        to_other(&mut g);
        let bolt = cast(&mut g, 1, "Lightning Bolt", vec![vec![Target::Player(0)]], &[], "R");
        to_other(&mut g);
        cast(&mut g, 0, "Shunt", vec![vec![Target::Object(bolt)]], &[], "WRR");
        pass(&mut g);
        change(&mut g, vec![vec![Target::Object(terror)]]);
        assert!(
            g.log
                .iter()
                .any(|e| matches!(e,Event::BecameTarget{object:Target::Object(t),of,..} if *t==terror && *of==bolt))
        );
        cite!(["702.21a"]; case: format!("holder {holder}"); assert_eq!(g.stack.iter().filter(|o| g.obj(**o).ability.is_some()).count(), usize::from(holder == 0)));
        if holder == 0 {
            cite!(["603.3a"]; assert_eq!(g.controller(*g.stack.last().unwrap()), 0));
        }
    }
}

#[test]
fn mandatory_empty_alternatives_and_optional_kept_illegal_are_distinct() {
    let mut g = setup(&["Giant Growth", "Grizzly Bears"]);
    let bear = put(&mut g, 0, "Grizzly Bears", Zone::Battlefield);
    let growth = cast(&mut g, 0, "Giant Growth", vec![vec![Target::Object(bear)]], &[], "G");
    to_other(&mut g);
    cast(&mut g, 1, "Imp's Mischief", vec![vec![Target::Object(growth)]], &[], "WB");
    g.move_object(bear, Zone::Hand);
    pass(&mut g);
    assert!(g.decision.is_none());
    cite!(["115.7a"]; assert_eq!(g.obj(growth).targets, [vec![Target::Object(bear)]]));
    assert_eq!(g.players[1].life, 19);
    pass(&mut g);
    cite!(["608.2b"]; assert!(g.log.iter().any(|e| matches!(e,Event::Fizzled{spell} if *spell==growth))));
    let mut g = setup(&["Arc Trail", "Craw Wurm"]);
    let a = put(&mut g, 0, "Craw Wurm", Zone::Battlefield);
    let b = put(&mut g, 1, "Craw Wurm", Zone::Battlefield);
    let c = put(&mut g, 1, "Craw Wurm", Zone::Battlefield);
    let arc = cast(&mut g, 0, "Arc Trail", vec![vec![Target::Object(a)], vec![Target::Object(b)]], &[], "WR");
    to_other(&mut g);
    cast(&mut g, 1, "Redirect", vec![vec![Target::Object(arc)]], &[], "UU");
    g.move_object(a, Zone::Hand);
    pass(&mut g);
    change(&mut g, vec![vec![Target::Object(a)], vec![Target::Object(c)]]);
    pass(&mut g);
    cite!(["115.7d"]; assert_eq!(g.obj(c).damage, 1));
    assert_eq!(g.obj(b).damage, 0);
}

#[test]
fn alternative_costs_and_power_discount_use_existing_payment() {
    let mut g = setup(&["Lightning Bolt", "Island"]);
    let bolt = cast(&mut g, 0, "Lightning Bolt", vec![vec![Target::Player(1)]], &[], "R");
    to_other(&mut g);
    let card = put(&mut g, 1, "Misdirection", Zone::Hand);
    let blue = put(&mut g, 1, "Redirect", Zone::Hand);
    let nonblue = put(&mut g, 1, "Island", Zone::Hand);
    let exile = |paid| {
        Move::CastFor(
            CastCost::Alternative(0),
            Box::new(Move::WithCosts(
                vec![vec![paid]],
                Box::new(Move::Cast { card, targets: vec![vec![Target::Object(bolt)]] }),
            )),
        )
    };
    cite!(["118.3"]; refused("Island is not what that cost asks for or cannot pay it (701.13a, 118.3)"): g.act(1, exile(nonblue)));
    assert_eq!(g.obj(nonblue).zone, Zone::Hand);
    cite!(["118.9"]; ok: g.act(1, exile(blue)));
    assert_eq!(g.players[1].pool.total(), 0);
    assert_eq!(g.exile.len(), 1);
    pass(&mut g);
    change(&mut g, vec![vec![Target::Player(0)]]);
    pass(&mut g);
    assert_eq!(g.players[0].life, 17);
    for (owner, name, discount) in [(1, "Craw Wurm", true), (0, "Craw Wurm", false), (1, "Hill Giant", false)] {
        let mut g = setup(&["Lightning Bolt", name]);
        put(&mut g, owner, name, Zone::Battlefield);
        let bolt = cast(&mut g, 0, "Lightning Bolt", vec![vec![Target::Player(1)]], &[], "R");
        to_other(&mut g);
        let bend = put(&mut g, 1, "Bolt Bend", Zone::Hand);
        g.players[1].pool.colors[3] = 1;
        let mv = paid(if discount { "R" } else { "CCCR" }, Move::Cast { card: bend, targets: vec![vec![Target::Object(bolt)]] });
        if !discount {
            cite!(["601.2f"]; refused("that mana is not in your mana pool (118.3a)"): g.act(1, mv.clone()));
            g.players[1].pool.colorless = 3;
        }
        g.act(1, mv).unwrap();
        cite!(["601.2f"]; case: format!("{owner} {name}"); assert_eq!(g.players[1].pool.total(), 0));
        pass(&mut g);
        change(&mut g, vec![vec![Target::Player(0)]]);
    }
    for own in [false, true] {
        let mut g = setup(&["Lightning Bolt", "Isamaru, Hound of Konda"]);
        let commander = put(&mut g, usize::from(own), "Isamaru, Hound of Konda", Zone::Command);
        g.designate_commander(commander).unwrap();
        g.move_object(commander, Zone::Battlefield);
        let bolt = cast(&mut g, 0, "Lightning Bolt", vec![vec![Target::Player(1)]], &[], "R");
        to_other(&mut g);
        let card = put(&mut g, 1, "Deflecting Swat", Zone::Hand);
        let mv = Move::CastFor(
            CastCost::Alternative(0),
            Box::new(Move::Cast { card, targets: vec![vec![Target::Object(bolt)]] }),
        );
        if own {
            cite!(["118.9"]; ok: g.act(1, mv));
            pass(&mut g);
            change(&mut g, vec![vec![Target::Player(0)]]);
        } else {
            cite!(["118.9"]; refused("the condition for that alternative cost is not true (118.9, 601.2b)"): g.act(1, mv));
            assert_eq!(g.obj(card).zone, Zone::Hand);
        }
    }
}

#[test]
fn ricochet_alternative_requires_opponents_blue_spell_this_turn() {
    for (blue, own, later, allowed) in [
        (true, false, false, true),
        (false, false, false, false),
        (true, true, false, false),
        (true, false, true, false),
    ] {
        let mut g = setup(&["Unsummon", "Lightning Bolt", "Island", "Grizzly Bears"]);
        for p in 0..2 {
            for _ in 0..4 {
                put(&mut g, p, "Island", Zone::Library);
            }
        }
        let player = usize::from(own);
        if player == 1 {
            to_other(&mut g);
        }
        let bear = put(&mut g, 0, "Grizzly Bears", Zone::Battlefield);
        cast(
            &mut g,
            player,
            if blue { "Unsummon" } else { "Lightning Bolt" },
            if blue { vec![vec![Target::Object(bear)]] } else { vec![vec![Target::Player(1)]] },
            &[],
        if blue { "U" } else { "R" });
        // The conditional cost observes cast history, even after that spell resolves.
        pass(&mut g);
        if later {
            let turn = g.turn;
            g.part = ir::Part::End;
            while g.turn == turn {
                g.act(g.priority, Move::Pass).unwrap();
            }
        }
        g.priority = 0;
        let bolt = cast(&mut g, 0, "Lightning Bolt", vec![vec![Target::Player(1)]], &[], "R");
        to_other(&mut g);
        let trap = put(&mut g, 1, "Ricochet Trap", Zone::Hand);
        g.players[1].pool.colors = [0; 5];
        g.players[1].pool.colorless = 0;
        g.players[1].pool.colors[3] = 1;
        let action = paid("R", Move::CastFor(
            CastCost::Alternative(0),
            Box::new(Move::Cast { card: trap, targets: vec![vec![Target::Object(bolt)]] }),
        ));
        if allowed {
            cite!(["118.9"]; ok: g.act(1, action));
            assert_eq!(g.players[1].pool.total(), 0);
            pass(&mut g);
            change(&mut g, vec![vec![Target::Player(0)]]);
        } else {
            cite!(["118.9"]; refused("the condition for that alternative cost is not true (118.9, 601.2b)"): g.act(1, action));
            assert_eq!(g.obj(trap).zone, Zone::Hand);
        }
    }
}

#[test]
fn other_modes_execute_and_original_departure_fizzles_changer() {
    let mut g = setup(&["Mind Stone", "Grizzly Bears"]);
    let stone = put(&mut g, 0, "Mind Stone", Zone::Battlefield);
    cast(&mut g, 0, "Untimely Malfunction", vec![vec![Target::Object(stone)]], &[0], "WR");
    pass(&mut g);
    assert_eq!(g.obj(stone).zone, Zone::Gone);
    let a = put(&mut g, 1, "Grizzly Bears", Zone::Battlefield);
    let b = put(&mut g, 1, "Grizzly Bears", Zone::Battlefield);
    cast(&mut g, 0, "Untimely Malfunction", vec![vec![Target::Object(a), Target::Object(b)]], &[2], "WR");
    pass(&mut g);
    assert!(g.cant_block(a, b, &[]));
    assert!(g.cant_block(b, a, &[]));
    for mode in [0, 2] {
        let mut g = setup(&["Lightning Bolt"]);
        let bolt = cast(&mut g, 0, "Lightning Bolt", vec![vec![Target::Player(1)]], &[], "R");
        to_other(&mut g);
        cast(&mut g, 1, "Insidious Will", vec![vec![Target::Object(bolt)]], &[mode], "WWUU");
        pass(&mut g);
        if mode == 0 {
            assert!(!g.stack.contains(&bolt));
        } else {
            cite!(["707.10c"]; assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::Retarget { .. }, .. }))));
            g.act(1, Move::Choose(Answer::Targets(vec![vec![Target::Player(0)]]))).unwrap();
            pass(&mut g);
            assert_eq!(g.players[0].life, 17);
        }
    }
    let mut g = setup(&["Lightning Bolt", "Counterspell"]);
    let bolt = cast(&mut g, 0, "Lightning Bolt", vec![vec![Target::Player(1)]], &[], "R");
    to_other(&mut g);
    let trap = cast(&mut g, 1, "Ricochet Trap", vec![vec![Target::Object(bolt)]], &[], "WWWR");
    to_other(&mut g);
    cast(&mut g, 0, "Counterspell", vec![vec![Target::Object(bolt)]], &[], "UU");
    pass(&mut g);
    pass(&mut g);
    assert!(g.decision.is_none());
    cite!(["608.2b"]; assert!(g.log.iter().any(|e| matches!(e,Event::Fizzled{spell} if *spell==trap))));
}

#[test]
fn optional_offers_exclude_the_original_stack_object() {
    let mut g = setup(&["Lightning Bolt", "Counterspell"]);
    let bolt = cast(&mut g, 0, "Lightning Bolt", vec![vec![Target::Player(1)]], &[], "R");
    to_other(&mut g);
    let counter = cast(&mut g, 1, "Counterspell", vec![vec![Target::Object(bolt)]], &[], "UU");
    to_other(&mut g);
    let redirect = cast(&mut g, 0, "Redirect", vec![vec![Target::Object(counter)]], &[], "UU");
    pass(&mut g);
    let Some(Decision::Choose { choice: Choice::ChangeTargets { slots, .. }, .. }) = &g.decision else { panic!() };
    cite!(["115.5"]; assert!(slots.iter().all(|slot| !slot.among.contains(&Target::Object(counter)))));
    assert!(slots[0].among.contains(&Target::Object(redirect)), "the resolving changer is an alternative");
    cite!(["115.5"]; refused("a changed target must be legal without invalidating an unchanged target (115.7d)"): g.act(0, Move::Choose(Answer::Targets(vec![vec![Target::Object(counter)]]))));
    change(&mut g, vec![vec![Target::Object(redirect)]]);
    pass(&mut g);
    cite!(["608.2b"]; assert!(g.log.iter().any(|e| matches!(e,Event::Fizzled{spell} if *spell==counter))));
}
