//! Lexical target-slot complements and target-dependent Controls conditions (608.2c).
use crate::support::cite;
use game::{CardDef, Game, Move, Target, Zone};
use game::state::Decision;

fn creature(name: &str, ty: &str, text: &str, pt: (i32, i32)) -> CardDef {
    CardDef::new(name, "{0}", ty, text, Some(pt)).unwrap()
}
fn bear() -> CardDef { creature("Bear", "Creature — Bear", "", (3, 3)) }
fn spell(text: &str) -> CardDef { CardDef::new("Probe", "{0}", "Sorcery", text, None).unwrap() }
fn pass(g: &mut Game) {
    for _ in 0..8 {
        if g.stack.is_empty() || g.decision.is_some() { break; }
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn cast(g: &mut Game, targets: Vec<Vec<Target>>, spending: &str) {
    g.players[0].pool.colors = [10; 5];
    let c = g.put(0, 0, Zone::Hand);
    g.act(0, crate::support::paid(spending, Move::Cast { card: c, targets })).unwrap();
    pass(g);
}

// No "other": isolates antecedent resolution from complement binding. The source adds
// another creature to my count; only the opponent's count may decide the suffix condition.
#[test]
fn suffix_controls_reads_the_targets_controller_before_guarded_actions() {
    let d = creature("Probe", "Creature — Human", "When this creature enters, target creature an opponent controls gets -2/-2 until end of turn if that opponent controls two or more creatures.", (2, 2));
    for (theirs, mine) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        let mut g = Game::new(vec![d.clone(), bear()], 2, 20);
        let t = g.put(1, 1, Zone::Battlefield);
        for _ in 0..theirs { g.put(1, 1, Zone::Battlefield); }
        for _ in 0..mine { g.put(0, 1, Zone::Battlefield); }
        cast(&mut g, vec![], "");
        assert!(matches!(g.decision, Some(Decision::Targets { .. })));
        g.act(0, Move::ChooseTargets(vec![vec![Target::Object(t)]])).unwrap();
        pass(&mut g);
        cite!(["608.2c"]; assert_eq!(g.power(t), Some(if theirs == 1 { 1 } else { 3 }), "theirs {theirs}, mine {mine}"));
    }
}

#[test]
fn one_optional_plural_slot_excludes_only_its_chosen_members_and_clones() {
    let d = spell("Up to two target creatures you control each get +2/+1 until end of turn. Each other creature you control gets +1/+1 until end of turn.");
    game::play::check(&d).unwrap();
    for chosen in 0..=2 {
        let mut g = Game::new(vec![d.clone(), bear()], 2, 20);
        let objects: Vec<_> = (0..3).map(|_| g.put(0, 1, Zone::Battlefield)).collect();
        let c = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card: c, targets: vec![objects[..chosen].iter().copied().map(Target::Object).collect()] }).unwrap();
        for mut g in [g.clone(), g] {
            pass(&mut g);
            for (i, o) in objects.iter().enumerate() {
                // Uncited (card text): "Up to two target creatures you control each get +2/+1 ... Each other creature you control gets +1/+1": other than the chosen targets
                assert_eq!((g.power(*o), g.toughness(*o)), if i < chosen { (Some(5), Some(4)) } else { (Some(4), Some(4)) });
            }
        }
    }
}

#[test]
fn player_targets_are_incompatible_but_artifacts_can_also_be_creatures() {
    let d = spell("Target player gains 1 life. Target creature you control gets +2/+1 until end of turn. Each other creature you control gets +1/+1 until end of turn.");
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d, bear()], 2, 20);
    let a = g.put(0, 1, Zone::Battlefield);
    let b = g.put(0, 1, Zone::Battlefield);
    cast(&mut g, vec![vec![Target::Player(0)], vec![Target::Object(a)]], "");
    // Uncited (card text): "Target creature you control gets +2/+1 ... Each other creature you control gets +1/+1": other than the creature target
    assert_eq!((g.power(a), g.power(b)), (Some(5), Some(4)));
    let two = spell("Tap target artifact. Tap target creature. Each other creature gets +1/+1 until end of turn.");
    assert!(game::play::check(&two).unwrap_err().0.contains("ambiguous announced target slots"));
}

#[test]
fn explicit_source_actor_wins_over_an_earlier_target_without_provenance() {
    let d = creature("Probe", "Creature — Human", "{T}: Tap target creature. This creature deals 1 damage to each other creature.", (3, 5));
    let mut g = Game::new(vec![d, bear()], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let target = g.put(1, 1, Zone::Battlefield);
    g.act(0, Move::Activate { source, ability: 0, targets: vec![vec![Target::Object(target)]], paying: vec![] }).unwrap();
    pass(&mut g);
    // Uncited (card text): "This creature deals 1 damage to each other creature": other than this creature, not the tapped target
    assert_eq!(g.obj(source).damage, 0);
    assert_eq!(g.obj(target).damage, 1);
}

#[test]
fn full_intimidation_bolt_surviving_target_can_attack() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let d = CardDef::from_faces(CardDef::catalog(&faces).get("Intimidation Bolt").unwrap()).unwrap();
    let mut g = Game::new(vec![d, creature("Ox", "Creature — Ox", "Haste", (3, 5))], 2, 20);
    let target = g.put(0, 1, Zone::Battlefield);
    let other = g.put(0, 1, Zone::Battlefield);
    cast(&mut g, vec![vec![Target::Object(target)]], "WWR");
    cite!(["120.3e"]; assert_eq!(g.obj(target).damage, 3));
    assert!(!g.cant_attack(target, Target::Player(1)));
    assert!(g.cant_attack(other, Target::Player(1)));
    for _ in 0..4 { g.act(g.priority, Move::Pass).unwrap(); }
    assert_eq!(g.part, ir::Part::DeclareAttackers);
    cite!(["508.1c"]; refused("Ox can't attack that (508.1c)"): g.act(0, Move::Attack(vec![(other, Target::Player(1))])));
    g.act(0, Move::Attack(vec![(target, Target::Player(1))])).unwrap();
}

#[test]
fn illegal_target_controller_capture_stays_empty_with_another_legal_target() {
    let d = spell("Target creature an opponent controls gets -2/-2 until end of turn if that opponent controls no other creatures. Target player gains 1 life.");
    let mut g = Game::new(vec![d, bear()], 2, 20);
    let t = g.put(1, 1, Zone::Battlefield);
    let c = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: c, targets: vec![vec![Target::Object(t)], vec![Target::Player(0)]] }).unwrap();
    g.move_object(t, Zone::Hand);
    pass(&mut g);
    cite!(["608.2b"]; assert_eq!(g.players[0].life, 21));
    assert_eq!(g.power(t), Some(3));
}

// The opponent's target cannot belong to "you control
// another Knight". The entering Protocol Knight must not satisfy its own condition.
#[test]
fn protocol_knight_requires_another_knight_controlled_by_you() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let d = CardDef::from_faces(CardDef::catalog(&faces).get("Protocol Knight").unwrap()).unwrap();
    for extra in [false, true] {
        let mut g = Game::new(vec![d.clone(), bear(), creature("Knight", "Creature — Knight", "", (2, 2))], 2, 20);
        let target = g.put(1, 1, Zone::Battlefield);
        if extra { g.put(0, 2, Zone::Battlefield); }
        cast(&mut g, vec![], "WWWU");
        g.act(0, Move::ChooseTargets(vec![vec![Target::Object(target)]])).unwrap();
        pass(&mut g);
        // Uncited (card text): Protocol Knight's "tap target creature an opponent controls"
        assert!(g.obj(target).tapped);
        // Uncited (card text): "Put a stun counter on that creature if you control another Knight": a Knight other than this one
        assert_eq!(g.obj(target).counters.get("stun").copied().unwrap_or(0), u32::from(extra));
    }
}

// Contrast: "you control another Knight" remains source-relative
// when the tapped target has no controller restriction. Target membership is not guessed.
#[test]
fn you_control_another_knight_with_any_controller_target_is_source_relative() {
    let d = creature("Probe", "Creature — Human Knight", "When this creature enters, tap target creature. Put a stun counter on that creature if you control another Knight.", (3, 4));
    for owner in [0, 1] {
        for extra in [false, true] {
            let mut g = Game::new(vec![d.clone(), bear(), creature("Knight", "Creature — Knight", "", (2, 2))], 2, 20);
            let target = g.put(owner, 1, Zone::Battlefield);
            if extra { g.put(0, 2, Zone::Battlefield); }
            cast(&mut g, vec![], "");
            g.act(0, Move::ChooseTargets(vec![vec![Target::Object(target)]])).unwrap();
            pass(&mut g);
            // Uncited (card text): "tap target creature"
            assert!(g.obj(target).tapped);
            // Uncited (card text): "Put a stun counter on that creature if you control another Knight": a Knight other than this one
            assert_eq!(g.obj(target).counters.get("stun").copied().unwrap_or(0), u32::from(extra));
        }
    }
}
