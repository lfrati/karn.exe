//! Toxic is a combat damage result (120.3g, 702.164), composed with ordinary damage and counters.
use crate::support::cite;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, Target, Zone};
use std::sync::OnceLock;
fn pinned(name: &str) -> CardDef {
    static FACES: OnceLock<Vec<oracle::corpus::Face>> = OnceLock::new();
    let faces = FACES.get_or_init(|| oracle::corpus::load(&oracle::corpus::default_path()).unwrap());
    let d = CardDef::from_faces(CardDef::catalog(faces).get(name).unwrap()).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
    d
}
fn def(name: &str, kind: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    let d = CardDef::new(name, "{0}", kind, text, pt).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{text}: {}", e.0));
    d
}
fn game(mut defs: Vec<CardDef>, players: usize, life: i32) -> Game {
    // A legal zero-power blocker makes the actual declare-blockers priority window reachable.
    let wall = defs.len();
    defs.push(def("Spare Wall", "Creature", "", Some((0, 8))));
    let mut g = Game::new(defs, players, life);
    g.put(1, wall, Zone::Battlefield);
    g
}
fn pass(g: &mut Game) { g.act(g.priority, Move::Pass).unwrap(); }
fn resolve(g: &mut Game) {
    for _ in 0..40 {
        if let Some(Decision::Order { player, abilities }) = g.decision.clone() {
            g.act(player, Move::Order((0..abilities.len()).collect())).unwrap();
        } else {
            assert!(g.decision.is_none(), "{:?}", g.decision);
            if g.stack.is_empty() { return; }
            pass(g);
        }
    }
    panic!("stack did not resolve")
}
fn attack(g: &mut Game, attacks: Vec<(game::ObjId, Target)>, blocks: Vec<(game::ObjId, game::ObjId)>) {
    g.turn = 2;
    let active = g.active;
    g.players[active].turn_began = 2;
    g.part = ir::Part::BeginningOfCombat;
    for (source, _) in &attacks { g.can_attack(*source).unwrap(); }
    for _ in 0..8 {
        if let Some(Decision::Attackers { player }) = g.decision { g.act(player, Move::Attack(attacks)).unwrap(); break; }
        pass(g);
    }
    for _ in 0..12 {
        if let Some(Decision::Blockers { player }) = g.decision { g.act(player, Move::Block(blocks)).unwrap(); return; }
        pass(g);
    }
    panic!("no blockers")
}
fn finish_combat(g: &mut Game) {
    for _ in 0..50 {
        if g.part == ir::Part::EndOfCombat && g.stack.is_empty() && g.decision.is_none() { return; }
        assert!(g.decision.is_none(), "{:?}", g.decision);
        pass(g);
    }
    panic!("combat did not finish")
}
fn poison(g: &Game, player: usize) -> u32 { g.counters(Target::Player(player)).get("poison").copied().unwrap_or(0) }

#[test]
fn pinned_bilious_player_and_blocked_creature_keep_ordinary_damage() {
    for blocked in [false, true] {
        let mut g = game(vec![pinned("Bilious Skulldweller"), def("Wall", "Creature", "", Some((0, 5)))], 2, 20);
        let source = g.put(0, 0, Zone::Battlefield);
        let wall = g.put(1, 1, Zone::Battlefield);
        attack(&mut g, vec![(source, Target::Player(1))], if blocked { vec![(wall, source)] } else { vec![] });
        finish_combat(&mut g);
        cite!(["120.3a"]; assert_eq!(g.players[1].life, if blocked { 20 } else { 19 }));
        cite!(["702.164c"]; assert_eq!(poison(&g, 1), if blocked { 0 } else { 1 }));
        if blocked { cite!(["702.2b"]; assert_eq!(g.obj(wall).zone, Zone::Gone, "ordinary damage still has deathtouch")); }
    }
}
#[test]
fn fixed_values_add_but_damage_amount_and_noncombat_do_not_change_toxic() {
    let mut g = game(vec![def("Dealer", "Creature", "Toxic 1\nToxic 2\nLifelink", Some((4, 5)))], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    g.damage(source, Target::Player(1), 4);
    cite!(["702.164c"]; assert_eq!((g.players[1].life, poison(&g, 1), g.players[0].life), (16, 0, 24)));
    attack(&mut g, vec![(source, Target::Player(1))], vec![]);
    finish_combat(&mut g);
    cite!(["702.164b", "702.164c", "702.15b"]; assert_eq!((g.players[1].life, poison(&g, 1), g.players[0].life), (12, 3, 28)));
}
#[test]
fn pinned_jawbone_adds_toxic_in_each_successful_damage_step() {
    let mut g = game(vec![pinned("Jawbone Duelist")], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    attack(&mut g, vec![(source, Target::Player(1))], vec![]);
    finish_combat(&mut g);
    cite!(["702.164c"]; assert_eq!((g.players[1].life, poison(&g, 1)), (18, 2)));
    let hits = g.log.iter().filter(|e| matches!(e, Event::DamageDealt { source:s, combat:true, .. } if *s == source)).count();
    cite!(["702.4b"]; assert_eq!(hits, 2));
}
#[test]
fn pinned_nurse_equal_grants_after_blockers_sum_and_expire_without_query_accumulation() {
    let mut g = game(vec![pinned("Bilious Skulldweller"), pinned("Plague Nurse")], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let nurses = [g.put(0, 1, Zone::Battlefield), g.put(0, 1, Zone::Battlefield)];
    attack(&mut g, vec![(source, Target::Player(1))], vec![]);
    g.players[0].pool.colors = [8; 5];
    for (i, nurse) in nurses.into_iter().enumerate() {
        g.act(0, crate::support::paid("GGG",Move::Activate { source:nurse, ability:1, targets:vec![], paying:vec![] })).unwrap();
        resolve(&mut g);
        for _ in 0..4 { cite!(["702.164b"]; assert_eq!(g.seen(source, None).total_toxic_value(), i as u32 + 2)); }
        cite!(["602.1b"]; refused("that ability is activated only once each turn (602.1b)"): g.act(0, crate::support::paid("GGG",Move::Activate { source:nurse, ability:1, targets:vec![], paying:vec![] })));
        if i == 0 {
            let mut one_grant = g.clone();
            finish_combat(&mut one_grant);
            assert_eq!((one_grant.players[1].life, poison(&one_grant, 1)), (19, 2));
        }
    }
    finish_combat(&mut g);
    cite!(["702.164c"]; assert_eq!((g.players[1].life, poison(&g, 1)), (19, 3)));
    // Drive real cleanup; no fabricated expiry or direct deletion of effects.
    for _ in 0..20 { if g.active == 1 { break; } pass(&mut g); }
    assert_eq!(g.active, 1);
    cite!(["514.2"]; assert_eq!(g.seen(source, None).total_toxic_value(), 1));
}
#[test]
fn losing_abilities_before_damage_and_zero_power_produce_no_poison() {
    for erase in [false, true] {
        let mut g = game(vec![pinned("Bilious Skulldweller"), def("Erase", "Instant", "Target creature loses all abilities until end of turn.", None), def("Shrink", "Instant", "Target creature gets -1/-0 until end of turn.", None)], 2, 20);
        let source = g.put(0, 0, Zone::Battlefield);
        attack(&mut g, vec![(source, Target::Player(1))], vec![]);
        let card = g.put(0, if erase {1} else {2}, Zone::Hand);
        g.act(0, Move::Cast { card, targets:vec![vec![source.into()]] }).unwrap();
        resolve(&mut g);
        finish_combat(&mut g);
        cite!(["702.164c"]; assert_eq!(poison(&g, 1), 0));
        cite!(["510.1a"]; assert_eq!(g.players[1].life, if erase {19} else {20}));
    }
}
#[test]
fn pinned_bloated_completes_toxic_before_its_proliferate_trigger_resolves() {
    let mut g = game(vec![pinned("Bloated Contaminator")], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    attack(&mut g, vec![(source, Target::Player(1))], vec![]);
    for _ in 0..30 {
        if let Some(Decision::Choose { player, choice:Choice::Holders { among, .. } }) = g.decision.clone() {
            cite!(["702.164c"]; assert_eq!((g.players[1].life, poison(&g, 1)), (16, 1)));
            cite!(["701.34a"]; assert!(among.contains(&Target::Player(1))));
            g.act(player, Move::Choose(Answer::Holders(vec![Target::Player(1)]))).unwrap();
            finish_combat(&mut g);
            cite!(["701.34a"]; assert_eq!(poison(&g, 1), 2));
            return;
        }
        pass(&mut g);
    }
    panic!("no proliferate question")
}
#[test]
fn pinned_chorus_creates_a_toxic_mite_that_cannot_block() {
    let mut g = game(vec![pinned("Crawling Chorus"), def("Dealer", "Creature", "", Some((2, 2))), pinned("Plague Nurse"), def("Mirror", "Instant", "Create a token that's a copy of target creature.", None)], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let dealer = g.put(1, 1, Zone::Battlefield);
    g.damage(dealer, source.into(), 2);
    g.state_based_actions();
    for _ in 0..12 {
        if g.battlefield.iter().any(|&id| g.obj(id).token() && g.controller(id) == 0) { break; }
        pass(&mut g);
    }
    resolve(&mut g);
    let mite = *g.battlefield.iter().find(|&&id| g.obj(id).token() && g.controller(id) == 0).unwrap();
    assert_eq!(g.seen(mite, None).total_toxic_value(), 1);
    let mut copied = g.clone();
    let spell = copied.put(0, 3, Zone::Hand);
    copied.act(0, Move::Cast { card:spell, targets:vec![vec![mite.into()]] }).unwrap();
    resolve(&mut copied);
    let new_mite = *copied.battlefield.iter().find(|&&id| id != mite && copied.obj(id).token() && copied.controller(id) == 0).unwrap();
    cite!(["707.2"]; assert_eq!(copied.seen(new_mite, None).total_toxic_value(), 1));
    attack(&mut copied, vec![(new_mite, Target::Player(1))], vec![]);
    finish_combat(&mut copied);
    cite!(["702.164c"]; assert_eq!((copied.players[1].life, poison(&copied, 1)), (19, 1)));
    let mut granted = g.clone();
    let nurse = granted.put(0, 2, Zone::Battlefield);
    attack(&mut granted, vec![(mite, Target::Player(1))], vec![]);
    granted.players[0].pool.colors = [8; 5];
    granted.act(0, crate::support::paid("GGG",Move::Activate { source:nurse, ability:1, targets:vec![], paying:vec![] })).unwrap();
    resolve(&mut granted);
    cite!(["702.164b"]; assert_eq!(granted.seen(mite, None).total_toxic_value(), 2));
    finish_combat(&mut granted);
    cite!(["702.164c"]; assert_eq!((granted.players[1].life, poison(&granted, 1)), (19, 2)));
    let mut defense = g.clone();
    // another creature that could block, so that the defending player is asked (509.1a)
    defense.put(0, 1, Zone::Battlefield);
    defense.active = 1;
    defense.priority = 1;
    defense.turn = 2;
    defense.part = ir::Part::BeginningOfCombat;
    for _ in 0..10 {
        if matches!(defense.decision, Some(Decision::Attackers { .. })) { break; }
        pass(&mut defense);
    }
    defense.act(1, Move::Attack(vec![(dealer, Target::Player(0))])).unwrap();
    for _ in 0..10 {
        if matches!(defense.decision, Some(Decision::Blockers { .. })) { break; }
        pass(&mut defense);
    }
    assert!(matches!(defense.decision, Some(Decision::Blockers { player: 0 })), "{:?}", defense.decision);
    cite!(["509.1b"]; refused("Phyrexian Mite Token can't block Dealer: Phyrexian Mite Token says it can't, or not that way (509.1b)"): defense.act(0, Move::Block(vec![(mite, dealer)])));
    attack(&mut g, vec![(mite, Target::Player(1))], vec![]);
    finish_combat(&mut g);
    assert_eq!((g.players[1].life, poison(&g, 1)), (19, 1));
}
#[test]
fn fixed_parameter_admission_is_shared_by_printed_and_granted_keywords() {
    for text in ["Toxic 0", "Toxic 3", "Toxic 1\nToxic 1", "{0}: This creature gains toxic 1 until end of turn."] {
        def("Good", "Creature", text, Some((1, 1)));
    }
    for text in ["Toxic X", "{0}: This creature gains toxic X until end of turn."] {
        let d = CardDef::new("Bad", "{0}", "Creature", text, Some((1, 1))).unwrap();
        crate::support::refused(game::play::check(&d).map_err(|e|e.0), "keyword: Toxic");
    }
    let d = CardDef::new("Bad", "{0}", "Instant", "Toxic 1", None).unwrap();
    crate::support::refused(game::play::check(&d).map_err(|e|e.0), "keyword: Toxic");
}

#[test]
fn trample_only_adds_toxic_when_positive_damage_reaches_the_player() {
    for reach_player in [false, true] {
        let mut g = game(vec![pinned("Bloated Contaminator"), def("Bear", "Creature", "", Some((2, 2)))], 2, 20);
        let source = g.put(0, 0, Zone::Battlefield);
        let bear = g.put(1, 1, Zone::Battlefield);
        attack(&mut g, vec![(source, Target::Player(1))], vec![(bear, source)]);
        for _ in 0..10 {
            if matches!(g.decision, Some(Decision::Divide { .. })) { break; }
            pass(&mut g);
        }
        g.act(0, Move::Divide(if reach_player { vec![(bear.into(), 2), (Target::Player(1), 2)] } else { vec![(bear.into(), 4)] })).unwrap();
        for _ in 0..40 {
            if g.part == ir::Part::EndOfCombat && g.stack.is_empty() { break; }
            if let Some(Decision::Choose { player, choice:Choice::Holders { .. } }) = g.decision {
                g.act(player, Move::Choose(Answer::Holders(vec![]))).unwrap();
            } else { pass(&mut g); }
        }
        cite!(["702.164c", "702.19b"]; assert_eq!((g.players[1].life, poison(&g, 1)), if reach_player { (18, 1) } else { (20, 0) }));
        assert_eq!(g.obj(bear).zone, Zone::Gone);
        cite!(["120.3e"]; assert!(g.counters(bear).is_empty(), "ordinary creature damage, not wither"));
    }
}
#[test]
fn first_strike_toxic_has_one_step_and_toxic_zero_has_no_poison_result() {
    for value in [0, 2] {
        let mut g = game(vec![def("Scout", "Creature", &format!("First strike\nToxic {value}"), Some((3, 3)))], 2, 20);
        let source = g.put(0, 0, Zone::Battlefield);
        attack(&mut g, vec![(source, Target::Player(1))], vec![]);
        finish_combat(&mut g);
        cite!(["702.164c"]; assert_eq!((g.players[1].life, poison(&g, 1)), (17, value)));
        cite!(["702.7b"]; assert_eq!(g.log.iter().filter(|e| matches!(e, Event::DamageDealt { source:s, .. } if *s == source)).count(), 1));
    }
}
#[test]
fn copying_printed_toxic_keeps_parameters_but_not_temporary_grants() {
    let mut g = game(vec![pinned("Bilious Skulldweller"), pinned("Plague Nurse"), def("Copy", "Instant", "Create a token that's a copy of target creature.", None)], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let nurse = g.put(0, 1, Zone::Battlefield);
    g.players[0].pool.colors = [8; 5];
    g.act(0, crate::support::paid("GGG",Move::Activate { source:nurse, ability:1, targets:vec![], paying:vec![] })).unwrap();
    resolve(&mut g);
    cite!(["702.164b"]; assert_eq!(g.seen(source, None).total_toxic_value(), 2));
    let card = g.put(0, 2, Zone::Hand);
    g.act(0, Move::Cast { card, targets:vec![vec![source.into()]] }).unwrap();
    resolve(&mut g);
    let copy = *g.battlefield.iter().find(|&&id| g.obj(id).token() && g.controller(id) == 0).unwrap();
    cite!(["707.2"]; assert_eq!(g.seen(copy, None).total_toxic_value(), 1));
}

#[test]
fn numeric_only_tokens_and_their_copies_preserve_all_toxic_instances() {
    let d = CardDef::new("Unsupported", "{0}", "Sorcery", "Create a 1/1 colorless Mite creature token with toxic X.", None).unwrap();
    assert_eq!(game::play::check(&d).unwrap_err().0, "token: with abilities: keyword: Toxic");
    let mut g = game(vec![
        def("Maker", "Sorcery", "Create a 1/1 colorless Mite creature token with toxic 1, toxic 1, and toxic 2.", None),
        def("Mirror", "Instant", "Create a token that's a copy of target creature.", None),
    ], 2, 20);
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets:vec![] }).unwrap();
    resolve(&mut g);
    let original = *g.battlefield.iter().find(|&&id| g.obj(id).token() && g.controller(id) == 0).unwrap();
    cite!(["702.164b"]; assert_eq!(g.seen(original, None).total_toxic_value(), 4));
    let card = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card, targets:vec![vec![original.into()]] }).unwrap();
    resolve(&mut g);
    let copy = *g.battlefield.iter().find(|&&id| id != original && g.obj(id).token() && g.controller(id) == 0).unwrap();
    cite!(["707.2"]; assert_eq!(g.seen(copy, None).total_toxic_value(), 4));
    attack(&mut g, vec![(original, Target::Player(1)), (copy, Target::Player(1))], vec![]);
    finish_combat(&mut g);
    cite!(["702.164c"]; assert_eq!((g.players[1].life, poison(&g, 1)), (18, 8)));
    let placements: Vec<_> = g.log.iter().filter_map(|e| match e {
        Event::CountersPut { on:Target::Player(1), kind, count, .. } if kind == "poison" => Some(*count),
        _ => None,
    }).collect();
    assert_eq!(placements, [8]);
}
