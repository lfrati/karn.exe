//! Whole-hand looks reuse retained private disclosures; no decision is needed after looking.
use crate::support::cite;
use game::{CardDef, Game, Move, Target, Zone};
use game::simulation::Simulation;
use game::observation::Kind;

fn game() -> Game {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let probe=CardDef::from_faces(CardDef::catalog(&faces).get("Gitaxian Probe").unwrap()).unwrap();
    game::play::check(&probe).unwrap();
    let land=CardDef::new("Island","","Basic Land — Island","",None).unwrap();
    let shield=CardDef::new("Targeting Restriction","{2}{W}{W}","Enchantment","You can't be the target of spells or abilities.",None).unwrap();
    game::play::check(&shield).unwrap();
    let mut g=Game::new(vec![probe,land,shield],3,20);
    for p in 0..3 { for _ in 0..4 { g.put(p,1,Zone::Library); } }
    g
}
fn passes(g:&mut Game) { for _ in 0..3 { g.act(g.priority,Move::Pass).unwrap(); } }

#[test]
fn r701_look_at_target_hand_is_private_and_delivered_after_draw_for_either_payment() {
    for life in [false,true] {
        let mut g=game();let secret=g.put(1,1,Zone::Hand);let card=g.put(0,0,Zone::Hand);
        if !life { g.players[0].pool.colors[1]=1; }
        g.act(0,crate::support::paid(if life { "" } else { "U" }, Move::WithLife(vec![life],Box::new(Move::Cast{card,targets:vec![vec![Target::Player(1)]]})))).unwrap();
        passes(&mut g);
        cite!(["107.4f"]; case: life; assert_eq!(g.players[0].life,if life {18}else{20}));
        cite!(["608.2c"]; case: life; assert_eq!(g.players[0].hand.len(),1));assert_eq!(g.players[0].library.len(),3);
        assert!(g.stack.is_empty());assert!(!g.sees(0,secret));
        let mut sim=Simulation::new(g);let mut clone=sim.clone();
        for s in [&mut sim,&mut clone] {
            let seen=s.take_observations(0).unwrap();assert_eq!(seen.len(),1);
            assert_eq!(seen[0].kind,Kind::Looked);cite!(["608.2c"]; case: life; assert_eq!(seen[0].cards[0].object,secret));
            assert_eq!(seen[0].cards[0].name,"Island");
            cite!(["400.2"]; case: life; assert!(s.take_observations(1).unwrap().is_empty()));assert!(s.take_observations(2).unwrap().is_empty());
            assert!(s.take_observations(0).unwrap().is_empty());
        }
    }
}
#[test]
fn r608_2b_player_target_must_remain_legal_and_empty_hand_still_draws() {
    for shield in [false,true] {
        let mut g=game();g.players[0].pool.colors[1]=1;let card=g.put(0,0,Zone::Hand);
        let cast=crate::support::paid("U",Move::Cast{card,targets:vec![vec![Target::Player(1)]]});
        if shield {
            let mask=g.put(1,2,Zone::Battlefield);cite!(["702.18a"]; refused("an illegal target, that player: that player has shroud (702.18a)"): g.act(0,cast.clone()));
            assert_eq!(g.players[0].pool.colors[1],1);assert_eq!(g.obj(card).zone,Zone::Hand);
            g.move_object(mask,Zone::Graveyard);
        }
        g.act(0,cast).unwrap();if shield {g.put(1,2,Zone::Battlefield);}
        passes(&mut g);
        cite!(["608.2b"]; case: shield; assert_eq!(g.players[0].hand.len(),usize::from(!shield)));
        assert!(g.observations(1).is_empty() && g.observations(2).is_empty());
        if shield {cite!(["608.2b"]; assert!(g.observations(0).is_empty()));}
    }
}
