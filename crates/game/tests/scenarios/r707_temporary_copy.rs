//! Resolved layer-one effects retain values, durations and their own linked identities.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
fn card(name: &str, types: &str, text: &str, pt: Option<(i32,i32)>) -> CardDef {
    let d = CardDef::new(name, "{0}", types, text, pt).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}",e.0)); d
}
fn woodland() -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let d = CardDef::from_face(faces.iter().find(|f| f.name == "Shifting Woodland").unwrap()).unwrap();
    game::play::check(&d).unwrap(); d
}
fn pass(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority,Move::Pass).unwrap(); } }
fn activate(g: &mut Game, source: ObjId, ability: usize, targets: Vec<Vec<Target>>, spending: &str) {
    g.act(g.priority, paid(spending,Move::Activate { source, ability, targets, paying: vec![] })).unwrap();
}
fn cast(g: &mut Game, index: usize, targets: Vec<Vec<Target>>) {
    let card = g.put(g.priority,index,Zone::Hand);
    g.act(g.priority,Move::Cast { card, targets }).unwrap();
}
fn targets(objects: &[ObjId]) -> Vec<Vec<Target>> { objects.iter().map(|o|vec![Target::Object(*o)]).collect() }
fn next_turn(g: &mut Game) {
    let turn = g.turn;
    for _ in 0..30 {
        if g.turn != turn { return; }
        if matches!(g.decision,Some(game::state::Decision::Attackers { .. })) {
            g.act(g.active,Move::Attack(vec![])).unwrap();
        } else { pass(g); }
    }
    panic!("turn did not advance");
}
#[test]
fn r707_2_woodland_full_text_admits_and_counts_distinct_graveyard_types() {
    let ds = vec![woodland(),card("Relic","Artifact Creature — Bear","{0}: You gain 2 life.",Some((2,2))),
        card("Enchantment","Enchantment","",None),card("Forest","Basic Land — Forest","",None)];
    let mut g=Game::new(ds,2,20);
    let wood=g.put(0,0,Zone::Hand);
    g.act(0,Move::PlayLand(wood)).unwrap();
    let wood=g.battlefield[0];assert!(g.obj(wood).tapped);
    let model=g.put(0,1,Zone::Graveyard);g.put(0,1,Zone::Graveyard);g.put(0,2,Zone::Graveyard);
    g.players[0].pool.colors[4]=4;
    cite!(["602.5"]; refused("the activation condition does not hold (602.5)"): g.act(0,paid("GGGG",Move::Activate{source:wood,ability:2,targets:targets(&[model]),paying:vec![]}))); // three types, despite three cards and duplicates
    let land=g.put(0,3,Zone::Graveyard);
    activate(&mut g,wood,2,targets(&[model]),"GGGG");
    g.move_object(land,Zone::Exile); // restriction checks announcement, not resolution
    pass(&mut g);cite!(["707.2"]; assert_eq!(g.def(wood).unwrap().name(),"Relic"));
    cite!(["110.5c"]; assert!(g.obj(wood).tapped));assert_eq!(g.obj(wood).card,Some(0));
    activate(&mut g,wood,0,vec![],"");pass(&mut g);assert_eq!(g.players[0].life,22);
    next_turn(&mut g);cite!(["514.2"]; assert_eq!(g.def(wood).unwrap().name(),"Shifting Woodland"));
    assert!(!g.is_a(wood,ir::CardType::Creature));
}
#[test]
fn r614_12_woodland_forest_entry_check_and_green_mana() {
    let mut g=Game::new(vec![woodland(),card("Forest","Basic Land — Forest","",None)],2,20);
    g.put(0,1,Zone::Battlefield);let wood=g.put(0,0,Zone::Hand);g.act(0,Move::PlayLand(wood)).unwrap();
    let wood=*g.battlefield.last().unwrap();cite!(["614.12"]; assert!(!g.obj(wood).tapped));
    activate(&mut g,wood,1,vec![],"");assert_eq!(g.players[0].pool.colors[4],1);assert!(g.obj(wood).tapped);
}

const TEMP: &str = "Target creature becomes a copy of target creature until end of turn.";
fn copies() -> Vec<CardDef> {
    vec![card("Temporary","Instant",TEMP,None),
        card("Forever","Instant","Target creature becomes a copy of target creature.",None),
        card("Bear","Creature — Bear","{0}: You gain 1 life.",Some((2,2))),
        card("Bird","Creature — Bird","Flying\n{0}: You gain 3 life.",Some((3,3))),
        card("Angel","Creature — Angel","Vigilance",Some((4,4))),
        card("Make Copy","Instant","Create a token that's a copy of target creature.",None)]
}
#[test]
fn r707_2_copiable_values_exclude_status_counters_and_later_layers_and_copying_does_not_enter() {
    let mut ds=copies();ds.push(card("Growth","Instant","Target creature gets +3/+3 until end of turn.",None));
    ds.push(card("Entry","Creature — Elf","This creature enters with three +1/+1 counters on it.\nWhen this creature enters, you gain 5 life.",Some((1,1))));
    let mut g=Game::new(ds,2,20);let subject=g.put(0,2,Zone::Battlefield);let model=g.put(1,7,Zone::Battlefield);
    g.set_tapped(subject,true);g.set_tapped(model,true);g.put_counters(subject,"+1/+1",1);g.put_counters(model,"+1/+1",8);
    cast(&mut g,6,targets(&[subject]));pass(&mut g);
    cast(&mut g,0,targets(&[subject,model]));pass(&mut g);
    cite!(["707.2"]; assert_eq!(g.power(subject),Some(5)));assert_eq!(g.obj(subject).counters["+1/+1"],1);
    assert!(g.obj(subject).tapped);assert_eq!(g.players[0].life,20);cite!(["707.4"]; assert!(g.stack.is_empty()));
    cast(&mut g,5,targets(&[subject]));pass(&mut g);
    let token=*g.battlefield.last().unwrap();assert!(g.obj(token).token());assert!(!g.obj(token).tapped);
    cite!(["707.2"]; assert_eq!(g.power(token),Some(4),"own entry counters, neither model's counters nor Giant Growth"));
    pass(&mut g);assert_eq!(g.players[0].life,25,"only actual entry triggers");
    next_turn(&mut g);cite!(["514.2"]; assert_eq!(g.power(subject),Some(3)));assert_eq!(g.def(token).unwrap().name(),"Entry");
}
#[test]
fn r611_2a_overlapping_temporary_copies_restore_the_latest_permanent_copy_and_keep_stack_programs() {
    let mut g=Game::new(copies(),2,20);let bear=g.put(0,2,Zone::Battlefield);let bird=g.put(0,3,Zone::Battlefield);let angel=g.put(0,4,Zone::Battlefield);
    activate(&mut g,bear,0,vec![],"");
    cast(&mut g,1,targets(&[bear,bird]));pass(&mut g);cite!(["707.2"]; assert!(g.has(bear,"Flying")));
    pass(&mut g);cite!(["113.7a"]; assert_eq!(g.players[0].life,21,"the Bear occurrence keeps its body"));
    activate(&mut g,bear,1,vec![],"");
    cast(&mut g,0,targets(&[bear,angel]));pass(&mut g);assert!(g.has(bear,"Vigilance"));cite!(["613.7"]; assert!(!g.has(bear,"Flying")));
    pass(&mut g);assert_eq!(g.players[0].life,24,"copied Bird occurrence keeps its body");
    cast(&mut g,0,targets(&[bear,bird]));pass(&mut g);
    cast(&mut g,0,targets(&[bear,angel]));pass(&mut g);
    assert!(g.has(bear,"Vigilance"));assert!(!g.has(bear,"Flying"));
    cast(&mut g,5,targets(&[bear]));pass(&mut g);let token=*g.battlefield.last().unwrap();
    g.move_object(bird,Zone::Graveyard);g.move_object(angel,Zone::Graveyard);
    let mut branch=g.clone();
    for game in [&mut g,&mut branch] {
        next_turn(game);cite!(["611.2a"]; assert_eq!(game.def(bear).unwrap().name(),"Bird"));assert!(game.has(bear,"Flying"));
        assert!(!game.has(bear,"Vigilance"));
        cite!(["707.2"]; assert_eq!(game.def(token).unwrap().name(),"Angel"));assert!(game.has(token,"Vigilance"));
        assert!(!game.has(token,"Flying"));
        assert_eq!(game.obj(bear).card,Some(2));
        game.move_object(bear,Zone::Hand);let returned=*game.players[0].hand.last().unwrap();
        assert_eq!(game.def(returned).unwrap().name(),"Bear");
    }
}
#[test]
fn r400_7_copy_targets_and_recipients_do_not_follow_zone_changes() {
    let mut g=Game::new(copies(),2,20);let bear=g.put(0,2,Zone::Battlefield);let bird=g.put(0,3,Zone::Battlefield);
    cast(&mut g,0,targets(&[bear,bird]));g.move_object(bird,Zone::Hand);pass(&mut g);
    cite!(["608.2b", "400.7"]; assert_eq!(g.def(bear).unwrap().name(),"Bear"));
    let bird=g.put(0,3,Zone::Battlefield);cast(&mut g,0,targets(&[bear,bird]));g.move_object(bear,Zone::Hand);pass(&mut g);
    cite!(["400.7"]; assert!(g.players[0].hand.iter().any(|o|g.def(*o).unwrap().name()=="Bear")));
}
#[test]
fn r707_7_new_copy_linked_choices_are_undefined_but_existing_occurrences_keep_their_links() {
    use game::binding::Value;
    use game::state::Answer;
    let chooser=card("Chooser","Creature — Human","As this creature enters, choose a color.\n{0}: Destroy target creature of the chosen color.",Some((2,2)));
    let mut ds=copies();ds.push(chooser);ds.push(CardDef::new("Red","{R}","Creature — Goblin","",Some((1,1))).unwrap());
    let mut g=Game::new(ds,2,20);cast(&mut g,6,vec![]);pass(&mut g);
    g.act(0,Move::Choose(Answer::Value(Value::Color(ir::Color::Red)))).unwrap();let original=g.battlefield[0];
    let red=g.put(0,7,Zone::Battlefield);let model=g.put(0,6,Zone::Battlefield);
    activate(&mut g,original,1,targets(&[red]),"");
    cast(&mut g,0,targets(&[original,model]));pass(&mut g);
    cite!(["115.1"]; refused("an illegal target, Red: it doesn't match the target's description (115.1)"): g.act(0,Move::Activate{source:original,ability:1,targets:targets(&[red]),paying:vec![]})); // a new copied producer never chose Red
    pass(&mut g);cite!(["707.7", "113.7a"]; assert!(!g.battlefield.contains(&red),"original occurrence still uses its own Red choice"));
    next_turn(&mut g);assert_eq!(g.def(original).unwrap().name(),"Chooser");
    let red=g.put(g.priority,7,Zone::Battlefield);g.priority=0;
    activate(&mut g,original,1,targets(&[red]),"");pass(&mut g);assert!(!g.battlefield.contains(&red));
}
#[test]
fn r707_7_each_acquisition_has_its_own_exile_links_and_occurrences_keep_the_old_link() {
    let imprint=card("Keeper","Creature — Human","{0}: Exile target card from your graveyard.\n{0}: Return the exiled card to its owner's hand.",Some((2,2)));
    let mut ds=copies();ds.push(imprint);let mut g=Game::new(ds,2,20);
    let subject=g.put(0,2,Zone::Battlefield);let model=g.put(0,6,Zone::Battlefield);let grave=g.put(0,4,Zone::Graveyard);
    cast(&mut g,0,targets(&[subject,model]));pass(&mut g);
    activate(&mut g,subject,0,targets(&[grave]),"");pass(&mut g);assert_eq!(g.exile.len(),1);
    activate(&mut g,subject,1,vec![],"");
    cast(&mut g,0,targets(&[subject,model]));pass(&mut g);
    activate(&mut g,subject,1,vec![],"");pass(&mut g);cite!(["707.7"]; assert_eq!(g.exile.len(),1,"new acquisition has no linked exiled card"));
    pass(&mut g);assert!(g.exile.is_empty());cite!(["707.7"]; assert_eq!(g.players[0].hand.len(),1,"old occurrence retains its original exile link"));
}
#[test]
fn r707_resolved_copy_admission_refuses_unimplemented_semantics() {
    for text in [
        "Target creature becomes a copy of target creature until your next turn.",
        "Target creature becomes a copy of target creature, except it's 1/1.",
        "Target creature becomes a copy of target creature for as long as you control this creature.",
        "Target creature becomes a copy of target creature spell until end of turn.",
        "Target creature becomes a copy of each creature until end of turn.",
        "Target creature card in your graveyard becomes a copy of target creature until end of turn.",
    ] {
        let d=CardDef::new("Unsupported","{0}","Instant",text,None).unwrap();
        let reason=if text.ends_with("until your next turn.") {"a duration: Until"}
            else if text.contains("except") {"modifier: Except of Becomes"}
            else if text.contains("as long as") {"resolved copy: conditional duration"}
            else if text.starts_with("Target creature card") {"resolved copy: battlefield permanent subjects"}
            else {"resolved copy: one targeted permanent or nonstack card"};
        crate::support::refused(game::play::check(&d).map_err(|e|e.0),reason);
    }
}

#[test]
fn r707_2_managed_copy_survives_model_retirement_and_cloned_cleanup_restores_original() {
    use game::simulation::Simulation;
    fn respond(s: &mut Simulation, action: Move) {
        let r=s.request().unwrap().unwrap().clone();s.respond(r.id,r.player,action).unwrap();
    }
    fn resolve(s: &mut Simulation) { respond(s,Move::Pass);respond(s,Move::Pass); }
    let mut ds=copies();ds.push(card("Exile","Instant","Exile target creature.",None));
    let mut g=Game::new(ds,2,20);let bear=g.put(0,2,Zone::Battlefield);let bird=g.put(0,3,Zone::Battlefield);
    let copy=g.put(0,0,Zone::Hand);let exile=g.put(0,6,Zone::Hand);let maker=g.put(0,5,Zone::Hand);
    for player in 0..2 { for _ in 0..8 { g.put(player,2,Zone::Library); } }
    let mut s=Simulation::new(g);
    respond(&mut s,Move::Cast{card:copy,targets:targets(&[bear,bird])});resolve(&mut s);
    respond(&mut s,Move::Cast{card:maker,targets:targets(&[bear])});resolve(&mut s);
    let token=*s.game().unwrap().battlefield.last().unwrap();
    respond(&mut s,Move::Cast{card:exile,targets:targets(&[bird])});resolve(&mut s);
    assert_eq!(s.game().unwrap().def(bear).unwrap().name(),"Bird");
    respond(&mut s,Move::Activate{source:bear,ability:1,targets:vec![],paying:vec![]});
    let fork=s.clone();
    for mut branch in [s,fork] {
        resolve(&mut branch);assert_eq!(branch.game().unwrap().players[0].life,23);
        let turn=branch.game().unwrap().turn;
        for _ in 0..150 {
            if branch.game().unwrap().turn>=turn+3 { break; }
            let action=if matches!(branch.game().unwrap().decision,Some(game::state::Decision::Attackers{..})) {Move::Attack(vec![])} else {Move::Pass};
            respond(&mut branch,action);
        }
        assert_eq!(branch.game().unwrap().turn,turn+3);
        assert!(branch.game().unwrap().get_object(bird).is_none(),"copy values must not retain the model's departed incarnation after history expires");
        cite!(["707.2"]; assert_eq!(branch.game().unwrap().def(token).unwrap().name(),"Bird"));
        cite!(["611.2a"]; assert_eq!(branch.game().unwrap().def(bear).unwrap().name(),"Bear"));
        assert!(!branch.game().unwrap().has(bear,"Flying"));
    }
}
#[test]
fn r707_4_woodland_departure_before_resolution_does_not_copy_its_new_incarnation() {
    let mut ds=copies();ds.push(woodland());ds.push(card("Artifact Enchantment","Artifact Enchantment","",None));
    let mut g=Game::new(ds,2,20);let wood=g.put(0,6,Zone::Battlefield);let model=g.put(0,3,Zone::Graveyard);
    g.put(0,7,Zone::Graveyard);g.put(0,6,Zone::Graveyard);g.players[0].pool.colors[4]=4;
    activate(&mut g,wood,2,targets(&[model]),"GGGG");g.move_object(wood,Zone::Hand);pass(&mut g);
    assert!(g.battlefield.is_empty());cite!(["400.7"]; assert_eq!(g.def(g.players[0].hand[0]).unwrap().name(),"Shifting Woodland"));
}

#[test]
fn r712_18_transformation_under_a_temporary_copy_restores_the_current_physical_face() {
    let front=card("Front","Creature — Human","",Some((1,1)));
    let back=CardDef::new("Back","","Creature — Werewolf","Trample",Some((5,5))).unwrap();
    let mut ds=copies();ds.push(CardDef::transforming(front,back).unwrap());
    let mut g=Game::new(ds,2,20);let subject=g.put(0,6,Zone::Battlefield);let bird=g.put(0,3,Zone::Battlefield);
    cast(&mut g,0,targets(&[subject,bird]));pass(&mut g);
    assert!(g.transform(subject,None));cite!(["712.18"]; assert_eq!(g.def(subject).unwrap().name(),"Bird"));
    next_turn(&mut g);assert!(g.obj(subject).transformed);cite!(["712.18", "514.2"]; assert_eq!(g.def(subject).unwrap().name(),"Back"));assert_eq!(g.power(subject),Some(5));
}
#[test]
fn r613_7a_copied_static_abilities_receive_the_copy_effect_timestamp() {
    let mut ds=copies();ds.push(card("Blue","Creature — Human","Creatures are blue.",Some((2,2))));
    ds.push(card("Red","Enchantment","Creatures are red.",None));
    let mut g=Game::new(ds,2,20);let subject=g.put(0,2,Zone::Battlefield);let model=g.put(0,6,Zone::Battlefield);
    g.put(0,7,Zone::Battlefield);assert_eq!(g.seen(subject,None).colors,vec![ir::Color::Red]);
    cast(&mut g,0,targets(&[subject,model]));pass(&mut g);
    cite!(["613.7a"]; assert_eq!(g.seen(subject,None).colors,vec![ir::Color::Blue]));
    next_turn(&mut g);assert_eq!(g.seen(subject,None).colors,vec![ir::Color::Red]);
}
#[test]
fn r611_2c_coordinated_changes_keep_the_recipient_when_copying_changes_its_type() {
    let copy=card("Animate","Instant","Target creature becomes a copy of target land and gains flying until end of turn.",None);
    let mut g=Game::new(vec![copy,card("Bear","Creature — Bear","",Some((2,2))),card("Forest","Basic Land — Forest","",None)],2,20);
    let bear=g.put(0,1,Zone::Battlefield);let forest=g.put(0,2,Zone::Battlefield);
    cast(&mut g,0,targets(&[bear,forest]));pass(&mut g);
    assert!(g.is_a(bear,ir::CardType::Land));cite!(["611.2c"]; assert!(g.has(bear,"Flying")));
}

#[test]
fn r607_5_ending_a_temporary_copy_restores_an_older_copy_groups_linked_exile() {
    let keeper=card("Keeper","Creature — Human","{0}: Exile target card from your graveyard.\n{0}: Return the exiled card to its owner's hand.",Some((2,2)));
    let mut ds=copies();ds.push(keeper);let mut g=Game::new(ds,2,20);
    let subject=g.put(0,2,Zone::Battlefield);let model=g.put(0,6,Zone::Battlefield);let grave=g.put(0,4,Zone::Graveyard);
    cast(&mut g,1,targets(&[subject,model]));pass(&mut g);
    activate(&mut g,subject,0,targets(&[grave]),"");pass(&mut g);
    cast(&mut g,0,targets(&[subject,model]));pass(&mut g);
    activate(&mut g,subject,1,vec![],"");pass(&mut g);cite!(["607.5"]; assert_eq!(g.exile.len(),1));
    next_turn(&mut g);g.priority=0;
    activate(&mut g,subject,1,vec![],"");pass(&mut g);assert!(g.exile.is_empty());cite!(["607.5"]; assert_eq!(g.players[0].hand.len(),1));
}
#[test]
fn r603_10a_copied_death_trigger_uses_departure_abilities_after_its_model_leaves() {
    let mut ds=copies();ds.push(card("Mourner","Creature — Human","When this creature dies, you gain 7 life.",Some((2,2))));
    ds.push(card("Kill","Instant","Destroy target creature.",None));
    let mut g=Game::new(ds,2,20);let bear=g.put(0,2,Zone::Battlefield);let model=g.put(0,6,Zone::Battlefield);
    cast(&mut g,0,targets(&[bear,model]));pass(&mut g);g.move_object(model,Zone::Exile);
    cast(&mut g,7,targets(&[bear]));pass(&mut g);cite!(["603.10a"]; assert_eq!(g.stack.len(),1));pass(&mut g);
    assert_eq!(g.players[0].life,27);assert_eq!(g.def(*g.players[0].graveyard.last().unwrap()).unwrap().name(),"Kill");
}

#[test]
fn harness_old_occurrence_metadata_does_not_follow_a_new_source_definition() {
    // Uncited (protocol): the rule behavior is covered above (113.7a). Audit must
    // preserve the captured definition or explicitly refuse it; live row 0 is not
    // evidence for an occurrence captured from a different copied definition.
    let mut g=Game::new(copies(),2,20);
    let bear=g.put(0,2,Zone::Battlefield);let bird=g.put(0,3,Zone::Battlefield);
    activate(&mut g,bear,0,vec![],"");
    let occurrence=g.stack[0];
    let before=g.audit_game_objects();
    assert_eq!(before["objects"][occurrence.0.to_string()]["occurrences"][0]["definition"],serde_json::json!("Bear"));
    cast(&mut g,1,targets(&[bear,bird]));pass(&mut g);
    let after=g.audit_game_objects();
    assert!(after["objects"][occurrence.0.to_string()]["occurrences"][0]["unavailable"].is_string());
    pass(&mut g);assert_eq!(g.players[0].life,21);
}
