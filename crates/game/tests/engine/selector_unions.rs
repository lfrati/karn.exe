//! Cross-domain unions retain complete arm constraints, not one new enum per phrase.
use crate::support::cite;
use game::{CardDef, Game, Target, Zone};
use game::select::{Ctx, all_in, in_set};
use grammar::kernel::Outcome;
use ir::{Atom, CardType, Domain, Filter, ObjectSet, Selector, Sort};

fn set(text: &str) -> ObjectSet {
    match grammar::selector::selectors().set_pl.parse_all(&oracle::token::tokenize(text)) {
        Outcome::Accepted { value, .. } => value,
        x => panic!("{text}: {x:?}"),
    }
}
fn described(domain: Domain, atom: Atom) -> ObjectSet {
    ObjectSet { filter: Filter::All(vec![Filter::Atom(atom)]), ..ObjectSet::of(domain) }
}
#[test]
fn r113_7a_ability_source_qualifiers_belong_only_to_the_ability_arm() {

    let s = set("black spells or abilities from black sources");
    let Domain::Union(arms) = &s.domain else { panic!("union") };
    assert_eq!(arms.len(), 2);
    assert_eq!(arms[0], described(Domain::Spell, Atom::Color(ir::Color::Black)));
    let source = described(Domain::Source, Atom::Color(ir::Color::Black));
    assert_eq!(arms[1], described(Domain::Ability, Atom::Source(Box::new(source))));
    let noun = Selector::new(ir::Quant::All, s.clone());
    assert!(ir::lexicon::of_kind(&noun, &[Sort::Spell, Sort::Ability]));
    assert!(!ir::lexicon::of_kind(&noun, &[Sort::Spell]));
    let rule = &grammar::selector::selectors().set_pl;
    assert_eq!(rule.print_text(&s, false).as_deref(), Some("black spells or abilities from black sources"));
    // Reading a new set does not authorize execution through a source/ability shortcut.
    let d = CardDef::new("Witness", "{0}", "Creature", "This creature can't be the target of black spells or abilities from black sources.", Some((2, 2))).unwrap();
    crate::support::refused(game::play::check(&d).map_err(|e|e.0),"description: Source");
}

#[test]
fn r400_1_r608_2c_union_enumeration_keeps_each_arms_zone_and_deduplicates_overlap() {
    let mut g = Game::new(vec![CardDef::new("Creature", "{0}", "Creature", "", Some((2, 2))).unwrap()], 3, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let other = g.put(1, 0, Zone::Battlefield);
    let dead = g.put(0, 0, Zone::Graveyard);
    let opponents_dead = g.put(1, 0, Zone::Graveyard);
    let hand = g.put(0, 0, Zone::Hand);
    let graveyard = ObjectSet {
        zone: Some(ir::selector::ZoneSpec::of(ir::Zone::Graveyard, Some(Selector::you()))),
        ..described(Domain::Card, Atom::Type(CardType::Creature))
    };
    let union = ObjectSet::of(Domain::Union(vec![
        described(Domain::Permanent, Atom::Type(CardType::Creature)),
        graveyard,
        ObjectSet::of(Domain::Permanent),
    ]));
    let ctx = Ctx::new(0, source);
    let found = all_in(&g, &ctx, &union).unwrap();
    assert_eq!(found, vec![Target::Object(source), Target::Object(other), Target::Object(dead)]);
    for o in [source, other, dead, opponents_dead, hand] {
        assert_eq!(in_set(&g, &ctx, &union, Target::Object(o)).unwrap(), found.contains(&Target::Object(o)));
    }
}

#[test]
fn r608_2c_an_unsupported_union_arm_is_rejected_even_when_an_earlier_arm_matches() {

    let mut g = Game::new(vec![CardDef::new("Creature", "{0}", "Creature", "", Some((2, 2))).unwrap()], 2, 20);
    let o = g.put(0, 0, Zone::Battlefield);
    let union = ObjectSet::of(Domain::Union(vec![ObjectSet::of(Domain::Permanent), ObjectSet::of(Domain::Source)]));
    crate::support::refused(in_set(&g, &Ctx::new(0, o), &union, Target::Object(o)).map_err(|e|e.0),"noun phrase: Source");
    crate::support::refused(all_in(&g, &Ctx::new(0, o), &ObjectSet::of(Domain::Union(vec![]))).map_err(|e|e.0),"an empty selector union");
    assert!(in_set(&g,&Ctx::new(0,o),&ObjectSet::of(Domain::Permanent),Target::Object(o)).unwrap());
}

#[test]
fn r120_3h_r310_4_explicit_battle_dependencies_stay_refused_after_domain_migration() {
    for text in [
        "{T}: This creature deals 2 damage to target player or battle.",
        "Whenever this creature deals combat damage to a player or battle, draw a card.",
        "{T}: This creature deals 2 damage to target battle.",
    ] {
        let d = CardDef::new("Witness", "{0}", "Creature", text, Some((2, 2))).unwrap();
        let error = game::play::check(&d).unwrap_err();
        assert!(error.0.contains("damage to battles"), "{text}: {error:?}");
    }
}

#[test]
fn r310_4_excluding_battles_does_not_require_executing_their_rules() {
    let mut g = Game::new(vec![CardDef::new("Artifact", "{0}", "Artifact", "", None).unwrap()], 2, 20);
    let o = g.put(0, 0, Zone::Battlefield);
    let mut s = ObjectSet::of(Domain::Permanent);
    s.filter = Filter::Not(Box::new(Filter::Atom(Atom::Type(CardType::Battle))));
    assert_eq!(all_in(&g, &Ctx::new(0, o), &s).unwrap(), vec![Target::Object(o)]);
    s.filter = Filter::Not(Box::new(s.filter));
    assert!(all_in(&g, &Ctx::new(0, o), &s).unwrap().is_empty());
}

#[test]
fn r701_8_damage_capability_does_not_disable_generic_destruction_of_a_battle() {
    let d = CardDef::new("Removal", "{0}", "Sorcery", "Destroy target battle.", None).unwrap();
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d, CardDef::new("Battle", "{0}", "Battle", "", None).unwrap()], 2, 20);
    let target = g.put(1, 1, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, game::Move::Cast { card, targets: vec![vec![Target::Object(target)]] }).unwrap();
    g.act(0, game::Move::Pass).unwrap();
    g.act(1, game::Move::Pass).unwrap();
    cite!(["701.8a"]; assert!(!g.battlefield.contains(&target)));
}
