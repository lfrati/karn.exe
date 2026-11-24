//! A mana ability that makes a delayed triggered ability (605.1a, 603.7a): Barbed Sextant's and
//! Astrolabe's "Draw a card at the beginning of the next turn's upkeep" is that delayed
//! ability's to do, later, so the ability that adds the mana moves no card to or from a
//! library: it is a mana ability, and resolves at once (605.3b). An ability that draws as it
//! adds mana is not one (Chromatic Sphere, 605.1a). Every card is read from its Oracle text.
use crate::support;
use support::*;
use game::legal::Step;
use ir::Color;
use ir::mana::ManaSymbol;

fn red(n: usize) -> Vec<ManaSymbol> { vec![ManaSymbol::Color(Color::Red); n] }

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Barbed Sextant", "Astrolabe", "Chromatic Sphere"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

// Rules: 605.1a, 605.3b, 603.7a, 603.7b
// Given: Barbed Sextant on its controller's battlefield, {1} in their pool.
// When: they activate it for red in their main phase.
// Expect: it resolves at once, using no stack, and the red mana is in their pool; nothing is
// drawn until the next turn's upkeep, player 1's, where its controller draws a card.
#[test]
fn r605_1a_barbed_sextant_is_a_mana_ability_whose_draw_waits() {
    // Barbed Sextant: "{1}, {T}, Sacrifice this artifact: Add one mana of any color. Draw a
    // card at the beginning of the next turn's upkeep."
    let mut t = Board::new(2).battlefield(0, &["Barbed Sextant"]).fill(5).build();
    let sextant = t.find_in_battlefield("Barbed Sextant");
    t.mana(0, "{1}");
    cite!(["605.1a", "605.3b"]; ok: t.g.act(0, Move::Adding(vec![red(1)], Box::new(paid("C", Move::Activate { source: sextant, ability: 0, targets: vec![], paying: vec![] })))));
    cite!(["605.3b"]; assert!(t.g.stack.is_empty(), "a mana ability does not use the stack"));
    cite!(["605.3b", "106.4"]; assert_eq!(t.g.players[0].pool.colors[Color::Red as usize], 1));
    t.expect().rules(["603.7a"]).zone(0, Zone::Graveyard, &["Barbed Sextant"]).holds(|g| g.players[0].hand.is_empty(), "nothing drawn yet").check();
    t.to_no_combat(1, Part::Upkeep);
    t.expect().rules(["603.7a", "603.7b"]).holds(|g| g.players[0].hand.len() == 1 && g.players[1].hand.is_empty(), "drawn at the next turn's upkeep").check();
}

// Rules: 605.3a, 601.2g, 603.7a
// Given: Astrolabe and {1} in the pool; Grizzly Bears in hand.
// When: Astrolabe is activated for {G}{G} while the Bears are cast, paying for them.
// Expect: the Bears alone are on the stack, paid for; the card is drawn at the next turn's
// upkeep.
#[test]
fn r605_3a_astrolabe_is_activated_while_a_spell_is_paid_for() {
    // Astrolabe: "{1}, {T}, Sacrifice this artifact: Add two mana of any one color. Draw a card
    // at the beginning of the next turn's upkeep."
    let mut t = Board::new(2).battlefield(0, &["Astrolabe"]).hand(0, &["Grizzly Bears"]).fill(5).build();
    let (astrolabe, bears) = (t.find_in_battlefield("Astrolabe"), t.find_in(0, Zone::Hand, "Grizzly Bears"));
    t.mana(0, "{1}");
    let green = vec![ManaSymbol::Color(Color::Green); 2];
    let step = Step { plan: vec![], x: None, life: vec![], with: Some(mana_pool("C")), cost_branches: None, chose: vec![], source: astrolabe, ability: 0, paying: vec![], adding: vec![green] };
    cite!(["605.3a", "601.2g"]; ok: t.g.act(0, Move::WithMana(vec![step], Box::new(paid("GG", Move::Cast { card: bears, targets: vec![] })))));
    t.expect().rules(["605.3a", "605.3b"]).stack(&["Grizzly Bears"]).zone(0, Zone::Graveyard, &["Astrolabe"]).check();
    cite!(["601.2h"]; assert_eq!(t.g.players[0].pool.total(), 0));
    t.resolve();
    t.to_no_combat(1, Part::Upkeep);
    t.expect().rules(["603.7a", "603.7b"]).zone(0, Zone::Battlefield, &["Grizzly Bears"]).holds(|g| g.players[0].hand.len() == 1, "drawn at the next turn's upkeep").check();
}

// Rules: 605.1a, 602.2
// Given: Chromatic Sphere and {1} in the pool.
// When: it is activated.
// Expect: its "Draw a card." moves a card from a library as it resolves: it is not a mana
// ability, says no mana as it is activated, and waits on the stack.
#[test]
fn r605_1a_chromatic_sphere_draws_as_it_resolves_and_is_no_mana_ability() {
    // Chromatic Sphere: "{1}, {T}, Sacrifice this artifact: Add one mana of any color. Draw a
    // card." Ruling: "Chromatic Sphere's ability is not a mana ability. Players can respond to
    // it, and it can't be activated in the middle of paying for a cost."
    let mut t = Board::new(2).battlefield(0, &["Chromatic Sphere"]).fill(5).build();
    let sphere = t.find_in_battlefield("Chromatic Sphere");
    t.mana(0, "{1}");
    let activate = Move::Activate { source: sphere, ability: 0, targets: vec![], paying: vec![] };
    cite!(["605.1a"]; refused("that ability offers no choice of mana during activation (605.3b, 608.2d)"): t.g.act(0, Move::Adding(vec![red(1)], Box::new(paid("C", activate.clone())))));
    cite!(["605.1a", "602.2"]; ok: t.g.act(0, paid("C", activate)));
    cite!(["605.1a", "602.2"]; assert_eq!(t.g.stack.len(), 1, "the ability waits on the stack"));
    cite!(["605.1a"]; assert!(t.g.players[0].hand.is_empty()));
}
