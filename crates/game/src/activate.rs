//! Activated abilities (CR 602): "[Cost]: [Effect.] [Activation instructions (if any).]"
//!
//! To activate an ability is to put it onto the stack and pay its costs (602.2). Only the
//! controller of the object whose ability it is can, whenever they have priority (113.3b). A
//! cost is an action or payment (118.1): each part of one is an action of the lexicon the player
//! performs, and it can be paid only when all of it can be (118.3), which is asked of the
//! same declarations the actions ask ([`Game::can`]).
//!
//! A mana ability (605.1a) is an activated ability that has no target and could add mana: it
//! does not use the stack and resolves as soon as it is activated (605.3b). A land with a
//! basic land type has one that no text gives it (305.6). A player activates one while they
//! have priority or while casting a spell or activating an ability (605.3a). Complete payment
//! plans resolve without intervening priority, trigger placement or state-based actions.

pub(crate) mod timing;

use crate::ability::Template;
use crate::card::CardDef;
use crate::event::Target;
use crate::announcement::target_slots;
use crate::select::Ctx;
use crate::cost::{Payment, Plan};
use crate::state::{Bound, Game, ObjId, PlayerId, Pool, Spent, Triggered, Zone};
use crate::Res;
use ir::action::Objects;
use ir::mana::{ManaCost, ManaSymbol};
use ir::{Ability, Action, CardType, Sort, Subtype, Verb};
use std::sync::OnceLock;

/// The keyword family retained separately from the ordinary activated program (702.177,
/// 702.193). Definitions, announcements and event predicates share this identity.
pub(crate) fn keyword_activation(ability: &Ability) -> Option<(ir::Keyword, Ability)> {
    let Ability::Prefixed { keyword, ability } = ability else { return None };
    if !matches!(keyword.name(), "Exhaust" | "Power-up") { return None; }
    let Ability::Activated { cost, effect } = ability.as_ref() else { return None };
    let mut effect = effect.clone();
    effect.0.push(ir::Sentence::Restrictions(vec![ir::Restriction::Once]));
    Some((*keyword, Ability::Activated { cost: cost.clone(), effect }))
}

/// A predicate on an activated occurrence, independent of the source's later characteristics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActivationFilter(ir::Filter);
impl ActivationFilter {
    pub(crate) fn compile(set: &ir::ObjectSet) -> Res<Self> {
        fn supported(filter: &ir::Filter) -> bool {
            match filter {
                ir::Filter::Atom(ir::Atom::Keyword(k)) => matches!(k.name(), "Exhaust" | "Power-up"),
                ir::Filter::Atom(ir::Atom::AbilityKind(ir::selector::AbilityKind::Activated | ir::selector::AbilityKind::Mana)) => true,
                ir::Filter::All(parts) | ir::Filter::Any(parts) => parts.iter().all(supported),
                ir::Filter::Not(part) => supported(part),
                _ => false,
            }
        }
        if *set != (ir::ObjectSet { filter: set.filter.clone(), ..ir::ObjectSet::of(ir::Domain::Ability) }) || !supported(&set.filter) {
            return crate::unsupported("activation selector: unsupported ability family or source relation");
        }
        Ok(Self(set.filter.clone()))
    }
    pub(crate) fn matches(&self, keyword: Option<ir::Keyword>, mana: bool) -> bool {
        fn matches(filter: &ir::Filter, keyword: Option<ir::Keyword>, mana: bool) -> bool {
            match filter {
                ir::Filter::Atom(ir::Atom::Keyword(k)) => keyword == Some(*k),
                ir::Filter::Atom(ir::Atom::AbilityKind(ir::selector::AbilityKind::Activated)) => true,
                ir::Filter::Atom(ir::Atom::AbilityKind(ir::selector::AbilityKind::Mana)) => mana,
                ir::Filter::All(parts) => parts.iter().all(|p| matches(p, keyword, mana)),
                ir::Filter::Any(parts) => parts.iter().any(|p| matches(p, keyword, mana)),
                ir::Filter::Not(part) => !matches(part, keyword, mana),
                _ => false,
            }
        }
        matches(&self.0, keyword, mana)
    }
    fn family_only(&self) -> bool {
        fn family(filter: &ir::Filter) -> bool {
            match filter {
                ir::Filter::Atom(ir::Atom::Keyword(_)) => true,
                ir::Filter::All(parts) | ir::Filter::Any(parts) => parts.iter().all(family),
                ir::Filter::Not(part) => family(part),
                _ => false,
            }
        }
        family(&self.0)
    }
    pub(crate) fn only_exhaust(&self) -> bool {
        self.0 == ir::Filter::Atom(ir::Atom::Keyword(ir::Keyword::from_name("Exhaust").unwrap()))
            || self.0 == ir::Filter::All(vec![ir::Filter::Atom(ir::Atom::Keyword(ir::Keyword::from_name("Exhaust").unwrap()))])
    }
}

/// A static permission changes whether earlier uses prohibit beginning an activation.
/// Conditions are queried before reservation; beginning the allowed activation cannot
/// invalidate its already-made announcement (702.177b, 602.2).
#[derive(Debug, Clone)]
pub(crate) struct UnusedPermission {
    subject: crate::program::registration::Subject,
    scope: crate::functions::Scope,
    filters: Vec<(crate::program::registration::When, ActivationFilter)>,
}
pub(crate) fn unused_permissions(def: &CardDef) -> Vec<crate::program::registration::Registered<UnusedPermission>> {
    use crate::program::registration::{Registered, Subject, When};
    def.abilities.iter().enumerate().filter_map(|(index, ability)| {
        let stated = crate::statement::read(ability)?;
        if !stated.acts.iter().any(|(a, _, _)| matches!(a, ir::Action::Static(s) if matches!(s.as_ref(), ir::StaticAct::ActivateAsThoughUnused(_)))) { return None; }
        let result = (|| {
            let support = crate::select::Support::definition(def, Some(crate::binding::AbilityKey::printed(index)));
            let mut bind = crate::program::expr::linked(&support);
            let subject = Subject::compile(&stated.subject, &support, &mut bind)?;
            let scope = def.scope(index).expect("a declared permission")?;
            let mut filters = Vec::new();
            for (action, when, scale) in stated.acts {
                let ir::Action::Static(action) = action else { return crate::unsupported("unused activation permission: mixed statement"); };
                let ir::StaticAct::ActivateAsThoughUnused(set) = action.as_ref() else { return crate::unsupported("unused activation permission: mixed static statement"); };
                if !matches!(scale, crate::statement::Scale::Once) { return crate::unsupported("unused activation permission: scaled permission"); }
                let filter = ActivationFilter::compile(set)?;
                if !filter.family_only() { return crate::unsupported("unused activation permission: only keyword-family predicates"); }
                filters.push((When::compile(&when, &support, &mut bind)?, filter));
            }
            Ok(UnusedPermission { subject, scope, filters })
        })();
        Some(Registered { ability: index, result })
    }).collect()
}
#[derive(Debug, Clone)]
pub(crate) struct Modifier {
    keyword: Option<ir::Keyword>,
    sources: crate::program::expr::Set,
    scope: crate::functions::Scope,
    change: Modification,
}
#[derive(Debug, Clone)]
enum Modification { Cost { more: bool, amount: u32 }, Additional(crate::program::expr::Amount) }
pub(crate) fn modifiers(def: &CardDef) -> Vec<crate::program::registration::Registered<Modifier>> {
    use crate::program::{expr, registration::Registered};
    use ir::statics::{AbilitiesOf, AbilityRule};
    def.abilities.iter().enumerate().filter_map(|(index, ability)| {
        let Ability::Text(ir::Effect(rows)) = ability else { return None };
        let [ir::Sentence::Static(stated)] = rows.as_slice() else { return None };
        let ir::statics::Static::ActivatedAbilities { of, rule, this_turn } = stated.as_ref() else { return None };
        if matches!(rule, AbilityRule::CantBeActivated { .. }) { return None; }
        let result = (|| {
            if *this_turn { return crate::unsupported("activation modifier: temporary effects need a retained duration"); }
            let (keyword, of) = match of {
                AbilitiesOf::KeywordOf { keyword, of } if matches!(keyword.name(), "Exhaust" | "Power-up") => (Some(*keyword), of),
                AbilitiesOf::Of { kind: ir::action::AbilityKind::Activated, of } => (None, of),
                _ => return crate::unsupported("activation modifier: unsupported ability group"),
            };
            let [ir::statics::Who::Set(set)] = of.items.as_slice() else { return crate::unsupported("activation modifier: multiple or relative sources"); };
            let support = crate::select::Support::definition(def, Some(crate::binding::AbilityKey::printed(index)));
            let mut bind = expr::linked(&support);
            let sources = expr::Set::compile(set, &support, &mut bind)?;
            let change = match rule {
                AbilityRule::CostChange { more, by } => {
                    let mut amount = 0;
                    for symbol in &by.0 {
                        let ManaSymbol::Generic(n) = symbol else { return crate::unsupported("activation modifier: only generic mana cost changes"); };
                        amount += n;
                    }
                    Modification::Cost { more: *more, amount }
                },
                AbilityRule::AdditionalTimes(n) => Modification::Additional(expr::Amount::compile(n, &support, &mut bind)?),
                AbilityRule::CantBeActivated { .. } => unreachable!("prohibitions have their own owner"),
                AbilityRule::DontTrigger => return crate::unsupported("trigger suppression: ability-family rule not implemented (603.2)"),
            };
            Ok(Modifier { keyword, sources, scope: def.scope(index).expect("a declared modifier")?, change })
        })();
        Some(Registered { ability: index, result })
    }).collect()
}
#[derive(Default)]
struct Modifications { increase: u32, reduction: u32, additional: u32 }

impl CardDef {
    pub(crate) fn activation_modifier_ok(&self, index: usize) -> Option<Res<()>> {
        self.activation_modifiers.iter().find(|p| p.ability == index).map(|p| p.result.as_ref().map(|_| ()).map_err(Clone::clone))
    }
    pub(crate) fn unused_permission_ok(&self, index: usize) -> Option<Res<()>> {
        self.unused_permissions.iter().find(|p| p.ability == index).map(|p| p.result.as_ref().map(|_| ()).map_err(Clone::clone))
    }
}

impl Game {
    fn activation_modifications(&self, source: ObjId, key: crate::binding::AbilityKey) -> Modifications {
        let keyword = self.activation_keyword(source, key);
        let changed = self.membership_changed();
        let mut changes = Modifications::default();
        for grant in &self.battlefield {
            for (origin, def) in self.origins(*grant, crate::ability::View::Effective { membership_changed: changed }) {
                for registration in &def.activation_modifiers {
                    let Ok(modifier) = &registration.result else { continue };
                    if !modifier.scope.contains(self.obj(*grant).zone) || modifier.keyword.is_some_and(|k| Some(k) != keyword) { continue; }
                    let ctx = Ctx::new(self.controller(*grant), *grant).with_key(crate::binding::AbilityKey { origin: origin.clone(), index: registration.ability });
                    if !modifier.sources.contains(self, &ctx, &crate::program::expr::NoLocals, Target::Object(source)) { continue; }
                    match &modifier.change {
                        Modification::Cost { more: true, amount } => changes.increase += amount,
                        Modification::Cost { more: false, amount } => changes.reduction += amount,
                        Modification::Additional(n) => changes.additional += n.evaluate(self, &ctx, &crate::program::expr::NoLocals),
                    }
                }
            }
        }
        changes
    }
    fn as_unused(&self, player: PlayerId, source: ObjId, key: crate::binding::AbilityKey) -> bool {
        let keyword = self.activation_keyword(source, key);
        let changed = self.membership_changed();
        self.battlefield.iter().any(|grant| self.origins(*grant, crate::ability::View::Effective { membership_changed: changed }).into_iter().any(|(origin, def)| {
            def.unused_permissions.iter().any(|registration| {
                let Ok(permission) = &registration.result else { return false };
                if !permission.scope.contains(self.obj(*grant).zone) { return false; }
                let ctx = Ctx::new(self.controller(*grant), *grant).with_key(crate::binding::AbilityKey { origin: origin.clone(), index: registration.ability });
                permission.subject.contains(self, &ctx, Target::Player(player))
                    // Family-only filters have no mana-kind predicates.
                    && permission.filters.iter().any(|(when, filter)| when.holds(self, &ctx) && filter.matches(keyword, false))
            })
        }))
    }
    pub(crate) fn activation_keyword(&self, source: ObjId, key: crate::binding::AbilityKey) -> Option<ir::Keyword> {
        self.definition(source, &key.origin)?.activation_keywords.get(key.index).copied().flatten()
    }
    pub(crate) fn reserve_activation(&mut self, player: PlayerId, source: ObjId, key: crate::binding::AbilityKey, mana: bool, lifetime: bool) -> crate::payment::Undo {
        let lifetime = lifetime.then(|| {
            let position = self.obj(source).activation_uses.len();
            self.obj_mut(source).activation_uses.push(key.clone());
            position
        });
        // A fixture can declare consumed limits without inventing the actor or event
        // order of earlier activations. Each inventory therefore owns its own index.
        let undo = crate::payment::Undo::Ability { reservation: self.activated.len(), started: self.activation_started.len(), lifetime };
        let keyword = self.activation_keyword(source, key.clone());
        if mana {self.active_mana_abilities.push((source,key.clone()));}
        self.activated.push((source, key));
        self.activation_started.push((player, keyword, mana));
        undo
    }
    pub(crate) fn reduce_power_up(&self, source: ObjId, key: crate::binding::AbilityKey, cost: Plan) -> Plan {
        if self.activation_keyword(source, key.clone()).is_some_and(|k| k.name() == "Power-up")
            && self.obj(source).entered == self.turn {
            if let Some(mana_cost) = self.def(source).and_then(CardDef::cost) { return cost.reduced_by(mana_cost, key); }
        }
        cost
    }


}

/// The basic land types and the intrinsic ability an object with the land card type and that
/// type has, "{T}: Add [mana symbol]" (305.6): the rule's own words, read by the grammar.
const BASIC: &[(&str, &str)] = &[
    ("Plains", "{T}: Add {W}."),
    ("Island", "{T}: Add {U}."),
    ("Swamp", "{T}: Add {B}."),
    ("Mountain", "{T}: Add {R}."),
    ("Forest", "{T}: Add {G}."),
];

fn rules() -> &'static Vec<(Subtype, Template)> {
    static READ: OnceLock<Vec<(Subtype, Template)>> = OnceLock::new();
    crate::token::init(&READ, || {
        BASIC
            .iter()
            .map(|(land_type, sentence)| {
                let subtype = Subtype::from_name(land_type).unwrap_or_else(|| panic!("{land_type} is a land type"));
                let def = CardDef::new(land_type, "", "Land", sentence, None)
                    .unwrap_or_else(|e| panic!("the grammar reads rule 305.6, \"{sentence}\": {e}"));
                let [_] = def.abilities.as_slice() else { panic!("rule 305.6 gives one ability") };
                (subtype, Template::keep(def))
            })
            .collect()
    })
}

/// The definitions of the mana abilities rule 305.6 gives what this is: one for each basic
/// land type a land has, compiled once and shared as any ability's definition is.
pub(crate) fn intrinsic(seen: &crate::layers::Seen) -> impl Iterator<Item = Template> + '_ {
    rules().iter().filter(|(subtype, _)| seen.intrinsic_abilities && seen.is(CardType::Land) && seen.subtypes.contains(subtype)).map(|(_, what)| *what)
}

/// Authenticate the retained rule identity, even if the source later loses its type.
pub(crate) fn basic_mana_template(what: Template) -> bool {
    rules().iter().any(|(_, rule)| *rule == what)
}

/// The colors a printed basic land type supplies (305.6, 903.5d), read from the same
/// immutable rule definitions that supply its intrinsic mana abilities.
pub(crate) fn basic_land_colors(subtypes: &[Subtype]) -> impl Iterator<Item = ir::Color> + '_ {
    rules().iter().filter(|(subtype, _)| subtypes.contains(subtype))
        .flat_map(|(_, what)| what.def().color_identity())
}

/// An activated ability a player can begin to activate: its parts, what its text targets, in
/// order (601.2c), and whether it is a mana ability (605.1a).
pub(crate) struct Activation {
    pub cost: Plan,
    pub lifetime: bool,
    pub slots: Vec<crate::announcement::TargetSpec>,
    pub mana: bool,
}

/// What a part of a cost has its player choose: what it could be paid with, and how many of
/// them it takes.
pub(crate) type Choosing = (Vec<ObjId>, crate::cost::Selection);

/// Actions with selected objects that pay a cost (118.1, 601.2h).
pub(crate) fn object_payment(action: &Action) -> Option<(Verb, &Objects)> {
    match action {
        Action::Sacrifice(objects) | Action::Discard(objects) | Action::Exile(objects) | Action::Tap(objects) => Some((action.verb().unwrap(), objects)),
        Action::Move { verb: ir::action::MoveVerb::Return, moves, from: None } => match moves.as_slice() {
            [(objects, ir::action::Destination::Hand(ir::Whose::Owner | ir::Whose::Owners))]
                if objects.items.iter().all(|s| s.set.domain == ir::Domain::Permanent && s.set.zone.is_none()) => Some((Verb::Move, objects)),
            _ => None,
        },
        _ => None,
    }
}

impl Game {
    /// The abilities an object has: printed rows, activated meanings of keywords, the
    /// intrinsic mana ability of each basic land type a land has (305.6), then those effects
    /// give it (613.1f). Derived abilities preserve the indices of printed rows;
    /// `CardDef::ability_row` gives their display order. A place in this list names an
    /// ability only now: what is kept of one is its key ([`Game::key`]).
    pub fn abilities(&self, o: ObjId) -> Vec<crate::ability::Member<'_>> { self.members(o) }

    /// The context of following the ability at this place among [`Game::abilities`].
    pub(crate) fn ability_ctx(&self, player: PlayerId, source: ObjId, index: usize) -> Ctx<'static> {
        let ctx = Ctx::new(player, source);
        let mut ctx = self.key(source, index).map_or(ctx.clone().with_ability(index), |key| ctx.with_key(key));
        ctx.link_generation = self.link_generation(ctx.source, ctx.ability.as_ref());
        ctx
    }

    /// Whose ability it is to activate: only the controller of the object (602.2), and of a
    /// permanent, since abilities of other objects function only where they say (113.6); of a
    /// card in a hand or graveyard whose ability functions there (113.6m), its owner, a card there
    /// having no controller (108.4a).
    fn activator(&self, player: PlayerId, source: ObjId, index: Option<usize>) -> Result<(), String> {
        if matches!(self.obj(source).zone, Zone::Graveyard | Zone::Hand | Zone::Exile) && index.is_some_and(|i| self.functions(source, i)) {
            if self.obj(source).owner != player {
                return Err("an ability of a card outside the battlefield is activated by its owner (108.4a, 602.2)".into());
            }
            return Ok(());
        }
        // an ability functioning only in the zone it moves its card out of (113.6m), as cycling
        // in its owner's hand (702.29a), is refused for that wherever else the card is
        let elsewhere = index.and_then(|i| self.key(source, i).ok()).and_then(|key| self.scope_of(source, key))
            .is_some_and(|scope| matches!(scope, crate::functions::Scope::In(zone) if zone != Zone::Battlefield));
        if elsewhere || index.is_some_and(|i| self.is(Target::Object(source), Sort::Permanent)
            && self.abilities(source).get(i).is_some() && !self.functions(source, i)) {
            return Err("that ability functions only in the zone it moves its card out of (113.6m)".into());
        }
        if !self.is(Target::Object(source), Sort::Permanent) {
            return Err("an ability of a permanent on the battlefield is activated (113.6)".into());
        }
        if self.controller(source) != player {
            return Err("only an object's controller can activate its ability (602.2)".into());
        }
        Ok(())
    }

    /// Does this activation instruction allow activating now (602.1b, 602.5)? "Only as a
    /// sorcery": when its controller could cast a sorcery (602.5d). "Only once each turn": of
    /// this object's ability, whoever controls it (602.5b).
    fn allowed(&self, player: PlayerId, source: ObjId, index: usize, r: &crate::announcement::Limit) -> Result<(), String> {
        let key = self.key(source, index)?;
        let times = self.activated.iter().filter(|a| **a == (source, key.clone())).count() as u32;
        match r {
            crate::announcement::Limit::Sorcery if !self.sorcery_timing(player) => {
                Err("that ability is activated only as a sorcery: in a main phase of your own turn with the stack empty (602.5d)".into())
            }
            crate::announcement::Limit::If(condition) if !condition.evaluate(self, &self.ability_ctx(player, source, index), &crate::program::expr::NoLocals) => {
                // a level bar is activated only from the level before it (716.2a)
                match self.abilities(source).get(index).and_then(|a| crate::class::bar_level(a)) {
                    Some(level) => Err(format!("this Class becomes level {level} only from level {} (716.2a)", level - 1)),
                    None => Err("the activation condition does not hold (602.5)".into()),
                }
            },
            crate::announcement::Limit::Lifetime => {
                let limit = 1 + self.activation_modifications(source, key.clone()).additional;
                let used = self.obj(source).activation_uses.iter().filter(|k| **k == key).count() as u32;
                if used < limit || self.as_unused(player, source, key) { return Ok(()); }
                if limit == 1 { Err("that ability is activated only once for this object (602.5b, 702.177a, 702.193a)".into()) }
                else { Err(format!("that ability has already been activated {limit} times for this object (602.5b, 702.193a)")) }
            }
            crate::announcement::Limit::Once if times >= 1 => Err("that ability is activated only once each turn (602.1b)".into()),
            crate::announcement::Limit::Times(n) if times >= *n => {
                Err(format!("that ability is activated no more than {n} times each turn (602.1b)"))
            }
            _ => Ok(()),
        }
    }

    /// Validate the whole cost and begin its retained commitment (118.3, 601.2h).
    /// Casting and activation share choice validation, mana spending and cost inputs;
    /// each subsequent part commits once, even when library ordering interrupts it.
    pub(crate) fn prepare_cost(
        &mut self, ctx: Ctx<'static>, cost: &Plan, paying: &[Vec<ObjId>], with: Option<&Pool>, on: Spent,
    ) -> crate::play::ActionResult<crate::payment::Commitment> {
        let player = ctx.controller;
        // each artifact or creature chosen for a waterbend cost pays {1} of it (701.67a)
        let cost = &cost.clone().bent(paying)?;
        // Objects used for costs are chosen at payment (601.2h). Mana production may have
        // created one, or consumed one the player proposed to use. Validate against this state.
        let (payments, choices) = cost.payments(self, &ctx, Some(paying))?;
        if paying.len() != choices {
            return Err(format!("that cost asks for {choices} choice(s), {} given", paying.len()).into());
        }
        let mana = self.payable_but_mana(player, ctx.object_source(), &payments)?;
        let mana = cost.requirement(mana);
        let audit_payment = self.audit_payment_start(player, &mana, with, on);
        let (left, spent) = self.spend_with(&self.players[player].pool, &mana, with, on).map_err(|e| {
            let crate::play::ActionError::Refused(reason) = &e else { return e };
            if reason != crate::mana::UNPAYABLE && reason != crate::mana::UNCHOSEN { return e; }
            // the total cost, as increases, reductions and then a minimum left it (601.2f), as
            // the payment computes it; generic first and as one amount, as a cost is written
            let symbols = &mana.symbols.0;
            let written: u32 = symbols.iter().map(|s| if let ManaSymbol::Generic(n) = s { *n as u32 } else { 0 }).sum();
            let reduced = written.saturating_sub(mana.reduction);
            // a {2/C} symbol is one mana or two: with one, the minimum is a bound, not a sum
            let two_hybrid = symbols.iter().any(|s| matches!(s, ManaSymbol::TwoHybrid(_)));
            let typed = symbols.iter().filter(|s| !matches!(s, ManaSymbol::Generic(_))).count() as u32;
            let generic = if two_hybrid { reduced } else { reduced.max(mana.minimum.saturating_sub(typed)) };
            let raised = !two_hybrid && generic > reduced;
            let generic = (generic > 0).then(|| ManaSymbol::Generic(generic as _));
            let shown: Vec<_> = generic.iter().chain(symbols.iter().filter(|s| !matches!(s, ManaSymbol::Generic(_)))).collect();
            let total: String = if shown.is_empty() { "{0}".into() } else { shown.iter().map(|s| format!("{{{}}}", s.text())).collect() };
            let mut why: Vec<String> = Vec::new();
            if !cost.changes().is_empty() { why.push(format!("{} than it was", cost.changes())); }
            if raised { why.push(format!("raised to its minimum of {{{}}}", mana.minimum)); }
            if two_hybrid && mana.minimum > typed { why.push(format!("at least {{{}}}", mana.minimum)); }
            if why.is_empty() { format!("{e}: {total}").into() } else { format!("{e}: {total} in total, {} (601.2f)", why.join(", ")).into() }
        })?;
        self.audit_payment_finish(audit_payment, &left);
        let mut bound = Bound::default();
        bound.inputs.push((crate::program::input::Input::Convoked,cost.convokers(paying)));
        if cost.reveals_source() { bound.revealed.push(ctx.object_source()); }
        let mut cost_index = 0;
        for p in &payments {
            if let Payment::Objects { objects, .. } = p {
                // Costs that reveal hand cards grant visibility until the announced
                // spell or ability leaves the stack, just like revealing its source
                // (701.20a). Beholding a battlefield permanent reveals nothing.
                if matches!(p, Payment::Objects { verb: ir::Verb::Reveal | ir::Verb::Behold, .. }) {
                    for &card in objects {
                        if self.obj(card).zone == Zone::Hand && !bound.revealed.contains(&card) { bound.revealed.push(card); }
                    }
                }
                let things: Vec<_> = objects.iter().copied().map(Target::Object).collect();
                bound.inputs.push((crate::program::input::Input::Cost(cost_index), things.clone()));
                let defenders = objects.iter().filter_map(|o| self.combat.attackers.iter().find(|(a,_)| a == o).map(|(_,t)| *t)).collect();
                bound.inputs.push((crate::program::input::Input::CostDefender(cost_index), defenders));
                cost_index += 1;
            }
        }
        if payments.iter().any(|p| matches!(p, Payment::RemoveCounters { on: Target::Object(on), .. } if *on == ctx.object_source())) {
            bound.inputs.push((crate::program::input::Input::CostSource, vec![Target::Object(ctx.object_source())]));
        }
        // Record the actual payment, including colored/colorless restricted mana.
        let paid = crate::state::PaidMana::between(&self.players[player].pool, &left, &spent);
        // Spend mana once; the retained commitment owns the remaining parts (601.2h).
        *self.mana_pool_mut(player) = left;
        Ok(crate::payment::Commitment::new(player, payments, bound, spent, paid))
    }

    /// Can the player pay everything of this that is not mana? Returns the mana that is left
    /// to pay.
    fn payable_but_mana(&self, player: PlayerId, source: ObjId, payments: &[Payment]) -> Result<ManaCost, String> {
        let said = |o: ObjId| self.obj(o).chars.name.clone();
        // The life of every part is paid together (119.4): summed exactly, wider than any part.
        let (mut mana, mut life, mut spent) = (Vec::new(), 0u64, Vec::new());
        let mut tapped = Vec::new();
        // A counter can pay only once, including across two parts of one cost (118.3,
        // 118.10). Positive loyalty costs cannot fund another cost in this announcement.
        let mut counters = std::collections::HashMap::<(Target, String), u32>::new();
        for p in payments {
            let removed = match p {
                Payment::RemoveCounters { on, kind, count } => Some((*on, kind.as_str(), *count)),
                Payment::Loyalty(o, n) if *n < 0 => Some((Target::Object(*o), "loyalty", n.unsigned_abs())),
                _ => None,
            };
            if let Some((o, kind, count)) = removed {
                let left = counters.entry((o, kind.to_owned())).or_insert_with(|| self.counters(o).get(kind).copied().unwrap_or(0));
                if !self.holds_counters(o) || *left < count {
                    let rules = if matches!(p, Payment::Loyalty(..)) { "606.6, 118.3, 118.10" } else { "118.3, 118.10" };
                    return Err(format!("{} can't have {count} {kind} counters removed to pay this cost ({rules})", match o { Target::Object(o) => said(o), Target::Player(p) => format!("player {p}") }));
                }
                *left -= count;
            }
            match p {
                Payment::Mana(c) => mana.extend(c.0.iter().copied()),
                Payment::Life(n) => life += u64::from(*n),
                Payment::Loyalty(on, count) if *count > 0 => {
                    if !self.may_place(Target::Object(*on), "loyalty", player) {
                        return Err(format!("you can't put loyalty counters on {} (606.4, 118.3)", said(*on)));
                    }
                }
                Payment::Reveal(_) | Payment::Loyalty(..) | Payment::RemoveCounters { .. } => {},
                Payment::Blight { on, count } => {
                    if !self.holds_counters(Target::Object(*on)) || (*count > 0 && !self.may_place(Target::Object(*on), "-1/-1", player)) {
                        return Err(format!("you can't put -1/-1 counters on {} (701.68b, 118.3)", said(*on)));
                    }
                }
                Payment::Tap(o) | Payment::Untap(o) => {
                    let (verb, symbol, rule) = if matches!(p, Payment::Tap(_)) { (Verb::Tap, "{T}", "107.5") } else { (Verb::Untap, "{Q}", "107.6") };
                    if verb == Verb::Tap && tapped.contains(o) {
                        return Err(format!("{} cannot be tapped twice to pay a cost (118.3)", said(*o)));
                    }
                    if verb == Verb::Tap { tapped.push(*o); }
                    if !self.can(verb, player, Target::Object(*o)) {
                        return Err(format!("{} can't pay {symbol} as it is ({rule}, 118.3)", said(*o)));
                    }
                    if *o == source && self.is_a(*o, CardType::Creature) && self.summoning_sick(*o) {
                        return Err(format!("{} has not been under your control since your turn began (602.5a)", said(*o)));
                    }
                }
                Payment::Objects { verb, objects, .. } => {
                    for o in objects {
                        let verb = if *verb == Verb::CollectEvidence { Verb::Exile } else { *verb };
                        let rule = verb.entry().rule;
                        let consumed = if verb == Verb::Tap { &mut tapped } else { &mut spent };
                        if consumed.contains(o) {
                            return Err(format!("{} pays for one thing (118.10)", said(*o)));
                        }
                        consumed.push(*o);
                        if !self.can(verb, player, Target::Object(*o)) {
                            return Err(format!("you can't {} {} ({rule}, 118.3)", verb.entry().word, said(*o)));
                        }
                    }
                }
            }
        }
        // More than any u32 is more than any life total: it can't be paid (119.4).
        if !u32::try_from(life).is_ok_and(|life| self.can_pay_life(player, life)) {
            return Err(format!("you can't pay {life} life with a life total of {} (119.4)", self.players[player].life));
        }
        Ok(ManaCost(mana))
    }

    /// Validate this supplied activation's announcement, without selecting payments or
    /// executing speculative mana abilities (602.2).
    pub(crate) fn activation(&self, player: PlayerId, source: ObjId, index: usize, modes: &[usize], x: Option<u32>) -> crate::play::ActionResult<Activation> {
        self.activator(player, source, Some(index))?;
        let mut cost = self.ability_cost(source, self.key(source, index)?)?;
        if cost.loyalty() {
            // A loyalty ability: as a sorcery could be cast, and only if no loyalty ability
            // of that permanent has been activated this turn, by any player (606.3).
            if !self.sorcery_timing(player) && !self.instant_loyalty(player, source) {
                return Err("a loyalty ability is activated in a main phase of your own turn with the stack empty (606.3)".into());
            }
            let before = self.activated.iter().any(|(o, key)| *o == source && self.ability_cost(*o, key.clone()).is_ok_and(|c| c.loyalty()));
            if before {
                return Err("a loyalty ability of that permanent has already been activated this turn (606.3)".into());
            }
        }
        let ctx = Ctx { x, ..self.ability_ctx(player, source, index) };
        let programs = crate::program::selected(self, ctx.clone(), modes)?;
        let (mut increase, mut reduction) = (0, 0);
        for change in programs.iter().flat_map(|p| &p.announcement.cost_changes) {
            let n = change.amount.evaluate(self, &ctx, &crate::program::expr::NoLocals);
            if change.more { increase += n; } else { reduction += n; }
        }
        for r in programs.iter().flat_map(|p| &p.announcement.limits) { self.allowed(player, source, index, r)?; }
        let modifications = self.activation_modifications(source, self.key(source, index)?);
        cost = cost.change_generic(increase + modifications.increase, reduction + modifications.reduction);
        cost = self.reduce_power_up(source, self.key(source, index)?, cost);

        let slots = target_slots(self, ctx.clone(), modes)?;
        let mana = mana_ability(&cost, &slots, &programs);
        if mana && self.active_mana_abilities.contains(&(source,self.key(source,index)?)) {
            return Err("that mana ability is already being activated or resolving (605.3c)".into());
        }
        if self.cant_activate(source, mana, self.activation_keyword(source, self.key(source,index)?)) { return Err("an effect prohibits activating that ability (602.5)".into()); }
        let lifetime = programs.iter().any(|p| p.announcement.limits.contains(&crate::announcement::Limit::Lifetime));
        Ok(Activation { cost, lifetime, slots, mana })
    }

    /// What paying this ability's cost asks its player to choose, in the order written: for
    /// each such part ("Sacrifice a creature", "Discard a card") what it could be paid with
    /// and how many of them it takes; and whether the rest of the cost can be paid, leaving
    /// which mana to pay.
    pub(crate) fn cost_choices(&self, ctx: Ctx<'_>, cost: &Plan) -> crate::play::ActionResult<(Vec<Choosing>, crate::mana::ReducedCost)> {
        let (player, source) = (ctx.controller, ctx.object_source());
        let (fixed, _) = cost.payments(self, &ctx, None)?;
        let mana = self.payable_but_mana(player, source, &fixed)?;
        let spent: Vec<ObjId> = fixed.iter().flat_map(|p| match p {
            Payment::Objects { objects, .. } => objects.as_slice(),
            Payment::Tap(o) if cost.selected_tap() => std::slice::from_ref(o),
            _ => &[],
        }).copied().collect();
        let choices = cost.choices(self, &ctx, &spent);
        Ok((choices, cost.requirement(mana)))
    }

    /// Activate an ability of a permanent (602.2): `index` among [`Game::abilities`], one group
    /// of targets per "target" of its text (601.2c), and one group of objects for each part of
    /// its cost that asks for a choice. An illegal activation is refused whole and nothing has
    /// happened (602.2, 733).
    pub(crate) fn activate(&mut self, player: PlayerId, source: ObjId, index: usize, targets: Vec<Vec<Target>>, paying: Vec<Vec<ObjId>>) -> crate::play::ActionResult<()> {
        self.activate_with(player, source, index, targets, paying, None, &[])
    }

    /// [`Game::activate`], the mana of the cost paid with this mana of the pool when the
    /// player says which (107.4b), and, of a mana ability whose text offers a choice of
    /// mana, adding this (`adding`: one answer for each such choice, in order).
    pub(crate) fn activate_with(
        &mut self,
        player: PlayerId,
        source: ObjId,
        index: usize,
        targets: Vec<Vec<Target>>,
        paying: Vec<Vec<ObjId>>,
        with: Option<&Pool>,
        adding: &[Vec<ManaSymbol>],
    ) -> crate::play::ActionResult<()> {
        self.activate_as(player, source, index, &[], targets, paying, with, adding)
    }

    /// [`Game::activate_with`], with these modes chosen for a modal ability (602.2b, 700.2a).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn activate_as(
        &mut self,
        player: PlayerId,
        source: ObjId,
        index: usize,
        modes: &[usize],
        targets: Vec<Vec<Target>>,
        paying: Vec<Vec<ObjId>>,
        with: Option<&Pool>,
        adding: &[Vec<ManaSymbol>],
    ) -> crate::play::ActionResult<()> {
        self.activate_plan(player, source, index, modes, targets, &[], paying, with, adding, &[], None, &[], &[], None)
    }

    /// Announce, produce mana, then pay: refusal commits none of the proposal (602.2b, 733.1).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn activate_plan(
        &mut self, player: PlayerId, source: ObjId, index: usize, modes: &[usize],
        targets: Vec<Vec<Target>>, division: &[Vec<u32>], paying: Vec<Vec<ObjId>>, with: Option<&Pool>,
        adding: &[Vec<ManaSymbol>], plan: &[crate::legal::Step], x: Option<u32>, life: &[bool], chose: &[usize], branches: Option<&[crate::cost::BranchSelection]>,
    ) -> crate::play::ActionResult<()> {
        let mut proposed = self.clone();
        proposed.activate_transaction(player, source, index, modes, targets, division, paying, with, adding, plan, x, life, chose, branches)?;
        self.adopt(proposed);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn activate_transaction(
        &mut self, player: PlayerId, source: ObjId, index: usize, modes: &[usize],
        targets: Vec<Vec<Target>>, division: &[Vec<u32>], paying: Vec<Vec<ObjId>>, with: Option<&Pool>,
        adding: &[Vec<ManaSymbol>], plan: &[crate::legal::Step], x: Option<u32>, life: &[bool], chose: &[usize], branches: Option<&[crate::cost::BranchSelection]>,
    ) -> crate::play::ActionResult<()> {
        let payment = self.announce_activation(player, source, index, modes, targets, division, paying, with, adding, plan, x, life, chose, branches)?;
        self.procedures.push(crate::procedure::Procedure::Payment(payment));
        let progress = self.drive_procedures()?;
        // a cost that can't be paid is the rules' refusal (602.5a, 118.3), not the engine's
        if let Some(reason) = progress.payment_error { return Err(reason.into()); }
        // Mana said for a choice the effect does not offer (608.2d), not mana left unsaid.
        if progress.unoffered_mana_answer && progress.unused_mana_answers == 0 {
            return Err("that ability doesn't offer the mana said for it: say one of what it offers (608.2d)".into());
        }
        let left = progress.unused_mana_answers;
        if left > 0 { return Err("that ability offers no such choice of mana (608.2d)".into()); }
        // A complete command must supply its initial mana answer. Once a different rules
        // question interrupts it, the retained procedure owns all remaining answers.
        if matches!(self.decision, Some(crate::state::Decision::Choose { player: who, choice: crate::state::Choice::Mana { .. } }) if who == player)
            && !self.mana_trigger_asks() {
            return Err("that ability offers a choice of mana: say which mana it adds (608.2d)".into());
        }
        self.expose_payments();
        Ok(())
    }

    /// Is the question asked of a triggered mana ability as it resolves (605.4a)? One the
    /// command did not say the answer to is asked, as any question of a resolving ability is.
    pub(crate) fn mana_trigger_asks(&self) -> bool {
        matches!(self.procedures.last(), Some(crate::procedure::Procedure::Resolution(at)) if matches!(at.completion, crate::state::Completion::Mana { triggered: true, .. }))
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn start_mana(&mut self, source: ObjId, player: PlayerId, invocations: Vec<crate::program::Invocation>, bound: Bound, taps_for_mana: bool,
        answers: &[Vec<ManaSymbol>], checked: bool, more: &[Vec<ManaSymbol>], triggered: bool) {
        let mut at = crate::state::Resolving::new(source, bound);
        at.payment_scope = self.payment_scope.clone().map(|mut scope| { scope.triggered=triggered; scope });
        at.completion = crate::state::Completion::Mana { source, player, taps_for_mana, answers: answers.iter().cloned().collect(), checked,
            more: more.iter().cloned().collect(), triggered, activation:(!triggered).then(||invocations.first().and_then(|i|i.ctx.ability.clone())).flatten() };
        at.execution = Some(crate::program::Resolution::new(invocations, &at.bound));
        self.start_resolution(at);
    }

    /// Announce an activation and retain its payment (602.2a–b, 602.2h). On completion a
    /// mana ability resolves at once (605.3b); any other stays on the stack. Supplied mana
    /// answers are validated against actual choices during resolution (608.2d).
    #[allow(clippy::too_many_arguments)]
    fn announce_activation(
        &mut self, player: PlayerId, source: ObjId, index: usize, modes: &[usize],
        targets: Vec<Vec<Target>>, division: &[Vec<u32>], paying: Vec<Vec<ObjId>>, with: Option<&Pool>,
        adding: &[Vec<ManaSymbol>], plan: &[crate::legal::Step], x: Option<u32>, life: &[bool], chose: &[usize], branches: Option<&[crate::cost::BranchSelection]>,
    ) -> crate::play::ActionResult<crate::payment::Payment> {
        let Activation { cost, lifetime, slots, mana } = self.activation(player, source, index, modes, x)?;
        // These answers belong only to a mana ability's immediate resolution (605.3b).
        // Other abilities resolve later, when any mana choice must be answered then.
        if !mana && !adding.is_empty() {
            return Err("that ability offers no choice of mana during activation (605.3b, 608.2d)".into());
        }
        // The announced X is locked into the cost and read by this activation alone (107.3a).
        // So are the alternatives of it chosen (602.2b, 601.2b) and the way each Phyrexian
        // symbol of it is paid (118.13a).
        let chose=cost.resolve_branches(self, &self.ability_ctx(player,source,index),chose,branches)?;
        let chose=chose.as_slice();
        let announced = cost.said_chosen(self, &self.ability_ctx(player, source, index), chose);
        let cost = cost.announced(x)?.chosen(chose)?.paid_with_life(life)?;
        if !plan.is_empty() && !cost.has_mana() {
            return Err("there is no mana payment during which to activate mana abilities (605.3a)".into());
        }
        let taps_for_mana = mana && cost.taps();
        let ctx = Ctx { x, ..self.ability_ctx(player, source, index) };
        let key = self.key(source, index)?;
        self.legal_targets(&ctx, &slots, &targets)?;
        let division = self.legal_division(&ctx, &slots, &targets, division)?;
        // Supplied mana answers are checked against each actual production as it resolves
        // (605.3b, 608.2d), after costs and prerequisite instructions have executed.
        let invocations = crate::program::capture(self, ctx.clone()).and_then(|c| c.select(modes))?;
        let mut modes = modes.to_vec();
        modes.sort_unstable();
        let triggers = self.triggered.len();
        let mut bound = Bound::default();
        if cost.reveals_source() || self.obj(source).zone == Zone::Hand {
            bound.revealed.push(source);
            self.disclose(crate::observation::Kind::Revealed, player, &[source]);
        }
        let ability = (!mana).then(|| self.ability_on_stack(&Triggered { source: Some(source), controller: player, origin: key.origin.clone(), index: key.index, bound, delayed: None, captured: None }, modes, targets));
        if let Some(ability) = ability {
            let on_stack = self.obj_mut(ability);
            on_stack.x = x;
            on_stack.division = division;
            on_stack.announced = announced;
            if let Some(Ok(captured)) = &mut on_stack.invocations { for invocation in captured { invocation.ctx.x = x; } }
        }
        // Reserve this activation before nested mana activations can try the same limited
        // ability again (602.5b). A rejected proposal rolls the reservation back too.
        let keyword = self.activation_keyword(source, key.clone());
        let undo = self.reserve_activation(player, source, key, mana, lifetime);
        self.check_triggers();
        let triggers = triggers..self.triggered.len();
        let finish = crate::payment::Finish::Ability { keyword, on_stack: ability, invocations,
            taps_for_mana, adding: adding.to_vec(), checked: false, more: Vec::new() };
        let scope=self.payment_audit.as_ref().map(|_| {
            let mut scope=self.payment_scope.clone().unwrap_or_else(||Box::new(crate::payment_audit::Scope::root()));
            scope.ability=Some(index);scope.x=x;scope.life=life.to_vec();scope.prior=None;scope
        });
        Ok(crate::payment::Payment::ability(ctx, cost, finish, plan, paying, with, undo, triggers).library_tops(self.library_tops()).audit_child(scope))
    }

    /// Start a mana child without advancing its retained payment parent.
    pub(crate) fn begin_payment_mana(&mut self, player: PlayerId, step: &crate::legal::Step, interactive:bool) -> crate::play::ActionResult<()> {
        let mut proposed = self.clone();
        if !proposed.activation(player, step.source, step.ability, &[], step.x)?.mana {
            return Err("only mana abilities can be activated during casting (605.3a)".into());
        }
        let mut payment = proposed.announce_activation(player, step.source, step.ability, &[], Vec::new(), &[],
            step.paying.clone(), step.with.as_ref(), &step.adding, &step.plan, step.x, &step.life, &step.chose, step.cost_branches.as_deref())?;
        payment.interactive=interactive;
        payment.prepare(&mut proposed)?;
        // Retain this child above its parent. Its own mana window and cost
        // continuations advance through the same driver exactly once.
        proposed.procedures.push(crate::procedure::Procedure::Payment(payment));
        self.adopt(proposed);
        Ok(())
    }

}

/// A mana ability, as 605.1a defines one: no targets, it could add mana, and it isn't a
/// loyalty ability, even when its effect adds mana.
fn mana_ability(cost: &crate::cost::Plan, slots: &[crate::announcement::TargetSpec], programs: &[std::sync::Arc<crate::program::Program>]) -> bool {
    !cost.loyalty() && slots.is_empty() && programs.iter().any(|p| p.announcement.adds_mana)
        && !programs.iter().any(|p| p.announcement.library_movement)
}

#[cfg(test)]
mod explicit_mana_answer_tests {
    use super::*;
    use crate::Move;

    #[test]
    fn nonmana_activation_refuses_mana_answers_before_paying_or_creating_the_stack_object() {
        // 602.2/605.1a: this ability uses the stack. Adding is an answer for an
        // immediately resolving mana ability, not an ignored activation annotation.
        let card = CardDef::new("Life device", "{0}", "Artifact", "{T}: You gain 1 life.", None).unwrap();
        let mut g = Game::new(vec![card], 2, 20);
        let source = g.put(0, 0, Zone::Battlefield);
        let activation = Move::Activate { source, ability: 0, targets: vec![], paying: vec![] };
        let before = format!("{g:?}");
        let error = g.act(0, Move::Adding(vec![vec![ManaSymbol::Color(ir::Color::Blue)]], Box::new(activation.clone()))).unwrap_err();
        assert!(error.contains("no choice of mana during activation"), "{error}");
        assert_eq!(format!("{g:?}"), before, "refusal is atomic (733.1)");
        g.act(0, activation).unwrap();
        assert!(g.obj(source).tapped, "the valid activation pays its tap cost (602.2b)");
        assert_eq!(g.stack.len(), 1, "a nonmana ability uses the stack (605.1a)");
        assert_eq!(g.players[0].life, 20, "it has not resolved yet (608)");
    }
}
