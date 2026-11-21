//! Independent explore ordering, changed control and refused-shape checks.
use crate::support::cite::cite;
use game::{CardDef, Game, Move, ObjId, Target, Zone, Event};
use game::state::{Answer, Choice, Decision};
use ir::Part;

fn game(players: usize) -> Game {
    let c = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).unwrap();
    let defs = vec![
        c("Expedition", "Instant", "Each creature explores.", None),
        c("Bear", "Creature — Bear", "", Some((2, 2))),
        c("Nonland", "Instant", "You gain 1 life.", None),
        c("Forest", "Basic Land — Forest", "", None),
        c("Scout", "Creature — Scout", "When this creature enters, it explores.", Some((1, 1))),
        c("Steal and bounce", "Instant", "Gain control of target creature until end of turn. Return it to its owner's hand.", None),
        c("Watch", "Enchantment", "Whenever a creature you control explores, you gain 3 life.", None),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, players, 20);
    for player in 0..players { for _ in 0..5 { g.put(player, 2, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn pass(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }
fn explore_events(g: &Game) -> Vec<(ObjId, usize)> {
    g.log.iter().filter_map(|e| if let Event::Explored { object, player } = e { Some((*object, *player)) } else { None }).collect()
}

#[test]
fn r701_44d_the_next_explorer_is_chosen_in_apnap_from_a_nonzero_active_player() {
    // Rules: 701.44d, 101.4
    let mut g = game(3);
    while !(g.active == 1 && g.part == Part::PrecombatMain) { g.act(g.priority, Move::Pass).unwrap(); }
    let p0 = g.put(0, 1, Zone::Battlefield);
    let p2 = g.put(2, 1, Zone::Battlefield);
    let p1a = g.put(1, 1, Zone::Battlefield);
    let p1b = g.put(1, 1, Zone::Battlefield);
    let spell = g.put(1, 0, Zone::Hand);
    g.act(1, Move::Cast { card: spell, targets: vec![] }).unwrap();
    pass(&mut g);
    let Some(Decision::Choose { player: 1, choice: Choice::Objects { verb: ir::Verb::Explore, among, count: 1, .. } }) = g.decision.clone() else { panic!("{:?}", g.decision) };
    cite!(["701.44d"]; assert_eq!(among, vec![p1a, p1b]));
    g.act(1, Move::Choose(Answer::Objects(vec![p1b]))).unwrap();
    for player in [1, 1, 2, 0] {
        cite!(["701.44a", "701.44d"]; case: format!("chooser {player}"); assert!(matches!(g.decision, Some(Decision::Choose { player: chooser, choice: Choice::May }) if chooser == player), "{:?}", g.decision));
        g.act(player, Move::Choose(Answer::No)).unwrap();
    }
    cite!(["701.44d"]; assert_eq!(explore_events(&g), vec![(p1b, 1), (p1a, 1), (p2, 2), (p0, 0)]));
}

#[test]
fn r701_44c_the_departed_explorer_uses_its_last_controller_rather_than_its_owner() {
    // Rules: 701.44a, 701.44b, 701.44c, 608.2h
    let mut g = game(2);
    g.put(1, 6, Zone::Battlefield);
    let forest = g.put(1, 3, Zone::Library);
    let card = g.put(0, 4, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    pass(&mut g);
    let scout = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(4)).unwrap();
    // Setup: the explore trigger is waiting before control changes.
    assert_eq!(g.stack.len(), 1);
    g.act(0, Move::Pass).unwrap();
    let steal = g.put(1, 5, Zone::Hand);
    g.act(1, Move::Cast { card: steal, targets: vec![vec![Target::Object(scout)]] }).unwrap();
    pass(&mut g);
    // Setup: the old object retains the controller it had immediately before bouncing.
    assert_eq!(g.controller(scout), 1);
    // Setup: ensure the explore instruction will need last-known information.
    assert!(!g.battlefield.contains(&scout));
    pass(&mut g);
    cite!(["701.44b", "701.44c"]; assert_eq!(explore_events(&g), vec![(scout, 1)]));
    cite!(["701.44a", "701.44c"]; assert!(!g.players[1].library.contains(&forest)));
    cite!(["701.44a", "701.44c"]; assert!(g.players[1].hand.iter().any(|o| g.obj(*o).card == Some(3))));
    // the bounce card's own text returned it to its owner's hand: uncited
    assert_eq!(g.players[0].hand.iter().filter(|o| g.obj(**o).card == Some(4)).count(), 1);
    pass(&mut g);
    cite!(["701.44b", "701.44c", "603.2"]; assert_eq!(g.players[1].life, 23, "Watch: a creature its controller last controlled explored"));
}

#[test]
fn r701_44a_unsupported_explore_shapes_are_refused_instead_of_being_ignored() {
    // Admission guards mix unsupported legal instructions with an invalid player explorer.
    // Their broad errors do not establish a particular rule, so remain uncited.
    for text in ["You may have target creature explore.", "Target creature explores twice.", "Each player explores."] {
        if text.starts_with("Each") {
            crate::support::refused(CardDef::new("Unsupported", "{0}", "Instant", text, None),"text the grammar does not read");
        } else {
            let d=CardDef::new("Unsupported", "{0}", "Instant", text, None).unwrap();
            let reason=if text.starts_with("You may") {"program: action not lowered: Have"} else {"modifier: Times of Keyword"};
            crate::support::refused(game::play::check(&d).map_err(|e|e.0),reason);
        }
    }
}
