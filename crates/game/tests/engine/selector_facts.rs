//! Static role compatibility and actual membership share domain alternatives (109, 115).
use game::{CardDef, Game, Move, Target, Zone};
use game::select::{Ctx, in_set};
use grammar::kernel::Outcome;
use ir::{CardType, Selector, Sort};

fn selector(text: &str) -> Selector {
    match grammar::selector::selectors().selector.parse_all(&oracle::token::tokenize(text)) {
        Outcome::Accepted { value, .. } => value,
        other => panic!("{text}: {other:?}"),
    }
}
fn permanent(types: &str) -> CardDef {
    CardDef::new(types, "{0}", types, "", types.contains("Creature").then_some((2, 2))).unwrap()
}

#[test]
fn r205_2_static_possibility_does_not_make_overlapping_types_actual() {
    let artifact = selector("an artifact");
    let noncreature = selector("a noncreature permanent");
    assert!(ir::lexicon::of_kind(&artifact, &[Sort::Of(CardType::Creature)]));
    assert!(!ir::lexicon::of_kind(&noncreature, &[Sort::Of(CardType::Creature)]));
    let mut g = Game::new(vec![permanent("Artifact"), permanent("Artifact Creature"), permanent("Creature")], 2, 20);
    let a = g.put(0, 0, Zone::Battlefield);
    let ac = g.put(0, 1, Zone::Battlefield);
    let c = g.put(0, 2, Zone::Battlefield);
    let ctx = Ctx::new(0, a);
    for (o, is_artifact, is_noncreature) in [(a, true, true), (ac, true, false), (c, false, false)] {
        assert_eq!(in_set(&g, &ctx, &artifact.set, Target::Object(o)).unwrap(), is_artifact);
        assert_eq!(in_set(&g, &ctx, &noncreature.set, Target::Object(o)).unwrap(), is_noncreature);
    }
}

#[test]
fn r115_4_r800_4a_mixed_domains_keep_player_and_object_membership_separate() {
    let mut g = Game::new(vec![permanent("Artifact"), permanent("Creature"), permanent("Planeswalker")], 3, 20);
    let artifact = g.put(0, 0, Zone::Battlefield);
    let creature = g.put(0, 1, Zone::Battlefield);
    let walker = g.put(0, 2, Zone::Battlefield);
    let ctx = Ctx::new(0, artifact);
    g.players[2].lost = true;
    for (text, expected) in [
        ("any target", [true, true, false, false, true, true]),
        ("target opponent or planeswalker", [false, true, false, false, false, true]),
        ("target player or planeswalker", [true, true, false, false, false, true]),
    ] {
        let s = selector(text);
        assert_eq!(ir::lexicon::denotes_players(&s), None);
        assert!(!ir::lexicon::of_kind(&s, &[Sort::Permanent]));
        for (t, yes) in [Target::Player(0), Target::Player(1), Target::Player(2), Target::Object(artifact), Target::Object(creature), Target::Object(walker)].into_iter().zip(expected) {
            assert_eq!(in_set(&g, &ctx, &s.set, t).unwrap(), yes, "{text}: {t:?}");
        }
    }
}

#[test]
fn r112_1_r113_1c_stack_union_contains_spells_and_abilities_without_conflating_them() {
    let source = CardDef::new("Witness", "", "Land", "{0}: You gain 1 life.", None).unwrap();
    let mut g = Game::new(vec![source, CardDef::new("Spell", "{0}", "Instant", "You gain 1 life.", None).unwrap()], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let card = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    let spell = *g.stack.last().unwrap();
    g.act(0, Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    let ability = *g.stack.last().unwrap();
    let ctx = Ctx::new(0, source);
    for (text, expected) in [("target spell", [true, false]), ("target ability", [false, true]), ("target spell or ability", [true, true])] {
        let s = selector(text);
        assert!(ir::lexicon::of_kind(&s, &[Sort::Spell, Sort::Ability]));
        assert_eq!(ir::lexicon::denotes_players(&s), Some(false));
        for (o, yes) in [spell, ability].into_iter().zip(expected) {
            assert_eq!(in_set(&g, &ctx, &s.set, Target::Object(o)).unwrap(), yes, "{text}");
        }
    }
}
