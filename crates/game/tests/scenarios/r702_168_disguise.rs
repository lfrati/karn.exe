//! Disguise (702.168): cast face down for {3} like morph, but the face-down spell and
//! permanent are 2/2 creatures with ward {2} (702.168a/b), turned face up for the disguise
//! cost (702.168d). Defenestrated Phantom ({4}{W}{W} 4/3 flying, disguise {4}{W}) and
//! Riftburst Hellion (disguise {4}{R/G}{R/G}) are Oracle cards.
use crate::support::cite;
use crate::support;
use game::announcement::CastCost;
use game::face::{Concealment, Down, Face, Way};
use game::observation::Kind;
use support::*;

fn game() -> T {
    let mut g = Game::new(["Defenestrated Phantom", "Riftburst Hellion", "Lightning Bolt"].iter().map(|n| oracle(n)).collect::<Vec<_>>(), 2, 20);
    std::sync::Arc::make_mut(&mut g.defs).push(CardDef::new("Wastes", "", "Basic Land", "", None).unwrap());
    for p in 0..2 { for _ in 0..10 { g.put(p, 3, Zone::Library); } }
    T::new(g)
}
/// Player 0 casts this card face down with three mana and it resolves.
fn disguise(t: &mut T, index: usize) -> ObjId {
    let card = t.g.put(0, index, Zone::Hand);
    t.mana(0, "{3}");
    t.g.act(0, paid("CCC",Move::CastFor(CastCost::Alternative(1), Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    let spell = *t.g.stack.last().unwrap();
    assert_eq!(t.g.face(spell), Face::Down(Down { by: Concealment::Disguised }));
    t.resolve();
    assert!(t.g.battlefield.contains(&spell) || t.g.battlefield.iter().any(|o| t.g.face_down(*o)));
    *t.g.battlefield.iter().find(|o| t.g.face_down(**o)).unwrap()
}
/// Player 1 bolts this permanent with this mana; the bolt and any ward trigger are put on
/// the stack and priority passes until something else is asked or the stack is empty.
fn bolt(t: &mut T, target: ObjId, mana: &str) -> usize {
    while t.g.priority != 1 { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    let bolt = t.g.put(1, 2, Zone::Hand);
    t.mana(1, mana);
    t.g.act(1, paid("R",Move::Cast { card: bolt, targets: vec![vec![Target::Object(target)]] })).unwrap();
    let on_stack = t.g.stack.len();
    while t.g.decision.is_none() && !t.g.stack.is_empty() { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    on_stack
}

#[test]
fn r702_168b_cast_face_down_it_is_a_2_2_with_ward_2() {
    let mut t = game();
    let card = t.g.put(0, 0, Zone::Hand);
    t.mana(0, "{3}");
    t.g.act(0, paid("CCC",Move::CastFor(CastCost::Alternative(1), Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    let spell = *t.g.stack.last().unwrap();
    cite!(["702.168b"]; assert_eq!((t.g.obj(spell).chars.name.as_str(), t.g.obj(spell).chars.pt), ("", Some((2, 2)))));
    // the face-down spell has ward {2} (702.168b), the keyword and its trigger, and nothing
    // else; ward works only on a permanent (702.21a)
    cite!(["702.168b"]; assert_eq!(t.g.abilities(spell).len(), 2));
    assert!(!t.g.has(spell, "Flying"));
    let theirs = game::snapshot::Snapshot::of(&t.g, Some(1)).objects.into_iter().find(|o| o.id == spell).unwrap();
    assert_eq!(theirs.face_down.map(|d| (d.by, d.card.is_none())), Some((Concealment::Disguised, true)));
    t.resolve();
    let o = *t.g.battlefield.iter().find(|o| t.g.face_down(**o)).unwrap();
    cite!(["702.168a"]; assert_eq!(t.g.abilities(o).len(), 2, "ward {{2}}"));
    assert!(!t.g.has(o, "Flying"));
    cite!(["702.168a"]; assert_eq!((t.g.power(o), t.g.toughness(o)), (Some(2), Some(2))));
}

#[test]
fn r702_168b_its_ward_counters_an_opponents_spell_unless_they_pay_2() {
    // cannot pay: countered
    let mut t = game();
    let o = disguise(&mut t, 0);
    let on_stack = bolt(&mut t, o, "{R}");
    cite!(["702.21a"]; assert_eq!(on_stack, 2, "the bolt and the ward trigger"));
    cite!(["118.3a"]; refused("your mana pool cannot pay that cost"): unchanged(&mut t.g, |g| g.act(1, Move::Choose(Answer::Pay { plan: vec![], with: None, tapped: vec![] }))));
    t.g.act(1, Move::Choose(Answer::No)).unwrap();
    cite!(["702.21a"]; assert!(t.g.stack.is_empty() && t.g.battlefield.contains(&o)));
    // pays: the bolt resolves and the face-down Phantom dies, revealed by its owner
    let mut t = game();
    let o = disguise(&mut t, 0);
    for p in 0..2 { t.g.take_observations(p); }
    bolt(&mut t, o, "{R}{2}");
    cite!(["702.21a"]; assert!(matches!(t.g.asked(), Some((1, Asked::Pay { life: 0, .. }))), "{:?}", t.g.asked()));
    t.g.act(1, Move::Choose(Answer::Pay { plan: vec![], with: Some(mana_pool("CC")), tapped: vec![] })).unwrap();
    t.resolve();
    assert!(!t.g.battlefield.contains(&o));
    let shown: Vec<_> = t.g.observations(1).iter().flat_map(|d| d.cards.iter().map(|c| (d.kind, d.by, c.name.clone()))).collect();
    cite!(["708.9"]; assert_eq!(shown, vec![(Kind::Revealed, 0, "Defenestrated Phantom".to_owned())]));
}

#[test]
fn r702_168d_turned_face_up_for_its_disguise_cost_it_loses_ward() {
    let mut t = game();
    let o = disguise(&mut t, 0);
    t.mana(0, "{4}");
    cite!(["118.3a"]; refused("the supplied mana does not pay that cost, all of it and no more"): unchanged(&mut t.g, |g| g.act(0, paid("CCCC", Move::TurnFaceUp(o, Way::Disguise)))));
    cite!(["702.37e"]; refused("that permanent would have no such cost if it were face up (702.37e)"): t.g.act(0, Move::TurnFaceUp(o, Way::Morph)));
    t.mana(0, "{W}");
    t.g.act(0, paid("CCCCW",Move::TurnFaceUp(o, Way::Disguise))).unwrap();
    cite!(["702.168d"]; assert_eq!(t.g.players[0].pool.total(), 0));
    cite!(["702.168d"]; assert_eq!((t.g.obj(o).chars.name.as_str(), t.g.power(o), t.g.toughness(o)), ("Defenestrated Phantom", Some(4), Some(3))));
    assert!(t.g.has(o, "Flying"));
    let on_stack = bolt(&mut t, o, "{R}");
    cite!(["702.168d"]; assert_eq!(on_stack, 1, "no ward trigger"));
    assert!(!t.g.battlefield.contains(&o), "bolted");
}

#[test]
fn r702_168d_a_hybrid_disguise_cost_is_paid_either_way() {
    let mut t = game();
    let o = disguise(&mut t, 1);
    t.mana(0, "{4}{R}{G}");
    t.g.act(0, paid("CCCCRG",Move::TurnFaceUp(o, Way::Disguise))).unwrap();
    cite!(["702.168d"]; assert_eq!(t.g.obj(o).chars.name, "Riftburst Hellion"));
    assert_eq!(t.g.players[0].pool.total(), 0);
}

#[track_caller]
fn unchanged(g: &mut Game, action: impl FnOnce(&mut Game) -> Result<(), String>) -> Result<(), String> {
    let before=format!("{g:?}");
    let result=action(g);
    assert_eq!(format!("{g:?}"),before);
    result
}
