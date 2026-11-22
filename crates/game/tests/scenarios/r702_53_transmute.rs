//! Transmute discards in hand and searches by that card's mana value at sorcery timing.
use crate::support::{cite, mana_pool, oracle, paid};
use game::{Game, Move, Zone};
use game::state::{Answer, Choice, Decision};

fn fixture(source: &str) -> (Game, game::ObjId, usize) {
    let mut g = Game::new(vec![oracle(source), oracle("Grizzly Bears"), oracle("Crib Swap"), oracle("Forest")], 2, 20);
    let card = g.put(0, 0, Zone::Hand);
    let ability = g.abilities(card).iter().rposition(|a| matches!(&**a, ir::Ability::Activated { .. })).unwrap();
    g.players[0].pool = mana_pool("UUU");
    (g, card, ability)
}
fn activation(source: game::ObjId, ability: usize) -> Move { paid("UUU", Move::Activate { source, ability, targets: vec![], paying: vec![] }) }
fn pass(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }
#[test]
fn r702_53a_muddle_discards_before_private_search_and_keeps_its_mana_value() {
    let (mut g, card, ability) = fixture("Muddle the Mixture");
    let right = g.put(0, 1, Zone::Library);
    let wrong = g.put(0, 2, Zone::Library);
    let land = g.put(0, 3, Zone::Library);
    g.act(0, activation(card, ability)).unwrap();
    cite!(["702.53a"]; assert!(g.players[0].hand.is_empty() && g.players[0].pool.total() == 0));
    let discarded = g.players[0].graveyard[0];
    g.exile(discarded);
    g.retire_events(g.log.end()); g.collect_departed();
    pass(&mut g);
    let Some(Decision::Choose { player: 0, choice: Choice::Search { among, .. } }) = &g.decision else { panic!("{:?}", g.decision) };
    cite!(["702.53a", "608.2h"]; assert_eq!(among, &vec![right]));
    for invalid in [wrong, land] {
        cite!(["701.23a"]; refused("a chosen card is not one the search can find"): g.act(0, Move::Choose(Answer::Objects(vec![invalid]))));
    }
    for (mut branch, found) in [(g.clone(), true), (g, false)] {
        branch.act(0, Move::Choose(Answer::Objects(if found { vec![right] } else { vec![] }))).unwrap();
        cite!(["702.53a", "701.23b"]; assert_eq!(branch.players[0].hand.len(), usize::from(found)));
        assert_eq!(branch.players[0].library.len(), 3 - usize::from(found));
        assert!(!branch.sees(0, wrong));
    }
}
#[test]
fn r702_53a_tolaria_west_finds_zero_mana_value_lands() {
    let (mut g, card, ability) = fixture("Tolaria West");
    let wrong = g.put(0, 1, Zone::Library);
    let right = g.put(0, 3, Zone::Library);
    g.act(0, activation(card, ability)).unwrap(); pass(&mut g);
    let Some(Decision::Choose { choice: Choice::Search { among, .. }, .. }) = &g.decision else { panic!("{:?}", g.decision) };
    cite!(["702.53a", "202.3a"]; assert_eq!(among, &vec![right]));
    assert!(!among.contains(&wrong));
    g.act(0, Move::Choose(Answer::Objects(vec![right]))).unwrap();
    cite!(["702.53a"]; assert_eq!(g.obj(g.players[0].hand[0]).card, Some(3)));
}
#[test]
fn r702_53a_transmute_requires_hand_and_sorcery_timing() {
    for zone in [Zone::Graveyard, Zone::Battlefield, Zone::Exile] {
        let (mut g, card, ability) = fixture("Muddle the Mixture");
        g.move_object(card, zone);
        let moved = match zone { Zone::Graveyard => g.players[0].graveyard[0], Zone::Battlefield => g.battlefield[0], _ => g.exile[0] };
        cite!(["702.53a", "113.6m"]; refused("that ability functions only in the zone it moves its card out of"): g.act(0, activation(moved, ability)));
    }
    let (mut g, card, ability) = fixture("Muddle the Mixture");
    g.active = 1;
    cite!(["702.53a"]; refused("sorcery"): g.act(0, activation(card, ability)));
    assert_eq!(g.players[0].hand, vec![card]);
    assert_eq!(g.players[0].pool.total(), 3);
}
