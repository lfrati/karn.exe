//! Zone moves by effects: "return … to …", "put … into/onto/on top of …".
//!
//! ```text
//! move        := ("return" | "put") pair ("and" pair)*
//! pair        := objects destination(number of the objects, verb)
//! destination := "to"/"into" whose "hand" | "to"/"onto" "the battlefield" entry*
//!              | "into" whose "graveyard" | library placement (see `library_place`)
//! entry       := "tapped" | "attacking" | "tapped and attacking" | "under" whose "control"
//!              | "with" counters "on it/them" | "transformed" | "face down" | "attached to" selector
//! whose       := "your" | "its owner's" (one object) | "their owner's" (several) | "their owners'"
//!              | "their" | possessive player
//! ```
//!
//! The preposition follows from the verb and the zone, so the IR does not keep it.

use crate::event::{Form, ordinal, verb};
use crate::kernel::{P, alt, leaf, lit};
use crate::selector::{is_plural, selectors};
use crate::words::Number;
use ir::selector::Anaphor;
use ir::{
    Action, Destination, Domain, Entry, LibraryPlace, LibraryPosition, MoveVerb, Objects, Order, Position, Quant, Ref,
    Selector, Whose, Zone, ZoneSpec,
};
use oracle::token::Kind;

fn plural(o: &Objects) -> bool {
    o.items.len() > 1 || o.items.first().is_some_and(is_plural)
}

/// Whose zone; `noun` is the zone's noun in the singular and plural ("hand", "hands").
pub(crate) fn whose(n: Number, sg: &str, pl: &str) -> P<Whose> {
    let s = selectors();
    let clitic =
        leaf("'s", |k| matches!(k, Kind::Clitic(c) if c == "'s").then_some(()), |()| Some(Kind::Clitic("'s".into())));
    // "that player's"; "its owner's" is the moved object's owner ([`Whose::Owner`])
    let named = |p: &Selector| {
        let owner_of_it = matches!(&p.quant, Quant::Definite(Ref::OwnerOf(o))
            if matches!(o.quant, Quant::Definite(Ref::Anaphor(Anaphor::It | Anaphor::They))));
        p.set.domain == Domain::Player && *p != Selector::you() && !owner_of_it
    };
    let player = s.selector.skip(&clitic).map(
        move |p| named(&p).then(|| Whose::Player(Box::new(p))),
        |w| match w {
            Whose::Player(p) => Some((**p).clone()),
            _ => None,
        },
    );
    let owner = lit(if n == Number::Sg { "its owner's" } else { "their owner's" }).to(Whose::Owner);
    let mut v = vec![
        lit("your").skip(&lit(sg)).to(Whose::You),
        owner.skip(&lit(sg)),
        lit("their").skip(&lit(sg)).to(Whose::Their),
        player.skip(&lit(sg)),
    ];
    if n == Number::Pl {
        v.push(lit("their owners'").skip(&lit(pl)).to(Whose::Owners));
    }
    alt("whose", v)
}

fn entry(n: Number) -> P<Entry> {
    let s = selectors();
    let under = lit("under").then_keep(&whose(n, "control", "control")).map(
        |w| Some(Entry::Under(w)),
        |e| match e {
            Entry::Under(w) => Some(w.clone()),
            _ => None,
        },
    );
    let under_control_of = lit("under the control of").then_keep(&s.selector).map(
        |who| Some(Entry::UnderControlOf(Box::new(who))),
        |e| match e {
            Entry::UnderControlOf(who) => Some((**who).clone()),
            _ => None,
        },
    );
    let counters = lit("with")
        .then_keep(&crate::selector::counters_on())
        .map(
            |c| Some(Entry::WithCounters(c)),
            |e| match e {
                Entry::WithCounters(c) => Some(c.clone()),
                _ => None,
            },
        );
    let attached = lit("attached to").then_keep(&s.selector).map(
        |x| Some(Entry::AttachedTo(Box::new(x))),
        |e| match e {
            Entry::AttachedTo(x) => Some((**x).clone()),
            _ => None,
        },
    );
    alt(
        "entry",
        vec![
            lit("tapped and attacking").to(Entry::TappedAndAttacking),
            lit("tapped and transformed").to(Entry::TappedAndTransformed),
            lit("tapped").to(Entry::Tapped),
            lit("attacking the same player or planeswalker as").then_keep(&s.selector).map(
                |s| Some(Entry::AttackingSameAs(Box::new(s))),
                |e| match e { Entry::AttackingSameAs(s) => Some((**s).clone()), _ => None },
            ),
            lit("attacking").to(Entry::Attacking),
            under,
            under_control_of,
            counters,
            lit("transformed").to(Entry::Transformed),
            lit("face down").to(Entry::FaceDown),
            attached,
        ],
    )
}

pub(crate) fn destination(n: Number, verb: MoveVerb) -> P<Destination> {
    let return_ = verb == MoveVerb::Return;
    let hand = lit(if return_ { "to" } else { "into" }).then_keep(&whose(n, "hand", "hands")).map(
        |w| Some(Destination::Hand(w)),
        |d| match d {
            Destination::Hand(w) => Some(w.clone()),
            _ => None,
        },
    );
    let battlefield =
        lit(if return_ { "to the battlefield" } else { "onto the battlefield" }).then_keep(&entry(n).many()).map(
            |e| Some(Destination::Battlefield(e)),
            |d| match d {
                Destination::Battlefield(e) => Some(e.clone()),
                _ => None,
            },
        );
    let command = lit(if return_ { "to the command zone" } else { "into the command zone" }).to(Destination::CommandZone);
    let graveyard = lit(if return_ { "to" } else { "into" }).then_keep(&whose(n, "graveyard", "graveyards")).map(
        |w| Some(Destination::Graveyard(w)),
        |d| match d { Destination::Graveyard(w) => Some(w.clone()), _ => None },
    );
    let mut v = vec![hand, battlefield, command, graveyard];
    if !return_ {
        v.push(library_place(n).map(
            |p| Some(Destination::Library(p)),
            |d| match d { Destination::Library(p) => Some(p.clone()), _ => None },
        ));
    }
    alt("destination", v)
}

/// Where cards come from, said after the move: "from the command zone", "from your graveyard",
/// "from your hand or graveyard", "from exile". The objects it applies to name no zone of
/// their own ([`origin_fits`]).
pub(crate) fn origin() -> P<ZoneSpec> {
    let owned = lit("from").then_keep(&selectors().possessive).then(&crate::selector::zones()).map(
        |(owner, zones)| Some(ZoneSpec { zones, owner: Some(Box::new(owner)), slice: None }),
        |z: &ZoneSpec| match &z.owner {
            Some(owner) if z.slice.is_none() => Some(((**owner).clone(), z.zones.clone())),
            _ => None,
        },
    );
    let shared = |text: &str, zone: Zone| lit(text).to(ZoneSpec::of(zone, None));
    alt("origin", vec![owned, shared("from the command zone", Zone::Command), shared("from exile", Zone::Exile)])
}

/// A trailing origin is read only for one move whose objects name no zone themselves, so a zone
/// has one place in the text.
pub(crate) fn origin_fits<'a>(objects: impl IntoIterator<Item = &'a Objects>, moves: usize, from: &Option<ZoneSpec>) -> bool {
    from.is_none()
        || (moves == 1 && objects.into_iter().all(|o| o.items.iter().all(|s| s.set.zone.is_none())))
}

/// Where in a library, and in what order (CR 401.4, 401.7).
///
/// ```text
/// place    := ordered [order] | unordered
/// ordered  := ("on top of" | "on the bottom of") library | "on top" | "on the bottom"
///           | "back on top of" library | "back"
/// unordered:= "into" library ORDINAL "from the top" | ORDINAL "from the top"
///           | "on" ["your" | "their" "choice of"] "the top or bottom of" library
///           | "into" library ORDINAL "from the top or on the bottom"
/// order    := "in any order" | "in a random order"
/// ```
///
/// A library the text does not name is the one it last spoke of; "back" puts cards into the
/// library they were looked at in.
fn library_place(n: Number) -> P<LibraryPlace> {
    use LibraryPosition::{Bottom, FromTop, Top};
    let library = whose(n, "library", "libraries");
    let order = alt("order", vec![lit("in any order").to(Order::Any), lit("in a random order").to(Order::Random)]);
    let end = alt("end", vec![lit("on top of").to(Top), lit("on the bottom of").to(Bottom)]);
    // (library, back, position)
    type Ordered = (Option<Whose>, bool, LibraryPosition);
    let named = end.then(&library).map(
        |(p, w)| Some((Some(w), false, p)),
        |o: &Ordered| match o {
            (Some(w), false, p @ (Top | Bottom)) => Some((*p, w.clone())),
            _ => None,
        },
    );
    let bare = alt("end", vec![lit("on top").to(Top), lit("on the bottom").to(Bottom)]).map(
        |p| Some((None, false, p)),
        |o: &Ordered| match o {
            (None, false, p @ (Top | Bottom)) => Some(*p),
            _ => None,
        },
    );
    let back_named = lit("back on top of").then_keep(&library).map(
        |w| Some((Some(w), true, Top)),
        |o: &Ordered| match o {
            (Some(w), true, Top) => Some(w.clone()),
            _ => None,
        },
    );
    let back = lit("back").to((None, true, Top));
    let ordered = alt("placement", vec![named, bare, back_named, back]).then(&order.opt()).map(
        |((library, back, p), order)| Some(LibraryPlace { library, back, position: Position::One(p), order }),
        |p: &LibraryPlace| match p {
            LibraryPlace { library, back, position: Position::One(q), order } => Some(((library.clone(), *back, *q), *order)),
            _ => None,
        },
    );
    let unordered = |position: P<(Option<Whose>, Position)>| {
        position.map(
            |(library, position)| Some(LibraryPlace { library, back: false, position, order: None }),
            |p: &LibraryPlace| (!p.back && p.order.is_none()).then(|| (p.library.clone(), p.position.clone())),
        )
    };
    let nth = lit("into").then_keep(&library).then(&ordinal()).skip(&lit("from the top")).map(
        |(w, k)| Some((Some(w), Position::One(FromTop(k)))),
        |(w, p): &(Option<Whose>, Position)| match (w, p) {
            (Some(w), Position::One(FromTop(k))) => Some((w.clone(), *k)),
            _ => None,
        },
    );
    let nth_bare = ordinal().skip(&lit("from the top")).map(
        |k| Some((None, Position::One(FromTop(k)))),
        |(w, p): &(Option<Whose>, Position)| match (w, p) {
            (None, Position::One(FromTop(k))) => Some(*k),
            _ => None,
        },
    );
    let chooser = alt("chooser", vec![lit("your").to(Whose::You), lit("their").to(Whose::Their)]);
    let top_or_bottom = |lead: P<Option<Whose>>| {
        lead.skip(&lit("the top or bottom of")).then(&library).map(
            |(chooser, w)| Some((Some(w), Position::Choice { chooser, between: [Top, Bottom] })),
            |(w, p): &(Option<Whose>, Position)| match (w, p) {
                (Some(w), Position::Choice { chooser, between: [Top, Bottom] }) => Some((chooser.clone(), w.clone())),
                _ => None,
            },
        )
    };
    let chosen = top_or_bottom(lit("on").then_keep(&chooser).skip(&lit("choice of")).map(|w| Some(Some(w)), |c: &Option<Whose>| c.clone()));
    let unchosen = top_or_bottom(lit("on").to(None::<Whose>));
    let nth_or_bottom = lit("into").then_keep(&library).then(&ordinal()).skip(&lit("from the top or on the bottom")).map(
        |(w, k)| Some((Some(w), Position::Choice { chooser: None, between: [FromTop(k), Bottom] })),
        |(w, p): &(Option<Whose>, Position)| match (w, p) {
            (Some(w), Position::Choice { chooser: None, between: [FromTop(k), Bottom] }) => Some((w.clone(), *k)),
            _ => None,
        },
    );
    // "into its owner's library just beneath the top X cards of that library"
    let beneath = lit("into").then_keep(&library).skip(&lit("just beneath the top X cards of that library")).map(
        |w| Some((Some(w), Position::BeneathTopX)),
        |(w, p): &(Option<Whose>, Position)| match (w, p) {
            (Some(w), Position::BeneathTopX) => Some(w.clone()),
            _ => None,
        },
    );
    let unordered = unordered(alt("placement", vec![nth, nth_bare, chosen, unchosen, nth_or_bottom, beneath]));
    alt("library placement", vec![ordered, unordered])
}

/// "return target creature to its owner's hand", "put it onto the battlefield tapped".
pub fn zone_move(form: Form, objects: &P<Objects>) -> P<Action> {
    let pairs = |mv: MoveVerb| {
        let sg = objects.map(|o| (!plural(&o)).then_some(o), |o| (!plural(o)).then(|| o.clone()));
        let pl = objects.map(|o| plural(&o).then_some(o), |o| plural(o).then(|| o.clone()));
        let pair = alt("move", vec![sg.then(&destination(Number::Sg, mv)), pl.then(&destination(Number::Pl, mv))]);
        pair.then(&lit("and").then_keep(&pair).many()).map(
            |(first, mut rest)| {
                rest.insert(0, first);
                Some(rest)
            },
            |v: &Vec<(Objects, Destination)>| v.split_first().map(|(f, r)| (f.clone(), r.to_vec())),
        )
    };
    let one = |mv: MoveVerb, third: &str, base: &str| {
        verb(form, third, base, "").then_keep(&pairs(mv)).then(&origin().opt()).map(
            move |(moves, from)| {
                origin_fits(moves.iter().map(|(o, _)| o), moves.len(), &from).then_some(Action::Move { verb: mv, moves, from })
            },
            move |a| match a {
                Action::Move { verb, moves, from } if *verb == mv && origin_fits(moves.iter().map(|(o, _)| o), moves.len(), from) => {
                    Some((moves.clone(), from.clone()))
                }
                _ => None,
            },
        )
    };
    // "return to your hand target artifact card in your graveyard": the destination said before
    // a long object. The same move; it prints with the object first.
    let shifted = |mv: MoveVerb, third: &str, base: &str| {
        let sg = objects.map(|o| (!plural(&o)).then_some(o), |o| (!plural(o)).then(|| o.clone()));
        let pl = objects.map(|o| plural(&o).then_some(o), |o| plural(o).then(|| o.clone()));
        let pair = alt("shifted move", vec![destination(Number::Sg, mv).then(&sg), destination(Number::Pl, mv).then(&pl)]);
        verb(form, third, base, "")
            .then_keep(&pair)
            .map(
                move |(to, objects)| origin_fits([&objects], 1, &None).then(|| Action::Move { verb: mv, moves: vec![(objects, to)], from: None }),
                |_: &Action| None,
            )
            .alias()
    };
    alt(
        "zone move",
        vec![
            one(MoveVerb::Return, "returns", "return"),
            one(MoveVerb::Put, "puts", "put"),
            shifted(MoveVerb::Return, "returns", "return"),
            shifted(MoveVerb::Put, "puts", "put"),
        ],
    )
}
