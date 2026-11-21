use super::*;
use crate::CardDef;
use crate::program::Program;
use crate::state::{Decision, ObjId, Zone};
use ir::action::Coord;
use ir::{ObjectSet, Selector};
use std::sync::Arc;

const G: ManaSymbol = ManaSymbol::Color(ir::Color::Green);
const R: ManaSymbol = ManaSymbol::Color(ir::Color::Red);

fn cost(symbols: &[ManaSymbol]) -> ManaCost {
    ManaCost(symbols.to_vec())
}

/// "{R} or {G}{G}": one of several fixed productions.
fn one_of(alternatives: &[&[ManaSymbol]]) -> ManaSpec {
    ManaSpec::OneOf(alternatives.iter().map(|symbols| ManaSpec::Symbols(cost(symbols))).collect())
}

/// A source and two creatures of player 0, and a frame of the source's text.
fn game() -> (Game, ObjId, Frame) {
    let source = CardDef::new("Source", "", "Land", "", None).unwrap();
    let bear = CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap();
    let mut game = Game::new(vec![source, bear], 2, 20);
    let source = game.put(0, 0, Zone::Battlefield);
    game.put(0, 1, Zone::Battlefield);
    game.put(0, 1, Zone::Battlefield);
    let frame = Frame::new(Arc::new(Program::new(Vec::new(), Vec::new()).unwrap()), Ctx::new(0, source));
    (game, source, frame)
}

fn compile(game: &Game, source: ObjId, spec: &ManaSpec) -> Res<Production> {
    Production::compile(spec, &Support::definition(game.def(source).unwrap(), None))
}

fn added(game: &Game) -> Vec<(ObjId, PlayerId, Vec<ManaSymbol>)> {
    let events = game.log.iter().filter_map(|e| match e {
        Event::ManaAdded { source, player, mana } => Some((*source, *player, mana.clone())),
        _ => None,
    });
    events.collect()
}

fn creatures() -> ir::Amount {
    let set = ObjectSet { filter: ir::Filter::All(vec![ir::Filter::Atom(ir::Atom::Type(ir::CardType::Creature))]), ..ObjectSet::of(ir::Domain::Permanent) };
    ir::Amount::Count(Box::new(set))
}

#[test]
fn r106_3_fixed_mana_is_added_as_one_event_of_its_source() {
    let (mut game, source, mut f) = game();
    let production = compile(&game, source, &ManaSpec::Symbols(cost(&[G, ManaSymbol::Colorless]))).unwrap();
    assert!(run(&mut game, &mut f, &production).is_ok());
    let pool = &game.players[0].pool;
    assert_eq!((pool.colors[ir::Color::Green as usize], pool.colorless, pool.total()), (1, 1, 2));
    assert_eq!(added(&game), vec![(source, 0, vec![G, ManaSymbol::Colorless])]);
}

#[test]
fn r608_2d_a_choice_of_mana_is_asked_before_any_is_added() {
    let (mut game, source, mut f) = game();
    let production = compile(&game, source, &one_of(&[&[R], &[G, G]])).unwrap();
    let Err(Halt::Ask(Decision::Choose { player: 0, choice: Choice::Mana { options } })) = run(&mut game, &mut f, &production) else {
        panic!("the player who adds the mana chooses which");
    };
    assert_eq!(options, vec![vec![R], vec![G, G]]);
    assert_eq!((game.players[0].pool.total(), added(&game).len()), (0, 0));

    // what was not offered is not added
    let mut wrong = f.clone();
    wrong.answer(Answer::Mana(vec![G]));
    assert!(matches!(run(&mut game, &mut wrong, &production), Err(Halt::Ask(_))));
    assert_eq!(game.players[0].pool.total(), 0);

    f.answer(Answer::Mana(vec![G, G]));
    assert!(run(&mut game, &mut f, &production).is_ok());
    assert_eq!(game.players[0].pool.colors[ir::Color::Green as usize], 2);
    assert_eq!(added(&game), vec![(source, 0, vec![G, G])]);
}

#[test]
fn r608_2h_an_amount_of_mana_is_counted_as_it_is_added() {
    let (mut game, source, mut f) = game();
    let power = compile(&game, source, &ManaSpec::AmountOf { symbol: G, amount: creatures() }).unwrap();
    assert!(run(&mut game, &mut f, &power).is_ok());
    assert_eq!(added(&game), vec![(source, 0, vec![G, G])]);

    // "for each": fixed mana repeats; a choice of color for each is not one production
    let support = Support::definition(game.def(source).unwrap(), None);
    let each = expr::Amount::compile(&creatures(), &support, &mut expr::linked(&support)).unwrap();
    let repeated = compile(&game, source, &ManaSpec::Symbols(cost(&[R, G]))).unwrap().each(each.clone()).unwrap();
    assert!(run(&mut game, &mut f, &repeated).is_ok());
    assert_eq!(added(&game)[1], (source, 0, vec![R, G, R, G]));
    assert_eq!(compile(&game, source, &one_of(&[&[R], &[G]])).unwrap().each(each).unwrap_err().0, "mana: for each with variable color choices");
}

#[test]
fn r106_1a_any_color_offers_the_five_colors_and_no_mana_asks_nothing() {
    let (mut game, source, mut f) = game();
    let one_color = compile(&game, source, &ManaSpec::Of { amount: creatures(), color: ManaColor::AnyOne }).unwrap();
    let Err(Halt::Ask(Decision::Choose { choice: Choice::Mana { options }, .. })) = run(&mut game, &mut f, &one_color) else {
        panic!("a color is chosen");
    };
    assert_eq!(options.len(), 5);
    assert!(options.iter().all(|o| o.len() == 2 && o[0] == o[1]));

    let mut f = Frame::new(f.program.clone(), f.ctx);
    let combination = compile(&game, source, &ManaSpec::Of { amount: ir::Amount::Const(2), color: ManaColor::Any(ManaTypes::Colors) }).unwrap();
    let Err(Halt::Ask(Decision::Choose { choice: Choice::Mana { options }, .. })) = run(&mut game, &mut f, &combination) else {
        panic!("colors are chosen");
    };
    assert_eq!(options.len(), 15);
    assert!(options.contains(&vec![R, G]) && !options.contains(&vec![G, R]));

    let mut f = Frame::new(f.program.clone(), f.ctx);
    let none = compile(&game, source, &ManaSpec::Of { amount: ir::Amount::Const(0), color: ManaColor::AnyOne }).unwrap();
    assert!(run(&mut game, &mut f, &none).is_ok());
    assert_eq!((game.players[0].pool.total(), added(&game).len()), (0, 0));
}

#[test]
fn r608_2h_a_counted_combination_of_colors_is_what_the_count_is_as_it_is_added() {
    // two creatures: two mana, each of any color
    let (mut game, source, mut f) = game();
    let counted = compile(&game, source, &ManaSpec::Of { amount: creatures(), color: ManaColor::Any(ManaTypes::Colors) }).unwrap();
    let options = counted.options(&game, &f.ctx, &f).unwrap();
    assert_eq!(options.len(), 15);
    assert!(options.contains(&vec![R, G]) && !options.contains(&vec![G, R]));

    // a third: every combination of three is still listed
    game.put(0, 1, Zone::Battlefield);
    assert_eq!(counted.options(&game, &f.ctx, &f).unwrap().len(), 35);

    // A fourth is not listed as combinations: the color of each mana is asked in turn, as
    // a mana ability is activated or as it resolves on the stack.
    game.put(0, 1, Zone::Battlefield);
    assert!(counted.options(&game, &f.ctx, &f).unwrap_err().0.contains("4 in any combination"));
    let offered = counted.offered(&game, &f.ctx, &f).unwrap();
    assert_eq!(offered, Offered::Units(4));
    for color in [R, G, R, G] {
        let Err(Halt::Ask(Decision::Choose { choice: Choice::Mana { options }, .. })) = run(&mut game, &mut f, &counted) else { panic!("one unit choice") };
        assert_eq!(options.len(), 5);
        assert!(options.iter().all(|o| o.len() == 1));
        assert_eq!((game.players[0].pool.total(), added(&game).len()), (0, 0));
        f.answer(Answer::Mana(vec![color]));
    }
    assert!(run(&mut game, &mut f, &counted).is_ok());
    assert_eq!(added(&game), vec![(source, 0, vec![R,G,R,G])]);

    // X is counted where the text says what X is (107.3c); none is no choice and no event
    let support = Support::definition(game.def(source).unwrap(), None).with_x(true);
    let x = Production::compile(&ManaSpec::Of { amount: ir::Amount::X, color: ManaColor::Any(ManaTypes::Colors) }, &support).unwrap();
    f.ctx.x = Some(0);
    assert_eq!(x.options(&game, &f.ctx, &f).unwrap(), vec![Vec::new()]);
    assert!(run(&mut game, &mut f, &x).is_ok());
    assert_eq!((game.players[0].pool.total(), added(&game).len()), (4, 1));
    f.ctx.x = Some(1);
    assert_eq!(x.options(&game, &f.ctx, &f).unwrap().len(), 5);
}

#[test]
fn r106_6_restricted_mana_remembers_whose_text_added_it() {
    let (mut game, source, mut f) = game();
    let def = game.def(source).unwrap().clone();
    let support = Support::definition(&def, None);
    let spell =Selector::new(ir::Quant::Indefinite(Count::Exactly(ir::Amount::Const(1))), ObjectSet::of(ir::Domain::Spell));
    let rider = |on: SpendOn| Rider::SpendOnly { on: Coord::one(on), uncounterable: true };
    let cast = rider(SpendOn::Cast(Coord::one(Who::One(spell))));
    let production = compile(&game, source, &ManaSpec::Symbols(cost(&[G]))).unwrap().spend_only(&cast, &support).unwrap();
    assert!(run(&mut game, &mut f, &production).is_ok());
    let pool = &game.players[0].pool;
    assert_eq!((pool.colors, pool.colorless), ([0; 5], 0));
    let [mana] = pool.restricted.as_slice() else { panic!("one restricted mana") };
    assert_eq!((mana.mana, mana.source, mana.controller, mana.uncounterable), (G, source, 0, true));
    assert_eq!(added(&game), vec![(source, 0, vec![G])]);

    // what the engine cannot tell the spending of is refused as the text is lowered
    let plain = compile(&game, source, &ManaSpec::Symbols(cost(&[G]))).unwrap();
    // CR 106.6: the produced restricted pool pays for spells, not abilities.
    let (spell, ability) = (crate::state::Spent::Spell(source, crate::card::CastAs::Whole), crate::state::Spent::Ability { source, keyword: None, mana: false });
    assert!(game.may_spend(&game.players[0].pool.restricted[0], spell) && !game.may_spend(&game.players[0].pool.restricted[0], ability));
    assert_eq!(plain.clone().spend_only(&rider(SpendOn::X), &support).unwrap_err().0, "spend this mana only: the X component (106.6)");
    assert_eq!(production.spend_only(&cast, &support).unwrap_err().0, "mana: restricted twice");
}

#[test]
fn unported_productions_are_refused_as_the_text_is_lowered() {
    let (game, source, _) = game();
    let refused = |spec: ManaSpec| compile(&game, source, &spec).unwrap_err().0;
    assert!(refused(ManaSpec::Symbols(cost(&[ManaSymbol::Generic(1)]))).contains("Generic"));
    assert!(refused(ManaSpec::Of { amount: ir::Amount::Const(1), color: ManaColor::Chosen }).contains("Chosen"));
    assert!(refused(ManaSpec::Of { amount: ir::Amount::Const(4), color: ManaColor::Any(ManaTypes::CommanderIdentity) }).contains("any combination"));
    // "four mana in any combination of colors" asks the color of each (Baxter Building)
    assert!(compile(&game, source, &ManaSpec::Of { amount: ir::Amount::Const(4), color: ManaColor::Any(ManaTypes::Colors) }).is_ok());
    // types the text names that nothing computes yet: an object's colors (105.2), the colors
    // among a set, and the type the sentence before produced
    let this = Selector::new(ir::Quant::Definite(ir::Ref::This), ObjectSet::of(ir::Domain::Permanent));
    let its_colors = || ManaTypes::ColorsOf(Box::new(this.clone()));
    assert_eq!(refused(ManaSpec::Of { amount: ir::Amount::Const(1), color: ManaColor::Any(its_colors()) }), "mana: of ColorsOf");
    assert_eq!(refused(ManaSpec::Of { amount: ir::Amount::Const(3), color: ManaColor::Any(its_colors()) }), "mana: of ColorsOf");
    let among = ManaTypes::ColorsAmong(Box::new(ObjectSet::of(ir::Domain::Permanent)));
    assert_eq!(refused(ManaSpec::Of { amount: ir::Amount::Const(1), color: ManaColor::Any(among) }), "mana: of ColorsAmong");
    assert_eq!(refused(ManaSpec::Of { amount: ir::Amount::Const(3), color: ManaColor::ThatType }), "mana: of ThatType");
    // a choice among productions is offered only among fixed mana ("{W} or one mana of the
    // chosen color", "X {G} or X {W}")
    let white = ManaSpec::Symbols(cost(&[ManaSymbol::Color(ir::Color::White)]));
    let described = ManaSpec::Of { amount: ir::Amount::Const(1), color: ManaColor::Chosen };
    assert_eq!(refused(ManaSpec::OneOf(vec![white.clone(), described])), "mana: one of several, with Of");
    let counted = ManaSpec::AmountOf { symbol: ManaSymbol::Color(ir::Color::Green), amount: ir::Amount::X };
    assert_eq!(refused(ManaSpec::OneOf(vec![counted, white])), "mana: one of several, with AmountOf");
    // X is an amount only where the text has one (107.3)
    assert!(refused(ManaSpec::Of { amount: ir::Amount::X, color: ManaColor::AnyOne }).contains("X"));
    let support = Support::definition(game.def(source).unwrap(), None).with_x(true);
    assert!(Production::compile(&ManaSpec::Of { amount: ir::Amount::X, color: ManaColor::AnyOne }, &support).is_ok());
}
