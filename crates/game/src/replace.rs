//! Replacement effects (CR 614): effects that watch for an event that would happen and replace
//! it, wholly or in part, with another (614.1). The event replaced never happens (614.6), so
//! they are applied before it does, where the event is made to happen.
//!
//! Supported replacements include unconditional life-gain multipliers (119.10, 614.1a),
//! and how a permanent enters the battlefield (614.1c–d): "This creature
//! enters tapped.", "This creature enters with two +1/+1 counters on it.", "Creatures your
//! opponents control enter tapped.", for as long as a condition holds when the text says one
//! ("This land enters tapped unless you control two or more other lands."). They are static
//! abilities read as statements ([`crate::statement`]), and the places a permanent enters
//! ([`Game::move_to`], [`Game::create_token`]) ask them how it does: it is never on the
//! battlefield untapped first, and nothing sees it become tapped.
//!
//! A permanent's own text may also have its controller choose as it enters, before it is on
//! the battlefield (614.12a): whether to pay life ("As this land enters, you may pay 2 life.
//! If you don't, it enters tapped."), a creature type ("As this land enters, choose a creature
//! type."), which the permanent keeps and its other abilities call "the chosen type" (607.2d).
//!
//! Every such effect that applies is applied, each once (614.5). When several apply to one
//! permanent its controller chooses the order (616.1); the ones here come to the same in any
//! order (tapped, and counters that add up), so nothing is asked.

mod damage;

use crate::card::CardDef;
use crate::event::Target;
use crate::binding::{AbilityKey, Kind, LinkId, Record};
use crate::operation::ReplacementId;
use crate::program::expr::{self, NoLocals, linked};
use crate::program::registration::{Registered, Subject as Said, When};
use crate::select::{Ctx, Support, name};
use crate::state::{Answer, Choice, Game, ObjId, PlayerId};
use crate::statement::{Scale, Statement};
use crate::trigger::Noun;
use crate::{Res, unsupported};
use ir::statics::{Arrival, StaticAct};
use ir::{Ability, Act, Action, Amount, Anaphor, Clause, Condition, Effect, Lead, Modifier, Quant, Ref, Selector, Sentence, Subject};

/// One way a permanent enters the battlefield (614.1c–d), lowered: its number is counted as
/// the permanent enters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Enters {
    Prepared,
    Tapped,
    /// "with two +1/+1 counters on it", "with an additional +1/+1 counter on it": on top of
    /// what it enters with anyway.
    Counters { kind: String, count: expr::Amount },
}

impl Enters {
    /// The one place a way of entering is refused.
    fn compile(arrival: &Arrival, each: Option<&expr::Amount>, support: &Support<'_>) -> Res<Enters> {
        match arrival {
            Arrival::Tapped if each.is_some() => unsupported("enters tapped for each of a number"),
            Arrival::Tapped => Ok(Enters::Tapped),
            Arrival::Prepared => Ok(Enters::Prepared),
            Arrival::Counters(with) => {
                let c = &with.counters;
                if c.up_to {
                    return unsupported("enters with up to a number of counters");
                }
                crate::layers::counter_ok(&c.kind.0)?;
                let count = expr::Amount::compile(&c.count, support, &mut linked(support))?;
                // "with a +1/+1 counter on it for each time it was kicked": that many times over
                Ok(Enters::Counters { kind: c.kind.0.clone(), count: match each { Some(each) => count.times(each.clone()), None => count } })
            }
            other => unsupported(format!("enters: {}", name(other))),
        }
    }

    /// Modify how a permanent enters by this.
    pub(crate) fn apply(&self, game:&Game, ctx:&Ctx, entry:&mut Entry) {
        self.checked_apply(game,ctx,entry).expect("bounded fixture entry counter expression");
    }
    pub(crate) fn checked_apply(&self, game: &Game, ctx: &Ctx, entry: &mut Entry) -> Res<()> {
        match self {
            Enters::Tapped => entry.tapped = true,
            Enters::Prepared => entry.enters_prepared = true,
            Enters::Counters { kind, count } => entry.counters.push((kind.clone(), count.checked(game, ctx, &NoLocals)?)),
        }        Ok(())
    }
}

/// The choice a permanent's own text has its controller make as it enters (614.12a), lowered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Asked {
    /// The life its controller may pay, and how it enters when they don't.
    Life { life: expr::Amount, otherwise: Vec<Enters> },
    /// Reveal a qualifying card from hand, or use the fallback entry modification.
    Reveal { card: expr::Set, otherwise: Vec<Enters> },
    /// Behold: a qualifying card of the hand or permanent its controller controls (701.4a),
    /// or the fallback entry modification.
    Behold { cards: expr::Cards, permanents: expr::Set, otherwise: Vec<Enters> },
    /// A single typed value, referred to by linked abilities (607.2d).
    Value(crate::binding::Domain),
    /// A typed value, then the life its controller may pay, and how it enters when they
    /// don't: Multiversal Passage.
    ValueThenLife { kind: crate::binding::Domain, life: expr::Amount, otherwise: Vec<Enters> },
}

/// A static ability that is a replacement effect the engine applies, lowered once when its
/// definition is loaded. It names no object: the permanent whose ability it is and who
/// controls it are supplied as the event it replaces is about to happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Replacement {
    /// A mandatory replacement of this permanent's battlefield-to-graveyard move.
    DeathToLibrary(ir::action::LibraryPosition),
    /// A resolving spell/ability changes this card's discard destination (614.1a, 701.9).
    DiscardToBattlefield { cause: Option<expr::Controller> },
    /// Madness (702.35a): "If a player would discard this card, that player discards it, but
    /// exiles it instead of putting it into their graveyard", functioning in a hand. Applied,
    /// it makes `trigger`, the index of its triggered ability among the card's, trigger.
    Madness { trigger: usize },
    /// Dredge instances within one printed ability row (702.52). Each keeps its own identity.
    Dredge(Vec<(usize, u32)>),
    /// Each printed instance supplies its own entry replacement (702.44d).
    Sunburst(Vec<usize>),
    /// Replace each individual draw with this many draws by the same player (614.1a).
    Draw { who: Noun, count: u32 },
    /// The choice its permanent's controller makes as it enters (614.12a).
    AsEnters(Asked),
    CopyEntry(crate::copy::Entry),
    /// An unconditional scaling of the life these players gain (119.10).
    LifeGain { who: Noun, factor: u32 },
    /// Token creation, before continuous effects and entry replacements (701.7b).
    Tokens { scope: TokenScope, change: TokenChange, effect_only: bool },
    /// Solved static replacement functions only with the noncopiable designation (702.169b).
    Solved(Box<Replacement>),
    /// An unconditional change of how many counters of one kind are put on these permanents
    /// (614.1a, 122.6).
    Counters { on: Noun, by: Option<Noun>, effect_only: bool, kinds: CounterScope, change: CounterChange },
    /// Modify a proposed damage amount, keeping its source and recipient (614.1a).
    Damage { from: damage::Source, to: Vec<Noun>, kind: ir::event::DamageKind, change: CounterChange },
    /// How what it is said of enters the battlefield (614.1c–d), each part with when it holds.
    Arrives { subject: Said, acts: Vec<(When, Vec<Enters>)> },
    /// "As this creature is turned face up, put five +1/+1 counters on it.": counters put on
    /// it while it is being turned face up, not afterward (708.11), of each kind how many.
    AsTurnedFaceUp(Vec<(String, expr::Amount)>),
    /// Where what it describes goes instead of a graveyard (614.1a), and what else is done
    /// in that modified event: "If a card or token would be put into a graveyard from
    /// anywhere, exile it instead.", "If a nontoken creature an opponent controls would die,
    /// instead exile it and you gain 2 life."
    Elsewhere { moved: Moved, instead: Instead, gain: Option<u32>, then: Option<std::sync::Arc<crate::program::Program>> },
}

/// What a zone-destination replacement watches for (614.1a).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Moved {
    /// "If this card would be put into a graveyard from anywhere": the object's own, which
    /// functions in every zone it can be put into a graveyard from (113.6).
    Own,
    /// "If a card or token would be put into an opponent's graveyard from anywhere": what is
    /// put there, as it is before it moves, and whose graveyard (None: any).
    IntoGraveyard { what: Vec<Noun>, into: Option<Noun> },
    /// "If a creature an opponent controls would die": put into a graveyard from the
    /// battlefield (700.4).
    Dies(expr::Set),
}

/// Where it goes instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Instead {
    Exile,
    /// "reveal this card and shuffle it into its owner's library instead"
    Shuffle,
}

impl Replacement {
    /// The instructions of the reflexive triggered ability applying it triggers: "When you
    /// do, create a 2/2 green Wolf creature token." (603.12)
    pub(crate) fn reflexive(&self) -> Option<&std::sync::Arc<crate::program::Program>> {
        match self {
            Self::Elsewhere { then, .. } => then.as_ref(),
            Self::Solved(underlying) => underlying.reflexive(),
            _ => None,
        }
    }
    /// These statics carry no prior entry choice, timestamp order or event history.
    /// Solved starts false in a bounded initial premise; the underlying replacement
    /// uses the same mandatory ordering once ordinary play solves the Case.
    pub(crate) fn initial_without_history(&self)->bool {
        match self { Self::Tokens {..}|Self::Counters {..}=>true,
            Self::Solved(inner)=>matches!(inner.as_ref(),Self::Tokens {..}), _=>false }
    }

    /// The ability lowered, when it is a replacement effect of one of these kinds; None when
    /// it is not one. The one place such an ability is refused. What the text refers back to
    /// is read through the definition's links (607.2d).
    fn compile(a: &Ability, support: &Support<'_>) -> Option<Res<Replacement>> {
        if let Ability::Prefixed { keyword, ability } = a
            && keyword.name() == "Solved" {
            return Self::compile(ability, support).map(|r| r.map(|r| Self::Solved(Box::new(r))));
        }
        if let Ability::Keywords(items) = a {
            let sunburst: Vec<_> = items.iter().enumerate().filter_map(|(part, item)| crate::keyword::sunburst(item).then_some(part)).collect();
            if !sunburst.is_empty() {
                if items.iter().any(|item| crate::draw::dredge(item).is_some()) {
                    return Some(unsupported("mixed sunburst and dredge registrations in one row"));
                }
                return Some(Ok(Replacement::Sunburst(sunburst)));
            }
            let found: Vec<_> = items.iter().enumerate().filter_map(|(part, item)|
                crate::draw::dredge(item).map(|result| result.map(|n| (part, n)))).collect();
            if !found.is_empty() { return Some(found.into_iter().collect::<Res<Vec<_>>>().map(Replacement::Dredge)); }
        }
        if let Ability::Text(Effect(sentences)) = a
            && let Some(Sentence::Static(rule)) = sentences.first()
            && let ir::Static::Replacement(r) = &**rule
            && r.single_event().is_none() {
            return Some(unsupported("replacement: alternative event patterns not lowered"));
        }
        if let Some(counters) = as_turned_face_up(a, support) { return Some(counters.map(Replacement::AsTurnedFaceUp)); }
        let arrive_each = |arrivals: &[Arrival], each: Option<&expr::Amount>| arrivals.iter().map(|arrival| Enters::compile(arrival, each, support)).collect::<Res<Vec<_>>>();
        let arrive = |arrivals: &[Arrival]| arrive_each(arrivals, None);
        if let Some(copy) = crate::copy::Entry::compile(a, support) { return Some(copy.map(Replacement::CopyEntry)); }
        if let Some(asked) = as_enters(a) {
            return Some((|| {
                Ok(Replacement::AsEnters(match asked {
                    AsEnters::Life(life, otherwise) => {
                        Asked::Life { life: expr::Amount::compile(life, support, &mut linked(support))?, otherwise: arrive(otherwise)? }
                    }
                    AsEnters::Reveal(card, otherwise) => {
                        if card.quant != Quant::Indefinite(ir::Count::Exactly(Amount::Const(1)))
                            || card.chooser.is_some() || card.random.is_some()
                            || card.set.domain != ir::Domain::Card
                            || !card.set.zone.as_ref().is_some_and(|zone| zone.zones.items == [ir::Zone::Hand]
                                && zone.slice.is_none() && zone.owner.as_deref().is_some_and(crate::select::is_you)) {
                            return unsupported("entry reveal: one qualifying card from your hand");
                        }
                        Asked::Reveal { card: expr::Set::compile(&card.set, support, &mut linked(support))?, otherwise: arrive(otherwise)? }
                    }
                    AsEnters::Behold(beheld, otherwise) => {
                        if beheld.quant != Quant::Indefinite(ir::Count::Exactly(Amount::Const(1))) || beheld.chooser.is_some() || beheld.random.is_some() {
                            return unsupported("entry behold: one qualifying card or permanent");
                        }
                        let (cards, permanents) = crate::cost::beheld(beheld)?;
                        Asked::Behold { cards: expr::Cards::compile(&cards, support, &mut linked(support))?,
                            permanents: expr::Set::compile(&permanents, support, &mut linked(support))?, otherwise: arrive(otherwise)? }
                    }
                    AsEnters::Value(kind) => Asked::Value(kind),
                    AsEnters::ValueThenLife(kind, life, otherwise) => Asked::ValueThenLife {
                        kind, life: expr::Amount::compile(life, support, &mut linked(support))?, otherwise: arrive(otherwise)?,
                    },
                }))
            })());
        }
        if let Some(position) = death_to_library(a) { return Some(Ok(Replacement::DeathToLibrary(position))); }
        if let Some(elsewhere) = elsewhere(a, support) { return Some(elsewhere); }
        if let Some(cause) = discard_to_battlefield(a) {
            return Some(cause.map(|cause| Replacement::DiscardToBattlefield { cause }));
        }
        if let Some((who, count)) = draw_replacement(a) {
            return Some(Noun::compile(who, support).map(|who| Replacement::Draw { who, count }));
        }
        if let Some((who, factor)) = life_gain_multiplier(a) {
            return Some(Noun::compile(who, support).map(|who| Replacement::LifeGain { who, factor }));
        }
        if let Some(replacement) = token_creation(a) { return Some(replacement); }
        if let Some(r) = counter_replacement(a) {
            if let CounterScope::Named(kind) = &r.kinds {
                if r.on.set.domain==ir::Domain::Player {
                    if let Err(error)=expr::player_counter_kind(&ir::PlayerCounter::Named(ir::CounterKind(kind.clone()))) {return Some(Err(error));}
                } else if kind!="+1/+1" {
                    return Some(unsupported("counter replacement: named permanent-counter replacements currently require +1/+1"));
                }
            }
            return Some((|| Ok(Replacement::Counters {
                on: Noun::compile(r.on, support)?,
                by: r.by.map(|by| Noun::compile(by, support)).transpose()?,
                effect_only: r.effect_only, kinds: r.kinds, change: r.change,
            }))());
        }
        if let Some(damage) = damage::compile(a, support) { return Some(damage); }
        let statement = statement(a)?;
        Some((|| {
            let subject = Said::compile(&statement.subject, support, &mut linked(support))?;
            // the permanent's own entry replacement reads the X of the spell it was (107.3m)
            let support = &if subject == Said::Source { support.clone().with_spell_x() } else { support.clone() };
            let mut acts = Vec::new();
            for (action, when, scale) in &statement.acts {
                let when = When::compile(when, support, &mut linked(support))?;
                // In "this creature enters ... exiled with it", the pronoun belongs to
                // the explicit subject of this statement, not an unbound effect result.
                let mut linked = linked(support);
                let mut bind = expr::SourceBinding { parent: &mut linked, source: subject == Said::Source };
                let each = match scale {
                    Scale::ForEach(n) => Some(expr::Amount::compile(n, support, &mut bind)?),
                    _ => None,
                };
                let enters = arrivals(action).unwrap_or_default().iter().map(|arrival| Enters::compile(arrival, each.as_ref(), support)).collect::<Res<Vec<_>>>()?;
                acts.push((when, enters));
            }
            Ok(Replacement::Arrives { subject, acts })
        })())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TokenScope { All, Creature, Artifact }
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TokenChange { Multiply(u32), Append(crate::copy::Token) }
impl TokenScope {
    pub(crate) fn matches(self,token:&crate::copy::Token)->bool {
        match self { Self::All=>true, Self::Creature=>token.current().chars.types.contains(&ir::CardType::Creature), Self::Artifact=>token.current().chars.types.contains(&ir::CardType::Artifact) }
    }
}

/// Mandatory multipliers retain token predicates separately from entry characteristics.
fn token_creation(a:&Ability)->Option<Res<Replacement>> {
    use ir::{EventAction,EventCause,Domain,Filter,Atom,Status,Role};
    use ir::statics::{Static,Substitute};
    let Ability::Text(Effect(sentences))=a else{return None};
    let [Sentence::Static(statement)]=sentences.as_slice() else{return None};
    let Static::Replacement(r)=statement.as_ref() else{return None};
    let event = r.single_event()?;
    if r.next_time || r.this_turn || r.while_.is_some() || event.counted()
        || event.during.is_some() || event.condition.is_some() {return None;}
    let (change,effect_voice,active_plus)=match &r.with {
        Substitute::TokensCreated {count:Amount::Multiple(factor,n),effect_voice}
            if matches!(**n,Amount::That {many:true}) && *factor>=2 && !r.instead_first =>
            (Ok(TokenChange::Multiply(*factor)),*effect_voice,false),
        Substitute::TokensPlus {token,active} if r.instead_first==*active
            && token.count==Amount::Const(1) && !token.tapped && token.combat.is_none() && token.attached_to.is_none() =>
            (crate::token::token_values(&token.kind).map(TokenChange::Append),false,*active),
        _=>return None,
    };
    let [EventAction::Deed(deed)]=event.actions.as_slice() else{return None};
    if deed.verb!=ir::Verb::Create {return None;}
    let effect_only=matches!(event.cause,Some(EventCause::Effect));
    let tokens=if deed.passive && !effect_only && !effect_voice && !active_plus {
        let [tokens]=event.subjects.as_slice() else{return None};tokens
    } else if !deed.passive && effect_only && effect_voice && event.subjects.is_empty() {
        deed.noun(Role::What)?
    } else if !deed.passive && !effect_only && active_plus {
        let [who]=event.subjects.as_slice() else{return None};
        if !crate::select::is_you(who) {return None;} deed.noun(Role::What)?
    } else {return None;};
    if !active_plus && (!crate::select::is_you(deed.noun(Role::Under)?) || deed.args.len()!=if deed.passive {1}else{2}) {return None;}
    if active_plus && deed.args.len()!=1 {return None;}
    if tokens.set.domain!=Domain::Permanent || !matches!(tokens.quant,Quant::Indefinite(ir::Count::AtLeast(Amount::Const(1))))
        || tokens.chooser.is_some() || tokens.random.is_some() || tokens.set.controller.is_some()
        || tokens.set.owner.is_some() || tokens.set.caster.is_some() || tokens.set.zone.is_some() || tokens.set.other {return None;}
    fn atoms(f:&Filter,out:&mut Vec<Atom>)->bool {match f {
        Filter::Atom(a)=>{out.push(a.clone());true},Filter::All(parts)=>parts.iter().all(|p|atoms(p,out)),_=>false,
    }}
    let mut parts=Vec::new();if !atoms(&tokens.set.filter,&mut parts) {return None;}
    if !parts.contains(&Atom::Status(Status::Token)) {return None;}
    parts.retain(|a|*a!=Atom::Status(Status::Token));
    let scope=match parts.as_slice(){[]=>TokenScope::All,[Atom::Type(ir::CardType::Creature)]=>TokenScope::Creature,[Atom::Type(ir::CardType::Artifact)]=>TokenScope::Artifact,_=>return None};
    Some(change.map(|change|Replacement::Tokens {scope,change,effect_only}))
}

/// Every replacement effect of these kinds that a definition's abilities (printed, then
/// those its keywords make) are, lowered once, each with the refusal of one the engine does
/// not apply.
pub(crate) fn replacements(def: &CardDef) -> Vec<Registered<Replacement>> {
    let madness = crate::keyword::madness(def);
    def.abilities.iter().chain(def.keyword_abilities.iter().map(|(_, a)| a)).enumerate().filter_map(|(i, a)| {
        if let Some((_, _, trigger)) = madness.iter().find(|(row, ..)| *row == i) {
            return Some(Registered { ability: i, result: Ok(Replacement::Madness { trigger: *trigger }) });
        }
        let support = Support::definition(def, Some(AbilityKey::printed(i)));
        Replacement::compile(a, &support).map(|result| Registered { ability: i, result })
    }).collect()
}

impl Game {
    /// The replacement effects of this object the engine applies, each with the key of its
    /// ability: what its surviving definition groups keep lowered. Nothing is lowered here.
    /// Removed abilities do not apply, including in a prospective-entry view. Granted counter replacements share this effective discovery; other replacement
    /// families remain refused by [`crate::ability::template`].
    pub(crate) fn replacements(&self, source: ObjId) -> impl Iterator<Item = (AbilityKey, &Replacement)> {
        self.origins(source, crate::ability::View::Effective { membership_changed: self.membership_changed() }).into_iter().flat_map(move |(origin, def)|
            def.replacements.iter().filter_map(move |r| Some((AbilityKey { origin: origin.clone(), index: r.ability }, match r.result.as_ref().ok()? { Replacement::Solved(inner) if self.obj(source).solved => inner.as_ref(), Replacement::Solved(_) => return None, other => other }))))
    }
}

/// Lower an unconditional self death destination. Other events, optional instructions,
/// owners, ordering and compound bodies remain refused by ordinary admission.
fn death_to_library(a: &Ability) -> Option<ir::action::LibraryPosition> {
    use ir::{Domain, EventAction, ObjectSet, Verb, Whose};
    use ir::statics::{Static, Substitute};
    let Ability::Text(Effect(sentences)) = a else { return None };
    let [Sentence::Static(statement)] = sentences.as_slice() else { return None };
    let Static::Replacement(r) = statement.as_ref() else { return None };
    let event = r.single_event()?;
    if r.next_time || r.this_turn || r.while_.is_some() || event.counted()
        || event.during.is_some() || event.condition.is_some() || event.cause.is_some() { return None; }
    let [who] = event.subjects.as_slice() else { return None };
    if *who != Selector::new(Quant::Definite(Ref::This), ObjectSet::of(Domain::Permanent)) { return None; }
    let [EventAction::Deed(deed)] = event.actions.as_slice() else { return None };
    if deed.verb != Verb::Die || deed.passive || !deed.args.is_empty() { return None; }
    let Substitute::Body(body) = &r.with else { return None };
    let Clause { subject: None, may: false, also: false, first, rest } = &body.first else { return None };
    if !rest.is_empty() || !body.rest.is_empty() || first.may
        || !(first.mods.as_slice() == [Modifier::Instead] || r.instead_first && first.mods.is_empty()) { return None; }
    let Action::Move { moves, from: None, .. } = &first.action else { return None };
    let [(objects, ir::Destination::Library(place))] = moves.as_slice() else { return None };
    let [what] = objects.items.as_slice() else { return None };
    if *what != Selector::new(Quant::Definite(Ref::Anaphor(Anaphor::It)), ObjectSet::of(Domain::Object))
        || place.library != Some(Whose::Owner) || place.back || place.order.is_some() { return None; }
    let ir::Position::One(position @ (ir::action::LibraryPosition::Top | ir::action::LibraryPosition::Bottom)) = place.position else { return None };
    Some(position)
}

/// A static replacement of where something would be put instead of a graveyard (614.1a):
/// exiled, or revealed and shuffled into its owner's library; Some(refusal) when the event
/// is that but what it does instead is not one of these.
fn elsewhere(a: &Ability, support: &Support<'_>) -> Option<Res<Replacement>> {
    use ir::{Domain, EventAction, ObjectSet, Whose};
    use ir::statics::{Static, Substitute};
    let Ability::Text(Effect(sentences)) = a else { return None };
    let Some(Sentence::Static(statement)) = sentences.first() else { return None };
    let Static::Replacement(r) = statement.as_ref() else { return None };
    let event = r.single_event()?;
    if r.next_time || r.this_turn || r.while_.is_some() || event.counted() || event.during.is_some()
        || event.condition.is_some() || event.cause.is_some() || event.nth_each_turn.is_some() || event.except_nth_in_step.is_some() { return None; }
    let [who] = event.subjects.as_slice() else { return None };
    let this = who.quant == Quant::Definite(Ref::This);
    let one = who.quant == Quant::Indefinite(ir::Count::Exactly(Amount::Const(1))) && who.chooser.is_none() && who.random.is_none();
    if !this && !one { return None; }
    // "would be put into a graveyard from anywhere", "would die"
    let dies = match event.actions.as_slice() {
        [EventAction::PutIntoGraveyard { from: None, .. }] => false,
        [EventAction::Deed(deed)] if deed.verb == ir::Verb::Die && !deed.passive && deed.args.is_empty() => true,
        _ => return None,
    };
    let refused = |why: &str| Some(unsupported(format!("zone-destination replacement: {why}")));
    // "When you do, create a 2/2 green Wolf creature token.": a reflexive triggered ability
    // the replacement's application triggers (603.12)
    let then = match &sentences[1..] {
        [] => None,
        [whole @ Sentence::Led { lead: ir::effect::Lead::WhenYouDo, rest }] if !this => match crate::program::reflexive(whole, rest, support) {
            Ok(program) => Some(program), Err(e) => return Some(Err(e)),
        },
        _ => return refused("a further sentence after it"),
    };
    let it = |s: &Selector| s.quant == Quant::Definite(Ref::Anaphor(Anaphor::It)) && s.set == ObjectSet::of(Domain::Object) && s.chooser.is_none() && s.random.is_none();
    let Substitute::Body(body) = &r.with else { return refused("what it does instead") };
    let Clause { subject: None, may: false, also: false, first, rest } = &body.first else { return refused("what it does instead") };
    if first.may || !body.rest.iter().all(|(_, c)| matches!(c, Clause { may: false, also: false, rest, .. } if rest.is_empty())) || body.rest.len() > 1 {
        return refused("what it does instead");
    }
    let marked = |mods: &[Modifier]| mods == [Modifier::Instead] || r.instead_first && mods.is_empty();
    let instead = match (&first.action, rest.as_slice()) {
        // "exile it instead", "instead exile it"
        (Action::Exile(objects), []) if marked(&first.mods) && matches!(objects.items.as_slice(), [o] if it(o)) => Instead::Exile,
        // "reveal this card and shuffle it into its owner's library instead"
        (Action::Reveal(objects), [(_, Act { action: Action::ShuffleInto { what, whose: Whose::Owner, from: None }, mods, may: false })])
            if this && first.mods.is_empty() && marked(mods) && matches!(objects.items.as_slice(), [o] if o.quant == Quant::Definite(Ref::This))
            && matches!(what.items.as_slice(), [o] if it(o)) => Instead::Shuffle,
        _ => return refused("what it does instead"),
    };
    // "and you gain 2 life": also done in the modified event, by the replacement's controller
    let gain = match body.rest.as_slice() {
        [] => None,
        [(_, Clause { subject: Some(Subject::Selector(you)), first: Act { action: Action::GainLife(Amount::Const(n)), mods, may: false }, .. })]
            if *you == Selector::you() && mods.is_empty() && r.instead_first && instead == Instead::Exile => Some(*n),
        _ => return refused("what else it does instead"),
    };
    if this && (dies || instead != Instead::Shuffle) { return refused("its own death or exile (the library form is DeathToLibrary)"); }
    let moved = (|| Ok(match (this, dies, event.actions.as_slice()) {
        (true, _, _) => Moved::Own,
        (false, true, _) => Moved::Dies(expr::Set::compile(&who.set, support, &mut linked(support))?),
        (false, false, [EventAction::PutIntoGraveyard { into, .. }]) => {
            let any = into.set == ObjectSet::of(Domain::Player) && into.quant == Quant::Indefinite(ir::Count::Exactly(Amount::Const(1)));
            // "a card or token": either
            let what = match &who.set.domain {
                Domain::Union(arms) if who.set == ObjectSet::of(who.set.domain.clone()) =>
                    arms.iter().map(|arm| Noun::compile(&Selector::new(who.quant.clone(), arm.clone()), support)).collect::<Res<Vec<_>>>()?,
                _ => vec![Noun::compile(who, support)?],
            };
            Moved::IntoGraveyard {
                what,
                into: if any { None } else { Some(Noun::compile(into, support)?) },
            }
        }
        _ => unreachable!("the event was matched above"),
    }))();
    Some(moved.map(|moved| Replacement::Elsewhere { moved, instead, gain, then }))
}

/// Discard replaces only this card's destination, not the discard itself. The
/// currently supported cause is a spell or ability, optionally qualified by controller.
/// More specific causes and other replacement bodies retain their admission refusal.
fn discard_to_battlefield(a: &Ability) -> Option<Res<Option<expr::Controller>>> {
    use ir::{Domain, EventAction, ObjectSet, Role, Verb, Whose};
    use ir::statics::{Static, Substitute, Elsewhere};
    let Ability::Text(Effect(sentences)) = a else { return None };
    let [Sentence::Static(statement)] = sentences.as_slice() else { return None };
    let Static::Replacement(r) = statement.as_ref() else { return None };
    let event = r.single_event()?;
    if r.next_time || r.this_turn || r.while_.is_some() || r.instead_first
        || event.counted() || event.during.is_some() || event.condition.is_some()
        || event.subjects.as_slice() != [Selector::you()] { return None; }
    let [EventAction::Deed(deed)] = event.actions.as_slice() else { return None };
    let [(Role::What, ir::event::Arg::Noun(discarded))] = deed.args.as_slice() else { return None };
    if deed.verb != Verb::Discard || deed.passive
        || *discarded != Selector::new(Quant::Definite(Ref::This), ObjectSet::of(Domain::Permanent)) { return None; }
    let ir::EventCause::Source(cause) = event.cause.as_ref()? else { return None; };
    let mut plain = cause.set.clone();
    plain.controller = None;
    if plain != ObjectSet::of(Domain::Union(vec![ObjectSet::of(Domain::Spell), ObjectSet::of(Domain::Ability)]))
        || cause.quant != Quant::Indefinite(ir::Count::Exactly(Amount::Const(1)))
        || cause.chooser.is_some() || cause.random.is_some() { return None; }
    let Substitute::Body(body) = &r.with else { return None };
    let Clause { subject: None, may: false, also: false, first, rest } = &body.first else { return None };
    if !rest.is_empty() || !body.rest.is_empty() || first.may
        || first.mods.as_slice() != [Modifier::InsteadOf(Elsewhere::Graveyard(Some(Whose::You)))] { return None; }
    let Action::Move { moves, from: None, .. } = &first.action else { return None };
    let [(objects, ir::Destination::Battlefield(entry))] = moves.as_slice() else { return None };
    let [to] = objects.items.as_slice() else { return None };
    if !entry.is_empty() || to.quant != Quant::Definite(Ref::Anaphor(Anaphor::It))
        || to.set != ObjectSet::of(Domain::Object) || to.chooser.is_some() || to.random.is_some() { return None; }
    Some(cause.set.controller.as_ref().map(expr::Controller::compile).transpose())
}

/// One unconditional individual draw replaced by a fixed number of draws (121.2, 614.1a).
/// The body must draw for the same player; delayed, conditional and compound bodies remain
/// explicit admission refusals, rather than partially executing their instructions.
fn draw_replacement(a: &Ability) -> Option<(&Selector, u32)> {
    use ir::statics::{Static, Substitute};
    use ir::{Count, Domain, EventAction, ObjectSet, Verb};
    let Ability::Text(Effect(sentences)) = a else { return None };
    let [Sentence::Static(statement)] = sentences.as_slice() else { return None };
    let Static::Replacement(r) = statement.as_ref() else { return None };
    let event = r.single_event()?;
    if r.next_time || r.this_turn || r.while_.is_some()
        || event.counted() || event.during.is_some()
        || event.nth_each_turn.is_some() || event.except_nth_in_step.is_some()
        || event.condition.is_some() || event.cause.is_some() { return None; }
    let card = Selector::new(Quant::Indefinite(Count::Exactly(Amount::Const(1))), ObjectSet::of(Domain::Card));
    if event.actions.as_slice() != [EventAction::does(Verb::Draw, card)] { return None; }
    let [who] = event.subjects.as_slice() else { return None };
    // An imperative body instructs "you". Broader player references need their own binding.
    if *who != Selector::you() { return None; }
    let Substitute::Body(body) = &r.with else { return None };
    let Clause { subject, may: false, also: false, first, rest } = &body.first else { return None };
    if subject.as_ref().is_some_and(|to| *to != Subject::Selector(who.clone()))
        || !rest.is_empty() || !body.rest.is_empty() || first.may { return None; }
    if !(first.mods.as_slice() == [Modifier::Instead] || r.instead_first && first.mods.is_empty()) { return None; }
    let Action::Draw(Amount::Const(count)) = first.action else { return None };
    Some((who, count))
}

/// An unconditional scaling of one player's life gain, keeping the recipient (119.10): read
/// as the definition is lowered. Only multipliers are admitted here:
/// they commute, so applying each once needs no replacement-order choice (614.5, 616.1).
/// Thresholds, optional effects, redirection, addition and temporary effects need the
/// general replacement-event/choice model and remain unsupported.
fn life_gain_multiplier(a: &Ability) -> Option<(&Selector, u32)> {
    use ir::statics::{Static, Substitute};
    use ir::{Domain, EventAction, Filter};
    let Ability::Text(Effect(sentences)) = a else { return None };
    let [Sentence::Static(statement)] = sentences.as_slice() else { return None };
    let Static::Replacement(r) = statement.as_ref() else { return None };
    let event = r.single_event()?;
    if r.next_time || r.this_turn || r.while_.is_some()
        || event.counted() || event.during.is_some()
        || event.condition.is_some() || event.cause.is_some()
        || event.actions.as_slice() != [EventAction::GainsLife { amount: None }] { return None; }
    let [who] = event.subjects.as_slice() else { return None };
    if who.set.domain != Domain::Player { return None; }
    let Substitute::Body(body) = &r.with else { return None };
    let Clause { subject: Some(Subject::Selector(to)), may: false, also: false, first, rest } = &body.first else { return None };
    if !rest.is_empty() || !body.rest.is_empty() || first.may { return None; }
    // "instead" may precede the clause or modify its sole act, never qualify another act.
    if !(first.mods.as_slice() == [Modifier::Instead] || r.instead_first && first.mods.is_empty()) { return None; }
    let same_you = who.quant == Quant::Definite(Ref::You) && to == who;
    let that_player = to.quant == Quant::Definite(Ref::Anaphor(Anaphor::That))
        && to.set.domain == Domain::Player && to.set.filter == Filter::All(Vec::new())
        && to.set.controller.is_none() && to.set.owner.is_none() && to.set.caster.is_none()
        && to.set.zone.is_none() && !to.set.other && to.chooser.is_none() && to.random.is_none();
    if !same_you && !that_player { return None; }
    let Action::GainLife(amount) = &first.action else { return None };
    fn factor(n: &Amount) -> Option<u32> {
        match n {
            Amount::That { many: false } => Some(1),
            Amount::Multiple(k, n) => factor(n)?.checked_mul(*k),
            _ => None,
        }
    }
    Some((who, factor(amount)?))
}

/// A named subset or all kinds present in the proposed holder inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CounterScope {
    Named(String),
    EachProposed,
}
impl CounterScope {
    pub(crate) fn includes(&self, kind: &str) -> bool {
        match self { Self::Named(named) => named == kind, Self::EachProposed => true }
    }
}

/// Constant addition or multiplication of a proposed counter/damage amount (614.1a).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CounterChange {
    /// "that many plus one"
    Plus(u32),
    /// "twice that many"
    Times(u32),
}

impl CounterChange {
    pub fn apply(self, count: u32) -> crate::Res<u32> {
        match self {
            CounterChange::Plus(n) => count.checked_add(n),
            CounterChange::Times(n) => count.checked_mul(n),
        }.ok_or_else(|| crate::Unsupported("replacement amount exceeds the engine numerical range".into()))
    }
}

/// "If one or more [kind] counters would be put on [permanents], [changed number] [kind]
/// counters are put on it instead."
struct CounterReplacement<'a> {
    on: &'a Selector,
    by: Option<&'a Selector>,
    effect_only: bool,
    kinds: CounterScope,
    change: CounterChange,
}

/// The proposal keeps its holder, placing player and effect cause independently (614.16).
fn counter_replacement(a: &Ability) -> Option<CounterReplacement<'_>> {
    use ir::statics::{Static, Substitute, CounterKinds, CounterVoice};
    use ir::{EventAction, EventCause, Filter};
    let Ability::Text(Effect(sentences)) = a else { return None };
    let [Sentence::Static(statement)] = sentences.as_slice() else { return None };
    let Static::Replacement(r) = statement.as_ref() else { return None };
    let event = r.single_event()?;
    if r.next_time || r.this_turn || r.instead_first || r.while_.is_some()
        || event.counted() || event.during.is_some() || event.condition.is_some() { return None; }
    let (on, by, effect_only, event_kind, voice) = match (event.subjects.as_slice(), event.actions.as_slice(), &event.cause) {
        ([on], [EventAction::CountersPut { kind, one_or_more: true, nth: None }], None) =>
            (on, None, false, kind.as_ref(), CounterVoice::Passive),
        ([by], [EventAction::PutsCounters { kind, one_or_more: true, on }], None) =>
            (on.as_ref(), Some(by), false, kind.as_ref(), CounterVoice::Imperative),
        ([], [EventAction::PutsCounters { kind, one_or_more: true, on }], Some(EventCause::Effect)) =>
            (on.as_ref(), None, true, kind.as_ref(), CounterVoice::Cause),
        _ => return None,
    };
    if !matches!(on.quant, Quant::Indefinite(_)) { return None; }
    let Substitute::CountersPut { count, kinds, on: to, voice: with_voice } = &r.with else { return None };
    if *with_voice != voice { return None; }
    let same = matches!(to.quant, Quant::Definite(Ref::Anaphor(Anaphor::It | Anaphor::That)))
        && to.set.controller.is_none() && to.set.owner.is_none() && to.set.caster.is_none()
        && to.set.zone.is_none() && !to.set.other && to.chooser.is_none() && to.random.is_none()
        && (to.set.filter == Filter::All(Vec::new()) || to.set.filter == on.set.filter);
    let legal_domains = |s: &Selector| s.set.possibilities().iter().all(|p|
        p.domain.sort.is_a(ir::Sort::Permanent) || p.domain.sort==ir::Sort::Player);
    let same_domain=to.set.domain==on.set.domain
        || to.set.domain==ir::Domain::Object && on.set.domain==ir::Domain::Permanent;
    if !same || !same_domain || !legal_domains(on) { return None; }
    let kinds = match (kinds, event_kind) {
        (CounterKinds::Named(put), Some(kind)) if put == kind => CounterScope::Named(kind.0.clone()),
        (CounterKinds::EachProposed, None) => CounterScope::EachProposed,
        _ => return None,
    };
    let that = |n: &Amount| matches!(n, Amount::That { many: true });
    let change = match count {
        Amount::Plus(a, b) if that(a) => match b.as_ref() {
            Amount::Const(n) => CounterChange::Plus(*n), _ => return None,
        },
        Amount::Multiple(k, n) if that(n) => CounterChange::Times(*k),
        _ => return None,
    };
    Some(CounterReplacement { on, by, effect_only, kinds, change })
}

impl Game {
    /// Apply each functioning life-gain multiplier once, before the modified gain occurs.
    /// The caller first excludes zero events and prohibited gains (119.7, 119.10).
    pub(crate) fn replace_life_gain(&self, player: PlayerId, mut amount: u32) -> Res<u32> {
        for source in &self.battlefield {
            let ctx = Ctx::new(self.controller(*source), *source);
            for (key, replacement) in self.replacements(*source) {
                if let Replacement::LifeGain { who, factor } = replacement
                    && who.is(self, &ctx.clone().with_key(key), Target::Player(player))
                {
                    amount = amount.checked_mul(*factor)
                        .ok_or_else(|| crate::Unsupported("life gain amount exceeds the engine numerical range".into()))?;
                }
            }
        }
        Ok(amount)
    }

    /// What the replacement effects in force make of how this permanent enters, other than
    /// those already applied to the event (614.5): whether it enters tapped, and the counters
    /// it enters with. Asked of a game in which it is seen as it would exist on the
    /// battlefield (614.12): its own abilities that are about it alone, and those of the
    /// permanents on the battlefield that are about permanents they describe.
    pub(crate) fn arrival_replacements(&self, on: ObjId, applied: &[ReplacementId]) -> Res<Vec<(ReplacementId, bool, bool, Vec<(String, u32)>)>> {
        let mut found = Vec::new();
        for source in std::iter::once(on).chain(self.battlefield.iter().copied()) {
            let own = source == on;
            for (key, replacement) in self.replacements(source) {
                let id = ReplacementId::from_ability(source, key.clone(), 0);
                if let Replacement::Sunburst(parts) = replacement {
                    if !own { continue; }
                    // Only a resolving spell transfers paid mana to this prospective object.
                    // Its copied/base type, ignoring layer-4 effects, determines the counter.
                    let object = self.obj(on);
                    let count = object.mana_paid.colors.iter().filter(|n| **n > 0).count() as u32;
                    if count == 0 { continue; }
                    let kind = if object.chars.types.contains(&ir::CardType::Creature) { "+1/+1" } else { "charge" };
                    for part in parts {
                        let id = ReplacementId::from_ability(source, key.clone(), *part);
                        if !applied.contains(&id) { found.push((id, false, false, vec![(kind.to_string(), count)])); }
                    }
                    continue;
                }
                let Replacement::Arrives { subject, acts } = replacement else { continue };
                let about = if own { *subject == Said::Source } else { matches!(subject, Said::Described(_)) };
                if applied.contains(&id) || !about { continue; }
                let ctx = Ctx::new(self.controller(source), source).with_key(key);
                if !subject.contains(self, &ctx, Target::Object(on)) { continue; }
                let mut delta = Entry::default();
                let mut applies = false;
                for (when, arrivals) in acts {
                    if !when.holds(self, &ctx) { continue; }
                    for arrival in arrivals {
                        arrival.checked_apply(self, &ctx, &mut delta)?;
                        applies = true;
                    }
                }
                if applies { found.push((id, delta.tapped, delta.enters_prepared, delta.counters)); }
            }
        }
        Ok(found)
    }
}

/// How a permanent enters the battlefield, the replacement effects that modify it applied
/// (614.12).
#[derive(Debug, Clone, Default)]
pub(crate) struct Entry {
    /// A reserved, non-zone object whose identity is adopted only when entry commits.
    pub prepared: Option<ObjId>,
    pub enters_prepared: bool,
    pub attached_to: Option<Target>,
    pub attacking: Option<Target>,
    pub tapped: bool,
    /// The counters it enters with, by kind.
    pub counters: Vec<(String, u32)>,
    /// Production entry has frozen all totals before any payment or movement commits.
    pub(crate) frozen_counters: Option<crate::operation::EntryInventory>,
    /// Values chosen before entry, with producer and chooser (614.12a, 607.2d).
    pub(crate) values: Vec<Record>,
    /// What permanents already on the battlefield say of how permanents they describe enter:
    /// whose statement it is, of what, and what it says. Whether the one entering is
    /// described is asked of it as it would exist on the battlefield (614.12).
    others: Vec<(Ctx<'static>, Said, Vec<Enters>)>,
}

impl Entry {
    pub(crate) fn freeze_counters(&mut self, inventory: crate::operation::EntryInventory) -> Res<()> {
        if !self.others.is_empty() {
            return unsupported("unprepared conditional entry counters");
        }
        self.counters.clear();
        self.frozen_counters = Some(inventory);
        Ok(())
    }
}

/// What an act says of how its subject enters, when that is what it says.
fn arrivals(a: &Action) -> Option<&[Arrival]> {
    match a {
        Action::Static(s) => match s.as_ref() {
            StaticAct::Enters(arrivals) => Some(arrivals),
            _ => None,
        },
        _ => None,
    }
}

/// The statement a static ability is, when it says how its subject enters the battlefield:
/// read as the definition is lowered.
fn statement(a: &Ability) -> Option<Statement<'_>> {
    crate::statement::read(a).filter(|s| s.acts.iter().all(|(action, _, scale)| arrivals(action).is_some() && matches!(scale, Scale::Once | Scale::ForEach(_))))
}

/// A permanent about to enter the battlefield whose controller is making, before it does
/// (614.12a), the choice its text asks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Entering {
    pub card: ObjId,
    pub player: PlayerId,
    pub asks: Asks,
}

/// The choice a permanent's text asks as it enters (614.1c, 614.12a), as the game asks it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Asks {
    /// Whether to pay this much life, and how it enters if they don't.
    Life { life: u32, otherwise: Vec<Enters> },
    Reveal { among: Vec<ObjId>, private: bool, otherwise: Vec<Enters> },
    /// Which of these cards of the hand and permanents to behold (701.4a), if any, and how it
    /// enters if none.
    Behold { among: Vec<ObjId>, private: bool, otherwise: Vec<Enters> },
    Copy { effect: crate::copy::Entry, among: Vec<ObjId> },
    /// A typed choice for this producer (105.1, 205.3m, 607.2d); `then`: what the same
    /// replacement asks next, once this is answered.
    Value { kind: crate::binding::Domain, link: LinkId, then: Option<Box<Asks>> },
}

impl Asks {
    /// The controller's entry question, or no question when its cost cannot be paid
    /// alongside earlier commitments (119.4, 614.12b).
    pub fn choice(&self, game: &Game, player: PlayerId, payments: &EntryPayments) -> Option<Choice> {
        match self {
            Asks::Copy { effect, among } => Some(Choice::Select { among: among.clone(), min: usize::from(!effect.optional && !among.is_empty()), max: usize::from(!among.is_empty()) }),
            Asks::Life { life, .. } => payments.can_reserve(game, player, *life).then_some(Choice::May { possible: true }),
            Asks::Reveal { among, private, .. } | Asks::Behold { among, private, .. } =>
                (*private || !among.is_empty()).then(|| Choice::Select { among: among.clone(), min: 0, max: 1 }),
            Asks::Value { kind, .. } if kind.kind == Kind::Player => Some(Choice::Player { among: players(game, player, *kind) }),
            Asks::Value { kind, .. } => Some(kind.choice()),
        }
    }

    /// Does the same ability ask again once this is answered?
    pub fn more(&self) -> bool { matches!(self, Asks::Value { then: Some(_), .. }) }

    /// Record an answer without entering or paying yet. None is the automatic nonpayment
    /// when no payment can be offered; typed values retain their producer and chooser.
    pub fn apply(&self, game: &Game, player: PlayerId, answer: Option<Answer>, chosen: &mut Choices, payments: &mut EntryPayments) -> Res<()> {
        if chosen.then.as_ref() == Some(self) { chosen.then = None; }
        match (self, answer) {
            (Asks::Copy { effect, among }, Some(Answer::Objects(objects))) if objects.len() <= 1
                && objects.iter().all(|o| among.contains(o)) && (effect.optional || !objects.is_empty() || among.is_empty()) => {
                chosen.copies_applied.push(effect.clone());
                if let Some(source) = objects.first() {
                    chosen.copy_acquisition = Some(std::sync::Arc::new(crate::copy::Acquisition {donor:*source, before:game.copiable(*source),producer:effect.producer.clone()}));
                    chosen.copied = Some(effect.values(game, *source));
                }
            },
            (Asks::Life { life, .. }, Some(Answer::Yes)) if payments.can_reserve(game, player, *life) => {
                payments.reserve(game, player, *life);
            }
            (Asks::Life { otherwise, .. }, Some(Answer::No)) => chosen.arrivals.extend(otherwise.clone()),
            (Asks::Life { life, otherwise, .. }, None) if !payments.can_reserve(game, player, *life) => {
                chosen.arrivals.extend(otherwise.clone());
            }
            (Asks::Reveal { among, otherwise, .. }, Some(Answer::Objects(objects))) if objects.len() <= 1
                && objects.iter().all(|o| among.contains(o)) => {
                if let Some(card) = objects.first() { payments.reveal(player, *card); }
                else { chosen.arrivals.extend(otherwise.clone()); }
            }
            (Asks::Reveal { among, private: false, otherwise }, None) if among.is_empty() => chosen.arrivals.extend(otherwise.clone()),
            (Asks::Behold { among, otherwise, .. }, Some(Answer::Objects(objects))) if objects.len() <= 1
                && objects.iter().all(|o| among.contains(o)) => {
                if let Some(beheld) = objects.first() { payments.behold(player, *beheld); }
                else { chosen.arrivals.extend(otherwise.clone()); }
            }
            (Asks::Behold { among, private: false, otherwise }, None) if among.is_empty() => chosen.arrivals.extend(otherwise.clone()),
            (Asks::Value { kind, link, then }, Some(Answer::Value(value))) if crate::binding::accepts(*kind, &value) => {
                chosen.values.push(Record { link: link.clone(), chooser: player, value });
                chosen.then = then.as_deref().cloned();
            }
            (Asks::Value { kind, link, then }, Some(Answer::Player(chosen_player)))
                if kind.kind == Kind::Player && players(game, player, *kind).contains(&chosen_player) => {
                chosen.values.push(Record { link: link.clone(), chooser: player, value: crate::binding::Value::Player(chosen_player) });
                chosen.then = then.as_deref().cloned();
            }
            _ => return unsupported("a legal answer to the entry choice is required (614.12a-b)"),
        }
        Ok(())
    }
}

/// The players a choice of a player as a permanent enters may pick: those still in the game
/// (800.4a), of "an opponent" the chooser's opponents alone.
fn players(game: &Game, chooser: PlayerId, domain: crate::binding::Domain) -> Vec<PlayerId> {
    (0..game.players.len()).filter(|p| !game.players[*p].lost && !(domain.opponent && *p == chooser)).collect()
}

/// Costs committed while collecting simultaneous entry choices (614.12b). Commitments
/// are per player; no payment occurs until every choice is collected. Keep individual
/// payments so their life-loss/reveal events are not merged into a different event.
#[derive(Debug, Clone, Default)]
pub(crate) struct EntryPayments(Vec<EntryPayment>);
#[derive(Debug, Clone)]
enum EntryPayment { Life(PlayerId, u32), Reveal(PlayerId, ObjId), Behold(PlayerId, ObjId) }

impl EntryPayments {
    pub fn can_reserve(&self, game: &Game, player: PlayerId, life: u32) -> bool {
        self.0.iter().filter_map(|p| match p { EntryPayment::Life(p,n) if *p == player => Some(*n), _ => None })
            .try_fold(life, |sum, n| sum.checked_add(n))
            .is_some_and(|total| game.can_pay_life(player, total))
    }

    pub fn reserve(&mut self, game: &Game, player: PlayerId, life: u32) {
        assert!(self.can_reserve(game, player, life), "entry choices must have payable combined costs (614.12b)");
        if life != 0 { self.0.push(EntryPayment::Life(player, life)); }
    }

    /// Revealing does not consume a card: one card may pay several simultaneous
    /// entry reveals (701.20b-c). Disclosures happen at commitment, before entry events.
    pub fn reveal(&mut self, player: PlayerId, card: ObjId) { self.0.push(EntryPayment::Reveal(player, card)); }

    /// Beholding consumes nothing either (701.4a): one card or permanent may be beheld for
    /// several simultaneous entries.
    pub fn behold(&mut self, player: PlayerId, object: ObjId) { self.0.push(EntryPayment::Behold(player, object)); }

    pub fn pay(self, game: &mut Game) -> Res<()> {
        for payment in self.0 {
            match payment {
                EntryPayment::Life(player, life) => assert!(game.pay_life(player, life)?, "a committed entry payment must remain payable (614.12b)"),
                EntryPayment::Reveal(player, card) => { game.reveal(player, vec![card]); }
                EntryPayment::Behold(player, object) => game.behold(player, vec![object]),
            }
        }
        Ok(())
    }
}

/// What was chosen for a permanent before it entered the battlefield (614.12a), as it then
/// enters: conditional arrivals and typed choice records.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Choices {
    /// Enter with the back face up (712.14a); this is not a transformation event.
    pub transformed: bool,
    /// Enter face down this way (708.3, 701.40a): with the face-down characteristics from
    /// the start, so none of its own abilities applies as it enters.
    pub face_down: Option<crate::face::Concealment>,
    pub copy_acquisition: Option<std::sync::Arc<crate::copy::Acquisition>>,
    pub copied: Option<crate::copy::Values>,
    pub copies_applied: Vec<crate::copy::Entry>,
    pub attached_to: Option<Target>,
    pub attacking: Option<Target>,
    pub choose_defender: bool,
    /// Of what its controller chooses it attacks, what the effect that made it allows (508.4).
    pub defending: Option<crate::creation::Defending>,
    pub arrivals: Vec<Enters>,
    pub values: Vec<Record>,
    /// What a replacement being applied asks next (614.12a): asked of it as it began, not
    /// of the permanent as its earlier answers make it.
    pub then: Option<Asks>,
}

impl Choices {
    pub fn arriving(arrivals: Vec<Enters>) -> Choices {
        Choices { arrivals, ..Choices::default() }
    }
}

/// A choice a permanent's own text has its controller make as it enters, before it is on the
/// battlefield (614.1c, 614.12a), as its text says it: what [`Asked`] is lowered from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsEnters<'a> {
    /// "As this land enters, you may pay 2 life. If you don't, it enters tapped.": the life
    /// its controller may pay, and how it enters when they don't.
    Life(&'a Amount, &'a [Arrival]),
    Reveal(&'a Selector, &'a [Arrival]),
    /// "As this land enters, you may behold a Jace. If you don't, this land enters tapped."
    /// (701.4a): what may be beheld, and how it enters when nothing is.
    Behold(&'a Selector, &'a [Arrival]),
    /// A single typed value, referred to by linked abilities (607.2d).
    Value(crate::binding::Domain),
    /// "As this land enters, choose a basic land type. Then you may pay 2 life. If you
    /// don't, it enters tapped.": the value, then the life and how it enters without it.
    ValueThenLife(crate::binding::Domain, &'a Amount, &'a [Arrival]),
}

/// The choice this ability has its permanent's controller make as it enters, when that is
/// what it is.
pub fn as_enters(a: &Ability) -> Option<AsEnters<'_>> {
    let Ability::Text(Effect(sentences)) = a else { return None };
    let this = |s: &Selector| s.quant == Quant::Definite(Ref::This);
    match sentences.as_slice() {
        [Sentence::Led { lead: Lead::AsEnters(it), rest }] if this(it) => value_choice(rest).map(AsEnters::Value),
        // "As this land enters, choose a basic land type. Then you may pay 2 life. If you
        // don't, it enters tapped."
        [Sentence::Led { lead: Lead::AsEnters(it), rest },
         Sentence::Led { lead: Lead::Then, rest: offer },
         Sentence::Led { lead: Lead::Modifier(Modifier::If(unpaid)), rest: then }] if this(it) => {
            let kind = value_choice(rest)?;
            match offered(offer, unpaid, then)? {
                (Action::PayLife(n), enters) => Some(AsEnters::ValueThenLife(kind, n, enters)),
                _ => None,
            }
        }
        [Sentence::Led { lead: Lead::AsEnters(it), rest: offer }, Sentence::Led { lead: Lead::Modifier(Modifier::If(unpaid)), rest: then }] if this(it) => {
            match offered(offer, unpaid, then)? {
                (Action::PayLife(n), enters) => Some(AsEnters::Life(n, enters)),
                (Action::Reveal(objects), enters) if objects.items.len() == 1 => Some(AsEnters::Reveal(&objects.items[0], enters)),
                (Action::OnObjects(ir::action::ObjectAction::Behold, objects), enters) if objects.items.len() == 1 => Some(AsEnters::Behold(&objects.items[0], enters)),
                _ => None,
            }
        }
        _ => None,
    }
}

/// "choose a creature type": the one typed value a sentence has its subject's controller
/// choose, when that is all it says.
fn value_choice(rest: &Sentence) -> Option<crate::binding::Domain> {
    let Sentence::Do(body) = rest else { return None };
    let Clause { subject: None, may: false, also: false, first: Act { action: Action::Choose(choice), mods, may: false }, rest } = &body.first
    else { return None };
    if !mods.is_empty() || !rest.is_empty() || !body.rest.is_empty() { return None; }
    // "choose a player", "choose an opponent": one player, kept with the permanent (607.2d)
    if let ir::action::Choice::Objects(objects) = choice {
        let [one] = objects.items.as_slice() else { return None };
        if one.quant != Quant::Indefinite(ir::Count::Exactly(Amount::Const(1))) || one.chooser.is_some() || one.random.is_some() { return None; }
        let player = ir::ObjectSet::of(ir::Domain::Player);
        let opponent = ir::ObjectSet { filter: ir::Filter::All(vec![ir::Filter::Atom(ir::Atom::Opponent)]), ..player.clone() };
        return (one.set == player || one.set == opponent)
            .then(|| crate::binding::Domain { kind: Kind::Player, names: crate::binding::Names::ANY, opponent: one.set == opponent });
    }
    let ir::action::Choice::Value { kind, count: Amount::Const(1), random: false, .. } = choice else { return None };
    match kind {
        // a basic land type is chosen only as a permanent enters
        ir::action::ValueKind::BasicLandType => matches!(choice, ir::action::Choice::Value { except, .. } if except.is_empty()).then(|| Kind::BasicLandType.into()),
        _ => crate::program::scalar_choice(choice).ok(),
    }
}

/// "you may pay 2 life. If you don't, it enters tapped.": what you may do, and how it enters
/// when you don't. The optional act is part of the entry replacement, not a resolving spell.
fn offered<'a>(offer: &'a Sentence, unpaid: &'a Condition, then: &'a Sentence) -> Option<(&'a Action, &'a [Arrival])> {
    let (Sentence::Do(offer), Sentence::Do(then)) = (offer, then) else { return None };
    let you = |s: &Selector| s.quant == Quant::Definite(Ref::You);
    let action = match &offer.first {
        Clause { subject: Some(Subject::Selector(who)), may: true, also: false,
            first: Act { action, mods, may: false }, rest }
            if you(who) && mods.is_empty() && rest.is_empty() && offer.rest.is_empty() => action,
        _ => return None,
    };
    // "If you don't, it enters tapped."
    let Condition::Did { who, outcome: ir::condition::Outcome::Didnt } = unpaid else { return None };
    if !you(who) { return None; }
    let enters = match &then.first {
        Clause { subject: Some(Subject::Selector(it)), may: false, first: Act { action, mods, may: false }, rest, .. }
            if matches!(it.quant, Quant::Definite(Ref::This | Ref::Anaphor(Anaphor::It))) && mods.is_empty() && rest.is_empty() && then.rest.is_empty() =>
        {
            arrivals(action)?
        }
        _ => return None,
    };
    Some((action, enters))
}

/// "As this creature is turned face up, put five +1/+1 counters on it." (708.11): the
/// counters, of each kind how many. None: the text does not open with "as ~ is turned face
/// up"; refused: it does something else then.
fn as_turned_face_up(a: &Ability, support: &Support<'_>) -> Option<Res<Vec<(String, expr::Amount)>>> {
    let Ability::Text(Effect(sentences)) = a else { return None };
    if !matches!(sentences.first(), Some(Sentence::Led { lead: Lead::AsTurnedFaceUp(_), .. })) { return None; }
    let refused = || unsupported("as this is turned face up: only putting counters on it (708.11)");
    let this = |s: &Selector| matches!(s.quant, Quant::Definite(Ref::This | Ref::Anaphor(Anaphor::It)));
    let [Sentence::Led { lead: Lead::AsTurnedFaceUp(what), rest }] = sentences.as_slice() else { return Some(refused()) };
    let Sentence::Do(body) = rest.as_ref() else { return Some(refused()) };
    let Clause { subject: None, may: false, also: false, first: Act { action: Action::PutCounters(placements), mods, may: false }, rest } = &body.first
    else { return Some(refused()) };
    if !this(what) || !mods.is_empty() || !rest.is_empty() || !body.rest.is_empty() { return Some(refused()); }
    let mut out = Vec::new();
    for placement in placements {
        if !this(&placement.on) { return Some(refused()); }
        for counters in &placement.counters {
            if counters.up_to || counters.extra.is_some() { return Some(refused()); }
            if let Err(e) = crate::layers::counter_ok(&counters.kind.0) { return Some(Err(e)); }
            match expr::Amount::compile(&counters.count, support, &mut linked(support)) {
                Ok(n) => out.push((counters.kind.0.clone(), n)),
                Err(e) => return Some(Err(e)),
            }
        }
    }
    Some(Ok(out))
}

/// Does this text say an act is done as a permanent enters, after the act ("If it's neither day
/// nor night, it becomes day as this creature enters.", 614.1c)? The engine does not do acts
/// as a permanent enters, other than the choices of [`as_enters`].
pub fn acts_as_enters(a: &Ability) -> bool {
    fn said(s: &Sentence) -> bool {
        match s {
            Sentence::Do(b) => std::iter::once(&b.first)
                .chain(b.rest.iter().map(|(_, c)| c))
                .flat_map(|c| std::iter::once(&c.first).chain(c.rest.iter().map(|(_, a)| a)))
                .any(|a| a.mods.iter().any(|m| matches!(m, Modifier::AsEnters(_)))),
            Sentence::Led { rest, .. } => said(rest),
            _ => false,
        }
    }
    match a {
        Ability::Text(e) | Ability::Triggered { effect: e, .. } | Ability::Activated { effect: e, .. } | Ability::Chapter { effect: e, .. } => e.0.iter().any(said),
        _ => false,
    }
}

impl Game {
    /// The choice a card's text asks of the player under whose control it would enter the
    /// battlefield, before it does (614.12a): "As this land enters, you may pay 2 life. If
    /// you don't, it enters tapped.", "As this land enters, choose a creature type." A card
    /// whose text asks more than one is not run ([`crate::play::check`]).
    pub(crate) fn enters_asking(&self, card: ObjId, player: PlayerId, chosen: &Choices) -> Option<Entering> {
        if let Some(asks) = &chosen.then { return Some(Entering { card, player, asks: asks.clone() }); }
        let ctx = Ctx::new(player, card);
        self.replacements(card).find_map(|(key, replacement)| {
            let Replacement::AsEnters(asked) = replacement else { return None };
            let asks = match asked {
                Asked::Life { life, otherwise } => Asks::Life { life: life.evaluate(self, &ctx.clone().with_key(key), &NoLocals), otherwise: otherwise.clone() },
                // applied already, with its question after the value asked
                Asked::ValueThenLife { .. } if chosen.values.iter().any(|r| r.link == LinkId(key.clone())) => return None,
                Asked::ValueThenLife { kind, life, otherwise } => Asks::Value { kind: *kind, link: LinkId(key.clone()),
                    then: Some(Box::new(Asks::Life { life: life.evaluate(self, &ctx.clone().with_key(key), &NoLocals), otherwise: otherwise.clone() })) },
                Asked::Reveal { card: filter, otherwise } => Asks::Reveal {
                    private: self.concealed_pool(player, self.players[player].hand.iter().copied().filter(|o| *o != card)),
                    among: self.players[player].hand.iter().copied().filter(|on| *on != card
                        && filter.contains(self, &ctx.clone().with_key(key.clone()), &NoLocals, Target::Object(*on))
                        && self.takes(ir::Verb::Reveal, ir::Role::What, Target::Object(*on))).collect(),
                    otherwise: otherwise.clone(),
                },
                // the cards of the hand it is not, and the permanents already on the battlefield:
                // the one entering is not there before it enters (614.12a)
                Asked::Behold { cards, permanents, otherwise } => {
                    let ctx = ctx.clone().with_key(key.clone());
                    Asks::Behold {
                        private: self.concealed_pool(player, self.players[player].hand.iter().copied().filter(|o| *o != card)),
                        among: self.players[player].hand.iter().copied().filter(|on| *on != card && cards.contains(self, &ctx, &NoLocals, *on))
                            .chain(self.battlefield.iter().copied().filter(|on| self.controller(*on) == player && permanents.contains(self, &ctx, &NoLocals, Target::Object(*on))))
                            .filter(|on| self.can(ir::Verb::Behold, player, Target::Object(*on))).collect(),
                        otherwise: otherwise.clone(),
                    }
                }
                Asked::Value(kind) => Asks::Value { kind: *kind, link: LinkId(key), then: None },
            };
            Some(Entering { card, player, asks })
        })
    }

    /// The intrinsic ability of a planeswalker (306.5b): "This permanent enters with a number
    /// of loyalty counters on it equal to its printed loyalty number." A replacement effect
    /// like any other "enters with": the counters are part of the one entry event.
    pub(crate) fn printed_loyalty(&self, object: ObjId) -> Option<(String, u32)> {
        let loyalty = self.def(object)?.loyalty?;
        self.obj(object).chars.types.contains(&ir::CardType::Planeswalker).then(|| ("loyalty".to_string(), loyalty))
    }

    /// [`Game::entry`], and what a choice made before the permanent enters has added
    /// (`chosen`: "If you don't, it enters tapped.", the creature type chosen).
    /// What do the replacement effects in force say of a permanent that would enter the
    /// battlefield under this player's control (614.12)? Asked before it moves, so a
    /// condition is true or not of the game without it ("unless you control two or more
    /// other lands"): the static abilities of the object itself that are about it alone
    /// (None: a token, which has none), applied here, and those of the permanents on the
    /// battlefield that are about permanents they describe, kept for [`Game::arrive`]. An
    /// ability of the object itself about a kind of permanent is not among them: it is not
    /// on the battlefield yet.
    pub(crate) fn entry_as(&self, object:Option<ObjId>, controller:PlayerId, chosen:&Choices)->Entry {
        self.try_entry_as(object,controller,chosen).expect("bounded fixture entry counter expression")
    }
    pub(crate) fn try_entry_as(&self, object: Option<ObjId>, controller: PlayerId, chosen: &Choices) -> Res<Entry> {
        let mut entry = Entry::default();
        entry.attached_to = chosen.attached_to;
        entry.attacking = chosen.attacking;
        if let Some(object) = object {
            let ctx = Ctx::new(controller, object);
            for arrival in &chosen.arrivals {
                arrival.checked_apply(self,&ctx,&mut entry)?;
            }
            entry.values = chosen.values.clone();
            entry.counters.extend(self.printed_loyalty(object));
        }
        // (a permanent a game is set up with is on the battlefield already: asked once)
        for source in object.into_iter().chain(self.battlefield.iter().copied().filter(|b| Some(*b) != object)) {
            let own = Some(source) == object;
            let ctx = Ctx::new(if own { controller } else { self.controller(source) }, source);
            for (key, replacement) in self.replacements(source) {
                let ctx = ctx.clone().with_key(key);
                let Replacement::Arrives { subject, acts } = replacement else { continue };
                for (when, arrivals) in acts {
                    if !when.holds(self, &ctx) {
                        continue;
                    }
                    match (own, subject) {
                        (true, Said::Source) => {
                            for arrival in arrivals {
                                arrival.checked_apply(self,&ctx,&mut entry)?;
                            }
                        }
                        (false, Said::Described(_)) => entry.others.push((ctx.clone(), subject.clone(), arrivals.clone())),
                        _ => {}
                    }
                }
            }
        }
        Ok(entry)
    }

    /// The permanent has been put onto the battlefield and nothing has seen it yet: of what
    /// the permanents that were there say, what is about it as it now exists is applied
    /// (614.12), and its arrival modifications and linked choice records take effect.
    /// Returns the counters it enters with, which are put on it once it has entered (122.6).
    pub(crate) fn arrive(&mut self, permanent:ObjId, entry:Entry)->Vec<(String,u32)> {
        self.try_arrive(permanent,entry).expect("frozen runtime or bounded fixture entry counters")
    }
    pub(crate) fn try_arrive(&mut self, permanent: ObjId, mut entry: Entry) -> Res<Vec<(String, u32)>> {
        self.obj_mut(permanent).choices = std::mem::take(&mut entry.values);
        // Entry preparation already enforces the Aura-specific failure rule (303.4i).
        // An additional nonattachment token ignores an impossible attachment (701.3b).
        self.obj_mut(permanent).attached_to = entry.attached_to.filter(|to| self.can_attach(permanent,*to));
        for (ctx, subject, arrivals) in std::mem::take(&mut entry.others) {
            if subject.contains(self, &ctx, Target::Object(permanent)) {
                for arrival in &arrivals {
                    arrival.checked_apply(self, &ctx, &mut entry)?;
                }
            }
        }
        let o = self.obj_mut(permanent);
        o.tapped |= entry.tapped;
        if entry.enters_prepared { self.prepare(permanent); }
        Ok(entry.counters)
    }

    /// Setting a game up: a permanent put onto the battlefield is as it would have entered
    /// with what is there already, tapped and with its counters. Nothing happened: no event
    /// is logged.
    pub fn as_entered(&mut self, permanent: ObjId) {
        self.try_as_entered(permanent).expect("bounded fixture entry inventory");
    }
    /// Checked setup only; unlike runtime entry this emits no counter events.
    pub fn try_as_entered(&mut self, permanent: ObjId) -> crate::Res<()> {
        let before=self.clone();
        let controller=self.controller(permanent);
        let entry=self.try_entry_as(Some(permanent),controller,&Choices::default())?;
        let counters=match self.try_arrive(permanent,entry) {Ok(counters)=>counters,Err(reason)=>{self.adopt(before);return Err(reason);}};
        let mut final_inventory=self.obj(permanent).counters.clone();
        for (kind,count) in counters {
            let total=final_inventory.entry(kind).or_default();
            match crate::operation::counter_add(*total,count) {
                Ok(value)=>*total=value,
                Err(reason)=>{self.adopt(before);return Err(reason);}
            }
        }
        self.obj_mut(permanent).counters=final_inventory;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
