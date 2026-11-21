use super::*;
use crate::CardDef;
use crate::binding::Kind;
use crate::program::expr::{NoLocals, linked};
use crate::state::{ObjId, Zone};
use ir::action::Coord;
use ir::reference::UnresolvedRef;
use ir::{Atom, Conj, Domain, ObjectSet, Status};

/// Player 0 has the source (a land) and a Goblin; player 1 has two Elves.
fn game() -> (Game, ObjId, Ctx<'static>) {
    let source = CardDef::new("Source", "", "Land", "", None).unwrap();
    let goblin = CardDef::new("Goblin", "{R}", "Creature — Goblin", "", Some((2, 1))).unwrap();
    let elf = CardDef::new("Elf", "{G}", "Creature — Elf", "", Some((1, 1))).unwrap();
    let mut game = Game::new(vec![source, goblin, elf], 2, 20);
    let source = game.put(0, 0, Zone::Battlefield);
    game.put(0, 1, Zone::Battlefield);
    game.put(1, 2, Zone::Battlefield);
    game.put(1, 2, Zone::Battlefield);
    (game, source, Ctx::new(0, source))
}

fn compile(game: &Game, source: ObjId, c: &ir::Condition) -> Res<Condition> {
    let support = Support::definition(game.def(source).unwrap(), None);
    Condition::compile(c, &support, &mut linked(&support))
}

fn holds(game: &Game, ctx: &Ctx, c: &ir::Condition) -> bool {
    compile(game, ctx.object_source(), c).unwrap().evaluate(game, ctx, &NoLocals)
}

fn of(domain: Domain, atoms: Vec<Atom>) -> ObjectSet {
    ObjectSet { filter: ir::Filter::All(atoms.into_iter().map(ir::Filter::Atom).collect()), ..ObjectSet::of(domain) }
}

fn creatures() -> ObjectSet {
    of(Domain::Permanent, vec![Atom::Type(ir::CardType::Creature)])
}

fn count(count: Count, set: ObjectSet) -> Selector {
    Selector::new(Quant::Indefinite(count), set)
}

fn n(n: u32) -> ir::Amount {
    ir::Amount::Const(n)
}

fn an_opponent() -> Selector {
    count(Count::Exactly(n(1)), of(Domain::Player, vec![Atom::Opponent]))
}

fn each_player() -> Selector {
    Selector::new(Quant::Each, ObjectSet::of(Domain::Player))
}

fn controls(player: Selector, what: Vec<Selector>, negated: bool) -> ir::Condition {
    ir::Condition::Controls { player: Box::new(player), what: Coord { items: what, conj: Conj::Or }, negated }
}

#[test]
fn r107_1_a_determiner_says_how_many_a_player_controls() {
    let (game, _, ctx) = game();
    let creature = |c| count(c, creatures());
    // a bare count is a lower bound; "exactly" is that many and no more
    assert!(holds(&game, &ctx, &controls(Selector::you(), vec![creature(Count::Exactly(n(1)))], false)));
    assert!(!holds(&game, &ctx, &controls(Selector::you(), vec![creature(Count::AtLeast(n(2)))], false)));
    assert!(holds(&game, &ctx, &controls(an_opponent(), vec![creature(Count::Exactly(n(1)))], false)));
    assert!(!holds(&game, &ctx, &controls(an_opponent(), vec![creature(Count::Precisely(n(1)))], false)));
    assert!(holds(&game, &ctx, &controls(an_opponent(), vec![creature(Count::Range { min: Box::new(n(2)), max: Box::new(n(3)) })], false)));
    assert!(holds(&game, &ctx, &controls(Selector::you(), vec![creature(Count::UpTo(n(1)))], false)));
    assert!(!holds(&game, &ctx, &controls(Selector::you(), vec![Selector::new(Quant::No, creatures())], false)));
    assert!(holds(&game, &ctx, &controls(Selector::you(), vec![creature(Count::AtLeast(n(2)))], true)));
    // several noun phrases are alternatives
    let lands = count(Count::AtLeast(n(1)), of(Domain::Permanent, vec![Atom::Type(ir::CardType::Land)]));
    assert!(holds(&game, &ctx, &controls(Selector::you(), vec![creature(Count::AtLeast(n(2))), lands], false)));
}

#[test]
fn a_condition_of_each_player_holds_of_all_and_of_a_player_of_one() {
    let (game, _, ctx) = game();
    let two = || vec![count(Count::AtLeast(n(2)), creatures())];
    let a_player = count(Count::Exactly(n(1)), ObjectSet::of(Domain::Player));
    assert!(holds(&game, &ctx, &controls(a_player, two(), false)));
    assert!(!holds(&game, &ctx, &controls(each_player(), two(), false)));
    assert!(holds(&game, &ctx, &controls(each_player(), vec![count(Count::AtLeast(n(1)), creatures())], false)));
}

#[test]
fn r608_2h_what_exists_and_what_a_thing_is_are_asked_of_the_game_as_it_is() {
    let (mut game, source, ctx) = game();
    let exists = |c| ir::Condition::Exists { what: Box::new(count(c, creatures())) };
    assert!(holds(&game, &ctx, &exists(Count::AtLeast(n(3)))));
    assert!(!holds(&game, &ctx, &exists(Count::AtLeast(n(4)))));

    let this = || Box::new(Selector::new(Quant::Definite(Ref::This), ObjectSet::of(Domain::Permanent)));
    let tapped = |negated| ir::Condition::Is { what: this(), pred: Predicate::Adjective(ir::Filter::Atom(Atom::Status(Status::Tapped))), negated };
    let compiled = compile(&game, source, &tapped(false)).unwrap();
    assert!(!compiled.evaluate(&game, &ctx, &NoLocals));
    assert!(holds(&game, &ctx, &tapped(true)));
    game.obj_mut(source).tapped = true;
    // the same compiled question, asked again
    assert!(compiled.evaluate(&game, &ctx, &NoLocals));

    let a_creature = |negated| ir::Condition::Is { what: this(), pred: Predicate::A(creatures()), negated };
    assert!(!holds(&game, &ctx, &a_creature(false)));
    assert!(holds(&game, &ctx, &a_creature(true)));
    assert!(holds(&game, &ctx, &ir::Condition::And(vec![tapped(false), a_creature(true)])));
    assert!(!holds(&game, &ctx, &ir::Condition::And(vec![tapped(false), a_creature(false)])));
    assert!(holds(&game, &ctx, &ir::Condition::Or(vec![tapped(true), a_creature(true)])));
}

#[test]
fn r102_1_whose_turn_it_is_is_asked_of_the_text_s_controller() {
    let (mut game, _, ctx) = game();
    let turn = |whose: Selector, negated| ir::Condition::Turn { whose: Box::new(whose), negated };
    game.active = 0;
    assert!(holds(&game, &ctx, &turn(Selector::you(), false)));
    assert!(!holds(&game, &ctx, &turn(an_opponent(), false)));
    assert!(holds(&game, &ctx, &turn(an_opponent(), true)));
    game.active = 1;
    assert!(!holds(&game, &ctx, &turn(Selector::you(), false)));
    assert!(holds(&game, &ctx, &turn(an_opponent(), false)));
    assert_eq!(compile(&game, ctx.object_source(), &turn(each_player(), false)).unwrap_err().0, "reference: nothing in scope supplies it");
}

#[test]
fn r119_1_compared_life_totals_stay_signed_and_keep_their_determiner() {
    let (mut game, _, ctx) = game();
    let life = |who: Selector| ir::Amount::Life(Box::new(who));
    let compare = |left, cmp, right| ir::Condition::Compare { left, cmp, right };
    let starting = || ir::Amount::StartingLife(Box::new(Selector::you()));
    assert!(holds(&game, &ctx, &compare(life(Selector::you()), Cmp::Eq, starting())));
    game.players[0].life = -3;
    game.players[1].life = 25;
    // a negative life total is less than zero, not zero (107.1b is of results, not of these)
    assert!(holds(&game, &ctx, &compare(life(Selector::you()), Cmp::Lt, n(0))));
    assert!(holds(&game, &ctx, &compare(life(an_opponent()), Cmp::Gt, starting())));
    // one player is enough of "a player"; "each player" is all of them, on either side
    let a_player = || count(Count::Exactly(n(1)), ObjectSet::of(Domain::Player));
    assert!(holds(&game, &ctx, &compare(life(a_player()), Cmp::Ge, n(21))));
    assert!(!holds(&game, &ctx, &compare(life(each_player()), Cmp::Ge, n(21))));
    assert!(holds(&game, &ctx, &compare(n(0), Cmp::Le, life(a_player()))));
    assert!(!holds(&game, &ctx, &compare(n(0), Cmp::Le, life(each_player()))));
    // a count against a count
    assert!(holds(&game, &ctx, &compare(ir::Amount::Count(Box::new(creatures())), Cmp::Eq, n(3))));
}

#[test]
fn what_is_not_asked_of_the_state_is_refused_as_the_text_is_lowered() {
    let (game, source, ctx) = game();
    let refused = |c: ir::Condition| compile(&game, source, &c).unwrap_err().0;
    let and = ir::Condition::Controls {
        player: Box::new(Selector::you()),
        what: Coord { items: vec![count(Count::Exactly(n(1)), creatures()), count(Count::Exactly(n(1)), creatures())], conj: Conj::And },
        negated: false,
    };
    assert_eq!(refused(and), "condition: control of a conjunction of noun phrases");
    let it = Selector::new(Quant::Definite(Ref::Granting), ObjectSet::of(Domain::Permanent));
    assert_eq!(refused(ir::Condition::Is { what: Box::new(it), pred: Predicate::A(creatures()), negated: false }), "condition: is, of \"Definite\"");
    let this = Selector::new(Quant::Definite(Ref::This), ObjectSet::of(Domain::Permanent));
    assert_eq!(refused(ir::Condition::Is { what: Box::new(this), pred: Predicate::InOpeningHand, negated: false }), "condition: is InOpeningHand");
    assert_eq!(refused(controls(Selector::new(Quant::No, ObjectSet::of(Domain::Player)), vec![count(Count::Exactly(n(1)), creatures())], false)), "condition: of \"No\" player");
    assert_eq!(refused(ir::Condition::Exists { what: Box::new(Selector::new(Quant::Each, creatures())) }), "condition: how many, of \"Each\"");
    // one refusal refuses the whole: nothing is left to fail when it is asked
    let turn = ir::Condition::Turn { whose: Box::new(Selector::you()), negated: false };
    let x = ir::Condition::Compare { left: ir::Amount::X, cmp: Cmp::Ge, right: n(1) };
    assert_eq!(compile(&game, source, &ir::Condition::Or(vec![turn.clone(), x.clone()])).unwrap_err().0, "amount: X");
    // the adapter for text not kept lowered is the same compiler
    assert!(crate::condition::holds(&game, &ctx, &turn).is_ok());
    assert_eq!(crate::condition::holds(&game, &ctx, &x).unwrap_err().0, "amount: X");

    // a chosen value the condition reads is reported, for checking it is produced before
    let chosen = of(Domain::Permanent, vec![Atom::Reference(UnresolvedRef::choice(Kind::Color))]);
    let support = Support::definition(game.def(source).unwrap(), None);
    let mut bind = |r: &UnresolvedRef| Ok(Scalar::Local { slot: 2, kind: r.expected });
    let reads = Condition::compile(&ir::Condition::Exists { what: Box::new(count(Count::AtLeast(n(1)), chosen)) }, &support, &mut bind).unwrap();
    assert_eq!(reads.scalars(), vec![Scalar::Local { slot: 2, kind: Kind::Color }]);
}
