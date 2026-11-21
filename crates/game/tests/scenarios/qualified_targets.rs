//! A restriction describes the targeting spell, not merely the object that generated an ability.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::{Color, mana::ManaSymbol};

fn def(name: &str, cost: &str, ty: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    CardDef::new(name, cost, ty, text, pt).unwrap()
}
fn game() -> Game {
    let defs = vec![
        def(
            "Fiendslayer Paladin",
            "{1}{W}{W}",
            "Creature — Human Knight",
            "First strike\nLifelink\nThis creature can't be the target of black or red spells your opponents control.",
            Some((2, 2)),
        ),
        def("Scorch", "{R}", "Instant", "Scorch deals 1 damage to target creature.", None),
        def("Darkness", "{B}", "Instant", "Darkness deals 1 damage to target creature.", None),
        def("Chill", "{U}", "Instant", "Chill deals 1 damage to target creature.", None),
        def("Clear", "{0}", "Instant", "Clear deals 1 damage to target creature.", None),
        def("Mixed", "{U}{R}", "Instant", "Mixed deals 1 damage to target creature.", None),
        def(
            "Pinger",
            "{R}",
            "Creature",
            "Sacrifice this creature: Pinger deals 1 damage to target creature.",
            Some((1, 1)),
        ),
        def("Paint", "{0}", "Instant", "Target spell becomes red until end of turn.", None),
        def("Aura", "{U}", "Enchantment — Aura", "Enchant creature\nEnchanted creature gets +1/+1.", None),
        def("Shield", "{0}", "Instant", "Target creature gains hexproof until end of turn.", None),
        def("Bear", "{0}", "Creature", "", Some((2, 2))),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0));
    }
    Game::new(defs, 3, 20)
}
fn mana(g: &mut Game, player: usize) {
    for color in [Color::White, Color::Blue, Color::Black, Color::Red, Color::Green] {
        for _ in 0..5 {
            g.add_mana(player, ManaSymbol::Color(color));
        }
    }
}
fn targets(o: ObjId) -> Vec<Vec<Target>> {
    vec![vec![Target::Object(o)]]
}
fn resolve(g: &mut Game) {
    for _ in 0..3 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn cast(g: &mut Game, player: usize, d: usize, target: ObjId, spending: &str) -> ObjId {
    let card = g.put(player, d, Zone::Hand);
    g.act(player, paid(spending, Move::Cast { card, targets: targets(target) })).unwrap();
    *g.stack.last().unwrap()
}
#[test]
fn r115_1_spell_color_and_controller_restrict_both_offers_and_explicit_targets() {
    for player in 0..3 {
        for (d, spending) in [(1,"R"),(2,"B"),(3,"U"),(4,""),(5,"UR")] {
            let mut g = game();
            let protected = g.put(0, 0, Zone::Battlefield);
            mana(&mut g, player);
            while g.priority != player {
                g.act(g.priority, Move::Pass).unwrap();
            }
            let card = g.put(player, d, Zone::Hand);
            let allowed = player == 0 || matches!(d, 3 | 4);
            let before = format!("{g:?}");
            cite!(["601.2c", "115.1"]; case: format!("player {player}, card {d}"); assert_eq!(g.act(player, paid(spending, Move::Cast { card, targets: targets(protected) })).is_ok(), allowed));
            if allowed {
                resolve(&mut g);
                assert_eq!(g.obj(protected).damage, 1);
            } else {
                assert_eq!(format!("{g:?}"), before);
            }
        }
    }
}
#[test]
fn r113_7_a_red_sources_activated_ability_is_not_a_red_spell() {
    let mut g = game();
    let protected = g.put(0, 0, Zone::Battlefield);
    let source = g.put(1, 6, Zone::Battlefield);
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Activate { source, ability: 0, targets: targets(protected), paying: vec![] }).unwrap();
    assert_eq!(g.obj(source).zone, Zone::Gone);
    resolve(&mut g);
    cite!(["113.7"]; assert_eq!(g.obj(protected).damage, 1));
}
#[test]
fn r608_2b_a_spell_becoming_red_loses_its_protected_target() {
    let mut g = game();
    let protected = g.put(0, 0, Zone::Battlefield);
    mana(&mut g, 1);
    g.act(0, Move::Pass).unwrap();
    let spell = cast(&mut g, 1, 3, protected, "U");
    cast(&mut g, 1, 7, spell, "");
    resolve(&mut g);
    resolve(&mut g);
    cite!(["608.2b"]; assert_eq!(g.obj(protected).damage, 0));
    cite!(["608.2b"]; assert!(g.log.iter().any(|e| matches!(e, game::Event::Fizzled { spell:s } if *s==spell))));
}
#[test]
fn r608_3b_an_aura_spell_rechecks_targeting_restrictions_before_entering() {
    let defs = vec![
        def("Bear", "{0}", "Creature", "", Some((2, 2))),
        def("Aura", "{U}", "Enchantment — Aura", "Enchant creature\nEnchanted creature gets +1/+1.", None),
        def("Shield", "{0}", "Instant", "Target creature gains hexproof until end of turn.", None),
    ];
    for d in &defs {
        game::play::check(d).unwrap();
    }
    let mut g = Game::new(defs, 3, 20);
    let bear = g.put(0, 0, Zone::Battlefield);
    mana(&mut g, 1);
    g.active = 1;
    g.priority = 1;
    let aura = cast(&mut g, 1, 1, bear, "U");
    while g.priority != 0 {
        g.act(g.priority, Move::Pass).unwrap();
    }
    cast(&mut g, 0, 2, bear, "");
    resolve(&mut g);
    resolve(&mut g);
    cite!(["608.3b"]; assert_eq!(g.battlefield, vec![bear]));
    cite!(["608.3b"]; assert!(g.log.iter().any(|e| matches!(e, game::Event::Fizzled { spell:s } if *s==aura))));
}

#[test]
fn r608_3b_an_aura_spell_uses_its_current_color_for_target_legality() {
    let mut g = game();
    let protected = g.put(0, 0, Zone::Battlefield);
    mana(&mut g, 0);
    let aura = cast(&mut g, 0, 8, protected, "U");
    cast(&mut g, 0, 7, aura, "");
    resolve(&mut g);
    resolve(&mut g);
    cite!(["608.3b"]; assert_eq!(g.power(protected), Some(3), "own red spell remains legal"));
    // Another player's red Aura does not resolve onto this creature.
    let mut g = game();
    let protected = g.put(0, 0, Zone::Battlefield);
    g.active = 1;
    g.priority = 1;
    mana(&mut g, 1);
    let aura = cast(&mut g, 1, 8, protected, "U");
    cast(&mut g, 1, 7, aura, "");
    resolve(&mut g);
    resolve(&mut g);
    cite!(["608.3b"]; assert_eq!(g.battlefield, vec![protected]));
}

#[test]
fn r709_3a_a_target_prohibition_checks_the_chosen_split_half() {
    use game::card::CastAs;
    let mut g = game();
    let split = CardDef::split([
        def("Warm", "{R}", "Instant", "Warm deals 1 damage to target creature.", None),
        def("Cool", "{U}", "Instant", "Cool deals 1 damage to target creature.", None),
    ])
    .unwrap();
    game::play::check(&split).unwrap();
    std::sync::Arc::make_mut(&mut g.defs).push(split);
    let protected = g.put(0, 0, Zone::Battlefield);
    let card = g.put(1, 11, Zone::Hand);
    mana(&mut g, 1);
    g.act(0, Move::Pass).unwrap();
    cite!(["709.3a"]; refused("an illegal target, Fiendslayer Paladin: an effect says it can't be the target of that (115.1)"): g.act(1, Move::CastAs(CastAs::Half(0), Box::new(Move::Cast { card, targets: targets(protected) }))));
    cite!(["709.3a"]; ok: g.act(1, paid("U", Move::CastAs(CastAs::Half(1), Box::new(Move::Cast { card, targets: targets(protected) })))));
    resolve(&mut g);
    assert_eq!(g.obj(protected).damage, 1);
}
#[test]
fn r603_3d_a_red_permanents_trigger_can_target_a_creature_protected_only_from_spells() {
    let mut g = game();
    let arrival = def(
        "Arrival",
        "{R}",
        "Creature",
        "When this creature enters, Arrival deals 1 damage to target creature.",
        Some((1, 1)),
    );
    game::play::check(&arrival).unwrap();
    std::sync::Arc::make_mut(&mut g.defs).push(arrival);
    let protected = g.put(0, 0, Zone::Battlefield);
    let source = g.put(1, 11, Zone::Hand);
    g.active = 1;
    g.priority = 1;
    mana(&mut g, 1);
    g.act(1, paid("R", Move::Cast { card: source, targets: vec![] })).unwrap();
    resolve(&mut g);
    cite!(["603.3d"]; assert!(matches!(g.decision, Some(game::state::Decision::Targets { player: 1, .. }))));
    cite!(["603.3d", "115.1d"]; ok: g.act(1, Move::ChooseTargets(targets(protected))));
    resolve(&mut g);
    assert_eq!(g.obj(protected).damage, 1);
}
#[test]
fn r115_10_spell_target_restrictions_do_not_prevent_nontargeted_damage() {
    let mut g = game();
    let sweep = def("Wave", "{R}", "Instant", "Wave deals 1 damage to each creature.", None);
    game::play::check(&sweep).unwrap();
    std::sync::Arc::make_mut(&mut g.defs).push(sweep);
    let protected = g.put(0, 0, Zone::Battlefield);
    let card = g.put(1, 11, Zone::Hand);
    mana(&mut g, 1);
    g.act(0, Move::Pass).unwrap();
    g.act(1, paid("R", Move::Cast { card, targets: vec![] })).unwrap();
    resolve(&mut g);
    cite!(["115.10"]; assert_eq!(g.obj(protected).damage, 1));
}
#[test]
fn r115_1b_an_attached_aura_is_no_longer_a_targeting_spell() {
    let mut g = game();
    let protected = g.put(0, 0, Zone::Battlefield);
    g.active = 1;
    g.priority = 1;
    mana(&mut g, 1);
    cast(&mut g, 1, 8, protected, "U");
    resolve(&mut g);
    let aura = *g.battlefield.last().unwrap();
    let paint = def("Tint", "{0}", "Instant", "Target permanent becomes red until end of turn.", None);
    game::play::check(&paint).unwrap();
    std::sync::Arc::make_mut(&mut g.defs).push(paint);
    while g.priority != 1 {
        g.act(g.priority, Move::Pass).unwrap();
    }
    cast(&mut g, 1, 11, aura, "");
    resolve(&mut g);
    cite!(["115.1b"]; assert_eq!(g.attached_to(aura), Some(Target::Object(protected))));
    assert_eq!(g.power(protected), Some(3));
}
#[test]
fn r115_1_spell_type_restrictions_share_the_spell_selector_predicates() {
    for (kind, allowed) in [("Instant", false), ("Sorcery", true)] {
        let defs = vec![
            def("Shelter", "{0}", "Creature", "This creature can't be the target of instant spells.", Some((2, 2))),
            def("Touch", "{0}", kind, "Touch deals 1 damage to target creature.", None),
        ];
        for d in &defs {
            game::play::check(d).unwrap();
        }
        let mut g = Game::new(defs, 3, 20);
        let protected = g.put(0, 0, Zone::Battlefield);
        let card = g.put(0, 1, Zone::Hand);
        cite!(["601.2c", "115.1"]; case: kind; assert_eq!(g.act(0, Move::Cast { card, targets: targets(protected) }).is_ok(), allowed));
        if allowed {
            resolve(&mut g);
            assert_eq!(g.obj(protected).damage, 1);
        }
    }
}
#[test]
fn r115_1_unmodeled_source_qualifications_are_refused() {
    for text in [
        "This creature can't be the target of red spells or abilities your opponents control.",
        "This creature can't be the target of spells you own.",
        "This creature can't be the target of players.",
    ] {
        if text.ends_with("players.") {
            assert_eq!(CardDef::new("Shelter","{0}","Creature",text,Some((2,2))).expect_err("a targeting source is a spell or ability"),"text the grammar does not read");
        } else {
            let d=CardDef::new("Shelter","{0}","Creature",text,Some((2,2))).unwrap();
            let reason=if text.contains("own.") {"target prohibition: another source, owner, caster or an explicit zone"}else{"target prohibition: qualities of abilities or their sources"};
            crate::support::refused(game::play::check(&d).map_err(|e|e.0),reason);
        }
    }
}
