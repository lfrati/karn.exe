//! Suspend's hand special action and two exile abilities (702.62). Costs use retained
//! payment, counter placement uses replacement ordering, and the last counter offers an
//! ordinary resolution-time cast. This first subset has fixed time and mana costs.
use crate::cost::Plan;
use crate::functions::Scope;
use crate::keyword::{Context, Declared, Gives};
use crate::operation::{CounterEvent, EventOrigin, Ordering};
use crate::state::{Answer, Pool};
use crate::{CardDef, Game, ObjId, PlayerId, Res, Zone, unsupported};
use ir::token::{KeywordInstance, KeywordParam};

#[derive(Debug, Clone)]
pub(crate) struct Suspension {
    pub count: u32,
    pub cost: Plan,
}

pub(crate) fn declare(item: &KeywordInstance) -> Res<Declared> {
    let KeywordParam::NumberAndMana(ir::Amount::Const(count), cost) = &item.param else {
        return unsupported("suspend: fixed time and mana cost required (702.62a)");
    };
    if cost.0.contains(&crate::cost::X) || !cost.0.iter().all(|symbol| symbol.mana_payments().is_some()) {
        return unsupported("suspend: variable or nonmana cost announcements (702.62a)");
    }
    Ok(Declared {
        gives: vec![
            (Scope::In(Zone::Hand), Gives::Suspend { count: *count, cost: cost.clone() }),
            (Scope::In(Zone::Exile), Gives::Asked),
            (Scope::In(Zone::Exile), Gives::Ability(triggers()[0].clone())),
            (Scope::In(Zone::Exile), Gives::Ability(triggers()[1].clone())),
        ],
        needs: vec![Context::PrintedOnly],
    })
}

fn triggers() -> &'static Vec<ir::Ability> {
    static ABILITIES: std::sync::OnceLock<Vec<ir::Ability>> = std::sync::OnceLock::new();
    crate::token::init(&ABILITIES, || CardDef::new("Suspend rule", "", "Enchantment",
        "At the beginning of your upkeep, if this card is suspended, remove a time counter from this card.\n\
        When the last time counter is removed from this card, if this card is exiled, you may cast this card without paying its mana cost.", None)
        .expect("the grammar reads the two triggered abilities of 702.62a").abilities.clone())
}

pub(crate) fn compile(def: &CardDef) -> Option<Res<Suspension>> {
    let abilities: Vec<_> = crate::keyword::contributions(def).filter_map(|(_, gives)| match gives {
        Gives::Suspend { count, cost } => Some((*count, cost)),
        _ => None,
    }).collect();
    let [(count, cost)] = abilities.as_slice() else {
        return (!abilities.is_empty()).then(|| unsupported("suspend: multiple abilities require a selected hand action"));
    };
    Some(Plan::compile(&ir::Cost(vec![ir::CostPart::Mana((*cost).clone())]),
        &crate::select::Support::definition(def, None)).map(|cost| Suspension { count: *count, cost }))
}

impl Game {
    /// Suspended is a designation of this exact exile incarnation (702.62b, 400.7).
    pub fn suspended(&self, card: ObjId) -> bool {
        let object = self.obj(card);
        object.zone == Zone::Exile && object.hidden.is_none() && !object.token() && object.ability.is_none()
            && self.has(card, "Suspend")
            && self.counters(card).get("time").is_some_and(|count| *count > 0)
    }

    pub(crate) fn suspend_cost(&self, player: PlayerId, card: ObjId) -> crate::play::ActionResult<Suspension> {
        if !self.players[player].hand.contains(&card) {
            return Err("suspend a card from your hand (702.62a)".into());
        }
        let def = self.def(card).ok_or("suspend needs a card")?;
        crate::play::check(def)?;
        let suspension = def.suspend.as_ref().ok_or("that card has no supported suspend ability")?.clone()?;
        if def.is(ir::CardType::Land) { return Err("a land is played, not cast (305.9, 702.62c)".into()); }
        if !self.legendary_cast_allowed(player, def) {
            return Err("a legendary instant or sorcery is cast only by a player who controls a legendary creature or planeswalker (205.4e, 702.62c)".into());
        }
        if self.cast_forbidden(player, card, crate::card::CastAs::Whole, None) {
            return Err("an effect prohibits casting that spell (601.3, 702.62c)".into());
        }
        if self.casting_requires_sorcery_timing(player) && !self.sorcery_timing(player) {
            return Err("an effect permits casting spells only at sorcery timing (307.5, 702.62c)".into());
        }
        if !self.sorcery_timing(player) && !self.cast_has_instant_timing(player, card, crate::card::CastAs::Whole, None) {
            return Err("suspend only when you could begin to cast that card (116.2f, 702.62a)".into());
        }
        Ok(suspension)
    }

    pub(crate) fn suspend(&mut self, player: PlayerId, card: ObjId,
        steps: &[crate::legal::Step], with: Option<&Pool>) -> crate::play::ActionResult<()> {
        let mut proposed = self.clone();
        let Suspension { count, cost } = proposed.suspend_cost(player, card)?;
        let triggers = proposed.triggered.len()..proposed.triggered.len();
        // 702.62a pays first: mana children see the original hand, and no public exile
        // incarnation exists until the special action's payment completes.
        let undo = crate::payment::Undo::HandAction;
        let library_tops = proposed.library_tops();
        proposed.procedures.push(crate::procedure::Procedure::Payment(crate::payment::Payment::suspend(
            player, card, count, cost, steps, with, undo, triggers,
        ).library_tops(library_tops)));
        proposed.continue_execution()?;
        proposed.expose_payments();
        self.adopt(proposed);
        Ok(())
    }
}

/// Counter replacement answers belong to the completed special-action payment. Resuming
/// this stage never replays the mana window or cost commitment (702.62a, 616.1).
#[derive(Debug, Clone)]
pub(crate) struct Placement {
    ordering: Ordering<CounterEvent>,
    answer: Option<Answer>,
    player: PlayerId,
}
impl Placement {
    pub(crate) fn new(game: &Game, player: PlayerId, card: ObjId, count: u32) -> Self {
        Self { ordering: Ordering::new(game, vec![CounterEvent::new(crate::operation::Placement {
            on: card.into(), kind: "time".into(), count, by: player,
        }, EventOrigin::Effect)]), answer: None, player }
    }
    pub(crate) fn answer(&mut self, answer: Answer) { self.answer = Some(answer); }
    pub(crate) fn advance(&mut self, game: &mut Game) -> crate::play::ActionResult<bool> {
        let decision = if let Some(answer) = self.answer.take() { self.ordering.answer(game, self.player, answer)? }
            else { self.ordering.advance(game)? };
        if let Some(decision) = decision { game.decision = Some(decision); return Ok(false); }
        self.ordering.clone().prepare_counters(game).commit(game)?;
        Ok(true)
    }
}

impl Game {
    /// The cast has completed, not merely been announced. Copies of its announcement do
    /// not create this effect; the existing spell-to-permanent transfer preserves it.
    pub(crate) fn suspend_cast_completed(&mut self, player: PlayerId, spell: ObjId) {
        if !self.obj(spell).cast_announcement.as_ref().is_some_and(|announcement| announcement.suspend_haste)
            || !self.seen_for(spell, crate::layers::Aspect::Types).is(ir::CardType::Creature) { return; }
        let haste = crate::keyword::Held::plain(ir::Keyword::from_name("Haste").expect("a keyword"));
        self.begin_effect_from(vec![crate::layers::Change::Gains(vec![haste])], vec![spell],
            crate::layers::Lifetime::Controlled { source: spell, player }, None);
    }
}
