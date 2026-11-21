//! A card definition is its printed parts and what was compiled from them, made together and
//! then only read: the compiled parts cannot describe other text than the definition holds.
//! A different card is another definition, built from its own text. (That the printed parts
//! cannot be assigned from outside the crate is checked where `CardDef` is declared.)

use crate::support::cite;
use game::play::check;
use game::program::{ActSpan, Instr, Program};
use game::{CardDef, Game, Move, Zone};
use ir::{CardType, Color};

fn card(name: &str, cost: &str, kind: &str, text: &str) -> CardDef {
    CardDef::new(name, cost, kind, text, None).unwrap()
}

#[test]
fn a_definition_is_read_as_it_was_printed() {
    let def = CardDef::new("Tested", "{1}{G}", "Legendary Creature — Bear", "Trample\n{T}: You gain 1 life.", Some((2, 3))).unwrap();
    assert_eq!(def.name(), "Tested");
    assert_eq!(def.cost().map(|c| c.0.len()), Some(2));
    assert_eq!(def.supertypes(), [ir::Supertype::Legendary]);
    assert_eq!(def.types(), [CardType::Creature]);
    assert_eq!(def.subtypes(), [ir::Subtype::from_name("Bear").unwrap()]);
    assert_eq!(def.colors(), [Color::Green]);
    assert_eq!(def.pt(), Some((2, 3)));
    assert_eq!(def.abilities().len(), 2);
    assert_eq!(def.modes().len(), def.abilities().len());
    assert!(def.halves().is_empty());
    // a land has no mana cost, which is not a cost of {0} (202.1b)
    cite!(["202.1b"]; assert!(card("Forest", "", "Basic Land — Forest", "").cost().is_none()));
}

#[test]
fn what_was_compiled_answers_for_the_text_the_definition_holds() {

    // the same card with other text is another definition, compiled from that text: neither
    // the admission nor the compiled abilities of the first are carried to it
    let refused = card("Tested", "{0}", "Artifact", "{T}: You gain 1 life.\nAs long as this artifact is untapped, players can't untap more than two permanents during their untap steps.");
    let rebuilt = card(refused.name(), "{0}", "Artifact", "{T}: You gain 1 life.\n{T}: Draw a card.");
    crate::support::refused(check(&refused).map_err(|e|e.0),"prohibition: a window of its own");
    assert_eq!(refused.abilities().len(), 2);
    check(&rebuilt).unwrap();
    assert_eq!(rebuilt.compiled_abilities(), [0, 1]);
    assert_ne!(refused.compiled_abilities(), rebuilt.compiled_abilities());
    // and a copy is the same definition whole
    let copy = rebuilt.clone();
    assert_eq!(copy.abilities(), rebuilt.abilities());
    assert_eq!(copy.compiled_abilities(), rebuilt.compiled_abilities());
    assert_eq!(check(&copy), check(&rebuilt));
}

#[test]
fn every_way_of_building_a_definition_compiles_it_whole() {
    let toil = card("Toil", "{2}{B}", "Sorcery", "Target player draws two cards and loses 2 life.\nFuse");
    let trouble = card("Trouble", "{2}{R}", "Sorcery", "Trouble deals damage to target player equal to the number of cards in that player's hand.\nFuse");
    let split = CardDef::split([toil.clone(), trouble.clone()]).unwrap();
    check(&split).unwrap();
    assert_eq!(split.name(), "Toil // Trouble");
    assert_eq!(split.cost().map(|c| c.0.len()), Some(4));
    assert_eq!(split.colors(), [Color::Black, Color::Red]);
    assert_eq!(split.types(), [CardType::Sorcery]);
    // the abilities of both halves, in order, each compiled as an ability of the whole card
    let both: Vec<_> = toil.abilities().iter().chain(trouble.abilities()).cloned().collect();
    assert_eq!(split.abilities(), both);
    assert_eq!(split.modes().len(), both.len());
    let whole = split.compiled_abilities();
    assert_eq!(whole.len(), toil.compiled_abilities().len() + trouble.compiled_abilities().len());
    // and each half is still the definition it was built as
    let [left, right] = split.halves() else { panic!("two halves") };
    for (half, made) in [(left, &toil), (right, &trouble)] {
        assert_eq!(half.name(), made.name());
        assert_eq!(half.abilities(), made.abilities());
        assert_eq!(half.compiled_abilities(), made.compiled_abilities());
    }
}

#[test]
fn a_program_is_read_as_it_was_validated() {

    let draw = |n: u32| Program::new(vec![Instr::TakeTop { n: n.into(), whose: game::program::action::Operand::Controller, out: 0 }], vec![ActSpan { end: 1, moves: false, keeps_x: false }]).unwrap();
    let one = draw(1);
    assert_eq!(one.instrs().len(), 1);
    assert_eq!(one.acts().len(), 1);
    assert!(one.targets().is_empty());
    // another program is built and validated anew, with the identity of what it is
    let two = draw(2);
    assert_ne!(one.instrs(), two.instrs());
    assert_ne!(one.id(), two.id());
    assert_eq!(draw(1).id(), one.id());
    assert_eq!(game::program::ProgramId::of_canonical(&one.canonical()), one.id());
    // what does not validate is no program at all, rather than one to be corrected afterwards
    crate::support::refused(Program::new(vec![Instr::Remainder { of: 3, minus: vec![], still_in: Zone::Library, out: 0 }], vec![ActSpan { end: 1, moves: false, keeps_x: false }]).map_err(|e|e.0),"slot 3 is read before any instruction produces it");
}

#[test]
fn a_game_runs_the_definition_it_was_given() {
    let def = card("Tested", "{0}", "Sorcery", "You gain 3 life.");
    let printed = def.abilities().to_vec();
    let mut g = Game::new(vec![def], 2, 20);
    let c = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: c, targets: vec![] }).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    assert_eq!(g.players[0].life, 23);
    assert_eq!(g.defs[0].abilities(), printed);
}
