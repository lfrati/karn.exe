//! Target restrictions are separate from what a selector describes (115, 702.11, 702.18).
use crate::support::{cite, paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};

fn game() -> Game {
    let defs = vec![
        CardDef::new("Hex Bear", "{0}", "Creature — Bear", "Hexproof", Some((2, 2))).unwrap(),
        CardDef::new("Shroud Bear", "{0}", "Creature — Bear", "Shroud", Some((2, 2))).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Bolt", "{0}", "Instant", "Bolt deals 2 damage to target creature.", None).unwrap(),
        CardDef::new("Shield", "{0}", "Instant", "Target creature gains hexproof until end of turn.", None).unwrap(),
        CardDef::new("Sweep", "{0}", "Sorcery", "Destroy all creatures.", None).unwrap(),
        CardDef::new("Double", "{0}", "Instant", "Destroy target creature. Destroy target creature.", None).unwrap(),
        CardDef::new("Kill and Draw", "{0}", "Instant", "Destroy target creature. Draw a card.", None).unwrap(),
        CardDef::new("Source", "{0}", "Artifact", "Sacrifice this artifact: Destroy target creature.", None).unwrap(),
        CardDef::new("Aura", "{0}", "Enchantment — Aura", "Enchant creature\nEnchanted creature gets +1/+1.", None).unwrap(),
        CardDef::new("Return", "{0}", "Instant", "Put target enchantment card from your graveyard onto the battlefield.", None).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Arrival", "{0}", "Enchantment", "When this enchantment enters, destroy target creature you don't control.", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0)); }
    Game::new(defs, 3, 20)
}

fn cast(g: &mut Game, player: usize, def: usize, targets: Vec<Vec<Target>>) -> ObjId {
    let card = g.put(player, def, Zone::Hand);
    g.act(player, Move::Cast { card, targets }).unwrap();
    *g.stack.last().unwrap()
}

fn resolve(g: &mut Game) { for _ in 0..3 { g.act(g.priority, Move::Pass).unwrap(); } }

fn creature(o: ObjId) -> Vec<Vec<Target>> { vec![vec![Target::Object(o)]] }

#[test]
fn r702_11b_hexproof_uses_the_spells_controller_in_multiplayer_and_preserves_own_targets() {
    let mut g = game();
    let protected = g.put(0, 0, Zone::Battlefield);
    for player in 0..3 {
        while g.priority != player { g.act(g.priority, Move::Pass).unwrap(); }
        let card = g.put(player, 3, Zone::Hand);
        if player != 0 {
            let before = format!("{g:?}");
            cite!(["702.11b"]; refused("an illegal target, Hex Bear: it has hexproof, and an opponent of its controller is targeting it (702.11b)"): g.act(player, Move::Cast { card, targets: creature(protected) }));
            assert_eq!(format!("{g:?}"), before);
        } else {
            cite!(["702.11b"]; ok: g.clone().act(player, Move::Cast { card, targets: creature(protected) }));
        }
    }
}

#[test]
fn r702_18a_shroud_prevents_even_its_controllers_targeted_spell() {
    let mut g = game();
    let protected = g.put(0, 1, Zone::Battlefield);
    for player in 0..3 {
        while g.priority != player { g.act(g.priority, Move::Pass).unwrap(); }
        let card = g.put(player, 3, Zone::Hand);
        cite!(["702.18a"]; refused("an illegal target, Shroud Bear: it has shroud (702.18a)"): g.act(player, Move::Cast { card, targets: creature(protected) }));
    }
    assert_eq!(g.obj(protected).zone, Zone::Battlefield);
}

#[test]
fn r115_10_a_nontargeted_effect_still_affects_hexproof_and_shroud_permanents() {
    let mut g = game();
    let hex = g.put(1, 0, Zone::Battlefield);
    let shroud = g.put(2, 1, Zone::Battlefield);
    cast(&mut g, 0, 5, vec![]);
    resolve(&mut g);
    cite!(["115.10a"]; assert_eq!((g.obj(hex).zone, g.obj(shroud).zone), (Zone::Gone, Zone::Gone)));
}

#[test]
fn r608_2b_gaining_hexproof_can_make_every_target_illegal_and_prevent_the_draw() {
    let mut g = game();
    let bear = g.put(0, 2, Zone::Battlefield);
    g.put(1, 11, Zone::Library);
    g.act(0, Move::Pass).unwrap();
    let kill = cast(&mut g, 1, 7, creature(bear));
    while g.priority != 0 { g.act(g.priority, Move::Pass).unwrap(); }
    cast(&mut g, 0, 4, creature(bear));
    resolve(&mut g);
    assert!(g.has(bear, "Hexproof"));
    resolve(&mut g);
    assert_eq!(g.obj(bear).zone, Zone::Battlefield);
    cite!(["608.2b"]; assert!(g.players[1].hand.is_empty()));
    cite!(["608.2b"]; assert!(g.log.iter().any(|e| matches!(e, Event::Fizzled { spell } if *spell == kill))));
}

#[test]
fn r608_2b_a_partly_illegal_spell_leaves_the_protected_target_alone() {
    let mut g = game();
    let protected = g.put(0, 2, Zone::Battlefield);
    let other = g.put(2, 2, Zone::Battlefield);
    g.act(0, Move::Pass).unwrap();
    cast(&mut g, 1, 6, vec![vec![Target::Object(protected)], vec![Target::Object(other)]]);
    while g.priority != 0 { g.act(g.priority, Move::Pass).unwrap(); }
    cast(&mut g, 0, 4, creature(protected));
    resolve(&mut g);
    resolve(&mut g);
    cite!(["608.2b"]; assert_eq!((g.obj(protected).zone, g.obj(other).zone), (Zone::Battlefield, Zone::Gone)));
}

#[test]
fn r702_11b_activated_abilities_use_their_controller_even_after_the_source_is_sacrificed() {
    let mut g = game();
    let protected = g.put(0, 0, Zone::Battlefield);
    let own_source = g.put(1, 8, Zone::Battlefield);
    let steal = CardDef::new("Steal", "{0}", "Instant", "Gain control of target artifact.", None).unwrap();
    game::play::check(&steal).unwrap();
    std::sync::Arc::make_mut(&mut g.defs).push(steal);
    cast(&mut g, 0, 13, creature(own_source));
    resolve(&mut g);
    assert_eq!((g.obj(own_source).owner, g.controller(own_source)), (1, 0));
    let enemy_source = g.put(1, 8, Zone::Battlefield);
    g.act(0, Move::Pass).unwrap();
    let before = format!("{g:?}");
    cite!(["702.11b"]; refused("an illegal target, Hex Bear: it has hexproof, and an opponent of its controller is targeting it (702.11b)"): g.act(1, Move::Activate { source: enemy_source, ability: 0, targets: creature(protected), paying: vec![] }));
    assert_eq!(format!("{g:?}"), before);
    while g.priority != 0 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["702.11b"]; ok: g.act(0, Move::Activate { source: own_source, ability: 0, targets: creature(protected), paying: vec![] }));
    assert_eq!(g.obj(own_source).zone, Zone::Gone);
    resolve(&mut g);
    cite!(["113.7a"]; assert_eq!(g.obj(protected).zone, Zone::Gone));
}

#[test]
fn r603_3d_a_trigger_without_an_unprotected_legal_target_is_removed() {
    for protected in [false, true] {
        let mut g = game();
        let bear = g.put(1, if protected { 0 } else { 2 }, Zone::Battlefield);
        cast(&mut g, 0, 12, vec![]);
        resolve(&mut g);
        if !protected {
            assert!(matches!(g.decision, Some(Decision::Targets { player: 0, .. })));
            g.act(0, Move::ChooseTargets(creature(bear))).unwrap();
            resolve(&mut g);
        }
        assert!(g.stack.is_empty());
        assert!(g.decision.is_none());
        cite!(["603.3d"]; case: format!("protected={protected}"); assert_eq!(g.obj(bear).zone, if protected { Zone::Battlefield } else { Zone::Gone }));
    }
}

#[test]
fn r303_4f_an_aura_entering_without_casting_can_attach_to_a_shrouded_creature() {
    let mut g = game();
    let bear = g.put(1, 1, Zone::Battlefield);
    let aura = g.put(0, 9, Zone::Hand);
    cite!(["702.18a"]; refused("an illegal target, Shroud Bear: it has shroud (702.18a)"): g.act(0, Move::Cast { card: aura, targets: creature(bear) }));
    let aura = g.put(0, 9, Zone::Graveyard);
    cast(&mut g, 0, 10, creature(aura));
    resolve(&mut g);
    if let Some(Decision::Choose { player, choice: Choice::Attach { among, .. } }) = g.decision.clone() {
        cite!(["303.4f"]; assert!(among.contains(&Target::Object(bear))));
        g.act(player, Move::Choose(Answer::Attach(Target::Object(bear)))).unwrap();
    }
    cite!(["303.4f"]; assert_eq!(g.power(bear), Some(3)));
}

#[test]
fn r611_3b_a_targeting_prohibition_is_a_shared_statement_that_ends_with_its_source() {
    let mut g = game();
    let ward = CardDef::new("Ward", "{0}", "Enchantment", "Creatures you control can't be the target of spells or abilities your opponents control.", None).unwrap();
    game::play::check(&ward).unwrap();
    std::sync::Arc::make_mut(&mut g.defs).push(ward);
    let source = g.put(0, 13, Zone::Battlefield);
    let mine = g.put(0, 2, Zone::Battlefield);
    let theirs = g.put(1, 2, Zone::Battlefield);
    assert!(g.can_target(&game::select::Ctx::new(0, mine), Target::Object(mine)));
    cite!(["611.3b"]; assert!(!g.can_target(&game::select::Ctx::new(1, mine), Target::Object(mine))));
    assert!(g.can_target(&game::select::Ctx::new(0, theirs), Target::Object(theirs)));
    g.destroy(source);
    cite!(["611.3b"]; assert!(g.can_target(&game::select::Ctx::new(1, mine), Target::Object(mine))));
}

#[test]
fn r113_6_hexproof_and_shroud_do_not_protect_creature_spells_or_graveyard_cards() {
    for def in [0, 1] {
        let mut g = game();
        std::sync::Arc::make_mut(&mut g.defs).extend([
            CardDef::new("Denial", "{0}", "Instant", "Counter target spell.", None).unwrap(),
            CardDef::new("Restore", "{0}", "Instant", "Put target creature card from a graveyard onto the battlefield under your control.", None).unwrap(),
        ]);
        let spell = cast(&mut g, 0, def, vec![]);
        g.act(0, Move::Pass).unwrap();
        cast(&mut g, 1, 13, creature(spell));
        resolve(&mut g);
        let card = g.players[0].graveyard[0];
        g.act(0, Move::Pass).unwrap();
        cast(&mut g, 1, 14, creature(card));
        resolve(&mut g);
        cite!(["113.6"]; case: format!("def {def}"); assert_eq!(g.battlefield.len(), 1));
        assert_eq!(g.controller(g.battlefield[0]), 1);
        assert_eq!(g.obj(g.battlefield[0]).owner, 0);
    }
}

#[test]
fn r514_2_granted_hexproof_stops_prohibiting_targets_when_its_duration_ends() {
    let mut g = game();
    let bear = g.put(0, 2, Zone::Battlefield);
    cast(&mut g, 0, 4, creature(bear));
    resolve(&mut g);
    cite!(["702.11b"]; assert!(!g.can_target(&game::select::Ctx::new(1, bear), Target::Object(bear))));
    for _ in 0..300 {
        if g.turn > 1 { break; }
        match g.decision {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    assert_eq!(g.turn, 2);
    cite!(["514.2"]; assert!(!g.has(bear, "Hexproof")));
    cite!(["514.2"]; assert!(g.can_target(&game::select::Ctx::new(1, bear), Target::Object(bear))));
}

#[test]
fn r702_11d_quality_specific_target_prohibitions_remain_refused_until_sources_are_modeled() {
    for text in [
        "Hexproof from red",
        "This creature can't be the target of blue spells or abilities your opponents control.",
        "This creature can't be the target of players.",
    ] {
        if text.ends_with("players.") {
            assert_eq!(CardDef::new("Unsupported","{0}","Creature",text,Some((2,2))).expect_err("a targeting source is a spell or ability"),"text the grammar does not read");
        } else {
            let d=CardDef::new("Unsupported","{0}","Creature",text,Some((2,2))).unwrap();
            let reason=if text.starts_with("Hexproof") {"keyword: Hexproof"}else{"target prohibition: qualities of abilities or their sources"};
            crate::support::refused(game::play::check(&d).map_err(|e|e.0),reason);
        }
    }
}

#[test]
fn r702_11b_thruns_hexproof_composes_with_regeneration_against_a_nontargeted_sweep() {
    let mut g = game();
    let thrun = CardDef::new("Thrun, the Last Troll", "{2}{G}{G}", "Legendary Creature — Troll Shaman", "This spell can't be countered.\nHexproof\n{1}{G}: Regenerate this creature.", Some((4, 4))).unwrap();
    game::play::check(&thrun).unwrap();
    std::sync::Arc::make_mut(&mut g.defs).push(thrun);
    let thrun = g.put(0, 13, Zone::Battlefield);
    cite!(["702.11b"]; assert!(!g.can_target(&game::select::Ctx::new(1, thrun), Target::Object(thrun))));
    g.players[0].pool.colors[ir::Color::Green as usize] = 1;
    g.players[0].pool.colorless = 1;
    g.act(0, paid("CG", Move::Activate { source: thrun, ability: 2, targets: vec![], paying: vec![] })).unwrap();
    resolve(&mut g);
    cast(&mut g, 0, 5, vec![]);
    resolve(&mut g);
    cite!(["701.19a"]; assert_eq!(g.obj(thrun).zone, Zone::Battlefield));
    cite!(["701.19a"]; assert!(g.obj(thrun).tapped));
    cast(&mut g, 0, 5, vec![]);
    resolve(&mut g);
    cite!(["701.19a"]; assert_eq!(g.obj(thrun).zone, Zone::Gone, "the regeneration shield was consumed"));
}
