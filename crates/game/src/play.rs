//! What players do: play a land, tap a land for mana, cast a spell, choose targets, pass. The
//! engine checks each move against the rules and refuses an illegal one with the reason; it
//! never moves on a player's behalf.

use crate::card::{CardDef, CastAs};
use crate::event::{Event, Target};
use crate::announcement::{append_targets, CastRoute, TargetSpec};
use crate::select::{Ctx, name};
use crate::replace::Choices;
use crate::state::{Answer, Choice, Decision, Game, ObjId, PlayerId, Pool, Resolving, Triggered, Zone};
use crate::Res;
use ir::mana::ManaSymbol;
use ir::{Ability, Action, CardType, Effect, Quant, Sentence, Supertype};

/// A refused move changes nothing: the player is asked the same question again. An illegal
/// total combat damage assignment is refused by returning the game to the moment before that
/// player began to assign (`Restarted`, 510.1e). Unsupported execution instead restores the
/// complete action proposal, including its pending choice. A move naming an object or player
/// that is not there is refused before anything is tried.
#[derive(Debug)]
pub enum ActionError { Refused(String), Restarted(String), NoDecision, Unsupported(crate::Unsupported), Unreferable(Target) }
pub(crate) type ActionResult<T> = Result<T,ActionError>;
impl From<&str> for ActionError { fn from(reason:&str)->Self { Self::Refused(reason.into()) } }
impl From<String> for ActionError { fn from(reason:String)->Self { Self::Refused(reason) } }
impl From<crate::Unsupported> for ActionError { fn from(reason:crate::Unsupported)->Self { Self::Unsupported(reason) } }
impl ActionError {
    /// What kind of refusal this is: a rules refusal, a restarted assignment among them
    /// (510.1e), is illegal.
    pub fn class(&self) -> crate::simulation::Class {
        use crate::simulation::Class;
        match self { Self::Refused(_) | Self::Restarted(_) => Class::Illegal, Self::Unsupported(_) => Class::Unsupported, Self::Unreferable(_) => Class::Unreferable, Self::NoDecision => Class::Stale }
    }
}
impl std::fmt::Display for ActionError {
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result {
        match self {
            Self::Refused(reason)|Self::Restarted(reason)=>reason.fmt(f), Self::NoDecision=>f.write_str("no decision is pending"),Self::Unsupported(reason)=>reason.0.fmt(f),
            Self::Unreferable(Target::Object(_))=>f.write_str("an object handle is stale or unknown"),
            Self::Unreferable(Target::Player(player))=>write!(f,"the game has no player {player}"),
        }
    }
}

/// Explicit resources substituting for mana; object identities remain ordinary cost groups.
#[derive(Default)]
struct Substitutions {
    delve: u32,
    convoke: Vec<Option<ir::Color>>,
    improvise: u32,
}
impl Substitutions {
    fn is_empty(&self) -> bool { self.delve == 0 && self.convoke.is_empty() && self.improvise == 0 }
}

/// What is said with a move: the mana that pays, the mana that is added, the modes chosen.
struct Said {
    with: Option<Pool>,
    adding: Vec<Vec<ManaSymbol>>,
    modes: Vec<usize>,
    cast_as: Option<CastAs>,
    plan: Option<Vec<crate::legal::Step>>,
    paying: Option<Vec<Vec<ObjId>>>,
    x: Option<u32>,
    kicks: u32,
    entwine: bool,
    entwine_announced: bool,
    buyback: bool,
    buyback_announced: bool,
    /// Its optional additional cost is announced paid (601.2b, 118.8b): bargain's sacrifice
    /// (702.166b), offspring's or casualty's cost, or the one its text prints.
    optional: bool,
    optional_announced: bool,
    replicate: u32,
    life: Vec<bool>,
    /// For each choice among alternatives of its cost, the alternative paid (601.2b).
    chose: Vec<usize>,
    branches: Option<Vec<crate::cost::BranchSelection>>,
    cost: Option<crate::announcement::CastCost>,
    route: Option<CastRoute>,
    substitutions: Substitutions,
    gift: Option<PlayerId>,
    /// The division announced (601.2d); empty when none is said.
    division: Vec<Vec<u32>>,
    what: Move,
}
impl Said {
    fn validate_optional_costs(&self, game: &Game) -> Result<(), String> {
        if self.entwine_announced {
            let Move::Cast { card, .. } = self.what else { unreachable!("Said rejects non-cast entwine") };
            let def = game.def(card).ok_or("not a card")?.spell(self.cast_as.unwrap_or_default())?;
            if crate::keyword::entwine(def).is_none() { return Err("that spell has no entwine cost (702.42)".into()); }
        }
        if self.buyback_announced {
            let Move::Cast { card, .. } = self.what else { unreachable!("Said rejects non-cast buyback") };
            let def = game.def(card).ok_or("not a card")?.spell(self.cast_as.unwrap_or_default())?;
            if crate::keyword::buyback(def).is_none() { return Err("that spell has no buyback cost (702.27a)".into()); }
        }
        if self.optional_announced {
            let Move::Cast { card, .. } = self.what else { unreachable!("Said rejects a non-cast optional cost") };
            let def = game.def(card).ok_or("not a card")?.spell(self.cast_as.unwrap_or_default())?;
            if def.costs.optional.is_empty() { return Err("that spell has no optional additional cost (118.8b)".into()); }
        }
        Ok(())
    }

    fn read(mv: Move) -> Result<Self, String> {
        let (mut with, mut adding, mut modes, mut what) = (None, Vec::new(), Vec::new(), mv);
        let mut cast_as = None;
        let mut plan = None;
        let mut paying = None;
        let mut x = None;
        let mut kicks = None;
        let mut buyback = None;
        let mut optional = None;
        let mut entwine = None;
        let mut replicate = None;
        let mut life = None;
        let mut chose = None;
        let mut branches = None;
        let mut cost = None;
        let mut route = None;
        let mut delve = None;
        let mut convoke = None;
        let mut improvise = None;
        let mut gift = None;
        let mut division = None;
        loop {
            what = match what {
                Move::WithDivision(said, inner) => {
                    if division.replace(said).is_some() { return Err("announce the division once (601.2d)".into()); }
                    *inner
                }
                Move::WithConvoke(allocation, inner) => {
                    if convoke.replace(allocation).is_some() { return Err("say the convoke allocation once (702.51)".into()); }
                    *inner
                }
                Move::WithImprovise(count, inner) => {
                    if improvise.replace(count).is_some() { return Err("say the improvise payment once (702.126)".into()); }
                    *inner
                }
                Move::WithDelve(count, inner) => {
                    if delve.replace(count).is_some() { return Err("say the delve payment once (702.66)".into()); }
                    *inner
                }
                Move::CastVia(chosen, inner) => {
                    if route.replace(chosen).is_some() { return Err("choose one casting route (601.3)".into()); }
                    *inner
                }
                Move::CastFor(chosen, inner) => {
                    if cost.replace(chosen).is_some() { return Err("choose the cost a spell is cast for only once (601.2b)".into()); }
                    *inner
                }
                Move::GiftTo(opponent, inner) => {
                    if gift.replace(opponent).is_some() { return Err("promise a spell's gift to one opponent, once (702.174a)".into()); }
                    *inner
                }
                Move::WithReplicate(times, inner) => {
                    if replicate.replace(times).is_some() { return Err("announce replicate payments once (601.2b)".into()); }
                    *inner
                }
                Move::WithEntwine(paid, inner) => {
                    if entwine.replace(paid).is_some() { return Err("announce entwine only once (601.2b)".into()); }
                    *inner
                }
                Move::WithBuyback(paid, inner) => {
                    if buyback.replace(paid).is_some() { return Err("announce buyback only once (601.2b)".into()); }
                    *inner
                }
                Move::WithOptional(paid, inner) => {
                    if optional.replace(paid).is_some() { return Err("announce an optional additional cost only once (601.2b)".into()); }
                    *inner
                }
                Move::WithKicks(times, inner) => {
                    if kicks.replace(times).is_some() { return Err("say how many times the kicker cost is paid only once (601.2b)".into()); }
                    *inner
                }
                Move::WithLife(said, inner) => {
                    if life.replace(said).is_some() { return Err("say how Phyrexian mana is paid only once (601.2b)".into()); }
                    *inner
                }
                Move::WithCostBranches(said, inner) => {
                    if branches.replace(said).is_some() { return Err("say semantic cost branches only once".into()); } *inner
                }
                Move::WithCostChoice(said, inner) => {
                    if chose.replace(said).is_some() { return Err("say which alternatives of the cost are paid only once (601.2b)".into()); }
                    *inner
                }
                Move::WithX(value, inner) => {
                    if x.replace(value).is_some() { return Err("announce the value of X only once (107.3a)".into()); }
                    *inner
                }
                Move::WithCosts(chosen, inner) => {
                    if paying.replace(chosen).is_some() { return Err("give additional cost choices only once".into()); }
                    *inner
                }
                Move::WithMana(steps, inner) => {
                    if plan.replace(steps).is_some() { return Err("give the casting mana plan only once".into()); }
                    *inner
                }
                Move::Paying(mana, inner) => {
                    with = Some(mana);
                    *inner
                }
                Move::Adding(mana, inner) => {
                    adding = mana;
                    *inner
                }
                Move::CastAs(chosen, inner) => {
                    if cast_as.replace(chosen).is_some() { return Err("choose a split half only once".into()); }
                    *inner
                }
                Move::Modes(chosen, inner) => {
                    modes = chosen;
                    *inner
                }
                other => break what = other,
            };
        }
        if branches.is_some() && !matches!(what,Move::Cast {..}|Move::Activate {..}) {return Err("semantic cost branches require a cast or activation".into());}
        if entwine.is_some() && !matches!(what, Move::Cast { .. }) { return Err("entwine is announced only with a cast (702.42)".into()); }
        if optional.is_some() && !matches!(what, Move::Cast { .. }) {
            return Err("an optional additional cost is announced only with a cast (601.2b)".into());
        }
        if buyback.is_some() && !matches!(what, Move::Cast { .. }) {
            return Err("buyback is announced only with a cast (702.27a)".into());
        }
        if improvise.is_some() && !matches!(what, Move::Cast { .. }) { return Err("improvise is a casting payment (702.126)".into()); }
        Ok(Self { with, adding, modes, cast_as, plan, paying, x, kicks: kicks.unwrap_or(0), buyback: buyback.unwrap_or(false), buyback_announced: buyback.is_some(), optional: optional.unwrap_or(false), optional_announced: optional.is_some(), entwine: entwine.unwrap_or(false), entwine_announced: entwine.is_some(), replicate: replicate.unwrap_or(0), life: life.unwrap_or_default(), chose: chose.unwrap_or_default(), branches, cost, route, substitutions: Substitutions { delve: delve.unwrap_or(0), convoke: convoke.unwrap_or_default(), improvise: improvise.unwrap_or(0) }, gift, division: division.unwrap_or_default(), what })
    }
}

/// The most times a kicker cost is declared paid in one cast: a cost with more parts than
/// this is one no game pays.
const MOST_KICKS: u32 = 10_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Move {
    /// Concede the game (104.3a): any player, at any time, whatever is pending.
    Concede,
    /// Pay {3} to take the chosen outside-game companion (116.2g).
    Companion,
    /// Exile a hand card and pay its plot cost as a special action (702.170).
    Plot(ObjId),
    /// Pay {2} and exile a hand card with foretell face down, a special action (702.143a).
    Foretell(ObjId),
    /// Exile a hand card with time counters by paying its suspend cost (116.2f, 702.62a).
    Suspend(ObjId),
    /// Turn a face-down permanent face up this way, a special action (116.2b, 701.40b).
    TurnFaceUp(ObjId, crate::face::Way),
    /// Pay the unlock cost of this half of a Room, 0 its left, a special action (116.2m,
    /// 709.5e).
    Unlock { room: ObjId, half: usize },
    /// Reverse a failed unpaid announcement, retaining its legal mana abilities (733.1).
    ReversePayment,
    /// Supply a corrected payment after a nested decision invalidated the saved plan.
    FinishPayment { plan: Vec<crate::legal::Step>, paying: Vec<Vec<ObjId>>, with: Option<Pool> },
    /// Play a land from the hand (305.1).
    PlayLand(ObjId),
    /// Play the chosen modal land face (712.12).
    PlayLandAs { card: ObjId, face: usize },
    /// Activate an ability of a permanent (602.2): which of its abilities, one group of targets
    /// per "target" of its text, and one group of objects for each part of its cost that asks
    /// for a choice ("Sacrifice a creature"), in the order the text says them.
    Activate { source: ObjId, ability: usize, targets: Vec<Vec<Target>>, paying: Vec<Vec<ObjId>> },
    /// Cast a spell from the hand (601.2), with one group of targets per "target" of its text,
    /// in the order the text says them. Its cost is paid from the pool.
    Cast { card: ObjId, targets: Vec<Vec<Target>> },
    /// Answer a pending [`Decision::Targets`]: one group per "target" of the ability's text.
    ChooseTargets(Vec<Vec<Target>>),
    /// Answer only the modes of a triggered ability; its targets are asked next (603.3c–d).
    ChooseModes(Vec<usize>),
    /// Answer a pending [`Decision::Order`]: the abilities, by their place in the decision's
    /// list, in the order they are put on the stack (603.3b). The last is on top and
    /// resolves first.
    Order(Vec<usize>),
    /// Answer a pending [`Decision::Discard`]: the cards to discard (514.1).
    Discard(Vec<ObjId>),
    /// Answer a pending [`Decision::Mulligan`]: take a mulligan (103.5).
    Mulligan,
    /// Answer a pending [`Decision::Mulligan`]: keep the hand, which becomes the opening hand.
    Keep,
    /// Answer a pending [`Decision::Bottom`]: the cards of the hand to put on the bottom of
    /// the library, the last of them the bottom card (103.5).
    Bottom(Vec<ObjId>),
    /// Answer a pending [`Decision::Attackers`]: each attacking creature and the player or
    /// planeswalker it attacks (508.1); none is a declaration too.
    Attack(Vec<(ObjId, Target)>),
    /// Answer a pending [`Decision::Blockers`]: each blocking creature and the attacking
    /// creature it blocks (509.1).
    Block(Vec<(ObjId, ObjId)>),
    /// Answer a pending [`Decision::Divide`]: how much of the blocked creature's combat damage
    /// each creature blocking it is assigned (510.1c), and with trample what it is attacking
    /// (702.19b).
    Divide(Vec<(Target, u32)>),
    /// Answer a pending [`Decision::Choose`]: a choice the effect being applied offers
    /// (608.2d).
    Choose(Answer),
    /// Cast a spell or activate an ability, paying the mana of its cost with this mana of
    /// the pool: generic mana in a cost can be paid with any type of mana (107.4b), which the
    /// player chooses as they pay (601.2h); restricted mana of the pool is said as it is
    /// there, and only where it may be spent (106.6). An absent selection leaves
    /// a nonempty mana cost unpaid; the caller must supply its spending.
    Paying(Pool, Box<Move>),
    /// Trusted host opt-in: retain payment scope evidence through a real interruption.
    /// The audit log itself is discarded after this action.
    AuditPayment(Box<Move>),
    /// Supply the already-authored selector for exactly one retained spell payment
    /// with the actual triggered-mana answer (601.2g-h, 605.4a).
    ContinueManaPayment { source: ObjId, mana: Pool, adding: Vec<ManaSymbol> },
    /// Activate these mana abilities after announcing a spell or ability and choosing targets (601.2g, 602.2b).
    WithMana(Vec<crate::legal::Step>, Box<Move>),
    /// Objects chosen to pay mandatory additional spell costs, one group per choice (601.2h).
    WithCosts(Vec<Vec<ObjId>>, Box<Move>),
    /// Cast a spell or activate an ability whose cost has {X}, announcing this value of X
    /// (107.3a, 601.2b). Any value may be announced; the cost it makes must then be paid.
    WithX(u32, Box<Move>),
    /// Declare how often the spell's kicker cost is paid: at most once for kicker,
    /// repeatedly for multikicker (601.2b, 702.33a,c). The total cost must then be paid.
    WithKicks(u32, Box<Move>),
    /// Pay the spell's optional fixed mana buyback cost (702.27a).
    WithBuyback(bool, Box<Move>),
    /// Whether the spell's optional additional cost is paid, announced as it is cast (118.8b,
    /// 601.2b): its bargain (702.166a), offspring (702.175a) or casualty (702.153a) cost, or the
    /// one its text prints.
    WithOptional(bool, Box<Move>),
    /// Pay entwine and choose every mode instead of the printed count (702.42).
    WithEntwine(bool, Box<Move>),
    /// Number of payments of its replicate cost (702.56a).
    WithReplicate(u32, Box<Move>),
    /// Cast a spell for this cost: its mana cost, or an alternative cost its card prints
    /// (601.2b, 118.9). Without this, its mana cost (or, from a graveyard, its flashback cost).
    CastFor(crate::announcement::CastCost, Box<Move>),
    /// Choose the permission used to cast: ordinary, flashback, retrace or escape (601.3).
    /// An omitted route retains the ordinary/default graveyard casting behavior.
    CastVia(CastRoute, Box<Move>),
    /// Exile this many graveyard cards instead of paying that much generic mana (702.66).
    /// The cards are the final ordinary selected-cost group.
    WithDelve(u32, Box<Move>),
    /// Colored/generic requirements paid by convoking creatures, whose identities are in cost selections.
    WithConvoke(Vec<Option<ir::Color>>, Box<Move>),
    /// Tap this many controlled untapped artifacts for generic mana; identities are one final cost group.
    WithImprovise(u32, Box<Move>),
    /// Cast a spell with gift promising its gift to this opponent: the additional cost "you
    /// may choose an opponent" (601.2b, 702.174a). Without this, its gift is not promised.
    GiftTo(PlayerId, Box<Move>),
    /// Cast a spell or activate an ability whose cost has Phyrexian mana symbols, saying for
    /// each of them in order whether 2 life is paid for it rather than mana of its color
    /// (107.4f, 118.13a, 601.2b). Without this, mana is paid for all of them.
    WithLife(Vec<bool>, Box<Move>),
    /// Cast a spell or activate an ability whose cost is a choice among alternatives
    /// ("discard a card or pay 3 life"), saying for each such choice in order which
    /// alternative is paid, numbered from 0 as its listing offers them (601.2b, 602.2b).
    WithCostChoice(Vec<usize>, Box<Move>),
    WithCostBranches(Vec<crate::cost::BranchSelection>, Box<Move>),
    /// Activate a mana ability whose text offers a choice of mana ("{T}: Add {R} or {G}.",
    /// "Add one mana of any color."), adding this: one answer for each such choice, in the
    /// order written. The ability resolves as it is activated (605.3b), so the choice is
    /// made with the activation.
    Adding(Vec<Vec<ManaSymbol>>, Box<Move>),
    /// Cast a modal spell, activate a modal ability, or put a modal triggered ability on the
    /// stack ([`Decision::Modes`], answered with its targets; [`Move::ChooseModes`] splits
    /// those choices) with these modes chosen, by their
    /// place among the modes printed (601.2b, 700.2a–b). The targets are those of the modes
    /// chosen (700.2c).
    Modes(Vec<usize>, Box<Move>),
    /// Choose a split half or fuse before putting the spell on the stack (709.3, 702.102a).
    CastAs(CastAs, Box<Move>),
    /// Cast a spell, activate an ability or put a triggered ability on the stack (answering
    /// its targets) that divides or distributes an amount among its targets, announcing the
    /// division (601.2d, 602.2b, 603.3d): for each target group, what each of its targets
    /// gets, in the order they are chosen; a group that divides nothing gets none. Each target
    /// gets at least one. With one target of a divided group, it gets all of it unsaid.
    WithDivision(Vec<Vec<u32>>, Box<Move>),
    /// Pass priority (117.3d).
    Pass,
}


/// Can the engine run this card? An instant or sorcery when it can follow all of its text; a
/// permanent when each of its abilities is an activated ability whose cost it can have paid
/// and whose effect it can follow, a triggered ability whose trigger it watches for and whose
/// instructions it can follow, a keyword ability it gives its meaning, or a static ability
/// that is a restriction or requirement it consults.
pub fn check(def: &CardDef) -> Res<()> { def.admission.clone() }

/// Definition-time admission: all executable bodies, registrations and cost plans have
/// already been lowered. No synthetic Game, targets or sample board decide support. Of a
/// definition whose faces are joined (card.rs), the whole card's.
pub(crate) fn validate_definition(def: &CardDef) -> Res<()> { admit(def, &mut Refusals::first()) }

/// Everything that keeps the engine from running this card, in the order [`check`] meets it:
/// the first is the refusal `check` gives, as both are [`admit`]. Rather than stop there,
/// admission goes on to each remaining check of the card, its faces, halves, Adventure and
/// prepare spell, and keeps each distinct refusal. A check that needs what a refused one would
/// have given (an ability's body, after its program is refused) is skipped; that, and what a
/// refused lowering did not reach, the refusal says it left unanalyzed, and only those. Empty
/// when the card is admitted. For reports of what a card
/// lacks; play consults [`check`].
pub fn blockers(def: &CardDef) -> Vec<Blocker> {
    let mut refusals = Refusals { all: Some(Vec::new()), part: String::new() };
    let _ = admit(def, &mut refusals);
    refusals.all.unwrap_or_default()
}

/// One thing that keeps a card from being admitted, for reports of what it lacks.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Blocker {
    pub stage: Stage,
    /// The refusal, as admission gives it.
    pub reason: String,
    /// Of a text row the grammar does not read: which, and how far it read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row: Option<UnreadRow>,
    /// What this refusal kept from being analyzed, which may hide further blockers: the rest
    /// of the row the grammar stopped in; what of an ability its lowering, stopping at this
    /// refusal, did not reach (instructions, cost parts, a condition); a check that needed what
    /// was refused; or the checks of an ability the engine does not know. Empty: it hides
    /// nothing, as a refusal of one keyword, or of an ability's last instruction.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unanalyzed: Vec<String>,
}

/// Where admission met a refusal: reading the face (the grammar, [`CardDef::unread_face`]) or
/// checking what it read ([`blockers`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage { Parse, Engine }

/// A printed text row of a face (after normalization: "~" for its name), as the grammar reads
/// it: an ability's own row, or a mode row under one.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct UnreadRow {
    /// Its position among the face's normalized rows, from 0.
    pub index: usize,
    pub mode: bool,
    pub text: String,
    /// Where the grammar stopped, of a row it does not read; none of one it reads two ways.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Stop>,
}

/// How far the grammar read a row: the longest prefix any rule read, the token it failed at,
/// what follows and what it expected there.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Stop { pub read: String, pub at: String, pub rest: String, pub expected: Vec<String> }

impl Blocker {
    /// A refusal of reading a face that is of no text row.
    pub fn parse(reason: impl Into<String>) -> Self {
        Blocker { stage: Stage::Parse, reason: reason.into(), row: None, unanalyzed: Vec::new() }
    }

    /// What reports count it by: the refusal, and of a row the grammar stops in, the token it
    /// stopped at (as `grammar-gate` ranks its declines).
    pub fn label(&self) -> String {
        match self.row.as_ref().and_then(|row| row.stop.as_ref()) {
            Some(stop) => format!("{}: at {:?}", self.reason, stop.at),
            None => self.reason.clone(),
        }
    }
}

/// A card's whole admission, as its faces are joined (card.rs): the front face, or the
/// normal characteristics, then its back face, its alternative spell or its prepare spell.
fn admit(def: &CardDef, r: &mut Refusals) -> Res<()> {
    validate_into(def, r)?;
    if let Some(back) = &def.back {
        validate_into(back, r)?;
        if !def.modal && (back.is(CardType::Instant) || back.is(CardType::Sorcery)) {
            r.say("a double-faced card with an instant or sorcery back face (712.10)")?;
        }
    }
    if let Some(alternative) = def.alternative_spell() { check_face_as(alternative, true, r)?; }
    if let Some(spell) = def.prepare_spell() { validate_into(spell, r)?; }
    match &def.overloaded {
        None => {}
        Some(Err(e)) => r.say(format!("overload: its text with \"each\" for \"target\" is not read: {e}"))?,
        Some(Ok(overloaded)) => {
            validate_into(overloaded, r)?;
            // the changed text has no target for the spell to announce (702.96b)
            if crate::program::spell_bodies(overloaded).is_ok_and(|bodies| bodies.iter().any(|(_, body)| match body {
                crate::program::Body::Plain(p) => !p.targets().is_empty(),
                crate::program::Body::Modes { programs, .. } => programs.iter().any(|p| !p.targets().is_empty()),
            })) {
                r.say("overload: its text with \"each\" for \"target\" still targets (702.96b)")?;
            }
        }
    }
    Ok(())
}

/// Where admission's refusals go: normally the first ends the check, as `?` would; collecting
/// ([`blockers`]), each is kept and the check goes on. `part`, collecting: the part of the
/// card being checked, which a refusal of what was lowered for it leaves unanalyzed.
struct Refusals { all: Option<Vec<Blocker>>, part: String }

impl Refusals {
    fn first() -> Self { Refusals { all: None, part: String::new() } }

    /// Collecting, the part of the card checked next (its name made only then).
    fn part(&mut self, what: impl FnOnce() -> String) { if self.all.is_some() { self.part = what(); } }

    /// A refusal: the check's result, or, collecting, kept (once per reason, with all it
    /// left unanalyzed) and the check continued.
    fn refuse(&mut self, why: crate::Unsupported, unanalyzed: Option<String>) -> Res<()> {
        let Some(all) = &mut self.all else { return Err(why) };
        let at = match all.iter().position(|b| b.reason == why.0) {
            Some(at) => at,
            None => { all.push(Blocker { stage: Stage::Engine, reason: why.0, row: None, unanalyzed: Vec::new() }); all.len() - 1 }
        };
        if let Some(what) = unanalyzed && !all[at].unanalyzed.contains(&what) { all[at].unanalyzed.push(what); }
        Ok(())
    }

    /// Collecting, more that the refusal kept for `reason` left unanalyzed.
    fn note(&mut self, reason: &str, what: String) {
        if let Some(b) = self.all.iter_mut().flatten().find(|b| b.reason == reason) && !b.unanalyzed.contains(&what) { b.unanalyzed.push(what); }
    }

    /// A refusal of a characteristic or combination, which hides nothing.
    fn say(&mut self, what: impl Into<String>) -> Res<()> { self.refuse(crate::Unsupported(what.into()), None) }

    /// A refusal that ends the checks of the current part.
    fn stop(&mut self, what: impl Into<String>) -> Res<()> {
        let rest = self.all.is_some().then(|| format!("the rest of {}: its checks are skipped", self.part));
        self.refuse(crate::Unsupported(what.into()), rest)
    }

    /// What a check of one thing gives; `None` when it refused and the refusal was kept, which
    /// hides nothing.
    fn need<T>(&mut self, result: Res<T>) -> Res<Option<T>> { self.lowered(result, || None) }

    /// What a check of what was lowered for the current part gives; `None` when it refused
    /// and the refusal was kept, with what of the part the lowering, stopping at its first
    /// refusal, may not have reached (`rest`; none when it reached all).
    fn lowered<T>(&mut self, result: Res<T>, rest: impl FnOnce() -> Option<String>) -> Res<Option<T>> {
        match result {
            Ok(value) => Ok(Some(value)),
            Err(why) => {
                let rest = if self.all.is_some() { rest().map(|rest| format!("{}, {rest}", self.part)) } else { None };
                self.refuse(why, rest).map(|()| None)
            }
        }
    }

    /// The executable body of the ability at `index`, with the instructions a refused one's
    /// lowering never reached.
    fn body<'d>(&mut self, def: &'d CardDef, index: usize) -> Res<Option<&'d crate::program::Body>> {
        self.lowered(def.executable_body(index), || match def.unreached_instructions(index) {
            0 => None,
            1 => Some("1 instruction its lowering did not reach: it stops at its first refusal".into()),
            n => Some(format!("{n} instructions its lowering did not reach: it stops at its first refusal")),
        })
    }

    /// A check of what states `n` things, lowered in turn: of one refused, any of them after
    /// it was not reached.
    fn several<T>(&mut self, result: Res<T>, n: usize, things: &str) -> Res<Option<T>> {
        self.lowered(result, || (n > 1).then(|| format!("of its {n} {things}, any after this refusal: its lowering stops at its first refusal")))
    }

    /// The trigger condition of the ability at `index`: of one refused, its intervening "if"
    /// condition (603.4), read after its trigger event, may not have been reached.
    fn trigger(&mut self, def: &CardDef, index: usize, result: Res<()>) -> Res<Option<()>> {
        let intervening = ability_at(def, index).and_then(crate::trigger::intervening_if).is_some();
        self.lowered(result, || intervening.then(|| "its intervening \"if\" condition, if after this refusal: its lowering stops at its first refusal".into()))
    }
}

/// Each keyword ability a row lists is one ability of its own (113.2c), checked on its own.
fn keywords(def: &CardDef, row: usize, r: &mut Refusals) -> Res<()> {
    for check in crate::keyword::row_checks(def, row) { r.need(check)?; }
    Ok(())
}

fn ability_at(def: &CardDef, index: usize) -> Option<&Ability> {
    def.abilities.iter().chain(def.keyword_abilities.iter().map(|(_, a)| a)).nth(index)
}

/// How many instructions the ability at `index` states: what lowering one of its statements,
/// which stops at its first refusal, may not have reached past a refusal.
fn stated(def: &CardDef, index: usize) -> usize {
    match ability_at(def, index) {
        Some(Ability::Text(effect)) => crate::program::instructions(&effect.0),
        _ => 1,
    }
}

/// How many parts the cost of the activated ability at `index` has (602.1a).
fn cost_parts(def: &CardDef, index: usize) -> usize {
    match ability_at(def, index) { Some(Ability::Activated { cost, .. }) => cost.0.len(), _ => 1 }
}

/// An ability of `def` by its printed row, as a report names it.
fn ability_part(def: &CardDef, index: usize) -> String {
    format!("{}, ability {}", def.name(), def.ability_row(index) + 1)
}

fn validate_into(def: &CardDef, r: &mut Refusals) -> Res<()> {
    if def.halves.is_empty() {
        if def.fuse() { r.say("fuse requires a whole split card (702.102a)")?; }
        return check_face_as(def, false, r);
    }
    for half in &def.halves {
        // a permanent split card is a Room, whose halves are locked and unlocked (709.5)
        if !half.is(CardType::Instant) && !half.is(CardType::Sorcery) && !def.is_room() {
            r.say("permanent split cards: locked and unlocked halves (709.5)")?;
        }
        check_face_as(half, false, r)?;
    }
    if def.fuse() { check_face_as(def, false, r)?; }
    Ok(())
}

pub(crate) fn check_alternative_spell(def: &CardDef) -> Res<()> { check_face_as(def, true, &mut Refusals::first()) }

fn check_face_as(def: &CardDef, alternative: bool, r: &mut Refusals) -> Res<()> {
    if !alternative && def.subtypes.contains(&ir::Subtype::omen()) {
        r.say("Omen characteristics require shuffle-on-resolution (720.3d)")?;
    }
    r.part(|| format!("{}, what it enchants", def.name()));
    r.need(def.enchant.clone())?;
    r.part(|| format!("{}, its plot cost", def.name()));
    if let Some(plot) = &def.plot { r.need(plot.clone())?; }
    r.part(|| format!("{}, its suspend action", def.name()));
    if let Some(suspend) = &def.suspend { r.need(suspend.clone())?; }
    if crate::keyword::replicate_count(def) > 1 { r.say("multiple replicate costs require separately identified announcements (702.56b)")?; }
    r.part(|| format!("{}, its companion condition", def.name()));
    if let Some(companion) = &def.companion { r.need(companion.clone())?; }
    if crate::keyword::entwine_count(def) > 1 { r.say("multiple entwine costs require separately identified announcements (702.42)")?; }
    r.part(|| format!("{}, its spell's modes", def.name()));
    if crate::keyword::entwine(def).is_some() && r.lowered(crate::program::spell_bodies(def), || Some("whether it is modal, as entwine requires (702.42): checked of its bodies".into()))?.is_some() {
        r.need(crate::program::spell_choices(def, true))?;
    }
    if crate::keyword::escalate_count(def) > 1 { r.say("multiple escalate costs (702.120b)")?; }
    r.part(|| format!("{}, its escalate cost", def.name()));
    if let (Some(plan), Some((_, cost))) = (&def.costs.escalate, crate::keyword::escalate(def)) { r.several(plan.clone(), cost.0.len(), "cost parts")?; }
    if crate::keyword::escalate(def).is_some() && matches!(r.lowered(crate::program::spell_modal(def), || Some("whether it is modal, as escalate requires (702.120a): checked of its bodies".into()))?, Some(None)) {
        r.say("escalate requires a modal spell (702.120a)")?;
    }
    if crate::keyword::buyback_count(def) > 1 { r.say("multiple buyback costs require separately identified announcements (702.27)")?; }
    if crate::keyword::kicker_count(def) > 1 { r.say("multiple kicker costs require separately identified announcements (702.33f)")?; }
    for registration in &def.cost_changes {
        r.part(|| ability_part(def, registration.ability));
        r.several(registration.result.as_ref().map_err(Clone::clone), stated(def, registration.ability), "instructions")?;
    }
    r.part(|| format!("{}, its additional costs", def.name()));
    let parts = def.casting_costs().map(|(_, cost)| cost.0.len()).sum();
    r.several(def.costs.additional.clone(), parts, "additional cost parts")?;
    // a spell cast as an Adventure is exiled as it resolves and may be played from there
    // (715.3d): followed as a plain instant or sorcery it would go to the graveyard
    if !alternative && def.subtypes.contains(&ir::Subtype::adventure()) {
        r.say("an Adventure: exiled as it resolves, played from exile")?;
    }
    // madness functions in a hand whatever the card's type (702.35a): one madness ability, as
    // two would be two replacements of one discard, the second not applying once the first
    // has (616.1); its triggered ability, run as any other
    // gift on a permanent is an enters trigger, "When this permanent enters, if its gift cost
    // was paid, …" (702.174b), not the spell ability made here
    if def.keyword_declarations.iter().any(|i| i.declared.as_ref().is_ok_and(|d| d.needs.contains(&crate::keyword::Context::GiftOnSpell))) && !(def.is(CardType::Instant) || def.is(CardType::Sorcery)) {
        r.say("gift on a permanent: its enters trigger (702.174b)")?;
    }
    let madness = crate::keyword::madness(def);
    if crate::keyword::contributions(def).filter(|(_, g)| matches!(g, crate::keyword::Gives::Replacement(crate::keyword::Replacement::Madness(_)))).count() > 1 {
        r.say("madness: more than one madness ability on a card")?;
    }
    for (_, _, trigger) in madness {
        r.part(|| ability_part(def, trigger));
        r.trigger(def, trigger, def.trigger_ok(trigger).expect("madness has a triggered ability"))?;
        r.body(def, trigger)?;
    }
    // Keyword activations are independent of the spell's resolving instructions,
    // including when the printed card is an instant or sorcery (702.29a/e).
    for (offset, (_, ability)) in def.keyword_abilities.iter().enumerate() {
        if matches!(ability, Ability::Activated { .. }) {
            let index = def.abilities.len() + offset;
            r.part(|| ability_part(def, index));
            r.need(def.scope_ok(index).expect("derived activation"))?;
            r.several(def.ability_cost(index).expect("derived activated cost").clone(), cost_parts(def, index), "cost parts")?;
            r.body(def, index)?;
        }
    }
    if def.is(CardType::Instant) || def.is(CardType::Sorcery) {
        for (row, a) in def.abilities.iter().enumerate() {
            r.part(|| ability_part(def, row));
            if matches!(a, Ability::Keywords(_)) { keywords(def, row, r)?; }
        }
        // A stack static is already in force before resolution; it is not an instruction
        // to perform when the spell resolves (113.6g). Still validate its conditions.
        for (i, _) in def.abilities.iter().enumerate().filter(|(_, a)| crate::functions::stack_static(a)) {
            r.part(|| ability_part(def, i));
            r.several(def.restriction_ok(i).expect("a stack static is a restriction"), stated(def, i), "instructions")?;
        }
        for (i, (_, ability)) in def.keyword_abilities.iter().enumerate() {
            if matches!(ability, Ability::Triggered { .. }) {
                let index = def.abilities.len() + i;
                r.part(|| ability_part(def, index));
                r.trigger(def, index, def.trigger_ok(index).expect("keyword trigger"))?;
                r.body(def, index)?;
            }
        }
        // each spell ability is lowered on its own (compile_definition): of one refused, the
        // others are still analyzed; then what the spell's bodies are together
        let spell = def.spell_paragraphs();
        // of the checks of them together, the one a refused paragraph leaves unanalyzed
        let modal = spell.iter().filter(|(i, _)| !def.modes_of(*i).is_empty()).count() > 1;
        let together = || format!("{}, whether more than one of its spell abilities is modal: checked of their bodies together", def.name());
        let mut lowered = true;
        // a paragraph that is not the spell's instructions is refused whole, before the bodies
        // (so the first refusal is the one admission gives), and none of its own checks made
        for (i, why) in spell.iter().filter_map(|(i, p)| Some((*i, p.as_ref().err()?))) {
            r.part(|| ability_part(def, i));
            r.stop(why.0.clone())?;
            lowered = false;
            if modal { r.note(&why.0, together()); }
        }
        for &(i, _) in spell.iter().filter(|(_, p)| p.is_ok()) {
            r.part(|| ability_part(def, i));
            if !def.programs.iter().any(|p| p.ability == i) || r.body(def, i)?.is_some() { continue; }
            lowered = false;
            if modal && let Err(why) = def.executable_body(i) { r.note(&why.0, together()); }
        }
        r.part(|| format!("{}, its spell abilities", def.name()));
        if lowered { r.need(crate::program::spell_bodies(def))?; }
        return check_cost(def, r);
    }
    if def.is(CardType::Battle) {
        r.say("defense")?;
    }
    // A back face may omit printed loyalty: transforming preserves existing counters
    // (712.18), while entering transformed supplies only its own printed loyalty.
    if def.is(CardType::Planeswalker) && def.loyalty.is_none() && def.front_mana_value.is_none() {
        r.say("a planeswalker without a printed number of loyalty counters (306.5b)")?;
    }
    // a Room is two halves on one type line, and a half has its abilities only while it is
    // unlocked (709.5c–d): one half's text alone, not read as a half of its card, is not
    // what the permanent does
    if def.subtypes.contains(&ir::Subtype::room()) && def.door_of(0).is_none() {
        r.say("a Room: locked and unlocked halves")?;
    }
    if def.is(CardType::Creature) && def.pt.is_none() {
        r.say("power and toughness that are not numbers")?;
    }
    // the choices are made one at a time before the permanent enters (614.12a); the game asks
    // one of a card
    if def.abilities.iter().filter(|a| crate::replace::as_enters(a).is_some()).count() > 1 {
        r.say("as this enters: more than one choice")?;
    }
    if def.replacements.iter().filter(|r| matches!(&r.result, Ok(crate::replace::Replacement::CopyEntry(_)))).count() > 1 {
        r.say("entry copy: ordering multiple copy replacements of one definition")?;
    }
    let destinations: Vec<_> = def.replacements.iter().filter_map(|r| match &r.result {
        Ok(crate::replace::Replacement::DeathToLibrary(position)) => Some(*position), _ => None,
    }).collect();
    if destinations.iter().any(|p| Some(p) != destinations.first()) {
        r.say("death destination: ordering competing replacements on one object")?;
    }
    for (i, a) in def.abilities.iter().chain(def.keyword_abilities.iter().map(|(_, a)| a)).enumerate() {
        let rows = def.modes_of(i);
        r.part(|| ability_part(def, i));
        // where it functions: on the battlefield, or in the zone it moves its card out of
        // (113.6m)
        r.need(def.scope_ok(i).expect("an ability"))?;
        if crate::replace::acts_as_enters(a) {
            r.stop("an act done as this permanent enters (614.1c)")?;
            continue;
        }
        match a {
            Ability::Companion { .. } | Ability::CompanionRule(_) => {},
            _ if def.library_permissions.iter().any(|p| p.ability==i) => {r.several(def.library_permissions.iter().find(|p|p.ability==i).unwrap().result.as_ref().map_err(Clone::clone), stated(def, i), "instructions")?;},
            _ if def.cast_permissions.iter().any(|p| p.ability == i) => { r.several(def.cast_permissions.iter().find(|p|p.ability==i).unwrap().result.as_ref().map_err(Clone::clone), stated(def, i), "instructions")?; },
            a if crate::functions::casting_cost(a).is_some() || crate::functions::optional_cost(a).is_some() || crate::functions::casting_zone(a).is_some() => {},
            // of a modal ability each mode, which may be chosen alone or with others (700.2)
            Ability::Triggered { trigger, .. } => {
                r.trigger(def, i, def.trigger_ok(i).expect("a triggered ability"))?;
                // a triggered mana ability resolves at once (605.4a): one with modes, of which
                // none is chosen before it resolves, is not
                let Some(body) = r.body(def, i)? else {
                    if crate::trigger::mana_event(trigger) && !rows.is_empty() && let Err(why) = def.executable_body(i) {
                        r.note(&why.0, format!("{}, whether it is a triggered mana ability with modes (605.1b): checked of its body", ability_part(def, i)));
                    }
                    continue
                };
                let programs = body.programs();
                if crate::trigger::mana_event(trigger) && programs.len() > 1 && programs.iter().any(|p| p.announcement.adds_mana) {
                    r.say("triggered mana ability with modes (605.1b)")?;
                }
            }
            Ability::Activated { .. } => {
                r.several(def.ability_cost(i).expect("activated cost").clone(), cost_parts(def, i), "cost parts")?;
                r.body(def, i)?;
            }
            // a chapter ability is a triggered ability of a Saga (714.2); its lore counters
            // are a Saga's (714.3)
            Ability::Chapter { .. } => {
                if !crate::saga::is_saga(&def.subtypes) { r.stop("chapter ability of a permanent that is not a Saga")?; continue; }
                r.need(def.trigger_ok(i).expect("a chapter ability"))?;
                r.body(def, i)?;
            }
            _ if !rows.is_empty() => r.stop("modes under an ability that is neither a spell's, triggered nor activated")?,
            // keyword abilities the engine gives their rule's meaning (702)
            Ability::Keywords(_) => keywords(def, i, r)?,
            // an opening-hand action (103.6): what it enters with, and what it has its player do
            _ if def.opening.contains(&i) => {
                if def.opening.len() > 1 { r.stop("opening hand: more than one action of a card")?; continue; }
                if let Some(crate::start::Kind::Begin { counters: Some((kind, _)), .. }) = crate::start::kind(a) { r.need(crate::layers::counter_ok(&kind))?; }
                if crate::start::follow_up(a).is_some() { r.body(def, i)?; }
            },
            // a static ability that says what its subject gets or has (604.1, 611.3)
            _ if def.activation_modifier_ok(i).is_some() => { r.several(def.activation_modifier_ok(i).expect("an activation modifier"), stated(def, i), "instructions")?; },
            _ if def.activation_timing_ok(i).is_some() => { r.several(def.activation_timing_ok(i).expect("a timing permission"), stated(def, i), "instructions")?; },
            _ if def.unused_permission_ok(i).is_some() => { r.several(def.unused_permission_ok(i).expect("an activation permission"), stated(def, i), "instructions")?; },
            _ if def.cost_change_ok(i).is_some() => { r.several(def.cost_change_ok(i).expect("a cost modifier"), stated(def, i), "instructions")?; },
            _ if def.characteristic_ok(i).is_some() => {
                r.several(def.characteristic_ok(i).expect("a statement"), stated(def, i), "instructions")?;
            }
            // a static ability that is a restriction or a requirement (604.1)
            _ if def.restriction_ok(i).is_some() => {
                r.several(def.restriction_ok(i).expect("a statement"), stated(def, i), "instructions")?;
            }
            _ if def.trigger_modifiers.iter().any(|m| m.ability == i) => {
                r.several(def.trigger_modifiers.iter().find(|m| m.ability == i).unwrap().result.as_ref().map_err(Clone::clone), stated(def, i), "instructions")?;
            }
            // All existing replacement families share definition-time admission.
            _ if def.replacement_ok(i).is_some() => { r.several(def.replacement_ok(i).expect("a replacement"), stated(def, i), "instructions")?; }
            other => r.stop(format!("ability on a permanent: {}", said(other)))?,
        }
    }
    check_cost(def, r)
}

/// What a permanent's ability the engine does not run says, in the grammar's terms: the kind
/// of sentence its text opens with, and of a statement what it states.
/// The name an answer gives, when it gives one.
fn value_name(answer: &Answer) -> Option<&str> {
    match answer { Answer::Value(crate::binding::Value::CardName(name)) => Some(name), _ => None }
}

impl Game {
    /// What asks the pending choice, as a refused other move names it: an opening-hand action,
    /// a state-based action, a replacement as a permanent enters or a commander moves, or
    /// else the effect being applied.
    fn pending_choice(&self, choice: &Choice) -> &'static str {
        use crate::{operation::Operation, procedure::Procedure};
        if !self.begun() { return "an opening-hand action (103.6)"; }
        match (self.procedures.last(), choice) {
            (Some(Procedure::StateBased(_)), Choice::CommandZone { .. }) => "whether the commander goes to the command zone (903.9a)",
            (Some(Procedure::StateBased(_)), _) => "a choice a state-based action asks for (704.3)",
            (_, Choice::CommandZone { .. }) => "whether the commander goes to the command zone instead (903.9b)",
            (Some(Procedure::Operation(Operation::Enter { .. })), Choice::Attach { .. }) => "what the entering Aura enchants (303.4f)",
            (Some(Procedure::Operation(Operation::Enter { .. })), _) => "a choice of how the permanent enters the battlefield (614.12)",
            _ => "a choice of the effect being applied (608.2d)",
        }
    }

    /// Why a name answer is refused: a split card's full name is no card's name (201.4b), a
    /// name is one in the Oracle card reference (201.4), and one of a qualified choice has its
    /// characteristics (201.4a).
    fn name_refusal(name: Option<&str>, qualified: bool) -> String {
        let halves = |n: &str| n.split_once(" // ").is_some_and(|(a, b)| oracle::names::is_name(a) && oracle::names::is_name(b));
        match name {
            Some(n) if halves(n) => "choose the name of one half of a split card, not both (201.4b)".into(),
            Some(n) if qualified && oracle::names::is_name(n) => "choose the name of a card with the characteristics the choice says (201.4a)".into(),
            _ => "choose the name of a card in the Oracle card reference (201.4)".into(),
        }
    }
}

fn said(a: &Ability) -> String {
    let Ability::Text(Effect(sentences)) = a else { return name(a) };
    match sentences.first() {
        Some(Sentence::Do(body)) => {
            let act = &body.first.first;
            match &act.action {
                Action::Static(s) => format!("static: {}", name(s.as_ref())),
                Action::Cant(items) => match items.first().and_then(|f| f.event.reading()) {
                    Some((verb, _)) => format!("prohibition: {verb:?}"),
                    None => format!("prohibition: {}", items.first().map(|f| name(&f.event)).unwrap_or_default()),
                },
                a if !act.mods.is_empty() => format!("{} with modifier {}", name(a), name(&act.mods[0])),
                a => format!("statement: {}", name(a)),
            }
        }
        Some(Sentence::Static(s)) => format!("sentence: {}", name(s.as_ref())),
        Some(Sentence::Led { lead, .. }) => format!("lead: {}", name(lead)),
        Some(other) => format!("sentence: {}", name(other)),
        None => "no text".to_string(),
    }
}

fn check_cost(def: &CardDef, r: &mut Refusals) -> Res<()> {
    let tap_substitution = crate::keyword::contributions(def).find_map(|(_, gives)| match gives {
        crate::keyword::Gives::Substitute(crate::keyword::Substitution::Convoke) => Some("convoke"),
        crate::keyword::Gives::Substitute(crate::keyword::Substitution::Improvise) => Some("improvise"),
        _ => None,
    });
    if let Some(kind) = tap_substitution {
        let plans=def.costs.normal.iter().chain(&def.costs.flashback).chain(&def.costs.escape).chain(&def.costs.harmonize).chain(&def.costs.foretell).chain(def.costs.offered.iter().map(|(_,plan)|plan));
        if plans.filter_map(|p|p.as_ref().ok()).any(|p|p.has_zone_cost()) || def.costs.alternatives.iter().filter_map(|(_,p)|p.as_ref().ok()).any(|p|p.plan.has_zone_cost()) {
            r.say(format!("{kind} combined with a zone-changing object cost requires independent resource kinds"))?;
        }
    }
    if crate::keyword::contributions(def).filter(|(_, g)| matches!(g, crate::keyword::Gives::Route { route: CastRoute::Escape, .. })).count() > 1 {
        r.say("escape: choosing among multiple escape costs")?;
    }
    if crate::keyword::contributions(def).filter(|(_, g)| matches!(g, crate::keyword::Gives::Route { route: CastRoute::Harmonize { .. }, .. })).count() > 1 {
        r.say("harmonize: choosing among multiple harmonize costs")?;
    }
    if crate::keyword::of_items(def).filter_map(crate::foretell::cost).count() > 1 {
        r.say("foretell: choosing among multiple foretell costs")?;
    }
    if def.costs.offered.iter().enumerate().any(|(i, (row, _))| def.costs.offered[..i].iter().any(|(other, _)| other == row)) {
        r.say("offered alternative costs: several instances on one printed row need distinct cost identities")?;
    }
    r.part(|| format!("{}, its casting costs", def.name()));
    // each lowered part by part, as made from what the card states (Definitions::compile)
    let parts = |cost: Option<ir::Cost>| cost.map_or(1, |c| c.0.len());
    let casts = [(&def.costs.normal, 1), (&def.costs.flashback, parts(crate::keyword::flashback(def))), (&def.costs.escape, parts(crate::keyword::escape(def))),
        (&def.costs.harmonize, parts(crate::keyword::harmonize(def))), (&def.costs.foretell, parts(crate::keyword::of_items(def).find_map(crate::foretell::cost)))];
    for (plan, n) in casts { if let Some(plan) = plan { r.several(plan.clone(), n, "cost parts")?; } }
    for ((_, plan), (_, cost, _)) in def.costs.offered.iter().zip(crate::keyword::offered_costs(def)) { r.several(plan.clone(), cost.0.len(), "cost parts")?; }
    for ((_, plan), (_, cost, condition)) in def.costs.alternatives.iter().zip(def.alternative_costs()) {
        r.several(plan.as_ref().map_err(Clone::clone), usize::from(condition.is_some()) + cost.0.len(), "cost parts and condition")?;
    }
    for ((_, _, plan), (_, _, cost)) in def.costs.optional.iter().zip(crate::keyword::optional_costs(def)) { r.several(plan.as_ref().map_err(Clone::clone), cost.0.len(), "cost parts")?; }
    // one optional additional cost is announced paid or not; several would each be announced
    // and linked on their own (601.2b, 702.153b, 702.175b)
    if def.costs.optional.len() > 1 { r.say("optional additional costs: more than one on a spell (601.2b)")?; }
    Ok(())
}

impl Game {
    /// A player's move. A refused one changes nothing, except that an illegal total combat
    /// damage assignment returns to the moment before that player began to assign (510.1e);
    /// debug builds check both against the game as it was. Unsupported execution restores
    /// its prior state.
    pub fn act(&mut self, player: PlayerId, mv: Move) -> Result<(), String> {
        self.attempt(player, mv).map_err(|refused| refused.to_string())
    }

    /// [`Self::act`], saying what kind of refusal a refused move is.
    pub fn attempt(&mut self, player: PlayerId, mv: Move) -> ActionResult<()> {
        self.attempt_with(player, mv, Self::act_proposed)
    }
    // Private seam: tests propose a refusal that changes the game, which the check must catch.
    #[cfg(all(test, debug_assertions))]
    fn act_with(&mut self, player: PlayerId, mv: Move, propose: impl FnOnce(&mut Game, PlayerId, Move) -> ActionResult<()>) -> Result<(), String> {
        self.attempt_with(player, mv, propose).map_err(|refused| refused.to_string())
    }
    fn attempt_with(&mut self, player: PlayerId, mv: Move, propose: impl FnOnce(&mut Game, PlayerId, Move) -> ActionResult<()>) -> ActionResult<()> {
        let (mv, audit) = match mv { Move::AuditPayment(inner) => (*inner, true), other => (other, false) };
        #[cfg(debug_assertions)]
        let dividing = matches!(mv, Move::Divide(_));
        #[cfg(debug_assertions)]
        self.note_assignment_began(dividing);
        let completed_before = self.completed_choices.len();
        let before = self.clone();
        if audit { self.begin_payment_audit(); }
        // a refusal deferred to the move's end makes it unsupported, restored below (613.7m)
        let result = self.deferring(|game| propose(game, player, mv));
        if audit { self.payment_audit = None; }
        match &result {
            Ok(()) => {
                // A refused move preserves the previous public output. On success,
                // retain only answers produced by this move, including one that
                // finished its resolution before a viewer could observe it.
                if completed_before > 0 { self.completed_choices_mut().drain(..completed_before); }
                #[cfg(debug_assertions)]
                self.note_assignment_interrupted(dividing);
            }
            Err(refused @ (ActionError::Refused(_) | ActionError::Unreferable(_) | ActionError::NoDecision)) => {
                #[cfg(debug_assertions)]
                crate::state::assert_unchanged(&before, self, &refused.to_string());
                #[cfg(not(debug_assertions))]
                let _ = refused;
            }
            Err(ActionError::Restarted(reason)) => {
                #[cfg(debug_assertions)]
                self.assert_assignment_restarted(reason);
                #[cfg(not(debug_assertions))]
                let _ = reason;
            }
            Err(ActionError::Unsupported(_)) => self.adopt(before),
        }
        result
    }
    fn act_proposed(&mut self, player: PlayerId, mv: Move) -> ActionResult<()> {
        // a move by, or naming, a player the game never had is no move of this game
        let mut unknown = (player >= self.players.len()).then_some(player);
        crate::retention::Players::players(&mv, &mut |named| if unknown.is_none() && named >= self.players.len() { unknown = Some(named) });
        if let Some(named) = unknown { return Err(ActionError::Unreferable(Target::Player(named))); }
        // "A player can concede the game at any time" (104.3a): whoever has priority and
        // whatever decision is pending, the conceding player's own move, never one made for
        // a controlled player (723.5).
        if mv == Move::Concede { return self.concede(player); }
        self.reconcile_linked_acquisitions();
        let caller=player;
        let actor=self.decision_actor();
        let maker=self.decision_maker(actor);
        let player=if maker != actor {
            if player != maker { return Err("that player's decisions are controlled by another player (723.5)".into()); }
            actor
        } else { player };
        let mut stale = None;
        crate::retention::Trace::trace(&mv, &mut |id| if stale.is_none() && self.get_object(id).is_none() { stale = Some(id) });
        if let Some(id) = stale { return Err(ActionError::Unreferable(Target::Object(id))); }
        if let Move::ContinueManaPayment { source, mana, adding } = mv {
            return self.continue_mana_payment(player, caller, source, mana, adding);
        }
        if self.finished() {
            return Err("the game is over".into());
        }
        if self.players.get(player).is_none_or(|p| p.lost) {
            return Err("that player is not in the game (800.4a)".into());
        }
        if let Some(Decision::Payment { player: who, .. }) = &self.decision {
            if player != *who { return Err("that decision is another player's".into()); }
            if mv == Move::ReversePayment {
                self.reverse_payment()?;
            } else {
                let Move::FinishPayment { plan, paying, with } = mv else {
                    return Err("a decision is pending: complete the announced payment (601.2g-h)".into());
                };
                self.finish_payment(plan, paying, with)?;
            }
            // An effect's payment ends inside its resolution, which may still be going on.
            if self.resolving().is_none() { self.settle()?; }
            return Ok(());
        }
        // before the first turn: the mulligans (103.5)
        if let Some(Decision::Mulligan { player: who }) = self.decision {
            let mulligan = match mv {
                Move::Mulligan => true,
                Move::Keep => false,
                _ => return Err("a decision is pending: whether to take a mulligan (103.5)".into()),
            };
            if player != who {
                return Err("that decision is another player's".into());
            }
            self.declare_mulligan(player, mulligan)?;
            return Ok(());
        }
        if let Some(Decision::Bottom { player: who, count }) = self.decision {
            let Move::Bottom(cards) = mv else {
                return Err("a decision is pending: put cards on the bottom of your library (103.5)".into());
            };
            if player != who {
                return Err("that decision is another player's".into());
            }
            return self.put_on_bottom(player, count, &cards).map_err(Into::into);
        }
        if let Some(Decision::Discard { player: who, count }) = self.decision.clone() {
            let Move::Discard(cards) = mv else {
                return Err("a decision is pending: discard down to your maximum hand size (514.1)".into());
            };
            if player != who {
                return Err("that decision is another player's".into());
            }
            return self.discard_to_hand_size(player, count, &cards);
        }
        // the turn-based actions of combat: once the players have decided, the active player
        // receives priority (508.2, 509.2, 510.3)
        if let Some(decision @ (Decision::Attackers { .. } | Decision::Blockers { .. } | Decision::Divide { .. })) = self.decision.clone() {
            let who = match &decision {
                Decision::Attackers { player } | Decision::Blockers { player } | Decision::Divide { player, .. } => *player,
                _ => unreachable!("a combat decision"),
            };
            if player != who {
                return Err("that decision is another player's".into());
            }
            match (decision, mv) {
                (Decision::Attackers { .. }, Move::Attack(attacks)) => self.declare_attackers(attacks)?,
                (Decision::Blockers { .. }, Move::Block(blocks)) => self.declare_blockers(player, blocks)?,
                (Decision::Divide { attacker, among, amount, .. }, Move::Divide(division)) => {
                    self.divide_damage(attacker, &among, amount, division)?
                }
                (Decision::Attackers { .. }, _) => return Err("a decision is pending: declare attackers (508.1)".into()),
                (Decision::Blockers { .. }, _) => return Err("a decision is pending: declare blockers (509.1)".into()),
                _ => return Err("a decision is pending: divide the combat damage (510.1c)".into()),
            }
            if self.decision.is_none() {
                self.passes = 0;
                self.priority_from(self.active);
                self.settle()?;
            }
            return Ok(());
        }
        // a choice of the effect being applied (608.2d): its instructions go on from there
        if let Some(Decision::Choose { player: who, choice }) = self.decision.clone() {
            // An effect's offer to cast is taken by casting: the move a spell is cast with.
            if let (Choice::Cast { cards }, false) = (&choice, matches!(mv, Move::Choose(_))) {
                if player != who {
                    return Err("that decision is another player's".into());
                }
                // A cast that cannot be made before a new question changes nothing.
                let mut proposed = self.clone();
                proposed.decision = None;
                proposed.cast_during(player, cards, mv)?;
                proposed.continue_execution()?;
                proposed.expose_payments();
                if proposed.resolving().is_none() {
                    proposed.passes = 0;
                    proposed.settle()?;
                }
                self.adopt(proposed);
                return Ok(());
            }
            let Move::Choose(answer) = mv else {
                return Err(format!("a decision is pending: {}", self.pending_choice(&choice)).into());
            };
            if player != who {
                return Err("that decision is another player's".into());
            }
            match (&choice, &answer) {
                (Choice::Alternative { optional: true, .. }, Answer::No) => {},
                (Choice::Alternative { among, .. }, Answer::Alternative(index)) if among.iter().any(|option| option.index == *index) => {},
                (Choice::Alternative { .. }, _) => return Err("choose one of the offered actions, or decline if optional (608.2d)".into()),
                (Choice::Search { among, min, max, .. } | Choice::Select { among, min, max }, answer) => {
                    // each mistake named, as for the verb choices below; the count follows,
                    // in the words the tests of this refusal expect
                    let search = matches!(choice, Choice::Search { .. });
                    let count = format!("choose between {min} and {max} distinct matching objects");
                    let Answer::Objects(chosen) = answer else {
                        return Err(format!("that answer chooses no objects (608.2d): {count}").into());
                    };
                    if chosen.iter().any(|o| !among.contains(o)) {
                        return Err(if search {
                            format!("a chosen card is not one the search can find, in the searched zone and matching its description (701.23a): {count}").into()
                        } else {
                            format!("a chosen object is not among the offered objects (608.2d): {count}").into()
                        });
                    }
                    if chosen.iter().enumerate().any(|(i, o)| chosen[..i].contains(o)) {
                        return Err(format!("each object is chosen once (608.2d): {count}").into());
                    }
                    if !(*min..=*max).contains(&chosen.len()) {
                        return Err(format!("{} chosen (608.2d): {count}", chosen.len()).into());
                    }
                }
                (Choice::Holders { among, .. }, answer) => {
                    let legal = matches!(answer, Answer::Holders(chosen) if chosen.iter().all(|t| among.contains(t))
                        && chosen.iter().enumerate().all(|(i,t)| !chosen[..i].contains(t)));
                    if !legal { return Err("choose a distinct subset of the offered counter holders (701.34a)".into()); }
                }
                (Choice::Defender { among, .. }, Answer::Defender(t)) if among.contains(t) => {}
                (Choice::Defender { .. }, _) => return Err("choose one of the offered players or planeswalkers for it to attack (508.4)".into()),
                (Choice::Attach { among, .. }, Answer::Attach(t)) if among.contains(t) => {}
                (Choice::Attach { .. }, _) => return Err("choose a legal attachment (303.4f)".into()),
                (Choice::CommandZone { .. }, Answer::Yes | Answer::No) => {}
                (Choice::CommandZone { .. }, _) => return Err("return the commander to the command zone: yes or no (903.9)".into()),
                (Choice::Miracle { .. }, Answer::Yes | Answer::No) => {},
                (Choice::Miracle { .. }, _) => return Err("choose whether to reveal this drawn card using Miracle: yes or no (702.94a)".into()),
                (Choice::May { .. }, Answer::No) | (Choice::May { possible: true }, Answer::Yes) => {}
                (Choice::May { possible: false }, Answer::Yes) => return Err("that option is impossible (608.2d)".into()),
                (Choice::May { .. }, _) => return Err("the choice is whether to do it: yes or no (608.2d)".into()),
                (Choice::OptionalObjects { .. }, Answer::OptionalObjects(None)) => {},
                (Choice::OptionalObjects { verb, among, count, .. }, Answer::OptionalObjects(Some(chosen))) => {
                    if chosen.iter().any(|o| !among.contains(o)) {
                        let word = if *verb == ir::Verb::Move {
                            if self.moved_as() == Some(ir::action::MoveVerb::Return) { "return".into() } else { "put".into() }
                        } else { verb.bare(ir::Inflection::Base) };
                        return Err(format!("a chosen object is not among the offered objects to {word} (608.2d)").into());
                    }
                    if chosen.len() != *count || chosen.iter().enumerate().any(|(i, o)| chosen[..i].contains(o)) {
                        return Err(format!("accept with exactly {count} distinct offered objects (608.2d)").into());
                    }
                },
                (Choice::OptionalObjects { .. }, _) => return Err("decline without objects, or accept with the offered object selection (608.2d)".into()),
                (Choice::ChangeTargets { object, original, slots, policy }, Answer::Targets(targets)) => {
                    self.legal_changed_targets(*object, original, slots, *policy, targets)?;
                    self.retained_target_relations(*object, original, targets)?;
                },
                (Choice::ChangeTargets { policy: crate::state::TargetPolicy::ChooseNew, .. }, Answer::No) => {},
                (Choice::ChangeTargets { .. }, _) => return Err("choose targets according to the offered target-change policy (115.7)".into()),
                (Choice::Retarget { copy, original, .. }, Answer::Targets(targets)) => {
                    self.legal_copy_targets(*copy, original, targets)?;
                    self.retained_target_relations(*copy, original, targets)?;
                },
                (Choice::Retarget { .. }, Answer::No) => {},
                (Choice::Retarget { .. }, _) => return Err("choose new targets or keep the original targets (707.10c)".into()),
                (Choice::Cast { .. }, Answer::No) => {}
                (Choice::Cast { .. }, _) => return Err("the choice is to cast one of the cards offered, or no (608.2g)".into()),
                (Choice::Pay { .. }, Answer::No) => {}
                (Choice::Pay { .. }, Answer::Yes) => return Err("submit explicit payment instructions or decline (118.12)".into()),
                (Choice::Pay { waterbend: None, .. }, Answer::Pay { tapped, .. }) if !tapped.is_empty() => return Err("only a waterbend cost is paid by tapping artifacts and creatures (701.67a)".into()),
                (Choice::Pay { .. }, Answer::Pay { .. }) => {
                    // A payment that cannot be made before a new question changes nothing,
                    // like any other answer; the player may still decline (118.12).
                    let mut proposed = self.clone();
                    proposed.decision = None;
                    proposed.answer_procedure(player, answer)?;
                    proposed.continue_execution()?;
                    proposed.expose_payments();
                    if proposed.resolving().is_none() {
                        proposed.passes = 0;
                        proposed.settle()?;
                    }
                    self.adopt(proposed);
                    return Ok(());
                }
                (Choice::Pay { .. }, _) => return Err("submit explicit payment instructions or decline (118.12)".into()),
                (Choice::ConstrainedObjects { among, selection, .. }, Answer::Objects(chosen))
                    if chosen.iter().enumerate().all(|(i,o)| among.contains(o) && !chosen[..i].contains(o)) && selection.accepts(chosen) => {},
                (Choice::ConstrainedObjects { selection: crate::cost::Selection::UpTo(maximum), .. }, _) =>
                    return Err(format!("choose up to {maximum} distinct offered objects (608.2d)").into()),
                (Choice::ConstrainedObjects { .. }, _) => return Err("choose distinct offered cards satisfying the whole selection (701.59a)".into()),
                (Choice::OrderCards { cards, .. }, Answer::Objects(ordered))
                    if ordered.len() == cards.len() && ordered.iter().enumerate().all(|(i, card)| cards.contains(card) && !ordered[..i].contains(card)) => {},
                (Choice::OrderCards { .. }, _) => return Err("order every offered card exactly once (401.4, 404.3)".into()),
                (Choice::Objects { verb, to, among, count }, answer) => {
                    // a zone change is "return" or "put" as the text says it ("return a land you
                    // control to its owner's hand", "put a land card from your graveyard onto
                    // the battlefield"); where the text is not at hand, "return" from the
                    // battlefield to a hand, or from a graveyard or exile, otherwise "put"
                    use crate::state::Placement;
                    let said = self.moved_as();
                    let returned = |to: Zone| among.iter().all(|o| match self.obj(*o).zone {
                        Zone::Battlefield => to == Zone::Hand,
                        Zone::Graveyard | Zone::Exile => true,
                        _ => false,
                    });
                    let word = match (verb, to) {
                        (ir::Verb::Move, _) if said == Some(ir::action::MoveVerb::Return) => "return".to_string(),
                        (ir::Verb::Move, Some(Placement::Zone(to @ (Zone::Hand | Zone::Battlefield))))
                            if said.is_none() && !among.is_empty() && returned(*to) => "return".to_string(),
                        (ir::Verb::Move, _) => "put".to_string(),
                        (verb, _) => verb.bare(ir::Inflection::Base),
                    };
                    let Answer::Objects(chosen) = answer else {
                        return Err(format!("choose exactly {count} of the offered objects to {word} (608.2d)").into());
                    };
                    if chosen.iter().any(|o| !among.contains(o)) {
                        return Err(format!("a chosen object is not among the offered objects to {word} (608.2d)").into());
                    }
                    if chosen.iter().enumerate().any(|(i, o)| chosen[..i].contains(o)) {
                        return Err(format!("choose each object to {word} once (608.2d)").into());
                    }
                    if chosen.len() != *count {
                        return Err(format!("choose exactly {count} of them to {word} (608.2d)").into());
                    }
                }
                (Choice::CoinCall, Answer::CoinCall(_)) => {}
                (Choice::CoinCall, _) => return Err("call heads or tails before flipping the coin (705.2)".into()),
                (Choice::Mana { options }, Answer::Mana(chosen)) if options.contains(chosen) => {}
                (Choice::Mana { .. }, _) => return Err("the choice is which of the mana offered to add (608.2d)".into()),
                (Choice::LibraryDestination { positions, .. }, Answer::LibraryPosition(position)) if positions.contains(position) => {}
                (Choice::LibraryDestination { .. }, _) => return Err("choose one of the offered library positions (608.2d)".into()),
                (Choice::Number { min, max }, Answer::Value(crate::binding::Value::Number(n))) if min <= n && n <= max => {}
                (Choice::Number { .. }, _) => return Err("choose an integer within the offered numeric bounds (608.2d)".into()),
                (Choice::Value(kind), Answer::Value(value)) if crate::binding::accepts(*kind, value) => {}
                (Choice::Value(kind), _) => return Err(match kind {
                    crate::binding::Kind::Color => "choose one of the five colors (105.4)".into(),
                    crate::binding::Kind::CreatureType => "choose a creature type (205.3m)".into(),
                    crate::binding::Kind::CardType => "choose a card type (205.2a)".into(),
                    crate::binding::Kind::BasicLandType => "choose a basic land type (305.6)".into(),
                    crate::binding::Kind::CardName => Game::name_refusal(value_name(&answer), false).into(),
                    kind => format!("choose a legal {kind:?} value (608.2d)").into(),
                }),
                (Choice::Name(names), Answer::Value(value)) if crate::binding::accepts(crate::binding::Domain { kind: crate::binding::Kind::CardName, names: *names }, value) => {}
                (Choice::Name(_), _) => return Err(Game::name_refusal(value_name(&answer), true).into()),
                (Choice::Arrange { cards, .. }, Answer::Arrange { top, away }) if Game::arrangement(cards, top, away) => {}
                (Choice::Arrange { verb, .. }, _) => {
                    let rule = if *verb == ir::Verb::Scry { "701.22a" } else { "701.25a" };
                    return Err(format!("put each of the cards looked at back on top or away, once ({rule})").into());
                }
                (Choice::Player { among }, Answer::Player(chosen)) if among.contains(chosen) => {}
                (Choice::Player { .. }, _) => return Err("choose one of the players offered (608.2d)".into()),
                (Choice::Pile { piles }, Answer::Pile(chosen)) if *chosen < piles.len() => {}
                (Choice::Pile { .. }, _) => return Err("choose one of the piles (700.3)".into()),
                (Choice::DamageOrder {among,..},Answer::DamageOrder(offer)) if among.contains(offer)=>{},
                (Choice::DamageOrder{..},_)=>return Err("choose an offered damage/prevention identity (616.1)".into()),
                (Choice::Prevention{total,hits,..},Answer::Prevention(allocation)) if crate::operation::valid_allocation(*total,hits,allocation)=>{},
                (Choice::Prevention{..},_)=>return Err("allocate the required shield total over each offered hit within its capacity (615.7)".into()),
                (Choice::DamageReplacement { among, .. }, Answer::DamageReplacement(offer)) if among.contains(offer) => {},
                (Choice::DamageReplacement { .. }, _) => return Err("choose an offered effect and damage hit (616.1)".into()),
                (Choice::SpellDestination { on, among }, Answer::SpellDestination { on: chosen, effect }) if on==chosen && among.contains(effect) => {},
                (Choice::SpellDestination { .. }, _) => return Err("choose an offered destination for this resolving spell (616.1)".into()),
                (Choice::Replacement { optional: true, .. }, Answer::No) => {},
                (Choice::Replacement { among, .. }, Answer::Replacement(effect)) if among.contains(effect) => {}
                (Choice::Replacement { .. }, _) => return Err("choose one of the replacement effects that apply (616.1)".into()),
                // not part of a resolution: the turn goes on from the occurrence chosen for
                (Choice::Skip { .. }, Answer::Skip(id)) => return self.choose_skip(player, *id).map_err(Into::into),
                (Choice::Skip { .. }, _) => return Err("choose one of the skip effects offered (616.1)".into()),
            }
            self.decision = None;
            // Only the active procedure receives this answer. Child completion is delivered
            // by the common driver to the immediate parent, independent of caller kind.
            self.answer_procedure(player, answer)?;
            if self.decision.is_some() { return Ok(()); }
            self.go_on()?;
            if self.resolving().is_none() {
                self.passes = 0;
                self.settle()?;
            }
            return Ok(());
        }
        if let Some(Decision::Targets { player: who, ability, .. } | Decision::Modes { player: who, ability }) = self.decision.clone() {
            // of a modal ability the modes, then the targets of those (603.3c, 700.2c)
            let modal = matches!(self.decision, Some(Decision::Modes { .. }));
            let retained_modes = match &self.decision {
                Some(Decision::Targets { modes, .. }) => modes.clone(),
                _ => Vec::new(),
            };
            let mv = if let Move::ChooseModes(modes) = mv {
                if !modal { return Err("the triggered ability's modes have already been chosen".into()); }
                if player != who { return Err("that decision is another player's".into()); }
                ability.captured.as_ref().ok_or("the ability has no captured definition")?.as_ref()
                    .map_err(Clone::clone)?.legal_modes(&modes)?;
                let slots = self.ability_slots(&ability, &modes);
                if self.unfillable(&ability.context(), &slots, &ability.bound) {
                    return Err("the chosen modes have no legal target selection (603.3c)".into());
                }
                if !slots.is_empty() {
                    self.decision = Some(Decision::Targets { player: who, ability, groups: slots.len(), modes });
                    return Ok(());
                }
                // Target-free modes complete announcement without a spurious target question.
                Move::Modes(modes, Box::new(Move::ChooseTargets(Vec::new())))
            } else { mv };
            // the division, said with the targets it divides among (603.3d, 601.2d)
            let (said, mv) = match mv {
                Move::WithDivision(said, inner) => (Some(said), *inner),
                mv => (None, mv),
            };
            let mut said = said;
            let (modes, targets) = match mv {
                Move::ChooseTargets(targets) if !modal => (retained_modes, targets),
                Move::Modes(modes, inner) if modal => match *inner {
                    Move::ChooseTargets(targets) => (modes, targets),
                    Move::WithDivision(division, inner) if said.is_none() && matches!(*inner, Move::ChooseTargets(_)) => {
                        said = Some(division);
                        let Move::ChooseTargets(targets) = *inner else { unreachable!() };
                        (modes, targets)
                    }
                    _ => return Err("the modes of a triggered ability are chosen with the targets of those modes (603.3c)".into()),
                },
                _ if modal => return Err("a decision is pending: choose the modes of the triggered ability and their targets (603.3c)".into()),
                _ => return Err("a decision is pending: choose the targets of the triggered ability".into()),
            };
            if player != who {
                return Err("that decision is another player's".into());
            }
            if modal {
                ability.captured.as_ref().ok_or("the ability has no captured definition")?.as_ref()
                    .map_err(Clone::clone)?.legal_modes(&modes)?;
            }
            let ctx = ability.context();
            let slots = self.ability_slots(&ability, &modes);
            self.legal_targets_with(&ctx, &slots, &targets, &ability.bound)?;
            let division = self.legal_division(&ctx, &slots, &targets, said.as_deref().unwrap_or(&[]))?;
            self.decision = None;
            // the one whose targets these are: the first waiting (another like it may wait too)
            if let Some(at) = self.triggered.iter().position(|t| *t == ability) {
                self.triggered.remove(at);
                self.ordered = self.ordered.saturating_sub(1);
            }
            // "choose up to one —" with none chosen: the ability is removed (603.3c, 700.2b)
            if modal && modes.is_empty() {
                self.passes = 0;
                self.settle()?;
                return Ok(());
            }
            let placed = self.ability_on_stack(&ability, modes, targets);
            self.announcement_crime(ability.controller, placed);
            self.obj_mut(placed).division = division;
            self.passes = 0;
            self.settle()?;
            return Ok(());
        }
        if let Some(Decision::Order { player: who, abilities }) = self.decision.clone() {
            let Move::Order(order) = mv else {
                return Err("a decision is pending: the order of the abilities that triggered (603.3b)".into());
            };
            if player != who {
                return Err("that decision is another player's".into());
            }
            let mut sorted = order.clone();
            sorted.sort_unstable();
            if sorted != (0..abilities.len()).collect::<Vec<_>>() {
                return Err(format!("an order is each of the {} abilities once (603.3b)", abilities.len()).into());
            }
            // they are the first waiting, and now wait in the order chosen
            for (place, chosen) in order.iter().enumerate() {
                self.triggered[place] = abilities[*chosen].clone();
            }
            self.ordered = abilities.len();
            self.decision = None;
            self.settle()?;
            return Ok(());
        }
        if player != self.priority {
            return Err("that player does not have priority".into());
        }
        match mv {
            Move::Plot(card) => self.plot(player,card,&[],None)?,
            Move::Foretell(card) => self.foretell(player,card,&[],None)?,
            Move::Suspend(card) => self.suspend(player,card,&[],None)?,
            Move::TurnFaceUp(permanent, way) => self.turn_face_up_action(player,permanent,way,&[],None)?,
            Move::Unlock { room, half } => self.unlock_action(player,room,half,&[],None)?,
            Move::Companion => self.take_companion(player,&[],None)?,
            Move::PlayLand(card) => self.play_land(player, card, None)?,
            Move::PlayLandAs {card,face} => self.play_land(player,card,Some(face))?,
            Move::Activate { source, ability, targets, paying } => self.activate(player, source, ability, targets, paying)?,
            Move::Cast { card, targets } => self.cast(player, card, CastAs::Whole, None, &Substitutions::default(), Default::default(), &[], targets, &[], &[], None, &[], None, 0, false, false, false, 0, None, &[], &[], None)?,
            mv @ (Move::Paying(..) | Move::Adding(..) | Move::Modes(..) | Move::CastAs(..) | Move::WithMana(..) | Move::WithCosts(..) | Move::WithX(..) | Move::WithReplicate(..) | Move::WithKicks(..) | Move::WithBuyback(..) | Move::WithOptional(..) | Move::WithEntwine(..) | Move::WithLife(..) | Move::WithCostChoice(..) | Move::WithCostBranches(..) | Move::CastFor(..) | Move::CastVia(..) | Move::WithDelve(..) | Move::WithConvoke(..) | Move::WithImprovise(..) | Move::GiftTo(..) | Move::WithDivision(..)) => {
                let said = Said::read(mv)?;
                said.validate_optional_costs(self)?;
                let Said { with, adding, modes, cast_as, plan, paying, x, kicks, buyback, optional, entwine, replicate, life, chose, branches, cost, route, substitutions, gift, division, what, .. } = said;
                match what {
                    Move::Companion if adding.is_empty() && modes.is_empty() && cast_as.is_none() && paying.is_none() && x.is_none() && kicks == 0 && !buyback && !entwine && replicate == 0 && life.is_empty() && chose.is_empty() && cost.is_none() && route.is_none() && substitutions.is_empty() && gift.is_none() && division.is_empty() => self.take_companion(player,plan.as_deref().unwrap_or(&[]),with.as_ref())?,
                    Move::Plot(card) if adding.is_empty() && modes.is_empty() && cast_as.is_none() && paying.is_none() && x.is_none() && kicks == 0 && !buyback && !entwine && replicate == 0 && life.is_empty() && chose.is_empty() && cost.is_none() && route.is_none() && substitutions.is_empty() && gift.is_none() && division.is_empty() => self.plot(player,card,plan.as_deref().unwrap_or(&[]),with.as_ref())?,
                    Move::Foretell(card) if adding.is_empty() && modes.is_empty() && cast_as.is_none() && paying.is_none() && x.is_none() && kicks == 0 && !buyback && !entwine && replicate == 0 && life.is_empty() && chose.is_empty() && cost.is_none() && route.is_none() && substitutions.is_empty() && gift.is_none() && division.is_empty() => self.foretell(player,card,plan.as_deref().unwrap_or(&[]),with.as_ref())?,
                    Move::Suspend(card) if adding.is_empty() && modes.is_empty() && cast_as.is_none() && paying.is_none() && x.is_none() && kicks == 0 && !buyback && !entwine && replicate == 0 && life.is_empty() && chose.is_empty() && cost.is_none() && route.is_none() && substitutions.is_empty() && gift.is_none() && division.is_empty() => self.suspend(player,card,plan.as_deref().unwrap_or(&[]),with.as_ref())?,
                    Move::Unlock { room, half } if adding.is_empty() && modes.is_empty() && cast_as.is_none() && paying.is_none() && x.is_none() && kicks == 0 && !buyback && !entwine && replicate == 0 && life.is_empty() && chose.is_empty() && cost.is_none() && route.is_none() && substitutions.is_empty() && gift.is_none() && division.is_empty() => self.unlock_action(player,room,half,plan.as_deref().unwrap_or(&[]),with.as_ref())?,
                    Move::TurnFaceUp(permanent, way) if adding.is_empty() && modes.is_empty() && cast_as.is_none() && paying.is_none() && x.is_none() && kicks == 0 && !buyback && !entwine && replicate == 0 && life.is_empty() && chose.is_empty() && cost.is_none() && route.is_none() && substitutions.is_empty() && gift.is_none() && division.is_empty() => self.turn_face_up_action(player,permanent,way,plan.as_deref().unwrap_or(&[]),with.as_ref())?,
                    Move::Activate { source, ability, targets, paying: chosen } if cast_as.is_none() && paying.is_none() && kicks == 0 && !buyback && !entwine && replicate == 0 && cost.is_none() && route.is_none() && substitutions.is_empty() && gift.is_none() => {
                        self.activate_plan(player, source, ability, &modes, targets, &division, chosen, with.as_ref(), &adding, plan.as_deref().unwrap_or(&[]), x, &life, &chose, branches.as_deref())?
                    }
                    Move::Cast { card, targets } if adding.is_empty() => self.cast(player, card, cast_as.unwrap_or_default(), route, &substitutions, cost.unwrap_or_default(), &modes, targets, &division, paying.as_deref().unwrap_or(&[]), with.as_ref(), plan.as_deref().unwrap_or(&[]), x, kicks, buyback, optional, entwine, replicate, gift, &life, &chose, branches.as_deref())?,
                    _ => {
                        return Err(
                            "mana pays the cost of a spell or an ability, is chosen for a mana ability, and modes are chosen for a spell or ability (601.2b, 601.2h, 605.3b)"
                                .into(),
                        );
                    }
                }
            }
            Move::ChooseTargets(_)
            | Move::ChooseModes(_)
            | Move::Order(_)
            | Move::Discard(_)
            | Move::Mulligan
            | Move::Keep
            | Move::Bottom(_)
            | Move::Attack(_)
            | Move::Block(_)
            | Move::Divide(_)
            | Move::Choose(_)
            | Move::AuditPayment(_) | Move::ContinueManaPayment { .. }
            | Move::FinishPayment { .. }
            | Move::ReversePayment => {
                // answers to a decision, and none is pending
                return Err(ActionError::NoDecision);
            }
            Move::Concede => unreachable!("a concession is taken before priority is checked"),
            Move::Pass => {
                self.pass()?;
                // nothing is checked while an effect is being applied and waits for a choice
                if self.resolving().is_none() {
                    self.settle()?;
                }
                return Ok(());
            }
        }
        self.passes = 0;
        self.settle()?;
        Ok(())
    }

    /// Before a player receives priority (117.5): state-based actions, then the abilities that
    /// triggered go on the stack, and again until nothing more happens or a player must decide.
    pub(crate) fn settle(&mut self) -> ActionResult<()> {
        loop {
            if self.decision.is_some() { break; }
            if self.returning() { self.go_on()?; continue; }
            // Costs and turn-based actions can change which continuous effects apply (506.4).
            self.update_combat();
            // an ability triggers when its event happens (603.2), whatever a state-based
            // action then makes of its source (113.7a)
            self.check_triggers();
            if self.returning() { self.go_on()?; continue; }
            let acted = self.state_based_checked()?;
            if self.decision.is_some() { break; }
            self.check_triggers();
            if self.returning() { self.go_on()?; continue; }
            let placed = self.put_triggers();
            if self.decision.is_some() || !(acted || placed) {
                break;
            }
        }
        Ok(())
    }

    /// What a triggered ability's text targets, with these modes chosen (700.2c).
    pub(crate) fn ability_slots(&mut self, t: &Triggered, modes: &[usize]) -> Vec<TargetSpec> {
        if let Some(Ok(captured)) = &t.captured { return captured.targets(modes).unwrap_or_default(); }
        Vec::new()
    }

    /// The choices of modes a modal triggered ability can be put on the stack with: those
    /// whose every "target" has a legal target (603.3c, 700.2b). None: it is not modal.
    pub(crate) fn trigger_modes(&mut self, t: &Triggered) -> Option<Vec<Vec<usize>>> {
        if let Some(Ok(captured)) = &t.captured {
            let choices = captured.choices()?;
            return Some(choices.into_iter().filter(|modes| captured.targets(modes).is_ok_and(|slots| !self.unfillable(&t.context(), &slots, &t.bound))).collect());
        }
        None
    }

    /// Is there a "target" of these with fewer legal targets than it needs (601.2c, 603.3d)?
    fn unfillable(&self, ctx: &Ctx, slots: &[TargetSpec], locals: &dyn crate::program::expr::Locals) -> bool {
        let offers = self.target_offers(ctx,slots,locals);
        crate::legal::target_selection(&offers).is_none()
    }

    /// Put the abilities that triggered on the stack, in order (603.3). One that targets waits
    /// for its controller to choose; one with no legal target for a "target" it needs is
    /// removed (603.3d). Returns whether any went on the stack.
    fn put_triggers(&mut self) -> bool {
        // A player can leave after an ability triggers but before it is put on the stack
        // (800.4d). Remove those triggers before offering ordering, modes, or targets.
        self.ordered = self.triggered.iter().take(self.ordered).filter(|t| !self.players[t.controller].lost).count();
        let state = self.state_mut();
        state.triggered.retain(|t| !state.players[t.controller].lost);
        let mut placed = false;
        loop {
            // each player in APNAP order puts the abilities they control on the stack in
            // any order they choose (603.3b). Only fully identical captured occurrences
            // are exchangeable; the same source/ability can bind different subjects.
            if self.ordered == 0 && !self.triggered.is_empty() {
                let (n, active) = (self.players.len(), self.active);
                self.triggered.sort_by_key(|t| (t.controller + n - active) % n);
                let first = &self.triggered[0];
                let mine = self.triggered.iter().take_while(|t| t.controller == first.controller).count();
                if self.triggered[..mine].iter().any(|t| t != first) {
                    self.decision = Some(Decision::Order { player: first.controller, abilities: self.triggered[..mine].to_vec() });
                    break;
                }
                self.ordered = mine;
            }
            let Some(t) = self.triggered.first().cloned() else { break };
            // a modal ability: its controller chooses the modes, of those that can be chosen,
            // and their targets; with none that can be, it is removed (603.3c, 700.2b)
            if let Some(possible) = self.trigger_modes(&t) {
                // none, or only none of its modes ("choose up to one —" with nothing to target)
                if possible.iter().all(|modes| modes.is_empty()) {
                    self.triggered.remove(0);
                    self.ordered -= 1;
                    continue;
                }
                self.decision = Some(Decision::Modes { player: t.controller, ability: t });
                break;
            }
            let slots = self.ability_slots(&t, &[]);
            if slots.is_empty() {
                self.triggered.remove(0);
                self.ordered -= 1;
                let on_stack = self.ability_on_stack(&t, Vec::new(), Vec::new());
                self.announcement_crime(t.controller, on_stack);
                self.passes = 0;
                placed = true;
                continue;
            }
            let ctx = t.context();
            if self.unfillable(&ctx, &slots, &t.bound) {
                self.triggered.remove(0);
                self.ordered -= 1;
                continue;
            }
            self.decision = Some(Decision::Targets { player: t.controller, groups: slots.len(), ability: t, modes: Vec::new() });
            break;
        }
        placed
    }

    /// One legal group of targets per "target" of the text (601.2c).
    pub(crate) fn legal_targets(&self, ctx: &Ctx, slots: &[TargetSpec], targets: &[Vec<Target>]) -> Result<(), String> {
        self.legal_targets_with(ctx,slots,targets,&crate::program::expr::NoLocals)
    }
    pub(crate) fn legal_targets_with(&self, ctx: &Ctx, slots: &[TargetSpec], targets: &[Vec<Target>], locals: &dyn crate::program::expr::Locals) -> Result<(),String> {
        if targets.len() != slots.len() {
            return Err(format!("that has {} target group(s), {} given", slots.len(), targets.len()));
        }
        for (index, (slot, chosen)) in slots.iter().zip(targets).enumerate() {
            if slot.distinct_from.iter().any(|prior| *prior >= index
                || targets[*prior].iter().any(|target| chosen.contains(target))) {
                return Err("another target must differ from the preceding targets (115.1, 601.2c)".into());
            }
            let Quant::Target(_) = &slot.selector.quant else { continue };
            let (min, max) = slot.limits(self, ctx);
            let values=crate::announcement::TargetValues{parent:locals,chosen:targets,base:slot.base};
            if (chosen.len() as u32) < min || (chosen.len() as u32) > max {
                let legal = slot.set.targets(self, ctx, &values).len() as u32;
                // an Aura spell being cast (303.4a), not an ability of an Aura permanent
                let aura = ctx.source.is_some_and(|s| self.obj(s).ability.is_none() && self.obj(s).zone != Zone::Battlefield
                    && self.seen_for(s, crate::layers::Aspect::Types).subtypes.contains(&ir::Subtype::aura()));
                return Err(match legal {
                    _ if legal >= min => {
                        let wanted = if min == max { format!("{min}") } else if max == u32::MAX { format!("{min} or more") } else { format!("{min} to {max}") };
                        format!("wrong number of targets: {} chosen, it takes {wanted} (601.2c)", chosen.len())
                    }
                    0 if aura => "there is nothing it could enchant to target (303.4a, 601.2c)".into(),
                    0 => "there is no legal target for it (601.2c)".into(),
                    1 => format!("there is only 1 legal target, and it needs {min} (601.2c)"),
                    _ => format!("there are only {legal} legal targets, and it needs {min} (601.2c)"),
                });
            }
            for (i, t) in chosen.iter().enumerate() {
                if chosen[..i].contains(t) {
                    return Err("the same target twice for one \"target\" (115.3)".into());
                }
                if !slot.set.legal_target(self, ctx, &values, *t) {
                    let named = match *t {
                        // an emblem, like a face-down permanent, can have no name (114.3, 708.2a)
                        Target::Object(o) if !self.obj(o).chars.name.is_empty() => self.obj(o).chars.name.clone(),
                        Target::Object(o) if self.obj(o).emblem() => "an emblem".into(),
                        Target::Object(_) => "an object with no name".into(),
                        Target::Player(p) if p == ctx.controller => "you".into(),
                        Target::Player(_) => "that player".into(),
                    };
                    return Err(format!("an illegal target, {named}: {}", slot.set.why_not(self, ctx, &values, *t)));
                }
            }
        }
        for (slot, chosen) in slots.iter().zip(targets) {
            let values = crate::announcement::TargetValues { parent: locals, chosen: targets, base: slot.base };
            if !slot.group_legal(self, ctx, &values, chosen) {
                return Err(match slot.relation {
                    Some(crate::announcement::TargetRelation::DifferentCreatureControllers) =>
                        "two target creatures must have different controllers (601.2c)",
                    Some(crate::announcement::TargetRelation::DifferentManaValues) =>
                        "target cards must have different mana values (601.2c)",
                    Some(crate::announcement::TargetRelation::SameGraveyard) =>
                        "targets for one group must be cards in a single graveyard (601.2c, 404.1)",
                    None => unreachable!("a unary target group has no collective failure"),
                }.into());
            }
        }
        Ok(())
    }

    /// The division announced for these targets (601.2d), each divided group's amounts in the
    /// order its targets were chosen: what each gets, at least one, all of the amount among
    /// them. With one target it gets all of it, said or not; a group with no target gets none.
    pub(crate) fn legal_division(&self, ctx: &Ctx, slots: &[TargetSpec], targets: &[Vec<Target>], said: &[Vec<u32>]) -> Result<Vec<Vec<u32>>, String> {
        if said.len() > slots.len() {
            return Err(format!("a division for {} target group(s), and it has {} (601.2d)", said.len(), slots.len()));
        }
        let mut division = Vec::with_capacity(slots.len());
        let mut divides = false;
        for (index, (slot, chosen)) in slots.iter().zip(targets).enumerate() {
            let amounts = said.get(index).map_or(&[][..], Vec::as_slice);
            let Some(total) = slot.divided_total(self, ctx) else {
                if !amounts.is_empty() { return Err("those targets divide nothing (601.2d)".into()); }
                division.push(Vec::new());
                continue;
            };
            divides = true;
            let amounts = match (chosen.len(), amounts) {
                (0, []) => Vec::new(),
                (1, []) => vec![total],
                (_, []) => return Err(format!("divide {total} among the {} targets: say what each gets (601.2d)", chosen.len())),
                (n, given) if given.len() != n => return Err(format!("{} amount(s) for {n} targets: one for each (601.2d)", given.len())),
                (_, given) if given.contains(&0) => return Err("each target gets at least one of what is divided (601.2d)".into()),
                (_, given) if given.iter().map(|&n| u64::from(n)).sum::<u64>() != u64::from(total) =>
                    return Err(format!("the division adds up to {}, and {total} is divided (601.2d)", given.iter().map(|&n| u64::from(n)).sum::<u64>())),
                (_, given) => given.to_vec(),
            };
            division.push(amounts);
        }
        Ok(if divides { division } else { Vec::new() })
    }

    /// The definition of a card in the player's hand; elsewhere, why nothing lets them `verb`
    /// it from there.
    fn in_hand(&self, player: PlayerId, card: ObjId, cast_as: CastAs, verb: &str) -> ActionResult<&CardDef> {
        if !self.players[player].hand.contains(&card) {
            return Err(self.not_permitted(player, card, cast_as, verb).into());
        }
        self.permitted_def(card)
    }

    /// The definition of a card an effect lets a player play from where it is.
    fn permitted_def(&self, card: ObjId) -> ActionResult<&CardDef> {
        let def = self.def(card).ok_or("that is not a card")?;
        check(def).map_err(|u| crate::Unsupported(format!("the engine cannot run {}: {}", def.name, u.0)))?;
        Ok(def)
    }

    /// A main phase of the player's own turn with the stack empty: when a land is played
    /// (305.1) and a spell that is not an instant is cast (117.1a, 505.6a).
    pub(crate) fn sorcery_timing(&self, player: PlayerId) -> bool {
        player == self.active && self.stack.is_empty() && self.part.main()
    }

    /// Can this player play this card as a land now (305.1–305.2)?
    pub(crate) fn can_play_land_as(&self, player: PlayerId, card: ObjId, face: Option<usize>) -> ActionResult<()> {
        // from the hand, or from exile where an effect lets them play it; a face-down card is
        // refused as unsupported only where something would let it be played
        let def = match self.permitted_from_exile(player, card) {
            Some(true) => { self.not_hidden(card)?; self.permitted_def(card)? }
            Some(false) => return Err("an effect lets you cast that card, not play it as a land".into()),
            None if self.top_land_allowed(player,card) => self.permitted_def(card)?,
            None => self.in_hand(player, card, CastAs::Whole, "play")?,
        };
        self.not_hidden(card)?;
        let def=match face {Some(face)=>def.modal_face(face)?,None=>def};
        if !def.is(CardType::Land) {
            return Err("that is not a land".into());
        }
        if !self.sorcery_timing(player) {
            return Err("a land is played in a main phase of your own turn with the stack empty (305.1)".into());
        }
        if self.players[player].land_played {
            return Err("you have played a land this turn (305.2)".into());
        }
        Ok(())
    }

    /// A card exiled face down is turned face up just before it is played (406.3a), which
    /// this engine does not do: such a card is not played.
    fn not_hidden(&self, card: ObjId) -> ActionResult<()> {
        if self.obj(card).zone == Zone::Exile && self.obj(card).hidden.is_some() {
            return Err(crate::Unsupported("playing a card exiled face down is not supported (406.3a)".into()).into());
        }
        Ok(())
    }

    fn play_land(&mut self, player: PlayerId, card: ObjId, face: Option<usize>) -> ActionResult<()> {
        self.can_play_land_as(player, card, face)?;
        let request = crate::entry::EntryRequest::card(card, player, Choices { transformed: face==Some(1), ..Default::default() }, false);
        let library_tops = self.library_tops();
        self.procedures.push(crate::procedure::Procedure::Land { player, result: None, library_tops });
        match self.prepare_entry_batch(vec![request], Vec::new())? {
            Ok(result) => { self.deliver_operation(result); self.go_on()?; }
            Err(decision) => self.decision = Some(decision),
        }
        Ok(())
    }

    /// The rules restriction of legendary instants and sorceries (205.4e), before costs.
    pub(crate) fn legendary_cast_allowed(&self, player: PlayerId, def: &CardDef) -> bool {
        !((def.is(CardType::Instant) || def.is(CardType::Sorcery)) && def.supertypes.contains(&Supertype::Legendary))
            || self.battlefield.iter().any(|object| {
                let seen = self.seen_for(*object, crate::layers::Aspect::Types);
                self.controller(*object) == player && seen.supertypes.contains(&Supertype::Legendary)
                    && (seen.is(CardType::Creature) || seen.is(CardType::Planeswalker))
            })
    }

    /// Casting timing, independent of the card's payable cost and target choices. Suspend
    /// asks exactly this before its special action (116.2f, 702.62c).
    pub(crate) fn cast_has_instant_timing(&self, player: PlayerId, card: ObjId, cast_as: CastAs, x: Option<u32>) -> bool {
        let Some(def) = self.def(card).and_then(|def| def.spell(cast_as).ok()) else { return false };
        if def.is(CardType::Instant) && !def.is(CardType::Sorcery)
            || crate::keyword::of(def).any(|keyword| keyword.name() == "Flash")
            || self.casting_as_though_flash(player, card, cast_as, x) { return true; }
        let ctx = Ctx { casting: Some((card, cast_as)), x, ..Ctx::for_spell(player, card) };
        def.characteristics.iter().any(|row| row.result.as_ref().is_ok_and(|statement|
            statement.self_flash(self, &ctx.clone().with_ability(row.ability))))
    }

    /// Can this player begin to cast this card now with these modes chosen (601.2, 117.1a,
    /// 601.2b)? Everything but the choice of its targets and the payment of its cost, which
    /// are returned: what its text targets, in order (601.2c), of a modal spell what the modes
    /// chosen target (700.2c), and its total cost (601.2f).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn castable(&self, player: PlayerId, card: ObjId, cast_as: CastAs, route: CastRoute, chosen: crate::announcement::CastCost, modes: &[usize], x: Option<u32>, kicks: u32, buyback: bool, optional: bool, entwine: bool, replicate: u32, gift: Option<PlayerId>) -> ActionResult<(Vec<TargetSpec>, crate::cost::Plan)> {
        // cast for its overload cost, it is announced with its changed text, before its
        // targets would be (702.96a, 601.2b–c)
        let cast_as = self.cast_as_for(card, cast_as, chosen)?;
        // Cast face down with a morph or disguise ability: the card is turned face down
        // before anything is asked about the spell, so prohibitions, timing and permissions
        // see only the face-down spell (708.4, 702.37c, 702.168b). It has no text: nothing
        // else is announced for it.
        if let crate::announcement::CastCost::Alternative(row) = chosen && !self.face_down(card)
            && let Some((by, _)) = self.face_down_cast(card, row)
        {
            let (_, def) = self.physical_card(card).expect("a card casts face down");
            check(def).map_err(|u| crate::Unsupported(format!("the engine cannot run {}: {}", def.name, u.0)))?;
            if cast_as != CastAs::Whole || !modes.is_empty() || x.is_some() || kicks > 0 || buyback || optional || entwine || replicate > 0 || gift.is_some() {
                return Err("a face-down spell has no text: nothing else is announced for it (702.37c, 702.168b)".into());
            }
            if !matches!(route, CastRoute::Normal | CastRoute::Permission { .. }) {
                return Err("only one alternative cost is paid for a spell (118.9a)".into());
            }
            let mut view = self.clone();
            view.obj_mut(card).face = crate::face::Face::Down(crate::face::Down { by });
            view.refresh_face(card);
            return view.castable(player, card, cast_as, route, chosen, modes, x, kicks, buyback, optional, entwine, replicate, gift);
        }
        // Cast bestowed, it is an Aura spell as it is announced: only what the bestow
        // ability makes of it is asked whether it can be cast (702.103b, 702.103d).
        if let crate::announcement::CastCost::Alternative(row) = chosen && !self.obj(card).bestowed
            && self.def(card).and_then(|whole| whole.spell(cast_as).ok()).is_some_and(|def| Self::bestow_row(def, row))
        {
            let mut view = self.clone();
            view.obj_mut(card).bestowed = true;
            return view.castable(player, card, cast_as, route, chosen, modes, x, kicks, buyback, optional, entwine, replicate, gift);
        }
        if self.obj(card).bestowed && !matches!(chosen, crate::announcement::CastCost::Alternative(row)
            if self.def(card).and_then(|whole| whole.spell(cast_as).ok()).is_some_and(|def| Self::bestow_row(def, row))) {
            return Err("a spell is cast bestowed only for its bestow cost (702.103a)".into());
        }
        // A selected half's own hard zone restriction also overrides external permissions
        // (101.2, 601.3); face-down characteristics have already been applied above.
        if let Some(zone) = self.def(card).and_then(|whole| whole.spell(cast_as).ok()).and_then(|def|
            crate::keyword::contributions(def).find_map(|(_, gives)| match gives {
                crate::keyword::Gives::CastOnlyFrom(zone) => Some(*zone), _ => None,
            })) && self.obj(card).zone != zone {
            return Err("that spell can't be cast from its current zone (601.3)".into());
        }
        // Restrictions override zone permissions before any announcement mutates the card
        // or pays its cost (101.2, 601.3).
        if self.cast_forbidden(player, card, cast_as, x) {
            return Err("an effect prohibits casting that spell (601.3)".into());
        }
        // a foretold card is turned face up just before it is announced (406.3a): what it
        // could be cast as is what it is face up
        if route == CastRoute::Foretell && self.obj(card).hidden.is_some() {
            let view = self.with_foretold_face_up(player, card).ok_or("that card cannot be cast as a foretold card (702.143a)")?;
            return view.castable(player, card, cast_as, route, chosen, modes, x, kicks, buyback, optional, entwine, replicate, gift);
        }
        // From the hand, a graveyard with its own permission (113.6f) or flashback, or exile
        // where an effect lets them play it. A face-down card is refused as unsupported only
        // where something would let it be cast: otherwise for what refuses any card there.
        // Flashback changes the cost and departure destination; ordinary permission does not.
        let flashback = route == CastRoute::Flashback;
        let flashback_costs = if flashback { self.flashback_costs(player, card, cast_as) } else { Vec::new() };
        let def = match route {
            CastRoute::Escape if self.escape_of(player, card, cast_as) => self.def(card).ok_or("not a card")?,
            CastRoute::Escape => return Err("that card cannot be cast using escape (702.138a)".into()),
            CastRoute::Harmonize { tapped } if self.harmonize_of(player, card, cast_as) => {
                if tapped.is_some_and(|c| !self.harmonizers(player).contains(&c)) {
                    return Err("the creature tapped for harmonize is an untapped creature you control (702.180b)".into());
                }
                self.def(card).ok_or("not a card")?
            }
            CastRoute::Harmonize { .. } => return Err("that card cannot be cast using harmonize (702.180a)".into()),
            CastRoute::Foretell if self.foretold_of(player, card, cast_as) => self.def(card).ok_or("not a card")?,
            CastRoute::Foretell => return Err("only a card you foretold is cast for its foretell cost, after the turn it was foretold (702.143a)".into()),
            CastRoute::Permission { source, ability } if self.casting_permission(player,card,cast_as,x,source,ability).is_some() => { self.not_hidden(card)?; self.def(card).ok_or("not a card")? }
            CastRoute::Permission { .. } => return Err("that casting permission is unavailable (601.3)".into()),
            CastRoute::Flashback if !flashback_costs.is_empty() => self.def(card).ok_or("not a card")?,
            CastRoute::Flashback => return Err("that card cannot be cast using flashback (702.34a)".into()),
            CastRoute::Retrace if self.retrace_of(player, card, cast_as) => self.def(card).ok_or("not a card")?,
            CastRoute::Retrace => return Err("that card cannot be cast using retrace (702.81a)".into()),
            CastRoute::Normal if self.command_cast_allowed(player, card) => self.def(card).ok_or("not a card")?,
            CastRoute::Normal if self.permitted_from_graveyard(player, card, cast_as) => self.def(card).ok_or("not a card")?,
            CastRoute::Normal if self.mayhem_of(player, card, cast_as, Some(chosen)) => self.def(card).ok_or("not a card")?,
            CastRoute::Normal if self.prepared_copy_controller(card) == Some(player) => { self.not_hidden(card)?; self.permitted_def(card)? }
            CastRoute::Normal if self.permitted_cast_from_exile(player, card, cast_as) => { self.not_hidden(card)?; self.permitted_def(card)? }
            CastRoute::Normal => self.in_hand(player, card, cast_as, "cast")?,
        };
        self.not_hidden(card)?;
        if cast_as == CastAs::Fused && self.obj(card).zone != Zone::Hand {
            return Err("fuse applies only when cast from the hand (702.102a)".into());
        }
        let def = def.spell(cast_as)?;
        if def.is(CardType::Land) {
            return Err("a land is played, not cast (305.9)".into());
        }
        if self.casting_requires_sorcery_timing(player)
            && !(self.sorcery_timing(player) && self.priority == player && self.decision.is_none() && self.procedures.is_empty()) {
            return Err("an effect permits casting spells only at sorcery timing (307.5, 601.3)".into());
        }
        // from exile, what the effect letting it be cast says: when, and for what
        use crate::permission::{Costs, Window};
        let permit = self.cast_permission(player, card, cast_as, chosen)?;
        let offered = permit.is_some_and(|p| p.window == Window::Offered);
        let costs = permit.map_or(Costs::Ordinary, |p| p.costs);
        let ceiling = permit.and_then(|p| p.ceiling);
        let library_life=self.top_life_cost(route);
        let free = costs == Costs::Free || library_life;
        if free && (flashback || matches!(route, CastRoute::Escape | CastRoute::Foretell | CastRoute::Harmonize { .. })) {
            return Err("only one alternative cost is paid for a spell (118.9a)".into());
        }
        // an instant any time its player has priority, anything else in a main phase of their
        // turn with the stack empty (117.1a), unless it has flash (702.8a); a spell an effect
        // has its player cast as it is applied ignores that timing (608.2g)
        // a sneak cost is paid any time its player could cast an instant during their declare
        // blockers step, and only then (702.190a)
        let sneak = matches!(chosen, crate::announcement::CastCost::Alternative(row) if Self::sneak_row(def, row));
        if sneak && !self.sneak_timing(player) {
            return Err("a sneak cost is paid only during your declare blockers step (702.190a)".into());
        }
        // 601.3d checks a conditional self-grant before costs or mana abilities;
        // the granting static itself still functions only on the stack (113.6e).
        if !offered && !sneak && !self.sorcery_timing(player) && !self.cast_has_instant_timing(player, card, cast_as, x) {
            return Err("that is cast in a main phase of your own turn with the stack empty (117.1a)".into());
        }
        // a legendary instant or sorcery is cast only by a player who controls a legendary
        // creature or a legendary planeswalker (205.4e)
        if !self.legendary_cast_allowed(player, def) {
            return Err("a legendary instant or sorcery is cast only by a player who controls a legendary creature or planeswalker (205.4e)".into());
        }
        // the cost it is cast for (601.2b, 601.2f): its mana cost, its flashback cost from a
        // graveyard, an alternative cost its card prints, or one an effect offers it for
        // (madness, 702.35a); never two alternatives (118.9a)
        use crate::announcement::CastCost;
        fn found(costs: &[(usize, crate::Res<crate::cost::Plan>)], row: usize) -> Option<&crate::Res<crate::cost::Plan>> {
            costs.iter().find(|(at, _)| *at == row).map(|(_, plan)| plan)
        }
        let cost = match chosen {
            CastCost::Mana | CastCost::Flashback(_) if flashback => {
                let index = match chosen {
                    CastCost::Flashback(index) => index,
                    _ if flashback_costs.len() == 1 => 0,
                    _ => return Err("multiple flashback costs are available; say which flashback cost is paid (113.2c, 601.2b)".into()),
                };
                let cost = flashback_costs.get(index).ok_or("that flashback cost is unavailable (702.34a)")?
                    .as_ref().ok_or("that card has no mana cost (202.1b)")?;
                // Retain the definition-time plan for a printed cost. Newly granted
                // prices use the existing given-cost compiler, including additional costs.
                if crate::keyword::flashback(def).as_ref() == Some(cost) {
                    def.costs.flashback.as_ref().expect("a printed flashback cost has a compiled plan").clone()?
                } else { def.costs.given(def, cost)? }
            },
            CastCost::Flashback(_) => return Err("a flashback cost is selected only with the flashback route (702.34a)".into()),
            CastCost::Mana if free && def.costs.normal.is_none() => crate::cost::Plan::default().kicked(&ir::mana::ManaCost(Vec::new()),1).plus(def.costs.additional.as_ref().map_err(Clone::clone)?),
            CastCost::Mana => if route == CastRoute::Escape { def.costs.escape.as_ref() } else if matches!(route, CastRoute::Harmonize { .. }) { def.costs.harmonize.as_ref() } else if route == CastRoute::Foretell { def.costs.foretell.as_ref() } else { def.costs.normal.as_ref() }
                .ok_or("that card has no mana cost (202.1b)")?.clone()?,
            CastCost::Alternative(_) if flashback || matches!(route, CastRoute::Escape | CastRoute::Foretell | CastRoute::Harmonize { .. }) || free => return Err("only one alternative cost is paid for a spell (118.9a)".into()),
            CastCost::Alternative(row) if self.face_down(card) => self.face_down_cast(card, row)
                .ok_or("that card prints no such ability to cast it face down (702.37c, 702.168b)")?.1.clone()?,
            CastCost::Alternative(row) if matches!(costs, Costs::Only(_)) => found(&def.costs.offered, row)
                .ok_or("that card states no such cost an effect offers (702.35a)")?.clone()?,
            CastCost::Alternative(row) if found(&def.costs.offered, row).is_some() => {
                let rule = if crate::miracle::instances(def).any(|(at, _, _)| at == row) { "702.94a" } else { "702.35a" };
                return Err(format!("that cost is paid only for a cast its ability offers ({rule})").into());
            },
            // a warp cost an effect gives a card in a hand (613.1f, 702.185a)
            CastCost::Alternative(row) if def.costs.alternatives.iter().all(|(at, _)| *at != row) && self.obj(card).zone == Zone::Hand
                && self.given_alternatives(card, cast_as).iter().any(|(at, _)| *at == row) =>
                self.given_alternatives(card, cast_as).into_iter().find(|(at, _)| *at == row).expect("found").1?,
            CastCost::Alternative(row) => {
                let alternative = def.costs.alternatives.iter().find(|(at, _)| *at == row)
                    .ok_or("that card prints no such alternative cost (118.9)")?.1.as_ref().map_err(Clone::clone)?;
                // a warp cost is paid only for a spell cast from a hand (702.185a), a mayhem cost
                // from a graveyard (702.187b)
                match crate::keyword::alternative_from(def, row) {
                    Some(Zone::Hand) if self.obj(card).zone != Zone::Hand => return Err("that alternative cost is paid only for a spell cast from a hand (702.185a)".into()),
                    Some(Zone::Graveyard) if self.obj(card).zone != Zone::Graveyard => return Err("a mayhem cost is paid only for a card cast from a graveyard (702.187b)".into()),
                    _ => {}
                }
                let ctx = Ctx { casting: Some((card, cast_as)), x, ..Ctx::for_spell(player, card).with_ability(row) };
                if let Some(condition) = &alternative.condition { condition.validate_cast_choice_history(self, &ctx)?; }
                if alternative.condition.as_ref().is_some_and(|c| !c.evaluate(self, &ctx, &crate::program::expr::NoLocals)) {
                    return Err("the condition for that alternative cost is not true (118.9, 601.2b)".into());
                }
                // Every branch produces the same ordinary locked payment plan.
                alternative.plan.clone()
            },
        };
        if ceiling.is_some_and(|bound| def.spell_mana_value(x) > bound) {
            return Err("the resulting spell's mana value exceeds the effect's bound (701.57a, 702.85a)".into());
        }
        // the creature chosen for harmonize, tapped as the cost is paid (702.180b)
        let cost = match route { CastRoute::Harmonize { tapped: Some(creature) } => cost.tapping(creature), _ => cost };
        // "without paying its mana cost": its other costs are paid, and X is 0 (107.3b, 118.9)
        let cost = if free {
            let mana_x = def.cost.as_ref().is_some_and(|mana| mana.0.contains(&crate::cost::X));
            if mana_x && x.is_some_and(|x| x != 0) { return Err("X is 0 for a spell cast without paying its mana cost (107.3b)".into()); }
            // An X only in an additional life cost is still chosen and paid (118.9d).
            let x = if mana_x { Some(0) } else { x };
            {let cost=cost.announced(x)?.without_mana_cost();if library_life {cost.with_life(def.mana_value())}else{cost}}
        } else { cost.announced(x)? };
        // a kicker cost its controller declares they will pay is an additional cost, paid
        // whatever is paid for the mana cost (601.2b, 702.33a, 118.9d)
        let cost = match crate::keyword::kicker(def) {
            Some((_, false)) if kicks > 1 => return Err("a kicker cost can be paid only once (702.33a)".into()),
            Some(_) if kicks > MOST_KICKS => return Err("no payment could be made for that many kicker costs (601.2h)".into()),
            Some((kicker, _)) => cost.kicked(kicker, kicks),
            None if kicks > 0 => return Err("that spell has no kicker cost to pay (702.33)".into()),
            None => cost,
        };
        let cost = if buyback {
            let extra = crate::keyword::buyback(def).ok_or("that spell has no buyback cost (702.27a)")?;
            cost.kicked(extra, 1)
        } else { cost };
        // its optional additional cost, announced paid: bargain's sacrifice of an artifact,
        // enchantment or token (702.166a), offspring's or casualty's cost (702.175a, 702.153a),
        // or the one its text prints (118.8b)
        let cost = if optional {
            let (_, _, plan) = def.costs.optional.first().ok_or("that spell has no optional additional cost (118.8b)")?;
            cost.plus(plan.as_ref().map_err(Clone::clone)?)
        } else { cost };
        let cost = if route == CastRoute::Retrace { cost.plus(crate::keyword::retrace_cost()) } else { cost };
        let cost = match crate::keyword::replicate(def) {
            Some(_) if replicate > MOST_KICKS => return Err("too many replicate payments (601.2h)".into()),
            Some(mana) => cost.kicked(mana, replicate),
            None if replicate > 0 => return Err("that spell has no replicate cost (702.56a)".into()),
            None => cost,
        };
        let cost = if entwine {
            let extra = crate::keyword::entwine(def).ok_or("that spell has no entwine cost (702.42)")?;
            cost.kicked(extra, 1)
        } else { cost };
        // the costs of the modes chosen (700.2h; Spree 702.172a, Tiered 702.183a), and escalate's
        // for each mode chosen beyond the first (702.120a): additional costs (601.2f)
        let rows = match crate::program::spell_modal(def)? { Some(i) => def.modes_of(i), None => &[] };
        let cost = crate::modes::costs(rows, modes).fold(cost, |cost, mode| cost.kicked(mode, 1));
        let cost = match &def.costs.escalate {
            Some(extra) => {
                let extra = extra.as_ref().map_err(Clone::clone)?;
                (1..modes.len()).fold(cost, |cost, _| cost.plus(extra))
            }
            None => cost,
        };
        let mut slots = Vec::new();
        // an Aura spell's target, of a bestowed one what enchant creature describes (702.103b)
        if self.obj(card).bestowed { slots.extend(self.enchant_spec(card).cloned()); }
        else if let Some(spec) = def.enchant.clone()? { slots.push(spec); }
        if def.is(CardType::Instant) || def.is(CardType::Sorcery) {
            for (_, programs) in crate::program::spell_programs(def, modes, crate::program::ModeCount { entwined: entwine, kicked: kicks > 0, optional })? {
                for program in programs {
                    let ctx = Ctx { casting: Some((card, cast_as)), x, ..Ctx::for_spell(player, card) };
                    program.check_partition_quantities(self, &ctx)?;
                    append_targets(&mut slots, program.targets.clone());
                }
            }
        } else if !modes.is_empty() {
            return Err("that spell is not modal: no mode is chosen (700.2)".into());
        }
        // its gift, promised to an opponent still in the game: an additional cost, chosen as it
        // is announced (601.2b, 702.174a)
        if let Some(opponent) = gift {
            if crate::keyword::gift(def).is_none() { return Err("that spell has no gift to promise (702.174a)".into()); }
            if opponent == player || self.players.get(opponent).is_none_or(|p| p.lost) {
                return Err("a gift is promised to an opponent (702.174a)".into());
            }
        }
        // targets its text has only as it was kicked, as its gift was promised, or as it was
        // bargained, are chosen only then (702.33g, 702.174m, 702.166d)
        for slot in &mut slots { slot.announce(kicks, gift.is_some(), optional); }
        Ok((slots, cost))
    }

    /// The selected characteristics are considered before choosing a zone permission
    /// (601.3e, 715.3). Rare alternative faces need a private view; the normal path borrows.
    pub(crate) fn cast_seen(&self, card: ObjId, cast_as: CastAs) -> Option<crate::layers::Seen> {
        if cast_as == CastAs::Whole { return Some(self.seen_for(card, crate::layers::Aspect::Abilities)); }
        let chars = self.def(card)?.spell(cast_as).ok()?.chars();
        let mut view = self.clone();
        view.obj_mut(card).cast_as = cast_as;
        view.obj_mut(card).chars = chars;
        Some(view.seen_for(card, crate::layers::Aspect::Abilities))
    }

    pub(crate) fn retrace_of(&self, player: PlayerId, card: ObjId, cast_as: CastAs) -> bool {
        self.players[player].graveyard.contains(&card) && cast_as != CastAs::Fused
            && self.def(card).is_some_and(|d| check(d).is_ok())
            && self.cast_seen(card, cast_as).is_some_and(|s| s.has(ir::Keyword::from_name("Retrace").unwrap()))
    }

    pub(crate) fn escape_of(&self, player: PlayerId, card: ObjId, cast_as: CastAs) -> bool {
        self.players[player].graveyard.contains(&card) && cast_as != CastAs::Fused
            && self.def(card).is_some_and(|d| check(d).is_ok() && d.spell(cast_as).is_ok_and(|d| d.costs.escape.is_some()))
            && self.cast_seen(card, cast_as).is_some_and(|s| s.base_abilities)
    }

    /// A card in this player's graveyard with harmonize, an instant or sorcery the engine runs
    /// (702.180a).
    pub(crate) fn harmonize_of(&self, player: PlayerId, card: ObjId, cast_as: CastAs) -> bool {
        self.players[player].graveyard.contains(&card) && cast_as != CastAs::Fused
            && self.def(card).is_some_and(|d| check(d).is_ok() && d.spell(cast_as).is_ok_and(|d| d.costs.harmonize.is_some()))
            && self.cast_seen(card, cast_as).is_some_and(|s| s.base_abilities)
    }

    /// The untapped creatures this player controls, any one of which they may choose to tap
    /// for harmonize (702.180b).
    pub(crate) fn harmonizers(&self, player: PlayerId) -> Vec<ObjId> {
        self.battlefield.iter().copied().filter(|c| self.controller(*c) == player && !self.obj(*c).tapped && self.is_a(*c, CardType::Creature)).collect()
    }


    /// A card's own static zone permission (113.6f). Normal timing and all costs are
    /// still checked by `castable`; a graveyard card's "you" is its owner (109.5).
    /// A card in this player's graveyard that they discarded this turn, cast for its mayhem
    /// cost (702.187b): for this cost, or for any of its mayhem costs.
    pub(crate) fn mayhem_of(&self, player: PlayerId, card: ObjId, cast_as: CastAs, chosen: Option<crate::announcement::CastCost>) -> bool {
        if !self.players[player].graveyard.contains(&card) || cast_as == CastAs::Fused || !self.discarded_this_turn(card) { return false; }
        let Some(whole) = self.def(card) else { return false };
        let Ok(def) = whole.spell(cast_as) else { return false };
        let rows = crate::keyword::alternative_rows(def, ir::Keyword::from_name("Mayhem").expect("a keyword"));
        let mayhem = match chosen {
            Some(crate::announcement::CastCost::Alternative(row)) => rows.contains(&row),
            Some(crate::announcement::CastCost::Mana | crate::announcement::CastCost::Flashback(_)) => false,
            None => !rows.is_empty(),
        };
        mayhem && check(whole).is_ok()
    }

    pub(crate) fn permitted_from_graveyard(&self, player: PlayerId, card: ObjId, cast_as: CastAs) -> bool {
        if !self.players[player].graveyard.contains(&card) || cast_as == CastAs::Fused { return false; }
        let Some(whole) = self.def(card) else { return false };
        let Ok(def) = whole.spell(cast_as) else { return false };
        def.cast_from(Zone::Graveyard)
            && check(whole).is_ok()
    }

    /// Effective costs of a card in its owner's graveyard (702.34a, 113.2c). Equal prices have
    /// the same casting behavior. None is an actual, unpayable mana cost, not {0}.
    pub(crate) fn flashback_costs(&self, player: PlayerId, card: ObjId, cast_as: CastAs) -> Vec<Option<ir::Cost>> {
        if !self.players[player].graveyard.contains(&card) || cast_as == CastAs::Fused { return Vec::new(); }
        let Some(whole) = self.def(card) else { return Vec::new() };
        let Ok(def) = whole.spell(cast_as) else { return Vec::new() };
        if !(def.is(CardType::Instant) || def.is(CardType::Sorcery)) || check(whole).is_err() { return Vec::new(); }
        let Some(seen) = self.cast_seen(card, cast_as) else { return Vec::new() };
        let mut costs = Vec::new();
        for held in &seen.keywords {
            for (scope, gives) in held.contributions() {
                let crate::keyword::Gives::Route { route: CastRoute::Flashback, cost } = gives else { continue };
                if !scope.contains(Zone::Graveyard) { continue; }
                let cost = match cost {
                    crate::keyword::RouteCost::Alternative(cost) => Some(cost.clone()),
                    crate::keyword::RouteCost::ManaCost => def.cost().cloned().map(|mana| ir::Cost(vec![ir::CostPart::Mana(mana)])),
                    crate::keyword::RouteCost::Additional => continue,
                };
                if !costs.contains(&cost) { costs.push(cost); }
            }
        }
        costs
    }

    /// Announce the supplied spell before producing mana (601.2a–g). `face_down`:
    /// it is cast face down, and turned face down before it is put on the stack (708.4).
    pub(crate) fn announce_spell(&mut self, player: PlayerId, card: ObjId, cast_as: CastAs, route: CastRoute, modes: &[usize], targets: Vec<Vec<Target>>, x: Option<u32>, kicks: u32, buyback: bool, optional: bool, entwine: bool, face_down: Option<crate::face::Concealment>) -> ActionResult<ObjId> {
        let flashback = route == CastRoute::Flashback || matches!(route, CastRoute::Harmonize { .. });
        let def = self.def(card).ok_or("not a card")?.spell(cast_as)?;
        let departure = if route == CastRoute::Flashback {
            Some((crate::functions::Scope::In(Zone::Stack), Zone::Exile))
        } else {
            crate::keyword::contributions(def).find_map(|(scope, gives)| match gives {
                crate::keyword::Gives::Destination { when: crate::keyword::Departure::Any, to, .. } if flashback => Some((scope, *to)),
                crate::keyword::Gives::Destination { when: crate::keyword::Departure::AnyIfCastFrom(from), to, .. }
                    if self.obj(card).zone == *from => Some((scope, *to)),
                _ => None,
            })
        };
        let mut chars = def.chars();
        // While a spell is on the stack, each {X} of its mana cost is the announced value
        // (107.3a, 202.3e).
        chars.mana_value = def.spell_mana_value(x);
        // cast under an effect's offer: what that offer says of the spell cast this way
        let rider = if route == CastRoute::Normal { self.offered_rider(player, card) } else { None };
        // a spell cast bestowed is a bestowed Aura spell as it is put onto the stack (702.103b)
        let bestowed = self.obj(card).bestowed;
        let spell = self.move_to(card, Zone::Stack, player).ok_or("the card did not reach the stack")?;
        self.obj_mut(spell).bestowed = bestowed;
        self.obj_mut(spell).cast_rider = rider;
        self.obj_mut(spell).cast_as = cast_as;
        self.obj_mut(spell).chars = chars;
        if let Some(by) = face_down {
            self.obj_mut(spell).face = crate::face::Face::Down(crate::face::Down { by });
            self.refresh_face(spell);
        }
        self.obj_mut(spell).targets = targets;
        let mut modes = modes.to_vec();
        modes.sort_unstable();
        self.obj_mut(spell).modes = modes;
        self.obj_mut(spell).flashback = departure;
        self.obj_mut(spell).harmonized = match route { CastRoute::Harmonize { tapped } => tapped, _ => None };
        self.obj_mut(spell).x = x;
        self.obj_mut(spell).kicked = kicks;
        self.obj_mut(spell).buyback_paid = buyback;
        // its controller declared its optional additional cost paid: it was bargained (702.166b),
        // its offspring or casualty cost was paid (702.175a, 702.153a)
        self.obj_mut(spell).optional_paid = optional;
        self.obj_mut(spell).entwined = entwine;
        self.obj_mut(spell).cast_by = Some(player);
        self.obj_mut(spell).cast_announcement = Some(Box::new(crate::state::CastAnnouncement { route, flashback_cost: None, permission: None, payment: None, given_alternative: None, suspend_haste: false }));
        self.capture_spell(spell);
        self.targeting_events(spell);
        Ok(spell)
    }

    #[allow(clippy::too_many_arguments)]
    fn cast(&mut self, player: PlayerId, card: ObjId, cast_as: CastAs, route: Option<CastRoute>, substitutions: &Substitutions, cost: crate::announcement::CastCost, modes: &[usize], targets: Vec<Vec<Target>>, division: &[Vec<u32>], paying: &[Vec<ObjId>], with: Option<&Pool>, plan: &[crate::legal::Step], x: Option<u32>, kicks: u32, buyback: bool, optional: bool, entwine: bool, replicate: u32, gift: Option<PlayerId>, life: &[bool], chose: &[usize], branches: Option<&[crate::cost::BranchSelection]>) -> ActionResult<()> {
        // A complete proposal is validated on a copy, including dependent mana payments.
        // Refusing it cannot leave half an announcement or its costs in the game (733.1).
        let mut proposed = self.clone();
        proposed.begin_cast(player, card, cast_as, route, substitutions, cost, modes, targets, division, paying, with, plan, x, kicks, buyback, optional, entwine, replicate, gift, life, chose, branches, false)?;
        proposed.continue_execution()?;
        proposed.expose_payments();
        self.adopt(proposed);
        Ok(())
    }

    /// A player answers an effect's offer to cast one of these cards by casting it (608.2g):
    /// the announcement and its payment are a child of the resolution, which goes on when
    /// the spell has been cast. No player receives priority.
    fn cast_during(&mut self, player: PlayerId, cards: &[ObjId], mv: Move) -> ActionResult<()> {
        let said = Said::read(mv)?;
                said.validate_optional_costs(self)?;
                let Said { with, adding, modes, cast_as, plan, paying, x, kicks, buyback, optional, entwine, replicate, life, chose, branches, cost, route, substitutions, gift, division, what, .. } = said;
        let Move::Cast { card, targets } = what else {
            return Err("a decision is pending: cast one of the cards offered, or decline (608.2g)".into());
        };
        if !adding.is_empty() { return Err("mana is added by a mana ability, not a spell (605.3b)".into()); }
        if !cards.contains(&card) { return Err("the effect offers other cards to cast (608.2g)".into()); }
        self.cast_begun();
        self.begin_cast(player, card, cast_as.unwrap_or_default(), route, &substitutions, cost.unwrap_or_default(), &modes, targets, &division, paying.as_deref().unwrap_or(&[]), with.as_ref(), plan.as_deref().unwrap_or(&[]), x, kicks, buyback, optional, entwine, replicate, gift, &life, &chose, branches.as_deref(), true).map_err(Into::into)
        // the offer stands until its effect goes on: a cast abandoned is offered again (733.1)
    }

    /// Announce the spell and put its payment on top (601.2a–f). `reports`: it is cast while
    /// an effect is applied, and its payment reports to that resolution.
    #[allow(clippy::too_many_arguments)]
    fn begin_cast(&mut self, player: PlayerId, card: ObjId, cast_as: CastAs, route: Option<CastRoute>, substitutions: &Substitutions, chosen: crate::announcement::CastCost, modes: &[usize], targets: Vec<Vec<Target>>, division: &[Vec<u32>], paying: &[Vec<ObjId>], with: Option<&Pool>, plan: &[crate::legal::Step], x: Option<u32>, kicks: u32, buyback: bool, optional: bool, entwine: bool, replicate: u32, gift: Option<PlayerId>, life: &[bool], chose: &[usize], branches: Option<&[crate::cost::BranchSelection]>, reports: bool) -> ActionResult<()> {
        let proposed = self;
        let route = route.unwrap_or(CastRoute::Normal);
        // what it is cast as for its overload cost (702.96a)
        let cast_as = proposed.cast_as_for(card, cast_as, chosen)?;
        // what goes back if the cast is reversed: the card as it was, face down if foretold
        let original = proposed.obj(card).clone();
        // a foretold card is turned face up just before it is announced (406.3a), and is
        // what it is face up for every check of the proposal; a refused proposal leaves it
        // face down, as does a reversed cast through `original`
        let turned = route == CastRoute::Foretell && original.hidden.is_some();
        if turned { proposed.turn_foretold_face_up(card); }
        // cast bestowed, it is a bestowed Aura spell for every check of the proposal (702.103b,
        // 702.103d); a refused proposal leaves it as it was, as does a reversed cast
        let bestowed = matches!(chosen, crate::announcement::CastCost::Alternative(row)
            if proposed.def(card).and_then(|whole| whole.spell(cast_as).ok()).is_some_and(|def| Self::bestow_row(def, row)));
        if bestowed { proposed.obj_mut(card).bestowed = true; }
        let checked = (|| -> ActionResult<_> {
            let (slots, cost) = proposed.castable(player, card, cast_as, route, chosen, modes, x, kicks, buyback, optional, entwine, replicate, gift)?;
            // which alternative of each choice its cost has is paid, and how each Phyrexian
            // symbol is, are said with the proposal and locked in (601.2b, 118.13a)
            let branch_ctx=Ctx {casting:Some((card,cast_as)),..Ctx::for_spell(player,card)};
            let chose=cost.resolve_branches(proposed, &branch_ctx, chose, branches)?;
            let chose=chose.as_slice();
            let announced = cost.said_chosen(proposed, &branch_ctx, chose);
            let cost = cost.chosen(chose)?.paid_with_life(life)?;
            // X is 0 where the mana cost is not paid (107.3b)
            let free = proposed.top_life_cost(route) || proposed.cast_permission(player, card, cast_as, chosen).ok().flatten().is_some_and(|p| p.costs == crate::permission::Costs::Free);
            let x = if free &&proposed.def(card).and_then(|d| d.spell(cast_as).ok()).and_then(|d| d.cost.as_ref()).is_some_and(|c| c.0.contains(&crate::cost::X)) { Some(0) } else { x };
            let ctx = Ctx { casting: Some((card, cast_as)), x, ..Ctx::for_spell(player, card) };
            proposed.legal_targets(&ctx, &slots, &targets)?;
            let cost = cost.with_targets(proposed, &targets);
            let division = proposed.legal_division(&ctx, &slots, &targets, division)?;
            Ok((cost, x, division, announced))
        })();
        let (cost, x, division, announced) = match checked {
            Ok(checked) => checked,
            Err(e) => {
                if turned || bestowed { *proposed.obj_mut(card) = original; }
                return Err(e);
            }
        };
        let library_tops=proposed.library_tops();
        let list = match original.zone {
            Zone::Library => &proposed.players[original.owner].library,
            Zone::Hand => &proposed.players[original.owner].hand,
            Zone::Graveyard => &proposed.players[original.owner].graveyard,
            Zone::Exile => &proposed.exile,
            Zone::Command => &proposed.command,
            _ => unreachable!("castable checks the supported origin zone"),
        };
        let undo = crate::payment::Undo::Spell { original: card, object: Box::new(original),
            position: list.iter().position(|id| *id == card).expect("card is in its zone") };
        let triggers = proposed.triggered.len();
        let permission = if let CastRoute::Permission {source,ability}=route { proposed.casting_permission(player,card,cast_as,x,source,ability) } else {None};
        let permission_owner = permission.as_ref().map(|usage| (usage.clone(), proposed.frozen_definition(usage.source, &usage.key.origin)));
        let face_down = match chosen { crate::announcement::CastCost::Alternative(row) => proposed.face_down_cast(card, row).map(|(by, _)| by), _ => None };
        // a warp cost an effect gives the card in its hand (613.1f), read while it is there
        let given = match chosen {
            crate::announcement::CastCost::Alternative(row) => proposed.given_alternative(card, cast_as, row),
            _ => None,
        };
        let suspend_haste = proposed.cast_permission(player, card, cast_as, chosen).ok().flatten().is_some_and(|permission| permission.haste);
        let spell = proposed.announce_spell(player, card, cast_as, route, modes, targets, x, kicks, buyback, optional, entwine, face_down)?;
        proposed.obj_mut(spell).cast_announcement.as_mut().unwrap().suspend_haste = suspend_haste;
        proposed.obj_mut(spell).cast_announcement.as_mut().unwrap().flashback_cost = match chosen {
            crate::announcement::CastCost::Flashback(index) => Some(index), _ => None,
        };
        proposed.obj_mut(spell).cast_announcement.as_mut().unwrap().permission = permission_owner;
        proposed.obj_mut(spell).cast_announcement.as_mut().unwrap().given_alternative = given.map(std::sync::Arc::new);
        // the opponent its gift is promised to, chosen as an additional cost (702.174a)
        proposed.obj_mut(spell).gift = gift;
        proposed.obj_mut(spell).announced = announced;
        proposed.obj_mut(spell).division = division;
        proposed.obj_mut(spell).alternative = match chosen { crate::announcement::CastCost::Alternative(row) => Some(row), _ => None };
        proposed.obj_mut(spell).replicated = replicate;
        proposed.check_triggers();
        let triggers = triggers..proposed.triggered.len();
        let (delve, convoke, improvise) = (substitutions.delve, substitutions.convoke.as_slice(), substitutions.improvise);
        let cost = proposed.modified_cast_cost(spell, cost);
        if delve > 0 && !proposed.has(spell, "Delve") { return Err("that spell has no delve payment (702.66)".into()); }
        // one card for each generic mana of the total cost, as it is after reductions (702.66a-b)
        if delve > cost.max_generic_payment() { return Err(format!("delve exiles at most {} card(s), one for each generic mana of the total cost (702.66a)", cost.max_generic_payment()).into()); }
        let cost = cost.delved(delve, proposed.delve_origin(spell));
        if !convoke.is_empty() && !proposed.has(spell,"Convoke") {return Err("that spell has no convoke (702.51)".into());}
        let cost = cost.convoked(convoke)?;
        if improvise > 0 && !proposed.has(spell, "Improvise") { return Err("that spell has no improvise (702.126)".into()); }
        if improvise > 0 && cost.has_zone_cost() { return Err(crate::Unsupported("improvise combined with a zone-changing object cost requires independent resource kinds".into()).into()); }
        let cost = cost.improvised(improvise)?;
        let payment = crate::payment::Payment::spell(player, spell, cost, plan, paying, with, undo, triggers).permission(permission).library_tops(library_tops);
        proposed.procedures.push(crate::procedure::Procedure::Payment(if reports { payment.reporting() } else { payment }));
        Ok(())
    }

    /// Priority passes on; when everyone has passed in succession the top of the stack
    /// resolves, or with an empty stack the step or phase ends (117.4, 500.2).
    fn pass(&mut self) -> ActionResult<()> {
        self.passes += 1;
        let alive: Vec<PlayerId> = (0..self.players.len()).filter(|p| !self.players[*p].lost).collect();
        if self.passes < alive.len() {
            let n = self.players.len();
            self.priority = (1..=n).map(|i| (self.priority + i) % n).find(|p| alive.contains(p)).unwrap_or(self.priority);
        } else if let Some(top) = self.stack.last().copied() {
            self.resolve_top(top)?;
            self.passes = 0;
            self.priority_from(self.active);
        } else {
            self.end_part()?;
        }
        Ok(())
    }

    /// Capture immutable spell executables at announcement, after choosing its face/modes.
    pub(crate) fn capture_spell(&mut self, top: ObjId) {
        let o = self.obj(top);
        let invocations = self.def(top).and_then(|def| {
            (def.is(CardType::Instant) || def.is(CardType::Sorcery)).then(|| {
                let mut invocations = Vec::new();
                for (index, body) in crate::program::spell_bodies(def)? {
                    let modes = if matches!(body, crate::program::Body::Modes { .. }) { o.modes.as_slice() } else { &[] };
                    // A spell put on the stack without an announcement has X of 0 (107.3b).
                    let x = o.x.or(def.spell_x().then_some(0));
                    invocations.extend(crate::program::capture(self, Ctx { x, ..Ctx::for_spell(o.controller, top).with_ability(index) })?.select_spell(modes, crate::program::ModeCount { entwined: o.entwined, kicked: o.kicked > 0, optional: o.optional_paid })?);
                }
                Ok(invocations)
            })
        });
        self.obj_mut(top).invocations = invocations;
    }

    /// Resolve the top of the stack (608): a permanent spell enters the battlefield (608.3a);
    /// an instant, a sorcery or an ability has its instructions followed (608.2), or does
    /// nothing if every target has become illegal (608.2b).
    fn resolve_top(&mut self, top: ObjId) -> ActionResult<()> {
        let controller = self.controller(top);
        let Some(invocations) = self.obj(top).invocations.clone() else {
            let mut chosen = Choices::default();
            // a face-down spell enters as the face-down permanent it was (702.37c, 708.4)
            if let crate::face::Face::Down(down) = self.obj(top).face { chosen.face_down = Some(down.by); }
            if self.enters_attached(top) {
                chosen.attached_to = self.obj(top).targets.first().and_then(|t| t.first()).copied();
                let legal = chosen.attached_to.is_some_and(|t| self.can_enchant(top, controller, t)
                    && self.can_target(&Ctx::for_spell(controller, top), t));
                if !legal && self.obj(top).bestowed {
                    // a bestowed Aura spell whose target is illegal ceases to be bestowed and
                    // goes on resolving as a creature spell (702.103e, 608.3b)
                    self.cease_bestowed(top);
                    chosen.attached_to = None;
                } else if !legal {
                    self.log.push(Event::Fizzled { spell: top });
                    self.leave_stack(top);
                    return Ok(());
                }
            }
            // its sneak cost was paid: tapped and attacking (702.190b)
            self.sneak_entry(top, &mut chosen);
            let mut at = Resolving::new(top, self.obj(top).bound.clone());
            at.execution = Some(crate::program::Resolution::permanent(self, top, chosen));
            self.start_resolution(at);
            self.go_on()?;
            return Ok(());
        };
        let invocations = invocations.expect("announced abilities have compiled executables");
        // Intervening conditions belong to the captured ability too (603.4), not the
        // ability now occupying that printed index on a changed source.
        if invocations.iter().any(|i| i.intervening.as_ref().is_some_and(|c|
            !c.evaluate(self, &i.ctx, &self.obj(top).bound))) {
            self.log.push(Event::Fizzled { spell: top });
            self.leave_stack(top);
            return Ok(());
        }
        let (targets, bound) = (self.obj(top).targets.clone(), self.obj(top).bound.clone());
        let mut base=0;
        let mut any_legal=false;
        for invocation in &invocations {
            let values=crate::announcement::TargetValues{parent:&bound,chosen:&targets,base};
            for (index,slot) in invocation.program.targets.iter().enumerate() {
                any_legal |= targets[base+index].iter().any(|t|slot.legal_target(self,&invocation.ctx,&values,&targets[base+index],*t));
            }
            base+=invocation.program.targets.len();
        }
        // "up to one target": a spell or ability no target was chosen for has none that is
        // illegal, and resolves (608.2b)
        let targeted = targets.iter().any(|chosen| !chosen.is_empty());
        if targeted && !any_legal {
            self.log.push(Event::Fizzled { spell: top });
            self.leave_stack(top);
        } else {
            let mut at = Resolving::new(top, bound);
            at.execution = Some(crate::program::Resolution::new(invocations, &at.bound));
            self.start_resolution(at);
            self.go_on()?;
        }
        Ok(())
    }

    /// Follow the instructions of what is resolving, from where they stopped, until they are
    /// done or one waits for a player's choice (608.2c–d).
    pub(crate) fn go_on(&mut self) -> ActionResult<()> {
        self.continue_execution().map(|_|())
    }

    /// Drive the same retained execution for a stack occurrence or an immediately resolving
    /// mana occurrence. Only completion determines where priority returns (117.3b, 605.3b).
    pub(crate) fn continue_execution(&mut self) -> ActionResult<usize> {
        let progress = self.drive_procedures()?;
        match progress.payment_error {
            Some(reason) => Err(ActionError::Refused(reason)),
            None => Ok(progress.unused_mana_answers),
        }
    }

    pub(crate) fn drive_procedures(&mut self) -> crate::Res<crate::procedure::Progress> {
        use crate::procedure::Procedure;
        let mut progress = crate::procedure::Progress::default();
        while self.decision.is_none() {
            match self.procedures.last() {
                None => return Ok(progress),
                Some(Procedure::Opening(_)) => if let Err(error)=self.advance_opening_actions() {
                    match error { ActionError::Unsupported(reason)=>return Err(reason), refused=>progress.payment_error=Some(refused.to_string()) }
                    return Ok(progress);
                },
                Some(Procedure::AttackDeclaration(_)) => self.advance_attack_declaration(),
                Some(Procedure::Payment(_)) => if let Err(error) = self.advance_payment() {
                    match error {
                        ActionError::Unsupported(reason) => return Err(reason),
                        refused => progress.payment_error = Some(refused.to_string()),
                    }
                    return Ok(progress);
                },
                Some(Procedure::Operation(_)) => {
                    match self.complete_operation()? {
                        Ok(result) => self.deliver_operation(result),
                        Err(decision) => self.decision = Some(decision),
                    }
                }
                Some(Procedure::Cleanup { .. }) => {
                    assert!(matches!(self.procedures.pop(), Some(Procedure::Cleanup { complete: true })));
                    self.finish_cleanup().map_err(|error| match error {
                        ActionError::Unsupported(reason) => reason,
                        refused => crate::Unsupported(refused.to_string()),
                    })?;
                }
                Some(Procedure::RuleAction { .. }) => {
                    assert!(matches!(self.procedures.pop(), Some(Procedure::RuleAction { complete: true })));
                }
                Some(Procedure::Land { .. }) => {
                    let Some(Procedure::Land { player, result: Some(result), .. }) = self.procedures.pop() else {
                        unreachable!("land resumes after its child returns");
                    };
                    let land = *result.arrived.first().expect("a played land enters");
                    self.players[player].land_played = true;
                    self.log.push(Event::LandPlayed { land, by: player });
                }
                Some(Procedure::Returns { .. }) => {
                    self.finish_returns();
                    self.check_triggers();
                }
                Some(Procedure::StateBased(_)) => {
                    let Some(Procedure::StateBased(state)) = self.procedures.pop() else { unreachable!() };
                    progress.state_based |= self.advance_state_based(state);
                }
                Some(Procedure::Resolution(_)) => {
                    (progress.unused_mana_answers, progress.unoffered_mana_answer) = self.advance_resolution()?;
                    self.payment_scope=None;
                    if progress.unused_mana_answers > 0 {
                        if let Some(payment) = self.procedures.iter().rev().find_map(|p|
                            if let Procedure::Payment(payment) = p { Some(payment) } else { None })
                        {
                            if !payment.interactive {
                                progress.payment_error = Some("that mana ability offers no such choice of mana (608.2d)".into());
                                return Ok(progress);
                            }
                            // A replacement answer can eliminate a planned mana choice.
                            // It was never made; finish the actual child, not its stale plan.
                            progress.unused_mana_answers = 0;
                        }
                    }
                }
            }
        }
        Ok(progress)
    }

    /// The mana answers left unused, and whether one said was not among the mana offered.
    fn advance_resolution(&mut self) -> crate::Res<(usize, bool)> {
        let Some(crate::procedure::Procedure::Resolution(mut at)) = self.procedures.pop() else {
            unreachable!("only the top procedure advances");
        };
        self.payment_scope=at.payment_scope.clone();
        let parent = self.procedures.len();
        let top = at.object;
        let (targets, division) = match at.completion {
            crate::state::Completion::Stack => (self.obj(top).targets.clone(), self.obj(top).division.clone()),
            crate::state::Completion::Mana { .. } | crate::state::Completion::Opening => (Vec::new(), Vec::new()),
        };
        loop {
            if let Some(destination) = &mut at.destination {
                if let Some(decision) = destination.advance(self, &mut at.public_choices) {
                    self.decision = Some(decision);
                    self.procedures.insert(parent, crate::procedure::Procedure::Resolution(at));
                    return Ok((0, false));
                }
                if !at.public_choices.is_empty() { self.completed_choices_mut().append(&mut at.public_choices); }
                self.priority_from(self.active);
                return Ok((0, false));
            }
            let mut execution = at.execution.take().expect("resolution has an execution plan");
            let result = execution.advance(self, &targets, &division, &mut at);
            at.execution = Some(execution);
            match result? {
                // The child it started is above it and runs next; it reports back here.
                Some(crate::program::Paused::Child) => {
                    self.procedures.insert(parent, crate::procedure::Procedure::Resolution(at));
                    return Ok((0, false));
                }
                Some(crate::program::Paused::Ask(decision)) => {
                    let mut unoffered = false;
                    if let (crate::state::Completion::Mana { player, answers, .. }, Decision::Choose { player: chooser, choice: Choice::Mana { options } }) = (&mut at.completion, &decision)
                        && player == chooser
                        && let Some(answer) = answers.pop_front()
                    {
                        // mana in a pool has no order: said in any order, it is the same mana
                        if let Some(option) = options.iter().find(|o| crate::mana::same_mana(o, &answer)) {
                            at.answer(Answer::Mana(option.clone()));
                            continue;
                        }
                        // Mana a proposed plan says that the ability never offered refuses
                        // the proposal (605.3b) rather than being set aside for a question.
                        let proposed = self.procedures.iter().rev().find_map(|p|
                            if let crate::procedure::Procedure::Payment(payment) = p { Some(!payment.interactive) } else { None });
                        self.audit_mana_answer(top,"unoffered");
                        if proposed == Some(true) { return Ok((answers.len() + 1, true)); }
                        // A saved plan is not permission to apply an answer that became
                        // illegal after a replacement choice. Keep the real request pending.
                        answers.clear();
                        unoffered = true;
                    }
                    if !unoffered && matches!(decision,Decision::Choose{choice:Choice::Mana{..},..}) {self.audit_mana_answer(top,"missing");}
                    self.decision = Some(decision);
                    self.procedures.insert(parent, crate::procedure::Procedure::Resolution(at));
                    return Ok((0, unoffered));
                }
                None => match at.completion {
                    crate::state::Completion::Stack => {
                        self.log.push(Event::Resolved { spell: top });
                        // "becomes crewed": a crew ability of the Vehicle resolved (702.122e)
                        if let (Some((Some(vehicle), _)), Some(by)) = (self.obj(top).ability, self.obj(top).bound.crew.clone()) {
                            self.log.push(Event::BecameCrewed { vehicle, by });
                        }
                        // a madness trigger that moved its card on without casting it (400.7k)
                        if let Some((Some(card), index)) = self.obj(top).ability
                            && let Some(now) = at.execution.as_ref().and_then(|e| e.moved(card)) {
                            self.madness_resolved(card, index, now);
                        }
                        if self.obj(top).ability.is_some() || self.obj(top).zone != Zone::Stack {
                            self.leave_stack(top);
                            self.priority_from(self.active);
                            return Ok((0, false));
                        }
                        let revealed = at.program_frame().map(|f| f.known_cards()).unwrap_or_default();
                        at.destination = Some(crate::completion::Destination::new(self, top, revealed));
                        continue;
                    }
                    // the opening actions go on (103.6)
                    crate::state::Completion::Opening => {
                        if let Some(crate::procedure::Procedure::Opening(actions)) = self.procedures.last_mut() { actions.completed(None); }
                        return Ok((0, false));
                    }
                    crate::state::Completion::Mana { source, player, taps_for_mana, answers, checked, more, triggered, activation } => {
                        if let Some(key)=activation {
                            let index=self.active_mana_abilities.iter().position(|(o,k)|*o==source && *k==key).expect("active mana ability owns completion");
                            self.active_mana_abilities.remove(index);
                        }
                        if taps_for_mana && at.execution.as_ref().expect("completed resolution").produced_mana(source) {
                            self.log.push(Event::TappedForMana { object: source, player });
                            // what it triggers that adds mana resolves now, above this
                            // payment (605.4a); any other trigger waits to be put on the stack
                            self.capture_triggers();
                        }
                        // whoever activated a mana ability keeps priority (117.3c); a triggered
                        // one it triggers changes nothing of that, whoever controls it
                        if !triggered { self.priority_from(player); }
                        // "If ~ has a luck counter on it, instead add one mana of any color."
                        // without one: the color said for it is of a choice not made
                        let mut rest = if checked { std::collections::VecDeque::new() } else { answers };
                        rest.extend(more);
                        // what is left is said for the triggered mana ability resolving next
                        if !rest.is_empty()
                            && let Some(crate::procedure::Procedure::Resolution(next)) = self.procedures.last_mut()
                            && let crate::state::Completion::Mana { triggered: true, answers, .. } = &mut next.completion
                        {
                            answers.extend(rest);
                            return Ok((0, false));
                        }
                        if !rest.is_empty() {self.audit_mana_answer(source,"unoffered");}
                        return Ok((rest.len(), false));
                    }
                }
            }
        }
    }

    /// The last part of resolving (608.2n): a spell is put into its owner's graveyard, an
    /// ability is removed from the stack and ceases to exist.
    fn leave_stack(&mut self, top: ObjId) {
        match self.obj(top).ability {
            Some(_) => self.ability_gone(top),
            // a spell its own effect has moved from the stack ("Exile Flood of
            // Recollection.") is where that put it (608.2m)
            None if self.obj(top).zone != Zone::Stack => {}
            None => {
                let controller = self.controller(top);
                self.move_to(top, Zone::Graveyard, controller);
            }
        }
    }
}

impl crate::retention::Trace for Move {
    fn trace(&self, visit: &mut dyn FnMut(ObjId)) {
        match self {
            Move::PlayLand(card) | Move::PlayLandAs{card,..} => card.trace(visit),
            Move::Activate { source, targets, paying, .. } => { source.trace(visit); targets.trace(visit); paying.trace(visit); },
            Move::Cast { card, targets } => { card.trace(visit); targets.trace(visit); },
            Move::Companion => {},
            Move::Plot(card) | Move::Foretell(card) | Move::Suspend(card) | Move::TurnFaceUp(card, _) | Move::Unlock { room: card, .. } => card.trace(visit),
            Move::ChooseTargets(targets) => targets.trace(visit),
            Move::Discard(cards) | Move::Bottom(cards) => cards.trace(visit),
            Move::Attack(attacks) => { for (source, target) in attacks { source.trace(visit); target.trace(visit); } },
            Move::Block(blocks) => { for (blocker, attacker) in blocks { blocker.trace(visit); attacker.trace(visit); } },
            Move::Divide(parts) => { for (target, _) in parts { target.trace(visit); } },
            Move::Choose(answer) => answer.trace(visit),
            Move::FinishPayment { plan, paying, with } => {
                plan.trace(visit);
                paying.trace(visit); with.trace(visit);
            },
            Move::Paying(pool, action) => { pool.trace(visit); action.trace(visit); },
            Move::AuditPayment(action) => action.trace(visit),
            Move::ContinueManaPayment { source, mana, .. } => { source.trace(visit); mana.trace(visit); },
            Move::WithMana(steps, action) => {
                steps.trace(visit);
                action.trace(visit);
            },
            Move::WithCosts(costs, action) => { costs.trace(visit); action.trace(visit); },
            Move::Adding(_, action) | Move::Modes(_, action) | Move::CastAs(_, action) | Move::WithX(_, action) | Move::WithReplicate(_, action) | Move::WithKicks(_, action) | Move::WithBuyback(_, action) | Move::WithOptional(_, action) | Move::WithEntwine(_, action) | Move::WithLife(_, action) | Move::WithCostChoice(_, action) | Move::WithCostBranches(_, action) | Move::CastFor(_, action) | Move::CastVia(_, action) | Move::WithDelve(_, action) | Move::WithConvoke(_, action) | Move::WithImprovise(_, action) | Move::GiftTo(_, action) | Move::WithDivision(_, action) => action.trace(visit),
            Move::ChooseModes(_) | Move::Order(_) | Move::Mulligan | Move::Keep | Move::Pass | Move::Concede | Move::ReversePayment => {},
        }
    }
}

// The check of refusals is a debug build's, and so are these tests of it.
#[cfg(all(test, debug_assertions))]
mod refusal_tests {
    use super::*;

    fn seated() -> Game {
        let mut game = Game::new(vec![CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap()], 2, 20);
        game.put(1, 0, Zone::Battlefield);
        game
    }

    #[test]
    fn a_refusal_that_changes_nothing_is_only_a_refusal() {
        let mut game = seated();
        let before = format!("{game:?}");
        assert_eq!(game.act_with(0, Move::Pass, |_, _, _| Err("refused".into())), Err("refused".into()));
        assert_eq!(format!("{game:?}"), before);
    }

    #[test]
    #[should_panic(expected = "a refused request changed the game (\"refused\")")]
    fn a_refusal_that_changes_the_game_is_caught() {
        seated().act_with(0, Move::Pass, |g, _, _| {
            *g.life_total_mut(1) -= 1;
            Err("refused".into())
        }).ok();
    }

    #[test]
    #[should_panic(expected = "a refused request changed the game (\"a decision is pending")]
    fn a_refusal_that_changes_only_a_pending_question_is_caught() {
        seated().act_with(0, Move::Pass, |g, _, _| {
            g.decision = Some(Decision::Discard { player: 0, count: 1 });
            Err("a decision is pending".into())
        }).ok();
    }

    /// An illegal total assignment returns to the moment before its player began to assign
    /// (510.1e): to that moment, and to no other.
    fn assigning() -> Game {
        let mut game = seated();
        let attacker = game.battlefield[0];
        game.decision = Some(Decision::Divide { player: 1, attacker, among: vec![Target::Player(0)], amount: 2 });
        game
    }

    fn division() -> Move { Move::Divide(vec![(Target::Player(0), 2)]) }

    #[test]
    fn r510_1e_a_restart_to_the_moment_the_player_began_assigning_is_its_refusal() {
        let mut game = assigning();
        let began = format!("{game:?}");
        assert_eq!(game.act_with(1, division(), |_, _, _| Err(ActionError::Restarted("again".into()))), Err("again".into()));
        assert_eq!(format!("{game:?}"), began);
    }

    #[test]
    #[should_panic(expected = "a refused request changed the game (\"again\")")]
    fn r510_1e_a_restart_to_another_moment_is_caught() {
        assigning().act_with(1, division(), |g, _, _| {
            *g.life_total_mut(0) -= 2;
            Err(ActionError::Restarted("again".into()))
        }).ok();
    }

    /// What the host was given or scoped to one response between the divisions is not part
    /// of the moment the player began at: a restart is checked without it.
    #[test]
    fn r510_1e_what_the_host_took_between_divisions_is_not_restarted() {
        let mut game = assigning();
        let bear = game.battlefield[0];
        game.disclose(crate::observation::Kind::Looked, 1, &[bear]);
        game.act_with(1, division(), |_, _, _| Ok(())).unwrap();
        assert_eq!(game.take_observations(1).len(), 1);
        game.replay_random = Some(Default::default());
        game.random.recorded_only(true);
        game.captured_random = Some(Vec::new());
        assert_eq!(game.act_with(1, division(), |_, _, _| Err(ActionError::Restarted("again".into()))), Err("again".into()));
    }

    /// Another player's concession while one assigns (104.3a) is not undone by a restart:
    /// the moment before they began is not one the game returns to, and is not checked.
    #[test]
    fn r510_1e_a_move_taken_while_a_player_assigns_ends_the_check() {
        let mut game = assigning();
        game.act_with(1, division(), |_, _, _| Ok(())).unwrap();
        game.act_with(0, Move::Concede, |g, _, _| { *g.life_total_mut(0) -= 2; Ok(()) }).unwrap();
        assert_eq!(game.act_with(1, division(), |_, _, _| Err(ActionError::Restarted("again".into()))), Err("again".into()));
    }
}

impl crate::retention::Players for Move {
    fn players(&self, visit: &mut dyn FnMut(PlayerId)) {
        match self {
            Move::Activate { targets, .. } | Move::Cast { targets, .. } | Move::ChooseTargets(targets) => targets.players(visit),
            Move::Attack(attacks) => { for (_, target) in attacks { target.players(visit); } },
            Move::Divide(parts) => { for (target, _) in parts { target.players(visit); } },
            Move::Choose(answer) => answer.players(visit),
            Move::GiftTo(player, action) => { visit(*player); action.players(visit); },
            Move::Paying(_, action) | Move::AuditPayment(action) | Move::WithMana(_, action) | Move::WithCosts(_, action) | Move::Adding(_, action) | Move::Modes(_, action) | Move::CastAs(_, action) | Move::WithX(_, action) | Move::WithReplicate(_, action) | Move::WithKicks(_, action) | Move::WithBuyback(_, action) | Move::WithOptional(_, action) | Move::WithEntwine(_, action) | Move::WithLife(_, action) | Move::WithCostChoice(_, action) | Move::WithCostBranches(_, action) | Move::CastFor(_, action) | Move::CastVia(_, action) | Move::WithDelve(_, action) | Move::WithConvoke(_, action) | Move::WithImprovise(_, action) | Move::WithDivision(_, action) => action.players(visit),
            Move::PlayLand(_) | Move::PlayLandAs { .. } | Move::Companion | Move::Plot(_) | Move::Foretell(_) | Move::Suspend(_) | Move::TurnFaceUp(..) | Move::Unlock { .. }
            | Move::Discard(_) | Move::Bottom(_) | Move::Block(_) | Move::FinishPayment { .. } | Move::ContinueManaPayment { .. }
            | Move::ChooseModes(_) | Move::Order(_) | Move::Mulligan | Move::Keep | Move::Pass | Move::Concede | Move::ReversePayment => {},
        }
    }
}

#[cfg(test)]
mod typed_refusal_tests {
    use super::*;
    use crate::simulation::Class;

    #[test]
    fn cached_cost_refusals_keep_their_class_through_casting() {
        // Controlled compiler refusals exercise the error boundary, not card admission.
        for alternative in [false, true] {
            let text = if alternative { "You may pay 1 life rather than pay this spell's mana cost.\nGain 1 life." }
                else { "Bargain\nGain 1 life." };
            let mut def = CardDef::new("Probe spell", "{0}", "Instant", text, None).unwrap();
            check(&def).unwrap();
            let refusal = crate::Unsupported("controlled cost lowering refusal".into());
            let row = if alternative {
                let (row, plan) = def.costs.alternatives.first_mut().expect("alternative cost");
                *plan = Err(refusal);
                *row
            } else {
                def.costs.optional.first_mut().expect("optional cost").2 = Err(refusal);
                0
            };
            let mut g = Game::new(vec![def], 2, 20);
            let card = g.put(0, 0, Zone::Hand);
            let action = Box::new(Move::Cast {card, targets:vec![]});
            let action = if alternative { Move::CastFor(crate::announcement::CastCost::Alternative(row), action) }
                else { Move::WithOptional(true, action) };
            let before = format!("{g:?}");
            let error = g.attempt(0, action).unwrap_err();
            assert_eq!(error.class(), Class::Unsupported);
            assert_eq!(error.to_string(), "controlled cost lowering refusal");
            assert_eq!(format!("{g:?}"), before);
        }
    }

    #[test]
    fn captured_modal_refusals_keep_their_class_and_pending_occurrence() {
        let mut g = Game::new(vec![], 2, 20);
        let ability = Triggered {source:None, controller:0, origin:crate::binding::Origin::Own,
            index:0, bound:Default::default(), delayed:None,
            captured:Some(Err(crate::Unsupported("controlled captured-mode refusal".into())))};
        g.triggered.push(ability.clone());
        g.decision = Some(Decision::Modes {player:0, ability});
        let before = format!("{g:?}");
        let error = g.attempt(0, Move::ChooseModes(vec![0])).unwrap_err();
        assert_eq!(error.class(), Class::Unsupported);
        assert_eq!(error.to_string(), "controlled captured-mode refusal");
        assert_eq!(format!("{g:?}"), before);
    }
}
