//! Endure 701.63: optional counter cost, exact permanent and resolution-time controller.
use crate::support::cite;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
fn card(name: &str, kind: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    let d = CardDef::new(name, "{0}", kind, text, pt).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
    d
}
fn setup(n: u32) -> Game {
    Game::new(
        vec![
            card("Endurer", "Creature", &format!("{{0}}: This creature endures {n}. You gain 1 life."), Some((2, 2))),
            card("Ban", "Enchantment", "Creatures can't have counters put on them.", None),
            card(
                "Plus",
                "Enchantment",
                "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.",
                None,
            ),
            card(
                "Double",
                "Enchantment",
                "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.",
                None,
            ),
            card("Steal", "Instant", "Gain control of target creature.", None),
        ],
        2,
        20,
    )
}
fn pass(g: &mut Game) {
    for _ in 0..2 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn activate(g: &mut Game, s: ObjId) {
    g.act(0, Move::Activate { source: s, ability: 0, targets: vec![], paying: vec![] }).unwrap();
}
fn pt(g: &Game, o: ObjId) -> Option<(i32, i32)> {
    Some((g.power(o)?, g.toughness(o)?))
}
fn tokens(g: &Game) -> Vec<ObjId> {
    g.log
        .iter()
        .filter_map(|e| match e {
            Event::Created { token } => Some(*token),
            _ => None,
        })
        .collect()
}
#[test]
fn r701_63a_optional_counter_cost_or_copiable_sized_spirit() {
    for pay in [false, true] {
        let mut g = setup(12);
        let s = g.put(0, 0, Zone::Battlefield);
        activate(&mut g, s);
        pass(&mut g);
        cite!(["701.63a","118.12a"];assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::Alternative{optional:true,..}}))));
        g.act(0, Move::Choose(if pay { Answer::Alternative(0) } else { Answer::No })).unwrap();
        cite!(["701.63a"];assert_eq!(g.obj(s).counters.get("+1/+1").copied().unwrap_or(0),if pay{12}else{0}));
        let made = tokens(&g);
        cite!(["701.63a"];assert_eq!(made.len(),usize::from(!pay)));
        if !pay {
            let t = made[0];
            assert_eq!(g.obj(t).chars.name, "Spirit Token");
            assert_eq!(g.obj(t).chars.colors, vec![ir::Color::White]);
            assert_eq!(pt(&g, t), Some((12, 12)));
            assert!(g.obj(t).counters.is_empty());
        }
        cite!(["608.2c"];assert_eq!(g.players[0].life,21));
    }
}
#[test]
fn r701_63b_zero_has_no_cost_choice_or_token() {
    let mut g = setup(0);
    let s = g.put(0, 0, Zone::Battlefield);
    g.put(0, 1, Zone::Battlefield);
    activate(&mut g, s);
    pass(&mut g);
    cite!(["701.63b"];assert!(g.decision.is_none()&&tokens(&g).is_empty()&&g.obj(s).counters.is_empty()));
    assert_eq!(g.players[0].life, 21);
}
#[test]
fn r701_63a_forbidden_counter_cost_forces_spirit_without_replacement_question() {
    let mut g = setup(2);
    let s = g.put(0, 0, Zone::Battlefield);
    for d in [1, 2, 3] {
        g.put(0, d, Zone::Battlefield);
    }
    activate(&mut g, s);
    pass(&mut g);
    cite!(["101.2","118.3","701.63a"];assert!(g.decision.is_none()));
    assert!(g.obj(s).counters.is_empty());
    assert_eq!(tokens(&g).len(), 1);
    assert_eq!(pt(&g, tokens(&g)[0]), Some((2, 2)));
}
#[test]
fn r118_11_replaced_endure_payment_resumes_once_and_suppresses_token() {
    let mut g = setup(2);
    let s = g.put(0, 0, Zone::Battlefield);
    let plus = g.put(0, 2, Zone::Battlefield);
    g.put(0, 3, Zone::Battlefield);
    activate(&mut g, s);
    pass(&mut g);
    g.act(0, Move::Choose(Answer::Alternative(0))).unwrap();
    cite!(["616.1"];assert!(matches!(g.decision,Some(Decision::Choose{choice:Choice::Replacement{..},..}))));
    for _ in 0..2 {
        if let Some(Decision::Choose { player, choice: Choice::Replacement { among, .. } }) = g.decision.clone() {
            g.act(player, Move::Choose(Answer::Replacement(among.iter().find(|id| id.source == plus).unwrap().clone())))
                .unwrap();
        }
    }
    cite!(["118.11","701.63a"];assert_eq!(g.obj(s).counters.get("+1/+1"),Some(&6)));
    assert!(tokens(&g).is_empty());
    assert_eq!(g.players[0].life, 21);
}
#[test]
fn r608_2h_endure_chooser_is_current_controller_not_ability_controller() {
    let mut g = setup(2);
    let s = g.put(0, 0, Zone::Battlefield);
    activate(&mut g, s);
    g.act(0, Move::Pass).unwrap();
    let steal = g.put(1, 4, Zone::Hand);
    g.act(1, Move::Cast { card: steal, targets: vec![vec![Target::Object(s)]] }).unwrap();
    pass(&mut g);
    pass(&mut g);
    cite!(["701.63a","608.2h"];assert!(matches!(g.decision,Some(Decision::Choose{player:1,..}))));
    g.act(1, Move::Choose(Answer::No)).unwrap();
    assert_eq!(g.controller(tokens(&g)[0]), 1);
}
#[test]
fn r400_7_departure_reentry_does_not_receive_old_endure_counters() {
    let mut g = setup(3);
    let s = g.put(0, 0, Zone::Battlefield);
    activate(&mut g, s);
    g.move_object(s, Zone::Graveyard);
    let grave = g.players[0].graveyard[0];
    g.move_object(grave, Zone::Battlefield);
    let fresh = g.battlefield[0];
    pass(&mut g);
    cite!(["400.7","701.63a"];assert_ne!(fresh,s));
    assert!(g.obj(fresh).counters.is_empty());
    assert!(g.decision.is_none());
    assert_eq!(tokens(&g).len(), 1);
}
#[test]
fn grouped_endure_reference_is_refused_instead_of_taking_first_member() {
    let d =
        CardDef::new("Grouped", "{0}", "Sorcery", "Create two 1/1 white Spirit creature tokens. They endure 2.", None)
            .unwrap();
    crate::support::refused(game::play::check(&d).map_err(|e|e.0),"input reference: nothing before it supplies the required participant/cardinality");
}
#[test]
fn native_fortress_cast_entry_endures_with_normal_cost_and_token_choice() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let d = CardDef::from_faces(&catalog["Fortress Kin-Guard"]).unwrap();
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d], 2, 20);
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[0] = 2;
    g.act(0, crate::support::paid("WW", Move::Cast { card, targets: vec![] })).unwrap();
    pass(&mut g);
    pass(&mut g);
    g.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["701.63a","603.3"];assert_eq!(pt(&g,tokens(&g)[0]),Some((1,1))));
    assert_eq!(g.battlefield.len(), 2);
}
#[test]
fn r608_2h_departed_endurer_uses_last_battlefield_controller() {
    let mut g = setup(2);
    let s = g.put(0, 0, Zone::Battlefield);
    activate(&mut g, s);
    g.act(0, Move::Pass).unwrap();
    let steal = g.put(1, 4, Zone::Hand);
    g.act(1, Move::Cast { card: steal, targets: vec![vec![Target::Object(s)]] }).unwrap();
    pass(&mut g);
    assert_eq!(g.controller(s), 1);
    g.move_object(s, Zone::Graveyard);
    pass(&mut g);
    cite!(["608.2h","701.63a"];assert!(g.decision.is_none()));
    assert_eq!(tokens(&g).len(), 1);
    assert_eq!(g.controller(tokens(&g)[0]), 1);
}
#[test]
fn r701_63a_noncreature_permanent_can_pay_the_counter_cost() {
    let d = card("Relic", "Artifact", "{0}: This permanent endures 2.", None);
    let mut g = Game::new(vec![d], 2, 20);
    let s = g.put(0, 0, Zone::Battlefield);
    activate(&mut g, s);
    pass(&mut g);
    g.act(0, Move::Choose(Answer::Alternative(0))).unwrap();
    cite!(["701.63a","122.1"];assert_eq!(g.obj(s).counters.get("+1/+1"),Some(&2)));
    assert!(tokens(&g).is_empty());
}
#[test]
fn native_warden_counts_all_counter_kinds_on_resolution_and_freezes_choice_amount() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let d = CardDef::from_faces(&catalog["Warden of the Grove"]).unwrap();
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d, card("Bear", "Creature", "", Some((2, 2)))], 2, 20);
    let w = g.put(0, 0, Zone::Battlefield);
    let bear = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card: bear, targets: vec![] }).unwrap();
    pass(&mut g);
    g.put_counters(w, "charge", 2);
    g.put_counters(w, "+1/+1", 1);
    pass(&mut g);
    let Some(Decision::Choose { choice: Choice::Alternative { among, .. }, .. }) = &g.decision else {
        panic!("{:?}", g.decision)
    };
    cite!(["701.63a","608.2h"];assert!(matches!(among[0].body,game::state::ActionOptionBody::Counters{count:3,..})));
    // A pending decision admits no priority. This direct mutation only distinguishes the
    // retained amount from recomputing it when the validated answer resumes execution.
    g.remove_counters(w, "charge", 2);
    g.act(0, Move::Choose(Answer::No)).unwrap();
    assert_eq!(pt(&g, tokens(&g)[0]), Some((3, 3)));
}
#[test]
fn actual_endure_consumer_admission_diagnostic() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    for name in [
        "Warden of the Grove",
        "Dusyut Earthcarver",
        "Anafenza, Unyielding Lineage",
        "Krumar Initiate",
        "Sinkhole Surveyor",
        "Kin-Tree Nurturer",
        "Descendant of Storms",
        "Inspirited Vanguard",
        "Sandskitter Outrider",
        "Fortress Kin-Guard",
    ] {
        let d = CardDef::from_faces(&catalog[name]).unwrap();
        game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
    }
}
#[test]
fn r609_3_empty_optional_target_group_does_not_endure_or_create_fallback() {
    let d = card("Optional", "Sorcery", "Tap up to one target creature. It endures 2.", None);
    let mut g = Game::new(vec![d], 2, 20);
    let s = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: s, targets: vec![vec![]] }).unwrap();
    pass(&mut g);
    cite!(["609.3","701.63a"];assert!(tokens(&g).is_empty()));
    assert!(g.decision.is_none());
}
#[test]
fn native_krumar_announced_x_paid_life_and_tap_determine_endure_amount() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let d = CardDef::from_faces(&catalog["Krumar Initiate"]).unwrap();
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d], 2, 20);
    let s = g.put(0, 0, Zone::Battlefield);
    g.turn = 2;
    g.players[0].pool.colorless = 4;
    g.players[0].pool.colors[2] = 1;
    g.act(0, crate::support::paid("CCCCB", Move::WithX(4, Box::new(Move::Activate { source: s, ability: 0, targets: vec![], paying: vec![] }))))
        .unwrap();
    cite!(["107.3","602.2b"];assert_eq!(g.players[0].life,16));
    assert!(g.obj(s).tapped);
    pass(&mut g);
    g.act(0, Move::Choose(Answer::Alternative(0))).unwrap();
    cite!(["701.63a"];assert_eq!(g.obj(s).counters.get("+1/+1"),Some(&4)));
}
