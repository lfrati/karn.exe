//! Explicit payload-owning references. Identity labels (grant provenance, membership,
//! activation history) deliberately do not implement an ownership edge on their own.
use crate::{ObjId, PlayerId, Target};

pub(crate) trait Trace {
    fn trace(&self, visit: &mut dyn FnMut(ObjId));
}
impl Trace for ObjId {
    fn trace(&self, visit: &mut dyn FnMut(ObjId)) { visit(*self); }
}
impl Trace for Target {
    fn trace(&self, visit: &mut dyn FnMut(ObjId)) {
        match self { Target::Object(id) => visit(*id), Target::Player(_) => {} }
    }
}
impl<T: Trace> Trace for Option<T> {
    fn trace(&self, visit: &mut dyn FnMut(ObjId)) { if let Some(value) = self { value.trace(visit); } }
}
impl<T: Trace> Trace for Vec<T> {
    fn trace(&self, visit: &mut dyn FnMut(ObjId)) { for value in self { value.trace(visit); } }
}

/// The players a move or an answer names, as [`Trace`] says the objects it names: each is
/// checked to be one of the game's before the move is tried.
pub(crate) trait Players {
    fn players(&self, visit: &mut dyn FnMut(PlayerId));
}
impl Players for Target {
    fn players(&self, visit: &mut dyn FnMut(PlayerId)) { if let Target::Player(player) = self { visit(*player); } }
}
impl<T: Players> Players for Option<T> {
    fn players(&self, visit: &mut dyn FnMut(PlayerId)) { if let Some(value) = self { value.players(visit); } }
}
impl<T: Players> Players for Vec<T> {
    fn players(&self, visit: &mut dyn FnMut(PlayerId)) { for value in self { value.players(visit); } }
}
