//! Negative values remain signed inside calculations and comparisons (107.1b).
use crate::support::cite;
use game::{CardDef, Game, Move, Target, Zone};
fn run_text(powers: &[i32], text: &str) -> i32 {
    let mut defs = vec![CardDef::new("Reward", "{0}", "Sorcery", text, None).unwrap()];
    defs.extend(powers.iter().enumerate().map(|(i,p)| CardDef::new(&format!("Creature {i}"), "{0}", "Creature — Spirit", "", Some((*p, 4))).unwrap()));
    for d in &defs { game::play::check(d).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    for i in 0..powers.len() { g.put(0, i+1, Zone::Battlefield); }
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
    g.act(0, Move::Pass).unwrap(); g.act(1, Move::Pass).unwrap();
    g.players[0].life - 20
}
fn run_total(powers: &[i32]) -> i32 {
    run_text(powers, "You gain life equal to the total power of creatures you control.")
}
#[test]
fn r107_1b_negative_components_are_not_clamped_before_summing_power() {
    cite!(["107.1b"]; assert_eq!(run_total(&[-2, 3]), 1));
    cite!(["107.1b"]; assert_eq!(run_total(&[3, -2]), 1));
    cite!(["107.1b"]; assert_eq!(run_total(&[-3, -2, 7]), 2));
}
#[test]
fn r107_1b_only_the_final_negative_life_gain_is_zero() {
    for powers in [&[][..], &[-3, -2], &[-3, 2], &[0, 0], &[-2, 2]] {
        cite!(["107.1b"]; case: format!("{powers:?}"); assert_eq!(run_total(powers), 0, "{powers:?}"));
    }
    assert_eq!(run_total(&[2, 3]), 5);
}
#[test]
fn r107_1b_target_power_comparisons_keep_a_negative_right_hand_value() {
    let defs = vec![
        CardDef::new("Source", "{0}", "Creature — Spirit", "{0}: Destroy target creature with power less than this creature's power.", Some((-2, 4))).unwrap(),
        CardDef::new("Larger", "{0}", "Creature — Spirit", "", Some((-1, 4))).unwrap(),
        CardDef::new("Smaller", "{0}", "Creature — Spirit", "", Some((-3, 4))).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let larger = g.put(1, 1, Zone::Battlefield);
    let smaller = g.put(1, 2, Zone::Battlefield);
    let before = format!("{g:?}");
    cite!(["115.1"]; refused("an illegal target, Larger: its power is -1, and the target says power less than -2 (115.1)"): g.act(0, Move::Activate { source, ability: 0, targets: vec![vec![Target::Object(larger)]], paying: vec![] }));
    assert_eq!(format!("{g:?}"), before);
    g.act(0, Move::Activate { source, ability: 0, targets: vec![vec![Target::Object(smaller)]], paying: vec![] }).unwrap();
    g.act(0, Move::Pass).unwrap(); g.act(1, Move::Pass).unwrap();
    cite!(["701.8a"]; assert_eq!(g.obj(smaller).zone, Zone::Gone));
    assert_eq!(g.obj(larger).zone, Zone::Battlefield);
}

#[test]
fn r107_1b_conditions_compare_signed_totals_and_extrema() {
    for (powers, gain) in [(&[-3, 2][..], 3), (&[-2, 3][..], 0), (&[][..], 0)] {
        cite!(["107.1b"]; case: format!("{powers:?}"); assert_eq!(run_text(powers, "If the total power of creatures you control is less than 0, you gain 3 life."), gain));
    }
    cite!(["107.1b"]; assert_eq!(run_text(&[-3, -2], "If the greatest power among creatures you control is less than 0, you gain 3 life."), 3));
    cite!(["107.1b"]; assert_eq!(run_text(&[-3, 2], "If the least power among creatures you control is less than 0, you gain 3 life."), 3));
    assert_eq!(run_text(&[], "You gain life equal to the greatest power among creatures you control."), 0);
}

#[test]
fn r608_2h_a_condition_reads_negative_power_from_a_sacrificed_sources_last_known_state() {
    let def = CardDef::new("Source", "{0}", "Creature — Spirit", "Sacrifice this creature: If this creature's power is less than 0, you gain 3 life.", Some((-2, 4))).unwrap();
    game::play::check(&def).unwrap();
    let mut g = Game::new(vec![def], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    g.act(0, Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    assert_eq!(g.obj(source).zone, Zone::Gone);
    g.act(0, Move::Pass).unwrap(); g.act(1, Move::Pass).unwrap();
    cite!(["608.2h", "107.1b"]; assert_eq!(g.players[0].life, 23));
}
