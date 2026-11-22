//! 119.3: "If an effect causes a player to gain life or lose life, that player's life total is
//! adjusted accordingly." The rules bound no life total (107.1b). The engine holds one in a
//! 32-bit signed number, what was lost this turn in a 32-bit unsigned one, and so the life one
//! instruction made its players lose and the life one payment pays: a gain, loss, payment or
//! damage result that would leave any of them past that range is refused as unsupported,
//! never wrapped or clamped, and the action is left undone. Up to each bound it is applied
//! exactly. Life past what the engine counts is more life than any life total, which can't be
//! paid (119.4). Each card is a test text read by the compiler. A player at 0 or less life
//! loses the game when state-based actions are checked (704.5a), so the spells' losses start
//! from 20 life and are larger than 2^31.
use crate::support::cite;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};

const ELIXIR: usize = 0;
const DRAIN: usize = 1;
const SIPHON: usize = 2;
const BOLT: usize = 3;
const BLAST: usize = 4;
const LEECH: usize = 5;
const BEAR: usize = 6;
const CRYSTAL: usize = 7;
const SPARK: usize = 8;
const TOLL: usize = 9;
const RUIN: usize = 10;
const COVENANT: usize = 11;
const FONT: usize = 12;
const BATTERY: usize = 13;
const WAGER: usize = 14;
const PLEDGE: usize = 15;

const RANGE: &str = "life total exceeds the engine numerical range";

fn game() -> Game {
    let defs: Vec<CardDef> = [
        ("Elixir", "{0}", "Sorcery", "You gain 5 life.", None),
        // 20 - 2147483668 = -2147483648, the smallest 32-bit signed number
        ("Drain", "{0}", "Sorcery", "Target player loses 2147483668 life.", None),
        ("Siphon", "{0}", "Sorcery", "Target player loses 2147483669 life.", None),
        ("Bolt", "{0}", "Instant", "Bolt deals 2147483668 damage to target player.", None),
        ("Blast", "{0}", "Instant", "Blast deals 2147483669 damage to target player.", None),
        ("Leech", "{0}", "Creature", "Lifelink", Some((1, 1))),
        ("Bear", "{0}", "Creature", "", Some((2, 2))),
        // The Darkness Crystal's replacement effect
        ("Crystal", "{0}", "Artifact", "If a nontoken creature an opponent controls would die, instead exile it and you gain 2 life.", None),
        ("Spark", "{0}", "Instant", "Spark deals 3 damage to target creature.", None),
        // 2 * 2147483647 = 4294967294, the most two equal losses can be within 32 unsigned bits
        ("Toll", "{0}", "Sorcery", "Each player loses 2147483647 life.", None),
        // 2 * 2147483648 = 4294967296, past them
        ("Ruin", "{0}", "Sorcery", "Each player loses 2147483648 life. You gain life equal to the life lost this way.", None),
        ("Covenant", "{B/P}", "Sorcery", "As an additional cost to cast this spell, pay X life.\nYou gain 1 life.", None),
        ("Font", "{0}", "Artifact", "{B/P}, Pay X life: You gain 1 life.", None),
        ("Battery", "{0}", "Artifact", "", None),
        // its life is counted as the payment is offered, not first given to an X
        ("Wager", "{0}", "Sorcery", "You may pay life equal to the number of charge counters on artifacts you control. If you do, you gain 1 life.", None),
        ("Pledge", "{0}", "Artifact", "{T}, Pay life equal to the number of charge counters on artifacts you control: You gain 1 life.", None),
    ].into_iter().map(|(name, cost, types, text, pt)| {
        let d = CardDef::new(name, cost, types, text, pt).unwrap_or_else(|e| panic!("{name}: {e}"));
        game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
        d
    }).collect();
    Game::new(defs, 2, 20)
}
/// Player 0 casts this spell, targeting `target` if it has one; the result of the pass that
/// resolves it, and the game as it was just before that pass.
fn resolve(g: &mut Game, card: usize, target: Option<Target>) -> (Result<(), String>, String) {
    let card = g.put(0, card, Zone::Hand);
    let targets = target.map(|t| vec![vec![t]]).unwrap_or_default();
    g.act(0, Move::Cast { card, targets }).unwrap();
    g.act(0, Move::Pass).unwrap();
    let before = format!("{g:?}");
    (g.act(1, Move::Pass), before)
}
fn life_events(g: &Game) -> usize { g.log.iter().filter(|e| matches!(e, Event::LifeGained { .. } | Event::LifeLost { .. })).count() }

#[test]
fn r119_3_a_gain_is_applied_up_to_the_largest_life_total_and_refused_past_it() {
    let mut g = game();
    g.players[0].life = i32::MAX - 5;
    resolve(&mut g, ELIXIR, None).0.unwrap();
    cite!(["119.3"]; assert_eq!(g.players[0].life, i32::MAX));
    let (result, before) = resolve(&mut g, ELIXIR, None);
    cite!(["119.3", "107.1b"]; refused(RANGE): result);
    assert_eq!(format!("{g:?}"), before);
}

#[test]
fn r119_3_a_loss_is_applied_down_to_the_smallest_life_total_and_refused_past_it() {
    let mut g = game();
    let (result, before) = resolve(&mut g, SIPHON, Some(Target::Player(1)));
    cite!(["119.3", "107.1b"]; refused(RANGE): result);
    assert_eq!(format!("{g:?}"), before);
    assert_eq!(g.players[1].life, 20);
    let mut g = game();
    resolve(&mut g, DRAIN, Some(Target::Player(1))).0.unwrap();
    cite!(["119.3"]; assert_eq!(g.players[1].life, i32::MIN));
    cite!(["119.3"]; assert_eq!(g.life_lost_this_turn(1), 2_147_483_668));
    cite!(["704.5a"]; assert!(g.players[1].lost));
}

#[test]
fn r120_3a_damage_to_a_player_is_applied_down_to_the_smallest_life_total_and_refused_past_it() {
    let mut g = game();
    let (result, before) = resolve(&mut g, BLAST, Some(Target::Player(1)));
    cite!(["120.3a", "107.1b"]; refused(RANGE): result);
    assert_eq!(format!("{g:?}"), before);
    let mut g = game();
    resolve(&mut g, BOLT, Some(Target::Player(1))).0.unwrap();
    cite!(["120.3a"]; assert_eq!(g.players[1].life, i32::MIN));
    cite!(["704.5a"]; assert!(g.players[1].lost));
}

#[test]
fn r120_3f_a_lifelink_gain_past_the_range_refuses_the_whole_damage_event() {
    let mut g = game();
    let leech = g.put(0, LEECH, Zone::Battlefield);
    g.players[0].life = i32::MAX - 1;
    g.try_damage(leech, Target::Player(1), 1).unwrap();
    // the damage's results: player 1 loses 1 life (120.3a), its controller gains 1 (120.3f)
    cite!(["120.3a", "120.3f"]; assert_eq!((g.players[0].life, g.players[1].life), (i32::MAX, 19)));
    let (events, before) = (life_events(&g), format!("{g:?}"));
    // the gain is a result of the same damage event (120.3): none of that event happens
    cite!(["120.3f", "107.1b"]; refused("exceeds the engine numerical range"): g.try_damage(leech, Target::Player(1), 1).map_err(|e| e.0));
    assert_eq!(format!("{g:?}"), before);
    assert_eq!((g.players[1].life, life_events(&g)), (19, events));
}

#[test]
fn r119_4_paying_life_is_bounded_by_the_life_total_and_counts_as_losing_it() {
    let mut g = game();
    g.players[0].life = i32::MAX;
    // more than the life total can't be paid (119.4), whatever its size
    cite!(["119.4"]; assert!(!g.can_pay_life(0, u32::MAX)));
    cite!(["119.4"]; assert!(!g.pay_life(0, i32::MAX as u32 + 1).unwrap()));
    assert_eq!(g.players[0].life, i32::MAX);
    cite!(["119.4"]; assert!(g.pay_life(0, i32::MAX as u32).unwrap()));
    cite!(["119.4"]; assert_eq!((g.players[0].life, g.life_lost_this_turn(0)), (0, i32::MAX as u32)));
    // 0 life can always be paid (119.4b)
    g.players[0].life = i32::MIN;
    cite!(["119.4b"]; assert!(g.pay_life(0, 0).unwrap()));
    assert_eq!(g.players[0].life, i32::MIN);
}

#[test]
fn r119_3_life_lost_this_turn_is_counted_exactly_and_refused_past_its_range() {
    let mut g = game();
    g.players[0].life = i32::MAX;
    // the largest loss there is: from the largest life total to the smallest
    g.lose_life(0, u32::MAX).unwrap();
    cite!(["119.3"]; assert_eq!((g.players[0].life, g.life_lost_this_turn(0)), (i32::MIN, u32::MAX)));
    // a gain does not reduce what was lost (119.3)
    g.gain_life(0, u32::MAX).unwrap();
    cite!(["119.3"]; assert_eq!((g.players[0].life, g.life_lost_this_turn(0)), (i32::MAX, u32::MAX)));
    let (events, before) = (life_events(&g), format!("{g:?}"));
    cite!(["119.3", "107.1b"]; refused("life lost this turn exceeds the engine numerical range"): g.lose_life(0, 1).map_err(|e| e.0));
    assert_eq!((g.players[0].life, life_events(&g), format!("{g:?}")), (i32::MAX, events, before));
}

#[test]
fn r119_3_a_direct_gain_or_loss_past_the_range_changes_nothing() {
    let mut g = game();
    g.players[0].life = i32::MIN + 1;
    cite!(["119.3", "107.1b"]; refused(RANGE): g.lose_life(0, 2).map_err(|e| e.0));
    cite!(["119.3"]; assert_eq!((g.players[0].life, g.life_lost_this_turn(0), life_events(&g)), (i32::MIN + 1, 0, 0)));
    g.players[0].life = 0;
    cite!(["119.3", "107.1b"]; refused(RANGE): g.gain_life(0, u32::MAX).map_err(|e| e.0));
    cite!(["119.3"]; assert_eq!((g.players[0].life, life_events(&g)), (0, 0)));
}

#[test]
fn r119_5_setting_or_exchanging_a_life_total_is_refused_as_the_card_is_read() {
    // neither is run by the engine (119.5, 701.12): refused at admission, never approximated
    for (text, action) in [("Your life total becomes 10.", "LifeTotalBecomes"), ("Exchange life totals with target player.", "ExchangeLifeTotals")] {
        let checked = CardDef::new("Swap", "{0}", "Sorcery", text, None).map_err(|e| e.to_string())
            .and_then(|d| game::play::check(&d).map_err(|e| e.0));
        cite!(["119.5", "701.12a"]; case: text; refused(&format!("action not lowered: {action}")): checked);
    }
}

#[test]
fn r614_6_a_replacements_gain_past_the_range_refuses_the_event_it_replaces() {
    // the Bear is exiled instead of dying and the Crystal's controller gains 2 life (614.1a,
    // 614.6): up to the largest life total that is applied; past it nothing of the event
    // happens, and the pass that resolves Spark is refused with it
    for (life, applied) in [(i32::MAX - 2, true), (i32::MAX - 1, false)] {
        let mut g = game();
        g.players[0].life = life;
        g.put(0, CRYSTAL, Zone::Battlefield);
        let bear = g.put(1, BEAR, Zone::Battlefield);
        let (result, before) = resolve(&mut g, SPARK, Some(Target::Object(bear)));
        if applied {
            result.unwrap();
            cite!(["614.6", "119.3"]; case: life; assert_eq!(g.players[0].life, i32::MAX));
            cite!(["614.6"]; case: life; assert!(g.exile.iter().any(|o| g.obj(*o).card == Some(BEAR))));
            assert!(g.players[1].graveyard.is_empty());
        } else {
            cite!(["614.6", "119.3", "107.1b"]; case: life; refused(RANGE): result);
            assert_eq!(format!("{g:?}"), before);
            assert_eq!((g.players[0].life, g.obj(bear).zone), (life, Zone::Battlefield));
        }
    }
}

#[test]
fn r119_3_the_life_one_instruction_makes_its_players_lose_is_counted_up_to_its_range() {
    // Toll: each player loses 2147483647 life, 4294967294 in all
    let mut g = game();
    resolve(&mut g, TOLL, None).0.unwrap();
    for p in 0..2 {
        cite!(["119.3"]; case: p; assert_eq!((g.players[p].life, g.life_lost_this_turn(p)), (20 - i32::MAX, i32::MAX as u32)));
        cite!(["704.5a"]; case: p; assert!(g.players[p].lost));
    }
    // Ruin: each loses 2147483648 life and its caster gains the life lost this way (119.3),
    // 4294967296 in all, which the engine does not count: refused, and no life is lost
    let mut g = game();
    let (result, before) = resolve(&mut g, RUIN, None);
    cite!(["119.3", "107.1b"]; refused("life lost exceeds the engine numerical range"): result);
    assert_eq!(format!("{g:?}"), before);
    assert_eq!((g.players[0].life, g.players[1].life, life_events(&g)), (20, 20, 0));
}

/// Pay each Phyrexian symbol with 2 life (107.4f), with this X.
fn with_life(x: u32, mv: Move) -> Move { Move::WithLife(vec![true], Box::new(Move::WithX(x, Box::new(mv)))) }

#[test]
fn r119_4_the_life_parts_of_one_cost_are_paid_together_counted_exactly() {
    // Covenant's {B/P} paid with 2 life and its additional X life are one payment of 2 + X
    // life (119.4, 601.2h), as are Font's: 2 + 4294967294 = 4294967296 is more than any life
    // total, never 0, and the cast or activation is refused with nothing paid
    let mut g = game();
    let card = g.put(0, COVENANT, Zone::Hand);
    let font = g.put(0, FONT, Zone::Battlefield);
    let cast = Move::Cast { card, targets: vec![] };
    let activate = Move::Activate { source: font, ability: 0, targets: vec![], paying: vec![] };
    for (rule, mv) in [("601.2h", &cast), ("602.2b", &activate)] {
        let before = format!("{g:?}");
        cite!(["119.4", "107.4f", rule]; case: rule; refused("you can't pay 4294967296 life with a life total of 20 (119.4)"): g.act(0, with_life(u32::MAX - 1, mv.clone())));
        assert_eq!(format!("{g:?}"), before);
    }
    // exactly the life total can be paid (119.4): 2 + 2147483645 = 2147483647
    for (rule, mv) in [("601.2h", cast), ("602.2b", activate)] {
        let mut g = g.clone();
        g.players[0].life = i32::MAX;
        g.act(0, with_life(i32::MAX as u32 - 2, mv)).unwrap();
        cite!(["119.4", rule]; case: rule; assert_eq!((g.players[0].life, g.life_lost_this_turn(0)), (0, i32::MAX as u32)));
        cite!(["704.5a"]; case: rule; assert!(g.players[0].lost));
    }
}

/// Wager resolves for player 0 at the largest life total, with charge counters on artifacts
/// it controls: the game, and the result of the pass that resolves it.
fn wager(counters: [u32; 2]) -> (Game, Result<(), String>) {
    let mut g = game();
    g.players[0].life = i32::MAX;
    for n in counters {
        let battery: ObjId = g.put(0, BATTERY, Zone::Battlefield);
        g.put_counters(battery, "charge", n);
    }
    let result = resolve(&mut g, WAGER, None).0;
    (g, result)
}

#[test]
fn r119_4_life_an_effect_offers_past_the_engines_count_is_more_than_any_life_total() {
    // 2^31 + 2^31 = 4294967296 life is more than any life total and can't be paid (119.4):
    // nothing is offered, and "if you do" finds it not paid (118.12)
    let (g, result) = wager([1 << 31, 1 << 31]);
    result.unwrap();
    cite!(["119.4"]; assert!(g.decision.is_none()));
    cite!(["119.4", "118.12"]; assert_eq!((g.players[0].life, g.stack.len(), life_events(&g)), (i32::MAX, 0, 0)));
    // 2^30 + 2^30 - 1 = 2147483647, the life total: offered, paid, then 1 life gained
    let (mut g, result) = wager([1 << 30, (1 << 30) - 1]);
    result.unwrap();
    let Some(Decision::Choose { player: 0, choice: Choice::Pay { life, .. } }) = g.decision.clone() else { panic!("a payment is offered: {:?}", g.decision) };
    cite!(["119.4"]; assert_eq!(life, i32::MAX as u32));
    g.act(0, Move::Choose(Answer::Pay { plan: vec![], with: None, tapped: vec![] })).unwrap();
    cite!(["119.4", "118.12", "119.3"]; assert_eq!((g.players[0].life, g.life_lost_this_turn(0)), (1, i32::MAX as u32)));
}

/// Pledge and another artifact of player 0's with these charge counters, at this life total:
/// the game, and Pledge's activation.
fn pledge(life: i32, counters: [u32; 2]) -> (Game, ObjId, Move) {
    let mut g = game();
    g.players[0].life = life;
    let pledge = g.put(0, PLEDGE, Zone::Battlefield);
    let battery = g.put(0, BATTERY, Zone::Battlefield);
    for (o, n) in [pledge, battery].into_iter().zip(counters) { g.put_counters(o, "charge", n); }
    (g, pledge, Move::Activate { source: pledge, ability: 0, targets: vec![], paying: vec![] })
}

#[test]
fn r119_4_a_life_cost_counted_past_the_engines_range_cannot_be_paid() {
    // Pledge's life is the charge counters on player 0's artifacts as its cost is paid
    // (601.2h, 602.2b): 2^31 + 2^31 = 4294967296 is more than any life total (119.4), and
    // the activation is refused with nothing paid, Pledge untapped
    let (mut g, source, activate) = pledge(20, [1 << 31, 1 << 31]);
    let before = format!("{g:?}");
    cite!(["119.4", "602.2b"]; refused("you can't pay more life than your life total of 20 (119.4)"): g.act(0, activate));
    assert_eq!(format!("{g:?}"), before);
    assert!(!g.obj(source).tapped);
    // 2^30 + 2^30 - 1 = 2147483647, the life total: paid
    let (mut g, source, activate) = pledge(i32::MAX, [1 << 30, (1 << 30) - 1]);
    g.act(0, activate).unwrap();
    cite!(["119.4", "602.2b"]; assert_eq!((g.players[0].life, g.life_lost_this_turn(0), g.obj(source).tapped), (0, i32::MAX as u32, true)));
}
