//! Static abilities kept lowered (604.1): what a statement is said of, when it holds, where
//! its ability functions (113.6) and what it states, compiled once when the definition is
//! loaded.
//!
//! [`restrictions`] is the one place a static restriction or requirement is refused, and
//! [`characteristics`] the one place a statement of what its subject is; the refusal is
//! kept beside what compiled, so admission reads it and no query lowers text.
//! What is kept is a template: it names no object. Whoever asks supplies the permanent whose
//! ability it is, and the subject and condition are asked of the game as it then is — a
//! static ability's effect is never locked in (611.3a).
//!
//! A keyword ability whose rule is a sentence is the static ability the sentence is
//! ([`crate::keyword::meaning`]); [`keyword`] keeps each lowered once for every game.

use crate::card::CardDef;
use crate::event::Target;
use crate::functions::Scope;
use crate::layers::Layer;
use crate::program::change::Change;
use crate::program::condition::Condition;
use crate::program::expr::{self, Bind, NoLocals, linked};
use crate::restrict::Rule;
use crate::select::{Ctx, Support, name};
use crate::state::Game;
use crate::{Res, unsupported};
use ir::{Ability, Action, Keyword, Quant, Ref};
use std::sync::OnceLock;

/// What a static ability's statement is said of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Subject {
    /// "You": the controller of the static ability.
    Controller,
    /// The permanent whose ability it is ("This creature can't block").
    Source,
    /// Whatever this Aura or Equipment is attached to, now (303.4m, 301.5f).
    Attached,
    /// This permanent and the creature it is paired with, while it is paired (702.95b):
    /// "both creatures".
    Paired,
    /// Whatever fits the description at any moment ("Creatures you control …").
    Described(expr::Set),
    /// Any source object, including a card whose ability functions off the battlefield.
    SourceFilter(expr::Filter),
}

impl Subject {
    pub(crate) fn compile(subject: &ir::Subject, support: &Support<'_>, bind: Bind<'_>) -> Res<Subject> {
        use ir::selector::Attachment;
        match subject {
            ir::Subject::Selector(s) if *s == ir::Selector::you() => Ok(Subject::Controller),
            ir::Subject::Selector(s) if s.quant == Quant::Definite(Ref::This) => Ok(Subject::Source),
            ir::Subject::Selector(s) if matches!(s.quant, Quant::Definite(Ref::AttachedTo(Attachment::Enchanted | Attachment::Equipped))) => Ok(Subject::Attached),
            ir::Subject::Selector(s) if s.quant == Quant::Definite(Ref::Anaphor(ir::Anaphor::BothPaired)) => Ok(Subject::Paired),
            ir::Subject::Selector(s) if matches!(s.quant, Quant::Each | Quant::All) && s.chooser.is_none() && s.random.is_none() => {
                expr::Set::compile(&s.set, support, bind).map(Subject::Described)
            }
            ir::Subject::Set(set) => expr::Set::compile(set, support, bind).map(Subject::Described),
            other => unsupported(format!("subject of a static ability: {}", name(other))),
        }
    }

    /// What deciding whom it is said of can read ([`expr::Reads`]). The source, its
    /// controller and what it is attached to are identities, in no bit of it.
    pub(crate) fn reads(&self) -> expr::Reads {
        match self {
            Subject::Controller | Subject::Source | Subject::Attached | Subject::Paired => expr::Reads::NONE,
            Subject::Described(set) => set.reads(),
            Subject::SourceFilter(filter) => filter.reads(),
        }
    }

    /// Is this object or player one the statement is said of, now?
    pub(crate) fn contains(&self, game: &Game, ctx: &Ctx, t: Target) -> bool {
        match self {
            Subject::Controller => t == Target::Player(ctx.controller),
            Subject::Source => Some(t) == ctx.source.map(Target::Object),
            Subject::Attached => ctx.source.and_then(|s| game.attached_to(s)) == Some(t),
            Subject::Paired => ctx.source.and_then(|s| Some((s, game.partner(s)?)))
                .is_some_and(|(s, partner)| t == Target::Object(s) || t == Target::Object(partner)),
            Subject::Described(set) => set.contains(game, ctx, &NoLocals, t),
            Subject::SourceFilter(filter) => matches!(t, Target::Object(_)) && filter.matches(game, ctx, &NoLocals, t),
        }
    }
}

/// When a statement holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum When {
    Always,
    All(Vec<When>),
    /// "as long as you control an Island"
    AsLongAs(Condition),
    /// "unless you control a Forest"
    Unless(Condition),
}

impl When {
    pub(crate) fn compile(when: &crate::statement::When<'_>, support: &Support<'_>, bind: Bind<'_>) -> Res<When> {
        Ok(match when {
            crate::statement::When::Always => When::Always,
            crate::statement::When::All(conditions) => When::All(conditions.iter().map(|when|Self::compile(when,support,&mut *bind)).collect::<Res<_>>()?),
            crate::statement::When::During(d) => {
                let (whose, negated) = match d {
                    ir::event::During::Turn(who) => (who, false),
                    ir::event::During::NotTurn(who) => (who, true),
                    _ => return unsupported("static condition: a window other than a player's turn"),
                };
                When::AsLongAs(Condition::compile(&ir::Condition::Turn { whose: whose.clone(), negated }, support, bind)?)
            },
            crate::statement::When::AsLongAs(c) => When::AsLongAs(Condition::compile(c, support, bind)?),
            crate::statement::When::Unless(c) => When::Unless(Condition::compile(c, support, bind)?),
        })
    }

    fn inputs(&self) -> super::footprint::Inputs {
        use super::footprint::Inputs as I;
        match self { Self::Always => I::NONE, Self::All(v) => v.iter().fold(I::NONE,|a,b|a | b.inputs()), Self::AsLongAs(c) | Self::Unless(c) => c.inputs() }
    }

    pub(crate) fn reads(&self) -> expr::Reads {
        match self {
            When::Always => expr::Reads::NONE,
            When::All(items)=>items.iter().fold(expr::Reads::NONE,|all,w|all | w.reads()),
            When::AsLongAs(c) | When::Unless(c) => c.reads(),
        }
    }

    pub(crate) fn reads_targets(&self) -> bool {
        match self { Self::Always => false, Self::All(v) => v.iter().any(Self::reads_targets), Self::AsLongAs(c) | Self::Unless(c) => c.reads_targets() }
    }

    /// Does the statement hold now?
    pub(crate) fn holds(&self, game: &Game, ctx: &Ctx) -> bool {
        match self {
            When::Always => true,
            When::All(items)=>items.iter().all(|when|when.holds(game,ctx)),
            When::AsLongAs(c) => c.evaluate(game, ctx, &NoLocals),
            When::Unless(c) => !c.evaluate(game, ctx, &NoLocals),
        }
    }
}

// Player characteristic statements need a real player consumer, not the object layer
// world. One owner handles the whole row, including mixed or unsupported grants.
fn player_changes(a: &Ability) -> Option<crate::statement::Statement<'_>> {
    fn players(subject: &ir::Subject) -> bool {
        match subject {
            ir::Subject::Selector(s) | ir::Subject::Each(s) => s.set.possibilities().iter().any(|p| p.domain.sort == ir::Sort::Player),
            ir::Subject::Set(set) => set.possibilities().iter().any(|p| p.domain.sort == ir::Sort::Player),
            ir::Subject::And { items, .. } => items.iter().any(players),
            ir::Subject::LifeTotal(_) => true,
        }
    }
    crate::statement::read(a).filter(|s| players(&s.subject)
        && s.acts.iter().any(|(action, _, _)| crate::layers::changes(action)))
}

/// A static ability that is a restriction or a requirement, lowered: where it functions,
/// what it is said of, and each rule it states with when that holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Restriction {
    pub(crate) scope: Scope,
    pub(crate) subject: Subject,
    pub(crate) acts: Vec<(When, Rule)>,
}

impl Restriction {
    /// What deciding whom it restricts and when can read ([`expr::Reads`]). What each rule
    /// itself describes (which attackers, which spells) is the rule's to say and is not
    /// looked into here.
    #[allow(dead_code)]
    pub(crate) fn applicability_reads(&self) -> expr::Reads {
        self.acts.iter().fold(self.subject.reads(), |all, (when, _)| all | when.reads())
    }

    /// The ability lowered, when it is a restriction or a requirement; None when it is not
    /// one. What the text refers back to is read through the definition's links (607.2d).
    fn compile(a: &Ability, modes: &[ir::Mode], support: &Support<'_>) -> Option<Res<Restriction>> {
        if let Some(statement) = player_changes(a) {
            return Some((|| {
                if !matches!(&statement.subject, ir::Subject::Selector(s) if *s == ir::Selector::you()) {
                    return unsupported("player characteristic: only static you-have hexproof/shroud is implemented");
                }
                let mut acts = Vec::new();
                for (action, when, scale) in &statement.acts {
                    if *scale != crate::statement::Scale::Once
                        || !matches!(action, Action::Static(s) if matches!(**s, ir::StaticAct::Has(_)))
                    {
                        return unsupported("player characteristic: only unscaled static keyword grants are implemented");
                    }
                    acts.push((When::compile(when, support, &mut linked(support))?,
                        Rule::compile(action, support, &mut linked(support))?));
                }
                Ok(Restriction { scope: crate::functions::scope(a, modes)?, subject: Subject::Controller, acts })
            })());
        }
        if let Ability::Text(ir::Effect(rows)) = a
            && let [ir::Sentence::Static(stated)] = rows.as_slice()
            && let ir::statics::Static::ActivatedAbilities { of, rule: rule @ ir::statics::AbilityRule::CantBeActivated { .. }, this_turn } = stated.as_ref()
        {
            return Some((|| {
                if matches!(of, ir::statics::AbilitiesOf::Matching(_)) {
                    let (rule, temporary) = Rule::activation_family(stated)?;
                    if temporary { return unsupported("static activation family prohibition with duration"); }
                    return Ok(Restriction { scope: crate::functions::scope(a,modes)?, subject: Subject::Controller, acts: vec![(When::Always,rule)] });
                }
                let ir::statics::AbilitiesOf::Of { kind: ir::action::AbilityKind::Activated, of } = of else {
                    return unsupported("activation prohibition: unsupported ability group");
                };
                let ir::statics::AbilityRule::CantBeActivated { unless_mana } = rule else {
                    return unsupported("activation cost modification");
                };
                if *this_turn { return unsupported("static activation prohibition with duration"); }
                let [ir::statics::Who::Set(set)] = of.items.as_slice() else {
                    return unsupported("activation prohibition: multiple or relative source groups");
                };
                let subject = if set.domain == ir::Domain::Source {
                    if set.controller.is_some() || set.owner.is_some() || set.caster.is_some() || set.zone.is_some() || set.other {
                        return unsupported("activation prohibition: qualified source relation");
                    }
                    Subject::SourceFilter(expr::Filter::compile(&set.filter, support, &mut linked(support))?)
                } else {
                    Subject::Described(expr::Set::compile(set, support, &mut linked(support))?)
                };
                Ok(Restriction {
                    scope: crate::functions::scope(a, modes)?, subject,
                    acts: vec![(When::Always, Rule::cant_activate(*unless_mana))],
                })
            })());
        }
        if let Ability::Text(ir::Effect(rows)) = a
            && let [ir::Sentence::Static(stated)] = rows.as_slice()
            && let ir::statics::Static::CantCause { sources, player, events } = stated.as_ref()
        {
            return Some((|| {
                let [event] = events.as_slice() else { return unsupported("causal prohibition: several events"); };
                let subject = Subject::compile(&ir::Subject::Selector(player.clone()), support, &mut linked(support))?;
                let rule = Rule::cant_cause(sources, event, support, &mut linked(support))?;
                Ok(Restriction { scope: crate::functions::scope(a, modes)?, subject, acts: vec![(When::Always, rule)] })
            })());
        }
        if let Ability::Text(ir::Effect(rows)) = a
            && let [ir::Sentence::Static(stated)] = rows.as_slice()
            && let ir::statics::Static::Unpreventable { damage, this_turn: false, redirect } = stated.as_ref()
        {
            return Some((|| Ok(Restriction {
                scope: crate::functions::scope(a, modes)?, subject: Subject::Source,
                acts: vec![(When::Always, Rule::unpreventable(damage, *redirect)?)],
            }))());
        }
        // "Prevent all damage that would be dealt to this creature." (615.1)
        if let Ability::Text(ir::Effect(rows)) = a
            && let [ir::Sentence::Do(body)] = rows.as_slice()
            && let ir::Clause { subject: None, may: false, also: false, first, rest } = &body.first
            && rest.is_empty() && body.rest.is_empty() && !first.may && first.mods.is_empty()
            && let Action::Prevent(prevention) = &first.action
        {
            return Some((|| Ok(Restriction {
                scope: crate::functions::scope(a, modes)?, subject: Subject::Source,
                acts: vec![(When::Always, Rule::prevent(prevention, support, &mut linked(support))?)],
            }))());
        }
        if let Some(payment) = Rule::attack_payment(a, support, &mut linked(support)) {
            return Some((|| {
                let (subject, rule) = payment?;
                Ok(Restriction { scope: crate::functions::scope(a, modes)?,
                    subject: Subject::compile(&subject, support, &mut linked(support))?,
                    acts: vec![(When::Always, rule)] })
            })());
        }
        let statement = crate::restrict::statement(a)?;
        Some((|| {
            let scope = crate::functions::scope(a, modes)?;
            let cast = crate::restrict::spells_cant_be_cast(&statement);
            let subject = Subject::compile(cast.as_ref().map_or(&statement.subject, |(s, _)| s), support, &mut linked(support))?;
            let mut acts = Vec::new();
            for (action, when, _) in &statement.acts {
                let action = cast.as_ref().map_or(*action, |(_, a)| a);
                let rule = Rule::compile(action, support, &mut linked(support))?;
                rule.validate_entry_subject(Some(&subject))?;
                // "its controller's next untap step" is how long a resolved effect lasts, not
                // something a static ability says
                if matches!(action, Action::DoesntUntap { next: true, .. }) {
                    return unsupported("a static ability about a next untap step");
                }
                // "This creature can't block as long as it has a +1/+1 counter on it": the
                // pronoun is the statement's explicit subject
                let mut linked = linked(support);
                let mut bind = expr::SourceBinding { parent: &mut linked, source: subject == Subject::Source };
                acts.push((When::compile(when, support, &mut bind)?, rule));
            }
            Ok(Restriction { scope, subject, acts })
        })())
    }
}

/// How many times over a statement says what it says, counted each time it is asked: a
/// static ability's number is not locked in (611.3a).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Scale {
    Once,
    /// "gets +1/+1 for each other creature you control"
    ForEach(expr::Amount),
    /// "gets +X/+X, where X is the number of cards in your hand" (107.3c)
    WhereX(expr::Amount),
}

impl Scale {
    /// The scale, and whether what it scales has an X to read. A number that is a power or
    /// toughness depends on what else changes power and toughness in the same layer (613.8),
    /// which the engine does not order yet.
    fn compile(scale: &crate::statement::Scale<'_>, support: &Support<'_>, bind: Bind<'_>) -> Res<(Scale, bool)> {
        use crate::statement::Scale as Said;
        if let Said::ForEach(n) | Said::WhereX(n) = scale
            && crate::layers::asks_pt(n)
        {
            return unsupported("a static ability whose number is counted from power or toughness");
        }
        Ok(match scale {
            Said::Once => (Scale::Once, false),
            Said::ForEach(n) => (Scale::ForEach(expr::Amount::compile(n, support, bind)?), false),
            Said::WhereX(x) => (Scale::WhereX(expr::Amount::compile(x, support, bind)?), true),
        })
    }

    /// The change as many times over as it says, with its numbers as they are now.
    fn apply(&self, change: &Change, game: &Game, ctx: &Ctx) -> crate::layers::Change {
        match self {
            Scale::Once => change.evaluate(game, ctx, &NoLocals),
            Scale::ForEach(n) => change.evaluate(game, ctx, &NoLocals).times(n.evaluate(game, ctx, &NoLocals)),
            Scale::WhereX(x) => change.evaluate(game, &Ctx { x: Some(x.evaluate(game, ctx, &NoLocals)), ..ctx.clone() }, &NoLocals),
        }
    }
}

/// A static ability that says what its subject is ("Creatures you control get +1/+1."),
/// lowered: what it is said of, and each thing it changes with when that holds and how many
/// times over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Characteristic {
    pub(crate) subject: Subject,
    acts: Vec<(When, Scale, Vec<Change>)>,
    footprints: Vec<(usize, Layer, super::footprint::EffectFootprint)>,
}

/// "As long as ~ is paired with another creature, each of those creatures gets +1/+1": the
/// creatures are the two the condition names, this one and the one it is paired with
/// (702.95b), as "both creatures" says.
fn those_paired(statement: &crate::statement::Statement<'_>) -> bool {
    let ir::Subject::Selector(s) = &statement.subject else { return false };
    let ir::Filter::All(parts) = &s.set.filter else { return false };
    let [ir::Filter::Atom(ir::Atom::Among(those))] = parts.as_slice() else { return false };
    let paired = |when: &crate::statement::When<'_>| matches!(when, crate::statement::When::AsLongAs(ir::Condition::Is {
        what, pred: ir::Predicate::PairedWith(_), negated: false }) if what.quant == Quant::Definite(Ref::This));
    s.quant == Quant::Each && s.set.domain == ir::Domain::Object && !s.set.other && s.chooser.is_none() && s.random.is_none()
        && those.quant == Quant::Definite(Ref::Anaphor(ir::Anaphor::Those))
        && !statement.acts.is_empty() && statement.acts.iter().all(|(_, when, _)| paired(when))
}

/// The unconditional printed definition of this object's own P/T (604.3a). A conditional
/// or an effect setting another object's stats is not a characteristic-defining ability.
pub(crate) fn defining_pt(a: &Ability) -> Option<(&Option<ir::Amount>, &Option<ir::Amount>)> {
    let Ability::Text(ir::Effect(rows)) = a else { return None };
    let [ir::Sentence::Static(s)] = rows.as_slice() else { return None };
    let ir::statics::Static::Pt { what, power, toughness } = s.as_ref() else { return None };
    (what.quant == Quant::Definite(Ref::This)).then_some((power,toughness))
}

impl Characteristic {
    fn compile(a: &Ability, support: &Support<'_>) -> Option<Res<Characteristic>> {
        Self::compile_body(a, support).map(|result| result.map(|mut out| { out.compile_footprints(); out }))
    }
    fn compile_footprints(&mut self) {
        use super::footprint::{Domain as D, EffectFootprint, Inputs as I, Properties as P};
        let subject = match &self.subject {
            Subject::Source => I::at(D::Source,P::IDENTITY),
            Subject::Controller => I::at(D::Context,P::CONTROL),
            Subject::Attached => I::at(D::Source,P::ATTACHMENT),
            Subject::Paired => I::at(D::Source,P::IDENTITY | P::PAIRING) | I::at(D::Objects,P::IDENTITY | P::ZONE | P::PAIRING),
            Subject::SourceFilter(f) => f.inputs().on_source(),
            Subject::Described(s) => s.inputs(),
        };
        let implicit = I::at(D::Source,P::MEMBERSHIP | P::CONTROL | P::ZONE | P::CATALOG | P::IDENTITY | P::TIMESTAMP)
            | I::at(D::Context,P::CONTINUATION | P::BINDINGS | P::LIFETIME | P::VIEW);
        let mut footprints = Vec::new();
        for part in self.parts() {
            for layer in [Layer::Control,Layer::Types,Layer::Colors,Layer::Abilities,Layer::DefinePt,Layer::SetPt,Layer::ChangePt] {
                if !self.part_in(part,layer) { continue; }
                let mut reads = subject | implicit;
                let mut writes = P::NONE;
                for (when,scale,changes) in &self.acts[part..self.end(part)] {
                    reads |= when.inputs() | match scale { Scale::Once => I::NONE, Scale::ForEach(n) | Scale::WhereX(n) => n.inputs() };
                    for change in changes.iter().filter(|c|c.layer()==layer) { reads |= change.inputs(); writes |= change.writes(); }
                }
                let footprint = EffectFootprint { reads, writes: I::at(D::Recipient,writes) };
                footprints.push((part,layer,footprint));
            }
        }
        self.footprints = footprints;
    }
    pub(crate) fn footprint(&self,part:usize,layer:Layer) -> super::footprint::EffectFootprint {
        self.footprints.iter().find(|(p,l,_)|*p==part && *l==layer).map(|(_,_,f)|*f).unwrap_or(super::footprint::EffectFootprint { reads: super::footprint::Inputs::UNKNOWN, writes: super::footprint::Inputs::UNKNOWN })
    }

    /// The ability lowered, when it says what its subject is; None when it does not. What
    /// the text refers back to is read through the definition's links (607.2d).
    fn compile_body(a: &Ability, support: &Support<'_>) -> Option<Res<Characteristic>> {
        // The restriction compiler owns these rows, including their explicit refusals.
        // Never admit a player grant as an object-world characteristic no-op.
        if player_changes(a).is_some() { return None; }
        let mut definitions = Vec::new();
        if crate::keyword::has_changeling(a) {
            definitions.push(Change::DefineTypes(crate::layers::Types {
                types: Vec::new(), subtypes: Vec::new(), every_creature_type: true, keeps: false,
            }));
        }
        if crate::keyword::has_devoid(a) { definitions.push(Change::DefineColors(Vec::new())); }
        if !definitions.is_empty() {
            return Some(Ok(Self { subject: Subject::Source,
                acts: vec![(When::Always, Scale::Once, definitions)], footprints: Vec::new() }));
        }
        if let Some((power, toughness)) = defining_pt(a) {
            return Some((|| {
                if power.iter().chain(toughness).any(crate::layers::asks_pt) { return unsupported("characteristic definition depending on power or toughness"); }
                let power = power.as_ref().map(|n| expr::Amount::compile(n, support, &mut linked(support))).transpose()?;
                let toughness = toughness.as_ref().map(|n| expr::Amount::compile_recalling(n, power.as_ref(), support, &mut linked(support))).transpose()?;
                Ok(Self { subject: Subject::Source, acts: vec![(When::Always, Scale::Once, vec![Change::DefinePt { power, toughness }])], footprints: Vec::new() })
            })());
        }
        let statement = crate::layers::statement(a)?;
        Some((|| {
            let subject = if those_paired(&statement) { Subject::Paired } else {
                Subject::compile(&statement.subject, support, &mut linked(support))?
            };
            let mut acts = Vec::new();
            for (action, when, scale) in &statement.acts {
                crate::layers::removal_scope(action, &statement.subject)?;
                let (scale, x) = Scale::compile(scale, support, &mut linked(support))?;
                // "where X is …" is what gives the act's own numbers an X to read
                let reads = support.clone().with_x(x);
                let changes = Change::compile(action, &reads, &mut linked(&reads))?;
                if scale != Scale::Once && changes.iter().any(|change|
                    matches!(change, Change::Fixed(crate::layers::Change::Inherits(_)))) {
                    return unsupported("inherited abilities: scaled acquisition is not modeled");
                }
                // "This creature gets +1/+1 as long as it's enchanted": the pronoun is the
                // statement's explicit subject
                let mut linked = linked(support);
                let mut bind = expr::SourceBinding { parent: &mut linked, source: subject == Subject::Source };
                acts.push((When::compile(when, support, &mut bind)?, scale, changes));
            }
            // An unconditional self color definition is a CDA (604.3a), including in
            // hidden zones and before the game. Keep that classification on the compiled
            // change so layer ordering and Commander identity use the same fact.
            if subject == Subject::Source && !acts.is_empty() && acts.iter().all(|(when, scale, changes)|
                *when == When::Always && *scale == Scale::Once && !changes.is_empty()
                    && changes.iter().all(|c| matches!(c, Change::Fixed(crate::layers::Change::Colors { added: false, .. }))))
            {
                for (_, _, changes) in &mut acts {
                    for change in changes {
                        if let Change::Fixed(crate::layers::Change::Colors { colors, .. }) = change {
                            *change = Change::DefineColors(std::mem::take(colors));
                        }
                    }
                }
            }
            Ok(Characteristic { subject, acts, footprints: Vec::new() })
        })())
    }

    /// Whether this object's own statement grants it flash as casting begins (601.3d).
    /// This tests the lowered condition without making the ability function in hand.
    pub(crate) fn self_flash(&self, game: &Game, ctx: &Ctx) -> bool {
        self.subject == Subject::Source && self.acts.iter().any(|(when, _, changes)|
            changes.iter().any(|change| matches!(change, Change::Fixed(crate::layers::Change::Gains(keywords))
                if keywords.iter().any(|held| held.keyword.name() == "Flash"))) && when.holds(game, ctx))
    }

    /// Whether it defines its object's colors (604.3): devoid's none as well.
    pub(crate) fn defines_colors(&self) -> bool {
        self.acts.iter().flat_map(|(_, _, changes)| changes).any(|change| matches!(change, Change::DefineColors(_)))
    }

    /// Colors stated by a characteristic-defining ability, including outside games (903.4).
    pub(crate) fn defined_colors(&self) -> impl Iterator<Item = ir::Color> + '_ {
        self.acts.iter().flat_map(|(_, _, changes)| changes).flat_map(|change| match change {
            Change::DefineColors(colors) => colors.as_slice(),
            _ => &[],
        }).copied()
    }

    /// A constant color definition (604.3) describes the selected face during announcement
    /// too, in every zone (113.6a): devoid's no colors, or "~ is all colors".
    pub(crate) fn color_definition(&self) -> Option<&[ir::Color]> {
        self.acts.iter().flat_map(|(_, _, changes)| changes).filter_map(|change| match change {
            Change::DefineColors(colors) => Some(colors.as_slice()), _ => None,
        }).last()
    }

    /// Constant subtype CDAs also describe the selected face during announcement,
    /// before there is a stack incarnation to ask (601.3a, 702.73a).
    pub(crate) fn defined_types(&self) -> impl Iterator<Item = &crate::layers::Types> + '_ {
        self.acts.iter().flat_map(|(_, _, changes)| changes).filter_map(|change| match change {
            Change::DefineTypes(types) => Some(types), _ => None,
        })
    }

    /// Constant characteristic definitions precede other effects in their layer (613.3).
    pub(crate) fn definition_in(&self, part: usize, layer: Layer) -> bool {
        self.acts[part..self.end(part)].iter().flat_map(|(_, _, changes)| changes)
            .any(|change| matches!(change, Change::DefineColors(_) | Change::DefineTypes(_) | Change::DefinePt { .. }) && change.layer() == layer)
    }

    pub(crate) fn has_definition(&self) -> bool {
        self.parts().any(|part| [Layer::Types, Layer::Colors, Layer::DefinePt].into_iter().any(|layer| self.definition_in(part, layer)))
    }

    /// Can it add or remove definition groups, whatever the game is?
    pub(crate) fn changes_membership(&self) -> bool {
        // a chosen basic land type sets the land's abilities (305.7)
        self.acts.iter().any(|(_, _, changes)| changes.iter().any(|c| matches!(c, Change::Fixed(change) if change.changes_membership())
            || matches!(c, Change::ChosenType { .. })))
    }

    /// Does it change anything in this layer, whatever the game is?
    pub(crate) fn changes_in(&self, layer: Layer) -> bool {
        self.acts.iter().any(|(_, _, changes)| changes.iter().any(|c| c.layer() == layer))
    }

    /// Adjacent coordinated changes under one condition form one continuous effect.
    /// Its layer components share applicability and continuation (613.6); separately
    /// conditioned statements retain their own applicability.
    pub(crate) fn parts(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.acts.len()).filter(|&i| i == 0 || self.acts[i].0 != self.acts[i - 1].0)
    }

    fn end(&self, part: usize) -> usize {
        (part + 1..self.acts.len()).find(|&i| self.acts[i].0 != self.acts[part].0).unwrap_or(self.acts.len())
    }

    pub(crate) fn part_in(&self, part: usize, layer: Layer) -> bool {
        self.acts[part..self.end(part)].iter().any(|(_, _, changes)| changes.iter().any(|c| c.layer() == layer))
    }

    /// An unconditional self definition cannot depend on another layer-7a effect when
    /// all its compiled quantities read characteristics fixed in earlier layers.
    pub(crate) fn independent_pt_definition(&self, part: usize) -> bool {
        matches!(self.subject, Subject::Source)
            && self.acts[part..self.end(part)].iter().all(|(when, scale, changes)|
                matches!(when, When::Always) && matches!(scale, Scale::Once)
                    && changes.iter().all(|change| matches!(change,
                        Change::DefinePt { power, toughness }
                            if !power.iter().chain(toughness).any(expr::Amount::reads_pt))))
    }

    /// Applying another effect in the abilities layer cannot change what this part applies
    /// to or does there (613.8a): neither what it is said of nor when it holds reads what
    /// abilities anything has, and what it gives or takes is said outright. Whether the
    /// ability stating it still exists is the layer's to ask ([`Self::removes_abilities`]).
    pub(crate) fn independent_of_abilities(&self, part: usize) -> bool {
        // a change of this layer is words given or taken, whatever its scale counts
        let said = self.acts[part..self.end(part)].iter().flat_map(|(_, _, changes)| changes)
            .filter(|c| c.layer() == Layer::Abilities).all(|c| matches!(c, Change::Fixed(change) if !matches!(change, crate::layers::Change::Inherits(_))));
        said && !(self.subject.reads() | self.acts[part].0.reads()).any(expr::Reads::ABILITIES)
    }

    /// What deciding the whole statement can read ([`expr::Reads`]): what it is said of,
    /// when each of its parts holds, and the numbers its changes and their scales count.
    #[allow(dead_code)]
    pub(crate) fn reads(&self) -> expr::Reads {
        self.acts.iter().fold(self.subject.reads(), |all, (when, scale, changes)| {
            let scale = match scale { Scale::Once => expr::Reads::NONE, Scale::ForEach(n) | Scale::WhereX(n) => n.reads() };
            changes.iter().fold(all | when.reads() | scale, |all, change| all | change.reads())
        })
    }

    /// Can this part take from an object the abilities of its own definition (113.10), so
    /// that another static ability may stop existing as this applies?
    pub(crate) fn removes_abilities(&self, part: usize) -> bool {
        self.acts[part..self.end(part)].iter().flat_map(|(_, _, changes)| changes)
            .any(|c| matches!(c, Change::Fixed(change) if change.removes_abilities()))
    }

    /// A stable ordinal in the compiled ability, including conditionally inactive grants.
    pub(crate) fn grant_offset(&self, part: usize) -> u32 {
        self.acts[..part].iter().flat_map(|(_, _, c)| c).map(|c| match c {
            Change::Fixed(crate::layers::Change::Gives(templates)) => templates.len() as u32,
            _ => 0,
        }).sum()
    }

    /// Public cards needed by a dynamic inheritance query, separately from the
    /// zones this statement can change or in which registrations function.
    pub(crate) fn reads_graveyard_donors(&self) -> bool {
        self.acts.iter().flat_map(|(_, _, changes)| changes).any(|change|
            matches!(change, Change::Fixed(crate::layers::Change::Inherits(set)) if set.graveyard_donors()))
    }

    /// Potential definition grants, including inactive conditions. The layer scheduler
    /// needs their identities before applying the producer; membership decides existence.
    pub(crate) fn potential_grants(&self, part: usize) -> Vec<(u32, crate::ability::Template)> {
        self.acts[part..self.end(part)].iter().flat_map(|(_, _, changes)| changes)
            .flat_map(|c| match c {
                Change::Fixed(crate::layers::Change::Gives(templates)) => templates.as_slice(),
                _ => &[],
            }).enumerate().map(|(i, what)| (self.grant_offset(part) + i as u32, *what)).collect()
    }

    pub(crate) fn part_changes(&self, game: &Game, ctx: &Ctx, layer: Layer, part: usize, begun: bool) -> Vec<crate::layers::Change> {
        if !begun && !self.acts[part].0.holds(game, ctx) { return Vec::new(); }
        self.acts[part..self.end(part)].iter().flat_map(|(_, scale, changes)|
            changes.iter().filter(|c| c.layer() == layer).map(|c| scale.apply(c, game, ctx))).collect()
    }
}

/// One printed ability's registration: what compiled, or why the engine does not hold it in
/// force. Identity is the ability's index within its definition's `abilities`.
#[derive(Debug, Clone)]
pub(crate) struct Registered<T> {
    pub(crate) ability: usize,
    pub(crate) result: Res<T>,
}

/// Each ability of the definition that is one of these statements, lowered with its own
/// identity: the printed abilities, then those its keywords make
/// ([`crate::keyword::executable_in`]).
fn registered<T>(def: &CardDef, compile: impl Fn(usize, &Ability, &Support<'_>) -> Option<Res<T>>) -> Vec<Registered<T>> {
    def.abilities.iter().chain(def.keyword_abilities.iter().map(|(_, a)| a)).enumerate().filter_map(|(i, a)| {
        let support = Support::definition(def, Some(crate::binding::AbilityKey::printed(i)));
        compile(i, a, &support).map(|result| Registered { ability: i, result })
    }).collect()
}

/// Every restriction and requirement a definition's static abilities state, lowered once.
pub(crate) fn restrictions(def: &CardDef) -> Vec<Registered<Restriction>> {
    registered(def, |i, a, support| Restriction::compile(a, def.modes_of(i), support))
}

/// Every statement of what its subject is that a definition's static abilities make,
/// lowered once.
pub(crate) fn characteristics(def: &CardDef) -> Vec<Registered<Characteristic>> {
    registered(def, |_, a, support| Characteristic::compile(a, support))
}

/// All rule contributions of a keyword, lowered once with their individual scopes.
/// Rules refer to nothing chosen, so they need no ability identity. A failed contribution
/// stays an error rather than silently disappearing beside a successful one.
pub(crate) fn keyword(k: Keyword) -> Option<&'static [Res<Restriction>]> {
    static KEPT: OnceLock<Vec<(Keyword, Vec<Res<Restriction>>)>> = OnceLock::new();
    let kept = crate::token::init(&KEPT, || {
        let def = CardDef::new("Keyword", "", "Creature", "", None).expect("a creature with no text is read");
        let support = Support::definition(&def, None);
        Keyword::all().filter_map(|k| {
            let item = ir::token::KeywordInstance { keyword: k, param: ir::token::KeywordParam::None };
            let declared = crate::keyword::declare(&item).ok()?;
            let rules = keyword_rules(&declared, &support);
            (!rules.is_empty()).then_some((k, rules))
        }).collect()
    });
    kept.iter().find(|(keyword, _)| *keyword == k).map(|(_, rules)| rules.as_slice())
}

fn keyword_rules(declared: &crate::keyword::Declared, support: &Support<'_>) -> Vec<Res<Restriction>> {
    declared.gives.iter().filter_map(|(scope, gives)| {
        let crate::keyword::Gives::Rule(ability) = gives else { return None };
        Some(Restriction::compile(ability, &[], support)
            .unwrap_or_else(|| crate::unsupported("keyword rule: no restriction"))
            .map(|mut rule| { rule.scope = *scope; rule }))
    }).collect()
}

#[cfg(test)]
mod tests;
