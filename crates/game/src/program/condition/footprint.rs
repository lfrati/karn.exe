//! Conservative condition metadata, independent of short-circuit execution.
use super::*;
use crate::program::footprint::{Domain as D, Inputs as I, Properties as P};
impl Players {
    fn inputs(&self) -> I { match &self.who { Who::You => I::at(D::Context,P::CONTROL), Who::Of(s) => s.inputs(), Who::Reference(s) => s.inputs() } }
}
impl Quantity {
    fn inputs(&self) -> I { match self {
        Self::None | Self::Any => I::NONE,
        Self::AtLeast(n) | Self::AtMost(n) | Self::Exactly(n) => n.inputs(),
        Self::Between(a,b) => a.inputs() | b.inputs(),
    } }
}
impl Values {
    fn inputs(&self) -> I { match self {
        Self::Life(p) => p.inputs() | I::at(D::Players,P::LIFE),
        Self::StartingLife(p) => p.inputs() | I::at(D::Context,P::LIFE),
        Self::Speed(p) => p.inputs() | I::at(D::Players,P::PLAYER_STATUS),
        Self::Number(n) => n.inputs(),
    } }
}
impl Condition { pub(crate) fn inputs(&self) -> I { self.0.inputs() } }
impl Asks {
    fn inputs(&self) -> I {
        match self {
            Self::And(v) | Self::Or(v) => v.iter().fold(I::NONE, |a,b|a | b.inputs()),
            Self::Controls { players, what, .. } => what.iter().fold(players.inputs() | I::at(D::Objects,P::CONTROL | P::COLLECTIONS), |a,(s,q)|a | s.inputs() | q.inputs()),
            Self::Exists { what, count } => what.inputs() | count.inputs(),
            Self::Controlled { players, what, .. } => players.inputs() | what.inputs() | I::at(D::Objects,P::CONTROL) | I::at(D::Context,P::HISTORY),
            Self::Is { what, is, past, .. } => {
                let of = match what { Thing::Source => I::at(D::Source,P::IDENTITY), Thing::Controller => I::at(D::Context,P::CONTROL), Thing::Named(s) => s.inputs() };
                // Lift candidate reads broadly: the reference may denote several objects.
                let reads = match is { Is::Adjective(f) => f.inputs().over_objects(), Is::Card(f) => f.inputs().over_objects() | I::at(D::Objects,P::ZONE | P::IDENTITY | P::CATALOG | P::ABILITIES), Is::A(s) => s.inputs(), Is::Designated(_) => I::at(D::Objects,P::ZONE | P::DESIGNATIONS) };
                of | reads | if *past { I::at(D::Context,P::HISTORY) } else { I::NONE }
            },
            Self::Turn { .. } | Self::NthTurn { .. } => I::at(D::Context,P::TURN | P::CONTROL),
            Self::TurnOf { who, .. } => who.inputs() | I::at(D::Context,P::TURN),
            Self::Compare { left, right, .. } => left.inputs() | right.inputs(),
            Self::MoreThan { players, measure, margin, than, .. } => {
                players.inputs() | match measure { Measure::Life => I::at(D::Players,P::LIFE), Measure::Controls(s) => s.inputs() | I::at(D::Objects,P::CONTROL | P::COLLECTIONS) }
                    | margin.as_ref().map_or(I::NONE,expr::Number::inputs)
                    | match than { Than::Players(p) => p.inputs(), Than::Values(v) => v.inputs() }
            },
            Self::HadCounters { count, .. } => count.inputs() | I::at(D::Source,P::COUNTERS) | I::at(D::Context,P::HISTORY),
            Self::Entered { under, count, .. } | Self::Departed { under, count, .. } => under.as_ref().map_or(I::NONE,Players::inputs) | count.inputs() | I::at(D::Context,P::HISTORY | P::TURN),
            Self::LifeLost { players, count, .. } => players.inputs() | count.inputs() | I::at(D::Context,P::HISTORY | P::TURN),
            Self::AttackedWith { players, those, count, .. } => players.inputs() | those.inputs().over_objects() | count.inputs() | I::at(D::Context,P::HISTORY | P::TURN),
            Self::CastBy { players, .. } => players.inputs() | I::at(D::Context,P::HISTORY | P::BINDINGS),
            Self::CastFrom { .. } | Self::Kicked { .. } | Self::OptionalPaid { .. } | Self::CostPaid { .. } | Self::Gift { .. } | Self::Stored { .. } | Self::Outcome { .. } => I::at(D::Context,P::BINDINGS),
            Self::CastFor { what, .. } => what.inputs() | I::at(D::Objects,P::BINDINGS),
            Self::Targets { what, target } => what.inputs() | target.inputs() | I::at(D::Objects,P::BINDINGS),
            Self::ManaSpent { spell, test, .. } => spell.inputs() | I::at(D::Objects,P::BINDINGS) | match test { PaidTest::Count(_,q) => q.inputs(), PaidTest::Symbols(_) => I::NONE },
            // Trigger patterns can inspect event/LKI/linked contexts and their selectors.
            // Their complete property walker is not implemented: keep broad trials.
            Self::Happened { .. } => I::UNKNOWN,
        }
    }
}
