//! Protection from qualities other than a fixed color (702.16a–k), through the same
//! targeting, attachment, damage and blocking questions as protection from a color. Real card
//! text, read by the grammar and run by the engine.
use crate::support::cite;
use game::binding::Value;
use game::select::Ctx;
use game::state::Answer;
use game::{CardDef, Event, Game, Move, Target, Zone};
use ir::Color;

fn def(name: &str, cost: &str, kind: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    let d = CardDef::new(name, cost, kind, text, pt).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
    d
}
fn creature(name: &str, cost: &str, kind: &str) -> CardDef {
    def(name, cost, kind, "", Some((2, 2)))
}
fn resolve(g: &mut Game) {
    for _ in 0..2 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn target(g: &Game, source: game::ObjId, of: game::ObjId) -> bool {
    g.can_target(&Ctx::new(g.controller(source), source), Target::Object(of))
}
/// Cast the card at `index` from player 0's hand at these targets, and pass until it resolves.
fn cast(g: &mut Game, index: usize, mana: &str, targets: Vec<Vec<Target>>) -> game::ObjId {
    g.players[0].pool.colors = [10; 5];
    let card = g.put(0, index, Zone::Hand);
    g.act(0, crate::support::paid(mana,Move::Cast { card, targets })).unwrap();
    resolve(g);
    card
}

#[test]
fn r702_16a_a_subtype_is_a_quality_of_permanents_and_of_sources_off_the_battlefield() {
    let mut g = Game::new(vec![
        def("Warren-Scourge Elf", "{1}{G}", "Creature — Elf Warrior", "Protection from Goblins", Some((1, 1))),
        creature("Goblin", "{R}", "Creature — Goblin"),
        creature("Elf", "{R}", "Creature — Elf"),
        def("Goblin Bolt", "{R}", "Kindred Instant — Goblin", "This spell deals 2 damage to target creature.", None),
        def("Bolt", "{R}", "Instant", "This spell deals 2 damage to target creature.", None),
    ], 2, 20);
    let elf = g.put(0, 0, Zone::Battlefield);
    let (goblin, other) = (g.put(1, 1, Zone::Battlefield), g.put(1, 2, Zone::Battlefield));
    cite!(["702.16a", "702.16b"]; assert!(!target(&g, goblin, elf) && target(&g, other, elf)));
    cite!(["702.16a", "702.16f"]; assert!(g.cant_block(goblin, elf, &[goblin]) && !g.cant_block(other, elf, &[other])));
    // a Goblin spell is no permanent, but has the subtype (702.16a, an exception to 109.2)
    let spell = g.put(1, 3, Zone::Hand);
    g.players[1].pool.colors = [1; 5];
    g.priority = 1;
    cite!(["702.16a", "702.16b"]; refused("an illegal target, Warren-Scourge Elf: it has protection from that (702.16b)"): g.act(1, Move::Cast { card: spell, targets: vec![vec![Target::Object(elf)]] }));
    let bolt = g.put(1, 4, Zone::Hand);
    cite!(["702.16b"]; ok: g.act(1, crate::support::paid("R",Move::Cast { card: bolt, targets: vec![vec![Target::Object(elf)]] })));
    // damage from a Goblin is prevented
    g.damage(goblin, Target::Object(elf), 1);
    cite!(["702.16a", "702.16e"]; assert_eq!(g.obj(elf).damage, 0));
}

#[test]
fn r702_16a_multicolored_and_monocolored_count_a_sources_colors() {
    let mut g = Game::new(vec![
        def("Spectrum Sentinel", "{1}", "Artifact Creature — Soldier", "Protection from multicolored", Some((1, 1))),
        def("Guardian of the Guildpact", "{3}{W}", "Creature — Spirit", "Protection from monocolored", Some((2, 3))),
        creature("Gold", "{R}{G}", "Creature — Beast"),
        creature("Red", "{R}", "Creature — Beast"),
        creature("Gray", "{2}", "Artifact Creature — Golem"),
    ], 2, 20);
    let (multi, mono) = (g.put(0, 0, Zone::Battlefield), g.put(0, 1, Zone::Battlefield));
    let (gold, red, gray) = (g.put(1, 2, Zone::Battlefield), g.put(1, 3, Zone::Battlefield), g.put(1, 4, Zone::Battlefield));
    cite!(["702.16a", "702.16b"]; assert_eq!([gold, red, gray].map(|s| target(&g, s, multi)), [false, true, true]));
    cite!(["702.16a", "702.16b"]; assert_eq!([gold, red, gray].map(|s| target(&g, s, mono)), [true, false, true]));
}

#[test]
fn r702_16j_protection_from_everything_stops_every_source_even_a_colorless_one() {
    let mut g = Game::new(vec![
        def("Shielded", "{W}", "Creature — Spirit", "Protection from everything", Some((2, 2))),
        creature("Gray", "{2}", "Artifact Creature — Golem"),
        def("Gear", "{1}", "Artifact — Equipment", "Equipped creature gets +1/+1.\nEquip {0}", None),
        def("Hex", "{1}", "Enchantment — Aura", "Enchant creature\nEnchanted creature gets +1/+1.", None),
    ], 2, 20);
    let guard = g.put(0, 0, Zone::Battlefield);
    let gray = g.put(1, 1, Zone::Battlefield);
    let gear = g.put(0, 2, Zone::Battlefield);
    cite!(["702.16j"]; assert!(!target(&g, gray, guard) && g.cant_block(gray, guard, &[gray])));
    cite!(["702.16j"]; assert!(!g.attach(gear, Target::Object(guard)), "not even its controller's Equipment (702.16j)"));
    let hex = g.put(0, 3, Zone::Battlefield);
    cite!(["702.16j"]; assert!(!g.attach(hex, Target::Object(guard))));
    g.damage(gray, Target::Object(guard), 2);
    cite!(["702.16j"]; assert_eq!(g.obj(guard).damage, 0));
}

#[test]
fn r702_16b_a_description_qualified_by_controller_and_spell_is_asked_of_the_source_as_written() {
    let mut g = Game::new(vec![
        creature("Bear", "{G}", "Creature — Bear"),
        def("Crypsis", "{1}{U}", "Instant", "Target creature you control gains protection from creatures your opponents control until end of turn. Untap it.", None),
        def("Devoted Caretaker", "{W}", "Creature — Human Cleric", "{W}, {T}: Target permanent you control gains protection from instant spells and from sorcery spells until end of turn.", Some((1, 2))),
        creature("Theirs", "{G}", "Creature — Beast"),
        def("Shock", "{R}", "Instant", "This spell deals 2 damage to any target.", None),
        def("Pinger", "{R}", "Creature — Human", "{T}: This creature deals 1 damage to any target.", Some((1, 1))),
    ], 2, 20);
    let bear = g.put(0, 0, Zone::Battlefield);
    let mine = g.put(0, 3, Zone::Battlefield);
    let theirs = g.put(1, 3, Zone::Battlefield);
    cast(&mut g, 1, "UU", vec![vec![Target::Object(bear)]]);
    cite!(["702.16f"]; assert!(g.cant_block(theirs, bear, &[theirs]) && !g.cant_block(mine, bear, &[mine])));
    cite!(["702.16b"]; assert!(!target(&g, theirs, bear) && target(&g, mine, bear)));
    // an opponent's instant is no creature
    let shock = g.put(1, 4, Zone::Stack);
    assert!(g.can_target(&Ctx { targeting_spell: true, ..Ctx::new(1, shock) }, Target::Object(bear)));

    let caretaker = g.put(0, 2, Zone::Battlefield);
    let bear2 = g.put(0, 0, Zone::Battlefield);
    g.players[0].pool.colors = [10; 5];
    g.act(0, crate::support::paid("W",Move::Activate { source: caretaker, ability: 0, targets: vec![vec![Target::Object(bear2)]], paying: vec![] })).unwrap();
    resolve(&mut g);
    let shock = g.put(1, 4, Zone::Stack);
    cite!(["702.16b"]; assert!(!g.can_target(&Ctx { targeting_spell: true, ..Ctx::new(1, shock) }, Target::Object(bear2))));
    // an ability's source is a creature, not an instant or sorcery spell
    let pinger = g.put(1, 5, Zone::Battlefield);
    cite!(["702.16b"]; assert!(target(&g, pinger, bear2)));
}

/// Gods Willing: the color is chosen as it resolves; a red Aura and Equipment already on the
/// creature leave by the state-based action after it resolves (702.16c–d, 704.5m–n).
#[test]
fn r702_16_the_color_of_your_choice_is_chosen_on_resolution_and_kept_by_the_effect() {
    let mut g = Game::new(vec![
        creature("Bear", "{G}", "Creature — Bear"),
        def("Gods Willing", "{W}", "Instant", "Target creature you control gains protection from the color of your choice until end of turn.\nScry 1.", None),
        creature("Red", "{R}", "Creature — Goblin"),
        creature("Blue", "{U}", "Creature — Merfolk"),
        def("Aura", "{R}", "Enchantment — Aura", "Enchant creature\nEnchanted creature gets +1/+1.", None),
        def("Gear", "{R}", "Artifact — Equipment", "Equipped creature gets +1/+1.\nEquip {0}", None),
    ], 2, 20);
    let bear = g.put(0, 0, Zone::Battlefield);
    let (red, blue) = (g.put(1, 2, Zone::Battlefield), g.put(1, 3, Zone::Battlefield));
    let aura = g.put(0, 4, Zone::Battlefield);
    let gear = g.put(0, 5, Zone::Battlefield);
    assert!(g.attach(aura, Target::Object(bear)) && g.attach(gear, Target::Object(bear)));
    for _ in 0..3 { g.put(0, 3, Zone::Library); }
    cast(&mut g, 1, "W", vec![vec![Target::Object(bear)]]);
    cite!(["608.2d"]; assert!(target(&g, red, bear), "no color is chosen before it resolves"));
    g.act(0, Move::Choose(Answer::Value(Value::Color(Color::Red)))).unwrap();
    // scry 1: keep the card looked at on top
    let library = g.players[0].library.clone();
    for card in library {
        if g.resolving().is_none() { break; }
        let _ = g.act(0, Move::Choose(Answer::Arrange { top: vec![card], away: vec![] }));
    }
    assert!(g.resolving().is_none());
    cite!(["702.16b"]; assert!(!target(&g, red, bear) && target(&g, blue, bear)));
    assert!(format!("{:?}", g.continuous).contains("Red"), "the effect keeps the chosen color");
    let clone = g.clone();
    assert!(!target(&clone, red, bear), "a copied game keeps the chosen color");
    assert_eq!(format!("{:?}", clone.continuous), format!("{:?}", g.continuous));
    // the Aura and the Equipment left after the spell resolved, as the state-based action
    // the spell on the stack is a new object (400.7): the one its cast event names
    let log = g.log.since(g.log.start());
    let spell = log.iter().find_map(|e| match e { Event::Cast { spell, .. } => Some(*spell), _ => None }).expect("cast");
    let resolved = log.iter().position(|e| *e == Event::Resolved { spell }).expect("resolved");
    let moved = log.iter().position(|e| matches!(e, Event::ZoneChanged { was, from: Zone::Battlefield, to: Zone::Graveyard, .. } if *was == aura)).expect("aura to graveyard");
    cite!(["702.16c", "704.5m"]; assert!(resolved < moved));
    assert_eq!(g.obj(gear).zone, Zone::Battlefield);
    cite!(["702.16d", "704.5n"]; assert_eq!(g.attached_to(gear), None));
    // until end of turn (514.2)
    while g.turn == 1 { g.act(g.priority, Move::Pass).unwrap_or_else(|_| { let _ = g.act(g.priority, Move::Attack(vec![])); }); }
    cite!(["514.2"]; assert!(target(&g, red, bear)));
}

#[test]
fn r702_16_the_chosen_color_of_an_earlier_choice_or_an_entry_choice() {
    let mut g = Game::new(vec![
        def("Brave the Elements", "{W}", "Instant", "Choose a color. White creatures you control gain protection from the chosen color until end of turn.", None),
        creature("Knight", "{W}", "Creature — Human Knight"),
        creature("Bear", "{G}", "Creature — Bear"),
        creature("Red", "{R}", "Creature — Goblin"),
        def("Order of the Stars", "{W}", "Creature — Human Cleric", "Defender\nAs this creature enters, choose a color.\nThis creature has protection from the chosen color.", Some((0, 1))),
        creature("Blue", "{U}", "Creature — Merfolk"),
    ], 2, 20);
    let (knight, bear, red) = (g.put(0, 1, Zone::Battlefield), g.put(0, 2, Zone::Battlefield), g.put(1, 3, Zone::Battlefield));
    cast(&mut g, 0, "W", vec![]);
    g.act(0, Move::Choose(Answer::Value(Value::Color(Color::Red)))).unwrap();
    cite!(["702.16b"]; assert!(!target(&g, red, knight) && target(&g, red, bear), "white creatures only"));

    let order = cast(&mut g, 4, "W", vec![]);
    g.act(0, Move::Choose(Answer::Value(Value::Color(Color::Blue)))).unwrap();
    let order = g.battlefield.iter().copied().find(|o| g.obj(*o).chars.name == "Order of the Stars" && *o != order).unwrap_or(order);
    let blue = g.put(1, 5, Zone::Battlefield);
    cite!(["702.16b"]; assert!(!target(&g, blue, order) && target(&g, red, order)));
}

#[test]
fn r702_16k_protection_from_each_of_your_opponents_stops_what_they_control() {
    let mut g = Game::new(vec![
        creature("Bear", "{G}", "Creature — Bear"),
        def("Shelter", "{2}{W}", "Instant", "Target creature you control gains protection from each of your opponents until end of turn.", None),
        creature("Gray", "{2}", "Artifact Creature — Golem"),
    ], 2, 20);
    let bear = g.put(0, 0, Zone::Battlefield);
    let (mine, theirs) = (g.put(0, 2, Zone::Battlefield), g.put(1, 2, Zone::Battlefield));
    cast(&mut g, 1, "WWW", vec![vec![Target::Object(bear)]]);
    cite!(["702.16k"]; assert!(!target(&g, theirs, bear) && target(&g, mine, bear)));
    cite!(["702.16k"]; assert!(g.cant_block(theirs, bear, &[theirs])));
    g.damage(theirs, Target::Object(bear), 1);
    g.damage(mine, Target::Object(bear), 1);
    cite!(["702.16k"]; assert_eq!(g.obj(bear).damage, 1));
}

#[test]
fn r702_16_what_protection_does_not_yet_say_stays_refused() {
    for (kind, text) in [
        ("Creature — Spirit", "Protection from the chosen color"),
        ("Creature — Spirit", "Protection from the chosen player"),
        // "or" is a choice among the qualities made as an effect resolves
        ("Instant", "Target creature gains protection from artifacts or from the color of your choice until end of turn."),
        ("Creature — Spirit", "Protection from the card name of your choice"),
        ("Creature — Spirit", "Protection from the color of your choice"),
    ] {
        if text=="Protection from the card name of your choice" {
            assert_eq!(CardDef::new("Unsupported","{W}",kind,text,Some((2,2))).expect_err("unsupported name-choice grammar"),"text the grammar does not read");
        } else {
            let card=CardDef::new("Unsupported","{W}",kind,text,if kind.starts_with("Creature") {Some((2,2))}else{None}).unwrap();
            let reason=if text=="Protection from the chosen color" {"linked choice: no ability identity"}else{"keyword: Protection"};
            crate::support::refused(game::play::check(&card).map_err(|e|e.0),reason);
        }
    }
}

fn targets_player(g: &Game, source: game::ObjId, player: usize) -> bool {
    g.can_target(&Ctx::new(g.controller(source), source), Target::Player(player))
}

/// Absolute Virtue: a player with protection from each of their opponents can't be
/// targeted, dealt damage or enchanted by what those opponents control (702.16b, c, e, k).
#[test]
fn r702_16k_a_player_with_protection_from_each_of_their_opponents() {
    let mut g = Game::new(vec![
        def("Absolute Virtue", "{6}{W}{U}", "Creature — Avatar Warrior", "This spell can't be countered.\nFlying\nYou have protection from each of your opponents.", Some((8, 8))),
        creature("Gray", "{2}", "Artifact Creature — Golem"),
        def("Curse", "{0}", "Enchantment — Aura Curse", "Enchant player\nAt the beginning of your upkeep, enchanted player loses 1 life.", None),
    ], 2, 20);
    let (mine, theirs) = (g.put(0, 1, Zone::Battlefield), g.put(1, 1, Zone::Battlefield));
    cite!(["113.6"]; assert!(targets_player(&g, theirs, 0), "no protection before it is on the battlefield"));
    g.put(0, 0, Zone::Battlefield);
    cite!(["702.16k"]; assert!(!targets_player(&g, theirs, 0) && targets_player(&g, mine, 0)));
    assert!(targets_player(&g, theirs, 1), "the opponent has no protection");
    g.damage(theirs, Target::Player(0), 3);
    g.damage(mine, Target::Player(0), 1);
    cite!(["702.16k"]; assert_eq!(g.players[0].life, 19));
    // 702.16c: the opponent's Curse can't enchant the player; the player's own Curse can
    // (each starts on the opponent and is moved, 701.3b)
    let theirs_curse = g.put(1, 2, Zone::Battlefield);
    g.attach(theirs_curse, Target::Player(1));
    cite!(["702.16k"]; assert!(!g.attach(theirs_curse, Target::Player(0))));
    assert_eq!(g.attached_to(theirs_curse), Some(Target::Player(1)));
    let mine_curse = g.put(0, 2, Zone::Battlefield);
    g.attach(mine_curse, Target::Player(1));
    cite!(["702.16k"]; assert!(g.attach(mine_curse, Target::Player(0))));
}

/// Seht's Tiger: "you gain protection from the color of your choice until end of turn". The
/// color is chosen as the trigger resolves; the player keeps protection from it, and only
/// it, until end of turn (514.2).
#[test]
fn r702_16_a_player_gains_protection_from_a_color_chosen_as_it_resolves() {
    let mut g = Game::new(vec![
        def("Seht's Tiger", "{2}{W}{W}", "Creature — Cat", "Flash\nWhen this creature enters, you gain protection from the color of your choice until end of turn.", Some((3, 2))),
        creature("Red", "{R}", "Creature — Goblin"),
        creature("Blue", "{U}", "Creature — Merfolk"),
    ], 2, 20);
    let (red, blue) = (g.put(1, 1, Zone::Battlefield), g.put(1, 2, Zone::Battlefield));
    cast(&mut g, 0, "WWWW", vec![]);
    resolve(&mut g);
    cite!(["608.2d"]; assert!(targets_player(&g, red, 0), "no color is chosen before it resolves"));
    g.act(0, Move::Choose(Answer::Value(Value::Color(Color::Red)))).unwrap();
    assert!(g.stack.is_empty() && g.resolving().is_none());
    cite!(["702.16b"]; assert!(!targets_player(&g, red, 0) && targets_player(&g, blue, 0)));
    g.damage(red, Target::Player(0), 2);
    g.damage(blue, Target::Player(0), 1);
    cite!(["702.16e"]; assert_eq!(g.players[0].life, 19));
    while g.turn == 1 { g.act(g.priority, Move::Pass).unwrap_or_else(|_| { let _ = g.act(g.priority, Move::Attack(vec![])); }); }
    cite!(["514.2"]; assert!(targets_player(&g, red, 0)));
}

/// The Stasis Coffin: protection from everything (702.16j) stops the player's own sources
/// too, and lasts until that player's next turn, through the opponent's turn (611.2a).
#[test]
fn r702_16j_a_player_with_protection_from_everything_until_their_next_turn() {
    let mut g = Game::new(vec![
        def("The Stasis Coffin", "{1}", "Legendary Artifact", "{2}, {T}, Exile The Stasis Coffin: You gain protection from everything until your next turn.", None),
        creature("Gray", "{2}", "Artifact Creature — Golem"),
    ], 2, 20);
    for player in 0..2 { for _ in 0..3 { g.put(player, 1, Zone::Library); } }
    let coffin = g.put(0, 0, Zone::Battlefield);
    let (mine, theirs) = (g.put(0, 1, Zone::Battlefield), g.put(1, 1, Zone::Battlefield));
    g.players[0].pool.colors = [10; 5];
    g.act(0, crate::support::paid("WW",Move::Activate { source: coffin, ability: 0, targets: vec![], paying: vec![] })).unwrap();
    resolve(&mut g);
    assert_ne!(g.obj(coffin).zone, Zone::Battlefield);
    cite!(["702.16j"]; assert!(!targets_player(&g, theirs, 0) && !targets_player(&g, mine, 0)));
    g.damage(theirs, Target::Player(0), 3);
    cite!(["702.16j"]; assert_eq!(g.players[0].life, 20));
    let pass = |g: &mut Game| { g.act(g.priority, Move::Pass).unwrap_or_else(|_| { let _ = g.act(g.priority, Move::Attack(vec![])); }); };
    while g.turn == 1 { pass(&mut g); }
    assert_eq!(g.active, 1);
    cite!(["611.2a"]; assert!(!targets_player(&g, theirs, 0), "still in force on the opponent's turn"));
    while g.turn == 2 { pass(&mut g); }
    cite!(["611.2a"]; assert!(targets_player(&g, theirs, 0) && targets_player(&g, mine, 0)));
}

/// Hunted Horror's tokens: a token is created with the protection its description says, and
/// a copy of it copies that keyword with its quality (707.2, 111.3).
#[test]
fn r702_16_a_token_created_with_protection_and_its_copy() {
    let mut g = Game::new(vec![
        def("Hunted Horror", "{B}{B}", "Creature — Horror", "Trample\nWhen this creature enters, target opponent creates two 3/3 green Centaur creature tokens with protection from black.", Some((7, 7))),
        def("Mirror", "{0}", "Instant", "Create a token that's a copy of target creature.", None),
        creature("Black", "{B}", "Creature — Zombie"),
        creature("Red", "{R}", "Creature — Goblin"),
    ], 2, 20);
    let (black, red) = (g.put(0, 2, Zone::Battlefield), g.put(0, 3, Zone::Battlefield));
    g.players[0].pool.colors = [10; 5];
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, crate::support::paid("BB",Move::Cast { card, targets: vec![] })).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap();
    resolve(&mut g);
    let centaurs: Vec<_> = g.battlefield.iter().copied().filter(|o| g.obj(*o).token() && g.controller(*o) == 1).collect();
    assert_eq!(centaurs.len(), 2);
    let centaur = centaurs[0];
    cite!(["111.3", "702.16b"]; assert!(!target(&g, black, centaur) && target(&g, red, centaur)));
    g.damage(black, Target::Object(centaur), 2);
    cite!(["702.16e"]; assert_eq!(g.obj(centaur).damage, 0));
    cast(&mut g, 1, "", vec![vec![Target::Object(centaur)]]);
    let copy = *g.battlefield.iter().find(|o| g.obj(**o).token() && g.controller(**o) == 0).expect("copy");
    cite!(["707.2", "111.3"]; assert!(!target(&g, black, copy) && target(&g, red, copy)));
}

/// The same route gives a player hexproof or shroud for a time (702.11c, 702.18a).
#[test]
fn r702_11c_a_player_gains_hexproof_or_shroud_for_a_time() {
    let mut g = Game::new(vec![
        def("Blossoming Calm", "{W}", "Instant", "You gain hexproof until your next turn. You gain 2 life.\nRebound", None),
        def("Gilded Light", "{1}{W}", "Instant", "You gain shroud until end of turn.\nCycling {2}", None),
        creature("Gray", "{2}", "Artifact Creature — Golem"),
    ], 2, 20);
    let (mine, theirs) = (g.put(0, 2, Zone::Battlefield), g.put(1, 2, Zone::Battlefield));
    cast(&mut g, 0, "W", vec![]);
    assert_eq!(g.players[0].life, 22);
    cite!(["702.11c"]; assert!(!targets_player(&g, theirs, 0) && targets_player(&g, mine, 0)));
    cast(&mut g, 1, "WW", vec![]);
    cite!(["702.18a"]; assert!(!targets_player(&g, mine, 0), "shroud: not even its own sources"));
}
