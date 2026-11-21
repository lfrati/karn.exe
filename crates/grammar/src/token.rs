//! Token descriptions (CR 111): the object of "create".
//!
//! ```text
//! token_spec := count(n) ["tapped"] (predefined(n) | described(n))
//! predefined := "Treasure" | "Food" | "Wicked Role" | …  "token(s)"           (CR 111.10)
//! described  := ["legendary"] [P/T] [colors] subtype* type+ "token(s)" ["named" Name] ["with" keywords]
//! ```

use crate::kernel::{P, alt, leaf, lit, table};
use crate::words::{Number, count_amount, digits};
use ir::token::Combat;
use ir::{
    Amount, CardType, Color, Domain, Granted, KeywordInstance, Predefined, Pt, Subtype, Supertype, TokenDesc,
    TokenKind, TokenSpec,
};
use oracle::token::{Kind, tokenize};
use std::sync::OnceLock;

pub fn token_spec() -> &'static P<TokenSpec> {
    static T: OnceLock<P<TokenSpec>> = OnceLock::new();
    T.get_or_init(|| alt("token", vec![spec(Number::Sg), spec(Number::Pl)]))
}

/// The append-one active replacement's additional token, with its own characteristics.
pub(crate) fn additional_spec() -> P<TokenSpec> {
    lit("an additional").then_keep(&alt("additional token", vec![predefined(Number::Sg), described(Number::Sg)]))
        .map(|kind| Some(TokenSpec {count:Amount::Const(1),tapped:false,kind,combat:None,attached_to:None}),
            |t| (t.count==Amount::Const(1) && !t.tapped && t.combat.is_none() && t.attached_to.is_none()).then(|| t.kind.clone()))
}

fn noun(n: Number) -> P<()> {
    lit(if n == Number::Sg { "token" } else { "tokens" })
}

fn count(n: Number) -> P<Amount> {
    match n {
        Number::Sg => alt("a", vec![lit("a"), lit("an").alias()]).to(Amount::Const(1)),
        Number::Pl => count_amount()
            .map(|a| (a != Amount::Const(1)).then_some(a), |a| (*a != Amount::Const(1)).then(|| a.clone())),
    }
}

fn spec(n: Number) -> P<TokenSpec> {
    // "a 2/2 Citizen creature token that's all colors": the five colors, said after the noun
    let all_colors = lit(if n == Number::Sg { "that's all colors" } else { "that are all colors" });
    let described = described(n).then(&all_colors.opt()).map(
        |(kind, all)| match (kind, all) {
            (kind, None) => Some(kind),
            (TokenKind::Described(mut d), Some(())) if d.colors.is_none() => {
                d.colors = Some(ir::Color::ALL.to_vec());
                Some(TokenKind::Described(d))
            }
            _ => None,
        },
        |kind| Some((kind.clone(), None)),
    );
    let mut bodies = vec![predefined(n), described];
    if n == Number::Pl {
        bodies.push(lit("of those tokens").to(TokenKind::Those));
    }
    let body = alt("token body", bodies);
    let attached = lit("attached to").then_keep(&selector());
    let rest = lit("tapped").opt().then(&body).then(&placement(n).opt()).then(&attached.opt());
    let make = |count: Amount, (((tapped, kind), state), attached): RestParts| {
        // "a tapped … token that's attacking" is "… that's tapped and attacking"; tapped twice is not
        if tapped.is_some() && state.as_ref().is_some_and(|(late, _)| *late) {
            return None;
        }
        let (late_tapped, combat) = match state {
            Some((tapped, combat)) => (tapped, Some(combat)),
            None => (false, None),
        };
        // only a creature can attack or block, and the predefined tokens are not creatures (111.10)
        if combat.is_some() && !creature(&kind) {
            return None;
        }
        Some(TokenSpec {
            count,
            tapped: tapped.is_some() || late_tapped,
            kind,
            combat,
            attached_to: attached.map(Box::new),
        })
    };
    let unmake = |t: &TokenSpec| -> RestParts {
        let state = t.combat.clone().map(|c| (t.tapped, c));
        let early = (t.tapped && t.combat.is_none()).then_some(());
        (((early, t.kind.clone()), state), t.attached_to.as_deref().cloned())
    };
    let counted = count(n).then(&rest).map(
        move |(count, r)| make(count, r),
        move |t| (t.combat.is_none() || creature(&t.kind)).then(|| (t.count.clone(), unmake(t))),
    );
    if n == Number::Sg {
        return alt("token", vec![counted.clone(), leading_name(&counted)]);
    }
    // "create one of those tokens": the count one, which "tokens" cannot carry (count above is
    // singular only for "a")
    let one_of_those = lit("one of those tokens").map(
        |()| {
            Some(TokenSpec {
                count: Amount::Const(1),
                tapped: false,
                kind: TokenKind::Those,
                combat: None,
                attached_to: None,
            })
        },
        |t: &TokenSpec| {
            (t.count == Amount::Const(1)
                && !t.tapped
                && t.kind == TokenKind::Those
                && t.combat.is_none()
                && t.attached_to.is_none())
            .then_some(())
        },
    );
    // "a number of 1/1 Soldier creature tokens with flying equal to …"
    let literal = crate::words::is_quantity;
    let numbered = lit("a number of")
        .then_keep(&rest)
        .skip(&lit("equal to"))
        .then(&crate::kernel::lazy("amount", || crate::amount::amounts().amount.clone()))
        .map(
            move |(r, count)| if literal(&count) { None } else { make(count, r) },
            move |t| (!literal(&t.count)).then(|| (unmake(t), t.count.clone())),
        );
    alt("tokens", vec![counted, numbered, one_of_those])
}

/// Can this token attack or block? A described creature token; "those tokens" were creatures when
/// described, a predefined token is not (111.10).
fn creature(kind: &TokenKind) -> bool {
    match kind {
        TokenKind::Described(d) => d.types.contains(&CardType::Creature),
        TokenKind::Those => true,
        TokenKind::Predefined(_) => false,
    }
}

fn selector() -> P<ir::Selector> {
    crate::kernel::lazy("selector", || crate::selector::selectors().selector.clone())
}

/// A token put onto the battlefield in combat: "that's tapped and attacking", "that's attacking
/// that opponent" (508.4: its controller chooses what it attacks unless the effect specifies),
/// "that's blocking that creature" (506.3e). The late "tapped" is part of the phrase. Public so
/// that other creations of tokens (copies) can say the same.
pub fn placement(n: Number) -> P<(bool, Combat)> {
    let that = if n == Number::Sg { "that's" } else { "that are" };
    // what it attacks: a player, or a permanent (planeswalker, battle); what it blocks: a permanent
    let player_or_permanent = |s: &ir::Selector| matches!(s.set.domain, Domain::Player | Domain::Permanent);
    let attacking = move |tapped: bool| {
        let words = if tapped { "tapped and attacking" } else { "attacking" };
        lit(&format!("{that} {words}")).then_keep(&selector().opt()).map(
            move |who| {
                who.as_ref().is_none_or(player_or_permanent).then(|| (tapped, Combat::Attacking(who.map(Box::new))))
            },
            move |(t, c): &(bool, Combat)| match c {
                Combat::Attacking(who) if *t == tapped && who.as_deref().is_none_or(player_or_permanent) => {
                    Some(who.as_deref().cloned())
                }
                _ => None,
            },
        )
    };
    let blocking = lit(&format!("{that} blocking")).then_keep(&selector()).map(
        |who| (who.set.domain == Domain::Permanent).then(|| (false, Combat::Blocking(Box::new(who)))),
        |(t, c): &(bool, Combat)| match c {
            Combat::Blocking(who) if !*t && who.set.domain == Domain::Permanent => Some((**who).clone()),
            _ => None,
        },
    );
    alt("combat placement", vec![attacking(true), attacking(false), blocking])
}

/// The parts of a token spec after its count: early "tapped", kind, "that's tapped and
/// attacking", "attached to".
type RestParts = (((Option<()>, TokenKind), Option<(bool, Combat)>), Option<ir::Selector>);

/// One piece of a token's name (CR 111.4: the effect that creates a token names it). Token names
/// are not card names, so they are read as pieces: capitalised words, the creating card's own
/// name (`~`, 201.5: "~'s Coil"), the possessive, and the small words that join them.
fn name_piece(comma: bool) -> P<String> {
    fn small(w: &str) -> bool {
        matches!(w, "of" | "the" | "to")
    }
    leaf(
        "token name word",
        move |k| match k {
            Kind::Word(w) if w.chars().next().is_some_and(char::is_uppercase) || small(w) => Some(w.clone()),
            Kind::SelfRef => Some("~".to_string()),
            Kind::Clitic(c) if c == "'s" => Some(c.clone()),
            Kind::Punct(',') if comma => Some(",".to_string()),
            _ => None,
        },
        |s| {
            Some(match s.as_str() {
                "~" => Kind::SelfRef,
                "'s" => Kind::Clitic("'s".into()),
                "," => Kind::Punct(','),
                w => Kind::Word(w.to_string()),
            })
        },
    )
}

/// A token's name. `comma`: a comma may be inside it ("Voja, Friend to Elves"), which is only
/// safe where something else ends the name (", a legendary …").
fn token_name(comma: bool) -> P<String> {
    fn small(w: &str) -> bool {
        matches!(w, "of" | "the" | "to")
    }
    name_piece(comma).many().map(
        |pieces| {
            let first = pieces.first()?;
            let last = pieces.last()?;
            if matches!(first.as_str(), "'s" | ",") || small(first) || last == "," || small(last) {
                return None;
            }
            let mut out = String::new();
            for (i, p) in pieces.iter().enumerate() {
                if matches!(p.as_str(), "'s" | ",") && (pieces[i - 1] == "," || small(&pieces[i - 1])) {
                    return None;
                }
                if i > 0 && !matches!(p.as_str(), "'s" | ",") {
                    out.push(' ');
                }
                out.push_str(p);
            }
            Some(out)
        },
        |name: &String| {
            tokenize(name)
                .iter()
                .map(|t| match &t.kind {
                    Kind::Word(w) => Some(w.clone()),
                    Kind::SelfRef => Some("~".to_string()),
                    Kind::Clitic(c) => Some(c.clone()),
                    Kind::Punct(',') => Some(",".to_string()),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()
        },
    )
}

/// "Boo, a legendary 1/1 red Hamster creature token": the name leads, and only a legendary token
/// is written this way (CR 205.4a: a legendary permanent's name is unique); any other token is
/// "… token named X". The value decides which, so neither spelling is recorded.
fn leading_name(counted: &P<TokenSpec>) -> P<TokenSpec> {
    token_name(true).skip(&lit(",")).then(counted).map(
        |(name, mut spec)| {
            let TokenKind::Described(d) = &mut spec.kind else {
                return None;
            };
            if d.name.is_some() || !d.supertypes.contains(&Supertype::Legendary) {
                return None;
            }
            d.name = Some(name);
            Some(spec)
        },
        |spec| {
            let TokenKind::Described(d) = &spec.kind else {
                return None;
            };
            let name = d.name.clone().filter(|_| d.supertypes.contains(&Supertype::Legendary))?;
            let mut bare = spec.clone();
            if let TokenKind::Described(b) = &mut bare.kind {
                b.name = None;
            }
            Some((name, bare))
        },
    )
}

fn predefined(n: Number) -> P<TokenKind> {
    let name = table("predefined token", Predefined::ALL.iter().map(|p| (p.name().to_string(), *p)).collect());
    name.skip(&noun(n)).map(
        |p| Some(TokenKind::Predefined(p)),
        |k| match k {
            TokenKind::Predefined(p) => Some(*p),
            _ => None,
        },
    )
}

fn pt_value() -> P<Pt> {
    alt(
        "power or toughness",
        vec![
            digits().map(
                |n| Some(Pt::Const(n)),
                |p| match p {
                    Pt::Const(n) => Some(*n),
                    Pt::X => None,
                },
            ),
            lit("X").to(Pt::X),
        ],
    )
}

pub fn pt() -> P<(Pt, Pt)> {
    pt_value().skip(&lit("/").tight()).then(&pt_value().tight())
}

pub fn colors() -> P<Vec<Color>> {
    let color = table("color", Color::ALL.iter().map(|c| (c.word().to_string(), *c)).collect());
    // "all colors" is the five colors of 105.1: the list of exactly those prints as "all colors"
    // and spelling them out is the alias
    let every = |v: &Vec<Color>| v.as_slice() == Color::ALL;
    alt(
        "colors",
        vec![
            lit("colorless").to(Vec::new()),
            lit("all colors").to(Color::ALL.to_vec()),
            color.map(|c| Some(vec![c]), |v| (v.len() == 1).then(|| v[0])),
            color.list("and").map(move |v| (!every(&v)).then_some(v), move |v| (!every(v)).then(|| v.clone())),
            color.list("and").map(move |v| every(&v).then_some(v), |_| None).alias(),
        ],
    )
}

fn abilities() -> P<Vec<Granted>> {
    lit("with").then_keep(&crate::action::granted())
}

/// "flying", "flying and haste", "toxic 1": keywords with their parameters.
pub fn keyword_list() -> P<Vec<KeywordInstance>> {
    let kw = crate::keyword::keyword();
    let one = kw.map(|k| Some(vec![k]), |v| (v.len() == 1).then(|| v[0].clone()));
    alt("keywords", vec![one, kw.list("and")])
}

fn described(n: Number) -> P<TokenKind> {
    let supertype = table("supertype", Supertype::ALL.iter().map(|s| (s.word().to_string(), *s)).collect());
    let subtype = table("subtype", Subtype::all().map(|s| (s.name().to_string(), s)).collect());
    let card_type = table("card type", CardType::ALL.iter().map(|t| (t.word().to_string(), *t)).collect());
    // "named X" before the abilities, or (an alias: the same name and abilities) after them
    let named = lit("named").then_keep(&token_name(false));
    let name_first = named.opt().then(&abilities().opt()).map(
        |(name, keywords)| Some((name, keywords.unwrap_or_default())),
        |(name, keywords): &(Option<String>, Vec<Granted>)| {
            Some((name.clone(), (!keywords.is_empty()).then(|| keywords.clone())))
        },
    );
    let name_last = abilities().then(&named).map(|(keywords, name)| Some((Some(name), keywords)), |_| None).alias();
    let tail = alt("name and abilities", vec![name_first, name_last]);
    supertype
        .many()
        .then(&pt().opt())
        .then(&colors().opt())
        .then(&subtype.many())
        .then(&card_type.then(&card_type.many()))
        .skip(&noun(n))
        .then(&tail)
        .map(
            |(((((supertypes, pt), colors), subtypes), (t0, mut types)), (name, keywords))| {
                // a legendary token's name leads its description (`leading_name`)
                if name.is_some() && supertypes.contains(&Supertype::Legendary) {
                    return None;
                }
                types.insert(0, t0);
                Some(TokenKind::Described(TokenDesc {
                    supertypes,
                    pt,
                    colors,
                    subtypes,
                    types,
                    name,
                    abilities: keywords,
                }))
            },
            |k| {
                let TokenKind::Described(d) = k else {
                    return None;
                };
                if d.name.is_some() && d.supertypes.contains(&Supertype::Legendary) {
                    return None;
                }
                let (t0, rest) = d.types.split_first()?;
                Some((
                    (
                        (((d.supertypes.clone(), d.pt.clone()), d.colors.clone()), d.subtypes.clone()),
                        (*t0, rest.to_vec()),
                    ),
                    (d.name.clone(), d.abilities.clone()),
                ))
            },
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::Outcome;
    use oracle::token::tokenize;

    #[test]
    fn round_trips() {
        for t in [
            "a Treasure token",
            "two Treasure tokens",
            "a tapped Powerstone token",
            "a Wicked Role token",
            "a 1/1 white Spirit creature token with flying",
            "a 2/2 black Zombie creature token",
            "X 1/1 colorless Thopter artifact creature tokens with flying",
            "a 4/4 blue and red Elemental creature token",
            "a 1/1 white Human Soldier creature token",
            "a 3/3 green Beast creature token with vigilance and trample",
            "a 1/1 black Phyrexian Mite artifact creature token with toxic 1",
            "a 2/2 white Cat creature token with first strike, vigilance, and lifelink",
            "a 1/1 colorless Construct artifact creature token named Cadet",
            // 111.4: the name; a legendary token's leads, any other's follows
            "Boo, a legendary 1/1 red Hamster creature token with trample",
            "Voja, Friend to Elves, a legendary 3/3 green and white Wolf creature token",
            "Ashaya, the Awoken World, a legendary 4/4 green Elemental creature token",
            "~'s Notebook, a legendary colorless Book artifact token",
            "a 3/3 blue Serpent creature token named ~'s Coil",
            "a 1/1 colorless Insect artifact creature token named Hornet with flying and haste",
            // 508.4, 506.3e
            "a 1/1 white Soldier creature token that's tapped and attacking",
            "a 1/1 white Soldier creature token that's tapped and attacking that opponent",
            "a 2/2 green Boar creature token that's attacking that player",
            "a 1/1 white Cat Soldier creature token with vigilance that's blocking that creature",
            "three 1/1 white Soldier creature tokens that are tapped and attacking",
            "one of those tokens",
        ] {
            let toks = tokenize(t);
            let Outcome::Accepted { value, .. } = token_spec().parse_all(&toks) else {
                panic!("{t}: {:?}", token_spec().parse_all(&toks))
            };
            assert_eq!(token_spec().print_text(&value, false).as_deref(), Some(t));
        }
    }

    /// The name after the abilities is the same value as the name before them, and prints there.
    #[test]
    fn name_after_the_abilities_is_an_alias() {
        let after = tokenize("a 1/1 green Insect creature token with flying named Butterfly");
        let before = tokenize("a 1/1 green Insect creature token named Butterfly with flying");
        let (Outcome::Accepted { value: a, .. }, Outcome::Accepted { value: b, .. }) =
            (token_spec().parse_all(&after), token_spec().parse_all(&before))
        else {
            panic!("both positions read")
        };
        assert_eq!(a, b);
        assert_eq!(
            token_spec().print_text(&a, false).as_deref(),
            Some("a 1/1 green Insect creature token named Butterfly with flying")
        );
    }

    /// "a tapped … token that's attacking" is the value of "… that's tapped and attacking".
    #[test]
    fn early_tapped_with_attacking_is_an_alias() {
        let early = tokenize("a tapped 1/1 white Soldier creature token that's attacking");
        let late = tokenize("a 1/1 white Soldier creature token that's tapped and attacking");
        let (Outcome::Accepted { value: a, .. }, Outcome::Accepted { value: b, .. }) =
            (token_spec().parse_all(&early), token_spec().parse_all(&late))
        else {
            panic!("both read")
        };
        assert_eq!(a, b);
    }

    #[test]
    fn rejected() {
        for t in [
            // a name leads only a legendary token's description, and follows any other's
            "Foo, a 1/1 red Hamster creature token",
            "a legendary 1/1 red Hamster creature token named Foo",
            // a name is capitalised words, not any text
            "a 1/1 red Hamster creature token named foo bar",
            "Foo bar, a legendary 1/1 red Hamster creature token",
            "a 1/1 red Hamster creature token named of",
            // a token is either tapped early or tapped and attacking late
            "a tapped 1/1 white Soldier creature token that's tapped and attacking",
            "a 1/1 white Soldier creature token that's tapped and blocking that creature",
            "a 1/1 white Soldier creature token that's blocking",
            // only a creature enters attacking or blocking, and blocks a creature
            "a Treasure token that's attacking",
            "a 1/1 white Soldier creature token that's blocking target spell",
            "a 1/1 white Soldier creature token that's attacking target spell",
        ] {
            let toks = tokenize(t);
            assert!(!matches!(token_spec().parse_all(&toks), Outcome::Accepted { .. }), "{t}");
        }
    }
}
