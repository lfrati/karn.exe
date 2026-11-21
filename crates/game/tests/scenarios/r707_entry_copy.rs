//! Copying uses copiable characteristics, then normal entry, layers and captured abilities.
use game::{CardDef, Game, Move, ObjId, Zone};
use crate::support::{cite, paid};
use game::state::{Answer, Choice, Decision};
const COPY: &str = "You may have this creature enter as a copy of any creature on the battlefield.";
fn defs(other: CardDef) -> Vec<CardDef> {
    let ds=vec![CardDef::new("Clone","{3}{U}","Creature — Shapeshifter",COPY,Some((0,0))).unwrap(), other];
    for d in &ds { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}",d.name(),e.0)); } ds
}
fn creature(text: &str) -> CardDef { CardDef::new("Model","{1}{G}","Creature — Bear",text,Some((2,2))).unwrap() }
fn pass(g: &mut Game) { for _ in 0..g.players.len() {g.act(g.priority,Move::Pass).unwrap();} }
fn cast(g: &mut Game, spending: &str) -> ObjId {
    let card=g.put(0,0,Zone::Hand); g.players[0].pool.colors[1]=4;
    g.act(0,paid(spending, Move::Cast{card,targets:vec![]})).unwrap(); pass(g); card
}
fn choose(g: &mut Game, objects: Vec<ObjId>) {g.act(0,Move::Choose(Answer::Objects(objects))).unwrap();}
fn copy(g: &Game) -> ObjId {*g.battlefield.iter().find(|o| g.obj(**o).card==Some(0)).unwrap()}
#[test]
fn r707_2_copy_ignores_status_counters_and_later_effects_and_restores_card_on_departure() {
    let mut ds=defs(creature("Hexproof"));
    ds.push(CardDef::new("Anthem","{0}","Enchantment","Creatures you control get +1/+1.",None).unwrap());
    let mut g=Game::new(ds,2,20);
    let model=g.put(1,1,Zone::Battlefield);
    g.put(1,2,Zone::Battlefield);g.set_tapped(model,true);g.put_counters(model,"+1/+1",3);
    cast(&mut g, "UUUU");choose(&mut g,vec![model]);let copied=copy(&g);
    cite!(["707.2"]; assert_eq!(g.obj(copied).chars.name,"Model"));cite!(["707.2"]; assert_eq!(g.power(copied),Some(2)));
    cite!(["707.2"]; assert!(!g.obj(copied).tapped));cite!(["707.2"]; assert!(g.obj(copied).counters.is_empty()));
    assert_eq!(g.obj(copied).chars.mana_value,2);
    g.move_object(copied,Zone::Hand);
    let hand=g.players[0].hand[0];cite!(["400.7"]; assert_eq!(g.obj(hand).chars.name,"Clone"));
    assert_eq!(g.def(hand).unwrap().name(),"Clone");
}
#[test]
fn r707_5_copy_enters_with_counters_and_triggers_its_own_entry_once() {
    let mut g=Game::new(defs(creature("This creature enters with two +1/+1 counters on it.\nWhen this creature enters, you gain 3 life.")),2,20);
    let model=g.put(1,1,Zone::Battlefield);cast(&mut g, "UUUU");choose(&mut g,vec![model]);
    let copied=copy(&g);cite!(["707.5"]; assert_eq!(g.power(copied),Some(4)));cite!(["707.5"]; assert_eq!(g.stack.len(),1));
    pass(&mut g);assert_eq!(g.players[0].life,23);
}
#[test]
fn r707_6_copy_makes_its_own_linked_choice_and_suspends_without_entering() {
    let mut g=Game::new(defs(creature("As this creature enters, choose a color.\nWhenever you cast a spell of the chosen color, you gain 1 life.")),2,20);
    let model=g.put(1,1,Zone::Battlefield);
    let spell=cast(&mut g, "UUUU");choose(&mut g,vec![model]);
    cite!(["707.6"]; assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::Value(_)}))));
    assert_eq!(g.stack.len(),1);assert_eq!(g.obj(spell).zone,Zone::Gone);
    assert_eq!(g.battlefield,vec![model]);cite!(["614.12"]; refused("a decision is pending: a choice of how the permanent enters the battlefield (614.12)"): g.act(0,Move::Pass));
    let mut branch=g.clone();
    for game in [&mut g,&mut branch] {
        game.act(0,Move::Choose(Answer::Value(game::binding::Value::Color(ir::Color::Red)))).unwrap();
        let copied=copy(game);assert_eq!(game.def(copied).unwrap().name(),"Model");
        assert!(game.decision.is_none());assert!(game.stack.is_empty());
    }
}
#[test]
fn r707_5_copy_selection_is_optional_not_targeting_and_rejects_invalid_answers() {
    let mut g=Game::new(defs(creature("Shroud")),2,20);let model=g.put(1,1,Zone::Battlefield);
    let hand=g.put(1,1,Zone::Hand);cast(&mut g, "UUUU");
    crate::support::refused(g.act(1,Move::Choose(Answer::Objects(vec![model]))), "that decision is another player's");
    cite!(["608.2d"]; refused("a chosen object is not among the offered objects (608.2d): choose between 0 and 1 distinct matching objects"): g.act(0,Move::Choose(Answer::Objects(vec![hand]))));
    cite!(["608.2d"]; refused("each object is chosen once (608.2d): choose between 0 and 1 distinct matching objects"): g.act(0,Move::Choose(Answer::Objects(vec![model,model]))));
    let mut declined=g.clone();choose(&mut declined,vec![]);
    assert!(declined.battlefield.iter().all(|o| declined.obj(*o).card!=Some(0)));
    assert_eq!(declined.players[0].graveyard.len(),1);
    choose(&mut g,vec![model]);cite!(["707.5"]; assert_eq!(g.power(copy(&g)),Some(2)));
}
#[test]
fn r707_2_copy_of_a_copy_uses_its_copiable_baseline() {
    let mut g=Game::new(defs(creature("")),2,20);let model=g.put(1,1,Zone::Battlefield);
    cast(&mut g, "UUUU");choose(&mut g,vec![model]);let first=copy(&g);g.put_counters(first,"+1/+1",2);
    cast(&mut g, "UUUU");choose(&mut g,vec![first]);
    let second=*g.battlefield.last().unwrap();cite!(["707.2"]; assert_eq!(g.power(second),Some(2)));
    g.move_object(model,Zone::Graveyard);g.move_object(first,Zone::Graveyard);
    cite!(["707.2"]; assert_eq!(g.power(second),Some(2)));assert_eq!(g.def(second).unwrap().name(),"Model");
}

const IMAGE: &str = "You may have this creature enter as a copy of any creature on the battlefield, except it's an Illusion in addition to its other types and it has \"When this creature becomes the target of a spell or ability, sacrifice it.\"";
fn image_game() -> (Game, ObjId) {
    let ds=vec![CardDef::new("Phantasmal Image","{1}{U}","Creature — Illusion",IMAGE,Some((0,0))).unwrap(),creature(""),
        CardDef::new("Giant Growth","{G}","Instant","Target creature gets +3/+3 until end of turn.",None).unwrap(),
        CardDef::new("Healer","{0}","Creature — Human","{0}: Target creature gets +1/+1 until end of turn.",Some((1,1))).unwrap()];
    for d in &ds {game::play::check(d).unwrap_or_else(|e|panic!("{}: {}",d.name(),e.0));}
    let mut g=Game::new(ds,2,20);let model=g.put(1,1,Zone::Battlefield);
    cast(&mut g, "UU");choose(&mut g,vec![model]);let image=copy(&g);(g,image)
}
#[test]
fn r707_9a_copy_exceptions_are_copiable_and_targeting_sacrifices_only_the_copy() {
    let (mut g,image)=image_game();
    cite!(["707.9b"]; assert!(g.obj(image).chars.subtypes.iter().any(|s|s.name()=="Illusion")));
    assert!(g.obj(image).chars.subtypes.iter().any(|s|s.name()=="Bear"));
    cast(&mut g, "UU");choose(&mut g,vec![image]);let second=*g.battlefield.last().unwrap();
    let growth=g.put(0,2,Zone::Hand);g.players[0].pool.colors[4]=1;
    g.act(0,paid("G", Move::Cast{card:growth,targets:vec![vec![game::Target::Object(second)]]})).unwrap();
    assert!(g.decision.is_some(), "both copied ability instances await ordering");
    g.act(0,Move::Order(vec![0,1])).unwrap();
    cite!(["707.9a"]; assert_eq!(g.stack.len(),3,"the copied ability and the new copy exception both trigger"));
    pass(&mut g);cite!(["707.9a"]; assert!(!g.battlefield.contains(&second)));assert!(g.battlefield.contains(&image));
    pass(&mut g);pass(&mut g);assert_eq!(g.power(image),Some(2));
}
#[test]
fn r603_2d_targeting_with_an_activated_ability_triggers_the_copied_exception() {
    let (mut g,image)=image_game();let healer=g.put(1,3,Zone::Battlefield);
    g.act(0,Move::Pass).unwrap();
    g.act(1,Move::Activate{source:healer,ability:0,targets:vec![vec![game::Target::Object(image)]],paying:vec![]}).unwrap();
    cite!(["707.9a"]; assert_eq!(g.stack.len(),2));pass(&mut g);cite!(["707.9a"]; assert!(!g.battlefield.contains(&image)));pass(&mut g);
}

#[test]
fn r707_9b_an_added_card_type_is_part_of_the_next_copys_values() {
    let mut ds=defs(creature(""));
    ds[0]=CardDef::new("Artifact Copy","{0}","Creature — Shapeshifter","You may have this creature enter as a copy of any creature on the battlefield, except it's an artifact in addition to its other types.",Some((0,0))).unwrap();
    game::play::check(&ds[0]).unwrap();
    ds.push(CardDef::new("Plain Copy","{0}","Creature — Shapeshifter",COPY,Some((0,0))).unwrap());
    game::play::check(&ds[2]).unwrap();
    let mut g=Game::new(ds,2,20);let model=g.put(1,1,Zone::Battlefield);
    cast(&mut g, "");choose(&mut g,vec![model]);let first=copy(&g);
    cite!(["707.9b"]; assert!(g.is_a(first,ir::CardType::Artifact)));assert!(g.is_a(first,ir::CardType::Creature));
    let plain=g.put(0,2,Zone::Hand);
    g.act(0,Move::Cast{card:plain,targets:vec![]}).unwrap();pass(&mut g);
    choose(&mut g,vec![first]);
    cite!(["707.9b"]; assert!(g.is_a(*g.battlefield.last().unwrap(),ir::CardType::Artifact)));
}

#[test]
fn r707_2_copying_does_not_copy_layer_six_ability_removal() {
    let mut ds=defs(creature("When this creature enters, you gain 2 life."));
    ds.push(CardDef::new("Silence","{0}","Enchantment","Creatures your opponents control lose all abilities.",None).unwrap());
    for d in &ds { game::play::check(d).unwrap(); }
    let mut g=Game::new(ds,2,20);let model=g.put(1,1,Zone::Battlefield);g.put(0,2,Zone::Battlefield);
    cast(&mut g, "UUUU");choose(&mut g,vec![model]);cite!(["707.2"]; assert_eq!(g.stack.len(),1));pass(&mut g);
    cite!(["707.2"]; assert_eq!(g.players[0].life,22));
}

#[test]
fn r303_4f_a_copied_aura_chooses_its_own_attachment_before_entry() {
    let ds=vec![
        CardDef::new("Copy Artifact","{1}{U}","Enchantment","You may have this enchantment enter as a copy of any artifact on the battlefield, except it's an enchantment in addition to its other types.",None).unwrap(),
        CardDef::new("Artifact Aura","{0}","Artifact Enchantment — Aura","Enchant creature\nEnchanted creature gets +1/+1.",None).unwrap(),creature("")];
    for d in &ds {game::play::check(d).unwrap();}
    let mut g=Game::new(ds,2,20);let first=g.put(1,2,Zone::Battlefield);let second=g.put(0,2,Zone::Battlefield);
    let aura=g.put(1,1,Zone::Battlefield);assert!(g.attach(aura,game::Target::Object(first)));
    cast(&mut g, "UU");choose(&mut g,vec![aura]);
    cite!(["303.4f"]; assert!(matches!(g.decision,Some(Decision::Choose{choice:Choice::Attach{..},..}))));
    assert_eq!(g.battlefield.len(),3);cite!(["303.4f"]; refused("a decision is pending: what the entering Aura enchants (303.4f)"): g.act(0,Move::Pass));
    g.act(0,Move::Choose(Answer::Attach(game::Target::Object(second)))).unwrap();
    let copied=copy(&g);cite!(["303.4f"]; assert_eq!(g.attached_to(copied),Some(game::Target::Object(second))));
    assert_eq!((g.power(first),g.power(second)),(Some(3),Some(3)));
}

#[test]
fn r707_2_a_token_sources_printed_characteristics_and_keywords_are_copiable() {
    let mut ds=defs(creature(""));
    ds.push(CardDef::new("Make Angel","{0}","Sorcery","Create a 4/4 white Angel creature token with flying.",None).unwrap());
    let mut g=Game::new(ds,2,20);let make=g.put(0,2,Zone::Hand);
    g.act(0,Move::Cast{card:make,targets:vec![]}).unwrap();pass(&mut g);
    let angel=*g.battlefield.last().unwrap();assert!(g.obj(angel).token());
    cast(&mut g, "UUUU");choose(&mut g,vec![angel]);let copied=copy(&g);
    cite!(["707.2"]; assert_eq!(g.power(copied),Some(4)));assert!(!g.obj(copied).token());
    cite!(["707.2"]; assert_eq!(g.obj(copied).chars.colors,vec![ir::Color::White]));
    cite!(["707.2"]; assert!(g.has(copied,"Flying")));
}

#[test]
fn harness_entry_copy_origin_keeps_donor_and_does_not_make_stack_abilities_physical_cards() {
    // Uncited (protocol): copied Oracle rows and quote-added rows require different
    // portable selectors; donor metadata is a historical identity, not a live rule link.
    let (mut g,image)=image_game();
    let model=*g.battlefield.iter().find(|id|**id!=image).unwrap();
    let audit=g.audit_game_objects();
    assert_eq!(audit["objects"][image.0.to_string()]["copy_acquisition"]["donor"],serde_json::json!(model.0));
    assert_eq!(audit["objects"][image.0.to_string()]["copy_acquisition"]["producer"],serde_json::json!(1));
    assert_eq!(audit["objects"][image.0.to_string()]["copy_acquisition"]["original_rows"].as_array().unwrap().len(),0);
    assert_eq!(audit["objects"][image.0.to_string()]["definition_rows"].as_array().unwrap().len(),1);
    let spell=g.put(0,2,Zone::Hand);g.players[0].pool.colors[4]=1;
    g.act(0,paid("G", Move::Cast{card:spell,targets:vec![vec![game::Target::Object(image)]]})).unwrap();
    let audit=g.audit_game_objects();
    for id in &g.stack {
        let object=&audit["objects"][id.0.to_string()];
        if g.obj(*id).ability.is_some() { assert_eq!(object["is_card"],serde_json::json!(false));assert!(object["physical"].is_null()); }
    }
}
