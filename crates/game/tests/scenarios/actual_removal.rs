//! Effect removal records actual quantities; costs retain full affordability (122/609.3).
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, Target, Zone};
use ir::{Color, Part};
use crate::support::{cite, paid};
const RELIC: &str = "{T}: Add one mana of any color.\n{T}: Put a charge counter on this artifact.\nAt the beginning of your first main phase, remove all charge counters from this artifact. Add one mana of any color for each charge counter removed this way.";
fn setup(text: &str) -> Game {
    let defs=vec![CardDef::new("Probe","{0}","Sorcery",text,None).unwrap(),
 CardDef::new("Coalition Relic","{3}","Artifact",RELIC,None).unwrap(),
 CardDef::new("Shrewd Hatchling","{3}{U/R}","Creature — Elemental","This creature enters with four -1/-1 counters on it.\n{U/R}: Target creature can't block this creature this turn.\nWhenever you cast a blue spell, remove a -1/-1 counter from this creature.\nWhenever you cast a red spell, remove a -1/-1 counter from this creature.",Some((6,6))).unwrap(),
 CardDef::new("Chainbreaker","{2}","Artifact Creature — Scarecrow","This creature enters with two -1/-1 counters on it.\n{3}, {T}: Remove a -1/-1 counter from target creature.",Some((3,3))).unwrap(),
 CardDef::new("Bear","{0}","Creature — Bear","",Some((2,2))).unwrap(),
 CardDef::new("Library","","Basic Land — Island","",None).unwrap(),
 CardDef::new("Both","{U}{R}","Instant","You gain 1 life.",None).unwrap(),
 CardDef::new("Haste","{0}","Enchantment","Creatures you control have haste.",None).unwrap()];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0));
    }
    Game::new(defs, 2, 20)
}
fn finish(g: &mut Game) {
    for _ in 0..30 {
        assert!(g.decision.is_none(), "{:?}", g.decision);
        if g.stack.is_empty() {
            assert!(g.resolving().is_none());
            return;
        }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("not finished")
}
fn cast(g: &mut Game, targets: Vec<Vec<Target>>) {
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets }).unwrap();
    finish(g);
}
fn begin_relic(g: &mut Game, n: u32) -> game::ObjId {
    let relic = g.put(0, 1, Zone::Battlefield);
    g.put_counters(relic, "charge", n);
    for p in 0..2 {
        for _ in 0..5 {
            g.put(p, 5, Zone::Library);
        }
    }
    g.part = Part::Draw;
    for _ in 0..10 {
        g.act(g.priority, Move::Pass).unwrap();
        if g.part == Part::PrecombatMain && !g.stack.is_empty() {
            return relic;
        }
    }
    panic!("trigger not queued")
}
#[test]
fn full_coalition_relic_zero_small_and_large_choices() {
    for n in [0, 1, 3, 4, 6] {
        let mut g = setup("You gain 1 life.");
        let relic = begin_relic(&mut g, n);
        for _ in 0..2 {
            g.act(g.priority, Move::Pass).unwrap();
        }
        if n > 0 {
            assert_eq!(g.counters(relic).get("charge"), None);
            if n <= 3 {
                let Some(Decision::Choose { choice: Choice::Mana { options }, .. }) = &g.decision else {
                    panic!("no combined choice")
                };
                cite!(["608.2d"]; case: format!("{n}"); assert!(options.iter().all(|v| v.len() == n as usize)));
                let chosen = vec![ir::mana::ManaSymbol::Color(Color::Green);n as usize];
                assert!(options.contains(&chosen));
                g.act(0, Move::Choose(Answer::Mana(chosen))).unwrap();
            } else {
                for i in 0..n {
                    let Some(Decision::Choose { choice: Choice::Mana { options }, .. }) = &g.decision else {
                        panic!("no unit choice")
                    };
                    assert_eq!(options.len(), 5);
                    assert!(options.iter().all(|v| v.len() == 1));
                    g.act(
                        0,
                        Move::Choose(Answer::Mana(vec![ir::mana::ManaSymbol::Color(if i % 2 == 0 {
                            Color::Red
                        } else {
                            Color::Green
                        })])),
                    )
                    .unwrap();
                }
            }
        }
        finish(&mut g);
        cite!(["106.4"]; case: format!("{n}"); assert_eq!(g.players[0].pool.colors.iter().sum::<u32>(), n));
        assert_eq!(g.log.iter().filter(|e|matches!(e,Event::CountersRemoved{from,kind,count} if *from==relic.into() && kind=="charge" && *count==n)).count(),usize::from(n>0));
        assert_eq!(g.log.iter().filter(|e| matches!(e, Event::ManaAdded { .. })).count(), usize::from(n > 0));
    }
}
#[test]
fn effect_removal_records_partial_all_and_zero_without_event_history() {
    for (have, expected) in [(0, 0), (1, 1), (4, 2)] {
        let mut g = setup(
            "Remove two charge counters from target artifact. You gain 1 life for each charge counter removed this way.",
        );
        let relic = g.put(0, 1, Zone::Battlefield);
        g.put_counters(relic, "charge", have);
        cast(&mut g, vec![vec![relic.into()]]);
        cite!(["609.3"]; case: format!("{have}"); assert_eq!(g.players[0].life, 20 + expected));
        cite!(["609.3"]; case: format!("{have}"); assert_eq!(g.counters(relic).get("charge").copied().unwrap_or(0), have - expected as u32));
    }
    let mut g =
        setup("Remove all poison counters from target player. You gain 1 life for each counter removed this way.");
    g.put_counters(Target::Player(1), "poison", 4);
    cast(&mut g, vec![vec![Target::Player(1)]]);
    assert_eq!(g.players[0].life, 24);
    cite!(["122.1"]; assert!(g.counters(Target::Player(1)).is_empty()));
}
#[test]
fn removal_requires_unique_effect_producer_and_supported_holder_kind() {
    for text in [
        "You gain 1 life for each counter removed this way.",
        "Remove a charge counter from target artifact. Remove a charge counter from target artifact. You gain 1 life for each counter removed this way.",
        "Remove a rad counter from target player.",
        "Remove any number of charge counters from target artifact.",
    ] {
        let d = CardDef::new("No", "{0}", "Sorcery", text, None).unwrap();
        let reason=if text.starts_with("Remove a rad") {"player counter: rad upkeep trigger"}else if text.starts_with("Remove any") {"counter removal: fixed count or all required"}else{"counter removal result: absent or ambiguous producer"};
        crate::support::refused(game::play::check(&d).map_err(|e|e.0),reason);
    }
}

#[test]
fn full_hatchling_and_chainbreaker_remove_available_counters() {
    let mut g = setup("You gain 1 life.");
    let card = g.put(0, 2, Zone::Hand);
    g.players[0].pool.colors = [10; 5];
    g.act(0, paid("UUUU", Move::Cast { card, targets: vec![] })).unwrap();
    finish(&mut g);
    let hatchling = *g.battlefield.iter().find(|o| g.obj(**o).chars.name == "Shrewd Hatchling").unwrap();
    cite!(["614.1c"]; assert_eq!(g.counters(hatchling).get("-1/-1"), Some(&4)));
    let both = g.put(0, 6, Zone::Hand);
    g.players[0].pool.colors = [10; 5];
    g.act(0, paid("UR", Move::Cast { card: both, targets: vec![] })).unwrap();
    let Some(Decision::Order{player:0,ref abilities})=g.decision else {panic!("two Hatchling color triggers")};
    assert_eq!(abilities.iter().map(|t|(t.source,t.controller,t.origin.clone(),t.index)).collect::<Vec<_>>(),vec![(Some(hatchling),0,game::binding::Origin::Own,2),(Some(hatchling),0,game::binding::Origin::Own,3)]);
    g.act(0,Move::Order(vec![0,1])).unwrap();
    finish(&mut g);
    cite!(["603.2"]; assert_eq!(g.counters(hatchling).get("-1/-1"), Some(&2)));
    let card = g.put(0, 3, Zone::Hand);
    g.players[0].pool.colors = [10; 5];
    g.act(0, paid("GG", Move::Cast { card, targets: vec![] })).unwrap();
    finish(&mut g);
    g.put(0, 7, Zone::Battlefield);
    let breaker = *g.battlefield.iter().find(|o| g.obj(**o).chars.name == "Chainbreaker").unwrap();
    assert_eq!(g.counters(breaker).get("-1/-1"), Some(&2));
    for _ in 0..3 {
        g.untap(breaker);
        g.players[0].pool.colors = [10; 5];
        let ability = 1; // Chainbreaker: entry replacement, then its activated ability.
        g.act(0, paid("GGG", Move::Activate { source: breaker, ability, targets: vec![vec![hatchling.into()]], paying: vec![] }))
            .unwrap();
        finish(&mut g);
    }
    assert!(g.counters(hatchling).is_empty());
    cite!(["609.3"]; assert_eq!(
        g.log.iter().filter(|e| matches!(e,Event::CountersRemoved{from,..} if *from==hatchling.into())).count(),
        4
    ));
}
#[test]
fn coalition_response_noops_and_mid_choice_clone_retains_one_result() {
    for departed in [false, true] {
        let mut g = setup("You gain 1 life.");
        let relic = begin_relic(&mut g, 4);
        if departed {
            g.destroy(relic);
        } else {
            g.remove_counters(relic, "charge", 4);
        }
        finish(&mut g);
        assert_eq!(g.players[0].pool.colors.iter().sum::<u32>(), 0);
        assert!(g.decision.is_none());
    }
    let mut g = setup("You gain 1 life.");
    let relic = begin_relic(&mut g, 4);
    for _ in 0..2 {
        g.act(g.priority, Move::Pass).unwrap();
    }
    g.act(0, Move::Choose(Answer::Mana(vec![ir::mana::ManaSymbol::Color(Color::Red)]))).unwrap();
    assert_eq!(g.players[0].pool.colors.iter().sum::<u32>(), 0);
    assert_eq!(g.log.iter().filter(|e| matches!(e, Event::CountersRemoved { .. })).count(), 1);
    g.destroy(relic);
    g.retire_events(g.log.end());
    assert_eq!(g.collect_departed(), 0);
    let mut copy = g.clone();
    for (game, color) in [(&mut g, Color::Green), (&mut copy, Color::Blue)] {
        for _ in 0..3 {
            game.act(0, Move::Choose(Answer::Mana(vec![ir::mana::ManaSymbol::Color(color)]))).unwrap();
        }
        finish(game);
        assert_eq!(game.players[0].pool.colors[Color::Red as usize], 1);
        cite!(["608.2d"]; case: format!("{color:?}"); assert_eq!(game.players[0].pool.colors[color as usize], 3));
        assert_eq!(game.log.iter().filter(|e| matches!(e, Event::CountersRemoved { .. })).count(), 0);
        assert_eq!(game.log.iter().filter(|e| matches!(e,Event::ManaAdded{mana,..} if mana.len()==4)).count(), 1);
    }
}
#[test]
fn optional_empty_and_multiple_holder_quantities_use_actual_records() {
    let mut g = setup(
        "You may remove a charge counter from target artifact. You gain 1 life for each charge counter removed this way.",
    );
    let relic = g.put(0, 1, Zone::Battlefield);
    g.put_counters(relic, "charge", 2);
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![vec![relic.into()]] }).unwrap();
    for _ in 0..2 {
        g.act(g.priority, Move::Pass).unwrap();
    }
    g.act(0, Move::Choose(Answer::No)).unwrap();
    finish(&mut g);
    assert_eq!(g.players[0].life, 20);
    cite!(["608.2d"]; assert_eq!(g.counters(relic).get("charge"), Some(&2)));
    let mut g = setup(
        "Remove all charge counters from each artifact. You gain 1 life for each charge counter removed this way.",
    );
    let a = g.put(0, 1, Zone::Battlefield);
    let b = g.put(1, 1, Zone::Battlefield);
    g.put_counters(a, "charge", 2);
    g.put_counters(b, "charge", 3);
    cast(&mut g, vec![]);
    assert_eq!(g.players[0].life, 25);
}

#[test]
fn retained_removal_holders_survive_collection_and_kind_readers_filter_actual_results() {
    let mut g = setup("Remove all charge counters from each artifact. Add one mana of any color for each charge counter removed this way.");
    let relic = g.put(1, 1, Zone::Battlefield);
    g.put_counters(relic, "charge", 4);
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    g.destroy(relic);
    g.retire_events(g.log.end());
    assert_eq!(g.collect_departed(), 0, "the operation value retains this untargeted holder");
    for _ in 0..4 {
        g.act(0, Move::Choose(Answer::Mana(vec![ir::mana::ManaSymbol::Color(Color::Green)]))).unwrap();
    }
    finish(&mut g);
    assert_eq!(g.players[0].pool.colors[Color::Green as usize], 4);
    assert_eq!(g.log.iter().filter(|e| matches!(e, Event::ManaAdded { .. })).count(), 1);
    let mut g = setup("Remove all energy counters from target player. You gain 1 life for each poison counter removed this way.");
    g.put_counters(Target::Player(1), "energy", 3);
    cast(&mut g, vec![vec![Target::Player(1)]]);
    assert_eq!(g.players[0].life, 20);
    assert!(g.counters(Target::Player(1)).is_empty());
}


#[test]
fn optional_removal_cannot_pay_with_fewer_counters_than_requested() {
    for have in [0,1,2] {
        let mut g=setup("You may remove two charge counters from target artifact. If you do, you gain 3 life.");
        let relic=g.put(0,1,Zone::Battlefield);g.put_counters(relic,"charge",have);
        let card=g.put(0,0,Zone::Hand);g.act(0,Move::Cast{card,targets:vec![vec![relic.into()]]}).unwrap();
        for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
        if have==2 {assert!(matches!(g.decision,Some(Decision::Choose{choice:Choice::May { possible: true },..})));g.act(0,Move::Choose(Answer::Yes)).unwrap();}
        else {cite!(["118.12", "118.3"]; case: format!("{have}"); assert!(g.decision.is_none(),"no impossible removal choice"));}
        finish(&mut g);cite!(["118.12"]; case: format!("{have}"); assert_eq!(g.players[0].life,if have==2{23}else{20}));
        assert_eq!(g.counters(relic).get("charge").copied().unwrap_or(0),if have==2{0}else{have});
    }
}
