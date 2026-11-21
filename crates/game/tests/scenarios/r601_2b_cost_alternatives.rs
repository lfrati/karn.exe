//! A cost that is a choice among alternatives: "discard a card or pay 3 life" (601.2b,
//! 602.2b). The alternative is chosen as the spell is cast or the ability activated, an
//! alternative that can't be paid is not offered, and none payable is no cast (601.2h);
//! a copy of the spell pays nothing (707.10). Reveal from the hand (701.20a), "behold and
//! exile it" (701.4a), forage (701.61a) and "remove counters and sacrifice it" (118.1) as the
//! parts of such costs.
use crate::support::{cite,paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const TRIUMPH: usize = 0;
const ISLAND: usize = 1;
const FORK: usize = 2;
const TRADE: usize = 3;
const SCOUT: usize = 4;
const GOBLIN: usize = 5;
const CHAMPION: usize = 6;
const CYCLE: usize = 7;
const FOOD: usize = 8;
const QUEST: usize = 9;

fn game() -> Game {
    let c = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    let defs = vec![
        c("Triumph", "{0}", "Instant", "As an additional cost to cast this spell, discard a card or pay 3 life.\nYou gain 1 life.", None),
        c("Island", "", "Basic Land — Island", "", None),
        c("Fork", "{0}", "Instant", "Copy target instant or sorcery spell. You may choose new targets for the copy.", None),
        c("Trading Post", "{0}", "Artifact", "Discard a card or pay 2 life: You gain 5 life.", None),
        c("Goblin Scout", "{0}", "Sorcery", "As an additional cost to cast this spell, reveal a Goblin card from your hand or pay {1}.\nYou gain 1 life.", None),
        c("Goblin", "{0}", "Creature — Goblin", "", Some((1, 1))),
        c("Champion", "{0}", "Creature — Goblin", "As an additional cost to cast this spell, behold a Goblin and exile it.\nWhen this creature leaves the battlefield, return the exiled card to its owner's hand.", Some((3, 3))),
        c("Feed", "{0}", "Instant", "As an additional cost to cast this spell, forage or pay {B}.\nYou gain 1 life.", None),
        c("Food", "", "Artifact — Food", "{2}, {T}, Sacrifice this artifact: You gain 3 life.", None),
        c("Quest", "{0}", "Enchantment", "Remove three quest counters from this enchantment and sacrifice it: You gain 5 life.", None),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..8 { g.put(p, ISLAND, Zone::Library); } }
    g
}
fn chose(alternative: usize, paying: Vec<Vec<ObjId>>, mv: Move) -> Move {
    let mv = if paying.is_empty() { mv } else { Move::WithCosts(paying, Box::new(mv)) };
    Move::WithCostChoice(vec![alternative], Box::new(mv))
}
fn cast(card: ObjId) -> Move { Move::Cast { card, targets: vec![] } }
fn resolve(g: &mut Game) {
    for _ in 0..40 {
        match &g.decision {
            Some(Decision::Choose { player, choice: Choice::Retarget { .. } }) => { let p = *player; g.act(p, Move::Choose(Answer::No)).unwrap(); }
            _ if g.stack.is_empty() => return,
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("the stack did not empty");
}

#[test]
fn r601_2b_either_alternative_is_chosen_and_paid_as_the_spell_is_cast() {
    let mut g = game();
    let card = g.put(0, TRIUMPH, Zone::Hand);
    let island = g.put(0, ISLAND, Zone::Hand);
    // the discard
    g.act(0, chose(0, vec![vec![island]], cast(card))).unwrap();
    cite!(["601.2h"]; case: "discard"; assert_eq!((g.players[0].hand.contains(&island), g.players[0].graveyard.len(), g.players[0].life), (false, 1, 20)));
    resolve(&mut g);
    cite!(["608.2"]; case: "discard"; assert_eq!(g.players[0].life, 21));

    // the life
    let mut g = game();
    let card = g.put(0, TRIUMPH, Zone::Hand);
    let island = g.put(0, ISLAND, Zone::Hand);
    g.act(0, chose(1, vec![], cast(card))).unwrap();
    cite!(["601.2h", "119.4"]; case: "life"; assert_eq!((g.obj(island).zone, g.players[0].life), (Zone::Hand, 17)));
    resolve(&mut g);
    cite!(["608.2"]; case: "life"; assert_eq!(g.players[0].life, 18));

    // which is paid is said: unsaid, or one that is not there, is no cast
    let mut g = game();
    let card = g.put(0, TRIUMPH, Zone::Hand);
    g.put(0, ISLAND, Zone::Hand);
    cite!(["601.2b"]; refused("that cost has 1 choice(s) among alternatives: say which alternative of each is paid or applied (601.2b, 118.7e)"): g.act(0, cast(card)));
    cite!(["601.2b"]; refused("that cost is a choice among 2 alternatives, numbered from 0 (601.2b)"): g.act(0, chose(2, vec![], cast(card))));
    cite!(["601.2b"]; case: "said once"; refused("say which alternatives of the cost are paid only once"): act_unchanged(&mut g, Move::WithCostChoice(vec![1], Box::new(chose(1, vec![], cast(card))))));
    assert_eq!((g.obj(card).zone, g.players[0].life), (Zone::Hand, 20));
}

#[test]
fn r601_2h_unpayable_alternatives_are_refused_and_neither_payable_is_no_cast() {
    // no card to discard: only the life
    let mut g = game();
    let card = g.put(0, TRIUMPH, Zone::Hand);
    cite!(["601.2h", "118.3"]; case: "no card"; refused("which asks for exactly 1"): act_unchanged(&mut g, chose(0, vec![vec![]], cast(card))));
    assert_eq!(g.obj(card).zone, Zone::Hand, "a refused cast is undone (733.1)");
    g.act(0, chose(1, vec![], cast(card))).unwrap();
    assert_eq!(g.players[0].life, 17);

    // too little life: only the discard (119.4)
    let mut g = game();
    let card = g.put(0, TRIUMPH, Zone::Hand);
    g.put(0, ISLAND, Zone::Hand);
    g.players[0].life = 2;
    cite!(["119.4"]; case: "2 life"; refused("you can't pay 3 life with a life total of 2"): act_unchanged(&mut g, chose(1, vec![], cast(card))));

    // neither: the spell can't be cast
    let mut g = game();
    let card = g.put(0, TRIUMPH, Zone::Hand);
    g.players[0].life = 2;
    for (alternative, reason) in [(0, "which asks for exactly 1"), (1, "you can't pay 3 life with a life total of 2")] {
        cite!(["118.3", "119.4"]; refused(reason): act_unchanged(&mut g, chose(alternative, vec![], cast(card))));
    }
    assert_eq!((g.obj(card).zone, g.players[0].life), (Zone::Hand, 2));
}

#[test]
fn r707_10_a_copy_of_the_spell_pays_no_alternative() {
    let mut g = game();
    let card = g.put(0, TRIUMPH, Zone::Hand);
    let island = g.put(0, ISLAND, Zone::Hand);
    g.act(0, chose(1, vec![], cast(card))).unwrap();
    let spell = *g.stack.last().unwrap();
    let fork = g.put(0, FORK, Zone::Hand);
    g.act(0, Move::Cast { card: fork, targets: vec![vec![Target::Object(spell)]] }).unwrap();
    resolve(&mut g);
    // 3 life paid once; the copy and the spell each gain 1; no card discarded for the copy
    cite!(["707.10", "601.2b"]; assert_eq!((g.players[0].life, g.obj(island).zone), (19, Zone::Hand)));
}

#[test]
fn r602_2b_an_activation_cost_alternative_is_chosen_as_it_is_activated() {
    let activate = |post| Move::Activate { source: post, ability: 0, targets: vec![], paying: vec![] };
    let mut g = game();
    let post = g.put(0, TRADE, Zone::Battlefield);
    let island = g.put(0, ISLAND, Zone::Hand);
    g.act(0, Move::WithCostChoice(vec![0], Box::new(Move::Activate { source: post, ability: 0, targets: vec![], paying: vec![vec![island]] }))).unwrap();
    cite!(["602.2b", "601.2h"]; case: "discard"; assert_eq!((g.players[0].hand.contains(&island), g.players[0].graveyard.len(), g.players[0].life), (false, 1, 20)));
    resolve(&mut g);
    assert_eq!(g.players[0].life, 25);
    // the hand empty now: only the life
    g.act(0, Move::WithCostChoice(vec![1], Box::new(activate(post)))).unwrap();
    cite!(["602.2b", "119.4"]; case: "life"; assert_eq!(g.players[0].life, 23));
    resolve(&mut g);
    assert_eq!(g.players[0].life, 28);
    // unsaid, it is not activated
    cite!(["602.2b"]; case: "unsaid"; refused("that cost has 1 choice(s) among alternatives"): act_unchanged(&mut g, activate(post)));
}

#[test]
fn r701_20a_a_card_revealed_from_the_hand_stays_there() {
    let mut g = game();
    let card = g.put(0, SCOUT, Zone::Hand);
    let goblin = g.put(0, GOBLIN, Zone::Hand);
    g.put(0, ISLAND, Zone::Hand);
    // the mana alternative needs {1}: only the reveal, then both with mana
    g.act(0, chose(0, vec![vec![goblin]], cast(card))).unwrap();
    cite!(["701.20a"]; assert_eq!(g.obj(goblin).zone, Zone::Hand));
    resolve(&mut g);
    assert_eq!(g.players[0].life, 21);

    let mut g = game();
    let card = g.put(0, SCOUT, Zone::Hand);
    let island = g.put(0, ISLAND, Zone::Hand);
    g.players[0].pool.colorless = 1;
    cite!(["701.20a"]; case: "not a Goblin"; refused("Island is not what that cost asks for"): act_unchanged(&mut g, chose(0, vec![vec![island]], cast(card))));
    g.act(0, paid("C",chose(1, vec![], cast(card)))).unwrap();
    assert_eq!(g.players[0].pool.colorless, 0);
}

#[test]
fn r701_4a_the_goblin_beheld_is_the_one_exiled_and_returns_with_the_champion() {
    for from in [Zone::Hand, Zone::Battlefield] {
        let mut g = game();
        let card = g.put(0, CHAMPION, Zone::Hand);
        let goblin = g.put(0, GOBLIN, from);
        g.act(0, Move::WithCosts(vec![vec![goblin]], Box::new(cast(card)))).unwrap();
        let exiled = *g.exile.iter().find(|o| g.obj(**o).card == Some(GOBLIN)).expect("the Goblin is exiled");
        cite!(["701.4a", "118.3"]; case: format!("{from:?}"); assert!(!g.players[0].hand.contains(&goblin) && !g.battlefield.contains(&goblin)));
        resolve(&mut g);
        let champion = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(CHAMPION)).unwrap();
        // the champion leaves: the exiled card returns to its owner's hand (607.2a)
        g.destroy(champion);
        g.act(g.priority, Move::Pass).unwrap();
        resolve(&mut g);
        cite!(["607.2a"]; case: format!("{from:?}"); assert!(g.players[0].hand.iter().any(|o| g.obj(*o).card == Some(GOBLIN)) && !g.exile.contains(&exiled)));
    }
    // no Goblin: no cast (the champion itself is on the stack, 601.2a)
    let mut g = game();
    let card = g.put(0, CHAMPION, Zone::Hand);
    cite!(["701.4a", "601.2h"]; case: "no Goblin"; refused("which asks for exactly 1"): act_unchanged(&mut g, Move::WithCosts(vec![vec![]], Box::new(cast(card)))));
}

#[test]
fn r701_61a_forage_is_either_of_its_ways_or_the_mana() {
    let mut g = game();
    let card = g.put(0, CYCLE, Zone::Hand);
    let food = g.put(0, FOOD, Zone::Battlefield);
    let buried: Vec<_> = (0..3).map(|_| g.put(0, ISLAND, Zone::Graveyard)).collect();
    g.players[0].pool.colors[2] = 1;
    // exile three cards from your graveyard, sacrifice a Food, or pay {B}
    let audit=g.audit_game_objects();
    let rows=audit["objects"][card.0.to_string()]["cost_branch_choices"][0].as_array().unwrap();
    cite!(["601.2b","701.61a"]; assert_eq!(rows.iter().map(|row|row["branch"].clone()).collect::<Vec<_>>(),vec![
        serde_json::json!({"kind":"printed_or","arm":1,"forage":"exile_three"}),
        serde_json::json!({"kind":"printed_or","arm":1,"forage":"sacrifice_food"}),
        serde_json::json!({"kind":"printed_or","arm":2}),
    ]));
    let selected:game::BranchSelection=serde_json::from_value(rows[1].clone()).unwrap();
    g.act(0,Move::WithCostBranches(vec![selected],Box::new(Move::WithCosts(vec![vec![food]],Box::new(cast(card)))))).unwrap();
    cite!(["701.61a"]; case: "food"; assert!(!g.battlefield.contains(&food) && buried.iter().all(|o| g.obj(*o).zone == Zone::Graveyard)));
    assert_eq!(g.players[0].pool.colors[2], 1);

    let mut g = game();
    let card = g.put(0, CYCLE, Zone::Hand);
    let buried: Vec<_> = (0..3).map(|_| g.put(0, ISLAND, Zone::Graveyard)).collect();
    g.act(0, chose(0, vec![buried.clone()], cast(card))).unwrap();
    cite!(["701.61a"]; case: "graveyard"; assert!(g.players[0].graveyard.is_empty()));
}

#[test]
fn r118_1_counters_removed_and_then_it_sacrificed() {
    let mut g = game();
    let quest = g.put(0, QUEST, Zone::Battlefield);
    let activate = Move::Activate { source: quest, ability: 0, targets: vec![], paying: vec![] };
    g.put_counters(quest, "quest", 2);
    cite!(["118.3"]; case: "two counters"; refused("Quest can't have 3 quest counters removed to pay this cost"): act_unchanged(&mut g, activate.clone()));
    g.put_counters(quest, "quest", 1);
    g.act(0, activate).unwrap();
    cite!(["118.1", "602.2b"]; assert!(!g.battlefield.contains(&quest)));
    resolve(&mut g);
    assert_eq!(g.players[0].life, 25);
}

/// A rejected proposal preserves the whole game; the caller asserts its specific reason.
#[track_caller]
fn act_unchanged(g: &mut Game, action: Move) -> Result<(), String> {
    let before = format!("{g:?}");
    let result = g.act(0, action);
    assert_eq!(format!("{g:?}"), before);
    result
}
