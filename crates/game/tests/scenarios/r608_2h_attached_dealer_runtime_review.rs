//! An attached dealer's "other"
//! exclusion leaves the existing earlier-target relation in charge. Run with
//! `r608::r608_2h_attached_dealer_runtime_review::`.
use crate::support::cite;
use game::state::Decision;
use game::{CardDef, Event, Game, Move, Target, Zone};

/// "Any target and … any other target" (Arc Trail templating): the second "other" is
/// distinctness from the first target, not from the dealer. The enchanted creature may be
/// either target, but not both.
#[test]
fn second_other_target_is_distinct_from_the_first_not_from_the_attached_dealer() {
    let text = "Enchant creature\nWhen this Aura enters, enchanted creature deals 2 damage to any target and 1 damage to any other target.";
    let aura = CardDef::new("Probe", "{0}", "Enchantment — Aura", text, None).unwrap();
    assert_eq!(game::play::check(&aura).map_err(|e| e.0), Ok(()));
    let bear = |g: &mut Game| g.put(0, 1, Zone::Battlefield);
    for (choose, legal) in [(0, false), (1, true), (2, true)] {
        let mut g = Game::new(vec![aura.clone(), CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((3, 9))).unwrap()], 2, 20);
        let b = bear(&mut g);
        let c = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card: c, targets: vec![vec![Target::Object(b)]] }).unwrap();
        for _ in 0..6 { if matches!(g.decision, Some(Decision::Targets { .. })) { break; } g.act(g.priority, Move::Pass).unwrap(); }
        let groups = match choose {
            0 => vec![vec![Target::Object(b)], vec![Target::Object(b)]],
            1 => vec![vec![Target::Object(b)], vec![Target::Player(1)]],
            _ => vec![vec![Target::Player(1)], vec![Target::Object(b)]],
        };
        cite!(["601.2c", "603.3d"]; case: format!("{groups:?}"); assert_eq!(g.act(0, Move::ChooseTargets(groups.clone())).is_ok(), legal, "{groups:?}"));
        if !legal { continue; }
        for _ in 0..6 { if g.stack.is_empty() { break; } g.act(g.priority, Move::Pass).unwrap(); }
        let mut dealt: Vec<_> = g.log.iter().filter_map(|e| match e { Event::DamageDealt { source, to, amount, .. } => Some((*source, *to, *amount)), _ => None }).collect();
        dealt.sort_by_key(|d| format!("{d:?}"));
        let mut want = vec![(b, groups[0][0], 2), (b, groups[1][0], 1)];
        want.sort_by_key(|d| format!("{d:?}"));
        cite!(["608.2h"]; case: format!("{groups:?}"); assert_eq!(dealt, want, "{groups:?}"));
    }
}
