//! The verb an object choice's refusal uses (608.2d): a card put from
//! a hand onto the battlefield is "put", not "returned" — it was never there.
use crate::support::cite;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, Zone};
use ir::Part;

#[test]
fn a_land_put_from_the_hand_onto_the_battlefield_is_put_not_returned() {
    let defs = vec![
        CardDef::new("Spiral", "{0}", "Instant", "You may put a land card from your hand onto the battlefield.", None).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, 1, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g.put(0, 1, Zone::Hand);
    g.put(0, 1, Zone::Hand);
    let bear = g.put(0, 2, Zone::Hand);
    let spiral = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: spiral, targets: vec![] }).unwrap();
    for _ in 0..10 {
        match g.decision.clone() {
            Some(Decision::Choose { player, choice: Choice::OptionalObjects { .. } }) => {
                let said = g.act(player, Move::Choose(Answer::OptionalObjects(Some(vec![bear])))).unwrap_err();
                cite!(["608.2d"]; assert_eq!(said, "a chosen object is not among the offered objects to put (608.2d)"));
                return;
            }
            Some(d) => panic!("{d:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("no object choice");
}

fn object_choice_refusal(text: &str, setup: &dyn Fn(&mut Game) -> game::ObjId) -> String {
    let defs = vec![
        CardDef::new("Spell", "{0}", "Sorcery", text, None).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, 1, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    let wrong = setup(&mut g);
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
    for _ in 0..10 {
        match g.decision.clone() {
            Some(Decision::Choose { player, choice: Choice::May { possible: true } }) => g.act(player, Move::Choose(Answer::Yes)).unwrap(),
            Some(Decision::Choose { player, choice: Choice::Objects { .. } }) => {
                return g.act(player, Move::Choose(Answer::Objects(vec![wrong]))).unwrap_err();
            }
            Some(d) => panic!("{d:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("no object choice");
}

/// Wreck and Rebuild's mode: "you may put a land card from your graveyard onto the battlefield
/// tapped" (here without "may" and "tapped"). The Oracle text puts it; a graveyard origin does
/// not make the action "return".
#[test]
fn a_land_put_from_the_graveyard_onto_the_battlefield_is_put_not_returned() {
    let said = object_choice_refusal("Put a land card from your graveyard onto the battlefield.", &|g| {
        g.put(0, 1, Zone::Graveyard);
        g.put(0, 1, Zone::Graveyard);
        g.put(0, 2, Zone::Graveyard)
    });
    cite!(["608.2d"]; assert_eq!(said, "a chosen object is not among the offered objects to put (608.2d)"));
}

/// The graveyard-to-hand counterpart keeps the Oracle text's "return".
#[test]
fn a_card_returned_from_the_graveyard_to_the_hand_is_returned() {
    let said = object_choice_refusal("Return a creature card from your graveyard to your hand.", &|g| {
        g.put(0, 2, Zone::Graveyard);
        g.put(0, 2, Zone::Graveyard);
        g.put(0, 1, Zone::Graveyard)
    });
    cite!(["608.2d"]; assert_eq!(said, "a chosen object is not among the offered objects to return (608.2d)"));
}

/// Pinned Llanowar Scout Oracle: "{T}: You may put a land card from your hand onto
/// the battlefield." Its 2018-04-27 ruling distinguishes the put from a land play.
fn scout(lands: &[&str]) -> crate::support::T {
    let mut t = crate::support::Board::new(2).battlefield(0, &["Llanowar Scout"])
        .hand(0, lands).build();
    t.activate(0, "Llanowar Scout", 0, vec![], crate::support::mana_pool("")).unwrap().pass_all();
    t
}

#[test]
fn r608_2d_scout_accepts_the_selected_second_card_or_declines_without_selecting() {
    for accept in [true, false] {
        let mut t = scout(&["Forest", "Island"]);
        let forest = t.find_in(0, Zone::Hand, "Forest");
        let island = t.find_in(0, Zone::Hand, "Island");
        cite!(["608.2d", "117.2e"]; assert!(matches!(t.g.asked(), Some((0, game::legal::Asked::OptionalObjects {
            verb: ir::Verb::Move, to: Some(game::state::Placement::Zone(Zone::Battlefield)), among, count: 1,
        })) if among == vec![forest, island])));
        cite!(["402.3"]; assert!(!t.g.sees(1, forest) && !t.g.sees(1, island)));
        let before = format!("{:?}", t.g);
        cite!(["608.2d"]; refused("that decision is another player's"): t.g.act(1, Move::Choose(Answer::OptionalObjects(None))));
        for answer in [Answer::Yes, Answer::OptionalObjects(Some(vec![])), Answer::OptionalObjects(Some(vec![island, island]))] {
            let reason=if matches!(&answer,Answer::Yes) {"decline without objects, or accept with the offered object selection"}
                else {"accept with exactly 1 distinct offered objects"};
            cite!(["608.2d"]; refused(reason): t.g.act(0, Move::Choose(answer)));
            cite!(["608.2d"]; assert_eq!(format!("{:?}", t.g), before));
        }
        t.answer(0, Answer::OptionalObjects(accept.then_some(vec![island]))).unwrap();
        cite!(["117.3b"]; assert_eq!(t.g.asked(), Some((0, game::legal::Asked::Priority))));
        cite!(["608.2c"]; assert!(t.g.stack.is_empty()));
        cite!(["602.2b"]; assert!(t.g.obj(t.find_in_battlefield("Llanowar Scout")).tapped));
        cite!(["305.4"]; assert!(!t.g.players[0].land_played));
        cite!(["608.2d", "400.7"]; assert_eq!(t.g.players[0].hand, if accept { vec![forest] } else { vec![forest, island] }));
        if accept { cite!(["400.7"]; assert_ne!(t.find_in_battlefield("Island"), island)); }
        cite!(["402.3"]; assert!(!t.g.sees(1, forest) && (accept || !t.g.sees(1, island))));
        t.expect().rules(["602.2b", "305.4"]).zone_exact(0, Zone::Battlefield,
            if accept { &["Llanowar Scout", "Island"] } else { &["Llanowar Scout"] }).check();
    }
}

#[test]
fn r608_2d_scout_still_offers_a_unique_card_but_cannot_accept_an_empty_domain() {
    for accept in [true, false] {
        let mut t = scout(&["Forest"]);
        let forest = t.find_in(0, Zone::Hand, "Forest");
        cite!(["608.2d"]; assert!(matches!(t.g.asked(), Some((0, game::legal::Asked::OptionalObjects { among, count: 1, .. })) if among == vec![forest])));
        t.answer(0, Answer::OptionalObjects(accept.then_some(vec![forest]))).unwrap();
        cite!(["608.2d"]; assert_eq!(t.g.players[0].hand.len(), usize::from(!accept)));
        cite!(["117.3b"]; assert_eq!(t.g.asked(), Some((0, game::legal::Asked::Priority))));
    }
    let mut t = scout(&[]);
    cite!(["608.2d", "117.3b"]; assert_eq!(t.g.asked(), Some((0, game::legal::Asked::Priority))));
    cite!(["608.2d"]; assert!(t.g.players[0].hand.is_empty() && t.g.stack.is_empty()));
}

#[test]
fn r608_2d_optional_selected_movement_retains_acceptance_for_if_you_do() {
    let defs = vec![CardDef::new("Optional move", "{0}", "Sorcery",
        "You may put a land card from your hand onto the battlefield. If you do, you gain 1 life.", None).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap()];
    for accept in [true, false] {
        let mut g = Game::new(defs.clone(), 2, 20);
        g.part = Part::PrecombatMain;
        let forest = g.put(0, 1, Zone::Hand);
        let spell = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
        g.act(0, Move::Pass).unwrap(); g.act(1, Move::Pass).unwrap();
        g.act(0, Move::Choose(Answer::OptionalObjects(accept.then_some(vec![forest])))).unwrap();
        cite!(["118.12", "608.2c"]; assert_eq!(g.players[0].life, if accept { 21 } else { 20 }));
    }
}
