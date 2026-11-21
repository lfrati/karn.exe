//! Explicit replacement, mana and mandatory combat decisions (601.2g-h, 605.3b, 616.1).
use crate::support::{cite,paid,mana_pool};
use game::legal::{Asked, Step};
use game::state::Answer;
use game::{CardDef, Game, Move, ObjId, Zone};
use ir::mana::ManaSymbol;

const ELF: usize = 0;
const SCALES: usize = 1;
const EVOLUTION: usize = 2;
const SPELL: usize = 3;
const LAND: usize = 4;
const BERSERKER: usize = 5;

fn game(cost: &str) -> Game {
    let defs = vec![
        CardDef::new("Mana creature", "{0}", "Creature — Elf", "{T}: You gain 2 life. Put a +1/+1 counter on this creature. Add X mana of any one color, where X is the number of +1/+1 counters on this creature.", Some((1, 1))).unwrap(),
        CardDef::new("Hardened Scales", "{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.", None).unwrap(),
        CardDef::new("Branching Evolution", "{2}{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.", None).unwrap(),
        CardDef::new("Spell", cost, "Sorcery", "You gain 3 life.", None).unwrap(),
        CardDef::new("Land", "", "Land", "{T}: Add {G}.", None).unwrap(),
        CardDef::new("Berserker", "{0}", "Creature", "This creature attacks each combat if able.", Some((2, 2))).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap_or_else(|e| panic!("{}: {}", def.name(), e.0)); }
    Game::new(defs, 2, 20)
}

/// Player 0 with the mana creature, the two replacement effects in this order, and the
/// spell in hand.
fn paying(cost: &str, effects: [usize; 2]) -> (Game, ObjId, ObjId) {
    let mut g = game(cost);
    let elf = g.put(0, ELF, Zone::Battlefield);
    for effect in effects { g.put(0, effect, Zone::Battlefield); }
    let card = g.put(0, SPELL, Zone::Hand);
    (g, elf, card)
}

fn cast(elf: ObjId, card: ObjId, units:&str) -> Move {
    let step = Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source: elf, ability: 0, paying: vec![], adding: vec![vec![ManaSymbol::Color(ir::Color::Green); 3]] };
    Move::WithMana(vec![step], Box::new(paid(units,Move::Cast { card, targets: vec![] })))
}

#[test]
fn r605_3b_explicit_answers_resume_a_mana_ability_inside_payment_once() {
    let (mut g, elf, card) = paying("{3}", [SCALES, EVOLUTION]);
    let scales = g.battlefield[1];
    g.act(0, cast(elf, card,"GGG")).unwrap();
    let Some(game::state::Decision::Choose { choice: game::state::Choice::Replacement { among, .. }, .. }) = &g.decision else { panic!("replacement choice") };
    let selected = among.iter().find(|e| e.source == scales).unwrap().clone();
    g.act(0, Move::Choose(Answer::Replacement(selected))).unwrap();
    g.act(0, Move::Choose(Answer::Mana(vec![ManaSymbol::Color(ir::Color::Green); 4]))).unwrap();
    cite!(["616.1"]; assert_eq!(g.obj(elf).counters.get("+1/+1"), Some(&4)));
    cite!(["605.3b", "601.2h"]; assert_eq!((g.stack.len(), g.players[0].life, g.players[0].pool.total()), (1, 22, 1)));
    assert!(g.obj(elf).tapped && g.decision.is_none() && g.resolving().is_none());
}

#[test]
fn r601_2h_an_explicit_payment_left_short_can_be_completed_without_repeating_its_mana_ability() {
    let (mut g, elf, card) = paying("{4}", [EVOLUTION, SCALES]);
    let evolution = g.battlefield[1];
    let land = g.put(0, LAND, Zone::Battlefield);
    g.act(0, cast(elf, card,"GGGG")).unwrap();
    let Some(game::state::Decision::Choose { choice: game::state::Choice::Replacement { among, .. }, .. }) = &g.decision else { panic!("replacement choice") };
    let selected = among.iter().find(|e| e.source == evolution).unwrap().clone();
    g.act(0, Move::Choose(Answer::Replacement(selected))).unwrap();
    assert!(matches!(g.asked(), Some((0, Asked::Payment { .. }))));
    let before = format!("{g:?}");
    crate::support::refused(g.act(0, Move::Pass), "a decision is pending: complete the announced payment (601.2g-h)");
    assert_eq!(format!("{g:?}"), before);
    assert_eq!((g.players[0].life, g.players[0].pool.total(), g.stack.len()), (22, 3, 1));
    let step = Step { plan: vec![], x: None, life: vec![], with: None, source: land, ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None };
    g.act(0, Move::FinishPayment { plan: vec![step], paying: vec![], with: Some(mana_pool("GGGG")) }).unwrap();
    cite!(["601.2h"]; assert_eq!((g.players[0].life, g.players[0].pool.total(), g.stack.len()), (22, 0, 1)));
    cite!(["616.1"]; assert_eq!(g.obj(elf).counters.get("+1/+1"), Some(&3)));
    assert_eq!(g.asked(), Some((0, Asked::Priority)));
}

#[test]
fn r508_1d_an_explicit_declaration_cannot_omit_a_creature_that_must_attack() {
    let mut g = game("{0}");
    let berserker = g.put(0, BERSERKER, Zone::Battlefield);
    g.part = ir::Part::DeclareAttackers;
    g.decision = Some(game::state::Decision::Attackers { player: 0 });
    let before = format!("{g:?}");
    cite!(["508.1d"]; refused("Berserker attacks if able (508.1d)"): g.act(0, Move::Attack(vec![])));
    assert_eq!(format!("{g:?}"), before);
    cite!(["508.1d"]; ok: g.act(0, Move::Attack(vec![(berserker, game::Target::Player(1))])));
    assert!(g.combat.attacking(berserker));
}
