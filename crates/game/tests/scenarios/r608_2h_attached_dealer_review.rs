//! An attached object as the acting
//! clause subject. Run with `r608::r608_2h_attached_dealer_review::`.
//!
//! - The possessor and "other" cases.
//! - The fixed-amount contrasts. They show the dealer side follows the
//!   Aura's last attachment when the Aura or the creature leaves before resolution.
use crate::support::{cite, paid};
use game::state::Decision;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};

fn dealt(g: &Game) -> Vec<(ObjId, Target, u32)> {
    g.log.iter().filter_map(|e| match e { Event::DamageDealt { source, to, amount, .. } => Some((*source, *to, *amount)), _ => None }).collect()
}
fn pain_for_all() -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    CardDef::from_faces(CardDef::catalog(&faces).get("Pain for All").unwrap()).unwrap()
}
fn aura(text: &str) -> CardDef { CardDef::new("Probe", "{0}", "Enchantment — Aura", text, None).unwrap() }

/// Player 0 enchants a 3/9 Bear (controlled by `holder`) and chooses `choice` for the
/// enters trigger. If given, player 1 answers the trigger with an instant aimed at the Bear
/// or, for "enchantment", at the Aura.
struct Run { g: Game, bear: ObjId, wall: ObjId, chose: Result<(), String> }
fn run(aura: CardDef, spending: &str, holder: usize, response: Option<&str>, choice: impl Fn(ObjId, ObjId) -> Target) -> Run {
    let mut defs = vec![aura, CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((3, 9))).unwrap(),
                        CardDef::new("Wall", "{0}", "Creature — Wall", "", Some((0, 9))).unwrap()];
    if let Some(text) = response { defs.push(CardDef::new("Answer", "{0}", "Instant", text, None).unwrap()); }
    let mut g = Game::new(defs, 2, 20);
    g.players[0].pool.colors = [10; 5];
    let bear = g.put(holder, 1, Zone::Battlefield);
    let wall = g.put(1, 2, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand);
    let answer = response.map(|_| g.put(1, 3, Zone::Hand));
    g.act(0, paid(spending, Move::Cast { card, targets: vec![vec![Target::Object(bear)]] })).unwrap();
    let (mut chose, mut answered) = (Ok(()), answer.is_none());
    for _ in 0..30 {
        if let Some(Decision::Targets { .. }) = g.decision {
            if let Err(e) = g.act(0, Move::ChooseTargets(vec![vec![choice(bear, wall)]])) {
                chose = Err(e);
                g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap();
            }
            continue;
        }
        if g.stack.is_empty() { break; }
        // The enters trigger is alone on the stack once the Aura is on the battlefield.
        if !answered && g.priority == 1 && g.stack.len() == 1 && g.battlefield.len() == 3 {
            let aimed = if response.unwrap().contains("enchantment") { g.battlefield.iter().copied().find(|o| ![bear, wall].contains(o)).unwrap() } else { bear };
            g.act(1, Move::Cast { card: answer.unwrap(), targets: vec![vec![Target::Object(aimed)]] }).unwrap();
            answered = true;
            continue;
        }
        g.act(g.priority, Move::Pass).unwrap();
    }
    assert!(answered, "the answer was cast while the trigger waited");
    Run { g, bear, wall, chose }
}
const KILL: Option<&str> = Some("Destroy target creature.");
const DISENCHANT: Option<&str> = Some("Destroy target enchantment.");
const PUMP: Option<&str> = Some("Target creature gets +3/+3 until end of turn.");

/// "its power" is the enchanted creature's, not the entering Aura's (which would deal nothing).
/// - Pain for All ruling 57c043733dccd779: if the creature leaves first, its last-known power
///   is used.
/// - The Aura's own departure uses the creature it last enchanted (Hydra's Growth, Warbriar
///   Blessing and Cartouche of Strength rulings).
/// - A pump in response is read at resolution (608.2h).
#[test]
fn pain_for_all_reads_the_enchanted_creatures_power_at_resolution() {
    for (response, amount) in [(None, 3), (KILL, 3), (DISENCHANT, 3), (PUMP, 6)] {
        let r = run(pain_for_all(), "RRR", 0, response, |_, _| Target::Player(1));
        cite!(["608.2h"]; case: format!("{response:?}"); assert_eq!(dealt(&r.g), vec![(r.bear, Target::Player(1), amount)], "{response:?}"));
        assert_eq!(r.g.players[1].life, 20 - amount as i32, "{response:?}");
    }
}

/// Separate from whose power "its power" reads: the dealer is
/// the enchanted creature, so "any other target" excludes that creature, not the Aura.
/// The target spec's `other` keeps its source default (`other_than: None`). Contrast:
/// Predatory Urge rulings 46f134e183ab3a92 and 4090488d9a15eea9 allow self-targeting only
/// because the text lacks "other".
#[test]
fn pain_for_all_cannot_target_the_enchanted_creature() {
    let r = run(pain_for_all(), "RRR", 0, None, |bear, _| Target::Object(bear));
    cite!(["115.1"]; refused("an illegal target, Bear: it is the object the target must be other than (115.1)"): r.chose);
    assert_eq!(dealt(&r.g), vec![(r.bear, Target::Player(1), 3)]);
}

/// "its controller" and "its owner" are not the Aura's controller and owner.
/// They must belong to the enchanted creature, here the opponent's Bear.
#[test]
fn attached_dealer_owns_its_controller_owner_and_statistics() {
    let r = run(aura("Enchant creature\nWhen this Aura enters, enchanted creature deals 1 damage to its controller."), "", 1, None, |_, _| unreachable!());
    cite!(["608.2c"]; assert_eq!(dealt(&r.g), vec![(r.bear, Target::Player(1), 1)]));
    let r = run(aura("Enchant creature\nWhen this Aura enters, enchanted creature deals damage equal to its toughness to its owner."), "", 1, None, |_, _| unreachable!());
    cite!(["608.2c", "608.2h"]; assert_eq!(dealt(&r.g), vec![(r.bear, Target::Player(1), 9)]));
    let r = run(aura("Enchant creature\nWhen this Aura enters, enchanted creature deals damage equal to its power to target creature."), "", 1, None, |_, wall| Target::Object(wall));
    cite!(["608.2h"]; assert_eq!(dealt(&r.g), vec![(r.bear, Target::Object(r.wall), 3)]));
}

/// Contrast: with a fixed amount the attached dealer acts as the rules require.
/// - It still deals the damage after the creature leaves (last known information).
/// - It still deals it after the Aura leaves (the creature it last enchanted).
/// - A non-target "each other creature" excludes that dealer.
#[test]
fn attached_dealer_survives_departures_and_excludes_itself_from_each_other() {
    let fixed = || aura("Enchant creature you control\nWhen this Aura enters, enchanted creature deals 2 damage to any other target.");
    for response in [None, KILL, DISENCHANT, PUMP] {
        let r = run(fixed(), "", 0, response, |_, _| Target::Player(1));
        cite!(["608.2h"]; case: format!("{response:?}"); assert_eq!(dealt(&r.g), vec![(r.bear, Target::Player(1), 2)], "{response:?}"));
    }
    let r = run(aura("Enchant creature\nWhen this Aura enters, enchanted creature deals 2 damage to each other creature."), "", 0, None, |_, _| unreachable!());
    cite!(["608.2c"]; assert_eq!(dealt(&r.g), vec![(r.bear, Target::Object(r.wall), 2)]));
}

/// Without "other", the attached dealer may target itself (Predatory Urge
/// ruling contrast). The quantity still belongs to the dealer, not the Aura.
#[test]
fn attached_dealer_can_target_itself_without_other() {
    let r = run(aura("Enchant creature\nWhen this Aura enters, enchanted creature deals damage equal to its power to target creature."), "", 0, None, |bear, _| Target::Object(bear));
    assert!(r.chose.is_ok());
    cite!(["608.2h"]; assert_eq!(dealt(&r.g), vec![(r.bear, Target::Object(r.bear), 3)]));
}

/// 608.2b: the same Attached exclusion is checked at resolution. A low-level fixture
/// attachment change exercises the legality boundary; this does not claim an admitted
/// instant-speed attachment mover. Clone retains the independent original relation.
#[test]
fn other_target_rechecks_attachment_and_clone_keeps_its_relation() {
    let d = aura("Enchant creature\nWhen this Aura enters, enchanted creature deals 2 damage to any other target.");
    let mut g = Game::new(vec![d,
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((3, 9))).unwrap(),
        CardDef::new("Wall", "{0}", "Creature — Wall", "", Some((0, 9))).unwrap()], 2, 20);
    let bear = g.put(0, 1, Zone::Battlefield);
    let wall = g.put(1, 2, Zone::Battlefield);
    let c = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: c, targets: vec![vec![Target::Object(bear)]] }).unwrap();
    for _ in 0..6 {
        if matches!(g.decision, Some(Decision::Targets { .. })) {
            g.act(0, Move::ChooseTargets(vec![vec![Target::Object(wall)]])).unwrap();
            break;
        }
        g.act(g.priority, Move::Pass).unwrap();
    }
    assert!(!g.stack.is_empty(), "trigger waits with the legal target announced");
    let source = *g.battlefield.iter().find(|o| g.obj(**o).chars.name == "Probe").unwrap();
    let mut original_relation = g.clone();
    assert!(g.attach(source, Target::Object(wall)));
    for branch in [&mut g, &mut original_relation] {
        for _ in 0..4 { if branch.stack.is_empty() { break; } branch.act(branch.priority, Move::Pass).unwrap(); }
        assert!(branch.stack.is_empty());
    }
    cite!(["608.2b"]; assert!(dealt(&g).is_empty(), "the sole target is now its own excluded dealer"));
    assert_eq!(dealt(&original_relation), vec![(bear, Target::Object(wall), 2)]);
}
