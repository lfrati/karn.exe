//! What a permanent is now: its characteristics after the continuous effects that apply to it
//! (CR 611), applied in layers (613).
//!
//! A continuous effect comes from a static ability, and then applies at any moment to whatever
//! its text describes for as long as its permanent is on the battlefield (611.3a–b), or from a
//! spell or ability that resolved, and then applies to the objects it affected when it began
//! (611.2c) for as long as it says (611.2a). The effects that apply to a permanent are applied
//! in the order of their layers (613.1), and within a layer in dependency and timestamp order (613.7–8); an
//! effect sees the partial world left by preceding effects, so "creatures with flying
//! get +1/+1" counts a creature that gained flying, and "creatures with power 4 or greater
//! have trample" does not count what only a later layer makes that big.
//!
//! Everything that asks what a permanent's types, colors, power, toughness or abilities are
//! asks here ([`Game::seen`]). So far the layers of types (4), colors (5), abilities (6),
//! and power and toughness (7a: characteristic definitions; 7b: set; 7c: changed, with counters).

use crate::event::Target;
use crate::select::{Ctx, name};
use crate::state::{Game, ObjId, Zone};
use crate::statement::Statement;
use crate::{Res, unsupported};
use ir::action::{Becoming, Characteristics, Granted, Keeps, SetTo, Setting};
use ir::vocab::SubtypeKind;
use ir::effect::{Duration, Modifier};
use ir::event::{Step, StepOwner, Time};
use ir::statics::StaticAct;
use crate::ability::Template;
use crate::binding::{AbilityKey, Grant, Origin};
use crate::keyword::Held;
use crate::select::Support;
use ir::token::Pt;
use ir::{Ability, Action, CardType, Color, Keyword, Stat, Subtype};

/// A characteristic world: partial during layer evaluation, or frozen immediately before
/// an event until its triggers are delivered. Occurrences cannot retain this query view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Working {
    pub(crate) objects: Vec<(ObjId, Seen)>,
    slots: std::collections::HashMap<ObjId, usize>,
    all: Vec<usize>,
    /// Immutable zone eligibility in this query; characteristic trials never move objects.
    live: Option<Vec<usize>>,
    zones: Option<[Vec<usize>; 8]>,
    /// A look-back view also freezes battlefield membership; partial layer worlds do not.
    battlefield: Option<Vec<ObjId>>,
    /// Exact pre-event edges, including endpoints that departed together.
    attachments: Option<Vec<(ObjId, Target)>>,
}
impl Working {
    pub(crate) fn before(objects: Vec<(ObjId, Seen)>) -> Self {
        let battlefield = Some(objects.iter().map(|(id, _)| *id).collect());
        let mut world = Self::new(objects);
        world.battlefield = battlefield;
        world
    }
    pub(crate) fn freeze_attachments(mut self, edges: Vec<(ObjId, Target)>) -> Self {
        self.attachments = Some(edges);
        self
    }
    pub(crate) fn attachments(&self) -> Option<&[(ObjId, Target)]> { self.attachments.as_deref() }
    fn new(objects: Vec<(ObjId, Seen)>) -> Self {
        let slots = objects.iter().enumerate().map(|(at, (id, _))| (*id, at)).collect();
        let all = (0..objects.len()).collect();
        Self { objects, slots, all, live: None, zones: None, battlefield: None, attachments: None }
    }
    fn slot(&self, id: ObjId) -> Option<usize> {
        if trials_only() { self.objects.iter().position(|(on, _)| *on == id) }
        else { self.slots.get(&id).copied() }
    }
    pub(crate) fn battlefield(&self) -> Option<&[ObjId]> { self.battlefield.as_deref() }
    pub(crate) fn get(&self, id: ObjId) -> Option<&Seen> {
        self.slot(id).map(|at| &self.objects[at].1)
    }
}

/// The layers the engine applies, in order (613.1d–g, 613.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    /// Layer 2: control-changing effects (613.1b).
    Control,
    /// Layer 4: card types, subtypes and supertypes changed (613.1d).
    Types,
    /// Layer 5: colors changed (613.1e).
    Colors,
    /// Layer 6: abilities added and removed.
    Abilities,
    /// Layer 7a: characteristic-defining power and toughness (604.3, 613.4a).
    DefinePt,
    /// Layer 7b: power and toughness set to specific values.
    SetPt,
    /// Layer 7c: power and toughness changed without being set, and counters.
    ChangePt,
}

const LAYERS: [Layer; 7] = [Layer::Control, Layer::Types, Layer::Colors, Layer::Abilities, Layer::DefinePt, Layer::SetPt, Layer::ChangePt];

/// What a question reads of what an object is. Layers apply in order (613.1), so a question
/// about a characteristic is settled once its layer has been applied: nothing a later layer
/// does is read by it, and the layers after it need not be evaluated to answer it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aspect {
    /// Its card types, subtypes and supertypes (and its controller, settled before them).
    Types,
    /// Its colors, and what [`Aspect::Types`] reads.
    Colors,
    /// The abilities it has, and what [`Aspect::Colors`] reads.
    Abilities,
    /// Everything, power and toughness included.
    All,
}
impl Aspect {
    /// The first layer the question does not need: the `before` of [`Game::seen`].
    pub(crate) fn cut(self) -> Option<Layer> {
        match self {
            Aspect::Types => Some(Layer::Colors),
            Aspect::Colors => Some(Layer::Abilities),
            Aspect::Abilities => Some(Layer::DefinePt),
            Aspect::All => None,
        }
    }
    /// That cut, within a view that itself stops before `layer`.
    pub(crate) fn within(self, layer: Option<Layer>) -> Option<Layer> {
        match (layer, self.cut()) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
}

/// Does this number ask what a power or toughness is ("its power", "the greatest toughness
/// among creatures you control", "for each creature with power 4 or greater")?
pub(crate) fn asks_pt(a: &ir::Amount) -> bool {
    use ir::{Amount, Atom, Filter, Stat};
    /// A power, a toughness, or their sum.
    fn pt(stat: &Stat) -> bool {
        matches!(stat, Stat::Power | Stat::Toughness | Stat::PowerAndToughness)
    }
    fn described(f: &Filter) -> bool {
        match f {
            Filter::Atom(Atom::Stat { stat, value, .. }) => pt(stat) || asks_pt(value),
            Filter::Atom(_) => false,
            Filter::Not(f) => described(f),
            Filter::All(v) | Filter::Any(v) => v.iter().any(described),
        }
    }
    match a {
        Amount::Stat { stat, .. } => pt(stat),
        Amount::Aggregate { stat, among, .. } => pt(stat) || described(&among.filter),
        Amount::Count(set) | Amount::CountersAmong { among: set, .. } => described(&set.filter),
        _ => false,
    }
}

/// What a continuous effect does to what it affects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Control(crate::PlayerId),
    /// "becomes a 2/2 creature with all creature types", "that's still a land" (613.1d).
    Types(Types),
    /// "isn't a creature": these card types are removed, and with them each subtype that is
    /// not also one of a card type it still has (205.1a, 613.1d).
    NotTypes(Vec<CardType>),
    /// "becomes blue" (613.1e); `added`: "in addition to its other colors".
    Colors { colors: Vec<Color>, added: bool },
    /// "gains flying", "have haste" (613.1f).
    Gains(Vec<Held>),
    /// "gains \"{T}: Add {G}.\"" (113.2c, 613.1f): each the shared definition of the words
    /// given. Which giving it is, the effect that holds this says ([`Grant`]).
    Gives(Vec<Template>),
    /// Dynamic battlefield donor membership, evaluated from the partial layer world.
    Inherits(crate::program::expr::Set),
    /// Exact selected activated members produced by one application in layer six.
    Inherited(Vec<AbilityKey>),
    /// Remove current definition, intrinsic, granted and keyword abilities (113.10).
    LosesAll,
    /// Remove the specified passive keyword instances (613.1f).
    Loses(Vec<Held>),
    /// "has base power and toughness 1/1", "becomes a 2/2 creature" (613.4b).
    BasePt(i32, i32),
    DefinedPt(Option<i32>, Option<i32>),
    /// "gets +3/+3", "get -1/-1" (613.4c).
    Pt(i32, i32),
}

/// What a type-changing effect makes of an object's card types and subtypes (205.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Types {
    pub types: Vec<CardType>,
    pub subtypes: Vec<Subtype>,
    /// "with all creature types": every creature type (205.3m).
    pub every_creature_type: bool,
    /// The prior card types, subtypes and supertypes are kept: "in addition to its other
    /// types", "it's still a land", "becomes an artifact creature" (205.1b). Otherwise the new
    /// card types replace the old (205.1a).
    pub keeps: bool,
}

impl Change {
    pub(crate) fn writes(&self) -> crate::program::footprint::Properties {
        use crate::program::footprint::Properties as P;
        match self {
            Self::Control(_) => P::CONTROL,
            Self::Types(t) => P::TYPES | if t.sets_basic_land_type() { P::ABILITIES } else { P::NONE },
            Self::NotTypes(_) => P::TYPES,
            Self::Colors { .. } => P::COLORS,
            Self::Gains(_) | Self::Loses(_) => P::KEYWORDS,
            Self::Gives(_) | Self::Inherits(_) | Self::Inherited(_) => P::MEMBERSHIP,
            Self::LosesAll => P::ABILITIES,
            Self::BasePt(..) | Self::DefinedPt(..) | Self::Pt(..) => P::PT,
        }
    }

    /// "gets +1/+1 for each creature you control": so many times the change in power and
    /// toughness.
    pub(crate) fn times(self, n: u32) -> Change {
        match self {
            Change::Pt(p, t) => Change::Pt(p * n as i32, t * n as i32),
            other => other,
        }
    }

    /// A resolving for-each modifier must retain the exact signed P/T product.
    /// Counts fit u32, but their modifiers may exceed the represented i32 change.
    pub(crate) fn checked_times(self, n: u32) -> crate::Res<Change> {
        let scale = |value: i32| i32::try_from(i64::from(value) * i64::from(n))
            .map_err(|_| crate::Unsupported("continuous P/T change exceeds the engine numerical range".into()));
        Ok(match self {
            Change::Pt(p, t) => Change::Pt(scale(p)?, scale(t)?),
            other => other,
        })
    }

    pub(crate) fn removes_abilities(&self) -> bool {
        matches!(self, Self::LosesAll) || matches!(self, Self::Types(t) if t.sets_basic_land_type())
    }
    pub(crate) fn changes_membership(&self) -> bool { self.removes_abilities() || matches!(self, Self::Gives(_) | Self::Inherits(_) | Self::Inherited(_)) }

    pub(crate) fn layer(&self) -> Layer {
        match self {
            Change::Control(_) => Layer::Control,
            Change::Types(_) | Change::NotTypes(_) => Layer::Types,
            Change::Colors { .. } => Layer::Colors,
            Change::Gains(_) | Change::Gives(_) | Change::Inherits(_) | Change::Inherited(_) | Change::LosesAll | Change::Loses(_) => Layer::Abilities,
            Change::BasePt(..) => Layer::SetPt,
            Change::DefinedPt(..) => Layer::DefinePt,
            Change::Pt(..) => Layer::ChangePt,
        }
    }
}

/// The card types whose subtypes these are (205.3): a creature type is a creature's and a
/// kindred's (205.3m), a spell type an instant's and a sorcery's (205.3k).
fn of_types(kind: SubtypeKind) -> &'static [CardType] {
    match kind {
        SubtypeKind::Artifact => &[CardType::Artifact],
        SubtypeKind::Enchantment => &[CardType::Enchantment],
        SubtypeKind::Land => &[CardType::Land],
        SubtypeKind::Planeswalker => &[CardType::Planeswalker],
        SubtypeKind::Spell => &[CardType::Instant, CardType::Sorcery],
        SubtypeKind::Creature => &[CardType::Creature, CardType::Kindred],
        SubtypeKind::Battle => &[CardType::Battle],
    }
}

impl Types {
    pub(crate) fn sets_basic_land_type(&self) -> bool {
        !self.keeps && self.subtypes.iter().any(|s| s.is_basic_land_type())
    }
    /// Apply it to what an object is (205.1). Keeping the old, it adds the new. Otherwise
    /// the new card types replace the old (an instant or sorcery stays one); a subtype stays
    /// while it is one of a card type the object still has, and a new subtype replaces the
    /// old ones of its own kind (205.1a).
    pub(crate) fn apply(&self, seen: &mut Seen) {
        let mut new_subtypes = self.subtypes.clone();
        // 205.3d: every creature type is still a subtype, and requires creature/kindred.
        let creature_kind = self.types.iter().any(|t| matches!(t, CardType::Creature | CardType::Kindred))
            || ((self.keeps || self.types.is_empty()) && seen.types.iter().any(|t| matches!(t, CardType::Creature | CardType::Kindred)));
        if self.every_creature_type && creature_kind {
            new_subtypes.extend_from_slice(Subtype::of_kind(SubtypeKind::Creature));
        }
        if self.keeps {
            add(&mut seen.types, &self.types);
            add(&mut seen.subtypes, &new_subtypes);
            return;
        }
        if !self.types.is_empty() {
            let spell: Vec<CardType> = seen.types.iter().copied().filter(|t| matches!(t, CardType::Instant | CardType::Sorcery)).collect();
            seen.types = self.types.clone();
            add(&mut seen.types, &spell);
        }
        // Setting a basic land type removes rules-text/copied abilities in layer 4,
        // but keeps granted abilities and supplies the new type's mana ability (305.7).
        if seen.is(CardType::Land) && self.sets_basic_land_type() {
            seen.base_abilities = false;
            seen.keywords.clear();
        }
        let kinds: Vec<SubtypeKind> = new_subtypes.iter().map(|s| s.kind()).collect();
        let types = seen.types.clone();
        seen.subtypes.retain(|s| !kinds.contains(&s.kind()) && of_types(s.kind()).iter().any(|t| types.contains(t)));
        add(&mut seen.subtypes, &new_subtypes);
    }

}

/// Add these to the list, each once.
fn add<T: PartialEq + Clone>(list: &mut Vec<T>, new: &[T]) {
    for x in new {
        if !list.contains(x) {
            list.push(x.clone());
        }
    }
}

/// A resolved duration is permanently removed once it ends (611.2b).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifetime {
    Forever,
    Turn,
    Controlled { source: ObjId, player: crate::PlayerId },
    /// Remains on the battlefield, regardless of later controller (611.2b).
    SourcePresent { source: ObjId },
}
impl Lifetime {
    fn holds(self, game: &Game) -> bool {
        match self {
            Self::SourcePresent { source } => game.get_object(source).is_some_and(|o| o.zone == Zone::Battlefield),
            Self::Controlled { source, player } => game.get_object(source).is_some_and(|o|
                matches!(o.zone, Zone::Battlefield | Zone::Stack) && game.controller(source) == player),
            _ => true,
        }
    }
}

/// A continuous effect of a spell or ability that resolved (611.2): what it changes, of the
/// objects it affected when it began (611.2c), with the timestamp it received then (613.7b).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Continuous {
    /// Frozen printed producer, not a live-source lookup or a payload-retention edge.
    pub producer: Option<std::sync::Arc<ResolvedProducer>>,
    pub change: Change,
    pub affected: Vec<ObjId>,
    pub stamp: u32,
    /// Its place among the changes that began with it and share its timestamp.
    pub part: u32,
    pub lifetime: Lifetime,
}

/// Immutable provenance for one instruction occurrence. The source is an identity
/// label: resolved fixed scalars do not read its later object or characteristics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProducer {
    pub(crate) source: ObjId,
    pub(crate) definition: usize,
    pub(crate) paragraph: usize,
    pub(crate) controller: crate::PlayerId,
}

/// One continuous effect, retaining its identity while its applicability is recomputed.
#[derive(Clone)]
struct Effect<'a> {
    stamp: u32,
    /// The other timestamps an effect of a static ability takes the later of (613.7a), each
    /// given at once with `stamp` ([`Game::later_stamp`]); `stamp` is the later in the
    /// engine's own order of them.
    also: Vec<u32>,
    by: Option<ObjId>,
    part: u32,
    body: Body<'a>,
    started: Option<Vec<ObjId>>,
}
#[derive(Clone)]
enum Body<'a> {
    Static { statement: &'a crate::program::registration::Characteristic, act: usize, key: AbilityKey, only: Option<ObjId> },
    Resolved(&'a Continuous),
    Counter { on: ObjId, keyword: Keyword },
    /// What a bestowed Aura spell or Aura is by its bestow ability: an Aura enchantment, with
    /// the object's timestamp (702.103b, 613.7a).
    Bestowed { on: ObjId },
}
/// Potential instances indexed by layer, independent of current applicability. In
/// particular a grant whose producing instance is absent still has an indexed reader.
struct LayerCandidates { layers: [Vec<usize>; 7], registered: usize }
impl LayerCandidates {
    fn new() -> Self { Self { layers: std::array::from_fn(|_|Vec::new()), registered: 0 } }
    fn extend(&mut self, effects: &[Effect]) {
        for (i,effect) in effects.iter().enumerate().skip(self.registered) {
            for layer in LAYERS { if effect.in_layer(layer) { self.layers[layer as usize].push(i); } }
        }
        self.registered = effects.len();
    }
}
type Evaluated = Vec<(ObjId, Vec<Change>)>;
impl Effect<'_> {
    fn in_layer(&self, layer: Layer) -> bool {
        match &self.body {
            Body::Static { statement, act, .. } => statement.part_in(*act, layer),
            Body::Resolved(c) => c.change.layer() == layer,
            Body::Counter { .. } => layer == Layer::Abilities,
            Body::Bestowed { .. } => layer == Layer::Types,
        }
    }
    fn footprint(&self, layer: Layer) -> crate::program::footprint::EffectFootprint {
        use crate::program::footprint::{Domain, EffectFootprint, Inputs, Properties};
        match &self.body {
            Body::Static { statement, act, .. } => statement.footprint(*act, layer),
            Body::Resolved(c) => EffectFootprint { reads: Inputs::NONE,
                writes: Inputs::at(Domain::Recipient, c.change.writes()) },
            Body::Counter { .. } => EffectFootprint { reads: Inputs::NONE,
                writes: Inputs::at(Domain::Recipient, Properties::KEYWORDS) },
            Body::Bestowed { .. } => EffectFootprint { reads: Inputs::NONE,
                writes: Inputs::at(Domain::Recipient, Properties::TYPES) },
        }
    }
    fn evaluate(&self, game: &Game, world: &Working, layer: Layer) -> Evaluated {
        #[cfg(feature = "profile")]
        crate::profile::count(crate::profile::Count::Evaluations, 1);
        // The source and its ability membership are constant during one evaluation.
        // Resolve them once, not once for every possible recipient in the world.
        let ctx = if let Body::Static { key, .. } = &self.body {
            let source = self.by.expect("static source");
            let seen = world.get(source).expect("source in layer world");
            if self.started.is_none() && !seen.has_origin(key.origin.clone()) { return Vec::new(); }
            Some(Ctx { view: Some(world), layer: Some(layer),
                ..Ctx::new(seen.controller, source).with_key(key.clone()) })
        } else { None };
        let candidates = self.candidates(game, world, layer);
        #[cfg(test)]
        VISITS.with(|n|n.set(n.get()+candidates.len()));
        #[cfg(feature = "profile")]
        crate::profile::count(crate::profile::Count::CandidateObjects, candidates.len());
        candidates.iter().filter_map(|at| {
            let (id, _) = &world.objects[*at];
            let obj = game.obj(*id);
            if obj.zone != Zone::Battlefield && !(obj.zone == Zone::Stack && obj.ability.is_none()) && !reaches_every_zone(layer) { return None; }
            let mut changes = match &self.body {
                Body::Static { statement, act, only, .. } => {
                    if only.is_some_and(|on| on != *id) { return None; }
                    let ctx = ctx.as_ref().expect("static context");
                    if let Some(started) = &self.started {
                        if !started.contains(id) { return None; }
                    } else if !statement.subject.contains(game, ctx, Target::Object(*id)) { return None; }
                    statement.part_changes(game, ctx, layer, *act, self.started.is_some())
                }
                Body::Counter { on, keyword } => {
                    if on != id { return None; }
                    vec![Change::Gains(vec![Held::plain(*keyword)])]
                }
                Body::Bestowed { on } => {
                    if on != id { return None; }
                    vec![Change::Types(crate::bestow::types().clone())]
                }
                Body::Resolved(c) => {
                    let prior_spell = game.prospective.filter(|(on, _)| on == id).and_then(|(_, was)| was)
                        .filter(|was| game.obj(*was).zone == Zone::Stack && game.obj(*was).ability.is_none());
                    if !c.affected.contains(id) && !prior_spell.is_some_and(|was| c.affected.contains(&was)) { return None; }
                    vec![c.change.clone()]
                }
            };
            if let Some(ctx) = &ctx {
                for (change_index, change) in changes.iter_mut().enumerate() {
                    if let Change::Inherits(from) = change {
                        let Body::Static { key, act, .. } = &self.body else { unreachable!("static context") };
                        let source = self.by.expect("inheritance granting source");
                        let grant_definition = game.frozen_definition(source, &key.origin).expect("static definition retained");
                        let grant = Grant { by: self.by, stamp: self.stamp, part: self.part };
                        let mut selected = Vec::new();
                        for (donor, donor_seen) in &world.objects {
                            if !from.contains(game, ctx, &crate::program::expr::NoLocals, Target::Object(*donor)) { continue; }
                            // Read this one partial world, never a recursive live ability query.
                            for member in game.members_seen(*donor, donor_seen, true) {
                                if !matches!(&*member, ir::Ability::Activated { .. }) { continue; }
                                let definition = game.frozen_definition(*donor, &member.key.origin).expect("donor member definition retained");
                                let family = crate::ability::InheritedFamily {
                                    grant, part: (*act, change_index), grant_ability: key.clone(), grant_definition: grant_definition.clone(),
                                    donor: *donor, donor_origin: member.key.origin.clone(), definition,
                                };
                                selected.push(AbilityKey { origin: Origin::Inherited(std::sync::Arc::new(family)), index: member.key.index });
                            }
                        }
                        *change = Change::Inherited(selected);
                    }
                }
            }
            (!changes.is_empty()).then_some((*id, changes))
        }).collect()
    }
    /// Candidate membership is broader than applicability: no type, color, keyword or
    /// producing-Origin predicate is used to discard a possible recipient. Trial patches
    /// can enable those predicates. Identity and zone membership cannot change here.
    fn candidates<'w>(&self, game: &Game, world: &'w Working, layer: Layer) -> std::borrow::Cow<'w, [usize]> {
        use crate::program::registration::Subject;
        let all = || if !trials_only() && !reaches_every_zone(layer) {
            std::borrow::Cow::Borrowed(world.live.as_deref().unwrap_or(&world.all))
        } else { std::borrow::Cow::Borrowed(world.all.as_slice()) };
        if trials_only() { return all(); }
        let ids = if let Some(started) = &self.started { Some(started.clone()) } else {
            match &self.body {
                Body::Static { statement, only, .. } => if let Some(on) = only { Some(vec![*on]) } else {
                    match statement.subject {
                        Subject::Source => Some(self.by.into_iter().collect()),
                        Subject::Controller => Some(Vec::new()),
                        Subject::Attached => Some(self.by.and_then(|by| game.attached_to(by))
                            .and_then(|target| match target { Target::Object(on) => Some(on), _ => None }).into_iter().collect()),
                        Subject::Described(ref set) => {
                            if game.prospective.is_none() && let (Some(mask), Some(zones)) = (set.layer_zones(), &world.zones) {
                                if mask == 0 { return Vec::new().into(); }
                                if mask.count_ones() == 1 { return zones[mask.trailing_zeros() as usize].as_slice().into(); }
                                let mut slots: Vec<_> = zones.iter().enumerate().filter(|(z,_)|mask & (1<<z) != 0)
                                    .flat_map(|(_,slots)|slots.iter().copied()).collect();
                                slots.sort_unstable(); return slots.into();
                            }
                            None
                        }
                        Subject::SourceFilter(_) => None,
                    }
                },
                Body::Counter { on, .. } | Body::Bestowed { on } => Some(vec![*on]),
                Body::Resolved(c) => {
                    let mut ids = c.affected.clone();
                    if let Some((entrant, Some(was))) = game.prospective {
                        if c.affected.contains(&was) && game.obj(was).zone == Zone::Stack && game.obj(was).ability.is_none() { ids.push(entrant); }
                    }
                    Some(ids)
                }
            }
        };
        match ids {
            None => all(),
            Some(ids) => {
                let mut slots: Vec<_> = ids.into_iter().filter_map(|id|world.slot(id)).collect();
                slots.sort_unstable(); slots.dedup(); // Preserve the original world order.
                slots.into()
            }
        }
    }
    fn commit(&mut self, world: &mut Working, evaluated: &Evaluated) {
        // Once a single effect starts in an earlier layer, its later components continue
        // to affect the same objects even if its subject or existence changes (613.6).
        if self.started.is_none() && !evaluated.is_empty() {
            if let Body::Static { statement, act, .. } = &self.body {
                if LAYERS.iter().filter(|layer| statement.part_in(*act, **layer)).count() > 1 {
                    self.started = Some(evaluated.iter().map(|(id, _)| *id).collect());
                }
            }
        }
        self.apply(world, evaluated);
    }
    fn apply(&self, world: &mut Working, evaluated: &Evaluated) {
        for (id, changes) in evaluated {
            let at = world.slot(*id).expect("evaluated object");
            let seen = &mut world.objects[at].1;
            let mut grant_index = match &self.body {
                Body::Static { statement, act, .. } => statement.grant_offset(*act),
                Body::Resolved(_) | Body::Counter { .. } | Body::Bestowed { .. } => 0,
            };
            for change in changes {
                match change {
                    Change::Gives(templates) => {
                        for what in templates {
                            seen.given.push((Grant { by: self.by, stamp: self.stamp, part: self.part << 16 | grant_index }, *what));
                            grant_index += 1;
                        }
                    },
                    Change::Inherited(keys) => seen.inherited.extend(keys.iter().cloned()),
                    Change::Inherits(_) => unreachable!("donor selection is frozen during evaluation"),
                    Change::Control(player) => seen.controller = *player,
                    Change::Types(types) => types.apply(seen),
                    Change::NotTypes(removed) => {
                        seen.types.retain(|t| !removed.contains(t));
                        let types = seen.types.clone();
                        seen.subtypes.retain(|s| of_types(s.kind()).iter().any(|t| types.contains(t)));
                    }
                    Change::Colors { colors, added: true } => add(&mut seen.colors, colors),
                    Change::Colors { colors, added: false } => seen.colors = colors.clone(),
                    Change::Gains(keywords) => {
                        // Every toxic instance contributes to the total (702.164b).
                        // Other held keywords retain their existing distinct-rule behavior.
                        for keyword in keywords {
                            if keyword.keyword.name() == "Toxic" || !seen.keywords.contains(keyword) {
                                seen.keywords.push(keyword.clone());
                            }
                        }
                    },
                    Change::LosesAll => {
                        seen.base_abilities = false;
                        seen.intrinsic_abilities = false;
                        seen.keywords.clear();
                        seen.given.clear();
                        seen.inherited.clear();
                    }
                    Change::Loses(keywords) => seen.keywords.retain(|k| !keywords.contains(k)),
                    Change::BasePt(p, t) => seen.pt = Some((*p, *t)),
                    Change::DefinedPt(p, t) => seen.pt = seen.pt.map(|(a,b)| (p.unwrap_or(a), t.unwrap_or(b))),
                    Change::Pt(p, t) => seen.pt = seen.pt.map(|(a, b)| (a + p, b + t)),
                }
            }
        }
    }
}

#[cfg(test)]
thread_local! {
    /// Tests: order every layer by the dependency trials, proving nothing beforehand.
    pub(crate) static TRIALS_ONLY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    pub(crate) static POTENTIAL_GRANTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(crate) static PAIRS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(crate) static VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// Tests: how many times the abilities layer was proven to need no trial.
    pub(crate) static PROVEN: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// Tests: hold every card of every zone in every world, proving none left out unchanged.
    pub(crate) static WHOLE_WORLD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Tests: how many objects the last world built held.
    pub(crate) static BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(crate) static HELD: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
/// The layers whose supported effects the engine applies to objects outside the battlefield
/// and the stack: types, colors and defined power and toughness (604.3), and explicit
/// off-zone ability grants (613.1f). Both [`Effect::evaluate`] and the world
/// `compute_seen` builds follow this one rule.
fn reaches_every_zone(layer: Layer) -> bool {
    matches!(layer, Layer::Types | Layer::Colors | Layer::Abilities | Layer::DefinePt)
}

/// What [`Game::seen`] refuses (613.7m): timestamps given at once ([`AtOnce`]) are ordered
/// in APNAP order, those of objects of the active player's first and then each other
/// player's in turn order, but each player chooses the order of their own, and the engine
/// asks no player for it. Each layer (or sublayer) applies the timestamps of a range in
/// APNAP order and, within one player's, in the engine's commit order. Exactly what is
/// refused: a layer in which effects with different timestamps from one range apply, and
/// in which some other order the players could give those timestamps, the rest of the layer
/// applied as usual with its dependencies (613.8), leaves any object of the world with other
/// characteristics, or an effect begun on other objects (613.6), than that order does. An
/// effect of a static ability takes the later of its object's timestamp and that of the
/// effect that gave it the ability (613.7a), the later in the order tried. Not refused:
/// effects that share one timestamp (613.7a-c), effects of different layers or sublayers,
/// effects of different players' objects (whose order APNAP order fixes), effects that change
/// nothing in common or whose changes commute, and orders that dependency fixes. A refusal
/// is deferred to the end of the action that asked and undoes it ([`Game::guarded`]); every
/// action ends by asking what each permanent, spell and object an effect names is
/// ([`Game::check_simultaneous_order`]), so a later arrival that the order decides is
/// refused too. Conservative where the rules would narrow the order: a timestamp no object
/// received, such as a counter's (613.7c), has no place 613.7m fixes, and with one every
/// order of its range's timestamps is tried; an object's own static ability is not put
/// before a resolving effect given its timestamp at once that applies to that object
/// (613.7n). More than 720 orders of one layer are not tried and are refused
/// ([`TOO_MANY_SIMULTANEOUS`]).
const SIMULTANEOUS_ORDER: &str = "the order of timestamps given at once, which the players choose in APNAP order (613.7m), changes a result here; the engine does not ask for it";
const TOO_MANY_SIMULTANEOUS: &str = "too many effects in one layer have timestamps given at once to establish that their APNAP order (613.7m) changes no result";

/// Timestamps given at once (613.7m), `first..=last`, each with where the player whose
/// object received it stood then in APNAP order from the active player (101.4): that
/// object's controller, or its owner if it has none (108.4a). `None` for a timestamp no
/// object received, such as a counter's (613.7c).
#[derive(Debug, Clone)]
pub(crate) struct AtOnce { first: u32, last: u32, seats: Vec<Option<u8>> }
impl AtOnce {
    fn seat(&self, stamp: u32) -> Option<u8> { self.seats[(stamp - self.first) as usize] }
}

/// Effects of one layer whose timestamps were given at once ([`AtOnce`] from `first`) and
/// whose order the players could change: the effects, and the timestamps they take in
/// groups in APNAP order, the order within each group being one player's choice (613.7m).
/// One group holds them all when a timestamp is no object's.
struct Run { first: u32, effects: Vec<usize>, groups: Vec<Vec<u32>> }
impl Run {
    /// The order the engine applies: APNAP order, and within each group the engine's own.
    fn applied(&self) -> Vec<u32> { self.groups.concat() }
}

/// Where an effect applies in its layer, when the timestamps of each run are in the order
/// `orders` gives them: an effect taking the later of its timestamps takes the later of
/// them there (613.7a). Any other effect stays where its timestamp puts it.
fn order_key(effect: &Effect, runs: &[Run], orders: &[Vec<u32>]) -> (u32, usize) {
    for (run, order) in runs.iter().zip(orders) {
        let place = |stamp: u32| order.iter().position(|s| *s == stamp);
        if let Some(at) = place(effect.stamp) {
            return (run.first, effect.also.iter().filter_map(|s| place(*s)).fold(at, usize::max));
        }
    }
    (effect.stamp, 0)
}

/// Every order of `0..n`.
fn permutations(n: usize) -> Vec<Vec<usize>> {
    let mut all = vec![Vec::new()];
    for k in 0..n {
        all = all.into_iter().flat_map(|order: Vec<usize>| (0..=order.len()).map(move |at| {
            let mut order = order.clone();
            order.insert(at, k);
            order
        })).collect();
    }
    all
}

/// Do two applications of one layer leave every object with the same characteristics, and
/// every effect that has begun applying to the same objects (613.6)? What an object has
/// is compared as a collection: the order its types or abilities were listed in is not a
/// characteristic.
fn same_layer_result(world: &Working, effects: &[Effect], other: &Working, others: &[Effect]) -> bool {
    fn same<T: PartialEq>(a: &[T], b: &[T]) -> bool {
        a.len() == b.len() && a.iter().all(|x| a.iter().filter(|y| *y == x).count() == b.iter().filter(|y| *y == x).count())
    }
    let seen = |a: &Seen, b: &Seen| a.controller == b.controller && a.pt == b.pt
        && a.base_abilities == b.base_abilities && a.intrinsic_abilities == b.intrinsic_abilities
        && same(&a.types, &b.types) && same(&a.subtypes, &b.subtypes) && same(&a.supertypes, &b.supertypes)
        && same(&a.colors, &b.colors) && same(&a.keywords, &b.keywords) && same(&a.given, &b.given)
        && same(&a.inherited, &b.inherited);
    world.objects.iter().zip(&other.objects).all(|((a, x), (b, y))| a == b && seen(x, y))
        && effects.iter().zip(others).all(|(a, b)| match (&a.started, &b.started) {
            (Some(x), Some(y)) => same(x, y),
            (x, y) => x.is_none() && y.is_none(),
        })
}

pub(crate) fn trials_only() -> bool {
    #[cfg(test)]
    return TRIALS_ONLY.with(std::cell::Cell::get);
    #[cfg(not(test))]
    false
}

fn whole_world() -> bool {
    #[cfg(test)]
    return WHOLE_WORLD.with(std::cell::Cell::get);
    #[cfg(not(test))]
    false
}

/// Can none of these effects of the abilities layer depend on another of them (613.8a), so
/// that they apply in timestamp order with no trial? Applying one there changes only what
/// abilities objects have. So one of them can change another only by what that other reads
/// of abilities, or by taking away the ability that states it:
/// - a resolved effect and a keyword counter read nothing;
/// - a static ability's part must read no ability to say what it applies to and when
///   ([`crate::program::registration::Characteristic::independent_of_abilities`]), and must
///   be of its source's own definition, which only losing all abilities ends;
/// - so nothing here may take all abilities from the source of a static ability here whose
///   existence is still asked: no static part that does at all, and no resolved one that
///   affects such a source.
/// Anything else is left to the trials.
fn independent_abilities(effects: &[Effect], remaining: &[usize], entering: bool) -> bool {
    #[cfg(test)]
    if TRIALS_ONLY.with(std::cell::Cell::get) { return false; }
    let asked: Vec<ObjId> = remaining.iter().filter_map(|i| match &effects[*i] {
        Effect { body: Body::Static { .. }, started: None, by, .. } => *by,
        _ => None,
    }).collect();
    let proven = remaining.iter().all(|i| match &effects[*i].body {
        Body::Static { statement, act, key, .. } => matches!(key.origin, Origin::Own | Origin::Copied(_) | Origin::Rule(_))
            && statement.independent_of_abilities(*act) && !statement.removes_abilities(*act),
        // what affects a spell affects the permanent it is about to become, under another
        // identity: that is not looked into
        Body::Resolved(c) => c.change != Change::LosesAll || (!entering && !c.affected.iter().any(|o| asked.contains(o))),
        Body::Counter { .. } | Body::Bestowed { .. } => true,
    });
    #[cfg(test)]
    if proven { PROVEN.with(|n| n.set(n.get() + 1)); }
    proven
}

/// What a permanent is, now: its card types, subtypes, supertypes and colors, its power and
/// toughness and the abilities it has beyond those of its own definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    pub controller: crate::PlayerId,
    pub types: Vec<CardType>,
    pub subtypes: Vec<Subtype>,
    pub supertypes: Vec<ir::Supertype>,
    pub colors: Vec<Color>,
    pub pt: Option<(i32, i32)>,
    pub keywords: Vec<Held>,
    /// Whether rules-text/copied abilities survive type setting and ability removal.
    /// Captured occurrences keep their definition independently (113.7a).
    pub base_abilities: bool,
    /// Intrinsic basic-land mana survives 305.7 type setting, but not ability removal.
    pub intrinsic_abilities: bool,
    /// The abilities effects give it (613.1f), in timestamp order, each with the giving it
    /// is. Kept with the rest of what an object was as it last existed, so what looks back
    /// at it finds them (603.10a).
    pub given: Vec<(Grant, Template)>,
    pub(crate) inherited: Vec<AbilityKey>,
}

impl Seen {
    pub(crate) fn has_origin(&self, origin: Origin) -> bool {
        match origin {
            Origin::Own | Origin::Copied(_) => self.base_abilities,
            Origin::Rule(_) => self.intrinsic_abilities,
            Origin::Inherited(family) => self.inherited.iter().any(|key| key.origin == Origin::Inherited(family.clone())),
            Origin::Given { grant, what } => self.given.contains(&(grant, what)),
        }
    }

    /// Megamorph is a variant of the morph ability (702.37b), and multikicker of kicker
    /// (702.33c): what has the variant has the ability.
    pub fn has(&self, keyword: Keyword) -> bool {
        let variant = match keyword.name() { "Morph" => Some("Megamorph"), "Kicker" => Some("Multikicker"), _ => None };
        self.keywords.iter().any(|i| i.keyword == keyword || variant == Some(i.keyword.name()))
    }

    /// Sum all functioning toxic instances, including equal printed and granted values.
    pub fn total_toxic_value(&self) -> u32 {
        self.keywords.iter().filter_map(|held| match &held.param {
            ir::token::KeywordParam::Number(ir::Amount::Const(n)) if held.keyword.name() == "Toxic" => Some(*n),
            _ => None,
        }).sum()
    }

    pub fn is(&self, t: CardType) -> bool {
        self.types.contains(&t)
    }

    /// A creature with no value for its power and toughness has 0 for each (208.5). Taken
    /// as the power/toughness layers begin, so that what sets or changes them, counters
    /// included, applies to that value (613.4).
    fn no_value_pt(&mut self) {
        if self.pt.is_none() && self.is(CardType::Creature) { self.pt = Some((0, 0)); }
    }
}

/// Is this act one that changes what its subject is while an effect lasts?
pub(crate) fn changes(a: &Action) -> bool {
    matches!(a, Action::Gets { .. } | Action::Gains(_) | Action::SetStat { of: None, .. })
        || matches!(a, Action::Static(s) if matches!(**s, StaticAct::Has(_) | StaticAct::Loses(_) | StaticAct::IsNot(Becoming::Is(_))))
        || matches!(a, Action::Is(Becoming::Is(_) | Becoming::Colors { .. } | Becoming::ChosenType(_)) | Action::Becomes(Becoming::Is(_) | Becoming::Colors { .. }))
}

/// The abilities granted, of those the engine gives their meaning: the keyword abilities,
/// and the abilities given in quotes, each the shared definition of its words.
pub(crate) fn granted(granted: &[Granted], support: &Support<'_>) -> Res<Vec<Change>> {
    granted_bound(granted, support, &mut crate::program::expr::linked(support))
}

/// [`granted`], with `bind` binding a value the granting text chose before it.
fn creature_donors(filter: &ir::Filter) -> bool {
    match filter {
        ir::Filter::Atom(ir::Atom::Type(ir::CardType::Creature)) => true,
        ir::Filter::All(parts) => parts.iter().any(creature_donors),
        ir::Filter::Any(parts) => !parts.is_empty() && parts.iter().all(creature_donors),
        _ => false,
    }
}

pub(crate) fn granted_bound(granted: &[Granted], support: &Support<'_>, bind: crate::program::expr::Bind<'_>) -> Res<Vec<Change>> {
    let (mut keywords, mut abilities) = (Vec::new(), Vec::new());
    let mut inherited = Vec::new();
    for g in granted {
        match g {
            // kept whole, parameter and all: "gains islandwalk" is landwalk of Islands
            // (702.14a), lowered here as a printed one is with its definition
            Granted::Keyword(k) if crate::keyword::changeling(k) => return unsupported("gains: changeling requires earlier-layer ability membership (613.1d)"),
            Granted::Keyword(k) => {
                let held = Held::compile_bound(k, support, &mut *bind)?;
                // a granted keyword that is a triggered ability, ward (702.21a), is given with
                // its trigger: each grant is its own instance, and each triggers separately
                for (_, gives) in held.contributions() {
                    if let crate::keyword::Gives::Ability(a) = gives { abilities.push(crate::ability::template(a)?); }
                }
                keywords.push(held);
            }
            Granted::Ability(a) => abilities.push(crate::ability::template(a)?),
            Granted::AbilitiesOf { kind: ir::action::AbilityKind::Activated, from }
                if matches!(from.quant, ir::Quant::All | ir::Quant::Each) && from.chooser.is_none() && from.random.is_none()
                    && ((from.set.domain == ir::Domain::Permanent && creature_donors(&from.set.filter) && from.set.zone.is_none())
                        || from.set.domain == ir::Domain::Card && from.set.zone.is_some())
                    && support.ability().is_some_and(|a| matches!(a, ir::Ability::Text(_))) && support.completed_source_creature() == Some(true) => {
                let set = crate::program::expr::Set::compile(&from.set, support, &mut *bind)?;
                if from.set.domain == ir::Domain::Card && !set.graveyard_donors() {
                    return unsupported("inherited card abilities: donor domain must be a fixed graveyard");
                }
                inherited.push(Change::Inherits(set));
            },
            other => return unsupported(format!("gains: {}", name(other))),
        }
    }
    let mut out = Vec::new();
    if !keywords.is_empty() { out.push(Change::Gains(keywords)); }
    if !abilities.is_empty() { out.push(Change::Gives(abilities)); }
    out.extend(inherited);
    Ok(out)
}

/// Removal outside the battlefield needs effective membership in those zones' readers
/// too. Do not admit it merely because the same words parse (113.6).
pub(crate) fn removal_scope(action: &Action, subject: &ir::Subject) -> Res<()> {
    fn permanent(subject: &ir::Subject) -> bool {
        let set = match subject {
            ir::Subject::Selector(s) | ir::Subject::Each(s) => &s.set,
            ir::Subject::Set(set) => set,
            ir::Subject::And { items, .. } => return items.iter().all(permanent),
            ir::Subject::LifeTotal(_) => return false,
        };
        set.possibilities().iter().all(|p| p.domain.sort.is_a(ir::Sort::Permanent))
    }
    if matches!(action, Action::Static(s) if matches!(**s, StaticAct::Loses(_))) && !permanent(subject) {
        return unsupported("loses: ability removal outside a permanent scope");
    }
    Ok(())
}

/// Removal is compiled once alongside membership_changed. Selective quoted abilities, "other"
/// and self-removal require finer membership identities and are explicitly unsupported.
pub(crate) fn lost(lost: &ir::statics::Lost, support: &Support<'_>) -> Res<Vec<Change>> {
    use ir::statics::{Lost, Lose};
    match lost {
        Lost::AllOf { other: false, what } if what.as_slice() == [Lose::Abilities] => Ok(vec![Change::LosesAll]),
        Lost::Abilities(abilities) => {
            let mut keywords = Vec::new();
            for a in abilities {
                let Granted::Keyword(k) = a else { return unsupported("loses: a specific non-keyword ability"); };
                if k.param != ir::token::KeywordParam::None {
                    return unsupported("loses: a parameterized keyword");
                }
                keywords.push(Held::compile(k, support)?);
            }
            Ok(vec![Change::Loses(keywords)])
        }
        _ => unsupported("loses: types, other abilities or this ability"),
    }
}

/// "base power and toughness 3/3": the one way of setting stats the engine runs, both of them to
/// numbers (613.4b).
pub(crate) fn profile(set: &Setting) -> Option<(i32, i32)> {
    match (set.stats.as_slice(), &set.to) {
        ([Stat::Power, Stat::Toughness], SetTo::Profile(Pt::Const(p), Pt::Const(t))) => Some((*p as i32, *t as i32)),
        _ => None,
    }
}

/// "becomes a 2/2 creature with all creature types": an effect in each layer it changes,
/// with one timestamp (613.7): its card types and subtypes (layer 4, 205.1), its colors
/// (layer 5), the abilities it gains (layer 6) and the power and toughness it has (layer 7b).
pub(crate) fn becoming(c: &Characteristics, support: &Support<'_>) -> Res<Vec<Change>> {
    // "in addition to its other types", "that's still a land", and "becomes an artifact
    // creature", which keeps the prior card types and subtypes too (205.1b)
    let keeps = match &c.keeps {
        Keeps::Nothing => c.types == [CardType::Artifact, CardType::Creature],
        Keeps::OtherTypes | Keeps::OtherCreatureTypes | Keeps::OtherLandTypes | Keeps::OtherColorsAndTypes | Keeps::StillA(_) => true,
        Keeps::OtherColors => false,
    };
    // the engine runs only what it ran before the grammar read supertypes, names, kept subtypes,
    // a setting listed with abilities, and a class of types listed with abilities
    if !c.supertypes.is_empty() || c.name.is_some() {
        return unsupported("becomes: supertypes or a name");
    }
    if matches!(&c.keeps, Keeps::StillA(k) if !k.subtypes.is_empty()) {
        return unsupported("becomes: still a subtype");
    }
    let listed = c.abilities.iter().any(|g| matches!(g, Granted::Every(_))) && c.abilities.len() > 1;
    if listed || (c.base_pt.is_some() && !c.abilities.is_empty()) {
        return unsupported("becomes: several things after 'with'");
    }
    // "with all creature types" (205.3m) is the one class of types the engine knows
    let mut every_creature_type = false;
    let mut abilities = Vec::new();
    for g in &c.abilities {
        match g {
            Granted::Every(ir::Every::CreatureTypes) => every_creature_type = true,
            Granted::Every(_) => return unsupported("becomes: a class of types other than creature types"),
            other => abilities.push(other.clone()),
        }
    }
    let mut out = Vec::new();
    if !c.types.is_empty() || !c.subtypes.is_empty() || every_creature_type {
        out.push(Change::Types(Types { types: c.types.clone(), subtypes: c.subtypes.clone(), every_creature_type, keeps }));
    }
    if let Some(colors) = &c.colors {
        out.push(Change::Colors { colors: colors.clone(), added: matches!(c.keeps, Keeps::OtherColors | Keeps::OtherColorsAndTypes) });
    }
    out.extend(granted(&abilities, support)?);
    match (&c.pt, &c.base_pt) {
        (Some((Pt::Const(p), Pt::Const(t))), None) => out.push(Change::BasePt(*p as i32, *t as i32)),
        (None, Some(set)) if profile(set).is_some() => {
            let (p, t) = profile(set).expect("checked");
            out.push(Change::BasePt(p, t))
        }
        (None, None) => {}
        _ => return unsupported("becomes: power and toughness that are not numbers"),
    }
    Ok(out)
}

/// "isn't a creature": the one type-removing effect the engine runs removes card types
/// only (613.1d); a removed supertype, subtype or color is not run.
pub(crate) fn not_types(c: &Characteristics) -> Res<Change> {
    let only_types = c.pt.is_none() && c.colors.is_none() && c.supertypes.is_empty() && c.subtypes.is_empty()
        && c.name.is_none() && c.abilities.is_empty() && c.base_pt.is_none() && matches!(c.keeps, Keeps::Nothing);
    if !only_types || c.types.is_empty() { return unsupported("isn't: anything but card types"); }
    Ok(Change::NotTypes(c.types.clone()))
}

/// "It's still a land.", said of the becoming before it: the prior card types, subtypes and
/// supertypes are kept (205.1b).
pub(crate) fn still(changes: Vec<Change>) -> Vec<Change> {
    changes
        .into_iter()
        .map(|c| match c {
            Change::Types(t) => Change::Types(Types { keeps: true, ..t }),
            other => other,
        })
        .collect()
}

/// The statement a static ability is, when it says what its subject is ("Creatures you
/// control get +1/+1.", "This creature has flying as long as you control an Island."). Read
/// as a definition is loaded, where it is lowered
/// ([`crate::program::registration::characteristics`]); no effect is asked through it.
pub fn statement(a: &Ability) -> Option<Statement<'_>> {
    crate::statement::read(a).filter(|s| s.acts.iter().all(|(action, _, _)| changes(action)))
}

/// CR 122.1b fixes this vocabulary; other counter names grant no keyword.
fn counter_keyword(kind: &str) -> Option<Keyword> {
    let keyword = Keyword::from_name(kind)?;
    match keyword.name() {
        "Flying" | "First Strike" | "Double Strike" | "Deathtouch" | "Decayed"
        | "Exalted" | "Haste" | "Hexproof" | "Indestructible" | "Lifelink"
        | "Menace" | "Reach" | "Shadow" | "Trample" | "Vigilance" => Some(keyword),
        _ => None,
    }
}

/// Does the engine give a counter of this kind what its rule says it does (122.1)? A +X/+Y
/// or -X/-Y counter changes power and toughness (122.1a) and a keyword counter gives its
/// keyword (122.1b), which is asked of the keyword. Finality counters create a departure
/// replacement (122.1h). Shield counters replace damage/destruction (122.1c, d) and
/// hone counters change another permanent (122.1j): those are not done yet, so text that
/// puts one is not run. A counter of any other name does nothing
/// by itself: other text says what it means.
pub fn counter_ok(kind: &str) -> Res<()> {
    match (kind, counter_keyword(kind)) {
        // the grammar reads the two-word keyword counters (first strike, double strike) since the
        // noun phrases name them; their effect on combat is not tested here yet, so they are refused
        (kind, Some(k)) if kind.contains(' ') => unsupported(format!("keyword counter: {}", k.name())),
        // a counter gives the keyword alone, not the trigger a triggered keyword is (exalted,
        // 702.83a): each counter would need its own instance of the ability
        (_, Some(k)) if crate::keyword::gives_ability(&ir::token::KeywordInstance { keyword: k, param: ir::token::KeywordParam::None }) =>
            unsupported(format!("keyword counter: {} gives a triggered ability", k.name())),
        (_, Some(k)) if crate::keyword::known(k) => Ok(()),
        (_, Some(k)) => unsupported(format!("keyword counter: {}", k.name())),
        ("shield" | "hone", None) => unsupported(format!("counter: {kind} (122.1)")),
        _ => Ok(()),
    }
}

/// What one counter of this kind adds to power and toughness, when it is a +X/+Y or -X/-Y
/// counter (122.1a): "+1/+1", "-0/-1", "+2/+0".
fn pt_counter(kind: &str) -> Option<(i32, i32)> {
    let (x, y) = kind.split_once('/')?;
    let signed = |s: &str| (s.starts_with('+') || s.starts_with('-')).then(|| s.parse::<i32>().ok()).flatten();
    Some((signed(x)?, signed(y)?))
}

/// How long a resolved effect lasts, by the duration its act says, or a later act of the same
/// clause says for it ("gets +2/+2 and gains trample until end of turn"). Some(true): until
/// the cleanup step (514.2). Some(false): no duration is stated, so until the end of the game
/// (611.2a).
pub(crate) fn lasts(mods: &[&[Modifier]]) -> Res<bool> {
    let end_of_turn = Time { end: true, step: Step::Turn, whose: StepOwner::This };
    let stated = mods.iter().find_map(|m| m.iter().find_map(|m| if let Modifier::Duration(d) = m { Some(d) } else { None }));
    match stated {
        None => Ok(false),
        Some(Duration::ThisTurn) => Ok(true),
        Some(Duration::Until(t)) if *t == end_of_turn => Ok(true),
        Some(other) => unsupported(format!("a duration: {}", name(other))),
    }
}

impl Game {
    /// A new timestamp (613.7): later than every one before it.
    pub(crate) fn tick(&mut self) -> u32 {
        self.clock += 1;
        self.clock
    }

    /// The timestamps given from `first` until now were given at once (613.7m): to objects
    /// entering or leaving zones together, permanents transforming or turning face up or
    /// down together, attachments made together, emblems gotten together, and counters
    /// put on by one instruction or event (613.7c), with anything else that event stamped.
    /// APNAP order and the players' choices, not this engine's commit order, decide their
    /// relative order, and nothing asks for the choices: [`Game::seen`] refuses wherever
    /// they change a result. Fewer than two timestamps leave no order to choose.
    pub(crate) fn stamped_at_once(&mut self, first: u32) {
        let last = self.clock;
        if first >= last { return; }
        let start = self.simultaneous.partition_point(|r| r.last < first);
        let end = self.simultaneous.partition_point(|r| r.first <= last);
        let (lo, hi) = match &self.simultaneous[start..end] {
            [] => (first, last),
            [one, ..] => (first.min(one.first), last.max(self.simultaneous[end - 1].last)),
        };
        // Whose object received each timestamp, in APNAP order from the active player
        // (613.7m, 101.4): its controller, or its owner if it has none (108.4a).
        let mut seats = vec![None; (hi - lo) as usize + 1];
        let seat = |player: crate::PlayerId| ((player + self.players.len() - self.active) % self.players.len()) as u8;
        let zones = self.battlefield.iter().chain(&self.stack).chain(&self.command).chain(&self.exile)
            .chain(self.players.iter().flat_map(|p| p.library.iter().chain(&p.hand).chain(&p.graveyard)));
        for id in zones {
            let o = self.obj(*id);
            if (first..=last).contains(&o.stamp) {
                let player = if matches!(o.zone, Zone::Battlefield | Zone::Stack) { self.controller(*id) } else { o.owner };
                seats[(o.stamp - lo) as usize] = Some(seat(player));
            }
        }
        // what an earlier part of this event gave a timestamp to keeps its place
        for range in &self.simultaneous[start..end] {
            for (at, seat) in range.seats.iter().enumerate() {
                let slot = &mut seats[(range.first - lo) as usize + at];
                *slot = seat.or(*slot);
            }
        }
        self.simultaneous.splice(start..end, [AtOnce { first: lo, last: hi, seats }]);
    }

    /// Ask what every permanent, spell and emblem is, and every object a resolved effect
    /// names, as an action stops: a characteristic that the players' order of simultaneous
    /// timestamps decides (613.7m) refuses the action that left it, even if nothing in the
    /// action itself asked. Static abilities function from permanents and emblems, and one
    /// answer applies every layer to every object its world holds, which holds each object
    /// a static ability can change; one refusal is enough.
    pub(crate) fn check_simultaneous_order(&self) {
        // no timestamps given at once: no order of them to decide anything
        if self.simultaneous.is_empty() { return; }
        let named = self.continuous.iter().flat_map(|c| &c.affected);
        for id in self.battlefield.iter().chain(&self.stack).chain(&self.command).chain(named) {
            let object = self.obj(*id);
            if object.ability.is_some() || object.zone == Zone::Gone { continue; }
            self.seen(*id, None);
            if self.refusing() { return; }
        }
    }

    /// Are these timestamps given at once ([`Game::stamped_at_once`])?
    pub(crate) fn at_once(&self, a: u32, b: u32) -> bool {
        a != b && self.given_at_once(a).is_some_and(|range| (range.first..=range.last).contains(&b))
    }

    /// The timestamps given at once with this one, if it was ([`Game::stamped_at_once`]).
    fn given_at_once(&self, stamp: u32) -> Option<&AtOnce> {
        let at = self.simultaneous.partition_point(|r| r.last < stamp);
        self.simultaneous.get(at).filter(|range| range.first <= stamp)
    }

    /// The later of these timestamps, which an effect of a static ability takes (613.7a), and
    /// the others given at once with it: which of those is the later is the players' order of
    /// them (613.7m), not this engine's numbers.
    fn later_stamp(&self, stamps: &[u32]) -> (u32, Vec<u32>) {
        let later = stamps.iter().copied().max().expect("a timestamp");
        let mut also: Vec<u32> = stamps.iter().copied().filter(|s| self.at_once(*s, later)).collect();
        also.sort_unstable();
        also.dedup();
        (later, also)
    }

    /// This layer's effects whose timestamps were given at once (613.7m), `remaining` being
    /// in timestamp order, in runs, one for each range of timestamps with two effects and two
    /// timestamps. Effects that share one timestamp are not ordered by 613.7m and keep the
    /// engine's order: one object's static abilities and those an effect gave it with that
    /// effect's timestamp (613.7a), the parts of one resolved effect (613.7b), and counters
    /// of one kind (613.7c).
    fn simultaneous_runs(&self, effects: &[Effect], remaining: &[usize]) -> Vec<Run> {
        let mut runs = Vec::new();
        if self.simultaneous.is_empty() { return runs; }
        let mut at = 0;
        while at < remaining.len() {
            let start = at;
            at += 1;
            let Some(range) = self.given_at_once(effects[remaining[start]].stamp) else { continue };
            while at < remaining.len() && effects[remaining[at]].stamp <= range.last { at += 1; }
            let members = &remaining[start..at];
            let mut stamps: Vec<u32> = members.iter().flat_map(|i| std::iter::once(effects[*i].stamp).chain(effects[*i].also.iter().copied())).collect();
            stamps.sort_unstable();
            stamps.dedup();
            if members.len() < 2 || stamps.len() < 2 { continue; }
            // Each player's objects in APNAP order, unless one timestamp is no object's.
            let groups = if stamps.iter().all(|s| range.seat(*s).is_some()) {
                let mut groups: Vec<(u8, Vec<u32>)> = Vec::new();
                for stamp in stamps {
                    let seat = range.seat(stamp).expect("every seat known");
                    match groups.iter_mut().find(|(s, _)| *s == seat) {
                        Some((_, group)) => group.push(stamp),
                        None => groups.push((seat, vec![stamp])),
                    }
                }
                groups.sort_by_key(|(seat, _)| *seat);
                groups.into_iter().map(|(_, group)| group).collect()
            } else { vec![stamps] };
            runs.push(Run { first: range.first, effects: members.to_vec(), groups });
        }
        runs
    }

    /// Does no order the players could give these runs change what this layer does, so that
    /// none needs trying? So when no effect of the layer may depend on another (613.8a):
    /// each then does the same wherever it is applied, in the order of its timestamp. Then it
    /// is enough that on each object, the changes of effects whose relative order the players
    /// could change all add to what it is, all take from it, or are all the same: those
    /// leave it the same in any order. What an object has is compared as a collection, as
    /// [`same_layer_result`] does. Effects of one timestamp, and those of different players'
    /// objects, keep their order; one taking the later of two timestamps has no fixed place.
    fn commuting(&self, layer: Layer, effects: &[Effect], footprints: &[crate::program::footprint::EffectFootprint],
        remaining: &[usize], runs: &[Run], world: &Working) -> bool {
        if trials_only() || remaining.iter().any(|a| remaining.iter().any(|b| a != b && footprints[*a].may_depend_on(footprints[*b]))) {
            return false;
        }
        /// `Some(true)` adds, `Some(false)` takes away; a change that sets is neither.
        fn adds(change: &Change) -> Option<bool> {
            match change {
                Change::Gains(_) | Change::Gives(_) | Change::Inherited(_) | Change::Pt(..) | Change::Colors { added: true, .. } => Some(true),
                // "with all creature types" asks whether it is a creature as it applies (205.3m)
                Change::Types(types) if types.keeps && !types.every_creature_type => Some(true),
                Change::NotTypes(_) | Change::Loses(_) | Change::LosesAll => Some(false),
                _ => None,
            }
        }
        runs.iter().all(|run| {
            let group = |stamp: u32| run.groups.iter().position(|group| group.contains(&stamp));
            let fixed = |a: &Effect, b: &Effect| a.also.is_empty() && b.also.is_empty()
                && (a.stamp == b.stamp || group(a.stamp) != group(b.stamp));
            let mut changed: Vec<(ObjId, usize, Change)> = Vec::new();
            for i in &run.effects {
                for (id, changes) in effects[*i].evaluate(self, world, layer) {
                    changed.extend(changes.into_iter().map(|change| (id, *i, change)));
                }
            }
            changed.iter().all(|(id, a, change)| {
                changed.iter().filter(|(on, b, _)| on == id && a != b && !fixed(&effects[*a], &effects[*b]))
                    .all(|(_, _, other)| other == change || adds(other).is_some_and(|x| adds(change) == Some(x)))
            })
        })
    }

    /// Each order the players could give the timestamps of these runs (613.7m), as the order
    /// of each run's: every combination of one order of each group, the groups in APNAP
    /// order, but the one the engine applies ([`Run::applied`]). `None` when there are too
    /// many to try.
    fn simultaneous_orders(runs: &[Run]) -> Option<Vec<Vec<Vec<u32>>>> {
        /// Seven timestamps have 5040 orders: past this, refuse rather than try them all.
        const MOST: usize = 720;
        let mut count = 1usize;
        for group in runs.iter().flat_map(|run| &run.groups) {
            count = (1..=group.len()).try_fold(count, |n, k| n.checked_mul(k).filter(|n| *n <= MOST))?;
        }
        let mut orders: Vec<Vec<Vec<u32>>> = vec![Vec::new()];
        for run in runs {
            let mut sequences: Vec<Vec<u32>> = vec![Vec::new()];
            for group in &run.groups {
                let mut next = Vec::new();
                for done in &sequences {
                    for permutation in permutations(group.len()) {
                        next.push(done.iter().copied().chain(permutation.iter().map(|at| group[*at])).collect());
                    }
                }
                sequences = next;
            }
            let mut next = Vec::new();
            for order in &orders {
                for sequence in &sequences {
                    let mut order = order.clone();
                    order.push(sequence.clone());
                    next.push(order);
                }
            }
            orders = next;
        }
        let applied: Vec<_> = runs.iter().map(Run::applied).collect();
        orders.retain(|order| *order != applied);
        Some(orders)
    }

    /// A resolving spell or ability begins a continuous effect (611.2): what it changes in
    /// each layer, with one timestamp (613.7b).
    #[cfg(test)]
    pub(crate) fn begin_effect_for(&mut self, changes: Vec<Change>, affected: Vec<ObjId>, lifetime: Lifetime) {
        self.begin_effect_from(changes, affected, lifetime, None);
    }

    pub(crate) fn begin_effect_from(&mut self, changes: Vec<Change>, affected: Vec<ObjId>, lifetime: Lifetime,
        producer: Option<std::sync::Arc<ResolvedProducer>>) {
        // 611.2b: a duration already ended before resolution never begins.
        if !lifetime.holds(self) { return; }
        let previous = self.control_snapshot();
        let stamp = self.tick();
        for (part, change) in changes.into_iter().enumerate() {
            self.continuous.push(Continuous { producer: producer.clone(), change, affected: affected.clone(), stamp, part: part as u32, lifetime });
        }
        self.prune_control_durations();
        self.control_transitions(previous);
    }

    /// Ending one duration may change control and end another. Recompute until stable;
    /// no removed effect can restart if its former condition becomes true again.
    pub(crate) fn prune_control_durations(&mut self) {
        loop {
            let ended: Vec<_> = self.continuous.iter().enumerate().filter_map(|(i,c)| (!c.lifetime.holds(self)).then_some(i)).collect();
            if ended.is_empty() { break; }
            for i in ended.into_iter().rev() { self.continuous.remove(i); }
        }
    }
    pub(crate) fn end_control_durations(&mut self) {
        if !self.continuous.iter().any(|c| matches!(c.lifetime, Lifetime::Controlled { .. } | Lifetime::SourcePresent { .. })) { return; }
        let previous = self.control_snapshot();
        self.prune_control_durations();
        self.control_transitions(previous);
    }

    /// Current control is a layer-two query; a departed incarnation uses its LKI.
    pub fn controller(&self, object: ObjId) -> crate::PlayerId {
        let o = self.obj(object);
        if o.zone == Zone::Gone { return o.last.as_ref().map_or(o.controller, |last| last.controller); }
        // No admitted static control effects yet. Avoid building a layer world when
        // none of the resolved effects can change control.
        if !self.continuous.iter().any(|c| matches!(c.change, Change::Control(_))) { return o.controller; }
        self.seen(object, Some(Layer::Types)).controller
    }

    pub(crate) fn control_snapshot(&self) -> Vec<(ObjId, crate::PlayerId)> {
        self.battlefield.iter().map(|o| (*o, self.controller(*o))).collect()
    }

    /// 302.6 and 506.4: the consequences of a transition, not a cached query result.
    pub(crate) fn control_transitions(&mut self, previous: Vec<(ObjId, crate::PlayerId)>) {
        for (object, player) in previous {
            if self.obj(object).zone == Zone::Battlefield && self.controller(object) != player {
                self.grant_hideaway_look(object, self.controller(object));
                self.obj_mut(object).since = self.turn;
                self.record_control_acquisition(object, self.controller(object));
                self.combat.remove(object);
                // Its new controller may look at a face-down permanent (708.5).
                if self.face_down(object) {
                    let now = self.controller(object);
                    self.disclose(crate::observation::Kind::Looked, now, &[object]);
                }
            }
        }
    }

    /// Could an effect change definition membership, by granting or removing abilities?
    /// Query consumers ask once before scanning sources; additive keywords alone do not
    /// require rebuilding the definition groups.
    pub(crate) fn membership_changed(&self) -> bool {
        let own = self.prospective.map(|(on, _)| on);
        self.continuous.iter().any(|c| c.change.changes_membership())
            || self.battlefield.iter().chain(own.as_ref()).any(|s| self.def(*s).is_some_and(|d| d.changes_membership()))
    }

    /// Printed/token characteristics, or the last known state of a departed incarnation.
    pub(crate) fn base_seen(&self, o: ObjId) -> Seen {
        let obj = self.obj(o);
        if let (Zone::Gone, Some(last)) = (obj.zone, &obj.last) { return last.clone(); }
        self.printed_seen(o)
    }

    /// The copiable values (707.2) before any continuous effect: the face or copy values
    /// up, or a token's creating definition. [`Game::base_seen`] of an object in a zone.
    pub(crate) fn printed_seen(&self, o: ObjId) -> Seen {
        let obj = self.obj(o);
        let printed = self.def(o).map(|d| d.keywords.clone())
            .unwrap_or_else(|| obj.chars.keywords.iter().map(|k| Held::plain(*k)).collect());
        let c = &obj.chars;
        Seen { controller: obj.controller, types: c.types.clone(), subtypes: c.subtypes.clone(), supertypes: c.supertypes.clone(),
            colors: c.colors.clone(), pt: c.pt, keywords: printed, base_abilities: true, intrinsic_abilities: true, given: Vec::new(), inherited: Vec::new() }
    }

    /// Ask one consistent partial world. Effects read the changes already applied in their
    /// layer, including changes to other objects used by conditions and quantities (613.8).
    pub fn seen(&self, o: ObjId, before: Option<Layer>) -> Seen {
        self.query_seen(o, before, || self.compute_seen(o, before))
    }

    fn compute_seen(&self, o: ObjId, before: Option<Layer>) -> Seen {
        #[cfg(feature = "profile")]
        let _profile = crate::profile::span(crate::profile::Scope::Seen);
        let obj = self.obj(o);
        if obj.zone == Zone::Gone { return self.base_seen(o); }
        let decided = self.order_dependence();
        self.subscribe_cut(before, crate::query::characteristic_reads());
        if self.no_layer_effects(before) { return self.unchanged_seen(o, before); }
        let own = self.prospective.map(|(on, _)| on).filter(|on| *on == o);
        #[cfg(feature = "profile")]
        let discovery = crate::profile::span(crate::profile::Scope::Discover);
        let mut effects = Vec::new();
        // Defining abilities function in every zone (113.6a). Other static effects still
        // require a battlefield source, including the prospective entrant.
        // Subtype, color and power/toughness definitions are taken below. Views stopping
        // before the type layer need not discover these off-battlefield sources.
        let defining = before.is_none_or(|b| b > Layer::Types);
        let elsewhere = defining.then(|| self.defining_sources(|| {
            self.stack.iter().chain(&self.exile).chain(&self.command)
                .chain(self.players.iter().flat_map(|p| p.library.iter().chain(&p.hand).chain(&p.graveyard)))
                .copied().filter(|id| {
                    let o = self.obj(*id);
                    !matches!(o.zone, Zone::Battlefield | Zone::Gone) && o.ability.is_none()
                        && self.origins(*id, crate::ability::View::Own).into_iter().any(|(_, def)|
                            def.characteristics.iter().any(|r| r.result.as_ref().is_ok_and(|s|
                                s.has_definition())))
                }).collect()
        })).into_iter().flatten();
        for source in self.battlefield.iter().copied().chain(own).chain(self.command.iter().copied().filter(|id| self.obj(*id).emblem.is_some())).chain(elsewhere) {
            #[cfg(feature = "profile")]
            crate::profile::count(crate::profile::Count::SourceVisits, 1);
            for (origin, def) in self.origins(source, crate::ability::View::Own) {
                for registered in &def.characteristics {
                    if let Ok(statement) = &registered.result {
                        if self.obj(source).zone != Zone::Battlefield && self.obj(source).emblem.is_none() && own != Some(source) && !statement.has_definition() { continue; }
                        for act in statement.parts() {
                            let (stamp, also) = match origin {
                                Origin::Copied(stamp) => self.later_stamp(&[stamp, self.obj(source).stamp]),
                                _ => (self.obj(source).stamp, Vec::new()),
                            };
                            effects.push(Effect { stamp, also, by: Some(source),
                                part: registered.ability as u32, started: None,
                                body: Body::Static { statement, act, key: AbilityKey { origin: origin.clone(), index: registered.ability },
                                    only: own.filter(|on| *on == source) } });
                        }
                    }
                }
            }
        }
        for c in &self.continuous {
            effects.push(Effect { stamp: c.stamp, also: Vec::new(), by: None, part: c.part, body: Body::Resolved(c), started: None });
        }
        // A bestowed Aura spell or Aura is one by an effect of its own bestow ability, which
        // has the object's timestamp (702.103b, 613.7a); so is a card asked about as it is
        // cast bestowed (702.103d).
        let mut bestowed: Vec<ObjId> = self.battlefield.iter().chain(&self.stack).copied().chain(own).chain([o])
            .filter(|id| self.obj(*id).bestowed).collect();
        bestowed.sort_unstable();
        bestowed.dedup();
        for on in bestowed {
            effects.push(Effect { stamp: self.obj(on).stamp, also: Vec::new(), by: None, part: 0, started: None, body: Body::Bestowed { on } });
        }
        // An effect none of whose parts is of a layer this view applies is never evaluated
        // below. It is left out before deciding whether a world is needed. One with a part
        // in an applied layer stays whole, its later parts simply not reached (613.6).
        if let Some(before) = before {
            effects.retain(|effect| LAYERS.iter().take_while(|l| **l < before).any(|l| effect.in_layer(*l)));
        }
        #[cfg(feature = "profile")]
        drop(discovery);
        // Most queries have no continuous effects. Do not build a world in that case.
        if effects.is_empty() {
            self.remember_no_effects(before);
            return self.unchanged_seen(o, before);
        }
        // Fixed resolved changes read only their captured recipients and operands. If
        // none can introduce a characteristic static, unrelated objects cannot affect
        // this query. Keep the full world for every static/granted characteristic rule.
        let needs_world = effects.iter().any(|effect| match &effect.body {
            Body::Static { .. } => true,
            Body::Resolved(Continuous { change: Change::Gives(templates), .. }) =>
                templates.iter().any(|what| !what.def().characteristics.is_empty()),
            Body::Resolved(_) | Body::Counter { .. } | Body::Bestowed { .. } => false,
        });
        #[cfg(feature = "profile")]
        let building = crate::profile::span(crate::profile::Scope::World);
        // Who an effect can apply to. Only a permanent or a spell is given anything outside
        // the layers that reach every zone ([`reaches_every_zone`], which `evaluate` asks
        // too), and there what is said of its own source reaches that source alone. So
        // unless something of those layers is said of what it describes, the cards of the
        // other zones an effect can change are the sources found there and those a resolved
        // effect names; every other one is as it is with no effect, which is how a view
        // answers for what it does not hold ([`Ctx::seen_for`]).
        use crate::program::registration::Subject;
        let everywhere = needs_world && (whole_world() || effects.iter().any(|effect| match &effect.body {
            Body::Static { statement, .. } => !matches!(statement.subject, Subject::Source | Subject::Controller)
                && LAYERS.iter().any(|layer| reaches_every_zone(*layer) && statement.changes_in(*layer)
                    && (*layer != Layer::Abilities || matches!(&statement.subject, Subject::Described(set) if set.off_battlefield_cards()))),
            Body::Resolved(_) | Body::Counter { .. } | Body::Bestowed { .. } => false,
        }));
        let mut ids: Vec<_> = if everywhere {
            self.battlefield.iter().chain(&self.stack).chain(&self.exile).chain(&self.command).copied()
                .chain(self.players.iter().flat_map(|p| p.library.iter().chain(&p.hand).chain(&p.graveyard)).copied()).collect()
        } else if needs_world {
            let mut ids: Vec<_> = self.battlefield.iter().chain(&self.stack).copied().collect();
            let elsewhere = |id: ObjId| matches!(self.obj(id).zone, Zone::Exile | Zone::Library | Zone::Hand | Zone::Graveyard | Zone::Command);
            let mut add = |id: ObjId| if elsewhere(id) && !ids.contains(&id) { ids.push(id); };
            for effect in &effects {
                match &effect.body {
                    Body::Static { statement, .. } => {
                        add(effect.by.expect("static source"));
                        // Donors are read in this partial world even when the recipient
                        // is the source alone. This does not register their abilities.
                        if statement.reads_graveyard_donors() {
                            self.players.iter().flat_map(|p| p.graveyard.iter()).copied().for_each(&mut add);
                        }
                    },
                    Body::Resolved(c) => c.affected.iter().copied().for_each(&mut add),
                    Body::Counter { .. } | Body::Bestowed { .. } => {}
                }
            }
            if let Some((entrant, was)) = self.prospective { [Some(entrant), was].into_iter().flatten().for_each(&mut add); }
            // A card of those zones that is none of these is as it is. Anything else asked
            // about that the world does not hold joins it below, as it always has: the
            // entrant, which is in no zone yet and has its own effects, and what is asked
            // from the command zone or from outside the zone it says it is in.
            if !ids.contains(&o) && own.is_none() && elsewhere(o) { return self.unchanged_seen(o, before); }
            ids
        } else { vec![o] };
        let listed = ids.contains(&o);
        if !listed { ids.push(o); }
        #[cfg(feature = "profile")]
        crate::profile::count(crate::profile::Count::Worlds, 1);
        #[cfg(feature = "profile")]
        crate::profile::count(crate::profile::Count::WorldObjects, ids.len());
        #[cfg(test)]
        { HELD.with(|held| held.set(ids.len())); BUILDS.with(|n|n.set(n.get()+1)); }
        let mut world = Working::new(ids.into_iter().map(|id| (id, self.base_seen(id))).collect());
        let mut zones: [Vec<usize>; 8] = std::array::from_fn(|_|Vec::new());
        for (at,(id,_)) in world.objects.iter().enumerate() { zones[self.obj(*id).zone as usize].push(at); }
        world.zones = Some(zones);
        world.live = Some(world.objects.iter().enumerate().filter_map(|(at,(id,_))| {
            let obj = self.obj(*id);
            (obj.zone == Zone::Battlefield || (obj.zone == Zone::Stack && obj.ability.is_none())).then_some(at)
        }).collect());
        #[cfg(feature = "profile")]
        drop(building);
        let mut candidates = LayerCandidates::new();
        candidates.extend(&effects);
        let mut order_dependent = None;
        for layer in LAYERS.into_iter().take_while(|l| before.is_none_or(|b| *l < b)) {
            // Discover potential granted characteristic effects before layer 6. Their
            // actual Origin membership gates evaluation, so dependency trials can see
            // an effect begin/end when its producing grant is added/removed (613.8a).
            // Compiled grants cannot introduce further definition grants here.
            if layer == Layer::Abilities {
                let mut granted_effects = Vec::new();
                for parent in &effects {
                    let grants: Vec<_> = match &parent.body {
                        Body::Static { statement, act, .. } => statement.potential_grants(*act),
                        Body::Resolved(Continuous { change: Change::Gives(templates), .. }) =>
                            templates.iter().enumerate().map(|(i, what)| (i as u32, *what)).collect(),
                        Body::Resolved(_) | Body::Counter { .. } | Body::Bestowed { .. } => Vec::new(),
                    };
                    for (index, what) in grants {
                        let grant = Grant { by: parent.by, stamp: parent.stamp, part: parent.part << 16 | index };
                        // Potential membership is bounded only by the parent's immutable
                        // recipient envelope, never by whether its conditions/Origin hold.
                        // Broad selectors retain every possible future match in this layer.
                        for slot in parent.candidates(self, &world, Layer::Abilities).iter() {
                            let (source, _) = &world.objects[*slot];
                            if !self.battlefield.contains(source) && own != Some(*source) { continue; }
                            for registered in &what.def().characteristics {
                                if let Ok(statement) = &registered.result {
                                    for act in statement.parts() {
                                        // the later of its object's and the giving effect's (613.7a)
                                        let stamps: Vec<u32> = [self.obj(*source).stamp, grant.stamp].into_iter().chain(parent.also.iter().copied()).collect();
                                        let (stamp, also) = self.later_stamp(&stamps);
                                        granted_effects.push(Effect { stamp, also, by: Some(*source),
                                            part: registered.ability as u32, started: None,
                                            body: Body::Static { statement, act, key: AbilityKey { origin: Origin::Given { grant, what }, index: registered.ability },
                                                only: own.filter(|on| *on == *source) } });
                                    }
                                }
                            }
                        }
                    }
                }
                #[cfg(test)]
                POTENTIAL_GRANTS.with(|n|n.set(n.get()+granted_effects.len()));
                effects.extend(granted_effects);
            }
            // All counters of one kind share the newest counter's timestamp (613.7c).
            // Participate in dependency ordering, so removal and later grants compose.
            if layer == Layer::Abilities {
                for (id, _) in &world.objects {
                    let obj = self.obj(*id);
                    if obj.zone != Zone::Battlefield { continue; }
                    for (kind, _) in obj.counters.iter().filter(|(_, n)| **n > 0) {
                        if let Some(keyword) = counter_keyword(kind) {
                            effects.push(Effect { stamp: obj.counter_stamps.get(kind).copied().unwrap_or(obj.stamp), also: Vec::new(),
                                by: None, part: 0, started: None, body: Body::Counter { on: *id, keyword } });
                        }
                    }
                }
            }
            if layer == Layer::DefinePt {
                for (_, seen) in &mut world.objects { seen.no_value_pt(); }
            }
            candidates.extend(&effects);
            for effect in &effects { if effect.in_layer(layer) { self.subscribe_cut(before,effect.footprint(layer).reads); } }
            let mut remaining = candidates.layers[layer as usize].clone();
            // Immutable possible-read/write metadata is valid even when predicates fail.
            // Actual proposals/edges are still recomputed from the partial world each step.
            let footprints: Vec<_> = effects.iter().map(|e|e.footprint(layer)).collect();
            remaining.sort_by_key(|i| effects[*i].stamp);
            // Timestamps given at once are in APNAP order, each player's own in the order
            // that player chooses, and no player is asked (613.7m). Apply the layer in APNAP
            // order with each player's in the engine's commit order; unless the effects whose
            // order the players could change commute, apply it in every other order they
            // could give too, and refuse if any leaves a different result.
            let runs = self.simultaneous_runs(&effects, &remaining);
            let in_order = |remaining: &[usize], orders: &[Vec<u32>]| {
                let mut ordered = remaining.to_vec();
                ordered.sort_by_key(|i| order_key(&effects[*i], &runs, orders));
                ordered
            };
            let orders = if runs.is_empty() || order_dependent.is_some() || self.commuting(layer, &effects, &footprints, &remaining, &runs, &world) { Some(Vec::new()) }
                else { Self::simultaneous_orders(&runs) };
            let trials: Vec<_> = orders.iter().flatten().map(|order| in_order(&remaining, order)).collect();
            if orders.is_none() { order_dependent = Some(TOO_MANY_SIMULTANEOUS); }
            if !runs.is_empty() { remaining = in_order(&remaining, &runs.iter().map(Run::applied).collect::<Vec<_>>()); }
            let initial = (!trials.is_empty()).then(|| (world.clone(), effects.iter().map(|e| e.started.clone()).collect::<Vec<_>>()));
            self.apply_layer(layer, &mut effects, &footprints, remaining, &mut world);
            if let Some((initial, started)) = initial {
                for order in trials {
                    let mut other = initial.clone();
                    let mut others: Vec<_> = effects.iter().zip(&started).map(|(e, s)| Effect { started: s.clone(), ..e.clone() }).collect();
                    self.apply_layer(layer, &mut others, &footprints, order, &mut other);
                    if !same_layer_result(&world, &effects, &other, &others) {
                        order_dependent = Some(SIMULTANEOUS_ORDER);
                        break;
                    }
                }
            }
            if layer == Layer::ChangePt {
                for (id, seen) in &mut world.objects { self.layer_counters(*id, seen, layer); }
            }
        }
        // An answer the players' order of simultaneous timestamps decides refuses the
        // action asking it, and is never kept for another question.
        if let Some(reason) = order_dependent {
            self.order_dependent(crate::Unsupported(reason.into()));
        }
        // This evaluation has already computed every object in its ordinary world.
        // Preserve those answers within the immutable read scope, rather than rebuild
        // the same world for each subsequent object. Prospective-entry worlds are local
        // to the queried object and never publish answers for other objects.
        if needs_world && listed && self.order_free_since(decided) && self.sharing_queries() && self.prospective.is_none() {
            for (id, seen) in &mut world.objects { self.finish_seen(*id, seen); }
            let seen = world.get(o).expect("query object included").clone();
            self.remember_world(before, world.objects);
            seen
        } else {
            let mut seen = world.objects.into_iter().find(|(id, _)| *id == o).expect("query object included").1;
            self.finish_seen(o, &mut seen);
            seen
        }
    }

    /// Apply one layer's effects to the world, `remaining` in their timestamp order:
    /// constant subtype and color definitions first (613.3), then each effect when every
    /// effect it depends on has applied, the earliest timestamp first (613.7, 613.8).
    fn apply_layer(&self, layer: Layer, effects: &mut [Effect], footprints: &[crate::program::footprint::EffectFootprint],
        mut remaining: Vec<usize>, world: &mut Working) {
        // 613.3: constant subtype/color definitions precede other effects in
        // their layer, irrespective of timestamps.
        if matches!(layer, Layer::Types | Layer::Colors) {
            let definitions: Vec<_> = remaining.iter().copied().filter(|i| matches!(
                &effects[*i].body, Body::Static { statement, act, .. } if statement.definition_in(*act, layer)
            )).collect();
            remaining.retain(|i| !definitions.contains(i));
            for i in definitions {
                let evaluated = effects[i].evaluate(self, world, layer);
                effects[i].commit(world, &evaluated);
            }
        }
        // Resolved effects and counters have fixed recipients and changes. Their
        // evaluation reads no partial-world characteristics, so applying another
        // effect cannot change their existence, recipients or result (613.8a).
        // The same holds for unconditional self definitions whose compiled inputs
        // cannot read P/T: layer 7a changes no existence, controller, type or ability.
        // Earlier layers still determine those inputs and source membership normally.
        let fixed = !trials_only() && remaining.iter().all(|i| match &effects[*i].body {
            Body::Static { statement, act, .. } => layer == Layer::DefinePt && statement.independent_pt_definition(*act),
            Body::Resolved(_) | Body::Counter { .. } | Body::Bestowed { .. } => true,
        });
        if fixed || (layer == Layer::Abilities && independent_abilities(effects, &remaining, self.prospective.is_some())) {
            for i in remaining.drain(..) {
                let evaluated = effects[i].evaluate(self, world, layer);
                effects[i].commit(world, &evaluated);
            }
        }
        while !remaining.is_empty() {
            if remaining.len() == 1 {
                let effect = remaining.pop().expect("one effect");
                let evaluated = effects[effect].evaluate(self, world, layer);
                effects[effect].commit(world, &evaluated);
                break;
            }
            #[cfg(feature = "profile")]
            let _dependencies = crate::profile::span(crate::profile::Scope::Dependencies);
            #[cfg(feature = "profile")]
            {
                use crate::profile::{count, Count};
                count(Count::Graphs, 1);
                match layer {
                    Layer::Abilities => count(Count::AbilityGraphs, 1),
                    Layer::DefinePt => count(Count::DefineGraphs, 1),
                    Layer::SetPt => count(Count::SetGraphs, 1),
                    Layer::ChangePt => count(Count::ChangeGraphs, 1),
                    _ => {}
                }
            }
            let evaluated: Vec<_> = remaining.iter().map(|i| effects[*i].evaluate(self, world, layer)).collect();
            #[cfg(feature = "profile")]
            crate::profile::count(crate::profile::Count::BaselineEvaluations,evaluated.len());
            let n = remaining.len();
            let mut depends = vec![vec![false; n]; n];
            for b in 0..n {
                if evaluated[b].is_empty() { continue; }
                let readers: Vec<_> = (0..n).filter(|&a| a != b && (trials_only()
                    || footprints[remaining[a]].may_depend_on(footprints[remaining[b]]))).collect();
                if readers.is_empty() { continue; }
                // A proposal changes only these recipients. Preserve those values,
                // evaluate against the changed world, then restore it before the next
                // proposal. No Game mutation or cross-query cache is involved.
                let saved: Vec<_> = evaluated[b].iter().map(|(id, _)| {
                    let at = world.slot(*id).expect("evaluated object");
                    (at, world.objects[at].1.clone())
                }).collect();
                #[cfg(feature = "profile")]
                {
                    crate::profile::count(crate::profile::Count::DependencyPairs, readers.len());
                    crate::profile::count(crate::profile::Count::TrialEvaluations, readers.len());
                    crate::profile::count(crate::profile::Count::DependencySnapshots, saved.len());
                }
                effects[remaining[b]].apply(world, &evaluated[b]);
                #[cfg(test)]
                PAIRS.with(|count|count.set(count.get()+readers.len()));
                for a in readers {
                    depends[a][b] = effects[remaining[a]].evaluate(self, world, layer) != evaluated[a];
                }
                for (at, seen) in saved { world.objects[at].1 = seen; }
            }
            let mut reachable = depends.clone();
            for k in 0..n { for a in 0..n { for b in 0..n {
                reachable[a][b] |= reachable[a][k] && reachable[k][b];
            } } }
            // Ignore dependencies within a cycle, then choose the earliest timestamp
            // among ready effects. Recompute after each application (613.8b-c).
            let ready = |a: usize| (0..n).all(|b| !depends[a][b] || reachable[b][a]);
            let next = (0..n).find(|&a| ready(a)).expect("dependency graph has a ready component");
            effects[remaining.remove(next)].commit(world, &evaluated[next]);
        }
    }

    fn unchanged_seen(&self, o: ObjId, before: Option<Layer>) -> Seen {
        let mut seen = self.base_seen(o);
        for layer in LAYERS.into_iter().take_while(|l| before.is_none_or(|b| *l < b)) {
            if layer == Layer::DefinePt { seen.no_value_pt(); }
            if layer == Layer::Types && self.obj(o).bestowed { crate::bestow::types().apply(&mut seen); }
            self.layer_counters(o, &mut seen, layer);
        }
        self.finish_seen(o, &mut seen);
        seen
    }

    fn layer_counters(&self, o: ObjId, seen: &mut Seen, layer: Layer) {
        if self.obj(o).zone != Zone::Battlefield { return; }
        if layer == Layer::Abilities {
            for k in self.obj(o).counters.iter().filter(|(_, n)| **n > 0).filter_map(|(kind, _)| counter_keyword(kind)) {
                if !seen.has(k) { seen.keywords.push(Held::plain(k)); }
            }
        }
        if layer == Layer::ChangePt {
            for (kind, n) in &self.obj(o).counters {
                if let Some((x, y)) = pt_counter(kind) {
                    seen.pt = seen.pt.map(|(p, t)| (p + x * *n as i32, t + y * *n as i32));
                }
            }
        }
    }

    pub(crate) fn finish_seen(&self, o: ObjId, seen: &mut Seen) {
        if self.obj(o).zone == Zone::Battlefield && !seen.is(CardType::Creature) { seen.pt = None; }
    }

    /// Is this object of this card type, now: a permanent as the layers have it (613.1d), an
    /// object that has left the battlefield as it last was there, anything else as printed.
    pub fn is_a(&self, o: ObjId, t: CardType) -> bool {
        self.seen_for(o, Aspect::Types).is(t)
    }

    /// [`Game::seen`], as far as this question reads: the fields the aspect names are those
    /// of the whole view, the rest are as the layers applied left them and are not to be read.
    pub fn seen_for(&self, o: ObjId, aspect: Aspect) -> Seen {
        self.seen(o, aspect.cut())
    }

    /// A permanent's power, now.
    pub fn power(&self, o: ObjId) -> Option<i32> {
        self.seen(o, None).pt.map(|(p, _)| p)
    }

    /// A permanent's toughness, now.
    pub fn toughness(&self, o: ObjId) -> Option<i32> {
        self.seen(o, None).pt.map(|(_, t)| t)
    }
}

#[cfg(test)]
mod aspect_tests;
#[cfg(test)]
mod independence_tests;
#[cfg(test)]
pub(crate) mod footprint_tests;
#[cfg(test)]
mod world_tests;
