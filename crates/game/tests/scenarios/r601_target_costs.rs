//! CR 601.2c/f: chosen targets determine the total cost, before mana production.
use crate::support::{cite,paid,mana_pool};
use game::{CardDef,Game,Move,ObjId,Target,Zone};
fn card(name:&str,cost:&str,text:&str)->CardDef{let d=CardDef::new(name,cost,"Instant",text,None).unwrap();game::play::check(&d).unwrap_or_else(|e|panic!("{name}: {e:?}"));d}
fn defs()->Vec<CardDef>{let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();let dispute=CardDef::from_face(faces.iter().find(|f|f.name=="Mystical Dispute").unwrap()).unwrap();game::play::check(&dispute).unwrap_or_else(|e|panic!("Dispute: {e:?}"));vec![dispute,card("Blue","{U}","You gain 1 life."),card("Red","{R}","You gain 1 life."),card("Colorless spell","{U}","Devoid\nYou gain 1 life.")]}
fn cast(card:ObjId,target:ObjId)->Move{Move::Cast{card,targets:vec![vec![Target::Object(target)]]}}
#[test]
fn r601_2c_listing_partitions_targets_by_the_same_cost_the_command_pays(){
 let mut g=Game::new(defs(),3,20);let blue=g.put(1,1,Zone::Stack);let red=g.put(2,2,Zone::Stack);let devoid=g.put(1,3,Zone::Stack);let dispute=g.put(0,0,Zone::Hand);g.players[0].pool.colors[1]=1;

 for target in [red,devoid]{let before=format!("{g:?}");cite!(["601.2f"]; refused("your mana pool cannot pay that cost (118.3a): {2}{U}"): g.act(0,cast(dispute,target)));assert_eq!(format!("{g:?}"),before);}
 let mut full=g.clone();full.players[0].pool.colorless=2;

 full.act(0,paid("CCU",cast(dispute,red))).unwrap();assert_eq!(full.players[0].pool.total(),0);cite!(["601.2f"]; ok: g.act(0,paid("U",cast(dispute,blue))));assert_eq!(g.players[0].pool.total(),0);
}
fn static_card(text:&str)->CardDef{let d=CardDef::new("Resource","{0}","Artifact",text,None).unwrap();game::play::check(&d).unwrap_or_else(|e|panic!("{text}: {e:?}"));d}
fn passes(g:&mut Game){for _ in 0..g.players.len(){g.act(g.priority,Move::Pass).unwrap();}}
#[test]
fn r601_2f_target_discount_combines_with_taxes_and_minimum_costs(){
 for (modifier,amount,why) in [("Noncreature spells cost {1} more to cast.",2,"your mana pool cannot pay that cost (118.3a): {1}{U} in total, {1} more and {2} less than it was (601.2f)"),("Spells cost {1} less to cast.",1,"your mana pool cannot pay that cost (118.3a): {U} in total, {2} less than it was (601.2f)"),("As long as this artifact is untapped, each spell that would cost less than three mana to cast costs three mana to cast.",3,"your mana pool cannot pay that cost (118.3a): {2}{U} in total, {2} less than it was, raised to its minimum of {3} (601.2f)")] {
  let mut cards=defs();cards.push(static_card(modifier));let mut g=Game::new(cards,2,20);g.put(1,4,Zone::Battlefield);let target=g.put(1,1,Zone::Stack);let dispute=g.put(0,0,Zone::Hand);g.players[0].pool.colors[1]=amount-1;assert!(!g.clone().act(0, cast(dispute, target)).is_ok());let before=format!("{g:?}");cite!(["601.2f"]; case: modifier; refused(why): g.act(0,cast(dispute,target)));assert_eq!(format!("{g:?}"),before);g.players[0].pool.colors[1]=amount;cite!(["601.2f"]; case: modifier; assert!(g.clone().act(0, paid(&"U".repeat(amount as usize),cast(dispute, target))).is_ok()));g.act(0,paid(&"U".repeat(amount as usize),cast(dispute,target))).unwrap();cite!(["601.2f"]; case: modifier; assert_eq!(g.players[0].pool.total(),0));
 }
}
#[test]
fn r601_2f_the_effective_target_color_is_locked_before_a_color_source_pays_mana(){
 let mut cards=defs();cards.push(static_card("Spells are blue.\nSacrifice this artifact: Add {U}."));let mut g=Game::new(cards,2,20);let source=g.put(0,4,Zone::Battlefield);let target=g.put(1,2,Zone::Stack);let dispute=g.put(0,0,Zone::Hand);assert_eq!(g.seen(target,None).colors,vec![ir::Color::Blue]);
 let plan=vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source, ability:1, paying:vec![], adding:vec![], chose:vec![], cost_branches:None }];cite!(["601.2f", "601.2g"]; ok: g.act(0,Move::WithMana(plan,Box::new(paid("U",cast(dispute,target))))));assert!(!g.battlefield.contains(&source));assert_eq!(g.seen(target,None).colors,vec![ir::Color::Red]);assert_eq!(g.players[0].pool.total(),0);passes(&mut g);g.act(1,Move::Choose(game::state::Answer::No)).unwrap();assert!(g.stack.is_empty());
}
#[test]
fn r601_2f_current_colors_override_printed_colors_and_later_changes_do_not_reprice(){
 let mut cards=defs();cards.push(card("Tint","{0}","Target spell becomes blue."));cards.push(card("Stain","{0}","Target spell becomes red."));
 let mut g=Game::new(cards,2,20);let target=g.put(1,2,Zone::Stack);let tint=g.put(0,4,Zone::Hand);g.act(0,cast(tint,target)).unwrap();passes(&mut g);assert_eq!(g.seen(target,None).colors,vec![ir::Color::Blue]);let dispute=g.put(0,0,Zone::Hand);g.players[0].pool.colors[1]=1;cite!(["601.2f"]; assert!(g.clone().act(0, paid("U",cast(dispute, target))).is_ok()));g.act(0,paid("U",cast(dispute,target))).unwrap();let stain=g.put(0,5,Zone::Hand);g.act(0,cast(stain,target)).unwrap();passes(&mut g);assert_eq!(g.seen(target,None).colors,vec![ir::Color::Red]);passes(&mut g);g.act(1,Move::Choose(game::state::Answer::No)).unwrap();assert!(g.stack.is_empty());cite!(["601.2f"]; assert_eq!(g.players[0].pool.total(),0));
}
#[test]
fn r118_12a_full_dispute_asks_only_the_target_controller_and_respects_uncounterability(){
 use game::state::{Answer,Choice,Decision};use game::simulation::Simulation;
 for (pay,uncounterable) in [(false,false),(true,false),(false,true),(true,true)]{
  let mut cards=defs();cards.push(card("Unstoppable","{U}","This spell can't be countered.\nYou gain 1 life."));let mut g=Game::new(cards,3,20);let target=g.put(2,if uncounterable{4}else{1},Zone::Stack);let dispute=g.put(0,0,Zone::Hand);g.players[0].pool.colors[1]=1;g.players[2].pool.colorless=3;g.act(0,paid("U",cast(dispute,target))).unwrap();passes(&mut g);
  cite!(["118.12a"]; case: format!("pay {pay}, uncounterable {uncounterable}"); assert!(matches!(g.decision,Some(Decision::Choose{player:2,choice:Choice::Pay{..}}))));let mut sim=Simulation::new(g);let r=sim.request().unwrap().unwrap().clone();assert_eq!(r.player,2);assert_eq!(sim.respond(r.id,0,Move::Choose(Answer::Yes)),Err(game::simulation::ResponseError::WrongPlayer{expected:2,received:0}));sim.respond(r.id,2,Move::Choose(if pay{Answer::Pay { plan: vec![], with: Some(mana_pool("CCC")), tapped: vec![] }}else{Answer::No})).unwrap();let g=sim.game().unwrap();assert_eq!(g.players[2].pool.total(),if pay{0}else{3});cite!(["118.12a", "101.2"]; case: format!("pay {pay}, uncounterable {uncounterable}"); assert_eq!(g.stack.len(),usize::from(pay||uncounterable)));
 }
}
#[test]
fn r601_target_dependent_unsupported_combinations_are_refused_before_play(){
 for (text,reason) in [
  ("This spell costs {2} less to cast if it targets a blue spell.\nCounter up to two target spells.","target-dependent cost: one unconditional required target"),
  ("This spell costs {2} less to cast if it targets a blue spell.\nCounter target spell. Target player draws a card.","target-dependent cost: one unconditional required target"),
  ("This spell costs {2} less to cast if it targets a blue spell.\nChoose one —\n• Counter target spell.\n• Draw a card.","target-dependent cost: modal target choices"),
  ("If this spell targets a blue spell, you may pay {U} rather than pay this spell's mana cost.\nCounter target spell.","alternative cost condition: announcement/payment facts not yet supplied (601.2b)"),
 ] {
  let d=CardDef::new("Unsupported","{2}{U}","Instant",text,None).unwrap_or_else(|why|panic!("{text}: {why}"));
  crate::support::refused(game::play::check(&d).map_err(|e|e.0),reason);
 }
}
fn mana_cards()->Vec<CardDef>{let mut cards=defs();cards.extend([
 CardDef::new("Mana creature","{0}","Creature — Elf","{T}: You gain 2 life. Put a +1/+1 counter on this creature. Add X mana of any one color, where X is the number of +1/+1 counters on this creature.",Some((1,1))).unwrap(),
 CardDef::new("Scales","{G}","Enchantment","If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.",None).unwrap(),
 CardDef::new("Evolution","{2}{G}","Enchantment","If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.",None).unwrap()]);cards}
fn mana_step(source:ObjId)->game::legal::Step { game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![], source,ability:0,paying:vec![],adding:vec![vec![ir::mana::ManaSymbol::Color(ir::Color::Blue);3]]}}
fn respond(sim:&mut game::simulation::Simulation,m:Move){let r=sim.request().unwrap().unwrap().clone();sim.respond(r.id,r.player,m).unwrap();}
fn replacement(sim:&mut game::simulation::Simulation,source:ObjId){let Some(game::state::Decision::Choose{choice:game::state::Choice::Replacement{among,..},..})=&sim.game().unwrap().decision else{panic!("replacement")};let effect=game::operation::ReplacementId::from_ability(source,game::binding::AbilityKey::printed(0),0);assert!(among.contains(&effect));respond(sim,Move::Choose(game::state::Answer::Replacement(effect)));}
#[test]
fn r601_2f_target_cost_survives_paused_mana_payment_and_reversal(){
 let mut g=Game::new(mana_cards(),2,20);let target=g.put(1,1,Zone::Stack);let dispute=g.put(0,0,Zone::Hand);let source=g.put(0,4,Zone::Battlefield);g.put(0,5,Zone::Battlefield);let evolution=g.put(0,6,Zone::Battlefield);let mut sim=game::simulation::Simulation::new(g);
 respond(&mut sim,Move::WithMana(vec![mana_step(source),mana_step(source)],Box::new(paid("U",cast(dispute,target)))));replacement(&mut sim,evolution);assert!(matches!(sim.game().unwrap().decision,Some(game::state::Decision::Payment{..})));
 let mut reversed=sim.clone();respond(&mut reversed,Move::ReversePayment);cite!(["733.1"]; assert_eq!(reversed.game().unwrap().players[0].hand,vec![dispute]));assert_eq!(reversed.game().unwrap().stack,vec![target]);assert_eq!(reversed.game().unwrap().players[0].pool.total(),3);assert!(reversed.game().unwrap().obj(source).tapped);
 respond(&mut sim,Move::FinishPayment{plan:vec![],paying:vec![],with:Some(mana_pool("U"))});cite!(["601.2f"]; assert_eq!(sim.game().unwrap().players[0].pool.total(),2));assert_eq!(sim.game().unwrap().obj(*sim.game().unwrap().stack.last().unwrap()).targets,vec![vec![Target::Object(target)]]);
}
#[test]
fn r118_12a_disputes_optional_payment_owns_suspended_mana_children(){
 use game::state::Answer;
 let mut g=Game::new(mana_cards(),3,20);let target=g.put(2,1,Zone::Stack);let dispute=g.put(0,0,Zone::Hand);let source=g.put(2,4,Zone::Battlefield);g.put(2,5,Zone::Battlefield);let evolution=g.put(2,6,Zone::Battlefield);g.players[0].pool.colors[1]=1;g.players[2].pool.colorless=3;g.act(0,paid("U",cast(dispute,target))).unwrap();passes(&mut g);let mut sim=game::simulation::Simulation::new(g);
 respond(&mut sim,Move::Choose(Answer::Pay{plan:vec![mana_step(source)],with:Some(mana_pool("CCC")),tapped:vec![]}));assert_eq!(sim.request().unwrap().unwrap().player,2);let mut clone=sim.clone();for sim in [&mut sim,&mut clone]{replacement(sim,evolution);assert!(sim.game().unwrap().decision.is_none());cite!(["118.12a"]; assert_eq!(sim.game().unwrap().stack,vec![target]));assert_eq!(sim.game().unwrap().players[2].pool.total(),3);assert_eq!(sim.game().unwrap().players[2].life,22);}
}
#[test]
fn r118_9_discount_modifies_the_selected_alternative_cost_and_graveyard_cost(){
 use game::announcement::CastCost;
 let text="This spell costs {2} less to cast if it targets a blue spell.\nCounter target spell unless its controller pays {3}.\nYou may pay {1}{U} rather than pay this spell's mana cost.\nFlashback {3}{U}";
 let d=card("Flexible answer","{4}{U}",text);for (zone,alternative,amount) in [(Zone::Hand,true,1),(Zone::Hand,false,3),(Zone::Graveyard,false,2)]{let mut cards=defs();cards.push(d.clone());let mut g=Game::new(cards,2,20);let target=g.put(1,1,Zone::Stack);let spell=g.put(0,4,zone);g.players[0].pool.colors[1]=amount;let command=if alternative{Move::CastFor(CastCost::Alternative(2),Box::new(cast(spell,target)))}else if zone==Zone::Graveyard {Move::CastVia(game::announcement::CastRoute::Flashback,Box::new(cast(spell,target)))}else{cast(spell,target)};g.act(0,paid(&"U".repeat(amount as usize),command)).unwrap();cite!(["601.2f"]; case: format!("{zone:?}, alternative {alternative}"); assert_eq!(g.players[0].pool.total(),0));}
}

#[test]
fn r601_2f_origin_and_target_discounts_compose_on_the_announced_cast() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let bilbo = CardDef::from_face(faces.iter().find(|f| f.name == "Bilbo, Thief in the Night").unwrap()).unwrap();
    game::play::check(&bilbo).unwrap();
    let answer = card("Grave answer", "{4}{U}",
        "This spell costs {2} less to cast if it targets a blue spell.\nCounter target spell unless its controller pays {3}.\nFlashback {3}{U}");
    for (zone, target_def, needed, why) in [(Zone::Hand, 1, 3, "your mana pool cannot pay that cost (118.3a): {2}{U} in total, {2} less than it was (601.2f)"),
                                          (Zone::Hand, 2, 5, "your mana pool cannot pay that cost (118.3a): {4}{U}"),
                                          (Zone::Graveyard, 1, 1, "your mana pool cannot pay that cost (118.3a): {U} in total, {3} less than it was (601.2f)"),
                                          (Zone::Graveyard, 2, 3, "your mana pool cannot pay that cost (118.3a): {2}{U} in total, {1} less than it was (601.2f)")] {
        let mut cards = defs();
        cards.extend([bilbo.clone(), answer.clone()]);
        let mut g = Game::new(cards, 2, 20);
        g.put(0, 4, Zone::Battlefield);
        let target = g.put(1, target_def, Zone::Stack);
        let spell = g.put(0, 5, zone);
        g.players[0].pool.colors[1] = needed - 1;
        cite!(["601.2f"]; case: why; refused(why): g.act(0, if zone==Zone::Graveyard {Move::CastVia(game::announcement::CastRoute::Flashback,Box::new(cast(spell,target)))} else {cast(spell,target)}));
        g.players[0].pool.colors[1] = needed;
        g.act(0, paid(&"U".repeat(needed as usize),if zone==Zone::Graveyard {Move::CastVia(game::announcement::CastRoute::Flashback,Box::new(cast(spell,target)))} else {cast(spell,target)})).unwrap();
        cite!(["601.2f"]; case: why; assert_eq!(g.players[0].pool.total(), 0, "{zone:?} targeting definition {target_def}"));
    }
}
