//! Fixed mana buyback composes announcement/payment with retained successful completion.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use game::state::{Answer, Choice, Decision, DestinationEffect};
use ir::Part;
const WHISPERS:usize=0; const CAPSIZE:usize=1; const REITERATE:usize=2; const TWINCAST:usize=3;
const LEYLINE:usize=4; const THROUGH:usize=5; const LAND:usize=6; const BEAR:usize=7;
fn full(name:&str)->CardDef {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let def=CardDef::from_faces(&CardDef::catalog(&faces)[name]).unwrap();
    game::play::check(&def).unwrap_or_else(|e|panic!("{name}: {}",e.0)); def
}
fn setup()->Game {
    let mut g=Game::new(vec![full("Whispers of the Muse"),full("Capsize"),full("Reiterate"),full("Twincast"),
        full("Leyline of Anticipation"),full("Cast Through Time"),full("Island"),full("Grizzly Bears")],2,20);
    for p in 0..2 { for _ in 0..24 {g.put(p,LAND,Zone::Library);} } g
}
fn blue(g:&mut Game,n:u32) {g.players[0].pool.colors[ir::Color::Blue as usize]=n;}
fn pass(g:&mut Game) {for _ in 0..g.players.len(){g.act(g.priority,Move::Pass).unwrap();}}
fn cast(g:&mut Game,card:ObjId,buyback:bool,mana:&str,targets:Vec<Vec<Target>>) {
    g.act(0,crate::support::paid(mana,Move::WithBuyback(buyback,Box::new(Move::Cast{card,targets})))).unwrap();
}

fn choice(g:&Game)->(ObjId,Vec<DestinationEffect>) {
    let Some(Decision::Choose{player:0,choice:Choice::SpellDestination{on,among}})=&g.decision else{panic!("{:?}",g.decision)};
    (*on,among.clone())
}
fn choose(g:&mut Game,on:ObjId,effect:DestinationEffect){g.act(0,Move::Choose(Answer::SpellDestination{on,effect})).unwrap();}
fn upkeep(g:&mut Game) {
    let turn=g.turn;
    for _ in 0..160 {
        if g.turn>turn&&g.active==0&&g.part==Part::Upkeep{return;}
        assert!(g.stack.is_empty(),"unexpected stack: {:?}",g.stack);
        match g.decision {Some(Decision::Attackers{player})=>g.act(player,Move::Attack(vec![])).unwrap(),
            Some(Decision::Blockers{player})=>g.act(player,Move::Block(vec![])).unwrap(),_=>{g.act(g.priority,Move::Pass).unwrap();}}
    } panic!("no upkeep");
}
#[test]
fn r702_27a_full_cards_and_fixed_mana_boundary() {
    setup();
    for text in ["Buyback {X}\nDraw a card.","Buyback—Discard a card.\nDraw a card.","Buyback {1}\nBuyback {2}\nDraw a card."] {
        let def=CardDef::new("Unsupported buyback","{U}","Instant",text,None).unwrap();
        let reason=if text.matches("Buyback").count()==2 { "multiple buyback costs require separately identified announcements (702.27)" } else { "keyword: Buyback" };
        crate::support::refused(game::play::check(&def).map_err(|e|e.0),reason);
    }
    let def=CardDef::new("Permanent buyback","{U}","Creature","Buyback {1}",Some((1,1))).unwrap();
    crate::support::refused(game::play::check(&def).map_err(|e|e.0),"keyword: Buyback");
}
#[test]
fn r601_2b_h_only_payable_buyback_is_offered_and_paid_once() {
    let mut g=setup(); let card=g.put(0,WHISPERS,Zone::Hand);blue(&mut g,1);
    let before=g.log.len(); cite!(["702.27a"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0,crate::support::paid("U",Move::WithBuyback(true,Box::new(Move::Cast{card,targets:vec![]})))));
    assert_eq!((g.players[0].pool.total(),g.obj(card).zone,g.log.len()),(1,Zone::Hand,before));
    blue(&mut g,6);cast(&mut g,card,true,"UUUUUU",vec![]);
    cite!(["601.2f"]; assert_eq!(g.players[0].pool.total(),0));let spell=g.stack[0];assert!(g.obj(spell).buyback_paid);
    let clone=g.clone();for mut g in [g,clone] {pass(&mut g);cite!(["702.27a"]; assert_eq!(g.players[0].hand.len(),2));
        let returned=*g.players[0].hand.iter().find(|o|g.obj(**o).card==Some(WHISPERS)).unwrap();cite!(["400.7"]; assert!(!g.obj(returned).buyback_paid));
        assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Resolved{spell:s} if *s==spell)).count(),1);}
}
#[test]
fn r702_27a_full_capsize_paid_and_unpaid_have_distinct_completion() {
    for paid in [false,true] {let mut g=setup();let target=g.put(1,BEAR,Zone::Battlefield);let card=g.put(0,CAPSIZE,Zone::Hand);
        blue(&mut g,6);cast(&mut g,card,paid,if paid { "UUUUUU" } else { "UUU" },vec![vec![Target::Object(target)]]);cite!(["601.2f"]; assert_eq!(g.players[0].pool.total(),if paid{0}else{3}));
        pass(&mut g);assert_eq!(g.players[1].hand.len(),1);assert_eq!(g.obj(target).zone,Zone::Gone);
        cite!(["702.27a"]; assert_eq!(g.players[0].hand.len(),usize::from(paid)));cite!(["702.27a"]; assert_eq!(g.players[0].graveyard.len(),usize::from(!paid)));}
}
#[test]
fn r601_2b_buyback_invalid_declarations_are_atomic() {
    let mut g=setup();let card=g.put(0,WHISPERS,Zone::Hand);blue(&mut g,6);
    cite!(["601.2b"]; refused("announce buyback only once (601.2b)"): g.act(0,Move::WithBuyback(true,Box::new(Move::WithBuyback(false,Box::new(Move::Cast{card,targets:vec![]}))))));
    let land=g.put(0,LAND,Zone::Battlefield);
    cite!(["702.27a"]; refused("buyback is announced only with a cast (702.27a)"): g.act(0,Move::WithBuyback(false,Box::new(Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }))));
    let ordinary=g.put(0,TWINCAST,Zone::Hand);
    for paid in [false,true] {cite!(["702.27a"]; refused("that spell has no buyback cost (702.27a)"): g.act(0,Move::WithBuyback(paid,Box::new(Move::Cast{card:ordinary,targets:vec![vec![Target::Object(card)]]}))));}
    assert_eq!((g.obj(card).zone,g.players[0].pool.total(),g.obj(land).tapped),(Zone::Hand,6,false));
}
#[test]
fn r608_2b_counter_and_wholly_illegal_target_do_not_buy_back() {
    for counter in [false,true] {let mut g=setup();let target=g.put(1,BEAR,Zone::Battlefield);let card=g.put(0,CAPSIZE,Zone::Hand);blue(&mut g,6);
        cast(&mut g,card,true,"UUUUUU",vec![vec![Target::Object(target)]]);let spell=g.stack[0];
        if counter {assert!(g.counter(spell));} else {g.move_object(target,Zone::Exile);pass(&mut g);}
        assert!(g.players[0].hand.is_empty());
        if counter {cite!(["701.6a"]; assert_eq!(g.players[0].graveyard.len(),1));} else {cite!(["608.2b"]; assert_eq!(g.players[0].graveyard.len(),1));}
        assert_eq!(g.players[0].pool.total(),0);
        assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Resolved{spell:s} if *s==spell)).count(),0);}
}
#[test]
fn r702_88a_late_full_rebound_grant_asks_after_body_and_clone_does_not_repeat_it() {
    for effect in [DestinationEffect::Buyback,DestinationEffect::Rebound] {let mut g=setup();g.put(0,LEYLINE,Zone::Battlefield);
        let card=g.put(0,WHISPERS,Zone::Hand);blue(&mut g,13);cast(&mut g,card,true,"UUUUUU",vec![]);let spell=g.stack[0];
        let through=g.put(0,THROUGH,Zone::Hand);g.act(0,crate::support::paid("UUUUUUU",Move::Cast{card:through,targets:vec![]})).unwrap();pass(&mut g);pass(&mut g);
        let (on,among)=choice(&g);assert_eq!(on,spell);cite!(["616.1"]; assert_eq!(among,vec![DestinationEffect::Buyback,DestinationEffect::Rebound]));
        assert_eq!(g.players[0].hand.len(),1,"Whispers drew before the completion choice");
        let count=g.log.len();let decision=g.decision.clone();
        crate::support::refused(g.act(1,Move::Choose(Answer::SpellDestination{on,effect})), "that decision is another player's");
        cite!(["616.1"]; refused("choose an offered destination for this resolving spell (616.1)"): g.act(0,Move::Choose(Answer::SpellDestination{on:card,effect})));
        cite!(["616.1"]; refused("choose an offered destination for this resolving spell (616.1)"): g.act(0,Move::Choose(Answer::SpellDestination{on,effect:DestinationEffect::CastRider})));
        cite!(["616.1"]; refused("choose an offered destination for this resolving spell (616.1)"): g.act(0,Move::Choose(Answer::No)));assert_eq!(g.decision,decision);assert_eq!(g.log.len(),count);
        let clone=g.clone();for mut g in [g,clone] {choose(&mut g,on,effect);
            assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Resolved{spell:s} if *s==spell)).count(),1);
            assert_eq!(g.players[0].library.len(),23);
            assert_eq!(g.players[0].hand.len(),if effect==DestinationEffect::Buyback{2}else{1});
            upkeep(&mut g);if effect==DestinationEffect::Buyback {assert!(g.stack.is_empty());} else {
                cite!(["702.88a"]; assert_eq!(g.stack.len(),1));pass(&mut g);assert!(matches!(g.decision,Some(Decision::Choose{choice:Choice::Cast{..},..})));
                let copy=g.clone();for mut g in [g,copy] {g.act(0,Move::Choose(Answer::No)).unwrap();cite!(["702.88a"]; assert_eq!(g.exile.len(),1));}
            }
        }
    }
}
#[test]
fn r702_88a_capsize_removing_rebound_grant_during_body_has_only_buyback() {
    let mut g=setup();let through=g.put(0,THROUGH,Zone::Battlefield);let card=g.put(0,CAPSIZE,Zone::Hand);blue(&mut g,6);
    cast(&mut g,card,true,"UUUUUU",vec![vec![Target::Object(through)]]);pass(&mut g);
    assert!(g.decision.is_none());cite!(["702.27a"]; assert_eq!(g.players[0].hand.len(),2));assert!(g.exile.is_empty());upkeep(&mut g);assert!(g.stack.is_empty());
}
#[test]
fn r707_10_copy_retains_buyback_paid_without_creating_another_real_card() {
    for paid in [false,true] {let mut g=setup();let card=g.put(0,WHISPERS,Zone::Hand);blue(&mut g,8);cast(&mut g,card,paid,if paid { "UUUUUU" } else { "U" },vec![]);
        let twin=g.put(0,TWINCAST,Zone::Hand);g.act(0,crate::support::paid("UU",Move::Cast{card:twin,targets:vec![vec![Target::Object(g.stack[0])]]})).unwrap();pass(&mut g);
        if matches!(g.decision,Some(Decision::Choose{choice:Choice::May { .. },..})) {g.act(0,Move::Choose(Answer::No)).unwrap();}
        assert_eq!(g.stack.len(),2);let copy=g.stack[1];cite!(["707.10"]; assert_eq!(g.obj(copy).buyback_paid,paid));let clone=g.clone();
        let audit=g.audit_game_objects();
        assert_eq!(audit["objects"][g.stack[0].0.to_string()]["announced"]["costs"]["buyback"],paid);
        let copied=&audit["objects"][copy.0.to_string()]["announced"];
        assert!(copied["cast_by"].is_null());
        // CR 707.10 copies whether an additional cost was paid without casting
        // or paying again. The audit now retains the exact frozen definition,
        // so export that choice rather than require the former missing-evidence refusal.
        cite!(["707.10"]; assert_eq!(copied["costs"]["buyback"],paid));
        assert!(copied["costs"]["unavailable"].is_null());
        for mut g in [g,clone] {pass(&mut g);cite!(["707.10a"]; assert_eq!(g.players[0].hand.len(),1,"copy draws once and ceases outside stack"));
            pass(&mut g);assert_eq!(g.players[0].hand.len(),if paid{3}else{2});
            cite!(["702.27a"]; assert_eq!(g.players[0].hand.iter().filter(|o|g.obj(**o).card==Some(WHISPERS)).count(),usize::from(paid)));}
    }
}
#[test]
fn r702_27a_full_reiterate_copies_body_and_buys_back_itself() {
    let mut g=setup();let whisper=g.put(0,WHISPERS,Zone::Hand);blue(&mut g,1);cast(&mut g,whisper,false,"U",vec![]);
    let original=g.stack[0];let card=g.put(0,REITERATE,Zone::Hand);g.players[0].pool.colors[3]=6;
    cast(&mut g,card,true,"RRRRRR",vec![vec![Target::Object(original)]]);pass(&mut g);
    if matches!(g.decision,Some(Decision::Choose{choice:Choice::May { .. },..})) {g.act(0,Move::Choose(Answer::No)).unwrap();}
    assert_eq!(g.players[0].hand.len(),1);cite!(["702.27a"]; assert_eq!(g.obj(g.players[0].hand[0]).card,Some(REITERATE)));pass(&mut g);pass(&mut g);
    cite!(["707.10"]; assert_eq!(g.players[0].hand.len(),3));
}
#[test]
fn r702_34a_buyback_flashback_exiles_and_the_copy_does_not_inherit_flashback() {
    let def=CardDef::new("Flash buyback","{U}","Instant","Buyback {1}\nFlashback {U}\nDraw a card.",None).unwrap();game::play::check(&def).unwrap();
    for paid in [false,true] {let mut defs=setup().defs.as_ref().clone();defs.push(def.clone());let mut g=Game::new(defs,2,20);for p in 0..2 {for _ in 0..24 {g.put(p,LAND,Zone::Library);}}let card=g.put(0,g.defs.len()-1,Zone::Graveyard);blue(&mut g,4);
        g.act(0,crate::support::paid(if paid { "UU" } else { "U" },Move::CastVia(game::announcement::CastRoute::Flashback,Box::new(Move::WithBuyback(paid,Box::new(Move::Cast{card,targets:vec![]})))))).unwrap();let original=g.stack[0];
        let twin=g.put(0,TWINCAST,Zone::Hand);g.act(0,crate::support::paid("UU",Move::Cast{card:twin,targets:vec![vec![Target::Object(original)]]})).unwrap();pass(&mut g);
        if matches!(g.decision,Some(Decision::Choose{choice:Choice::May { .. },..})) {g.act(0,Move::Choose(Answer::No)).unwrap();}
        pass(&mut g);pass(&mut g);cite!(["702.34a"]; assert_eq!(g.exile.len(),1));assert_eq!(g.players[0].hand.len(),2);
        cite!(["702.34a"]; assert_eq!(g.players[0].hand.iter().filter(|o|g.obj(**o).card==Some(g.defs.len()-1)).count(),0));}
}

#[test]
fn r608_2n_offered_free_cast_still_pays_buyback_and_chooses_against_rider() {
    let offer=CardDef::new("Free rider","{0}","Instant","Exile the top card of your library. You may cast that card without paying its mana cost. If an instant or sorcery spell cast this way would be put into your graveyard, exile it instead.",None).unwrap();
    game::play::check(&offer).unwrap();
    for paid in [false,true] {for effect in [DestinationEffect::Buyback,DestinationEffect::CastRider] {
        let mut defs=setup().defs.as_ref().clone();defs.push(offer.clone());let mut g=Game::new(defs,2,20);
        for _ in 0..4 {g.put(0,LAND,Zone::Library);}g.put(0,WHISPERS,Zone::Library);
        let offer=g.put(0,g.defs.len()-1,Zone::Hand);blue(&mut g,5);
        g.act(0,Move::Cast{card:offer,targets:vec![]}).unwrap();pass(&mut g);
        assert!(matches!(g.decision,Some(Decision::Choose{choice:Choice::Cast{..},..})),"{:?}",g.decision);
        let card=*g.exile.last().unwrap();let Some((_,game::legal::Asked::Cast{cards}))=g.asked() else {panic!("missing cast permission")};
        assert_eq!(cards, vec![card]);
        cast(&mut g,card,paid,if paid { "UUUUU" } else { "" },vec![]);cite!(["601.2f"]; assert_eq!(g.players[0].pool.total(),if paid{0}else{5}));pass(&mut g);
        if paid {let(on,among)=choice(&g);cite!(["616.1"]; assert_eq!(among,vec![DestinationEffect::Buyback,DestinationEffect::CastRider]));
            let clone=g.clone();for mut g in [g,clone] {choose(&mut g,on,effect);assert_eq!(g.players[0].library.len(),3);
                assert_eq!(g.players[0].hand.len(),if effect==DestinationEffect::Buyback{2}else{1});assert_eq!(g.exile.len(),usize::from(effect==DestinationEffect::CastRider));}}
        else {assert!(g.decision.is_none());assert_eq!(g.exile.len(),1);assert_eq!(g.players[0].hand.len(),1);}
    }}
}

#[test]
fn r601_2f_buyback_joins_alternative_cost_before_one_total_reduction() {
    let spell=CardDef::new("Alternative buyback","{4}{U}","Instant","Buyback {3}\nYou may pay {U} rather than pay this spell's mana cost.\nYou gain 1 life.",None).unwrap();
    let discount=CardDef::new("Discount","{0}","Enchantment","Spells cost {1} less to cast.",None).unwrap();
    for d in [&spell,&discount]{game::play::check(d).unwrap();}
    for alternative in [false,true] {
        let mut g=Game::new(vec![spell.clone(),discount.clone()],2,20);g.put(0,1,Zone::Battlefield);let card=g.put(0,0,Zone::Hand);
        blue(&mut g,if alternative{3}else{7});
        let base=Move::WithBuyback(true,Box::new(Move::Cast{card,targets:vec![]}));
        g.act(0,crate::support::paid(if alternative { "UUU" } else { "UUUUUUU" },if alternative{Move::CastFor(game::announcement::CastCost::Alternative(1),Box::new(base))}else{base})).unwrap();
        cite!(["601.2f"]; assert_eq!(g.players[0].pool.total(),0));pass(&mut g);assert_eq!(g.players[0].life,21);cite!(["702.27a"]; assert_eq!(g.players[0].hand.len(),1));
    }
}

#[test]
fn r601_2g_buyback_fact_survives_mana_child_clone_and_payment_reversal() {
    use game::legal::Step;
    let texts=[("Mana creature","Creature — Elf","{T}: Put a +1/+1 counter on this creature. Add X mana of any one color, where X is the number of +1/+1 counters on this creature."),
        ("Scales","Enchantment","If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead."),
        ("Evolution","Enchantment","If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.")];
    let mut defs=Vec::new();for(n,t,x) in texts {let d=CardDef::new(n,"{0}",t,x,if t.starts_with("Creature"){Some((1,1))}else{None}).unwrap();game::play::check(&d).unwrap();defs.push(d);}
    defs.push(CardDef::new("Buyback consumer","{1}","Sorcery","Buyback {3}\nYou gain 1 life.",None).unwrap());
    let mut g=Game::new(defs,2,20);let source=g.put(0,0,Zone::Battlefield);g.put(0,1,Zone::Battlefield);let evolution=g.put(0,2,Zone::Battlefield);let card=g.put(0,3,Zone::Hand);
    let step=Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![], source,ability:0,paying:vec![],adding:vec![vec![ir::mana::ManaSymbol::Color(ir::Color::Blue);4]]};
    g.act(0,Move::WithBuyback(true,Box::new(Move::WithMana(vec![step],Box::new(Move::Cast{card,targets:vec![]}))))).unwrap();
    let spell=g.stack[0];assert!(g.obj(spell).buyback_paid);let Some(Decision::Choose{choice:Choice::Replacement{among,..},..})=&g.decision else {panic!("{:?}",g.decision)};
    let effect=among.iter().find(|e|e.source==evolution).unwrap().clone();let clone=g.clone();
    for mut g in [g,clone] {
        g.act(0,Move::Choose(Answer::Replacement(effect.clone()))).unwrap();
        if matches!(g.decision,Some(Decision::Choose{choice:Choice::Mana{..},..})) {g.act(0,Move::Choose(Answer::Mana(vec![ir::mana::ManaSymbol::Color(ir::Color::Blue);3]))).unwrap();}
        assert!(matches!(g.decision,Some(Decision::Payment{..})));let mut corrected=g.clone();
        corrected.players[0].pool.colorless+=1;corrected.act(0,Move::FinishPayment{plan:vec![],paying:vec![],with:Some(crate::support::mana_pool("UUUC"))}).unwrap();
        assert!(corrected.obj(spell).buyback_paid);pass(&mut corrected);cite!(["702.27a"]; assert_eq!(corrected.players[0].hand.len(),1));assert_eq!(corrected.players[0].life,21);
        g.act(0,Move::ReversePayment).unwrap();cite!(["733.1"]; assert_eq!(g.obj(card).zone,Zone::Hand));assert!(!g.obj(card).buyback_paid);assert!(g.stack.is_empty());assert!(g.obj(source).tapped);
    }
}
