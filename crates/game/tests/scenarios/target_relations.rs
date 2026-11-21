//! Target relationships are casting requirements, not just independent target sets (115.1, 601.2c).
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Target, Zone};

fn game() -> Game {
    let defs = vec![
        CardDef::new(
            "Arc Trail",
            "{1}{R}",
            "Sorcery",
            "Arc Trail deals 2 damage to any target and 1 damage to any other target.",
            None,
        )
        .unwrap(),
        CardDef::new(
            "Archer",
            "{0}",
            "Creature — Archer",
            "{0}: This creature deals 1 damage to any other target.",
            Some((2, 2)),
        )
        .unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    let mut g = Game::new(defs, 3, 20);
    g.add_mana(0, ir::mana::ManaSymbol::Color(ir::Color::Red));
    g.add_mana(0, ir::mana::ManaSymbol::Color(ir::Color::Red));
    g
}
#[test]
fn r601_2c_another_damage_target_must_differ_from_the_preceding_target() {
    let mut g = game();
    let card = g.put(0, 0, Zone::Hand);
    let before = format!("{g:?}");
    cite!(["601.2c"]; refused("another target must differ from the preceding targets (115.1, 601.2c)"): g.act(0, paid("RR", Move::Cast { card, targets: vec![vec![Target::Player(1)], vec![Target::Player(1)]] })));
    assert_eq!(format!("{g:?}"), before);
}
#[test]
fn r115_4_another_damage_target_can_be_the_caster_when_the_first_target_is_someone_else() {
    let mut g = game();
    let card = g.put(0, 0, Zone::Hand);
    cite!(["115.4"]; ok: g.act(0, paid("RR", Move::Cast { card, targets: vec![vec![Target::Player(1)], vec![Target::Player(0)]] })));
    for _ in 0..3 {
        g.act(g.priority, Move::Pass).unwrap();
    }
    cite!(["115.4"]; assert_eq!(g.players.iter().map(|p| p.life).collect::<Vec<_>>(), vec![19, 18, 20]));
}
#[test]
fn r115_4_a_single_another_target_excludes_the_source_not_its_controller() {
    let mut g = game();
    let source = g.put(0, 1, Zone::Battlefield);
    cite!(["115.1"]; refused("an illegal target, Archer: it is the object whose ability this is, and the target says \"another\" (115.1)"): g.act(0, Move::Activate { source, ability: 0, targets: vec![vec![Target::Object(source)]], paying: vec![] }));
    cite!(["115.4"]; ok: g.act(0, Move::Activate { source, ability: 0, targets: vec![vec![Target::Player(0)]], paying: vec![] }));
    for _ in 0..3 {
        g.act(g.priority, Move::Pass).unwrap();
    }
    assert_eq!(g.players[0].life, 19);
}

#[test]
fn r601_2c_individually_nonempty_target_groups_may_have_no_joint_legal_choice() {
    let defs = vec![
        CardDef::new(
            "Twin",
            "{0}",
            "Instant",
            "Twin deals 2 damage to target creature and 1 damage to another target creature.",
            None,
        )
        .unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap();
    }
    let mut g = Game::new(defs, 3, 20);
    let card = g.put(0, 0, Zone::Hand);
    let first = g.put(0, 1, Zone::Battlefield);
    cite!(["601.2c"]; refused("another target must differ from the preceding targets"): act_unchanged(&mut g,Move::Cast { card, targets: vec![vec![Target::Object(first)]; 2] }));
    let second = g.put(1, 1, Zone::Battlefield);
    cite!(["601.2c"]; ok: g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(first)], vec![Target::Object(second)]] }));
    for _ in 0..3 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["608.2c", "704.5g"]; assert_eq!(g.obj(first).zone, Zone::Gone));
    cite!(["608.2c"]; assert!(g.battlefield.contains(&second) && g.obj(second).damage == 1 && g.stack.is_empty()));
}
#[test]
fn r608_2b_losing_the_first_damage_target_does_not_invalidate_the_distinct_second_target() {
    let mut defs = vec![CardDef::new("Kill", "{0}", "Instant", "Destroy target creature.", None).unwrap()];
    defs.extend(game().defs.iter().cloned());
    let mut g = Game::new(defs, 3, 20);
    let creature = g.put(1, 2, Zone::Battlefield);
    g.add_mana(0, ir::mana::ManaSymbol::Color(ir::Color::Red));
    g.add_mana(0, ir::mana::ManaSymbol::Color(ir::Color::Red));
    let arc = g.put(0, 1, Zone::Hand);
    g.act(0, paid("RR", Move::Cast { card: arc, targets: vec![vec![Target::Object(creature)], vec![Target::Player(0)]] })).unwrap();
    let kill = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: kill, targets: vec![vec![Target::Object(creature)]] }).unwrap();
    for _ in 0..6 {
        g.act(g.priority, Move::Pass).unwrap();
    }
    cite!(["608.2b"]; assert_eq!(g.players[0].life, 19));
    assert_eq!(g.obj(creature).zone, Zone::Gone);
}
#[test]
fn r113_2c_target_relationship_indices_survive_multiple_spell_abilities() {
    let d = CardDef::new(
        "Composite",
        "{0}",
        "Instant",
        "Target player gains 1 life.\nComposite deals 2 damage to any target and 1 damage to any other target.",
        None,
    )
    .unwrap();
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d], 3, 20);
    let card = g.put(0, 0, Zone::Hand);
    cite!(["601.2c"]; refused("another target must differ from the preceding targets"): act_unchanged(&mut g,Move::Cast { card, targets: vec![vec![Target::Player(0)], vec![Target::Player(1)], vec![Target::Player(1)]] }));
    cite!(["115.3"]; ok: g.act(
        0,
        Move::Cast { card, targets: vec![vec![Target::Player(1)], vec![Target::Player(1)], vec![Target::Player(0)]] },
    ));
    for _ in 0..3 {
        g.act(g.priority, Move::Pass).unwrap();
    }
    cite!(["115.3"]; assert_eq!(g.players.iter().map(|p| p.life).collect::<Vec<_>>(), vec![19, 19, 20]));
}
#[test]
fn r601_2c_unresolved_another_references_across_actions_are_refused() {
    let d =
        CardDef::new("Ambiguous", "{0}", "Instant", "Destroy target creature. Exile another target creature.", None)
            .unwrap();
    assert!(game::play::check(&d).unwrap_err().0.contains("referent is not modeled"));
}

#[test]
fn r603_3d_triggered_damage_requires_a_jointly_legal_set_of_targets() {
    let defs=vec![
        CardDef::new("Watcher","{0}","Enchantment","When this enchantment enters, this enchantment deals 2 damage to target creature and 1 damage to another target creature.",None).unwrap(),
        CardDef::new("Bear","{0}","Creature — Bear","",Some((2,2))).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap();
    }
    for count in [1, 2] {
        let mut g = Game::new(defs.clone(), 3, 20);
        let creatures: Vec<_> = (0..count).map(|_| g.put(0, 1, Zone::Battlefield)).collect();
        let card = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
        for _ in 0..3 {
            g.act(g.priority, Move::Pass).unwrap();
        }
        if count == 1 {
            assert!(g.decision.is_none());
            cite!(["603.3d"]; assert!(g.stack.is_empty()));
        } else {
            cite!(["603.3d"]; assert!(matches!(g.decision, Some(game::state::Decision::Targets { .. }))));
            let before = format!("{g:?}");
            cite!(["603.3d"]; refused("another target must differ from the preceding targets (115.1, 601.2c)"): g.act(0, Move::ChooseTargets(vec![vec![Target::Object(creatures[0])]; 2])));
            assert_eq!(format!("{g:?}"), before);
            cite!(["603.3d"]; ok: g.act(0, Move::ChooseTargets(creatures.iter().map(|c| vec![Target::Object(*c)]).collect())));
            for _ in 0..3 {
                g.act(g.priority, Move::Pass).unwrap();
            }
            assert_eq!(g.obj(creatures[0]).zone, Zone::Gone);
            assert_eq!(g.obj(creatures[1]).damage, 1);
        }
    }
}
#[test]
fn r700_2a_modes_require_a_jointly_legal_target_assignment() {
    let defs=vec![
        CardDef::new("Fork","{0}","Instant","Choose one —\n• Fork deals 2 damage to target creature and 1 damage to another target creature.\n• You gain 1 life.",None).unwrap(),
        CardDef::new("Bear","{0}","Creature — Bear","",Some((2,2))).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap();
    }
    let mut g = Game::new(defs, 3, 20);
    let bear = g.put(0, 1, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand);
    cite!(["700.2a", "601.2c"]; refused("another target must differ from the preceding targets"): act_unchanged(&mut g,Move::Modes(vec![0], Box::new(Move::Cast { card, targets: vec![vec![Target::Object(bear)]; 2] }))));
    cite!(["700.2a"]; ok: g.act(0, Move::Modes(vec![1], Box::new(Move::Cast { card, targets: vec![] }))));
    for _ in 0..3 {g.act(g.priority,Move::Pass).unwrap();}
    assert_eq!(g.players[0].life,21); assert!(g.stack.is_empty());
}

#[track_caller]
fn act_unchanged(g:&mut Game,action:Move)->Result<(),String> {
 let before=format!("{g:?}");let result=g.act(0,action);assert_eq!(format!("{g:?}"),before);result
}
