//! "Choose three. You may choose the same mode more than once." (700.2d): a mode chosen n times
//! is treated as if it appeared n times in sequence, each instance with its own targets, the
//! same object or different ones; the modes are still followed in printed order (608.2c), and
//! a copy has the modes chosen for the original (700.2g).
use crate::support::{cite,paid,mana_pool};
use game::card::CardDef;
use game::state::{Answer, Choice, Decision};
use game::{Game, Move, ObjId, Target, Zone};

const FIERY: usize = 0;
const MYSTIC: usize = 1;
const ROCK: usize = 2;
const BEAR: usize = 3;
const TWINCAST: usize = 4;
const CLARION: usize = 5;
const BOLT: usize = 6;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Fiery Confluence", "{2}{R}{R}", "Sorcery",
            "Choose three. You may choose the same mode more than once.\n• Fiery Confluence deals 1 damage to each creature.\n• Fiery Confluence deals 2 damage to each opponent.\n• Destroy target artifact.", None),
        card("Mystic Confluence", "{3}{U}{U}", "Instant",
            "Choose three. You may choose the same mode more than once.\n• Counter target spell unless its controller pays {3}.\n• Return target creature to its owner's hand.\n• Draw a card.", None),
        card("Rock", "{1}", "Artifact", "", None),
        card("Bear", "{1}{G}", "Creature — Bear", "", Some((3, 3))),
        card("Twincast", "{U}{U}", "Instant", "Copy target instant or sorcery spell. You may choose new targets for the copy.", None),
        card("Deafening Clarion", "{1}{R}{W}", "Sorcery",
            "Choose one or both —\n• Deafening Clarion deals 3 damage to each creature.\n• Creatures you control gain lifelink until end of turn.", None),
        card("Lightning Bolt", "{R}", "Instant", "Lightning Bolt deals 3 damage to any target.", None),
    ]
}
fn game() -> Game {
    for d in defs() {
        game::play::check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    let mut g = Game::new(defs(), 2, 20);
    for p in 0..2 {
        for _ in 0..5 { g.put(p, ROCK, Zone::Library); }
    }
    g
}
fn modal(modes: &[usize], card: ObjId, targets: Vec<Vec<Target>>,units:&str) -> Move {
    Move::Modes(modes.to_vec(), Box::new(paid(units,Move::Cast { card, targets })))
}
fn pass(g: &mut Game) {
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
}
/// Player 0 casts a Fiery Confluence with these modes and targets, with the mana for it.
fn fiery(g: &mut Game, modes: &[usize], targets: Vec<Vec<Target>>) -> Result<ObjId, String> {
    let card = g.put(0, FIERY, Zone::Hand);
    g.players[0].pool.colors[3] = 4;
    g.act(0, modal(modes, card, targets,"RRRR")).map(|_| card)
}


#[test]
fn r700_2d_a_mode_chosen_three_times_happens_three_times() {
    let mut g = game();
    fiery(&mut g, &[1, 1, 1], vec![]).unwrap();
    cite!(["700.2d"]; assert_eq!(g.obj(*g.stack.last().unwrap()).modes, vec![1, 1, 1]));
    pass(&mut g);
    cite!(["700.2d"]; assert_eq!((g.players[0].life, g.players[1].life), (20, 14)));

    // three separate 1 damage instances, all before state-based actions (704.3)
    let mut g = game();
    let bear = g.put(1, BEAR, Zone::Battlefield);
    let mut copy = g.clone();
    fiery(&mut g, &[0, 0, 0], vec![]).unwrap();
    pass(&mut g);
    cite!(["700.2d"]; assert_eq!(g.obj(bear).zone, Zone::Gone, "3 damage to a 3/3"));
    fiery(&mut copy, &[0, 0, 1], vec![]).unwrap();
    pass(&mut copy);
    assert_eq!((copy.obj(bear).damage, copy.players[1].life), (2, 18));
}

#[test]
fn r700_2d_each_instance_has_its_own_target_in_printed_order() {
    // chosen out of order: followed as printed, damage before either destruction (608.2c)
    let mut g = game();
    let (a, b) = (g.put(1, ROCK, Zone::Battlefield), g.put(1, ROCK, Zone::Battlefield));
    let two = vec![vec![Target::Object(a)], vec![Target::Object(b)]];
    crate::support::refused(fiery(&mut g.clone(), &[2, 2, 1], vec![vec![Target::Object(a)]]), "that has 2 target group(s), 1 given");
    cite!(["700.2d"]; refused("wrong number of targets: 0 chosen, it takes 1 (601.2c)"): fiery(&mut g.clone(), &[2, 2, 1], vec![vec![Target::Object(a)], vec![]]));
    fiery(&mut g, &[2, 1, 2], two).unwrap();
    cite!(["608.2c"]; assert_eq!(g.obj(*g.stack.last().unwrap()).modes, vec![1, 2, 2]));
    pass(&mut g);
    cite!(["700.2d"]; assert_eq!((g.obj(a).zone, g.obj(b).zone, g.players[1].life), (Zone::Gone, Zone::Gone, 18)));

    // the same object for each instance: the second finds it gone and does nothing (608.2b)
    let mut g = game();
    let a = g.put(1, ROCK, Zone::Battlefield);
    cite!(["700.2d"]; ok: fiery(&mut g, &[2, 2, 1], vec![vec![Target::Object(a)], vec![Target::Object(a)]]));
    pass(&mut g);
    assert_eq!((g.obj(a).zone, g.players[1].life), (Zone::Gone, 18));
}

#[test]
fn r700_2d_the_choices_are_every_multiset_of_legal_modes() {
    let mut g = game();
    let card = g.put(0, FIERY, Zone::Hand);
    g.players[0].pool.colors[3] = 4;
    // no artifact: "Destroy target artifact" can't be chosen (700.2a), the others any number of times
    for modes in [vec![0, 0, 0], vec![0, 0, 1], vec![0, 1, 1], vec![1, 1, 1]] {
        cite!(["700.2a", "700.2d"]; case: format!("{modes:?}"); ok: g.clone().act(0, modal(&modes, card, vec![],"RRRR")));
    }
    let artifact = g.put(1, ROCK, Zone::Battlefield);
    for modes in [vec![0, 0, 2], vec![0, 1, 2], vec![0, 2, 2], vec![1, 1, 2], vec![1, 2, 2], vec![2, 2, 2]] {
        let targets = vec![vec![Target::Object(artifact)]; modes.iter().filter(|m| **m == 2).count()];
        cite!(["700.2d"]; case: format!("{modes:?}"); ok: g.clone().act(0, modal(&modes, card, targets,"RRRR")));
    }
    for (wrong, why) in [(&[1, 1][..], "choose 3 to 3 of its modes (700.2a)"), (&[1, 1, 1, 1], "choose 3 to 3 of its modes (700.2a)"), (&[1, 1, 3], "a mode the ability does not have")] {
        cite!(["700.2a"]; case: format!("{wrong:?}"); refused(why): g.clone().act(0, modal(wrong, card, vec![],"RRRR")));
    }
    // without the instruction a mode is still chosen once
    let mut g = game();
    let clarion = g.put(0, CLARION, Zone::Hand);
    g.players[0].pool.colors = [1, 0, 0, 2, 0];
    cite!(["700.2d"]; refused("a mode is chosen once (700.2d)"): g.act(0, Move::Modes(vec![0, 0], Box::new(Move::Cast { card: clarion, targets: vec![] }))));
}

#[test]
fn r700_2d_a_creature_returned_twice_and_a_card_drawn() {
    // Mystic Confluence: two returns of different creatures, then a draw
    let mut g = game();
    let (a, b) = (g.put(1, BEAR, Zone::Battlefield), g.put(0, BEAR, Zone::Battlefield));
    let card = g.put(0, MYSTIC, Zone::Hand);
    g.players[0].pool.colors = [0, 2, 0, 0, 0];
    g.players[0].pool.colorless = 3;
    let hand = g.players[0].hand.len();
    cite!(["700.2d"]; ok: g.act(0, modal(&[2, 1, 1], card, vec![vec![Target::Object(a)], vec![Target::Object(b)]],"CCCUU")));
    pass(&mut g);
    assert!(!g.battlefield.iter().any(|o| g.obj(*o).card == Some(BEAR)));
    assert_eq!(g.players[1].hand.len(), 1);
    cite!(["700.2d"]; assert_eq!(g.players[0].hand.len(), hand - 1 + 1 + 1, "its own Bear back, and a card drawn"));
}

#[test]
fn r700_2g_a_copy_has_the_repeated_modes() {
    let mut g = game();
    fiery(&mut g, &[1, 1, 1], vec![]).unwrap();
    let spell = *g.stack.last().unwrap();
    let twin = g.put(0, TWINCAST, Zone::Hand);
    g.players[0].pool.colors[1] = 2;
    g.act(0, paid("UU",Move::Cast { card: twin, targets: vec![vec![Target::Object(spell)]] })).unwrap();
    pass(&mut g); // Twincast: the copy
    let copy = *g.stack.last().unwrap();
    assert_ne!(copy, spell);
    cite!(["700.2g"]; assert_eq!(g.obj(copy).modes, vec![1, 1, 1]));
    while !g.stack.is_empty() { pass(&mut g); }
    cite!(["700.2g"]; assert_eq!(g.players[1].life, 8));
}

#[test]
fn r700_2d_one_artifact_may_be_the_target_of_every_instance() {
    let mut g = game();
    let rock = g.put(1, ROCK, Zone::Battlefield);
    let three = vec![vec![Target::Object(rock)]; 3];
    cite!(["700.2d"]; ok: fiery(&mut g, &[2, 2, 2], three));
    pass(&mut g);
    assert_eq!(g.obj(rock).zone, Zone::Gone);
}

#[test]
fn r608_2b_with_every_target_gone_the_whole_spell_does_nothing() {
    // the destroy instance's only target leaves: the damage instances don't happen either
    let mut g = game();
    let rock = g.put(1, ROCK, Zone::Battlefield);
    let bear = g.put(1, BEAR, Zone::Battlefield);
    fiery(&mut g, &[0, 1, 2], vec![vec![Target::Object(rock)]]).unwrap();
    g.move_object(rock, Zone::Hand);
    pass(&mut g);
    assert!(g.stack.is_empty());
    cite!(["608.2b"]; assert_eq!((g.obj(bear).damage, g.players[1].life), (0, 20)));

    // one of two targets still legal: the spell resolves, doing what it still can
    let mut g = game();
    let (a, b) = (g.put(1, ROCK, Zone::Battlefield), g.put(1, ROCK, Zone::Battlefield));
    fiery(&mut g, &[1, 2, 2], vec![vec![Target::Object(a)], vec![Target::Object(b)]]).unwrap();
    g.move_object(a, Zone::Hand);
    let in_hand = *g.players[1].hand.last().unwrap();
    g.move_object(in_hand, Zone::Battlefield);
    pass(&mut g);
    cite!(["608.2b"]; assert_eq!(g.players[1].life, 18));
    cite!(["608.2b"]; assert_eq!(g.obj(b).zone, Zone::Gone));
    cite!(["400.7"]; assert_eq!(g.battlefield.iter().filter(|o| g.obj(**o).card == Some(ROCK)).count(), 1, "the returned Rock is a new object (400.7)"));
}

#[test]
fn r700_2d_each_counter_instance_asks_for_its_own_payment() {
    // Mystic Confluence counter, counter, draw: paying the first does not pay the second
    let setup = || {
        let mut g = game();
        let bolt = g.put(0, BOLT, Zone::Hand);
        let mystic = g.put(0, MYSTIC, Zone::Hand);
        g.players[0].pool.colors = [0, 2, 0, 1, 0];
        g.players[0].pool.colorless = 9;
        g.act(0, paid("R",Move::Cast { card: bolt, targets: vec![vec![Target::Player(1)]] })).unwrap();
        let spell = *g.stack.last().unwrap();
        g.act(0, modal(&[0, 2, 0], mystic, vec![vec![Target::Object(spell)], vec![Target::Object(spell)]],"CCCUU")).unwrap();
        pass(&mut g);
        let hand = g.players[0].hand.len();
        (g, hand)
    };
    let asked = |g: &Game| matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::Pay { .. } }));
    // pay the first, decline the second: countered, and the card is still drawn
    let (mut g, hand) = setup();
    assert!(asked(&g));
    g.act(0, Move::Choose(Answer::Pay { plan: vec![], with: Some(mana_pool("CCC")), tapped: vec![] })).unwrap();
    cite!(["700.2d"]; assert!(asked(&g), "asked again by the second instance"));
    g.act(0, Move::Choose(Answer::No)).unwrap();
    assert_eq!(g.players[0].hand.len(), hand + 1);
    cite!(["700.2d"]; assert!(g.stack.is_empty() && g.players[1].life == 20, "Bolt was countered"));
    // decline the first: countered at once; the rest goes on without stalling
    let (mut g, hand) = setup();
    g.act(0, Move::Choose(Answer::No)).unwrap();
    for _ in 0..3 {
        if !asked(&g) { break; }
        g.act(0, Move::Choose(Answer::No)).unwrap();
    }
    assert_eq!(g.players[0].hand.len(), hand + 1);
    assert!(g.stack.is_empty() && g.players[1].life == 20);
}
