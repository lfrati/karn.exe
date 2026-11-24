//! "Spells your opponents cast during your turn cost {1} more to cast" (Paladin Class), "The
//! first non-Lemur creature spell with flying you cast during each of your turns costs {1} less
//! to cast" (Momo, Friendly Flier): the total cost is determined as the spell is cast (601.2f),
//! so the change applies to casts in that window only. Every card is read from its Oracle text.
use crate::support::{Board, cite, mana_pool, paid, player};
use game::{Move, Zone};

/// Can player `p` cast `name` from their hand now, spending exactly `units`? A spell that targets
/// targets the other player.
fn castable(t: &crate::support::T, p: usize, name: &str, units: &str, targeted: bool) -> bool {
    let card = t.find_in(p, Zone::Hand, name);
    let targets = if targeted { vec![vec![game::Target::Player(1 - p)]] } else { vec![] };
    t.g.clone().act(p, paid(units, Move::Cast { card, targets })).is_ok()
}

#[test]
fn r601_2f_paladin_class_taxes_opponents_spells_during_its_controllers_turn() {
    let mut t = Board::new(2).battlefield(0, &["Paladin Class"]).hand(1, &["Shock"]).build();
    // player 0's turn: player 1 has priority once player 0 passes
    t.g.act(0, Move::Pass).unwrap();
    t.mana(1, "{1}{R}");
    cite!(["601.2f"]; assert!(!castable(&t, 1, "Shock", "R", true), "{{R}} and {{1}} more"));
    t.cast(1, "Shock", vec![player(0)], mana_pool("CR")).unwrap();
    t.resolve();
    cite!(["120.3a"]; assert_eq!(t.g.players[0].life, 18));
}

#[test]
fn r601_2f_paladin_class_leaves_spells_cast_in_the_opponents_turn_alone() {
    let mut t = Board::new(2).active(1).battlefield(0, &["Paladin Class"]).hand(1, &["Shock"]).build();
    t.mana(1, "{R}");
    cite!(["601.2f"]; assert!(castable(&t, 1, "Shock", "R", true), "not during its controller's turn"));
}

#[test]
fn r601_2f_momo_discounts_the_first_flying_creature_spell_of_its_controllers_turn() {
    let mut t = Board::new(2)
        .battlefield(0, &["Momo, Friendly Flier"])
        .hand(0, &["Grizzly Bears", "Wind Drake", "Vexing Gull"])
        .build();
    t.mana(0, "{1}{G}");
    cite!(["601.2f"]; assert!(!castable(&t, 0, "Grizzly Bears", "G", false), "a creature spell without flying"));
    t.cast(0, "Grizzly Bears", vec![], mana_pool("CG")).unwrap();
    t.resolve();
    t.mana(0, "{1}{U}");
    // {2}{U}, {1} less
    t.cast(0, "Wind Drake", vec![], mana_pool("CU")).unwrap();
    t.resolve();
    cite!(["601.2f"]; assert!(t.g.players[0].pool.total() == 0));
    t.mana(0, "{2}{U}");
    cite!(["601.2f"]; assert!(!castable(&t, 0, "Vexing Gull", "CU", false), "the second such spell this turn"));
    t.cast(0, "Vexing Gull", vec![], mana_pool("CCU")).unwrap();
    t.resolve();
    cite!(["608.3"]; assert_eq!(t.g.battlefield.iter().filter(|o| t.g.controller(**o) == 0).count(), 4));
}

#[test]
fn r601_2f_momo_does_not_discount_a_spell_cast_in_an_opponents_turn() {
    let mut t = Board::new(2).active(1).battlefield(0, &["Momo, Friendly Flier"]).hand(0, &["Vexing Gull"]).build();
    // player 1's turn: player 0 has priority once player 1 passes, and Vexing Gull has flash
    t.g.act(1, Move::Pass).unwrap();
    t.mana(0, "{2}{U}");
    cite!(["601.2f"]; assert!(!castable(&t, 0, "Vexing Gull", "CU", false), "not during its controller's turn"));
    t.cast(0, "Vexing Gull", vec![], mana_pool("CCU")).unwrap();
}
