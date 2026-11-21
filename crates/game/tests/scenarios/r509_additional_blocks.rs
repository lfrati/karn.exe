//! Additional blocking permissions compose with ordinary declaration, triggers and damage.
use crate::support::cite;
use game::{CardDef,Game,Move,ObjId,Target,Zone};
use game::state::Decision;
use game::legal::Asked;
fn card(name:&str,kind:&str,text:&str,pt:Option<(i32,i32)>)->CardDef {
    let d=CardDef::new(name,"{0}",kind,text,pt).unwrap();game::play::check(&d).unwrap_or_else(|e|panic!("{name}: {}",e.0)); d
}
fn game(extra:&str)->(Game,ObjId,ObjId,ObjId) {
    let mut g=Game::new(vec![card("Attacker","Creature — Bear","",Some((2,2))),card("Blocker","Creature — Giant",extra,Some((3,5))),card("Echo Circlet","Artifact — Equipment","Equipped creature can block an additional creature each combat.\nEquip {1}",None)],2,20);
    let a=g.put(0,0,Zone::Battlefield);let b=g.put(0,0,Zone::Battlefield);let blocker=g.put(1,1,Zone::Battlefield);
    (g,a,b,blocker)
}
fn pass(g:&mut Game){for _ in 0..g.players.len(){g.act(g.priority,Move::Pass).unwrap();}}
fn begin(g:&mut Game,attackers:&[ObjId]) {
    pass(g);pass(g);g.act(0,Move::Attack(attackers.iter().map(|a|(*a,Target::Player(1))).collect())).unwrap();pass(g);
    assert!(matches!(g.decision,Some(Decision::Blockers{player:1})));
}
fn equip(g:&mut Game,blocker:ObjId)->ObjId {let e=g.put(1,2,Zone::Battlefield);assert!(g.attach(e,Target::Object(blocker)));e}
#[test]
fn r509_1a_extra_capacity_stacks_and_tracks_effective_attachments() {
    let (mut g,a,b,blocker)=game("");let idle=g.put(1,0,Zone::Battlefield);
    let e=equip(&mut g,blocker);let f=equip(&mut g,blocker);
    // Uncited (card text): "Equipped creature can block an additional creature each combat.": one more than 509.1a's one, for each Circlet
    assert_eq!(g.block_capacity(blocker),3);
    assert!(g.attach(e,Target::Object(idle)));cite!(["301.5f"]; assert_eq!(g.block_capacity(blocker),2));cite!(["301.5f"]; assert_eq!(g.block_capacity(idle),2));
    g.move_object(f,Zone::Graveyard);cite!(["611.3b"]; assert_eq!(g.block_capacity(blocker),1));
    begin(&mut g,&[a,b]);
    cite!(["509.1a"]; refused("that creature cannot block that many attacking creatures (509.1a)"): g.act(1,Move::Block(vec![(blocker,a),(blocker,b)])));
    assert!(g.combat.blockers.is_empty());
}
#[test]
fn r510_1d_a_multi_blocker_divides_one_power_total_and_resumes_on_a_clone() {
    let (mut g,a,b,blocker)=game("");equip(&mut g,blocker);begin(&mut g,&[a,b]);
    let Some((1,Asked::Blockers{capacity,..}))=g.asked() else {panic!("expected blockers")};assert!(capacity.contains(&(blocker,2)));
    cite!(["509.1a"]; refused("the same block cannot be declared twice (509.1a)"): g.act(1,Move::Block(vec![(blocker,a),(blocker,a)])));
    g.act(1,Move::Block(vec![(blocker,a),(blocker,b)])).unwrap();pass(&mut g);
    cite!(["510.1d"]; assert!(matches!(g.decision,Some(Decision::Divide{player:1,attacker,amount:3,..}) if attacker==blocker)));
    let copy=g.clone();
    for mut game in [g,copy] {
        cite!(["510.1"]; refused("that decision is another player's"): game.act(0,Move::Divide(vec![(Target::Object(a),2),(Target::Object(b),1)])));
        cite!(["510.1a"]; refused("it assigns combat damage equal to its power, 3 (510.1a)"): game.act(1,Move::Divide(vec![(Target::Object(a),3),(Target::Object(b),3)])));
        game.act(1,Move::Divide(vec![(Target::Object(a),2),(Target::Object(b),1)])).unwrap();
        assert!(!game.battlefield.contains(&a));assert!(game.battlefield.contains(&b));cite!(["510.1d"]; assert_eq!(game.obj(b).damage,1));
        cite!(["510.1c"]; assert_eq!(game.obj(blocker).damage,4));assert_eq!(game.players[1].life,20);
    }
}
#[test]
fn r509_3a_unqualified_blocks_triggers_once_and_qualified_blocks_for_each_creature() {
    for (text,expected) in [("Whenever this creature blocks, you gain 1 life.",1),("Whenever this creature blocks a creature, you gain 1 life.",2)] {
        let (mut g,a,b,blocker)=game(text);equip(&mut g,blocker);begin(&mut g,&[a,b]);
        g.act(1,Move::Block(vec![(blocker,a),(blocker,b)])).unwrap();
        if let Some(Decision::Order{player,..})=g.decision {g.act(player,Move::Order(vec![0,1])).unwrap();}
        cite!(["509.3a","509.3b"]; case: text; assert_eq!(g.stack.len(),expected));
        for _ in 0..expected {pass(&mut g);}
        assert_eq!(g.players[1].life,20+expected as i32);
    }
}
#[test]
fn r510_1d_a_departed_attacker_is_no_longer_a_damage_recipient() {
    let (mut g,a,b,blocker)=game("");equip(&mut g,blocker);begin(&mut g,&[a,b]);
    g.act(1,Move::Block(vec![(blocker,a),(blocker,b)])).unwrap();g.move_object(a,Zone::Graveyard);pass(&mut g);
    cite!(["510.1d"]; assert!(g.decision.is_none()));assert!(!g.battlefield.contains(&b));cite!(["506.4","510.1c"]; assert_eq!(g.obj(blocker).damage,2));
}
#[test]
fn r510_4_first_strike_uses_the_same_multi_block_damage_assignment() {
    let (mut g,a,b,blocker)=game("First strike");equip(&mut g,blocker);begin(&mut g,&[a,b]);
    g.act(1,Move::Block(vec![(blocker,a),(blocker,b)])).unwrap();pass(&mut g);
    g.act(1,Move::Divide(vec![(Target::Object(a),2),(Target::Object(b),1)])).unwrap();
    cite!(["510.4"]; assert_eq!(g.obj(blocker).damage,0));pass(&mut g);
    cite!(["510.4"]; assert_eq!(g.obj(blocker).damage,2));assert!(g.battlefield.contains(&b));
}
#[test]
fn r509_1a_unlimited_and_temporary_permissions_use_the_same_query() {
    let (mut g,a,b,blocker)=game("This creature can block any number of creatures.");let c=g.put(0,0,Zone::Battlefield);begin(&mut g,&[a,b,c]);
    g.act(1,Move::Block(vec![(blocker,a),(blocker,b),(blocker,c)])).unwrap();
    let spell=card("Permission","Instant","Target creature can block an additional creature this turn.",None);
    let mut g=Game::new(vec![spell,card("Bear","Creature — Bear","",Some((2,2)))],2,20);
    let creature=g.put(0,1,Zone::Battlefield);let spell=g.put(0,0,Zone::Hand);
    g.act(0,Move::Cast{card:spell,targets:vec![vec![Target::Object(creature)]]}).unwrap();pass(&mut g);
    // Uncited (card text): "Target creature can block an additional creature this turn.": one more than 509.1a's one
    assert_eq!(g.block_capacity(creature),2);
}

#[test]
fn r702_19b_trampling_attackers_share_the_lethal_assignment_to_one_blocker() {
    let mut g=Game::new(vec![card("Attacker","Creature — Beast","Trample",Some((3,3))),card("Blocker","Creature — Giant","This creature can block an additional creature each combat.",Some((1,3)))],2,20);
    let a=g.put(0,0,Zone::Battlefield);let b=g.put(0,0,Zone::Battlefield);let blocker=g.put(1,1,Zone::Battlefield);
    begin(&mut g,&[a,b]);g.act(1,Move::Block(vec![(blocker,a),(blocker,b)])).unwrap();pass(&mut g);
    g.act(0,Move::Divide(vec![(Target::Object(blocker),2),(Target::Player(1),1)])).unwrap();
    g.act(0,Move::Divide(vec![(Target::Object(blocker),1),(Target::Player(1),2)])).unwrap();
    assert!(matches!(g.decision,Some(Decision::Divide{player:1,..})));
    cite!(["510.1","510.2"]; assert_eq!(g.players[1].life,20,"damage waits until every player assigned"));
    g.act(1,Move::Divide(vec![(Target::Object(a),1)])).unwrap();
    cite!(["702.19b"]; assert_eq!(g.players[1].life,17));assert!(!g.battlefield.contains(&blocker));assert_eq!(g.obj(a).damage,1);
}

/// 510.1e: once a player has assigned the damage of each creature they control, their total
/// is checked; an illegal one returns the game to the moment before that player began, and
/// the next player is not asked to assign after it.
#[test]
fn r510_1e_the_active_players_total_is_checked_before_the_defending_player_assigns() {
    let mut g=Game::new(vec![card("Attacker","Creature — Beast","Trample",Some((3,3))),card("Blocker","Creature — Giant","This creature can block an additional creature each combat.",Some((1,3)))],2,20);
    let a=g.put(0,0,Zone::Battlefield);let b=g.put(0,0,Zone::Battlefield);let blocker=g.put(1,1,Zone::Battlefield);
    begin(&mut g,&[a,b]);g.act(1,Move::Block(vec![(blocker,a),(blocker,b)])).unwrap();pass(&mut g);
    // 510.1: the active player assigns first; the blocker's controller divides its damage after
    assert!(matches!(g.decision,Some(Decision::Divide{player:0,attacker,..}) if attacker==a));
    let began=format!("{g:?}");
    g.act(0,Move::Divide(vec![(Target::Object(blocker),1),(Target::Player(1),2)])).unwrap();
    // 2 of a lethal 3 assigned to the blocker, and both trample over it (702.19b)
    cite!(["510.1e","702.19b"]; refused("Attacker assigns damage to what it attacks once Blocker is assigned lethal damage (702.19b); the assignment begins again (510.1e)"): g.act(0,Move::Divide(vec![(Target::Object(blocker),1),(Target::Player(1),2)])));
    cite!(["510.1e"]; assert!(matches!(g.decision,Some(Decision::Divide{player:0,attacker,..}) if attacker==a),"{:?}",g.decision));
    cite!(["510.1e"]; assert_eq!(format!("{g:?}"),began,"the moment before the active player began to assign"));
    g.act(0,Move::Divide(vec![(Target::Object(blocker),2),(Target::Player(1),1)])).unwrap();
    g.act(0,Move::Divide(vec![(Target::Object(blocker),1),(Target::Player(1),2)])).unwrap();
    cite!(["510.1"]; assert!(matches!(g.decision,Some(Decision::Divide{player:1,attacker,..}) if attacker==blocker)));
    g.act(1,Move::Divide(vec![(Target::Object(b),1)])).unwrap();
    cite!(["510.2","702.19b"]; assert_eq!(g.players[1].life,17));assert!(!g.battlefield.contains(&blocker));assert_eq!(g.obj(b).damage,1);
}

#[test]
fn r509_1a_removing_the_permission_after_declaration_does_not_undo_legal_blocks() {
    let (mut g,a,b,blocker)=game("");let third=g.put(0,0,Zone::Battlefield);let e=equip(&mut g,blocker);begin(&mut g,&[a,b,third]);
    cite!(["509.1a"]; refused("that creature cannot block that many attacking creatures (509.1a)"): g.act(1,Move::Block(vec![(blocker,a),(blocker,b),(blocker,third)])));
    g.act(1,Move::Block(vec![(blocker,a),(blocker,b)])).unwrap();g.move_object(e,Zone::Graveyard);
    cite!(["611.3b"]; assert_eq!(g.block_capacity(blocker),1));cite!(["506.4a"]; assert_eq!(g.combat.blockers.len(),2));pass(&mut g);
    cite!(["510.1d"]; assert!(matches!(g.decision,Some(Decision::Divide{player:1,amount:3,..}))));
}
