//! Operation-local destruction/death facts: 701.8, 700.4, 400.7, 608.2h.
use crate::support::cite;
use game::{CardDef, Game, Move, Zone, Target};
const HELLFIRE: &str = "Destroy all nonblack creatures. Hellfire deals X plus 3 damage to you, where X is the number of creatures that died this way.";
const CINDER: &str = "Destroy target creature. If a white creature dies this way, Cinder Cloud deals damage to that creature's controller equal to the creature's power.";
fn game() -> Game {
    let definitions = vec![
        CardDef::new("Hellfire", "{2}{B}{B}{B}", "Sorcery", HELLFIRE, None).unwrap(),
        CardDef::new("Cinder Cloud", "{3}{R}{R}", "Instant", CINDER, None).unwrap(),
        CardDef::new("White", "{W}", "Creature — Human", "", Some((2,2))).unwrap(),
        CardDef::new("Black", "{B}", "Creature — Human", "", Some((2,2))).unwrap(),
        CardDef::new("Shielded", "{W}", "Creature — Human", "Indestructible", Some((2,2))).unwrap(),
        CardDef::new("Replaced", "{W}", "Creature — Human", "If this creature would die, put it on the bottom of its owner's library instead.", Some((2,2))).unwrap(),
        CardDef::new("Green", "{G}", "Creature — Human", "", Some((2,2))).unwrap(),
        CardDef::new("Changes", "{0}", "Enchantment — Aura", "Enchant creature\nEnchanted creature is white.\nEnchanted creature gets +3/+3.", None).unwrap(),
        CardDef::new("Steal", "{0}", "Instant", "Gain control of target creature.", None).unwrap(),
    ];
    for def in &definitions { game::play::check(def).unwrap_or_else(|e| panic!("{}: {}\n{:?}",def.name(),e.0,def.abilities())); }
    Game::new(definitions, 2, 20)
}
fn cast(g: &mut Game, definition: usize, target: Option<game::ObjId>) {
    let card = g.put(0,definition,Zone::Hand);
    g.players[0].pool.colorless=3;g.players[0].pool.colors[2]=3;g.players[0].pool.colors[3]=2;
    g.act(0,crate::support::paid(["CCBBB", "CCCRR", "W", "B", "W", "W", "G", "", ""][definition], Move::Cast { card, targets: target.map(|o|vec![vec![Target::Object(o)]]).unwrap_or_default() })).unwrap();
    g.act(g.priority,Move::Pass).unwrap();g.act(g.priority,Move::Pass).unwrap();
}
#[test]
fn full_hellfire_counts_only_actual_deaths_and_empty_result_still_deals_three() {
    for n in [0,1,3] {
        let mut g=game();let arrivals: Vec<_> = (0..n).map(|_| g.put(1,2,Zone::Battlefield)).collect();
        let black=g.put(1,3,Zone::Battlefield);let indestructible=g.put(1,4,Zone::Battlefield);
        let regenerated=g.put(1,2,Zone::Battlefield);g.regenerate(regenerated);
        g.put(1,5,Zone::Battlefield);cast(&mut g,0,None);
        if n > 1 {
            assert_eq!(g.decision, Some(game::state::Decision::Choose { player: 1, choice: game::state::Choice::OrderCards { to: game::state::Placement::Zone(Zone::Graveyard), cards: arrivals.clone() } }));
            g.act(1, Move::Choose(game::state::Answer::Objects(vec![arrivals[2], arrivals[0], arrivals[1]]))).unwrap();
        }
        cite!(["608.2c", "700.4"]; case: n; assert_eq!(g.players[0].life,20-(n+3)));cite!(["701.8a"]; case: n; assert_eq!(g.players[1].graveyard.len(),n as usize));
        assert!(g.battlefield.contains(&black));cite!(["702.12b"]; case: n; assert!(g.battlefield.contains(&indestructible)));cite!(["701.19a"]; case: n; assert!(g.battlefield.contains(&regenerated)));
        cite!(["614.6"]; case: n; assert_eq!(g.players[1].library.len(),1));
    }
}
#[test]
fn full_cinder_reads_former_effective_color_power_and_controller_not_owner_or_printing() {
    let mut g=game();let creature=g.put(1,6,Zone::Battlefield);let aura=g.put(0,7,Zone::Battlefield);
    assert!(g.attach(aura,Target::Object(creature)));cast(&mut g,8,Some(creature));assert_eq!(g.controller(creature),0);
    cast(&mut g,1,Some(creature));cite!(["608.2h"]; assert_eq!(g.players[0].life,15));assert_eq!(g.players[1].life,20);
    assert_eq!(g.players[1].graveyard.len(),1);
}
#[test]
fn cinder_does_not_count_indestructible_regenerated_replaced_or_unrelated_deaths() {
    for definition in [2,3,4,5] {
        let mut g=game();let unrelated=g.put(1,2,Zone::Battlefield);g.destroy(unrelated);
        let creature=g.put(1,definition,Zone::Battlefield);if definition==2 {g.regenerate(creature);}
        cast(&mut g,1,Some(creature));cite!(["608.2c", "700.4"]; case: definition; assert_eq!(g.players[1].life,20));
    }
}
#[test]
fn missing_and_ambiguous_death_producers_are_refused() {
    for text in ["Hellfire deals X plus 3 damage to you, where X is the number of creatures that died this way.",
        "Destroy all creatures. Destroy all creatures. Hellfire deals X plus 3 damage to you, where X is the number of creatures that died this way.",
        "If a white creature dies this way, Cinder Cloud deals damage to that creature's controller equal to the creature's power."] {
        // The named source must match the text so grammar normalizes its self-reference.
        // The old name Refused silently skipped every row at the parse stage.
        let name=if text.contains("Cinder Cloud") {"Cinder Cloud"} else {"Hellfire"};
        let def=CardDef::new(name,"{0}","Sorcery",text,None).unwrap();
        crate::support::refused(game::play::check(&def).map_err(|e|e.0),"destruction result: absent or ambiguous producer");
    }
}
#[test]
fn tokens_dying_count_and_multicolored_white_creature_qualifies() {
    for spell in [0,1] {
        let mut g=game();let printed=g.put(1,2,Zone::Battlefield);
        let mut chars=g.obj(printed).chars.clone();g.destroy(printed);
        chars.colors.push(ir::Color::Green);chars.pt=Some((4,4));
        let token=g.create_token(1,chars,false);cast(&mut g,spell,Some(token).filter(|_|spell==1));
        cite!(["608.2h", "700.4"]; case: spell; assert_eq!(g.players[spell].life,16));
        assert!(!g.battlefield.contains(&token));
        cite!(["704.5d"]; case: spell; assert_eq!(g.players[1].graveyard.len(),1)); // the earlier card, never the ceased token
    }
}
#[test]
fn suspended_library_replacements_clone_and_deliver_death_count_once() {
    use game::state::{Answer,Choice,Decision};
    let mut g=game();let a=g.put(1,5,Zone::Battlefield);let b=g.put(1,5,Zone::Battlefield);
    let dead=g.put(1,2,Zone::Battlefield);cast(&mut g,0,None);
    cite!(["401.4"]; assert!(matches!(g.decision,Some(Decision::Choose { player:1, choice:Choice::OrderCards{to:game::state::Placement::Library(ir::action::LibraryPosition::Bottom),..} }))));
    assert_eq!(g.players[0].life,20);assert!(g.battlefield.contains(&dead));
    let mut copy=g.clone();for game in [&mut g,&mut copy] {
        game.act(1,Move::Choose(Answer::Objects(vec![b,a]))).unwrap();
        cite!(["608.2c", "700.4"]; assert_eq!(game.players[0].life,16));assert_eq!(game.players[1].graveyard.len(),1);
        cite!(["614.6"]; assert_eq!(game.players[1].library.len(),2));
        assert_eq!(game.log.iter().filter(|e|matches!(e,game::Event::Destroyed{..})).count(),3);
        assert!(game.decision.is_none());
    }
}
#[test]
fn completed_result_survives_later_choice_clone_and_conservative_collection_boundary() {
    use game::state::{Answer,Decision};
    let def=CardDef::new("Paused","{0}","Instant","Destroy target creature. You may pay {1}. If a white creature dies this way, Paused deals damage to that creature's controller equal to the creature's power.",None).unwrap();
    game::play::check(&def).unwrap();
    let white=CardDef::new("Victim","{W}","Creature — Human","",Some((3,3))).unwrap();
    let mut g=Game::new(vec![def,white],2,20);let victim=g.put(1,1,Zone::Battlefield);g.players[0].pool.colorless=1;
    let spell=g.put(0,0,Zone::Hand);g.act(0,Move::Cast{card:spell,targets:vec![vec![Target::Object(victim)]]}).unwrap();
    for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
    cite!(["608.2d"]; assert!(matches!(g.decision,Some(Decision::Choose{player:0,..}))));
    cite!(["701.8a"]; assert!(!g.battlefield.contains(&victim)));assert_eq!(g.players[1].life,20);
    let mut copy=g.clone();for game in [&mut g,&mut copy] {
        game.retire_events(game.log.end());assert_eq!(game.collect_departed(),0, "a suspended procedure conservatively pins its arena");
        game.act(0,Move::Choose(Answer::No)).unwrap();
        cite!(["608.2h", "608.2c"]; assert_eq!(game.players[1].life,17));assert!(game.decision.is_none());
    }
}
