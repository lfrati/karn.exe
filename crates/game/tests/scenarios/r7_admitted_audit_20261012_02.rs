//! A second fixed random selection of cards: native clauses and consequential boundaries, not whole cards.
use crate::support::{cite, oracle, paid};
use game::legal::Asked;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

fn setup(native: &str) -> Game {
    let mut g = Game::new(vec![oracle(native),
        CardDef::new("Gate Fixture", "", "Land — Gate", "", None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2,2))).unwrap(),
        CardDef::new("Wall", "{0}", "Creature — Wall", "", Some((1,4))).unwrap(),
        CardDef::new("Metal Bear", "{0}", "Artifact Creature — Bear", "", Some((2,2))).unwrap(),
        CardDef::new("Rock", "{0}", "Artifact", "", None).unwrap(),
        CardDef::new("Gift", "{0}", "Instant", "You gain 1 life.", None).unwrap(),
        CardDef::new("Shrink", "{0}", "Instant", "Target creature gets -1/-1 until end of turn.", None).unwrap(),
        CardDef::new("Destroy", "{0}", "Instant", "Destroy target creature.", None).unwrap()], 2,20);
    for p in 0..2 { for _ in 0..40 { g.put(p,1,Zone::Library); } }
    g
}
fn cast(g: &mut Game, player: usize, def: usize, targets: Vec<Vec<Target>>) {
    while g.priority != player { g.act(g.priority,Move::Pass).unwrap(); }
    let card=g.put(player,def,Zone::Hand);g.act(player,Move::Cast {card,targets}).unwrap();
}
fn resolve(g: &mut Game) { for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();} }
fn settle(g: &mut Game) { for _ in 0..10 {if g.stack.is_empty(){return;} resolve(g);} panic!("stack not settled"); }
fn progress(g: &mut Game) {
    let (p,a)=g.asked().unwrap();let mv=match a {
        Asked::Priority=>Move::Pass,Asked::Attackers(_)=>Move::Attack(vec![]),
        Asked::Blockers {..}=>Move::Block(vec![]),other=>panic!("unexpected {other:?}")};g.act(p,mv).unwrap();
}
fn to_turn(g: &mut Game, turn: u32) {for _ in 0..200 {if g.turn==turn{return;} progress(g);}panic!("turn not reached");}
fn to_main(g: &mut Game, turn: u32, player: usize) {
    for _ in 0..300 {if g.turn==turn && g.part==Part::PrecombatMain && g.priority==player {return;} progress(g);}panic!("main phase not reached");
}
fn stance(g: &mut Game, mode: usize, target: ObjId) {
    g.players[0].pool.colors[0]=1;g.players[0].pool.colorless=1;
    let card=g.put(0,0,Zone::Hand);
    g.act(0,Move::Modes(vec![mode],Box::new(paid("CW",Move::Cast {card,targets:vec![vec![Target::Object(target)]]})))).unwrap();
}

#[test]
fn r603_traxos_artifact_or_creature_cast_is_one_trigger_and_not_an_opponents_cast() {
    let mut g=setup("Traxos, Scourge Eternal");let traxos=g.put(0,0,Zone::Battlefield);g.tap(traxos);
    cast(&mut g,0,4,vec![]); // The same spell is both artifact and creature.
    cite!(["603.2", "603.2c"]; assert_eq!(g.stack.len(),2));
    resolve(&mut g);assert!(!g.obj(traxos).tapped);assert_eq!(g.stack.len(),1);settle(&mut g);
    g.tap(traxos);to_main(&mut g,2,1);cast(&mut g,1,4,vec![]);
    cite!(["109.5", "603.2"]; assert_eq!(g.stack.len(),1));settle(&mut g);assert!(g.obj(traxos).tapped);
    cast(&mut g,0,6,vec![]);assert_eq!(g.stack.len(),1);settle(&mut g);assert!(g.obj(traxos).tapped);
    to_main(&mut g,3,0);cast(&mut g,0,5,vec![]);assert_eq!(g.stack.len(),2);resolve(&mut g);assert!(!g.obj(traxos).tapped);settle(&mut g);
}

#[test]
fn r502_traxos_own_untap_restriction_does_not_forbid_triggered_untapping() {
    let mut g=setup("Traxos, Scourge Eternal");let traxos=g.put(0,0,Zone::Battlefield);let rock=g.put(0,5,Zone::Battlefield);
    g.tap(traxos);g.tap(rock);to_turn(&mut g,3);
    cite!(["502.3"]; assert!(g.obj(traxos).tapped));assert!(!g.obj(rock).tapped);
    assert_eq!((g.power(traxos),g.toughness(traxos)),(Some(5),Some(4)));assert!(g.has(traxos,"Trample"));
    to_main(&mut g,3,0);cast(&mut g,0,2,vec![]);resolve(&mut g);
    cite!(["502.3", "608.2c"]; assert!(!g.obj(traxos).tapped));settle(&mut g);
}

#[test]
fn r700_valorous_stance_chooses_one_mode_and_destroy_mode_checks_four_toughness() {

    let mut g=setup("Valorous Stance");let bear=g.put(1,2,Zone::Battlefield);let wall=g.put(1,3,Zone::Battlefield);
    g.players[0].pool.colors[0]=1;g.players[0].pool.colorless=1;let card=g.put(0,0,Zone::Hand);
    let before=format!("{g:?}");
    for modes in [vec![],vec![0,1]] {
        cite!(["700.2a"]; refused("choose 1 to 1 of its modes (700.2a)"): g.act(0,Move::Modes(modes,Box::new(paid("CW",Move::Cast {card,targets:vec![vec![Target::Object(wall)]]})))));
        cite!(["700.2a", "733.1"]; assert_eq!(format!("{g:?}"),before));
    }
    cite!(["601.2c"]; refused("an illegal target, Bear: its toughness is 2"): g.act(0,Move::Modes(vec![1],Box::new(paid("CW",Move::Cast {card,targets:vec![vec![Target::Object(bear)]]})))));
    cite!(["601.2c", "733.1"]; assert_eq!(format!("{g:?}"),before));
    g.act(0,Move::Modes(vec![1],Box::new(paid("CW",Move::Cast {card,targets:vec![vec![Target::Object(wall)]]})))).unwrap();settle(&mut g);
    cite!(["701.8a"]; assert!(!g.battlefield.contains(&wall)));assert!(g.battlefield.contains(&bear));
}

#[test]
fn r608_valorous_stance_destroy_target_losing_required_toughness_is_illegal() {
    let mut g=setup("Valorous Stance");let wall=g.put(1,3,Zone::Battlefield);stance(&mut g,1,wall);
    cast(&mut g,0,7,vec![vec![Target::Object(wall)]]);resolve(&mut g);assert_eq!(g.toughness(wall),Some(3));
    resolve(&mut g);
    cite!(["608.2b"]; assert!(g.battlefield.contains(&wall)));assert_eq!(g.players[0].graveyard.len(),2);
}

#[test]
fn r702_valorous_stance_indestructible_mode_protects_opponent_until_cleanup() {
    let mut g=setup("Valorous Stance");let bear=g.put(1,2,Zone::Battlefield);stance(&mut g,0,bear);settle(&mut g);
    assert!(g.has(bear,"Indestructible"));cast(&mut g,0,8,vec![vec![Target::Object(bear)]]);settle(&mut g);
    cite!(["702.12b"]; assert!(g.battlefield.contains(&bear)));
    to_turn(&mut g,2);cite!(["514.2", "611.2a"]; assert!(!g.has(bear,"Indestructible")));
    cast(&mut g,0,8,vec![vec![Target::Object(bear)]]);settle(&mut g);assert!(!g.battlefield.contains(&bear));
}

#[test]
fn r603_gateway_sneak_own_gate_prevents_blocking_and_combat_draw_waits_for_resolution() {
    let mut g=setup("Gateway Sneak");let sneak=g.put(0,0,Zone::Battlefield);let blocker=g.put(1,2,Zone::Battlefield);
    to_main(&mut g,2,1);
    let theirs=g.put(1,1,Zone::Hand);g.act(1,Move::PlayLand(theirs)).unwrap();
    assert!(g.stack.is_empty());assert!(!g.cant_block(blocker,sneak,&[blocker]));
    to_main(&mut g,3,0);
    let mine=g.put(0,1,Zone::Hand);g.act(0,Move::PlayLand(mine)).unwrap();
    cite!(["603.2"]; assert_eq!(g.stack.len(),1));settle(&mut g);
    cite!(["509.1b"]; assert!(g.cant_block(blocker,sneak,&[blocker])));
    while g.part!=Part::BeginningOfCombat {progress(&mut g);}resolve(&mut g);
    g.act(0,Move::Attack(vec![(sneak,Target::Player(1))])).unwrap();resolve(&mut g);
    assert_eq!(g.part,Part::DeclareBlockers);
    // If there are no legal blocks, the owner may omit the empty choice.
    if matches!(g.asked(),Some((1,Asked::Blockers { .. }))) {g.act(1,Move::Block(vec![])).unwrap();}
    let hand=g.players[0].hand.len();
    for _ in 0..30 {if !g.stack.is_empty(){break;} progress(&mut g);}
    assert_eq!(g.players[1].life,19);assert_eq!(g.stack.len(),1);
    cite!(["603.3", "120.2a"]; assert_eq!(g.players[0].hand.len(),hand));
    resolve(&mut g);cite!(["121.1"]; assert_eq!(g.players[0].hand.len(),hand+1));
    to_turn(&mut g,4);cite!(["514.2", "611.2a"]; assert!(!g.cant_block(blocker,sneak,&[blocker])));
}
