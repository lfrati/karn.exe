//! Game-local incarnation handles. Reusing storage never reuses an identity (400.7).
use super::{ObjId, Object};

#[derive(Debug, Clone)]
struct Slot {
    generation: u32,
    object: Option<Object>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct Arena {
    slots: Vec<Slot>,
    free: Vec<u32>,
}

impl ObjId {
    pub(super) fn slot(self) -> usize { self.0 as u32 as usize }
    fn generation(self) -> u32 { (self.0 >> 32) as u32 }
}

impl Arena {
    pub(super) fn insert(&mut self, make: impl FnOnce(ObjId) -> Object) -> ObjId {
        let slot = self.free.pop().unwrap_or_else(|| {
            let slot = u32::try_from(self.slots.len()).expect("object slot space exhausted");
            self.slots.push(Slot { generation: 0, object: None });
            slot
        });
        let entry = &mut self.slots[slot as usize];
        let id = ObjId((u64::from(entry.generation) << 32) | u64::from(slot));
        entry.object = Some(make(id));
        id
    }

    pub(super) fn get(&self, id: ObjId) -> Option<&Object> {
        let slot = self.slots.get(id.slot())?;
        (slot.generation == id.generation()).then_some(slot.object.as_ref()).flatten()
    }

    pub(super) fn get_mut(&mut self, id: ObjId) -> Option<&mut Object> {
        let slot = self.slots.get_mut(id.slot())?;
        (slot.generation == id.generation()).then_some(slot.object.as_mut()).flatten()
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &Object> {
        self.slots.iter().filter_map(|slot| slot.object.as_ref())
    }

    pub(super) fn slots(&self) -> usize { self.slots.len() }

    pub(super) fn sweep(&mut self, marked: &[bool]) -> usize {
        let mut removed = 0;
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if !marked[index] && slot.object.take().is_some() {
                removed += 1;
                // Never wrap: an exhausted generation permanently retires this slot.
                if let Some(next) = slot.generation.checked_add(1) {
                    slot.generation = next;
                    self.free.push(index as u32);
                }
            }
        }
        removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r400_7_exhausted_generations_retire_the_slot_instead_of_reusing_a_handle() {
        let def = crate::CardDef::new("Land", "", "Land", "", None).unwrap();
        let mut game = crate::Game::new(vec![def], 2, 20);
        let id = game.put(0, 0, crate::Zone::Hand);
        let prototype = game.obj(id).clone();
        let arena = &mut game.objects;
        arena.slots[0].generation = u32::MAX;
        arena.slots[0].object.as_mut().unwrap().id = ObjId(u64::from(u32::MAX) << 32);
        assert_eq!(arena.sweep(&[false]), 1);
        let next = arena.insert(|id| Object { id, ..prototype });
        assert_eq!(next.slot(), 1);
        assert!(arena.get(id).is_none());
        assert!(arena.get(ObjId(u64::from(u32::MAX) << 32)).is_none());
    }
}
