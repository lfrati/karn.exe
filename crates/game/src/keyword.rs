//! Keyword abilities (CR 702): each means what its rule says, declared once here.
//!
//! Where the rule is a sentence about the permanent that has the ability ("a creature with
//! defender can't attack", 702.3b), the sentence is the declaration: the grammar reads it and
//! the engine holds it in force like the static ability it is ([`crate::restrict`]). Nothing is
//! written for the keyword beyond its rule's words. Where the rule changes a rule of the turn
//! instead (haste, 702.10b; vigilance, 702.20b), the engine asks at that rule whether the
//! permanent has the ability.

use crate::card::CardDef;
use crate::program::expr::{Filter, linked};
use crate::select::Support;
use crate::state::{Game, ObjId};
use ir::token::KeywordParam;
use ir::{Ability, Keyword};
use std::sync::{Arc, OnceLock};

mod declaration;
pub(crate) use declaration::*;

/// The keyword abilities whose rule is a sentence the engine holds in force: the keyword, the
/// rule, and the rule's sentence said of the permanent.
const SENTENCES: &[(&str, &str, &str)] = &[
    ("Defender", "702.3b", "This creature can't attack."),
    ("Horsemanship", "702.31b", "This creature can't be blocked except by creatures with horsemanship."),
    ("Fear", "702.36b", "This creature can't be blocked except by artifact creatures and/or black creatures."),
    ("Flying", "702.9b", "This creature can't be blocked except by creatures with flying and/or reach."),
    ("Skulk", "702.118b", "This creature can't be blocked by creatures with power greater than this creature's power."),
    ("Menace", "702.111b", "This creature can't be blocked except by two or more creatures."),
    ("Indestructible", "702.12b", "This permanent can't be destroyed."),
    ("Hexproof", "702.11b", "This permanent can't be the target of spells or abilities your opponents control."),
    ("Shroud", "702.18a", "This permanent can't be the target of spells or abilities."),
];

/// The keyword abilities a rule of the game asks about, where that rule is: reach is named by
/// flying's rule (702.17b); a creature with haste can attack though it has not been its
/// controller's since their turn began (702.10b, 302.6); attacking doesn't cause a creature
/// with vigilance to tap (702.20b, 508.1f); creatures with first strike or double strike assign
/// combat damage in a step of their own, those with double strike again in the second (702.7b,
/// 702.4b, 510.4); an attacking creature with trample may assign what is more than lethal to
/// the creatures blocking it to what it attacks (702.19b); a creature dealt damage by a source
/// with deathtouch is destroyed (702.2b, 704.5h), and any of it is lethal (702.2c); damage
/// dealt by a source with lifelink also causes its controller to gain that much life (702.15b,
/// 120.3f); wither or infect damage to a creature causes its source controller to put -1/-1
/// counters instead of marking damage (702.80a, 702.90c, 120.3d); infect damage to a player
/// gives poison instead of life loss (702.90b, 120.3b); a card with flash is cast any time an instant could be (702.8a, 117.1a).
const ASKED: &[(&str, &str)] = &[
    ("Reach", "702.17b"),
    ("Retrace", "702.81a"),
    ("Delve", "702.66a"),
    ("Convoke", "702.51a"),
    ("Improvise", "702.126a"),
    ("Rebound", "702.88a"),
    ("Haste", "702.10b"),
    ("Vigilance", "702.20b"),
    ("First strike", "702.7b"),
    ("Double strike", "702.4b"),
    ("Trample", "702.19b"),
    ("Deathtouch", "702.2b"),
    ("Lifelink", "702.15b"),
    ("Wither", "702.80a"),
    ("Infect", "702.90b"),
    ("Flash", "702.8a"),
];

/// Each sentence read by the grammar, once.
fn read() -> &'static Vec<(Keyword, Ability)> {
    static READ: OnceLock<Vec<(Keyword, Ability)>> = OnceLock::new();
    crate::token::init(&READ, || {
        SENTENCES
            .iter()
            .map(|(name, rule, sentence)| {
                let keyword = Keyword::from_name(name).unwrap_or_else(|| panic!("{name} is a keyword ability"));
                let def = CardDef::new(name, "", "Creature", sentence, None)
                    .unwrap_or_else(|e| panic!("the grammar reads rule {rule}, \"{sentence}\": {e}"));
                let [ability] = def.abilities.as_slice() else { panic!("rule {rule} is one sentence") };
                (keyword, ability.clone())
            })
            .collect()
    })
}

/// The rule that states a keyword's sentence: "702.9b" for flying.
pub(crate) fn sentence_rule(k: Keyword) -> Option<&'static str> {
    if k.name() == "Shadow" { return Some("702.28b"); }
    if k.name() == "Intimidate" { return Some("702.13b"); }
    SENTENCES.iter().find(|(name, _, _)| Keyword::from_name(name) == Some(k)).map(|(_, rule, _)| *rule)
}

/// The static ability a keyword is, when its rule is a sentence.
pub fn meaning(k: Keyword) -> Option<&'static Ability> {
    read().iter().find(|(keyword, _)| *keyword == k).map(|(_, a)| a)
}

/// Split second states two independent rules with different subjects (702.61a).
/// Keep the parsed abilities once; each becomes its own stack-scoped contribution.
fn split_second_rules() -> &'static [Ability] {
    static RULES: OnceLock<Vec<Ability>> = OnceLock::new();
    let rules: &'static Vec<Ability> = crate::token::init(&RULES, || {
        let def = CardDef::new("Split second", "", "Creature",
            "Players can't cast spells.\nActivated abilities of sources can't be activated unless they're mana abilities.", None)
            .expect("the grammar reads both split-second rules (702.61a)");
        assert_eq!(def.abilities.len(), 2, "split second has two rule subjects");
        def.abilities.clone()
    });
    rules
}

/// 702.13b: either an artifact creature, or one sharing a color with the attacker.
/// Keep the two alternatives and the explicit source referent separate in typed syntax.
fn intimidate_rule() -> &'static Ability {
    static RULE: OnceLock<Ability> = OnceLock::new();
    crate::token::init(&RULE, || {
        use ir::{Atom, Filter, ObjectSet, Quant, Ref, Selector};
        let source = Selector::new(Quant::Definite(Ref::This), ObjectSet::of(ir::Domain::Permanent));
        let blocker = |quality| Selector::new(Quant::Indefinite(ir::Count::Exactly(ir::Amount::Const(1))), ObjectSet {
            filter: Filter::All(vec![Filter::Atom(Atom::Type(ir::CardType::Creature)), Filter::Atom(quality)]),
            ..ObjectSet::of(ir::Domain::Permanent)
        });
        Ability::Text(ir::Effect(vec![ir::Sentence::Do(ir::effect::Body {
            first: ir::effect::Clause { subject: Some(ir::Subject::Selector(source.clone())), may: false, also: false,
                first: ir::effect::Act { action: ir::Action::Cant(vec![ir::statics::Forbidden {
                    event: ir::EventAction::BecomesBlocked { by: vec![blocker(Atom::Type(ir::CardType::Artifact)),
                        blocker(Atom::Shares { what: ir::amount::Characteristic::Color, with: Box::new(source) })], except: true }, within: None,
                }]), mods: vec![], may: false }, rest: vec![] }, rest: vec![],
        })]))
    })
}

/// Shadow constrains both sides of a block (702.28b); both are ordinary restrictions.
fn shadow_rules() -> &'static [Ability] {
    static RULES: OnceLock<Vec<Ability>> = OnceLock::new();
    crate::token::init(&RULES, || CardDef::new("Shadow rule", "", "Creature",
        "This creature can't be blocked except by creatures with shadow.\nThis creature can block only creatures with shadow.", None)
        .expect("the grammar reads both restrictions of 702.28b").abilities.clone()).as_slice()
}

/// Does the engine give this keyword ability its meaning?
pub fn known(k: Keyword) -> bool {
    passive(&ir::token::KeywordInstance { keyword: k, param: KeywordParam::None })
}

/// A supported passive keyword instance, preserving its parameter (702).
/// 702.114a: a color characteristic definition, functioning in every zone.
pub(crate) fn devoid(item: &ir::token::KeywordInstance) -> bool {
    item.keyword.name() == "Devoid" && item.param == KeywordParam::None
}
/// 702.73a: an unconditional subtype CDA, including a token's defining ability.
pub(crate) fn changeling(item: &ir::token::KeywordInstance) -> bool {
    item.keyword.name() == "Changeling" && item.param == KeywordParam::None
}
pub(crate) fn has_changeling(a: &Ability) -> bool {
    matches!(a, Ability::Keywords(items) if items.iter().any(changeling))
}
/// Sunburst is registered as an entry replacement, not a grantable passive ability.
pub(crate) fn sunburst(item: &ir::token::KeywordInstance) -> bool {
    item.keyword.name() == "Sunburst" && item.param == KeywordParam::None
}
pub(crate) fn has_devoid(a: &Ability) -> bool {
    matches!(a, Ability::Keywords(items) if items.iter().any(devoid))
}

pub(crate) fn passive(item: &ir::token::KeywordInstance) -> bool {
    passive_ok(item).is_ok()
}

/// Does the engine give this passive keyword instance its meaning, parameter and all? If not,
/// what it stops at. Admitting a card that prints it, an effect that grants it, and holding it
/// in force all ask this one question.
pub(crate) fn passive_ok(item: &ir::token::KeywordInstance) -> crate::Res<()> {
    let declared = declare(item)?;
    if declared.grantable() { Ok(()) } else { crate::unsupported(format!("keyword: {}", item.keyword.name())) }
}

fn is_landwalk(k: Keyword) -> bool {
    Keyword::from_name("Landwalk") == Some(k)
}

/// A passive keyword ability something has (702), lowered once: as its definition is loaded
/// for one it is printed with, as the granting text is lowered for one it is given. Its
/// parameter is kept as said; what the engine evaluates of it is lowered beside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    pub keyword: Keyword,
    pub param: KeywordParam,
    /// "[type]walk" (702.14a): the type a land the defending player controls must have for
    /// the creature with it to be unblockable (702.14c).
    landwalk: Option<Filter>,
    /// Protection's qualities (702.16a), as the source of each is asked of them.
    protection: Option<Arc<crate::protection::Protection>>,
    declared: Arc<Declared>,
}

impl Held {
    /// A paired cost definition gives this instance the recipient spell's own mana cost.
    /// Keep the definition, rather than freezing mana value or one face's cost at grant time.
    pub(crate) fn flashback_at_mana_cost() -> Self {
        use crate::functions::Scope;
        static DECLARED: OnceLock<Arc<Declared>> = OnceLock::new();
        let declared = crate::token::init(&DECLARED, || Arc::new(Declared {
            gives: vec![(Scope::In(crate::Zone::Graveyard), Gives::Route {
                route: crate::announcement::CastRoute::Flashback, cost: RouteCost::ManaCost,
            }), (Scope::In(crate::Zone::Stack), Gives::Destination { when: Departure::Any, to: crate::Zone::Exile, then: None })],
            needs: vec![],
        })).clone();
        Self { keyword: Keyword::from_name("Flashback").expect("a keyword"), param: KeywordParam::None,
            landwalk: None, protection: None, declared }
    }

    /// A keyword ability with no parameter. Counters and definition-less objects can hold
    /// unsupported keywords; retain their identity without inventing executable meaning.
    pub(crate) fn plain(keyword: Keyword) -> Held {
        let item = ir::token::KeywordInstance { keyword, param: KeywordParam::None };
        let declared = declare(&item).unwrap_or_else(|_| Declared { gives: vec![], needs: vec![Context::Unavailable] });
        Held { keyword, param: KeywordParam::None, landwalk: None, protection: None, declared: Arc::new(declared) }
    }

    /// The instance lowered, or what the engine stops at ([`passive_ok`]). Admitting a card
    /// that prints it, an effect that grants it, and holding it in force share this.
    pub(crate) fn compile(item: &ir::token::KeywordInstance, support: &Support<'_>) -> crate::Res<Held> {
        Self::compile_bound(item, support, &mut linked(support))
    }

    /// [`Held::compile`], with `bind` binding what the granting text chose before it: "choose a
    /// color. … gain protection from the chosen color".
    pub(crate) fn compile_bound(item: &ir::token::KeywordInstance, support: &Support<'_>, bind: crate::program::expr::Bind<'_>) -> crate::Res<Held> {
        let declared = Arc::new(declare(item)?);
        if !declared.grantable() { return crate::unsupported(format!("keyword: {}", item.keyword.name())); }
        Self::from_declared(item, declared, support, bind)
    }

    fn from_declared(item: &ir::token::KeywordInstance, declared: Arc<Declared>, support: &Support<'_>, bind: crate::program::expr::Bind<'_>) -> crate::Res<Held> {
        let landwalk = match &item.param {
            KeywordParam::Landwalk(filter) if is_landwalk(item.keyword) => Some(Filter::compile(filter, support, &mut *bind)?),
            _ => None,
        };
        let protection = match &item.param {
            KeywordParam::Protection(_) if crate::protection::known(item) => Some(Arc::new(crate::protection::Protection::compile(item, support, bind)?)),
            _ => None,
        };
        Ok(Held { keyword: item.keyword, param: item.param.clone(), landwalk, protection, declared })
    }

    /// Protection's qualities, if it is protection (702.16a).
    pub(crate) fn protection(&self) -> Option<&crate::protection::Protection> {
        self.protection.as_deref()
    }

    /// The chosen values it reads, for checking that what produces them comes first.
    pub(crate) fn scalars(&self) -> Vec<crate::binding::Scalar> {
        self.protection.as_ref().map(|p| p.scalars()).unwrap_or_default()
    }

    /// Does an effect granting it say something of it only as the effect applies?
    pub(crate) fn captures(&self) -> bool {
        self.protection.is_some()
    }

    /// As the effect granting it says it now (608.2h, 611.3a): its protection's chosen values
    /// and players.
    pub(crate) fn capture(&self, game: &Game, ctx: &crate::select::Ctx, locals: &dyn crate::program::expr::Locals) -> Held {
        let mut held = self.clone();
        if let Some(p) = &self.protection { held.protection = Some(Arc::new(p.capture(game, ctx, locals))); }
        held
    }

    pub(crate) fn contributions(&self) -> impl Iterator<Item = (crate::functions::Scope, &Gives)> {
        self.declared.gives.iter().map(|(scope, gives)| (*scope, gives))
    }

    /// The lands a landwalk ability walks (702.14c); None for any other keyword.
    pub(crate) fn landwalk(&self) -> Option<&Filter> {
        self.landwalk.as_ref()
    }
}

/// Is this the type of a landwalk ability as 702.14a–c say it is? "[type] is usually a land
/// type, but it can also be the card type land plus any combination of land types, card types,
/// and/or supertypes" (702.14a); a land has it "with the specified land type", "with the
/// specified type or supertype", "without the specified type or supertype", or "with both the
/// specified type or supertype and the specified subtype" (702.14c). Each part is then a
/// quality of the land's characteristics the selector evaluation reads as they are now.
pub(crate) fn landwalk_ok(filter: &ir::Filter) -> crate::Res<()> {
    use ir::{Atom, Filter};
    let part = |f: &Filter| match f {
        Filter::Atom(Atom::Type(_) | Atom::Supertype(_)) => Ok(()),
        Filter::Atom(Atom::Subtype(s)) if s.kind() == ir::vocab::SubtypeKind::Land => Ok(()),
        Filter::Atom(Atom::Subtype(s)) => crate::unsupported(format!("landwalk: {}, not a land type (702.14a)", s.name())),
        Filter::Not(x) if matches!(**x, Filter::Atom(Atom::Type(_) | Atom::Supertype(_))) => Ok(()),
        other => crate::unsupported(format!("landwalk: a type described by {} (702.14a)", crate::select::name(other))),
    };
    match filter {
        Filter::All(parts) if !parts.is_empty() => parts.iter().try_for_each(|f| part(f)),
        Filter::All(_) => crate::unsupported("landwalk: no type (702.14a)"),
        other => part(other),
    }
}

/// The passive keyword abilities a definition prints, lowered once as it is loaded. One the
/// engine does not give its meaning is left out: its card is refused ([`passive_ok`]).
pub(crate) fn held(def: &CardDef) -> Vec<Held> {
    let support = Support::definition(def, None);
    def.keyword_declarations.iter().filter_map(|instance| {
        let declared = instance.declared.as_ref().ok()?;
        declared.held().then(|| Held::from_declared(&instance.item, declared.clone(), &support, &mut linked(&support)).ok()).flatten()
    }).collect()
}

/// Is this keyword one an object is asked to have ([`Held`]), rather than only the
/// activated, triggered or spell ability its row is ("equip {3}")?
/// The same question [`held`] asks of a definition's keywords.
pub(crate) fn is_held(item: &ir::KeywordInstance) -> bool {
    declare(item).is_ok_and(|declared| declared.held())
}

/// The keyword abilities a card has, as it is printed.
pub fn of(def: &CardDef) -> impl Iterator<Item = Keyword> + '_ {
    def.abilities.iter().flat_map(|a| match a {
        Ability::Keywords(v) => v.iter().filter(|i| i.param == KeywordParam::None).map(|i| i.keyword).collect(),
        _ => Vec::new(),
    })
}

/// Delve substitutes exiling a graveyard card for one generic mana after the total
/// cost is fixed (702.66a-b). The selected cards use ordinary object-cost validation.
pub(crate) fn convoke_cost(color: Option<ir::Color>) -> &'static crate::cost::Plan {
    static COSTS: OnceLock<Vec<crate::cost::Plan>>=OnceLock::new();
    &crate::token::init(&COSTS, || ["white ","blue ","black ","red ","green ",""].iter().map(|c|
        CardDef::new("Convoke rule","{0}","Artifact",&format!("As an additional cost to cast this spell, tap an untapped {c}creature you control."),None)
            .expect("convoke creature selection reads").costs.additional.expect("convoke tap selection compiles")
    ).collect())[color.map_or(5,|c|c as usize)]
}
/// 702.126a: artifacts pay generic mana by the ordinary object-tapping cost owner.
pub(crate) fn improvise_cost() -> &'static crate::cost::Plan {
    static COST: OnceLock<crate::cost::Plan> = OnceLock::new();
    crate::token::init(&COST, || CardDef::new("Improvise rule", "{0}", "Artifact",
        "As an additional cost to cast this spell, tap an untapped artifact you control.", None)
        .expect("the grammar reads rule 702.126a's artifact selection").costs.additional.expect("artifact tap cost compiles"))
}
/// What a waterbend cost taps instead of paying a generic mana (701.67a).
pub(crate) fn waterbend_cost() -> &'static crate::cost::Plan {
    static COST: OnceLock<crate::cost::Plan> = OnceLock::new();
    crate::token::init(&COST, || CardDef::new("Waterbend rule", "{0}", "Artifact",
        "As an additional cost to cast this spell, tap an untapped artifact or creature you control.", None)
        .expect("waterbend's tapping reads").costs.additional.expect("waterbend's tapping compiles"))
}
pub(crate) fn delve_cost() -> &'static crate::cost::Plan {
    static COST: OnceLock<crate::cost::Plan> = OnceLock::new();
    crate::token::init(&COST, || CardDef::new("Delve rule", "{0}", "Artifact",
        "As an additional cost to cast this spell, exile a card from your graveyard.", None)
        .expect("the grammar reads 702.66a's exile payment").costs.additional.expect("exile graveyard cost compiles"))
}

/// Retrace functions in the graveyard (702.81a), and its additional discard uses
/// the same compiled cost as a printed additional cost (601.2f-h).
pub(crate) fn retrace_cost() -> &'static crate::cost::Plan {
    static COST: OnceLock<crate::cost::Plan> = OnceLock::new();
    crate::token::init(&COST, || CardDef::new("Retrace rule", "{0}", "Artifact",
        "As an additional cost to cast this spell, discard a land card.", None)
        .expect("the grammar reads 702.81a's additional cost").costs.additional.expect("discard land cost compiles"))
}

/// Does this spell have bargain (702.166a)? Only one is ever printed; more would say nothing more.
pub(crate) fn bargain(def: &CardDef) -> bool {
    of_items(def).any(|k| k.keyword.name() == "Bargain" && k.param == KeywordParam::None)
}

/// What a spell's optional additional cost is (118.8b): its controller announces as it is
/// cast whether it is paid (601.2b), and what it does is linked to that (607.2): bargain
/// (702.166a), a printed "As an additional cost to cast this spell, you may …", offspring
/// (702.175a), casualty (702.153a) or teamwork (702.194a).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Optional { Bargain, Printed, Offspring, Casualty, Teamwork }
impl Optional {
    /// How a listing and a move name its announcement.
    pub fn name(self) -> &'static str {
        match self { Self::Bargain => "bargain", Self::Printed => "additional", Self::Offspring => "offspring", Self::Casualty => "casualty", Self::Teamwork => "teamwork" }
    }
}

/// Each optional additional cost of the definition, with the row stating it, as written: a
/// printed "you may" cost, or a keyword's.
pub(crate) fn optional_costs(def: &CardDef) -> impl Iterator<Item = (usize, Optional, &ir::Cost)> {
    def.abilities.iter().enumerate().filter_map(|(row, a)| Some((row, Optional::Printed, crate::functions::optional_cost(a)?)))
        .chain(def.keyword_declarations.iter().filter_map(|i| Some((i.row, i.declared.as_ref().ok()?)))
            .flat_map(|(row, d)| d.gives.iter().filter_map(move |(_, gives)| match gives {
                Gives::Optional { kind, cost } => Some((row, *kind, cost)),
                _ => None,
            })))
}

/// "As an additional cost to cast this spell, you may sacrifice an artifact, enchantment, or
/// token." (702.166a): the sacrifice as a printed additional cost says it.
fn bargain_cost_text() -> &'static ir::Cost {
    static COST: OnceLock<ir::Cost> = OnceLock::new();
    crate::token::init(&COST, || {
        // the grammar reads "an artifact or enchantment"; the rule's third kind is a token
        let def = CardDef::new("Bargain rule", "{0}", "Instant", "As an additional cost to cast this spell, sacrifice an artifact or enchantment.", None)
            .expect("the grammar reads an additional sacrifice of an artifact or enchantment");
        let mut cost = def.abilities.iter().find_map(crate::functions::casting_cost).expect("an additional cost").clone();
        let [ir::CostPart::Action(ir::Action::Sacrifice(objects))] = cost.0.as_mut_slice() else { unreachable!("one sacrifice") };
        let ir::Filter::All(parts) = &mut objects.items[0].set.filter else { unreachable!("artifact or enchantment") };
        let [ir::Filter::Any(kinds)] = parts.as_mut_slice() else { unreachable!("artifact or enchantment") };
        kinds.push(ir::Filter::Atom(ir::Atom::Status(ir::selector::Status::Token)));
        cost
    })
}

/// 702.85a: the resulting-spell restriction is carried by the compiled cast offer.
/// The final move uses the actual remaining exile incarnations, so a cast card is
/// excluded and a declined hit is returned along with the other exiled cards.
fn cascade() -> &'static Ability {
    static RULE: OnceLock<Ability> = OnceLock::new();
    crate::token::init(&RULE, || CardDef::new("Cascade rule", "", "Creature",
        "When you cast this spell, exile cards from the top of your library until you exile a nonland card with mana value less than this spell's mana value. You may cast that card without paying its mana cost. Put all cards exiled this way on the bottom of your library in a random order.", None)
        .expect("the grammar reads Cascade's scan, cast and return (702.85a)").abilities[0].clone())
}

/// A keyword's executable meaning, read from its rule sentence. For equip (702.6a),
/// the parsed cost replaces the template cost; triggered meanings use the same definitions.
pub(crate) fn executable(item: &ir::token::KeywordInstance) -> Option<Ability> {
    if item.keyword.name() == "Aftermath" && item.param == KeywordParam::None { return Some(aftermath_permission().clone()); }
    if item.keyword.name() == "Partner" && let KeywordParam::PartnerWith(name) = &item.param {
        return Some(partner_with(name));
    }
    if replicate_cost(item).is_some() { return Some(replicate_trigger()); }
    if let Some(ability) = triggered(item) { return Some(ability.clone()); }
    if let Some(cost) = madness_cost(item) { return Some(madness_trigger(&cost)); }
    if let Some((cost, filter)) = cycling_cost(item) { return Some(cycling(cost, filter)); }
    if let Some(of) = gift_of(item) { return gift_effect(of); }
    if let Some(cost) = ward_cost(item) { return Some(ward_trigger(&cost)); }
    if let Some(ability) = crew(item) { return Some(ability); }
    if let Some(ability) = saddle(item) { return Some(ability); }
    if let Some(ability) = affinity(item) { return Some(ability); }
    if let Some(ability) = cumulative_upkeep(item) { return Some(ability); }
    if let Some(ability) = echo(item) { return Some(ability); }
    if let Some(ability) = bloodthirst(item) { return Some(ability); }
    if let Some(ability) = fabricate(item) { return Some(ability); }
    if let Some(ability) = bushido(item) { return Some(ability); }
    if let Some(ability) = afflict(item) { return Some(ability); }
    if let Some(ability) = rampage(item) { return Some(ability); }
    if let Some(ability) = soulshift(item) { return Some(ability); }
    if let Some(ability) = renown(item) { return Some(ability); }
    if let Some(ability) = annihilator(item) { return Some(ability); }
    if let Some(ability) = station(item) { return Some(ability.clone()); }
    if let Some(ability) = firebending(item) { return Some(ability); }
    if let Some(ability) = mobilize(item) { return Some(ability); }
    static EQUIP: OnceLock<Ability> = OnceLock::new();
    static LEVEL_UP: OnceLock<Ability> = OnceLock::new();
    static NINJUTSU: OnceLock<Ability> = OnceLock::new();
    static TRANSMUTE: OnceLock<Ability> = OnceLock::new();
    let (kept, type_line, rule, number) = match item.keyword.name() {
        "Ninjutsu" => (&NINJUTSU, "Creature", "{0}, Reveal this card from your hand, Return an unblocked attacking creature you control to its owner's hand: Put this card from your hand onto the battlefield tapped attacking the same player, planeswalker, or battle as that creature.", "702.49a-c"),
        "Transmute" => (&TRANSMUTE, "Artifact", "{0}, Discard this card: Search your library for a card with mana value equal to this card's mana value, reveal it, put it into your hand, then shuffle. Activate only as a sorcery.", "702.53a"),
        "Equip" => (&EQUIP, "Artifact — Equipment", "{0}: Attach this permanent to target creature you control. Activate only as a sorcery.", "702.6a"),
        // "Level up [cost]" means "[Cost]: Put a level counter on this permanent. Activate
        // only as a sorcery." (702.87a)
        "Level Up" => (&LEVEL_UP, "Creature", "{0}: Put a level counter on this permanent. Activate only as a sorcery.", "702.87a"),
        _ => return None,
    };
    let cost = match &item.param {
        KeywordParam::Mana(mana) => ir::Cost(vec![ir::CostPart::Mana(mana.clone())]),
        KeywordParam::Cost(cost) => cost.clone(),
        _ => return None,
    };
    let template = crate::token::init(kept, || {
        let def = CardDef::new("Keyword rule", "", type_line, rule, None)
            .unwrap_or_else(|e| panic!("the grammar reads rule {number}: {e}"));
        def.abilities[0].clone()
    });
    let Ability::Activated { cost: extra, effect } = template else { unreachable!("the keyword is an activated ability") };
    let mut cost = cost;
    // The first part is the keyword parameter. Ninjutsu also retains its reveal
    // beyond payment: this annotation belongs to its rule, not the printed reminder.
    cost.0.extend(extra.0.iter().skip(1).cloned().map(|part| match part {
        ir::CostPart::Action(ir::Action::Reveal(objects)) => ir::CostPart::RevealUntilLeavesStack(objects),
        other => other,
    }));
    Some(Ability::Activated { cost, effect: effect.clone() })
}

/// 702.127a: the selected half grants its ordinary graveyard self-permission.
/// Its other two rules are source-zone and departure contributions in the declaration.
fn aftermath_permission() -> &'static Ability {
    static RULE: OnceLock<Ability> = OnceLock::new();
    crate::token::init(&RULE, || CardDef::new("Aftermath rule", "", "Sorcery",
        "You may cast this card from your graveyard.", None)
        .expect("the grammar reads rule 702.127a's self-permission").abilities[0].clone())
}

/// Cycling and typecycling use the same activation/discard procedure (702.29a, e).
fn cycling_cost(item: &ir::token::KeywordInstance) -> Option<(ir::Cost, Option<ir::Filter>)> {
    if item.keyword.name() != "Cycling" { return None; }
    match &item.param {
        KeywordParam::Mana(mana) => Some((ir::Cost(vec![ir::CostPart::Mana(mana.clone())]), None)),
        KeywordParam::Cost(cost) => Some((cost.clone(), None)),
        KeywordParam::Typecycling(filter, mana) => Some((ir::Cost(vec![ir::CostPart::Mana(mana.clone())]), Some(filter.clone()))),
        _ => None,
    }
}
fn cycling(mut cost: ir::Cost, filter: Option<ir::Filter>) -> Ability {
    static CYCLING: OnceLock<Ability> = OnceLock::new();
    static TYPECYCLING: OnceLock<Ability> = OnceLock::new();
    let (kept, text) = if filter.is_some() {
        (&TYPECYCLING, "{0}, Discard this card: Search your library for a card, reveal it, put it into your hand, then shuffle.")
    } else { (&CYCLING, "{0}, Discard this card: Draw a card.") };
    let template = crate::token::init(kept, || CardDef::new("Cycling rule", "", "Artifact", text, None)
        .expect("the grammar reads CR 702.29a/e").abilities[0].clone());
    let Ability::Activated { cost: template_cost, effect } = template else { unreachable!("cycling is activated") };
    cost.0.extend(template_cost.0.iter().skip(1).cloned());
    let mut effect = effect.clone();
    if let Some(filter) = filter {
        let ir::Sentence::Do(body) = &mut effect.0[0] else { unreachable!("typecycling begins with search") };
        let ir::Action::Search { what, .. } = &mut body.first.first.action else { unreachable!("typecycling begins with search") };
        what.items[0].set.filter = filter;
    }
    Ability::Activated { cost, effect }
}

/// Keyword-triggered abilities are cached as ordinary abilities. Each declaration
/// determines whether its executable instances may also be granted.
fn triggered(item: &ir::token::KeywordInstance) -> Option<&'static Ability> {
    if item.param != KeywordParam::None { return None; }
    match item.keyword.name() {
        "Undying" => Some(death_return(true)),
        "Persist" => Some(death_return(false)),
        "Extort" => Some(extort()),
        "Living Weapon" => Some(living_weapon()),
        "Job Select" => Some(job_select()),
        "Prowess" => Some(prowess()),
        "Battle Cry" => Some(battle_cry()),
        "Exalted" => Some(exalted()),
        "Flanking" => Some(flanking()),
        "Evolve" => Some(evolve()),
        "Mentor" => Some(mentor()),
        "Dethrone" => Some(dethrone()),
        "Training" => Some(training()),
        "Ingest" => Some(ingest()),
        _ => None,
    }
}

/// 702.115a: the damaged player exiles one library card, regardless of damage amount.
fn ingest() -> &'static Ability {
    static RULE: OnceLock<Ability> = OnceLock::new();
    crate::token::init(&RULE, || CardDef::new("Ingest rule", "", "Creature",
        "Whenever this creature deals combat damage to a player, that player exiles the top card of their library.", None)
        .expect("the grammar reads rule 702.115a").abilities[0].clone())
}

/// 702.149a: another greater-power attacker is required at the attack event.
/// Consumers of a successful "trains" event remain explicitly unsupported.
fn training() -> &'static Ability {
    static RULE: OnceLock<Ability> = OnceLock::new();
    crate::token::init(&RULE, || CardDef::new("Training rule", "", "Creature",
        "Whenever this creature attacks while you control another attacking creature with power greater than this creature's power, put a +1/+1 counter on this creature.", None)
        .expect("the grammar reads rule 702.149a").abilities[0].clone())
}

/// 702.105a: the defender's life rank is checked by the attack event, not
/// again when this ordinary counter-placement ability resolves.
fn dethrone() -> &'static Ability {
    static RULE: OnceLock<Ability> = OnceLock::new();
    crate::token::init(&RULE, || CardDef::new("Dethrone rule", "", "Creature",
        "Whenever this creature attacks the player with the most life or tied for most life, put a +1/+1 counter on this creature.", None)
        .expect("the grammar reads rule 702.105a").abilities[0].clone())
}

/// 702.46a-b: an ordinary dies trigger, with its target chosen before the optional return.
fn soulshift(item: &ir::token::KeywordInstance) -> Option<Ability> {
    if item.keyword.name() != "Soulshift" { return None; }
    let KeywordParam::Number(ir::Amount::Const(n)) = item.param else { return None; };
    let text = format!("When this permanent is put into a graveyard from the battlefield, you may return target Spirit card with mana value {n} or less from your graveyard to your hand.");
    Some(CardDef::new("Soulshift rule", "", "Creature", &text, None)
        .expect("the grammar reads rule 702.46a").abilities[0].clone())
}

/// 702.100a/c: both participants must have creature characteristics to compare them.
/// The ordinary intervening-if is checked when triggering and again on resolution (603.4).
fn evolve() -> &'static Ability {
    static EVOLVE: OnceLock<Ability> = OnceLock::new();
    crate::token::init(&EVOLVE, || {
        let mut ability = CardDef::new("Evolve rule", "", "Creature",
            "Whenever a creature you control enters, if that creature's power is greater than this creature's power or that creature's toughness is greater than this creature's toughness, put a +1/+1 counter on this creature.", None)
            .expect("the grammar reads explicit comparisons of rule 702.100a").abilities[0].clone();
        let Ability::Triggered { effect, .. } = &mut ability else { unreachable!("evolve triggers") };
        let ir::Sentence::Led { lead: ir::Lead::Modifier(ir::Modifier::If(condition)), .. } = &mut effect.0[0] else { unreachable!("evolve has an intervening if") };
        let creature = |reference| ir::Condition::Is {
            what: Box::new(ir::Selector::new(ir::Quant::Definite(reference), ir::ObjectSet::of(ir::Domain::Permanent))),
            pred: ir::Predicate::Adjective(ir::Filter::Atom(ir::Atom::Type(ir::CardType::Creature))), negated: false,
        };
        *condition = ir::Condition::And(vec![creature(ir::Ref::This), creature(ir::Ref::Anaphor(ir::Anaphor::That)), condition.clone()]);
        ability
    })
}

/// 702.134a: the dynamic target comparison uses ordinary announcement and resolution checks.
fn mentor() -> &'static Ability {
    static MENTOR: OnceLock<Ability> = OnceLock::new();
    crate::token::init(&MENTOR, || CardDef::new("Mentor rule", "", "Creature",
        "Whenever this creature attacks, put a +1/+1 counter on target attacking creature with power less than this creature's power.", None)
        .expect("the grammar reads rule 702.134a").abilities[0].clone())
}

/// 702.108a-b: every printed instance is an ordinary independent cast trigger.
fn prowess() -> &'static Ability {
    static PROWESS: OnceLock<Ability> = OnceLock::new();
    crate::token::init(&PROWESS, || CardDef::new("Prowess rule", "", "Creature",
        "Whenever you cast a noncreature spell, this creature gets +1/+1 until end of turn.", None)
        .expect("the grammar reads rule 702.108a").abilities[0].clone())
}

/// 702.83a: the creature declared alone is retained by the ordinary event binding.
fn exalted() -> &'static Ability {
    static EXALTED: OnceLock<Ability> = OnceLock::new();
    crate::token::init(&EXALTED, || CardDef::new("Exalted rule", "", "Creature",
        "Whenever a creature you control attacks alone, that creature gets +1/+1 until end of turn.", None)
        .expect("the grammar reads rule 702.83a").abilities[0].clone())
}

/// 702.25a: each blocker lacking flanking receives its own independent effect.
fn flanking() -> &'static Ability {
    static FLANKING: OnceLock<Ability> = OnceLock::new();
    crate::token::init(&FLANKING, || CardDef::new("Flanking rule", "", "Creature",
        "Whenever a creature without flanking blocks this creature, the blocking creature gets -1/-1 until end of turn.", None)
        .expect("the grammar reads rule 702.25a").abilities[0].clone())
}

/// 702.91a-b: each instance triggers separately as this creature attacks.
fn battle_cry() -> &'static Ability {
    static BATTLE_CRY: OnceLock<Ability> = OnceLock::new();
    crate::token::init(&BATTLE_CRY, || CardDef::new("Battle cry rule", "", "Creature",
        "Whenever this creature attacks, each other attacking creature gets +1/+0 until end of turn.", None)
        .expect("the grammar reads rule 702.91a").abilities[0].clone())
}

/// "Hideaway N" (702.75a): "When this permanent enters, look at the top N cards of your
/// library. Exile one of them face down and put the rest on the bottom of your library in a
/// random order." The card's granted ability, that the controller of the permanent that exiled
/// it may look at it, is the exile's own ([`crate::face::Hidden::watcher`]). None for a number
/// the rule text is not read with.
pub(crate) fn hideaway(item: &ir::token::KeywordInstance) -> Option<Ability> {
    if item.keyword.name() != "Hideaway" { return None; }
    let KeywordParam::Number(ir::Amount::Const(n)) = item.param else { return None };
    const WORDS: [&str; 10] = ["one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten"];
    // every printed hideaway number is 4 or 5 (702.75b)
    if !(2..=10).contains(&n) { return None; }
    let text = format!("When this permanent enters, look at the top {} cards of your library. Exile one of them face down and put the rest on the bottom of your library in a random order.", WORDS[n as usize - 1]);
    let def = CardDef::new("Hideaway rule", "", "Artifact", &text, None).expect("the grammar reads rule 702.75a");
    Some(def.abilities[0].clone())
}

/// "Crew N" means "Tap any number of other untapped creatures you control with total power N
/// or greater: This permanent becomes an artifact creature until end of turn." (702.122a). Its
/// own power and toughness are the printed ones once it is a creature (301.7b, 208.3).
fn crew(item: &ir::token::KeywordInstance) -> Option<Ability> {
    if item.keyword.name() != "Crew" { return None; }
    let KeywordParam::Number(ir::Amount::Const(n)) = item.param else { return None };
    let text = format!("Tap any number of other untapped creatures you control with total power {n} or greater: This permanent becomes an artifact creature until end of turn.");
    let def = CardDef::new("Crew rule", "", "Artifact — Vehicle", &text, None).expect("the grammar reads rule 702.122a");
    Some(def.abilities[0].clone())
}

/// "Saddle N" means "Tap any number of other untapped creatures you control with total power N
/// or greater: This permanent becomes saddled until end of turn. Activate only as a sorcery."
/// (702.171a)
fn saddle(item: &ir::token::KeywordInstance) -> Option<Ability> {
    if item.keyword.name() != "Saddle" { return None; }
    let KeywordParam::Number(ir::Amount::Const(n)) = item.param else { return None };
    let text = format!("Tap any number of other untapped creatures you control with total power {n} or greater: This permanent becomes saddled until end of turn. Activate only as a sorcery.");
    let def = CardDef::new("Saddle rule", "", "Creature — Mount", &text, Some((0, 0))).expect("the grammar reads rule 702.171a");
    Some(def.abilities[0].clone())
}

/// 702.41a: affinity is an ordinary self cost reduction over a controlled set.
fn affinity(item: &ir::token::KeywordInstance) -> Option<Ability> {
    if item.keyword.name() != "Affinity" { return None; }
    let KeywordParam::Affinity(quality) = &item.param else { return None; };
    if quality.domain != ir::Domain::Permanent || quality.controller.is_some() || quality.owner.is_some()
        || quality.caster.is_some() || quality.zone.is_some() || quality.other { return None; }
    let mut set = quality.clone();
    set.controller = Some(ir::Relation { who: Box::new(ir::Selector::you()), negated: false, past: false });
    let source = ir::Selector::new(ir::Quant::Definite(ir::Ref::This), ir::ObjectSet::of(ir::Domain::Spell));
    Some(Ability::Text(ir::Effect(vec![ir::Sentence::Do(ir::effect::Body {
        first: ir::effect::Clause { subject: Some(ir::Subject::Selector(source)), may: false, also: false,
            first: ir::effect::Act { action: ir::Action::Static(Box::new(ir::statics::StaticAct::CostChange {
                more: false, by: ir::mana::ManaCost(vec![ir::mana::ManaSymbol::Generic(1)]), of: ir::statics::CostOf::Cast,
            })), mods: vec![ir::effect::Modifier::ForEach(ir::Amount::Count(Box::new(set)))], may: false }, rest: vec![] }, rest: vec![],
    })])))
}

/// 702.24a: put the age counter before computing one atomic generic-mana payment.
/// Other costs remain refused until repeated payment choices have their own lowering.
fn cumulative_upkeep(item: &ir::token::KeywordInstance) -> Option<Ability> {
    if item.keyword.name() != "Cumulative Upkeep" { return None; }
    let KeywordParam::Mana(mana) = &item.param else { return None; };
    let generic = mana.0.iter().try_fold(0u32, |total, symbol| match symbol {
        ir::mana::ManaSymbol::Generic(n) => total.checked_add(*n), _ => None,
    })?;
    let text = format!("At the beginning of your upkeep, if this permanent is on the battlefield, put an age counter on this permanent. You may pay {{{generic}}} for each age counter on this permanent. If you don't, sacrifice this permanent.");
    CardDef::new("Cumulative upkeep rule", "", "Enchantment", &text, None).ok()?.abilities.into_iter().next()
}

/// 702.30a: a fixed mana echo cost is paid through the ordinary retained unless-payment.
fn echo(item: &ir::token::KeywordInstance) -> Option<Ability> {
    if item.keyword.name() != "Echo" { return None; }
    let KeywordParam::Mana(mana) = &item.param else { return None };
    if mana.0.is_empty() || mana.0.iter().any(|symbol| *symbol == crate::cost::X || symbol.mana_payments().is_none()) { return None; }
    let cost: String = mana.0.iter().map(|symbol| format!("{{{}}}", symbol.text())).collect();
    let text = format!("At the beginning of your upkeep, if this permanent came under your control since the beginning of your last upkeep, sacrifice this permanent unless you pay {cost}.");
    CardDef::new("Echo rule", "", "Enchantment", &text, None).ok()?.abilities.into_iter().next()
}

/// 702.54a: the ordinary prospective-entry replacement checks damage history.
fn bloodthirst(item: &ir::token::KeywordInstance) -> Option<Ability> {
    if item.keyword.name() != "Bloodthirst" { return None; }
    let KeywordParam::Number(ir::Amount::Const(n)) = item.param else { return None; };
    static RULE: OnceLock<Ability> = OnceLock::new();
    let mut ability = crate::token::init(&RULE, || CardDef::new("Bloodthirst rule", "", "Creature",
        "If an opponent was dealt damage this turn, this permanent enters with a +1/+1 counter on it.", None)
        .expect("the grammar reads rule 702.54a").abilities[0].clone()).clone();
    entry_counter_count(&mut ability, n);
    Some(ability)
}

/// 702.63a/b: fixed entry counters and two ordinary triggered abilities. Bare
/// vanishing has the triggers alone; zero is a supported fixed count, not a sentinel.
fn vanishing(item: &ir::token::KeywordInstance) -> Option<Vec<Ability>> {
    let count = match item.param {
        KeywordParam::None => None,
        KeywordParam::Number(ir::Amount::Const(n)) => Some(n),
        _ => return None,
    };
    static RULE: OnceLock<Vec<Ability>> = OnceLock::new();
    let rule = crate::token::init(&RULE, || CardDef::new("Vanishing rule", "", "Artifact",
        "This permanent enters with a time counter on it.\nAt the beginning of your upkeep, if this permanent has a time counter on it, remove a time counter from this permanent.\nWhen the last time counter is removed from this permanent, sacrifice this permanent.", None)
        .expect("the grammar reads rule 702.63a").abilities);
    let mut abilities = rule[1..].to_vec();
    if let Some(count) = count {
        let mut entry = rule[0].clone();
        entry_counter_count(&mut entry, count);
        abilities.insert(0, entry);
    }
    Some(abilities)
}

/// 702.32a: inability to pay the mandatory upkeep removal sacrifices the
/// source. Removing the last counter alone does not trigger sacrifice.
fn fading(item: &ir::token::KeywordInstance) -> Option<Vec<Ability>> {
    let KeywordParam::Number(ir::Amount::Const(count)) = item.param else { return None };
    static RULE: OnceLock<Vec<Ability>> = OnceLock::new();
    let mut abilities = crate::token::init(&RULE, || CardDef::new("Fading rule", "", "Artifact",
        "This permanent enters with a fade counter on it.\nAt the beginning of your upkeep, remove a fade counter from this permanent. If you can't, sacrifice this permanent.", None)
        .expect("the grammar reads rule 702.32a").abilities).clone();
    entry_counter_count(&mut abilities[0], count);
    Some(abilities)
}

/// 702.123a: the choice is made on resolution. The ordinary optional action
/// falls through to Servos when counters cannot be put on the source.
fn fabricate(item: &ir::token::KeywordInstance) -> Option<Ability> {
    if item.keyword.name() != "Fabricate" { return None; }
    let KeywordParam::Number(ir::Amount::Const(n)) = item.param else { return None };
    let (number, plural) = if n == 1 { ("a".to_owned(), "") } else { (n.to_string(), "s") };
    let text = format!("When this permanent enters, you may put {number} +1/+1 counter{plural} on this permanent. If you don't, create {number} 1/1 colorless Servo artifact creature token{plural}.");
    CardDef::new("Fabricate rule", "", "Creature", &text, None).ok()?.abilities.into_iter().next()
}

/// 702.43a: transfer the actual last-known counters, independently of printed N.
fn modular(item: &ir::token::KeywordInstance) -> Option<Vec<Ability>> {
    let KeywordParam::Number(ir::Amount::Const(count)) = item.param else { return None };
    static RULE: OnceLock<Vec<Ability>> = OnceLock::new();
    let mut abilities = crate::token::init(&RULE, || CardDef::new("Modular rule", "", "Artifact Creature",
        "This permanent enters with a +1/+1 counter on it.\nWhen this permanent is put into a graveyard from the battlefield, you may put a +1/+1 counter on target artifact creature for each +1/+1 counter on this permanent.", Some((0, 0)))
        .expect("the grammar reads rule 702.43a").abilities).clone();
    entry_counter_count(&mut abilities[0], count);
    Some(abilities)
}

/// Fixed counter entry replacements share the same ordinary typed arrival.
fn entry_counter_count(ability: &mut Ability, count: u32) {
    let Ability::Text(ir::Effect(sentences)) = ability else { unreachable!("entry rule is static") };
    let mut sentence = &mut sentences[0];
    while let ir::Sentence::Led { rest, .. } = sentence { sentence = rest; }
    let ir::Sentence::Do(body) = sentence else { unreachable!("entry statement") };
    let ir::Action::Static(action) = &mut body.first.first.action else { unreachable!("entry statement") };
    let ir::statics::StaticAct::Enters(arrivals) = action.as_mut() else { unreachable!("entry counters") };
    let [ir::statics::Arrival::Counters(on)] = arrivals.as_mut_slice() else { unreachable!("one counter kind") };
    on.counters.count = ir::Amount::Const(count);
}

/// 702.23a/b: count current blockers on resolution and freeze the resulting bonus.
fn rampage(item: &ir::token::KeywordInstance) -> Option<Ability> {
    if item.keyword.name() != "Rampage" { return None; }
    let KeywordParam::Number(ir::Amount::Const(n)) = item.param else { return None };
    let text = format!("Whenever this creature becomes blocked, it gets +{n}/+{n} until end of turn for each creature blocking it beyond the first.");
    CardDef::new("Rampage rule", "", "Creature", &text, None).ok()?.abilities.into_iter().next()
}

/// 702.130a: the blocked occurrence retains its own defending player.
fn afflict(item: &ir::token::KeywordInstance) -> Option<Ability> {
    if item.keyword.name() != "Afflict" { return None; }
    let KeywordParam::Number(ir::Amount::Const(n)) = item.param else { return None };
    let text = format!("Whenever this creature becomes blocked, defending player loses {n} life.");
    CardDef::new("Afflict rule", "", "Creature", &text, None).ok()?.abilities.into_iter().next()
}

/// 702.45a: becoming blocked is one event regardless of the number of blockers.
fn bushido(item: &ir::token::KeywordInstance) -> Option<Ability> {
    if item.keyword.name() != "Bushido" { return None; }
    let KeywordParam::Number(ir::Amount::Const(n)) = item.param else { return None; };
    if n > i32::MAX as u32 { return None; }
    let text = format!("Whenever this creature blocks or becomes blocked, it gets +{n}/+{n} until end of turn.");
    Some(CardDef::new("Bushido rule", "", "Creature", &text, None)
        .expect("the grammar reads rule 702.45a").abilities[0].clone())
}

/// "Renown N" means "When this creature deals combat damage to a player, if it isn't renowned,
/// put N +1/+1 counters on it and it becomes renowned." (702.112a) Each instance triggers
/// separately; the first to resolve makes it renowned (702.112c).
fn renown(item: &ir::token::KeywordInstance) -> Option<Ability> {
    if item.keyword.name() != "Renown" { return None; }
    let KeywordParam::Number(ir::Amount::Const(n @ 1..=10)) = item.param else { return None };
    const WORDS: [&str; 10] = ["a", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten"];
    let text = format!("When this creature deals combat damage to a player, if it isn't renowned, put {} +1/+1 counter{} on it and it becomes renowned.",
        WORDS[n as usize - 1], if n == 1 { "" } else { "s" });
    let def = CardDef::new("Renown rule", "", "Creature", &text, Some((0, 0))).expect("the grammar reads rule 702.112a");
    Some(def.abilities[0].clone())
}

/// "Annihilator N" means "Whenever this creature attacks, defending player sacrifices N
/// permanents." (702.86a); each instance triggers separately (702.86b).
fn annihilator(item: &ir::token::KeywordInstance) -> Option<Ability> {
    if item.keyword.name() != "Annihilator" { return None; }
    let KeywordParam::Number(ir::Amount::Const(n @ 1..=10)) = item.param else { return None };
    const WORDS: [&str; 10] = ["a permanent", "two permanents", "three permanents", "four permanents", "five permanents",
        "six permanents", "seven permanents", "eight permanents", "nine permanents", "ten permanents"];
    let text = format!("Whenever this creature attacks, defending player sacrifices {}.", WORDS[n as usize - 1]);
    let def = CardDef::new("Annihilator rule", "", "Creature", &text, Some((0, 0))).expect("the grammar reads rule 702.86a");
    Some(def.abilities[0].clone())
}

/// "Station" means "Tap another untapped creature you control: Put a number of charge counters
/// on this permanent equal to the tapped creature's power. Activate only as a sorcery."
/// (702.184a) What its symbols give is read with the card's text ([`crate::station`]).
fn station(item: &ir::token::KeywordInstance) -> Option<&'static Ability> {
    if item.keyword.name() != "Station" || item.param != KeywordParam::None { return None; }
    static STATION: OnceLock<Ability> = OnceLock::new();
    Some(crate::token::init(&STATION, || CardDef::new("Station rule", "", "Artifact",
        "Tap another untapped creature you control: Put a number of charge counters on this permanent equal to the tapped creature's power. Activate only as a sorcery.", None)
        .expect("the grammar reads rule 702.184a").abilities[0].clone()))
}

/// "Firebending N" means "Whenever this creature attacks, add N {R}. Until end of combat, you
/// don't lose this mana as steps and phases end." (702.189a): N {R} is one {R} for each of N,
/// a number or "X, where X is …".
fn firebending(item: &ir::token::KeywordInstance) -> Option<Ability> {
    if item.keyword.name() != "Firebending" { return None; }
    let KeywordParam::Number(n) = &item.param else { return None };
    static FIREBENDING: OnceLock<Ability> = OnceLock::new();
    let mut ability = crate::token::init(&FIREBENDING, || CardDef::new("Firebending rule", "", "Creature",
        "Whenever this creature attacks, add {R}. Until end of combat, you don't lose this mana as steps and phases end.", Some((0, 0)))
        .expect("the grammar reads rule 702.189a").abilities[0].clone()).clone();
    let Ability::Triggered { effect, .. } = &mut ability else { unreachable!("firebending triggers") };
    let ir::Sentence::Do(body) = &mut effect.0[0] else { unreachable!("firebending adds mana first") };
    let red = ir::mana::ManaSymbol::Color(ir::Color::Red);
    body.first.first.action = ir::Action::AddMana(match n {
        ir::Amount::Const(n) => ir::mana::ManaSpec::Symbols(ir::mana::ManaCost(vec![red; *n as usize])),
        _ => ir::mana::ManaSpec::AmountOf {symbol:red, amount:n.clone()},
    });
    Some(ability)
}

/// "Mobilize N" means "Whenever this creature attacks, create N 1/1 red Warrior creature
/// tokens. Those tokens enter tapped and attacking. Sacrifice them at the beginning of the next
/// end step." (702.181a): N a number or "X, where X is …".
fn mobilize(item: &ir::token::KeywordInstance) -> Option<Ability> {
    if item.keyword.name() != "Mobilize" { return None; }
    let KeywordParam::Number(n) = &item.param else { return None };
    static MOBILIZE: OnceLock<Ability> = OnceLock::new();
    let mut ability = crate::token::init(&MOBILIZE, || CardDef::new("Mobilize rule", "", "Creature",
        "Whenever this creature attacks, create two 1/1 red Warrior creature tokens that are tapped and attacking. Sacrifice them at the beginning of the next end step.", Some((0, 0)))
        .expect("the grammar reads rule 702.181a").abilities[0].clone()).clone();
    let Ability::Triggered { effect, .. } = &mut ability else { unreachable!("mobilize triggers") };
    let ir::Sentence::Do(body) = &mut effect.0[0] else { unreachable!("mobilize creates first") };
    let ir::Action::Create(tokens) = &mut body.first.first.action else { unreachable!("mobilize creates first") };
    let [spec] = tokens.items.as_mut_slice() else { unreachable!("one kind of token") };
    spec.count = n.clone();
    Some(ability)
}

/// Is this, of a card's abilities (the printed ones, then those [`executable_in`] makes, in
/// order), a crew ability (702.122a)? "Becomes crewed" is said of its resolving (702.122e).
pub(crate) fn crews(def: &CardDef, index: usize) -> bool {
    made_by(def, index).is_some_and(|item| item.keyword.name() == "Crew"
        && matches!(item.param, KeywordParam::Number(ir::Amount::Const(_))))
}

/// Is this, of a card's abilities, a saddle ability (702.171a)? The creatures tapped to pay
/// its cost saddle the permanent (702.171c).
pub(crate) fn saddles(def: &CardDef, index: usize) -> bool {
    made_by(def, index).is_some_and(|item| saddle(item).is_some())
}

/// Is this, of a card's abilities, a ninjutsu ability (702.49a)?
pub(crate) fn ninjutsu(def: &CardDef, index: usize) -> bool {
    made_by(def, index).is_some_and(|item| item.keyword.name() == "Ninjutsu")
}

/// Generated owner indices follow the cached declaration's exact ability order.
pub(crate) fn generated_by(def: &CardDef) -> impl Iterator<Item = (usize, &Instance)> {
    def.keyword_declarations.iter().flat_map(|instance| {
        instance.declared.iter().flat_map(|d| &d.gives)
            .filter_map(move |(_, gives)| matches!(gives, Gives::Ability(_)).then_some(instance))
    }).enumerate().map(move |(offset, instance)| (def.abilities.len() + offset, instance))
}

/// The keyword that made this ability, in the same cached order as [`executable_in`].
fn made_by(def: &CardDef, index: usize) -> Option<&ir::token::KeywordInstance> {
    generated_by(def).find_map(|(made, instance)| (made == index).then_some(&instance.item))
}

/// The in-game ability of 702.124j. The setup API still accepts one commander per seat;
/// having this keyword in a library does not enable the distinct two-commander format.
fn partner_with(name: &str) -> Ability {
    static PARTNER: OnceLock<Ability> = OnceLock::new();
    let mut ability = crate::token::init(&PARTNER, || CardDef::new("Partner rule", "", "Creature",
        "When this permanent enters, target player may search their library for a card, reveal it, put it into their hand, then shuffle.", None)
        .expect("the grammar reads rule 702.124j").abilities[0].clone()).clone();
    let Ability::Triggered { effect, .. } = &mut ability else { unreachable!("partner search is triggered") };
    let ir::Sentence::Do(body) = &mut effect.0[0] else { unreachable!("partner begins with a search") };
    let ir::Action::Search { what, .. } = &mut body.first.first.action else { unreachable!("partner begins with a search") };
    what.items[0].set.filter = ir::Filter::All(vec![ir::Filter::Atom(ir::Atom::Named(ir::Coord {
        items: vec![ir::selector::Name::Card(name.into())], conj: ir::Conj::And,
    }))]);
    ability
}

/// Living weapon is exactly a create-then-attach trigger (702.92a). State-based
/// actions wait until this ordinary program completes, so its 0/0 Germ can be equipped.
fn living_weapon() -> &'static Ability {
    static LIVING_WEAPON: OnceLock<Ability> = OnceLock::new();
    crate::token::init(&LIVING_WEAPON, || {
        let def = CardDef::new("Living weapon rule", "", "Artifact — Equipment",
            "When this Equipment enters, create a 0/0 black Phyrexian Germ creature token, then attach this Equipment to it.", None)
            .expect("the grammar reads rule 702.92a");
        def.abilities[0].clone()
    })
}

/// Job select uses the ordinary retained create/entry/attachment program (702.182a).
fn job_select() -> &'static Ability {
    static JOB_SELECT: OnceLock<Ability> = OnceLock::new();
    crate::token::init(&JOB_SELECT, || CardDef::new("Job select rule", "", "Artifact — Equipment",
        "When this Equipment enters, create a 1/1 colorless Hero creature token, then attach this Equipment to it.", None)
        .expect("the grammar reads rule 702.182a").abilities[0].clone())
}

/// Persist and undying are the same triggered rule shape (702.79a, 702.93a).
/// Read the rule once; ordinary LKI predicates, returns and entry counters execute it.
fn death_return(undying: bool) -> &'static Ability {
    static UNDYING: OnceLock<Ability> = OnceLock::new();
    static PERSIST: OnceLock<Ability> = OnceLock::new();
    let (kept, kind) = if undying { (&UNDYING, "+1/+1") } else { (&PERSIST, "-1/-1") };
    crate::token::init(kept, || {
        let text = format!("When this permanent is put into a graveyard from the battlefield, if it had no {kind} counters on it, return it to the battlefield under its owner's control with a {kind} counter on it.");
        let def = CardDef::new("Death return rule", "", "Creature", &text, None)
            .expect("the grammar reads rules 702.79a and 702.93a");
        def.abilities[0].clone()
    })
}

/// The triggered ability extort is (702.101a). Each instance is its own ability and
/// triggers separately (702.101b). The rule says "the total life lost this way"; the grammar
/// reads the same amount without "total".
fn extort() -> &'static Ability {
    static EXTORT: OnceLock<Ability> = OnceLock::new();
    crate::token::init(&EXTORT, || {
        let def = CardDef::new("Extort rule", "", "Enchantment",
            "Whenever you cast a spell, you may pay {W/B}. If you do, each opponent loses 1 life and you gain life equal to the life lost this way.", None)
            .expect("the grammar reads rule 702.101a");
        def.abilities[0].clone()
    })
}

/// Derived abilities keep their printed row for interface ordering; the original keyword
/// remains available as a characteristic. They are stored once in the shared card definition.
pub(crate) fn executable_in(instances: &[Instance]) -> Vec<(usize, Ability)> {
    instances.iter().filter_map(|i| i.declared.as_ref().ok().map(|d| (i.row, d)))
        .flat_map(|(row, d)| d.gives.iter().filter_map(move |(_, gives)| match gives {
            Gives::Ability(a) => Some((row, a.clone())), _ => None,
        })).collect()
}

/// "Flashback [cost]": the cost a card with flashback may be cast from its owner's graveyard
/// for, rather than its mana cost (702.34a): "Flashback {2}{R}", or a cost that is not only
/// mana, "Flashback—Sacrifice a Mountain."
pub fn flashback(def: &CardDef) -> Option<ir::Cost> {
    contributions(def).find_map(|(scope, gives)| match gives {
        Gives::Route { route: crate::announcement::CastRoute::Flashback, cost: RouteCost::Alternative(cost) }
            if scope.contains(crate::Zone::Graveyard) => Some(cost.clone()),
        _ => None,
    })
}

fn flashback_cost(item: &ir::token::KeywordInstance) -> Option<ir::Cost> {
    if Keyword::from_name("Flashback") != Some(item.keyword) { return None; }
    match &item.param {
        KeywordParam::Mana(mana) => Some(ir::Cost(vec![ir::CostPart::Mana(mana.clone())])),
        KeywordParam::Cost(cost) => Some(cost.clone()),
        _ => None,
    }
}

/// Escape supplies a graveyard casting permission and its own alternative cost (702.138a).
pub(crate) fn escape_cost(item: &ir::token::KeywordInstance) -> Option<ir::Cost> {
    if item.keyword.name() != "Escape" { return None; }
    match &item.param {
        KeywordParam::Mana(mana) => Some(ir::Cost(vec![ir::CostPart::Mana(mana.clone())])),
        KeywordParam::Cost(cost) => Some(cost.clone()),
        _ => None,
    }
}
pub(crate) fn escape(def: &CardDef) -> Option<ir::Cost> { of_items(def).find_map(escape_cost) }
/// Its harmonize cost, cast from a graveyard (702.180a).
pub(crate) fn harmonize(def: &CardDef) -> Option<ir::Cost> {
    contributions(def).find_map(|(_, gives)| match gives {
        Gives::Route { route: crate::announcement::CastRoute::Harmonize { .. }, cost: RouteCost::Alternative(cost) } => Some(cost.clone()),
        _ => None,
    })
}

/// The mana in a card's flashback cost, if it has one and any.
pub(crate) fn flashback_mana(def: &CardDef) -> Option<ir::mana::ManaCost> {
    flashback(def)?.0.into_iter().find_map(|part| match part { ir::CostPart::Mana(mana) => Some(mana), _ => None })
}

/// "Madness [cost]": the cost its card may be cast for, rather than its mana cost, when its
/// madness ability offers that cast (702.35a): "Madness {R}", "Madness—Pay six {C}."
pub(crate) fn madness_cost(item: &ir::token::KeywordInstance) -> Option<ir::Cost> {
    if Keyword::from_name("Madness") != Some(item.keyword) { return None; }
    match &item.param {
        KeywordParam::Mana(mana) => Some(ir::Cost(vec![ir::CostPart::Mana(mana.clone())])),
        // a cost a sentence can say it is cast by paying: mana and actions
        KeywordParam::Cost(cost) if cost.0.iter().all(|p| matches!(p, ir::CostPart::Mana(_) | ir::CostPart::Action(_))) => Some(cost.clone()),
        _ => None,
    }
}

/// Alternative prices available only through their owning linked keyword trigger:
/// the printed cost row, cost, and generated trigger index (702.35a, 702.94a).
pub(crate) fn offered_costs(def: &CardDef) -> Vec<(usize, ir::Cost, usize)> {
    madness(def).into_iter().chain(crate::miracle::instances(def)).collect()
}

/// Each Madness instance's printed row, cost and exact generated trigger.
pub(crate) fn madness(def: &CardDef) -> Vec<(usize, ir::Cost, usize)> {
    generated_by(def).filter_map(|(made, instance)| {
        madness_cost(&instance.item).map(|cost| (instance.row, cost, made))
    }).collect()
}

/// The triggered ability of madness (702.35a): "When this card is exiled this way, its owner
/// may cast it by paying [cost] rather than paying its mana cost. If that player doesn't,
/// they put this card into their graveyard." It triggers as its card's madness replacement
/// is applied to its discard, not by watching for an event (see
/// [`crate::replace::Replacement::Madness`]): the discard pattern it is read with binds "it"
/// to the card as it was exiled. The triggered ability's controller is the card's owner, the
/// player who discarded it.
fn madness_trigger(cost: &ir::Cost) -> Ability {
    static MADNESS: OnceLock<Ability> = OnceLock::new();
    let template = crate::token::init(&MADNESS, || {
        let def = CardDef::new("Madness rule", "", "Enchantment",
            "When you discard this card, you may cast it by paying {0} rather than paying its mana cost. If you don't, put it into your graveyard.", None)
            .expect("the grammar reads rule 702.35a");
        def.abilities[0].clone()
    });
    let mut ability = template.clone();
    let Ability::Triggered { effect: ir::Effect(sentences), .. } = &mut ability else { unreachable!("the rule is a triggered ability") };
    let Some(ir::Sentence::Do(body)) = sentences.first_mut() else { unreachable!("the rule's first sentence is its cast") };
    let Some(ir::effect::Modifier::By(said)) = body.first.first.mods.first_mut() else { unreachable!("the cast is by paying a cost") };
    said.actions.items = cost.0.iter().map(|part| match part {
        ir::CostPart::Mana(mana) => ir::Action::PayMana(mana.clone()),
        ir::CostPart::Action(action) => action.clone(),
        _ => unreachable!("a madness cost is mana and actions"),
    }).collect();
    ability
}

/// The cost a "by paying [cost] rather than paying its mana cost" says, as a cost.
pub(crate) fn said_cost(said: &ir::statics::CastCost) -> ir::Cost {
    ir::Cost(said.actions.items.iter().map(|action| match action {
        ir::Action::PayMana(mana) => ir::CostPart::Mana(mana.clone()),
        other => ir::CostPart::Action(other.clone()),
    }).collect())
}

/// "Ward {2}", "Ward—Pay 7 life.", "Ward—Discard a card." (702.21a): the cost of a ward
/// ability, mana and the actions its trigger's "unless" has the opponent perform. Whether the
/// engine performs those actions is the trigger's own admission ([`ward_trigger`]). A ward cost
/// with X (702.21b) is refused as a keyword the engine does not run: no X is chosen for it.
fn ward_cost(item: &ir::token::KeywordInstance) -> Option<ir::Cost> {
    if Keyword::from_name("Ward") != Some(item.keyword) { return None; }
    let no_x = |mana: &ir::mana::ManaCost| !mana.0.contains(&crate::cost::X);
    match &item.param {
        KeywordParam::Mana(mana) if no_x(mana) => Some(ir::Cost(vec![ir::CostPart::Mana(mana.clone())])),
        KeywordParam::Cost(cost) if !cost.0.is_empty() && cost.0.iter().all(|p| match p {
            ir::CostPart::Mana(mana) => no_x(mana),
            ir::CostPart::Action(_) => true,
            // "Ward—Discard a card or pay {2}": the opponent chooses which (702.21a)
            ir::CostPart::Joined(joined) => cost.0.len() == 1 && joined.conj == ir::Conj::Or,
            _ => false,
        }) => Some(cost.clone()),
        _ => None,
    }
}

/// Ward is a triggered ability (702.21a): "Whenever this permanent becomes the target of a
/// spell or ability an opponent controls, counter that spell or ability unless that player
/// pays [cost]." Said here as rule text, with "its controller" for "that player" (the
/// opponent who controls it), and the instance's cost substituted into its "unless".
fn ward_trigger(cost: &ir::Cost) -> Ability {
    static WARD: OnceLock<Ability> = OnceLock::new();
    let template = crate::token::init(&WARD, || {
        let def = CardDef::new("Ward rule", "", "Enchantment",
            "Whenever this permanent becomes the target of a spell or ability an opponent controls, counter that spell or ability unless its controller pays {0}.", None)
            .unwrap_or_else(|e| panic!("the grammar reads rule 702.21a: {e}"));
        def.abilities[0].clone()
    });
    let mut ability = template.clone();
    let Ability::Triggered { effect: ir::Effect(sentences), .. } = &mut ability else { unreachable!("ward is a triggered ability") };
    let Some(ir::Sentence::Do(body)) = sentences.first_mut() else { unreachable!("the rule's sentence is its counter") };
    let Some(ir::effect::Modifier::Unless { actions, .. }) = body.first.first.mods.first_mut() else { unreachable!("the counter is unless a payment") };
    if let [ir::CostPart::Joined(joined)] = cost.0.as_slice() {
        *actions = joined.clone();
        return ability;
    }
    actions.items = cost.0.iter().map(|part| match part {
        ir::CostPart::Mana(mana) => ir::Action::PayMana(mana.clone()),
        ir::CostPart::Action(action) => action.clone(),
        _ => unreachable!("a ward cost is mana and actions"),
    }).collect();
    ability
}

/// "Gift a [something]" (702.174a): what its card's gift gives.
pub(crate) fn gift(def: &CardDef) -> Option<&ir::token::GiftOf> {
    of_items(def).find_map(gift_of)
}

fn gift_of(item: &ir::token::KeywordInstance) -> Option<&ir::token::GiftOf> {
    match &item.param {
        KeywordParam::Gift(of) if item.keyword.name() == "Gift" => Some(of),
        _ => None,
    }
}

/// The second ability of gift on an instant or sorcery (702.174b): "If this spell's gift cost
/// was paid, [effect]", the effect as 702.174d–i define it, said as rule text: "If the gift
/// was promised, the chosen player creates a tapped 1/1 blue Fish creature token." None: a
/// gift the engine does not give (an extra turn, 702.174g; an Octopus, given only by
/// permanents, 702.174i).
fn gift_effect(of: &ir::token::GiftOf) -> Option<Ability> {
    use ir::token::GiftOf;
    static CARD: OnceLock<Ability> = OnceLock::new();
    static FISH: OnceLock<Ability> = OnceLock::new();
    static FOOD: OnceLock<Ability> = OnceLock::new();
    static TREASURE: OnceLock<Ability> = OnceLock::new();
    let (kept, effect, rule) = match of {
        GiftOf::Card => (&CARD, "the chosen player draws a card.", "702.174e"),
        GiftOf::Token { tapped: true, name } if name == "Fish" => (&FISH, "the chosen player creates a tapped 1/1 blue Fish creature token.", "702.174f"),
        GiftOf::Token { tapped: false, name } if name == "Food" => (&FOOD, "the chosen player creates a Food token.", "702.174d"),
        GiftOf::Token { tapped: false, name } if name == "Treasure" => (&TREASURE, "the chosen player creates a Treasure token.", "702.174h"),
        _ => return None,
    };
    Some(crate::token::init(kept, || {
        let def = CardDef::new("Gift rule", "", "Instant", &format!("If the gift was promised, {effect}"), None)
            .unwrap_or_else(|e| panic!("the grammar reads rule {rule}: {e}"));
        def.abilities[0].clone()
    }).clone())
}

/// The index of the gift ability among its card's abilities (after the printed ones, in the
/// order [`executable_in`] makes them), when it has one the engine gives.
pub(crate) fn gift_ability(def: &CardDef) -> Option<usize> {
    generated_by(def).find_map(|(made, instance)| gift_of(&instance.item).is_some().then_some(made))
}

/// One supported kicker cost and whether it can be paid repeatedly (702.33a,c).
pub(crate) fn kicker(def: &CardDef) -> Option<(&ir::mana::ManaCost, bool)> {
    contributions(def).find_map(|(scope, gives)| match gives {
        Gives::Additional { cost, times, then: Paid::Kicked } if scope.contains(crate::Zone::Stack) => Some((cost, *times == Times::Repeatable)),
        _ => None,
    })
}

pub(crate) fn kicker_cost(item: &ir::token::KeywordInstance) -> Option<(&ir::mana::ManaCost, bool)> {
    match &item.param {
        KeywordParam::Mana(cost) if matches!(item.keyword.name(), "Kicker" | "Multikicker")
            && cost.0.iter().all(|s| s.mana_payments().is_some() || matches!(s, ir::mana::ManaSymbol::Phyrexian(_) | ir::mana::ManaSymbol::HybridPhyrexian(..))) => Some((cost, item.keyword.name() == "Multikicker")),
        _ => None,
    }
}

pub(crate) fn entwine(def: &CardDef) -> Option<&ir::mana::ManaCost> {
    contributions(def).find_map(|(scope,gives)| match gives {
        Gives::Additional { cost, then: Paid::AllModes, .. } if scope.contains(crate::Zone::Stack) => Some(cost),
        _ => None,
    })
}
pub(crate) fn entwine_count(def: &CardDef) -> usize {
    contributions(def).filter(|(_,g)| matches!(g,Gives::Additional {then:Paid::AllModes,..})).count()
}

/// Its escalate cost, paid once for each mode chosen beyond the first (702.120a).
pub(crate) fn escalate(def: &CardDef) -> Option<&ir::mana::ManaCost> {
    contributions(def).find_map(|(scope, gives)| match gives {
        Gives::Additional { cost, then: Paid::Escalated, .. } if scope.contains(crate::Zone::Stack) => Some(cost),
        _ => None,
    })
}
pub(crate) fn escalate_count(def: &CardDef) -> usize {
    contributions(def).filter(|(_, g)| matches!(g, Gives::Additional { then: Paid::Escalated, .. })).count()
}

pub(crate) fn buyback(def: &CardDef) -> Option<&ir::mana::ManaCost> {
    contributions(def).find_map(|(scope,gives)| match gives {
        Gives::Additional { cost, then: Paid::ReturnsToHand, .. } if scope.contains(crate::Zone::Stack) => Some(cost),
        _ => None,
    })
}
pub(crate) fn buyback_count(def: &CardDef) -> usize {
    contributions(def).filter(|(_,g)| matches!(g,Gives::Additional {then:Paid::ReturnsToHand,..})).count()
}

pub(crate) fn kicker_count(def: &CardDef) -> usize { contributions(def).filter(|(_, g)| matches!(g, Gives::Additional { then: Paid::Kicked, .. })).count() }

pub(crate) fn of_items(def: &CardDef) -> impl Iterator<Item = &ir::token::KeywordInstance> {
    def.abilities.iter().flat_map(|a| match a { Ability::Keywords(v) => v.as_slice(), _ => &[] })
}

impl Game {
    /// Does this object have this keyword ability, now? What its card is printed with and
    /// what continuous effects give it (613.1f); nothing removes abilities yet.
    /// A printed delve instance is the linked cost-exile producer (607.2q).
    pub(crate) fn delve_origin(&self, object: ObjId) -> Option<crate::binding::AbilityKey> {
        self.seen_for(object, crate::layers::Aspect::Abilities).base_abilities.then_some(())?;
        self.def(object)?.abilities.iter().enumerate().find_map(|(row, a)|
            matches!(a, Ability::Keywords(items) if items.iter().any(|i| i.keyword.name() == "Delve" && i.param == KeywordParam::None))
                .then_some(crate::binding::AbilityKey::printed(row)))
    }

    pub fn has(&self, o: ObjId, keyword: &str) -> bool {
        let Some(k) = Keyword::from_name(keyword) else { return false };
        self.has_keyword(o, k)
    }

    pub(crate) fn has_keyword(&self, o: ObjId, k: Keyword) -> bool {
        self.seen_for(o, crate::layers::Aspect::Abilities).has(k)
    }
}

#[cfg(test)]
mod landwalk_tests {
    use super::*;
    use ir::{Atom, CardType, Color, Filter, Subtype};
    /// Every type a landwalk ability is read with is one the shared selector evaluation can
    /// tell (702.14a, c), lowered with its definition: the blocking restriction asks what was
    /// lowered, never a second evaluator.
    #[test]
    fn r702_14a_each_read_landwalk_type_is_one_the_selector_describes() {
        for text in ["Islandwalk", "Snow forestwalk", "Nonbasic landwalk", "Legendary landwalk", "Snow landwalk", "Artifact landwalk", "Desertwalk"] {
            let def = CardDef::new("Walker", "", "Creature — Bear", text, Some((2, 2))).unwrap();
            assert!(def.keywords.iter().any(|k| k.landwalk().is_some()), "{text}: a supported landwalk");
        }
    }

    /// Anything that is not a land type, a card type or a supertype (with or without "non") is
    /// not a landwalk type the engine runs: it is refused, with what it stopped at.
    #[test]
    fn r702_14a_a_type_other_than_types_and_supertypes_is_refused() {
        let land = Filter::Atom(Atom::Type(CardType::Land));
        let red = Filter::All(vec![Filter::Atom(Atom::Color(Color::Red)), land]);
        assert_eq!(landwalk_ok(&red).unwrap_err().0, "landwalk: a type described by Atom (702.14a)");
        let goblin = Filter::All(vec![Filter::Atom(Atom::Subtype(Subtype::from_name("Goblin").unwrap()))]);
        assert_eq!(landwalk_ok(&goblin).unwrap_err().0, "landwalk: Goblin, not a land type (702.14a)");
        assert_eq!(landwalk_ok(&Filter::All(vec![])).unwrap_err().0, "landwalk: no type (702.14a)");
        let refused = ir::token::KeywordInstance { keyword: Keyword::from_name("Landwalk").unwrap(), param: KeywordParam::Landwalk(red) };
        let def = CardDef::new("Walker", "", "Creature — Bear", "", Some((2, 2))).unwrap();
        assert!(!passive(&refused));
        assert_eq!(Held::compile(&refused, &Support::definition(&def, None)).unwrap_err().0,
            "landwalk: a type described by Atom (702.14a)");
        assert_eq!(passive_ok(&refused).unwrap_err().0, "landwalk: a type described by Atom (702.14a)");
    }
}

#[cfg(test)]
mod death_return_tests {
    use crate::{CardDef, Game, Move, Zone};
    #[test]
    fn r702_93_trigger_controller_is_last_controller_but_return_is_under_owners_control() {
        let def = CardDef::new("Strangleroot Geist", "{G}{G}", "Creature — Spirit", "Haste\nUndying", Some((2, 1))).unwrap();
        crate::play::check(&def).unwrap();
        let mut g = Game::new(vec![def], 3, 20);
        let source = g.put(0, 0, Zone::Battlefield);
        // Prepare a control-change fixture; changing control by an effect is a separate mechanism.
        g.obj_mut(source).controller = 1;
        g.destroy(source); g.act(0, Move::Pass).unwrap();
        assert_eq!(g.obj(g.stack[0]).controller, 1);
        assert_eq!(g.players[0].graveyard.len(), 1);
        assert!(g.players[1].graveyard.is_empty());
        let clone = g.clone();
        for mut branch in [g, clone] {
            for _ in 0..3 { branch.act(branch.priority, Move::Pass).unwrap(); }
            let returned = branch.obj(branch.battlefield[0]);
            assert_eq!((returned.owner, returned.controller), (0, 0));
            assert_eq!(returned.counters.get("+1/+1"), Some(&1));
        }
    }
}

/// 702.56a: one repeated additional mana cost. Multiple instances require separate identities.
pub(crate) fn replicate_cost(item: &ir::token::KeywordInstance) -> Option<&ir::mana::ManaCost> {
    match &item.param { KeywordParam::Mana(cost) if item.keyword.name() == "Replicate"
        && cost.0.iter().all(|s| s.mana_payments().is_some()) => Some(cost), _ => None }
}
pub(crate) fn replicate(def: &CardDef) -> Option<&ir::mana::ManaCost> { of_items(def).find_map(replicate_cost) }
pub(crate) fn replicate_count(def: &CardDef) -> usize { contributions(def).filter(|(_, g)| matches!(g, Gives::Additional { then: Paid::Replicated, .. })).count() }
fn replicate_trigger() -> Ability {
    let source = ir::Selector::new(ir::Quant::Definite(ir::Ref::This), ir::ObjectSet::of(ir::Domain::Spell));
    spell_copy_trigger(ir::Amount::Times { what: Box::new(source), fact: ir::amount::Occurrence::Replicated }, true)
}
/// 702.40a: every player's earlier casts count, and later casts do not.
fn storm_trigger() -> Ability {
    let one = |domain| ir::Selector::new(ir::Quant::Indefinite(ir::Count::Exactly(ir::Amount::Const(1))), ir::ObjectSet::of(domain));
    let amount = ir::Amount::Participants {
        pattern: Box::new(ir::EventPattern::new(vec![one(ir::Domain::Player)], vec![ir::EventAction::does(ir::Verb::Cast, one(ir::Domain::Spell))])),
        counted: ir::amount::Role::Object, when: ir::amount::When::ThisTurn,
    };
    spell_copy_trigger(ir::Amount::AsOf { amount: Box::new(amount), moment: ir::amount::Moment::Casting }, false)
}
fn spell_copy_trigger(amount: ir::Amount, requires_payment: bool) -> Ability {
    use ir::{Action, Amount, Condition, Effect, Lead, Modifier, ObjectSet, Quant, Ref, Selector, Sentence};
    use ir::effect::{Act, Body, Clause};
    use ir::event::{Arg, Deed, EventPattern, TriggerWord};
    // 702.56a expands into ordinary typed trigger, condition, quantity and copy actions.
    // It never reparses a generated rule sentence or duplicates the copy executor.
    let source = Selector::new(Quant::Definite(Ref::This), ObjectSet::of(ir::Domain::Spell));
    let sentence = |action, mods, may: bool| Sentence::Do(Body { first: Clause {
        subject: may.then(|| ir::effect::Subject::Selector(Selector::you())), may, also: false, first: Act { action, mods, may: false }, rest: Vec::new(),
    }, rest: Vec::new() });
    let copies = Selector::new(Quant::Definite(Ref::Anaphor(ir::Anaphor::Copy { plural: true })), ObjectSet::of(ir::Domain::Object));
    Ability::Triggered {
        trigger: ir::Trigger::Event { word: TriggerWord::When, pattern: EventPattern {
            subjects: vec![Selector::you()], actions: vec![ir::EventAction::Deed(Deed {
                verb: ir::Verb::Cast, passive: false, args: vec![(ir::Role::What, Arg::Noun(source.clone()))],
            })], nth_each_turn: None, during: None, except_nth_in_step: None, condition: None, cause: None,
        }},
        effect: Effect(vec![
            {
                let copying = sentence(Action::Copy(ir::Objects { items: vec![source], conj: ir::Conj::And }), vec![Modifier::ForEach(amount.clone())], false);
                if requires_payment { Sentence::Led { lead: Lead::Modifier(Modifier::If(Condition::Compare { left: amount, cmp: ir::Cmp::Ge, right: Amount::Const(1) })), rest: Box::new(copying) } }
                else { copying }
            },
            sentence(Action::ChooseNewTargets { for_: ir::Objects { items: vec![copies], conj: ir::Conj::And }, one: false }, Vec::new(), true),
        ]),
    }
}
