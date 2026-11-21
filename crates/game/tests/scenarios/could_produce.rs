//! CR 106.7: current possible output is independent of activation/payment feasibility.
use crate::support::*;
use game::Event;
use ir::{Color, mana::ManaSymbol};

fn symbol(c: Color) -> ManaSymbol { ManaSymbol::Color(c) }
fn activation(source: ObjId) -> Move {
    Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }
}
/// Exercise the fixed six-type mana domain with rule-derived expected outputs. Each
/// submitted answer is independent; the source game is never used to choose one.
fn check_output(g: &Game, source: ObjId, expected: &[ManaSymbol]) {
    let types = [Color::White, Color::Blue, Color::Black, Color::Red, Color::Green]
        .map(symbol).into_iter().chain([ManaSymbol::Colorless]);
    for mana in types {
        let mut branch = g.clone();
        let command = Move::Adding(vec![vec![mana]], Box::new(activation(source)));
        if expected.len() > 1 && expected.contains(&mana) {
            cite!(["106.7", "608.2d"]; ok: branch.act(0, command));
            assert!(branch.obj(source).tapped);
            assert_eq!(branch.players[0].pool.total(), g.players[0].pool.total() + 1);
            match mana {
                ManaSymbol::Color(color) => assert_eq!(branch.players[0].pool.colors[color as usize], g.players[0].pool.colors[color as usize] + 1),
                ManaSymbol::Colorless => assert_eq!(branch.players[0].pool.colorless, g.players[0].pool.colorless + 1),
                _ => unreachable!("the fixed domain contains only mana types"),
            }
        } else {
            let reason = if expected.len() <= 1 {
                "that ability offers no such choice of mana"
            } else {
                "that ability doesn't offer the mana said for it"
            };
            cite!(["608.2d"]; refused(reason): branch.act(0, command));
            assert_eq!(format!("{branch:?}"), format!("{g:?}"));
        }
    }
    if expected.len() <= 1 {
        let mut branch = g.clone();
        branch.act(0, activation(source)).unwrap();
        cite!(["106.5", "106.7"]; assert_eq!(branch.players[0].pool.total(), g.players[0].pool.total() + expected.len() as u32));
        if let Some(ManaSymbol::Color(color)) = expected.first() {
            assert_eq!(branch.players[0].pool.colors[*color as usize], g.players[0].pool.colors[*color as usize] + 1);
        }
    }
}

#[test]
fn r106_7_tapped_lands_unpayable_costs_and_spending_restrictions_do_not_remove_colors() {
    for name in ["Fellwar Stone", "Exotic Orchard"] {
        // Crystal Quarry can produce every color despite its unpaid {5} cost. Ancient
        // Ziggurat's spending restriction does not follow the mana made by our source.
        for land in ["Crystal Quarry", "Ancient Ziggurat"] {
            let mut t = Board::new(2).battlefield(0, &[name]).tapped(1, &[land]).hand(0, &["Opt"]).build();
            let source = t.find_in_battlefield(name);
            let before = t.g.log.end();
            let colors = [Color::White, Color::Blue, Color::Black, Color::Red, Color::Green];
            check_output(&t.g, source, &colors.map(symbol));
            assert_eq!(t.g.log.end(), before);
            assert_eq!(t.g.players[1].pool.total(), 0);
            t.g.act(0, Move::Adding(vec![vec![symbol(Color::Blue)]], Box::new(activation(source)))).unwrap();
            assert_eq!(t.g.players[0].pool.colors[Color::Blue as usize], 1);
            assert!(t.g.players[0].pool.restricted.is_empty());
            let opt = t.find_in(0, Zone::Hand, "Opt");
            t.g.clone().act(0, crate::support::paid("U", Move::Cast { card: opt, targets: vec![] })).unwrap();
        }
    }
}

#[test]
fn r106_5_r605_2_no_defined_color_still_activates_without_an_impossible_choice() {
    for lands in [vec![], vec!["Wastes"], vec!["Exotic Orchard"]] {
        let mut t = Board::new(2).battlefield(0, &["Exotic Orchard"]).battlefield(1, &lands).build();
        let source = t.find_in(0, Zone::Battlefield, "Exotic Orchard");
        check_output(&t.g, source, &[]);
        cite!(["605.2"]; case: format!("{lands:?}"); ok: t.g.act(0, activation(source)));
        assert!(t.g.obj(source).tapped);
        cite!(["106.5"]; case: format!("{lands:?}"); assert_eq!(t.g.players[0].pool.total(), 0));
        assert!(t.g.decision.is_none());
    }
}

#[test]
fn r106_7_rulebook_example_propagates_a_real_color_through_circular_orchards() {
    // Player 0's Forest grounds both Orchards, although only player 1's Orchard is
    // directly named by the ability we activate. Removing that Forest removes green.
    let mut t = Board::new(2).battlefield(0, &["Exotic Orchard", "Forest"])
        .battlefield(1, &["Exotic Orchard"]).build();
    let source = t.find_in(0, Zone::Battlefield, "Exotic Orchard");
    t.g.act(0, activation(source)).unwrap();
    cite!(["106.7"]; assert_eq!(t.g.players[0].pool.colors[Color::Green as usize], 1));
    let forest = t.find_in_battlefield("Forest");
    t.g.move_object(forest, Zone::Graveyard);
    t.g.set_tapped(source, false);
    t.g.act(0, activation(source)).unwrap();
    cite!(["106.7"]; assert_eq!(t.g.players[0].pool.total(), 1, "a former dependency cannot leave a cached color"));
}

#[test]
fn r106_1b_types_include_colorless_but_colors_do_not() {
    let mut t = Board::new(2).battlefield(0, &["Reflecting Pool", "Wastes"]).build();
    let source = t.find_in_battlefield("Reflecting Pool");
    t.g.act(0, activation(source)).unwrap();
    cite!(["106.1b"]; assert_eq!(t.g.players[0].pool.colorless, 1));
}

#[test]
fn r106_7_opponents_lands_supply_choices_without_running_their_followup_actions() {
    let mut t = Board::new(3).battlefield(0, &["Fellwar Stone", "Forest"])
        .tapped(1, &["Underground River"]).battlefield(2, &["Mountain"]).build();
    let source = t.find_in_battlefield("Fellwar Stone");
    check_output(&t.g, source, &[symbol(Color::Blue), symbol(Color::Black), symbol(Color::Red)]);
    for invalid in [symbol(Color::Green), ManaSymbol::Colorless] {
        cite!(["608.2d"]; refused("that ability doesn't offer the mana said for it: say one of what it offers (608.2d)"): t.g.act(0, Move::Adding(vec![vec![invalid]], Box::new(activation(source)))));
        assert!(!t.g.obj(source).tapped);
    }
    t.g.act(0, Move::Adding(vec![vec![symbol(Color::Red)]], Box::new(activation(source)))).unwrap();
    assert_eq!(t.g.players[0].pool.colors[Color::Red as usize], 1);
    cite!(["106.7"]; assert_eq!(t.g.players.iter().map(|p| p.life).collect::<Vec<_>>(), vec![20, 20, 20]));
    assert_eq!(t.g.players[1].pool.total(), 0);
}

#[test]
fn r106_7_conditional_output_and_defined_x_use_the_current_state() {
    let mut t = Board::new(2).battlefield(0, &["Fellwar Stone"])
        .battlefield(1, &["Gemstone Caverns", "Baldur's Gate"]).build();
    let source = t.find_in_battlefield("Fellwar Stone");
    check_output(&t.g, source, &[]);
    let caverns = t.find_in_battlefield("Gemstone Caverns");
    t.g.set_initial_counters(caverns, "luck", 1);
    check_output(&t.g, source, &[Color::White, Color::Blue, Color::Black, Color::Red, Color::Green].map(symbol));
    t.g.set_initial_counters(caverns, "luck", 0);
    let mut with_gate = Board::new(2).battlefield(0, &["Fellwar Stone"])
        .battlefield(1, &["Baldur's Gate", "Azorius Guildgate"]).build();
    let source = with_gate.find_in_battlefield("Fellwar Stone");
    check_output(&with_gate.g, source, &[Color::White, Color::Blue, Color::Black, Color::Red, Color::Green].map(symbol));
    with_gate.g.act(0, Move::Adding(vec![vec![symbol(Color::White)]], Box::new(activation(source)))).unwrap();
    assert_eq!(with_gate.g.players[0].pool.colors[Color::White as usize], 1);
}

#[test]
fn r601_2g_authored_payment_uses_possible_output_without_tapping_opponents_lands() {
    let t = Board::new(2).battlefield(0, &["Fellwar Stone"])
        .tapped(1, &["Island"]).hand(0, &["Opt", "Lightning Bolt"]).build();
    let opt = t.find_in(0, Zone::Hand, "Opt");
    let bolt = t.find_in(0, Zone::Hand, "Lightning Bolt");
    let source = t.find_in_battlefield("Fellwar Stone");
    let plan = vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source, ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None }];
    // Island supplies only blue, which is a forced output rather than a player choice.
    let mut accepted = t.g.clone();
    cite!(["106.7"]; ok: accepted.act(0, crate::support::paid("U", Move::WithMana(plan.clone(), Box::new(Move::Cast { card: opt, targets: vec![] })))));
    let mut refused = t.g.clone();
    // The explicitly produced/spent blue mana cannot pay Lightning Bolt's {R}.
    cite!(["106.7", "601.2g", "601.2h", "118.3a"]; refused("the supplied mana does not pay that cost, all of it and no more"): refused.act(0, crate::support::paid("U", Move::WithMana(plan, Box::new(Move::Cast { card: bolt, targets: vec![vec![Target::Player(1)]] })))));
    assert_eq!(format!("{refused:?}"), format!("{:?}", t.g));
    let island = t.find_in(1, Zone::Battlefield, "Island");
    assert!(accepted.obj(island).tapped);
    assert!(accepted.obj(source).tapped);
    assert_eq!(accepted.players[1].pool.total(), 0);
    assert_eq!(accepted.players[0].pool.total(), 0);
    assert_eq!(accepted.stack.len(), 1);
    assert_eq!(accepted.obj(accepted.stack[0]).chars.name, "Opt");
    cite!(["106.7"]; assert!(!accepted.log.iter().any(|event| matches!(event, Event::Activated { source, .. } if *source == island))));

}

#[test]
fn r106_7_r305_7_effective_abilities_include_grants_and_basic_type_replacement() {
    let mut t = Board::new(2).battlefield(0, &["Fellwar Stone", "Blood Moon"])
        .battlefield(1, &["Underground River"]).build();
    let source = t.find_in_battlefield("Fellwar Stone");
    check_output(&t.g, source, &[symbol(Color::Red)]);
    t.g.act(0, activation(source)).unwrap();
    assert_eq!(t.g.players[0].pool.colors[Color::Red as usize], 1);
    assert_eq!(t.g.players[0].pool.total(), 1);

    let t = Board::new(2).battlefield(0, &["Fellwar Stone"])
        .battlefield(1, &["Wastes", "Chromatic Lantern"]).build();
    check_output(&t.g, t.find_in_battlefield("Fellwar Stone"), &[Color::White, Color::Blue, Color::Black, Color::Red, Color::Green].map(symbol));
}

#[test]
fn r106_7_triggered_production_counts_without_activating_or_triggering_the_land() {
    let mut t = Board::new(2).battlefield(0, &["Fellwar Stone"])
        .battlefield(1, &["Crumbling Vestige"]).build();
    let source = t.find_in_battlefield("Fellwar Stone");
    check_output(&t.g, source, &[Color::White, Color::Blue, Color::Black, Color::Red, Color::Green].map(symbol));
    t.g.act(0, Move::Adding(vec![vec![symbol(Color::White)]], Box::new(activation(source)))).unwrap();
    assert_eq!(t.g.players[0].pool.colors[Color::White as usize], 1);
    assert_eq!(t.g.players[1].pool.total(), 0);
    cite!(["106.7"]; assert!(t.g.stack.is_empty()));
}

#[test]
fn r106_7_cyclic_network_converges_independently_of_battlefield_order() {
    // More than a pair: a propagation algorithm must not enumerate paths around the cycle.
    for reverse in [false, true] {
        let mut board = Board::new(4).battlefield(0, &["Fellwar Stone"]);
        for p in 0..4 { board = board.battlefield(p, &["Exotic Orchard", "Exotic Orchard", "Exotic Orchard"]); }
        let mut t = board.battlefield(0, &["Forest"]).build();
        if reverse { t.g.battlefield.reverse(); }
        let source = t.find_in_battlefield("Fellwar Stone");
        t.g.act(0, activation(source)).unwrap();
        cite!(["106.7"]; case: format!("reverse {reverse}"); assert_eq!(t.g.players[0].pool.colors[Color::Green as usize], 1));
    }
}
