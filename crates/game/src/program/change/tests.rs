use super::*;
use crate::CardDef;
use crate::binding::Kind;
use crate::program::expr::{NoLocals, linked};
use crate::state::{ObjId, Zone};
use ir::action::{Characteristics, Granted, Keeps, SetTo, Setting};
use ir::reference::UnresolvedRef;
use ir::token::{KeywordInstance, KeywordParam, Pt};
use ir::{Atom, CardType, Domain, Keyword, ObjectSet, Stat};

/// A source and two creatures of player 0.
fn game() -> (Game, ObjId, Ctx<'static>) {
    let source = CardDef::new("Source", "", "Land", "", None).unwrap();
    let bear = CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap();
    let mut game = Game::new(vec![source, bear], 2, 20);
    let source = game.put(0, 0, Zone::Battlefield);
    game.put(0, 1, Zone::Battlefield);
    game.put(0, 1, Zone::Battlefield);
    (game, source, Ctx::new(0, source))
}

fn compile(game: &Game, source: ObjId, action: &Action) -> Res<Vec<Change>> {
    let support = Support::definition(game.def(source).unwrap(), None);
    Change::compile(action, &support, &mut linked(&support))
}

fn now(game: &Game, ctx: &Ctx, action: &Action) -> Vec<layers::Change> {
    compile(game, ctx.object_source(), action).unwrap().iter().map(|c| c.evaluate(game, ctx, &NoLocals)).collect()
}

fn creatures(atoms: Vec<Atom>) -> ObjectSet {
    let atoms = std::iter::once(Atom::Type(CardType::Creature)).chain(atoms);
    ObjectSet { filter: ir::Filter::All(atoms.map(ir::Filter::Atom).collect()), ..ObjectSet::of(Domain::Permanent) }
}

fn gets(minus: bool, power: ir::Amount, toughness: ir::Amount) -> Action {
    Action::Gets { power: ir::action::Delta { minus, amount: power }, toughness: ir::action::Delta { minus, amount: toughness } }
}

fn keyword(name: &str) -> Granted {
    Granted::Keyword(Box::new(KeywordInstance { keyword: Keyword::from_name(name).unwrap(), param: KeywordParam::None }))
}

fn base(p: Pt, t: Pt) -> Setting {
    Setting { stats: vec![Stat::Power, Stat::Toughness], to: SetTo::Profile(p, t) }
}

#[test]
fn r611_2d_a_counted_change_is_the_number_as_it_is_when_asked() {
    let (mut game, source, ctx) = game();
    let count = || ir::Amount::Count(Box::new(creatures(Vec::new())));
    let changes = compile(&game, source, &gets(false, count(), ir::Amount::Const(1))).unwrap();
    assert_eq!(changes[0].evaluate(&game, &ctx, &NoLocals), layers::Change::Pt(2, 1));
    game.put(0, 1, Zone::Battlefield);
    // the same compiled change, asked again
    assert_eq!(changes[0].evaluate(&game, &ctx, &NoLocals), layers::Change::Pt(3, 1));
    assert_eq!(now(&game, &ctx, &gets(true, ir::Amount::Const(0), count())), vec![layers::Change::Pt(0, -3)]);

    // X is an amount only where the text has one (107.3)
    assert_eq!(compile(&game, source, &gets(true, ir::Amount::X, ir::Amount::X)).unwrap_err().0, "amount: X");
    let support = Support::definition(game.def(source).unwrap(), None).with_x(true);
    let x = Change::compile(&gets(true, ir::Amount::X, ir::Amount::X), &support, &mut linked(&support)).unwrap();
    assert_eq!(x[0].evaluate(&game, &Ctx { x: Some(4), ..ctx }, &NoLocals), layers::Change::Pt(-4, -4));
}

#[test]
fn r613_what_the_text_says_outright_is_the_layers_own_change() {
    let (game, source, ctx) = game();
    let flying = crate::keyword::Held::plain(Keyword::from_name("Flying").unwrap());
    let gains = compile(&game, source, &Action::Gains(vec![keyword("Flying")])).unwrap();
    assert_eq!(gains, vec![Change::Fixed(layers::Change::Gains(vec![flying.clone()]))]);
    assert!(gains[0].scalars().is_empty());
    let has = Action::Static(Box::new(ir::statics::StaticAct::Has(vec![keyword("Flying")])));
    assert_eq!(now(&game, &ctx, &has), vec![layers::Change::Gains(vec![flying])]);

    let set = Action::SetStat { of: None, caused: false, set: base(Pt::Const(1), Pt::Const(4)) };
    assert_eq!(now(&game, &ctx, &set), vec![layers::Change::BasePt(1, 4)]);

    let blue = Action::Becomes(Becoming::Colors { colors: vec![Color::Blue], added: true });
    assert_eq!(now(&game, &ctx, &blue), vec![layers::Change::Colors { colors: vec![Color::Blue], added: true }]);

    // "becomes a 3/3 green creature in addition to its other types": one change in each layer
    let becomes = Characteristics {
        pt: Some((Pt::Const(3), Pt::Const(3))),
        colors: Some(vec![Color::Green]),
        types: vec![CardType::Creature],
        keeps: Keeps::OtherTypes,
        ..Characteristics::default()
    };
    let changes = now(&game, &ctx, &Action::Becomes(Becoming::Is(becomes.clone())));
    let types = layers::Types { types: vec![CardType::Creature], subtypes: Vec::new(), every_creature_type: false, keeps: true };
    // "It's still a land." keeps the prior types of a becoming that did not say so
    let replaces = Action::Becomes(Becoming::Is(Characteristics { keeps: Keeps::Nothing, ..becomes }));
    let replaced = compile(&game, source, &replaces).unwrap();
    assert_eq!(replaced[0], Change::Fixed(layers::Change::Types(layers::Types { keeps: false, ..types.clone() })));
    assert_eq!(Change::still(replaced)[0], Change::Fixed(layers::Change::Types(types.clone())));
    assert_eq!(changes, vec![
        layers::Change::Types(types),
        layers::Change::Colors { colors: vec![Color::Green], added: false },
        layers::Change::BasePt(3, 3),
    ]);
}

#[test]
fn what_the_engine_does_not_apply_is_refused_as_the_text_is_lowered() {
    let (game, source, ctx) = game();
    let refused = |a: Action| compile(&game, source, &a).unwrap_err().0;
    assert_eq!(refused(Action::SetStat { of: None, caused: false, set: base(Pt::X, Pt::X) }), "change: set a stat to anything but numbers");
    assert_eq!(refused(Action::Becomes(Becoming::Colors { colors: Color::ALL.to_vec(), added: false })), "becomes: all colors");
    assert_eq!(refused(Action::Gains(vec![Granted::Quoted("{T}: Add {G}.".into())])), "gains: Quoted");
    let named = Characteristics { name: Some("Bear".into()), ..Characteristics::default() };
    assert_eq!(refused(Action::Becomes(Becoming::Is(named))), "becomes: supertypes or a name");
    assert_eq!(refused(Action::LosesChoice(Vec::new())), "change: LosesChoice");

    // an X is read only where the text says what X is (107.3c)
    assert_eq!(now(&game, &ctx, &gets(false, ir::Amount::Const(2), ir::Amount::Const(2))), vec![layers::Change::Pt(2, 2)]);
    assert_eq!(compile(&game, source, &gets(false, ir::Amount::X, ir::Amount::X)).unwrap_err().0, "amount: X");

    // a chosen value the change reads is reported, for checking it is produced before
    let chosen = creatures(vec![Atom::Reference(UnresolvedRef::choice(Kind::Color))]);
    let support = Support::definition(game.def(source).unwrap(), None);
    let mut bind = |r: &UnresolvedRef| Ok(Scalar::Local { slot: 1, kind: r.expected });
    let reads = Change::compile(&gets(false, ir::Amount::Count(Box::new(chosen)), ir::Amount::Const(0)), &support, &mut bind).unwrap();
    assert_eq!(reads[0].scalars(), vec![Scalar::Local { slot: 1, kind: Kind::Color }]);
}
