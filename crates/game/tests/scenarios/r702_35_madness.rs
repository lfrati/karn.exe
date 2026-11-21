//! Madness (702.35): "Madness {R}" means "If a player would discard this card, that player
//! discards it, but exiles it instead of putting it into their graveyard" and "When this card
//! is exiled this way, its owner may cast it by paying {R} rather than paying its mana cost.
//! If that player doesn't, they put this card into their graveyard." (702.35a). The cast
//! follows the rules for alternative costs (702.35b, 601.2b, 601.2f-h); the spell goes where
//! any spell goes. The card is Fiery Temper's Oracle text; the others are test cards.
use crate::support::cite;
use game::announcement::CastCost;
use game::card::CardDef;
use game::event::Event;
use game::legal::Asked;
use game::play::check;
use game::state::{Answer, Decision};
use game::{Game, Move, ObjId, Target, Zone};
use ir::mana::ManaSymbol;
use ir::Color;

const FIERY: usize = 0;
const MOUNTAIN: usize = 1;
const RUMMAGE: usize = 2;
const MONGREL: usize = 3;
const AXE: usize = 4;
const RAID: usize = 5;
const HOUND: usize = 6;
const BLAST: usize = 7;
const DENY: usize = 8;
const BEARS: usize = 9;
const SIFTER: usize = 10;
const RECALLER: usize = 11;

const TEMPER: &str = "Fiery Temper deals 3 damage to any target.\nMadness {R}";

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Fiery Temper", "{1}{R}{R}", "Instant", TEMPER, None),
        card("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None),
        card("Rummage", "{0}", "Instant", "Discard a card.", None),
        card("Mongrel", "{0}", "Creature — Dog", "Discard a card: This creature gets +1/+1 until end of turn.", Some((2, 2))),
        card("Axe", "{0}", "Instant", "As an additional cost to cast this spell, discard a card.\nThis spell deals 2 damage to any target.", None),
        card("Raid", "{0}", "Sorcery", "Target player discards a card.", None),
        // madness beside another replacement of the same discard (616.1)
        card("Stubborn Hound", "{1}{G}{G}", "Creature — Dog", "Madness {G}\nIf a spell or ability an opponent controls causes you to discard this card, put it onto the battlefield instead of putting it into your graveyard.", Some((3, 3))),
        // madness beside a printed alternative cost (118.9a)
        card("Mad Blast", "{4}{R}{R}", "Instant", "You may sacrifice a Mountain rather than pay this spell's mana cost.\nThis spell deals 4 damage to any target.\nMadness {R}", None),
        card("Deny", "{0}", "Instant", "Counter target spell.", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        // what reads the discarded card afterwards (400.7k, 702.35c)
        card("Sifter", "{0}", "Enchantment", "Whenever you discard a card, exile that card from your graveyard.", None),
        card("Recaller", "{0}", "Enchantment", "Whenever you discard a card, return that card to your hand.", None),
    ]
}

/// Fiery Temper's madness ability is its second row.
const MADNESS: CastCost = CastCost::Alternative(1);

fn game() -> Game {
    let mut g = Game::new(defs(), 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, MOUNTAIN, Zone::Library); } }
    g
}

fn pass_all(g: &mut Game) {
    for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); }
}

fn discards(g: &Game) -> Vec<(ObjId, bool)> {
    g.log.iter().filter_map(|e| match e { Event::Discarded { card, madness, .. } => Some((*card, madness.is_some())), _ => None }).collect()
}

fn named(g: &Game, ids: &[ObjId], name: &str) -> Vec<ObjId> {
    ids.iter().copied().filter(|o| g.obj(*o).chars.name == name).collect()
}

/// Player 0 discards Fiery Temper to their own Rummage: it is exiled, and its trigger waits on
/// the stack. Returns the card as it was in hand and as it is in exile.
fn discarded_to_rummage(g: &mut Game) -> (ObjId, ObjId) {
    let temper = g.put(0, FIERY, Zone::Hand);
    let rummage = g.put(0, RUMMAGE, Zone::Hand);
    g.act(0, Move::Cast { card: rummage, targets: vec![] }).unwrap();
    pass_all(g);
    let exiled = named(g, &g.exile.clone(), "Fiery Temper");
    assert_eq!(exiled.len(), 1, "discarded into exile");
    (temper, exiled[0])
}

fn offer(g: &mut Game) -> Vec<ObjId> {
    match g.asked() { Some((0, Asked::Cast { cards })) => cards, other => panic!("not an offer to cast: {other:?}") }
}

fn madness_cast(card: ObjId, target: Target) -> Move {
    crate::support::paid("R",Move::CastFor(MADNESS, Box::new(Move::Cast { card, targets: vec![vec![target]] })))
}

#[test]
fn fiery_temper_is_its_oracle_card_and_the_test_cards_are_admitted() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let real = CardDef::from_faces(catalog.get("Fiery Temper").unwrap()).unwrap();
    check(&real).unwrap();
    assert_eq!(real.abilities(), defs()[FIERY].abilities());
    for d in defs() { check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0)); }
    // two madness abilities of one card: refused, not one of them ignored
    let twice = CardDef::new("Twice", "{2}{R}", "Instant", "This spell deals 2 damage to any target.\nMadness {R}\nMadness {1}", None).unwrap();
    // Uncited (admission contract): duplicate madness abilities are explicitly unsupported.
    assert_eq!(check(&twice).expect_err("duplicate madness is unsupported").0, "madness: more than one madness ability on a card");
}

#[test]
fn r702_35a_discarded_by_an_effect_it_is_exiled_and_cast_for_its_madness_cost() {
    let mut g = game();
    let (temper, exiled) = discarded_to_rummage(&mut g);
    // it is discarded, once, into exile: a new object (400.7)
    cite!(["702.35a"]; assert_eq!(discards(&g), vec![(exiled, true)]));
    assert_ne!(temper, exiled);
    assert!(g.players[0].graveyard.iter().all(|o| g.obj(*o).chars.name != "Fiery Temper"));
    // its trigger waits on the stack, its owner's, its source the exiled card
    cite!(["702.35a"]; assert_eq!(g.stack.len(), 1));
    let trigger = g.stack[0];
    assert_eq!(g.obj(trigger).ability.and_then(|(source, _)| source), Some(exiled));
    assert_eq!(g.controller(trigger), 0);
    for _ in 0..3 { g.add_mana(0, ManaSymbol::Color(Color::Red)); }
    pass_all(&mut g);
    // offered for its madness cost only
    let cards = offer(&mut g);
    assert_eq!(cards, vec![exiled]);
    // its mana cost is not what the ability lets it be cast for
    let mana = Move::Cast { card: exiled, targets: vec![vec![Target::Player(1)]] };
    cite!(["702.35a"]; refused("the effect lets that card be cast only for the cost it offers (118.9a)"): g.act(0, mana));
    g.act(0, madness_cast(exiled, Target::Player(1))).unwrap();
    cite!(["702.35b"]; assert_eq!(g.players[0].pool.total(), 2, "only {{R}} paid, despite having enough for the printed cost"));
    // cast: "If you don't" does not put anything into the graveyard, and priority follows
    let spell = *g.stack.last().unwrap();
    assert_eq!(g.obj(spell).chars.name, "Fiery Temper");
    assert_eq!(g.asked().map(|(p, a)| (p, a == Asked::Priority)), Some((0, true)));
    pass_all(&mut g);
    assert_eq!(g.players[1].life, 17);
    // a spell cast with madness goes to its owner's graveyard, as any spell (702.35b)
    cite!(["608.2n"]; assert_eq!(named(&g, &g.players[0].graveyard.clone(), "Fiery Temper").len(), 1));
    assert!(g.exile.is_empty());
    assert_eq!(discards(&g).len(), 1, "the discard event triggered once");
}

#[test]
fn r702_35a_declined_or_unpayable_it_goes_to_its_owners_graveyard() {
    for red in [true, false] {
        let mut g = game();
        let (_, exiled) = discarded_to_rummage(&mut g);
        if red { g.add_mana(0, ManaSymbol::Color(Color::Red)); }
        pass_all(&mut g);
        let cards = offer(&mut g);
        assert_eq!(cards, vec![exiled]);
        // without {R} there is no payment for it: nothing to cast, still asked
        let mut attempt = g.clone();
        if red {
            cite!(["702.35a", "601.2h"]; ok: attempt.act(0, madness_cast(exiled, Target::Player(1))));
            assert_eq!(attempt.stack.len(), 1);
            assert_eq!(attempt.players[0].pool.total(), 0);
        } else {
            cite!(["118.3a"]; refused("that mana is not in your mana pool"): attempt.act(0, madness_cast(exiled, Target::Player(1))));
            assert_eq!(format!("{attempt:?}"), format!("{g:?}"));
        }
        g.act(0, Move::Choose(Answer::No)).unwrap();
        assert!(g.exile.is_empty() && g.stack.is_empty());
        let graveyard = named(&g, &g.players[0].graveyard.clone(), "Fiery Temper");
        cite!(["702.35a"]; case: if red { "declined" } else { "unpayable" }; assert_eq!(graveyard.len(), 1));
        cite!(["400.7"]; assert_ne!(graveyard[0], exiled, "a new object in the graveyard (400.7)"));
        assert_eq!(g.players[0].pool.total(), u32::from(red), "the {{R}} stays");
        assert_eq!(g.players[1].life, 20);
    }
}

#[test]
fn r733_1_a_madness_cast_reversed_is_not_a_cast_made() {
    // a mana ability that asks a question as it is activated to pay: the payment is suspended
    // in the middle of the cast, and what it makes then does not pay {R}
    let mut defs = defs();
    defs.push(CardDef::new("Mana creature", "{0}", "Creature — Elf", "{T}: Put a +1/+1 counter on this creature. Add X mana of any one color, where X is the number of +1/+1 counters on this creature.", Some((1, 1))).unwrap());
    defs.push(CardDef::new("Hardened Scales", "{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.", None).unwrap());
    defs.push(CardDef::new("Branching Evolution", "{2}{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.", None).unwrap());
    let n = defs.len();
    let mut g = Game::new(defs, 2, 20);
    let source = g.put(0, n - 3, Zone::Battlefield);
    g.put(0, n - 2, Zone::Battlefield);
    let evolution = g.put(0, n - 1, Zone::Battlefield);
    let (_, exiled) = discarded_to_rummage(&mut g);
    pass_all(&mut g);
    let green = game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source, ability: 0, paying: vec![], adding: vec![vec![ManaSymbol::Color(Color::Green); 3]] };
    g.act(0, Move::WithMana(vec![green], Box::new(madness_cast(exiled, Target::Player(1))))).unwrap();
    let Some(Decision::Choose { choice: game::state::Choice::Replacement { among, .. }, .. }) = &g.decision else { panic!("{:?}", g.decision) };
    let effect = among.iter().find(|e| e.source == evolution).unwrap().clone();
    g.act(0, Move::Choose(Answer::Replacement(effect))).unwrap();
    // three green mana do not pay {R}: the cast is begun, and its payment cannot be completed
    assert!(matches!(g.decision, Some(Decision::Payment { .. })), "{:?}", g.decision);
    g.act(0, Move::ReversePayment).unwrap();
    cite!(["733.1"]; assert_eq!(g.obj(exiled).zone, Zone::Exile, "back where it was (733.1)"));
    // offered again, declined: it was not cast, so it is put into the graveyard
    assert_eq!(offer(&mut g), vec![exiled]);
    g.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["702.35a"]; assert_eq!(named(&g, &g.players[0].graveyard.clone(), "Fiery Temper").len(), 1));
    assert!(g.exile.is_empty() && g.stack.is_empty());
}

#[test]
fn r608_2g_no_priority_while_offered_targets_are_checked_and_a_copy_resumes_the_same() {
    let mut g = game();
    let (_, exiled) = discarded_to_rummage(&mut g);
    g.add_mana(0, ManaSymbol::Color(Color::Red));
    let land = g.put(1, MOUNTAIN, Zone::Battlefield);
    pass_all(&mut g);
    offer(&mut g);
    // the trigger is resolving: no player has priority, no other spell is cast
    cite!(["608.2g"]; assert!(!matches!(g.asked(), Some((_, Asked::Priority)))));
    cite!(["608.2g"]; refused("a decision is pending: cast one of the cards offered, or decline (608.2g)"): g.act(0, Move::Pass));
    crate::support::refused(g.act(1, Move::Choose(Answer::No)), "that decision is another player's");
    // a land is no legal target (115.4): refused, nothing changes
    let before = format!("{g:?}");
    cite!(["601.2c", "115.1"]; refused("an illegal target, Mountain: it isn't a creature or a planeswalker or a battle or a player (115.1)"): g.act(0, madness_cast(exiled, Target::Object(land))));
    assert_eq!(format!("{g:?}"), before);
    // a copy taken while it is asked continues the same
    let mut copy = g.clone();
    for g in [&mut g, &mut copy] {
        g.act(0, madness_cast(exiled, Target::Player(1))).unwrap();
        pass_all(g);
    }
    assert_eq!(format!("{g:?}"), format!("{copy:?}"));
    assert_eq!(g.players[1].life, 17);
}

#[test]
fn r603_3_discarded_as_a_cost_its_trigger_resolves_before_what_it_paid_for() {
    // an activated ability's cost
    let mut g = game();
    let mongrel = g.put(0, MONGREL, Zone::Battlefield);
    let temper = g.put(0, FIERY, Zone::Hand);
    g.add_mana(0, ManaSymbol::Color(Color::Red));
    g.act(0, Move::Activate { source: mongrel, ability: 0, targets: vec![], paying: vec![vec![temper]] }).unwrap();
    let exiled = named(&g, &g.exile.clone(), "Fiery Temper");
    assert_eq!(discards(&g), vec![(exiled[0], true)]);
    // the trigger is put on the stack after the ability is activated, above it
    assert_eq!(g.stack.len(), 2);
    assert_eq!(g.obj(g.stack[0]).ability.and_then(|(s, _)| s), Some(mongrel));
    cite!(["603.3"]; assert_eq!(g.obj(g.stack[1]).ability.and_then(|(s, _)| s), Some(exiled[0])));
    pass_all(&mut g);
    g.act(0, madness_cast(exiled[0], Target::Player(1))).unwrap();
    // Fiery Temper resolves first, then the ability
    pass_all(&mut g);
    cite!(["405.5"]; assert_eq!((g.players[1].life, g.stack.len()), (17, 1)));
    pass_all(&mut g);
    assert!(g.stack.is_empty());
    // a spell's additional cost: the trigger above the spell
    let mut g = game();
    let axe = g.put(0, AXE, Zone::Hand);
    let temper = g.put(0, FIERY, Zone::Hand);
    g.act(0, Move::WithCosts(vec![vec![temper]], Box::new(Move::Cast { card: axe, targets: vec![vec![Target::Player(1)]] }))).unwrap();
    assert_eq!(g.stack.len(), 2);
    assert_eq!(g.obj(g.stack[0]).chars.name, "Axe");
    cite!(["603.3"]; assert!(g.obj(g.stack[1]).ability.is_some()));
    pass_all(&mut g);
    g.act(0, Move::Choose(Answer::No)).unwrap();
    assert_eq!(named(&g, &g.players[0].graveyard.clone(), "Fiery Temper").len(), 1);
    pass_all(&mut g);
    assert_eq!(g.players[1].life, 18);
}

#[test]
fn r118_9a_cast_with_madness_no_other_alternative_cost_and_madness_not_from_a_hand() {
    let mut g = game();
    let blast = g.put(0, BLAST, Zone::Hand);
    let mountain = g.put(0, MOUNTAIN, Zone::Battlefield);
    // from a hand: its mana cost or its printed alternative, never its madness cost
    cite!(["118.9a"]; ok: g.clone().act(0, Move::CastFor(CastCost::Alternative(0), Box::new(Move::WithCosts(vec![vec![mountain]], Box::new(Move::Cast { card: blast, targets: vec![vec![Target::Player(1)]] }))))));
    g.add_mana(0, ManaSymbol::Color(Color::Red));
    cite!(["702.35a"]; refused("that cost is paid only for a cast its ability offers (702.35a)"): g.act(0, Move::CastFor(CastCost::Alternative(2), Box::new(Move::Cast { card: blast, targets: vec![vec![Target::Player(1)]] }))));
    // discarded, its madness ability offers only its madness cost
    let rummage = g.put(0, RUMMAGE, Zone::Hand);
    g.act(0, Move::Cast { card: rummage, targets: vec![] }).unwrap();
    pass_all(&mut g);
    // Blast is the only card in hand, so the mandatory discard has completed.
    assert!(g.players[0].hand.is_empty());
    let exiled = named(&g, &g.exile.clone(), "Mad Blast")[0];
    pass_all(&mut g);
    let sacrifice = Move::CastFor(CastCost::Alternative(0), Box::new(Move::WithCosts(vec![vec![mountain]], Box::new(Move::Cast { card: exiled, targets: vec![vec![Target::Player(1)]] }))));
    cite!(["118.9a"]; refused("the effect lets that card be cast only for the cost it offers (118.9a)"): g.act(0, sacrifice));
    g.act(0, crate::support::paid("R",Move::CastFor(CastCost::Alternative(2), Box::new(Move::Cast { card: exiled, targets: vec![vec![Target::Player(1)]] })))).unwrap();
    assert!(g.battlefield.contains(&mountain));
    pass_all(&mut g);
    assert_eq!(g.players[1].life, 16);
}

#[test]
fn r616_1_its_owner_chooses_between_madness_and_another_replacement_of_the_discard() {
    for madness in [true, false] {
        let mut g = game();
        let hound = g.put(0, HOUND, Zone::Hand);
        // the opponent's spell makes player 0 discard: both replacements apply
        while g.active == 0 { g.act(g.priority, Move::Pass).unwrap(); }
        while g.priority != 1 { g.act(g.priority, Move::Pass).unwrap(); }
        while !g.part.main() || !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
        let raid = g.put(1, RAID, Zone::Hand);
        g.act(1, Move::Cast { card: raid, targets: vec![vec![Target::Player(0)]] }).unwrap();
        pass_all(&mut g);
        let among = match g.asked() {
            Some((0, Asked::Replacement { among, optional: false, .. })) => among,
            other => panic!("the discarding player chooses (616.1): {other:?}"),
        };
        let case = if madness { "madness" } else { "battlefield" };
        cite!(["616.1"]; case: case; assert_eq!(among.len(), 2));
        assert!(among.iter().all(|id| id.source == hound));
        // a copy at this suspended discard resumes the same
        let copy = g.clone();
        let ability = game::binding::AbilityKey::printed(if madness { 0 } else { 1 });
        let chosen = among.iter().find(|id| id.ability() == Some(&ability)).expect("the selected printed replacement").clone();
        g.act(0, Move::Choose(Answer::Replacement(chosen.clone()))).unwrap();
        let mut copy = copy;
        copy.act(0, Move::Choose(Answer::Replacement(chosen))).unwrap();
        assert_eq!(format!("{g:?}"), format!("{copy:?}"));
        let on_battlefield = named(&g, &g.battlefield.clone(), "Stubborn Hound");
        let exiled = named(&g, &g.exile.clone(), "Stubborn Hound");
        // the one applied leaves the other nothing to replace: it goes one place, once
        cite!(["616.1f"]; case: case; assert_eq!(on_battlefield.len() + exiled.len(), 1));
        cite!(["616.1"]; case: case; assert_eq!(exiled.len(), usize::from(madness), "the chosen replacement determines the destination"));
        assert_eq!(discards(&g).len(), 1);
        assert_eq!(discards(&g)[0].1, !exiled.is_empty());
        assert_eq!(g.stack.len(), usize::from(!exiled.is_empty()), "the madness trigger only when exiled by madness");
    }
    // its own discard: only madness applies, nothing is asked
    let mut g = game();
    g.put(0, HOUND, Zone::Hand);
    let rummage = g.put(0, RUMMAGE, Zone::Hand);
    g.act(0, Move::Cast { card: rummage, targets: vec![] }).unwrap();
    pass_all(&mut g);
    cite!(["702.35a"]; assert_eq!(named(&g, &g.exile.clone(), "Stubborn Hound").len(), 1));
}

#[test]
fn r514_1_discarded_to_hand_size_it_is_exiled_and_offered() {
    let mut g = game();
    let temper = g.put(0, FIERY, Zone::Hand);
    for _ in 0..7 { g.put(0, BEARS, Zone::Hand); }
    while !matches!(g.decision, Some(Decision::Discard { .. })) { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Discard(vec![temper])).unwrap();
    let exiled = named(&g, &g.exile.clone(), "Fiery Temper");
    cite!(["514.1", "702.35a"]; assert_eq!(discards(&g), vec![(exiled[0], true)]));
    // the trigger is put on the stack in the cleanup step, and players get priority (514.3a)
    cite!(["514.3a"]; assert_eq!(g.stack.len(), 1));
    g.add_mana(0, ManaSymbol::Color(Color::Red));
    pass_all(&mut g);
    g.act(0, madness_cast(exiled[0], Target::Player(1))).unwrap();
    pass_all(&mut g);
    assert_eq!(g.players[1].life, 17);
}

#[test]
fn r608_2g_a_madness_creature_is_cast_in_an_opponents_turn_and_enters() {
    let rootwalla = CardDef::new("Basking Rootwalla", "{G}", "Creature — Lizard",
        "{1}{G}: This creature gets +2/+2 until end of turn. Activate only once each turn.\nMadness {0}", Some((1, 1))).unwrap();
    check(&rootwalla).unwrap();
    let mut defs = defs();
    defs.push(rootwalla);
    let mut g = Game::new(defs.clone(), 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, MOUNTAIN, Zone::Library); } }
    // in the opponent's turn, with a spell of theirs on the stack: no time to cast a creature
    while g.active == 0 { g.act(g.priority, Move::Pass).unwrap(); }
    while !g.part.main() || g.priority != 1 { g.act(g.priority, Move::Pass).unwrap(); }
    let raid = g.put(1, RAID, Zone::Hand);
    g.put(0, defs.len() - 1, Zone::Hand);
    g.act(1, Move::Cast { card: raid, targets: vec![vec![Target::Player(0)]] }).unwrap();
    pass_all(&mut g);
    let exiled = named(&g, &g.exile.clone(), "Basking Rootwalla")[0];
    pass_all(&mut g);
    g.act(0, Move::CastFor(MADNESS, Box::new(Move::Cast { card: exiled, targets: vec![] }))).unwrap();
    pass_all(&mut g);
    assert_eq!(named(&g, &g.battlefield.clone(), "Basking Rootwalla").len(), 1);
    cite!(["608.3a"]; assert_eq!(g.controller(named(&g, &g.battlefield.clone(), "Basking Rootwalla")[0]), 0));
}

#[test]
fn r702_35b_countered_a_madness_spell_goes_to_its_owners_graveyard() {
    let mut g = game();
    let (_, exiled) = discarded_to_rummage(&mut g);
    g.add_mana(0, ManaSymbol::Color(Color::Red));
    pass_all(&mut g);
    g.act(0, madness_cast(exiled, Target::Player(1))).unwrap();
    let spell = *g.stack.last().unwrap();
    g.act(0, Move::Pass).unwrap();
    let deny = g.put(1, DENY, Zone::Hand);
    g.act(1, Move::Cast { card: deny, targets: vec![vec![Target::Object(spell)]] }).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    assert_eq!(g.players[1].life, 20);
    cite!(["701.6a"]; assert_eq!(named(&g, &g.players[0].graveyard.clone(), "Fiery Temper").len(), 1));
    assert!(g.exile.is_empty());
}

/// Put the named trigger sources on the stack in the authored resolution order.
/// The interface orders bottom-to-top, so reverse the expected resolution list.
fn resolve_in_order(g: &mut Game, sources: &[&str]) {
    let Some((0, Asked::Order(abilities))) = g.asked() else { panic!("{:?}", g.asked()) };
    assert_eq!(abilities.len(), sources.len());
    let order: Vec<_> = sources.iter().rev().map(|name| {
        let indices: Vec<_> = abilities.iter().enumerate().filter_map(|(index, (source, _))|
            (g.obj(source.expect("named trigger source")).chars.name == *name).then_some(index)).collect();
        assert_eq!(indices.len(), 1, "one trigger from authored source {name}");
        indices[0]
    }).collect();
    g.act(0, Move::Order(order)).unwrap();
    let top_down: Vec<_> = g.stack.iter().rev().map(|o|
        g.obj(g.obj(*o).ability.unwrap().0.unwrap()).chars.name.as_str()).collect();
    assert_eq!(top_down, sources);
}

/// Resolve the top of the stack, declining a madness offer it makes.
fn resolve_top_declining(g: &mut Game) {
    let depth = g.stack.len();
    while g.stack.len() >= depth {
        match g.asked() {
            Some((0, Asked::Cast { .. })) => g.act(0, Move::Choose(Answer::No)).unwrap(),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
}

/// Reclaim what nothing refers to any more, as a host does between moves.
fn collect(g: &mut Game) {
    g.retire_events(g.log.end());
    g.collect_departed();
}

/// 702.35c, 400.7k: after a madness trigger resolves, if the card was not cast and was moved
/// to a public zone, effects that refer to the discarded card find it there. A discard
/// trigger resolving after the declined madness trigger exiles it from the graveyard, across
/// a collection of departed objects in between.
#[test]
fn r400_7k_after_its_madness_trigger_an_effect_naming_the_discarded_card_finds_it() {
    let mut g = game();
    g.put(0, SIFTER, Zone::Battlefield);
    let (_, exiled) = discarded_to_rummage(&mut g);
    resolve_in_order(&mut g, &["Fiery Temper", "Sifter"]);
    resolve_top_declining(&mut g);
    let in_graveyard = named(&g, &g.players[0].graveyard.clone(), "Fiery Temper");
    assert_eq!(in_graveyard.len(), 1);
    assert_ne!(in_graveyard[0], exiled);
    collect(&mut g);
    resolve_top_declining(&mut g);
    cite!(["400.7k", "702.35c"]; assert!(g.players[0].graveyard.iter().all(|o| g.obj(*o).chars.name != "Fiery Temper"), "the Sifter found it"));
    assert_eq!(named(&g, &g.exile.clone(), "Fiery Temper").len(), 1);
}

#[test]
fn r400_7k_a_card_cast_with_madness_is_not_found_afterwards() {
    let mut g = game();
    g.put(0, SIFTER, Zone::Battlefield);
    let (_, exiled) = discarded_to_rummage(&mut g);
    resolve_in_order(&mut g, &["Fiery Temper", "Sifter"]);
    g.add_mana(0, ManaSymbol::Color(Color::Red));
    pass_all(&mut g);
    g.act(0, madness_cast(exiled, Target::Player(1))).unwrap();
    // the spell resolves above the Sifter's ability, then the ability: the spell's card in the
    // graveyard is not the discarded card it names
    pass_all(&mut g);
    assert_eq!(g.players[1].life, 17);
    pass_all(&mut g);
    assert!(g.stack.is_empty());
    cite!(["400.7k"]; assert_eq!(named(&g, &g.players[0].graveyard.clone(), "Fiery Temper").len(), 1));
}

#[test]
fn r400_7k_before_its_madness_trigger_resolves_the_card_in_exile_is_not_in_the_graveyard() {
    let mut g = game();
    g.put(0, SIFTER, Zone::Battlefield);
    let (_, exiled) = discarded_to_rummage(&mut g);
    resolve_in_order(&mut g, &["Sifter", "Fiery Temper"]);
    resolve_top_declining(&mut g);
    // "from your graveyard": the card is in exile, untouched, and still offered
    assert_eq!(g.obj(exiled).zone, Zone::Exile);
    pass_all(&mut g);
    assert_eq!(offer(&mut g), vec![exiled]);
    g.act(0, Move::Choose(Answer::No)).unwrap();
    assert_eq!(named(&g, &g.players[0].graveyard.clone(), "Fiery Temper").len(), 1, "the Sifter has resolved");
}

#[test]
fn r400_7k_found_once_where_it_went_not_after_it_moves_again() {
    let mut g = game();
    g.put(0, SIFTER, Zone::Battlefield);
    g.put(0, RECALLER, Zone::Battlefield);
    discarded_to_rummage(&mut g);
    resolve_in_order(&mut g, &["Fiery Temper", "Recaller", "Sifter"]);
    resolve_top_declining(&mut g);
    // the Recaller finds it in the graveyard and returns it to its owner's hand
    resolve_top_declining(&mut g);
    cite!(["400.7k"]; assert_eq!(named(&g, &g.players[0].hand.clone(), "Fiery Temper").len(), 1));
    // the card in hand is a new object again, in a hidden zone: the Sifter finds nothing
    resolve_top_declining(&mut g);
    assert_eq!(named(&g, &g.players[0].hand.clone(), "Fiery Temper").len(), 1);
    cite!(["400.7k"]; assert!(named(&g, &g.exile.clone(), "Fiery Temper").is_empty()));
}

#[test]
fn r400_7e_without_madness_a_discard_trigger_finds_the_card_in_the_graveyard_anyway() {
    let mut g = game();
    g.put(0, SIFTER, Zone::Battlefield);
    g.put(0, BEARS, Zone::Hand);
    let rummage = g.put(0, RUMMAGE, Zone::Hand);
    g.act(0, Move::Cast { card: rummage, targets: vec![] }).unwrap();
    pass_all(&mut g);
    assert_eq!(discards(&g).len(), 1);
    assert!(!discards(&g)[0].1, "not by madness");
    pass_all(&mut g);
    cite!(["400.7e"]; assert_eq!(named(&g, &g.exile.clone(), "Grizzly Bears").len(), 1));
}
