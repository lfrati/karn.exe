use super::*;
use crate::CardDef;
use crate::binding::Kind;
use crate::program::expr::linked;
use crate::state::Zone;
use ir::reference::UnresolvedRef;
use ir::{Atom, CardType, Color, Deed};

/// A source of player 0, a green and a red creature of player 0, a green one of player 1.
fn game(text: &str) -> (Game, Ctx<'static>, [ObjId; 3]) {
    let source = CardDef::new("Source", "{2}", "Creature — Wall", text, Some((0, 4))).unwrap();
    let bear = CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap();
    let goblin = CardDef::new("Goblin", "{R}", "Creature — Goblin", "", Some((1, 1))).unwrap();
    let mut game = Game::new(vec![source, bear, goblin], 2, 20);
    let source = game.put(0, 0, Zone::Battlefield);
    let creatures = [game.put(0, 1, Zone::Battlefield), game.put(0, 2, Zone::Battlefield), game.put(1, 1, Zone::Battlefield)];
    (game, Ctx::new(0, source), creatures)
}

fn compile(game: &Game, ctx: &Ctx, action: &Action) -> Res<Rule> {
    let support = Support::definition(game.def(ctx.object_source()).unwrap(), None);
    Rule::compile(action, &support, &mut linked(&support))
}

fn creatures(atoms: Vec<Atom>) -> ObjectSet {
    let atoms = std::iter::once(Atom::Type(CardType::Creature)).chain(atoms);
    ObjectSet { filter: ir::Filter::All(atoms.map(ir::Filter::Atom).collect()), ..ObjectSet::of(Domain::Permanent) }
}

fn a(set: ObjectSet) -> Selector {
    Selector::new(Quant::Indefinite(Count::Exactly(Amount::Const(1))), set)
}

fn cant(event: EventAction) -> Action {
    Action::Cant(vec![Forbidden { event, within: None }])
}

fn attacks() -> EventAction {
    EventAction::Attacks { alone: false, defender: Vec::new(), with: None }
}

fn object(o: ObjId) -> Vec<Target> {
    vec![Target::Object(o)]
}

#[test]
fn r604_2_a_static_ability_is_the_same_rule_kept_lowered() {
    let (game, ctx, [bear, _, theirs]) = game("This creature can't block.");
    assert!(game.cant_block(ctx.object_source(), theirs, &[ctx.object_source()]));
    assert!(!game.cant_block(bear, theirs, &[bear]));
    assert!(!game.cant_attack(ctx.object_source(), Target::Player(1)));

    let (game, ctx, [bear, ..]) = self::game("This creature attacks each combat if able.");
    assert!(game.must_attack(ctx.object_source()));
    assert!(!game.must_attack(bear));
}

#[test]
fn r611_2a_a_kept_rule_lasts_until_the_cleanup_step() {
    let (mut game, ctx, [bear, goblin, theirs]) = game("");
    let rule = compile(&game, &ctx, &cant(attacks())).unwrap();
    assert!(rule.scalars().is_empty());
    let lifetime = Lifetime::compile(&[Modifier::Duration(ir::effect::Duration::ThisTurn)], &cant(attacks())).unwrap();
    assert_eq!(lifetime, Lifetime::ThisTurn);
    game.keep(ctx.clone(), Affected::Things(object(bear)), rule, lifetime, Vec::new());
    assert!(game.cant_attack(bear, Target::Player(1)));
    assert!(!game.cant_attack(goblin, Target::Player(1)));

    // "can't be blocked except by two or more creatures": counted over all that block it
    let two = Selector::new(Quant::Indefinite(Count::AtLeast(Amount::Const(2))), creatures(Vec::new()));
    let menace = cant(EventAction::BecomesBlocked { by: vec![two], except: true });
    game.keep(ctx.clone(), Affected::Things(object(theirs)), compile(&game, &ctx, &menace).unwrap(), Lifetime::ThisTurn, Vec::new());
    assert!(game.cant_block(bear, theirs, &[bear]));
    assert!(!game.cant_block(bear, theirs, &[bear, goblin]));

    // a requirement names its window inside itself
    let must = Action::Static(Box::new(StaticAct::IfAble { what: vec![attacks()], when: Some(Within::ThisTurn) }));
    assert_eq!(Lifetime::compile(&[], &must), Ok(Lifetime::ThisTurn));
    game.keep(ctx.clone(), Affected::Things(object(goblin)), compile(&game, &ctx, &must).unwrap(), Lifetime::ThisTurn, Vec::new());
    assert!(game.must_attack(goblin));

    game.end_turn_rules();
    assert!(!game.cant_attack(bear, Target::Player(1)));
    assert!(!game.cant_block(bear, theirs, &[bear]));
    assert!(!game.must_attack(goblin));
}

#[test]
fn r502_3_a_next_untap_step_is_each_players_first_since_it_began() {
    let (mut game, ctx, [bear, _, theirs]) = game("");
    let stays = Action::DoesntUntap { whose: UntapWhose::Controller, next: true };
    let lifetime = Lifetime::compile(&[], &stays).unwrap();
    assert_eq!(lifetime, Lifetime::NextUntap);
    let rule = compile(&game, &ctx, &stays).unwrap();
    game.keep(ctx.clone(), Affected::Things(vec![Target::Object(bear), Target::Object(theirs)]), rule, lifetime, Vec::new());
    assert!(game.kept_tapped(bear) && game.kept_tapped(theirs));
    assert!(!game.kept_tapped(ctx.object_source()));

    // the cleanup step does not end it; a player's untap step passing does, for theirs
    game.end_turn_rules();
    game.untap_step_passed();
    assert!(!game.kept_tapped(bear));
    game.active = 1;
    assert!(game.kept_tapped(theirs));
    game.untap_step_passed();
    assert!(!game.kept_tapped(theirs));
    assert!(game.lasting.is_empty());

    // "during your next untap step" is over with its controller's
    let yours = Action::DoesntUntap { whose: UntapWhose::Yours, next: true };
    game.keep(ctx.clone(), Affected::Things(object(bear)), compile(&game, &ctx, &yours).unwrap(), Lifetime::NextUntap, Vec::new());
    assert!(!game.kept_tapped(bear));
    game.untap_step_passed();
    assert_eq!(game.lasting.len(), 1);
    game.active = 0;
    assert!(game.kept_tapped(bear));
    game.untap_step_passed();
    assert!(game.lasting.is_empty());
}

#[test]
fn a_kept_rule_reads_what_its_frame_held_when_it_began() {
    let (mut game, ctx, [bear, goblin, theirs]) = game("");
    let chosen = creatures(vec![Atom::Reference(UnresolvedRef::choice(Kind::Color))]);
    let action = cant(EventAction::Blocks { blocked: Some(Box::new(a(chosen))) });
    let support = Support::definition(game.def(ctx.object_source()).unwrap(), None);
    let mut bind = |r: &UnresolvedRef| Ok(Scalar::Local { slot: 1, kind: r.expected });
    let rule = Rule::compile(&action, &support, &mut bind).unwrap();
    assert_eq!(rule.scalars(), vec![Scalar::Local { slot: 1, kind: Kind::Color }]);

    game.keep(ctx, Affected::Things(object(theirs)), rule, Lifetime::ThisTurn, vec![None, Some(Value::Color(Color::Red))]);
    assert!(game.cant_block(theirs, goblin, &[theirs]));
    assert!(!game.cant_block(theirs, bear, &[theirs]));
}

#[test]
fn r101_2_what_cant_happen_is_asked_before_it_does() {
    let (mut game, ctx, [bear, goblin, _]) = game("");
    let gains = |player| Fact::new(Verb::GainLife, Some(Target::Player(player)), Vec::new());
    let destroyed = |what| Fact::new(Verb::Destroy, None, vec![(Role::What, Target::Object(what))]);
    assert!(!game.forbidden(&gains(1)));

    let rule = compile(&game, &ctx, &cant(EventAction::GainsLife { amount: None })).unwrap();
    game.keep(ctx.clone(), Affected::Things(vec![Target::Player(1)]), rule, Lifetime::ThisTurn, Vec::new());
    assert!(game.forbidden(&gains(1)));
    assert!(!game.forbidden(&gains(0)));

    let passive = EventAction::Deed(Deed { verb: Verb::Destroy, passive: true, args: Vec::new() });
    game.keep(ctx.clone(), Affected::Things(object(bear)), compile(&game, &ctx, &cant(passive)).unwrap(), Lifetime::ThisTurn, Vec::new());
    assert!(game.forbidden(&destroyed(bear)));
    assert!(!game.forbidden(&destroyed(goblin)));

    // "can't be the target of spells": of a spell, not of an ability (115.1)
    let spells = cant(EventAction::BecomesTarget { of: Some(Box::new(Selector::new(Quant::All, ObjectSet::of(Domain::Spell)))) });
    game.keep(ctx.clone(), Affected::Things(object(goblin)), compile(&game, &ctx, &spells).unwrap(), Lifetime::ThisTurn, Vec::new());
    let by = Ctx::new(1, bear);
    assert!(game.can_target(&by, Target::Object(goblin)));
    assert!(!game.can_target(&Ctx { targeting_spell: true, ..by.clone() }, Target::Object(goblin)));
    assert!(game.can_target(&Ctx { targeting_spell: true, ..by.clone() }, Target::Object(bear)));
}

#[test]
fn what_the_engine_does_not_hold_in_force_is_refused_as_the_text_is_lowered() {
    let (game, ctx, _) = game("");
    let refused = |a: Action| compile(&game, &ctx, &a).unwrap_err().0;
    let windowed = Action::Cant(vec![Forbidden { event: attacks(), within: Some(Within::ThisTurn) }]);
    assert_eq!(refused(windowed), "prohibition: a window of its own");
    let must = |what| Action::Static(Box::new(StaticAct::IfAble { what, when: None }));
    assert_eq!(refused(must(vec![EventAction::Blocks { blocked: None }])), "requirement: to block");
    // "attacks a player if able" names what it attacks (508.1d); "attacks target opponent if
    // able" names a target the rule doesn't read
    let a_player = EventAction::Attacks { alone: false, defender: vec![a(ObjectSet::of(Domain::Player))], with: None };
    assert!(compile(&game, &ctx, &must(vec![a_player])).is_ok());
    let target = Selector::new(Quant::Target(Count::Exactly(Amount::Const(1))), ObjectSet::of(Domain::Player));
    let targeted = EventAction::Attacks { alone: false, defender: vec![target], with: None };
    assert_eq!(refused(must(vec![targeted])), "requirement: to attack something in particular");
    assert_eq!(refused(Action::DoesntUntap { whose: UntapWhose::ThatPlayer, next: false }), "doesn't untap during the untap step of ThatPlayer");
    let abilities = cant(EventAction::BecomesTarget { of: Some(Box::new(Selector::new(Quant::All, ObjectSet::of(Domain::Ability)))) });
    assert_eq!(refused(abilities), "target prohibition: qualities of abilities or their sources");

    // only the lifetimes the engine ends
    assert_eq!(Lifetime::compile(&[], &cant(attacks())).unwrap_err().0, "a rule that lasts other than this turn");
    assert_eq!(Lifetime::compile(&[], &Action::DoesntUntap { whose: UntapWhose::Controller, next: false }).unwrap_err().0,
        "a rule that lasts other than this turn");
    assert!(Game::keeps(&cant(attacks())) && !Game::keeps(&Action::LosesChoice(Vec::new())));
}


#[test]
fn r113_8_sacrifice_cause_uses_the_ability_controller_not_owner_or_current_source_controller() {
    use crate::Move;
    use crate::state::{Answer, Choice, Decision};
    let preserver = CardDef::new("Tajuru Preserver", "{1}{G}", "Creature — Elf Shaman",
        "Spells and abilities your opponents control can't cause you to sacrifice permanents.", Some((2, 1))).unwrap();
    let altar = CardDef::new("Altar", "{0}", "Artifact", "{0}: Target player sacrifices a creature.", None).unwrap();
    let bear = CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap();
    for def in [&preserver, &altar, &bear] { crate::play::check(def).unwrap(); }
    for controller in [0, 1] {
        let mut g = Game::new(vec![preserver.clone(), altar.clone(), bear.clone()], 2, 20);
        let protection = g.put(0, 0, Zone::Battlefield);
        let victim = g.put(0, 2, Zone::Battlefield);
        let source = g.put(1 - controller, 1, Zone::Battlefield);
        g.obj_mut(source).controller = controller;
        while g.priority != controller { g.act(g.priority, Move::Pass).unwrap(); }
        g.act(controller, Move::Activate { source, ability: 0, targets: vec![vec![Target::Player(0)]], paying: vec![] }).unwrap();
        g.obj_mut(source).controller = 1 - controller;
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        if controller == 0 {
            assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::Objects { .. } })));
            g.act(0, Move::Choose(Answer::Objects(vec![victim]))).unwrap();
            assert!(!g.battlefield.contains(&victim));
        } else {
            assert!(g.decision.is_none());
            assert!(g.battlefield.contains(&victim));
        }
        assert!(g.battlefield.contains(&protection));
    }
}

#[test]
fn r611_3_causal_prohibition_follows_its_current_controller_not_its_owner() {
    let (mut g, ctx, [ours, _, theirs]) = game("Spells and abilities your opponents control can't cause you to sacrifice permanents.");
    crate::play::check(g.def(ctx.object_source()).unwrap()).unwrap();
    let source = g.put(0, 1, Zone::Battlefield);
    let cause = crate::event::Cause { source, controller: 0 };
    assert!(g.can_cause_sacrifice(cause, 0, ours));
    assert!(g.can_cause_sacrifice(cause, 1, theirs));
    g.obj_mut(ctx.object_source()).controller = 1;
    assert!(g.can_cause_sacrifice(cause, 0, ours));
    assert!(!g.can_cause_sacrifice(cause, 1, theirs));
}
