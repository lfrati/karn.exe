//! Emblems are independent public command-zone objects (114), whose compiled triggered
//! abilities use the ordinary stack, controller, targeting and retention rules.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use game::state::Decision;
use ir::{Part, Sort};

const VENSER: &str = "+2: Exile target permanent you own. Return it to the battlefield under your control at the beginning of the next end step.\n−1: Creatures can't be blocked this turn.\n−8: You get an emblem with \"Whenever you cast a spell, exile target permanent.\"";
fn game() -> Game {
    let defs = vec![
        CardDef::planeswalker("Venser, the Sojourner", "{3}{W}{U}", "Legendary Planeswalker — Venser", VENSER, 3).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Blessing", "{0}", "Instant", "You gain 1 life.", None).unwrap(),
        CardDef::new("Land", "", "Land", "", None).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap_or_else(|e| panic!("{}: {}", def.name(), e.0)); }
    Game::new(defs, 3, 20)
}
fn pass(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }
fn activate(g: &mut Game, source: ObjId, ability: usize, targets: Vec<Vec<Target>>) {
    g.act(g.priority, Move::Activate { source, ability, targets, paying: vec![] }).unwrap();
}
fn emblem(g: &mut Game) -> (ObjId, ObjId) {
    let source = g.put(0, 0, Zone::Battlefield);
    g.set_initial_counters(source, "loyalty", 8);
    activate(g, source, 2, vec![]);
    assert!(!g.battlefield.contains(&source), "paying the ultimate's last loyalty removes Venser");
    pass(g);
    (source, *g.command.last().unwrap())
}
fn cast(g: &mut Game, player: usize) {
    let card = g.put(player, 2, Zone::Hand);
    g.act(player, Move::Cast { card, targets: vec![] }).unwrap();
}
fn advance(g: &mut Game, done: impl Fn(&Game) -> bool) {
    for _ in 0..150 {
        if done(g) { return; }
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            None => g.act(g.priority, Move::Pass).unwrap(),
            other => panic!("unexpected {other:?}"),
        }
    }
    panic!("did not reach requested point");
}

#[test]
fn r114_2_3_emblem_is_public_owned_controlled_and_neither_card_token_nor_permanent() {
    let mut g = game();
    let (source, emblem) = emblem(&mut g);
    let o = g.obj(emblem);
    cite!(["114.2"]; assert_eq!((o.zone, o.owner, o.controller), (Zone::Command, 0, 0)));
    assert!(!o.token());
    for sort in [Sort::Card, Sort::Permanent, Sort::Spell, Sort::Ability] { cite!(["114.5"]; case: format!("{sort:?}"); assert!(!g.is(Target::Object(emblem), sort))); }
    cite!(["114.3"]; assert!(o.chars.name.is_empty() && o.chars.types.is_empty() && o.chars.colors.is_empty()));
    for p in 0..3 { cite!(["400.2"]; assert!(g.sees(p, emblem))); }
    assert_eq!(g.abilities(emblem).len(), 1);
    assert!(g.log.iter().any(|e| matches!(e, Event::EmblemCreated { object } if *object == emblem)));
    g.retire_events(g.log.end());
    g.collect_departed();
    assert!(g.get_object(source).is_none(), "the emblem must not retain its creator");
    assert_eq!(g.command, vec![emblem]);
    assert!(g.get_object(emblem).is_some(), "live command-zone objects remain roots");
    let target = g.put(1, 1, Zone::Battlefield);
    cast(&mut g, 0);
    assert!(matches!(g.decision, Some(Decision::Targets { player: 0, .. })));
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(target)]])).unwrap();
    pass(&mut g);
    cite!(["114.4"]; assert!(g.exile.iter().any(|o| g.obj(*o).card == Some(1))));
}

#[test]
fn r114_4_controller_spell_trigger_uses_ordinary_targets_and_cloned_occurrences() {
    let mut g = game(); let (_, emblem) = emblem(&mut g);
    let a = g.put(0, 1, Zone::Battlefield); let b = g.put(1, 1, Zone::Battlefield);
    g.act(0, Move::Pass).unwrap();
    cast(&mut g, 1);
    assert!(g.decision.is_none()); cite!(["109.5"]; assert_eq!(g.stack.len(), 1, "opponent's spell does not trigger your emblem"));
    pass(&mut g);
    cast(&mut g, 0);
    crate::support::refused(g.act(1, Move::ChooseTargets(vec![vec![Target::Object(b)]])), "that decision is another player's");
    cite!(["114.5"]; refused("an illegal target, an emblem: it isn't a permanent on the battlefield (115.1)"): g.act(0, Move::ChooseTargets(vec![vec![Target::Object(emblem)]])));
    cite!(["115.1"]; refused("an illegal target, that player: a player isn't a permanent (115.1)"): g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)]])));
    let clone = g.clone();
    for (mut branch, chosen, remaining) in [(g, a, b), (clone, b, a)] {
        branch.act(0, Move::ChooseTargets(vec![vec![Target::Object(chosen)]])).unwrap();
        let stack = *branch.stack.last().unwrap();
        cite!(["113.7"]; assert_eq!(branch.obj(stack).ability.unwrap().0.unwrap(), emblem));
        pass(&mut branch);
        assert!(!branch.battlefield.contains(&chosen)); assert!(branch.battlefield.contains(&remaining));
        cite!(["603.3"]; assert_eq!(branch.stack.len(), 1, "spell waits below its trigger"));
        pass(&mut branch); assert_eq!(branch.players[0].life, 21);
    }
}

#[test]
fn r114_2_multiple_emblems_trigger_independently_and_illegal_target_does_not_retarget() {
    let mut g = game(); let (_, first) = emblem(&mut g); let (_, second) = emblem(&mut g);
    assert_ne!(first, second);
    assert!(std::ptr::eq(g.def(first).unwrap(), g.def(second).unwrap()), "each emblem shares one compiled definition");
    let a = g.put(1, 1, Zone::Battlefield); let b = g.put(2, 1, Zone::Battlefield);
    cast(&mut g, 0);
    cite!(["603.3b"]; assert!(matches!(g.decision, Some(Decision::Order { player: 0, .. }))));
    g.act(0, Move::Order(vec![0, 1])).unwrap();
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(a)]])).unwrap();
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(b)]])).unwrap();
    cite!(["603.2"]; assert_eq!(g.stack.len(), 3));
    g.move_object(b, Zone::Hand);
    pass(&mut g); pass(&mut g);
    assert_eq!(g.exile.len(), 1);
    assert_eq!(g.obj(g.exile[0]).owner, 1);
    assert!(g.players[2].hand.iter().any(|o| g.obj(*o).card == Some(1)));
    cite!(["608.2b"]; assert!(g.log.iter().any(|e| matches!(e, Event::Fizzled { .. }))));
    assert_eq!(g.command.len(), 2);
}

#[test]
fn r603_7_full_venser_plus_two_delayed_return_survives_source_departure_and_clone() {
    let mut g = game(); let source = g.put(0, 0, Zone::Battlefield);
    g.set_initial_counters(source, "loyalty", 3);
    let own = g.put(0, 1, Zone::Battlefield); let other = g.put(1, 1, Zone::Battlefield);
    cite!(["115.1"]; refused("an illegal target, Bear: the target is one you own, and another player owns it (115.1)"): g.act(0, Move::Activate { source, ability: 0, targets: vec![vec![Target::Object(other)]], paying: vec![] }));
    activate(&mut g, source, 0, vec![vec![Target::Object(own)]]); pass(&mut g);
    assert_eq!(g.exile.len(), 1); g.destroy(source);
    advance(&mut g, |g| g.part == Part::End && !g.stack.is_empty());
    let clone = g.clone();
    for mut branch in [g, clone] { pass(&mut branch); assert!(branch.exile.is_empty());
        cite!(["603.7"]; assert!(branch.battlefield.iter().any(|o| branch.obj(*o).owner == 0 && branch.obj(*o).card == Some(1) && *o != own))); }
}

#[test]
fn r611_2_full_venser_minus_one_affects_all_creatures_for_this_turn() {
    let mut g = game(); let source = g.put(0, 0, Zone::Battlefield);
    let a = g.put(0, 1, Zone::Battlefield); let b = g.put(1, 1, Zone::Battlefield);
    g.set_initial_counters(source, "loyalty", 3);
    activate(&mut g, source, 1, vec![]); pass(&mut g);
    assert!(g.cant_block(b, a, &[b])); assert!(g.cant_block(a, b, &[a]));
    let c = g.put(2, 1, Zone::Battlefield); cite!(["611.2c"]; assert!(g.cant_block(b, c, &[b])));
    let turn = g.turn; advance(&mut g, |g| g.turn > turn);
    cite!(["611.2a"]; assert!(!g.cant_block(b, a, &[b])));
}

#[test]
fn r114_4_unsupported_command_zone_ability_consumers_are_refused() {
    for ability in ["{0}: You gain 1 life."] {
        let def = CardDef::new("Maker", "{0}", "Sorcery", &format!("You get an emblem with \"{ability}\""), None).unwrap();
        assert!(game::play::check(&def).unwrap_err().0.contains("emblem"));
    }
}

#[test]
fn r114_2_recipient_owns_and_controls_emblem_and_it_leaves_when_they_leave() {
    let maker = CardDef::new("Maker", "{0}", "Sorcery", "Target opponent gets an emblem with \"Whenever you cast a spell, you gain 1 life.\"", None).unwrap();
    let spell = CardDef::new("Blessing", "{0}", "Instant", "You gain 1 life.", None).unwrap();
    game::play::check(&maker).unwrap();
    let mut g = Game::new(vec![maker, spell], 3, 20);
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![vec![Target::Player(1)]] }).unwrap(); pass(&mut g);
    let emblem = g.command[0]; cite!(["114.2"]; assert_eq!((g.obj(emblem).owner, g.obj(emblem).controller), (1, 1)));
    g.act(0, Move::Pass).unwrap();
    let card = g.put(1, 1, Zone::Hand);
    g.act(1, Move::Cast { card, targets: vec![] }).unwrap();
    assert_eq!(g.stack.len(), 2); pass(&mut g); pass(&mut g);
    cite!(["109.5"]; assert_eq!(g.players[1].life, 22)); assert_eq!(g.players[0].life, 20);
    g.lose_life(1, 22).unwrap(); g.state_based_actions();
    assert!(g.players[1].lost); cite!(["800.4a"]; assert!(g.command.is_empty()));
    assert_eq!(g.obj(emblem).zone, Zone::Gone);
}

#[test]
fn r603_2_emblem_sees_only_events_after_creation_and_step_triggers_use_command_zone() {
    let maker = CardDef::new("Maker", "{0}", "Sorcery", "You gain 1 life. You get an emblem with \"Whenever you gain life, draw a card.\"", None).unwrap();
    let step = CardDef::new("Step", "{0}", "Sorcery", "You get an emblem with \"At the beginning of your end step, you gain 1 life.\"", None).unwrap();
    for d in [&maker, &step] { game::play::check(d).unwrap(); }
    let mut g = Game::new(vec![maker, step], 2, 20);
    g.put(0, 1, Zone::Library); // One card for the later life-gain trigger.
    for i in 0..2 { let card = g.put(0, i, Zone::Hand); g.act(0, Move::Cast { card, targets: vec![] }).unwrap(); pass(&mut g); }
    cite!(["603.2"]; assert!(g.stack.is_empty(), "life gained before the emblem existed did not trigger it"));
    assert_eq!(g.players[0].life, 21); assert!(g.players[0].hand.is_empty());
    advance(&mut g, |g| g.part == Part::End && !g.stack.is_empty());
    pass(&mut g); cite!(["114.4", "603.2b"]; assert_eq!(g.players[0].life, 22));
    cite!(["603.2"]; assert_eq!(g.stack.len(), 1, "the older emblem sees the later life gain"));
    pass(&mut g); assert_eq!(g.players[0].hand.len(), 1);
}
