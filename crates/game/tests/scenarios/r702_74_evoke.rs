//! Evoke (702.74a): "You may cast this spell by paying [cost] rather than paying its mana
//! cost" and "When this permanent enters, if its evoke cost was paid, its controller
//! sacrifices it". Paying it is casting for an alternative cost (118.9); the sacrifice is one
//! enters trigger among its others, in the order its controller chooses (603.3b).
use crate::support::cite;
use game::announcement::CastCost;
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const MULLDRIFTER: usize = 0;
const SOLITUDE: usize = 1;
const BEAR: usize = 2;
const ISLAND: usize = 3;
const WHITE: usize = 4;
const SPELL: usize = 5;
// the rows of their evoke keywords
const MULLDRIFTER_EVOKE: usize = 2;
const SOLITUDE_EVOKE: usize = 3;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap();
    let c = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    let defs = vec![
        real("Mulldrifter"),
        real("Solitude"),
        c("Bear", "{0}", "Creature — Bear", "", Some((3, 2))),
        c("Island", "", "Basic Land — Island", "", None),
        c("White Card", "{W}", "Instant", "You gain 1 life.", None),
        c("Spell", "{0}", "Instant", "You gain 1 life.", None),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}

/// Resolve everything; triggers target `target`; simultaneous triggers are put on the stack
/// in the order listed, or reversed.
fn settle(g: &mut Game, target: Option<ObjId>, reversed: bool) {
    for _ in 0..50 {
        match g.decision.clone() {
            Some(Decision::Order { player, abilities }) => {
                let mut order: Vec<usize> = (0..abilities.len()).collect();
                if reversed { order.reverse(); }
                g.act(player, Move::Order(order)).unwrap();
            }
            Some(Decision::Targets { .. }) => g.act(0, Move::ChooseTargets(vec![target.map(Target::Object).into_iter().collect()])).unwrap(),
            Some(d) => panic!("unexpected decision {d:?}"),
            None if g.stack.is_empty() => return,
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("did not settle");
}
fn count(g: &Game, zone: &[ObjId], card: usize) -> usize { zone.iter().filter(|o| g.obj(**o).card == Some(card)).count() }

#[test]
fn r702_74a_evoked_mulldrifter_draws_two_and_is_sacrificed() {
    for reversed in [false, true] {
        let mut g = game();
        let card = g.put(0, MULLDRIFTER, Zone::Hand);
        g.players[0].pool.colors[1] = 1;
        g.players[0].pool.colorless = 2;
        g.act(0, crate::support::paid("CCU",Move::CastFor(CastCost::Alternative(MULLDRIFTER_EVOKE), Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
        cite!(["702.74a", "118.9"]; case: format!("reversed {reversed}"); assert_eq!(g.players[0].pool.total(), 0, "{{2}}{{U}}, not its mana cost {{4}}{{U}}"));
        settle(&mut g, None, reversed);
        cite!(["603.3b"]; case: format!("reversed {reversed}"); assert_eq!(g.players[0].hand.len(), 2));
        cite!(["702.74a"]; case: format!("reversed {reversed}"); assert_eq!(count(&g, &g.battlefield, MULLDRIFTER), 0));
        assert_eq!(count(&g, &g.players[0].graveyard, MULLDRIFTER), 1);
    }
}

#[test]
fn r702_74a_mulldrifter_cast_for_its_mana_cost_stays() {
    let mut g = game();
    let card = g.put(0, MULLDRIFTER, Zone::Hand);
    g.players[0].pool.colors[1] = 1;
    g.players[0].pool.colorless = 4;
    g.act(0, crate::support::paid("CCCCU",Move::Cast { card, targets: vec![] })).unwrap();
    settle(&mut g, None, false);
    assert_eq!(g.players[0].hand.len(), 2);
    cite!(["702.74a"]; assert_eq!(count(&g, &g.battlefield, MULLDRIFTER), 1));
}

#[test]
fn r702_74a_the_evoke_cost_must_be_paid() {
    let mut g = game();
    let card = g.put(0, MULLDRIFTER, Zone::Hand);
    g.players[0].pool.colorless = 3;
    cite!(["118.3a"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0, crate::support::paid("CCC",Move::CastFor(CastCost::Alternative(MULLDRIFTER_EVOKE), Box::new(Move::Cast { card, targets: vec![] })))));
    assert!(g.players[0].hand.contains(&card));
}

#[test]
fn r702_74a_a_nonmana_evoke_cost_exiles_a_white_card_and_solitude_still_exiles_its_target() {
    for reversed in [false, true] {
        let mut g = game();
        let card = g.put(0, SOLITUDE, Zone::Hand);
        let white = g.put(0, WHITE, Zone::Hand);
        g.put(0, SPELL, Zone::Hand);
        let bear = g.put(1, BEAR, Zone::Battlefield);
        g.act(0, Move::CastFor(CastCost::Alternative(SOLITUDE_EVOKE), Box::new(Move::WithCosts(vec![vec![white]], Box::new(Move::Cast { card, targets: vec![] }))))).unwrap();
        settle(&mut g, Some(bear), reversed);
        cite!(["702.74a"]; case: format!("reversed {reversed}"); assert_eq!(count(&g, &g.exile, WHITE), 1));
        assert_eq!(count(&g, &g.players[0].hand, SPELL), 1);
        cite!(["113.7a", "603.3b"]; case: format!("reversed {reversed}"); assert_eq!(count(&g, &g.exile, BEAR), 1));
        assert_eq!(g.players[1].life, 23, "its controller gains life equal to its power");
        cite!(["702.74a"]; assert_eq!(count(&g, &g.players[0].graveyard, SOLITUDE), 1));
    }
}

#[test]
fn r702_74a_a_nonwhite_card_does_not_pay_solitudes_evoke_cost() {
    let mut g = game();
    let card = g.put(0, SOLITUDE, Zone::Hand);
    let other = g.put(0, SPELL, Zone::Hand);
    cite!(["118.3"]; refused("Spell is not what that cost asks for or cannot pay it (701.13a, 118.3)"): g.act(0, Move::CastFor(CastCost::Alternative(SOLITUDE_EVOKE), Box::new(Move::WithCosts(vec![vec![other]], Box::new(Move::Cast { card, targets: vec![] }))))));
    // nor does Solitude itself: the card being cast is on the stack as its costs are paid
    cite!(["601.2a", "118.3"]; refused("Solitude is not what that cost asks for or cannot pay it (701.13a, 118.3)"): g.act(0, Move::CastFor(CastCost::Alternative(SOLITUDE_EVOKE), Box::new(Move::WithCosts(vec![vec![card]], Box::new(Move::Cast { card, targets: vec![] }))))));
    assert!(g.players[0].hand.contains(&card));
}
