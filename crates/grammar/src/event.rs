//! Event patterns and trigger conditions.
//!
//! ```text
//! trigger  := one ("and" one)*                           "When ~ enters and whenever you attack"
//! one      := ("when" | "whenever") pattern | "at the beginning of" step
//! pattern  := clause ["for the" ORDINAL "time each turn"] ["during" (possessive "turn" | step | "combat")]
//! clause   := subjects(3sg) actions(3sg) | subject(pl) actions(base) | "you" actions(base)
//!           | "you cast your" ORDINAL "spell each turn" | counters "is/are put on" selector
//! subjects := selector | selector "or" selector …        "~ or another Ally you control"
//! actions  := action | action "or" action …              "enters or attacks"
//! ```
//!
//! Verbs agree with the subject; the past forms serve history conditions ("if a creature died
//! this turn"). Literals are lowercase: the kernel matches them capitalized at a sentence start.

use crate::kernel::{P, Val, alt, empty, lit, table};
use crate::selector::{adjective_group, counter_kind, is_plural, selectors};
use crate::words::{STATUS_WORDS, digits, stat_amount};
use ir::event::{Arg, CounterNth, Deed, During, Quantity, Tense, Time, ZoneArg};
use ir::lexicon::{Inflection, Role, Verb};
use ir::selector::{Anaphor, Status};
use ir::{
    Condition, Domain, ObjectSet, Predicate, Ref, Amount, Count, DamageKind, EventAction, EventCause, EventPattern, Quant, Selector, Step, StepOwner, Trigger, TriggerWord, Zone,
    ZoneSpec,
};

/// The verb form a subject takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Form {
    /// "a creature dies"
    Third,
    /// "one or more creatures die", "you cast"
    Base,
    /// "a creature died this turn"
    Past,
    /// "you've cast a spell this turn", "you've drawn two cards this turn"
    Participle,
    /// After "would": "would die", "would be put into a graveyard".
    Infinitive,
    /// "a creature dying", "a permanent entering": the event as a noun phrase.
    Gerund,
    /// The infinitive in a rule about what can't happen ("can't block creatures with flying"): a
    /// bare plural there is any one such thing, and a number is a limit ("more than one").
    Cant,
    /// The infinitive in a rule about what must happen ("must be blocked by two or more
    /// creatures"): a bare plural as in [`Form::Cant`], a number as anywhere else.
    Must,
}

pub struct Events {
    pub trigger: P<Trigger>,
    /// Present-tense patterns ("a creature you control dies").
    pub pattern: P<EventPattern>,
    /// The present-tense patterns of a delayed trigger an effect creates (603.7), and whether they
    /// say "next": "when you next cast …" triggers once, however long it lasts (603.7b).
    pub delayed: P<(EventPattern, bool)>,
    /// Past-tense patterns ("a creature died", "you gained life"), for history conditions.
    pub past: P<EventPattern>,
    /// Perfect patterns ("you've cast a spell", "you've been attacked").
    pub perfect: P<EventPattern>,
    /// Negated past ("~ didn't attack") and perfect ("you haven't cast a spell").
    pub past_not: P<EventPattern>,
    pub perfect_not: P<EventPattern>,
}

/// Built together with the conditions, which they refer to ("attacks while saddled") and which
/// refer to them ("if a creature died this turn").
pub fn events() -> &'static Events {
    crate::condition::events_and_conditions().0
}

/// One verb in three forms: third person singular, base, past.
pub fn verb(form: Form, third: &str, base: &str, past: &str) -> P<()> {
    match form {
        Form::Third => lit(third),
        Form::Base => lit(base),
        Form::Past => lit(past),
        Form::Participle => lit(&participle(past)),
        // "is put into" is "be put into"; any other verb is its base form
        Form::Infinitive | Form::Cant | Form::Must => match third.strip_prefix("is ") {
            Some(rest) => lit(&format!("be {rest}")),
            None => lit(base),
        },
        // "being put into"; any other verb inflects its first word
        Form::Gerund => match (third.strip_prefix("is "), base.split_once(' ')) {
            (Some(rest), _) => lit(&format!("being {rest}")),
            (None, Some((first, rest))) => lit(&format!("{} {rest}", ir::lexicon::inflect(first, Inflection::Gerund))),
            (None, None) => lit(&ir::lexicon::inflect(base, Inflection::Gerund)),
        },
    }
}

/// The past participle, from the simple past: the same word except for a few strong verbs and
/// the passive "was …" ("been …").
fn participle(past: &str) -> String {
    let p = past.replacen("drew", "drawn", 1);
    match p.strip_prefix("was ") {
        Some(rest) => format!("been {rest}"),
        None => p,
    }
}

fn one_or_list<T: Val>(p: &P<T>) -> P<Vec<T>> {
    alt("list", vec![p.map(|x| Some(vec![x]), |v: &Vec<T>| (v.len() == 1).then(|| v[0].clone())), p.list("or")])
}

pub fn ordinal() -> P<u32> {
    table(
        "ordinal",
        ["first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth", "tenth"]
            .iter()
            .enumerate()
            .map(|(i, w)| (w.to_string(), i as u32 + 1))
            .collect(),
    )
}

/// How much of damage or life an event is about: "3 or more", "exactly 1".
fn quantity() -> P<Quantity> {
    alt(
        "quantity",
        vec![
            stat_amount().skip(&lit("or more")).map(
                |n| Some(Quantity::AtLeast(n)),
                |q| match q {
                    Quantity::AtLeast(n) => Some(n.clone()),
                    _ => None,
                },
            ),
            lit("exactly").then_keep(&stat_amount()).map(
                |n| Some(Quantity::Exactly(n)),
                |q| match q {
                    Quantity::Exactly(n) => Some(n.clone()),
                    _ => None,
                },
            ),
        ],
    )
}

/// "gains life", "gained 3 or more life", "lose exactly 1 life".
fn life(
    verb: P<()>,
    mk: fn(Option<Quantity>) -> EventAction,
    un: fn(&EventAction) -> Option<Option<Quantity>>,
) -> P<EventAction> {
    verb.then_keep(&quantity().opt()).skip(&lit("life")).map(move |n| Some(mk(n)), un)
}

/// A zone a zone change comes from or goes to (CR 400.1): "your hand", "a graveyard", "your
/// hand or graveyard", "graveyards" (every player's), "exile", "the battlefield", "the command
/// zone". The zones a player owns (400.1: library, hand, graveyard) say whose.
pub fn zone_phrase() -> P<ZoneSpec> {
    let s = selectors();
    let owned_zone = |z: &Zone| matches!(z, Zone::Hand | Zone::Graveyard | Zone::Library);
    let everyone = Selector::new(Quant::Each, ObjectSet::of(Domain::Player));
    let every = everyone.clone();
    let owned = s.possessive.then(&crate::selector::zones()).map(
        |(o, zones)| Some(ZoneSpec { zones, owner: Some(Box::new(o)), slice: None }),
        move |z| match (&z.owner, &z.slice) {
            (Some(o), None) if **o != everyone && z.zones.items.iter().all(owned_zone) => Some(((**o).clone(), z.zones.clone())),
            _ => None,
        },
    );
    // "your hand or your graveyard" says "your hand or graveyard" with the owner twice
    let item = s.possessive.then(&crate::selector::zones());
    let twice = item
        .skip(&lit("or"))
        .then(&item)
        .map(
            |((o1, z1), (o2, z2))| {
                let mut zones = z1;
                zones.items.extend(z2.items);
                (o1 == o2 && zones.conj == ir::Conj::And && !zones.items.is_empty())
                    .then(|| ZoneSpec { zones: ir::Coord { items: zones.items, conj: ir::Conj::Or }, owner: Some(Box::new(o1)), slice: None })
            },
            |_| None,
        )
        .alias();
    // "graveyards" names the zone of every player
    let plural = crate::action::coord(&crate::selector::zone_noun(crate::words::Number::Pl)).map(
        move |zones| Some(ZoneSpec { zones, owner: Some(Box::new(every.clone())), slice: None }),
        |z| match (&z.owner, &z.slice) {
            (Some(o), None) if o.quant == Quant::Each => Some(z.zones.clone()),
            _ => None,
        },
    );
    let shared = |text: &str, zone: Zone| lit(text).to(ZoneSpec::of(zone, None));
    alt(
        "zone",
        vec![
            owned,
            twice,
            plural,
            shared("exile", Zone::Exile),
            shared("the battlefield", Zone::Battlefield),
            shared("the command zone", Zone::Command),
        ],
    )
}

/// Where an object comes from: "from your hand", "from a graveyard", "from exile", "from the
/// battlefield", "from anywhere other than your hand". Not saying is saying "from anywhere".
pub fn from_zone() -> P<ZoneArg> {
    let zone = zone_phrase();
    alt(
        "from zone",
        vec![
            lit("from").then_keep(&zone).map(
                |zone| Some(ZoneArg { zone, other_than: false }),
                |f| (!f.other_than).then(|| f.zone.clone()),
            ),
            lit("from anywhere other than").then_keep(&zone).map(
                |zone| Some(ZoneArg { zone, other_than: true }),
                |f| f.other_than.then(|| f.zone.clone()),
            ),
        ],
    )
}

/// The lexicon's form of a verb for the form a subject takes.
fn inflection(form: Form) -> Inflection {
    match form {
        Form::Third => Inflection::Third,
        Form::Base | Form::Infinitive | Form::Cant | Form::Must => Inflection::Base,
        Form::Past => Inflection::Past,
        Form::Participle => Inflection::Participle,
        Form::Gerund => Inflection::Gerund,
    }
}

/// The verbs whose event phrases still have a rule of their own in [`action`]: they take an
/// argument the lexicon's entries do not describe yet (a zone, a kind of damage, counters,
/// "under your control"), or their subject is what they happen to ("enters", "dies"). A verb
/// leaves this list when its phrase is said from its entry.
pub(crate) fn own_rule(v: Verb) -> bool {
    use Verb::*;
    v.entry().word.is_empty()
        || matches!(
            v,
            Attack
                | Block
                | Move
                | Search
                | Shuffle
                | SeparateIntoPiles
                | TurnFace
                | Attach
                | Unattach
                | Transform
                | DoublePt
                | TriplePt
                | SwitchPt
                | DealDamage
                | GainLife
                | LoseLife
                | DoubleLife
                | PutCounters
                | RemoveCounters
                | MoveCounters
                | DoubleCounters
                | Distribute
                | CreateCopy
                | Champion
                | ExchangeLifeTotals
                | ExchangeLifeWithStat
        )
}

/// Regular player actions with one object slot; By is an optional passive agent.
/// Counts and literal limits use these same role signatures rather than separate verb lists.
pub(crate) fn player_object_verbs() -> impl Iterator<Item = Verb> {
    Verb::ALL.iter().copied().filter(|v| {
        let e = v.entry();
        !own_rule(*v) && v.tail().is_empty() && e.doer == ir::lexicon::Doer::Player
            && e.roles.first().is_some_and(|(role, _)| *role == Role::What)
            && e.roles.iter().skip(1).all(|(role, _)| *role == Role::By)
    })
}

/// Does the verb take what it is done to as its object ("sacrifices a creature")?
fn transitive(v: Verb) -> bool {
    v.subject() == ir::SubjectIs::Doer && v.entry().roles.iter().any(|(r, _)| *r == Role::What)
}

/// The verbs said the regular way: the verb, then what it is done to.
fn regular() -> impl Iterator<Item = Verb> {
    Verb::ALL.iter().copied().filter(|v| !own_rule(*v))
}

/// The verbs with a regular passive ("is exiled", "becomes tapped", "you've been attacked").
pub(crate) fn passives() -> impl Iterator<Item = Verb> {
    Verb::ALL.iter().copied().filter(|v| v.passive().is_some() && (!own_rule(*v) || matches!(v, Verb::Attack | Verb::Champion)))
}

/// Is the passive said with "becomes" ("becomes tapped")? It is when the action leaves the object
/// in a state ([`Verb::becomes`]), which "is tapped" would say instead of the event.
pub(crate) fn becomes(v: Verb) -> bool {
    v.becomes()
}

/// A regular verb with one object and nothing else: "sacrifices", "looks at".
pub(crate) fn plain(v: Verb) -> bool {
    transitive(v) && v.tail().is_empty() && !v.changes_zone()
}

/// Where an object comes from, when it is said: "from your graveyard". Not saying is the same as
/// "from anywhere", which is its alias.
fn origin() -> P<Option<ZoneArg>> {
    alt(
        "origin",
        vec![
            from_zone().map(|z| Some(Some(z)), |o: &Option<ZoneArg>| o.clone()),
            lit("from anywhere").to(None).alias(),
            empty().to(None),
        ],
    )
}

/// The roles of a moved object: what, and where from.
fn moved(what: Selector, from: Option<ZoneArg>) -> Vec<(Role, Arg)> {
    let mut args = vec![(Role::What, Arg::Noun(what))];
    args.extend(from.map(|z| (Role::From, Arg::Zone(z))));
    args
}

/// [`moved`] read back.
fn unmoved(args: &[(Role, Arg)]) -> Option<(Selector, Option<ZoneArg>)> {
    match args {
        [(Role::What, Arg::Noun(what))] => Some((what.clone(), None)),
        [(Role::What, Arg::Noun(what)), (Role::From, Arg::Zone(z))] => Some((what.clone(), Some(z.clone()))),
        _ => None,
    }
}

/// The object of a deed that has only that.
fn only_object(a: &EventAction) -> Option<(Verb, &Selector)> {
    match a {
        EventAction::Deed(Deed { verb, passive: false, args }) => match args.as_slice() {
            [(Role::What, Arg::Noun(what))] => Some((*verb, what)),
            _ => None,
        },
        _ => None,
    }
}

/// Both ways of naming one source's ability share the existing activation-event value (602.2).
fn deed_with_object(verb: Verb, what: Selector) -> EventAction {
    if verb == Verb::Activate && what.quant == Quant::Indefinite(Count::Exactly(Amount::Const(1)))
        && what.chooser.is_none() && what.random.is_none()
        && let Some((kind @ (None | Some(ir::selector::AbilityKind::Loyalty)), source)) = crate::selector::ability_source(&what.set)
    {
        return EventAction::ActivatesAbilityOf {
            loyalty: kind.is_some(), of: Box::new(Selector::new(what.quant, source.clone())),
        };
    }
    EventAction::does(verb, what)
}

/// What a die roll shows (706.2): "a natural 20" (the number on the die, before modifiers), "a 4
/// or higher" (the result, after them), "a die's highest natural result" (its number of sides,
/// 706.1a).
fn roll_result() -> P<ir::event::RollResult> {
    use ir::event::{RollResult, RollValue};
    let natural = lit("natural").opt().map(|o| Some(o.is_some()), |n: &bool| Some(n.then_some(())));
    // no die shows 0 (706.1a: the outcomes are numbered from 1)
    let face = digits().map(|n| (n >= 1).then_some(n), |n| (*n >= 1).then_some(*n));
    let number = face.then(&lit("or higher").opt()).map(
        |(n, higher)| {
            let n = Amount::Const(n);
            Some(if higher.is_some() { Quantity::AtLeast(n) } else { Quantity::Exactly(n) })
        },
        |q| match q {
            Quantity::AtLeast(Amount::Const(n)) => Some((*n, Some(()))),
            Quantity::Exactly(Amount::Const(n)) => Some((*n, None)),
            _ => None,
        },
    );
    // "a natural 8", "an 8": the article the next word takes
    let article = alt("a", vec![lit("a").to(false), lit("an").to(true)]);
    let an = |natural: bool, q: &Quantity| match q {
        Quantity::AtLeast(Amount::Const(n)) | Quantity::Exactly(Amount::Const(n)) if !natural => {
            crate::kernel::takes_an(&oracle::token::Kind::Number { value: *n, text: n.to_string() })
        }
        _ => false,
    };
    let shown = article.then(&natural).then(&number).map(
        move |((said_an, natural), q)| (said_an == an(natural, &q)).then(|| RollResult { natural, value: RollValue::Number(q) }),
        move |r| match &r.value {
            RollValue::Number(q) => Some(((an(r.natural, q), r.natural), q.clone())),
            RollValue::Highest => None,
        },
    );
    let highest = lit("a die's highest").then_keep(&natural).skip(&lit("result")).map(
        |natural| Some(RollResult { natural, value: RollValue::Highest }),
        |r| (r.value == RollValue::Highest).then_some(r.natural),
    );
    alt("roll result", vec![shown, highest])
}

/// The actions said the regular way, each from its lexicon entry: "sacrifices a creature",
/// "casts a spell from their hand", "taps a land for mana", "scries", and the passives said
/// with "becomes" ("becomes tapped").
fn deeds(form: Form, object: &P<Selector>) -> Vec<P<EventAction>> {
    let f = inflection(form);
    let heads = |keep: fn(Verb) -> bool| table("verb", regular().filter(|v| keep(*v)).map(|v| (v.head(f), v)).collect());
    let with_object = heads(plain)
        .then(object)
        .map(|(verb, what)| Some(deed_with_object(verb, what)), |a| only_object(a).map(|(v, what)| (v, what.clone())));
    // the object stands inside the verb: "taps a land for mana"
    let around = regular().filter(|v| transitive(*v) && !v.tail().is_empty()).map(|verb| {
        lit(&verb.head(f)).then_keep(object).skip(&lit(verb.tail())).map(
            move |what| Some(EventAction::does(verb, what)),
            move |a| only_object(a).filter(|(v, _)| *v == verb).map(|(_, what)| what.clone()),
        )
    });
    // "you put a Town card into your hand", "returned a Squirrel card to your hand": a zone
    // change and where it went (400.7), "put" with "into", "return" with "to"
    let to_zone = |intro: String, into: &'static str| {
        lit(&intro).then_keep(object).skip(&lit(into)).then(&zone_phrase()).map(
            |(what, zone)| Some(EventAction::Deed(Deed { verb: Verb::Move, passive: false,
                args: vec![(Role::What, Arg::Noun(what)), (Role::To, Arg::Zone(ZoneArg { zone, other_than: false }))] })),
            |a| match a {
                EventAction::Deed(Deed { verb: Verb::Move, passive: false, args }) => match args.as_slice() {
                    [(Role::What, Arg::Noun(what)), (Role::To, Arg::Zone(ZoneArg { zone, other_than: false }))] => Some((what.clone(), zone.clone())),
                    _ => None,
                },
                _ => None,
            },
        )
    };
    let put_into = to_zone(Verb::Move.head(f), "into");
    let returned_to = to_zone(Verb::Move.bare(f), "to");
    // "casts a spell from your hand", "plays a land from exile", "exiles a card from a graveyard":
    // an action that moves its object (400.1) says where from; "a spell that targets ~" is the
    // selector's (Atom::Targets)
    let moving = heads(|v| transitive(v) && v.changes_zone() && v.tail().is_empty()).then(object).then(&origin()).map(
        |((verb, what), from)| Some(EventAction::Deed(Deed { verb, passive: false, args: moved(what, from) })),
        |a| match a {
            EventAction::Deed(Deed { verb, passive: false, args }) => {
                let (what, from) = unmoved(args)?;
                Some(((*verb, what), from))
            }
            _ => None,
        },
    );
    let bare = |passive: bool| {
        move |verb| Some(EventAction::Deed(Deed { verb, passive, args: Vec::new() }))
    };
    let said_bare = |passive: bool| {
        move |a: &EventAction| match a {
            EventAction::Deed(Deed { verb, passive: p, args }) if *p == passive && args.is_empty() => Some(*verb),
            _ => None,
        }
    };
    let without_object = heads(|v| !transitive(v)).map(bare(false), said_bare(false));
    // "enters tapped", "enters attacking": the status it has as it arrives (110.5, 614.12)
    let arriving = table(
        "status",
        STATUS_WORDS
            .iter()
            .filter(|(_, s)| matches!(s, Status::Tapped | Status::Untapped | Status::Attacking | Status::Blocking | Status::Transformed))
            .map(|(w, s)| (w.to_string(), *s))
            .collect(),
    );
    let arrives = heads(|v| !transitive(v) && v.arrives()).then(&arriving).map(
        |(verb, status)| Some(EventAction::Deed(Deed { verb, passive: false, args: vec![(Role::As, Arg::Status(status))] })),
        |a| match a {
            EventAction::Deed(Deed { verb, passive: false, args }) => match args.as_slice() {
                [(Role::As, Arg::Status(s))] => Some((*verb, *s)),
                _ => None,
            },
            _ => None,
        },
    );
    // "enters from a graveyard": what happens to the subject, and where it came from
    let entering = heads(|v| !transitive(v) && v.changes_zone()).then(&from_zone()).map(
        |(verb, z)| Some(EventAction::Deed(Deed { verb, passive: false, args: vec![(Role::From, Arg::Zone(z))] })),
        |a| match a {
            EventAction::Deed(Deed { verb, passive: false, args }) => match args.as_slice() {
                [(Role::From, Arg::Zone(z))] => Some((*verb, z.clone())),
                _ => None,
            },
            _ => None,
        },
    );
    // "enters under your control", "died under your control": where the action has that role
    let under = lit("under").then_keep(&selectors().possessive).skip(&lit("control"));
    let creates_under = lit(&Verb::Create.head(f)).then_keep(object).then(&under).map(
        |(what,who)| Some(EventAction::Deed(Deed {verb:Verb::Create,passive:false,args:vec![(Role::What,Arg::Noun(what)),(Role::Under,Arg::Noun(who))]})),
        |a| match a {EventAction::Deed(Deed {verb:Verb::Create,passive:false,args})=>match args.as_slice(){
            [(Role::What,Arg::Noun(what)),(Role::Under,Arg::Noun(who))]=>Some((what.clone(),who.clone())),_=>None},_=>None},
    );
    let has_under = |v: Verb| !transitive(v) && v.entry().roles.iter().any(|(r, _)| *r == Role::Under);
    let under_control = heads(has_under).then(&under).map(
        |(verb, who)| Some(EventAction::under(verb, who)),
        |a| match a {
            EventAction::Deed(Deed { verb, passive: false, args }) => match args.as_slice() {
                [(Role::Under, Arg::Noun(who))] => Some((*verb, who.clone())),
                _ => None,
            },
            _ => None,
        },
    );
    // "enters the battlefield" says "enters"
    let enters_the_battlefield = lit(&format!("{} the battlefield", Verb::Enter.head(f)))
        .then_keep(&under.opt())
        .map(
            |u| Some(match u {
                Some(who) => EventAction::under(Verb::Enter, who),
                None => EventAction::just(Verb::Enter),
            }),
            |_| None,
        )
        .alias();
    let aux = match form {
        Form::Third => "becomes",
        Form::Base | Form::Participle | Form::Infinitive | Form::Cant | Form::Must => "become",
        Form::Past => "became",
        Form::Gerund => "becoming",
    };
    let becomes_done = table(
        "becomes",
        passives().filter(|v| becomes(*v)).map(|v| (format!("{aux} {}", v.phrase(Inflection::Participle)), v)).collect(),
    )
    .map(bare(true), said_bare(true));
    // "rolls a natural 20": rolling dice (706.1), with what the roll shows
    let rolls = lit(&ir::lexicon::inflect("roll", f)).then_keep(&roll_result()).map(
        |r| Some(EventAction::Deed(Deed { verb: Verb::RollDice, passive: false, args: vec![(Role::Result, Arg::Roll(r))] })),
        |a| match a {
            EventAction::Deed(Deed { verb: Verb::RollDice, passive: false, args }) => match args.as_slice() {
                [(Role::Result, Arg::Roll(r))] => Some(r.clone()),
                _ => None,
            },
            _ => None,
        },
    );
    let mut all = vec![with_object, moving, put_into, returned_to, entering, arrives, without_object, creates_under, under_control, enters_the_battlefield, becomes_done];
    all.extend(around);
    all.push(rolls);
    // after "would" and "can't" the passive is a phrase of its own: "be destroyed"; as a noun
    // phrase, "being destroyed"
    let be = match form {
        Form::Infinitive | Form::Cant | Form::Must => Some("be"),
        Form::Gerund => Some("being"),
        Form::Third | Form::Base | Form::Past | Form::Participle => None,
    };
    if let Some(be) = be {
        all.push(lit(be).then_keep(&participle_phrase()).map(
            |d| Some(EventAction::Deed(d)),
            |a| match a {
                EventAction::Deed(d) => Some(d.clone()),
                _ => None,
            },
        ));
    }
    all
}

/// A noun phrase in a rule about what can happen: a bare plural is any one such thing ("can't
/// cast noncreature spells": a noncreature spell), and with `limits` "more than one" a number
/// beyond a limit ("can't be blocked by more than one creature": by two or more).
pub(crate) fn generic(limits: bool) -> P<Selector> {
    let s = selectors();
    let one = Quant::Indefinite(Count::Exactly(Amount::Const(1)));
    let plain = |x: &Selector| x.chooser.is_none() && x.random.is_none();
    let bare = {
        let (q, q2) = (one.clone(), one);
        s.set_pl.map(
            move |set| Some(Selector::new(q.clone(), set)),
            move |x: &Selector| (x.quant == q2 && plain(x)).then(|| x.set.clone()),
        )
    };
    let mut v = vec![bare];
    if limits {
        let two = Quant::Indefinite(Count::AtLeast(Amount::Const(2)));
        let two2 = two.clone();
        v.push(lit("more than one").then_keep(&s.set_sg).map(
            move |set| Some(Selector::new(two.clone(), set)),
            move |x: &Selector| (x.quant == two2 && plain(x)).then(|| x.set.clone()),
        ));
        v.push(lit("more than").then_keep(&crate::words::number_words()).then(&s.set_pl).map(
            |(n, set)| (n >= 2).then(|| Selector::new(Quant::Indefinite(Count::AtLeast(Amount::Const(n + 1))), set)),
            move |x: &Selector| match &x.quant {
                Quant::Indefinite(Count::AtLeast(Amount::Const(n))) if *n >= 3 && plain(x) => Some((n - 1, x.set.clone())),
                _ => None,
            },
        ));
    }
    v.push(s.selector.clone());
    alt("noun phrase", v)
}

/// One [`generic`] noun phrase, or alternatives joined by "or"; bare plurals alone are one set
/// with a union of types ("Knights or Walls").
pub(crate) fn generic_list(limits: bool) -> P<Vec<Selector>> {
    let item = generic(limits);
    let bare = |x: &Selector| x.quant == Quant::Indefinite(Count::Exactly(Amount::Const(1)));
    let single = item.map(|x| Some(vec![x]), |v: &Vec<Selector>| (v.len() == 1).then(|| v[0].clone()));
    let list = item.list("or").map(
        move |v| (!v.iter().all(bare)).then_some(v),
        move |v: &Vec<Selector>| (v.len() >= 2 && !v.iter().all(bare)).then(|| v.clone()),
    );
    alt("noun phrases", vec![single, list])
}

/// The three passive zone changes, each given the words that introduce it ("is put into", "put
/// into"): into a graveyard, into any other zone but a hand ("put into exile from the
/// battlefield", "are put into a library from anywhere") and the bounce of a permanent to a
/// hand ("is returned to your hand"): a move with the zone it goes to and, when said, the one it
/// comes from (400.7). The bounce is said "returned to", every other destination "put into".
fn moves(put: P<()>, returned: P<()>, says_anywhere: bool) -> (P<EventAction>, P<EventAction>, P<EventAction>) {
    let s = selectors();
    let anywhere = lit("from anywhere").to(None);
    let gy_from = alt(
        "from zone",
        vec![
            lit("from the battlefield").to(Some(Zone::Battlefield)),
            if says_anywhere { anywhere.clone() } else { anywhere.alias() },
            lit("from your library").to(Some(Zone::Library)),
            lit("from your hand").to(Some(Zone::Hand)),
        ],
    );
    // no "from" is from anywhere; an event says so, a reduced relative leaves it out
    let none = empty().map(|()| Some(None), |z: &Option<Zone>| z.is_none().then_some(()));
    let gy_from = alt("from zone", vec![gy_from, if says_anywhere { none.alias() } else { none }]);
    // In an event, bare "graveyards" permits any destination owner, just as "a graveyard".
    // The subject still says whether to match one card or a batch of one or more cards.
    let anyone = Selector::new(Quant::Indefinite(ir::Count::Exactly(ir::Amount::Const(1))), ObjectSet::of(Domain::Player));
    let graveyard_owner = alt("graveyard owner", vec![
        s.possessive.skip(&lit("graveyard")),
        lit("graveyards").to(anyone).alias(),
    ]);
    let put_into_gy = put
        .then_keep(&graveyard_owner)
        .then(&gy_from)
        .map(
            |(into, from)| Some(EventAction::PutIntoGraveyard { into: Box::new(into), from }),
            |a| match a {
                EventAction::PutIntoGraveyard { into, from } => Some(((**into).clone(), *from)),
                _ => None,
            },
        );
    let move_to = |intro: P<()>, wanted: fn(&ZoneSpec) -> bool, printed: fn(&ZoneSpec) -> bool| {
        intro.then_keep(&zone_phrase()).then(&origin()).map(
            move |(to, from)| {
                wanted(&to).then(|| {
                    let mut args = vec![(Role::To, Arg::Zone(ZoneArg { zone: to, other_than: false }))];
                    args.extend(from.map(|z| (Role::From, Arg::Zone(z))));
                    EventAction::Deed(Deed { verb: Verb::Move, passive: true, args })
                })
            },
            move |a| match a {
                EventAction::Deed(Deed { verb: Verb::Move, passive: true, args }) => match args.as_slice() {
                    [(Role::To, Arg::Zone(to))] if printed(&to.zone) => Some((to.zone.clone(), None)),
                    [(Role::To, Arg::Zone(to)), (Role::From, Arg::Zone(from))] if printed(&to.zone) => {
                        Some((to.zone.clone(), Some(from.clone())))
                    }
                    _ => None,
                },
                _ => None,
            },
        )
    };
    if !says_anywhere {
        // the reduced relative: a card is "put into" a hand as readily as "returned to" it, and
        // "put onto the battlefield" is the arrival's own phrase
        let put_into = move_to(put, |d| !matches!(d.zone(), Some(Zone::Graveyard | Zone::Battlefield)), |d| {
            !matches!(d.zone(), Some(Zone::Graveyard | Zone::Battlefield))
        });
        let returned = move_to(returned, |d| d.zone() == Some(Zone::Hand), |_| false).alias();
        return (put_into_gy, put_into, returned);
    }
    let put_into = move_to(put, |d| d.zone() != Some(Zone::Graveyard), |d| {
        d.zone() != Some(Zone::Graveyard) && d.zone() != Some(Zone::Hand)
    });
    let returned = move_to(returned, |d| d.zone() == Some(Zone::Hand), |d| d.zone() == Some(Zone::Hand));
    (put_into_gy, put_into, returned)
}

/// What was done to a thing, said with the participle alone, no "was": "exiled", "revealed from
/// their hand", "cast from a graveyard", "put into your hand", "returned". Every passive of the
/// lexicon, in the reduced relative clause ("cards exiled this way", "a creature not chosen this
/// way") that the construction "the …ed one" (CR 608.2c) makes of it.
pub fn reduced_passive() -> P<EventAction> {
    let (into_graveyard, put_into, returned_to) = moves(lit("put into"), lit("returned to"), false);
    let deed = participle_phrase().map(
        |d| Some(EventAction::Deed(d)),
        |a| match a {
            EventAction::Deed(d) => Some(d.clone()),
            _ => None,
        },
    );
    // what an action leaves in a state is "tapped", "goaded" here, as "becomes tapped" is there
    let states = table(
        "participle",
        passives().filter(|v| becomes(*v)).map(|v| (v.phrase(Inflection::Participle), v)).collect(),
    )
    .map(
        |verb| Some(EventAction::Deed(Deed { verb, passive: true, args: Vec::new() })),
        |a| match a {
            EventAction::Deed(Deed { verb, passive: true, args }) if args.is_empty() => Some(*verb),
            _ => None,
        },
    );
    // "returned" with no destination said, "put onto the battlefield" (400.7)
    let returned = lit(&Verb::Move.bare(Inflection::Participle))
        .to(EventAction::Deed(Deed { verb: Verb::Move, passive: true, args: Vec::new() }));
    let battlefield = ZoneArg { zone: ZoneSpec::of(Zone::Battlefield, None), other_than: false };
    let onto = lit("put onto the battlefield")
        .to(EventAction::Deed(Deed { verb: Verb::Move, passive: true, args: vec![(Role::To, Arg::Zone(battlefield))] }));
    alt("reduced passive", vec![deed, states, into_graveyard, put_into, returned_to, returned, onto])
}

/// A past predicate whose player subject is supplied by the surrounding condition.
pub(crate) fn past_predicate(plural: bool) -> P<EventAction> {
    if plural {
        crate::kernel::lazy("plural past predicate", || action(Form::Past, true))
    } else {
        crate::kernel::lazy("singular past predicate", || action(Form::Past, false))
    }
}

/// `plural`: the subject is plural or "you", which turns a past "was …" into "were …".
pub fn action(form: Form, plural: bool) -> P<EventAction> {
    let s = selectors();
    let v = |third: &str, base: &str, past: &str| {
        let past = match past.strip_prefix("was ") {
            Some(rest) if plural => format!("were {rest}"),
            _ => past.to_string(),
        };
        verb(form, third, base, &past)
    };
    let unit = |p: P<()>, a: EventAction| p.to(a);
    // what a phrase names: a noun phrase, or in a rule about what can happen a kind of thing
    let rule = matches!(form, Form::Cant | Form::Must);
    let limits = form == Form::Cant;
    let object = if rule { generic(limits) } else { s.selector.clone() };
    // Singular event roles own complete noun predicates; recipients already have their
    // own coordination and must keep primitive items.
    let predicate = if rule { object.clone() } else { s.predicate.clone() };
    let (put_into_gy, put_into, returned) =
        moves(v("is put into", "are put into", "was put into"), v("is returned to", "are returned to", "was returned to"), true);
    let leaves_zone = v("leaves", "leave", "left").then_keep(&s.possessive).skip(&lit("graveyard")).map(
        |owner| {
            Some(EventAction::LeavesZone {
                zone: ZoneSpec { zones: ir::Coord::one(Zone::Graveyard), owner: Some(Box::new(owner)), slice: None },
            })
        },
        |a| match a {
            EventAction::LeavesZone { zone } => zone.owner_of(Zone::Graveyard).cloned(),
            _ => None,
        },
    );
    let defenders = if rule { generic_list(limits).opt() } else { one_or_list(&s.selector).opt() };
    let with = lit("with").then_keep(&alt("attackers", vec![s.selector.clone(), crate::selector::bare_plural()]));
    // "can't attack alone" is said of the creatures together (statics)
    let alone = if rule { empty().map(|()| Some(None), |o: &Option<()>| o.is_none().then_some(())) } else { lit("alone").opt() };
    let attacks =
        v("attacks", "attack", "attacked").then_keep(&alone).then(&defenders).then(&with.opt()).map(
            |((alone, d), with)| {
                Some(EventAction::Attacks {
                    alone: alone.is_some(),
                    defender: d.unwrap_or_default(),
                    with: with.map(Box::new),
                })
            },
            |a| match a {
                EventAction::Attacks { alone, defender, with } => Some((
                    (alone.then_some(()), (!defender.is_empty()).then(|| defender.clone())),
                    with.as_deref().cloned(),
                )),
                _ => None,
            },
        );
    let blocks = v("blocks", "block", "blocked").then_keep(&predicate.opt()).map(
        |b| Some(EventAction::Blocks { blocked: b.map(Box::new) }),
        |a| match a {
            EventAction::Blocks { blocked } => Some(blocked.as_deref().cloned()),
            _ => None,
        },
    );
    let blocked = if rule {
        // "be blocked", "be blocked by creatures with flying", "be blocked except by Walls"
        let by = alt(
            "blocked by",
            vec![
                empty().to((Vec::new(), false)),
                lit("by").then_keep(&generic_list(limits)).map(
                    |by| Some((by, false)),
                    |(by, except): &(Vec<Selector>, bool)| (!except && !by.is_empty()).then(|| by.clone()),
                ),
                lit("except by").then_keep(&generic_list(false)).map(
                    |by| Some((by, true)),
                    |(by, except): &(Vec<Selector>, bool)| (*except && !by.is_empty()).then(|| by.clone()),
                ),
            ],
        );
        lit("be blocked").then_keep(&by).map(
            |(by, except)| Some(EventAction::BecomesBlocked { by, except }),
            |a| match a {
                EventAction::BecomesBlocked { by, except } => Some((by.clone(), *except)),
                _ => None,
            },
        )
    } else {
        v("becomes blocked", "become blocked", "became blocked").then_keep(&lit("by").then_keep(&s.selector).opt()).map(
            |by| Some(EventAction::BecomesBlocked { by: by.into_iter().collect(), except: false }),
            |a| match a {
                EventAction::BecomesBlocked { by, except: false } => match by.as_slice() {
                    [] => Some(None),
                    [by] => Some(Some(by.clone())),
                    _ => None,
                },
                _ => None,
            },
        )
    };
    let kind = alt(
        "damage kind",
        vec![
            lit("combat damage").to(DamageKind::Combat),
            lit("noncombat damage").to(DamageKind::Noncombat),
            lit("damage").to(DamageKind::Any),
        ],
    );
    let deals = v("deals", "deal", "dealt")
        .then_keep(&quantity().opt())
        .then(&kind)
        .then(&lit("to").then_keep(&crate::action::coord(&object)).opt())
        .map(
        |((amount, kind), to)| Some(EventAction::DealsDamage { kind, amount, to }),
        |a| match a {
            EventAction::DealsDamage { kind, amount, to } => Some(((amount.clone(), *kind), to.clone())),
            _ => None,
        },
    );
    // a rule says "be the target of", the noun agreeing with its subject
    let becomes_target = if rule {
        let (own, other) = if plural { ("be the targets of", "be the target of") } else { ("be the target of", "be the targets of") };
        alt("be the target of", vec![lit(own), lit(other).alias()])
    } else {
        v("becomes the target of", "become the target of", "became the target of")
    };
    let target_of = becomes_target.then_keep(&predicate).map(
        |of| Some(EventAction::BecomesTarget { of: Some(Box::new(of)) }),
        |a| match a {
            EventAction::BecomesTarget { of: Some(of) } => Some((**of).clone()),
            _ => None,
        },
    );
    let mut rules = deeds(form, &predicate);
    if rule {
        // "search libraries": any player's
        let anyone = Selector::new(Quant::Indefinite(Count::Exactly(Amount::Const(1))), ObjectSet::of(Domain::Player));
        rules.push(lit("search libraries").to(EventAction::Searches { zone: ZoneSpec::of(Zone::Library, Some(anyone)) }));
        // "have -1/-1 counters put on it": counters put on the subject, any number of them
        let it = if plural { "counters put on them" } else { "counters put on it" };
        rules.push(lit("have").then_keep(&counter_kind().opt()).skip(&lit(it)).map(
            |kind| Some(EventAction::CountersPut { kind, one_or_more: false, nth: None }),
            |a| match a {
                EventAction::CountersPut { kind, one_or_more: false, nth: None } => Some(kind.clone()),
                _ => None,
            },
        ));
    }
    rules.extend(
        vec![
            v("begins their turn", "begin your turn", "began their turn").to(EventAction::just(Verb::BeginTurn)).alias(),
            leaves_zone,
            put_into_gy,
            unit(
                v("attacks and isn't blocked", "attack and aren't blocked", "attacked and wasn't blocked"),
                EventAction::AttacksUnblocked,
            ),
            attacks,
            blocks,
            blocked,
            deals,
            // the source, the doer named after the passive: "by a red instant or sorcery spell
            // you control or by a red planeswalker you control"
            v("is dealt", "are dealt", "was dealt").then_keep(&lit("excess").opt()).then(&kind).then(&one_or_list(&lit("by").then_keep(&s.selector)).opt()).map(
                |((excess, kind), by)| Some(EventAction::IsDealtDamage { kind, excess: excess.is_some(), by: by.unwrap_or_default() }),
                |a| match a {
                    EventAction::IsDealtDamage { kind, excess, by } => Some((((*excess).then_some(()), *kind), (!by.is_empty()).then(|| by.clone()))),
                    _ => None,
                },
            ),
            put_into,
            returned,
            v("becomes attached", "become attached", "became attached").then_keep(&lit("to").then_keep(&s.selector).opt()).map(
                |to| Some(EventAction::BecomesAttached { to: to.map(Box::new) }),
                |a| match a {
                    EventAction::BecomesAttached { to } => Some(to.as_deref().cloned()),
                    _ => None,
                },
            ),
            v("becomes unattached", "become unattached", "became unattached")
                .then_keep(&lit("from").then_keep(&s.selector).opt())
                .map(
                    |from| Some(EventAction::BecomesUnattached { from: from.map(Box::new) }),
                    |a| match a {
                        EventAction::BecomesUnattached { from } => Some(from.as_deref().cloned()),
                        _ => None,
                    },
                ),
            v("shuffles", "shuffle", "shuffled").then_keep(&s.possessive).skip(&lit("library")).map(
                |owner| {
                    Some(EventAction::Shuffles {
                        zone: ZoneSpec { zones: ir::Coord::one(Zone::Library), owner: Some(Box::new(owner)), slice: None },
                    })
                },
                |a| match a {
                    EventAction::Shuffles { zone } => zone.owner_of(Zone::Library).cloned(),
                    _ => None,
                },
            ),
            v("loses control of", "lose control of", "lost control of").then_keep(&s.selector).map(
                |of| Some(EventAction::LosesControl { of: Box::new(of) }),
                |a| match a {
                    EventAction::LosesControl { of } => Some((**of).clone()),
                    _ => None,
                },
            ),
            unit(v("is turned face up", "are turned face up", "was turned face up"), EventAction::TurnedFaceUp),
            unit(
                v("leaves the battlefield without dying", "leave the battlefield without dying", "left the battlefield without dying"),
                EventAction::LeavesWithoutDying,
            ),
            target_of,
            v("transforms", "transform", "transformed").then_keep(&lit("into").then_keep(&s.selector).opt()).map(
                |into| Some(EventAction::Transforms { into: into.map(Box::new) }),
                |a| match a {
                    EventAction::Transforms { into } => Some(into.as_deref().cloned()),
                    _ => None,
                },
            ),
            v("expends", "expend", "expended").then_keep(&stat_amount()).map(
                |n| Some(EventAction::Expends(n)),
                |a| match a {
                    EventAction::Expends(n) => Some(n.clone()),
                    _ => None,
                },
            ),
            v("becomes level", "become level", "became level").then_keep(&digits()).map(
                |n| Some(EventAction::BecomesLevel(n)),
                |a| match a {
                    EventAction::BecomesLevel(n) => Some(*n),
                    _ => None,
                },
            ),
            v("fully unlocks", "fully unlock", "fully unlocked").then_keep(&s.selector).map(
                |what| Some(EventAction::FullyUnlocks { what: Box::new(what) }),
                |a| match a {
                    EventAction::FullyUnlocks { what } => Some((**what).clone()),
                    _ => None,
                },
            ),
            unit(v("gains or loses life", "gain or lose life", "gained or lost life"), EventAction::GainsOrLosesLife),
            unit(
                v("is enchanted by other Auras", "are enchanted by other Auras", "was enchanted by other Auras"),
                EventAction::EnchantedByOtherAuras,
            ),
            // "you activate a loyalty ability of a Chandra planeswalker"
            alt(
                "ability of",
                vec![
                    v("activates an ability of", "activate an ability of", "activated an ability of").to(false),
                    v("activates a loyalty ability of", "activate a loyalty ability of", "activated a loyalty ability of").to(true),
                ],
            )
            .then(&s.selector)
            .map(
                |(loyalty, of)| Some(EventAction::ActivatesAbilityOf { loyalty, of: Box::new(of) }),
                |a| match a {
                    EventAction::ActivatesAbilityOf { loyalty, of } => Some((*loyalty, (**of).clone())),
                    _ => None,
                },
            ),
            life(v("gains", "gain", "gained"), |amount| EventAction::GainsLife { amount }, |a| match a {
                EventAction::GainsLife { amount } => Some(amount.clone()),
                _ => None,
            }),
            life(v("loses", "lose", "lost"), |amount| EventAction::LosesLife { amount }, |a| match a {
                EventAction::LosesLife { amount } => Some(amount.clone()),
                _ => None,
            }),
            v("searches", "search", "searched").then_keep(&s.possessive).skip(&lit("library")).map(
                |owner| {
                    Some(EventAction::Searches {
                        zone: ZoneSpec { zones: ir::Coord::one(Zone::Library), owner: Some(Box::new(owner)), slice: None },
                    })
                },
                |a| match a {
                    EventAction::Searches { zone } => zone.owner_of(Zone::Library).cloned(),
                    _ => None,
                },
            ),
            v("puts", "put", "put").then_keep(&s.selector).skip(&lit("onto the battlefield")).map(
                |what| Some(EventAction::PutsOntoBattlefield { what: Box::new(what) }),
                |a| match a {
                    EventAction::PutsOntoBattlefield { what } => Some((**what).clone()),
                    _ => None,
                },
            ),
            puts_counters(v("puts", "put", "put")),
            // "you get one or more {E}", "you would get one or more poison counters"
            v("gets one or more", "get one or more", "got one or more")
                .then_keep(&alt(
                    "counters a player gets",
                    vec![
                        lit("{E}").to(Some(ir::PlayerCounter::Energy)),
                        crate::selector::counter_kind().skip(&lit("counters")).map(
                            |k| Some(Some(ir::PlayerCounter::Named(k))),
                            |k: &Option<ir::PlayerCounter>| match k {
                                Some(ir::PlayerCounter::Named(k)) => Some(k.clone()),
                                _ => None,
                            },
                        ),
                        lit("counters").to(None),
                    ],
                ))
                .map(
                    |kind| Some(EventAction::GetsCounters { kind }),
                    |a| match a {
                        EventAction::GetsCounters { kind } => Some(kind.clone()),
                        _ => None,
                    },
                ),
        ],
    );
    alt("event", rules)
}

/// "the fourth", "the last": which counter of a kind (CR 122.7), counted with the ordinal reader
/// of "for the second time".
fn counter_nth() -> P<CounterNth> {
    lit("the").then_keep(&alt(
        "which",
        vec![
            ordinal().map(
                |n| Some(CounterNth::Nth(n)),
                |w| match w {
                    CounterNth::Nth(n) => Some(*n),
                    CounterNth::Last => None,
                },
            ),
            lit("last").to(CounterNth::Last),
        ],
    ))
}

/// "one or more +1/+1 counters are put on ~", "a counter is put on a creature you control".
fn counters_put(is: &str, are: &str) -> P<EventPattern> {
    let s = selectors();
    let a = alt("a", vec![lit("a"), lit("an").alias()]);
    let one = a
        .then_keep(&counter_kind().opt())
        .skip(&lit(&format!("counter {is}")))
        .map(|k| Some((k, false, None)), |(k, many, nth)| (!many && nth.is_none()).then(|| k.clone()));
    let many = lit("one or more")
        .then_keep(&counter_kind().opt())
        .skip(&lit(&format!("counters {are}")))
        .map(|k| Some((k, true, None)), |(k, many, _)| many.then(|| k.clone()));
    // "the fourth plan counter is put on ~": the counter that brings the object to N (122.7)
    let nth = counter_nth().then(&counter_kind().opt()).skip(&lit(&format!("counter {is}"))).map(
        |(n, k)| (n != CounterNth::Last).then_some((k, false, Some(n))),
        |(k, many, nth)| if *many { None } else { nth.map(|n| (n, k.clone())) },
    );
    alt("counters", vec![one, many, nth]).then(&s.selector).map(
        |((kind, one_or_more, nth), on)| {
            Some(EventPattern::new(vec![on], vec![EventAction::CountersPut { kind, one_or_more, nth }]))
        },
        |p| match (p.subjects.as_slice(), p.actions.as_slice()) {
            ([on], [EventAction::CountersPut { kind, one_or_more, nth }]) => {
                Some(((kind.clone(), *one_or_more, *nth), on.clone()))
            }
            _ => None,
        },
    )
}

/// "puts one or more +1/+1 counters on a creature you control".
fn puts_counters(verb: P<()>) -> P<EventAction> {
    let a = alt("a", vec![lit("a"), lit("an").alias()]);
    let one = a.then_keep(&counter_kind().opt()).skip(&lit("counter on")).map(|k| Some((k, false)), |(k, many)| (!many).then(|| k.clone()));
    let many = lit("one or more")
        .then_keep(&counter_kind().opt())
        .skip(&lit("counters on"))
        .map(|k| Some((k, true)), |(k, many)| many.then(|| k.clone()));
    verb.then_keep(&alt("counters", vec![one, many])).then(&selectors().selector).map(
        |((kind, one_or_more), on)| Some(EventAction::PutsCounters { kind, one_or_more, on: Box::new(on) }),
        |a| match a {
            EventAction::PutsCounters { kind, one_or_more, on } => Some(((kind.clone(), *one_or_more), (**on).clone())),
            _ => None,
        },
    )
}

/// "a time counter is removed from ~", "one or more counters are removed from ~".
fn counters_removed() -> P<EventPattern> {
    let s = selectors();
    let a = alt("a", vec![lit("a"), lit("an").alias()]);
    let one = a
        .then_keep(&counter_kind().opt())
        .skip(&lit("counter is removed from"))
        .map(|k| Some((k, false, None)), |(k, many, nth)| (!many && nth.is_none()).then(|| k.clone()));
    let many = lit("one or more")
        .then_keep(&counter_kind().opt())
        .skip(&lit("counters are removed from"))
        .map(|k| Some((k, true, None)), |(k, many, _)| many.then(|| k.clone()));
    // "the last time counter is removed from ~"
    let nth = counter_nth().then(&counter_kind().opt()).skip(&lit("counter is removed from")).map(
        |(n, k)| Some((k, false, Some(n))),
        |(k, many, nth)| if *many { None } else { nth.map(|n| (n, k.clone())) },
    );
    alt("counters", vec![one, many, nth]).then(&s.selector).map(
        |((kind, one_or_more, nth), from)| {
            Some(EventPattern::new(vec![from], vec![EventAction::CountersRemoved { kind, one_or_more, nth }]))
        },
        |p| match (p.subjects.as_slice(), p.actions.as_slice()) {
            ([from], [EventAction::CountersRemoved { kind, one_or_more, nth }]) => {
                Some(((kind.clone(), *one_or_more, *nth), from.clone()))
            }
            _ => None,
        },
    )
}

/// `~` as a subject list.
fn self_ref() -> P<Vec<Selector>> {
    let this = |s: &Selector| s.quant == Quant::Definite(Ref::This);
    selectors().singular.map(
        move |s| this(&s).then(|| vec![s]),
        move |v: &Vec<Selector>| (v.len() == 1 && this(&v[0])).then(|| v[0].clone()),
    )
}

/// "a spell or ability an opponent controls causes you to discard ~".
pub(crate) fn caused() -> P<EventPattern> {
    let s = selectors();
    let player = alt("player", vec![lit("you").to(Selector::you()), s.singular.clone()]);
    s.selector.skip(&lit("causes")).then(&player).skip(&lit("to")).then(&action(Form::Base, false)).map(
        |((cause, player), action)| {
            let mut p = EventPattern::new(vec![player], vec![action]);
            p.cause = Some(EventCause::Source(Box::new(cause)));
            p.typed().then_some(p)
        },
        |p| match (p.subjects.as_slice(), p.actions.as_slice(), &p.cause, p.nth_each_turn, &p.during) {
            ([player], [action], Some(EventCause::Source(cause)), None, None) => Some((((**cause).clone(), player.clone()), action.clone())),
            _ => None,
        },
    )
}

/// "~ and at least two other creatures attack": the subject attacks with the others.
fn together() -> P<EventPattern> {
    let s = selectors();
    let not_player = |x: &Selector| x.set.domain != Domain::Player;
    // "at least two other creatures": the count spelled before the noun
    // "at least two other creatures", "at least one other creature": the noun agrees with the count
    let one = |n: &Amount| *n == Amount::Const(1);
    let at_least = |sets: &P<ObjectSet>, singular: bool| {
        lit("at least").then_keep(&crate::words::count_amount()).then(sets).map(
            move |(n, set)| (one(&n) == singular).then(|| Selector::new(Quant::Indefinite(Count::AtLeast(n)), set)),
            move |x: &Selector| match &x.quant {
                Quant::Indefinite(Count::AtLeast(n)) if one(n) == singular && x.chooser.is_none() && x.random.is_none() => {
                    Some((n.clone(), x.set.clone()))
                }
                _ => None,
            },
        )
    };
    let at_least = alt("at least", vec![at_least(&s.set_pl, false), at_least(&s.set_sg, true)]);
    let others = alt("others", vec![at_least, s.selector.clone()]);
    s.singular.skip(&lit("and")).then(&others).skip(&lit("attack")).map(
        move |(first, others)| {
            not_player(&first).then(|| {
                EventPattern::new(
                    vec![first],
                    vec![EventAction::Attacks { alone: false, defender: Vec::new(), with: Some(Box::new(others)) }],
                )
            })
        },
        move |p| match (p.subjects.as_slice(), p.actions.as_slice(), p.nth_each_turn, &p.during) {
            ([first], [EventAction::Attacks { alone: false, defender, with: Some(w) }], None, None)
                if defender.is_empty() && not_player(first) =>
            {
                Some((first.clone(), (**w).clone()))
            }
            _ => None,
        },
    )
}

/// The scope of a count of events: "each turn" (every turn), or the turns or steps it is
/// counted in, "during each opponent's turn" (603.2).
pub(crate) fn scope() -> P<Option<During>> {
    alt(
        "scope",
        vec![
            lit("each turn").to(None),
            lit("in a turn").to(None),
            during_phrase().map(|d| Some(Some(d)), |d: &Option<During>| d.clone()),
        ],
    )
}

/// The verbs whose events are counted per player: those a player does to one thing at a time.
fn counted(v: &Verb) -> bool {
    transitive(*v) && v.tail().is_empty() && v.entry().doer == ir::Doer::Player
}

/// "you cast your second spell each turn", "an opponent draws their second card each turn",
/// "you cast your first spell during each opponent's turn", "you cast your first instant or
/// sorcery spell from your graveyard each turn": the n-th such event in a turn.
fn ordinal_pattern(past: bool) -> P<EventPattern> {
    let s = selectors();
    let (you_form, third_form) = if past { (Inflection::Past, Inflection::Past) } else { (Inflection::Base, Inflection::Third) };
    let verbs = |f: Inflection| table("verb", Verb::ALL.iter().filter(|v| !own_rule(**v) && counted(v)).map(|v| (v.head(f), *v)).collect());
    let players = s.singular.map(
        |p| (p.set.domain == Domain::Player).then_some(p),
        |p| (p.set.domain == Domain::Player && *p != Selector::you()).then(|| p.clone()),
    );
    let subject = alt(
        "subject",
        vec![
            lit("you").to(Selector::you()).then(&verbs(you_form)).skip(&lit("your")),
            players.then(&verbs(third_form)).skip(&lit("their")),
        ],
    );
    subject.then(&ordinal()).then(&s.set_sg).then(&origin()).then(&scope()).map(
        |(((((subject, verb), n), set), from), during)| {
            (from.is_none() || verb.changes_zone()).then(|| {
                let what = Selector::new(Quant::Indefinite(Count::Exactly(Amount::Const(1))), set);
                let action = EventAction::Deed(Deed { verb, passive: false, args: moved(what, from) });
                let mut p = EventPattern::new(vec![subject], vec![action]);
                p.nth_each_turn = Some(n);
                p.during = during;
                p
            })
        },
        |p| {
            if !ordinal_inside(p) {
                return None;
            }
            let ([subject], [EventAction::Deed(d)], Some(n)) = (p.subjects.as_slice(), p.actions.as_slice(), p.nth_each_turn) else {
                return None;
            };
            let (what, from) = unmoved(&d.args)?;
            Some((((((subject.clone(), d.verb), n), what.set), from), p.during.clone()))
        },
    )
    .map(|p| p.typed().then_some(p), |p| Some(p.clone()))
}

/// Does [`ordinal_pattern`] spell this pattern ("you cast your second spell each turn")?
fn ordinal_inside(p: &EventPattern) -> bool {
    let one = Quant::Indefinite(Count::Exactly(Amount::Const(1)));
    p.nth_each_turn.is_some()
        && p.condition.is_none()
        && p.cause.is_none()
        && matches!(p.subjects.as_slice(), [s] if s.set.domain == Domain::Player)
        && matches!(p.actions.as_slice(), [EventAction::Deed(d)] if !d.passive && counted(&d.verb)
            && matches!(unmoved(&d.args), Some((o, from)) if o.quant == one && o.chooser.is_none() && o.random.is_none()
                && (from.is_none() || d.verb.changes_zone())))
}

/// Two or more verbs with one object: "cast or copy an instant or sorcery spell", "cycle or
/// discard a card". One action per verb, all on the same object.
fn shared_object(form: Form) -> P<Vec<EventAction>> {
    let f = inflection(form);
    let shares = |v: &Verb| transitive(*v) && v.tail().is_empty();
    let verbs = table("verb", regular().filter(shares).map(|v| (v.head(f), v)).collect());
    verbs.list("or").then(&selectors().selector).map(
        |(verbs, obj)| Some(verbs.into_iter().map(|v| EventAction::does(v, obj.clone())).collect()),
        |v: &Vec<EventAction>| {
            let parts: Vec<(Verb, &Selector)> = v.iter().map(only_object).collect::<Option<_>>()?;
            let obj = parts.first()?.1;
            (parts.len() >= 2 && parts.iter().all(|(_, o)| *o == obj))
                .then(|| (parts.iter().map(|(v, _)| *v).collect(), obj.clone()))
        },
    )
}

/// The participle of a passive said with "is", and what follows it: "exiled", "cast from a
/// graveyard", "tapped for mana", "attacked". The subject is what it is done to.
fn participle_phrase() -> P<Deed> {
    // "cast from a graveyard", "exiled from the battlefield": an action that moves its object
    // says where from (400.1)
    let movers = table(
        "participle",
        passives()
            .filter(|v| !becomes(*v) && v.changes_zone() && v.tail().is_empty())
            .map(|v| (v.phrase(Inflection::Participle), v))
            .collect(),
    );
    let cast = movers.then(&origin()).map(
        |(verb, from)| {
            let args = from.into_iter().map(|z| (Role::From, Arg::Zone(z))).collect();
            Some(Deed { verb, passive: true, args })
        },
        |d| match (d.passive, d.args.as_slice()) {
            (true, []) => Some((d.verb, None)),
            (true, [(Role::From, Arg::Zone(z))]) => Some((d.verb, Some(z.clone()))),
            _ => None,
        },
    );
    let words = table(
        "participle",
        passives()
            .filter(|v| !becomes(*v) && !(v.changes_zone() && v.tail().is_empty()))
            .map(|v| (v.phrase(Inflection::Participle), v))
            .collect(),
    );
    let done = words.map(
        |verb| Some(Deed { verb, passive: true, args: Vec::new() }),
        |d: &Deed| (d.passive && d.args.is_empty()).then_some(d.verb),
    );
    // "created under your control": under whose control it comes to be, where the action has
    // that role
    let under = lit("under").then_keep(&selectors().possessive).skip(&lit("control"));
    let done_under = words.then(&under).map(
        |(verb, who)| {
            let has_role = verb.entry().roles.iter().any(|(r, _)| *r == Role::Under);
            has_role.then(|| Deed { verb, passive: true, args: vec![(Role::Under, Arg::Noun(who))] })
        },
        |d: &Deed| match (d.passive, d.args.as_slice()) {
            (true, [(Role::Under, Arg::Noun(who))]) => Some((d.verb, who.clone())),
            _ => None,
        },
    );
    // Only explicit agent roles: the player instructed to act is not an effect source.
    let with_agent = |mut deed: Deed, who: Selector| {
        (deed.passive && !deed.args.iter().any(|(role, _)| *role == Role::By)
            && !deed.verb.kinds(Role::By).is_empty() && ir::lexicon::fills(&who, deed.verb, Role::By))
            .then(|| { deed.args.push((Role::By, Arg::Noun(who))); deed })
    };
    let agentless = alt("passive before agent", vec![cast.clone(), done.clone(), done_under.clone()]);
    let done_by = agentless.skip(&lit("by")).then(&selectors().selector).map(
        move |(deed, who)| with_agent(deed, who),
        move |d: &Deed| {
            let (Role::By, Arg::Noun(who)) = d.args.last()? else { return None };
            let mut base = d.clone(); base.args.pop();
            (with_agent(base.clone(), who.clone()).as_ref() == Some(d)).then(|| (base, who.clone()))
        },
    );
    // Oracle also names the permanent supplying champion with "with" (702.72c).
    let championed_with = lit(&Verb::Champion.phrase(Inflection::Participle)).skip(&lit("with"))
        .then(&selectors().selector).map(
            move |((), who)| with_agent(Deed { verb: Verb::Champion, passive: true, args: Vec::new() }, who), |_| None,
        ).alias();
    alt("participle", vec![cast, done, done_under, done_by, championed_with])
}

/// The be-verb of a passive, agreeing with its subject: "is"/"are", "was"/"were", "has
/// been"/"have been", and their negations.
fn be(tense: Tense, plural: bool, negated: bool) -> P<()> {
    lit(match (tense, plural, negated) {
        (Tense::Present, false, false) => "is",
        (Tense::Present, true, false) => "are",
        (Tense::Present, false, true) => "isn't",
        (Tense::Present, true, true) => "aren't",
        (Tense::Past, false, false) => "was",
        (Tense::Past, true, false) => "were",
        (Tense::Past, false, true) => "wasn't",
        (Tense::Past, true, true) => "weren't",
        (Tense::Perfect, false, false) => "has been",
        (Tense::Perfect, true, false) => "have been",
        (Tense::Perfect, false, true) => "hasn't been",
        (Tense::Perfect, true, true) => "haven't been",
    })
}

/// "a creature card is exiled", "no spells were cast", "you've been attacked": who does it is
/// left open (an empty subject list).
fn passive(tense: Tense, negated: bool) -> P<EventPattern> {
    let s = selectors();
    let you = match (tense, negated) {
        (Tense::Present, false) => "you are",
        (Tense::Present, true) => "you aren't",
        (Tense::Past, false) => "you were",
        (Tense::Past, true) => "you weren't",
        (Tense::Perfect, false) => "you've been",
        (Tense::Perfect, true) => "you haven't been",
    };
    let one = |subject: P<Selector>, plural: Option<bool>| {
        // does the verb agree with this subject?
        let fits = move |obj: &Selector| match plural {
            None => *obj == Selector::you(),
            // "no spells were cast": "no" takes the plural here
            Some(pl) => *obj != Selector::you() && (is_plural(obj) || obj.quant == Quant::No) == pl,
        };
        let done = subject.then(&participle_phrase()).map(
            |(obj, deed)| Some(EventPattern::new(vec![obj], vec![EventAction::Deed(deed)])),
            move |pat: &EventPattern| {
                let ([obj], [EventAction::Deed(deed)], None, None) =
                    (pat.subjects.as_slice(), pat.actions.as_slice(), pat.nth_each_turn, &pat.during)
                else {
                    return None;
                };
                fits(obj).then(|| (obj.clone(), deed.clone()))
            },
        );
        // "a permanent was returned this way": a zone change said with no destination (400.7),
        // as the reduced relative "cards returned this way" says it
        let returned = subject.skip(&lit(&Verb::Move.bare(Inflection::Participle))).map(
            |obj| Some(EventPattern::new(vec![obj], vec![EventAction::Deed(Deed { verb: Verb::Move, passive: true, args: Vec::new() })])),
            move |pat: &EventPattern| {
                let ([obj], [EventAction::Deed(Deed { verb: Verb::Move, passive: true, args })], None, None) =
                    (pat.subjects.as_slice(), pat.actions.as_slice(), pat.nth_each_turn, &pat.during)
                else {
                    return None;
                };
                (args.is_empty() && fits(obj)).then(|| obj.clone())
            },
        );
        // a zone change with its destination, still said of no subject: someone puts it there
        let put = subject.skip(&lit("put onto the battlefield")).map(
            |obj| Some(EventPattern::new(Vec::new(), vec![EventAction::PutsOntoBattlefield { what: Box::new(obj) }])),
            move |pat: &EventPattern| {
                let ([], [EventAction::PutsOntoBattlefield { what }], None, None) =
                    (pat.subjects.as_slice(), pat.actions.as_slice(), pat.nth_each_turn, &pat.during)
                else {
                    return None;
                };
                fits(what).then(|| (**what).clone())
            },
        );
        alt("passive", vec![returned, done, put])
    };
    alt(
        "passive",
        vec![
            one(s.singular.skip(&be(tense, false, negated)), Some(false)),
            one(s.plural.skip(&be(tense, true, negated)), Some(true)),
            one(lit(you).to(Selector::you()), None),
        ],
    )
}

/// "you had a land enter the battlefield under your control": the same event as "a land
/// entered the battlefield under your control", said from the player's side.
fn had_enter() -> P<EventPattern> {
    let s = selectors();
    let player = alt("player", vec![lit("you").to(Selector::you()), s.singular.clone()]);
    let enter = alt("enter", vec![lit("enter the battlefield"), lit("enter")]);
    player
        .skip(&lit("had"))
        .then(&s.selector)
        .skip(&enter)
        .skip(&lit("under"))
        .then(&s.possessive)
        .skip(&lit("control"))
        .map(
            |((_, what), under)| {
                Some(EventPattern::new(vec![what], vec![EventAction::under(Verb::Enter, under)]))
            },
            |_| None,
        )
        .alias()
}

/// An event clause in a tense: present for triggers, past and perfect for history conditions;
/// `negated` ("~ didn't attack", "you haven't cast a spell") only in the past and perfect.
fn clause(tense: Tense, negated: bool, next: bool) -> P<EventPattern> {
    let s = selectors();
    // "When you next cast a spell this turn,": a delayed trigger happens the next time (603.7), so
    // "next" limits a delayed trigger with a duration to the one time (603.7b); only a delayed
    // trigger takes it
    let adverb = if next { lit("next") } else { empty() };
    let mk = |subjects: P<Vec<Selector>>, form: Form, plural_verb: bool| {
        let actions = alt("actions", vec![shared_object(form), one_or_list(&action(form, plural_verb))]);
        subjects.skip(&adverb).then(&actions).map(
            |(subjects, actions)| Some(doing(subjects, actions)),
            |p| (p.nth_each_turn.is_none() && p.during.is_none()).then(|| said_doing(p)).flatten(),
        )
    };
    let you = || lit("you").to(vec![Selector::you()]);
    let singular = || one_or_list(&s.singular);
    let plural = || s.plural.map(|x| Some(vec![x]), |v: &Vec<Selector>| (v.len() == 1).then(|| v[0].clone()));
    let all = || alt("subject", vec![you(), singular(), plural()]);
    let player_or_object = alt("subject", vec![lit("you").to(Selector::you()), s.singular.clone()]);
    let with_you = |v: &Vec<Selector>| v.len() >= 2 && v.contains(&Selector::you());
    let mixed = || {
        player_or_object.list("or").map(move |v| with_you(&v).then_some(v), move |v: &Vec<Selector>| with_you(v).then(|| v.clone()))
    };
    let with = |subjects: P<Vec<Selector>>, aux: &str| subjects.skip(&lit(aux));
    if next {
        // "When you cast your next instant or sorcery spell this turn,": the trigger "when you
        // next cast an instant or sorcery spell" says (Twinferno)
        let your_next = lit("you cast your next").then_keep(&s.set_sg).map(
            |set| {
                let one = Selector::new(Quant::Indefinite(Count::Exactly(Amount::Const(1))), set);
                Some(doing(vec![Selector::you()], vec![EventAction::does(Verb::Cast, one)]))
            },
            |_| None,
        );
        // "next" stands after the subject of a verb; the clauses with no subject before their
        // verb have no place for it
        return alt(
            "event clause",
            vec![
                mk(singular(), Form::Third, false),
                mk(plural(), Form::Base, true),
                mk(you(), Form::Base, true),
                mk(mixed(), Form::Third, false),
                your_next.alias(),
            ],
        );
    }
    let mut v = vec![passive(tense, negated)];
    // "damage is prevented this way": no subject to agree with
    let prevented = match (tense, negated) {
        (Tense::Present, false) => Some("damage is prevented"),
        (Tense::Past, false) => Some("damage was prevented"),
        (Tense::Perfect, false) => Some("damage has been prevented"),
        _ => None,
    };
    if let Some(text) = prevented {
        v.push(lit(text).to(EventPattern::new(Vec::new(), vec![EventAction::DamagePrevented])));
    }
    // "damage from a red source is prevented this way": the prevented damage's source
    if tense == Tense::Present && !negated {
        v.push(lit("damage from").then_keep(&s.selector).skip(&lit("is prevented")).map(
            |from| {
                let mut p = EventPattern::new(Vec::new(), vec![EventAction::DamagePrevented]);
                p.cause = Some(EventCause::Source(Box::new(from)));
                Some(p)
            },
            |p| match (&p.cause, p.actions.as_slice()) {
                (Some(EventCause::Source(from)), [EventAction::DamagePrevented]) if p.subjects.is_empty() => Some((**from).clone()),
                _ => None,
            },
        ));
    }
    match (tense, negated) {
        (Tense::Present, _) => {
            let fixed = |text: &str, subjects: Vec<Selector>, actions: Vec<EventAction>| {
                lit(text).to(EventPattern::new(subjects, actions))
            };
            // Canonical passive has the recipient as subject; this equivalent Oracle
            // word order has the damage first (120.10), as on Rhino's Rampage.
            let damage_kind=alt("damage kind",vec![lit("combat damage").to(DamageKind::Combat),lit("noncombat damage").to(DamageKind::Noncombat),lit("damage").to(DamageKind::Any)]);
            v.push(lit("excess").then_keep(&damage_kind).skip(&lit("is dealt to")).then(&s.selector).map(
                |(kind,to)|Some(EventPattern::new(vec![to],vec![EventAction::IsDealtDamage {kind,excess:true,by:Vec::new()}])),
                |_|None,
            ).alias());
            // "combat damage is dealt to you or a planeswalker you control": the same word order
            // without "excess"
            v.push(damage_kind.skip(&lit("is dealt to")).then(&one_or_list(&s.selector)).map(
                |(kind,to)|Some(EventPattern::new(to,vec![EventAction::IsDealtDamage {kind,excess:false,by:Vec::new()}])),
                |_|None,
            ).alias());
            v.extend([
                // the Ring is not an object: the event is the player's
                fixed("the Ring tempts you", vec![Selector::you()], vec![EventAction::RingTempts]),
                fixed("players finish voting", Vec::new(), vec![EventAction::FinishVoting]),
                fixed(
                    "the final chapter ability of a Saga you control resolves",
                    Vec::new(),
                    vec![EventAction::FinalChapterResolves],
                ),
                fixed("day becomes night", Vec::new(), vec![EventAction::BecomesNight]),
                fixed("night becomes day", Vec::new(), vec![EventAction::BecomesDay]),
                fixed(
                    "you're dealt damage",
                    vec![Selector::you()],
                    vec![EventAction::IsDealtDamage { kind: DamageKind::Any, excess: false, by: Vec::new() }],
                ),
                together(),
                counters_removed(),
                caused(),
                // a card naming a pair ("Don & Raph") takes the plural verb
                mk(self_ref(), Form::Base, true).alias(),
                counters_put("is put on", "are put on"),
                mk(singular(), Form::Third, false),
                mk(plural(), Form::Base, true),
                mk(you(), Form::Base, true),
                // "you or a permanent you control becomes the target of …": a player and an
                // object among the subjects, the verb agreeing with the last
                mk(mixed(), Form::Third, false),
            ]);
        }
        (Tense::Past, false) => v.extend([
            had_enter(),
            counters_put("was put on", "were put on"),
            mk(singular(), Form::Past, false),
            mk(alt("subject", vec![you(), plural()]), Form::Past, true),
        ]),
        (Tense::Past, true) => v.push(mk(with(all(), "didn't"), Form::Base, false)),
        (Tense::Perfect, n) => {
            let (you_have, has, have) =
                if n { ("you haven't", "hasn't", "haven't") } else { ("you've", "has", "have") };
            v.extend([
                mk(lit(you_have).to(vec![Selector::you()]), Form::Participle, true),
                mk(with(singular(), has), Form::Participle, false),
                mk(with(plural(), have), Form::Participle, true),
            ]);
        }
    }
    alt("event clause", v)
}

/// Subjects doing actions. A spell or ability as the subject of what a player does is what
/// makes them do it, not who does it: "a spell or ability you control counters a spell" is the
/// pattern's cause, as in "a spell or ability an opponent controls causes you to discard a
/// card", with the player left open.
fn doing(subjects: Vec<Selector>, actions: Vec<EventAction>) -> EventPattern {
    let effect = |s: &Selector| {
        let facts = s.set.possibilities();
        !facts.is_empty() && facts.iter().all(|p| matches!(p.domain.sort, ir::Sort::Spell | ir::Sort::Ability))
    };
    let by_player = |a: &EventAction| {
        matches!(a.reading(), Some((verb, ir::SubjectIs::Doer)) if verb.entry().doer == ir::Doer::Player)
    };
    match subjects.as_slice() {
        [cause] if effect(cause) && actions.iter().all(by_player) => {
            let mut p = EventPattern::new(Vec::new(), actions);
            p.cause = Some(EventCause::Source(Box::new(cause.clone())));
            p
        }
        _ => EventPattern::new(subjects, actions),
    }
}

/// The subjects and actions [`doing`] reads a pattern from.
fn said_doing(p: &EventPattern) -> Option<(Vec<Selector>, Vec<EventAction>)> {
    match (&p.cause, p.subjects.is_empty()) {
        (None, false) => Some((p.subjects.clone(), p.actions.clone())),
        (Some(EventCause::Source(cause)), true) => {
            let subjects = vec![(**cause).clone()];
            (doing(subjects.clone(), p.actions.clone()) == *p).then(|| (subjects, p.actions.clone()))
        }
        _ => None,
    }
}

/// An event that would happen: what a replacement effect watches for (CR 614.1a). The event
/// clause after "would": "that creature would die", "you would draw a card", "~ would be put
/// into a graveyard from anywhere", "~ would be destroyed", "damage would be dealt to ~", "one or
/// more +1/+1 counters would be put on a creature you control".
pub fn would() -> P<EventPattern> {
    let s = selectors();
    let unmodified = |p: &EventPattern| p.nth_each_turn.is_none() && p.during.is_none() && p.condition.is_none();
    let plain = move |p: &EventPattern| unmodified(p) && p.cause.is_none();
    let you = lit("you").to(vec![Selector::you()]);
    let plural = s.plural.map(|x| Some(vec![x]), |v: &Vec<Selector>| (v.len() == 1).then(|| v[0].clone()));
    let subjects = alt("subjects", vec![you, one_or_list(&s.singular), plural]);
    let actions = alt("actions", vec![shared_object(Form::Infinitive), one_or_list(&action(Form::Infinitive, false))]);
    // "if you would draw a card except the first one you draw in each of your draw steps"
    let does = subjects.skip(&lit("would")).then(&actions).then(&draw_exception()).map(
        |((subjects, actions), except)| {
            let mut p = doing(subjects, actions);
            if let Some((nth, you)) = except {
                let draws = matches!(p.actions.as_slice(), [EventAction::Deed(d)] if d.verb == Verb::Draw && !d.passive);
                let said_of_you = matches!(p.subjects.as_slice(), [s] if s.quant == ir::Quant::Definite(ir::Ref::You));
                if !draws || said_of_you != you || p.subjects.len() != 1 { return None; }
                p.except_nth_in_step = Some((nth, Step::Draw));
            }
            Some(p)
        },
        move |p| {
            if !unmodified(p) { return None; }
            let except = match p.except_nth_in_step {
                None => None,
                Some((nth, Step::Draw)) => Some((nth, matches!(p.subjects.as_slice(), [s] if s.quant == ir::Quant::Definite(ir::Ref::You)))),
                Some(_) => return None,
            };
            let mut base = p.clone();
            base.except_nth_in_step = None;
            said_doing(&base).map(|said| (said, except))
        },
    );
    // "if a nontoken creature would enter and it wasn't cast": a condition on the event
    let and_if = crate::kernel::lazy("condition", || crate::condition::conditions().condition.clone());
    // "would be put into your graveyard from anywhere and it wasn't cycled": this zone change is
    // not its discard to pay a cycling cost (702.29c, 702.29f; Abandoned Sarcophagus rulings).
    // Said of the event, as "it wasn't cast" is of an entering: no window, which a condition
    // alone would ask for ("if it was cycled this turn")
    let it = Selector::new(Quant::Definite(Ref::Anaphor(Anaphor::It)), ObjectSet::of(Domain::Object));
    let cycled = EventPattern::new(vec![it], vec![EventAction::Deed(Deed { verb: Verb::Cycle, passive: true, args: Vec::new() })]);
    let not_cycled = lit("it wasn't cycled").to(Condition::Happened { pattern: cycled, window: None, times: None, tense: Tense::Past, negated: true });
    let and_if = alt("event condition", vec![and_if, not_cycled]);
    let does = does.then(&lit("and").then_keep(&and_if).opt()).map(
        |(mut p, c)| {
            // "that player or that planeswalker's controller would gain life": two subjects, not
            // one that is either
            if matches!(p.subjects.as_slice(), [s] if matches!(&s.quant, Quant::Definite(Ref::Either(v))
                if v.iter().any(|x| matches!(x.quant, Quant::Definite(Ref::ControllerOf(_))))))
            {
                return None;
            }
            p.condition = c.map(Box::new);
            Some(p)
        },
        |p| {
            let mut base = p.clone();
            let c = base.condition.take().map(|c| *c);
            Some((base, c))
        },
    );
    // "damage would be dealt to ~": the damage is the subject of the sentence, the one dealt it
    // the subject of the event
    let kind = alt(
        "damage kind",
        vec![
            lit("combat damage").to(DamageKind::Combat),
            lit("noncombat damage").to(DamageKind::Noncombat),
            lit("damage").to(DamageKind::Any),
        ],
    );
    let dealt = kind.skip(&lit("would be dealt to")).then(&one_or_list(&s.selector)).map(
        |(kind, to)| Some(EventPattern::new(to, vec![EventAction::IsDealtDamage { kind, excess: false, by: Vec::new() }])),
        move |p| match p.actions.as_slice() {
            [EventAction::IsDealtDamage { kind, excess: false, by }] if by.is_empty() && plain(p) && !p.subjects.is_empty() => Some((*kind, p.subjects.clone())),
            _ => None,
        },
    );
    // The effect is a cause, not a player or permanent called "effect".
    let by_effect = lit("an effect would").then_keep(&action(Form::Infinitive, false)).map(
        |action| {
            let mut p = EventPattern::new(Vec::new(), vec![action]);
            p.cause = Some(EventCause::Effect);
            Some(p)
        },
        move |p| match (p.subjects.as_slice(), p.actions.as_slice(), &p.cause) {
            ([], [action], Some(EventCause::Effect)) if unmodified(p) => Some(action.clone()),
            _ => None,
        },
    );
    let counters = counters_put("would be put on", "would be put on");
    // each role takes what the lexicon says, as in a trigger
    alt("would", vec![dealt, counters, does, by_effect]).map(|p| p.typed().then_some(p), |p| Some(p.clone()))
}

/// Is this a condition on the game state, which a state trigger can wait for (CR 603.8), rather
/// than on events or offers ("you cast ~", "you sacrifice ~")?
fn is_state(c: &Condition) -> bool {
    match c {
        Condition::Controls { .. }
        | Condition::Exists { .. }
        | Condition::Has { .. }
        | Condition::Is { .. }
        | Condition::Compare { .. }
        | Condition::MoreThan { .. }
        | Condition::Designation { .. } => true,
        Condition::And(v) | Condition::Or(v) => v.iter().all(is_state),
        _ => false,
    }
}

/// "while saddled" (of the subject: "while it's saddled"), "while you control a creature with
/// power 4 or greater".
fn with_condition(p: &P<EventPattern>, condition: &P<Condition>) -> P<EventPattern> {
    let it = Selector::new(Quant::Definite(Ref::Anaphor(Anaphor::It)), ObjectSet::of(Domain::Object));
    let it2 = it.clone();
    let elliptical = adjective_group().map(
        move |f| Some(Condition::Is { what: Box::new(it.clone()), pred: Predicate::Adjective(f), negated: false }),
        move |c| match c {
            Condition::Is { what, pred: Predicate::Adjective(f), negated: false } if **what == it2 => Some(f.clone()),
            _ => None,
        },
    );
    let cond = lit("while").then_keep(&alt("while", vec![elliptical, condition.clone()]));
    p.then(&cond.opt()).map(
        |(mut p, c)| {
            p.condition = c.map(Box::new);
            Some(p)
        },
        |p| {
            let mut base = p.clone();
            base.condition = None;
            Some((base, p.condition.as_deref().cloned()))
        },
    )
}

/// "except the first one they draw in each of their draw steps" (said of "you": "you draw …
/// your draw steps"): the subject's own draws, numbered within each such step. The ordinal and
/// whether it was said of "you".
fn draw_exception() -> P<Option<(u32, bool)>> {
    let except_draw = |you: bool| {
        let (they, their) = if you { ("you", "your") } else { ("they", "their") };
        lit("except the").then_keep(&ordinal())
            .skip(&lit(&format!("one {they} draw in each of {their} draw steps")))
            .map(move |n| Some((n, you)), move |(n, said_you): &(u32, bool)| (*said_you == you).then_some(*n))
    };
    alt("except", vec![
        except_draw(false).map(|e| Some(Some(e)), |e: &Option<(u32, bool)>| *e),
        except_draw(true).map(|e| Some(Some(e)), |e: &Option<(u32, bool)>| *e),
        empty().to(None),
    ])
}

/// A clause with its trailing modifiers: "for the first time each turn", "for the first time
/// during each of your turns", "during your turn".
fn pattern(tense: Tense, negated: bool, next: bool) -> P<EventPattern> {
    let nth = lit("for the").then_keep(&ordinal()).skip(&lit("time")).then(&scope()).map(
        |(n, d)| Some((Some(n), d)),
        |(n, d): &(Option<u32>, Option<During>)| n.map(|n| (n, d.clone())),
    );
    let during = during_phrase().map(|d| Some((None, Some(d))), |(n, d): &(Option<u32>, Option<During>)| if n.is_none() { d.clone() } else { None });
    let none = empty().to((None, None));
    let modifiers = alt("modifiers", vec![nth, during, none]);
    let general = clause(tense, negated, next).then(&draw_exception()).then(&modifiers).map(
        |((mut p, except), (n, d))| {
            // each event's roles take what the lexicon says ("whenever a creature dies", never
            // "whenever a player dies")
            if !p.typed() {
                return None;
            }
            if let Some((nth, you)) = except {
                // a draw by its subject, said with the pronoun that subject takes
                let draws = matches!(p.actions.as_slice(), [EventAction::Deed(d)] if d.verb == Verb::Draw && !d.passive);
                let said_of_you = matches!(p.subjects.as_slice(), [s] if s.quant == ir::Quant::Definite(ir::Ref::You));
                if !draws || said_of_you != you || p.subjects.len() != 1 || n.is_some() || d.is_some() { return None; }
                p.except_nth_in_step = Some((nth, Step::Draw));
            }
            p.nth_each_turn = n;
            p.during = d;
            Some(p)
        },
        |p| {
            // "your second spell each turn" carries its ordinal and scope inside the clause
            if ordinal_inside(p) {
                return None;
            }
            let except = match p.except_nth_in_step {
                None => None,
                Some((nth, Step::Draw)) => Some((nth, matches!(p.subjects.as_slice(), [s] if s.quant == ir::Quant::Definite(ir::Ref::You)))),
                Some(_) => return None,
            };
            let mut base = p.clone();
            base.during = None;
            base.nth_each_turn = None;
            base.except_nth_in_step = None;
            Some(((base, except), (p.nth_each_turn, p.during.clone())))
        },
    );
    let counted = match (tense, negated) {
        (Tense::Present, false) => Some(ordinal_pattern(false)),
        (Tense::Past, false) => Some(ordinal_pattern(true)),
        _ => None,
    };
    match counted {
        Some(c) => alt("pattern", vec![c, general]),
        None => general,
    }
}

/// After "during": "your turn", "an opponent's turn", "your main phase", "combat".
pub fn during() -> P<During> {
    let turn = selectors().possessive.skip(&lit("turn")).map(
        |p| Some(During::Turn(Box::new(p))),
        |d| match d {
            During::Turn(p) => Some((**p).clone()),
            _ => None,
        },
    );
    let step = span(true).map(
        |(s, o)| (s != Step::Untap && (s != Step::Turn || matches!(o,StepOwner::Player{next:true,..}|StepOwner::ThatTurns))).then_some(During::Step(s, o)),
        |d| match d {
            During::Step(s, o) if *s != Step::Untap && (*s != Step::Turn || matches!(o,StepOwner::Player{next:true,..}|StepOwner::ThatTurns)) => Some((*s, o.clone())),
            _ => None,
        },
    );
    let other_turns = lit("turns other than yours").to(During::NotTurn(Box::new(Selector::you())));
    // "each of your turns" counts the same turns as "your turn"
    let own_turns = selectors().possessive.skip(&lit("own"))
        .skip(&alt("turn", vec![lit("turn"), lit("turns")])).map(
            |p| Some(During::Turn(Box::new(p))), |_| None,
        ).alias();
    let each_of = lit("each of")
        .then_keep(&selectors().possessive)
        .skip(&lit("turns"))
        .map(|p| Some(During::Turn(Box::new(p))), |_| None)
        .alias();
    // "any turn you attacked with a Rogue"
    let turn_when = lit("any turn").then_keep(&crate::kernel::lazy("past event", || events().past.clone())).map(
        |p| Some(During::TurnWhen(Box::new(p))),
        |d| match d {
            During::TurnWhen(p) => Some((**p).clone()),
            _ => None,
        },
    );
    alt("during", vec![turn, own_turns, each_of, other_turns, lit("combat").to(During::Combat), step, turn_when])
}

/// "during your turn", "during combat", "any time a sorcery couldn't have been cast".
fn during_phrase() -> P<During> {
    alt(
        "during",
        vec![
            lit("during").then_keep(&during()),
            lit("any time a sorcery couldn't have been cast").to(During::NotSorceryTiming),
        ],
    )
}

/// A step or phase and which one: "your upkeep", "each end step", "the next end step", "combat
/// on your turn", "that player's next main phase".
///
/// ```text
/// step := possessive ["next"] noun | "combat on" possessive "turn" | "the" noun "of" selector
///       | ("each" | "the" | "the next" | "the next turn's" | "that turn's") noun
/// ```
pub fn step() -> P<(Step, StepOwner)> {
    span(false)
}

/// [`step`], and with `turn` also the turn itself and the untap step, which only points of time
/// name this way ("your next turn", "that turn", "its controller's next untap step").
pub fn span(turn: bool) -> P<(Step, StepOwner)> {
    let mut nouns = vec![
        lit("upkeep").to(Step::Upkeep),
        lit("draw step").to(Step::Draw),
        lit("end step").to(Step::End),
        lit("cleanup step").to(Step::Cleanup),
        lit("declare attackers step").to(Step::DeclareAttackers),
        lit("declare blockers step").to(Step::DeclareBlockers),
        lit("main phase").to(Step::Main),
        lit("first main phase").to(Step::FirstMain),
        lit("precombat main phase").to(Step::FirstMain).alias(),
        lit("second main phase").to(Step::SecondMain),
        lit("postcombat main phase").to(Step::SecondMain).alias(),
        lit("combat").to(Step::Combat),
        // the step by its name (511): "during the end of combat step"
        lit("end of combat step").to(Step::EndOfCombat),
    ];
    if turn {
        nouns.push(lit("untap step").to(Step::Untap));
        nouns.push(lit("turn").to(Step::Turn));
    }
    let noun = alt("step", nouns);
    let s = selectors();
    // "the upkeep of enchanted creature's controller": a possessive inside a possessive
    let double = |p: &Selector| matches!(p.quant, Quant::Definite(Ref::ControllerOf(_) | Ref::OwnerOf(_)));
    // a player's step is "<possessive> [next] <step>", except the two spelled apart below
    let apart = move |n: Step, who: &Selector, next: bool| !next && (n == Step::Combat || double(who));
    let players = s.possessive.then(&lit("next").opt()).then(&noun).map(
        move |((who, next), n)| {
            let next = next.is_some();
            (!apart(n, &who, next)).then(|| (n, StepOwner::Player { who: Box::new(who), next }))
        },
        move |(n, o)| match o {
            StepOwner::Player { who, next } if !apart(*n, who, *next) => {
                Some((((**who).clone(), next.then_some(())), *n))
            }
            _ => None,
        },
    );
    // combat of a player's turns: "combat on your turn", "combat on each opponent's turn"
    let on_turn = lit("combat on").then_keep(&s.possessive).skip(&lit("turn")).map(
        |who| Some((Step::Combat, StepOwner::Player { who: Box::new(who), next: false })),
        |(n, o)| match (n, o) {
            (Step::Combat, StepOwner::Player { who, next: false }) => Some((**who).clone()),
            _ => None,
        },
    );
    let of_player = lit("the").then_keep(&noun).skip(&lit("of")).then(&s.selector).map(
        |(n, p)| Some((n, StepOwner::Player { who: Box::new(p), next: false })),
        move |(n, o)| match o {
            StepOwner::Player { who, next: false } if *n != Step::Combat && double(who) => Some((*n, (**who).clone())),
            _ => None,
        },
    );
    let which = alt(
        "which",
        vec![
            lit("the next turn's").to(StepOwner::NextTurns),
            lit("that turn's").to(StepOwner::ThatTurns),
            lit("each").to(StepOwner::Each),
            lit("the next").to(StepOwner::The { next: true }),
            lit("the").to(StepOwner::The { next: false }),
        ],
    );
    // of turns only "the next turn" is said this way ("that turn" below; "each turn" is a count)
    let said = |n: Step, o: &StepOwner| n != Step::Turn || *o == StepOwner::The { next: true };
    let others = which.then(&noun).map(
        move |(o, n)| said(n, &o).then_some((n, o)),
        move |(n, o)| (!matches!(o, StepOwner::Player { .. } | StepOwner::This) && said(*n, o)).then(|| (o.clone(), *n)),
    );
    // "each of your postcombat main phases": every one, as "your" says already (there may be
    // several in a turn, CR 500.8)
    let plural_noun = alt(
        "steps",
        vec![
            lit("postcombat main phases").to(Step::SecondMain),
            lit("precombat main phases").to(Step::FirstMain),
            lit("main phases").to(Step::Main),
            lit("upkeeps").to(Step::Upkeep),
            lit("end steps").to(Step::End),
            lit("draw steps").to(Step::Draw),
        ],
    );
    let each_of_your = lit("each of your")
        .then_keep(&plural_noun)
        .map(|n| Some((n, StepOwner::Player { who: Box::new(Selector::you()), next: false })), |_| None)
        .alias();
    // "your first upkeep", "the first upkeep": the first of the game (103.6)
    let first_upkeep = alt("first", vec![
        s.possessive.skip(&lit("first upkeep")).map(
            |who| Some((Step::Upkeep, StepOwner::First { who: Some(Box::new(who)) })),
            |(n, o)| match (n, o) { (Step::Upkeep, StepOwner::First { who: Some(who) }) => Some((**who).clone()), _ => None },
        ),
        lit("the first upkeep").to((Step::Upkeep, StepOwner::First { who: None })),
    ]);
    // "the end step on your next turn": your next one
    let on_your_next_turn = lit("the")
        .then_keep(&noun)
        .skip(&lit("on your next turn"))
        .map(|n| Some((n, StepOwner::Player { who: Box::new(Selector::you()), next: true })), |_| None)
        .alias();
    let mut all = vec![on_turn, each_of_your, players, others, of_player, first_upkeep, on_your_next_turn];
    if turn {
        all.push(lit("that turn").to((Step::Turn, StepOwner::ThatTurns)));
    }
    alt("step", all)
}

/// A point of time: "end of turn", "end of combat", "your next turn", "the end of your next
/// turn", "your next end step".
///
/// ```text
/// time := "end of turn" | "end of combat" | ["the end of"] step-or-turn
/// ```
///
/// Without "the end of" it is where the step or turn begins; "the beginning of" says the same.
pub fn time() -> P<Time> {
    let this = |step| Time { end: true, step, whose: StepOwner::This };
    let span = span(true);
    let at = |end: bool, intro: P<()>| {
        intro.then_keep(&span).map(
            move |(step, whose)| Some(Time { end, step, whose }),
            move |t: &Time| (t.end == end && t.whose != StepOwner::This).then(|| (t.step, t.whose.clone())),
        )
    };
    alt(
        "time",
        vec![
            lit("end of turn").to(this(Step::Turn)),
            lit("end of combat").to(this(Step::Combat)),
            at(true, lit("the end of")),
            at(false, alt("beginning", vec![empty(), lit("the beginning of").alias()])),
        ],
    )
}

/// The word and the patterns of triggers that are all events under one word.
fn same_word(v: &[Trigger]) -> Option<(TriggerWord, Vec<EventPattern>)> {
    let events: Vec<(TriggerWord, &EventPattern)> = v
        .iter()
        .map(|t| match t {
            Trigger::Event { word, pattern } => Some((*word, pattern)),
            _ => None,
        })
        .collect::<Option<_>>()?;
    let word = events.first()?.0;
    (events.len() >= 2 && events.iter().all(|(w, _)| *w == word)).then(|| (word, events.into_iter().map(|(_, p)| p.clone()).collect()))
}

/// `condition`: the condition grammar, defined once the conditions are built.
pub(crate) fn build(condition: &P<Condition>) -> Events {
    let word = alt("when", vec![lit("when").to(TriggerWord::When), lit("whenever").to(TriggerWord::Whenever)]);
    let present = with_condition(&pattern(Tense::Present, false, false), condition);
    let event = word.then(&present).map(
        |(word, pattern)| Some(Trigger::Event { word, pattern }),
        |t| match t {
            Trigger::Event { word, pattern } => Some((*word, pattern.clone())),
            _ => None,
        },
    );
    // "At the beginning of the end of combat step" is the point "At end of combat" names (511.2),
    // which prints
    let said_at_end_of_combat = |step: Step, whose: &StepOwner| {
        step == Step::EndOfCombat
            && (*whose == StepOwner::Each || *whose == StepOwner::Player { who: Box::new(Selector::you()), next: false })
    };
    let at = lit("at the beginning of").then_keep(&step()).map(
        |(step, whose)| Some(Trigger::Step { step, whose }),
        move |t| match t {
            Trigger::Step { step, whose } if !said_at_end_of_combat(*step, whose) => Some((*step, whose.clone())),
            _ => None,
        },
    );
    // a state trigger: "When you control no Islands"
    let state = lit("when").then_keep(condition).map(
        |c| is_state(&c).then_some(Trigger::State(c)),
        |t| match t {
            Trigger::State(c) => Some(c.clone()),
            _ => None,
        },
    );
    // "At end of combat": as the end of combat step begins (CR 511.2)
    let end_of_combat =
        lit("at end of combat").to(Trigger::Step { step: Step::EndOfCombat, whose: StepOwner::Each });
    // "At end of combat on your turn"
    let end_of_your_combat = lit("at end of combat on your turn")
        .to(Trigger::Step { step: Step::EndOfCombat, whose: StepOwner::Player { who: Box::new(Selector::you()), next: false } });
    // "At the end of combat on your turn" is the same point
    let the_end_of_your_combat = lit("at the end of combat on your turn")
        .to(Trigger::Step { step: Step::EndOfCombat, whose: StepOwner::Player { who: Box::new(Selector::you()), next: false } })
        .alias();
    let one = alt("trigger", vec![event, at, end_of_your_combat, the_end_of_your_combat, end_of_combat, state]);
    let many = one.then(&lit("and").then_keep(&one).many()).map(
        |(first, rest)| {
            Some(if rest.is_empty() { first } else { Trigger::Any(std::iter::once(first).chain(rest).collect()) })
        },
        |t| match t {
            Trigger::Any(v) if v.len() >= 2 && !v.iter().any(|x| matches!(x, Trigger::Any(_))) && same_word(v).is_none() => {
                Some((v[0].clone(), v[1..].to_vec()))
            }
            Trigger::Any(_) => None,
            single => Some((single.clone(), Vec::new())),
        },
    );
    // "Whenever you cast a green spell or a Forest you control enters": the events a trigger waits
    // for, each its own clause with its own subject (603.2), under one word. "… and whenever …"
    // says the same and reads, but this is how most cards say it.
    let either = word.then(&present.list("or")).map(
        |(word, patterns)| {
            (patterns.len() >= 2)
                .then(|| Trigger::Any(patterns.into_iter().map(|pattern| Trigger::Event { word, pattern }).collect()))
        },
        |t| match t {
            Trigger::Any(v) => same_word(v),
            _ => None,
        },
    );
    let trigger = alt("trigger", vec![many, either]);
    Events {
        trigger,
        pattern: present,
        delayed: {
            let said = |next: bool| with_condition(&pattern(Tense::Present, false, next), condition);
            alt(
                "delayed event",
                vec![
                    said(false).map(|p| Some((p, false)), |(p, next): &(EventPattern, bool)| (!next).then(|| p.clone())),
                    said(true).map(|p| Some((p, true)), |(p, next): &(EventPattern, bool)| next.then(|| p.clone())),
                ],
            )
        },
        past: pattern(Tense::Past, false, false),
        perfect: pattern(Tense::Perfect, false, false),
        past_not: pattern(Tense::Past, true, false),
        perfect_not: pattern(Tense::Perfect, true, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::Outcome;
    use oracle::token::tokenize;

    #[test]
    fn r601_3_cast_origins_keep_the_coordinated_owned_zones() {
        let rule = rule_action(Form::Cant, true);
        let text = "cast spells from graveyards or libraries";
        let Outcome::Accepted { value: EventAction::Deed(deed), .. } = rule.parse_all(&tokenize(text)) else { panic!("must read"); };
        let zone = &deed.zone(Role::From).unwrap().zone;
        assert_eq!(zone.zones.items, vec![Zone::Graveyard, Zone::Library]);
        assert_eq!(zone.zones.conj, ir::Conj::Or);
        assert_eq!(*zone.owner.as_ref().unwrap().as_ref(), Selector::new(Quant::Each, ObjectSet::of(Domain::Player)));
        assert_eq!(rule.print_text(&EventAction::Deed(deed), false).as_deref(), Some(text));
    }

    #[test]
    fn triggers_round_trip() {
        let t = &events().trigger;
        for text in [
            "When ~ enters",
            "Whenever ~ attacks",
            "At the beginning of your upkeep",
            "When ~ dies",
            "Whenever ~ deals combat damage to a player",
            "At the beginning of your end step",
            "At the beginning of combat on your turn",
            "Whenever a land you control enters",
            "Whenever ~ enters or attacks",
            "When ~ leaves the battlefield",
            "Whenever you cast a noncreature spell",
            "Whenever you cast an instant or sorcery spell",
            "At the beginning of each upkeep",
            "Whenever you attack",
            "When ~ is turned face up",
            "At the beginning of each player's upkeep",
            "Whenever you gain life",
            "Whenever another creature you control enters",
            "When ~ is put into a graveyard from the battlefield",
            "Whenever ~ or another Ally you control enters",
            "Whenever one or more creatures you control attack",
            "Whenever a creature you control attacks alone",
            "When enchanted creature dies",
            "At the beginning of the next end step",
            "Whenever you cast your second spell each turn",
            "Whenever you draw your second card each turn",
            "Whenever ~ attacks for the first time each turn",
            "Whenever ~ blocks or becomes blocked by a creature",
            "Whenever ~ becomes the target of a spell or ability an opponent controls",
            "Whenever you sacrifice a creature",
            "Whenever you play a land",
            "Whenever one or more +1/+1 counters are put on ~",
            "Whenever a land enters during your turn",
            "When ~ enters and whenever you attack",
            "At the beginning of the upkeep of enchanted creature's controller",
            "Whenever a creature attacks you",
            "Whenever ~ deals noncombat damage to an opponent",
            "Whenever ~ attacks while saddled",
            "When you control no Islands",
            "Whenever you cast a spell that targets ~",
            "Whenever you cast or copy an instant or sorcery spell",
            "Whenever one or more cards leave your graveyard",
            "At end of combat",
            "Whenever ~ and at least two other creatures attack",
        ] {
            let toks = tokenize(text);
            match t.parse_all(&toks) {
                Outcome::Accepted { value, .. } => {
                    assert_eq!(t.print_text(&value, true).as_deref(), Some(text), "{value:#?}")
                }
                other => panic!("{text}: {other:#?}"),
            }
        }
    }

    fn reads(text: &str) -> bool {
        matches!(events().trigger.parse_all(&tokenize(text)), Outcome::Accepted { .. })
    }

    fn round_trips(text: &str) {
        let t = &events().trigger;
        match t.parse_all(&tokenize(text)) {
            Outcome::Accepted { value, .. } => assert_eq!(t.print_text(&value, true).as_deref(), Some(text), "{value:#?}"),
            other => panic!("{text}: {other:#?}"),
        }
    }

    /// "Except the first one they draw in each of their draw steps" (504.1): of a draw by the
    /// pattern's subject, with the pronoun that subject takes.
    #[test]
    fn r504_1_draws_except_the_first_of_each_draw_step() {
        for text in [
            "Whenever an opponent draws a card except the first one they draw in each of their draw steps",
            "Whenever you draw a card except the first one you draw in each of your draw steps",
            "When ~ enters and whenever an opponent draws a card except the first one they draw in each of their draw steps",
        ] {
            round_trips(text);
        }
        let Outcome::Accepted { value: ir::Trigger::Event { pattern, .. }, .. } =
            events().trigger.parse_all(&tokenize("Whenever an opponent draws a card except the first one they draw in each of their draw steps"))
        else { panic!("one reading") };
        assert_eq!((pattern.except_nth_in_step, pattern.nth_each_turn), (Some((1, Step::Draw)), None));
        for text in [
            "Whenever an opponent draws a card except the first one you draw in each of your draw steps",
            "Whenever you draw a card except the first one they draw in each of their draw steps",
            "Whenever an opponent casts a spell except the first one they draw in each of their draw steps",
            "Whenever an opponent draws a card except the first one they draw in each of their draw steps during your turn",
        ] {
            assert!(!reads(text), "{text}");
        }
    }

    /// An object that changes zones (400.1) comes from a zone and goes to one; the verbs that
    /// move an object say where from, whichever construction names them.
    #[test]
    fn r400_1_zone_changes_say_where_from_and_to() {
        for text in [
            "When ~ enters from a graveyard",
            "Whenever a creature enters from your graveyard",
            "Whenever a land you control enters from anywhere other than your hand or graveyard",
            "Whenever a creature is exiled from the battlefield",
            "When ~ is put into exile from the battlefield",
            "Whenever one or more cards are put into exile",
            "Whenever a permanent is returned to your hand",
            "Whenever a creature dies or is put into exile",
            "Whenever you play a land from exile",
            "Whenever you cast a spell from exile",
        ] {
            if reads(text) {
                round_trips(text);
            } else {
                panic!("{text} does not read");
            }
        }
        // "from anywhere" is not saying where from
        assert!(reads("Whenever one or more cards are put into a library from anywhere"));
        assert!(reads("Whenever a creature you control is put into the command zone from anywhere"));
        assert!(reads("Whenever a land you control enters from anywhere other than your hand or your graveyard"));
    }

    /// A delayed trigger an effect creates happens once, the next time (603.7): "when you next
    /// cast" says what "when you cast" does; a printed trigger does not take "next".
    #[test]
    fn r603_7b_a_delayed_event_may_say_next() {
        let delayed = &events().delayed;
        let text = "you next cast a spell with {X} in its mana cost";
        let plain = "you cast a spell with {X} in its mana cost";
        match (delayed.parse_all(&tokenize(text)), delayed.parse_all(&tokenize(plain))) {
            (Outcome::Accepted { value: (a, next), .. }, Outcome::Accepted { value: (b, plain_next), .. }) => {
                // the same event; "next" is the one time within the duration, not a spelling
                assert_eq!(a, b);
                assert!(next && !plain_next);
                assert_eq!(delayed.print_text(&(a, next), false).as_deref(), Some(text));
            }
            other => panic!("{other:#?}"),
        }
        assert!(!reads("When you next cast a spell"));
        assert!(!reads("Whenever you next cast a spell"));
    }

    /// A keyword action is an entry of the lexicon, so every construction says it: watched for
    /// (701.61, 701.59, 702.143a), a state a permanent acquires (702.122e), by whoever the entry
    /// says does it.
    #[test]
    fn r701_61_keyword_actions_are_events_by_their_entries() {
        for text in [
            "Whenever you forage",
            "Whenever you collect evidence",
            "Whenever you discover",
            "Whenever a player foretells a card",
            "Whenever ~ enlists a creature",
            "Whenever ~ becomes crewed for the first time each turn",
            "When ~ becomes plotted",
            "Whenever ~ phases out",
        ] {
            round_trips(text);
        }
        // a creature does not forage, a player does not train, and "is crewed" is a state
        for text in ["Whenever a creature forages", "Whenever you train", "Whenever you foretell a player"] {
            assert!(!reads(text), "{text} must not read");
        }
    }

    /// An object deals an amount of damage (120.1) and a player gains or loses an amount of life
    /// (119.3); one event can be about "3 or more" or "exactly 1" of it.
    #[test]
    fn r120_1_events_can_be_about_an_amount() {
        for text in [
            "Whenever ~ deals 4 or more damage",
            "Whenever a source you control deals exactly 1 damage to a permanent or player",
            "Whenever ~ deals 3 or more combat damage to a player",
            "Whenever you gain 3 or more life",
            "Whenever you lose exactly 1 life",
        ] {
            round_trips(text);
        }
        assert!(!reads("Whenever you draw 3 or more cards or more"));
        assert!(!reads("Whenever ~ dies 3 or more times"));
    }

    /// Damage recipients already own their coordination (120.1); complete event noun
    /// predicates must not create a second grouping route inside that coordination.
    #[test]
    fn r120_1_damage_recipients_keep_outer_coordination() {
        for text in [
            "deals damage to a creature or a player",
            "deals damage to a permanent or a player",
            "deals damage to a creature you control or a player",
        ] {
            let Outcome::Accepted { value: EventAction::DealsDamage { to: Some(to), .. }, .. } =
                action(Form::Third, false).parse_all(&tokenize(text)) else { panic!("must read uniquely: {text}"); };
            assert_eq!(to.items.len(), 2, "recipient alternatives stay separate: {text}");
            assert_eq!(to.conj, ir::Conj::Or);
        }
        let text = "deals damage to an opponent or a permanent an opponent controls";
        let Outcome::Accepted { value: EventAction::DealsDamage { to: Some(to), .. }, .. } =
            action(Form::Third, false).parse_all(&tokenize(text)) else { panic!("Torbran recipients must read uniquely"); };
        assert_eq!(to.items.len(), 2);
        assert!(to.items[0].set.controller.is_none(), "opponent arm has no permanent controller relation");
        let controller = to.items[1].set.controller.as_ref().expect("permanent arm retains opponent controller");
        assert_eq!(controller.who.as_ref(), &to.items[0]);
        round_trips("Whenever you cast a noncreature spell or a Dragon spell");
    }

    /// A permanent can arrive with a status (110.5, 614.12), but not a card in a hand.
    #[test]
    fn r614_12_a_permanent_enters_with_a_status() {
        for text in ["When ~ enters untapped", "Whenever a land you control enters tapped", "Whenever a creature you control enters attacking"] {
            round_trips(text);
        }
        for text in ["Whenever you draw tapped", "Whenever a creature dies tapped", "When ~ enters flying"] {
            assert!(!reads(text), "{text} must not read");
        }
    }

    /// An ability triggers on each of several events, each with its own subject and verb (603.2);
    /// "or" under one word and "and whenever" say the same ability, and "or" prints.
    #[test]
    fn r603_2_events_joined_by_or_are_one_ability() {
        for text in [
            "Whenever you cast a green spell or a Forest you control enters",
            "Whenever a creature dies or a creature card is put into a graveyard from anywhere",
            "Whenever day becomes night or night becomes day",
            "Whenever you or a permanent you control becomes the target of a spell or ability an opponent controls",
            "Whenever ~ and at least one other creature attack",
            "Whenever ~ deals damage to you or a permanent you control",
        ] {
            round_trips(text);
        }
        let t = &events().trigger;
        let and = t.parse_all(&tokenize("Whenever you cast a spell and whenever a land enters"));
        let or = t.parse_all(&tokenize("Whenever you cast a spell or a land enters"));
        match (and, or) {
            (Outcome::Accepted { value: a, .. }, Outcome::Accepted { value: o, .. }) => {
                assert_eq!(a, o);
                assert_eq!(t.print_text(&a, true).as_deref(), Some("Whenever you cast a spell or a land enters"));
            }
            other => panic!("{other:#?}"),
        }
        // different words keep "and"
        round_trips("When ~ enters and whenever you attack");
    }

    #[test]
    fn r603_2_each_event_of_a_disjunction_is_a_whole_event() {
        for text in [
            "Whenever you cast a spell or enters",
            "Whenever a player dies or a creature enters",
            "Whenever you or a permanent you control dies",
            "Whenever ~ and at least two other creature attack",
        ] {
            assert!(!reads(text), "{text} must not read");
        }
    }

    /// The n-th event of a kind in a turn (603.2) is counted over every turn or over the turns
    /// the text names; the same count is said with the ordinal before the thing or after "for
    /// the … time".
    #[test]
    fn r603_2_ordinal_events_are_counted_over_a_scope() {
        for text in [
            "Whenever you cast your first spell each turn",
            "Whenever you cast your first spell during each opponent's turn",
            "Whenever a player casts their second spell during their turn",
            "Whenever you draw your first card during your draw step",
            "Whenever you discard your second card each turn",
            "Whenever you cast your first instant or sorcery spell from your graveyard each turn",
            "Whenever you gain life for the first time during your turn",
            "Whenever ~ attacks for the first time each turn",
            "Whenever an opponent loses life for the first time during their turn",
        ] {
            round_trips(text);
        }
        assert!(reads("When you draw your third card in a turn"));
        assert!(reads("Whenever you gain life for the first time during each of your turns"));
        assert!(reads("Whenever you cast your first spell during each of your turns"));
    }

    /// CR 122.7: "When the Nth [kind] counter is put on" is about the counter that takes the
    /// object to N; "the last" counter removed leaves none.
    #[test]
    fn r122_7_nth_counter_events() {
        for text in [
            "When the fourth plan counter is put on ~",
            "When the tenth +1/+1 counter is put on ~",
            "When the last time counter is removed from ~",
            "When the third charge counter is removed from ~",
            "When one or more +1/+1 counters are put on ~",
        ] {
            round_trips(text);
        }
        for text in [
            "When the last plan counter is put on ~",
            "When the fourth plan counters are put on ~",
            "When the fourth one or more plan counters are put on ~",
            "When the fourth counter is put",
        ] {
            assert!(!reads(text), "{text} must not read");
        }
    }

    #[test]
    fn r603_2_ordinal_events_need_a_countable_thing_and_a_scope() {
        for text in [
            "Whenever you cast your fifth other player each turn",
            "Whenever you cast your first spell during",
            "Whenever you cast your first spell",
            "Whenever you cast your first spell from your library each turn each turn",
        ] {
            assert!(!reads(text), "{text} must not read");
        }
    }

    /// Only a verb that moves its object says where from; a player is not exiled, and a creature
    /// dies from the battlefield only (700.4).
    #[test]
    fn r400_1_origin_belongs_to_zone_changing_verbs_only() {
        for text in [
            "Whenever a creature dies from a graveyard",
            "Whenever you draw a card from exile",
            "Whenever a player is exiled from the battlefield",
            "Whenever a creature enters from",
        ] {
            assert!(!reads(text), "{text} must not read");
        }
    }
}

/// The event actions in the base form, for patterns built elsewhere: "would die", "would deal
/// damage to you" in replacement effects (`crate::statics`).
pub fn base_action() -> P<EventAction> {
    static A: std::sync::OnceLock<P<EventAction>> = std::sync::OnceLock::new();
    A.get_or_init(|| action(Form::Base, false)).clone()
}

/// The event actions as a rule about what can happen says them: [`Form::Cant`] ("can't attack
/// you", "can't be blocked by creatures with flying") or [`Form::Must`] ("must be blocked");
/// `plural`: the subject is plural.
pub fn rule_action(form: Form, plural: bool) -> P<EventAction> {
    static A: [std::sync::OnceLock<P<EventAction>>; 4] =
        [std::sync::OnceLock::new(), std::sync::OnceLock::new(), std::sync::OnceLock::new(), std::sync::OnceLock::new()];
    assert!(matches!(form, Form::Cant | Form::Must), "a rule's form");
    A[2 * (form == Form::Must) as usize + plural as usize].get_or_init(|| action(form, plural)).clone()
}
