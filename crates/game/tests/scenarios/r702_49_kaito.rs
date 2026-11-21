//! Full real-card witness for 702.49 and its composition with 508.4/606/613.
use crate::support::cite;
use game::{CardDef,Game,Move,ObjId,Target,Zone};
use game::state::Decision;
use game::simulation::Simulation;
use ir::CardType;
fn kaito()->CardDef {let fs=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();let d=CardDef::from_face(fs.iter().find(|f|f.face_name=="Kaito, Bane of Nightmares").unwrap()).unwrap();game::play::check(&d).unwrap();d}
fn setup()->Game {let bear=CardDef::new("Bear","{0}","Creature — Ninja","",Some((2,2))).unwrap();let walker=CardDef::planeswalker("Walker","{0}","Planeswalker — Jace","+1: You gain 1 life.",10).unwrap();let mut g=Game::new(vec![kaito(),bear,walker],3,20);for p in 0..3{for _ in 0..15{g.put(p,1,Zone::Library);}}g}
fn pass(g:&mut Game){for _ in 0..g.players.len(){g.act(g.decision_maker(g.priority),Move::Pass).unwrap();}}
fn begin(g:&mut Game,attacks:Vec<(ObjId,Target)>){pass(g);pass(g);g.act(0,Move::Attack(attacks)).unwrap();}
fn unblock(g:&mut Game){pass(g);while let Some(Decision::Blockers{player})=g.decision {g.act(player,Move::Block(vec![])).unwrap();}}
fn mana(g:&mut Game){g.players[0].pool.colors=[0,1,1,0,0];g.players[0].pool.colorless=1;}
fn ninja(k:ObjId,a:ObjId)->Move {crate::support::paid("CUB",Move::Activate{source:k,ability:5,targets:vec![],paying:vec![vec![a]]})}
fn entered(g:&Game)->ObjId {*g.battlefield.iter().find(|o|g.obj(**o).card==Some(0)).unwrap()}
#[test]fn r702_49_full_card_rejects_before_blockers_then_enters_tapped_attacking_same_defender(){
 let mut g=setup();let a=g.put(0,1,Zone::Battlefield);let k=g.put(0,0,Zone::Hand);begin(&mut g,vec![(a,Target::Player(2))]);mana(&mut g);
 assert!(!g.sees(1,k));cite!(["702.49a", "509.1h"]; refused("Bear is not an unblocked attacking creature you control: no attacking creature is unblocked until blockers are declared (702.49a, 509.1h)"): g.act(0,ninja(k,a)));assert!(!g.sees(1,k));unblock(&mut g);mana(&mut g);

 g.act(0,ninja(k,a)).unwrap();assert_eq!(g.obj(a).zone,Zone::Gone);cite!(["702.49a"]; assert_eq!(g.players[0].hand.len(),2));cite!(["702.49b"]; assert!(g.sees(1,k)));assert!(g.sees(2,k));assert!(!g.combat.attacking(a));
 pass(&mut g);let now=entered(&g);assert_ne!(now,k);cite!(["702.49a"]; assert!(g.obj(now).tapped));cite!(["306.5b"]; assert_eq!(g.obj(now).counters["loyalty"],4));cite!(["702.49c"]; assert_eq!(g.combat.attackers,vec![(now,Target::Player(2))]));assert!(g.is_a(now,CardType::Creature));assert!(!g.is_a(now,CardType::Planeswalker));assert_eq!(g.seen(now,None).pt,Some((3,4)));
 pass(&mut g);assert_eq!(g.players[2].life,17);assert_eq!(g.players[1].life,20);
}
#[test]fn r702_49b_two_occurrences_keep_reveal_until_both_leave_and_do_not_retarget_source(){
 let mut g=setup();let a=g.put(0,1,Zone::Battlefield);let b=g.put(0,1,Zone::Battlefield);let k=g.put(0,0,Zone::Hand);begin(&mut g,vec![(a,Target::Player(1)),(b,Target::Player(2))]);unblock(&mut g);mana(&mut g);g.act(0,ninja(k,a)).unwrap();let first=g.stack[0];mana(&mut g);g.act(0,ninja(k,b)).unwrap();let second=g.stack[1];
 assert!(g.counter(second));cite!(["702.49b"]; assert!(g.sees(2,k)));assert!(g.counter(first));cite!(["702.49b"]; assert!(!g.sees(2,k)));assert_eq!(g.obj(k).zone,Zone::Hand);assert_eq!(g.players[0].hand.len(),3);
}
#[test]fn r508_4a_planeswalker_departure_does_not_redirect_attacker_to_player(){
 let mut g=setup();let a=g.put(0,1,Zone::Battlefield);let pw=g.put(2,2,Zone::Battlefield);g.as_entered(pw);let k=g.put(0,0,Zone::Hand);begin(&mut g,vec![(a,Target::Object(pw))]);unblock(&mut g);mana(&mut g);g.act(0,ninja(k,a)).unwrap();g.exile(pw);pass(&mut g);let now=entered(&g);assert!(g.obj(now).tapped);cite!(["508.4a"]; assert!(!g.combat.attacking(now)));
}
#[test]fn r702_49c_planeswalker_defender_and_returned_creature_are_retained_by_occurrence(){
 let mut g=setup();let a=g.put(0,1,Zone::Battlefield);let pw=g.put(2,2,Zone::Battlefield);g.as_entered(pw);let k=g.put(0,0,Zone::Hand);begin(&mut g,vec![(a,Target::Object(pw))]);unblock(&mut g);mana(&mut g);g.act(0,ninja(k,a)).unwrap();pass(&mut g);let now=entered(&g);cite!(["702.49c"]; assert_eq!(g.combat.attackers,vec![(now,Target::Object(pw))]));pass(&mut g);assert_eq!(g.obj(pw).counters["loyalty"],7);assert_eq!(g.players[2].life,20);
}
#[test]fn r400_7_departed_hand_source_does_not_follow_a_returned_card(){
 let mut g=setup();let a=g.put(0,1,Zone::Battlefield);let k=g.put(0,0,Zone::Hand);begin(&mut g,vec![(a,Target::Player(1))]);unblock(&mut g);mana(&mut g);g.act(0,ninja(k,a)).unwrap();g.move_object(k,Zone::Graveyard);let grave=*g.players[0].graveyard.last().unwrap();g.move_object(grave,Zone::Hand);let returned=*g.players[0].hand.last().unwrap();cite!(["400.7"]; assert!(!g.sees(1,returned)));pass(&mut g);cite!(["400.7"]; assert!(!g.battlefield.iter().any(|o|g.obj(*o).card==Some(0))));assert_eq!(g.obj(returned).zone,Zone::Hand);
}
#[test]fn r702_49_managed_protocol_clones_reveal_and_retained_defender_then_reclaims_cost_source(){
 let mut g=setup();let a=g.put(0,1,Zone::Battlefield);let k=g.put(0,0,Zone::Hand);begin(&mut g,vec![(a,Target::Player(2))]);unblock(&mut g);mana(&mut g);let mut sim=Simulation::new(g);
 let request=sim.request().unwrap().unwrap().clone();assert_eq!((request.player,request.acting),(0,0));sim.respond(request.id,request.player,ninja(k,a)).unwrap();let copy=sim.clone();
 for mut branch in [sim,copy]{assert!(branch.game().unwrap().sees(1,k));for _ in 0..3{let r=branch.request().unwrap().unwrap().clone();branch.respond(r.id,r.player,Move::Pass).unwrap();}let now=entered(branch.game().unwrap());cite!(["702.49c"]; assert_eq!(branch.game().unwrap().combat.attackers,vec![(now,Target::Player(2))]));let turn=branch.game().unwrap().turn;for _ in 0..200 {let g=branch.game().unwrap();if g.turn>=turn+3{break;}let mv=match &g.decision{Some(Decision::Attackers{..})=>Move::Attack(vec![]),Some(Decision::Discard{player,count})=>Move::Discard(g.players[*player].hand[..*count].to_vec()),None=>Move::Pass,x=>panic!("{x:?}")};let r=branch.request().unwrap().unwrap().clone();branch.respond(r.id,r.player,mv).unwrap();}assert!(branch.game().unwrap().get_object(a).is_none());assert!(branch.game().unwrap().get_object(k).is_none());}
}
#[test]fn unsupported_entry_and_reveal_forms_remain_refused(){
 for (kind,text) in [("Creature","Commander ninjutsu {1}{U}{B}"),("Creature","{0}, Reveal this card from your hand: You gain 1 life."),("Instant","Put target creature card from your graveyard onto the battlefield tapped and attacking."),("Creature","{0}, Sacrifice two creatures: Put this card from your hand onto the battlefield attacking the same player or planeswalker as those creatures.")]{let d=CardDef::new("Boundary","{0}",kind,text,if kind=="Creature"{Some((2,2))}else{None}).unwrap();let reason=if text.starts_with("Commander") { "keyword: Commander ninjutsu" } else if text.contains("Reveal this card") { "cost: reveal other than a number of cards" } else if kind=="Instant" { "battlefield entry modifier not lowered: TappedAndAttacking" } else { "entry defender requires a singular cost participant" };
 crate::support::refused(game::play::check(&d).map_err(|e|e.0),reason);}
}
#[test]fn r702_49b_reveal_is_visible_during_paused_mana_payment_and_survives_clone(){
 use game::state::{Choice,Answer};
 let mut g=setup();let d=CardDef::new("Mana choice","{0}","Artifact","{T}: Scry 1. Add {C}{U}{B}.",None).unwrap();game::play::check(&d).unwrap();std::sync::Arc::make_mut(&mut g.defs).push(d);let mana_source=g.put(0,3,Zone::Battlefield);let a=g.put(0,1,Zone::Battlefield);let k=g.put(0,0,Zone::Hand);begin(&mut g,vec![(a,Target::Player(2))]);unblock(&mut g);
 let plan=vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![], source:mana_source,ability:0,paying:vec![],adding:vec![]}];g.act(0,Move::WithMana(plan,Box::new(ninja(k,a)))).unwrap();cite!(["702.49b", "602.2a"]; assert!(g.sees(1,k)));assert!(g.combat.attacking(a));let Some(Decision::Choose{choice:Choice::Arrange{cards,..},..})=g.decision.clone()else{panic!("mana child must pause")};let copy=g.clone();
 for mut branch in [g,copy]{branch.act(0,Move::Choose(Answer::Arrange{top:cards.clone(),away:vec![]})).unwrap();assert!(!branch.combat.attacking(a));cite!(["702.49b"]; assert!(branch.sees(1,k)));pass(&mut branch);let now=entered(&branch);cite!(["702.49c"]; assert_eq!(branch.combat.attackers,vec![(now,Target::Player(2))]));}
}
#[test]fn r702_49_returning_an_opponents_creature_you_control_goes_to_its_owners_hand(){
 let mut g=setup();let a=g.put(1,1,Zone::Battlefield);let take=CardDef::new("Take","{0}","Instant","Gain control of target creature until end of turn. It gains haste until end of turn.",None).unwrap();game::play::check(&take).unwrap();std::sync::Arc::make_mut(&mut g.defs).push(take);let spell=g.put(0,3,Zone::Hand);g.act(0,Move::Cast{card:spell,targets:vec![vec![Target::Object(a)]]}).unwrap();pass(&mut g);let k=g.put(0,0,Zone::Hand);begin(&mut g,vec![(a,Target::Player(2))]);unblock(&mut g);mana(&mut g);g.act(0,ninja(k,a)).unwrap();assert_eq!(g.players[0].hand.len(),1);cite!(["702.49a"]; assert_eq!(g.players[1].hand.len(),1));pass(&mut g);cite!(["702.49c"]; assert_eq!(g.combat.attackers,vec![(entered(&g),Target::Player(2))]));
}
#[test]fn r122_6_entering_loyalty_and_attacking_are_visible_to_entry_triggers_and_turn_facts(){
 let mut g=setup();let d=CardDef::new("Entry observer","{0}","Artifact","Whenever a creature enters, you gain 1 life.\nWhenever a creature attacks, you gain 2 life.\n{0}: If a creature entered under your control this turn, you gain 3 life.",None).unwrap();game::play::check(&d).unwrap();std::sync::Arc::make_mut(&mut g.defs).push(d);let a=g.put(0,1,Zone::Battlefield);let k=g.put(0,0,Zone::Hand);begin(&mut g,vec![(a,Target::Player(2))]);unblock(&mut g);mana(&mut g);g.act(0,ninja(k,a)).unwrap();let observer=g.put(0,3,Zone::Battlefield);pass(&mut g);cite!(["508.3a"]; assert_eq!(g.stack.len(),1,"entry triggers, but entry attacking never declares an attack"));pass(&mut g);assert_eq!(g.players[0].life,21);g.act(0,Move::Activate{source:observer,ability:2,targets:vec![],paying:vec![]}).unwrap();pass(&mut g);assert_eq!(g.players[0].life,24,"entry history sees Kaito as a creature with its entering loyalty counters");
}

#[test]
fn r702_49b_snapshot_reveal_grants_follow_live_occurrences_not_past_knowledge() {
    use game::snapshot::Snapshot;
    let mut g = setup();
    let a = g.put(0, 1, Zone::Battlefield);
    let b = g.put(0, 1, Zone::Battlefield);
    let k = g.put(0, 0, Zone::Hand);
    let unrelated = g.put(0, 0, Zone::Hand);
    begin(&mut g, vec![(a, Target::Player(1)), (b, Target::Player(2))]);
    unblock(&mut g);
    assert!(Snapshot::of(&g, Some(1)).publicly_revealed.is_empty());
    mana(&mut g);
    g.act(0, ninja(k, a)).unwrap();
    let first = g.stack[0];
    mana(&mut g);
    g.act(0, ninja(k, b)).unwrap();
    let second = g.stack[1];
    for mut branch in [g.clone(), g] {
        let seen = Snapshot::of(&branch, Some(1));
        cite!(["702.49b"]; assert_eq!(seen.publicly_revealed, vec![k], "duplicate grants name the card once"));
        assert!(seen.objects.iter().any(|o| o.id == k));
        assert!(!seen.objects.iter().any(|o| o.id == unrelated));
        assert!(branch.counter(second));
        cite!(["702.49b"]; assert_eq!(Snapshot::of(&branch, Some(2)).publicly_revealed, vec![k]));
        assert!(branch.counter(first));
        let ended = Snapshot::of(&branch, Some(2));
        cite!(["702.49b"]; assert!(ended.publicly_revealed.is_empty()));
        assert!(!ended.objects.iter().any(|o| o.id == k));
        assert_eq!(branch.obj(k).zone, Zone::Hand);
    }
}

#[test]
fn r702_49a_return_cost_refuses_what_is_not_an_unblocked_attacking_creature_you_control() {
    // 702.49a: the creature returned is "an unblocked attacking creature you control"; once
    // blockers are declared, one with a blocker declared for it is blocked (509.1h)
    let mut g = setup();
    let a = g.put(0, 1, Zone::Battlefield);
    let blocked = g.put(0, 1, Zone::Battlefield);
    let idle = g.put(0, 1, Zone::Battlefield);
    let theirs = g.put(2, 1, Zone::Battlefield);
    let blocker = g.put(2, 1, Zone::Battlefield);
    let k = g.put(0, 0, Zone::Hand);
    begin(&mut g, vec![(a, Target::Player(2)), (blocked, Target::Player(2))]);
    mana(&mut g);
    // before blockers only an attacker is told that no attacker is unblocked yet (509.1h)
    cite!(["702.49a"]; case: "an opponent's creature before blockers"; refused("Bear is not an unblocked attacking creature you control (702.49a)"): g.act(0, ninja(k, theirs)));
    cite!(["702.49a"]; case: "a creature that is not attacking before blockers"; refused("Bear is not an unblocked attacking creature you control (702.49a)"): g.act(0, ninja(k, idle)));
    pass(&mut g);
    while let Some(Decision::Blockers { player }) = g.decision {
        g.act(player, Move::Block(if player == 2 { vec![(blocker, blocked)] } else { vec![] })).unwrap();
    }
    mana(&mut g);
    for (case, o) in [("not attacking", idle), ("an opponent's", theirs)] {
        cite!(["702.49a"]; case: case; refused("Bear is not an unblocked attacking creature you control (702.49a)"): g.act(0, ninja(k, o)));
    }
    cite!(["702.49a", "509.1h"]; case: "a blocked attacker"; refused("Bear is not an unblocked attacking creature you control (702.49a)"): g.act(0, ninja(k, blocked)));
    assert_eq!(g.obj(k).zone, Zone::Hand);
    assert!(g.combat.attacking(blocked) && g.combat.attacking(a));
    g.act(0, ninja(k, a)).unwrap();
    cite!(["702.49a"]; assert_eq!(g.obj(a).zone, Zone::Gone));
}

#[test]
fn r702_49a_the_ninjutsu_ability_reads_as_its_rule_defines_it_in_the_prompt_and_on_the_stack() {
    let mut g = setup();
    let a = g.put(0, 1, Zone::Battlefield);
    let k = g.put(0, 0, Zone::Hand);
    // 702.49a: "Ninjutsu [cost]" means "[Cost], Reveal this card from your hand, Return an
    // unblocked attacking creature you control to its owner's hand: Put this card onto the
    // battlefield from your hand tapped and attacking." Kaito's cost is {1}{U}{B}; the text
    // names the card itself where the rule says "this card".
    let cost = "{1}{U}{B}, Reveal Kaito, Bane of Nightmares from your hand, Return an unblocked attacking creature you control to its owner's hand: ";
    // it is put onto the battlefield from the hand, tapped and attacking (702.49a) the
    // returned creature's defender (702.49c)
    let reads = |text: &str| text.strip_prefix(cost).is_some_and(|effect| effect.starts_with("Put Kaito, Bane of Nightmares ")
        && ["onto the battlefield", "from your hand", "tapped", "attacking the same", "as that creature"].iter().all(|said| effect.contains(said)));
    begin(&mut g, vec![(a, Target::Player(2))]);
    unblock(&mut g);
    mana(&mut g);
    let mut sim = Simulation::new(g);
    let prompt = sim.prompt(0).unwrap();
    let ability = prompt.entities[&k].abilities.iter().find(|entry| entry.index == 5).expect("the hand card offers its ninjutsu ability");
    cite!(["702.49a"]; assert!(reads(&ability.text), "{:?}", ability.text));
    let request = sim.request().unwrap().unwrap().clone();
    sim.respond(request.id, request.player, ninja(k, a)).unwrap();
    let stacked = sim.prompt(1).unwrap();
    let top = *stacked.view.stack.last().unwrap();
    let occurrence = stacked.entities[&top].occurrence.as_ref().unwrap();
    let text = occurrence.text.as_deref().expect("the activated ability keeps its text on the stack");
    cite!(["702.49a"]; assert!(reads(text), "{text:?}"));
}

#[test]
fn r702_49a_return_cost_refuses_other_than_one_unblocked_attacking_creature() {
    // 702.49a: the cost returns "an unblocked attacking creature you control"; an attacking
    // creature becomes unblocked only as blockers are declared (509.1h)
    let returning = |k: ObjId, chosen: Vec<ObjId>| crate::support::paid("CUB", Move::Activate { source: k, ability: 5, targets: vec![], paying: vec![chosen] });
    let mut g = setup();
    let a = g.put(0, 1, Zone::Battlefield);
    let b = g.put(0, 1, Zone::Battlefield);
    let k = g.put(0, 0, Zone::Hand);
    begin(&mut g, vec![(a, Target::Player(2)), (b, Target::Player(2))]);
    mana(&mut g);
    // before blockers no creature can be returned, so none returned is told why (509.1h)
    cite!(["702.49a", "509.1h"]; case: "none before blockers"; refused("ninjutsu's cost returns one unblocked attacking creature you control: 0 chosen; no attacking creature is unblocked until blockers are declared (702.49a, 509.1h)"): g.act(0, returning(k, vec![])));
    // two attackers before blockers: the first named is not yet unblocked
    cite!(["702.49a", "509.1h"]; case: "two before blockers"; refused("Bear is not an unblocked attacking creature you control: no attacking creature is unblocked until blockers are declared (702.49a, 509.1h)"): g.act(0, returning(k, vec![a, b])));
    unblock(&mut g);
    mana(&mut g);
    cite!(["702.49a"]; case: "none after blockers"; refused("ninjutsu's cost returns one unblocked attacking creature you control: 0 chosen (702.49a)"): g.act(0, returning(k, vec![])));
    cite!(["702.49a"]; case: "two after blockers"; refused("ninjutsu's cost returns one unblocked attacking creature you control: 2 chosen (702.49a)"): g.act(0, returning(k, vec![a, b])));
    assert_eq!(g.obj(k).zone, Zone::Hand);
    assert!(g.combat.attacking(a) && g.combat.attacking(b));
    g.act(0, returning(k, vec![b])).unwrap();
    cite!(["702.49a"]; assert_eq!((g.obj(a).zone, g.obj(b).zone), (Zone::Battlefield, Zone::Gone)));
}
