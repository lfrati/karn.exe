//! Supported synthetic boundary witnesses, not tests of whole native cards.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use game::state::Decision;
use ir::Part;

fn def(n: &str, t: &str, x: &str, p: Option<(i32,i32)>) -> CardDef {
    CardDef::new(n,"{0}",t,x,p).unwrap()
}
fn setup(history: bool, instruction: &str, prohibit: bool) -> (Game,ObjId,ObjId) {
    let watcher = if history {
        "At the beginning of each end step, if you put one or more counters on a creature with power 5 or greater this turn, you gain 1 life."
    } else {
        "Whenever you put one or more counters on a creature with power 5 or greater, you gain 1 life."
    };
    let defs = vec![
        def("Source","Creature", &format!("Wither\nLifelink\n{{0}}: {instruction}"),Some((1,8))),
        def("Bear","Creature","",Some((4,4))),
        def("Anthem","Enchantment","As long as you have 21 or more life, creatures you control get +2/+2.",None),
        def("Watcher","Enchantment",watcher,None),
        def("Prohibition","Enchantment","Players can't gain life.",None),
    ];
    for d in &defs { game::play::check(d).unwrap(); } // Uncited (admission): supported synthetic text.
    let mut g=Game::new(defs,2,20);
    for p in 0..2 { for _ in 0..20 { g.put(p,1,Zone::Library); } }
    let s=g.put(0,0,Zone::Battlefield);
    let b=g.put(0,1,Zone::Battlefield);
    g.put(0,2,Zone::Battlefield); g.put(0,3,Zone::Battlefield);
    if prohibit { g.put(1,4,Zone::Battlefield); }
    (g,s,b)
}
fn resolve(g:&mut Game) { g.act(g.priority,Move::Pass).unwrap(); g.act(g.priority,Move::Pass).unwrap(); }
fn activate(g:&mut Game,s:ObjId,targets:Vec<Vec<Target>>) {
    g.act(0,Move::Activate{source:s,ability:2,targets,paying:vec![]}).unwrap();
    resolve(g);
}
fn snapshots(g:&Game,b:ObjId)->Vec<((i32,i32),usize,u32)> {
    g.log.iter().filter_map(|e| match e {
        Event::CountersPut{on:Target::Object(on),recipient:Some(seen),by,count,..} if *on==b => Some((seen.pt.unwrap(),*by,*count)),
        _ => None,
    }).collect()
}
fn end(g:&mut Game) {
    for _ in 0..100 {
        if g.part==Part::End { return; }
        let mv=match g.decision {
            Some(Decision::Attackers{..})=>Move::Attack(vec![]),
            Some(Decision::Blockers{..})=>Move::Block(vec![]),
            Some(ref d)=>panic!("unexpected {d:?}"), None=>Move::Pass,
        };
        let p=g.asked().unwrap().0; g.act(p,mv).unwrap();
    }
    panic!("end not reached");
}

// Rules: 120.3d, 120.3f, 120.4b, 120.4c, 120.4d, 603.10, 122.6
// Immediate snapshot: 4 - 1 + 2 = 5 after simultaneous lifelink.
#[test]
fn r120_immediate_snapshot_includes_lifelink() {
    let (mut g,s,b)=setup(false,"This creature deals 1 damage to target creature.",false);
    activate(&mut g,s,vec![vec![b.into()]]);
    cite!(["120.3d","120.3f","120.4c","120.4d"];assert_eq!(g.players[0].life,21));
    cite!(["120.3d","120.3f","603.10"];assert_eq!(g.seen(b,None).pt,Some((5,5))));
    cite!(["603.10","122.6"];assert_eq!(snapshots(&g,b),vec![((5,5),0,1)]));
    cite!(["603.10"];assert_eq!(g.stack.len(),1));
    resolve(&mut g);
    cite!(["120.3f","603.10"];assert_eq!(g.players[0].life,22));
}

// Rules: 120.3d, 120.3f, 120.4c, 120.4d, 603.10, 608.2i, 603.4, 400.7
// Historical snapshot, also with departure and a counter-free new incarnation.
#[test]
fn r608_historical_snapshot_survives_departure() {
    for depart in [false,true] {
        let (mut g,s,b)=setup(true,"This creature deals 1 damage to target creature.",false);
        activate(&mut g,s,vec![vec![b.into()]]);
        cite!(["120.3d","120.3f","603.10"];assert_eq!(snapshots(&g,b),vec![((5,5),0,1)]));
        cite!(["603.4"];assert!(g.stack.is_empty()));
        if depart {
            g.move_object(b,Zone::Hand);
            // Remove the anthem too: the new incarnation is only 4/4 and cannot
            // qualify by its current appearance instead of the old completed event.
            let anthem=*g.battlefield.iter().find(|&&o| g.def(o).unwrap().name()=="Anthem").unwrap();
            g.move_object(anthem,Zone::Graveyard);
            let card=*g.players[0].hand.last().unwrap();
            g.act(0,Move::Cast{card,targets:vec![]}).unwrap(); resolve(&mut g);
            let fresh=*g.battlefield.iter().find(|&&o| o!=s && g.def(o).unwrap().name()=="Bear").unwrap();
            cite!(["400.7"];assert_ne!(fresh,b));
            cite!(["400.7"];assert!(g.counters(fresh).is_empty()));
            cite!(["400.7","608.2i"];assert_eq!(g.seen(fresh,None).pt,Some((4,4))));
        }
        end(&mut g);
        cite!(["603.4","608.2i"];assert_eq!(g.stack.len(),1));
        cite!(["608.2i"];assert_eq!(snapshots(&g,b),vec![((5,5),0,1)]));
        resolve(&mut g);
        cite!(["603.4","608.2i"];assert_eq!(g.players[0].life,22));
    }
}

// Rules: 120.3d, 120.3f, 120.4c, 120.4d, 603.10
#[test]
fn r120_simultaneous_recipients_ignore_iteration_order() {
    for reverse in [false,true] {
        let (mut g,s,a)=setup(false,"This creature deals 1 damage to each of up to two target creatures.",false);
        let b=g.put(0,1,Zone::Battlefield);
        let watcher=*g.battlefield.iter().find(|id|g.obj(**id).card==Some(3)).unwrap();
        let targets=if reverse {vec![b.into(),a.into()]} else {vec![a.into(),b.into()]};
        activate(&mut g,s,vec![targets]);
        cite!(["120.3f","120.4d"];assert_eq!(g.players[0].life,22));
        for on in [a,b] {
            cite!(["120.3d","603.10"];assert_eq!(snapshots(&g,on),vec![((5,5),0,1)]));
        }
        let Some(Decision::Order { player, abilities }) = &g.decision else { panic!("expected trigger order: {:?}", g.decision) };
        cite!(["603.3b"]; assert_eq!((*player, abilities.len()), (0, 2)));
        assert!(abilities.iter().all(|trigger| trigger.controller == 0 && trigger.source == Some(watcher)));
        g.act(0, Move::Order(vec![0, 1])).unwrap();
        cite!(["603.10"];assert_eq!(g.stack.len(),2));
    }
}

// Rules: 608.2c, 120.3d, 120.3f, 120.4d, 603.10, 608.2i
// First damage completes at 21 life with 5/5; second at 22 with 4/4.
#[test]
fn r608_sequential_damage_retains_distinct_snapshots() {
    let (mut g,s,b)=setup(false,"This creature deals 1 damage to target creature. Then it deals 1 damage to that creature.",false);
    activate(&mut g,s,vec![vec![b.into()]]);
    cite!(["608.2c","120.3f"];assert_eq!(g.players[0].life,22));
    cite!(["608.2c","120.3d","603.10","608.2i"];assert_eq!(snapshots(&g,b),vec![((5,5),0,1),((4,4),0,1)]));
    cite!(["603.10"];assert_eq!(g.stack.len(),1));
}

// Rules: 101.2, 120.3d, 120.3f, 120.4c, 120.4d, 603.10, 603.4, 608.2i
#[test]
fn r120_prohibited_lifelink_does_not_qualify() {
    for history in [false,true] {
        let (mut g,s,b)=setup(history,"This creature deals 1 damage to target creature.",true);
        activate(&mut g,s,vec![vec![b.into()]]);
        cite!(["101.2","120.3f"];assert_eq!(g.players[0].life,20));
        cite!(["120.3d","603.10"];assert_eq!(snapshots(&g,b),vec![((3,3),0,1)]));
        if history { end(&mut g); }
        cite!(["603.4","603.10","608.2i"];assert!(g.stack.is_empty()));
    }
}
