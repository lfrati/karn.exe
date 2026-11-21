//! Produced-mana delayed triggers (106.6a, 603.7e), full Path of Ancestry.
use crate::support::cite;
use game::{CardDef,Game,Move,Zone,ObjId};
use game::state::{Answer,Pool};
use ir::Color;
fn def(name:&str,cost:&str,types:&str,text:&str,pt:Option<(i32,i32)>)->CardDef {CardDef::new(name,cost,types,text,pt).unwrap()}
fn setup()->(Game,ObjId,ObjId) {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let path=CardDef::from_face(faces.iter().find(|f|f.name=="Path of Ancestry").unwrap()).unwrap();
    game::play::check(&path).unwrap();
    let mut g=Game::new(vec![path,def("Leader","{R}","Legendary Creature — Goblin","",Some((2,2))),
        def("Shape","{R}","Creature — Shapeshifter","Changeling",Some((2,2))),
        def("Bear","{R}","Creature — Bear","",Some((2,2))),
        def("Blank","","Basic Land — Mountain","",None),def("Take","{0}","Instant","Gain control of target creature.",None)],3,20);
    let leader=g.put(0,1,Zone::Command);g.designate_commander(leader).unwrap();
    let path=g.put(0,0,Zone::Battlefield);g.set_tapped(path,false);
    for _ in 0..5 {g.put(0,4,Zone::Library);}
    (g,path,leader)
}
fn produce(g:&mut Game,path:ObjId) {g.act(0,Move::Activate{source:path,ability:1,targets:vec![],paying:vec![]}).unwrap();}
// A caller names exact authenticated units; this helper neither scans nor allocates mana.
fn marked(g:&Game, indices:&[usize]) -> Pool {
    Pool { restricted: indices.iter().map(|i|g.players[0].pool.restricted[*i].clone()).collect(), ..Pool::default() }
}
fn pass(g:&mut Game) {for _ in 0..3{g.act(g.priority,Move::Pass).unwrap();}}
#[test]
fn r106_6a_full_path_marked_mana_creates_scry_trigger_after_completed_changeling_cast() {
    let (mut g,path,_)=setup();produce(&mut g,path);assert_eq!(g.players[0].pool.restricted.len(),1);
    let card=g.put(0,2,Zone::Hand);g.act(0,Move::Paying(marked(&g,&[0]),Box::new(Move::Cast{card,targets:vec![]}))).unwrap();
    cite!(["106.6"]; assert_eq!(g.stack.len(),2));pass(&mut g);
    assert!(matches!(g.decision,Some(game::state::Decision::Choose{player:0,choice:game::state::Choice::Arrange{..}})));
}
/// What each seat is shown of this ability on the stack: its kind, text and creator.
fn shown(g:&Game,ability:ObjId)->Vec<(game::prompt::OccurrenceKind,Option<String>,Option<String>)> {
    (0..g.players.len()).map(|seat|{let p=game::simulation::Simulation::new(g.clone()).prompt(seat).unwrap();let o=p.entities[&ability].occurrence.clone().unwrap();(o.kind,o.text,o.created_by)}).collect()
}
/// The mana ability creates a delayed triggered ability that triggers when that mana is spent
/// (106.6, 603.7a), one for each mana (106.6a). Once triggered, each has "the text of the
/// ability that created it" (603.3, 405.4): the "When that mana is spent" sentence alone, said
/// as Path of Ancestry's Oracle text says it, with the mana ability that created it beside it,
/// whole; still so once its source has left the battlefield (113.7a, 603.7c).
#[test]
fn r106_6a_a_spent_manas_delayed_ability_has_its_sentence_as_text_and_the_mana_ability_as_creator() {
    let path_text="When that mana is spent to cast a creature spell that shares a creature type with your commander, scry 1.";
    let path_line=format!("{{T}}: Add one mana of any color in your commander's color identity. {path_text}");
    for leaves in [false,true] {
        let (mut g,path,_)=setup();produce(&mut g,path);if leaves {g.move_object(path,Zone::Graveyard);}
        let card=g.put(0,2,Zone::Hand);g.act(0,Move::Paying(marked(&g,&[0]),Box::new(Move::Cast{card,targets:vec![]}))).unwrap();
        let scry=*g.stack.last().unwrap();
        for (seat,(kind,text,by)) in shown(&g,scry).into_iter().enumerate() {
            cite!(["603.7"]; case: format!("leaves={leaves} p{seat}"); assert_eq!(kind,game::prompt::OccurrenceKind::Delayed));
            cite!(["603.3","405.4","106.6"]; case: format!("leaves={leaves} p{seat}"); assert_eq!(text.as_deref(),Some(path_text)));
            cite!(["603.7a","106.6","113.7a"]; case: format!("leaves={leaves} p{seat}"); assert_eq!(by.as_deref(),Some(path_line.as_str())));
        }
    }
    let mana=def("Producer","","Land","{T}: Add {R}{R}. When that mana is spent to cast a creature spell, scry 1.",None);
    game::play::check(&mana).unwrap();
    let watcher=def("Watcher","{0}","Enchantment","Whenever you cast a creature spell, you gain 1 life.",None);
    game::play::check(&watcher).unwrap();
    let mut g=Game::new(vec![mana,def("Creature","{R}{R}","Creature — Bear","",Some((2,2))),def("Blank","","Basic Land — Mountain","",None),watcher],3,20);
    let source=g.put(0,0,Zone::Battlefield);g.put(0,3,Zone::Battlefield);g.act(0,Move::Activate{source,ability:0,targets:vec![],paying:vec![]}).unwrap();
    for _ in 0..3{g.put(0,2,Zone::Library);}let card=g.put(0,1,Zone::Hand);
    g.act(0,Move::Paying(marked(&g,&[0,1]),Box::new(Move::Cast{card,targets:vec![]}))).unwrap();
    // its controller orders them with Watcher's as they wait to be put on the stack (603.3b)
    assert!(matches!(g.decision,Some(game::state::Decision::Order{..})),"{:?}",g.decision);
    let p=game::simulation::Simulation::new(g.clone()).prompt(0).unwrap();
    let ordered=p.decision.as_ref().unwrap().ordered.clone();assert_eq!(ordered.len(),3);
    let scry_text="When that mana is spent to cast a creature spell, scry 1.";
    let mut shown_order=Vec::new();
    for o in &ordered {
        if o.kind==game::prompt::OccurrenceKind::Delayed {
            cite!(["603.3","106.6a"]; assert_eq!(o.text.as_deref(),Some(scry_text)));
            cite!(["603.7a","106.6a"]; assert_eq!(o.created_by.as_deref(),Some("{T}: Add {R}{R}. When that mana is spent to cast a creature spell, scry 1.")));
        } else {
            cite!(["603.3"]; assert_eq!((o.text.as_deref(),o.created_by.as_deref()),(Some("Whenever you cast a creature spell, you gain 1 life."),None)));
        }
        shown_order.push(o.kind);
    }
    cite!(["106.6a"]; assert_eq!(shown_order.iter().filter(|k|**k==game::prompt::OccurrenceKind::Delayed).count(),2));
    let watched=ordered.iter().position(|o|o.kind!=game::prompt::OccurrenceKind::Delayed).unwrap();
    let mut order:Vec<usize>=(0..3).filter(|i|*i!=watched).collect();order.insert(0,watched);
    g.act(0,Move::Order(order)).unwrap();
    assert_eq!(g.stack.len(),4);
    for scry in g.stack[2..].to_vec() {
        for (seat,(_,text,by)) in shown(&g,scry).into_iter().enumerate() {
            cite!(["603.3","106.6a"]; case: format!("p{seat}"); assert_eq!(text.as_deref(),Some("When that mana is spent to cast a creature spell, scry 1.")));
            cite!(["603.7a","106.6a"]; case: format!("p{seat}"); assert_eq!(by.as_deref(),Some("{T}: Add {R}{R}. When that mana is spent to cast a creature spell, scry 1.")));
        }
    }
}
#[test]
fn r106_6_identical_plain_mana_can_be_selected_without_consuming_marked_mana() {
    let (mut g,path,_)=setup();produce(&mut g,path);g.players[0].pool.colors[Color::Red as usize]=1;
    let card=g.put(0,2,Zone::Hand);let snapshot=g.players[0].pool.clone();
    let mut invalid=Pool::default();invalid.colors[Color::Green as usize]=1;
    cite!(["118.3a"]; refused("that mana is not in your mana pool (118.3a)"): g.act(0,Move::Paying(invalid,Box::new(Move::Cast{card,targets:vec![]}))));
    assert_eq!(g.players[0].pool,snapshot);assert!(g.stack.is_empty());
    let mut marked=g.clone();let mut payment=Pool::default();payment.colors[Color::Red as usize]=1;
    g.act(0,Move::Paying(payment,Box::new(Move::Cast{card,targets:vec![]}))).unwrap();
    cite!(["106.6"]; assert_eq!(g.stack.len(),1));cite!(["118.3a"]; assert_eq!(g.players[0].pool.restricted.len(),1));
    let mut payment=Pool::default();payment.restricted.push(marked.players[0].pool.restricted[0].clone());
    marked.act(0,Move::Paying(payment,Box::new(Move::Cast{card,targets:vec![]}))).unwrap();cite!(["106.6"]; assert_eq!(marked.stack.len(),2));
}
#[test]
fn r603_7e_spending_subscription_survives_source_departure_and_cloning() {
    let (mut g,path,_)=setup();produce(&mut g,path);g.move_object(path,Zone::Graveyard);
    let card=g.put(0,2,Zone::Hand);
    // Uncited (ruling 026a4f6bbb331c5d): Resurrection Orb's delayed trigger "triggers even if Resurrection Orb is no longer on the battlefield" (also Jaya 09a5ff6e4c8f2c3e); 603.7d supports it (a spell source has always left first), and 603.7c limits only what it affects; 405.6a and 610.2 (an effect creates it, to act later) support it without saying so. The clone branch is (implementation).
    for mut branch in [g.clone(),g] {branch.act(0,Move::Paying(marked(&branch,&[0]),Box::new(Move::Cast{card,targets:vec![]}))).unwrap();assert_eq!(branch.stack.len(),2);pass(&mut branch);assert!(branch.decision.is_some());}
}
#[test]
fn r106_6_nonmatching_creature_and_noncreature_spending_consumes_mana_without_trigger() {
    for creature in [true,false] {
        let (mut g,path,_)=setup();
        if !creature {g=Game::new(vec![g.defs[0].clone(),g.defs[1].clone(),def("Spell","{R}","Sorcery","You gain 1 life.",None)],3,20);let leader=g.put(0,1,Zone::Command);g.designate_commander(leader).unwrap();}
        let source=if creature {path} else {let o=g.put(0,0,Zone::Battlefield);g.set_tapped(o,false);o};
        produce(&mut g,source);let card=g.put(0,if creature {3}else{2},Zone::Hand);
        // Uncited (card text): "When that mana is spent to cast a creature spell that shares a creature type with your commander": a Bear or a sorcery does not match.
        g.act(0,Move::Paying(marked(&g,&[0]),Box::new(Move::Cast{card,targets:vec![]}))).unwrap();assert_eq!(g.stack.len(),1);cite!(["118.3a"]; case: format!("creature={creature}"); assert_eq!(g.players[0].pool.total(),0));
    }
}
#[test]
fn r903_3_current_commander_incarnation_is_used_in_every_zone_independent_of_control() {
    for zone in [Zone::Command,Zone::Hand,Zone::Library,Zone::Graveyard,Zone::Exile,Zone::Battlefield] {
        let (mut g,path,leader)=setup();if zone!=Zone::Command {g.move_object(leader,zone);}
        if matches!(zone,Zone::Graveyard|Zone::Exile) {g.state_based_actions();g.act(0,Move::Choose(Answer::No)).unwrap();}
        if zone==Zone::Battlefield {let now=*g.battlefield.last().unwrap();let take=g.put(1,5,Zone::Hand);g.act(0,Move::Pass).unwrap();g.act(1,Move::Cast{card:take,targets:vec![vec![game::Target::Object(now)]]}).unwrap();pass(&mut g);assert_eq!(g.controller(now),1);}
        produce(&mut g,path);let card=g.put(0,1,Zone::Hand);
        g.act(0,Move::Paying(marked(&g,&[0]),Box::new(Move::Cast{card,targets:vec![]}))).unwrap();cite!(["903.3"]; case: format!("{zone:?}"); assert_eq!(g.stack.len(),2,"{zone:?}"));
    }
}
#[test]
fn r106_6a_two_mana_create_two_distinct_scry_one_occurrences() {
    let mana=def("Producer","","Land","{T}: Add {R}{R}. When that mana is spent to cast a creature spell, scry 1.",None);
    game::play::check(&mana).unwrap();
    let mut g=Game::new(vec![mana,def("Creature","{R}{R}","Creature — Bear","",Some((2,2))),def("Blank","","Basic Land — Mountain","",None)],3,20);
    let source=g.put(0,0,Zone::Battlefield);g.act(0,Move::Activate{source,ability:0,targets:vec![],paying:vec![]}).unwrap();assert_eq!(g.players[0].pool.restricted.len(),2);
    for _ in 0..3{g.put(0,2,Zone::Library);}let card=g.put(0,1,Zone::Hand);
    g.act(0,Move::Paying(marked(&g,&[0,1]),Box::new(Move::Cast{card,targets:vec![]}))).unwrap();
    if matches!(g.decision,Some(game::state::Decision::Order{..})) {g.act(0,Move::Order(vec![0,1])).unwrap();}
    cite!(["106.6"]; assert_eq!(g.stack.len(),3));
    for _ in 0..2 {pass(&mut g);let top=*g.players[0].library.last().unwrap();g.act(0,Move::Choose(Answer::Arrange{top:vec![top],away:vec![]})).unwrap();}
    assert_eq!(g.stack.len(),1);
}
#[test]
fn r733_1_failed_announcements_reverse_with_produced_mana_without_spurious_triggers() {
    use game::simulation::Simulation;
    let mut g=Game::new(vec![
        def("Producer","{0}","Creature — Elf","{T}: Put a +1/+1 counter on this creature. Add X mana of any one color, where X is the number of +1/+1 counters on this creature. When that mana is spent to cast a creature spell, scry 1.",Some((1,1))),
        def("Creature","{4}","Creature — Goblin","",Some((2,2))),
        def("Scales","{0}","Enchantment","If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.",None),
        def("Evolution","{0}","Enchantment","If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on it instead.",None)],3,20);
    let source=g.put(0,0,Zone::Battlefield);g.put(0,2,Zone::Battlefield);let evolution=g.put(0,3,Zone::Battlefield);g.turn+=1;
    let card=g.put(0,1,Zone::Hand);let mut sim=Simulation::new(g);
    let answer=|sim:&mut Simulation,mv|{let r=sim.request().unwrap().unwrap().clone();sim.respond(r.id,r.player,mv).unwrap();};
    let plan=vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![], source,ability:0,paying:vec![],adding:vec![vec![ir::mana::ManaSymbol::Color(Color::Red);3]]}];
    answer(&mut sim,Move::WithMana(plan,Box::new(Move::Cast{card,targets:vec![]})));
    let Some(game::state::Decision::Choose{choice:game::state::Choice::Replacement{among,..},..})=&sim.game().unwrap().decision else {panic!("replacement choice")};
    let effect=among.iter().find(|e|e.source==evolution).unwrap().clone();
    answer(&mut sim,Move::Choose(Answer::Replacement(effect)));
    assert!(matches!(sim.game().unwrap().decision,Some(game::state::Decision::Payment{..})));
    let mut reversed=sim.clone();answer(&mut reversed,Move::ReversePayment);
    let g=reversed.game().unwrap();cite!(["733.1"]; assert!(g.players[0].hand.contains(&card)));cite!(["733.1"]; assert!(g.stack.is_empty()));assert_eq!(g.players[0].pool.restricted.len(),3);assert!(g.obj(source).tapped);
}
#[test]
fn r106_6_commander_types_are_checked_after_spending_not_fixed_at_production() {
    let (base,_,_)=setup();let mut defs=base.defs.as_ref().clone();defs.push(def("Frog","{0}","Instant","Target creature becomes a Frog creature until end of turn.",None));
    let mut g=Game::new(defs,3,20);let leader=g.put(0,1,Zone::Command);g.designate_commander(leader).unwrap();g.move_object(leader,Zone::Battlefield);let leader=*g.battlefield.last().unwrap();
    let path=g.put(0,0,Zone::Battlefield);g.set_tapped(path,false);produce(&mut g,path);
    let change=g.put(0,6,Zone::Hand);g.act(0,Move::Cast{card:change,targets:vec![vec![game::Target::Object(leader)]]}).unwrap();pass(&mut g);
    let card=g.put(0,1,Zone::Hand);g.act(0,Move::Paying(marked(&g,&[0]),Box::new(Move::Cast{card,targets:vec![]}))).unwrap();cite!(["205.1a"]; assert_eq!(g.stack.len(),1,"former Goblin commander is now only a Frog"));
}
#[test]
fn r106_6a_unbound_or_unsupported_spending_triggers_remain_refused() {
    for text in ["When that mana is spent to cast a creature spell, scry 1.","Add {R}. When that mana is spent to activate an ability, scry 1.","Add {R}. Add {G}. When that mana is spent to cast a creature spell, scry 1.","Add {R}. When any of that mana is spent to cast a creature spell, scry 1."] {
        if text.contains("any of") {
            crate::support::refused(CardDef::new("Refused","{0}","Instant",text,None),"text the grammar does not read");
        } else {
            let d=CardDef::new("Refused","{0}","Instant",text,None).unwrap();
            let reason=if text.starts_with("When") {"spending trigger: no immediately preceding mana production"}
                else if text.contains("activate") {"spending trigger: only completed casts are supported"}
                else {"spending trigger: ambiguous produced mana reference"};
            crate::support::refused(game::play::check(&d).map_err(|e|e.0),reason);
        }
    }
}
#[test]
fn r106_6_changeling_commander_and_casting_the_commander_share_effective_types() {
    for changeling in [true,false] {
        let (base,_,_)=setup();let mut defs=base.defs.as_ref().clone();
        if changeling {defs[1]=def("Leader","{R}","Legendary Creature — Shapeshifter","Changeling",Some((2,2)));}
        let mut g=Game::new(defs,3,20);let leader=g.put(0,1,Zone::Command);g.designate_commander(leader).unwrap();
        let source=g.put(0,0,Zone::Battlefield);g.set_tapped(source,false);produce(&mut g,source);
        let card=if changeling {g.put(0,3,Zone::Hand)}else{leader};
        g.act(0,Move::Paying(marked(&g,&[0]),Box::new(Move::Cast{card,targets:vec![]}))).unwrap();
        if changeling {cite!(["702.73a"]; assert_eq!(g.stack.len(),2));}
        // Uncited (ruling): "If you cast your commander with mana from Path of Ancestry ... you'll scry 1."
        else {assert_eq!(g.stack.len(),2);}
    }
}
#[test]
fn r903_4f_full_path_enters_tapped_and_no_identity_never_produces_colorless() {
    for commander in [None,Some("{0}"),Some("{R}")] {
        let (base,_,_)=setup();let mut defs=base.defs.as_ref().clone();
        defs[1]=def("Leader",commander.unwrap_or("{0}"),"Legendary Creature","",Some((2,2)));
        let mut g=Game::new(defs,3,20);
        if commander.is_some() {let leader=g.put(0,1,Zone::Command);g.designate_commander(leader).unwrap();}
        let card=g.put(0,0,Zone::Hand);g.act(0,Move::PlayLand(card)).unwrap();let source=*g.battlefield.last().unwrap();assert!(g.obj(source).tapped);
        cite!(["107.5"]; refused("Path of Ancestry can't pay {T} as it is (107.5, 118.3)"): g.act(0,Move::Activate{source,ability:1,targets:vec![],paying:vec![]}));g.set_tapped(source,false);
        produce(&mut g,source);cite!(["903.4f", "106.5"]; case: format!("{commander:?}"); assert_eq!(g.players[0].pool.total(),u32::from(commander==Some("{R}"))));assert_eq!(g.players[0].pool.colorless,0);
        if commander==Some("{R}") {let card=g.put(0,2,Zone::Hand);g.act(0,Move::Paying(marked(&g,&[0]),Box::new(Move::Cast{card,targets:vec![]}))).unwrap();assert_eq!(g.stack.len(),1,"a commander with no creature types cannot share one");}
    }
}
