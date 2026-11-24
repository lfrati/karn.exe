//! Effects: sentences of clauses.
//!
//! ```text
//! sentence := lead sentence | body | "activate" restrictions | "choose" modes "—" | static | …
//! lead     := leading "," | delayed "," | "when you do," | "otherwise," | "starting with" selector ","
//!           | "once each turn," | "as" selector "enters," | "then" | "instead"
//! leading  := the modifiers that can stand in front: "if" condition | duration | "for each" …
//!           | "at the beginning of" step | "at end of combat" | "as long as" condition
//!           | "during" possessive "turn" | "unless" player action
//! delayed  := ("when" | "whenever") pattern ["this turn" | "this way"]
//! body     := clause (join subject-clause)*          "you draw a card and you lose 1 life"
//! clause   := [subject] ["may"] act(form) (join act(form))*
//! act      := action modifier*                        "draw a card for each creature you control"
//! modifier := leading | ", where X is" amount | "until" selector "leaves the battlefield"
//!           | "instead" | "face down" | N "times" | …
//! join     := ", then" | ", and" | "," | "and"
//! ```
//!
//! A lead governs the whole sentence after it, and that sentence may have leads of its own, so
//! every order the text uses is one rule: "Then if …, …", "When you do, until end of turn, …",
//! "Until your next turn, whenever …, …", "If …, choose one —", "During your turn, as long as …, …".
//!
//! The verb form follows the subject: none (an imperative) and "you" or plural subjects take the
//! base form, singular ones the third person; after "may" every subject takes the base form.

use crate::action::action;
use crate::amount::amounts;
use crate::condition::conditions;
use crate::event::{Form, step};
use crate::kernel::{P, alt, empty, forward, lit};
use crate::selector::selectors;
use ir::{Act, Amount, Body, Clause, Duration, Join, Lead, Modes, Modifier, Rider, Selector, Sentence, Subject, Unchosen, DelayLimit};
use std::sync::OnceLock;

pub struct Effects {
    pub sentence: P<Sentence>,
    pub body: P<Body>,
}

pub fn effects() -> &'static Effects {
    static E: OnceLock<Effects> = OnceLock::new();
    E.get_or_init(build)
}

fn join() -> P<Join> {
    alt(
        "join",
        vec![
            lit(", then").to(Join::CommaThen),
            lit(", and").to(Join::CommaAnd),
            lit(",").to(Join::Comma),
            lit("and").to(Join::And),
            lit("or").to(Join::Or),
            // "Tap all untapped permanents of the chosen type target player controls, or untap
            // all tapped permanents of that type that player controls": the comma sets off a
            // long first act; the same choice as "or"
            lit(", or").to(Join::Or).alias(),
        ],
    )
}

/// A subject with its verb form: "you" and plurals take the base form, singulars the third
/// person; `None` is the imperative.
fn subjects() -> Vec<(P<Option<Subject>>, Form)> {
    let s = selectors();
    let some = |p: &P<Selector>| {
        p.map(
            |s| Some(Some(Subject::Selector(s))),
            |s: &Option<Subject>| match s {
                Some(Subject::Selector(s)) => Some(s.clone()),
                _ => None,
            },
        )
    };
    let set = s.set_pl.map(
        |s| Some(Some(Subject::Set(s))),
        |s: &Option<Subject>| match s {
            Some(Subject::Set(s)) => Some(s.clone()),
            _ => None,
        },
    );
    // "you and that player [each]": members in either number, the verb plural
    let member = alt(
        "subject",
        vec![
            lit("you").to(Subject::Selector(Selector::you())),
            alt("selector", vec![s.singular.clone(), s.plural.clone()]).map(
                |x| (x != Selector::you()).then_some(Subject::Selector(x)),
                |m| match m {
                    Subject::Selector(x) if *x != Selector::you() => Some(x.clone()),
                    _ => None,
                },
            ),
            s.set_pl.map(
                |x| Some(Subject::Set(x)),
                |m| match m {
                    Subject::Set(x) => Some(x.clone()),
                    _ => None,
                },
            ),
        ],
    );
    // bare sets alone are one set with a type union ("Ninjas and Rogues you control"), and a
    // quantifier carries over a bare set after it ("All creatures and Vehicles"): a bare set
    // follows only a definite member ("You and permanents you control")
    let not_all_sets = |items: &Vec<Subject>| {
        !items.iter().all(|m| matches!(m, Subject::Set(_)))
            && items.windows(2).all(|w| match &w[1] {
                Subject::Set(set) => matches!(&w[0], Subject::Selector(s)
                    if matches!(s.quant, ir::Quant::Definite(_))
                        || (s.quant == ir::Quant::All && s.set.domain == ir::Domain::Player && set.domain != ir::Domain::Player)),
                _ => true,
            })
    };
    let joint = member.list("and").then(&lit("each").opt()).map(
        move |(items, each)| not_all_sets(&items).then_some(Some(Subject::And { items, each: each.is_some() })),
        move |s: &Option<Subject>| match s {
            Some(Subject::And { items, each }) if not_all_sets(items) => Some((items.clone(), each.then_some(()))),
            _ => None,
        },
    );
    // the imperative first: later clauses skip it
    // "up to two target creatures each get +1/+0"
    // "They each get +2/+2 until end of turn"
    let each = alt("subject", vec![s.plural.clone(), lit("they").to(they())]).skip(&lit("each")).map(
        |x| Some(Some(Subject::Each(x))),
        |s: &Option<Subject>| match s {
            Some(Subject::Each(x)) => Some(x.clone()),
            _ => None,
        },
    );
    // plural pronoun-like references take the base form: "both creatures have vigilance", "the
    // copies gain haste"
    let plural_special = |x: &Selector| {
        use ir::selector::Anaphor;
        matches!(&x.quant, ir::Quant::Definite(ir::Ref::Anaphor(Anaphor::BothPaired | Anaphor::Copy { plural: true })))
    };
    let plural_specials = s.singular.map(
        move |x| plural_special(&x).then_some(Some(Subject::Selector(x))),
        move |s: &Option<Subject>| match s {
            Some(Subject::Selector(x)) if plural_special(x) => Some(x.clone()),
            _ => None,
        },
    );
    vec![
        (empty().to(None), Form::Base),
        (joint, Form::Base),
        (plural_specials, Form::Base),
        (each, Form::Base),
        (some(&lit("you").to(Selector::you())), Form::Base),
        (some(&lit("they").to(they())), Form::Base),
        (some(&s.plural), Form::Base),
        (set, Form::Base),
        (some(&s.singular.map(move |x| (!plural_special(&x)).then_some(x), move |x| (!plural_special(x)).then(|| x.clone()))), Form::Third),
    ]
}

fn duration() -> P<Duration> {
    alt(
        "duration",
        vec![
            // "until end of turn", "until your next turn", "until the end of that player's next turn"
            lit("until").then_keep(&crate::event::time()).map(
                |t| Some(Duration::Until(t)),
                |d| match d {
                    Duration::Until(t) => Some(t.clone()),
                    _ => None,
                },
            ),
            lit("until you exile another card with ~").to(Duration::UntilExileAnother),
            lit("this turn").to(Duration::ThisTurn),
            lit("this combat").to(Duration::ThisCombat),
            lit("for the rest of the game").to(Duration::RestOfGame),
            // "until ~ leaves the battlefield", "until an opponent becomes the monarch"
            lit("until").then_keep(&crate::kernel::lazy("event pattern", || crate::event::events().pattern.clone())).map(
                |p| Some(Duration::UntilEvent(Box::new(p))),
                |d| match d {
                    Duration::UntilEvent(p) => Some((**p).clone()),
                    _ => None,
                },
            ),
            // "for as long as it remains exiled", "for as long as you control ~", "for as long as
            // it has a flood counter on it"
            lit("for as long as").then_keep(&crate::kernel::lazy("condition", || conditions().condition.clone())).map(
                |c| Some(Duration::While(Box::new(c))),
                |d| match d {
                    Duration::While(c) => Some((**c).clone()),
                    _ => None,
                },
            ),
        ],
    )
}

fn they() -> Selector {
    Selector::new(
        ir::Quant::Definite(ir::Ref::Anaphor(ir::selector::Anaphor::They)),
        ir::ObjectSet::of(ir::Domain::Player),
    )
}

/// "they" for objects ("for as long as they remain exiled").
fn they_objects() -> Selector {
    Selector::new(
        ir::Quant::Definite(ir::Ref::Anaphor(ir::selector::Anaphor::They)),
        ir::ObjectSet::of(ir::Domain::Object),
    )
}

/// "unless its controller pays {2}", "unless you sacrifice a creature", "unless they discard a
/// card or sacrifice a creature", "unless that player pays {1} or 1 life": what the player may
/// do instead, one act or alternatives joined by "or" (118.12a).
fn unless() -> P<Modifier> {
    let s = selectors();
    let they = lit("they").to(they());
    let base = alt("player", vec![lit("you").to(Selector::you()), they, s.plural.clone()]);
    let offered = |form: Form| {
        crate::action::actions(form).map(
            |c| (c.items.len() == 1 || c.conj == ir::Conj::Or).then_some(c),
            |c| (c.items.len() == 1 || c.conj == ir::Conj::Or).then(|| c.clone()),
        )
    };
    let who_action = alt("unless clause", vec![base.then(&offered(Form::Base)), s.singular.then(&offered(Form::Third))]);
    let can = |who: &Selector, c: &ir::Coord<ir::Action>| c.items.iter().all(|a| crate::action::does(Some(who), a));
    lit("unless").then_keep(&who_action).map(
        // what "unless" asks is a payment-like act (608.2d), not a villainous choice (701.55b)
        move |(who, actions)| {
            (can(&who, &actions) && !actions.items.iter().any(|a| matches!(a, ir::Action::FaceVillainousChoice(_))))
                .then(|| Modifier::Unless { who, actions })
        },
        move |m| match m {
            Modifier::Unless { who, actions } => Some((who.clone(), actions.clone())),
            _ => None,
        },
    )
}

/// What a player may do instead of an act ("… unless they discard a card") takes any "or" that
/// follows: "or" never offers the act and another as a choice after it (118.12a).
fn or_after_unless(c: &Clause) -> bool {
    let acts = std::iter::once(&c.first).chain(c.rest.iter().map(|(_, a)| a));
    acts.zip(&c.rest).any(|(before, (join, _))| *join == Join::Or && before.mods.iter().any(|m| matches!(m, Modifier::Unless { .. })))
}

/// The modifiers, built once: after an act any of them, in front of a sentence those that English
/// puts there.
struct Modifiers {
    any: P<Modifier>,
    leading: P<Modifier>,
}

/// What modifies an act when it follows it: "for each creature you control", "until end of
/// turn", "unless you pay {2}", "if it attacked this turn".
pub fn modifier() -> P<Modifier> {
    modifiers().any.clone()
}

fn modifiers() -> &'static Modifiers {
    static M: OnceLock<Modifiers> = OnceLock::new();
    M.get_or_init(build_modifiers)
}

fn build_modifiers() -> Modifiers {
    let a = amounts();
    let for_each = a.for_each.map(
        |n| Some(Modifier::ForEach(n)),
        |m| match m {
            Modifier::ForEach(n) => Some(n.clone()),
            _ => None,
        },
    );
    let for_each_value = lit("for each").then_keep(&crate::choice::value_class(crate::words::Number::Sg)).map(
        |kind| Some(Modifier::ForEachValue(kind)),
        |m| match m {
            Modifier::ForEachValue(kind) => Some(kind.clone()),
            _ => None,
        },
    );
    let where_x = lit(", where X is").then_keep(&a.amount).map(
        |n| Some(Modifier::WhereX(n)),
        |m| match m {
            Modifier::WhereX(n) => Some(n.clone()),
            _ => None,
        },
    );
    let at = lit("at the beginning of").then_keep(&step()).map(
        |(step, whose)| Some(Modifier::At { step, whose }),
        |m| match m {
            Modifier::At { step, whose } => Some((*step, whose.clone())),
            _ => None,
        },
    );
    let counted = alt("times", vec![crate::words::count_atom(), lit("that many").to(Amount::That { many: true })])
        .skip(&lit("times"))
        .map(|n| (n.clone() != Amount::Const(2)).then_some(n), |n| (*n != Amount::Const(2)).then(|| n.clone()));
    let times = alt("times", vec![lit("twice").to(Amount::Const(2)), counted]).map(
        |n| Some(Modifier::Times(n)),
        |m| match m {
            Modifier::Times(n) => Some(n.clone()),
            _ => None,
        },
    );
    // "with three time counters on it"
    let with_counters = lit("with")
        .then_keep(&crate::selector::counters_on())
        .map(
            |c| Some(Modifier::WithCounters(c)),
            |m| match m {
                Modifier::WithCounters(c) => Some(c.clone()),
                _ => None,
            },
        );
    let if_ = lit("if").then_keep(&conditions().condition).map(
        |c| Some(Modifier::If(c)),
        |m| match m {
            Modifier::If(c) => Some(c.clone()),
            _ => None,
        },
    );
    let as_long_as = lit("as long as").then_keep(&conditions().condition).map(
        |c| Some(Modifier::AsLongAs(c)),
        |m| match m {
            Modifier::AsLongAs(c) => Some(c.clone()),
            _ => None,
        },
    );
    let after_turn = alt("after turn",vec![
        lit("after this one").to(Modifier::AfterTurn(ir::effect::TurnRef::This)),
        lit("after this turn").to(Modifier::AfterTurn(ir::effect::TurnRef::This)).alias(),
        lit("after that turn").to(Modifier::AfterTurn(ir::effect::TurnRef::That)),
    ]);
    let during_turn = lit("during").then_keep(&crate::event::during()).map(
        |d| Some(Modifier::During(d)),
        |m| match m {
            Modifier::During(d) => Some(d.clone()),
            _ => None,
        },
    );
    // "at this turn's next end of combat" is the same point
    let at_end_of_combat =
        alt("at end of combat", vec![lit("at end of combat"), lit("at this turn's next end of combat").alias()]).to(Modifier::AtEndOfCombat);
    let duration = duration().map(
        |d| Some(Modifier::Duration(d)),
        |m| match m {
            Modifier::Duration(d) => Some(d.clone()),
            _ => None,
        },
    );
    let unless = unless();
    // "it becomes day as ~ enters" (614.1c); in front of a sentence it is the lead
    // ([`Lead::AsEnters`]), which governs the whole sentence, so it is not a leading modifier.
    // "As [this permanent] enters" (614.1c): one particular permanent ("~", "it", "that
    // creature"), never a player or a card in another zone
    let enters = |s: &Selector| matches!(s.set.domain, ir::Domain::Permanent | ir::Domain::Object) && matches!(s.quant, ir::Quant::Definite(_));
    let as_enters = lit("as").then_keep(&selectors().singular).skip(&lit("enters")).map(
        move |what| enters(&what).then_some(Modifier::AsEnters(what)),
        move |m| match m {
            Modifier::AsEnters(what) if enters(what) => Some(what.clone()),
            _ => None,
        },
    );
    let leading = alt(
        "leading modifier",
        vec![
            if_.clone(),
            duration.clone(),
            for_each.clone(),
            for_each_value.clone(),
            at.clone(),
            at_end_of_combat.clone(),
            as_long_as.clone(),
            during_turn.clone(),
            after_turn.clone(),
            unless.clone(),
        ],
    );
    let any = alt(
        "modifier",
        vec![
            for_each,
            for_each_value,
            where_x,
            unless,
            at,
            at_end_of_combat,
            duration,
            lit("instead").to(Modifier::Instead),
            lit("face down").to(Modifier::FaceDown),
            lit("as an Adventure").to(Modifier::AsAdventure),
            lit(", divided as you choose").to(Modifier::DividedAsYouChoose),
            lit("without paying their mana costs").to(Modifier::WithoutPayingManaCosts),
            // "… without paying its mana cost and as though it had flash": only after a free cast;
            // the plural is read with its free cast, below
            lit("and as though it had flash").to(Modifier::AsThoughFlash),
            lit("using its").then_keep(&crate::selector::keyword_name()).skip(&lit("ability")).map(
                |k| Some(Modifier::Using(k)),
                |m| match m {
                    Modifier::Using(k) => Some(*k),
                    _ => None,
                },
            ),
            lit("again").to(Modifier::Again),
            lit(", rounded up").to(Modifier::Rounded { up: true }),
            lit(", rounded down").to(Modifier::Rounded { up: false }),
            crate::procedure::in_pile(),
            with_counters,
            lit("instead of").then_keep(&crate::words::count_amount()).map(
                |n| Some(Modifier::InsteadOfCount(n)),
                |m| match m { Modifier::InsteadOfCount(n) => Some(n.clone()), _ => None },
            ),
            lit("for that player to ignore this effect until end of turn").to(Modifier::ToIgnoreThisEffect),
            lit("when you cast it").to(Modifier::WhenYouCastIt),
            lit("when ~ leaves the battlefield").to(Modifier::WhenThisLeaves),
            lit("except for").then_keep(&selectors().set_pl).map(
                |set| Some(Modifier::ExceptFor(set)),
                |m| match m {
                    Modifier::ExceptFor(set) => Some(set.clone()),
                    _ => None,
                },
            ),
            crate::statics::instead_of().map(
                |e| Some(Modifier::InsteadOf(e)),
                |m| match m {
                    Modifier::InsteadOf(e) => Some(e.clone()),
                    _ => None,
                },
            ),
            times,
            crate::except::except(),
            crate::statics::cast_cost(),
            if_,
            as_long_as,
            during_turn,
            after_turn,
            as_enters,
        ],
    );
    Modifiers { any, leading }
}

/// A keyword action done to "a"/"an" thing, done N times, names N of them: "flip three coins" is
/// flipping a coin three times, each coin one flip (705.1); "take two extra turns after this one"
/// is taking an extra turn after this one twice, the turns added one at a time (500.7). The act
/// is the keyword action with [`Modifier::Times`], which these actions print in this form ("flip
/// a coin twice" still reads, as a variant). The table is each such action's verb, and its
/// object counted.
fn counted_keyword(form: Form) -> P<(ir::KeywordAction, Amount)> {
    use ir::KeywordAction as K;
    let v = |third: &str, base: &str| crate::event::verb(form, third, base, "");
    let table = [(K::FlipCoin, v("flips", "flip"), "coins"), (K::ExtraTurn, v("takes", "take"), "extra turns")];
    let many = crate::words::count_atom().map(|n| (n != Amount::Const(1)).then_some(n), |n| (*n != Amount::Const(1)).then(|| n.clone()));
    alt(
        "counted keyword action",
        table
            .into_iter()
            .map(|(k, verb, nouns)| {
                verb.then_keep(&many).skip(&lit(nouns)).map(
                    move |n| Some((k, n)),
                    move |(kk, n): &(ir::KeywordAction, Amount)| (*kk == k).then(|| n.clone()),
                )
            })
            .collect(),
    )
}

fn act(form: Form) -> P<Act> {
    // "face down" modifies exiling; a move onto the battlefield says it as an entry state
    // and a number of modes is chosen as an action only to replace the header's ("choose both
    // instead"); alone it is the header
    let fits = |a: &Act| {
        let exile = matches!(a.action, ir::Action::Exile(_) | ir::Action::Until { .. });
        // "exile that card with three time counters on it": a card or spell gets the counters as
        // it is exiled; only a permanent can already have them (a filter)
        let last = match &a.action {
            ir::Action::Exile(o) => o.items.last(),
            _ => None,
        };
        // a definite reference ("~", "it", "that card") takes no filter
        let permanent =
            last.is_some_and(|s| s.set.domain == ir::Domain::Permanent && !matches!(s.quant, ir::Quant::Definite(_)));
        let ends_in_counters = last.is_some_and(|s| match &s.set.filter {
            ir::Filter::All(v) => matches!(v.last(), Some(ir::Filter::Atom(ir::Atom::Counters { .. }))),
            f => matches!(f, ir::Filter::Atom(ir::Atom::Counters { .. })),
        });
        let with_counters = a.mods.iter().any(|m| matches!(m, Modifier::WithCounters(_)));
        // a cast is paid with something (118.8, 118.9)
        let by = a.mods.iter().any(|m| matches!(m, Modifier::By(_)));
        let play = |zone: bool| match &a.action {
            ir::Action::Static(s) => matches!(s.as_ref(), ir::StaticAct::PlayFrom { from, .. } if from.is_some() == zone),
            _ => false,
        };
        // a permission to cast a kind of spell from no zone says what it is paid with
        let flash = matches!(&a.action, ir::Action::Static(s) if matches!(s.as_ref(), ir::StaticAct::CastAsThoughFlash { .. }));
        // "cast creature spells from among … by removing counters": the permission that says
        // what it is paid with is the static one, not a cast of some spells
        let some_spells = matches!(&a.action, ir::Action::Cast { what, .. }
            if matches!(what.items.as_slice(), [s] if s.set.domain == ir::Domain::Spell
                && s.quant == ir::Quant::Indefinite(ir::selector::Count::AtLeast(ir::Amount::Const(1)))));
        if by && some_spells {
            return false;
        }
        // "except it enters with a shield counter on it if you control that creature": the
        // condition is the exception's, not the whole instruction's
        let counters_then_if = a.mods.windows(2).any(|w| {
            matches!(&w[0], Modifier::Except(v) if matches!(v.last(), Some(ir::Exception { change: ir::CopyChange::EntersWith { if_: None, .. }, .. })))
                && matches!(w[1], Modifier::If(_))
        });
        if counters_then_if {
            return false;
        }
        // the rounding at the end of an act belongs to the acts whose half says none itself
        if a.mods.iter().any(|m| matches!(m, Modifier::Rounded { .. }))
            && !matches!(a.action, ir::Action::Create(_) | ir::Action::GetCounters { .. } | ir::Action::Move { .. } | ir::Action::GainLife(_))
        {
            return false;
        }
        // After independently guarded damage parts, another trailing "if" belongs to
        // the final part. A whole-action guard can still lead the sentence explicitly.
        if matches!(&a.action, ir::Action::DealDamage(hits) if hits.iter().any(|h| h.condition.is_some()))
            && a.mods.iter().any(|m| matches!(m, Modifier::If(_))) { return false; }
        // "… to their owners' hands except for Giants": said after where a move goes; right
        // after the things themselves it is part of their description
        if a.mods.iter().any(|m| matches!(m, Modifier::ExceptFor(_))) && !matches!(a.action, ir::Action::Move { .. }) {
            return false;
        }
        // "without paying their mana costs" as a modifier is said of a permission only; a cast
        // carries it itself
        let unpaid = a.mods.contains(&Modifier::WithoutPayingManaCosts);
        if unpaid && !(play(true) || play(false)) {
            return false;
        }
        // A free cast of card selectors has its own action form. The static spelling
        // remains for spell classes and for permissions specifying payment by a cost.
        if unpaid && !by && matches!(&a.action, ir::Action::Static(rule)
            if matches!(rule.as_ref(), ir::StaticAct::PlayFrom {what,from:None}
                if matches!(what.as_slice(), [one] if one.what.domain == ir::Domain::Card))) {
            return false;
        }
        // The controlled player's next turn is carried by the control action itself.
        // Other players' turns and shorter windows keep their explicit duration.
        if matches!(&a.action, ir::Action::Static(rule)
            if matches!(rule.as_ref(), ir::StaticAct::Controls(who) if who.set.domain == ir::Domain::Player))
            && a.mods.iter().any(|m| matches!(m, Modifier::During(ir::event::During::Step(ir::Step::Turn,
                ir::StepOwner::Player {who,next:true})) if matches!(who.as_ref(), Selector {
                    quant:ir::Quant::Definite(ir::Ref::Anaphor(ir::Anaphor::That | ir::Anaphor::They)),set,chooser:None,random:None
                } if *set == ir::ObjectSet::of(ir::Domain::Player)))) {
            return false;
        }
        // "cast noncreature spells from among those cards without paying their mana costs" is
        // that permission, not a free cast of some spells
        if some_spells && matches!(a.action, ir::Action::Cast { free: true, .. }) {
            return false;
        }
        (!by || matches!(a.action, ir::Action::Cast { free: false, .. }) || flash || play(true) || play(false))
            && (by || unpaid || !play(false))
            && (!a.mods.contains(&Modifier::FaceDown) || exile)
            && (!a.mods.contains(&Modifier::AsAdventure) || matches!(a.action, ir::Action::Cast { play: false, .. }))
            && (!a.mods.contains(&Modifier::AsThoughFlash) || match (&a.action, a.mods.as_slice()) {
                (ir::Action::Cast { play: false, free: true, .. }, [Modifier::AsThoughFlash]) => true,
                (ir::Action::Static(_), [Modifier::WithoutPayingManaCosts, Modifier::AsThoughFlash]) => true,
                _ => false,
            })
            && (!a.mods.iter().any(|m| matches!(m, Modifier::InPile(_))) || exile)
            && (!with_counters || (exile && !permanent))
            && !(exile && ends_in_counters && !permanent)
            && (!matches!(a.action, ir::Action::Choose(ir::Choice::Modes(_))) || a.mods.contains(&Modifier::Instead))
    };
    let plain = action(form).then(&modifier().many()).map(
        move |(action, mut mods): (ir::Action, Vec<Modifier>)| {
            // Counted keyword actions canonically put their count on the noun.
            if matches!(action,ir::Action::Keyword(ir::KeywordAction::FlipCoin | ir::KeywordAction::ExtraTurn))
                && let Some(at)=mods.iter().position(|m|matches!(m,Modifier::Times(_))) {
                let times=mods.remove(at);mods.insert(0,times);
            }
            let a = Act { action, mods, may: false };
            fits(&a).then_some(a)
        },
        move |a: &Act| (fits(a) && !a.may).then(|| (a.action.clone(), a.mods.clone())),
    );
    // "you may play it this turn without paying its mana cost": the free cast said after another
    // modifier. Read as the cast it is; it prints with the cost right after what is cast.
    let without = alt("without paying", vec![lit("without paying its mana cost"), lit("without paying their mana costs")]);
    let freed = action(form)
        .then(&modifier().many())
        .skip(&without)
        .then(&modifier().many())
        .map(
            move |((action, mut mods), more): ((ir::Action, Vec<Modifier>), Vec<Modifier>)| {
                let ir::Action::Cast { play, what, free: false } = action else { return None };
                if mods.is_empty() {
                    return None;
                }
                mods.extend(more);
                let a = Act { action: ir::Action::Cast { play, what, free: true }, mods, may: false };
                fits(&a).then_some(a)
            },
            |_: &Act| None,
        )
        .alias();
    // first, so that it prints: the count is the first modifier
    let counted = counted_keyword(form).then(&modifier().many()).map(
        move |((k, n), more)| {
            let a = Act { action: ir::Action::Keyword(k), mods: std::iter::once(Modifier::Times(n)).chain(more).collect(), may: false };
            fits(&a).then_some(a)
        },
        move |a: &Act| match (&a.action, a.mods.split_first()) {
            (ir::Action::Keyword(k), Some((Modifier::Times(n), more))) if fits(a) && !a.may => Some(((*k, n.clone()), more.to_vec())),
            _ => None,
        },
    );
    // "Any player may cast creature spells … without paying their mana costs and as though they
    // had flash": the free cast of a permission and its timing, in its number (Aluren)
    let both = || vec![Modifier::WithoutPayingManaCosts, Modifier::AsThoughFlash];
    let free_flash = action(form).skip(&lit("without paying their mana costs and as though they had flash")).map(
        move |action| {
            let a = Act { action, mods: both(), may: false };
            fits(&a).then_some(a)
        },
        move |a: &Act| (a.mods == both() && fits(a) && !a.may).then(|| a.action.clone()),
    );
    alt("act", vec![counted, free_flash, plain, freed])
}

/// The acts of one subject in one verb form, with "may" or without.
/// Acts in order: the first, then each with its join.
type Chain = (Act, Vec<(Join, Act)>);

fn predicate(form: Form, prefix: &P<()>) -> P<(bool, Chain)> {
    // a later act may have its own "may" ("you may copy ~ and may choose new targets for it")
    let may_act = lit("may").then_keep(&act(Form::Base)).map(
        |a| Some(Act { may: true, ..a }),
        |a: &Act| a.may.then(|| Act { may: false, ..a.clone() }),
    );
    let chain = move |a: &P<Act>| a.then(&join().then(&alt("act", vec![a.clone(), may_act.clone()])).many());
    alt(
        "predicate",
        vec![
            prefix.then_keep(&chain(&act(form))).map(|c| Some((false, c)), |(may, c)| (!may).then(|| c.clone())),
            lit("may").then_keep(prefix).then_keep(&chain(&act(Form::Base))).map(|c| Some((true, c)), |(may, c)| may.then(|| c.clone())),
        ],
    )
}

/// Can the clause's subject perform each of its actions? A player draws, an object deals
/// damage; with no subject the controller acts ([`crate::action::does`]).
fn subject_fits(c: &Clause) -> bool {
    use crate::action::does;
    fn fits(subject: Option<&Subject>, a: &ir::Action) -> bool {
        match subject {
            None => does(None, a),
            Some(Subject::Selector(s) | Subject::Each(s)) => does(Some(s), a),
            Some(Subject::Set(set)) => does(Some(&Selector::new(ir::Quant::All, set.clone())), a),
            Some(Subject::And { items, .. }) => items.iter().all(|s| fits(Some(s), a)),
            Some(Subject::LifeTotal(_)) => true,
        }
    }
    std::iter::once(&c.first).chain(c.rest.iter().map(|(_, a)| a)).all(|a| fits(c.subject.as_ref(), &a.action))
}

/// After a prohibition, "or" goes on with what is forbidden ("can't lose the game or win the
/// game"): it never offers the prohibition and another act as a choice.
fn or_after_cant(c: &Clause) -> bool {
    let acts = std::iter::once(&c.first).chain(c.rest.iter().map(|(_, a)| a));
    acts.zip(&c.rest).any(|(before, (join, _))| *join == Join::Or && matches!(before.action, ir::Action::Cant(_)))
}

fn clause(subject: &P<Option<Subject>>, form: Form) -> P<Clause> {
    clause_prefixed(subject, form, &empty())
}

fn clause_prefixed(subject: &P<Option<Subject>>, form: Form, prefix: &P<()>) -> P<Clause> {
    subject.then(&lit("also").opt()).then(&predicate(form, prefix)).map(
        |((subject, also), (may, (first, rest)))| {
            let c = Clause { subject, may, also: also.is_some(), first, rest };
            (subject_fits(&c) && !or_after_cant(&c) && !or_after_unless(&c)).then_some(c)
        },
        |c: &Clause| {
            (subject_fits(c) && !or_after_cant(c) && !or_after_unless(c))
                .then(|| ((c.subject.clone(), c.also.then_some(())), (c.may, (c.first.clone(), c.rest.clone()))))
        },
    )
}

/// "They're still lands", "They're 2/2 Horror creatures", "It's still a land": the pronoun and its
/// verb are one word (other subjects spell [`ir::Action::Is`] "is").
fn contraction(word: &'static str, subject: Selector, number: crate::words::Number) -> P<Clause> {
    let is = crate::becomes::becoming(&[number])
        .map(|b| Some(Act { action: ir::Action::Is(b), mods: Vec::new(), may: false }), |a: &Act| match &a.action {
            ir::Action::Is(b) => Some(b.clone()),
            _ => None,
        })
        .then(&modifier().many())
        .map(
            |(mut a, mods)| {
                // a cast is what is paid for (118.8, 118.9)
                let by = mods.iter().any(|m| matches!(m, Modifier::By(_)));
                a.mods = mods;
                (!by).then_some(a)
            },
            |a: &Act| (!a.may).then(|| (Act { action: a.action.clone(), mods: Vec::new(), may: false }, a.mods.clone())),
        );
    let s2 = subject.clone();
    let form=match number { crate::words::Number::Sg=>Form::Third, crate::words::Number::Pl=>Form::Base };
    lit(word).then_keep(&is).then(&join().then(&act(form)).many()).map(
        move |(first,rest)| {
            let clause=Clause { subject: Some(Subject::Selector(subject.clone())), may: false, also: false, first, rest };
            (subject_fits(&clause) && !or_after_cant(&clause) && !or_after_unless(&clause)).then_some(clause)
        },
        move |c: &Clause| match (&c.subject, c.may || c.also) {
            (Some(Subject::Selector(s)), false) if *s == s2 && subject_fits(c) && !or_after_cant(c) && !or_after_unless(c) => Some((c.first.clone(),c.rest.clone())),
            _ => None,
        },
    )
}

/// "your life total becomes 10", "each player's life total becomes the highest life total among
/// all players" (119.5). Before an expression "equal to" says the same ("becomes equal to your
/// starting life total"); the corpus says it less often than the bare amount, and never before
/// a number. An optional setting is the controller's choice (`may`), which English says with
/// the causative: "you may have your life total become the number of charge counters on ~"; the
/// life total is not a doer, so a causative without "may" is not read.
fn life_total_clause() -> P<Clause> {
    let quantity = crate::words::is_quantity;
    let equal_to = lit("equal to")
        .then_keep(&amounts().amount)
        .map(move |n| (!quantity(&n)).then_some(n), move |n| (!quantity(n)).then(|| n.clone()));
    let value = alt("life total value", vec![amounts().amount.clone(), equal_to.alias()]);
    let becomes = |verb: &str| {
        lit(verb)
            .then_keep(&value)
            .map(|n| Some(Act { action: ir::Action::LifeTotalBecomes(n), mods: Vec::new(), may: false }), |a: &Act| match &a.action {
                ir::Action::LifeTotalBecomes(n) if a.mods.is_empty() && !a.may => Some(n.clone()),
                _ => None,
            })
    };
    let clause = |may: bool| {
        let whose = selectors().possessive.skip(&lit("life total"));
        let whose = if may { lit("you may have").then_keep(&whose) } else { whose };
        whose.then(&becomes(if may { "become" } else { "becomes" })).map(
            move |(p, first)| Some(Clause { subject: Some(Subject::LifeTotal(p)), may, also: false, first, rest: Vec::new() }),
            move |c: &Clause| match (&c.subject, c.may == may && !c.also, c.rest.is_empty()) {
                (Some(Subject::LifeTotal(p)), true, true) => Some((p.clone(), c.first.clone())),
                _ => None,
            },
        )
    };
    alt("life total clause", vec![clause(false), clause(true)])
}

/// "Damage that would reduce your life total to less than 1 reduces it to 1 instead" (614.1a,
/// 120.3a): a static act of the player whose life total it names, who is the clause's subject;
/// as any act, it takes a lead ("If you control a creature, …", "Until end of turn, …").
fn life_floor_clause() -> P<Clause> {
    crate::statics::damage_life_floor().map(
        |(player, act)| {
            let first = Act { action: ir::Action::Static(Box::new(act)), mods: Vec::new(), may: false };
            Some(Clause { subject: Some(Subject::Selector(player)), may: false, also: false, first, rest: Vec::new() })
        },
        |c: &Clause| match (&c.subject, &c.first.action) {
            (Some(Subject::Selector(player)), ir::Action::Static(act)) if !c.may && !c.also && c.rest.is_empty() && c.first.mods.is_empty() && !c.first.may => {
                Some((player.clone(), (**act).clone()))
            }
            _ => None,
        },
    )
}

/// "Each player who has cast a nonartifact spell this turn can't cast additional nonartifact
/// spells": each turn, each player may cast one such spell and no more, counting those cast
/// before the ability applied (Ethersworn Canonist rulings of 2020-08-07). That is the
/// prohibition (101.2) "Each player can't cast more than one nonartifact spell each turn" says,
/// and it is printed as that.
fn additional_cast_clause() -> P<Clause> {
    let s = selectors();
    let a = alt("a", vec![lit("a"), lit("an").alias()]);
    lit("each player who has cast").skip(&a).then_keep(&s.set_sg)
        .skip(&lit("this turn can't cast additional")).then(&s.set_pl)
        .map(
            |(one, more)| {
                if one != more || one.domain != ir::Domain::Spell { return None; }
                let spells = Selector::new(ir::Quant::Indefinite(ir::Count::AtLeast(Amount::Const(2))), one);
                let cant = ir::Action::Cant(vec![ir::Forbidden { event: ir::EventAction::does(ir::Verb::Cast, spells), within: Some(ir::Within::EachTurn) }]);
                let players = Selector::new(ir::Quant::Each, ir::ObjectSet::of(ir::Domain::Player));
                Some(Clause { subject: Some(Subject::Selector(players)), may: false, also: false, first: Act { action: cant, mods: Vec::new(), may: false }, rest: Vec::new() })
            },
            |_: &Clause| None,
        )
        .alias()
}

/// "Each spell costs {3} more to cast except during its controller's turn." (Defense Grid): the
/// spells whose controller isn't the active player, so their controller's turn is not now
/// (102.1, 601.2f); said as "each spell the active player doesn't control".
fn except_controller_turn_cost_clause() -> P<Clause> {
    let more = alt("more or less", vec![lit("more").to(true), lit("less").to(false)]);
    lit("each").then_keep(&selectors().set_sg).skip(&lit("costs")).then(&crate::mana::mana_cost()).then(&more)
        .skip(&lit("to cast except during its controller's turn"))
        .map(
            |((mut set, by), more)| {
                if set.domain != ir::Domain::Spell || set.controller.is_some() { return None; }
                let active = Selector::new(ir::Quant::Definite(ir::Ref::ActivePlayer), ir::ObjectSet::of(ir::Domain::Player));
                set.controller = Some(ir::selector::Relation { who: Box::new(active), negated: true, past: false });
                let cost = ir::Action::Static(Box::new(ir::StaticAct::CostChange { more, by, of: ir::CostOf::Cast }));
                Some(Clause { subject: Some(Subject::Selector(Selector::new(ir::Quant::Each, set))), may: false, also: false, first: Act { action: cost, mods: Vec::new(), may: false }, rest: Vec::new() })
            },
            |_: &Clause| None,
        )
        .alias()
}

/// "after this phase, there is an additional combat phase", "after this main phase, there are two
/// additional combat phases followed by an additional main phase", "there is an additional
/// beginning phase after this phase": phases added directly after the one named (500.8). Nobody
/// does it, so the clause has no subject (as "it becomes day" has none), and it joins other
/// clauses as any clause does ("untap all attacking creatures and after this phase, there is an
/// additional combat phase").
fn additional_phase_clause() -> P<Clause> {
    use ir::action::AfterPhase;
    use ir::turn::Phase;
    let after = lit("after").then_keep(&alt(
        "this phase",
        vec![
            lit("this phase").to(AfterPhase::This),
            lit("this main phase").to(AfterPhase::ThisMain),
            lit("this combat phase").to(AfterPhase::ThisCombat),
        ],
    ));
    let kind = alt("added phase", vec![lit("combat").to(Phase::Combat), lit("beginning").to(Phase::Beginning)]);
    let there_is = alt("there is", vec![lit("there is"), lit("there's").alias()]);
    let one = there_is.skip(&lit("an additional")).then_keep(&kind).skip(&lit("phase")).map(
        |k| Some((Amount::Const(1), k)),
        |(n, k): &(Amount, Phase)| (*n == Amount::Const(1)).then_some(*k),
    );
    let many = lit("there are").then_keep(&crate::words::count_atom()).skip(&lit("additional")).then(&kind).skip(&lit("phases")).map(
        |(n, k)| (n != Amount::Const(1)).then_some((n, k)),
        |(n, k): &(Amount, Phase)| (*n != Amount::Const(1)).then(|| (n.clone(), *k)),
    );
    let added = alt("additional phases", vec![one, many]);
    let followed = lit("followed by an additional main phase").opt().map(|o| Some(o.is_some()), |b: &bool| Some(b.then_some(())));
    let spec = |((after, (count, kind)), then_main): ((AfterPhase, (Amount, Phase)), bool)| {
        Some(ir::Action::AdditionalPhase { after, kind, count, then_main })
    };
    let unspec = |a: &ir::Action| match a {
        ir::Action::AdditionalPhase { after, kind, count, then_main } => Some(((*after, (count.clone(), *kind)), *then_main)),
        _ => None,
    };
    let leading = after.skip(&lit(",")).then(&added).then(&followed).map(spec, unspec);
    // the time phrase last, as about a third of the corpus says it ("there is an additional
    // combat phase after this phase[,] followed by an additional main phase"): the same value,
    // printed with the time phrase first
    let followed_after = alt(
        "followed by",
        vec![
            lit(", followed by an additional main phase").to(true),
            lit("followed by an additional main phase").to(true),
            empty().to(false),
        ],
    );
    let trailing = added
        .then(&after)
        .then(&followed_after)
        .map(move |((added, after), then_main)| spec(((after, added), then_main)), move |a| unspec(a).map(|((after, added), m)| ((added, after), m)));
    let added_phase = alt("additional phase", vec![leading, trailing.alias()]);
    added_phase.map(
        |action| Some(Clause { subject: None, may: false, also: false, first: Act { action, mods: Vec::new(), may: false }, rest: Vec::new() }),
        |c: &Clause| match (&c.subject, c.may || c.also || c.first.may, c.rest.is_empty() && c.first.mods.is_empty()) {
            (None, false, true) if matches!(c.first.action, ir::Action::AdditionalPhase { .. }) => Some(c.first.action.clone()),
            _ => None,
        },
    )
}

/// ", except …" runs to the end of its sentence: only the last act may have exceptions. So does
/// a villainous choice (701.55a): what follows its second option is that option's, so it is the
/// last act and nothing modifies it.
fn except_last(b: &Body) -> bool {
    let acts: Vec<&Act> = std::iter::once(&b.first)
        .chain(b.rest.iter().map(|(_, c)| c))
        .flat_map(|c| std::iter::once(&c.first).chain(c.rest.iter().map(|(_, a)| a)))
        .collect();
    let has = |a: &Act| a.mods.iter().any(|m| matches!(m, Modifier::Except(_)));
    let villainous = |a: &Act| matches!(a.action, ir::Action::FaceVillainousChoice(_));
    acts.iter().rev().skip(1).all(|a| !has(a) && !villainous(a)) && acts.iter().all(|a| !villainous(a) || a.mods.is_empty())
}

fn build() -> Effects {
    let all = subjects();
    let it = Selector::new(
        ir::Quant::Definite(ir::Ref::Anaphor(ir::selector::Anaphor::It)),
        ir::ObjectSet::of(ir::Domain::Object),
    );
    // the contractions first: they print before "it is"
    let mut clauses: Vec<P<Clause>> = vec![
        contraction("they're", they_objects(), crate::words::Number::Pl),
        contraction("it's", it, crate::words::Number::Sg),
    ];
    clauses.extend(all.iter().map(|(s, f)| clause(s, *f)));
    clauses.push(life_total_clause());
    clauses.push(life_floor_clause());
    clauses.push(additional_cast_clause());
    clauses.push(except_controller_turn_cost_clause());
    clauses.push(additional_phase_clause());
    let any_clause = alt("clause", clauses);
    // a later clause names its subject (an instruction after an object's clause, "All creatures
    // lose indestructible until end of turn, then destroy all creatures", is not read yet: its
    // doer is the controller, not the clause's subject)
    let mut subjected: Vec<P<Clause>> = all.iter().skip(1).map(|(s, f)| clause(s, *f)).collect();
    subjected.push(life_total_clause());
    subjected.push(additional_phase_clause());
    let subjected = alt("clause with subject", subjected);
    let body = any_clause.then(&join().then(&subjected).many()).map(
        |(first, rest)| {
            let b = Body { first, rest };
            except_last(&b).then_some(b)
        },
        |b: &Body| except_last(b).then(|| (b.first.clone(), b.rest.clone())),
    );
    let c = conditions();
    let plain = body.map(
        |body| Some(match crate::statics::ability_cost_sentence(&body) {
            Some(rule) => Sentence::Static(Box::new(rule)),
            None => Sentence::Do(body),
        }),
        |s| match s {
            Sentence::Do(body) if crate::statics::ability_cost_sentence(body).is_none() => Some(body.clone()),
            _ => None,
        },
    );

    // A predicate prefix scopes one complete clause, not later clauses with other actors.
    // The ordinary leading form prints the same scope without duplicating clause grammar.
    let simultaneous = alt("simultaneous clause", all.iter().map(|(subject, form)| {
        clause_prefixed(subject, *form, &lit("simultaneously")).map(
            |first| {
                if first.rest.iter().any(|(join, _)| matches!(join, Join::CommaThen | Join::Or)) { return None; }
                let body = Body { first, rest: Vec::new() };
                (except_last(&body) && crate::statics::ability_cost_sentence(&body).is_none()).then_some(
                    Sentence::Led { lead: Lead::Simultaneously, rest: Box::new(Sentence::Do(body)) },
                )
            },
            |_: &Sentence| None,
        )
    }).collect()).alias();

    // A repeated procedure names new classes, not a quantity of objects to act on yet.
    let same_way = alt("procedure application", all.iter().map(|(subject, form)| {
        use ir::lexicon::{Verb, Role, Doer, Inflection};
        let inflection = if *form == Form::Third { Inflection::Third } else { Inflection::Base };
        let verbs = crate::kernel::table("procedure action", Verb::ALL.iter().copied().filter(|v| {
            let entry = v.entry();
            !entry.word.is_empty() && v.tail().is_empty() && entry.doer == Doer::Player
                && matches!(entry.roles, [(Role::What, _)])
        }).map(|v| (v.head(inflection), v)).collect());
        let action = verbs.then(&selectors().set_pl).map(
            |(verb, set)| ir::lexicon::fills(&Selector::new(ir::Quant::All, set.clone()), verb, Role::What)
                .then_some((verb, set)),
            |(verb, set): &(Verb, ir::ObjectSet)| ir::lexicon::fills(&Selector::new(ir::Quant::All, set.clone()), *verb, Role::What)
                .then(|| (*verb, set.clone())),
        );
        let actions = alt("procedure actions", vec![
            action.map(|a| Some(vec![a]), |v: &Vec<(Verb, ir::ObjectSet)>| (v.len() == 1).then(|| v[0].clone())),
            action.list("and"),
        ]);
        subject.then(&actions).skip(&lit("the same way")).map(
            |(subject, actions)| match subject {
                Some(subject) if match &subject {
                    Subject::Selector(s) => s.set.domain == ir::Domain::Player,
                    Subject::Set(s) => s.domain == ir::Domain::Player,
                    _ => false,
                } => Some(Sentence::SameWay { subject, actions }),
                _ => None,
            },
            |s| match s { Sentence::SameWay { subject, actions } => Some((Some(subject.clone()), actions.clone())), _ => None },
        )
    }).collect());
    let restrictions = lit("activate").then_keep(&c.restrictions).map(
        |r| Some(Sentence::Restrictions(r)),
        |x| match x {
            Sentence::Restrictions(r) => Some(r.clone()),
            _ => None,
        },
    );
    let rider = alt(
        "rider",
        vec![
            lit("this ability triggers only once each turn").to(Rider::TriggersOnlyOnceEachTurn),
            lit("this ability triggers only once").to(Rider::TriggersOnlyOnce),
            lit("repeat this process until no one puts a card onto the battlefield").to(Rider::RepeatUntilNoOnePuts),
            lit("do this only once each turn").to(Rider::DoThisOnlyOnceEachTurn),
            lit("any player may activate this ability").to(Rider::AnyPlayerMayActivate),
            lit("only your opponents may activate this ability").to(Rider::OnlyOpponentsMayActivate),
            alt(
                "who may activate",
                vec![
                    lit("any player may activate this ability but").to(false),
                    lit("only your opponents may activate this ability and").to(true),
                ],
            )
            .then(&c.restrictions)
            .map(
                |(opponents, restrictions)| Some(Rider::MayActivateOnly { opponents, restrictions }),
                |r| match r {
                    Rider::MayActivateOnly { opponents, restrictions } => Some((*opponents, restrictions.clone())),
                    _ => None,
                },
            ),
            lit("you may choose the same mode more than once").to(Rider::SameModeMoreThanOnce),
            lit("this ability can't cause the total number of")
                .then_keep(&crate::selector::counter_kind())
                .skip(&lit("counters on ~ to be greater than"))
                .then(&crate::words::count_amount())
                .map(
                    |(kind, max)| Some(Rider::CounterCap { kind, max }),
                    |r| match r {
                        Rider::CounterCap { kind, max } => Some((kind.clone(), max.clone())),
                        _ => None,
                    },
                ),
            lit("the escape cost is equal to")
                .then_keep(&alt("whose", vec![lit("the card's").to(false), lit("its").to(true)]))
                .skip(&lit("mana cost plus exile"))
                .then(&crate::words::count_amount())
                .skip(&lit("other cards from your graveyard"))
                .map(
                    |(its, cards)| Some(Rider::EscapeCostManaPlusExile { cards, its }),
                    |r| match r {
                        Rider::EscapeCostManaPlusExile { cards, its } => Some((*its, cards.clone())),
                        _ => None,
                    },
                ),
            lit("mana of any type can be spent to cast a spell this way").to(Rider::AnyManaThisWay { plural: false }),
            lit("mana of any type can be spent to cast spells this way").to(Rider::AnyManaThisWay { plural: true }),
            lit("creatures it was blocking that had become blocked by only ~ this combat become unblocked")
                .to(Rider::BlockedOnlyByBecomeUnblocked { that_creature: false }),
            lit("creatures it was blocking that had become blocked by only that creature this combat become unblocked")
                .to(Rider::BlockedOnlyByBecomeUnblocked { that_creature: true }),
            lit("the copy targets").then_keep(&selectors().selector).map(
                |s| Some(Rider::CopyTargets(s)),
                |r| match r {
                    Rider::CopyTargets(s) => Some(s.clone()),
                    _ => None,
                },
            ),
            lit("the new target must be").then_keep(&selectors().selector).map(
                |s| Some(Rider::NewTargetMustBe(s)),
                |r| match r {
                    Rider::NewTargetMustBe(s) => Some(s.clone()),
                    _ => None,
                },
            ),
            lit("if two or more")
                .then_keep(&alt("tied things", vec![lit("creatures").to(false), lit("cards").to(true)]))
                .skip(&lit("are tied for"))
                .then(&alt("superlative", vec![lit("greatest").to(true), lit("least").to(false)]))
                .then(&crate::selector::stat())
                .skip(&lit(", you choose one of them"))
                .map(
                    |((cards, greatest), stat)| Some(Rider::TiedYouChoose { cards, greatest, stat }),
                    |r| match r {
                        Rider::TiedYouChoose { cards, greatest, stat } => Some(((*cards, *greatest), *stat)),
                        _ => None,
                    },
                ),
            lit("each mode must target a different player").to(Rider::ModesTargetDifferentPlayers),
            lit("each copy targets a different one of").then_keep(&selectors().plural).map(
                |s| Some(Rider::CopiesTargetDifferent(s)),
                |r| match r {
                    Rider::CopiesTargetDifferent(s) => Some(s.clone()),
                    _ => None,
                },
            ),
            lit("this effect reduces only the amount of")
                .then_keep(&alt(
                    "colored",
                    vec![
                        lit("colored").to(None),
                        crate::kernel::table("color", ir::Color::ALL.iter().map(|c| (c.word().to_string(), Some(*c))).collect()),
                    ],
                ))
                .skip(&lit("mana you pay"))
                .map(
                    |c| Some(Rider::ReducesOnly(c)),
                    |r| match r {
                        Rider::ReducesOnly(c) => Some(*c),
                        _ => None,
                    },
                ),
            // "…, and that spell can't be countered": what the mana does to the spell it is
            // spent on (106.6)
            lit("spend only").then_keep(&crate::mana::spendable_mana()).then(&crate::mana::spendable_on()).map(
                |(mana, on)| Some(Rider::SpendOnlyMana { mana, on }),
                |r| match r {
                    Rider::SpendOnlyMana { mana, on } => Some((mana.clone(), on.clone())),
                    _ => None,
                },
            ),
            lit("spend this mana only on costs that contain {X}").to(Rider::SpendOnlyOnXCosts),
            lit("excess damage is dealt to that creature's controller instead").to(Rider::ExcessToController),
            lit("spend this mana only to pay cumulative upkeep costs").to(Rider::SpendOnlyOnCumulativeUpkeep),
            lit("this cost is reduced by")
                .then_keep(&crate::mana::mana_cost())
                .then(&crate::kernel::lazy("for each", || crate::amount::amounts().for_each.clone()))
                .map(|(by, each)| Some(Rider::CostReducedForEach { by, each }), |r| match r {
                    Rider::CostReducedForEach { by, each } => Some((by.clone(), each.clone())),
                    _ => None,
                }),
            alt("its foretell cost", vec![lit("its foretell cost is equal to its mana cost reduced by"), lit("its foretell cost is its mana cost reduced by").alias()])
                .then_keep(&crate::mana::mana_cost())
                .map(|c| Some(Rider::ForetellCostReduced(c)), |r| match r {
                    Rider::ForetellCostReduced(c) => Some(c.clone()),
                    _ => None,
                }),
            lit("spend this mana only")
                .then_keep(&crate::mana::spend_to())
                .then(&lit(", and that spell can't be countered").opt())
                .map(
                    |(on, uncounterable)| (uncounterable.is_none() || crate::mana::casts(&on)).then_some(Rider::SpendOnly { on, uncounterable: uncounterable.is_some() }),
                    |r| match r {
                        Rider::SpendOnly { on, uncounterable } if !uncounterable || crate::mana::casts(on) => Some((on.clone(), uncounterable.then_some(()))),
                        _ => None,
                    },
                ),
        ],
    )
    .map(
        |r| Some(Sentence::Rider(r)),
        |x| match x {
            Sentence::Rider(r) => Some(r.clone()),
            _ => None,
        },
    );
    let same_for = {
        use ir::effect::SameFor;
        alt(
            "same for",
            vec![
                crate::selector::keyword_name().list("and").map(
                    |k| Some(SameFor::Keywords(k)),
                    |f| match f {
                        SameFor::Keywords(k) => Some(k.clone()),
                        _ => None,
                    },
                ),
                selectors().set_pl.list("and").map(
                    |v| Some(SameFor::Sets(v)),
                    |f| match f {
                        SameFor::Sets(v) => Some(v.clone()),
                        _ => None,
                    },
                ),
            ],
        )
    };
    let same_is_true = lit("the same is true for").then_keep(&same_for).map(
        |f| Some(Sentence::SameIsTrueFor(f)),
        |x| match x {
            Sentence::SameIsTrueFor(f) => Some(f.clone()),
            _ => None,
        },
    );
    let cast_restrictions = lit("cast ~").then_keep(&c.restrictions).map(
        |r| Some(Sentence::CastRestrictions(r)),
        |x| match x {
            Sentence::CastRestrictions(r) => Some(r.clone()),
            _ => None,
        },
    );
    let modes = alt(
        "modes",
        vec![
            lit("one or both").to(Modes::OneOrBoth),
            lit("one or more").to(Modes::OneOrMore),
            lit("both").to(Modes::Both),
            lit("any number").to(Modes::AnyNumber),
            lit("X").to(Modes::X),
            // "Choose up to five {P} worth of modes." (700.2i)
            lit("up to").then_keep(&crate::words::number_words())
                .skip(&crate::kernel::leaf("{P}", |k| matches!(k, oracle::token::Kind::Symbol(s) if s == "P").then_some(()), |()| Some(oracle::token::Kind::Symbol("P".into()))))
                .skip(&lit("worth of modes")).map(
                |n| Some(Modes::Pawprints(n)),
                |m| match m {
                    Modes::Pawprints(n) => Some(*n),
                    _ => None,
                },
            ),
            lit("up to").then_keep(&crate::words::number_words()).map(
                |n| Some(Modes::UpTo(n)),
                |m| match m {
                    Modes::UpTo(n) => Some(*n),
                    _ => None,
                },
            ),
            crate::words::number_words().map(
                |n| Some(Modes::Exactly(n)),
                |m| match m {
                    Modes::Exactly(n) => Some(*n),
                    _ => None,
                },
            ),
        ],
    );
    let unchosen = alt(
        "unchosen",
        vec![
            lit("that hasn't been chosen this turn").to(Unchosen::ThisTurn),
            lit("that hasn't been chosen").to(Unchosen::Ever),
        ],
    );
    let random = lit("at random").opt().map(|o| Some(o.is_some()), |b| Some(b.then_some(())));
    // who chooses: the controller ("Choose one —"), or another player ("An opponent chooses one
    // —", 700.2e): one player, not each of several, and one who is never the controller, an
    // opponent (102.3); "you" is the controller, said only by the imperative
    let opponent = |f: &ir::Filter| match f {
        ir::Filter::Atom(ir::Atom::Opponent) => true,
        ir::Filter::All(v) => v.contains(&ir::Filter::Atom(ir::Atom::Opponent)),
        _ => false,
    };
    let player = move |s: &Selector| {
        s.set.domain == ir::Domain::Player && opponent(&s.set.filter) && !matches!(s.quant, ir::Quant::Each | ir::Quant::All)
    };
    let chooser = alt(
        "chooser",
        vec![
            lit("choose").to(None),
            selectors().singular.skip(&lit("chooses")).map(
                move |who| player(&who).then_some(Some(who)),
                move |c: &Option<Selector>| c.as_ref().filter(|s| player(s)).cloned(),
            ),
        ],
    );
    // the dash ends a header that ends its row; before more sentences it takes a period. But
    // another player's choice of modes is always a header with its dash: "Target opponent
    // chooses one." is a choice of one of the objects just named, not of modes
    let dashed = alt("header end", vec![lit("—").to(true), empty().alias().to(false)]);
    let choose_modes = chooser.then(&modes).then(&random).then(&unchosen.opt()).then(&dashed).map(
        // chance picks one mode, and not one that was held back; a player who chooses is not
        // chance
        |((((chooser, modes), random), unchosen), dashed)| {
            (!random || (modes == Modes::Exactly(1) && unchosen.is_none() && chooser.is_none()))
                .then_some(())
                .filter(|()| dashed || chooser.is_none())
                .map(|()| Sentence::ChooseModes { modes, unchosen, random, chooser })
        },
        |x| match x {
            Sentence::ChooseModes { modes, unchosen, random, chooser } => Some(((((chooser.clone(), modes.clone()), *random), *unchosen), true)),
            _ => None,
        },
    );
    // What leads a sentence. With a comma: a modifier in front ("If …,", "Until end of turn,",
    // "For each opponent,", "At the beginning of the next end step,", "As long as …,"), a delayed
    // trigger ("When that creature dies this turn,", "Whenever a creature you control dies this
    // turn,", "When you next cast a creature spell this turn,"), "When you do,", "Otherwise,",
    // "Starting with you,", "Once each turn,", "As ~ enters,". Without: "Then", and "instead"
    // ("If …, instead …").
    let when = alt("when", vec![lit("when").to(false), lit("whenever").to(true)]);
    let limit = alt("limit", vec![lit("this turn").to(DelayLimit::ThisTurn), lit("this way").to(DelayLimit::ThisWay)]);
    let delayed = when.then(&crate::event::events().delayed).then(&limit.opt()).map(
        |((whenever, (pattern, next)), limit)| Some(Lead::When { pattern, limit, whenever, next }),
        |l| match l {
            Lead::When { pattern, limit, whenever, next } => Some(((*whenever, (pattern.clone(), *next)), *limit)),
            _ => None,
        },
    );
    // "Starting with the next opponent in turn order,"
    let next_opponent = lit("starting with the next opponent in turn order").to(Lead::StartingWithNextOpponent);
    let starting_with = lit("starting with").then_keep(&selectors().selector).map(
        |who| Some(Lead::StartingWith(who)),
        |l| match l {
            Lead::StartingWith(who) => Some(who.clone()),
            _ => None,
        },
    );
    let as_enters = lit("as").then_keep(&selectors().singular).skip(&lit("enters")).map(
        |what| Some(Lead::AsEnters(what)),
        |l| match l {
            Lead::AsEnters(what) => Some(what.clone()),
            _ => None,
        },
    );
    let as_either = lit("as").then_keep(&selectors().singular).skip(&lit("enters or is turned face up")).map(
        |what| Some(Lead::AsEntersOrTurnedFaceUp(what)),
        |l| match l {
            Lead::AsEntersOrTurnedFaceUp(what) => Some(what.clone()),
            _ => None,
        },
    );
    let as_turned_face_up = lit("as").then_keep(&selectors().singular).skip(&lit("is turned face up")).map(
        |what| Some(Lead::AsTurnedFaceUp(what)),
        |l| match l {
            Lead::AsTurnedFaceUp(what) => Some(what.clone()),
            _ => None,
        },
    );
    let untapping = alt("untapping player", all.iter().map(|(subject, form)| {
        subject.skip(&lit(if *form == Form::Third { "untaps" } else { "untap" }))
            .then(&selectors().possessive).skip(&lit("permanents")).map(
                |(subject, possessor)| match subject {
                    Some(Subject::Selector(who)) if who.set.domain == ir::Domain::Player && matches!(who.quant, ir::Quant::Definite(_)) && who == possessor => Some(who),
                    _ => None,
                },
                |who: &Selector| Some((Some(Subject::Selector(who.clone())), who.clone())),
            )
    }).collect());
    let as_untaps = lit("during").then_keep(&crate::event::span(true)).skip(&lit(", as"))
        .then(&untapping).map(
            |((step, owner), actor)| match owner {
                ir::StepOwner::Player { who, next } if step == ir::Step::Untap && *who == actor =>
                    Some(Lead::AsPermanentsUntap { who: actor, next }),
                _ => None,
            },
            |lead| match lead {
                Lead::AsPermanentsUntap { who, next } => Some(((ir::Step::Untap,
                    ir::StepOwner::Player { who: Box::new(who.clone()), next: *next }), who.clone())),
                _ => None,
            },
        );
    let leading = modifiers().leading.map(
        |m| Some(Lead::Modifier(m)),
        |l| match l {
            Lead::Modifier(m) if *m != Modifier::Instead => Some(m.clone()),
            _ => None,
        },
    );
    let with_comma = alt(
        "lead",
        vec![
            leading,
            delayed,
            lit("when").then_keep(&conditions().condition).map(
                |c| match c {
                    ir::Condition::ManaSpent { mana: ir::condition::SpentMana::That, on, negated: false } => Some(Lead::WhenManaSpent { on }),
                    _ => None,
                },
                |l| match l {
                    Lead::WhenManaSpent { on } => Some(ir::Condition::ManaSpent { mana: ir::condition::SpentMana::That, on: on.clone(), negated: false }),
                    _ => None,
                },
            ),
            // "When you spend this mana to cast a Dragon creature spell,": the same trigger, said
            // of the player
            lit("when you spend this mana to")
                .then_keep(&crate::mana::spend_on())
                .map(|on| Some(Lead::WhenManaSpent { on }), |_: &Lead| None)
                .alias(),
            lit("when it regenerates this way").to(Lead::WhenRegeneratesThisWay),
            lit("as ~ transforms into ~").to(Lead::AsTransformsIntoThis),
            lit("when you do").to(Lead::WhenYouDo),
            lit("when you pay this cost one or more times").to(Lead::WhenPaidTimes),
            lit("otherwise").to(Lead::Otherwise),
            lit("simultaneously").to(Lead::Simultaneously),
            next_opponent,
            starting_with,
            lit("once each turn").to(Lead::Once { yours: false }),
            lit("once during each of your turns").to(Lead::Once { yours: true }),
            as_either,
            as_enters,
            as_turned_face_up,
            as_untaps,
        ],
    )
    .skip(&lit(","));
    let bare = alt("lead", vec![lit("then").to(Lead::Then), lit("instead").to(Lead::Modifier(Modifier::Instead))]);
    let any_sentence = forward::<Sentence>("sentence");
    let led = alt("lead", vec![with_comma, bare]).then(&any_sentence.rule()).map(
        |(lead, rest)| Some(Sentence::Led { lead, rest: Box::new(rest) }),
        |x| match x {
            Sentence::Led { lead, rest } => Some((lead.clone(), (**rest).clone())),
            _ => None,
        },
    );
    // alternatives ("sacrifice a creature or pay {3}") are the cost's own coordination
    let additional = lit("as an additional cost to cast ~,")
        .then_keep(&lit("you may").opt())
        .then(&crate::cost::costs().cost)
        .map(
            |(optional, cost)| Some(Sentence::AdditionalCost { optional: optional.is_some(), cost }),
            |x| match x {
                Sentence::AdditionalCost { optional, cost } => Some((optional.then_some(()), cost.clone())),
                _ => None,
            },
        );
    let statics = crate::statics::sentence(&body).map(
        |s| Some(Sentence::Static(Box::new(s))),
        |x| match x {
            Sentence::Static(s) => Some((**s).clone()),
            _ => None,
        },
    );
    // a lead governs something that happens or holds; the sentences about the ability itself
    // (restrictions, riders, additional costs) take none
    any_sentence.define(alt(
        "sentence after a lead",
        vec![plain.clone(), simultaneous.clone(), same_way.clone(), led.clone(), choose_modes.clone(), statics.clone()],
    ));
    // "X is the number of creatures you control."
    let x_is = lit("X is").then_keep(&amounts().amount).map(
        |a| Some(Sentence::XIs(a)),
        |s| match s {
            Sentence::XIs(a) => Some(a.clone()),
            _ => None,
        },
    );
    let round_each = lit("round").then_keep(&alt("rounding", vec![lit("up").to(true), lit("down").to(false)]))
        .skip(&lit("each time")).map(
            |up| Some(Sentence::RoundEach { up }),
            |s| match s { Sentence::RoundEach { up } => Some(*up), _ => None },
        );
    let sentence = alt(
        "sentence",
        vec![plain, simultaneous, same_way, led, restrictions, cast_restrictions, same_is_true, rider, choose_modes, additional, statics, x_is, round_each],
    )
    .sentence();
    Effects { sentence, body }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::Outcome;
    use oracle::token::tokenize;

    /// Slots are typed: an action takes only what the rules let it be done to, and only who can
    /// perform it does.
    #[test]
    fn slots_are_typed() {
        let s = &effects().sentence;
        for text in [
            "Destroy target player",
            "Destroy each opponent",
            "Sacrifice target opponent",
            "Counter target creature",
            "Tap target spell",
            "Untap target player's hand",
            "Discard target creature",
            "~ deals 3 damage to target spell",
            "Target creature draws a card",
            "Target player gets +2/+2 until end of turn",
            "Deal 3 damage to any target",
        ] {
            assert!(matches!(s.parse_all(&tokenize(text)), Outcome::Declined { .. }), "{text} must not read");
        }
        for text in [
            "Destroy target creature",
            "Counter target spell",
            "Exile target spell",
            "Target player draws a card",
            "Target player sacrifices a creature",
            "~ deals 3 damage to target player",
            "Discard ~",
            "They gain trample until end of turn",
        ] {
            assert!(matches!(s.parse_all(&tokenize(text)), Outcome::Accepted { .. }), "{text} must read");
        }
    }

    #[test]
    fn sentences_round_trip() {
        let s = &effects().sentence;
        for text in [
            "Draw a card",
            "Target player draws two cards",
            "You may draw a card",
            "Each opponent loses 1 life",
            "~ deals 3 damage to any target",
            "Destroy target creature",
            "Draw two cards, then discard a card",
            "It explores, then it explores again",
            "You draw a card and you lose 1 life",
            "You gain 3 life and draw a card",
            "Create a 1/1 white Spirit creature token with flying",
            "Add {G}",
            "Add one mana of any color",
            "Draw a card for each creature you control",
            "Counter target spell unless its controller pays {2}",
            "Sacrifice it at the beginning of the next end step",
            "If you do, draw a card",
            "Then shuffle",
            "Activate only as a sorcery",
            "Target creature gets +2/+2 until end of turn",
            "Target creature gets +1/+1 and gains flying until end of turn",
            "Creatures you control get +1/+1 until end of turn",
            "Target creature can't block this turn",
            "Until end of turn, target creature gains haste",
            "Choose one —",
            "Choose one or both —",
            "When you do, ~ deals 2 damage to any target",
            "Search your library for a basic land card, put it onto the battlefield tapped, then shuffle",
            "Look at the top four cards of your library",
            "Regenerate ~",
            "~ deals damage equal to its power to any target",
            "You gain life equal to its toughness",
            "You may choose new targets for the copy",
            "They can't be regenerated",
            "Choose target creature",
            "Choose a color",
            "Choose a nonland card name",
            "Gain control of target creature until end of turn",
            "You get {E}{E}",
            "You get an experience counter",
            "Create a token that's a copy of target artifact",
            "You may have it deal 2 damage to target creature",
            "Put a number of +1/+1 counters equal to its power on it",
            "Target opponent reveals their hand",
            "Exile target player's graveyard",
            "Then if you control a Swamp, draw a card",
            "When that creature dies this turn, draw a card",
            "When that mana is spent to cast a creature spell that shares a creature type with your commander, scry 1",
            "As an additional cost to cast ~, sacrifice a creature",
            "As an additional cost to cast ~, reveal a Dinosaur card from your hand or pay {1}",
            "Put a +1/+1 counter on each creature you control",
            "Return target creature to its owner's hand",
            "Return target creature card from your graveyard to your hand",
            "Return two target creatures to their owners' hands",
            "Put target creature on top of its owner's library",
            "Return it to the battlefield tapped under your control",
            "Put that card onto the battlefield",
            "Put the rest on the bottom of your library in a random order",
            "Return ~ to the battlefield with a finality counter on it",
            "Put it into its owner's library third from the top",
            "Return it to the battlefield under its owner's control at the beginning of the next end step",
        ] {
            let toks = tokenize(text);
            match s.parse_all(&toks) {
                Outcome::Accepted { value, .. } => {
                    assert_eq!(s.print_text(&value, true).as_deref(), Some(text), "{value:#?}")
                }
                other => panic!("{text}: {other:#?}"),
            }
        }
    }

    /// CR 107.1: a number of things may be arithmetic on a number that varies; "that many" takes
    /// its arithmetic after the noun; arithmetic on literals alone is not read.
    #[test]
    fn r107_1_arithmetic_in_the_quantity() {
        let s = &effects().sentence;
        for text in [
            "Target player mills twice X cards",
            "Target player gains twice X life",
            "~ deals twice X damage to target creature",
            "You gain X plus 3 life",
            "~ deals X plus 1 damage to any target",
            "~ deals 5 minus X damage to target creature",
            "Each opponent loses three times X life",
            "Draw that many cards plus one",
            "Draw that many cards minus one",
            "Reveal the top X plus one cards of your library",
            "Put twice X stun counters on it",
            "Pay life equal to its mana value",
        ] {
            let toks = tokenize(text);
            match s.parse_all(&toks) {
                Outcome::Accepted { value, .. } => {
                    assert_eq!(s.print_text(&value, true).as_deref(), Some(text), "{value:#?}")
                }
                other => panic!("{text}: {other:#?}"),
            }
        }
        for text in ["You gain two plus one life", "Draw twice two cards", "Draw that many plus one cards"] {
            assert!(matches!(s.parse_all(&tokenize(text)), Outcome::Declined { .. }), "{text} must not read");
        }
    }

    /// CR 107.1, 608.2h: how many things may be given by an amount after the noun, wherever the
    /// action takes things of that kind; the action still decides what it can be done to.
    #[test]
    fn r608_2h_quantity_after_the_noun() {
        let s = &effects().sentence;
        for text in [
            "Exile a number of cards from the top of your library equal to its power",
            "Target player discards a number of cards equal to the sacrificed creature's power",
            "Target player reveals a number of cards from their hand equal to the number of Swamps you control",
            "Put a number of +1/+1 counters equal to its power on up to one target creature",
            "Pay life equal to its mana value",
        ] {
            let toks = tokenize(text);
            match s.parse_all(&toks) {
                Outcome::Accepted { value, .. } => {
                    assert_eq!(s.print_text(&value, true).as_deref(), Some(text), "{value:#?}")
                }
                other => panic!("{text}: {other:#?}"),
            }
        }
        // the amount after the whole phrase is a second spelling of the same count
        let toks = tokenize("Put a number of +1/+1 counters on it equal to its power");
        let Outcome::Accepted { value, .. } = s.parse_all(&toks) else { panic!("alias must read") };
        assert_eq!(
            s.print_text(&value, true).as_deref(),
            Some("Put a number of +1/+1 counters equal to its power on it")
        );
        for text in [
            "Destroy a number of players equal to its power",
            "Target creature draws a number of cards equal to its power",
            "Exile a number of cards equal to two",
        ] {
            assert!(!matches!(s.parse_all(&tokenize(text)), Outcome::Accepted { .. }), "{text} must not read");
        }
    }

    /// CR 701.10e: doubling counters of one kind, or of each kind the permanent has.
    #[test]
    fn r701_10e_double_each_kind_of_counter() {
        let s = &effects().sentence;
        for text in [
            "Double the number of +1/+1 counters on target creature",
            "Double the number of each kind of counter on target creature",
            "Double the number of each kind of counter on any number of target permanents",
        ] {
            let toks = tokenize(text);
            match s.parse_all(&toks) {
                Outcome::Accepted { value, .. } => {
                    assert_eq!(s.print_text(&value, true).as_deref(), Some(text), "{value:#?}")
                }
                other => panic!("{text}: {other:#?}"),
            }
        }
        // a count is not doubled, and "the total number of" is the same count as "the number of"
        assert!(!matches!(s.parse_all(&tokenize("Double the number of each kind of card on target creature")), Outcome::Accepted { .. }));
        let toks = tokenize("You gain life equal to the total number of cards in your hand");
        let Outcome::Accepted { value, .. } = s.parse_all(&toks) else { panic!("alias must read") };
        assert_eq!(
            s.print_text(&value, true).as_deref(),
            Some("You gain life equal to the number of cards in your hand")
        );
    }

    /// CR 122.5: moving counters takes a count (one, all, any number) and an optional kind, as
    /// removing them does.
    #[test]
    fn r122_5_move_counters_count_and_kind() {
        let s = &effects().sentence;
        for text in [
            "Move a +1/+1 counter from target creature onto another target creature",
            "Move a counter from target creature onto another target creature",
            "Move all counters from target creature onto another target creature",
            "Move any number of +1/+1 counters from ~ onto target creature",
            "Move two counters from ~ onto target creature",
        ] {
            let toks = tokenize(text);
            match s.parse_all(&toks) {
                Outcome::Accepted { value, .. } => {
                    assert_eq!(s.print_text(&value, true).as_deref(), Some(text), "{value:#?}")
                }
                other => panic!("{text}: {other:#?}"),
            }
        }
        for text in ["Move all counters", "Move counters from target creature onto another target creature"] {
            assert!(!matches!(s.parse_all(&tokenize(text)), Outcome::Accepted { .. }), "{text} must not read");
        }
    }

    /// CR 810.10b, 728.1: a player loses counters (their own are removed); energy is paid or
    /// gotten as an amount.
    #[test]
    fn r122_1_player_counters_and_energy() {
        let s = &effects().sentence;
        for text in [
            "Target player loses all rad counters",
            "Each opponent loses all counters",
            "Target player loses two poison counters",
            "Pay an amount of {E} equal to its mana value",
            "You get an amount of {E} equal to its mana value",
            "You may pay one or more {E}",
            "Pay any amount of {E}",
            "Each opponent gets a number of rad counters equal to its power",
        ] {
            let toks = tokenize(text);
            match s.parse_all(&toks) {
                Outcome::Accepted { value, .. } => {
                    assert_eq!(s.print_text(&value, true).as_deref(), Some(text), "{value:#?}")
                }
                other => panic!("{text}: {other:#?}"),
            }
        }
        for text in [
            "Target player loses all rad counters from ~",
            "Pay an amount of {E} equal to three",
            "Pay one or more life",
            "Each opponent gets a number of rad counters equal to X",
        ] {
            assert!(!matches!(s.parse_all(&tokenize(text)), Outcome::Accepted { .. }), "{text} must not read");
        }
    }

    /// CR 608.2d, 607.2f, 607.2d, 701.9b: what a "choose" picks. A class (with exclusions or a
    /// count), a number, an explicit list of words, colors or abilities, several of these.
    #[test]
    fn r608_2d_choices() {
        let s = &effects().sentence;
        for text in [
            "Choose a color other than red",
            "Choose a creature type other than Wall",
            "Choose a card type other than creature or land",
            "Choose a card name other than a basic land card name",
            "Choose two basic land types",
            "Choose a creature type at random",
            "Choose a number",
            "Choose a number between 0 and 13",
            "Choose Khans or Dragons",
            "Choose odd or even",
            "Choose money, friends, or secrets",
            "Choose black or red",
            "Choose flying or indestructible at random",
            "Choose a color and an opponent",
            "Choose a color and a creature type",
            "Choose one or more instead",
            // 700.2: how many modes, and by chance
            "Choose up to one —",
            "Choose up to two —",
            "Choose one at random —",
        ] {
            match s.parse_all(&tokenize(text)) {
                Outcome::Accepted { value, .. } => {
                    assert_eq!(s.print_text(&value, true).as_deref(), Some(text), "{value:#?}")
                }
                other => panic!("{text}: {other:#?}"),
            }
        }
        // "any card name" is "a card name"
        let Outcome::Accepted { value, .. } = s.parse_all(&tokenize("Choose any card name")) else { panic!("alias must read") };
        assert_eq!(s.print_text(&value, true).as_deref(), Some("Choose a card name"));
        for text in [
            // an exclusion is of the class chosen from
            "Choose a color other than Wall",
            "Choose a creature type other than red",
            // the bounds are ordered, and a list has two members
            "Choose a number between 9 and 3",
            "Choose odd",
            // a count of one is "a", and a class is not a list of members
            "Choose one color",
            "Choose a color other than red or black or",
            // a keyword or a color is not a word without a rules meaning (and "a color" has no members)
            "Choose white or flying",
            "Choose up to —",
            "Choose at random —",
        ] {
            assert!(!matches!(s.parse_all(&tokenize(text)), Outcome::Accepted { .. }), "{text} must not read");
        }
    }

    /// CR 121.2b: an effect may offer a choice of how many cards to draw, at most a number; the
    /// bound is a number of cards, and only a player draws.
    #[test]
    fn r121_2b_draw_up_to_a_number_of_cards() {
        let s = &effects().sentence;
        for text in [
            "Draw up to three cards",
            "Each player draws up to two cards",
            "Target player draws up to X cards",
            "Each other player may draw up to three cards",
            "Draw up to one card",
        ] {
            match s.parse_all(&tokenize(text)) {
                Outcome::Accepted { value, .. } => {
                    assert_eq!(s.print_text(&value, true).as_deref(), Some(text), "{value:#?}")
                }
                other => panic!("{text}: {other:#?}"),
            }
        }
        for text in [
            "Target creature draws up to two cards",
            "Draw up to a card",
            "Draw up to one cards",
            "Draw up to three card",
            "Draw up to three creatures",
            "Draw up to cards",
        ] {
            assert!(!matches!(s.parse_all(&tokenize(text)), Outcome::Accepted { .. }), "{text} must not read");
        }
    }

    /// CR 118.12a: "unless" offers the player what they may do instead, one act or alternatives
    /// joined by "or", whatever the player; an "or" after it never joins the act it modifies.
    #[test]
    fn r118_12a_unless_offers_one_act_or_alternatives() {
        let s = &effects().sentence;
        for text in [
            "Each opponent loses 3 life unless they discard a card or sacrifice a creature",
            "Target opponent loses 3 life unless they sacrifice a nonland permanent of their choice or discard a card",
            "Its controller loses 3 life unless that player sacrifices a creature or discards a card",
            "You lose 3 life unless you discard a card or sacrifice a creature",
            "Draw a card unless target opponent sacrifices a creature of their choice or pays 3 life",
            "Each opponent loses 3 life unless they discard a card",
        ] {
            match s.parse_all(&tokenize(text)) {
                Outcome::Accepted { value, .. } => {
                    assert_eq!(s.print_text(&value, true).as_deref(), Some(text), "{value:#?}");
                    let Sentence::Do(b) = &value else { panic!("{text}: {value:#?}") };
                    assert!(b.first.rest.is_empty(), "{text}: the alternatives are the offer's, {value:#?}");
                }
                other => panic!("{text}: {other:#?}"),
            }
        }
        for text in [
            // alternatives are joined by "or"; who is offered must be able to do each one
            "Each opponent loses 3 life unless they discard a card and sacrifice a creature",
            "Each opponent loses 3 life unless they discard a card or destroy target player",
            "Each opponent loses 3 life unless they discard a card or sacrifices a creature",
        ] {
            assert!(!matches!(s.parse_all(&tokenize(text)), Outcome::Accepted { .. }), "{text} must not read");
        }
    }

    /// CR 118.12a, 118.1: payments offered as alternatives say "pay" once ("pays {B} or {3}" is
    /// "pays {B} or pays {3}"); only what is paid (mana, life, energy) is elided after the verb.
    #[test]
    fn r118_12a_alternative_payments_say_the_verb_once() {
        let s = &effects().sentence;
        for text in [
            "Counter target spell unless its controller pays {B} or {3}",
            "Destroy that land unless that player pays {1} or 1 life",
            "~ deals 1 damage to that player unless they pay {B} or {3}",
        ] {
            match s.parse_all(&tokenize(text)) {
                Outcome::Accepted { value, .. } => {
                    assert_eq!(s.print_text(&value, true).as_deref(), Some(text), "{value:#?}")
                }
                other => panic!("{text}: {other:#?}"),
            }
        }
        // the verb said twice is the same offer
        let Outcome::Accepted { value, .. } = s.parse_all(&tokenize("Counter target spell unless its controller pays {B} or pays {3}")) else {
            panic!("alias must read")
        };
        assert_eq!(s.print_text(&value, true).as_deref(), Some("Counter target spell unless its controller pays {B} or {3}"));
        for text in [
            "Counter target spell unless its controller pays {B} or a creature",
            "Counter target spell unless its controller pays {B} or 3 damage",
            "Counter target spell unless its controller pays {B} and {3}",
        ] {
            assert!(!matches!(s.parse_all(&tokenize(text)), Outcome::Accepted { .. }), "{text} must not read");
        }
    }

    /// CR 701.7, 608.2d: what is created is one token description, several joined by "and", or
    /// alternatives joined by "or" that the creator chooses among; only tokens are created.
    #[test]
    fn r608_2d_create_one_of_several_tokens() {
        let s = &effects().sentence;
        for text in [
            "Create a Food token or a Treasure token",
            "Create a 1/1 white Soldier creature token or a Treasure token",
            "Create a Food token and a Treasure token",
            "Create a Treasure token",
        ] {
            match s.parse_all(&tokenize(text)) {
                Outcome::Accepted { value, .. } => {
                    assert_eq!(s.print_text(&value, true).as_deref(), Some(text), "{value:#?}")
                }
                other => panic!("{text}: {other:#?}"),
            }
        }
        for text in ["Create a Food token or target creature", "Create a Food token and/or a Treasure token", "Create a Food token or"] {
            assert!(!matches!(s.parse_all(&tokenize(text)), Outcome::Accepted { .. }), "{text} must not read");
        }
    }

    /// CR 701.40a, 701.58a: a card is manifested or cloaked from wherever it is, so the card may
    /// be said with its zone after "from", as for exile; what is turned face down is a card.
    #[test]
    fn r701_40a_manifest_and_cloak_a_card_from_a_zone() {
        let s = &effects().sentence;
        for text in [
            "Manifest a card from your hand",
            "Cloak a card from your hand",
            "Manifest the top card of your library",
            "Exile a card from your hand",
        ] {
            match s.parse_all(&tokenize(text)) {
                Outcome::Accepted { value, .. } => {
                    assert_eq!(s.print_text(&value, true).as_deref(), Some(text), "{value:#?}")
                }
                other => panic!("{text}: {other:#?}"),
            }
        }
        for text in ["Manifest target creature", "Manifest a creature from your hand", "Cloak target player", "Manifest a card from"] {
            assert!(!matches!(s.parse_all(&tokenize(text)), Outcome::Accepted { .. }), "{text} must not read");
        }
    }

    /// CR 614.1c, 614.12a: "as ~ enters" after an act says the act is done as the permanent
    /// enters; it governs that act, while in front of a sentence it governs the whole sentence.
    #[test]
    fn r614_1c_an_act_done_as_this_permanent_enters() {
        let s = &effects().sentence;
        let text = "If it's neither day nor night, it becomes day as ~ enters";
        let Outcome::Accepted { value, .. } = s.parse_all(&tokenize(text)) else { panic!("{text} must read") };
        assert_eq!(s.print_text(&value, true).as_deref(), Some(text));
        let Sentence::Led { lead: Lead::Modifier(Modifier::If(_)), rest } = &value else { panic!("{value:#?}") };
        let Sentence::Do(body) = rest.as_ref() else { panic!("{rest:#?}") };
        assert!(matches!(body.first.first.mods.as_slice(), [Modifier::AsEnters(_)]), "{body:#?}");
        // the lead stays the lead: one reading
        let Outcome::Accepted { value, .. } = s.parse_all(&tokenize("As ~ enters, it becomes day")) else { panic!("lead must read") };
        assert!(matches!(value, Sentence::Led { lead: Lead::AsEnters(_), .. }), "{value:#?}");
        // a player does not enter the battlefield (110.1)
        for text in [
            "It becomes day as target player enters",
            "It becomes day as enters",
            "It becomes day as the top card of your library enters",
            "It becomes day as the next creature this turn enters",
        ] {
            assert!(!matches!(s.parse_all(&tokenize(text)), Outcome::Accepted { .. }), "{text} must not read");
        }
    }

    /// CR 701.55a: a player faces a villainous choice between two options, each instructions of
    /// its own; the choice runs to the end of its sentence.
    #[test]
    fn r701_55a_a_villainous_choice() {
        let s = &effects().sentence;
        for text in [
            "Target opponent faces a villainous choice — You draw a card, or that player discards a card",
            "Then each opponent faces a villainous choice — Destroy all Dalek creatures and each of your opponents loses 2 life, or destroy all non-Dalek creatures",
        ] {
            match s.parse_all(&tokenize(text)) {
                Outcome::Accepted { value, .. } => assert_eq!(s.print_text(&value, true).as_deref(), Some(text), "{value:#?}"),
                other => panic!("{text}: {other:#?}"),
            }
        }
        for text in [
            // a player faces it, with two options
            "Target creature faces a villainous choice — You draw a card, or you lose 1 life",
            "Target opponent faces a villainous choice — You draw a card",
            "Target opponent faces a villainous choice — You draw a card, or you lose 1 life, or you gain 1 life",
            // it is not a payment (701.55b)
            "Counter target spell unless its controller faces a villainous choice — You draw a card, or you lose 1 life",
            "As an additional cost to cast ~, face a villainous choice — You draw a card, or you lose 1 life",
        ] {
            assert!(!matches!(s.parse_all(&tokenize(text)), Outcome::Accepted { .. }), "{text} must not read");
        }
    }

    /// CR 700.2e: a player other than the controller may choose the modes; one player, and not
    /// by chance.
    #[test]
    fn r700_2e_another_player_chooses_the_mode() {
        let s = &effects().sentence;
        for text in ["An opponent chooses one —", "Target opponent chooses one or both —"] {
            match s.parse_all(&tokenize(text)) {
                Outcome::Accepted { value: v @ Sentence::ChooseModes { chooser: Some(_), .. }, .. } => {
                    assert_eq!(s.print_text(&v, true).as_deref(), Some(text), "{v:#?}")
                }
                other => panic!("{text}: {other:#?}"),
            }
        }
        // "Target opponent chooses one" without the dash chooses one of the objects named before
        assert!(matches!(s.parse_all(&tokenize("Target opponent chooses one")), Outcome::Accepted { value: Sentence::Do(_), .. }));
        for text in ["You choose one —", "An opponent chooses one at random —", "Target creature chooses one —", "Each opponent chooses one —", "~'s controller chooses one —", "Defending player chooses one —"] {
            assert!(!matches!(s.parse_all(&tokenize(text)), Outcome::Accepted { .. }), "{text} must not read");
        }
    }

    /// CR 119.5: an effect sets a life total to a number or to an expression; before an
    /// expression "equal to" says the same, before a number it is not said.
    #[test]
    fn r119_5_a_life_total_becomes_an_amount() {
        let s = &effects().sentence;
        for (text, printed) in [
            ("Your life total becomes 10", "Your life total becomes 10"),
            ("Your life total becomes your starting life total", "Your life total becomes your starting life total"),
            ("Your life total becomes equal to your starting life total", "Your life total becomes your starting life total"),
            // an optional setting: the controller chooses (the causative says it)
            (
                "You may have your life total become the number of charge counters on ~",
                "You may have your life total become the number of charge counters on ~",
            ),
        ] {
            match s.parse_all(&tokenize(text)) {
                Outcome::Accepted { value, .. } => assert_eq!(s.print_text(&value, true).as_deref(), Some(printed), "{value:#?}"),
                other => panic!("{text}: {other:#?}"),
            }
        }
        for text in [
            "Your life total becomes equal to 1",
            "Your life total becomes equal to X",
            "Your life total becomes equal to {2}",
            // the life total is not a doer: the causative is the optional setting only
            "You have your life total become 1",
            "You may have your life total becomes 1",
            "Your life total may become 1",
        ] {
            assert!(!matches!(s.parse_all(&tokenize(text)), Outcome::Accepted { .. }), "{text} must not read");
        }
    }

    /// CR 701.12g: a life total is exchanged with a permanent's power or toughness; a player has
    /// no power, a creature no life total.
    #[test]
    fn r701_12g_exchange_a_life_total_with_a_stat() {
        let s = &effects().sentence;
        for text in [
            "Exchange your life total with ~'s toughness",
            "Exchange your life total with ~'s power",
            "Exchange target opponent's life total with ~'s toughness",
        ] {
            match s.parse_all(&tokenize(text)) {
                Outcome::Accepted { value, .. } => assert_eq!(s.print_text(&value, true).as_deref(), Some(text), "{value:#?}"),
                other => panic!("{text}: {other:#?}"),
            }
        }
        for text in [
            "Exchange your life total with target player's power",
            "Exchange target creature's life total with ~'s power",
            "Exchange your life total with ~'s mana value",
        ] {
            assert!(!matches!(s.parse_all(&tokenize(text)), Outcome::Accepted { .. }), "{text} must not read");
        }
    }

    /// CR 614.1a, 120.3a: a replacement of the life loss damage causes keeps a life total at a
    /// floor; it stands alone, under a condition, or for a duration. The floor is one number.
    #[test]
    fn r614_1a_damage_cant_reduce_a_life_total_below_a_floor() {
        let s = &effects().sentence;
        for text in [
            "Damage that would reduce your life total to less than 1 reduces it to 1 instead",
            "If you control a creature, damage that would reduce your life total to less than 1 reduces it to 1 instead",
            "As long as you have 7 or more life, damage that would reduce your life total to less than 7 reduces it to 7 instead",
            "Until end of turn, damage that would reduce your life total to less than 1 reduces it to 1 instead",
        ] {
            match s.parse_all(&tokenize(text)) {
                Outcome::Accepted { value, .. } => assert_eq!(s.print_text(&value, true).as_deref(), Some(text), "{value:#?}"),
                other => panic!("{text}: {other:#?}"),
            }
        }
        for text in [
            "Damage that would reduce your life total to less than 2 reduces it to 1 instead",
            "Damage that would reduce target creature's life total to less than 1 reduces it to 1 instead",
            "Damage that would reduce your life total to less than 1 reduces it to 1",
        ] {
            assert!(!matches!(s.parse_all(&tokenize(text)), Outcome::Accepted { .. }), "{text} must not read");
        }
    }

    /// CR 500.8: phases are added directly after the one named; nobody adds them, so the clause
    /// has no subject, and it joins another clause as any clause does.
    #[test]
    fn r500_8_additional_phases() {
        let s = &effects().sentence;
        for text in [
            "After this phase, there is an additional combat phase",
            "After this main phase, there is an additional combat phase followed by an additional main phase",
            "After this main phase, there are two additional combat phases",
            "After this combat phase, there is an additional combat phase",
            "After this phase, there is an additional beginning phase",
            "If it's your main phase, after this phase, there is an additional combat phase",
            "If you do, untap all attacking creatures and after this phase, there is an additional combat phase",
            "Untap all creatures you control, and after this phase, there is an additional combat phase",
        ] {
            match s.parse_all(&tokenize(text)) {
                Outcome::Accepted { value, .. } => assert_eq!(s.print_text(&value, true).as_deref(), Some(text), "{value:#?}"),
                other => panic!("{text}: {other:#?}"),
            }
        }
        // the time phrase last is the same value, printed with it first
        for (variant, canonical) in [
            ("If it's your main phase, there is an additional combat phase after this phase", "If it's your main phase, after this phase, there is an additional combat phase"),
            ("If it's your main phase, there's an additional combat phase after this phase", "If it's your main phase, after this phase, there is an additional combat phase"),
            (
                "If it's your main phase, there is an additional combat phase after this phase, followed by an additional main phase",
                "If it's your main phase, after this phase, there is an additional combat phase followed by an additional main phase",
            ),
            (
                "If it's your main phase, there is an additional combat phase after this phase followed by an additional main phase",
                "If it's your main phase, after this phase, there is an additional combat phase followed by an additional main phase",
            ),
            ("If you do, there is an additional beginning phase after this phase", "If you do, after this phase, there is an additional beginning phase"),
            ("Untap it and there is an additional combat phase after this phase", "Untap it and after this phase, there is an additional combat phase"),
        ] {
            match s.parse_all(&tokenize(variant)) {
                Outcome::Accepted { value, .. } => assert_eq!(s.print_text(&value, true).as_deref(), Some(canonical), "{value:#?}"),
                other => panic!("{variant}: {other:#?}"),
            }
        }
        for text in [
            "There is an additional upkeep phase after this phase",
            "There is an additional combat phase after this phase, followed by",
            "After this phase, there is an additional upkeep phase",
            "After this phase there are 1 additional combat phases",
            "After this phase, there are one additional combat phases",
            "After this phase, there is two additional combat phases",
            "After this phase, target player is an additional combat phase",
        ] {
            assert!(!matches!(s.parse_all(&tokenize(text)), Outcome::Accepted { .. }), "{text} must not read");
        }
    }

    /// CR 705.1, 500.7: a keyword action done to "a coin" or "an extra turn", done N times, names
    /// N of them; that is the action with "N times", which prints so.
    #[test]
    fn r705_1_r500_7_a_counted_object_is_the_action_repeated() {
        let s = &effects().sentence;
        for text in [
            "Flip five coins",
            "Flip X coins",
            "Target player takes two extra turns after this one",
            "Take three extra turns after this one",
            "Each player flips two coins",
        ] {
            match s.parse_all(&tokenize(text)) {
                Outcome::Accepted { value, .. } => {
                    let Sentence::Do(b) = &value else { panic!("{value:#?}") };
                    assert!(matches!(b.first.first.action, ir::Action::Keyword(_)) && matches!(b.first.first.mods[0], Modifier::Times(_)), "{value:#?}");
                    assert_eq!(s.print_text(&value, true).as_deref(), Some(text), "{value:#?}")
                }
                other => panic!("{text}: {other:#?}"),
            }
        }
        // "twice" is the same value, printed with the count on the object
        for (variant, canonical) in [
            ("Take an extra turn after this one twice", "Take two extra turns after this one"),
            ("Flip a coin three times", "Flip three coins"),
        ] {
            match s.parse_all(&tokenize(variant)) {
                Outcome::Accepted { value, .. } => assert_eq!(s.print_text(&value, true).as_deref(), Some(canonical), "{value:#?}"),
                other => panic!("{variant}: {other:#?}"),
            }
        }
        for text in [
            "Take two extra turn after this one",
            "Take one extra turns after this one",
            "Flip one coins",
            "Flip two coin",
            "Target player take two extra turns after this one",
            "Investigate two clues",
        ] {
            assert!(!matches!(s.parse_all(&tokenize(text)), Outcome::Accepted { .. }), "{text} must not read");
        }
    }

    /// CR 107.1 counts quantities; how many times something happens is a plain number.
    #[test]
    fn occurrences_take_a_plain_number() {
        let s = &effects().sentence;
        for text in ["Activate no more than three times each turn", "Activate no more than twice each turn"] {
            assert!(matches!(s.parse_all(&tokenize(text)), Outcome::Accepted { .. }), "{text} must read");
        }
        for text in ["Activate no more than four minus X times each turn", "Activate no more than three times X times each turn"] {
            assert!(!matches!(s.parse_all(&tokenize(text)), Outcome::Accepted { .. }), "{text} must not read");
        }
    }
    #[test]
    fn free_card_casts_and_spell_class_permissions_have_one_reading() {
        let grammar = &effects().sentence;
        fn act(sentence: &Sentence) -> &Act {
            match sentence { Sentence::Led {rest,..} => act(rest), Sentence::Do(body) => &body.first.first,
                _ => panic!("an instruction") }
        }
        for text in [
            "Until end of turn, you may cast cards exiled this way without paying their mana costs",
            "Until your next turn, you may cast cards exiled this way without paying their mana costs",
            "You may cast cards exiled this way without paying their mana costs this turn",
            "If you control a creature, you may cast cards exiled this way without paying their mana costs",
        ] {
            let Outcome::Accepted {value,..} = grammar.parse_all(&tokenize(text)) else {panic!("{text} must have one reading")};
            assert!(matches!(act(&value).action, ir::Action::Cast {free:true,..}), "{value:?}");
            let printed = grammar.print_text(&value,true).expect("a canonical spelling");
            let Outcome::Accepted {value: again,..} = grammar.parse_all(&tokenize(&printed)) else {panic!("{printed} must have one reading")};
            assert_eq!(again,value);
        }
        // A class of spells, rather than card selectors, keeps its permission form.
        let text = "Until end of turn, you may cast noncreature spells from among those cards without paying their mana costs";
        let Outcome::Accepted {value,..} = grammar.parse_all(&tokenize(text)) else {panic!("{text}")};
        assert!(matches!(&act(&value).action, ir::Action::Static(rule) if matches!(rule.as_ref(),ir::StaticAct::PlayFrom {..})));
        let printed = grammar.print_text(&value,true).unwrap();
        let Outcome::Accepted {value: again,..} = grammar.parse_all(&tokenize(&printed)) else {panic!("{printed}")};
        assert_eq!(again,value);
    }

    #[test]
    fn next_turn_player_control_is_canonical_without_losing_other_control_windows() {
        let grammar = &effects().sentence;
        fn act(sentence: &Sentence) -> &Act {
            match sentence { Sentence::Led {rest,..} => act(rest), Sentence::Do(body) => &body.first.first,
                _ => panic!("an instruction") }
        }
        for text in [
            "You control that player during their next turn",
            "You control that player during that player's next turn",
            // Oracle normalization represents the card's self-reference with ~.
            "If ~'s additional cost was paid, you control that player during their next turn instead",
            "If you control a creature, you control that opponent during their next turn instead",
        ] {
            let Outcome::Accepted {value,..} = grammar.parse_all(&tokenize(text)) else {panic!("{text} must have one reading")};
            assert!(matches!(act(&value).action,ir::Action::ControlPlayerNextTurn(_)), "{value:?}");
            let printed = grammar.print_text(&value,true).unwrap();
            assert!(printed.contains("during that player's next turn"));
            let Outcome::Accepted {value: again,..} = grammar.parse_all(&tokenize(&printed)) else {panic!("{printed}")};
            assert_eq!(again,value);
        }
        for text in ["You control that player during combat", "You control that player during your next turn"] {
            let Outcome::Accepted {value,..} = grammar.parse_all(&tokenize(text)) else {panic!("{text}")};
            assert!(matches!(&act(&value).action,ir::Action::Static(rule) if matches!(rule.as_ref(),ir::StaticAct::Controls(_))));
            let printed = grammar.print_text(&value,true).unwrap();
            let Outcome::Accepted {value: again,..} = grammar.parse_all(&tokenize(&printed)) else {panic!("{printed}")};
            assert_eq!(again,value);
        }
        // Next-combat ownership is a separate phrasing gap, already declined before
        // this ambiguity repair; do not silently reinterpret it as a whole next turn.
        assert!(matches!(grammar.parse_all(&tokenize("You control that player during their next combat phase")),Outcome::Declined {..}));
    }

    #[test]
    fn r723_next_turn_control_and_anchored_extra_turns_have_distinct_temporal_scope() {
        let grammar=&effects().sentence;
        for text in [
            "You gain control of target opponent during that player's next turn",
            "After that turn, that player takes an extra turn",
            "Target player takes an extra turn after this one",
            "After this one, each player takes two extra turns",
        ] {
            let Outcome::Accepted{value,..}=grammar.parse_all(&tokenize(text)) else {panic!("{text}")};
            assert_eq!(grammar.print_text(&value,true).as_deref(),Some(text));
        }
        for text in ["After that turns, you take an extra turn", "You gain control of target opponent during that player's next turns", "Target opponent takes an extra turns after this one"] {
            assert!(!matches!(grammar.parse_all(&tokenize(text)),Outcome::Accepted{..}),"{text}");
        }
    }

}
#[test]
fn created_turn_activation_family_has_one_rule_reading() {
    use crate::kernel::Outcome;
    for text in [
        "Power-up abilities can't be activated.",
        "During that turn, power-up abilities can't be activated.",
        "Take an extra turn after this one. During that turn, power-up abilities can't be activated.",
    ] {
        let grammar = crate::ability::effect();
        let Outcome::Accepted { value, .. } = grammar.parse_all(&oracle::token::tokenize(text)) else { panic!("{text}") };
        assert_eq!(grammar.print_text(&value, true).as_deref(), Some(text));
    }
}
