//! "Add one mana of any of the exiled card's colors": a color the card this object's linked
//! ability exiled is now (607.2a, 105.2); with no such card, or a colorless one, the ability adds
//! no mana and never {C} (Chrome Mox and Pit of Offerings rulings).
use crate::support;
use support::*;
use ir::{Color, mana::ManaSymbol};

fn add(t: &mut T, source: ObjId, ability: usize, adding: Vec<Vec<ManaSymbol>>) -> Result<(), String> {
    let action = Move::Activate { source, ability, targets: vec![], paying: vec![] };
    t.g.act(0, if adding.is_empty() { action } else { Move::Adding(adding, Box::new(action)) })
}
fn color(c: Color) -> Vec<Vec<ManaSymbol>> { vec![vec![ManaSymbol::Color(c)]] }
const NOT_OFFERED: &str = "that ability doesn't offer the mana said for it: say one of what it offers (608.2d)";

/// Cast Chrome Mox and imprint these cards (none, or one).
fn imprint(cards: &[&str]) -> (T, ObjId) {
    let mut t = Board::new(2).hand(0, &["Chrome Mox", "Lightning Helix", "Ulamog's Crusher"]).fill(10).build();
    t.to_no_combat(0, Part::PrecombatMain);
    t.cast(0, "Chrome Mox", vec![], mana_pool("")).unwrap();
    let boundary = t.settle();
    let Boundary::Question(p, _) = boundary else { panic!("expected the imprint choice, got {boundary:?}") };
    if cards.is_empty() { t.answer(p, Answer::No).unwrap(); } else {
        t.answer(p, Answer::Yes).unwrap();
        let Boundary::Question(p, _) = t.settle() else { panic!("expected the card choice") };
        t.choose(p, cards).unwrap();
    }
    t.resolve();
    let mox = t.find_in_battlefield("Chrome Mox");
    (t, mox)
}

// Rules: 607.2a, 106.7, 105.2
// Given: player 0 casts Chrome Mox and exiles Lightning Helix ({R}{W}) with it.
// When: they tap the Mox for each color in turn.
// Expect: red or white is added; blue, black, green and colorless are not offered.
#[test]
fn r607_2a_chrome_mox_adds_one_of_the_imprinted_cards_colors() {
    let (t, mox) = imprint(&["Lightning Helix"]);
    for c in [Color::Red, Color::White] {
        let mut branch = t.clone();
        add(&mut branch, mox, 1, color(c)).unwrap();
        cite!(["607.2a", "105.2"]; case: format!("{c:?}"); assert_eq!(branch.g.players[0].pool.total(), 1));
    }
    for c in [Color::Blue, Color::Black, Color::Green] {
        let mut branch = t.clone();
        cite!(["106.7"]; case: format!("{c:?}"); refused(NOT_OFFERED): add(&mut branch, mox, 1, color(c)));
    }
    let mut branch = t.clone();
    cite!(["106.7"]; refused(NOT_OFFERED): add(&mut branch, mox, 1, vec![vec![ManaSymbol::Colorless]]));
}

// Rules: 106.5, 607.2a
// Given: player 0 casts Chrome Mox and exiles nothing with it, or exiles Ulamog's Crusher (colorless).
// When: they tap the Mox.
// Expect: it becomes tapped and adds no mana.
#[test]
fn r106_5_chrome_mox_with_no_colored_imprint_adds_nothing() {
    for cards in [&[][..], &["Ulamog's Crusher"][..]] {
        let (mut t, mox) = imprint(cards);
        add(&mut t, mox, 1, vec![]).unwrap();
        cite!(["106.5"]; case: format!("{cards:?}"); assert_eq!(t.g.players[0].pool.total(), 0));
        assert!(t.g.obj(mox).tapped);
    }
}

// Rules: 607.2a, 105.2
// Given: Pit of Offerings enters under player 0's control; player 1's graveyard holds Lightning
// Bolt ({R}) and Llanowar Elves ({G}).
// When: its enters ability exiles both, and player 0 taps the Pit for its last ability.
// Expect: red or green is offered, blue is not.
#[test]
fn r607_2a_pit_of_offerings_adds_any_of_the_exiled_cards_colors() {
    let mut t = Board::new(2).hand(0, &["Pit of Offerings"]).graveyard(1, &["Lightning Bolt", "Llanowar Elves"]).fill(10).build();
    let (bolt, elves) = (t.find("Lightning Bolt"), t.find("Llanowar Elves"));
    t.to_no_combat(0, Part::PrecombatMain);
    let pit = t.find_in(0, Zone::Hand, "Pit of Offerings");
    t.g.act(0, Move::PlayLand(pit)).unwrap();
    let boundary = t.settle();
    let Boundary::Question(p, _) = boundary else { panic!("expected targets, got {boundary:?}") };
    t.g.act(p, Move::ChooseTargets(vec![vec![Target::Object(bolt), Target::Object(elves)]])).unwrap();
    t.resolve();
    let pit = t.find_in_battlefield("Pit of Offerings");
    t.g.untap(pit); // it entered tapped
    for c in [Color::Red, Color::Green] {
        let mut branch = t.clone();
        add(&mut branch, pit, 3, color(c)).unwrap();
        cite!(["607.2a", "105.2"]; case: format!("{c:?}"); assert_eq!(branch.g.players[0].pool.total(), 1));
    }
    cite!(["106.7"]; refused(NOT_OFFERED): add(&mut t, pit, 3, color(Color::Blue)));
}
