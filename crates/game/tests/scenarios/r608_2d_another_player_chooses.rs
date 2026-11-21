//! An instruction said of another player than the controller is that player's to follow,
//! and the choices it offers are theirs (608.2d): "Target opponent exiles a creature they
//! control." asks the opponent, among the permanents they control; with none, nothing is
//! exiled. Blot Out's choice is among the greatest mana value only (a tie: any of them).
//! Every card is read from its Oracle text.

use crate::support::{cite, paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const DOOMFALL: usize = 0;
const BLOT: usize = 1;
const BEARS: usize = 2;
const GIANT: usize = 3;
const LILIANA: usize = 4;
const SWAMP: usize = 5;
const WINTER: usize = 6;
const BOLT: usize = 7;
const COURSER: usize = 8;
const BETRAYAL: usize = 9;
const NEGOTIATION: usize = 10;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let mut defs: Vec<_> = ["Doomfall", "Blot Out", "Grizzly Bears", "Hill Giant", "Hill Giant", "Swamp", "Early Winter", "Lightning Bolt", "Centaur Courser", "Strategic Betrayal", "Ruthless Negotiation"]
        .map(|n| CardDef::from_faces(&catalog[n]).unwrap()).to_vec();
    // a planeswalker of mana value three, with no abilities
    defs[LILIANA] = CardDef::planeswalker("Walker", "{1}{B}{B}", "Legendary Planeswalker — Liliana", "", 3).unwrap();
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..6 { g.put(p, SWAMP, Zone::Library); } }
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}

fn pass(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }

/// Player 0 casts this card at player 1, choosing these modes, and it resolves.
fn cast(g: &mut Game, card: usize, modes: &[usize], spending: &str) {
    let c = g.put(0, card, Zone::Hand);
    g.players[0].pool.colors = [0, 0, 6, 0, 0];
    let cast = Move::Cast { card: c, targets: vec![vec![Target::Player(1)]] };
    g.act(0, paid(spending, if modes.is_empty() { cast } else { Move::Modes(modes.to_vec(), Box::new(cast)) })).unwrap();
    pass(g);
}

fn asked(g: &Game) -> (usize, Vec<ObjId>) {
    match g.decision.clone() {
        Some(Decision::Choose { player, choice: Choice::Objects { among, .. } | Choice::Select { among, .. } }) => (player, among),
        other => panic!("a choice of objects: {other:?}"),
    }
}

#[test]
fn r608_2d_doomfall_the_target_opponent_chooses_among_creatures_they_control() {
    // "• Target opponent exiles a creature they control."
    let mut g = game();
    let mine = g.put(0, GIANT, Zone::Battlefield);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    cast(&mut g, DOOMFALL, &[0], "BBB");
    let (player, among) = asked(&g);
    cite!(["608.2d"]; assert_eq!(player, 1, "the opponent chooses"));
    cite!(["608.2d"]; assert_eq!(among, vec![bears, giant], "a creature they control"));
    let before = format!("{g:?}");
    cite!(["608.2d"]; refused("that decision is another player's"): g.act(0, Move::Choose(Answer::Objects(vec![bears]))));
    assert_eq!(format!("{g:?}"), before);
    g.act(1, Move::Choose(Answer::Objects(vec![bears]))).unwrap();
    cite!(["608.2d"]; assert!(g.battlefield.contains(&giant) && g.battlefield.contains(&mine) && !g.battlefield.contains(&bears)));
    assert_eq!(g.exile.iter().filter(|o| g.obj(**o).chars.name == "Grizzly Bears").count(), 1);
}

#[test]
fn r608_2d_one_creature_is_exiled_without_asking() {
    let mut g = game();
    let bears = g.put(1, BEARS, Zone::Battlefield);
    cast(&mut g, DOOMFALL, &[0], "BBB");
    cite!(["608.2d"]; assert!(g.decision.is_none() && !g.battlefield.contains(&bears)));
}

#[test]
fn r608_2d_with_no_creature_they_control_nothing_is_exiled() {
    let mut g = game();
    let mine = g.put(0, BEARS, Zone::Battlefield);
    cast(&mut g, DOOMFALL, &[0], "BBB");
    cite!(["608.2d"]; assert!(g.decision.is_none() && g.battlefield.contains(&mine) && g.exile.is_empty() && g.stack.is_empty()));
}

#[test]
fn r608_2d_doomfall_the_controller_chooses_from_the_revealed_hand() {
    // "• Target opponent reveals their hand. You choose a nonland card from it. Exile that card."
    let mut g = game();
    let bolt = g.put(1, BOLT, Zone::Hand);
    let bears = g.put(1, BEARS, Zone::Hand);
    let swamp = g.put(1, SWAMP, Zone::Hand);
    cast(&mut g, DOOMFALL, &[1], "BBB");
    let (player, among) = asked(&g);
    cite!(["608.2d"]; assert_eq!((player, among), (0, vec![bolt, bears]), "you choose a nonland card"));
    g.act(0, Move::Choose(Answer::Objects(vec![bolt]))).unwrap();
    assert_eq!(g.players[1].hand, vec![bears, swamp]);
}

#[test]
fn r608_2d_blot_out_offers_only_the_greatest_mana_value() {
    // "Target opponent exiles a creature or planeswalker they control with the greatest mana
    // value among creatures and planeswalkers they control."
    let mut g = game();
    let bears = g.put(1, BEARS, Zone::Battlefield);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    let mine = g.put(0, GIANT, Zone::Battlefield);
    g.put(0, GIANT, Zone::Battlefield);
    cast(&mut g, BLOT, &[], "BBB");
    // the one Hill Giant they control has the greatest, four: exiled without a choice
    cite!(["608.2d"]; assert!(g.decision.is_none(), "{:?}", g.decision));
    cite!(["608.2d"]; assert!(!g.battlefield.contains(&giant) && g.battlefield.contains(&bears) && g.battlefield.contains(&mine)));
}

#[test]
fn r608_2d_blot_out_a_tie_is_the_opponents_choice() {
    let mut g = game();
    let liliana = g.put(1, LILIANA, Zone::Battlefield);
    g.put_counters(liliana, "loyalty", 3);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    g.put(1, BEARS, Zone::Battlefield);
    // Liliana (3) is greatest; then with a second three, a tie between them
    let mut tie = g.clone();
    cast(&mut g, BLOT, &[], "BBB");
    cite!(["608.2d"]; assert!(!g.battlefield.contains(&liliana) && g.battlefield.contains(&bears)));
    let other = tie.put(1, COURSER, Zone::Battlefield);
    cast(&mut tie, BLOT, &[], "BBB");
    let (player, mut among) = asked(&tie);
    among.sort();
    cite!(["608.2d"]; assert_eq!((player, among), (1, vec![liliana, other])));
}

#[test]
fn r608_2d_early_winter_the_opponent_exiles_an_enchantment_they_control() {
    // "• Target opponent exiles an enchantment they control." with none: nothing happens
    let mut g = game();
    cast(&mut g, WINTER, &[1], "BBBBB");
    cite!(["608.2d"]; assert!(g.decision.is_none() && g.stack.is_empty() && g.exile.is_empty()));
}

#[test]
fn r608_2d_strategic_betrayal_the_opponent_chooses_a_creature_and_exiles_it_with_their_graveyard() {
    // "Target opponent exiles a creature they control and their graveyard."
    let mut g = game();
    let mine = g.put(0, BOLT, Zone::Graveyard);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    let theirs = [g.put(1, BOLT, Zone::Graveyard), g.put(1, SWAMP, Zone::Graveyard)];
    cast(&mut g, BETRAYAL, &[], "BB");
    let (player, among) = asked(&g);
    cite!(["608.2d"]; assert_eq!((player, among), (1, vec![bears, giant])));
    g.act(1, Move::Choose(Answer::Objects(vec![giant]))).unwrap();
    cite!(["608.2d"]; assert!(g.battlefield.contains(&bears) && !g.battlefield.contains(&giant)));
    cite!(["608.2c"]; assert!(theirs.iter().all(|c| g.obj(*c).zone == Zone::Gone) && g.players[1].graveyard.is_empty(), "their graveyard, all of it"));
    assert_eq!(g.players[0].graveyard.first(), Some(&mine), "not the caster's");
    assert_eq!(g.exile.len(), 3);
}

#[test]
fn r608_2d_ruthless_negotiation_the_opponent_chooses_a_card_of_their_hand_unseen() {
    // "Target opponent exiles a card from their hand. If this spell was cast from a graveyard, draw a card."
    let mut g = game();
    let mine = g.put(0, BOLT, Zone::Hand);
    let theirs = [g.put(1, BOLT, Zone::Hand), g.put(1, BEARS, Zone::Hand)];
    cast(&mut g, NEGOTIATION, &[], "B");
    let (player, among) = asked(&g);
    cite!(["608.2d"]; assert_eq!((player, among), (1, theirs.to_vec()), "the opponent, among their own hand"));
    cite!(["402.3"]; assert!(theirs.iter().all(|c| !g.sees(0, *c)), "the caster does not see the choices"));
    g.act(1, Move::Choose(Answer::Objects(vec![theirs[1]]))).unwrap();
    cite!(["608.2d"]; assert_eq!(g.players[1].hand, vec![theirs[0]]));
    assert_eq!(g.exile.iter().map(|c| g.obj(*c).chars.name.as_str()).collect::<Vec<_>>(), ["Grizzly Bears"]);
    cite!(["608.2c"]; assert_eq!(g.players[0].hand, vec![mine], "cast from a hand: no card drawn"));
}

#[test]
fn r608_2d_an_empty_hand_exiles_nothing() {
    let mut g = game();
    cast(&mut g, NEGOTIATION, &[], "B");
    cite!(["608.2d"]; assert!(g.decision.is_none() && g.stack.is_empty() && g.exile.is_empty()));
}

#[test]
fn r608_2d_one_card_in_hand_is_exiled() {
    let mut g = game();
    let only = g.put(1, BEARS, Zone::Hand);
    cast(&mut g, NEGOTIATION, &[], "B");
    if g.decision.is_some() { g.act(1, Move::Choose(Answer::Objects(vec![only]))).unwrap(); }
    cite!(["608.2d"]; assert!(g.players[1].hand.is_empty() && g.exile.len() == 1));
}
