//! Collect Evidence's selection, semantic occurrence and compound payment (701.59).
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Zone};
use game::state::{Answer, Choice, Decision};
fn card(name:&str,cost:&str,kind:&str,text:&str,pt:Option<(i32,i32)>)->CardDef {
    CardDef::new(name,cost,kind,text,pt).unwrap_or_else(|e|panic!("{name}: {e}"))
}
fn setup(text:&str)->Game {
    let defs=vec![card("Collector","{0}","Sorcery",text,None),
        card("Two","{2}","Creature — Bear","",Some((2,2))),
        card("Four","{4}","Creature — Bear","",Some((4,4))),
        card("Land","","Basic Land — Forest","",None),
        card("Watcher","{0}","Creature — Human","Whenever you collect evidence, you gain 1 life.",Some((1,1)))];
    for d in &defs {game::play::check(d).unwrap_or_else(|e|panic!("{}: {}",d.name(),e.0));}
    Game::new(defs,2,20)
}
fn pass(g:&mut Game){for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}}
fn start(g:&mut Game){let spell=g.put(0,0,Zone::Hand);g.act(0,Move::Cast{card:spell,targets:vec![]}).unwrap();pass(g);}
fn evidence(g:&Game)->Vec<usize>{g.log.iter().filter_map(|e|if let Event::EvidenceCollected{player}=e{Some(*player)}else{None}).collect()}
fn choose(g:&mut Game,objects:Vec<ObjId>){g.act(0,Move::Choose(Answer::Objects(objects))).unwrap();}
#[test]
fn r701_59a_whole_group_threshold_distinct_own_graveyard_cards() {
    let mut g=setup("Collect evidence 4.");let two=g.put(0,1,Zone::Graveyard);let four=g.put(0,2,Zone::Graveyard);let other=g.put(1,2,Zone::Graveyard);start(&mut g);
    let Some(Decision::Choose{choice:Choice::ConstrainedObjects{among,selection,..},..})=&g.decision else{panic!("{:?}",g.decision)};
    cite!(["701.59a"];assert_eq!(among,&vec![two,four]));
    cite!(["701.59a"];assert!(!selection.accepts(&[two])));
    cite!(["701.59a"];refused("selection"):g.act(0,Move::Choose(Answer::Objects(vec![other]))));
    cite!(["701.59a"];refused("selection"):g.act(0,Move::Choose(Answer::Objects(vec![two,two]))));
    cite!(["701.59a"];refused("selection"):g.act(0,Move::Choose(Answer::Objects(vec![two]))));
    choose(&mut g,vec![four]);
    cite!(["701.59a"];assert_eq!(g.obj(two).zone,Zone::Graveyard));
    cite!(["701.59a"];assert_eq!(g.exile.len(),1));
    cite!(["701.59a"];assert_eq!(evidence(&g),vec![0]));
}
#[test]
fn r701_59b_impossible_optional_collection_is_not_offered_or_paid() {
    let mut g=setup("You may collect evidence 4. If you do, you gain 3 life.");g.put(0,1,Zone::Graveyard);start(&mut g);
    cite!(["701.59b","118.3"];assert!(g.decision.is_none()));
    cite!(["118.12"];assert_eq!(g.players[0].life,20));
    cite!(["701.59b"];assert!(evidence(&g).is_empty()));
}
#[test]
fn r701_59a_zero_collection_is_an_occurrence_but_declining_is_not() {
    for consent in [false,true] {
        let mut g=setup("You may collect evidence 0. If you do, you gain 3 life.");g.put(0,4,Zone::Battlefield);start(&mut g);
        cite!(["701.59a","118.12"];assert!(matches!(g.decision,Some(Decision::Choose{choice:Choice::May,..}))));
        g.act(0,Move::Choose(if consent{Answer::Yes}else{Answer::No})).unwrap();
        cite!(["701.59a","118.12"];assert_eq!(evidence(&g),if consent{vec![0]}else{vec![]}));
        cite!(["118.12"];assert_eq!(g.players[0].life,if consent{23}else{20}));
        if consent {pass(&mut g);cite!(["603.2"];assert_eq!(g.players[0].life,24));}
    }
}
#[test]
fn r701_59a_additional_cost_retains_collection_not_ordinary_exile() {
    let mut g=setup("As an additional cost to cast this spell, collect evidence 4.\nYou gain 1 life.");g.put(0,4,Zone::Battlefield);let four=g.put(0,2,Zone::Graveyard);let spell=g.put(0,0,Zone::Hand);
    cite!(["601.2h","701.59a"];ok:g.act(0,Move::WithCosts(vec![vec![four]],Box::new(Move::Cast{card:spell,targets:vec![]}))));
    cite!(["701.59a","603.2"];assert_eq!(evidence(&g),vec![0]));
    pass(&mut g);cite!(["603.3"];assert_eq!(g.players[0].life,21));
    pass(&mut g);cite!(["608.2c"];assert_eq!(g.players[0].life,22));
    let two=g.put(0,1,Zone::Graveyard);g.exile(two);
    cite!(["701.59a"];assert_eq!(evidence(&g),vec![0]));
}
fn phoenix()->Game {
    let defs=vec![card("Phoenix","{1}{R}{R}","Creature — Phoenix","Flying\nWhen this creature dies, you may exile it and collect evidence 4. If you do, return this card to the battlefield tapped.",Some((3,3))),
        card("Two","{2}","Creature — Bear","",Some((2,2))),card("Four","{4}","Creature — Bear","",Some((4,4))),card("Kill","{0}","Instant","Destroy target creature.",None)];
    for d in &defs{game::play::check(d).unwrap_or_else(|e|panic!("{}: {}",d.name(),e.0));}Game::new(defs,2,20)
}
fn kill(g:&mut Game,bird:ObjId){let spell=g.put(0,3,Zone::Hand);g.act(0,Move::Cast{card:spell,targets:vec![vec![game::Target::Object(bird)]]}).unwrap();pass(g);}
#[test]
fn r118_12_compound_payment_projects_required_self_exile_before_evidence() {
    let mut g=phoenix();let bird=g.put(0,0,Zone::Battlefield);g.put(0,1,Zone::Graveyard);kill(&mut g,bird);pass(&mut g);
    cite!(["118.3","118.12","701.59b"];assert!(g.decision.is_none()));
    cite!(["118.3"];assert!(g.exile.is_empty()));
    cite!(["701.59b"];assert!(evidence(&g).is_empty()));
    cite!(["118.3"];assert_eq!(g.players[0].graveyard.len(),3));
}
#[test]
fn r400_7j_compound_tracks_own_public_successor_and_returns_only_that_phoenix() {
    let mut g=phoenix();let bird=g.put(0,0,Zone::Battlefield);let other=g.put(0,0,Zone::Graveyard);let four=g.put(0,2,Zone::Graveyard);kill(&mut g,bird);pass(&mut g);
    g.act(0,Move::Choose(Answer::Yes)).unwrap();
    cite!(["118.3","608.2c"];assert!(g.exile.is_empty()));
    choose(&mut g,vec![four]);
    cite!(["400.7e","400.7j"];assert_eq!(g.battlefield.len(),1));
    let returned=g.battlefield[0];cite!(["400.7"];assert_ne!(returned,bird));
    cite!(["608.2c"];assert!(g.obj(returned).tapped));
    cite!(["400.7"];assert_eq!(g.obj(other).zone,Zone::Graveyard));
    cite!(["701.59a"];assert_eq!(evidence(&g),vec![0]));
}
#[test]
fn collect_evidence_consumer_inventory() {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();let catalog=CardDef::catalog(&faces);
    let legal:std::collections::HashSet<String>=std::fs::read_to_string(oracle::corpus::root().join("resources/oracle/legal-standard.jsonl")).unwrap().lines()
        .map(|line|serde_json::from_str::<serde_json::Value>(line).unwrap()["oracle_id"].as_str().unwrap().to_owned()).collect();
    let names:std::collections::BTreeSet<_>=faces.iter().filter(|f|legal.contains(&f.oracle_id)&& (f.oracle_text.to_ascii_lowercase().contains("evidence") || f.keywords.iter().any(|k|k.to_ascii_lowercase().contains("evidence")))).map(|f|f.name.clone()).collect();
    let expected:std::collections::BTreeSet<_>=["Analyze the Pollen","Axebane Ferox","Behind the Mask","Bite Down on Crime","Conspiracy Unraveler","Crimestopper Sprite","Cryptex","Deadly Cover-Up","Evidence Examiner","Extract a Confession","Forensic Researcher","Hedge Whisperer","Incinerator of the Guilty","Izoni, Center of the Web","Kylox's Voltstrider","Lamplight Phoenix","Polygraph Orb","Sample Collector","Surveillance Monitor","Tenth District Hero","Urgent Necropsy","Vitu-Ghazi Inspector"].into_iter().map(str::to_string).collect();
    assert_eq!(names,expected);
    for name in names {
        let result=CardDef::from_faces(catalog.get(name.as_str()).unwrap()).and_then(|d|game::play::check(&d).map(|_|d).map_err(|e|e.0));
        let expected = match name.as_str() {
            "Axebane Ferox" => Some("unless: a payment of Numbered"),
            "Bite Down on Crime" | "Kylox's Voltstrider" => Some("program: control flow not lowered"),
            "Conspiracy Unraveler" => Some("ability on a permanent: sentence: AlternativeCost"),
            "Deadly Cover-Up" => Some("noun phrase: an unsupported zone or zone owner"),
            "Hedge Whisperer" => Some("ability on a permanent: static: ChooseNotToUntap"),
            "Incinerator of the Guilty" => Some("amount: X"),
            "Polygraph Orb" => Some("unless: alternative payments"),
            "Tenth District Hero" => Some("text the grammar does not read"),
            _ => None,
        };
        match expected {
            Some(reason) => assert_eq!(result.unwrap_err(),reason,"{name}"),
            None => { result.unwrap_or_else(|e|panic!("{name}: {e}")); },
        }
    }
}
fn target_cost()->Game {
    let defs=vec![card("Demand","{1}","Instant","As an additional cost to cast this spell, collect evidence X, where X is the total mana value of the permanents this spell targets.\nDestroy up to one target artifact, up to one target creature, up to one target enchantment, and up to one target planeswalker.",None),
        card("Resource","{4}","Artifact Creature — Bear","Sacrifice this creature: Add {C}.",Some((4,4))),card("Four","{4}","Creature — Bear","",Some((4,4))),card("Two","{2}","Creature — Bear","",Some((2,2)))];
    for d in &defs{game::play::check(d).unwrap_or_else(|e|panic!("{}: {}",d.name(),e.0));}Game::new(defs,2,20)
}
#[test]
fn r601_2f_target_total_counts_a_permanent_once_across_separate_target_groups() {
    let mut g=target_cost();let target=g.put(1,1,Zone::Battlefield);let four=g.put(0,2,Zone::Graveyard);let spell=g.put(0,0,Zone::Hand);g.players[0].pool.colorless=1;
    let targets=vec![vec![game::Target::Object(target)],vec![game::Target::Object(target)],vec![],vec![]];
    cite!(["115.3","601.2f","701.59a"];ok:g.act(0,crate::support::paid("C",Move::WithCosts(vec![vec![four]],Box::new(Move::Cast{card:spell,targets})))));
    cite!(["701.59a"];assert_eq!(evidence(&g),vec![0]));pass(&mut g);
    cite!(["608.2b"];assert!(!g.battlefield.contains(&target)));
}
#[test]
fn r601_2f_target_minimum_is_locked_before_target_leaves_during_mana_payment() {
    let mut g=target_cost();let target=g.put(0,1,Zone::Battlefield);let two=g.put(0,3,Zone::Graveyard);let four=g.put(0,2,Zone::Graveyard);let spell=g.put(0,0,Zone::Hand);
    let targets=vec![vec![game::Target::Object(target)],vec![game::Target::Object(target)],vec![],vec![]];
    let plan=vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose:vec![],source:target,ability:0,paying:vec![],adding:vec![]}];
    let cast=|cards|crate::support::paid("C",Move::WithMana(plan.clone(),Box::new(Move::WithCosts(vec![cards],Box::new(Move::Cast{card:spell,targets:targets.clone()})))));
    cite!(["601.2f","601.2g","701.59a","733.1"];refused("less than 4"):g.act(0,cast(vec![two])));
    cite!(["733.1"];assert_eq!(g.obj(target).zone,Zone::Battlefield));
    cite!(["601.2f","701.59a"];ok:g.act(0,cast(vec![four])));
    cite!(["601.2g"];assert!(!g.battlefield.contains(&target)));
    pass(&mut g);cite!(["608.2b"];assert!(g.stack.is_empty()));
    cite!(["701.59a"];assert_eq!(evidence(&g),vec![0]));
}
#[test]
fn r601_2c_zero_targets_have_zero_evidence_minimum_and_a_truthful_listed_witness() {
    let mut g=target_cost();let spell=g.put(0,0,Zone::Hand);g.players[0].pool.colorless=1;
    cite!(["701.59a"];ok:g.act(0,crate::support::paid("C",Move::WithCosts(vec![vec![]],Box::new(Move::Cast{card:spell,targets:vec![vec![],vec![],vec![],vec![]]})))));
    cite!(["701.59a"];assert_eq!(evidence(&g),vec![0]));pass(&mut g);
    cite!(["608.2b"];assert!(g.stack.is_empty()));
}
#[test]
fn r603_2c_multi_card_evidence_is_one_collection_and_one_graveyard_leave_batch() {
    let mut g=setup("Collect evidence 4.");let mut defs=g.defs.to_vec();
    defs.push(card("Leave watcher","{0}","Creature — Human","Whenever one or more creature cards leave your graveyard, you gain 1 life.",Some((1,1))));g=Game::new(defs,2,20);g.put(0,5,Zone::Battlefield);
    let a=g.put(0,1,Zone::Graveyard);let b=g.put(0,1,Zone::Graveyard);start(&mut g);choose(&mut g,vec![a,b]);
    cite!(["701.59a"];assert_eq!(evidence(&g),vec![0]));
    cite!(["603.2c","608.2e"];assert_eq!(g.stack.len(),1));pass(&mut g);
    cite!(["603.2c"];assert_eq!(g.players[0].life,21));
}
#[test]
fn r400_7_unrelated_graveyard_departure_and_return_does_not_recover_phoenix() {
    let mut g=phoenix();let mut defs=g.defs.to_vec();defs.push(card("Detour","{0}","Instant","Exile target card from a graveyard. Return it to its owner's graveyard.",None));g=Game::new(defs,2,20);
    let bird=g.put(0,0,Zone::Battlefield);g.put(0,2,Zone::Graveyard);kill(&mut g,bird);
    let dead=*g.players[0].graveyard.iter().find(|o|g.obj(**o).chars.name=="Phoenix").unwrap();let spell=g.put(0,4,Zone::Hand);
    g.act(0,Move::Cast{card:spell,targets:vec![vec![game::Target::Object(dead)]]}).unwrap();pass(&mut g);
    let fresh=*g.players[0].graveyard.iter().find(|o|g.obj(**o).chars.name=="Phoenix").unwrap();cite!(["400.7"];assert_ne!(fresh,dead));
    pass(&mut g);cite!(["400.7","118.3"];assert!(g.decision.is_none()));
    cite!(["400.7"];assert!(g.battlefield.is_empty()));
    cite!(["400.7"];assert_eq!(g.obj(fresh).zone,Zone::Graveyard));
    cite!(["701.59b"];assert!(evidence(&g).is_empty()));
}
#[test]
fn r118_3_projection_uses_changed_eligibility_after_the_required_prefix() {
    let defs=vec![card("Payment","{0}","Sorcery","You may exile target permanent and collect evidence 4. If you do, you gain 3 life.",None),
        card("Ban","{0}","Artifact","Cards in your graveyard can't be exiled.",None),card("Four","{4}","Creature — Bear","",Some((4,4)))];
    for d in &defs {game::play::check(d).unwrap_or_else(|e|panic!("{}: {}",d.name(),e.0));}
    let mut g=Game::new(defs,2,20);let ban=g.put(0,1,Zone::Battlefield);let four=g.put(0,2,Zone::Graveyard);let spell=g.put(0,0,Zone::Hand);
    g.act(0,Move::Cast{card:spell,targets:vec![vec![game::Target::Object(ban)]]}).unwrap();pass(&mut g);
    cite!(["118.3","608.2c"];assert!(matches!(g.decision,Some(Decision::Choose{choice:Choice::May,..}))));
    g.act(0,Move::Choose(Answer::Yes)).unwrap();cite!(["118.3"];assert_eq!(g.obj(ban).zone,Zone::Battlefield));
    choose(&mut g,vec![four]);cite!(["118.12"];assert_eq!(g.players[0].life,23));
    cite!(["701.59a"];assert_eq!(evidence(&g),vec![0]));
}
#[test]
fn r607_2i_optional_evidence_link_belongs_to_this_cast_not_a_global_collection() {
    for paid in [false,true] {
        let defs=vec![card("Earlier","{0}","Sorcery","Collect evidence 0.",None),
            card("Linked","{0}","Creature — Bear","As an additional cost to cast this spell, you may collect evidence 0.\nWhen this creature enters, if evidence was collected, you gain 3 life.",Some((2,2)))];
        for d in &defs {game::play::check(d).unwrap_or_else(|e|panic!("{}: {}",d.name(),e.0));}
        let mut g=Game::new(defs,2,20);start(&mut g);
        cite!(["701.59a"];assert_eq!(evidence(&g),vec![0]));
        let spell=g.put(0,1,Zone::Hand);let cast=Move::WithOptional(paid,Box::new(Move::WithCosts(if paid{vec![vec![]]}else{vec![]},Box::new(Move::Cast{card:spell,targets:vec![]}))));g.act(0,cast).unwrap();pass(&mut g);
        if paid {pass(&mut g);}
        cite!(["607.2i","701.59c"];assert_eq!(g.players[0].life,if paid{23}else{20}));
        cite!(["701.59a"];assert_eq!(evidence(&g),if paid{vec![0,0]}else{vec![0]}));
    }
}
#[test]
fn r701_59a_threshold_above_ten_has_no_text_expansion_cap() {
    let mut g=setup("Collect evidence 11.");let cards=(0..3).map(|_|g.put(0,2,Zone::Graveyard)).collect::<Vec<_>>();start(&mut g);choose(&mut g,cards);
    cite!(["701.59a"];assert_eq!(g.exile.len(),3));
    cite!(["701.59a"];assert_eq!(evidence(&g),vec![0]));
}
#[test]
fn r400_7j_same_occurrence_public_chain_stops_at_a_hidden_destination() {
    let mut g=setup("Exile target creature. Return it to its owner's hand. Exile it.");let bear=g.put(1,1,Zone::Battlefield);let spell=g.put(0,0,Zone::Hand);
    g.act(0,Move::Cast{card:spell,targets:vec![vec![game::Target::Object(bear)]]}).unwrap();pass(&mut g);
    cite!(["400.7","400.7j"];assert_eq!(g.players[1].hand.len(),1));
    cite!(["400.7j"];assert!(g.exile.is_empty()));
    let exiles=g.log.iter().filter(|e|matches!(e,Event::Exiled{..})).count();
    cite!(["400.7j"];assert_eq!(exiles,1));
}
#[test]
fn r701_59b_exile_prohibition_prevents_collecting_and_zero_is_still_possible() {
    for minimum in [0,4] {
        let mut g=setup(&format!("You may collect evidence {minimum}."));let mut defs=g.defs.to_vec();
        defs.push(card("Ban","{0}","Artifact","Cards in your graveyard can't be exiled.",None));g=Game::new(defs,2,20);g.put(0,5,Zone::Battlefield);let four=g.put(0,2,Zone::Graveyard);start(&mut g);
        if minimum==0 {g.act(0,Move::Choose(Answer::Yes)).unwrap();}
        cite!(["101.2","701.59b"];assert_eq!(g.obj(four).zone,Zone::Graveyard));
        cite!(["701.59a","701.59b"];assert_eq!(evidence(&g),if minimum==0{vec![0]}else{vec![]}));
    }
}
