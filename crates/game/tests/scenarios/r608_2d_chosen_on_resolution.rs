//! An object an instruction is done to, described but not targeted ("put a +1/+1 counter on a
//! creature you control", "return a creature card from your graveyard to your hand"), is chosen
//! as the instruction is followed by the player who follows it, among the objects the action can
//! be done to (608.2d); with none, the instruction does nothing (609.3).
use crate::support::cite;
use game::{CardDef,Game,Move,ObjId,Target,Zone};
use game::state::{Answer,Choice,Decision};
const EFFECT:usize=0;const BEAR:usize=1;const HEXPROOF:usize=2;const ROCK:usize=3;
fn card(name:&str,ty:&str,text:&str,pt:Option<(i32,i32)>)->CardDef{CardDef::new(name,"{0}",ty,text,pt).unwrap()}
fn game(text:&str)->Game{
 let ds=vec![card("Effect","Sorcery",text,None),card("Bear","Creature — Bear","",Some((2,2))),
  card("Wisp","Creature — Spirit","Hexproof",Some((1,1))),card("Rock","Artifact","",None)];
 for d in &ds{game::play::check(d).unwrap_or_else(|e|panic!("{}: {}",d.name(),e.0));}Game::new(ds,2,20)
}
fn cast(g:&mut Game,targets:Vec<Vec<Target>>){let card=g.put(0,EFFECT,Zone::Hand);g.act(0,Move::Cast{card,targets}).unwrap();for _ in 0..2{g.act(g.priority,Move::Pass).unwrap();}}
fn among(g:&Game)->Vec<ObjId>{
 let Some(Decision::Choose{player:0,choice:Choice::Objects{among,count:1,..}})=&g.decision else{panic!("{:?}",g.decision)};among.clone()
}
fn choose(g:&mut Game,p:usize,o:ObjId)->Result<(),String>{g.act(p,Move::Choose(Answer::Objects(vec![o]))).map(|_|())}
fn counters(g:&Game,o:ObjId)->u32{g.obj(o).counters.get("+1/+1").copied().unwrap_or(0)}

#[test]
fn r608_2d_a_counter_goes_on_a_creature_chosen_on_resolution_not_targeted(){
 // none: nothing happens and nothing is asked
 let mut g=game("Put a +1/+1 counter on a creature you control.");let theirs=g.put(1,BEAR,Zone::Battlefield);
 cast(&mut g,vec![]);assert!(g.decision.is_none());cite!(["609.3"]; assert_eq!(counters(&g,theirs),0));
 // a hexproof creature and a token can be chosen; an opponent's creature can't; only you choose
 let mut g=game("Put a +1/+1 counter on a creature you control.");let bear=g.put(0,BEAR,Zone::Battlefield);
 let wisp=g.put(0,HEXPROOF,Zone::Battlefield);let theirs=g.put(1,BEAR,Zone::Battlefield);
 let token=g.create_token(0,g.obj(bear).chars.clone(),false);g.put(0,ROCK,Zone::Battlefield);
 cast(&mut g,vec![]);let mut options=among(&g);options.sort();let mut expected=vec![bear,wisp,token];expected.sort();
 cite!(["608.2d"]; assert_eq!(options,expected));cite!(["608.2d"]; refused("a chosen object is not among the offered objects to put (608.2d)"): choose(&mut g,0,theirs));crate::support::refused(choose(&mut g,1,wisp), "that decision is another player's");
 let mut copy=g.clone();choose(&mut g,0,wisp).unwrap();choose(&mut copy,0,token).unwrap();
 cite!(["608.2d"]; assert_eq!((counters(&g,wisp),counters(&g,bear),counters(&g,token)),(1,0,0)));
 cite!(["608.2d"]; assert_eq!((counters(&copy,wisp),counters(&copy,token)),(0,1)));
}

#[test]
fn r608_2d_a_spell_with_a_target_chooses_the_other_object_only_as_it_resolves(){
 let mut g=game("Destroy target artifact. Put a +1/+1 counter on a creature you control.");
 let rock=g.put(1,ROCK,Zone::Battlefield);let bear=g.put(0,BEAR,Zone::Battlefield);let wisp=g.put(0,HEXPROOF,Zone::Battlefield);
 cast(&mut g,vec![vec![Target::Object(rock)]]);cite!(["608.2c"]; assert!(!g.battlefield.contains(&rock),"the target is destroyed first"));
 let mut options=among(&g);options.sort();cite!(["608.2d"]; assert_eq!(options,vec![bear,wisp]));choose(&mut g,0,bear).unwrap();
 assert_eq!(counters(&g,bear),1);
}

#[test]
fn r608_2d_a_creature_to_destroy_can_be_any_players_and_untargetable(){
 let mut g=game("Destroy a creature.");let bear=g.put(0,BEAR,Zone::Battlefield);let wisp=g.put(1,HEXPROOF,Zone::Battlefield);
 cast(&mut g,vec![]);let mut options=among(&g);options.sort();cite!(["608.2d"]; assert_eq!(options,vec![bear,wisp]));
 choose(&mut g,0,wisp).unwrap();assert!(g.battlefield.contains(&bear)&&!g.battlefield.contains(&wisp));
}

#[test]
fn r608_2d_a_card_is_chosen_from_your_graveyard_among_those_the_description_allows(){
 let mut g=game("Return a creature card from your graveyard to your hand.");g.put(1,BEAR,Zone::Graveyard);g.put(0,ROCK,Zone::Graveyard);
 cast(&mut g,vec![]);assert!(g.decision.is_none());cite!(["609.3"]; assert!(g.players[0].hand.is_empty()));assert_eq!(g.players[1].graveyard.len(),1);
 let mut g=game("Return a creature card from your graveyard to your hand.");let bear=g.put(0,BEAR,Zone::Graveyard);
 let wisp=g.put(0,HEXPROOF,Zone::Graveyard);let theirs=g.put(1,BEAR,Zone::Graveyard);g.put(0,ROCK,Zone::Graveyard);
 cast(&mut g,vec![]);let mut options=among(&g);options.sort();cite!(["608.2d"]; assert_eq!(options,vec![bear,wisp]));
 cite!(["608.2d"]; refused("a chosen object is not among the offered objects to return (608.2d)"): choose(&mut g,0,theirs));choose(&mut g,0,wisp).unwrap();
 cite!(["608.2d"]; assert_eq!(g.players[0].hand.len(),1));assert!(g.players[0].graveyard.contains(&bear));assert!(g.players[1].graveyard.contains(&theirs));
}

#[test]
fn r608_2d_a_card_from_any_graveyard_comes_under_your_control(){
 let mut g=game("Put a creature card from a graveyard onto the battlefield under your control.");
 let mine=g.put(0,BEAR,Zone::Graveyard);let theirs=g.put(1,HEXPROOF,Zone::Graveyard);
 cast(&mut g,vec![]);let mut options=among(&g);options.sort();cite!(["608.2d"]; assert_eq!(options,vec![mine,theirs]));
 choose(&mut g,0,theirs).unwrap();
 let [wisp]=g.battlefield.iter().copied().filter(|o|g.obj(*o).chars.name=="Wisp").collect::<Vec<_>>()[..] else{panic!()};
 cite!(["110.2"]; assert_eq!((g.obj(wisp).controller,g.obj(wisp).owner),(0,1)));
}

#[test]
fn r608_2d_if_you_do_follows_whether_a_card_could_be_chosen(){
 for cards in [0,1]{
  let mut g=game("Exile a creature card from your graveyard. If you do, you gain 3 life. Otherwise, you lose 3 life.");
  for _ in 0..cards{g.put(0,BEAR,Zone::Graveyard);}g.put(0,ROCK,Zone::Graveyard);cast(&mut g,vec![]);
  assert!(g.decision.is_none());cite!(["118.12"]; case: format!("{cards}"); assert_eq!(g.players[0].life,if cards==1{23}else{17}));assert!(g.players[0].graveyard.iter().all(|o|g.obj(*o).card!=Some(BEAR)));
 }
}

fn pinned(names:&[&str])->Game{
 let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();let catalog=CardDef::catalog(&faces);
 let mut ds:Vec<CardDef>=names.iter().map(|n|CardDef::from_faces(catalog.get(*n).unwrap()).unwrap()).collect();
 ds.extend([card("Bear","Creature — Bear","",Some((2,2))),CardDef::new("Big Spell","{3}{U}{U}","Instant","",None).unwrap(),card("Land","Land","",None)]);
 for d in &ds{game::play::check(d).unwrap_or_else(|e|panic!("{}: {}",d.name(),e.0));}Game::new(ds,2,20)
}
fn resolve(g:&mut Game){for _ in 0..2{g.act(g.priority,Move::Pass).unwrap();}}
/// Play on until an ability is on the stack, attacking with nothing.
fn upkeep(g:&mut Game){while g.stack.is_empty(){
 let mv=if let Some(Decision::Attackers{player})=g.decision{(player,Move::Attack(vec![]))}else{(g.priority,Move::Pass)};g.act(mv.0,mv.1).unwrap();
}}

#[test]
fn r608_2d_egon_exiles_two_cards_only_if_it_can_otherwise_it_is_sacrificed(){
 // "If there's only one card in your graveyard, you won't exile it" (Egon ruling, 2021-02-05)
 for cards in [0,1,2,3]{
  let mut g=pinned(&["Egon, God of Death"]);let egon=g.put(0,0,Zone::Battlefield);for p in 0..2{for _ in 0..4{g.put(p,1,Zone::Library);}}
  let grave:Vec<_>=(0..cards).map(|_|g.put(0,1,Zone::Graveyard)).collect();
  upkeep(&mut g);let hand=g.players[0].hand.len();
  resolve(&mut g);
  if cards==3{let Some(Decision::Choose{player:0,choice:Choice::Objects{among,count:2,..}})=&g.decision else{panic!("{:?}",g.decision)};
   cite!(["608.2d"]; assert_eq!(among,&grave));g.act(0,Move::Choose(Answer::Objects(vec![grave[0],grave[2]]))).unwrap();}
  let kept=g.battlefield.contains(&egon);cite!(["118.12"]; case: format!("{cards}"); assert_eq!(kept,cards>=2,"{cards} cards"));
  assert_eq!(g.players[0].graveyard.len(),if cards>=2{cards-2}else{cards+1},"{cards} cards; Egon itself goes there when sacrificed");
  assert_eq!(g.players[0].hand.len(),hand+usize::from(!kept));
  if cards==3{assert!(g.players[0].graveyard.contains(&grave[1]));}
 }
}

#[test]
fn r608_2d_the_chosen_card_is_the_one_later_instructions_and_linked_abilities_mean(){
 // Torrent Sculptor: "that card's mana value" is the chosen card's, 5 halved and rounded up
 // (the front face alone: Flamethrower Sonata's text is not admitted)
 let front=CardDef::new("Torrent Sculptor","{2}{U}{U}","Creature — Merfolk Wizard","Ward {2}\nWhen this creature enters, exile an instant or sorcery card from your graveyard. Put a number of +1/+1 counters on this creature equal to half that card's mana value, rounded up.",Some((2,2))).unwrap();
 let mut g=pinned(&[]);let mut ds=g.defs.to_vec();ds.push(front);g=Game::new(ds,2,20);
 let big=g.put(0,1,Zone::Graveyard);let bear=g.put(0,0,Zone::Graveyard);
 let sculptor=g.put(0,3,Zone::Hand);g.move_object(sculptor,Zone::Battlefield);let sculptor=*g.battlefield.last().unwrap();
 upkeep(&mut g);resolve(&mut g);
 assert!(g.decision.is_none(),"only an instant or sorcery card can be chosen: {:?}",g.decision);
 cite!(["608.2d"]; assert!(!g.players[0].graveyard.contains(&big)));assert!(g.players[0].graveyard.contains(&bear));
 // Uncited (card text): "equal to half that card's mana value, rounded up": that card is the exiled instant (mana value 5), not the Bear
 assert_eq!(counters(&g,sculptor),3);
 // Wormfang Newt: the land chosen on entry is "the exiled card" its leaves ability returns (607.2a)
 let mut g=pinned(&["Wormfang Newt"]);let a=g.put(0,3,Zone::Battlefield);let b=g.put(0,3,Zone::Battlefield);
 let newt=g.put(0,0,Zone::Hand);g.move_object(newt,Zone::Battlefield);let newt=*g.battlefield.last().unwrap();
 upkeep(&mut g);resolve(&mut g);
 let mut options=among(&g);options.sort();assert_eq!(options,vec![a,b]);choose(&mut g,0,b).unwrap();
 assert!(g.battlefield.contains(&a)&&!g.battlefield.contains(&b));
 g.move_object(newt,Zone::Graveyard);upkeep(&mut g);resolve(&mut g);
 let lands=g.battlefield.iter().filter(|o|g.obj(**o).card==Some(3)).count();cite!(["607.2a"]; assert_eq!(lands,2));
}

#[test]
fn r608_2d_choices_by_other_players_or_optional_ones_stay_refused(){
 for text in ["You may put a +1/+1 counter on a creature you control.",
  "Each player returns a creature card from their graveyard to their hand.",
  "Target opponent exiles a creature card from their graveyard.",
  // several object groups: each would be chosen separately
  "Return a creature card from your graveyard to your hand and a land card from your graveyard to the battlefield."]{
  let reason=if text.starts_with("You may") {"selected object: one mandatory choice by the controller"}
    else if text.starts_with("Each") {"selected object: a choice by several players"}
    else if text.starts_with("Target") {"noun phrase: an unsupported zone or zone owner"}
    else {"selected object: one description"};
  crate::support::refused(game::play::check(&card("Effect","Sorcery",text,None)).map_err(|e|e.0),reason);
 }
}
