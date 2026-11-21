//! "Choose one — • Conceal — …": a word before a mode is a flavor word with no rules meaning
//! (207.2d); the mode is what follows it (700.2).
use crate::support;
use support::*;

fn cast(mode: usize) -> T {
    let mut t = Board::new(2).battlefield(0, &["Grizzly Bears"]).hand(0, &["Secret Identity"]).build();
    t.mana(0, "{U}");
    let (bears, card) = (t.find_in_battlefield("Grizzly Bears"), t.find_in(0, Zone::Hand, "Secret Identity"));
    t.g.act(0, Move::Modes(vec![mode], Box::new(paid("U",Move::Cast { card, targets: vec![vec![Target::Object(bears)]] })))).unwrap();
    t.resolve();
    t
}

#[test]
fn r207_2d_secret_identity_conceal_is_its_first_mode() {
    let t = cast(0);
    let bears = t.find_in_battlefield("Grizzly Bears");
    t.expect().rules(["207.2d", "700.2"]).pt("Grizzly Bears", 1, 1).check();
    cite!(["700.2"]; assert!(t.g.has(bears, "Hexproof")));
    cite!(["700.2"]; assert!(t.g.seen(bears, None).subtypes.iter().any(|s| s.name() == "Citizen")));
}

#[test]
fn r207_2d_secret_identity_reveal_is_its_second_mode() {
    let t = cast(1);
    let bears = t.find_in_battlefield("Grizzly Bears");
    t.expect().rules(["207.2d", "700.2"]).pt("Grizzly Bears", 3, 4).check();
    cite!(["700.2"]; assert!(t.g.has(bears, "Flying") && t.g.has(bears, "Vigilance") && !t.g.has(bears, "Hexproof")));
    cite!(["700.2"]; assert!(t.g.seen(bears, None).subtypes.iter().any(|s| s.name() == "Hero")));
}
