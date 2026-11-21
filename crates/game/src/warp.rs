//! 702.185a: a permanent spell cast for its warp cost makes, as it resolves, a delayed
//! triggered ability that exiles the permanent it became at the beginning of the next end step.
use crate::{Game,ObjId};
use crate::select::{Ctx,Support};
impl Game {
    /// Was one of this spell's warp costs paid (702.185a, 702.185c)? A copy of a spell cast
    /// for it has its cost paid too (707.10).
    pub(crate) fn warp_paid(&self,spell:ObjId) -> bool {
        ir::Keyword::from_name("Warp").is_some_and(|warp| self.paid_for(spell,warp))
    }
    /// Was this spell, or the permanent it became, cast for an alternative cost of this
    /// keyword (601.2b): "that creature was cast for its warp cost"? A copy of a spell, and
    /// the token it becomes, has the alternative cost of what it copies but was not cast
    /// (707.10): it has no caster.
    pub(crate) fn cast_for(&self,object:ObjId,keyword:ir::Keyword) -> bool {
        self.obj(object).cast_by.is_some() && self.paid_for(object,keyword)
    }
    /// Is this object's alternative cost, paid or copied, one of this keyword?
    fn paid_for(&self,object:ObjId,keyword:ir::Keyword) -> bool {
        let Some(row)=self.obj(object).alternative else { return false };
        // a row past those it prints is a warp cost an effect gave it ([`Game::given_alternatives`])
        let given=|def:&crate::CardDef| row>=given_base(def) && Some(keyword)==ir::Keyword::from_name("Warp");
        // its rows are those its card prints, whatever it now is: a face-down spell's
        // definition prints none, its morph cost being a row of its card (702.37c)
        self.underlying(object).is_some_and(|def| crate::keyword::alternative_rows(def,keyword).contains(&row) || given(def))
    }
    /// The warp costs effects give this card in a hand (613.1f: "Artifact cards and red
    /// creature cards in your hand have warp {2}{R}"), each by a row past those its card
    /// prints, with the plan it is cast for. A warp cost it prints is its own row, not one of
    /// these, though an effect give it the same one too.
    pub(crate) fn given_alternatives(&self,card:ObjId,cast_as:crate::card::CastAs) -> Vec<(usize,crate::Res<crate::cost::Plan>)> {
        self.given_warps(card,cast_as).into_iter().map(|(row,_,plan)| (row,plan)).collect()
    }
    /// The warp ability an effect gives this card in a hand whose cost is this row (613.1f):
    /// what a spell cast for that cost has created its delayed exile, as it was (702.185a).
    pub(crate) fn given_alternative(&self,card:ObjId,cast_as:crate::card::CastAs,row:usize) -> Option<ir::token::KeywordInstance> {
        self.given_warps(card,cast_as).into_iter().find(|(at,_,_)| *at==row).map(|(_,item,_)| item)
    }
    fn given_warps(&self,card:ObjId,cast_as:crate::card::CastAs) -> Vec<(usize,ir::token::KeywordInstance,crate::Res<crate::cost::Plan>)> {
        let (Some(def),Some(warp))=(self.def(card),ir::Keyword::from_name("Warp")) else { return Vec::new() };
        let Ok(spell)=def.spell(cast_as) else { return Vec::new() };
        if self.obj(card).zone!=crate::Zone::Hand { return Vec::new() }
        let Some(seen)=self.cast_seen(card,cast_as) else { return Vec::new() };
        let mut printed:Vec<&ir::token::KeywordInstance>=crate::keyword::of_items(spell).filter(|i| i.keyword==warp).collect();
        let mut out=Vec::new();
        for held in seen.keywords.iter().filter(|h| h.keyword==warp) {
            let item=ir::token::KeywordInstance{keyword:held.keyword,param:held.param.clone()};
            if let Some(i)=printed.iter().position(|p| **p==item) { printed.remove(i); continue; }
            let Some(cost)=crate::keyword::alternative_cost(&item) else { continue };
            let plan=spell.costs.given(spell,&cost);
            out.push((given_base(spell)+out.len(),item,plan));
        }
        out
    }
    /// The spell became this permanent: if its warp cost was paid, the delayed trigger that
    /// exiles it. Its static ability made it, not an enters trigger (702.185a, 603.7).
    pub(crate) fn warp_resolved(&mut self,spell:ObjId,permanent:ObjId) {
        if !self.warp_paid(spell) { return; }
        let controller=self.obj(spell).controller;
        let mut ctx=Ctx::for_spell(controller,spell);
        // The paid alternative is the declaring keyword row, not a guessed ability
        // index on the resulting permanent. Granted alternatives have no printed row.
        if let Some(row)=self.obj(spell).alternative {
            let origin=self.copy_origin(spell);
            if self.definition(spell,&origin).is_some_and(|def|
                ir::Keyword::from_name("Warp").is_some_and(|warp|
                    crate::keyword::alternative_rows(def,warp).contains(&row))) {
                ctx=ctx.with_key(crate::binding::AbilityKey{origin,index:row});
            }
        }
        let pattern=crate::trigger::DelayedPattern::step(ir::Step::End,&ir::StepOwner::The{next:true},&Support::runtime(self,&ctx))
            .expect("warp watches the next end step");
        let captured=crate::program::warp_exile(self,ctx,permanent);
        self.delayed.push(crate::state::Delayed{source:spell,controller,pattern,subjects:Vec::new(),
            captured,bound:Default::default(),lasts:crate::state::Lasts::Indefinite,once:true});
    }
}

/// The first row past every ability its card prints, its keyword abilities among them.
fn given_base(def:&crate::CardDef) -> usize { def.abilities().len()+def.keyword_abilities.len() }
