//! Rooms (709.5): permanent split cards whose halves are doors, each locked until the
//! permanent has its "unlocked" designation. What the permanent is with its doors as they are
//! is [`crate::CardDef::room`]; this is the unlock cost, the special action that pays it
//! (116.2m, 709.5e) and the unlocking itself (709.5f, 709.5h).
use crate::cost::Plan;
use crate::state::{Pool, Zone};
use crate::{Event, Game, ObjId, PlayerId};

impl Game {
    /// The Room card this permanent is, with doors: not while it is face down, when it is a
    /// 2/2 with no halves at all (708.2a), and no one can tell that it is a Room (708.5).
    pub(crate) fn room_card(&self, permanent: ObjId) -> Option<&crate::CardDef> {
        let o = self.get_object(permanent)?;
        if matches!(o.face, crate::face::Face::Down(_)) { return None; }
        o.card.map(|i| &self.defs[i]).filter(|d| d.is_room())
    }

    /// The unlock cost of this half of a Room permanent, if this player may pay it now: the
    /// mana cost of a locked half of a permanent they control, any time they have priority
    /// and the stack is empty during a main phase of their turn (116.2m, 709.5e).
    pub(crate) fn unlock_cost(&self, player: PlayerId, room: ObjId, half: usize) -> Result<Plan, String> {
        let o = self.get_object(room).ok_or("no such object")?;
        if o.zone != Zone::Battlefield { return Err("only a Room permanent is unlocked (709.5e)".into()); }
        let def = self.room_card(room).ok_or("only a Room has doors to unlock (709.5e)")?;
        if self.controller(room) != player { return Err("only its controller unlocks a door (116.2m)".into()); }
        let door = def.halves().get(half).ok_or("a Room has two doors, 0 and 1 (709.5j)")?;
        if o.unlocked[half] { return Err("that door is already unlocked (709.5e)".into()); }
        if !self.sorcery_timing(player) {
            return Err("unlock a door only during a main phase of your turn with the stack empty (116.2m)".into());
        }
        let cost = door.cost.as_ref().ok_or("a door with no mana cost has no unlock cost (709.5e)")?;
        Ok(Plan::default().kicked(cost, 1))
    }

    /// The special action (116.2m): pay the unlock cost of this half, then it is unlocked.
    pub(crate) fn unlock_action(&mut self, player: PlayerId, room: ObjId, half: usize, steps: &[crate::legal::Step], with: Option<&Pool>) -> crate::play::ActionResult<()> {
        let mut proposed = self.clone();
        let cost = proposed.unlock_cost(player, room, half)?;
        let library_tops = proposed.library_tops();
        proposed.procedures.push(crate::procedure::Procedure::Payment(
            crate::payment::Payment::unlock(player, room, half, cost, steps, with).library_tops(library_tops),
        ));
        proposed.continue_execution()?;
        proposed.expose_payments();
        self.adopt(proposed);
        Ok(())
    }

    /// Give a Room permanent the unlocked designation of this half (709.5f), as `player`
    /// unlocks it: it is now what its unlocked halves make it.
    pub(crate) fn unlock(&mut self, room: ObjId, half: usize, player: PlayerId) {
        let o = self.obj(room);
        if o.zone != Zone::Battlefield || o.unlocked[half] { return; }
        self.obj_mut(room).unlocked[half] = true;
        self.refresh_face(room);
        self.event_log_mut().push(Event::Unlocked { object: room, half, player });
    }
}
