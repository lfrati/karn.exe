//! The abilities an object has, and where each comes from (CR 113).
//!
//! Three things are kept apart. A *definition* is immutable and shared by every game: a
//! card's, or the definition of one ability by itself, compiled once from the words an
//! effect gives in quotes ("has \"{T}: Add {G}.\"", 113.2c) or a rule gives (305.6). An
//! *instance* is what one game holds: an [`Origin`] saying which definition an ability of an
//! object belongs to and which giving of it this is. Inherited instances retain an immutable
//! definition and structural granting/donor provenance through shared family storage. An *occurrence* is an ability activated or
//! triggered: it keeps the key of its ability and its captured program, so it resolves as
//! what it was when it came to be, whatever its source has or is by then (113.7a).
//!
//! An object's abilities are its own definition's, the mana ability of each basic land type
//! it has (305.6), and those the abilities layer gives it (613.1f), which [`Game::seen`]
//! finds and [`crate::layers::Seen::given`] keeps. What an object was as it last existed
//! keeps the same list, so an ability that looks back at a departed object (603.10a) finds
//! the abilities it was given, not only those of its card.
//!
//! Two effects that give the same words give two instances ([`Grant`]): each triggers, each
//! counts its own activations, and one ending leaves the other.
//!
//! Not run, and refused where the words are compiled rather than approximated: a given
//! static ability that would change types, colors or definition membership (613.1d–f); a given ability that makes a linked
//! choice, is a keyword's or is a replacement outside the counter family; one that names what
//! gave it. Dynamic inheritance of activated abilities from battlefield creatures and fixed
//! graveyard card sets uses exact donor acquisitions in the layer world. Donor membership
//! reads current characteristics, independently of where an ability functions; borrowed
//! activation scope and source context belong to the receiver. Linked-zone donor sets and inherited
//! triggered abilities remain refused.
//! Ability removal and entry copying use the same effective view; selective non-keyword
//! removal remains refused. Captured occurrences never recheck live membership.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use crate::binding::{AbilityKey, Origin};
use crate::card::CardDef;
use crate::layers::Layer;
use crate::state::{Game, ObjId};
use crate::{Res, unsupported};

/// One ability compiled apart from any card, as the definition of that one ability: shared
/// by every object given it in every game. Two are the same when they are one definition.
#[derive(Clone, Copy)]
pub struct Template(&'static CardDef);

impl Template {
    pub(crate) fn def(self) -> &'static CardDef { self.0 }
    pub fn ability(self) -> &'static ir::Ability { &self.0.abilities[0] }
    /// A definition kept for every game as the definition of its one ability.
    pub(crate) fn keep(def: CardDef) -> Template { Template(Box::leak(Box::new(def))) }
}
impl PartialEq for Template {
    fn eq(&self, other: &Self) -> bool { std::ptr::eq(self.0, other.0) }
}
impl Eq for Template {}
impl std::fmt::Debug for Template {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "Template({:?})", self.ability()) }
}

/// The definition of this one ability, compiled the first time it is asked for and kept for
/// every game, or why the engine does not give it. Whatever the definition of a card keeps
/// for each of its abilities, this keeps for the one: its program, cost, where it functions,
/// what it watches for and what it states.
pub fn template(ability: &ir::Ability) -> Res<Template> {
    static KEPT: OnceLock<Mutex<HashMap<ir::Ability, Res<Template>>>> = OnceLock::new();
    let kept = KEPT.get_or_init(Default::default);
    if let Some(found) = kept.lock().expect("no holder panics").get(ability) {
        return found.clone();
    }
    // compiled outside the lock: an ability whose effect gives another asks here again
    let compiled = compile(ability);
    // Another compilation may have won the race. Only the selected definition is
    // retained for the process lifetime; discard a redundant candidate normally.
    kept.lock().expect("no holder panics").entry(ability.clone())
        .or_insert_with(|| compiled.map(Template::keep)).clone()
}

fn compile(ability: &ir::Ability) -> Res<CardDef> {
    if matches!(ability, ir::Ability::Keywords(_)) {
        return unsupported("gains: keyword abilities in quotes");
    }
    if crate::program::registration::defining_pt(ability).is_some() {
        return unsupported("granted characteristic definition requires ordinary setting semantics (604.3a)");
    }
    let def = CardDef::of_ability(ability.clone(), Vec::new());
    def.admission.clone()?;
    // A granted static can participate in layer 6 through its actual grant identity.
    // Earlier-layer changes and recursive definition grants need a wider dependency model.
    if def.characteristics.iter().filter_map(|r| r.result.as_ref().ok()).any(|c|
        c.changes_in(Layer::Types) || c.changes_in(Layer::Colors) || c.changes_membership()) {
        return unsupported("gains: a static ability that changes types, colors or definition membership (613.1d-f)");
    }
    if def.links.produces() {
        return unsupported("gains: an ability that makes a linked choice (607.5)");
    }
    // Counter modifiers use the effective grant's identity and controller, including
    // prospective entry. Other replacement families need their own scope evidence.
    if def.replacements.iter().any(|r|!matches!(r.result,Ok(crate::replace::Replacement::Counters{..}))) {
        return unsupported("gains: a replacement effect outside the counter family");
    }
    Ok(def)
}

/// Compile an emblem's text once using the shared definition and program machinery.
/// Triggered, characteristic and restriction registrations share command-zone discovery
/// (114.4). Activated and replacement abilities remain explicitly refused.
pub(crate) fn emblem(granted: &[ir::action::Granted]) -> Res<Template> {
    let [ir::action::Granted::Ability(ability)] = granted else {
        return unsupported("emblem: only a single triggered or supported static ability is implemented (114.4)");
    };
    if !matches!(ability.as_ref(), ir::Ability::Triggered { .. } | ir::Ability::Text(_)) {
        return unsupported("emblem: only a single triggered or supported static ability is implemented (114.4)");
    }
    let definition = template(ability)?;
    if !matches!(ability.as_ref(), ir::Ability::Triggered { .. }) && (definition.def().characteristics.is_empty() && definition.def().restrictions.is_empty()) {
        return unsupported("emblem: static ability needs command-zone discovery in its consumer (114.4)");
    }
    if definition.def().scope(0) != Some(Ok(crate::functions::Scope::In(crate::Zone::Battlefield))) {
        return unsupported("emblem: ability refers to another functioning zone (114.4)");
    }
    Ok(definition)
}

/// An immutable definition/face retained independently of the donor's later state.
#[derive(Clone)]
pub(crate) enum FrozenDefinition {
    Face { defs: Arc<Vec<CardDef>>, face: crate::copy::Definition, token_back: Option<bool> },
    Template(Template),
}
impl std::fmt::Debug for FrozenDefinition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Face { face, token_back, .. } => f.debug_struct("Face").field("face", face).field("token_back", token_back).finish(),
            Self::Template(what) => what.fmt(f),
        }
    }
}
impl PartialEq for FrozenDefinition {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Face { face: x, token_back: a, .. }, Self::Face { face: y, token_back: b, .. }) => x == y && a == b,
            (Self::Template(a), Self::Template(b)) => a == b,
            _ => false,
        }
    }
}
impl Eq for FrozenDefinition {}
impl FrozenDefinition {
    /// Audit-only structural identity of this frozen declaring definition.
    pub(crate) fn identity(&self) -> serde_json::Value {
        use serde_json::json;
        match self {
            Self::Template(what) => json!({ "template": what.ability() }),
            Self::Face { face: crate::copy::Definition::Card(i, cast), .. } => json!({ "card": i, "face": format!("{cast:?}") }),
            Self::Face { face: crate::copy::Definition::Back(i), .. } => json!({ "card": i, "face": "back" }),
            Self::Face { token_back, .. } => {
                let def = self.def();
                json!({ "token_back": token_back, "definition": { "name": def.name(), "cost": def.cost(),
                    "types": def.types(), "abilities": def.abilities.iter().enumerate().map(|(i, a)| def.activation_declaration(i).unwrap_or(a)).collect::<Vec<_>>(), "modes": def.modes } })
            }
        }
    }
    fn of_values(game: &Game, values: &crate::copy::Values, token_back: Option<bool>) -> Option<Self> {
        match &values.definition {
            Some(face) => Some(Self::Face { defs: game.defs.clone(), face: face.clone(), token_back }),
            None => values.chars.predefined.and_then(|k| crate::token::definition(k).ok()).map(|d| Self::Template(Template(d))),
        }
    }
    /// The finite cost-owner wire vocabulary authenticates a whole, single-faced
    /// printed definition. Derived rows and alternate faces need a distinct identity.
    pub(crate) fn printed_cost_definition(&self) -> Option<&CardDef> {
        let Self::Face { face: crate::copy::Definition::Card(_,crate::card::CastAs::Whole), token_back:None, .. }=self else {return None};
        let def=self.def();
        (def.back.is_none() && def.halves.is_empty() && !def.is_room()).then_some(def)
    }
    pub(crate) fn def(&self) -> &CardDef {
        match self {
            Self::Face { defs, face, .. } => face.definition(defs).expect("retained definition exists"),
            Self::Template(what) => what.def(),
        }
    }
}

/// A granting instance and an exact donor ability family, with immutable definition data.
/// The outer AbilityKey keeps the donor's original row, so acquired linked rows share this
/// family while distinct donor acquisitions remain distinct (602.5c, 607.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InheritedFamily {
    pub(crate) grant: crate::binding::Grant,
    pub(crate) part: (usize, usize),
    pub(crate) grant_ability: AbilityKey,
    pub(crate) grant_definition: FrozenDefinition,
    pub(crate) donor: ObjId,
    pub(crate) donor_origin: Origin,
    pub(crate) definition: FrozenDefinition,
}

impl InheritedFamily {
    /// An offered replacement identity must describe the retained acquisition, not a
    /// current visible ordinal. Nodes recurse only through the finite provenance DAG.
    /// Derived token faces are immutable within an acquisition: another creation changes
    /// donor ObjId, another copy acquisition changes Copied stamp, and changing token face
    /// changes token_back. Thus canonical face data cannot alias a distinct reachable
    /// family solely because its immutable backing Arc has a different allocation.
    pub fn identity(&self) -> serde_json::Value {
        use serde_json::json;
        fn origin(o: &Origin) -> serde_json::Value {
            match o {
                Origin::Own => json!("own"), Origin::Copied(stamp) => json!({ "copied": stamp }),
                Origin::Rule(what) => json!({ "rule": what.ability() }),
                Origin::Given { grant, what } => json!({ "by": grant.by.map(|o| o.0), "stamp": grant.stamp, "part": grant.part, "given": what.ability() }),
                Origin::Inherited(family) => family.identity(),
            }
        }
        json!({ "grant": { "by": self.grant.by.map(|o| o.0), "stamp": self.grant.stamp, "part": self.grant.part,
            "part_in_ability": self.part, "ability": self.grant_ability.index, "origin": origin(&self.grant_ability.origin), "definition": self.grant_definition.identity() },
            "donor": self.donor.0, "donor_origin": origin(&self.donor_origin), "definition": self.definition.identity() })
    }
}

/// Storage for one selected member. Borrowed ordinary definitions retain the common path;
/// inherited members own the immutable family from which metadata is borrowed locally.
#[derive(Debug, Clone)]
pub(crate) enum DefinitionView<'g> {
    Borrowed(&'g CardDef),
    Inherited(Arc<InheritedFamily>),
}
impl std::ops::Deref for DefinitionView<'_> {
    type Target = CardDef;
    fn deref(&self) -> &CardDef {
        match self { Self::Borrowed(def) => def, Self::Inherited(family) => family.definition.def() }
    }
}
#[derive(Clone)]
pub struct Member<'g> {
    pub(crate) key: AbilityKey,
    definition: DefinitionView<'g>,
}
impl std::fmt::Debug for Member<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { (**self).fmt(f) }
}
impl Member<'_> {
    pub(crate) fn def(&self) -> &CardDef { &self.definition }
}
impl std::ops::Deref for Member<'_> {
    type Target = ir::Ability;
    fn deref(&self) -> &ir::Ability {
        self.def().abilities.iter().chain(self.def().keyword_abilities.iter().map(|(_, a)| a))
            .nth(self.key.index).expect("selected member retains its row")
    }
}

/// Which of an object's abilities are asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum View {
    /// Those of its own definition: all it has before the abilities layer (613.1f), which is
    /// what the layers up to that one are computed from.
    Own,
    /// What survives the abilities layer, now or as last known (603.10a).
    /// The caller checks once whether any effect can change definition membership;
    /// without one, live objects use their own definitions without allocating.
    Effective { membership_changed: bool },
    /// All it has now, with what the rules give for what it is after every layer (305.6).
    Now,
}

/// The definitions an object's abilities belong to, in one view of it: its own, then the
/// rules', then those it was given in timestamp order, each with the origin an ability of it
/// is known by. Asked of every permanent wherever the game is looked at, so one that has
/// only its own definition's, as nearly all do, costs no more than naming that definition.
pub(crate) struct Origins<'a> {
    own: Option<&'a CardDef>,
    origin: Origin,
    more: Vec<(Origin, Template)>,
    at: usize,
}

impl<'a> Origins<'a> {
    #[inline(always)]
    fn own(own: Option<&'a CardDef>, origin: Origin) -> Self { Origins { own, origin, more: Vec::new(), at: 0 } }
}

impl<'a> Iterator for Origins<'a> {
    type Item = (Origin, &'a CardDef);
    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(def) = self.own.take() {
            return Some((self.origin.clone(), def));
        }
        let (origin, what) = self.more.get(self.at)?.clone();
        self.at += 1;
        Some((origin, what.def()))
    }
}

impl Game {
    /// The shared definition an ability of this object belongs to: its own (the face it was
    /// cast as, the rule text of a predefined token), or the one a rule or an effect gives.
    pub(crate) fn definition<'g>(&'g self, o: ObjId, origin: &'g Origin) -> Option<&'g CardDef> {
        match origin {
            Origin::Own => self.def(o),
            Origin::Copied(stamp) => self.copied_definition(o, *stamp),
            Origin::Inherited(family) => Some(family.definition.def()),
            Origin::Rule(what) | Origin::Given { what, .. } => Some(what.def()),
        }
    }

    pub(crate) fn frozen_definition(&self, o: ObjId, origin: &Origin) -> Option<FrozenDefinition> {
        match origin {
            Origin::Inherited(family) => Some(family.definition.clone()),
            Origin::Rule(what) | Origin::Given { what, .. } => Some(FrozenDefinition::Template(*what)),
            Origin::Copied(stamp) => FrozenDefinition::of_values(self, self.copied_values(o, *stamp)?, None),
            Origin::Own => self.obj(o).emblem.map(FrozenDefinition::Template)
                .or_else(|| FrozenDefinition::of_values(self, &self.copiable(o), self.obj(o).token_faces.as_ref().map(|_| self.obj(o).transformed || self.obj(o).cast_as == crate::card::CastAs::Face(1)))),
        }
    }

    /// The definitions whose abilities this object has, in this view: the one place that
    /// says where an object's abilities come from. Whatever looks for a kind of ability
    /// (what triggers, what is in force, what a layer applies, what is activated) filters
    /// these for what each definition keeps compiled of that kind.
    #[inline(always)]
    pub(crate) fn origins(&self, o: ObjId, view: View) -> Origins<'_> {
        let own = self.def(o);
        if view == View::Own || (view == View::Effective { membership_changed: false } && self.obj(o).zone != crate::Zone::Gone) {
            return Origins::own(own, self.copy_origin(o));
        }
        // what it has is settled by the abilities layer (613.1f), and the land types the
        // rule's abilities come from (305.6) before it; nothing of layer 7 is read
        let seen = self.seen_for(o, crate::layers::Aspect::Abilities);
        self.origins_seen(o, &seen, view == View::Now)
    }

    /// Membership in a supplied characteristic view, including a complete pre-event view.
    pub(crate) fn origins_seen(&self, o: ObjId, seen: &crate::layers::Seen, intrinsic: bool) -> Origins<'_> {
        let mut more = Vec::new();
        if intrinsic {
            more.extend(crate::activate::intrinsic(seen).map(|what| (Origin::Rule(what), what)));
        }
        more.extend(seen.given.iter().copied().map(|(grant, what)| (Origin::Given { grant, what }, what)));
        Origins { own: self.def(o).filter(|_| seen.base_abilities), origin: self.copy_origin(o), more, at: 0 }
    }

    /// The abilities this object has now, each as the identity an occurrence of it keeps, in
    /// the order of [`Game::abilities`]: its own definition's, the rule's for each basic
    /// land type it has (305.6), then those it was given, in timestamp order.
    pub(crate) fn members_seen(&self, o: ObjId, seen: &crate::layers::Seen, intrinsic: bool) -> Vec<Member<'_>> {
        let mut members: Vec<_> = self.origins_seen(o, seen, intrinsic).flat_map(|(origin, def)|
            (0..def.abilities.len() + def.keyword_abilities.len()).map(move |index| Member {
                key: AbilityKey { origin: origin.clone(), index }, definition: DefinitionView::Borrowed(def),
            })).collect();
        members.extend(seen.inherited.iter().map(|key| {
            let Origin::Inherited(family) = &key.origin else { unreachable!("inherited member has its family") };
            Member { key: key.clone(), definition: DefinitionView::Inherited(family.clone()) }
        }));
        members
    }
    pub(crate) fn members(&self, o: ObjId) -> Vec<Member<'_>> {
        self.members_seen(o, &self.seen_for(o, crate::layers::Aspect::Abilities), true)
    }
    pub(crate) fn held(&self, o: ObjId) -> Vec<AbilityKey> {
        self.members(o).into_iter().map(|member| member.key).collect()
    }

    /// The ability at this place among [`Game::abilities`]: how a player names one. What is
    /// kept of it afterwards is this key, which does not move when the list does.
    pub(crate) fn key(&self, o: ObjId, index: usize) -> Result<AbilityKey, String> {
        // of its own definition without asking what it is now: nearly every ability named
        if !self.membership_changed() && self.obj(o).zone != crate::Zone::Gone && index < self.def(o).map_or(0, |d| d.abilities.len() + d.keyword_abilities.len()) {
            return Ok(AbilityKey { origin: self.copy_origin(o), index });
        }
        self.held(o).get(index).cloned().ok_or_else(|| "that is not an activated ability of that permanent (602.1c)".to_string())
    }

    /// The ability a key names, of this object.
    pub(crate) fn ability_of<'g>(&'g self, o: ObjId, key: &'g AbilityKey) -> Option<&'g ir::Ability> {
        let def = self.definition(o, &key.origin)?;
        def.abilities.iter().chain(def.keyword_abilities.iter().map(|(_, a)| a)).nth(key.index)
    }
}
