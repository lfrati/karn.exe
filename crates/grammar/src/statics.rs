//! Static abilities, replacement and prevention effects, rule changes.
//!
//! ```text
//! static      := "as long as" condition "," body | body "as long as" condition
//!              | "during" possessive "turn" "," body | body "during" possessive "turn"
//!              | body "unless" condition | "as" selector "enters," body
//!              | ("if" | "the next time") would ["while" condition] ["this turn"] ","
//!                ["instead"] substitute
//!              | possessor "power and toughness are each equal to" amount | …
//!              | ["if" condition ","] "you may" actions "rather than pay ~'s mana cost"
//!              | damage "is dealt to" party "instead"
//!              | damage "can't be prevented" | "X can't be" … | "the" keyword "cost is equal to" …
//!              | [body join] ("its activated abilities" | kind "abilities of" party) rule
//!              | possessive "maximum hand size is" … | "all creatures able to block" party "do so"
//! would       := subjects "would" event-action(base) | selectors "would be put into" graveyard
//!              | "damage would be dealt to" party | … "would be destroyed" | tokens/counters
//! static-act  := "has"/"have" granted | "loses"/"lose" lost | "isn't"/"aren't" becoming
//!              | "enters"/"enter" arrival+ | "costs"/"cost" mana ("less"|"more") "to" ("cast"|"activate")
//!              | required [window] "if able" | "can block only" set | "can block" additional
//!              | "can't" forbidden ("or" forbidden)* | "can attack" [window] "as though …"
//!              | "assigns"/"assign" … | "has"/"have" "no maximum hand size" | "skips"/"skip" …
//!              | "plays"/"play" playable ["and" playable] "from" zone | …
//! damage      := ("the next" N | "all") damage-kind "that would be dealt" ["this turn"] [to] [by]
//! ```
//!
//! A static act is an action ([`Action::Static`]), so it composes with subjects, "and" chains
//! and modifiers like any other; a [`Static`] sentence wraps whole bodies.

use crate::amount::amounts;
use crate::condition::conditions;
use crate::event::{Form, verb};
use crate::kernel::{P, Val, alt, empty, leaf, lit};
use crate::mana::{mana_cost, spend_on, who};
use crate::selector::{counter_kind, selectors};
use crate::words::{Number, count_amount, stat_amount};
use ir::{
    AbilitiesOf, AbilityKind, AbilityRule, Action, Amount, DamageKind, Anaphor, AnyMana, Arrival, HandSize, Join, Playable, SpentMana, SkipStep, Skipped, Slice, ZoneSpec, Body, CostOf, Domain, Elsewhere, EventAction, EventPattern, Forbidden,
    Granted, Lose, Lost, ObjectSet, Party, Prevented, Prevention, Quant, Ref, Replacement, Selector,
    Static, StaticAct, Substitute, TurnAt, Who, Whose, Within, XLimit, Zone,
};
use ir::lexicon::Verb;
use ir::mana::SpendOn;
use ir::statics::{CounterKinds, CounterVoice};
use oracle::token::Kind;

/// One item, or two or more joined by `conj`.
fn one_or_list<T: Val>(p: &P<T>, conj: &str) -> P<Vec<T>> {
    alt("list", vec![p.map(|x| Some(vec![x]), |v: &Vec<T>| (v.len() == 1).then(|| v[0].clone())), p.list(conj)])
}

/// A rule for one [`StaticAct`] variant.
fn act<T: Val>(p: P<T>, mk: fn(T) -> StaticAct, un: fn(&StaticAct) -> Option<T>) -> P<StaticAct> {
    p.map(move |t| Some(mk(t)), un)
}

/// "each combat", "this turn", "this combat".
fn within() -> P<Within> {
    alt(
        "window",
        vec![
            lit("each combat").to(Within::EachCombat),
            lit("this turn").to(Within::ThisTurn),
            lit("this combat").to(Within::ThisCombat),
        ],
    )
}

/// The zone after "from" in a play permission: "the top of your library", "your graveyard".
fn from_zone() -> P<ZoneSpec> {
    let owner = &selectors().possessive;
    alt(
        "from zone",
        vec![
            lit("the top of").then_keep(owner).skip(&lit("library")).map(
                |o| Some(ZoneSpec { zones: ir::Coord::one(Zone::Library), owner: Some(Box::new(o)), slice: Some(Slice::Top(Amount::Const(1))) }),
                |z| match (z.zone(), &z.owner, &z.slice) {
                    (Some(Zone::Library), Some(o), Some(Slice::Top(Amount::Const(1)))) => Some((**o).clone()),
                    _ => None,
                },
            ),
            owner.then(&alt("zone", vec![lit("graveyard").to(Zone::Graveyard), lit("hand").to(Zone::Hand)])).map(
                |(o, zone)| Some(ZoneSpec::of(zone, Some(o))),
                |z| match (&z.owner, z.zone(), &z.slice) {
                    (Some(o), Some(zone @ (Zone::Graveyard | Zone::Hand)), None) => Some(((**o).clone(), zone)),
                    _ => None,
                },
            ),
        ],
    )
}

/// Does this take a plural verb or pronoun ("creature spells", "those cards")?
fn who_plural(w: &Who) -> bool {
    match w {
        Who::Set(_) => true,
        Who::One(s) => crate::selector::is_plural(s),
    }
}

/// "to cast that spell", "to activate those abilities".
fn purpose() -> P<SpendOn> {
    lit("to").then_keep(&spend_on())
}


/// Granted abilities: keywords and quoted abilities ([`crate::action::granted`]), or "all activated
/// abilities of …" taken from other objects.
pub(crate) fn granted_any() -> P<Vec<Granted>> {
    let kind = alt(
        "ability kind",
        vec![
            lit("activated abilities").to(AbilityKind::Activated),
            lit("triggered abilities").to(AbilityKind::Triggered),
            lit("activated and triggered abilities").to(AbilityKind::ActivatedAndTriggered),
            lit("loyalty abilities").to(AbilityKind::Loyalty),
        ],
    );
    // A bare plural donor phrase denotes every matching object, just as an explicit
    // universal selector does. Keep this at the ability-donor role, not all selectors.
    let donor = alt("ability donor", vec![selectors().selector.clone(), selectors().set_pl.map(
        |set| Some(ir::Selector::new(ir::Quant::All, set)),
        |s| (s.quant == ir::Quant::All && s.chooser.is_none() && s.random.is_none()).then(|| s.set.clone()),
    ).alias()]);
    let of = lit("all").then_keep(&kind).skip(&lit("of")).then(&donor).map(
        |(kind, from)| Some(vec![Granted::AbilitiesOf { kind, from: Box::new(from) }]),
        |g| match g.as_slice() {
            [Granted::AbilitiesOf { kind, from }] => Some((*kind, (**from).clone())),
            _ => None,
        },
    );
    alt("granted", vec![crate::action::granted(), of])
}

/// "tapped", "with two +1/+1 counters on it", "with an additional +1/+1 counter on it",
/// "prepared".
/// "If ~ is in your opening hand, you may begin the game with it on the battlefield"; "If ~ is
/// in your opening hand and you're not the starting player, you may begin the game with ~ on
/// the battlefield with a luck counter on it" (103.6a).
fn begin_on_battlefield() -> P<Static> {
    let not_starting = lit("and you're not the starting player").opt();
    let named = alt("begun", vec![lit("it").to(false), lit("~").to(true)]);
    let counters = lit("with").then_keep(&crate::selector::counters_on()).opt();
    lit("if ~ is in your opening hand").then_keep(&not_starting)
        .skip(&lit(", you may begin the game with")).then(&named)
        .skip(&lit("on the battlefield")).then(&counters)
        .map(
            |((not_starting, named), counters)| Some(Static::BeginOnBattlefield { not_starting: not_starting.is_some(), named, counters }),
            |s| match s {
                Static::BeginOnBattlefield { not_starting, named, counters } => Some(((not_starting.then_some(()), *named), counters.clone())),
                _ => None,
            },
        )
}

fn arrival() -> P<Arrival> {
    let additional = lit("additional").opt().map(|o| Some(o.is_some()), |b| Some(b.then_some(())));
    let on = || {
        use ir::On;
        lit("on it").to(On::It)
    };
    let counters = lit("with").then_keep(&crate::selector::counters_on()).map(
        |c| Some(Arrival::Counters(c)),
        |a| match a {
            Arrival::Counters(c) => Some(c.clone()),
            _ => None,
        },
    );
    // "with your choice of a reach counter or a vigilance counter on it"; "a +1/+1, first
    // strike, or vigilance counter" names one counter of each kind
    let kinds_short = lit("a")
        .then_keep(&crate::selector::counter_kind().list("or"))
        .skip(&lit("counter"))
        .map(
            |kinds| Some(kinds.into_iter().map(|k| ir::Counters::new(Amount::Const(1), k)).collect::<Vec<ir::Counters>>()),
            |_: &Vec<ir::Counters>| None,
        )
        .alias();
    let choice = lit("with your choice of")
        .then_keep(&alt("counter choices", vec![crate::selector::some_counters().list("or"), kinds_short]))
        .skip(&lit("on it"))
        .map(
            |choices| Some(Arrival::ChoiceOfCounters(choices)),
            |a| match a {
                Arrival::ChoiceOfCounters(choices) => Some(choices.clone()),
                _ => None,
            },
        );
    // "with a number of +1/+1 counters on it equal to the amount of mana spent to cast it": a
    // count the number words can't spell
    let literal = crate::words::is_quantity;
    let number = lit("with a number of")
        .then_keep(&additional)
        .then(&counter_kind())
        .skip(&lit("counters"))
        .then(&on())
        .skip(&lit("equal to"))
        .then(&amounts().amount)
        .map(
            move |(((additional, kind), on), count)| {
                let extra = additional.then_some(ir::Extra::Additional);
                (!literal(&count)).then_some(Arrival::Counters(ir::CountersOn {
                    counters: ir::Counters { count, up_to: false, extra, kind },
                    on,
                }))
            },
            move |e| match e {
                Arrival::Counters(ir::CountersOn { counters: ir::Counters { count, up_to: false, extra, kind }, on })
                    if !literal(count) && *extra != Some(ir::Extra::Another) =>
                {
                    Some((((extra.is_some(), kind.clone()), *on), count.clone()))
                }
                _ => None,
            },
        );
    // "with two +1/+1 counters and a lifelink counter on it"
    let several = lit("with").then_keep(&crate::selector::some_counters().list("and")).then(&on()).map(
        |(counters, on)| Some(Arrival::SeveralCounters { counters, on }),
        |a| match a {
            Arrival::SeveralCounters { counters, on } => Some((counters.clone(), *on)),
            _ => None,
        },
    );
    let copy = lit("as a copy of").then_keep(&selectors().selector).map(
        |x| Some(Arrival::AsCopyOf(x)),
        |a| match a {
            Arrival::AsCopyOf(x) => Some(x.clone()),
            _ => None,
        },
    );
    alt(
        "arrival",
        vec![
            lit("tapped and attacking").to(Arrival::TappedAndAttacking),
            lit("tapped").to(Arrival::Tapped),
            lit("prepared").to(Arrival::Prepared),
            counters,
            several,
            choice,
            number,
            copy,
            lit("untapped").to(Arrival::Untapped),
            lit("under the control of").then_keep(&selectors().selector).map(
                |x| Some(Arrival::UnderControlOf(x)),
                |a| match a {
                    Arrival::UnderControlOf(x) => Some(x.clone()),
                    _ => None,
                },
            ),
        ],
    )
}

/// "with flying": the abilities a permanent enters with (614.1c).
fn arrival_has() -> P<Arrival> {
    lit("with").then_keep(&crate::action::granted()).map(
        |g| Some(Arrival::Has(g)),
        |a| match a {
            Arrival::Has(g) => Some(g.clone()),
            _ => None,
        },
    )
}

/// How a permanent enters: one part, then more joined by nothing ("tapped with three charge
/// counters on it") or by "and" ("two +1/+1 counters on it and with flying"). An ability part is
/// always joined with "and"; "tapped and with three coin counters" is the same meaning as the
/// plain juxtaposition and reads as an alias. "as a copy of …" comes last: what follows it belongs
/// to its selector ("as a copy of a creature card in exile with a takeover counter on it").
fn arrivals() -> P<Vec<Arrival>> {
    let not_has = |a: &Arrival| !matches!(a, Arrival::Has(_));
    let plain = arrival().map(move |a| not_has(&a).then_some(a), move |a| not_has(a).then(|| a.clone()));
    let and_has = lit("and").then_keep(&arrival_has());
    let and_plain = lit("and").then_keep(&plain).alias();
    let later = alt("later arrival", vec![plain, and_has, and_plain]);
    let first = alt("first arrival", vec![arrival(), arrival_has()]);
    first.then(&later.many()).map(
        |(first, mut rest)| {
            rest.insert(0, first);
            let n = rest.len();
            (!rest[..n - 1].iter().any(|a| matches!(a, Arrival::AsCopyOf(_)))).then_some(rest)
        },
        |v: &Vec<Arrival>| v.split_first().map(|(f, r)| (f.clone(), r.to_vec())),
    )
}

/// One thing "can't" forbids: an event of the subject, as a rule says it ("attack", "be blocked
/// by creatures with flying", "be countered", "cast noncreature spells"), any of the event
/// grammar's. A limit names what it is counted within ("cast more than one spell each turn").
fn forbidden(form: Form) -> P<Forbidden> {
    let event = crate::event::rule_action(Form::Cant, form != Form::Third);
    let plain = event.map(
        |event| Some(Forbidden { event, within: None }),
        |f: &Forbidden| f.within.is_none().then(|| f.event.clone()),
    );
    // "more than one spell each turn", "more than two permanents during their untap steps"
    let limit = |e: &EventAction| match e {
        EventAction::Deed(d) => matches!(d.what().map(|w| &w.quant), Some(Quant::Indefinite(ir::Count::AtLeast(_)))),
        _ => false,
    };
    let span = alt(
        "span",
        vec![
            lit("each turn").to(Within::EachTurn),
            lit("during their untap steps").to(Within::UntapStep),
            // after "you"
            lit("during your untap step").to(Within::UntapStep).alias(),
        ],
    );
    let limited = event.then(&span).map(
        move |(event, within)| limit(&event).then(|| Forbidden { event, within: Some(within) }),
        |f: &Forbidden| match f.within {
            Some(w @ (Within::EachTurn | Within::UntapStep)) => Some((f.event.clone(), w)),
            _ => None,
        },
    );
    // "be blocked this turn except by Walls": the exception follows how long it lasts
    let blocked = lit("be blocked").then_keep(&within()).skip(&lit("except by")).then(&crate::event::generic_list(false)).map(
        |(within, by)| Some(Forbidden { event: EventAction::BecomesBlocked { by, except: true }, within: Some(within) }),
        |f: &Forbidden| match (&f.event, f.within) {
            (EventAction::BecomesBlocked { by, except: true }, Some(w @ (Within::EachCombat | Within::ThisTurn | Within::ThisCombat))) => {
                Some((w, by.clone()))
            }
            _ => None,
        },
    );
    alt("forbidden", vec![plain, limited, blocked])
}

/// "can't attack", "can't attack or block", "can't be blocked by creatures with flying", "can't
/// cast noncreature spells": a prohibition (101.2) on events with the clause's subject.
pub(crate) fn cant(form: Form) -> P<Action> {
    let plain = lit("can't").then_keep(&one_or_list(&forbidden(form), "or")).map(
        |v| Some(Action::Cant(v)),
        |a| match a {
            Action::Cant(v) => Some(v.clone()),
            _ => None,
        },
    );
    // "can cast no more than two spells each turn" is the same prohibition as
    // "can't cast more than two spells each turn"; it grants no extra casting permission.
    let verbs = crate::kernel::table("count-limited action", crate::event::player_object_verbs()
        .map(|v| (v.head(ir::lexicon::Inflection::Base), v)).collect());
    let capped = lit("can").then_keep(&verbs).skip(&lit("no more than"))
        .then(&selectors().selector).skip(&lit("each turn")).map(
            |(verb, mut what)| {
                let Quant::Indefinite(ir::Count::Exactly(Amount::Const(n))) = what.quant else { return None };
                if what.chooser.is_some() || what.random.is_some() || !ir::lexicon::fills(&what, verb, ir::Role::What) {
                    return None;
                }
                what.quant = Quant::Indefinite(ir::Count::AtLeast(Amount::Const(n.checked_add(1)?)));
                Some(Action::Cant(vec![Forbidden { event: EventAction::does(verb, what), within: Some(Within::EachTurn) }]))
            },
            |_: &Action| None,
        ).alias();
    alt("prohibition", vec![plain, capped])
}

/// What a losing effect takes away: "all abilities", "all other card types and abilities", "all
/// creature types", "flying" (613.1d, 613.1f).
pub(crate) fn lost() -> P<Lost> {
    let lose = alt(
        "what is lost",
        vec![
            lit("abilities").to(Lose::Abilities),
            crate::becomes::class_noun(&[ir::Every::CardTypes, ir::Every::CreatureTypes, ir::Every::LandTypes]).map(|e| Some(Lose::Types(e)), |l| match l {
                Lose::Types(e) => Some(*e),
                Lose::Abilities => None,
            }),
        ],
    );
    let one_lose = lose.map(|l| Some(vec![l]), |v: &Vec<Lose>| (v.len() == 1).then(|| v[0]));
    let all_of = lit("all")
        .then_keep(&lit("other").opt())
        .then(&alt("what is lost", vec![one_lose, lose.list("and")]))
        .map(
            |(other, what)| Some(Lost::AllOf { other: other.is_some(), what }),
            |l| match l {
                Lost::AllOf { other, what } => Some((other.then_some(()), what.clone())),
                _ => None,
            },
        );
    // a class of types is `AllOf`'s, never a named ability
    let named = granted_any().map(
        |g| (!g.iter().any(|g| matches!(g, Granted::Every(_)))).then_some(Lost::Abilities(g)),
        |l| match l {
            Lost::Abilities(g) if !g.iter().any(|g| matches!(g, Granted::Every(_))) => Some(g.clone()),
            _ => None,
        },
    );
    alt("lost", vec![all_of, lit("this ability").to(Lost::ThisAbility), named])
}

/// "can attack as though it didn't have defender", "can block creatures with shadow as though they
/// didn't have shadow", "can be blocked as though it didn't have swampwalk", "can be cast as though
/// it had flash" (609.4): a permission, in the event grammar's words for what can happen, with the
/// condition it treats as true. The pronoun names the object the ability is about: the subject, except
/// after "block" where what did not have the ability is the creature blocked.
fn can_as_though(form: Form) -> P<StaticAct> {
    use ir::statics::Premise;
    let plural = form != Form::Third;
    let events = one_or_list(&crate::event::rule_action(Form::Cant, plural), "or");
    let pronoun = alt("pronoun", vec![lit("it").to(false), lit("they").to(true)]);
    let absent = crate::keyword::keyword();
    let premise = alt(
        "premise",
        vec![
            pronoun.skip(&lit("had")).then(&crate::action::granted()).map(
                |(p, g)| Some((p, Premise::Had(g))),
                |(p, x)| match x {
                    Premise::Had(g) => Some((*p, g.clone())),
                    _ => None,
                },
            ),
            pronoun.skip(&lit("didn't have")).then(&absent).map(
                |(p, a)| Some((p, Premise::DidntHave(a))),
                |(p, x)| match x {
                    Premise::DidntHave(a) => Some((*p, a.clone())),
                    _ => None,
                },
            ),
            lit("it was untapped").to((false, Premise::Untapped)),
            lit("they were untapped").to((true, Premise::Untapped)),
        ],
    );
    // the creatures blocked: what "didn't have" an ability after "block X" (a rule says the
    // bare plural "creatures with shadow", so the pronoun is "they")
    let blocked = |events: &[EventAction]| matches!(events, [EventAction::Blocks { blocked: Some(_) }]);
    // does the pronoun agree with whom the premise is about?
    let fits = move |events: &[EventAction], p: bool, x: &Premise| {
        let blocks = events.iter().all(|e| matches!(e, EventAction::Blocks { .. }));
        // what abilities decide: attacking and blocking (702.3, 702.9, 702.28 …), and casting (702.8)
        let combat_or_cast = events.iter().all(|e| {
            matches!(e, EventAction::Attacks { .. } | EventAction::Blocks { .. } | EventAction::BecomesBlocked { .. })
                || matches!(e.reading(), Some((Verb::Cast | Verb::Play, _)))
        });
        let targetable = events.iter().all(|e| matches!(e, EventAction::BecomesTarget { .. }));
        (combat_or_cast || targetable)
            && events.iter().all(EventAction::roles_fit)
            && match x {
            Premise::Untapped => blocks,
            Premise::DidntHave(_) if blocked(events) => p,
            _ => p == plural,
        }
    };
    lit("can").then_keep(&events).then(&within().opt()).skip(&lit("as though")).then(&premise).map(
        move |((events, when), (p, as_though))| fits(&events, p, &as_though).then_some(StaticAct::Can { events, when, as_though }),
        move |a| match a {
            StaticAct::Can { events, when, as_though } => {
                let p = plural || (matches!(as_though, Premise::DidntHave(_)) && blocked(events));
                fits(events, p, as_though).then(|| ((events.clone(), *when), (p, as_though.clone())))
            }
            _ => None,
        },
    )
}

/// The static verb phrases in one verb form, as actions ([`Action::Static`]).
pub fn static_act(form: Form) -> P<Action> {
    let s = selectors();
    let v = |third: &str, base: &str| verb(form, third, base, "");
    let has = act(v("has", "have").then_keep(&granted_any()), StaticAct::Has, |a| match a {
        StaticAct::Has(g) => Some(g.clone()),
        _ => None,
    });
    let loses = act(v("loses", "lose").then_keep(&lost()), StaticAct::Loses, |a| match a {
        StaticAct::Loses(l) => Some(l.clone()),
        _ => None,
    });
    let numbers: &[Number] = if form == Form::Third { &[Number::Sg] } else { &[Number::Pl] };
    let is_not = act(v("isn't", "aren't").then_keep(&crate::becomes::becoming(numbers)), StaticAct::IsNot, |a| {
        match a {
            StaticAct::IsNot(b) => Some(b.clone()),
            _ => None,
        }
    });
    let arrivals = arrivals();
    let enters = act(v("enters", "enter").then_keep(&arrivals), StaticAct::Enters, |a| match a {
        StaticAct::Enters(how) => Some(how.clone()),
        _ => None,
    });
    let saddles = act(
        alt(
            "crew power",
            vec![
                v("saddles Mounts and crews Vehicles as though its power were", "saddle Mounts and crew Vehicles as though its power were")
                    .then_keep(&crate::words::stat_amount())
                    .skip(&lit("greater"))
                    .map(|n| Some(Some(n)), |o: &Option<Amount>| o.clone()),
                v(
                    "saddles Mounts and crews Vehicles using its toughness rather than its power",
                    "saddle Mounts and crew Vehicles using its toughness rather than its power",
                )
                .to(None),
            ],
        ),
        StaticAct::SaddlesAndCrews,
        |a| match a {
            StaticAct::SaddlesAndCrews(n) => Some(n.clone()),
            _ => None,
        },
    );
    let crews = act(
        v("crews Vehicles as though its power were", "crew Vehicles as though its power were")
            .then_keep(&crate::words::stat_amount())
            .skip(&lit("greater")),
        StaticAct::CrewsAsThoughGreater,
        |a| match a {
            StaticAct::CrewsAsThoughGreater(n) => Some(n.clone()),
            _ => None,
        },
    );
    let escapes = act(v("escapes", "escape").then_keep(&arrivals), StaticAct::EscapesWith, |a| match a {
        StaticAct::EscapesWith(how) => Some(how.clone()),
        _ => None,
    });
    let more = alt("less or more", vec![lit("less").to(false), lit("more").to(true)]);
    let of = alt("cost of", vec![lit("to cast").to(CostOf::Cast), lit("to activate").to(CostOf::Activate)]);
    let cost_change = act(
        v("costs", "cost").then_keep(&mana_cost()).then(&more).then(&of),
        |((by, more), of)| StaticAct::CostChange { more, by, of },
        |a| match a {
            StaticAct::CostChange { more, by, of } => Some(((by.clone(), *more), *of)),
            _ => None,
        },
    );
    // "costs {X} less to cast this way"
    let cost_change_this_way = act(
        v("costs", "cost").then_keep(&mana_cost()).then(&more).skip(&lit("to cast this way")),
        |(by, more)| StaticAct::CostChangeThisWay { more, by },
        |a| match a {
            StaticAct::CostChangeThisWay { more, by } => Some((by.clone(), *more)),
            _ => None,
        },
    );
    // A floor of the final cost (601.2f): the threshold and resulting minimum
    // are the same quantity. An arbitrary conditional cost-setting rule is different.
    let minimum = act(
        lit("that would cost less than").then_keep(&count_amount()).skip(&lit("mana"))
            .then(&of).skip(&v("costs", "cost")).then(&count_amount()).skip(&lit("mana")).then(&of)
            .map(|(((threshold, of), amount), again)| (threshold == amount && of == again).then_some((amount, of)),
                |(amount, of): &(Amount, CostOf)| Some((((amount.clone(), *of), amount.clone()), *of))),
        |(amount, of)| StaticAct::CostMinimum { amount, of },
        |a| match a { StaticAct::CostMinimum { amount, of } => Some((amount.clone(), *of)), _ => None },
    );
    // a requirement to attack or block (508.1d, 509.1c: what an effect instructs is done "if
    // able" as an instruction): said of the one who does it as the event's own phrase ("attacks
    // each combat if able"), of the one it is done to with "must" ("must be blocked if able")
    let plural = form != Form::Third;
    let combat = |a: &EventAction| matches!(a.reading(), Some((Verb::Attack | Verb::Block, _)));
    let does_it = |a: &EventAction| matches!(a.reading(), Some((_, ir::SubjectIs::Doer)));
    let does = one_or_list(&crate::event::action(form, plural), "or").map(
        move |v| v.iter().all(|a| combat(a) && does_it(a)).then_some(v),
        move |v: &Vec<EventAction>| v.iter().all(|a| combat(a) && does_it(a)).then(|| v.clone()),
    );
    let must = lit("must").then_keep(&one_or_list(&crate::event::rule_action(Form::Must, plural), "or")).map(
        move |v| v.iter().all(combat).then_some(v),
        move |v: &Vec<EventAction>| (v.iter().all(combat) && !v.iter().all(does_it)).then(|| v.clone()),
    );
    // "attacks each turn if able" is obeyed by attacking in one combat of the turn, not in
    // each (508.1d): its own window, not "each combat"
    let required_within = alt("requirement window", vec![within(), lit("each turn").to(Within::EachTurn)]);
    let if_able = act(
        alt("required", vec![does, must]).then(&required_within.opt()).skip(&lit("if able")),
        |(what, when)| StaticAct::IfAble { what, when },
        |a| match a {
            StaticAct::IfAble { what, when } => Some((what.clone(), *when)),
            _ => None,
        },
    );
    let block_only = act(lit("can block only").then_keep(&s.set_pl), StaticAct::CanBlockOnly, |a| match a {
        StaticAct::CanBlockOnly(x) => Some(x.clone()),
        _ => None,
    });
    // a counted number always names its window ("each combat", "this turn"); after "any number
    // of creatures" a "this turn" is the act's duration
    let window = alt("window", vec![lit("each combat").to(Within::EachCombat), lit("this turn").to(Within::ThisTurn)]);
    let counted = alt(
        "additional blocks",
        vec![
            lit("an additional creature").to(Amount::Const(1)),
            count_amount().skip(&lit("additional creatures")).map(
                |n| (n != Amount::Const(1)).then_some(n),
                |n| (*n != Amount::Const(1)).then(|| n.clone()),
            ),
        ],
    );
    let additional = alt(
        "additional blocks",
        vec![
            lit("up to").opt().then(&counted).then(&window).map(
                |((up_to, n), w)| Some((Some(n), up_to.is_some(), Some(w))),
                |(n, up_to, w)| Some(((up_to.then_some(()), n.clone()?), (*w)?)),
            ),
            lit("any number of creatures").to((None, false, None)),
        ],
    );
    let block_additional = act(
        lit("can block").then_keep(&additional),
        |(count, up_to, when)| StaticAct::CanBlockAdditional { count, up_to, when },
        |a| match a {
            StaticAct::CanBlockAdditional { count, up_to, when } => Some((count.clone(), *up_to, *when)),
            _ => None,
        },
    );
    let can = can_as_though(form);
    let unused = act(
        v("activates", "activate").then_keep(&s.set_pl).skip(&lit("as though they haven't been activated")),
        StaticAct::ActivateAsThoughUnused,
        |a| match a { StaticAct::ActivateAsThoughUnused(set) => Some(set.clone()), _ => None },
    );
    // the pronouns agree with the subject: singular after "has ~ assign" too
    let unblocked_as = |plural: bool| {
        let (its, it) = if plural { ("their", "they") } else { ("its", "it") };
        lit(&format!("{its} combat damage"))
            .then_keep(&lit("this turn").opt())
            .skip(&lit(&format!("as though {it} weren't blocked")))
            .map(move |t| Some((t.is_some(), plural)), move |(t, p)| (*p == plural).then(|| t.then_some(())))
    };
    let unblocked_forms =
        if form == Form::Third { vec![unblocked_as(false)] } else { vec![unblocked_as(true), unblocked_as(false)] };
    let unblocked = act(
        v("assigns", "assign").then_keep(&alt("as though unblocked", unblocked_forms)),
        |(this_turn, plural)| StaticAct::AssignAsThoughUnblocked { this_turn, plural },
        |a| match a {
            StaticAct::AssignAsThoughUnblocked { this_turn, plural } => Some((*this_turn, *plural)),
            _ => None,
        },
    );
    let look = act(v("looks at", "look at").then_keep(&who()).skip(&lit("any time")), StaticAct::LookAnyTime, |a| {
        match a {
            StaticAct::LookAnyTime(w) => Some(w.clone()),
            _ => None,
        }
    });
    // "it had flash" after one object, "they had flash" after several
    let flash_one = who().map(|w| (!who_plural(&w)).then_some(w), |w| (!who_plural(w)).then(|| w.clone()));
    let flash_many = who().map(|w| who_plural(&w).then_some(w), |w| who_plural(w).then(|| w.clone()));
    let flash_what = alt(
        "cast as though it had flash",
        vec![
            flash_one.then(&lit("this turn").opt()).skip(&lit("as though it had flash")),
            flash_many.then(&lit("this turn").opt()).skip(&lit("as though they had flash")),
        ],
    );
    let flash = act(
        v("casts", "cast").then_keep(&flash_what),
        |(what, t)| StaticAct::CastAsThoughFlash { what, this_turn: t.is_some() },
        |a| match a {
            StaticAct::CastAsThoughFlash { what, this_turn } => Some((what.clone(), this_turn.then_some(()))),
            _ => None,
        },
    );
    let color = crate::kernel::table("color", ir::Color::ALL.iter().map(|c| (c.word().to_string(), *c)).collect());
    let any = alt("any mana", vec![lit("color").to(AnyMana::Color), lit("type").to(AnyMana::Type)]);
    let spent = alt(
        "mana",
        vec![
            empty().to(SpentMana::Any),
            lit("colorless").to(SpentMana::Colorless),
            color.map(
                |c| Some(SpentMana::Color(c)),
                |m| match m {
                    SpentMana::Color(c) => Some(*c),
                    _ => None,
                },
            ),
        ],
    );
    let spend = act(
        v("spends", "spend")
            .then_keep(&spent)
            .skip(&lit("mana as though it were mana of any"))
            .then(&any)
            .then(&purpose().opt()),
        |((mana, any), to)| StaticAct::SpendAsThough { mana, any, to },
        |a| match a {
            StaticAct::SpendAsThough { mana, any, to } => Some(((*mana, *any), to.clone())),
            _ => None,
        },
    );
    let delta = crate::action::delta;
    let gets_additional = act(
        v("gets an additional", "get an additional").then_keep(&delta()).skip(&lit("/").tight()).then(&delta().tight()),
        |(power, toughness)| StaticAct::GetsAdditional { power, toughness },
        |a| match a {
            StaticAct::GetsAdditional { power, toughness } => Some((power.clone(), toughness.clone())),
            _ => None,
        },
    );
    // a definite object ("enchanted creature"): "unless you control a planeswalker" is a condition
    let definite = |x: &Selector| matches!(x.quant, Quant::Definite(_));
    let controls = act(
        v("controls", "control").then_keep(&s.selector).map(move |x| definite(&x).then_some(x), move |x| definite(x).then(|| x.clone())),
        StaticAct::Controls,
        |a| match a {
            StaticAct::Controls(x) => Some(x.clone()),
            _ => None,
        },
    );
    let lands = alt(
        "additional lands",
        vec![
            lit("an additional land").to(Amount::Const(1)),
            count_amount().skip(&lit("additional lands")).map(
                |n| (n != Amount::Const(1)).then_some(n),
                |n| (*n != Amount::Const(1)).then(|| n.clone()),
            ),
        ],
    );
    let whose_turns = alt("whose turns", vec![lit("on each of your turns").to(false), lit("on each of their turns").to(true)]);
    let more_lands = act(
        v("plays", "play").then_keep(&lands).then(&whose_turns),
        |(count, their)| StaticAct::AdditionalLandsEachTurn { count, their },
        |a| match a {
            StaticAct::AdditionalLandsEachTurn { count, their } => Some((count.clone(), *their)),
            _ => None,
        },
    );
    let not_untap = act(
        v("chooses not to untap", "choose not to untap").then_keep(&s.selector).skip(&lit("during your untap step")),
        StaticAct::ChooseNotToUntap,
        |a| match a {
            StaticAct::ChooseNotToUntap(x) => Some(x.clone()),
            _ => None,
        },
    );
    let skip_step = alt(
        "step",
        vec![
            lit("untap step").to(SkipStep::Untap),
            lit("upkeep step").to(SkipStep::Upkeep),
            lit("draw step").to(SkipStep::Draw),
            lit("combat phase").to(SkipStep::Combat),
        ],
    );
    // what is skipped is the skipper's own (500.11, 614.10): "you skip your draw step", "target
    // player skips their next untap step"; never another player's ("target player skips your
    // next draw step")
    let own = if form == Form::Third { "their" } else { "your" };
    let skipped = alt(
        "skipped",
        vec![
            lit(own).then_keep(&lit("next").opt()).then(&skip_step).map(
                |(next, step)| Some(Skipped::Step { step, next: next.is_some() }),
                |s| match s {
                    Skipped::Step { step, next } => Some((next.then_some(()), *step)),
                    _ => None,
                },
            ),
            lit(&format!("{own} next turn")).to(Skipped::Turns(Amount::Const(1))),
            lit(&format!("all combat phases of {own} next turn")).to(Skipped::CombatPhasesOfNextTurn),
            lit("that turn").to(Skipped::ThatTurn),
            lit("that draw").to(Skipped::ThatDraw),
            lit(&format!("{own} next")).then_keep(&crate::words::count_atom()).skip(&lit("turns")).map(
                |n| (n != Amount::Const(1)).then_some(Skipped::Turns(n)),
                |s| match s {
                    Skipped::Turns(n) if *n != Amount::Const(1) => Some(n.clone()),
                    _ => None,
                },
            ),
        ],
    );
    let skip = act(v("skips", "skip").then_keep(&skipped), StaticAct::Skip, |a| match a {
        StaticAct::Skip(s) => Some(s.clone()),
        _ => None,
    });
    // "their hands" of several players is each one's hand ("Players play with their hands
    // revealed"), read as "their hand"
    let their_hands = lit("their hands")
        .map(
            |()| {
                let they = ir::Selector::new(ir::Quant::Definite(ir::Ref::Anaphor(ir::selector::Anaphor::They)), ir::ObjectSet::of(ir::Domain::Player));
                let mut hand = ir::ObjectSet::of(ir::Domain::Card);
                hand.zone = Some(ir::ZoneSpec::of(ir::Zone::Hand, Some(they)));
                Some(ir::Selector::new(ir::Quant::All, hand))
            },
            |_: &ir::Selector| None,
        )
        .alias();
    let revealed = act(
        v("plays with", "play with").then_keep(&alt("revealed what", vec![s.selector.clone(), their_hands])).skip(&lit("revealed")),
        StaticAct::PlayWithRevealed,
        |a| match a {
            StaticAct::PlayWithRevealed(x) => Some(x.clone()),
            _ => None,
        },
    );
    let activity = alt("activity", vec![lit("attack").to(Verb::Attack), lit("block").to(Verb::Block)]);
    let cant_alone = act(
        lit("can't").then_keep(&one_or_list(&activity, "or")).skip(&lit("alone")),
        StaticAct::CantAlone,
        |a| match a {
            StaticAct::CantAlone(v) => Some(v.clone()),
            _ => None,
        },
    );
    let playable = alt(
        "playable",
        vec![v("plays", "play").to(true), v("casts", "cast").to(false)],
    )
    .then(&s.set_pl)
    .map(|(play, what)| Some(Playable { play, what }), |p| Some((p.play, p.what.clone())));
    let playables = alt(
        "playables",
        vec![
            playable.map(|p| Some(vec![p]), |v: &Vec<Playable>| (v.len() == 1).then(|| v[0].clone())),
            playable.skip(&lit("and")).then(&playable).map(
                |(a, b)| Some(vec![a, b]),
                |v: &Vec<Playable>| (v.len() == 2).then(|| (v[0].clone(), v[1].clone())),
            ),
        ],
    );
    let play_from = act(
        playables.then(&lit("from").then_keep(&from_zone()).opt()),
        |(what, from)| StaticAct::PlayFrom { what, from },
        |a| match a {
            StaticAct::PlayFrom { what, from } => Some((what.clone(), from.clone())),
            _ => None,
        },
    );
    let untap_others = act(
        v("untaps", "untap")
            .then_keep(&crate::action::objects(&s.selector))
            .skip(&lit("during each other player's untap step")),
        StaticAct::UntapDuringOthersUntapSteps,
        |a| match a {
            StaticAct::UntapDuringOthersUntapSteps(o) => Some(o.clone()),
            _ => None,
        },
    );
    let all = alt(
        "static act",
        vec![
            can,
            unused,
            untap_others,
            play_from,
            lit("can be your commander").to(StaticAct::CanBeYourCommander),
            lit("do so").to(StaticAct::DoSo),
            v("doesn't lose the game for having 0 or less life", "don't lose the game for having 0 or less life")
                .to(StaticAct::NoLossAtZeroLife),
            // the recipient is the replaced event's: only modified amounts of "that damage"
            act(
                v("deals", "deal").then_keep(&crate::action::noun_amount("damage")).map(
                    |a| matches!(a, Amount::Multiple(..) | Amount::Plus(..) | Amount::Const(_) | Amount::X).then_some(a),
                    |a| matches!(a, Amount::Multiple(..) | Amount::Plus(..) | Amount::Const(_) | Amount::X).then(|| a.clone()),
                ),
                StaticAct::DealsToSame,
                |a| match a {
                    StaticAct::DealsToSame(n) => Some(n.clone()),
                    _ => None,
                },
            ),
            skip,
            revealed,
            cant_alone,
            gets_additional,
            controls,
            more_lands,
            not_untap,
            unblocked,
            look,
            flash,
            lit("can cast spells only any time they could cast a sorcery").to(StaticAct::CastOnly {
                what: ObjectSet::of(Domain::Spell), restrictions: vec![ir::Restriction::Sorcery],
            }),
            lit("can cast").then_keep(&s.set_pl).then(&crate::kernel::lazy("casting restrictions", || conditions().restrictions.clone())).map(
                |(what, restrictions)| (what.domain == Domain::Spell).then_some(StaticAct::CastOnly { what, restrictions }),
                |a| match a {
                    StaticAct::CastOnly { what, restrictions } if what.domain == Domain::Spell => Some((what.clone(), restrictions.clone())),
                    _ => None,
                },
            ),
            spend,
            has,
            loses,
            is_not,
            enters,
            escapes,
            saddles,
            crews,
            cost_change,
            cost_change_this_way,
            minimum,
            if_able,
            block_only,
            block_additional,
            v("assigns no combat damage", "assign no combat damage").to(StaticAct::AssignsNoCombatDamage),
            v(
                "assigns combat damage equal to its toughness rather than its power",
                "assign combat damage equal to their toughness rather than their power",
            )
            .to(StaticAct::AssignsByToughness),
            v("has no maximum hand size", "have no maximum hand size").to(StaticAct::NoMaximumHandSize),
        ],
    );
    all.map(
        |a| Some(Action::Static(Box::new(a))),
        |a| match a {
            Action::Static(s) => Some((**s).clone()),
            _ => None,
        },
    )
}

/// Source-qualified activation costs have one representation, whether the source is said
/// with "of" or "from" (602.2b). Other subject/action compositions stay ordinary bodies.
pub(crate) fn ability_cost_sentence(body: &Body) -> Option<Static> {
    let c = &body.first;
    if c.may || c.also || !c.rest.is_empty() || !body.rest.is_empty()
        || c.first.may || !c.first.mods.is_empty() { return None; }
    let Some(ir::Subject::Set(set)) = &c.subject else { return None };
    let (Some(kind), source) = crate::selector::ability_source(set)? else { return None };
    let kind = match kind {
        ir::selector::AbilityKind::Activated => AbilityKind::Activated,
        ir::selector::AbilityKind::Triggered => AbilityKind::Triggered,
        ir::selector::AbilityKind::Loyalty => AbilityKind::Loyalty,
        ir::selector::AbilityKind::Mana => return None,
    };
    let Action::Static(action) = &c.first.action else { return None };
    let StaticAct::CostChange { more, by, of: CostOf::Activate } = action.as_ref() else { return None };
    Some(Static::ActivatedAbilities {
        of: AbilitiesOf::Of { kind, of: ir::Coord::one(Who::Set(source.clone())) },
        rule: AbilityRule::CostChange { more: *more, by: by.clone(), unless_mana: false }, this_turn: false,
    })
}

/// Static sentences around a body (the effect grammar's, passed in).
pub fn sentence(body: &P<Body>) -> P<Static> {
    let c = conditions();
    let s = selectors();
    let no_more = lit("no more than")
        .then_keep(&alt(
            "count",
            vec![
                lit("one creature").to(Amount::Const(1)),
                count_amount().skip(&lit("creatures")).map(
                    |n| (n != Amount::Const(1)).then_some(n),
                    |n| (*n != Amount::Const(1)).then(|| n.clone()),
                ),
            ],
        ))
        .then(&alt("can", vec![lit("can attack").to(Verb::Attack), lit("can block").to(Verb::Block)]))
        .skip(&lit("each combat"))
        .map(
            |(count, activity)| Some(Static::NoMoreThan { count, activity }),
            |x| match x {
                Static::NoMoreThan { count, activity } => Some((count.clone(), *activity)),
                _ => None,
            },
        );
    let redirect = prevention(true).skip(&lit("is dealt to")).then(&party()).skip(&lit("instead")).map(
        |(damage, onto)| Some(Static::Redirect { damage, onto }),
        |x| match x {
            Static::Redirect { damage, onto } => Some((damage.clone(), onto.clone())),
            _ => None,
        },
    );
    let mana = lit("mana of any type can be spent").then_keep(&purpose());
    let join = alt("join", vec![lit(", and").to(Join::CommaAnd), lit("and").to(Join::And)]);
    let any_type = mana.map(
        |to| Some(Static::AnyTypeMana { to }),
        |x| match x {
            Static::AnyTypeMana { to } => Some(to.clone()),
            _ => None,
        },
    );
    let size = alt(
        "hand size",
        vec![
            lit("is reduced by").then_keep(&count_amount()).map(
                |n| Some(HandSize::ReducedBy(n)),
                |h| match h {
                    HandSize::ReducedBy(n) => Some(n.clone()),
                    _ => None,
                },
            ),
            lit("is increased by").then_keep(&count_amount()).map(
                |n| Some(HandSize::IncreasedBy(n)),
                |h| match h {
                    HandSize::IncreasedBy(n) => Some(n.clone()),
                    _ => None,
                },
            ),
            lit("is").then_keep(&count_amount()).map(
                |n| Some(HandSize::Is(n)),
                |h| match h {
                    HandSize::Is(n) => Some(n.clone()),
                    _ => None,
                },
            ),
            // "is equal to the number of hour counters on ~"
            lit("is equal to").then_keep(&crate::amount::amounts().amount).map(
                |n| (!crate::words::is_quantity(&n)).then(|| HandSize::Is(n)),
                |h| match h {
                    HandSize::Is(n) if !crate::words::is_quantity(n) => Some(n.clone()),
                    _ => None,
                },
            ),
        ],
    );
    let hand_size = s.possessive.skip(&lit("maximum hand size")).then(&size).map(
        |(whose, size)| Some(Static::MaxHandSize { whose, size }),
        |x| match x {
            Static::MaxHandSize { whose, size } => Some((whose.clone(), size.clone())),
            _ => None,
        },
    );
    let legend = lit("the \"legend rule\" doesn't apply")
        .then_keep(&lit("to").then_keep(&party()).opt())
        .then(&lit("this turn").opt())
        .map(
            |(to, t)| Some(Static::NoLegendRule { to, this_turn: t.is_some() }),
            |x| match x {
                Static::NoLegendRule { to, this_turn } => Some((to.clone(), this_turn.then_some(()))),
                _ => None,
            },
        );
    let mana_cant = lit("this mana can't be spent").then_keep(&purpose()).map(
        |p| Some(Static::ManaCantBeSpent(p)),
        |x| match x {
            Static::ManaCantBeSpent(p) => Some(p.clone()),
            _ => None,
        },
    );
    // an offer ("unless its controller pays {2}") is the act modifier
    let not_payment = |c: &ir::Condition| !matches!(c, ir::Condition::Does { .. });
    let unless = body.skip(&lit("unless")).then(&c.condition).map(
        move |(body, cond)| not_payment(&cond).then_some(Static::Unless { body, cond }),
        move |x| match x {
            Static::Unless { body, cond } if not_payment(cond) => Some((body.clone(), cond.clone())),
            _ => None,
        },
    );
    // A following rule may refer to the preceding clause, or replace an event that
    // happens later. It retains the surrounding sentence's leads and conditions.
    let replacement = replacement(body);
    let following = alt("following", vec![activated_abilities(), any_type, unpreventable(), replacement]);
    let after = body.then(&join).then(&following).map(
        |((body, join), rest)| Some(Static::After { body, join, rest: Box::new(rest) }),
        |x| match x {
            Static::After { body, join, rest } => Some(((body.clone(), *join), (**rest).clone())),
            _ => None,
        },
    );
    alt(
        "static",
        vec![
            lit("this effect can't reduce the mana in that cost to less than one mana").to(Static::ReductionFloorOneMana),
            lit("players play with the top card of their libraries revealed").to(Static::PlayersPlayRevealed),
            lit("counters remain on ~ as it moves to any zone other than a player's hand or library").to(Static::CountersRemain),
            lit("players can't activate planeswalkers' loyalty abilities").to(Static::NoLoyaltyAbilities),
            lit("all lands are no longer snow").to(Static::LandsNoLongerSnow),
            // "If ~ attacks, all creatures you control attack if able."
            lit("if")
                .then_keep(&selectors().singular)
                .skip(&lit("attacks, all"))
                .then(&selectors().set_pl)
                .skip(&lit("attack if able"))
                .map(
                    |(attacker, all)| Some(Static::IfAttacksAllAttack { attacker, all }),
                    |s| match s {
                        Static::IfAttacksAllAttack { attacker, all } => Some((attacker.clone(), all.clone())),
                        _ => None,
                    },
                ),
            lit("you may reveal the first card you draw each turn as you draw it").to(Static::RevealFirstDraw { may: true }),
            lit("reveal the first card you draw each turn").to(Static::RevealFirstDraw { may: false }),
            instant_activations(),
            lit("you may activate loyalty abilities of planeswalkers you control on any player's turn any time you could cast an instant")
                .to(Static::LoyaltyAtInstantSpeed),
            lit("you may activate the loyalty abilities of planeswalkers you control twice each turn rather than only once")
                .to(Static::LoyaltyTwice { this_turn: false }),
            lit("you may activate the loyalty abilities of planeswalkers you control twice this turn rather than only once")
                .to(Static::LoyaltyTwice { this_turn: true }),
            lit("you may exert ~ as it attacks").to(Static::ExertAsItAttacks),
            lit("this mana can't be spent to cast spells from your hand").to(Static::ManaCantBeSpentFromHand),
            lit("remove ~ from your deck before playing if you're not playing for ante").to(Static::AnteOnly),
            lit("while an opponent is choosing targets as part of casting a spell they control or activating an ability they control, that player must choose at least one Flagbearer on the battlefield if able")
                .to(Static::Flagbearer),
            lit("Room abilities of dungeons you own trigger an additional time").to(Static::RoomAbilitiesTriggerAgain),
            lit("your opponents can't cast spells or activate abilities of artifacts, creatures, or enchantments")
                .to(Static::OpponentsCantCastOrActivate),
            lit("draft ~ face up").to(Static::DraftFaceUp),
            lit("~ can't attack a player it has already attacked this turn").to(Static::CantAttackSamePlayerAgain),
            // "If ~ is in a graveyard, effects from spells named N count it as a card named N."
            lit("if ~ is in a graveyard, effects from spells named")
                .then_keep(&crate::selector::name())
                .skip(&lit("count it as a card named"))
                .then(&crate::selector::name())
                .map(
                    |(a, b)| (a == b).then(|| Static::CountsAsNamedInGraveyard(a)),
                    |s| match s {
                        Static::CountsAsNamedInGraveyard(n) => Some((n.clone(), n.clone())),
                        _ => None,
                    },
                ),
            lit("no more than")
                .then_keep(&alt(
                    "count",
                    vec![
                        lit("one creature").to(Amount::Const(1)),
                        crate::words::count_amount().skip(&lit("creatures")).map(
                            |n| (n != Amount::Const(1)).then_some(n),
                            |n| (*n != Amount::Const(1)).then(|| n.clone()),
                        ),
                    ],
                ))
                .skip(&lit("can attack you each combat"))
                .map(|n| Some(Static::NoMoreThanAttackYou(n)), |s| match s {
                    Static::NoMoreThanAttackYou(n) => Some(n.clone()),
                    _ => None,
                }),
            lit("~ can be attached only to").then_keep(&selectors().selector).map(
                |to| Some(Static::AttachOnlyTo(to)),
                |s| match s {
                    Static::AttachOnlyTo(to) => Some(to.clone()),
                    _ => None,
                },
            ),
            lit("you may activate abilities of creatures you control as though those creatures had haste").to(Static::ActivateAsThoughHaste),
            lit("if you would roll one or more dice, instead roll that many dice plus one and ignore the lowest roll").to(Static::RollExtraIgnoreLowest),
            lit("double all damage that").then_keep(&selectors().set_pl).skip(&lit("would deal")).map(
                |set| Some(Static::DoubleDamageOf(set)),
                |s| match s {
                    Static::DoubleDamageOf(set) => Some(set.clone()),
                    _ => None,
                },
            ),
            lit("each player may attack only the nearest opponent in the last chosen direction and planeswalkers controlled by that opponent")
                .to(Static::AttackNearestInDirection { last: true }),
            lit("each player may attack only the nearest opponent in the chosen direction and planeswalkers controlled by that opponent")
                .to(Static::AttackNearestInDirection { last: false }),
            lit("reveal ~ as you draft it and note how many cards you've drafted this draft round, including ~").to(Static::DraftRevealNoteCount),
            lit("while voting, you get an additional vote").to(Static::AdditionalVote { vote_again: false }),
            lit("while voting, you may vote an additional time").to(Static::AdditionalVote { vote_again: true }),
            // "~ is also a Cleric, Rogue, Warrior, and Wizard."
            lit("~ is also a")
                .then_keep(
                    &crate::kernel::table(
                        "creature type",
                        ir::Subtype::all()
                            .filter(|s| s.kind() == ir::vocab::SubtypeKind::Creature)
                            .map(|s| (s.name().to_string(), s))
                            .collect(),
                    )
                    .list("and"),
                )
                .map(|v| Some(Static::AlsoSubtypes(v)), |s| match s {
                    Static::AlsoSubtypes(v) => Some(v.clone()),
                    _ => None,
                }),
            // "If you would lose unspent mana, that mana becomes black instead."
            lit("if you would lose unspent mana, that mana becomes")
                .then_keep(&alt(
                    "color or colorless",
                    vec![
                        crate::kernel::table("color", ir::Color::ALL.iter().map(|c| (c.word().to_string(), Some(*c))).collect()),
                        lit("colorless").to(None),
                    ],
                ))
                .skip(&lit("instead"))
                .map(|c| Some(Static::UnspentManaBecomes(c)), |s| match s {
                    Static::UnspentManaBecomes(c) => Some(*c),
                    _ => None,
                }),
            // "Creatures entering [the battlefield] [or dying] don't cause abilities [of …] to trigger."
            selectors()
                .set_pl
                .skip(&alt("entering", vec![lit("entering"), lit("entering the battlefield").alias()]))
                .then(&lit("or dying").opt())
                .skip(&lit("don't cause abilities"))
                .then(&lit("of").then_keep(&selectors().set_pl).opt())
                .skip(&lit("to trigger"))
                .map(
                    |((what, dying), of)| Some(Static::DontCauseTriggers { what, dying: dying.is_some(), of }),
                    |s| match s {
                        Static::DontCauseTriggers { what, dying, of } => Some(((what.clone(), dying.then_some(())), of.clone())),
                        _ => None,
                    },
                ),
            triggers_additional(),
            cant_cause(),
            lit("a deck can have").then_keep(&selectors().selector).map(
                |x| Some(Static::DeckCanHave(x)),
                |s| match s {
                    Static::DeckCanHave(x) => Some(x.clone()),
                    _ => None,
                },
            ),
            lit("you may pay").then_keep(&mana_cost()).skip(&lit("to end this effect")).map(
                |m| Some(Static::PayToEnd(m)),
                |s| match s {
                    Static::PayToEnd(m) => Some(m.clone()),
                    _ => None,
                },
            ),
            lit("this effect doesn't remove").then_keep(&selectors().selector).map(
                |x| Some(Static::DoesntRemove(x)),
                |s| match s {
                    Static::DoesntRemove(x) => Some(x.clone()),
                    _ => None,
                },
            ),
            begin_on_battlefield(),
            lit("you may reveal ~ from your opening hand").to(Static::RevealFromOpeningHand),
            unless,
            legend,
            mana_cant,
            hand_size,
            following.clone(),
            after,
            no_more,
            pt(),
            alternative_cost(),
            alternative_keyword_cost(),
            redirect,
            x_limit(),
            keyword_cost(),
        ],
    )
}

/// Activation timing permissions retain the ability class and the named source (606.3).
fn instant_activations() -> P<Static> {
    let kind = alt("activatable ability", vec![
        lit("loyalty").to(AbilityKind::Loyalty),
        lit("activated").to(AbilityKind::Activated),
    ]);
    let explicit = kind.skip(&lit("abilities of")).then(&party());
    let it = Selector::new(Quant::Definite(Ref::Anaphor(Anaphor::It)), ObjectSet::of(Domain::Object));
    let pronoun = alt("possessive pronoun", vec![lit("its"), lit("her").alias(), lit("his").alias()]);
    let possessed = pronoun.then_keep(&kind).skip(&lit("abilities")).map(
        move |kind| Some((kind, ir::Coord::one(Who::One(it.clone())))),
        |_| None,
    ).alias();
    lit("you may activate").then_keep(&alt("ability sources", vec![explicit, possessed]))
        .skip(&lit("any time you could cast an instant")).map(
            |(kind, of)| Some(Static::ActivateAtInstantSpeed { of: AbilitiesOf::Of { kind, of } }),
            |s| match s {
                Static::ActivateAtInstantSpeed { of: AbilitiesOf::Of { kind, of } }
                    if matches!(kind, AbilityKind::Loyalty | AbilityKind::Activated) => Some((*kind, of.clone())),
                _ => None,
            },
        )
}

/// A causal prohibition keeps the source description distinct from the player who acts.
fn cant_cause() -> P<Static> {
    let s = selectors();
    let player = alt("player", vec![lit("you").to(Selector::you()), s.singular.clone(), s.plural.clone()]);
    s.set_pl.skip(&lit("can't cause")).then(&player).skip(&lit("to")).then(&crate::event::rule_action(Form::Cant, false)).map(
        |((sources, player), event)| {
            let pattern = EventPattern::new(vec![player.clone()], vec![event.clone()]);
            pattern.typed().then_some(Static::CantCause { sources, player, event })
        },
        |stated| match stated {
            Static::CantCause { sources, player, event } => Some(((sources.clone(), player.clone()), event.clone())),
            _ => None,
        },
    )
}

/// "Activated abilities of artifacts can't be activated", "…, and its activated abilities can't be
/// activated unless they're mana abilities", "Activated abilities of creatures you control cost
/// {2} less to activate".
fn activated_abilities() -> P<Static> {
    let of = alt(
        "abilities of",
        vec![
            crate::selector::keyword_name().skip(&lit("abilities")).map(
                |keyword| Some(AbilitiesOf::Matching(ir::ObjectSet { filter: ir::Filter::Atom(ir::Atom::Keyword(keyword)), ..ir::ObjectSet::of(ir::Domain::Ability) })),
                |of| match of { AbilitiesOf::Matching(set) if *set == (ir::ObjectSet { filter: set.filter.clone(), ..ir::ObjectSet::of(ir::Domain::Ability) }) => match set.filter { ir::Filter::Atom(ir::Atom::Keyword(k)) => Some(k), _ => None }, _ => None },
            ),
            alt("keyword ability group", vec![
                crate::selector::keyword_name().skip(&lit("abilities of")),
                lit("each").then_keep(&crate::selector::keyword_name()).skip(&lit("ability of")),
            ]).then(&party()).map(
                |(keyword, of)| Some(AbilitiesOf::KeywordOf { keyword, of }),
                |o| match o { AbilitiesOf::KeywordOf { keyword, of } => Some((*keyword, of.clone())), _ => None },
            ),
            lit("its activated abilities").to(AbilitiesOf::Its),
            // "… and their activated abilities can't be activated": of the plural subject before
            lit("their activated abilities").to(AbilitiesOf::Its).alias(),
            // "enchanted creature's activated abilities": the possessive of "activated abilities
            // of enchanted creature"
            selectors()
                .selector
                .skip(&crate::kernel::leaf(
                    "'s",
                    |k| matches!(k, oracle::token::Kind::Clitic(c) if c == "'s").then_some(()),
                    |()| Some(oracle::token::Kind::Clitic("'s".into())),
                ))
                .skip(&lit("activated abilities"))
                .map(
                    |s| Some(AbilitiesOf::Of { kind: AbilityKind::Activated, of: ir::Coord::one(Who::One(s)) }),
                    |_: &AbilitiesOf| None,
                )
                .alias(),
            alt(
                "abilities",
                vec![
                    lit("activated abilities of").to(AbilityKind::Activated),
                    lit("loyalty abilities of").to(AbilityKind::Loyalty),
                    lit("triggered abilities of").to(AbilityKind::Triggered),
                ],
            )
            .then(&party())
            .map(
                |(kind, of)| Some(AbilitiesOf::Of { kind, of }),
                |o| match o {
                    AbilitiesOf::Of { kind, of } => Some((*kind, of.clone())),
                    _ => None,
                },
            ),
            crate::selector::keyword_name()
                .skip(&lit("abilities you activate"))
                .then(&lit("that target").then_keep(&selectors().selector).opt())
                .map(
                    |(keyword, target)| Some(AbilitiesOf::KeywordYouActivate { keyword, target: target.map(Box::new) }),
                    |o| match o {
                        AbilitiesOf::KeywordYouActivate { keyword, target } => Some((*keyword, target.as_deref().cloned())),
                        _ => None,
                    },
                ),
        ],
    );
    let more = alt("less or more", vec![lit("less").to(false), lit("more").to(true)]);
    let rule = alt(
        "ability rule",
        vec![
            lit("can't be activated").then_keep(&lit("unless they're mana abilities").opt()).map(
                |u| Some(AbilityRule::CantBeActivated { unless_mana: u.is_some() }),
                |r| match r {
                    AbilityRule::CantBeActivated { unless_mana } => Some(unless_mana.then_some(())),
                    _ => None,
                },
            ),
            lit("can be activated an additional time").to(AbilityRule::AdditionalTimes(Amount::Const(1))),
            lit("don't trigger").to(AbilityRule::DontTrigger),
            lit("cost").then_keep(&mana_cost()).then(&more).skip(&lit("to activate"))
                .then(&lit("unless they're mana abilities").opt()).map(
                |((by, more), unless)| Some(AbilityRule::CostChange { more, by, unless_mana: unless.is_some() }),
                |r| match r {
                    AbilityRule::CostChange { more, by, unless_mana } => Some(((by.clone(), *more), unless_mana.then_some(()))),
                    _ => None,
                },
            ),
        ],
    );
    of.then(&rule).then(&lit("this turn").opt()).map(
        |((of, rule), t)| Some(Static::ActivatedAbilities { of, rule, this_turn: t.is_some() }),
        |x| match x {
            Static::ActivatedAbilities { of, rule, this_turn } => Some(((of.clone(), rule.clone()), this_turn.then_some(()))),
            _ => None,
        },
    )
}

/// "~'s", "enchanted creature's", "its".
fn possessor() -> P<Selector> {
    let it = Selector::new(Quant::Definite(Ref::Anaphor(Anaphor::It)), ObjectSet::of(Domain::Object));
    alt("possessor", vec![lit("its").to(it), selectors().selector.skip(&clitic_s())])
}

fn clitic_s() -> P<()> {
    leaf("'s", |k| matches!(k, Kind::Clitic(c) if c == "'s").then_some(()), |()| Some(Kind::Clitic("'s".into())))
}

/// "~'s power and toughness are each equal to …", "~'s power is equal to … [and its toughness is
/// equal to …]", "~'s toughness is equal to …".
fn pt() -> P<Static> {
    let n = &amounts().amount;
    let each = lit("power and toughness are each equal to").then_keep(n).map(
        |a| Some((Some(a.clone()), Some(a))),
        |(p, t)| match (p, t) {
            (Some(p), Some(t)) if p == t => Some(p.clone()),
            _ => None,
        },
    );
    let its_toughness = lit("and its toughness is equal to").then_keep(n);
    let power = lit("power is equal to").then_keep(n).then(&its_toughness.opt()).map(
        |(p, t)| Some((Some(p), t)),
        |(p, t)| match (p, t) {
            (Some(p), Some(t)) if p == t => None,
            (Some(p), t) => Some((p.clone(), t.clone())),
            _ => None,
        },
    );
    let toughness = lit("toughness is equal to").then_keep(n).map(
        |t| Some((None, Some(t))),
        |(p, t)| match (p, t) {
            (None, Some(t)) => Some(t.clone()),
            _ => None,
        },
    );
    possessor().then(&alt("power and toughness", vec![each, power, toughness])).map(
        |(what, (power, toughness))| Some(Static::Pt { what, power, toughness }),
        |x| match x {
            Static::Pt { what, power, toughness } => Some((what.clone(), (power.clone(), toughness.clone()))),
            _ => None,
        },
    )
}

/// "you may tap an untapped creature you control rather than pay ~'s mana cost [if …]"; a
/// condition in front is the sentence's lead.
fn alternative_cost() -> P<Static> {
    let c = &conditions().condition;
    let actions = crate::action::actions(Form::Base);
    // the spell cast this way, "its mana cost"
    let it = Who::One(Selector::new(Quant::Definite(Ref::Anaphor(Anaphor::It)), ObjectSet::of(Domain::Object)));
    let (it1, it2) = (it.clone(), it.clone());
    // whose mana cost: ~'s, that spell's, or "the mana cost for spells you cast"
    let of = alt(
        "mana cost of",
        vec![
            lit("~'s mana cost").to(None),
            lit("its mana cost").to(Some(it.clone())),
            lit("the mana cost for").then_keep(&who()).map(
                move |w| (w != it1).then_some(Some(w)),
                move |o: &Option<Who>| o.clone().filter(|w| *w != it2),
            ),
        ],
    );
    let rather = lit("rather than pay").then_keep(&of);
    // an effect that allows casting may require it (118.9b): then it is about that spell
    // by paying life, the one thing the cards require instead of a mana cost
    let required = move |optional: bool, cost: &ir::Coord<Action>, of: &Option<Who>| {
        optional || (*of == Some(it.clone()) && cost.items.iter().all(|a| matches!(a, Action::PayLife(_))))
    };
    let (r1, r2) = (required.clone(), required);
    let may = alt("may", vec![lit("you may").to(true), crate::kernel::empty().to(false)]);
    let core = may.then(&actions).then(&rather).map(
        move |((optional, cost), of)| r1(optional, &cost, &of).then_some((optional, cost, of)),
        move |(optional, cost, of): &(bool, ir::Coord<Action>, Option<Who>)| {
            r2(*optional, cost, of).then(|| ((*optional, cost.clone()), of.clone()))
        },
    );
    // "rather than pay ~'s mana cost, you may …": the same alternative cost
    let rather_first = rather
        .skip(&lit(", you may"))
        .then(&actions)
        .map(|(of, cost)| Some((true, cost, of)), |_| None)
        .alias();
    let core_any = alt("alternative cost", vec![core.clone(), rather_first]);
    let plain = core_any.map(
        |(optional, cost, of)| Some(Static::AlternativeCost { cond: None, cost, of, optional }),
        |x| match x {
            Static::AlternativeCost { cond: None, cost, of, optional } => Some((*optional, cost.clone(), of.clone())),
            _ => None,
        },
    );
    let trailing = core.skip(&lit("if")).then(c).map(
        |((optional, cost, of), cond)| Some(Static::AlternativeCost { cond: Some(cond), cost, of, optional }),
        |x| match x {
            Static::AlternativeCost { cond: Some(cond), cost, of, optional } => {
                Some(((*optional, cost.clone(), of.clone()), cond.clone()))
            }
            _ => None,
        },
    );
    alt("alternative cost", vec![plain, trailing])
}

/// A replacement payment names a keyword's cost rather than a spell's mana cost (118.9).
fn alternative_keyword_cost() -> P<Static> {
    let keyword = crate::selector::keyword_name().map(
        |k| k.has_cost().then_some(k), |k| k.has_cost().then_some(*k),
    );
    lit("you may").then_keep(&crate::action::actions(Form::Base))
        .skip(&lit("rather than pay")).then(&possessor()).then(&keyword).skip(&lit("cost")).map(
            |((cost, of), keyword)| (of.set.domain != Domain::Player).then_some(Static::AlternativeKeywordCost { cost, of, keyword }),
            |s| match s {
                Static::AlternativeKeywordCost { cost, of, keyword } => Some(((cost.clone(), of.clone()), *keyword)),
                _ => None,
            },
        )
}

/// "by discarding a card in addition to paying its other costs", "by paying {1} rather than paying
/// its mana cost": what a cast is paid with (118.8: an additional cost, 118.9: an alternative
/// cost), after the act that casts.
pub(crate) fn cast_cost() -> P<ir::Modifier> {
    use ir::statics::{CastCost, CastCostKind};
    let tail = alt(
        "cast cost",
        vec![
            lit("in addition to paying its other costs").to((CastCostKind::Additional, false)),
            lit("in addition to paying their other costs").to((CastCostKind::Additional, true)),
            lit("rather than paying its mana cost").to((CastCostKind::Alternative, false)),
            lit("rather than paying their mana costs").to((CastCostKind::Alternative, true)),
        ],
    );
    lit("by").then_keep(&crate::action::actions(Form::Gerund)).then(&tail).map(
        |(actions, (kind, plural))| Some(ir::Modifier::By(CastCost { actions, kind, plural })),
        |m| match m {
            ir::Modifier::By(c) => Some((c.actions.clone(), (c.kind, c.plural))),
            _ => None,
        },
    )
}

/// "instead of putting it into its owner's graveyard", "instead of putting it anywhere else".
pub fn instead_of() -> P<Elsewhere> {
    let whose = alt(
        "graveyard",
        vec![
            lit("a graveyard").to(None),
            lit("your graveyard").to(Some(Whose::You)),
            lit("its owner's graveyard").to(Some(Whose::Owner)),
            lit("their graveyard").to(Some(Whose::Their)),
            lit("that player's graveyard").to(Some(Whose::Player(Box::new(ir::Selector::new(
                ir::Quant::Definite(ir::Ref::Anaphor(ir::selector::Anaphor::That)),
                ir::ObjectSet::of(ir::Domain::Player),
            ))))),
        ],
    );
    let graveyard = whose.then(&lit("as it resolves").opt()).map(
        |(w, r)| Some(if r.is_some() { Elsewhere::GraveyardAsItResolves(w) } else { Elsewhere::Graveyard(w) }),
        |e| match e {
            Elsewhere::Graveyard(w) => Some((w.clone(), None)),
            Elsewhere::GraveyardAsItResolves(w) => Some((w.clone(), Some(()))),
            _ => None,
        },
    );
    let place = alt(
        "elsewhere",
        vec![
            lit("into").then_keep(&graveyard),
            lit("into your hand").to(Elsewhere::Hand(Whose::You)),
            lit("anywhere else").to(Elsewhere::AnywhereElse),
        ],
    );
    // "put it into its owner's hand instead of into that player's graveyard": the same, with
    // "putting it" left out
    alt("instead of", vec![lit("instead of putting it"), lit("instead of").alias()]).then_keep(&place)
}

/// "damage", "combat damage", "noncombat damage".
fn damage_kind() -> P<DamageKind> {
    alt(
        "damage",
        vec![
            lit("damage").to(DamageKind::Any),
            lit("combat damage").to(DamageKind::Combat),
            lit("noncombat damage").to(DamageKind::Noncombat),
        ],
    )
}

/// "Damage can't be prevented this turn", "The damage can't be prevented", "Damage that would be
/// dealt by ~ can't be prevented".
fn unpreventable() -> P<Static> {
    let none = |amount: Prevented| Prevention { amount, kind: DamageKind::Any, to: None, by: None, to_or_by: false, this_turn: None };
    let damage = alt(
        "damage",
        vec![
            lit("damage").to(none(Prevented::All)),
            lit("the damage").to(none(Prevented::That)),
            // "damage that would be dealt by ~": the prevention phrase without its "all"
            damage_kind().skip(&lit("that would be dealt")).then(&alt(
                "to or by",
                vec![
                    lit("to").then_keep(&party()).map(|t| Some((Some(t), None)), |(t, b)| if b.is_none() { t.clone() } else { None }),
                    lit("by").then_keep(&party()).map(|b| Some((None, Some(b))), |(t, b)| if t.is_none() { b.clone() } else { None }),
                ],
            ))
            // "damage that would be dealt to that creature this turn": the window ends the phrase
            .then(&lit("this turn").opt())
            .map(
                |((kind, (to, by)), t)| {
                    Some(Prevention { amount: Prevented::All, kind, to, by, to_or_by: false, this_turn: t.map(|()| TurnAt::End) })
                },
                |p: &Prevention| {
                    (p.amount == Prevented::All
                        && matches!(p.this_turn, None | Some(TurnAt::End))
                        && (p.to.is_some() || p.by.is_some()))
                    .then(|| ((p.kind, (p.to.clone(), p.by.clone())), p.this_turn.map(|_| ())))
                },
            ),
        ],
    );
    // "can't be prevented or dealt instead to another permanent or player" (614.9, 615.12)
    let redirect = lit("or dealt instead to another permanent or player").opt();
    damage.skip(&lit("can't be prevented")).then(&redirect).then(&lit("this turn").opt()).map(
        |((damage, r), t)| Some(Static::Unpreventable { damage, this_turn: t.is_some(), redirect: r.is_some() }),
        |x| match x {
            Static::Unpreventable { damage, this_turn, redirect } => {
                Some(((damage.clone(), redirect.then_some(())), this_turn.then_some(())))
            }
            _ => None,
        },
    )
}

/// "X can't be 0", "X can't be greater than …".
fn x_limit() -> P<Static> {
    let limit = alt(
        "X limit",
        vec![
            lit("0").to(XLimit::NotZero),
            lit("greater than").then_keep(&amounts().amount).map(
                |a| Some(XLimit::AtMost(a)),
                |l| match l {
                    XLimit::AtMost(a) => Some(a.clone()),
                    _ => None,
                },
            ),
        ],
    );
    lit("X can't be").then_keep(&limit).map(
        |l| Some(Static::X(l)),
        |x| match x {
            Static::X(l) => Some(l.clone()),
            _ => None,
        },
    )
}

/// "The flashback cost is equal to its mana cost".
fn keyword_cost() -> P<Static> {
    lit("the")
        .then_keep(&crate::selector::keyword_name())
        .skip(&lit("cost is equal to"))
        .then(&possessor())
        .skip(&lit("mana cost"))
        .map(
            |(keyword, of)| Some(Static::KeywordCost { keyword, of }),
            |x| match x {
                Static::KeywordCost { keyword, of } => Some((*keyword, of.clone())),
                _ => None,
            },
        )
}

/// Objects, players or bare sets joined by "and", "or" or "and/or" ("you and creatures you
/// control"); bare sets alone are one set with a type union.
pub fn party() -> P<Party> {
    // several bare sets are one set with a type union ("creatures and planeswalkers you control")
    let sets_only = |p: &Party| p.items.len() >= 2 && p.items.iter().all(|w| matches!(w, Who::Set(_)));
    crate::action::coord(&who()).map(move |p| (!sets_only(&p)).then_some(p), move |p| (!sets_only(p)).then(|| p.clone()))
}

/// The damage a prevention or redirection names: "the next 3 damage that would be dealt to any
/// target", "all combat damage that would be dealt this turn by creatures your opponents control",
/// "all damage that would be dealt to you this turn by sources you don't control", "that
/// damage", "1 of that damage". `end_turn`: "this turn" may end the phrase (a redirection's
/// subject; after "prevent" it is the act's duration instead).
pub fn prevention(end_turn: bool) -> P<Prevention> {
    let amount = alt(
        "prevented",
        vec![
            lit("the next").then_keep(&stat_amount()).map(
                |n| Some(Prevented::Next(n)),
                |p| match p {
                    Prevented::Next(n) => Some(n.clone()),
                    _ => None,
                },
            ),
            lit("all").to(Prevented::All),
        ],
    );
    let damage = amount.then(&damage_kind());
    let this_turn = |at: TurnAt| lit("this turn").to(Some(at));
    let none = || empty().to(None::<TurnAt>);
    let to = lit("to").then_keep(&party());
    let by = lit("by").then_keep(&party());
    // (to, by, "this turn" inside, either direction)
    type Whom = (Option<Party>, Option<Party>, Option<TurnAt>, bool);
    let mut whom: Vec<P<Whom>> = vec![
        empty().to((None, None, None, false)),
        to.map(|o| Some((Some(o), None, None, false)), |(t, b, at, either)| if !either && b.is_none() && at.is_none() { t.clone() } else { None }),
        by.map(|o| Some((None, Some(o), None, false)), |(t, b, at, either)| if !either && t.is_none() && at.is_none() { b.clone() } else { None }),
        lit("to and dealt by").then_keep(&party()).map(
            |o| Some((Some(o.clone()), Some(o), None, true)),
            |(t, b, at, either)| match (t, b, at, either) {
                (Some(t), Some(b), None, true) if t == b => Some(t.clone()),
                _ => None,
            },
        ),
        to.then(&alt("turn", vec![this_turn(TurnAt::BeforeBy), none()])).then(&by).map(
            |((t, at), b)| Some((Some(t), Some(b), at, false)),
            |(t, b, at, either)| match (t, b, at, either) {
                (Some(t), Some(b), None | Some(TurnAt::BeforeBy), false) => Some(((t.clone(), *at), b.clone())),
                _ => None,
            },
        ),
        // "dealt this turn to …", "dealt this turn by …"
        lit("this turn")
            .then_keep(&alt("to or by", vec![to.map(|o| Some((Some(o), None)), |(t, b)| if b.is_none() { t.clone() } else { None }), by.map(|o| Some((None, Some(o))), |(t, b)| if t.is_none() { b.clone() } else { None })]))
            .map(|(t, b)| Some((t, b, Some(TurnAt::AfterDealt), false)), |(t, b, at, either)| (!either && *at == Some(TurnAt::AfterDealt)).then(|| (t.clone(), b.clone()))),
    ];
    if end_turn {
        // "dealt to ~ this turn": the redirection's subject ends there
        let end = lit("this turn");
        whom.push(
            to.skip(&end).map(
                |t| Some((Some(t), None, Some(TurnAt::End), false)),
                |(t, b, at, either)| if !either && b.is_none() && *at == Some(TurnAt::End) { t.clone() } else { None },
            ),
        );
        whom.push(
            by.skip(&end).map(
                |b| Some((None, Some(b), Some(TurnAt::End), false)),
                |(t, b, at, either)| if !either && t.is_none() && *at == Some(TurnAt::End) { b.clone() } else { None },
            ),
        );
        whom.push(to.then(&by).skip(&end).map(
            |(t, b)| Some((Some(t), Some(b), Some(TurnAt::End), false)),
            |(t, b, at, either)| match (t, b, at, either) {
                (Some(t), Some(b), Some(TurnAt::End), false) => Some((t.clone(), b.clone())),
                _ => None,
            },
        ));
    }
    let dealt = damage.skip(&lit("that would be dealt")).then(&alt("dealt to or by", whom)).map(
        |((amount, kind), (to, by, this_turn, to_or_by))| Some(Prevention { amount, kind, to, by, this_turn, to_or_by }),
        |p: &Prevention| match p.amount {
            Prevented::Next(_) | Prevented::All => {
                Some(((p.amount.clone(), p.kind), (p.to.clone(), p.by.clone(), p.this_turn, p.to_or_by)))
            }
            _ => None,
        },
    );
    // "all combat damage target creature would deal": the "dealt by" wording
    let would_deal = damage
        .skip(&lit("that").opt().map(|_| Some(()), |()| Some(None)))
        .then(&party())
        .skip(&lit("would deal"))
        .then(&lit("to").then_keep(&party()).opt())
        .map(|(((amount, kind), by), to)| Some(Prevention { amount, kind, to, by: Some(by), to_or_by: false, this_turn: None }), |_| None);
    let that = |p: Prevented| Prevention { amount: p, kind: DamageKind::Any, to: None, by: None, to_or_by: false, this_turn: None };
    let of_that =
        |p: &Prevention| p.kind != DamageKind::Any || p.to.is_some() || p.by.is_some() || p.this_turn.is_some();
    let part = alt(
        "of that damage",
        vec![
            lit("all but").then_keep(&stat_amount()).map(
                |n| Some(Prevented::AllButOfThat(n)),
                |p| match p {
                    Prevented::AllButOfThat(n) => Some(n.clone()),
                    _ => None,
                },
            ),
            stat_amount().map(
                |n| Some(Prevented::OfThat(n)),
                |p| match p {
                    Prevented::OfThat(n) => Some(n.clone()),
                    _ => None,
                },
            ),
        ],
    )
    .skip(&lit("of that damage"))
    .map(move |p| Some(that(p)), move |p: &Prevention| (!of_that(p)).then(|| p.amount.clone()));
    alt(
        "prevention",
        vec![
            dealt,
            would_deal.alias(),
            lit("that damage").to(that(Prevented::That)),
            lit("half that damage, rounded down").to(that(Prevented::HalfThat { round_up: false })),
            lit("half that damage, rounded up").to(that(Prevented::HalfThat { round_up: true })),
            part,
        ],
    )
}

/// What a replacement does instead: instructions, or a passive "… are created/put … instead".
fn substitute(body: &P<Body>) -> P<Substitute> {
    let that_many = Amount::That { many: true };
    let tm = that_many.clone();
    let count = alt(
        "count",
        vec![
            lit("twice that many").to(Amount::Multiple(2, Box::new(that_many.clone()))),
            lit("three times that many").to(Amount::Multiple(3, Box::new(that_many.clone()))),
            lit("that many plus one").to(Amount::Plus(Box::new(tm), Box::new(Amount::Const(1)))),
        ],
    );
    // The regular imperative parser also reads this replacement phrase. Give the
    // exact effect-voice form one typed reading, without choosing a first parse.
    fn effect_token_phrase(b:&Body)->bool {
        let c=&b.first;
        matches!(&c.subject,Some(ir::Subject::Selector(s)) if s.quant==Quant::Definite(Ref::Anaphor(ir::Anaphor::It)) && s.set==ir::ObjectSet::of(ir::Domain::Object))
            && !c.may && !c.also && c.rest.is_empty() && b.rest.is_empty() && !c.first.may
            && c.first.mods==[ir::Modifier::Instead]
            && matches!(&c.first.action,Action::Create(tokens) if matches!(tokens.items.as_slice(),[ir::token::TokenSpec {count:Amount::Multiple(2|3,n),kind:ir::token::TokenKind::Those,tapped:false,combat:None,attached_to:None}] if matches!(**n,Amount::That {many:true})))
    }
    let instructions = body.map(
        |b| (!effect_token_phrase(&b)).then(||Substitute::Body(Box::new(b))),
        |s| match s {
            Substitute::Body(b) if !effect_token_phrase(b) => Some((**b).clone()),
            _ => None,
        },
    );
    let tokens = count.skip(&lit("of those tokens are created instead")).map(
        |n| Some(Substitute::TokensCreated { count:n,effect_voice:false }),
        |s| match s {
            Substitute::TokensCreated { count:n,effect_voice:false } => Some(n.clone()),
            _ => None,
        },
    );
    let effect_tokens = lit("it creates").then_keep(&count).skip(&lit("of those tokens instead")).map(
        |count| Some(Substitute::TokensCreated { count,effect_voice:true }),
        |s| match s { Substitute::TokensCreated { count,effect_voice:true } => Some(count.clone()), _ => None },
    );
    let plus = lit("those tokens plus")
        .then_keep(crate::token::token_spec())
        .skip(&lit("are created instead"))
        .map(
            |t| Some(Substitute::TokensPlus { token:t, active:false }),
            |s| match s {
                Substitute::TokensPlus { token:t, active:false } => Some(t.clone()),
                _ => None,
            },
        );
    let active_plus = lit("create those tokens plus")
        .then_keep(&crate::token::additional_spec()).map(
            |token| Some(Substitute::TokensPlus { token, active:true }),
            |s| match s { Substitute::TokensPlus {token,active:true} => Some(token.clone()), _ => None });
    let counters = count.then(&counter_kind()).skip(&lit("counters are put on")).then(&selectors().selector).skip(&lit("instead")).map(
        |((count, kind), on)| Some(Substitute::CountersPut { count, kinds: CounterKinds::Named(kind), on, voice: CounterVoice::Passive }),
        |s| match s {
            Substitute::CountersPut { count, kinds: CounterKinds::Named(kind), on, voice: CounterVoice::Passive } => Some(((count.clone(), kind.clone()), on.clone())),
            _ => None,
        },
    );
    let inventory = |prefix: P<()>, wording: &str, voice| {
        prefix.then_keep(&count).skip(&lit(wording)).then(&selectors().selector).skip(&lit("instead")).map(
            move |(count, on)| Some(Substitute::CountersPut { count, kinds: CounterKinds::EachProposed, on, voice }),
            move |s| match s {
                Substitute::CountersPut { count, kinds: CounterKinds::EachProposed, on, voice: v } if *v == voice => Some((count.clone(), on.clone())),
                _ => None,
            },
        )
    };
    // "you get that many plus one of each of those kinds of counters instead": counters on a
    // player (122.1) are said with "get"; no one is named as placing them, as in the passive
    let player = |s: &Selector| s.set.domain == ir::Domain::Player;
    let recipient = alt("player getting counters", vec![
        lit("you get").to(Selector::you()),
        selectors().singular.skip(&lit("gets")).map(move |s| player(&s).then_some(s), move |s| (player(s) && *s != Selector::you()).then(|| s.clone())),
    ]);
    let gets = recipient.then(&count).skip(&lit("of each of those kinds of counters instead")).map(
        move |(on, count)| player(&on).then_some(Substitute::CountersPut { count, kinds: CounterKinds::EachProposed, on, voice: CounterVoice::Passive }),
        move |s| match s {
            Substitute::CountersPut { count, kinds: CounterKinds::EachProposed, on, voice: CounterVoice::Passive } if player(on) => Some((on.clone(), count.clone())),
            _ => None,
        },
    );
    let each_passive = inventory(empty(), "of each of those kinds of counters are put on", CounterVoice::Passive);
    let each_active = inventory(lit("put"), "of each of those kinds of counters on", CounterVoice::Imperative);
    let effect = inventory(lit("it puts"), "of those counters on", CounterVoice::Cause);
    alt("substitute", vec![instructions, tokens, effect_tokens, plus, active_plus, counters, gets, each_passive, each_active, effect])
}

/// "it produces {B} instead of any other type and amount", "it produces twice as much of that mana
/// instead" (106.12b).
fn produces() -> P<Substitute> {
    use ir::statics::{Displaced, Produced, Produces};
    let that_mana = |n: u32| Amount::Multiple(n, Box::new(Amount::That { many: false }));
    let times = alt(
        "multiplier",
        vec![
            lit("twice").to(2),
            crate::words::count_atom().skip(&lit("times")).map(
                |a| match a {
                    Amount::Const(n) if n > 2 => Some(n),
                    _ => None,
                },
                |n| (*n > 2).then(|| Amount::Const(*n)),
            ),
        ],
    );
    let as_much = times.skip(&lit("as much of that mana")).map(
        move |n| Some(Produced::AsMuch(that_mana(n))),
        |p| match p {
            Produced::AsMuch(Amount::Multiple(n, _)) => Some(*n),
            _ => None,
        },
    );
    let exactly = crate::mana::mana_spec().map(
        |m| matches!(m, ir::mana::ManaSpec::Symbols(_)).then_some(Produced::Exactly(m)),
        |p| match p {
            Produced::Exactly(m) => Some(m.clone()),
            _ => None,
        },
    );
    let mana = alt(
        "produced mana",
        vec![
            exactly,
            lit("colorless mana").to(Produced::Colorless),
            lit("mana of a color of your choice").to(Produced::OfYourChoice),
            // "one" says nothing more: each mana is of a color of its controller's choice
            lit("one mana of a color of your choice").to(Produced::OfYourChoice).alias(),
            as_much,
        ],
    );
    let color = crate::kernel::table("color", ir::Color::ALL.iter().map(|c| (c.word().to_string(), *c)).collect());
    let displaced = alt(
        "displaced mana",
        vec![
            lit("instead of any other type and amount").to(Displaced::AnyOtherTypeAndAmount),
            lit("instead of any other type").to(Displaced::AnyOtherType),
            lit("instead of").then_keep(&color).skip(&lit("mana")).map(
                |c| Some(Displaced::Color(c)),
                |d| match d {
                    Displaced::Color(c) => Some(*c),
                    _ => None,
                },
            ),
            lit("instead").to(Displaced::Nothing),
        ],
    );
    lit("it produces").then_keep(&mana).then(&displaced).map(
        |(mana, instead_of)| {
            // multiplying keeps the type, so there is nothing it replaces
            let ok = matches!(mana, Produced::AsMuch(_)) == (instead_of == Displaced::Nothing);
            ok.then_some(Substitute::Produces(Produces { mana, instead_of }))
        },
        |s| match s {
            Substitute::Produces(p) => Some((p.mana.clone(), p.instead_of.clone())),
            _ => None,
        },
    )
}

/// "that damage is dealt to ~ instead", "it deals that damage to target creature instead" (614.9):
/// the damage is the same damage and keeps its source, so the active wording, which names the
/// source again, is an alias of the passive one.
fn redirect_to() -> P<Substitute> {
    let to = party();
    let passive = lit("that damage is dealt to").then_keep(&to).skip(&lit("instead"));
    let anaphor = |s: &Selector| matches!(s.quant, Quant::Definite(Ref::Anaphor(_)));
    let source = selectors().selector.map(move |s| anaphor(&s).then_some(s), move |s: &Selector| anaphor(s).then(|| s.clone()));
    let active = source
        .skip(&lit("deals that damage to"))
        .then(&to)
        .skip(&lit("instead"))
        .map(|(_, to)| Some(to), |_| None)
        .alias();
    alt("redirect", vec![passive, active]).map(
        |to| Some(Substitute::Redirect(to)),
        |s| match s {
            Substitute::Redirect(to) => Some(to.clone()),
            _ => None,
        },
    )
}

/// "that player searches the top four cards of that library instead" (701.23): the one who would
/// search searches part of the library they would search. "That library" is the library
/// searched, whoever owns it.
fn searches_part() -> P<Substitute> {
    let part = |s: &Selector| {
        let zone = s.set.zone.as_ref()?;
        let that = Selector::new(Quant::Definite(Ref::ThatZone), ObjectSet::of(Domain::Player));
        let plain = ObjectSet { zone: Some(zone.clone()), ..ObjectSet::of(Domain::Card) };
        (s.quant == Quant::The && s.set == plain && zone.zones == ir::Coord::one(Zone::Library)
            && zone.owner.as_deref() == Some(&that) && matches!(zone.slice, Some(Slice::Top(_))))
            .then(|| zone.clone())
    };
    lit("that player searches").then_keep(&selectors().selector).skip(&lit("instead")).map(
        move |s| part(&s).map(Substitute::Searches),
        |w| match w {
            Substitute::Searches(zone) => Some(Selector::new(Quant::The, ObjectSet { zone: Some(zone.clone()), ..ObjectSet::of(Domain::Card) })),
            _ => None,
        },
    )
}

/// An event of searching a library: what [`searches_part`] replaces.
fn search_event() -> P<EventPattern> {
    let search = |p: &EventPattern| !p.actions.is_empty() && p.actions.iter().all(|a| matches!(a, EventAction::Searches { .. }));
    crate::event::would().map(move |p| search(&p).then_some(p), move |p: &EventPattern| search(p).then(|| p.clone()))
}

/// An event of damage being dealt: what a redirection redirects.
fn damage_event() -> P<EventPattern> {
    let damage = |p: &EventPattern| {
        !p.actions.is_empty()
            && p.actions.iter().all(|a| matches!(a, EventAction::DealsDamage { .. } | EventAction::IsDealtDamage { excess: false, .. }))
    };
    crate::event::would().map(move |p| damage(&p).then_some(p), move |p: &EventPattern| damage(p).then(|| p.clone()))
}

/// The event 106.12b replaces: a permanent "is tapped for mana", "you tap a permanent for mana".
/// The rule states it without "would".
fn tapped_for_mana() -> P<EventPattern> {
    let tap = |p: &EventPattern| {
        !p.actions.is_empty() && p.actions.iter().all(|a| matches!(a.reading(), Some((Verb::TapForMana, _))))
    };
    crate::event::events().pattern.map(move |p| tap(&p).then_some(p), move |p: &EventPattern| tap(p).then(|| p.clone()))
}

/// "If that creature would die this turn, exile it instead", "The next time a source of your
/// choice would deal damage to you this turn, prevent that damage", "If a land is tapped for mana,
/// it produces {B} instead of any other type".
fn replacement(body: &P<Body>) -> P<Static> {
    let would = replacement_of(&crate::event::would(), &substitute(body), false);
    let tapped = replacement_of(&tapped_for_mana(), &produces(), true);
    let redirected = replacement_of(&damage_event(), &redirect_to(), true);
    let searched = replacement_of(&search_event(), &searches_part(), true);
    let caused = replacement_of(&crate::event::caused(), &substitute(body), false);
    alt("replacement", vec![would, tapped, redirected, searched, caused])
}

/// "Damage that would reduce your life total to less than 1 reduces it to 1 instead": a
/// replacement (614.1a) of the life loss damage causes (120.3a), with the player whose life
/// total it is (the clause's subject). The floor is said twice and must be one number.
pub(crate) fn damage_life_floor() -> P<(Selector, StaticAct)> {
    lit("damage that would reduce")
        .then_keep(&selectors().possessive)
        .skip(&lit("life total to less than"))
        .then(&stat_amount())
        .skip(&lit("reduces it to"))
        .then(&stat_amount())
        .skip(&lit("instead"))
        .map(
            |((player, floor), again)| (floor == again).then_some((player, StaticAct::DamageLifeFloor { floor })),
            |(player, act)| match act {
                StaticAct::DamageLifeFloor { floor } => Some(((player.clone(), floor.clone()), floor.clone())),
                _ => None,
            },
        )
}

/// A replacement of the events `event` says, with what happens instead; `bare`: the substitute
/// carries its own "instead".
fn replacement_of(event: &P<EventPattern>, with: &P<Substitute>, bare: bool) -> P<Static> {
    let head = alt("replacement", vec![lit("if").to(false), lit("the next time").to(true)]);
    let while_ = lit("while").then_keep(&conditions().condition).opt();
    let instead = lit("instead").opt().map(
        move |i| (!(bare && i.is_some())).then_some(i.is_some()),
        move |b| (!(bare && *b)).then(|| b.then_some(())),
    );
    // "you may instead create that many tokens": what "instead you may create" says, "instead"
    // after the optional subject (Abundance, Jinnie Fay), and printed as that
    let may_instead = lit("you may instead").then_keep(with).map(
        move |with| match with {
            Substitute::Body(mut b) if !bare && b.first.subject.is_none() && !b.first.may && !b.first.also && !b.first.first.may => {
                b.first.subject = Some(ir::effect::Subject::Selector(Selector::you()));
                b.first.may = true;
                Some((true, Substitute::Body(b)))
            }
            _ => None,
        },
        |_| None,
    );
    let with = &alt("instead", vec![instead.then(with), may_instead.alias()]);
    // "The first time you would draw a card each turn, instead …": the first such event of each
    // turn, or of each of the turns the scope names
    let first_time = lit("the first time")
        .then_keep(event)
        .then(&crate::event::scope())
        .skip(&lit(","))
        .then(with)
        .map(
            |((mut event, during), (instead_first, with))| {
                if event.nth_each_turn.is_some() || event.during.is_some() {
                    return None;
                }
                event.nth_each_turn = Some(1);
                event.during = during;
                Some(Static::Replacement(Box::new(Replacement { next_time: false, event: ir::Coord::one(event), while_: None, this_turn: false, with, instead_first })))
            },
            |x| match x {
                Static::Replacement(r) if !r.next_time && r.while_.is_none() && !r.this_turn => {
                    let mut event = r.single_event()?.clone();
                    if event.nth_each_turn != Some(1) { return None; }
                    event.nth_each_turn = None;
                    let during = event.during.take();
                    Some(((event, during), (r.instead_first, r.with.clone())))
                }
                _ => None,
            },
        );
    // Only "or" relates complete replacement-event alternatives. The first-time ordinal
    // above still takes one pattern: assigning it independently to each arm changes scope.
    let one = event.map(|event| Some(ir::Coord::one(event)), |events: &ir::Coord<EventPattern>| {
        match events.items.as_slice() { [event] if events.conj == ir::Conj::And => Some(event.clone()), _ => None }
    });
    let alternatives = event.list("or").map(
        |items| Some(ir::Coord { items, conj: ir::Conj::Or }),
        |events: &ir::Coord<EventPattern>| (events.conj == ir::Conj::Or && events.items.len() >= 2).then(|| events.items.clone()),
    );
    let events = alt("replacement events", vec![one, alternatives]);
    let said_if = head.then(&events)
        .then(&while_)
        .then(&lit("this turn").opt())
        .skip(&lit(","))
        .then(with)
        .map(
            |((((next_time, event), while_), this_turn), (instead_first, with))| {
                Some(Static::Replacement(Box::new(Replacement {
                    next_time,
                    event,
                    while_,
                    this_turn: this_turn.is_some(),
                    with,
                    instead_first,
                })))
            },
            |x| match x {
                Static::Replacement(r) => Some((
                    (((r.next_time, r.event.clone()), r.while_.clone()), r.this_turn.then_some(())),
                    (r.instead_first, r.with.clone()),
                )),
                _ => None,
            },
        );
    // "If you would create a Clue, Food, or Treasure token, instead create one of each.": what
    // the Academy Manufactor rulings of 2021-06-18 say it does, "if you would create one or more
    // Clue, Food, or Treasure tokens, instead create that many Clue tokens, that many Food
    // tokens, and that many Treasure tokens"; printed as that
    let one_of_each = lit("if").then_keep(event).skip(&lit(", instead create one of each")).map(
        move |mut event| {
            if bare { return None; }
            let [EventAction::Deed(ir::Deed { verb: Verb::Create, passive: false, args })] = event.actions.as_mut_slice() else { return None };
            let [(ir::Role::What, ir::Arg::Noun(made))] = args.as_mut_slice() else { return None };
            let ir::Filter::All(parts) = &made.set.filter else { return None };
            let [ir::Filter::Any(kinds), ir::Filter::Atom(ir::Atom::Status(ir::Status::Token))] = parts.as_slice() else { return None };
            let specs = kinds.iter().map(|kind| match kind {
                ir::Filter::Atom(ir::Atom::Subtype(t)) => ir::token::Predefined::ALL.iter().find(|p| p.name() == t.name()).map(|p| ir::token::TokenSpec {
                    count: Amount::That { many: true }, tapped: false, kind: ir::token::TokenKind::Predefined(*p), combat: None, attached_to: None,
                }),
                _ => None,
            }).collect::<Option<Vec<_>>>()?;
            if specs.len() < 2 || made.quant != Quant::Indefinite(ir::Count::Exactly(Amount::Const(1))) { return None; }
            made.quant = Quant::Indefinite(ir::Count::AtLeast(Amount::Const(1)));
            let create = ir::effect::Act { action: ir::Action::Create(ir::Coord { items: specs, conj: ir::Conj::And }), mods: Vec::new(), may: false };
            let body = ir::effect::Body { first: ir::effect::Clause { subject: None, may: false, also: false, first: create, rest: Vec::new() }, rest: Vec::new() };
            Some(Static::Replacement(Box::new(Replacement { next_time: false, event: ir::Coord::one(event), while_: None, this_turn: false, with: Substitute::Body(Box::new(body)), instead_first: true })))
        },
        |_| None,
    ).alias();
    alt("replacement", vec![said_if, first_time, one_of_each])
}

/// "If a triggered ability of an Ally you control triggers, that ability triggers an additional
/// time", "If a permanent entering causes a triggered ability of a permanent you control to
/// trigger, …" (603.2d).
fn triggers_additional() -> P<Static> {
    use ir::statics::TriggerCause;
    let s = selectors();
    let of = lit("a triggered ability of").then_keep(&s.selector);
    // "a creature dying", "a permanent entering": the event clause with the gerund
    let happening = s.selector.then(&crate::event::action(Form::Gerund, false)).map(
        |(what, action)| {
            let p = EventPattern::new(vec![what], vec![action]);
            p.typed().then_some(p)
        },
        |p: &EventPattern| match (p.subjects.as_slice(), p.actions.as_slice()) {
            ([what], [action]) if p.nth_each_turn.is_none() && p.during.is_none() && p.condition.is_none() && p.cause.is_none() => {
                Some((what.clone(), action.clone()))
            }
            _ => None,
        },
    );
    let cause = alt(
        "trigger cause",
        vec![
            of.skip(&lit("triggers")).map(
                |x| Some(TriggerCause::AbilityOf(x)),
                |c| match c {
                    TriggerCause::AbilityOf(x) => Some(x.clone()),
                    _ => None,
                },
            ),
            happening.skip(&lit("causes")).then(&of).skip(&lit("to trigger")).map(
                |(event, of)| Some(TriggerCause::Event { event, of }),
                |c| match c {
                    TriggerCause::Event { event, of } => Some((event.clone(), of.clone())),
                    _ => None,
                },
            ),
        ],
    );
    let it = alt("that ability", vec![lit("that ability").to(false), lit("it").to(true)]);
    lit("if").then_keep(&cause).skip(&lit(",")).then(&it).skip(&lit("triggers an additional time")).map(
        |(cause, it)| Some(Static::TriggersAdditional { cause, it }),
        |x| match x {
            Static::TriggersAdditional { cause, it } => Some((cause.clone(), *it)),
            _ => None,
        },
    )
}

#[cfg(test)]
mod tests {
    use crate::effect::effects;
    use crate::kernel::Outcome;
    use oracle::token::tokenize;

    fn round_trip(texts: &[&str]) {
        let s = &effects().sentence;
        for text in texts {
            let toks = tokenize(text);
            match s.parse_all(&toks) {
                Outcome::Accepted { value, .. } => {
                    assert_eq!(s.print_text(&value, true).as_deref(), Some(*text), "{value:#?}")
                }
                other => panic!("{text}: {other:#?}"),
            }
        }
    }

    #[test]
    fn r701_7_token_creation_multiplier_voice() {
        round_trip(&[
            "If one or more tokens would be created under your control, twice that many of those tokens are created instead",
            "If one or more creature tokens would be created under your control, three times that many of those tokens are created instead",
            "If an effect would create one or more tokens under your control, it creates twice that many of those tokens instead",
        ]);
    }

    /// 614.1a/614.16: generic inventory transformations retain actor versus effect cause.
    #[test]
    fn r614_16_generic_counter_inventory_replacements() {
        round_trip(&[
            "If one or more counters would be put on a creature, Spacecraft, or Planet you control, twice that many of each of those kinds of counters are put on it instead",
            "If you would put one or more counters on a permanent you control, put that many plus one of each of those kinds of counters on that permanent instead",
            "If you would put one or more counters on a permanent or player, put twice that many of each of those kinds of counters on that permanent or player instead",
            "If an effect would put one or more counters on a permanent you control, it puts twice that many of those counters on that permanent instead",
        ]);
    }

    /// 601.2a: a cast is from a zone; "from anywhere other than your hand" is from every zone
    /// but that one, a different reading from "from your hand".
    #[test]
    fn r601_2a_spells_cast_from_anywhere_other_than_a_zone() {
        let s = &effects().sentence;
        let caster = |text: &str| match s.parse_all(&tokenize(text)) {
            Outcome::Accepted { value, .. } => format!("{value:?}"),
            other => panic!("{text}: {other:#?}"),
        };
        let other = caster("Spells you cast from anywhere other than your hand cost {1} less to cast");
        let from = caster("Spells you cast from your hand cost {1} less to cast");
        assert!(other.contains("other_than: true") && from.contains("other_than: false"));
        assert!(matches!(s.parse_all(&tokenize("Spells you cast from anywhere other than cost {1} less to cast")), Outcome::Declined { .. }));
    }

    #[test]
    fn r613_1d_losing_a_class_of_types() {
        round_trip(&[
            "Target creature loses all creature types until end of turn",
            "Enchanted permanent is an enchantment and loses all other card types",
            "Enchanted land loses all land types and abilities",
            "Enchanted creature loses all other abilities, card types, and creature types",
            "Creatures target player controls get -2/-0 and lose all creature types until end of turn",
        ]);
        // a class of types is not a named ability, and "other" comes right after "all"
        let s = &effects().sentence;
        for t in ["Target creature loses creature types", "Target creature loses all and other abilities"] {
            assert!(!matches!(s.parse_all(&tokenize(t)), Outcome::Accepted { .. }), "{t}");
        }
    }

    #[test]
    fn static_acts_round_trip() {
        round_trip(&[
            "Equipped creature gets +2/+2 and has trample",
            "Creatures you control have flying",
            "Enchanted creature gets +2/+2 and has first strike and \"{T}: Add {G}.\"",
            "Target creature loses flying until end of turn",
            "Enchanted creature loses all abilities",
            "~ enters tapped",
            "~ enters with two +1/+1 counters on it",
            "~ enters with an additional +1/+1 counter on it",
            "~ enters tapped unless you control two or more basic lands",
            "~ costs {1} less to cast for each creature you control",
            "Artifact spells you cast cost {1} less to cast",
            "Spells you cast from anywhere other than your hand cost {1} less to cast",
            "Spells you cast from your graveyard cost {1} less to cast",
            "Each spell that would cost less than three mana to cast costs three mana to cast",
            "As long as ~ is untapped, each spell that would cost less than three mana to cast costs three mana to cast",
            "~ attacks each combat if able",
            "Target creature blocks ~ this turn if able",
            "~ can block only creatures with flying",
            "~ can block an additional creature each combat",
            "~ can't be blocked by creatures with power 2 or less",
            "Your opponents can't cast spells this turn",
            "Players can't gain life this turn",
            "~ can attack this turn as though it didn't have defender",
            "If you do, ~ assigns no combat damage this turn",
            "You have no maximum hand size",
        ]);
    }

    #[test]
    fn r601_2f_minimum_cost_is_one_quantity_and_one_cost_kind() {
        let sentence = &effects().sentence;
        for text in [
            "Each spell that would cost less than three mana to cast costs two mana to cast",
            "Each spell that would cost less than three mana to cast costs three mana to activate",
        ] {
            assert!(!matches!(sentence.parse_all(&tokenize(text)), Outcome::Accepted { .. }), "{text}");
        }
    }

    #[test]
    fn static_sentences_round_trip() {
        round_trip(&[
            "As long as you control a Mountain, ~ has menace",
            "~ has flying as long as you control a blue creature",
            "During your turn, ~ has first strike",
            "During turns other than yours, ~ has hexproof",
            "As ~ enters, choose a color",
            "~'s power and toughness are each equal to the number of creatures you control",
            "~'s power is equal to the number of creatures you control",
            "You may sacrifice a Mountain rather than pay ~'s mana cost",
            "If you control a Plains, you may tap an untapped creature you control rather than pay ~'s mana cost",
            "If that creature would die this turn, exile it instead",
            "If ~ would be put into a graveyard from anywhere, exile it instead",
            "The next time a source of your choice would deal damage to you this turn, prevent that damage",
            "If it would leave the battlefield, exile it instead of putting it anywhere else",
            "You may have ~ enter as a copy of a creature you control, except it has haste",
            "Damage can't be prevented this turn",
            "The damage can't be prevented",
            "X can't be 0",
            "The flashback cost is equal to its mana cost",
            "You may cast that card for as long as it remains exiled, and mana of any type can be spent to cast that spell",
            "Enchanted creature can't attack or block, and its activated abilities can't be activated",
            "Activated abilities of creatures you control cost {2} less to activate",
            "All creatures able to block ~ do so",
            "Each opponent's maximum hand size is reduced by one",
            "This mana can't be spent to cast nonartifact spells",
        ]);
    }

    /// Reads this text and prints it as `printed` (an alias spelling reads as the same meaning).
    fn reads_as(text: &str, printed: &str) {
        let s = &effects().sentence;
        match s.parse_all(&tokenize(text)) {
            Outcome::Accepted { value, .. } => assert_eq!(s.print_text(&value, true).as_deref(), Some(printed)),
            other => panic!("{text}: {other:#?}"),
        }
    }

    fn rejects(texts: &[&str]) {
        let s = &effects().sentence;
        for text in texts {
            assert!(!matches!(s.parse_all(&tokenize(text)), Outcome::Accepted { .. }), "{text} should not read");
        }
    }

    #[test]
    fn r101_2_source_qualified_causal_prohibitions() {
        reads_as("Spells and abilities your opponents control can't cause you to sacrifice permanents",
            "Spells or abilities your opponents control can't cause you to sacrifice permanents");
        round_trip(&[
            "Spells can't cause you to sacrifice creatures",
            "Abilities your opponents control can't cause you to discard cards",
        ]);
    }

    #[test]
    fn r614_1c_enters_with_parts() {
        round_trip(&[
            "~ enters with flying",
            "If ~ was kicked, it enters with two +1/+1 counters on it and with flying",
            "If ~ was kicked, it enters with a +1/+1 counter on it and with \"Pay 3 life: Regenerate ~.\"",
            "~ enters tapped with three charge counters on it",
        ]);
        reads_as("~ enters tapped and with three coin counters on it", "~ enters tapped with three coin counters on it");
        // an ability part is joined with "and"; "enters with" takes abilities, not a creature type
        rejects(&["~ enters tapped with flying", "~ enters with Elf"]);
    }

    #[test]
    fn r106_12b_mana_production_replacement() {
        round_trip(&[
            "If a land is tapped for mana, it produces {B} instead of any other type and amount",
            "If a basic land you control is tapped for mana, it produces mana of a color of your choice instead of any other type",
            "If you tap a permanent for mana, it produces twice as much of that mana instead",
            "If you tap a basic land for mana, it produces three times as much of that mana instead",
            "If target Plains is tapped for mana, it produces colorless mana instead of white mana",
            "Until end of turn, if a player taps a nonbasic land for mana, it produces colorless mana instead of any other type",
        ]);
        reads_as(
            "If a land is tapped for mana, it produces one mana of a color of your choice instead of any other type",
            "If a land is tapped for mana, it produces mana of a color of your choice instead of any other type",
        );
        // the produced mana belongs to the tapping event, and doubling keeps the type
        rejects(&[
            "If a land is tapped for mana, exile it instead",
            "If a creature would die, it produces {B} instead of any other type",
            "If a land is tapped for mana, it produces twice as much of that mana instead of any other type",
            "If a land is tapped for mana, instead it produces {B} instead of any other type",
        ]);
    }

    #[test]
    fn r614_9_redirection() {
        round_trip(&[
            "The next time a source of your choice would deal damage to you this turn, that damage is dealt to ~ instead",
            "The next time damage would be dealt to ~ this turn, that damage is dealt to any target instead",
        ]);
        // the active wording names the source again; the damage and its source are the same
        reads_as(
            "The next time ~ would deal combat damage to an opponent this turn, it deals that damage to target creature instead",
            "The next time ~ would deal combat damage to an opponent this turn, that damage is dealt to target creature instead",
        );
        rejects(&[
            "If a creature would die, that damage is dealt to ~ instead",
            "The next time damage would be dealt to ~ this turn, target player deals that damage to you instead",
        ]);
    }

    #[test]
    fn r615_12_unpreventable_damage() {
        round_trip(&[
            "~ can't be countered and the damage can't be prevented",
            "Damage that would be dealt to that creature this turn can't be prevented or dealt instead to another permanent or player",
            "If you attacked this turn, instead ~ deals 5 damage to that permanent or player and the damage can't be prevented",
            "You may play that card, and mana of any type can be spent to cast it",
        ]);
        rejects(&["~ can't be countered and the creature can't be prevented", "Damage can't be prevented or dealt to you"]);
    }

    #[test]
    fn r609_4_as_though_an_ability() {
        round_trip(&[
            "~ can attack as though it didn't have defender",
            "~ can attack this turn as though it didn't have defender",
            "Enchanted creature can attack as though it had haste",
            "Creatures you control can attack as though they didn't have defender",
            "~ can block creatures with shadow as though they didn't have shadow",
            "~ can block creatures with shadow as though it had shadow",
            "~ can block Dragons as though it had reach",
            "Creatures with forestwalk can be blocked as though they didn't have forestwalk",
            "~ can be blocked as though it didn't have swampwalk",
            "The next spell you cast this turn can be cast as though it had flash",
            "Tapped creatures you control can block as though they were untapped",
        ]);
        // the pronoun is the object the ability is about; untapped is a blocker's condition
        rejects(&[
            "~ can block creatures with shadow as though it didn't have shadow",
            "~ can attack as though they had haste",
            "~ can attack as though it was untapped",
            "~ can't attack as though it had haste",
            "~ can be put onto the battlefield as though it had flash",
        ]);
    }

    #[test]
    fn r118_8_r118_9_cast_by_paying() {
        round_trip(&[
            "You may cast ~ from your graveyard by discarding a card in addition to paying its other costs",
            "You may cast ~ from your graveyard by paying 3 life and discarding a card in addition to paying its other costs",
            "You may cast ~ from your graveyard by paying {B}{B} and sacrificing two creatures rather than paying its mana cost",
            "You may cast artifact spells from your graveyard by paying 3 life in addition to paying their other costs",
            "You may cast creature spells with mana value 3 or less by paying {E} rather than paying their mana costs",
            "You may cast spells from the top of your library by removing a counter from a creature you control in addition to paying their other costs",
            "You may cast ~ as though it had flash by tapping three untapped creatures you control with flying in addition to paying its other costs",
            "If you cast a spell this way, pay life equal to its mana value rather than pay its mana cost",
        ]);
        // only a cast is paid for this way; an alternative cost not about the spell cast is optional
        rejects(&[
            "Draw a card by discarding a card in addition to paying its other costs",
            "You may cast artifact spells this turn",
            "Pay {1} rather than pay ~'s mana cost",
            "It's the chosen color by paying {B} rather than paying its mana cost",
            "Sacrifice a creature rather than pay its mana cost",
        ]);
    }

    #[test]
    fn prevention_direction_is_not_inferred_from_equal_parties() {
        let parser = super::prevention(false);
        let parse = |text: &str| match parser.parse_all(&tokenize(text)) {
            Outcome::Accepted { value, .. } => value,
            other => panic!("{text}: {other:#?}"),
        };
        let either = parse("all combat damage that would be dealt to and dealt by that creature");
        let both = parse("all combat damage that would be dealt to that creature by that creature");
        assert_eq!(either.to, both.to);
        assert_eq!(either.by, both.by);
        assert!(either.to_or_by);
        assert!(!both.to_or_by);
        assert_ne!(either, both);
    }

    #[test]
    fn prevention_round_trip() {
        round_trip(&[
            "Prevent all combat damage that would be dealt this turn",
            "Prevent all combat damage that would be dealt by target creature this turn",
            "Prevent all damage that would be dealt to you this turn by sources you don't control",
            "Prevent all damage that would be dealt this turn to creatures you control",
            "Prevent all damage that would be dealt to you and creatures you control this turn",
            "Prevent all combat damage that would be dealt to and dealt by that creature this turn",
            "The next 1 damage that would be dealt to ~ this turn is dealt to target creature you control instead",
        ]);
    }

    #[test]
    fn more_static_acts_round_trip() {
        round_trip(&[
            "Enchanted land is a 6/4 green Elemental creature",
            "It's still a land",
            "~ isn't a creature",
            "~ loses this ability",
            "This ability costs {1} less to activate for each Shrine you control",
            "You may look at the top card of your library any time",
            "You may cast creature spells this turn as though they had flash",
            "You may have ~ assign its combat damage as though it weren't blocked",
            "Until end of turn, you may spend white mana as though it were mana of any color",
            "You may choose not to untap ~ during your untap step",
            "Each player may play an additional land on each of their turns",
            "You control enchanted creature",
            "Each player can't cast more than one spell each turn",
            "~ can't be copied",
            "White creatures get an additional +1/+1 as long as there are seven or more cards in your graveyard",
            "Each creature you control assigns combat damage equal to its toughness rather than its power",
            "~ can be your commander",
            "Skip your draw step",
            "Skip your next two turns",
            "Play with the top card of your library revealed",
            "~ can't attack or block alone",
            "You may play lands and cast spells from your graveyard",
            "Equip abilities you activate cost {1} less to activate",
            "~ has all activated abilities of all creature cards exiled with ~",
            "Lands you control enter untapped",
            "Untap all permanents you control during each other player's untap step",
            "You can't lose the game and your opponents can't win the game",
            "Enchanted creature can't be the target of spells or abilities your opponents control",
            "~ attacks or blocks each combat if able",
            "That creature can block up to two additional creatures this turn",
            "~ can't be blocked this turn except by creatures with haste",
            "Creatures you control are Equipment in addition to their other types",
            "You don't lose the game for having 0 or less life",
            "Players can't untap more than one land during their untap steps",
            "Each player can't draw more than one card each turn",
            "You may cast that card for as long as it remains exiled, and you may spend colorless mana as though it were mana of any color to cast that spell",
        ]);
    }

    /// Skipping is a replacement of the skipper's own step, phase or turn with nothing (500.11,
    /// 614.10): "their" after a third-person subject, "your" after "you" and in an imperative;
    /// never another player's.
    #[test]
    fn r614_10_what_is_skipped_is_the_skippers_own() {
        round_trip(&[
            "Target player skips their next draw step",
            "Each player skips their untap step",
            "That player skips their next combat phase",
            "Target player skips their next combat phase this turn",
            "Target opponent skips their next X turns",
            "You skip your next untap step",
            "Skip your next turn",
            "Target player skips all combat phases of their next turn",
            "Skip all combat phases of your next turn",
        ]);
        rejects(&[
            "Target player skips your next draw step",
            "Target player skips all combat phases of your next turn",
            "Target player skips all combat phases of their turn",
            "Target opponent skips your next turn",
            "Skip their untap step",
            "You skip their next turn",
        ]);
    }

    #[test]
    fn more_static_sentences_round_trip() {
        round_trip(&[
            "~ can't attack unless defending player controls an Island",
            "~ enters tapped unless you control two or more basic lands",
            "No more than one creature can attack each combat",
            "Your opponents can't cast spells during your turn",
            "If ~ would be destroyed, regenerate it",
            "If a creature an opponent controls would die, instead exile it",
            "All combat damage that would be dealt to you by unblocked creatures this turn is dealt to ~ instead",
            "If you would gain life, you gain twice that much life instead",
            "If a source you control would deal damage to a permanent or player, it deals double that damage to that permanent or player instead",
            "If enchanted creature would deal combat damage to a permanent or player, it deals double that damage instead",
            "If one or more tokens would be created under your control, twice that many of those tokens are created instead",
            "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead",
            "The \"legend rule\" doesn't apply to permanents you control",
            "You may pay {W}{U}{B}{R}{G} rather than pay the mana cost for spells you cast",
            "Loyalty abilities of planeswalkers your opponents control cost {1} more to activate",
            "This effect can't reduce the mana in that cost to less than one mana",
            "Prevent all noncombat damage that would be dealt to creatures you control",
            "Exile it instead of putting it into a graveyard as it resolves",
            "~ can't be blocked this combat",
            "You have no maximum hand size for the rest of the game",
            "If a player would begin an extra turn, that player skips that turn instead",
        ]);
    }
}
