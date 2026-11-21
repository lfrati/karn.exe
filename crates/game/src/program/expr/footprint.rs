//! Exhaustive typed-node audit. New variants must choose metadata or explicit Unknown.
use super::*;
use crate::program::footprint::{Domain as D, Inputs as I, Properties as P};
fn context(p: P) -> I { I::at(D::Context, p) }
fn recipient(p: P) -> I { I::at(D::Recipient, p) }
impl Subject {
    pub(crate) fn inputs(&self) -> I {
        match self {
            Self::Source => I::at(D::Source, P::IDENTITY),
            Self::Controller => context(P::CONTROL),
            Self::Attached => I::at(D::Source, P::ATTACHMENT | P::IDENTITY),
            Self::Commanders(of) => of.inputs() | I::at(D::Objects, P::COMMANDER | P::OWNER | P::ZONE | P::COLLECTIONS),
            Self::Union(v) => v.iter().fold(I::NONE, |a, b| a | b.inputs()),
            Self::Players(set) => set.inputs(),
            Self::ControllerOf(of) => of.inputs() | I::at(D::Objects, P::CONTROL),
            Self::OwnerOf(of) => of.inputs() | I::at(D::Objects, P::OWNER),
            Self::LinkedExile(_) => I::at(D::Objects, P::ZONE | P::COLLECTIONS) | context(P::BINDINGS),
            Self::Actor | Self::ObjectActor | Self::Local(_) | Self::Input(_) | Self::Targets(_) => context(P::BINDINGS),
        }
    }
}
impl Stat { fn inputs(self) -> P { match self { Self::Power | Self::Toughness | Self::PowerAndToughness => P::PT, Self::ManaValue => P::MANA } } }
impl Filter {
    pub(crate) fn inputs(&self) -> I {
        match self {
            Self::All(v) | Self::Any(v) => v.iter().fold(I::NONE, |a, b| a | b.inputs()),
            Self::Not(f) => f.inputs(), // Never drop negative reads or short-circuited branches.
            Self::Atom(a) => match a {
                Atom::Type(_) | Atom::PermanentType | Atom::Subtype(_) | Atom::BasicLandType | Atom::Supertype(_) => recipient(P::TYPES),
                Atom::HasAdventure | Atom::PreparedSpell | Atom::ManaCost(_) => recipient(P::CATALOG | P::MANA),
                Atom::Color(_) | Atom::Colorless | Atom::Multicolored | Atom::Monocolored => recipient(P::COLORS),
                Atom::Opponent => context(P::CONTROL) | I::at(D::Players, P::IDENTITY),
                Atom::Keyword(_) => recipient(P::KEYWORDS),
                Atom::Ability { .. } => recipient(P::ABILITIES),
                Atom::TargetCount(_) => recipient(P::BINDINGS),
                // Reads the recipient's target identities and other objects' descriptions;
                // a precise nested-recipient dependency mapping is not modeled yet.
                Atom::Targets(_) => I::UNKNOWN,
                Atom::Tapped(_) => recipient(P::TAPPED),
                Atom::Token => recipient(P::IDENTITY | P::CATALOG),
                Atom::Saddled => recipient(P::DESIGNATIONS | P::ZONE),
                Atom::Commander => recipient(P::COMMANDER),
                // the face selects the definition (708.2a); only a spell or permanent, or one as it last was there
                Atom::FaceDown => recipient(P::IDENTITY | P::CATALOG | P::ZONE),
                Atom::Attached(_) => recipient(P::IDENTITY | P::ZONE) | I::at(D::Objects, P::COLLECTIONS | P::IDENTITY | P::ZONE | P::ATTACHMENT | P::TYPES),
                Atom::Combat(_) => recipient(P::COMBAT),
                Atom::Stat { stat, value, .. } => recipient(stat.inputs()) | value.inputs(),
                Atom::Most { stat, among, .. } => recipient(stat.inputs()) | among.inputs() | I::at(D::Objects, stat.inputs()),
                Atom::Chosen(_) => I::UNKNOWN, // Binding kind chooses type/color; no narrower proof yet.
                Atom::SameName(of) => recipient(P::NAME) | of.inputs() | I::at(D::Objects, P::NAME),
                Atom::SharesCreatureType(of) => recipient(P::TYPES) | of.inputs() | I::at(D::Objects, P::TYPES),
                Atom::Named(_) => recipient(P::NAME),
                Atom::EnteredThisTurn => recipient(P::HISTORY) | context(P::TURN),
                // trigger patterns: no complete property walker (as conditions' Happened)
                Atom::HappenedTo(_) => I::UNKNOWN,
                Atom::LostLifeThisTurn => I::at(D::Players, P::HISTORY) | context(P::TURN),
                Atom::DealtDamageBy(by) => recipient(P::HISTORY) | I::at(D::Players, P::HISTORY) | by.inputs() | context(P::TURN),
                Atom::TappedFor { what, .. } => recipient(P::HISTORY) | what.inputs() | context(P::TURN),
                Atom::FoughtThisTurn { .. } => recipient(P::HISTORY) | I::at(D::Players, P::HISTORY) | context(P::TURN),
                Atom::Damage { to, .. } => recipient(P::HISTORY) | I::at(D::Players, P::HISTORY) | to.as_ref().map_or(I::default(), Subject::inputs) | context(P::TURN),
                Atom::Counters { .. } => recipient(P::COUNTERS),
                Atom::Level { .. } => recipient(P::LEVEL),
                Atom::Entered(_) | Atom::InSubjects(_) => context(P::BINDINGS),
            },
        }
    }
}
impl Relation { fn inputs(&self) -> I { match self { Self::Relative(_) => context(P::CONTROL), Self::Named { who, .. } => who.inputs() } } }
impl Set {
    pub(crate) fn inputs(&self) -> I {
        let domain = match &self.domain {
            Domain::Union(v) => v.iter().fold(I::NONE, |a, b| a | b.inputs()),
            Domain::LinkedExile(_) | Domain::CostExile(_) => I::at(D::Objects, P::ZONE | P::COLLECTIONS) | context(P::BINDINGS),
            Domain::Zones { .. } => I::at(D::Objects, P::ZONE | P::OWNER | P::COLLECTIONS | P::IDENTITY | P::CATALOG | P::ABILITIES) | context(P::CONTROL),
            // Sort membership can change with types, definition membership or zone.
            Domain::Sorts { sorts, .. } => I::at(D::Objects, P::ZONE | P::IDENTITY | P::COLLECTIONS
                | if sorts.iter().any(|s|matches!(s,Sort::Of(_))) { P::TYPES } else { P::NONE }
                | if sorts.iter().any(|s|matches!(s,Sort::Spell | Sort::Ability | Sort::Card)) { P::ABILITIES | P::CATALOG } else { P::NONE })
                | I::at(D::Players, P::IDENTITY | P::COLLECTIONS | P::PLAYER_STATUS),
        };
        let mut all = domain | self.filter.inputs().over_objects() | I::at(D::Players,P::PLAYER_STATUS);
        if self.other { all |= context(P::IDENTITY | P::CONTROL); }
        if let Some(of) = &self.other_than { all |= of.inputs(); }
        if let Some(rel) = &self.controller { all |= rel.inputs() | I::at(D::Objects, P::CONTROL); }
        if let Some(rel) = &self.owner { all |= rel.inputs() | I::at(D::Objects, P::OWNER); }
        all
    }
}
impl Number {
    pub(crate) fn inputs(&self) -> I {
        match self {
            Self::Product(a,b) | Self::Sum(a,b) | Self::Difference(a,b) => a.inputs() | b.inputs(),
            Self::Half { of, .. } => of.inputs(),
            Self::FilteredResultCount { filter, .. } | Self::DeathCount { filter, .. } | Self::Members { filter, .. } => context(P::BINDINGS) | filter.inputs().over_objects(),
            Self::Happened(_) => I::UNKNOWN,
            Self::DeathCountByActor { filter, .. } => context(P::BINDINGS) | filter.inputs().over_objects() | I::at(D::Objects, P::CONTROL),
            Self::Damage { to, .. } => context(P::BINDINGS) | to.as_ref().map_or(I::NONE, Subject::inputs),
            Self::Const(_) => I::NONE,
            Self::X | Self::EventAmount | Self::LifeLost(_) | Self::ResultCount(_) | Self::RemovedCount { .. } => context(P::BINDINGS),
            Self::CountZone(z) => z.owner.inputs() | z.cards.filter.inputs().over_objects() | I::at(D::Objects, P::ZONE | P::COLLECTIONS | P::IDENTITY),
            Self::Count(s) => s.inputs(),
            Self::Colors(s) => s.inputs() | I::at(D::Objects, P::COLORS),
            Self::CardTypes(s) => s.inputs() | I::at(D::Objects, P::TYPES),
            Self::Stat(of, stat) => of.inputs() | I::at(D::Objects, stat.inputs()),
            Self::Counters(of, _) => of.inputs() | I::at(D::Objects, P::COUNTERS),
            Self::CountersAmong(s, _) => s.inputs() | I::at(D::Objects, P::COUNTERS),
            Self::PlayerCounters(of, _) => of.inputs() | I::at(D::Players, P::COUNTERS | P::PLAYER_STATUS),
            Self::Life(of) => of.inputs() | I::at(D::Players, P::LIFE),
            Self::Speed(of) => of.inputs() | I::at(D::Players, P::PLAYER_STATUS),
            Self::Aggregate(_, stat, s) => s.inputs() | I::at(D::Objects, stat.inputs()),
            Self::Kicked(of) | Self::Replicated(of) => of.inputs() | I::at(D::Objects, P::BINDINGS),
            // the X kept on the source permanent from the spell it was (107.3m)
            Self::SpellX | Self::ManaColorsSpent => context(P::BINDINGS) | I::at(D::Objects, P::BINDINGS),
        }
    }
}
impl Amount { pub(crate) fn inputs(&self) -> I { self.0.inputs() } }
