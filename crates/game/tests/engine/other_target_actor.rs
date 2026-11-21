//! "Another target creature" after a targeted actor (115.3): the second target word may not
//! choose the creature (or creatures) the first chose; any other creature, including another
//! of the caster's own, may be chosen. These target
//! exclusions do not depend on how the "other" recipients of the damage are bound.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};

fn pinned(name: &str) -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let d = CardDef::from_faces(CardDef::catalog(&faces).get(name).unwrap()).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
    d
}
fn creature(name: &str, p: i32, t: i32) -> CardDef { CardDef::new(name, "{0}", "Creature — Beast", "", Some((p, t))).unwrap() }
fn damage(g: &Game) -> Vec<(ObjId, Target, u32)> {
    g.log.iter().filter_map(|e| if let Event::DamageDealt { source, to, amount, .. } = e { Some((*source, *to, *amount)) } else { None }).collect()
}
fn obj(o: ObjId) -> Target { Target::Object(o) }
fn cast(g: &mut Game, card: usize, targets: Vec<Vec<Target>>, spending: &str) -> Result<(), String> {
    g.players[0].pool.colors = [10; 5];
    let c = g.put(0, card, Zone::Hand);
    g.act(0, crate::support::paid(spending, Move::Cast { card: c, targets }))?;
    for _ in 0..2 { if !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); } }
    Ok(())
}

#[test]
fn fall_of_the_hammer_second_target_is_another_creature() {
    let defs = || vec![pinned("Fall of the Hammer"), creature("Brute", 3, 5), creature("Bear", 2, 2)];
    let mut g = Game::new(defs(), 2, 20);
    let (actor, mine, theirs) = (g.put(0, 1, Zone::Battlefield), g.put(0, 2, Zone::Battlefield), g.put(1, 2, Zone::Battlefield));
    cite!(["115.1"]; refused("another target must differ from the preceding targets (115.1, 601.2c)"): cast(&mut g.clone(), 0, vec![vec![obj(actor)], vec![obj(actor)]], "WR"));
    cast(&mut g.clone(), 0, vec![vec![obj(actor)], vec![obj(mine)]], "WR").expect("another creature of mine is legal");
    cast(&mut g, 0, vec![vec![obj(actor)], vec![obj(theirs)]], "WR").unwrap();
    // the creature that deals it is the damage's source (120.1); its power is read as the effect applies (608.2h)
    cite!(["120.1", "608.2h"]; assert_eq!(damage(&g), vec![(actor, obj(theirs), 3)]));
}

#[test]
fn band_together_another_excludes_both_actors() {
    let mut g = Game::new(vec![pinned("Band Together"), creature("Brute", 3, 5), creature("Bear", 2, 2)], 2, 20);
    let (a, b, theirs) = (g.put(0, 1, Zone::Battlefield), g.put(0, 2, Zone::Battlefield), g.put(1, 2, Zone::Battlefield));
    let mine = g.put(0, 2, Zone::Battlefield);
    for actor in [a, b] { cite!(["115.1"]; refused("another target must differ from the preceding targets (115.1, 601.2c)"): cast(&mut g.clone(), 0, vec![vec![obj(a), obj(b)], vec![obj(actor)]], "WWG")); }
    cast(&mut g.clone(), 0, vec![vec![obj(a), obj(b)], vec![obj(mine)]], "WWG").expect("a third creature of mine is legal");
    cast(&mut g, 0, vec![vec![obj(a), obj(b)], vec![obj(theirs)]], "WWG").unwrap();
    cite!(["120.1", "608.2h"]; assert!(damage(&g).iter().all(|(s, t, _)| [a, b].contains(s) && *t == obj(theirs))));
}

#[test]
fn betrayal_at_the_vault_two_other_targets() {
    let mut g = Game::new(vec![pinned("Betrayal at the Vault"), creature("Brute", 3, 5), creature("Bear", 2, 2)], 2, 20);
    let (actor, x, y) = (g.put(0, 1, Zone::Battlefield), g.put(1, 2, Zone::Battlefield), g.put(1, 2, Zone::Battlefield));
    cite!(["115.1"]; refused("another target must differ from the preceding targets (115.1, 601.2c)"): cast(&mut g.clone(), 0, vec![vec![obj(actor)], vec![obj(actor), obj(x)]], "WWWWGG"));
    cast(&mut g, 0, vec![vec![obj(actor)], vec![obj(x), obj(y)]], "WWWWGG").unwrap();
    let mut d = damage(&g); d.sort_by_key(|x| format!("{x:?}"));
    cite!(["120.1", "608.2h"]; assert_eq!(d, vec![(actor, obj(x), 3), (actor, obj(y), 3)]));
}


// Band Together in full: each of the two creatures deals its own power.
#[test]
fn band_together_each_deals_its_own_power() {
    let mut g = Game::new(vec![pinned("Band Together"), creature("Brute", 3, 5), creature("Bear", 2, 2)], 2, 20);
    let (a, b, theirs) = (g.put(0, 1, Zone::Battlefield), g.put(0, 2, Zone::Battlefield), g.put(1, 1, Zone::Battlefield));
    cast(&mut g, 0, vec![vec![obj(a), obj(b)], vec![obj(theirs)]], "WWG").unwrap();
    let mut d = damage(&g); d.sort_by_key(|x| format!("{x:?}"));
    cite!(["120.1", "608.2h"]; assert_eq!(d, vec![(a, obj(theirs), 3), (b, obj(theirs), 2)]));
}
