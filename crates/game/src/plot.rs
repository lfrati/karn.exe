//! Plot is a paid special action in hand, followed by a later-turn casting permission
//! (702.170). Its payment uses the same retained procedure as casting and activation.
use crate::cost::Plan;
use crate::state::Pool;
use crate::{CardDef, Game, ObjId, PlayerId, Res, unsupported};
use ir::{
    Ability,
    token::{KeywordInstance, KeywordParam},
};

pub(crate) fn cost(item: &KeywordInstance) -> Option<&ir::mana::ManaCost> {
    match &item.param {
        KeywordParam::Mana(cost) if item.keyword.name() == "Plot" => Some(cost),
        _ => None,
    }
}
pub(crate) fn compile(def: &CardDef) -> Option<Res<Plan>> {
    let costs: Vec<_> = def
        .abilities
        .iter()
        .flat_map(|a| match a {
            Ability::Keywords(items) => items.as_slice(),
            _ => &[],
        })
        .filter_map(cost)
        .collect();
    let [mana] = costs.as_slice() else {
        return (!costs.is_empty()).then(|| unsupported("plot: multiple costs need a selected ability"));
    };
    Some((|| {
        let plan = Plan::compile(
            &ir::Cost(vec![ir::CostPart::Mana((*mana).clone())]),
            &crate::select::Support::definition(def, None),
        )?;
        if plan.x() != 0 || !plan.phyrexian().is_empty() {
            return unsupported("plot: variable or Phyrexian cost announcements");
        }
        Ok(plan)
    })())
}
impl Game {
    pub(crate) fn plot_cost(&self, player: PlayerId, card: ObjId) -> crate::play::ActionResult<Plan> {
        if !self.sorcery_timing(player) {
            return Err("plot only in your main phase with an empty stack (702.170a)".into());
        }
        if !self.players[player].hand.contains(&card) {
            return Err("plot a card from your hand (702.170a)".into());
        }
        let def = self.def(card).ok_or("plot needs a card")?;
        crate::play::check(def)?;
        def.plot.as_ref().ok_or("that card has no supported plot ability")?.clone().map_err(Into::into)
    }
    pub(crate) fn plot(
        &mut self,
        player: PlayerId,
        card: ObjId,
        steps: &[crate::legal::Step],
        with: Option<&Pool>,
    ) -> crate::play::ActionResult<()> {
        let mut proposed = self.clone();
        let cost = proposed.plot_cost(player, card)?;
        let original = proposed.obj(card).clone();
        let position = proposed.players[player].hand.iter().position(|o| *o == card).expect("plot checked the hand");
        let triggers = proposed.triggered.len();
        let exiled = proposed.exile_result(card).ok_or("the plot card could not be exiled")?;
        proposed.check_triggers();
        let triggers = triggers..proposed.triggered.len();
        let undo = crate::payment::Undo::ExiledFromHand { original: card, object: Box::new(original), position };
        let library_tops = proposed.library_tops();
        proposed.procedures.push(crate::procedure::Procedure::Payment(crate::payment::Payment::plot(
            player, exiled, cost, steps, with, undo, triggers,
        ).library_tops(library_tops)));
        proposed.continue_execution()?;
        proposed.expose_payments();
        self.adopt(proposed);
        Ok(())
    }
}
