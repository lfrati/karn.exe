//! Game-local incarnation handles. Reusing storage never reuses an identity (400.7).
use super::{ObjId, Object};
use std::sync::Arc;

#[derive(Debug, Clone)]
struct Slot {
    generation: u32,
    object: Option<Object>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct Arena {
    // Rollback branches share immutable slots. A write copies only its slot's
    // object, retaining the same incarnation handle in that branch.
    slots: Vec<Arc<Slot>>,
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
            self.slots.push(Arc::new(Slot { generation: 0, object: None }));
            slot
        });
        let entry = Arc::make_mut(&mut self.slots[slot as usize]);
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
        if slot.generation != id.generation() || slot.object.is_none() { return None; }
        Arc::make_mut(slot).object.as_mut()
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &Object> {
        self.slots.iter().filter_map(|slot| slot.object.as_ref())
    }

    pub(super) fn slots(&self) -> usize { self.slots.len() }

    pub(super) fn sweep(&mut self, marked: &[bool]) -> usize {
        let mut removed = 0;
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if !marked[index] && slot.object.is_some() {
                let slot = Arc::make_mut(slot);
                slot.object = None;
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
        let slot = Arc::make_mut(&mut arena.slots[0]);
        slot.generation = u32::MAX;
        slot.object.as_mut().unwrap().id = ObjId(u64::from(u32::MAX) << 32);
        assert_eq!(arena.sweep(&[false]), 1);
        let next = arena.insert(|id| Object { id, ..prototype });
        assert_eq!(next.slot(), 1);
        assert!(arena.get(id).is_none());
        assert!(arena.get(ObjId(u64::from(u32::MAX) << 32)).is_none());
    }

    #[test]
    fn branches_isolate_writes_sweeps_and_reused_incarnations() {
        let def = crate::CardDef::new("Land", "", "Land", "", None).unwrap();
        let mut game = crate::Game::new(vec![def], 2, 20);
        let id = game.put(0, 0, crate::Zone::Hand);
        let mut branch = game.objects.clone();
        assert!(Arc::ptr_eq(&game.objects.slots[0], &branch.slots[0]));
        branch.get_mut(id).unwrap().tapped = true;
        assert!(!game.obj(id).tapped);
        assert!(branch.get(id).unwrap().tapped);
        let retained = branch.get(id).unwrap().clone();
        assert_eq!(branch.sweep(&[false]), 1);
        let next = branch.insert(|id| Object { id, ..retained });
        assert_eq!(next.slot(), id.slot());
        assert_ne!(next, id);
        assert!(branch.get(id).is_none());
        assert!(game.get_object(id).is_some());
        assert!(game.get_object(next).is_none());
    }
}
