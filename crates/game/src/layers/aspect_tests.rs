//! [`Aspect`] in the contexts a question can be asked from: a permanent about to enter, a
//! view that stops before a layer, a view of what was, a spell being cast.
use super::{Aspect, Layer, Seen, Working, LAYERS};
use crate::card::{CardDef, CastAs};
use crate::select::Ctx;
use crate::state::{Game, ObjId, Zone};
use ir::CardType;

const ASPECTS: [Aspect; 4] = [Aspect::Types, Aspect::Colors, Aspect::Abilities, Aspect::All];

fn game() -> (Game, Vec<ObjId>) {
    let card = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).unwrap();
    let defs = vec![
        card("Bear", "Creature — Bear", "", Some((2, 2))),
        card("Anthem", "Enchantment", "Creatures you control get +1/+1.", None),
        card("Fervor", "Enchantment", "Creatures you control have haste.", None),
        card("Animation", "Enchantment", "Noncreature artifacts become 3/3 artifact creatures.", None),
        card("Rock", "Artifact", "", None),
        card("Humility", "Enchantment", "All creatures lose all abilities and have base power and toughness 1/1.", None),
        card("Captain", "Creature — Soldier", "Flying\nCreatures you control have vigilance.\nThis creature gets +1/+1 as long as you control an artifact.", Some((2, 2))),
        card("Goyf", "Creature — Lhurgoyf", "This creature's power and toughness are each equal to the number of creatures you control.", Some((0, 0))),
    ];
    for d in &defs {
        crate::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    let mut g = Game::new(defs, 2, 20);
    let mut objects: Vec<ObjId> = [0, 1, 2, 3, 4, 7].into_iter().map(|def| g.put(0, def, Zone::Battlefield)).collect();
    objects.push(g.put(1, 0, Zone::Battlefield));
    objects.push(g.put(0, 7, Zone::Library));
    objects.push(g.put(1, 7, Zone::Graveyard));
    (g, objects)
}
/// The fields this aspect reads are equal.
fn same(aspect: Aspect, narrow: &Seen, full: &Seen, at: &str) {
    assert_eq!((narrow.controller, &narrow.types, &narrow.subtypes, &narrow.supertypes), (full.controller, &full.types, &full.subtypes, &full.supertypes), "types {at}");
    if aspect == Aspect::Types { return; }
    assert_eq!(narrow.colors, full.colors, "colors {at}");
    if aspect == Aspect::Colors { return; }
    assert_eq!((&narrow.keywords, narrow.base_abilities, narrow.intrinsic_abilities, &narrow.given), (&full.keywords, full.base_abilities, full.intrinsic_abilities, &full.given), "abilities {at}");
    if aspect == Aspect::All { assert_eq!(narrow, full, "all {at}"); }
}

#[test]
fn the_cut_of_an_aspect_is_never_later_than_the_view_it_is_asked_in() {
    assert_eq!(Aspect::Types.within(None), Some(Layer::Colors));
    assert_eq!(Aspect::All.within(None), None);
    assert_eq!(Aspect::All.within(Some(Layer::Types)), Some(Layer::Types));
    assert_eq!(Aspect::Abilities.within(Some(Layer::Types)), Some(Layer::Types), "the view stops first");
    assert_eq!(Aspect::Types.within(Some(Layer::ChangePt)), Some(Layer::Colors), "the question stops first");
    assert_eq!(Aspect::Abilities.within(Some(Layer::DefinePt)), Some(Layer::DefinePt));
}

#[test]
fn a_permanent_about_to_enter_is_asked_with_its_own_abilities_and_the_others_without() {
    for with_humility in [false, true] {
        let (mut g, mut objects) = game();
        if with_humility { objects.push(g.put(0, 5, Zone::Battlefield)); }
        let entrant = g.put(0, 6, Zone::Hand);
        objects.push(entrant);
        g.prospective = Some((entrant, None));
        for o in &objects {
            let full = g.seen(*o, None);
            for aspect in ASPECTS {
                same(aspect, &g.seen_for(*o, aspect), &full, &format!("of {} entering={} humility={with_humility}", g.obj(*o).chars.name, *o == entrant));
            }
        }
        // the entrant's own abilities are discovered for it alone: nothing of it reaches a
        // permanent, asked narrowly or whole
        let vigilance = ir::Keyword::from_name("Vigilance").unwrap();
        assert!(!g.seen_for(objects[0], Aspect::Abilities).has(vigilance));
        assert!(g.seen_for(entrant, Aspect::Abilities).has(ir::Keyword::from_name("Flying").unwrap()));
    }
}

#[test]
fn a_view_that_stops_before_a_layer_is_not_asked_past_it() {
    let (mut g, mut objects) = game();
    objects.push(g.put(0, 5, Zone::Battlefield));
    objects.push(g.put(0, 6, Zone::Battlefield));
    for layer in LAYERS {
        let ctx = Ctx { layer: Some(layer), ..Ctx::new(0, objects[0]) };
        for o in &objects {
            let partial = ctx.seen(&g, *o);
            assert_eq!(partial, g.seen(*o, Some(layer)));
            for aspect in ASPECTS {
                let narrow = ctx.seen_for(&g, *o, aspect);
                // where the question reads past the view, it has what the view has: no more
                let read = |a: Aspect| a.cut().is_none_or(|cut| cut > layer);
                if read(aspect) { assert_eq!(narrow, partial, "{aspect:?} before {layer:?}"); }
                else { same(aspect, &narrow, &partial, &format!("{aspect:?} before {layer:?} of {}", g.obj(*o).chars.name)); }
            }
        }
    }
    // and the layers do stop: before the abilities layer the Rock is a creature without
    // Humility having taken anything from the Captain
    let rock = objects[4];
    let captain = *objects.last().unwrap();
    let before = Ctx { layer: Some(Layer::Abilities), ..Ctx::new(0, rock) };
    assert!(before.seen_for(&g, rock, Aspect::Types).is(CardType::Creature));
    assert!(before.seen_for(&g, captain, Aspect::Abilities).base_abilities);
    assert!(!Ctx::new(0, rock).seen_for(&g, captain, Aspect::Abilities).base_abilities);
}

#[test]
fn a_view_carried_by_the_context_answers_by_itself() {
    let (g, objects) = game();
    let (bear, rock) = (objects[0], objects[4]);
    // what was before an event: the Bear as an artifact another player controlled
    let mut was = g.seen(bear, None);
    was.types.push(CardType::Artifact);
    was.controller = 1;
    was.pt = Some((7, 7));
    let view = Working::before(vec![(bear, was.clone())]);
    for o in &objects { g.seen(*o,None); } // ordinary cache must not leak into LKI
    let ctx = Ctx { view: Some(&view), ..Ctx::new(0, bear) };
    for aspect in ASPECTS {
        assert_eq!(ctx.seen_for(&g, bear, aspect), was, "{aspect:?}: the view's own record, whole");
        // an object the view does not hold is as it is printed, not as the layers have it now
        assert_eq!(ctx.seen_for(&g, rock, aspect), g.base_seen(rock), "{aspect:?}");
    }
    assert!(g.seen_for(rock, Aspect::Types).is(CardType::Creature), "which now is a creature");
    // a partial layer world: the same, finished as the context finishes it
    let partial = Working::new(vec![(bear, was.clone())]);
    let ctx = Ctx { view: Some(&partial), layer: Some(Layer::SetPt), ..Ctx::new(0, bear) };
    for aspect in ASPECTS {
        assert_eq!(ctx.seen_for(&g, bear, aspect), ctx.seen(&g, bear), "{aspect:?}");
        assert_eq!(ctx.seen_for(&g, bear, aspect).pt, Some((7, 7)));
    }
}

#[test]
fn a_spell_being_cast_is_as_the_face_chosen_for_it() {
    let (mut g, _) = game();
    let card = g.put(1, 6, Zone::Hand);
    assert_eq!(g.seen(card,None).controller,1); // warm a conflicting ordinary answer
    let ctx = Ctx { casting: Some((card, CastAs::Whole)), ..Ctx::new(0, card) };
    let whole = ctx.seen(&g, card);
    assert_eq!(whole.controller, 0, "the player casting it, not the layers' answer");
    for aspect in ASPECTS {
        assert_eq!(ctx.seen_for(&g, card, aspect), whole, "{aspect:?}");
    }
}

#[test]
fn an_object_that_has_left_is_as_it_last_was_whatever_is_asked() {
    let (mut g, objects) = game();
    let bear = objects[0];
    let last = g.seen(bear, None);
    assert_eq!(last.pt, Some((3, 3)));
    g.move_object(bear, Zone::Graveyard);
    assert_eq!(g.obj(bear).zone, Zone::Gone);
    for aspect in ASPECTS {
        assert_eq!(g.seen_for(bear, aspect), last, "{aspect:?}");
    }
}

#[test]
fn cached_queries_keep_entry_and_layer_views_distinct() {
    let (mut g, mut objects) = game();
    let entrant = g.put(0, 6, Zone::Hand);
    objects.push(entrant);
    for prospective in [Some((entrant, None)), None] {
        g.prospective = prospective;
        let expected: Vec<_> = objects.iter().flat_map(|o| ASPECTS.map(|aspect| (*o, aspect, g.seen_for(*o, aspect)))).collect();
        // Narrow before whole, then reverse; cached answers must never cross cuts.
        for (o, aspect, expected) in expected.iter().chain(expected.iter().rev()) {
            assert_eq!(&g.seen_for(*o, *aspect), expected);
        }
    }
}
