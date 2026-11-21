//! 702.88: replacement at successful resolution, then a one-shot optional upkeep cast.
use crate::{Game,ObjId,Zone};
use crate::select::{Ctx,Support};
impl Game {
    pub(crate) fn rebound_applies(&self,spell:ObjId) -> bool {
        let object=self.obj(spell);
        object.zone==Zone::Stack && object.ability.is_none() && object.cast_from==Some(Zone::Hand)
            && object.owner==object.controller && self.seen_for(spell, crate::layers::Aspect::Abilities).keywords.iter().any(|held|
                held.contributions().any(|(scope, gives)| scope.contains(object.zone) && matches!(gives,
                    crate::keyword::Gives::Destination { when: crate::keyword::Departure::Resolution, to: Zone::Exile, then: Some(crate::keyword::Then::Rebound) })))
    }
    /// Only the selected Rebound replacement, after a successful exile, creates the permission.
    pub(crate) fn rebound_exiled(&mut self,spell:ObjId,card:ObjId) {
        let controller=self.obj(spell).controller;
        let ctx=Ctx::for_spell(controller,spell);
        let whose=ir::StepOwner::Player{who:Box::new(ir::Selector::you()),next:true};
        let pattern=crate::trigger::DelayedPattern::step(ir::Step::Upkeep,&whose,&Support::runtime(self,&ctx))
            .expect("rebound watches its controller's next upkeep");
        let captured=crate::program::rebound_cast(self,ctx,card);
        self.delayed.push(crate::state::Delayed{source:spell,controller,pattern,subjects:Vec::new(),
            captured,bound:Default::default(),lasts:crate::state::Lasts::Indefinite,once:true});
    }
}
