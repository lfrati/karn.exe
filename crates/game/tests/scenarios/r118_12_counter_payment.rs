use crate::support::{cite, mana_pool, paid};
use game::{CardDef,Game,Move,Target,Zone};
use game::state::{Answer,Choice,Decision};
fn card(name:&str,cost:&str,types:&str,text:&str,pt:Option<(i32,i32)>)->CardDef{
    let d=CardDef::new(name,cost,types,text,pt).unwrap();game::play::check(&d).unwrap();d
}
fn pass(g:&mut Game){for _ in 0..g.players.len(){g.act(g.priority,Move::Pass).unwrap();}}
#[test]
fn r118_12a_frost_titan_offers_the_spell_controller_payment_then_counters_if_declined(){
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let titan=CardDef::from_face(faces.iter().find(|f|f.name=="Frost Titan").unwrap()).unwrap();game::play::check(&titan).unwrap();
    let kill=card("Kill","{0}","Instant","Destroy target creature.",None);
    for pay in [false,true]{
        let mut g=Game::new(vec![titan.clone(),kill.clone()],3,20);let source=g.put(0,0,Zone::Battlefield);
        g.act(0,Move::Pass).unwrap();let spell=g.put(1,1,Zone::Hand);
        g.act(1,Move::Cast{card:spell,targets:vec![vec![Target::Object(source)]]}).unwrap();
        g.players[1].pool.colorless=2;pass(&mut g);
        cite!(["118.12a"]; assert!(matches!(g.decision,Some(Decision::Choose{player:1,choice:Choice::Pay{..}})),"{:?}",g.decision));
        cite!(["118.12a"]; refused("that decision is another player's"): g.act(0,Move::Choose(Answer::Pay { plan: vec![], with: None, tapped: vec![] })));
        g.act(1,Move::Choose(if pay{Answer::Pay { plan: vec![], with: Some(mana_pool("CC")), tapped: vec![] }}else{Answer::No})).unwrap();
        if pay {cite!(["118.12a"]; assert_eq!(g.stack.len(),1));pass(&mut g);assert!(g.battlefield.is_empty());}
        else {cite!(["118.12a","701.6a"]; assert!(g.stack.is_empty()));cite!(["701.6a"]; assert_eq!(g.battlefield,vec![source]));assert_eq!(g.players[1].pool.colorless,2);}
    }
}
#[test]
fn r107_3a_unless_payment_uses_the_announced_x(){
    let demand=card("Demand","{X}","Instant","Counter target spell unless its controller pays {X}.",None);
    let draw=card("Life","{0}","Instant","You gain 3 life.",None);
    let mut g=Game::new(vec![demand,draw],2,20);let spell=g.put(0,1,Zone::Hand);
    g.act(0,Move::Cast{card:spell,targets:vec![]}).unwrap();g.act(0,Move::Pass).unwrap();
    let target=*g.stack.last().unwrap();let counter=g.put(1,0,Zone::Hand);g.players[1].pool.colorless=3;g.players[0].pool.colorless=3;
    g.act(1,paid("CCC", Move::WithX(3,Box::new(Move::Cast{card:counter,targets:vec![vec![Target::Object(target)]]})))).unwrap();pass(&mut g);
    assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::Pay{..}})));
    let clone=g.clone();for mut g in [g,clone]{g.act(0,Move::Choose(Answer::Pay { plan: vec![], with: Some(mana_pool("CCC")), tapped: vec![] })).unwrap();cite!(["107.3a","107.3i"]; assert_eq!(g.players[0].pool.colorless,0));pass(&mut g);assert_eq!(g.players[0].life,23);}
}
#[test]
fn r701_6_syncopate_has_its_counter_destination_replacement(){
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let d=CardDef::from_face(faces.iter().find(|f|f.name=="Syncopate").unwrap()).unwrap();
    game::play::check(&d).unwrap();
    for (uncounterable,pay,available,x) in [(false,false,0,3),(false,false,3,3),(false,true,3,3),(true,false,0,3),(false,true,0,0)] {
        let text=if uncounterable {"This spell can't be countered.\nYou gain 3 life."} else {"You gain 3 life."};
        let life=card("Life","{0}","Instant",text,None);
        let mut g=Game::new(vec![d.clone(),life],2,20);let spell=g.put(0,1,Zone::Hand);
        g.act(0,Move::Cast{card:spell,targets:vec![]}).unwrap();let target=*g.stack.last().unwrap();g.act(0,Move::Pass).unwrap();
        let counter=g.put(1,0,Zone::Hand);g.players[1].pool.colors[1]=x+1;g.players[0].pool.colorless=available;
        g.act(1,paid(&"U".repeat((x+1) as usize), Move::WithX(x,Box::new(Move::Cast{card:counter,targets:vec![vec![Target::Object(target)]]})))).unwrap();pass(&mut g);
        if g.decision.is_some(){g.act(0,Move::Choose(if pay{Answer::Pay { plan: vec![], with: Some(mana_pool(&"C".repeat(x as usize))), tapped: vec![] }}else{Answer::No})).unwrap();}
        if !pay&&!uncounterable {
            assert!(g.stack.is_empty());cite!(["608.2c"]; assert_eq!(g.exile.len(),1));assert_eq!(g.obj(g.exile[0]).card,Some(1));
            cite!(["614.1a"]; assert!(!g.log.iter().any(|e|matches!(e,game::Event::ZoneChanged{was,to:Zone::Graveyard,..} if *was==target)),"counter destination is replaced, not followed by another move"));
        } else {
            // can't be countered (101.2), paid {X} (118.12a), or paid {0} with nothing (118.3a)
            let rule=if uncounterable {"101.2"} else if x==0 {"118.3a"} else {"118.12a"};
            assert!(g.exile.is_empty());cite!([rule]; case: format!("{uncounterable},{pay},{available},{x}"); assert_eq!(g.stack,vec![target]));pass(&mut g);assert_eq!(g.players[0].life,23);
        }
    }
}

#[test]
fn r603_2_frost_titans_other_trigger_taps_and_skips_only_the_next_untap(){
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let d=CardDef::from_face(faces.iter().find(|f|f.name=="Frost Titan").unwrap()).unwrap();
    let bear=card("Bear","{0}","Creature — Bear","",Some((2,2)));
    let mut g=Game::new(vec![d,bear],2,20);for p in 0..2{for _ in 0..4{g.put(p,1,Zone::Library);}}
    let target=g.put(1,1,Zone::Battlefield);let spell=g.put(0,0,Zone::Hand);g.players[0].pool.colors[1]=6;
    g.act(0,paid("UUUUUU", Move::Cast{card:spell,targets:vec![]})).unwrap();pass(&mut g);
    g.act(0,Move::ChooseTargets(vec![vec![Target::Object(target)]])).unwrap();pass(&mut g);cite!(["603.2"]; assert!(g.obj(target).tapped));
    let mut upkeeps=0;for _ in 0..120 {
        if g.active==1&&g.part==ir::Part::Upkeep&&g.priority==1 {upkeeps+=1;if upkeeps==1{cite!(["502.3"]; assert!(g.obj(target).tapped));}else{cite!(["502.3"]; assert!(!g.obj(target).tapped));return;}}
        match g.decision.clone(){
            Some(Decision::Attackers{player})=>g.act(player,Move::Attack(vec![])).unwrap(),
            Some(Decision::Blockers{player})=>g.act(player,Move::Block(vec![])).unwrap(),
            None=>g.act(g.priority,Move::Pass).unwrap(),other=>panic!("{other:?}"),
        }
    }panic!("never reached second upkeep");
}
