//! "You may pay {B}. If you do, …" (118.12): mana paid while an effect is applied. The
//! payment is the one procedure a spell's is, as a child of the resolution that asked: it is
//! described to the player, who supplies payment instructions; no player has priority, and "if you do"
//! asks whether it was made.
use crate::support::{cite, mana_pool, paid};
use game::legal::{Asked, Step};
use game::state::{Answer, Choice, Decision, Pool};
use game::{CardDef, Event, Game, Move, ObjId, Zone};
use ir::mana::ManaSymbol;
use ir::Color;

const BOMB: usize = 0;
const SWAMP: usize = 1;
const FOREST: usize = 2;
const BEAR: usize = 3;
const GIFT: usize = 4;
const TOLL: usize = 5;
const ELF: usize = 6;
const SCALES: usize = 7;
const EVOLUTION: usize = 8;

const SPELLBOMB: &str = "{T}, Sacrifice this artifact: Exile target player's graveyard.\nWhen this artifact is put into a graveyard from the battlefield, you may pay {B}. If you do, draw a card.";

fn game() -> Game {
    let card = |name: &str, cost: &str, types: &str, text: &str, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    let defs = vec![
        card("Nihil Spellbomb", "{1}", "Artifact", SPELLBOMB, None),
        card("Swamp", "", "Basic Land — Swamp", "", None),
        card("Forest", "", "Basic Land — Forest", "", None),
        card("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Gift", "{G}", "Sorcery", "Target player may pay {B}. If they do, they draw a card.", None),
        card("Toll", "{G}", "Sorcery", "You may pay {W/B}. If you do, you gain 3 life.", None),
        card("Mana creature", "{0}", "Creature — Elf", "{T}: You gain 2 life. Put a +1/+1 counter on this creature. Add X mana of any one color, where X is the number of +1/+1 counters on this creature.", Some((1, 1))),
        card("Hardened Scales", "{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.", None),
        card("Branching Evolution", "{2}{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.", None),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    Game::new(defs, 2, 20)
}

fn pass_all(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn black(g: &mut Game, player: usize, n: u32) { g.players[player].pool.colors[2] += n; }
fn tap(land: ObjId) -> Step {  Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source: land, ability: 0, paying: vec![], adding: vec![] } }
fn draws(g: &Game, player: usize) -> usize {
    g.log.iter().filter(|e| matches!(e, Event::Drew { player: p, .. } if *p == player)).count()
}

/// The Spellbomb was sacrificed for its ability; its trigger resolves first and asks.
/// Returns the game at the question, with one Forest for player 0 to draw.
fn sacrificed(prepare: impl FnOnce(&mut Game)) -> Game {
    let mut g = game();
    g.put(0, FOREST, Zone::Library);
    g.put(1, BEAR, Zone::Graveyard);
    let bomb = g.put(0, BOMB, Zone::Battlefield);
    prepare(&mut g);
    g.act(0, Move::Activate { source: bomb, ability: 0, targets: vec![vec![game::Target::Player(1)]], paying: vec![] }).unwrap();
    assert_eq!(g.stack.len(), 2, "the ability, and above it the trigger of the sacrifice");
    pass_all(&mut g);
    g
}
fn asks_to_pay(g: &Game) {
    match &g.decision {
        Some(Decision::Choose { player: 0, choice: Choice::Pay { cost, life: 0, .. } }) => {
            assert_eq!(cost.0, vec![ManaSymbol::Color(Color::Black)]);
        }
        other => panic!("the payment is asked: {other:?}"),
    }
}

#[test]
fn r118_12_nihil_spellbomb_pays_from_the_pool_then_draws_and_exiles() {
    let mut g = sacrificed(|g| black(g, 0, 1));
    asks_to_pay(&g);
    assert!(matches!(g.asked(), Some((0, Asked::Pay { .. }))));
    // no player has priority while the effect is applied (117.1, 608.2)
    for player in 0..2 { crate::support::refused(act_unchanged(&mut g, player, Move::Pass), "a decision is pending: a choice of the effect being applied (608.2d)"); }
    cite!(["118.12a"]; refused("that decision is another player's"): act_unchanged(&mut g, 1, Move::Choose(Answer::Yes)));
    let hand = g.players[0].hand.len();
    let before = format!("{g:?}");
    // Omission supplies no mana even though B is present; it never chooses from the pool. The
    // pool could pay {B}, so the refusal is the choice not made (601.2h), not a shortfall.
    cite!(["118.3a"]; refused("no mana was chosen from your mana pool to pay that cost (118.3a): {B}"): act_unchanged(&mut g, 0, Move::Choose(Answer::Pay { plan: vec![], with: None, tapped: vec![] })));
    assert_eq!(format!("{g:?}"), before);
    g.act(0, Move::Choose(Answer::Pay { plan: vec![], with: Some(mana_pool("B")), tapped: vec![] })).unwrap();
    assert!(g.decision.is_none() && g.resolving().is_none());
    cite!(["118.3a"]; assert_eq!(g.players[0].pool.total(), 0));
    cite!(["118.12"]; assert_eq!((g.players[0].hand.len(), draws(&g, 0)), (hand + 1, 1)));
    assert_eq!(g.stack.len(), 1, "the activated ability is still to resolve");
    pass_all(&mut g);
    assert!(g.players[1].graveyard.is_empty(), "the whole graveyard is exiled");
}

#[test]
fn r118_12_declining_draws_nothing_and_keeps_the_mana() {
    let mut g = sacrificed(|g| black(g, 0, 1));
    asks_to_pay(&g);
    g.act(0, Move::Choose(Answer::No)).unwrap();
    assert!(g.decision.is_none() && g.resolving().is_none());
    cite!(["118.12"]; assert_eq!((g.players[0].pool.total(), draws(&g, 0)), (1, 0)));
    assert_eq!(g.stack.len(), 1);
}

#[test]
fn r118_12_an_unaffordable_supplied_payment_is_refused_and_can_be_declined() {
    let mut g = sacrificed(|g| { g.put(0, FOREST, Zone::Battlefield); });
    asks_to_pay(&g);
    let before = format!("{g:?}");
    cite!(["118.3"]; refused("that mana is not in your mana pool"): act_unchanged(&mut g, 0, Move::Choose(Answer::Pay { plan: vec![], with: Some(mana_pool("B")), tapped: vec![] })));
    assert_eq!(format!("{g:?}"), before);
    g.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["118.12"]; assert_eq!((draws(&g, 0), g.stack.len()), (0, 1)));
}

#[test]
fn r605_3a_mana_abilities_are_activated_to_pay_and_a_plan_can_be_said() {
    let mut swamps = Vec::new();
    let mut g = sacrificed(|g| { for _ in 0..2 { swamps.push(g.put(0, SWAMP, Zone::Battlefield)); } });
    asks_to_pay(&g);
    // a plan that does not pay changes nothing, and the question stands
    let before = g.clone();
    cite!(["118.3a"]; refused("your mana pool cannot pay that cost (118.3a): {B}"): act_unchanged(&mut g, 0, Move::Choose(Answer::Pay { plan: vec![], with: None, tapped: vec![] })));
    cite!(["118.3a"]; refused("that mana is not in your mana pool (118.3a)"): act_unchanged(&mut g, 0, Move::Choose(Answer::Pay { plan: vec![], with: Some(Pool { colors: [0, 0, 1, 0, 0], ..Pool::default() }), tapped: vec![] })));
    assert_eq!(format!("{g:?}"), format!("{before:?}"));
    // Each independently supplied Swamp can pay the cost.
    let other = swamps[1];
    let mut said = g.clone();
    said.act(0, Move::Choose(Answer::Pay { plan: vec![tap(other)], with: Some(mana_pool("B")), tapped: vec![] })).unwrap();
    cite!(["605.3a"]; assert!(said.obj(other).tapped && !said.obj(swamps[0]).tapped));
    g.act(0, Move::Choose(Answer::Pay { plan: vec![tap(swamps[0])], with: Some(mana_pool("B")), tapped: vec![] })).unwrap();
    cite!(["605.3a"]; assert!(g.obj(swamps[0]).tapped && !g.obj(other).tapped));
    for game in [&g, &said] {
        assert!(game.decision.is_none() && game.resolving().is_none());
        cite!(["118.12"]; assert_eq!((draws(game, 0), game.players[0].pool.total()), (1, 0)));
        assert_eq!(game.log.iter().filter(|e| matches!(e, Event::TappedForMana { .. })).count(), 1);
        // paying an effect announces nothing
        assert!(!game.log.iter().any(|e| matches!(e, Event::AnnouncementReversed { .. })));
    }
}

#[test]
fn r118_12_the_player_the_effect_names_pays_from_their_own_mana() {
    let mut g = game();
    g.put(1, FOREST, Zone::Library);
    let gift = g.put(0, GIFT, Zone::Hand);
    g.players[0].pool.colors[4] = 1;
    // the controller's black mana pays nothing for the opponent
    black(&mut g, 0, 1);
    let swamp = g.put(1, SWAMP, Zone::Battlefield);
    g.act(0, paid("G", Move::Cast { card: gift, targets: vec![vec![game::Target::Player(1)]] })).unwrap();
    pass_all(&mut g);
    assert!(matches!(g.decision, Some(Decision::Choose { player: 1, choice: Choice::Pay { .. } })), "the target is asked");
    cite!(["118.12a"]; refused("that decision is another player's"): act_unchanged(&mut g, 0, Move::Choose(Answer::Yes)));
    g.act(1, Move::Choose(Answer::Pay { plan: vec![tap(swamp)], with: Some(mana_pool("B")), tapped: vec![] })).unwrap();
    assert!(g.obj(swamp).tapped);
    cite!(["118.12"]; assert_eq!((draws(&g, 1), draws(&g, 0)), (1, 0)));
    cite!(["118.3a"]; assert_eq!(g.players[0].pool.colors[2], 1, "the controller's mana was not spent"));

    // The target cannot spend the controller's black mana; an authored B payment is unavailable.
    let mut g = game();
    g.put(1, FOREST, Zone::Library);
    let gift = g.put(0, GIFT, Zone::Hand);
    g.players[0].pool.colors[4] = 1;
    black(&mut g, 0, 1);
    g.act(0, paid("G", Move::Cast { card: gift, targets: vec![vec![game::Target::Player(1)]] })).unwrap();
    pass_all(&mut g);
    cite!(["118.3"]; refused("that mana is not in your mana pool"): act_unchanged(&mut g, 1, Move::Choose(Answer::Pay { plan: vec![], with: Some(mana_pool("B")), tapped: vec![] })));
    g.act(1, Move::Choose(Answer::No)).unwrap();
    assert!(g.decision.is_none() && g.resolving().is_none());
    assert_eq!(draws(&g, 1), 0);
}

#[test]
fn r107_4e_a_hybrid_payment_is_paid_with_either_half() {
    for (color, pays) in [(0, true), (2, true), (3, false)] {
        let mut g = game();
        let toll = g.put(0, TOLL, Zone::Hand);
        g.players[0].pool.colors[4] = 1;
        g.players[0].pool.colors[color] = 1;
        g.act(0, paid("G", Move::Cast { card: toll, targets: vec![] })).unwrap();
        pass_all(&mut g);
        if !pays {
            cite!(["107.4e"]; refused("the supplied mana does not pay that cost, all of it and no more"): act_unchanged(&mut g, 0, Move::Choose(Answer::Pay { plan: vec![], with: Some(mana_pool("R")), tapped: vec![] })));
            g.act(0, Move::Choose(Answer::No)).unwrap();
            assert_eq!(g.players[0].life, 20);
            continue;
        }
        cite!(["107.4e"]; case: format!("color {color}"); assert!(matches!(&g.decision, Some(Decision::Choose { player: 0, choice: Choice::Pay { cost, .. } }) if cost.0.len() == 1), "{:?}", g.decision));
        g.act(0, Move::Choose(Answer::Pay { plan: vec![], with: Some({ let mut pool = Pool::default(); pool.colors[color] = 1; pool }), tapped: vec![] })).unwrap();
        cite!(["107.4e", "118.12"]; case: format!("color {color}"); assert_eq!((g.players[0].life, g.players[0].pool.total()), (23, 0)));
    }
}

/// The Elf's mana ability asks which replacement applies first, so how much mana it makes
/// is known only inside the payment.
fn nested() -> (Game, ObjId, ObjId, ObjId) {
    let mut g = game();
    let elf = g.put(0, ELF, Zone::Battlefield);
    let scales = g.put(0, SCALES, Zone::Battlefield);
    let evolution = g.put(0, EVOLUTION, Zone::Battlefield);
    (g, elf, scales, evolution)
}
fn elf_step(elf: ObjId, n: usize) -> Step {  Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source: elf, ability: 0, paying: vec![], adding: vec![vec![ManaSymbol::Color(Color::Black); n]] } }
fn choose(g: &mut Game, source: ObjId) {
    let Some(Decision::Choose { choice: Choice::Replacement { among, .. }, .. }) = &g.decision else { panic!("replacement question: {:?}", g.decision) };
    let effect = game::operation::ReplacementId::from_ability(source, game::binding::AbilityKey::printed(0), 0);
    assert!(among.contains(&effect));
    g.act(0, Move::Choose(Answer::Replacement(effect))).unwrap();
}
fn toll_asked() -> (Game, ObjId, ObjId, ObjId) {
    let (mut g, elf, scales, evolution) = nested();
    let toll = g.put(0, TOLL, Zone::Hand);
    g.players[0].pool.colors[4] = 1;
    // What the Elf will make is not found by planning; one black of the pool is, and with
    // the question asked its player may say any payment.
    black(&mut g, 0, 1);
    g.act(0, paid("G", Move::Cast { card: toll, targets: vec![] })).unwrap();
    pass_all(&mut g);
    assert!(matches!(&g.decision, Some(Decision::Choose { player: 0, choice: Choice::Pay { .. } })), "{:?}", g.decision);
    (g, elf, scales, evolution)
}

#[test]
fn r605_3b_a_question_inside_the_payment_returns_to_it_once() {
    let (mut g, elf, scales, _) = toll_asked();
    g.act(0, Move::Choose(Answer::Pay { plan: vec![elf_step(elf, 3)], with: Some(mana_pool("B")), tapped: vec![] })).unwrap();
    // the mana ability is under way: its life is gained, its counters are not yet placed
    cite!(["605.3a", "605.3b"]; assert_eq!(g.players[0].life, 22));
    cite!(["616.1"]; assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::Replacement { .. }, .. }))));
    for player in 0..2 { crate::support::refused(act_unchanged(&mut g, player, Move::Pass), "a decision is pending: a choice of the effect being applied (608.2d)"); }
    // the suspended game, copied, goes on to the same end
    let mut copy = g.clone();
    for game in [&mut g, &mut copy] {
        choose(game, scales);
        // (1 + 1) * 2 counters: four mana where three were said
        assert!(matches!(game.decision, Some(Decision::Choose { choice: Choice::Mana { .. }, .. })), "{:?}", game.decision);
        game.act(0, Move::Choose(Answer::Mana(vec![ManaSymbol::Color(Color::Black); 4]))).unwrap();
        assert!(game.decision.is_none() && game.resolving().is_none());
        cite!(["118.12"]; assert_eq!(game.players[0].life, 25, "2 of the Elf, 3 of the Toll"));
        assert_eq!(game.players[0].pool.total(), 4, "one of the five paid");
        assert_eq!(game.log.iter().filter(|e| matches!(e, Event::TappedForMana { .. })).count(), 1);
        assert_eq!(game.log.iter().filter(|e| matches!(e, Event::LifeGained { .. })).count(), 2);
    }
    assert_eq!(format!("{g:?}"), format!("{copy:?}"));
}

/// A payment said with mana of the pool the ability then does not make.
fn short() -> (Game, ObjId) {
    let (mut g, elf, _, evolution) = toll_asked();
    let swamp = g.put(0, SWAMP, Zone::Battlefield);
    // five black of the pool are said to pay; with the three the Elf makes it holds four
    let with = Pool { colors: [0, 0, 5, 0, 0], ..Pool::default() };
    g.act(0, Move::Choose(Answer::Pay { plan: vec![elf_step(elf, 3)], with: Some(with), tapped: vec![] })).unwrap();
    choose(&mut g, evolution);
    assert!(matches!(g.decision, Some(Decision::Payment { player: 0, .. })), "{:?}", g.decision);
    assert!(matches!(g.asked(), Some((0, Asked::Payment { .. }))));
    assert_eq!(g.players[0].pool.total(), 4);
    crate::support::refused(act_unchanged(&mut g, 0, Move::Pass), "a decision is pending: complete the announced payment (601.2g-h)");
    crate::support::refused(act_unchanged(&mut g, 1, Move::Pass), "that decision is another player's");
    (g, swamp)
}

#[test]
fn r118_12_a_payment_that_fails_after_a_question_is_corrected() {
    let (mut g, swamp) = short();
    let before = g.clone();
    cite!(["118.3a"]; refused("that mana is not in your mana pool (118.3a)"): act_unchanged(&mut g, 0, Move::FinishPayment { plan: vec![], paying: vec![], with: Some(Pool { colors: [0, 0, 5, 0, 0], ..Pool::default() }) }));
    crate::support::refused(act_unchanged(&mut g, 1, Move::FinishPayment { plan: vec![], paying: vec![], with: None }), "that decision is another player's");
    assert_eq!(format!("{g:?}"), format!("{before:?}"));
    g.act(0, Move::FinishPayment { plan: vec![], paying: vec![], with: Some(mana_pool("B")) }).unwrap();
    assert!(g.decision.is_none() && g.resolving().is_none());
    cite!(["118.12"]; assert_eq!((g.players[0].life, g.players[0].pool.total()), (25, 3)));
    assert!(!g.obj(swamp).tapped);
    assert_eq!(g.log.iter().filter(|e| matches!(e, Event::TappedForMana { .. })).count(), 1, "the Elf's ability is not activated again");
}

#[test]
fn r733_1_abandoning_it_declines_and_keeps_the_mana_made() {
    let (mut g, _) = short();
    g.act(0, Move::ReversePayment).unwrap();
    assert!(g.decision.is_none() && g.resolving().is_none());
    cite!(["118.12"]; assert_eq!(g.players[0].life, 22, "the Elf's life, not the Toll's"));
    cite!(["733.1"]; assert_eq!(g.players[0].pool.total(), 4, "the mana made stays (733.1)"));
    assert!(!g.log.iter().any(|e| matches!(e, Event::AnnouncementReversed { .. })), "nothing was announced");
    assert!(g.stack.is_empty(), "the spell finished resolving");
    assert_eq!(g.players[0].graveyard.len(), 1);
}

#[test]
fn r605_3a_a_manual_payment_is_offered_when_its_only_mana_source_has_a_question() {
    let (mut g, elf, scales, _) = nested();
    let toll = g.put(0, TOLL, Zone::Hand);
    g.players[0].pool.colors[4] = 1;
    g.act(0, paid("G", Move::Cast { card: toll, targets: vec![] })).unwrap();
    pass_all(&mut g);
    cite!(["605.3a"]; assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::Pay { .. } })), "{:?}", g.decision));
    crate::support::refused(act_unchanged(&mut g, 0, Move::Choose(Answer::Yes)), "submit explicit payment instructions or decline (118.12)");
    let mut declined = g.clone();
    declined.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["118.12"]; assert_eq!(declined.players[0].life, 20));
    g.act(0, Move::Choose(Answer::Pay { plan: vec![elf_step(elf, 3)], with: Some(mana_pool("B")), tapped: vec![] })).unwrap();
    choose(&mut g, scales);
    g.act(0, Move::Choose(Answer::Mana(vec![ManaSymbol::Color(Color::Black); 4]))).unwrap();
    assert!(g.decision.is_none() && g.resolving().is_none());
    cite!(["605.3a", "118.12"]; assert_eq!((g.players[0].life, g.players[0].pool.colors[2]), (25, 3)));
}

#[test]
fn r118_12_mana_impossibility_is_not_inferred_from_bounded_search() {
    let d = CardDef::new("Proof", "{0}", "Sorcery", "You may pay {B}. If you can't, you gain 1 life.", None).unwrap();
    assert!(game::play::check(&d).unwrap_err().0.contains("exhaustive payment proof"));
}

#[track_caller]
fn act_unchanged(g: &mut Game, player: usize, action: Move) -> Result<(), String> {
    let before = format!("{g:?}");
    let result = g.act(player, action);
    assert_eq!(format!("{g:?}"), before);
    result
}
