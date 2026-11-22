//! Multiversal Passage: "As this land enters, choose a basic land type. Then you may pay 2
//! life. If you don't, it enters tapped.\nThis land is the chosen type." Both are made as it
//! enters, the type first (614.12a); its controller may decline the life, or can't pay it
//! (119.4). Being the chosen basic land type, it taps for that type's mana and nothing else
//! (305.7).
use crate::support::{cite, paid};
use game::binding::{Kind, Value};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Zone};
use ir::{Part, Subtype};

fn game(life: i32) -> (Game, ObjId) {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let passage = CardDef::from_face(faces.iter().find(|f| f.name == "Multiversal Passage").unwrap()).unwrap();
    game::play::check(&passage).unwrap();
    let defs = vec![passage, CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap()];
    let mut g = Game::new(defs, 2, life);
    for p in 0..2 { for _ in 0..5 { g.put(p, 1, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    let card = g.put(0, 0, Zone::Hand);
    (g, card)
}
fn land(name: &str) -> Value { Value::BasicLandType(Subtype::from_name(name).unwrap()) }
fn passage(g: &Game) -> ObjId { *g.battlefield.iter().find(|o| g.obj(**o).card == Some(0)).expect("it entered") }
fn subtypes(g: &Game, o: ObjId) -> Vec<String> { g.seen(o, None).subtypes.iter().map(|s| s.name().to_string()).collect() }

#[test]
fn r614_12a_the_type_is_chosen_then_the_life_paid_and_it_enters_untapped() {
    let (mut g, card) = game(20);
    g.act(0, Move::PlayLand(card)).unwrap();
    cite!(["614.12a"]; assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::Value(Kind::BasicLandType) }), "the type first"));
    cite!(["305.6"]; refused("choose a basic land type (305.6)"): g.act(0, Move::Choose(Answer::Value(Value::CreatureType(Subtype::from_name("Elf").unwrap())))));
    cite!(["305.6"]; refused("choose a basic land type (305.6)"): g.act(0, Move::Choose(Answer::Value(Value::Color(ir::Color::Blue)))));
    g.act(0, Move::Choose(Answer::Value(land("Island")))).unwrap();
    cite!(["614.12a"]; assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May { possible: true } }), "then the life"));
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    assert!(g.decision.is_none());
    let o = passage(&g);
    cite!(["119.4"]; assert_eq!(g.players[0].life, 18));
    cite!(["614.1c"]; assert!(!g.obj(o).tapped));
    assert_eq!(subtypes(&g, o), vec!["Island"]);
    // an Island's mana ability, and no other (305.7)
    assert_eq!(g.abilities(o).len(), 1);
    g.act(0, Move::Activate { source: o, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    cite!(["305.6"]; assert_eq!(g.players[0].pool.colors[1], 1, "{{U}}"));
}

#[test]
fn r614_12a_declined_it_enters_tapped_and_is_still_the_chosen_type() {
    let (mut g, card) = game(20);
    g.act(0, Move::PlayLand(card)).unwrap();
    g.act(0, Move::Choose(Answer::Value(land("Mountain")))).unwrap();
    g.act(0, Move::Choose(Answer::No)).unwrap();
    let o = passage(&g);
    assert_eq!(g.players[0].life, 20);
    cite!(["614.1c"]; assert!(g.obj(o).tapped));
    cite!(["205.1a"]; assert_eq!(subtypes(&g, o), vec!["Mountain"]));
}

#[test]
fn r119_4_without_the_life_to_pay_it_is_not_offered_and_enters_tapped() {
    let (mut g, card) = game(1);
    g.act(0, Move::PlayLand(card)).unwrap();
    g.act(0, Move::Choose(Answer::Value(land("Forest")))).unwrap();
    cite!(["119.4"]; assert!(g.decision.is_none(), "{:?}", g.decision));
    let o = passage(&g);
    assert_eq!(g.players[0].life, 1);
    cite!(["614.1c"]; assert!(g.obj(o).tapped));
    assert_eq!(subtypes(&g, o), vec!["Forest"]);
}

fn real(name: &str) -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let d = CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
    d
}
/// Cast `def` from player 0's hand with `targets`, choosing `value` as it enters.
fn cast_choosing(g: &mut Game, def: usize, targets: Vec<Vec<game::Target>>, value: Value, spending: &str) -> ObjId {
    let card = g.put(0, def, Zone::Hand);
    g.players[0].pool.colors = [5; 5];
    g.act(0, paid(spending, Move::Cast { card, targets })).unwrap();
    for _ in 0..10 {
        match g.decision.clone() {
            Some(Decision::Choose { player, choice: Choice::Value(_) }) => g.act(player, Move::Choose(Answer::Value(value.clone()))).unwrap(),
            Some(d) => panic!("{d:?}"),
            None if g.stack.is_empty() => break,
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    *g.battlefield.iter().find(|o| g.obj(**o).card == Some(def)).expect("it entered")
}

/// Convincing Mirage: "Enchanted land is the chosen type." The Forest is only a Swamp, with
/// only a Swamp's mana ability (305.7).
#[test]
fn r305_7_an_aura_makes_the_enchanted_land_the_chosen_type() {
    let defs = vec![real("Convincing Mirage"), CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap()];
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, 1, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    let forest = g.put(1, 1, Zone::Battlefield);
    cast_choosing(&mut g, 0, vec![vec![game::Target::Object(forest)]], land("Swamp"), "UU");
    cite!(["305.7"]; assert_eq!(subtypes(&g, forest), vec!["Swamp"]));
    cite!(["305.7"]; assert_eq!(g.abilities(forest).len(), 1));
}

/// Realmwright: "Lands you control are the chosen type in addition to their other types.":
/// a Forest that is also an Island has both mana abilities (305.7); an opponent's isn't.
#[test]
fn r305_7_in_addition_a_land_keeps_its_types_and_abilities() {
    let defs = vec![real("Realmwright"), CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap()];
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, 1, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    let mine = g.put(0, 1, Zone::Battlefield);
    let theirs = g.put(1, 1, Zone::Battlefield);
    cast_choosing(&mut g, 0, vec![], land("Island"), "U");
    let mut seen = subtypes(&g, mine);
    seen.sort();
    cite!(["305.7"]; assert_eq!(seen, vec!["Forest", "Island"]));
    cite!(["305.7"]; assert_eq!(g.abilities(mine).len(), 2));
    assert_eq!(subtypes(&g, theirs), vec!["Forest"]);
}

/// Adaptive Automaton: "This creature is the chosen type in addition to its other types.
/// Other creatures you control of the chosen type get +1/+1."
#[test]
fn r205_1b_a_chosen_creature_type_in_addition() {
    let defs = vec![real("Adaptive Automaton"), CardDef::new("Elf", "{0}", "Creature — Elf", "", Some((1, 1))).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap()];
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, 2, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    let elf = g.put(0, 1, Zone::Battlefield);
    let automaton = cast_choosing(&mut g, 0, vec![], Value::CreatureType(Subtype::from_name("Elf").unwrap()), "UUU");
    let mut seen = subtypes(&g, automaton);
    seen.sort();
    cite!(["205.1b"]; assert_eq!(seen, vec!["Construct", "Elf"]));
    assert_eq!(g.seen(automaton, None).pt, Some((2, 2)), "not another creature");
    cite!(["613.4c"]; assert_eq!(g.seen(elf, None).pt, Some((2, 2))));
}
