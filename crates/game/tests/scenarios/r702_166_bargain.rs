//! Bargain (702.166): "As an additional cost to cast this spell, you may sacrifice an
//! artifact, enchantment, or token." Declaring it makes the spell "bargained" (702.166b), which
//! its other text asks. Torch the Tower, Ice Out, Hamlet Glutton.
use crate::support::cite;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const TORCH: usize = 0;
const ICE: usize = 1;
const GLUTTON: usize = 2;
const BEAR: usize = 3;
const GIANT: usize = 4;
const RELIC: usize = 5;
const CHARM: usize = 6;
const ISLAND: usize = 7;

fn real(name: &str) -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let def = CardDef::from_faces(&CardDef::catalog(&faces)[name]).unwrap();
    game::play::check(&def).unwrap_or_else(|e| panic!("{name}: {}", e.0));
    def
}
fn game() -> Game {
    let c = |name, types, pt| CardDef::new(name, "{0}", types, "", pt).expect(name);
    let mut g = Game::new(vec![real("Torch the Tower"), real("Ice Out"), real("Hamlet Glutton"),
        c("Bear", "Creature — Bear", Some((2, 2))), c("Giant", "Creature — Giant", Some((3, 3))),
        c("Relic", "Artifact", None), c("Charm", "Enchantment", None), real("Island")], 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn pool(g: &mut Game, colors: [u32; 5], colorless: u32) { g.players[0].pool.colors = colors; g.players[0].pool.colorless = colorless; }
fn resolve(g: &mut Game) {
    for _ in 0..30 {
        match g.asked() {
            Some((p, game::legal::Asked::Arrange { cards, .. })) => g.act(p, Move::Choose(game::state::Answer::Arrange { top: cards, away: vec![] })).unwrap(),
            _ if g.stack.is_empty() => return,
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
}
fn bargained(card: ObjId, fodder: ObjId, targets: Vec<Vec<Target>>) -> Move {
    Move::WithOptional(true, Box::new(Move::WithCosts(vec![vec![fodder]], Box::new(Move::Cast { card, targets }))))
}

fn exiled(g: &Game, card: usize) -> bool { g.exile.iter().any(|o| g.obj(*o).card == Some(card)) }

#[test]
fn r702_166a_bargained_only_with_something_to_sacrifice() {
    let mut g = game();
    let card = g.put(0, TORCH, Zone::Hand);
    let target = g.put(1, BEAR, Zone::Battlefield);
    pool(&mut g, [0, 0, 0, 1, 0], 0);
    // a creature that is not a token is not an artifact, enchantment, or token
    let bear = g.put(0, BEAR, Zone::Battlefield);
    cite!(["702.166a"]; refused("Bear is not what that cost asks for or cannot pay it"): unchanged(&mut g, |g| g.act(0, crate::support::paid("R", bargained(card, bear, vec![vec![Target::Object(target)]])))));
    cite!(["702.166a"]; ok: g.clone().act(0, crate::support::paid("R",Move::Cast { card, targets: vec![vec![Target::Object(target)]] })));
    let charm = g.put(0, CHARM, Zone::Battlefield);
    cite!(["702.166a"]; ok: g.clone().act(0, crate::support::paid("R",bargained(card, charm, vec![vec![Target::Object(target)]]))));
    // a spell without bargain announces none
    let ice_free = g.put(0, BEAR, Zone::Hand);
    cite!(["702.166a", "118.8b"]; refused("that spell has no optional additional cost (118.8b)"): g.act(0, Move::WithOptional(false, Box::new(Move::Cast { card: ice_free, targets: vec![] }))));
}

#[test]
fn r702_166b_torch_the_tower_bargained_deals_3_and_scries_and_exiles() {
    for (fodder_kind, bargain) in [(RELIC, true), (CHARM, true), (RELIC, false)] {
        let mut g = game();
        let card = g.put(0, TORCH, Zone::Hand);
        let giant = g.put(1, GIANT, Zone::Battlefield);
        let fodder = g.put(0, fodder_kind, Zone::Battlefield);
        pool(&mut g, [0, 0, 0, 1, 0], 0);
        let targets = vec![vec![Target::Object(giant)]];
        g.act(0, crate::support::paid("R",if bargain { bargained(card, fodder, targets) } else { Move::Cast { card, targets } })).unwrap();
        // the sacrifice is a cost: done as it is cast (601.2h)
        cite!(["601.2h"]; assert_eq!(g.obj(fodder).zone == Zone::Battlefield, !bargain));
        resolve(&mut g);
        let case = format!("fodder={fodder_kind} bargain={bargain}");
        cite!(["702.166b"]; case: case; assert_eq!(exiled(&g, GIANT), bargain, "3 damage kills the 3/3 and it is exiled instead"));
        if !bargain { cite!(["702.166b"]; case: case; assert_eq!(g.obj(giant).damage, 2)); }
    }
}

#[test]
fn r702_166a_a_token_is_bargained() {
    let mut g = game();
    let card = g.put(0, TORCH, Zone::Hand);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    let bear = g.put(1, BEAR, Zone::Battlefield);
    let token = g.create_token(0, g.obj(bear).chars.clone(), false);
    pool(&mut g, [0, 0, 0, 1, 0], 0);
    g.act(0, crate::support::paid("R",bargained(card, token, vec![vec![Target::Object(giant)]]))).unwrap();
    resolve(&mut g);
    cite!(["702.166a", "702.166b"]; assert!(exiled(&g, GIANT)));
    // something not an artifact, enchantment or token is not bargained with
    let mut g = game();
    let card = g.put(0, TORCH, Zone::Hand);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    g.put(0, RELIC, Zone::Battlefield);
    pool(&mut g, [0, 0, 0, 1, 0], 0);
    cite!(["702.166a"]; refused("Bear is not what that cost asks for or cannot pay it (701.21a, 118.3)"): g.act(0, bargained(card, bear, vec![vec![Target::Object(giant)]])));
    assert_eq!(g.obj(bear).zone, Zone::Battlefield);
}

#[test]
fn r702_166b_this_spell_costs_less_if_its_bargained() {
    // Ice Out {1}{U}{U}: {U}{U} bargained
    let mut g = game();
    let card = g.put(0, ICE, Zone::Hand);
    let relic = g.put(0, RELIC, Zone::Battlefield);
    // something to counter: our own Torch the Tower
    let torch = g.put(0, TORCH, Zone::Hand);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    pool(&mut g, [0, 2, 0, 1, 0], 0);
    g.act(0, crate::support::paid("R",Move::Cast { card: torch, targets: vec![vec![Target::Object(giant)]] })).unwrap();
    let spell = g.stack[0];
    g.act(0, crate::support::paid("UU",bargained(card, relic, vec![vec![Target::Object(spell)]]))).unwrap();
    cite!(["702.166b", "601.2f"]; assert_eq!(g.players[0].pool.total(), 0, "{{U}}{{U}}: one less"));
    resolve(&mut g);
    assert_eq!(g.obj(giant).damage, 0);
    // unbargained it costs all three
    let mut g = game();
    let card = g.put(0, ICE, Zone::Hand);
    g.put(0, RELIC, Zone::Battlefield);
    let torch = g.put(0, TORCH, Zone::Hand);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    pool(&mut g, [0, 2, 0, 1, 0], 0);
    g.act(0, crate::support::paid("R",Move::Cast { card: torch, targets: vec![vec![Target::Object(giant)]] })).unwrap();
    let spell = g.stack[0];
    cite!(["702.166b"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0, crate::support::paid("UU",Move::Cast { card, targets: vec![vec![Target::Object(spell)]] })));
}

#[test]
fn r702_166b_a_bargained_permanent_spell_costs_less() {
    // Hamlet Glutton {5}{G}{G}: {3}{G}{G} bargained
    let mut g = game();
    let card = g.put(0, GLUTTON, Zone::Hand);
    let relic = g.put(0, RELIC, Zone::Battlefield);
    pool(&mut g, [0, 0, 0, 0, 2], 3);
    cite!(["702.166b", "601.2f"]; ok: g.act(0, crate::support::paid("CCCGG",bargained(card, relic, vec![]))));
    resolve(&mut g);
    assert!(g.battlefield.iter().any(|o| g.obj(*o).card == Some(GLUTTON)));
    assert_eq!(g.players[0].life, 23);
}

#[test]
fn r702_166d_a_target_only_a_bargained_spell_has_is_chosen_only_then() {
    let text = "Bargain\nYou gain 1 life. If this spell was bargained, destroy target creature.";
    let def = CardDef::new("Grim Bargain", "{0}", "Sorcery", text, None).unwrap();
    game::play::check(&def).unwrap();
    let mut defs = vec![real("Torch the Tower"), real("Ice Out"), real("Hamlet Glutton")];
    let c = |name, types, pt| CardDef::new(name, "{0}", types, "", pt).expect(name);
    defs.extend([c("Bear", "Creature — Bear", Some((2, 2))), def, c("Relic", "Artifact", None)]);
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, 1, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    let bear = g.put(1, BEAR, Zone::Battlefield);
    let relic = g.put(0, 5, Zone::Battlefield);
    // unbargained: no target is chosen, and naming one is refused
    let card = g.put(0, 4, Zone::Hand);
    cite!(["702.166d"]; refused("wrong number of targets: 1 chosen, it takes 0 (601.2c)"): g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(bear)]] }));
    cite!(["702.166d"]; ok: g.act(0, Move::Cast { card, targets: vec![vec![]] }));
    resolve(&mut g);
    cite!(["702.166d"]; assert_eq!((g.players[0].life, g.obj(bear).zone), (21, Zone::Battlefield)));
    // bargained: the target is chosen and destroyed
    let card = g.put(0, 4, Zone::Hand);
    cite!(["702.166d"]; refused("wrong number of targets: 0 chosen, it takes 1 (601.2c)"): g.act(0, bargained(card, relic, vec![vec![]])));
    g.act(0, bargained(card, relic, vec![vec![Target::Object(bear)]])).unwrap();
    resolve(&mut g);
    cite!(["702.166d"]; assert_eq!((g.players[0].life, g.obj(bear).zone), (22, Zone::Gone)));
}

#[track_caller]
fn unchanged(g: &mut Game, action: impl FnOnce(&mut Game) -> Result<(), String>) -> Result<(), String> {
    let before=format!("{g:?}");
    let result=action(g);
    assert_eq!(format!("{g:?}"),before);
    result
}
