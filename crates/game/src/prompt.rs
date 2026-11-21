//! What whoever plays a seat is told and how they answer: the player-facing contract (API
//! [`API`]). A player is prompted, thinks, and answers, and is not assumed to watch the game
//! between prompts. So each [`Prompt`] carries the game as its seat may see it (a seat-filtered
//! [`Snapshot`]), the readable text of everything in it, and, when that seat is the one asked,
//! the question with who asks it and why: the spell or ability whose effect or announcement it
//! belongs to, as that occurrence was, even after its source has left.
//!
//! Every entity a player may point to has a label: a player `p0`; an object `o12`, for as long
//! as it is that object (a card that changes zones is a new object with a new label, 400.7,
//! and a card in a hidden zone has none another player could follow); an ability `o12.a3`,
//! one of that object's abilities as the prompt lists it. A label points; it promises no
//! move is legal. Object labels are the seat's own ([`Labels`]): numbered in the order the
//! seat was first shown each object, never by how the engine stores objects, so they say
//! nothing the seat was not shown. A label in an answer that names nothing this seat may
//! refer to now is refused before the game is asked ([`ResponseError::Unreferable`]), as one
//! that names nothing at all is; everything else the game checks as it checks any answer.
//!
//! Pending choices expose their rule-defined domains. The caller supplies each action.
//!
//! The typed contract is this module; `game-serve` says the same as JSON (`prompt`/`answer`).

use crate::binding::{AbilityKey, Origin};
use crate::legal::Asked;
use crate::simulation::{Fault, RequestId, ResponseError, Simulation};
use crate::snapshot::Snapshot;
use crate::state::{Answer, Decision, Game, ObjId, PlayerId, Triggered};
use crate::{Move, Target};
use ir::Ability;
use std::collections::BTreeMap;
use std::sync::Arc;

/// The version of the player-facing contract: what a prompt says and an answer may say.
pub const API: &str = "player/1";

/// A label as written: a player, or an object or one of its abilities by the number the
/// seat's [`Labels`] gave the object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Label {
    Player(PlayerId),
    Object(u64),
    /// One of this object's abilities, by its place among those the prompt lists.
    Ability(u64, usize),
}

/// The names one seat has been given for objects: the first object shown to it is `o1`, the
/// next new one `o2`, and so on. A name stays with its object for as long as it is that object
/// and is never given to another. What it numbers is only what the seat was shown, in the
/// order shown, so a name says nothing of objects it was not (how many there are, which were
/// made or forgotten). A copy of a simulation keeps its names and goes on numbering by itself.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Labels {
    objects: Vec<ObjId>,
    numbers: std::collections::HashMap<ObjId, u64>,
}

impl Labels {
    /// This object's number, given it now if it has none.
    pub fn name(&mut self, id: ObjId) -> u64 {
        if let Some(n) = self.numbers.get(&id) { return *n; }
        self.objects.push(id);
        let n = self.objects.len() as u64;
        self.numbers.insert(id, n);
        n
    }

    /// The number this object was given, if it was.
    pub fn number(&self, id: ObjId) -> Option<u64> { self.numbers.get(&id).copied() }

    /// The object this number was given to, if any was.
    pub fn object(&self, number: u64) -> Option<ObjId> {
        number.checked_sub(1).and_then(|i| self.objects.get(i as usize)).copied()
    }

    /// Every number given, with its object, in the order given.
    pub fn given(&self) -> impl Iterator<Item = (u64, ObjId)> + '_ {
        self.objects.iter().enumerate().map(|(i, id)| (i as u64 + 1, *id))
    }
}

impl Label {
    pub fn parse(text: &str) -> Option<Label> {
        let number = |s: &str| (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse::<u64>().ok()).flatten();
        if let Some(seat) = text.strip_prefix('p') { return number(seat).map(|n| Label::Player(n as usize)); }
        let object = text.strip_prefix('o')?;
        match object.split_once(".a") {
            Some((o, a)) => Some(Label::Ability(number(o)?, number(a)? as usize)),
            None => number(object).map(Label::Object),
        }
    }
}

impl std::fmt::Display for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Label::Player(p) => write!(f, "p{p}"),
            Label::Object(o) => write!(f, "o{o}"),
            Label::Ability(o, a) => write!(f, "o{o}.a{a}"),
        }
    }
}

/// One prompt for one seat. The same while the game is unchanged: prompting again gives the
/// same view (the simulation keeps its snapshot) and the same request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    pub api: &'static str,
    pub seat: PlayerId,
    /// The request the game waits on, whoever must answer it; None once the game is over.
    pub request: Option<RequestId>,
    /// The seat entitled to answer it (723.5): this one or another.
    pub waiting_for: Option<PlayerId>,
    /// The game as this seat may see it (400.2).
    pub view: Arc<Snapshot>,
    /// What each object of the view is, readably: every object in `view.objects`.
    pub entities: BTreeMap<ObjId, Entity>,
    /// The question, only when this seat must answer it (or it is this seat's own decision
    /// another seat makes, 723.5).
    pub decision: Option<PlayerDecision>,
}

/// An object's text as it is now (613.1f: what effects leave of its abilities). An activated
/// or triggered ability on the stack has the text of the ability that created it and no
/// other characteristics (405.4): only `text` and `occurrence`, nothing of its source's card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entity {
    /// Its mana cost, as printed on the face it has (202.1).
    pub mana_cost: Option<String>,
    /// Its rules text: one line for each ability it has, its own in printed order, then those
    /// the rules or effects give it. Of an ability on the stack, the one line of the ability
    /// that created it, as [`Occurrence::text`] says it: of a delayed or reflexive one, its own
    /// "when", "whenever" or "at" sentences (603.3, 603.7, 603.12), the ability that made it
    /// being [`Occurrence::created_by`].
    pub text: Vec<String>,
    /// Its activated abilities, each by the place an ability label names.
    pub abilities: Vec<AbilityEntry>,
    /// The printed text of the card a face-down or face-down exiled object is, for a seat
    /// that may look at it now (708.5, 406.3). Another seat is told only that it is face down.
    pub card: Option<Vec<String>>,
    /// The alternative characteristics its inset frame defines, which it has without using
    /// them (715.2a, 720.2a, 722.2a). None for a spell cast as its Adventure or Omen, which
    /// has only those (715.3b, 720.3b), and for a prepared copy, whose own they are (722.3c).
    pub inset: Option<Inset>,
    /// Of a spell or ability on the stack: what it is and where it came from.
    pub occurrence: Option<Occurrence>,
}

/// The characteristics an inset frame defines, as printed (715.2, 720.2, 722.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inset {
    pub kind: InsetKind,
    pub name: String,
    pub mana_cost: Option<String>,
    pub supertypes: Vec<ir::Supertype>,
    pub types: Vec<ir::CardType>,
    pub subtypes: Vec<ir::Subtype>,
    /// The colors of its mana cost's symbols (105.2).
    pub colors: Vec<ir::Color>,
    /// One line for each of its abilities, in printed order.
    pub text: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsetKind {
    /// An adventurer card's Adventure, which its owner may cast from the card (715.3).
    Adventure,
    /// An omen card's Omen, which its owner may cast from the card (720.3).
    Omen,
    /// A preparation card's prepare spell, cast only as the copy a prepared permanent
    /// makes (722.3, 722.3c).
    Prepare,
}

impl InsetKind {
    /// What the wire calls it: for the two cast from the card, the `cast_as` that casts it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Adventure => "adventure",
            Self::Omen => "omen",
            Self::Prepare => "prepare_spell",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbilityEntry {
    /// Its place among the object's abilities: the `a` of its label.
    pub index: usize,
    pub text: String,
    /// An ability an effect gives it (613.1f): the permanent whose static ability gives it,
    /// if this seat may see that permanent, or None for an effect that resolved.
    pub granted: Option<Option<ObjId>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OccurrenceKind {
    Spell,
    Activated,
    Triggered,
    /// A delayed or reflexive triggered ability an effect made (603.7, 603.12).
    Delayed,
    /// An opening-hand action a card's ability lets its player take (103.6): followed as
    /// it is taken, with no stack.
    Opening,
}

impl OccurrenceKind {
    pub fn name(self) -> &'static str {
        match self { Self::Spell => "spell", Self::Activated => "activated", Self::Triggered => "triggered", Self::Delayed => "delayed",
            Self::Opening => "opening" }
    }
}

/// A spell or ability, as it occurred: on the stack, being put there, or being announced.
/// Its source is named as the occurrence keeps it, not as the battlefield is now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Occurrence {
    pub kind: OccurrenceKind,
    /// The stack object, once there is one.
    pub object: Option<ObjId>,
    /// The object whose ability it is, when this seat may refer to it (the incarnation that
    /// had the ability; it may since have left). None for a spell, a rule's ability (725.2),
    /// or a source this seat may not see.
    pub source: Option<ObjId>,
    /// That source's name, as it was when the ability was put on the stack: a name the
    /// harness shows, no rule's and no characteristic of the ability (405.4); empty if it
    /// has none to this seat.
    pub source_name: String,
    /// The source is no longer where it was when the ability came from it (400.7).
    pub source_left: bool,
    /// Who controls the spell or ability (the ability's controller, 113.8).
    pub controller: PlayerId,
    /// Its text. None for a spell, whose text is its object's.
    pub text: Option<String>,
    /// Of a delayed or reflexive ability: the text of the ability that created it.
    pub created_by: Option<String>,
    /// The ability was given to its source by an effect (613.1f).
    pub granted: bool,
    /// Of a modal spell or ability, the modes chosen for it (700.2): each by its place among
    /// the modes printed, with its text.
    pub modes: Vec<(usize, String)>,
}

/// Why a question is asked: which rule or procedure it belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// The player has priority (117.1).
    Priority,
    /// Mulligans (103.5).
    Mulligan,
    /// Discarding to hand size in the cleanup step (514.1).
    Cleanup,
    /// Declaring attackers or blockers, or dividing combat damage (508, 509, 510).
    Combat,
    /// A triggered ability being put on the stack: its modes or targets (603.3c-d).
    Trigger,
    /// Triggered abilities being put on the stack in an order (603.3b).
    TriggerOrder,
    /// Paying for a spell or ability being cast or activated (601.2g-h).
    Announcement,
    /// The effect of a resolving spell or ability (608.2).
    Resolution,
    /// A replacement effect or the way something enters (614, 616).
    Replacement,
    /// A rule other than these: state-based actions, turn-based actions.
    Rules,
}

impl Reason {
    pub fn name(self) -> &'static str {
        match self {
            Self::Priority => "priority", Self::Mulligan => "mulligan", Self::Cleanup => "cleanup", Self::Combat => "combat",
            Self::Trigger => "trigger", Self::TriggerOrder => "trigger_order", Self::Announcement => "announcement",
            Self::Resolution => "resolution", Self::Replacement => "replacement", Self::Rules => "rules",
        }
    }
}

/// The pending question for the seat that answers it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerDecision {
    pub id: RequestId,
    /// The seat entitled to answer (723.5).
    pub chooser: PlayerId,
    /// The player whose action, resources or choice it is (723.3).
    pub acting: PlayerId,
    /// What may be answered: the request's whole answer domain. Priority's is the move
    /// language itself, with no listing.
    pub question: Asked,
    pub reason: Reason,
    /// The spell or ability it is asked for, when there is one.
    pub occurrence: Option<Occurrence>,
    /// Of a trigger-order question: each ability, in the question's order.
    pub ordered: Vec<Occurrence>,
    /// The object about to enter, whose entry the question is about (614.12).
    pub entering: Option<ObjId>,
    /// Of a "may": what it lets the player do, and for a zone change where to (608.2d).
    pub optional: Option<(ir::Verb, Option<crate::state::Placement>)>,
}

/// The text of an ability as its key names it, of this source: what an occurrence of it keeps.
/// A modal ability's text is its "Choose …" and every mode under it (700.2).
pub(crate) fn ability_text(g: &Game, source: Option<ObjId>, key: AbilityKey) -> Option<String> {
    // the monarch's abilities have the text 725.2 gives them
    if let Origin::Rule(what) = &key.origin && let Some(text) = crate::monarch::Rule::stated(*what) { return Some(text.to_string()); }
    let ability = match key.origin {
        Origin::Own | Origin::Copied(_) | Origin::Inherited(_) => source.and_then(|s| g.ability_of(s, &key)),
        Origin::Rule(what) | Origin::Given { what, .. } => Some(what.ability()),
    }?;
    let ability = source.and_then(|s| g.definition(s, &key.origin))
        .and_then(|d| d.activation_declaration(key.index)).unwrap_or(ability);
    let name = source.map_or("", |s| g.obj(s).chars.name.as_str());
    let rows = source.map_or(&[][..], |s| g.mode_rows(s, &key));
    let forms = source.and_then(|s| g.definition(s, &key.origin)).map_or(&[][..], |d| d.forms(key.index));
    let mode_forms = source.and_then(|s| g.definition(s, &key.origin)).map_or(&[][..], |d| d.mode_forms(key.index));
    Some(with_modes(print(ability, name, forms)?, rows, name, mode_forms))
}

/// The line of a keyword an effect gives an object, which no definition of it declares
/// (613.1f): "Rebound", "Warp {2}{R}".
fn granted_keyword_text(keyword: &ir::token::KeywordInstance, name: &str) -> Option<String> {
    print(&Ability::Keywords(vec![keyword.clone()]), name, &[])
}

/// Print an own ability from immutable production-time evidence, without a live object.
pub(crate) fn frozen_ability_text(def: &crate::CardDef, index: usize, name: &str) -> Option<String> {
    let ability = def.abilities.iter().chain(def.keyword_abilities.iter().map(|(_, a)| a)).nth(index)?;
    let ability = def.activation_declaration(index).unwrap_or(ability);
    Some(with_modes(print(ability, name, def.forms(index))?, def.modes_of(index), name, def.mode_forms(index)))
}

/// An ability's line, with the mode rows printed under it, one a line.
/// `forms`: the Oracle text's words for what each row names ([`crate::CardDef::mode_forms`]).
fn with_modes(line: String, rows: &[ir::Mode], name: &str, forms: &[Vec<String>]) -> String {
    rows.iter().enumerate().filter_map(|(i, m)| print_mode(m, name, forms.get(i).map_or(&[], |f| f.as_slice())))
        .fold(line, |text, row| text + "\n" + &row)
}

fn print_mode(mode: &ir::Mode, name: &str, forms: &[String]) -> Option<String> {
    let text = grammar::ability::abilities().mode.print_text(mode, true)?;
    Some(named(&text, name, forms))
}

/// An ability's text, with the name of the object it is on where the text names that object
/// ("~", 201.4b); "this" for an object with no name (a face-down permanent, 708.2). `forms`
/// are the Oracle text's words for each object it names ([`crate::CardDef::forms`]).
fn print(ability: &Ability, name: &str, forms: &[String]) -> Option<String> {
    let text = match revealed_as_printed(ability) {
        Some(printed) => grammar::ability::abilities().ability.print_text(&printed, true),
        None => grammar::ability::abilities().ability.print_text(ability, true),
    }?;
    Some(named(&text, name, forms))
}

/// Ninjutsu's cost reveals the card from its player's hand (702.49a); that the reveal lasts
/// until the ability leaves the stack is its rule's annotation (702.49b), which the text does
/// not say. So the cost is printed as the reveal its rule words.
fn revealed_as_printed(ability: &Ability) -> Option<Ability> {
    let Ability::Activated { cost, effect } = ability else { return None };
    if !cost.0.iter().any(|p| matches!(p, ir::CostPart::RevealUntilLeavesStack(_))) { return None; }
    let cost = ir::Cost(cost.0.iter().cloned().map(|part| match part {
        ir::CostPart::RevealUntilLeavesStack(objects) => ir::CostPart::Action(ir::Action::Reveal(objects)),
        other => other,
    }).collect());
    Some(Ability::Activated { cost, effect: effect.clone() })
}

/// Text with the names it refers to: "~" is the object the ability is on, by `name`. Inside a
/// quoted ability that object grants, "~" is what will have the quoted ability, "this token"
/// or "this creature" as the text says, and "§" is the granting object, by `name` (201.5a).
/// With `forms`, one for each "~" and "§" of the text in order, a quoted reference is worded
/// as the Oracle text words it: "this emblem", "this card", "this Aura".
pub(crate) fn named(text: &str, name: &str, forms: &[String]) -> String {
    let own = if name.is_empty() { "this" } else { name };
    let refs = text.chars().filter(|c| matches!(*c, '~' | oracle::normalize::GRANTING_SELF)).count();
    let mut said = (refs == forms.len()).then(|| forms.iter());
    let mut out = String::with_capacity(text.len());
    let mut quoted: Option<&str> = None;
    for c in text.chars() {
        let form = if matches!(c, '~' | oracle::normalize::GRANTING_SELF) { said.as_mut().and_then(|f| f.next()) } else { None };
        if let (Some(form), Some(_)) = (form, quoted) {
            out.push_str(form);
            continue;
        }
        match c {
            '"' | '“' | '”' => {
                quoted = if quoted.is_some() { None } else { Some(grantee(&out)) };
                out.push(c);
            }
            '~' => match quoted {
                Some(noun) => {
                    // "This token gets …" where it begins a sentence of the quote
                    let start = out.trim_end().chars().last().is_none_or(|c| matches!(c, '"' | '“' | '.' | ':'));
                    out.push_str(if start { "This " } else { "this " });
                    out.push_str(noun);
                }
                None => out.push_str(own),
            },
            oracle::normalize::GRANTING_SELF => out.push_str(own),
            _ => out.push(c),
        }
    }
    out
}

/// What a quoted ability is given to, by the last such noun before the quote: Basking
/// Broodscale's Spawn is a 'token with "Sacrifice this token: …"'.
fn grantee(before: &str) -> &'static str {
    const NOUNS: &[&str] = &["token", "creature", "land", "artifact", "Equipment", "enchantment", "planeswalker", "permanent"];
    NOUNS.iter().filter_map(|n| before.rfind(n).map(|at| (at, *n))).max().map_or("permanent", |(_, n)| n)
}

fn mana_cost(cost: &ir::mana::ManaCost) -> String {
    cost.0.iter().map(|s| format!("{{{}}}", s.text())).collect()
}

/// The printed text of a definition, one line per ability as printed.
fn printed(def: &crate::CardDef) -> Vec<String> {
    def.abilities().iter().enumerate()
        .filter_map(|(i, a)| Some(with_modes(print(a, def.name(), def.forms(i))?, def.modes_of(i), def.name(), def.mode_forms(i)))).collect()
}

/// The inset spell a definition has, as printed: its Adventure, Omen or prepare spell.
fn inset(def: &crate::CardDef) -> Option<Inset> {
    let (kind, spell) = def.adventure().map(|s| (InsetKind::Adventure, s))
        .or_else(|| def.omen().map(|s| (InsetKind::Omen, s)))
        .or_else(|| def.prepare_spell().map(|s| (InsetKind::Prepare, s)))?;
    Some(Inset {
        kind,
        name: spell.name().to_owned(),
        mana_cost: spell.cost().map(mana_cost),
        supertypes: spell.supertypes().to_vec(),
        types: spell.types().to_vec(),
        subtypes: spell.subtypes().to_vec(),
        colors: spell.colors().to_vec(),
        text: printed(spell),
    })
}

impl Game {
    /// The text of the ability a key names, of this source: a printed ability, or one its
    /// keyword stands for ("Battle cry" is "Whenever this creature attacks, …", 702.91a).
    pub fn ability_text(&self, source: Option<ObjId>, key: AbilityKey) -> Option<String> {
        ability_text(self, source, key)
    }

    /// The mode rows printed under one of this object's abilities (700.2); none for a granted
    /// or a rules ability, whose text is all one line.
    fn mode_rows<'g>(&'g self, o: ObjId, key: &'g AbilityKey) -> &'g [ir::Mode] {
        match &key.origin {
            Origin::Own | Origin::Copied(_) | Origin::Inherited(_) => self.definition(o, &key.origin).map_or(&[], |d| d.modes_of(key.index)),
            Origin::Rule(_) | Origin::Given { .. } => &[],
        }
    }

    /// The modes chosen for a modal spell or ability on the stack (700.2a–b), each with its
    /// row's text, by its place among the modes printed.
    fn chosen_modes(&self, id: ObjId, holder: Option<ObjId>, key: AbilityKey) -> Vec<(usize, String)> {
        let o = self.obj(id);
        let rows = holder.map_or(&[][..], |h| self.mode_rows(h, &key));
        let forms = holder.filter(|_| rows.len() > 0).and_then(|h| self.definition(h, &key.origin)).map_or(&[][..], |d| d.mode_forms(key.index));
        o.modes.iter().filter_map(|m| Some((*m, print_mode(rows.get(*m)?, &o.chars.name, forms.get(*m).map_or(&[], |f| f.as_slice()))?))).collect()
    }

    /// The definition, among the one its source has now (an emblem's, 114.1), those of every
    /// face its source's card, copy or double-faced token has (707.8a, 712.8) and the
    /// face-down characteristics it may have had, whose ability compiled to this program, and
    /// which ability it is. Found by the program, a definition the source has now is never
    /// taken for one it had. A face-down source's ward trigger is the face-down definition's
    /// (701.58a, 702.168a), and stays its text once the source is turned face up; a
    /// transformed source's ability keeps its front face's text (113.7a).
    fn retained(&self, source: ObjId, program: &std::sync::Arc<crate::program::Program>) -> Option<(&crate::CardDef, usize)> {
        let o = self.obj(source);
        let mut defs: Vec<&crate::CardDef> = self.def(source).into_iter().chain(self.underlying(source)).collect();
        // a token's faces are those its creating effect gave it, not a card's (111.3, 707.8a)
        if let Some(faces) = &o.token_faces {
            defs.extend(std::iter::once(&faces.front).chain(&faces.back).filter_map(|face| face.definition(self)));
        }
        if let Some(card) = o.card {
            let def = &self.defs[card];
            defs.push(def);
            defs.extend(def.back.as_deref().into_iter().chain(&def.halves).chain(def.alternative_spell()));
        }
        use crate::face::Concealment;
        defs.extend([Concealment::Manifested, Concealment::Cloaked].map(|by| &**crate::face::definition(by)));
        defs.into_iter().find_map(|d| crate::program::ability_compiled_to(d, program).map(|i| (d, i)))
    }

    /// An object as this seat may read it. Only for an object this seat perceives.
    fn entity(&self, seat: PlayerId, id: ObjId) -> Entity {
        let o = self.obj(id);
        let occurrence = (o.zone == crate::Zone::Stack).then(|| self.occurrence(seat, id));
        let Some(shown) = self.characteristics(id) else {
            // an ability on the stack has the text of the ability that created it and no other
            // characteristics (405.4, 602.2a, 603.3): nothing of its source's card, which may be
            // face down (708.5), nor of its source's characteristics
            return Entity {
                mana_cost: None,
                text: occurrence.as_ref().and_then(|x| x.text.clone()).into_iter().collect(),
                abilities: Vec::new(),
                card: None,
                inset: None,
                occurrence,
            };
        };
        let def = shown.def();
        let keys = self.held(id);
        let abilities = self.abilities(id);
        let mut text = Vec::new();
        let mut activated = Vec::new();
        for (index, (key, ability)) in keys.iter().zip(&abilities).enumerate() {
            // a keyword's activated meaning stands beside the keyword's own line (702.1)
            let derived = matches!(key.origin, Origin::Own | Origin::Copied(_))
                && self.definition(id, &key.origin).is_some_and(|d| key.index >= d.abilities.len());
            let forms = self.definition(id, &key.origin).map_or(&[][..], |d| d.forms(key.index));
            let declaration = self.definition(id, &key.origin).and_then(|d| d.activation_declaration(key.index)).unwrap_or(ability);
            let line = print(declaration, &o.chars.name, forms).map(|l| with_modes(l, self.mode_rows(id, &key), &o.chars.name,
                self.definition(id, &key.origin).map_or(&[], |d| d.mode_forms(key.index))));
            if !derived && let Some(line) = &line { text.push(line.clone()); }
            if matches!(&**ability, Ability::Activated { .. }) {
                activated.push(AbilityEntry {
                    index,
                    text: line.unwrap_or_default(),
                    granted: match &key.origin {
                        Origin::Given { grant, .. } => Some(grant.by.filter(|by| self.perceives(seat, *by))),
                        Origin::Inherited(family) => Some(family.grant.by.filter(|by| self.perceives(seat, *by))),
                        _ => None,
                    },
                });
            }
        }
        // a keyword an effect gives it, which no definition of its abilities has (613.1f)
        let seen = self.seen_for(id, crate::layers::Aspect::Abilities);
        let defined: Vec<&crate::keyword::Held> = self.origins_seen(id, &seen, true).flat_map(|(_, d)| d.keywords.iter()).collect();
        for k in seen.keywords.iter().filter(|k| !defined.contains(k)) {
            let given = ir::token::KeywordInstance { keyword: k.keyword, param: k.param.clone() };
            if let Some(line) = granted_keyword_text(&given, &o.chars.name) { text.push(line); }
        }
        let shown_card = (o.hidden.is_some() || matches!(o.face, crate::face::Face::Down(_))) && self.sees(seat, id);
        Entity {
            // a card with two halves shows each half's cost as printed, not the combined one
            // (709.4d, 709.5)
            mana_cost: def.and_then(|d| match d.halves() {
                [] => d.cost().map(mana_cost),
                halves => Some(halves.iter().map(|h| h.cost().map(mana_cost).unwrap_or_default()).collect::<Vec<_>>().join(" // ")),
            }),
            text,
            abilities: activated,
            card: shown_card.then(|| self.physical_card(id)).flatten().map(|(_, def)| printed(def)),
            // the definition of what it is now: a face-down or hidden object's has no inset
            // (708.2a, 406.3a), a copy's is the copied one (715.2b, 720.2b, 722.2b)
            inset: def.and_then(inset),
            occurrence,
        }
    }

    /// The text an ability on the stack has (405.4, 113.7a), as every seat reads it; None for
    /// a spell, or an ability whose line the engine cannot say.
    pub fn stack_ability_text(&self, id: ObjId) -> Option<String> {
        // which seat asks changes only whether it perceives the source, not the text
        self.obj(id).ability.and_then(|_| self.occurrence(self.controller(id), id).text)
    }

    /// A spell or ability on the stack, as it occurred.
    fn occurrence(&self, seat: PlayerId, id: ObjId) -> Occurrence {
        let o = self.obj(id);
        let controller = self.controller(id);
        let Some((source, index)) = o.ability else {
            // a spell's text is the object's own; of the modes printed, those chosen
            let modes = (0..self.abilities(id).len()).find(|i| !self.mode_rows(id, &AbilityKey { origin: Origin::Own, index: *i }).is_empty())
                .map(|index| self.chosen_modes(id, Some(id), AbilityKey { origin: Origin::Own, index })).unwrap_or_default();
            return Occurrence { kind: OccurrenceKind::Spell, object: Some(id), source: None, source_name: o.chars.name.clone(),
                source_left: false, controller, text: None, created_by: None, granted: false, modes };
        };
        let invocation = o.invocations.as_ref().and_then(|i| i.as_ref().ok()).and_then(|i| i.first());
        let key = invocation.and_then(|i| i.ctx.ability.clone()).unwrap_or(AbilityKey { origin: Origin::Own, index });
        let kind = if o.delayed { OccurrenceKind::Delayed }
            else if invocation.is_some_and(|i| i.triggered) { OccurrenceKind::Triggered }
            else { OccurrenceKind::Activated };
        // the instructions it has are those it was put on the stack with (113.7a), found by the
        // program it runs among the faces its source's card has, whichever face is up now
        // (a delayed one's, the ability that made it, 603.7)
        let retained = invocation.and_then(|i| source.and_then(|s| self.retained(s, &i.program)));
        let keyword = invocation.and_then(|i| crate::program::keyword_delayed_text(&i.program));
        let printed = |(def, index): (&crate::CardDef, usize)| def.abilities.iter().chain(def.keyword_abilities.iter().map(|(_, a)| a)).nth(index)
            .and_then(|a| print(a, &o.chars.name, def.forms(index))).map(|line| with_modes(line, def.modes_of(index), &o.chars.name, def.mode_forms(index)));
        let line = match retained {
            Some(found) => printed(found),
            // never the line its source has now at that place: a source turned face up, or
            // whose face or copy effects changed, has another ability there (113.7a)
            None if matches!(key.origin, Origin::Own) => None,
            // a rule's or an effect's ability, or a copy effect's by its timestamp: the key
            // names that definition, whatever the source is now
            None => ability_text(self, source, key.clone()),
        };
        let (text, created_by) = if o.delayed {
            // a delayed or reflexive one has its own text, the "when", "whenever" or "at"
            // sentences its creator's text says (603.3, 603.7, 603.12); the ability that
            // created it is said beside it: a keyword's (702.88a, 702.185a) or its source's
            let own = keyword.map(str::to_string)
                .or_else(|| invocation.and_then(|i| crate::program::delayed_text(&i.program)).map(|t| named(&t, &o.chars.name, &[])));
            let creator = match invocation.and_then(|i| i.keyword_producer.as_deref()) {
                Some((def, key)) if keyword.is_some() => printed((def.def(), key.index)),
                // a keyword an effect gave the spell (613.1f), as it was then
                _ if keyword.is_some() => invocation.and_then(|i| i.granted_keyword.as_deref()).and_then(|k| granted_keyword_text(k, &o.chars.name)),
                _ => line,
            };
            (own, creator)
        } else { (line, None) };
        let modes = match retained {
            Some((def, index)) => o.modes.iter().filter_map(|m| Some((*m, print_mode(def.modes_of(index).get(*m)?, &o.chars.name, def.mode_forms(index).get(*m).map_or(&[], |f| f.as_slice()))?))).collect(),
            None if matches!(key.origin, Origin::Own) => Vec::new(),
            None => self.chosen_modes(id, source, key.clone()),
        };
        Occurrence {
            kind, object: Some(id),
            source: source.filter(|s| self.perceives(seat, *s)),
            // shown by the harness's convention: the source's name copied onto the object
            // when it was put on the stack, as every player sees it (no characteristic, 405.4)
            source_name: o.chars.name.clone(),
            source_left: source.is_some_and(|s| self.obj(s).zone == crate::Zone::Gone),
            controller,
            text,
            created_by,
            granted: matches!(key.origin, Origin::Given { .. } | Origin::Inherited(_)),
            modes,
        }
    }

    /// What is being followed, as the seat sees it: a spell or ability on the stack, or one
    /// followed without the stack, a mana ability (605.3) or an opening-hand action (103.6),
    /// whose source is the object that has the ability.
    fn followed(&self, seat: PlayerId, r: &crate::state::Resolving) -> Occurrence {
        use crate::state::Completion;
        let kind = match r.completion {
            Completion::Stack => return self.occurrence(seat, r.object),
            Completion::Opening => OccurrenceKind::Opening,
            Completion::Mana { triggered: true, .. } => OccurrenceKind::Triggered,
            Completion::Mana { .. } => OccurrenceKind::Activated,
        };
        let key = r.execution.as_ref().and_then(|e| e.ability()).or_else(|| match kind {
            OccurrenceKind::Opening => self.def(r.object).and_then(|d| d.opening.first().copied())
                .map(|index| AbilityKey { origin: Origin::Own, index }),
            _ => None,
        });
        let source = Some(r.object).filter(|s| self.perceives(seat, *s));
        Occurrence {
            kind, object: None, source,
            source_name: source.map(|s| self.obj(s).chars.name.clone()).unwrap_or_default(),
            source_left: false,
            controller: self.controller(r.object),
            text: key.clone().and_then(|key| ability_text(self, Some(r.object), key)),
            created_by: None,
            granted: key.is_some_and(|k| matches!(k.origin, Origin::Given { .. } | Origin::Inherited(_))),
            modes: Vec::new(),
        }
    }

    /// A triggered ability that waits to be put on the stack, as its controller sees it.
    fn trigger(&self, seat: PlayerId, t: &Triggered) -> Occurrence {
        let key = AbilityKey { origin: t.origin.clone(), index: t.index };
        let delayed = t.delayed.is_some();
        let created = if delayed { t.context().ability } else { None };
        let captured = t.captured.as_ref().and_then(|c| c.as_ref().ok());
        let source = t.source.filter(|s| self.perceives(seat, *s));
        Occurrence {
            kind: if delayed { OccurrenceKind::Delayed } else { OccurrenceKind::Triggered },
            object: None,
            source,
            source_name: source.map(|s| self.obj(s).chars.name.clone()).unwrap_or_default(),
            source_left: t.source.is_some_and(|s| self.obj(s).zone == crate::Zone::Gone),
            controller: t.controller,
            // a delayed or reflexive one's own sentences (603.3, 603.7, 603.12)
            text: match captured.map(|c| c.program()) {
                Some(program) if delayed => crate::program::keyword_delayed_text(program).map(str::to_string)
                    .or_else(|| crate::program::delayed_text(program).map(|said| named(&said, t.source.map_or("", |s| self.obj(s).chars.name.as_str()), &[]))),
                _ if delayed => None,
                _ => ability_text(self, t.source, key),
            },
            // the ability that created a delayed one (603.7): of a keyword's rule, that keyword
            // (702.88a, 702.185a), declared by its definition or given by an effect (613.1f);
            // otherwise found by the instructions it was made with, not equal ones of another
            // ability, and never the line its source has now at that place (113.7a)
            created_by: {
                let name = t.source.map_or("", |s| self.obj(s).chars.name.as_str());
                let line = |(def, index): (&crate::CardDef, usize)| {
                    let a = def.abilities.iter().chain(def.keyword_abilities.iter().map(|(_, a)| a)).nth(index)?;
                    Some(with_modes(print(a, name, def.forms(index))?, def.modes_of(index), name, def.mode_forms(index)))
                };
                match (captured.filter(|_| delayed), t.source) {
                    (Some(c), _) if crate::program::keyword_delayed_text(c.program()).is_some() => match c.keyword_producer.as_deref() {
                        Some((def, key)) => line((def.def(), key.index)),
                        None => c.granted_keyword.as_deref().and_then(|k| granted_keyword_text(k, name)),
                    },
                    (c, s) => match c.zip(s).and_then(|(c, s)| self.retained(s, c.program())) {
                        Some(found) => line(found),
                        // a rule's or an effect's ability, or a copy effect's by its timestamp:
                        // the key names that definition, whatever the source is now
                        None => created.filter(|key| !matches!(key.origin, Origin::Own)).and_then(|key| ability_text(self, t.source, key)),
                    },
                }
            },
            granted: matches!(t.origin, Origin::Given { .. } | Origin::Inherited(_)),
            modes: Vec::new(),
        }
    }

    /// Why this question is asked, and of which spell or ability.
    fn context(&self, seat: PlayerId, question: &Asked) -> (Reason, Option<Occurrence>, Vec<Occurrence>) {
        let resolving = || self.resolving().map(|r| self.followed(seat, r));
        match (&self.decision, question) {
            (_, Asked::Priority) => (Reason::Priority, None, Vec::new()),
            (Some(Decision::Mulligan { .. } | Decision::Bottom { .. }), _) => (Reason::Mulligan, None, Vec::new()),
            (Some(Decision::Discard { .. }), _) => (Reason::Cleanup, None, Vec::new()),
            (Some(Decision::Attackers { .. } | Decision::Blockers { .. } | Decision::Divide { .. }), _) => (Reason::Combat, None, Vec::new()),
            (Some(Decision::Targets { ability, .. } | Decision::Modes { ability, .. }), _) => (Reason::Trigger, Some(self.trigger(seat, ability)), Vec::new()),
            (Some(Decision::Order { abilities, .. }), _) => (Reason::TriggerOrder, None, abilities.iter().map(|t| self.trigger(seat, t)).collect()),
            (Some(Decision::Payment { source, .. }), _) => (Reason::Announcement, Some(self.occurrence(seat, *source)), Vec::new()),
            (_, Asked::Replacement { .. } | Asked::DamageOrder { .. } | Asked::DamageReplacement { .. } | Asked::Prevention { .. }
                | Asked::SpellDestination { .. } | Asked::Skip { .. } | Asked::CommandZone { .. }) => (Reason::Replacement, resolving(), Vec::new()),
            (Some(Decision::Choose { .. }), _) if self.entering_card().is_some() => (Reason::Replacement, resolving(), Vec::new()),
            (Some(Decision::Choose { .. }), _) => match resolving() {
                Some(occurrence) => (Reason::Resolution, Some(occurrence), Vec::new()),
                None => (Reason::Rules, None, Vec::new()),
            },
            (None, _) => (Reason::Rules, None, Vec::new()),
        }
    }
}

impl Simulation {
    /// The prompt for this seat. It searches for no move. Within the failure boundary, like
    /// every other read of the game.
    pub fn prompt(&mut self, seat: PlayerId) -> Result<Prompt, Fault> {
        let request = self.request()?.cloned();
        let view = self.snapshot(Some(seat))?;
        let objects: Vec<ObjId> = view.objects.iter().map(|o| o.id).collect();
        let (prompt, hidden) = self.observe(|g| {
            let entities = objects.iter().map(|id| (*id, g.entity(seat, *id))).collect();
            // what is asked is shown to the seat that answers it and to the player whose decision
            // it is (723.5); another seat is told only whom the game waits for
            let decision = request.as_ref().filter(|r| r.player == seat || r.acting == seat).map(|r| {
                let (reason, occurrence, ordered) = g.context(seat, &r.question);
                PlayerDecision { id: r.id, chooser: r.player, acting: r.acting, question: r.question.clone(), reason, occurrence, ordered,
                    entering: g.entering_card().filter(|c| g.perceives(seat, *c)), optional: g.optional() }
            });
            // a card of a hidden zone the seat may see is in a place it sees it in: a library from
            // the top (a revealed top card first, 401.2), a hand in its order
            let hidden: Vec<ObjId> = g.players.iter().flat_map(|p| p.library.iter().rev().chain(&p.hand)).copied().collect();
            (Prompt { api: API, seat, request: request.as_ref().map(|r| r.id), waiting_for: request.as_ref().map(|r| r.player), view, entities, decision }, hidden)
        })?;
        // name what is newly shown in an order the seat can see, never in the order objects are
        // stored: the stack from the bottom, the battlefield, each player's cards, what only
        // the question shows, then the hidden zones' cards it sees, where they are
        let shown = &prompt.view;
        let mut order: Vec<ObjId> = shown.stack.iter().chain(&shown.battlefield).copied().collect();
        for p in &shown.players { order.extend(p.hand.iter().flatten().chain(&p.graveyard).chain(&p.top_card)); }
        order.extend(shown.exile.iter().chain(&shown.command));
        if let Some(d) = &prompt.decision { order.extend(question_objects(&d.question)); }
        // a revealed card (`publicly_revealed`, kept in id order) is one of these too
        order.extend(hidden.into_iter().filter(|id| prompt.entities.contains_key(id)));
        debug_assert!(prompt.entities.keys().all(|id| order.contains(id)), "every object shown has a place it is seen in");
        order.extend(prompt.entities.keys());
        let labels = self.labels_mut(seat);
        for id in order { if prompt.entities.contains_key(&id) { labels.name(id); } }
        Ok(prompt)
    }

    /// The names this seat has been given for objects.
    pub fn labels(&self, seat: PlayerId) -> Option<&Labels> { self.labels.get(seat) }

    /// The names this seat has been given, to name more by: for a host that says more to the
    /// seat than a [`Prompt`] does (what happened since it was last prompted).
    pub fn labels_mut(&mut self, seat: PlayerId) -> &mut Labels {
        if self.labels.len() <= seat { self.labels.resize_with(seat + 1, Labels::default); }
        &mut self.labels[seat]
    }

    /// Answer as a seat: as [`Simulation::respond`], after every object the answer names is
    /// checked to be one this seat may refer to now, so that an answer cannot find out about
    /// what it may not see. Such an answer is refused and changes nothing. A player it names
    /// that the game does not have is refused by the game, as in any answer ([`crate::Game::attempt`]).
    pub fn answer(&mut self, seat: PlayerId, id: RequestId, action: Move) -> Result<(), ResponseError> {
        let question = match self.request().map_err(ResponseError::Fault)? {
            Some(r) if r.id == id && r.player == seat => r.question.clone(),
            // stale, another seat's, or the game is over: refused as any such answer is
            _ => return self.respond(id, seat, action),
        };
        let mut named = Vec::new();
        move_objects(&action, &mut named);
        let offered = question_objects(&question);
        let refused = self.observe(|g| named.into_iter().find(|o| {
            !offered.contains(o) && (g.get_object(*o).is_none() || !g.perceives(seat, *o))
        })).map_err(ResponseError::Fault)?;
        if let Some(object) = refused { return Err(ResponseError::Unreferable(crate::Target::Object(object))); }
        self.respond(id, seat, action)
    }
}

/// The objects a question offers its chooser, wherever they are: those of a hidden zone it
/// lets them choose among without showing them to anyone else.
fn question_objects(question: &Asked) -> Vec<ObjId> {
    match question {
        Asked::Search { cards, among, .. } => cards.iter().chain(among).copied().collect(),
        Asked::ConstrainedObjects { among, .. } | Asked::Select { among, .. } | Asked::Objects { among, .. } | Asked::OptionalObjects { among, .. } | Asked::Discard { among, .. } | Asked::Bottom { among, .. } => among.clone(),
        Asked::OrderCards { cards, .. } | Asked::LibraryDestination { cards, .. } | Asked::Arrange { cards, .. } | Asked::Cast { cards, .. } => cards.clone(),
        Asked::Pile { piles } => piles.iter().flatten().copied().collect(),
        Asked::Alternative { among, .. } => among.iter().flat_map(|o| match &o.body {
            crate::state::ActionOptionBody::Objects { among, .. } => among.clone(),
            crate::state::ActionOptionBody::Create { .. } => Vec::new(),
            crate::state::ActionOptionBody::Counters { on, .. } => vec![*on],
        }).collect(),
        _ => Vec::new(),
    }
}

fn targets(groups: &[Vec<Target>], out: &mut Vec<ObjId>) {
    out.extend(groups.iter().flatten().filter_map(|t| match t { Target::Object(o) => Some(*o), Target::Player(_) => None }));
}

fn steps(plan: &[crate::legal::Step], out: &mut Vec<ObjId>) {
    for step in plan { out.push(step.source); out.extend(step.paying.iter().flatten()); }
}

/// Every object an answer names. Replacement, damage and skip identities are matched against
/// what was offered as they are read, so they name only what the question did.
fn move_objects(m: &Move, out: &mut Vec<ObjId>) {
    match m {
        Move::Pass | Move::Concede | Move::Companion | Move::ReversePayment | Move::Mulligan | Move::Keep | Move::ChooseModes(_) | Move::Order(_) => {}
        Move::Plot(o) | Move::Foretell(o) | Move::TurnFaceUp(o, _) | Move::Unlock { room: o, .. } | Move::PlayLand(o) | Move::PlayLandAs { card: o, .. } => out.push(*o),
        Move::FinishPayment { plan, paying, .. } => { steps(plan, out); out.extend(paying.iter().flatten()); }
        Move::Activate { source, targets: groups, paying, .. } => { out.push(*source); targets(groups, out); out.extend(paying.iter().flatten()); }
        Move::Cast { card, targets: groups } => { out.push(*card); targets(groups, out); }
        Move::ChooseTargets(groups) => targets(groups, out),
        Move::Discard(cards) | Move::Bottom(cards) => out.extend(cards),
        Move::Attack(attacks) => for (attacker, at) in attacks { out.push(*attacker); targets(std::slice::from_ref(&vec![*at]), out); },
        Move::Block(blocks) => for (blocker, attacker) in blocks { out.push(*blocker); out.push(*attacker); },
        Move::Divide(division) => for (to, _) in division { targets(std::slice::from_ref(&vec![*to]), out); },
        Move::ContinueManaPayment { source, .. } => out.push(*source),
        Move::AuditPayment(inner) => move_objects(inner, out),
        Move::Choose(answer) => match answer {
            Answer::SpellDestination { on, .. } => out.push(*on),
            Answer::Targets(groups) => targets(groups, out),
            Answer::Attach(to) | Answer::Defender(to) => targets(std::slice::from_ref(&vec![*to]), out),
            Answer::Holders(holders) => targets(std::slice::from_ref(holders), out),
            Answer::Objects(objects) => out.extend(objects),
            Answer::OptionalObjects(objects) => out.extend(objects.iter().flatten()),
            Answer::Arrange { top, away } => out.extend(top.iter().chain(away)),
            Answer::Pay { plan, .. } => steps(plan, out),
            Answer::LibraryPosition(_) | Answer::Yes | Answer::No | Answer::Alternative(_) | Answer::Mana(_) | Answer::Value(_) | Answer::Player(_) | Answer::Pile(_)
                | Answer::Replacement(_) | Answer::DamageReplacement(_) | Answer::DamageOrder(_) | Answer::Prevention(_) | Answer::Skip(_) => {}
        },
        Move::WithMana(plan, inner) => { steps(plan, out); move_objects(inner, out); }
        Move::WithCosts(paying, inner) => { out.extend(paying.iter().flatten()); move_objects(inner, out); }
        Move::CastVia(route, inner) => {
            if let crate::announcement::CastRoute::Permission { source, .. } = route { out.push(*source); }
            move_objects(inner, out);
        }
        Move::Paying(_, inner) | Move::WithX(_, inner) | Move::WithKicks(_, inner) | Move::WithBuyback(_, inner) | Move::WithOptional(_, inner) | Move::WithEntwine(_, inner)
            | Move::WithReplicate(_, inner) | Move::CastFor(_, inner) | Move::WithDelve(_, inner) | Move::WithConvoke(_, inner)
            | Move::GiftTo(_, inner) | Move::WithLife(_, inner) | Move::WithCostChoice(_, inner) | Move::WithCostBranches(_, inner) | Move::Adding(_, inner) | Move::Modes(_, inner) | Move::CastAs(_, inner) | Move::WithDivision(_, inner) => move_objects(inner, out),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_round_trip_and_refuse_what_is_no_label() {
        for label in [Label::Player(1), Label::Object(42), Label::Ability(42, 3)] {
            assert_eq!(Label::parse(&label.to_string()), Some(label));
        }
        for text in ["", "o", "p", "42", "o-1", "o4x", "o42.a", "o42.b3", "q1", "o 4"] {
            assert_eq!(Label::parse(text), None, "{text}");
        }
    }

    #[test]
    fn labels_number_objects_in_the_order_shown_and_keep_their_names() {
        let mut labels = Labels::default();
        assert_eq!((labels.name(ObjId(9 << 32 | 4)), labels.name(ObjId(2)), labels.name(ObjId(9 << 32 | 4))), (1, 2, 1));
        assert_eq!((labels.object(1), labels.object(2), labels.object(0), labels.object(3)), (Some(ObjId(9 << 32 | 4)), Some(ObjId(2)), None, None));
        assert_eq!(labels.number(ObjId(3)), None);
    }

    /// A quoted ability a card grants names what will have it, not the card (201.5a): Basking
    /// Broodscale's Spawn has "Sacrifice this token: Add {C}.", which the Spawn itself says as
    /// "Sacrifice Eldrazi Spawn Token: …".
    #[test]
    fn a_quoted_ability_names_its_holder_and_the_granter_by_name() {
        let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
        let def = |n: &str| crate::CardDef::from_face(faces.iter().find(|f| f.name == n).unwrap()).unwrap();
        let broodscale = printed(&def("Basking Broodscale"));
        assert!(broodscale.iter().any(|l| l.contains(r#""Sacrifice this token: Add {C}.""#)), "{broodscale:?}");
        assert!(!broodscale.iter().any(|l| l.contains("Sacrifice Basking Broodscale")), "{broodscale:?}");
        assert!(broodscale.iter().any(|l| l.contains("on Basking Broodscale")), "its own name outside the quote: {broodscale:?}");
        assert_eq!(super::named("~ gets +1/+1.", "", &[]), "this gets +1/+1.");
        assert_eq!(super::named("Create a token with \"~ gets +1/+1. Sacrifice ~: Draw.\"", "Saga", &[]),
            "Create a token with \"This token gets +1/+1. Sacrifice this token: Draw.\"");
        assert_eq!(super::named("Equipped creature has \"{T}: Return § to its owner's hand.\"", "Boomerang", &[]),
            "Equipped creature has \"{T}: Return Boomerang to its owner's hand.\"");
    }

    /// The quoted self-reference prints the noun the Oracle text uses for what
    /// will have the ability (201.5a), which the last-noun guess misses for a capitalized
    /// grantee, an emblem, a card, a copy's description, the ability's own object and a
    /// quote inside a quote. 103 of 768 quoted self-references in the corpus differ from the
    /// Oracle text only in that noun.
    #[test]
    fn a_quoted_ability_names_its_holder_as_the_oracle_text_does() {
        let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
        let lines = |n: &str| printed(&crate::CardDef::from_face(faces.iter().find(|f| f.name == n).unwrap()).unwrap()).join("\n");
        for (card, oracle) in [
            ("Phenax, God of Deception", "where X is this creature's toughness"),
            ("Case of the Uneaten Feast", "\"You may cast this card from your graveyard\""),
            ("Karn, Living Legacy", "\"Tap an untapped artifact you control: This emblem deals 1 damage"),
            ("Kethis, the Hidden Hand", "\"You may play this card from your graveyard.\""),
            ("Electroduplicate", "\"At the beginning of the end step, sacrifice this token.\""),
            ("Dance of the Dead", "\"enchant creature put onto the battlefield with this Aura.\""),
            ("Old-Growth Troll", "Sacrifice this land: Create a tapped 4/4"),
            ("Freewind Equenaut", "\"{T}: This creature deals 2 damage"),
            // a quote granted to another object names it, not the granting card
            ("Caduceus, Staff of Hermes", "\"Prevent all damage that would be dealt to this creature.\""),
            // a mode whose flavor word (207.2d) is also a keyword action is printed without it,
            // as any other flavor word is
            ("Secret Identity", "\n• Until end of turn, target creature you control becomes a Hero"),
            // a mode row's
            ("Choreographed Sparks", "\"At the beginning of the end step, sacrifice this token.\""),
        ] {
            let printed = lines(card);
            assert!(printed.contains(oracle), "{card}: {printed}");
        }
    }

    /// A keyword an effect gives an object is part of its text as a seat reads it, whether a
    /// resolved spell or a static ability gives it (613.1f); one it has already is not said twice.
    #[test]
    fn r613_1f_a_keyword_an_effect_gives_is_in_the_objects_text() {
        use crate::{CardDef, Game, Move, Target, Zone};
        let defs = vec![
            CardDef::new("Bear", "{0}", "Creature — Bear", "Flying", Some((2, 2))).unwrap(),
            CardDef::new("Strike", "{0}", "Instant", "Target creature gains double strike and flying until end of turn.", None).unwrap(),
            CardDef::new("Banner", "{0}", "Enchantment", "Creatures you control have haste.", None).unwrap(),
            CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        ];
        let mut g = Game::new(defs, 2, 20);
        for p in 0..2 { for _ in 0..5 { g.put(p, 3, Zone::Library); } }
        while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
        let bear = g.put(0, 0, Zone::Battlefield);
        assert_eq!(g.entity(1, bear).text, ["Flying"]);
        let strike = g.put(0, 1, Zone::Hand);
        g.act(0, Move::Cast { card: strike, targets: vec![vec![Target::Object(bear)]] }).unwrap();
        while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
        g.put(0, 2, Zone::Battlefield);
        let text = g.entity(1, bear).text;
        assert_eq!(text.iter().filter(|l| l.contains("Flying")).count(), 1, "{text:?}");
        assert!(text.iter().any(|l| l == "Double strike") && text.iter().any(|l| l == "Haste"), "{text:?}");
    }
}
