//! Conservative inputs of characteristic programs, separate from the narrow expr::Reads.
//! Metadata describes POSSIBLE reads/writes, including failed predicates and empty domains.
//! It is not an applicability test, dependency edge, or cache-validity certificate.

use std::ops::{BitOr, BitOrAssign};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Properties(u64);
impl Properties {
    pub(crate) const NONE: Self = Self(0);
    pub(crate) const ALL: Self = Self(u64::MAX);
    pub(crate) const IDENTITY: Self = Self(1 << 0);
    pub(crate) const CONTROL: Self = Self(1 << 1);
    pub(crate) const OWNER: Self = Self(1 << 2);
    pub(crate) const ZONE: Self = Self(1 << 3);
    pub(crate) const TYPES: Self = Self(1 << 4);
    pub(crate) const COLORS: Self = Self(1 << 5);
    /// Producing definition/Origin/Grant and intrinsic ability membership. Passive keyword
    /// gains/losses do not change these identities; complete removal changes both bits.
    pub(crate) const MEMBERSHIP: Self = Self(1 << 6);
    pub(crate) const KEYWORDS: Self = Self(1 << 26);
    pub(crate) const ABILITIES: Self = Self(Self::MEMBERSHIP.0 | Self::KEYWORDS.0);
    pub(crate) const PT: Self = Self(1 << 7);
    pub(crate) const NAME: Self = Self(1 << 8);
    pub(crate) const MANA: Self = Self(1 << 9);
    pub(crate) const TAPPED: Self = Self(1 << 10);
    pub(crate) const COUNTERS: Self = Self(1 << 11);
    /// A permanent's level (716.2b).
    pub(crate) const LEVEL: Self = Self(1 << 27);
    /// A permanent's monstrous, renowned and saddled designations (701.37b, 702.112b, 702.171b).
    pub(crate) const DESIGNATIONS: Self = Self(1 << 28);
    /// Which creatures soulbond paired (702.95b).
    pub(crate) const PAIRING: Self = Self(1 << 29);
    pub(crate) const ATTACHMENT: Self = Self(1 << 12);
    pub(crate) const COMBAT: Self = Self(1 << 13);
    pub(crate) const TIMESTAMP: Self = Self(1 << 23);
    pub(crate) const LIFETIME: Self = Self(1 << 24);
    pub(crate) const VIEW: Self = Self(1 << 25);
    pub(crate) const PLAYER_STATUS: Self = Self(1 << 22);
    pub(crate) const LIFE: Self = Self(1 << 14);
    pub(crate) const HISTORY: Self = Self(1 << 15);
    pub(crate) const TURN: Self = Self(1 << 16);
    pub(crate) const COMMANDER: Self = Self(1 << 17);
    pub(crate) const BINDINGS: Self = Self(1 << 18);
    pub(crate) const CATALOG: Self = Self(1 << 19);
    pub(crate) const COLLECTIONS: Self = Self(1 << 20);
    pub(crate) const CONTINUATION: Self = Self(1 << 21);
    pub(crate) fn bits(self) -> u64 { self.0 }
    pub(crate) fn intersects(self, other: Self) -> bool { self.0 & other.0 != 0 }
}
impl BitOr for Properties { type Output = Self; fn bitor(self, rhs: Self) -> Self { Self(self.0 | rhs.0) } }
impl BitOrAssign for Properties { fn bitor_assign(&mut self, rhs: Self) { *self = *self | rhs; } }

/// Symbolic scope, never a list of currently matching objects. Broad domains deliberately
/// include ineligible objects and future members. Identity resolution belongs to runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Domain { Source, Recipient, Objects, Players, Context }
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Inputs {
    pub(crate) source: Properties,
    pub(crate) recipient: Properties,
    pub(crate) objects: Properties,
    pub(crate) players: Properties,
    pub(crate) context: Properties,
    pub(crate) unknown: bool,
}
impl Inputs {
    pub(crate) const NONE: Self = Self { source: Properties::NONE, recipient: Properties::NONE,
        objects: Properties::NONE, players: Properties::NONE, context: Properties::NONE, unknown: false };
    pub(crate) const UNKNOWN: Self = Self { source: Properties::ALL, recipient: Properties::ALL,
        objects: Properties::ALL, players: Properties::ALL, context: Properties::ALL, unknown: true };
    pub(crate) fn at(domain: Domain, properties: Properties) -> Self {
        let mut out = Self::NONE;
        *match domain { Domain::Source => &mut out.source, Domain::Recipient => &mut out.recipient,
            Domain::Objects => &mut out.objects, Domain::Players => &mut out.players, Domain::Context => &mut out.context } = properties;
        out
    }
    pub(crate) fn all(self) -> Properties { self.source | self.recipient | self.objects | self.players | self.context }
    /// Conservative lifting of a per-candidate predicate to its entire eligible domain.
    pub(crate) fn on_source(self) -> Self { Self { source: self.source | self.recipient, recipient: Properties::NONE, ..self } }
    pub(crate) fn over_objects(self) -> Self {
        Self { objects: self.objects | self.recipient, recipient: Properties::NONE, ..self }
    }
}
impl BitOr for Inputs {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self { Self { source: self.source | rhs.source,
        recipient: self.recipient | rhs.recipient, objects: self.objects | rhs.objects,
        players: self.players | rhs.players, context: self.context | rhs.context, unknown: self.unknown || rhs.unknown } }
}
impl BitOrAssign for Inputs { fn bitor_assign(&mut self, rhs: Self) { *self = *self | rhs; } }
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct EffectFootprint { pub(crate) reads: Inputs, pub(crate) writes: Inputs }
impl EffectFootprint {
    /// Property-only proof. Domain narrowing and runtime identities are deliberately not
    /// used yet; Unknown always requires exact trials. This does not establish ordering.
    pub(crate) fn may_depend_on(self, writer: Self) -> bool {
        self.reads.unknown || writer.writes.unknown || self.reads.all().intersects(writer.writes.all())
    }
}
