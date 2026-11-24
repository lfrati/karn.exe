//! Keyword abilities with their parameters (CR 702).
//!
//! ```text
//! keyword := name [N | mana | mana "and/or" mana | "—" cost | N "—" mana]
//!          | "enchant" set | "protection" ("from" quality)+ | "affinity for" set
//!          | TYPE "walk" | TYPE "cycling" mana | "basic landcycling" mana | "splice onto" TYPE mana
//! ```
//!
//! The shape of the parameter is read, not looked up per keyword; the keywords whose parameter
//! is a set or a quality take it after their own preposition.

use crate::cost::costs;
use crate::kernel::{P, alt, lazy, leaf, lit, table};
use crate::mana::mana_cost;
use crate::selector::{keyword_name, selectors};
use crate::words::digits;
use ir::action::Conj;
use ir::{Amount, Atom, CardType, Color, Filter, GiftOf, Keyword, KeywordInstance, KeywordParam, Quality, Subtype};
use oracle::token::Kind;
use std::sync::OnceLock;

pub fn keyword() -> P<KeywordInstance> {
    static K: OnceLock<P<KeywordInstance>> = OnceLock::new();
    K.get_or_init(build).clone()
}

fn is_mana_only(c: &ir::Cost) -> bool {
    matches!(c.0.as_slice(), [ir::CostPart::Mana(_)])
}

/// A number written out ("2", "X") or defined after it (", where X is the number of ...").
fn is_plain(a: &Amount) -> bool {
    matches!(a, Amount::Const(_) | Amount::X)
}

/// Does the keyword end in a sentence of its own, so that its row ends with a period? A cost
/// written after a dash ("Ward—Pay 3 life.", "Splice onto Arcane—Exile four cards from your
/// graveyard.") and an X defined after it ("Firebending X, where X is ~'s power.") are sentences;
/// a mana cost or a number is not.
pub fn ends_with_sentence(k: &KeywordInstance) -> bool {
    match &k.param {
        KeywordParam::Cost(_) => true,
        KeywordParam::Splice(_, c) => !is_mana_only(c),
        KeywordParam::Number(a) => !is_plain(a),
        _ => false,
    }
}

/// A printed keyword row whose last keyword has an X that the row defines: "Firebending X, where
/// X is ~'s power." (107.3). The X is the keyword's number, so the amount takes its place; a
/// keyword granted by an effect says it with the effect's own ", where X is" (a modifier).
pub fn defined_x(row: &P<Vec<KeywordInstance>>) -> P<Vec<KeywordInstance>> {
    row.skip(&lit(", where X is")).then(&crate::amount::amounts().amount).map(
        |(mut v, a)| {
            let last = v.last_mut()?;
            (last.param == KeywordParam::Number(Amount::X) && !is_plain(&a)).then(|| last.param = KeywordParam::Number(a))?;
            Some(v)
        },
        |v: &Vec<KeywordInstance>| {
            let mut w = v.clone();
            let last = w.last_mut()?;
            let KeywordParam::Number(a) = std::mem::replace(&mut last.param, KeywordParam::Number(Amount::X)) else { return None };
            (!is_plain(&a)).then_some((w, a))
        },
    )
}

fn named(name: &'static str) -> Keyword {
    Keyword::from_name(name).unwrap_or_else(|| panic!("CR 702 has {name}"))
}

fn n() -> P<Amount> {
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
}

/// A keyword written with its land or creature type as one word: "swampwalk", "plainscycling".
fn typed_word(suffix: &'static str) -> P<Subtype> {
    leaf(
        suffix,
        move |k| match k {
            Kind::Word(w) => {
                let lower = w.to_lowercase();
                let prefix = lower.strip_suffix(suffix)?;
                Subtype::from_name(&crate::kernel::capitalize(prefix))
            }
            _ => None,
        },
        move |s: &Subtype| Some(Kind::Word(format!("{}{suffix}", s.name().to_lowercase()))),
    )
}

/// A word made of a given word's text, as the tokenizer keeps it: "landwalk".
fn one_word(text: String) -> P<()> {
    let t = text.clone();
    leaf(
        "word",
        move |k| matches!(k, Kind::Word(w) if w.eq_ignore_ascii_case(&text)).then_some(()),
        move |()| Some(Kind::Word(t.clone())),
    )
}

/// What may stand before the type a keyword is named for: a supertype or its negation ("snow",
/// "legendary", "nonbasic") or a card type ("artifact").
fn qualifiers() -> P<Filter> {
    let supertype = crate::words::adjective().map(
        |f| supertype_or_negation(&f).then_some(f),
        |f| supertype_or_negation(f).then(|| f.clone()),
    );
    let card_type = crate::words::type_word(crate::words::Number::Sg, false).map(
        |a| matches!(a, Atom::Type(_)).then_some(Filter::Atom(a)),
        |f| match f {
            Filter::Atom(a @ Atom::Type(_)) => Some(a.clone()),
            _ => None,
        },
    );
    alt("keyword qualifier", vec![supertype, card_type])
}

/// A supertype ("snow") or the negation of a supertype or card type ("nonbasic", "nonland"), not
/// a color or a status.
fn supertype_or_negation(f: &Filter) -> bool {
    match f {
        Filter::Atom(Atom::Supertype(_)) => true,
        Filter::Not(inner) => matches!(**inner, Filter::Atom(Atom::Supertype(_) | Atom::Type(_))),
        _ => false,
    }
}

/// The type itself: a card type or a subtype ("artifact", "Goblin", "Food").
fn kind_word() -> P<Filter> {
    crate::words::type_word(crate::words::Number::Sg, false).map(|a| Some(Filter::Atom(a)), |f| match f {
        Filter::Atom(a @ (Atom::Type(_) | Atom::Subtype(_))) => Some(a.clone()),
        _ => None,
    })
}

/// "[type]walk", "[type]cycling" (702.14a, 702.29e): the type is any combination of supertypes,
/// card types and a subtype, which is joined to the keyword's word ("snow forestwalk",
/// "swampwalk"), or the card type land ("legendary landwalk", "basic landcycling", "artifact
/// landcycling"): one supertype or card type at least before the land word.
fn type_named(suffix: &'static str) -> P<Filter> {
    let by_subtype = typed_word(suffix).map(
        |t| Some(Filter::Atom(Atom::Subtype(t))),
        |f| match f {
            Filter::Atom(Atom::Subtype(t)) => Some(*t),
            _ => None,
        },
    );
    let by_land = qualifiers().many().skip(&one_word(format!("land{suffix}"))).map(
        |mut qs| {
            if qs.is_empty() {
                return None; // a bare "landwalk" names no type
            }
            qs.push(Filter::Atom(Atom::Type(CardType::Land)));
            Some(Filter::All(qs))
        },
        |f: &Filter| match f {
            Filter::All(v) => match v.split_last() {
                Some((last, qs)) if *last == Filter::Atom(Atom::Type(CardType::Land)) && !qs.is_empty() => Some(qs.to_vec()),
                _ => None,
            },
            _ => None,
        },
    );
    let with_subtype = qualifiers().many().then(&by_subtype).map(
        |(mut qs, f)| {
            qs.push(f);
            Some(Filter::All(qs))
        },
        |f: &Filter| match f {
            Filter::All(v) => v.split_last().map(|(last, qs)| (qs.to_vec(), last.clone())),
            _ => None,
        },
    );
    alt("type named", vec![with_subtype, by_land])
}

/// Does the noun phrase name players ("that player", "your opponents", "each of your
/// opponents"), whatever else it says?
fn names_players(x: &ir::Selector) -> bool {
    use ir::selector::Domain;
    x.set.domain == Domain::Player
        || matches!(&x.set.filter, Filter::All(v) if matches!(v.as_slice(), [Filter::Atom(Atom::Among(inner))] if names_players(inner)))
}

/// "the chosen player": a reference to a choice, which has a value of its own.
fn is_chosen_player(x: &ir::Selector) -> bool {
    matches!(x.quant, ir::selector::Quant::Definite(ir::selector::Ref::Anaphor(ir::selector::Anaphor::TheParticiple { .. })))
}

fn quality() -> P<Quality> {
    let s = selectors();
    let color = table("color", Color::ALL.iter().map(|c| (c.word().to_string(), Quality::Color(*c))).collect());
    // a player: "that player", "each of your opponents" ("the chosen player" is its own value)
    let player = s.selector.map(
        |x| (names_players(&x) && !is_chosen_player(&x)).then(|| Quality::Player(Box::new(x))),
        |q| match q {
            Quality::Player(x) => Some((**x).clone()),
            _ => None,
        },
    );
    // a supertype alone: "snow"
    let supertype = table(
        "supertype",
        ir::Supertype::ALL.iter().map(|t| (t.word().to_string(), Quality::Filter(Filter::Atom(Atom::Supertype(*t))))).collect(),
    );
    alt(
        "quality",
        vec![
            color,
            lit("multicolored").to(Quality::Multicolored),
            lit("monocolored").to(Quality::Monocolored),
            // "protection from colorless or from the color of your choice": from colorless
            // sources, a characteristic as a color is (105.2c, 702.16a)
            lit("colorless").to(Quality::Filter(Filter::Atom(Atom::Colorless))),
            lit("everything").to(Quality::Everything),
            lit("the chosen color").to(Quality::ChosenColor),
            lit("the chosen player").to(Quality::ChosenPlayer),
            // "protection from the chosen card name" (Runed Halo), "… the chosen card type"
            // (Serra's Emissary): from what has that name or is of that type (702.16a)
            lit("the chosen card name").to(Quality::Filter(Filter::Atom(Atom::Reference(ir::reference::UnresolvedRef::choice(ir::reference::Kind::CardName))))),
            lit("the chosen card type").to(Quality::Filter(Filter::Atom(Atom::Reference(ir::reference::UnresolvedRef::choice(ir::reference::Kind::Type))))),
            lit("the color of your choice").to(Quality::ColorOfChoice),
            // "Choose a color. … gains hexproof from that color"
            lit("that color").to(Quality::ChosenColor).alias(),
            lit("the color of its controller's choice").to(Quality::ColorOfControllersChoice),
            lit("each of its colors").to(Quality::EachOfItsColors),
            lit("each color among").then_keep(&s.set_pl).map(
                |x| Some(Quality::EachColorAmong(x)),
                |q| match q {
                    Quality::EachColorAmong(x) => Some(x.clone()),
                    _ => None,
                },
            ),
            player,
            supertype,
            // "protection from mana value 3 or less"
            lit("mana value")
                .then_keep(&crate::words::stat_amount())
                .then(&alt("comparison", vec![lit("or less").to(ir::Cmp::Le), lit("or greater").to(ir::Cmp::Ge)]))
                .map(
                    |(value, cmp)| Some(Quality::Filter(Filter::Atom(Atom::Stat { stat: ir::Stat::ManaValue, cmp, value }))),
                    |q| match q {
                        Quality::Filter(Filter::Atom(Atom::Stat { stat: ir::Stat::ManaValue, cmp, value }))
                            if matches!(cmp, ir::Cmp::Le | ir::Cmp::Ge) =>
                        {
                            Some((value.clone(), *cmp))
                        }
                        _ => None,
                    },
                ),
            lit("each color").to(Quality::EachColor),
            s.set_pl.map(
                |x| Some(Quality::Set(x)),
                |q| match q {
                    Quality::Set(x) => Some(x.clone()),
                    _ => None,
                },
            ),
        ],
    )
}

fn build() -> P<KeywordInstance> {
    let s = selectors();
    let dash = || lit("—").tight();
    let param = |p: P<KeywordParam>| p;
    let generic = keyword_name().then(&alt(
        "keyword parameter",
        vec![
            crate::kernel::empty().to(KeywordParam::None),
            param(n().map(
                |a| Some(KeywordParam::Number(a)),
                |p| match p {
                    KeywordParam::Number(a) => Some(a.clone()),
                    _ => None,
                },
            )),
            mana_cost().map(
                |m| Some(KeywordParam::Mana(m)),
                |p| match p {
                    KeywordParam::Mana(m) => Some(m.clone()),
                    _ => None,
                },
            ),
            // alternatives: "{U} or {B}" is a choice, "{1}{U} and/or {B}" one or both (702.33b)
            crate::action::coord(&mana_cost()).map(
                |c| (c.items.len() >= 2 && c.conj != Conj::And).then_some(KeywordParam::ManaList(c)),
                |p| match p {
                    KeywordParam::ManaList(c) if c.items.len() >= 2 && c.conj != Conj::And => Some(c.clone()),
                    _ => None,
                },
            ),
            dash().then_keep(&lazy("cost", || costs().cost.clone()).tight()).map(
                |c| Some(KeywordParam::Cost(c)),
                |p| match p {
                    KeywordParam::Cost(c) => Some(c.clone()),
                    _ => None,
                },
            ),
            n().skip(&dash()).then(&mana_cost().tight()).map(
                |(a, m)| Some(KeywordParam::NumberAndMana(a, m)),
                |p| match p {
                    KeywordParam::NumberAndMana(a, m) => Some((a.clone(), m.clone())),
                    _ => None,
                },
            ),
        ],
    ));
    // keywords whose parameter has its own shape are read only below
    let special = [
        named("Enchant"),
        named("Protection"),
        named("Affinity"),
        named("Splice"),
        named("Gift"),
        named("Prototype"),
        named("More Than Meets the Eye"),
        named("Craft"),
        named("Champion"),
    ];
    let generic = generic.map(
        move |(keyword, param)| (!special.contains(&keyword)).then_some(KeywordInstance { keyword, param }),
        move |k| (!special.contains(&k.keyword)).then(|| (k.keyword, k.param.clone())),
    );
    let with = |keyword: Keyword, p: P<KeywordParam>| {
        p.map(
            move |param| Some(KeywordInstance { keyword, param }),
            move |k: &KeywordInstance| (k.keyword == keyword).then(|| k.param.clone()),
        )
    };
    let enchant = lit("enchant").then_keep(&s.set_sg).map(
        |x| Some(KeywordParam::Enchant(x)),
        |p| match p {
            KeywordParam::Enchant(x) => Some(x.clone()),
            _ => None,
        },
    );
    // "from black and from red", "from blue, from black, and from red", "from artifacts or from
    // the color of your choice": each quality has its own "from"
    let from = crate::action::coord(&lit("from").then_keep(&quality()));
    let protection = lit("protection").then_keep(&from).map(
        |q| Some(KeywordParam::Protection(q)),
        |p| match p {
            KeywordParam::Protection(v) => Some(v.clone()),
            _ => None,
        },
    );
    let affinity = lit("affinity for").then_keep(&s.set_pl).map(
        |x| Some(KeywordParam::Affinity(x)),
        |p| match p {
            KeywordParam::Affinity(x) => Some(x.clone()),
            _ => None,
        },
    );
    // a cost is written as it is, or after a dash when it is more than mana
    let any_cost = {
        let dashed = dash().then_keep(&lazy("cost", || costs().cost.clone()).tight()).map(
            |c: ir::Cost| (!is_mana_only(&c)).then_some(c),
            |c| (!is_mana_only(c)).then(|| c.clone()),
        );
        let mana = mana_cost().map(
            |m| Some(ir::Cost(vec![ir::CostPart::Mana(m)])),
            |c| match c.0.as_slice() {
                [ir::CostPart::Mana(m)] => Some(m.clone()),
                _ => None,
            },
        );
        alt("keyword cost", vec![mana, dashed])
    };
    let splice = lit("splice onto").then_keep(&s.set_sg).then(&any_cost).map(
        |(t, m)| Some(KeywordParam::Splice(t, m)),
        |p| match p {
            KeywordParam::Splice(t, m) => Some((t.clone(), m.clone())),
            _ => None,
        },
    );
    let hexproof_from = lit("hexproof").then_keep(&from).map(
        |q| Some(KeywordParam::Protection(q)),
        |p| match p {
            KeywordParam::Protection(v) => Some(v.clone()),
            _ => None,
        },
    );
    // "gift a card", "gift a tapped Fish", "gift an extra turn"
    let token_name = leaf(
        "token name",
        |k| match k {
            Kind::Word(w) if w.chars().next().is_some_and(char::is_uppercase) => Some(w.clone()),
            _ => None,
        },
        |w: &String| Some(Kind::Word(w.clone())),
    );
    let a = || alt("a", vec![lit("a"), lit("an").alias()]);
    let gift_of = alt(
        "gift",
        vec![
            lit("a card").to(GiftOf::Card),
            lit("an extra turn").to(GiftOf::ExtraTurn),
            a().then_keep(&lit("tapped").opt()).then(&token_name).map(
                |(t, name)| Some(GiftOf::Token { tapped: t.is_some(), name }),
                |g| match g {
                    GiftOf::Token { tapped, name } => Some((tapped.then_some(()), name.clone())),
                    _ => None,
                },
            ),
        ],
    );
    let gift = lit("gift").then_keep(&gift_of).map(
        |g| Some(KeywordParam::Gift(g)),
        |p| match p {
            KeywordParam::Gift(g) => Some(g.clone()),
            _ => None,
        },
    );
    let prototype = lit("prototype").then_keep(&mana_cost()).skip(&lit("—")).then(&crate::token::pt()).map(
        |(m, (p, t))| Some(KeywordParam::Prototype(m, p, t)),
        |k| match k {
            KeywordParam::Prototype(m, p, t) => Some((m.clone(), (p.clone(), t.clone()))),
            _ => None,
        },
    );
    // the one keyword Oracle capitalizes like a title
    let mtmte = lit("More Than Meets the Eye").then_keep(&mana_cost()).map(
        |m| Some(KeywordParam::Mana(m)),
        |p| match p {
            KeywordParam::Mana(m) => Some(m.clone()),
            _ => None,
        },
    );
    // "partner—Friends forever", "partner with Ley Weaver"
    let text_word = leaf(
        "word",
        |k| match k {
            Kind::Word(w) => Some(w.clone()),
            Kind::Punct('&') => Some("&".to_string()),
            _ => None,
        },
        |w: &String| Some(if w == "&" { Kind::Punct('&') } else { Kind::Word(w.clone()) }),
    );
    let words = text_word.then(&text_word.many()).map(
        |(f, rest)| Some(std::iter::once(f).chain(rest).collect::<Vec<_>>().join(" ")),
        |s: &String| {
            let mut v = s.split(' ').map(str::to_string);
            let first = v.next()?;
            Some((first, v.collect()))
        },
    );
    let partner_variant = lit("partner").skip(&dash()).then_keep(&words.clone().tight()).map(
        |w| Some(KeywordParam::PartnerVariant(w)),
        |p| match p {
            KeywordParam::PartnerVariant(w) => Some(w.clone()),
            _ => None,
        },
    );
    let partner_with = lit("partner with").then_keep(&crate::kernel::name_run()).map(
        |w| Some(KeywordParam::PartnerWith(w)),
        |p| match p {
            KeywordParam::PartnerWith(w) => Some(w.clone()),
            _ => None,
        },
    );
    let craft_one = lit("craft with").then_keep(&s.set_sg).then(&mana_cost()).map(
        |(x, m)| Some(KeywordParam::Craft(x, m)),
        |p| match p {
            KeywordParam::Craft(x, m) => Some((x.clone(), m.clone())),
            _ => None,
        },
    );
    let craft_many = lit("craft with one or more").then_keep(&s.set_pl.opt()).then(&mana_cost()).map(
        |(x, m)| Some(KeywordParam::CraftMany(x, m)),
        |p| match p {
            KeywordParam::CraftMany(x, m) => Some((x.clone(), m.clone())),
            _ => None,
        },
    );
    let craft = alt("craft", vec![craft_many, craft_one]);
    let equip_quality = lit("equip").then_keep(&s.set_sg).then(&mana_cost()).map(
        |(x, m)| Some(KeywordParam::EquipQuality(x, m)),
        |p| match p {
            KeywordParam::EquipQuality(x, m) => Some((x.clone(), m.clone())),
            _ => None,
        },
    );
    // "bands with other legendary creatures" (702.22b)
    let bands_with = lit("bands with other").then_keep(&s.set_pl).map(
        |x| Some(KeywordParam::BandsWithOther(x)),
        |p| match p {
            KeywordParam::BandsWithOther(x) => Some(x.clone()),
            _ => None,
        },
    );
    let champion = lit("champion").then_keep(&a()).then_keep(&s.set_sg).map(
        |x| Some(KeywordParam::Champion(x)),
        |p| match p {
            KeywordParam::Champion(x) => Some(x.clone()),
            _ => None,
        },
    );
    // "landwalk of the chosen type", "snow landwalk of the chosen type"
    let chosen_land = || (Filter::Atom(Atom::Chosen(ir::selector::Chosen::Type)), Filter::Atom(Atom::Type(CardType::Land)));
    let chosen_walk = qualifiers().many().skip(&one_word("landwalk".to_string())).skip(&lit("of the chosen type")).map(
        move |mut qs| {
            let (chosen, land) = chosen_land();
            qs.push(chosen);
            qs.push(land);
            Some(Filter::All(qs))
        },
        move |f: &Filter| match f {
            Filter::All(v) if v.len() >= 2 && (v[v.len() - 2].clone(), v[v.len() - 1].clone()) == chosen_land() => Some(v[..v.len() - 2].to_vec()),
            _ => None,
        },
    );
    let landwalk = alt("landwalk", vec![chosen_walk, type_named("walk")]).map(
        |f| Some(KeywordParam::Landwalk(f)),
        |p| match p {
            KeywordParam::Landwalk(f) => Some(f.clone()),
            _ => None,
        },
    );
    let typecycling = type_named("cycling").then(&mana_cost()).map(
        |(f, m)| Some(KeywordParam::Typecycling(f, m)),
        |p| match p {
            KeywordParam::Typecycling(f, m) => Some((f.clone(), m.clone())),
            _ => None,
        },
    );
    // "Goblin offering", "artifact offering" (702.48a): the quality of the permanent sacrificed
    let offering = qualifiers().many().then(&kind_word()).skip(&lit("offering")).map(
        |(mut qs, k)| {
            qs.push(k);
            Some(KeywordParam::Offering(Filter::All(qs)))
        },
        |p| match p {
            KeywordParam::Offering(Filter::All(v)) => v.split_last().map(|(k, qs)| (qs.to_vec(), k.clone())),
            _ => None,
        },
    );
    // "Devour artifact 1", "Devour Food 3" (702.82c)
    let devour_typed = lit("devour").then_keep(&kind_word()).then(&n()).map(
        |(k, a)| Some(KeywordParam::DevourQuality(Filter::All(vec![k]), a)),
        |p| match p {
            KeywordParam::DevourQuality(Filter::All(v), a) => match v.as_slice() {
                [k] => Some((k.clone(), a.clone())),
                _ => None,
            },
            _ => None,
        },
    );
    alt(
        "keyword",
        vec![
            generic,
            with(named("Enchant"), enchant),
            with(named("Protection"), protection),
            with(named("Affinity"), affinity),
            with(named("Splice"), splice),
            with(named("Hexproof"), hexproof_from),
            with(named("Gift"), gift),
            with(named("Prototype"), prototype),
            with(named("More Than Meets the Eye"), mtmte),
            with(named("Partner"), partner_variant),
            with(named("Partner"), partner_with),
            with(named("Partner"), lit("choose a Background").to(KeywordParam::ChooseBackground)),
            with(named("Partner"), lit("Doctor's companion").to(KeywordParam::DoctorsCompanion)),
            with(named("Craft"), craft),
            with(named("Equip"), equip_quality),
            with(named("Champion"), champion),
            with(named("Banding"), bands_with),
            with(named("Landwalk"), landwalk),
            with(named("Cycling"), typecycling),
            with(named("Offering"), offering),
            with(named("Devour"), devour_typed),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::Outcome;
    use oracle::token::tokenize;

    /// Does the whole ability read, and print back as the text it was read from?
    fn reads(text: &str) -> bool {
        let a = &crate::ability::abilities().ability;
        match a.parse_all(&tokenize(text)) {
            Outcome::Accepted { value, .. } => a.print_text(&value, true).as_deref() == Some(text),
            _ => false,
        }
    }

    /// CR 701.4a: to behold is to reveal a card from the hand or choose a permanent, so it is an
    /// action of a player on cards and permanents: instruction, cost, event.
    #[test]
    fn r701_4a_behold() {
        for t in [
            "As an additional cost to cast ~, behold a Dragon or pay {1}.",
            "As an additional cost to cast ~, behold a Kithkin and exile it.",
            "As an additional cost to cast ~, you may behold a Dragon.",
            "As an additional cost to cast ~, you may choose a creature type and behold two creatures of that type.",
            "Flashback—{1}{R}, Behold three Elementals.",
            "When ~ enters, you may behold a Dragon. If you do, create a Treasure token.",
            "Whenever you behold a Dragon, draw a card.",
        ] {
            assert!(reads(t), "{t} must read");
        }
        for t in [
            "As an additional cost to cast ~, behold a player.",
            "Whenever a creature beholds a Dragon, draw a card.",
            "As an additional cost to cast ~, behold.",
        ] {
            assert!(!reads(t), "{t} must not read");
        }
    }

    /// CR 702.33b, 702.47a: a keyword's cost is mana, mana alternatives, or after a dash any cost.
    #[test]
    fn r702_keyword_cost_forms() {
        for t in [
            "Cumulative upkeep {U} or {B}",
            "Kicker {1}{U} and/or {B}",
            "Equip {2} or {B}",
            "Splice onto Arcane {1}{U}",
            "Splice onto Arcane—Exile four cards from your graveyard.",
            "Splice onto Arcane—Sacrifice two Mountains.",
            "Splice onto Arcane—Tap an untapped white creature you control.",
            "Ward—Pay 3 life.",
        ] {
            assert!(reads(t), "{t} must read");
        }
        for t in [
            // mana is not written after a dash, and a dash cost ends with its period
            "Splice onto Arcane—{1}{U}.",
            "Splice onto Arcane—Exile four cards from your graveyard",
            "Cumulative upkeep {U} and {B}",
        ] {
            assert!(!reads(t), "{t} must not read");
        }
    }

    /// CR 107.3: X is defined by the text that uses it, after a keyword's number as anywhere.
    #[test]
    fn r107_3_keyword_number_defined() {
        for t in [
            "Firebending X, where X is ~'s power.",
            "Mobilize X, where X is the number of creature cards in your graveyard.",
            "Devour X, where X is the number of creatures you control.",
            "Toxic 2",
            "Toxic X",
        ] {
            assert!(reads(t), "{t} must read");
        }
        // a plain number needs no definition, and a definition needs the X it defines
        for t in ["Toxic 2, where X is ~'s power.", "Toxic X, where X is 2.", "Toxic, where X is ~'s power."] {
            assert!(!reads(t), "{t} must not read");
        }
    }

    /// CR 702.14a, 702.29e, 702.48a, 702.82c: the type a keyword is named for is a combination of
    /// supertypes, card types and a subtype.
    #[test]
    fn r702_14a_the_type_a_keyword_is_named_for() {
        for t in [
            "Swampwalk",
            "Snow forestwalk",
            "Snow landwalk",
            "Legendary landwalk",
            "Nonbasic landwalk",
            "Artifact landwalk",
            "Plainscycling {2}",
            "Basic landcycling {1}{B}",
            "Artifact landcycling {2}",
            "Goblin offering",
            "Artifact offering",
            "Devour artifact 1",
            "Devour Food 3",
            "Devour land 3",
            "Devour 1",
            "First strike, legendary landwalk",
        ] {
            assert!(reads(t), "{t} must read");
        }
        // older Oracle text separates keywords with a semicolon: read as the comma list
        let a = &crate::ability::abilities().ability;
        assert!(matches!(a.parse_all(&tokenize("First strike; legendary landwalk")), Outcome::Accepted { .. }));
        // a color or status is no qualifier, and the quality of a devour has a number after it
        for t in ["Red landwalk", "Tapped landwalk", "Goblin landwalk", "Devour creature"] {
            assert!(!reads(t), "{t} must not read");
        }
    }

    /// CR 702.16a: protection is from a quality; several are listed with their own "from", joined as
    /// anything is. A color has one reading.
    #[test]
    fn r702_16a_protection_qualities() {
        for t in [
            "Protection from red",
            "Protection from black and from red",
            "Protection from blue, from black, and from red",
            "Protection from Vampires, from Werewolves, and from Zombies",
            "Protection from everything",
            "Protection from multicolored",
            "Protection from snow",
            "Hexproof from white",
            "Flying, hexproof from black and from blue",
            "{T}: Target creature gains protection from artifacts or from the color of your choice until end of turn.",
            "{T}: Another target creature you control gains protection from colorless or from the color of your choice until end of turn.",
            "Enchanted creature has protection from the chosen color.",
            "You have protection from each of your opponents.",
            "Until your next turn, you gain protection from that player.",
        ] {
            assert!(reads(t), "{t} must read");
        }
        for t in [
            // a quality is not a count, a status, or a made-up kind of choice
            "Protection from the chosen creature",
            "Protection from tapped",
            "Protection from the card name of your choice",
            "Protection from red and red",
        ] {
            assert!(!reads(t), "{t} must not read");
        }
        // the color is one value however it is reached
        let a = &crate::ability::abilities().ability;
        let Outcome::Accepted { value, .. } = a.parse_all(&tokenize("Protection from red")) else { panic!("reads") };
        assert!(format!("{value:?}").contains("Color(Red)"), "{value:?}");
    }

    /// CR 702.6a, 702.62a: sentences that follow a keyword row are about it.
    #[test]
    fn r702_keyword_row_with_sentences() {
        for t in [
            "Equip {0}. Activate only once each turn.",
            "Equip—Pay 3 life. Activate only once each turn.",
            "Equip—Sacrifice another nonland permanent. Activate only once each turn.",
            "Crew 1. Activate only once each turn.",
            "Suspend X—{X}{W}{W}. X can't be 0.",
            "Kicker {X}. X can't be 0.",
            "Equip {3}. This ability costs {1} less to activate for each other Equipment you control.",
        ] {
            assert!(reads(t), "{t} must read");
        }
        for t in [
            // the sentence must be about the ability: no instruction follows a keyword row
            "Flying. Draw a card.",
            "Equip {2}. X can't be 1.",
            "Equip {2}.",
        ] {
            assert!(!reads(t), "{t} must not read");
        }
    }

    #[test]
    fn keywords_round_trip() {
        let k = keyword();
        for t in [
            "flying",
            "toxic 2",
            "equip {2}",
            "kicker {1}{U} and/or {B}",
            "ward—Pay 3 life",
            "suspend 2—{1}{W}",
            "enchant creature you control",
            "protection from black and from red",
            "protection from everything",
            "affinity for artifacts",
            "swampwalk",
            "plainscycling {2}",
            "basic landcycling {1}{B}",
            "splice onto Arcane {1}{U}",
            "partner with Frodo, Adventurous Hobbit",
        ] {
            let toks = tokenize(t);
            let Outcome::Accepted { value, .. } = k.parse_all(&toks) else { panic!("{t}: {:?}", k.parse_all(&toks)) };
            assert_eq!(k.print_text(&value, false).as_deref(), Some(t));
        }
    }
}
