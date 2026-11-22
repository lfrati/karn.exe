//! Multiversal Passage: the chosen type and the life as it enters by an
//! effect, under Blood Moon, and a clone's own choice (614.12, 707.2). Vesuva and
//! Thespian's Stage, which would copy Passage, aren't admitted yet.
use crate::support::{cite, paid};
use game::binding::Value;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Zone};
use ir::{Part, Subtype};

fn real(name: &str) -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let d = CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
    d
}
/// A game whose defs are `names` then an Island, in player 0's precombat main phase.
fn game(names: &[&str], life: i32) -> Game {
    let mut defs: Vec<CardDef> = names.iter().map(|n| real(n)).collect();
    defs.push(CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap());
    let island = defs.len() - 1;
    let mut g = Game::new(defs, 2, life);
    for p in 0..2 { for _ in 0..5 { g.put(p, island, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn land(name: &str) -> Value { Value::BasicLandType(Subtype::from_name(name).unwrap()) }
fn subtypes(g: &Game, o: ObjId) -> Vec<String> { g.seen(o, None).subtypes.iter().map(|s| s.name().to_string()).collect() }
fn entered(g: &Game, def: usize) -> Vec<ObjId> { g.battlefield.iter().copied().filter(|o| g.obj(*o).card == Some(def)).collect() }
fn resolve(g: &mut Game) {
    while !g.stack.is_empty() && g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
}

/// Blood Moon ruling 519e5fba (2020-08-07): a nonbasic land loses an ability that applies
/// "as" it enters, or makes it enter tapped, before it can apply (614.12). Passage asks
/// nothing, costs no life, enters untapped and is a Mountain.
#[test]
fn r614_12_under_blood_moon_passage_asks_nothing_and_enters_untapped_as_a_mountain() {
    let mut g = game(&["Multiversal Passage", "Blood Moon"], 20);
    g.put(1, 1, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::PlayLand(card)).unwrap();
    cite!(["614.12"]; assert_eq!(g.decision, None, "its 'as it enters' ability is gone before it applies"));
    let passage = entered(&g, 0)[0];
    cite!(["614.12"]; assert!(!g.obj(passage).tapped));
    assert_eq!(g.players[0].life, 20);
    cite!(["305.7"]; assert_eq!(subtypes(&g, passage), vec!["Mountain"]));
}

/// Splendid Reclamation returns two Passages at once: each asks its own type and life
/// (614.12a), the second's life can't be paid after the first's is (119.4), and both enter
/// tapped whatever is paid (ruling 3a0ec772).
#[test]
fn r614_12a_passages_put_onto_the_battlefield_together_each_choose() {
    let mut g = game(&["Multiversal Passage", "Splendid Reclamation"], 3);
    g.put(0, 0, Zone::Graveyard);
    g.put(0, 0, Zone::Graveyard);
    let spell = g.put(0, 1, Zone::Hand);
    g.players[0].pool.colors = [5; 5];
    g.act(0, paid("GGGG", Move::Cast { card: spell, targets: vec![] })).unwrap();
    resolve(&mut g);
    let mut asked = Vec::new();
    for _ in 0..8 {
        match g.decision.clone() {
            Some(Decision::Choose { player: 0, choice: Choice::Value(_) }) => {
                let pick = if asked.iter().any(|a| *a == "type") { "Forest" } else { "Swamp" };
                asked.push("type");
                g.act(0, Move::Choose(Answer::Value(land(pick)))).unwrap();
            }
            Some(Decision::Choose { player: 0, choice: Choice::May { possible: true } }) => { asked.push("life"); g.act(0, Move::Choose(Answer::Yes)).unwrap(); }
            Some(d) => panic!("{d:?}"),
            None => break,
        }
    }
    cite!(["614.12a", "614.12b"]; assert_eq!(asked, vec!["type", "life", "type"], "the second's life can't be paid"));
    let passages = entered(&g, 0);
    assert_eq!(passages.len(), 2);
    assert_eq!(g.players[0].life, 1);
    assert!(passages.iter().all(|p| g.obj(*p).tapped));
    let mut types: Vec<_> = passages.iter().flat_map(|p| subtypes(&g, *p)).collect();
    types.sort();
    cite!(["614.12a"]; assert_eq!(types, vec!["Forest", "Swamp"]));
}

/// Clone entering as a copy of Adaptive Automaton chooses a creature type for itself
/// (614.12 example: a copy of Voice of All); the choice isn't copied (707.2).
#[test]
fn r707_2_a_clone_of_adaptive_automaton_chooses_its_own_type() {
    let mut g = game(&["Adaptive Automaton", "Clone"], 20);
    g.players[0].pool.colors = [9; 5];
    let automaton = g.put(0, 0, Zone::Hand);
    g.act(0, paid("WWW", Move::Cast { card: automaton, targets: vec![] })).unwrap();
    resolve(&mut g);
    g.act(0, Move::Choose(Answer::Value(Value::CreatureType(Subtype::from_name("Elf").unwrap())))).unwrap();
    let original = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(0)).unwrap();
    let clone = g.put(0, 1, Zone::Hand);
    g.act(0, paid("UUUU", Move::Cast { card: clone, targets: vec![] })).unwrap();
    resolve(&mut g);
    let mut asked = Vec::new();
    for _ in 0..6 {
        match g.decision.clone() {
            Some(Decision::Choose { player: 0, choice: Choice::Value(_) }) => { asked.push("type"); g.act(0, Move::Choose(Answer::Value(Value::CreatureType(Subtype::from_name("Goblin").unwrap())))).unwrap(); }
            Some(Decision::Choose { player: 0, .. }) => { asked.push("copy"); g.act(0, Move::Choose(Answer::Objects(vec![original]))).unwrap(); }
            Some(d) => panic!("{d:?}"),
            None if !g.stack.is_empty() => g.act(g.priority, Move::Pass).unwrap(),
            None => break,
        }
    }
    cite!(["614.12"]; assert_eq!(asked, vec!["copy", "type"]));
    let copy = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(1)).expect("Clone entered");
    let mut seen = subtypes(&g, copy);
    seen.sort();
    cite!(["707.2"]; assert_eq!(seen, vec!["Construct", "Goblin"]));
    let mut orig = subtypes(&g, original);
    orig.sort();
    assert_eq!(orig, vec!["Construct", "Elf"]);
}
