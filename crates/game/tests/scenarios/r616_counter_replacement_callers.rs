//! Where a counter replacement question (616.1) can arise in the text of an admitted card:
//! the two places the legacy resolution cannot wait for one, tried with real cards.
//!
//! 1. After part of the same replayed step has happened. A step is one act of a clause, so
//!    "add {R} and put a +1/+1 counter", "draw a card, then put a +1/+1 counter" and "create
//!    a token. Put a +1/+1 counter on it" each put their counters in a step of their own:
//!    what came before is done and stays done, and the question is asked with nothing of its
//!    own step done.
//! 2. Inside a mana ability, which resolves as it is activated (605.3b). The admitted mana
//!    abilities that put counters put a nest, stash or charge counter; every counter
//!    replacement the engine reads is said of +1/+1 counters, so none applies there.
//!    Empowered Autogenerator is covered separately in r605_x_mana.rs.
//!
//! Every definition below passes admission; fixture placement only prepares the starting
//! state. The tested spells and choices run through `Game::act`.
use crate::support::{cite, paid};
use game::operation::ReplacementId;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::Color;
use ir::mana::ManaSymbol;

const SCALES: usize = 0;
const EVOLUTION: usize = 1;
const BEAR: usize = 2;
const DOLL: usize = 3;
const STOCKPILE: usize = 4;
const VISION: usize = 5;
const SPHINX: usize = 6;
const BLADEMASTER: usize = 7;
const FANGS: usize = 8;
const ODDS: usize = 9;

fn game() -> Game {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).unwrap();
    let defs = vec![
        card("Hardened Scales", "{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.", None),
        card("Branching Evolution", "{2}{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.", None),
        card("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Twitching Doll", "{1}{G}", "Artifact Creature — Spider Toy", "{T}: Add one mana of any color. Put a nest counter on this creature.\n{T}, Sacrifice this creature: Create a 2/2 green Spider creature token with reach for each counter on this creature. Activate only as a sorcery.", Some((2, 2))),
        card("Glittering Stockpile", "{2}{R}", "Artifact — Treasure", "{T}: Add {R}. Put a stash counter on this artifact.\n{T}, Sacrifice this artifact: Add X mana of any one color, where X is the number of stash counters on this artifact.", None),
        card("The Vision and Scarlet Witch", "{2}{R}{R}", "Legendary Artifact Creature — Mutant Hero", "Flying\nWhenever you cast a spell, add {R} and put a +1/+1 counter on The Vision and Scarlet Witch.", Some((3, 3))),
        card("Sphinx of Magosi", "{3}{U}{U}{U}", "Creature — Sphinx", "Flying\n{2}{U}: Draw a card, then put a +1/+1 counter on this creature.", Some((6, 6))),
        card("Kazandu Blademaster", "{W}{W}", "Creature — Human Soldier Ally", "First strike, vigilance\nWhenever this creature or another Ally you control enters, you may put a +1/+1 counter on this creature.", Some((1, 1))),
        card("Unexpected Fangs", "{1}{B}", "Instant", "Put a +1/+1 counter and a lifelink counter on target creature.", None),
        card("Match the Odds", "{2}{G}", "Sorcery — Lesson", "Create a 1/1 white Ally creature token. Put a +1/+1 counter on it for each creature your opponents control.", None),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0));
    }
    let mut g = Game::new(defs, 2, 20);
    g.players[0].pool.colors = [10; 5];
    g
}

/// Both replacements under the first player's control.
fn replacements(g: &mut Game) -> (ObjId, ObjId) {
    (g.put(0, SCALES, Zone::Battlefield), g.put(0, EVOLUTION, Zone::Battlefield))
}

/// Everyone passes until the game asks something or the stack is empty.
fn pass(g: &mut Game) {
    for _ in 0..20 {
        if g.decision.is_some() || (g.stack.is_empty() && g.resolving().is_none()) {
            return;
        }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("the stack does not empty");
}

fn counters(g: &Game, o: ObjId, kind: &str) -> u32 {
    g.obj(o).counters.get(kind).copied().unwrap_or(0)
}

fn effect(source: ObjId) -> ReplacementId {
    ReplacementId::from_ability(source, game::binding::AbilityKey::printed(0), 0)
}

/// The replacement question now asked: of whom, about which permanent, how many +1/+1
/// counters, and which effects.
fn question(g: &Game) -> (usize, ObjId, u32, Vec<ReplacementId>) {
    question_kind(g, "+1/+1")
}
fn question_kind(g: &Game, expected_kind: &str) -> (usize, ObjId, u32, Vec<ReplacementId>) {
    match &g.decision {
        Some(Decision::Choose { player, choice: Choice::Replacement { on: Target::Object(on), kind, count, among, optional: false, .. } }) => {
            assert_eq!(kind, expected_kind);
            (*player, *on, *count, among.clone())
        }
        other => panic!("no replacement question: {other:?}"),
    }
}

fn choose(g: &mut Game, source: ObjId) {
    g.act(0, Move::Choose(Answer::Replacement(effect(source)))).unwrap();
    assert_eq!(g.decision, None, "the effect left applies without a question (614.5)");
}

fn happened(g: &Game, from: usize, what: impl Fn(&Event) -> bool) -> usize {
    g.log[from..].iter().filter(|e| what(e)).count()
}

// ---- inside a mana ability (605.3b) ----

#[test]
fn r605_3b_a_mana_ability_that_puts_a_nest_counter_asks_nothing_under_both_replacements() {
    // "{T}: Add one mana of any color. Put a nest counter on this creature.": the creature is
    // one "a creature you control" names, the counter is not a +1/+1 counter
    let mut g = game();
    replacements(&mut g);
    let doll = g.put(0, DOLL, Zone::Battlefield);
    assert!(!g.summoning_sick(doll), "what a game is set up with can be tapped (302.6)");
    let before = g.players[0].pool.total();
    let tap = Move::Activate { source: doll, ability: 0, targets: vec![], paying: vec![] };
    g.act(0, Move::Adding(vec![vec![ManaSymbol::Color(Color::Green)]], Box::new(tap))).unwrap();
    assert_eq!(g.decision, None);
    cite!(["605.3b"]; assert!(g.stack.is_empty()));
    cite!(["605.3b"]; assert_eq!((counters(&g, doll, "nest"), counters(&g, doll, "+1/+1"), g.players[0].pool.total()), (1, 0, before + 1)));
}

#[test]
fn r605_3b_a_mana_ability_that_puts_a_stash_counter_asks_nothing_under_both_replacements() {
    // "{T}: Add {R}. Put a stash counter on this artifact.": not a creature, not a +1/+1
    // counter
    let mut g = game();
    replacements(&mut g);
    let stockpile = g.put(0, STOCKPILE, Zone::Battlefield);
    let before = g.players[0].pool.total();
    g.act(0, Move::Activate { source: stockpile, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    cite!(["605.3b"]; assert_eq!((g.decision.clone(), counters(&g, stockpile, "stash"), g.players[0].pool.total()), (None, 1, before + 1)));
}

// ---- after something else the same text does ----

#[test]
fn r616_1_add_mana_and_put_a_counter_asks_after_the_mana_and_adds_it_once() {
    // "Whenever you cast a spell, add {R} and put a +1/+1 counter on …": two acts of one
    // clause, a step each
    let mut g = game();
    let (scales, evolution) = replacements(&mut g);
    let vision = g.put(0, VISION, Zone::Battlefield);
    let from = g.log.len();
    let card = g.put(0, BEAR, Zone::Hand);
    g.act(0, paid("GG", Move::Cast { card, targets: vec![] })).unwrap();
    pass(&mut g);
    cite!(["616.1"]; assert_eq!(question(&g), (0, vision, 1, vec![effect(scales), effect(evolution)])));
    cite!(["608.2c"]; assert_eq!(happened(&g, from, |e| matches!(e, Event::ManaAdded { source, .. } if *source == vision)), 1, "the mana is added before the question"));
    assert_eq!(counters(&g, vision, "+1/+1"), 0);
    choose(&mut g, scales);
    cite!(["616.1f"]; assert_eq!(counters(&g, vision, "+1/+1"), 4));
    pass(&mut g);
    assert_eq!(happened(&g, from, |e| matches!(e, Event::ManaAdded { source, .. } if *source == vision)), 1, "and not again when the ability goes on");
}

#[test]
fn r616_1_draw_then_put_a_counter_asks_after_the_draw_and_draws_once() {
    // "{2}{U}: Draw a card, then put a +1/+1 counter on this creature."
    let mut g = game();
    let (scales, evolution) = replacements(&mut g);
    let sphinx = g.put(0, SPHINX, Zone::Battlefield);
    g.put(0, BEAR, Zone::Library);
    g.put(0, BEAR, Zone::Library);
    let from = g.log.len();
    g.act(0, paid("UUU", Move::Activate { source: sphinx, ability: 1, targets: vec![], paying: vec![] })).unwrap();
    pass(&mut g);
    cite!(["616.1"]; assert_eq!(question(&g), (0, sphinx, 1, vec![effect(scales), effect(evolution)])));
    cite!(["608.2c"]; assert_eq!(happened(&g, from, |e| matches!(e, Event::Drew { .. })), 1, "the card is drawn before the question"));
    choose(&mut g, evolution);
    cite!(["616.1f"]; assert_eq!(counters(&g, sphinx, "+1/+1"), 3));
    pass(&mut g);
    assert_eq!(happened(&g, from, |e| matches!(e, Event::Drew { .. })), 1);
    assert_eq!(g.players[0].library.len(), 1);
}

#[test]
fn r616_1_create_a_token_then_put_counters_on_it_asks_about_the_token_made_once() {
    // "Create a 1/1 white Ally creature token. Put a +1/+1 counter on it for each creature
    // your opponents control."
    let mut g = game();
    let (scales, evolution) = replacements(&mut g);
    g.put(1, BEAR, Zone::Battlefield);
    g.put(1, BEAR, Zone::Battlefield);
    let from = g.log.len();
    let card = g.put(0, ODDS, Zone::Hand);
    g.act(0, paid("GGG", Move::Cast { card, targets: vec![] })).unwrap();
    pass(&mut g);
    let tokens: Vec<ObjId> = g.log[from..].iter().filter_map(|e| if let Event::Created { token } = e { Some(*token) } else { None }).collect();
    let [token] = tokens[..] else { panic!("one token is made before the question: {tokens:?}") };
    cite!(["616.1"]; assert_eq!(question(&g), (0, token, 2, vec![effect(scales), effect(evolution)])));
    choose(&mut g, scales);
    cite!(["616.1f"]; assert_eq!(counters(&g, token, "+1/+1"), 6));
    pass(&mut g);
    assert_eq!(happened(&g, from, |e| matches!(e, Event::Created { .. })), 1);
}

// ---- with another question or another counter in the same step ----

#[test]
fn r616_1_you_may_put_a_counter_asks_whether_and_then_which_replacement() {
    // "Whenever this creature or another Ally you control enters, you may put a +1/+1
    // counter on this creature.": both questions belong to one step, the first answered to
    // the resolution and the second to the counter operation
    let mut g = game();
    let (scales, evolution) = replacements(&mut g);
    let card = g.put(0, BLADEMASTER, Zone::Hand);
    g.act(0, paid("WW", Move::Cast { card, targets: vec![] })).unwrap();
    pass(&mut g);
    assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May })), "{:?}", g.decision);
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    let (player, blademaster, count, among) = question(&g);
    cite!(["616.1"]; assert_eq!((player, count, among), (0, 1, vec![effect(scales), effect(evolution)])));
    assert_eq!(g.obj(blademaster).card, Some(BLADEMASTER));
    choose(&mut g, evolution);
    cite!(["616.1f"]; assert_eq!(counters(&g, blademaster, "+1/+1"), 3));
    pass(&mut g);
    assert_eq!((g.decision.clone(), g.stack.len(), counters(&g, blademaster, "+1/+1")), (None, 0, 3));
}

#[test]
fn r616_1_two_kinds_of_counter_of_one_instruction_are_put_together_after_the_question() {
    // "Put a +1/+1 counter and a lifelink counter on target creature.": the lifelink counter
    // is of no replacement's kind and waits in the same holder inventory. The
    // protocol question describes that complete two-counter inventory; only +1/+1 changes.
    let mut g = game();
    let (scales, evolution) = replacements(&mut g);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let card = g.put(0, FANGS, Zone::Hand);
    g.act(0, paid("BB", Move::Cast { card, targets: vec![vec![Target::Object(bear)]] })).unwrap();
    pass(&mut g);
    let Some(Decision::Choose { player, choice: Choice::Replacement { on, counters: Some(proposal), among, optional: false, .. } }) = &g.decision else { panic!("complete counter inventory question") };
    cite!(["616.1"]; assert_eq!((*player,*on,proposal.by), (0,Target::Object(bear),0)));
    assert_eq!(proposal.inventory,vec![("+1/+1".into(),1),("lifelink".into(),1)]);
    assert_eq!(among,&vec![effect(scales),effect(evolution)]);
    assert_eq!((counters(&g, bear, "+1/+1"), counters(&g, bear, "lifelink")), (0, 0), "nothing is put while the order is chosen");
    choose(&mut g, scales);
    cite!(["616.1f"]; assert_eq!((counters(&g, bear, "+1/+1"), counters(&g, bear, "lifelink")), (4, 1)));
}
