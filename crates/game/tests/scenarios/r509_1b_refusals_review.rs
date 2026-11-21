//! Block refusals (509.1b): the restriction named must be the one that
//! applies, on the creature that has it.
use crate::support::cite;
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};

fn c(name: &str, cost: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef { CardDef::new(name, cost, types, text, pt).expect(name) }

fn at_blocks(cards: Vec<CardDef>, attacker: usize, blockers: &[usize], others: &[(usize, usize)]) -> (Game, ObjId, Vec<ObjId>) {
    for d in &cards { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(cards, 2, 20);
    let a = g.put(0, attacker, Zone::Battlefield);
    let bs: Vec<ObjId> = blockers.iter().map(|b| g.put(1, *b, Zone::Battlefield)).collect();
    for (p, i) in others { g.put(*p, *i, Zone::Battlefield); }
    for _ in 0..40 {
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![(a, Target::Player(1))])).unwrap(),
            Some(Decision::Blockers { .. }) => return (g, a, bs),
            Some(d) => panic!("{d:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("no declaration of blockers")
}

/// A third permanent's effect: "Creatures can't block." is named by its source.
#[test]
fn a_global_effect_names_its_source() {
    let (mut g, a, bs) = at_blocks(vec![
        c("Bear", "{0}", "Creature — Bear", "", Some((2, 2))),
        c("Ox", "{0}", "Creature — Ox", "", Some((2, 2))),
        c("Panic", "{0}", "Enchantment", "Creatures with power 2 or less can't block.", None),
        c("Giant", "{0}", "Creature — Giant", "", Some((3, 3))),
    ], 0, &[1, 3], &[(0, 2)]);
    let said = g.act(1, Move::Block(vec![(bs[0], a)])).unwrap_err();
    cite!(["509.1b"]; assert_eq!(said, "Ox can't block Bear: Panic says it can't, or not that way (509.1b)"));
    cite!(["509.1b"]; ok: g.act(1, Move::Block(vec![(bs[1], a)])));
}
