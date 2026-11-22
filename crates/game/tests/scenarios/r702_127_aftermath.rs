//! Aftermath belongs to the selected half and redirects every departure of its graveyard cast.
use crate::support::{cite, mana_pool, oracle, paid};
use game::{Game, Move, ObjId, Target, Zone};
use game::announcement::CastRoute;
use game::card::CastAs;
fn half(card: ObjId, face: usize, targets: Vec<Vec<Target>>) -> Move {
    Move::CastAs(CastAs::Half(face), Box::new(Move::Cast { card, targets }))
}
fn pass(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }
#[test]
fn r702_127a_front_goes_to_graveyard_then_fame_casts_for_its_own_mana_cost_and_is_exiled() {
    let mut g = Game::new(vec![oracle("Claim // Fame"), oracle("Grizzly Bears")], 2, 20);
    let card = g.put(0, 0, Zone::Hand); let grave = g.put(0, 1, Zone::Graveyard); let old = g.put(0, 1, Zone::Battlefield);
    g.players[0].pool = mana_pool("BRR");
    cite!(["702.127a", "601.3"]; refused("can't be cast from its current zone"): g.act(0, paid("RR", half(card, 1, vec![vec![Target::Object(old)]]))));
    g.act(0, paid("B", half(card, 0, vec![vec![Target::Object(grave)]]))).unwrap(); pass(&mut g);
    let parent = g.players[0].graveyard[0];
    cite!(["709.4", "702.127a"]; assert_eq!(g.obj(parent).chars.mana_value, 3));
    g.act(0, paid("RR", half(parent, 1, vec![vec![Target::Object(old)]]))).unwrap();
    cite!(["709.3b"]; assert_eq!(g.obj(g.stack[0]).chars.mana_value, 2));
    pass(&mut g);
    cite!(["702.127a"]; assert_eq!(g.exile.len(), 1));
    assert!(g.players[0].graveyard.is_empty());
    cite!(["709.4"]; assert_eq!(g.obj(g.exile[0]).chars.mana_value, 3));
    assert_eq!(g.power(old), Some(4)); assert!(g.has(old, "Haste"));
}
#[test]
fn r702_127a_aftermath_does_not_grant_the_other_half_a_graveyard_permission() {
    let mut g = Game::new(vec![oracle("Claim // Fame"), oracle("Grizzly Bears")], 2, 20);
    let card = g.put(0, 0, Zone::Graveyard); let target = g.put(0, 1, Zone::Graveyard);
    g.players[0].pool = mana_pool("B");
    cite!(["702.127a", "709.3"]; refused("hand"): g.act(0, paid("B", half(card, 0, vec![vec![Target::Object(target)]]))));
    assert!(g.players[0].graveyard.contains(&card));
}
#[test]
fn r702_127a_hard_zone_restriction_overrides_a_valid_external_casting_permission() {
    let mut g = Game::new(vec![oracle("Claim // Fame"), oracle("Grizzly Bears"), oracle("Bolas's Citadel")], 2, 20);
    let permit = g.put(0, 2, Zone::Battlefield);
    let old = g.put(0, 1, Zone::Battlefield); let grave = g.put(0, 1, Zone::Graveyard); let card = g.put(0, 0, Zone::Library);
    let route = CastRoute::Permission { source: permit, ability: 1 };
    cite!(["702.127a", "101.2"]; refused("can't be cast from its current zone"): g.act(0, Move::CastVia(route, Box::new(half(card, 1, vec![vec![Target::Object(old)]])))));
    assert_eq!(g.players[0].life, 20);
    g.act(0, Move::CastVia(route, Box::new(half(card, 0, vec![vec![Target::Object(grave)]])))).unwrap();
    // Uncited (Oracle): the Citadel pays life equal to the selected Claim half's mana value.
    assert_eq!(g.players[0].life, 19); pass(&mut g);
    cite!(["702.127a", "709.3"]; assert!(g.exile.is_empty() && g.players[0].graveyard.len() == 1));
}
#[test]
fn r702_127a_x_and_instant_timing_come_from_the_selected_half() {
    let mut g = Game::new(vec![oracle("Spring // Mind"), oracle("Forest")], 3, 20);
    g.active = 1;
    let card = g.put(0, 0, Zone::Graveyard); for _ in 0..2 { g.put(0, 1, Zone::Library); }
    g.players[0].pool = mana_pool("UUUUUU");
    cite!(["117.1a", "702.127a"]; ok: g.act(0, paid("UUUUUU", half(card, 1, vec![]))));
    pass(&mut g);
    cite!(["702.127a"]; assert_eq!(g.players[0].hand.len(), 2));
    let mut g = Game::new(vec![oracle("Cut // Ribbons")], 3, 20);
    let card = g.put(0, 0, Zone::Graveyard); g.players[0].pool = mana_pool("BBBBB");
    g.act(0, paid("BBBBB", Move::WithX(3, Box::new(half(card, 1, vec![]))))).unwrap(); pass(&mut g);
    cite!(["107.3", "702.127a"]; assert_eq!((g.players[1].life, g.players[2].life), (17, 17)));
}
#[test]
fn r702_127a_countering_and_other_stack_departures_all_exile_the_original_card() {
    let mut g = Game::new(vec![oracle("Cut // Ribbons")], 2, 20);
    let card = g.put(0, 0, Zone::Graveyard); g.players[0].pool = mana_pool("BB");
    g.act(0, paid("BB", Move::WithX(0, Box::new(half(card, 1, vec![]))))).unwrap();
    let spell = g.stack[0];
    for destination in [Zone::Graveyard, Zone::Hand, Zone::Library] {
        let mut branch = g.clone(); branch.move_object(spell, destination);
        cite!(["702.127a"]; case: format!("{destination:?}"); assert_eq!(branch.exile.len(), 1));
    }
    cite!(["701.6a", "702.127a"]; assert!(g.counter(spell)));
    assert_eq!(g.exile.len(), 1);
}
#[test]
fn r702_127a_a_spell_copy_does_not_inherit_a_graveyard_casts_exile_replacement() {
    let mut g = Game::new(vec![oracle("Spring // Mind"), oracle("Twincast"), oracle("Forest")], 2, 20);
    let card = g.put(0, 0, Zone::Graveyard); for _ in 0..4 { g.put(0, 2, Zone::Library); }
    g.players[0].pool = mana_pool("UUUUUUUU");
    g.act(0, paid("UUUUUU", half(card, 1, vec![]))).unwrap();
    let original = g.stack[0]; let twin = g.put(0, 1, Zone::Hand);
    g.act(0, paid("UU", Move::Cast { card: twin, targets: vec![vec![Target::Object(original)]] })).unwrap(); pass(&mut g);
    let copy = *g.stack.last().unwrap(); assert_ne!(copy, original);
    pass(&mut g);
    cite!(["702.127a", "707.10"]; assert!(g.log.iter().any(|e| matches!(e, game::Event::ZoneChanged { was, to: Zone::Graveyard, .. } if *was == copy))));
    pass(&mut g);
    cite!(["702.127a", "707.10"]; assert_eq!(g.exile.len(), 1));
    assert_eq!(g.players[0].hand.len(), 4);
}
