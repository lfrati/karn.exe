//! Showstopping Surprise: "Choose target creature you control. Turn it face up if it's face
//! down. Then it deals damage equal to its power to each other creature." A manifested
//! creature card turns face up (708.8) and deals damage with its own power; a face-up creature
//! just deals its power; a manifested instant stays face down (701.40g) and deals 2.
use crate::support::{cite, paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const SURPRISE: usize = 0;
const SUMMONS: usize = 1;
const WURM: usize = 2;
const BEARS: usize = 3;
const GIANT: usize = 4;
const BOLT: usize = 5;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let defs: Vec<_> = ["Showstopping Surprise", "Soul Summons", "Craw Wurm", "Grizzly Bears", "Hill Giant", "Lightning Bolt", "Wastes"]
        .iter().map(|n| {
            let d = CardDef::from_faces(&catalog[*n]).unwrap_or_else(|e| panic!("{n}: {e:?}"));
            game::play::check(&d).unwrap_or_else(|e| panic!("{n}: {}", e.0));
            d
        }).collect();
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, 6, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn mana(g: &mut Game) { g.players[0].pool.colors = [9; 5]; g.players[0].pool.colorless = 9; }
fn settle(g: &mut Game) {
    for _ in 0..30 {
        match g.decision.clone() {
            None if g.stack.is_empty() => return,
            None => g.act(g.priority, Move::Pass).unwrap(),
            // creatures dying together: their owner orders them into the graveyard (404.3)
            Some(Decision::Choose { player, choice: Choice::OrderCards { cards, .. } }) => g.act(player, Move::Choose(Answer::Objects(cards))).unwrap(),
            Some(d) => panic!("{d:?}"),
        }
    }
}
/// Player 0 manifests this card from the top of their library with Soul Summons.
fn manifest(g: &mut Game, card: usize) -> ObjId {
    g.put(0, card, Zone::Library);
    let summons = g.put(0, SUMMONS, Zone::Hand);
    mana(g);
    g.act(0, paid("CW", Move::Cast { card: summons, targets: vec![] })).unwrap();
    let before = g.battlefield.clone();
    settle(g);
    let o = *g.battlefield.iter().find(|o| !before.contains(o)).expect("it entered");
    assert!(g.face_down(o));
    o
}
fn surprise(g: &mut Game, at: ObjId) {
    let card = g.put(0, SURPRISE, Zone::Hand);
    mana(g);
    g.act(0, paid("CCCRR", Move::Cast { card, targets: vec![vec![Target::Object(at)]] })).unwrap();
    settle(g);
}
fn named(g: &Game, name: &str) -> Vec<ObjId> {
    g.battlefield.iter().copied().filter(|o| g.obj(*o).chars.name == name).collect()
}

/// A manifested Craw Wurm turns face up and deals 6 damage to each other creature.
#[test]
fn r708_8_a_manifested_creature_card_turns_face_up_and_deals_its_own_power() {
    let mut g = game();
    let wurm = manifest(&mut g, WURM);
    g.put(0, BEARS, Zone::Battlefield);
    g.put(1, GIANT, Zone::Battlefield);
    surprise(&mut g, wurm);
    cite!(["701.40a", "708.8"]; assert!(!g.face_down(wurm) && g.power(wurm) == Some(6)));
    cite!(["120.3"]; assert!(named(&g, "Grizzly Bears").is_empty() && named(&g, "Hill Giant").is_empty()));
    cite!(["120.3"]; assert_eq!(g.obj(wurm).damage, 0, "each other creature"));
}

/// A face-up Hill Giant deals 3 damage to each other creature: the Bears and an opponent's
/// Hill Giant die, a Craw Wurm doesn't.
#[test]
fn r708_8_a_face_up_creature_just_deals_its_power() {
    let mut g = game();
    let giant = g.put(0, GIANT, Zone::Battlefield);
    g.put(1, BEARS, Zone::Battlefield);
    g.put(1, GIANT, Zone::Battlefield);
    let wurm = g.put(1, WURM, Zone::Battlefield);
    surprise(&mut g, giant);
    cite!(["120.3"]; assert!(named(&g, "Grizzly Bears").is_empty() && named(&g, "Hill Giant") == vec![giant]));
    cite!(["120.3"]; assert_eq!((g.obj(wurm).damage, g.obj(giant).damage), (3, 0)));
}

/// A manifested Lightning Bolt can't turn face up (701.40g): it stays a face-down 2/2 and deals
/// 2 damage to each other creature.
#[test]
fn r701_40g_a_manifested_instant_stays_face_down_and_deals_2() {
    let mut g = game();
    let bolt = manifest(&mut g, BOLT);
    g.put(1, BEARS, Zone::Battlefield);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    surprise(&mut g, bolt);
    cite!(["701.40g"]; assert!(g.face_down(bolt) && g.power(bolt) == Some(2)));
    cite!(["701.40g", "120.3"]; assert!(named(&g, "Grizzly Bears").is_empty() && g.obj(giant).damage == 2));
}
