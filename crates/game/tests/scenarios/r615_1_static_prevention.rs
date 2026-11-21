//! A permanent's "Prevent all [combat] damage that would be dealt to … [by …]" (615.1): a
//! prevention effect in force while the ability functions, of the damage it describes only.
use crate::support::{cite, paid};
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const FOG_BANK: usize = 0;
const DAWN: usize = 1;
const INVIOLABILITY: usize = 2;
const LANCER: usize = 3;
const MUZZLE: usize = 4;
const SANCTUARY: usize = 5;
const LYNX: usize = 6;
const SHOCK: usize = 7;
const GIANT: usize = 8;
const BEAR: usize = 9;
const MOUNTAIN: usize = 10;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap();
    let c = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).expect(name);
    let defs = vec![
        real("Fog Bank"),
        real("Dawn Elemental"),
        real("Inviolability"),
        real("Champion Lancer"),
        real("Muzzle"),
        real("Blessed Sanctuary"),
        real("Sunspine Lynx"),
        real("Shock"),
        c("Giant", "Creature — Giant", "", Some((5, 5))),
        c("Bear", "Creature — Bear", "", Some((2, 2))),
        c("Mountain", "Basic Land — Mountain", "", None),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, MOUNTAIN, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn cast(g: &mut Game, card: usize, targets: Vec<Vec<Target>>, spending: &str) {
    let spell = g.put(0, card, Zone::Hand);
    g.players[0].pool.colors = [9; 5];
    g.players[0].pool.colorless = 9;
    g.act(0, paid(spending, Move::Cast { card: spell, targets })).unwrap();
    while !g.stack.is_empty() && g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
}
fn shock(g: &mut Game, to: Target) { cast(g, SHOCK, vec![vec![to]], "R"); }
/// Attack player 1 with `attacker`; player 1 blocks with `blocker`, if any. Through damage.
fn fight(g: &mut Game, attacker: ObjId, blocker: Option<ObjId>) {
    while g.decision != Some(Decision::Attackers { player: 0 }) { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Attack(vec![(attacker, Target::Player(1))])).unwrap();
    while g.part != Part::EndOfCombat {
        if g.decision == Some(Decision::Blockers { player: 1 }) {
            g.act(1, Move::Block(blocker.map(|b| (b, attacker)).into_iter().collect())).unwrap();
        } else { g.act(g.priority, Move::Pass).unwrap(); }
    }
}

#[test]
fn r615_1_fog_bank_prevents_combat_damage_to_and_by_it_but_not_noncombat() {
    let mut g = game();
    let giant = g.put(0, GIANT, Zone::Battlefield);
    let bank = g.put(1, FOG_BANK, Zone::Battlefield);
    fight(&mut g, giant, Some(bank));
    assert!(g.battlefield.contains(&bank));
    cite!(["615.1a"]; assert_eq!(g.obj(bank).damage, 0));
    // noncombat damage is not combat damage: Shock kills the 0/2
    shock(&mut g, Target::Object(bank));
    // "Prevent all combat damage that would be dealt to and dealt by this creature": Shock's damage is dealt by a spell, not combat damage
    cite!(["120.2a", "120.2b"]; assert!(!g.battlefield.contains(&bank)));
}

#[test]
fn r615_1_dawn_elemental_prevents_all_damage_to_it() {
    let mut g = game();
    let dawn = g.put(1, DAWN, Zone::Battlefield);
    shock(&mut g, Target::Object(dawn));
    cite!(["615.1a"]; assert_eq!(g.obj(dawn).damage, 0));
    let giant = g.put(0, GIANT, Zone::Battlefield);
    fight(&mut g, giant, Some(dawn));
    assert!(g.battlefield.contains(&dawn));
    cite!(["615.1a"]; assert_eq!(g.obj(dawn).damage, 0));
}

#[test]
fn r615_1_an_aura_protects_what_it_enchants_while_attached() {
    let mut g = game();
    let bear = g.put(1, BEAR, Zone::Battlefield);
    cast(&mut g, INVIOLABILITY, vec![vec![Target::Object(bear)]], "CW");
    let aura = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(INVIOLABILITY)).unwrap();
    assert_eq!(g.obj(aura).attached_to, Some(Target::Object(bear)));
    shock(&mut g, Target::Object(bear));
    cite!(["615.1a"]; assert_eq!(g.obj(bear).damage, 0));
    // other creatures are not enchanted by it
    let other = g.put(1, BEAR, Zone::Battlefield);
    shock(&mut g, Target::Object(other));
    cite!(["303.4m"]; assert!(!g.battlefield.contains(&other)));
    // once it is gone, the ability no longer functions (611.3a)
    g.move_object(aura, Zone::Graveyard);
    shock(&mut g, Target::Object(bear));
    cite!(["604.2"]; assert!(!g.battlefield.contains(&bear)));
}

#[test]
fn r615_1_only_damage_by_the_described_sources_is_prevented() {
    let mut g = game();
    let lancer = g.put(1, LANCER, Zone::Battlefield);
    let giant = g.put(0, GIANT, Zone::Battlefield);
    fight(&mut g, giant, Some(lancer));
    assert!(g.battlefield.contains(&lancer));
    cite!(["615.1a"]; assert_eq!(g.obj(lancer).damage, 0, "a creature's damage is prevented"));
    // Shock is not a creature
    shock(&mut g, Target::Object(lancer));
    // Uncited (card text): "Prevent all damage that would be dealt to this creature by creatures": Shock is an instant
    assert_eq!(g.obj(lancer).damage, 2);
}

#[test]
fn r615_1_damage_by_the_enchanted_creature_is_prevented() {
    let mut g = game();
    let giant = g.put(0, GIANT, Zone::Battlefield);
    cast(&mut g, MUZZLE, vec![vec![Target::Object(giant)]], "CW");
    fight(&mut g, giant, None);
    cite!(["615.1a"]; assert_eq!(g.players[1].life, 20));
}

#[test]
fn r615_1_noncombat_damage_to_you_and_your_creatures_only() {
    let mut g = game();
    g.put(1, SANCTUARY, Zone::Battlefield);
    let bear = g.put(1, BEAR, Zone::Battlefield);
    shock(&mut g, Target::Player(1));
    shock(&mut g, Target::Object(bear));
    cite!(["615.1a"]; assert_eq!((g.players[1].life, g.obj(bear).damage), (20, 0)));
    // not its opponent, nor combat damage to its controller
    shock(&mut g, Target::Player(0));
    // Uncited (card text): "Prevent all noncombat damage that would be dealt to you and creatures you control": not its controller's opponent
    assert_eq!(g.players[0].life, 18);
    let giant = g.put(0, GIANT, Zone::Battlefield);
    fight(&mut g, giant, None);
    // the same "noncombat damage": the Giant's combat damage is dealt
    cite!(["120.2a", "510.2"]; assert_eq!(g.players[1].life, 15));
}

#[test]
fn r615_12_damage_that_cant_be_prevented_is_dealt() {
    let mut g = game();
    let dawn = g.put(1, DAWN, Zone::Battlefield);
    g.put(0, LYNX, Zone::Battlefield);
    shock(&mut g, Target::Object(dawn));
    cite!(["615.12"]; assert_eq!(g.obj(dawn).damage, 2));
}
