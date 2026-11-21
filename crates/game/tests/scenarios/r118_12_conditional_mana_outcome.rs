//! Conditional optional payments export one false outcome when their branch is skipped.
//! Pinned Weapons Vendor and Flamewake Phoenix exercise reflexive and ordinary riders.
use crate::support::cite;
use game::legal::Asked;
use game::state::{Answer, Pool};
use game::{CardDef, Game, Move, Target, Zone};

fn pinned(name: &str) -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let card = CardDef::from_faces(catalog.get(name).unwrap()).unwrap();
    game::play::check(&card).unwrap_or_else(|e| panic!("{name}: {}", e.0));
    card
}
fn pass(g: &mut Game) {
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
}
fn pay(g: &mut Game, with: Pool) {
    g.act(0, Move::Choose(Answer::Pay { plan: vec![], with: Some(with), tapped: vec![] })).unwrap();
}

#[test]
fn r118_12_weapons_vendor_conditional_payment_attaches_only_after_payment_and_reflexive_targets() {
    for (eligible, loses_condition, paid) in [(false, false, false), (true, true, false), (true, false, false), (true, false, true)] {
        let body = CardDef::new("Body", "{0}", "Creature", "", Some((2, 2))).unwrap();
        let mut g = Game::new(vec![pinned("Weapons Vendor"), pinned("Short Sword"), body], 2, 20);
        g.put(0, 0, Zone::Battlefield);
        let body = g.put(0, 2, Zone::Battlefield);
        let equipment = eligible.then(|| g.put(0, 1, Zone::Battlefield));
        pass(&mut g); // Beginning of combat (507.1), where the intervening-if trigger checks.
        if !eligible {
            cite!(["603.4"]; assert!(g.stack.is_empty() && g.decision.is_none()));
            continue;
        }
        assert_eq!(g.stack.len(), 1);
        if loses_condition { g.move_object(equipment.unwrap(), Zone::Graveyard); }
        g.players[0].pool.colorless = 1;
        pass(&mut g);
        if loses_condition {
            cite!(["603.4"]; assert!(g.stack.is_empty() && g.decision.is_none()));
            assert_eq!(g.players[0].pool.colorless, 1);
            continue;
        }
        assert!(matches!(g.asked(), Some((0, Asked::Pay { .. }))));
        if !paid {
            g.act(0, Move::Choose(Answer::No)).unwrap();
            cite!(["118.12", "603.12"]; assert!(g.stack.is_empty() && g.decision.is_none()));
            assert_eq!(g.players[0].pool.colorless, 1);
            assert_eq!(g.attached_to(equipment.unwrap()), None);
            continue;
        }
        pay(&mut g, Pool { colorless: 1, ..Pool::default() });
        assert_eq!(g.players[0].pool.total(), 0);
        cite!(["603.12"]; assert!(matches!(g.asked(), Some((0, Asked::Targets { .. })))));
        let equipment = equipment.unwrap();
        g.act(0, Move::ChooseTargets(vec![vec![Target::Object(equipment)], vec![Target::Object(body)]])).unwrap();
        assert_eq!(g.attached_to(equipment), None, "the reflexive trigger has not resolved");
        pass(&mut g);
        cite!(["603.12", "701.3"]; assert_eq!(g.attached_to(equipment), Some(Target::Object(body))));
    }
}

#[test]
fn r118_12_flamewake_phoenix_ferocious_payment_returns_only_after_the_intervening_condition_and_payment() {
    for (eligible, loses_condition, paid) in [(false, false, false), (true, true, false), (true, false, false), (true, false, true)] {
        let body = CardDef::new("Large body", "{0}", "Creature", "", Some((4, 4))).unwrap();
        let mut g = Game::new(vec![pinned("Flamewake Phoenix"), body], 2, 20);
        let phoenix = g.put(0, 0, Zone::Graveyard);
        let body = eligible.then(|| g.put(0, 1, Zone::Battlefield));
        pass(&mut g);
        if !eligible {
            cite!(["603.4"]; assert!(g.stack.is_empty() && g.decision.is_none()));
            assert!(g.players[0].graveyard.contains(&phoenix));
            continue;
        }
        assert_eq!(g.stack.len(), 1);
        if loses_condition { g.move_object(body.unwrap(), Zone::Graveyard); }
        g.players[0].pool.colors[ir::Color::Red as usize] = 1;
        pass(&mut g);
        if loses_condition {
            cite!(["603.4"]; assert!(g.stack.is_empty() && g.decision.is_none()));
            assert!(g.players[0].graveyard.contains(&phoenix));
            assert_eq!(g.players[0].pool.total(), 1);
            continue;
        }
        assert!(matches!(g.asked(), Some((0, Asked::Pay { .. }))));
        if paid { pay(&mut g, Pool { colors: [0,0,0,1,0], ..Pool::default() }); } else { g.act(0, Move::Choose(Answer::No)).unwrap(); }
        cite!(["118.12"]; assert_eq!(g.players[0].graveyard.contains(&phoenix), !paid));
        assert_eq!(g.players[0].pool.total(), if paid { 0 } else { 1 });
        assert_eq!(g.battlefield.iter().filter(|o| g.obj(**o).card == Some(0)).count(), usize::from(paid));
        assert!(g.stack.is_empty() && g.decision.is_none());
    }
}
