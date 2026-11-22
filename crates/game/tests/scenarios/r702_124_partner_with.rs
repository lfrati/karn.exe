//! The entry ability of partner with (702.124j), independently of commander designation.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, Target, Zone};
use game::state::{Answer, Choice, Decision};

pub(super) const ALISAIE: &str="Partner with Alphinaud Leveilleur (When this creature enters, target player may put Alphinaud Leveilleur into their hand from their library, then shuffle.)\nFirst strike\nDualcast — The second spell you cast each turn costs {2} less to cast.";
pub(super) const ALPHINAUD: &str="Partner with Alisaie Leveilleur (When this creature enters, target player may put Alisaie Leveilleur into their hand from their library, then shuffle.)\nVigilance\nEukrasia — Whenever you cast your second spell each turn, draw a card.";
fn fixture() -> Game {
    let defs=vec![
        CardDef::new("Alisaie Leveilleur","{2}{W}","Legendary Creature — Elf Wizard",ALISAIE,Some((3,2))).unwrap(),
        CardDef::new("Alphinaud Leveilleur","{3}{U}","Legendary Creature — Elf Wizard",ALPHINAUD,Some((2,4))).unwrap(),
        CardDef::new("Island","","Basic Land — Island","",None).unwrap(),
    ];
    for d in &defs {game::play::check(d).unwrap_or_else(|e|panic!("{}: {}",d.name(),e.0));} Game::new(defs,3,20)
}
fn pass(g: &mut Game) {for _ in 0..3 {g.act(g.priority,Move::Pass).unwrap();}}
fn enter(g: &mut Game, index: usize, target: usize) {
    g.part=ir::Part::PrecombatMain;
    let card=g.put(0,index,Zone::Hand);
    g.players[0].pool.colorless=3;
    g.players[0].pool.colors[if index==0 {ir::Color::White} else {ir::Color::Blue} as usize]=1;
    g.act(0,crate::support::paid(if index == 0 { "CCW" } else { "CCCU" },Move::Cast{card,targets:vec![]})).unwrap();pass(g);
    assert!(matches!(g.decision,Some(Decision::Targets{player:0,groups:1,..})));
    crate::support::refused(g.act(1,Move::ChooseTargets(vec![vec![Target::Player(target)]])), "that decision is another player's");
    let name=["Alisaie Leveilleur","Alphinaud Leveilleur"][index];
    cite!(["115.1"]; refused(&format!("an illegal target, {name}: it isn't a player (115.1)")): g.act(0,Move::ChooseTargets(vec![vec![Target::Object(g.battlefield[0])]])));
    g.act(0,Move::ChooseTargets(vec![vec![Target::Player(target)]])).unwrap();pass(g);
    assert_eq!(g.decision,Some(Decision::Choose{player:target,choice:Choice::May { possible: true }}));
}

#[test]
fn r702_124j_both_full_leveilleurs_target_another_player_for_private_named_search() {
    for index in 0..2 {
        let mut g=fixture();let wanted=g.put(1,1-index,Zone::Library);let other=g.put(1,2,Zone::Library);
        let own=g.put(0,1-index,Zone::Library);
        enter(&mut g,index,1);
        cite!(["702.124j"]; refused("that decision is another player's"): g.act(0,Move::Choose(Answer::Yes)));
        g.act(1,Move::Choose(Answer::Yes)).unwrap();
        cite!(["702.124j"]; assert_eq!(g.decision,Some(Decision::Choose{player:1,choice:Choice::Search{cards:vec![wanted,other],among:vec![wanted],min:0,max:1}})));
        for card in [wanted,other] {assert!(g.sees(1,card));assert!(!g.sees(0,card));assert!(!g.sees(2,card));}
        cite!(["701.23a"]; refused("a chosen card is not one the search can find, in the searched zone and matching its description (701.23a): choose between 0 and 1 distinct matching objects"): g.act(1,Move::Choose(Answer::Objects(vec![other]))));
        let clone=g.clone();
        for mut branch in [g,clone] {
            branch.act(1,Move::Choose(Answer::Objects(vec![wanted]))).unwrap();
            let hand=branch.players[1].hand[0];cite!(["702.124j"]; assert_eq!(branch.obj(hand).card,Some(1-index)));
            assert_eq!(branch.players[0].library,vec![own]);assert_eq!(branch.players[1].library,vec![other]);
            assert!(!branch.sees(1,other));assert!(!branch.sees(0,hand));
            assert_eq!(branch.log.iter().filter(|e|matches!(e,Event::Revealed{player:1,cards} if cards==&vec![wanted])).count(),1);
            cite!(["702.124j"]; assert_eq!(branch.log.iter().filter(|e|matches!(e,Event::Shuffled{player:1, ..})).count(),1));
            assert!(branch.stack.is_empty()&&branch.decision.is_none());
            assert!(branch.has(branch.battlefield[0],if index==0 {"First strike"} else {"Vigilance"}));
        }
    }
}

#[test]
fn r702_124j_declining_does_not_shuffle_but_failing_the_qualified_search_does() {
    for (accept,present) in [(false,true),(true,true),(true,false)] {
        let mut g=fixture();g.put(0,if present {1} else {2},Zone::Library);
        enter(&mut g,0,0);
        g.act(0,Move::Choose(if accept {Answer::Yes} else {Answer::No})).unwrap();
        if accept {g.act(0,Move::Choose(Answer::Objects(vec![]))).unwrap();}
        cite!(["701.23b"]; assert!(g.players[0].hand.is_empty()));assert_eq!(g.players[0].library.len(),1);
        cite!(["702.124j"]; assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Shuffled{player:0, ..})).count(),usize::from(accept)));
        assert!(!g.log.iter().any(|e|matches!(e,Event::Revealed{..})));
    }
}

#[test]
fn r702_124_ordinary_partner_remains_refused() {
    let d=CardDef::new("Ordinary","{0}","Legendary Creature","Partner",Some((2,2))).unwrap();
    crate::support::refused(game::play::check(&d).map_err(|e|e.0), "keyword: Partner");
}

#[test]
fn r608_2_optional_compound_actor_forms_remain_bounded() {
    for text in [
        "Each opponent may search their library for a card, reveal it, put it into their hand, then shuffle.",
        "Target player may search their library for a card, reveal it, put it into their hand, then draw a card.",
    ] {
        let d=CardDef::new("Unsupported","{0}","Instant",text,None).unwrap();
        assert!(game::play::check(&d).unwrap_err().0.contains("optional compound instruction"));
    }
}

#[test]
fn r603_3_partner_with_captured_entry_trigger_survives_source_departure() {
    let mut g=fixture();let wanted=g.put(2,1,Zone::Library);enter(&mut g,0,2);
    let source=g.battlefield[0];g.destroy(source);
    g.act(2,Move::Choose(Answer::Yes)).unwrap();
    let clone=g.clone();for mut branch in [g,clone] {
        branch.act(2,Move::Choose(Answer::Objects(vec![wanted]))).unwrap();
        cite!(["113.7a"]; assert_eq!(branch.players[2].hand.len(),1));assert!(branch.battlefield.is_empty());
        assert_eq!(branch.log.iter().filter(|e|matches!(e,Event::Revealed{player:2,cards} if cards==&vec![wanted])).count(),1);
        assert_eq!(branch.log.iter().filter(|e|matches!(e,Event::Shuffled{player:2, ..})).count(),1);
    }
}
