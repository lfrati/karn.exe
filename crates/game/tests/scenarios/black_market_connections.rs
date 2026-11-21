//! Full choose-one-or-more trigger: printed order, life loss and token-defining changeling.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, Zone};
use game::state::Decision;
use ir::{Part, Subtype};
use ir::vocab::SubtypeKind;
fn setup() -> Game {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let market=CardDef::from_face(faces.iter().find(|f|f.face_name=="Black Market Connections").unwrap()).unwrap();
    let forest=CardDef::new("Forest","","Basic Land — Forest","",None).unwrap();
    for d in [&market,&forest] {game::play::check(d).unwrap();}
    let mut g=Game::new(vec![market,forest],3,20);g.put(0,0,Zone::Battlefield);
    for p in 0..3 {for _ in 0..16 {g.put(p,1,Zone::Library);}}
    g
}
fn next_main(g: &mut Game) {
    for _ in 0..250 {
        if matches!(g.decision,Some(Decision::Modes{player:0,..})) {return;}
        match g.decision {Some(Decision::Attackers{player})=>g.act(player,Move::Attack(vec![])).unwrap(),_=>g.act(g.priority,Move::Pass).unwrap()};
    }
    panic!("did not offer modes");
}
fn pass(g: &mut Game) {for _ in 0..3 {g.act(g.priority,Move::Pass).unwrap();}}
#[test]
fn r700_2_all_seven_nonempty_subsets_execute_only_chosen_modes_in_printed_order() {
    for bits in 1..8 {
        let mut g=setup();next_main(&mut g);let before=g.players[0].hand.len();
        let chosen:Vec<_>=(0..3).filter(|n|bits&(1<<n)!=0).collect();
        let mut reverse=chosen.clone();reverse.reverse();
        g.act(0,Move::Modes(reverse,Box::new(Move::ChooseTargets(vec![])))).unwrap();pass(&mut g);
        let life:u32=chosen.iter().map(|n|n+1).sum::<usize>() as u32;
        cite!(["119.3"]; case: format!("{bits}"); assert_eq!(g.players[0].life,20-life as i32,"bits {bits}"));
        assert_eq!(g.players[0].hand.len(),before+usize::from(bits&2!=0));
        let tokens:Vec<_>=g.battlefield.iter().filter(|o|g.obj(**o).token()).copied().collect();
        assert_eq!(tokens.len(),usize::from(bits&1!=0)+usize::from(bits&4!=0));
        for token in tokens {
            assert!(!g.obj(token).tapped);
            if g.obj(token).chars.name.contains("Treasure") {cite!(["111.10a"]; case: format!("{bits}"); assert!(g.is_a(token,ir::CardType::Artifact)));}
            else {
                assert_eq!((g.power(token),g.toughness(token)),(Some(3),Some(2)));assert!(g.has(token,"Changeling"));
                assert!(g.seen(token,None).colors.is_empty());
                cite!(["702.73a"]; case: format!("{bits}"); assert_eq!(g.seen(token,None).subtypes.len(),Subtype::of_kind(SubtypeKind::Creature).len()));
            }
        }
        let losses:Vec<_>=g.log.iter().filter_map(|e|if let Event::LifeLost{player:0,amount}=e{Some(*amount)}else{None}).collect();
        cite!(["608.2c"]; case: format!("{bits}"); assert_eq!(losses,chosen.iter().map(|n|(*n+1)as u32).collect::<Vec<_>>()));
    }
}
#[test]
fn r700_2d_rejected_mode_answers_leave_choice_and_effects_unchanged_and_clone_resumes_once() {
    let mut g=setup();next_main(&mut g);let before=format!("{g:?}");
    crate::support::refused(g.act(1,Move::Modes(vec![0],Box::new(Move::ChooseTargets(vec![])))), "that decision is another player's");assert_eq!(format!("{g:?}"),before);
    for (rule,modes,why) in [("700.2",vec![],"choose 1 to 3 modes of 3 (700.2)"),("700.2d",vec![0,0],"choose 1 to 3 distinct modes of 3 (700.2d)"),("700.2",vec![3],"choose 1 to 3 modes of 3 (700.2)")] {
        cite!([rule]; case: format!("{modes:?}"); refused(why): g.act(0,Move::Modes(modes,Box::new(Move::ChooseTargets(vec![])))));assert_eq!(format!("{g:?}"),before);
    }
    let mut clone=g.clone();
    for game in [&mut g,&mut clone] {game.act(0,Move::Modes(vec![0,1,2],Box::new(Move::ChooseTargets(vec![])))).unwrap();pass(game);}
    assert_eq!(format!("{g:?}"),format!("{clone:?}"));
}
#[test]
fn r118_2_life_loss_is_an_effect_and_does_not_require_enough_life_to_choose_modes() {
    let mut g=setup();next_main(&mut g);g.lose_life(0,19).unwrap();
    g.act(0,Move::Modes(vec![0,1,2],Box::new(Move::ChooseTargets(vec![])))).unwrap();pass(&mut g);
    cite!(["119.3"]; assert_eq!(g.players[0].life,-5));cite!(["704.5a"]; assert!(g.players[0].lost));
    cite!(["704.3"]; assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Created{..})).count(),2,"all selected modes execute before state-based elimination"));
}
#[test]
fn r603_2_first_main_triggers_only_on_its_controllers_precombat_main() {
    let mut g=setup();next_main(&mut g);g.act(0,Move::Modes(vec![0],Box::new(Move::ChooseTargets(vec![])))).unwrap();pass(&mut g);
    for _ in 0..250 {
        if g.active==0 && g.turn>1 && matches!(g.decision,Some(Decision::Modes{..})) {cite!(["603.2", "505.1a"]; assert_eq!(g.part,Part::PrecombatMain));return;}
        cite!(["603.2"]; assert!(!matches!(g.decision,Some(Decision::Modes{..})),"no other main triggers"));
        match g.decision {Some(Decision::Attackers{player})=>g.act(player,Move::Attack(vec![])).unwrap(),_=>g.act(g.priority,Move::Pass).unwrap()};
    }
    panic!("did not get the next own first main");
}
