//! Continuous private library-top access and ordinary announcement/payment composition.
use crate::support::{cite,paid,mana_pool};
use game::announcement::{CastCost, CastRoute};
use game::legal::Step;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, Zone};
use ir::Part;
use std::sync::OnceLock;
fn full(name: &str) -> CardDef {
    static FS: OnceLock<Vec<oracle::corpus::Face>> = OnceLock::new();
    let fs = FS.get_or_init(|| oracle::corpus::load(&oracle::corpus::default_path()).unwrap());
    let cat = CardDef::catalog(fs);
    let d = CardDef::from_faces(&cat[name]).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
    d
}
fn card(n: &str, c: &str, k: &str, t: &str) -> CardDef {
    let d = CardDef::new(n, c, k, t, k.contains("Creature").then_some((1, 1))).unwrap();
    game::play::check(&d).unwrap();
    d
}
fn setup(mut extras: Vec<CardDef>) -> (Game, ObjId) {
    let mut defs = vec![
        full("Bolas's Citadel"),
        card("Island", "", "Basic Land — Island", "{T}: Add {U}."),
        card("Spell", "{2}{U}", "Instant", "You gain 1 life."),
    ];
    defs.append(&mut extras);
    let mut g = Game::new(defs, 3, 20);
    for p in 0..3 {
        for _ in 0..12 {
            g.put(p, 1, Zone::Library);
        }
    }
    let citadel = g.put(0, 0, Zone::Battlefield);
    g.part = Part::PrecombatMain;
    (g, citadel)
}
fn on_top(g: &mut Game, def: usize) -> ObjId {
    g.put(0, def, Zone::Library)
}
fn pass(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn top_cast(source: ObjId, card: ObjId) -> Move {
    Move::CastVia(CastRoute::Permission{source,ability:1},Box::new(cast(card)))
}
fn cast(card: ObjId) -> Move {
    Move::Cast { card, targets: vec![] }
}
#[test]
fn r401_5_private_access_is_continuous_changes_with_top_and_effective_source() {
    let (mut g, c) = setup(vec![]);
    let top = g.top_cards(0, 1)[0];
    assert!(g.sees(0, top));
    for p in [1, 2] {
        cite!(["401.2"]; case: format!("player {p}"); assert!(!g.sees(p, top)));
        assert_eq!(game::snapshot::Snapshot::of(&g, Some(p)).players[0].top_card, None);
    }
    assert_eq!(game::snapshot::Snapshot::of(&g, Some(0)).players[0].top_card, Some(top));
    let other = g.top_cards(0, 2)[1];
    assert!(!g.sees(0, other));
    let n = g.players[0].library.len();
    g.players[0].library.swap(n - 1, n - 2);
    cite!(["611.3a"]; assert!(g.sees(0, other) && !g.sees(0, top)));
    g.put(0, 0, Zone::Battlefield);
    g.move_object(c, Zone::Graveyard);
    assert!(g.sees(0, other));
    g.players[0].library.clear();
    assert_eq!(game::snapshot::Snapshot::of(&g, Some(0)).players[0].top_card, None);
}
#[test]
fn r601_2f_top_spell_requires_life_not_mana_and_only_current_top_is_permitted() {
    let (mut g, c) = setup(vec![]);
    let citadel=c;
    let top = on_top(&mut g, 2);
    let lower = g.top_cards(0, 2)[1];
    cite!(["601.3"]; assert!(g.clone().act(g.priority, top_cast(citadel,top)).is_ok()));
    cite!(["601.3"]; refused("that card is not in your hand: it is in a library, and no rule or effect lets you cast it from there (601.3)"): g.act(0, cast(lower)));
    cite!(["601.3"]; refused("that card is not in your hand: it is in a library, and no rule or effect lets you cast it from there (601.3)"): g.act(0, Move::CastVia(CastRoute::Normal, Box::new(cast(top)))));
    g.act(0, Move::CastVia(CastRoute::Permission { source: c, ability: 1 }, Box::new(cast(top)))).unwrap();
    cite!(["118.9", "119.4"]; assert_eq!(g.players[0].life, 17));
    cite!(["401.5"]; assert!(g.sees(0, lower)));
    assert!(!g.sees(1, lower));
    pass(&mut g);
    assert_eq!(g.players[0].life, 18);
}
#[test]
fn r118_9_forced_life_rejects_other_alternatives_and_nonzero_x() {
    let alt = card(
        "Alternate",
        "{5}",
        "Instant",
        "You may pay 2 life rather than pay this spell's mana cost.\nYou gain 1 life.",
    );
    let x = card("Variable", "{X}{U}", "Instant", "You draw X cards.");
    let (mut g, citadel) = setup(vec![alt, x]);
    let top = on_top(&mut g, 3);
    cite!(["118.9a"]; refused("only one alternative cost is paid for a spell (118.9a)"): g.act(0, Move::CastFor(CastCost::Alternative(0), Box::new(top_cast(citadel,top)))));
    g.act(0, top_cast(citadel,top)).unwrap();
    cite!(["118.9b"]; assert_eq!(g.players[0].life, 15));
    pass(&mut g);
    let top = on_top(&mut g, 4);
    cite!(["107.3b"]; refused("X is 0 for a spell cast without paying its mana cost (107.3b)"): g.act(0, Move::WithX(2, Box::new(top_cast(citadel,top)))));
    g.act(0, Move::WithX(0, Box::new(top_cast(citadel,top)))).unwrap();
    assert_eq!(g.players[0].life, 15);
    cite!(["107.3b"]; assert_eq!(g.obj(g.stack[0]).x, Some(0)));
}
#[test]
fn r119_4_affordability_and_additional_costs_and_trinisphere_remain() {
    let extra = card(
        "Extra",
        "{1}{U}",
        "Instant",
        "As an additional cost to cast this spell, sacrifice an artifact.\nYou gain 1 life.",
    );
    let (mut g, c) = setup(vec![extra, full("Trinisphere")]);
    let citadel=c;
    let top = on_top(&mut g, 3);
    cite!(["601.2h"]; refused("that selection does not satisfy the cost, which asks for exactly 1 permanent(s): 0 chosen (601.2h, 701.21a)"): g.act(0, top_cast(citadel,top)));
    g.act(0, Move::WithCosts(vec![vec![c]], Box::new(top_cast(citadel,top)))).unwrap();
    cite!(["118.9d"]; assert_eq!(g.players[0].life, 18));
    assert_eq!(g.obj(c).zone, Zone::Gone);
    pass(&mut g);
    let (mut g, citadel) = setup(vec![full("Trinisphere")]);
    g.put(0, 3, Zone::Battlefield);
    let top = on_top(&mut g, 2);
    // The life alternative still owes Trinisphere's three mana: two supplied C are short.
    g.players[0].pool.colorless = 2;
    let before = format!("{g:?}");
    cite!(["118.9d", "601.2h"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0, paid("CC",top_cast(citadel,top))));
    assert_eq!(format!("{g:?}"), before);
    g.players[0].pool.colorless = 3;
    g.players[0].life = 2;
    assert!(!g.clone().act(g.priority, paid("CCC",top_cast(citadel,top))).is_ok());
    cite!(["119.4"]; refused("you can't pay 3 life with a life total of 2 (119.4)"): g.act(0, paid("CCC",top_cast(citadel,top))));
    g.players[0].life = 4;
    cite!(["119.4"]; assert!(g.clone().act(g.priority, paid("CCC",top_cast(citadel,top))).is_ok()));
    g.act(0, paid("CCC",top_cast(citadel,top))).unwrap();
    cite!(["119.4"]; assert_eq!(g.players[0].life, 1));
    cite!(["118.9d"]; assert_eq!(g.players[0].pool.total(), 0));
}
#[test]
fn r712_11_top_modal_face_uses_its_own_cost_and_land_limits() {
    let (mut g, _) = setup(vec![full("Sink into Stupor"), full("Teferi, Time Raveler")]);
    let top = on_top(&mut g, 3);
    cite!(["712.12"]; ok: g.act(0, Move::PlayLandAs { card: top, face: 1 }));
    assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::May { .. }, .. })));
    let clone = g.clone();
    for mut g in [g, clone] {
        g.act(0, Move::Choose(Answer::No)).unwrap();
        assert!(g.players[0].land_played);
        let second = g.top_cards(0, 1)[0];
        cite!(["305.2"]; refused("you have played a land this turn (305.2)"): g.act(0, Move::PlayLand(second)));
        cite!(["401.5"]; assert!(g.sees(0, second)));
    }
    let back = card("Back spell", "{1}", "Instant", "You gain 2 life.");
    let front = card("Front spell", "{7}", "Instant", "You gain 1 life.");
    let modal = CardDef::modal(front, back).unwrap();
    let (mut g, citadel) = setup(vec![modal]);
    let top = on_top(&mut g, 3);
    g.act(0, Move::CastAs(game::card::CastAs::Face(1), Box::new(top_cast(citadel,top)))).unwrap();
    cite!(["712.11b"]; assert_eq!(g.players[0].life, 19));
    cite!(["712.11c"]; assert_eq!(g.obj(g.stack[0]).chars.mana_value, 1));
    pass(&mut g);
    assert_eq!(g.players[0].life, 21);
}
#[test]
fn r307_5_top_permission_obeys_normal_timing_and_teferi() {
    let (mut g, citadel) = setup(vec![full("Teferi, Time Raveler"), card("Slow", "{1}", "Sorcery", "You gain 1 life.")]);
    let top = on_top(&mut g, 4);
    g.part = Part::Upkeep;
    cite!(["117.1a"]; assert!(!g.clone().act(g.priority, top_cast(citadel,top)).is_ok()));
    cite!(["117.1a"]; refused("that is cast in a main phase of your own turn with the stack empty (117.1a)"): g.act(0, top_cast(citadel,top)));
    let top = on_top(&mut g, 2);
    cite!(["117.1a"]; assert!(g.clone().act(g.priority, top_cast(citadel,top)).is_ok()));
    g.put(1, 3, Zone::Battlefield);
    cite!(["307.5"]; assert!(!g.clone().act(g.priority, top_cast(citadel,top)).is_ok()));
    cite!(["307.5"]; refused("an effect permits casting spells only at sorcery timing (307.5, 601.3)"): g.act(0, top_cast(citadel,top)));
    g.part = Part::PrecombatMain;
    cite!(["307.5"]; assert!(g.clone().act(g.priority, top_cast(citadel,top)).is_ok()));
}
#[test]
fn r602_2_ten_sacrifices_include_source_without_partial_invalid_payment() {
    let (mut g, c) = setup(vec![]);
    let mut chosen = vec![c];
    for _ in 0..9 {
        chosen.push(g.put(0, 0, Zone::Battlefield));
    }
    let land = g.put(0, 1, Zone::Battlefield);
    for (bad, why) in [(chosen[..9].to_vec(), "that selection does not satisfy the cost, which asks for exactly 10 permanent(s): 9 chosen (601.2h, 701.21a)"),
        (vec![c; 10], "a card is chosen once for that cost (601.2h)"), ({
        let mut a = chosen.clone();
        a[9] = land;
        a
    }, "Island is not what that cost asks for or cannot pay it (701.21a, 118.3)")] {
        let before = format!("{g:?}");
        cite!(["601.2h"]; case: why; refused(why): g.act(0, Move::Activate { source: c, ability: 2, targets: vec![], paying: vec![bad] }));
        assert_eq!(format!("{g:?}"), before);
    }
    g.act(0, Move::Activate { source: c, ability: 2, targets: vec![], paying: vec![chosen.clone()] }).unwrap();
    // CR404.3: the owner explicitly orders these simultaneous graveyard arrivals.
    assert_eq!(g.decision, Some(game::state::Decision::Choose { player: 0, choice: game::state::Choice::OrderCards { to: game::state::Placement::Zone(Zone::Graveyard), cards: chosen.clone() } }));
    g.act(0, Move::Choose(game::state::Answer::Objects(chosen.iter().rev().copied().collect()))).unwrap();
    cite!(["601.2h", "602.2b"]; assert!(chosen.iter().all(|o| g.obj(*o).zone == Zone::Gone)));
    cite!(["611.3b"]; assert!(!g.sees(0, g.top_cards(0, 1)[0])));
    pass(&mut g);
    assert_eq!(g.players.iter().map(|p| p.life).collect::<Vec<_>>(), vec![20, 10, 10]);
}
#[test]
fn r401_5_library_origin_suspended_cast_hides_new_top_and_reverses_without_replay() {
    let mana = card(
        "Mana producer",
        "{0}",
        "Artifact Creature",
        "{T}: You gain 1 life. Put a +1/+1 counter on this creature. Add {C}{C}{C}.",
    );
    let creature = CardDef::new("Creature", "{0}", "Creature", "", Some((1, 1))).unwrap();
    let scales = card(
        "Scales",
        "{0}",
        "Enchantment",
        "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.",
    );
    let double = card(
        "Double",
        "{0}",
        "Enchantment",
        "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.",
    );
    let (mut g, citadel) = setup(vec![mana, creature, scales, double, full("Trinisphere")]);
    let m = g.put(0, 3, Zone::Battlefield);
    let _bear = g.put(0, 4, Zone::Battlefield);
    g.put(0, 5, Zone::Battlefield);
    g.put(0, 6, Zone::Battlefield);
    g.put(0, 7, Zone::Battlefield);
    let top = on_top(&mut g, 2);
    let next = g.top_cards(0, 2)[1];
    let step = Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source: m, ability: 0, paying: vec![], adding: vec![] };
    g.act(0, Move::WithMana(vec![step.clone(), step], Box::new(top_cast(citadel,top)))).unwrap();
    assert!(g.decision.is_some());
    cite!(["401.5"]; assert!(!g.sees(0, next)));
    assert_eq!(game::snapshot::Snapshot::of(&g, Some(0)).players[0].top_card, None);
    let Some(Decision::Choose { choice: Choice::Replacement { among, .. }, .. }) = &g.decision else {
        panic!("{:?}", g.decision)
    };
    let replacement = among[0].clone();
    let clone = g.clone();
    for mut g in [g, clone] {
        g.act(0, Move::Choose(Answer::Replacement(replacement.clone()))).unwrap();
        assert!(matches!(g.decision, Some(Decision::Payment { .. })));
        assert!(!g.sees(0, next));
        let mut completed = g.clone();
        completed.act(0, Move::FinishPayment { plan: vec![], paying: vec![], with: Some(mana_pool("CCC")) }).unwrap();
        cite!(["401.5", "601.2i"]; assert!(completed.sees(0, next)));
        assert_eq!(completed.players[0].life, 18);
        assert_eq!(completed.log.iter().filter(|e| matches!(e, Event::Cast { .. })).count(), 1);
        crate::support::refused(g.act(1, Move::ReversePayment), "that decision is another player's");
        g.act(0, Move::ReversePayment).unwrap();
        assert_eq!(g.top_cards(0, 1)[0], top);
        assert!(g.sees(0, top) && !g.sees(1, top));
        assert!(!g.sees(0, next));
        assert_eq!(g.players[0].life, 21);
        assert!(g.obj(m).tapped);
        assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Cast { .. })).count(), 0);
    }
}
#[test]
fn unsupported_library_permission_qualifiers_are_refused() {
    for text in [
        "You may look at the top two cards of your library any time.",
        "You may look at the top card of target player's library any time.",
    ] {
        let d = CardDef::new("Unsupported", "{0}", "Artifact", text, None).unwrap();
        crate::support::refused(game::play::check(&d).map_err(|e|e.0),"library look: your single top card");
    }
}
#[test]
fn r401_5_hand_cast_and_activation_keep_unchanged_top_access_during_mana_choice() {
    for activated in [false, true] {
        let producer = card(
            "Producer",
            "{0}",
            "Artifact Creature",
            "{T}: Put a +1/+1 counter on this creature. Add X mana of any one color, where X is the number of +1/+1 counters on this creature.",
        );
        let consumer = card("Consumer", "{0}", "Artifact", "{1}, {T}: You gain 1 life.");
        let (mut g, _) = setup(vec![producer, consumer, full("Hardened Scales"), full("Branching Evolution")]);
        g.put(0, 5, Zone::Battlefield);
        g.put(0, 6, Zone::Battlefield);
        let source = g.put(0, 3, Zone::Battlefield);
        let old = g.top_cards(0, 1)[0];
        let next = g.top_cards(0, 2)[1];
        let action = if activated {
            let source = g.put(0, 4, Zone::Battlefield);
            Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }
        } else {
            let c = g.put(0, 2, Zone::Hand);
            g.players[0].pool.colorless = 2;
            cast(c)
        };
        g.act(0, Move::WithMana(vec![Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source, ability: 0, paying: vec![], adding: vec![] }], Box::new(paid(if activated {"U"}else{"CCU"},action))))
            .unwrap();
        let Some(Decision::Choose { choice: Choice::Replacement { among, .. }, .. }) = &g.decision else {
            panic!("{:?}", g.decision)
        };
        let replacement = among[0].clone();
        assert!(g.sees(0, old));
        assert!(!g.sees(0, next));
        assert!(!g.sees(1, next));
        let clone = g.clone();
        for mut g in [g, clone] {
            g.act(0, Move::Choose(Answer::Replacement(replacement.clone()))).unwrap();
            assert!(g.sees(0, old));
            let n = g.obj(source).counters["+1/+1"] as usize;
            g.act(0, Move::Choose(Answer::Mana(vec![ir::mana::ManaSymbol::Color(ir::Color::Blue); n]))).unwrap();
            assert!(g.decision.is_none());
            cite!(["611.3a"]; case: format!("activated {activated}"); assert!(g.sees(0, old)));
            assert!(!g.sees(0, next));
        }
    }
}
#[test]
fn r611_3_effective_ability_removal_and_changed_controller_end_private_access() {
    let removal = card("Removal", "{0}", "Instant", "Target artifact loses all abilities until end of turn.");
    let steal = card("Steal", "{0}", "Instant", "Gain control of target artifact.");
    let (mut g, c) = setup(vec![removal, steal]);
    let top = g.top_cards(0, 1)[0];
    let s = g.put(0, 3, Zone::Hand);
    g.act(0, Move::Cast { card: s, targets: vec![vec![game::Target::Object(c)]] }).unwrap();
    pass(&mut g);
    cite!(["604.2"]; assert!(!g.sees(0, top)));
    let s = g.put(0, 4, Zone::Hand);
    g.act(0, Move::Cast { card: s, targets: vec![vec![game::Target::Object(c)]] }).unwrap();
    pass(&mut g);
    assert!(!g.sees(0, top));
    let (mut g, c) = setup(vec![card("Steal", "{0}", "Instant", "Gain control of target artifact.")]);
    let s = g.put(1, 3, Zone::Hand);
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Cast { card: s, targets: vec![vec![game::Target::Object(c)]] }).unwrap();
    pass(&mut g);
    assert_eq!(g.controller(c), 1);
    cite!(["109.5"]; assert!(!g.sees(0, g.top_cards(0, 1)[0])));
    cite!(["109.5"]; assert!(g.sees(1, g.top_cards(1, 1)[0])));
}
#[test]
fn r118_9_zero_mana_value_and_no_mana_cost_can_use_the_permission() {
    let (mut g, citadel) = setup(vec![card("No cost", "", "Instant", "You gain 1 life.")]);
    let c = on_top(&mut g, 3);
    assert!(g.clone().act(g.priority, top_cast(citadel,c)).is_ok());
    cite!(["118.6a"]; ok: g.act(0, top_cast(citadel,c)));
    cite!(["202.3a"]; assert_eq!(g.players[0].life, 20));
    pass(&mut g);
    assert_eq!(g.players[0].life, 21);
}
