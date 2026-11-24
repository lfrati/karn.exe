//! Compiled conditions: what "if …" asks of the game as it is (608.2c, 608.2h).
//!
//! [`Condition::compile`] is the one place a condition is refused. What it returns asks the
//! present state in the engine's own types: what a player controls, what exists, what a thing
//! is, whose turn it is, how two numbers compare. Nothing here reads the text again as it is
//! asked, and asking cannot fail.
//!
//! What an earlier instruction did ("if you do") is the resolution's own outcome, bound by
//! the program's control flow, not a question about the state: it is refused here. What
//! happened earlier in the turn ("if a creature died this turn", 608.2i) is not asked yet.
use super::expr::{self, Bind, Locals};
use crate::binding::Scalar;
use crate::event::Target;
use crate::select::{Ctx, Support, is_an_opponent, is_you, name};
use crate::state::{Game, PlayerId};
use crate::{Res, unsupported};
use ir::amount::Cmp;
use ir::condition::Predicate;
use ir::{Count, Quant, Ref, Selector};

/// The players a condition is about, and whether it must hold of every one of them: "you",
/// "each player"; of "an opponent" or "a player", one is enough.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Players {
    who: Who,
    every: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Who {
    You,
    Of(expr::Set),
    Reference(expr::Subject),
}

impl Players {
    fn compile(who: &Selector, support: &Support<'_>, bind: Bind<'_>) -> Res<Players> {
        let mut of = |every| Ok(Players { who: Who::Of(expr::Set::compile(&who.set, support, &mut *bind)?), every });
        match &who.quant {
            Quant::Definite(Ref::You) => Ok(Players { who: Who::You, every: true }),
            Quant::Definite(_) => Ok(Players { who: Who::Reference(expr::Subject::compile(who, bind)?), every: false }),
            Quant::Each | Quant::All => of(true),
            Quant::Indefinite(Count::Exactly(ir::Amount::Const(1))) => of(false),
            q => unsupported(format!("condition: of \"{}\" player", name(q))),
        }
    }

    fn players(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals) -> Vec<PlayerId> {
        match &self.who {
            Who::You => vec![ctx.controller],
            Who::Reference(subject) => subject.evaluate(game, ctx, locals).into_iter()
                .filter_map(|t| if let Target::Player(p) = t { Some(p) } else { None }).collect(),
            Who::Of(set) => set.members(game, ctx, locals).into_iter().filter_map(|t| if let Target::Player(p) = t { Some(p) } else { None }).collect(),
        }
    }

    /// Is it so of these players: of every one of them, or of one?
    fn hold(&self, mut each: impl Iterator<Item = bool>) -> bool {
        if self.every { each.all(|x| x) } else { each.any(|x| x) }
    }

    fn scalars(&self, out: &mut Vec<Scalar>) {
        match &self.who {
            Who::Of(set) => out.extend(set.scalars()),
            Who::Reference(subject) => subject.scalars(out),
            Who::You => {},
        }
    }
}

/// How many a noun phrase's determiner says. In a condition a bare count is a lower bound
/// ("you control two creatures"), "no" is none, "exactly two" that many and no more (107.1).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Quantity {
    None,
    AtLeast(expr::Number),
    AtMost(expr::Number),
    Exactly(expr::Number),
    Between(expr::Number, expr::Number),
    Any,
}

impl Quantity {
    fn compile(quant: &Quant, support: &Support<'_>, bind: Bind<'_>) -> Res<Quantity> {
        let mut n = |a: &ir::Amount| expr::Number::compile(a, support, &mut *bind);
        Ok(match quant {
            Quant::No => Quantity::None,
            Quant::Indefinite(Count::Exactly(a) | Count::AtLeast(a)) => Quantity::AtLeast(n(a)?),
            Quant::Indefinite(Count::UpTo(a)) => Quantity::AtMost(n(a)?),
            Quant::Indefinite(Count::Precisely(a)) => Quantity::Exactly(n(a)?),
            Quant::Indefinite(Count::Range { min, max }) => Quantity::Between(n(min)?, n(max)?),
            Quant::Indefinite(Count::AnyNumber) => Quantity::Any,
            q => return unsupported(format!("condition: how many, of \"{}\"", name(q))),
        })
    }

    fn event(q: &ir::event::Quantity, support: &Support<'_>, bind: Bind<'_>) -> Res<Self> {
        Ok(match q {
            ir::event::Quantity::AtLeast(n) => Self::AtLeast(expr::Number::compile(n, support, bind)?),
            ir::event::Quantity::Exactly(n) => Self::Exactly(expr::Number::compile(n, support, bind)?),
        })
    }

    fn is(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals, count: usize) -> bool {
        let n = |a: &expr::Number| a.evaluate(game, ctx, locals);
        let count = count as i64;
        match self {
            Quantity::None => count == 0,
            Quantity::AtLeast(a) => count >= n(a),
            Quantity::AtMost(a) => count <= n(a),
            Quantity::Exactly(a) => count == n(a),
            Quantity::Between(min, max) => n(min) <= count && count <= n(max),
            Quantity::Any => true,
        }
    }

    fn scalars(&self, out: &mut Vec<Scalar>) {
        match self {
            Quantity::AtLeast(a) | Quantity::AtMost(a) | Quantity::Exactly(a) => a.scalars(out),
            Quantity::Between(min, max) => {
                min.scalars(out);
                max.scalars(out);
            }
            Quantity::None | Quantity::Any => {}
        }
    }
}

/// A quantity of the actual mana paid for a spell (601.2h), rather than mana value.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ManaMeasure { Total, Color(ir::Color), Colorless, Colored, Colors }
#[derive(Debug, Clone, PartialEq, Eq)]
enum PaidTest { Count(ManaMeasure, Quantity), Symbols(crate::state::PaidMana) }
impl PaidTest {
    fn compile(mana: &ir::condition::SpentMana, support: &Support<'_>, bind: Bind<'_>) -> Res<Self> {
        use ir::condition::{SpentMana, ManaKind};
        let quantity = |q: &ir::event::Quantity, bind: Bind<'_>| Quantity::event(q, support, bind);
        Ok(match mana {
            SpentMana::Count { count, kind } => Self::Count(match kind {
                ManaKind::Any => ManaMeasure::Total,
                ManaKind::Color(c) => ManaMeasure::Color(*c),
                ManaKind::Colorless => ManaMeasure::Colorless,
                ManaKind::Colored => ManaMeasure::Colored,
                _ => return unsupported("mana spent: source or same-color relation"),
            }, quantity(count, bind)?),
            SpentMana::Colors(count) => Self::Count(ManaMeasure::Colors, quantity(count, bind)?),
            SpentMana::Symbols(cost) => {
                let mut paid = crate::state::PaidMana::default();
                for symbol in &cost.0 {
                    match symbol {
                        ir::mana::ManaSymbol::Color(c) => paid.colors[*c as usize] += 1,
                        ir::mana::ManaSymbol::Colorless => paid.colorless += 1,
                        _ => return unsupported("mana spent: symbols must describe mana types"),
                    }
                }
                Self::Symbols(paid)
            }
            _ => return unsupported("mana spent: produced mana reference"),
        })
    }
    fn matches(&self, paid: &crate::state::PaidMana, game: &Game, ctx: &Ctx, locals: &dyn Locals) -> bool {
        match self {
            Self::Count(kind, quantity) => {
                let n = match kind {
                    ManaMeasure::Total => paid.total(), ManaMeasure::Color(c) => paid.colors[*c as usize],
                    ManaMeasure::Colorless => paid.colorless, ManaMeasure::Colored => paid.colors.iter().sum(),
                    ManaMeasure::Colors => paid.colors.iter().filter(|n| **n > 0).count() as u32,
                };
                quantity.is(game, ctx, locals, n as usize)
            }
            Self::Symbols(required) => paid.colorless >= required.colorless && paid.colors.iter().zip(required.colors).all(|(a,b)| *a >= b),
        }
    }
}

/// What a condition says something is: "~", "you", or what the text has named before, as
/// the binder says ("if it's blue": the target; "if the discarded card wasn't a land card":
/// the card paid for the cost).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Thing {
    Source,
    Controller,
    Named(expr::Subject),
}

/// What it is said to be: "tapped", "a creature".
#[derive(Debug, Clone, PartialEq, Eq)]
enum Is {
    Adjective(expr::Filter),
    A(expr::Set),
    /// Current zone, independent of departure characteristics or a historical view.
    Zone(crate::Zone),
    /// "a land card": a card (108.1; not a token or an ability, 111.6), as described,
    /// wherever it is or last was.
    Card(expr::Filter),
    /// "monstrous", "renowned" (701.37b, 702.112b): a permanent's designation.
    Designated(ir::condition::ObjectDesignation),
}

/// The values a condition compares. Player quantities retain their determiner (one opponent
/// may suffice, "each opponent" requires all of them) and life totals stay signed (102.2,
/// 107.1b, 119.1).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Values {
    Life(Players),
    StartingLife(Players),
    /// "your speed": 0 for a player who has none (702.179f).
    Speed(Players),
    Number(expr::Number),
}

impl Values {
    fn compile(a: &ir::Amount, support: &Support<'_>, bind: Bind<'_>) -> Res<Values> {
        Ok(match a {
            ir::Amount::Life(who) => Values::Life(Players::compile(who, support, bind)?),
            ir::Amount::StartingLife(who) => Values::StartingLife(Players::compile(who, support, bind)?),
            ir::Amount::Speed(who) => Values::Speed(Players::compile(who, support, bind)?),
            other => Values::Number(expr::Number::compile(other, support, bind)?),
        })
    }

    /// The values, and whether the comparison must hold of every one of them.
    fn values(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals) -> (Vec<i64>, bool) {
        match self {
            Values::Life(who) => (who.players(game, ctx, locals).into_iter().map(|p| i64::from(game.players[p].life)).collect(), who.every),
            Values::StartingLife(who) => (who.players(game, ctx, locals).iter().map(|_| i64::from(game.starting_life)).collect(), who.every),
            Values::Speed(who) => (who.players(game, ctx, locals).into_iter().map(|p| i64::from(game.speed(p))).collect(), who.every),
            Values::Number(n) => (vec![n.evaluate(game, ctx, locals)], true),
        }
    }

    fn scalars(&self, out: &mut Vec<Scalar>) {
        match self {
            Values::Life(who) | Values::StartingLife(who) | Values::Speed(who) => who.scalars(out),
            Values::Number(n) => n.scalars(out),
        }
    }
}

/// What a player has more or less of than another: a life total (119), or how many such
/// permanents they control.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Measure {
    Life,
    Controls(expr::Set),
}

impl Measure {
    fn of(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals, player: PlayerId) -> i64 {
        match self {
            Measure::Life => i64::from(game.players[player].life),
            Measure::Controls(set) => ctx.permanents(game).iter()
                .filter(|o| ctx.controlled_by(game, **o) == player && set.contains(game, ctx, locals, Target::Object(**o))).count() as i64,
        }
    }
}

/// What the measure is held against: the same measure of other players, or a number.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Than {
    Players(Players),
    Values(Values),
}

/// A question about the game as it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Condition(Asks);

#[derive(Debug, Clone, PartialEq, Eq)]
enum Asks {
    /// One of the named spell or ability's announced targets fits this description (115.1).
    Targets { what: expr::Subject, target: expr::Set },
    Kicked { negated: bool },
    /// "if this spell was bargained" (702.166b), "if this spell's additional cost was paid",
    /// "if its offspring cost was paid" (702.175a), "if a casualty cost was paid for it"
    /// (702.153a), "if evidence was collected" (701.59c), "if a Dragon was beheld" (701.4b): its
    /// optional additional cost was announced paid (601.2b, 607.2).
    OptionalPaid { negated: bool },
    /// "if its evoke cost was paid": it was cast for the alternative cost at this row (702.74a).
    CostPaid { row: usize, negated: bool },
    /// "that creature was cast for its warp cost": each of these objects was ([`crate::Game::cast_for`]).
    CastFor { what: expr::Subject, keyword: ir::Keyword, negated: bool },
    /// "If the gift was promised" (702.174k): an opponent was chosen as the spell was cast.
    Gift { negated: bool },
    CastBy { players: Players, negated: bool },
    ManaSpent { spell: expr::Subject, test: PaidTest, negated: bool },
    Stored { slot: usize, is: bool },
    Outcome { paid: usize, possible: usize, outcome: ir::condition::Outcome },
    Coin { slot: usize, won: bool },
    /// "you control a Swamp", "you control no creatures", "an opponent controls two or more
    /// artifacts": several noun phrases are alternatives.
    Controls { players: Players, what: Vec<(expr::Set, Quantity)>, negated: bool },
    /// "If you controlled that permanent": who controlled one object named before, as it
    /// last existed on the battlefield if it has left (608.2h).
    Controlled { players: Players, what: expr::Subject, negated: bool },
    /// "there is a creature with power 4 or greater on the battlefield"
    Exists { what: expr::Set, count: Quantity },
    /// "~ is tapped", "~ is a creature", "~ isn't attacking"
    /// `past`: asked of what it was as it last existed (608.2h), a permanent that has left
    /// being described as the permanent it was.
    Is { what: Thing, is: Is, negated: bool, past: bool },
    /// Counter facts of the source immediately before its departure (603.10a).
    HadCounters { kind: Option<ir::CounterKind>, count: Quantity, negated: bool },
    /// "it's your turn", "it's not your turn"; `yours`: or an opponent's.
    Turn { yours: bool, negated: bool },
    /// "it's your first, second, or third turn of the game"
    NthTurn { turns: Vec<u32> },
    /// Its retained cast ordinal, or the pending announcement's next ordinal (608.2i).
    GameCastChoice { n: u32 },
    /// Historical acquisition by this ability's controller since their prior upkeep.
    CameUnderControlSinceUpkeep,
    TurnOf { who: expr::Subject, negated: bool },
    /// "your life total is greater than your starting life total"
    Compare { left: Values, cmp: Cmp, right: Values },
    /// "you have less life than an opponent", "an opponent controls more lands than you":
    /// these players' measure against that of each or one of `than`. `margin`: by at least
    /// this much; None is by any amount.
    MoreThan { players: Players, measure: Measure, fewer: bool, margin: Option<expr::Number>, than: Than },
    /// "a creature died this turn", "you've cast two or more spells this turn": how many times
    /// it has happened since the turn began (608.2i), read from what was noted as it did
    /// ([`crate::history`]); "no spells were cast last turn": or in the turn before. `each`:
    /// "a player cast two or more spells", that many times by one player.
    Entered { kind: ir::CardType, under: Option<Players>, count: Quantity, negated: bool },
    Departed { under: Option<Players>, count: Quantity, negated: bool },
    /// "if you attacked with two or more creatures this turn": the creatures these players
    /// declared as attackers this turn that `those` describes, as each was or last was.
    AttackedWith { players: Players, those: expr::Set, count: Quantity, negated: bool },
    LifeLost { players: Players, count: Quantity, negated: bool },
    Happened { pattern: crate::trigger::Pattern, turn: crate::history::Turn, each: bool, times: Quantity, negated: bool },
    /// "this spell was cast from a graveyard", "… from your hand", "… from anywhere other
    /// than your hand": the zone the spell whose text this is was cast from (601.2a), one of
    /// these or `other_than` them; `yours`: and the zone was this player's own.
    CastFrom { zones: Vec<crate::Zone>, yours: bool, other_than: bool, negated: bool },
    And(Vec<Asks>),
    Or(Vec<Asks>),
}

/// "a permanent left the battlefield this turn", "an artifact entered under your control this
/// turn": one unqualified permanent description of this verb, maybe under whose control,
/// which the turn's per-controller summaries answer ([`crate::history`]). Its set.
fn summarized(pattern: &ir::EventPattern, verb: ir::Verb) -> Option<&ir::ObjectSet> {
    let ([subject], [ir::EventAction::Deed(deed)]) = (pattern.subjects.as_slice(), pattern.actions.as_slice()) else { return None };
    let mut bare = ir::ObjectSet::of(ir::Domain::Permanent); bare.filter = subject.set.filter.clone();
    let plain = subject.set == bare && subject.chooser.is_none() && subject.random.is_none()
        && !pattern.counted() && pattern.during.is_none() && pattern.condition.is_none() && pattern.cause.is_none();
    let args = matches!(deed.args.as_slice(), [] | [(ir::Role::Under, ir::event::Arg::Noun(_))]);
    (deed.verb == verb && !deed.passive && plain && args).then_some(&subject.set)
}

fn summary_under(pattern: &ir::EventPattern, support: &Support<'_>, bind: Bind<'_>) -> Res<Option<Players>> {
    let [ir::EventAction::Deed(deed)] = pattern.actions.as_slice() else { unreachable!() };
    match deed.args.as_slice() {
        [(ir::Role::Under, ir::event::Arg::Noun(player))] => Players::compile(player, support, bind).map(Some),
        _ => Ok(None),
    }
}

/// "this spell was cast from [a zone]": the zone said, if that is what the pattern says.
fn cast_from(pattern: &ir::EventPattern) -> Option<&ir::event::ZoneArg> {
    let ([subject], [ir::event::EventAction::Deed(deed)]) = (pattern.subjects.as_slice(), pattern.actions.as_slice()) else { return None };
    let [(ir::Role::From, ir::event::Arg::Zone(from))] = deed.args.as_slice() else { return None };
    (subject.quant == Quant::Definite(ir::Ref::This) && deed.verb == ir::Verb::Cast && deed.passive).then_some(from)
}

/// "a creature card is exiled this way", "you exiled a land card this way", "you put a Town
/// card into your hand this way": the verb of the instruction that is asked about, what is
/// asked of its result, whether of what the controller did, and where it was put, if that is
/// all it says. Dying is asked otherwise.
pub(super) fn this_way_members(pattern: &ir::EventPattern) -> Option<(ir::Verb, &Selector, bool, Option<crate::Zone>)> {
    if *pattern != ir::event::EventPattern::new(pattern.subjects.clone(), pattern.actions.clone()) { return None; }
    let ([subject], [ir::event::EventAction::Deed(deed)]) = (pattern.subjects.as_slice(), pattern.actions.as_slice()) else { return None };
    if !matches!(deed.verb, ir::Verb::Exile | ir::Verb::Mill | ir::Verb::Discard | ir::Verb::Destroy | ir::Verb::Move | ir::Verb::Reveal) { return None; }
    match deed.args.as_slice() {
        [] if deed.passive => Some((deed.verb, subject, false, None)),
        [(ir::Role::What, ir::event::Arg::Noun(what))] if !deed.passive && is_you(subject) => Some((deed.verb, what, true, None)),
        // a zone of the player who put it there, or any player's of that kind (400.3)
        [(ir::Role::What, ir::event::Arg::Noun(what)), (ir::Role::To, ir::event::Arg::Zone(to))]
            if !deed.passive && is_you(subject) && deed.verb == ir::Verb::Move && !to.other_than && to.zone.slice.is_none()
                && to.zone.owner.as_deref().is_none_or(|o| is_you(o) || o.quant == Quant::Indefinite(Count::Exactly(ir::Amount::Const(1)))) =>
            Some((deed.verb, what, true, Some(match to.zone.zone()? {
                ir::Zone::Hand => crate::Zone::Hand,
                ir::Zone::Graveyard => crate::Zone::Graveyard,
                ir::Zone::Library => crate::Zone::Library,
                ir::Zone::Battlefield => crate::Zone::Battlefield,
                ir::Zone::Exile => crate::Zone::Exile,
                _ => return None,
            }))),
        _ => None,
    }
}

impl Condition {
    pub(super) fn coin(slot: usize, won: bool) -> Self { Self(Asks::Coin {slot, won}) }
    pub(super) fn stored(slot: usize, is: bool) -> Self { Self(Asks::Stored { slot, is }) }
    /// A positive completed-excess comparison names this immutable result quantity
    /// for "that much" in its true branch, not on the opposite or outer path.
    pub(super) fn completed_quantity(&self) -> Option<expr::Amount> {
        match &self.0 {
            Asks::Compare { left: Values::Number(n @ expr::Number::Damage { excess: true, .. }),
                cmp: Cmp::Ge, right: Values::Number(expr::Number::Const(1)) } => Some(expr::Amount::from_number(n.clone())),
            _ => None,
        }
    }
    /// What holds wherever it is asked.
    pub(super) fn always() -> Self { Self(Asks::And(Vec::new())) }

    /// Whether this asks only whether what that slot holds is so: "if it's a land card". Of
    /// nothing, nothing is so, so this is false of an empty group.
    pub(super) fn is_of(&self, slot: usize) -> bool {
        matches!(&self.0, Asks::Is { what: Thing::Named(expr::Subject::Local(of)), negated: false, .. } if *of == slot)
            && self.scalars().iter().all(|s| matches!(s, Scalar::Local { slot: read, .. } if *read == slot))
    }
    pub(super) fn outcome(paid: usize, possible: usize, outcome: ir::condition::Outcome) -> Self {
        Self(Asks::Outcome { paid, possible, outcome })
    }
    /// A proved retained subject group contains an object. Empty successful-result
    /// groups and their projections are false (701.6a, 608.2c).
    pub(super) fn present(slot: usize) -> Self {
        Self(Asks::Is { what: Thing::Named(expr::Subject::Local(slot)),
            is: Is::Adjective(expr::Filter::All(Vec::new())), negated: false, past: false })
    }

    /// The question this condition asks. A reference to a chosen value is bound by `bind`.
    pub(crate) fn compile(c: &ir::Condition, support: &Support<'_>, bind: Bind<'_>) -> Res<Condition> {
        Asks::compile(c, support, bind).map(Condition)
    }

    /// Conditions choosing a printed alternative cost are asked before the spell's
    /// announcement/payment facts are committed (601.2b). Do not admit a predicate
    /// whose values this announcement context does not yet supply.
    pub(crate) fn compile_cast_choice(c: &ir::Condition, support: &Support<'_>, bind: Bind<'_>) -> Res<Condition> {
        if let ir::Condition::NthCastThisGame { what, n, set } = c {
            if *n == 0 || what.quant != Quant::Definite(Ref::This) || what.chooser.is_some() || what.random.is_some()
                || !matches!(what.set.domain, ir::Domain::Permanent | ir::Domain::Spell)
                || what.set != ir::ObjectSet::of(what.set.domain.clone()) || *set != ir::ObjectSet::of(ir::Domain::Spell) {
                return unsupported("game cast ordinal: one unqualified own-spell alternative required");
            }
            return Ok(Condition(Asks::GameCastChoice { n: *n }));
        }

        fn available(asks: &Asks) -> bool {
            match asks {
                Asks::Targets { .. } | Asks::Kicked { .. } | Asks::OptionalPaid { .. } | Asks::CostPaid { .. } | Asks::CastFor { .. } | Asks::Gift { .. } | Asks::CastBy { .. } | Asks::ManaSpent { .. }
                    | Asks::Coin { .. } | Asks::Stored { .. } | Asks::Outcome { .. } | Asks::HadCounters { .. } => false,
                Asks::And(parts) | Asks::Or(parts) => parts.iter().all(available),
                _ => true,
            }
        }
        let condition = Self::compile(c, support, bind)?;
        if !available(&condition.0) { return unsupported("alternative cost condition: announcement/payment facts not yet supplied (601.2b)"); }
        Ok(condition)
    }

    /// Missing positioned history cannot silently turn an optional free route off.
    pub(crate) fn validate_cast_choice_history(&self, game: &Game, ctx: &Ctx) -> Res<()> {
        fn needs(asks: &Asks) -> bool { match asks {
            Asks::GameCastChoice { .. } => true,
            Asks::And(parts) | Asks::Or(parts) => parts.iter().any(needs),
            _ => false,
        } }
        if needs(&self.0) && game.players[ctx.controller].game_casts.is_none() {
            return unsupported("alternative cost requires a known whole-game cast count; positioned history did not supply it");
        }
        Ok(())
    }

    /// In which announcements ([`crate::announcement::ANY`]) can it take each branch: (true,
    /// false)? Only whether it was kicked and whether its gift was promised are known as the
    /// spell is announced; other facts affect resolution, not whether a target is announced
    /// (601.2c, 702.33g, 702.174m).
    pub(super) fn announced_outcomes(&self) -> (u8, u8) {
        use crate::announcement::{ANY, OPTIONAL_PAID, KICKED, PROMISED};
        fn outcomes(asks: &Asks) -> (u8, u8) {
            let said = |held: u8, negated: bool| if negated { (ANY & !held, held) } else { (held, ANY & !held) };
            match asks {
                Asks::Kicked { negated } => said(KICKED, *negated),
                Asks::OptionalPaid { negated } => said(OPTIONAL_PAID, *negated),
                Asks::Gift { negated } => said(PROMISED, *negated),
                Asks::And(parts) => parts.iter().map(outcomes).fold((ANY, 0), |(yes,no),(y,n)| (yes & y, no | n)),
                Asks::Or(parts) => parts.iter().map(outcomes).fold((0, ANY), |(yes,no),(y,n)| (yes | y, no & n)),
                _ => (ANY, ANY),
            }
        }
        outcomes(&self.0)
    }

    /// Does it ask whether its source came under its controller's control since an upkeep
    /// began (702.30a)?
    pub(crate) fn asks_control_history(&self) -> bool {
        fn history(asks: &Asks) -> bool {
            match asks { Asks::CameUnderControlSinceUpkeep => true, Asks::And(v) | Asks::Or(v) => v.iter().any(history), _ => false }
        }
        history(&self.0)
    }

    pub(crate) fn reads_targets(&self) -> bool {
        fn reads(asks: &Asks) -> bool {
            match asks { Asks::Targets { .. } => true, Asks::And(v) | Asks::Or(v) => v.iter().any(reads), _ => false }
        }
        reads(&self.0)
    }

    /// Does it hold, now?
    pub fn evaluate(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals) -> bool {
        self.0.evaluate(game, ctx, locals)
    }

    /// What answering can read, for a proof that something leaves the answer unchanged.
    pub(crate) fn reads(&self) -> expr::Reads { self.0.reads() }

    /// The chosen values the condition reads, for checking they are produced before it.
    pub fn scalars(&self) -> Vec<Scalar> {
        let mut out = Vec::new();
        self.0.scalars(&mut out);
        out
    }
}

impl Asks {
    fn compile(c: &ir::Condition, support: &Support<'_>, bind: Bind<'_>) -> Res<Asks> {
        Ok(match c {
            ir::Condition::ManaSpent { mana, on: ir::mana::SpendOn::Cast(spells), negated } => {
                let [ir::Who::One(spell)] = spells.items.as_slice() else { return unsupported("mana spent: one referenced spell"); };
                Asks::ManaSpent { spell: expr::Subject::compile(spell, &mut *bind)?, test: PaidTest::compile(mana, support, bind)?, negated: *negated }
            }
            ir::Condition::Targets { what, target } => {
                if target.quant != Quant::Indefinite(Count::Exactly(ir::Amount::Const(1))) || target.chooser.is_some() || target.random.is_some() {
                    return unsupported("targeting condition: an existential description of a target");
                }
                Asks::Targets { what: expr::Subject::compile(what, &mut *bind)?, target: expr::Set::compile(&target.set, support, bind)? }
            }
            ir::Condition::Controls { player, what, negated } => {
                // only alternatives run; "an artifact and an enchantment" is read, not run
                if what.conj != ir::Conj::Or && what.items.len() > 1 {
                    return unsupported("condition: control of a conjunction of noun phrases");
                }
                let players = Players::compile(player, support, &mut *bind)?;
                let mut said = Vec::new();
                for s in &what.items {
                    said.push((expr::Set::controlled_root(&s.set, player, support, &mut *bind)?, Quantity::compile(&s.quant, support, &mut *bind)?));
                }
                Asks::Controls { players, what: said, negated: *negated }
            }
            ir::Condition::Exists { what } => {
                Asks::Exists { what: expr::Set::compile_root(&what.set, support, &mut *bind)?, count: Quantity::compile(&what.quant, support, bind)? }
            }
            ir::Condition::CastWas { what, fact: ir::condition::CastFact::Kicked, negated }
                if support.has_kicker() && matches!(what.quant, Quant::Definite(Ref::This | Ref::Anaphor(_))) => Asks::Kicked { negated: *negated },
            // "if this spell was bargained", "if its offspring cost was paid": its optional
            // additional cost, of the spell or of the permanent it became (702.166b, 702.175a)
            ir::Condition::CastWas { what, fact, negated } if matches!(what.quant, Quant::Definite(Ref::This | Ref::Anaphor(_)))
                && support.optional().is_some_and(|(kind, cost)| optional_paid(fact, kind, cost)) => Asks::OptionalPaid { negated: *negated },
            ir::Condition::CastWas { what, fact: ir::condition::CastFact::CostPaid(keyword), negated }
                if matches!(what.quant, Quant::Definite(Ref::This | Ref::Anaphor(_))) && support.alternative_row(*keyword).is_some() =>
                Asks::CostPaid { row: support.alternative_row(*keyword).expect("checked"), negated: *negated },
            ir::Condition::CastWas { what, fact: ir::condition::CastFact::CastFor(keyword), negated } =>
                Asks::CastFor { what: expr::Subject::compile(what, &mut *bind)?, keyword: *keyword, negated: *negated },
            // "if the gift was promised": of the spell whose gift it is (702.174k)
            ir::Condition::CastWas { what, fact: ir::condition::CastFact::GiftPromised, negated }
                if support.has_gift() && matches!(what.quant, Quant::Definite(Ref::This | Ref::Anaphor(_))) => Asks::Gift { negated: *negated },
            // "If the discarded card wasn't a land card": what a card paid for a cost was,
            // asked of that card as it last existed (608.2h) — the object paid is an identity
            // of the occurrence that no later change reaches. Only such a paid object is asked
            // about in the past here; any other past is still refused.
            ir::Condition::Was(before) if matches!(before.as_ref(), ir::Condition::Is { what, .. }
                if matches!(what.quant, Quant::Definite(Ref::Anaphor(ir::selector::Anaphor::TheParticiple { .. })))
                    && super::input::find(support, what).is_ok_and(|i| matches!(i, Some(super::input::Input::Cost(_))))) => {
                let Asks::Is { what, is, negated, .. } = Asks::compile(before, support, bind)? else { unreachable!("an Is condition compiles to one") };
                Asks::Is { what, is, negated, past: true }
            }
            ir::Condition::Was(before) if matches!(before.as_ref(), ir::Condition::Controls { what, .. }
                if what.items.len() == 1 && matches!(what.items[0].quant, Quant::Definite(Ref::This | Ref::Anaphor(_)))) => {
                let ir::Condition::Controls { player, what, negated } = before.as_ref() else { unreachable!("matched") };
                Asks::Controlled { players: Players::compile(player, support, &mut *bind)?, what: expr::Subject::compile(&what.items[0], &mut *bind)?, negated: *negated }
            }
            ir::Condition::Was(before) if matches!(before.as_ref(), ir::Condition::Is { pred: Predicate::A(_), .. }) => {
                let ir::Condition::Is { what, pred: Predicate::A(set), negated } = before.as_ref() else { unreachable!("matched") };
                if set.domain != ir::Domain::Card || set.zone.is_some() || set.controller.is_some()
                    || set.owner.is_some() || set.caster.is_some() || set.other {
                    return unsupported("past condition: only a moved card's characteristics are implemented");
                }
                let filter = expr::Filter::compile(&set.filter, support, bind)?;
                if !filter.printable() { return unsupported("past condition: only a moved card's characteristics are implemented"); }
                let subject = bind.past_subject(what)?;
                bind.singular_read(&subject)?;
                Asks::Is { what: Thing::Named(subject), is: Is::Card(filter), negated: *negated, past: true }
            }
            ir::Condition::Was(before) => {
                let ir::Condition::Is { what, pred: Predicate::Has(ir::Filter::Atom(ir::Atom::Counters { kind, count })), negated } = before.as_ref() else {
                    return unsupported("past condition: only departure counter facts are implemented");
                };
                let Some(ir::Ability::Triggered { trigger: ir::Trigger::Event { pattern, .. }, .. }) = support.ability() else {
                    return unsupported("past counter condition requires a source departure trigger");
                };
                let source = pattern.subjects.len() == 1 && pattern.subjects[0].quant == Quant::Definite(Ref::This);
                let departure = !pattern.actions.is_empty() && pattern.actions.iter().all(|a| {
                    matches!(a, ir::EventAction::PutIntoGraveyard { from: Some(ir::Zone::Battlefield), .. })
                        || a.reading().is_some_and(|(v, _)| matches!(v, ir::Verb::Die | ir::Verb::LeaveBattlefield))
                });
                let own = what.quant == Quant::Definite(Ref::This)
                    || super::input::resolve(support, what).is_ok_and(|i| i == super::input::Input::Subject);
                if !source || !departure || !own {
                    return unsupported("past counter condition requires the departing source's facts");
                }
                // Counter predicates use exact counts, unlike bare noun cardinalities.
                let count = match count { Count::Exactly(n) => Count::Precisely(n.clone()), other => other.clone() };
                Asks::HadCounters { kind: kind.clone(), count: Quantity::compile(&Quant::Indefinite(count), support, bind)?, negated: *negated }
            }
            ir::Condition::Is { what, pred: Predicate::Adjective(filter) | Predicate::Has(filter), negated }
                if matches!(what.quant, Quant::Indefinite(_)) => {
                let mut set = what.set.clone();
                let predicate = if *negated { ir::Filter::Not(Box::new(filter.clone())) } else { filter.clone() };
                set.filter = ir::Filter::All(vec![set.filter, predicate]);
                Asks::Exists { what: expr::Set::compile_root(&set, support, &mut *bind)?, count: Quantity::compile(&what.quant, support, bind)? }
            }
            ir::Condition::CameUnderControl { what, player, window } => {
                let own_upkeep = matches!(support.ability(), Some(ir::Ability::Triggered {
                    trigger: ir::Trigger::Step { step: ir::Step::Upkeep, whose: ir::StepOwner::Player { who, next: false } }, ..
                }) if is_you(who));
                if !own_upkeep || what.quant != Quant::Definite(Ref::This)
                    || what.set != ir::ObjectSet::of(ir::Domain::Permanent) || what.chooser.is_some() || what.random.is_some()
                    || !is_you(player) || !matches!(window, ir::Window::Since { step: ir::Step::Upkeep, whose } if is_you(whose)) {
                    return unsupported("control acquisition: only this source, you and the prior own-upkeep window are implemented");
                }
                Asks::CameUnderControlSinceUpkeep
            }
            ir::Condition::Is { what, pred, negated } => {
                let thing = match &what.quant {
                    Quant::Definite(Ref::This) => Thing::Source,
                    Quant::Definite(Ref::You) => Thing::Controller,
                    Quant::Definite(Ref::AttachedTo(_)) => Thing::Named(expr::Subject::compile(what, &mut *bind)?),
                    // one thing named before: "it", "that creature", "the discarded card"
                    Quant::Definite(Ref::Anaphor(form)) if matches!(form, ir::selector::Anaphor::It | ir::selector::Anaphor::That
                        | ir::selector::Anaphor::The { plural: false } | ir::selector::Anaphor::TheParticiple { plural: false, .. }) =>
                        Thing::Named(expr::Subject::compile(what, &mut *bind)?),
                    q => return unsupported(format!("condition: is, of \"{}\"", name(q))),
                };
                if let Thing::Named(subject) = &thing { bind.singular_read(subject)?; }
                let is = match pred {
                    // what it has is said of it as what it is would be: "has flying", "has
                    // two or fewer loyalty counters on him"
                    Predicate::Adjective(f) | Predicate::Has(f) => Is::Adjective(expr::Filter::compile(f, support, bind)?),
                    Predicate::A(set) if set.domain == ir::Domain::Card && set.zone.is_none() && set.controller.is_none()
                        && set.owner.is_none() && set.caster.is_none() && !set.other => {
                        // what a revealed face-down card is asked is read from the card as
                        // printed (708.12): only what a card says
                        let filter = expr::Filter::compile(&set.filter, support, bind)?;
                        if !filter.printable() { return unsupported("condition: is a card, with more than printed characteristics"); }
                        Is::Card(filter)
                    }
                    Predicate::A(set) => Is::A(expr::Set::compile(set, support, bind)?),
                    Predicate::In(zone) if zone.zones.items.len() == 1 && matches!(zone.zones.items[0], ir::Zone::Battlefield | ir::Zone::Exile) && zone.owner.is_none() && zone.slice.is_none() => Is::Zone(match zone.zones.items[0] { ir::Zone::Exile => crate::Zone::Exile, _ => crate::Zone::Battlefield }),
                    // "~ is paired with another creature": a creature is paired only with
                    // another creature (702.95b), so this is being paired
                    Predicate::PairedWith(other) if another_creature(other) =>
                        Is::Adjective(expr::Filter::compile(&ir::Filter::Atom(ir::Atom::Status(ir::selector::Status::Paired)), support, bind)?),
                    Predicate::Designated(d @ (ir::condition::ObjectDesignation::Solved | ir::condition::ObjectDesignation::Monstrous | ir::condition::ObjectDesignation::Renowned | ir::condition::ObjectDesignation::Prepared | ir::condition::ObjectDesignation::Suspended)) => Is::Designated(*d),
                    other => return unsupported(format!("condition: is {}", name(other))),
                };
                Asks::Is { what: thing, is, negated: *negated, past: false }
            }
            ir::Condition::NthTurn { whose, turns } if is_you(whose) => Asks::NthTurn { turns: turns.clone() },
            ir::Condition::Turn { whose, negated } => {
                let yours = if is_you(whose) {
                    true
                } else if is_an_opponent(whose) {
                    false
                } else {
                    return Ok(Asks::TurnOf { who: expr::Subject::compile(whose, bind)?, negated: *negated });
                };
                Asks::Turn { yours, negated: *negated }
            }
            ir::Condition::Compare { left, cmp, right } => {
                Asks::Compare { left: Values::compile(left, support, &mut *bind)?, cmp: *cmp, right: Values::compile(right, support, bind)? }
            }
            ir::Condition::MostOf { player, measure: ir::condition::Measure::Life, least, tied }
                if player.set == ir::ObjectSet::of(ir::Domain::Player) && player.chooser.is_none() && player.random.is_none() => {
                let predicate = ir::Condition::Is { what: player.clone(),
                    pred: ir::Predicate::Adjective(ir::Filter::Atom(ir::Atom::MostOf {
                        measure: Box::new(ir::condition::Measure::Life), least: *least, tied: *tied })), negated: false };
                return Asks::compile(&predicate, support, bind);
            }
            ir::Condition::MoreThan { player, measure, fewer, margin, than } => {
                let players = Players::compile(player, support, &mut *bind)?;
                let measure = match measure {
                    ir::condition::Measure::Life => Measure::Life,
                    ir::condition::Measure::Controls(set) => Measure::Controls(expr::Set::controlled(set, support, &mut *bind)?),
                    ir::condition::Measure::Has(_) => return unsupported("condition: more or fewer cards had than another player"),
                };
                let margin = margin.as_ref().map(|a| expr::Number::compile(a, support, &mut *bind)).transpose()?;
                let than = match than {
                    ir::condition::Than::Players(who) => Than::Players(Players::compile(who, support, bind)?),
                    ir::condition::Than::Amount(a) => Than::Values(Values::compile(a, support, bind)?),
                };
                Asks::MoreThan { players, measure, fewer: *fewer, margin, than }
            }
            // Of the event a triggered ability watches for, and part of what it watches for
            // ([`crate::trigger`]); the spell that was the n-th cast stays the n-th.
            ir::Condition::NthCastThisTurn { .. } if matches!(support.ability(), Some(ir::Ability::Triggered { .. })) => Asks::And(Vec::new()),
            ir::Condition::Happened { pattern, window: Some(ir::condition::Window::ThisWay), times: None, negated, .. }
                if matches!(pattern.actions.as_slice(), [ir::EventAction::IsDealtDamage { excess: true, by, .. }] if by.is_empty()) => {
                let [subject] = pattern.subjects.as_slice() else { return unsupported("excess result: one recipient description required"); };
                if *pattern != ir::event::EventPattern::new(pattern.subjects.clone(),pattern.actions.clone()) {
                    return unsupported("excess result: unsupported event qualifications");
                }
                let [ir::EventAction::IsDealtDamage { kind, .. }] = pattern.actions.as_slice() else { unreachable!() };
                let amount=ir::Amount::Damage {by:None,to:Some(Box::new(subject.clone())),kind:*kind,excess:true,when:ir::amount::When::ThisWay};
                Asks::Compare {left:Values::Number(expr::Number::compile(&amount,support,bind)?),
                    cmp:if *negated {Cmp::Lt} else {Cmp::Ge},right:Values::Number(expr::Number::Const(1))}
            }
            ir::Condition::Happened { pattern, window: Some(ir::condition::Window::ThisWay), times: None, negated, .. }
                if matches!(pattern.actions.as_slice(), [ir::EventAction::Deed(d)] if d.verb == ir::Verb::Die && !d.passive && d.args.is_empty()) => {
                let [subject] = pattern.subjects.as_slice() else { return unsupported("death result: one subject description required"); };
                if subject.quant != Quant::Indefinite(Count::Exactly(ir::Amount::Const(1))) || subject.chooser.is_some() || subject.random.is_some()
                    || *pattern != ir::event::EventPattern::new(pattern.subjects.clone(), pattern.actions.clone()) {
                    return unsupported("death result: unsupported event qualifications");
                }
                Asks::Compare { left: Values::Number(expr::Number::death_count(&subject.set, support, bind)?),
                    cmp: if *negated { ir::amount::Cmp::Lt } else { ir::amount::Cmp::Ge }, right: Values::Number(expr::Number::Const(1)) }
            }
            // "If a creature card is exiled this way", "if you exiled a land card this way",
            // "if no cards were milled this way": whether the result groups of the
            // instructions this names have such members (608.2c)
            ir::Condition::Happened { pattern, window: Some(ir::condition::Window::ThisWay), times: None, negated, .. }
                if this_way_members(pattern).is_some() => {
                let (verb, what, by_you, into) = this_way_members(pattern).expect("matched");
                let (cmp, n) = match &what.quant {
                    Quant::Indefinite(Count::Exactly(ir::Amount::Const(n)) | Count::AtLeast(ir::Amount::Const(n))) if *n > 0 => (Cmp::Ge, *n),
                    Quant::No => (Cmp::Lt, 1),
                    _ => return unsupported("result group condition: how many, other than one or more or none"),
                };
                if what.chooser.is_some() || what.random.is_some() { return unsupported("result group condition: a chosen or random member"); }
                let cmp = if *negated { match cmp { Cmp::Ge => Cmp::Lt, _ => Cmp::Ge } } else { cmp };
                Asks::Compare { left: Values::Number(expr::Number::members(&what.set, Some(verb), into, by_you, support, bind)?), cmp, right: Values::Number(expr::Number::Const(n)) }
            }
            ir::Condition::Happened { pattern, window: Some(ir::condition::Window::ThisTurn), times: None, negated, .. }
                if summarized(pattern, ir::Verb::Enter).is_some_and(|set| matches!(&set.filter, ir::Filter::All(parts) if matches!(parts.as_slice(), [ir::Filter::Atom(ir::Atom::Type(_))]))) => {
                // Entry summaries retain event-time types and controllers, not mutable objects:
                // "if an artifact entered the battlefield under your control this turn"
                let [subject] = pattern.subjects.as_slice() else { unreachable!() };
                let ir::Filter::All(parts) = &subject.set.filter else { unreachable!() };
                let [ir::Filter::Atom(ir::Atom::Type(kind))] = parts.as_slice() else { unreachable!() };
                let under = summary_under(pattern, support, &mut *bind)?;
                Asks::Entered { kind: *kind, under, count: Quantity::compile(&subject.quant, support, bind)?, negated: *negated }
            }
            ir::Condition::Happened { pattern, window: Some(ir::condition::Window::ThisTurn), times: None, negated, .. }
                if summarized(pattern, ir::Verb::LeaveBattlefield).is_some_and(|set| set.filter == ir::Filter::All(Vec::new())) => {
                // revolt: how many permanents left under each player's control
                let under = summary_under(pattern, support, &mut *bind)?;
                Asks::Departed { under, count: Quantity::compile(&pattern.subjects[0].quant, support, bind)?, negated: *negated }
            }
            // "if you attacked with three or more creatures this turn": the creatures declared as
            // attackers in any combat of the turn, each once (508.1, 508.4)
            ir::Condition::Happened { pattern, window: Some(ir::condition::Window::ThisTurn), times: None, negated, .. }
                if matches!(pattern.actions.as_slice(), [ir::EventAction::Attacks { alone: false, defender, with: Some(_) }] if defender.is_empty()) => {
                let [player] = pattern.subjects.as_slice() else { return unsupported("attack history: one attacking player required"); };
                let [ir::EventAction::Attacks { with: Some(with), .. }] = pattern.actions.as_slice() else { unreachable!() };
                if player.set.domain != ir::Domain::Player || pattern.counted() || pattern.during.is_some() || pattern.condition.is_some() || pattern.cause.is_some()
                    || with.chooser.is_some() || with.random.is_some() {
                    return unsupported("attack history: qualified attack");
                }
                Asks::AttackedWith { players: Players::compile(player, support, &mut *bind)?, those: expr::Set::compile(&with.set, support, &mut *bind)?,
                    count: Quantity::compile(&with.quant, support, bind)?, negated: *negated }
            }
            ir::Condition::Happened { pattern, window: Some(ir::condition::Window::ThisTurn), times: None, negated, .. }
                if matches!(pattern.actions.as_slice(), [ir::EventAction::LosesLife { .. }]) => {
                let [subject] = pattern.subjects.as_slice() else { return unsupported("life-loss history: one player description required"); };
                if pattern.counted() || pattern.during.is_some() || pattern.condition.is_some() || pattern.cause.is_some() {
                    return unsupported("life-loss history: only per-player turn totals are retained");
                }
                let [ir::EventAction::LosesLife { amount }] = pattern.actions.as_slice() else { unreachable!() };
                let count = amount.as_ref().map(|n| Quantity::event(n,support,&mut *bind)).transpose()?
                    .unwrap_or(Quantity::AtLeast(expr::Number::Const(1)));
                Asks::LifeLost { players: Players::compile(subject,support,bind)?, count, negated:*negated }
            }
            ir::Condition::Happened { pattern, window: Some(window @ (ir::condition::Window::ThisTurn | ir::condition::Window::LastTurn)), times, tense: _, negated } => {
                if pattern.counted() { return unsupported("condition: whether the n-th of something happened this turn"); }
                // what happened this turn is all noted ([`crate::history`]); of the turn
                // before, only what is kept a turn
                let turn = match window {
                    ir::condition::Window::ThisTurn => crate::history::Turn::This,
                    _ => {
                        for action in &pattern.actions {
                            match action {
                                ir::event::EventAction::Deed(deed) if crate::history::KEPT_A_TURN.contains(&deed.verb) => {}
                                other => return unsupported(format!("condition: whether {} happened last turn", name(other))),
                            }
                        }
                        crate::history::Turn::Last
                    }
                };
                // "a player cast two or more spells": that many by one player, not among them all
                let each = matches!(pattern.subjects.as_slice(), [who] if who.set.domain == ir::Domain::Player)
                    && pattern.actions.iter().all(|a| matches!(a, ir::event::EventAction::Deed(deed) if !deed.passive));
                // "two or more creatures died", "you've cast three or more spells", "no
                // creatures died": how many is said of what it happened to. Each is one
                // event, and the number is how many such events there were.
                let mut one = pattern.clone();
                let single = Quant::Indefinite(Count::Exactly(ir::Amount::Const(1)));
                let counted = |q: &Quant| matches!(q, Quant::No | Quant::Indefinite(_)) && *q != single;
                let mut said: Vec<&mut Quant> = Vec::new();
                if let [subject] = one.subjects.as_mut_slice() && counted(&subject.quant) { said.push(&mut subject.quant); }
                if let [ir::event::EventAction::Deed(deed)] = one.actions.as_mut_slice() {
                    for (role, arg) in &mut deed.args {
                        if let (ir::Role::What, ir::event::Arg::Noun(noun)) = (&*role, arg) && counted(&noun.quant) { said.push(&mut noun.quant); }
                    }
                }
                let times = match (times, said.as_mut_slice()) {
                    (None, []) => Quantity::AtLeast(expr::Number::compile(&ir::Amount::Const(1), support, &mut *bind)?),
                    (None, [quant]) => {
                        let times = Quantity::compile(quant, support, &mut *bind)?;
                        **quant = single.clone();
                        times
                    }
                    (Some(ir::event::Quantity::AtLeast(n)), []) => Quantity::AtLeast(expr::Number::compile(n, support, &mut *bind)?),
                    (Some(ir::event::Quantity::Exactly(n)), []) => Quantity::Exactly(expr::Number::compile(n, support, &mut *bind)?),
                    _ => return unsupported("condition: how many times, said more than once"),
                };
                Asks::Happened { pattern: crate::trigger::Pattern::compile(&one, support)?, turn, each, times, negated: *negated }
            }
            ir::Condition::Happened { pattern, window: None, times: None, tense: _, negated }
                if matches!(pattern.actions.as_slice(), [ir::EventAction::Deed(d)] if d.verb == ir::Verb::Cast && !d.passive) => {
                let [player] = pattern.subjects.as_slice() else { return unsupported("cast fact: one caster description required"); };
                let [ir::EventAction::Deed(deed)] = pattern.actions.as_slice() else { unreachable!() };
                let [(ir::Role::What, ir::event::Arg::Noun(what))] = deed.args.as_slice() else { return unsupported("cast fact: qualified casting event"); };
                if pattern.counted() || pattern.during.is_some() || pattern.condition.is_some() || pattern.cause.is_some() {
                    return unsupported("cast fact: qualified casting event");
                }
                let own = what.quant == Quant::Definite(Ref::This)
                    || super::input::resolve(support, what).is_ok_and(|i| super::input::own(support, i));
                if !own { return unsupported("cast fact: only the source's casting is retained"); }
                Asks::CastBy { players: Players::compile(player, support, bind)?, negated: *negated }
            }
            ir::Condition::Happened { pattern, window: None, times: None, tense: _, negated } if cast_from(pattern).is_some() => {
                let from = cast_from(pattern).expect("checked above");
                if pattern.counted() || pattern.during.is_some() || pattern.condition.is_some() || pattern.cause.is_some() || from.zone.slice.is_some() {
                    return unsupported("condition: how this spell was cast, said with more than where from");
                }
                // only a spell has been cast, and only its own text asks while it is one
                if !matches!(support.ability(), Some(ir::Ability::Text(_))) { return unsupported("condition: where it was cast from, asked of other than a spell by its own text"); }
                let zones = from.zone.zones.items.iter().map(|zone| match zone {
                    ir::Zone::Hand => Ok(crate::Zone::Hand),
                    ir::Zone::Graveyard => Ok(crate::Zone::Graveyard),
                    ir::Zone::Library => Ok(crate::Zone::Library),
                    ir::Zone::Exile => Ok(crate::Zone::Exile),
                    other => unsupported(format!("condition: cast from {}", name(other))),
                }).collect::<Res<_>>()?;
                let yours = match from.zone.owner.as_deref().map(|owner| &owner.quant) {
                    None | Some(Quant::Indefinite(Count::Exactly(ir::Amount::Const(1)))) => false,
                    Some(Quant::Definite(ir::Ref::You)) => true,
                    Some(_) => return unsupported("condition: cast from another player's zone"),
                };
                Asks::CastFrom { zones, yours, other_than: from.other_than, negated: *negated }
            }
            ir::Condition::And(v) => Asks::And(v.iter().map(|c| Asks::compile(c, support, &mut *bind)).collect::<Res<_>>()?),
            ir::Condition::Or(v) => Asks::Or(v.iter().map(|c| Asks::compile(c, support, &mut *bind)).collect::<Res<_>>()?),
            other => return unsupported(format!("condition: {}", name(other))),
        })
    }

    fn evaluate(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals) -> bool {
        match self {
            Asks::Targets { what, target } => {
                let named = what.evaluate(game, ctx, locals);
                !named.is_empty() && named.iter().all(|t| matches!(t, Target::Object(o)
                    if game.obj(*o).targets.iter().flatten().any(|t| target.contains(game, ctx, locals, *t))))
            }
            Asks::Stored { slot, is } => matches!(locals.local(*slot), Some(crate::binding::Value::Consent(value)) if value == is),
            Asks::Coin { slot, won } => matches!(locals.local(*slot), Some(crate::binding::Value::Coin(flip)) if flip.won() == Some(*won)),
            Asks::Outcome { paid, possible, outcome } => {
                let yes = |slot| matches!(locals.local(slot), Some(crate::binding::Value::Consent(true)));
                match outcome { ir::condition::Outcome::Did => yes(*paid), ir::condition::Outcome::Didnt => !yes(*paid), ir::condition::Outcome::Couldnt => !yes(*possible) }
            }
            Asks::Controls { players, what, negated } => {
                let controls = |p: PlayerId| {
                    let any = what.iter().any(|(set, count)| {
                        let controlled = ctx.permanents(game).iter().filter(|o| ctx.controlled_by(game, **o) == p && set.contains(game, ctx, locals, Target::Object(**o)));
                        count.is(game, ctx, locals, controlled.count())
                    });
                    any != *negated
                };
                players.hold(players.players(game, ctx, locals).into_iter().map(controls))
            }
            Asks::Controlled { players, what, negated } => {
                let objects: Vec<_> = what.evaluate(game, ctx, locals).into_iter().filter_map(|t| match t { Target::Object(o) => Some(o), _ => None }).collect();
                // of what was named and is no more, nobody controlled it
                players.hold(players.players(game, ctx, locals).into_iter()
                    .map(|p| (!objects.is_empty() && objects.iter().all(|o| game.controller(*o) == p)) != *negated))
            }
            Asks::Kicked { negated } => ctx.source.is_some_and(|source| game.obj(source).kicked > 0) != *negated,
            Asks::OptionalPaid { negated } => ctx.source.is_some_and(|source| game.obj(source).optional_paid) != *negated,
            Asks::CostPaid { row, negated } => ctx.source.is_some_and(|source| game.obj(source).alternative == Some(*row)) != *negated,
            Asks::CastFor { what, keyword, negated } => {
                let objects: Vec<_> = what.evaluate(game, ctx, locals).into_iter().filter_map(|t| match t { Target::Object(o) => Some(o), _ => None }).collect();
                (!objects.is_empty() && objects.iter().all(|o| game.cast_for(*o, *keyword))) != *negated
            }
            Asks::Gift { negated } => ctx.source.is_some_and(|source| game.obj(source).gift.is_some()) != *negated,
            Asks::Exists { what, count } => count.is(game, ctx, locals, what.members(game, ctx, locals).len()),
            Asks::HadCounters { kind, count, negated } => {
                let Some(object) = ctx.source else { return false; };
                let source = game.obj(object);
                source.zone == crate::Zone::Gone && source.left == Some(crate::Zone::Battlefield)
                    && (count.is(game, ctx, locals, crate::select::counters_on(game, object, kind.as_ref()) as usize) != *negated)
            }
            Asks::Is { what, is, negated, past } => {
                let things = match what {
                    Thing::Source => ctx.source.into_iter().map(Target::Object).collect(),
                    Thing::Controller => vec![Target::Player(ctx.controller)],
                    Thing::Named(subject) => subject.evaluate(game, ctx, locals),
                };
                let matches = |thing: Target| {
                    // in the past, of the object as it last was: a permanent that has left is
                    // still described as one (`Ctx::was`)
                    let was = match thing { Target::Object(o) if *past => Ctx { was: Some(o), ..ctx.clone() }, _ => ctx.clone() };
                    let ctx = &was;
                    match is {
                        Is::Adjective(f) => f.matches(game, ctx, locals, thing),
                        Is::A(set) => set.contains(game, ctx, locals, thing),
                        Is::Zone(zone) => matches!(thing, Target::Object(o) if game.obj(o).zone == *zone),
                        Is::Designated(d) => matches!(thing, Target::Object(o) if {
                            if *d == ir::condition::ObjectDesignation::Suspended { game.suspended(o) } else {
                            let o = game.obj(o);
                            o.zone == crate::Zone::Battlefield && match d {
                                ir::condition::ObjectDesignation::Solved => o.solved,
                                ir::condition::ObjectDesignation::Monstrous => o.monstrous,
                                ir::condition::ObjectDesignation::Saddled => o.saddled,
                                ir::condition::ObjectDesignation::Prepared => o.prepared,
                                _ => o.renowned,
                            }
                            }
                        }),
                        // of a face-down permanent this effect revealed, the card it represents,
                        // as printed (708.12; Hauntwoods Shrieker ruling 2024-09-20); unrevealed,
                        // it is the face-down 2/2 creature card (708.2)
                        Is::Card(f) => matches!(thing, Target::Object(o) if game.obj(o).card.is_some() && game.obj(o).ability.is_none())
                            && match thing {
                                Target::Object(o) if game.face_down(o) && locals.revealed(o) =>
                                    game.physical_card(o).is_some_and(|(_, card)| f.printed(game, ctx, locals, card)),
                                _ => f.matches(game, ctx, locals, thing),
                            },
                    }
                };
                // of what was named and is no more (an illegal target), nothing is so
                let is = !things.is_empty() && things.into_iter().all(matches);
                is != *negated
            }
            Asks::TurnOf { who, negated } => (who.evaluate(game, ctx, locals) == vec![Target::Player(game.active)]) != *negated,
            Asks::Turn { yours, negated } => ((game.active == ctx.controller) == *yours) != *negated,
            Asks::NthTurn { turns } => game.active == ctx.controller && turns.contains(&game.players[ctx.controller].turns_taken),
            Asks::CameUnderControlSinceUpkeep => ctx.source.is_some_and(|object| game.came_under_control_since_upkeep(object, ctx.controller)),
            Asks::GameCastChoice { n } => {
                if let Some(source) = ctx.source.and_then(|source| game.get_object(source))
                    && let Some(ordinal) = source.cast_ordinal {
                    source.cast_by == Some(ctx.controller) && ordinal == *n
                } else { game.players[ctx.controller].game_casts.and_then(|count| count.checked_add(1)) == Some(*n) }
            },
            // quantify either side in written order
            Asks::Compare { left, cmp, right } => {
                let (left, each_left) = left.values(game, ctx, locals);
                let (right, each_right) = right.values(game, ctx, locals);
                let mut outcomes = left.iter().map(|l| {
                    let mut matches = right.iter().map(|r| match cmp {
                        Cmp::Eq => l == r,
                        Cmp::Le => l <= r,
                        Cmp::Ge => l >= r,
                        Cmp::Lt => l < r,
                        Cmp::Gt => l > r,
                    });
                    if each_right { matches.all(|x| x) } else { matches.any(|x| x) }
                });
                if each_left { outcomes.all(|x| x) } else { outcomes.any(|x| x) }
            }
            Asks::MoreThan { players, measure, fewer, margin, than } => {
                // "more than" is by one or more; "at least two more than" by that margin
                let by = margin.as_ref().map_or(1, |m| m.evaluate(game, ctx, locals));
                let beyond = |mine: i64, theirs: i64| if *fewer { theirs - mine >= by } else { mine - theirs >= by };
                players.hold(players.players(game, ctx, locals).into_iter().map(|p| {
                    let mine = measure.of(game, ctx, locals, p);
                    match than {
                        // a player is not measured against themselves
                        Than::Players(others) => others.hold(others.players(game, ctx, locals).into_iter().filter(|q| *q != p).map(|q| beyond(mine, measure.of(game, ctx, locals, q)))),
                        Than::Values(values) => {
                            let (values, every) = values.values(game, ctx, locals);
                            let mut held = values.into_iter().map(|v| beyond(mine, v));
                            if every { held.all(|x| x) } else { held.any(|x| x) }
                        }
                    }
                }))
            }
            Asks::Entered { kind, under, count, negated } => {
                let test = |player| count.is(game, ctx, locals, game.entered_this_turn(player, *kind));
                under.as_ref().map_or_else(|| test(None), |who| who.hold(who.players(game,ctx,locals).into_iter().map(|p| test(Some(p))))) != *negated
            }
            Asks::Departed { under, count, negated } => {
                let test = |player| count.is(game, ctx, locals, game.departed_this_turn(player));
                under.as_ref().map_or_else(|| test(None), |who| who.hold(who.players(game,ctx,locals).into_iter().map(|p| test(Some(p))))) != *negated
            }
            Asks::AttackedWith { players, those, count, negated } => players.hold(players.players(game, ctx, locals).into_iter().map(|p| {
                let with = game.declared_attackers().iter().filter(|(who, o, seen)| *who == p
                    && those.contains(game, &Ctx { was: Some(*o), then: Some((*o, seen)), ..ctx.clone() }, locals, Target::Object(*o))).count();
                count.is(game, ctx, locals, with)
            })) != *negated,
            Asks::LifeLost { players, count, negated } => players.hold(players.players(game,ctx,locals).into_iter()
                .map(|p|count.is(game,ctx,locals,game.life_lost_this_turn(p) as usize))) != *negated,
            Asks::Happened { pattern, turn, times: Quantity::AtLeast(expr::Number::Const(1)), negated, .. } =>
                game.has_happened(ctx, pattern, *turn) != *negated,
            Asks::Happened { pattern, turn, each, times, negated } => times.is(game, ctx, locals, game.happened(ctx, pattern, *turn, *each)) != *negated,
            Asks::ManaSpent { spell, test, negated } => {
                let spells = spell.evaluate(game, ctx, locals);
                (!spells.is_empty() && spells.iter().all(|t| matches!(t, Target::Object(o)
                    if test.matches(&game.obj(*o).mana_paid, game, ctx, locals)))) != *negated
            }
            Asks::CastBy { players, negated } => {
                let caster = ctx.source.and_then(|source| game.obj(source).cast_by);
                players.hold(players.players(game, ctx, locals).into_iter().map(|p| caster == Some(p))) != *negated
            }
            Asks::CastFrom { zones, yours, other_than, negated } => {
                // a copy of a spell was not cast (707.10), and was cast from nowhere
                let Some(source)=ctx.source else {return *negated;};
                let spell = game.obj(source);
                // Before 601.2a, an announcement context names the card's current
                // zone; after announcement, the spell retains that same origin.
                let origin = if ctx.casting.is_some_and(|(card, _)| card == source) { Some(spell.zone) } else { spell.cast_from };
                let from = origin.is_some_and(|zone| zones.contains(&zone) && (!*yours || spell.owner == ctx.controller));
                (origin.is_some() && from != *other_than) != *negated
            }
            Asks::And(v) => v.iter().all(|c| c.evaluate(game, ctx, locals)),
            Asks::Or(v) => v.iter().any(|c| c.evaluate(game, ctx, locals)),
        }
    }

    fn scalars(&self, out: &mut Vec<Scalar>) {
        match self {
            Asks::Targets { what, target } => { what.scalars(out); out.extend(target.scalars()); }
            Asks::Stored { slot, .. } => out.push(Scalar::Local { slot: *slot, kind: crate::binding::Kind::Consent }),
            Asks::Coin { slot, .. } => out.push(Scalar::Local {slot: *slot, kind: crate::binding::Kind::Coin}),
            Asks::Outcome { paid, possible, .. } => out.extend([*paid, *possible].map(|slot| Scalar::Local { slot, kind: crate::binding::Kind::Consent })),
            Asks::Controls { players, what, .. } => {
                players.scalars(out);
                for (set, count) in what {
                    out.extend(set.scalars());
                    count.scalars(out);
                }
            }
            Asks::Exists { what, count } => {
                out.extend(what.scalars());
                count.scalars(out);
            }
            Asks::Is { what, is, .. } => {
                if let Thing::Named(subject) = what { subject.scalars(out); }
                match is { Is::Adjective(f) | Is::Card(f) => f.scalars(out), Is::A(set) => out.extend(set.scalars()), Is::Designated(_) | Is::Zone(_) => {} }
            }
            Asks::HadCounters { count, .. } => count.scalars(out),
            Asks::Controlled { players, what, .. } => {
                players.scalars(out);
                what.scalars(out);
            }
            Asks::TurnOf { who, .. } => who.scalars(out),
            Asks::Turn { .. } | Asks::NthTurn { .. } | Asks::GameCastChoice { .. } | Asks::CameUnderControlSinceUpkeep | Asks::Kicked { .. } | Asks::OptionalPaid { .. } | Asks::CostPaid { .. } | Asks::Gift { .. } => {}
            Asks::Compare { left, right, .. } => {
                left.scalars(out);
                right.scalars(out);
            }
            Asks::MoreThan { players, measure, margin, than, .. } => {
                players.scalars(out);
                if let Measure::Controls(set) = measure { out.extend(set.scalars()); }
                if let Some(margin) = margin { margin.scalars(out); }
                match than {
                    Than::Players(others) => others.scalars(out),
                    Than::Values(values) => values.scalars(out),
                }
            }
            Asks::Entered { under, count, .. } | Asks::Departed { under, count, .. } => { if let Some(players) = under { players.scalars(out); } count.scalars(out); },
            Asks::LifeLost { players, count, .. } => { players.scalars(out); count.scalars(out); },
            Asks::AttackedWith { players, those, count, .. } => { players.scalars(out); out.extend(those.scalars()); count.scalars(out); },
            Asks::Happened { times, .. } => times.scalars(out),
            Asks::CastFor { what, .. } => what.scalars(out),
            Asks::ManaSpent { spell, test, .. } => {
                spell.scalars(out); if let PaidTest::Count(_, q) = test { q.scalars(out); }
            }
            Asks::CastBy { players, .. } => players.scalars(out),
            Asks::CastFrom { .. } => {}
            Asks::And(v) | Asks::Or(v) => v.iter().for_each(|c| c.scalars(out)),
        }
    }

    /// What answering can read ([`Reads`]). What happened is described by event patterns
    /// this does not look into: it says they may read anything.
    fn reads(&self) -> expr::Reads {
        use expr::Reads;
        let players = |p: &Players| match &p.who { Who::Of(set) => set.reads(), Who::You => Reads::NONE, Who::Reference(_) => Reads::ALL };
        let quantity = |q: &Quantity| match q {
            Quantity::AtLeast(a) | Quantity::AtMost(a) | Quantity::Exactly(a) => a.reads(),
            Quantity::Between(min, max) => min.reads() | max.reads(),
            Quantity::None | Quantity::Any => Reads::NONE,
        };
        let values = |v: &Values| match v {
            Values::Life(who) | Values::StartingLife(who) | Values::Speed(who) => players(who),
            Values::Number(n) => n.reads(),
        };
        match self {
            Asks::TurnOf { .. } => Reads::NONE,
            Asks::Coin { .. } | Asks::Stored { .. } | Asks::Outcome { .. } | Asks::Turn { .. } | Asks::NthTurn { .. } | Asks::Kicked { .. } | Asks::OptionalPaid { .. } | Asks::CostPaid { .. } | Asks::Gift { .. } => Reads::NONE,
            Asks::CastBy { players: who, .. } => Reads::HISTORY | players(who),
            Asks::CastFrom { .. } | Asks::GameCastChoice { .. } | Asks::CameUnderControlSinceUpkeep => Reads::HISTORY,
            Asks::Targets { target, .. } => target.reads(),
            Asks::CastFor { .. } => Reads::NONE,
            Asks::ManaSpent { test, .. } => Reads::HISTORY | match test { PaidTest::Count(_, q) => quantity(q), PaidTest::Symbols(_) => Reads::NONE },
            Asks::Controls { players: who, what, .. } => what.iter().fold(players(who), |all, (set, count)| all | set.reads() | quantity(count)),
            Asks::Exists { what, count } => what.reads() | quantity(count),
            Asks::Is { is: Is::Adjective(f) | Is::Card(f), .. } => f.reads(),
            Asks::Is { is: Is::A(set), .. } => set.reads(),
            Asks::Is { is: Is::Designated(_) | Is::Zone(_), .. } => Reads::NONE,
            Asks::HadCounters { count, .. } => quantity(count),
            Asks::Controlled { players: who, .. } => players(who),
            Asks::Compare { left, right, .. } => values(left) | values(right),
            Asks::MoreThan { players: who, measure, margin, than, .. } => {
                players(who) | match measure { Measure::Controls(set) => set.reads(), Measure::Life => Reads::NONE }
                    | margin.as_ref().map_or(Reads::NONE, expr::Number::reads)
                    | match than { Than::Players(others) => players(others), Than::Values(v) => values(v) }
            }
            Asks::Entered { under, count, .. } | Asks::Departed { under, count, .. } => Reads::HISTORY | quantity(count) | under.as_ref().map_or(Reads::NONE, players),
            Asks::LifeLost { players: who, count, .. } => Reads::HISTORY | players(who) | quantity(count),
            Asks::Happened { .. } | Asks::AttackedWith { .. } => Reads::ALL,
            Asks::And(v) | Asks::Or(v) => v.iter().fold(Reads::NONE, |all, c| all | c.reads()),
        }
    }
}

mod footprint;

#[cfg(test)]
mod tests;

/// Does "was bargained", "the additional cost was paid", "its offspring cost was paid", "a
/// casualty cost was paid", "cast using teamwork", "evidence was collected" or "a Dragon was
/// beheld" ask of this
/// optional additional cost (607.2)? What was beheld had the quality the cost asks for as it
/// was paid (701.4b), so the phrase asks only whether it was.
pub(crate) fn optional_paid(fact: &ir::condition::CastFact, kind: crate::keyword::Optional, cost: &ir::Cost) -> bool {
    use crate::keyword::Optional;
    use ir::condition::CastFact;
    let only = match cost.0.as_slice() { [ir::CostPart::Action(action)] => Some(action), _ => None };
    match fact {
        CastFact::Bargained => kind == Optional::Bargain,
        // "if this spell was cast using teamwork" (702.194b)
        CastFact::Teamwork => kind == Optional::Teamwork,
        CastFact::AdditionalCostPaid => kind == Optional::Printed,
        CastFact::CostPaid(keyword) => matches!((keyword.name(), kind), ("Offspring", Optional::Offspring) | ("Casualty", Optional::Casualty)),
        CastFact::EvidenceCollected => kind == Optional::Printed
            && matches!(only, Some(ir::Action::Numbered(ir::action::NumberedAction::CollectEvidence, _))),
        CastFact::Beheld(set) => kind == Optional::Printed && matches!(only, Some(ir::Action::OnObjects(ir::action::ObjectAction::Behold, objects))
            if objects.items.len() == 1 && objects.items[0].set == *set),
        _ => false,
    }
}

/// "another creature", and no more said of it.
fn another_creature(s: &ir::Selector) -> bool {
    fn creature(f: &ir::Filter) -> bool {
        match f {
            ir::Filter::Atom(ir::Atom::Type(ir::CardType::Creature)) => true,
            ir::Filter::All(parts) => matches!(parts.as_slice(), [one] if creature(one)),
            _ => false,
        }
    }
    s.quant == Quant::Indefinite(Count::Exactly(ir::Amount::Const(1))) && s.chooser.is_none() && s.random.is_none()
        && s.set.other && s.set.domain == ir::Domain::Permanent && s.set.zone.is_none() && s.set.controller.is_none()
        && s.set.owner.is_none() && s.set.caster.is_none() && creature(&s.set.filter)
}
