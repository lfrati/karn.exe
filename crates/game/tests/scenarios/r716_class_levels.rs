//! Class level bars (716.2a): "[Cost]: Level N — [Abilities]" is "[Cost]: This Class's level
//! becomes N. Activate only if this Class is level N-1 and only as a sorcery" and "As long as
//! this Class is level N or greater, it has [abilities]". A permanent without a level is
//! level 1 (716.2d); a new object has none (400.7).
use crate::support::{cite, paid};
use game::state::Decision;
use ir::Ability;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::Part;

fn real(name: &str) -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let d = CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
    d
}
const TALENT: usize = 0;
const BOLT: usize = 1;
const ISLAND: usize = 2;
const BEAR: usize = 3;

fn game() -> Game {
    let defs = vec![real("Stormchaser's Talent"), CardDef::new("Shock", "{0}", "Instant", "Shock deals 2 damage to any target.", None).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(), CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap()];
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn mana(g: &mut Game) { g.players[0].pool.colors = [9; 5]; g.players[0].pool.colorless = 9; }
fn resolve(g: &mut Game) {
    for _ in 0..30 {
        match g.decision.clone() {
            None if g.stack.is_empty() => return,
            None => g.act(g.priority, Move::Pass).unwrap(),
            Some(Decision::Order { player, abilities }) => g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(),
            Some(d) => panic!("{d:?}"),
        }
    }
}
fn otters(g: &Game) -> usize { g.battlefield.iter().filter(|o| g.obj(**o).chars.name == "Otter Token").count() }
/// The index among its abilities of its level bar for `level`: the `level - 1`th activated one.
fn bar(g: &Game, o: ObjId, level: usize) -> usize {
    g.abilities(o).iter().enumerate().filter(|(_, a)| matches!(&***a, Ability::Activated { .. })).nth(level - 2).unwrap().0
}
fn level_up(g: &mut Game, o: ObjId, level: usize, units: &str) -> Result<(), String> {
    let ability = bar(g, o, level);
    g.act(0, paid(units, Move::Activate { source: o, ability, targets: vec![], paying: vec![] }))
}
fn talent(g: &mut Game) -> ObjId {
    let card = g.put(0, TALENT, Zone::Hand);
    mana(g);
    g.act(0, paid("U", Move::Cast { card, targets: vec![] })).unwrap();
    resolve(g);
    *g.battlefield.iter().find(|o| g.obj(**o).card == Some(TALENT)).unwrap()
}
fn shock(g: &mut Game, at: Target) {
    let card = g.put(0, BOLT, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![vec![at]] }).unwrap();
}

#[test]
fn r716_2a_level_one_has_only_its_first_section_and_no_level_three_bar_yet() {
    let mut g = game();
    let o = talent(&mut g);
    cite!(["716.3"]; assert_eq!(otters(&g), 1, "its enters ability (716.3)"));
    assert_eq!(g.obj(o).level, None);
    // level 3 needs level 2 first
    mana(&mut g);
    cite!(["716.2a"]; assert_eq!(level_up(&mut g, o, 3, "CCCCCU").unwrap_err(), "this Class becomes level 3 only from level 2 (716.2a)"));
    // no Otter for an instant before level 3
    shock(&mut g, Target::Player(1));
    resolve(&mut g);
    cite!(["716.2a"]; assert_eq!(otters(&g), 1));
}

#[test]
fn r716_2a_gaining_level_two_triggers_its_becomes_level_ability_and_level_three_gives_otters() {
    let mut g = game();
    let o = talent(&mut g);
    shock(&mut g, Target::Player(1));
    resolve(&mut g);
    let shock_card = *g.players[0].graveyard.last().unwrap();
    mana(&mut g);
    level_up(&mut g, o, 2, "CCCU").unwrap();
    // the level changes as the ability resolves, not as it is activated
    cite!(["716.2a"]; assert_eq!(g.obj(o).level, None));
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    cite!(["716.2a"]; assert_eq!(g.obj(o).level, Some(2)));
    assert!(g.log.contains(&Event::BecameLevel { object: o, level: 2 }));
    // "When this Class becomes level 2, return target instant or sorcery card from your
    // graveyard to your hand."
    match g.decision.clone() {
        Some(Decision::Targets { .. }) | None => {}
        d => panic!("{d:?}"),
    }
    if g.decision.is_some() {
        g.act(0, Move::ChooseTargets(vec![vec![Target::Object(shock_card)]])).unwrap();
    }
    resolve(&mut g);
    assert!(g.players[0].hand.iter().any(|c| g.obj(*c).card == Some(BOLT)), "Shock returned");
    // not again: it is level 2 now (716.2a)
    mana(&mut g);
    cite!(["716.2a"]; refused("this Class becomes level 2 only from level 1 (716.2a)"): level_up(&mut g, o, 2, "CCCU"));
    level_up(&mut g, o, 3, "CCCCCU").unwrap();
    resolve(&mut g);
    assert_eq!(g.obj(o).level, Some(3));
    let before = otters(&g);
    shock(&mut g, Target::Player(1));
    resolve(&mut g);
    cite!(["716.2a"]; assert_eq!(otters(&g), before + 1, "Whenever you cast an instant or sorcery spell, create an Otter"));
}

#[test]
fn r716_2a_a_level_is_gained_only_as_a_sorcery() {
    let mut g = game();
    let o = talent(&mut g);
    mana(&mut g);
    // with something on the stack
    let card = g.put(0, BEAR, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    cite!(["716.2a", "602.5d"]; refused("that ability is activated only as a sorcery: in a main phase of your own turn with the stack empty (602.5d)"): level_up(&mut g, o, 2, "CCCU"));
    resolve(&mut g);
    // in another player's turn
    let turn = g.turn;
    while g.turn == turn || g.part != Part::PrecombatMain {
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            None => g.act(g.priority, Move::Pass).unwrap(),
            Some(d) => panic!("{d:?}"),
        }
    }
    assert_eq!(g.active, 1);
    g.act(1, Move::Pass).unwrap();
    assert_eq!(g.priority, 0);
    mana(&mut g);
    cite!(["716.2a", "602.5d"]; refused("that ability is activated only as a sorcery: in a main phase of your own turn with the stack empty (602.5d)"): level_up(&mut g, o, 2, "CCCU"));
}

#[test]
fn r716_2b_another_talent_object_has_no_level_of_the_destroyed_one() {
    let mut g = game();
    let o = talent(&mut g);
    mana(&mut g);
    level_up(&mut g, o, 2, "CCCU").unwrap();
    resolve(&mut g);
    assert_eq!(g.obj(o).level, Some(2));
    g.destroy(o);
    let again = talent(&mut g);
    cite!(["400.7", "716.2b"]; assert_eq!(g.obj(again).level, None));
    mana(&mut g);
    cite!(["716.2a"]; refused("this Class becomes level 3 only from level 2 (716.2a)"): level_up(&mut g, again, 3, "CCCCCU"));
    level_up(&mut g, again, 2, "CCCU").unwrap();
}
