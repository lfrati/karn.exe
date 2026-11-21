//! "each green creature that entered this turn" (608.2i): a permanent that came onto the
//! battlefield in the current turn, as the object it is now (400.7).
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Zone};

const RIEF: usize = 0;
const BEAR: usize = 1;
const GOBLIN: usize = 2;
const LAND: usize = 3;
const SWEEP: usize = 4;

fn game() -> Game {
    let defs = vec![
        CardDef::new("Oran-Rief, the Vastwood", "", "Land", "This land enters tapped.\n{T}: Add {G}.\n{T}: Put a +1/+1 counter on each green creature that entered this turn.", None).unwrap(),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Goblin", "{R}", "Creature — Goblin", "", Some((1, 1))).unwrap(),
        CardDef::new("Wastes", "", "Land", "", None).unwrap(),
        CardDef::new("Sweep", "{G}", "Instant", "Destroy each creature that didn't enter this turn.", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    let mut g = Game::new(defs, 2, 20);
    for player in 0..2 { for _ in 0..8 { g.put(player, LAND, Zone::Library); } }
    g
}
fn pass_all(g: &mut Game) {
    for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); }
}
/// The first player casts this card with mana given for it, and it resolves.
fn resolve(g: &mut Game, card: usize, spending: &str) {
    let spell = g.put(0, card, Zone::Hand);
    g.players[0].pool.colors = [9, 9, 9, 9, 9];
    g.act(0, paid(spending, Move::Cast { card: spell, targets: vec![] })).unwrap();
    pass_all(g);
    g.players[0].pool = Default::default();
}
fn named(g: &Game, name: &str) -> Vec<ObjId> {
    g.battlefield.iter().copied().filter(|o| g.obj(*o).chars.name == name).collect()
}
fn counters(g: &Game, on: ObjId) -> u32 {
    g.obj(on).counters.get("+1/+1").copied().unwrap_or(0)
}
fn grow(g: &mut Game, rief: ObjId) {
    g.act(0, Move::Activate { source: rief, ability: 2, targets: vec![], paying: vec![] }).unwrap();
    pass_all(g);
}

#[test]
fn only_what_entered_in_this_turn_is_described() {
    let mut g = game();
    let rief = g.put(0, RIEF, Zone::Battlefield);
    let old = g.put(0, BEAR, Zone::Battlefield);
    resolve(&mut g, BEAR, "GG");
    resolve(&mut g, GOBLIN, "R");
    let new = named(&g, "Bear").into_iter().find(|o| *o != old).expect("the Bear that was cast");
    let [goblin] = named(&g, "Goblin")[..] else { panic!("no Goblin") };
    grow(&mut g, rief);
    cite!(["608.2i"]; assert_eq!(counters(&g, new), 1, "green, and it entered this turn"));
    cite!(["608.2i"]; assert_eq!(counters(&g, old), 0, "it was here before the turn began"));
    assert_eq!(counters(&g, goblin), 0, "it entered this turn, and is not green");
    // in a later turn, nothing of this turn's is still what entered "this turn"
    let mut t = crate::support::T::new(g);
    t.to_no_combat(1, ir::Part::PrecombatMain);
    let mut g = t.g;
    while g.priority != 0 { g.act(g.priority, Move::Pass).unwrap(); }
    let another = g.put(0, RIEF, Zone::Battlefield);
    grow(&mut g, another);
    cite!(["608.2i"]; assert_eq!(counters(&g, new), 1));
}

#[test]
fn what_did_not_enter_this_turn_is_everything_else() {
    let mut g = game();
    let old = g.put(1, BEAR, Zone::Battlefield);
    resolve(&mut g, GOBLIN, "R");
    resolve(&mut g, SWEEP, "G");
    cite!(["608.2i"]; assert!(!g.battlefield.contains(&old)));
    cite!(["608.2i"]; assert_eq!(named(&g, "Goblin").len(), 1, "it entered this turn"));
}
