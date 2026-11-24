//! Phasing (CR 702.26). A permanent that phases out is treated as though it does not exist
//! (702.26b) until it phases in (702.26c), the same permanent where it was: phasing is no
//! zone change, and the object, its status, counters and history go on (702.26d). The engine
//! keeps it apart from the battlefield ([`Zone::PhasedOut`]), so nothing that looks at the
//! battlefield finds it.

use crate::state::{Game, ObjId, Zone};
use crate::{Event, PlayerId, Target};

/// How a phased-out permanent phased out (702.26).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Phased {
    /// Its controller as it phased out: it phases in during that player's untap step (502.1).
    pub under: PlayerId,
    /// It phased out with the permanent it is attached to, "indirectly" (702.26g): it phases
    /// in with that permanent, never by itself.
    pub indirect: bool,
    /// How many of `under`'s untap steps it waits through first. One that phased out under a
    /// player who has since left the game phases in during the next untap step after that
    /// player's next turn would have begun (702.26n).
    pub(crate) waits: u8,
}

impl Game {
    /// These permanents phase out, at once (702.26b).
    pub(crate) fn phase_out(&mut self, permanents: &[ObjId]) {
        self.phase(permanents, &[]);
    }

    /// The untap step's phasing (502.1, 702.26a), before its permanents untap: the phased-in
    /// permanents with phasing the active player controls phase out and, simultaneously, those
    /// that phased out directly under that player's control phase in.
    pub(crate) fn untap_step_phasing(&mut self) {
        let player = self.active;
        let phasing = ir::Keyword::from_name("Phasing").expect("phasing is a keyword ability");
        let outgoing: Vec<ObjId> =
            self.battlefield.iter().copied().filter(|id| self.controller(*id) == player && self.has_keyword(*id, phasing)).collect();
        if outgoing.is_empty() && self.phased_out.is_empty() { return; }
        let mut incoming = Vec::new();
        for id in self.phased_out.clone() {
            let Some(phased) = self.obj(id).phased else { continue };
            if phased.indirect || phased.under != player { continue; }
            if phased.waits > 0 {
                self.obj_mut(id).phased = Some(Phased { waits: phased.waits - 1, ..phased });
            } else {
                incoming.push(id);
            }
        }
        self.phase(&outgoing, &incoming);
    }

    /// A player leaves the game (800.4a): what phased out under their control and is not
    /// theirs (what is theirs leaves with them, 702.26k) phases in during the next untap step
    /// after their next turn would have begun (702.26n). That is the untap step of the next
    /// player in turn order still in the game, one round later if the player leaves during
    /// their own turn. Turns taken out of order are not counted.
    pub(crate) fn phased_out_under_departed(&mut self, player: PlayerId) {
        let n = self.players.len();
        let Some(next) = (1..n).map(|i| (player + i) % n).find(|p| !self.players[*p].lost) else { return };
        let waits = u8::from(self.active == player);
        for id in self.phased_out.clone() {
            if self.obj(id).phased.is_some_and(|p| p.under == player && !p.indirect) {
                self.obj_mut(id).phased = Some(Phased { under: next, indirect: false, waits });
            }
        }
    }

    /// One phasing event: these permanents phase out, and those phase in, simultaneously.
    /// What is attached to a permanent that phases out phases out with it, indirectly
    /// (702.26g), also one that would phase out directly at the same time (702.26h); one that
    /// phased out indirectly phases in with what it is attached to. A permanent that phases
    /// out is removed from combat (506.4), and effects that last for as long as it is on the
    /// battlefield or its controller's end (702.26f). An Aura, Equipment or Fortification that
    /// phased out directly phases in attached to what it was attached to, if that is still on
    /// the battlefield or that player still in the game, unattached otherwise (702.26i); one
    /// attached to what is now phased out is attached to nothing that exists, and the
    /// state-based actions see to it (704.5m, 704.5n).
    fn phase(&mut self, out: &[ObjId], into: &[ObjId]) {
        let direct: Vec<ObjId> = out.iter().copied().filter(|id| self.obj(*id).zone == Zone::Battlefield).collect();
        let hosted_by_another = |id: ObjId| {
            let mut at = id;
            let mut seen = vec![id];
            while let Some(Target::Object(host)) = self.obj(at).attached_to {
                if direct.contains(&host) { return true; }
                if seen.contains(&host) { return false; }
                seen.push(host);
                at = host;
            }
            false
        };
        let mut outgoing: Vec<(ObjId, PlayerId, bool)> = Vec::new();
        for id in direct.iter().copied().filter(|id| !hosted_by_another(*id)) {
            self.gather_out(id, false, &mut outgoing);
        }
        let mut incoming = Vec::new();
        for id in into { self.gather_in(*id, &mut incoming); }
        if outgoing.is_empty() && incoming.is_empty() { return; }
        let directly_in: Vec<ObjId> = incoming.iter().copied().filter(|id| self.obj(*id).phased.is_some_and(|p| !p.indirect)).collect();
        for (id, under, indirect) in &outgoing {
            self.battlefield.retain(|o| o != id);
            self.phased_out.push(*id);
            let object = self.obj_mut(*id);
            object.zone = Zone::PhasedOut;
            object.phased = Some(Phased { under: *under, indirect: *indirect, waits: 0 });
            self.combat.remove(*id);
            self.event_log_mut().push(Event::PhasedOut { object: *id });
        }
        for id in &incoming {
            self.phased_out.retain(|o| o != id);
            self.battlefield.push(*id);
            let object = self.obj_mut(*id);
            object.zone = Zone::Battlefield;
            object.phased = None;
            self.event_log_mut().push(Event::PhasedIn { object: *id });
        }
        for id in directly_in {
            let still = match self.obj(id).attached_to {
                Some(Target::Object(host)) => matches!(self.obj(host).zone, Zone::Battlefield | Zone::PhasedOut),
                Some(Target::Player(player)) => !self.players[player].lost,
                None => true,
            };
            if !still {
                if self.obj(id).bestowed { self.cease_bestowed(id); } else { self.obj_mut(id).attached_to = None; }
            }
        }
        if !outgoing.is_empty() { self.end_control_durations(); }
    }

    /// It and, indirectly, what is attached to it (702.26g), each with its controller now.
    fn gather_out(&self, id: ObjId, indirect: bool, out: &mut Vec<(ObjId, PlayerId, bool)>) {
        if out.iter().any(|(o, _, _)| *o == id) || self.obj(id).zone != Zone::Battlefield { return; }
        out.push((id, self.controller(id), indirect));
        let attached: Vec<ObjId> =
            self.battlefield.iter().copied().filter(|a| self.obj(*a).attached_to == Some(Target::Object(id))).collect();
        for a in attached { self.gather_out(a, true, out); }
    }

    /// It and what phased out indirectly with it (702.26g).
    fn gather_in(&self, id: ObjId, into: &mut Vec<ObjId>) {
        if into.contains(&id) || self.obj(id).zone != Zone::PhasedOut { return; }
        into.push(id);
        let attached: Vec<ObjId> = self.phased_out.iter().copied()
            .filter(|a| self.obj(*a).attached_to == Some(Target::Object(id)) && self.obj(*a).phased.is_some_and(|p| p.indirect)).collect();
        for a in attached { self.gather_in(a, into); }
    }
}
