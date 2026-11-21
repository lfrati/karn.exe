//! Independent evoke composition: copies, new incarnations and current control.
use crate::support::cite::cite;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use game::announcement::CastCost;
use game::state::Decision;
use ir::Part;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap();
    let defs = vec![real("Solitude"), real("Ephemerate"),
        CardDef::new("Echo review", "{0}", "Instant", "Copy target spell.", None).unwrap(),
        real("Grizzly Bears"), real("Plains"), real("Island"), real("Swords to Plowshares"), real("Mulldrifter"),
        CardDef::new("Control review", "{0}", "Instant", "Gain control of target creature.", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, 5, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn settle(g: &mut Game, target: Option<ObjId>) {
    for _ in 0..80 {
        match g.decision.clone() {
            Some(Decision::Order { player, abilities }) => g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(),
            Some(Decision::Targets { player, .. }) => g.act(player, Move::ChooseTargets(vec![target.map(Target::Object).into_iter().collect()])).unwrap(),
            Some(d) => panic!("unexpected {d:?}"),
            None if g.stack.is_empty() => return,
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("stack did not settle");
}
fn evoke_solitude(g: &mut Game, target: ObjId) -> ObjId {
    let card = g.put(0, 0, Zone::Hand);
    let white = g.put(0, 6, Zone::Hand);
    g.act(0, Move::CastFor(CastCost::Alternative(3), Box::new(Move::WithCosts(vec![vec![white]], Box::new(Move::Cast { card, targets: vec![] }))))).unwrap();
    while g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
    let Some(Decision::Order { player: 0, abilities }) = g.decision.clone() else { panic!("{:?}", g.decision) };
    g.act(0, Move::Order((0..abilities.len()).collect())).unwrap();
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(target)]])).unwrap();
    *g.battlefield.iter().find(|o| g.obj(**o).card == Some(0)).unwrap()
}
#[test]
fn r400_7_blinking_an_evoked_solitude_does_not_evoke_the_new_incarnation() {
    // Rules: 400.7, 702.74a, 603.4
    let mut g = game();
    let bear = g.put(1, 3, Zone::Battlefield);
    let old = evoke_solitude(&mut g, bear);
    let blink = g.put(0, 1, Zone::Hand);
    g.players[0].pool.colors[0] = 1;
    g.act(0, crate::support::paid("W",Move::Cast { card: blink, targets: vec![vec![Target::Object(old)]] })).unwrap();
    settle(&mut g, Some(bear));
    let now: Vec<_> = g.battlefield.iter().filter(|o| g.obj(**o).card == Some(0)).copied().collect();
    cite!(["400.7", "702.74a", "603.4"]; assert_eq!(now.len(), 1));
    cite!(["400.7"]; assert_ne!(now[0], old));
    // Solitude's own text (the exiled Bears' controller gains 2) resolved once: uncited
    assert_eq!(g.players[1].life, 22);
    cite!(["400.7", "702.74a"]; assert!(g.players[0].graveyard.iter().all(|o| g.obj(*o).card != Some(0)), "old evoke trigger cannot sacrifice the returned object"));
}
#[test]
fn r702_74a_its_current_controller_sacrifices_the_evoked_permanent() {
    // Rules: 702.74a, 701.21a, 400.3
    let mut g = game();
    let bear = g.put(1, 3, Zone::Battlefield);
    let solitude = evoke_solitude(&mut g, bear);
    g.act(0, Move::Pass).unwrap();
    let take = g.put(1, 8, Zone::Hand);
    g.act(1, Move::Cast { card: take, targets: vec![vec![Target::Object(solitude)]] }).unwrap();
    settle(&mut g, Some(bear));
    cite!(["702.74a", "701.21a"]; assert!(g.log.iter().any(|e| matches!(e, Event::Sacrificed { player: 1, object, .. } if *object == solitude))));
    cite!(["701.21a", "400.3"]; assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).card == Some(0))));
    cite!(["702.74a", "701.21a"]; assert!(g.battlefield.iter().all(|o| g.obj(*o).card != Some(0))));
}
#[test]
fn r707_10_an_evoked_creature_spell_copy_keeps_the_alternative_choice() {
    // Rules: 707.10, 707.10f, 702.74a
    let mut g = game();
    let original = g.put(0, 7, Zone::Hand);
    g.players[0].pool.colors[1] = 3;
    g.act(0, crate::support::paid("UUU",Move::CastFor(CastCost::Alternative(2), Box::new(Move::Cast { card: original, targets: vec![] })))).unwrap();
    let spell = *g.stack.last().unwrap();
    g.act(0, Move::Pass).unwrap();
    let echo = g.put(1, 2, Zone::Hand);
    g.act(1, Move::Cast { card: echo, targets: vec![vec![Target::Object(spell)]] }).unwrap();
    settle(&mut g, None);
    cite!(["121.1", "603.6a"]; assert_eq!(g.players[0].hand.len(), 2));
    cite!(["707.10f", "603.6a", "121.1"]; assert_eq!(g.players[1].hand.len(), 2));
    cite!(["707.10", "702.74a"]; assert!(g.battlefield.iter().all(|o| g.obj(*o).chars.name != "Mulldrifter")));
    let sacrificed: Vec<_> = g.log.iter().filter_map(|e| if let Event::Sacrificed { player, .. } = e { Some(*player) } else { None }).collect();
    cite!(["707.10", "707.10f", "702.74a"]; assert_eq!(sacrificed, vec![1, 0], "copy's controller sacrifices its token before original resolves"));
}
#[test]
fn r702_74_granted_evoke_and_evoke_on_an_instant_stay_refused() {
    // These are implementation admission limits, not prohibitions imposed by 702.74a.
    // Keep these admission contracts uncited.
    let granted = CardDef::new("Grant review", "{0}", "Enchantment", "Creatures you control have evoke {1}.", None).unwrap();
    crate::support::refused(game::play::check(&granted).map_err(|e|e.0),"keyword: Evoke");
    let instant = CardDef::new("Instant review", "{0}", "Instant", "You gain 1 life.\nEvoke {0}", None).unwrap();
    crate::support::refused(game::play::check(&instant).map_err(|e|e.0),"keyword: Evoke");
}
#[test]
fn r113_2c_a_second_printed_evoke_cost_has_its_own_sacrifice_condition() {
    // Rules: 113.2c, 702.74a
    let def = CardDef::new("Twin review", "{5}", "Creature", "Evoke {0}\nEvoke {1}", Some((2, 2))).unwrap();
    game::play::check(&def).unwrap();
    for row in [0, 1] {
        let mut g = Game::new(vec![def.clone()], 2, 20);
        while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
        let card = g.put(0, 0, Zone::Hand);
        g.players[0].pool.colorless = 1;
        g.act(0, crate::support::paid(if row == 0 { "" } else { "C" },Move::CastFor(CastCost::Alternative(row), Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
        settle(&mut g, None);
        cite!(["113.2c", "702.74a"]; case: format!("evoke row {row}"); assert!(g.battlefield.is_empty(), "paying evoke row {row} must sacrifice the creature"));
        cite!(["113.2c", "702.74a", "701.21a"]; case: format!("evoke row {row}"); assert_eq!(g.players[0].graveyard.len(), 1));
    }
}
