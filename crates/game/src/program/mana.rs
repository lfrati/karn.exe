//! Compiled mana production: what an "add" adds (106.1, 106.4), who chooses among what it
//! offers, and how the mana it adds may be spent (106.6).
//!
//! [`Production::compile`] is the one place a production is refused. What it returns says
//! the mana in the engine's own types: nothing here reads the text again as it runs. The
//! instruction counts what it adds and asks which mana, where its player has a choice,
//! before any mana is added; the mana of one instruction is one event (106.3).
use super::{Frame, Halt, Instr, Program, expr};
use crate::event::Event;
use crate::select::{Ctx, Support, name};
use crate::state::{Spent, Answer, Choice, Game, PlayerId, Restricted};
use crate::{Res, unsupported};
use ir::effect::Rider;
use ir::mana::{ManaColor, ManaCost, ManaSpec, ManaSymbol, ManaTypes, SpendOn};
use ir::{Conj, Count, Quant, Who};

/// One mana, by its type: one of the five colors, or colorless (106.1a, 106.1b).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mana {
    Color(ir::Color),
    Colorless,
}

impl Mana {
    /// A symbol of an "add" is a type of mana; a cost's other symbols are not mana.
    fn compile(symbol: &ManaSymbol) -> Res<Mana> {
        match symbol {
            ManaSymbol::Color(c) => Ok(Mana::Color(*c)),
            ManaSymbol::Colorless => Ok(Mana::Colorless),
            other => unsupported(format!("mana: {}", name(other))),
        }
    }

    fn symbol(self) -> ManaSymbol {
        match self {
            Mana::Color(c) => ManaSymbol::Color(c),
            Mana::Colorless => ManaSymbol::Colorless,
        }
    }
}

fn mana(cost: &ManaCost) -> Res<Vec<Mana>> {
    cost.0.iter().map(Mana::compile).collect()
}

/// Read the productions reached by compiled control flow, without executing an action.
/// Both payment discovery and 106.7 queries use the same branch locals and act boundaries.
/// The caller establishes whether reading the current state is appropriate for its query.
fn visit_productions(game: &Game, mut ctx: Ctx<'static>, program: &Program,
    visit: &mut dyn FnMut(&Ctx, &dyn expr::Locals, &Production) -> Res<()>) -> Res<()> {
    let Some(last) = program.instrs.iter().rposition(|i| matches!(i, Instr::Mana { .. })) else { return Ok(()); };
    let mut locals = vec![None; program.slots];
    let mut pc = 0;
    while pc <= last {
        match &program.instrs[pc] {
            Instr::Mana { production, .. } => visit(&ctx, &locals, production)?,
            Instr::SetX(amount) => ctx.x = Some(amount.evaluate(game, &ctx, &locals)),
            Instr::Branch { condition, end, remember, skipped_values } => {
                let yes = condition.evaluate(game, &ctx, &locals);
                if let Some(slot) = remember { locals[*slot] = Some(crate::binding::Value::Consent(yes)); }
                if !yes {
                    for (slot, value) in skipped_values { locals[*slot] = Some(value.clone()); }
                    pc = *end;
                    continue;
                }
            }
            _ => return unsupported("mana: could produce after a prerequisite action or choice (106.7)"),
        }
        pc += 1;
        if program.acts.iter().any(|act| act.end == pc && !act.keeps_x) { ctx.x = None; }
    }
    Ok(())
}

/// "Spend this mana only to cast a creature spell of the chosen type, and that spell can't
/// be countered." (106.6): said of all the mana its text adds, and kept by that mana.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spend {
    only: Only,
    uncounterable: bool,
    trigger: Option<(Only, std::sync::Arc<super::Program>)>,
    kept: Option<crate::state::Kept>,
}

/// What restricted mana may be spent on (106.6): any of the spells its text describes, as
/// that text was lowered. Nothing of the text is read again as mana is spent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Only(Vec<Use>);
#[derive(Debug, Clone, PartialEq, Eq)]
enum Use { Any, Spell(expr::Filter), Ability(Option<expr::Set>), Activation(crate::activate::ActivationFilter) }

impl Only {

    /// Restrictions describe the announced spell or the source of an activated ability.
    /// Paying an instruction during resolution is neither casting nor activating (106.6).
    pub(crate) fn allows(&self, game: &Game, ctx: &Ctx, on: Spent) -> bool {
        self.0.iter().any(|u| match (u, on) {
            (Use::Any, _) => true,
            (Use::Spell(filter), Spent::Spell(card, cast_as)) => {
                let ctx = Ctx { casting: Some((card, cast_as)), ..ctx.clone() };
                filter.matches(game, &ctx, &expr::NoLocals, crate::event::Target::Object(card))
            }
            (Use::Ability(source), Spent::Ability { source: object, .. }) => source.as_ref().is_none_or(|s|
                s.contains(game, ctx, &expr::NoLocals, crate::event::Target::Object(object))),
            (Use::Activation(filter), Spent::Ability { keyword, mana, .. }) => filter.matches(keyword, mana),
            _ => false,
        })
    }
}

impl Spend {
    /// Refused where the engine cannot tell what the mana may be spent on.
    pub(crate) fn compile(rider: &Rider, support: &Support<'_>) -> Res<Spend> {
        let Rider::SpendOnly { on, uncounterable } = rider else {
            return unsupported(format!("rider: {}", name(rider)));
        };
        if on.items.len() > 1 && on.conj != Conj::Or {
            return unsupported("spend this mana only: uses must be alternatives");
        }
        let mut only = Vec::new();
        for spent in &on.items {
            match spent {
                // "to cast a creature spell of the chosen type": a spell, said by what it is
                SpendOn::Cast(spells) => {
                    if spells.items.len() > 1 && spells.conj == Conj::And {
                        return unsupported("spend this mana only to cast: spells joined by \"and\"");
                    }
                    for s in &spells.items {
                        let set = match s {
                            Who::Set(set) => set,
                            Who::One(s) => {
                                if s.quant != Quant::Indefinite(Count::Exactly(ir::Amount::Const(1))) || s.chooser.is_some() || s.random.is_some() {
                                    return unsupported(format!("spend this mana only to cast: spells said by {}", name(&s.quant)));
                                }
                                &s.set
                            }
                        };
                        if set.domain != ir::Domain::Spell || set.controller.is_some() || set.owner.is_some() || set.caster.is_some() || set.zone.is_some() || set.other {
                            return unsupported("spend this mana only to cast: a spell said by its controller, owner, caster or zone");
                        }
                        only.push(Use::Spell(expr::Filter::compile(&set.filter, support, &mut expr::linked(support))?));
                    }
                }
                SpendOn::ActivateAbilities(source) => only.push(Use::Ability(source.as_ref()
                    .map(|s| expr::Set::compile(s, support, &mut expr::linked(support))).transpose()?)),
                SpendOn::ActivateAnAbility(source) => {
                    let source = source.as_ref().map(|s| {
                        if s.quant != Quant::Indefinite(Count::Exactly(ir::Amount::Const(1))) || s.chooser.is_some() || s.random.is_some() {
                            return unsupported("spend this mana only to activate: a referenced or chosen source");
                        }
                        expr::Set::compile(&s.set, support, &mut expr::linked(support))
                    }).transpose()?;
                    only.push(Use::Ability(source));
                }
                SpendOn::Activate(ir::statics::Who::Set(set)) => only.push(Use::Activation(crate::activate::ActivationFilter::compile(set)?)),
                SpendOn::Activate(_) => return unsupported("spend this mana only to activate: unsupported relative ability source (106.6)"),
                SpendOn::X => return unsupported("spend this mana only: the X component (106.6)"),
            }
        }
        Ok(Spend { only: Only(only), uncounterable: *uncounterable, trigger: None, kept: None })
    }

    /// This mana as it is in a pool: it remembers whose text added it, what "the chosen
    /// type" and "you" in that text are said of (106.6, 607.2d).
    pub(crate) fn restricted(&self, ctx: &Ctx, mana: ManaSymbol) -> Restricted {
        Restricted { provenance: None, ability: ctx.ability.clone(), mana, only: self.only.clone(), uncounterable: self.uncounterable, source: ctx.object_source(), controller: ctx.controller, trigger: None, kept: self.kept }
    }
}

/// What an "add" adds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Production {
    /// "{G}", "{C}{C}": this mana.
    Fixed(Vec<Mana>),
    /// "{R} or {G}", "{W}{W}, {U}{U}, or {B}{B}": one of these, as its player chooses.
    OneOf(Vec<Vec<Mana>>),
    /// "an amount of {G} equal to its power", "{G} for each creature you control": this
    /// mana, so many times as the amount is when it is added (608.2h).
    Times { mana: Vec<Mana>, amount: expr::Amount },
    /// "X mana of any one color": so many, all of the one color its player chooses.
    AnyOneColor(expr::Amount),
    /// "one mana of any color", "X mana in any combination of colors": so many, each of a
    /// color its player chooses. How many is counted as the mana is added (608.2h).
    AnyColors(expr::Amount),
    /// "X mana in any combination of {U} and/or {R}": so many, each of one of these types, as
    /// its player chooses. How many is counted as the mana is added (608.2h).
    CombinationOf { amount: expr::Amount, of: Vec<ManaSymbol> },
    /// Colors fixed by this player's commander designation, not its current characteristics
    /// or location (903.4a, 903.4f).
    CommanderColors(expr::Amount),
    /// Mana types defined by another permanent's current abilities (106.7).
    CouldProduce { amount: expr::Amount, by: expr::Set, types: bool },
    /// Mana that remembers what it may be spent on (106.6).
    Restricted { production: Box<Production>, spend: Spend },
}

/// The most mana whose every combination of colors is offered as one choice: with more,
/// the answers are too many to list, and the color of each mana is asked on its own.
pub(crate) const LISTED: u32 = 3;

/// The five colors, each one mana: what is offered for each mana of "X mana in any
/// combination of colors" when X is more than [`LISTED`].
pub(crate) fn each_color() -> Vec<Vec<ManaSymbol>> {
    COLORS.iter().map(|c| vec![ManaSymbol::Color(*c)]).collect()
}

/// What an "add" offers its player, now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Offered {
    /// One of these, each all the mana it adds; none to choose where there is one.
    Listed(Vec<Vec<ManaSymbol>>),
    /// So many mana, more than [`LISTED`], the color of each chosen in turn: one choice of
    /// the five colors for each.
    Units(u32),
    /// So many mana, the type of each chosen in turn among these: where listing every
    /// combination would offer too many.
    UnitsOf(u32, Vec<ManaSymbol>),
}

/// The most combinations of a few mana types listed as one choice (`CombinationOf`).
const LISTED_COMBINATIONS: usize = 16;

fn unlisted<T>(n: u32) -> Res<T> {
    unsupported(format!("mana: {n} in any combination of colors"))
}

const COLORS: [ir::Color; 5] = [ir::Color::White, ir::Color::Blue, ir::Color::Black, ir::Color::Red, ir::Color::Green];

/// Every way to take `n` mana types, whatever their order.
fn combinations<T: Copy>(n: u32, types: &[T], from: usize) -> Vec<Vec<T>> {
    if n == 0 {
        return vec![Vec::new()];
    }
    (from..types.len())
        .flat_map(|c| combinations(n - 1, types, c).into_iter().map(move |rest| std::iter::once(types[c]).chain(rest).collect()))
        .collect()
}

impl Production {

    /// The production of this text. An amount is lowered with the definition's links: "the
    /// chosen color" of an amount is the linked choice (607.2d). X is an amount only where
    /// `support` says the text has one (107.3).
    pub(crate) fn compile(spec: &ManaSpec, support: &Support<'_>) -> Res<Production> {
        Self::compile_bound(spec, support, &mut expr::linked(support))
    }

    pub(super) fn compile_bound(spec: &ManaSpec, support: &Support<'_>, bind: expr::Bind<'_>) -> Res<Production> {
        let mut amount = |n: &ir::Amount| expr::Amount::compile(n, support, &mut *bind);
        Ok(match spec {
            ManaSpec::Symbols(cost) => Production::Fixed(mana(cost)?),
            // a choice among fixed mana; an alternative that is counted or described is not offered
            ManaSpec::OneOf(alternatives) => Production::OneOf(alternatives.iter().map(|one| match one {
                ManaSpec::Symbols(cost) => mana(cost),
                other => unsupported(format!("mana: one of several, with {}", name(other))),
            }).collect::<Res<_>>()?),
            ManaSpec::AmountOf { symbol, amount: n } => Production::Times { mana: vec![Mana::compile(symbol)?], amount: amount(n)? },
            ManaSpec::Of { amount: n, color: ManaColor::AnyOne } => Production::AnyOneColor(amount(n)?),
            ManaSpec::Of { amount: n, color: ManaColor::Any(ManaTypes::Colors) } => Production::AnyColors(amount(n)?),
            ManaSpec::Of { amount: n, color: ManaColor::Any(ManaTypes::Symbols(of)) } => {
                let of: Vec<ManaSymbol> = of.iter().map(|s| Mana::compile(s).map(|m| m.symbol())).collect::<Res<_>>()?;
                if of.is_empty() { return unsupported("mana: a combination of no type"); }
                Production::CombinationOf { amount: amount(n)?, of }
            },
            ManaSpec::Of { amount: n, color: ManaColor::Any(ManaTypes::CommanderIdentity) } => {
                if let ir::Amount::Const(n) = n && *n > LISTED { return unlisted(*n); }
                Production::CommanderColors(amount(n)?)
            },
            ManaSpec::Of { amount: n, color: ManaColor::Any(ManaTypes::Produced { types, by, could: true }) }
                if by.quant == Quant::Indefinite(Count::Exactly(ir::Amount::Const(1)))
                    && by.chooser.is_none() && by.random.is_none() => {
                if let ir::Amount::Const(n) = n && *n > LISTED { return unlisted(*n); }
                Production::CouldProduce {
                    amount: amount(n)?, types: *types,
                    by: expr::Set::compile(&by.set, support, &mut expr::linked(support))?,
                }
            },
            // types the choice is made among that nothing computes yet: an object's colors
            // (105.2), the colors among a set, what a permanent produced (106.7)
            ManaSpec::Of { color: ManaColor::Any(types), .. } => return unsupported(format!("mana: of {}", name(types))),
            ManaSpec::Of { color, .. } => return unsupported(format!("mana: of {}", name(color))),
        })
    }

    /// "For each" multiplies fixed mana. A single any-color unit contributes independently
    /// chosen colors to the resulting production; it never becomes AnyOneColor.
    pub fn each(self, amount: expr::Amount) -> Res<Production> {
        match self {
            Production::Fixed(mana) => Ok(Production::Times { mana, amount }),
            Production::AnyColors(unit) if unit.as_constant() == Some(1) => Ok(Production::AnyColors(amount)),
            _ => unsupported("mana: for each with variable color choices"),
        }
    }

    /// "Spend this mana only to …": said of all the mana this adds (106.6a).
    pub(crate) fn spend_only(self, rider: &Rider, support: &Support<'_>) -> Res<Production> {
        if matches!(self, Production::Restricted { .. }) {
            return unsupported("mana: restricted twice");
        }
        Ok(Production::Restricted { production: Box::new(self), spend: Spend::compile(rider, support)? })
    }

    /// "Until end of combat, you don't lose this mana as steps and phases end" (106.4): the
    /// mana added is kept in its pool until then.
    pub(crate) fn kept(self, kept: crate::state::Kept) -> Production {
        match self {
            Production::Restricted { production, mut spend } => { spend.kept = Some(kept); Production::Restricted { production, spend } }
            other => Production::Restricted { production: Box::new(other), spend: Spend { only: Only(vec![Use::Any]), uncounterable: false, trigger: None, kept: Some(kept) } },
        }
    }

    /// The delayed body belongs to each unit, not to the eventual source's ability membership.
    pub(super) fn has_spending_trigger(&self) -> bool { self.spending_trigger().is_some() }
    /// The instructions of the delayed triggered ability this production creates for each
    /// mana it adds, that triggers when that mana is spent (106.6, 106.6a).
    pub(super) fn spending_trigger(&self) -> Option<&std::sync::Arc<super::Program>> { self.spend()?.trigger.as_ref().map(|(_, program)| program) }
    pub(super) fn when_spent(&mut self, on: &SpendOn, program: std::sync::Arc<super::Program>, support: &Support<'_>) -> Res<()> {
        if !matches!(on, SpendOn::Cast(_)) { return unsupported("spending trigger: only completed casts are supported"); }
        let rider = Rider::SpendOnly { on: ir::Coord { items: vec![on.clone()], conj: Conj::And }, uncounterable: false };
        let condition = Spend::compile(&rider, support)?.only;
        if let Production::Restricted { spend, .. } = self { spend.trigger = Some((condition, program)); }
        else {
            *self = Production::Restricted { production: Box::new(self.clone()), spend: Spend {
                only: Only(vec![Use::Any]), uncounterable: false, trigger: Some((condition, program)), kept: None,
            }};
        }
        Ok(())
    }

    /// The chosen values the production's amounts read, for checking they are produced
    /// before it.
    pub fn scalars(&self) -> Vec<crate::binding::Scalar> {
        match self {
            Production::CombinationOf { amount, .. } => amount.scalars(),
            Production::Times { amount, .. } | Production::AnyOneColor(amount) | Production::AnyColors(amount) | Production::CommanderColors(amount) => amount.scalars(),
            Production::Restricted { production, .. } => production.scalars(),
            Production::CouldProduce { amount, by, .. } => amount.scalars().into_iter().chain(by.scalars()).collect(),
            Production::Fixed(_) | Production::OneOf(_) => Vec::new(),
        }
    }

    /// What may be added, now: one production where the text says which, several where its
    /// player chooses, or the color of each mana in turn where "X mana in any combination of
    /// colors" is more than [`LISTED`]. Nothing changes here. Refused where other
    /// combinations of colors are then too many to list.
    pub(crate) fn offered(&self, game: &Game, ctx: &Ctx, locals: &dyn expr::Locals) -> Res<Offered> {
        match self {
            Production::AnyColors(amount) => match amount.evaluate(game, ctx, locals) {
                n if n > LISTED => Ok(Offered::Units(n)),
                _ => self.options(game, ctx, locals).map(Offered::Listed),
            },
            Production::CombinationOf { amount, of } => {
                let n = amount.evaluate(game, ctx, locals);
                // multisets of n among k types: C(n+k-1, k-1), listed only while few
                let k = of.len() as u64;
                let count = (1..k).try_fold(1u64, |c, i| c.checked_mul(u64::from(n) + i).map(|c| c / i));
                if count.is_some_and(|c| c as usize <= LISTED_COMBINATIONS) { self.options(game, ctx, locals).map(Offered::Listed) }
                else { Ok(Offered::UnitsOf(n, of.clone())) }
            }
            Production::Restricted { production, .. } => production.offered(game, ctx, locals),
            _ => self.options(game, ctx, locals).map(Offered::Listed),
        }
    }

    /// [`Production::offered`] where it lists every answer.
    fn options(&self, game: &Game, ctx: &Ctx, locals: &dyn expr::Locals) -> Res<Vec<Vec<ManaSymbol>>> {
        let symbols = |mana: &[Mana]| mana.iter().map(|m| m.symbol()).collect::<Vec<_>>();
        Ok(match self {
            Production::Fixed(mana) => vec![symbols(mana)],
            Production::OneOf(options) => options.iter().map(|o| symbols(o)).collect(),
            Production::Times { mana, amount } => vec![symbols(mana).repeat(amount.checked(game, ctx, locals)? as usize)],
            Production::AnyOneColor(amount) => match amount.evaluate(game, ctx, locals) {
                // no mana: nothing to choose the color of
                0 => vec![Vec::new()],
                n => COLORS.iter().map(|c| vec![ManaSymbol::Color(*c); n as usize]).collect(),
            },
            // no mana is one combination, the empty one: nothing to choose
            Production::AnyColors(amount) => match amount.evaluate(game, ctx, locals) {
                n if n > LISTED => return unlisted(n),
                n => combinations(n, &COLORS, 0).into_iter().map(|o| o.into_iter().map(ManaSymbol::Color).collect()).collect(),
            },
            Production::CombinationOf { amount, of } => combinations(amount.evaluate(game, ctx, locals), of, 0),
            Production::CommanderColors(amount) => {
                let colors = game.commander_identity(ctx.controller);
                let n = amount.evaluate(game, ctx, locals);
                if colors.is_empty() { vec![Vec::new()] }
                else if n > LISTED { return unlisted(n); }
                else { combinations(n, &colors, 0).into_iter().map(|o| o.into_iter().map(ManaSymbol::Color).collect()).collect() }
            },
            Production::CouldProduce { amount, by, types } => {
                let n = amount.evaluate(game, ctx, locals);
                if n == 0 { return Ok(vec![Vec::new()]); }
                if n > LISTED { return unlisted(n); }
                let available = could_produce::query(game, by, ctx, locals)?;
                let symbols: Vec<_> = COLORS.iter().map(|c| ManaSymbol::Color(*c))
                    .chain(types.then_some(ManaSymbol::Colorless))
                    .filter(|s| available.contains(*s)).collect();
                if symbols.is_empty() { vec![Vec::new()] } else { combinations(n, &symbols, 0) }
            },
            Production::Restricted { production, .. } => return production.options(game, ctx, locals),
        })
    }

    fn spend(&self) -> Option<&Spend> {
        match self {
            Production::Restricted { spend, .. } => Some(spend),
            _ => None,
        }
    }
}

/// One proposed mana action. Players choose in APNAP order before any pool changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Pending {
    provenance: Option<std::sync::Arc<crate::audit::RestrictedManaProvenance>>,
    ctx: Ctx<'static>,
    spend: Option<Spend>,
    trigger: Option<crate::state::SpendTrigger>,
    parts: Vec<(PlayerId, ManaChoice)>,
}
/// What the production offered as it began, frozen with each answer given so far: on a
/// stack or as a mana ability (605.3b), the same choice, also kept across a copy of the game.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ManaChoice {
    Listed { options: Vec<Vec<ManaSymbol>>, chosen: Option<Vec<ManaSymbol>> },
    /// `each`: what each mana may be.
    Units { remaining: u32, chosen: Vec<ManaSymbol>, each: Vec<Vec<ManaSymbol>> },
}
impl ManaChoice {
    fn new(production: &Production, game: &Game, frame: &Frame) -> Res<Self> {
        Ok(match production.offered(game, &frame.ctx, frame)? {
            Offered::Units(n) => Self::Units { remaining: n, chosen: Vec::new(), each: each_color() },
            Offered::UnitsOf(n, of) => Self::Units { remaining: n, chosen: Vec::new(), each: of.iter().map(|s| vec![*s]).collect() },
            Offered::Listed(options) => {
                let chosen = if options.len() == 1 { Some(options[0].clone()) } else { None };
                Self::Listed { options, chosen }
            }
        })
    }
    fn complete(&self) -> bool { match self { Self::Listed { chosen, .. } => chosen.is_some(), Self::Units { remaining, .. } => *remaining == 0 } }
    fn options(&self) -> Vec<Vec<ManaSymbol>> { match self {
        Self::Listed { options, .. } => options.clone(),
        Self::Units { each, .. } => each.clone(),
    } }
    fn answer(&mut self, mana: Vec<ManaSymbol>) {
        if self.complete() || !self.options().contains(&mana) { return; }
        match self {
            Self::Listed { chosen, .. } => *chosen = Some(mana),
            Self::Units { remaining, chosen, .. } => { *remaining -= 1; chosen.extend(mana); }
        }
    }
    fn produced(self) -> Vec<ManaSymbol> { match self {
        Self::Listed { chosen, .. } => chosen.expect("every production chosen"),
        Self::Units { remaining, chosen, .. } => { assert_eq!(remaining, 0); chosen }
    } }
}
impl Pending {
    fn new(game: &Game, frame: &Frame, production: &Production, mut players: Vec<PlayerId>) -> Res<Self> {
        players.sort_by_key(|p| (p + game.players.len() - game.active) % game.players.len());
        players.dedup();
        let choice = ManaChoice::new(production, game, frame)?;
        let trigger=production.spend().and_then(|s|s.trigger.as_ref()).map(|(on,program)|crate::state::SpendTrigger {
            on:on.clone(), captured:super::Captured {ctx:frame.ctx.clone(),body:super::Body::Plain(program.clone()),intervening:None,triggered:true,chapter:false,values:frame.slots.clone(),keyword_producer:None,granted_keyword:None,sparse_checked:false},
            bound:crate::state::Bound {inputs:frame.inputs.clone(),event_amount:frame.event_amount,became:frame.transitions.clone(),linked_ability:frame.ctx.ability.clone(),..Default::default()}
        });
        let provenance = production.spend().and_then(|spend| {
            // Runtime-added riders need their own wording/provenance. Refuse their
            // portable projection rather than silently omitting them (106.6).
            if spend.trigger.is_some() || spend.kept.is_some() { return None; }
            let key = frame.ctx.ability.as_ref()?;
            let source = frame.ctx.object_source();
            crate::audit::RestrictedManaProvenance::freeze(game, source, key)
        });
        Ok(Self { provenance, ctx: frame.ctx.clone(), trigger, spend: production.spend().cloned(), parts: players.into_iter().map(|p| (p, choice.clone())).collect() })
    }
    pub(super) fn answer(&mut self, answer: Answer) {
        if let Some((_, choice)) = self.parts.iter_mut().find(|(_, c)| !c.complete())
            && let Answer::Mana(mana) = answer { choice.answer(mana); }
    }
    fn advance(&mut self, game: &mut Game) -> Result<bool, crate::state::Decision> {
        if let Some((player, choice)) = self.parts.iter().find(|(_, c)| !c.complete()) {
            return Err(crate::state::Decision::Choose { player: *player, choice: Choice::Mana { options: choice.options() } });
        }
        let mut added = false;
        for (player, choice) in std::mem::take(&mut self.parts) {
            let produced = choice.produced();
            let audit_before=game.payment_audit.as_ref().map(|_|game.players[player].pool.clone());
            for symbol in &produced {
                match &self.spend {
                    Some(spend) => {let mut unit=spend.restricted(&self.ctx,*symbol);unit.trigger=self.trigger.clone();unit.provenance=self.provenance.clone();game.add_restricted_mana(player,unit);},
                    None => game.add_mana(player, *symbol),
                }
            }
            if !produced.is_empty() {
                added = true;
                if let Some(before)=audit_before { game.audit_mana_production(self.ctx.object_source(),player,self.ctx.ability.as_ref(),&produced,before); }
                game.log.push(Event::ManaAdded { source: self.ctx.object_source(), player, mana: produced });
            }
        }
        Ok(added)
    }
}
pub(super) fn to_players(game: &mut Game, frame: &mut Frame, production: &Production, players: Vec<PlayerId>) -> Result<(), Halt> {
    let mut pending = match frame.pending.take() {
        Some(super::Pending::Mana(pending)) => pending,
        None => Pending::new(game, frame, production, players).map_err(Halt::Unsupported)?,
        _ => unreachable!("mana instruction resumes its own procedure"),
    };
    match pending.advance(game) {
        Ok(added) => { frame.produced_mana |= added; Ok(()) }
        Err(decision) => {
            frame.pending = Some(super::Pending::Mana(pending));
            Err(Halt::Ask(decision))
        }
    }
}

/// "Add …": the player whose text it is adds the mana.
#[cfg(test)]
pub(super) fn run(game: &mut Game, f: &mut Frame, production: &Production) -> Result<(), Halt> {
    to_players(game, f, production, vec![f.ctx.controller])
}

#[cfg(test)]
mod tests;

mod could_produce;
