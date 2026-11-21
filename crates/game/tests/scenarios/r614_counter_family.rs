//! Admission witnesses for the selector families composed with counter replacement (614.1a).
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Target, Zone};
use game::state::{Answer, Choice, Decision};

fn game() -> Game {
    let defs = vec![
        CardDef::new("Mauhúr, Uruk-hai Captain", "{B}{R}", "Legendary Creature — Orc Soldier", "Menace\nIf one or more +1/+1 counters would be put on an Army, Goblin, or Orc you control, that many plus one +1/+1 counters are put on it instead.", Some((2, 2))).unwrap(),
        CardDef::new("Caradora, Heart of Alacria", "{2}{G}{W}", "Legendary Creature — Human Knight", "When Caradora enters, you may search your library for a Mount or Vehicle card, reveal it, put it into your hand, then shuffle.\nIf one or more +1/+1 counters would be put on a creature or Vehicle you control, that many plus one +1/+1 counters are put on it instead.", Some((4, 2))).unwrap(),
        CardDef::new("Kami of Whispered Hopes", "{2}{G}", "Creature — Spirit", "If one or more +1/+1 counters would be put on a permanent you control, that many plus one +1/+1 counters are put on that permanent instead.\n{T}: Add X mana of any one color, where X is this creature's power.", Some((1, 1))).unwrap(),
        CardDef::new("Conclave Mentor", "{G}{W}", "Creature — Centaur Cleric", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on that creature instead.\nWhen this creature dies, you gain life equal to its power.", Some((2, 2))).unwrap(),
        CardDef::new("Legendary counter probe", "{2}{W}", "Legendary Creature — Dog", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.\n{6}: Put a +1/+1 counter on target legendary creature.", Some((2, 2))).unwrap(),
        CardDef::new("Place", "{0}", "Instant", "Put a +1/+1 counter on target permanent.", None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Goblin", "{0}", "Creature — Goblin", "", Some((2, 2))).unwrap(),
        CardDef::new("Orc", "{0}", "Creature — Orc", "", Some((2, 2))).unwrap(),
        CardDef::new("Army", "{0}", "Creature — Army", "", Some((2, 2))).unwrap(),
        CardDef::new("Vehicle", "{0}", "Artifact — Vehicle", "", Some((2, 2))).unwrap(),
        CardDef::new("Artifact", "{0}", "Artifact", "", None).unwrap(),
        CardDef::new("Murder", "{1}{B}{B}", "Instant", "Destroy target creature.", None).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap_or_else(|e| panic!("{}: {}", def.name(), e.0)); }
    let mut game = Game::new(defs, 2, 20);
    game.players[0].pool.colors = [10; 5];
    game
}
fn resolve(g: &mut Game) {
    for _ in 0..20 {
        if g.decision.is_some() || g.stack.is_empty() { return; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("resolution did not finish");
}
fn cast(g: &mut Game, def: usize, targets: Vec<Vec<Target>>, spending: &str) {
    let card = g.put(0, def, Zone::Hand);
    g.act(0, paid(spending, Move::Cast { card, targets })).unwrap();
    resolve(g);
}
fn place(g: &mut Game, on: game::ObjId) {
    cast(g, 5, vec![vec![Target::Object(on)]], "");
}

#[test]
fn r614_1a_a_subtype_union_replaces_only_counters_on_its_controller_matching_creatures() {
    let mut g = game();
    let source = g.put(0, 0, Zone::Battlefield);
    place(&mut g, source);
    cite!(["614.1a"]; assert_eq!(g.obj(source).counters.get("+1/+1"), Some(&2)));
    for (player, def, expected) in [(0,6,1),(0,7,2),(0,8,2),(0,9,2),(1,7,1)] {
        let on = g.put(player, def, Zone::Battlefield);
        place(&mut g, on);
        cite!(["614.1a"]; case: format!("player {player} def {def}"); assert_eq!(g.obj(on).counters.get("+1/+1"), Some(&expected)));
    }
}

#[test]
fn r614_1a_a_creature_or_vehicle_replacement_includes_a_noncreature_vehicle() {
    let mut g = game();
    cast(&mut g, 1, vec![], "GGGW");
    assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::May, .. })));
    g.act(0, Move::Choose(Answer::No)).unwrap();
    for (def, expected) in [(6,2),(10,2),(11,1)] {
        let on = g.put(0, def, Zone::Battlefield);
        place(&mut g, on);
        cite!(["614.1a"]; case: format!("def {def}"); assert_eq!(g.obj(on).counters.get("+1/+1"), Some(&expected)));
    }
}

#[test]
fn r605_3b_replaced_counters_change_kamis_power_before_its_x_mana_is_computed() {
    let mut g = game();
    let kami = g.put(0, 2, Zone::Battlefield);
    place(&mut g, kami);
    cite!(["613.4c"]; assert_eq!(g.power(kami), Some(3)));
    let artifact = g.put(0, 11, Zone::Battlefield);
    place(&mut g, artifact);
    cite!(["614.1a"]; assert_eq!(g.obj(artifact).counters.get("+1/+1"), Some(&2)));
    g.players[0].pool.colors = [0; 5];
    let mana = vec![ir::mana::ManaSymbol::Color(ir::Color::Green);3];
    g.act(0, Move::Adding(vec![mana], Box::new(Move::Activate {source:kami, ability:1, targets:vec![], paying:vec![]}))).unwrap();
    cite!(["605.3b"]; assert_eq!(g.players[0].pool.colors, [0,0,0,0,3]));
}

#[test]
fn r608_2h_conclave_mentor_dies_with_the_power_its_replaced_counters_gave_it() {
    let mut g = game();
    let mentor = g.put(0, 3, Zone::Battlefield);
    place(&mut g, mentor);
    cast(&mut g, 12, vec![vec![Target::Object(mentor)]], "BBB");
    cite!(["608.2h"]; assert_eq!(g.players[0].life, 24));
}

#[test]
fn r614_1a_a_counter_replacement_also_modifies_its_own_activated_placement() {
    let mut g = game();
    let source = g.put(0, 4, Zone::Battlefield);
    g.act(0, paid("WWWWWW", Move::Activate {source, ability:1, targets:vec![vec![Target::Object(source)]], paying:vec![]})).unwrap();
    resolve(&mut g);
    cite!(["614.1a"]; assert_eq!(g.obj(source).counters.get("+1/+1"), Some(&2)));
}
