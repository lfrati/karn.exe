//! Last-counter triggers retain the removal's result, including later counter changes.
use crate::support::{cite, CardDef, Game, Move, Zone};

fn game(trigger: &str) -> Game {
    let text = format!("{{0}}: Remove a time counter from this artifact.\n\
        {{0}}: Remove two time counters from this artifact.\n\
        {{0}}: Remove a charge counter from this artifact.\n\
        {{0}}: Remove two time counters from this artifact. Put a time counter on this artifact.\n{trigger}");
    let def = CardDef::new("Counter watcher", "{0}", "Artifact", &text, None).unwrap();
    // Uncited (admission): the full generic counter-removal family compiles.
    game::play::check(&def).unwrap();
    Game::new(vec![def], 2, 20)
}
fn resolve(g: &mut Game) {
    while !g.stack.is_empty() || g.decision.is_some() {
        if let Some(game::state::Decision::Order { player, abilities }) = &g.decision {
            assert_eq!(abilities.len(), 2);
            g.act(*player, Move::Order(vec![0, 1])).unwrap();
        } else { g.act(g.priority, Move::Pass).unwrap(); }
    }
}
fn activate(g: &mut Game, source: game::ObjId, ability: usize) {
    g.act(0, Move::Activate { source, ability, targets: vec![], paying: vec![] }).unwrap();
    resolve(g);
}

#[test]
fn r603_2_last_counter_means_last_of_the_named_kind_and_one_occurrence_per_batch() {
    let mut g = game("When the last time counter is removed from this artifact, you gain 1 life.");
    let source = g.put(0, 0, Zone::Battlefield);
    g.put_counters(source, "time", 3);
    g.put_counters(source, "charge", 1);
    activate(&mut g, source, 2);
    cite!(["603.2"]; assert_eq!(g.players[0].life, 20));
    activate(&mut g, source, 0);
    cite!(["603.2"]; assert_eq!(g.players[0].life, 20));
    activate(&mut g, source, 1);
    cite!(["603.2", "603.2c"]; assert_eq!(g.players[0].life, 21));
    activate(&mut g, source, 0);
    cite!(["122.1", "603.2"]; assert_eq!(g.players[0].life, 21));
}

#[test]
fn r603_2_last_counter_uses_the_removal_result_before_later_placement() {
    let mut g = game("When the last time counter is removed from this artifact, you gain 1 life.");
    let source = g.put(0, 0, Zone::Battlefield);
    g.put_counters(source, "time", 2);
    activate(&mut g, source, 3);
    cite!(["603.2", "603.2c"]; assert_eq!(g.players[0].life, 21));
    cite!(["122.1"]; assert_eq!(g.counters(source).get("time"), Some(&1)));
}

#[test]
fn r603_2c_removal_of_a_counter_and_one_or_more_counters_have_distinct_multiplicity() {
    for (trigger, life) in [
        ("Whenever a time counter is removed from this artifact, you gain 1 life.", 22),
        ("Whenever one or more time counters are removed from this artifact, you gain 1 life.", 21),
    ] {
        let mut g = game(trigger);
        let source = g.put(0, 0, Zone::Battlefield);
        g.put_counters(source, "time", 2);
        activate(&mut g, source, 1);
        cite!(["603.2c"]; assert_eq!(g.players[0].life, life));
    }
}

#[test]
fn r603_2_orcish_mine_uses_its_full_oracle_last_ore_counter_trigger() {
    let mine = crate::support::oracle_read("Orcish Mine");
    let land = crate::support::oracle_read("Mountain");
    let remover = CardDef::new("Counter remover", "{0}", "Artifact",
        "{0}: Remove two ore counters from target permanent.\n{0}: Remove a charge counter from target permanent.", None).unwrap();
    // Uncited (admission): the complete pinned card, including its other abilities.
    for def in [&mine, &land, &remover] { game::play::check(def).unwrap(); }
    let mut g = Game::new(vec![mine, land, remover], 2, 20);
    let land = g.put(1, 1, Zone::Battlefield);
    let source = g.put(0, 2, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool = crate::support::mana_pool("RRR");
    cite!(["601.2c", "601.2h"]; ok: g.act(0, crate::support::paid("RRR", Move::Cast {
        card, targets: vec![vec![land.into()]],
    })));
    resolve(&mut g);
    let aura = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(0)).unwrap();
    cite!(["122.6"]; assert_eq!(g.counters(aura).get("ore"), Some(&3)));
    g.put_counters(aura, "charge", 1);
    for (ability, ore) in [(1, 3), (0, 1)] {
        g.act(0, Move::Activate { source, ability, targets: vec![vec![aura.into()]], paying: vec![] }).unwrap();
        resolve(&mut g);
        cite!(["603.2"]; assert!(g.battlefield.contains(&land)));
        cite!(["122.1"]; assert_eq!(g.counters(aura).get("ore"), Some(&ore)));
        // Uncited (card text): neither removing charge nor leaving ore triggers its damage.
        assert_eq!(g.players[1].life, 20);
    }
    g.act(0, Move::Activate { source, ability: 0, targets: vec![vec![aura.into()]], paying: vec![] }).unwrap();
    resolve(&mut g);
    cite!(["603.2", "701.7a"]; assert_eq!(g.obj(land).zone, Zone::Gone));
    cite!(["704.5m"]; assert_eq!(g.obj(aura).zone, Zone::Gone));
    // Uncited (card text): the last ore counter destroys the enchanted land and deals 2 to its controller.
    assert_eq!(g.players[1].life, 18);
    cite!(["701.7a", "704.5m"]; assert_eq!((g.players[0].graveyard.len(), g.players[1].graveyard.len()), (1, 1)));
}
