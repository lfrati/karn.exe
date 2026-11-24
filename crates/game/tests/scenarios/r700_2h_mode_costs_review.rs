//! Modes with their own costs: cost reductions after the mode costs and
//! escalate's are added (601.2f), a copy of a tiered spell (700.2g, 202.3), the same pawprint
//! mode chosen twice with its own targets (700.2d), an up-to trigger with a targetless mode
//! (603.3c), and costs that are not modes' (a choice of additional costs) still refused.

use crate::support::{cite,paid};
use game::card::CardDef;
use game::legal::Asked;
use game::state::{Answer, Decision};
use game::{Game, Move, Target, Zone};

const STEPS: usize = 0;
const FIRE: usize = 1;
const ALLIANCE: usize = 2;
const TWINCAST: usize = 3;
const BEARS: usize = 4;
const ISLAND: usize = 5;
const SNIP: usize = 6;
const ACCIDENT: usize = 7;
const BURROW: usize = 8;
const REDUCER: usize = 9;
const WATCHER: usize = 10;

fn defs() -> Vec<CardDef> {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let real = |name: &str| CardDef::from_faces(catalog.get(name).expect(name)).expect(name);
    vec![
        real("Three Steps Ahead"),
        real("Fire Magic"),
        real("Blessed Alliance"),
        real("Twincast"),
        CardDef::new("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "({T}: Add {U}.)", None).unwrap(),
        CardDef::new("Low Snip", "{U}", "Instant", "Counter target spell with mana value 1 or less.", None).unwrap(),
        real("Unfortunate Accident"),
        real("Season of the Burrow"),
        CardDef::new("Reducer", "{0}", "Artifact", "Instant and sorcery spells you cast cost {1} less to cast.", None).unwrap(),
        CardDef::new("Watcher", "{0}", "Enchantment", "At the beginning of combat on your turn, choose up to one —\n• Tap target creature.\n• You gain 1 life.", None).unwrap(),
    ]
}

fn game() -> Game {
    let mut g = Game::new(defs(), 2, 20);
    for p in 0..2 { for _ in 0..6 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}

fn pool(g: &mut Game, player: usize, colors: [u32; 5], colorless: u32) {
    g.players[player].pool.colors = colors;
    g.players[player].pool.colorless = colorless;
}

const W: usize = 0;
const U: usize = 1;
const B: usize = 2;
const R: usize = 3;

fn colored(color: usize, n: u32) -> [u32; 5] {
    let mut colors = [0; 5];
    colors[color] = n;
    colors
}



fn modal(modes: &[usize], mv: Move) -> Move { Move::Modes(modes.to_vec(), Box::new(mv)) }

fn pass(g: &mut Game) {
    for _ in 0..2 { let p = g.priority; g.act(p, Move::Pass).unwrap(); }
}

#[test]
fn every_card_of_the_review_is_one_the_engine_runs() {
    for def in defs() {
        assert_eq!(game::play::check(&def).err().map(|u| u.0), None, "{}", def.name());
    }
}

/// 601.2f: the total cost is the mana cost plus all additional costs, minus all reductions: a
/// generic reduction applies to the generic mana a mode's cost adds, even when the mana cost
/// itself has none.
#[test]
fn a_cost_reduction_applies_to_the_total_with_the_mode_costs() {
    // Three Steps Ahead {U}, "+ {2} — Draw two cards, then discard a card.": {U}{2} - {1}
    let mut g = game();
    g.put(0, REDUCER, Zone::Battlefield);
    let steps = g.put(0, STEPS, Zone::Hand);
    pool(&mut g, 0, colored(U, 1), 1);
    g.act(0, paid("CU", modal(&[2], Move::Cast { card: steps, targets: vec![] }))).unwrap();
    cite!(["601.2f"]; assert_eq!(g.players[0].pool.total(), 0));

    // Unfortunate Accident {B}, both modes: {B} + {2}{B} + {1} - {1} = {2}{B}{B}
    let mut g = game();
    g.put(0, REDUCER, Zone::Battlefield);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    let accident = g.put(0, ACCIDENT, Zone::Hand);
    pool(&mut g, 0, colored(B, 2), 1);
    cite!(["601.2f"]; refused("the supplied mana does not pay that cost, all of it and no more"): act_unchanged(&mut g, 0, paid("CBB", modal(&[0, 1], Move::Cast { card: accident, targets: vec![vec![Target::Object(bears)]] }))));
    pool(&mut g, 0, colored(B, 2), 2);
    g.act(0, paid("CCBB", modal(&[0, 1], Move::Cast { card: accident, targets: vec![vec![Target::Object(bears)]] }))).unwrap();
    cite!(["601.2f"]; assert_eq!(g.players[0].pool.total(), 0));

    // Fire's {0} tier: {R}, nothing generic to reduce; it can't be less than {R}
    let mut g = game();
    g.put(0, REDUCER, Zone::Battlefield);
    let fire = g.put(0, FIRE, Zone::Hand);
    pool(&mut g, 0, [0; 5], 0);
    cite!(["601.2f"]; refused("the supplied mana does not pay that cost, all of it and no more"): act_unchanged(&mut g, 0, paid("", modal(&[0], Move::Cast { card: fire, targets: vec![] }))));
    pool(&mut g, 0, colored(R, 1), 1);
    g.act(0, paid("R", modal(&[0], Move::Cast { card: fire, targets: vec![] }))).unwrap();
    assert_eq!(g.players[0].pool.total(), 1, "{{R}} paid, the colorless left");
}

#[test]
fn a_cost_reduction_applies_after_escalate_is_added() {
    // Blessed Alliance {1}{W}, escalate {2}, all three modes: {1}{W}{2}{2} - {1} = {4}{W}
    let mut g = game();
    g.put(0, REDUCER, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let alliance = g.put(0, ALLIANCE, Zone::Hand);
    let three = modal(&[0, 1, 2], Move::Cast { card: alliance, targets: vec![vec![Target::Player(0)], vec![Target::Object(bears)], vec![Target::Player(1)]] });
    pool(&mut g, 0, colored(W, 1), 3);
    cite!(["601.2f", "702.120a"]; refused("the supplied mana does not pay that cost, all of it and no more"): act_unchanged(&mut g, 0, paid("CCCW", three.clone())));
    pool(&mut g, 0, colored(W, 1), 4);
    g.act(0, paid("CCCCW", three)).unwrap();
    cite!(["601.2f", "702.120a"]; assert_eq!(g.players[0].pool.total(), 0));
}

/// 700.2g: a copy of a tiered spell has the tier chosen; 707.2: its mana cost is copied, and
/// its mana value is that of its mana cost alone (202.3); paying the tier was not copiable.
#[test]
fn a_copy_of_a_tiered_spell_has_its_tier_and_the_mana_value_of_its_mana_cost() {
    let mut g = game();
    let bears = g.put(1, BEARS, Zone::Battlefield);
    let fire = g.put(0, FIRE, Zone::Hand);
    let twincast = g.put(0, TWINCAST, Zone::Hand);
    let snip = g.put(1, SNIP, Zone::Hand);
    pool(&mut g, 0, colored(R, 1), 5);
    g.act(0, modal(&[2], paid("CCCCCR",Move::Cast { card: fire, targets: vec![] }))).unwrap();
    let spell = *g.stack.last().unwrap();
    pool(&mut g, 0, colored(U, 2), 0);
    g.act(0, paid("UU",Move::Cast { card: twincast, targets: vec![vec![Target::Object(spell)]] })).unwrap();
    pass(&mut g);
    while let Some(Decision::Choose { player, .. }) = g.decision.clone() { g.act(player, Move::Choose(Answer::No)).unwrap(); }
    let copy = *g.stack.last().unwrap();
    assert_ne!(copy, spell);
    cite!(["700.2g", "707.10"]; assert_eq!(g.obj(copy).modes, vec![2]));
    // the copy has mana value 1: Low Snip can counter it
    g.act(0, Move::Pass).unwrap();
    pool(&mut g, 1, colored(U, 1), 0);
    cite!(["202.3", "707.2"]; assert!(g.act(1, paid("U",Move::Cast { card: snip, targets: vec![vec![Target::Object(copy)]] })).is_ok()));
    pass(&mut g);
    assert!(!g.stack.contains(&copy));
    // the original's Firaga: 3 damage
    pass(&mut g);
    assert!(!g.battlefield.contains(&bears));
}

/// 700.2d: the same mode chosen twice is as if printed twice; each has its own target, the same
/// object or different ones.
#[test]
fn the_same_pawprint_mode_twice_takes_a_target_each_time() {
    // Season of the Burrow, "{P}{P} — Exile target nonland permanent. Its controller draws a
    // card." twice and "{P} — Create a 1/1 white Rabbit creature token."
    let mut g = game();
    let a = g.put(1, BEARS, Zone::Battlefield);
    let b = g.put(1, BEARS, Zone::Battlefield);
    let burrow = g.put(0, BURROW, Zone::Hand);
    pool(&mut g, 0, colored(W, 2), 3);
    let hand = g.players[1].hand.len();
    g.act(0, modal(&[0, 1, 1], paid("CCCWW",Move::Cast { card: burrow, targets: vec![vec![Target::Object(a)], vec![Target::Object(b)]] }))).unwrap();
    pass(&mut g);
    cite!(["700.2d"]; assert!(!g.battlefield.contains(&a) && !g.battlefield.contains(&b)));
    assert_eq!(g.players[1].hand.len(), hand + 2);
    assert_eq!(g.battlefield.iter().filter(|o| g.obj(**o).token()).count(), 1);

    // the same target for both: exiled by the first, the second's target is gone (608.2b),
    // and the first mode's draw is the only draw
    let mut g = game();
    let a = g.put(1, BEARS, Zone::Battlefield);
    let burrow = g.put(0, BURROW, Zone::Hand);
    pool(&mut g, 0, colored(W, 2), 3);
    let hand = g.players[1].hand.len();
    cite!(["700.2d"]; assert!(g.act(0, modal(&[1, 1], paid("CCCWW",Move::Cast { card: burrow, targets: vec![vec![Target::Object(a)], vec![Target::Object(a)]] }))).is_ok()));
    pass(&mut g);
    assert!(!g.battlefield.contains(&a));
    cite!(["608.2b"]; assert_eq!(g.players[1].hand.len(), hand + 1));
}

/// 603.3c: a mode without targets can be chosen though the other has none; choosing none
/// removes the ability.
#[test]
fn an_up_to_trigger_with_a_targetless_mode_still_asks() {
    let mut g = game();
    g.put(0, WATCHER, Zone::Battlefield);
    while g.decision.is_none() { let p = g.priority; g.act(p, Move::Pass).unwrap(); }
    let Some((0, Asked::Modes { options, .. })) = g.asked() else { panic!("the modes are asked: {:?}", g.decision) };
    let options: Vec<_> = options.into_iter().map(|(m, _)| m).collect();
    cite!(["603.3c", "700.2b"]; assert!(options.contains(&vec![]) && options.contains(&vec![1]) && !options.contains(&vec![0]), "{options:?}"));
    g.act(0, modal(&[1], Move::ChooseTargets(vec![]))).unwrap();
    pass(&mut g);
    assert_eq!(g.players[0].life, 21);
}

/// An escalate cost that is a choice among alternatives would be chosen once for each mode
/// beyond the first (601.2b); one announcement for all of them is not that. Nonmana escalate
/// costs without a choice are covered by r702_120_escalate.
#[test]
fn an_escalate_cost_with_a_choice_stays_refused() {
    let text = "Escalate—Discard a card or pay 3 life.\nChoose one or more —\n• You gain 1 life.\n• You gain 2 life.";
    let def = CardDef::new("Choice escalate witness", "{1}{B}", "Sorcery", text, None).unwrap();
    // Uncited (admission): each payment's own choice is not announced.
    assert_eq!(game::play::check(&def).expect_err("a choice in an escalate cost is not executable").0,
        "escalate: a choice among alternatives in a cost paid for each extra mode");
}

/// 702.120a with 700.2d: a mode chosen twice is two modes chosen, the second beyond the first.
/// No printed card has both; the engine refuses the combination.
#[test]
fn unbounded_repeated_escalate_modes_remain_explicitly_refused() {
    let def = CardDef::new("Echo", "{W}", "Sorcery", "Escalate {1}\nChoose one or more. You may choose the same mode more than once.\n• You gain 1 life.\n• You gain 2 life.", None).unwrap();
    // Uncited (admission): unbounded repeated modes have no supported announcement bound.
    assert_eq!(game::play::check(&def).expect_err("unbounded repeated modes remain unsupported").0,
        "modes: any number of them, the same more than once");
}

#[track_caller]
fn act_unchanged(g: &mut Game, player: usize, action: Move) -> Result<(), String> {
    let before = format!("{g:?}");
    let result = g.act(player, action);
    assert_eq!(format!("{g:?}"), before);
    result
}
