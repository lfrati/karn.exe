//! Actions: the verb phrases of costs and effects.
//!
//! ```text
//! action(form) := "sacrifice" selector | "discard" selector | "discard your hand"
//!               | "exile" selector | "return" selector "to its/their owner's hand"
//!               | "put" selector "on top of/on the bottom of" library | "tap" selector | "untap" selector
//!               | "reveal" selector | "exert" selector | "pay" N "life" | "pay" mana | "pay" energy
//!               | "mill" N "card(s)" | "put" N kind "counter(s) on" selector
//!               | "remove" count [kind] "counter(s) from" selector | "collect evidence" N
//!               | "waterbend" mana | "blight" N | "forage"
//! ```
//!
//! The form is the verb's: [`Form::Base`] for imperatives ("Sacrifice a creature") and plural or
//! "you" subjects, [`Form::Third`] after a singular subject ("target player sacrifices a creature").

use crate::event::{Form, verb};
use crate::kernel::{P, alt, forward, leaf, lit};
use crate::mana::mana_cost;
use crate::selector::{counter_kind, selectors};
use crate::words::{Number, count_amount, digits, stat_quantity};
use ir::Count;
use ir::lexicon::{Role, Verb};
use ir::{
    Action, Amount, Conj, Coord, CounterCount, Counters, CountersOf, Hit, CounterKind, Delta, Granted, KeywordAction, NumberedAction,
    ObjectAction, Objects, Placement, PlayerCounter, Quant, UntilVerb, UntapWhose, Selector, Subtype, Zone,
};
use oracle::token::{Kind, Token, detokenize, tokenize};
use std::sync::OnceLock;

/// A verb with its objects: "sacrifice a creature", "tap enchanted creature".
fn with_object(
    head: P<()>,
    objects: &P<Objects>,
    mk: fn(Objects) -> Action,
    un: fn(&Action) -> Option<&Objects>,
) -> P<Action> {
    head.then_keep(objects).map(move |o| Some(mk(o)), move |a| un(a).cloned())
}

/// The one coordination: an item, or several joined by "and", "or" or "and/or" ("x and y",
/// "x, y, or z"). Whatever is joined reads and prints through this.
pub(crate) fn coord<T: crate::kernel::Val>(one: &P<T>) -> P<Coord<T>> {
    let single = one.map(
        |x| Some(Coord::one(x)),
        |c: &Coord<T>| match c.items.as_slice() {
            [x] if c.conj == Conj::And => Some(x.clone()),
            _ => None,
        },
    );
    let list = |word: &str, conj: Conj| {
        one.list(word).map(
            move |items| Some(Coord { items, conj }),
            move |c: &Coord<T>| (c.conj == conj && c.items.len() >= 2).then(|| c.items.clone()),
        )
    };
    alt("list", vec![single, list("and", Conj::And), list("or", Conj::Or), list("and/or", Conj::AndOr)])
}

/// One selector, several joined by "and", or alternatives joined by "or" (each with its own
/// determiner: "another creature or an enchantment").
pub(crate) fn objects(one: &P<Selector>) -> P<Objects> {
    coord(one)
}

// What a slot takes comes from the lexicon ([`ir::lexicon`]): each action states there what it
// can be done to and who performs it, by the rule that defines it. "Destroy" takes a permanent
// (701.8), "counter" a spell or ability (701.6), "discard" a card (701.9).

pub(crate) use ir::lexicon::{fills, of_kind};

/// The slot of one role of an action, taking several objects: only what the lexicon says the
/// action takes there.
fn slot(objects: &P<Objects>, verb: Verb, role: Role) -> P<Objects> {
    objects.map(
        move |o| o.items.iter().all(|s| fills(s, verb, role)).then_some(o),
        move |o| o.items.iter().all(|s| fills(s, verb, role)).then(|| o.clone()),
    )
}

/// The slot of one role of an action, taking one noun phrase.
fn slot_one(one: &P<Selector>, verb: Verb, role: Role) -> P<Selector> {
    one.map(move |s| fills(&s, verb, role).then_some(s), move |s| fills(s, verb, role).then(|| s.clone()))
}

/// Can a subject of this kind (None: no subject, an imperative) perform the action?
pub(crate) fn does(subject: Option<&Selector>, a: &Action) -> bool {
    let fits = ir::lexicon::can_do(a.doer(), subject);
    match (a, subject) {
        // Bare "Choose one" is a mode header. Object ellipsis states its player (608.2d).
        (Action::Choose(ir::Choice::Objects(objects)), None)
            if matches!(objects.items.as_slice(), [one] if matches!(one.quant,
                Quant::Definite(ir::Ref::Anaphor(ir::Anaphor::One)))) => false,
        // what a prohibition forbids is an event of the subject: "~ can't be countered", never
        // "target player can't be countered"
        // Ability activation prohibitions belong to the activation-family rule grammar,
        // rather than a passive action on an ability object.
        (Action::Cant(v), Some(s)) => fits && v.iter().all(|f| {
            !matches!((&s.set.domain, &f.event), (ir::Domain::Ability, ir::EventAction::Deed(d)) if d.verb == Verb::Activate && d.passive)
                && f.event.takes_subject(s) && f.event.roles_fit()
        }),
        // and what a requirement demands: "target creature attacks this turn if able"
        (Action::Static(act), Some(s)) => match act.as_ref() {
            ir::StaticAct::IfAble { what, .. } => fits && what.iter().all(|e| e.takes_subject(s) && e.roles_fit()),
            _ => fits,
        },
        // "~'s base power becomes 3" has no subject, "you may have ~'s base power become 3" has
        // the player who has it so
        (Action::SetStat { of: Some(_), caused, .. }, _) => fits && *caused == subject.is_some(),
        // the game becomes day: nobody does it, and nothing is named
        (Action::Keyword(ir::KeywordAction::BecomeDay | ir::KeywordAction::BecomeNight | ir::KeywordAction::IncreaseSpeed), s) => s.is_none(),
        _ => fits,
    }
}

/// Actions as one group: "sacrifice a creature", "remove five quest counters from ~ and
/// sacrifice it", "pay {3} or discard a card".
pub(crate) fn actions(form: Form) -> P<Coord<Action>> {
    alt("actions", vec![payments(form), coord(&action(form))])
}

/// What "pay" takes, as the action of paying it: life (119.4), mana (106.4, 118.1), energy
/// (107.14), a cost an object has (202.1, 702).
fn payment() -> P<Action> {
    let life = noun_amount("life").map(
        |n| Some(Action::PayLife(n)),
        |a| match a {
            Action::PayLife(n) => Some(n.clone()),
            _ => None,
        },
    );
    let mana = mana_cost().map(
        |c| Some(Action::PayMana(c)),
        |a| match a {
            Action::PayMana(c) => Some(c.clone()),
            _ => None,
        },
    );
    let energy = energy().map(
        |n| Some(Action::PayEnergy(n)),
        |a| match a {
            Action::PayEnergy(n) => Some(n.clone()),
            _ => None,
        },
    );
    // "its mana cost", "~'s suspend cost": a cost an object has (118.1, 202.1)
    let named = crate::cost::cost_expr().map(
        |e| Some(Action::PayCost(e)),
        |a| match a {
            Action::PayCost(e) => Some(e.clone()),
            _ => None,
        },
    );
    alt("payment", vec![life, mana, energy, named])
}

/// One "pay" governing several payments: "pay {B} and 1 life" (pay {B} and pay 1 life), "pay
/// {2} or {E}{E}{E}" (pay one or the other). The same actions as "pay" each written out, and
/// the way they print when all are payments.
pub(crate) fn payments(form: Form) -> P<Coord<Action>> {
    verb(form, "pays", "pay", "").then_keep(&coord(&payment())).map(
        |c| (c.items.len() >= 2).then_some(c),
        |c| (c.items.len() >= 2).then(|| c.clone()),
    )
}

fn plural_objects(o: &Objects) -> bool {
    o.items.len() > 1 || o.items.first().is_some_and(crate::selector::is_plural)
}

/// "a card", "two cards": a count of cards (mill).
fn cards() -> P<Amount> {
    let one = lit("a card").to(Amount::Const(1));
    let many = count_amount().skip(&lit("cards")).map(|n| (n != Amount::Const(1)).then_some(n), |n| Some(n.clone()));
    // "cards equal to its power"
    let literal = crate::words::is_quantity;
    let equal = lit("cards equal to")
        .then_keep(&crate::kernel::lazy("amount", || crate::amount::amounts().amount.clone()))
        .map(move |a| (!literal(&a)).then_some(a), move |a| (!literal(a)).then(|| a.clone()));
    // "that many cards plus one": "that many" takes the arithmetic after the noun
    let that = Amount::That { many: true };
    let that2 = that.clone();
    let after = lit("that many cards")
        .then_keep(&alt("plus or minus", vec![lit("plus").to(true), lit("minus").to(false)]))
        .then(&crate::words::count_atom())
        .map(
            move |(plus, n)| {
                let (x, n) = (Box::new(that.clone()), Box::new(n));
                Some(if plus { Amount::Plus(x, n) } else { Amount::Minus(x, n) })
            },
            move |a| match a {
                Amount::Plus(x, n) if **x == that2 => Some((true, (**n).clone())),
                Amount::Minus(x, n) if **x == that2 => Some((false, (**n).clone())),
                _ => None,
            },
        );
    // "X cards plus an additional card for each …"
    let unit = alt(
        "additional cards",
        vec![
            lit("card").to(1u32),
            count_amount().skip(&lit("cards")).map(
                |n| match n {
                    Amount::Const(m) if m != 1 => Some(m),
                    _ => None,
                },
                |m| (*m != 1).then_some(Amount::Const(*m)),
            ),
        ],
    );
    let additional = plus_additional(alt("cards", vec![one.clone(), many.clone()]), unit);
    // "half X cards, rounded down": the rounding is said after the noun
    let rounded = count_amount()
        .skip(&lit("cards"))
        .then(&alt("rounding", vec![lit(", rounded up").to(true), lit(", rounded down").to(false)]))
        .map(
            |(n, up)| match n {
                Amount::Half { of, round_up: None } => Some(Amount::Half { of, round_up: Some(up) }),
                _ => None,
            },
            |a| match a {
                Amount::Half { of, round_up: Some(up) } => Some((Amount::Half { of: of.clone(), round_up: None }, *up)),
                _ => None,
            },
        );
    alt("cards", vec![one, rounded, many, additional, equal, after])
}

/// {E} symbols: "{E}{E}" up to five, "six {E}" and "X {E}" beyond (as Oracle prints them).
fn energy() -> P<Amount> {
    let e =
        leaf("{E}", |k| matches!(k, Kind::Symbol(s) if s == "E").then_some(()), |()| Some(Kind::Symbol("E".into())));
    let symbols = e.then(&e.clone().tight().many()).map(
        |((), rest)| Some(Amount::Const(rest.len() as u32 + 1)),
        |a| match a {
            Amount::Const(n @ 1..=5) => Some(((), vec![(); *n as usize - 1])),
            _ => None,
        },
    );
    let counted = count_amount().skip(&e).map(Some, |n| match n {
        Amount::Const(1..=5) => None,
        n => Some(n.clone()),
    });
    // "an amount of {E} equal to its mana value": the amount is a quantity, not a literal
    let literal = crate::words::is_quantity;
    let equal = lit("an amount of")
        .then_keep(&e)
        .skip(&lit("equal to"))
        .then_keep(&crate::kernel::lazy("amount", || crate::amount::amounts().amount.clone()))
        .map(move |a| (!literal(&a)).then_some(a), move |a| (!literal(a)).then(|| a.clone()));
    // "that many plus one {E}"
    let that_plus_one = || Amount::Plus(Box::new(Amount::That { many: true }), Box::new(Amount::Const(1)));
    let plus_one = lit("that many plus one").then_keep(&e).map(
        move |()| Some(that_plus_one()),
        move |a| (*a == that_plus_one()).then_some(()),
    );
    alt("energy", vec![symbols, plus_one, counted, equal])
}

/// The counters of "put … counter(s) on" and "remove … counter(s) from".
fn counters_of(
    count: P<CounterCount>,
    kind: P<Option<CounterKind>>,
    prep: &'static str,
) -> P<(CounterCount, Option<CounterKind>)> {
    let a = alt("a", vec![lit("a"), lit("an").alias()]);
    let one = a.then_keep(&kind).skip(&lit(format!("counter {prep}").trim())).map(
        |k| Some((CounterCount::Exactly(Amount::Const(1)), k)),
        |(c, k)| (*c == CounterCount::Exactly(Amount::Const(1))).then(|| k.clone()),
    );
    let many = count.then(&kind).skip(&lit(format!("counters {prep}").trim())).map(
        |(c, k)| (c != CounterCount::Exactly(Amount::Const(1))).then_some((c, k)),
        |(c, k)| Some((c.clone(), k.clone())),
    );
    alt("counters", vec![one, many])
}

/// "+2", "-1", "+X", "+0".
pub(crate) fn delta() -> P<Delta> {
    let sign = alt("sign", vec![lit("+").to(false), lit("-").to(true), lit("−").to(true).alias()]);
    let n = alt(
        "N",
        vec![
            digits().map(
                |n| Some(Amount::Const(n)),
                |a| match a {
                    Amount::Const(n) => Some(*n),
                    _ => None,
                },
            ),
            lit("X").to(Amount::X),
        ],
    );
    sign.then(&n.tight()).map(|(minus, amount)| Some(Delta { minus, amount }), |d| Some((d.minus, d.amount.clone())))
}

thread_local! {
    /// How many quoted abilities the printer is inside.
    static QUOTE_DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// A quoted ability: read by the ability grammar (bracketed loyalty costs, 606.4) into
/// [`Granted::Ability`]; one it declines stays text ([`Granted::Quoted`]); an ambiguous one is
/// not read at all.
pub(crate) fn quoted() -> P<Granted> {
    use crate::kernel::{Out, Outcome};
    leaf(
        "quoted ability",
        |k| {
            let Kind::Quote { body, closed: true, .. } = k else { return None };
            match crate::ability::abilities().granted.parse_all(body) {
                Outcome::Accepted { value, .. } => Some(Granted::Ability(Box::new(value))),
                Outcome::Declined { .. } => Some(Granted::Quoted(detokenize(&[Token { kind: k.clone(), space_before: 0 }]))),
                Outcome::Ambiguous(_) => None,
            }
        },
        |g| match g {
            Granted::Ability(a) => {
                // a quote inside a quote is spelled with single quotation marks
                let depth = QUOTE_DEPTH.with(|d| d.replace(d.get() + 1));
                let mut out = Out::default();
                let ok = crate::ability::abilities().granted.print(a, &mut out);
                QUOTE_DEPTH.with(|d| d.set(depth));
                // a quoted enchant ability is spelled as a phrase, not a sentence: it loses
                // "enchant creature card in a graveyard" (702.5a; Animate Dead)
                let phrase = matches!(&**a, ir::Ability::Keywords(ks)
                    if matches!(ks.first(), Some(k) if matches!(k.param, ir::KeywordParam::Enchant(_))));
                ok.then(|| Kind::Quote { mark: if depth == 0 { '"' } else { '\'' }, body: out.tokens(!phrase), closed: true })
            }
            // the text is the quote with its marks; a single-quoted one does not tokenize alone
            Granted::Quoted(s) => {
                let mark = s.chars().next()?;
                let inner = s.strip_prefix(mark)?.strip_suffix(mark)?;
                Some(Kind::Quote { mark, body: tokenize(inner), closed: true })
            }
            Granted::Keyword(_) | Granted::AbilitiesOf { .. } | Granted::Every(_) | Granted::ThatAbility => None,
        },
    )
}

/// Abilities offered as a choice: "flying, first strike, or trample".
fn granted_or() -> P<Vec<Granted>> {
    let keyword = crate::keyword::keyword().map(
        |k| Some(Granted::Keyword(Box::new(k))),
        |g| match g {
            Granted::Keyword(k) => Some((**k).clone()),
            _ => None,
        },
    );
    keyword.list("or")
}

/// What an effect grants: keywords and quoted abilities, joined by "and" ("flying and haste",
/// "haste and \"At the beginning of the end step, sacrifice ~.\"").
pub(crate) fn granted() -> P<Vec<Granted>> {
    let one = granted_one();
    let single = one.map(|g| Some(vec![g]), |v: &Vec<Granted>| (v.len() == 1).then(|| v[0].clone()));
    alt("granted", vec![single, one.list("and")])
}

/// One thing granted: a keyword, a quoted ability, a whole class of types.
pub(crate) fn granted_one() -> P<Granted> {
    let keyword = crate::keyword::keyword().map(
        |k| Some(Granted::Keyword(Box::new(k))),
        |g| match g {
            Granted::Keyword(k) => Some((**k).clone()),
            _ => None,
        },
    );
    // "all creature types" (205.3m) stands in the list with abilities: "vigilance and all creature types"
    let types = crate::becomes::every(true, &crate::becomes::LAND_AND_CREATURE).map(
        |e| Some(Granted::Every(e)),
        |g| match g {
            Granted::Every(e) => Some(*e),
            _ => None,
        },
    );
    // "Choose flying or indestructible. ~ gains that ability until end of turn."
    alt("granted", vec![keyword, quoted(), types, lit("that ability").to(Granted::ThatAbility)])
}

/// An amount of a mass noun: "3 damage", "X life", "damage equal to its power", "that much
/// life". A literal (a number or X) prints before the noun, anything else after "equal to".
/// "half their life, rounded up": half a life total, as the life a player loses or pays.
fn half_life() -> P<Amount> {
    let rounding = alt(
        "rounding",
        vec![lit(", rounded up").to(Some(true)), lit(", rounded down").to(Some(false)), crate::kernel::empty().to(None)],
    );
    lit("half").then_keep(&selectors().possessive).skip(&lit("life")).then(&rounding).map(
        |(p, round_up)| Some(Amount::Half { of: Box::new(Amount::Life(Box::new(p))), round_up }),
        |a| match a {
            Amount::Half { of, round_up } => match of.as_ref() {
                Amount::Life(p) => Some(((**p).clone(), *round_up)),
                _ => None,
            },
            _ => None,
        },
    )
}

/// "X cards plus an additional card for each creature you control", "3 life plus an additional
/// 3 life for each …": a number of things and as many more again for each thing counted (107.1),
/// `Plus(N, Multiple(M, count))`. `unit` is the M things after "an additional", of the same noun.
pub(crate) fn plus_additional(first: P<Amount>, unit: P<u32>) -> P<Amount> {
    let each = crate::kernel::lazy("for each", || crate::amount::amounts().for_each.clone());
    first.skip(&lit("plus an additional")).then(&unit).then(&each).map(
        |((n, m), c)| Some(Amount::Plus(Box::new(n), Box::new(Amount::Multiple(m, Box::new(c))))),
        |a| match a {
            Amount::Plus(n, more) => match more.as_ref() {
                Amount::Multiple(m, c) => Some((((**n).clone(), *m), (**c).clone())),
                _ => None,
            },
            _ => None,
        },
    )
}

pub(crate) fn noun_amount(noun: &str) -> P<Amount> {
    let literal = crate::words::is_quantity;
    let that = |a: &Amount| matches!(a, Amount::That { many: false });
    // "half your life, rounded up" is [`half_life`]'s: the one spelling of half a life total
    let is_life = noun == "life";
    let half_of_life = move |a: &Amount| is_life && matches!(a, Amount::Half { of, .. } if matches!(**of, Amount::Life(_)));
    let before = stat_quantity().skip(&lit(noun));
    let equal = lit(noun)
        .skip(&lit("equal to"))
        .then_keep(&crate::amount::amounts().amount)
        .map(
            move |a| (!literal(&a) && !that(&a) && !half_of_life(&a)).then_some(a),
            move |a| (!literal(a) && !that(a) && !half_of_life(a)).then(|| a.clone()),
        );
    let that_much = lit("that much").skip(&lit(noun)).to(Amount::That { many: false });
    // replacement substitutes: "double that damage", "twice that much life", "that much damage
    // plus 1" (614.1a); damage doubles as "double", life as "twice"
    let that_amount = Amount::That { many: false };
    let twice = Amount::Multiple(2, Box::new(that_amount.clone()));
    let doubled = if noun == "damage" {
        alt(
            "double",
            vec![
                lit("double that damage").to(twice.clone()),
                lit("twice that much damage").to(twice).alias(),
                lit("triple that damage").to(Amount::Multiple(3, Box::new(that_amount.clone()))),
            ],
        )
    } else {
        lit("twice that much").skip(&lit(noun)).to(twice)
    };
    let t2 = that_amount.clone();
    let op = alt("plus or minus", vec![lit("plus").to(true), lit("minus").to(false)]);
    let plus = lit("that much").skip(&lit(noun)).then_keep(&op).then(&crate::words::stat_amount()).map(
        move |(plus, n)| {
            let (x, n) = (Box::new(that_amount.clone()), Box::new(n));
            Some(if plus { Amount::Plus(x, n) } else { Amount::Minus(x, n) })
        },
        move |a| match a {
            Amount::Plus(x, n) if **x == t2 && matches!(**n, Amount::Const(_) | Amount::X) => Some((true, (**n).clone())),
            Amount::Minus(x, n) if **x == t2 && matches!(**n, Amount::Const(_) | Amount::X) => Some((false, (**n).clone())),
            _ => None,
        },
    );
    // "3 life plus an additional 3 life for each …"
    let unit = stat_quantity().skip(&lit(noun)).map(
        |a| match a {
            Amount::Const(m) => Some(m),
            _ => None,
        },
        |m| Some(Amount::Const(*m)),
    );
    let additional = plus_additional(before.clone(), unit);
    // before "equal to", which can say the same ("life equal to 3 plus three times the number
    // of …") and prints otherwise
    let mut forms = vec![doubled, plus, before, additional, equal, that_much];
    if noun == "life" {
        forms.push(half_life());
    }
    alt(noun, forms)
}

/// How many things, given by an amount after the noun (CR 107.1, 608.2h): "a number of cards from
/// the top of your library equal to its power", "cards equal to the sacrificed creature's power".
/// One way of filling the quantity of a counted noun phrase from any amount; what the action may
/// be done to is the slot's business ([`slot`]).
fn measured_objects() -> P<Selector> {
    let s = selectors();
    let lead = alt("a number of", vec![lit("a number of"), crate::kernel::empty().alias()]);
    let sets = alt("plural set", vec![s.set_pl_from.clone(), s.set_pl.clone()]);
    let amount = crate::kernel::lazy("amount", || crate::amount::amounts().amount.clone());
    lead.then_keep(&sets).skip(&lit("equal to")).then(&amount).map(
        |(set, amount)| {
            (!crate::words::is_quantity(&amount)).then(|| Selector::new(Quant::Indefinite(Count::Exactly(amount)), set))
        },
        |sel: &Selector| match &sel.quant {
            Quant::Indefinite(Count::Exactly(amount))
                if !crate::words::is_quantity(amount) && sel.chooser.is_none() && sel.random.is_none() =>
            {
                Some((sel.set.clone(), amount.clone()))
            }
            _ => None,
        },
    )
}

/// The action grammar in one verb form ([`Form::Base`] or [`Form::Third`]); built once per form.
pub fn action(form: Form) -> P<Action> {
    static BASE: OnceLock<P<Action>> = OnceLock::new();
    static THIRD: OnceLock<P<Action>> = OnceLock::new();
    static GERUND: OnceLock<P<Action>> = OnceLock::new();
    match form {
        Form::Base => BASE
            .get_or_init(|| {
                let f = forward::<Action>("action");
                let p = build(Form::Base, &f.rule());
                f.define(p.clone());
                p
            })
            .clone(),
        Form::Third => THIRD.get_or_init(|| build(Form::Third, &action(Form::Base))).clone(),
        Form::Gerund => GERUND.get_or_init(|| build(Form::Gerund, &action(Form::Base))).clone(),
        Form::Past | Form::Participle | Form::Infinitive | Form::Cant | Form::Must => panic!("actions have no {form:?} form"),
    }
}

/// `base`: the base-form action grammar, for actions inside actions ("have it deal 2 damage").
fn build(form: Form, base: &P<Action>) -> P<Action> {
    let s = selectors();
    let v = |third: &str, base: &str| verb(form, third, base, "");
    // a number of things that is not a numeral: "a number of cards … equal to its power"
    let measured = measured_objects();
    let object = objects(&alt("object", vec![s.selector.clone(), measured.clone()]));
    // a zone after the object is spelled "from" ("exile ~ from your graveyard")
    let from = objects(&alt("object", vec![s.selector_from.clone(), s.selector.clone(), measured]));
    // a verb with what it is done to, typed by the lexicon entry of the same name (or `$verb`)
    macro_rules! verb_objects {
        ($third:literal, $base:literal, $objects:expr, $variant:ident) => {
            verb_objects!($third, $base, $objects, $variant, $variant)
        };
        ($third:literal, $base:literal, $objects:expr, $variant:ident, $verb:ident) => {
            with_object(v($third, $base), &slot(&$objects, Verb::$verb, Role::What), Action::$variant, |a| match a {
                Action::$variant(o) => Some(o),
                _ => None,
            })
        };
    }
    // "Put one into your hand and the other into your graveyard": "one" of the cards just named,
    // as the object of a move only ("one or two target creatures", "choose one")
    let one = Selector::new(Quant::Definite(ir::Ref::Anaphor(ir::selector::Anaphor::One)), ir::ObjectSet::of(ir::Domain::Object));
    let one_obj = Objects { items: vec![one], conj: Conj::And };
    // the slot of a role: several objects, or one noun phrase
    let what = |verb: Verb| slot(&object, verb, Role::What);
    let np = |verb: Verb, role: Role| slot_one(&s.selector, verb, role);
    let moved = alt("moved", vec![slot(&from, Verb::Move, Role::What), lit("one").to(one_obj)]);
    let zone_move = crate::zone_move::zone_move(form, &moved);
    let unattach = v("unattaches", "unattach").then_keep(&what(Verb::Unattach)).then(&lit("from").then_keep(&np(Verb::Unattach, Role::From)).opt()).map(
        |(what, from)| Some(Action::Unattach { what, from: from.map(Box::new) }),
        |a| match a {
            Action::Unattach { what, from } => Some((what.clone(), from.as_deref().cloned())),
            _ => None,
        },
    );
    let sacrifice = verb_objects!("sacrifices", "sacrifice", object, Sacrifice);
    let discard = verb_objects!("discards", "discard", object, Discard);
    let exile = verb_objects!("exiles", "exile", from, Exile);
    let tap = verb_objects!("taps", "tap", object, Tap);
    let untap = verb_objects!("untaps", "untap", object, Untap);
    let reveal = verb_objects!("reveals", "reveal", from, Reveal);
    let exert = verb_objects!("exerts", "exert", object, Exert);
    let pay = v("pays", "pay").then_keep(&payment());
    // "mills half their library, rounded down": half the cards in it
    let library = |p: Selector| {
        let mut set = ir::ObjectSet::of(ir::Domain::Card);
        set.zone = Some(ir::ZoneSpec { zones: ir::Coord::one(Zone::Library), owner: Some(Box::new(p)), slice: None });
        set
    };
    let rounding = alt(
        "rounding",
        vec![lit(", rounded up").to(Some(true)), lit(", rounded down").to(Some(false)), crate::kernel::empty().to(None)],
    );
    let half_library = lit("half").then_keep(&s.possessive).skip(&lit("library")).then(&rounding).map(
        move |(p, round_up)| Some(Amount::Half { of: Box::new(Amount::Count(Box::new(library(p)))), round_up }),
        move |a| match a {
            Amount::Half { of, round_up } => match of.as_ref() {
                Amount::Count(set) => {
                    let z = set.zone.as_ref()?;
                    let p = z.owner.as_deref()?;
                    (**set == library(p.clone())).then(|| (p.clone(), *round_up))
                }
                _ => None,
            },
            _ => None,
        },
    );
    let mill = v("mills", "mill").then_keep(&alt("milled", vec![cards(), half_library])).map(
        |n| Some(Action::Mill(n)),
        |a| match a {
            Action::Mill(n) => Some(n.clone()),
            _ => None,
        },
    );
    let exactly = count_amount().map(
        |n| Some(CounterCount::Exactly(n)),
        |c| match c {
            CounterCount::Exactly(n) => Some(n.clone()),
            _ => None,
        },
    );
    // "a +1/+1 counter on ~", "two -1/-1 counters on target creature"
    // "a +1/+1 counter", "two -1/-1 counters"
    let some_counters = crate::selector::some_counters();
    let one_kind = some_counters.map(|c| Some(vec![c]), |v: &Vec<Counters>| (v.len() == 1).then(|| v[0].clone()));
    // "a +1/+1 counter on ~", "two +1/+1 counters and a reach counter on target creature"
    let placement = alt("counters", vec![one_kind, some_counters.list("and")])
        .skip(&lit("on"))
        .then(&np(Verb::PutCounters, Role::On))
        .map(
            |(counters, on)| Some(Placement { counters, on: Box::new(on) }),
            |p: &Placement| Some((p.counters.clone(), (*p.on).clone())),
        );
    let one_placement = placement.map(|p| Some(vec![p]), |v: &Vec<Placement>| (v.len() == 1).then(|| v[0].clone()));
    let put_counters = v("puts", "put").then_keep(&alt("placements", vec![one_placement, placement.list("and")])).map(
        |v| Some(Action::PutCounters(v)),
        |a| match a {
            Action::PutCounters(v) => Some(v.clone()),
            _ => None,
        },
    );
    // "put your choice of a reach counter, a vigilance counter, or a trample counter on it";
    // "a +1/+1, first strike, or trample counter" names one counter of each kind
    let kinds_short = lit("a")
        .then_keep(&crate::selector::counter_kind().list("or"))
        .skip(&lit("counter"))
        .map(|kinds| Some(kinds.into_iter().map(|k| Counters::new(Amount::Const(1), k)).collect::<Vec<Counters>>()), |_: &Vec<Counters>| None)
        .alias();
    let put_choice = v("puts", "put")
        .skip(&lit("your choice of"))
        .then_keep(&alt("counter choices", vec![some_counters.list("or"), kinds_short]))
        .skip(&lit("on"))
        .then(&np(Verb::PutCounters, Role::On))
        .map(
            |(choices, on)| Some(Action::PutChoiceOfCounters { choices, on: Box::new(on) }),
            |a| match a {
                Action::PutChoiceOfCounters { choices, on } => Some((choices.clone(), (**on).clone())),
                _ => None,
            },
        );
    // "put a +1/+1 counter or a loyalty counter on it": the same choice, said without "your
    // choice of"
    let put_either = v("puts", "put")
        .then_keep(&some_counters.list("or"))
        .skip(&lit("on"))
        .then(&np(Verb::PutCounters, Role::On))
        .map(
            |(choices, on)| (choices.len() >= 2).then(|| Action::PutChoiceOfCounters { choices, on: Box::new(on) }),
            |_: &Action| None,
        )
        .alias();
    // "put a counter of that kind on it", "give that permanent or player another counter of that
    // kind": the kind the text before named
    let of_that_kind = alt(
        "a or another",
        vec![
            lit("a").to(None),
            lit("an additional").to(Some(ir::Extra::Additional)),
            lit("another").to(Some(ir::Extra::Another)),
        ],
    )
    .skip(&lit("counter of that kind"));
    let put_that_kind = v("puts", "put").then_keep(&of_that_kind).skip(&lit("on")).then(&np(Verb::PutCounters, Role::On)).map(
        |(extra, on)| Some(Action::PutCounterOfThatKind { extra, on: Box::new(on), give: false }),
        |a| match a {
            Action::PutCounterOfThatKind { extra, on, give: false } => Some((*extra, (**on).clone())),
            _ => None,
        },
    );
    let give_that_kind = v("gives", "give").then_keep(&np(Verb::PutCounters, Role::On)).then(&of_that_kind).map(
        |(on, extra)| Some(Action::PutCounterOfThatKind { extra, on: Box::new(on), give: true }),
        |a| match a {
            Action::PutCounterOfThatKind { extra, on, give: true } => Some(((**on).clone(), *extra)),
            _ => None,
        },
    );
    // "put the same number and kind of counters on ~"
    let put_same = v("puts", "put").skip(&lit("the same number and kind of counters on")).then_keep(&np(Verb::PutCounters, Role::On)).map(
        |on| Some(Action::PutSameCounters(Box::new(on))),
        |a| match a {
            Action::PutSameCounters(on) => Some((**on).clone()),
            _ => None,
        },
    );
    // "put a charge counter on ~ or remove one from it"
    let or_remove = alt("put a counter", vec![put_counters.clone(), put_that_kind.clone()]).skip(&lit("or remove one from it")).map(
        |a| Some(Action::OrRemoveOne(Box::new(a))),
        |a| match a {
            Action::OrRemoveOne(a) => Some((**a).clone()),
            _ => None,
        },
    );
    // "pay {1}{G} any number of times"
    let pay_times = pay.clone().skip(&lit("any number of times")).map(
        |a| Some(Action::AnyNumberOfTimes(Box::new(a))),
        |a| match a {
            Action::AnyNumberOfTimes(a) => Some((**a).clone()),
            _ => None,
        },
    );
    // "pay {G} up to three times"
    let pay_up_to = pay.clone().skip(&lit("up to")).then(&crate::words::count_amount()).skip(&lit("times")).map(
        |(a, n)| Some(Action::UpToTimes(Box::new(a), n)),
        |a| match a {
            Action::UpToTimes(a, n) => Some(((**a).clone(), n.clone())),
            _ => None,
        },
    );
    // "choose a counter on target permanent", "choose a kind of counter on a creature you control"
    let choose_counter = v("chooses", "choose")
        .skip(&alt("a counter on", vec![lit("a counter on"), lit("a kind of counter on").alias()]))
        .then_keep(&np(Verb::PutCounters, Role::On))
        .map(
            |on| Some(Action::ChooseCounterOn(Box::new(on))),
            |a| match a {
                Action::ChooseCounterOn(on) => Some((**on).clone()),
                _ => None,
            },
        );
    // "put a number of +1/+1 counters on it equal to …": the amount after the whole phrase is the
    // alias of the one after the noun ("counters equal to its power on it")
    let put_number = v("puts", "put")
        .skip(&lit("a number of"))
        .then_keep(&counter_kind())
        .skip(&lit("counters on"))
        .then(&np(Verb::PutCounters, Role::On))
        .skip(&lit("equal to"))
        .then(&crate::amount::amounts().amount)
        .map(
            |((kind, on), count)| {
                (!matches!(count, Amount::Const(_) | Amount::X | Amount::That { .. }))
                    .then(|| Action::PutCounters(vec![Placement { counters: vec![Counters::new(count, kind)], on: Box::new(on) }]))
            },
            |a| match a {
                Action::PutCounters(v) => match v.as_slice() {
                    [Placement { counters, on }] => match counters.as_slice() {
                        [Counters { count, up_to: false, extra: None, kind }]
                            if !matches!(count, Amount::Const(_) | Amount::X | Amount::That { .. }) =>
                        {
                            Some(((kind.clone(), (**on).clone()), count.clone()))
                        }
                        _ => None,
                    },
                    _ => None,
                },
                _ => None,
            },
        );
    let removal_count = alt(
        "how many counters",
        vec![
            exactly,
            lit("any number of").to(CounterCount::AnyNumber),
            lit("one or more").to(CounterCount::OneOrMore),
            lit("all").to(CounterCount::All),
            lit("up to").then_keep(&count_amount()).map(
                |n| Some(CounterCount::UpTo(n)),
                |c| match c {
                    CounterCount::UpTo(n) => Some(n.clone()),
                    _ => None,
                },
            ),
        ],
    );
    // "from among creatures you control": a bare plural set, counters split as the payer likes
    let among = lit("among").then_keep(&s.set_pl).map(
        |set| Some(Selector::new(Quant::All, set)),
        |sel: &Selector| (sel.quant == Quant::All && sel.chooser.is_none() && sel.random.is_none()).then(|| sel.set.clone()),
    );
    let source = alt(
        "counter source",
        vec![
            np(Verb::RemoveCounters, Role::From).map(|s| Some((s, false)), |(s, a)| (!a).then(|| s.clone())),
            among.map(|s| Some((s, true)), |(s, a)| a.then(|| s.clone())),
        ],
    );
    let remove_counters =
        v("removes", "remove").then_keep(&counters_of(removal_count.clone(), counter_kind().opt(), "from")).then(&source).map(
            |((count, kind), (from, among))| Some(Action::RemoveCounters { count, kind, from: Some(Box::new(from)), among }),
            |a| match a {
                Action::RemoveCounters { count, kind, from: Some(from), among } => {
                    Some(((count.clone(), kind.clone()), ((**from).clone(), *among)))
                }
                _ => None,
            },
        );
    // "target player loses all rad counters": the player's own counters removed (810.10b, 728.1)
    let lose_counters = v("loses", "lose").then_keep(&counters_of(removal_count.clone(), counter_kind().opt(), "")).map(
        |(count, kind)| Some(Action::RemoveCounters { count, kind, from: None, among: false }),
        |a| match a {
            Action::RemoveCounters { count, kind, from: None, .. } => Some((count.clone(), kind.clone())),
            _ => None,
        },
    );
    let n = || {
        alt(
            "N",
            vec![
                digits().map(
                    |n| Some(Amount::Const(n)),
                    |a| match a {
                        Amount::Const(n) => Some(*n),
                        _ => None,
                    },
                ),
                lit("X").to(Amount::X),
            ],
        )
    };
    let waterbend = v("waterbends", "waterbend").then_keep(&mana_cost()).map(
        |c| Some(Action::Waterbend(c)),
        |a| match a {
            Action::Waterbend(c) => Some(c.clone()),
            _ => None,
        },
    );
    macro_rules! verb_amount {
        ($third:literal, $base:literal, $amount:expr, $after:expr, $variant:ident) => {
            v($third, $base).then_keep(&$amount).skip(&$after).map(
                |n| Some(Action::$variant(n)),
                |a| match a {
                    Action::$variant(n) => Some(n.clone()),
                    _ => None,
                },
            )
        };
    }
    let nothing = crate::kernel::empty;
    let draw = verb_amount!("draws", "draw", cards(), nothing(), Draw);
    // "draw up to three cards": the drawer chooses how many, at most that (121.2b)
    let at_most = lit("up to").then_keep(&alt(
        "up to cards",
        vec![
            lit("one card").to(Amount::Const(1)),
            count_amount()
                .skip(&lit("cards"))
                .map(|n| (n != Amount::Const(1)).then_some(n), |n| (*n != Amount::Const(1)).then(|| n.clone())),
        ],
    ));
    let draw_up_to = verb_amount!("draws", "draw", at_most, nothing(), DrawUpTo);
    let gain_life = verb_amount!("gains", "gain", noun_amount("life"), nothing(), GainLife);
    let lose_life = verb_amount!("loses", "lose", noun_amount("life"), nothing(), LoseLife);
    // "that player gains no life instead": none
    let gain_no_life =
        v("gains no life", "gain no life").map(|()| Some(Action::GainLife(Amount::Const(0))), |_: &Action| None).alias();
    let scry = verb_amount!("scries", "scry", n(), nothing(), Scry);
    let surveil = verb_amount!("surveils", "surveil", n(), nothing(), Surveil);
    // "create a Treasure token", "create X 1/1 Halfling tokens and X Food tokens", "create a Food
    // token or a Treasure token" (the creator chooses one, 608.2d)
    let and_or = |c: &Coord<ir::TokenSpec>| matches!(c.conj, Conj::And | Conj::Or);
    let tokens = coord(&crate::token::token_spec()).map(move |c| and_or(&c).then_some(c), move |c| and_or(c).then(|| c.clone()));
    // "create your choice of a Clue token, a Food token, or a Treasure token": one of them, said
    // with "or" alone too
    let your_choice = v("creates", "create")
        .skip(&lit("your choice of"))
        .then_keep(&tokens)
        .map(|t| (t.conj == Conj::Or && t.items.len() >= 2).then(|| Action::Create(t)), |_: &Action| None)
        .alias();
    let create = v("creates", "create").then_keep(&tokens).map(
        |t| Some(Action::Create(t)),
        |a| match a {
            Action::Create(t) => Some(t.clone()),
            _ => None,
        },
    );
    let create = alt("create", vec![create, your_choice]);
    // "3 damage to any target", "1 damage to any target and 1 damage to you"
    let damaged = slot(&object, Verb::DealDamage, Role::To);
    let hit = noun_amount("damage").skip(&lit("to")).then(&damaged).map(
        |(amount, to)| Some(Hit { amount, to, amount_after_targets: false, condition: None }),
        |h: &Hit| (!h.amount_after_targets && h.condition.is_none()).then(|| (h.amount.clone(), h.to.clone())),
    );
    let guarded_hit = hit.then(&lit("if").then_keep(&crate::kernel::lazy("damage condition", || crate::condition::conditions().condition.clone())).opt()).map(
        |(mut hit, condition)| { hit.condition = condition.map(Box::new); Some(hit) },
        |hit: &Hit| {
            let mut plain = hit.clone(); plain.condition = None;
            Some((plain, hit.condition.as_deref().cloned()))
        },
    );
    // A local guard on the first part establishes independent part scope for the list.
    // A lone trailing guard keeps the existing whole-action modifier representation.
    let guarded_hits = guarded_hit.list("and").map(
        |hits| hits.first().is_some_and(|h| h.condition.is_some()).then_some(hits),
        |hits: &Vec<Hit>| (hits.len() >= 2 && hits[0].condition.is_some()).then(|| hits.clone()),
    );
    let one_hit = hit.map(|h| Some(vec![h]), |v: &Vec<Hit>| (v.len() == 1).then(|| v[0].clone()));
    let deal_damage = v("deals", "deal").then_keep(&alt("hits", vec![one_hit, hit.list("and"), guarded_hits])).map(
        |v| Some(Action::DealDamage(v)),
        |a| match a {
            Action::DealDamage(v) => Some(v.clone()),
            _ => None,
        },
    );
    let deal_divided = v("deals", "deal")
        .then_keep(&noun_amount("damage"))
        .then(&alt("damage division", vec![
            lit("divided as you choose").to(ir::action::DamageDivision::Chosen),
            lit("divided evenly, rounded").then_keep(&alt("rounding", vec![lit("up").to(true), lit("down").to(false)])).skip(&lit(","))
                .map(|round_up| Some(ir::action::DamageDivision::Evenly { round_up }),
                    |d| match d { ir::action::DamageDivision::Evenly { round_up } => Some(*round_up), _ => None }),
        ]))
        .skip(&lit("among")).then(&damaged)
        .map(
            |((amount, division), among)| Some(Action::DealDivided { amount, among, division }),
            |a| match a {
                Action::DealDivided { amount, among, division } => Some(((amount.clone(), *division), among.clone())),
                _ => None,
            },
        );
    let flip = verb_objects!("flips", "flip", object, Flip);
    let emblem = v("gets an emblem with", "get an emblem with").then_keep(&granted()).map(
        |g| Some(Action::GetEmblem(g)),
        |a| match a {
            Action::GetEmblem(g) => Some(g.clone()),
            _ => None,
        },
    );
    let separate = v("separates", "separate")
        .then_keep(&what(Verb::SeparateIntoPiles))
        .skip(&lit("into"))
        .then(&crate::procedure::piles())
        .map(
            |(what, piles)| Some(Action::SeparateIntoPiles { what, piles }),
            |a| match a {
                Action::SeparateIntoPiles { what, piles } => Some((what.clone(), piles.clone())),
                _ => None,
            },
        );
    let remove_from_combat = v("removes", "remove").then_keep(&what(Verb::RemoveFromCombat)).skip(&lit("from combat")).map(
        |o| Some(Action::RemoveFromCombat(o)),
        |a| match a {
            Action::RemoveFromCombat(o) => Some(o.clone()),
            _ => None,
        },
    );
    let move_counters = v("moves", "move")
        .then_keep(&counters_of(removal_count, counter_kind().opt(), "from"))
        .then(&np(Verb::MoveCounters, Role::From))
        .skip(&lit("onto"))
        .then(&np(Verb::MoveCounters, Role::To))
        .map(
            |(((count, kind), from), onto)| Some(Action::MoveCounters { count, kind, from: Box::new(from), onto: Box::new(onto) }),
            |a| match a {
                Action::MoveCounters { count, kind, from, onto } => {
                    Some((((count.clone(), kind.clone()), (**from).clone()), (**onto).clone()))
                }
                _ => None,
            },
        );
    let turn_face = v("turns", "turn")
        .then_keep(&what(Verb::TurnFace))
        .then(&alt("face", vec![lit("face up").to(true), lit("face down").to(false)]))
        .map(
            |(what, up)| Some(Action::TurnFace { what, up }),
            |a| match a {
                Action::TurnFace { what, up } => Some((what.clone(), *up)),
                _ => None,
            },
        );
    let double_pt = crate::becomes::scale_stat(form);
    let double_life = crate::becomes::double_life(form);
    // "pay any amount of {E}" (nothing is allowed), "pay one or more {E}" (at least one)
    let pay_any = v("pays", "pay")
        .then_keep(&alt(
            "any amount of",
            vec![
                lit("any amount of")
                    .then_keep(&alt(
                        "what",
                        vec![
                            lit("mana").to(ir::AnyAmountOf::Mana),
                            lit("{E}").to(ir::AnyAmountOf::Energy),
                            lit("life").to(ir::AnyAmountOf::Life),
                        ],
                    ))
                    .map(|w| Some((w, false)), |(w, one): &(ir::AnyAmountOf, bool)| (!one).then(|| w.clone())),
                lit("one or more")
                    .then_keep(&lit("{E}").to(ir::AnyAmountOf::Energy))
                    .map(|w| Some((w, true)), |(w, one): &(ir::AnyAmountOf, bool)| one.then(|| w.clone())),
            ],
        ))
        .map(
            |(what, at_least_one)| Some(Action::PayAnyAmount { what, at_least_one }),
            |a| match a {
                Action::PayAnyAmount { what, at_least_one } => Some((what.clone(), *at_least_one)),
                _ => None,
            },
        );
    let switch_pt = v("switches", "switch")
        .then_keep(&alt(
            "whose power and toughness",
            vec![
                slot_one(&crate::amount::owner_of(), Verb::SwitchPt, Role::Of).skip(&lit("power and toughness")).map(|o| Some((o, true)), |(o, p)| p.then(|| o.clone())),
                lit("the power and toughness of").then_keep(&np(Verb::SwitchPt, Role::Of)).map(|o| Some((o, false)), |(o, p)| (!p).then(|| o.clone())),
            ],
        ))
        .map(
            |(of, possessive)| Some(Action::SwitchPt { of: Box::new(of), possessive }),
            |a| match a {
                Action::SwitchPt { of, possessive } => Some(((**of).clone(), *possessive)),
                _ => None,
            },
        );
    let distribute = v("distributes", "distribute")
        .then_keep(&crate::selector::some_counters())
        .skip(&lit("among"))
        .then(&slot(&object, Verb::Distribute, Role::Among))
        .map(
            |(counters, among)| Some(Action::Distribute { counters, among }),
            |a| match a {
                Action::Distribute { counters, among } => Some((counters.clone(), among.clone())),
                _ => None,
            },
        );
    // The recipients are named before the amount's references are evaluated (608.2h).
    let damage_late = v("deals", "deal")
        .skip(&lit("damage to"))
        .then_keep(&damaged)
        .skip(&lit("equal to"))
        .then(&crate::amount::amounts().amount)
        .map(|(to, amount)| Some(Action::DealDamage(vec![Hit { amount, to, amount_after_targets: true, condition: None }])), |a| match a {
            Action::DealDamage(hits) if hits.len() == 1 && hits[0].amount_after_targets && hits[0].condition.is_none() => Some((hits[0].to.clone(), hits[0].amount.clone())),
            _ => None,
        });
    let gets = v("gets", "get").then_keep(&delta()).skip(&lit("/").tight()).then(&delta().tight()).map(
        |(power, toughness)| Some(Action::Gets { power, toughness }),
        |a| match a {
            Action::Gets { power, toughness } => Some((power.clone(), toughness.clone())),
            _ => None,
        },
    );
    // "gets +1/-1 or -1/+1"
    let change = delta().skip(&lit("/").tight()).then(&delta().tight());
    let gets_either = v("gets", "get").then_keep(&change).skip(&lit("or")).then(&change).map(
        |(a, b)| Some(Action::GetsEither(vec![a, b])),
        |a| match a {
            Action::GetsEither(v) => match v.as_slice() {
                [a, b] => Some((a.clone(), b.clone())),
                _ => None,
            },
            _ => None,
        },
    );
    // "gains all activated abilities of target creature"
    let taken = |g: &Vec<Granted>| matches!(g.as_slice(), [Granted::AbilitiesOf { .. }]);
    let gains_of = v("gains", "gain").then_keep(&crate::kernel::lazy("granted", crate::statics::granted_any)).map(
        move |g| taken(&g).then(|| Action::Gains(g)),
        move |a| match a {
            Action::Gains(g) if taken(g) => Some(g.clone()),
            _ => None,
        },
    );
    let gains = v("gains", "gain").then_keep(&granted()).map(
        |g| Some(Action::Gains(g)),
        |a| match a {
            Action::Gains(g) => Some(g.clone()),
            _ => None,
        },
    );
    let cant = crate::statics::cant(form);
    let destroy = verb_objects!("destroys", "destroy", object, Destroy);
    let regenerate = verb_objects!("regenerates", "regenerate", object, Regenerate);
    // "you may look at cards exiled with ~", "you may play cards exiled this way": a bare
    // plural, alone, is one or more of them
    let bare = crate::selector::bare_plural().map(|s| Some(ir::Coord::one(s)), |_: &Objects| None).alias();
    let from_or_bare = alt("objects", vec![from.clone(), bare]);
    let look_at = verb_objects!("looks at", "look at", from_or_bare, LookAt);
    // "change the text of target permanent by replacing all instances of one color word with
    // another or one basic land type with another" (612)
    let text_word = alt(
        "kind of word",
        vec![
            lit("color word").to(ir::action::TextWord::Color),
            lit("basic land type").to(ir::action::TextWord::BasicLandType),
            lit("creature type").to(ir::action::TextWord::CreatureType),
        ],
    );
    let subtype = leaf(
        "subtype",
        |k| match k {
            Kind::Word(w) => ir::Subtype::from_name(w),
            _ => None,
        },
        |s: &ir::Subtype| Some(Kind::Word(s.name().to_string())),
    );
    let replaced = lit("one").then_keep(&text_word).skip(&lit("with"));
    let to_another = replaced.skip(&lit("another"));
    let several = alt(
        "replacements",
        vec![to_another.map(|w| Some(vec![w]), |v: &Vec<ir::action::TextWord>| (v.len() == 1).then(|| v[0])), to_another.list("or")],
    )
    .map(|words| Some((words, None)), |(words, to): &(Vec<ir::action::TextWord>, Option<ir::Subtype>)| to.is_none().then(|| words.clone()));
    let to_named = replaced.then(&subtype).map(
        |(word, to)| Some((vec![word], Some(to))),
        |(words, to): &(Vec<ir::action::TextWord>, Option<ir::Subtype>)| match (words.as_slice(), to) {
            ([word], Some(to)) => Some((*word, *to)),
            _ => None,
        },
    );
    let change_text = v("changes", "change")
        .skip(&lit("the text of"))
        .then_keep(&s.selector)
        .skip(&lit("by replacing all instances of"))
        .then(&alt("replacement", vec![several, to_named]))
        .map(
            |(what, (words, to))| Some(Action::ChangeText { what: Box::new(what), words, to }),
            |a| match a {
                Action::ChangeText { what, words, to } => Some(((**what).clone(), (words.clone(), *to))),
                _ => None,
            },
        );
    // "meld them into Hanweir, the Writhing Township" (701.42a)
    let meld = v("melds", "meld").skip(&lit("them into")).then_keep(&crate::selector::name()).map(
        |name| Some(Action::Meld(name)),
        |a| match a {
            Action::Meld(name) => Some(name.clone()),
            _ => None,
        },
    );
    let new_targets = alt(
        "new targets",
        vec![
            v("chooses new targets for", "choose new targets for").to(false),
            v("chooses a new target for", "choose a new target for").to(true),
        ],
    )
    .then(&what(Verb::ChangeTarget))
    .map(
        |(one, for_)| Some(Action::ChooseNewTargets { for_, one }),
        |a| match a {
            Action::ChooseNewTargets { for_, one } => Some((*one, for_.clone())),
            _ => None,
        },
    );
    let choice = crate::choice::choice(&from, &crate::kernel::lazy("modes", modes), &s.set_sg);
    let choose = v("chooses", "choose").then_keep(&choice).map(
        |c| Some(Action::Choose(c)),
        |a| match a {
            Action::Choose(c) => Some(c.clone()),
            _ => None,
        },
    );
    let gain_control = verb_objects!("gains control of", "gain control of", object, GainControl);
    let a_an = || alt("a", vec![lit("a"), lit("an").alias()]);
    let named_counters = alt(
        "player counters",
        vec![
            a_an()
                .then_keep(&counter_kind())
                .skip(&lit("counter"))
                .map(|k| Some((Amount::Const(1), k)), |(n, k)| (*n == Amount::Const(1)).then(|| k.clone())),
            count_amount().then(&counter_kind()).skip(&lit("counters")).map(
                |(n, k)| (n != Amount::Const(1)).then_some((n, k)),
                |(n, k)| (*n != Amount::Const(1)).then(|| (n.clone(), k.clone())),
            ),
            // "a number of poison counters equal to the difference"
            lit("a number of")
                .then_keep(&counter_kind())
                .skip(&lit("counters equal to"))
                .then(&crate::kernel::lazy("amount", || crate::amount::amounts().amount.clone()))
                .map(
                    |(k, n)| (!crate::words::is_quantity(&n)).then_some((n, k)),
                    |(n, k)| (!crate::words::is_quantity(n)).then(|| (k.clone(), n.clone())),
                ),
        ],
    )
    .map(
        |(count, k)| Some((count, PlayerCounter::Named(k))),
        |(count, k)| match k {
            PlayerCounter::Named(k) => Some((count.clone(), k.clone())),
            PlayerCounter::Energy => None,
        },
    );
    let energy_counters =
        energy().map(|n| Some((n, PlayerCounter::Energy)), |(n, k)| (*k == PlayerCounter::Energy).then(|| n.clone()));
    let get_counters = v("gets", "get").then_keep(&alt("counters", vec![energy_counters, named_counters])).map(
        |(count, kind)| Some(Action::GetCounters { count, kind }),
        |a| match a {
            Action::GetCounters { count, kind } => Some((count.clone(), kind.clone())),
            _ => None,
        },
    );
    // (tapped, attacking)
    let state = alt(
        "token state",
        vec![
            crate::kernel::empty().to((false, false)),
            lit("tapped").to((true, false)),
            lit("tapped and attacking").to((true, true)),
        ],
    );
    let one_copy = alt("a", vec![lit("a"), lit("an").alias()])
        .then_keep(&state)
        .skip(&lit("token that's a copy of"))
        .map(|st| Some((Amount::Const(1), st)), |(n, st)| (*n == Amount::Const(1)).then_some(*st));
    let copies = count_amount().then(&state).skip(&lit("tokens that are copies of")).map(
        |(n, st)| (n != Amount::Const(1)).then_some((n, st)),
        |(n, st)| (*n != Amount::Const(1)).then(|| (n.clone(), *st)),
    );
    // "a token that's a copy of ~ that's tapped and attacking that player or a planeswalker
    // they control": what it attacks, said after what it copies (508.4). Said without what
    // it attacks, the state is the early "a tapped and attacking token", which prints.
    let copy = |number: Number, head: &P<(Amount, (bool, bool))>| v("creates", "create").then_keep(head)
        .then(&np(Verb::CreateCopy, Role::Of)).then(&crate::token::placement(number).opt()).map(
        |(((count, (tapped, attacking)), of), late)| match late {
            None => Some(Action::CreateCopy { count, tapped, attacking, of: Box::new(of), defender: None }),
            Some((late_tapped, ir::token::Combat::Attacking(Some(who)))) if !tapped && !attacking =>
                Some(Action::CreateCopy { count, tapped: late_tapped, attacking: true, of: Box::new(of), defender: Some(who) }),
            Some(_) => None,
        },
        |a| match a {
            Action::CreateCopy { count, tapped, attacking, of, defender: None } => {
                Some((((count.clone(), (*tapped, *attacking)), (**of).clone()), None))
            }
            Action::CreateCopy { count, tapped, attacking: true, of, defender: Some(who) } => {
                Some((((count.clone(), (false, false)), (**of).clone()), Some((*tapped, ir::token::Combat::Attacking(Some(who.clone()))))))
            }
            _ => None,
        },
    );
    let create_copy = alt("create copy", vec![copy(Number::Sg, &one_copy), copy(Number::Pl, &copies)]);
    // "create a token that's a copy of that card and that's tapped and attacking", "… that
    // card that's tapped and attacking": the state said after what is copied
    let create_copy_late = v("creates", "create")
        .skip(&lit("a token that's a copy of"))
        .then_keep(&np(Verb::CreateCopy, Role::Of))
        .skip(&alt("late state", vec![lit("and that's tapped and attacking"), lit("that's tapped and attacking")]))
        .map(
            |of| Some(Action::CreateCopy { count: Amount::Const(1), tapped: true, attacking: true, of: Box::new(of), defender: None }),
            |_: &Action| None,
        )
        .alias();
    // the one made to act is the doer of the action inside
    // "you may have creatures you control get +1/+1": a bare plural is all of them
    let all_of = crate::kernel::lazy("plural set", || selectors().set_pl.clone())
        .map(|set| Some(Selector::new(Quant::All, set)), |_: &Selector| None)
        .alias();
    let have = v("has", "have").then_keep(&alt("made to act", vec![s.selector.clone(), all_of])).then(base).map(
        |(who, action)| does(Some(&who), &action).then(|| Action::Have { who: Box::new(who), action: Box::new(action) }),
        |a| match a {
            Action::Have { who, action } => Some(((**who).clone(), (**action).clone())),
            _ => None,
        },
    );
    // "your library", "its controller's graveyard, hand, and library", "your library and/or graveyard"
    let search = v("searches", "search")
        .then_keep(&s.possessive)
        .then(&crate::selector::zones())
        .skip(&lit("for"))
        .then(&what(Verb::Search))
        .map(
            |((owner, zones), what)| {
                Some(Action::Search { zone: ir::ZoneSpec { zones, owner: Some(Box::new(owner)), slice: None }, what })
            },
            |a| match a {
                Action::Search { zone, what } if zone.slice.is_none() => {
                    Some((((**zone.owner.as_ref()?).clone(), zone.zones.clone()), what.clone()))
                }
                _ => None,
            },
        );
    let counter = verb_objects!("counters", "counter", object, Counter);
    let tap_or_untap = verb_objects!("taps or untaps", "tap or untap", object, TapOrUntap, Tap);
    let color = crate::kernel::table("color", ir::Color::ALL.iter().map(|c| (c.word().to_string(), *c)).collect());
    let vote_for = alt(
        "vote for",
        vec![
            crate::vote::option().list("or").map(
                |v| Some(ir::VoteFor::Words(v)),
                |f| match f {
                    ir::VoteFor::Words(v) => Some(v.clone()),
                    _ => None,
                },
            ),
            color.list("or").map(
                |v| Some(ir::VoteFor::Colors(v)),
                |f| match f {
                    ir::VoteFor::Colors(v) => Some(v.clone()),
                    _ => None,
                },
            ),
            s.selector.map(
                |x| Some(ir::VoteFor::Objects(Box::new(x))),
                |f| match f {
                    ir::VoteFor::Objects(x) => Some((**x).clone()),
                    _ => None,
                },
            ),
        ],
    );
    let vote = v("votes for", "vote for").then_keep(&vote_for).map(
        |f| Some(Action::Vote(f)),
        |a| match a {
            Action::Vote(f) => Some(f.clone()),
            _ => None,
        },
    );
    // "faces a villainous choice — A, or B" (701.55a): two options, each instructions of its own
    let option = crate::kernel::lazy("option", || crate::effect::effects().body.clone());
    let villainous = v("faces a villainous choice", "face a villainous choice")
        .skip(&lit("—"))
        .then_keep(&option)
        .skip(&lit(", or"))
        .then(&option)
        .map(
            |(a, b)| Some(Action::FaceVillainousChoice(Box::new([a, b]))),
            |x| match x {
                Action::FaceVillainousChoice(o) => Some((o[0].clone(), o[1].clone())),
                _ => None,
            },
        );
    let exchange_control = verb_objects!("exchanges control of", "exchange control of", object, ExchangeControl);
    let change_target = verb_objects!("changes the target of", "change the target of", object, ChangeTarget);
    // "reselect which player or permanent target attacking creature is attacking"
    let reselect = alt(
        "reselect",
        vec![
            v("reselects which player or permanent", "reselect which player or permanent").to(false),
            v("reselects which player", "reselect which player").to(true),
        ],
    )
    .then(&s.selector)
    .skip(&lit("is attacking"))
    .map(
        |(player_only, attacker)| Some(Action::ReselectAttacked { attacker: Box::new(attacker), player_only }),
        |a| match a {
            Action::ReselectAttacked { attacker, player_only } => Some((*player_only, (**attacker).clone())),
            _ => None,
        },
    );
    // "change a target of target spell or ability to ~"
    let change_a_target = v("changes a target of", "change a target of").then_keep(&s.selector).skip(&lit("to")).then(&s.selector).map(
        |(of, to)| Some(Action::ChangeATargetTo { of: Box::new(of), to: Box::new(to) }),
        |a| match a {
            Action::ChangeATargetTo { of, to } => Some(((**of).clone(), (**to).clone())),
            _ => None,
        },
    );
    let life_totals = v("exchanges life totals", "exchange life totals")
        .then_keep(&lit("with").then_keep(&np(Verb::ExchangeLifeTotals, Role::With)).opt())
        .map(
            |with| Some(Action::ExchangeLifeTotals { with: with.map(Box::new) }),
            |a| match a {
                Action::ExchangeLifeTotals { with } => Some(with.as_deref().cloned()),
                _ => None,
            },
        );
    // "exchange your life total with ~'s toughness" (701.12g): a player's life total and a
    // permanent's power or toughness
    let power_or_toughness =
        alt("power or toughness", vec![lit("power").to(ir::Stat::Power), lit("toughness").to(ir::Stat::Toughness)]);
    let life_with_stat = v("exchanges", "exchange")
        .then_keep(&slot_one(&s.possessive, Verb::ExchangeLifeWithStat, Role::Of))
        .skip(&lit("life total with"))
        .then(&slot_one(&crate::amount::owner_of(), Verb::ExchangeLifeWithStat, Role::With))
        .then(&power_or_toughness)
        .map(
            |((player, of), stat)| Some(Action::ExchangeLifeWithStat { player: Box::new(player), of: Box::new(of), stat }),
            |a| match a {
                Action::ExchangeLifeWithStat { player, of, stat } => Some((((**player).clone(), (**of).clone()), *stat)),
                _ => None,
            },
        );
    // plain "loses flying" / "loses all abilities" and skips are static acts (`crate::statics`)
    let loses_choice = v("loses your choice of", "lose your choice of").then_keep(&granted_or()).map(
        |g| Some(Action::LosesChoice(g)),
        |a| match a {
            Action::LosesChoice(g) => Some(g.clone()),
            _ => None,
        },
    );
    let gains_choice = v("gains your choice of", "gain your choice of").then_keep(&granted_or()).map(
        |g| Some(Action::GainsChoice(g)),
        |a| match a {
            Action::GainsChoice(g) => Some(g.clone()),
            _ => None,
        },
    );
    let additional = alt(
        "additional cards",
        vec![
            lit("an additional card").to(Amount::Const(1)),
            count_amount()
                .skip(&lit("additional cards"))
                .map(|n| (n != Amount::Const(1)).then_some(n), |n| (*n != Amount::Const(1)).then(|| n.clone())),
        ],
    );
    let draw_additional = v("draws", "draw").then_keep(&additional).map(
        |n| Some(Action::DrawAdditional(n)),
        |a| match a {
            Action::DrawAdditional(n) => Some(n.clone()),
            _ => None,
        },
    );
    let counters_of = alt("whose counters", vec![lit("its counters on").to(CountersOf::Its), lit("those counters on").to(CountersOf::Those)]);
    let put_counters_of = v("puts", "put").then_keep(&counters_of).then(&np(Verb::MoveCounters, Role::To)).map(
        |(whose, on)| Some(Action::PutCountersOf { whose, on: Box::new(on) }),
        |a| match a {
            Action::PutCountersOf { whose, on } => Some((*whose, (**on).clone())),
            _ => None,
        },
    );
    // "shuffle it into its owner's library", "shuffles their graveyard into their library"
    let shuffled = slot(&from, Verb::Shuffle, Role::What);
    let shuffle_into = |n: Number| {
        let what = shuffled.map(
            move |o| (plural_objects(&o) == (n == Number::Pl)).then_some(o),
            move |o| (plural_objects(o) == (n == Number::Pl)).then(|| o.clone()),
        );
        what.skip(&lit("into")).then(&crate::zone_move::whose(n, "library", "libraries"))
    };
    let shuffle_into = v("shuffles", "shuffle")
        .then_keep(&alt("shuffle into", vec![shuffle_into(Number::Sg), shuffle_into(Number::Pl)]))
        .then(&crate::zone_move::origin().opt())
        .map(
            |((what, whose), from)| {
                crate::zone_move::origin_fits([&what], 1, &from).then_some(Action::ShuffleInto { what, whose, from })
            },
            |a| match a {
                Action::ShuffleInto { what, whose, from } if crate::zone_move::origin_fits([what], 1, from) => {
                    Some(((what.clone(), whose.clone()), from.clone()))
                }
                _ => None,
            },
        );
    let double_counters = v("doubles the number of", "double the number of")
        .then_keep(&alt(
            "counters",
            vec![
                counter_kind().skip(&lit("counters on")).map(|k| Some(Some(k)), |k: &Option<CounterKind>| k.clone()),
                lit("each kind of counter on").to(None),
            ],
        ))
        .then(&np(Verb::DoubleCounters, Role::On))
        .map(
            |(kind, on)| Some(Action::DoubleCounters { kind, on: Box::new(on) }),
            |a| match a {
                Action::DoubleCounters { kind, on } => Some((kind.clone(), (**on).clone())),
                _ => None,
            },
        );
    let copy = verb_objects!("copies", "copy", object, Copy);
    // "cast that card", "play it this turn", "cast the copy without paying its mana cost"
    // of cards, or of spells from among a group ("cast instant and sorcery spells from among
    // them"): "play lands and cast spells from …" is a permission of its own (statics)
    fn said_among(f: &ir::Filter) -> bool {
        match f {
            ir::Filter::Atom(ir::Atom::Among(_) | ir::Atom::AmongSet(_)) => true,
            ir::Filter::All(v) => v.iter().any(said_among),
            _ => false,
        }
    }
    let bare_cards = crate::selector::bare_plural()
        .map(
            // of any kind when from among a group: "play lands from among cards exiled with ~"
            |s| (s.set.domain == ir::Domain::Card || said_among(&s.set.filter)).then(|| ir::Coord::one(s)),
            |_: &Objects| None,
        )
        .alias();
    let castable = slot(&alt("castable", vec![from.clone(), bare_cards]), Verb::Play, Role::What);
    let free = alt(
        "free",
        vec![
            castable
                .skip(&lit("without paying its mana cost"))
                .map(|o| (!plural_objects(&o)).then_some((o, true)), |(o, f)| (*f && !plural_objects(o)).then(|| o.clone())),
            castable
                .skip(&lit("without paying their mana costs"))
                .map(|o| plural_objects(&o).then_some((o, true)), |(o, f)| (*f && plural_objects(o)).then(|| o.clone())),
            castable.map(|o| Some((o, false)), |(o, f)| (!f).then(|| o.clone())),
        ],
    );
    let cast_verb = alt("cast", vec![v("casts", "cast").to(false), v("plays", "play").to(true)]);
    let cast = cast_verb.then(&free).map(
        |(play, (what, free))| {
            let cast = Verb::Cast.kinds(Role::What);
            (play || what.items.iter().all(|s| of_kind(s, cast))).then_some(Action::Cast { play, what, free })
        },
        |a| match a {
            Action::Cast { play, what, free } => Some((*play, (what.clone(), *free))),
            _ => None,
        },
    );
    // "play lands and cast spells from among cards exiled with ~": play those cards
    let play_among = v("plays lands and casts spells from among", "play lands and cast spells from among")
        .then_keep(&alt("group", vec![crate::selector::bare_plural(), s.plural.clone()]))
        .map(|s| Some(Action::Cast { play: true, what: ir::Coord::one(s), free: false }), |_: &Action| None)
        .alias();
    // "look at and play those cards": both permissions over the same cards
    let look_and_cast = alt(
        "look at and cast",
        vec![v("looks at and casts", "look at and cast").to(false), v("looks at and plays", "look at and play").to(true)],
    )
    .then(&castable)
    .map(
        |(play, what)| Some(Action::LookAtAndCast { play, what }),
        |a| match a {
            Action::LookAtAndCast { play, what } => Some((*play, what.clone())),
            _ => None,
        },
    );
    // "you control target player during that player's next turn" (723)
    let control_player = v("controls", "control")
        .then_keep(&s.selector)
        .skip(&alt("next turn", vec![lit("during that player's next turn"), lit("during their next turn").alias()]))
        .map(
            |who| (who.set.domain == ir::Domain::Player).then(|| Action::ControlPlayerNextTurn(Box::new(who))),
            |a| match a {
                Action::ControlPlayerNextTurn(who) => Some((**who).clone()),
                _ => None,
            },
        );
    // "secretly choose an opponent", "secretly votes for a player"
    let secretly = lit("secretly").then_keep(&alt("secret", vec![vote.clone(), choose.clone()])).map(
        |a| Some(Action::Secretly(Box::new(a))),
        |a| match a {
            Action::Secretly(a) => Some((**a).clone()),
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
    let additional_lands = v("plays", "play").then_keep(&lands).map(
        |n| Some(Action::PlayAdditionalLands(n)),
        |a| match a {
            Action::PlayAdditionalLands(n) => Some(n.clone()),
            _ => None,
        },
    );
    let numbers: &[Number] = if form == Form::Third { &[Number::Sg] } else { &[Number::Sg, Number::Pl] };
    let becomes = v("becomes", "become").then_keep(&crate::becomes::becoming(numbers)).map(
        |b| Some(Action::Becomes(b)),
        |a| match a {
            Action::Becomes(b) => Some(b.clone()),
            _ => None,
        },
    );
    let untap_whose = alt(
        "whose untap step",
        vec![
            // "its" after a singular subject, "their" after a plural one
            if form == Form::Third {
                alt("controller's", vec![lit("its controller's"), lit("their controller's").alias()])
            } else {
                alt("controller's", vec![lit("their controller's"), lit("its controller's").alias()])
            }
            .to(UntapWhose::Controller),
            lit("your").to(UntapWhose::Yours),
            lit("that player's").to(UntapWhose::ThatPlayer),
            // "lands target player controls don't untap during their next untap step": that
            // player's, who controls them
            lit("their").to(UntapWhose::ThatPlayer).alias(),
        ],
    );
    let step = untap_whose.then(&lit("next").opt()).skip(&lit("untap step"));
    let steps = lit("their controllers'").then_keep(&lit("next").opt()).skip(&lit("untap steps"));
    let doesnt_untap = v("doesn't untap during", "don't untap during")
        .then_keep(&alt("untap step", vec![step, steps.map(|n| Some((UntapWhose::Controllers, n)), |(w, n)| (*w == UntapWhose::Controllers).then_some(*n))]))
        .map(
            |(whose, next)| Some(Action::DoesntUntap { whose, next: next.is_some() }),
            |a| match a {
                Action::DoesntUntap { whose, next } => Some((*whose, next.then_some(()))),
                _ => None,
            },
        );
    let keep_mana = v("doesn't lose", "don't lose")
        .then_keep(&crate::mana::pooled())
        .skip(&lit("as steps"))
        .then(&lit("and phases").opt())
        .skip(&lit("end"))
        .map(
            |(mana, p)| Some(Action::KeepMana { mana, phases: p.is_some() }),
            |a| match a {
                Action::KeepMana { mana, phases } => Some((*mana, phases.then_some(()))),
                _ => None,
            },
        );
    // "lose all unspent mana" (106.4)
    let lose_unspent = v("loses all", "lose all").then_keep(&crate::mana::unspent()).map(
        |c| Some(Action::LoseUnspentMana(c)),
        |a| match a {
            Action::LoseUnspentMana(c) => Some(*c),
            _ => None,
        },
    );
    let set_stat = crate::becomes::set_stat(form);
    // "exile cards from the top of your library until you exile …": "your … you" for an imperative,
    // "their … they" after a third-person subject
    let until = |verb: UntilVerb, third: &'static str, base: &'static str| {
        let (own, other) = if form == Form::Third { ("their", "your") } else { ("your", "their") };
        let pronoun = |o: &str| if o == "your" { "you" } else { "they" };
        let tail = |o: &'static str| lit(&format!("cards from the top of {o} library until {}", pronoun(o))).skip(&lit(base));
        let active = v(third, base)
            .then_keep(&alt("until", vec![tail(own), tail(other).alias()]))
            .then_keep(&np(Verb::RevealUntil, Role::What))
            .map(
                move |x| Some(Action::Until { verb, until: Box::new(x) }),
                move |a| match a {
                    Action::Until { verb: w, until } if *w == verb => Some((**until).clone()),
                    _ => None,
                },
            );
        // "until a creature card is revealed" has the same stop selector as "until they
        // reveal a creature card". Keep grammatical number and the declared card role.
        let participle = match verb { UntilVerb::Reveal => "revealed", UntilVerb::Exile => "exiled" };
        let stopped = alt("passive until", vec![
            slot_one(&s.singular, Verb::RevealUntil, Role::What).skip(&lit(&format!("is {participle}"))),
            slot_one(&s.plural, Verb::RevealUntil, Role::What).skip(&lit(&format!("are {participle}"))),
        ]);
        let passive = v(third, base)
            .skip(&lit(&format!("cards from the top of {own} library until")))
            .then_keep(&stopped)
            .map(move |until| Some(Action::Until { verb, until: Box::new(until) }), |_| None).alias();
        alt("until wording", vec![active, passive])
    };
    let until = alt(
        "until",
        vec![until(UntilVerb::Reveal, "reveals", "reveal"), until(UntilVerb::Exile, "exiles", "exile")],
    );
    let prevent = v("prevents", "prevent").then_keep(&crate::statics::prevention(false)).map(
        |p| Some(Action::Prevent(p)),
        |a| match a {
            Action::Prevent(p) => Some(p.clone()),
            _ => None,
        },
    );
    // "enchanted land is a 6/4 creature"; "it's …" and "they're …" are clauses of their own (one
    // word, crate::effect)
    // "is" after a singular subject, "are" after a plural one ("Creatures you control are
    // Equipment in addition to their other types")
    let (is_word, is_number) = if form == Form::Third { ("is", Number::Sg) } else { ("are", Number::Pl) };
    let is = lit(is_word).then_keep(&crate::becomes::becoming(&[is_number])).map(
        |b| Some(Action::Is(b)),
        |a| match a {
            Action::Is(b) => Some(b.clone()),
            _ => None,
        },
    );
    let becoming = vec![becomes, is];
    alt(
        "action",
        vec![
            until,
            alt("becomes", becoming),
            set_stat,
            doesnt_untap,
            keep_mana,
            lose_unspent,
            prevent,
            cast,
            additional_lands,
            sacrifice,
            discard,
            exile,
            put_counters,
            zone_move,
            unattach,
            tap,
            untap,
            reveal,
            exert,
            pay,
            mill,
            remove_counters,
            lose_counters,
            waterbend,
            keyword_actions(form, &object, &from, &n()),
            draw,
            draw_up_to,
            create,
            deal_damage,
            damage_late,
            deal_divided,
            destroy,
            gain_life,
            lose_life,
            scry,
            surveil,
            counter,
            copy,
            tap_or_untap,
            vote,
            villainous,
            exchange_control,
            change_target,
            change_a_target,
            reselect,
            put_either,
            life_totals,
            life_with_stat,
            loses_choice,
            gains_choice,
            draw_additional,
            put_counters_of,
            shuffle_into,
            distribute,
            switch_pt,
            double_pt,
            double_life,
            pay_any,
            flip,
            emblem,
            separate,
            turn_face,
            remove_from_combat,
            move_counters,
            double_counters,
            crate::procedure::procedure(form),
            gets,
            gains,
            gains_of,
            cant,
            regenerate,
            look_at,
            meld,
            change_text,
            put_choice,
            put_that_kind,
            give_that_kind,
            choose_counter,
            or_remove,
            pay_times,
            pay_up_to,
            look_and_cast,
            play_among,
            gain_no_life,
            secretly,
            gets_either,
            control_player,
            put_same,
            put_number.alias(),
            choose,
            gain_control,
            get_counters,
            create_copy,
            create_copy_late,
            have,
            new_targets,
            search,
            crate::statics::static_act(form),
            v("adds", "add").then_keep(&crate::mana::mana_spec()).map(
                |m| Some(Action::AddMana(m)),
                |a| match a {
                    Action::AddMana(m) => Some(m.clone()),
                    _ => None,
                },
            ),
            v("adds an additional", "add an additional").then_keep(&crate::mana::mana_spec()).map(
                |m| Some(Action::AddAdditionalMana(m)),
                |a| match a {
                    Action::AddAdditionalMana(m) => Some(m.clone()),
                    _ => None,
                },
            ),
        ],
    )
}

/// Keyword actions (701) by shape: no argument, a number, objects, and the few with their own.
/// `from`: objects that may name the zone they are in ("a card from your hand"), for actions
/// done to a card wherever it is.
fn keyword_actions(form: Form, object: &P<Objects>, from: &P<Objects>, n: &P<Amount>) -> P<Action> {
    use KeywordAction as K;
    use NumberedAction as N;
    use ObjectAction as O;
    let s = selectors();
    let v = |third: &str, base: &str| verb(form, third, base, "");
    let mut bare = vec![
        (v("investigates", "investigate"), K::Investigate),
        (v("proliferates", "proliferate"), K::Proliferate),
        (v("populates", "populate"), K::Populate),
        (v("learns", "learn"), K::Learn),
        (v("recruits", "recruit"), K::Recruit),
        (v("ventures into the dungeon", "venture into the dungeon"), K::Venture),
        (v("time travels", "time travel"), K::TimeTravel),
        (v("manifests dread", "manifest dread"), K::ManifestDread),
        (v("forages", "forage"), K::Forage),
        (v("connives", "connive"), K::Connive),
        (v("explores", "explore"), K::Explore),
        (v("becomes the monarch", "become the monarch"), K::BecomeMonarch),
        (v("flips a coin", "flip a coin"), K::FlipCoin),
        (v("takes an extra turn", "take an extra turn"), K::ExtraTurn),
        (v("ends the turn", "end the turn"), K::EndTheTurn),
        (v("those votes are revealed", "those votes are revealed"), K::RevealVotes),
        (v("those choices are revealed", "those choices are revealed"), K::RevealChoices),
        (v("removes those counters", "remove those counters"), K::RemoveThoseCounters),
        (v("planeswalks", "planeswalk"), K::Planeswalk),
        (v("chaos ensues", "chaos ensues"), K::ChaosEnsues),
        // "if it has five or more hatchling counters on it, remove all of them": the same counters
        (v("removes all of them", "remove all of them"), K::RemoveThoseCounters),
        (v("flips a coin until they lose a flip", "flip a coin until you lose a flip"), K::FlipUntilLose),
        (v("does the same", "do the same"), K::DoTheSame),
        (v("takes the initiative", "take the initiative"), K::TakeInitiative),
        (v("phases out", "phase out"), K::PhaseOut),
        (v("wins the game", "win the game"), K::WinGame),
        (v("loses the game", "lose the game"), K::LoseGame),
    ];
    // a whole clause of its own, read where the imperative is
    if form == Form::Base {
        bare.push((lit("the Ring tempts you"), K::RingTemptsYou));
        // "it" is the game: day and night are its designations (731.1)
        bare.push((lit("it becomes day"), K::BecomeDay));
        bare.push((lit("it becomes night"), K::BecomeNight));
        // "your speed increases by 1" (702.179d): of the player whose ability it is
        bare.push((lit("your speed increases by 1"), K::IncreaseSpeed));
    }
    let bare = alt("keyword action", bare.into_iter().map(|(p, k)| p.to(k)).collect()).map(
        |k| Some(Action::Keyword(k)),
        |a| match a {
            Action::Keyword(k) => Some(*k),
            _ => None,
        },
    );
    let numbered = alt(
        "numbered keyword action",
        [
            (v("monstrosity", "monstrosity"), N::Monstrosity),
            (v("adapts", "adapt"), N::Adapt),
            (v("bolsters", "bolster"), N::Bolster),
            (v("supports", "support"), N::Support),
            (v("incubates", "incubate"), N::Incubate),
            (v("discovers", "discover"), N::Discover),
            (v("earthbends", "earthbend"), N::Earthbend),
            (v("endures", "endure"), N::Endure),
            (v("connives", "connive"), N::Connive),
            (v("fateseals", "fateseal"), N::Fateseal),
            (v("collects evidence", "collect evidence"), N::CollectEvidence),
            (v("blights", "blight"), N::Blight),
            (v("empowers Jace", "empower Jace"), N::EmpowerJace),
        ]
        .into_iter()
        .map(|(p, k)| p.to(k))
        .collect(),
    )
    .then(n)
    .map(
        |(k, n)| Some(Action::Numbered(k, n)),
        |a| match a {
            Action::Numbered(k, n) => Some((*k, n.clone())),
            _ => None,
        },
    );
    // each with what its lexicon entry says it is done to: a permanent, a card (manifest,
    // cloak), a permanent or spell (airbend); a card may be said with the zone it is in, after
    // "from", as for exile and reveal ("manifest a card from your hand", 701.40a, 701.58a)
    let on_objects = alt(
        "keyword action on objects",
        [
            (v("transforms", "transform"), O::Transform, Verb::Transform, object),
            (v("converts", "convert"), O::Convert, Verb::Convert, object),
            (v("goads", "goad"), O::Goad, Verb::Goad, object),
            (v("detains", "detain"), O::Detain, Verb::Detain, object),
            (v("suspects", "suspect"), O::Suspect, Verb::Suspect, object),
            (v("harnesses", "harness"), O::Harness, Verb::Harness, object),
            (v("airbends", "airbend"), O::Airbend, Verb::Airbend, object),
            (v("manifests", "manifest"), O::Manifest, Verb::Manifest, from),
            (v("cloaks", "cloak"), O::Cloak, Verb::Cloak, from),
            (v("beholds", "behold"), O::Behold, Verb::Behold, object),
        ]
        .into_iter()
        .map(|(p, k, verb, object)| {
            p.then_keep(&slot(object, verb, Role::What)).map(
                move |o| Some(Action::OnObjects(k, o)),
                move |a| match a {
                    Action::OnObjects(k2, o) if *k2 == k => Some(o.clone()),
                    _ => None,
                },
            )
        })
        .collect(),
    );
    let attach = v("attaches", "attach").then_keep(&slot(object, Verb::Attach, Role::What)).skip(&lit("to")).then(&slot_one(&s.selector, Verb::Attach, Role::To)).map(
        |(what, to)| Some(Action::Attach { what, to: Box::new(to) }),
        |a| match a {
            Action::Attach { what, to } => Some((what.clone(), (**to).clone())),
            _ => None,
        },
    );
    // "pair this creature with another unpaired creature you control" (702.95a)
    let pair_with = v("pairs", "pair").then_keep(&slot(object, Verb::Pair, Role::What)).skip(&lit("with")).then(&slot_one(&s.selector, Verb::Pair, Role::With)).map(
        |(what, with)| Some(Action::Pair { what, with: Box::new(with) }),
        |a| match a {
            Action::Pair { what, with } => Some((what.clone(), (**with).clone())),
            _ => None,
        },
    );
    let fight = v("fights", "fight").then_keep(&slot(object, Verb::Fight, Role::What)).map(
        |o| Some(Action::Fight(o)),
        |a| match a {
            Action::Fight(o) => Some(o.clone()),
            _ => None,
        },
    );
    let fight_each_other = lit("fight each other").to(Action::FightEachOther);
    let subtypes = crate::kernel::table(
        "subtypes",
        Subtype::all().map(|t| (crate::words::plural(t.name()), t)).collect(),
    );
    let amass = v("amasses", "amass").then_keep(&subtypes).then(n).map(
        |(subtype, amount)| Some(Action::Amass { subtype, amount }),
        |a| match a {
            Action::Amass { subtype, amount } => Some((*subtype, amount.clone())),
            _ => None,
        },
    );
    let clash = v("clashes with", "clash with").then_keep(&slot_one(&s.selector, Verb::Clash, Role::With)).map(
        |p| Some(Action::Clash(Box::new(p))),
        |a| match a {
            Action::Clash(p) => Some((**p).clone()),
            _ => None,
        },
    );
    // the kind of die, by its number of sides (706.1a): "d20", one word, or "N-sided" before
    // "die" or "dice" as the count agrees; the corpus says "dN" more often, so "N-sided" is its
    // alias
    let d_n = leaf(
        "die",
        |k| match k {
            Kind::Word(w) => w.strip_prefix('d').and_then(|n| n.parse::<u32>().ok()).filter(|n| *n > 0),
            _ => None,
        },
        |n| Some(Kind::Word(format!("d{n}"))),
    );
    let sided = crate::kernel::table(
        "N-sided",
        (1..=20u32)
            .filter_map(|n| crate::words::number_words().print_text(&n, false).map(|w| (format!("{w}-sided"), n)))
            .collect(),
    );
    let die = alt("die", vec![d_n.clone(), sided.skip(&lit("die")).alias()]);
    let dice_of = alt("dice", vec![d_n, sided.skip(&lit("dice")).alias()]);
    let a_die = lit("a").then_keep(&die).map(|sides| Some((Amount::Const(1), sides)), |(c, s)| {
        (*c == Amount::Const(1)).then_some(*s)
    });
    let dice = count_amount().then(&dice_of).map(
        |(c, s)| (c != Amount::Const(1)).then_some((c, s)),
        |(c, s)| (*c != Amount::Const(1)).then(|| (c.clone(), *s)),
    );
    // a modifier the instruction includes, added to or subtracted from the natural result
    // (706.2): one act with the roll, "roll a d20 and add the number of cards in your hand"; the
    // amount is a number, never mana ("roll a d20 and add {R}" is two acts)
    let by = crate::kernel::lazy("amount", || crate::amount::amounts().amount.clone());
    use ir::action::RollModifier as M;
    let modifier = lit("and").then_keep(&alt(
        "roll modifier",
        vec![
            v("adds", "add").then_keep(&by).map(
                |a| Some(M::Add(a)),
                |m| match m {
                    M::Add(a) => Some(a.clone()),
                    _ => None,
                },
            ),
            v("subtracts", "subtract").then_keep(&by).map(
                |a| Some(M::Subtract(a)),
                |m| match m {
                    M::Subtract(a) => Some(a.clone()),
                    _ => None,
                },
            ),
        ],
    ));
    // rolls the instruction ignores, as if they never happened (706.6): "the lower roll" of
    // exactly two dice, "all but the highest roll" of more than one; either keeps one roll, so
    // of two dice they say the same and "the lower roll" prints
    let extreme = |pair: bool, highest: &str, lowest: &str| {
        crate::kernel::table("highest or lowest", vec![(highest.to_string(), true), (lowest.to_string(), false)])
            .map(move |h| Some((pair, h)), move |(p, h)| (*p == pair).then_some(*h))
    };
    let ignored = v("ignores", "ignore").then_keep(&alt(
        "ignored rolls",
        vec![
            lit("the").then_keep(&extreme(true, "lower", "higher")).skip(&lit("roll")),
            lit("all but the").then_keep(&extreme(false, "highest", "lowest")).skip(&lit("roll")),
        ],
    ));
    let pair = |count: &Amount| *count == Amount::Const(2);
    let roll = v("rolls", "roll")
        .then_keep(&alt("dice", vec![a_die, dice]))
        .then(&modifier.opt())
        .then(&lit("and").then_keep(&ignored).opt())
        .map(
            move |(((count, sides), modifier), ignored)| {
                let kept = match ignored {
                    None => None,
                    Some((said_pair, highest)) => {
                        if (said_pair && !pair(&count)) || count == Amount::Const(1) {
                            return None;
                        }
                        Some(ir::action::KeptRolls { highest, count: Amount::Const(1) })
                    }
                };
                Some(Action::RollDie { count, sides, modifier, kept })
            },
            move |a| match a {
                Action::RollDie { count, sides, modifier, kept } => {
                    let ignored = match kept {
                        None => None,
                        Some(k) if k.count == Amount::Const(1) && *count != Amount::Const(1) => Some((pair(count), k.highest)),
                        Some(_) => return None,
                    };
                    Some((((count.clone(), *sides), modifier.clone()), ignored))
                }
                _ => None,
            },
        );
    let mut all = vec![bare, numbered, on_objects, attach, pair_with, fight, amass, clash, roll];
    if form == Form::Base {
        all.push(fight_each_other);
    }
    alt("keyword actions", all)
}


/// How many modes: "one or more", "both", "two", "any number" (700.2).
fn modes() -> P<ir::Modes> {
    use ir::Modes;
    alt(
        "modes",
        vec![
            lit("one or both").to(Modes::OneOrBoth),
            lit("one or more").to(Modes::OneOrMore),
            lit("both").to(Modes::Both),
            lit("any number").to(Modes::AnyNumber),
            crate::words::number_words().map(
                |n| Some(Modes::Exactly(n)),
                |m| match m {
                    Modes::Exactly(n) => Some(*n),
                    _ => None,
                },
            ),
        ],
    )
}
