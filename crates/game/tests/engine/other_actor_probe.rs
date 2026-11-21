//! "Each other creature" after an explicit actor is other than that actor, not the effect's
//! source; an implicit or self-naming actor still excludes the source. Pinned full cards.
//! Run with `engine::other_actor_probe::`.
use crate::support::cite;
use game::state::Answer;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};

fn pinned(name: &str) -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let d = CardDef::from_faces(CardDef::catalog(&faces).get(name).unwrap()).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
    d
}
fn creature(name: &str, p: i32, t: i32, text: &str) -> CardDef {
    let d = CardDef::new(name, "{0}", "Creature — Beast", text, Some((p, t))).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
    d
}
fn pass(g: &mut Game) { for _ in 0..3 { if g.decision.is_some() || g.stack.is_empty() { return; } g.act(g.priority, Move::Pass).unwrap(); } }
fn cast(g: &mut Game, card: usize, targets: Vec<Target>, spending: &str) -> ObjId {
    g.players[0].pool.colors = [10; 5];
    let c = g.put(0, card, Zone::Hand);
    g.act(0, crate::support::paid(spending, Move::Cast { card: c, targets: vec![targets] })).unwrap();
    pass(g);
    c
}
fn activate(g: &mut Game, source: ObjId, ability: usize, targets: Vec<Vec<Target>>, spending: &str) {
    g.players[0].pool.colors = [10; 5];
    g.act(0, crate::support::paid(spending, Move::Activate { source, ability, targets, paying: vec![] })).unwrap();
    pass(g);
}
fn damage(g: &Game) -> Vec<(ObjId, Target, u32)> {
    g.log.iter().filter_map(|e| if let Event::DamageDealt { source, to, amount, .. } = e { Some((*source, *to, *amount)) } else { None }).collect()
}
fn to(g: &Game, o: ObjId) -> u32 { damage(g).iter().filter(|(_, t, _)| *t == Target::Object(o)).map(|(_, _, n)| n).sum() }

/// Too Greedily, Too Deep: the returned creature (a new object, 400.7) is the actor; it is not
/// "each other creature". Plain return, so the identity check is not only a Clone artefact.
#[test]
fn too_greedily_returned_plain_creature_is_not_its_own_recipient() {
    let mut g = Game::new(vec![pinned("Too Greedily, Too Deep"), creature("Brute", 3, 5, ""), creature("Bear", 2, 2, "")], 2, 20);
    let mine = g.put(0, 2, Zone::Battlefield);
    let theirs = g.put(1, 2, Zone::Battlefield);
    let dead = g.put(1, 1, Zone::Graveyard);
    let spell = cast(&mut g, 0, vec![Target::Object(dead)], "WWWWWBR");
    let back = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(1)).expect("returned Brute");
    println!("spell={spell:?} graveyard={dead:?} returned={back:?} damage={:?}", damage(&g));
    cite!(["608.2h", "400.7", "120.1"]; assert!(damage(&g).iter().all(|(s, _, _)| *s == back), "the returned creature deals all of it"));
    // "damage equal to its power": the Brute's current 3
    cite!(["608.2h"]; assert_eq!((to(&g, mine), to(&g, theirs)), (3, 3)));
    // Uncited (card text): "each other creature" is other than "That creature", the one dealing it
    assert_eq!(to(&g, back), 0, "returned actor excluded");
    assert_eq!(g.obj(back).zone, Zone::Battlefield);
}

/// Chandra's Ignition: target creature you control is the actor; the spell is the effect
/// source. Without any return, the actor must still be excluded (no self-damage).
#[test]
fn chandras_ignition_actor_is_excluded_and_opponent_is_hit() {
    let mut g = Game::new(vec![pinned("Chandra's Ignition"), creature("Brute", 3, 5, ""), creature("Bear", 2, 2, "")], 2, 20);
    let actor = g.put(0, 1, Zone::Battlefield);
    let mine = g.put(0, 2, Zone::Battlefield);
    let theirs = g.put(1, 2, Zone::Battlefield);
    cast(&mut g, 0, vec![Target::Object(actor)], "WWWRR");
    println!("actor={actor:?} damage={:?}", damage(&g));
    // the amount is its current power (608.2h); "each other creature" excludes that creature (card text)
    cite!(["608.2h"]; assert_eq!((to(&g, mine), to(&g, theirs), to(&g, actor)), (3, 3, 0)));
    cite!(["120.3a"]; assert_eq!((g.players[0].life, g.players[1].life), (20, 17)));
}

/// Actor versus effect source on one permanent: a creature's ability makes another creature
/// the actor. The ability's source is then one of the "other creatures" and is dealt damage;
/// only the actor is excluded. Distinguishes rebinding from merely adding an exclusion.
#[test]
fn creature_source_is_hit_when_another_creature_acts() {
    let text = "{T}: Target creature you control deals damage equal to its power to each other creature.";
    let mut g = Game::new(vec![creature("Instigator", 1, 4, text), creature("Brute", 3, 5, ""), creature("Bear", 2, 2, "")], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let actor = g.put(0, 1, Zone::Battlefield);
    let theirs = g.put(1, 2, Zone::Battlefield);
    activate(&mut g, source, 0, vec![vec![Target::Object(actor)]], "");
    println!("source={source:?} actor={actor:?} damage={:?}", damage(&g));
    // the amount is its current power (608.2h); "each other creature" is other than that creature, so the ability's source is dealt damage too (card text)
    cite!(["608.2h"]; assert_eq!((to(&g, source), to(&g, theirs), to(&g, actor)), (3, 3, 0)));
}

/// Same permanent, self-naming actor ("this creature"): the source is excluded as before.
#[test]
fn creature_source_is_excluded_when_it_acts_itself() {
    let text = "{T}: This creature deals 1 damage to each other creature.";
    let mut g = Game::new(vec![creature("Sparker", 1, 4, text), creature("Bear", 2, 2, "")], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let theirs = g.put(1, 1, Zone::Battlefield);
    activate(&mut g, source, 0, vec![], "");
    // Uncited (card text): "This creature deals 1 damage to each other creature"
    assert_eq!((to(&g, source), to(&g, theirs)), (0, 1));
}

/// Ordinary pinned source exclusions: Fire Ants (no flying) and Scourge of Kher Ridges, a
/// flier whose "each other creature with flying" must not include itself.
#[test]
fn ordinary_source_relative_other_is_unchanged() {
    let mut g = Game::new(vec![pinned("Fire Ants"), creature("Bear", 2, 2, "")], 2, 20);
    let ants = g.put(0, 0, Zone::Battlefield);
    let bear = g.put(1, 1, Zone::Battlefield);
    activate(&mut g, ants, 0, vec![], "");
    // Uncited (card text): Fire Ants' "This creature deals 1 damage to each other creature without flying"
    assert_eq!((to(&g, ants), to(&g, bear)), (0, 1));

    let flier = CardDef::new("Bird", "{0}", "Creature — Bird", "Flying", Some((1, 7))).unwrap();
    let mut g = Game::new(vec![pinned("Scourge of Kher Ridges"), flier], 2, 20);
    let scourge = g.put(0, 0, Zone::Battlefield);
    let bird = g.put(1, 1, Zone::Battlefield);
    activate(&mut g, scourge, 2, vec![], "WWWWWR");
    println!("scourge damage={:?}", damage(&g));
    // Uncited (card text): Scourge of Kher Ridges' "This creature deals 6 damage to each other creature with flying"
    assert_eq!((to(&g, scourge), to(&g, bird)), (0, 6));
}

/// A returned Clone copying a 6/6 is the actor (ObjId 5,
/// spell ObjId 3); recipients {Big, Wall}; Wall keeps the full-card six-damage contrast.
#[test]
fn too_greedily_returned_clone_is_not_its_own_recipient() {
    let wall = CardDef::new("Wall", "{0}", "Creature — Wall", "", Some((0, 8))).unwrap();
    let big = CardDef::new("Big", "{5}{G}", "Creature — Beast", "", Some((6, 6))).unwrap();
    let mut g = Game::new(vec![pinned("Too Greedily, Too Deep"), pinned("Clone"), big, wall], 3, 20);
    let big = g.put(1, 2, Zone::Battlefield);
    let wall = g.put(1, 3, Zone::Battlefield);
    let dead = g.put(0, 1, Zone::Graveyard);
    cast(&mut g, 0, vec![Target::Object(dead)], "WWWWWBR");
    let branch = g.clone();
    for mut g in [g, branch] {
        g.act(0, Move::Choose(Answer::Objects(vec![big]))).unwrap();
        let d = damage(&g);
        println!("clone damage={d:?}");
        let source = d[0].0;
        cite!(["608.2h"]; assert_eq!(g.obj(wall).damage, 6));
        assert_eq!(g.obj(big).zone, Zone::Gone);
        // Uncited (card text): "That creature deals damage ... to each other creature": other than the returned Clone
        assert_eq!(to(&g, source), 0, "returned Clone excluded");
        assert_eq!(g.obj(source).zone, Zone::Battlefield);
    }
}

/// Rebinding recipients is local to the explicit actor's action. A later self-naming act
/// keeps its ordinary source-relative exclusion, even within the same ability.
#[test]
fn r608_2h_actor_exclusion_does_not_leak_into_a_subsequent_source_action() {
    let text = "{T}: Target creature you control deals 3 damage to each other creature. This creature deals 1 damage to each other creature.";
    let mut g = Game::new(vec![creature("Instigator", 1, 10, text), creature("Actor", 3, 10, ""), creature("Other", 2, 10, "")], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let actor = g.put(0, 1, Zone::Battlefield);
    let other = g.put(1, 2, Zone::Battlefield);
    activate(&mut g, source, 0, vec![vec![Target::Object(actor)]], "");
    let actual = damage(&g);
    // each creature that deals damage is its source (120.1, 608.2h); the recipients are the card text's "each other creature", other than the creature dealing it
    cite!(["120.1", "608.2h"]; assert!(actual.contains(&(actor, Target::Object(source), 3))));
    cite!(["120.1", "608.2h"]; assert!(actual.contains(&(actor, Target::Object(other), 3))));
    cite!(["120.1", "608.2h"]; assert!(actual.contains(&(source, Target::Object(actor), 1))));
    cite!(["120.1", "608.2h"]; assert!(actual.contains(&(source, Target::Object(other), 1))));
    assert_eq!(actual.len(), 4);
}

/// “Opponent” remains relative to the ability controller (109.5), even when an opponent's
/// creature is the object performing the damage (608.2h).
#[test]
fn r109_5_actor_exclusion_does_not_rebind_the_effect_controller() {
    let text = "{T}: Target creature deals 1 damage to each other creature and each opponent.";
    let mut g = Game::new(vec![creature("Instigator", 1, 10, text), creature("Actor", 3, 10, "")], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let actor = g.put(1, 1, Zone::Battlefield);
    activate(&mut g, source, 0, vec![vec![Target::Object(actor)]], "");
    // Uncited (card text): "Target creature deals 1 damage to each other creature": other than that creature
    assert_eq!((to(&g, source), to(&g, actor)), (1, 0));
    cite!(["109.5"]; assert_eq!((g.players[0].life, g.players[1].life), (20, 19)));
    cite!(["120.1", "608.2h"]; assert!(damage(&g).iter().all(|(s, _, _)| *s == actor)));
}


// Alpha Brawl in full: the target is not one of "those creatures".
#[test]
fn alpha_brawl_target_is_not_among_those_creatures() {
    let mut g = Game::new(vec![pinned("Alpha Brawl"), creature("Boss", 4, 9, ""), creature("Bear", 2, 2, ""), creature("Wall", 1, 5, "")], 2, 20);
    let boss = g.put(1, 1, Zone::Battlefield);
    let bear = g.put(1, 2, Zone::Battlefield);
    let wall = g.put(1, 3, Zone::Battlefield);
    let mine = g.put(0, 2, Zone::Battlefield);
    cast(&mut g, 0, vec![Target::Object(boss)], "WWWWWWRR");
    println!("boss={boss:?} damage={:?}", damage(&g));
    // Alpha Brawl's "deals damage equal to its power to each other creature that player controls": current power (608.2h); "other" is card text
    cite!(["608.2h"]; assert_eq!((to(&g, bear), to(&g, wall), to(&g, mine)), (4, 4, 0)));
    // "then each of those creatures deals damage equal to its power to that creature": current powers (608.2h); the target is not one of those (card text)
    cite!(["608.2h"]; assert_eq!(to(&g, boss), 3, "bear 2 + wall 1, not its own 4"));
}
