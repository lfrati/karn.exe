//! Role tokens (303.7a, 704.5z): an Aura gets a new timestamp each time it becomes
//! attached (613.7e), so a Role moved onto a creature is the newest there; a Role whose control
//! changes joins its new controller's Roles; a Role whose creature dies goes to the graveyard
//! (704.5m), and a Wicked one drains as it does.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const RAGE: usize = 0;
const BEAR: usize = 1;
const CURSE: usize = 2;
const WICKED: usize = 3; // Witch's Mark
const MOVE: usize = 4;
const STEAL: usize = 5;
const MURDER: usize = 6;
const ISLAND: usize = 7;

fn real(name: &str) -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    CardDef::from_faces(&CardDef::catalog(&faces)[name]).unwrap()
}
fn game() -> Game {
    let c = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).expect(name);
    let defs = vec![real("Monstrous Rage"), c("Bear", "Creature — Bear", "", Some((2, 2))),
        c("Curse", "Instant", "Create a Cursed Role token attached to target creature.", None),
        real("Witch's Mark"),
        c("Move", "Instant", "Attach target Aura to target creature.", None),
        c("Steal", "Instant", "Gain control of target permanent.", None),
        // Synthetic free removal isolates the Role death trigger; Oracle Murder costs 1BB.
        c("Murder", "Instant", "Destroy target creature.", None),
        real("Island")];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn cast(g: &mut Game, player: usize, card: usize, targets: Vec<Vec<Target>>, units: &str) {
    let spell = g.put(player, card, Zone::Hand);
    g.players[player].pool.colors = [9; 5];
    g.players[player].pool.colorless = 9;
    if g.priority != player { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(player, paid(units, Move::Cast { card: spell, targets })).unwrap();
    // Witch's Mark's "You may discard a card": declined
    while !g.stack.is_empty() {
        match g.decision.clone() {
            Some(game::state::Decision::Choose { player, .. }) => g.act(player, Move::Choose(game::state::Answer::No)).unwrap(),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
}
fn roles_on(g: &Game, creature: ObjId) -> Vec<ObjId> {
    g.battlefield.iter().copied().filter(|o| g.obj(*o).attached_to == Some(Target::Object(creature))).collect()
}
fn on(t: ObjId) -> Vec<Vec<Target>> { vec![vec![Target::Object(t)]] }

/// The Monster Role is older than the Cursed one, but attached to the Cursed Bear after it:
/// the Monster is the newest there and the Cursed Role goes (613.7e, 704.5z).
#[test]
fn a_role_moved_onto_a_creature_is_its_newest() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let other = g.put(0, BEAR, Zone::Battlefield);
    cast(&mut g, 0, RAGE, on(other), "R");
    let [monster] = roles_on(&g, other)[..] else { panic!() };
    cast(&mut g, 0, CURSE, on(bear), "");
    let [cursed] = roles_on(&g, bear)[..] else { panic!() };
    cast(&mut g, 0, MOVE, vec![vec![Target::Object(monster)], vec![Target::Object(bear)]], "");
    cite!(["613.7e", "704.5z"]; assert_eq!(roles_on(&g, bear), [monster]));
    cite!(["704.5z", "111.7"]; assert_eq!(g.obj(cursed).zone, Zone::Gone));
}

/// Player 1's Cursed Role and player 0's Monster Role on player 0's Bear: when player 0 gains
/// control of the Cursed one, it controls two Roles there, and only the newer stays.
#[test]
fn a_role_whose_control_changes_joins_its_new_controllers_roles() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    cast(&mut g, 0, RAGE, on(bear), "R");
    let [monster] = roles_on(&g, bear)[..] else { panic!() };
    cast(&mut g, 1, CURSE, on(bear), "");
    let cursed = *roles_on(&g, bear).iter().find(|r| **r != monster).unwrap();
    assert_eq!(roles_on(&g, bear).len(), 2);
    cast(&mut g, 0, STEAL, on(cursed), "");
    cite!(["704.5z"]; assert_eq!(roles_on(&g, bear), [cursed], "the Cursed Role is the newer"));
    cite!(["704.5z", "111.7"]; assert_eq!(g.obj(monster).zone, Zone::Gone));
}

/// "When this token is put into a graveyard from the battlefield, each opponent loses 1
/// life.": when its creature dies, the Role is put into the graveyard too (704.5m).
#[test]
fn a_wicked_role_whose_creature_dies_drains() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    cast(&mut g, 0, WICKED, on(bear), "CR");
    let [wicked] = roles_on(&g, bear)[..] else { panic!() };
    cast(&mut g, 0, MURDER, on(bear), "");
    cite!(["704.5m", "111.7"]; assert_eq!(g.obj(wicked).zone, Zone::Gone));
    cite!(["603.6c", "603.10a"]; assert_eq!(g.players[1].life, 19));
    assert_eq!(g.players[0].life, 20);
}
