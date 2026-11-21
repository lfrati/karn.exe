//! Full multi-face cards compose with dependent targets and retained copiable values.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use game::state::{Answer, Decision};
fn pass(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }
fn corpus(name: &str) -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let d = CardDef::from_faces(&catalog[name]).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0)); d
}
fn cast(g: &mut Game, index: usize, targets: Vec<Vec<Target>>, units: &str) {
    let card = g.put(0, index, Zone::Hand);
    g.players[0].pool.colors = [9; 5];
    g.act(0, paid(units, Move::Cast { card, targets })).unwrap(); pass(g);
}
#[test]
fn r701_27e_full_huntmaster_uses_each_faces_trigger_and_related_targets() {
    let bear = CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((3, 3))).unwrap();
    let turner = CardDef::new("Turner", "{0}", "Instant", "Transform target creature.", None).unwrap();
    let mut g = Game::new(vec![corpus("Huntmaster of the Fells"), bear, turner], 3, 20);
    let first = g.put(1, 1, Zone::Battlefield); let other = g.put(2, 1, Zone::Battlefield);
    cast(&mut g, 0, vec![], "GGRR"); pass(&mut g);
    let hunt = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(0)).unwrap();
    assert_eq!(g.players[0].life, 22);
    cast(&mut g, 2, vec![vec![Target::Object(hunt)]], "");
    cite!(["701.27a"]; assert_eq!(g.obj(hunt).chars.name, "Ravager of the Fells"));
    cite!(["701.27e"]; assert!(matches!(g.decision, Some(Decision::Targets { player: 0, .. }))));
    cite!(["115.1"]; refused("an illegal target, Bear: the target is one the player it names controls, and another player controls it (115.1)"): g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)], vec![Target::Object(other)]])));
    g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)], vec![Target::Object(first)]])).unwrap();
    let clone = g.clone();
    for mut g in [g, clone] {
        pass(&mut g);
        cite!(["701.27e"]; assert_eq!(g.players[1].life, 18)); assert_eq!(g.obj(first).damage, 2); assert_eq!(g.obj(other).damage, 0);
        cast(&mut g, 2, vec![vec![Target::Object(hunt)]], ""); pass(&mut g);
        assert_eq!(g.obj(hunt).chars.name, "Huntmaster of the Fells"); cite!(["701.27e"]; assert_eq!(g.players[0].life, 24));
        assert_eq!(g.battlefield.iter().filter(|o| g.obj(**o).chars.name == "Wolf Token").count(), 2);
    }
}
#[test]
fn r712_8e_full_daybreak_ranger_back_face_fights_and_front_face_targets_flyers() {
    let flyer = CardDef::new("Flyer", "{0}", "Creature — Bird", "Flying", Some((2, 3))).unwrap();
    let mut g = Game::new(vec![corpus("Daybreak Ranger"), flyer, corpus("Garruk Relentless")], 2, 20);
    let ranger = g.put(0, 0, Zone::Battlefield); let flyer = g.put(1, 1, Zone::Battlefield);
    g.act(0, Move::Activate { source: ranger, ability: 0, targets: vec![vec![Target::Object(flyer)]], paying: vec![] }).unwrap();
    pass(&mut g); cite!(["712.8d"]; assert_eq!(g.obj(flyer).damage, 2));
    g.untap(ranger); assert!(g.transform(ranger, None));
    g.players[0].pool.colors = [9; 5];
    g.act(0, paid("R", Move::Activate { source: ranger, ability: 0, targets: vec![vec![Target::Object(flyer)]], paying: vec![] })).unwrap();
    pass(&mut g); assert_eq!(g.obj(flyer).zone, Zone::Gone); cite!(["712.8e"]; assert_eq!(g.obj(ranger).damage, 2));
}
fn copied_back(double: bool) -> (Game, ObjId) {
    let front = CardDef::new("Front", "{3}{G}", "Creature", "{0}: You gain 1 life.", Some((2, 2))).unwrap();
    let back = CardDef::new("Back", "", "Creature", "{0}: You gain 3 life.", Some((4, 4))).unwrap();
    let model = CardDef::transforming(front, back).unwrap();
    let mut clone = corpus("Clone");
    if double {
        clone = CardDef::transforming(clone, CardDef::new("Other", "", "Creature", "", Some((1, 1))).unwrap()).unwrap();
    }
    let mut g = Game::new(vec![model, clone], 2, 20);
    let model = g.put(1, 0, Zone::Battlefield); assert!(g.transform(model, None));
    cast(&mut g, 1, vec![], "UUUU");
    g.act(0, Move::Choose(Answer::Objects(vec![model]))).unwrap();
    let copied = *g.battlefield.last().unwrap();
    cite!(["712.8e", "202.3b"]; assert_eq!(g.obj(copied).chars.mana_value, 0));
    cite!(["707.8"]; assert_eq!(g.def(copied).unwrap().name(), "Back")); cite!(["707.8"]; assert_eq!(g.power(copied), Some(4)));
    g.move_object(model, Zone::Graveyard);
    (g, copied)
}
#[test]
fn r707_8_an_entry_copy_keeps_only_the_current_face_and_its_executable_ability() {
    for double in [false, true] {
        let (mut g, copy) = copied_back(double);
        cite!(["701.27c"]; case: format!("double {double}"); assert_eq!(g.transform(copy, None), double));
        assert_eq!(g.obj(copy).chars.name, "Back", "physical turning does not end the copy effect");
        let clone = g.clone();
        for mut g in [g, clone] {
            g.act(0, Move::Activate { source: copy, ability: 0, targets: vec![], paying: vec![] }).unwrap();
            pass(&mut g); cite!(["707.8"]; case: format!("double {double}"); assert_eq!(g.players[0].life, 23));
            g.move_object(copy, Zone::Hand);
            let hand = g.players[0].hand[0];
            cite!(["400.7", "712.8a"]; case: format!("double {double}"); assert_eq!(g.obj(hand).chars.name, "Clone")); assert!(!g.obj(hand).transformed);
        }
    }
}
