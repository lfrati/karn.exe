use crate::support::{cite,paid,mana_pool};
use game::{CardDef,Game,Move,Zone,Target};
use game::announcement::CastCost;
use ir::{Color,mana::ManaSymbol};
fn def(name:&str,cost:&str,kind:&str,text:&str) -> CardDef {
    let d=CardDef::new(name,cost,kind,text,None).unwrap();game::play::check(&d).unwrap_or_else(|e|panic!("{name}: {}",e.0));d
}
fn resolve(g:&mut Game) {for _ in 0..20 {if g.stack.is_empty(){return;}g.act(g.priority,Move::Pass).unwrap();}panic!("stack did not finish");}
#[test]
fn r603_4_bauble_checks_the_triggering_spells_actual_payment_not_its_mana_value() {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let bauble=CardDef::from_face(faces.iter().find(|f|f.face_name=="Vexing Bauble").unwrap()).unwrap();
    game::play::check(&bauble).unwrap();
    let defs=vec![bauble,def("Free","{0}","Instant","You gain 1 life."),
        def("Tax","{0}","Enchantment","Noncreature spells cost {1} more to cast."),
        def("Mountain","","Basic Land — Mountain","({T}: Add {R}.)"),
        def("Alternate","{5}{R}","Instant","You may sacrifice a Mountain rather than pay this spell's mana cost.\nThis spell deals 2 damage to any target."),
    ];
    for (alternative,tax) in [(false,false),(false,true),(true,false),(true,true)] {
        let mut g=Game::new(defs.clone(),2,20);g.put(0,0,Zone::Battlefield);
        if tax {g.put(0,2,Zone::Battlefield);g.add_mana(0,ManaSymbol::Colorless);}
        let card=g.put(0,if alternative {4}else{1},Zone::Hand);
        let action=if alternative {
            let mountain=g.put(0,3,Zone::Battlefield);
            Move::CastFor(CastCost::Alternative(0),Box::new(Move::WithCosts(vec![vec![mountain]],Box::new(Move::Cast{card,targets:vec![vec![Target::Player(1)]]}))))
        } else {Move::Cast{card,targets:vec![]}};
        g.act(0,paid(if tax{"C"}else{""},action)).unwrap();
        cite!(["603.4"];case:format!("alternative={alternative}, tax={tax}");assert_eq!(g.stack.len(),if tax {1}else{2},"alternative={alternative}, tax={tax}"));
        resolve(&mut g);
        assert_eq!(g.players[0].life,if tax&&!alternative{21}else{20});
        assert_eq!(g.players[1].life,if tax&&alternative{18}else{20});
    }
}
#[test]
fn r601_2h_mana_types_record_actual_restricted_and_unrestricted_payment() {
    for (condition,expected) in [("{G} was spent to cast this spell",21),("no mana was spent to cast this spell",20),("exactly two colors of mana were spent to cast this spell",21)] {
        let text=format!("If {condition}, you gain 1 life.");
        let spell=def("Paid","{2}","Instant",&text);
        let source=def("Restricted","","Land","{T}: Add {G}. Spend this mana only to cast a spell.");
        let mut g=Game::new(vec![spell,source],2,20);let land=g.put(0,1,Zone::Battlefield);
        g.act(0,Move::Activate{source:land,ability:0,targets:vec![],paying:vec![]}).unwrap();g.add_mana(0,ManaSymbol::Color(Color::Red));
        let card=g.put(0,0,Zone::Hand);let mut selected=mana_pool("R");selected.restricted=vec![g.players[0].pool.restricted[0].clone()];g.act(0,Move::Paying(selected,Box::new(Move::Cast{card,targets:vec![]}))).unwrap();resolve(&mut g);
        cite!(["601.2h"];case:condition.to_string();assert_eq!(g.players[0].life,expected,"{condition}"));
    }
}
#[test]
fn r400_7c_entry_abilities_can_read_cast_payment_but_later_incarnations_cannot() {
    let permanent=def("Entry","{1}","Artifact","When this artifact enters, if {G} was spent to cast it, you gain 1 life.");
    let mut g=Game::new(vec![permanent],2,20);let card=g.put(0,0,Zone::Hand);g.add_mana(0,ManaSymbol::Color(Color::Green));
    g.act(0,paid("G",Move::Cast{card,targets:vec![]})).unwrap();resolve(&mut g);cite!(["400.7d"];assert_eq!(g.players[0].life,21));
    let artifact=g.battlefield[0];g.move_object(artifact,Zone::Graveyard);let grave=g.players[0].graveyard[0];g.move_object(grave,Zone::Battlefield);
    g.act(g.priority,Move::Pass).unwrap();resolve(&mut g);cite!(["400.7"];assert_eq!(g.players[0].life,21));
}

#[test]
fn r608_2h_native_converge_uses_distinct_paid_colors_and_uncast_copy_zero() {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let together=CardDef::from_faces(&CardDef::catalog(&faces)["Together as One"]).unwrap();
    game::play::check(&together).unwrap();
    // Full pinned converge rulings 9a2b4042e51a0dca (colorless is not a
    // color) and 742fcc8c35435938 (an uncast copy spent no mana).
    for (colors,colorless,x,copy) in [([2,2,0,0,0],2,2,false),([0;5],6,0,false),
                                    ([1;5],1,5,false),([2,2,0,0,0],2,2,true)] {
        let mut g=Game::new(vec![together.clone(),def("Blank","{0}","Artifact",""),
            def("Mirror","{0}","Instant","Copy target sorcery spell.")],2,20);
        for _ in 0..10 {g.put(0,1,Zone::Library);}
        g.players[0].pool.colors=colors;g.players[0].pool.colorless=colorless;
        let spell=g.put(0,0,Zone::Hand);
        g.act(0,Move::Paying(game::state::Pool { colors, colorless, ..Default::default() },Box::new(Move::Cast {card:spell,targets:vec![vec![Target::Player(0)],vec![Target::Player(1)]]}))).unwrap();
        if copy {
            let original=*g.stack.last().unwrap();let mirror=g.put(0,2,Zone::Hand);
            g.act(0,Move::Cast {card:mirror,targets:vec![vec![original.into()]]}).unwrap();
            for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();} // make the copy
            for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();} // resolve uncast copy
            cite!(["601.2h", "707.10"]; assert_eq!((g.players[0].hand.len(),g.players[0].life,g.players[1].life),(0,20,20)));
            assert_eq!(g.stack.len(),1);
        }
        resolve(&mut g);
        cite!(["601.2h", "608.2c", "608.2h"]; assert_eq!((g.players[0].hand.len(),g.players[0].life,g.players[1].life),(x as usize,20+x,20-x)));
    }
}

#[test]
fn r601_2c_target_counts_cannot_read_mana_not_yet_paid() {
    let spell=CardDef::new("Premature payment", "{2}", "Sorcery",
        "Tap up to X target creatures, where X is the number of colors of mana spent to cast this spell.", None).unwrap();
    // Targets precede payment; refusing this unsupported announcement quantity is
    // necessary rather than treating the not-yet-paid source as a zero-color cast.
    cite!(["601.2c", "601.2h"]; assert!(game::play::check(&spell).unwrap_err().0.contains("unavailable before cast payment")));
}
