//! Blocking-only restrictions use effective characteristics and the ordinary combat query.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use game::card::CastAs;
use game::legal::Asked;
use std::sync::OnceLock;
fn game() -> Game {
    static DS:OnceLock<Vec<CardDef>>=OnceLock::new();
    Game::new(DS.get_or_init(|| {
        let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();let catalog=CardDef::catalog(&faces);
        let mut ds=vec![CardDef::from_faces(catalog.get("Brazen Borrower // Petty Theft").or_else(||catalog.get("Brazen Borrower")).unwrap()).unwrap()];
        for (name,ty,text,pt) in [("Ground","Creature — Bear","",Some((2,2))),("Flying creature","Creature — Bird","Flying",Some((2,2))),("Land","Land","",None),("Strip","Instant","Target permanent loses all abilities until end of turn.",None)] {ds.push(CardDef::new(name,"{0}",ty,text,pt).unwrap());}
        for d in &ds {game::play::check(d).unwrap_or_else(|e|panic!("{}: {e:?}",d.name()));}ds
    }).clone(),2,20)
}
fn resolve(g:&mut Game) {for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}}
fn strip(g:&mut Game,target:ObjId) {let spell=g.put(g.priority,4,Zone::Hand);g.act(g.priority,Move::Cast {card:spell,targets:vec![vec![Target::Object(target)]]}).unwrap();resolve(g);}
#[test]
fn r509_1b_borrower_blocks_flying_only_using_current_abilities() {
    let mut g=game();let borrower=g.put(1,0,Zone::Battlefield);let ground=g.put(0,1,Zone::Battlefield);let flyer=g.put(0,2,Zone::Battlefield);
    cite!(["509.1b"]; assert!(g.cant_block(borrower,ground,&[borrower])));cite!(["509.1b"]; assert!(!g.cant_block(borrower,flyer,&[borrower])));
    strip(&mut g,flyer);cite!(["509.1b"]; assert!(g.cant_block(borrower,flyer,&[borrower])));
    strip(&mut g,borrower);cite!(["509.1b"]; assert!(!g.cant_block(borrower,ground,&[borrower])));
}
#[test]
fn r509_1b_blocker_options_and_commands_enforce_the_same_restriction() {
    let mut g=game();let ground=g.put(0,1,Zone::Battlefield);let flyer=g.put(0,2,Zone::Battlefield);let borrower=g.put(1,0,Zone::Battlefield);
    resolve(&mut g);resolve(&mut g);g.act(0,Move::Attack(vec![(ground,Target::Player(1)),(flyer,Target::Player(1))])).unwrap();resolve(&mut g);
    let Some((1,Asked::Blockers {able,..}))=g.asked() else { panic!("expected blockers") };
    cite!(["509.1b"]; assert!(able.iter().any(|(blocker,attackers)| *blocker==borrower && attackers.contains(&flyer))));
    cite!(["509.1b"]; assert!(!able.iter().any(|(blocker,attackers)| *blocker==borrower && attackers.contains(&ground))));
    cite!(["509.1b"]; refused("Brazen Borrower can't block Ground: Brazen Borrower says it can't, or not that way (509.1b)"): g.act(1,Move::Block(vec![(borrower,ground)])));assert!(g.combat.blockers.is_empty());
    g.act(1,Move::Block(vec![(borrower,flyer)])).unwrap();cite!(["509.1g"]; assert_eq!(g.combat.blockers_of(flyer),vec![borrower]));
}
#[test]
fn r715_3_petty_theft_bounces_only_opponents_nonlands_and_borrower_casts_with_flash_from_exile() {
    let mut g=game();g.active=1;g.priority=0;
    let borrower=g.put(0,0,Zone::Hand);let theirs=g.put(1,1,Zone::Battlefield);let mine=g.put(0,1,Zone::Battlefield);let land=g.put(1,3,Zone::Battlefield);
    g.players[0].pool.colors[1]=2;
    let adventure=|target|paid("UU", Move::CastAs(CastAs::Adventure,Box::new(Move::Cast {card:borrower,targets:vec![vec![Target::Object(target)]]})));
    for (target,why) in [(mine,"the target is one an opponent controls, and you control it (115.1)"),(land,"it is a land (115.1)")] {cite!(["115.1"]; case: why; refused(why): g.act(0,adventure(target)));}
    g.act(0,adventure(theirs)).unwrap();resolve(&mut g);assert_eq!(g.players[1].hand.len(),1);assert!(!g.battlefield.contains(&theirs));
    let exiled=g.exile[0];g.players[0].pool.colors[1]=3;g.priority=0;
    g.act(0,paid("UUU", Move::Cast {card:exiled,targets:vec![]})).unwrap();resolve(&mut g);
    let permanent=*g.battlefield.last().unwrap();cite!(["715.4"]; assert_eq!(g.def(permanent).unwrap().name(),"Brazen Borrower"));assert!(g.has(permanent,"Flying"));
}
