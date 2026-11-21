//! CR 607.2d: one binding mechanism for entry choices of different kinds. The small land
//! below is a rule-derived composition, not a claimed real-card implementation.
use crate::support::{cite, mana_pool};
use game::binding::{AbilityKey, Grant, Kind, LinkId, Links, Origin, Value};
use game::legal::Asked;
use game::select::{Ctx, in_set};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::{Atom, CardType, Color, Domain, Filter, ObjectSet, Subtype};

fn land(text: &str) -> CardDef {
    CardDef::new("Choice witness", "", "Land", text, None).unwrap()
}
fn defs() -> Vec<CardDef> {
    vec![
        land(
            "As this land enters, choose a color.\n{T}: Add {C}. Spend this mana only to cast a spell of the chosen color.\n{T}: Destroy target creature of the chosen color.\nCreatures of the chosen color get +1/+1.",
        ),
        CardDef::new("Red witness", "{1}{R}", "Creature — Goblin", "", Some((2, 2))).unwrap(),
        CardDef::new("Blue witness", "{1}{U}", "Creature — Merfolk", "", Some((2, 2))).unwrap(),
    ]
}
fn choose(g: &mut Game, c: Color) -> ObjId {
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::PlayLand(card)).unwrap();
    g.act(0, Move::Choose(Answer::Value(Value::Color(c)))).unwrap();
    *g.battlefield.last().unwrap()
}
fn activate(g: &mut Game, source: ObjId, ability: usize, targets: Vec<Vec<Target>>) -> Result<(), String> {
    g.act(0, Move::Activate { source, ability, targets, paying: vec![] })
}

#[test]
fn r105_1_r614_12a_color_uses_a_typed_choice_before_entry_and_records_its_chooser() {
    let ds = defs();
    for d in &ds {
        game::play::check(d).unwrap_or_else(|e| panic!("{}: {e:?}", d.name()));
    }
    let mut g = Game::new(ds, 3, 20);
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::PlayLand(card)).unwrap();
    cite!(["614.12a"]; assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::Value(Kind::Color) })));
    let Some((0, Asked::Value { kind: Kind::Color, options })) = g.asked() else { panic!("color decision") };
    cite!(["105.4"]; assert_eq!(options.len(), 5));
    cite!(["614.12a"]; assert!(g.battlefield.is_empty()));
    let before = format!("{g:?}");
    crate::support::refused(g.act(1, Move::Choose(Answer::Value(Value::Color(Color::Red)))), "that decision is another player's");
    cite!(["105.4"]; refused("choose one of the five colors (105.4)"): g.act(0, Move::Choose(Answer::Value(Value::CreatureType(Subtype::from_name("Goblin").unwrap())))));
    cite!(["105.4"]; refused("choose one of the five colors (105.4)"): g.act(0, Move::Choose(Answer::Yes)));
    assert_eq!(before, format!("{g:?}"));
    let mut blue = g.clone();
    g.act(0, Move::Choose(Answer::Value(Value::Color(Color::Red)))).unwrap();
    blue.act(0, Move::Choose(Answer::Value(Value::Color(Color::Blue)))).unwrap();
    for (game, color) in [(&g, Color::Red), (&blue, Color::Blue)] {
        let record = &game.obj(game.battlefield[0]).choices[0];
        assert_eq!(record.link, LinkId(AbilityKey::printed(0)));
        assert_eq!(record.chooser, 0);
        assert_eq!(record.value, Value::Color(color));
    }
}

#[test]
fn r607_2d_one_choice_is_shared_by_target_queries_and_continuous_effects() {
    let mut g = Game::new(defs(), 2, 20);
    let source = choose(&mut g, Color::Red);
    let red = g.put(1, 1, Zone::Battlefield);
    let blue = g.put(1, 2, Zone::Battlefield);
    cite!(["607.2d"]; assert_eq!(g.power(red), Some(3)));
    cite!(["607.2d"]; assert_eq!(g.power(blue), Some(2)));
    cite!(["607.2d", "115.1"]; refused("an illegal target, Blue witness: it doesn't match the target's description (115.1)"): activate(&mut g, source, 2, vec![vec![Target::Object(blue)]]));
    assert!(!g.obj(source).tapped);
    cite!(["607.2d"]; ok: activate(&mut g, source, 2, vec![vec![Target::Object(red)]]));
    // Removing the source also removes its static bonus, but not the activated ability's
    // source incarnation and its choice (113.7a, 607.2d).
    g.move_object(source, Zone::Graveyard);
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    cite!(["113.7a", "607.2d"]; assert!(!g.battlefield.contains(&red)));
    assert!(g.battlefield.contains(&blue));
}

#[test]
fn r106_6_restricted_mana_keeps_its_linked_choice_after_the_source_leaves() {
    let mut g = Game::new(defs(), 2, 20);
    let source = choose(&mut g, Color::Red);
    activate(&mut g, source, 1, vec![]).unwrap();
    assert_eq!(g.players[0].pool.restricted[0].ability, Some(AbilityKey::printed(1)));
    g.move_object(source, Zone::Graveyard);
    g.players[0].pool.colors[Color::Red as usize] = 1;
    g.players[0].pool.colors[Color::Blue as usize] = 1;
    let red = g.put(0, 1, Zone::Hand);
    let blue = g.put(0, 2, Zone::Hand);
    // Leave only the matching colored payment in each independent attempt, so unrestricted
    // mana cannot pay the generic part instead of the restricted mana being tested.
    let mut blue_game = g.clone();
    blue_game.players[0].pool.colors[Color::Red as usize] = 0;
    let restricted = g.players[0].pool.restricted[0].clone();
    let mut blue_payment = mana_pool("U");
    blue_payment.restricted.push(restricted.clone());
    cite!(["106.6"]; refused("that mana can't be spent on that: its text restricts how it can be spent (106.6)"): blue_game.act(0, Move::Paying(blue_payment, Box::new(Move::Cast { card: blue, targets: vec![] }))));
    g.players[0].pool.colors[Color::Blue as usize] = 0;
    let mut red_payment = mana_pool("R");
    red_payment.restricted.push(restricted);
    g.act(0, Move::Paying(red_payment, Box::new(Move::Cast { card: red, targets: vec![] }))).unwrap();
    cite!(["106.6"]; assert_eq!(g.players[0].pool.total(), 0));
}

#[test]
fn r400_7_r607_5a_a_new_object_has_an_undefined_choice_not_the_previous_incarnations() {
    let mut g = Game::new(defs(), 2, 20);
    let source = choose(&mut g, Color::Red);
    let red = g.put(1, 1, Zone::Battlefield);
    let set = ObjectSet {
        domain: Domain::Permanent,
        filter: Filter::All(vec![
            Filter::Atom(Atom::Type(CardType::Creature)),
            Filter::Atom(Atom::Reference(ir::reference::UnresolvedRef::choice(Kind::Color))),
        ]),
        controller: None,
        owner: None,
        caster: None,
        zone: None,
        other: false,
    };
    assert!(in_set(&g, &Ctx::new(0, source).with_ability(2), &set, Target::Object(red)).unwrap());
    g.move_object(source, Zone::Graveyard);
    let new = g.players[0].graveyard[0];
    cite!(["400.7"]; assert!(g.obj(new).choices.is_empty()));
    cite!(["400.7"]; assert!(!in_set(&g, &Ctx::new(0, new).with_ability(2), &set, Target::Object(red)).unwrap()));
    assert!(in_set(&g, &Ctx::new(0, source).with_ability(2), &set, Target::Object(red)).unwrap());
}

#[test]
fn r607_2d_missing_wrong_kind_and_ambiguous_producers_are_not_guessed() {
    for text in [
        "{T}: Destroy target creature of the chosen color.",
        "As this land enters, choose a creature type.\n{T}: Destroy target creature of the chosen color.",
    ] {
        assert_eq!(game::play::check(&land(text)).unwrap_err().0,"linked choice: no Color producer (607.2d)");
    }
    let ambiguous = Links::group(Origin::Own, 3, &[(0, Kind::Color), (1, Kind::Color)]);
    assert_eq!(ambiguous.bind(AbilityKey::printed(2), Kind::Color).unwrap_err().0,"linked choice: ambiguous Color producers (607.2d)");
    let missing = Links::group(Origin::Own, 1, &[]);
    assert_eq!(missing.bind(AbilityKey::printed(0), Kind::CreatureType).unwrap_err().0,"linked choice: no CreatureType producer (607.2d)");
}

#[test]
fn r607_4_r607_5_link_groups_keep_acquired_pairs_distinct_at_the_binding_boundary() {
    // This unit fixture checks the provenance binding boundary, without claiming
    // an executed ability acquisition: identical ability indices in distinct provenance groups must never alias.
    let what = game::ability::template(&CardDef::new("Elf", "", "Creature", "{T}: Add {G}.", None).unwrap().abilities()[0]).unwrap();
    let given = Origin::Given { grant: Grant { by: None, stamp: 7, part: 0 }, what };
    let printed = Links::group(Origin::Own, 3, &[(0, Kind::Color)]);
    let acquired = Links::group(given.clone(), 3, &[(0, Kind::Color)]);
    let first = printed.bind(AbilityKey::printed(1), Kind::Color).unwrap();
    cite!(["607.4"]; assert_eq!(first, printed.bind(AbilityKey::printed(2), Kind::Color).unwrap()));
    let second = acquired.bind(AbilityKey { origin: given, index: 1 }, Kind::Color).unwrap();
    cite!(["607.5"]; assert_ne!(first, second));
    assert_eq!(acquired.bind(AbilityKey::printed(1), Kind::Color).unwrap_err().0,"linked choice: unknown ability provenance or kind (607.5)");
}

#[test]
fn r608_2c_compiled_program_selectors_read_the_same_linked_color_as_legacy_abilities() {
    let mut ds = defs();
    ds[0] = land(
        "As this land enters, choose a color.\n{T}: Reveal the top four cards of your library. Put all cards of the chosen color revealed this way into your hand and the rest into your graveyard.",
    );
    game::play::check(&ds[0]).unwrap();
    let mut g = Game::new(ds, 2, 20);
    let source = choose(&mut g, Color::Red);
    g.put(0, 1, Zone::Library);
    g.put(0, 2, Zone::Library);
    activate(&mut g, source, 1, vec![]).unwrap();
    g.move_object(source, Zone::Exile);
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    assert_eq!(g.players[0].hand.len(), 1);
    cite!(["607.2d"]; assert_eq!(g.obj(g.players[0].hand[0]).chars.name, "Red witness"));
    assert_eq!(g.players[0].graveyard.len(), 1);
    cite!(["607.2d"]; assert_eq!(g.obj(g.players[0].graveyard[0]).chars.name, "Blue witness"));
}

#[test]
fn r603_7_delayed_instructions_keep_the_creating_abilitys_linked_choice() {
    let mut ds = defs();
    ds[0] = land(
        "As this land enters, choose a color.\n{T}: This land deals 2 damage to target creature. When that creature dies this turn, destroy all creatures of the chosen color.",
    );
    game::play::check(&ds[0]).unwrap();
    let mut g = Game::new(ds, 2, 20);
    let source = choose(&mut g, Color::Red);
    let red = g.put(1, 1, Zone::Battlefield);
    let blue = g.put(1, 2, Zone::Battlefield);
    let bystander = g.put(1, 2, Zone::Battlefield);
    activate(&mut g, source, 1, vec![vec![Target::Object(blue)]]).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    cite!(["603.7"]; assert_eq!(g.stack.len(), 1, "the delayed ability triggered"));
    assert!(g.battlefield.contains(&red));
    g.move_object(source, Zone::Graveyard);
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    cite!(["607.2d"]; assert!(!g.battlefield.contains(&red)));
    cite!(["607.2d"]; assert!(g.battlefield.contains(&bystander), "the delayed effect still filters by the chosen color"));
}

#[test]
fn r106_6_the_payment_planner_does_not_merge_sources_with_different_linked_choices() {
    let mut ds = defs();
    ds[0] = land(
        "As this land enters, choose a color.\n{1}, {T}: Add {C}{C}{C}. Spend this mana only to cast a spell of the chosen color.",
    );
    ds.push(
        CardDef::new(
            "Restore witness",
            "{0}",
            "Sorcery",
            "Put target land card from a graveyard onto the battlefield under your control.",
            None,
        )
        .unwrap(),
    );
    ds.push(CardDef::new("Payment witness", "{3}{U}", "Creature — Merfolk", "", Some((2, 2))).unwrap());
    for d in &ds {
        game::play::check(d).unwrap();
    }
    let mut g = Game::new(ds, 2, 20);
    let red_source = choose(&mut g, Color::Red);
    let buried = g.put(0, 0, Zone::Graveyard);
    let restore = g.put(0, 3, Zone::Hand);
    g.act(0, Move::Cast { card: restore, targets: vec![vec![Target::Object(buried)]] }).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    g.act(0, Move::Choose(Answer::Value(Value::Color(Color::Blue)))).unwrap();
    let blue_source = *g.battlefield.last().unwrap();
    g.players[0].pool.colorless = 1;
    g.players[0].pool.colors[Color::Blue as usize] = 1;
    let card = g.put(0, 4, Zone::Hand);
    for (source, succeeds) in [(red_source,false),(blue_source,true)] {
        let mut branch=g.clone();
        branch.act(0,crate::support::paid("C",Move::Activate{source,ability:1,targets:vec![],paying:vec![]})).unwrap();
        // The entire authored three-C lot carries opaque compiled provenance.
        let units=branch.players[0].pool.restricted.clone();
        assert_eq!(units.len(),3);
        assert!(units.iter().all(|u|u.source==source && u.ability==Some(AbilityKey::printed(1)) && u.mana==ir::mana::ManaSymbol::Colorless));
        let mut payment=mana_pool("U");payment.restricted=units;
        let action=Move::Paying(payment,Box::new(Move::Cast{card,targets:vec![]}));
        if succeeds {
            cite!(["106.6"]; ok:branch.act(0,action));
            assert!(branch.obj(blue_source).tapped && !branch.obj(red_source).tapped);
            assert_eq!(branch.players[0].pool.total(),0);assert_eq!(branch.stack.len(),1);
        } else {
            let before=format!("{branch:?}");
            cite!(["106.6"]; refused("that mana can't be spent on that: its text restricts how it can be spent"):branch.act(0,action));
            assert_eq!(format!("{branch:?}"),before);
        }
    }
}
