//! Turning a face-down permanent face up for its disguise or morph cost asks what it would be
//! face up: "If the permanent wouldn't have a disguise cost if it were face up, it can't be
//! turned face up this way" (702.168d; morph, 702.37e). The effects applied to it face down
//! still apply face up (708.8, 611.2c), so a face-down creature that lost all abilities "can't
//! be turned face up with a disguise ability because it will no longer have a disguise ability
//! (or a disguise cost) once face up" (Dog Walker ruling, 2024-02-02). A manifested or cloaked
//! card is still turned up for its card's mana cost (701.40b, 701.40d, 701.58b, 701.58d).
//! Dog Walker ({R}{W} 3/1 vigilance, disguise {R/W}{R/W}), Eaten by Piranhas, Noggle the Mind,
//! Final Showdown and Soul Summons are Oracle cards; no Standard card has morph, so a test card
//! stands for Fledgling Mawcor.
use crate::support::cite;
use crate::support;
use crate::scenarios::r708_8_turned_face_up::{face_down, game, manifest, put, turn_up};
use game::face::{Concealment, Down, Face, Way};
use support::*;

const DISGUISE_LOST: &str = "that permanent would have no such cost if it were face up (702.168d)";
const MORPH_LOST: &str = "that permanent would have no such cost if it were face up (702.37e)";

/// Player 1 casts this flash Aura ({1}{U}) on this permanent, with priority passed to them.
fn cast_aura(t: &mut T, name: &str, target: ObjId) {
    while t.g.priority != 1 { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    let card = put(t, 1, name, Zone::Hand);
    t.mana(1, "{1}{U}");
    t.g.act(1, paid("CU", Move::Cast { card, targets: vec![vec![Target::Object(target)]] })).unwrap();
}
/// Player 1 pays the ward {2} its target's ward trigger asks for.
fn pay_ward(t: &mut T) {
    assert!(matches!(t.settle(), Boundary::Question(1, Asked::Pay { .. })), "{:?}", t.g.asked());
    t.mana(1, "{2}");
    t.g.act(1, Move::Choose(Answer::Pay { plan: vec![], with: Some(mana_pool("CC")), tapped: vec![] })).unwrap();
}
/// Player 0 casts Final Showdown for its first mode only, {W} and {1}: "All creatures lose
/// all abilities until end of turn." It resolves.
fn showdown(t: &mut T) {
    let card = put(t, 0, "Final Showdown", Zone::Hand);
    t.mana(0, "{1}{W}");
    t.g.act(0, paid("CW", Move::Modes(vec![0], Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    t.resolve();
}
fn dogs(g: &Game) -> usize {
    g.battlefield.iter().filter(|o| g.obj(**o).chars.name == "Dog Token").count()
}

#[test]
fn r702_168d_an_enchanted_disguised_permanent_that_lost_its_abilities_is_not_turned_up_for_its_cost() {
    let mut t = game(&["Dog Walker", "Eaten by Piranhas"], &[]);
    let o = face_down(&mut t, 0, "Dog Walker");
    cast_aura(&mut t, "Eaten by Piranhas", o);
    cite!(["702.168a", "702.21a", "303.4a"]; assert_eq!(t.g.stack.len(), 2, "the Aura spell targets it: its ward {{2}} triggers"));
    assert!(matches!(t.settle(), Boundary::Question(1, Asked::Pay { .. })));
    // unpaid, the Aura spell is countered and the Walker still has its disguise ability
    let mut unpaid = t.clone();
    unpaid.g.act(1, Move::Choose(Answer::No)).unwrap();
    cite!(["702.21a"]; assert!(unpaid.g.stack.is_empty() && unpaid.g.battlefield.len() == 1));
    unpaid.find_in(1, Zone::Graveyard, "Eaten by Piranhas");
    unpaid.mana(0, "{R}{W}");
    cite!(["702.168d"]; ok: unpaid.g.act(0, paid("RW", Move::TurnFaceUp(o, Way::Disguise))));
    // paid, it resolves: the face-down permanent is a 1/1 black Skeleton with no abilities
    pay_ward(&mut t);
    t.resolve();
    let aura = t.find_in_battlefield("Eaten by Piranhas");
    assert_eq!(t.g.attached_to(aura), Some(Target::Object(o)));
    let seen = t.g.seen(o, None);
    cite!(["613.1d", "613.1e", "613.4b"]; assert_eq!((seen.colors, seen.pt), (vec![ir::Color::Black], Some((1, 1)))));
    cite!(["613.1f"]; assert!(t.g.abilities(o).is_empty(), "not even ward"));
    // face up it would still be enchanted, with no disguise ability or cost (708.8)
    assert_eq!(t.g.priority, 0);
    t.mana(0, "{R}{W}");
    cite!(["702.168d", "708.8"]; refused(DISGUISE_LOST): unchanged(&mut t.g, |g| g.act(0, paid("RW", Move::TurnFaceUp(o, Way::Disguise)))));
    cite!(["702.168d"]; assert!(t.g.face_down(o)));
    // neither has it any morph cost, nor is it a manifested permanent
    cite!(["702.37e"]; refused(MORPH_LOST): unchanged(&mut t.g, |g| g.act(0, paid("RW", Move::TurnFaceUp(o, Way::Morph)))));
    cite!(["701.40b"]; refused("only a manifested permanent is turned face up for its card's mana cost (701.40b)"): unchanged(&mut t.g, |g| g.act(0, paid("RW", Move::TurnFaceUp(o, Way::Manifested)))));
}

#[test]
fn r702_168d_noggle_the_mind_also_removes_the_disguise_it_would_have_face_up() {
    let mut t = game(&["Dog Walker", "Noggle the Mind"], &[]);
    let o = face_down(&mut t, 0, "Dog Walker");
    cast_aura(&mut t, "Noggle the Mind", o);
    cite!(["702.168a", "702.21a"]; assert_eq!(t.g.stack.len(), 2, "the Aura spell and the ward trigger"));
    pay_ward(&mut t);
    t.resolve();
    assert_eq!(t.g.attached_to(t.find_in_battlefield("Noggle the Mind")), Some(Target::Object(o)));
    t.mana(0, "{R}{W}");
    cite!(["702.168d", "708.8"]; refused(DISGUISE_LOST): unchanged(&mut t.g, |g| g.act(0, paid("RW", Move::TurnFaceUp(o, Way::Disguise)))));
}

#[test]
fn r702_168d_turned_up_while_the_aura_spell_is_on_the_stack_it_then_loses_its_abilities() {
    let mut t = game(&["Dog Walker", "Eaten by Piranhas"], &[]);
    let o = face_down(&mut t, 0, "Dog Walker");
    cast_aura(&mut t, "Eaten by Piranhas", o);
    pay_ward(&mut t);
    // the ward trigger resolved; the active player receives priority with the Aura spell on
    // the stack (117.3b), and nothing applies to the Walker yet
    assert_eq!((t.g.priority, t.g.stack.len()), (0, 1));
    t.find_in(1, Zone::Stack, "Eaten by Piranhas");
    t.mana(0, "{R}{W}");
    cite!(["702.168d", "116.2b"]; ok: t.g.act(0, paid("RW", Move::TurnFaceUp(o, Way::Disguise))));
    cite!(["702.168d"]; assert_eq!(t.g.players[0].pool.total(), 0, "{{R/W}}{{R/W}} paid"));
    cite!(["702.168d"]; assert_eq!(t.g.obj(o).chars.name, "Dog Walker"));
    t.resolve();
    // its trigger made the Dogs first; the Aura spell's target is still a creature (608.2b)
    cite!(["603.2", "708.8"]; assert_eq!(dogs(&t.g), 2));
    cite!(["303.4a", "608.2b"]; assert_eq!(t.g.attached_to(t.find_in_battlefield("Eaten by Piranhas")), Some(Target::Object(o))));
    cite!(["613.1f"]; assert!(!t.g.has(o, "Vigilance") && t.g.abilities(o).is_empty()));
    cite!(["613.4b"]; assert_eq!((t.g.power(o), t.g.toughness(o)), (Some(1), Some(1))));
}

#[test]
fn r702_168d_after_final_showdown_it_turns_up_once_until_end_of_turn_has_ended() {
    let mut t = game(&["Dog Walker", "Final Showdown"], &[]);
    let o = face_down(&mut t, 0, "Dog Walker");
    showdown(&mut t);
    cite!(["611.2c"]; assert!(t.g.abilities(o).is_empty(), "the face-down Walker was among all creatures"));
    t.mana(0, "{R}{W}");
    cite!(["702.168d", "708.8"]; refused(DISGUISE_LOST): unchanged(&mut t.g, |g| g.act(0, paid("RW", Move::TurnFaceUp(o, Way::Disguise)))));
    // "until end of turn" still applies in the end step; it ends in the cleanup step (514.2)
    t.to_no_combat(0, ir::Part::End);
    t.mana(0, "{R}{W}");
    cite!(["514.2", "702.168d"]; refused(DISGUISE_LOST): unchanged(&mut t.g, |g| g.act(0, paid("RW", Move::TurnFaceUp(o, Way::Disguise)))));
    t.to_no_combat(1, ir::Part::Upkeep);
    turn_up(&mut t, o, Way::Disguise, "{R}{W}");
    cite!(["514.2", "702.168d"]; assert_eq!(t.g.obj(o).chars.name, "Dog Walker"));
    cite!(["702.168d"]; assert_eq!(t.g.players[0].pool.total(), 0));
    t.resolve();
    cite!(["603.2"]; assert_eq!(dogs(&t.g), 2, "it has its turned-face-up trigger again"));
    assert!(t.g.has(o, "Vigilance"));
}

#[test]
fn r702_168d_what_it_would_have_face_up_is_asked_not_what_it_has_face_down() {
    // the face-down permanent is colorless (702.168a); the face-up Walker is red and white
    let texts = [
        ("Red Hush", "{0}", "Enchantment", "Red creatures lose all abilities.", None),
        ("Colorless Hush", "{0}", "Enchantment", "Colorless creatures lose all abilities.", None),
    ];
    let mut t = game(&["Dog Walker"], &texts);
    let o = face_down(&mut t, 0, "Dog Walker");
    let mut red = t.clone();
    put(&mut red, 0, "Red Hush", Zone::Battlefield);
    cite!(["702.168a"]; assert_eq!(red.g.abilities(o).len(), 2, "face down it keeps ward {{2}}"));
    red.mana(0, "{R}{W}");
    cite!(["702.168d", "613.1f"]; refused(DISGUISE_LOST): unchanged(&mut red.g, |g| g.act(0, paid("RW", Move::TurnFaceUp(o, Way::Disguise)))));
    put(&mut t, 0, "Colorless Hush", Zone::Battlefield);
    cite!(["613.1f"]; assert!(t.g.abilities(o).is_empty(), "face down it lost ward"));
    t.mana(0, "{R}{W}");
    cite!(["702.168d"]; ok: t.g.act(0, paid("RW", Move::TurnFaceUp(o, Way::Disguise))));
    t.resolve();
    cite!(["702.168d", "603.2"]; assert_eq!(dogs(&t.g), 2));
}

#[test]
fn r701_40d_a_manifested_disguise_card_that_lost_its_abilities_is_turned_up_only_for_its_mana_cost() {
    let mut t = game(&["Dog Walker", "Soul Summons", "Noggle the Mind"], &[]);
    let o = manifest(&mut t, 0, "Dog Walker");
    assert_eq!(t.g.face(o), Face::Down(Down { by: Concealment::Manifested }));
    t.mana(0, "{R}{W}");
    for way in [Way::Disguise, Way::Manifested] {
        cite!(["701.40d"]; case: way.name(); ok: t.g.clone().act(0, paid("RW", Move::TurnFaceUp(o, way))));
    }
    t.g.players[0].pool = Default::default();
    cast_aura(&mut t, "Noggle the Mind", o);
    cite!(["701.40a"]; assert_eq!(t.g.stack.len(), 1, "a manifested 2/2 has no ward"));
    t.resolve();
    t.mana(0, "{R}{W}");
    cite!(["701.40d", "702.168d"]; refused(DISGUISE_LOST): unchanged(&mut t.g, |g| g.act(0, paid("RW", Move::TurnFaceUp(o, Way::Disguise)))));
    // the manifest way asks only the card: a creature card with mana cost {R}{W} (701.40b)
    t.g.act(0, paid("RW", Move::TurnFaceUp(o, Way::Manifested))).unwrap();
    cite!(["701.40b"]; assert_eq!((t.g.obj(o).chars.name.as_str(), t.g.players[0].pool.total()), ("Dog Walker", 0)));
    cite!(["708.8"]; assert_eq!((t.g.power(o), t.g.toughness(o), t.g.seen(o, None).colors), (Some(1), Some(1), vec![])));
    // turned up with no abilities, it has no turned-face-up trigger to trigger (603.10)
    cite!(["603.10", "613.1f"]; assert!(t.g.stack.is_empty() && t.g.abilities(o).is_empty()));
}

#[test]
fn r701_58d_a_cloaked_disguise_card_that_lost_its_abilities_is_turned_up_only_for_its_mana_cost() {
    let mut t = game(&["Dog Walker", "Final Showdown"], &[("Cloaker", "{0}", "Sorcery", "Cloak the top card of your library.", None)]);
    put(&mut t, 0, "Dog Walker", Zone::Library);
    let cloaker = put(&mut t, 0, "Cloaker", Zone::Hand);
    t.g.act(0, Move::Cast { card: cloaker, targets: vec![] }).unwrap();
    t.resolve();
    let o = *t.g.battlefield.iter().find(|o| t.g.face_down(**o)).expect("cloaked");
    assert_eq!(t.g.face(o), Face::Down(Down { by: Concealment::Cloaked }));
    showdown(&mut t);
    t.mana(0, "{R}{W}");
    cite!(["701.58d", "702.168d"]; refused(DISGUISE_LOST): unchanged(&mut t.g, |g| g.act(0, paid("RW", Move::TurnFaceUp(o, Way::Disguise)))));
    t.g.act(0, paid("RW", Move::TurnFaceUp(o, Way::Cloaked))).unwrap();
    cite!(["701.58b"]; assert_eq!((t.g.obj(o).chars.name.as_str(), t.g.players[0].pool.total()), ("Dog Walker", 0)));
    cite!(["603.10", "708.8"]; assert!(t.g.stack.is_empty() && t.g.abilities(o).is_empty()));
}

#[test]
fn r702_37e_a_morph_permanent_that_lost_its_abilities_is_turned_up_once_the_aura_is_gone() {
    let mut t = game(&["Noggle the Mind"], &[
        ("Morph Probe", "{3}{U}", "Creature — Beast", "Flying\nMorph {U}{U}", Some((2, 2))),
        ("Purge", "{0}", "Instant", "Destroy target enchantment.", None),
    ]);
    let o = face_down(&mut t, 0, "Morph Probe");
    cast_aura(&mut t, "Noggle the Mind", o);
    cite!(["702.37a"]; assert_eq!(t.g.stack.len(), 1, "a face-down morph has no ward"));
    t.resolve();
    t.mana(0, "{U}{U}");
    cite!(["702.37e", "708.8"]; refused(MORPH_LOST): unchanged(&mut t.g, |g| g.act(0, paid("UU", Move::TurnFaceUp(o, Way::Morph)))));
    // with the Aura gone, nothing would remove its morph ability face up
    let aura = t.find_in_battlefield("Noggle the Mind");
    let purge = put(&mut t, 0, "Purge", Zone::Hand);
    t.g.act(0, Move::Cast { card: purge, targets: vec![vec![Target::Object(aura)]] }).unwrap();
    t.resolve();
    t.find_in(1, Zone::Graveyard, "Noggle the Mind");
    cite!(["702.37e"]; ok: t.g.act(0, paid("UU", Move::TurnFaceUp(o, Way::Morph))));
    cite!(["702.37e"]; assert_eq!((t.g.obj(o).chars.name.as_str(), t.g.players[0].pool.total()), ("Morph Probe", 0)));
    assert!(t.g.has(o, "Flying"));
}

#[track_caller]
fn unchanged(g: &mut Game, action: impl FnOnce(&mut Game) -> Result<(), String>) -> Result<(), String> {
    let before = format!("{g:?}");
    let result = action(g);
    assert_eq!(format!("{g:?}"), before);
    result
}
