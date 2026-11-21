//! Resolution selections and references to completed moves (608.2c–d, 701.20, 400.7).
use crate::support::{cite, paid};
use game::{CardDef, Event, Game, Move, ObjId, Zone};
use game::state::{Answer, Choice, Decision};

const WAVE: usize = 0;
const BEAR: usize = 1;
const GIANT: usize = 2;
const LAND: usize = 3;
const SPELL: usize = 4;
const WATCHER: usize = 5;
const CHOOSER: usize = 6;
const AURA: usize = 7;
const RELIC: usize = 8;

fn game() -> Game {
    static DEFS: std::sync::OnceLock<Vec<CardDef>> = std::sync::OnceLock::new();
    let defs = DEFS.get_or_init(|| {
        let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
        let wave = CardDef::from_face(faces.iter().find(|f| f.name == "Genesis Wave").unwrap()).unwrap();
        let mut defs = vec![wave];
        for (name, cost, kind, text, pt) in [
            ("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
            ("Giant", "{7}{G}", "Creature — Giant", "", Some((8, 8))),
            ("Forest", "", "Basic Land — Forest", "", None),
            ("Reward", "{0}", "Instant", "You gain 1 life.", None),
            ("Watcher", "{G}", "Creature — Elf", "Whenever another creature enters, you gain 1 life.", Some((1, 1))),
            ("Chooser", "{G}", "Creature — Elf", "As this creature enters, choose a creature type.", Some((1, 1))),
            ("Aura", "{G}", "Enchantment — Aura", "Enchant creature", None),
            ("Relic", "{3}", "Artifact", "", None),
        ] { defs.push(CardDef::new(name, cost, kind, text, pt).unwrap()); }
        for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
        defs
    });
    Game::new(defs.clone(), 2, 20)
}
fn resolve(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
fn cast(g: &mut Game, x: u32) {
    let card = g.put(0, WAVE, Zone::Hand);
    g.players[0].pool.colors[4] = x + 3;
    g.act(0, paid(&"G".repeat((x+3) as usize), Move::WithX(x, Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    resolve(g);
}
fn selection(g: &Game) -> Vec<ObjId> {
    match &g.decision {
        Some(Decision::Choose { player: 0, choice: Choice::Select { among, min: 0, max } }) if *max == among.len() => among.clone(),
        other => panic!("expected subset selection, got {other:?}"),
    }
}
fn choose(g: &mut Game, selected: Vec<ObjId>) { g.act(0, Move::Choose(Answer::Objects(selected))).unwrap(); }
fn count(g: &Game, zone: Zone, def: usize) -> usize {
    let cards = match zone { Zone::Battlefield => &g.battlefield, Zone::Graveyard => &g.players[0].graveyard, Zone::Library => &g.players[0].library, _ => panic!() };
    cards.iter().filter(|o| g.obj(**o).card == Some(def)).count()
}
fn reveals(g: &Game) -> usize { g.log.iter().filter(|e| matches!(e, Event::Revealed { .. })).count() }

#[test]
fn r107_3a_full_corpus_wave_with_zero_x_does_nothing_to_the_library() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Library);
    cast(&mut g, 0);
    assert!(g.decision.is_none() && g.stack.is_empty());
    cite!(["107.3a"]; assert_eq!(g.players[0].library, vec![bear]));
    assert!(g.battlefield.is_empty());
    assert_eq!(count(&g, Zone::Graveyard, WAVE), 1);
}

#[test]
fn r608_2d_subset_is_bounded_by_permanent_type_x_and_the_revealed_group() {
    let mut g = game();
    let hidden = g.put(0, BEAR, Zone::Library);
    let bear = g.put(0, BEAR, Zone::Library);
    let forest = g.put(0, LAND, Zone::Library);
    let relic = g.put(0, RELIC, Zone::Library);
    let giant = g.put(0, GIANT, Zone::Library);
    let instant = g.put(0, SPELL, Zone::Library);
    cast(&mut g, 5);
    let among = selection(&g);
    cite!(["608.2d"]; assert_eq!(among, vec![relic, forest, bear]));
    let before = format!("{g:?}");
    // too great a mana value, not a permanent card, not revealed; then the same card twice
    for (selected, why) in [(vec![giant], "a chosen object is not among the offered objects (608.2d)"),
        (vec![instant], "a chosen object is not among the offered objects (608.2d)"),
        (vec![hidden], "a chosen object is not among the offered objects (608.2d)"),
        (vec![bear, bear], "each object is chosen once (608.2d)")] {
        cite!(["608.2d"]; case: format!("{selected:?}"); refused(why): g.act(0, Move::Choose(Answer::Objects(selected))));
        assert_eq!(format!("{g:?}"), before);
    }
    crate::support::refused(g.act(1, Move::Choose(Answer::Objects(vec![bear]))), "that decision is another player's");
    let clone = g.clone();
    for mut g in [g, clone] {
        choose(&mut g, vec![forest, bear]);
        // CR404.3: owner authors these simultaneous arrivals' top-first order.
        assert_eq!(g.decision,Some(game::state::Decision::Choose{player:0,choice:game::state::Choice::OrderCards{to:game::state::Placement::Zone(Zone::Graveyard),cards:vec![instant,giant,relic]}}));
        g.act(0,Move::Choose(game::state::Answer::Objects(vec![relic,giant,instant]))).unwrap();

        assert!(g.decision.is_none() && g.stack.is_empty());
        assert_eq!(g.players[0].library, vec![hidden]);
        cite!(["608.2d"]; assert_eq!(g.battlefield.len(), 2));
        for def in [RELIC, GIANT, SPELL, WAVE] { cite!(["608.2c"]; case: format!("{def}"); assert_eq!(count(&g, Zone::Graveyard, def), 1)); }
        cite!(["608.2c"]; assert_eq!(reveals(&g), 1, "resumption does not repeat the reveal"));
        crate::support::refused(g.act(0, Move::Choose(Answer::Objects(vec![bear]))), "no decision is pending");
    }
}

#[test]
fn r608_2d_any_number_includes_none_and_all_and_short_libraries() {
    for take_all in [false, true] {
        let mut g = game();
        let bear=g.put(0, BEAR, Zone::Library);
        let land=g.put(0, LAND, Zone::Library);
        cast(&mut g, 7);
        let among = selection(&g);
        choose(&mut g, if take_all { among } else { vec![] });
        if !take_all {
            // CR404.3: owner authors these simultaneous arrivals' top-first order.
            assert_eq!(g.decision,Some(game::state::Decision::Choose{player:0,choice:game::state::Choice::OrderCards{to:game::state::Placement::Zone(Zone::Graveyard),cards:vec![land,bear]}}));
            g.act(0,Move::Choose(game::state::Answer::Objects(vec![bear,land]))).unwrap();
        }
        cite!(["608.2d"]; case: format!("take_all={take_all}"); assert_eq!(g.battlefield.len(), if take_all { 2 } else { 0 }));
        assert_eq!(g.players[0].graveyard.len(), if take_all { 1 } else { 3 });
        assert!(g.players[0].library.is_empty());
    }
    let mut g = game();
    let giant=g.put(0, GIANT, Zone::Library);
    let instant=g.put(0, SPELL, Zone::Library);
    cast(&mut g, 2);
    // CR404.3: owner authors these simultaneous arrivals' top-first order.
    assert_eq!(g.decision,Some(game::state::Decision::Choose{player:0,choice:game::state::Choice::OrderCards{to:game::state::Placement::Zone(Zone::Graveyard),cards:vec![instant,giant]}}));
    g.act(0,Move::Choose(game::state::Answer::Objects(vec![giant,instant]))).unwrap();
    assert!(g.decision.is_none(), "an empty eligible set needs no selection");
    assert_eq!(g.players[0].graveyard.len(), 3);
}

#[test]
fn r603_6a_selected_permanents_enter_simultaneously_and_see_each_other() {
    let mut g = game();
    g.put(0, WATCHER, Zone::Library);
    g.put(0, BEAR, Zone::Library);
    cast(&mut g, 2);
    let among = selection(&g);
    choose(&mut g, among);
    assert_eq!(g.battlefield.len(), 2);
    cite!(["603.6a"]; assert_eq!(g.stack.len(), 1, "the entering watcher saw the other entering creature"));
    assert_eq!(g.players[0].life, 20);
    resolve(&mut g);
    cite!(["603.6a"]; assert_eq!(g.players[0].life, 21));
}

#[test]
fn r614_12_entry_choice_suspends_the_group_and_clone_resumption_keeps_the_selection() {
    let mut g = game();
    g.put(0, SPELL, Zone::Library);
    g.put(0, BEAR, Zone::Library);
    g.put(0, CHOOSER, Zone::Library);
    cast(&mut g, 3);
    let among = selection(&g);
    choose(&mut g, among);
    cite!(["614.12a"]; assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::Value(game::binding::Kind::CreatureType) }))));
    cite!(["614.12a"]; assert!(g.battlefield.is_empty(), "the group waits together for entry choices"));
    let clone = g.clone();
    for mut g in [g, clone] {
        g.act(0, Move::Choose(Answer::Value(game::binding::Value::CreatureType(ir::Subtype::from_name("Elf").unwrap())))).unwrap();
        assert!(g.decision.is_none() && g.stack.is_empty());
        assert_eq!(g.battlefield.len(), 2);
        assert_eq!(count(&g, Zone::Graveyard, SPELL), 1);
        cite!(["608.2c"]; assert_eq!(reveals(&g), 1));
    }
}

#[test]
fn r303_4g_selected_aura_that_cannot_enter_is_still_part_of_the_revealed_remainder() {
    let mut g = game();
    g.put(0, AURA, Zone::Library);
    cast(&mut g, 1);
    let among = selection(&g);
    choose(&mut g, among);
    cite!(["303.4g"]; assert!(g.battlefield.is_empty()));
    assert!(g.players[0].library.is_empty());
    cite!(["303.4g"]; assert_eq!(count(&g, Zone::Graveyard, AURA), 1, "selection alone is not a completed move"));
}

#[test]
fn r608_2c_movement_predicates_require_one_identifiable_completed_action() {
    for (text, reason) in [
        ("Reveal the top two cards of your library. Put all cards revealed this way that weren't put onto the battlefield into your graveyard.", "no earlier battlefield-entry producer"),
        ("Reveal the top two cards of your library. Put all creature cards revealed this way onto the battlefield. Put all land cards revealed this way onto the battlefield. Put all cards revealed this way that weren't put onto the battlefield into your graveyard.", "ambiguous battlefield-entry producers"),
    ] {
        let def = CardDef::new("Unbound", "{0}", "Sorcery", text, None).unwrap();
        let error = game::play::check(&def).unwrap_err();
        assert!(error.0.contains(reason), "{}", error.0);
    }
}

#[test]
fn r608_2d_arbitrary_subset_composes_with_other_destinations() {
    let mut g = game();
    let def = CardDef::new("Selection", "{0}", "Sorcery", "Reveal the top three cards of your library. Put any number of creature cards from among them into your hand. Put the rest into your graveyard.", None).unwrap();
    game::play::check(&def).unwrap();
    std::sync::Arc::make_mut(&mut g.defs).push(def);
    let bear = g.put(0, BEAR, Zone::Library);
    let giant=g.put(0, GIANT, Zone::Library);
    let instant=g.put(0, SPELL, Zone::Library);
    let card = g.put(0, g.defs.len() - 1, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    resolve(&mut g);
    cite!(["608.2d"]; assert_eq!(selection(&g).len(), 2));
    choose(&mut g, vec![bear]);
    // CR404.3: owner authors these simultaneous arrivals' top-first order.
    assert_eq!(g.decision,Some(game::state::Decision::Choose{player:0,choice:game::state::Choice::OrderCards{to:game::state::Placement::Zone(Zone::Graveyard),cards:vec![instant,giant]}}));
    g.act(0,Move::Choose(game::state::Answer::Objects(vec![giant,instant]))).unwrap();
    assert_eq!(g.players[0].hand.len(), 1);
    cite!(["608.2d"]; assert_eq!(g.obj(g.players[0].hand[0]).card, Some(BEAR)));
    cite!(["608.2c"]; assert_eq!(g.players[0].graveyard.len(), 3));
}

#[test]
fn r400_7_completed_movement_exclusion_recognizes_the_arrived_identity_and_an_empty_result() {
    for accept in [false, true] {
        let mut g = game();
        let def = CardDef::new("Return and destroy", "{0}", "Sorcery", "You may return target creature card from your graveyard to the battlefield. Destroy all creatures that weren't put onto the battlefield this way.", None).unwrap();
        game::play::check(&def).unwrap();
        std::sync::Arc::make_mut(&mut g.defs).push(def);
        let bear = g.put(0, BEAR, Zone::Graveyard);
        let giant = g.put(1, GIANT, Zone::Battlefield);
        let card = g.put(0, g.defs.len() - 1, Zone::Hand);
        g.act(0, Move::Cast { card, targets: vec![vec![game::Target::Object(bear)]] }).unwrap();
        resolve(&mut g);
        g.act(0, Move::Choose(if accept { Answer::Yes } else { Answer::No })).unwrap();
        cite!(["400.7"]; case: format!("accept={accept}"); assert_eq!(count(&g, Zone::Battlefield, BEAR), usize::from(accept)));
        assert_eq!(g.obj(giant).zone, Zone::Gone);
        assert!(g.stack.is_empty() && g.decision.is_none());
    }
}

#[test]
fn r608_2c_skipped_conditional_entry_exports_an_empty_completed_move() {
    for enabled in [false, true] {
        let mut g = game();
        let def = CardDef::new("Conditional return", "{0}", "Sorcery", "If you control an artifact, return target creature card from your graveyard to the battlefield. Destroy all creatures that weren't put onto the battlefield this way.", None).unwrap();
        game::play::check(&def).unwrap();
        std::sync::Arc::make_mut(&mut g.defs).push(def);
        let bear = g.put(0, BEAR, Zone::Graveyard);
        let giant = g.put(1, GIANT, Zone::Battlefield);
        if enabled { g.put(0, RELIC, Zone::Battlefield); }
        let card = g.put(0, g.defs.len() - 1, Zone::Hand);
        g.act(0, Move::Cast { card, targets: vec![vec![game::Target::Object(bear)]] }).unwrap();
        resolve(&mut g);
        cite!(["608.2c"]; case: format!("enabled={enabled}"); assert_eq!(count(&g, Zone::Battlefield, BEAR), usize::from(enabled)));
        cite!(["608.2c"]; case: format!("enabled={enabled}"); assert_eq!(g.obj(giant).zone, Zone::Gone));
        assert!(g.stack.is_empty() && g.decision.is_none());
    }
}
