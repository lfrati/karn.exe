//! Bestow (702.103). "Bestow [cost]" means "As you cast this spell, you may choose to cast it
//! bestowed. If you do, you pay [cost] rather than its mana cost" (702.103a). As a spell cast
//! bestowed is put onto the stack it becomes an Aura enchantment with enchant creature: a
//! bestowed Aura spell, and the permanent it becomes a bestowed Aura, until it ceases to be
//! bestowed (702.103b). That is the object's [`crate::state::Object::bestowed`], an effect of
//! its static ability applied in layer 4 with the object's timestamp (613.1d, 613.7a), not a
//! change of its copiable values: a copy of the permanent is a creature, a copy of the spell
//! is bestowed (702.103c).
//!
//! It ceases to be bestowed as the spell begins to resolve with an illegal target, and goes on
//! resolving as a creature spell (702.103e, 608.3b); and as the Aura becomes unattached or is
//! attached illegally, when it stays on the battlefield as the same object, an enchantment
//! creature (702.103f): the engine sees that as it checks state-based actions, where an
//! ordinary Aura would be put into its owner's graveyard (704.5m).
use crate::announcement::TargetSpec;
use crate::card::CardDef;
use crate::layers::Types;
use crate::{Game, ObjId};

/// The card types and subtypes a bestowed object has: an Aura enchantment, which is not a
/// creature and has none of its creature types (702.103b, 205.1a). Its supertypes stay.
pub(crate) fn types() -> &'static Types {
    static TYPES: std::sync::OnceLock<Types> = std::sync::OnceLock::new();
    TYPES.get_or_init(|| Types { types: vec![ir::CardType::Enchantment], subtypes: vec![ir::Subtype::aura()], every_creature_type: false, keeps: false })
}

/// The enchant creature ability a bestowed object gains: what its Aura spell targets and what
/// the Aura can be attached to (702.103b, 303.4a, 303.4d).
fn enchant_creature() -> &'static TargetSpec {
    static ENCHANT: std::sync::OnceLock<TargetSpec> = std::sync::OnceLock::new();
    crate::token::init(&ENCHANT, || {
        let def = CardDef::new("Bestow rule", "", "Enchantment — Aura", "Enchant creature", None).expect("the grammar reads rule 702.103b's enchant ability");
        def.enchant.clone().ok().flatten().expect("rule 702.103b's enchant ability compiles")
    })
}

impl Game {
    /// Is the alternative cost at this row of the definition a bestow cost (702.103a)?
    pub(crate) fn bestow_row(def: &CardDef, row: usize) -> bool {
        ir::Keyword::from_name("Bestow").is_some_and(|bestow| crate::keyword::alternative_rows(def, bestow).contains(&row))
    }

    /// What this Aura spell targets and this Aura can enchant: enchant creature for a bestowed
    /// one (702.103b), otherwise its own enchant ability (303.4a, 702.5a). None for what is no
    /// Aura, or one whose enchant ability the engine refuses.
    pub(crate) fn enchant_spec(&self, object: ObjId) -> Option<&TargetSpec> {
        if self.obj(object).bestowed { return Some(enchant_creature()); }
        self.def(object).and_then(crate::attach::enchants)
    }

    /// Does it enter the battlefield attached to what it enchants (303.4f, 608.3a): an Aura card,
    /// or a bestowed Aura spell (702.103b)?
    pub(crate) fn enters_attached(&self, object: ObjId) -> bool {
        self.obj(object).bestowed || self.def(object).is_some_and(crate::attach::aura)
    }

    /// It ceases to be bestowed: the effect making it an Aura ends and it is unattached
    /// (702.103e–f). The object stays the same object.
    pub(crate) fn cease_bestowed(&mut self, object: ObjId) {
        let o = self.obj_mut(object);
        o.bestowed = false;
        o.attached_to = None;
    }
}
