//! Direct-instruction checks: these are verified programs, not admitted cards.
//! CR 107.1b keeps signed intermediate values and uses zero for negative action counts.
//! Unrepresentable values are support refusals, never substitute Magic quantities.
use crate::program::*;
use super::{Amount, Number};
use crate::CardDef;

const RANGE: &str = "quantity exceeds the supported numerical range (107.1b)";
const EXPRESSION_RANGE: &str = "quantity expression exceeds the supported numerical range (107.1b)";

#[derive(Clone, Copy, Debug)]
enum Caller { Arrange, ExtraTurns, SkipTurns, AddPhases, Continuous, CopyStack, CopyCards }
const CALLERS: [Caller; 7] = [Caller::Arrange, Caller::ExtraTurns, Caller::SkipTurns,
    Caller::AddPhases, Caller::Continuous, Caller::CopyStack, Caller::CopyCards];

fn instruction(caller: Caller, n: Amount) -> Instr {
    use action::Operand::{Controller, Source};
    match caller {
        Caller::Arrange => Instr::Arrange { players: Controller, verb: Verb::Scry, amount: n, optional: false },
        Caller::ExtraTurns => Instr::ExtraTurns { players: Controller, count: n, after: None, out: 0 },
        Caller::SkipTurns => Instr::SkipTurns { players: Controller, count: n },
        Caller::AddPhases => Instr::AddPhases { after: ir::action::AfterPhase::This, kinds: vec![ir::turn::Phase::Combat], count: n },
        Caller::Continuous => Instr::Continuous { objects: vec![Source], changes: vec![change::Change::Fixed(crate::layers::Change::Pt(1, -1))], each: Some(n), duration: change::Duration::Turn },
        Caller::CopyStack => Instr::CopyStack { cards: Source, times: n, retarget: false, out: 0 },
        Caller::CopyCards => Instr::CopyCards { cards: Source, times: n, out: 0 },
    }
}

fn fixture(caller: Caller, n: Amount) -> (Game, Frame, Instr) {
    let mut g = Game::new(vec![
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Spell", "{0}", "Instant", "Target player gains 1 life.", None).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
    ], 2, 20);
    let source = match caller {
        Caller::CopyStack => g.put(0, 1, Zone::Stack),
        Caller::CopyCards => g.put(0, 1, Zone::Hand),
        _ => g.put(0, 0, Zone::Battlefield),
    };
    for _ in 0..3 { g.put(0, 2, Zone::Library); }
    let instr = instruction(caller, n);
    let program = Program::new(vec![instr.clone()], vec![ActSpan { end: 1, moves: false, keeps_x: false }]).unwrap();
    let f = Frame::new(Arc::new(program), Ctx::new(0, source));
    (g, f, instr)
}

fn refuse(caller: Caller, n: Amount, reason: &str) {
    let (mut g, mut f, instr) = fixture(caller, n);
    let before = format!("{g:?}");
    let frame = format!("{f:?}");
    match step(&mut g, &mut f, &instr) {
        Err(Halt::Unsupported(error)) => assert_eq!(error.0, reason, "{caller:?}"),
        _ => panic!("{caller:?} must refuse before executing"),
    }
    // No disclosure, choice, object/turn allocation, effect, or retained procedure.
    assert_eq!(format!("{g:?}"), before, "{caller:?}");
    assert_eq!(format!("{f:?}"), frame, "{caller:?}");
}

fn sum(a: Number, b: Number) -> Amount { Amount(Number::Sum(Box::new(a), Box::new(b))) }
fn oversized() -> Amount { sum(Number::Const(u32::MAX), Number::Const(1)) }
fn overflowing_negative() -> Amount {
    // The exact expression exceeds i64 while computing a negative result; it
    // cannot silently saturate and then masquerade as a supported zero count.
    Amount(Number::Difference(Box::new(Number::Const(0)), Box::new(Number::Product(
        Box::new(Number::Const(u32::MAX)), Box::new(Number::Const(u32::MAX))))))
}

// A saturated negative result would read as a zero count and run: each caller
// must refuse the overflowing expression instead, one test per caller.
macro_rules! expression_refusal {
    ($name:ident, $caller:ident) => {
        #[test]
        fn $name() { refuse(Caller::$caller, overflowing_negative(), EXPRESSION_RANGE); }
    };
}
expression_refusal!(r107_1b_arrange_refuses_expression_overflow, Arrange);
expression_refusal!(r107_1b_extra_turns_refuses_expression_overflow, ExtraTurns);
expression_refusal!(r107_1b_skip_turns_refuses_expression_overflow, SkipTurns);
expression_refusal!(r107_1b_add_phases_refuses_expression_overflow, AddPhases);
expression_refusal!(r107_1b_continuous_refuses_expression_overflow, Continuous);
expression_refusal!(r107_1b_copy_stack_refuses_expression_overflow, CopyStack);
expression_refusal!(r107_1b_copy_cards_refuses_expression_overflow, CopyCards);

#[test]
fn r107_1b_all_seven_refuse_oversized_final_quantity_before_any_consequence() {
    for caller in CALLERS { refuse(caller, oversized(), RANGE); }
}

#[test]
fn r107_1b_signed_intermediate_is_used_and_only_negative_final_count_is_zero() {
    for caller in CALLERS {
        for n in [Amount::constant(0), Amount(Number::Difference(Box::new(Number::Const(1)), Box::new(Number::Const(2))))] {
            let (mut g, mut f, instr) = fixture(caller, n);
            let before = g.clone();
            assert!(step(&mut g, &mut f, &instr).is_ok(), "{caller:?}");
            assert_eq!(g.players[0].library, before.players[0].library);
            assert_eq!(g.stack, before.stack);
            assert_eq!(g.skips.len(), before.skips.len());
            assert_eq!(format!("{:?}", g.turn_schedule), format!("{:?}", before.turn_schedule));
            assert!(f.pending.is_none());
            if matches!(caller, Caller::Continuous) {
                assert_eq!(g.power(f.ctx.source.unwrap()), Some(2));
                assert_eq!(g.toughness(f.ctx.source.unwrap()), Some(2));
            }
        }
        // (1 - 3) + 3 = 1, not 3: no early zero-clamping (107.1b).
        let n = sum(Number::Difference(Box::new(Number::Const(1)), Box::new(Number::Const(3))), Number::Const(3));
        let (mut g, mut f, instr) = fixture(caller, n);
        match (caller, step(&mut g, &mut f, &instr)) {
            (Caller::Arrange, Err(Halt::Ask(Decision::Choose { choice: Choice::Arrange { cards, .. }, .. }))) => assert_eq!(cards.len(), 1),
            (_, Ok(())) => match caller {
                Caller::ExtraTurns => assert!(matches!(f.slot(0), Some(Value::CreatedTurns(turns)) if turns.len() == 1)), // 500.7
                Caller::SkipTurns => assert_eq!(g.skips.len(), 1), // 614.10a
                Caller::AddPhases => assert!(g.plan.modified()), // 500.8
                Caller::Continuous => { // 611.2d; the signed modifier itself is retained.
                    assert_eq!(g.power(f.ctx.source.unwrap()), Some(3));
                    assert_eq!(g.toughness(f.ctx.source.unwrap()), Some(1));
                }
                Caller::CopyStack | Caller::CopyCards => assert!(matches!(f.slot(0), Some(Value::Subjects(copies)) if copies.len() == 1)), // 707.10 / 707.12
                Caller::Arrange => unreachable!(),
            },
            _ => panic!("{caller:?} must execute the signed intermediate exactly"),
        }
    }
}

#[test]
fn r107_1b_r701_22_maximum_arrangement_retains_exact_group_across_clone() {
    let (mut g, mut f, instr) = fixture(Caller::Arrange, Amount(Number::X));
    f.ctx.x = Some(u32::MAX);
    let offered = g.top_cards(0, 3);
    assert!(matches!(step(&mut g, &mut f, &instr), Err(Halt::Ask(Decision::Choose { player: 0, choice: Choice::Arrange { cards, .. } })) if cards == offered));
    // Mutating X cannot change the retained request when its instruction resumes.
    for (mut branch, mut frame) in [(g.clone(), f.clone()), (g, f)] {
        frame.ctx.x = Some(0);
        frame.answer(Answer::Arrange { top: offered.clone(), away: vec![] });
        assert!(step(&mut branch, &mut frame, &instr).is_ok());
        assert_eq!(branch.top_cards(0, 3), offered);
        assert_eq!(branch.log.iter().filter(|e| matches!(e, crate::event::Event::Arranged { .. })).count(), 1);
    }
}

#[test]
fn r107_1b_r611_2d_scaling_refuses_before_installing_any_effect() {
    for n in [i32::MAX as u32 + 1, u32::MAX] {
        refuse(Caller::Continuous, Amount::constant(n), "continuous P/T change exceeds the supported numerical range (107.1b)");
    }
    let (mut g, mut f, mut instr) = fixture(Caller::Continuous, Amount::constant(i32::MAX as u32));
    if let Instr::Continuous { changes, .. } = &mut instr {
        *changes = vec![change::Change::Fixed(crate::layers::Change::Pt(0, 0)), change::Change::Fixed(crate::layers::Change::Pt(2, -2))];
    }
    let before = format!("{g:?}");
    match step(&mut g, &mut f, &instr) {
        Err(Halt::Unsupported(e)) => assert_eq!(e.0, "continuous P/T change exceeds the supported numerical range (107.1b)"),
        _ => panic!("must refuse the whole effect"),
    }
    assert_eq!(format!("{g:?}"), before);
}

#[test]
fn r107_1b_signed_scaling_checks_the_product_not_the_unsigned_count() {
    use crate::layers::Change;
    assert_eq!(Change::Pt(-1, 0).checked_times(2_147_483_648).unwrap(), Change::Pt(i32::MIN, 0));
    assert_eq!(Change::Pt(0, 0).checked_times(u32::MAX).unwrap(), Change::Pt(0, 0));
    assert_eq!(Change::Pt(1, -1).checked_times(i32::MAX as u32).unwrap(), Change::Pt(i32::MAX, -i32::MAX));
    // Do not query effective P/T here: adding this valid change to nonzero base
    // P/T is a separate numeric owner. These assert the exact represented modifier.
}

#[test]
fn r107_1b_public_resolution_refusal_restores_retained_choice_and_prior_effects() {
    // Synthetic verified programs deliberately stress each caller after a real
    // public cast and retained Scry choice. No Oracle-card reachability claim.
    for caller in CALLERS {
        let mut def = CardDef::new("Quantity witness", "{0}", "Instant", "You gain 1 life. Scry 1.", None).unwrap();
        let instrs = vec![
            Instr::Action { action: action::Action::GainLife { players: action::Operand::Controller, amount: 1.into() }, created: None, attempt: None },
            instruction(Caller::Arrange, 1.into()),
            instruction(caller, oversized()),
        ];
        let program = Program::new(instrs, (1..=3).map(|end| ActSpan { end, moves: false, keeps_x: false }).collect()).unwrap();
        def.programs[0].result = Ok(Body::Plain(Arc::new(program)));
        let land = CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap();
        let mut g = Game::new(vec![def, land], 2, 20);
        let spell = g.put(0, 0, Zone::Hand);
        let top = g.put(0, 1, Zone::Library);
        g.act(0, crate::Move::Cast { card: spell, targets: vec![] }).unwrap();
        for _ in 0..2 { g.act(g.priority, crate::Move::Pass).unwrap(); }
        assert_eq!(g.players[0].life, 21);
        assert!(matches!(&g.decision, Some(Decision::Choose { player: 0, choice: Choice::Arrange { cards, .. } }) if cards == &[top]));
        assert_eq!(g.observations(1).len(), 0, "opponent never receives the private look");
        let before = format!("{g:?}");
        for mut branch in [g.clone(), g] {
            for _ in 0..2 {
                assert_eq!(branch.act(0, crate::Move::Choose(Answer::Arrange { top: vec![top], away: vec![] })).unwrap_err(), RANGE, "{caller:?}");
                // Includes disclosures, RNG, stack, frame pc/slots/pending,
                // arrangements, and the already committed gain-life effect.
                assert_eq!(format!("{branch:?}"), before, "{caller:?}");
            }
        }
    }
}

#[test]
fn r707_10c_stack_copy_count_is_frozen_across_target_choices_and_clone() {
    let (mut g, mut f, mut instr) = fixture(Caller::CopyStack, Amount(Number::X));
    f.ctx.x = Some(2);
    let source = f.ctx.source.unwrap();
    g.obj_mut(source).targets = vec![vec![Target::Player(1)]];
    if let Instr::CopyStack { retarget, .. } = &mut instr { *retarget = true; }
    assert!(matches!(step(&mut g, &mut f, &instr), Err(Halt::Ask(Decision::Choose { choice: Choice::Retarget { .. }, .. }))));
    assert_eq!(g.stack, [source], "copy goes on stack only after its target choice");
    for (mut branch, mut frame) in [(g.clone(), f.clone()), (g, f)] {
        frame.ctx.x = Some(u32::MAX);
        frame.answer(Answer::No);
        assert!(matches!(step(&mut branch, &mut frame, &instr), Err(Halt::Ask(Decision::Choose { choice: Choice::Retarget { .. }, .. }))));
        assert_eq!(branch.stack.len(), 2);
        frame.answer(Answer::No);
        assert!(step(&mut branch, &mut frame, &instr).is_ok());
        assert_eq!(branch.stack.len(), 3, "exactly the original two copies");
        assert!(matches!(frame.slot(0), Some(Value::Subjects(copies)) if copies.len() == 2));
        assert_eq!(branch.log.iter().filter(|e| matches!(e, crate::event::Event::SpellCopied { .. })).count(), 2);
    }
}
