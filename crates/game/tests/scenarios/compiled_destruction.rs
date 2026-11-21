//! Targeted destruction migration: real spell/trigger witnesses, plus isolated rule states.
//! Route assertions are separate from routed outcomes so legacy execution cannot conceal a
//! missing compiled implementation, and compilation alone is not evidence of correct play.
use crate::support::{cite, paid};
use game::card::CastAs;
use game::program::compile;
use game::select::Ctx;
use game::state::Decision;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::{Ability, Color};

const MURDER: usize = 0;
const NATURALIZE: usize = 1;
const TERROR: usize = 2;
const OGRE: usize = 3;
const PONGIFY: usize = 4;
const VERDICT: usize = 5;
const SHATTER: usize = 6;
const BEAR: usize = 7;
const FOREST: usize = 8;
const ARTIFACT: usize = 9;
const ENCHANTMENT: usize = 10;
const BOTH: usize = 11;
const INDESTRUCTIBLE: usize = 12;
const BLACK: usize = 13;
const CUTTHROAT: usize = 14;

fn game() -> Game {
    let c = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    let defs = vec![
        c("Murder", "{1}{B}{B}", "Instant", "Destroy target creature.", None),
        c("Naturalize", "{1}{G}", "Instant", "Destroy target artifact or enchantment.", None),
        c("Terror", "{1}{B}", "Instant", "Destroy target nonartifact, nonblack creature. It can't be regenerated.", None),
        c("Ogre Arsonist", "{4}{R}", "Creature — Ogre", "When this creature enters, destroy target land.", Some((3, 3))),
        c("Pongify", "{U}", "Instant", "Destroy target creature. It can't be regenerated. Its controller creates a 3/3 green Ape creature token.", None),
        c("Supreme Verdict", "{1}{W}{W}{U}", "Sorcery", "This spell can't be countered.\nDestroy all creatures.", None),
        c("Shatter", "{1}{R}", "Instant", "Destroy target artifact.", None),
        c("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        c("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        c("Artifact witness", "{0}", "Artifact", "", None),
        c("Enchantment witness", "{0}", "Enchantment", "", None),
        c("Intersection witness", "{0}", "Artifact Enchantment", "", None),
        c("Indestructible witness", "{G}", "Creature — Bear", "Indestructible", Some((2, 2))),
        c("Black witness", "{B}", "Creature — Zombie", "", Some((2, 2))),
        c("Zulaport Cutthroat", "{1}{B}", "Creature — Human Rogue Ally", "Whenever this creature or another creature you control dies, each opponent loses 1 life and you gain 1 life.", Some((1, 1))),
    ];
    for def in &defs {
        game::play::check(def).unwrap_or_else(|e| panic!("{}: {e:?}", def.name()));
    }
    let mut g = Game::new(defs, 3, 20);
    // Pay real printed costs without making mana production part of these tests.
    g.players[0].pool.colors = [20; 5];
    g
}

fn pass(g: &mut Game) {
    for _ in 0..3 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}

fn cast(g: &mut Game, def: usize, target: Option<ObjId>, spending: &str) {
    let card = g.put(0, def, Zone::Hand);
    let targets = target.map(|o| vec![vec![Target::Object(o)]]).unwrap_or_default();
    g.act(0, paid(spending, Move::Cast { card, targets })).unwrap();
}

fn effects(def: &CardDef) -> impl Iterator<Item = (usize, &ir::Effect)> {
    def.abilities().iter().enumerate().filter_map(|(i, a)| match a {
        Ability::Text(e) | Ability::Triggered { effect: e, .. } => Some((i, e)),
        _ => None,
    })
}

#[test]
fn r701_8a_targeted_destruction_witnesses_compile_as_whole_abilities() {
    let mut g = game();
    for def in [MURDER, NATURALIZE, TERROR, OGRE, SHATTER] {
        let source = g.put(0, def, Zone::Stack);
        let (ability, effect) = effects(&g.defs[def]).next().unwrap();
        assert!(
            compile(effect, &g, &Ctx::new(0, source).with_ability(ability)).is_ok(),
            "{} must use a compiled program",
            g.defs[def].name()
        );
    }
}

#[test]
fn r701_8a_destruction_accepts_permanents_and_rejects_players_in_the_grammar() {
    assert!(CardDef::new("Rule witness", "{0}", "Instant", "Destroy target permanent.", None).is_ok());
    assert_eq!(CardDef::new("Nonsense","{0}","Instant","Destroy target player.",None).unwrap_err(),"text the grammar does not read");
}

#[test]
fn r601_2c_r608_2b_murder_checks_targets_at_announcement_and_resolution() {
    let mut g = game();
    let bear = g.put(1, BEAR, Zone::Battlefield);
    let land = g.put(1, FOREST, Zone::Battlefield);
    let card = g.put(0, MURDER, Zone::Hand);
    let before = format!("{g:?}");
    for (invalid, why) in [(Target::Object(land), "an illegal target, Forest: it isn't a creature (115.1)"), (Target::Player(1), "an illegal target, that player: a player isn't a permanent (115.1)")] {
        cite!(["115.1"]; case: why; refused(why): g.act(0, Move::Cast { card, targets: vec![vec![invalid]] }));
        assert_eq!(format!("{g:?}"), before, "invalid announcement is transactional");
    }
    g.act(0, paid("WBB", Move::Cast { card, targets: vec![vec![Target::Object(bear)]] })).unwrap();
    // A new incarnation of the same card is not the spell's announced target (400.7).
    g.move_object(bear, Zone::Hand);
    let returned = *g.players[1].hand.last().unwrap();
    g.move_object(returned, Zone::Battlefield);
    let new_bear = *g.battlefield.last().unwrap();
    pass(&mut g);
    assert_ne!(new_bear, bear);
    assert_eq!(g.obj(new_bear).zone, Zone::Battlefield);
    cite!(["608.2b", "400.7"]; assert!(!g.log.iter().any(|e| matches!(e, Event::Destroyed { object } if *object == bear || *object == new_bear))));
    assert!(g.stack.is_empty());
}

#[test]
fn r601_2c_naturalize_union_offers_each_eligible_object_once_and_destroys_each_arm() {
    for selected in 0..3 {
        let mut g = game();
        let candidates = [ARTIFACT, ENCHANTMENT, BOTH].map(|d| g.put(1, d, Zone::Battlefield));
        g.put(1, BEAR, Zone::Battlefield);
        g.put(1, FOREST, Zone::Battlefield);
        let card = g.put(0, NATURALIZE, Zone::Hand);
        g.act(0, paid("WG", Move::Cast { card, targets: vec![vec![Target::Object(candidates[selected])]] })).unwrap();
        pass(&mut g);
        for (i, object) in candidates.into_iter().enumerate() {
            cite!(["701.8a"]; case: format!("{selected}/{i}"); assert_eq!(g.obj(object).zone, if i == selected { Zone::Gone } else { Zone::Battlefield }));
        }
    }
}

#[test]
fn r701_19a_r608_2c_terror_rider_disables_regeneration_but_murder_uses_the_shield() {
    for (spell, spending) in [(MURDER, "WBB"), (TERROR, "WB")] {
        let mut g = game();
        let bear = g.put(1, BEAR, Zone::Battlefield);
        g.regenerate(bear);
        cast(&mut g, spell, Some(bear), spending);
        pass(&mut g);
        if spell == MURDER {
            cite!(["701.19a"]; assert_eq!(g.obj(bear).zone, Zone::Battlefield));
            cite!(["701.19a"]; assert!(g.obj(bear).tapped));
            assert!(g.log.contains(&Event::Regenerated { object: bear }));
            assert!(!g.log.contains(&Event::Destroyed { object: bear }));
        } else {
            cite!(["701.19c"]; assert_eq!(g.obj(bear).zone, Zone::Gone));
            assert!(g.log.contains(&Event::Destroyed { object: bear }));
            cite!(["701.19c"]; assert!(!g.log.contains(&Event::Regenerated { object: bear })));
        }
    }
}

#[test]
fn r702_12b_terror_does_not_override_indestructible() {
    let mut g = game();
    let bear = g.put(1, INDESTRUCTIBLE, Zone::Battlefield);
    g.regenerate(bear);
    cast(&mut g, TERROR, Some(bear), "WB");
    pass(&mut g);
    cite!(["702.12b"]; assert_eq!(g.obj(bear).zone, Zone::Battlefield));
    assert!(!g.obj(bear).tapped);
    cite!(["702.12b"]; assert!(!g.log.contains(&Event::Destroyed { object: bear })));
    assert!(!g.log.contains(&Event::Regenerated { object: bear }));
}

#[test]
fn r601_2c_terror_rejects_black_creatures_and_destroys_an_eligible_creature() {
    let mut g = game();
    let black = g.put(1, BLACK, Zone::Battlefield);
    let green = g.put(1, BEAR, Zone::Battlefield);
    let card = g.put(0, TERROR, Zone::Hand);
    cite!(["115.1"]; refused("an illegal target, Black witness: it is black (115.1)"): g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(black)]] }));
    g.act(0, paid("WB", Move::Cast { card, targets: vec![vec![Target::Object(green)]] })).unwrap();
    pass(&mut g);
    assert_eq!(g.obj(black).zone, Zone::Battlefield);
    cite!(["701.8a"]; assert_eq!(g.obj(green).zone, Zone::Gone));
}

#[test]
fn r603_3d_r113_7a_ogre_targets_when_trigger_stacks_and_survives_source_removal() {
    let mut g = game();
    let land = g.put(2, FOREST, Zone::Battlefield);
    cast(&mut g, OGRE, None, "WWWWR");
    assert!(g.decision.is_none(), "the creature spell itself has no target");
    pass(&mut g);
    cite!(["603.3d"]; assert!(matches!(g.decision, Some(Decision::Targets { player: 0, groups: 1, .. }))));
    crate::support::refused(g.act(1, Move::ChooseTargets(vec![vec![Target::Object(land)]])), "that decision is another player's");
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(land)]])).unwrap();
    let ogre = *g.battlefield.last().unwrap();
    g.move_object(ogre, Zone::Graveyard);
    pass(&mut g);
    assert_eq!(g.obj(land).zone, Zone::Gone);
    cite!(["113.7a"]; assert!(g.log.contains(&Event::Destroyed { object: land })));
}

#[test]
fn r608_2c_compound_destruction_compiles_and_keeps_later_instructions() {
    let mut g = game();
    let source = g.put(0, PONGIFY, Zone::Stack);
    let (ability, effect) = effects(&g.defs[PONGIFY]).next().unwrap();
    assert!(compile(effect, &g, &Ctx::new(0, source).with_ability(ability)).is_ok());
    g.move_object(source, Zone::Exile);
    let bear = g.put(2, BEAR, Zone::Battlefield);
    g.regenerate(bear);
    cast(&mut g, PONGIFY, Some(bear), "U");
    pass(&mut g);
    cite!(["701.19c"]; assert_eq!(g.obj(bear).zone, Zone::Gone));
    let ape =
        g.battlefield.iter().find(|o| g.obj(**o).token()).copied().expect("later creation instruction was not dropped");
    cite!(["608.2c", "608.2h"]; assert_eq!(g.obj(ape).controller, 2));
    assert_eq!((g.power(ape), g.toughness(ape)), (Some(3), Some(3)));
    assert!(g.seen(ape, None).colors.contains(&Color::Green));
    let extra =
        CardDef::new("Sequence witness", "{0}", "Instant", "Destroy target creature. You gain 1 life.", None).unwrap();
    assert!(
        compile(effects(&extra).next().unwrap().1, &g, &Ctx::new(0, source)).is_ok(),
        "both instructions must compile together"
    );
}

#[test]
fn r603_10a_r608_2f_supreme_verdict_keeps_simultaneous_death_observation() {
    for observer_first in [true, false] {
        let mut g = game();
        let order = if observer_first { [CUTTHROAT, BEAR] } else { [BEAR, CUTTHROAT] };
        let mut cutthroat = None;
        let mut arrivals = Vec::new();
        for def in order {
            let object = g.put(0, def, Zone::Battlefield);
            arrivals.push(object);
            if def == CUTTHROAT { cutthroat = Some(object); }
        }
        cast(&mut g, VERDICT, None, "WWWU");
        pass(&mut g);
        assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: game::state::Choice::OrderCards { to: game::state::Placement::Zone(Zone::Graveyard), cards: arrivals.clone() } }));
        g.act(0, Move::Choose(game::state::Answer::Objects(vec![arrivals[1], arrivals[0]]))).unwrap();
        assert!(g.battlefield.is_empty());
        let Some(Decision::Order { player, abilities }) = &g.decision else { panic!("expected trigger order: {:?}", g.decision) };
        cite!(["603.3b"]; assert_eq!((*player, abilities.len()), (0, 2)));
        assert!(abilities.iter().all(|trigger| trigger.controller == 0 && trigger.source == cutthroat));
        g.act(0, Move::Order(vec![0, 1])).unwrap();
        cite!(["603.10a"]; case: format!("{observer_first}"); assert_eq!(g.stack.len(), 2, "Cutthroat observes both simultaneous deaths in either insertion order"));
        pass(&mut g);
        pass(&mut g);
        cite!(["603.10a"]; case: format!("{observer_first}"); assert_eq!(g.players.iter().map(|p| p.life).collect::<Vec<_>>(), [22, 18, 18]));
    }
}

#[test]
fn r608_2c_separate_compiled_spell_abilities_keep_target_offsets() {
    // Independent paragraphs need independent effect identities and target slices. These
    // rule-derived compositions intentionally contain no reference between paragraphs.
    let cases = [
        ("Destroy target artifact.\nReturn target creature to its owner's hand.", [true, false]),
        ("Return target artifact to its owner's hand.\nDestroy target creature.", [false, true]),
        ("Destroy target artifact.\nDestroy target creature.", [true, true]),
    ];
    for (text, destroys) in cases {
        for invalid in [None, Some(0), Some(1)] {
            let mut g = game();
            let def = CardDef::new("Independent abilities", "{0}", "Instant", text, None).unwrap();
            game::play::check(&def).unwrap();
            assert_eq!(effects(&def).count(), 2);
            let id = g.defs.len();
            std::sync::Arc::make_mut(&mut g.defs).push(def);
            let targets = [ARTIFACT, BEAR].map(|d| g.put(1, d, Zone::Battlefield));
            let card = g.put(0, id, Zone::Hand);
            for (index, effect) in effects(&g.defs[id]) {
                assert!(compile(effect, &g, &Ctx::new(0, card).with_ability(index)).is_ok());
            }
            g.act(0, Move::Cast { card, targets: targets.iter().map(|o| vec![Target::Object(*o)]).collect() }).unwrap();
            if let Some(i) = invalid {
                g.exile(targets[i]);
            }
            pass(&mut g);
            for (i, target) in targets.into_iter().enumerate() {
                assert_eq!(g.obj(target).zone, Zone::Gone);
                let destroyed = g.log.contains(&Event::Destroyed { object: target });
                cite!(["608.2b"]; case: format!("{text}; invalid={invalid:?}, target={i}"); assert_eq!(destroyed, destroys[i] && invalid != Some(i), "{text}; invalid={invalid:?}, target={i}"));
                let expected_zone = if invalid == Some(i) {
                    &g.exile
                } else if destroys[i] {
                    &g.players[1].graveyard
                } else {
                    &g.players[1].hand
                };
                assert!(
                    expected_zone.iter().any(|o| g.obj(*o).chars.name == g.obj(target).chars.name),
                    "target {i} reached the right zone: {text}"
                );
            }
        }
    }
}

#[test]
fn r608_2c_a_skipped_condition_preserves_the_next_abilitys_target_offset() {
    for condition_true in [false, true] {
        let mut g = game();
        let def = CardDef::new(
            "Conditional abilities",
            "{0}",
            "Instant",
            "If you control a creature, tap target artifact.\nDestroy target creature.",
            None,
        )
        .unwrap();
        game::play::check(&def).unwrap();
        let id = g.defs.len();
        std::sync::Arc::make_mut(&mut g.defs).push(def);
        let artifact = g.put(1, ARTIFACT, Zone::Battlefield);
        let bear = g.put(1, BEAR, Zone::Battlefield);
        if condition_true {
            g.put(0, BEAR, Zone::Battlefield);
        }
        let card = g.put(0, id, Zone::Hand);
        for (index, effect) in effects(&g.defs[id]) {
            assert!(compile(effect, &g, &Ctx::new(0, card).with_ability(index)).is_ok());
        }
        g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(artifact)], vec![Target::Object(bear)]] })
            .unwrap();
        pass(&mut g);
        assert_eq!(g.obj(artifact).zone, Zone::Battlefield, "the artifact was not the destruction target");
        cite!(["608.2c"]; case: format!("{condition_true}"); assert_eq!(g.obj(artifact).tapped, condition_true));
        assert_eq!(g.obj(bear).zone, Zone::Gone, "a skipped first ability still owns its announced target group");
        cite!(["701.8a"]; case: format!("{condition_true}"); assert!(g.log.contains(&Event::Destroyed { object: bear })));
    }
}

#[test]
fn r601_2c_r608_2b_rain_of_salt_requires_distinct_targets_and_resolves_remaining_legal_ones() {
    for invalid_count in 0..=2 {
        let mut g = game();
        let def = CardDef::new("Rain of Salt", "{4}{R}{R}", "Sorcery", "Destroy two target lands.", None).unwrap();
        game::play::check(&def).unwrap();
        let id = g.defs.len();
        std::sync::Arc::make_mut(&mut g.defs).push(def);
        let lands = [g.put(1, FOREST, Zone::Battlefield), g.put(2, FOREST, Zone::Battlefield)];
        let card = g.put(0, id, Zone::Hand);
        let (index, effect) = effects(&g.defs[id]).next().unwrap();
        assert!(compile(effect, &g, &Ctx::new(0, card).with_ability(index)).is_ok());
        let before = format!("{g:?}");
        for (rule, wrong, why) in [("601.2c", vec![Target::Object(lands[0])], "wrong number of targets: 1 chosen, it takes 2 (601.2c)"), ("115.3", vec![Target::Object(lands[0]); 2], "the same target twice for one \"target\" (115.3)")] {
            cite!([rule]; case: why; refused(why): g.act(0, Move::Cast { card, targets: vec![wrong] }));
            assert_eq!(format!("{g:?}"), before, "two distinct lands must be announced");
        }
        g.act(0, paid("WWWWRR", Move::Cast { card, targets: vec![lands.map(Target::Object).to_vec()] })).unwrap();
        for land in lands.iter().take(invalid_count) {
            g.exile(*land);
        }
        pass(&mut g);
        for (i, land) in lands.into_iter().enumerate() {
            cite!(["608.2b"]; case: format!("{invalid_count}/{i}"); assert_eq!(g.log.contains(&Event::Destroyed { object: land }), i >= invalid_count));
            assert_eq!(g.players[i + 1].graveyard.len(), usize::from(i >= invalid_count));
        }
        cite!(["608.2b"]; case: format!("{invalid_count}"); assert_eq!(g.log.iter().any(|e| matches!(e, Event::Fizzled { .. })), invalid_count == 2));
        assert_eq!(g.players[0].graveyard.len(), 1);
        assert!(g.stack.is_empty());
    }
}

#[test]
fn r709_3_r702_102_wear_tear_uses_the_chosen_definition_and_fused_target_offsets() {
    for (choice, invalidate_first, spending) in
        [(CastAs::Half(0), false, "WR"), (CastAs::Half(1), false, "W"), (CastAs::Fused, false, "WWR"), (CastAs::Fused, true, "WWR")]
    {
        let mut g = game();
        let split = CardDef::split([
            CardDef::new("Wear", "{1}{R}", "Instant", "Destroy target artifact.\nFuse", None).unwrap(),
            CardDef::new("Tear", "{W}", "Instant", "Destroy target enchantment.\nFuse", None).unwrap(),
        ])
        .unwrap();
        game::play::check(&split).unwrap();
        let id = g.defs.len();
        std::sync::Arc::make_mut(&mut g.defs).push(split);
        let artifact = g.put(1, ARTIFACT, Zone::Battlefield);
        let enchantment = g.put(2, ENCHANTMENT, Zone::Battlefield);
        let card = g.put(0, id, Zone::Hand);
        let target_ids = match choice {
            CastAs::Half(0) => vec![artifact],
            CastAs::Half(1) => vec![enchantment],
            CastAs::Fused => vec![artifact, enchantment],
            _ => unreachable!(),
        };
        let targets = target_ids.iter().map(|o| vec![Target::Object(*o)]).collect();
        g.act(0, paid(spending, Move::CastAs(choice, Box::new(Move::Cast { card, targets })))).unwrap();
        if invalidate_first {
            g.exile(artifact);
        }
        pass(&mut g);
        cite!(["709.3a", "702.102d"]; case: format!("{choice:?}/{invalidate_first}"); assert_eq!(g.obj(artifact).zone, if choice == CastAs::Half(1) { Zone::Battlefield } else { Zone::Gone }));
        cite!(["709.3a", "702.102d"]; case: format!("{choice:?}/{invalidate_first}"); assert_eq!(g.obj(enchantment).zone, if choice == CastAs::Half(0) { Zone::Battlefield } else { Zone::Gone }));
        assert_eq!(g.players[0].graveyard.len(), 1, "a fused spell is one card");
        assert_eq!(g.obj(g.players[0].graveyard[0]).chars.name, "Wear // Tear");
        cite!(["608.2b"]; case: format!("{choice:?}/{invalidate_first}"); assert_eq!(
            g.log.contains(&Event::Destroyed { object: artifact }),
            choice != CastAs::Half(1) && !invalidate_first
        ));
        assert_eq!(g.log.contains(&Event::Destroyed { object: enchantment }), choice != CastAs::Half(0));
    }
}
