//! Prospective battlefield abilities own entry choices (614.12), before any entrant moves.
use crate::support::cite;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use game::binding::{Kind, Value};
use game::state::{Answer, Choice, Decision};
use ir::Subtype;

const CHOOSER: usize = 0;
const HUMILITY: usize = 1;
const RETURN: usize = 2;
const REANIMATE: usize = 3;
const YOUR_CREATURES: usize = 4;

fn game() -> Game {
    let card = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).unwrap();
    let defs = vec![
        card("Type chooser", "Artifact Creature — Golem", "As this creature enters, choose a creature type.\nCreatures of the chosen type get +1/+1.", Some((2, 2))),
        card("Humility", "Enchantment", "All creatures lose all abilities and have base power and toughness 1/1.", None),
        card("Open the Vaults", "Sorcery", "Return all artifact and enchantment cards from all graveyards to the battlefield under their owners' control.", None),
        card("Reanimate", "Sorcery", "Return target creature card from a graveyard to the battlefield under your control.", None),
        card("Your creatures", "Enchantment", "Creatures you control lose all abilities.", None),
    ];
    for def in &defs { game::play::check(def).unwrap_or_else(|e| panic!("{}: {}", def.name(), e.0)); }
    Game::new(defs, 2, 20)
}

fn pass(g: &mut Game) {
    for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); }
}
fn cast(g: &mut Game, card: usize, targets: Vec<Vec<Target>>) -> ObjId {
    let id = g.put(g.active, card, Zone::Hand);
    g.act(g.active, Move::Cast { card: id, targets }).unwrap();
    *g.stack.last().unwrap()
}
fn answer(g: &mut Game, player: usize) {
    assert_eq!(g.decision, Some(Decision::Choose { player, choice: Choice::Value(Kind::CreatureType) }));
    g.act(player, Move::Choose(Answer::Value(Value::CreatureType(Subtype::from_name("Golem").unwrap())))).unwrap();
}
fn entered(g: &Game) -> ObjId {
    *g.battlefield.iter().find(|o| g.obj(**o).card == Some(CHOOSER)).unwrap()
}

#[test]
fn r614_12_an_existing_removal_effect_suppresses_the_as_enters_choice() {
    for removed in [false, true] {
        let mut g = game();
        if removed { g.put(0, HUMILITY, Zone::Battlefield); }
        let spell = cast(&mut g, CHOOSER, vec![]);
        let objects = g.object_count();
        pass(&mut g);
        if !removed {
            cite!(["614.12a"]; assert_eq!(g.obj(spell).zone, Zone::Stack, "the query restores the spell before asking"));
            assert_eq!(g.obj(spell).controller, 0);
            assert!(g.obj(spell).choices.is_empty());
            assert_eq!(g.object_count(), objects, "asking allocates no prospective incarnation");
            answer(&mut g, 0);
        } else {
            cite!(["614.12"]; assert!(g.decision.is_none(), "a removed replacement cannot ask"));
        }
        let on = entered(&g);
        cite!(["614.12"]; case: format!("removed={removed}"); assert_eq!(g.obj(on).choices.len(), usize::from(!removed)));
        assert_eq!(g.power(on), Some(if removed { 1 } else { 3 }));
    }
}

#[test]
fn r614_12_simultaneous_entrants_are_not_existing_removal_sources() {
    for removal_first in [false, true] {
        let mut g = game();
        let order = if removal_first { [HUMILITY, CHOOSER] } else { [CHOOSER, HUMILITY] };
        let original: Vec<_> = order.into_iter().map(|card| g.put(0, card, Zone::Graveyard)).collect();
        cast(&mut g, RETURN, vec![]);
        pass(&mut g);
        cite!(["614.12a"]; case: format!("removal_first={removal_first}"); assert!(g.battlefield.is_empty()));
        assert!(original.iter().all(|o| g.obj(*o).zone == Zone::Graveyard));
        let mut clone = g.clone();
        answer(&mut g, 0);
        answer(&mut clone, 0);
        for game in [&g, &clone] {
            assert!(game.decision.is_none());
            let on = entered(game);
            cite!(["614.12"]; case: format!("removal_first={removal_first}"); assert_eq!(game.obj(on).choices.len(), 1, "the choice happened before either entered"));
            assert!(game.abilities(on).is_empty(), "Humility applies after the simultaneous entry");
            assert_eq!(game.power(on), Some(1));
        }
    }
}

#[test]
fn r614_12_entry_choices_use_the_prospective_controller_and_restore_the_original() {
    let mut g = game();
    g.put(0, YOUR_CREATURES, Zone::Battlefield);
    let original = g.put(0, CHOOSER, Zone::Graveyard);
    g.active = 1; g.priority = 1;
    cast(&mut g, REANIMATE, vec![vec![Target::Object(original)]]);
    pass(&mut g);
    assert_eq!(g.obj(original).controller, 0);
    cite!(["614.12a"]; assert_eq!(g.obj(original).zone, Zone::Graveyard));
    answer(&mut g, 1);
    let on = entered(&g);
    assert_eq!(g.obj(on).controller, 1);
    assert_eq!(g.obj(on).choices.len(), 1);
    cite!(["614.12"]; assert_eq!(g.power(on), Some(3)));
}
