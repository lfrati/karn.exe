//! Prepared designation, persistent public copies and ordinary casting (CR722).
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use game::card::CastAs;
fn card(n:&str,k:&str,t:&str,pt:Option<(i32,i32)>)->CardDef { CardDef::new(n,"{0}",k,t,pt).unwrap() }
fn preparation()->CardDef {
    CardDef::preparation(card("Scholar","Creature — Bird","This creature enters prepared.",Some((2,2))),
        CardDef::new("Lesson","{U}","Instant","Target player gains 3 life.",None).unwrap()).unwrap()
}
fn game(extras:Vec<CardDef>)->Game { let mut ds=vec![preparation()];ds.extend(extras);for d in &ds { game::play::check(d).unwrap_or_else(|e|panic!("{}: {}",d.name(),e.0)); }Game::new(ds,2,20) }
fn resolve(g:&mut Game) { for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();} }
fn cast(g:&mut Game,c:ObjId,ts:Vec<Vec<Target>>,units:&str) {g.act(g.priority,paid(units,Move::Cast{card:c,targets:ts})).unwrap();}
fn copy(g:&Game)->ObjId { *g.exile.iter().find(|c|g.obj(**c).chars.name=="Lesson").unwrap() }
#[test]
fn r722_3c_exile_copy_persists_publicly_pays_ordinary_cost_then_unprepares() {
 let mut g=game(vec![]);let s=g.put(0,0,Zone::Battlefield);g.as_entered(s);let c=copy(&g);
 let audit=g.audit_game_objects();
 cite!(["722.3a","722.3c"];assert_eq!(audit["objects"][s.0.to_string()]["prepared_copy"],c.0));
 cite!(["722.3c"];assert_eq!(audit["objects"][c.0.to_string()]["prepare_source"],s.0));
 assert_eq!(audit["objects"][s.0.to_string()]["prepared"],true);
 assert!(audit["objects"][c.0.to_string()]["stack_copy_of"].is_null());
 cite!(["722.3a","722.3c"];assert!(g.obj(s).prepared));
 cite!(["722.3c","704.5e"];assert!(!g.obj(c).token()&&g.obj(c).card.is_none()&&g.sees(0,c)&&g.sees(1,c)));
 g.state_based_actions();cite!(["722.3c","704.5e"];assert_eq!(g.obj(c).zone,Zone::Exile));
 cite!(["722.3c","601.2f"];refused("that mana is not in your mana pool (118.3a)"):g.act(0,paid("U",Move::Cast{card:c,targets:vec![vec![Target::Player(0)]]})));
 cite!(["722.3c","601.2i"];assert!(g.obj(s).prepared&&g.obj(c).zone==Zone::Exile));
 g.players[0].pool.colors[1]=1;cast(&mut g,c,vec![vec![Target::Player(0)]],"U");
 cite!(["722.3c","601.2i"];assert!(!g.obj(s).prepared&&g.stack.len()==1));
 cite!(["722.3c","202.3"];assert_eq!(g.obj(g.stack[0]).chars.mana_value,1));
 resolve(&mut g);cite!(["608.2c"];assert_eq!(g.players[0].life,23));
 cite!(["704.5e"];assert!(g.exile.is_empty()&&g.players[0].graveyard.is_empty()));
}
#[test]
fn r722_3a_only_eligible_not_already_prepared_permanents_create_one_copy() {
 let mut g=game(vec![card("Prepare","Instant","Target creature becomes prepared.",None),card("Bear","Creature — Bear","",Some((2,2)))]);
 let s=g.put(0,0,Zone::Battlefield);g.as_entered(s);let b=g.put(0,2,Zone::Battlefield);let c=copy(&g);
 for target in [s,b] {let p=g.put(0,1,Zone::Hand);cast(&mut g,p,vec![vec![target.into()]],"");resolve(&mut g);}
 cite!(["722.3a"];assert_eq!(g.exile,vec![c]));cite!(["722.3a"];assert!(!g.obj(b).prepared));
}
#[test]
fn r722_3b_unprepare_and_source_departure_end_exile_copy_lifetime() {
 let mut g=game(vec![card("Unprepare","Instant","Target creature becomes unprepared.",None)]);
 let s=g.put(0,0,Zone::Battlefield);g.as_entered(s);let c=copy(&g);let u=g.put(0,1,Zone::Hand);
 cast(&mut g,u,vec![vec![s.into()]],"");resolve(&mut g);
 cite!(["722.3b","704.5e"];assert!(!g.obj(s).prepared&&g.obj(c).zone==Zone::Gone));
 let s2=g.put(0,0,Zone::Battlefield);g.as_entered(s2);let c2=copy(&g);g.move_object(s2,Zone::Graveyard);
 g.state_based_actions();cite!(["722.3c","704.5e"];assert_eq!(g.obj(c2).zone,Zone::Gone));
}
#[test]
fn r722_3_physical_card_cannot_cast_inset_characteristics() {
 let mut g=game(vec![]);let s=g.put(0,0,Zone::Hand);
 cite!(["722.4"];assert_eq!(g.obj(s).chars.name,"Scholar"));
 cite!(["722.3"];refused("no Adventure"):g.act(0,Move::CastAs(CastAs::Adventure,Box::new(Move::Cast{card:s,targets:vec![vec![Target::Player(0)]]}))));
 cast(&mut g,s,vec![],"");resolve(&mut g);
 cite!(["722.3a","722.3c"];assert!(g.obj(g.battlefield[0]).prepared&&g.exile.len()==1));
}
#[test]
fn r722_3c_live_controller_permission_changes_without_changing_copy_owner() {
 let mut g=game(vec![card("Steal","Instant","Gain control of target creature.",None)]);
 let s=g.put(0,0,Zone::Battlefield);g.as_entered(s);let c=copy(&g);
 // P0 causes the opponent to gain control with an independently controlled spell.
 g.act(0,Move::Pass).unwrap();let steal1=g.put(1,1,Zone::Hand);
 cast(&mut g,steal1,vec![vec![s.into()]],"");resolve(&mut g);
 cite!(["722.3c"];assert_eq!((g.controller(s),g.obj(c).owner),(1,0)));
 g.players[0].pool.colors[1]=1;g.players[1].pool.colors[1]=1;
 if g.priority!=0 {g.act(g.priority,Move::Pass).unwrap();}
 cite!(["722.3c"];refused("no rule or effect"):g.act(0,paid("U",Move::Cast{card:c,targets:vec![vec![Target::Player(0)]]})));
 g.act(0,Move::Pass).unwrap();cast(&mut g,c,vec![vec![Target::Player(1)]],"U");
 cite!(["722.3c","601.2i"];assert!(!g.obj(s).prepared&&g.obj(g.stack[0]).owner==0&&g.controller(g.stack[0])==1));
 resolve(&mut g);cite!(["608.2c"];assert_eq!(g.players[1].life,23));
}
#[test]
fn r722_3c_existing_snapshot_survives_ability_loss_and_copying_nonpreparation_card() {
 let mut g=game(vec![card("Strip","Instant","Target creature loses all abilities until end of turn.",None),
  card("Copy","Instant","Target creature becomes a copy of target creature.",None),card("Bear","Creature — Bear","",Some((4,4)))]);
 let s=g.put(0,0,Zone::Battlefield);g.as_entered(s);let c=copy(&g);let bear=g.put(0,3,Zone::Battlefield);
 let strip=g.put(0,1,Zone::Hand);cast(&mut g,strip,vec![vec![s.into()]],"");resolve(&mut g);
 cite!(["722.3c"];assert!(g.obj(s).prepared&&g.obj(c).zone==Zone::Exile));
 let copier=g.put(0,2,Zone::Hand);cast(&mut g,copier,vec![vec![s.into()],vec![bear.into()]],"");resolve(&mut g);
 cite!(["722.3c","722.2b"];assert!(g.obj(s).prepared&&g.def(s).unwrap().prepare_spell().is_none()&&g.exile==vec![c]));
 g.players[0].pool.colors[1]=1;cast(&mut g,c,vec![vec![Target::Player(0)]],"U");resolve(&mut g);
 cite!(["722.3c"];assert_eq!(g.players[0].life,23));
}
#[test]
fn r722_2b_copy_exceptions_to_normal_characteristics_do_not_modify_inset_copy() {
 let mut g=game(vec![card("Duplicate","Instant","Create a token that's a copy of target creature, except it's 1/1.",None)]);
 let s=g.put(0,0,Zone::Battlefield);g.as_entered(s);let duplicate=g.put(0,1,Zone::Hand);cast(&mut g,duplicate,vec![vec![s.into()]],"");resolve(&mut g);
 let token=*g.battlefield.iter().find(|o|g.obj(**o).token()).unwrap();
 cite!(["707.9b"];assert_eq!(g.seen(token,None).pt,Some((1,1))));
 cite!(["722.2b","722.3c"];assert!(g.obj(token).prepared&&g.exile.len()==2));
 for c in &g.exile {cite!(["722.3c"];assert_eq!((g.obj(*c).chars.name.as_str(),g.seen(*c,None).pt,g.seen(*c,None).colors),("Lesson",None,vec![ir::Color::Blue])));}
}
#[test]
fn r722_3d_copied_prepare_spell_retains_identity_without_casting_or_consuming_another_designation() {
 let watcher=card("Watcher","Creature — Wizard","Whenever you cast a prepared spell, you gain 1 life.",Some((1,1)));
 let copier=card("Copy spell","Instant","Copy target instant spell. You may choose new targets for the copy.",None);
 let mut g=game(vec![watcher,copier,card("Counter prepared","Instant","Counter target prepared spell.",None)]);let s=g.put(0,0,Zone::Battlefield);g.as_entered(s);g.put(0,1,Zone::Battlefield);let c=copy(&g);
 g.players[0].pool.colors[1]=1;cast(&mut g,c,vec![vec![Target::Player(0)]],"U");
 cite!(["722.3c","601.2i"];assert!(!g.obj(s).prepared));
 resolve(&mut g); // cast watcher, not the prepare spell
 cite!(["722.3d","603.2"];assert_eq!(g.players[0].life,21));
 let original=g.stack[0];let cp=g.put(0,2,Zone::Hand);cast(&mut g,cp,vec![vec![original.into()]],"");resolve(&mut g);
 // No new target selection is made; decline the optional change.
 if g.decision.is_some() { g.act(g.priority,Move::Choose(game::state::Answer::No)).unwrap(); }
 cite!(["707.10","722.3d"];assert_eq!(g.stack.len(),2));
 let duplicate=*g.stack.last().unwrap();let counter=g.put(0,3,Zone::Hand);
 cite!(["722.3d"];ok:g.act(0,Move::Cast{card:counter,targets:vec![vec![duplicate.into()]]}));
 resolve(&mut g);cite!(["701.5a","722.3d"];assert_eq!(g.stack,vec![original]));
 resolve(&mut g);
 cite!(["707.10","722.3d"];assert_eq!(g.players[0].life,24));
}
#[test]
fn r722_3c_prepared_source_sacrificed_during_additional_payment_does_not_abort_cast() {
 let front=card("Scholar","Creature — Bird","This creature enters prepared.",Some((2,2)));
 let spell=card("Sacrifice lesson","Instant","As an additional cost to cast this spell, sacrifice a creature.\nYou gain 3 life.",None);
 let mut g=Game::new(vec![CardDef::preparation(front,spell).unwrap()],2,20);
 let s=g.put(0,0,Zone::Hand);cast(&mut g,s,vec![],"");resolve(&mut g);let s=g.battlefield[0];let c=g.exile[0];
 cite!(["722.3c","601.2h"];ok:g.act(0,Move::WithCosts(vec![vec![s]],Box::new(Move::Cast{card:c,targets:vec![]}))));
 cite!(["601.2i","722.3c"];assert!(g.obj(s).zone==Zone::Gone&&g.stack.len()==1));
 resolve(&mut g);cite!(["608.2c"];assert_eq!(g.players[0].life,23));
}
fn real(name:&str)->CardDef {
 static FACES:std::sync::LazyLock<Vec<oracle::corpus::Face>>=std::sync::LazyLock::new(||oracle::corpus::load(&oracle::corpus::default_path()).unwrap());
 CardDef::from_faces(&CardDef::catalog(&FACES)[name]).unwrap()
}
#[test]
fn r722_3c_native_glasswright_cast_entry_and_sorcery_timing_then_treasure() {
 let d=real("Goblin Glasswright // Craft with Pride");game::play::check(&d).unwrap();let mut g=Game::new(vec![d],2,20);
 let s=g.put(0,0,Zone::Hand);g.players[0].pool.colors[3]=3;cast(&mut g,s,vec![],"RR");resolve(&mut g);
 let source=g.battlefield[0];let prepared=g.exile[0];
 cite!(["722.3a","722.3c"];assert!(g.obj(source).prepared&&g.obj(prepared).chars.name=="Craft with Pride"));
 cite!(["722.3c","722.4"];assert_eq!((g.obj(prepared).chars.mana_value,g.obj(source).chars.mana_value),(1,2)));
 g.active=1; // Fixture timing: source controller has priority in the opponent's main phase.
 cite!(["722.3c","117.1a"];refused("main phase"):g.act(0,paid("R",Move::Cast{card:prepared,targets:vec![]})));
 g.active=0;cast(&mut g,prepared,vec![],"R");resolve(&mut g);
 cite!(["722.3c","601.2i"];assert!(!g.obj(source).prepared&&g.exile.is_empty()));
 cite!(["608.2c"];assert_eq!(g.battlefield.iter().filter(|o|g.obj(**o).chars.subtypes.contains(&ir::Subtype::from_name("Treasure").unwrap())).count(),1));
}
