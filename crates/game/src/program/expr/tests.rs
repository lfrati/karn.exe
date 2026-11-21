use super::*;
use crate::CardDef;
use crate::binding::AbilityKey;
use ir::{Atom as Said, ObjectSet};

/// A frame's slots, without a frame.
struct Slots(Vec<Value>);

impl Locals for Slots {
    fn local(&self, slot: usize) -> Option<&Value> {
        self.0.get(slot)
    }
}

fn permanents(filter: Vec<ir::Filter>) -> ObjectSet {
    ObjectSet { filter: ir::Filter::All(filter), ..ObjectSet::of(ir::Domain::Permanent) }
}

fn chosen(kind: Kind) -> ir::Filter {
    ir::Filter::Atom(Said::Reference(UnresolvedRef::choice(kind)))
}

fn game() -> (Game, ObjId, ObjId, ObjId) {
    let source = CardDef::new("Source", "", "Land", "", None).unwrap();
    let red = CardDef::new("Red", "{R}", "Creature — Goblin", "", Some((2, 1))).unwrap();
    let green = CardDef::new("Green", "{2}{G}", "Creature — Elf", "", Some((3, 3))).unwrap();
    let mut game = Game::new(vec![source, red, green], 2, 20);
    let source = game.put(0, 0, Zone::Battlefield);
    let red = game.put(0, 1, Zone::Battlefield);
    let green = game.put(1, 2, Zone::Battlefield);
    (game, source, red, green)
}

#[test]
fn r608_2h_a_local_choice_is_read_from_the_frame_and_an_unmade_one_describes_nothing() {
    let (game, source, red, _) = game();
    let def = game.def(source).unwrap();
    let support = Support::definition(def, None);
    let mut bind = |r: &UnresolvedRef| Ok(Scalar::Local { slot: 0, kind: r.expected });
    let set = Set::compile(&permanents(vec![chosen(Kind::Color)]), &support, &mut bind).unwrap();
    assert_eq!(set.scalars(), vec![Scalar::Local { slot: 0, kind: Kind::Color }]);
    let ctx = Ctx::new(0, source);
    assert_eq!(set.members(&game, &ctx, &Slots(vec![Value::Color(ir::Color::Red)])), vec![Target::Object(red)]);
    assert_eq!(set.members(&game, &ctx, &NoLocals), Vec::new());
}

#[test]
fn r607_2d_a_linked_choice_needs_a_link_in_the_definition_and_a_fitting_kind() {
    let (game, source, ..) = game();
    let support = Support::definition(game.def(source).unwrap(), Some(AbilityKey::printed(0)));
    // no "choose a color" on the source: nothing to link to
    assert_eq!(Set::compile(&permanents(vec![chosen(Kind::Color)]), &support, &mut linked(&support)).unwrap_err().0, "linked choice: unknown ability provenance or kind (607.5)");
    // a binder that hands back a value of another kind is refused at lowering
    let mut wrong = |_: &UnresolvedRef| Ok(Scalar::Local { slot: 0, kind: Kind::Cards });
    assert_eq!(Set::compile(&permanents(vec![chosen(Kind::Color)]), &support, &mut wrong).unwrap_err().0, "choice reference: unsupported scalar constraint");
}

#[test]
fn r107_1b_comparisons_stay_signed_and_only_the_final_amount_is_clamped() {
    let (game, source, red, green) = game();
    let support = Support::definition(game.def(source).unwrap(), None);
    let ctx = Ctx::new(0, source);
    let big = permanents(vec![ir::Filter::Atom(Said::Stat { stat: ir::Stat::Power, cmp: Cmp::Ge, value: ir::Amount::Const(3) })]);
    let set = Set::compile(&big, &support, &mut linked(&support)).unwrap();
    assert!(set.contains(&game, &ctx, &NoLocals, Target::Object(green)));
    assert!(!set.contains(&game, &ctx, &NoLocals, Target::Object(red)));
    // a land has no power: the comparison is false of it, not an error
    assert!(!set.contains(&game, &ctx, &NoLocals, Target::Object(source)));
    // a player is never what describes an object
    assert!(!set.contains(&game, &ctx, &NoLocals, Target::Player(1)));

    let total = ir::Amount::Aggregate { agg: Aggregate::Total, stat: ir::Stat::Power, among: Box::new(permanents(Vec::new())) };
    assert_eq!(Amount::compile(&total, &support, &mut linked(&support)).unwrap().evaluate(&game, &ctx, &NoLocals), 5);
    let count = ir::Amount::Count(Box::new(big));
    assert_eq!(Amount::compile(&count, &support, &mut linked(&support)).unwrap().evaluate(&game, &ctx, &NoLocals), 1);
}

#[test]
fn r107_3_x_is_an_amount_only_where_the_text_has_one() {
    let (game, source, ..) = game();
    let support = Support::definition(game.def(source).unwrap(), None);
    assert_eq!(Amount::compile(&ir::Amount::X, &support, &mut linked(&support)).unwrap_err().0, "amount: X");
    let support = support.with_x(true);
    let x = Amount::compile(&ir::Amount::X, &support, &mut linked(&support)).unwrap();
    assert_eq!(x.evaluate(&game, &Ctx { x: Some(4), ..Ctx::new(0, source) }, &NoLocals), 4);
}

#[test]
fn unsupported_operands_are_refused_at_lowering_whatever_the_game_holds() {
    let (game, source, ..) = game();
    let support = Support::definition(game.def(source).unwrap(), None);
    let mut bind = linked(&support);
    let base = ir::Amount::Aggregate { agg: Aggregate::Greatest, stat: ir::Stat::BasePower, among: Box::new(permanents(Vec::new())) };
    assert_eq!(Amount::compile(&base, &support, &mut bind).unwrap_err().0, "amount: BasePower");
    // an unsupported amount inside a description inside a union arm
    let nested = permanents(vec![ir::Filter::Atom(Said::Stat { stat: ir::Stat::ManaValue, cmp: Cmp::Le, value: base.clone() })]);
    let union = ObjectSet::of(ir::Domain::Union(vec![ObjectSet::of(ir::Domain::Player), nested]));
    assert_eq!(Set::compile(&union, &support, &mut bind).unwrap_err().0, "amount: BasePower");
    assert_eq!(Set::compile(&ObjectSet::of(ir::Domain::Union(Vec::new())), &support, &mut bind).unwrap_err().0, "an empty selector union");
    // a card is described for an action that says where to look; a permanent is not a card
    assert!(Cards::compile(&ObjectSet::of(ir::Domain::Card), &support, &mut bind).is_ok());
    assert_eq!(Cards::compile(&ObjectSet::of(ir::Domain::Permanent), &support, &mut bind).unwrap_err().0, "noun phrase: a card with an owner, a controller or a zone");
    assert_eq!(Set::compile(&ObjectSet::of(ir::Domain::Card), &support, &mut bind).unwrap_err().0, "noun phrase: Card");
}

/// A frame's slots and its occurrence's inputs, without a frame.
struct Bound(Vec<Value>, Vec<(Input, Vec<Target>)>);

impl Locals for Bound {
    fn local(&self, slot: usize) -> Option<&Value> {
        self.0.get(slot)
    }

    fn input(&self, key: Input) -> Option<&[Target]> {
        self.1.iter().find(|(k, _)| *k == key).map(|(_, named)| named.as_slice())
    }
}

/// A binder with a subject in scope: every reference back is to this one.
struct Scoped(Subject);

impl Binding for Scoped {
    fn scalar(&mut self, _: &UnresolvedRef) -> Res<Scalar> {
        unsupported("no choice in scope")
    }

    fn subject(&mut self, _: &ir::Selector) -> Res<Subject> {
        Ok(self.0.clone())
    }
}

fn that(domain: ir::Domain) -> ir::Selector {
    ir::Selector::new(Quant::Definite(Ref::Anaphor(ir::selector::Anaphor::That)), ObjectSet::of(domain))
}

fn you() -> ir::Selector {
    ir::Selector::new(Quant::Definite(Ref::You), ObjectSet::of(ir::Domain::Player))
}

fn hand_of(who: ir::Selector) -> ir::Amount {
    let zone = ir::selector::ZoneSpec::of(ir::Zone::Hand, Some(who));
    ir::Amount::Count(Box::new(ObjectSet { zone: Some(zone), ..ObjectSet::of(ir::Domain::Card) }))
}

#[test]
fn r608_2h_a_number_of_a_retained_object_is_of_that_object_after_it_has_moved() {
    // "Put target creature card from a graveyard onto the battlefield under your control.
    // You lose life equal to that card's mana value."
    let (mut game, source, red, _) = game();
    let card = game.put(1, 2, Zone::Graveyard);
    let def = game.def(source).unwrap().clone();
    let support = Support::definition(&def, None);
    let ctx = Ctx::new(0, source);
    let said = |stat| ir::Amount::Stat { of: Box::new(that(ir::Domain::Card)), stat };

    // with nothing in scope to be "that card" it is refused, not read from the source
    assert_eq!(Amount::compile(&said(ir::Stat::ManaValue), &support, &mut linked(&support)).unwrap_err().0, "reference: nothing in scope supplies it");
    let value = Amount::compile(&said(ir::Stat::ManaValue), &support, &mut Scoped(Subject::Local(0))).unwrap();
    assert_eq!(value.scalars(), vec![Scalar::Local { slot: 0, kind: Kind::Subjects }]);
    let held = Bound(vec![Value::Subjects(vec![Target::Object(card)])], Vec::new());
    assert_eq!(value.evaluate(&game, &ctx, &held), 3);

    // the card moves and is a new object (400.7); the number is still of the one retained
    let entered = game.move_to(card, Zone::Battlefield, 0).unwrap();
    assert_ne!(entered, card);
    assert_eq!(game.obj(card).zone, Zone::Gone);
    assert_eq!(value.evaluate(&game, &ctx, &held), 3);
    // an unfilled slot names nothing, and the number of nothing is zero
    assert_eq!(value.evaluate(&game, &ctx, &NoLocals), 0);

    // its owner and its controller are of the identity retained, as it last was
    let life = |who: Ref| ir::Amount::Life(Box::new(ir::Selector::new(Quant::Definite(who), ObjectSet::of(ir::Domain::Player))));
    game.players[1].life = 7;
    let owner = Amount::compile(&life(Ref::OwnerOf(Box::new(that(ir::Domain::Card)))), &support, &mut Scoped(Subject::Local(0))).unwrap();
    assert_eq!(owner.scalars(), vec![Scalar::Local { slot: 0, kind: Kind::Subjects }]);
    assert_eq!(owner.evaluate(&game, &ctx, &held), 7);

    // a creature that has left the battlefield: its power as it last was there
    let power = Amount::compile(&said(ir::Stat::Power), &support, &mut Scoped(Subject::Local(0))).unwrap();
    let held = Bound(vec![Value::Subjects(vec![Target::Object(red)])], Vec::new());
    game.move_to(red, Zone::Graveyard, 0).unwrap();
    assert_eq!(power.evaluate(&game, &ctx, &held), 2);
    let controller = Amount::compile(&life(Ref::ControllerOf(Box::new(that(ir::Domain::Permanent)))), &support, &mut Scoped(Subject::Local(0))).unwrap();
    assert_eq!(controller.evaluate(&game, &ctx, &held), 20);
}

#[test]
fn r608_2k_a_bound_players_zone_is_counted_whatever_became_of_the_source() {
    // "… deals damage to that player equal to the number of cards in their hand and you
    // gain 1 life for each card in your hand."
    let (mut game, source, ..) = game();
    for (player, cards) in [(0, 1), (1, 3)] {
        for _ in 0..cards {
            game.put(player, 1, Zone::Hand);
        }
    }
    let def = game.def(source).unwrap().clone();
    let support = Support::definition(&def, None);
    let ctx = Ctx::new(0, source);
    let key = Input::Argument(0);

    // "their hand" is nobody's with no occurrence in scope; "your hand" needs none
    assert_eq!(Amount::compile(&hand_of(that(ir::Domain::Player)), &support, &mut linked(&support)).unwrap_err().0, "reference: nothing in scope supplies it");
    let yours = Amount::compile(&hand_of(you()), &support, &mut linked(&support)).unwrap();
    let theirs = Amount::compile(&hand_of(that(ir::Domain::Player)), &support, &mut Scoped(Subject::Input(key))).unwrap();
    // an input is the occurrence's, not a slot to be filled before it is read
    assert!(theirs.scalars().is_empty());
    let occurrence = Bound(Vec::new(), vec![(Input::Subject, vec![Target::Object(source)]), (key, vec![Target::Player(1)])]);
    assert_eq!((yours.evaluate(&game, &ctx, &occurrence), theirs.evaluate(&game, &ctx, &occurrence)), (1, 3));

    // the source is gone: the player it named is the same player
    game.move_to(source, Zone::Graveyard, 0).unwrap();
    game.put(1, 2, Zone::Hand);
    assert_eq!((yours.evaluate(&game, &ctx, &occurrence), theirs.evaluate(&game, &ctx, &occurrence)), (1, 4));
    // the count is of the cards described: "creature cards in their hand"
    let creatures = match hand_of(that(ir::Domain::Player)) {
        ir::Amount::Count(set) => ir::Amount::Count(Box::new(ObjectSet { filter: ir::Filter::Atom(Said::Type(ir::CardType::Creature)), ..*set })),
        _ => unreachable!(),
    };
    let creatures = Amount::compile(&creatures, &support, &mut Scoped(Subject::Input(key))).unwrap();
    assert_eq!(creatures.evaluate(&game, &ctx, &occurrence), 4);
    // an occurrence without that input names nobody
    assert_eq!(theirs.evaluate(&game, &ctx, &NoLocals), 0);

    // that player's life, and the counters on the object the occurrence named
    let life = ir::Amount::Life(Box::new(that(ir::Domain::Player)));
    game.players[1].life = 13;
    assert_eq!(Amount::compile(&life, &support, &mut Scoped(Subject::Input(key))).unwrap().evaluate(&game, &ctx, &occurrence), 13);
    let counters = ir::Amount::Counters { on: Box::new(that(ir::Domain::Permanent)), kind: None };
    assert_eq!(Amount::compile(&counters, &support, &mut linked(&support)).unwrap_err().0, "reference: nothing in scope supplies it");
    let counters = Amount::compile(&counters, &support, &mut Scoped(Subject::Input(Input::Subject))).unwrap();
    assert_eq!(counters.evaluate(&game, &ctx, &occurrence), 0);
}

#[test]
fn r107_1b_nested_arithmetic_clamps_the_final_amount_not_its_components() {
    let (game,source,..)=game();
    let support=Support::definition(game.def(source).unwrap(),None);
    let ctx=Ctx::new(0,source);
    let difference=ir::Amount::Minus(Box::new(ir::Amount::Const(1)),Box::new(ir::Amount::Const(3)));
    let expression=ir::Amount::Plus(Box::new(difference.clone()),Box::new(ir::Amount::Const(4)));
    assert_eq!(Number::compile(&difference,&support,&mut linked(&support)).unwrap().evaluate(&game,&ctx,&NoLocals),-2);
    assert_eq!(Amount::compile(&difference,&support,&mut linked(&support)).unwrap().evaluate(&game,&ctx,&NoLocals),0);
    assert_eq!(Amount::compile(&expression,&support,&mut linked(&support)).unwrap().evaluate(&game,&ctx,&NoLocals),2);
}

#[test]
fn r613_8_compiled_dependency_reads_follow_nested_numbers_and_set_filters() {
    let (game, source, ..) = game();
    let support = Support::definition(game.def(source).unwrap(), None);
    let power = ir::Amount::Stat { of: Box::new(ir::Selector::new(Quant::Definite(Ref::This), ObjectSet::of(ir::Domain::Permanent))), stat: ir::Stat::Power };
    let count = |filter| ir::Amount::Count(Box::new(permanents(vec![filter])));
    let subtype = count(ir::Filter::Atom(Said::Subtype(ir::Subtype::from_name("Rat").unwrap())));
    let nested = ir::Amount::Plus(Box::new(ir::Amount::Const(1)), Box::new(power.clone()));
    let threshold = count(ir::Filter::Not(Box::new(ir::Filter::Atom(Said::Stat {
        stat: ir::Stat::ManaValue, cmp: Cmp::Ge, value: nested.clone(),
    }))));
    for (amount, reads) in [(subtype, false), (power, true), (nested, true), (threshold, true)] {
        assert_eq!(Amount::compile(&amount, &support, &mut linked(&support)).unwrap().reads_pt(), reads);
    }
}

#[test]
fn r107_1a_rounding_preserves_signed_intermediates_and_refuses_unspecified_rounding() {
    let (game,source,..)=game();let support=Support::definition(game.def(source).unwrap(),None);let ctx=Ctx::new(0,source);
    for (left,right,up,expected) in [(5,0,true,3),(5,0,false,2),(0,5,true,-2),(0,5,false,-3),(0,0,true,0),(4,0,true,2)] {
        let of=Box::new(ir::Amount::Minus(Box::new(ir::Amount::Const(left)),Box::new(ir::Amount::Const(right))));
        let amount=ir::Amount::Half{of:of.clone(),round_up:Some(up)};
        assert_eq!(Number::compile(&amount,&support,&mut linked(&support)).unwrap().evaluate(&game,&ctx,&NoLocals),expected);
        assert_eq!(Number::compile(&ir::Amount::Half{of,round_up:None},&support,&mut linked(&support)).unwrap_err().0, "amount: Half");
    }
}

#[test]
fn r608_2c_root_target_exclusion_does_not_rebind_nested_sets() {
    let (game, source, red, green) = game();
    let support = Support::definition(game.def(source).unwrap(), None);
    let mut nested = permanents(vec![ir::Filter::Atom(Said::Type(ir::CardType::Creature))]);
    nested.other = true;
    let mut root = permanents(vec![ir::Filter::Atom(Said::Stat {
        stat: ir::Stat::Power, cmp: Cmp::Eq,
        value: ir::Amount::Plus(Box::new(ir::Amount::Count(Box::new(nested))), Box::new(ir::Amount::Const(1))),
    })]);
    root.other = true;
    let mut bind = |r: &UnresolvedRef| Ok(Scalar::Local { slot: 0, kind: r.expected });
    let set = Set::compile_excluding(&root, &support, &mut bind, || Ok(Some(Subject::Local(0))), false).unwrap();
    assert_eq!(set.members(&game, &Ctx::new(0, source), &Slots(vec![Value::Subjects(vec![Target::Object(red)])])), vec![Target::Object(green)]);
}
