//! Keyword-instance contributions, each with its own scope (113.6).
//! Context requirements preserve current implementation boundaries; they are not Magic rules.
use super::*;
use crate::functions::Scope;
use crate::{Res, Zone, unsupported};
use ir::token::KeywordInstance;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Declared {
    pub gives: Vec<(Scope, Gives)>,
    pub needs: Vec<Context>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Context {
    SpellsOnly,
    PermanentsOnly,
    Aura,
    GiftOnSpell,
    PrintedOnly,
    /// A recognized instance whose mechanism is not implemented in any admitted context.
    Unavailable,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Times { Once, Repeatable }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Paid { Kicked, Replicated, ReturnsToHand, AllModes, Escalated }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Departure { Any, Resolution }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Then { Rebound }
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RouteCost { Alternative(ir::Cost), Additional,
    /// The selected spell's mana cost, including its announced X (702.34a).
    ManaCost }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Substitution { Delve, Convoke }
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Replacement { Dredge(u32), Madness(ir::Cost), Sunburst }
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Gives {
    Rule(&'static Ability),
    Asked,
    Characteristic,
    Ability(Ability),
    Additional { cost: ir::mana::ManaCost, times: Times, then: Paid },
    Route { route: crate::announcement::CastRoute, cost: RouteCost },
    Substitute(Substitution),
    Destination { when: Departure, to: Zone, then: Option<Then> },
    Replacement(Replacement),
    Plot(ir::mana::ManaCost),
    /// An alternative cost the spell may be cast for rather than its mana cost (118.9): from
    /// any zone it is cast from, its evoke cost (702.74a); `from` one zone only, its warp
    /// cost, from a hand (702.185a).
    Alternative { cost: ir::Cost, from: Option<Zone> },
    /// Cast face down for {3} rather than its mana cost, and turned face up for this cost
    /// (702.37a/e, 702.168a/d); a megamorph cost puts a +1/+1 counter on it as it turns
    /// face up (702.37b).
    FaceDown { by: crate::face::Concealment, cost: ir::mana::ManaCost, megamorph: bool },
    /// The special action from a hand, and the cost of a foretold card (702.143a).
    Foretell(ir::Cost),
    /// An optional additional cost the spell is announced with (118.8b, 601.2b): bargain's,
    /// offspring's or casualty's ([`Optional`]).
    Optional { kind: Optional, cost: ir::Cost },
    /// How many of the modes printed under the keyword are chosen: Spree's one or more,
    /// Tiered's one (702.172a, 702.183a).
    Modes(ir::effect::Modes),
    Enchant,
    Fuse,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Instance {
    pub row: usize,
    pub item: KeywordInstance,
    pub declared: Res<Arc<Declared>>,
}
impl Declared {
    fn new(gives: Vec<(Scope, Gives)>, needs: Vec<Context>) -> Self { Self { gives, needs } }
    /// Is it an ability its permanent is asked to have: "a creature with flying", "target
    /// creature with a morph ability"?
    pub(super) fn held(&self) -> bool {
        self.gives.iter().any(|(_, gives)| matches!(gives, Gives::Rule(_) | Gives::Asked | Gives::Characteristic | Gives::FaceDown { .. }
            | Gives::Route { route: crate::announcement::CastRoute::Flashback, .. }))
    }
    pub(super) fn grantable(&self) -> bool {
        // Literal granted flashback costs can refer to the granting ability's choices.
        // Only the paired recipient-mana-cost definition is lowered for now.
        self.held() && !self.needs.contains(&Context::PrintedOnly)
            && !self.gives.iter().any(|(_, gives)| matches!(gives, Gives::Route { route: crate::announcement::CastRoute::Flashback, .. }))
    }
    fn context_ok(&self, def: &CardDef, item: &KeywordInstance) -> Res<()> {
        let spell = def.is(ir::CardType::Instant) || def.is(ir::CardType::Sorcery);
        // what a printed protection says is lowered as its definition is loaded ([`held`]),
        // with only the definition's links to bind a chosen value
        if crate::protection::known(item) {
            let support = crate::select::Support::definition(def, None);
            crate::protection::Protection::compile(item, &support, &mut crate::program::expr::linked(&support))?;
        }
        for need in &self.needs {
            match need {
                Context::GiftOnSpell if !spell => return unsupported("gift on a permanent: its enters trigger (702.174b)"),
                Context::SpellsOnly if !spell => return refused(item),
                Context::PermanentsOnly if spell => return refused(item),
                Context::Aura if !crate::attach::aura(def) => return refused(item),
                Context::Unavailable => return refused(item),
                _ => {},
            }
        }
        Ok(())
    }
}
fn refused<T>(item: &KeywordInstance) -> Res<T> { unsupported(format!("keyword: {}", item.keyword.name())) }
fn ability(item: &KeywordInstance) -> Res<Ability> { executable(item).ok_or_else(|| crate::Unsupported(format!("keyword: {}", item.keyword.name()))) }
fn generated(ability: Ability, held: bool, needs: Vec<Context>) -> Res<Declared> {
    let scope = crate::functions::scope(&ability, &[])?;
    let mut gives = Vec::new();
    if held { gives.push((scope, Gives::Asked)); }
    gives.push((scope, Gives::Ability(ability)));
    Ok(Declared::new(gives, needs))
}
/// Sole answer for row admission, contribution scopes and held meanings.
/// Existing readers remain the sole parsers of keyword parameters.
pub(crate) fn declare(item: &KeywordInstance) -> Res<Declared> {
    let (bf, stack, grave, hand) = (Scope::In(Zone::Battlefield), Scope::In(Zone::Stack), Scope::In(Zone::Graveyard), Scope::In(Zone::Hand));
    let plain = item.param == KeywordParam::None;
    let one = |scope, gives, needs| Ok(Declared::new(vec![(scope, gives)], needs));
    match item.keyword.name() {
        "Kicker" | "Multikicker" => {
            let Some((cost, repeatable)) = kicker_cost(item) else { return refused(item) };
            one(stack, Gives::Additional { cost: cost.clone(), times: if repeatable { Times::Repeatable } else { Times::Once }, then: Paid::Kicked }, vec![])
        }
        // "Choose one or more —", "Choose one —" of modes each with its own cost, paid as an
        // additional cost as the mode is chosen (702.172a, 702.183a, 700.2h)
        "Spree" | "Tiered" if plain => Ok(Declared::new(vec![
            (stack, Gives::Asked),
            (stack, Gives::Modes(if item.keyword.name() == "Spree" { ir::effect::Modes::OneOrMore } else { ir::effect::Modes::Exactly(1) })),
        ], vec![Context::SpellsOnly, Context::PrintedOnly])),
        // an additional cost for each mode chosen beyond the first (702.120a)
        "Escalate" => {
            let KeywordParam::Mana(cost) = &item.param else { return refused(item); };
            if !cost.0.iter().all(|s| s.mana_payments().is_some()) { return refused(item); }
            one(stack, Gives::Additional { cost: cost.clone(), times: Times::Repeatable, then: Paid::Escalated }, vec![Context::SpellsOnly, Context::PrintedOnly])
        }
        "Entwine" => {
            let KeywordParam::Mana(cost) = &item.param else { return refused(item); };
            if !cost.0.iter().all(|s| s.mana_payments().is_some()) { return refused(item); }
            one(stack, Gives::Additional { cost: cost.clone(), times: Times::Once, then: Paid::AllModes }, vec![Context::SpellsOnly, Context::PrintedOnly])
        }
        "Buyback" => {
            let KeywordParam::Mana(cost) = &item.param else { return refused(item); };
            if !cost.0.iter().all(|s| s.mana_payments().is_some()) { return refused(item); }
            Ok(Declared::new(vec![
                (stack, Gives::Additional { cost: cost.clone(), times: Times::Once, then: Paid::ReturnsToHand }),
                (stack, Gives::Asked),
                (stack, Gives::Destination { when: Departure::Resolution, to: Zone::Hand, then: None }),
            ], vec![Context::SpellsOnly, Context::PrintedOnly]))
        }
        "Flashback" => {
            let Some(cost) = flashback_cost(item) else { return refused(item) };
            Ok(Declared::new(vec![
                (grave, Gives::Route { route: crate::announcement::CastRoute::Flashback, cost: RouteCost::Alternative(cost) }),
                (stack, Gives::Destination { when: Departure::Any, to: Zone::Exile, then: None }),
            ], vec![Context::SpellsOnly]))
        }
        // "You may cast this card from your graveyard by paying [cost] rather than paying its
        // mana cost. If the harmonize cost was paid, exile this card instead of putting it
        // anywhere else any time it would leave the stack" (702.180a); tapping a creature as
        // it is paid is a choice of the route ([`CastRoute::Harmonize`], 702.180b)
        "Harmonize" => {
            let Some(cost) = route_cost(item) else { return refused(item) };
            Ok(Declared::new(vec![
                (grave, Gives::Route { route: crate::announcement::CastRoute::Harmonize { tapped: None }, cost: RouteCost::Alternative(cost) }),
                (stack, Gives::Destination { when: Departure::Any, to: Zone::Exile, then: None }),
            ], vec![Context::SpellsOnly]))
        }
        // "You may cast this card from your hand by paying [cost] rather than its mana cost",
        // and if it was, the delayed exile of the permanent it becomes (702.185a): a rule of
        // the resolving spell, created with [`crate::Game::alternative_resolved`]
        // An effect may give it to cards in a hand ("Artifact cards and red creature cards in
        // your hand have warp {2}{R}"): held there, and cast for as [`crate::Game::given_alternatives`]
        "Dash" => {
            let Some(cost) = route_cost(item) else { return refused(item) };
            Ok(Declared::new(vec![(stack, Gives::Alternative { cost, from: None }),
                (bf, Gives::Ability(dash_haste()))], vec![Context::PermanentsOnly, Context::PrintedOnly]))
        }
        "Warp" => {
            let Some(cost) = route_cost(item) else { return refused(item) };
            Ok(Declared::new(vec![(hand, Gives::Asked), (stack, Gives::Alternative { cost, from: Some(Zone::Hand) })], vec![Context::PermanentsOnly]))
        }
        // "Any time you could cast an instant during your declare blockers step, you may cast
        // this spell by paying [cost] and returning an unblocked creature you control to its
        // owner's hand rather than paying this spell's mana cost" (702.190a): its timing is
        // [`crate::Game::sneak_timing`], and the permanent it becomes enters tapped and attacking
        // (702.190b, [`crate::Game::sneak_entry`])
        "Sneak" => {
            let Some(mut cost) = route_cost(item) else { return refused(item) };
            cost.0.extend(sneak_return().0.iter().cloned());
            one(stack, Gives::Alternative { cost, from: None }, vec![Context::PrintedOnly])
        }
        // "You may cast this spell by paying [cost] and returning a tapped creature you control
        // to its owner's hand rather than paying its mana cost" (702.188a)
        "Web-slinging" => {
            let Some(mut cost) = route_cost(item) else { return refused(item) };
            cost.0.extend(web_slinging_return().0.iter().cloned());
            one(stack, Gives::Alternative { cost, from: None }, vec![Context::PrintedOnly])
        }
        // "As long as you discarded this card this turn, you may cast it from your graveyard by
        // paying [cost] rather than paying its mana cost" (702.187b): [`crate::Game::mayhem_of`]
        "Mayhem" => {
            let Some(cost) = route_cost(item) else { return refused(item) };
            one(grave, Gives::Alternative { cost, from: Some(Zone::Graveyard) }, vec![Context::PrintedOnly])
        }
        "Escape" => {
            let Some(cost) = escape_cost(item) else { return refused(item) };
            one(grave, Gives::Route { route: crate::announcement::CastRoute::Escape, cost: RouteCost::Alternative(cost) }, vec![])
        }
        "Retrace" if plain => Ok(Declared::new(vec![
            (grave, Gives::Asked),
            (grave, Gives::Route { route: crate::announcement::CastRoute::Retrace, cost: RouteCost::Additional }),
        ], vec![])),
        "Delve" | "Convoke" if plain => Ok(Declared::new(vec![
            (stack, Gives::Asked),
            (stack, Gives::Substitute(if item.keyword.name() == "Delve" { Substitution::Delve } else { Substitution::Convoke })),
        ], vec![])),
        "Intimidate" if plain => one(bf, Gives::Rule(intimidate_rule()), vec![Context::PermanentsOnly]),
        "Shadow" if plain => Ok(Declared::new(shadow_rules().iter()
            .map(|rule| (bf, Gives::Rule(rule))).collect(), vec![Context::PermanentsOnly])),
        "Split Second" if plain => Ok(Declared::new(split_second_rules().iter()
            .map(|rule| (stack, Gives::Rule(rule))).collect(), vec![])),
        // an optional additional cost the spell is announced with (702.166a): asked of the
        // spell ([`crate::keyword::bargain`]), its cost [`crate::keyword::bargain_cost`]
        "Bargain" if plain => Ok(Declared::new(vec![
            (stack, Gives::Asked),
            (stack, Gives::Optional { kind: Optional::Bargain, cost: bargain_cost_text().clone() }),
        ], vec![])),
        // "You may pay an additional [cost] as you cast this spell" and "When this permanent
        // enters, if its offspring cost was paid, create a token that's a copy of it, except
        // it's 1/1" (702.175a)
        "Offspring" => {
            let KeywordParam::Mana(mana) = &item.param else { return refused(item) };
            if mana.0.contains(&crate::cost::X) { return refused(item); }
            let ability = offspring_trigger();
            Ok(Declared::new(vec![
                (stack, Gives::Optional { kind: Optional::Offspring, cost: ir::Cost(vec![ir::CostPart::Mana(mana.clone())]) }),
                (crate::functions::scope(&ability, &[])?, Gives::Ability(ability)),
            ], vec![Context::PermanentsOnly, Context::PrintedOnly]))
        }
        // "As an additional cost to cast this spell, you may sacrifice a creature with power N
        // or greater" and "When you cast this spell, if a casualty cost was paid for it, copy
        // it. If the spell has any targets, you may choose new targets for the copy" (702.153a)
        "Casualty" => {
            let KeywordParam::Number(ir::Amount::Const(n)) = item.param else { return refused(item) };
            Ok(Declared::new(vec![
                (stack, Gives::Optional { kind: Optional::Casualty, cost: casualty_cost(n) }),
                (stack, Gives::Ability(casualty_trigger())),
            ], vec![Context::PrintedOnly]))
        }
        // "As an additional cost to cast this spell, you may tap any number of creatures you
        // control with total power N or more" (702.194a)
        "Teamwork" => {
            let KeywordParam::Number(ir::Amount::Const(n)) = item.param else { return refused(item) };
            one(stack, Gives::Optional { kind: Optional::Teamwork, cost: teamwork_cost(n) }, vec![Context::PrintedOnly])
        }
        // whose permanent has it gets speed 1, a state-based action (704.5aa): asked of the
        // battlefield ([`crate::Game::starting_engines`])
        "Start Your Engines!" if plain => one(bf, Gives::Asked, vec![Context::PermanentsOnly]),
        "Rebound" if plain => Ok(Declared::new(vec![
            (stack, Gives::Asked),
            (stack, Gives::Destination { when: Departure::Resolution, to: Zone::Exile, then: Some(Then::Rebound) }),
        ], vec![])),
        "Devoid" if plain => one(Scope::Everywhere, Gives::Characteristic, vec![Context::PrintedOnly]),
        "Changeling" if plain => one(Scope::Everywhere, Gives::Characteristic, vec![]),
        "Dredge" => {
            let Some(n) = crate::draw::dredge(item) else { return refused(item) };
            Ok(Declared::new(vec![(grave, Gives::Asked), (grave, Gives::Replacement(Replacement::Dredge(n?)))], vec![Context::PrintedOnly]))
        }
        "Sunburst" if plain => {
            // Existing entry replacement reads the prospective battlefield view (113.6h).
            Ok(Declared::new(vec![(bf, Gives::Asked), (bf, Gives::Replacement(Replacement::Sunburst))], vec![Context::PrintedOnly, Context::PermanentsOnly]))
        }
        "Cycling" | "Typecycling" => {
            let Some((cost, filter)) = cycling_cost(item) else { return refused(item) };
            generated(cycling(cost, filter), true, vec![Context::PrintedOnly])
        }
        "Transmute" => generated(ability(item)?, true, vec![Context::PrintedOnly]),
        "Soulshift" => generated(ability(item)?, true, vec![Context::PermanentsOnly]),
        "Madness" => {
            let Some(cost) = madness_cost(item) else { return refused(item) };
            let ability = madness_trigger(&cost);
            Ok(Declared::new(vec![
                (hand, Gives::Replacement(Replacement::Madness(cost))),
                (crate::functions::scope(&ability, &[])?, Gives::Ability(ability)),
            ], vec![]))
        }
        "Gift" => {
            let Some(of) = gift_of(item) else { return refused(item) };
            match gift_effect(of) {
                Some(ability) => one(stack, Gives::Ability(ability), vec![Context::GiftOnSpell]),
                // The permanent trigger is a separate earlier capability boundary even if
                // this gift's spell effect is not implemented (702.174b/j).
                None => Ok(Declared::new(vec![], vec![Context::GiftOnSpell, Context::Unavailable])),
            }
        }
        "Storm" if plain => one(stack, Gives::Ability(storm_trigger()), vec![Context::PrintedOnly]),
        "Replicate" => {
            let Some(cost) = replicate_cost(item) else { return refused(item) };
            Ok(Declared::new(vec![
                (stack, Gives::Additional { cost: cost.clone(), times: Times::Repeatable, then: Paid::Replicated }),
                (stack, Gives::Ability(replicate_trigger())),
            ], vec![]))
        }
        // "You may cast this spell by paying [cost] rather than paying its mana cost" and
        // "When this permanent enters, if its evoke cost was paid, its controller sacrifices
        // it" (702.74a)
        "Evoke" => {
            let cost = match &item.param {
                KeywordParam::Mana(mana) => ir::Cost(vec![ir::CostPart::Mana(mana.clone())]),
                KeywordParam::Cost(cost) => cost.clone(),
                _ => return refused(item),
            };
            let ability = evoke_trigger();
            Ok(Declared::new(vec![
                (stack, Gives::Alternative { cost, from: None }),
                (crate::functions::scope(&ability, &[])?, Gives::Ability(ability)),
            ], vec![Context::PermanentsOnly, Context::PrintedOnly]))
        }
        // "You may choose to pay [cost] rather than pay this spell's mana cost", the N time
        // counters it enters with, its not being a creature while it has one and their
        // removal at its controller's end step (702.176a), each only if that cost was paid
        "Impending" => {
            let KeywordParam::NumberAndMana(ir::Amount::Const(n), mana) = &item.param else { return refused(item) };
            let Some(abilities) = impending(*n) else { return refused(item) };
            let mut gives = vec![(stack, Gives::Alternative { cost: ir::Cost(vec![ir::CostPart::Mana(mana.clone())]), from: None })];
            for ability in abilities { gives.push((crate::functions::scope(&ability, &[])?, Gives::Ability(ability))); }
            Ok(Declared::new(gives, vec![Context::PermanentsOnly, Context::PrintedOnly]))
        }
        "Plot" => {
            let Some(cost) = crate::plot::cost(item) else { return refused(item) };
            one(hand, Gives::Plot(cost.clone()), vec![])
        }
        // megamorph is morph that also puts a +1/+1 counter on it as it turns up (702.37b)
        "Morph" | "Megamorph" | "Disguise" => {
            let KeywordParam::Mana(cost) = &item.param else {
                return unsupported(format!("{}: a cost that is not mana (702.37a, 702.168a)", item.keyword.name()));
            };
            if cost.0.contains(&crate::cost::X) { return unsupported(format!("{}: X in its cost (702.37f, 702.168e)", item.keyword.name())); }
            let megamorph = item.keyword.name() == "Megamorph";
            let by = if item.keyword.name() == "Disguise" { crate::face::Concealment::Disguised } else { crate::face::Concealment::Morph };
            // It functions in any zone from which the card could be played, and its effect
            // works wherever the card is face down (702.37a, 702.168a).
            one(Scope::Everywhere, Gives::FaceDown { by, cost: cost.clone(), megamorph }, vec![Context::PermanentsOnly, Context::PrintedOnly])
        }
        "Foretell" => {
            let Some(cost) = crate::foretell::cost(item) else { return refused(item) };
            one(hand, Gives::Foretell(cost), vec![Context::PrintedOnly])
        }
        "Enchant" if matches!(item.param, KeywordParam::Enchant(_)) => one(bf, Gives::Enchant, vec![Context::Aura, Context::PermanentsOnly]),
        "Fuse" if plain => one(stack, Gives::Fuse, vec![Context::SpellsOnly]),
        "Toxic" if matches!(item.param, KeywordParam::Number(ir::Amount::Const(_))) =>
            one(bf, Gives::Asked, vec![Context::PermanentsOnly]),
        "Undying" | "Persist" | "Extort" | "Living Weapon" | "Job Select" | "Prowess" | "Battle Cry" | "Renown" | "Annihilator" => generated(ability(item)?, true, vec![Context::PrintedOnly, Context::PermanentsOnly]),
        "Affinity" => generated(ability(item)?, false, vec![Context::PrintedOnly]),
        "Cumulative Upkeep" => generated(ability(item)?, true, vec![Context::PrintedOnly, Context::PermanentsOnly]),
        "Bloodthirst" => generated(ability(item)?, true, vec![Context::PrintedOnly, Context::PermanentsOnly]),
        "Hideaway" => {
            let Some(ability) = hideaway(item) else { return refused(item) };
            generated(ability, true, vec![Context::PrintedOnly, Context::PermanentsOnly])
        }
        // Ward is held, so effects can grant it (613.1f); its trigger goes with each grant
        // ([`crate::layers::granted_bound`]), and each instance triggers on its own.
        "Exalted" | "Flanking" | "Bushido" => generated(ability(item)?, true, vec![Context::PermanentsOnly]),
        "Ward" => generated(ability(item)?, true, vec![Context::PermanentsOnly]),
        // held as ward is: "gains firebending 4 until end of turn" grants the trigger (613.1f)
        "Firebending" => generated(ability(item)?, true, vec![Context::PermanentsOnly]),
        "Mobilize" => generated(ability(item)?, true, vec![Context::PermanentsOnly]),
        "Partner" | "Equip" | "Level Up" | "Ninjutsu" | "Crew" | "Saddle" | "Station" => generated(ability(item)?, false, vec![Context::PermanentsOnly]),
        _ => {
            if plain && let Some(rule) = meaning(item.keyword) {
                return one(bf, Gives::Rule(rule), vec![Context::PermanentsOnly]);
            }
            if plain && ASKED.iter().any(|(name, _)| Keyword::from_name(name) == Some(item.keyword)) {
                let scope = if item.keyword.name() == "Flash" { Scope::Everywhere } else { bf };
                return one(scope, Gives::Asked, vec![Context::PermanentsOnly]);
            }
            if crate::protection::known(item) { return one(bf, Gives::Asked, vec![Context::PermanentsOnly]); }
            if let KeywordParam::Landwalk(filter) = &item.param && is_landwalk(item.keyword) {
                landwalk_ok(filter)?;
                return one(bf, Gives::Asked, vec![Context::PermanentsOnly]);
            }
            refused(item)
        }
    }
}
fn offspring_trigger() -> Ability {
    static OFFSPRING: std::sync::OnceLock<Ability> = std::sync::OnceLock::new();
    crate::token::init(&OFFSPRING, || {
        let def = CardDef::new("Offspring rule", "", "Creature — Rabbit",
            "When this permanent enters, if its offspring cost was paid, create a token that's a copy of it, except it's 1/1.", Some((0, 0)))
            .expect("the grammar reads rule 702.175a");
        def.abilities[0].clone()
    }).clone()
}
/// Casualty N's cost (702.153a): the sacrifice of a creature with power N or greater, as the
/// grammar reads that additional cost.
fn casualty_cost(n: u32) -> ir::Cost {
    let text = format!("As an additional cost to cast this spell, sacrifice a creature with power {n} or greater.");
    let def = CardDef::new("Casualty rule", "{0}", "Instant", &text, None).expect("the grammar reads rule 702.153a's cost");
    def.abilities.iter().find_map(crate::functions::casting_cost).expect("an additional cost").clone()
}
/// Sneak's returning an unblocked creature you control to its owner's hand (702.190a), as the
/// grammar reads it in ninjutsu's words (702.49a): only an attacking creature is unblocked
/// (509.1h).
fn sneak_return() -> &'static ir::Cost {
    static RETURN: std::sync::OnceLock<ir::Cost> = std::sync::OnceLock::new();
    crate::token::init(&RETURN, || {
        let def = CardDef::new("Sneak rule", "{0}", "Instant", "As an additional cost to cast this spell, return an unblocked attacking creature you control to its owner's hand.", None)
            .expect("the grammar reads rule 702.190a's cost");
        def.abilities.iter().find_map(crate::functions::casting_cost).expect("an additional cost").clone()
    })
}
fn web_slinging_return() -> &'static ir::Cost {
    static RETURN: std::sync::OnceLock<ir::Cost> = std::sync::OnceLock::new();
    crate::token::init(&RETURN, || {
        let def = CardDef::new("Web-slinging rule", "{0}", "Instant", "As an additional cost to cast this spell, return a tapped creature you control to its owner's hand.", None)
            .expect("the grammar reads rule 702.188a's cost");
        def.abilities.iter().find_map(crate::functions::casting_cost).expect("an additional cost").clone()
    })
}
/// Teamwork N's cost (702.194a), as the grammar reads it in crew's words (702.122a): only an
/// untapped creature can be tapped for a cost (701.26a).
fn teamwork_cost(n: u32) -> ir::Cost {
    let text = format!("As an additional cost to cast this spell, tap any number of untapped creatures you control with total power {n} or greater.");
    let def = CardDef::new("Teamwork rule", "{0}", "Instant", &text, None).expect("the grammar reads rule 702.194a's cost");
    def.abilities.iter().find_map(crate::functions::casting_cost).expect("an additional cost").clone()
}
/// "When you cast this spell, if a casualty cost was paid for it, copy it. If the spell has
/// any targets, you may choose new targets for the copy." (702.153a), expanded into typed
/// trigger, condition and copy actions as replicate's is ([`super::replicate_trigger`]).
fn casualty_trigger() -> Ability {
    use ir::{Action, Condition, Effect, Lead, Modifier, ObjectSet, Quant, Ref, Selector, Sentence};
    use ir::effect::{Act, Body, Clause};
    use ir::event::{Arg, Deed, EventPattern, TriggerWord};
    let source = Selector::new(Quant::Definite(Ref::This), ObjectSet::of(ir::Domain::Spell));
    let sentence = |action, may: bool| Sentence::Do(Body { first: Clause {
        subject: may.then(|| ir::effect::Subject::Selector(Selector::you())), may, also: false, first: Act { action, mods: Vec::new(), may: false }, rest: Vec::new(),
    }, rest: Vec::new() });
    let copy = Selector::new(Quant::Definite(Ref::Anaphor(ir::Anaphor::Copy { plural: false })), ObjectSet::of(ir::Domain::Object));
    let paid = Condition::CastWas { what: Box::new(source.clone()), fact: ir::condition::CastFact::CostPaid(Keyword::from_name("Casualty").expect("a keyword")), negated: false };
    Ability::Triggered {
        trigger: ir::Trigger::Event { word: TriggerWord::When, pattern: EventPattern {
            subjects: vec![Selector::you()], actions: vec![ir::EventAction::Deed(Deed {
                verb: ir::Verb::Cast, passive: false, args: vec![(ir::Role::What, Arg::Noun(source.clone()))],
            })], nth_each_turn: None, during: None, except_nth_in_step: None, condition: None, cause: None,
        }},
        effect: Effect(vec![
            Sentence::Led { lead: Lead::Modifier(Modifier::If(paid)),
                rest: Box::new(sentence(Action::Copy(ir::Objects { items: vec![source], conj: ir::Conj::And }), false)) },
            sentence(Action::ChooseNewTargets { for_: ir::Objects { items: vec![copy], conj: ir::Conj::And }, one: false }, true),
        ]),
    }
}
/// 702.109a: copied spell announcements retain whether the dash cost was paid.
fn dash_haste() -> Ability {
    static HASTE: std::sync::OnceLock<Ability> = std::sync::OnceLock::new();
    crate::token::init(&HASTE, || CardDef::new("Dash rule", "", "Creature",
        "If this creature's dash cost was paid, this creature has haste.", None)
        .expect("the grammar reads rule 702.109a's conditional haste").abilities[0].clone()).clone()
}
fn evoke_trigger() -> Ability {
    static EVOKE: std::sync::OnceLock<Ability> = std::sync::OnceLock::new();
    crate::token::init(&EVOKE, || {
        let def = CardDef::new("Evoke rule", "", "Creature — Elemental",
            "When this permanent enters, if its evoke cost was paid, its controller sacrifices it.", Some((0, 0)))
            .expect("the grammar reads rule 702.74a");
        def.abilities[0].clone()
    }).clone()
}
/// The abilities of "Impending N" besides its alternative cost (702.176a): a replacement
/// effect, a static ability and a triggered ability, each asking whether the impending cost
/// was paid. The rule's "it" after "and" is said as "this permanent", which the grammar reads.
/// None for a number the rule text is not read with.
fn impending(n: u32) -> Option<Vec<Ability>> {
    const WORDS: [&str; 10] = ["a", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten"];
    if !(1..=10).contains(&n) { return None; }
    let text = format!("If its impending cost was paid, this permanent enters with {} time counter{} on it.\n\
        As long as its impending cost was paid and this permanent has a time counter on it, this permanent isn't a creature.\n\
        At the beginning of your end step, if its impending cost was paid and this permanent has a time counter on it, remove a time counter from this permanent.",
        WORDS[n as usize - 1], if n == 1 { "" } else { "s" });
    let def = CardDef::new("Impending rule", "", "Enchantment Creature", &text, Some((0, 0))).expect("the grammar reads rule 702.176a");
    Some(def.abilities.clone())
}
/// Each alternative cost a keyword of the definition states, by its row (118.9, 702.74a).
pub(crate) fn alternative_costs(def: &CardDef) -> impl Iterator<Item = (usize, &ir::Cost)> {
    def.keyword_declarations.iter().filter_map(|i| Some((i.row, i.declared.as_ref().ok()?)))
        .flat_map(|(row, d)| d.gives.iter().filter_map(move |(_, gives)| match gives {
            Gives::Alternative { cost, .. } => Some((row, cost)),
            _ => None,
        }))
}
/// The rows of the keyword instances whose alternative cost this is: "its evoke cost"
/// (702.74a), each instance its own (113.2c).
pub(crate) fn alternative_rows(def: &CardDef, keyword: Keyword) -> Vec<usize> {
    def.keyword_declarations.iter().filter(|i| i.item.keyword == keyword)
        .filter(|i| i.declared.as_ref().is_ok_and(|d| d.gives.iter().any(|(_, g)| matches!(g, Gives::Alternative { .. }))))
        .map(|i| i.row).collect()
}
/// The alternative cost this keyword instance states (118.9), if it is one an effect may give.
pub(crate) fn alternative_cost(item: &KeywordInstance) -> Option<ir::Cost> {
    let declared = declare(item).ok().filter(|d| !d.needs.contains(&Context::PrintedOnly))?;
    declared.gives.iter().find_map(|(_, gives)| match gives { Gives::Alternative { cost, .. } => Some(cost.clone()), _ => None })
}
/// The one zone the alternative cost at this row is paid for a spell cast from, if it says
/// one: a warp cost, from a hand (702.185a).
pub(crate) fn alternative_from(def: &CardDef, row: usize) -> Option<Zone> {
    def.keyword_declarations.iter().filter(|i| i.row == row).filter_map(|i| i.declared.as_ref().ok())
        .flat_map(|d| d.gives.iter()).find_map(|(_, gives)| match gives { Gives::Alternative { from, .. } => *from, _ => None })
}
/// A cost a keyword states as mana or as a cost.
fn route_cost(item: &KeywordInstance) -> Option<ir::Cost> {
    match &item.param {
        KeywordParam::Mana(mana) => Some(ir::Cost(vec![ir::CostPart::Mana(mana.clone())])),
        KeywordParam::Cost(cost) => Some(cost.clone()),
        _ => None,
    }
}
pub(crate) fn declarations(abilities: &[Ability]) -> Vec<Instance> {
    abilities.iter().enumerate().flat_map(|(row, ability)| match ability {
        Ability::Keywords(items) => items.iter().map(move |item| Instance { row, item: item.clone(), declared: declare(item).map(Arc::new) }).collect(),
        _ => Vec::new(),
    }).collect()
}
pub(crate) fn contributions(def: &CardDef) -> impl Iterator<Item = (Scope, &Gives)> {
    def.keyword_declarations.iter().filter_map(|i| i.declared.as_ref().ok())
        .flat_map(|d| d.gives.iter().map(|(scope, gives)| (*scope, gives)))
}
/// Each morph, megamorph or disguise ability a definition prints: its row, how it makes the
/// card face down, the cost of turning it face up and whether that is a megamorph cost
/// (702.37a–b, 702.168a).
pub(crate) fn face_down_casts(def: &CardDef) -> impl Iterator<Item = (usize, crate::face::Concealment, &ir::mana::ManaCost, bool)> {
    def.keyword_declarations.iter().filter_map(|i| Some((i.row, i.declared.as_ref().ok()?)))
        .flat_map(|(row, d)| d.gives.iter().filter_map(move |(_, gives)| match gives {
            Gives::FaceDown { by, cost, megamorph } => Some((row, *by, cost, *megamorph)),
            _ => None,
        }))
}
/// Each morph, megamorph or disguise ability among the keyword abilities an object has, as
/// the layers leave them (613.1f): how it makes the card face down, the cost of turning it
/// face up and whether that is a megamorph cost (702.37a–b, 702.168a).
pub(crate) fn face_down_costs(held: &[Held]) -> impl Iterator<Item = (crate::face::Concealment, &ir::mana::ManaCost, bool)> {
    held.iter().flat_map(Held::contributions).filter_map(|(_, gives)| match gives {
        Gives::FaceDown { by, cost, megamorph } => Some((*by, cost, *megamorph)),
        _ => None,
    })
}
/// The check of each keyword ability a row lists, each one ability of its own (113.2c).
pub(crate) fn row_checks(def: &CardDef, row: usize) -> impl Iterator<Item = Res<()>> {
    def.keyword_declarations.iter().filter(move |i| i.row == row)
        .map(|i| i.declared.as_ref().map_err(Clone::clone)?.context_ok(def, &i.item))
}
/// The row of a keyword whose meaning is a choice of the modes printed under it, Spree's or
/// Tiered's, and how many of them are chosen (702.172a, 702.183a).
pub(crate) fn modal_row(def: &CardDef) -> Option<(usize, &ir::effect::Modes)> {
    def.keyword_declarations.iter().filter_map(|i| Some((i.row, i.declared.as_ref().ok()?)))
        .find_map(|(row, d)| d.gives.iter().find_map(|(_, gives)| match gives { Gives::Modes(modes) => Some((row, modes)), _ => None }))
}
