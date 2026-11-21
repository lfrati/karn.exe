//! The reasons a refused target gives (115.1): each must name something
//! that is actually false of the target, and agree with what legality checks.
use crate::support::cite;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

fn game_of(players: usize, custom: Vec<CardDef>) -> Game {
    let mut defs = custom;
    defs.push(CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap());
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let island = defs.len() - 1;
    let mut g = Game::new(defs, players, 20);
    for p in 0..players { for _ in 0..8 { g.put(p, island, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn game(custom: Vec<CardDef>) -> Game { game_of(2, custom) }
fn c(name: &str, cost: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef { CardDef::new(name, cost, types, text, pt).expect(name) }
fn refusal(g: &mut Game, card: ObjId, targets: Vec<Vec<Target>>) -> String {
    g.act(0, Move::Cast { card, targets }).expect_err("refused")
}

/// "target opponent or planeswalker" at yourself: you are a player, and not an opponent.
#[test]
fn opponent_or_planeswalker_at_yourself_says_you_are_no_opponent() {
    let mut g = game(vec![c("Jet", "{0}", "Sorcery", "Jet deals 6 damage to target opponent or planeswalker.", None)]);
    let jet = g.put(0, 0, Zone::Hand);
    let said = refusal(&mut g, jet, vec![vec![Target::Player(0)]]);
    cite!(["102.2", "115.1"]; assert_eq!(said, "an illegal target, you: you aren't an opponent (115.1)"));
    assert!(!said.contains("isn't a card"), "a player is refused for not being a card: {said}");
    cite!(["102.2", "115.1"]; ok: g.act(0, Move::Cast { card: jet, targets: vec![vec![Target::Player(1)]] }));
}

/// A union whose arms share a trailing controller: the refusal must name the controller, not
/// claim the object is not of the arm it does belong to.
#[test]
fn spell_or_creature_you_control_at_their_creature_names_the_controller() {
    let mut g = game(vec![
        c("Bounce", "{0}", "Instant", "Tap target artifact or creature you control.", None),
        c("Bear", "{0}", "Creature — Bear", "", Some((2, 2))),
    ]);
    let bear = g.put(1, 1, Zone::Battlefield);
    let bounce = g.put(0, 0, Zone::Hand);
    let said = refusal(&mut g, bounce, vec![vec![Target::Object(bear)]]);
    cite!(["115.1"]; assert_eq!(said, "an illegal target, Bear: the target is one you control, and another player controls it (115.1)"));
}

/// An Aura's own activated ability with nothing to target is not an Aura spell looking for
/// something to enchant (303.4a is about casting an Aura spell).
#[test]
fn an_auras_activated_ability_with_no_target_does_not_cite_enchanting() {
    let mut g = game(vec![
        c("Shackle", "{0}", "Enchantment — Aura", "Enchant creature\n{0}: Tap target artifact.", None),
        c("Bear", "{0}", "Creature — Bear", "", Some((2, 2))),
    ]);
    let bear = g.put(0, 1, Zone::Battlefield);
    let aura = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: aura, targets: vec![vec![Target::Object(bear)]] }).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    let aura = *g.battlefield.iter().find(|o| g.obj(**o).chars.name == "Shackle").expect("attached");
    let mut said = Vec::new();
    for ability in 0..3 {
        if let Err(e) = g.act(0, Move::Activate { source: aura, ability, targets: vec![vec![]], paying: vec![] }) { said.push(e); }
    }
    assert_eq!(said.len(), 3, "{said:?}");
    cite!(["601.2c"]; assert_eq!(said[1], "there is no legal target for it (601.2c)"));
    assert!(!said.iter().any(|s| s.contains("303.4a")), "{said:?}");
}

/// "another target creature" after a first target excludes that first target, not the source:
/// a reason saying it is "the object whose ability this is" is false of a spell's target.
#[test]
fn another_target_excluding_the_first_target_is_not_said_to_be_the_source() {
    let mut g = game(vec![
        c("Brawl", "{0}", "Sorcery", "Brawl deals 2 damage to target creature and 1 damage to another target creature.", None),
        c("Bear", "{0}", "Creature — Bear", "", Some((2, 2))),
    ]);
    let mine = g.put(0, 1, Zone::Battlefield);
    g.put(1, 1, Zone::Battlefield);
    let brawl = g.put(0, 0, Zone::Hand);
    let said = refusal(&mut g, brawl, vec![vec![Target::Object(mine)], vec![Target::Object(mine)]]);
    eprintln!("another: {said}");
    assert!(!said.contains("whose ability this is"), "{said}");
    cite!(["601.2c"]; assert_eq!(said, "another target must differ from the preceding targets (115.1, 601.2c)"));
}

/// A player with hexproof (702.11c), targeted by an opponent.
#[test]
fn a_player_with_hexproof() {
    let mut g = game(vec![
        c("Leyline", "{0}", "Enchantment", "You have hexproof.", None),
        c("Shock", "{0}", "Instant", "Shock deals 2 damage to any target.", None),
    ]);
    g.put(1, 0, Zone::Battlefield);
    let shock = g.put(0, 1, Zone::Hand);
    let said = refusal(&mut g, shock, vec![vec![Target::Player(1)]]);
    cite!(["702.11c"]; assert_eq!(said, "an illegal target, that player: that player has hexproof (702.11c)"));
}

/// Hexproof and protection from another color: the spell's color is not protected from, so
/// hexproof is the reason.
#[test]
fn hexproof_not_protection_when_the_color_differs() {
    let mut g = game(vec![
        c("Bolt", "{R}", "Instant", "Bolt deals 3 damage to target creature.", None),
        c("Wisp", "{0}", "Creature — Spirit", "Hexproof, protection from blue", Some((1, 1))),
        c("Knight", "{0}", "Creature — Knight", "Protection from red", Some((2, 2))),
        c("Veil", "{0}", "Creature — Spirit", "Shroud", Some((1, 1))),
    ]);
    let wisp = g.put(1, 1, Zone::Battlefield);
    let knight = g.put(1, 2, Zone::Battlefield);
    let veil = g.put(0, 3, Zone::Battlefield);
    let bolt = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[3] = 1;
    let said = refusal(&mut g, bolt, vec![vec![Target::Object(wisp)]]);
    cite!(["702.11b"]; assert_eq!(said, "an illegal target, Wisp: it has hexproof, and an opponent of its controller is targeting it (702.11b)"));
    let said = refusal(&mut g, bolt, vec![vec![Target::Object(knight)]]);
    cite!(["702.16b"]; assert_eq!(said, "an illegal target, Knight: it has protection from that (702.16b)"));
    let said = refusal(&mut g, bolt, vec![vec![Target::Object(veil)]]);
    cite!(["702.18a"]; assert_eq!(said, "an illegal target, Veil: it has shroud (702.18a)"));
}

/// Filter atoms: a negated keyword, a negated color, a spell's type, a non-spell on the stack.
#[test]
fn filter_atoms_are_named_for_what_they_are() {
    let mut g = game(vec![
        c("Swat", "{0}", "Instant", "Destroy target nonblack creature without flying.", None),
        c("Bat", "{0}", "Creature — Bat", "Flying", Some((1, 1))),
        c("Shade", "{B}", "Creature — Shade", "", Some((1, 1))),
        c("Essence", "{0}", "Instant", "Counter target creature spell.", None),
        c("Rite", "{0}", "Sorcery", "You gain 1 life.", None),
    ]);
    let bat = g.put(1, 1, Zone::Battlefield);
    let shade = g.put(1, 2, Zone::Battlefield);
    let swat = g.put(0, 0, Zone::Hand);
    cite!(["115.1"]; assert_eq!(refusal(&mut g, swat, vec![vec![Target::Object(bat)]]), "an illegal target, Bat: it has flying (115.1)"));
    cite!(["115.1"]; assert_eq!(refusal(&mut g, swat, vec![vec![Target::Object(shade)]]), "an illegal target, Shade: it is black (115.1)"));
    let said = refusal(&mut g, swat, vec![vec![Target::Player(1)]]);
    cite!(["115.1"]; assert_eq!(said, "an illegal target, that player: a player isn't a permanent (115.1)"));
    // a sorcery spell on the stack is a spell, but not a creature spell
    let rite = g.put(0, 4, Zone::Hand);
    g.act(0, Move::Cast { card: rite, targets: vec![] }).unwrap();
    let rite = g.stack[0];
    let essence = g.put(0, 3, Zone::Hand);
    cite!(["115.1"]; assert_eq!(refusal(&mut g, essence, vec![vec![Target::Object(rite)]]), "an illegal target, Rite: it isn't a creature (115.1)"));
    cite!(["115.1"]; assert_eq!(refusal(&mut g, essence, vec![vec![Target::Object(bat)]]), "an illegal target, Bat: it isn't a spell (115.1)"));
}

/// With three players: "an opponent controls" against your own creature, "you control" against
/// a third player's.
#[test]
fn multiplayer_controller_reasons() {
    let mut g = game_of(3, vec![
        c("Steal", "{0}", "Sorcery", "Tap target creature an opponent controls.", None),
        c("Boost", "{0}", "Sorcery", "Target creature you control gets +1/+1 until end of turn.", None),
        c("Bear", "{0}", "Creature — Bear", "", Some((2, 2))),
    ]);
    let mine = g.put(0, 2, Zone::Battlefield);
    let third = g.put(2, 2, Zone::Battlefield);
    let steal = g.put(0, 0, Zone::Hand);
    let boost = g.put(0, 1, Zone::Hand);
    cite!(["115.1"]; assert_eq!(refusal(&mut g, steal, vec![vec![Target::Object(mine)]]),
        "an illegal target, Bear: the target is one an opponent controls, and you control it (115.1)"));
    cite!(["115.1"]; assert_eq!(refusal(&mut g, boost, vec![vec![Target::Object(third)]]),
        "an illegal target, Bear: the target is one you control, and another player controls it (115.1)"));
    cite!(["115.1"]; ok: g.act(0, Move::Cast { card: steal, targets: vec![vec![Target::Object(third)]] }));
}

/// "another target creature" on an ability of a creature, at that creature.
#[test]
fn another_at_the_source_itself() {
    let mut g = game(vec![
        c("Pinger", "{0}", "Creature — Wizard", "{0}: Pinger deals 1 damage to another target creature.", Some((1, 1))),
    ]);
    let pinger = g.put(0, 0, Zone::Battlefield);
    let said = g.act(0, Move::Activate { source: pinger, ability: 0, targets: vec![vec![Target::Object(pinger)]], paying: vec![] }).unwrap_err();
    cite!(["115.1"]; assert_eq!(said, "an illegal target, Pinger: it is the object whose ability this is, and the target says \"another\" (115.1)"));
}

/// Two targets wanted, one exists: the count refusal.
#[test]
fn too_few_legal_targets_says_how_many() {
    let mut g = game(vec![
        c("Twin", "{0}", "Sorcery", "Tap two target creatures.", None),
        c("Bear", "{0}", "Creature — Bear", "", Some((2, 2))),
    ]);
    let bear = g.put(1, 1, Zone::Battlefield);
    let twin = g.put(0, 0, Zone::Hand);
    let said = refusal(&mut g, twin, vec![vec![Target::Object(bear)]]);
    eprintln!("one of two: {said}");
    cite!(["601.2c"]; assert_eq!(said, "there is only 1 legal target, and it needs 2 (601.2c)"));
}

/// A union of domains with a shared controller ("spell or nonland permanent an opponent
/// controls"): at your own permanent the reason is who controls it, not that it isn't a spell —
/// it is a nonland permanent, which the union allows.
#[test]
fn spell_or_nonland_permanent_an_opponent_controls_at_your_own_names_the_controller() {
    let mut g = game(vec![
        c("Unsummon", "{0}", "Instant", "Return target spell or nonland permanent an opponent controls to its owner's hand.", None),
        c("Bear", "{0}", "Creature — Bear", "", Some((2, 2))),
    ]);
    let mine = g.put(0, 1, Zone::Battlefield);
    let theirs = g.put(1, 1, Zone::Battlefield);
    let unsummon = g.put(0, 0, Zone::Hand);
    let said = refusal(&mut g, unsummon, vec![vec![Target::Object(mine)]]);
    eprintln!("union controller: {said}");
    cite!(["115.1"]; assert_eq!(said, "an illegal target, Bear: the target is one an opponent controls, and you control it (115.1)"));
    assert!(!said.contains("isn't a spell"), "{said}");
    cite!(["115.1"]; ok: g.act(0, Move::Cast { card: unsummon, targets: vec![vec![Target::Object(theirs)]] }));
}

/// "spell or ability an opponent controls" at your own spell: it is a spell.
#[test]
fn spell_or_ability_an_opponent_controls_at_your_own_spell_names_the_controller() {
    let mut g = game(vec![
        c("Stifle", "{0}", "Instant", "Counter target spell or ability an opponent controls.", None),
        c("Rite", "{0}", "Sorcery", "You gain 1 life.", None),
    ]);
    let rite = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card: rite, targets: vec![] }).unwrap();
    let rite = g.stack[0];
    let stifle = g.put(0, 0, Zone::Hand);
    let said = refusal(&mut g, stifle, vec![vec![Target::Object(rite)]]);
    eprintln!("spell or ability: {said}");
    cite!(["115.1"]; assert_eq!(said, "an illegal target, Rite: the target is one an opponent controls, and you control it (115.1)"));
    assert!(!said.contains("isn't an ability"), "{said}");
}

/// Unions and keywords after the union rework: each reason is true of the target, and a
/// player's hexproof or shroud is named (702.11c, 702.18a).
#[test]
fn union_and_player_keyword_reasons_after_the_rework() {
    let mut g = game(vec![
        c("Doom", "{0}", "Instant", "Destroy target creature or planeswalker.", None),
        c("Bear", "{0}", "Creature — Bear", "", Some((2, 2))),
        c("Fry", "{0}", "Instant", "Destroy target creature or planeswalker an opponent controls.", None),
        c("Blast", "{0}", "Instant", "Blast deals 3 damage to target player or planeswalker.", None),
        c("Leyline", "{0}", "Enchantment", "You have hexproof.", None),
        c("Veil", "{0}", "Creature — Spirit", "Shroud", Some((1, 1))),
        c("Jet", "{0}", "Sorcery", "Jet deals 6 damage to target opponent or planeswalker.", None),
        c("Sanct", "{0}", "Enchantment", "You have shroud.", None),
    ]);
    let island = g.put(0, 8, Zone::Battlefield);
    let bear = g.put(0, 1, Zone::Battlefield);
    let veil = g.put(1, 5, Zone::Battlefield);
    g.put(1, 4, Zone::Battlefield);
    let doom = g.put(0, 0, Zone::Hand);
    let fry = g.put(0, 2, Zone::Hand);
    let blast = g.put(0, 3, Zone::Hand);
    let jet = g.put(0, 6, Zone::Hand);
    let said = refusal(&mut g, doom, vec![vec![Target::Object(island)]]);
    cite!(["115.1"]; assert_eq!(said, "an illegal target, Island: it is neither a creature nor a planeswalker (115.1)"));
    let said = refusal(&mut g, fry, vec![vec![Target::Object(bear)]]);
    cite!(["115.1"]; assert_eq!(said, "an illegal target, Bear: the target is one an opponent controls, and you control it (115.1)"));
    cite!(["702.18a"]; assert_eq!(refusal(&mut g, doom, vec![vec![Target::Object(veil)]]), "an illegal target, Veil: it has shroud (702.18a)"));
    cite!(["702.11c"]; assert_eq!(refusal(&mut g, blast, vec![vec![Target::Player(1)]]), "an illegal target, that player: that player has hexproof (702.11c)"));
    cite!(["702.11c"]; assert_eq!(refusal(&mut g, jet, vec![vec![Target::Player(1)]]), "an illegal target, that player: that player has hexproof (702.11c)"));
    cite!(["115.1"]; assert_eq!(refusal(&mut g, jet, vec![vec![Target::Player(0)]]), "an illegal target, you: you aren't an opponent (115.1)"));
    g.put(0, 7, Zone::Battlefield);
    cite!(["702.18a"]; assert_eq!(refusal(&mut g, blast, vec![vec![Target::Player(0)]]), "an illegal target, you: you have shroud (702.18a)"));
}
