//! Valiant: "Whenever this creature becomes the target of a spell or ability you control for
//! the first time each turn" (603.2, 115.9). Each becoming-a-target counts, as it happens,
//! how many times this turn the object became the target of a spell or ability that player
//! controls. Heartfire Hero, Emberheart Challenger.
use crate::support::cite;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const HERO: usize = 0;
const CHALLENGER: usize = 1;
const PUMP: usize = 2;
const WAND: usize = 3;
const ISLAND: usize = 4;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_faces(&CardDef::catalog(&faces)[name]).unwrap();
    let c = |name, types, text| CardDef::new(name, "{0}", types, text, None).expect(name);
    let defs = vec![real("Heartfire Hero"), real("Emberheart Challenger"),
        c("Pump", "Instant", "Target creature gets +0/+0 until end of turn."),
        c("Wand", "Artifact", "{0}: Target creature gets +0/+0 until end of turn."),
        real("Island")];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn resolve(g: &mut Game) {
    while !g.stack.is_empty() || g.decision.is_some() {
        match g.decision.clone() {
            // prowess and valiant together: in the order listed
            Some(game::state::Decision::Order { player, abilities }) => g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
}
fn pump(g: &mut Game, player: usize, at: ObjId) {
    let spell = g.put(player, PUMP, Zone::Hand);
    g.act(player, Move::Cast { card: spell, targets: vec![vec![Target::Object(at)]] }).unwrap();
}
fn counters(g: &Game, o: ObjId) -> u32 { g.obj(o).counters.get("+1/+1").copied().unwrap_or(0) }

#[test]
fn r603_2_valiant_triggers_only_the_first_time_each_turn() {
    let mut g = game();
    let hero = g.put(0, HERO, Zone::Battlefield);
    pump(&mut g, 0, hero);
    resolve(&mut g);
    cite!(["603.2"]; assert_eq!(counters(&g, hero), 1));
    pump(&mut g, 0, hero);
    resolve(&mut g);
    assert_eq!(counters(&g, hero), 1, "not the first time this turn");
    // a spell and an ability on the stack together: only the first of them
    let wand = g.put(0, WAND, Zone::Battlefield);
    g.act(0, Move::Activate { source: wand, ability: 0, targets: vec![vec![Target::Object(hero)]], paying: vec![] }).unwrap();
    resolve(&mut g);
    assert_eq!(counters(&g, hero), 1);
}

#[test]
fn r603_2_an_ability_counts_as_a_spell_does() {
    let mut g = game();
    let hero = g.put(0, HERO, Zone::Battlefield);
    let wand = g.put(0, WAND, Zone::Battlefield);
    g.act(0, Move::Activate { source: wand, ability: 0, targets: vec![vec![Target::Object(hero)]], paying: vec![] }).unwrap();
    resolve(&mut g);
    cite!(["603.2"]; assert_eq!(counters(&g, hero), 1));
    pump(&mut g, 0, hero);
    resolve(&mut g);
    assert_eq!(counters(&g, hero), 1);
}

#[test]
fn r603_2_an_opponents_spell_neither_triggers_it_nor_uses_up_the_first_time() {
    let mut g = game();
    let hero = g.put(0, HERO, Zone::Battlefield);
    // the opponent targets it while holding priority after our pass
    g.act(0, Move::Pass).unwrap();
    pump(&mut g, 1, hero);
    resolve(&mut g);
    assert_eq!(counters(&g, hero), 0);
    pump(&mut g, 0, hero);
    resolve(&mut g);
    cite!(["603.2"]; assert_eq!(counters(&g, hero), 1, "the first time a spell its controller controls targets it"));
}

#[test]
fn r603_2_each_turn_counts_anew() {
    let mut g = game();
    let hero = g.put(0, HERO, Zone::Battlefield);
    pump(&mut g, 0, hero);
    resolve(&mut g);
    assert_eq!(counters(&g, hero), 1);
    // the opponent's turn: our instant at their upkeep is our spell, the first this turn
    while g.active == 0 || g.part != Part::Upkeep {
        match g.decision {
            Some(game::state::Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(game::state::Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    if g.priority != 0 { g.act(g.priority, Move::Pass).unwrap(); }
    pump(&mut g, 0, hero);
    resolve(&mut g);
    cite!(["603.2"]; assert_eq!(counters(&g, hero), 2));
}

#[test]
fn r603_2_emberheart_challenger_exiles_its_top_card_to_play_this_turn() {
    let mut g = game();
    let challenger = g.put(0, CHALLENGER, Zone::Battlefield);
    let top = *g.players[0].library.last().unwrap();
    pump(&mut g, 0, challenger);
    resolve(&mut g);
    let exiled = *g.exile.iter().find(|o| g.obj(**o).card == Some(ISLAND) && g.obj(**o).owner == 0).expect("the top card exiled");
    cite!(["400.7"]; assert_ne!(exiled, top, "a new object in exile (400.7)"));
    // until end of turn, its controller may play it: an Island, as the land for the turn
    g.act(0, Move::PlayLand(exiled)).unwrap();
    assert!(g.battlefield.iter().any(|o| g.obj(*o).card == Some(ISLAND)));
}
