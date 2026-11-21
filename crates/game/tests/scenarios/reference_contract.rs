//! Grammar-to-binder reference contract (608.2c). These checks distinguish a scoped
//! instruction result from an event predicate and from an explicitly qualified group.
use game::program::{mention, Taking};
use grammar::kernel::Outcome;
use ir::reference::{Kind, Qualifier, RefForm, UnresolvedRef};
use ir::{Atom, Filter, Quant, Ref, Selector, Verb};

fn read(text: &str) -> Selector {
    let rule = &grammar::selector::selectors().selector;
    let Outcome::Accepted { value, .. } = rule.parse_all(&oracle::token::tokenize(text)) else { panic!("{text}") };
    let said = rule.print_text(&value, false).unwrap_or_else(|| panic!("cannot print {text}: {value:?}"));
    let Outcome::Accepted { value: again, .. } = rule.parse_all(&oracle::token::tokenize(&said)) else { panic!("{said}") };
    assert_eq!(value, again, "{text} -> {said}");
    value
}

#[test]
fn r608_2c_grammar_emits_typed_result_membership_for_each_producer() {
    for (word, verb) in [("revealed", Verb::Reveal), ("exiled", Verb::Exile), ("discarded", Verb::Discard)] {
        let reference = UnresolvedRef::action_result(verb);
        let s = read(&format!("all land cards {word} this way"));
        let Filter::All(filters) = &s.set.filter else { panic!("{s:?}") };
        assert!(filters.contains(&Filter::Atom(Atom::Reference(reference.clone()))));
        assert!(!filters.iter().any(|f| matches!(f, Filter::Atom(Atom::Did { .. }))));
        let m = mention(&s).unwrap();
        assert_eq!(m.reference, reference);
        assert_eq!(m.taking, Taking::All);
        assert_eq!(m.set.filter, Filter::All(vec![Filter::Atom(Atom::Type(ir::CardType::Land))]));
        assert_eq!(s, read(&format!("all land cards that were {word} this way")));
    }
}

#[test]
fn r608_2c_anaphoric_membership_and_remainder_are_explicit_reference_forms() {
    let m = mention(&read("a creature card from among them")).unwrap();
    assert_eq!(m.reference, UnresolvedRef::cards(RefForm::Anaphor));
    assert_eq!(m.taking, Taking::Exactly(1));
    let rest = read("the rest");
    assert_eq!(rest.quant, Quant::Definite(Ref::Unresolved(UnresolvedRef::cards(RefForm::Remainder))));
    assert_eq!(mention(&rest).unwrap().taking, Taking::Remainder);
    // The explicit group cannot be thrown away and replaced by the reveal frame.
    assert!(mention(&read("the rest of your opponents")).is_none());
    assert!(mention(&read("a card from among cards in your graveyard")).is_none());
}

#[test]
fn r608_2c_reference_lowering_does_not_erase_event_qualifiers() {
    for text in [
        "each creature that died this turn",
        "all cards that weren't exiled this way",
        "all cards they exiled this way",
        "a creature card put into your hand this way",
    ] {
        let s = read(text);
        assert!(mention(&s).is_none(), "must retain the event predicate: {text}");
        let Filter::All(filters) = &s.set.filter else { panic!("{s:?}") };
        assert!(filters.iter().any(|f| matches!(f, Filter::Atom(Atom::Did { .. }))), "{s:?}");
    }
    let m = mention(&read("all cards revealed this way")).unwrap();
    assert_eq!(m.reference.expected, Kind::Cards);
    assert_eq!(m.reference.qualifiers, vec![Qualifier::ThisWay(Verb::Reveal)]);
}

#[test]
fn r608_2c_stat_of_a_result_preserves_its_postmodifier_and_canonical_printing() {
    let rule = &grammar::amount::amounts().amount;
    for text in ["the power of the creature exiled this way", "the mana value of the card revealed this way"] {
        let Outcome::Accepted { value, .. } = rule.parse_all(&oracle::token::tokenize(text)) else { panic!("{text}") };
        assert_eq!(rule.print_text(&value, false).as_deref(), Some(text));
    }
}
