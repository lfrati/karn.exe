//! Effects on permanent spells continue onto the resulting permanent (112.4, 400.7a).
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Target, Zone};
use ir::Color;

fn game() -> Game {
    let defs = vec![
        CardDef::new("Bear", "{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Tint", "{0}", "Instant", "Target spell becomes red.", None).unwrap(),
        CardDef::new("Brief Tint", "{0}", "Instant", "Target spell becomes blue until end of turn.", None).unwrap(),
        CardDef::new("Watcher", "{0}", "Enchantment", "Whenever a red creature enters, you gain 2 life.", None)
            .unwrap(),
        CardDef::new("Delay", "{0}", "Enchantment", "Red creatures enter tapped.", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    Game::new(defs, 3, 20)
}
fn pass(g: &mut Game) {
    for _ in 0..3 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn tint(g: &mut Game, spell: game::ObjId, def: usize, mana: &str) {
    let card = g.put(0, def, Zone::Hand);
    g.act(0, paid(mana, Move::Cast { card, targets: vec![vec![Target::Object(spell)]] })).unwrap();
    pass(g);
}
#[test]
fn r112_4_a_color_change_on_a_permanent_spell_continues_after_resolution() {
    let mut g = game();
    let spell = g.put(0, 0, Zone::Stack);
    tint(&mut g, spell, 1, "");
    cite!(["105.3"]; assert_eq!(g.seen(spell, None).colors, vec![Color::Red]));
    pass(&mut g);
    let creature = g.battlefield[0];
    assert_ne!(spell, creature);
    cite!(["112.4"]; assert_eq!(g.seen(creature, None).colors, vec![Color::Red]));
    assert_eq!(g.seen(spell, None).colors, vec![Color::Red], "last-known information remains intact");
    let other = g.put(0, 0, Zone::Battlefield);
    cite!(["112.4"]; assert_eq!(
        g.seen(other, None).colors,
        vec![Color::Green],
        "the effect names an object, not every copy of its card"
    ));
}
#[test]
fn r400_7a_carried_spell_effects_apply_to_entry_replacements_and_triggers() {
    let mut g = game();
    g.put(0, 3, Zone::Battlefield);
    g.put(0, 4, Zone::Battlefield);
    let spell = g.put(0, 0, Zone::Stack);
    tint(&mut g, spell, 1, "");
    pass(&mut g);
    let creature = *g.battlefield.last().unwrap();
    cite!(["400.7a", "614.12"]; assert!(g.obj(creature).tapped, "the changed color applies as the creature enters"));
    cite!(["400.7a", "603.6a"]; assert_eq!(g.stack.len(), 1, "the red-creature entry trigger fired"));
    pass(&mut g);
    assert_eq!(g.players[0].life, 22);
}
#[test]
fn r400_7a_a_carried_spell_effect_does_not_follow_later_zone_changes() {
    let mut g = game();
    let spell = g.put(0, 0, Zone::Stack);
    tint(&mut g, spell, 1, "");
    pass(&mut g);
    let creature = g.battlefield[0];
    assert_eq!(g.seen(creature, None).colors, vec![Color::Red]);
    g.move_object(creature, Zone::Hand);
    let card = *g.players[0].hand.last().unwrap();
    cite!(["400.7"]; assert_eq!(g.seen(card, None).colors, vec![Color::Green]));
    assert_eq!(g.seen(creature, None).colors, vec![Color::Red]);
    g.move_object(card, Zone::Battlefield);
    cite!(["400.7"]; assert_eq!(g.seen(*g.battlefield.last().unwrap(), None).colors, vec![Color::Green]));
}
#[test]
fn r112_4_carried_effects_keep_their_order_and_expire_at_the_original_duration() {
    let mut g = game();
    for player in 0..3 {
        g.put(player, 0, Zone::Library);
    }
    let spell = g.put(0, 0, Zone::Stack);
    tint(&mut g, spell, 1, "");
    tint(&mut g, spell, 2, "");
    pass(&mut g);
    let creature = g.battlefield[0];
    cite!(["400.7a", "613.7"]; assert_eq!(g.seen(creature, None).colors, vec![Color::Blue]));
    for _ in 0..70 {
        if g.turn > 1 {
            break;
        }
        let mv = if matches!(g.decision, Some(game::state::Decision::Attackers { .. })) {
            Move::Attack(vec![])
        } else {
            Move::Pass
        };
        g.act(g.priority, mv).unwrap();
    }
    assert_eq!(g.turn, 2);
    cite!(["611.2a", "514.2"]; assert_eq!(g.seen(creature, None).colors, vec![Color::Red]));
}
#[test]
fn r400_7_countering_a_changed_permanent_spell_does_not_carry_its_effect_to_the_graveyard() {
    let mut g = game();
    let spell = g.put(0, 0, Zone::Stack);
    tint(&mut g, spell, 1, "");
    assert!(g.counter(spell));
    let card = *g.players[0].graveyard.last().unwrap();
    cite!(["400.7"]; assert_eq!(g.seen(card, None).colors, vec![Color::Green]));
    assert_eq!(g.seen(spell, None).colors, vec![Color::Red]);
}

#[test]
fn r112_4_chaoslace_changes_a_creature_spell_and_the_permanent_it_becomes() {
    let defs = vec![
        CardDef::new("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new(
            "Chaoslace",
            "{R}",
            "Instant",
            "Target spell or permanent becomes red. (Its mana symbols remain unchanged.)",
            None,
        )
        .unwrap(),
        CardDef::new("Mountain", "", "Basic Land — Mountain", "", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap();
    }
    let mut g = Game::new(defs, 3, 20);
    let spell = g.put(0, 0, Zone::Stack);
    let land = g.put(0, 2, Zone::Battlefield);
    g.act(0, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    tint(&mut g, spell, 1, "R");
    assert_eq!(g.seen(spell, None).colors, vec![Color::Red]);
    pass(&mut g);
    let creature = *g.battlefield.last().unwrap();
    cite!(["112.4"]; assert_eq!(g.seen(creature, None).colors, vec![Color::Red]));
    cite!(["105.3"]; assert_eq!(g.def(creature).unwrap().cost(), g.def(spell).unwrap().cost()));
}

#[test]
fn r115_1a_spell_or_permanent_targets_exclude_abilities_players_and_other_zones() {

        let defs = vec![
        CardDef::new("Tint", "{0}", "Instant", "Target spell or permanent becomes red.", None).unwrap(),
        CardDef::new("Source", "{0}", "Enchantment", "{0}: You gain 1 life.", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap();
    }
    let mut g = Game::new(defs, 3, 20);
    let source = g.put(0, 1, Zone::Battlefield);
    g.act(0, Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    let ability = g.stack[0];
    let spell = g.put(1, 0, Zone::Stack);
    let tint = g.put(0, 0, Zone::Hand);
    let graveyard = g.put(1, 0, Zone::Graveyard);
    let exiled = g.put(2, 0, Zone::Exile);
    let departed = g.put(2, 1, Zone::Battlefield);
    g.move_object(departed, Zone::Hand);
    for object in [source, spell] {
        let mut accepted=g.clone();
        cite!(["115.1a"]; case: format!("{object:?}"); ok: accepted.act(0, Move::Cast { card: tint, targets: vec![vec![Target::Object(object)]] }));
        pass(&mut accepted);
        cite!(["608.2c"]; assert_eq!(accepted.seen(object,None).colors,vec![Color::Red]));
    }
    for object in [ability, tint, graveyard, exiled, departed] {
        cite!(["112.1", "110.1"]; case: format!("{object:?}"); refused("an illegal target"): unchanged(&mut g,Move::Cast { card: tint, targets: vec![vec![Target::Object(object)]] }));
    }
    for player in 0..3 {
        cite!(["115.1a"]; case: player; refused("an illegal target"): unchanged(&mut g,Move::Cast { card: tint, targets: vec![vec![Target::Player(player)]] }));
    }
    assert_eq!(CardDef::new("Nonsense", "{0}", "Instant", "Destroy target spell or permanent.", None).expect_err("destroy acts on permanents (701.8a)"),"text the grammar does not read");
}

#[track_caller]
fn unchanged(g:&mut Game,action:Move)->Result<(),String>{let before=format!("{g:?}");let result=g.act(0,action);assert_eq!(format!("{g:?}"),before);result}
