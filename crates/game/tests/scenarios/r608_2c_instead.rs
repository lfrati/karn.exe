//! "[Do this.] If [condition], [do that] instead." (608.2c): the condition is asked once as
//! the instruction is followed, and one of the two is done, never both. The replacement may
//! be its own paragraph under an ability word ("Metalcraft — … deals 4 damage instead if
//! you control three or more artifacts."), and may ask where the spell was cast from.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const BLAST: usize = 0;
const VOLLEY: usize = 1;
const SAVAGERY: usize = 2;
const FEAST: usize = 3;
const MURDER: usize = 4;
const BEAR: usize = 5;
const RELIC: usize = 6;

const SHATTER: usize = 8;

fn game() -> Game {
    let defs = vec![
        CardDef::new("Galvanic Blast", "{R}", "Instant", "Galvanic Blast deals 2 damage to any target.\nMetalcraft — Galvanic Blast deals 4 damage instead if you control three or more artifacts.", None).unwrap(),
        CardDef::new("Brimstone Volley", "{2}{R}", "Instant", "Brimstone Volley deals 3 damage to any target.\nMorbid — Brimstone Volley deals 5 damage instead if a creature died this turn.", None).unwrap(),
        CardDef::new("Increasing Savagery", "{2}{G}{G}", "Sorcery", "Put five +1/+1 counters on target creature. If this spell was cast from a graveyard, put ten +1/+1 counters on that creature instead.\nFlashback {5}{G}{G}", None).unwrap(),
        CardDef::new("Feast", "{G}", "Instant", "You gain 2 life. If you control three or more artifacts, you gain 5 life instead.", None).unwrap(),
        CardDef::new("Murder", "{B}", "Instant", "Destroy target creature.", None).unwrap(),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Relic", "{1}", "Artifact", "", None).unwrap(),
        CardDef::new("Wastes", "", "Land", "", None).unwrap(),
        CardDef::new("Shatter", "{1}{R}", "Instant", "Destroy target artifact.", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    Game::new(defs, 2, 20)
}
fn pass_all(g: &mut Game) {
    for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); }
}
/// The first player casts this card, with mana given for it.
fn cast(g: &mut Game, spell: ObjId, targets: Vec<Vec<Target>>, spending: &str) {
    g.players[0].pool.colors = [9, 9, 9, 9, 9];
    g.act(0, paid(spending, Move::Cast { card: spell, targets })).unwrap();
}
fn resolve(g: &mut Game, card: usize, targets: Vec<Vec<Target>>, spending: &str) {
    let spell = g.put(0, card, Zone::Hand);
    cast(g, spell, targets, spending);
    pass_all(g);
    g.players[0].pool = Default::default();
}
fn counters(g: &Game, on: ObjId) -> u32 {
    g.obj(on).counters.get("+1/+1").copied().unwrap_or(0)
}

#[test]
fn a_paragraph_that_says_instead_replaces_the_one_before_it() {
    let mut g = game();
    let them = vec![vec![Target::Player(1)]];
    // one target is announced for the spell: the replacement names none of its own
    let card = g.put(0, BLAST, Zone::Hand);
    g.players[0].pool.colors[3] = 1;
    let error = g.act(0, Move::Cast { card, targets: vec![vec![Target::Player(1)], vec![Target::Player(0)]] }).unwrap_err();
    assert_eq!(error, "that has 1 target group(s), 2 given");
    resolve(&mut g, BLAST, them.clone(), "R");
    assert_eq!(g.players[1].life, 18, "2, without metalcraft");
    for _ in 0..2 { g.put(0, RELIC, Zone::Battlefield); }
    g.put(1, RELIC, Zone::Battlefield);
    resolve(&mut g, BLAST, them.clone(), "R");
    cite!(["608.2c"]; assert_eq!(g.players[1].life, 16, "two artifacts of one's own are not three"));
    g.put(0, RELIC, Zone::Battlefield);
    resolve(&mut g, BLAST, them, "R");
    cite!(["608.2c"]; assert_eq!(g.players[1].life, 12, "4 instead of 2, not as well as it"));
}

#[test]
fn the_condition_is_asked_as_the_spell_resolves() {
    let mut g = game();
    for _ in 0..3 { g.put(0, RELIC, Zone::Battlefield); }
    let bear = g.put(1, BEAR, Zone::Battlefield);
    // cast with metalcraft, which is lost before the spell resolves
    let blast = g.put(0, BLAST, Zone::Hand);
    cast(&mut g, blast, vec![vec![Target::Player(1)]], "R");
    let relic = g.battlefield.iter().copied().find(|o| g.obj(*o).chars.name == "Relic").unwrap();
    let shatter = g.put(0, SHATTER, Zone::Hand);
    cast(&mut g, shatter, vec![vec![Target::Object(relic)]], "RR");
    pass_all(&mut g);
    assert!(!g.battlefield.contains(&relic));
    pass_all(&mut g);
    cite!(["608.2c"]; assert_eq!(g.players[1].life, 18));
    // cast before anything died; a creature dies in response
    let volley = g.put(0, VOLLEY, Zone::Hand);
    cast(&mut g, volley, vec![vec![Target::Player(1)]], "RRR");
    let murder = g.put(0, MURDER, Zone::Hand);
    cast(&mut g, murder, vec![vec![Target::Object(bear)]], "B");
    pass_all(&mut g);
    assert!(!g.battlefield.contains(&bear));
    pass_all(&mut g);
    cite!(["608.2c"]; assert_eq!(g.players[1].life, 13, "5 instead of 3"));
}

#[test]
fn morbid_damage_is_dealt_to_the_same_target() {
    let mut g = game();
    let bear = g.put(1, BEAR, Zone::Battlefield);
    let other = g.put(1, BEAR, Zone::Battlefield);
    g.set_initial_counters(other, "+1/+1", 2); // 4 toughness: three damage is insufficient
    resolve(&mut g, VOLLEY, vec![vec![Target::Player(1)]], "RRR");
    assert_eq!(g.players[1].life, 17, "3: nothing has died");
    resolve(&mut g, MURDER, vec![vec![Target::Object(bear)]], "B");
    resolve(&mut g, VOLLEY, vec![vec![Target::Object(other)]], "RRR");
    cite!(["608.2c"]; assert!(!g.battlefield.contains(&other), "the creature it targeted is dealt the 5"));
    cite!(["608.2c"]; assert_eq!(g.players[1].life, 17));
}

#[test]
fn one_of_the_two_is_done_in_the_same_paragraph() {
    let mut g = game();
    resolve(&mut g, FEAST, vec![], "G");
    cite!(["608.2c"]; assert_eq!(g.players[0].life, 22));
    for _ in 0..3 { g.put(0, RELIC, Zone::Battlefield); }
    resolve(&mut g, FEAST, vec![], "G");
    cite!(["608.2c"]; assert_eq!(g.players[0].life, 27, "5 instead of 2"));
}

#[test]
fn r601_2a_a_spell_knows_the_zone_it_was_cast_from() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let on = vec![vec![Target::Object(bear)]];
    resolve(&mut g, SAVAGERY, on.clone(), "GGGG");
    cite!(["608.2c"]; assert_eq!(counters(&g, bear), 5, "cast from the hand"));
    // now from the graveyard, for its flashback cost
    let [savagery] = g.players[0].graveyard[..] else { panic!("not in the graveyard") };
    g.players[0].pool.colors=[9;5];
    g.act(0,paid("GGGGGGG",Move::CastVia(game::announcement::CastRoute::Flashback,Box::new(Move::Cast{card:savagery,targets:on})))).unwrap();
    pass_all(&mut g);
    cite!(["608.2c"]; assert_eq!(counters(&g, bear), 15, "ten more on that creature, not five and not fifteen"));
    cite!(["702.34a"]; assert!(g.players[0].graveyard.is_empty() && g.exile.len() == 1, "and it is exiled (702.34a)"));
}
