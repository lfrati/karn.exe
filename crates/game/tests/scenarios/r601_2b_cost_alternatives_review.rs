//! Costs that are a choice among alternatives, with the pool's cards.
//!
//! 601.2b: "If the spell has alternative or additional costs that will be paid as it's being
//! cast such as buyback or kicker costs (see rules 118.8 and 118.9), the player announces
//! their intentions to pay any or all of those costs (see rule 601.2f)."
//!
//! 601.2h: "The player pays the total cost. First, they pay all costs that don't involve
//! random elements or moving objects from the library to a public zone, in any order. Then
//! they pay all remaining costs in any order. Partial payments are not allowed. Unpayable
//! costs can't be paid."
//!
//! 119.4: "If a cost or effect allows a player to pay an amount of life greater than 0, the
//! player may do so only if their life total is greater than or equal to the amount of the
//! payment."
//!
//! 701.4a: "To behold a [quality] means to choose a permanent you control with that quality
//! or reveal a card with that quality from your hand." 701.61a: "To forage means to exile
//! three cards from your graveyard or sacrifice a Food."
use crate::support::{cite,paid};
use game::legal::Step;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, Target, Zone};
use ir::Part;

const NAMES: [&str; 7] = ["Redirect Lightning", "Thornvault Forager", "Bloodthorn Flail", "Champion of the Clachan", "Bullseye, Death Dealer", "Deadly Precision", "Grimoire of the Dead"];
const REDIRECT: usize = 0;
const FORAGER: usize = 1;
const FLAIL: usize = 2;
const CLACHAN: usize = 3;
const BULLSEYE: usize = 4;
const PRECISION: usize = 5;
const GRIMOIRE: usize = 6;
const WASTES: usize = 7;
const PAINLAND: usize = 8;
const BEAR: usize = 9;
const KITHKIN: usize = 10;
const FOOD: usize = 11;
const SHOCK: usize = 12;
const RELIC: usize = 13;
const PROTOCOL: usize = 14;
const ROCK: usize = 15;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let mut defs: Vec<CardDef> = NAMES.iter().map(|n| CardDef::from_faces(&catalog[*n]).unwrap()).collect();
    let c = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    defs.extend([
        c("Wastes", "", "Basic Land", "{T}: Add {C}.", None),
        c("Pain Spring", "", "Land", "{T}, Pay 1 life: Add {R}.", None),
        c("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        c("Kithkin", "{W}", "Creature — Kithkin", "", Some((1, 1))),
        c("Food", "", "Artifact — Food", "{2}, {T}, Sacrifice this artifact: You gain 3 life.", None),
        c("Shock", "{R}", "Instant", "Shock deals 2 damage to any target.", None),
        c("Relic", "{1}", "Artifact", "", None),
        c("Protocol", "{U}", "Instant", "As an additional cost to cast this spell, tap an untapped artifact you control or pay {1}.\nYou gain 1 life.", None),
        c("Rock", "{1}", "Artifact", "{T}: Add {U}.", None),
    ]);
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..8 { g.put(p, WASTES, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
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
fn r119_4_redirect_lightning_pays_5_life_only_at_5_life_or_more() {
    // Redirect Lightning {R}: "As an additional cost to cast this spell, pay 5 life or pay
    // {2}.", "Change the target of target spell or ability with a single target."
    for (life, mana, offered) in [(5, 1, vec![vec![0]]), (4, 1, vec![]), (4, 3, vec![vec![1]]), (5, 3, vec![vec![0], vec![1]])] {
        let mut g = game();
        let shock = g.put(1, SHOCK, Zone::Hand);
        let bear = g.put(0, BEAR, Zone::Battlefield);
        g.put(0, BEAR, Zone::Battlefield);
        g.players[1].pool.colors[3] = 1;
        g.act(0, Move::Pass).unwrap();
        g.act(1, paid("R",Move::Cast { card: shock, targets: vec![vec![Target::Object(bear)]] })).unwrap();
        let shock=*g.stack.last().unwrap();
        g.act(1, Move::Pass).unwrap();
        let card = g.put(0, REDIRECT, Zone::Hand);
        g.players[0].life = life;
        g.players[0].pool.colors[3] = 1;
        g.players[0].pool.colorless = mana - 1;
        for arm in [0,1] {
            let action=Move::WithCostChoice(vec![arm],Box::new(paid(if arm==0{"R"}else{"CCR"},Move::Cast{card,targets:vec![vec![Target::Object(shock)]]})));
            cite!(["119.4","601.2b"]; case: format!("{life} life, {mana} mana, arm {arm}"); assert_eq!(g.clone().act(0,action).is_ok(),offered.contains(&vec![arm])));
        }
    }
}

#[test]
fn r601_2h_life_paid_for_the_mana_leaves_too_little_for_the_life_alternative() {
    // Redirect Lightning {R} at 5 life, its only red mana a land that costs 1 life: the mana
    // abilities are activated before the costs are paid (601.2g, 601.2h), so 4 life remain
    // for the 5 life alternative, and {2} can't be paid either. The
    // move with the life chosen is refused leaving nothing paid.
    let mut g = game();
    let shock = g.put(1, SHOCK, Zone::Hand);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    g.players[1].pool.colors[3] = 1;
    g.act(0, Move::Pass).unwrap();
    g.act(1, paid("R",Move::Cast { card: shock, targets: vec![vec![Target::Object(bear)]] })).unwrap();
        let shock=*g.stack.last().unwrap();
    g.act(1, Move::Pass).unwrap();
    let card = g.put(0, REDIRECT, Zone::Hand);
    let spring = g.put(0, PAINLAND, Zone::Battlefield);
    g.players[0].life = 5;
    let mv = Move::WithCostChoice(vec![0], Box::new(Move::WithMana(vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source: spring, ability: 0, paying: vec![], adding: vec![] }],
        Box::new(paid("R",Move::Cast { card, targets: vec![vec![Target::Object(shock)]] })))));
    cite!(["601.2h", "119.4"]; refused("you can't pay 5 life with a life total of 4"): act_unchanged(&mut g, mv.clone()));
    // at 6 life it is paid: 1 for the mana, then 5
    g.players[0].life = 6;
    cite!(["601.2b", "119.4"]; ok: g.act(0,mv));
    assert_eq!(g.players[0].life,0);
}

#[test]
fn r602_2b_bloodthorn_flail_equips_for_three_or_a_discard() {
    // Bloodthorn Flail: "Equipped creature gets +2/+1.", "Equip—Pay {3} or discard a card."
    let mut g = game();
    let flail = g.put(0, FLAIL, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let card = g.put(0, WASTES, Zone::Hand);
    let equip=2; // Two printed abilities precede the derived Equip activation.
    cite!(["602.2b","702.6a"]; refused("your mana pool cannot pay that cost"): act_unchanged(&mut g, Move::WithCostChoice(vec![0],Box::new(Move::Activate{source:flail,ability:equip,targets:vec![vec![Target::Object(bear)]],paying:vec![]}))));
    let audit=g.audit_game_objects();
    let selected=audit["objects"][flail.0.to_string()]["cost_branch_choices"][0][1].clone();
    cite!(["602.2b","702.6a"]; assert_eq!(selected["branch"],serde_json::json!({"kind":"printed_or","arm":2})));
    assert_eq!(selected["ability"]["paragraph"],2);
    assert_eq!(selected["ability"]["keyword"],"Equip");
    let selected:game::BranchSelection=serde_json::from_value(selected).unwrap();
    g.act(0, Move::WithCostBranches(vec![selected], Box::new(Move::Activate { source: flail, ability: equip, targets: vec![vec![Target::Object(bear)]], paying: vec![vec![card]] }))).unwrap();
    resolve(&mut g);
    cite!(["602.2b", "702.6a"]; assert_eq!((g.obj(card).zone == Zone::Hand, g.power(bear), g.toughness(bear)), (false, Some(4), Some(3))));
}

#[test]
fn r701_61a_thornvault_forager_forages_for_mana() {
    // Thornvault Forager: "{T}: Add {G}.", "{T}, Forage: Add two mana in any combination of
    // colors." (701.61a: exile three cards from your graveyard or sacrifice a Food.)
    let mut g = game();
    let forager = g.put(0, FORAGER, Zone::Battlefield);
    let food = g.put(0, FOOD, Zone::Battlefield);
    let two: Vec<_> = (0..2).map(|_| g.put(0, WASTES, Zone::Graveyard)).collect();
    let forage=|cards|Move::WithCostChoice(vec![0],Box::new(Move::Adding(vec![vec![ir::mana::ManaSymbol::Color(ir::Color::Green);2]],Box::new(Move::Activate{source:forager,ability:1,targets:vec![],paying:vec![cards]}))));
    cite!(["701.61a","605.1a"]; refused("which asks for exactly 3"): act_unchanged(&mut g, forage(two.clone())));
    let third = g.put(0, WASTES, Zone::Graveyard);
    cite!(["701.61a","605.1a"]; ok: g.clone().act(0,forage(vec![two[0],two[1],third])));
    g.act(0, Move::WithCostChoice(vec![1], Box::new(Move::Adding(vec![vec![ir::mana::ManaSymbol::Color(ir::Color::Red), ir::mana::ManaSymbol::Color(ir::Color::Green)]],
        Box::new(Move::Activate { source: forager, ability: 1, targets: vec![], paying: vec![vec![food]] }))))).unwrap();
    cite!(["701.61a", "605.3b"]; assert_eq!((g.battlefield.contains(&food), g.players[0].pool.colors[4], g.players[0].pool.colors[3], g.obj(forager).tapped), (false, 1, 1, true)));
    cite!(["701.61a"]; assert!(two.iter().chain([&third]).all(|o| g.obj(*o).zone == Zone::Graveyard)));
}

#[test]
fn r601_2g_a_forage_mana_ability_pays_for_a_spell() {
    // A Bear {1}{G} with Thornvault Forager as the only mana: its forage ability adds the two
    // mana, a mana ability activated while the spell is cast (601.2g, 605.3a).
    let mut g = game();
    let forager=g.put(0, FORAGER, Zone::Battlefield);
    let food=g.put(0, FOOD, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Hand);
    let plan=vec![Step { plan: vec![], x: None, life: vec![], with: None,source:forager,ability:1,paying:vec![vec![food]],adding:vec![vec![ir::mana::ManaSymbol::Color(ir::Color::Green);2]],chose:vec![1],cost_branches:None}];
    cite!(["601.2g","605.3a","701.61a"]; ok: g.act(0,Move::WithMana(plan,Box::new(paid("GG",Move::Cast{card:bear,targets:vec![]})))));
}

#[test]
fn r605_3a_the_forage_mana_activated_first_pays_for_a_spell() {
    // the same, the mana ability activated with priority first (605.3a): the Bear is cast
    let mut g = game();
    let forager = g.put(0, FORAGER, Zone::Battlefield);
    let food = g.put(0, FOOD, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Hand);
    g.act(0, Move::WithCostChoice(vec![1], Box::new(Move::Adding(vec![vec![ir::mana::ManaSymbol::Color(ir::Color::Green), ir::mana::ManaSymbol::Color(ir::Color::Green)]],
        Box::new(Move::Activate { source: forager, ability: 1, targets: vec![], paying: vec![vec![food]] }))))).unwrap();
    g.act(0, paid("GG",Move::Cast { card: bear, targets: vec![] })).unwrap();
    resolve(&mut g);
    cite!(["605.3a"]; assert!(g.battlefield.iter().any(|o| g.obj(*o).card == Some(BEAR))));
}

#[test]
fn r701_4a_a_token_beheld_and_exiled_returns_nothing() {
    // Champion of the Clachan {3}{W}: "Flash", "As an additional cost to cast this spell,
    // behold a Kithkin and exile it.", "Other Kithkin you control get +1/+1.", "When this
    // creature leaves the battlefield, return the exiled card to its owner's hand." A token
    // exiled ceases to exist (111.7, 704.5d): there is no exiled card to return.
    let mut g = game();
    let card = g.put(0, CLACHAN, Zone::Hand);
    let model = g.put(0, KITHKIN, Zone::Graveyard);
    let token = g.create_token(0, g.obj(model).chars.clone(), false);
    let hand = g.players[0].hand.len();
    g.players[0].pool.colors[0] = 1;
    g.players[0].pool.colorless = 3;
    g.act(0, Move::WithCosts(vec![vec![token]], Box::new(paid("CCCW",Move::Cast { card, targets: vec![] })))).unwrap();
    cite!(["701.4a", "111.7"]; assert!(!g.battlefield.contains(&token) && !g.exile.iter().any(|o| g.obj(*o).token())));
    resolve(&mut g);
    let champion = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(CLACHAN)).unwrap();
    g.destroy(champion);
    g.act(g.priority, Move::Pass).unwrap();
    resolve(&mut g);
    cite!(["607.2a", "111.7"]; assert_eq!(g.players[0].hand.len(), hand - 1, "nothing returns"));
}

#[test]
fn r603_12_bullseye_sacrifices_an_artifact_or_discards_a_nonland_card() {
    // Bullseye, Death Dealer: "When Bullseye enters, you may sacrifice an artifact or discard a
    // nonland card. When you do, Bullseye deals 2 damage to any target."
    let mut g = game();
    let card = g.put(0, BULLSEYE, Zone::Hand);
    g.put(0, RELIC, Zone::Battlefield);
    g.put(0, BEAR, Zone::Hand);
    g.put(0, WASTES, Zone::Hand);
    g.players[0].pool.colorless = 10; g.players[0].pool.colors = [2; 5];
    g.act(0, paid("CCB",Move::Cast { card, targets: vec![] })).unwrap();
    while g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
    // a nonland card, not the Wastes; the Relic is the artifact
    let Some(Decision::Choose { player: 0, choice: Choice::Alternative { among, optional: true } }) = &g.decision else { panic!("{:?}", g.decision) };
    cite!(["608.2d"]; assert_eq!(among.len(), 2));
    g.act(0, Move::Choose(Answer::Alternative(1))).unwrap();
    cite!(["603.12"]; assert!(matches!(g.decision, Some(Decision::Targets { player: 0, .. }))));
    g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap();
    resolve(&mut g);
    cite!(["603.12", "701.9a"]; assert_eq!((g.players[1].life, g.players[0].hand.iter().filter(|o| g.obj(**o).card == Some(BEAR)).count(), g.battlefield.iter().any(|o| g.obj(*o).card == Some(RELIC))), (18, 0, true)));
}

#[test]
fn r601_2g_an_artifact_tapped_for_mana_cant_also_be_tapped_for_the_cost() {
    // Disruption Protocol's "tap an untapped artifact you control or pay {1}": the Rock is the
    // only blue mana, so tapping it for mana (601.2g) leaves no untapped artifact to tap for
    // the cost (601.2h); with the Wastes, {1} is payable. Only "pay {1}" is offered.
    let mut g = game();
    let card = g.put(0, PROTOCOL, Zone::Hand);
    let rock = g.put(0, ROCK, Zone::Battlefield);
    let wastes = g.put(0, WASTES, Zone::Battlefield);
    let step = |source| game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source, ability: 0, paying: vec![], adding: vec![] };
    let action = Move::WithCostChoice(vec![0], Box::new(Move::WithCosts(vec![vec![rock]], Box::new(Move::WithMana(vec![step(rock)], Box::new(paid("U",Move::Cast { card, targets: vec![] })))))));
    cite!(["601.2h", "118.3"]; refused("Rock is not what that cost asks for"): act_unchanged(&mut g, action));
    g.act(0, Move::WithCostChoice(vec![1], Box::new(Move::WithMana(vec![step(rock), step(wastes)], Box::new(paid("CU",Move::Cast { card, targets: vec![] })))))).unwrap();
    resolve(&mut g);
    cite!(["601.2b"]; assert_eq!(g.players[0].life, 21));
    // with a second artifact, tapping it for the cost and the Rock for {U}
    let mut g = game();
    let card = g.put(0, PROTOCOL, Zone::Hand);
    let rock = g.put(0, ROCK, Zone::Battlefield);
    let relic = g.put(0, RELIC, Zone::Battlefield);
    g.act(0, Move::WithCostChoice(vec![0], Box::new(Move::WithCosts(vec![vec![relic]], Box::new(Move::WithMana(vec![step(rock)], Box::new(paid("U",Move::Cast { card, targets: vec![] })))))))).unwrap();
    cite!(["601.2h"]; case: "a Relic"; assert!(g.obj(relic).tapped && g.obj(rock).tapped));
}

#[test]
fn r118_1_it_is_the_grimoire_named_in_the_cost() {
    // Grimoire of the Dead: "{T}, Remove three study counters from Grimoire of the Dead and
    // sacrifice it: Put all creature cards from all graveyards onto the battlefield under
    // your control." "It" is the Grimoire; with two counters nothing is paid (118.3).
    let mut g = game();
    let grimoire = g.put(0, GRIMOIRE, Zone::Battlefield);
    let bear = g.put(1, BEAR, Zone::Graveyard);
    let ability = g.def(grimoire).unwrap().abilities().len() - 1;
    let activate = Move::Activate { source: grimoire, ability, targets: vec![], paying: vec![] };
    g.put_counters(grimoire, "study", 2);
    cite!(["118.3"]; case: "two counters"; refused("Grimoire of the Dead can't have 3 study counters removed to pay this cost"): act_unchanged(&mut g, activate.clone()));
    g.put_counters(grimoire, "study", 1);
    g.act(0, activate).unwrap();
    cite!(["118.1", "602.2b"]; assert!(!g.battlefield.contains(&grimoire)));
    resolve(&mut g);
    cite!(["118.1"]; assert!(g.battlefield.iter().any(|o| g.obj(*o).card == Some(BEAR) && g.obj(*o).controller == 0) && g.obj(bear).zone != Zone::Graveyard));
}

#[test]
fn r608_2b_the_creature_sacrificed_for_the_cost_was_the_target() {
    // Deadly Precision {B}: "As an additional cost to cast this spell, pay {4} or sacrifice an
    // artifact or creature.", "Destroy target creature." Targets are chosen (601.2c) before
    // the cost is paid (601.2h): the Bear targeted may be the one sacrificed, and the spell
    // then has no legal target as it would resolve (608.2b).
    let mut g = game();
    let card = g.put(0, PRECISION, Zone::Hand);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    g.players[0].pool.colors[2] = 1;
    g.act(0, Move::WithCostChoice(vec![1], Box::new(Move::WithCosts(vec![vec![bear]], Box::new(paid("B",Move::Cast { card, targets: vec![vec![Target::Object(bear)]] })))))).unwrap();
    cite!(["601.2c", "601.2h"]; assert!(!g.battlefield.contains(&bear)));
    resolve(&mut g);
    cite!(["608.2b"]; assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).card == Some(PRECISION))));
}

/// A rejected proposal preserves the whole game; the caller asserts its specific reason.
#[track_caller]
fn act_unchanged(g: &mut Game, action: Move) -> Result<(), String> {
    let before = format!("{g:?}");
    let result = g.act(0, action);
    assert_eq!(format!("{g:?}"), before);
    result
}
