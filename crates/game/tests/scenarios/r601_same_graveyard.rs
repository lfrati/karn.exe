//! A single graveyard is an actual player's zone, not the unary Zone::Graveyard kind.
use crate::support::{cite,paid};
use game::state::Answer;
use game::{CardDef, Game, Move, ObjId, Target, Zone, Event};
use std::sync::LazyLock;
static FACES: LazyLock<Vec<oracle::corpus::Face>> = LazyLock::new(|| oracle::corpus::load(&oracle::corpus::default_path()).unwrap());
fn real(name: &str) -> CardDef { CardDef::from_faces(&CardDef::catalog(&FACES)[name]).unwrap_or_else(|e|panic!("{name}: {e}")) }
fn card(name: &str, kind: &str, text: &str, pt: Option<(i32,i32)>) -> CardDef { CardDef::new(name,"{0}",kind,text,pt).unwrap_or_else(|e|panic!("{name}: {text}: {e}")) }
fn setup(text: &str) -> Game {
    let defs = vec![card("Clear", "Instant", text, None),card("Bear","Creature — Bear","",Some((2,2))),card("Land","Basic Land — Island","",None),
        card("Change", "Instant", "You may choose new targets for target spell.", None),
        card("Mirror", "Instant", "Copy target instant or sorcery spell. You may choose new targets for the copy.", None)];
    for d in &defs { game::play::check(d).unwrap_or_else(|e|panic!("{}: {}",d.name(),e.0)); }
    Game::new(defs,2,20)
}
fn pass(g:&mut Game) { for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();} }
fn spell(g:&mut Game, chosen: Vec<Target>) -> ObjId {
    let hand=g.put(0,0,Zone::Hand);g.act(0,Move::Cast {card:hand,targets:vec![chosen]}).unwrap();*g.stack.last().unwrap()
}
fn change_setup(copy:bool) -> (Game,ObjId,ObjId,ObjId,ObjId) {
    let mut g=setup("Exile up to two target cards from a single graveyard.");
    let a=g.put(0,1,Zone::Graveyard);let b=g.put(0,2,Zone::Graveyard);let c=g.put(1,1,Zone::Graveyard);let d=g.put(1,2,Zone::Graveyard);
    let original=spell(&mut g,vec![a.into(),b.into()]);let changer=g.put(0,if copy{4}else{3},Zone::Hand);
    g.act(0,Move::Cast{card:changer,targets:vec![vec![original.into()]]}).unwrap();pass(&mut g);
    assert!(g.decision.is_some());(g,a,b,c,d)
}
#[test]
fn r601_2c_standard_consumers_keep_distinct_full_card_refusals() {
    for name in ["Arashin Sunshield","Feral Deathgorger // Dusk Sight","Qutrub Forayer","Griffnaut Tracker","Push // Pull","Cease // Desist","Digsite Conservator","Soul-Shackled Zombie"] {
        if name == "Soul-Shackled Zombie" {
            // Uncited (admission): its separate conditional followup is not parsed.
            assert_eq!(CardDef::from_faces(&CardDef::catalog(&FACES)[name]).unwrap_err(),"text the grammar does not read");
            continue;
        }
        let d=real(name);
        // Uncited (admission): full Oracle consumers, not an execution coverage claim.
        // Discover's owning mechanism now lowers Digsite's separate death clause.
        game::play::check(&d).unwrap_or_else(|e|panic!("{name}: {}",e.0));
    }
}
#[test]
fn r601_2c_same_graveyard_allows_zero_one_two_but_refuses_mixed_duplicates_and_excess() {
    let mut g=setup("Exile up to two target cards from a single graveyard. You gain 1 life.");
    let a=g.put(0,1,Zone::Graveyard);let b=g.put(0,2,Zone::Graveyard);let c=g.put(1,1,Zone::Graveyard);let d=g.put(1,2,Zone::Graveyard);
    let hand=g.put(0,0,Zone::Hand);
    for (targets, why) in [(vec![a.into(),c.into()],"single graveyard"),(vec![a.into(),a.into()],"same target twice"),(vec![a.into(),b.into(),c.into()],"wrong number of targets")] {
        cite!(["601.2c","115.3"];case:why;refused(why):g.act(0,Move::Cast{card:hand,targets:vec![targets]}));
        cite!(["601.2c"];assert_eq!(g.obj(hand).zone,Zone::Hand));
    }
    for chosen in [vec![],vec![a.into()],vec![a.into(),b.into()],vec![c.into(),d.into()]] {
        let mut copy=g.clone();
        cite!(["601.2c"];case:format!("{}",chosen.len());ok:copy.act(0,Move::Cast{card:hand,targets:vec![chosen.clone()]}));pass(&mut copy);
        cite!(["608.2b"];assert_eq!(copy.players[0].life,21));
        for target in chosen {let Target::Object(old)=target else{unreachable!()};cite!(["608.2b"];assert!(!copy.players[copy.obj(old).owner].graveyard.contains(&old)));}
    }
}
#[test]
fn r601_2c_required_group_listing_has_no_witness_across_two_different_graveyards() {
    let mut g=setup("Exile two target cards from a single graveyard.");
    let first=g.put(0,1,Zone::Graveyard);let other=g.put(1,1,Zone::Graveyard);let hand=g.put(0,0,Zone::Hand);
    let before = format!("{g:?}");
    cite!(["601.2c", "404.1"]; refused("single graveyard"): g.act(0, Move::Cast { card: hand, targets: vec![vec![first.into(), other.into()]] }));
    assert_eq!(format!("{g:?}"), before);
    let second=g.put(0,2,Zone::Graveyard);
    cite!(["601.2c"];ok:g.act(0,Move::Cast{card:hand,targets:vec![vec![first.into(), second.into()]]}));
}
#[test]
fn r608_2b_peer_departure_and_new_incarnation_do_not_invalidate_survivor() {
    for loss in ["one","both","return"] {
        let mut g=setup("Exile up to two target cards from a single graveyard. You gain 1 life.");let a=g.put(0,1,Zone::Graveyard);let b=g.put(0,2,Zone::Graveyard);
        let on=spell(&mut g,vec![a.into(),b.into()]);g.move_object(a,Zone::Hand);
        if loss=="both" {g.move_object(b,Zone::Hand);}else if loss=="return"{let back=g.players[0].hand[0];g.move_object(back,Zone::Graveyard);}
        pass(&mut g);
        cite!(["608.2b"];case:loss;assert_eq!(g.players[0].life,if loss=="both"{20}else{21}));
        cite!(["608.2b"];case:loss;assert_eq!(g.log.iter().any(|e|matches!(e,Event::Fizzled{spell}if *spell==on)),loss=="both"));
        if loss=="return" {cite!(["400.7","608.2b"];assert_eq!(g.players[0].graveyard.iter().filter(|o|g.obj(**o).card==Some(1)).count(),1));}
        if loss!="both" {cite!(["608.2b"];assert!(g.exile.iter().any(|o|g.obj(*o).card==Some(2))));}
    }
}
#[test]
fn r115_7e_changed_final_group_can_move_both_but_not_one_legal_target_to_another_graveyard() {
    for copy in [false,true] {
        let (g,a,b,c,d)=change_setup(copy);
        for targets in [vec![a.into(),c.into()],vec![c.into(),b.into()]] {
            let mut branch=g.clone();
            cite!(["115.7d","115.7e"];case:format!("copy={copy}");refused("changed target must be legal"):branch.act(0,Move::Choose(Answer::Targets(vec![targets]))));
        }
        let mut branch=g.clone();
        cite!(["601.2c","115.7f"];refused(if copy {"retains target counts and division"} else {"number of targets"}):branch.act(0,Move::Choose(Answer::Targets(vec![vec![c.into()]]))));
        let mut branch=g.clone();
        cite!(["115.7d","115.7e"];ok:branch.act(0,Move::Choose(Answer::Targets(vec![vec![c.into(),d.into()]]))));pass(&mut branch);
        cite!(["608.2b"];assert_eq!(branch.players[1].graveyard.len(),0));
        cite!(["608.2b"];assert_eq!(branch.players[0].graveyard.iter().filter(|o|branch.obj(**o).card==Some(1)||branch.obj(**o).card==Some(2)).count(),2));
    }
}
#[test]
fn r115_7d_unchanged_illegal_peer_does_not_anchor_new_target_but_new_illegal_target_is_refused() {
    let (mut g,a,b,c,_)=change_setup(false);g.move_object(b,Zone::Hand);
    cite!(["115.7d"];refused("changed target must be legal"):g.act(0,Move::Choose(Answer::Targets(vec![vec![b.into(),a.into()]]))));
    cite!(["115.7d","115.7e"];ok:g.act(0,Move::Choose(Answer::Targets(vec![vec![c.into(),b.into()]]))));pass(&mut g);
    cite!(["608.2b"];assert!(!g.players[1].graveyard.contains(&c)&&g.players[0].graveyard.contains(&a)));
}
#[test]
fn r608_2b_cease_groups_resolve_independently_and_zero_cards_is_not_an_extra_target() {
    for (cards, remain, player_legal) in [(true,true,true),(true,true,false),(true,false,true),(true,false,false),(false,false,true),(false,false,false)] {
        let mut g=Game::new(vec![real("Cease // Desist"),card("Bear","Creature","",Some((2,2))),card("Shield","Enchantment","You have hexproof.",None)],2,20);
        let target=g.put(0,1,Zone::Graveyard);g.put(1,1,Zone::Library);let hand=g.put(0,0,Zone::Hand);g.players[0].pool.colors=[10;5];
        let chosen=if cards{vec![target.into()]}else{vec![]};g.act(0,Move::CastAs(game::card::CastAs::Half(0),Box::new(paid("GB",Move::Cast{card:hand,targets:vec![chosen,vec![Target::Player(1)]]})))).unwrap();
        if cards && !remain {g.move_object(target,Zone::Hand);}if !player_legal {g.put(1,2,Zone::Battlefield);}pass(&mut g);
        cite!(["608.2b"];case:format!("cards={cards}, remain={remain}, player={player_legal}");assert_eq!(g.players[1].life,if player_legal{22}else{20}));
        cite!(["608.2b"];assert_eq!(g.players[1].hand.len(),usize::from(player_legal)));
        cite!(["608.2b"];assert_eq!(g.exile.iter().filter(|o|g.obj(**o).card==Some(1)).count(),usize::from(cards && remain)));
    }
}
#[test]
fn r601_2c_digsite_max_four_relation_is_chosen_before_sacrificing_the_source() {
    // The exact activated clause is isolated from Digsite's separately unsupported discover.
    let d=card("Synthetic Digsite clause","Creature","Sacrifice this creature: Exile up to four target cards from a single graveyard. Activate only as a sorcery.",Some((2,1)));
    game::play::check(&d).unwrap();
    let mut g=Game::new(vec![d,card("Bear","Creature","",Some((2,2)))],2,20);
    let source=g.put(0,0,Zone::Battlefield);
    let own:Vec<_>=(0..5).map(|_|g.put(0,1,Zone::Graveyard)).collect();let other=g.put(1,1,Zone::Graveyard);
    for (targets,why) in [(vec![own[0].into(),other.into()],"single graveyard"),(own.iter().copied().map(Target::Object).collect(),"wrong number of targets"),(vec![source.into()],"illegal target")] {
        cite!(["601.2c","602.2b"];case:why;refused(why):g.act(0,Move::Activate{source,ability:0,targets:vec![targets],paying:vec![]}));
        cite!(["601.2c","602.2b"];assert!(g.battlefield.contains(&source)));
    }
    for count in [0,1,4] {
        let mut branch=g.clone();let targets=own[..count].iter().copied().map(Target::Object).collect();
        cite!(["601.2c","602.2b"];case:format!("count={count}");ok:branch.act(0,Move::Activate{source,ability:0,targets:vec![targets],paying:vec![]}));
        cite!(["602.2b"];assert!(!branch.battlefield.contains(&source)));pass(&mut branch);
        cite!(["608.2b"];assert_eq!(branch.exile.len(),count));
        cite!(["608.2b"];assert!(branch.players[1].graveyard.contains(&other)));
    }
}
#[test]
fn r608_2b_pull_grants_haste_and_schedules_sacrifice_only_for_actually_returned_cards() {
    use game::state::Decision;
    use ir::Part;
    for chosen_count in [0,2] {
        let mut g=Game::new(vec![real("Push // Pull"),card("Bear","Creature — Bear","",Some((2,2))),card("Land","Basic Land — Island","",None),card("Shield","Enchantment","You have hexproof.",None)],2,20);
        let a=g.put(1,1,Zone::Graveyard);let b=g.put(1,1,Zone::Graveyard);let land=g.put(1,2,Zone::Graveyard);g.put(1,3,Zone::Battlefield);
        let hand=g.put(0,0,Zone::Hand);g.players[0].pool.colors=[10;5];
        cite!(["601.2c"];refused("illegal target"):g.act(0,Move::CastAs(game::card::CastAs::Half(1),Box::new(Move::Cast{card:hand,targets:vec![vec![land.into()]]}))));
        let chosen=if chosen_count==0{vec![]}else{vec![a.into(),b.into()]};
        cite!(["601.2c","702.11c"];ok:g.act(0,Move::CastAs(game::card::CastAs::Half(1),Box::new(paid("BBBBBB",Move::Cast{card:hand,targets:vec![chosen]})))));
        if chosen_count>0 {g.move_object(a,Zone::Hand);}pass(&mut g);
        let returned:Vec<_>=g.battlefield.iter().copied().filter(|o|g.obj(*o).card==Some(1)).collect();
        cite!(["608.2b"];assert_eq!(returned.len(),usize::from(chosen_count>0)));
        for object in returned {
            cite!(["110.2a","702.10a"];assert_eq!((g.obj(object).controller,g.obj(object).owner,g.can_attack(object).is_ok()),(0,1,true)));
        }
        for _ in 0..100 {
            if g.part==Part::End&&g.stack.is_empty(){break;}
            match g.decision.clone(){Some(Decision::Attackers{player})=>g.act(player,Move::Attack(vec![])).unwrap(),_=>g.act(g.priority,Move::Pass).unwrap()}
        }
        cite!(["603.7","701.17a"];assert!(g.battlefield.iter().all(|o|g.obj(*o).card!=Some(1))));
        cite!(["400.7","608.2b"];assert_eq!(g.players[1].hand.len(),usize::from(chosen_count>0)));
    }
}
#[test]
fn r601_2c_non_target_same_zone_and_same_zone_costs_stay_refused() {
    // These are selections/costs, not target-group metadata. No unary SameZone
    // approximation is enabled by extracting the predicate from TargetSpec.
    for (kind,text) in [("Sorcery","Exile all cards from a single graveyard."),("Enchantment","Exile two creature cards from a single graveyard: You gain 1 life.")] {
        let d=card("Still refused",kind,text,None);
        crate::support::refused(game::play::check(&d).map_err(|e|e.0),"description: SameZone");
    }
}
