//! 107.14 / 122.1: players and permanents share counter operations, with distinct lifetimes.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, Target, Zone};
fn spell(text: &str) -> CardDef {
    let def = CardDef::new("Counter spell", "{0}", "Sorcery", text, None).unwrap();
    game::play::check(&def).unwrap();
    def
}
fn cast(g: &mut Game, targets: Vec<Vec<Target>>) {
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets }).unwrap();
    for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); }
}
#[test]
fn r107_14_energy_and_named_player_counters_use_the_same_events() {
    for (text, kind, count) in [("You get {E}{E}.", "energy", 2), ("You get an experience counter.", "experience", 1), ("You get two poison counters.", "poison", 2)] {
        let mut g = Game::new(vec![spell(text)], 2, 20);
        cast(&mut g, vec![]);
        cite!(["107.14", "122.1"]; case: kind; assert_eq!(g.counters(Target::Player(0))[kind], count));
        assert!(g.counters(Target::Player(1)).is_empty());
        assert!(g.log.iter().any(|e| matches!(e, Event::CountersPut { on: Target::Player(0), kind: k, count: n, before: 0, .. } if k == kind && *n == count)));
    }
}
#[test]
fn r122_1_targeted_and_plural_players_get_their_own_counters() {
    let mut g = Game::new(vec![spell("Target player gets two poison counters.")], 3, 20);
    cast(&mut g, vec![vec![Target::Player(2)]]);
    cite!(["122.1"]; assert_eq!(g.counters(Target::Player(2))["poison"], 2));
    assert!(g.counters(Target::Player(0)).is_empty());
    let mut g = Game::new(vec![spell("Each opponent gets a poison counter.")], 3, 20);
    cast(&mut g, vec![]);
    for p in [1,2] { cite!(["122.1"]; case: p; assert_eq!(g.counters(Target::Player(p))["poison"],1)); }
    assert!(g.counters(Target::Player(0)).is_empty());
}
#[test]
fn r122_2_player_counters_survive_permanent_zone_changes_and_removal_is_actual() {
    let bear = CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2,2))).unwrap();
    let mut g = Game::new(vec![bear], 2, 20);
    let old = g.put(0,0,Zone::Battlefield);
    g.put_counters(old, "+1/+1", 3);
    g.put_counters(Target::Player(0), "energy", 5);
    g.move_object(old, Zone::Graveyard);
    let card = g.players[0].graveyard[0];
    g.move_object(card, Zone::Battlefield);
    cite!(["122.2"]; assert!(g.counters(g.battlefield[0]).is_empty()));
    assert_eq!(g.counters(Target::Player(0))["energy"],5);
    cite!(["122.2"]; assert_eq!(g.remove_counters(old,"+1/+1",3),0));
    assert_eq!(g.remove_counters(Target::Player(0),"energy",2),2);
    assert_eq!(g.remove_counters(Target::Player(0),"energy",9),3);
    let events = g.log.len();
    assert_eq!(g.remove_counters(Target::Player(0),"energy",1),0);
    assert_eq!(g.log.len(),events);
    assert!(g.counters(Target::Player(0)).is_empty());
    cite!(["122.2"]; assert_eq!(g.log.iter().filter_map(|e| if let Event::CountersRemoved { from: Target::Player(0), count, .. } = e { Some(*count) } else { None }).collect::<Vec<_>>(),vec![2,3]));
}
#[test]
fn r122_1_player_counter_amounts_read_the_named_holder_and_compose() {
    let mut g = Game::new(vec![spell("You gain life equal to the number of experience counters you have.")],2,20);
    g.put_counters(Target::Player(0),"experience",3);
    g.put_counters(Target::Player(1),"experience",8);
    cast(&mut g,vec![]);
    cite!(["109.5"]; assert_eq!(g.players[0].life,23));
    let mut g = Game::new(vec![spell("You get {E} for each experience counter you have.")],2,20);
    g.put_counters(Target::Player(0),"experience",3);
    cast(&mut g,vec![]);
    assert_eq!(g.counters(Target::Player(0))["energy"],3);
}
#[test]
fn r122_1i_rad_rule_consequences_remain_explicitly_refused() {
    let def = CardDef::new("Rad", "{0}", "Sorcery", "You get a rad counter.",None).unwrap();
    crate::support::refused(game::play::check(&def).map_err(|e| e.0), "player counter: rad upkeep trigger (122.1i) is not implemented");
    let def=CardDef::new("Placed rad","{0}","Sorcery","Put a rad counter on target player.",None).unwrap();
    crate::support::refused(game::play::check(&def).map_err(|e| e.0), "player counter: rad upkeep trigger (122.1i) is not implemented");
    let def=CardDef::new("Placed rad","{0}","Sorcery","Target player gains 1 life. Put a rad counter on that player.",None).unwrap();
    crate::support::refused(game::play::check(&def).map_err(|e| e.0), "player counter: rad upkeep trigger (122.1i) is not implemented");
    let def=CardDef::new("Inert object rad","{0}","Sorcery","Put a rad counter on target creature.",None).unwrap();
    game::play::check(&def).unwrap();
}

#[test]
fn r122_1_optional_player_counters_collect_consent_before_placement() {
    let mut g=Game::new(vec![spell("Each opponent may get a poison counter.")],3,20);
    cast(&mut g,vec![]);
    cite!(["101.4"]; assert!(matches!(g.decision,Some(game::state::Decision::Choose{player:1,choice:game::state::Choice::May}))));
    g.act(1,Move::Choose(game::state::Answer::No)).unwrap();
    cite!(["101.4"]; assert!(g.players.iter().all(|p|p.counters.is_empty())));
    cite!(["101.4"]; assert!(matches!(g.decision,Some(game::state::Decision::Choose{player:2,choice:game::state::Choice::May}))));
    g.act(2,Move::Choose(game::state::Answer::Yes)).unwrap();
    assert!(g.counters(Target::Player(1)).is_empty());
    cite!(["608.2d"]; assert_eq!(g.counters(Target::Player(2))["poison"],1));
}

#[test]
fn r122_1_plural_player_counter_quantities_read_each_actors_counters() {
    for (text,kind) in [
        ("Each player gets {E} for each experience counter they have.","energy"),
        ("Each player gets a number of poison counters equal to the number of experience counters they have.","poison"),
    ] {
        let mut g=Game::new(vec![spell(text)],3,20);
        for (p,n) in [(0,1),(1,3),(2,5)] {g.put_counters(Target::Player(p),"experience",n);}
        cast(&mut g,vec![]);
        for (p,n) in [(0,1),(1,3),(2,5)] {assert_eq!(g.counters(Target::Player(p))[kind],n);}
    }
}

#[test]
fn r122_1_getting_player_counters_does_not_turn_an_object_into_a_player() {
    let def=CardDef::new("Object","{0}","Creature — Bear","{0}: This creature gets {E}.",Some((2,2)));
    let def = def.unwrap();
    // Uncited (admission): the player-counter verb cannot be rebound to an object.
    crate::support::refused(game::play::check(&def).map_err(|e|e.0), "player counters: an object where a player gets counters");
}
