//! The amount grammar: numbers in card text.
//!
//! ```text
//! amount    := literal | "X" | "the number of" counted(pl) | possessive stat | "the" stat "of" selector
//!            | aggregate | possessive ("life total" | "starting life total" | "speed")
//!            | possessive "devotion to" colors | "that much" | "that many" | recalled | tally
//!            | "twice" amount | "half" amount [", rounded up" | ", rounded down"]
//! counted(n):= set(n) | [kind] "counter(s) on" selector | [kind] "counters among" set(pl)
//!            | "basic land type(s) among" set(pl) | "differently named" set(pl)
//!            | [kind] "counter(s)" player "have" | [kind] "counter(s) removed this way"
//!            | "time(s)" selector "was kicked" | "color(s) of mana spent to cast" selector | …
//! tally     := "the amount of life" player "gained" window | "the damage dealt" ["to" selector] window
//!            | "the amount of mana spent to cast" selector | "the total amount of mana paid" window | …
//! window    := "this way" | "this turn" | "this game" | ε (the trigger event or the step just done)
//! for_each  := "for each" counted(sg) ["beyond the first"] | "for each of" selector(pl)
//!            | "for each 1 life" player "lost" window | "for each 1 damage dealt" window | …
//! ```

use crate::kernel::{P, Val, alt, forward, leaf, lit, table};
use crate::mana::mana_symbol;
use crate::selector::{counter_kind, is_plural, selectors, stat};
use crate::words::{Number, number_words, stat_amount};
use ir::action::PlayerCounter;
use ir::amount::{ManaMeasure, Moment, Occurrence, Payment, Recalled, Role, When};
use ir::lexicon::{Inflection, Verb};
use ir::selector::Anaphor;
use ir::{
    Aggregate, Amount, Atom, Characteristic, Color, Count, DamageKind, Domain, EventAction, EventPattern, Filter,
    ObjectSet, Quant, Ref, Selector, Status,
};
use oracle::token::Kind;
use std::sync::OnceLock;

pub struct Amounts {
    /// An amount as a noun phrase: after "equal to", "where X is", "is".
    pub amount: P<Amount>,
    /// "for each …": the multiplier spelling of a count.
    pub for_each: P<Amount>,
    /// What a plural count counts: "cards in your graveyard", "card types among cards in your
    /// graveyard", "+1/+1 counters on ~".
    pub counted: P<Amount>,
}

pub fn amounts() -> &'static Amounts {
    static A: OnceLock<Amounts> = OnceLock::new();
    A.get_or_init(build)
}

fn clitic_s() -> P<()> {
    leaf("'s", |k| matches!(k, Kind::Clitic(c) if c == "'s").then_some(()), |()| Some(Kind::Clitic("'s".into())))
}

fn it() -> Selector {
    Selector::new(Quant::Definite(Ref::Anaphor(Anaphor::It)), ObjectSet::of(Domain::Object))
}

/// "they" / "their" for objects ("deal damage equal to their power").
fn they_objects() -> Selector {
    Selector::new(Quant::Definite(Ref::Anaphor(Anaphor::They)), ObjectSet::of(Domain::Object))
}

fn they_player() -> Selector {
    Selector::new(Quant::Definite(Ref::Anaphor(Anaphor::They)), ObjectSet::of(Domain::Player))
}

/// "its", "their", or a selector with "'s": "~'s", "that creature's", "the sacrificed creature's";
/// a plural one takes the bare apostrophe ("the exiled cards'").
/// "his" and "her" (a character's pronoun for the object) read as "its" (kernel `is_pronoun_of`).
pub(crate) fn owner_of() -> P<Selector> {
    let apostrophe = leaf("'", |k| matches!(k, Kind::Clitic(c) if c == "'").then_some(()), |()| Some(Kind::Clitic("'".into())));
    let plural = |s: &Selector| crate::selector::is_plural(s);
    alt(
        "possessive",
        vec![
            lit("its").to(it()),
            lit("their").to(they_objects()),
            selectors().selector.skip(&clitic_s()).map(move |s| (!plural(&s)).then_some(s), move |s: &Selector| (!plural(s)).then(|| s.clone())),
            selectors().selector.skip(&apostrophe).map(move |s| plural(&s).then_some(s), move |s: &Selector| plural(s).then(|| s.clone())),
        ],
    )
}

/// An object reference ("he" / "she" are selectors too, a character's pronoun for "it").
fn object() -> P<Selector> {
    selectors().selector.clone()
}

/// Does this selector carry a postmodifier ("target creature you control", "the card exiled
/// this way")? Then a stat of it is spelled "the power of …", not with "'s".
pub(crate) fn has_post(s: &Selector) -> bool {
    let set = &s.set;
    let post_atom = |f: &Filter| {
        matches!(
            f,
            Filter::Atom(
                Atom::Keyword(_)
                    | Atom::Stat { .. }
                    | Atom::Counters { .. }
                    | Atom::Named(_)
                    | Atom::ExiledWith(_)
                    | Atom::Chosen(_)
                    | Atom::AttachedTo(_)
                    | Atom::Among(_)
                    | Atom::AmongSet(_)
                    | Atom::Reference(_)
            )
        ) || matches!(f, Filter::Atom(Atom::Did { when: When::ThisWay, .. }))
    };
    let filter_post = match &set.filter {
        Filter::All(v) => v.iter().any(post_atom),
        f => post_atom(f),
    };
    set.controller.is_some()
        || set.owner.is_some()
        || set.caster.is_some()
        || set.zone.is_some()
        || s.chooser.is_some()
        || s.random.is_some()
        || filter_post
}

/// A player as the subject of a past-tense verb: "you", "they", "that player", "your opponents".
fn past_subject() -> P<Selector> {
    let s = selectors();
    let player = |p: &P<Selector>| {
        p.map(
            |s| (s.set.domain == Domain::Player).then_some(s),
            |s| (s.set.domain == Domain::Player).then(|| s.clone()),
        )
    };
    alt(
        "player",
        vec![lit("you").to(Selector::you()), lit("they").to(they_player()), player(&s.singular), player(&s.plural)],
    )
}

/// The perfect: "you've", "they've", "your opponents have", "that player has".
fn perfect_subject() -> P<Selector> {
    let s = selectors();
    let player = |p: &P<Selector>, verb: &str| {
        p.map(
            |s| (s.set.domain == Domain::Player && s != Selector::you()).then_some(s),
            |s| (s.set.domain == Domain::Player && *s != Selector::you()).then(|| s.clone()),
        )
        .skip(&lit(verb))
    };
    alt(
        "player",
        vec![
            lit("you've").to(Selector::you()),
            lit("they've").to(they_player()),
            player(&s.singular, "has"),
            player(&s.plural, "have"),
        ],
    )
}

/// A player with "have" / "has": "you have", "your opponents have", "that player has".
fn player_has() -> P<Selector> {
    let s = selectors();
    let player = |p: &P<Selector>, verb: &str| {
        p.map(
            |s| (s.set.domain == Domain::Player && s != Selector::you()).then_some(s),
            |s| (s.set.domain == Domain::Player && *s != Selector::you()).then(|| s.clone()),
        )
        .skip(&lit(verb))
    };
    alt(
        "player has",
        vec![
            lit("you have").to(Selector::you()),
            lit("they have").to(they_player()),
            player(&s.singular, "has"),
            player(&s.plural, "have"),
        ],
    )
}

/// "this way", "this turn", "this game".
fn window() -> P<When> {
    alt(
        "window",
        vec![
            lit("this way").to(When::ThisWay),
            lit("this turn").to(When::ThisTurn),
            lit("this game").to(When::ThisGame),
        ],
    )
}

/// A tally and its window; no window is the trigger event or the step just done, and then the
/// tally must not stop right before a window (that reading belongs to the window).
fn windowed<T: Val>(body: &P<T>, explicit_only: bool) -> P<(T, When)> {
    let explicit = body.then(&window());
    if explicit_only {
        return explicit;
    }
    let guard = alt("window", vec![lit("this way"), lit("this turn"), lit("this game")]);
    let context =
        body.not_before(&guard).map(|b| Some((b, When::Context)), |(b, w)| (*w == When::Context).then(|| b.clone()));
    alt("windowed", vec![explicit, context])
}

/// A whole unit boxed into an amount, for the one-selector variants.
fn boxed(p: &P<Selector>, mk: fn(Box<Selector>) -> Amount, un: fn(&Amount) -> Option<&Selector>) -> P<Amount> {
    p.map(move |s| Some(mk(Box::new(s))), move |a| un(a).cloned())
}

/// "your commander", "a commander": a commander card (CR 903.3).
fn commander() -> P<Selector> {
    let commander_set = |owner: Option<Selector>| {
        let mut set = ObjectSet::of(Domain::Card);
        set.filter = Filter::All(vec![Filter::Atom(Atom::Status(Status::Commander))]);
        set.owner = owner.map(|who| ir::Relation { who: Box::new(who), negated: false, past: false });
        set
    };
    // the possessive's "a" ("a graveyard") is the indefinite "a commander" here
    let a_player = Selector::new(Quant::Indefinite(Count::Exactly(Amount::Const(1))), ObjectSet::of(Domain::Player));
    let yours = selectors().possessive.skip(&lit("commander")).map(
        move |p| (p != a_player).then(|| Selector::new(Quant::The, commander_set(Some(p)))),
        move |s| {
            let who = s.set.owner.as_ref()?.who.as_ref().clone();
            (s.quant == Quant::The && *s == Selector::new(Quant::The, commander_set(Some(who.clone())))).then_some(who)
        },
    );
    let one = Selector::new(Quant::Indefinite(Count::Exactly(Amount::Const(1))), commander_set(None));
    alt("commander", vec![yours, alt("a", vec![lit("a")]).skip(&lit("commander")).to(one)])
}

/// The objects a selector stands for, as a set.
fn members(s: Selector) -> ObjectSet {
    let mut set = ObjectSet::of(Domain::Object);
    set.filter = Filter::All(vec![Filter::Atom(Atom::Among(Box::new(s)))]);
    set
}

/// The inverse of [`members`].
fn unmembers(set: &ObjectSet) -> Option<Selector> {
    match &set.filter {
        Filter::All(v) if *set == members_shape(set) => match v.as_slice() {
            [Filter::Atom(Atom::Among(s))] => Some((**s).clone()),
            _ => None,
        },
        _ => None,
    }
}

fn members_shape(set: &ObjectSet) -> ObjectSet {
    ObjectSet { filter: set.filter.clone(), ..ObjectSet::of(Domain::Object) }
}

/// The set "those creatures", "them" stand for, as a set: its members are the selector's.
fn among_selector() -> P<ObjectSet> {
    selectors().selector.map(|s| is_plural(&s).then(|| members(s)), |set| unmembers(set).filter(is_plural))
}

/// A plural set, or a plural selector standing for its members ("those creatures", "them").
fn plural_group() -> P<ObjectSet> {
    alt("plural set", vec![selectors().set_pl.clone(), among_selector()])
}

fn one_of(set: ObjectSet) -> Selector {
    Selector::new(Quant::Indefinite(Count::Exactly(Amount::Const(1))), set)
}

/// The set of a selector "a <set>" (the counted slot of a [`Amount::Participants`] pattern).
fn set_of_one(s: &Selector) -> Option<ObjectSet> {
    (s.quant == Quant::Indefinite(Count::Exactly(Amount::Const(1))) && s.chooser.is_none() && s.random.is_none())
        .then(|| s.set.clone())
}

/// The one subject and one action of a pattern without other modifiers.
fn simple(p: &EventPattern) -> Option<(&Selector, &EventAction)> {
    match (p.subjects.as_slice(), p.actions.as_slice(), p.nth_each_turn, &p.during) {
        ([s], [a], None, None) => Some((s, a)),
        _ => None,
    }
}

/// Members of a set that took part in an event: "creatures that died this turn", "opponents
/// who lost life this turn" (the subject), "cards you've drawn this turn", "opponents you
/// attacked this turn" (the object). The window is required: without it, "this turn" after the
/// count would also read as the duration of the instruction.
fn participants(set: &P<ObjectSet>) -> P<Amount> {
    // subject relatives ("creatures that died this turn") are the set's own (selector Atom::Did)
    let mut v = Vec::new();
    // object relatives: "cards you've drawn", "opponents you attacked"; the perfect is Oracle's
    // spelling after a player for casting, drawing, discarding and sacrificing
    // (the verb, whether the perfect is the spelling printed)
    let verbs = [(Verb::Cast, true), (Verb::Draw, true), (Verb::Discard, true), (Verb::Sacrifice, true), (Verb::Attack, false)];
    // "opponents you attacked": the one attacked; the others, what it is done to
    let mk = |verb: Verb, o: Selector| match verb {
        Verb::Attack => EventAction::Attacks { alone: false, defender: vec![o], with: None },
        verb => EventAction::does(verb, o),
    };
    fn un(verb: Verb, a: &EventAction) -> Option<&Selector> {
        match (verb, a) {
            (Verb::Attack, EventAction::Attacks { alone: false, defender, with: None }) if defender.len() == 1 => Some(&defender[0]),
            (Verb::Attack, _) => None,
            (verb, a) => match a.deed(verb)? {
                ir::Deed { passive: false, args, .. } => match args.as_slice() {
                    [(ir::Role::What, ir::Arg::Noun(what))] => Some(what),
                    _ => None,
                },
                _ => None,
            },
        }
    }
    // the passive names no player: "spells cast this turn" (by anyone); "this way" after a bare
    // participle is the selector's ("cards drawn this way")
    let anyone = one_of(ObjectSet::of(Domain::Player));
    let not_this_way = alt("window", vec![lit("this turn").to(When::ThisTurn), lit("this game").to(When::ThisGame)]);
    for (verb, perfect) in verbs {
        let (past, participle) = (verb.head(Inflection::Past), verb.head(Inflection::Participle));
        let simple_past = past_subject().skip(&lit(&past));
        let perfect_form = perfect_subject().skip(&lit(&participle));
        let subject = if perfect {
            alt("player and verb", vec![perfect_form, simple_past.alias()])
        } else {
            alt("player and verb", vec![simple_past, perfect_form.alias()])
        };
        let (anyone, anyone2) = (anyone.clone(), anyone.clone());
        v.push(set.skip(&lit(&participle)).then(&not_this_way).map(
            move |(set, when)| {
                Some(Amount::Participants {
                    pattern: Box::new(EventPattern::new(vec![anyone.clone()], vec![mk(verb, one_of(set))])),
                    counted: Role::Object,
                    when,
                })
            },
            move |a| match a {
                Amount::Participants { pattern, counted: Role::Object, when } if *when != When::ThisWay => {
                    let (subject, action) = simple(pattern)?;
                    (*subject == anyone2).then_some(())?;
                    Some((set_of_one(un(verb, action)?)?, *when))
                }
                _ => None,
            },
        ));
        v.push(set.then(&subject).then(&not_this_way).map(
            move |((set, subject), when)| {
                Some(Amount::Participants {
                    pattern: Box::new(EventPattern::new(vec![subject], vec![mk(verb, one_of(set))])),
                    counted: Role::Object,
                    when,
                })
            },
            move |a| match a {
                Amount::Participants { pattern, counted: Role::Object, when } if *when != When::ThisWay => {
                    let (subject, action) = simple(pattern)?;
                    Some(((set_of_one(un(verb, action)?)?, subject.clone()), *when))
                }
                _ => None,
            },
        ));
    }
    alt("participants", v)
}

fn counted(n: Number) -> P<Amount> {
    let s = selectors();
    let sg = n == Number::Sg;
    let word = |one: &str, many: &str| lit(if sg { one } else { many });
    let set = match n {
        // only after "for each": unions spelled as after "each"
        Number::Sg => s.set_each.clone(),
        Number::Pl => s.set_pl.clone(),
    };
    let count = set.map(
        |set| Some(Amount::Count(Box::new(set))),
        |a| match a {
            Amount::Count(set) => Some((**set).clone()),
            _ => None,
        },
    );
    let counters = counter_kind().opt().skip(&word("counter on", "counters on")).then(&s.selector).map(
        |(kind, on)| Some(Amount::Counters { on: Box::new(on), kind }),
        |a| match a {
            Amount::Counters { on, kind } if on.set.domain != Domain::Player => Some((kind.clone(), (**on).clone())),
            _ => None,
        },
    );
    // counters summed over a set: "counters among permanents you control"; "on" is an alternate
    // spelling ("+1/+1 counters on creatures you control")
    let among_word =
        alt("among", vec![word("counter among", "counters among"), word("counter on", "counters on").alias()]);
    let counters_among = counter_kind().opt().skip(&among_word).then(&s.set_pl).map(
        |(kind, among)| Some(Amount::CountersAmong { among: Box::new(among), kind }),
        |a| match a {
            Amount::CountersAmong { among, kind } => Some((kind.clone(), (**among).clone())),
            _ => None,
        },
    );
    // "experience counters you have", "poison counter your opponents have"
    let player_counters = counter_kind().skip(&word("counter", "counters")).then(&player_has()).map(
        |(kind, player)| Some(Amount::PlayerCounters { player: Box::new(player), kind: PlayerCounter::Named(kind) }),
        |a| match a {
            Amount::PlayerCounters { player, kind: PlayerCounter::Named(kind) } => {
                Some((kind.clone(), (**player).clone()))
            }
            _ => None,
        },
    );
    // "counters removed this way"; with no window, the removal just paid ("Remove X ki counters
    // from this creature: For each counter removed, …")
    let removed = windowed(&counter_kind().opt().skip(&word("counter removed", "counters removed")), false).map(
        |(kind, when)| Some(Amount::CountersRemoved { kind, when }),
        |a| match a {
            Amount::CountersRemoved { kind, when } => Some((kind.clone(), *when)),
            _ => None,
        },
    );
    let what = |one: &str, many: &str, c: Characteristic| lit(if sg { one } else { many }).to(c);
    let characteristic = alt(
        "characteristic",
        vec![
            what("color", "colors", Characteristic::Color),
            what("card type", "card types", Characteristic::CardType),
            what("basic land type", "basic land types", Characteristic::BasicLandType),
            what("creature type", "creature types", Characteristic::CreatureType),
            what("different power", "different powers", Characteristic::Power),
            what("different mana value", "different mana values", Characteristic::ManaValue),
            // "five or more mana values among cards in your graveyard": the same count
            what("mana value", "mana values", Characteristic::ManaValue).alias(),
            what("permanent type", "permanent types", Characteristic::PermanentType),
            what("planeswalker type", "planeswalker types", Characteristic::PlaneswalkerType),
            what("different kind of counter", "different kinds of counters", Characteristic::CounterKind),
        ],
    );
    // "colors it is", "colors that spell is"; "was" for an object gone ("colors that creature
    // was")
    let is = alt("is", vec![lit("is"), lit("was").alias()]);
    let colors_of = lit("colors").then_keep(&object()).skip(&is).map(
        move |s| (!sg).then(|| Amount::Colors(Box::new(s))),
        move |a| match a {
            Amount::Colors(s) if !sg => Some((**s).clone()),
            _ => None,
        },
    );
    // "red mana symbols in the mana costs of permanents you control", "white mana symbol in its
    // mana cost"
    let color = table("color", Color::ALL.iter().map(|c| (c.word().to_string(), *c)).collect());
    let in_costs = alt(
        "mana costs",
        vec![
            lit("in the mana costs of").then_keep(&plural_group()),
            lit("in").then_keep(&owner_of()).skip(&lit("mana cost")).map(|s| Some(members(s)), unmembers),
        ],
    );
    let symbols = color.skip(&word("mana symbol", "mana symbols")).then(&in_costs).map(
        |(color, among)| Some(Amount::ManaSymbols { color, among: Box::new(among) }),
        |a| match a {
            Amount::ManaSymbols { color, among } => Some((*color, (**among).clone())),
            _ => None,
        },
    );
    let distinct = characteristic.skip(&lit("among")).then(&plural_group()).map(
        |(what, among)| Some(Amount::Distinct { what, among: Box::new(among) }),
        |a| match a {
            Amount::Distinct { what, among } if *what != Characteristic::Name => Some((*what, (**among).clone())),
            _ => None,
        },
    );
    // "differently named lands you control": the distinct names among them
    let named = lit("differently named").then_keep(&s.set_pl).map(
        |among| Some(Amount::Distinct { what: Characteristic::Name, among: Box::new(among) }),
        |a| match a {
            Amount::Distinct { what: Characteristic::Name, among } => Some((**among).clone()),
            _ => None,
        },
    );
    // "colors of mana spent to cast ~", "{S} spent to cast ~"
    let spent_to_cast = lit("spent to cast").then_keep(&object());
    let colors_spent = word("color of mana", "colors of mana").then_keep(&spent_to_cast).map(
        |spell| Some(Amount::ManaSpent { spell: Box::new(spell), measure: ManaMeasure::Colors }),
        |a| match a {
            Amount::ManaSpent { spell, measure: ManaMeasure::Colors } => Some((**spell).clone()),
            _ => None,
        },
    );
    // "times it was kicked", "times ~ has mutated", "times you've cast your commander from the
    // command zone this game", "times it's been cast from the command zone this game"
    let times = word("time", "times");
    let occurrence =
        alt("occurrence", vec![lit("was kicked").to(Occurrence::Kicked), lit("has mutated").to(Occurrence::Mutated)]);
    let fact = times.then_keep(&object()).then(&occurrence).map(
        |(what, fact)| Some(Amount::Times { what: Box::new(what), fact }),
        |a| match a {
            Amount::Times { what, fact: fact @ (Occurrence::Kicked | Occurrence::Mutated) } => {
                Some(((**what).clone(), fact.clone()))
            }
            _ => None,
        },
    );
    let from_command_zone = lit("from the command zone this game");
    let cast_by = alt("player", vec![lit("you've").to(Selector::you()), lit("they've").to(they_player())])
        .skip(&lit("cast"))
        .then(&commander())
        .skip(&from_command_zone)
        .map(
            |(by, what)| {
                Some(Amount::Times {
                    what: Box::new(what),
                    fact: Occurrence::CastFromCommandZone { by: Some(Box::new(by)) },
                })
            },
            |a| match a {
                Amount::Times { what, fact: Occurrence::CastFromCommandZone { by: Some(by) } } => {
                    Some(((**by).clone(), (**what).clone()))
                }
                _ => None,
            },
        );
    let been_cast = object().skip(&clitic_s()).skip(&lit("been cast")).skip(&from_command_zone).map(
        |what| Some(Amount::Times { what: Box::new(what), fact: Occurrence::CastFromCommandZone { by: None } }),
        |a| match a {
            Amount::Times { what, fact: Occurrence::CastFromCommandZone { by: None } } => Some((**what).clone()),
            _ => None,
        },
    );
    let replicate_times = times.clone().skip(&lit("its replicate cost was paid")).to(Amount::Times {
        what: Box::new(Selector::new(ir::Quant::Definite(ir::Ref::This), ObjectSet::of(Domain::Spell))),
        fact: Occurrence::Replicated,
    });
    let cast_times = times.then_keep(&alt("commander casts", vec![cast_by, been_cast]));
    // "opponents you have", "opponent you have": all of them
    let you_have = set
        .skip(&lit("you have"))
        .map(|set| (set.domain == Domain::Player).then(|| Amount::Count(Box::new(set))), |_: &Amount| None)
        .alias();
    alt(
        if sg { "counted" } else { "counted (plural)" },
        vec![
            counters,
            counters_among,
            player_counters,
            removed,
            distinct,
            colors_of,
            symbols,
            named,
            colors_spent,
            fact,
            cast_times,
            replicate_times,
            participants(&set),
            count,
            you_have,
        ],
    )
}

fn colors() -> P<Vec<Color>> {
    let color = table("color", Color::ALL.iter().map(|c| (c.word().to_string(), *c)).collect());
    alt("colors", vec![color.map(|c| Some(vec![c]), |v| (v.len() == 1).then(|| v[0])), color.list("and")])
}

fn energy_symbol() -> P<()> {
    leaf("{E}", |k| matches!(k, Kind::Symbol(s) if s == "E").then_some(()), |()| Some(Kind::Symbol("E".into())))
}

/// The tallies of what happened: life gained or lost, damage dealt or prevented, mana spent or
/// paid, as noun phrases ("the amount of life you gained this turn").
fn tallies() -> Vec<P<Amount>> {
    let s = selectors();
    let gained = alt("gained or lost", vec![lit("gained").to(true), lit("lost").to(false)]);
    // with a player: "the amount of life you gained this turn"; "the life they lost", "the
    // total amount of life your opponents lost" and the perfect are alternate spellings
    let who_gained =
        alt("player and verb", vec![past_subject().then(&gained), perfect_subject().then(&gained).alias()]);
    let life_intro = alt(
        "the amount of life",
        vec![lit("the amount of life"), lit("the life").alias(), lit("the total amount of life").alias()],
    );
    let life_player = windowed(&life_intro.then_keep(&who_gained), false).map(
        |((player, gained), when)| Some(Amount::LifeChange { player: Some(Box::new(player)), gained, when }),
        |a| match a {
            Amount::LifeChange { player: Some(p), gained, when } => Some((((**p).clone(), *gained), *when)),
            _ => None,
        },
    );
    // without one: "the life lost this way"; "the total life lost by your opponents this turn"
    let life_none = windowed(&lit("the life").then_keep(&gained), false).map(
        |(gained, when)| Some(Amount::LifeChange { player: None, gained, when }),
        |a| match a {
            Amount::LifeChange { player: None, gained, when } => Some((*gained, *when)),
            _ => None,
        },
    );
    let life_by = windowed(
        &alt("the life", vec![lit("the life"), lit("the total life")])
            .then_keep(&gained)
            .skip(&lit("by"))
            .then(&past_subject()),
        false,
    )
    .map(
        |((gained, player), when)| Some(Amount::LifeChange { player: Some(Box::new(player)), gained, when }),
        |_: &Amount| None,
    )
    .alias();

    // damage: "the damage dealt this way", "the amount of damage ~ dealt to that player", "the
    // excess damage dealt this way"; "the amount of damage dealt" is an alternate spelling
    let kind = alt(
        "damage",
        vec![
            lit("damage").to(DamageKind::Any),
            lit("combat damage").to(DamageKind::Combat),
            lit("noncombat damage").to(DamageKind::Noncombat),
        ],
    );
    let damage = lit("excess").opt().map(|e| Some(e.is_some()), |e| Some(e.then_some(()))).then(&kind);
    let to = lit("to").then_keep(&s.selector).opt();
    let passive = windowed(
        &alt("the", vec![lit("the"), lit("the amount of").alias(), lit("the total amount of").alias()])
            .then_keep(&damage)
            .skip(&lit("dealt"))
            .then(&to),
        false,
    )
    .map(
        |(((excess, kind), to), when)| Some(Amount::Damage { by: None, to: to.map(Box::new), kind, excess, when }),
        |a| match a {
            Amount::Damage { by: None, to, kind, excess, when } => {
                Some((((*excess, *kind), to.as_deref().cloned()), *when))
            }
            _ => None,
        },
    );
    let active = windowed(
        &alt("the amount of", vec![lit("the amount of"), lit("the total amount of").alias()])
            .then_keep(&damage)
            .then(&object())
            .skip(&lit("dealt"))
            .then(&to),
        false,
    )
    .map(
        |((((excess, kind), by), to), when)| {
            Some(Amount::Damage { by: Some(Box::new(by)), to: to.map(Box::new), kind, excess, when })
        },
        |a| match a {
            Amount::Damage { by: Some(by), to, kind, excess, when } => {
                Some(((((*excess, *kind), (**by).clone()), to.as_deref().cloned()), *when))
            }
            _ => None,
        },
    );
    // "that excess damage": the trigger's ("Whenever ~ deals excess damage …"); Oracle's spelling
    // of the excess damage from context, before "the amount of excess damage dealt"
    let that_excess = lit("that excess damage").to(Amount::Damage {
        by: None,
        to: None,
        kind: DamageKind::Any,
        excess: true,
        when: When::Context,
    });
    let prevented = windowed(
        &alt("the damage", vec![lit("the damage"), lit("the amount of damage").alias()]).skip(&lit("prevented")),
        false,
    )
    .map(
        |((), when)| Some(Amount::DamagePrevented { when }),
        |a| match a {
            Amount::DamagePrevented { when } => Some(((), *when)),
            _ => None,
        },
    );

    // mana: "the amount of mana spent to cast ~", "the amount of {S} spent to cast ~"
    let measure = alt(
        "mana",
        vec![
            lit("mana").to(ManaMeasure::Total),
            mana_symbol().map(
                |m| Some(ManaMeasure::Symbol(m)),
                |m| match m {
                    ManaMeasure::Symbol(m) => Some(*m),
                    _ => None,
                },
            ),
        ],
    );
    let spent = lit("the amount of").then_keep(&measure).skip(&lit("spent to cast")).then(&object()).map(
        |(measure, spell)| Some(Amount::ManaSpent { spell: Box::new(spell), measure }),
        |a| match a {
            Amount::ManaSpent { spell, measure: m @ (ManaMeasure::Total | ManaMeasure::Symbol(_)) } => {
                Some((*m, (**spell).clone()))
            }
            _ => None,
        },
    );
    // paid: "the total amount of mana paid this way", "the amount of mana that player paid this
    // way", "the amount of {E} paid this way", "the life paid as it entered"
    let as_entered = alt("as it entered", vec![lit("as it entered"), lit("as ~ entered").alias()]).to(When::AsEntered);
    let paid_when = alt("window", vec![window(), as_entered]);
    let payment = alt("payment", vec![lit("mana").to(Payment::Mana), energy_symbol().to(Payment::Energy)]);
    let paid_none = alt(
        "the total amount of",
        vec![
            lit("the total amount of").then_keep(&lit("mana").to(Payment::Mana)),
            lit("the amount of").then_keep(&energy_symbol().to(Payment::Energy)),
            lit("the amount of mana").to(Payment::Mana).alias(),
            lit("the life").to(Payment::Life),
        ],
    )
    .skip(&lit("paid"))
    .then(&paid_when)
    .map(
        |(what, when)| Some(Amount::Paid { player: None, what, when }),
        |a| match a {
            Amount::Paid { player: None, what, when } => Some((*what, *when)),
            _ => None,
        },
    );
    let paid_by =
        lit("the amount of").then_keep(&payment).then(&past_subject()).skip(&lit("paid")).then(&paid_when).map(
            |((what, player), when)| Some(Amount::Paid { player: Some(Box::new(player)), what, when }),
            |a| match a {
                Amount::Paid { player: Some(p), what, when } => Some(((*what, (**p).clone()), *when)),
                _ => None,
            },
        );
    // "the amount of {E} you have"
    let energy = lit("the amount of").then_keep(&energy_symbol()).then_keep(&player_has()).map(
        |player| Some(Amount::PlayerCounters { player: Box::new(player), kind: PlayerCounter::Energy }),
        |a| match a {
            Amount::PlayerCounters { player, kind: PlayerCounter::Energy } => Some((**player).clone()),
            _ => None,
        },
    );
    // "the amount of unspent mana you have" (106.4)
    let unspent = lit("the amount of").then_keep(&unspent_mana());
    vec![life_player, life_none, life_by, that_excess, passive, active, prevented, spent, paid_none, paid_by, energy, unspent]
}

/// "unspent mana you have", "unspent green mana your opponents have" (CR 106.4).
fn unspent_mana() -> P<Amount> {
    crate::mana::unspent().then(&player_has()).map(
        |(color, player)| Some(Amount::Unspent { player: Box::new(player), color }),
        |a| match a {
            Amount::Unspent { player, color } => Some((*color, (**player).clone())),
            _ => None,
        },
    )
}

/// The multiplier spellings of the tallies: "for each 1 life you lost this turn", "for each 1
/// damage prevented this way", "for each {E} paid this way", "for each {S} spent to cast ~".
fn tallies_for_each() -> Vec<P<Amount>> {
    let gained = alt("gained or lost", vec![lit("gained").to(true), lit("lost").to(false)]);
    let who_gained =
        alt("player and verb", vec![past_subject().then(&gained), perfect_subject().then(&gained).alias()]);
    let life = windowed(&lit("1 life").then_keep(&who_gained), false).map(
        |((player, gained), when)| Some(Amount::LifeChange { player: Some(Box::new(player)), gained, when }),
        |a| match a {
            Amount::LifeChange { player: Some(p), gained, when } => Some((((**p).clone(), *gained), *when)),
            _ => None,
        },
    );
    let to = lit("to").then_keep(&selectors().selector).opt();
    let dealt = windowed(&lit("1 damage dealt").then_keep(&to), false).map(
        |(to, when)| {
            Some(Amount::Damage { by: None, to: to.map(Box::new), kind: DamageKind::Any, excess: false, when })
        },
        |a| match a {
            Amount::Damage { by: None, to, kind: DamageKind::Any, excess: false, when } => {
                Some((to.as_deref().cloned(), *when))
            }
            _ => None,
        },
    );
    let prevented = windowed(&lit("1 damage prevented"), false).map(
        |((), when)| Some(Amount::DamagePrevented { when }),
        |a| match a {
            Amount::DamagePrevented { when } => Some(((), *when)),
            _ => None,
        },
    );
    let paid = energy_symbol().skip(&lit("paid")).then_keep(&window()).map(
        |when| Some(Amount::Paid { player: None, what: Payment::Energy, when }),
        |a| match a {
            Amount::Paid { player: None, what: Payment::Energy, when } => Some(*when),
            _ => None,
        },
    );
    let spent = mana_symbol().skip(&lit("spent to cast")).then(&object()).map(
        |(m, spell)| Some(Amount::ManaSpent { spell: Box::new(spell), measure: ManaMeasure::Symbol(m) }),
        |a| match a {
            Amount::ManaSpent { spell, measure: ManaMeasure::Symbol(m) } => Some((*m, (**spell).clone())),
            _ => None,
        },
    );
    // "for each mana from a Treasure spent to cast it"
    let spent_from = lit("mana from")
        .then_keep(&alt("a", vec![lit("a"), lit("an").alias()]))
        .then_keep(&selectors().set_sg)
        .skip(&alt("spent to cast", vec![lit("spent to cast"), lit("that was spent to cast").alias()]))
        .then(&object())
        .map(
            |(from, spell)| Some(Amount::ManaSpentFrom { spell: Box::new(spell), from: Box::new(from) }),
            |a| match a {
                Amount::ManaSpentFrom { spell, from } => Some(((**from).clone(), (**spell).clone())),
                _ => None,
            },
        );
    vec![life, dealt, prevented, paid, spent, spent_from]
}

fn build() -> Amounts {
    let s = selectors();
    let amount = forward::<Amount>("amount");
    // "twice" and "half" bind tighter than "plus"/"minus": they take one term, not a chain
    let term = forward::<Amount>("term");
    let literal =
        alt("number", vec![stat_amount(), number_words().map(|n| Some(Amount::Const(n)), |_: &Amount| None).alias()]);
    // "the total number of" is the same count: a total of a count adds nothing
    let number_of = alt("the number of", vec![lit("the number of"), lit("the total number of").alias()]).then_keep(&counted(Number::Pl));
    // "the greatest number of creatures a player controls": the largest such count, over the
    // players or groups the count leaves open
    let greatest_number = lit("the greatest number of").then_keep(&counted(Number::Pl)).map(
        |a| Some(Amount::GreatestCount(Box::new(a))),
        |a| match a {
            Amount::GreatestCount(a) => Some((**a).clone()),
            _ => None,
        },
    );
    let least_number = lit("the least number of").then_keep(&counted(Number::Pl)).map(
        |a| Some(Amount::LeastCount(Box::new(a))),
        |a| match a { Amount::LeastCount(a) => Some((**a).clone()), _ => None },
    );
    // The count of one least-populated controller cohort, including ties; never the sum
    // controlled by all players tied for fewest. Canonical printing states the minimum.
    let least_controlled = lit("the number of").then_keep(&s.set_pl)
        .skip(&lit("controlled by the player who controls the fewest")).map(
            |mut set| {
                if set.controller.is_some() || set.domain != Domain::Permanent { return None; }
                set.controller = Some(ir::selector::Relation {
                    who: Box::new(Selector::new(Quant::Indefinite(Count::Exactly(Amount::Const(1))), ObjectSet::of(Domain::Player))),
                    negated: false, past: false,
                });
                Some(Amount::LeastCount(Box::new(Amount::Count(Box::new(set)))))
            }, |_| None,
        ).alias();
    // "its power", "the sacrificed creature's power"; "the power of target creature you control"
    // when the object has a postmodifier, and as an alternate spelling otherwise
    let possessive_stat = owner_of().then(&stat()).map(
        |(of, stat)| Some(Amount::Stat { of: Box::new(of), stat }),
        |a| match a {
            Amount::Stat { of, stat } if !has_post(of) => Some(((**of).clone(), *stat)),
            _ => None,
        },
    );
    let of_stat = |canonical: bool| {
        lit("the").then_keep(&stat()).skip(&lit("of")).then(&object()).map(
            |(stat, of)| Some(Amount::Stat { of: Box::new(of), stat }),
            move |a| match a {
                Amount::Stat { of, stat } if has_post(of) == canonical => Some((*stat, (**of).clone())),
                _ => None,
            },
        )
    };
    let stat_of = alt("stat", vec![possessive_stat, of_stat(true), of_stat(false).alias()]);
    let agg = alt(
        "aggregate",
        vec![
            lit("greatest").to(Aggregate::Greatest),
            lit("least").to(Aggregate::Least),
            lit("highest").to(Aggregate::Greatest).alias(),
        ],
    );
    let among = lit("the").then_keep(&agg).then(&stat()).skip(&lit("among")).then(&plural_group());
    // "the greatest mana value of a commander you own on the battlefield or in the command
    // zone": the same, said of one of them
    let of_one = lit("the")
        .then_keep(&agg)
        .then(&stat())
        .skip(&alt("of a", vec![lit("of a"), lit("of an")]))
        .then(&selectors().set_sg)
        .map(|x| Some(x), |_: &((Aggregate, ir::Stat), ObjectSet)| None)
        .alias();
    let among = alt("among", vec![among, of_one]);
    let total = lit("the total").then_keep(&stat()).skip(&lit("of")).then(&plural_group());
    // "their total power": of the objects "they" stands for
    let they = they_objects();
    let their_set = {
        let mut set = ObjectSet::of(Domain::Object);
        set.filter = Filter::All(vec![Filter::Atom(Atom::Among(Box::new(they)))]);
        set
    };
    let their_set2 = their_set.clone();
    let their_total = lit("their total").then_keep(&stat()).map(
        move |stat| Some(Amount::Aggregate { agg: Aggregate::Total, stat, among: Box::new(their_set.clone()) }),
        move |a| match a {
            Amount::Aggregate { agg: Aggregate::Total, stat, among } if **among == their_set2 => Some(*stat),
            _ => None,
        },
    );
    // a stat is an object's characteristic: players have none ("the greatest power among players"
    // is not read; a player's number is a life total, below)
    let objects = |set: &ObjectSet| set.possibilities().iter().all(|p| p.domain.sort != ir::Sort::Player);
    let aggregate = alt(
        "aggregate",
        vec![
            among.map(
                move |((agg, stat), set)| objects(&set).then(|| Amount::Aggregate { agg, stat, among: Box::new(set) }),
                move |a| match a {
                    Amount::Aggregate { agg, stat, among } if *agg != Aggregate::Total && objects(among) => {
                        Some(((*agg, *stat), (**among).clone()))
                    }
                    _ => None,
                },
            ),
            their_total,
            total.map(
                move |(stat, set)| objects(&set).then(|| Amount::Aggregate { agg: Aggregate::Total, stat, among: Box::new(set) }),
                move |a| match a {
                    Amount::Aggregate { agg: Aggregate::Total, stat, among } if objects(among) => Some((*stat, (**among).clone())),
                    _ => None,
                },
            ),
        ],
    );
    // "the highest life total among all players", "the lowest life total among your opponents"
    // (119.1: each player has a life total): Oracle says "highest" and "lowest" of life totals.
    // The players are all of a group, said as a selector ("all players", "your opponents") or, as
    // an alternate spelling, as a bare plural ("among players").
    let life_agg = alt("highest or lowest", vec![lit("highest").to(Aggregate::Greatest), lit("lowest").to(Aggregate::Least)]);
    let all_players = s.selector.map(
        |x| (x.quant == Quant::All && x.set.domain == Domain::Player && x.chooser.is_none() && x.random.is_none()).then_some(x.set),
        |set: &ObjectSet| (set.domain == Domain::Player).then(|| Selector::new(Quant::All, set.clone())),
    );
    let players = s.set_pl.map(|set| (set.domain == Domain::Player).then_some(set), |set| (set.domain == Domain::Player).then(|| set.clone()));
    let life_among = lit("the")
        .then_keep(&life_agg)
        .skip(&lit("life total among"))
        .then(&alt("players", vec![all_players, players.alias()]))
        .map(
            |(agg, among)| Some(Amount::LifeAggregate { agg, among: Box::new(among) }),
            |a| match a {
                Amount::LifeAggregate { agg, among } if *agg != Aggregate::Total => Some((*agg, (**among).clone())),
                _ => None,
            },
        );
    let life = s.possessive.skip(&alt("life total", vec![lit("life total"), lit("life").alias()])).map(
        |p| Some(Amount::Life(Box::new(p))),
        |a| match a {
            Amount::Life(p) => Some((**p).clone()),
            _ => None,
        },
    );
    let starting_life = boxed(&s.possessive.skip(&lit("starting life total")), Amount::StartingLife, |a| match a {
        Amount::StartingLife(p) => Some(p),
        _ => None,
    });
    let speed = boxed(&s.possessive.skip(&lit("speed")), Amount::Speed, |a| match a {
        Amount::Speed(p) => Some(p),
        _ => None,
    });
    // "black", "white and blue", or "that color": the color an instruction before chose
    let devoted_to = alt(
        "devotion colors",
        vec![
            colors().map(
                |v| Some(ir::amount::DevotionTo::Colors(v)),
                |to| match to {
                    ir::amount::DevotionTo::Colors(v) => Some(v.clone()),
                    ir::amount::DevotionTo::ThatColor => None,
                },
            ),
            lit("that color").to(ir::amount::DevotionTo::ThatColor),
        ],
    );
    let devotion = s.possessive.skip(&lit("devotion to")).then(&devoted_to).map(
        |(player, to)| Some(Amount::Devotion { player: Box::new(player), to }),
        |a| match a {
            Amount::Devotion { player, to } => Some(((**player).clone(), to.clone())),
            _ => None,
        },
    );
    let that = alt(
        "that much",
        vec![
            lit("that much").to(Amount::That { many: false }),
            lit("that many").to(Amount::That { many: true }),
            // "with mana value less than or equal to that damage": the damage the trigger named
            lit("that damage").to(Amount::That { many: false }).alias(),
        ],
    );
    let recalled = |text: &str, r: Recalled| lit(text).to(Amount::Recalled(r));
    let recall = alt(
        "recalled",
        vec![
            recalled("the result", Recalled::Result),
            recalled("that result", Recalled::Result).alias(),
            recalled("the other result", Recalled::OtherResult),
            recalled("that number", Recalled::Number),
            recalled("the chosen number", Recalled::ChosenNumber),
            recalled("the difference", Recalled::Difference),
        ],
    );
    let twice = lit("twice").then_keep(&term.rule()).map(
        |a| Some(Amount::Multiple(2, Box::new(a))),
        |a| match a {
            Amount::Multiple(2, x) => Some((**x).clone()),
            _ => None,
        },
    );
    // "three times X": a whole multiple from three up ("twice" is the two)
    let times = number_words().skip(&lit("times")).then(&term.rule()).map(
        |(n, a)| (n >= 3).then(|| Amount::Multiple(n, Box::new(a))),
        |a| match a {
            Amount::Multiple(n, x) if *n >= 3 => Some((*n, (**x).clone())),
            _ => None,
        },
    );
    let rounding = alt("rounding", vec![lit(", rounded up").to(true), lit(", rounded down").to(false)]);
    let rounded = lit("half").then_keep(&term.rule()).then(&rounding).map(
        |(of, round_up)| Some(Amount::Half { of: Box::new(of), round_up: Some(round_up) }),
        |a| match a {
            Amount::Half { of, round_up: Some(round_up) } => Some(((**of).clone(), *round_up)),
            _ => None,
        },
    );
    // the rounding elsewhere in the sentence ("draw half X cards, rounded down") or not stated
    let unrounded = lit("half").then_keep(&term.rule()).not_before(&lit(", rounded")).map(
        |of| Some(Amount::Half { of: Box::new(of), round_up: None }),
        |a| match a {
            Amount::Half { of, round_up: None } => Some((**of).clone()),
            _ => None,
        },
    );
    let party = lit("the number of creatures in").then_keep(&s.possessive).skip(&lit("party")).map(
        |p| Some(Amount::Party(Box::new(p))),
        |a| match a {
            Amount::Party(p) => Some((**p).clone()),
            _ => None,
        },
    );
    let mut base = vec![
        lit("the total mana value of the permanents ~ targets").to(Amount::TargetedPermanentManaValue),
        literal,
        party,
        lit("the number of cards looked at while scrying this way").to(Amount::LookedAtScrying),
        lit("the highest number you noted for cards named ~").to(Amount::HighestNoted),
        lit("the number of unlocked doors among Rooms you control").to(Amount::UnlockedDoors),
        number_of,
        greatest_number,
        least_number,
        least_controlled,
        stat_of,
        aggregate,
        life_among,
        life,
        starting_life,
        speed,
        devotion,
        that,
        recall,
        twice,
        times,
        rounded,
        unrounded,
    ];
    base.extend(tallies());
    let base = alt("amount", base);
    term.define(base.clone());
    // "a plus b minus c", folded to the left; a chain rather than left recursion
    let op = alt("plus or minus", vec![lit("plus").to(true), lit("minus").to(false)]);
    let rule = base.then(&op.then(&base).many()).map(
        |(first, rest)| {
            Some(rest.into_iter().fold(first, |acc, (plus, b)| {
                if plus { Amount::Plus(Box::new(acc), Box::new(b)) } else { Amount::Minus(Box::new(acc), Box::new(b)) }
            }))
        },
        |a| {
            let mut rest = Vec::new();
            let mut cur = a.clone();
            loop {
                match cur {
                    Amount::Plus(x, y) => {
                        rest.push((true, *y));
                        cur = *x;
                    }
                    Amount::Minus(x, y) => {
                        rest.push((false, *y));
                        cur = *x;
                    }
                    other => {
                        rest.reverse();
                        return Some((other, rest));
                    }
                }
            }
        },
    );
    // "… as you cast ~": the whole amount, fixed at that moment
    let self_ref = leaf("~", |k| matches!(k, Kind::SelfRef).then_some(()), |()| Some(Kind::SelfRef));
    let moment = alt(
        "moment",
        vec![
            lit("as you cast").then_keep(&self_ref).to(Moment::Casting),
            lit("as you activate this ability").to(Moment::Activating),
            lit("as this ability resolves").to(Moment::Resolving),
        ],
    );
    let as_of = rule.then(&moment).map(
        |(amount, moment)| Some(Amount::AsOf { amount: Box::new(amount), moment }),
        |a| match a {
            Amount::AsOf { amount, moment } => Some(((**amount).clone(), *moment)),
            _ => None,
        },
    );
    let rule = alt("amount", vec![rule, as_of]);
    amount.define(rule.clone());
    Amounts { amount: rule, for_each: for_each(), counted: counted(Number::Pl) }
}

fn for_each() -> P<Amount> {
    let s = selectors();
    let each = counted(Number::Sg);
    // "for each target beyond the first": all but one
    let beyond = each.skip(&lit("beyond the first")).map(
        |a| Some(Amount::Minus(Box::new(a), Box::new(Amount::Const(1)))),
        |a| match a {
            Amount::Minus(x, one) if **one == Amount::Const(1) => Some((**x).clone()),
            _ => None,
        },
    );
    // "for each of those creatures": the members of a group the text named
    let of = lit("of").then_keep(&among_selector()).map(
        |set| Some(Amount::Count(Box::new(set))),
        |a| match a {
            Amount::Count(set) => Some((**set).clone()),
            _ => None,
        },
    );
    let party = lit("creature in").then_keep(&s.possessive).skip(&lit("party")).map(
        |p| Some(Amount::Party(Box::new(p))),
        |a| match a {
            Amount::Party(p) => Some((**p).clone()),
            _ => None,
        },
    );
    // "for each of its colors", "for each of that spell's colors"
    let colors = lit("of").then_keep(&owner_of()).skip(&lit("colors")).map(
        |s| Some(Amount::Colors(Box::new(s))),
        |a| match a {
            Amount::Colors(s) => Some((**s).clone()),
            _ => None,
        },
    );
    let votes = crate::vote::option().skip(&lit("vote")).map(
        |o| Some(Amount::Votes(o)),
        |a| match a {
            Amount::Votes(o) => Some(o.clone()),
            _ => None,
        },
    );
    // "for each land you control and each land card in your graveyard": the two counts together
    let counts = |a: &Amount| matches!(a, Amount::Count(_));
    let and_each = each.skip(&lit("and each")).then(&each).map(
        move |(a, b)| (counts(&a) && counts(&b)).then(|| Amount::Plus(Box::new(a), Box::new(b))),
        move |a| match a {
            Amount::Plus(x, y) if counts(x) && counts(y) => Some(((**x).clone(), (**y).clone())),
            _ => None,
        },
    );
    // "for each kind of counter on target permanent or player", "… among creatures you control"
    let kinds_on = lit("kind of counter on").then_keep(&s.selector).map(
        |on| Some(Amount::CounterKindsOn(Box::new(on))),
        |a| match a {
            Amount::CounterKindsOn(on) => Some((**on).clone()),
            _ => None,
        },
    );
    let kinds_among = lit("kind of counter among").then_keep(&s.set_pl).map(
        |among| Some(Amount::CounterKindsAmong(Box::new(among))),
        |a| match a {
            Amount::CounterKindsAmong(among) => Some((**among).clone()),
            _ => None,
        },
    );
    // "for each graveyard with seven or more cards in it"
    let graveyards_with = lit("graveyard with").then_keep(&s.selector).skip(&lit("in it")).map(
        |what| Some(Amount::GraveyardsWith(Box::new(what))),
        |a| match a {
            Amount::GraveyardsWith(what) => Some((**what).clone()),
            _ => None,
        },
    );
    let flips_won = alt("flips won", vec![lit("flip you won").to(Amount::FlipsWon), lit("flip you win").to(Amount::FlipsWon).alias()]);
    let flips_lost = alt("flips lost", vec![lit("flip you lose").to(Amount::FlipsLost), lit("flip you lost").to(Amount::FlipsLost).alias()]);
    let mut v = vec![each, beyond, and_each, of, colors, party, votes, unspent_mana(), kinds_on, kinds_among, flips_won, flips_lost, graveyards_with];
    v.extend(tallies_for_each());
    // "investigate once for each nontoken attacking creature"
    let plain = alt("for each", vec![lit("for each"), lit("once for each").alias()]).then_keep(&alt("for each", v));
    let source = leaf("~", |k| matches!(k, Kind::SelfRef).then_some(()), |()| Some(Kind::SelfRef));
    let casting = plain.skip(&lit("as you cast")).skip(&source).map(
        |amount| Some(Amount::AsOf { amount: Box::new(amount), moment: Moment::Casting }),
        |a| match a { Amount::AsOf { amount, moment: Moment::Casting } => Some((**amount).clone()), _ => None },
    );
    alt("for each", vec![casting, plain])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::Outcome;
    use oracle::token::tokenize;

    fn rt(rule: &P<Amount>, text: &str) -> Amount {
        let toks = tokenize(text);
        match rule.parse_all(&toks) {
            Outcome::Accepted { value, .. } => {
                assert_eq!(rule.print_text(&value, false).as_deref(), Some(text), "{value:#?}");
                value
            }
            other => panic!("{text}: {other:#?}"),
        }
    }

    #[test]
    fn for_each_can_fix_a_count_at_casting() {
        for text in ["for each spell cast this turn as you cast ~", "for each artifact you control as you cast ~"] {
            assert!(matches!(rt(&amounts().for_each, text), Amount::AsOf { moment: Moment::Casting, .. }));
        }
    }

    #[test]
    fn amounts_round_trip() {
        let a = &amounts().amount;
        for t in [
            "the number of times its replicate cost was paid",
            "the number of creatures you control",
            "the number of cards in your hand",
            "its power",
            "~'s power",
            "that creature's toughness",
            "the sacrificed creature's power",
            "the greatest power among creatures you control",
            "the total power of creatures you control",
            "the number of +1/+1 counters on ~",
            "your life total",
            "your devotion to black",
            "your devotion to white and blue",
            "that much",
            "twice that many",
            "three times X",
            "three times ~'s power",
            "half your life total, rounded up",
            "the number of basic land types among lands you control",
            "X",
            "1",
            "the number of creatures in your party",
            "2 plus the sacrificed creature's mana value",
            "the number of cards in your hand minus 4",
        ] {
            rt(a, t);
        }
        let f = &amounts().for_each;
        for t in [
            "for each creature you control",
            "for each card in your graveyard",
            "for each +1/+1 counter on ~",
            "for each charge counter on it",
        ] {
            rt(f, t);
        }
    }

    /// CR 107.1: "twice" is two times; a multiple of one or none is not a multiple.
    #[test]
    fn r107_1_multiples() {
        let a = &amounts().amount;
        assert_eq!(rt(a, "twice X"), Amount::Multiple(2, Box::new(Amount::X)));
        assert_eq!(rt(a, "five times X"), Amount::Multiple(5, Box::new(Amount::X)));
        for t in ["two times X", "one times X", "times X"] {
            assert!(!matches!(a.parse_all(&tokenize(t)), Outcome::Accepted { .. }), "{t}");
        }
    }

    #[test]
    fn tallies_round_trip() {
        let a = &amounts().amount;
        for t in [
            "the amount of life you gained this turn",
            "the amount of life you gained",
            "the life lost this way",
            "the damage dealt this way",
            "the damage dealt",
            "the amount of damage it dealt to that player",
            "the excess damage dealt this way",
            "the damage prevented this way",
            "the amount of mana spent to cast ~",
            "the number of colors of mana spent to cast ~",
            "the amount of {S} spent to cast ~",
            "the total amount of mana paid this way",
            "the amount of mana that player paid this way",
            "the amount of {E} paid this way",
            "the life paid as it entered",
            "the number of times ~ was kicked",
            "the number of times ~ has mutated",
            "the number of times it's been cast from the command zone this game",
            "the number of counters removed this way",
            "the number of +1/+1 counters removed this way",
            "the number of experience counters you have",
            "the amount of {E} you have",
            "your starting life total",
            "your speed",
            "the result",
            "that number",
            "the difference",
            "the number of differently named lands you control",
            "the number of different powers among creatures you control",
            "the power of target creature you control",
            "their power",
            "their total power",
            "the total power of those creatures",
            "the greatest power among them",
            "the number of counters among permanents you control",
            "half X",
            "twice the number of times it was kicked",
            "the number of colors it is",
            "the number of red mana symbols in the mana costs of permanents you control",
            "the number of different kinds of counters among permanents you control",
            "the number of creatures that died this turn",
            "the number of cards you've drawn this turn",
            "the number of opponents you attacked this turn",
            "that excess damage",
        ] {
            rt(a, t);
        }
        let f = &amounts().for_each;
        for t in [
            "for each time it was kicked",
            "for each time you've cast your commander from the command zone this game",
            "for each color of mana spent to cast it",
            "for each {S} spent to cast ~",
            "for each charge counter removed this way",
            "for each poison counter your opponents have",
            "for each 1 life you lost",
            "for each 1 damage prevented this way",
            "for each {E} paid this way",
            "for each of those creatures",
            "for each creature in your party",
            "for each target beyond the first",
            "for each different power among creatures you control",
            "for each of its colors",
            "for each white mana symbol in its mana cost",
            "for each opponent who lost life this turn",
            "for each counter removed",
        ] {
            rt(f, t);
        }
    }

    #[test]
    fn stat_spellings() {
        let a = &amounts().amount;
        // a selector without postmodifiers takes the possessive; "the … of" reads the same
        let of = a.parse_all(&tokenize("the mana value of the exiled card"));
        let Outcome::Accepted { value, .. } = of else { panic!("{of:?}") };
        assert_eq!(a.print_text(&value, false).as_deref(), Some("the exiled card's mana value"));
    }

    /// CR 119.1: each player has a life total, so a group of players has a highest and a lowest
    /// one; objects have none.
    #[test]
    fn r119_1_highest_and_lowest_life_totals() {
        let a = &amounts().amount;
        for t in [
            "the highest life total among all players",
            "the lowest life total among all players",
            "the highest life total among your opponents",
            "20 minus the highest life total among all players",
            "half the highest life total among your opponents, rounded up",
        ] {
            rt(a, t);
        }
        let players = a.parse_all(&tokenize("the highest life total among players"));
        let Outcome::Accepted { value, .. } = players else { panic!("{players:?}") };
        assert_eq!(a.print_text(&value, false).as_deref(), Some("the highest life total among all players"));
        for t in [
            "the lowest life total among creatures",
            "the highest life total among creatures you control",
            "the highest power among players",
            "the greatest power among creatures or players",
            "the greatest mana value among spells or players",
            "the greatest life total among all players",
            "the total life total among all players",
            "the highest life total among each player",
        ] {
            assert!(!matches!(a.parse_all(&tokenize(t)), Outcome::Accepted { .. }), "{t} must not read");
        }
    }
}
