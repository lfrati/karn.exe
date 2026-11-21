use crate::support::cite;
use game::{CardDef,Game,Move,Target,Zone};
use game::state::{Answer,Decision,Choice};
fn game()->Game{
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let d=CardDef::from_face(faces.iter().find(|f|f.name=="Dimir Charm").unwrap()).unwrap();game::play::check(&d).unwrap();
    let bear=CardDef::new("Bear","{0}","Creature — Bear","",Some((2,2))).unwrap();
    let other=CardDef::new("Wolf","{0}","Creature — Wolf","",Some((3,3))).unwrap();
    Game::new(vec![d,bear,other],3,20)
}
fn pass(g:&mut Game){for _ in 0..g.players.len(){g.act(g.priority,Move::Pass).unwrap();}}
#[test]
fn r701_20e_look_at_another_library_keeps_chooser_visibility_and_destination_separate(){
    let mut g=game();let own=g.put(0,1,Zone::Library);let cards:Vec<_>=(0..4).map(|i|g.put(1,if i==2 {2}else{1},Zone::Library)).collect();
    let spell=g.put(0,0,Zone::Hand);g.players[0].pool.colors[1]=1;g.players[0].pool.colors[2]=1;
    g.act(0,Move::Modes(vec![2],Box::new(crate::support::paid("UB", Move::Cast{card:spell,targets:vec![vec![Target::Player(1)]]})))).unwrap();pass(&mut g);
    assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::Objects{..}})),"{:?}",g.decision);
    cite!(["701.20e"]; assert_eq!(g.observations(0).len(),1));cite!(["701.20e"]; assert!(g.observations(1).is_empty()));assert!(g.observations(2).is_empty());
    assert_eq!(g.observations(0)[0].cards.iter().map(|c|c.object).collect::<Vec<_>>(),vec![cards[3],cards[2],cards[1]]);
    crate::support::refused(g.act(1,Move::Choose(Answer::Objects(vec![cards[2]]))), "that decision is another player's");
    cite!(["608.2d"]; refused("a chosen object is not among the offered objects to put (608.2d)"): g.act(0,Move::Choose(Answer::Objects(vec![own]))));
    let clone=g.clone();for mut g in [g,clone]{
        g.act(0,Move::Choose(Answer::Objects(vec![cards[2]]))).unwrap();
        assert_eq!(g.decision,Some(Decision::Choose{player:1,choice:Choice::OrderCards{to:game::state::Placement::Zone(Zone::Graveyard),cards:vec![cards[3],cards[1]]}}));
        g.act(1,Move::Choose(Answer::Objects(vec![cards[1],cards[3]]))).unwrap();
        assert_eq!(g.players[0].library,vec![own]);assert_eq!(g.players[1].library.len(),2);assert_eq!(g.players[1].library[0],cards[0]);
        let top=g.players[1].library[1];assert_eq!(g.obj(top).card,Some(2));cite!(["701.20d"]; assert_ne!(top,cards[2],"701.20d/e: reordered looked-at card becomes a new object"));
        cite!(["608.2c"]; assert_eq!(g.players[1].graveyard.len(),2));assert!(g.players[0].graveyard.iter().all(|o|g.obj(*o).card==Some(0)));
    }
}

#[test]
fn r609_3_a_short_library_supplies_only_its_remaining_cards(){
    for count in 0..=2 {
        let mut g=game();
        let cards:Vec<_>=(0..count).map(|i|g.put(1,1+i,Zone::Library)).collect();
        let spell=g.put(0,0,Zone::Hand);g.players[0].pool.colors[1]=1;g.players[0].pool.colors[2]=1;
        g.act(0,Move::Modes(vec![2],Box::new(crate::support::paid("UB", Move::Cast{card:spell,targets:vec![vec![Target::Player(1)]]})))).unwrap();pass(&mut g);
        if count == 2 {
            assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::Objects{..}})));
            // Keep the bottom Bear, not the top Wolf: ignoring the answer is observable.
            g.act(0,Move::Choose(Answer::Objects(vec![cards[0]]))).unwrap();
            cite!(["608.2d"]; assert_eq!(g.obj(g.players[1].library[0]).card,Some(1)));
            cite!(["608.2d"]; assert_eq!(g.obj(g.players[1].graveyard[0]).card,Some(2)));
        } else if let Some(Decision::Choose{choice:Choice::Objects{..},..})=g.decision {
            g.act(0,Move::Choose(Answer::Objects(cards.clone()))).unwrap();
        }
        assert!(g.decision.is_none(),"{:?}",g.decision);
        cite!(["609.3"]; case: count; assert_eq!(g.players[1].library.len(),count.min(1)));
        cite!(["609.3"]; case: count; assert_eq!(g.players[1].graveyard.len(),count.saturating_sub(1)));
        assert!(g.stack.is_empty());
    }
}
#[test]
fn r700_2_other_modes_keep_their_own_target_constraints(){
    let mut g=game();let small=g.put(1,1,Zone::Battlefield);let large=g.put(1,2,Zone::Battlefield);
    let spell=g.put(0,0,Zone::Hand);g.players[0].pool.colors[1]=1;g.players[0].pool.colors[2]=1;
    cite!(["115.1"]; refused("an illegal target, Wolf: its power is 3, and the target says power 2 or less (115.1)"): g.act(0,Move::Modes(vec![1],Box::new(crate::support::paid("UB", Move::Cast{card:spell,targets:vec![vec![Target::Object(large)]]})))));
    g.act(0,Move::Modes(vec![1],Box::new(crate::support::paid("UB", Move::Cast{card:spell,targets:vec![vec![Target::Object(small)]]})))).unwrap();pass(&mut g);
    assert!(g.battlefield.contains(&large));cite!(["700.2"]; assert!(!g.battlefield.contains(&small)));
}
