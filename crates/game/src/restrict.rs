//! Restrictions and requirements: effects that say something can't happen, which take
//! precedence over what allows it (CR 101.2), or that it happens if able. They come from static
//! abilities, in force while their permanent is on the battlefield (604.2), and from spells and
//! abilities that resolved, in force for as long as they say (611.2a).
//!
//! [`Rule`] lowers and validates each prohibition; every query consults that compiled rule,
//! whichever of the two sources it comes from. A static ability's rule is lowered
//! by that same compiler when its definition is loaded and kept with it
//! ([`crate::program::registration`]); no query here reads text.
//!
//! Combat declarations, targeting and proposed actions consult these rules at their legality
//! boundary. A restriction overrides a permission, including casting from another zone
//! (601.3) or putting a card onto the battlefield (614.17).

use crate::binding::{Scalar, Value};
use crate::event::Target;
use crate::program::expr::{self, Bind, Locals};
use crate::program::registration::{self, Restriction, Subject};
use crate::select::{Ctx, Support, name};
use crate::state::{Game, ObjId, PlayerId};
use crate::statement::{Affected, Scale, Statement, affects};
use crate::trigger::Fact;
use crate::{Res, unsupported};
use ir::action::UntapWhose;
use ir::effect::Modifier;
use ir::statics::{Forbidden, StaticAct, Within};
use ir::{Ability, Action, Amount, Count, Domain, EventAction, ObjectSet, Quant, Ref, Role, Selector, Verb};

/// What a noun phrase of a rule names: "this creature", "enchanted creature", "you", or
/// whichever thing fits a description ("a creature", "each player").
#[derive(Debug, Clone, PartialEq, Eq)]
enum Noun {
    This,
    Attached,
    You,
    In(expr::Set),
}

impl Noun {
    fn compile(s: &Selector, support: &Support<'_>, bind: Bind<'_>) -> Res<Noun> {
        if s.chooser.is_some() || s.random.is_some() {
            return unsupported("trigger: a chooser or a random pick");
        }
        match &s.quant {
            Quant::Definite(Ref::This) => Ok(Noun::This),
            Quant::Definite(Ref::You) => Ok(Noun::You),
            Quant::Definite(Ref::AttachedTo(ir::selector::Attachment::Enchanted | ir::selector::Attachment::Equipped)) => Ok(Noun::Attached),
            // "a creature", "each player": whichever one it is
            Quant::Indefinite(Count::Exactly(Amount::Const(1))) | Quant::Each | Quant::All => expr::Set::compile(&s.set, support, bind).map(Noun::In),
            q => unsupported(format!("trigger: \"{}\" in a pattern", name(q))),
        }
    }

    fn is(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals, t: Target) -> bool {
        match self {
            Noun::This => Some(t) == ctx.source.map(Target::Object),
            Noun::Attached => {
                ctx.source.is_some_and(|source| game.attached_to(source) == Some(t) || ctx.was.is_some_and(|o| t == Target::Object(o) && game.obj(source).attached_to == Some(t)))
            }
            Noun::You => t == Target::Player(ctx.controller),
            Noun::In(set) => set.contains(game, ctx, locals, t),
        }
    }

    fn scalars(&self, out: &mut Vec<Scalar>) {
        if let Noun::In(set) = self {
            out.extend(set.scalars());
        }
    }
}

/// The spells and abilities a thing can't be the target of: "spells", "spells or abilities
/// your opponents control", "blue spells".
#[derive(Debug, Clone, PartialEq, Eq)]
struct Sources {
    /// Spells alone, not abilities (115.1a–d).
    spells: bool,
    controller: Option<expr::Controller>,
    filter: expr::Filter,
}

/// What blocks: one creature the phrase names, or a number of them ("by more than one
/// creature", "except by three or more creatures").
#[derive(Debug, Clone, PartialEq, Eq)]
enum Blockers {
    One(Noun),
    AtLeast(usize, expr::Set),
}

/// An event asked of where it is made to happen ([`Game::forbidden`]): "can't gain life"
/// (119.7), "can't be destroyed" (702.12b), "can't be countered".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Happening {
    GainsLife,
    Destroyed,
    Countered,
    Exiled,
    Enters,
}

impl Happening {
    /// Whom the prohibition is said of in what would happen, when it is this: who would do
    /// it ("can't gain life"), or what it would be done to ("can't be destroyed").
    fn subject(self, would: &Fact) -> Option<Target> {
        let (verb, done_to) = match self {
            Happening::GainsLife => (Verb::GainLife, false),
            Happening::Destroyed => (Verb::Destroy, true),
            Happening::Countered => (Verb::Counter, true),
            Happening::Exiled => (Verb::Exile, true),
            Happening::Enters => (Verb::Enter, true),
        };
        if would.verb() != verb {
            return None;
        }
        // said of what it is done to alone, it is the event that happens once (509.3c),
        // not the one that says by whom
        if done_to { would.role(Role::What).filter(|_| would.role(Role::By).is_none()) } else { would.doer() }
    }
}

/// One thing a prohibition says can't happen.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Cant {
    /// "can't be the target of …"; none: of anything.
    BeTargeted(Option<Sources>),
    /// "can't attack", "can't attack you or planeswalkers you control": any of these; none:
    /// at all.
    Attack(Vec<Noun>),
    /// "can't block", "can't block creatures with flying".
    Block { blocked: Option<Noun>, except: bool },
    /// "can't be blocked", "by Walls" (any of these), "except by Walls" (by anything else).
    BeBlocked { by: Vec<Blockers>, except: bool },
    Happen(Happening),
    /// A passive recipient ban, optionally naming one counter kind.
    ReceiveCounters(Option<String>),
    /// Casting any spell, optionally from described source zones (601.3), optionally only
    /// spells with these characteristics, as the spell would have them (601.3e).
    /// `own`: the zones are the caster's own ("can't cast spells from anywhere other than their
    /// hands": each caster's hand, 601.2a), not any player's.
    Cast { from: Option<expr::Set>, except: bool, spell: Option<expr::Filter>, own: bool },
}

impl Cant {
    fn compile(item: &Forbidden, support: &Support<'_>, bind: Bind<'_>) -> Res<Cant> {
        if item.within.is_some() {
            return unsupported("prohibition: a window of its own");
        }
        // "by more than one creature": a number of blockers rather than one of them
        let blockers = |s: &Selector, bind: Bind<'_>| match &s.quant {
            Quant::Indefinite(Count::AtLeast(Amount::Const(n))) if s.chooser.is_none() && s.random.is_none() => {
                Ok(Blockers::AtLeast(*n as usize, expr::Set::compile(&s.set, support, bind)?))
            }
            _ => Noun::compile(s, support, bind).map(Blockers::One),
        };
        Ok(match &item.event {
            EventAction::BecomesTarget { of: None } => Cant::BeTargeted(None),
            EventAction::BecomesTarget { of: Some(of) } => {
                Noun::compile(of, support, &mut *bind)?;
                let mut description = of.set.clone();
                description.controller = None;
                // A spell's characteristics are shared selector predicates (109.3).
                // Qualities of abilities or their sources need their own distinction;
                // admitting a qualified stack-object union would conflate the two.
                if description.domain != Domain::Spell && description != ObjectSet::of(Domain::Union(vec![ObjectSet::of(Domain::Spell), ObjectSet::of(Domain::Ability)])) {
                    return unsupported("target prohibition: qualities of abilities or their sources");
                }
                if of.set.other || of.set.zone.is_some() || of.set.owner.is_some() || of.set.caster.is_some() {
                    return unsupported("target prohibition: another source, owner, caster or an explicit zone");
                }
                Cant::BeTargeted(Some(Sources {
                    spells: of.set.domain == Domain::Spell,
                    controller: of.set.controller.as_ref().map(expr::Controller::compile).transpose()?,
                    filter: expr::Filter::compile(&of.set.filter, support, bind)?,
                }))
            }
            EventAction::Attacks { alone: false, defender, with: None } => {
                Cant::Attack(defender.iter().map(|d| Noun::compile(d, support, &mut *bind)).collect::<Res<_>>()?)
            }
            // "Creatures with power less than this creature's power can't block it.": what the
            // blockers are said of is the blocked, which "it" names, this permanent
            EventAction::Blocks { blocked: Some(b) } if b.quant == Quant::Definite(Ref::Anaphor(ir::Anaphor::It)) && b.set.filter == ir::Filter::All(Vec::new()) =>
                Cant::Block { blocked: Some(Noun::This), except: false },
            EventAction::Blocks { blocked } => Cant::Block { blocked: blocked.as_ref().map(|b| Noun::compile(b, support, bind)).transpose()?, except: false },
            EventAction::BecomesBlocked { by, except } => {
                Cant::BeBlocked { by: by.iter().map(|s| blockers(s, &mut *bind)).collect::<Res<_>>()?, except: *except }
            }
            EventAction::Deed(d) if !d.passive && d.args.is_empty() && d.verb == Verb::Enter => Cant::Happen(Happening::Enters),
            EventAction::Deed(d) if !d.passive && d.verb == Verb::Cast => {
                let Some(what) = d.noun(Role::What) else { return unsupported("cast prohibition: no spell description"); };
                Noun::compile(what, support, &mut *bind)?;
                // the spell's own characteristics, as the announced face and choices give
                // them (601.3e); not who controls or owns it, nor where it is
                if what.set != (ObjectSet { filter: what.set.filter.clone(), ..ObjectSet::of(Domain::Spell) })
                    || d.args.len() != 1 + usize::from(d.zone(Role::From).is_some()) {
                    return unsupported("cast prohibition: spell qualities or other event roles");
                }
                let spell = (what.set.filter != ir::Filter::All(vec![])).then(|| expr::Filter::compile(&what.set.filter, support, &mut *bind)).transpose()?;
                let from = d.zone(Role::From);
                // "their hands" after a subject of players: the pronoun is the subject's, so each
                // one's own hand; the zones are of any player, and the card is its caster's
                let they = |o: &Selector| o.quant == Quant::Definite(Ref::Anaphor(ir::selector::Anaphor::They))
                    && o.set == ObjectSet::of(Domain::Player) && o.chooser.is_none() && o.random.is_none();
                let own = from.is_some_and(|f| f.zone.owner.as_deref().is_some_and(they));
                let set = from.map(|from| {
                    let mut zone = from.zone.clone();
                    if own { zone.owner = Some(Box::new(Selector::new(Quant::Each, ObjectSet::of(Domain::Player)))); }
                    ObjectSet { zone: Some(zone), ..ObjectSet::of(Domain::Card) }
                });
                Cant::Cast { from: set.as_ref().map(|s| expr::Set::compile(s, support, bind)).transpose()?,
                    except: from.is_some_and(|f| f.other_than), spell, own }
            }
            EventAction::CountersPut { kind, one_or_more: false, nth: None } => Cant::ReceiveCounters(kind.as_ref().map(|k|k.0.clone())),
            EventAction::GainsLife { amount: None } => Cant::Happen(Happening::GainsLife),
            EventAction::Deed(d) if d.passive && d.args.is_empty() && d.verb == Verb::Destroy => Cant::Happen(Happening::Destroyed),
            EventAction::Deed(d) if d.passive && d.args.is_empty() && d.verb == Verb::Exile => Cant::Happen(Happening::Exiled),
            EventAction::Deed(d) if d.passive && d.args.is_empty() && d.verb == Verb::Counter => Cant::Happen(Happening::Countered),
            other => {
                return unsupported(match other.reading() {
                    Some((verb, _)) => format!("prohibition: {verb:?}"),
                    None => format!("prohibition: {}", name(other)),
                });
            }
        })
    }

    fn scalars(&self, out: &mut Vec<Scalar>) {
        match self {
            Cant::BeTargeted(Some(sources)) => sources.filter.scalars(out),
            Cant::Attack(nouns) => nouns.iter().for_each(|n| n.scalars(out)),
            Cant::Block { blocked: Some(noun), .. } => noun.scalars(out),
            Cant::BeBlocked { by, .. } => by.iter().for_each(|b| match b {
                Blockers::One(noun) => noun.scalars(out),
                Blockers::AtLeast(_, set) => out.extend(set.scalars()),
            }),
            Cant::Cast { from, spell, .. } => {
                if let Some(set) = from { out.extend(set.scalars()); }
                if let Some(filter) = spell { filter.scalars(out); }
            }
            Cant::BeTargeted(None) | Cant::Block { blocked: None, .. } | Cant::Happen(_) | Cant::ReceiveCounters(_) => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Says {
    NoMaximumHandSize,
    /// All unspent mana of this color, independent of its producing source (106.4).
    KeepMana(ir::Color),
    CastOnlyAsSorcery,
    CastAsThoughFlash(expr::Filter),
    AttackPayment { defender: Noun, cost: ir::ManaCost },
    /// A resolving spell or ability of this controller relation cannot make the affected
    /// player sacrifice a described permanent. Announced costs have no resolving cause.
    CantCauseSacrifice { controller: Option<expr::Controller>, what: Noun },
    CantActivate { unless_mana: bool },
    CantActivateFamily(crate::activate::ActivationFilter),
    Cant(Vec<Cant>),
    /// "doesn't untap during its controller's untap step" (502.3): an effect that keeps a
    /// permanent from untapping. `yours`: "during your untap step"; `next`: the next one.
    DoesntUntap { yours: bool, next: bool },
    /// "attacks each combat if able" (508.1d), and "attacks Gideon Jura if able": a requirement
    /// obeyed by attacking any of these, or anything when there are none.
    MustAttack(Vec<Noun>),
    /// Additional attacking creatures one creature may block (509.1a); None is unlimited.
    ExtraBlocks(Option<u32>),
    /// "saddles Mounts and crews Vehicles as though its power were N greater" (Some(N)),
    /// "… using its toughness rather than its power" (None): the power a creature counts with
    /// toward a crew or saddle cost's total (702.122a, 702.171a). `crew_only`: "crews
    /// Vehicles as though …", not saddling.
    CrewPower { by: Option<u32>, crew_only: bool },
    /// "Damage can't be prevented" (615.12): said of all damage, whatever would prevent it.
    Unpreventable,
    /// "If that creature would die this turn, exile it instead." (614.1a): where it would go
    /// from the battlefield to a graveyard, it is exiled.
    ExiledInsteadOfDying,
    /// "If it would leave the battlefield, exile it instead of putting it anywhere else."
    /// (614.1a, 702.84a): wherever else it would go from the battlefield, it is exiled. Said of
    /// that permanent, it no longer applies once it has left (400.7).
    ExiledInsteadOfLeaving,
    /// "Prevent all combat damage that would be dealt to and dealt by this creature." (615.1):
    /// a permanent's prevention, of damage dealt to any of `to` (or to anything when empty),
    /// by any of `by` (or by anything); `to_or_by`: either suffices.
    Prevent { to: Vec<Noun>, by: Vec<Noun>, to_or_by: bool, kind: ir::event::DamageKind },
    /// "You have protection from everything" (702.16b–c, e): a player's protection, its
    /// qualities said by the effect or static ability that gives it.
    Protected(std::sync::Arc<crate::protection::Protection>),
}

/// A restriction or a requirement, as the engine keeps and consults it. Only attacking and
/// blocking, targeting, untapping and a few events are consulted so far (508.1c–d, 509.1b); a
/// requirement to block needs the most requirements that can be obeyed together (509.1c),
/// which is not worked out yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule(Says);

impl Rule {
    /// Recognize a per-attacker payment from typed grammar, checking that the payer and
    /// multiplier refer to precisely the creatures the prohibition describes.
    pub(crate) fn attack_payment(a: &Ability, support: &Support<'_>, bind: Bind<'_>) -> Option<Res<(ir::Subject, Self)>> {
        let Ability::Text(ir::Effect(rows)) = a else { return None; };
        let [ir::Sentence::Do(body)] = rows.as_slice() else { return None; };
        let clause = &body.first;
        let Action::Cant(items) = &clause.first.action else { return None; };
        if !clause.first.mods.iter().any(|m| matches!(m, Modifier::Unless { .. })) { return None; }
        Some((|| {
            if !body.rest.is_empty() || !clause.rest.is_empty() || clause.may || clause.also || clause.first.may {
                return unsupported("attack payment: additional or optional clauses");
            }
            let [Forbidden { event: EventAction::Attacks { alone: false, defender, with: None }, within: None }] = items.as_slice() else {
                return unsupported("attack payment: unsupported prohibition");
            };
            let [recipient] = defender.as_slice() else { return unsupported("attack payment: ambiguous recipient"); };
            let [Modifier::Unless { who, actions }, Modifier::ForEach(Amount::Count(count))] = clause.first.mods.as_slice() else {
                return unsupported("attack payment: missing per-attacker multiplier");
            };
            let [Action::PayMana(cost)] = actions.items.as_slice() else { return unsupported("attack payment: nonmana or multiple costs"); };
            if actions.conj != ir::Conj::And || cost.0.iter().any(|m| m.mana_payments().is_none()) {
                return unsupported("attack payment: variable, snow or life alternative cost");
            }
            let ir::Subject::Set(subject) = clause.subject.as_ref().ok_or_else(|| crate::Unsupported("attack payment: missing subject".into()))? else {
                return unsupported("attack payment: unsupported subject");
            };
            let Quant::Definite(Ref::ControllerOf(of)) = &who.quant else { return unsupported("attack payment: payer is not the attackers' controller"); };
            let anaphor = |s: &Selector, domain| s.quant == Quant::Definite(Ref::Anaphor(ir::selector::Anaphor::They))
                && s.set == ObjectSet::of(domain) && s.chooser.is_none() && s.random.is_none();
            if !anaphor(of, Domain::Object) || who.set != ObjectSet::of(Domain::Player) || who.chooser.is_some() || who.random.is_some() {
                return unsupported("attack payment: ambiguous payer");
            }
            let mut counted = count.clone();
            let Some(controller) = counted.controller.take() else { return unsupported("attack payment: multiplier lacks controller"); };
            if controller.negated || controller.past || !anaphor(&controller.who, Domain::Player) {
                return unsupported("attack payment: different multiplier controller");
            }
            let ir::Filter::All(parts) = &mut counted.filter else { return unsupported("attack payment: ambiguous multiplier"); };
            let attack = ir::Filter::Atom(ir::Atom::Attacking(Box::new(recipient.clone())));
            let Some(i) = parts.iter().position(|p| *p == attack) else { return unsupported("attack payment: different multiplier recipient"); };
            parts.remove(i);
            if *counted != *subject { return unsupported("attack payment: different multiplier creatures"); }
            Ok((clause.subject.clone().unwrap(), Self(Says::AttackPayment { defender: Noun::compile(recipient, support, bind)?, cost: cost.clone() })))
        })())
    }
    /// Source-zone entry restrictions are checked before proposals and replacements.
    /// Prospective-permanent/own-entry rules require the 614.17d view, not this hook.
    pub(crate) fn validate_entry_subject(&self, subject: Option<&Subject>) -> Res<()> {
        if self.cants().contains(&Cant::Happen(Happening::Enters))
            && !matches!(subject, Some(Subject::Described(set)) if set.source_zone_cards()) {
            return unsupported("entry prohibition: only cards described in their source zones");
        }
        Ok(())
    }
    /// Dependencies of the compiled predicates, shared with resource feasibility.

    pub(crate) fn activation_family(s: &ir::statics::Static) -> Res<(Self, bool)> {
        let ir::statics::Static::ActivatedAbilities { of: ir::statics::AbilitiesOf::Matching(set), rule: ir::statics::AbilityRule::CantBeActivated { unless_mana: false }, this_turn } = s else {
            return unsupported("activation family: an unqualified ability-family prohibition");
        };
        Ok((Self(Says::CantActivateFamily(crate::activate::ActivationFilter::compile(set)?)), *this_turn))
    }
    pub(crate) fn cant_activate(unless_mana: bool) -> Self { Self(Says::CantActivate { unless_mana }) }
    pub(crate) fn cant_cause(sources: &ObjectSet, event: &EventAction, support: &Support<'_>, bind: Bind<'_>) -> Res<Self> {
        let mut description = sources.clone();
        description.controller = None;
        // A frame retains the causing effect's controller, not a snapshot of all of its
        // characteristics. Do not conflate ability characteristics with their source's.
        if description != ObjectSet::of(Domain::Union(vec![ObjectSet::of(Domain::Spell), ObjectSet::of(Domain::Ability)])) {
            return unsupported("causal prohibition: only spells and abilities qualified by controller");
        }
        let EventAction::Deed(deed) = event else { return unsupported("causal prohibition: unsupported event"); };
        if deed.passive || deed.verb != Verb::Sacrifice || deed.args.len() != 1 {
            return unsupported("causal prohibition: only sacrificing described permanents");
        }
        let Some(what) = deed.noun(Role::What) else { return unsupported("causal prohibition: missing sacrifice object"); };
        Ok(Self(Says::CantCauseSacrifice {
            controller: sources.controller.as_ref().map(expr::Controller::compile).transpose()?,
            what: Noun::compile(what, support, bind)?,
        }))
    }
    /// A keyword a player has (702.11c, 702.16b, 702.18a): hexproof and shroud are their
    /// targeting prohibitions, protection the qualities it says. Pure static "you" grants
    /// are routed here by registration, and so are a resolving effect's grants to players;
    /// their holder is the player the rule affects. Ordinary object grants retain their
    /// layer path.
    fn player_keyword(granted: &[ir::Granted], support: &Support<'_>, bind: Bind<'_>) -> Res<Rule> {
        let [ir::Granted::Keyword(item)] = granted else {
            return unsupported("player keyword: only hexproof, shroud or protection is implemented");
        };
        if crate::protection::known(item) {
            return Ok(Rule(Says::Protected(std::sync::Arc::new(crate::protection::Protection::compile(item, support, bind)?))));
        }
        if item.param != ir::token::KeywordParam::None
            || !matches!(item.keyword.name(), "Hexproof" | "Shroud")
        {
            return unsupported("player keyword: only unparameterized hexproof or shroud, or protection, is implemented");
        }
        let Some([rule]) = registration::keyword(item.keyword) else {
            return unsupported("player keyword: no single targeting rule");
        };
        let rule = rule.as_ref().map_err(Clone::clone)?;
        let [(registration::When::Always, Rule(Says::Cant(cants)))] = rule.acts.as_slice() else {
            return unsupported("player keyword: no unconditional targeting rule");
        };
        if !matches!(cants.as_slice(), [Cant::BeTargeted(_)]) {
            return unsupported("player keyword: no single targeting prohibition");
        }
        Ok(Rule(Says::Cant(cants.clone())))
    }

    /// The rule the act states. A reference to a chosen value is bound by `bind`.
    pub(crate) fn compile(action: &Action, support: &Support<'_>, bind: Bind<'_>) -> Res<Rule> {
        Ok(Rule(match action {
            // a resolving effect's grant to players ([`Game::player_grant`])
            Action::Gains(granted) => Self::player_keyword(granted, support, bind)?.0,
            Action::KeepMana { mana: ir::mana::Pooled::Unspent(Some(color)), phases: true } => Says::KeepMana(*color),
            Action::Cant(items) => Says::Cant(items.iter().map(|item| Cant::compile(item, support, &mut *bind)).collect::<Res<_>>()?),
            // asked as the untap step's permanents untap ([`Game::kept_tapped`])
            Action::DoesntUntap { whose: whose @ (UntapWhose::Controller | UntapWhose::Controllers | UntapWhose::Yours), next } => {
                Says::DoesntUntap { yours: *whose == UntapWhose::Yours, next: *next }
            }
            Action::DoesntUntap { whose, .. } => return unsupported(format!("doesn't untap during the untap step of {}", name(whose))),
            Action::Static(s) => match s.as_ref() {
                StaticAct::CanBlockOnly(set) => Says::Cant(vec![Cant::Block { blocked: Some(Noun::In(expr::Set::compile(set, support, bind)?)), except: true }]),
                StaticAct::NoMaximumHandSize => Says::NoMaximumHandSize,
                // Pure static "you" grants are routed here by registration. Their holder
                // is this rule's controller (702.11c, 702.18a), so reuse the exact existing
                // keyword prohibition; ordinary object grants retain their layer path.
                StaticAct::Has(granted) => Self::player_keyword(granted, support, bind)?.0,
                StaticAct::CastOnly { what, restrictions }
                    if *what == ObjectSet::of(Domain::Spell) && restrictions.as_slice() == [ir::Restriction::Sorcery] =>
                        Says::CastOnlyAsSorcery,
                StaticAct::CastOnly { .. } => return unsupported("casting restriction: only all spells at sorcery timing are lowered"),
                StaticAct::CastAsThoughFlash { what, .. } => {
                    let ir::Who::Set(set) = what else {
                        return unsupported("flash permission: only described spells");
                    };
                    if *set != (ObjectSet { filter: set.filter.clone(), ..ObjectSet::of(Domain::Spell) }) {
                        return unsupported("flash permission: only unqualified spell descriptions");
                    }
                    Says::CastAsThoughFlash(expr::Filter::compile(&set.filter, support, bind)?)
                },
                StaticAct::CanBlockAdditional { count, when: None | Some(Within::EachCombat | Within::ThisTurn), .. } => {
                    Says::ExtraBlocks(match count {
                        None => None,
                        Some(Amount::Const(n)) => Some(*n),
                        _ => return unsupported("blocking capacity: variable count"),
                    })
                }
                // "attacks each turn if able" is obeyed by attacking once that turn, not in
                // each combat of it (508.1d; ruling 0e5bb944856b8e46): that needs the
                // creature's attack history, which no requirement reads yet
                StaticAct::IfAble { when: Some(Within::EachTurn), .. } => return unsupported("requirement: once each turn (needs attack history)"),
                StaticAct::IfAble { what, .. } => {
                    for e in what {
                        match e {
                            EventAction::Attacks { alone: false, defender, with: None } if defender.is_empty() || what.len() == 1 => {}
                            EventAction::Attacks { .. } => return unsupported("requirement: to attack something in particular"),
                            _ => return unsupported("requirement: to block"),
                        }
                    }
                    let [EventAction::Attacks { defender, .. }, ..] = what.as_slice() else {
                        return unsupported("requirement: of nothing");
                    };
                    // "attacks Gideon Jura if able", "attack you if able": what it attacks to
                    // obey it (508.1d). "An opponent with the most life" is any one no other
                    // opponent has more life than: of those tied for it, whichever it attacks.
                    let any_of_the_most = |d: &Selector| {
                        let mut d = d.clone();
                        if let (Quant::Indefinite(_), ir::Filter::All(parts)) = (&d.quant, &mut d.set.filter) {
                            for part in parts {
                                if let ir::Filter::Atom(ir::Atom::MostOf { tied, .. }) = part { *tied = true; }
                            }
                        }
                        d
                    };
                    Says::MustAttack(defender.iter().map(|d| Noun::compile(&any_of_the_most(d), support, &mut *bind)).collect::<Res<_>>()
                        .map_err(|_| crate::Unsupported("requirement: to attack something in particular".into()))?)
                }
                StaticAct::SaddlesAndCrews(by) => Says::CrewPower { by: match by {
                    None => None,
                    Some(Amount::Const(n)) => Some(*n),
                    Some(_) => return unsupported("crew power: a variable amount"),
                }, crew_only: false },
                StaticAct::CrewsAsThoughGreater(Amount::Const(n)) => Says::CrewPower { by: Some(*n), crew_only: true },
                StaticAct::CrewsAsThoughGreater(_) => return unsupported("crew power: a variable amount"),
                other => return unsupported(format!("static: {}", name(other))),
            },
            other => return unsupported(format!("action: {}", name(other))),
        }))
    }

    /// The rule a sentence of a resolving spell or ability states of the game rather than
    /// of a subject, and how long it lasts: "Damage can't be prevented this turn." (615.12),
    /// asked where damage would be prevented ([`Game::unpreventable`]).
    pub(crate) fn sentence(s: &ir::statics::Static) -> Res<(Rule, Lifetime)> {
        match s {
            ir::statics::Static::ActivatedAbilities { of: ir::statics::AbilitiesOf::Matching(_),.. } => {
                let (rule,this_turn)=Self::activation_family(s)?;
                if !this_turn { return unsupported("activation-family effect: an explicit implemented duration is required"); }
                Ok((rule,Lifetime::ThisTurn))
            },
            ir::statics::Static::Unpreventable { damage, this_turn, redirect } => {
                if !*this_turn {
                    return unsupported("a rule that lasts other than this turn");
                }
                Ok((Self::unpreventable(damage, *redirect)?, Lifetime::ThisTurn))
            }
            other => unsupported(format!("static: {}", name(other))),
        }
    }

    /// Said by a resolving spell or ability, "If that creature would die this turn, exile it
    /// instead." or "If it would leave the battlefield, exile it instead of putting it
    /// anywhere else." (702.84a): the objects it is said of, and which departure it replaces.
    /// Only "it", of them, is exiled; nothing else is done instead.
    pub(crate) fn exiled_instead_of(s: &ir::statics::Static) -> Option<(&ir::Selector, Exiled)> {
        use ir::statics::{Static, Substitute};
        let Static::Replacement(r) = s else { return None };
        let event = r.single_event()?;
        if r.next_time || r.while_.is_some() || r.instead_first || event.counted()
            || event.during.is_some() || event.condition.is_some() || event.cause.is_some()
            || event.nth_each_turn.is_some() || event.except_nth_in_step.is_some() { return None; }
        let [who] = event.subjects.as_slice() else { return None };
        let [ir::EventAction::Deed(deed)] = event.actions.as_slice() else { return None };
        if deed.passive || !deed.args.is_empty() { return None; }
        let Substitute::Body(body) = &r.with else { return None };
        let ir::Clause { subject: None, may: false, also: false, first, rest } = &body.first else { return None };
        if !rest.is_empty() || !body.rest.is_empty() || first.may { return None; }
        let exiled = match (deed.verb, r.this_turn, first.mods.as_slice()) {
            (ir::Verb::Die, true, [Modifier::Instead]) => Exiled::Dying,
            (ir::Verb::LeaveBattlefield, false, [Modifier::InsteadOf(ir::statics::Elsewhere::AnywhereElse)]) => Exiled::Leaving,
            _ => return None,
        };
        let Action::Exile(objects) = &first.action else { return None };
        let [it] = objects.items.as_slice() else { return None };
        (*it == ir::Selector::new(ir::Quant::Definite(ir::Ref::Anaphor(ir::Anaphor::It)), ir::ObjectSet::of(ir::Domain::Object))).then_some((who, exiled))
    }
    /// The rule of [`Rule::exiled_instead_of`], and how long it lasts: "this turn", or with no
    /// duration stated, until the game ends (611.2a).
    pub(crate) fn exiled_instead(exiled: Exiled) -> (Rule, Lifetime) {
        match exiled {
            Exiled::Dying => (Rule(Says::ExiledInsteadOfDying), Lifetime::ThisTurn),
            Exiled::Leaving => (Rule(Says::ExiledInsteadOfLeaving), Lifetime::Game),
        }
    }

    /// A permanent's "Prevent all [combat|noncombat] damage that would be dealt to … [by …]"
    /// (615.1), in force while the ability functions rather than for a duration.
    pub(crate) fn prevent(p: &ir::Prevention, support: &Support<'_>, bind: Bind<'_>) -> Res<Rule> {
        if p.amount != ir::Prevented::All || p.this_turn.is_some() {
            return unsupported("static prevention: all damage, for as long as the ability functions");
        }
        let mut party = |party: &Option<ir::statics::Party>, sources: bool| -> Res<Vec<Noun>> {
            let Some(party) = party else { return Ok(Vec::new()) };
            party.items.iter().map(|who| {
                let (domain, noun) = match who {
                    ir::statics::Who::One(s) => (&s.set.domain, Noun::compile(s, support, &mut *bind)?),
                    ir::statics::Who::Set(set) => (&set.domain, Noun::In(expr::Set::compile(set, support, &mut *bind)?)),
                };
                if sources && (*domain == Domain::Player || matches!(noun, Noun::You)) {
                    return unsupported("static prevention: damage sources cannot be players");
                }
                Ok(noun)
            }).collect()
        };
        let to = party(&p.to, false)?;
        let by = party(&p.by, true)?;
        if to.is_empty() && by.is_empty() { return unsupported("static prevention: of all damage"); }
        Ok(Rule(Says::Prevent { to, by, to_or_by: p.to_or_by, kind: p.kind }))
    }

    /// One damage prohibition, whether installed by a static or resolving ability (615.12).
    pub(crate) fn unpreventable(damage: &ir::Prevention, redirect: bool) -> Res<Rule> {
        if redirect { return unsupported("unpreventable: damage that can't be redirected"); }
        let all = ir::Prevention { amount: ir::Prevented::All, kind: ir::event::DamageKind::Any, to: None, by: None, to_or_by: false, this_turn: None };
        if *damage != all { return unsupported("unpreventable: of some damage only"); }
        Ok(Rule(Says::Unpreventable))
    }

    /// The chosen values the rule's descriptions read, for checking they are produced
    /// before it.
    pub fn scalars(&self) -> Vec<Scalar> {
        let mut out = Vec::new();
        match &self.0 {
            Says::CastAsThoughFlash(filter) => filter.scalars(&mut out),
            Says::AttackPayment { defender, .. } => defender.scalars(&mut out),
            Says::Cant(items) => items.iter().for_each(|item| item.scalars(&mut out)),
            Says::CantCauseSacrifice { what, .. } => what.scalars(&mut out),
            Says::Protected(p) => out.extend(p.scalars()),
            Says::Prevent { to, by, .. } => to.iter().chain(by).for_each(|n| n.scalars(&mut out)),
            Says::MustAttack(defenders) => defenders.iter().for_each(|n| n.scalars(&mut out)),
            _ => {}
        }
        out
    }

    fn cants(&self) -> &[Cant] {
        match &self.0 {
            Says::Cant(items) => items,
            _ => &[],
        }
    }
}

/// How long a rule a resolved spell or ability put in force lasts (611.2a).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifetime {
    /// "this turn", "until end of turn": until the cleanup step (514.2).
    ThisTurn,
    /// "doesn't untap during its controller's next untap step": through the next untap step
    /// of each player it is about.
    NextUntap,
    /// Expires as the granting controller's next turn begins (611.2a).
    UntilYourNextTurn,
    /// "attacks this combat if able": for that combat phase only, until it ends (500.5a). A
    /// compiled rule says None, "the combat it is put in force in"; as it is put in force it
    /// names that combat phase, or the next one of the turn when none is in progress. With
    /// none, it applies to nothing.
    ThisCombat(Option<crate::turn::PhaseId>),
    /// No duration stated: until the game ends (611.2a).
    Game,
    /// "During target opponent's next turn": in force through the next turn that player
    /// begins after it was put in force, and over once that turn ends. `player` is who that
    /// is, named as it is put in force; `began`, once that turn has begun.
    NextTurnOf { player: Option<crate::PlayerId>, began: bool },
}

/// Which departure of the objects it is said of a resolved "exile it instead" replaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Exiled {
    /// Its death: from the battlefield to a graveyard (700.4).
    Dying,
    /// Its leaving the battlefield, for anywhere but exile (702.84a).
    Leaving,
}

impl Lifetime {
    /// How long the text says the act lasts, when the engine keeps a rule that long.
    pub(crate) fn compile(modifiers: &[Modifier], action: &Action) -> Res<Lifetime> {
        if modifiers.is_empty() && matches!(action, Action::Static(s) if matches!(**s, StaticAct::IfAble { when: Some(Within::ThisCombat), .. })) {
            Ok(Lifetime::ThisCombat(None))
        } else if lasts_this_turn(modifiers, action) {
            Ok(Lifetime::ThisTurn)
        } else if matches!(modifiers, [Modifier::Duration(ir::effect::Duration::Until(ir::event::Time {
            end: false, step: ir::event::Step::Turn, whose: ir::event::StepOwner::Player { who, next: true },
        }))] if crate::select::is_you(who)) {
            Ok(Lifetime::UntilYourNextTurn)
        } else if through_next_untap(modifiers, action) {
            Ok(Lifetime::NextUntap)
        } else {
            unsupported("a rule that lasts other than this turn")
        }
    }

    /// "During target opponent's next turn, creatures that player controls attack Gideon Jura
    /// if able.": a rule that says no window of its own, kept through that player's next turn.
    pub(crate) fn during_next_turn(modifiers: &[Modifier], action: &Action) -> Res<Lifetime> {
        if !modifiers.is_empty() || Lifetime::compile(&[], action).is_ok() {
            return unsupported("during a player's next turn: a rule with a window of its own");
        }
        Ok(Lifetime::NextTurnOf { player: None, began: false })
    }
}

/// The values of the frame that put a rule in force, as they were then: what "the chosen
/// color" of its descriptions reads for as long as it lasts.
struct Kept<'a>(&'a [Option<Value>]);

impl Locals for Kept<'_> {
    fn local(&self, slot: usize) -> Option<&Value> {
        self.0.get(slot)?.as_ref()
    }
}

/// An effect of a resolved spell or ability that lasts (611.2a): until the cleanup step
/// (514.2), through the next named untap steps, or until its controller's next turn begins.
#[derive(Debug, Clone)]
pub struct Lasting {
    pub ctx: Ctx<'static>,
    pub affected: Affected,
    rule: Rule,
    lifetime: Lifetime,
    /// What the frame that put it in force held, by slot.
    locals: Vec<Option<Value>>,
    /// Some: it lasts through next untap steps, and these players' has passed since it began.
    untap_steps: Option<Vec<PlayerId>>,
}

/// The statement a static ability is, when it is one of these: a subject, and what it can't
/// do or must ("This creature can't block.", "Creatures you control attack each combat if
/// able."), for as long as its condition holds if it says one ("This creature can't attack
/// unless defending player controls an Island"). Read as a definition is loaded, where it is
/// lowered ([`registration::restrictions`]); nothing in force is asked through it.
pub fn statement(a: &Ability) -> Option<Statement<'_>> {
    let statement = crate::statement::read(a)?;
    if statement.acts.iter().all(|(action, _, scale)| rule(action) && *scale == Scale::Once) { return Some(statement); }
    // its rules, beside what it says its subject is
    statement.mixed().then(|| statement.only(|action, scale| rule(action) && *scale == Scale::Once))
}

/// "Spells with the chosen name can't be cast.": that players can't cast such spells
/// (601.3), the subject and the act said that way, when the statement is this.
pub(crate) fn spells_cant_be_cast(statement: &Statement<'_>) -> Option<(ir::Subject, Action)> {
    let ir::Subject::Set(set) = &statement.subject else { return None };
    let [(Action::Cant(items), _, _)] = statement.acts.as_slice() else { return None };
    let [Forbidden { event: EventAction::Deed(deed), within: None }] = items.as_slice() else { return None };
    if set.domain != Domain::Spell || deed.verb != Verb::Cast || !deed.passive || !deed.args.is_empty() { return None; }
    let spells = ir::Selector { quant: Quant::Indefinite(Count::Exactly(Amount::Const(1))), set: set.clone(), chooser: None, random: None };
    let cast = ir::event::Deed { verb: Verb::Cast, passive: false, args: vec![(Role::What, ir::event::Arg::Noun(spells))] };
    Some((ir::Subject::Set(ObjectSet::of(Domain::Player)), Action::Cant(vec![Forbidden { event: EventAction::Deed(cast), within: None }])))
}

/// Is this act a restriction or a requirement? "Doesn't untap during its controller's untap
/// step" is one: an effect that keeps a permanent from untapping (502.3).
pub(crate) fn rule(a: &Action) -> bool {
    matches!(a, Action::Cant(_) | Action::DoesntUntap { .. } | Action::KeepMana { mana: ir::mana::Pooled::Unspent(Some(_)), phases: true }) || matches!(a, Action::Static(s) if matches!(**s, StaticAct::CastOnly { .. } | StaticAct::CastAsThoughFlash { .. } | StaticAct::NoMaximumHandSize | StaticAct::IfAble { .. } | StaticAct::CanBlockAdditional { .. } | StaticAct::CanBlockOnly(_) | StaticAct::SaddlesAndCrews(_) | StaticAct::CrewsAsThoughGreater(_)))
}

/// How long an act that is a rule lasts, when the engine keeps it: "this turn", "until end of
/// turn" (514.2).
fn lasts_this_turn(mods: &[Modifier], a: &Action) -> bool {
    use ir::effect::Duration;
    use ir::event::{Step, StepOwner, Time};
    let end_of_turn = Time { end: true, step: Step::Turn, whose: StepOwner::This };
    match (mods, a) {
        ([Modifier::Duration(Duration::ThisTurn)], _) => true,
        ([Modifier::Duration(Duration::Until(t))], _) => *t == end_of_turn,
        // a requirement names its window inside itself: "attacks this turn if able"
        ([], Action::Static(s)) => matches!(**s, StaticAct::CastAsThoughFlash { this_turn: true, .. } | StaticAct::IfAble { when: Some(Within::ThisTurn), .. } | StaticAct::CanBlockAdditional { when: Some(Within::ThisTurn), .. }),
        _ => false,
    }
}

/// Does the act last through next untap steps: "doesn't untap during its controller's next
/// untap step", "don't untap during your next untap step"?
fn through_next_untap(mods: &[Modifier], a: &Action) -> bool {
    mods.is_empty() && matches!(a, Action::DoesntUntap { next: true, .. })
}

/// A rule in force and what it is said of. `passed`: of one that lasts through next untap
/// steps, the players whose has passed.
struct InForce<'a> {
    ctx: Ctx<'static>,
    of: Of<'a>,
    rule: &'a Rule,
    locals: Kept<'a>,
    passed: &'a [PlayerId],
    /// The keyword ability whose rule this is (702), when it is one.
    keyword: Option<ir::Keyword>,
}

/// What a rule in force is said of.
enum Of<'a> {
    /// A static ability's subject, asked of the game as it is (611.3a).
    Subject(&'a Subject),
    /// What a resolved spell or ability said it of.
    Kept(&'a Affected),
}

impl InForce<'_> {
    /// Is this object or player one the rule is said of, now?
    fn affects(&self, game: &Game, t: Target) -> bool {
        match self.of {
            Of::Subject(subject) => subject.contains(game, &self.ctx, t),
            Of::Kept(affected) => affects(game, &self.ctx, affected, t),
        }
    }
}

impl Game {
    /// Color-wide retention is a rule about the affected player's pool, not a tag
    /// placed only on the mana produced by the resolving instruction.
    pub(crate) fn kept_mana_colors(&self, player: PlayerId) -> [bool; 5] {
        let mut colors = [false; 5];
        for r in self.in_force() {
            if let Says::KeepMana(color) = r.rule.0
                && r.affects(self, Target::Player(player)) { colors[color as usize] = true; }
        }
        colors
    }

    /// May this player's spell or ability target this object/player (115.1)? Asked at
    /// announcement and again at resolution. Membership and nontargeting effects are separate.
    pub fn can_target(&self, ctx: &Ctx, target: Target) -> bool {
        if self.protected_from(target, ctx) { return false; }
        let controller = ctx.controller;
        !self.in_force().iter().any(|r| {
            if !r.affects(self, target) { return false; }
            r.rule.cants().iter().any(|item| {
                let Cant::BeTargeted(of) = item else { return false };
                let Some(of) = of else { return true };
                if of.spells && !ctx.targeting_spell { return false; }
                // The restriction's "you" names its controller, but a split spell being
                // announced supplies only the chosen half's characteristics (709.3a).
                let description = Ctx { casting: ctx.casting, ..r.ctx.clone() };
                of.controller.is_none_or(|rel| rel.matches(&r.ctx, controller))
                    && of.filter.matches(self, &description, &r.locals, Target::Object(ctx.object_source()))
            })
        })
    }

    /// What keeps `target` from being targeted by what `ctx` is, as [`Game::can_target`] finds
    /// it, with the rule: a keyword by name ("it has hexproof (702.11b)"), else the source.
    pub(crate) fn target_restriction(&self, ctx: &Ctx, target: Target) -> Option<String> {
        let controller = ctx.controller;
        let restricting = self.in_force().into_iter().find(|r| r.affects(self, target) && r.rule.cants().iter().any(|item| {
            let Cant::BeTargeted(of) = item else { return false };
            let Some(of) = of else { return true };
            if of.spells && !ctx.targeting_spell { return false; }
            let description = Ctx { casting: ctx.casting, ..r.ctx.clone() };
            of.controller.is_none_or(|rel| rel.matches(&r.ctx, controller))
                && of.filter.matches(self, &description, &r.locals, Target::Object(ctx.object_source()))
        }))?;
        let who = match target {
            Target::Player(p) if p == controller => "you have".to_string(),
            Target::Player(_) => "that player has".to_string(),
            Target::Object(_) => "it has".to_string(),
        };
        let source = restricting.ctx.source.filter(|s| Target::Object(*s) != target).map(|s| self.obj(s).chars.name.clone());
        // a player's hexproof or shroud is that keyword's prohibition, given to the player
        // (702.11c, 702.18a)
        if let Target::Player(_) = target {
            for (name, rule) in [("Hexproof", "702.11c"), ("Shroud", "702.18a")] {
                let same = ir::Keyword::from_name(name).and_then(registration::keyword)
                    .is_some_and(|rules| rules.iter().any(|r| r.as_ref().is_ok_and(|r| r.acts.iter().any(|(_, said)| said == restricting.rule))));
                if same { return Some(format!("{who} {} ({rule})", name.to_lowercase())); }
            }
        }
        Some(match restricting.keyword.and_then(|k| crate::keyword::sentence_rule(k).map(|rule| (k, rule))) {
            Some((keyword, rule)) => format!("{who} {} ({rule})", keyword.name().to_lowercase()),
            None => match source {
                Some(source) => format!("{source} says it can't be the target of that (115.1)"),
                None => "an effect says it can't be the target of that (115.1)".into(),
            },
        })
    }

    /// Does a rule in force give this player protection from the source `source` names
    /// (702.16b–c, e)? Its qualities are said by the frame that gives it.
    pub(crate) fn player_protected_from(&self, player: PlayerId, source: &Ctx) -> bool {
        self.in_force().iter().any(|r| matches!(&r.rule.0, Says::Protected(p)
            if r.affects(self, Target::Player(player)) && p.protects(self, &r.ctx, &r.locals, source)))
    }

    /// A resolving spell or ability puts a rule in force, said of these and lasting this
    /// long. `locals`: what its frame holds now, by slot, which is what the rule's
    /// descriptions read from then on.
    pub(crate) fn keep(&mut self, ctx: Ctx<'static>, affected: Affected, rule: Rule, lifetime: Lifetime, locals: Vec<Option<Value>>) {
        let untap_steps = (lifetime == Lifetime::NextUntap).then(Vec::new);
        // "this combat": the combat phase it is put in force in, or the next one (500.5a)
        let lifetime = match lifetime {
            Lifetime::ThisCombat(None) => {
                self.sync_plan();
                Lifetime::ThisCombat(self.plan.this_combat())
            }
            other => other,
        };
        self.lasting.push(Lasting { ctx, affected, rule, lifetime, locals, untap_steps });
    }

    /// The turn ends: the rules that last until the cleanup step are over (514.2), and none
    /// said of one of its combat phases outlives it.
    pub(crate) fn end_turn_rules(&mut self) {
        self.lasting.retain(|l| !matches!(l.lifetime, Lifetime::ThisTurn | Lifetime::ThisCombat(_) | Lifetime::NextTurnOf { began: true, .. }));
        // what is said only of permanents that have left the battlefield since no longer
        // applies to anything (400.7)
        let mut lasting = std::mem::take(&mut self.lasting);
        lasting.retain(|l| !(l.rule.0 == Says::ExiledInsteadOfLeaving && matches!(&l.affected,
            crate::statement::Affected::Things(things) if things.iter().all(|t| !matches!(t, Target::Object(o) if self.battlefield.contains(o))))));
        self.lasting = lasting;
    }

    /// A combat phase ends: what lasts "this combat" of it is over (500.5a). An effect that
    /// lasts this turn is not: it applies in each combat of the turn (508.1d).
    pub(crate) fn end_combat_rules(&mut self, combat: Option<crate::turn::PhaseId>) {
        self.lasting.retain(|l| !matches!(l.lifetime, Lifetime::ThisCombat(id) if id == combat || id.is_none()));
    }

    pub(crate) fn begin_turn_rules(&mut self) {
        let active = self.active;
        self.lasting.retain(|l| l.lifetime != Lifetime::UntilYourNextTurn || l.ctx.controller != active);
        // "during that player's next turn": this one, when it is theirs
        for l in &mut self.lasting {
            if let Lifetime::NextTurnOf { player: Some(player), began } = &mut l.lifetime && *player == active { *began = true; }
        }
    }

    /// Does an effect keep this permanent from untapping as its controller's untap step has
    /// them untap their permanents (502.3)? "Doesn't untap during its controller's untap
    /// step" of any permanent it is about; "during your untap step", in that player's; "next
    /// untap step", in the first one since the effect began.
    pub(crate) fn kept_tapped(&self, permanent: ObjId) -> bool {
        let active = self.active;
        self.in_force().iter().any(|r| match r.rule.0 {
            Says::DoesntUntap { yours, next } => (!yours || r.ctx.controller == active) && (!next || !r.passed.contains(&active)) && self.affects(r, permanent),
            _ => false,
        })
    }

    /// The active player's untap step has untapped their permanents: for the effects that
    /// last through next untap steps this player's has passed, and one is over when the step
    /// it names has ("your next untap step": its controller's; "its controller's": each
    /// player's, whoever controls what it is about).
    pub(crate) fn untap_step_passed(&mut self) {
        let (active, players) = (self.active, self.players.len());
        for l in &mut self.lasting {
            if let Some(passed) = &mut l.untap_steps
                && !passed.contains(&active)
            {
                passed.push(active);
            }
        }
        self.lasting.retain(|l| match (&l.untap_steps, &l.rule.0) {
            (Some(_), Says::DoesntUntap { yours: true, .. }) => l.ctx.controller != active,
            (Some(passed), _) => passed.len() < players,
            (None, _) => true,
        });
    }

    /// Is this act one a resolving spell or ability keeps as a rule?
    pub(crate) fn keeps(a: &Action) -> bool {
        rule(a)
    }


    fn in_force<'a>(&'a self) -> Vec<InForce<'a>> {
        let mut out = Vec::new();
        let membership_changed = self.membership_changed();
        // Entry prohibitions inspect the prospective entrant as well as existing
        // sources (614.17d/614.12); it has not joined battlefield membership yet.
        let entering=self.prospective.map(|(on,_)|on).filter(|on|!self.battlefield.contains(on) && !self.stack.contains(on) && !self.command.contains(on));
        for id in self.battlefield.iter().chain(&self.stack).chain(&self.command).chain(entering.iter()) {
            let o = self.obj(*id);
            if o.ability.is_some() { continue; }
            let ctx = Ctx::new(self.controller(*id), *id);
            // where the ability functions, and each rule it states whose condition holds now
            let mut says = |ctx: Ctx<'static>, r: &'a Restriction, keyword: Option<ir::Keyword>| {
                if !(if o.emblem.is_some() { o.zone == crate::Zone::Command } else { r.scope.contains(o.zone) }) { return; }
                for (when, rule) in &r.acts {
                    if when.holds(self, &ctx) {
                        out.push(InForce { ctx: ctx.clone(), of: Of::Subject(&r.subject), rule, locals: Kept(&[]), passed: &[], keyword });
                    }
                }
            };
            // its own static abilities and those effects give it (613.1f), each its own
            // instance
            for (origin, def) in self.origins(*id, crate::ability::View::Effective { membership_changed }).into_iter() {
                for registered in &def.restrictions {
                    // text the engine does not hold in force states no rule: its card was refused
                    if let Ok(r) = &registered.result {
                        says(ctx.clone().with_key(crate::binding::AbilityKey { origin: origin.clone(), index: registered.ability }), r, None);
                    }
                }
            }
            // a keyword ability whose rule is a sentence is the static ability the sentence
            // is, for whatever has the keyword now: printed with it or given it (613.1f)
            for keyword in self.seen_for(*id, crate::layers::Aspect::Abilities).keywords {
                if keyword.contributions().any(|(scope, gives)| scope.contains(o.zone) && matches!(gives, crate::keyword::Gives::Rule(_))) {
                    if let Some(rules) = registration::keyword(keyword.keyword) {
                        for rule in rules { if let Ok(rule) = rule { says(ctx.clone(), rule, Some(keyword.keyword)); } }
                    }
                }
            }
        }
        // what lasts "this combat" applies in that combat phase only (500.5a)
        let combat = if self.lasting.iter().any(|l| matches!(l.lifetime, Lifetime::ThisCombat(_))) {
            self.turn_plan().phase().filter(|run| run.phase == ir::turn::Phase::Combat).map(|run| run.id)
        } else { None };
        // what lasts during a player's next turn applies in that turn only
        out.extend(self.lasting.iter().filter(|l| !matches!(l.lifetime, Lifetime::ThisCombat(id) if id.is_none() || id != combat)
            && !matches!(l.lifetime, Lifetime::NextTurnOf { began: false, .. })).map(|l| InForce {
            ctx: l.ctx.clone(),
            of: Of::Kept(&l.affected),
            rule: &l.rule,
            locals: Kept(&l.locals),
            passed: l.untap_steps.as_deref().unwrap_or(&[]),
            keyword: None,
        }));
        out
    }

    fn affects(&self, r: &InForce, o: ObjId) -> bool {
        r.affects(self, Target::Object(o))
    }

    /// The effective hand-size rule is queried before cleanup discards (402.2, 514.1).
    pub fn maximum_hand_size(&self, player: PlayerId) -> Option<usize> {
        if self.in_force().iter().any(|r| matches!(r.rule.0, Says::NoMaximumHandSize) && r.affects(self, Target::Player(player))) { None }
        else { Some(crate::turn::MAXIMUM_HAND_SIZE) }
    }

    /// Maximum simultaneous blocks allowed by effective static and lasting rules (509.1a).
    /// The caller bounds an unlimited permission by the actual attacking creatures.
    pub fn block_capacity(&self, creature: ObjId) -> usize {
        let mut count = 1usize;
        for r in self.in_force() {
            if let Says::ExtraBlocks(extra) = r.rule.0 {
                if self.affects(&r, creature) {
                    match extra { Some(n) => count = count.saturating_add(n as usize), None => return usize::MAX }
                }
            }
        }
        count
    }

    /// The power this creature counts with toward a crew or saddle cost's total (702.122a,
    /// 702.171a): its toughness if a rule says to use it, plus what rules add.
    /// `saddling`: toward a saddle cost, which "crews Vehicles as though …" doesn't change.
    pub(crate) fn crew_power(&self, creature: ObjId, saddling: bool) -> i32 {
        let (mut toughness, mut plus) = (false, 0i32);
        for r in self.in_force() {
            if let Says::CrewPower { by, crew_only } = r.rule.0 && !(crew_only && saddling) && self.affects(&r, creature) {
                match by { Some(n) => plus = plus.saturating_add(n as i32), None => toughness = true }
            }
        }
        let base = if toughness { self.toughness(creature) } else { self.power(creature) };
        base.unwrap_or(0).saturating_add(plus)
    }

    /// Whether the resolving spell or ability can cause this sacrifice (101.2, 701.21).
    /// Query before collecting simultaneous choices, including costs offered in resolution
    /// (118.12). The causing controller is retained even after its source leaves or changes.
    pub(crate) fn can_cause_sacrifice(&self, cause: crate::event::Cause, player: PlayerId, permanent: ObjId) -> bool {
        !self.in_force().iter().any(|r| {
            let Says::CantCauseSacrifice { controller, what } = &r.rule.0 else { return false };
            r.affects(self, Target::Player(player))
                && controller.is_none_or(|relation| relation.matches(&r.ctx, cause.controller))
                && what.is(self, &r.ctx, &r.locals, Target::Object(permanent))
        })
    }

    /// Activation restrictions inspect the current source and the shared mana classification.
    pub(crate) fn cant_activate(&self, source: ObjId, mana: bool, keyword: Option<ir::Keyword>) -> bool {
        let family = |rule: &Rule| matches!(&rule.0, Says::CantActivateFamily(filter) if filter.matches(keyword,mana));
        self.created_turn_rules().any(family) || self.in_force().iter().any(|r|
            family(&r.rule) || matches!(r.rule.0, Says::CantActivate { unless_mana } if !(unless_mana && mana)) && self.affects(r,source))
    }

    /// Does an effect say this event can't happen (101.2)? Asked of the event before it
    /// happens, said as what it would be: the action, who would do it and what would fill
    /// its roles. A prohibition is of its subject doing it or having it done to it ("Players
    /// can't gain life.", "This permanent can't be destroyed.").
    pub(crate) fn forbidden(&self, would: &Fact) -> bool {
        self.in_force().iter().any(|r| {
            r.rule.cants().iter().any(|item| {
                match item {
                    Cant::Happen(happening) => happening.subject(would).is_some_and(|subject| r.affects(self, subject)),
                    Cant::ReceiveCounters(kind) if would.verb() == Verb::PutCounters =>
                        kind.as_ref().is_none_or(|kind|would.counter_kind()==Some(kind.as_str()))
                            && would.role(Role::On).is_some_and(|holder|r.affects(self,holder)),
                    Cant::Cast { from, except, spell, own } if would.verb() == Verb::Cast => {
                        would.doer().is_some_and(|player| r.affects(self, player))
                            && would.role(Role::What).is_some_and(|card| from.as_ref().is_none_or(|set|
                                (set.contains(self, &r.ctx, &r.locals, card)
                                    && (!own || matches!((card, would.doer()), (Target::Object(c), Some(Target::Player(p))) if self.obj(c).owner == p))) != *except)
                                // as the card is, without an announced face (see [`Game::cast_forbidden`])
                                && spell.as_ref().is_none_or(|filter| filter.matches(self, &r.ctx, &r.locals, card)))
                    }
                    _ => false,
                }
            })
        })
    }

    /// Does an effect say this player can't cast this card as this spell (601.3)? Its
    /// characteristics are those of the spell it would become: the half or face cast,
    /// with the value announced for X (601.3e).
    pub(crate) fn cast_forbidden(&self, player: PlayerId, card: ObjId, cast_as: crate::card::CastAs, x: Option<u32>) -> bool {
        self.in_force().iter().any(|r| {
            r.rule.cants().iter().any(|item| {
                let Cant::Cast { from, except, spell, own } = item else { return false };
                r.affects(self, Target::Player(player))
                    && from.as_ref().is_none_or(|set| (set.contains(self, &r.ctx, &r.locals, Target::Object(card))
                        && (!own || self.obj(card).owner == player)) != *except)
                    && spell.as_ref().is_none_or(|filter| {
                        let ctx = Ctx { casting: Some((card, cast_as)), x, ..r.ctx.clone() };
                        filter.matches(self, &ctx, &r.locals, Target::Object(card))
                    })
            })
        })
    }

    /// Restrictions apply even to casts offered during resolution (101.2, 307.5).
    pub(crate) fn casting_requires_sorcery_timing(&self, player: PlayerId) -> bool {
        self.in_force().iter().any(|r| r.rule.0 == Says::CastOnlyAsSorcery && r.affects(self, Target::Player(player)))
    }

    /// Matching uses the selected spell's characteristics before it moves (601.3b/e).
    pub(crate) fn casting_as_though_flash(&self, player: PlayerId, card: ObjId, cast_as: crate::card::CastAs, x: Option<u32>) -> bool {
        self.in_force().iter().any(|r| {
            let Says::CastAsThoughFlash(filter) = &r.rule.0 else { return false; };
            if !r.affects(self, Target::Player(player)) { return false; }
            let ctx = Ctx { casting: Some((card, cast_as)), x, ..r.ctx.clone() };
            filter.matches(self, &ctx, &r.locals, Target::Object(card))
        })
    }

    /// Does an effect say this creature can't attack that (508.1c)?
    pub fn cant_attack(&self, creature: ObjId, attacked: Target) -> bool {
        self.in_force().iter().any(|r| {
            self.affects(r, creature)
                && r.rule.cants().iter().any(|item| match item {
                    Cant::Attack(defender) => defender.is_empty() || defender.iter().any(|d| d.is(self, &r.ctx, &r.locals, attacked)),
                    _ => false,
                })
        })
    }

    /// Total fixed cost for this declaration (508.1h), preserving the exact recipient.
    pub(crate) fn attack_cost(&self, attacks: &[(ObjId, Target)]) -> ir::ManaCost {
        let mut total = Vec::new();
        for rule in self.in_force() {
            if let Says::AttackPayment { defender, cost } = &rule.rule.0 {
                for (creature, target) in attacks {
                    if self.affects(&rule, *creature) && defender.is(self, &rule.ctx, &rule.locals, *target) {
                        total.extend_from_slice(&cost.0);
                    }
                }
            }
        }
        ir::ManaCost(total)
    }

    /// Does an effect say this creature attacks if able, or attacks something in particular if
    /// able (508.1d)?
    pub fn must_attack(&self, creature: ObjId) -> bool {
        self.in_force().iter().any(|r| matches!(r.rule.0, Says::MustAttack(_)) && self.affects(r, creature))
    }

    /// How many of the requirements said of this creature it obeys by attacking this player,
    /// planeswalker or battle (508.1d): "attacks each combat if able" by attacking anything,
    /// "attacks Gideon Jura if able" by attacking him.
    pub fn requirements_obeyed(&self, creature: ObjId, attacked: Target) -> usize {
        self.in_force().iter().filter(|r| matches!(&r.rule.0, Says::MustAttack(defenders)
            if defenders.is_empty() || defenders.iter().any(|d| d.is(self, &r.ctx, &r.locals, attacked))) && self.affects(r, creature)).count()
    }

    /// Does an effect say damage can't be prevented (615.12)? Asked wherever damage would
    /// be prevented: what prevents it then does not (702.16e).
    pub fn unpreventable(&self) -> bool {
        self.in_force().iter().any(|r| r.rule.0 == Says::Unpreventable)
    }

    /// The permanents' prevention abilities in force that prevent this damage (615.1): a
    /// source that has left is seen as it last existed (608.2h).
    pub(crate) fn static_preventions(&self, source: ObjId, to: Target, combat: bool) -> Vec<crate::operation::ReplacementId> {
        use ir::event::DamageKind;
        let mut out = Vec::new();
        for r in self.in_force() {
            let Says::Prevent { to: recipients, by, to_or_by, kind } = &r.rule.0 else { continue };
            if matches!((kind, combat), (DamageKind::Combat, false) | (DamageKind::Noncombat, true)) { continue; }
            let recipient = recipients.iter().any(|n| n.is(self, &r.ctx, &r.locals, to));
            let mut ctx = r.ctx.clone();
            if self.obj(source).zone == crate::Zone::Gone && self.obj(source).left == Some(crate::Zone::Battlefield) { ctx.was = Some(source); }
            let dealt_by = by.iter().any(|n| n.is(self, &ctx, &r.locals, Target::Object(source)));
            let holds = if *to_or_by { recipient || dealt_by }
                else { (recipients.is_empty() || recipient) && (by.is_empty() || dealt_by) };
            let (Some(at), Some(ability)) = (r.ctx.source, r.ctx.ability) else { continue };
            let id = crate::operation::ReplacementId::from_ability(at, ability, 0);
            if holds && !out.contains(&id) { out.push(id); }
        }
        out
    }

    /// Does an effect say this permanent is exiled if it would die (614.1a)? The first that
    /// does, as a replacement its controller can choose (616.1): any of them comes to the same.
    pub(crate) fn exiled_instead_of_dying(&self, permanent: ObjId) -> Option<crate::operation::ReplacementId> {
        self.exiled_by(Says::ExiledInsteadOfDying, permanent)
    }

    /// Does an effect say this permanent is exiled if it would leave the battlefield, instead
    /// of being put anywhere else (614.1a, 702.84a)? Only a resolved effect says it of one.
    pub(crate) fn exiled_instead_of_leaving(&self, permanent: ObjId) -> Option<crate::operation::ReplacementId> {
        if !self.lasting.iter().any(|l| l.rule.0 == Says::ExiledInsteadOfLeaving) { return None; }
        self.exiled_by(Says::ExiledInsteadOfLeaving, permanent)
    }

    fn exiled_by(&self, says: Says, permanent: ObjId) -> Option<crate::operation::ReplacementId> {
        self.in_force().iter().find(|r| r.rule.0 == says && self.affects(r, permanent)).map(|r| crate::operation::ReplacementId::from_ability(r.ctx.source.unwrap_or(permanent), r.ctx.ability.clone().unwrap_or(crate::binding::AbilityKey { origin: crate::binding::Origin::Own, index: 0 }), 0))
    }

    /// Is this attacking creature unblockable by landwalk (702.14c)? "A creature with landwalk
    /// can't be blocked as long as the defending player controls at least one land" with the
    /// type it names. The defending player is the one it attacks, or the controller of the
    /// planeswalker it attacks (506.2, 802.2a): another player's lands don't matter. The
    /// lands are asked as they are now, through the layers (613.1d), with the shared selector
    /// evaluation; each landwalk ability is asked on its own (702.14d).
    fn walks(&self, attacker: ObjId) -> bool {
        let Some(defending) = self.combat.attackers.iter().find(|(a, _)| *a == attacker).map(|(_, t)| self.defending(*t)) else {
            return false;
        };
        let seen = self.seen_for(attacker, crate::layers::Aspect::Abilities);
        let ctx = Ctx::new(self.controller(attacker), attacker);
        seen.keywords.iter().filter_map(|k| k.landwalk()).any(|kind| {
            self.battlefield.iter().any(|land| {
                self.controller(*land) == defending
                    && self.is_a(*land, ir::CardType::Land)
                    // lowered with the keyword: only qualities the selector evaluates on any object
                    && kind.matches(self, &ctx, &expr::NoLocals, Target::Object(*land))
            })
        })
    }

    /// Does an effect say this creature can't block that one, or that one can't be blocked by
    /// this, given every creature declared as blocking it (509.1b)? "By more than one creature"
    /// and "except by three or more creatures" count them.
    pub fn cant_block(&self, blocker: ObjId, attacker: ObjId, all_blocking_it: &[ObjId]) -> bool {
        self.block_restriction(blocker, attacker, all_blocking_it).is_some()
    }

    /// The restriction [`Game::cant_block`] finds, said with its rule: the attacker's
    /// protection (702.16f), landwalk (702.14c), a keyword whose rule is a sentence (flying,
    /// 702.9b), or an effect (509.1b).
    pub(crate) fn block_restriction(&self, blocker: ObjId, attacker: ObjId, all_blocking_it: &[ObjId]) -> Option<String> {
        let (b, a) = (&self.obj(blocker).chars.name, &self.obj(attacker).chars.name);
        if self.protected_from(Target::Object(attacker), &Ctx::new(self.controller(blocker), blocker)) {
            return Some(format!("{a} has protection from a quality {b} has (702.16f, 509.1b)"));
        }
        if self.walks(attacker) {
            return Some(format!("{a} has landwalk, and the defending player controls a land of that kind (702.14c, 509.1b)"));
        }
        // how many of the creatures blocking it the description names, and how many it asks for
        let named = |r: &InForce, by: &Blockers| -> (usize, usize) {
            match by {
                Blockers::AtLeast(n, set) => {
                    (all_blocking_it.iter().filter(|b| set.contains(self, &r.ctx, &r.locals, Target::Object(**b))).count(), *n)
                }
                Blockers::One(noun) => (noun.is(self, &r.ctx, &r.locals, Target::Object(blocker)) as usize, 1),
            }
        };
        let restricting = self.in_force().into_iter().find(|r| {
            r.rule.cants().iter().any(|item| match item {
                Cant::Block { blocked, except } => {
                    self.affects(r, blocker) && (blocked.as_ref().is_none_or(|b| b.is(self, &r.ctx, &r.locals, Target::Object(attacker))) != *except)
                }
                Cant::BeBlocked { by, except } => {
                    let by_these = by.iter().any(|b| {
                        let (have, want) = named(r, b);
                        have >= want
                    });
                    self.affects(r, attacker) && if *except { !by_these } else { by.is_empty() || by_these }
                }
                _ => false,
            })
        })?;
        let source = restricting.ctx.source.map(|s| self.obj(s).chars.name.clone());
        Some(match restricting.keyword.and_then(|k| crate::keyword::sentence_rule(k).map(|rule| (k, rule))) {
            Some((keyword, rule)) => {
                let holder = if restricting.ctx.source == Some(blocker) { b } else { a };
                format!("{holder} has {} ({rule}, 509.1b)", keyword.name().to_lowercase())
            }
            None => match source {
                Some(source) => format!("{source} says it can't, or not that way (509.1b)"),
                None => "an effect says it can't, or not that way (509.1b)".into(),
            },
        })
    }
}

#[cfg(test)]
mod tests;

impl crate::retention::Trace for Lasting {
    fn trace(&self, visit: &mut dyn FnMut(ObjId)) {
        let Self { ctx, affected, locals, rule: _, lifetime: _, untap_steps: _ } = self;
        ctx.trace(visit);
        affected.trace(visit);
        locals.trace(visit);
    }
}
