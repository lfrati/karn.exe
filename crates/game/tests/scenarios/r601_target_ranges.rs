//! Bounded target counts are announced once (601.2c); resolution checks each target (608.2b).
use crate::support::{cite,paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

fn game() -> Game {
    let defs = vec![
        CardDef::new("Get Out", "{U}{U}", "Instant", "Choose one —\n• Counter target creature or enchantment spell.\n• Return one or two target creatures and/or enchantments you own to your hand.", None).unwrap(),
        CardDef::new("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2,2))).unwrap(),
        CardDef::new("Glorious Anthem", "{1}{W}{W}", "Enchantment", "Creatures you control get +1/+1.", None).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap();
    }
    let mut g = Game::new(defs, 2, 20);
    g.players[0].pool.colors[ir::Color::Blue as usize] = 2;
    g
}
fn cast(card: ObjId, targets: Vec<ObjId>) -> Move {
    Move::Modes(
        vec![1],
        Box::new(paid("UU",Move::Cast { card, targets: vec![targets.into_iter().map(Target::Object).collect()] })),
    )
}
fn pass(g: &mut Game) {
    for _ in 0..2 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}

#[test]
fn r601_2c_range_listing_and_casting_agree_on_minimum_maximum_and_ownership() {
    let mut g = game();
    let a = g.put(0, 1, Zone::Battlefield);
    let b = g.put(0, 2, Zone::Battlefield);
    let c = g.put(0, 1, Zone::Battlefield);
    let enemy = g.put(1, 1, Zone::Battlefield);
    let land = g.put(0, 3, Zone::Battlefield);
    let spell = g.put(0, 0, Zone::Hand);
    for (invalid, why) in [
        (vec![], "wrong number of targets: 0 chosen, it takes 1 to 2 (601.2c)"),
        (vec![a, b, c], "wrong number of targets: 3 chosen, it takes 1 to 2 (601.2c)"),
        (vec![a, a], "the same target twice for one \"target\" (115.3)"),
        (vec![enemy], "an illegal target, Grizzly Bears: the target is one you own, and another player owns it (115.1)"),
        (vec![land], "an illegal target, Island: it is neither a creature nor an enchantment (115.1)"),
    ] {
        cite!(["601.2c"]; case: why; refused(why): g.act(0, cast(spell, invalid)));
        assert_eq!(g.players[0].pool.total(), 2);
        assert_eq!(g.obj(spell).zone, Zone::Hand);
    }
    for chosen in [vec![a], vec![a, b]] {
        let mut copy = g.clone();
        cite!(["601.2c"]; case: format!("{} target(s)", chosen.len()); ok: copy.act(0, cast(spell, chosen.clone())));
        pass(&mut copy);
        assert_eq!(copy.players[0].hand.len(), chosen.len());
        assert!(copy.stack.is_empty());
        for id in chosen {
            assert!(!copy.battlefield.contains(&id));
        }
    }
}

#[test]
fn r608_2b_range_resolves_for_the_remaining_legal_target() {
    let mut g = game();
    let a = g.put(0, 1, Zone::Battlefield);
    let b = g.put(0, 2, Zone::Battlefield);
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, cast(spell, vec![a, b])).unwrap();
    g.move_object(a, Zone::Graveyard);
    pass(&mut g);
    cite!(["608.2b"]; assert_eq!(g.players[0].hand.len(), 1));
    assert_eq!(g.obj(g.players[0].hand[0]).card, Some(2));
    cite!(["608.2b"]; assert!(!g.battlefield.contains(&b)));
}

#[test]
fn r700_2_a_mode_with_no_legal_minimum_is_not_offered() {
    let mut g = game();
    g.put(1, 1, Zone::Battlefield);
    let spell = g.put(0, 0, Zone::Hand);
    let before = format!("{g:?}");
    cite!(["700.2a"]; refused("there is no legal target for it (601.2c)"): g.act(0, Move::Modes(vec![0], Box::new(Move::Cast { card: spell, targets: vec![vec![]] }))));
    assert_eq!(format!("{g:?}"), before);
    let creature = g.put(0, 1, Zone::Hand);
    g.players[0].pool.colors[ir::Color::Green as usize] = 2;
    g.act(0, paid("GG",Move::Cast { card: creature, targets: vec![] })).unwrap();
    let target = *g.stack.last().unwrap();
    g.players[0].pool.colors[ir::Color::Blue as usize] = 2;
    g.act(0, Move::Modes(vec![0], Box::new(paid("UU",Move::Cast { card: spell, targets: vec![vec![Target::Object(target)]] }))))
        .unwrap();
    pass(&mut g);
    assert!(g.stack.is_empty());
    assert_eq!(g.players[0].graveyard.len(), 2);
}

#[test]
fn r608_2b_resolution_does_not_reapply_the_announcement_minimum() {
    let def = CardDef::new(
        "Range witness",
        "{0}",
        "Sorcery",
        "Return two or three target creatures to their owners' hands.",
        None,
    )
    .unwrap();
    game::play::check(&def).unwrap();
    let bear = CardDef::new("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap();
    let mut g = Game::new(vec![def, bear], 2, 20);
    let a = g.put(0, 1, Zone::Battlefield);
    let b = g.put(0, 1, Zone::Battlefield);
    let spell = g.put(0, 0, Zone::Hand);
    cite!(["601.2c"]; refused("wrong number of targets: 1 chosen, it takes 2 to 3 (601.2c)"): g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(a)]] }));
    g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(a), Target::Object(b)]] }).unwrap();
    g.move_object(a, Zone::Graveyard);
    pass(&mut g);
    cite!(["608.2b"]; assert_eq!(g.players[0].hand.len(), 1));
    cite!(["608.2b"]; assert!(!g.battlefield.contains(&b)));
}
