//! Compiled continuous changes: what "gets +X/+X", "gains flying", "becomes a 2/2 creature"
//! make of what they affect while an effect lasts (611).
//!
//! [`Change::compile`] is the one place a change is refused. What does not depend on the game
//! is the layers' own change already ([`Change::Fixed`]); a number that does is an amount
//! counted when the change is asked: once for a resolved effect, as it begins (611.2d), each
//! time for a static ability (611.3a). Nothing here reads the text again as it is asked.
//! Whom the change affects and how long it lasts are not said here.
use super::expr::{self, Bind, Locals};
use crate::binding::Scalar;
use crate::layers::{self, becoming, profile};
use crate::select::{Ctx, Support, name};
use crate::state::Game;
use crate::{Res, unsupported};
use ir::action::Becoming;
use ir::statics::StaticAct;
use ir::{Action, Color};

/// A signed change to power or toughness: "+2", "-1", "+X".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delta {
    amount: expr::Amount,
    minus: bool,
}

impl Delta {
    fn compile(d: &ir::action::Delta, support: &Support<'_>, bind: Bind<'_>) -> Res<Delta> {
        Ok(Delta { amount: expr::Amount::compile(&d.amount, support, bind)?, minus: d.minus })
    }

    fn evaluate(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals) -> i32 {
        let n = self.amount.evaluate(game, ctx, locals) as i32;
        if self.minus { -n } else { n }
    }
}

/// What a continuous effect does to what it affects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// The resolving occurrence gains control (613.1b); captured when the effect begins.
    Controller,
    /// What the text says outright: types, colors, abilities, base power and toughness.
    Fixed(layers::Change),
    /// A color characteristic definition, applied before other layer-five effects (613.2).
    DefineColors(Vec<Color>),
    /// Subtype characteristic definition (604.3, 702.73), before other layer-four effects.
    DefineTypes(layers::Types),
    DefinePt { power: Option<expr::Amount>, toughness: Option<expr::Amount> },
    /// "gets +3/+3", "gets -X/-X" (613.4c): by amounts counted when it is asked.
    Pt { power: Delta, toughness: Delta },
    /// Keyword abilities that say something only as the effect applies: "gains protection
    /// from the chosen color", the color as it was chosen (608.2h).
    Gains(Vec<crate::keyword::Held>),
    /// "This land is the chosen type." (205.1, 305.7): the subtype chosen, set or, `keeps`,
    /// added. Nothing chosen changes nothing (607.5a).
    ChosenType { scalar: Scalar, keeps: bool },
}

impl Change {
    /// Frozen, context-independent changes from a resolved printed instruction.
    /// Initial positions must not reevaluate a historical X, choice or count.
    pub(crate) fn initial_fixed(&self, controller: usize) -> Option<layers::Change> {
        match self {
            Self::Fixed(change) => Some(change.clone()),
            Self::Controller => Some(layers::Change::Control(controller)),
            Self::Pt { power, toughness } => {
                let signed = |delta: &Delta| {
                    let n = i32::try_from(delta.amount.as_constant()?).ok()?;
                    Some(if delta.minus { -n } else { n })
                };
                Some(layers::Change::Pt(signed(power)?, signed(toughness)?))
            }
            _ => None,
        }
    }
    pub(crate) fn inputs(&self) -> super::footprint::Inputs {
        use super::footprint::{Domain as D, Inputs as I, Properties as P};
        match self {
            Self::Fixed(layers::Change::Inherits(set)) => set.inputs() | I::at(D::Objects, P::MEMBERSHIP),
            Self::Fixed(_) | Self::DefineColors(_) | Self::DefineTypes(_) | Self::Gains(_) | Self::ChosenType { .. } => I::NONE,
            Self::Controller => I::at(D::Context,P::CONTROL),
            Self::DefinePt { power, toughness } => power.iter().chain(toughness).fold(I::NONE, |all,n|all | n.inputs()),
            Self::Pt { power, toughness } => power.amount.inputs() | toughness.amount.inputs(),
        }
    }
    pub(crate) fn writes(&self) -> super::footprint::Properties {
        use super::footprint::Properties as P;
        match self {
            Self::Fixed(c) => c.writes(), Self::Controller => P::CONTROL,
            Self::Gains(_) => P::KEYWORDS,
            Self::DefineTypes(t) => P::TYPES | if t.sets_basic_land_type() { P::ABILITIES } else { P::NONE },
            Self::DefineColors(_) => P::COLORS,
            // a basic land type sets its mana ability and removes others (305.7)
            Self::ChosenType { .. } => P::TYPES | P::ABILITIES,
            Self::DefinePt { .. } | Self::Pt { .. } => P::PT,
        }
    }

    /// What the act changes: an effect in each layer it changes. A reference to a chosen
    /// value is bound by `bind`.
    pub(crate) fn compile(action: &Action, support: &Support<'_>, bind: Bind<'_>) -> Res<Vec<Change>> {
        let fixed = |changes: Vec<layers::Change>| changes.into_iter().map(Change::Fixed).collect();
        let gains = |changes: Vec<layers::Change>| changes.into_iter().map(|c| match c {
            layers::Change::Gains(held) if held.iter().any(crate::keyword::Held::captures) => Change::Gains(held),
            other => Change::Fixed(other),
        }).collect();
        Ok(match action {
            Action::Gets { power, toughness } => {
                vec![Change::Pt { power: Delta::compile(power, support, &mut *bind)?, toughness: Delta::compile(toughness, support, bind)? }]
            }
            Action::Gains(granted) => gains(layers::granted_bound(granted, support, bind)?),
            Action::SetStat { of: None, set, .. } => match profile(set) {
                Some((p, t)) => fixed(vec![layers::Change::BasePt(p, t)]),
                None => return unsupported("change: set a stat to anything but numbers"),
            },
            Action::Is(Becoming::Is(c)) | Action::Becomes(Becoming::Is(c)) => fixed(becoming(c, support)?),
            Action::Becomes(Becoming::Colors { colors, .. }) if colors.as_slice() == Color::ALL => return unsupported("becomes: all colors"),
            Action::Is(Becoming::Colors { colors, added }) | Action::Becomes(Becoming::Colors { colors, added }) => fixed(vec![layers::Change::Colors { colors: colors.clone(), added: *added }]),
            Action::Is(Becoming::ChosenType(keeps)) => {
                use ir::action::Keeps;
                let keeps = match keeps {
                    Keeps::Nothing => false,
                    Keeps::OtherTypes | Keeps::OtherCreatureTypes | Keeps::OtherLandTypes => true,
                    _ => return unsupported("is the chosen type: in addition to its colors"),
                };
                let scalar = bind.scalar(&ir::reference::UnresolvedRef::choice(ir::reference::Kind::Type))?;
                if !matches!(scalar.clone().kind(), ir::reference::Kind::BasicLandType | ir::reference::Kind::CreatureType) {
                    return unsupported("is the chosen type: a type other than a basic land or creature type");
                }
                vec![Change::ChosenType { scalar, keeps }]
            }
            Action::Static(s) => match s.as_ref() {
                StaticAct::Has(granted) => gains(layers::granted_bound(granted, support, bind)?),
                StaticAct::Loses(lost) => fixed(layers::lost(lost, support)?),
                StaticAct::IsNot(Becoming::Is(c)) => fixed(vec![layers::not_types(c)?]),
                other => return unsupported(format!("static: {}", name(other))),
            },
            other => return unsupported(format!("change: {}", name(other))),
        })
    }

    /// "It's still a land.", said of the becoming before it: the prior card types, subtypes
    /// and supertypes are kept (205.1b).
    pub fn still(changes: Vec<Change>) -> Vec<Change> {
        let still = |change| match change {
            Change::Fixed(fixed) => layers::still(vec![fixed]).into_iter().map(Change::Fixed).collect(),
            other => vec![other],
        };
        changes.into_iter().flat_map(still).collect()
    }

    /// The layer it is applied in (613.1), which no number of it changes.
    pub(crate) fn layer(&self) -> layers::Layer {
        match self {
            Change::DefineColors(_) => layers::Layer::Colors,
            Change::DefineTypes(_) => layers::Layer::Types,
            Change::Controller => layers::Layer::Control,
            Change::Fixed(change) => change.layer(),
            Change::Gains(_) => layers::Layer::Abilities,
            Change::ChosenType { .. } => layers::Layer::Types,
            Change::Pt { .. } => layers::Layer::ChangePt,
            Change::DefinePt { .. } => layers::Layer::DefinePt,
        }
    }

    /// The change, with its numbers as they are now.
    pub fn evaluate(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals) -> layers::Change {
        match self {
            Change::DefineTypes(types) => layers::Change::Types(types.clone()),
            Change::DefineColors(colors) => layers::Change::Colors { colors: colors.clone(), added: false },
            Change::Controller => layers::Change::Control(ctx.controller),
            Change::Fixed(change) => change.clone(),
            Change::Gains(held) => layers::Change::Gains(held.iter().map(|h| h.capture(game, ctx, locals)).collect()),
            Change::DefinePt { power, toughness } => layers::Change::DefinedPt(power.as_ref().map(|n| n.evaluate(game,ctx,locals) as i32), toughness.as_ref().map(|n| n.evaluate(game,ctx,locals) as i32)),
            Change::Pt { power, toughness } => layers::Change::Pt(power.evaluate(game, ctx, locals), toughness.evaluate(game, ctx, locals)),
            Change::ChosenType { scalar, keeps } => {
                let subtypes = match expr::chosen(game, ctx, locals, scalar.clone()) {
                    Some(crate::binding::Value::BasicLandType(t) | crate::binding::Value::CreatureType(t)) => vec![t.clone()],
                    _ => Vec::new(),
                };
                let keeps = *keeps || subtypes.is_empty();
                layers::Change::Types(layers::Types { types: Vec::new(), subtypes, every_creature_type: false, keeps })
            }
        }
    }

    /// What counting the change's numbers can read ([`expr::Reads`]).
    #[allow(dead_code)]
    pub(crate) fn reads(&self) -> expr::Reads {
        match self {
            Change::Fixed(layers::Change::Inherits(set)) => set.reads() | expr::Reads::ABILITIES,
            Change::Fixed(_) | Change::Controller | Change::DefineColors(_) | Change::DefineTypes(_) | Change::Gains(_) | Change::ChosenType { .. } => expr::Reads::NONE,
            Change::DefinePt { power, toughness } => power.iter().chain(toughness).fold(expr::Reads::NONE, |all, n| all | n.reads()),
            Change::Pt { power, toughness } => power.amount.reads() | toughness.amount.reads(),
        }
    }

    /// The chosen values the change's amounts read, for checking they are produced before it.
    pub fn scalars(&self) -> Vec<Scalar> {
        match self {
            Change::Fixed(_) | Change::Controller | Change::DefineColors(_) | Change::DefineTypes(_) => Vec::new(),
            Change::Gains(held) => held.iter().flat_map(crate::keyword::Held::scalars).collect(),
            Change::ChosenType { scalar, .. } => vec![scalar.clone()],
            Change::DefinePt { power, toughness } => power.iter().chain(toughness).flat_map(expr::Amount::scalars).collect(),
            Change::Pt { power, toughness } => power.amount.scalars().into_iter().chain(toughness.amount.scalars()).collect(),
        }
    }
}

#[cfg(test)]
mod tests;

/// The supported durations of a resolving characteristic effect (611.2a–b).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Duration { Forever, Turn, SourceControlled, SourcePresent }
impl Duration {
    pub(crate) fn compile(mods: &[ir::effect::Modifier]) -> Res<Self> {
        use ir::{Condition, Quant, Ref};
        use ir::effect::{Modifier, Duration as D};
        let stated: Vec<_> = mods.iter().filter_map(|m| if let Modifier::Duration(d) = m { Some(d) } else { None }).collect();
        if stated.len() > 1 { return unsupported("multiple continuous-effect durations"); }
        if let Some(D::While(c)) = stated.first() {
            if let Condition::Remains { what, pred: ir::Predicate::In(zone) } = c.as_ref()
                && what.quant == Quant::Definite(Ref::This) && what.chooser.is_none() && what.random.is_none()
                && what.set == ir::ObjectSet::of(ir::Domain::Permanent)
                && zone.zones.items == [ir::Zone::Battlefield] && zone.owner.is_none() && zone.slice.is_none() {
                return Ok(Self::SourcePresent);
            }
            if let Condition::Controls { player, what, negated: false } = c.as_ref()
                && player.quant == Quant::Definite(Ref::You)
                && matches!(what.items.as_slice(), [s] if s.quant == Quant::Definite(Ref::This)) {
                return Ok(Self::SourceControlled);
            }
            return unsupported("continuous duration: requires control or battlefield presence of this source");
        }
        Ok(if crate::layers::lasts(&[mods])? { Self::Turn } else { Self::Forever })
    }
    pub(crate) fn evaluate(self, ctx: &Ctx) -> crate::layers::Lifetime {
        match self {
            Self::Forever => crate::layers::Lifetime::Forever,
            Self::Turn => crate::layers::Lifetime::Turn,
            Self::SourceControlled => crate::layers::Lifetime::Controlled { source: ctx.object_source(), player: ctx.controller },
            Self::SourcePresent => crate::layers::Lifetime::SourcePresent { source: ctx.object_source() },
        }
    }
}
