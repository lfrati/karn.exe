//! An attachment relation can refer to the object named by its trigger (608.2k).
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, Target, Zone};
use crate::support::cite;
fn defs(ty: &str, text: &str) -> Vec<CardDef> {
    vec![
        CardDef::new("Harness", "{0}", ty, text, None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
    ]
}
fn resolve(g: &mut Game) {
    for _ in 0..3 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
#[test]
fn r608_2k_attaching_this_aura_does_not_replace_the_creature_named_by_the_trigger() {
    for accept in [false, true] {
        let definitions = defs(
            "Enchantment — Aura",
            "Enchant creature\nEnchanted creature has indestructible.\nWhenever a creature enters, you may attach this Aura to that creature.",
        );
        for d in &definitions {
            game::play::check(d).unwrap();
        }
        let mut g = Game::new(definitions, 3, 20);
        let aura = g.put(0, 0, Zone::Battlefield);
        let old = g.put(0, 1, Zone::Battlefield);
        assert!(g.attach(aura, Target::Object(old)));
        let card = g.put(0, 1, Zone::Hand);
        g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
        resolve(&mut g);
        let new = *g.battlefield.last().unwrap();
        assert_ne!(old, new);
        resolve(&mut g);
        assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::May, .. })));
        g.act(0, Move::Choose(if accept { Answer::Yes } else { Answer::No })).unwrap();
        cite!(["608.2k", "701.3a"]; case: format!("{accept}"); assert_eq!(g.attached_to(aura), Some(Target::Object(if accept { new } else { old }))));
        assert_eq!(g.has(new, "Indestructible"), accept);
        assert_eq!(g.has(old, "Indestructible"), !accept);
    }
}
#[test]
fn r608_2k_attaching_this_equipment_preserves_the_trigger_subject_for_it() {
    let definitions = defs("Artifact — Equipment", "Whenever a creature enters, attach this Equipment to it.");
    for d in &definitions {
        game::play::check(d).unwrap();
    }
    let mut g = Game::new(definitions, 3, 20);
    let equipment = g.put(0, 0, Zone::Battlefield);
    let old = g.put(0, 1, Zone::Battlefield);
    assert!(g.attach(equipment, Target::Object(old)));
    let card = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    resolve(&mut g);
    let new = *g.battlefield.last().unwrap();
    resolve(&mut g);
    cite!(["608.2k", "701.3a"]; assert_eq!(g.attached_to(equipment), Some(Target::Object(new))));
}
#[test]
fn r608_2k_an_attachment_destination_without_an_antecedent_is_refused() {
    let d = CardDef::new("Harness", "{0}", "Artifact — Equipment", "{0}: Attach this Equipment to it.", None).unwrap();
    crate::support::refused(game::play::check(&d).map_err(|e|e.0),"input reference: nothing before it supplies the required participant/cardinality");
}

#[test]
fn r115_10_triggered_attachment_can_attach_to_an_opponents_shrouded_creature() {
    let mut definitions = defs(
        "Enchantment — Aura",
        "Enchant creature\nWhenever a creature enters, you may attach this Aura to that creature.",
    );
    definitions.push(CardDef::new("Hidden", "{0}", "Creature", "Shroud", Some((2, 2))).unwrap());
    for d in &definitions {
        game::play::check(d).unwrap();
    }
    let mut g = Game::new(definitions, 3, 20);
    let aura = g.put(0, 0, Zone::Battlefield);
    let old = g.put(0, 1, Zone::Battlefield);
    g.attach(aura, Target::Object(old));
    g.active = 1;
    g.priority = 1;
    let card = g.put(1, 2, Zone::Hand);
    g.act(1, Move::Cast { card, targets: vec![] }).unwrap();
    resolve(&mut g);
    let new = *g.battlefield.last().unwrap();
    resolve(&mut g);
    assert!(matches!(g.decision, Some(Decision::Choose { player: 0, .. })));
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    cite!(["115.10"]; assert_eq!(g.attached_to(aura), Some(Target::Object(new))));
    cite!(["303.4e"]; assert_eq!(g.obj(aura).controller, 0));
    cite!(["303.4e"]; assert_eq!(g.obj(new).controller, 1));
}
#[test]
fn r400_7_a_triggered_attachment_does_not_follow_a_departed_destination() {
    let definitions = defs("Artifact — Equipment", "Whenever a creature enters, attach this Equipment to it.");
    let mut g = Game::new(definitions, 3, 20);
    let equipment = g.put(0, 0, Zone::Battlefield);
    let old = g.put(0, 1, Zone::Battlefield);
    g.attach(equipment, Target::Object(old));
    let card = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    resolve(&mut g);
    let new = *g.battlefield.last().unwrap();
    g.destroy(new);
    resolve(&mut g);
    cite!(["400.7"]; assert_eq!(g.attached_to(equipment), Some(Target::Object(old))));
    assert_eq!(g.obj(new).zone, Zone::Gone);
}
#[test]
fn r400_7_a_triggered_attachment_does_not_follow_a_departed_source() {
    let definitions = defs("Artifact — Equipment", "Whenever a creature enters, attach this Equipment to it.");
    let mut g = Game::new(definitions, 3, 20);
    let equipment = g.put(0, 0, Zone::Battlefield);
    let card = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    resolve(&mut g);
    g.exile(equipment);
    let exiled = *g.exile.last().unwrap();
    resolve(&mut g);
    cite!(["400.7"]; assert_eq!(g.attached_to(exiled), None));
    assert_eq!(g.obj(exiled).zone, Zone::Exile);
    assert!(g.stack.is_empty());
}
