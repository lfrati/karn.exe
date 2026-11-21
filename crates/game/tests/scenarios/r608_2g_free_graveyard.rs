//! Free casting keeps the targeted source incarnation/zone and ordinary additional costs.
use crate::support::{cite, paid, mana_pool};
use game::legal::Asked;
use game::state::{Answer, Choice, Decision, DestinationEffect};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use std::sync::OnceLock;
const SCHOLAR: usize = 0;
const UNSUMMON: usize = 1;
const DRAW: usize = 2;
const ARTIFACT: usize = 3;
const CAPSIZE: usize = 4;
const RUDE: usize = 5;
const ISLAND: usize = 6;
const BEAR: usize = 7;
fn full(n: &str) -> CardDef {
    static F: OnceLock<Vec<oracle::corpus::Face>> = OnceLock::new();
    let f = F.get_or_init(|| oracle::corpus::load(&oracle::corpus::default_path()).unwrap());
    let d = CardDef::from_faces(&CardDef::catalog(f)[n]).unwrap();
    game::play::check(&d).unwrap();
    d
}
fn pass(g: &mut Game) {
    for _ in 0..2 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn at_targets(kind: usize) -> (Game, ObjId) {
    let mut defs = [
        "Scholar of the Lost Trove",
        "Unsummon",
        "Divination",
        "Spellbook",
        "Capsize",
        "Rude Awakening",
        "Island",
        "Grizzly Bears",
    ]
    .iter()
    .map(|n| full(n))
    .collect::<Vec<_>>();
    let mana=CardDef::new("Mana choice","{0}","Creature — Elf","{T}: Put a +1/+1 counter on this creature. Add X mana of any one color, where X is the number of +1/+1 counters on this creature.",Some((1,1))).unwrap();
    game::play::check(&mana).unwrap();
    defs.push(mana);
    for (n, text) in [
        (
            "Scales",
            "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.",
        ),
        (
            "Evolution",
            "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.",
        ),
        ("Tax", "Spells cost {1} more to cast."),
    ] {
        let d = CardDef::new(n, "{0}", "Enchantment", text, None).unwrap();
        game::play::check(&d).unwrap();
        defs.push(d);
    }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 {
        for _ in 0..8 {
            g.put(p, ISLAND, Zone::Library);
        }
    }
    let target = g.put(0, kind, Zone::Graveyard);
    let scholar = g.put(0, SCHOLAR, Zone::Hand);
    g.players[0].pool.colors[ir::Color::Blue as usize] = 7;
    g.act(0, paid("UUUUUUU", Move::Cast { card: scholar, targets: vec![] })).unwrap();
    pass(&mut g);
    assert!(matches!(g.decision, Some(Decision::Targets { .. })));
    (g, target)
}
fn offer(kind: usize) -> (Game, ObjId) {
    let (mut g, card) = at_targets(kind);
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(card)]])).unwrap();
    pass(&mut g);
    let Some((0, Asked::Cast { cards, .. })) = g.asked() else { panic!("{:?}", g.decision) };
    assert_eq!(cards, vec![card]);
    (g, card)
}
#[test]
fn full_scholar_free_offer_and_clone_keep_graveyard_rider_and_artifact_destination() {
    for kind in [UNSUMMON, DRAW, ARTIFACT] {
        let (mut g, card) = offer(kind);
        assert_eq!(g.obj(card).zone, Zone::Graveyard);
        let bear = g.put(1, BEAR, Zone::Battlefield);
        let copy = g.clone();
        for mut g in [g, copy] {
            let targets = if kind == UNSUMMON { vec![vec![Target::Object(bear)]] } else { vec![] };
            cite!(["608.2g", "118.9"]; ok: g.act(0, Move::Cast { card, targets }));
            assert_eq!(g.players[0].pool.total(), 0);
            pass(&mut g);
            assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Cast { .. })).count(), 2);
            if kind == ARTIFACT {
                assert!(g.battlefield.iter().any(|o| g.obj(*o).card == Some(ARTIFACT)));
                assert!(g.exile.is_empty());
            } else {
                assert_eq!(g.exile.len(), 1);
                assert_eq!(g.obj(g.exile[0]).card, Some(kind));
            }
            if kind == DRAW {
                assert_eq!(g.players[0].hand.len(), 2);
            }
            if kind == UNSUMMON {
                assert_eq!(g.players[1].hand.len(), 1);
            }
        }
    }
}
#[test]
fn declining_revokes_permission_and_opponent_graveyard_is_not_a_target() {
    let (mut g, card) = at_targets(DRAW);
    let theirs = g.put(1, DRAW, Zone::Graveyard);
    let before = format!("{g:?}");
    cite!(["601.2c", "115.1"]; refused("an illegal target, Divination: it is in an opponent's graveyard, not yours (115.1)"): g.act(0, Move::ChooseTargets(vec![vec![Target::Object(theirs)]])));
    assert_eq!(format!("{g:?}"), before);
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(card)]])).unwrap();
    pass(&mut g);
    g.act(0, Move::Choose(Answer::No)).unwrap();
    assert_eq!(g.obj(card).zone, Zone::Graveyard);
    assert!(g.stack.is_empty());
    cite!(["601.3"]; refused("that card is not in your hand: it is in a graveyard, and no rule or effect lets you cast it from there (601.3)"): g.act(0, Move::Cast { card, targets: vec![] }));
}
#[test]
fn changed_graveyard_incarnation_is_not_offered() {
    let (mut g, card) = at_targets(DRAW);
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(card)]])).unwrap();
    g.move_object(card, Zone::Exile);
    let ex = g.exile[0];
    g.move_object(ex, Zone::Graveyard);
    let new = g.players[0].graveyard[0];
    assert_ne!(card, new);
    pass(&mut g);
    assert!(g.stack.is_empty());
    cite!(["608.2b", "400.7"]; assert!(g.decision.is_none()));
    assert_eq!(g.obj(new).zone, Zone::Graveyard);
}
#[test]
fn free_graveyard_cast_keeps_buyback_cost_and_completion_choice() {
    for effect in [DestinationEffect::Buyback, DestinationEffect::CastRider] {
        let (mut g, card) = offer(CAPSIZE);
        let bear = g.put(1, BEAR, Zone::Battlefield);
        let mv = Move::WithBuyback(true, Box::new(Move::Cast { card, targets: vec![vec![Target::Object(bear)]] }));
        let before = format!("{g:?}");
        cite!(["118.9d"]; refused("that mana is not in your mana pool (118.3a)"): g.act(0, paid("CCC", mv.clone())));
        assert_eq!(format!("{g:?}"), before);
        g.players[0].pool.colorless = 3;
        cite!(["118.9d"]; ok: g.act(0, paid("CCC", mv)));
        cite!(["118.9d", "702.27a"]; assert_eq!(g.players[0].pool.total(), 0));
        pass(&mut g);
        let Some(Decision::Choose { player: 0, choice: Choice::SpellDestination { on, among } }) = &g.decision else {
            panic!("{:?}", g.decision)
        };
        cite!(["616.1"]; assert_eq!(among, &vec![DestinationEffect::Buyback, DestinationEffect::CastRider]));
        let on = *on;
        let clone = g.clone();
        for mut g in [g, clone] {
            g.act(0, Move::Choose(Answer::SpellDestination { on, effect })).unwrap();
            assert_eq!(g.players[1].hand.len(), 1);
            cite!(["616.1"]; case: format!("{effect:?}"); assert_eq!(g.players[0].hand.len(), usize::from(effect == DestinationEffect::Buyback)));
            assert_eq!(g.exile.len(), usize::from(effect == DestinationEffect::CastRider));
        }
    }
}
#[test]
fn full_rude_awakening_free_from_graveyard_still_pays_entwine() {
    let (mut g, card) = offer(RUDE);
    let land = g.put(0, ISLAND, Zone::Battlefield);
    g.set_tapped(land, true);
    g.players[0].pool.colorless = 2;
    g.players[0].pool.colors[ir::Color::Green as usize] = 1;
    let Some((_, Asked::Cast { .. })) = g.asked() else { panic!("no cast") };
    g.act(
        0,
        paid("CCG", Move::WithEntwine(true, Box::new(Move::Modes(vec![1, 0], Box::new(Move::Cast { card, targets: vec![] }))))),
    )
    .unwrap();
    cite!(["118.9d", "702.42a"]; assert_eq!(g.players[0].pool.total(), 0));
    pass(&mut g);
    cite!(["702.42b"]; assert!(!g.obj(land).tapped));
    cite!(["702.42b"]; assert_eq!(g.power(land), Some(2)));
    assert_eq!(g.exile.len(), 1);
}

#[test]
fn mana_child_clone_and_reversal_keep_the_same_graveyard_offer() {
    use game::legal::Step;
    use ir::mana::ManaSymbol;
    let (mut g, card) = offer(CAPSIZE);
    let source = g.put(0, 8, Zone::Battlefield);
    g.put(0, 9, Zone::Battlefield);
    let evolution = g.put(0, 10, Zone::Battlefield);
    g.put(0, 11, Zone::Battlefield);
    let land = g.put(0, ISLAND, Zone::Battlefield);
    let bear = g.put(1, BEAR, Zone::Battlefield);
    g.act(
        0,
        Move::WithBuyback(
            true,
            Box::new(Move::WithMana(
                vec![Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],
                    source,
                    ability: 0,
                    paying: vec![],
                    adding: vec![vec![ManaSymbol::Color(ir::Color::Blue); 4]],
                }],
                Box::new(paid("UUUU", Move::Cast { card, targets: vec![vec![Target::Object(bear)]] })),
            )),
        ),
    )
    .unwrap();
    let Some(Decision::Choose { choice: Choice::Replacement { among, .. }, .. }) = &g.decision else {
        panic!("no counter question")
    };
    let replacement = among.iter().find(|e| e.source == evolution).unwrap().clone();
    let clone = g.clone();
    for mut g in [g, clone] {
        g.act(0, Move::Choose(Answer::Replacement(replacement.clone()))).unwrap();
        if matches!(g.decision, Some(Decision::Choose { choice: Choice::Mana { .. }, .. })) {
            g.act(0, Move::Choose(Answer::Mana(vec![ManaSymbol::Color(ir::Color::Blue); 3]))).unwrap();
        }
        assert!(matches!(g.decision, Some(Decision::Payment { .. })));
        let mut reversed = g.clone();
        reversed.act(0, Move::ReversePayment).unwrap();
        assert_eq!(reversed.obj(card).zone, Zone::Graveyard);
        assert!(reversed.obj(source).tapped);
        assert!(!reversed.obj(card).buyback_paid);
        let Some((0, Asked::Cast { cards, .. })) = reversed.asked() else { panic!("missing restored offer") };
        assert_eq!(cards, vec![card]);
        reversed.act(0, Move::Choose(Answer::No)).unwrap();
        assert!(reversed.stack.is_empty());
        g.act(
            0,
            Move::FinishPayment {
                plan: vec![Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source: land, ability: 0, paying: vec![], adding: vec![] }],
                paying: vec![],
                with: Some(mana_pool("UUUU")),
            },
        )
        .unwrap();
        pass(&mut g);
        let Some(Decision::Choose { choice: Choice::SpellDestination { on, .. }, .. }) = g.decision else {
            panic!("missing completion")
        };
        g.act(0, Move::Choose(Answer::SpellDestination { on, effect: DestinationEffect::Buyback })).unwrap();
        cite!(["702.27a"]; assert_eq!(g.players[0].hand.len(), 1));
        assert_eq!(g.players[1].hand.len(), 1);
        assert_eq!(g.players[0].pool.total(), 0);
    }
}
