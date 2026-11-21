//! Mandatory explicit creature targets: CR 115.1a/d, 601.2c, 603.3d,
//! 608.2b/c/d/h and 400.7. Native Oracle/rulings: selection-next-contract/primary.json.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Zone};
use game::state::Decision;
fn native(names: &[&str]) -> Vec<CardDef> {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    names.iter().map(|name| CardDef::from_faces(&catalog[name]).unwrap()).collect()
}
fn resolve(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }

fn prefix(restriction: &str) -> CardDef {
    CardDef::new("Prefix", "{0}", "Instant", &format!("Choose target creature{restriction}. Tap that creature. Put a stun counter on it."), None).unwrap()
}
fn bear() -> CardDef { CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((3, 6))).unwrap() }
fn spell(g: &mut Game, def: usize, targets: Vec<Vec<game::Target>>, spending: &str) -> Result<(), String> {
    let card = g.put(0, def, Zone::Hand);
    g.act(0, paid(spending,Move::Cast { card, targets }))
}

#[test]
fn r601_singleton_spell_count_domain_seat_and_lexical_binding() {
    for (restriction, controller) in [("", 0), ("", 1), (" you control", 0), (" an opponent controls", 1)] {
        let land = native(&["Island"]).remove(0);
        let mut g = Game::new(vec![prefix(restriction), bear(), land], 2, 20);
        let own = g.put(0, 1, Zone::Battlefield);
        let theirs = g.put(1, 1, Zone::Battlefield);
        let land = g.put(0, 2, Zone::Battlefield);
        let selected = if controller == 0 { own } else { theirs };
        let card = g.put(0, 0, Zone::Hand);
        for (seat, targets, reason) in [(0, vec![], "that has 1 target group(s), 0 given"), (0, vec![vec![]], "wrong number of targets: 0 chosen, it takes 1"), (0, vec![vec![own.into(), theirs.into()]], "wrong number of targets: 2 chosen, it takes 1"),
            (0, vec![vec![selected.into()], vec![selected.into()]], "that has 1 target group(s), 2 given"), (0, vec![vec![land.into()]], "an illegal target, Island"),
            (0, vec![vec![game::Target::Player(0)]], "an illegal target, you"), (1, vec![vec![selected.into()]], "that player does not have priority")] {
            let before = format!("{g:?}");
            cite!(["601.2c", "115.1a"]; refused(reason): g.act(seat, Move::Cast { card, targets }));
            assert_eq!(format!("{g:?}"), before);
        }
        if !restriction.is_empty() {
            let wrong = if controller == 0 { theirs } else { own };
            cite!(["601.2c"]; refused("an illegal target, Bear"): act_unchanged(&mut g,Move::Cast { card, targets: vec![vec![wrong.into()]] }));
        }
        g.act(0, Move::Cast { card, targets: vec![vec![selected.into()]] }).unwrap();
        cite!(["601.2c"]; assert!(g.decision.is_none() && !g.obj(selected).tapped));
        resolve(&mut g);
        cite!(["608.2c", "608.2d"]; assert!(g.stack.is_empty() && g.decision.is_none() && g.resolving().is_none()));
        cite!(["608.2c"]; assert!(g.obj(selected).tapped && g.obj(selected).counters.get("stun") == Some(&1)));
        let other = if controller == 0 { theirs } else { own };
        cite!(["608.2c"]; assert!(!g.obj(other).tapped && g.obj(other).counters.is_empty()));
    }
}

#[test]
fn r608_singleton_zone_and_control_changes_recheck_without_substitution() {
    let steal = CardDef::new("Control", "{0}", "Instant", "Gain control of target creature.", None).unwrap();
    let bounce = native(&["Unsummon"]).remove(0);
    for restriction in ["", " you control", " an opponent controls"] {
        for zone_change in [false, true] {
            let mut g = Game::new(vec![prefix(restriction), bear(), steal.clone(), bounce.clone()], 2, 20);
            let controller = if restriction == " you control" { 0 } else { 1 };
            let target = g.put(controller, 1, Zone::Battlefield);
            let other = g.put(controller, 1, Zone::Battlefield);
            spell(&mut g, 0, vec![vec![target.into()]], "").unwrap();
            let response = g.put(1-controller, if zone_change { 3 } else { 2 }, Zone::Hand);
            g.priority = 1-controller;
            g.players[1-controller].pool.colors[1] = 1;
            g.act(1-controller, paid(if zone_change { "U" } else { "" }, Move::Cast { card: response, targets: vec![vec![target.into()]] })).unwrap();
            resolve(&mut g);
            if zone_change {
                let returned = g.players[controller].hand[0];
                cite!(["400.7"]; assert_ne!(returned, target));
                // A fresh incarnation of the same card cannot inherit the target.
                let replacement = g.put(controller, 1, Zone::Battlefield);
                resolve(&mut g);
                cite!(["608.2b", "400.7"]; assert!(!g.obj(replacement).tapped && g.obj(replacement).counters.is_empty()));
            } else {
                resolve(&mut g);
                cite!(["608.2b"]; assert_eq!(g.obj(target).tapped, restriction.is_empty()));
                cite!(["608.2b"]; assert_eq!(g.obj(target).counters.get("stun").copied(), if restriction.is_empty() { Some(1) } else { None }));
            }
            cite!(["608.2b", "608.2d"]; assert!(!g.obj(other).tapped && g.decision.is_none() && g.stack.is_empty()));
        }
    }
}

#[test]
fn r603_mandatory_no_target_trigger_removed_spell_refuses_before_payment() {
    let trigger = CardDef::new("Speaker", "{0}", "Creature — Bear", "When this creature enters, choose target creature an opponent controls. Tap that creature.", Some((2, 2))).unwrap();
    let mut g = Game::new(vec![trigger, prefix(" you control")], 2, 20);
    let card = g.put(0, 1, Zone::Hand);
    let before = format!("{g:?}");
    cite!(["601.2c"]; refused("there is no legal target for it"): g.act(0, Move::Cast { card, targets: vec![vec![]] }));
    assert_eq!(format!("{g:?}"), before);
    spell(&mut g, 0, vec![], "").unwrap();
    resolve(&mut g);
    cite!(["603.3d"]; assert!(g.stack.is_empty() && g.decision.is_none()));
    cite!(["603.3d"]; assert_eq!(g.battlefield.len(), 1));
}

fn shackle() -> (Game, game::ObjId, game::ObjId) {
    let mut g = Game::new(native(&["Shackle Slinger", "Llanowar Elves", "Island"]), 2, 20);
    for p in 0..2 { g.put(p, 2, Zone::Library); }
    let target = g.put(1, 1, Zone::Battlefield);
    let other = g.put(1, 1, Zone::Battlefield);
    g.players[0].pool.colors = [10; 5];
    spell(&mut g, 0, vec![], "UUU").unwrap(); resolve(&mut g);
    // Native ruling cdb3e839f2214665: Slinger itself was the first spell;
    // it was not yet on the battlefield when that spell was cast.
    spell(&mut g, 1, vec![], "G").unwrap();
    (g, target, other)
}
#[test]
fn r603_native_shackle_history_singleton_and_live_tappedness() {
    for (announced_tapped, resolved_tapped) in [(false, false), (true, true), (false, true), (true, false)] {
        let (mut g, target, other) = shackle();
        g.set_tapped(target, announced_tapped);
        cite!(["115.1d", "603.3d"]; assert!(matches!(g.decision, Some(Decision::Targets { player: 0, .. }))));
        let source = g.battlefield.iter().copied().find(|o| g.obj(*o).chars.name == "Shackle Slinger").unwrap();
        for (seat, targets, reason) in [(1, vec![vec![target.into()]], "that decision is another player's"), (0, vec![vec![]], "wrong number of targets: 0 chosen, it takes 1"),
            (0, vec![vec![target.into(), other.into()]], "wrong number of targets: 2 chosen, it takes 1"), (0, vec![vec![source.into()]], "an illegal target, Shackle Slinger")] {
            let before = format!("{g:?}");
            cite!(["601.2c", "603.3d"]; refused(reason): g.act(seat, Move::ChooseTargets(targets)));
            assert_eq!(format!("{g:?}"), before);
        }
        g.act(0, Move::ChooseTargets(vec![vec![target.into()]])).unwrap();
        // Explicit state transition isolates the resolution-time condition.
        g.set_tapped(target, resolved_tapped);
        resolve(&mut g);
        cite!(["608.2c", "608.2h"]; assert!(g.obj(target).tapped));
        cite!(["608.2h"]; assert_eq!(g.obj(target).counters.get("stun").copied(), if resolved_tapped { Some(1) } else { None }));
        cite!(["608.2c", "608.2d"]; assert!(!g.obj(other).tapped && g.obj(other).counters.is_empty() && g.decision.is_none()));
        resolve(&mut g); // second spell itself, after the triggered ability
        assert!(g.stack.is_empty());
    }
    let mut g = Game::new(native(&["Shackle Slinger", "Llanowar Elves", "Island"]), 2, 20);
    g.put(1, 1, Zone::Battlefield); g.players[0].pool.colors = [10; 5];
    spell(&mut g, 1, vec![], "G").unwrap(); resolve(&mut g);
    spell(&mut g, 0, vec![], "UUU").unwrap(); resolve(&mut g);
    spell(&mut g, 1, vec![], "G").unwrap();
    // Two spells before entry means this is the third, not Slinger's second.
    cite!(["603.2"]; assert!(g.decision.is_none() && g.stack.len() == 1));
}

#[test]
fn r608_native_recruiter_unrestricted_high_power_and_live_condition() {
    // Full ruling 6a7a0586eecbc581: power is not announcement eligibility.
    for (power, final_power, extra_lizard, remove_lizard) in [(3, 3, false, false), (1, 4, false, false),
        (3, 1, false, false), (3, 3, true, false), (3, 3, true, true)] {
        let mut defs = native(&["Reptilian Recruiter", "Llanowar Elves", "Shackle Slinger", "Island", "Unsummon"]);
        defs.push(CardDef::new("Shrink", "{0}", "Instant", "Target creature gets -2/-0 until end of turn.", None).unwrap());
        let mut g = Game::new(defs, 2, 20);
        for p in 0..2 { g.put(p, 3, Zone::Library); }
        let target = g.put(1, if power == 1 { 1 } else { 2 }, Zone::Battlefield);
        let other = g.put(1, 1, Zone::Battlefield);
        let lizard = extra_lizard.then(|| g.put(0, 0, Zone::Battlefield));
        g.set_tapped(target, true); g.players[0].pool.colors = [10; 5];
        let card = g.put(0, 0, Zone::Hand);
        g.act(0, paid("RRRRR", Move::Cast { card, targets: vec![] })).unwrap(); resolve(&mut g);
        cite!(["603.3d"]; assert!(matches!(g.decision, Some(Decision::Targets { player: 0, .. }))));
        g.act(0, Move::ChooseTargets(vec![vec![target.into()]])).unwrap();
        if final_power > power { g.put_counters(target, "+1/+1", (final_power-power) as u32); }
        if final_power < power { spell(&mut g, 5, vec![vec![target.into()]], "").unwrap(); resolve(&mut g); }
        if remove_lizard {
            let card = g.put(0, 4, Zone::Hand);
            g.act(0, paid("U", Move::Cast { card, targets: vec![vec![lizard.unwrap().into()]] })).unwrap(); resolve(&mut g);
        }
        resolve(&mut g);
        let succeeds = final_power <= 2 || (extra_lizard && !remove_lizard);
        cite!(["608.2h", "608.2c"]; assert_eq!(g.controller(target), if succeeds { 0 } else { 1 }));
        cite!(["608.2c"]; assert_eq!(g.obj(target).tapped, !succeeds));
        cite!(["608.2c"]; assert_eq!(g.has(target, "Haste"), succeeds));
        cite!(["608.2d"]; assert!(g.stack.is_empty() && g.decision.is_none()));
        assert_eq!(g.controller(other), 1); assert!(!g.has(other, "Haste"));
    }
}

#[test]
fn r608_native_nibelheim_current_power_creature_source_and_graveyard_continuation() {
    for (graveyard, illegal) in [(false, false), (true, false), (true, true)] {
        let mut defs = native(&["Nibelheim Aflame", "Llanowar Elves", "Island", "Giant Growth", "Unsummon"]);
        defs.push(bear());
        let mut g = Game::new(defs, 2, 20);
        let target = g.put(0, 1, Zone::Battlefield);
        let own_other = g.put(0, 5, Zone::Battlefield);
        let opposing = g.put(1, 5, Zone::Battlefield);
        for _ in 0..4 { g.put(0, 2, Zone::Library); }
        let held = g.put(0, 2, Zone::Hand);
        let card = g.put(0, 0, if graveyard { Zone::Graveyard } else { Zone::Hand });
        g.players[0].pool.colors = [20; 5];
        let before = format!("{g:?}");
        cite!(["601.2c"]; refused("an illegal target, Bear"): g.act(0,paid(if graveyard {"RRRRRRR"}else{"RRRR"},if graveyard { Move::CastVia(game::announcement::CastRoute::Flashback,Box::new(Move::Cast {card,targets:vec![vec![opposing.into()]]})) } else {Move::Cast {card,targets:vec![vec![opposing.into()]]}})));
        assert_eq!(format!("{g:?}"), before);
        g.act(0, paid(if graveyard {"RRRRRRR"}else{"RRRR"},if graveyard { Move::CastVia(game::announcement::CastRoute::Flashback,Box::new(Move::Cast {card,targets:vec![vec![target.into()]]})) } else {Move::Cast {card,targets:vec![vec![target.into()]]}})).unwrap();
        // Native instant response changes the relevant power or incarnation.
        spell(&mut g, if illegal { 4 } else { 3 }, vec![vec![target.into()]], if illegal {"U"} else {"G"}).unwrap(); resolve(&mut g);
        resolve(&mut g);
        let damage: Vec<_> = g.log.iter().filter_map(|e| match e {
            game::Event::DamageDealt { source, to, amount, .. } => Some((*source, *to, *amount)), _ => None,
        }).collect();
        if illegal {
            cite!(["608.2b", "400.7"]; assert!(damage.is_empty()));
            cite!(["608.2b"]; assert!(g.players[0].hand.contains(&held) && g.players[0].library.len() == 4));
            assert_eq!(g.players[0].hand.len(), 2); // held Island and returned Elf
        } else {
            cite!(["608.2h"]; assert_eq!(damage, vec![(target, own_other.into(), 4), (target, opposing.into(), 4)]));
            cite!(["608.2c"]; assert!(g.battlefield.contains(&target) && g.obj(target).damage == 0));
            if graveyard {
                cite!(["608.2c"]; assert_eq!(g.players[0].hand.len(), 4));
                assert_eq!(g.players[0].library.len(), 0); assert!(!g.players[0].hand.contains(&held));
            } else { assert_eq!(g.players[0].hand, vec![held]); assert_eq!(g.players[0].library.len(), 4); }
        }
        cite!(["608.2b", "608.2d"]; assert!(g.stack.is_empty() && g.decision.is_none()));
        if graveyard { cite!(["702.34a"]; assert_eq!(g.exile.len(), 1)); }
    }
}

#[test]
fn r115_prefix_registers_one_capture_and_one_target_group() {
    use game::program::{self, Instr, action::Operand};
    use game::select::Ctx;
    let d = prefix(" an opponent controls");
    let ir::Ability::Text(effect) = &d.abilities()[0] else { panic!() };
    let mut g = Game::new(vec![d.clone()], 2, 20);
    let source = g.put(0, 0, Zone::Stack);
    let mut ctx = Ctx::new(0, source);
    ctx.ability = Some(game::binding::AbilityKey::printed(0));
    let p = program::compile(effect, &g, &ctx).unwrap();
    cite!(["115.1a", "601.2c"]; assert_eq!(p.targets().len(), 1));
    cite!(["601.2c"]; assert_eq!(p.targets()[0].bounds, (1,1)));
    // Uncited (implementation ownership): precisely one target Capture supplies
    // later lexical references; no choice instruction is emitted by the prefix.
    assert_eq!(p.instrs().iter().filter(|i| matches!(i, Instr::Capture { operand: Operand::Targets(0), .. })).count(), 1);
    assert!(!p.instrs().iter().any(|i| matches!(i, Instr::Choice { .. } | Instr::Select { .. } | Instr::Choose { .. })));
}

#[test]
fn r608_prefix_capture_survives_a_downstream_resolution_suspension() {
    use game::state::Answer;
    use game::program::Value;
    let d = CardDef::new("Retained", "{0}", "Instant", "Choose target creature. You gain 1 life. Choose a color. Tap that creature.", None).unwrap();
    let mut g = Game::new(vec![d, bear()], 2, 20);
    let target = g.put(1, 1, Zone::Battlefield);
    let other = g.put(1, 1, Zone::Battlefield);
    spell(&mut g, 0, vec![vec![target.into()]], "").unwrap(); resolve(&mut g);
    cite!(["608.2c", "608.2d"]; assert!(matches!(g.decision, Some(Decision::Choose { player: 0, .. }))));
    assert_eq!(g.players[0].life, 21); assert!(!g.obj(target).tapped);
    let before = format!("{g:?}");
    cite!(["608.2d"]; refused("a decision is pending: a choice of the effect being applied"): g.act(0, Move::Pass));
    assert_eq!(format!("{g:?}"), before);
    g.act(0, Move::Choose(Answer::Value(Value::Color(ir::Color::Blue)))).unwrap();
    cite!(["608.2c"]; assert_eq!(g.players[0].life, 21));
    cite!(["608.2c", "608.2d"]; assert!(g.obj(target).tapped && !g.obj(other).tapped && g.stack.is_empty() && g.decision.is_none()));
}

#[test]
fn r603_native_no_target_trigger_removed_and_costly_spell_refused() {
    let mut g = Game::new(native(&["Shackle Slinger", "Llanowar Elves", "Nibelheim Aflame"]), 2, 20);
    g.players[0].pool.colors = [10; 5];
    let card = g.put(0, 2, Zone::Hand);
    let before = format!("{g:?}");
    cite!(["601.2c"]; refused("there is no legal target for it"): g.act(0, paid("RRRR",Move::Cast { card, targets: vec![vec![]] })));
    assert_eq!(format!("{g:?}"), before, "no payment or disclosure without a legal target");
    spell(&mut g, 0, vec![], "UUU").unwrap(); resolve(&mut g);
    spell(&mut g, 1, vec![], "G").unwrap();
    cite!(["603.3d"]; assert!(g.decision.is_none() && g.stack.len() == 1));
    resolve(&mut g);
    assert!(g.stack.is_empty());
}

#[track_caller]
fn act_unchanged(g:&mut Game,action:Move)->Result<(),String> {
 let before=format!("{g:?}");let result=g.act(0,action);assert_eq!(format!("{g:?}"),before);result
}
