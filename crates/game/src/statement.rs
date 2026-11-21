//! Static abilities are written as statements (CR 604.1): a subject and what is true of it,
//! for as long as a condition holds when the text says one. "Creatures you control get +1/+1."
//! "This creature can't block." "This land enters tapped unless you control two or more other
//! lands." "As long as this creature is untapped, other permanents enter untapped."
//!
//! What a statement is of and when it holds is read here once; what it states is for the part
//! of the engine that consults it: what a permanent is ([`crate::layers`]), what can't happen
//! or must ([`crate::restrict`]), what an event that would happen is instead
//! ([`crate::replace`]).

use crate::event::Target;
use crate::select::Ctx;
use crate::state::Game;
use ir::effect::{Body, Modifier};
use ir::statics::Static;
use ir::{Ability, Action, Amount, Condition, Effect, Lead, Sentence, Subject};

/// When a statement holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum When<'a> {
    Always,
    All(Vec<When<'a>>),
    During(&'a ir::event::During),
    /// "as long as you control an Island"
    AsLongAs(&'a Condition),
    /// "unless you control a Forest"
    Unless(&'a Condition),
}

/// How many times over a statement says what it says: "gets +1/+1 for each other creature
/// you control", "gets +X/+X, where X is the number of cards in your hand".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scale<'a> {
    Once,
    ForEach(&'a Amount),
    WhereX(&'a Amount),
}

/// A static ability read as a statement: its subject, and each thing it states with when that
/// holds and how many times over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement<'a> {
    pub subject: Subject,
    pub acts: Vec<(&'a Action, When<'a>, Scale<'a>)>,
}

/// The statement an ability's text is, when it is one sentence that states something of a
/// subject. The condition stands in front of the sentence ("As long as …,"), after one of its
/// acts ("has flying as long as …") or after all of it ("… unless you control a Forest").
pub fn read(a: &Ability) -> Option<Statement<'_>> {
    let Ability::Text(Effect(sentences)) = a else { return None };
    let [sentence] = sentences.as_slice() else { return None };
    fn body<'a>(sentence: &'a Sentence, conditions: &mut Vec<When<'a>>) -> Option<&'a Body> {
        match sentence {
            Sentence::Do(body)=>Some(body),
            Sentence::Led {lead:Lead::Modifier(Modifier::AsLongAs(c)|Modifier::If(c)),rest}=>{conditions.push(When::AsLongAs(c));body(rest,conditions)},
            Sentence::Led {lead:Lead::Modifier(Modifier::During(d)),rest}=>{conditions.push(When::During(d));body(rest,conditions)},
            Sentence::Static(s)=>match s.as_ref(){Static::Unless{body,cond}=>{conditions.push(When::Unless(cond));Some(body)},_=>None},
            _=>None,
        }
    }
    let mut leading=Vec::new();
    let body=body(sentence,&mut leading)?;
    let Body { first, rest } = body;
    let permission = |a: &Action| matches!(a, Action::Static(s) if matches!(**s, ir::StaticAct::CastAsThoughFlash { this_turn: false, .. } | ir::StaticAct::ActivateAsThoughUnused(_)));
    let permission_clause = std::iter::once(&first.first).chain(first.rest.iter().map(|(_, a)| a)).all(|a| permission(&a.action));
    if !rest.is_empty() || (first.may && !permission_clause) || first.also {
        return None;
    }
    let mut subject = first.subject.as_ref()?.clone();
    // A singular pronoun in a statement can name the explicit subject of its condition.
    // Keep this binding definition-local, before the layers execute it (201.5).
    if matches!(&subject,Subject::Selector(s) if s.quant==ir::Quant::Definite(ir::Ref::Anaphor(ir::selector::Anaphor::It))) {
        let antecedents:Vec<_>=leading.iter().filter_map(|w|match w {
            When::AsLongAs(Condition::Is{what,..})=>Some(what.as_ref()),_=>None,
        }).collect();
        if let [source]=antecedents.as_slice() && matches!(source.quant, ir::Quant::Definite(ir::Ref::This | ir::Ref::AttachedTo(_))) {subject=Subject::Selector((*source).clone());}
    }
    let mut acts = Vec::new();
    for act in std::iter::once(&first.first).chain(first.rest.iter().map(|(_, a)| a)) {
        let (mut when, mut scale) = (leading.clone(), Scale::Once);
        for m in &act.mods {
            match m {
                Modifier::AsLongAs(c) | Modifier::If(c) => when.push(When::AsLongAs(c)),
                Modifier::During(d) => when.push(When::During(d)),
                Modifier::ForEach(n) if scale == Scale::Once => scale = Scale::ForEach(n),
                Modifier::WhereX(x) if scale == Scale::Once => scale = Scale::WhereX(x),
                _ => return None,
            }
        }
        if act.may && !permission(&act.action) {
            return None;
        }
        let when=match when.len(){0=>When::Always,1=>when.pop().unwrap(),_=>When::All(when)};
        acts.push((&act.action, when, scale));
    }
    Some(Statement { subject, acts })
}

/// What a statement, or a rule a resolved spell put in force, is said of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Affected {
    /// The things an effect named when it began: its targets.
    Things(Vec<Target>),
    /// A rule-changing effect keeps its predicate live (611.2c), with captured local values.
    Compiled { set: crate::program::expr::Set, locals: Vec<Option<crate::binding::Value>> },
    Union(Vec<Affected>),

}

/// Is this object or player one the statement is said of, now?
pub fn affects(game: &Game, ctx: &Ctx, affected: &Affected, t: Target) -> bool {
    match affected {
        Affected::Things(things) => things.contains(&t),
        Affected::Compiled { set, locals } => set.contains(game, ctx, locals, t),
        Affected::Union(items) => items.iter().any(|a| affects(game, ctx, a, t)),
    }
}

impl crate::retention::Trace for Affected {
    fn trace(&self, visit: &mut dyn FnMut(crate::ObjId)) {
        match self {
            // These are compared to the queried target, never dereferenced.
            Affected::Things(_) => {},
            Affected::Compiled { set: _, locals } => locals.trace(visit),
            Affected::Union(items) => items.trace(visit),
        }
    }
}
