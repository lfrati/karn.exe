//! Fixed voluntary counts are all chosen publicly in APNAP order before any draw (101.4b,
//! 608.2e, 121.2c). Empty libraries do not narrow the offered domain (121.3a).
use crate::support::{cite, paid};
use game::{CardDef, Event, Game, Move, Target, Zone};
use game::binding::Value;
use game::state::{Answer, Choice, Decision, PublicChoice};

fn def(name: &str, kind: &str, text: &str, pt: Option<(i32,i32)>) -> CardDef {
    CardDef::new(name,"{0}",kind,text,pt).unwrap()
}
fn setup(text: &str, players: usize) -> Game {
    let d=def("Bounded draw","Sorcery",text,None);
    game::play::check(&d).unwrap();
    Game::new(vec![d,def("Blank","Land","",None),
        def("Stinkweed Imp","Creature — Imp","Flying\nWhenever this creature deals combat damage to a creature, destroy that creature.\nDredge 5",Some((1,2)))],players,20)
}
fn library(g: &mut Game,p: usize,n: usize) {for _ in 0..n {g.put(p,1,Zone::Library);}}
fn passes(g: &mut Game) {for _ in 0..g.players.len() {g.act(g.priority,Move::Pass).unwrap();}}
fn start(g:&mut Game) {let card=g.put(0,0,Zone::Hand);g.act(0,Move::Cast{card,targets:vec![]}).unwrap();passes(g);}
fn asks(p:usize,max:u32)->Option<Decision> {Some(Decision::Choose{player:p,choice:Choice::Number{min:0,max}})}
fn ask(g:&Game,p:usize,max:u32) {assert_eq!(g.decision,asks(p,max));}
fn number(g:&mut Game,p:usize,n:u32) {g.act(p,Move::Choose(Answer::Value(Value::Number(n)))).unwrap();}
fn draws(g:&Game)->Vec<usize> {g.log.iter().filter_map(|e|if let Event::Drew{player,..}=e{Some(*player)}else{None}).collect()}
fn choices(g:&Game)->Vec<(usize,u32)> {g.public_choices().filter_map(|c|if let PublicChoice::Value{player,value:Value::Number(n),..}=c {Some((*player,*n))}else{None}).collect()}

#[test]
fn all_counts_are_public_before_next_chooser_and_draws_wait_for_everyone() {
    let mut g=setup("Each player may draw up to three cards.",3);
    for p in 0..3 {library(&mut g,p,6);}
    start(&mut g);ask(&g,0,3);
    number(&mut g,0,2);cite!(["101.4"]; assert_eq!(g.decision,asks(1,3)));
    cite!(["101.4b"]; assert_eq!(choices(&g),[(0,2)]));cite!(["608.2e"]; assert!(draws(&g).is_empty()));
    let mut clone=g.clone();
    for branch in [&mut g,&mut clone] {
        number(branch,1,0);ask(branch,2,3);
        assert_eq!(choices(branch),[(0,2),(1,0)]);assert!(draws(branch).is_empty());
        number(branch,2,3);cite!(["121.2c"]; assert_eq!(draws(branch),[0,0,2,2,2]));
        assert_eq!(branch.players.iter().map(|p|p.hand.len()).collect::<Vec<_>>(),[2,0,3]);
    }
}

#[test]
fn invalid_numeric_answers_are_atomic_and_zero_does_not_attempt_a_draw() {
    let mut g=setup("Draw up to three cards.",2);start(&mut g);ask(&g,0,3);
    for (p,answer) in [(1,Answer::Value(Value::Number(1))),(0,Answer::Value(Value::Number(4))),
        (0,Answer::Yes),(0,Answer::No),(0,Answer::Value(Value::CardType("creature".into())))] {
        let before=format!("{g:?}");if p==1{crate::support::refused(g.act(p,Move::Choose(answer)), "that decision is another player's");}else{cite!(["608.2d"]; case: format!("{answer:?}"); refused("choose an integer within the offered numeric bounds (608.2d)"): g.act(p,Move::Choose(answer)));}assert_eq!(format!("{g:?}"),before);
    }
    number(&mut g,0,0);cite!(["121.2"]; assert!(draws(&g).is_empty()));assert!(!g.players[0].lost);
}

#[test]
fn choosing_three_from_an_empty_or_one_card_library_still_attempts_three() {
    for n in [0,1] {let mut g=setup("You may draw up to three cards.",2);library(&mut g,0,n);start(&mut g);
        cite!(["121.3"]; case: n; assert_eq!(g.decision,asks(0,3)));number(&mut g,0,3);cite!(["104.3c"]; case: n; assert_eq!(draws(&g).len(),n));cite!(["104.3c"]; case: n; assert!(g.players[0].lost));}
}

#[test]
fn dredge_child_and_its_clone_resume_the_chosen_count_without_asking_again() {
    let mut g=setup("Draw up to three cards.",2);library(&mut g,0,8);let imp=g.put(0,2,Zone::Graveyard);start(&mut g);number(&mut g,0,2);
    let Some(Decision::Choose{player:0,choice:Choice::Replacement{among,..}})=&g.decision else {panic!("{:?}",g.decision)};
    let replacement=among.iter().find(|r|r.source==imp).unwrap().clone();
    assert_eq!(choices(&g),[(0,2)]);let mut clone=g.clone();
    let milled = g.players[0].library.iter().rev().take(5).copied().collect::<Vec<_>>();
    g.act(0,Move::Choose(Answer::Replacement(replacement))).unwrap();
    assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::OrderCards { to: game::state::Placement::Zone(Zone::Graveyard), cards: milled.clone() } }));
    g.act(0, Move::Choose(Answer::Objects(vec![milled[4],milled[3],milled[2],milled[1],milled[0]]))).unwrap();
    cite!(["614.11a"]; assert_eq!(draws(&g),[0]));cite!(["702.52a"]; assert_eq!(g.players[0].hand.len(),2));
    clone.act(0,Move::Choose(Answer::No)).unwrap();
    cite!(["121.2"]; assert!(matches!(clone.decision,Some(Decision::Choose{choice:Choice::Replacement{..},..}))));
    clone.act(0,Move::Choose(Answer::No)).unwrap();assert_eq!(draws(&clone),[0,0]);
}

#[test]
fn full_djinn_captured_controller_differs_from_active_player_and_source_can_depart() {
    let djinn=CardDef::new("Indentured Djinn","{1}{U}{U}","Creature — Djinn","Flying\nWhen this creature enters, each other player may draw up to three cards.",Some((4,4))).unwrap();
    game::play::check(&djinn).unwrap();
    let mut g=Game::new(vec![djinn,def("Blank","Land","",None),
        CardDef::new("Leyline of Anticipation","{2}{U}{U}","Enchantment","If this card is in your opening hand, you may begin the game with it on the battlefield.\nYou may cast spells as though they had flash.",None).unwrap(),
        def("Departure","Instant","Exile target creature.",None)],3,20);
    for p in 0..3 {library(&mut g,p,5);}
    g.put(1,2,Zone::Battlefield);g.act(0,Move::Pass).unwrap();
    g.players[1].pool.colors[ir::Color::Blue as usize]=3;
    let card=g.put(1,0,Zone::Hand);g.act(1,paid("UUU",Move::Cast{card,targets:vec![]})).unwrap();passes(&mut g);
    assert_eq!(g.active,0);assert_eq!(g.stack.len(),1);
    let djinn=*g.battlefield.iter().find(|o|g.def(**o).unwrap().name()=="Indentured Djinn").unwrap();
    let departure=g.put(0,3,Zone::Hand);g.act(0,Move::Cast{card:departure,targets:vec![vec![Target::Object(djinn)]]}).unwrap();passes(&mut g);
    assert!(!g.battlefield.contains(&djinn));passes(&mut g);cite!(["113.7a","101.4"]; assert_eq!(g.decision,asks(0,3)));
    number(&mut g,0,1);cite!(["101.4"]; assert_eq!(g.decision,asks(2,3)));cite!(["101.4b"]; assert_eq!(choices(&g),[(0,1)]));number(&mut g,2,2);
    cite!(["121.2c"]; assert_eq!(draws(&g),[0,2,2]));assert!(g.players[1].hand.is_empty());
}

#[test]
fn standalone_fixed_bound_admits_but_outcomes_delays_and_variable_bounds_do_not() {
    for text in ["Draw up to three cards.","Each player draws up to two cards.","Target player may draw up to two cards.","Draw three cards.","Each player may draw a card."] {
        game::play::check(&def("Procedure","Sorcery",text,None)).unwrap_or_else(|e|panic!("{text}: {e:?}"));
    }
    for text in ["You may draw up to three cards. If you do, you gain 1 life.","Draw up to three cards and gain 1 life.","Draw up to X cards.","Draw up to two cards at the beginning of the next turn's upkeep."] {
        let reason=if text=="Draw up to X cards." {"draw up to: fixed numeric bound required"}else{"draw up to: standalone fixed-count action required"};
        crate::support::refused(game::play::check(&def("Procedure","Sorcery",text,None)).map_err(|e|e.0),reason);
    }
}

#[test]
fn target_player_and_zero_bound_use_the_same_numeric_policy() {
    let mut g=setup("Target player may draw up to two cards.",2);library(&mut g,1,3);
    let card=g.put(0,0,Zone::Hand);g.act(0,Move::Cast{card,targets:vec![vec![Target::Player(1)]]}).unwrap();passes(&mut g);
    ask(&g,1,2);number(&mut g,1,2);assert_eq!(draws(&g),[1,1]);
    let mut g=setup("Draw up to zero cards.",2);start(&mut g);ask(&g,0,0);
    number(&mut g,0,0);assert!(!g.players[0].lost);assert!(draws(&g).is_empty());
}

#[test]
fn ordinary_optional_draw_keeps_yes_no_and_numeric_does_not_admit_prohibitions() {
    for yes in [true,false] {
        let mut g=setup("Each player may draw a card.",2);for p in 0..2 {library(&mut g,p,2);}start(&mut g);
        for p in 0..2 {cite!(["101.4"]; case: p; assert_eq!(g.decision,Some(Decision::Choose{player:p,choice:Choice::May})));
            g.act(p,Move::Choose(if yes {Answer::Yes}else{Answer::No})).unwrap();}
        assert_eq!(draws(&g),if yes{vec![0,1]}else{vec![]});
    }
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();let catalog=CardDef::catalog(&faces);
    for name in ["Maralen of the Mornsong","Mornsong Aria","Omen Machine","Spirit of the Labyrinth","Narset, Parter of Veils","Leovold, Emissary of Trest","Arcane Denial"] {
        let card=CardDef::from_faces(&catalog[name]).unwrap();
        let refusal=game::play::check(&card).expect_err(name);
        assert!(refusal.0.contains("prohibition") || name=="Arcane Denial" && refusal.0.contains("draw up to"),"{name}: {}",refusal.0);
    }
}
