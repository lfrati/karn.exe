//! Shared noun heads constrain every coordinated modifier (109.2, 205.2, 205.3m, 608.2c).
//! Witnesses deliberately include kindred noncreatures: having a creature subtype does not
//! itself make a permanent a creature.
use crate::support::cite;
use game::{CardDef, Game, Move, Target, Zone};

fn witness(name: &str, types: &str, cost: &str) -> CardDef {
    CardDef::new(name, cost, types, "", types.contains("Creature").then_some((2, 2))).unwrap()
}
fn resolve(g: &mut Game) {
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    for _ in 0..3 { g.act(g.priority, Move::Pass).unwrap(); }
}

#[test]
fn r205_3m_r608_2c_shared_creature_head_excludes_kindred_noncreatures_for_both_arms() {
    // Changing the verb must reuse the same set semantics, without another card-specific rule.
    for verb in ["Destroy", "Exile"] {
        let text = format!("{verb} all Ninja and Rogue creatures.");
        let mut g = Game::new(vec![
            CardDef::new("Sweep", "{0}", "Sorcery", &text, None).unwrap(),
            witness("Ninja creature", "Creature — Ninja", "{0}"),
            witness("Rogue creature", "Creature — Rogue", "{0}"),
            witness("Ninja noncreature", "Kindred Enchantment — Ninja", "{0}"),
            witness("Rogue noncreature", "Kindred Enchantment — Rogue", "{0}"),
            witness("Unrelated", "Creature — Bear", "{0}"),
        ], 3, 20);
        game::play::check(&g.defs[0]).unwrap();
        let objects: Vec<_> = (1..6).map(|d| g.put(1, d, Zone::Battlefield)).collect();
        resolve(&mut g);
        if verb == "Destroy" {cite!(["404.3"]; ok: g.act(1,Move::Choose(game::state::Answer::Objects(vec![objects[0],objects[1]]))));}
        for (i, o) in objects.iter().enumerate() {
            cite!(["608.2c"]; case: format!("{verb}, witness {i}"); assert_eq!(g.battlefield.contains(o), i >= 2, "{verb}, witness {i}"));
        }
    }
}

#[test]
fn r205_2_r608_2c_color_or_type_modifies_the_shared_creature_head() {
    let text = "Destroy target black or artifact creature.";
    let mut g = Game::new(vec![
        CardDef::new("Removal", "{0}", "Sorcery", text, None).unwrap(),
        witness("Black creature", "Creature", "{B}"),
        witness("Artifact creature", "Artifact Creature", "{0}"),
        witness("Black noncreature", "Enchantment", "{B}"),
        witness("Artifact noncreature", "Artifact", "{0}"),
        witness("Unrelated creature", "Creature", "{G}"),
    ], 3, 20);
    game::play::check(&g.defs[0]).unwrap();
    let objects: Vec<_> = (1..6).map(|d| g.put(1, d, Zone::Battlefield)).collect();
    let card = g.put(0, 0, Zone::Hand);
    for (i, object) in objects.into_iter().enumerate() {
        let mut trial = g.clone();
        let result = trial.act(0, Move::Cast { card, targets: vec![vec![Target::Object(object)]] });
        cite!(["608.2c"]; case: format!("witness {i}"); assert_eq!(result.is_ok(), i < 2, "witness {i}: {result:?}"));
        if result.is_ok() {
            for _ in 0..3 { trial.act(trial.priority, Move::Pass).unwrap(); }
            assert!(!trial.battlefield.contains(&object));
        }
    }
}

#[test]
fn r205_4g_r611_3a_shared_head_applies_to_static_effects_and_both_union_arms() {
    let text = "Other snow and Zombie creatures you control get +1/+1.";
    let mut g = Game::new(vec![
        CardDef::new("Witness", "{0}", "Snow Creature — Zombie", text, Some((2, 2))).unwrap(),
        witness("Snow creature", "Snow Creature — Bear", "{0}"),
        witness("Zombie", "Creature — Zombie", "{0}"),
        witness("Both", "Snow Creature — Zombie", "{0}"),
        witness("Neither", "Creature — Bear", "{0}"),
        witness("Snow noncreature", "Snow Artifact", "{0}"),
        witness("Kindred Zombie", "Kindred Enchantment — Zombie", "{0}"),
    ], 3, 20);
    game::play::check(&g.defs[0]).unwrap();
    let source = g.put(0, 0, Zone::Battlefield);
    for owner in [0, 1] {
        for (d, affected) in [(1, true), (2, true), (3, true), (4, false)] {
            let o = g.put(owner, d, Zone::Battlefield);
            cite!(["611.3a"]; case: format!("owner {owner}, witness {d}"); assert_eq!(g.power(o), Some(if owner == 0 && affected { 3 } else { 2 })));
        }
    }
    for d in [5, 6] {
        let o = g.put(0, d, Zone::Battlefield);
        assert_eq!(g.power(o), None);
    }
    assert_eq!(g.power(source), Some(2), "other excludes the source");
}

#[test]
fn r109_2_r608_2c_shared_heads_read_say_and_reject_incompatible_domains() {

    use grammar::kernel::Outcome;
    let rule = &grammar::selector::selectors().selector;
    for text in [
        "a Ninja or Rogue creature",
        "one or more Ninja or Rogue creatures",
        "a Ninja or Rogue creature card",
        "a black or artifact creature",
        "a blue or artifact creature card",
        "target snow or Zombie creature",
    ] {
        let Outcome::Accepted { value, .. } = rule.parse_all(&oracle::token::tokenize(text)) else { panic!("{text}") };
        let printed = rule.print_text(&value, false).expect("shared head prints");
        let Outcome::Accepted { value: again, .. } = rule.parse_all(&oracle::token::tokenize(&printed)) else { panic!("{printed}") };
        assert_eq!(again, value, "printing preserves grouping: {text}");
        let ir::Filter::All(items) = &value.set.filter else { panic!("conjunction") };
        assert!(matches!(items.as_slice(), [ir::Filter::Any(_), ir::Filter::Atom(ir::Atom::Type(ir::CardType::Creature))]));
    }
    for text in [
        "Destroy target black or artifact player.",
        "Destroy target Ninja or Rogue player.",
        "Target black or artifact creature draws a card.",
    ] {
        assert_eq!(CardDef::new("Nonsense", "{0}", "Sorcery", text, None).expect_err("incompatible noun domains"),"text the grammar does not read");
    }
}

#[test]
fn r608_2c_whole_noun_unions_keep_each_arms_own_modifiers() {
    use grammar::kernel::Outcome;
    use game::select::{Ctx, in_set};
    let rule = &grammar::selector::selectors().selector;
    let Outcome::Accepted { value, .. } = rule.parse_all(&oracle::token::tokenize("target artifact or tapped creature")) else { panic!("whole nouns") };
    let mut g = Game::new(vec![
        witness("Artifact", "Artifact", "{0}"),
        witness("Creature", "Creature", "{0}"),
    ], 2, 20);
    let artifact = g.put(0, 0, Zone::Battlefield);
    let creature = g.put(0, 1, Zone::Battlefield);
    let ctx = Ctx::new(0, artifact);
    assert!(in_set(&g, &ctx, &value.set, Target::Object(artifact)).unwrap());
    assert!(!in_set(&g, &ctx, &value.set, Target::Object(creature)).unwrap());
    g.tap(creature);
    cite!(["608.2c"]; assert!(in_set(&g, &ctx, &value.set, Target::Object(creature)).unwrap()));
    for text in [
        "all artifacts, enchantments, and nonbasic lands they control",
        "target tapped Merfolk or Kithkin creature",
        "a Bird or basic land card",
    ] {
        assert!(matches!(rule.parse_all(&oracle::token::tokenize(text)), Outcome::Accepted { .. }), "{text}");
    }
}
