//! A bare "it" damage recipient
//! may name a mixed (object-or-player) target, never a player-only one, and the bound target's
//! player possibility feeds the member-amount guard per hit. Run with
//! `r608::r608_2k_retained_recipient_runtime_review::`.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};

fn spell(text: &str) -> CardDef { CardDef::new("Probe", "{0}", "Sorcery", text, None).unwrap() }
fn admitted(text: &str) -> Result<(), String> { game::play::check(&spell(text)).map_err(|e| e.0) }
fn bear(name: &str, p: i32) -> CardDef { CardDef::new(name, "{0}", "Creature — Bear", "", Some((p, 9))).unwrap() }

/// Mine: Big 3/9 and Small 2/9; the opponent's Wall 1/9. Targets by letter: P the opponent,
/// O the Wall, M my Big. Returns the damage log, (Big, Small, Wall, spell) and life totals.
fn run(text: &str, targets: &str) -> (Vec<(ObjId, Target, u32)>, [ObjId; 4], Vec<i32>) {
    let mut g = Game::new(vec![spell(text), bear("Big", 3), bear("Small", 2), bear("Wall", 1)], 2, 20);
    let (a, b, w) = (g.put(0, 1, Zone::Battlefield), g.put(0, 2, Zone::Battlefield), g.put(1, 3, Zone::Battlefield));
    let c = g.put(0, 0, Zone::Hand);
    let targets = targets.chars().map(|k| vec![match k { 'P' => Target::Player(1), 'O' => Target::Object(w), _ => Target::Object(a) }]).collect();
    g.act(0, Move::Cast { card: c, targets }).unwrap();
    let s = *g.stack.last().unwrap();
    for _ in 0..3 { if g.stack.is_empty() { break; } g.act(g.priority, Move::Pass).unwrap(); }
    let dealt = g.log.iter().filter_map(|e| match e {
        Event::DamageDealt { source, to, amount, .. } => Some((*source, *to, *amount)),
        _ => None,
    }).collect();
    (dealt, [a, b, w, s], g.players.iter().map(|p| p.life).collect())
}

/// The singular target dealer is excluded before narrowing and deals its own
/// power to the earlier mixed target; the guard is per hit and per bound producer.
#[test]
fn dealer_exclusion_and_per_hit_guard() {
    let text = "This spell deals 1 damage to any target. Target creature you control deals damage equal to its power to it.";
    let (dealt, [a, _, _, s], life) = run(text, "PM");
    cite!(["608.2c"]; assert_eq!((dealt, life), (vec![(s, Target::Player(1), 1), (a, Target::Player(1), 3)], vec![20, 16])));
    // Member amount in a hit whose bound recipient may be a player: refused (admitted, it
    // would deal nothing to "it").
    assert_eq!(admitted("This spell deals 1 damage to any target. Each creature you control deals damage equal to its power to it and 1 damage to you."),
               Err("per-source object quantity in damage a player may be dealt".into()));
    // A fixed amount to "it" beside a member amount to creatures: each hit keeps its own guard.
    let text = "This spell deals 1 damage to any target. Each creature you control deals 1 damage to it and damage equal to its power to each creature your opponents control.";
    let (dealt, [a, b, w, s], life) = run(text, "P");
    cite!(["608.2c"]; assert_eq!(dealt, vec![(s, Target::Player(1), 1), (a, Target::Player(1), 1), (b, Target::Player(1), 1),
                           (a, Target::Object(w), 3), (b, Target::Object(w), 2)]));
    assert_eq!(life, vec![20, 17]);
    // "it" inside the dealer group is fine: the tapped creature is dealt damage by each, itself included.
    let (dealt, [a, b, _, _], _) = run("Tap target creature you control. Each creature you control deals 1 damage to it.", "M");
    cite!(["608.2c"]; assert_eq!(dealt, vec![(a, Target::Object(a), 1), (b, Target::Object(a), 1)]));
}
