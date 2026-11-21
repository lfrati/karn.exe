//! Power-compared blocking restrictions (509.1b) and flavor-word modes (207.2d):
//! Silumgar Assassin's "power greater"; a face-down Den Protector has no restriction, turned
//! up with megamorph's counter it compares its new power; a real declaration with a too-small
//! blocker is refused; Secret Identity is offered and printed with both flavor-word modes.
use crate::support::cite;
use crate::support;
use game::announcement::CastCost;
use game::face::Way;
use support::*;

fn find(t: &T, p: usize, name: &str) -> ObjId {
    *t.g.battlefield.iter().find(|o| t.g.controller(**o) == p && t.g.obj(**o).chars.name == name).unwrap_or_else(|| panic!("{name}"))
}

#[test]
fn silumgar_assassin_greater_power_cant_block_it() {
    let t = Board::new(2).battlefield(0, &["Silumgar Assassin"]).battlefield(1, &["Hill Giant", "Grizzly Bears", "Llanowar Elves"]).build();
    let assassin = find(&t, 0, "Silumgar Assassin");
    let (giant, bears, elves) = (find(&t, 1, "Hill Giant"), find(&t, 1, "Grizzly Bears"), find(&t, 1, "Llanowar Elves"));
    cite!(["509.1b"]; assert!(t.g.cant_block(giant, assassin, &[giant]), "3 is greater than 2"));
    cite!(["509.1b"]; assert!(!t.g.cant_block(bears, assassin, &[bears]) && !t.g.cant_block(elves, assassin, &[elves])));
}

/// Face down, Den Protector is a 2/2 with no abilities: an Elf may block it. Turned face up
/// for its megamorph cost it is a 3/2: now the Bears (2) can't, the Hill Giant (3) can.
#[test]
fn den_protector_face_down_then_up() {
    let mut t = Board::new(2).hand(0, &["Den Protector"]).battlefield(1, &["Hill Giant", "Grizzly Bears", "Llanowar Elves"]).build();
    let card = t.find_in(0, Zone::Hand, "Den Protector");
    t.mana(0, "{3}");
    let cost = CastCost::Alternative(1); // Den Protector's second printed row is megamorph.
    t.g.act(0, paid("CCC", Move::CastFor(cost, Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    let before = t.g.battlefield.clone();
    t.resolve();
    let den = *t.g.battlefield.iter().find(|o| !before.contains(o)).unwrap();
    let (giant, bears, elves) = (find(&t, 1, "Hill Giant"), find(&t, 1, "Grizzly Bears"), find(&t, 1, "Llanowar Elves"));
    cite!(["708.2a", "509.1b"]; assert!(!t.g.cant_block(elves, den, &[elves]), "face down: no restriction"));
    while t.g.priority != 0 { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.mana(0, "{1}{G}");
    cite!(["702.37e"]; ok: t.g.act(0, paid("CG", Move::TurnFaceUp(den, Way::Morph))));
    t.resolve();
    cite!(["702.37b", "708.8"]; assert_eq!(t.g.power(den), Some(3), "its own 2/1 with megamorph's +1/+1 counter"));
    cite!(["509.1b", "708.8"]; assert!(t.g.cant_block(elves, den, &[elves]) && t.g.cant_block(bears, den, &[bears])));
    cite!(["509.1b", "708.8"]; assert!(!t.g.cant_block(giant, den, &[giant])));
}

/// Den Protector attacks: declaring the Elves as its blocker is refused; the Bears may.
#[test]
fn a_declaration_with_a_smaller_blocker_is_refused() {
    let mut t = Board::new(2).battlefield(0, &["Den Protector"]).battlefield(1, &["Grizzly Bears", "Llanowar Elves"]).build();
    let den = find(&t, 0, "Den Protector");
    let (bears, elves) = (find(&t, 1, "Grizzly Bears"), find(&t, 1, "Llanowar Elves"));
    while t.g.decision != Some(game::state::Decision::Attackers { player: 0 }) { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.g.act(0, Move::Attack(vec![(den, Target::Player(1))])).unwrap();
    while t.g.decision != Some(game::state::Decision::Blockers { player: 1 }) { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    cite!(["509.1b"]; refused("Llanowar Elves can't block Den Protector"): t.g.clone().act(1, Move::Block(vec![(elves, den)])));
    // one disobeyed restriction is enough
    cite!(["509.1b"]; refused("Llanowar Elves can't block Den Protector"): t.g.clone().act(1, Move::Block(vec![(elves, den), (bears, den)])));
    cite!(["509.1a", "509.1b"]; ok: t.g.act(1, Move::Block(vec![(bears, den)])));
}

/// Secret Identity is offered with each of its modes, and its text prints both alike: as
/// with every ability word and flavor word (207.2c–d), neither label.
#[test]
fn secret_identity_offers_and_prints_both_modes() {
    let mut t = Board::new(2).battlefield(0, &["Grizzly Bears"]).hand(0, &["Secret Identity"]).build();
    t.mana(0, "{U}");
    let card = t.find_in(0, Zone::Hand, "Secret Identity");
    let bear = t.find_in_battlefield("Grizzly Bears");
    for mode in [0, 1] {
        let mut attempt = t.g.clone();
        cite!(["700.2a"]; case: mode; ok: attempt.act(0, paid("U", Move::Modes(vec![mode], Box::new(Move::Cast { card, targets: vec![vec![bear.into()]] })))));
    }
    // how the prompt prints the modes is presentation, not a rule: uncited
    let mut sim = game::simulation::Simulation::new(t.g.clone());
    let text = sim.prompt(0).unwrap().entities[&card].text.join("\n");
    assert!(text.contains("\n• Until end of turn, target creature you control becomes a Citizen")
        && text.contains("\n• Until end of turn, target creature you control becomes a Hero"), "{text}");
    assert!(!text.contains("Conceal —") && !text.contains("Reveal —"), "{text}");
}
