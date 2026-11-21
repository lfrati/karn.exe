//! "…, then … again" (Jadelight Ranger, Defossilize, Over the Edge,
//! Roalesk): the act is done twice, the second time on the state the first left.
use crate::support::cite;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::Part;

const RANGER: usize = 0;
const DEFOSSILIZE: usize = 1;
const EDGE: usize = 2;
const ROALESK: usize = 3;
const BEAR: usize = 4;
const LAND: usize = 5;
const KILL: usize = 6;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap();
    let defs = vec![
        real("Jadelight Ranger"), real("Defossilize"), real("Over the Edge"), real("Roalesk, Apex Hybrid"),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Land", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Kill", "{0}", "Instant", "Destroy target creature.", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn explored(g: &Game) -> usize { g.log.iter().filter(|e| matches!(e, Event::Explored { .. })).count() }
fn plus(g: &Game, o: ObjId) -> u32 { g.obj(o).counters.get("+1/+1").copied().unwrap_or(0) }
fn mana(g: &mut Game) { g.players[0].pool.colors = [5; 5]; g.players[0].pool.colorless = 5; }
/// Resolve everything: "may" answered no, holders by `holders`, targets by `targets`.
fn settle(g: &mut Game, holders: &mut dyn FnMut(&[Target]) -> Vec<Target>, targets: Vec<Vec<Target>>) -> usize {
    let mut asked = 0;
    for _ in 0..60 {
        match g.decision.clone() {
            Some(Decision::Choose { player, choice: Choice::May }) => g.act(player, Move::Choose(Answer::No)).unwrap(),
            Some(Decision::Choose { player, choice: Choice::Holders { among, .. } }) => { asked += 1; g.act(player, Move::Choose(Answer::Holders(holders(&among)))).unwrap(); }
            Some(Decision::Targets { player, .. }) => g.act(player, Move::ChooseTargets(targets.clone())).unwrap(),
            Some(Decision::Order { player, abilities }) => g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(),
            Some(d) => panic!("{d:?}"),
            None if g.stack.is_empty() => return asked,
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("did not settle")
}

#[test]
fn r701_44a_with_an_empty_library_each_explore_still_gives_a_counter() {
    let mut g = game();
    let card = g.put(0, RANGER, Zone::Hand);
    mana(&mut g);
    g.act(0, crate::support::paid("CGG", Move::Cast { card, targets: vec![] })).unwrap();
    settle(&mut g, &mut |_| vec![], vec![]);
    let r = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(RANGER)).unwrap();
    cite!(["701.44a", "701.44b"]; assert_eq!((plus(&g, r), explored(&g)), (2, 2), "no land revealed, twice (701.44a-b)"));
}

#[test]
fn r701_44a_defossilize_returns_the_creature_and_it_explores_twice() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Graveyard);
    g.put(0, LAND, Zone::Library);
    g.put(0, LAND, Zone::Library);
    let card = g.put(0, DEFOSSILIZE, Zone::Hand);
    mana(&mut g);
    g.act(0, crate::support::paid("CCCCB", Move::Cast { card, targets: vec![vec![Target::Object(bear)]] })).unwrap();
    settle(&mut g, &mut |_| vec![], vec![]);
    let back = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(BEAR)).unwrap();
    cite!(["701.44a"]; assert_eq!(plus(&g, back), 0));
    cite!(["701.44a", "608.2c"]; assert_eq!(g.players[0].hand.iter().filter(|o| g.obj(**o).card == Some(LAND)).count(), 2));
    cite!(["608.2c"]; assert_eq!(explored(&g), 2));
}

#[test]
fn r701_44a_over_the_edge_explores_its_target_twice_and_not_after_it_is_gone() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let card = g.put(0, EDGE, Zone::Hand);
    mana(&mut g);
    g.act(0, crate::support::paid("CG", Move::Modes(vec![1], Box::new(Move::Cast { card, targets: vec![vec![Target::Object(bear)]] })))).unwrap();
    settle(&mut g, &mut |_| vec![], vec![]);
    cite!(["701.44a", "608.2c"]; assert_eq!((plus(&g, bear), explored(&g)), (2, 2)));

    // the target is gone as it resolves: the spell does nothing (608.2b)
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let card = g.put(0, EDGE, Zone::Hand);
    let kill = g.put(0, KILL, Zone::Hand);
    mana(&mut g);
    g.act(0, crate::support::paid("CG", Move::Modes(vec![1], Box::new(Move::Cast { card, targets: vec![vec![Target::Object(bear)]] })))).unwrap();
    g.act(0, Move::Cast { card: kill, targets: vec![vec![Target::Object(bear)]] }).unwrap();
    settle(&mut g, &mut |_| vec![], vec![]);
    cite!(["608.2b"]; assert_eq!(explored(&g), 0));
}

#[test]
fn r701_34a_roalesk_proliferates_twice_the_second_seeing_the_first() {
    let mut g = game();
    let roalesk = g.put(0, ROALESK, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    g.put_counters(bear, "+1/+1", 1);
    let kill = g.put(0, KILL, Zone::Hand);
    g.act(0, Move::Cast { card: kill, targets: vec![vec![Target::Object(roalesk)]] }).unwrap();
    let asked = settle(&mut g, &mut |among| among.to_vec(), vec![]);
    cite!(["701.34a", "608.2c"]; assert_eq!(asked, 2, "one choice per proliferate"));
    cite!(["701.34a", "608.2c"]; assert_eq!(plus(&g, bear), 3));
}

#[test]
fn again_restates_only_the_same_act_in_its_own_sentence() {
    for text in ["Draw a card. Draw a card again.", "Destroy target creature, then draw a card again."] {
        let def = CardDef::new("Again", "{0}", "Instant", text, None).unwrap();
        crate::support::refused(game::play::check(&def).map_err(|e|e.0),"modifier: Again of Draw");
    }
}
