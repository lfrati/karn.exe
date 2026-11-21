//! A fixed random selection of cards the engine runs; native Oracle cards, scoped rule-derived expectations.
use crate::support::{cite, oracle, paid};
use game::binding::Value;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::{Color, Part};

fn setup(name: &str, players: usize) -> Game {
    let mut g = Game::new(
        vec![
            oracle(name),
            oracle("Island"),
            CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
            CardDef::new("Gift", "{0}", "Instant", "You gain 1 life.", None).unwrap(),
            CardDef::new("Red", "{R}", "Instant", "You gain 1 life.", None).unwrap(),
            CardDef::new("Counter", "{0}", "Instant", "Put a +1/+1 counter on target creature.", None).unwrap(),
            CardDef::new("Destroy", "{0}", "Instant", "Destroy target creature.", None).unwrap(),
        ],
        players,
        20,
    );
    for p in 0..players {
        for _ in 0..20 {
            g.put(p, 1, Zone::Library);
        }
    }
    g
}
fn resolve(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn cast(g: &mut Game, def: usize, targets: Vec<Vec<Target>>, spending: &str) {
    let card = g.put(0, def, Zone::Hand);
    g.act(0, paid(spending, Move::Cast { card, targets })).unwrap();
}
fn mana(g: &mut Game, generic: u32, color: Color, n: u32) {
    g.players[0].pool.colorless = generic;
    g.players[0].pool.colors[color as usize] = n;
}
fn ability(g: &mut Game, source: ObjId, index: usize, spending: &str) {
    g.act(0, paid(spending, Move::Activate { source, ability: index, targets: vec![], paying: vec![] })).unwrap();
}
fn until(g: &mut Game, turn: u32, part: Part) {
    for _ in 0..300 {
        if g.turn == turn && g.part == part {
            return;
        }
        let mv = match g.decision {
            Some(Decision::Attackers { .. }) => Move::Attack(vec![]),
            Some(Decision::Blockers { .. }) => Move::Block(vec![]),
            Some(ref d) => panic!("unexpected {d:?}"),
            None => Move::Pass,
        };
        let player = g.asked().unwrap().0;
        g.act(player, mv).unwrap();
    }
    panic!("boundary not reached")
}

// Rules: 601.2f, 601.2h, 701.6a, 733.1
// Creature spells reduce generic by two; noncreature spells still cost 2UU.
#[test]
fn r601_out_of_air_creature_discount_and_noncreature_payment() {
    for creature in [true, false] {
        let mut g = setup("Out of Air", 2);
        cast(&mut g, if creature { 2 } else { 3 }, vec![], "");
        let target = g.stack[0];
        let card = g.put(0, 0, Zone::Hand);
        mana(&mut g, 0, Color::Blue, 2);
        if !creature {
            cite!(["601.2f","601.2h"]; refused("mana"): g.act(0,paid("CCUU", Move::Cast{card,targets:vec![vec![Target::Object(target)]]})));
            cite!(["733.1"]; assert!(g.players[0].hand.contains(&card) && g.stack.len()==1));
            mana(&mut g, 2, Color::Blue, 2);
        }
        g.act(0, paid(if creature { "UU" } else { "CCUU" }, Move::Cast { card, targets: vec![vec![Target::Object(target)]] })).unwrap();
        resolve(&mut g);
        cite!(["701.6a"]; assert!(g.stack.is_empty()));
        cite!(["701.6a"]; assert_eq!(g.players[0].graveyard.len(),2));
    }
}

// Rules: 603.2, 700.2, 603.3d, 613.4c, 514.2
// Only own combat; native first mode rejects opponent target and expires in cleanup.
#[test]
fn r603_ferocification_power_mode_controller_and_duration() {
    let mut g = setup("Ferocification", 2);
    g.put(0, 0, Zone::Battlefield);
    let bear = g.put(0, 2, Zone::Battlefield);
    let enemy = g.put(1, 2, Zone::Battlefield);
    until(&mut g, 1, Part::BeginningOfCombat);
    cite!(["700.2"]; assert!(matches!(g.decision,Some(Decision::Modes{player:0,..}))));
    cite!(["603.3d"]; refused("target"): g.act(0,Move::Modes(vec![0],Box::new(Move::ChooseTargets(vec![vec![Target::Object(enemy)]])))));
    g.act(0, Move::Modes(vec![0], Box::new(Move::ChooseTargets(vec![vec![Target::Object(bear)]])))).unwrap();
    resolve(&mut g);
    cite!(["613.4c"]; assert_eq!((g.power(bear),g.toughness(bear)),(Some(4),Some(2))));
    until(&mut g, 2, Part::BeginningOfCombat);
    cite!(["514.2","603.2"]; assert_eq!((g.power(bear),g.stack.len()),(Some(2),0)));
}

// Rules: 700.2, 702.10b, 702.111b, 514.2
// Second mode grants both keywords only to its recipient for this turn.
#[test]
fn r702_ferocification_second_mode_keywords_expire() {
    let mut g = setup("Ferocification", 2);
    g.put(0, 0, Zone::Battlefield);
    let bear = g.put(0, 2, Zone::Battlefield);
    let other = g.put(0, 2, Zone::Battlefield);
    until(&mut g, 1, Part::BeginningOfCombat);
    g.act(0, Move::Modes(vec![1], Box::new(Move::ChooseTargets(vec![vec![Target::Object(bear)]])))).unwrap();
    resolve(&mut g);
    cite!(["702.10b","702.111b"]; assert!(g.has(bear,"Haste") && g.has(bear,"Menace") && !g.has(other,"Haste")));
    until(&mut g, 2, Part::PrecombatMain);
    cite!(["514.2"]; assert!(!g.has(bear,"Haste") && !g.has(bear,"Menace")));
}

// Rules: 602.2b, 701.23a, 701.23b, 701.26, 400.7
// Sacrifice before resolution; private basic-only choice can intentionally fail.
#[test]
fn r701_expanse_search_privacy_basic_filter_and_fail_to_find() {
    for fail in [false, true] {
        let mut g = setup("Terramorphic Expanse", 2);
        g.players[0].library.clear();
        let land = g.put(0, 1, Zone::Library);
        let bear = g.put(0, 2, Zone::Library);
        let source = g.put(0, 0, Zone::Battlefield);
        ability(&mut g, source, 0, "");
        cite!(["602.2b"]; assert!(!g.battlefield.contains(&source) && g.players[0].graveyard.len()==1));
        resolve(&mut g);
        let Some(Decision::Choose { player: 0, choice: Choice::Search { cards, among, min, max } }) =
            g.decision.clone()
        else {
            panic!("{:?}", g.decision)
        };
        cite!(["701.23a","701.23b"]; assert_eq!((cards,among,min,max),(vec![land,bear],vec![land],0,1)));
        cite!(["701.23a"]; assert!(g.sees(0,bear) && !g.sees(1,bear)));
        cite!(["701.23a"]; refused("matching"): g.act(0,Move::Choose(Answer::Objects(vec![bear]))));
        g.act(0, Move::Choose(Answer::Objects(if fail { vec![] } else { vec![land] }))).unwrap();
        cite!(["701.23b"]; assert_eq!(g.battlefield.len(),usize::from(!fail)));
        if !fail {
            let arrived = g.battlefield[0];
            cite!(["400.7","701.26a"]; assert!(arrived!=land && g.obj(arrived).tapped));
        }
        cite!(["701.24a"]; assert!(g.log.iter().any(|e|matches!(e,Event::Shuffled{player:0,..}))));
        cite!(["401.2"]; assert!(!g.sees(0,bear) && !g.sees(1,bear)));
    }
}

// Rules: 603.4, 603.2, 120.3a
// No counter means no trigger; a placement after end step begins is too late.
#[test]
fn r603_tarfire_no_counter_and_too_late_placement() {
    let mut g = setup("Lasting Tarfire", 3);
    g.put(0, 0, Zone::Battlefield);
    let bear = g.put(1, 2, Zone::Battlefield);
    until(&mut g, 1, Part::End);
    cite!(["603.4"]; assert!(g.stack.is_empty()));
    cast(&mut g, 5, vec![vec![Target::Object(bear)]], "");
    resolve(&mut g);
    cite!(["603.2","603.4"]; assert!(g.stack.is_empty())); // Too late to trigger on turn 1.
    // The positive counter-history case is r603_tarfire_counter_history_each_end_step_and_reset
    // in counter_placement_provenance.
}

// Rules: 614.12, 603.2, 109.5, 119.3
// Real entry chooses red; red own cast triggers before resolution, colorless does not.
#[test]
fn r614_mare_entry_choice_and_own_matching_cast() {
    let mut g = setup("Diamond Mare", 2);
    mana(&mut g, 2, Color::Red, 0);
    cast(&mut g, 0, vec![], "CC");
    resolve(&mut g);
    cite!(["614.12"]; assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::Value(_)}))));
    g.act(0, Move::Choose(Answer::Value(Value::Color(Color::Red)))).unwrap();
    mana(&mut g, 0, Color::Red, 1);
    cast(&mut g, 4, vec![], "R");
    cite!(["603.2"]; assert_eq!(g.stack.len(),2));
    resolve(&mut g);
    cite!(["119.3"]; assert_eq!(g.players[0].life,21));
    resolve(&mut g);
    cast(&mut g, 3, vec![], "");
    cite!(["603.2","109.5"]; assert_eq!(g.stack.len(),1));
    resolve(&mut g);
    cite!(["119.3"]; assert_eq!(g.players[0].life,23));
}

// Rules: 303.4a, 603.6a, 502.3, 603.6c, 118.12, 701.22a, 121.2
// Native Aura attaches, taps at trigger resolution, skips untap, then optional death draw.
#[test]
fn r303_chill_entry_skip_untap_and_declined_death_payment() {
    let mut g = setup("Bitter Chill", 2);
    let bear = g.put(1, 2, Zone::Battlefield);
    mana(&mut g, 1, Color::Blue, 1);
    cast(&mut g, 0, vec![vec![Target::Object(bear)]], "CU");
    resolve(&mut g);
    cite!(["603.6a"]; assert!(!g.obj(bear).tapped && g.stack.len()==1));
    resolve(&mut g);
    cite!(["701.26a"]; assert!(g.obj(bear).tapped));
    until(&mut g, 2, Part::PrecombatMain);
    cite!(["502.3"]; assert!(g.obj(bear).tapped));
    g.act(1, Move::Pass).unwrap();
    mana(&mut g, 1, Color::Blue, 0);
    cast(&mut g, 6, vec![vec![Target::Object(bear)]], "");
    resolve(&mut g);
    resolve(&mut g);
    cite!(["118.12"]; assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::Pay{..}}))));
    g.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["118.12"]; assert!(g.players[0].hand.is_empty()));
}

// Rules: 608.2b, 303.4a, 603.6c
// Illegal Aura target goes stack-to-graveyard, never battlefield-to-graveyard trigger.
#[test]
fn r608_chill_illegal_target_does_not_trigger_death_draw() {
    let mut g = setup("Bitter Chill", 2);
    let bear = g.put(1, 2, Zone::Battlefield);
    mana(&mut g, 1, Color::Blue, 1);
    cast(&mut g, 0, vec![vec![Target::Object(bear)]], "CU");
    cast(&mut g, 6, vec![vec![Target::Object(bear)]], "");
    resolve(&mut g);
    resolve(&mut g);
    cite!(["608.2b","603.6c"]; assert!(g.stack.is_empty() && g.decision.is_none()));
    cite!(["303.4a"]; assert!(g.battlefield.is_empty()));
}

// Rules: 603.4, 111.3, 121.2
// Untapped does not trigger; first tapped turn makes Glimmer; next tapped turn draws.
#[test]
fn r603_seeker_tapped_gate_token_then_draw() {
    let mut g = setup("Glimmer Seeker", 2);
    let seeker = g.put(0, 0, Zone::Battlefield);
    until(&mut g, 1, Part::PostcombatMain);
    cite!(["603.4"]; assert!(g.stack.is_empty()));
    until(&mut g, 3, Part::BeginningOfCombat);
    g.tap(seeker);
    until(&mut g, 3, Part::PostcombatMain);
    resolve(&mut g);
    let token = *g.battlefield.last().unwrap();
    let seen = g.seen(token, None);
    cite!(["111.3"]; assert!(g.obj(token).token() && seen.pt==Some((1,1)) && seen.colors==vec![Color::White] && seen.types.contains(&ir::CardType::Enchantment) && seen.types.contains(&ir::CardType::Creature) && seen.subtypes.iter().any(|s|s.name()=="Glimmer")));
    until(&mut g, 5, Part::BeginningOfCombat);
    g.tap(seeker);
    until(&mut g, 5, Part::PostcombatMain);
    let hand = g.players[0].hand.len();
    resolve(&mut g);
    cite!(["121.2"]; assert_eq!(g.players[0].hand.len(),hand+1));
    cite!(["111.3"]; assert_eq!(g.battlefield.len(),2));
}

// Rules: 603.4
// Tapped condition is checked again, not frozen at trigger time.
#[test]
fn r603_seeker_untapped_before_resolution_does_nothing() {
    let mut g = setup("Glimmer Seeker", 2);
    let seeker = g.put(0, 0, Zone::Battlefield);
    g.tap(seeker);
    until(&mut g, 1, Part::PostcombatMain);
    cite!(["603.4"]; assert_eq!(g.stack.len(),1));
    g.untap(seeker);
    resolve(&mut g);
    cite!(["603.4"]; assert_eq!(g.battlefield.len(),1));
    cite!(["603.4"]; assert!(g.players[0].hand.is_empty()));
}

// Rules: 605.3b, 602.2b, 701.57a, 400.7
// Sacrifice mana is immediate; graveyard ability exiles as cost and offers public discover hit.
#[test]
fn r701_treasure_mana_then_graveyard_discover_decline() {
    let mut g = setup("Buried Treasure", 2);
    g.players[0].library.clear();
    let hit = g.put(0, 2, Zone::Library);
    g.put(0, 1, Zone::Library);
    let source = g.put(0, 0, Zone::Battlefield);
    g.act(
        0,
        Move::Adding(
            vec![vec![ir::mana::ManaSymbol::Color(Color::Blue)]],
            Box::new(Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }),
        ),
    )
    .unwrap();
    if matches!(g.decision, Some(Decision::Choose { choice: Choice::Value(_), .. })) {
        g.act(0, Move::Choose(Answer::Value(Value::Color(Color::Blue)))).unwrap();
    }
    cite!(["605.3b"]; assert!(g.stack.is_empty() && g.players[0].pool.colors[Color::Blue as usize]==1));
    let grave = g.players[0].graveyard[0];
    mana(&mut g, 5, Color::Blue, 0);
    ability(&mut g, grave, 1, "CCCCC");
    cite!(["602.2b","400.7"]; assert!(g.players[0].graveyard.is_empty() && g.exile.len()==1));
    resolve(&mut g);
    let Some(Decision::Choose { player: 0, choice: Choice::Cast { cards } }) = g.decision.clone() else {
        panic!("{:?}", g.decision)
    };
    cite!(["701.57a"]; assert_eq!(cards.len(),1));
    let offered = cards[0];
    cite!(["701.57a","400.7"]; assert!(offered!=hit && g.sees(0,offered) && g.sees(1,offered)));
    g.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["701.57a"]; assert_eq!((g.players[0].hand.len(),g.players[0].library.len(),g.exile.len()),(1,1,1)));
}

// Rules: 601.2f, 601.2h, 202.3, 109.5, 611.3b
// Greatest own power discounts generic only; no creatures means full price; enemy excluded.
#[test]
fn r601_ghalta_discount_floor_scope_and_trample_departure() {
    for power in [0, 2, 10] {
        let mut g = setup("Ghalta the Unstoppable", 2);
        let enemy = g.put(1, 2, Zone::Battlefield);
        let mut mine = None;
        if power > 0 {
            let bear = g.put(0, 2, Zone::Battlefield);
            if power > 2 {
                g.set_initial_counters(bear, "+1/+1", (power - 2) as u32);
            }
            mine = Some(bear);
        }
        mana(&mut g, 8u32.saturating_sub(power as u32), Color::Green, 1);
        cast(&mut g, 0, vec![], &format!("{}G", "C".repeat(8u32.saturating_sub(power as u32) as usize)));
        resolve(&mut g);
        let source = *g.battlefield.last().unwrap();
        cite!(["202.3"]; assert_eq!(g.obj(source).chars.mana_value,9));
        cite!(["109.5","702.19a"]; assert!(g.has(source,"Trample") && !g.has(enemy,"Trample")));
        if let Some(bear) = mine {
            cite!(["611.3b"]; assert!(g.has(bear,"Trample")));
            cast(&mut g, 6, vec![vec![Target::Object(source)]], "");
            resolve(&mut g);
            cite!(["611.3b"]; assert!(!g.has(bear,"Trample")));
        }
    }
}

// Rules: 702.174a, 702.174e, 702.174j, 702.174m, 701.6a
// Without gift only creature spell; with gift noncreature and one opponent draw.
#[test]
fn r702_pull_gift_changes_target_scope_and_draws_once() {
    for (gift, creature) in [(false, true), (true, true), (true, false)] {
        let mut g = setup("Long River's Pull", 2);
        cast(&mut g, if creature { 2 } else { 3 }, vec![], "");
        let target = g.stack[0];
        let card = g.put(0, 0, Zone::Hand);
        mana(&mut g, 0, Color::Blue, 2);
        let mv = Move::Cast {
            card,
            targets: if gift {
                vec![vec![], vec![Target::Object(target)]]
            } else {
                vec![vec![Target::Object(target)], vec![]]
            },
        };
        g.act(0, paid("UU", if gift { Move::GiftTo(1, Box::new(mv)) } else { mv })).unwrap();
        cite!(["702.174a"]; assert!(g.players[1].hand.is_empty()));
        resolve(&mut g);
        cite!(["702.174e"]; assert_eq!(g.players[1].hand.len(),usize::from(gift)));
        cite!(["701.6a"]; assert!(g.stack.is_empty()));
    }
}

// Rules: 701.23b, 609.3, 701.24a, 602.2b
// An empty library is still searched and shuffled; tapped source cannot pay tap twice.
#[test]
fn r701_expanse_empty_library_and_tapped_cost_refusal() {
    let mut g = setup("Terramorphic Expanse", 2);
    g.players[0].library.clear();
    let source = g.put(0, 0, Zone::Battlefield);
    g.tap(source);
    cite!(["602.2b"]; refused("can't pay {T}"): g.act(0,Move::Activate{source,ability:0,targets:vec![],paying:vec![]}));
    cite!(["733.1"]; assert!(g.battlefield.contains(&source) && g.stack.is_empty()));
    g.untap(source);
    ability(&mut g, source, 0, "");
    resolve(&mut g);
    if let Some(Decision::Choose { player: 0, choice: Choice::Search { cards, among, min, max } }) = g.decision.clone()
    {
        cite!(["701.23b","609.3"]; assert_eq!((cards,among,min,max),(vec![],vec![],0,0)));
        g.act(0, Move::Choose(Answer::Objects(vec![]))).unwrap();
    }
    cite!(["609.3"]; assert!(g.battlefield.is_empty() && !g.players[0].lost));
    cite!(["701.24a"]; assert!(g.log.iter().any(|e|matches!(e,Event::Shuffled{player:0,..}))));
}

// Rules: 702.174m, 601.2c, 608.2b, 702.174j
// Noncreature target needs gift; target departure makes entire spell fail with no gift draw.
#[test]
fn r702_pull_nongift_target_denial_and_departed_target_no_gift() {
    let mut g = setup("Long River's Pull", 2);
    cast(&mut g, 3, vec![], "");
    let target = g.stack[0];
    let card = g.put(0, 0, Zone::Hand);
    mana(&mut g, 0, Color::Blue, 2);
    cite!(["601.2c","702.174m"]; refused("target"): g.act(0,Move::Cast{card,targets:vec![vec![Target::Object(target)],vec![]]}));
    g.act(0, paid("UU", Move::GiftTo(1, Box::new(Move::Cast { card, targets: vec![vec![], vec![Target::Object(target)]] }))))
        .unwrap();
    // Direct counter of the target is the explicit position just before Pull resolves;
    // it models an intervening counterspell, not casting that counterspell here.
    g.counter(target);
    resolve(&mut g);
    cite!(["608.2b","702.174j"]; assert!(g.players[1].hand.is_empty() && g.stack.is_empty()));
}

// Rules: 601.2f, 601.2h, 733.1
// Even ten power cannot remove G; opponent power cannot reduce generic.
#[test]
fn r601_ghalta_colored_floor_and_enemy_power_excluded() {
    for own in [true, false] {
        let mut g = setup("Ghalta the Unstoppable", 2);
        let bear = g.put(if own { 0 } else { 1 }, 2, Zone::Battlefield);
        g.set_initial_counters(bear, "+1/+1", 8);
        let card = g.put(0, 0, Zone::Hand);
        mana(&mut g, 0, Color::Green, if own { 0 } else { 1 });
        cite!(["601.2f","601.2h"]; refused("mana"): g.act(0,paid(if own { "G" } else { "CCCCCCCCG" }, Move::Cast{card,targets:vec![]})));
        cite!(["733.1"]; assert!(g.players[0].hand.contains(&card) && g.stack.is_empty()));
    }
}
