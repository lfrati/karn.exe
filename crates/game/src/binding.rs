//! Typed produced values and cross-ability links (CR 607.1, 607.2d).
//!
//! Frames and persistent choices use the same value vocabulary, with different lifetimes.
//! A link identifies a producer within one ability group. The object incarnation identifies
//! the storage owner; the record identifies the chooser. Neither is the current controller.
//! An object's own abilities form one group. Abilities an object is given are another group
//! for each time they are given ([`Origin`], 607.5); one that makes a linked choice is not
//! given yet. A missing runtime record is an undefined choice, not an invalid reference and
//! not a default color/type (607.5a).

use crate::state::{ObjId, PlayerId};
use crate::{Res, unsupported};
use ir::{Color, Subtype};

pub use ir::reference::Kind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Destructions(Vec<Destruction>),
    Removals(Vec<Removal>),
    Damage(Vec<crate::damage::CompletedDamage>),
    Cards(Vec<ObjId>),
    Subjects(Vec<crate::event::Target>),
    Consent(bool),
    Number(u32),
    Turn(Option<crate::turn_control::Anchor>),
    CreatedTurns(Vec<crate::turn_control::CreatedTurnId>),
    /// An exact CR 205.2a name. Unlike the executable characteristic vocabulary, choices
    /// include deferred card types too: choosing "plane" is legal and can match no library card.
    CardType(String),
    CardName(String),
    CreatureType(Subtype),
    Color(Color),
    /// One of the five basic land types (305.6).
    BasicLandType(Subtype),
}

/// A completed destruction's original/LKI object and actual resulting incarnation (400.7).
/// Death readers require the actual battlefield-to-graveyard transition (700.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destruction { pub before: ObjId, pub after: ObjId, pub from: crate::state::Zone, pub to: crate::state::Zone }

/// Actual counters removed by one completed effect instruction (122.1, 609.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removal { pub from: crate::Target, pub kind: String, pub count: u32 }

/// What a single value choice may pick: a kind of value and, of a card name, the
/// characteristics the card it names must have (201.4a): "choose a nonland card name".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Domain { pub kind: Kind, pub names: Names }

impl Domain {
    /// The decision that asks for a value of this domain.
    pub fn choice(self) -> crate::state::Choice {
        if self.names == Names::ANY { crate::state::Choice::Value(self.kind) } else { crate::state::Choice::Name(self.names) }
    }
}

impl From<Kind> for Domain {
    fn from(kind: Kind) -> Self { Domain { kind, names: Names::ANY } }
}

/// Which card names a choice of one may pick, by the card types and supertypes of the face
/// a name names (201.4a–f): one with every type of `all`, none of `none`, and, when `except`
/// is not empty, not every type of `except` ("other than a basic land card name").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Names { all: u16, none: u16, except: u16 }

impl Names {
    pub const ANY: Names = Names { all: 0, none: 0, except: 0 };

    /// "a nonland card name", "a creature card name", "a noncreature, nonland card name":
    /// the card types and supertypes a card set of these names says, when that is all it
    /// says; and the names it excepts, the same way.
    pub fn compile(set: &ir::ObjectSet, except: &[ir::action::Exclusion]) -> crate::Res<Names> {
        let mut names = Names::ANY;
        if set.domain != ir::Domain::Card || *set != (ir::ObjectSet { filter: set.filter.clone(), ..ir::ObjectSet::of(ir::Domain::Card) }) {
            return unsupported("card name choice: characteristics other than types");
        }
        names.read(&set.filter, true)?;
        for e in except {
            let ir::action::Exclusion::CardName(other) = e else { return unsupported("card name choice: excepted values other than names") };
            if other.domain != ir::Domain::Card || names.except != 0 { return unsupported("card name choice: excepted names"); }
            let mut excepted = Names::ANY;
            excepted.read(&other.filter, true)?;
            if excepted.none != 0 || excepted.all == 0 { return unsupported("card name choice: excepted names"); }
            names.except = excepted.all;
        }
        Ok(names)
    }

    fn read(&mut self, filter: &ir::Filter, positive: bool) -> crate::Res<()> {
        use ir::{Atom, Filter};
        match filter {
            Filter::All(items) if positive => items.iter().try_for_each(|f| self.read(f, true)),
            Filter::Not(inner) if positive => self.read(inner, false),
            Filter::Atom(Atom::Type(t)) => { self.bit(Self::card_type(*t), positive); Ok(()) }
            Filter::Atom(Atom::Supertype(t)) => { self.bit(Self::supertype(*t), positive); Ok(()) }
            _ => unsupported("card name choice: characteristics other than types"),
        }
    }

    fn bit(&mut self, bit: u16, positive: bool) {
        if positive { self.all |= bit } else { self.none |= bit }
    }
    fn card_type(t: ir::CardType) -> u16 { 1 << ir::CardType::ALL.iter().position(|c| *c == t).expect("a card type") }
    fn supertype(t: ir::Supertype) -> u16 { 1 << (ir::CardType::ALL.len() + ir::Supertype::ALL.iter().position(|c| *c == t).expect("a supertype")) }

    /// Does a face with these types ("Legendary Creature") qualify?
    fn admits_types(self, types: &str) -> bool {
        let mut has = 0;
        for word in types.split_whitespace().map(str::to_lowercase) {
            if let Some(t) = ir::CardType::from_word(&word) { has |= Self::card_type(t); }
            if let Some(t) = ir::Supertype::from_word(&word) { has |= Self::supertype(t); }
        }
        has & self.all == self.all && has & self.none == 0 && (self.except == 0 || has & self.except != self.except)
    }

    /// Does this card name qualify? It does when some face of that name does (201.4b–f).
    pub fn admits(self, name: &str) -> bool {
        self == Names::ANY || oracle::names::types(name).any(|t| self.admits_types(t))
    }
}

/// Legal answers to a single value choice (105.1, 205.3m). Frame-only kinds have
/// no finite choice domain and must not be used to ask this decision.
pub fn options(domain: impl Into<Domain>) -> Vec<Value> {
    let Domain { kind, names: within } = domain.into();
    match kind {
        Kind::CardName => {
            let mut names: Vec<_> = oracle::names::all().iter().filter(|n| within.admits(n)).cloned().collect();
            names.sort();
            names.into_iter().map(Value::CardName).collect()
        },
        Kind::CardType => ir::vocab::list("card types").iter().cloned().map(Value::CardType).collect(),
        Kind::CreatureType => crate::state::creature_types().into_iter().map(Value::CreatureType).collect(),
        Kind::Color => [Color::White, Color::Blue, Color::Black, Color::Red, Color::Green]
            .into_iter().map(Value::Color).collect(),
        Kind::BasicLandType => Subtype::basic_land_types().iter().copied().map(Value::BasicLandType).collect(),
        _ => Vec::new(),
    }
}
pub fn accepts(domain: impl Into<Domain>, value: &Value) -> bool {
    let Domain { kind, names } = domain.into();
    match (kind, value) {
        (Kind::CardType, Value::CardType(t)) => ir::vocab::list("card types").contains(t),
        (Kind::CreatureType, Value::CreatureType(t)) => t.kind() == ir::vocab::SubtypeKind::Creature,
        (Kind::Color, Value::Color(_)) => true,
        (Kind::CardName, Value::CardName(name)) => oracle::names::is_name(name) && names.admits(name),
        (Kind::BasicLandType, Value::BasicLandType(t)) => t.is_basic_land_type(),
        _ => false,
    }
}


/// One giving of abilities to an object (613.1f): two effects that give the same words are
/// two of these, so each gives its own instance and one ending leaves the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grant {
    /// The permanent whose static ability gives it; None of an effect that resolved, which
    /// its timestamp alone tells apart (613.7b).
    pub by: Option<ObjId>,
    /// The timestamp of the effect that gives it (613.7).
    pub stamp: u32,
    /// Which of the things that effect gives, where it gives several.
    pub part: u32,
}

/// Which definition an ability an object has belongs to, and what gave it to the object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// The object's own definition: the face of its card, a predefined token's rule text.
    Own,
    /// One selected activated family acquired from another object (113.10c, 602.5c).
    Inherited(std::sync::Arc<crate::ability::InheritedFamily>),
    /// One acquisition of copied text (707.7); immutable programs keep this identity.
    Copied(u32),
    /// The rules give it to what has a characteristic: the mana ability of a basic land
    /// type (305.6).
    Rule(crate::ability::Template),
    /// An effect gives it (613.1f).
    Given { grant: Grant, what: crate::ability::Template },
}
impl Origin {
    pub fn name(self) -> &'static str {
        match self { Origin::Own => "own", Origin::Inherited(_) => "inherited", Origin::Copied(_) => "copied", Origin::Rule(_) => "rule", Origin::Given { .. } => "given" }
    }
}

/// An ability's provenance, independent of the object currently carrying it: the
/// definition it belongs to and its place among that definition's abilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbilityKey {
    pub origin: Origin,
    pub index: usize,
}
impl AbilityKey {
    pub fn printed(index: usize) -> Self {
        Self { origin: Origin::Own, index }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkId(pub AbilityKey);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub link: LinkId,
    pub chooser: PlayerId,
    pub value: Value,
}

/// Whether a producer supplies the value constraint written by the grammar.
pub(crate) fn fits(expected: Kind, actual: Kind) -> bool {
    expected == actual || expected == Kind::Type && matches!(actual, Kind::CardType | Kind::CreatureType | Kind::BasicLandType)
}

/// Definition-time producer identities. Unresolved "type" is refined from a producer,
/// never defaulted to a creature subtype. Unrelated same-kind producers remain ambiguous.
#[derive(Debug, Clone)]
pub struct Links {
    group: Origin,
    abilities: usize,
    producers: Vec<(usize, Kind)>,
    reads: Vec<(AbilityKey, Kind, Res<(LinkId, Kind)>)>,
}
impl Default for Links {
    fn default() -> Self { Self::group(Origin::Own, 0, &[]) }
}
impl Links {
    /// Does an ability of the group make a choice another of them reads?
    pub(crate) fn produces(&self) -> bool { !self.producers.is_empty() }

    pub fn group(group: Origin, abilities: usize, producers: &[(usize, Kind)]) -> Self {
        let mut result = Self { group: group.clone(), abilities, producers: producers.to_vec(), reads: Vec::new() };
        for index in 0..abilities {
            for expected in [Kind::Type, Kind::CardType, Kind::CreatureType, Kind::Color, Kind::CardName, Kind::BasicLandType] {
                let reader = AbilityKey { origin: group.clone(), index };
                let bound = match result.candidates(reader.clone(), expected).expect("known ability").as_slice() {
                    [one] => Ok(one.clone()),
                    [] => unsupported(format!("linked choice: no {expected:?} producer (607.2d)")),
                    _ => unsupported(format!("linked choice: ambiguous {expected:?} producers (607.2d)")),
                };
                result.reads.push((reader, expected, bound));
            }
        }
        result
    }
    pub(crate) fn candidates(&self, reader: AbilityKey, expected: Kind) -> Res<Vec<(LinkId, Kind)>> {
        if reader.origin != self.group || reader.index >= self.abilities {
            return unsupported("linked choice: unknown ability provenance (607.5)");
        }
        Ok(self.producers.iter().filter(|(_, kind)| fits(expected, *kind))
            .map(|(index, kind)| (LinkId(AbilityKey { origin: self.group.clone(), index: *index }), *kind)).collect())
    }
    pub(crate) fn resolve(&self, reader: AbilityKey, expected: Kind) -> Res<(LinkId, Kind)> {
        self.reads.iter().find(|(r, kind, _)| *r == reader && *kind == expected)
            .map(|(_, _, value)| value.clone())
            .unwrap_or_else(|| unsupported("linked choice: unknown ability provenance or kind (607.5)"))
    }
    pub fn bind(&self, reader: AbilityKey, kind: Kind) -> Res<LinkId> {
        self.resolve(reader, kind).map(|(link, _)| link)
    }
}

pub(crate) fn compile(def: &crate::card::CardDef) -> Links {
    let producers: Vec<_> = def
        .abilities
        .iter()
        .enumerate()
        .filter_map(|(index, a)| match crate::replace::as_enters(a) {
            Some(crate::replace::AsEnters::Value(domain) | crate::replace::AsEnters::ValueThenLife(domain, ..)) => Some((index, domain.kind)),
            _ => None,
        })
        .collect();
    Links::group(Origin::Own, def.abilities.len() + def.keyword_abilities.len(), &producers)
}

/// A scalar operand is bound once to either this execution's slot or a linked cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scalar {
    Local { slot: usize, kind: Kind },
    Linked { link: LinkId, kind: Kind },
}
impl Scalar {
    pub fn kind(self) -> Kind {
        match self { Self::Local { kind, .. } | Self::Linked { kind, .. } => kind }
    }
}

/// A static casting-cost ability that can exile cards (607.2q).
pub(crate) fn cost_exile_writer(def: &crate::CardDef, row: usize) -> bool {
    let Some(a) = def.abilities.get(row) else { return false };
    // "behold a Kithkin and exile it" too
    let exiles = |cost: &ir::Cost| cost.0.iter().any(|p| match p {
        ir::CostPart::Action(ir::Action::Exile(_)) => true,
        ir::CostPart::Joined(joined) => joined.items.iter().any(|a| matches!(a, ir::Action::Exile(_))),
        _ => false,
    });
    matches!(a, ir::Ability::Keywords(items) if items.iter().any(|i| i.keyword.name() == "Delve" && i.param == ir::token::KeywordParam::None))
        || crate::functions::casting_cost(a).is_some_and(exiles)
}

/// Identify the printed exile producer for a linked reader (607.2a).
/// An explicit linked-card reader can occur in its own producing ability.
/// Resolve at definition loading; runtime reads an identified writer, never scans text.
pub(crate) fn exile_producer(def: &crate::CardDef, reader: AbilityKey, explicit: bool) -> Res<AbilityKey> {
    fn sentence(s: &ir::Sentence, cards_only: bool) -> bool {
        match s {
            ir::Sentence::Do(body) => std::iter::once(&body.first).chain(body.rest.iter().map(|(_, c)| c))
                .any(|c| std::iter::once(&c.first).chain(c.rest.iter().map(|(_, a)| a)).any(|a| matches!(&a.action, ir::Action::Exile(objects) if !cards_only || objects.items.iter().all(|s| s.set.domain == ir::Domain::Card)))),
            ir::Sentence::Led { rest, .. } => sentence(rest, cards_only),
            _ => false,
        }
    }
    // a keyword's own triggered ability exiles too, as hideaway's does (702.75a)
    let writers: Vec<_> = def.abilities.iter().chain(def.keyword_abilities.iter().map(|(_, a)| a)).enumerate().filter_map(|(i, a)| {
        if cost_exile_writer(def, i) { return Some(AbilityKey::printed(i)); }
        let e = match a { ir::Ability::Triggered { effect, .. } | ir::Ability::Activated { effect, .. } | ir::Ability::Chapter { effect, .. } | ir::Ability::Text(effect) => effect, _ => return None };
        let exiles = e.0.iter().any(|s| sentence(s, explicit)) || def.modes.get(i).into_iter().flatten().any(|m| matches!(m, ir::Mode::Bullet(e) if e.0.iter().any(|s| sentence(s, explicit))));
        exiles.then_some(AbilityKey::printed(i))
    }).collect();
    if explicit && writers.contains(&reader) { return Ok(reader); }
    if writers.contains(&reader) { return unsupported("reference: the exiled card, of the ability that exiles it"); }
    match writers.as_slice() {
        [writer] => Ok(writer.clone()),
        _ => unsupported("reference: the exiled card, with no one ability of the object that exiles"),
    }
}

impl crate::retention::Trace for Value {
    fn trace(&self, visit: &mut dyn FnMut(ObjId)) {
        match self {
            Value::Damage(results) => for result in results { result.recipient.trace(visit); result.sources.trace(visit); },
            Value::Removals(results) => for result in results { result.from.trace(visit); },
            Value::Destructions(results) => for result in results { visit(result.before); visit(result.after); },
            Value::Cards(cards) => cards.trace(visit),
            Value::Subjects(subjects) => subjects.trace(visit),
            Value::Turn(_) | Value::CreatedTurns(_) | Value::Consent(_) | Value::Number(_) | Value::CardType(_) | Value::CreatureType(_) | Value::Color(_) | Value::CardName(_) | Value::BasicLandType(_) => {},
        }
    }
}
impl crate::retention::Players for Value {
    fn players(&self, visit: &mut dyn FnMut(crate::PlayerId)) {
        match self {
            Value::Damage(results) => for result in results { result.recipient.players(visit); },
            Value::Removals(results) => for result in results { result.from.players(visit); },
            Value::Subjects(subjects) => subjects.players(visit),
            Value::Destructions(_) | Value::Cards(_) | Value::Turn(_) | Value::CreatedTurns(_) | Value::Consent(_) | Value::Number(_)
            | Value::CardType(_) | Value::CreatureType(_) | Value::Color(_) | Value::CardName(_) | Value::BasicLandType(_) => {},
        }
    }
}
impl crate::retention::Trace for Record {
    fn trace(&self, visit: &mut dyn FnMut(ObjId)) {
        // LinkId/Grant.by identify an ability instance; its immutable definition is held
        // by the key and does not inspect the granting object's payload.
        self.value.trace(visit);
    }
}
