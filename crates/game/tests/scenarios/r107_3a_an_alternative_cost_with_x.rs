//! "You may exile a blue card with mana value X from your hand rather than pay this spell's
//! mana cost": an alternative cost with an X in it. Its caster announces X as the spell is cast,
//! the card exiled must have that mana value, and the spell's X is that value while it is on the
//! stack (107.3a, 601.2b, 118.9).
use crate::support;
use game::announcement::CastCost;
use support::*;

/// Cast `card` for its alternative cost, announcing X and exiling `exiled`.
fn alternative(x: Option<u32>, card: ObjId, exiled: ObjId, targets: Vec<Vec<Target>>) -> Move {
    let cast = Move::CastFor(CastCost::Alternative(0), Box::new(Move::WithCosts(vec![vec![exiled]], Box::new(Move::Cast { card, targets }))));
    match x { Some(x) => Move::WithX(x, Box::new(cast)), None => cast }
}

/// Pass until the stack is empty.
fn resolve(t: &mut T) {
    for _ in 0..8 {
        if t.g.stack.is_empty() { return; }
        t.pass_all();
    }
}

/// Player 0 casts the creature spell; player 1 holds Disrupting Shoal and Counterspell.
fn spell_and_shoal(spell: &str, cost: &str, pool: &str) -> (T, ObjId, ObjId, ObjId) {
    let mut t = Board::new(2).hand(0, &[spell]).hand(1, &["Disrupting Shoal", "Counterspell"]).build();
    t.mana(0, cost).cast(0, spell, vec![], mana_pool(pool)).unwrap();
    let cast = t.g.stack[0];
    t.g.act(0, Move::Pass).unwrap();
    let shoal = t.find_in(1, Zone::Hand, "Disrupting Shoal");
    let blue = t.find_in(1, Zone::Hand, "Counterspell");
    (t, cast, shoal, blue)
}

// Rules: 107.3a, 118.9, 608.2c
// Given: a creature spell of mana value 2 or 3 on the stack; its opponent casts Disrupting Shoal
// for its alternative cost, announcing X 2 and exiling Counterspell (mana value 2).
// When: Disrupting Shoal resolves.
// Expect: Counterspell is exiled; the spell is countered only if its mana value is 2.
#[test]
fn r107_3a_disrupting_shoal_for_its_alternative_cost_counters_a_spell_of_mana_value_x() {
    for (spell, cost, pool, countered) in [("Grizzly Bears", "{1}{G}", "CG", true), ("Centaur Courser", "{2}{G}", "CCG", false)] {
        let (mut t, cast, shoal, blue) = spell_and_shoal(spell, cost, pool);
        t.g.act(1, alternative(Some(2), shoal, blue, vec![vec![Target::Object(cast)]])).unwrap();
        let exiled: Vec<_> = t.g.exile.iter().map(|o| t.g.obj(*o).chars.name.clone()).collect();
        cite!(["118.9"]; case: spell; assert_eq!((exiled, t.g.players[1].hand.contains(&blue)), (vec!["Counterspell".to_string()], false)));
        resolve(&mut t);
        let on_battlefield = t.g.battlefield.iter().any(|o| t.g.obj(*o).chars.name == spell);
        cite!(["107.3a", "608.2c"]; case: spell; assert_eq!(on_battlefield, !countered));
    }
}

// Rules: 107.3a, 601.2b, 118.3
// Given: a creature spell on the stack; its opponent holds Disrupting Shoal and Counterspell
// (mana value 2).
// When: they cast Disrupting Shoal for its alternative cost without announcing X, or announcing
// X 3 and exiling Counterspell.
// Expect: both are refused, and Counterspell stays in that player's hand.
#[test]
fn r107_3a_disrupting_shoal_asks_for_x_and_a_card_of_that_mana_value() {
    let (mut t, cast, shoal, blue) = spell_and_shoal("Grizzly Bears", "{1}{G}", "CG");
    cite!(["107.3a", "601.2b"]; refused("announce the value of X"): t.g.act(1, alternative(None, shoal, blue, vec![vec![Target::Object(cast)]])));
    cite!(["118.3"]; refused("Counterspell is not what that cost asks for"): t.g.act(1, alternative(Some(3), shoal, blue, vec![vec![Target::Object(cast)]])));
    assert!(t.g.players[1].hand.contains(&blue));
}

// Rules: 107.3a, 118.9, 601.2f
// Given: Nourishing Shoal and Centaur Courser (mana value 3) in a player's hand.
// When: they cast Nourishing Shoal for its alternative cost, announcing X 3 and exiling the
// Courser, or for its mana cost {X}{G}{G} with X 2.
// Expect: they gain 3 life, or 2.
#[test]
fn r107_3a_nourishing_shoal_gains_the_announced_x_by_either_cost() {
    let mut t = Board::new(2).hand(0, &["Nourishing Shoal", "Centaur Courser"]).build();
    let shoal = t.find_in(0, Zone::Hand, "Nourishing Shoal");
    let green = t.find_in(0, Zone::Hand, "Centaur Courser");
    t.g.act(0, alternative(Some(3), shoal, green, vec![])).unwrap();
    resolve(&mut t);
    cite!(["107.3a", "118.9"]; assert_eq!(t.g.players[0].life, 23));

    let mut t = Board::new(2).hand(0, &["Nourishing Shoal"]).build();
    let shoal = t.find_in(0, Zone::Hand, "Nourishing Shoal");
    t.mana(0, "{2}{G}{G}");
    t.g.act(0, Move::Paying(mana_pool("CCGG"), Box::new(Move::WithX(2, Box::new(Move::Cast { card: shoal, targets: vec![] }))))).unwrap();
    resolve(&mut t);
    cite!(["107.3a", "601.2f"]; assert_eq!(t.g.players[0].life, 22));
}
