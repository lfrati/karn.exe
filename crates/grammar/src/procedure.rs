//! Procedures: shuffling a named library, and repeating an ability's own instructions.
//!
//! ```text
//! shuffle := "shuffle" [whose "library"]
//! ```

use crate::event::{Form, verb};
use crate::kernel::{P, alt, lazy, lit};
use crate::words::{Number, count_amount, count_atom};
use crate::zone_move::{destination, whose};
use ir::procedure::{PileKind, PileRef, Piles, Repeats, Shuffled};
use ir::{Action, Amount, Destination, Modifier, MoveVerb};

/// The actions of this module, read after the subject's verb form.
pub fn procedure(form: Form) -> P<Action> {
    alt("procedure", vec![shuffle(form), repeat(form), choose_pile(form), move_pile(form)])
}

/// "a face-down pile", "a face-up pile".
fn a_pile() -> P<PileKind> {
    lit("a")
        .then_keep(&alt("facing", vec![lit("face-down").to(PileKind::FaceDown), lit("face-up").to(PileKind::FaceUp)]))
        .skip(&lit("pile"))
}

/// "two piles", "a face-down pile and a face-up pile" (CR 700.3).
pub(crate) fn piles() -> P<Piles> {
    let count = count_amount().skip(&lit("piles")).map(|n| Some(Piles::Count(n)), |p| match p {
        Piles::Count(n) => Some(n.clone()),
        Piles::FaceDownAndUp => None,
    });
    let facing = a_pile().skip(&lit("and")).then(&a_pile()).map(
        |(a, b)| (a == PileKind::FaceDown && b == PileKind::FaceUp).then_some(Piles::FaceDownAndUp),
        |p| (*p == Piles::FaceDownAndUp).then_some((PileKind::FaceDown, PileKind::FaceUp)),
    );
    alt("piles", vec![count, facing])
}

/// "exile them in a face-down pile": the cards exiled are grouped into a pile (CR 700.3).
pub(crate) fn in_pile() -> P<Modifier> {
    lit("in").then_keep(&a_pile()).map(
        |k| Some(Modifier::InPile(k)),
        |m| match m {
            Modifier::InPile(k) => Some(*k),
            _ => None,
        },
    )
}

/// "one pile", "one of those piles", "that pile", "the other".
fn pile_ref() -> P<PileRef> {
    alt(
        "pile",
        vec![
            lit("one pile").to(PileRef::One),
            lit("one of those piles").to(PileRef::OneOfThose),
            lit("that pile").to(PileRef::That),
            lit("the other").to(PileRef::TheOther),
        ],
    )
}

/// "chooses one of those piles" (700.3).
fn choose_pile(form: Form) -> P<Action> {
    verb(form, "chooses", "choose", "").then_keep(&pile_ref()).map(
        |p| (p == PileRef::OneOfThose).then_some(Action::ChoosePile(p)),
        |a| match a {
            Action::ChoosePile(p) if *p == PileRef::OneOfThose => Some(*p),
            _ => None,
        },
    )
}

/// "put one pile into your hand and the other into your graveyard", "put that pile into your
/// hand": the first pile is named, the other is what is left (700.3a: every object is in
/// exactly one pile).
fn move_pile(form: Form) -> P<Action> {
    let to = destination(Number::Sg, MoveVerb::Put);
    let first = pile_ref().then(&to).map(
        |(p, d)| matches!(p, PileRef::One | PileRef::That).then_some((p, d)),
        |(p, d): &(PileRef, Destination)| matches!(p, PileRef::One | PileRef::That).then(|| (*p, d.clone())),
    );
    let other = lit("and").then_keep(&lit("the other").then_keep(&to));
    verb(form, "puts", "put", "").then_keep(&first.then(&other.opt())).map(
        |(first, other)| {
            let mut moves = vec![first];
            moves.extend(other.map(|d| (PileRef::TheOther, d)));
            Some(Action::MovePile(moves))
        },
        |a| match a {
            Action::MovePile(moves) => match moves.as_slice() {
                [first] => Some((first.clone(), None)),
                [first, (PileRef::TheOther, d)] => Some((first.clone(), Some(d.clone()))),
                _ => None,
            },
            _ => None,
        },
    )
}

/// "repeat this process", "repeats this process once", "repeat this process X more times",
/// "repeat this process any number of times", "repeat this process until …".
fn repeat(form: Form) -> P<Action> {
    let times = count_atom().skip(&lit("more times")).map(
        |n| (n != Amount::Const(1)).then_some(Repeats::Times(n)),
        |r| match r {
            Repeats::Times(n) if *n != Amount::Const(1) => Some(n.clone()),
            _ => None,
        },
    );
    // "for an enchantment and a planeswalker", "for artifacts and creatures": kinds of object,
    // one ("an enchantment") or all of a kind ("artifacts"), each a kind of its own (not the
    // union "artifacts and creatures")
    fn plain(set: &ir::ObjectSet) -> bool {
        let union = |f: &ir::Filter| matches!(f, ir::Filter::Any(_));
        !matches!(set.domain, ir::Domain::Union(_)) && match &set.filter {
            ir::Filter::All(v) => !v.iter().any(union),
            f => !union(f),
        }
    }
    fn kind(w: &ir::Who) -> bool {
        match w {
            ir::Who::One(s) => s.quant == ir::Quant::Indefinite(ir::Count::Exactly(Amount::Const(1))) && s.chooser.is_none() && s.random.is_none() && plain(&s.set),
            ir::Who::Set(set) => plain(set),
        }
    }
    let kinds = crate::mana::who().map(|w| kind(&w).then_some(w), |w| kind(w).then(|| w.clone()));
    let others = lit("for").then_keep(&alt("kinds", vec![kinds.map(|w| Some(vec![w]), |v: &Vec<ir::Who>| (v.len() == 1).then(|| v[0].clone())), kinds.list("and")]))
        .map(|v| Some(Repeats::For(v)), |r| match r {
            Repeats::For(v) => Some(v.clone()),
            _ => None,
        });
    let how = alt(
        "repeats",
        vec![
            lit("once").to(Repeats::Times(Amount::Const(1))),
            others,
            times,
            lit("any number of times").to(Repeats::AnyNumber),
            lit("until")
                .then_keep(&lazy("condition", || crate::condition::conditions().condition.clone()))
                .map(|c| Some(Repeats::Until(Box::new(c))), |r| match r {
                    Repeats::Until(c) => Some((**c).clone()),
                    _ => None,
                }),
        ],
    );
    verb(form, "repeats", "repeat", "").skip(&lit("this process")).then(&how.opt()).map(
        |((), how)| Some(Action::Repeat(how.unwrap_or(Repeats::Process))),
        |a| match a {
            Action::Repeat(Repeats::Process) => Some(((), None)),
            Action::Repeat(r) => Some(((), Some(r.clone()))),
            _ => None,
        },
    )
}

/// "shuffle", "shuffles their library" (CR 701.24a). The bare form shuffles the library the
/// ability searched (701.24b), the named one the library named: they differ in meaning.
fn shuffle(form: Form) -> P<Action> {
    let library = verb(form, "shuffles", "shuffle", "").then(&whose(Number::Sg, "library", "libraries").opt()).map(
        |((), library)| Some(Action::Shuffle(library.map_or(Shuffled::Default, Shuffled::Library))),
        |a| match a {
            Action::Shuffle(Shuffled::Default) => Some(((), None)),
            Action::Shuffle(Shuffled::Library(w)) => Some(((), Some(w.clone()))),
            _ => None,
        },
    );
    // a face-down pile is shuffled like a library (701.24a)
    let pile = verb(form, "shuffles", "shuffle", "").then_keep(&lit("that pile")).map(
        |()| Some(Action::Shuffle(Shuffled::Pile(PileRef::That))),
        |a| matches!(a, Action::Shuffle(Shuffled::Pile(PileRef::That))).then_some(()),
    );
    alt("shuffle", vec![library, pile])
}

#[cfg(test)]
mod tests {
    use crate::effect::effects;
    use crate::kernel::Outcome;
    use oracle::token::tokenize;

    fn round_trips(text: &str) {
        let s = &effects().sentence;
        match s.parse_all(&tokenize(text)) {
            Outcome::Accepted { value, .. } => assert_eq!(s.print_text(&value, true).as_deref(), Some(text), "{value:#?}"),
            other => panic!("{text}: {other:#?}"),
        }
    }

    fn declined(text: &str) {
        assert!(matches!(effects().sentence.parse_all(&tokenize(text)), Outcome::Declined { .. }), "{text} must not read");
    }

    /// CR 701.24a: a library is shuffled; a bare "shuffle" and a named library are two readings.
    #[test]
    fn r701_24_shuffle_a_named_library() {
        for text in [
            "Shuffle",
            "Shuffle your library",
            "Target player shuffles their library",
            "Target player shuffles",
            "Shuffle your library, then exile the top card of your library",
        ] {
            round_trips(text);
        }
        for text in ["Shuffle target creature's library", "Shuffle your graveyard"] {
            declined(text);
        }
    }

    /// CR 401.4, 401.7: a position in a library, with the library said or implied, and an order
    /// only where cards share a position.
    #[test]
    fn r401_4_where_in_a_library_and_in_what_order() {
        for text in [
            "Put the rest on the bottom of your library in a random order",
            "Put the rest on the bottom in a random order",
            "Put it on top",
            "Put that card third from the top",
            "Put them back in any order",
            "Put it back",
            "Put one of those cards back on top of your library",
            "Put it into its owner's library second from the top or on the bottom",
            "Put that card on your choice of the top or bottom of its owner's library",
            "Put it on their choice of the top or bottom of their library",
            "Search your library for a card, then shuffle and put that card on top",
            "Put one on top of your library and the rest on the bottom in a random order",
        ] {
            round_trips(text);
        }
        for text in [
            "Put it third from the top in any order",
            "Put it into your graveyard in any order",
            "Put it on your choice of the top of your library",
            "Put it second from the top of your library",
        ] {
            declined(text);
        }
    }

    /// CR 903.9, 400.7: where cards are moved from, said after the move for objects that name no
    /// zone; the commander's zone is a destination.
    #[test]
    fn r400_7_a_move_from_a_zone_said_after_it() {
        for text in [
            "Put your commander into your hand from the command zone",
            "Put ~ onto the battlefield from the command zone",
            "Put target creature card into your hand from your graveyard",
            "Put a creature card onto the battlefield from your hand or graveyard",
            "Shuffle ~ into your library from your graveyard",
            "Return each commander they control from the battlefield to the command zone",
            "Put your commander into the command zone",
            "Return target card from your graveyard to your hand",
        ] {
            round_trips(text);
        }
        // the zone is said once: before the move or after it
        for text in [
            "Put target creature card from your graveyard onto the battlefield from your graveyard",
            "Put your commander into your hand and ~ onto the battlefield from the command zone",
        ] {
            declined(text);
        }
    }

    /// CR 700.3: objects grouped into piles are separated, a pile is chosen, a pile is moved as
    /// a whole; 701.24a: a face-down pile is shuffled.
    #[test]
    fn r700_3_piles() {
        for text in [
            "Separate all permanents target player controls into two piles",
            "Look at the top four cards of your library and separate them into a face-down pile and a face-up pile",
            "An opponent chooses one of those piles",
            "Put that pile into your hand and the other into your graveyard",
            "Put one pile into your hand and the other on the bottom of your library in any order",
            "Exile the top four cards of your library in a face-down pile, then exile the top four cards of your library in a face-up pile",
            "Exile all creature cards from target player's graveyard in a face-down pile, shuffle that pile, then manifest those cards",
        ] {
            round_trips(text);
        }
        // a pile is not an object (700.3b): it is not something to draw, to choose as a creature
        for text in [
            "Draw a card in a face-down pile",
            "Target creature chooses one of those piles",
            "Put the other into your graveyard and one pile into your hand",
            "Separate them into a face-up pile and a face-down pile",
        ] {
            declined(text);
        }
    }

    /// CR 608.2c: the instructions are followed in the order written; "repeat this process"
    /// follows them again, after any instruction, with or without a condition in front.
    #[test]
    fn r608_2c_repeat_this_process() {
        for text in [
            "Repeat this process",
            "Repeat this process once",
            "Repeat this process any number of times",
            "Repeat this process X more times",
            "Mill two cards, then repeat this process",
            "Draw a card and repeat this process",
            "You may repeat this process any number of times",
            "If you win, repeat this process",
            "If you control eight or more lands, repeat this process once",
            "Target opponent mills a card, then repeats this process",
            "Then repeat this process six more times",
        ] {
            round_trips(text);
        }
        for text in ["Repeat target creature", "Repeat this", "Target creature repeats this process", "Repeat this process one more time"] {
            declined(text);
        }
    }
}
