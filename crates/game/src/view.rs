//! What a player sees of the game (CR 400.2): the faces of the cards in the public zones, and
//! of the hidden zones their own hand alone. A program that plays a seat is given the game as
//! its player may know it, and nothing more.

use crate::state::{Choice, Decision, Game, ObjId, PlayerId, Zone};

impl Game {
    /// Whether a choice's unfiltered pool contains an identity hidden from another
    /// player. Public emptiness can skip a choice; private qualification cannot.
    pub(crate) fn concealed_pool(&self, chooser: PlayerId, cards: impl IntoIterator<Item = ObjId>) -> bool {
        cards.into_iter().any(|o| {
            let object = self.obj(o);
            (matches!(object.zone, Zone::Hand | Zone::Library)
                || object.zone == Zone::Exile && object.hidden.is_some())
                && self.players.iter().enumerate().any(|(seat, p)| seat != chooser && !p.lost && !self.sees(seat, o))
        })
    }
    /// What the "may" asked now lets its player do, when the instruction says (608.2d): the
    /// action and, for a zone change, where to ([`crate::program::Frame::optional`]).
    /// Whether the text says "return" or "put" of the objects a pending choice is of.
    pub(crate) fn moved_as(&self) -> Option<ir::action::MoveVerb> {
        if !matches!(self.decision, Some(Decision::Choose { .. })) { return None; }
        self.resolving()?.execution.as_ref()?.frame()?.moved_as()
    }

    /// Instruction-derived context for a whole private look-and-return ordering.
    pub fn library_arrangement(&self) -> Option<ObjId> {
        let Some(Decision::Choose { player, choice:Choice::OrderCards {cards,..} })=&self.decision else {return None};
        self.resolving()?.program_frame()?.library_arrangement(self,*player,cards)
    }

    pub fn optional(&self) -> Option<(ir::Verb, Option<crate::state::Placement>)> {
        if !matches!(self.decision, Some(Decision::Choose { choice: Choice::May { .. }, .. })) { return None; }
        // "As this land enters, you may pay 2 life" (614.12a): yes pays it
        if let Some(crate::replace::Asks::Life { .. }) = self.operation().and_then(|op| op.entry_asks()) {
            return Some((ir::Verb::PayLife, None));
        }
        self.resolving()?.execution.as_ref()?.frame()?.optional()
    }

    /// Which object the pending entry question belongs to (614.12a). This is context for
    /// the decision, not permission to see a hidden card's face; `sees` still governs that.
    pub fn entering_card(&self) -> Option<ObjId> {
        if !matches!(self.decision, Some(Decision::Choose { .. })) { return None; }
        self.operation().and_then(|op| op.entry_context()).and_then(|(_, card)| card)
            .or_else(|| self.resolving().and_then(|r| {
                r.execution.as_ref().and_then(|e| e.entering_card())
            }))
    }

    /// Identity reserved for the entry event whose replacement order is being chosen.
    /// It is not yet a battlefield object, including when it will be a token.
    pub fn entering_object(&self) -> Option<ObjId> {
        self.operation().filter(|op| op.waiting()).and_then(|op| op.entry_context()).map(|(on, _)| on)
    }

    /// Public answers made while collecting simultaneous choices (101.4b), including a
    /// one-shot return that interrupted resolution. No hidden search/order answers appear.
    pub fn public_choices(&self) -> impl Iterator<Item = &crate::state::PublicChoice> {
        self.resolving().into_iter().flat_map(|r| r.public_choices.iter())
            .chain(self.resolving().into_iter().filter_map(|r| r.program_frame()).flat_map(|f| f.choices.iter()))
            .chain(self.operations().flat_map(|op| op.public_choices()))
            .chain(self.procedures.iter().filter_map(|p| match p {
                crate::procedure::Procedure::StateBased(state) => Some(state.public_choices()), _ => None,
            }).flatten())
    }

    /// May this player see this object's face? In a public zone every player does (400.2:
    /// graveyard, battlefield, stack, exile, command). A hand is looked at by its owner alone, the
    /// others may only count its cards (402.3); a library by nobody, though anyone may count
    /// it (401.2, 401.3). A card an effect has revealed is shown to all players for as long
    /// as the effect needs it (701.20a); the cards a player looks at as they scry or surveil,
    /// by that player while they decide where they go (701.22a, 701.25a). Of an object that is
    /// no more, as where it last was.
    ///
    /// This is what may be looked at now. What a player was shown before is not asked here:
    /// it was written down for them as it was ([`crate::observation`]) and gives no
    /// permission to look again once the effect that showed it is over.
    ///
    /// A face-down permanent's face is seen by its controller alone (708.5), a card exiled
    /// face down by the players who may look at it (406.3); every player still
    /// [`Game::perceives`] the object itself.
    pub fn sees(&self, viewer: PlayerId, o: ObjId) -> bool {
        (0..self.players.len()).filter(|player| self.shares_information(viewer,*player))
            .any(|player| self.sees_directly(player,o))
    }
    /// May this player refer to this object at all: does it exist for them, whether or not
    /// they may look at its face? Every player perceives a face-down spell or permanent
    /// (708.2, 708.5) and a card exiled face down (406.3, 406.4); elsewhere this is
    /// [`Game::sees`].
    pub fn perceives(&self, viewer: PlayerId, o: ObjId) -> bool {
        let obj = self.obj(o);
        (matches!(obj.zone, Zone::Battlefield | Zone::Stack) && matches!(obj.face, crate::face::Face::Down(_)))
            // also once it has left: every player saw it leave, whatever it became
            || ((if obj.zone == Zone::Gone { obj.left } else { Some(obj.zone) }) == Some(Zone::Exile) && obj.hidden.is_some())
            || self.sees(viewer, o)
    }
    /// Current reveal grants owned by stack occurrences (602.2a, 702.49b).
    /// These are not historical disclosures; removing the occurrence ends its grant.
    pub(crate) fn reveal_grants(&self) -> impl Iterator<Item = ObjId> + '_ {
        self.stack.iter().flat_map(|ability| self.obj(*ability).bound.revealed.iter().copied())
    }

    fn sees_directly(&self, viewer: PlayerId, o: ObjId) -> bool {
        let obj = self.obj(o);
        if obj.zone==Zone::Library && self.continuous_top_look(viewer,o){return true;}
        if self.reveal_grants().any(|card| card == o) { return true; }
        if self.resolving().is_some_and(|r| r.program_frame().is_some_and(|f| f.shown_to(self, viewer, o))) {
            return true;
        }
        if let Some(Decision::Choose { player, choice: Choice::Arrange { cards, .. } | Choice::Search { cards, .. } }) = &self.decision
            && *player == viewer
            && cards.contains(&o)
        {
            return true;
        }
        match if obj.zone == Zone::Gone { obj.left } else { Some(obj.zone) } {
            // its controller may look at it at any time (708.5)
            Some(zone @ (Zone::Battlefield | Zone::Stack)) if obj.zone == zone && matches!(obj.face, crate::face::Face::Down(_)) => self.controller(o) == viewer,
            // only a player allowed to look at it, until it leaves exile (406.3)
            Some(Zone::Exile) if obj.hidden.is_some() => obj.hidden.as_ref().is_some_and(|h| h.lookers.contains(&viewer)),
            Some(Zone::Battlefield | Zone::Stack | Zone::Graveyard | Zone::Exile | Zone::Command) => true,
            Some(Zone::Hand) => obj.owner == viewer,
            Some(Zone::Library | Zone::Gone) | None => false,
        }
    }
}
