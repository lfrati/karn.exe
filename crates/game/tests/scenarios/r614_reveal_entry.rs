//! 614.12a-b: a reveal cost is chosen before entry and paid once at batch commitment.
use crate::support;
use support::{Board,T,cite};
use game::{Move,Zone};
use game::state::{Answer,Choice,Decision};
use game::observation::Kind;

fn choices(t:&T)->Vec<game::ObjId> {
    let Some(Decision::Choose{player:0,choice:Choice::Select{among,min:0,max:1}})=&t.g.decision else {panic!("{:?}",t.g.decision)};
    among.clone()
}
fn play(t:&mut T,name:&str) {let o=t.find_in(0,Zone::Hand,name);t.g.act(0,Move::PlayLand(o)).unwrap();}
fn tapped(t:&T,name:&str)->bool {t.g.obj(t.find_in_battlefield(name)).tapped}

#[test]
fn r614_12a_entry_reveal_offers_only_qualifying_cards_in_the_controllers_hand() {
    for (land,qualifying,wrong) in [("Choked Estuary","Watery Grave","Plains"),("Port Town","Hallowed Fountain","Swamp")] {
        let mut t=Board::new(3).hand(0,&[land,qualifying,wrong]).hand(1,&["Island"])
            .battlefield(0,&["Island"]).graveyard(0,&["Island"]).build();
        let card=t.find_in(0,Zone::Hand,qualifying);let wrong=t.find_in(0,Zone::Hand,wrong);
        let opponent=t.find_in(1,Zone::Hand,"Island");play(&mut t,land);
        assert_eq!(choices(&t),vec![card]);
        cite!(["614.12a"]; assert!(t.g.battlefield.iter().all(|o|t.g.obj(*o).chars.name!=land)));
        crate::support::refused(t.g.act(0,Move::Pass), "a decision is pending: a choice of how the permanent enters the battlefield (614.12)");
        for (answer,why) in [(vec![wrong],"a chosen object is not among the offered objects (608.2d)"),(vec![opponent],"a chosen object is not among the offered objects (608.2d)"),(vec![card,card],"each object is chosen once (608.2d)")] {crate::support::refused(t.answer(0,Answer::Objects(answer)),why);}
        crate::support::refused(t.answer(1,Answer::Objects(vec![card])), "that decision is another player's");
        assert!(t.g.observations(1).is_empty());
        t.answer(0,Answer::Objects(vec![card])).unwrap();
        assert!(!tapped(&t,land));cite!(["701.20b"]; assert_eq!(t.g.obj(card).zone,Zone::Hand));
        for p in 0..3 {
            let shown=t.g.observations(p);assert_eq!(shown.len(),1);
            cite!(["701.20a"]; case: format!("player {p}"); assert_eq!(shown[0].kind,Kind::Revealed));assert_eq!(shown[0].cards.len(),1);
            assert_eq!(shown[0].cards[0].object,card);
        }
        assert!(!t.g.sees(1,card),"reveal knowledge does not grant ongoing hand access");
    }
}

#[test]
fn r614_12a_declining_or_having_no_qualifying_card_enters_tapped_without_disclosure() {
    for hand in [&["Choked Estuary","Island"][..],&["Choked Estuary","Plains"][..]] {
        let mut t=Board::new(2).hand(0,hand).build();play(&mut t,"Choked Estuary");
        cite!(["402.3"]; assert_eq!(choices(&t).len(),usize::from(hand[1] == "Island")));
        t.choose(0,&[]).unwrap();
        cite!(["614.1d"]; assert!(tapped(&t,"Choked Estuary")));assert!(t.g.observations(1).is_empty());
    }
}

#[test]
fn r614_12b_simultaneous_reveals_may_use_the_same_card_and_resume_clones_without_replay() {
    let mut t=Board::new(3).hand(0,&["Splendid Reclamation","Island"])
        .graveyard(0,&["Choked Estuary","Port Town"]).build();
    t.mana(0,"{3}{G}").cast(0,"Splendid Reclamation",vec![], crate::support::mana_pool("CCCG")).unwrap().pass_all();
    let island=t.find_in(0,Zone::Hand,"Island");assert_eq!(choices(&t),vec![island]);
    t.choose(0,&["Island"]).unwrap();assert_eq!(choices(&t),vec![island]);
    cite!(["614.12a"]; assert_eq!(t.g.players[0].graveyard.len(),2,"neither land enters yet"));
    assert!(t.g.observations(1).is_empty(),"all costs commit after the batch choices");
    let clone=t.g.clone();
    for mut t in [t,T::new(clone)] {
        t.choose(0,&["Island"]).unwrap();
        assert!(tapped(&t,"Choked Estuary"));assert!(tapped(&t,"Port Town")); // the spell itself returns lands tapped
        assert!(t.g.stack.is_empty());assert_eq!(t.g.players[0].hand,vec![island]);
        cite!(["701.20c"]; assert_eq!(t.g.observations(1).len(),2,"one reveal per cost, no replay"));
        assert!(t.g.players[0].graveyard.iter().any(|o|t.g.obj(*o).chars.name=="Splendid Reclamation"));
    }
}

#[test]
fn r614_12_revealing_does_not_override_an_external_tapped_replacement() {
    let mut t=Board::new(2).hand(0,&["Port Town","Island"]).battlefield(1,&["Root Maze"]).build();
    play(&mut t,"Port Town");t.choose(0,&["Island"]).unwrap();cite!(["614.1d"]; assert!(tapped(&t,"Port Town")));
}

#[test]
fn r614_12_losing_the_entry_ability_in_the_prospective_view_removes_its_question() {
    let mut t=Board::new(2).hand(0,&["Choked Estuary","Island"]).battlefield(1,&["Blood Moon"]).build();
    play(&mut t,"Choked Estuary");cite!(["614.12"]; assert!(t.g.decision.is_none()));
    cite!(["614.12"]; assert!(!tapped(&t,"Choked Estuary")));assert!(t.g.observations(1).is_empty());
}

#[test]
fn r614_12b_reveal_and_life_payments_commit_once_after_all_entry_choices() {
    let mut t=Board::new(3).hand(0,&["Splendid Reclamation","Island"])
        .graveyard(0,&["Choked Estuary","Temple Garden","Watery Grave"]).build();
    t.g.players[0].life=3;
    t.mana(0,"{3}{G}").cast(0,"Splendid Reclamation",vec![], crate::support::mana_pool("CCCG")).unwrap().pass_all();
    t.choose(0,&["Island"]).unwrap();
    assert_eq!(t.g.players[0].life,3);assert!(t.g.observations(2).is_empty());
    t.answer(0,Answer::Yes).unwrap();
    cite!(["614.12b"]; assert_eq!(t.g.players[0].life,1,"the second shockland cannot reserve two more life"));
    assert_eq!(t.g.observations(2).len(),1);assert!(t.g.stack.is_empty());
    assert_eq!(t.g.battlefield.len(),3);
}
