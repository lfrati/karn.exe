//! CR608.2f/101.4: one shared-verb put action partitions destinations;
//! every selection/order precedes all movement. CR404.3/401.4 owners order cards.
use crate::support::cite;
use game::{CardDef,Event,Game,Move,ObjId,Zone};
use game::state::{Answer,Choice,Decision};
use game::program::{Dest,Instr,Program,ActSpan};

const MULCH:&str="Reveal the top four cards of your library. Put all land cards revealed this way into your hand and the rest into your graveyard.";
fn setup(text:&str,lands:usize)->(Game,Vec<ObjId>,ObjId) {
    let defs=vec![CardDef::new("Partition","{0}","Sorcery",text,None).unwrap(),
        CardDef::new("Forest","","Basic Land — Forest","",None).unwrap(),
        CardDef::new("First","{0}","Sorcery","You gain 1 life.",None).unwrap(),
        CardDef::new("Second","{0}","Sorcery","You gain 2 life.",None).unwrap(),
        CardDef::new("Third","{0}","Sorcery","You gain 3 life.",None).unwrap(),
        CardDef::new("Fourth","{0}","Sorcery","You gain 4 life.",None).unwrap()];
    for def in &defs {game::play::check(def).unwrap();}
    let mut g=Game::new(defs,2,20);
    let cards=(0..4).map(|i|g.put(0,if i<lands {1}else{2+i-lands},Zone::Library)).collect();
    let spell=g.put(0,0,Zone::Hand);
    g.act(0,Move::Cast {card:spell,targets:vec![]}).unwrap();
    for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
    (g,cards,spell)
}
fn choose(g:&mut Game,order:Vec<ObjId>){g.act(0,Move::Choose(Answer::Objects(order))).unwrap();}
fn physical(g:&Game,cards:&[ObjId])->Vec<Option<u64>>{cards.iter().map(|card|g.physical_card_identity(*card)).collect()}

#[test]
fn r608_2f_mulch_keeps_every_partition_in_library_until_graveyard_order_then_moves_once() {
    // Full pinned Mulch text; its printing/cost is immaterial to this resolving mechanism.
    let (mut g,cards,_spell)=setup(MULCH,1);
    assert!(matches!(&g.decision,Some(Decision::Choose {player:0,choice:Choice::OrderCards {to:game::state::Placement::Zone(Zone::Graveyard),cards:offered}}) if *offered==vec![cards[3],cards[2],cards[1]]));
    cite!(["701.20b","608.2f","101.4"];assert_eq!(g.players[0].library,cards));
    assert!(g.players[0].hand.is_empty() && g.players[0].graveyard.is_empty());
    assert!(cards.iter().all(|card|g.sees(0,*card)&&g.sees(1,*card)));
    let before=format!("{g:?}");
    cite!(["404.3"];refused("order every offered card exactly once (401.4, 404.3)"):g.act(0,Move::Choose(Answer::Objects(vec![cards[1],cards[1],cards[2]]))));
    assert_eq!(format!("{g:?}"),before);
    cite!(["404.3"];refused("that decision is another player's"):g.act(1,Move::Choose(Answer::Objects(vec![cards[1],cards[2],cards[3]]))));
    assert_eq!(format!("{g:?}"),before);
    let mut clone=g.clone();
    choose(&mut g,vec![cards[1],cards[3],cards[2]]);
    choose(&mut clone,vec![cards[2],cards[1],cards[3]]);
    for game in [&g,&clone] {
        assert!(game.players[0].library.is_empty());
        assert_eq!(physical(game,&game.players[0].hand),physical(game,&cards[..1]));
        assert!(!game.sees(1,game.players[0].hand[0]));
        assert_eq!(game.players[0].graveyard.len(),4,"three nonlands, then resolving spell");
        assert!(game.stack.is_empty() && game.resolving().is_none());
        assert_eq!(game.players[0].life,20,"none of the revealed sorceries was cast");
        for original in &cards {assert_eq!(game.log.iter().filter(|e|matches!(e,Event::Moved {object,..} if object==original)).count(),1);}
    }
    // Answers are top-first; the graveyard stores oldest first, with the spell last.
    assert_eq!(physical(&g,&g.players[0].graveyard[..3]),physical(&g,&[cards[2],cards[3],cards[1]]));
    assert_eq!(physical(&clone,&clone.players[0].graveyard[..3]),physical(&clone,&[cards[3],cards[1],cards[2]]));
}

#[test]
fn r608_2f_empty_partitions_do_not_invent_choices_or_lose_the_nonempty_arm() {
    let (mut no_land,cards,_)=setup(MULCH,0);
    assert_eq!(no_land.players[0].library,cards);
    choose(&mut no_land,vec![cards[0],cards[2],cards[3],cards[1]]);
    assert!(no_land.players[0].hand.is_empty());
    assert_eq!(no_land.players[0].graveyard.len(),5);
    let (all_land,cards,_)=setup(MULCH,4);
    assert!(all_land.decision.is_none());
    assert!(all_land.players[0].library.is_empty());
    assert_eq!(physical(&all_land,&all_land.players[0].hand),physical(&all_land,&cards.iter().rev().copied().collect::<Vec<_>>()));
    assert_eq!(all_land.players[0].graveyard.len(),1);
}

#[test]
fn r608_2f_library_partition_waits_before_hand_move_and_private_order_does_not_change_public_events() {
    // Synthetic shared mechanism: no card admission claim.
    let text="Reveal the top four cards of your library. Put all land cards revealed this way into your hand and the rest on the bottom of your library in any order.";
    let (mut g,cards,_)=setup(text,1);
    assert_eq!(g.players[0].library,cards);
    assert!(g.players[0].hand.is_empty());
    let mut clone=g.clone();
    choose(&mut g,vec![cards[1],cards[2],cards[3]]);
    choose(&mut clone,vec![cards[3],cards[1],cards[2]]);
    assert_eq!(physical(&g,&g.players[0].library.iter().rev().copied().collect::<Vec<_>>()),physical(&g,&[cards[1],cards[2],cards[3]]));
    assert_eq!(physical(&clone,&clone.players[0].library.iter().rev().copied().collect::<Vec<_>>()),physical(&clone,&[cards[3],cards[1],cards[2]]));
    cite!(["701.20d"];assert!(cards[1..].iter().all(|card|g.obj(*card).zone==Zone::Gone)));
    cite!(["401.4"];assert_eq!(format!("{:?}",g.log),format!("{:?}",clone.log)));
    assert_eq!(g.players[0].hand.len(),1);
}

#[test]
fn r608_2f_battlefield_partition_keeps_hidden_sibling_unmoved_during_entry_choice_and_clone() {
    let defs=vec![CardDef::new("Partition","{0}","Sorcery","Search your library for up to two basic land cards, reveal them, put one onto the battlefield tapped and the other into your hand, then shuffle.",None).unwrap(),
        CardDef::new("Choosing Forest","","Basic Land — Forest","As this land enters, choose a color.\nThis land enters with two +1/+1 counters on it.",None).unwrap(),
        CardDef::new("Forest","","Basic Land — Forest","",None).unwrap()];
    for def in &defs {game::play::check(def).unwrap();}
    let mut g=Game::new(defs,2,20);let entry=g.put(0,1,Zone::Library);let hand=g.put(0,2,Zone::Library);let spell=g.put(0,0,Zone::Hand);
    g.act(0,Move::Cast {card:spell,targets:vec![]}).unwrap();for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
    choose(&mut g,vec![entry,hand]);choose(&mut g,vec![entry]);
    assert!(matches!(g.decision,Some(Decision::Choose {choice:Choice::Value(_),..})));
    assert!(g.players[0].hand.is_empty() && g.battlefield.is_empty());
    assert_eq!(g.obj(hand).zone,Zone::Library);assert_eq!(g.obj(entry).zone,Zone::Library);
    let mut clone=g.clone();
    for (game,color) in [(&mut g,ir::Color::Blue),(&mut clone,ir::Color::Red)] {
        game.act(0,Move::Choose(Answer::Value(game::binding::Value::Color(color)))).unwrap();
        assert_eq!(physical(game,&game.players[0].hand),physical(game,&[hand]));
        assert_eq!(physical(game,&game.battlefield),physical(game,&[entry]));
        let now=game.battlefield[0];assert!(game.obj(now).tapped);assert_eq!(game.obj(now).counters.get("+1/+1"),Some(&2));
        assert!(!game.sees(1,game.players[0].hand[0]));
    }
}

#[test]
fn invalid_single_move_cannot_smuggle_an_entry_destination_without_result_provenance() {
    let entry=game::program::action::Destination::Battlefield {owner_controls:true,by:game::program::Who::Controller,tapped:false,counters:vec![],attacking:None,transformed:false,face_down:None};
    let error=Program::new(vec![Instr::Move {src:0,to:Dest::Battlefield(entry),by:game::program::Who::Controller,when:None}],vec![ActSpan {end:1,moves:true,keeps_x:false}]).unwrap_err();
    assert_eq!(error.0,"battlefield result moves require a declared entry output");
}

#[test]
fn r608_2c_separate_put_instructions_stay_sequential() {
    let text="Reveal the top four cards of your library. Put all land cards revealed this way into your hand. Put the rest into your graveyard.";
    let (g,cards,_)=setup(text,1);
    assert_eq!(physical(&g,&g.players[0].hand),physical(&g,&cards[..1]));
    assert_eq!(g.players[0].library,cards[1..]);
    assert!(g.players[0].graveyard.is_empty());
    assert!(matches!(g.decision,Some(Decision::Choose {choice:Choice::OrderCards {..},..})));
}

#[test]
fn r608_2f_partition_entry_output_contains_only_its_successful_public_arrival() {
    use game::program::{Frame,Value,Who,run};
    use game::select::Ctx;
    use std::sync::Arc;
    for prevented in [false,true] {
        let defs=vec![CardDef::new("Source","{0}","Artifact","",None).unwrap(),
            CardDef::new("Bear","{1}{G}","Creature — Bear","",Some((2,2))).unwrap(),
            CardDef::new("Entry restriction","{0}","Artifact","Creature cards in libraries can't enter the battlefield.",None).unwrap()];
        for def in &defs {game::play::check(def).unwrap();}
        let mut g=Game::new(defs,2,20);let source=g.put(0,0,Zone::Battlefield);
        if prevented {g.put(1,2,Zone::Battlefield);}
        let sibling=g.put(0,1,Zone::Library);let entry=g.put(0,1,Zone::Library);
        let to=game::program::action::Destination::Battlefield {owner_controls:true,by:Who::Controller,tapped:true,counters:vec![],attacking:None,transformed:false,face_down:None};
        let program=Program::new(vec![
            Instr::TakeTop {n:1.into(),whose:game::program::action::Operand::Controller,out:0},
            Instr::TakeTop {n:2.into(),whose:game::program::action::Operand::Controller,out:1},
            Instr::Remainder {of:1,minus:vec![0],still_in:Zone::Library,out:2},
            Instr::Subjects {cards:0,out:3,still_in:None},
            Instr::MoveTogether {parts:vec![(3,Dest::Battlefield(to)),(2,Dest::Hand)],entered:vec![(3,4)]},
        ],vec![ActSpan {end:5,moves:true,keeps_x:false}]).unwrap();
        let mut frame=Frame::new(Arc::new(program),Ctx::new(0,source));
        assert!(run(&mut g,&mut frame).unwrap().is_none());
        assert_eq!(physical(&g,&g.players[0].hand),physical(&g,&[sibling]));
        if prevented {
            cite!(["609.3","614.17"];assert_eq!(frame.slot(4),Some(&Value::Cards(vec![]))));
            assert_eq!(g.obj(entry).zone,Zone::Library);
        } else {
            let now=*g.battlefield.iter().find(|card|g.physical_card_identity(**card)==g.physical_card_identity(entry)).unwrap();
            assert_eq!(frame.slot(4),Some(&Value::Cards(vec![entry,now])));
            assert!(g.obj(now).tapped);
        }
    }
}


#[test]
fn r608_2f_fact_or_fiction_waits_for_graveyard_order_before_moving_either_pile() {
    // Full pinned text; a four-card library reveals as much as possible (609.3).
    let text="Reveal the top five cards of your library. An opponent separates those cards into two piles. Put one pile into your hand and the other into your graveyard.";
    let (mut g,cards,_)=setup(text,1);
    g.act(1,Move::Choose(Answer::Objects(vec![cards[0]]))).unwrap();
    g.act(0,Move::Choose(Answer::Pile(0))).unwrap();
    assert_eq!(g.players[0].library,cards);
    assert!(g.players[0].hand.is_empty() && g.players[0].graveyard.is_empty());
    choose(&mut g,vec![cards[1],cards[3],cards[2]]);
    assert_eq!(physical(&g,&g.players[0].hand),physical(&g,&cards[..1]));
    assert_eq!(physical(&g,&g.players[0].graveyard[..3]),physical(&g,&[cards[2],cards[3],cards[1]]));
}

#[test]
fn r608_2f_random_library_arm_is_a_permutation_without_an_owner_order_choice() {
    let text="Reveal the top four cards of your library. Put all land cards revealed this way into your hand and the rest on the bottom of your library in a random order.";
    let (g,cards,_)=setup(text,1);
    assert!(g.decision.is_none());
    assert_eq!(physical(&g,&g.players[0].hand),physical(&g,&cards[..1]));
    let mut actual=physical(&g,&g.players[0].library);actual.sort();
    let mut expected=physical(&g,&cards[1..]);expected.sort();
    assert_eq!(actual,expected);
    assert!(cards[1..].iter().all(|card|g.obj(*card).zone==Zone::Gone));
    for original in cards {assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Moved {object,..} if *object==original)).count(),1);}
}
