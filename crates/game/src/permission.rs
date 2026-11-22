//! Permission an effect gives to play a card from where it is.
//!
//! A permission says two things apart: when the card may be cast ([`Window`]) and for which
//! cost ([`Costs`]).
//!
//! "You may play it this turn": the permission is the effect; nothing is asked as it
//! resolves, and it lasts until the turn ends. Playing the card that way follows every other
//! rule of playing it: timing, costs, and a land is the player's land for the turn (305.2,
//! 601.3).
//!
//! "You may cast it without paying its mana cost", "you may cast it by paying {R} rather than
//! paying its mana cost" (madness, 702.35a): the card is cast while the effect is applied or
//! not at all (608.2g), for that cost only. That permission lasts only while the effect offers
//! it.
//!
//! Either is about one object: a card that leaves exile is a new object the permission does
//! not follow (400.7).
use crate::announcement::CastCost;
use crate::state::{Game, ObjId, PlayerId, Zone};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Permission {
    pub(crate) player: PlayerId,
    pub(crate) object: ObjId,
    /// "play": a land may be played too; "cast": only a spell cast.
    pub(crate) play: bool,
    pub(crate) window: Window,
    pub(crate) costs: Costs,
    /// 715.3d: normal characteristics only, for as long as this incarnation stays exiled.
    pub(crate) adventure: bool,
    /// Creature spells cast under this offer gain haste while continuously controlled (702.62a).
    pub(crate) haste: bool,
    /// Where the card is: exile, or a graveyard for an offer of "a spell from your graveyard".
    /// A card that leaves it is a new object the permission does not follow (400.7).
    pub(crate) zone: Zone,
    /// "If an instant or sorcery spell cast this way would be put into your graveyard, exile it
    /// instead": what a spell cast under this offer carries (614.1a).
    pub(crate) rider: Option<Rider>,
    /// Inclusive resulting-spell mana-value ceiling of this offer (701.57a, 702.85a).
    pub(crate) ceiling: Option<u32>,
    /// When an effect's permission ends (611.2a): the adventure and plot permissions have no
    /// stated deadline and say [`Until::EndOfTurn`] without being ended by it.
    pub(crate) until: Until,
}

/// The deadline a resolved permission states (611.2a), for the player it is given to: "until
/// end of turn" / "this turn" (514.2); "until the end of your next turn", which a turn of
/// theirs other than the one it began in ends; "until your next end step", "until the
/// beginning of your next upkeep", ended by the first such step of theirs to begin after it
/// began, perhaps later this same turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Until {
    EndOfTurn,
    EndOfYourNextTurn { began: u32 },
    YourNext(ir::Part),
}

/// A deadline as the text states it, before the turn it is put in force in is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deadline {
    EndOfTurn,
    EndOfYourNextTurn,
    YourNext(ir::Part),
    /// "after the current turn has ended for as long as it remains exiled": a warped card
    /// (702.185a).
    Warped,
}

impl Deadline {
    /// The deadline a lowered duration states, if a permission can have it: "this turn",
    /// "until end of turn", "until the end of your next turn", "until your next upkeep" / "end
    /// step" (also "the beginning of" them).
    pub(crate) fn compile(duration: &ir::effect::Duration) -> Option<Deadline> {
        use ir::effect::Duration;
        use ir::event::{Step, StepOwner, Time};
        match duration {
            Duration::ThisTurn => Some(Deadline::EndOfTurn),
            Duration::Until(Time { end: true, step: Step::Turn, whose: StepOwner::This }) => Some(Deadline::EndOfTurn),
            Duration::Until(Time { end, step, whose: StepOwner::Player { who, next: true } }) if crate::select::is_you(who) => match (end, step) {
                (true, Step::Turn) => Some(Deadline::EndOfYourNextTurn),
                (false, Step::Upkeep) => Some(Deadline::YourNext(ir::Part::Upkeep)),
                (false, Step::End) => Some(Deadline::YourNext(ir::Part::End)),
                _ => None,
            },
            _ => None,
        }
    }

    /// Put in force during this turn.
    pub(crate) fn from(self, turn: u32) -> Until {
        match self {
            Deadline::EndOfTurn => Until::EndOfTurn,
            Deadline::EndOfYourNextTurn => Until::EndOfYourNextTurn { began: turn },
            Deadline::YourNext(part) => Until::YourNext(part),
            // it lasts while the card stays exiled: its window says when (702.185a)
            Deadline::Warped => Until::EndOfTurn,
        }
    }
}

/// "If an instant or sorcery spell cast this way would be put into your graveyard, exile it
/// instead": of a spell cast under the offer, while it is one of these types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rider {
    /// Empty for a singular identified "that spell"; otherwise the printed type filter.
    pub(crate) types: Vec<ir::CardType>,
    /// The object and ability whose effect made the offer, once it is made: the identity of
    /// the replacement effect the rider is (616.1). An identity label, owning nothing.
    pub(crate) by: Option<(Option<ObjId>, Option<crate::binding::AbilityKey>)>,
}

impl Rider {
    /// Is the spell, as it is about to leave the stack for its owner's graveyard, exiled
    /// instead?
    pub(crate) fn exiles(&self, game: &Game, spell: ObjId) -> bool {
        let seen = game.seen_for(spell, crate::layers::Aspect::Types);
        self.types.is_empty() || self.types.iter().any(|t| seen.is(*t))
    }
}

/// When a permission lets its card be cast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Window {
    /// At the times its card could be cast anyway (117.1a), until the turn ends (514.2); or,
    /// for an adventurer card, as long as it stays exiled (715.3d).
    Ordinary,
    /// Only while the effect that offers it is applied, whatever the card's timing (608.2g).
    Offered,
    /// 702.170d: a later turn, own main phase, empty stack; remains until it leaves exile.
    Plotted(u32),
    /// 702.185a: after the turn it was warped in has ended, at the times its card could be
    /// cast anyway, for its ordinary costs; remains until it leaves exile.
    Warped(u32),
}

/// What a permission lets its card be cast for (601.2b, 601.2f).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Costs {
    /// What it could be cast for from a hand: its mana cost, or an alternative cost its card
    /// prints.
    Ordinary,
    /// Without paying its mana cost: itself an alternative cost (118.9), said with the
    /// default identity [`CastCost::Mana`], as no cost of the card is named.
    Free,
    /// This alternative cost its card states, and no other (118.9a): its madness cost
    /// (702.35a).
    Only(CastCost),
}

impl Costs {
    /// Does it let its card be cast for the cost so said?
    pub(crate) fn allow(self, chosen: CastCost) -> bool {
        match self {
            Costs::Ordinary => true,
            Costs::Free => chosen == CastCost::Mana,
            Costs::Only(only) => chosen == only,
        }
    }
}

impl Game {
    /// An effect lets this player play (or only cast) this exiled card until its deadline.
    pub(crate) fn permit(&mut self, player: PlayerId, object: ObjId, play: bool, until: Until) {
        self.permissions.push(Permission { player, object, play, window: Window::Ordinary, costs: Costs::Ordinary, adventure: false, haste: false, zone: Zone::Exile, rider: None, ceiling: None, until });
    }

    /// [`Game::permit`], put in force now with the deadline the text states.
    pub(crate) fn permit_until(&mut self, player: PlayerId, object: ObjId, play: bool, until: Deadline) {
        match until {
            Deadline::Warped => {
                let turn = self.turn;
                self.permissions.push(Permission { player, object, play, window: Window::Warped(turn), costs: Costs::Ordinary, adventure: false, haste: false, zone: Zone::Exile, rider: None, ceiling: None, until: Until::EndOfTurn });
            }
            until => self.permit(player, object, play, until.from(self.turn)),
        }
    }

    /// The turn this exile incarnation was warped in, a public designation (702.185a–b).
    pub fn warped(&self, card: ObjId) -> Option<u32> {
        if self.obj(card).zone != Zone::Exile { return None; }
        self.permissions.iter().find_map(|p| if p.object == card { if let Window::Warped(turn) = p.window { Some(turn) } else { None } } else { None })
    }

    /// An effect being applied offers this player a cast of this card where it is (exile, or
    /// their graveyard), for these costs, now (608.2g); a spell cast so carries the rider.
    pub(crate) fn offer(&mut self, player: PlayerId, object: ObjId, costs: Costs, rider: Option<Rider>, ceiling: Option<u32>) -> &mut Permission {
        let zone = self.obj(object).zone;
        self.permissions.push(Permission { player, object, play: false, window: Window::Offered, costs, adventure: false, haste: false, zone, rider, ceiling, until: Until::EndOfTurn });
        self.permissions.last_mut().expect("the new resolution-time offer")
    }

    pub(crate) fn permit_adventure(&mut self, player: PlayerId, object: ObjId) {
        self.permissions.push(Permission { player, object, play: true, window: Window::Ordinary, costs: Costs::Ordinary, adventure: true, haste: false, zone: Zone::Exile, rider: None, ceiling: None, until: Until::EndOfTurn });
    }

    pub(crate) fn permit_plotted(&mut self, player: PlayerId, object: ObjId) {
        let turn = self.turn;
        self.permissions.push(Permission { player, object, play: false, window: Window::Plotted(turn), costs: Costs::Free, adventure: false, haste: false, zone: Zone::Exile, rider: None, ceiling: None, until: Until::EndOfTurn });
    }

    /// A public designation of this exile incarnation, even before it can be cast (702.170c).
    pub fn plotted(&self, card: ObjId) -> Option<u32> {
        if self.obj(card).zone != Zone::Exile { return None; }
        self.permissions.iter().find_map(|p| if p.object == card { if let Window::Plotted(turn) = p.window { Some(turn) } else { None } } else { None })
    }

    /// The permissions under which this player could cast this exiled card as `choice`.
    pub(crate) fn cast_permissions(&self, player: PlayerId, card: ObjId, choice: crate::card::CastAs) -> impl Iterator<Item = &Permission> {
        self.permissions_of(player, card).filter(move |p| (!p.adventure || choice == crate::card::CastAs::Whole)
            && p.ceiling.is_none_or(|bound| self.def(card).is_some_and(|def|
                def.spell(choice).is_ok_and(|spell| spell.spell_mana_value(Some(0)) <= bound))))
    }

    pub(crate) fn permitted_cast_from_exile(&self, player: PlayerId, card: ObjId, choice: crate::card::CastAs) -> bool {
        self.cast_permissions(player, card, choice).next().is_some()
    }

    /// The permission this player casts this card under, for the cost so said: none for a
    /// card not in exile; in exile, one that lets it be cast for that cost, the offer of the
    /// effect being applied before one that lasts.
    pub(crate) fn cast_permission(&self, player: PlayerId, card: ObjId, choice: crate::card::CastAs, chosen: CastCost) -> Result<Option<&Permission>, String> {
        let mut permits = self.cast_permissions(player, card, choice).peekable();
        if permits.peek().is_none() { return Ok(None); }
        let mut allowing: Vec<&Permission> = permits.filter(|p| p.costs.allow(chosen)).collect();
        allowing.sort_by_key(|p| p.window != Window::Offered);
        allowing.first().map(|p| Some(*p)).ok_or_else(|| "the effect lets that card be cast only for the cost it offers (118.9a)".into())
    }

    fn permissions_of(&self, player: PlayerId, card: ObjId) -> impl Iterator<Item = &Permission> {
        let zone = self.obj(card).zone;
        self.permissions.iter().filter(move |p| p.zone == zone && p.player == player && p.object == card && match p.window { Window::Plotted(turn) => self.turn > turn && self.sorcery_timing(player), Window::Warped(turn) => self.turn > turn, _ => true })
    }

    /// Why this player can't cast this card (or play it, `verb`) from where it is: no rule or
    /// effect allows it (601.3, 305.1), or the one that does allows only part of it.
    pub(crate) fn not_permitted(&self, player: PlayerId, card: ObjId, choice: crate::card::CastAs, verb: &str) -> String {
        let o = self.obj(card);
        let rule = if verb == "play" { "305.1" } else { "601.3" };
        if o.zone == Zone::Gone {
            // a copy or token outside its zones, or an object otherwise gone (704.5d-e)
            return format!("that object has ceased to exist: there is nothing to {verb}");
        }
        if o.zone == Zone::Hand {
            return format!("that card is in another player's hand; you {verb} only from your own ({rule})");
        }
        if self.permissions_of(player, card).next().is_some() && self.cast_permissions(player, card, choice).next().is_none() {
            if self.permissions_of(player, card).any(|p| p.ceiling.is_some()) {
                return "the resulting spell's mana value exceeds the effect's bound (701.57a, 702.85a)".into();
            }
            return "it was exiled on an adventure: it may be cast as its creature or other main card, not as its Adventure (715.3d)".into();
        }
        if self.permissions.iter().any(|p| p.object == card && p.zone == o.zone && p.player != player) {
            return format!("an effect lets another player {verb} that card, not you ({rule})");
        }
        if self.permissions.iter().any(|p| p.object == card && p.zone == o.zone && p.player == player) {
            return format!("the effect that lets you {verb} that card doesn't yet ({rule})");
        }
        let zone = match o.zone {
            Zone::Library => "in a library", Zone::Graveyard => "in a graveyard", Zone::Exile => "in exile",
            Zone::Battlefield => "on the battlefield", Zone::Stack => "on the stack", Zone::Command => "in the command zone",
            Zone::Hand | Zone::Gone => unreachable!("answered above"),
        };
        format!("that card is not in your hand: it is {zone}, and no rule or effect lets you {verb} it from there ({rule})")
    }

    /// Does an effect let this player play this exiled card now? Whether a land may be
    /// played that way, not only a spell cast.
    pub(crate) fn permitted_from_exile(&self, player: PlayerId, card: ObjId) -> Option<bool> {
        self.permissions_of(player, card).map(|p| p.play).max()
    }

    /// What a spell cast under the offer of the effect being applied carries ("If an instant or
    /// sorcery spell cast this way would be put into your graveyard, exile it instead").
    pub(crate) fn offered_rider(&self, player: PlayerId, card: ObjId) -> Option<Rider> {
        self.permissions_of(player, card).find(|p| p.window == Window::Offered).and_then(|p| p.rider.clone())
    }

    /// Is an effect being applied that offers this player a cast of this card?
    pub(crate) fn offered(&self, player: PlayerId, card: ObjId) -> bool {
        self.permissions_of(player, card).any(|p| p.window == Window::Offered)
    }

    /// Who may play this exiled card until the turn ends, and whether as a land too: what a
    /// player is shown.
    pub fn permission(&self, card: ObjId) -> Option<(PlayerId, bool)> {
        let p = self.permissions.iter().find(|p| p.object == card && p.window == Window::Ordinary && self.obj(card).zone == Zone::Exile)?;
        Some((p.player, self.permitted_from_exile(p.player, card) == Some(true)))
    }

    /// When the player [`Game::permission`] shows may no longer play this card, said of them,
    /// when all their grants ([`Game::permission_grants`]) end alike. None when they differ:
    /// which ends last can depend on what is still to happen (a skipped upkeep, 614.10), so
    /// each is shown instead; or when one lasts as long as the card stays exiled (715.3d).
    pub fn permission_until(&self, card: ObjId) -> Option<&'static str> {
        let (player, _) = self.permission(card)?;
        let mut deadlines = self.permission_grants(card).into_iter().filter(|g| g.0 == player).map(|g| g.2);
        let first = deadlines.next()?;
        if deadlines.all(|d| d == first) { first } else { None }
    }

    /// Each lasting grant to play this exiled card, once per distinct right and deadline: its
    /// player, whether as a land too, and when it ends, said of that player: `end_of_turn`
    /// (this turn's, perhaps as the end of their next turn now under way),
    /// `end_of_their_next_turn`, `their_next_upkeep`, `their_next_end_step` (as it begins);
    /// None for an Adventure's, which lasts as long as the card stays exiled (715.3d).
    pub fn permission_grants(&self, card: ObjId) -> Vec<(PlayerId, bool, Option<&'static str>)> {
        if self.obj(card).zone != Zone::Exile { return Vec::new(); }
        let mut grants = Vec::new();
        for p in self.permissions.iter().filter(|p| p.object == card && p.window == Window::Ordinary) {
            let until = if p.adventure { None } else {
                Some(match p.until {
                    Until::EndOfTurn => "end_of_turn",
                    Until::EndOfYourNextTurn { began } if self.active == p.player && self.turn != began => "end_of_turn",
                    Until::EndOfYourNextTurn { .. } => "end_of_their_next_turn",
                    Until::YourNext(ir::Part::Upkeep) => "their_next_upkeep",
                    Until::YourNext(_) => "their_next_end_step",
                })
            };
            let grant = (p.player, p.play, until);
            if !grants.contains(&grant) { grants.push(grant); }
        }
        grants
    }


    /// The effect has stopped offering this card: if it was not cast, it is not cast later.
    /// Each offer ends for its own cards only.
    pub(crate) fn revoke(&mut self, player: PlayerId, card: ObjId) {
        self.permissions.retain(|p| !(p.window == Window::Offered && p.player == player && p.object == card));
    }

    /// "This turn" is over (514.2), and so is "the end of your next turn" of its player's turn
    /// after the one it began in.
    pub(crate) fn end_turn_permissions(&mut self) {
        let (active, turn) = (self.active, self.turn);
        self.permissions.retain(|p| p.adventure || matches!(p.window, Window::Plotted(_) | Window::Warped(_)) || match p.until {
            Until::EndOfTurn => false,
            Until::EndOfYourNextTurn { began } => p.player != active || turn == began,
            Until::YourNext(_) => true,
        });
    }

    /// A step of the active player's turn begins: "until your next upkeep" / "end step" of
    /// theirs is over.
    pub(crate) fn begin_part_permissions(&mut self, part: ir::Part) {
        let active = self.active;
        self.permissions.retain(|p| !(p.until == Until::YourNext(part) && p.player == active));
    }
}
