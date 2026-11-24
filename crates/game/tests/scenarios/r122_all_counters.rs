//! Explicit-holder removal of every counter kind, from pinned CR122 and Oracle text.
use crate::support::{cite, paid};
use game::state::Decision;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use std::collections::BTreeMap;
use std::sync::LazyLock;
static FACES: LazyLock<Vec<oracle::corpus::Face>> =
    LazyLock::new(|| oracle::corpus::load(&oracle::corpus::default_path()).unwrap());
fn real(name: &str) -> CardDef {
    CardDef::from_faces(&CardDef::catalog(&FACES)[name]).unwrap()
}
fn card(name: &str, kind: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    CardDef::new(name, "{0}", kind, text, pt).unwrap()
}
fn setup(text: &str, extra: Vec<CardDef>) -> Game {
    let mut cards = vec![card("Clear", "Instant", text, None), card("Bear", "Creature — Bear", "", Some((2, 2)))];
    cards.extend(extra);
    for d in &cards {
        game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0));
    }
    Game::new(cards, 2, 20)
}
fn resolve(g: &mut Game) {
    for _ in 0..2 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn cast(g: &mut Game, targets: Vec<Target>) -> ObjId {
    let c = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: c, targets: vec![targets] }).unwrap();
    g.stack[0]
}
fn removed(g: &Game, holder: ObjId) -> BTreeMap<String, u32> {
    let mut counts = BTreeMap::new();
    for e in g.log.iter() {
        if let Event::CountersRemoved { from: Target::Object(o), kind, count, .. } = e {
            if *o == holder {
                assert!(counts.insert(kind.clone(), *count).is_none(), "duplicate removal event for {kind}");
            }
        }
    }
    counts
}

#[test]
fn r122_1_standard_consumers_compile_removal_and_keep_distinct_static_refusals() {
    for name in ["Purging Stormbrood // Absorb Essence", "Perfect Intimidation"] {
        let d = real(name);
        cite!(["122.1"];case:name;assert!(game::play::check(&d).is_ok()));
    }
    // Counter removal no longer blocks this card, but its separate name-changing layer
    // remains unsupported; all four consumer names are not an admission claim.
    assert_eq!(game::play::check(&real("Honest Work")).unwrap_err().0, "becomes: supertypes or a name");
    // its static ability says both what the creature is and a rule for it, and both run
    // (604.1): see r604_1_a_statement_and_its_rules
    cite!(["122.1"]; case: "Enchanted River's Grasp"; assert!(game::play::check(&real("Enchanted River's Grasp")).is_ok()));
}

// Removing the +1/+1 counters makes the marked damage lethal, but every other kind
// must already have been removed before that state-based action. In particular finality
// must not exile the dying creature and indestructible must no longer protect it.
#[test]
fn r704_4_mixed_removal_finishes_before_lethal_damage_sba_and_finality_is_gone() {
    let mut g = setup("Remove all counters from target creature.", vec![]);
    let bear = g.put(1, 1, Zone::Battlefield);
    for (kind, n) in [("+1/+1", 3), ("finality", 2), ("indestructible", 1), ("stun", 2)] {
        g.put_counters(bear, kind, n);
    }
    g.damage(bear, bear.into(), 3);
    cite!(["122.1a","122.1b"];assert_eq!(g.seen(bear,None).pt,Some((5,5))));
    cite!(["122.1b"];assert!(g.has(bear,"Indestructible")));
    cast(&mut g, vec![bear.into()]);
    resolve(&mut g);
    cite!(["122.1"];assert_eq!(removed(&g,bear),BTreeMap::from([("+1/+1".into(),3),("finality".into(),2),("indestructible".into(),1),("stun".into(),2)])));
    cite!(["704.4","704.5g","122.1h"];assert_eq!(g.players[1].graveyard.len(),1));
    cite!(["122.1h"];assert!(g.exile.is_empty()));
    let last_removal = g.log.iter().rposition(|e| matches!(e, Event::CountersRemoved { .. })).unwrap();
    let departure =
        g.log.iter().position(|e| matches!(e,Event::ZoneChanged{was,from:Zone::Battlefield,..} if *was==bear)).unwrap();
    cite!(["704.4"];assert!(last_removal<departure));
}

#[test]
fn r122_1b_all_removal_changes_power_and_keyword_and_stun_no_longer_replaces_untap() {
    let mut g = setup(
        "Remove all counters from target creature.",
        vec![card("Untap", "Instant", "Untap target creature.", None)],
    );
    let bear = g.put(0, 1, Zone::Battlefield);
    g.put_counters(bear, "+1/+1", 2);
    g.put_counters(bear, "flying", 3);
    g.put_counters(bear, "stun", 2);
    g.set_tapped(bear, true);
    cast(&mut g, vec![bear.into()]);
    resolve(&mut g);
    cite!(["122.1a","122.1b"];assert_eq!(g.seen(bear,None).pt,Some((2,2))));
    cite!(["122.1b"];assert!(!g.has(bear,"Flying")));
    cite!(["122.1"];assert_eq!(removed(&g,bear).values().sum::<u32>(),7));
    let untap = g.put(0, 2, Zone::Hand);
    g.act(0, Move::Cast { card: untap, targets: vec![vec![bear.into()]] }).unwrap();
    resolve(&mut g);
    cite!(["122.1d","701.26b"];assert!(!g.obj(bear).tapped));
    cite!(["122.1d"];assert_eq!(removed(&g,bear)["stun"],2));
}

#[test]
fn r122_1e_explicit_permanent_holder_loses_all_loyalty_and_is_put_in_graveyard() {
    // A synthetic permanent-targeting instruction isolates the same removal operation;
    // the four production consumers do not target unanimated planeswalkers.
    let mut g = setup(
        "Remove all counters from target permanent.",
        vec![CardDef::planeswalker("Walker", "{0}", "Planeswalker — Jace", "", 4).unwrap()],
    );
    let walker = g.put(1, 2, Zone::Battlefield);
    g.put_counters(walker, "loyalty", 4);
    g.put_counters(walker, "charge", 2);
    cast(&mut g, vec![walker.into()]);
    resolve(&mut g);
    cite!(["122.1e","704.5i"];assert_eq!(g.players[1].graveyard.len(),1));
    cite!(["122.1"];assert_eq!(removed(&g,walker),BTreeMap::from([("charge".into(),2),("loyalty".into(),4)])));
}

#[test]
fn r601_2c_real_purging_optional_zero_and_counterless_creature_are_legal() {
    for choose_target in [false, true] {
        let mut g = Game::new(
            vec![real("Purging Stormbrood // Absorb Essence"), card("Bear", "Creature — Bear", "", Some((2, 2)))],
            2,
            20,
        );
        let bear = g.put(1, 1, Zone::Battlefield);
        let purging = g.put(0, 0, Zone::Hand);
        g.players[0].pool.colors = [5; 5];
        g.players[0].pool.colorless = 10;
        g.act(0, paid("CCCCB", Move::Cast { card: purging, targets: vec![] })).unwrap();
        resolve(&mut g);
        cite!(["603.3d"];case:choose_target;assert!(matches!(g.decision,Some(Decision::Targets{player:0,groups:1,..}))));
        cite!(["601.2c"];case:choose_target;refused("illegal target"):g.act(0,Move::ChooseTargets(vec![vec![Target::Player(1)]])));
        g.act(0, Move::ChooseTargets(vec![if choose_target { vec![bear.into()] } else { vec![] }])).unwrap();
        resolve(&mut g);
        cite!(["601.2c","122.1"];case:choose_target;assert!(removed(&g,bear).is_empty()));
        cite!(["608.2b"];case:choose_target;assert!(g.stack.is_empty()&&g.battlefield.len()==2));
    }
}

#[test]
fn r122_2_target_departure_drops_counters_without_removal_and_new_incarnation_is_unaffected() {
    let mut g = setup("Remove all counters from target creature.", vec![]);
    let bear = g.put(1, 1, Zone::Battlefield);
    g.put_counters(bear, "+1/+1", 2);
    g.put_counters(bear, "flying", 1);
    cast(&mut g, vec![bear.into()]);
    g.move_object(bear, Zone::Hand);
    let in_hand = g.players[1].hand[0];
    cite!(["122.2","400.7"];assert!(g.obj(in_hand).counters.is_empty()));
    g.move_object(in_hand, Zone::Battlefield);
    let returned = g.battlefield[0];
    g.put_counters(returned, "+1/+1", 1);
    resolve(&mut g);
    cite!(["122.2","608.2b"];assert!(removed(&g,bear).is_empty()&&removed(&g,returned).is_empty()));
    cite!(["400.7","608.2b"];assert_eq!(g.obj(returned).counters.get("+1/+1"),Some(&1)));
}

#[test]
fn r122_1_unsupported_selective_count_among_and_ordinal_removal_watchers_remain_refused() {
    for text in [
        "Remove a counter from target creature.",
        "Remove up to three counters from target creature.",
        "Remove three counters from among creatures you control.",
        "Whenever the third time counter is removed from this creature, you gain 1 life.",
        "Remove all counters from this creature: You gain 1 life.",
    ] {
        let permanent = text.starts_with("Whenever") || text.contains(":");
        let d = CardDef::new("Refused", "{0}", if permanent { "Creature" } else { "Sorcery" }, text, permanent.then_some((2, 2))).unwrap();
        // Uncited (admission): these are implementation limits, not rules prohibitions.
        let reason = if text.starts_with("Whenever") { "trigger: CountersRemoved" }
            else if text.contains(":") { "cost: counter removal needs a fixed named count from this source or one selected permanent" }
            else { "counter removal: named fixed/all count or all kinds, explicit holder and no among required" };
        crate::support::refused(game::play::check(&d).map_err(|e|e.0), reason);
    }
}

#[test]
fn r608_2h_attached_antecedent_does_not_displace_another_explicit_body_participant() {
    // Two explicit creatures are not disambiguated by choosing the most recent one.
    // Unnamed trigger participants have different lexical provenance (Blade of the
    // Bloodchief); this boundary must keep an ambiguous body reference refused.
    let d = CardDef::new("Ambiguous Aura", "{0}", "Enchantment — Aura",
        "Enchant creature\nWhen this Aura enters, tap enchanted creature and tap target creature and remove all counters from it.", None).unwrap();
    cite!(["608.2h"];assert_eq!(game::play::check(&d).unwrap_err().0,"reference: ambiguous earlier subject producers"));
}

#[test]
fn r614_1_counter_placement_replacement_does_not_replace_removal() {
    let mut g=setup("Remove all counters from target creature.",vec![
        card("Double","Enchantment","If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.",None),
        card("Add","Instant","Put a +1/+1 counter on target creature.",None),
    ]);
    let bear = g.put(0, 1, Zone::Battlefield);
    // These existing counters are a declared initial inventory, not a new placement.
    g.set_initial_counters(bear, "+1/+1", 3);
    g.set_initial_counters(bear, "flying", 1);
    g.put(0, 2, Zone::Battlefield);
    cast(&mut g, vec![bear.into()]);
    resolve(&mut g);
    cite!(["614.1","122.1"];assert_eq!(removed(&g,bear),BTreeMap::from([("+1/+1".into(),3),("flying".into(),1)])));
    cite!(["122.1"];assert!(g.obj(bear).counters.is_empty()));
    let add = g.put(0, 3, Zone::Hand);
    g.act(0, Move::Cast { card: add, targets: vec![vec![bear.into()]] }).unwrap();
    resolve(&mut g);
    cite!(["614.1"];assert_eq!(g.obj(bear).counters.get("+1/+1"),Some(&2)));
}

#[test]
fn r608_2h_aura_etb_uses_last_attachment_if_aura_left_and_never_follows_departed_holder() {
    // The exact shared ETB clause is isolated from the still-refused static abilities
    // on Honest Work and Enchanted River's Grasp; this is mechanism evidence.
    for (name, text, damage) in [
        ("synthetic shared Aura ETB", "Enchant creature\nWhen this Aura enters, tap enchanted creature and remove all counters from it.", false),
        ("synthetic attached dealer antecedent", "Enchant creature\nWhen this Aura enters, enchanted creature deals 1 damage to you. Remove all counters from it.", true),
    ] {
        for departure in ["none", "aura", "holder"] {
            let mut g = Game::new(vec![card("Aura","Enchantment — Aura",text,None), card("Bear", "Creature — Bear", "", Some((2, 2)))], 2, 20);
            let bear = g.put(1, 1, Zone::Battlefield);
            g.put_counters(bear, "+1/+1", 2);
            g.put_counters(bear, "stun", 1);
            let aura = g.put(0, 0, Zone::Hand);
            g.players[0].pool.colors = [5; 5];
            g.players[0].pool.colorless = 10;
            g.act(0, Move::Cast { card: aura, targets: vec![vec![bear.into()]] }).unwrap();
            resolve(&mut g);
            cite!(["603.3","115.1"];case:format!("{name}, departure={departure}");assert!(g.decision.is_none()&&g.stack.len()==1));
            let attached = g.battlefield.iter().copied().find(|o| *o != bear).unwrap();
            if departure == "holder" {
                g.move_object(bear, Zone::Hand);
            } else if departure == "aura" {
                g.exile(attached);
            }
            resolve(&mut g);
            if departure == "holder" {
                cite!(["122.2","400.7"];case:name;assert!(removed(&g,bear).is_empty()));
                cite!(["122.2"];case:name;assert!(g.obj(g.players[1].hand[0]).counters.is_empty()));
            } else {
                cite!(["608.2h"];case:name;assert_eq!(g.obj(bear).tapped,!damage));
                cite!(["608.2h","120.3a"];case:name;assert_eq!(g.players[0].life,if damage {19} else {20}));
                cite!(["608.2h","122.1"];case:name;assert_eq!(removed(&g,bear),BTreeMap::from([("+1/+1".into(),2),("stun".into(),1)])));
                cite!(["613.1","122.1a"];case:name;assert_eq!(g.seen(bear,None).pt,Some((2,2))));
            }
        }
    }
}

#[test]
fn r608_2b_perfect_intimidation_other_valid_mode_resolves_but_all_illegal_targets_stop_both() {
    for opponent_hexproof in [false, true] {
        let mut g = Game::new(
            vec![
                real("Perfect Intimidation"),
                card("Bear", "Creature — Bear", "", Some((2, 2))),
                card("Veil", "Enchantment", "You have hexproof.", None),
                card("Discarded", "Instant", "", None),
            ],
            2,
            20,
        );
        let bear = g.put(1, 1, Zone::Battlefield);
        g.put_counters(bear, "+1/+1", 2);
        let first = g.put(1, 3, Zone::Hand);
        let second = g.put(1, 3, Zone::Hand);
        let spell = g.put(0, 0, Zone::Hand);
        g.players[0].pool.colors = [5; 5];
        g.players[0].pool.colorless = 10;
        g.act(
            0,
            Move::Modes(
                vec![0, 1],
                Box::new(paid("CCCB", Move::Cast { card: spell, targets: vec![vec![Target::Player(1)], vec![bear.into()]] })),
            ),
        )
        .unwrap();
        g.move_object(bear, Zone::Hand);
        if opponent_hexproof {
            g.put(1, 2, Zone::Battlefield);
        }
        resolve(&mut g);
        if !opponent_hexproof {
            cite!(["608.2b"];assert!(matches!(g.decision,Some(Decision::Choose{player:1,..}))));
            g.act(1, Move::Choose(game::state::Answer::Objects(vec![first, second]))).unwrap();
            cite!(["608.2b"];assert_eq!(g.exile.len(),2));
        } else {
            cite!(["608.2b"];assert!(g.decision.is_none()&&g.exile.is_empty()));
        }
        cite!(["122.2","608.2b"];case:opponent_hexproof;assert!(removed(&g,bear).is_empty()));
        cite!(["608.2b"];case:opponent_hexproof;assert!(g.stack.is_empty()));
    }
}

#[test]
fn r122_1_explicit_player_holder_removes_each_existing_kind_and_empty_inventory_emits_nothing() {
    // None of the four Standard consumers targets a player; this synthetic instruction
    // checks the shared explicit-holder contract, not additional card admission.
    for populated in [false, true] {
        let mut g = setup("Remove all counters from target player.", vec![]);
        if populated {
            g.put_counters(Target::Player(1), "energy", 4);
            g.put_counters(Target::Player(1), "poison", 2);
        }
        cast(&mut g, vec![Target::Player(1)]);
        resolve(&mut g);
        let removals: Vec<_> = g
            .log
            .iter()
            .filter_map(|e| match e {
                Event::CountersRemoved { from: Target::Player(1), kind, count, .. } => Some((kind.as_str(), *count)),
                _ => None,
            })
            .collect();
        cite!(["122.1"];case:populated;assert_eq!(removals,if populated {vec![("energy",4),("poison",2)]}else{vec![]}));
        cite!(["122.1"];case:populated;assert!(g.players[1].counters.is_empty()));
    }
}
