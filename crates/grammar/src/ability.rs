//! Whole abilities: one printed row, and the mode rows under it.
//!
//! ```text
//! ability   := keywords | trigger "," effect | cost ":" effect | chapters "—" effect | effect
//! effect    := sentence ("." | after a quote ending in ".")+
//! keywords  := keyword ("," keyword)*                          "Flying, vigilance"
//! chapters  := numeral ("," numeral)*                          "I, II"
//! mode      := "•" effect | ("+" | "•") mana "—" effect | "{P}"+ "—" effect | range "|" effect
//! ```
//!
//! A row whose first sentence would be a delayed trigger without "this turn" ("When ~ enters,
//! …") is a triggered ability, never text: delayed triggers are made by effects (603.7), printed
//! ones are abilities (603.1).

use crate::cost::costs;
use crate::effect::effects;
use crate::kernel::{P, alt, leaf, lit, table};
use crate::mana::mana_cost;
use crate::selector::keyword_name;
use crate::words::digits;
use ir::{Ability, Effect, KeywordInstance, Lead, Mode, Modifier, ResultRange, Sentence};
use oracle::token::Kind;
use std::sync::OnceLock;

pub struct Abilities {
    /// A printed ability row.
    pub ability: P<Ability>,
    /// An ability inside a granting quote: loyalty costs are bracketed ("[+1]: …").
    pub granted: P<Ability>,
    /// A mode row.
    pub mode: P<Mode>,
}

pub fn abilities() -> &'static Abilities {
    static A: OnceLock<Abilities> = OnceLock::new();
    A.get_or_init(build)
}

/// Sentences, each ended by its period.
pub fn effect() -> P<Effect> {
    effects().sentence.sentences().map(|v| Some(Effect(v)), |e: &Effect| Some(e.0.clone()))
}

/// The sentences of a quoted ability: the last period may be missing (see [`P::sentences_open`]).
fn quoted_effect() -> P<Effect> {
    effects().sentence.sentences_open().map(|v| Some(Effect(v)), |e: &Effect| Some(e.0.clone()))
}

/// A keyword row: "Flying, vigilance", "Equip {2}", "Ward—Pay 3 life." (a row ending in a
/// non-mana cost takes the cost's period).
/// In a quote (`granted`) keywords end with a period like any ability ("Cumulative upkeep {1}."),
/// which may be dropped mid-sentence (see [`P::sentences_open`]).
fn keywords(granted: bool) -> P<Vec<KeywordInstance>> {
    let ends_in_cost = |v: &Vec<KeywordInstance>| v.last().is_some_and(crate::keyword::ends_with_sentence);
    let kw = crate::keyword::keyword();
    let list = alt("keywords", vec![one_or_more(&kw), semicolon_list(&kw)]);
    if granted {
        // a cost's period is the ability's
        let period = alt("period", vec![lit("."), crate::kernel::empty().alias()]);
        return list.then(&period).map(|(v, ())| Some(v), |v| Some((v.clone(), ())));
    }
    let list = alt("keyword row", vec![list.clone(), crate::keyword::defined_x(&list)]);
    list.then(&lit(".").opt()).map(
        move |(v, period)| (period.is_some() == ends_in_cost(&v)).then_some(v),
        move |v| Some((v.clone(), ends_in_cost(v).then_some(()))),
    )
}

/// One item, or several separated by commas.
fn one_or_more<T: crate::kernel::Val>(p: &P<T>) -> P<Vec<T>> {
    let one = p.map(|x| Some(vec![x]), |v: &Vec<T>| (v.len() == 1).then(|| v[0].clone()));
    alt("one or more", vec![one, p.comma_list()])
}

/// Keywords separated by semicolons, as older Oracle text has them ("Flying; banding"); read
/// as the comma list.
fn semicolon_list<T: crate::kernel::Val>(p: &P<T>) -> P<Vec<T>> {
    p.then(&lit(";").then_keep(p).many())
        .map(
            |(f, mut rest)| {
                rest.insert(0, f);
                (rest.len() >= 2).then_some(rest)
            },
            |_: &Vec<T>| None,
        )
        .alias()
}

fn chapters() -> P<Vec<u32>> {
    const NUMERALS: [&str; 10] = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"];
    let numeral = table("chapter", NUMERALS.iter().enumerate().map(|(i, n)| (n.to_string(), i as u32 + 1)).collect());
    one_or_more(&numeral)
}

/// Text whose first sentence is not a printed trigger in disguise (see the module comment).
fn text(effect: &P<Effect>) -> P<Ability> {
    let trigger_like = |e: &Effect| {
        matches!(
            e.0.first(),
            Some(Sentence::Led {
                lead: Lead::When { limit: None, .. } | Lead::Modifier(Modifier::At { .. } | Modifier::AtEndOfCombat),
                ..
            })
        )
    };
    effect.map(
        move |e| (!trigger_like(&e)).then_some(Ability::Text(e)),
        move |a| match a {
            Ability::Text(e) if !trigger_like(e) => Some(e.clone()),
            _ => None,
        },
    )
}

/// Is the sentence about the ability it follows rather than something to do: a restriction on
/// activating it, a sentence about it ("This ability costs {1} less to activate"), a limit on its
/// X (107.3)? A keyword row says nothing else.
fn about_the_ability(s: &Sentence) -> bool {
    use ir::effect::Subject;
    use ir::selector::Domain;
    match s {
        Sentence::Restrictions(_) | Sentence::Rider(_) => true,
        Sentence::Static(st) => matches!(**st, ir::statics::Static::X(_)),
        Sentence::Do(body) => {
            matches!(&body.first.subject, Some(Subject::Selector(x)) if x.set.domain == Domain::Ability)
                // "Flashback {8}{B}{B}. ~ costs {X} less to cast this way, where X is …": about
                // the cast the keyword allows
                || matches!(&body.first.first.action, ir::Action::Static(st) if matches!(**st, ir::StaticAct::CostChangeThisWay { .. }))
        }
        _ => false,
    }
}

fn ability(granted: bool) -> P<Ability> {
    let effect = if granted { quoted_effect() } else { effect() };
    let keywords = keywords(granted).map(
        |k| Some(Ability::Keywords(k)),
        |a| match a {
            Ability::Keywords(k) => Some(k.clone()),
            _ => None,
        },
    );
    // a keyword row followed by sentences about it: "Equip {2}. Activate only once each turn.",
    // "Suspend X—{X}{W}{W}. X can't be 0."
    let mut v = Vec::new();
    if !granted {
        let kw = crate::keyword::keyword();
        let list = alt("keywords", vec![one_or_more(&kw), semicolon_list(&kw)]);
        v.push(list.skip(&lit(".")).then(&effect).map(
            |(keywords, rider)| rider.0.iter().all(about_the_ability).then_some(Ability::KeywordsWith { keywords, rider }),
            |a| match a {
                Ability::KeywordsWith { keywords, rider } if rider.0.iter().all(about_the_ability) => Some((keywords.clone(), rider.clone())),
                _ => None,
            },
        ));
    }
    let keywords_with = v;
    let triggered = crate::event::events().trigger.skip(&lit(",")).then(&effect).map(
        |(trigger, effect)| Some(Ability::Triggered { trigger, effect }),
        |a| match a {
            Ability::Triggered { trigger, effect } => Some((trigger.clone(), effect.clone())),
            _ => None,
        },
    );
    let cost = if granted { &costs().granted_cost } else { &costs().cost };
    // the effect's first word is capitalized after the colon
    let activated = cost.skip(&lit(":")).then(&effect).map(
        |(cost, effect)| Some(Ability::Activated { cost, effect }),
        |a| match a {
            Ability::Activated { cost, effect } => Some((cost.clone(), effect.clone())),
            _ => None,
        },
    );
    let chapter = chapters().skip(&lit("—")).then(&effect).map(
        |(chapters, effect)| Some(Ability::Chapter { chapters, effect }),
        |a| match a {
            Ability::Chapter { chapters, effect } => Some((chapters.clone(), effect.clone())),
            _ => None,
        },
    );
    let mut v = vec![keywords, triggered, activated, chapter, text(&effect)];
    v.extend(keywords_with);
    if !granted {
        v.extend(printed_only());
    }
    alt("ability", v)
}

/// Rows only a card prints: keyword prefixes, Case conditions, Class levels, leveler bands,
/// station thresholds.
fn printed_only() -> Vec<P<Ability>> {
    use crate::kernel::lazy;
    let inner = || lazy("ability", || abilities().ability.clone());
    // the keywords written "Keyword — [ability]" (702.57a, 702.142a, 702.169a, 702.177a,
    // 702.178a, 702.193a); other keywords' dashes introduce a cost ("Ward—Pay 3 life.")
    let prefixing: Vec<ir::Keyword> = ["Forecast", "Boast", "Solved", "Exhaust", "Max Speed", "Power-up"]
        .iter()
        .map(|n| ir::Keyword::from_name(n).unwrap_or_else(|| panic!("CR 702 has {n}")))
        .collect();
    let prefix = keyword_name().skip(&lit("—")).then(&inner()).map(
        {
            let prefixing = prefixing.clone();
            move |(keyword, a)| prefixing.contains(&keyword).then(|| Ability::Prefixed { keyword, ability: Box::new(a) })
        },
        move |a| match a {
            Ability::Prefixed { keyword, ability } if prefixing.contains(keyword) => Some((*keyword, (**ability).clone())),
            _ => None,
        },
    );
    let to_solve = lit("to solve —").then_keep(&crate::condition::conditions().condition).skip(&lit(".")).map(
        |c| Some(Ability::ToSolve(c)),
        |a| match a {
            Ability::ToSolve(c) => Some(c.clone()),
            _ => None,
        },
    );
    let class_level = mana_cost().skip(&lit(": level")).then(&digits()).map(
        |(cost, level)| Some(Ability::ClassLevel { cost, level }),
        |a| match a {
            Ability::ClassLevel { cost, level } => Some((cost.clone(), *level)),
            _ => None,
        },
    );
    let upper = alt(
        "band end",
        vec![
            lit("-").tight().then_keep(&digits().tight()).map(|n| Some(Some(n)), |n: &Option<u32>| *n),
            lit("+").tight().to(None),
        ],
    );
    let band = lit("LEVEL").then_keep(&digits()).then(&upper).map(
        |(from, to)| Some(Ability::LevelBand { from, to }),
        |a| match a {
            Ability::LevelBand { from, to } => Some((*from, *to)),
            _ => None,
        },
    );
    let level_pt = crate::token::pt().map(
        |(p, t)| Some(Ability::LevelPt(p, t)),
        |a| match a {
            Ability::LevelPt(p, t) => Some((p.clone(), t.clone())),
            _ => None,
        },
    );
    let station = digits().skip(&lit("+").tight()).skip(&lit("|")).then(&inner()).map(
        |(threshold, a)| Some(Ability::Station { threshold, ability: Box::new(a) }),
        |a| match a {
            Ability::Station { threshold, ability } => Some((*threshold, (**ability).clone())),
            _ => None,
        },
    );
    let companion = lit("companion —").then_keep(&crate::selector::selectors().selector)
        .skip(&lit("in your starting deck has")).then(&crate::condition::has_what()).skip(&lit(".")).map(
            |(s, has)| (s.quant == ir::Quant::Each && s.set.domain == ir::Domain::Card && s.chooser.is_none() && s.random.is_none()).then_some(Ability::Companion { each: s.set, has }),
            |a| match a { Ability::Companion {each,has} => Some((ir::Selector::new(ir::Quant::Each,each.clone()),has.clone())), _=>None });
    // the companion conditions of other shapes, each one companion's own (702.139)
    use ir::ability::CompanionRule as C;
    let rule = |text: &str, rule: C| lit(&format!("companion — {text}.")).to(Ability::CompanionRule(rule));
    let companion_rule = alt(
        "companion condition",
        vec![
            rule("each nonland card in your starting deck has a different name", C::DifferentNames),
            rule("your starting deck contains only cards with even mana values", C::EvenManaValues),
            rule("your starting deck contains only cards with odd mana values and land cards", C::OddManaValuesAndLands),
            rule(
                "your starting deck contains only cards with mana value 3 or greater and land cards",
                C::ManaValueThreeOrGreaterAndLands,
            ),
            rule(
                "each creature card in your starting deck is a Cat, Elemental, Nightmare, Dinosaur, or Beast card",
                C::CreatureTypesOfKaheera,
            ),
            rule("each permanent card in your starting deck has an activated ability", C::ActivatedAbilities),
            rule("each nonland card in your starting deck shares a card type", C::SharedCardType),
            rule(
                "your starting deck contains at least twenty cards more than the minimum deck size",
                C::TwentyMoreCards,
            ),
            rule(
                "no card in your starting deck has more than one of the same mana symbol in its mana cost",
                C::NoRepeatedManaSymbol,
            ),
        ],
    );
    vec![prefix, to_solve, class_level, band, level_pt, station, companion, companion_rule]
}

fn mode() -> P<Mode> {
    let effect = effect();
    let bullet = lit("•").then_keep(&effect).map(
        |e| Some(Mode::Bullet(e)),
        |m| match m {
            Mode::Bullet(e) => Some(e.clone()),
            _ => None,
        },
    );
    // Spree prints a plus sign, Tiered a bullet (702.172b): one row, the keyword is on the card
    let leader = alt("mode leader", vec![lit("+"), lit("•").alias()]);
    let costed = leader.then_keep(&mana_cost()).skip(&lit("—")).then(&effect).map(
        |(cost, effect)| Some(Mode::Costed { cost, effect }),
        |m| match m {
            Mode::Costed { cost, effect } => Some((cost.clone(), effect.clone())),
            _ => None,
        },
    );
    let result = result_range().skip(&lit("|")).then(&effect).map(
        |(range, effect)| Some(Mode::Result { range, effect }),
        |m| match m {
            Mode::Result { range, effect } => Some((*range, effect.clone())),
            _ => None,
        },
    );
    let paw =
        leaf("{P}", |k| matches!(k, Kind::Symbol(s) if s == "P").then_some(()), |()| Some(Kind::Symbol("P".into())));
    let paws = paw.then(&paw.clone().tight().many()).map(|((), rest)| Some(rest.len() as u32 + 1), |n| {
        (*n >= 1).then(|| ((), vec![(); *n as usize - 1]))
    });
    let pawprint = paws.skip(&lit("—")).then(&effect).map(
        |(paws, effect)| Some(Mode::Pawprint { paws, effect }),
        |m| match m {
            Mode::Pawprint { paws, effect } => Some((*paws, effect.clone())),
            _ => None,
        },
    );
    // the normaliser leaves the word on a row only when the face's own "choose" names it
    // (614.12c), so any such word starts a row of this kind
    let anchor = lit("•").then_keep(&crate::vote::word()).skip(&lit("—")).then(&effect).map(
        |(word, effect)| Some(Mode::Anchor { word, effect }),
        |m| match m {
            Mode::Anchor { word, effect } => Some((word.clone(), effect.clone())),
            _ => None,
        },
    );
    alt("mode", vec![bullet, costed, pawprint, result, anchor])
}

/// The results a die-roll table row covers (706.3a): "N", "N1—N2", "N+". The hyphen and en
/// dash are spellings of the em dash.
fn result_range() -> P<ResultRange> {
    let dash = alt("range dash", vec![lit("—").tight(), lit("-").tight().alias(), lit("–").tight().alias()]);
    let single = digits().map(
        |n| Some(ResultRange { from: n, to: Some(n) }),
        |r: &ResultRange| (r.to == Some(r.from)).then_some(r.from),
    );
    let span = digits().skip(&dash).then(&digits().tight()).map(
        |(from, to)| (from < to).then_some(ResultRange { from, to: Some(to) }),
        |r: &ResultRange| r.to.filter(|to| *to > r.from).map(|to| (r.from, to)),
    );
    let open = digits().skip(&lit("+").tight()).map(
        |from| Some(ResultRange { from, to: None }),
        |r: &ResultRange| r.to.is_none().then_some(r.from),
    );
    alt("die result range", vec![single, span, open])
}

fn build() -> Abilities {
    Abilities { ability: ability(false), granted: ability(true), mode: mode() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::Outcome;
    use oracle::token::tokenize;

    /// Events take what the lexicon says: the same types as the instructions.
    #[test]
    fn r614_1a_caused_discard_replacement_round_trips_and_rejects_wrong_objects() {
        let grammar = &abilities().ability;
        let text = "If a spell or ability an opponent controls causes you to discard ~, put it onto the battlefield instead of putting it into your graveyard.";
        let Outcome::Accepted { value, .. } = grammar.parse_all(&tokenize(text)) else { panic!("replacement must read"); };
        assert_eq!(grammar.print_text(&value, true).as_deref(), Some(text));
        for text in [
            "If a spell or ability an opponent controls causes you to discard a player, put it onto the battlefield instead of putting it into your graveyard.",
            "If a spell or ability an opponent controls causes you to discard ~, put target player onto the battlefield instead of putting it into your graveyard.",
        ] {
            assert!(!matches!(grammar.parse_all(&tokenize(text)), Outcome::Accepted { .. }), "{text}");
        }
    }

    #[test]
    fn triggers_are_typed() {
        let a = &abilities().ability;
        let reads = |t: &str| matches!(a.parse_all(&tokenize(&format!("{t}, draw a card."))), Outcome::Accepted { .. });
        for t in [
            "Whenever a creature dies",
            "Whenever you sacrifice a creature",
            "Whenever a creature becomes tapped",
            "Whenever you draw a card",
            "Whenever ~ deals damage to a player",
            "When ~ enters",
        ] {
            assert!(reads(t), "{t} must read");
        }
        for t in [
            "Whenever a player dies",
            "Whenever a creature draws a card",
            "Whenever you sacrifice an opponent",
            "Whenever you cast your fifth other player or battle each turn",
            "Whenever a player becomes tapped",
        ] {
            assert!(!reads(t), "{t} must not read");
        }
    }

    fn mode_reads(t: &str) -> Option<Mode> {
        match abilities().mode.parse_all(&tokenize(t)) {
            Outcome::Accepted { value, .. } => Some(value),
            _ => None,
        }
    }

    /// CR 706.3a: a results table row is a single number, "N1–N2" or "N+", then its effect.
    #[test]
    fn r706_3a_die_result_rows() {
        let m = &abilities().mode;
        for text in ["1—9 | Draw a card.", "20 | Scry 1.", "15+ | You gain 3 life.", "10—19 | Each player draws a card."] {
            let value = mode_reads(text).unwrap_or_else(|| panic!("{text} must read"));
            assert!(matches!(value, Mode::Result { .. }));
            assert_eq!(m.print_text(&value, true).as_deref(), Some(text));
        }
        // the hyphen is the same row
        assert_eq!(mode_reads("1-9 | Draw a card."), mode_reads("1—9 | Draw a card."));
        assert!(mode_reads("1-9 | Draw a card.").is_some());
        for text in ["9—1 | Draw a card.", "5—5 | Draw a card.", "| Draw a card.", "d20 | Draw a card.", "1—9 Draw a card."] {
            assert!(mode_reads(text).is_none(), "{text} must not read");
        }
    }

    /// CR 702.172a, 702.183a: a mode with its own cost; Spree's plus sign and Tiered's bullet are
    /// one row (702.172b).
    #[test]
    fn r702_183a_mode_with_its_own_cost() {
        let m = &abilities().mode;
        let spree = mode_reads("+ {1} — Draw a card.").expect("spree row");
        assert!(matches!(spree, Mode::Costed { .. }));
        assert_eq!(m.print_text(&spree, true).as_deref(), Some("+ {1} — Draw a card."));
        assert_eq!(mode_reads("• {1} — Draw a card."), Some(spree));
        assert!(mode_reads("• {1} Draw a card.").is_none());
        assert!(mode_reads("{1} — Draw a card.").is_none());
    }

    /// CR 614.12c: "[Anchor word] — [ability]" under a choice between words.
    #[test]
    fn r614_12c_anchor_word_row() {
        let m = &abilities().mode;
        for text in [
            "• Khans — At the beginning of your upkeep, draw a card.",
            "• Temur — Creatures you control get +1/+0 and have trample and haste.",
            "• NCR — Whenever a creature you control dies, draw a card.",
        ] {
            let value = mode_reads(text).unwrap_or_else(|| panic!("{text} must read"));
            assert!(matches!(value, Mode::Anchor { .. }));
            assert_eq!(m.print_text(&value, true).as_deref(), Some(text));
        }
        // a number, a color or a keyword is not an anchor word; a cost is a costed mode
        for text in ["• 5 — Draw a card.", "• red — Draw a card.", "• flying — Draw a card.", "• Khans Draw a card.", "Khans — Draw a card."] {
            assert!(mode_reads(text).is_none(), "{text} must not read");
        }
    }

    /// Codie, Ravenous Codex (a17e414c-8de0-47fd-bcbc-540c7fec7642), full
    /// triggered Oracle row. CR 603.1/722.3c-d: its caster is you and its object
    /// is a prepared spell; copying it and optionally retargeting the copy stay separate.
    #[test]
    fn r722_3c_codie_prepared_copy_row_preserves_trigger_and_optional_retargeting() {
        use ir::{Atom, Domain, Filter, ObjectSet, Selector, Quant, Count, Amount, EventAction};
        use ir::event::{Trigger, TriggerWord, EventPattern};
        let text = "Whenever you cast a prepared spell, copy it. You may choose new targets for the copy.";
        let grammar = &abilities().ability;
        let Outcome::Accepted { value, .. } = grammar.parse_all(&tokenize(text)) else { panic!("Codie's pinned trigger must parse") };
        let Ability::Triggered { trigger, effect } = &value else { panic!("triggered ability") };
        let spell = Selector::new(Quant::Indefinite(Count::Exactly(Amount::Const(1))),
            ObjectSet { filter: Filter::All(vec![Filter::Atom(Atom::PreparedSpell)]), ..ObjectSet::of(Domain::Spell) });
        assert_eq!(*trigger, Trigger::Event { word: TriggerWord::Whenever,
            pattern: EventPattern::new(vec![Selector::you()], vec![EventAction::does(ir::Verb::Cast, spell)]) });
        let [ir::effect::Sentence::Do(copy), ir::effect::Sentence::Do(retarget)] = effect.0.as_slice() else { panic!("two instructions") };
        assert!(!copy.first.may);
        assert!(matches!(&copy.first.first.action, ir::action::Action::Copy(objects)
            if objects.items.len()==1 && objects.items[0].quant==Quant::Definite(ir::Ref::Anaphor(ir::selector::Anaphor::It))));
        assert!(retarget.first.may);
        assert_eq!(retarget.first.subject, Some(ir::effect::Subject::Selector(Selector::you())));
        assert!(matches!(&retarget.first.first.action, ir::action::Action::ChooseNewTargets {for_,one:false}
            if for_.items.len()==1 && for_.items[0].quant==Quant::Definite(ir::Ref::Anaphor(ir::selector::Anaphor::Copy {plural:false}))));
        assert_eq!(grammar.print_text(&value, true).as_deref(), Some(text));
        let Outcome::Accepted { value: reparsed, .. } = grammar.parse_all(&tokenize(&grammar.print_text(&value, true).unwrap())) else { panic!("printed row must parse") };
        assert_eq!(reparsed, value);
    }

    #[test]
    fn abilities_round_trip() {
        let a = &abilities().ability;
        for text in [
            "Flying, vigilance",
            "When ~ enters, draw a card.",
            "{T}: Add {G}.",
            "{2}, {T}, Sacrifice ~: Draw a card. Activate only as a sorcery.",
            "+1: Draw a card.",
            "I, II — Draw a card.",
            "Destroy target creature. Its controller gains 3 life.",
            "Creatures you control get +1/+1.",
            "Until end of turn, ~ becomes a 2/2 creature with \"{T}: Add {G}.\" It's still a land.",
        ] {
            let toks = tokenize(text);
            match a.parse_all(&toks) {
                Outcome::Accepted { value, .. } => {
                    assert_eq!(a.print_text(&value, true).as_deref(), Some(text), "{value:#?}")
                }
                other => panic!("{text}: {other:#?}"),
            }
        }
        let m = &abilities().mode;
        for text in ["• Draw a card.", "+ {1} — Draw a card.", "{P}{P} — Draw a card."] {
            let toks = tokenize(text);
            match m.parse_all(&toks) {
                Outcome::Accepted { value, .. } => {
                    assert_eq!(m.print_text(&value, true).as_deref(), Some(text), "{value:#?}")
                }
                other => panic!("{text}: {other:#?}"),
            }
        }
    }
}

#[test]
fn r702_139_starting_deck_condition_roundtrip_and_rejection() {
    use crate::kernel::Outcome;
    use oracle::token::tokenize;
    let grammar=&abilities().ability;
    for text in ["Companion — Each permanent card in your starting deck has mana value 2 or less.","Companion — Each creature card in your starting deck has mana value 3 or greater."] {
        let Outcome::Accepted {value,..}=grammar.parse_all(&tokenize(text)) else {panic!("{text}")};
        let said=grammar.print_text(&value,false).unwrap();
        let Outcome::Accepted {value:again,..}=grammar.parse_all(&tokenize(&said)) else {panic!("{said}")};assert_eq!(value,again);
    }
    for text in ["Companion — Target permanent card in your starting deck has mana value 2 or less.","Companion — Each player in your starting deck has mana value 2 or less.","Companion — Each permanent card in your graveyard has mana value 2 or less."] {assert!(!matches!(grammar.parse_all(&tokenize(text)),Outcome::Accepted{..}),"{text}");}
}
