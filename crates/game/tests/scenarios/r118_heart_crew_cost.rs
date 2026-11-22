//! Heart's static permission replaces its Crew cost; counter removal is not a loyalty ability.
use crate::support::{cite, oracle, paid};
use game::{Event, Game, Move, ObjId, Target, Zone};
use ir::CardType;

const HEART: usize = 0;
const JACE: usize = 1;
const LILIANA: usize = 2;
const BEARS: usize = 3;
const MEMNITE: usize = 4;
const FOREST: usize = 5;
const STIFLE: usize = 6;
const MATRIX: usize = 7;
const KRAJ: usize = 8;
const QUILL: usize = 9;

fn game() -> Game {
    let defs = ["Heart of Kiran", "Jace Beleren", "Liliana of the Veil", "Grizzly Bears", "Memnite", "Forest", "Stifle", "Damping Matrix", "Experiment Kraj", "Quillspike"]
        .into_iter().map(oracle).collect::<Vec<_>>();
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..20 { g.put(p, FOREST, Zone::Library); } }
    g
}
fn walker(g: &mut Game, player: usize, card: usize, loyalty: u32) -> ObjId {
    let object = g.put(player, card, Zone::Battlefield);
    g.set_initial_counters(object, "loyalty", loyalty);
    object
}
fn crew_index(g: &Game, source: ObjId) -> usize {
    g.abilities(source).iter().position(|member| matches!(&**member,
        ir::Ability::Activated { cost, .. } if matches!(cost.0.as_slice(),
            [ir::CostPart::Action(ir::Action::Tap(_))]))).expect("one ordinary Crew activation")
}
fn activate(g: &Game, source: ObjId, paying: Vec<ObjId>, alternative: bool) -> Move {
    Move::WithCostChoice(vec![usize::from(alternative)], Box::new(Move::Activate {
        source, ability: crew_index(g, source), targets: vec![], paying: vec![paying],
    }))
}
fn passes(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }
fn loyalty(g: &Game, walker: ObjId) -> u32 { g.obj(walker).counters.get("loyalty").copied().unwrap_or(0) }

#[test]
fn r602_2b_heart_removes_one_counter_from_the_chosen_planeswalker_before_crew_resolves() {
    let mut g = game();
    let heart = g.put(0, HEART, Zone::Battlefield);
    let jace = walker(&mut g, 0, JACE, 3);
    let liliana = walker(&mut g, 0, LILIANA, 3);
    let action = activate(&g, heart, vec![liliana], true);
    cite!(["602.2b", "601.2h"]; ok: g.act(0, action));
    assert_eq!((loyalty(&g, jace), loyalty(&g, liliana)), (3, 2));
    assert!(!g.obj(jace).tapped && !g.obj(liliana).tapped);
    cite!(["702.122b"]; assert!(g.log.iter().any(|event| matches!(event,
        Event::Crews { vehicle, creatures } if *vehicle == heart && creatures.is_empty()))));
    cite!(["702.122a"]; assert!(!g.is_a(heart, CardType::Creature)));
    passes(&mut g);
    cite!(["702.122a"]; assert!(g.is_a(heart, CardType::Artifact) && g.is_a(heart, CardType::Creature)));
    assert_eq!((g.power(heart), g.toughness(heart)), (Some(4), Some(4)));
    assert!(g.has(heart, "Flying") && g.has(heart, "Vigilance"));
}

#[test]
fn r606_2_counter_payment_is_repeatable_after_a_loyalty_activation_and_does_not_use_its_limit() {
    let mut g = game();
    let heart = g.put(0, HEART, Zone::Battlefield);
    let jace = walker(&mut g, 0, JACE, 3);
    g.act(0, Move::Activate { source: jace, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    passes(&mut g);
    assert_eq!(loyalty(&g, jace), 5);
    for remaining in [4, 3] {
        let action = activate(&g, heart, vec![jace], true);
        cite!(["606.2", "602.2b"]; ok: g.act(0, action));
        assert_eq!(loyalty(&g, jace), remaining);
    }
    assert_eq!(g.stack.len(), 2);
    passes(&mut g); passes(&mut g);
    // A second ordinary Jace loyalty activation remains forbidden, not reset by paying Heart.
    cite!(["606.3"]; refused("a loyalty ability of that permanent has already been activated this turn (606.3)"):
        g.act(0, Move::Activate { source: jace, ability: 0, targets: vec![], paying: vec![] }));
    let removed = g.log.iter().filter(|e| matches!(e, Event::CountersRemoved { from: Target::Object(o), count: 1, kind, .. } if *o == jace && kind == "loyalty")).count();
    assert_eq!(removed, 2);
}

#[test]
fn r606_3_counter_payment_can_be_made_during_the_other_players_turn() {
    let mut g = game();
    let heart = g.put(0, HEART, Zone::Battlefield);
    let jace = walker(&mut g, 0, JACE, 3);
    // A valid positioned main phase of the other player's turn, with our priority.
    g.turn = 2; g.active = 1; g.priority = 0;
    let action = activate(&g, heart, vec![jace], true);
    cite!(["606.2", "117.1b"]; ok: g.act(0, action));
    assert_eq!(loyalty(&g, jace), 2);
    passes(&mut g);
    assert!(g.is_a(heart, CardType::Creature));
}

#[test]
fn r118_3_invalid_holders_branch_and_resource_counts_leave_the_whole_proposal_unchanged() {
    let mut g = game();
    let heart = g.put(0, HEART, Zone::Battlefield);
    let jace = walker(&mut g, 0, JACE, 3);
    let theirs = walker(&mut g, 1, JACE, 3);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    g.set_initial_counters(bears, "loyalty", 3);
    let missing = g.put(0, LILIANA, Zone::Graveyard);
    for paying in [vec![], vec![jace, theirs], vec![theirs], vec![bears], vec![missing]] {
        let before = format!("{g:?}");
        let action = activate(&g, heart, paying.clone(), true);
        let error = g.attempt(0, action).unwrap_err();
        cite!(["118.3", "733.1"]; case: format!("{paying:?}"); assert_eq!(error.class(), game::simulation::Class::Illegal));
        assert_eq!(format!("{g:?}"), before);
    }
    let before = format!("{g:?}");
    let action = Move::Activate { source: heart, ability: crew_index(&g, heart), targets: vec![], paying: vec![vec![jace]] };
    assert!(g.act(0, action).unwrap_err().to_string().contains("alternative"));
    assert_eq!(format!("{g:?}"), before);
}

#[test]
fn r702_122a_the_normal_crew_cost_still_taps_creatures_and_never_spends_a_loyalty_counter() {
    let mut g = game();
    let heart = g.put(0, HEART, Zone::Battlefield);
    let jace = walker(&mut g, 0, JACE, 3);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let memnite = g.put(0, MEMNITE, Zone::Battlefield);
    let action = activate(&g, heart, vec![bears], false);
    cite!(["702.122a"]; refused("chosen creatures have total power less than 3"): g.act(0, action));
    let action = activate(&g, heart, vec![bears, memnite], false);
    cite!(["702.122a"]; ok: g.act(0, action));
    assert!(g.obj(bears).tapped && g.obj(memnite).tapped);
    assert_eq!(loyalty(&g, jace), 3);
    passes(&mut g);
    assert!(g.is_a(heart, CardType::Creature));
}

#[test]
fn r704_5i_the_last_loyalty_counter_can_pay_and_a_countered_crew_ability_does_not_refund_it() {
    let mut g = game();
    let heart = g.put(0, HEART, Zone::Battlefield);
    let jace = walker(&mut g, 0, JACE, 1);
    let action = activate(&g, heart, vec![jace], true);
    g.act(0, action).unwrap();
    cite!(["704.5i"]; assert!(!g.battlefield.contains(&jace)));
    let ability = *g.stack.last().unwrap();
    let stifle = g.put(0, STIFLE, Zone::Hand);
    g.players[0].pool = crate::support::mana_pool("U");
    g.act(0, paid("U", Move::Cast { card: stifle, targets: vec![vec![Target::Object(ability)]] })).unwrap();
    passes(&mut g);
    cite!(["701.6a"]; assert!(g.stack.is_empty()));
    assert!(!g.is_a(heart, CardType::Creature));
    assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).card == Some(JACE)));
}

#[test]
fn r602_5_alternative_payment_does_not_bypass_a_prohibition_on_the_crew_ability() {
    let mut g = game();
    let heart = g.put(0, HEART, Zone::Battlefield);
    let jace = walker(&mut g, 0, JACE, 3);
    g.put(1, MATRIX, Zone::Battlefield);
    let before = format!("{g:?}");
    let action = activate(&g, heart, vec![jace], true);
    cite!(["602.5"]; refused("an effect prohibits activating that ability (602.5)"): g.act(0, action));
    assert_eq!(format!("{g:?}"), before);
}

#[test]
fn r113_2c_inheriting_crew_alone_does_not_copy_hearts_separate_static_permission() {
    let mut g = game();
    let heart = g.put(0, HEART, Zone::Battlefield);
    let jace = walker(&mut g, 0, JACE, 3);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let memnite = g.put(0, MEMNITE, Zone::Battlefield);
    let action = activate(&g, heart, vec![bears, memnite], false);
    g.act(0, action).unwrap(); passes(&mut g);
    g.put_counters(heart, "+1/+1", 1);
    let kraj = g.put(0, KRAJ, Zone::Battlefield);
    let inherited = crew_index(&g, kraj);
    let before = format!("{g:?}");
    let wrong = Move::WithCostChoice(vec![1], Box::new(Move::Activate { source: kraj, ability: inherited, targets: vec![], paying: vec![vec![jace]] }));
    // Uncited (cost protocol): inheriting Crew creates no alternative-choice site.
    crate::support::refused(g.act(0, wrong), "that cost has 0 choice(s) among alternatives");
    assert_eq!(format!("{g:?}"), before);
    cite!(["113.2c", "702.122a"]; ok: g.act(0, Move::Activate { source: kraj, ability: inherited, targets: vec![], paying: vec![vec![heart]] }));
    assert!(g.obj(heart).tapped);
    assert_eq!(loyalty(&g, jace), 3);
    passes(&mut g);
    assert!(g.is_a(kraj, CardType::Artifact));
}

#[test]
fn r118_3_quillspike_pays_with_a_named_counter_on_another_selected_creature() {
    let mut g = game();
    let quill = g.put(0, QUILL, Zone::Battlefield);
    let bear = g.put(0, BEARS, Zone::Battlefield);
    g.set_initial_counters(bear, "-1/-1", 1);
    g.players[0].pool = crate::support::mana_pool("G");
    cite!(["601.2h", "122.1"]; ok: g.act(0, paid("G", Move::Activate {
        source: quill, ability: 0, targets: vec![], paying: vec![vec![bear]],
    })));
    assert!(!g.obj(bear).counters.contains_key("-1/-1"));
    assert_eq!(g.power(quill), Some(1));
    passes(&mut g);
    cite!(["611.2a"]; assert_eq!((g.power(quill), g.toughness(quill)), (Some(4), Some(4))));
}
