//! 702.190a: "Sneak [cost]" means "Any time you could cast an instant during your declare
//! blockers step, you may cast this spell by paying [cost] and returning an unblocked creature
//! you control to its owner's hand rather than paying this spell's mana cost." 702.190b: the
//! permanent spell whose sneak cost was paid enters tapped and attacking the same player,
//! planeswalker or battle as the creature returned to pay it.
use crate::{Game,ObjId,PlayerId};
use crate::card::CardDef;
impl Game {
    /// Is the alternative cost at this row of the definition a sneak cost (702.190a)?
    pub(crate) fn sneak_row(def:&CardDef,row:usize) -> bool {
        ir::Keyword::from_name("Sneak").is_some_and(|sneak| crate::keyword::alternative_rows(def,sneak).contains(&row))
    }
    /// When a sneak cost is paid: any time this player could cast an instant during their
    /// declare blockers step (702.190a).
    pub(crate) fn sneak_timing(&self,player:PlayerId) -> bool {
        player==self.active && self.part==ir::Part::DeclareBlockers
    }
    /// The spell became this permanent's entry: if its sneak cost was paid, tapped and
    /// attacking what the creature returned for it attacked (702.190b, 506.3a).
    pub(crate) fn sneak_entry(&self,spell:ObjId,chosen:&mut crate::replace::Choices) {
        let Some(row)=self.obj(spell).alternative else { return };
        if !self.def(spell).is_some_and(|def| Self::sneak_row(def,row)) { return; }
        chosen.arrivals.push(crate::replace::Enters::Tapped);
        // the returned creature is the one of the sneak cost's objects that was attacking
        chosen.attacking=self.obj(spell).bound.inputs.iter()
            .find_map(|(input,defenders)| matches!(input,crate::program::input::Input::CostDefender(_)).then(|| defenders.first().copied()).flatten());
    }
}
