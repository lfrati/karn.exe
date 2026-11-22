//! Costs compiled once into payment plans (118, 601.2f–h, 602.2b). Runtime asks the
//! same compiled candidate predicates for validation and payment; it never interprets syntax.
use crate::binding::AbilityKey;
use crate::program::expr::{self, NoLocals, linked};
use crate::select::{Ctx, Support, name};
use crate::{CardDef, Game, ObjId, Res, Target, unsupported};
use ir::{Ability, Action, Amount, Conj, Cost, CostPart, Count, Domain, Loyalty, Quant, Ref, Verb};
use ir::mana::{ManaCost, ManaSymbol};

/// The one variable symbol whose value a player announces (107.3a).
pub(crate) const X: ManaSymbol = ManaSymbol::Var('X');

/// A symbol paid with mana of its color or with 2 life (107.4f).
fn phyrexian(s: &ManaSymbol) -> bool { matches!(s, ManaSymbol::Phyrexian(_) | ManaSymbol::HybridPhyrexian(..)) }

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Payment {
    Mana(ManaCost), Tap(ObjId), Untap(ObjId), Life(u32), Reveal(ObjId),
    Objects { verb: Verb, objects: Vec<ObjId>, producer: Option<AbilityKey> },
    /// Put that many loyalty counters on the permanent, or remove them when negative (606.4).
    Loyalty(ObjId, i32),
    RemoveCounters { on: Target, kind: String, count: u32 },
    /// Blight N: that many -1/-1 counters put on the creature its payer chose (701.68a).
    Blight { on: ObjId, count: u32 },
}
#[derive(Debug, Clone, PartialEq, Eq)]
/// `Behold`: a [quality] card in your hand or a [quality] permanent you control (701.4a).
enum Candidates { Hand(expr::Cards), Set(expr::Set), Behold(expr::Cards, expr::Set) }
/// What "behold a Dragon" chooses among, which 701.4a says it means: "Reveal a Dragon card
/// from your hand or choose a Dragon permanent you control on the battlefield." The cards, of
/// whatever hand they are looked for in; the permanents, controlled by "you".
pub(crate) fn beheld(selector: &ir::Selector) -> Res<(ir::ObjectSet, ir::ObjectSet)> {
    let set = &selector.set;
    if set.controller.is_some() || set.owner.is_some() || set.caster.is_some() || set.zone.is_some() || set.other {
        return unsupported("behold: of other than a quality");
    }
    let you = ir::Relation { who: Box::new(ir::Selector::new(Quant::Definite(Ref::You), ir::ObjectSet::of(Domain::Player))), negated: false, past: false };
    Ok((ir::ObjectSet { domain: Domain::Card, ..set.clone() }, ir::ObjectSet { domain: Domain::Permanent, controller: Some(you), ..set.clone() }))
}

impl Candidates {
    /// "behold a Dragon" (701.4a): a Dragon card in your hand or a Dragon permanent you control
    fn behold(selector: &ir::Selector, support: &Support<'_>) -> Res<Self> {
        let (card, permanent) = beheld(selector)?;
        Ok(Self::Behold(expr::Cards::compile(&card, support, &mut linked(support))?, expr::Set::compile(&permanent, support, &mut linked(support))?))
    }
    fn compile(verb: Verb, selector: &ir::Selector, support: &Support<'_>) -> Res<Self> {
        if selector.set.zone.as_ref().is_some_and(|z| z.zones.items.contains(&ir::Zone::Library)) {
            return unsupported("cost: selecting cards in a library requires an explicit observation/selection procedure");
        }
        if selector.set.domain == Domain::Card && selector.set.zone.is_none() {
            if verb != Verb::Discard { return unsupported("cost: exile a card without a supported zone"); }
            expr::Cards::compile(&selector.set, support, &mut linked(support)).map(Self::Hand)
        } else { expr::Set::compile(&selector.set, support, &mut linked(support)).map(Self::Set) }
    }

    fn among(&self, game: &Game, ctx: &Ctx, verb: Verb, blight: u32) -> Vec<ObjId> {
        let objects: Vec<ObjId> = match self {
            Self::Hand(cards) => game.players[ctx.controller].hand.iter().copied().filter(|o| cards.contains(game, ctx, &NoLocals, *o)).collect(),
            Self::Set(set) => set.members(game, ctx, &NoLocals).into_iter().filter_map(|t| if let Target::Object(o) = t { Some(o) } else { None }).collect(),
            Self::Behold(cards, set) => game.players[ctx.controller].hand.iter().copied().filter(|o| cards.contains(game, ctx, &NoLocals, *o))
                .chain(set.members(game, ctx, &NoLocals).into_iter().filter_map(|t| if let Target::Object(o) = t { Some(o) } else { None })).collect(),
        };
        objects.into_iter().filter(|o| match verb {
            // blight: what can have counters put on it (701.68a, 122.1)
            Verb::PutCounters if blight == 0 => game.holds_counters(Target::Object(*o)),
            Verb::PutCounters => game.may_place(Target::Object(*o), "-1/-1", ctx.controller),
            _ => game.can(if verb == Verb::CollectEvidence { Verb::Exile } else { verb }, ctx.controller, Target::Object(*o)),
        }).collect()
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
enum Objects { Source(Option<Candidates>), Choose { selection: Selection, among: Candidates },
    /// The one object its payer chose as they announced it (601.2b): the creature tapped for
    /// harmonize (702.180b).
    Fixed(ObjId) }

/// A cost selection's cardinality or a property of the whole selected group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    Exact(usize),
    /// Any subset up to the stated maximum; zero remains a valid choice.
    UpTo(usize),
    /// Any number of candidates whose union contains at least this many card types.
    CardTypesAtLeast { minimum: u32, types: Vec<(ObjId, u16)> },
    /// Any number of candidates with total power this or greater (702.122a), each candidate's
    /// power as it is now; a negative power counts as itself (107.1b).
    PowerAtLeast { minimum: u32, powers: Vec<(ObjId, i32)> },
    /// Any number of candidates with total mana value this or greater: collect evidence N
    /// (701.59a), each card's mana value as it is now.
    ManaValueAtLeast { minimum: u32, values: Vec<(ObjId, u32)> },
    /// Up to this many artifacts and creatures, each paying {1} of a waterbend cost's generic
    /// mana instead (701.67a, 701.67b): `generic`, and `xs` more for each {X} until announced.
    Waterbend { generic: u32, xs: u32 },
}
impl Selection {
    pub fn exact(&self) -> Option<usize> { if let Self::Exact(n) = self { Some(*n) } else { None } }
    pub fn accepts(&self, cards: &[ObjId]) -> bool {
        match self {
            Self::Exact(n) => cards.len() == *n,
            Self::UpTo(n) => cards.len() <= *n,
            Self::CardTypesAtLeast { minimum, types } =>
                types.iter().filter(|(o,_)| cards.contains(o)).fold(0u16, |mask,(_,bits)| mask | bits).count_ones() >= *minimum,
            Self::PowerAtLeast { minimum, powers } =>
                powers.iter().filter(|(o,_)| cards.contains(o)).map(|(_,p)| i64::from(*p)).sum::<i64>() >= i64::from(*minimum),
            Self::ManaValueAtLeast { minimum, values } =>
                values.iter().filter(|(o,_)| cards.contains(o)).map(|(_,v)| u64::from(*v)).sum::<u64>() >= u64::from(*minimum),
            Self::Waterbend { generic, .. } => cards.len() <= *generic as usize,
        }
    }
    pub(crate) fn evaluated(&self, game: &Game, ctx: &Ctx, among: &[ObjId]) -> Self {
        match self {
            Self::Exact(n) => Self::Exact(*n),
            Self::UpTo(n) => Self::UpTo(*n),
            Self::CardTypesAtLeast { minimum, .. } => Self::CardTypesAtLeast { minimum: *minimum,
                types: among.iter().map(|o| (*o, expr::card_type_mask(game, ctx, *o))).collect() },
            Self::PowerAtLeast { minimum, .. } => {
                // a crew or saddle cost counts each creature's power as rules say to (702.122a, 702.171a)
                let def = ctx.ability.as_ref().zip(ctx.source).and_then(|(key, source)| Some((key, game.definition(source, &key.origin)?)));
                let crewing = def.clone().is_some_and(|(key, d)| crate::keyword::crews(d, key.index));
                let saddling = def.is_some_and(|(key, d)| crate::keyword::saddles(d, key.index));
                Self::PowerAtLeast { minimum: *minimum,
                    powers: among.iter().map(|o| (*o, if crewing || saddling { game.crew_power(*o, saddling) } else { game.power(*o).unwrap_or(0) })).collect() }
            }
            Self::ManaValueAtLeast { minimum, .. } => Self::ManaValueAtLeast { minimum: *minimum,
                values: among.iter().map(|o| (*o, game.obj(*o).chars.mana_value)).collect() },
            Self::Waterbend { generic, xs } => Self::Waterbend { generic: *generic, xs: *xs },
        }
    }
}

/// What blight puts counters on (701.68a): a creature you control.
fn blight_candidates() -> ir::Selector {
    static COST: std::sync::OnceLock<ir::Selector> = std::sync::OnceLock::new();
    COST.get_or_init(|| {
        let def = crate::CardDef::new("Blight rule", "{0}", "Instant", "As an additional cost to cast this spell, exile a creature you control.", None)
            .expect("the grammar reads a creature you control");
        let cost = def.abilities.iter().find_map(crate::functions::casting_cost).expect("an additional cost");
        let [CostPart::Action(action)] = cost.0.as_slice() else { panic!("one action: {cost:?}") };
        crate::activate::object_payment(action).expect("an exile").1.items[0].clone()
    }).clone()
}

/// What collect evidence exiles (701.59a): any number of cards from your graveyard.
pub(crate) fn evidence_cost() -> (Verb, ir::Selector) {
    static COST: std::sync::OnceLock<(Verb, ir::Selector)> = std::sync::OnceLock::new();
    COST.get_or_init(|| {
        let def = crate::CardDef::new("Collect evidence rule", "{0}", "Instant", "As an additional cost to cast this spell, exile any number of cards from your graveyard.", None)
            .expect("the grammar reads rule 701.59a's exile");
        let cost = def.abilities.iter().find_map(crate::functions::casting_cost).expect("an additional cost");
        let [CostPart::Action(action)] = cost.0.as_slice() else { panic!("one action: {cost:?}") };
        let (verb, objects) = crate::activate::object_payment(action).expect("an exile");
        (verb, objects.items[0].clone())
    }).clone()
}

/// What foraging is (701.61a): "Exile three cards from your graveyard or sacrifice a Food."
fn forage_cost() -> Cost {
    static COST: std::sync::OnceLock<Cost> = std::sync::OnceLock::new();
    COST.get_or_init(|| {
        let def = crate::CardDef::new("Forage rule", "{0}", "Instant", "As an additional cost to cast this spell, exile three cards from your graveyard or sacrifice a Food.", None)
            .expect("the grammar reads rule 701.61a's forage");
        def.abilities.iter().find_map(crate::functions::casting_cost).expect("an additional cost").clone()
    }).clone()
}

/// "Tap any number of untapped creatures you control with total power N or greater"
/// (702.122a): the candidates without the collective predicate, and N.
fn total_power(s: &ir::Selector) -> Option<(ir::Selector, u32)> {
    let ir::Filter::All(items) = &s.set.filter else { return None };
    let total = |f: &ir::Filter| match f {
        ir::Filter::Atom(ir::Atom::Total { stat: ir::selector::Stat::Power, cmp: ir::Cmp::Ge, value: Amount::Const(n) }) => Some(*n),
        _ => None,
    };
    let [minimum] = items.iter().filter_map(total).collect::<Vec<_>>()[..] else { return None };
    let mut rest = s.clone();
    rest.set.filter = ir::Filter::All(items.iter().filter(|f| total(f).is_none()).cloned().collect());
    Some((rest, minimum))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Part { Reveal, Mana(ManaCost), Tap, Untap, Energy(u32), Life(expr::Amount), /// `blight`: with [`Verb::PutCounters`], that many -1/-1 counters put on each (701.68a).
    Objects { verb: Verb, groups: Vec<Objects>, producer: Option<AbilityKey>, blight: u32, target_minimum: bool }, Loyalty(i32), RemoveCounters { kind: String, count: u32 } }
/// Selected tapping and a zone-changing cost together: the current object-choice protocol
/// requires all selected resources to be distinct, and these can reuse an object.
fn mixed(parts: &[Part]) -> bool {
    parts.iter().any(|p| matches!(p, Part::Objects { verb: Verb::Tap, .. }))
        && parts.iter().any(|p| matches!(p, Part::Objects { verb, .. } if *verb != Verb::Tap))
}

/// Whether an action of a cost is done to the object whose cost it is.
fn about_this(action: &Action) -> bool {
    let this = |s: &ir::Selector| s.quant == Quant::Definite(Ref::This);
    match action {
        Action::RemoveCounters { from: Some(from), .. } => this(from),
        _ => crate::activate::object_payment(action).is_some_and(|(_, objects)| objects.items.iter().any(this)),
    }
}

/// The action with its "it" the object whose cost it is: "remove three quest counters from
/// this enchantment and sacrifice it".
fn this_for_it(action: &Action) -> Action {
    let mut action = action.clone();
    if let Action::Sacrifice(objects) | Action::Discard(objects) | Action::Exile(objects) | Action::Tap(objects) = &mut action {
        for s in &mut objects.items {
            if s.quant == Quant::Definite(Ref::Anaphor(ir::Anaphor::It)) { s.quant = Quant::Definite(Ref::This); }
        }
    }
    action
}

/// Rule choice identity, independent of flattened execution alternatives.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag="kind", rename_all="snake_case", deny_unknown_fields)]
pub enum Branch {
    PrintedOr { arm: usize, #[serde(skip_serializing_if="Option::is_none", default)] forage: Option<ForagePayment> },
    Forage { payment: ForagePayment },
    HybridReduction { symbol: usize, half: HybridHalf },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HybridHalf { W, U, B, R, G, C, #[serde(rename="2")] Two }
impl HybridHalf {
    fn of(symbol:ManaSymbol) -> Self {
        match symbol {
            ManaSymbol::Color(ir::Color::White)=>Self::W, ManaSymbol::Color(ir::Color::Blue)=>Self::U,
            ManaSymbol::Color(ir::Color::Black)=>Self::B, ManaSymbol::Color(ir::Color::Red)=>Self::R,
            ManaSymbol::Color(ir::Color::Green)=>Self::G, ManaSymbol::Colorless=>Self::C,
            ManaSymbol::Generic(2)=>Self::Two, _=>unreachable!("a compiled hybrid reduction half"),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all="snake_case")]
pub enum ForagePayment { ExileThree, SacrificeFood }
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BranchSelection { pub ability: serde_json::Value, pub branch: Branch }
/// One retained announcement, with its display and immutable declaring definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedBranch {
    pub text: String,
    pub(crate) owner: Option<crate::binding::AbilityKey>,
    pub(crate) definition: Option<crate::ability::FrozenDefinition>,
    pub(crate) source: Option<crate::ObjId>,
    pub(crate) branch: Option<Branch>,
}
/// "Discard a card or pay 3 life": a cost that is a choice among alternatives, its payer
/// choosing one as the spell or ability is proposed (601.2b, 602.2b), before the total cost
/// is determined (601.2f). Each alternative is the parts paying it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Choice { owner: Option<crate::binding::AbilityKey>, branches: Vec<Branch>, among: Vec<Vec<Part>>,
    /// Each alternative as its text says it ("sacrifice a Food"), where one action of the text is
    /// it; none where that action is itself a choice and this is one of its ways.
    said: Vec<Option<String>> }

impl Part {
    /// The part said plainly, as a player is offered it: "pay {3}{B}", "pay 3 life",
    /// "sacrifice 1".
    fn said(&self, game: &Game, ctx: &Ctx) -> String {
        match self {
            Part::Reveal => "reveal this card".into(),
            Part::Mana(mana) => format!("pay {}", mana.0.iter().map(|s| format!("{{{}}}", s.text())).collect::<String>()),
            Part::Tap => "{T}".into(), Part::Untap => "{Q}".into(),
            Part::Energy(n) => format!("pay {n} energy"),
            Part::Life(n) => format!("pay {} life", n.evaluate(game, ctx, &NoLocals)),
            Part::Loyalty(n) => format!("{n:+} loyalty"),
            Part::RemoveCounters { kind, count } => format!("remove {count} {kind} counter(s)"),
            Part::Objects { verb: Verb::PutCounters, blight, .. } => format!("blight {blight}"),
            Part::Objects { verb, groups, .. } => format!("{} {}", verb.entry().word, groups.iter().map(|g| match g {
                Objects::Source(_) => "this".to_string(),
                Objects::Fixed(_) => "1".to_string(),
                Objects::Choose { selection: Selection::Exact(n), .. } => n.to_string(),
                Objects::Choose { .. } => "any number".to_string(),
            }).collect::<Vec<_>>().join(" and ")),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan { parts: Vec<Part>, generic_reduction: u32, minimum: u32, generic_substitution: u32, convoke_colors: [u32;5], convoke_groups: Vec<usize>, convoke_allocation: Vec<Option<ir::Color>>, delve_group: Option<usize>, changed: (u32, u32),
    /// Its choices among alternatives not yet made ([`Plan::chosen`]); none is paid until made.
    choices: Vec<Choice>,
    typed_reduction: [u32; 6],
    reduction_choices: Vec<Vec<ManaSymbol>>,
    reduction_owners: Vec<(crate::binding::AbilityKey, usize)>,
}
impl Plan {
    pub(crate) fn compile(cost: &Cost, support: &Support<'_>) -> Res<Self> {
        let mut parts = Vec::new();
        let mut choices = Vec::new();
        for part in &cost.0 {
            if let CostPart::Joined(joined) = part {
                let each = |action: &Action| Plan::compile(&Cost(vec![CostPart::Action(action.clone())]), support);
                match joined.conj {
                    // "X or Y": one of them, chosen as it is proposed (601.2b); an alternative
                    // that is itself a choice, as foraging is, is each of its ways
                    Conj::Or => {
                        let (mut among, mut said, mut branches) = (Vec::new(), Vec::new(), Vec::new());
                        for (arm, item) in joined.items.iter().enumerate() {
                            let plan = each(item)?;
                            let ways = plan.ways();
                            let text = (ways.len() == 1).then(|| grammar::action::action(grammar::event::Form::Base).print_text(item, false)).flatten();
                            for way in ways {
                                // a way of an alternative that is itself one choice is said as that choice says it
                                said.push(text.clone().or_else(|| match (plan.choices.as_slice(), way.as_slice()) { ([only], [i]) => only.said[*i].clone(), _ => None }));
                                let forage = match (plan.choices.as_slice(), way.as_slice()) { ([only], [i]) => match only.branches.get(*i) { Some(Branch::Forage { payment }) => Some(*payment), _ => None }, _ => None };
                                branches.push(Branch::PrintedOr { arm: arm + 1, forage });
                                among.push(plan.clone().chosen(&way).map_err(crate::Unsupported)?.parts);
                            }
                        }
                        choices.push(Choice { owner: support.ability_key(), branches, among, said });
                    }
                    // "Remove three quest counters from this enchantment and sacrifice it": each,
                    // in order (118.1); its "it" is this when what precedes it is
                    // "behold a Kithkin and exile it": the object beheld is the one exiled (701.4a)
                    Conj::And if matches!(joined.items.as_slice(), [Action::OnObjects(ir::action::ObjectAction::Behold, _), Action::Exile(it)]
                        if it.items.len() == 1 && it.items[0].quant == Quant::Definite(Ref::Anaphor(ir::Anaphor::It))) => {
                        let mut beheld = each(&joined.items[0])?.parts;
                        for part in &mut beheld { if let Part::Objects { verb, .. } = part { *verb = Verb::Exile; } }
                        parts.extend(beheld);
                    }
                    Conj::And => for (i, action) in joined.items.iter().enumerate() {
                        let it_is_this = joined.items[..i].iter().any(about_this);
                        parts.extend(each(&if it_is_this { this_for_it(action) } else { action.clone() })?.parts);
                    },
                    Conj::AndOr => return unsupported("cost: \"and/or\" among payments"),
                }
                continue;
            }
            // "waterbend [cost]": pay it, tapping an artifact or creature for each generic mana
            // in it rather than paying that mana (701.67a)
            if let CostPart::Action(Action::Waterbend(mana)) = part {
                if let Some(s) = mana.0.iter().find(|s| !matches!(s, ManaSymbol::Generic(_)) && **s != X) { return unsupported(format!("cost: waterbend {}", name(s))); }
                if parts.iter().any(|p: &Part| matches!(p, Part::Objects { groups, .. } if groups.iter().any(|g| matches!(g, Objects::Choose { selection: Selection::Waterbend { .. }, .. })))) {
                    return unsupported("cost: more than one waterbend cost");
                }
                let generic = mana.0.iter().map(|s| if let ManaSymbol::Generic(n) = s { *n } else { 0 }).sum();
                let xs = mana.0.iter().filter(|s| **s == X).count() as u32;
                let mut tap = crate::keyword::waterbend_cost().parts[0].clone();
                if let Part::Objects { groups, producer, .. } = &mut tap {
                    *producer = support.ability_key();
                    for group in groups { if let Objects::Choose { selection, .. } = group { *selection = Selection::Waterbend { generic, xs }; } }
                }
                parts.push(Part::Mana(mana.clone()));
                parts.push(tap);
                continue;
            }
            if let Some(mana) = part.mana() {
                // {X} is announced with the spell or ability (107.3a); {Y} and {Z} are not.
                // A Phyrexian symbol stays until its player says how it is paid (118.13a).
                if let Some(s) = mana.0.iter().find(|s| s.mana_payments().is_none() && **s != X && !phyrexian(s)) { return unsupported(format!("cost: {}", name(s))); }
                parts.push(Part::Mana(mana.clone())); continue;
            }
            parts.push(match part {
                CostPart::Tap => Part::Tap, CostPart::Untap => Part::Untap,
                // a loyalty symbol: that many counters put on or removed (606.4)
                CostPart::Loyalty(Loyalty::Zero) => Part::Loyalty(0),
                CostPart::Loyalty(Loyalty::Plus(Amount::Const(n))) => Part::Loyalty(i32::try_from(*n).map_err(|_| crate::Unsupported("cost: loyalty".into()))?),
                CostPart::Loyalty(Loyalty::Minus(Amount::Const(n))) => Part::Loyalty(-i32::try_from(*n).map_err(|_| crate::Unsupported("cost: loyalty".into()))?),
                CostPart::Loyalty(_) => return unsupported("cost: a loyalty cost of X"),
                CostPart::Action(Action::PayLife(Amount::X)) => Part::Life(expr::Amount::announced_x()),
                CostPart::RevealUntilLeavesStack(objects) if objects.items.len() == 1 && crate::functions::out_of(&objects.items[0])? == Some(crate::Zone::Hand) => Part::Reveal,
                CostPart::Action(Action::PayEnergy(Amount::Const(n))) => Part::Energy(*n),
                CostPart::Action(Action::PayEnergy(_)) => return unsupported("cost: energy requires a fixed amount"),
                CostPart::Action(Action::PayLife(n)) => Part::Life(expr::Amount::compile(n, support, &mut linked(support))?),
                CostPart::Action(Action::RemoveCounters { count: ir::CounterCount::Exactly(Amount::Const(count)), kind: Some(kind), from: Some(from), among: false })
                    if from.quant == Quant::Definite(Ref::This) && from.chooser.is_none() && from.random.is_none() && from.set.zone.is_none() => {
                        Part::RemoveCounters { kind: kind.0.clone(), count: *count }
                    },
                CostPart::Action(Action::RemoveCounters { .. }) => return unsupported("cost: removing counters requires a fixed count and kind from the source"),
                // "collect evidence N": exile any number of cards from your graveyard with total
                // mana value N or greater (701.59a)
                // "blight N": put N -1/-1 counters on a creature you control (701.68a); one that
                // can't have them put on it is no choice (701.68b)
                CostPart::Action(Action::Numbered(ir::NumberedAction::Blight, Amount::Const(n))) => {
                    Part::Objects { verb: Verb::PutCounters, groups: vec![Objects::Choose { selection: Selection::Exact(1),
                        among: Candidates::compile(Verb::PutCounters, &blight_candidates(), support)? }], producer: support.ability_key(), blight: *n, target_minimum: false }
                }
                // "forage" (701.61a): exile three cards from your graveyard or sacrifice a Food
                CostPart::Action(Action::Keyword(ir::action::KeywordAction::Forage)) => {
                    let mut forage = Plan::compile(&forage_cost(), support)?;
                    forage.choices[0].branches = vec![Branch::Forage { payment: ForagePayment::ExileThree }, Branch::Forage { payment: ForagePayment::SacrificeFood }];
                    parts.extend(forage.parts);
                    choices.extend(forage.choices);
                    continue;
                }
                // "behold a Dragon" (701.4a): one chosen, nothing done to it; "behold three
                // Elementals": three different ones, of the hand and the battlefield together
                CostPart::Action(Action::OnObjects(ir::action::ObjectAction::Behold, objects)) => {
                    let [s] = objects.items.as_slice() else { return unsupported("cost: behold more than one kind") };
                    let Quant::Indefinite(Count::Exactly(Amount::Const(n @ 1..))) = s.quant else { return unsupported("cost: behold other than a number") };
                    if s.chooser.is_some() || s.random.is_some() { return unsupported("cost: behold other than a number") }
                    Part::Objects { verb: Verb::Behold, groups: vec![Objects::Choose { selection: Selection::Exact(n as usize), among: Candidates::behold(s, support)? }],
                        producer: support.ability_key(), blight: 0, target_minimum: false }
                }
                // "reveal a Dinosaur card from your hand" (701.20a): chosen and shown to all
                // players, nothing else done to them
                CostPart::Action(Action::Reveal(objects)) => {
                    let [s] = objects.items.as_slice() else { return unsupported("cost: reveal more than one kind") };
                    let Quant::Indefinite(Count::Exactly(Amount::Const(n))) = s.quant else { return unsupported("cost: reveal other than a number of cards") };
                    if s.chooser.is_some() || s.random.is_some() || !s.set.zone.as_ref().is_some_and(|z| z.zones.items == [ir::Zone::Hand] && z.slice.is_none()
                        && z.owner.as_ref().is_some_and(|o| o.quant == Quant::Definite(Ref::You))) { return unsupported("cost: reveal other than cards from your hand") }
                    Part::Objects { verb: Verb::Reveal, groups: vec![Objects::Choose { selection: Selection::Exact(n as usize), among: Candidates::compile(Verb::Reveal, s, support)? }],
                        producer: support.ability_key(), blight: 0, target_minimum: false }
                }
                CostPart::DefinedX(Action::Numbered(ir::NumberedAction::CollectEvidence, Amount::X), Amount::TargetedPermanentManaValue) => {
                    let (_, objects) = evidence_cost();
                    Part::Objects { verb: Verb::CollectEvidence, groups: vec![Objects::Choose {
                        selection: Selection::ManaValueAtLeast { minimum: 0, values: Vec::new() },
                        among: Candidates::compile(Verb::Exile, &objects, support)? }], producer: support.ability_key(), blight: 0, target_minimum: true }
                }
                CostPart::Action(Action::Numbered(ir::NumberedAction::CollectEvidence, Amount::Const(n))) => {
                    let (_, objects) = evidence_cost();
                    let verb = Verb::CollectEvidence;
                    Part::Objects { verb, groups: vec![Objects::Choose { selection: Selection::ManaValueAtLeast { minimum: *n, values: Vec::new() },
                        among: Candidates::compile(verb, &objects, support)? }], producer: support.ability_key(), blight: 0, target_minimum: false }
                }
                CostPart::Action(action) | CostPart::WithCardTypes(action, _) => {
                    let Some((verb, objects)) = crate::activate::object_payment(action) else { return unsupported(format!("cost: {}", name(action))) };
                    if objects.items.len() > 1 && objects.conj != Conj::And { return unsupported("cost: a choice among kinds of object"); }
                    let constraint = match part { CostPart::WithCardTypes(_, minimum) => Some(*minimum), _ => None };
                    if constraint.is_some() && (objects.items.len() != 1 || verb != Verb::Exile) {
                        return unsupported("cost: a collective type constraint requires one exile selection");
                    }
                    let mut groups = Vec::new();
                    for s in &objects.items {
                        if s.chooser.is_some() || s.random.is_some() { return unsupported("cost: a chooser or a random pick"); }
                        groups.push(match (&s.quant, constraint) {
                            (Quant::Definite(Ref::This), None) => Objects::Source(if s.set.zone.is_some() { Some(Candidates::compile(verb, s, support)?) } else { None }),
                            (Quant::Indefinite(Count::Exactly(Amount::Const(n))), None) => Objects::Choose { selection: Selection::Exact(*n as usize), among: Candidates::compile(verb, s, support)? },
                            (Quant::Indefinite(Count::AnyNumber), Some(minimum)) => Objects::Choose { selection: Selection::CardTypesAtLeast { minimum, types: Vec::new() }, among: Candidates::compile(verb, s, support)? },
                            (Quant::Indefinite(Count::AnyNumber), None) if verb == Verb::Tap && total_power(s).is_some() => {
                                let (rest, minimum) = total_power(s).expect("checked");
                                Objects::Choose { selection: Selection::PowerAtLeast { minimum, powers: Vec::new() }, among: Candidates::compile(verb, &rest, support)? }
                            }
                            q => return unsupported(format!("cost: {} of \"{}\"", name(&verb), name(q.0))),
                        });
                    }
                    Part::Objects { verb, groups, producer: support.ability_key(), blight: 0, target_minimum: false }
                }
                other => return unsupported(format!("cost: {}", name(other))),
            });
        }
        // The current object-choice protocol requires all selected resources to be
        // distinct. Mixed tapping and zone-change costs can reuse an object; do not
        // silently admit them until choices describe independent resource kinds.
        if mixed(&parts) { return unsupported("cost: selected tapping combined with a zone-changing cost"); }
        if choices.len() > 1 { return unsupported("cost: more than one choice among alternatives"); }
        let plan = Self { parts, generic_reduction: 0, minimum: 0, generic_substitution: 0, convoke_colors: [0;5], convoke_groups: Vec::new(), convoke_allocation: Vec::new(), delve_group: None, changed: (0, 0), choices, typed_reduction: [0;6], reduction_choices: Vec::new(), reduction_owners: Vec::new() };
        plan.validate_selections()?;
        for way in plan.ways() { plan.clone().chosen(&way).map_err(crate::Unsupported)?; }
        Ok(plan)
    }
    /// Each way of making its choices among alternatives (601.2b): one alternative of each.
    pub(crate) fn ways(&self) -> Vec<Vec<usize>> {
        self.choices.iter().map(|choice| choice.among.len()).chain(self.reduction_choices.iter().map(Vec::len))
            .fold(vec![Vec::new()], |ways, count| ways.into_iter()
                .flat_map(|way| (0..count).map(move |i| [way.clone(), vec![i]].concat())).collect())
    }
    /// Whether its payer chooses among alternatives as it is proposed (601.2b).
    pub(crate) fn chooses(&self) -> bool { !self.choices.is_empty() || !self.reduction_choices.is_empty() }
    /// The cost with its choices among alternatives made, as its payer says them while
    /// proposing the spell or ability (601.2b, 602.2b): for each choice in order, the
    /// alternative paid. It is locked in with the rest of the cost (601.2f).
    pub(crate) fn chosen(mut self, chose: &[usize]) -> Result<Self, String> {
        let count = self.choices.len() + self.reduction_choices.len();
        if chose.len() != count {
            return Err(format!("that cost has {count} choice(s) among alternatives: say which alternative of each is paid or applied (601.2b, 118.7e)"));
        }
        let reductions = &chose[self.choices.len()..];
        for (choice, i) in std::mem::take(&mut self.choices).into_iter().zip(chose) {
            let Some(parts) = choice.among.get(*i) else {
                return Err(format!("that cost is a choice among {} alternatives, numbered from 0 (601.2b)", choice.among.len()));
            };
            self.parts.extend(parts.iter().cloned());
        }
        for (options, index) in std::mem::take(&mut self.reduction_choices).into_iter().zip(reductions) {
            let symbol = *options.get(*index).ok_or("that reduction choice names no offered hybrid half (118.7e)")?;
            self = self.reduced_symbol(symbol);
        }
        if mixed(&self.parts) { return Err("that alternative taps an object the rest of the cost moves (601.2b)".into()); }
        self.validate_selections().map_err(|u| u.0)?;
        Ok(self)
    }
    /// What each alternative of each of its choices pays, said plainly, as a player is offered
    /// them (601.2b).
    pub(crate) fn alternatives(&self, game: &Game, ctx: &Ctx) -> Vec<Vec<String>> {
        self.choices.iter().map(|choice| choice.among.iter().zip(&choice.said)
            .map(|(parts, said)| said.as_deref().map(|s| crate::prompt::named(s, &game.obj(ctx.object_source()).chars.name, &[])).unwrap_or_else(|| parts.iter().map(|p| p.said(game, ctx)).collect::<Vec<_>>().join(" and "))).collect())
            .chain(self.reduction_choices.iter().map(|options| options.iter().map(|s|
                format!("reduce the cost by {{{}}}", s.text())).collect())).collect()
    }
    /// The alternative of each of its choices that is chosen, said as [`Plan::alternatives`]
    /// says it: what its payer announces (601.2b). None for a choice not said.
    pub(crate) fn branch_choices(&self, game: &Game, ctx: &Ctx) -> Result<Vec<Vec<BranchSelection>>, String> {
        let mut rows = Vec::new();
        for choice in &self.choices {
            let owner=choice.owner.as_ref().ok_or("cost branch has no declaring ability")?;
            let mut owner=owner.clone();
            if let Some(key)=&ctx.ability {owner.origin=key.origin.clone();}
            if !matches!(owner.origin,crate::binding::Origin::Own) || game.frozen_definition(ctx.object_source(),&owner.origin).is_none_or(|definition|definition.printed_cost_definition().is_none())
                || ctx.casting.is_some_and(|(_,cast)|cast!=crate::card::CastAs::Whole) {
                return Err("cost branch owner requires a whole printed definition".into());
            }
            let ability=game.audit_ability(Some(ctx.object_source()), &owner);
            rows.push(choice.branches.iter().cloned().map(|branch|BranchSelection { ability: ability.clone(), branch }).collect());
        }
        for ((owner, occurrence), options) in self.reduction_owners.iter().zip(&self.reduction_choices) {
            let mut owner=owner.clone();
            if let Some(key)=&ctx.ability {owner.origin=key.origin.clone();}
            if !matches!(owner.origin,crate::binding::Origin::Own) || game.frozen_definition(ctx.object_source(),&owner.origin).is_none_or(|definition|definition.printed_cost_definition().is_none())
                || ctx.casting.is_some_and(|(_,cast)|cast!=crate::card::CastAs::Whole) {
                return Err("cost branch owner requires a whole printed definition".into());
            }
            let ability=game.audit_ability(Some(ctx.object_source()), &owner);
            rows.push(options.iter().map(|half|BranchSelection { ability:ability.clone(), branch:Branch::HybridReduction { symbol:*occurrence, half:HybridHalf::of(*half) } }).collect());
        }
        Ok(rows)
    }
    pub(crate) fn resolve_branches(&self, game:&Game, ctx:&Ctx, ordinal:&[usize], semantic:Option<&[BranchSelection]>) -> Result<Vec<usize>, String> {
        let Some(selected)=semantic else { return Ok(ordinal.to_vec()) };
        if !ordinal.is_empty() {return Err("cannot combine semantic and ordinal cost choices".into());}
        let sites=self.branch_choices(game,ctx)?;
        if selected.len()!=sites.len() {return Err("missing or foreign cost branch selection".into());}
        let mut used=vec![false;selected.len()];
        let mut indices=Vec::new();
        for site in sites {
            let found:Vec<_>=selected.iter().enumerate().flat_map(|(i,row)|site.iter().enumerate().filter_map(move |(j,offered)|(row==offered).then_some((i,j)))).collect();
            let [(i,j)]=found.as_slice() else {return Err("cost branch must match exactly one actual choice site".into())};
            if used[*i] {return Err("duplicate or ambiguous cost branch owner".into());} used[*i]=true; indices.push(*j);
        }
        Ok(indices)
    }
    pub(crate) fn said_chosen(&self, game: &Game, ctx: &Ctx, chose: &[usize]) -> Vec<SelectedBranch> {
        let descriptors=self.choices.iter().map(|choice|(choice.owner.clone(),choice.branches.clone()))
            .chain(self.reduction_owners.iter().zip(&self.reduction_choices).map(|((owner,symbol),options)|(Some(owner.clone()),options.iter().map(|half|Branch::HybridReduction { symbol:*symbol, half:HybridHalf::of(*half) }).collect())));
        self.alternatives(game, ctx).into_iter().zip(descriptors).zip(chose).filter_map(|((said,(owner,branches)),i)| {
            let text=said.get(*i)?.clone();
            let owner=owner.map(|mut owner| {if let Some(key)=&ctx.ability {owner.origin=key.origin.clone();} owner});
            let definition=owner.as_ref().and_then(|owner|game.frozen_definition(ctx.object_source(),&owner.origin))
                .filter(|_|ctx.casting.is_none_or(|(_,cast)|cast==crate::card::CastAs::Whole));
            Some(SelectedBranch { text, owner, definition, source: ctx.ability.as_ref().map(|_|ctx.object_source()), branch:branches.get(*i).cloned() })
        }).collect()
    }
    /// Add another plan's parts and its choices among alternatives.
    fn join(&mut self, other: Plan) {
        self.parts.extend(other.parts);
        self.choices.extend(other.choices);
        self.reduction_choices.extend(other.reduction_choices);
        self.reduction_owners.extend(other.reduction_owners);
    }
    fn validate_selections(&self) -> Res<()> {
        let (count, collective) = self.parts.iter().flat_map(|p| match p { Part::Objects {groups,..} => groups.as_slice(), _ => &[] })
            .fold((0, false), |(n, any), group| (n + 1, any || matches!(group, Objects::Choose { selection: Selection::CardTypesAtLeast {..} | Selection::PowerAtLeast {..} | Selection::ManaValueAtLeast {..}, .. })));
        if count > 1 && collective {
            return unsupported("cost: a collective type selection combined with another object cost");
        }
        Ok(())
    }
    /// How many mana or life amounts announce X (107.3a).
    pub(crate) fn x(&self) -> usize {
        self.parts.iter().map(|p| match p { Part::Mana(mana) => mana.0.iter().filter(|s| **s == X).count(), Part::Life(n) if n.is_x() => 1, _ => 0 }).sum()
    }
    /// The cost with the announced value in place of each {X}; it is locked in from then on
    /// (601.2b, 601.2f). A cost with {X} needs a value, and no other cost takes one.
    pub(crate) fn announced(mut self, x: Option<u32>) -> Result<Self, String> {
        match (self.x(), x) {
            (0, None) => Ok(self),
            (0, Some(_)) => Err("that cost has no {X} whose value is announced (107.3a)".into()),
            (_, None) => Err("that cost has {X}: announce the value of X (107.3a, 601.2b)".into()),
            (_, Some(x)) => {
                for part in &mut self.parts {
                    match part {
                        Part::Mana(mana) => for s in &mut mana.0 { if *s == X { *s = ManaSymbol::Generic(x); } },
                        Part::Life(n) if n.is_x() => *n = expr::Amount::constant(x),
                        Part::Objects { groups, .. } => for group in groups {
                            if let Objects::Choose { selection: Selection::Waterbend { generic, xs }, .. } = group { *generic += *xs * x; *xs = 0; }
                        },
                        _ => {},
                    }
                }
                Ok(self)
            }
        }
    }
    /// The cost with a kicker cost its payer declared they would pay this many times: an
    /// additional cost, locked in with the rest (601.2b, 601.2f, 702.33a).
    pub(crate) fn kicked(mut self, kicker: &ManaCost, times: u32) -> Self {
        self.parts.extend((0..times).map(|_| Part::Mana(kicker.clone())));
        self
    }
    /// Add a permission's already compiled additional cost before cost adjustment
    /// and locking (601.2f). It applies to alternative costs too (118.9d).
    pub(crate) fn plus(mut self, additional: &Plan) -> Self {
        self.join(additional.clone());
        self
    }
    /// All the mana of this cost.
    pub(crate) fn mana(&self) -> ManaCost {
        ManaCost(self.parts.iter().flat_map(|p| if let Part::Mana(mana) = p { mana.0.clone() } else { Vec::new() }).collect())
    }
    /// The life the cost has its payer pay (119.4), as the game now is; `None` past what the
    /// engine counts in, which is more than any life total it holds, never wrapped or clamped.
    pub(crate) fn life(&self, game: &Game, ctx: &Ctx) -> Option<u32> {
        self.parts.iter().filter_map(|p| if let Part::Life(n) = p { Some(n.checked(game, ctx, &NoLocals).ok()) } else { None })
            .try_fold(0u32, |sum, life| sum.checked_add(life?))
    }
    /// The Phyrexian mana symbols of the cost, in the order it says them (107.4f).
    pub(crate) fn phyrexian(&self) -> Vec<ManaSymbol> {
        self.parts.iter().flat_map(|p| if let Part::Mana(mana) = p { mana.0.iter().copied().filter(phyrexian).collect() } else { Vec::new() }).collect()
    }
    /// The cost with the way each Phyrexian symbol is paid locked in, as its player says
    /// while proposing the spell or ability (118.13a, 601.2b): for each such symbol in
    /// order, whether 2 life is paid for it rather than mana of its color (107.4f). Saying
    /// nothing pays mana for all of them.
    pub(crate) fn paid_with_life(mut self, life: &[bool]) -> Result<Self, String> {
        let count = self.phyrexian().len();
        if life.is_empty() && count == 0 { return Ok(self); }
        if !life.is_empty() && life.len() != count {
            return Err(format!("that cost has {count} Phyrexian mana symbols: say for each whether life is paid (107.4f, 601.2b)"));
        }
        let mut said = life.iter().copied();
        let mut paid = 0;
        for part in &mut self.parts {
            let Part::Mana(mana) = part else { continue };
            let mut symbols = Vec::new();
            for s in mana.0.drain(..) {
                match s {
                    ManaSymbol::Phyrexian(_) | ManaSymbol::HybridPhyrexian(..) if said.next().unwrap_or(false) => paid += 2,
                    ManaSymbol::Phyrexian(c) => symbols.push(ManaSymbol::Color(c)),
                    ManaSymbol::HybridPhyrexian(a, b) => symbols.push(ManaSymbol::Hybrid(a, b)),
                    other => symbols.push(other),
                }
            }
            mana.0 = symbols;
        }
        if paid > 0 { self.parts.push(Part::Life(expr::Amount::constant(paid))); }
        Ok(self)
    }
    /// Reduction by a mana cost (118.7, 702.193b). Hybrid halves are explicit announcement
    /// alternatives; typed excess is applied only after activation hybrid symbols choose halves.
    pub(crate) fn reduced_by(mut self, cost: &ManaCost, owner: crate::binding::AbilityKey) -> Self {
        let mut occurrence = 0;
        for symbol in &cost.0 {
            if matches!(symbol, ManaSymbol::Hybrid(..) | ManaSymbol::TwoHybrid(..) | ManaSymbol::ColorlessHybrid(..) | ManaSymbol::HybridPhyrexian(..)) {
                occurrence += 1; self.reduction_owners.push((owner.clone(), occurrence));
            }
            match symbol {
                ManaSymbol::Hybrid(a, b) => self.reduction_choices.push(vec![ManaSymbol::Color(*a), ManaSymbol::Color(*b)]),
                ManaSymbol::TwoHybrid(c) => self.reduction_choices.push(vec![ManaSymbol::Generic(2), ManaSymbol::Color(*c)]),
                ManaSymbol::ColorlessHybrid(c) => self.reduction_choices.push(vec![ManaSymbol::Colorless, ManaSymbol::Color(*c)]),
                // Phyrexian reductions use their color, not a life payment (118.7f).
                ManaSymbol::Phyrexian(c) => self = self.reduced_symbol(ManaSymbol::Color(*c)),
                ManaSymbol::HybridPhyrexian(a, b) => self.reduction_choices.push(vec![ManaSymbol::Color(*a), ManaSymbol::Color(*b)]),
                ManaSymbol::Snow => self = self.reduced_symbol(ManaSymbol::Generic(1)),
                ManaSymbol::Var(_) => {}, // a permanent's X is zero (107.3g)
                symbol => self = self.reduced_symbol(*symbol),
            }
        }
        self
    }
    fn reduced_symbol(mut self, symbol: ManaSymbol) -> Self {
        match symbol {
            ManaSymbol::Generic(n) => self = self.change_generic(0, n),
            ManaSymbol::Color(c) => self.typed_reduction[c as usize] += 1,
            ManaSymbol::Colorless => self.typed_reduction[5] += 1,
            _ => unreachable!("a reduction alternative is nonhybrid mana"),
        }
        self
    }
    /// All increases precede all reductions (601.2f). Generic reductions never consume
    /// colored or colorless requirements, and cannot make a cost negative (118.7a).
    pub(crate) fn change_generic(mut self, increase: u32, reduction: u32) -> Self {
        if increase == 0 && reduction == 0 { return self; }
        let mut generic = increase;
        for part in &mut self.parts {
            if let Part::Mana(mana) = part {
                mana.0.retain(|symbol| match symbol {
                    ManaSymbol::Generic(n) => { generic += *n; false },
                    _ => true,
                });
            }
        }
        // what was added and what was taken off: a reduction takes only generic mana (118.7a)
        self.changed = (self.changed.0 + increase, self.changed.1 + reduction.min(generic));
        self.parts.push(Part::Mana(ManaCost(vec![ManaSymbol::Generic(generic.saturating_sub(reduction))])));
        // A {2/W} may still contribute generic mana after choosing how to pay it.
        self.generic_reduction += reduction.saturating_sub(generic);
        self
    }
    /// The generic mana effects added to and took from this cost (601.2f), as a player is
    /// told when the total cannot be paid: "{1} more", "{2} less".
    pub(crate) fn changes(&self) -> String {
        let (more, less) = self.changed;
        [(more, "more"), (less, "less")].iter().filter(|(n, _)| *n > 0)
            .map(|(n, word)| format!("{{{n}}} {word}")).collect::<Vec<_>>().join(" and ")
    }
    /// The cost with this creature tapped as it is paid, which its payer chose as they
    /// announced the spell (601.2b, 702.180b).
    pub(crate) fn tapping(mut self, creature: ObjId) -> Self {
        self.parts.push(Part::Objects { verb: Verb::Tap, groups: vec![Objects::Fixed(creature)], producer: None, blight: 0, target_minimum: false });
        self
    }
    /// Direct total-cost effects are applied last and locked before mana production.
    pub(crate) fn minimum(mut self, amount: u32) -> Self { self.minimum = self.minimum.max(amount); self }
    pub(crate) fn requirement(&self, symbols: ManaCost) -> crate::mana::ReducedCost {
        crate::mana::ReducedCost { symbols, reduction: self.generic_reduction, typed_reduction: self.typed_reduction, minimum: self.minimum, substitution: self.generic_substitution, convoke: self.convoke_colors }
    }
    /// Delve is paid only after reductions and cost floors (702.66b). The mana
    /// solver substitutes generic payment after choosing its nonhybrid equivalent.
    pub(crate) fn delved(mut self, count: u32, producer: Option<AbilityKey>) -> Self {
        self.generic_substitution = count;
        if count > 0 {
            self.delve_group = Some(self.object_choice_count());
            let mut exile = crate::keyword::delve_cost().clone();
            for part in &mut exile.parts {
                if let Part::Objects { groups, producer: link, .. } = part {
                    *link = producer.clone();
                    for group in groups { if let Objects::Choose { selection, .. } = group { *selection = Selection::Exact(count as usize); } }
                }
            }
            self.parts.extend(exile.parts);
        }
        self
    }
    /// One tapping resource substitutes for one locked colored/generic requirement.
    pub(crate) fn has_zone_cost(&self)->bool {self.parts.iter().any(|p|matches!(p,Part::Objects{verb,..} if *verb!=Verb::Tap))}
    pub(crate) fn convoked(mut self, allocation: &[Option<ir::Color>]) -> Result<Self,String> {
        if !allocation.is_empty() && self.has_zone_cost() {return Err("convoke combined with a zone-changing object cost requires independent resource kinds".into());}
        for color in allocation {
            let index = self.object_choice_count();
            self.convoke_groups.push(index);
            self.convoke_allocation.push(*color);
            self.parts.extend(crate::keyword::convoke_cost(*color).parts.iter().cloned());
            if let Some(color)=color { self.convoke_colors[*color as usize]+=1; }
            else { self.generic_substitution+=1; }
        }
        Ok(self)
    }
    fn object_choice_count(&self) -> usize {
        self.parts.iter().map(|p| match p { Part::Objects { groups, .. } => groups.iter().filter(|g| matches!(g, Objects::Choose { .. })).count(), _ => 0 }).sum()
    }
    /// Read only after the owning payment has validated and committed every group.
    pub(crate) fn cast_payment(&self, paying: &[Vec<ObjId>]) -> crate::state::CastPayment {
        crate::state::CastPayment {
            convoke: self.convoke_groups.iter().zip(&self.convoke_allocation)
                .flat_map(|(index, color)| paying[*index].iter().map(move |object| (*object, *color))).collect(),
            delve: self.delve_group.map(|index| paying[index].clone()).unwrap_or_default(),
        }
    }
    /// The choice index of its waterbend cost's tapping, and the most it may tap (701.67a).
    pub(crate) fn waterbend(&self) -> Option<(usize, u32)> {
        self.parts.iter().flat_map(|p| match p { Part::Objects { groups, .. } => groups.as_slice(), _ => &[] })
            .filter(|g| matches!(g, Objects::Choose { .. })).enumerate()
            .find_map(|(i, g)| match g { Objects::Choose { selection: Selection::Waterbend { generic, .. }, .. } => Some((i, *generic)), _ => None })
    }
    /// The cost with this many artifacts and creatures tapped for its waterbend cost, each
    /// paying {1} of the generic mana in it and of no other (701.67a, 701.67b).
    pub(crate) fn waterbent(mut self, count: u32) -> Result<Self, String> {
        let Some((_, most)) = self.waterbend() else { return if count == 0 { Ok(self) } else { Err("that cost has no waterbend cost (701.67)".into()) } };
        if count > most { return Err(format!("a waterbend cost taps at most one artifact or creature for each generic mana in it, {most}: {count} chosen (701.67a, 701.67b)")); }
        for part in &mut self.parts {
            if let Part::Objects { groups, .. } = part {
                for group in groups { if let Objects::Choose { selection: selection @ Selection::Waterbend { .. }, .. } = group { *selection = Selection::Exact(count as usize); } }
            }
        }
        self.generic_substitution += count;
        Ok(self)
    }
    /// The cost as these choices pay its waterbend cost: as many tapped as were chosen.
    pub(crate) fn bent(self, paying: &[Vec<ObjId>]) -> Result<Self, String> {
        match self.waterbend() {
            Some((index, _)) => { let count = paying.get(index).map_or(0, |c| c.len() as u32); self.waterbent(count) }
            None => Ok(self),
        }
    }
    pub(crate) fn convokers(&self, paying: &[Vec<ObjId>]) -> Vec<crate::Target> {
        self.convoke_groups.iter().flat_map(|i|paying[*i].iter().copied().map(crate::Target::Object)).collect()
    }
    pub(crate) fn max_delve(&self) -> u32 {
        self.mana().0.iter().map(|s| match s {
            ManaSymbol::Generic(n) => *n,
            _ => s.mana_payments().map(|(a,b)| std::iter::once(a).chain(b).map(|s| if let ManaSymbol::Generic(n)=s {n} else {0}).max().unwrap_or(0)).unwrap_or(0),
        }).sum::<u32>().saturating_sub(self.generic_reduction).max(self.minimum)
    }

    /// A spell's cost when it is cast without paying its mana cost, which is said first:
    /// its additional costs remain (118.9d).
    pub(crate) fn with_life(mut self,n:u32)->Self {self.parts.push(Part::Life(expr::Amount::constant(n)));self}
    pub(crate) fn without_mana_cost(mut self) -> Self {
        if matches!(self.parts.first(), Some(Part::Mana(_))) { self.parts.remove(0); }
        self
    }
    pub(crate) fn has_mana(&self) -> bool { self.parts.iter().any(|p| matches!(p, Part::Mana(_))) }
    /// A loyalty symbol in its cost makes an ability a loyalty ability (606.2).
    pub(crate) fn loyalty(&self) -> bool { self.parts.iter().any(|p| matches!(p, Part::Loyalty(_))) }
    pub(crate) fn taps(&self) -> bool { self.parts.contains(&Part::Tap) }
    /// Lock the target-derived minimum after target announcement (601.2c, 601.2f).
    pub(crate) fn with_targets(mut self, game: &Game, targets: &[Vec<Target>]) -> Self {
        let mut objects = Vec::new();
        for target in targets.iter().flatten() {
            if let Target::Object(o) = target && game.obj(*o).zone == crate::Zone::Battlefield && !objects.contains(o) { objects.push(*o); }
        }
        let total = objects.iter().map(|o| game.obj(*o).chars.mana_value).fold(0u32, u32::saturating_add);
        for part in &mut self.parts {
            if let Part::Objects { groups, target_minimum: true, .. } = part {
                for group in groups { if let Objects::Choose { selection: Selection::ManaValueAtLeast { minimum, .. }, .. } = group { *minimum = total; } }
                if let Part::Objects { target_minimum, .. } = part { *target_minimum = false; }
            }
        }
        self
    }

    pub(crate) fn selected_tap(&self) -> bool { self.parts.iter().any(|p| matches!(p, Part::Objects { verb: Verb::Tap, .. })) }
    pub(crate) fn reveals_source(&self) -> bool { self.parts.contains(&Part::Reveal) }
    pub(crate) fn payments(&self, game: &Game, ctx: &Ctx, chosen: Option<&[Vec<ObjId>]>) -> Result<(Vec<Payment>, usize), String> {
        self.validate_selections().map_err(|e| e.0)?;
        if self.chooses() { return Err("say which alternative of that cost is paid (601.2b)".into()); }
        let mut payments = Vec::new();
        let mut choices = 0;
        for part in &self.parts {
            payments.push(match part {
                Part::Reveal => {
                    if game.obj(ctx.object_source()).zone != crate::Zone::Hand || game.obj(ctx.object_source()).owner != ctx.controller { return Err("the revealed source must be in your hand (702.49a)".into()); }
                    Payment::Reveal(ctx.object_source())
                },
                Part::Mana(mana) => Payment::Mana(mana.clone()), Part::Tap => Payment::Tap(ctx.object_source()), Part::Untap => Payment::Untap(ctx.object_source()),
                Part::Energy(n) => Payment::RemoveCounters { on: Target::Player(ctx.controller), kind: "energy".into(), count: *n },
                // more life than any life total the engine holds can't be paid (119.4)
                Part::Life(n) => Payment::Life(n.checked(game, ctx, &NoLocals)
                    .map_err(|_| format!("you can't pay more life than your life total of {} (119.4)", game.players[ctx.controller].life))?),
                Part::Loyalty(n) => Payment::Loyalty(ctx.object_source(), *n),
                Part::RemoveCounters { kind, count } => Payment::RemoveCounters { on: Target::Object(ctx.object_source()), kind: kind.clone(), count: *count },
                Part::Objects { verb, groups, producer, blight, .. } => {
                    let mut objects = Vec::new();
                    for group in groups {
                        match group {
                            Objects::Source(among) => {
                                if chosen.is_some() && among.as_ref().is_some_and(|a| !a.among(game, ctx, *verb, *blight).contains(&ctx.object_source())) { return Err("this card is not in the zone its cost requires (118.3)".into()); }
                                objects.push(ctx.object_source());
                            }
                            Objects::Choose { selection, among } => {
                                let selected = chosen.map(|c| c.get(choices).cloned().unwrap_or_default()); choices += 1;
                                let Some(selected) = selected else { continue };
                                let legal = among.among(game, ctx, *verb, *blight);
                                if selected.iter().enumerate().any(|(i, o)| selected[..i].contains(o)) {
                                    return Err("a card is chosen once for that cost (601.2h)".into());
                                }
                                // each card first, then what they must be together: a card the cost can't
                                // take is named, rather than counted as missing from the whole
                                for o in &selected {
                                    if !legal.contains(o) { return Err(unfit(game, ctx, *verb, *o)); }
                                }
                                let evaluated = selection.evaluated(game, ctx, &legal);
                                if !evaluated.accepts(&selected) {
                                    let rule = verb.entry().rule;
                                    if returns_attacker(game, ctx, *verb) { return Err(returned_count(game, selected.len())); }
                                    return Err(match evaluated {
                                        Selection::Exact(n) => format!("that selection does not satisfy the cost, which asks for exactly {n} {}: {} chosen (601.2h, {rule})", if !legal.is_empty() && legal.iter().all(|o| game.obj(*o).zone == crate::Zone::Battlefield) { "permanent(s)" } else { "card(s)" }, selected.len()),
                                        Selection::UpTo(n) => format!("that selection exceeds the maximum of {n}: {} chosen (601.2h)", selected.len()),
                                        Selection::CardTypesAtLeast { minimum, .. } => format!("that selection does not satisfy the cost: the chosen cards have fewer than {minimum} card types among them (601.2h, {rule})"),
                                        Selection::PowerAtLeast { minimum, .. } => format!("that selection does not satisfy the cost: the chosen creatures have total power less than {minimum} (601.2h)"),
                                        Selection::ManaValueAtLeast { minimum, .. } => format!("that selection does not satisfy the cost: the chosen cards have total mana value less than {minimum} (601.2h, 701.59a)"),
                                        Selection::Waterbend { generic, .. } => format!("a waterbend cost taps at most one artifact or creature for each generic mana in it, {generic}: {} chosen (701.67a, 701.67b)", selected.len()),
                                    });
                                }
                                objects.extend(selected);
                            }
                            Objects::Fixed(o) => objects.push(*o),
                        }
                    }
                    match (verb, objects.as_slice()) {
                        // as announced with no choice yet: nothing to put counters on
                        (Verb::PutCounters, [on]) => Payment::Blight { on: *on, count: *blight },
                        _ => Payment::Objects { verb: *verb, objects, producer: producer.clone() },
                    }
                }
            });
        }
        Ok((payments, choices))
    }
    pub(crate) fn choices(&self, game: &Game, ctx: &Ctx, spent: &[ObjId]) -> Vec<(Vec<ObjId>, Selection)> {
        self.parts.iter().filter_map(|p| if let Part::Objects { verb, groups, blight, .. } = p { Some((verb, groups, blight)) } else { None })
            .flat_map(|(verb, groups, blight)| groups.iter().filter_map(move |g| if let Objects::Choose { selection, among } = g {
                let among: Vec<_> = among.among(game, ctx, *verb, *blight).into_iter().filter(|o| !spent.contains(o)).collect();
                let selection = selection.evaluated(game, ctx, &among);
                Some((among, selection))
            } else { None })).collect()
    }
}

/// Why a chosen object cannot pay a cost's action. Ninjutsu's return says what its rule asks
/// for (702.49a); an attacking creature is neither blocked nor unblocked until blockers are
/// declared (509.1h), so before then none can be returned for it.
fn unfit(game: &Game, ctx: &Ctx, verb: Verb, o: ObjId) -> String {
    let name = &game.obj(o).chars.name;
    if !returns_attacker(game, ctx, verb) { return format!("{name} is not what that cost asks for or cannot pay it ({}, 118.3)", verb.entry().rule); }
    if game.combat.attacking(o) && !game.combat.blockers_declared {
        return format!("{name} is not an unblocked attacking creature you control: no attacking creature is unblocked until blockers are declared (702.49a, 509.1h)");
    }
    format!("{name} is not an unblocked attacking creature you control (702.49a)")
}

/// Why ninjutsu's return is paid with the wrong number of creatures: it returns one unblocked
/// attacking creature (702.49a), and before blockers are declared there is none (509.1h).
fn returned_count(game: &Game, chosen: usize) -> String {
    let asked = format!("ninjutsu's cost returns one unblocked attacking creature you control: {chosen} chosen");
    if game.combat.blockers_declared { format!("{asked} (702.49a)") }
    else { format!("{asked}; no attacking creature is unblocked until blockers are declared (702.49a, 509.1h)") }
}

/// Is `verb` the return of a ninjutsu ability's cost (702.49a)?
fn returns_attacker(game: &Game, ctx: &Ctx, verb: Verb) -> bool {
    verb == Verb::Move && ctx.ability.as_ref().zip(ctx.source)
        .and_then(|(key, source)| Some((key, game.definition(source, &key.origin)?)))
        .is_some_and(|(key, d)| crate::keyword::ninjutsu(d, key.index))
}

/// A printed alternative cost and the condition allowing its announcement (118.9,
/// 601.2b). The condition is checked before the cost is locked, not during payment.
#[derive(Debug, Clone)]
pub(crate) struct Alternative {
    pub(crate) condition: Option<crate::program::condition::Condition>,
    pub(crate) plan: Plan,
}

/// Definition-time results, including refusals; no sample Game is required for admission.
#[derive(Debug, Clone)]
pub(crate) struct Definitions {
    pub(crate) normal: Option<Res<Plan>>, pub(crate) flashback: Option<Res<Plan>>, pub(crate) escape: Option<Res<Plan>>, pub(crate) harmonize: Option<Res<Plan>>,
    /// Its foretell cost, an alternative cost of a foretold card cast from exile (702.143a).
    pub(crate) foretell: Option<Res<Plan>>,
    /// Each printed alternative cost (118.9) by its row, with the additional costs.
    pub(crate) alternatives: Vec<(usize, Res<Alternative>)>,
    /// Each alternative cost its card states that it is cast for only when an ability offers
    /// that cast, by its row, with the additional costs: its madness cost (702.35a, 702.35b).
    pub(crate) offered: Vec<(usize, Res<Plan>)>,
    /// Each morph or disguise ability by its row: the card may be cast face down by paying
    /// {3} rather than its mana cost (702.37c, 702.168b). The face-down spell has no text,
    /// so no additional cost the card prints is part of it.
    pub(crate) face_down: Vec<(usize, crate::face::Concealment, Res<Plan>)>,
    pub(crate) additional: Res<Plan>, abilities: Vec<Option<Res<Plan>>>,
    /// Each optional additional cost (118.8b), by the row stating it: added to the cost it is
    /// cast for when its controller announces it paid (601.2b, 601.2f).
    pub(crate) optional: Vec<(usize, crate::keyword::Optional, Res<Plan>)>,
}
impl Default for Definitions {
    fn default() -> Self { Self { normal: None, flashback: None, escape: None, harmonize: None, foretell: None, alternatives: Vec::new(), offered: Vec::new(), face_down: Vec::new(), additional: Ok(Plan::default()), abilities: Vec::new(), optional: Vec::new() } }
}
impl Definitions {
    /// The plan of an alternative cost an effect gives the card (613.1f), with the additional
    /// costs it prints, as one it prints is made.
    pub(crate) fn given(&self, def: &CardDef, cost: &Cost) -> Res<Plan> {
        let mut plan = Plan::compile(cost, &Support::definition(def, None))?;
        plan.join(self.additional.clone()?); plan.validate_selections()?; Ok(plan)
    }
    pub(crate) fn compile(def: &CardDef) -> Self {
        let additional = (|| {
            let mut plan = Plan::default();
            for (i, cost) in def.casting_costs() {
                if cost.0.iter().any(|p| matches!(p, CostPart::Action(Action::PayLife(n)) if !matches!(n, Amount::Const(_) | Amount::X))) {
                    return unsupported("additional cost: a variable amount must be locked before mana production (601.2f)");
                }
                plan.join(Plan::compile(cost, &Support::definition(def, Some(AbilityKey::printed(i))))?);
            }
            Ok(plan)
        })();
        // a cost the spell is cast for (its mana cost, or an alternative cost such as its
        // flashback cost, 118.9), and its additional costs (601.2f)
        let spell_with = |cost: &Cost, owner:Option<AbilityKey>| {
            if cost.0.iter().any(|p| matches!(p, CostPart::Action(Action::PayLife(n)) if !matches!(n, Amount::Const(_) | Amount::X))) {
                return unsupported("casting cost: a variable life amount must be announced and locked before mana production (601.2f)");
            }
            let mut plan = Plan::compile(cost, &Support::definition(def, owner))?;
            plan.join(additional.clone()?); plan.validate_selections()?; Ok(plan)
        };
        let spell=|cost:&Cost|spell_with(cost,None);
        Self {
            normal: def.cost.as_ref().map(|mana| spell(&Cost(vec![CostPart::Mana(mana.clone())]))),
            flashback: crate::keyword::flashback(def).as_ref().map(spell),
            escape: crate::keyword::escape(def).as_ref().map(spell),
            harmonize: crate::keyword::harmonize(def).as_ref().map(spell),
            foretell: crate::keyword::of_items(def).find_map(crate::foretell::cost).as_ref().map(spell),
            alternatives: def.alternative_costs().map(|(row, cost, condition)| {
                let support = Support::definition(def, Some(AbilityKey::printed(row)));
                (row, (|| Ok(Alternative {
                    condition: condition.map(|c| crate::program::condition::Condition::compile_cast_choice(c, &support, &mut linked(&support))).transpose()?,
                    plan: spell_with(cost,Some(AbilityKey::printed(row)))?,
                }))())
            }).collect(),
            offered: crate::keyword::madness(def).into_iter().map(|(row, cost, _)| (row, spell(&cost))).collect(),
            face_down: crate::keyword::face_down_casts(def).map(|(row, by, _, _)| {
                let three = Cost(vec![CostPart::Mana(ir::mana::ManaCost(vec![ir::mana::ManaSymbol::Generic(3)]))]);
                (row, by, Plan::compile(&three, &Support::definition(crate::face::definition(by), None)))
            }).collect(),
            abilities: def.abilities.iter().chain(def.keyword_abilities.iter().map(|(_, a)| a)).enumerate().map(|(i, a)| match a {
                Ability::Activated { cost, .. } => Some(Plan::compile(cost, &Support::definition(def, Some(AbilityKey::printed(i))))), _ => None,
            }).collect(), additional,
            optional: crate::keyword::optional_costs(def).map(|(row, kind, cost)| (row, kind, (|| {
                let plan = Plan::compile(cost, &Support::definition(def, Some(AbilityKey::printed(row))))?;
                plan.validate_selections()?;
                Ok(plan)
            })())).collect(),
        }
    }
}
impl CardDef {
    pub(crate) fn ability_cost(&self, index: usize) -> Option<&Res<Plan>> { self.costs.abilities.get(index)?.as_ref() }
}
impl Game {
    pub(crate) fn ability_cost(&self, source: ObjId, key: crate::binding::AbilityKey) -> Res<Plan> {
        self.definition(source, &key.origin).and_then(|d| d.ability_cost(key.index)).cloned()
            .unwrap_or_else(|| unsupported("that is not an activated ability of that permanent (602.1c)"))
    }
}

#[cfg(test)]
mod selection_tests {
    use super::*;
    #[test]
    fn r702_122a_total_power_counts_a_negative_power_as_itself() {
        // 107.1b: a calculation that needs a negative value uses it.
        let selection = Selection::PowerAtLeast { minimum: 3, powers: vec![(ObjId(1), 4), (ObjId(2), -2), (ObjId(3), 1)] };
        assert!(selection.accepts(&[ObjId(1)]));
        assert!(!selection.accepts(&[ObjId(1), ObjId(2)]));
        assert!(selection.accepts(&[ObjId(1), ObjId(2), ObjId(3)]));
    }
}
