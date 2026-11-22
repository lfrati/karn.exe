//! "A copy of it", "a copy of that creature" where the anaphor names an object
//! other than the source of the ability: the creature that entered (Miirym, Ultron), the
//! creature that dealt damage (Impostor Syndrome), the creature that died (Vaultborn Tyrant),
//! and a copy created by another player (Bramble Sovereign: "that creature's controller
//! creates"). 707.2, 608.2h, 111.2.
use crate::support::{cite, paid, mana_pool};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::{CardType, Part};

fn game(names: &[&str], custom: Vec<CardDef>) -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let mut defs: Vec<CardDef> = names.iter().map(|n| CardDef::from_face(faces.iter().find(|f| f.name == *n).unwrap()).unwrap()).collect();
    defs.extend(custom);
    defs.push(CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap());
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let island = defs.len() - 1;
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..8 { g.put(p, island, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn c(name: &str, cost: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef { CardDef::new(name, cost, types, text, pt).expect(name) }
/// Resolve the stack, saying yes to each "may", attacking with `attackers` if asked.
fn settle(g: &mut Game, attackers: &[ObjId], payment: Option<&str>) {
    let mut payment = payment;
    for _ in 0..60 {
        match g.decision.clone() {
            Some(Decision::Choose { player, choice: Choice::May { possible: true } }) => g.act(player, Move::Choose(Answer::Yes)).unwrap(),
            Some(Decision::Choose { player, choice: Choice::Pay { .. } }) => g.act(player, Move::Choose(Answer::Pay { plan: vec![], with: Some(mana_pool(payment.take().expect("caller-authored optional payment"))), tapped: vec![] })).unwrap(),
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(attackers.iter().map(|a| (*a, Target::Player(1))).collect())).unwrap(),
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            Some(d) => panic!("{d:?}"),
            None if g.stack.is_empty() => return,
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("did not settle");
}
fn tokens(g: &Game) -> Vec<ObjId> {
    g.log.iter().filter_map(|e| if let Event::Created { token } = e { Some(*token) } else { None }).collect()
}
fn name(g: &Game, o: ObjId) -> String { g.obj(o).chars.name.clone() }

#[test]
fn miirym_copies_the_dragon_that_entered_not_itself() {
    let mut g = game(&["Miirym, Sentinel Wyrm"], vec![c("Drake", "{0}", "Legendary Creature — Dragon", "", Some((3, 3)))]);
    g.put(0, 0, Zone::Battlefield);
    let drake = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card: drake, targets: vec![] }).unwrap();
    settle(&mut g, &[], None);
    let made = tokens(&g);
    assert_eq!(made.len(), 1);
    cite!(["707.2"]; assert_eq!(name(&g, made[0]), "Drake"));
    cite!(["707.9b"]; assert!(!g.seen(made[0], None).supertypes.contains(&ir::Supertype::Legendary), "except the token isn't legendary"));
}

#[test]
fn impostor_syndrome_copies_the_creature_that_dealt_the_damage() {
    let mut g = game(&["Impostor Syndrome"], vec![c("Bear", "{0}", "Creature — Bear", "", Some((2, 2)))]);
    g.put(0, 0, Zone::Battlefield);
    let bear = g.put(0, 1, Zone::Battlefield);
    let turn = g.turn;
    for _ in 0..80 {
        if !tokens(&g).is_empty() && g.stack.is_empty() || g.turn != turn { break; }
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![(bear, Target::Player(1))])).unwrap(),
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            Some(d) => panic!("{d:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    assert_eq!(g.players[1].life, 18);
    let made = tokens(&g);
    assert_eq!(made.len(), 1);
    cite!(["707.2"]; assert_eq!(name(&g, made[0]), "Bear"));
}

#[test]
fn ultron_copies_the_artifact_and_a_noncreature_token_becomes_a_robot() {
    let mut g = game(&["Ultron, Artificial Malevolence"], vec![c("Rock", "{0}", "Artifact", "", None)]);
    g.put(0, 0, Zone::Battlefield);
    let rock = g.put(0, 1, Zone::Hand);
    g.players[0].pool.colors[1] = 2;
    g.act(0, Move::Cast { card: rock, targets: vec![] }).unwrap();
    settle(&mut g, &[], Some("UU"));
    let made = tokens(&g);
    assert_eq!(made.len(), 1, "{:?}", g.decision);
    cite!(["707.2"]; assert_eq!(name(&g, made[0]), "Rock"));
    cite!(["707.9b"]; assert!(g.is_a(made[0], CardType::Artifact) && g.is_a(made[0], CardType::Creature)));
    cite!(["707.9b"]; assert_eq!((g.power(made[0]), g.toughness(made[0])), (Some(2), Some(2))));
}

#[test]
fn vaultborn_tyrant_copies_itself_as_it_last_existed_when_it_dies() {
    let mut g = game(&["Vaultborn Tyrant"], vec![c("Doom", "{0}", "Instant", "Destroy target creature.", None)]);
    let tyrant = g.put(0, 0, Zone::Battlefield);
    let doom = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card: doom, targets: vec![vec![Target::Object(tyrant)]] }).unwrap();
    settle(&mut g, &[], None);
    let made = tokens(&g);
    assert_eq!(made.len(), 1);
    cite!(["707.2", "608.2h"]; assert_eq!(name(&g, made[0]), "Vaultborn Tyrant"));
    cite!(["707.9b"]; assert!(g.is_a(made[0], CardType::Artifact), "an artifact in addition"));
    // the token's own death makes no further token: "if it's not a token"
    let doom = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card: doom, targets: vec![vec![Target::Object(made[0])]] }).unwrap();
    settle(&mut g, &[], None);
    assert_eq!(tokens(&g).len(), 1);
}

#[test]
fn bramble_sovereign_the_entering_creatures_controller_creates_the_copy() {
    let mut g = game(&["Bramble Sovereign"], vec![c("Bear", "{0}", "Creature — Bear", "", Some((2, 2)))]);
    g.put(0, 0, Zone::Battlefield);
    // the opponent's creature enters on their turn
    while g.active != 1 || g.part != Part::PrecombatMain || g.priority != 1 {
        match g.decision.clone() { Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(), _ => g.act(g.priority, Move::Pass).unwrap() }
    }
    let bear = g.put(1, 1, Zone::Hand);
    g.act(1, Move::Cast { card: bear, targets: vec![] }).unwrap();
    g.players[0].pool.colors[4] = 2;
    settle(&mut g, &[], Some("GG"));
    let made = tokens(&g);
    assert_eq!(made.len(), 1, "{:?}", g.decision);
    assert_eq!(name(&g, made[0]), "Bear");
    cite!(["111.2"]; assert_eq!(g.controller(made[0]), 1, "that creature's controller creates it (111.2)"));
    assert_eq!(g.obj(made[0]).owner, 1);
}

#[test]
fn gruff_triplets_makes_two_copies_of_itself_and_they_make_none() {
    let mut g = game(&["Gruff Triplets"], vec![]);
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[4] = 6;
    g.players[0].pool.colors[0] = 6;
    g.act(0, paid("GGGGGG", Move::Cast { card, targets: vec![] })).unwrap();
    settle(&mut g, &[], None);
    let made = tokens(&g);
    assert_eq!(made.len(), 2);
    cite!(["707.2"]; assert!(made.iter().all(|t| name(&g, *t) == "Gruff Triplets")));
}

#[test]
fn flameshadow_conjuring_exiles_the_hasty_token_not_the_creature_it_copied() {
    let mut g = game(&["Flameshadow Conjuring"], vec![c("Bear", "{0}", "Creature — Bear", "", Some((2, 2)))]);
    g.put(0, 0, Zone::Battlefield);
    let card = g.put(0, 1, Zone::Hand);
    g.players[0].pool.colors[3] = 1;
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    settle(&mut g, &[], Some("R"));
    let bear = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(1) && !g.obj(**o).token()).unwrap();
    let made = tokens(&g);
    assert_eq!(made.len(), 1);
    assert_eq!(name(&g, made[0]), "Bear");
    assert!(g.has(made[0], "Haste"), "that token gains haste");
    assert!(!g.has(bear, "Haste"));
    while g.part != Part::End { match g.decision.clone() { Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(), _ => g.act(g.priority, Move::Pass).unwrap() } }
    settle(&mut g, &[], None);
    assert!(g.battlefield.contains(&bear), "the creature stays");
    cite!(["603.7c"]; assert!(!g.battlefield.contains(&made[0]), "the token is exiled"));
}
