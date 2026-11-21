//! A cost that is a choice among alternatives, as the players are told of it: each
//! alternative said as its text says it ("sacrifice a Food"), the one announced shown to
//! every player on the stack (601.2b, 602.2b), a permanent beheld told of as a card beheld
//! from the hand is revealed (701.4a); and a mana ability whose cost is such a choice paid
//! either way as a step of a payment (601.2g, 605.3a).
use crate::support::{cite,paid};
use game::legal::Step;
use game::snapshot::Snapshot;
use game::{CardDef, Game, Move, ObjId, Zone};
use ir::Part;

const NAMES: [&str; 2] = ["Thornvault Forager", "Kinsbaile Aspirant"];
const FORAGER: usize = 0;
const ASPIRANT: usize = 1;
const WASTES: usize = 2;
const BEAR: usize = 3;
const KITHKIN: usize = 4;
const FOOD: usize = 5;
const POST: usize = 6;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let mut defs: Vec<CardDef> = NAMES.iter().map(|n| CardDef::from_faces(&catalog[*n]).unwrap()).collect();
    let c = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    defs.extend([
        c("Wastes", "", "Basic Land", "{T}: Add {C}.", None),
        c("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        c("Kithkin", "{W}", "Creature — Kithkin", "", Some((1, 1))),
        c("Food", "", "Artifact — Food", "{2}, {T}, Sacrifice this artifact: You gain 3 life.", None),
        c("Trading Post", "{0}", "Artifact", "Discard a card or pay 2 life: You gain 5 life.", None),
    ]);
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..8 { g.put(p, WASTES, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn announced(g: &Game, viewer: usize) -> Vec<String> {
    let top = *g.stack.last().expect("something on the stack");
    Snapshot::of(g, Some(viewer)).objects.into_iter().find(|o| o.id == top).and_then(|o| o.on_stack).map(|s| s.announced).unwrap_or_default()
}
fn beheld(g: &Game) -> Vec<(usize, Vec<ObjId>)> {
    g.log.iter().filter_map(|e| match e { game::Event::Beheld { player, permanents } => Some((*player, permanents.clone())), _ => None }).collect()
}
#[test]
fn r601_2b_the_alternative_announced_is_shown_to_every_player() {
    let mut g = game();
    let aspirant = g.put(0, ASPIRANT, Zone::Hand);
    let kithkin = g.put(0, KITHKIN, Zone::Battlefield);
    g.players[0].pool.colors[0] = 1;
    g.act(0, Move::WithCostChoice(vec![0], Box::new(Move::WithCosts(vec![vec![kithkin]], Box::new(paid("W",Move::Cast { card: aspirant, targets: vec![] })))))).unwrap();
    cite!(["601.2b"]; assert_eq!((announced(&g, 0), announced(&g, 1)), (vec!["behold a Kithkin".to_string()], vec!["behold a Kithkin".to_string()])));
    // the permanent beheld is told of: the opponent can tell it from {2} paid (701.4a)
    cite!(["701.4a"]; assert_eq!(beheld(&g), [(0, vec![kithkin])]));
    cite!(["701.4a"]; assert!(g.battlefield.contains(&kithkin) && !g.obj(kithkin).tapped));
}

#[test]
fn r601_2b_the_other_alternative_announced_and_a_card_beheld_from_the_hand() {
    // {2} paid instead: no behold is told of
    let mut g = game();
    let aspirant = g.put(0, ASPIRANT, Zone::Hand);
    g.put(0, KITHKIN, Zone::Battlefield);
    g.players[0].pool.colors[0] = 1;
    g.players[0].pool.colorless = 2;
    g.act(0, Move::WithCostChoice(vec![1], Box::new(paid("CCW",Move::Cast { card: aspirant, targets: vec![] })))).unwrap();
    cite!(["601.2b"]; assert_eq!(announced(&g, 1), ["pay {2}"]));
    cite!(["701.4a"]; assert!(beheld(&g).is_empty()));
    // a Kithkin card beheld from the hand is revealed (701.4a, 701.20a), not a permanent beheld
    let mut g = game();
    let aspirant = g.put(0, ASPIRANT, Zone::Hand);
    let card = g.put(0, KITHKIN, Zone::Hand);
    g.players[0].pool.colors[0] = 1;
    g.act(0, Move::WithCostChoice(vec![0], Box::new(Move::WithCosts(vec![vec![card]], Box::new(paid("W",Move::Cast { card: aspirant, targets: vec![] })))))).unwrap();
    cite!(["701.4a", "701.20a"]; assert!(beheld(&g).is_empty() && g.log.iter().any(|e| matches!(e, game::Event::Revealed { cards, .. } if cards == &vec![card]))));
    cite!(["601.2b"]; assert_eq!(announced(&g, 1), ["behold a Kithkin"]));
}

#[test]
fn r602_2b_an_ability_on_the_stack_shows_the_alternative_announced() {
    let mut g = game();
    let post = g.put(0, POST, Zone::Battlefield);
    g.act(0, Move::WithCostChoice(vec![1], Box::new(Move::Activate { source: post, ability: 0, targets: vec![], paying: vec![] }))).unwrap();
    cite!(["602.2b", "601.2b"]; assert_eq!((announced(&g, 0), announced(&g, 1), g.players[0].life), (vec!["pay 2 life".to_string()], vec!["pay 2 life".to_string()], 18)));
}

#[test]
fn r605_3a_a_forage_mana_ability_pays_for_a_spell_either_way() {
    // three cards in the graveyard and no Food: the step exiles them (601.2g, 605.3a, 701.61a)
    let mut g = game();
    let forager = g.put(0, FORAGER, Zone::Battlefield);
    let cards: Vec<_> = (0..3).map(|_| g.put(0, WASTES, Zone::Graveyard)).collect();
    let bear = g.put(0, BEAR, Zone::Hand);
    let plan = vec![Step { plan: vec![], x: None, life: vec![], with: None, source:forager, ability:1, paying:vec![cards.clone()], adding:vec![vec![ir::mana::ManaSymbol::Color(ir::Color::Green);2]], chose:vec![0], cost_branches:None }];
    g.act(0, Move::WithMana(plan, Box::new(paid("GG",Move::Cast { card:bear, targets:vec![] })))).unwrap();
    cite!(["701.61a", "601.2h"]; assert!(cards.iter().all(|c| g.obj(*c).zone != Zone::Graveyard) && g.players[0].graveyard.is_empty()));
    // a Food and an empty graveyard: the step sacrifices the Food
    let mut g = game();
    let forager = g.put(0, FORAGER, Zone::Battlefield);
    let food = g.put(0, FOOD, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Hand);
    let plan = vec![Step { plan: vec![], x: None, life: vec![], with: None, source:forager, ability:1, paying:vec![vec![food]], adding:vec![vec![ir::mana::ManaSymbol::Color(ir::Color::Green);2]], chose:vec![1], cost_branches:None }];
    g.act(0, Move::WithMana(plan, Box::new(paid("GG",Move::Cast { card:bear, targets:vec![] })))).unwrap();
    cite!(["701.61a", "601.2h"]; assert!(!g.battlefield.contains(&food) && g.stack.iter().any(|o| g.obj(*o).card == Some(BEAR))));
    // Neither forage resource exists, so the authored Food payment fails.
    let mut g = game();
    let forager = g.put(0, FORAGER, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Hand);
    let plan = vec![Step { plan: vec![], x: None, life: vec![], with: None, source:forager, ability:1, paying:vec![vec![]], adding:vec![vec![ir::mana::ManaSymbol::Color(ir::Color::Green);2]], chose:vec![1], cost_branches:None }];
    cite!(["601.2g", "701.61a"]; refused("which asks for exactly 1"): act_unchanged(&mut g, Move::WithMana(plan, Box::new(paid("GG",Move::Cast { card:bear, targets:vec![] })))));
}

#[test]
fn r601_2b_each_forage_alternative_pays_the_announced_resource() {
    // a Food and three cards in the graveyard: which forage pays is the player's choice
    // (601.2b); each authored payment must consume exactly its selected resources.
    let setup = || {
        let mut g = game();
        let forager = g.put(0, FORAGER, Zone::Battlefield);
        let food = g.put(0, FOOD, Zone::Battlefield);
        let cards: Vec<_> = (0..3).map(|_| g.put(0, WASTES, Zone::Graveyard)).collect();
        let bear = g.put(0, BEAR, Zone::Hand);
        (g, forager, food, cards, bear)
    };
    for way in [0, 1] {
        let (mut g, forager, food, cards, bear) = setup();
        let plan = vec![Step { plan: vec![], x: None, life: vec![], with: None, source:forager, ability:1, paying:vec![if way==0 {cards.clone()} else {vec![food]}], adding:vec![vec![ir::mana::ManaSymbol::Color(ir::Color::Green);2]], chose:vec![way], cost_branches:None }];
        g.act(0, Move::WithMana(plan, Box::new(paid("GG",Move::Cast { card: bear, targets: vec![] })))).unwrap();
        let (sacrificed, exiled) = (!g.battlefield.contains(&food), cards.iter().all(|c| g.obj(*c).zone != Zone::Graveyard));
        cite!(["701.61a", "601.2h"]; case: way; assert_eq!((sacrificed, exiled), (way == 1, way == 0)));
    }
}

#[test]
fn r605_3a_a_step_saying_no_alternative_of_a_choice_cost_is_refused() {
    let mut g = game();
    let forager = g.put(0, FORAGER, Zone::Battlefield);
    g.put(0, FOOD, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Hand);
    let green = vec![ir::mana::ManaSymbol::Color(ir::Color::Green); 2];
    let step = game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, source: forager, ability: 1, paying: vec![], adding: vec![green], chose: vec![] };
    let refused = g.act(0, Move::WithMana(vec![step], Box::new(paid("GG",Move::Cast { card: bear, targets: vec![] })))).unwrap_err();
    cite!(["601.2b", "605.3a"]; assert!(refused.contains("choice(s) among alternatives"), "{refused}"));
}

/// A rejected proposal preserves the whole game; the caller asserts its specific reason.
#[track_caller]
fn act_unchanged(g: &mut Game, action: Move) -> Result<(), String> {
    let before = format!("{g:?}");
    let result = g.act(0, action);
    assert_eq!(format!("{g:?}"), before);
    result
}
