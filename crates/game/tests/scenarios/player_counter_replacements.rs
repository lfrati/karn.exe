//! 616.1: the affected player orders player-counter replacements, retaining one whole batch.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, Target, Zone};
use game::state::{Answer, Choice, Decision};
use game::operation::ReplacementId;
const PLUS: &str = "If one or more poison counters would be put on a player, that many plus one poison counters are put on that player instead.";
const TIMES: &str = "If one or more poison counters would be put on a player, twice that many poison counters are put on that player instead.";
fn setup(text: &str) -> (Game, game::ObjId, game::ObjId) {
    let defs = vec![CardDef::new("Plus", "{0}", "Enchantment",PLUS,None).unwrap(),CardDef::new("Times","{0}","Enchantment",TIMES,None).unwrap(),CardDef::new("Poison","{0}","Sorcery",text,None).unwrap()];
    for d in &defs { game::play::check(d).unwrap(); }
    let mut g = Game::new(defs,3,20);
    let plus=g.put(0,0,Zone::Battlefield); let times=g.put(0,1,Zone::Battlefield);
    (g,plus,times)
}
fn question(g:&Game,p:usize) {
    assert!(matches!(&g.decision,Some(Decision::Choose { player,choice:Choice::Replacement { on:Target::Player(on),kind,count:1,among,optional:false,.. } }) if *player==p && *on==p && kind=="poison" && among.len()==2),"{:?}",g.decision);
}
fn choose(g:&mut Game,p:usize,source:game::ObjId) {
    g.act(p,Move::Choose(Answer::Replacement(ReplacementId::from_ability(source, game::binding::AbilityKey::printed(0), 0)))).unwrap();
}
#[test]
fn r616_1_player_surface_is_typed_and_preserves_permanent_it_substitution() {
    setup("Target player gets a poison counter.");
    for text in [TIMES.replace("that player","that creature"),TIMES.replace("that player","it")] {
        let d=CardDef::new("Mismatch","{0}","Enchantment",&text,None).unwrap();
        // Uncited (admission): a player replacement must retain its typed player recipient.
        crate::support::refused(game::play::check(&d).map_err(|e|e.0), "ability on a permanent: sentence: Replacement");
    }
    let d=CardDef::new("Scales","{0}","Enchantment","If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.",None).unwrap();
    game::play::check(&d).unwrap();
}
#[test]
fn r616_1_affected_player_orders_and_cloned_questions_commit_once() {
    let (mut g,plus,times)=setup("Target player gets a poison counter.");
    let spell=g.put(0,2,Zone::Hand);
    g.act(0,Move::Cast{card:spell,targets:vec![vec![Target::Player(2)]]}).unwrap();
    for _ in 0..3 {g.act(g.priority,Move::Pass).unwrap();}
    question(&g,2);
    assert!(g.players.iter().all(|p|p.counters.is_empty()));
    let mut other=g.clone();
    choose(&mut g,2,plus); choose(&mut other,2,times);
    cite!(["616.1","616.1f"]; assert_eq!(g.counters(Target::Player(2))["poison"],4));
    cite!(["616.1","616.1f"]; assert_eq!(other.counters(Target::Player(2))["poison"],3));
    for g in [&g,&other] {
        assert!(g.stack.is_empty());
        cite!(["614.6"]; assert_eq!(g.log.iter().filter(|e|matches!(e,Event::CountersPut{on:Target::Player(2),..})).count(),1));
    }
}
#[test]
fn r616_1_mixed_player_batch_is_unwritten_until_all_apnap_choices_finish() {
    let (mut g,plus,_)=setup("Each opponent gets a poison counter.");
    let spell=g.put(0,2,Zone::Hand);
    g.act(0,Move::Cast{card:spell,targets:vec![]}).unwrap();
    for _ in 0..3 {g.act(g.priority,Move::Pass).unwrap();}
    question(&g,1); choose(&mut g,1,plus); question(&g,2);
    cite!(["101.4"]; assert!(g.players.iter().all(|p|p.counters.is_empty())));
    choose(&mut g,2,plus);
    assert!(g.counters(Target::Player(0)).is_empty());
    for p in [1,2] {cite!(["616.1"]; case: format!("player {p}"); assert_eq!(g.counters(Target::Player(p))["poison"],4));}
}
