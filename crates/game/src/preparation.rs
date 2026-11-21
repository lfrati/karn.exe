//! Prepared designation and its frozen public copy (722.3). This is not a physical
//! alternative casting choice or an exile-on-resolution permission.
use std::sync::Arc;
use crate::{Game, ObjId, PlayerId, Zone};

impl Game {
    pub(crate) fn has_prepare_spell(&self, source: ObjId) -> bool {
        self.copiable(source).definition(self).is_some_and(|d| d.prepare_spell().is_some())
    }
    pub(crate) fn prepare(&mut self, source: ObjId) {
        if self.obj(source).zone != Zone::Battlefield || self.obj(source).prepared { return; }
        let Some(spell) = self.copiable(source).definition(self).and_then(|d| d.prepare_spell()).cloned() else { return; };
        let controller = self.controller(source);
        let values = crate::copy::Values { chars: spell.chars(), definition: Some(crate::copy::Definition::Derived(Arc::new(spell))) };
        let copy = self.new_object(None, values.chars.clone(), controller, controller, Zone::Exile);
        let made = self.obj_mut(copy);
        made.card_copy = true;
        made.token_faces = Some(Arc::new(crate::copy::Faces { front: values, back: None }));
        made.prepare_source = Some(source);
        self.obj_mut(source).prepared = true;
        self.obj_mut(source).prepared_copy = Some(copy);
        self.event_log_mut().push(crate::Event::Designated { object: source, designation: ir::condition::ObjectDesignation::Prepared });
    }
    pub(crate) fn unprepare(&mut self, source: ObjId) {
        if self.obj(source).zone != Zone::Battlefield { return; }
        self.obj_mut(source).prepared = false;
        self.obj_mut(source).prepared_copy = None;
    }
    /// Live lifetime/permission, independent of the source's current characteristics.
    pub(crate) fn prepared_copy_controller(&self, copy: ObjId) -> Option<PlayerId> {
        let object = self.get_object(copy)?;
        if object.zone != Zone::Exile { return None; }
        let source = self.get_object(object.prepare_source?)?;
        (source.zone == Zone::Battlefield && source.prepared && source.prepared_copy == Some(copy))
            .then(|| self.controller(source.id))
    }
    pub(crate) fn prepared_cast_completed(&mut self, spell: ObjId) {
        if let Some(source) = self.obj(spell).prepare_source { self.unprepare(source); }
    }
}
