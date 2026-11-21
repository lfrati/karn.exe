//! Protection (702.16). Expectations come from pinned 702.16a-k,
//! 303.4d, 608.2b/h, 611.2c, 615.12 and 113.7a/113.8.
use crate::support::cite;
use crate::support::*;
use game::binding::Value;
use game::select::Ctx;
use ir::Color;

fn checked(name: &str, cost: &str, kind: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    let def = CardDef::new(name, cost, kind, text, pt).unwrap();
    game::play::check(&def).unwrap_or_else(|e| panic!("{name}: {}", e.0));
    def
}

fn choose_color(t: &mut T, p: usize, color: Color) {
    assert!(matches!(t.settle(), Boundary::Question(who, _) if who == p));
    t.answer(p, Answer::Value(Value::Color(color))).unwrap();
    t.resolve();
}

fn targets(g: &Game, source: ObjId, target: Target) -> bool {
    g.can_target(&Ctx::new(g.controller(source), source), target)
}

#[test]
fn r702_16c_review_a_players_own_existing_curse_leaves_when_red_protection_begins() {
    let mut t = Board::new(2).fill(8)
        .hand(0, &["Curse of the Nightly Hunt", "Seht's Tiger"])
        .battlefield(1, &["Prodigal Pyromancer", "Merfolk Looter"])
        .build();
    t.mana(0, "{2}{R}{2}{W}{W}");
    t.cast(0, "Curse of the Nightly Hunt", vec![player(0)], crate::support::mana_pool("CCR")).unwrap();
    t.resolve();
    let curse = t.find_in_battlefield("Curse of the Nightly Hunt");
    assert_eq!(t.g.attached_to(curse), Some(Target::Player(0)), "303.4d prohibits self-enchantment of an Aura, not of its controller");
    t.cast(0, "Seht's Tiger", vec![], crate::support::mana_pool("CCWW")).unwrap();
    choose_color(&mut t, 0, Color::Red);
    cite!(["702.16c"]; assert!(!t.g.battlefield.contains(&curse), "702.16c applies to friendly Auras too"));
    assert_eq!(t.g.players[0].graveyard.len(), 1);
    let red = t.find_in_battlefield("Prodigal Pyromancer");
    let blue = t.find_in_battlefield("Merfolk Looter");
    cite!(["702.16b"]; assert!(!targets(&t.g, red, Target::Player(0))));
    assert!(targets(&t.g, blue, Target::Player(0)));
    t.g.damage(red, Target::Player(0), 2);
    t.g.damage(blue, Target::Player(0), 1);
    cite!(["702.16e"]; assert_eq!(t.g.players[0].life, 19));
}

#[test]
fn r608_2b_review_tigers_response_invalidates_only_the_matching_spell_target() {
    let mut t = Board::new(2).fill(8)
        .hand(0, &["Lightning Bolt"])
        .hand(1, &["Seht's Tiger"])
        .build();
    t.mana(0, "{R}").mana(1, "{2}{W}{W}");
    t.cast(0, "Lightning Bolt", vec![player(1)], crate::support::mana_pool("R")).unwrap();
    t.g.act(0, Move::Pass).unwrap();
    t.cast(1, "Seht's Tiger", vec![], crate::support::mana_pool("CCWW")).unwrap();
    choose_color(&mut t, 1, Color::Red);
    cite!(["608.2b"]; assert_eq!(t.g.players[1].life, 20));
    assert!(t.g.stack.is_empty());
    cite!(["608.2b"]; assert_eq!(t.g.players[0].graveyard.len(), 1));
    assert!(t.g.battlefield.iter().any(|o| t.g.obj(*o).chars.name == "Seht's Tiger"));
}

#[test]
fn r615_12_review_unpreventable_damage_bypasses_player_protection_but_not_targeting() {
    let mut t = Board::new(2).fill(8)
        .battlefield(0, &["The Stasis Coffin"])
        .hand(1, &["Flaring Pain"])
        .battlefield(1, &["Prodigal Pyromancer"])
        .build();
    t.mana(0, "{2}");
    t.activate(0, "The Stasis Coffin", 0, vec![], crate::support::mana_pool("CC")).unwrap();
    t.resolve();
    let source = t.find_in_battlefield("Prodigal Pyromancer");
    cite!(["702.16b"]; assert!(!targets(&t.g, source, Target::Player(0))));
    t.g.damage(source, Target::Player(0), 3);
    cite!(["702.16e"]; assert_eq!(t.g.players[0].life, 20));
    t.g.priority = 1;
    t.mana(1, "{1}{R}");
    t.cast(1, "Flaring Pain", vec![], crate::support::mana_pool("CR")).unwrap();
    t.resolve();
    cite!(["702.16b"]; assert!(!targets(&t.g, source, Target::Player(0)), "615.12 does not remove the targeting prohibition"));
    t.g.damage(source, Target::Player(0), 3);
    cite!(["615.12"]; assert_eq!(t.g.players[0].life, 17));
}

#[test]
fn r702_16_review_independent_color_grants_accumulate_and_expire() {
    let mut t = Board::new(2).fill(8)
        .battlefield(0, &["Jareth, Leonine Titan"])
        .battlefield(1, &["Prodigal Pyromancer", "Merfolk Looter"])
        .build();
    t.mana(0, "{W}{W}");
    let holder = t.find_in_battlefield("Jareth, Leonine Titan");
    let red = t.find_in_battlefield("Prodigal Pyromancer");
    let blue = t.find_in_battlefield("Merfolk Looter");
    t.activate(0, "Jareth, Leonine Titan", 1, vec![], crate::support::mana_pool("W")).unwrap();
    choose_color(&mut t, 0, Color::Blue);
    cite!(["702.16b"]; assert!(!targets(&t.g, blue, Target::Object(holder))));
    assert!(targets(&t.g, red, Target::Object(holder)));
    t.activate(0, "Jareth, Leonine Titan", 1, vec![], crate::support::mana_pool("W")).unwrap();
    choose_color(&mut t, 0, Color::Red);
    for source in [red, blue] {
        cite!(["702.16b"]; assert!(!targets(&t.g, source, Target::Object(holder))));
        assert!(!targets(&t.g.clone(), source, Target::Object(holder)));
    }
    t.to_no_combat(1, Part::PrecombatMain);
    cite!(["514.2"]; assert!(targets(&t.g, red, Target::Object(holder)) && targets(&t.g, blue, Target::Object(holder))));
}

#[test]
fn r702_16b_review_a_sources_second_ability_cannot_change_the_holders_linked_color() {
    let mut g = Game::new(vec![
        oracle("Order of the Stars"),
        checked("Blue Source", "{U}", "Creature — Wizard", "{0}: You gain 1 life.\n{0}: This creature deals 1 damage to any target.", Some((2, 2))),
        checked("Red Source", "{R}", "Creature — Wizard", "{0}: You gain 1 life.\n{0}: This creature deals 1 damage to any target.", Some((2, 2))),
    ], 2, 20);
    let card = g.put(0, 0, Zone::Hand);
    let blue = g.put(1, 1, Zone::Battlefield);
    let red = g.put(1, 2, Zone::Battlefield);
    g.players[0].pool.colors = [1, 0, 0, 0, 0];
    g.act(0, crate::support::paid("W", Move::Cast { card, targets: vec![] })).unwrap();
    let mut t = T::new(g);
    choose_color(&mut t, 0, Color::Blue);
    let holder = t.find_in_battlefield("Order of the Stars");
    t.g.priority = 1;
    cite!(["702.16b"]; refused("an illegal target, Order of the Stars: it has protection from that (702.16b)"): t.g.act(1, Move::Activate { source: blue, ability: 1, targets: vec![vec![Target::Object(holder)]], paying: vec![] }));
    t.g.act(1, Move::Activate { source: red, ability: 1, targets: vec![vec![Target::Object(holder)]], paying: vec![] }).unwrap();
    t.resolve();
    cite!(["702.16e"]; assert!(!t.g.battlefield.contains(&holder), "protection from blue does not prevent the red ability"));
}

#[test]
fn r611_2c_review_granted_opponent_protection_retains_the_granting_players_frame_after_theft() {
    let mut g = Game::new(vec![
        checked("Bear", "{G}", "Creature — Bear", "", Some((2, 2))),
        checked("Shelter", "{0}", "Instant", "Target creature you control gains protection from each of your opponents until end of turn.", None),
        checked("Steal", "{0}", "Instant", "Gain control of all creatures.", None),
        checked("Source", "{0}", "Artifact Creature — Golem", "", Some((2, 2))),
    ], 2, 20);
    let holder = g.put(0, 0, Zone::Battlefield);
    let own = g.put(0, 3, Zone::Battlefield);
    let opponent = g.put(1, 3, Zone::Battlefield);
    let shelter = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card: shelter, targets: vec![vec![Target::Object(holder)]] }).unwrap();
    let mut t = T::new(g);
    t.resolve();
    cite!(["702.16k"]; assert!(!targets(&t.g, opponent, Target::Object(holder))));
    // An untargeted theft avoids protection stopping the intended controller change.
    let steal = t.g.put(1, 2, Zone::Hand);
    t.g.priority = 1;
    t.g.act(1, Move::Cast { card: steal, targets: vec![] }).unwrap();
    t.resolve();
    assert_eq!(t.g.controller(holder), 1);
    // The original grant still names P1, even though P1 now controls the protected creature.
    cite!(["611.2c", "702.16k"]; assert!(!targets(&t.g, opponent, Target::Object(holder))));
    // The original P0 source was stolen too, so it now has the protected-against controller.
    cite!(["702.16k"]; assert!(!targets(&t.g, own, Target::Object(holder))));
    let fresh = t.g.put(0, 3, Zone::Battlefield);
    cite!(["611.2c"]; assert!(targets(&t.g, fresh, Target::Object(holder)), "P0 is not retroactively made an opponent in the original grant"));
}

#[test]
fn r702_16a_review_departed_damage_source_keeps_its_changed_color() {
    let mut t = Board::new(2).fill(8)
        .battlefield(0, &["Eight-and-a-Half-Tails", "Mogg Fanatic"])
        .battlefield(1, &["Silver Knight"])
        .build();
    let fanatic = t.find_in_battlefield("Mogg Fanatic");
    let holder = t.find_in_battlefield("Silver Knight");
    t.mana(0, "{1}");
    t.activate(0, "Eight-and-a-Half-Tails", 1, vec![Tgt::Object(fanatic)], crate::support::mana_pool("C")).unwrap();
    t.resolve();
    assert_eq!(t.g.seen(fanatic, None).colors, [Color::White]);
    t.activate(0, "Mogg Fanatic", 0, vec![Tgt::Object(holder)], crate::support::mana_pool("")).unwrap();
    assert!(!t.g.battlefield.contains(&fanatic));
    t.resolve();
    cite!(["113.7a", "702.16e"]; assert_eq!(t.g.obj(holder).damage, 1, "a white last-known source is not the card's printed red source"));
}

#[test]
fn r702_16_review_static_player_protection_ends_when_its_source_loses_abilities() {
    let mut t = Board::new(3).fill(8)
        .battlefield(0, &["Absolute Virtue"])
        .hand(0, &["Humble"])
        .battlefield(1, &["Prodigal Pyromancer"])
        .battlefield(2, &["Merfolk Looter"])
        .build();
    let red = t.find_in_battlefield("Prodigal Pyromancer");
    let blue = t.find_in_battlefield("Merfolk Looter");
    cite!(["702.16k"]; assert!(!targets(&t.g, red, Target::Player(0)) && !targets(&t.g, blue, Target::Player(0))));
    t.mana(0, "{1}{W}");
    t.cast(0, "Humble", vec!["Absolute Virtue".into()], crate::support::mana_pool("CW")).unwrap();
    t.resolve();
    cite!(["604.2"]; assert!(targets(&t.g, red, Target::Player(0)) && targets(&t.g, blue, Target::Player(0))));
    t.g.damage(red, Target::Player(0), 1);
    assert_eq!(t.g.players[0].life, 19);
}
