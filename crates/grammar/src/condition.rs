//! Conditions and activation restrictions.
//!
//! ```text
//! condition := clause | clause "and" clause … | clause "or" clause …
//! clause    := player "control(s)" selector | "there are" selector | player "ha(s|ve)" selector
//!            | object "is" predicate | object "remain(s)" predicate | past-event [window]
//!            | object "was kicked" | "it's" possessive "turn" | "you're the monarch"
//!            | amount "is" comparison | player "ha(s|ve)" N "or less life" | player "pay(s)" cost
//!            | "you win the flip" | player "controls more" set "than" player | object "shares a"
//!            characteristic "with" object | "this is the" ORDINAL "time this ability has resolved
//!            this turn" | mana "was spent to cast" object | past-tense clause (last known information)
//! restriction := "only as a sorcery" | "only once each turn" | "only during …" | "only if" condition
//! ```

use crate::amount::amounts;
use crate::event::{Events, Form, during, events, ordinal, step};
use crate::kernel::{P, Val, alt, forward, leaf, lit, table};
use crate::mana::mana_cost;
use crate::selector::{adjective_group, counter_kind, counters_body, is_plural, keyword_name, selectors, stat};
use crate::words::Number;
use crate::words::{count_amount, count_atom, stat_amount};
use ir::condition::{Contest, DayNight, ManaKind, Measure, ObjectDesignation, Outcome, SpentMana, Than};
use ir::event::{Quantity, Step, Tense};
use ir::lexicon::Verb;
use ir::selector::{Anaphor, Chosen};
use ir::{
    Action, Aggregate, Amount, Atom, CastFact, Characteristic, Cmp, Color, Condition, Count, Designation, Domain, EventAction, EventPattern,
    Filter, ObjectSet, Predicate, Quant, Ref, Restriction, Selector, Window, Zone, ZoneSpec,
};
use oracle::token::Kind;
use std::sync::OnceLock;

pub struct Conditions {
    pub condition: P<Condition>,
    /// "only as a sorcery and only once each turn".
    pub restrictions: P<Vec<Restriction>>,
}

pub fn conditions() -> &'static Conditions {
    events_and_conditions().1
}

/// Events and conditions refer to each other ("attacks while saddled"; "if a creature died this
/// turn"), so they are built together, the conditions through a forward rule.
pub(crate) fn events_and_conditions() -> (&'static Events, &'static Conditions) {
    static G: OnceLock<(Events, Conditions)> = OnceLock::new();
    let (e, c) = G.get_or_init(|| {
        let condition = forward::<Condition>("condition");
        let e = crate::event::build(&condition.rule());
        let c = build(&e);
        condition.define(c.condition.clone());
        (e, c)
    });
    (e, c)
}

fn they() -> Selector {
    Selector::new(Quant::Definite(Ref::Anaphor(Anaphor::They)), ObjectSet::of(Domain::Player))
}

fn it() -> Selector {
    Selector::new(Quant::Definite(Ref::Anaphor(Anaphor::It)), ObjectSet::of(Domain::Object))
}

fn this() -> Selector {
    Selector::new(Quant::Definite(Ref::This), ObjectSet::of(Domain::Permanent))
}

fn is_it(s: &Selector) -> bool {
    s.quant == Quant::Definite(Ref::Anaphor(Anaphor::It))
}

/// A subject with its verb: `base` after "you", "they" and plurals, `third` after singulars.
fn subject_verb<T: Val>(base: &str, third: &str, then: &P<T>) -> P<(Selector, T)> {
    let s = selectors();
    let base_subject = alt("subject", vec![lit("you").to(Selector::you()), lit("they").to(they()), s.plural.clone()]);
    let players = |p: &P<Selector>| {
        p.map(
            |s| (s.set.domain == Domain::Player).then_some(s),
            |s| (s.set.domain == Domain::Player).then(|| s.clone()),
        )
    };
    alt(
        "subject and verb",
        vec![players(&base_subject).skip(&lit(base)).then(then), players(&s.singular).skip(&lit(third)).then(then)],
    )
}

/// An object subject with its verb: `third` after singulars (and "it"), `base` after plurals and
/// "they" (objects).
fn object_verb<T: Val>(base: &str, third: &str, then: &P<T>, then_pl: &P<T>) -> P<(Selector, T)> {
    let s = selectors();
    let they_objects = Selector::new(Quant::Definite(Ref::Anaphor(Anaphor::They)), ObjectSet::of(Domain::Object));
    let not_player = |p: &P<Selector>| {
        p.map(
            |s| (s.set.domain != Domain::Player).then_some(s),
            |s| (s.set.domain != Domain::Player).then(|| s.clone()),
        )
    };
    let singular = not_player(&alt("object", vec![lit("it").to(it()), s.singular.clone()]));
    let plural = not_player(&alt("objects", vec![lit("they").to(they_objects), s.plural.clone()]));
    alt("object and verb", vec![singular.skip(&lit(third)).then(then), plural.skip(&lit(base)).then(then_pl)])
}

fn clitic_s() -> P<()> {
    leaf("'s", |k| matches!(k, Kind::Clitic(c) if c == "'s").then_some(()), |()| Some(Kind::Clitic("'s".into())))
}

/// "is" / "isn't", and "it's" / "it isn't" for the pronoun.
fn object_is() -> P<(Selector, bool)> {
    let s = selectors();
    let copula = alt("is", vec![lit("is").to(false), lit("isn't").to(true), lit("is not").to(true).alias()]);
    let its = lit("it").then_keep(&clitic_s()).map(|()| Some((it(), false)), |(s, n): &(Selector, bool)| {
        (is_it(s) && !n).then_some(())
    });
    let its_not = lit("it").then_keep(&clitic_s()).skip(&lit("not")).map(|()| Some((it(), true)), |_| None).alias();
    let it_isnt =
        lit("it isn't").map(|()| Some((it(), true)), |(s, n): &(Selector, bool)| (is_it(s) && *n).then_some(()));
    let other = s.singular.then(&copula).map(
        |(s, n)| (!is_it(&s)).then_some((s, n)),
        |(s, n)| (!is_it(s)).then(|| (s.clone(), *n)),
    );
    alt("object is", vec![its, it_isnt, its_not, other])
}

/// "are" / "aren't" after a plural, and "they're" / "they aren't".
fn objects_are() -> P<(Selector, bool)> {
    let s = selectors();
    let they = Selector::new(Quant::Definite(Ref::Anaphor(Anaphor::They)), ObjectSet::of(Domain::Object));
    let t2 = they.clone();
    let copula = alt("are", vec![lit("are").to(false), lit("aren't").to(true)]);
    let theyre = alt("they're", vec![lit("they're").to(false), lit("they aren't").to(true)]).map(
        move |n| Some((they.clone(), n)),
        move |(s, n): &(Selector, bool)| (*s == t2).then_some(*n),
    );
    let plural = s.plural.then(&copula).map(
        |(s, n)| (s.set.domain != Domain::Player).then_some((s, n)),
        |(s, n)| (s.set.domain != Domain::Player).then(|| (s.clone(), *n)),
    );
    alt("objects are", vec![theyre, plural])
}

/// "was" / "wasn't": "it was a creature card", "that land was nonbasic".
fn object_was() -> P<(Selector, bool)> {
    let s = selectors();
    let copula = alt("was", vec![lit("was").to(false), lit("wasn't").to(true)]);
    alt("object", vec![lit("it").to(it()), s.singular.clone()]).then(&copula)
}

/// What "is" / "are" asserts, in the subject's number: "a creature" / "Islands", "a mana
/// ability" / "mana abilities".
fn predicate(n: Number) -> P<Predicate> {
    let s = selectors();
    let a = alt("a", vec![lit("a"), lit("an").alias()]);
    let set = match n {
        Number::Sg => a.then_keep(&s.set_sg),
        Number::Pl => s.set_pl.clone(),
    };
    let exile = ZoneSpec { zones: ir::Coord::one(Zone::Exile), owner: None, slice: None };
    let designation = table(
        "designation",
        vec![
            ("monstrous".into(), ObjectDesignation::Monstrous),
            ("renowned".into(), ObjectDesignation::Renowned),
            ("suspended".into(), ObjectDesignation::Suspended),
            ("prepared".into(), ObjectDesignation::Prepared),
            ("solved".into(), ObjectDesignation::Solved),
        ],
    );
    let place = alt(
        "place",
        vec![
            crate::selector::exiled_with().map(
                |f| Some(Predicate::Adjective(f)),
                |p| match p { Predicate::Adjective(f) => Some(f.clone()), _ => None },
            ),
            // "exiled" is how Oracle says "in exile" after a verb
            lit("exiled").to(Predicate::In(exile)),
            s.zone.map(
                |z| Some(Predicate::In(z)),
                |p| match p {
                    Predicate::In(z) => Some(z.clone()),
                    _ => None,
                },
            ),
        ],
    );
    alt(
        "predicate",
        vec![
            adjective_group().map(
                |f| Some(Predicate::Adjective(f)),
                |p| match p {
                    Predicate::Adjective(f) => Some(f.clone()),
                    _ => None,
                },
            ),
            set.map(
                |set| Some(Predicate::A(set)),
                |p| match p {
                    Predicate::A(set) => Some(set.clone()),
                    _ => None,
                },
            ),
            place.clone(),
            // "is level 2 or greater", "is level 1": a Class's level (716.2a)
            lit("level").then_keep(&crate::words::digits()).skip(&lit("or greater")).map(
                |n| Some(Predicate::Adjective(Filter::Atom(Atom::Level(ir::selector::Count::AtLeast(ir::Amount::Const(n)))))),
                |p| match p {
                    Predicate::Adjective(Filter::Atom(Atom::Level(ir::selector::Count::AtLeast(ir::Amount::Const(n))))) => Some(*n),
                    _ => None,
                },
            ),
            lit("level").then_keep(&crate::words::digits()).map(
                |n| Some(Predicate::Adjective(Filter::Atom(Atom::Level(ir::selector::Count::Precisely(ir::Amount::Const(n)))))),
                |p| match p {
                    Predicate::Adjective(Filter::Atom(Atom::Level(ir::selector::Count::Precisely(ir::Amount::Const(n))))) => Some(*n),
                    _ => None,
                },
            ),
            // "in the command zone or on the battlefield"
            place.list("or").map(
                |v| Some(Predicate::Any(v)),
                |p| match p {
                    Predicate::Any(v) => Some(v.clone()),
                    _ => None,
                },
            ),
            lit("attached to").then_keep(&s.selector).map(
                |x| Some(Predicate::Adjective(Filter::Atom(Atom::AttachedTo(Box::new(x))))),
                |p| match p {
                    Predicate::Adjective(Filter::Atom(Atom::AttachedTo(x))) => Some((**x).clone()),
                    _ => None,
                },
            ),
            lit("paired with").then_keep(&s.selector).map(
                |x| Some(Predicate::PairedWith(Box::new(x))),
                |p| match p {
                    Predicate::PairedWith(x) => Some((**x).clone()),
                    _ => None,
                },
            ),
            lit("in your opening hand").to(Predicate::InOpeningHand),
            // 506.5: only attacking and blocking can be alone
            alt(
                "alone",
                vec![
                    lit("attacking alone").to(Predicate::Alone(ir::selector::Status::Attacking)),
                    lit("blocking alone").to(Predicate::Alone(ir::selector::Status::Blocking)),
                ],
            ),
            lit("your commander").to(Predicate::CommanderOf(Box::new(Selector::you()))),
            designation.map(
                |d| Some(Predicate::Designated(d)),
                |p| match p {
                    Predicate::Designated(d) => Some(*d),
                    _ => None,
                },
            ),
        ],
    )
}

/// "N or more", "N or less", "less than N", "greater than N", "equal to N". `greater`: the "or
/// more" comparison is spelled "or greater" (after a characteristic: "~'s power is 4 or
/// greater"; after a number: "X is 5 or more"). `words`: a number after "less than",
/// "greater than" or "equal to" is spelled as a word ("your devotion to blue is less than
/// five"), not as a numeral ("your speed is less than 4"); either is read.
fn comparison(greater: bool, words: bool) -> P<(Cmp, Amount)> {
    let a = &alt("amount", if words { vec![count_amount(), amounts().amount.clone()] } else { vec![amounts().amount.clone(), count_amount()] });
    let n = stat_amount();
    let (more, more_alias) = if greater { ("or greater", "or more") } else { ("or more", "or greater") };
    alt(
        "comparison",
        vec![
            n.skip(&lit(more)).map(|v| Some((Cmp::Ge, v)), |(c, v)| (*c == Cmp::Ge).then(|| v.clone())),
            n.skip(&lit(more_alias)).map(|v| Some((Cmp::Ge, v)), |_: &(Cmp, Amount)| None).alias(),
            n.skip(&lit("or less")).map(|v| Some((Cmp::Le, v)), |(c, v)| (*c == Cmp::Le).then(|| v.clone())),
            n.skip(&lit("or fewer")).map(|v| Some((Cmp::Le, v)), |_: &(Cmp, Amount)| None).alias(),
            lit("less than or equal to")
                .then_keep(a)
                .map(|v| Some((Cmp::Le, v)), |(c, v)| (*c == Cmp::Le).then(|| v.clone())),
            lit("greater than or equal to")
                .then_keep(a)
                .map(|v| Some((Cmp::Ge, v)), |(c, v)| (*c == Cmp::Ge).then(|| v.clone())),
            lit("less than").then_keep(a).map(|v| Some((Cmp::Lt, v)), |(c, v)| (*c == Cmp::Lt).then(|| v.clone())),
            lit("greater than").then_keep(a).map(|v| Some((Cmp::Gt, v)), |(c, v)| (*c == Cmp::Gt).then(|| v.clone())),
            lit("equal to")
                .then_keep(a)
                .map(|v| Some((Cmp::Eq, v)), |(c, v)| (*c == Cmp::Eq && !literal(v)).then(|| v.clone())),
            // a number alone is equal to it ("X is 3", "its power is 2"); "exactly" says it again
            n.map(|v| Some((Cmp::Eq, v)), |(c, v)| (*c == Cmp::Eq && literal(v)).then(|| v.clone())),
            lit("exactly").then_keep(&n).map(|v| Some((Cmp::Eq, v)), |_: &(Cmp, Amount)| None).alias(),
        ],
    )
}

/// A number said as itself: a literal or X.
fn literal(a: &Amount) -> bool {
    matches!(a, Amount::Const(_) | Amount::X)
}

/// Is this amount a characteristic of objects (power, mana value), whose "or more" Oracle spells
/// "or greater"?
fn is_stat(a: &Amount) -> bool {
    matches!(a, Amount::Stat { .. } | Amount::Aggregate { .. })
}

/// "a +1/+1 counter on it", "three or more +1/+1 counters on it", "counters on it", "no time
/// counters on it": what "has" / "had" asserts about counters.
fn has_counters() -> P<Filter> {
    let kind = counter_kind().opt();
    let mk = |kind: Option<ir::CounterKind>, count: Count| Some(Filter::Atom(Atom::Counters { kind, count }));
    let bare = kind.skip(&lit("counters on it")).map(
        // "no counters on it" is a count, not a kind
        move |k| if k.as_ref().is_some_and(|k| k.0 == "no") { None } else { mk(k, Count::AtLeast(Amount::Const(1))) },
        |f| match f {
            Filter::Atom(Atom::Counters { kind, count: Count::AtLeast(Amount::Const(1)) }) => Some(kind.clone()),
            _ => None,
        },
    );
    let none = lit("no").then_keep(&kind).skip(&lit("counters on it")).map(
        move |k| mk(k, Count::Exactly(Amount::Const(0))),
        |f| match f {
            Filter::Atom(Atom::Counters { kind, count: Count::Exactly(Amount::Const(0)) }) => Some(kind.clone()),
            _ => None,
        },
    );
    alt("counters", vec![bare, none, counters_body(Number::Sg)])
}

/// What "has" asserts of an object: a keyword, counters, a characteristic value ("mana value 3
/// or less").
pub(crate) fn has_what() -> P<Filter> {
    let stat_cmp = stat().then(&comparison(true, true)).map(
        |(stat, (cmp, value))| Some(Filter::Atom(Atom::Stat { stat, cmp, value })),
        |f| match f {
            Filter::Atom(Atom::Stat { stat, cmp, value }) => Some((*stat, (*cmp, value.clone()))),
            _ => None,
        },
    );
    alt(
        "has what",
        vec![
            keyword_name().map(
                |k| Some(Filter::Atom(Atom::Keyword(k))),
                |f| match f {
                    Filter::Atom(Atom::Keyword(k)) => Some(*k),
                    _ => None,
                },
            ),
            has_counters(),
            stat_cmp,
            lit("the chosen name").to(Filter::Atom(Atom::Chosen(Chosen::Name))),
            // "it has the same name as that card"
            lit("the same")
                .then_keep(&crate::selector::same_word())
                .skip(&lit("as"))
                .then(&crate::kernel::lazy("selector", || selectors().selector.clone()))
                .map(
                    |(what, with)| Some(Filter::Atom(Atom::Shares { what, with: Box::new(with) })),
                    |f| match f {
                        Filter::Atom(Atom::Shares { what, with }) => Some((*what, (**with).clone())),
                        _ => None,
                    },
                ),
        ],
    )
}

/// Does the pattern ask how a definite object came to be (its casting, its entering), an event
/// that happens once to it and so needs no window ("if you cast it", "~ was cast from a
/// graveyard", "it entered under your control")?
fn origin(p: &EventPattern) -> bool {
    let definite = |s: &Selector| matches!(s.quant, Quant::Definite(_)) && s.set.domain != Domain::Player;
    p.nth_each_turn.is_none()
        && p.actions.iter().all(|a| match a {
            // "you cast it"; "it was cast from a graveyard"
            EventAction::Deed(d) if d.verb == Verb::Cast && !d.passive => d.what().is_some_and(definite),
            EventAction::Deed(d) if d.verb == Verb::Cast => !p.subjects.is_empty() && p.subjects.iter().all(definite),
            EventAction::Deed(d) if d.verb == Verb::Enter => !p.subjects.is_empty() && p.subjects.iter().all(definite),
            _ => false,
        })
}

/// The bounded span shared by historical events and control acquisitions.
fn history_window() -> P<Window> {
    let s = selectors();
    // the most recent step or turn of a player, which "since" counts from
    let since_step = table(
        "step",
        vec![
            ("turn".into(), Step::Turn),
            ("upkeep".into(), Step::Upkeep),
            ("draw step".into(), Step::Draw),
            ("end step".into(), Step::End),
        ],
    );
    alt(
        "window",
        vec![
            lit("this turn").to(Window::ThisTurn),
            lit("last turn").to(Window::LastTurn),
            lit("this step").to(Window::ThisStep),
            lit("this combat").to(Window::ThisCombat),
            lit("this way").to(Window::ThisWay),
            lit("this game").to(Window::ThisGame),
            lit("that turn").to(Window::ThatTurn),
            lit("during").then_keep(&s.possessive).skip(&lit("last turn")).map(
                |who| Some(Window::LastTurnOf(Box::new(who))),
                |w| match w {
                    Window::LastTurnOf(who) => Some((**who).clone()),
                    _ => None,
                },
            ),
            alt("since", vec![lit("since"), lit("since the beginning of").alias()]).then_keep(&s.possessive).skip(&lit("last")).then(&since_step).map(
                |(who, step)| Some(Window::Since { step, whose: Box::new(who) }),
                |w| match w {
                    Window::Since { step, whose } => Some(((**whose).clone(), *step)),
                    _ => None,
                },
            ),
        ],
    )
}

/// History: an event pattern in a tense, with its window.
fn happened(e: &Events, inherited_plural: Option<bool>) -> P<Condition> {
    let s = selectors();
    let window = history_window();
    // "four or more times", "exactly two times": how often, in the window
    let times = alt(
        "times",
        vec![
            count_atom().skip(&lit("or more times")).map(
                |n| Some(Quantity::AtLeast(n)),
                |q| match q {
                    Quantity::AtLeast(n) => Some(n.clone()),
                    _ => None,
                },
            ),
            lit("exactly").then_keep(&count_atom()).skip(&lit("times")).map(
                |n| Some(Quantity::Exactly(n)),
                |q| match q {
                    Quantity::Exactly(n) => Some(n.clone()),
                    _ => None,
                },
            ),
        ],
    );
    // a window word right after the pattern is the window, never a duration of what follows
    let any_window = window.map(|_| Some(()), |()| None);
    type Said = (EventPattern, (Option<Quantity>, Option<Window>));
    let with_window = |p: &P<EventPattern>, tense: Tense, negated: bool| {
        // Historical excess damage already has the quantitative Compare owner
        // below. Do not give the same words a second Happened-event meaning;
        // present "is dealt ... this way" remains the reflexive occurrence form.
        let historical_excess = move |pattern: &EventPattern| tense != Tense::Present
            && pattern.actions.iter().any(|a| matches!(a, EventAction::IsDealtDamage { excess: true, .. }));
        let timeless = p.not_before(&any_window).map(
            |x| Some((x, (None, None))),
            |(x, (t, w)): &Said| (w.is_none() && t.is_none()).then(|| x.clone()),
        );
        let windowed = p.then(&times.opt()).then(&window).map(
            |((x, t), w)| Some((x, (t, Some(w)))),
            |(x, (t, w)): &Said| w.clone().map(|w| ((x.clone(), t.clone()), w)),
        );
        // A temporal adjunct may precede the passive's source. Keep it in the history
        // window and print the ordinary source-before-window ordering.
        let late_source = windowed.then(&lit("by").then_keep(&s.selector)).map(
            |((mut pattern, when), source)| {
                match pattern.actions.as_mut_slice() {
                    [EventAction::IsDealtDamage { by, .. }] if by.is_empty()
                        && ir::lexicon::fills(&source, ir::Verb::DealDamage, ir::Role::By) => by.push(source),
                    _ => return None,
                }
                Some((pattern, when))
            }, |_| None,
        ).alias();
        alt("history", vec![windowed, late_source, timeless]).map(
            move |(pattern, (times, window))| {
                (!historical_excess(&pattern) && (window.is_some() || origin(&pattern)))
                    .then_some(Condition::Happened { pattern, window, times, tense, negated })
            },
            move |c| match c {
                Condition::Happened { pattern, window, times, tense: t, negated: n }
                    if *t == tense && *n == negated && !historical_excess(pattern) && (window.is_some() || origin(pattern)) =>
                {
                    Some((pattern.clone(), (times.clone(), window.clone())))
                }
                _ => None,
            },
        )
    };
    if let Some(plural) = inherited_plural {
        let predicate = crate::event::past_predicate(plural).map(
            |action| Some(EventPattern::new(Vec::new(), vec![action])), |_| None,
        );
        return with_window(&predicate, Tense::Past, false);
    }
    // "that spell is countered this way": the present, for the resolving ability's own events;
    // where the present reads the same as the past ("you cast a spell this way"), it is the past
    let present = e.pattern.skip(&lit("this way")).map(
        |pattern| {
            let past = events().past.print_text(&pattern, false);
            (past.is_none() || past != events().pattern.print_text(&pattern, false)).then_some(Condition::Happened {
                pattern,
                window: Some(Window::ThisWay),
                times: None,
                tense: Tense::Present,
                negated: false,
            })
        },
        |c| match c {
            Condition::Happened {
                pattern,
                window: Some(Window::ThisWay),
                times: None,
                tense: Tense::Present,
                negated: false,
            } => {
                Some(pattern.clone())
            }
            _ => None,
        },
    );
    alt(
        "happened",
        vec![
            with_window(&e.past, Tense::Past, false),
            with_window(&e.perfect, Tense::Perfect, false),
            with_window(&e.past_not, Tense::Past, true),
            with_window(&e.perfect_not, Tense::Perfect, true),
            present,
        ],
    )
}

/// "~ was kicked", "the gift wasn't promised", "~'s madness cost was paid", "it escaped".
fn cast_was() -> P<Condition> {
    let s = selectors();
    let was = alt("was", vec![lit("was").to(false), lit("wasn't").to(true)]);
    let object = alt("object", vec![lit("it").to(it()), s.singular.clone()]);
    let fact = table(
        "cast fact",
        vec![
            ("kicked".into(), CastFact::Kicked),
            ("bargained".into(), CastFact::Bargained),
            ("foretold".into(), CastFact::Foretold),
            ("cast using teamwork".into(), CastFact::Teamwork),
        ],
    );
    let mk = |((what, negated), fact): ((Selector, bool), CastFact)| {
        Some(Condition::CastWas { what: Box::new(what), fact, negated })
    };
    let un = |c: &Condition| match c {
        Condition::CastWas { what, fact, negated } => Some((((**what).clone(), *negated), fact.clone())),
        _ => None,
    };
    let subject_was = object.then(&was).then(&fact).map(mk, un);
    let kicked_with = object.skip(&lit("was kicked with its")).then(&mana_cost()).skip(&lit("kicker")).map(
        |(what, cost)| Some(Condition::CastWas { what: Box::new(what), fact: CastFact::KickedWith(cost), negated: false }),
        |c| match c {
            Condition::CastWas { what, fact: CastFact::KickedWith(cost), negated: false } => {
                Some(((**what).clone(), cost.clone()))
            }
            _ => None,
        },
    );
    // "~'s additional cost was paid", "its prowl cost was paid"
    let whose = alt("whose", vec![lit("its").to(it()), object.skip(&clitic_s())]);
    let cost = alt(
        "cost",
        vec![
            lit("additional cost").to(CastFact::AdditionalCostPaid),
            keyword_name().skip(&lit("cost")).map(
                |k| Some(CastFact::CostPaid(k)),
                |f| match f {
                    CastFact::CostPaid(k) => Some(*k),
                    _ => None,
                },
            ),
        ],
    );
    let paid = alt("was paid", vec![lit("was paid").to(false), lit("wasn't paid").to(true)]);
    let cost_paid = whose.then(&cost).then(&paid).map(
        |((what, fact), negated)| Some(Condition::CastWas { what: Box::new(what), fact, negated }),
        |c| match c {
            Condition::CastWas { what, fact: f @ (CastFact::AdditionalCostPaid | CastFact::CostPaid(_)), negated } => {
                Some((((**what).clone(), f.clone()), *negated))
            }
            _ => None,
        },
    );
    // "that creature was cast for its warp cost"
    let cast_for = object.then(&was).skip(&lit("cast for its")).then(&keyword_name()).skip(&lit("cost")).map(
        |((what, negated), k)| Some(Condition::CastWas { what: Box::new(what), fact: CastFact::CastFor(k), negated }),
        |c| match c {
            Condition::CastWas { what, fact: CastFact::CastFor(k), negated } => Some((((**what).clone(), *negated), *k)),
            _ => None,
        },
    );
    // "the {2}{U} cost was paid": the spell was cast paying that cost (601.2b), of the spell itself
    let mana_paid = lit("the").then_keep(&mana_cost()).skip(&lit("cost")).then(&paid).map(
        |(cost, negated)| Some(Condition::CastWas { what: Box::new(this()), fact: CastFact::ManaCostPaid(cost), negated }),
        |c| match c {
            Condition::CastWas { what, fact: CastFact::ManaCostPaid(cost), negated } if **what == this() => {
                Some((cost.clone(), *negated))
            }
            _ => None,
        },
    );
    let escaped = object.skip(&lit("escaped")).map(
        |what| Some(Condition::CastWas { what: Box::new(what), fact: CastFact::Escaped, negated: false }),
        |c| match c {
            Condition::CastWas { what, fact: CastFact::Escaped, negated: false } => Some((**what).clone()),
            _ => None,
        },
    );
    // of the spell or permanent itself, with no subject: "the gift was promised"
    let own = |yes: &str, no: &str, fact: CastFact| {
        let f2 = fact.clone();
        alt("fact", vec![lit(yes).to(false), lit(no).to(true)]).map(
            move |negated| Some(Condition::CastWas { what: Box::new(this()), fact: fact.clone(), negated }),
            move |c| match c {
                Condition::CastWas { what, fact, negated } if **what == this() && *fact == f2 => Some(*negated),
                _ => None,
            },
        )
    };
    // "a Dragon was beheld" (of the spell whose behold cost it was)
    let a = alt("a", vec![lit("a"), lit("an").alias()]);
    let beheld = a.then_keep(&s.set_sg).skip(&lit("was beheld")).map(
        |set| Some(Condition::CastWas { what: Box::new(this()), fact: CastFact::Beheld(set), negated: false }),
        |c| match c {
            Condition::CastWas { what, fact: CastFact::Beheld(set), negated: false } if **what == this() => {
                Some(set.clone())
            }
            _ => None,
        },
    );
    // "it's bargained": the same as "it was bargained"
    let its = lit("it").then_keep(&clitic_s()).then_keep(&fact).map(
        |fact| Some(Condition::CastWas { what: Box::new(it()), fact, negated: false }),
        |_| None,
    );
    alt(
        "cast fact",
        vec![
            its.alias(),
            kicked_with,
            subject_was,
            cost_paid,
            cast_for,
            mana_paid,
            escaped,
            own("the gift was promised", "the gift wasn't promised", CastFact::GiftPromised),
            own("tribute was paid", "tribute wasn't paid", CastFact::TributePaid),
            own("evidence was collected", "evidence wasn't collected", CastFact::EvidenceCollected),
            beheld,
        ],
    )
}

/// "{B} was spent to cast ~", "at least three white mana was spent to cast ~".
fn mana_spent() -> P<Condition> {
    let s = selectors();
    let color = table("color", Color::ALL.iter().map(|c| (c.word().to_string(), *c)).collect());
    let a = alt("a", vec![lit("a"), lit("an").alias()]);
    // which mana a count is of: "mana", "white mana", "colorless mana", "colored mana", "mana of
    // the same color", "mana from a Treasure", "mana from creatures"
    let from = a.then_keep(&s.set_sg);
    let kind = alt(
        "kind of mana",
        vec![
            color.skip(&lit("mana")).map(
                |c| Some(ManaKind::Color(c)),
                |k| match k {
                    ManaKind::Color(c) => Some(*c),
                    _ => None,
                },
            ),
            lit("colorless mana").to(ManaKind::Colorless),
            lit("colored mana").to(ManaKind::Colored),
            lit("mana of the same color").to(ManaKind::SameColor),
            lit("mana from").then_keep(&from).map(
                |set| Some(ManaKind::From(set)),
                |k| match k {
                    ManaKind::From(set) => Some(set.clone()),
                    _ => None,
                },
            ),
            lit("mana").to(ManaKind::Any),
        ],
    );
    let mana = alt(
        "mana",
        vec![
            mana_cost().map(
                |c| Some(SpentMana::Symbols(c)),
                |m| match m {
                    SpentMana::Symbols(c) => Some(c.clone()),
                    _ => None,
                },
            ),
            // "mana from a Treasure": at least one
            kind.map(
                |k| {
                    matches!(k, ManaKind::From(_))
                        .then(|| SpentMana::Count { count: Quantity::AtLeast(Amount::Const(1)), kind: k })
                },
                |m| match m {
                    SpentMana::Count { count: Quantity::AtLeast(Amount::Const(1)), kind: k @ ManaKind::From(_) } => {
                        Some(k.clone())
                    }
                    _ => None,
                },
            ),
            // "at least three white mana"; "five or more mana" says it after the number when no
            // kind of mana is named
            lit("at least").then_keep(&count_atom()).then(&kind).map(
                |(n, k)| Some(SpentMana::Count { count: Quantity::AtLeast(n), kind: k }),
                |m| match m {
                    SpentMana::Count { count: Quantity::AtLeast(n), kind } if *kind != ManaKind::Any => {
                        Some((n.clone(), kind.clone()))
                    }
                    _ => None,
                },
            ),
            count_atom().skip(&lit("or more")).then(&kind).map(
                |(n, k)| Some(SpentMana::Count { count: Quantity::AtLeast(n), kind: k }),
                |m| match m {
                    SpentMana::Count { count: Quantity::AtLeast(n), kind: ManaKind::Any } => {
                        Some((n.clone(), ManaKind::Any))
                    }
                    _ => None,
                },
            ),
            lit("exactly").then_keep(&count_atom()).then(&kind).map(
                |(n, k)| (n != Amount::Const(0)).then_some(SpentMana::Count { count: Quantity::Exactly(n), kind: k }),
                |m| match m {
                    SpentMana::Count { count: Quantity::Exactly(n), kind } if *n != Amount::Const(0) => {
                        Some((n.clone(), kind.clone()))
                    }
                    _ => None,
                },
            ),
            lit("no").then_keep(&kind).map(
                |k| Some(SpentMana::Count { count: Quantity::Exactly(Amount::Const(0)), kind: k }),
                |m| match m {
                    SpentMana::Count { count: Quantity::Exactly(Amount::Const(0)), kind } => Some(kind.clone()),
                    _ => None,
                },
            ),
            // "two or more colors of mana were spent": how many colors, not how much mana
            count_atom().skip(&lit("or more colors of mana")).map(
                |n| Some(SpentMana::Colors(Quantity::AtLeast(n))),
                |m| match m {
                    SpentMana::Colors(Quantity::AtLeast(n)) => Some(n.clone()),
                    _ => None,
                },
            ),
            lit("exactly").then_keep(&count_atom()).skip(&lit("colors of mana")).map(
                |n| Some(SpentMana::Colors(Quantity::Exactly(n))),
                |m| match m {
                    SpentMana::Colors(Quantity::Exactly(n)) => Some(n.clone()),
                    _ => None,
                },
            ),
        ],
    );
    // what the text produced is spent later ("is"); what paid for something has been spent ("was"),
    // of mana in quantity "was", of colors "were"
    let was = alt("was", vec![lit("was spent").to(false), lit("wasn't spent").to(true)]);
    let were = lit("were spent").to(false);
    let spent = |m: &P<SpentMana>, verb: &P<bool>, colors: bool| {
        m.then(verb).then(&crate::mana::spent_on()).map(
            move |((mana, negated), on)| {
                (matches!(mana, SpentMana::Colors(_)) == colors).then_some(Condition::ManaSpent { mana, on, negated })
            },
            move |c| match c {
                Condition::ManaSpent { mana, on, negated }
                    if !produced(mana) && matches!(mana, SpentMana::Colors(_)) == colors =>
                {
                    Some((((mana.clone()), *negated), on.clone()))
                }
                _ => None,
            },
        )
    };
    let past = alt("spent", vec![spent(&mana, &was, false), spent(&mana, &were, true)]);
    // "if that mana is spent on a creature spell", "if any of that mana is spent on …"
    let that = alt("that mana", vec![lit("that mana").to(SpentMana::That), lit("this mana").to(SpentMana::That).alias(), lit("any of that mana").to(SpentMana::AnyOfThat)]);
    let present = that.skip(&lit("is spent")).then(&crate::mana::spent_on()).map(
        |(mana, on)| Some(Condition::ManaSpent { mana, on, negated: false }),
        |c| match c {
            Condition::ManaSpent { mana, on, negated: false } if produced(mana) => Some((mana.clone(), on.clone())),
            _ => None,
        },
    );
    alt("mana spent", vec![past, present])
}

fn produced(m: &SpentMana) -> bool {
    matches!(m, SpentMana::That | SpentMana::AnyOfThat)
}

/// "an opponent controls more lands than you", "you have more cards in hand than each opponent".
///
/// The margin is the difference asked for ("at least two more"); none asks for any difference.
fn more_than() -> P<Condition> {
    type Rest = (Measure, bool, Option<Amount>, Than);
    let s = selectors();
    let more = alt("more", vec![lit("more").to(false), lit("fewer").to(true)]);
    let margin = lit("at least").then_keep(&count_atom()).opt();
    let players = lit("than").then_keep(&s.selector).map(
        |p| (p.set.domain == Domain::Player).then(|| Than::Players(Box::new(p))),
        |t: &Than| match t {
            Than::Players(p) => Some((**p).clone()),
            _ => None,
        },
    );
    // a number to measure against, for life: "than your starting life total" (CR 103.4)
    let number = lit("than").then_keep(&amounts().amount).map(
        |a| matches!(a, Amount::StartingLife(_)).then(|| Than::Amount(a)),
        |t: &Than| match t {
            Than::Amount(a) => Some(a.clone()),
            _ => None,
        },
    );
    let than = alt("than", vec![players, number]);
    // the number of such things each player controls or has, against other players'
    let counted = |wrap: fn(ObjectSet) -> Measure, unwrap: fn(&Measure) -> Option<&ObjectSet>| {
        margin.then(&more).then(&s.set_pl).then(&than).map(
            move |(((margin, fewer), set), than)| match than {
                Than::Players(_) => Some((wrap(set), fewer, margin, than)),
                Than::Amount(_) => None,
            },
            move |(m, fewer, margin, than): &Rest| {
                unwrap(m).map(|set| (((margin.clone(), *fewer), set.clone()), than.clone()))
            },
        )
    };
    let controls = counted(Measure::Controls, |m| match m {
        Measure::Controls(set) => Some(set),
        _ => None,
    });
    let has = counted(Measure::Has, |m| match m {
        Measure::Has(set) => Some(set),
        _ => None,
    });
    // life is a mass noun: "more life", "less life"; with a margin the number comes first
    let plain = alt("life", vec![lit("more life").to(false), lit("less life").to(true)]).then(&than).map(
        |(fewer, than)| Some((Measure::Life, fewer, None, than)),
        |(m, fewer, margin, than): &Rest| (*m == Measure::Life && margin.is_none()).then(|| (*fewer, than.clone())),
    );
    let margin_life = lit("at least").then_keep(&stat_amount()).skip(&lit("life more")).then(&than).map(
        |(n, than)| Some((Measure::Life, false, Some(n), than)),
        |(m, fewer, margin, than): &Rest| match (m, margin) {
            (Measure::Life, Some(n)) if !fewer => Some((n.clone(), than.clone())),
            _ => None,
        },
    );
    let life = alt("life", vec![plain, margin_life]);
    let mk = |(player, (measure, fewer, margin, than)): (Selector, Rest)| {
        Some(Condition::MoreThan { player: Box::new(player), measure, fewer, margin, than })
    };
    let un = |c: &Condition| match c {
        Condition::MoreThan { player, measure, fewer, margin, than } => {
            Some(((**player).clone(), (measure.clone(), *fewer, margin.clone(), than.clone())))
        }
        _ => None,
    };
    // "your life total is at least 10 greater than your starting life total": the same question
    let life_total = amounts()
        .amount
        .skip(&lit("is at least"))
        .then(&stat_amount())
        .skip(&lit("greater"))
        .then(&than)
        .map(
            |((left, n), than)| match left {
                Amount::Life(player) => {
                    Some(Condition::MoreThan { player, measure: Measure::Life, fewer: false, margin: Some(n), than })
                }
                _ => None,
            },
            |_| None,
        )
        .alias();
    alt(
        "more than",
        vec![
            subject_verb("control", "controls", &controls).map(mk, un),
            subject_verb("have", "has", &alt("has more", vec![life, has])).map(mk, un),
            life_total,
        ],
    )
}

/// How a condition counts: "two or fewer other lands", "one or fewer cards in hand" (at most N,
/// the "up to" of effects).
fn thresholds() -> P<Selector> {
    let s = selectors();
    let fewer = count_amount().skip(&lit("or fewer")).then(&s.set_pl).map(
        |(n, set)| Some(Selector::new(Quant::Indefinite(Count::UpTo(n)), set)),
        |x| match &x.quant {
            Quant::Indefinite(Count::UpTo(n)) if x.chooser.is_none() && x.random.is_none() => Some((n.clone(), x.set.clone())),
            _ => None,
        },
    );
    // "fewer than seven cards", "more than four cards": whole numbers (CR 107.1), so the same as
    // "six or fewer", "five or more"; only a literal number can be moved by one
    let around = |text: &'static str, step: fn(u32) -> Option<Count>| {
        lit(text)
            .then_keep(&count_amount())
            .then(&alt("things", vec![s.set_pl.clone(), s.set_sg.clone()]))
            .map(
                move |(n, set)| match n {
                    Amount::Const(k) => step(k).map(|count| Selector::new(Quant::Indefinite(count), set)),
                    _ => None,
                },
                |_| None,
            )
            .alias()
    };
    let below = around("fewer than", |k| (k >= 2).then(|| Count::UpTo(Amount::Const(k - 1))));
    let above = around("more than", |k| Some(Count::AtLeast(Amount::Const(k + 1))));
    alt("thresholds", vec![fewer, below, above])
}

/// A selector as a condition counts it.
fn counted_selector() -> P<Selector> {
    alt("counted", vec![thresholds(), selectors().selector.clone()])
}

/// How much damage was dealt, asked of the damage itself: "excess damage was dealt this way",
/// "4 or more damage was dealt to it this turn", "that creature was dealt excess damage this
/// turn", "an opponent was dealt 7 or more combat damage this turn". One value, whichever side
/// is the subject: the damage dealt to something (CR 120, excess damage 120.10) at least that
/// much. The damage's own side is printed when it names nobody or is about what an instruction
/// did ("this way"); the receiver's side when it names who was dealt it.
fn damage_dealt() -> P<Condition> {
    use ir::event::DamageKind;
    use ir::amount::When;
    type Said = (bool, DamageKind, Amount);
    let s = selectors();
    let kind = alt(
        "damage",
        vec![
            lit("combat damage").to(DamageKind::Combat),
            lit("noncombat damage").to(DamageKind::Noncombat),
            lit("damage").to(DamageKind::Any),
        ],
    );
    // excess is at least one over what was lethal (120.10); otherwise at least N
    let quant = alt(
        "how much",
        vec![
            lit("excess").then_keep(&kind).map(
                |k| Some((true, k, Amount::Const(1))),
                |(excess, k, n): &Said| (*excess && *n == Amount::Const(1)).then_some(*k),
            ),
            stat_amount().skip(&lit("or more")).then(&kind).map(
                |(n, k)| Some((false, k, n)),
                |(excess, k, n): &Said| (!excess).then(|| (n.clone(), *k)),
            ),
        ],
    );
    let when = alt("when", vec![lit("this way").to(When::ThisWay), lit("this turn").to(When::ThisTurn)]);
    let receiver = alt("receiver", vec![lit("it").to(it()), s.selector.clone()]);
    let subject = alt(
        "was dealt",
        vec![
            lit("you were").to(Selector::you()),
            lit("they were").to(they()).alias(),
            alt("it", vec![lit("it").to(it()), s.singular.clone()]).skip(&lit("was")).map(
                |x| (x != Selector::you()).then_some(x),
                |x| (*x != Selector::you()).then(|| x.clone()),
            ),
            s.plural.skip(&lit("were")),
        ],
    );
    let mk = |((excess, kind, n), (to, when)): (Said, (Option<Selector>, When))| {
        Condition::Compare {
            left: Amount::Damage { by: None, to: to.map(Box::new), kind, excess, when },
            cmp: Cmp::Ge,
            right: n,
        }
    };
    let un = |c: &Condition| match c {
        Condition::Compare { left: Amount::Damage { by: None, to, kind, excess, when }, cmp: Cmp::Ge, right } => {
            Some((((*excess, *kind, right.clone())), (to.as_deref().cloned(), *when)))
        }
        _ => None,
    };
    // "excess damage was dealt to a creature this way": the damage is the subject
    let damage_first = quant.skip(&lit("was dealt")).then(&lit("to").then_keep(&receiver).opt()).then(&when).map(
        move |((q, to), when)| Some(mk((q, (to, when)))),
        move |c| {
            let (q, (to, when)) = un(c)?;
            (to.is_none() || when == When::ThisWay).then(|| ((q, to), when))
        },
    );
    // "that creature was dealt excess damage this turn": the receiver is the subject
    let receiver_first = subject.skip(&lit("dealt")).then(&quant).then(&when).map(
        move |((to, q), when)| Some(mk((q, (Some(to), when)))),
        move |c| {
            let (q, (to, when)) = un(c)?;
            (when == When::ThisTurn).then_some(to).flatten().map(|to| ((to, q), when))
        },
    );
    alt("damage dealt", vec![damage_first, receiver_first])
}

/// Ranked player measures in conditions use the same metric as noun phrases (119.1).
fn most_life() -> P<Condition> {
    let rank = alt("life rank", vec![lit("most life").to(false), lit("least life").to(true)]);
    let suffix = lit("the").then_keep(&rank)
        .then(&lit("or are tied for").then_keep(&rank).opt()).map(
            |(least, tied)| tied.is_none_or(|t| t == least).then_some((least, tied.is_some())),
            |(least, tied): &(bool, bool)| Some((*least, tied.then_some(*least))),
        );
    subject_verb("have", "has", &suffix).map(
        |(player, (least, tied))| Some(Condition::MostOf { player: Box::new(player), measure: Measure::Life, least, tied }),
        |c| match c {
            Condition::MostOf { player, measure: Measure::Life, least, tied } => Some(((**player).clone(), (*least, *tied))),
            _ => None,
        },
    )
}

fn clause(e: &Events) -> P<Condition> {
    let s = selectors();
    let b = |s: Selector| Box::new(s);
    // "a Plains or a Swamp": alternatives
    let counted = counted_selector();
    // "you control artifacts named A and B": a bare plural, alone, is one or more of them
    let bare = crate::selector::bare_plural().map(|s| Some(ir::Coord::one(s)), |_: &ir::Coord<Selector>| None).alias();
    let what = alt("controlled", vec![crate::action::coord(&counted), bare]);
    let controls_verb = |base: &str, third: &str, negated: bool| {
        subject_verb(base, third, &what).map(
            move |(p, what)| Some(Condition::Controls { player: Box::new(p), what, negated }),
            move |c| match c {
                Condition::Controls { player, what, negated: n } if *n == negated => {
                    Some(((**player).clone(), what.clone()))
                }
                _ => None,
            },
        )
    };
    let controls = alt(
        "controls",
        vec![
            controls_verb("control", "controls", false),
            controls_verb("don't control", "doesn't control", true),
            // "you own a card in exile"
            subject_verb("own", "owns", &what).map(
                |(p, what)| Some(Condition::Owns { player: Box::new(p), what }),
                |c| match c {
                    Condition::Owns { player, what } => Some(((**player).clone(), what.clone())),
                    _ => None,
                },
            ),
            // "you both own and control ~ and a creature named …" (meld, 701.42a)
            subject_verb("both own and control", "both owns and controls", &what).map(
                |(p, what)| Some(Condition::OwnsAndControls { player: Box::new(p), what }),
                |c| match c {
                    Condition::OwnsAndControls { player, what } => Some(((**player).clone(), what.clone())),
                    _ => None,
                },
            ),
            // "an opponent is poisoned"
            subject_verb("are poisoned", "is poisoned", &crate::kernel::empty()).map(
                |(p, ())| Some(Condition::Poisoned(Box::new(p))),
                |c| match c {
                    Condition::Poisoned(p) => Some(((**p).clone(), ())),
                    _ => None,
                },
            ),
            lit("a permanent's ability is countered this way").to(Condition::PermanentsAbilityCountered),
            lit("it isn't being declared as an attacker").to(Condition::NotBeingDeclaredAttacker),
            lit("it hasn't dealt damage yet").to(Condition::HasntDealtDamageYet),
            lit("a creature is attacking you").to(Condition::CreatureAttackingYou),
            lit("~ is in your graveyard with a creature card directly above it").to(Condition::InGraveyardBelowCreature),
        ],
    );
    // "you controlled that permanent": as it was (last known information)
    let controlled = controls_verb("controlled", "controlled", false).map(
        |c| Some(Condition::Was(Box::new(c))),
        |c| match c {
            Condition::Was(c) => Some((**c).clone()),
            _ => None,
        },
    );
    // "there are four or more card types among cards in your graveyard": a count threshold
    let there_count = lit("there are").then_keep(&count_amount()).skip(&lit("or more")).then(&amounts().counted).map(
        |(n, counted)| match counted {
            Amount::Count(_) => None, // a plain set reads as a selector ("there are two or more cards …")
            c => Some(Condition::Compare { left: c, cmp: Cmp::Ge, right: n }),
        },
        |c| match c {
            Condition::Compare { left, cmp: Cmp::Ge, right }
                if !matches!(left, Amount::Count(_) | Amount::Life(_))
                    && !matches!(left, Amount::Counters { on, .. } if on.set.domain == Domain::Player) =>
            {
                Some((right.clone(), left.clone()))
            }
            _ => None,
        },
    );
    // "there are five colors among permanents you control": all five, said without "or more"
    let there_five = lit("there are five")
        .then_keep(&amounts().counted)
        .map(
            |counted| match counted {
                c @ Amount::Distinct { .. } => Some(Condition::Compare { left: c, cmp: Cmp::Ge, right: Amount::Const(5) }),
                _ => None,
            },
            |_: &Condition| None,
        )
        .alias();
    // "there are no charge counters on ~", "there are exactly three tide counters on ~"
    let there_exactly = {
        let n = alt(
            "exactly",
            vec![
                lit("no").to(Amount::Const(0)),
                lit("exactly").then_keep(&count_amount()).map(|n| (n != Amount::Const(0)).then_some(n), |n| {
                    (*n != Amount::Const(0)).then(|| n.clone())
                }),
            ],
        );
        let verb = alt("there are", vec![lit("there are"), lit("there is").alias()]);
        verb.then_keep(&n).then(&amounts().counted).map(
            |(n, counted)| match counted {
                Amount::Counters { .. } => Some(Condition::Compare { left: counted, cmp: Cmp::Eq, right: n }),
                _ => None,
            },
            |c| match c {
                Condition::Compare { left: l @ Amount::Counters { on, .. }, cmp: Cmp::Eq, right }
                    if on.set.domain != Domain::Player =>
                {
                    Some((right.clone(), l.clone()))
                }
                _ => None,
            },
        )
    };
    // "an opponent has three or more poison counters": counters on a player
    let player_counters = {
        let n = alt(
            "counters",
            vec![
                count_amount().skip(&lit("or more")).then(&counter_kind().opt()).skip(&lit("counters")).map(
                    |(n, k)| Some((Cmp::Ge, n, k)),
                    |(c, n, k): &(Cmp, Amount, Option<ir::CounterKind>)| (*c == Cmp::Ge).then(|| (n.clone(), k.clone())),
                ),
                // counts of things are "fewer", not "less"
                count_amount().skip(&lit("or fewer")).then(&counter_kind().opt()).skip(&lit("counters")).map(
                    |(n, k)| Some((Cmp::Le, n, k)),
                    |(c, n, k): &(Cmp, Amount, Option<ir::CounterKind>)| (*c == Cmp::Le).then(|| (n.clone(), k.clone())),
                ),
                lit("fewer than").then_keep(&count_amount()).then(&counter_kind().opt()).skip(&lit("counters")).map(
                    |(n, k)| Some((Cmp::Lt, n, k)),
                    |(c, n, k): &(Cmp, Amount, Option<ir::CounterKind>)| (*c == Cmp::Lt).then(|| (n.clone(), k.clone())),
                ),
                // "has a rad counter": at least one
                alt("a", vec![lit("a"), lit("an").alias()]).then_keep(&counter_kind().opt()).skip(&lit("counter")).map(
                    |k| Some((Cmp::Ge, Amount::Const(1), k)),
                    |_: &(Cmp, Amount, Option<ir::CounterKind>)| None,
                )
                .alias(),
            ],
        );
        subject_verb("have", "has", &n).map(
            |(p, (cmp, right, kind))| {
                Some(Condition::Compare { left: Amount::Counters { on: Box::new(p), kind }, cmp, right })
            },
            |c| match c {
                Condition::Compare { left: Amount::Counters { on, kind }, cmp, right } if on.set.domain == Domain::Player => {
                    Some(((**on).clone(), (*cmp, right.clone(), kind.clone())))
                }
                _ => None,
            },
        )
    };
    // "you have six or more unspent mana" (106.4): the pool of a player
    let unspent = {
        let n = count_amount().skip(&lit("or more")).then(&crate::mana::unspent()).map(
            |(n, c)| Some((n, c)),
            |(n, c): &(Amount, Option<ir::Color>)| Some((n.clone(), *c)),
        );
        subject_verb("have", "has", &n).map(
            |(p, (right, color))| {
                Some(Condition::Compare { left: Amount::Unspent { player: Box::new(p), color }, cmp: Cmp::Ge, right })
            },
            |c| match c {
                Condition::Compare { left: Amount::Unspent { player, color }, cmp: Cmp::Ge, right } => {
                    Some(((**player).clone(), (right.clone(), *color)))
                }
                _ => None,
            },
        )
    };
    // "~ has flying", "it has a +1/+1 counter on it", "it doesn't have suspend"
    let object =alt("object", vec![lit("it").to(it()), s.singular.clone()]).map(
        |x| (x.set.domain != Domain::Player).then_some(x),
        |x| (x.set.domain != Domain::Player).then(|| x.clone()),
    );
    let has_verb = alt("has", vec![lit("has").to(false), lit("doesn't have").to(true)]);
    let it_has = object.then(&has_verb).then(&has_what()).map(
        |((w, negated), f)| Some(Condition::Is { what: Box::new(w), pred: Predicate::Has(f), negated }),
        |c| match c {
            Condition::Is { what, pred: Predicate::Has(f), negated } => Some((((**what).clone(), *negated), f.clone())),
            _ => None,
        },
    );
    // "it had counters on it"
    let it_had = object.skip(&lit("had")).then(&has_what()).map(
        |(w, f)| {
            Some(Condition::Was(Box::new(Condition::Is { what: Box::new(w), pred: Predicate::Has(f), negated: false })))
        },
        |c| match c {
            Condition::Was(c) => match &**c {
                Condition::Is { what, pred: Predicate::Has(f), negated: false } => Some(((**what).clone(), f.clone())),
                _ => None,
            },
            _ => None,
        },
    );
    // "your library has no cards in it": a whole zone as the subject
    let whole_zone = |z: &Selector| {
        z.quant == ir::Quant::The
            && z.set.zone.as_ref().is_some_and(|zs| zs.owner.is_some() && zs.slice.is_none())
            && z.set == ir::ObjectSet { zone: z.set.zone.clone(), ..ir::ObjectSet::of(ir::Domain::Card) }
    };
    let zone_has = s.selector.skip(&lit("has")).then(&counted).skip(&lit("in it")).map(
        move |(zone, cards)| whole_zone(&zone).then(|| Condition::ZoneHas { zone: b(zone), cards: b(cards) }),
        move |c| match c {
            Condition::ZoneHas { zone, cards } if whole_zone(zone) => Some(((**zone).clone(), (**cards).clone())),
            _ => None,
        },
    );
    let there_is = alt("there is", vec![lit("there is"), lit("there's").alias()]);
    let exists = alt(
        "there is",
        vec![
            lit("there are").then_keep(&alt("plural", vec![thresholds(), s.plural.clone()])),
            // "there are cards exiled with it": a bare plural is one or more of them
            lit("there are").then_keep(&crate::selector::bare_plural()),
            there_is.then_keep(&s.singular),
        ],
    )
        .map(
            move |w| Some(Condition::Exists { what: b(w) }),
            |c| match c {
                Condition::Exists { what } => Some((**what).clone()),
                _ => None,
            },
        );
    // "there is an instant card and a sorcery card in your graveyard": one zone said of both,
    // each of which is there
    let in_zone = |w: &Selector, z: &ir::ZoneSpec| Selector { set: ir::ObjectSet { zone: Some(z.clone()), ..w.set.clone() }, ..w.clone() };
    let unzoned = |w: &Selector| w.set.zone.is_none() && matches!(w.quant, ir::Quant::Indefinite(_));
    let both_there = alt("there is", vec![lit("there is"), lit("there's").alias()])
        .then_keep(&s.singular).skip(&lit("and")).then(&s.singular).then(&s.zone)
        .map(
            move |((x, y), z)| (unzoned(&x) && unzoned(&y)).then(|| Condition::And(vec![
                Condition::Exists { what: b(in_zone(&x, &z)) },
                Condition::Exists { what: b(in_zone(&y, &z)) },
            ])),
            move |c| match c {
                Condition::And(v) => match v.as_slice() {
                    [Condition::Exists { what: x }, Condition::Exists { what: y }]
                        if x.set.zone.is_some() && x.set.zone == y.set.zone
                            && matches!(x.quant, ir::Quant::Indefinite(_)) && matches!(y.quant, ir::Quant::Indefinite(_)) =>
                    {
                        let z = x.set.zone.clone().unwrap();
                        let bare = |w: &Selector| Selector { set: ir::ObjectSet { zone: None, ..w.set.clone() }, ..w.clone() };
                        Some(((bare(x), bare(y)), z))
                    }
                    _ => None,
                },
                _ => None,
            },
        );
    let has = subject_verb("have", "has", &counted).map(
        move |(p, w)| Some(Condition::Has { player: b(p), what: b(w) }),
        |c| match c {
            Condition::Has { player, what } => Some(((**player).clone(), (**what).clone())),
            _ => None,
        },
    );
    let is = alt(
        "is",
        vec![
            object_is().then(&predicate(Number::Sg)).map(
                |((w, n), p)| (!is_plural(&w)).then_some(((w, n), p)),
                |((w, n), p)| (!is_plural(w)).then(|| ((w.clone(), *n), p.clone())),
            ),
            objects_are().then(&predicate(Number::Pl)),
        ],
    )
    .map(
        move |((w, negated), pred)| Some(Condition::Is { what: b(w), pred, negated }),
        |c| match c {
            Condition::Is { what, pred, negated } => Some((((**what).clone(), *negated), pred.clone())),
            _ => None,
        },
    );
    let was = object_was().then(&predicate(Number::Sg)).map(
        move |((w, negated), pred)| Some(Condition::Was(Box::new(Condition::Is { what: b(w), pred, negated }))),
        |c| match c {
            Condition::Was(c) => match &**c {
                Condition::Is { what, pred, negated } => Some((((**what).clone(), *negated), pred.clone())),
                _ => None,
            },
            _ => None,
        },
    );
    let remains = object_verb("remain", "remains", &predicate(Number::Sg), &predicate(Number::Pl)).map(
        |(w, pred)| Some(Condition::Remains { what: Box::new(w), pred }),
        |c| match c {
            Condition::Remains { what, pred } => Some(((**what).clone(), pred.clone())),
            _ => None,
        },
    );
    let in_step = lit("it's").then_keep(&step()).map(
        |(step, whose)| Some(Condition::Step { step, whose }),
        |c| match c {
            Condition::Step { step, whose } => Some((*step, whose.clone())),
            _ => None,
        },
    );
    let turn = alt("turn copula", vec![
        lit("it's").then_keep(&lit("not").opt()),
        lit("it isn't").to(Some(())),
    ]).then(&s.possessive).skip(&lit("turn")).map(
        move |(n, p)| Some(Condition::Turn { whose: b(p), negated: n.is_some() }),
        |c| match c {
            Condition::Turn { whose, negated } => Some((negated.then_some(()), (**whose).clone())),
            _ => None,
        },
    );
    let nth_turn = lit("it's").then_keep(&s.possessive)
        .then(&alt("ordinals", vec![crate::event::ordinal().list("or"), crate::event::ordinal().map(|n| Some(vec![n]), |v: &Vec<u32>| match v.as_slice() { [n] => Some(*n), _ => None })]))
        .skip(&lit("turn of the game")).map(
        move |(p, turns)| Some(Condition::NthTurn { whose: b(p), turns }),
        |c| match c {
            Condition::NthTurn { whose, turns } => Some(((**whose).clone(), turns.clone())),
            _ => None,
        },
    );
    let you = Selector::you;
    let designation = |text: &str, not: &str, d: Designation| {
        alt("designation", vec![lit(text).to(false), lit(not).to(true)]).map(
            move |negated| Some(Condition::Designation { player: Box::new(Selector::you()), designation: d, negated }),
            move |c| match c {
                Condition::Designation { player, designation, negated } if **player == Selector::you() && *designation == d => {
                    Some(*negated)
                }
                _ => None,
            },
        )
    };
    // "that player is the monarch", "there is no monarch" (no player is)
    let nobody = Selector::new(Quant::No, ObjectSet::of(Domain::Player));
    let monarch_other = alt(
        "monarch",
        vec![
            lit("there is no monarch").to(nobody.clone()),
            s.singular.skip(&lit("is the monarch")).map(
                move |p| (p.set.domain == Domain::Player && p != nobody && p != Selector::you()).then_some(p),
                |p| (p.set.domain == Domain::Player && *p != Selector::you()).then(|| p.clone()),
            ),
        ],
    )
    .map(
        |p| Some(Condition::Designation { player: Box::new(p), designation: Designation::Monarch, negated: false }),
        |c| match c {
            Condition::Designation { player, designation: Designation::Monarch, negated: false }
                if **player != Selector::you() =>
            {
                Some((**player).clone())
            }
            _ => None,
        },
    );
    // Oracle spells the number a devotion is compared with as a word ("your devotion to black
    // is less than five"), and any other as a numeral ("your life total is less than 7", "the
    // discovered card's mana value is less than 10", 702.179d "your speed is less than 4")
    let worded = |left: &Amount| matches!(left, Amount::Devotion { .. });
    let compare_is = |greater: bool, copula: &str, past: bool, words: bool| {
        amounts().amount.skip(&lit(copula)).then(&comparison(greater, words)).map(
            move |(left, (cmp, right))| {
                (is_stat(&left) == greater && worded(&left) == words).then(|| {
                    let c = Condition::Compare { left, cmp, right };
                    if past { Condition::Was(Box::new(c)) } else { c }
                })
            },
            move |c| {
                let c = match (c, past) {
                    (Condition::Was(c), true) => &**c,
                    (c, false) => c,
                    _ => return None,
                };
                match c {
                    // "a player has 10 or less life" is the spelling of life ≤ / ≥ N
                    Condition::Compare { left, cmp, right }
                        if is_stat(left) == greater
                            && worded(left) == words
                            && !(matches!(left, Amount::Life(_))
                                && matches!(cmp, Cmp::Le | Cmp::Ge)
                                && matches!(right, Amount::Const(_))) =>
                    {
                        Some((left.clone(), (*cmp, right.clone())))
                    }
                    _ => None,
                }
            },
        )
    };
    // "creatures you control have total power 8 or greater"
    let total = s.set_pl.skip(&lit("have total")).then(&stat()).then(&comparison(true, true)).map(
        |((among, stat), (cmp, right))| {
            Some(Condition::Compare {
                left: Amount::Aggregate { agg: Aggregate::Total, stat, among: Box::new(among) },
                cmp,
                right,
            })
        },
        |c| match c {
            Condition::Compare { left: Amount::Aggregate { agg: Aggregate::Total, stat, among }, cmp, right } => {
                Some((((**among).clone(), *stat), (*cmp, right.clone())))
            }
            _ => None,
        },
    );
    let life_cmp = alt(
        "life",
        vec![
            stat_amount()
                .skip(&lit("or less life"))
                .map(|v| Some((Cmp::Le, v)), |(c, v)| (*c == Cmp::Le).then(|| v.clone())),
            stat_amount()
                .skip(&lit("or more life"))
                .map(|v| Some((Cmp::Ge, v)), |(c, v)| (*c == Cmp::Ge).then(|| v.clone())),
            lit("exactly")
                .then_keep(&stat_amount())
                .skip(&lit("life"))
                .map(|v| Some((Cmp::Eq, v)), |(c, v)| (*c == Cmp::Eq).then(|| v.clone())),
        ],
    );
    // "you have a full party": four creatures in the party (CR 700.8c)
    let full_party = subject_verb("have", "has", &lit("a full party")).map(
        |(p, ())| Some(Condition::Compare { left: Amount::Party(Box::new(p)), cmp: Cmp::Eq, right: Amount::Const(4) }),
        |c| match c {
            Condition::Compare { left: Amount::Party(p), cmp: Cmp::Eq, right: Amount::Const(4) } => {
                Some(((**p).clone(), ()))
            }
            _ => None,
        },
    );
    let life = subject_verb("have", "has", &life_cmp).map(
        |(p, (cmp, right))| Some(Condition::Compare { left: Amount::Life(Box::new(p)), cmp, right }),
        |c| match c {
            Condition::Compare { left: Amount::Life(p), cmp, right } => Some(((**p).clone(), (*cmp, right.clone()))),
            _ => None,
        },
    );
    let does = {
        let base_subject =
            alt("subject", vec![lit("you").to(Selector::you()), lit("they").to(they()), s.plural.clone()]);
        let players = |p: &P<Selector>| {
            p.map(
                |s| (s.set.domain == Domain::Player).then_some(s),
                |s| (s.set.domain == Domain::Player).then(|| s.clone()),
            )
        };
        let actions = crate::action::actions;
        type Actions = ir::Coord<Action>;
        let flag = |p: P<()>| p.opt().map(|o| Some(o.is_some()), |n: &bool| Some(n.then_some(())));
        alt(
            "does",
            vec![
                players(&base_subject).then(&flag(lit("don't"))).then(&actions(Form::Base)),
                players(&s.singular).then(&alt(
                    "does or doesn't",
                    vec![
                        actions(Form::Third).map(|a| Some((false, a)), |(n, a): &(bool, Actions)| (!n).then(|| a.clone())),
                        lit("doesn't").then_keep(&actions(Form::Base)).map(|a| Some((true, a)), |(n, a): &(bool, Actions)| n.then(|| a.clone())),
                    ],
                ))
                .map(|(p, (n, a))| Some(((p, n), a)), |((p, n), a): &((Selector, bool), Actions)| Some((p.clone(), (*n, a.clone())))),
            ],
        )
        .map(
            // "you cast it" is the past ("if you cast it"): "cast" reads the same in both
            // an offer is a deed (118.12a): never a statement of what is ("you control ~" is
            // `Controls`, "it has flying" a static)
            move |((p, negated), actions)| {
                (!actions.items.iter().any(|a| matches!(a, Action::Cast { .. } | Action::Static(_) | Action::Is(_))))
                    .then(|| Condition::Does { player: b(p), actions, negated })
            },
            |c| match c {
                Condition::Does { player, actions, negated }
                    if !actions.items.iter().any(|a| matches!(a, Action::Static(_) | Action::Is(_))) =>
                {
                    Some((((**player).clone(), *negated), actions.clone()))
                }
                _ => None,
            },
        )
    };
    // "If you don't put a card into your hand this way": the deed, said of this effect
    // (the deed done "this way" is a past event of its own rule)
    let does_this_way = does
        .skip(&lit("this way"))
        .map(|c| matches!(c, Condition::Does { negated: true, .. }).then_some(c), |_: &Condition| None)
        .alias();
    let won = |text: &str, won: bool, contest: Option<Contest>| {
        lit(text).to(Condition::Won { player: Box::new(you()), won, contest })
    };
    let characteristic = table(
        "characteristic",
        vec![
            ("creature type".into(), Characteristic::CreatureType),
            ("card type".into(), Characteristic::CardType),
            ("color".into(), Characteristic::Color),
        ],
    );
    let share_verb = alt("shares", vec![lit("shares a").to(false), lit("doesn't share a").to(true)]);
    let shares = object.then(&share_verb).then(&characteristic).skip(&lit("with")).then(&s.selector).map(
        |(((what, negated), characteristic), with)| {
            Some(Condition::Shares { what: Box::new(what), characteristic, with: Box::new(with), negated })
        },
        |c| match c {
            Condition::Shares { what, characteristic, with, negated } => {
                Some(((((**what).clone(), *negated), *characteristic), (**with).clone()))
            }
            _ => None,
        },
    );
    let nth = alt(
        "nth resolution",
        vec![
            lit("this is the").then_keep(&ordinal()).skip(&lit("time this ability has resolved this turn")).map(
                |n| Some(Condition::NthResolution { n, short: false }),
                |c| match c {
                    Condition::NthResolution { n, short: false } => Some(*n),
                    _ => None,
                },
            ),
            lit("it's the").then_keep(&ordinal()).skip(&lit("time")).map(
                |n| Some(Condition::NthResolution { n, short: true }),
                |c| match c {
                    Condition::NthResolution { n, short: true } => Some(*n),
                    _ => None,
                },
            ),
        ],
    );
    // "you do", "the player does", "they don't", "you can't"
    let did = {
        let no_one = Selector::new(Quant::No, ObjectSet::of(Domain::Player));
        let base_subject = alt(
            "subject",
            vec![lit("you").to(Selector::you()), lit("they").to(they()), s.plural.clone()],
        );
        let singular = alt("subject", vec![lit("no one").to(no_one), lit("it").to(it()), s.singular.clone()]);
        let base_verb = alt(
            "do",
            vec![lit("do").to(Outcome::Did), lit("don't").to(Outcome::Didnt), lit("can't").to(Outcome::Couldnt)],
        );
        let third_verb = alt(
            "does",
            vec![lit("does").to(Outcome::Did), lit("doesn't").to(Outcome::Didnt), lit("can't").to(Outcome::Couldnt)],
        );
        alt("did", vec![base_subject.then(&base_verb), singular.then(&third_verb)]).map(
            |(who, outcome)| Some(Condition::Did { who: Box::new(who), outcome }),
            |c| match c {
                Condition::Did { who, outcome } => Some(((**who).clone(), *outcome)),
                _ => None,
            },
        )
    };
    let came_under_control = s.selector.skip(&lit("came under")).then(&s.possessive)
        .skip(&lit("control")).then(&history_window()).map(
            |((what, player), window)| {
                (matches!(what.set.domain, Domain::Permanent | Domain::Spell | Domain::Object)
                    && player.set.domain == Domain::Player).then_some(Condition::CameUnderControl {
                        what: Box::new(what), player: Box::new(player), window,
                    })
            },
            |c| match c {
                Condition::CameUnderControl { what, player, window }
                    if matches!(what.set.domain, Domain::Permanent | Domain::Spell | Domain::Object)
                        && player.set.domain == Domain::Player =>
                            Some((((**what).clone(), (**player).clone()), window.clone())),
                _ => None,
            },
        );
    let targets_legal = alt("existing target group", vec![
        lit("both targets are still legal as").to(Some(2)),
        lit("all targets are still legal as").to(None),
    ]).then(&s.singular).skip(&lit("resolves")).map(
        |(expected, source)| {
            (matches!(source.quant, Quant::Definite(_)) && matches!(source.set.domain, Domain::Spell | Domain::Ability))
                .then_some(Condition::TargetsRemainLegal { source: Box::new(source), expected })
        },
        |c| match c {
            Condition::TargetsRemainLegal { source, expected }
                if matches!(source.quant, Quant::Definite(_)) && matches!(source.set.domain, Domain::Spell | Domain::Ability) =>
                    Some((*expected, (**source).clone())),
            _ => None,
        },
    );
    let targets = object.skip(&lit("targets")).then(&s.selector).map(
        |(what, target)| Some(Condition::Targets { what: Box::new(what), target: Box::new(target) }),
        |c| match c {
            Condition::Targets { what, target } => Some(((**what).clone(), (**target).clone())),
            _ => None,
        },
    );
    alt(
        "condition",
        vec![
            controls,
            controlled,
            most_life(),
            more_than(),
            there_count,
            there_five,
            there_exactly,
            exists,
            both_there,
            zone_has,
            player_counters,
            unspent,
            it_has,
            it_had,
            life,
            has,
            is,
            was,
            remains,
            damage_dealt(),
            happened(e, None),
            cast_was(),
            mana_spent(),
            turn,
            nth_turn,
            in_step,
            designation("you're the monarch", "you're not the monarch", Designation::Monarch),
            designation("you have the city's blessing", "you don't have the city's blessing", Designation::CitysBlessing),
            designation("you have the initiative", "you don't have the initiative", Designation::Initiative),
            designation("you've completed a dungeon", "you haven't completed a dungeon", Designation::CompletedDungeon),
            designation("you have an enduring story", "you don't have an enduring story", Designation::EnduringStory),
            monarch_other,
            total,
            full_party,
            compare_is(true, "is", false, false),
            compare_is(false, "is", false, false),
            compare_is(true, "was", true, false),
            compare_is(false, "was", true, false),
            compare_is(false, "is", false, true),
            compare_is(false, "was", true, true),
            does,
            does_this_way,
            won("you win the flip", true, Some(Contest::Flip)),
            won("you lose the flip", false, Some(Contest::Flip)),
            won("you win", true, None),
            lit("you won").to(Condition::Was(Box::new(Condition::Won {
                player: Box::new(you()),
                won: true,
                contest: None,
            }))),
            shares,
            nth,
            lit("it's day").to(Condition::DayNight(DayNight::Day)),
            lit("it's night").to(Condition::DayNight(DayNight::Night)),
            lit("it's neither day nor night").to(Condition::DayNight(DayNight::Neither)),
            lit("it regenerates this way").to(Condition::RegeneratesThisWay),
            lit("you chose a creature other than ~ as your Ring-bearer").to(Condition::ChoseOtherRingBearer),
            lit("one or more of them entered from a graveyard or was cast from a graveyard").to(Condition::EnteredOrCastFrom { exile: false }),
            lit("one or more of them entered from exile or was cast from exile").to(Condition::EnteredOrCastFrom { exile: true }),
            crate::kernel::table("color", ir::Color::ALL.iter().map(|c| (c.word().to_string(), *c)).collect())
                .skip(&lit("is the most common color among all permanents or is tied for most common"))
                .map(|c| Some(Condition::MostCommonColor(c)), |c| match c {
                    Condition::MostCommonColor(c) => Some(*c),
                    _ => None,
                }),
            // "you revealed a Dragon card or controlled a Dragon as you cast ~"
            lit("you revealed a")
                .then_keep(&crate::kernel::table("subtype", ir::Subtype::all().map(|s| (s.name().to_string(), s)).collect()))
                .skip(&lit("card or controlled a"))
                .then(&crate::kernel::table("subtype", ir::Subtype::all().map(|s| (s.name().to_string(), s)).collect()))
                .skip(&lit("as you cast ~"))
                .map(
                    |(revealed, controlled)| (revealed == controlled).then_some(Condition::RevealedOrControlledAsCast(revealed)),
                    |c| match c {
                        Condition::RevealedOrControlledAsCast(t) => Some((*t, *t)),
                        _ => None,
                    },
                ),
            targets,
            targets_legal,
            came_under_control,
            lit("able").to(Condition::Able),
            crate::vote::option().skip(&lit("gets more votes")).then(&lit("or the vote is tied").opt()).map(
                |(option, tied)| Some(Condition::MoreVotes { option, or_tied: tied.is_some() }),
                |c| match c {
                    Condition::MoreVotes { option, or_tied } => Some((option.clone(), or_tied.then_some(()))),
                    _ => None,
                },
            ),
            did,
            lit("you pay").then_keep(&mana_cost()).skip(&lit("more to cast")).then(&alt("spell", vec![lit("it").to(it()), s.selector.clone()])).map(
                |(cost, spell)| Some(Condition::PaysMore { cost, spell: Box::new(spell) }),
                |c| match c {
                    Condition::PaysMore { cost, spell } => Some((cost.clone(), (**spell).clone())),
                    _ => None,
                },
            ),
            lit("it's the").then_keep(&ordinal()).skip(&lit("combat phase of the turn")).map(
                |n| Some(Condition::NthCombatPhase { n }),
                |c| match c {
                    Condition::NthCombatPhase { n } => Some(*n),
                    _ => None,
                },
            ),
            lit("it's the").then_keep(&ordinal()).then(&selectors().set_sg).skip(&lit("you cast this turn")).map(
                |(n, set)| (set.domain == Domain::Spell).then_some(Condition::NthCastThisTurn { n, set }),
                |c| match c {
                    Condition::NthCastThisTurn { n, set } => Some((*n, set.clone())),
                    _ => None,
                },
            ),
            object.skip(&lit("is the")).then(&ordinal()).then(&s.set_sg)
                .skip(&alt("cast this game", vec![lit("you've cast this game"), lit("you have cast this game").alias()]))
                .map(
                    |((what, n), set)| (set.domain == Domain::Spell
                        && (what.set.domain == Domain::Spell || matches!(what.quant, Quant::Definite(Ref::This | Ref::Anaphor(Anaphor::It)))))
                        .then(|| Condition::NthCastThisGame { what: Box::new(what), n, set }),
                    |c| match c {
                        Condition::NthCastThisGame { what, n, set } => Some((((**what).clone(), *n), set.clone())),
                        _ => None,
                    },
                ),
            lit("you're playing for ante").to(Condition::PlayingForAnte { negated: false }),
            lit("you're not playing for ante").to(Condition::PlayingForAnte { negated: true }),
        ],
    )
}

fn build(e: &Events) -> Conditions {
    let c = clause(e);
    // Sharing an already definite actor is an alias of repeating it. Choosing a fresh
    // actor here would require correlated binding, so indefinite/targeted actors stay refused.
    let inherited = alt("shared condition subject", [false, true].into_iter().map(|plural| {
        let join = table("boolean join", vec![("and".into(), false), ("or".into(), true)]);
        c.then(&join).then(&happened(e, Some(plural))).map(
            move |((first, or), mut past)| {
                let who = match &first {
                    Condition::Controls { player, .. } | Condition::Owns { player, .. } | Condition::OwnsAndControls { player, .. } => (**player).clone(),
                    _ => return None,
                };
                let definite = matches!(who.quant, Quant::Definite(Ref::You | Ref::Anaphor(Anaphor::That | Anaphor::They)));
                let grammatical_plural = crate::selector::is_plural(&who) || who == Selector::you()
                    || matches!(who.quant, Quant::Definite(Ref::Anaphor(Anaphor::They)));
                if who.set.domain != Domain::Player || !definite || grammatical_plural != plural { return None; }
                let Condition::Happened { pattern, window: Some(_), .. } = &mut past else { return None };
                if !pattern.subjects.is_empty() || !pattern.actions.iter().all(|a| a.takes_subject(&who) && a.roles_fit()) { return None; }
                pattern.subjects.push(who);
                Some(if or { Condition::Or(vec![first, past]) } else { Condition::And(vec![first, past]) })
            }, |_| None,
        ).alias()
    }).collect());
    let condition = alt(
        "conditions",
        vec![
            c.clone(),
            inherited,
            c.list("and").map(
                |v| Some(Condition::And(v)),
                |c| match c {
                    Condition::And(v) => Some(v.clone()),
                    _ => None,
                },
            ),
            c.list("or").map(
                |v| Some(Condition::Or(v)),
                |c| match c {
                    Condition::Or(v) => Some(v.clone()),
                    _ => None,
                },
            ),
            // "~ entered this turn or if you control a basic land": the repeated "if" means
            // the same as "or" alone
            c.skip(&lit("or if")).then(&c).map(|(a, b)| Some(Condition::Or(vec![a, b])), |_| None).alias(),
            c.skip(&lit("as you cast ~")).map(
                |x| Some(Condition::AsYouCast(Box::new(x))),
                |c| match c {
                    Condition::AsYouCast(x) => Some((**x).clone()),
                    _ => None,
                },
            ),
        ],
    );
    let declared = alt("declared", vec![lit("attackers are declared").to(false), lit("blockers are declared").to(true)]);
    let when = alt(
        "before or after",
        vec![
            lit("before").to((false, false)),
            lit("during combat before").to((true, false)),
            lit("during combat after").to((true, true)),
        ],
    );
    let declaration = when.then(&declared).map(
        |((in_combat, after), blockers)| Some(Restriction::Declaration { in_combat, after, blockers }),
        |r| match r {
            Restriction::Declaration { in_combat, after, blockers } => Some(((*in_combat, *after), *blockers)),
            _ => None,
        },
    );
    let one = alt(
        "restriction",
        vec![
            lit("only as a sorcery").to(Restriction::Sorcery),
            lit("only after combat").to(Restriction::AfterCombat),
            lit("only as an instant").to(Restriction::Instant),
            lit("only during any upkeep step").to(Restriction::AnyUpkeep),
            lit("only before the combat damage step").to(Restriction::BeforeCombatDamage),
            lit("only once each turn").to(Restriction::OnceEachTurn),
            lit("only once").to(Restriction::Once),
            lit("no more than")
                .then_keep(&alt(
                    "times",
                    vec![
                        lit("twice").to(Amount::Const(2)),
                        crate::words::count_atom()
                            .skip(&lit("times"))
                            .map(|n| (n != Amount::Const(2)).then_some(n), |n| (*n != Amount::Const(2)).then(|| n.clone())),
                    ],
                ))
                .skip(&lit("each turn"))
                .map(
                    |n| Some(Restriction::TimesEachTurn(n)),
                    |r| match r {
                        Restriction::TimesEachTurn(n) => Some(n.clone()),
                        _ => None,
                    },
                ),
            lit("only during").then_keep(&during()).map(
                |d| Some(Restriction::During(d)),
                |r| match r {
                    Restriction::During(d) => Some(d.clone()),
                    _ => None,
                },
            ),
            lit("only").then_keep(&declaration),
            lit("only during").then_keep(&during()).skip(&lit(",").tight()).skip(&lit("before")).then(&declared).map(
                |(during, blockers)| Some(Restriction::DuringBefore { during, blockers }),
                |r| match r {
                    Restriction::DuringBefore { during, blockers } => Some((during.clone(), *blockers)),
                    _ => None,
                },
            ),
            lit("only if").then_keep(&condition).map(
                |c| Some(Restriction::If(Box::new(c))),
                |r| match r {
                    Restriction::If(c) => Some((**c).clone()),
                    _ => None,
                },
            ),
        ],
    );
    let restrictions = one.then(&lit("and").then_keep(&one).many()).map(
        |(f, mut rest)| {
            rest.insert(0, f);
            Some(rest)
        },
        |v: &Vec<Restriction>| v.split_first().map(|(f, r)| (f.clone(), r.to_vec())),
    );
    Conditions { condition, restrictions }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::Outcome;
    use oracle::token::tokenize;

    #[test]
    fn conditions_round_trip() {
        let c = &conditions().condition;
        for t in [
            "~ was kicked",
            "there are seven or more cards in your graveyard",
            "its controller pays {2}",
            "a creature died this turn",
            "you control two or more artifacts",
            "you attacked this turn",
            "you control a creature with power 4 or greater",
            "you control ~",
            "it's a land card",
            "you gained life this turn",
            "you have no cards in hand",
            "~ is tapped",
            "~ is untapped",
            "defending player controls an Island",
            "a permanent left the battlefield under your control this turn",
            "~ is in your graveyard",
            "X is 5 or more",
            "you control an artifact",
            "~ was cast from a graveyard",
            "you're the monarch",
            "you have the city's blessing",
            "you've cast a noncreature spell this turn",
            "it's your turn",
            "it's your first, second, or third turn of the game",
            "a player has 10 or less life",
            "you win the flip",
            "~ is equipped",
            "they pay {X}",
            "you control two or more basic lands",
            "your devotion to blue and black is less than seven",
            // 702.179d; Oracle's numerals for any other number compared
            "your speed is less than 4",
            "your life total is less than 7",
            "its toughness was less than 1",
            "you search your library this way",
            "that spell is countered this way",
            "a creature card was exiled this way",
            "you cast a spell this way",
            "it remains exiled",
            "~ remains tapped",
            "they remain exiled",
            "you cast it",
            "you cast it from your hand",
            "no spells were cast last turn",
            "you've been attacked this step",
            "~ didn't attack this turn",
            "you haven't cast a spell from your hand this turn",
            "an opponent controls more lands than you",
            "you have more cards in hand than each opponent",
            "it shares a creature type with ~",
            "this is the second time this ability has resolved this turn",
            "it's the third time",
            "the gift wasn't promised",
            "~'s madness cost was paid",
            "it was a creature card",
            "it had counters on it",
            "its mana value was 2 or less",
            "{B} was spent to cast ~",
            "at least three white mana was spent to cast ~",
            "mana from a Treasure was spent to cast it",
            "mana from a Treasure was spent to activate this ability",
            "that mana is spent to cast a creature spell",
            "any of that mana is spent to cast an instant or sorcery spell",
            "creatures you control have total power 8 or greater",
            "an opponent has three or more poison counters",
            "target player has fewer than nine poison counters",
            "an opponent has two or fewer poison counters",
            "~'s power is 4 or greater",
            "it doesn't have suspend",
            "it's neither day nor night",
            "it targets a tapped creature",
            "you gained 3 or more life this turn",
            "you won",
            "~ is paired with another creature",
            "there are no depletion counters on ~",
            "the player does",
            "you can't",
            "~ is on the battlefield or in your graveyard",
            "they're mana abilities",
            "it was kicked with its {2}{U} kicker",
            "you've been attacked this step",
        ] {
            let toks = tokenize(t);
            match c.parse_all(&toks) {
                Outcome::Accepted { value, .. } => {
                    assert_eq!(c.print_text(&value, false).as_deref(), Some(t), "{value:#?}")
                }
                other => panic!("{t}: {other:#?}"),
            }
        }
        let r = &conditions().restrictions;
        for t in [
            "only if you control an artifact and an enchantment",
            "only as a sorcery",
            "only during your turn and only once each turn",
            "only if you have no cards in hand",
            "only during the declare blockers step",
            "only during combat before blockers are declared",
            "only during an opponent's turn",
        ] {
            let Outcome::Accepted { value, .. } = r.parse_all(&tokenize(t)) else { panic!("{t}") };
            assert_eq!(r.print_text(&value, false).as_deref(), Some(t));
        }
    }

    fn reads(t: &str) -> Option<String> {
        let c = &conditions().condition;
        match c.parse_all(&tokenize(t)) {
            Outcome::Accepted { value, .. } => c.print_text(&value, false),
            _ => None,
        }
    }

    /// Texts that read and print back as they are said, and texts that do not read.
    fn check(yes: &[&str], no: &[&str]) {
        for t in yes {
            assert_eq!(reads(t).as_deref(), Some(*t), "{t}");
        }
        for t in no {
            assert_eq!(reads(t), None, "{t}");
        }
    }

    /// CR 608.2c: "and" joins noun phrases that all must be controlled, "or" alternatives.
    #[test]
    fn r608_2c_control_of_coordinated_noun_phrases() {
        check(
            &[
                "you control an artifact and an enchantment",
                "you control a white permanent and a blue permanent",
                "you control a Plains or a Swamp",
                "you control seven or more lands and/or Treefolk",
            ],
            &["you control an artifact and", "you control and an enchantment"],
        );
    }

    /// CR 608.2c: "there is an instant card and a sorcery card in your graveyard" (Flow State):
    /// each noun phrase is there, in the one zone said after both.
    #[test]
    fn r608_2c_coordinated_existence_in_one_zone() {
        check(
            &[
                "there is an instant card and a sorcery card in your graveyard",
                "there is an instant card in your graveyard",
            ],
            &[
                "there is an instant card and a sorcery card",
                "there is an instant card in your graveyard and a sorcery card in your graveyard",
                "there is an instant card and in your graveyard",
                "there is ~ and a sorcery card in your graveyard",
            ],
        );
        let Some(Outcome::Accepted { value: Condition::And(both), .. }) =
            Some(conditions().condition.parse_all(&tokenize("there is an instant card and a sorcery card in your graveyard")))
        else { panic!("one reading") };
        assert_eq!(both.len(), 2);
        for c in &both {
            let Condition::Exists { what } = c else { panic!("{c:?}") };
            assert!(what.set.zone.as_ref().is_some_and(|z| z.zones.items == vec![ir::Zone::Graveyard]), "{c:?}");
        }
    }

    /// CR 119, 103.4: a margin is the difference asked for; a number to measure against is only
    /// for life.
    #[test]
    fn r119_comparison_with_a_margin() {
        check(
            &[
                "an opponent controls at least two more lands than you",
                "an opponent has at least four more cards in hand than you",
                "you have at least 10 life more than your starting life total",
                "you have less life than an opponent",
                "you have more cards in hand than each opponent",
            ],
            &[
                "you control at least two more lands than 3",
                "you have at least 10 life less than your starting life total",
                "you have at least 10 life more",
            ],
        );
        // the same question said two ways: the first is the printed one
        assert_eq!(
            reads("your life total is at least 10 greater than your starting life total").as_deref(),
            Some("you have at least 10 life more than your starting life total")
        );
    }

    /// CR 608.2i: history looks back over a window and may count how often; a bare perfect with no
    /// window is not a history.
    #[test]
    fn r608_2i_windows_and_counts() {
        check(
            &[
                "this ability has been activated four or more times this turn",
                "you've cycled a card named ~ four or more times this game",
                "you've cast a spell this game",
                "it attacked or blocked since your last upkeep",
                "that player attacked you during their last turn",
                "it attacked during your last turn",
            ],
            &[
                "this ability has been activated four or more times",
                "you've cast a spell since last upkeep",
                "you've cast a spell during last turn",
            ],
        );
    }

    /// CR 601.2h, 106.6: what the mana paid for a spell was, by count, kind and colors; the count
    /// of colors takes "were".
    #[test]
    fn r601_2h_mana_spent_by_count_and_kind() {
        check(
            &[
                "no colored mana was spent to cast it",
                "{C} wasn't spent to cast it",
                "at least three colorless mana was spent to cast ~",
                "at least three mana of the same color was spent to cast it",
                "two or more colors of mana were spent to cast it",
                "exactly three colors of mana were spent to activate this ability",
                "five or more mana was spent to cast that spell",
                "mana from a Treasure was spent to cast it",
                "no mana was spent to cast it",
                "the {2}{U} cost was paid",
            ],
            &[
                "two or more colors of mana was spent to cast it",
                "no colored mana were spent to cast it",
                "mana from was spent to cast it",
            ],
        );
        // "at least N mana" and "N or more mana" are one meaning; the second prints
        assert_eq!(
            reads("at least four mana was spent to cast it").as_deref(),
            Some("four or more mana was spent to cast it")
        );
    }

    /// CR 120.10: excess damage; the damage or the one dealt it can be the subject, one meaning.
    #[test]
    fn r120_10_damage_dealt_as_a_condition() {
        check(
            &[
                "excess damage was dealt this way",
                "excess damage was dealt to a creature this way",
                "that creature was dealt excess damage this turn",
                "an opponent was dealt 7 or more damage this turn",
                "a player was dealt 6 or more combat damage this turn",
                "you were dealt 4 or more damage this turn",
            ],
            &["you was dealt 4 or more damage this turn", "excess damage was dealt", "3 damage was dealt this turn"],
        );
        assert_eq!(reads("4 or more damage was dealt to it this turn").as_deref(), Some("it was dealt 4 or more damage this turn"));
    }

    /// CR 506.5: only attacking and blocking can be alone; CR 903.3: a player's commander; CR 725:
    /// designations can be absent.
    #[test]
    fn r506_5_alone_and_other_predicates() {
        check(
            &[
                "it's attacking alone",
                "enchanted creature is attacking alone",
                "~ is blocking alone",
                "~ is your commander",
                "you're not the monarch",
                "you haven't completed a dungeon",
            ],
            &["~ is tapped alone", "it's untapped alone"],
        );
    }

    /// CR 107.1: numbers are integers, so "fewer than seven" is "six or fewer"; a number alone is
    /// equal to it.
    #[test]
    fn r107_1_thresholds_and_equality() {
        for (said, printed) in [
            ("you have fewer than seven cards in hand", "you have six or fewer cards in hand"),
            ("its controller has more than four cards in hand", "its controller has five or more cards in hand"),
            ("there are fewer than eight cards in your graveyard", "there are seven or fewer cards in your graveyard"),
            ("its power is exactly 20", "its power is 20"),
        ] {
            assert_eq!(reads(said).as_deref(), Some(printed), "{said}");
        }
        check(&["you have exactly 13 life", "X is 3", "its mana value is X"], &["you have fewer than X cards in hand"]);
    }
}
