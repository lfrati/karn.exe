//! Placer identity and event-time recipient information; admission alone says nothing of how
//! native consumers play. Synthetic spells isolate shared semantics using the actual command path.
use crate::support::{cite, oracle, oracle_read};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::Part;

fn def(name: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    CardDef::new(name, "{0}", types, text, pt).unwrap_or_else(|e| panic!("{name}: {text}: {e}"))
}
fn setup(watcher: CardDef, text: &str, players: usize) -> Game {
    let defs = vec![
        watcher,
        oracle("Island"),
        def("Bear", "Creature — Bear", "", Some((4, 4))),
        def("Place", "Instant", text, None),
        def("Bounce", "Instant", "Return target creature to its owner's hand.", None),
        def("Trinket", "Artifact", "", None),
        def(
            "Animate",
            "Instant",
            "Target artifact becomes an artifact creature with base power and toughness 3/3 until end of turn.",
            None,
        ),
        def("Frog", "Instant", "Target creature becomes an artifact until end of turn.", None),
        def("Steal", "Instant", "Gain control of target creature.", None),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0));
    }
    let mut g = Game::new(defs, players, 20);
    for p in 0..players {
        for _ in 0..20 {
            g.put(p, 1, Zone::Library);
        }
    }
    g
}
fn resolve(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn cast(g: &mut Game, p: usize, d: usize, targets: Vec<Vec<Target>>) {
    while g.priority != p {
        g.act(g.priority, Move::Pass).unwrap();
    }
    let card = g.put(p, d, Zone::Hand);
    g.act(p, Move::Cast { card, targets }).unwrap();
    resolve(g);
}
fn until(g: &mut Game, turn: u32, part: Part) {
    for _ in 0..400 {
        if g.turn == turn && g.part == part {
            return;
        }
        let mv = match g.decision {
            Some(Decision::Attackers { .. }) => Move::Attack(vec![]),
            Some(Decision::Blockers { .. }) => Move::Block(vec![]),
            Some(ref d) => panic!("unexpected {d:?}"),
            None => Move::Pass,
        };
        let p = g.asked().unwrap().0;
        g.act(p, mv).unwrap();
    }
    panic!("boundary not reached")
}
fn placements(g: &Game) -> Vec<(Target, String, u32, usize)> {
    g.log
        .iter()
        .filter_map(|e| match e {
            Event::CountersPut { on, kind, count, by, .. } => Some((*on, kind.clone(), *count, *by)),
            _ => None,
        })
        .collect()
}
fn counter(g: &Game, o: ObjId, k: &str) -> u32 {
    g.counters(o).get(k).copied().unwrap_or(0)
}

// Rules: 603.4, 603.2, 120.3a, 109.5
// Each end step includes the opponents' end steps. The pinned
// ruling's "your end step" is narrower than Oracle and cannot override "each end step".
#[test]
fn r603_tarfire_counter_history_each_end_step_and_reset() {
    let mut g = setup(oracle("Lasting Tarfire"), "Put a +1/+1 counter on target creature.", 3);
    g.put(0, 0, Zone::Battlefield);
    let bear = g.put(1, 2, Zone::Battlefield);
    until(&mut g, 1, Part::End);
    cite!(["603.4"];assert!(g.stack.is_empty()));
    cast(&mut g, 0, 3, vec![vec![bear.into()]]);
    cite!(["603.2","603.4"];assert!(g.stack.is_empty()));
    until(&mut g, 2, Part::PrecombatMain);
    cast(&mut g, 0, 3, vec![vec![bear.into()]]);
    until(&mut g, 2, Part::End);
    cite!(["603.4"];assert_eq!(g.stack.len(),1));
    resolve(&mut g);
    cite!(["120.3a"];assert_eq!(g.players.iter().map(|p|p.life).collect::<Vec<_>>(),vec![20,18,18]));
    until(&mut g, 3, Part::End);
    cite!(["603.4"];assert!(g.stack.is_empty()));
}

// Rules: 109.5, 603.4, 608.2i, 122.6
#[test]
fn r608_tarfire_placer_recipient_kind_zero_and_departure() {
    for (placer, owner, kind, count, leave, want) in [
        (0, 0, "charge", 1, false, true),
        (0, 1, "charge", 3, true, true),
        (1, 0, "+1/+1", 1, false, false),
        (0, 1, "charge", 0, false, false),
    ] {
        let text = if count == 1 {
            format!("Put a {kind} counter on target creature.")
        } else {
            format!("Put {count} {kind} counters on target creature.")
        };
        let mut g = setup(oracle("Lasting Tarfire"), &text, 3);
        g.put(0, 0, Zone::Battlefield);
        let bear = g.put(owner, 2, Zone::Battlefield);
        cast(&mut g, placer, 3, vec![vec![bear.into()]]);
        cite!(["122.6","109.5"];assert_eq!(placements(&g).len(),usize::from(count>0)));
        if count > 0 {
            cite!(["122.6","109.5"];assert_eq!(placements(&g)[0],(bear.into(),kind.into(),count,placer)));
        }
        if leave {
            cast(&mut g, 0, 4, vec![vec![bear.into()]]);
        }
        until(&mut g, 1, Part::End);
        cite!(["603.4","608.2i"];assert_eq!(g.stack.len(),usize::from(want)));
        if want {
            resolve(&mut g);
            cite!(["120.3a"];assert_eq!((g.players[1].life,g.players[2].life),(18,18)));
        }
    }
}

// Rules: 608.2i, 603.4, 613.1d, 400.7
// A noncreature animated later must not count; a creature turned into a noncreature
// later must still count. An old incarnation's placement does not belong to a new one.
#[test]
fn r608_history_uses_placement_time_type_and_source_incarnation() {
    for before in [true, false] {
        let mut g = setup(oracle("Lasting Tarfire"), "Put a charge counter on target artifact.", 2);
        g.put(0, 0, Zone::Battlefield);
        let trinket = g.put(1, 5, Zone::Battlefield);
        if before {
            cast(&mut g, 0, 6, vec![vec![trinket.into()]]);
        }
        cast(&mut g, 0, 3, vec![vec![trinket.into()]]);
        if !before {
            cast(&mut g, 0, 6, vec![vec![trinket.into()]]);
        }
        until(&mut g, 1, Part::End);
        cite!(["608.2i","603.4"];assert_eq!(g.stack.len(),usize::from(before)));
    }
    let watcher = def(
        "History",
        "Enchantment",
        "At the beginning of each end step, if you put a counter on a creature you control this turn, you gain 1 life.",
        None,
    );
    let mut g = setup(watcher, "Put a charge counter on target creature.", 2);
    g.put(0, 0, Zone::Battlefield);
    let bear = g.put(0, 2, Zone::Battlefield);
    cast(&mut g, 0, 3, vec![vec![bear.into()]]);
    cast(&mut g, 1, 8, vec![vec![bear.into()]]);
    until(&mut g, 1, Part::End);
    cite!(["608.2i","603.4"];assert_eq!(g.stack.len(),1));
    resolve(&mut g);
    cite!(["119.3"];assert_eq!(g.players[0].life,21));
    let watcher = def(
        "Self history",
        "Creature",
        "At the beginning of each end step, if you put a counter on this creature this turn, you gain 1 life.",
        Some((2, 2)),
    );
    let mut g = setup(watcher, "Put a charge counter on target creature.", 2);
    let old = g.put(0, 0, Zone::Battlefield);
    cast(&mut g, 0, 3, vec![vec![old.into()]]);
    cast(&mut g, 0, 4, vec![vec![old.into()]]);
    let new = g.players[0].hand[0];
    g.act(0, Move::Cast { card: new, targets: vec![] }).unwrap();
    resolve(&mut g);
    until(&mut g, 1, Part::End);
    cite!(["400.7","603.4"];assert!(g.stack.is_empty()));
}

// Rules: 603.4, 608.2i, 109.5
// The condition is checked again at resolution, but still asks about the past:
// a recipient's departure between the two checks does not erase its placement.
#[test]
fn r603_intervening_if_resolution_still_sees_departed_recipient() {
    // Ability control stays with its triggering controller even if its source changes
    // control (113.8); a creature recipient's departure does not invalidate the history.
    let mut g = setup(oracle("Lasting Tarfire"), "Put a +1/+1 counter on target creature.", 2);
    g.put(0, 0, Zone::Battlefield);
    let bear = g.put(1, 2, Zone::Battlefield);
    cast(&mut g, 0, 3, vec![vec![bear.into()]]);
    until(&mut g, 1, Part::End);
    cite!(["603.4"];assert_eq!(g.stack.len(),1));
    cast(&mut g, 0, 4, vec![vec![bear.into()]]);
    resolve(&mut g);
    cite!(["603.4","608.2i","120.3a"];assert_eq!(g.players[1].life,18));
}

// Rules: 603.2c, 109.5, 122.6
#[test]
fn r603_native_pantry_filters_actor_controller_kind_and_batches() {
    for (placer, owner, kind, want) in
        [(0, 0, "+1/+1", true), (0, 1, "+1/+1", false), (1, 0, "+1/+1", false), (0, 0, "charge", false)]
    {
        let mut g = setup(oracle("Stocking the Pantry"), &format!("Put three {kind} counters on target creature."), 2);
        let pantry = g.put(0, 0, Zone::Battlefield);
        let bear = g.put(owner, 2, Zone::Battlefield);
        cast(&mut g, placer, 3, vec![vec![bear.into()]]);
        cite!(["603.2c","109.5"];assert_eq!(g.stack.len(),usize::from(want)));
        if want {
            resolve(&mut g);
        }
        cite!(["122.6"];assert_eq!(counter(&g,pantry,"supply"),u32::from(want)));
    }
}

// Uncited (admission): inventory of full native definitions; failures are separate blockers.
#[test]
fn native_consumer_inventory() {
    for name in [
        "Lasting Tarfire",
        "Stocking the Pantry",
        "Knight of Wundagore",
        "Inspired Tethermage",
        "The Great Goblin",
        "Mikey & Leo, Chaos & Order",
        "Ant-Man, Colony Commander",
        "Fractal Tender",
    ] {
        let d = oracle_read(name);
        let refusal = game::play::check(&d).err().map(|e| e.0);
        if name == "Fractal Tender" {
            assert_eq!(refusal.as_deref(), Some("keyword: Increment"));
        } else { assert_eq!(refusal, None, "{name}"); }
    }
}

// Rules: 122.6a, 614.12, 603.2, 109.5
// Opponent-controlled Grumgully doesn't make its controller the placer. Likewise a
// spell's caster doesn't place another player's entrant's intrinsic entry counters.
#[test]
fn r122_entry_default_is_entrant_controller_not_effect_controller_or_owner() {
    let defs = vec![
        oracle("Stocking the Pantry"),
        oracle("Faithful Watchdog"),
        def("Global entry", "Enchantment", "Each creature enters with an additional +1/+1 counter on it.", None),
        def(
            "Return",
            "Instant",
            "Put target creature card from a graveyard onto the battlefield under your control.",
            None,
        ),
    ];
    for d in &defs {
        game::play::check(d).unwrap();
    }
    let mut g = Game::new(defs, 2, 20);
    let pantry = g.put(1, 0, Zone::Battlefield);
    let modifier = g.put(0, 2, Zone::Battlefield);
    let card = g.put(0, 1, Zone::Graveyard);
    cast(&mut g, 1, 3, vec![vec![card.into()]]);
    while let Some(Decision::Choose { player, choice: Choice::Replacement { among, .. } }) = g.decision.clone() {
        let r = among.iter().find(|r| r.source == modifier).unwrap_or(&among[0]).clone();
        g.act(player, Move::Choose(Answer::Replacement(r))).unwrap();
    }
    let dog = *g.battlefield.iter().find(|o| g.obj(**o).chars.name == "Faithful Watchdog").unwrap();
    cite!(["122.6a","614.12"];assert_eq!(placements(&g),vec![(dog.into(),"+1/+1".into(),4,1)]));
    cite!(["603.2c","109.5"];assert_eq!(g.stack.len(),1));
    resolve(&mut g);
    cite!(["122.6"];assert_eq!(counter(&g,pantry,"supply"),1));
}

// Rules: 616.1, 614.5, 603.2c, 122.6
#[test]
fn r616_replacement_chooser_does_not_become_placer_and_batches_remain_distinct() {
    let mut g = setup(oracle("Stocking the Pantry"), "Put two +1/+1 counters on each creature.", 2);
    let modifier = g.defs.len();
    // A separate fixture game holds the full immutable definitions.
    let mut defs = g.defs.to_vec();
    defs.push(def("Double","Enchantment","If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.",None));
    g = Game::new(defs, 2, 20);
    let pantry = g.put(0, 0, Zone::Battlefield);
    let mine = g.put(0, 2, Zone::Battlefield);
    let enemy = g.put(1, 2, Zone::Battlefield);
    g.put(1, modifier, Zone::Battlefield);
    cast(&mut g, 0, 3, vec![]);
    cite!(["616.1","122.6"];assert_eq!(placements(&g),vec![(mine.into(),"+1/+1".into(),2,0),(enemy.into(),"+1/+1".into(),4,0)]));
    cite!(["603.2c","109.5"];assert_eq!(g.stack.len(),1));
    resolve(&mut g);
    cite!(["122.6"];assert_eq!(counter(&g,pantry,"supply"),1));
}

// Rules: 701.34a, 109.5, 603.2c
#[test]
fn r701_proliferate_retains_chooser_on_own_and_opponent_holders() {
    let mut g = setup(oracle("Stocking the Pantry"), "Proliferate.", 2);
    let pantry = g.put(0, 0, Zone::Battlefield);
    let mine = g.put(0, 2, Zone::Battlefield);
    let enemy = g.put(1, 2, Zone::Battlefield);
    // Initial inventories are setup, not placement events.
    for o in [mine, enemy] {
        g.set_initial_counters(o, "+1/+1", 1);
        g.set_initial_counters(o, "charge", 2);
    }
    cast(&mut g, 0, 3, vec![]);
    cite!(["701.34a"];assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::Holders{..}}))));
    g.act(0, Move::Choose(Answer::Holders(vec![mine.into(), enemy.into()]))).unwrap();
    cite!(["701.34a","109.5"];assert_eq!(placements(&g).iter().map(|p|p.3).collect::<Vec<_>>(),vec![0,0,0,0]));
    cite!(["603.2c"];assert_eq!(g.stack.len(),1));
    resolve(&mut g);
    cite!(["122.6"];assert_eq!(counter(&g,pantry,"supply"),1));
}

// Rules: 120.3d, 702.80a, 122.6, 608.2i
#[test]
fn r120_damage_results_use_source_controller_for_tarfire_history() {
    let mut defs = setup(oracle("Lasting Tarfire"), "Put a charge counter on target creature.", 2).defs.to_vec();
    defs.push(def(
        "Wither source",
        "Creature",
        "Wither\n{0}: This creature deals 1 damage to target creature.",
        Some((1, 1)),
    ));
    for placer in [0, 1] {
        let mut g = Game::new(defs.clone(), 2, 20);
        for p in 0..2 {
            for _ in 0..10 {
                g.put(p, 1, Zone::Library);
            }
        }
        g.put(0, 0, Zone::Battlefield);
        let source = g.put(placer, 9, Zone::Battlefield);
        let bear = g.put(1 - placer, 2, Zone::Battlefield);
        while g.priority != placer {
            g.act(g.priority, Move::Pass).unwrap();
        }
        g.act(placer, Move::Activate { source, ability: 1, targets: vec![vec![bear.into()]], paying: vec![] }).unwrap();
        resolve(&mut g);
        cite!(["120.3d","702.80a"];assert_eq!(placements(&g),vec![(bear.into(),"-1/-1".into(),1,placer)]));
        until(&mut g, 1, Part::End);
        cite!(["608.2i","603.4"];assert_eq!(g.stack.len(),usize::from(placer==0)));
    }
}

// Rules: 107.1b, 733.1, 122.6, 603.4
#[test]
fn r107_overflow_refuses_before_events_history_or_partial_inventory() {
    let mut g = setup(oracle("Lasting Tarfire"), "Put a charge counter on each creature.", 2);
    g.put(0, 0, Zone::Battlefield);
    let a = g.put(0, 2, Zone::Battlefield);
    let b = g.put(1, 2, Zone::Battlefield);
    g.set_initial_counters(b, "charge", u32::MAX);
    let spell = g.put(0, 3, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
    g.act(0, Move::Pass).unwrap();
    let before = format!("{g:?}");
    cite!(["107.1b","733.1"];refused("engine numerical range"):g.act(1,Move::Pass));
    cite!(["733.1"];assert_eq!(format!("{g:?}"),before));
    cite!(["122.6"];assert_eq!((counter(&g,a,"charge"),placements(&g).len()),(0,0)));
}

// Rules: 122.6, 101.2, 603.4
#[test]
fn r122_forbidden_placements_emit_no_actor_event_or_history() {
    let mut defs = setup(oracle("Lasting Tarfire"), "Put a +1/+1 counter on target creature.", 2).defs.to_vec();
    defs.push(def("Ban", "Enchantment", "Creatures can't have counters put on them.", None));
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 {
        for _ in 0..10 {
            g.put(p, 1, Zone::Library);
        }
    }
    g.put(0, 0, Zone::Battlefield);
    g.put(0, 9, Zone::Battlefield);
    let bear = g.put(1, 2, Zone::Battlefield);
    cast(&mut g, 0, 3, vec![vec![bear.into()]]);
    cite!(["101.2","122.6"];assert!(placements(&g).is_empty()));
    until(&mut g, 1, Part::End);
    cite!(["603.4"];assert!(g.stack.is_empty()));
}

// Rules: 603.2c, 101.1, 109.5, 122.6
// Full native cards: another player's placement must not spend the once-per-turn
// allowance. A three-counter batch by you fires just once; a later placement doesn't.
#[test]
fn r603_native_once_per_turn_consumers_keep_actor_and_allowance() {
    for name in ["Knight of Wundagore", "Mikey & Leo, Chaos & Order", "Ant-Man, Colony Commander"] {
        let mut g = setup(oracle(name), "Put three +1/+1 counters on target creature.", 2);
        let watcher = g.put(0, 0, Zone::Battlefield);
        let bear = g.put(0, 2, Zone::Battlefield);
        cast(&mut g, 1, 3, vec![vec![bear.into()]]);
        cite!(["603.2","109.5"];assert!(g.stack.is_empty()));
        cast(&mut g, 0, 3, vec![vec![bear.into()]]);
        cite!(["603.2c","101.1"];assert_eq!(g.stack.len(),1));
        resolve(&mut g);
        match name {
            "Knight of Wundagore" => cite!(["122.6"];assert_eq!(counter(&g,watcher,"+1/+1"),1)),
            "Mikey & Leo, Chaos & Order" => cite!(["121.2"];assert_eq!(g.players[0].hand.len(),1)),
            _ => cite!(["111.3"];assert_eq!(g.battlefield.iter().filter(|o|g.obj(**o).token()).count(),1)),
        }
        cast(&mut g, 0, 3, vec![vec![bear.into()]]);
        cite!(["101.1"];assert!(g.stack.is_empty()));
        until(&mut g, 2, Part::PrecombatMain);
        cast(&mut g, 0, 3, vec![vec![bear.into()]]);
        cite!(["101.1"];assert_eq!(g.stack.len(),1));
    }
}

// Rules: 606.4, 118.11, 122.6a, 603.2c, 109.5
#[test]
fn r606_native_tethermage_loyalty_cost_and_entry_attribution() {
    let walker = CardDef::planeswalker("Walker", "{0}", "Planeswalker — Jace", "+2: You gain 1 life.", 3).unwrap();
    let defs = vec![oracle("Inspired Tethermage"), walker];
    let mut g = Game::new(defs.clone(), 2, 20);
    let mage = g.put(0, 0, Zone::Battlefield);
    let w = g.put(0, 1, Zone::Battlefield);
    g.set_initial_counters(w, "loyalty", 3);
    g.act(0, Move::Activate { source: w, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    cite!(["606.4","118.11"];assert_eq!(placements(&g),vec![(w.into(),"loyalty".into(),2,0)]));
    cite!(["603.2c"];assert_eq!(g.stack.len(),2));
    resolve(&mut g);
    cite!(["122.6"];assert_eq!(counter(&g,mage,"+1/+1"),1));
    resolve(&mut g);
    for entrant in [0, 1] {
        let mut g = Game::new(defs.clone(), 2, 20);
        for p in 0..2 {
            for _ in 0..10 {
                g.put(p, 1, Zone::Library);
            }
        }
        g.put(0, 0, Zone::Battlefield);
        if entrant == 1 {
            until(&mut g, 2, Part::PrecombatMain);
        }
        let spell = g.put(entrant, 1, Zone::Hand);
        g.act(entrant, Move::Cast { card: spell, targets: vec![] }).unwrap();
        resolve(&mut g);
        cite!(["122.6a","603.2c","109.5"];assert_eq!(g.stack.len(),usize::from(entrant==0)));
        cite!(["122.6a"];assert_eq!(placements(&g)[0].3,entrant));
    }
}

// Rules: 603.2c, 109.5, 603.3d, 120.3a
#[test]
fn r603_native_great_goblin_filters_actor_subtype_and_control() {
    for (placer, owner, subtype, want) in [
        (0, 0, "Goblin", true),
        (0, 0, "Orc", true),
        (0, 0, "Army", true),
        (0, 0, "Bear", false),
        (0, 1, "Goblin", false),
        (1, 0, "Goblin", false),
    ] {
        let mut defs =
            setup(oracle("The Great Goblin"), "Put three charge counters on target creature.", 2).defs.to_vec();
        defs[2] = def("Recipient", &format!("Creature — {subtype}"), "", Some((4, 4)));
        let mut g = Game::new(defs, 2, 20);
        g.put(0, 0, Zone::Battlefield);
        let recipient = g.put(owner, 2, Zone::Battlefield);
        cast(&mut g, placer, 3, vec![vec![recipient.into()]]);
        if want {
            cite!(["603.3d"];assert!(matches!(g.decision,Some(Decision::Targets{player:0,..}))));
            g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap();
            cite!(["603.2c"];assert_eq!(g.stack.len(),1));
            resolve(&mut g);
        } else {
            cite!(["109.5","603.2"];assert!(g.stack.is_empty() && g.decision.is_none()));
        }
        cite!(["120.3a"];assert_eq!(g.players[1].life,if want{18}else{20}));
    }
}

// Rules: 608.2c, 109.5, 122.6, 603.2
// Ability controller p0 instructs target player p1 to place on p0's creature.
#[test]
fn r608_explicit_instructed_player_is_not_ability_controller() {
    let mut g = setup(oracle("Stocking the Pantry"), "Target player puts a +1/+1 counter on target creature.", 2);
    g.put(0, 0, Zone::Battlefield);
    let bear = g.put(0, 2, Zone::Battlefield);
    cast(&mut g, 0, 3, vec![vec![Target::Player(1)], vec![bear.into()]]);
    cite!(["608.2c","122.6"];assert_eq!(placements(&g),vec![(bear.into(),"+1/+1".into(),1,1)]));
    cite!(["109.5","603.2"];assert!(g.stack.is_empty()));
}

// Rules: 608.2i, 603.4, 613.1d
#[test]
fn r608_creature_losing_its_type_after_placement_still_counts() {
    let mut g = setup(oracle("Lasting Tarfire"), "Put a charge counter on target creature.", 2);
    g.put(0, 0, Zone::Battlefield);
    let bear = g.put(1, 2, Zone::Battlefield);
    cast(&mut g, 0, 3, vec![vec![bear.into()]]);
    cast(&mut g, 0, 7, vec![vec![bear.into()]]);
    cite!(["613.1d"];assert!(!g.seen(bear,None).types.contains(&ir::CardType::Creature)));
    until(&mut g, 1, Part::End);
    cite!(["608.2i","603.4"];assert_eq!(g.stack.len(),1));
}

// Rules: 113.8, 603.4, 109.5, 608.2i, 120.3a
#[test]
fn r113_ability_controller_survives_source_control_change_before_resolution() {
    let mut defs = setup(oracle("Lasting Tarfire"), "Put a charge counter on target creature.", 2).defs.to_vec();
    defs[8] = def("Steal", "Instant", "Gain control of target permanent.", None);
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 {
        for _ in 0..10 {
            g.put(p, 1, Zone::Library);
        }
    }
    let tarfire = g.put(0, 0, Zone::Battlefield);
    let bear = g.put(1, 2, Zone::Battlefield);
    cast(&mut g, 0, 3, vec![vec![bear.into()]]);
    until(&mut g, 1, Part::End);
    cite!(["603.4"];assert_eq!(g.stack.len(),1));
    cast(&mut g, 1, 8, vec![vec![tarfire.into()]]);
    cite!(["611.2a"];assert_eq!(g.controller(tarfire),1));
    resolve(&mut g);
    cite!(["113.8","109.5","608.2i","120.3a"];assert_eq!((g.players[0].life,g.players[1].life),(20,18)));
}

// Rules: 122.6a, 614.12, 109.5, 603.2c
#[test]
fn r122_entry_caster_is_not_default_placer_when_entry_controller_differs() {
    let defs = vec![
        oracle("Stocking the Pantry"),
        oracle("Faithful Watchdog"),
        def(
            "Return",
            "Instant",
            "Exile target creature you control, then return that card to the battlefield under its owner's control.",
            None,
        ),
        def("Steal", "Instant", "Gain control of target creature.", None),
    ];
    for d in &defs {
        game::play::check(d).unwrap();
    }
    let mut g = Game::new(defs, 2, 20);
    g.put(0, 0, Zone::Battlefield);
    let card = g.put(0, 1, Zone::Battlefield);
    g.set_initial_counters(card, "+1/+1", 3);
    cast(&mut g, 1, 3, vec![vec![card.into()]]);
    cast(&mut g, 1, 2, vec![vec![card.into()]]);
    cite!(["122.6a"];assert_eq!(placements(&g).iter().map(|p|p.3).collect::<Vec<_>>(),vec![0]));
    cite!(["603.2c","109.5"];assert_eq!(g.stack.len(),1));
}

// Rules: 400.7, 608.2i, 603.4, 111.3, 122.6
// Exact Oracle last ability of Fractal Tender, isolated because the full card refuses
// Increment. This is composition evidence, not evidence of full native support.
#[test]
fn r608_fractal_scoped_history_token_and_new_incarnation() {
    for leave in [false, true] {
        let watcher = def(
            "Fractal history",
            "Creature — Elf Wizard",
            "At the beginning of each end step, if you put a counter on this creature this turn, create a 0/0 green and blue Fractal creature token and put three +1/+1 counters on it.",
            Some((3, 3)),
        );
        let mut g = setup(watcher, "Put a charge counter on target creature.", 2);
        let old = g.put(0, 0, Zone::Battlefield);
        cast(&mut g, 0, 3, vec![vec![old.into()]]);
        if leave {
            cast(&mut g, 0, 4, vec![vec![old.into()]]);
            let card = g.players[0].hand[0];
            g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
            resolve(&mut g);
        }
        until(&mut g, 1, Part::End);
        cite!(["400.7","608.2i","603.4"];assert_eq!(g.stack.len(),usize::from(!leave)));
        if !leave {
            resolve(&mut g);
            let tokens: Vec<_> = g.battlefield.iter().filter(|o| g.obj(**o).token()).copied().collect();
            cite!(["111.3"];assert_eq!(tokens.len(),1));
            let token = tokens[0];
            cite!(["111.3","122.6"];assert_eq!((g.seen(token,None).pt,counter(&g,token,"+1/+1")),(Some((3,3)),3)));
        }
    }
}

// Rules: 603.2c, 122.6, 701.34a
#[test]
fn r603_generic_counter_trigger_groups_kinds_but_not_instructions_or_recipients() {
    for text in ["Proliferate.", "Put a charge counter on target creature. Put a stun counter on that creature."] {
        let watcher = def(
            "Watcher",
            "Enchantment",
            "Whenever you put one or more counters on a creature you control, you gain 1 life.",
            None,
        );
        let mut g = setup(watcher, text, 2);
        g.put(0, 0, Zone::Battlefield);
        let a = g.put(0, 2, Zone::Battlefield);
        let b = g.put(0, 2, Zone::Battlefield);
        for o in [a, b] {
            g.set_initial_counters(o, "charge", 1);
            g.set_initial_counters(o, "stun", 1);
        }
        if text == "Proliferate." {
            cast(&mut g, 0, 3, vec![]);
            g.act(0, Move::Choose(Answer::Holders(vec![a.into(), b.into()]))).unwrap();
        } else {
            cast(&mut g, 0, 3, vec![vec![a.into()]]);
        }
        if let Some(Decision::Order { player, abilities }) = g.decision.clone() {
            g.act(player, Move::Order((0..abilities.len()).collect())).unwrap();
        }
        // Proliferating two creatures: one per creature, not one per counter kind.
        // Two written instructions on one creature: two, not one coalesced trigger.
        cite!(["603.2c","122.6"];assert_eq!(g.stack.len(),2));
        resolve(&mut g);
        resolve(&mut g);
        cite!(["119.3"];assert_eq!(g.players[0].life,22));
    }
}

// Rules: 603.10, 603.2c, 608.2c, 608.2i, 122.1a, 613.4c
#[test]
fn r603_simultaneous_recipients_use_completed_instruction_characteristics() {
    for lord_first in [false, true] {
        for text in [
            "Put a +1/+1 counter on each creature you control.",
            "Put a +1/+1 counter on target creature. Put a +1/+1 counter on target creature.",
        ] {
            let simultaneous = text.starts_with("Put a +1/+1 counter on each");
            let defs = vec![
                def("Elf", "Creature — Elf", "", Some((2, 2))),
                def("Lord", "Creature — Elf", "Other Elf creatures you control get +1/+1 for each +1/+1 counter on this creature.", Some((2, 2))),
                def("Place", "Instant", text, None),
                def("Watcher", "Enchantment", "Whenever you put one or more counters on a creature with power 4 or greater, you gain 1 life.", None),
            ];
            for d in &defs { game::play::check(d).unwrap(); }
            let mut g = Game::new(defs, 2, 20);
            let (elf, lord) = if lord_first {
                let lord = g.put(0, 1, Zone::Battlefield);
                (g.put(0, 0, Zone::Battlefield), lord)
            } else {
                let elf = g.put(0, 0, Zone::Battlefield);
                (elf, g.put(0, 1, Zone::Battlefield))
            };
            g.put(0, 3, Zone::Battlefield);
            let targets = if simultaneous { vec![] } else { vec![vec![elf.into()], vec![lord.into()]] };
            cast(&mut g, 0, 2, targets);
            let snapshot = g.log.iter().find_map(|e| match e {
                Event::CountersPut { on: Target::Object(o), recipient: Some(seen), by: 0, .. } if *o == elf => Some(seen.pt),
                _ => None,
            }).unwrap();
            // The final Elf is 4/4 in both cases. Separate written instructions must
            // retain the 3/3 state of its first placement, even in one resolution.
            cite!(["122.1a","613.4c"];assert_eq!(g.seen(elf,None).pt,Some((4,4))));
            cite!(["603.10","608.2c","608.2i"];assert_eq!(snapshot,Some((if simultaneous {4} else {3},if simultaneous {4} else {3}))));
            cite!(["603.2c","603.10"];assert_eq!(g.stack.len(),usize::from(simultaneous)));
            if simultaneous {
                resolve(&mut g);
                cite!(["119.3"];assert_eq!(g.players[0].life,21));
            }
        }
    }
}

// Rules: 603.4, 603.10, 608.2i, 400.7, 613.4c, 122.1a
#[test]
fn r608_simultaneous_recipient_history_survives_departure() {
    let defs = vec![
        def("Elf", "Creature — Elf", "", Some((2, 2))),
        def("Lord", "Creature — Elf", "Other Elf creatures you control get +1/+1 for each +1/+1 counter on this creature.", Some((2, 2))),
        def("Place", "Instant", "Put a +1/+1 counter on each creature you control.", None),
        def("Watcher", "Enchantment", "At the beginning of each end step, if you put one or more counters on a creature with power 4 or greater this turn, you gain 1 life.", None),
        def("Bounce", "Instant", "Return target creature to its owner's hand.", None),
        oracle("Island"),
    ];
    for d in &defs { game::play::check(d).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p,5,Zone::Library); } }
    let elf = g.put(0,0,Zone::Battlefield);
    g.put(0,1,Zone::Battlefield);
    g.put(0,3,Zone::Battlefield);
    cast(&mut g,0,2,vec![]);
    // The Elf was 4/4 immediately after the simultaneous instruction. Neither
    // leaving the battlefield nor its new hand incarnation can erase that fact.
    cast(&mut g,0,4,vec![vec![elf.into()]]);
    until(&mut g,1,Part::End);
    cite!(["603.4","608.2i","400.7"];assert_eq!(g.stack.len(),1));
    resolve(&mut g);
    cite!(["119.3","608.2i"];assert_eq!(g.players[0].life,21));
}

// Entry counters must observe the whole
// entry event, rather than the implementation's order of installing entrants.
// Rules: 122.6, 122.6a, 122.1a, 603.10, 613.4c, 608.2i, 608.2c
#[test]
fn r603_simultaneous_entry_counters_use_completed_event_characteristics() {
    for lord_first in [false, true] {
        for simultaneous in [false, true] {
            let text = if simultaneous {
                "Put all creature cards from all graveyards onto the battlefield under your control."
            } else {
                "Put target creature card from a graveyard onto the battlefield under your control. Put target creature card from a graveyard onto the battlefield under your control."
            };
            let defs = vec![
                def("Elf", "Creature — Elf", "This creature enters with a +1/+1 counter on it.", Some((2, 2))),
                def("Lord", "Creature — Elf", "This creature enters with a +1/+1 counter on it.\nOther Elf creatures you control get +1/+1 for each +1/+1 counter on this creature.", Some((2, 2))),
                def("Return", "Sorcery", text, None),
                def("Watcher", "Enchantment", "Whenever you put one or more counters on a creature with power 4 or greater, you gain 1 life.", None),
            ];
            for d in &defs { game::play::check(d).unwrap(); }
            let mut g = Game::new(defs, 2, 20);
            // Ownership differs from the entry controller; the placement default is
            // that controller, not the owner or the player whose graveyard it left.
            let (elf, lord) = if lord_first {
                let lord = g.put(1, 1, Zone::Graveyard);
                (g.put(1, 0, Zone::Graveyard), lord)
            } else {
                let elf = g.put(1, 0, Zone::Graveyard);
                (elf, g.put(1, 1, Zone::Graveyard))
            };
            g.put(0, 3, Zone::Battlefield);
            let targets = if simultaneous { vec![] } else { vec![vec![elf.into()], vec![lord.into()]] };
            cast(&mut g, 0, 2, targets);
            let elf = *g.battlefield.iter().find(|o|g.obj(**o).chars.name=="Elf").unwrap();
            let snapshot = g.log.iter().find_map(|e| match e {
                Event::CountersPut { on: Target::Object(o), recipient: Some(seen), by: 0, .. } if *o == elf => Some(seen.pt),
                _ => None,
            }).unwrap();
            cite!(["122.1a","613.4c"];assert_eq!(g.seen(elf,None).pt,Some((4,4))));
            let expected = if simultaneous { (4,4) } else { (3,3) };
            cite!(["122.6","603.10","608.2c","608.2i"];assert_eq!(snapshot,Some(expected)));
            cite!(["122.6a"];assert!(placements(&g).iter().all(|(_,_,_,by)|*by==0)));
            cite!(["603.10"];assert_eq!(g.stack.len(),usize::from(simultaneous)));
            if simultaneous {
                resolve(&mut g);
                cite!(["119.3"];assert_eq!(g.players[0].life,21));
            }
        }
    }
}

// Rules: 122.6, 603.10, 608.2i, 400.7, 603.4
#[test]
fn r608_simultaneous_entry_history_survives_departure_and_new_incarnation() {
    for lord_first in [false, true] {
        let defs = vec![
            def("Elf", "Creature — Elf", "This creature enters with a +1/+1 counter on it.", Some((2,2))),
            def("Lord", "Creature — Elf", "This creature enters with a +1/+1 counter on it.\nOther Elf creatures you control get +1/+1 for each +1/+1 counter on this creature.", Some((2,2))),
            def("Return", "Sorcery", "Put all creature cards from all graveyards onto the battlefield under your control.", None),
            def("Watcher", "Enchantment", "At the beginning of each end step, if you put one or more counters on a creature with power 4 or greater this turn, you gain 1 life.", None),
            def("Bounce", "Instant", "Return target creature to its owner's hand.", None),
            oracle("Island"),
        ];
        for d in &defs { game::play::check(d).unwrap(); }
        let mut g = Game::new(defs, 2, 20);
        for p in 0..2 { for _ in 0..10 { g.put(p,5,Zone::Library); } }
        let cards = if lord_first { [1,0] } else { [0,1] };
        for d in cards { g.put(0,d,Zone::Graveyard); }
        g.put(0,3,Zone::Battlefield);
        cast(&mut g,0,2,vec![]);
        let elf = *g.battlefield.iter().find(|o|g.obj(**o).chars.name=="Elf").unwrap();
        cast(&mut g,0,4,vec![vec![elf.into()]]);
        let hand = *g.players[0].hand.iter().find(|o|g.obj(**o).chars.name=="Elf").unwrap();
        let lord = *g.battlefield.iter().find(|o|g.obj(**o).chars.name=="Lord").unwrap();
        cast(&mut g,0,4,vec![vec![lord.into()]]);
        // The new incarnation enters as a 3/3 after the Lord leaves. Only the old
        // placement qualifies for the historical power-four condition.
        g.act(0,Move::Cast { card:hand, targets:vec![] }).unwrap();
        resolve(&mut g);
        let new_elf = *g.battlefield.iter().find(|o|g.obj(**o).chars.name=="Elf").unwrap();
        cite!(["400.7","122.1a"];assert_eq!(g.seen(new_elf,None).pt,Some((3,3))));
        let old_snapshot = g.log.iter().find_map(|e| match e {
            Event::CountersPut { on:Target::Object(o), recipient:Some(seen), .. } if *o==elf => Some(seen.pt),
            _ => None,
        });
        cite!(["608.2i","400.7"];assert_eq!(old_snapshot,Some(Some((4,4)))));
        until(&mut g,1,Part::End);
        cite!(["603.4","608.2i"];assert_eq!(g.stack.len(),1));
        resolve(&mut g);
        cite!(["119.3"];assert_eq!(g.players[0].life,21));
    }
}
