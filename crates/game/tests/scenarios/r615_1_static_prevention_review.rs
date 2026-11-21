//! Static prevention identity, changed/granted abilities and source LKI.
use crate::support::cite::cite;
use game::{CardDef, Game, Move, Target, Zone};
use game::state::{Answer, Choice, Decision};
use game::operation::DamageOption;

fn game() -> Game {
    let c = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).unwrap();
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap();
    let defs = vec![
        c("Two guards", "Enchantment", "Prevent all damage that would be dealt to you.\nPrevent all noncombat damage that would be dealt to you.", None),
        c("Guard", "Enchantment", "Prevent all damage that would be dealt to you.", None),
        c("Shield", "Instant", "Prevent the next 3 damage that would be dealt to target player this turn.", None),
        c("Hit", "Instant", "Hit deals 2 damage to target player.", None),
        c("Creature guard", "Enchantment", "Prevent all damage that would be dealt to you by creatures.", None),
        real("Mogg Fanatic"),
        real("Caduceus, Staff of Hermes"),
        c("Bear", "Creature — Bear", "", Some((2, 2))),
        c("Bolt", "Instant", "Bolt deals 2 damage to target creature.", None),
        c("Remove", "Instant", "Target creature loses all abilities until end of turn.", None),
        c("Gift", "Enchantment", "Creatures you control have \"Prevent all damage that would be dealt to this creature.\"", None),
        c("Creature shield", "Instant", "Prevent the next 3 damage that would be dealt to target creature this turn.", None),
        c("Mountain", "Basic Land — Mountain", "", None),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, 12, Zone::Library); } }
    g
}
fn resolve(g: &mut Game) {
    for _ in 0..30 {
        if g.stack.is_empty() || g.decision.is_some() { return; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("stack did not settle");
}
fn cast(g: &mut Game, def: usize, target: Target) {
    let card = g.put(0, def, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![vec![target]] }).unwrap();
    resolve(g);
}

#[test]
fn r616_1_static_ids_distinguish_sources_and_two_abilities_on_one_source() {
    // Rules: 113.2c, 616.1, 615.7
    let mut g = game();
    let one = g.put(1, 0, Zone::Battlefield);
    let two = g.put(1, 1, Zone::Battlefield);
    cast(&mut g, 2, Target::Player(1));
    cast(&mut g, 3, Target::Player(1));
    let Some(Decision::Choose { player: 1, choice: Choice::DamageOrder { among, .. } }) = g.decision.clone() else { panic!("{:?}", g.decision) };
    let ids: Vec<_> = among.iter().filter_map(|o| if let DamageOption::Prevention { effect, .. } = o { Some(effect.clone()) } else { None }).collect();
    cite!(["113.2c", "616.1"]; assert_eq!(ids.len(), 3));
    cite!(["113.2c", "616.1"]; assert_eq!(ids.iter().filter(|e| e.source == one).count(), 2));
    cite!(["113.2c", "616.1"]; assert_eq!(ids.iter().filter(|e| e.source == two).count(), 1));
    cite!(["113.2c", "616.1"]; assert!(ids.iter().enumerate().all(|(i, id)| !ids[..i].contains(id))));
    let chosen = among.iter().find(|o| matches!(o, DamageOption::Prevention { effect, .. } if effect.source == one && effect.ability().unwrap().index == 1)).unwrap().clone();
    cite!(["616.1", "616.1e"]; ok: g.act(1, Move::Choose(Answer::DamageOrder(chosen))));
    cite!(["615.6"]; assert_eq!(g.players[1].life, 20));
    g.move_object(one, Zone::Graveyard);
    g.move_object(two, Zone::Graveyard);
    // Choosing a static prevent-all did not spend the three-point shield.
    cast(&mut g, 3, Target::Player(1));
    cite!(["616.1f", "615.7"]; assert_eq!(g.players[1].life, 20));
    cast(&mut g, 3, Target::Player(1));
    cite!(["615.7"]; assert_eq!(g.players[1].life, 19));
}

#[test]
fn r615_1_two_granted_instances_have_distinct_ids_on_the_same_holder() {
    // Rules: 113.2c, 113.7, 615.6, 616.1
    let mut g = game();
    let bear = g.put(1, 7, Zone::Battlefield);
    g.put(1, 10, Zone::Battlefield);
    g.put(1, 10, Zone::Battlefield);
    cast(&mut g, 11, Target::Object(bear));
    cast(&mut g, 8, Target::Object(bear));
    let Some(Decision::Choose { player: 1, choice: Choice::DamageOrder { among, .. } }) = g.decision.clone() else { panic!("{:?}", g.decision) };
    let ids: Vec<_> = among.iter().filter_map(|o| if let DamageOption::Prevention { effect, .. } = o { Some(effect.clone()) } else { None }).collect();
    cite!(["113.2c", "616.1"]; assert_eq!(ids.len(), 2));
    cite!(["113.7"]; assert!(ids.iter().all(|id| id.source == bear)));
    cite!(["113.2c", "616.1"]; assert_ne!(ids[0], ids[1]));
    // Internal provenance representation, beyond the independently selectable effects above.
    assert_ne!(ids[0].ability().unwrap().origin, ids[1].ability().unwrap().origin);
    let chosen = among.iter().find(|o| matches!(o, DamageOption::Prevention { .. })).unwrap().clone();
    cite!(["616.1", "616.1e"]; ok: g.act(1, Move::Choose(Answer::DamageOrder(chosen))));
    cite!(["615.6"]; assert_eq!(g.obj(bear).damage, 0));
}

#[test]
fn r608_2h_a_sacrificed_creatures_damage_is_described_by_its_last_known_information() {
    // Rules: 608.2h, 615.1
    let mut g = game();
    g.put(1, 4, Zone::Battlefield);
    let mogg = g.put(0, 5, Zone::Battlefield);
    g.act(0, Move::Activate { source: mogg, ability: 0, targets: vec![vec![Target::Player(1)]], paying: vec![] }).unwrap();
    resolve(&mut g);
    // Setup sanity: the damage source has left before checking its last known type.
    assert!(!g.battlefield.contains(&mogg));
    cite!(["608.2h", "113.7a", "615.1"]; assert_eq!(g.players[1].life, 20, "its source, gone, was a creature"));
    // A spell is not a creature source.
    cast(&mut g, 3, Target::Player(1));
    cite!(["615.1"]; assert_eq!(g.players[1].life, 18));
}

#[test]
fn r615_1_caduceus_granted_prevention_tracks_the_condition_and_ability_loss() {
    // Rules: 615.1, 613.1f, 611.3a
    let mut g = game();
    let bear = g.put(1, 7, Zone::Battlefield);
    let staff = g.put(1, 6, Zone::Battlefield);
    // Fixture attachment, not an equip-action expectation.
    assert!(g.attach(staff, Target::Object(bear)));
    g.players[1].life = 30;
    cast(&mut g, 8, Target::Object(bear));
    cite!(["611.3a", "615.6"]; assert_eq!(g.obj(bear).damage, 0));
    g.players[1].life = 29;
    // Losing one life switches off both prevention and the +5/+5 grant.
    cast(&mut g, 8, Target::Object(bear));
    cite!(["611.3a", "704.5g"]; assert!(!g.battlefield.contains(&bear)));
    let another = g.put(1, 7, Zone::Battlefield);
    // Fixture attachment, not an equip-action expectation.
    assert!(g.attach(staff, Target::Object(another)));
    g.players[1].life = 30;
    cast(&mut g, 9, Target::Object(another));
    cast(&mut g, 8, Target::Object(another));
    cite!(["613.1f", "615.1"]; assert_eq!(g.obj(another).damage, 2, "ability loss removed granted prevention"));
}
