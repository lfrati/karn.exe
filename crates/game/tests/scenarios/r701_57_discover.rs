//! Discover's sequential exile, cast permission and continuation (CR701.57a-c).
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Zone};
use game::state::{Answer, Choice, Decision};
fn card(name:&str,cost:&str,kind:&str,text:&str,pt:Option<(i32,i32)>)->CardDef {
    CardDef::new(name,cost,kind,text,pt).unwrap_or_else(|e|panic!("{name}: {e}"))
}
fn setup(text:&str)->Game {
    let defs=vec![card("Finder","{0}","Sorcery",text,None),
        card("Land","","Basic Land — Forest","",None),
        card("Bear","{2}","Creature — Bear","",Some((2,2))),
        card("Huge","{6}","Creature — Wurm","",Some((6,6))),
        card("Zero","{0}","Artifact","",None),
        card("Witness","{0}","Creature — Human","Whenever you discover, you gain 1 life.",Some((1,1)))];
    for d in &defs {game::play::check(d).unwrap_or_else(|e|panic!("{}: {}",d.name(),e.0));}
    Game::new(defs,2,20)
}
fn library(g:&mut Game,indices:&[usize])->Vec<ObjId>{indices.iter().map(|i|g.put(0,*i,Zone::Library)).collect()}
fn pass(g:&mut Game){for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}}
fn start(g:&mut Game){let spell=g.put(0,0,Zone::Hand);g.act(0,Move::Cast{card:spell,targets:vec![]}).unwrap();pass(g);}
fn cast_offer(g:&Game)->ObjId {let Some(Decision::Choose{choice:Choice::Cast{cards},..})=&g.decision else{panic!("{:?}",g.decision)};assert_eq!(cards.len(),1);cards[0]}
fn discoveries(g:&Game)->Vec<(usize,u32)>{g.log.iter().filter_map(|e|if let Event::Discovered{player,value}=e{Some((*player,*value))}else{None}).collect()}
#[test]
fn r701_57a_sequential_public_exile_cast_waits_for_parent_and_random_bottom() {
    let mut g=setup("Discover 3.");
    // Top is array end: Huge, Land, Bear; untouched Land remains above the returned bottom group.
    let cards=library(&mut g,&[1,2,1,3]);start(&mut g);
    let hit=cast_offer(&g);
    cite!(["701.57a","400.7"];assert_ne!(hit,cards[1]));
    let exiled:Vec<_>=g.log.iter().filter_map(|e|if let Event::Exiled{object}=e{Some(*object)}else{None}).collect();
    cite!(["701.57a"];assert_eq!(exiled,vec![cards[3],cards[2],cards[1]]));
    cite!(["701.57b"];assert!(discoveries(&g).is_empty()));
    cite!(["608.2g"];assert_eq!(g.stack.len(),1));
    cite!(["701.57a","118.9"];ok:g.act(0,Move::Cast{card:hit,targets:vec![]}));
    cite!(["608.2g"];assert_eq!(g.stack.len(),1));
    cite!(["608.2g"];assert_eq!(g.obj(g.stack[0]).chars.name,"Bear"));
    cite!(["701.57a"];assert_eq!(g.players[0].library.len(),3));
    cite!(["701.57a"];assert_eq!(*g.players[0].library.last().unwrap(),cards[0]));
    cite!(["701.57b"];assert_eq!(discoveries(&g),vec![(0,3)]));
    pass(&mut g);
    cite!(["608.3a"];assert!(g.battlefield.iter().any(|o|g.obj(*o).chars.name=="Bear")));
}
#[test]
fn r701_57a_decline_moves_only_hit_to_hand_and_never_retains_offer() {
    let mut g=setup("Discover 3.");library(&mut g,&[2,1]);start(&mut g);let hit=cast_offer(&g);
    cite!(["701.57a"];ok:g.act(0,Move::Choose(Answer::No)));
    cite!(["701.57a","400.7"];assert_eq!(g.obj(hit).zone,Zone::Gone));
    cite!(["701.57a"];assert_eq!(g.players[0].hand.len(),1));
    cite!(["701.57a"];assert_eq!(g.obj(g.players[0].hand[0]).chars.name,"Bear"));
    cite!(["701.57a"];assert_eq!(g.players[0].library.len(),1));
    cite!(["701.57b"];assert_eq!(discoveries(&g),vec![(0,3)]));
    cite!(["400.7"];refused("ceased to exist"):g.act(0,Move::Cast{card:hit,targets:vec![]}));
}
#[test]
fn r701_57b_empty_and_no_nonland_hit_still_finish_and_trigger_once() {
    for indices in [vec![],vec![1,1],vec![3,1]] {
        let mut g=setup("Discover 3.");g.put(0,5,Zone::Battlefield);library(&mut g,&indices);start(&mut g);
        cite!(["701.57a"];assert!(g.players[0].hand.is_empty()));
        cite!(["701.57a"];assert_eq!(g.players[0].library.len(),indices.len()));
        cite!(["701.57b","603.2"];assert_eq!(discoveries(&g),vec![(0,3)]));
        cite!(["603.3"];assert_eq!(g.stack.len(),1));pass(&mut g);
        cite!(["701.57b","603.2"];assert_eq!(g.players[0].life,21));
    }
}
#[test]
fn r701_57a_zero_and_large_values_are_semantic_amounts_without_text_expansion_caps() {
    for (text,bound,index,name) in [("Discover 0.",0,4,"Zero"),("Discover 11.",11,3,"Huge")] {
        let mut g=setup(text);library(&mut g,&[index,1]);start(&mut g);
        let hit=cast_offer(&g);
        cite!(["701.57a"];assert_eq!(g.obj(hit).chars.name,name));
        cite!(["701.57a"];ok:g.act(0,Move::Choose(Answer::No)));
        cite!(["701.57b"];assert_eq!(discoveries(&g),vec![(0,bound)]));
    }
}
#[test]
fn r701_57a_repeated_actions_complete_each_before_the_next_and_distinguish_ordinary_followup() {
    let mut g=setup("Discover 3, then discover 3. You gain 2 life.");library(&mut g,&[2,2]);start(&mut g);
    cite!(["701.57a"];ok:g.act(0,Move::Choose(Answer::No)));
    cite!(["701.57b"];assert_eq!(discoveries(&g),vec![(0,3)]));
    cite!(["608.2c"];assert_eq!(g.players[0].life,20));
    cast_offer(&g);
    cite!(["701.57a"];ok:g.act(0,Move::Choose(Answer::No)));
    cite!(["701.57b"];assert_eq!(discoveries(&g),vec![(0,3),(0,3)]));
    cite!(["608.2c"];assert_eq!(g.players[0].hand.len(),2));
    cite!(["608.2c"];assert_eq!(g.players[0].life,22));
}
#[test]
fn r701_57a_adventure_resulting_spell_bound_and_ordinary_free_cast_choices() {
    use game::card::CastAs;
    // Synthetic dual characteristics isolate the official Galvanic Giant/Storm Reading
    // ruling's 4-versus-7 boundary without claiming its other abilities are implemented.
    let normal=card("Giant","{4}","Creature — Giant","",Some((3,3)));
    let alternate=card("Reading","{7}","Sorcery — Adventure","You gain 1 life.",None);
    let adventure=CardDef::adventurer(normal,alternate).unwrap();
    let mut g=setup("Discover 4.");
    let mut defs=g.defs.to_vec();defs.push(adventure);
    g=Game::new(defs,2,20);g.put(0,6,Zone::Library);start(&mut g);let hit=cast_offer(&g);
    cite!(["701.57a","715.3a"];refused("effect's bound"):g.act(0,Move::CastAs(CastAs::Adventure,Box::new(Move::Cast{card:hit,targets:vec![]}))));
    cite!(["400.7","733.1"];assert_eq!(g.obj(hit).zone,Zone::Exile));
    cite!(["701.57a"];ok:g.act(0,Move::Cast{card:hit,targets:vec![]}));
    cite!(["701.57a","608.2g"];assert_eq!(g.obj(g.stack[0]).chars.name,"Giant"));
}
#[test]
fn r701_57a_omen_uses_normal_card_value_for_scan_and_its_own_spell_value_for_cast() {
    use game::card::CastAs;
    let primary=card("Dragon","{2}","Creature — Dragon","",Some((3,3)));
    let omen=card("Wind","{4}","Instant — Omen","You gain 1 life.",None);
    let physical=CardDef::omen_card(primary,omen).unwrap();
    for bound in [2,4] {
        let mut g=setup(&format!("Discover {bound}."));let mut defs=g.defs.to_vec();defs.push(physical.clone());g=Game::new(defs,2,20);
        g.put(0,6,Zone::Library);start(&mut g);let hit=cast_offer(&g);
        let cast=Move::CastAs(CastAs::Omen,Box::new(Move::Cast{card:hit,targets:vec![]}));
        if bound==2 {cite!(["701.57a","720.3a"];refused("effect's bound"):g.act(0,cast));g.act(0,Move::Choose(Answer::No)).unwrap();}
        else {cite!(["701.57a","720.3a"];ok:g.act(0,cast));cite!(["608.2g"];assert_eq!(g.obj(g.stack[0]).chars.mana_value,4));pass(&mut g);cite!(["720.3d"];assert_eq!(g.players[0].life,21));cite!(["720.3d"];assert_eq!(g.players[0].library.len(),1));}
    }
}
fn real(name:&str)->CardDef {
    use std::sync::LazyLock;
    static FACES:LazyLock<Vec<oracle::corpus::Face>>=LazyLock::new(||oracle::corpus::load(&oracle::corpus::default_path()).unwrap());
    CardDef::from_faces(&CardDef::catalog(&FACES)[name]).unwrap_or_else(|e|panic!("{name}: {e}"))
}
#[test]
fn r701_57a_hurl_uses_countered_spell_mana_value_with_chosen_x_as_last_known() {
    let mut g=setup("Discover 3.");let mut defs=g.defs.to_vec();defs[0]=real("Hurl into History");
    defs.push(card("Variable","{X}{U}","Artifact","",None));g=Game::new(defs,2,20);
    g.players[1].pool.colors[1]=1;g.players[1].pool.colorless=5;g.priority=1;g.active=1;
    let other=g.put(1,6,Zone::Hand);g.act(1,crate::support::paid("CCCCCU", Move::WithX(5,Box::new(Move::Cast{card:other,targets:vec![]})))).unwrap();
    let target=g.stack[0];g.act(1,Move::Pass).unwrap();
    g.players[0].pool.colors[1]=2;g.players[0].pool.colorless=3;
    library(&mut g,&[3]);let hurl=g.put(0,0,Zone::Hand);
    cite!(["202.3e","107.3a"];assert_eq!(g.obj(target).chars.mana_value,6));
    cite!(["601.2c"];ok:g.act(0,crate::support::paid("CCCUU", Move::Cast{card:hurl,targets:vec![vec![target.into()]]})));pass(&mut g);
    let hit=cast_offer(&g);
    cite!(["701.57a","608.2h"];assert_eq!(g.obj(hit).chars.name,"Huge"));
    cite!(["701.6a","400.7"];assert_eq!(g.obj(target).zone,Zone::Gone));
    cite!(["202.3e"];assert_eq!(g.obj(g.players[1].graveyard[0]).chars.mana_value,1));
    g.act(0,Move::Choose(Answer::No)).unwrap();
    cite!(["701.57b","608.2h"];assert_eq!(discoveries(&g),vec![(0,6)]));
}
#[test]
fn r701_57a_zoyowa_uses_owner_as_actor_and_predeparture_permanent_value() {
    let mut g=setup("Discover 3.");let mut defs=g.defs.to_vec();defs[0]=real("Zoyowa's Justice");
    defs.push(card("Owned","{4}","Creature — Bear","",Some((4,4))));
    defs.push(card("Steal","{0}","Sorcery","Gain control of target creature.",None));g=Game::new(defs,2,20);
    let target=g.put(1,6,Zone::Battlefield);let steal=g.put(0,7,Zone::Hand);
    g.act(0,Move::Cast{card:steal,targets:vec![vec![target.into()]]}).unwrap();pass(&mut g);
    cite!(["110.2a"];assert_eq!((g.obj(target).owner,g.controller(target)),(1,0)));
    let untouched=library(&mut g,&[3])[0];g.players[0].pool.colors[3]=1;g.players[0].pool.colorless=1;
    let justice=g.put(0,0,Zone::Hand);g.act(0,crate::support::paid("CR", Move::Cast{card:justice,targets:vec![vec![target.into()]]})).unwrap();pass(&mut g);
    let Some(Decision::Choose{player,choice:Choice::Cast{cards}})=&g.decision else{panic!("{:?}",g.decision)};
    cite!(["701.57a","400.3"];assert_eq!(*player,1));
    cite!(["701.57a","400.7"];assert_eq!(cards.len(),1));
    cite!(["701.57a"];assert_eq!(g.obj(cards[0]).chars.name,"Owned"));
    cite!(["400.3"];assert_eq!(g.players[0].library,vec![untouched]));
    g.act(1,Move::Choose(Answer::No)).unwrap();
    cite!(["701.57b","608.2h"];assert_eq!(discoveries(&g),vec![(1,4)]));
    cite!(["400.3"];assert_eq!(g.players[1].hand.len(),1));
}
#[test]
fn r701_57a_free_cast_x_is_zero_and_additional_cost_failure_keeps_hand_route() {
    for (extra,x_refusal) in [
        (card("Variable","{X}","Artifact","",None),true),
        (card("Demand","{0}","Sorcery","As an additional cost to cast this spell, sacrifice a creature.\nYou gain 1 life.",None),false),
    ] {
        let mut g=setup("Discover 3.");let mut defs=g.defs.to_vec();defs.push(extra);g=Game::new(defs,2,20);
        g.put(0,6,Zone::Library);start(&mut g);let hit=cast_offer(&g);
        let command=if x_refusal {Move::WithX(1,Box::new(Move::Cast{card:hit,targets:vec![]}))} else {Move::Cast{card:hit,targets:vec![]}};
        cite!(["107.3b","118.9d","601.2h"];refused(if x_refusal{"X is 0"}else{"cost"}):g.act(0,command));
        cite!(["733.1","400.7"];assert_eq!(g.obj(hit).zone,Zone::Exile));
        cite!(["701.57b"];assert!(discoveries(&g).is_empty()));
        g.act(0,Move::Choose(Answer::No)).unwrap();
        cite!(["701.57a"];assert_eq!(g.players[0].hand.len(),1));
        cite!(["701.57b"];assert_eq!(discoveries(&g),vec![(0,3)]));
    }
}
#[test]
fn r701_57a_free_cast_pays_additional_cost_before_parent_finishes() {
    let mut g=setup("Discover 3.");let mut defs=g.defs.to_vec();
    defs.push(card("Demand","{0}","Sorcery","As an additional cost to cast this spell, sacrifice a creature.\nYou gain 1 life.",None));g=Game::new(defs,2,20);
    let bear=g.put(0,2,Zone::Battlefield);g.put(0,6,Zone::Library);start(&mut g);let hit=cast_offer(&g);
    cite!(["118.9d","601.2h"];ok:g.act(0,Move::WithCosts(vec![vec![bear]],Box::new(Move::Cast{card:hit,targets:vec![]}))));
    cite!(["601.2h","701.17a"];assert!(g.players[0].graveyard.iter().any(|o|g.obj(*o).chars.name=="Bear")));
    cite!(["608.2g"];assert_eq!(g.obj(g.stack[0]).chars.name,"Demand"));
    cite!(["701.57b","608.2g"];assert_eq!(discoveries(&g),vec![(0,3)]));
    cite!(["608.2g"];assert_eq!(g.players[0].life,20));pass(&mut g);
    cite!(["608.2c"];assert_eq!(g.players[0].life,21));
}
#[test]
fn r701_57a_no_legal_target_keeps_hit_available_for_hand_instead() {
    let mut g=setup("Discover 3.");let mut defs=g.defs.to_vec();
    defs.push(card("Removal","{1}","Sorcery","Destroy target creature.",None));g=Game::new(defs,2,20);
    g.put(0,6,Zone::Library);start(&mut g);let hit=cast_offer(&g);
    cite!(["601.2c","701.57a"];refused("target group"):g.act(0,Move::Cast{card:hit,targets:vec![]}));
    cite!(["733.1"];assert_eq!(g.obj(hit).zone,Zone::Exile));
    cite!(["701.57a"];ok:g.act(0,Move::Choose(Answer::No)));
    cite!(["701.57a"];assert_eq!(g.players[0].hand.len(),1));
}
#[test]
fn discover_full_consumer_inventory() {
    let names=["Hurl into History","Curator of Sun's Creation","Chimil, the Inner Sun","Zoyowa's Justice","Hidden Nursery","Hidden Cataract","Hidden Volcano","Hidden Courtyard","Hidden Necropolis","Zoetic Glyph","Buried Treasure","Swashbuckler's Whip","Caparocti Sunborn","Primordial Gnawer","Quintorius Kand","Geological Appraiser","Hit the Mother Lode","Daring Discovery","Brass's Tunnel-Grinder // Tecutlan, the Searing Rift","Walk with the Ancestors","Digsite Conservator","Etali's Favor","Trumpeting Carnosaur"];
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog=CardDef::catalog(&faces);
    let legal:std::collections::HashSet<String>=std::fs::read_to_string(oracle::corpus::root().join("resources/oracle/legal-standard.jsonl")).unwrap().lines()
        .map(|line|serde_json::from_str::<serde_json::Value>(line).unwrap()["oracle_id"].as_str().unwrap().to_owned()).collect();
    // Conservative stem/keyword superset across every legal face includes action,
    // completed-action trigger and discovered-card/value reference uses.
    let found:std::collections::BTreeSet<_>=faces.iter().filter(|f|legal.contains(&f.oracle_id)
        && (f.oracle_text.to_ascii_lowercase().contains("discover") || f.keywords.iter().any(|k|k.to_ascii_lowercase().contains("discover"))))
        .map(|f|f.name.as_str()).collect();
    assert_eq!(found,names.into_iter().collect());
    for name in names {
        let result=CardDef::from_faces(&catalog[name]).and_then(|d|game::play::check(&d).map_err(|e|e.0));
        // Full-card admission evidence only: these independent concepts remain
        // explicitly refused; no action/trigger/reference use is silently omitted.
        let refusal=match name {
            "Curator of Sun's Creation"|"Hit the Mother Lode"|"Brass's Tunnel-Grinder // Tecutlan, the Searing Rift"=>Some("text the grammar does not read"),
            "Caparocti Sunborn"=>Some("bounded tap selection: controller chooses a mandatory action"),
            "Quintorius Kand"=>Some("trigger: Cast"),
            _=>None,
        };
        if let Some(reason)=refusal {assert_eq!(result.unwrap_err(),reason,"{name}");}
        else {result.unwrap_or_else(|e|panic!("{name}: {e}"));}
    }
}
