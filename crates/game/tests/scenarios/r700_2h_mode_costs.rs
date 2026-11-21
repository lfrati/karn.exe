//! Modes with their own costs (700.2h): Spree's one or more (702.172a), Tiered's one
//! (702.183a), each chosen mode's cost an additional cost of the spell (601.2f); escalate's cost
//! for each mode beyond the first (702.120a); "choose up to one —" of a triggered ability with
//! none chosen (700.2b); a copy's modes (700.2g); the mana value on the stack (202.3); up to
//! N {P} worth of modes, each as often as it is chosen (700.2i, 700.2d).
//! Every card is read from its Oracle text.

use crate::support::{cite,paid};
use game::card::CardDef;
use game::legal::Asked;
use game::state::{Answer, Choice, Decision};
use game::{Game, Move, Target, Zone};

const STEPS: usize = 0;
const FIRE: usize = 1;
const ALLIANCE: usize = 2;
const GEIST: usize = 3;
const TWINCAST: usize = 4;
const BEARS: usize = 5;
const ISLAND: usize = 6;
const SNIP: usize = 7;
const RAID: usize = 8;
const WEAVING: usize = 9;

fn defs() -> Vec<CardDef> {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let real = |name: &str| CardDef::from_faces(catalog.get(name).expect(name)).expect(name);
    vec![
        real("Three Steps Ahead"),
        real("Fire Magic"),
        real("Blessed Alliance"),
        real("Dreamshackle Geist"),
        real("Twincast"),
        CardDef::new("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "({T}: Add {U}.)", None).unwrap(),
        CardDef::new("Low Snip", "{U}", "Instant", "Counter target spell with mana value 1 or less.", None).unwrap(),
        real("Requisition Raid"),
        real("Season of Weaving"),
    ]
}

/// Player 0's precombat main phase, with cards in both libraries to draw.
fn game() -> Game {
    let mut g = Game::new(defs(), 2, 20);
    for p in 0..2 { for _ in 0..6 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}

/// Mana in player 0's pool: of each color in WUBRG order, and colorless.
fn pool(g: &mut Game, colors: [u32; 5], colorless: u32) {
    g.players[0].pool.colors = colors;
    g.players[0].pool.colorless = colorless;
}

const W: usize = 0;
const U: usize = 1;
const R: usize = 3;
const G: usize = 4;

fn colored(color: usize, n: u32) -> [u32; 5] {
    let mut colors = [0; 5];
    colors[color] = n;
    colors
}

/// The choices of modes listed for casting this card.


fn modal(modes: &[usize], mv: Move) -> Move { Move::Modes(modes.to_vec(), Box::new(mv)) }

fn pass(g: &mut Game) {
    let p = g.priority;
    g.act(p, Move::Pass).unwrap();
    let p = g.priority;
    g.act(p, Move::Pass).unwrap();
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for def in defs() {
        assert_eq!(game::play::check(&def).err().map(|u| u.0), None, "{}", def.name());
    }
}

#[test]
fn r702_172a_three_steps_ahead_with_all_three_modes_pays_each_cost() {
    // "Spree. + {1}{U} — Counter target spell. + {3} — Create a token that's a copy of target
    // artifact or creature you control. + {2} — Draw two cards, then discard a card."
    let mut g = game();
    let mine = g.put(0, BEARS, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Hand);
    pool(&mut g, colored(G, 1), 1);
    g.act(0, paid("CG", Move::Cast { card: bears, targets: vec![] })).unwrap();
    let spell = *g.stack.last().unwrap();
    let steps = g.put(0, STEPS, Zone::Hand);
    let discard = g.put(0, ISLAND, Zone::Hand);
    // {U} + {1}{U} + {3} + {2}: eight mana, two of it blue; with seven, all three can't be chosen
    pool(&mut g, colored(U, 2), 5);
    let all = modal(&[0, 1, 2], Move::Cast { card: steps, targets: vec![vec![Target::Object(spell)], vec![Target::Object(mine)]] });
    cite!(["700.2h", "601.2f", "601.2h"]; refused("the supplied mana does not pay that cost, all of it and no more"): act_unchanged(&mut g, 0, paid("CCCCCUU", all.clone())));
    pool(&mut g, colored(U, 2), 6);
    // no mode is not a choice of Spree's: one or more
    let hand = g.players[0].hand.len();
    g.act(0, paid("CCCCCCUU", all)).unwrap();
    cite!(["700.2h", "601.2f"]; assert_eq!(g.players[0].pool.total(), 0, "all eight are paid"));
    pass(&mut g);
    let Some(Decision::Choose { player: 0, choice: Choice::Objects { among, .. } }) = g.decision.clone() else { panic!("the discard: {:?}", g.decision) };
    assert!(among.contains(&discard));
    g.act(0, Move::Choose(Answer::Objects(vec![discard]))).unwrap();
    cite!(["702.172a"]; assert!(g.stack.is_empty() && g.players[0].graveyard.iter().any(|o| g.obj(*o).chars.name == "Grizzly Bears"), "the Bears spell is countered"));
    let copies = g.battlefield.iter().filter(|o| g.obj(**o).chars.name == "Grizzly Bears").count();
    cite!(["702.172a"]; assert_eq!(copies, 2, "a token copy of the Bears"));
    assert_eq!(g.players[0].hand.len(), hand - 1 + 2 - 1, "Three Steps Ahead left; two drawn, one discarded");
}

#[test]
fn r702_172a_one_spree_mode_pays_only_its_cost() {
    // "+ {2} — Draw two cards, then discard a card." alone: {U} and {2}
    let mut g = game();
    let steps = g.put(0, STEPS, Zone::Hand);
    pool(&mut g, colored(U, 1), 2);
    g.act(0, modal(&[2], paid("CCU",Move::Cast { card: steps, targets: vec![] }))).unwrap();
    cite!(["700.2h"]; assert_eq!(g.players[0].pool.total(), 0));
    let spell = *g.stack.last().unwrap();
    cite!(["700.2a"]; assert_eq!(g.obj(spell).modes, vec![2]));
}

#[test]
fn r702_183a_fire_magic_chooses_one_tier_and_pays_its_cost() {
    // "Tiered. • Fire — {0} — 1 damage to each creature. • Fira — {2} — 2 damage.
    // • Firaga — {5} — 3 damage."
    let mut g = game();
    let bears = g.put(1, BEARS, Zone::Battlefield);
    let fire = g.put(0, FIRE, Zone::Hand);
    // {R} and {2}: Fire or Fira, not Firaga, and never two of them
    pool(&mut g, colored(R, 1), 2);
    cite!(["702.183a"]; refused("choose 1 to 1 of its modes"): g.act(0, modal(&[0, 1], Move::Cast { card: fire, targets: vec![] })));
    cite!(["700.2h", "601.2h"]; refused("the supplied mana does not pay that cost, all of it and no more"): act_unchanged(&mut g, 0, paid("CCR", modal(&[2], Move::Cast { card: fire, targets: vec![] }))));
    assert_eq!(g.obj(fire).zone, Zone::Hand, "a refused cast changes nothing");
    g.act(0, paid("CCR", modal(&[1], Move::Cast { card: fire, targets: vec![] }))).unwrap();
    cite!(["700.2h"]; assert_eq!(g.players[0].pool.total(), 0, "{{R}} and Fira's {{2}}"));
    pass(&mut g);
    cite!(["702.183a"]; assert!(!g.battlefield.contains(&bears) && g.players[1].graveyard.len() == 1, "2 damage to each creature"));
}

#[test]
fn r702_183a_the_zero_tier_costs_only_the_mana_cost() {
    let mut g = game();
    let fire = g.put(0, FIRE, Zone::Hand);
    pool(&mut g, colored(R, 1), 0);
    g.act(0, modal(&[0], paid("R",Move::Cast { card: fire, targets: vec![] }))).unwrap();
}

#[test]
fn r702_120a_escalate_is_paid_for_each_mode_beyond_the_first() {
    // "Escalate {2}. Choose one or more — • Target player gains 4 life. • Untap up to two
    // target creatures. • Target opponent sacrifices an attacking creature of their choice."
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    g.tap(bears);
    let alliance = g.put(0, ALLIANCE, Zone::Hand);
    // {1}{W} and {2} for a second mode: two modes, not three
    pool(&mut g, colored(W, 1), 3);
    let three = modal(&[0, 1, 2], Move::Cast { card: alliance, targets: vec![vec![Target::Player(0)], vec![Target::Object(bears)], vec![Target::Player(1)]] });
    cite!(["702.120a", "601.2h"]; refused("the supplied mana does not pay that cost, all of it and no more"): act_unchanged(&mut g, 0, paid("CCCW", three.clone())));
    pool(&mut g, colored(W, 1), 5);
    g.act(0, paid("CCCCCW", three)).unwrap();
    cite!(["702.120a"]; assert_eq!(g.players[0].pool.total(), 0, "{{1}}{{W}} and {{2}}{{2}}"));
    pass(&mut g);
    assert_eq!((g.obj(bears).tapped, g.players[0].life), (false, 24));
}

#[test]
fn r702_120a_one_mode_pays_no_escalate_cost() {
    let mut g = game();
    let alliance = g.put(0, ALLIANCE, Zone::Hand);
    pool(&mut g, colored(W, 1), 1);
    g.act(0, modal(&[0], paid("CW",Move::Cast { card: alliance, targets: vec![vec![Target::Player(0)]] }))).unwrap();
    assert_eq!(g.players[0].pool.total(), 0);
}

#[test]
fn r700_2b_up_to_one_mode_with_none_chosen_removes_the_trigger() {
    // "At the beginning of combat on your turn, choose up to one — • Tap target creature.
    // • Target creature doesn't untap during its controller's next untap step."
    let mut g = game();
    g.put(0, GEIST, Zone::Battlefield);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    while g.decision.is_none() { let p = g.priority; g.act(p, Move::Pass).unwrap(); }
    assert_eq!(g.part, ir::Part::BeginningOfCombat);
    let Some((0, Asked::Modes { options, .. })) = g.asked() else { panic!("the modes are asked: {:?}", g.decision) };
    let options: Vec<_> = options.into_iter().map(|(m, _)| m).collect();
    cite!(["700.2b"]; assert!(options.contains(&vec![]) && options.contains(&vec![0]) && !options.contains(&vec![0, 1]), "{options:?}"));
    g.act(0, modal(&[], Move::ChooseTargets(vec![]))).unwrap();
    cite!(["700.2b", "603.3c"]; assert!(g.stack.is_empty() && g.decision.is_none(), "removed: nothing on the stack"));
    assert!(!g.obj(bears).tapped);
}

#[test]
fn r700_2b_up_to_one_mode_chosen_taps_the_target() {
    let mut g = game();
    g.put(0, GEIST, Zone::Battlefield);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    while g.decision.is_none() { let p = g.priority; g.act(p, Move::Pass).unwrap(); }
    g.act(0, modal(&[0], Move::ChooseTargets(vec![vec![Target::Object(bears)]]))).unwrap();
    assert_eq!(g.stack.len(), 1);
    pass(&mut g);
    assert!(g.obj(bears).tapped);
}

#[test]
fn r700_2g_a_copy_of_a_spree_spell_has_its_modes_and_pays_nothing() {
    // Twincast: "Copy target instant or sorcery spell. You may choose new targets for the copy."
    let mut g = game();
    let steps = g.put(0, STEPS, Zone::Hand);
    let discard = g.put(0, ISLAND, Zone::Hand);
    let twincast = g.put(0, TWINCAST, Zone::Hand);
    pool(&mut g, colored(U, 1), 2);
    g.act(0, modal(&[2], paid("CCU",Move::Cast { card: steps, targets: vec![] }))).unwrap();
    let spell = *g.stack.last().unwrap();
    pool(&mut g, colored(U, 2), 0);
    g.act(0, paid("UU",Move::Cast { card: twincast, targets: vec![vec![Target::Object(spell)]] })).unwrap();
    pass(&mut g);
    while g.decision.is_some() { let Some(Decision::Choose { player, .. }) = g.decision.clone() else { panic!("{:?}", g.decision) }; g.act(player, Move::Choose(Answer::No)).unwrap(); }
    let copy = *g.stack.last().unwrap();
    assert_ne!(copy, spell);
    cite!(["700.2g", "707.10"]; assert_eq!(g.obj(copy).modes, vec![2]));
    cite!(["707.10"]; assert_eq!(g.players[0].pool.total(), 0));
    // the copy draws two, then discards one
    let hand = g.players[0].hand.len();
    pass(&mut g);
    let Some(Decision::Choose { player: 0, choice: Choice::Objects { among, .. } }) = g.decision.clone() else { panic!("the copy's discard: {:?}", g.decision) };
    assert!(among.contains(&discard));
    g.act(0, Move::Choose(Answer::Objects(vec![discard]))).unwrap();
    assert_eq!(g.players[0].hand.len(), hand + 1);
}

#[test]
fn r202_3_the_mana_value_on_the_stack_excludes_mode_costs() {
    // Three Steps Ahead with two modes, paid for six mana: its mana value is that of {U}
    let mut g = game();
    let mine = g.put(0, BEARS, Zone::Battlefield);
    let steps = g.put(0, STEPS, Zone::Hand);
    let snip = g.put(1, SNIP, Zone::Hand);
    pool(&mut g, colored(U, 1), 5);
    g.act(0, paid("CCCCCU", modal(&[1, 2], Move::Cast { card: steps, targets: vec![vec![Target::Object(mine)]] }))).unwrap();
    let spell = *g.stack.last().unwrap();
    g.act(0, Move::Pass).unwrap();
    // "Counter target spell with mana value 1 or less."
    g.players[1].pool.colors = colored(U, 1);
    g.act(1, paid("U", Move::Cast { card: snip, targets: vec![vec![Target::Object(spell)]] })).unwrap();
    pass(&mut g);
    cite!(["202.3", "700.2h"]; assert_eq!(g.stack.len(), 0, "countered: mana value 1"));
    assert_eq!(g.battlefield.iter().filter(|o| g.obj(**o).chars.name == "Grizzly Bears").count(), 1, "no copy was made");
    // and a spell of mana value 2 can't be its target
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Hand);
    let snip = g.put(1, SNIP, Zone::Hand);
    pool(&mut g, colored(G, 1), 1);
    g.act(0, paid("CG", Move::Cast { card: bears, targets: vec![] })).unwrap();
    let spell = *g.stack.last().unwrap();
    g.act(0, Move::Pass).unwrap();
    g.players[1].pool.colors = colored(U, 1);
    cite!(["202.3", "601.2c"]; refused("an illegal target, Grizzly Bears"): act_unchanged(&mut g, 1, paid("U", Move::Cast { card: snip, targets: vec![vec![Target::Object(spell)]] })));
}

#[test]
fn r700_2a_a_spree_mode_without_a_legal_target_is_not_offered() {
    // Requisition Raid: "+ {1} — Destroy target artifact. + {1} — Destroy target enchantment.
    // + {1} — Put a +1/+1 counter on each creature target player controls."
    let mut g = game();
    let raid = g.put(0, RAID, Zone::Hand);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    pool(&mut g, colored(W, 1), 3);
    g.act(0, modal(&[2], paid("CW",Move::Cast { card: raid, targets: vec![vec![Target::Player(0)]] }))).unwrap();
    cite!(["700.2h"]; assert_eq!(g.players[0].pool.total(), 2, "{{W}} and one {{1}}"));
    pass(&mut g);
    assert_eq!(g.power(bears), Some(3));
}

#[test]
fn r700_2i_season_of_weaving_spends_up_to_five_pawprints() {
    // "Choose up to five {P} worth of modes. You may choose the same mode more than once.
    // {P} — Draw a card. {P}{P} — Choose an artifact or creature you control. Create a token
    // that's a copy of it. {P}{P}{P} — Return each nonland, nontoken permanent to its owner's hand."
    let mut g = game();
    let weaving = g.put(0, WEAVING, Zone::Hand);
    pool(&mut g, colored(U, 2), 4);
    for modes in [vec![], vec![0, 0, 0, 1], vec![1, 2]] {
        cite!(["700.2i", "700.2d"]; case: format!("{modes:?}"); ok: g.clone().act(0, paid("CCCCUU", modal(&modes, Move::Cast { card: weaving, targets: vec![] }))));
    }
    for modes in [vec![1, 1, 1], vec![0, 1, 2]] {
        cite!(["700.2i"]; case: format!("{modes:?}"); refused("choose modes worth no more {P} than its text allows"): act_unchanged(&mut g, 0, paid("CCCCUU", modal(&modes, Move::Cast { card: weaving, targets: vec![] }))));
    }
    cite!(["700.2i"]; refused("choose modes worth no more {P} than its text allows"): act_unchanged(&mut g, 0, paid("CCCCUU", modal(&[2, 2], Move::Cast { card: weaving, targets: vec![] }))));
    let hand = g.players[0].hand.len();
    g.act(0, paid("CCCCUU", modal(&[0, 0, 0, 0, 0], Move::Cast { card: weaving, targets: vec![] }))).unwrap();
    cite!(["700.2i"]; assert_eq!(g.players[0].pool.total(), 0, "the mana cost alone"));
    pass(&mut g);
    cite!(["700.2d"]; assert_eq!(g.players[0].hand.len(), hand - 1 + 5, "the draw mode five times"));
}

#[test]
fn r700_2i_season_of_weaving_follows_its_modes_in_printed_order() {
    // {P}{P} then {P}{P}{P}: the token copy is made, then each nonland, nontoken permanent
    // returns; the token stays (608.2c)
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let weaving = g.put(0, WEAVING, Zone::Hand);
    pool(&mut g, colored(U, 2), 4);
    g.act(0, modal(&[2, 1], paid("CCCCUU",Move::Cast { card: weaving, targets: vec![] }))).unwrap();
    pass(&mut g);
    if let Some(Decision::Choose { player: 0, choice: Choice::Objects { among, .. } }) = g.decision.clone() {
        assert_eq!(among, vec![bears]);
        g.act(0, Move::Choose(Answer::Objects(vec![bears]))).unwrap();
    }
    let left: Vec<_> = g.battlefield.iter().filter(|o| g.obj(**o).chars.name == "Grizzly Bears").map(|o| g.obj(*o).token()).collect();
    cite!(["608.2c", "700.2d"]; assert_eq!(left, vec![true], "the token copy stays; the Bears return"));
    cite!(["608.2c"]; assert!(g.players[0].hand.iter().any(|o| g.obj(*o).chars.name == "Grizzly Bears")));
}

#[track_caller]
fn act_unchanged(g: &mut Game, player: usize, action: Move) -> Result<(), String> {
    let before = format!("{g:?}");
    let result = g.act(player, action);
    assert_eq!(format!("{g:?}"), before);
    result
}
