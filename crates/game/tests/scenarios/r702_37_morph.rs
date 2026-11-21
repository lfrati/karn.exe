//! Morph (702.37): a card is cast face down as a 2/2 creature spell with no text, no name, no
//! subtypes and no mana cost, for {3} rather than its mana cost (702.37a/c). It is turned face
//! down before it is put on the stack, so what cares about the spell sees only the face-down
//! spell (708.4). It resolves into a face-down permanent, which its controller may turn face up
//! any time they have priority by paying its morph cost (702.37e), and a countered face-down
//! spell is revealed (708.9). Scornful Egotist ({7}{U}, morph {U}), Glacial Stalker ({5}{U},
//! morph {4}{U}) and Zoetic Cavern (a land with morph {2}) are Oracle cards.
use crate::support::cite;
use crate::support;
use game::announcement::CastCost;
use game::face::{Concealment, Down, Face, Way};
use game::observation::Kind;
use game::Event;
use support::*;

type Text<'a> = (&'a str, &'a str, &'a str, &'a str, Option<(i32, i32)>);

/// Oracle cards by name, then test texts, each at its index among the definitions.
fn game(names: &[&str], texts: &[Text]) -> T {
    let mut defs: Vec<CardDef> = names.iter().map(|n| oracle(n)).collect();
    for (name, cost, types, text, pt) in texts {
        let d = CardDef::new(name, cost, types, text, *pt).unwrap();
        game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
        defs.push(d);
    }
    let mut g = Game::new(defs, 2, 20);
    let filler = g.defs.len();
    std::sync::Arc::make_mut(&mut g.defs).push(CardDef::new("Wastes", "", "Basic Land", "", None).unwrap());
    for p in 0..2 { for _ in 0..10 { g.put(p, filler, Zone::Library); } }
    T::new(g)
}
fn put(t: &mut T, p: usize, name: &str, zone: Zone) -> ObjId {
    let index = t.g.defs.iter().position(|d| d.name() == name).unwrap_or_else(|| panic!("no {name}"));
    t.g.put(p, index, zone)
}
/// Cast this card face down for the alternative cost of its ability at `row`.
fn cast_face_down(g: &mut Game, p: usize, card: ObjId, row: usize, mana: &str) -> Result<(), String> {
    g.act(p, paid(mana, Move::CastFor(CastCost::Alternative(row), Box::new(Move::Cast { card, targets: vec![] }))))
}
/// Player `p` casts this card face down with three mana and it resolves: the face-down
/// permanent it becomes.
fn morph(t: &mut T, p: usize, card: ObjId, row: usize) -> ObjId {
    while t.g.priority != p { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.mana(p, "{3}");
    cast_face_down(&mut t.g, p, card, row, "CCC").unwrap();
    let before = t.g.battlefield.clone();
    t.resolve();
    let new: Vec<ObjId> = t.g.battlefield.iter().copied().filter(|o| !before.contains(o)).collect();
    assert_eq!(new.len(), 1, "it entered");
    new[0]
}


#[test]
fn r702_37c_it_is_cast_face_down_for_three_as_a_nameless_2_2_creature_spell() {
    let mut t = game(&["Scornful Egotist"], &[]);
    let card = put(&mut t, 0, "Scornful Egotist", Zone::Hand);
    t.mana(0, "{3}");
    cite!(["601.2h"]; refused("the supplied mana does not pay that cost, all of it and no more"): unchanged(&mut t.g, |g| g.act(0, paid("CCC", Move::Cast { card, targets: vec![] }))));
    cast_face_down(&mut t.g, 0, card, 0, "CCC").unwrap();
    cite!(["702.37c"]; assert_eq!(t.g.players[0].pool.total(), 0, "{{3}} paid rather than its mana cost (702.37c)"));
    assert!(t.g.players[0].hand.is_empty());
    let spell = *t.g.stack.last().unwrap();
    assert_eq!(t.g.face(spell), Face::Down(Down { by: Concealment::Morph }));
    let seen = t.g.seen(spell, None);
    cite!(["702.37c"]; assert_eq!(t.g.obj(spell).chars.name, ""));
    cite!(["702.37c"]; assert_eq!(seen.types, vec![ir::CardType::Creature]));
    assert!(seen.subtypes.is_empty() && seen.colors.is_empty());
    cite!(["702.37c"]; assert_eq!(t.g.obj(spell).chars.mana_value, 0, "no mana cost"));
    assert_eq!(t.g.obj(spell).chars.pt, Some((2, 2)));
    cite!(["702.37c"]; assert!(t.g.abilities(spell).is_empty(), "no text"));
    assert_eq!(t.g.physical_card(spell).map(|(_, d)| d.name()), Some("Scornful Egotist"));
    t.resolve();
    let o = *t.g.battlefield.iter().find(|o| t.g.face_down(**o)).expect("it enters face down (702.37c)");
    assert_eq!(t.g.face(o), Face::Down(Down { by: Concealment::Morph }));
    cite!(["708.4", "708.2a"]; assert_eq!((t.g.obj(o).chars.name.as_str(), t.g.power(o), t.g.toughness(o)), ("", Some(2), Some(2))));
    assert_eq!(t.g.controller(o), 0);
}

#[test]
fn r702_37c_its_mana_cost_and_its_morph_cost_are_separate_announcements() {
    let mut t = game(&["Scornful Egotist"], &[]);
    let card = put(&mut t, 0, "Scornful Egotist", Zone::Hand);
    t.mana(0, "{7}{U}");
    cite!(["702.37a", "118.9"]; ok: cast_face_down(&mut t.g.clone(), 0, card, 0, "CCC"));
    // cast face up, it is the Egotist on the stack
    t.g.act(0, paid("CCCCCCCU", Move::Cast { card, targets: vec![] })).unwrap();
    let spell = *t.g.stack.last().unwrap();
    cite!(["702.37d"]; assert!(!t.g.face_down(spell)));
    assert_eq!(t.g.obj(spell).chars.name, "Scornful Egotist");
}

#[test]
fn r702_37c_a_face_down_spell_has_no_text_to_announce_with() {
    let mut t = game(&["Scornful Egotist"], &[]);
    let card = put(&mut t, 0, "Scornful Egotist", Zone::Hand);
    t.mana(0, "{3}");
    // the row of no morph ability
    cite!(["118.9"]; refused("that card prints no such alternative cost (118.9)"): unchanged(&mut t.g, |g| cast_face_down(g, 0, card, 1, "CCC")) );
    // only in a main phase of its controller's turn with the stack empty: it is a creature
    // spell (117.1a)
    let mut later = t.g.clone();
    later.act(0, Move::Pass).unwrap();
    later.act(1, Move::Pass).unwrap();
    assert_eq!((later.part, later.priority), (Part::BeginningOfCombat, 0));
    later.players[0].pool=mana_pool("CCC"); // the phase transition emptied the earlier pool (106.4)
    cite!(["117.1a", "708.4"]; refused("that is cast in a main phase of your own turn with the stack empty"): unchanged(&mut later, |g| cast_face_down(g, 0, card, 0, "CCC")));
    // what is not in a zone it could be cast from is not cast face down either (702.37c)
    let mut t = game(&["Scornful Egotist"], &[]);
    let card = put(&mut t, 0, "Scornful Egotist", Zone::Graveyard);
    t.mana(0, "{3}");
    cite!(["702.37c", "601.3"]; refused("that card is not in your hand: it is in a graveyard, and no rule or effect lets you cast it from there (601.3)"): unchanged(&mut t.g, |g| cast_face_down(g, 0, card, 0, "CCC")));
}

#[test]
fn r708_4_cost_changes_and_cast_triggers_see_only_the_face_down_spell() {
    let mut t = game(&["Scornful Egotist", "Spined Basher"], &[
        ("Creature rebate", "{0}", "Enchantment", "Creature spells you cast cost {1} less to cast.", None),
        ("Colorless tax", "{0}", "Enchantment", "Colorless spells you cast cost {1} more to cast.", None),
        ("Creature tithe", "{0}", "Enchantment", "Whenever you cast a creature spell, you gain 1 life.", None),
        ("Big watcher", "{0}", "Enchantment", "Whenever a player casts a spell with mana value 5 or greater, you draw a card.", None),
    ]);
    // a black card cast face down is a colorless spell: taxed (708.4)
    put(&mut t, 0, "Colorless tax", Zone::Battlefield);
    let basher = put(&mut t, 0, "Spined Basher", Zone::Hand);
    t.mana(0, "{3}");
    cite!(["708.4"]; refused("the supplied mana does not pay that cost, all of it and no more"): unchanged(&mut t.g, |g| cast_face_down(g, 0, basher, 0, "CCC")));
    t.mana(0, "{1}");
    assert!(cast_face_down(&mut t.g.clone(), 0, basher, 0, "CCCC").is_ok());
    cast_face_down(&mut t.g, 0, basher, 0, "CCCC").unwrap();
    cite!(["708.4", "118.9d"]; assert_eq!(t.g.players[0].pool.total(), 0, "{{4}} paid"));
    t.resolve();
    // a creature spell: the rebate applies; the mana value 8 card is a mana value 0 spell
    let mut t = game(&["Scornful Egotist"], &[
        ("Creature rebate", "{0}", "Enchantment", "Creature spells you cast cost {1} less to cast.", None),
        ("Creature tithe", "{0}", "Enchantment", "Whenever you cast a creature spell, you gain 1 life.", None),
        ("Big watcher", "{0}", "Enchantment", "Whenever a player casts a spell with mana value 5 or greater, you draw a card.", None),
    ]);
    put(&mut t, 0, "Creature rebate", Zone::Battlefield);
    put(&mut t, 0, "Creature tithe", Zone::Battlefield);
    put(&mut t, 1, "Big watcher", Zone::Battlefield);
    let card = put(&mut t, 0, "Scornful Egotist", Zone::Hand);
    t.mana(0, "{2}");
    cast_face_down(&mut t.g, 0, card, 0, "CC").unwrap();
    cite!(["708.4"]; assert_eq!(t.g.players[0].pool.total(), 0, "{{2}}: a creature spell (708.4)"));
    t.resolve();
    cite!(["708.4"]; assert_eq!(t.g.players[0].life, 21, "a creature spell was cast"));
    cite!(["708.4"]; assert_eq!(t.g.players[1].hand.len(), 0, "no spell with mana value 5 or greater was cast"));
}

#[test]
fn r708_4_a_land_card_cast_face_down_is_a_creature_spell_a_flash_permission_applies_to() {
    let mut t = game(&["Zoetic Cavern"], &[("Quickening", "{0}", "Enchantment", "You may cast creature spells as though they had flash.", None)]);
    let cavern = put(&mut t, 0, "Zoetic Cavern", Zone::Hand);
    t.mana(0, "{3}");
    cite!(["305.1"]; ok: t.g.clone().act(0, Move::PlayLand(cavern)));
    assert!(cast_face_down(&mut t.g.clone(), 0, cavern, 1, "CCC").is_ok());
    // in the opponent's turn only with a permission to cast creature spells as though they
    // had flash (708.4)
    t.g.act(0, Move::Pass).unwrap();
    t.g.act(1, Move::Pass).unwrap();
    while t.g.active != 1 || t.g.priority != 0 {
        let p = t.g.decision_maker(t.g.decision_actor());
        let mv = if matches!(t.g.decision, Some(game::state::Decision::Attackers { .. })) { Move::Attack(vec![]) } else { Move::Pass };
        t.g.act(p, mv).unwrap();
    }
    t.mana(0, "{3}");
    cite!(["117.1a"]; refused("that is cast in a main phase of your own turn with the stack empty"): unchanged(&mut t.g, |g| cast_face_down(g, 0, cavern, 1, "CCC")));
    put(&mut t, 0, "Quickening", Zone::Battlefield);
    cite!(["708.4"]; ok: cast_face_down(&mut t.g.clone(), 0, cavern, 1, "CCC"));
    cast_face_down(&mut t.g, 0, cavern, 1, "CCC").unwrap();
    t.resolve();
    let o = *t.g.battlefield.iter().find(|o| t.g.face_down(**o)).unwrap();
    cite!(["702.37c"]; assert!(t.g.seen(o, None).is(ir::CardType::Creature) && !t.g.seen(o, None).is(ir::CardType::Land)));
    // turned face up for {2} it is the land it is, when its controller next has priority
    assert_eq!(t.g.priority, 1, "the active player");
    t.g.act(1, Move::Pass).unwrap();
    t.mana(0, "{2}");
    cite!(["701.40b"]; refused("only a manifested permanent is turned face up for its card's mana cost"): unchanged(&mut t.g, |g| g.act(0, paid("CC", Move::TurnFaceUp(o, Way::Manifested)))));
    t.g.act(0, paid("CC", Move::TurnFaceUp(o, Way::Morph))).unwrap();
    assert_eq!(t.g.obj(o).chars.name, "Zoetic Cavern");
    cite!(["702.37e"]; assert!(t.g.seen(o, None).is(ir::CardType::Land) && !t.g.seen(o, None).is(ir::CardType::Creature)));
}

#[test]
fn r702_37e_the_special_action_pays_the_morph_cost_with_a_nonempty_stack_and_nothing_enters() {
    let mut t = game(&["Glacial Stalker", "Lightning Bolt", "Soul Warden"], &[]);
    put(&mut t, 0, "Soul Warden", Zone::Battlefield);
    let card = put(&mut t, 0, "Glacial Stalker", Zone::Hand);
    let o = morph(&mut t, 0, card, 0);
    assert_eq!(t.g.players[0].life, 21, "a face-down creature entered");
    for p in 0..2 { t.g.take_observations(p); }
    // the opponent bolts it; in response its controller turns it face up
    t.g.act(0, Move::Pass).unwrap();
    let bolt = put(&mut t, 1, "Lightning Bolt", Zone::Hand);
    t.mana(1, "{R}");
    t.g.act(1, paid("R",Move::Cast { card: bolt, targets: vec![vec![Target::Object(o)]] })).unwrap();
    t.g.act(1, Move::Pass).unwrap();
    assert_eq!(t.g.priority, 0);
    cite!(["118.3a"]; refused("the supplied mana does not pay that cost, all of it and no more"): unchanged(&mut t.g, |g| g.act(0, paid("", Move::TurnFaceUp(o, Way::Morph)))));
    let mut asked = t.g.clone();
    asked.priority = 1;
    asked.players[1].pool = mana_pool("CCCCU");
    cite!(["116.2b", "702.37e"]; refused("only its controller turns a face-down permanent face up (116.2b)"): unchanged(&mut asked, |g| g.act(1, paid("CCCCU", Move::TurnFaceUp(o, Way::Morph)))));
    t.mana(0, "{4}{U}");
    assert!(t.g.observations(1).is_empty(), "listing it shows no one anything");
    let from = t.mark();
    t.g.act(0, paid("CCCCU", Move::TurnFaceUp(o, Way::Morph))).unwrap();
    cite!(["702.37e", "116.2b"]; assert_eq!(t.g.stack.len(), 1, "a special action does not use the stack (116.2b)"));
    cite!(["116.3"]; assert_eq!(t.g.priority, 0));
    assert_eq!(t.g.players[0].pool.total(), 0, "{{4}}{{U}} paid");
    cite!(["708.8"]; assert!(!t.g.face_down(o) && t.g.battlefield.contains(&o), "the same permanent (708.8)"));
    cite!(["702.37e"]; assert_eq!((t.g.obj(o).chars.name.as_str(), t.g.power(o), t.g.toughness(o)), ("Glacial Stalker", Some(4), Some(5))));
    assert_eq!(t.g.log.since(from).iter().filter(|e| matches!(e, Event::TurnedFaceUp { object } if *object == o)).count(), 1);
    for p in 0..2 {
        let shown = t.g.observations(p);
        assert_eq!(shown.len(), 1, "seat {p}: shown once, as it was announced");
        cite!(["702.37e"]; case: format!("seat {p}"); assert_eq!((shown[0].kind, shown[0].by, shown[0].cards[0].name.as_str()), (Kind::Revealed, 0, "Glacial Stalker")));
    }
    t.resolve();
    cite!(["708.8"]; assert_eq!(t.g.players[0].life, 21, "turning it up is not entering (702.37e)"));
    assert!(t.g.battlefield.contains(&o), "a 4/5 survives the bolt");
}

#[test]
fn r701_40c_a_manifested_card_with_morph_may_be_turned_up_either_way() {
    let mut t = game(&["Soul Summons", "Glacial Stalker", "Grizzly Bears"], &[]);
    let soul = put(&mut t, 0, "Soul Summons", Zone::Hand);
    put(&mut t, 0, "Glacial Stalker", Zone::Library);
    t.mana(0, "{1}{W}");
    t.g.act(0, paid("CW",Move::Cast { card: soul, targets: vec![] })).unwrap();
    t.resolve();
    let o = *t.g.battlefield.iter().find(|o| t.g.face_down(**o)).unwrap();
    cite!(["701.40a"]; assert_eq!(t.g.face(o), Face::Down(Down { by: Concealment::Manifested })));
    t.mana(0, "{5}{U}");
    for way in [Way::Manifested, Way::Morph] {
        cite!(["701.40c"]; ok: t.g.clone().act(0, paid(if way == Way::Manifested { "CCCCCU" } else { "CCCCU" }, Move::TurnFaceUp(o, way))));
    }
    t.g.players[0].pool = Default::default();
    t.mana(0, "{4}{U}");
    cite!(["701.40c", "118.3a"]; refused("the supplied mana does not pay that cost, all of it and no more"): unchanged(&mut t.g, |g| g.act(0, paid("CCCCU", Move::TurnFaceUp(o, Way::Manifested)))));
    t.g.act(0, paid("CCCCU", Move::TurnFaceUp(o, Way::Morph))).unwrap();
    cite!(["701.40c", "702.37e"]; assert_eq!(t.g.obj(o).chars.name, "Glacial Stalker"));
    // a manifested card with no morph ability has only its mana cost
    let soul = put(&mut t, 0, "Soul Summons", Zone::Hand);
    put(&mut t, 0, "Grizzly Bears", Zone::Library);
    t.mana(0, "{1}{W}");
    t.g.act(0, paid("CW",Move::Cast { card: soul, targets: vec![] })).unwrap();
    t.resolve();
    let bears = *t.g.battlefield.iter().find(|o| t.g.face_down(**o)).unwrap();
    t.mana(0, "{5}{G}");
    cite!(["701.40b"]; ok: t.g.clone().act(0, paid("CG", Move::TurnFaceUp(bears, Way::Manifested))));
    cite!(["702.37e"]; refused("that permanent would have no such cost if it were face up (702.37e)"): unchanged(&mut t.g, |g| g.act(0, Move::TurnFaceUp(bears, Way::Morph))));
}

#[test]
fn r708_9_a_countered_face_down_spell_is_revealed_by_its_owner() {
    let mut t = game(&["Scornful Egotist", "Cancel"], &[]);
    let card = put(&mut t, 0, "Scornful Egotist", Zone::Hand);
    t.mana(0, "{3}");
    cast_face_down(&mut t.g, 0, card, 0, "CCC").unwrap();
    let spell = *t.g.stack.last().unwrap();
    for p in 0..2 { t.g.take_observations(p); }
    t.g.act(0, Move::Pass).unwrap();
    let cancel = put(&mut t, 1, "Cancel", Zone::Hand);
    t.mana(1, "{1}{U}{U}");
    t.g.act(1, paid("CUU",Move::Cast { card: cancel, targets: vec![vec![Target::Object(spell)]] })).unwrap();
    let from = t.mark();
    t.resolve();
    assert!(t.g.stack.is_empty());
    let revealed: Vec<_> = t.g.log.since(from).iter().filter_map(|e| match e { Event::Revealed { player, cards } => Some((*player, cards.clone())), _ => None }).collect();
    cite!(["708.9"]; assert_eq!(revealed, vec![(0, vec![spell])], "by its owner, as the face-down spell it was"));
    for p in 0..2 {
        let shown = t.g.observations(p);
        assert_eq!(shown.len(), 1, "seat {p}");
        cite!(["708.9"]; case: format!("seat {p}"); assert_eq!((shown[0].kind, shown[0].by, shown[0].cards[0].object, shown[0].cards[0].name.as_str()), (Kind::Revealed, 0, spell, "Scornful Egotist")));
    }
    let card = t.g.players[0].graveyard[0];
    assert!(!t.g.face_down(card), "in the graveyard it is a face-up card");
    assert_eq!(t.g.obj(card).chars.name, "Scornful Egotist");
}

#[test]
fn r708_5_only_its_controller_sees_the_face_down_spell() {
    let mut t = game(&["Scornful Egotist"], &[]);
    let card = put(&mut t, 0, "Scornful Egotist", Zone::Hand);
    t.mana(0, "{3}");
    cast_face_down(&mut t.g, 0, card, 0, "CCC").unwrap();
    let spell = *t.g.stack.last().unwrap();
    cite!(["708.5"]; assert!(t.g.sees(0, spell) && !t.g.sees(1, spell)));
    assert!(t.g.perceives(1, spell), "every player knows the spell is there");
    let shown = |viewer| game::snapshot::Snapshot::of(&t.g, Some(viewer)).objects.into_iter().find(|o| o.id == spell).expect("perceived");
    let mine = shown(0);
    assert_eq!(mine.name, "");
    cite!(["708.5"]; assert_eq!(mine.face_down.as_ref().map(|d| (d.by, d.card.as_ref().map(|c| c.name.clone()))), Some((Concealment::Morph, Some("Scornful Egotist".to_owned())))));
    let theirs = shown(1);
    assert_eq!(theirs.name, "");
    cite!(["708.6"]; assert_eq!(theirs.face_down.as_ref().map(|d| (d.by, d.card.is_none())), Some((Concealment::Morph, true)), "how it is face down is public (708.6)"));
    cite!(["708.5"]; assert!(game::snapshot::Snapshot::of(&t.g, Some(1)).objects.iter().all(|o| o.name != "Scornful Egotist")));
}

#[test]
fn r702_37_morph_costs_the_engine_does_not_pay_are_refused_by_name() {
    for (text, why) in [
        ("Morph—Pay 5 life.", "Morph: a cost that is not mana"),
        ("Morph {X}{X}{R}", "Morph: X in its cost"),
        ("Megamorph {X}{G}", "Megamorph: X in its cost"),
        ("Disguise {X}{3}{W}", "Disguise: X in its cost"),
    ] {
        let d = CardDef::new("Probe", "{4}", "Creature — Beast", text, Some((3, 3))).unwrap();
        let refused = game::play::check(&d).expect_err(text).0;
        assert!(refused.starts_with(why), "{text}: {refused}");
    }
    for text in ["Flying\nMorph {2}{U}", "Megamorph {2}{G}", "Morph {2}\nWhen this creature is turned face up, you gain 2 life."] {
        let d = CardDef::new("Probe", "{4}", "Creature — Beast", text, Some((3, 3))).unwrap();
        game::play::check(&d).unwrap_or_else(|e| panic!("{text}: {}", e.0));
    }
}

/// What this seat was shown of the card with this name: how, by whom, as which object.
fn revealed(g: &Game, seat: usize, name: &str) -> Vec<(Kind, usize, ObjId)> {
    g.observations(seat).iter().flat_map(|d| d.cards.iter().filter(|c| c.name == name).map(|c| (d.kind, d.by, c.object))).collect()
}

#[test]
fn r708_9_at_the_end_of_the_game_a_face_down_spell_is_revealed_once() {
    let mut t = game(&["Scornful Egotist", "Lightning Bolt"], &[]);
    let card = put(&mut t, 0, "Scornful Egotist", Zone::Hand);
    t.mana(0, "{3}");
    cast_face_down(&mut t.g, 0, card, 0, "CCC").unwrap();
    let spell = *t.g.stack.last().unwrap();
    for p in 0..2 { t.g.take_observations(p); }
    t.g.players[0].life = 3;
    t.expect_losses(&[0]);
    t.g.act(0, Move::Pass).unwrap();
    let bolt = put(&mut t, 1, "Lightning Bolt", Zone::Hand);
    t.mana(1, "{R}");
    t.g.act(1, paid("R",Move::Cast { card: bolt, targets: vec![vec![Target::Player(0)]] })).unwrap();
    t.settle();
    assert!(t.g.finished());
    cite!(["708.9"]; assert_eq!(revealed(&t.g, 1, "Scornful Egotist"), vec![(Kind::Revealed, 0, spell)]));
}

#[test]
fn r708_9_a_player_who_leaves_reveals_their_face_down_spell_once() {
    let mut t = game(&["Scornful Egotist", "Lightning Bolt"], &[]);
    let mut g = Game::new(t.g.defs.clone(), 3, 20);
    std::mem::swap(&mut t.g, &mut g);
    (t.g.active, t.g.priority) = (1, 1);
    let card = put(&mut t, 1, "Scornful Egotist", Zone::Hand);
    t.mana(1, "{3}");
    cast_face_down(&mut t.g, 1, card, 0, "CCC").unwrap();
    let spell = *t.g.stack.last().unwrap();
    for p in 0..3 { t.g.take_observations(p); }
    t.g.players[1].life = 3;
    t.expect_losses(&[1]);
    while t.g.priority != 0 { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    let bolt = put(&mut t, 0, "Lightning Bolt", Zone::Hand);
    t.mana(0, "{R}");
    t.g.act(0, paid("R",Move::Cast { card: bolt, targets: vec![vec![Target::Player(1)]] })).unwrap();
    t.settle();
    assert!(!t.g.finished() && t.g.players[1].lost);
    cite!(["800.4a"]; assert!(!t.g.stack.contains(&spell), "it left the game with its owner"));
    for seat in [0, 2] { cite!(["708.9"]; case: format!("seat {seat}"); assert_eq!(revealed(&t.g, seat, "Scornful Egotist"), vec![(Kind::Revealed, 1, spell)], "seat {seat}")); }
}

#[test]
fn r702_37e_a_face_down_copy_is_turned_up_for_the_morph_cost_of_what_it_copies() {
    let mut t = game(&["Scornful Egotist", "Glacial Stalker", "Grizzly Bears"], &[
        ("Forever", "{0}", "Instant", "Target creature becomes a copy of target creature.", None),
    ]);
    let card = put(&mut t, 0, "Scornful Egotist", Zone::Hand);
    let o = morph(&mut t, 0, card, 0);
    let stalker = put(&mut t, 1, "Glacial Stalker", Zone::Battlefield);
    let forever = put(&mut t, 0, "Forever", Zone::Hand);
    t.g.act(0, Move::Cast { card: forever, targets: vec![vec![Target::Object(o)], vec![Target::Object(stalker)]] }).unwrap();
    t.resolve();
    assert!(t.g.face_down(o));
    cite!(["708.10"]; assert_eq!((t.g.power(o), t.g.toughness(o)), (Some(2), Some(2)), "still the face-down 2/2 (708.10)"));
    t.mana(0, "{U}");
    cite!(["702.37e", "118.3a"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): t.g.act(0, crate::support::paid("U",Move::TurnFaceUp(o, Way::Morph))));
    t.mana(0, "{4}");
    t.g.act(0, paid("CCCCU", Move::TurnFaceUp(o, Way::Morph))).unwrap();
    cite!(["708.10"]; assert_eq!((t.g.obj(o).chars.name.as_str(), t.g.power(o)), ("Glacial Stalker", Some(4))));
    // a copy of a card with no morph ability has no morph cost face up
    let card = put(&mut t, 0, "Scornful Egotist", Zone::Hand);
    let o = morph(&mut t, 0, card, 0);
    let bears = put(&mut t, 1, "Grizzly Bears", Zone::Battlefield);
    let forever = put(&mut t, 0, "Forever", Zone::Hand);
    t.g.act(0, Move::Cast { card: forever, targets: vec![vec![Target::Object(o)], vec![Target::Object(bears)]] }).unwrap();
    t.resolve();
    t.mana(0, "{7}{U}");
    cite!(["702.37e"]; refused("that permanent would have no such cost if it were face up (702.37e)"): t.g.act(0, Move::TurnFaceUp(o, Way::Morph)));
}

#[test]
fn r118_9a_a_card_cast_without_paying_its_mana_cost_is_not_cast_face_down() {
    let mut t = game(&["Scornful Egotist"], &[
        ("Gift", "{R}", "Instant", "Exile the top card of your library. You may cast that card without paying its mana cost.", None),
    ]);
    let egotist = put(&mut t, 0, "Scornful Egotist", Zone::Library);
    let gift = put(&mut t, 0, "Gift", Zone::Hand);
    t.mana(0, "{R}{3}");
    t.g.act(0, paid("R",Move::Cast { card: gift, targets: vec![] })).unwrap();
    while t.g.asked().is_some_and(|(_, a)| a == Asked::Priority) && !t.g.stack.is_empty() { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    let card = t.g.exile.iter().copied().find(|o| t.g.obj(*o).card == t.g.obj(egotist).card).expect("exiled");
    let Some((0, Asked::Cast { cards })) = t.g.asked() else { panic!("{:?}", t.g.asked()) };
    assert_eq!(cards, vec![card]);
    cite!(["118.9a"]; refused("the effect lets that card be cast only for the cost it offers (118.9a)"): cast_face_down(&mut t.g, 0, card, 0, "CCC"));
    t.g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    let spell = *t.g.stack.last().unwrap();
    assert!(!t.g.face_down(spell));
    cite!(["118.9"]; assert_eq!(t.g.players[0].pool.total(), 3, "free"));
}

#[track_caller]
fn unchanged(g: &mut Game, action: impl FnOnce(&mut Game) -> Result<(), String>) -> Result<(), String> {
    let before=format!("{g:?}");
    let result=action(g);
    assert_eq!(format!("{g:?}"),before);
    result
}
