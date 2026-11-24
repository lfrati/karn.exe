//! Jump-start (702.133a): "You may cast this card from your graveyard if the resulting spell is
//! an instant or sorcery spell by discarding a card as an additional cost to cast it" and "If
//! this spell was cast using its jump-start ability, exile this card instead of putting it
//! anywhere else any time it would leave the stack." Every card is read from its Oracle text.
use crate::support::{Board, T, cite, mana_pool, oracle, paid};
use game::announcement::CastRoute;
use game::{Move, ObjId, Target, Zone};

fn jump_start(card: ObjId, discard: Vec<ObjId>, targets: Vec<Vec<Target>>) -> Move {
    Move::CastVia(CastRoute::JumpStart, Box::new(Move::WithCosts(vec![discard], Box::new(Move::Cast { card, targets }))))
}

/// Radical Idea, {1}{U} instant: "Draw a card. / Jump-start", in player 0's graveyard with a
/// card in hand to discard.
fn setup() -> (T, ObjId, ObjId) {
    let mut t = Board::new(2).named(0, Zone::Graveyard, "idea", "Radical Idea").hand(0, &["Forest"])
        .hand(1, &["Remand"]).library(0, &["Island", "Island"]).library(1, &["Forest"]).build();
    let (idea, forest) = (t.id("idea"), t.find_in(0, Zone::Hand, "Forest"));
    t.g.players[0].pool = mana_pool("UU");
    (t, idea, forest)
}

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Radical Idea", "Direct Current", "Remand"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&crate::support::oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

#[test]
fn r702_133a_cast_from_the_graveyard_by_discarding_a_card_then_exiled() {
    let (mut t, idea, forest) = setup();
    cite!(["702.133a", "601.2f"]; ok: t.g.act(0, paid("UU", jump_start(idea, vec![forest], vec![]))));
    cite!(["702.133a", "701.9a"]; assert_eq!(t.g.players[0].hand.len(), 0, "the Forest is discarded"));
    t.pass_all();
    cite!(["702.133a"]; assert_eq!(t.g.players[0].hand.len(), 1, "it resolved and drew a card"));
    t.expect().rules(["702.133a"]).zone_exact(0, Zone::Graveyard, &["Forest"]).zone_exact(0, Zone::Exile, &["Radical Idea"]).check();
}

#[test]
fn r702_133a_a_countered_jump_started_spell_is_exiled_instead_of_going_to_hand() {
    // Ruling: "A spell cast using jump-start will always be exiled afterward, whether it
    // resolves, it's countered, or it leaves the stack in some other way."
    let (mut t, idea, forest) = setup();
    t.g.act(0, paid("UU", jump_start(idea, vec![forest], vec![]))).unwrap();
    let spell = *t.g.stack.last().unwrap();
    t.g.act(0, Move::Pass).unwrap();
    let remand = t.find_in(1, Zone::Hand, "Remand");
    t.g.players[1].pool = mana_pool("UU");
    t.g.act(1, paid("UU", Move::Cast { card: remand, targets: vec![vec![Target::Object(spell)]] })).unwrap();
    t.pass_all();
    cite!(["702.133a", "614.1a"]; assert!(t.g.players[0].hand.is_empty(), "Remand's return to hand is replaced"));
    t.expect().rules(["702.133a"]).zone_exact(0, Zone::Exile, &["Radical Idea"]).check();
}

#[test]
fn r702_133a_from_the_hand_it_is_cast_as_usual_and_goes_to_the_graveyard() {
    let mut t = Board::new(2).hand(0, &["Radical Idea", "Forest"]).library(0, &["Island"]).build();
    let idea = t.find_in(0, Zone::Hand, "Radical Idea");
    t.g.players[0].pool = mana_pool("UU");
    let forest = t.find_in(0, Zone::Hand, "Forest");
    cite!(["702.133a"]; refused("cannot be cast using jump-start"): t.g.act(0, paid("UU", jump_start(idea, vec![forest], vec![]))));
    cite!(["601.2"]; ok: t.g.act(0, paid("UU", Move::Cast { card: idea, targets: vec![] })));
    t.pass_all();
    t.expect().rules(["702.133a", "608.2n"]).zone_exact(0, Zone::Graveyard, &["Radical Idea"]).zone_exact(0, Zone::Exile, &[]).check();
}

#[test]
fn r702_133a_the_discard_is_required_and_is_one_card() {
    let (mut t, idea, _) = setup();
    cite!(["702.133a", "601.2h"]; refused("exactly 1 card"): t.g.act(0, paid("UU", jump_start(idea, vec![], vec![]))));
    cite!(["702.133a", "601.2h"]; refused("not what that cost asks for"): t.g.act(0, paid("UU", jump_start(idea, vec![idea], vec![]))));
}

#[test]
fn r702_133a_a_sorcery_keeps_its_timing() {
    // Ruling: "you can cast a sorcery using jump-start only when you could normally cast a
    // sorcery."
    let mut t = Board::new(2).fill(5).named(0, Zone::Graveyard, "current", "Direct Current").hand(0, &["Forest"]).build();
    let current = t.id("current");
    let forest = t.find_in(0, Zone::Hand, "Forest");
    t.to_no_combat(1, crate::support::Part::PrecombatMain);
    t.g.act(1, Move::Pass).unwrap();
    t.g.players[0].pool = mana_pool("RRR");
    cite!(["702.133a", "307.1", "117.1a"]; refused("in a main phase of your own turn with the stack empty"): t.g.act(0, paid("CRR", jump_start(current, vec![forest], vec![vec![Target::Player(1)]]))));
}

#[test]
fn r702_133a_a_given_jump_start_also_exiles_the_card() {
    // what Filigree Racer's reflexive trigger does: "target instant or sorcery card in your
    // graveyard gains jump-start until end of turn"
    let grant = game::CardDef::new("Grant witness", "{0}", "Instant",
        "Target instant or sorcery card in your graveyard gains jump-start until end of turn.", None).unwrap();
    let mut g = game::Game::new(vec![grant, oracle("Giant Growth"), oracle("Forest"), oracle("Grizzly Bears")], 2, 20);
    for player in 0..2 { for _ in 0..5 { g.put(player, 2, Zone::Library); } }
    let witness = g.put(0, 0, Zone::Hand);
    let growth = g.put(0, 1, Zone::Graveyard);
    let forest = g.put(0, 2, Zone::Hand);
    let bears = g.put(0, 3, Zone::Battlefield);
    let cast = || paid("G", jump_start(growth, vec![forest], vec![vec![Target::Object(bears)]]));
    g.players[0].pool = mana_pool("G");
    cite!(["702.133a"]; refused("cannot be cast using jump-start"): g.act(0, cast()));
    g.act(0, Move::Cast { card: witness, targets: vec![vec![Target::Object(growth)]] }).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    g.players[0].pool = mana_pool("G");
    cite!(["702.133a", "611.2a"]; ok: g.act(0, cast()));
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["702.133a"]; assert_eq!(g.power(bears), Some(5), "Giant Growth resolved"));
    let exiled: Vec<_> = g.exile.iter().filter_map(|o| g.obj(*o).card).collect();
    cite!(["702.133a"]; assert_eq!(exiled, vec![1], "the card is exiled, not put into its graveyard"));
}
