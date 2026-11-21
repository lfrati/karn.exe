//! A fixed selection of cards the engine runs: native cards, expectations derived from CR and Oracle.
use crate::support::{cite, oracle, paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::{CardType, Color, Part};

fn setup(name: &str, players: usize) -> Game {
    let mut g = Game::new(
        vec![
            oracle(name),
            CardDef::new("Land", "", "Basic Land — Forest", "", None).unwrap(),
            CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
            CardDef::new("Wall", "{0}", "Creature — Wall", "", Some((1, 4))).unwrap(),
            CardDef::new("Gift", "{0}", "Instant", "You gain 1 life.", None).unwrap(),
            CardDef::new("Boost", "{0}", "Instant", "Target creature gets +3/+0 until end of turn.", None).unwrap(),
            CardDef::new("Destroy", "{0}", "Instant", "Destroy target creature.", None).unwrap(),
            CardDef::new("Shield", "{0}", "Instant", "Target creature gains indestructible until end of turn.", None)
                .unwrap(),
            CardDef::new("Vehicle", "{0}", "Artifact — Vehicle", "Crew 1", Some((3, 3))).unwrap(),
            CardDef::new("Rock", "{0}", "Artifact", "", None).unwrap(),
            CardDef::new(
                "Raise",
                "{0}",
                "Instant",
                "Return target creature card from your graveyard to the battlefield.",
                None,
            )
            .unwrap(),
            oracle("Island"),
            oracle("Mountain"),
            oracle("Plains"),
        ],
        players,
        20,
    );
    for p in 0..players {
        for _ in 0..20 {
            g.put(p, 1, Zone::Library);
        }
    }
    g
}
fn resolve(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn cast(g: &mut Game, def: usize, targets: Vec<Vec<Target>>, spending: &str) -> ObjId {
    assert_eq!(g.priority, 0);
    let card = g.put(0, def, Zone::Hand);
    g.act(0, paid(spending, Move::Cast { card, targets })).unwrap();
    card
}
fn step(g: &mut Game) {
    let mv = match &g.decision {
        Some(Decision::Attackers { .. }) => Move::Attack(vec![]),
        Some(Decision::Blockers { .. }) => Move::Block(vec![]),
        Some(other) => panic!("unexpected {other:?}"),
        None => Move::Pass,
    };
    let p = g.asked().unwrap().0;
    g.act(p, mv).unwrap();
}
fn until(g: &mut Game, turn: u32, part: Part) {
    for _ in 0..300 {
        if g.turn == turn && g.part == part {
            return;
        }
        step(g);
    }
    panic!("boundary not reached");
}
fn ability(
    g: &mut Game,
    source: ObjId,
    index: usize,
    targets: Vec<Vec<Target>>,
    paying: Vec<Vec<ObjId>>,
    spending: &str,
) -> Result<(), String> {
    g.act(0, paid(spending, Move::Activate { source, ability: index, targets, paying }))
}
fn mana(g: &mut Game, generic: u32, colors: &[(Color, u32)]) {
    g.players[0].pool.colorless = generic;
    for (c, n) in colors {
        g.players[0].pool.colors[*c as usize] = *n;
    }
}

// Rules: 603.2, 601.2c, 608.2h, 611.2a, 514.2
// X reads current source power at resolution, not at trigger creation. Another excludes self.
#[test]
fn r608_war_machine_current_power_own_other_target_and_cleanup() {
    let mut g = setup("War Machine, Legacy of Iron", 2);
    let source = g.put(0, 0, Zone::Battlefield);
    let bear = g.put(0, 2, Zone::Battlefield);
    let enemy = g.put(1, 2, Zone::Battlefield);
    until(&mut g, 1, Part::BeginningOfCombat);
    cite!(["603.2"]; assert!(matches!(g.decision,Some(Decision::Targets{player:0,..}))));
    for target in [source, enemy] {
        cite!(["601.2c"]; refused("target"): g.act(0,Move::ChooseTargets(vec![vec![Target::Object(target)]])));
    }
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(bear)]])).unwrap();
    cast(&mut g, 5, vec![vec![Target::Object(source)]], "");
    resolve(&mut g);
    resolve(&mut g);
    cite!(["608.2h"]; assert_eq!(g.power(bear),Some(6))); // 2 + (1 + 3)
    until(&mut g, 2, Part::PrecombatMain);
    cite!(["514.2","611.2a"]; assert_eq!(g.power(bear),Some(2)));
    cite!(["603.2"]; assert!(g.stack.is_empty())); // no residual own-turn trigger
}

// Rules: 603.6a, 119.3, 702.122a, 301.7b, 514.2
#[test]
fn r603_debris_beetle_multiplayer_entry_once_and_crew_two() {
    let mut g = setup("Debris Beetle", 3);
    let bear = g.put(0, 2, Zone::Battlefield);
    mana(&mut g, 2, &[(Color::Black, 1), (Color::Green, 1)]);
    cast(&mut g, 0, vec![], "CCBG");
    resolve(&mut g);
    cite!(["603.6a"]; assert_eq!(g.stack.len(),1));
    cite!(["603.3"]; assert_eq!(g.players.iter().map(|p|p.life).collect::<Vec<_>>(),vec![20,20,20]));
    resolve(&mut g);
    cite!(["119.3"]; assert_eq!(g.players.iter().map(|p|p.life).collect::<Vec<_>>(),vec![23,17,17]));
    let beetle = *g.battlefield.iter().find(|o| g.obj(**o).chars.name == "Debris Beetle").unwrap();
    let index = g.abilities(beetle).iter().position(|a| matches!(&**a, ir::Ability::Activated { .. })).unwrap();
    cite!(["301.7a"]; assert_eq!(g.power(beetle),None));
    ability(&mut g, beetle, index, vec![], vec![vec![bear]], "").unwrap();
    cite!(["702.122a"]; assert!(g.obj(bear).tapped));
    resolve(&mut g);
    cite!(["301.7b","702.122a"]; assert_eq!((g.power(beetle),g.toughness(beetle)),(Some(6),Some(6))));
    assert!(g.has(beetle, "Trample"));
    until(&mut g, 2, Part::PrecombatMain);
    cite!(["514.2"]; assert!(!g.is_a(beetle,CardType::Creature)));
}

// Rules: 614.1d, 605.3b, 602.5d, 701.57a, 400.7
#[test]
fn r701_hidden_nursery_entry_mana_sorcery_cost_and_discover_hand() {
    let mut g = setup("Hidden Nursery", 2);
    let land = g.put(0, 0, Zone::Hand);
    g.act(0, Move::PlayLand(land)).unwrap();
    let nursery = *g.battlefield.last().unwrap();
    cite!(["614.1d"]; assert!(g.obj(nursery).tapped));
    until(&mut g, 3, Part::PrecombatMain);
    ability(&mut g, nursery, 1, vec![], vec![], "").unwrap();
    cite!(["605.3b"]; assert!(g.stack.is_empty()));
    assert_eq!(g.players[0].pool.colors[Color::Green as usize], 1);
    until(&mut g, 5, Part::PrecombatMain);
    g.put(0, 2, Zone::Library);
    g.put(0, 1, Zone::Library);
    mana(&mut g, 4, &[(Color::Green, 1)]);
    let mut combat = g.clone();
    until(&mut combat, 5, Part::BeginningOfCombat);
    mana(&mut combat, 4, &[(Color::Green, 1)]);
    cite!(["602.5d"]; refused("sorcery"): ability(&mut combat,nursery,2,vec![],vec![], "CCCCG"));
    ability(&mut g, nursery, 2, vec![], vec![], "CCCCG").unwrap();
    cite!(["602.2b","701.21a"]; assert!(!g.battlefield.contains(&nursery)));
    resolve(&mut g);
    let Some(Decision::Choose { player: 0, choice: Choice::Cast { cards } }) = g.decision.clone() else {
        panic!("discover offer {:?}", g.decision)
    };
    cite!(["701.57a"]; assert_eq!(cards.len(),1));
    assert_eq!(g.obj(cards[0]).chars.name, "Bear");
    g.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["701.57a"]; assert_eq!(g.players[0].hand.iter().map(|o|g.obj(*o).chars.name.as_str()).collect::<Vec<_>>(),vec!["Land","Land","Bear"]));
    assert!(g.exile.is_empty());
    assert!(g.stack.is_empty());
}

// Rules: 701.23b, 603.6a, 602.2b, 602.5d, 701.7a
#[test]
fn r701_jeskai_monument_basic_search_and_paid_two_flying_birds() {
    let mut g = setup("Jeskai Monument", 2);
    g.put(0, 11, Zone::Library);
    g.put(0, 12, Zone::Library);
    g.put(0, 13, Zone::Library);
    mana(&mut g, 2, &[]);
    cast(&mut g, 0, vec![], "CC");
    resolve(&mut g);
    resolve(&mut g);
    let Some(Decision::Choose { player: 0, choice: Choice::Search { among, .. } }) = g.decision.clone() else {
        panic!("search {:?}", g.decision)
    };
    cite!(["701.23b"]; assert_eq!(among.len(),3)); // Forest fillers excluded
    g.act(0, Move::Choose(Answer::Objects(vec![among[0]]))).unwrap();
    let monument = *g.battlefield.iter().find(|o| g.obj(**o).chars.name == "Jeskai Monument").unwrap();
    mana(&mut g, 1, &[(Color::Blue, 1), (Color::Red, 1), (Color::White, 1)]);
    ability(&mut g, monument, 1, vec![], vec![], "CURW").unwrap();
    cite!(["602.2b","701.21a"]; assert!(!g.battlefield.contains(&monument)));
    resolve(&mut g);
    let birds: Vec<_> = g.battlefield.iter().copied().filter(|o| g.obj(*o).token()).collect();
    cite!(["701.7a"]; assert_eq!(birds.len(),2));
    for bird in birds {
        assert_eq!((g.power(bird), g.toughness(bird)), (Some(1), Some(1)));
        assert!(g.has(bird, "Flying"));
        assert!(g.is_a(bird, CardType::Creature));
    }
}

// Rules: 608.2c, 400.7, 110.4, 603.6a
#[test]
fn r608_return_light_realms_only_own_nonland_permanents() {
    let mut g = setup("Return to the Light Realms", 2);
    for def in [1, 2, 4, 9] {
        g.put(0, def, Zone::Graveyard);
    }
    g.put(1, 2, Zone::Graveyard);
    mana(&mut g, 7, &[(Color::White, 2)]);
    cast(&mut g, 0, vec![], "CCCCCCCWW");
    resolve(&mut g);
    let names: Vec<_> = g.battlefield.iter().map(|o| g.obj(*o).chars.name.as_str()).collect();
    cite!(["608.2c","110.4"]; assert_eq!(names,vec!["Bear","Rock"]));
    cite!(["109.5"]; assert_eq!(g.players[1].graveyard.len(),1));
    cite!(["608.2n"]; assert_eq!(g.players[0].graveyard.len(),3)); // land, instant, resolved sorcery
}

// Rules: 601.2c, 608.2b, 613.4c, 704.5f, 702.12b
#[test]
fn r704_overkill_zero_toughness_bypasses_indestructible_and_accepts_own_target() {
    let mut g = setup("Overkill", 2);
    let bear = g.put(0, 2, Zone::Battlefield);
    cast(&mut g, 7, vec![vec![Target::Object(bear)]], "");
    resolve(&mut g);
    assert!(g.has(bear, "Indestructible"));
    mana(&mut g, 2, &[(Color::Black, 1)]);
    cast(&mut g, 0, vec![vec![Target::Object(bear)]], "CCB");
    resolve(&mut g);
    cite!(["704.5f","702.12b"]; assert!(!g.battlefield.contains(&bear)));
    assert_eq!(g.players[0].graveyard.len(), 3);
}

// Rules: 608.2b, 733.1
#[test]
fn r608_overkill_illegal_target_and_noncreature_refusal() {
    let mut g = setup("Overkill", 2);
    let rock = g.put(1, 9, Zone::Battlefield);
    let bear = g.put(1, 2, Zone::Battlefield);
    mana(&mut g, 2, &[(Color::Black, 1)]);
    let spell = g.put(0, 0, Zone::Hand);
    cite!(["601.2c"]; refused("target"): g.act(0,Move::Cast{card:spell,targets:vec![vec![Target::Object(rock)]]}));
    assert!(g.players[0].hand.contains(&spell));
    g.act(0, paid("CCB", Move::Cast { card: spell, targets: vec![vec![Target::Object(bear)]] })).unwrap();
    cast(&mut g, 6, vec![vec![Target::Object(bear)]], "");
    resolve(&mut g);
    resolve(&mut g);
    cite!(["608.2b"]; assert!(g.battlefield.contains(&rock)));
    assert_eq!(g.players[1].graveyard.len(), 1);
}

// Rules: 702.177a, 122.1a, 514.2, 702.189a
#[test]
fn r702_rhino_exhaust_once_counters_persist_trample_expires() {
    let mut g = setup("Rough Rhino Cavalry", 2);
    let rhino = g.put(0, 0, Zone::Battlefield);
    mana(&mut g, 8, &[]);
    ability(&mut g, rhino, 1, vec![], vec![], "CCCCCCCC").unwrap();
    resolve(&mut g);
    cite!(["122.1a"]; assert_eq!((g.power(rhino),g.toughness(rhino)),(Some(7),Some(7))));
    assert!(g.has(rhino, "Trample"));
    mana(&mut g, 8, &[]);
    cite!(["702.177a"]; refused("only once"): ability(&mut g,rhino,1,vec![],vec![], "CCCCCCCC"));
    until(&mut g, 2, Part::PrecombatMain);
    cite!(["514.2"]; assert!(!g.has(rhino,"Trample")));
    assert_eq!(g.power(rhino), Some(7));
}

// Rules: 603.7b, 707.10, 707.10c
#[test]
fn r603_teach_next_qualifying_spell_once_copy_is_not_cast() {
    let mut g = setup("Teach by Example", 2);
    mana(&mut g, 0, &[(Color::Blue, 1), (Color::Red, 1)]);
    cast(&mut g, 0, vec![], "UR");
    resolve(&mut g);
    cast(&mut g, 2, vec![], "");
    cite!(["603.7b"]; assert_eq!(g.stack.len(),1));
    resolve(&mut g);
    cast(&mut g, 4, vec![], "");
    cite!(["603.7b"]; assert_eq!(g.stack.len(),2));
    resolve(&mut g);
    // Non-targeted spell has no retarget question.
    cite!(["707.10"]; assert_eq!(g.stack.len(),2));
    resolve(&mut g);
    resolve(&mut g);
    cite!(["707.10"]; assert_eq!(g.players[0].life,22));
    cast(&mut g, 4, vec![], "");
    cite!(["603.7b"]; assert_eq!(g.stack.len(),1));
    resolve(&mut g);
    assert_eq!(g.players[0].life, 23);
}

// Rules: 601.2f, 702.9b, 702.20b, 508.1f, 510.1a
#[test]
fn r702_serra_paid_native_cast_flying_blocks_and_vigilance_attack() {
    let mut g = setup("Serra Angel", 2);
    let blocker = g.put(1, 2, Zone::Battlefield);
    mana(&mut g, 3, &[(Color::White, 2)]);
    cast(&mut g, 0, vec![], "CCCWW");
    resolve(&mut g);
    let angel = *g.battlefield.iter().find(|o| g.obj(**o).chars.name == "Serra Angel").unwrap();
    cite!(["601.2f"]; assert_eq!(g.players[0].pool.colors[Color::White as usize],0));
    assert_eq!((g.power(angel), g.toughness(angel)), (Some(4), Some(4)));
    until(&mut g, 3, Part::DeclareAttackers);
    g.act(0, Move::Attack(vec![(angel, Target::Player(1))])).unwrap();
    cite!(["508.1f","702.20b"]; assert!(!g.obj(angel).tapped));
    cite!(["702.9b"]; assert!(g.cant_block(blocker,angel,&[blocker])));
    until(&mut g, 3, Part::CombatDamage);
    cite!(["510.1a"]; assert_eq!(g.players[1].life,16));
}

// Rules: 602.2b, 608.2h, 120.3e, 701.8a, 301.7a
#[test]
fn r608_scrap_compactor_sacrificed_source_damage_and_noncreature_vehicle_destroy() {
    let mut g = setup("Scrap Compactor", 2);
    let wall = g.put(1, 3, Zone::Battlefield);
    let rock = g.put(0, 0, Zone::Battlefield);
    mana(&mut g, 3, &[]);
    ability(&mut g, rock, 0, vec![vec![Target::Object(wall)]], vec![], "CCC").unwrap();
    cite!(["602.2b"]; assert!(!g.battlefield.contains(&rock)));
    resolve(&mut g);
    cite!(["120.3e","608.2h"]; assert_eq!(g.obj(wall).damage,3));
    assert!(g.battlefield.contains(&wall));
    let second = g.put(0, 0, Zone::Battlefield);
    let vehicle = g.put(1, 8, Zone::Battlefield);
    cite!(["301.7a"]; assert!(!g.is_a(vehicle,CardType::Creature)));
    mana(&mut g, 6, &[]);
    ability(&mut g, second, 1, vec![vec![Target::Object(vehicle)]], vec![], "CCCCCC").unwrap();
    resolve(&mut g);
    cite!(["701.8a"]; assert!(!g.battlefield.contains(&vehicle)));
}

// Rules: 613.4c, 704.5f, 514.2, 611.2a
// 9999 is a finite reduction, not an unconditional kill. Power is unchanged.
#[test]
fn r613_overkill_exact_reduction_surviving_boundary_and_expiry() {
    let mut g = setup("Overkill", 2);
    let giant = CardDef::new("Giant", "{0}", "Creature — Giant", "", Some((7, 10000))).unwrap();
    std::sync::Arc::make_mut(&mut g.defs).push(giant);
    let target = g.put(1, 14, Zone::Battlefield);
    mana(&mut g, 2, &[(Color::Black, 1)]);
    cast(&mut g, 0, vec![vec![Target::Object(target)]], "CCB");
    resolve(&mut g);
    cite!(["613.4c","704.5f"]; assert_eq!((g.power(target),g.toughness(target)),(Some(7),Some(1))));
    assert!(g.battlefield.contains(&target));
    until(&mut g, 2, Part::PrecombatMain);
    cite!(["514.2","611.2a"]; assert_eq!(g.toughness(target),Some(10000)));
}

// Rules: 608.2h, 113.7a, 603.3d
#[test]
fn r608_war_machine_departed_source_uses_last_known_power() {
    let mut g = setup("War Machine, Legacy of Iron", 2);
    let source = g.put(0, 0, Zone::Battlefield);
    let bear = g.put(0, 2, Zone::Battlefield);
    cast(&mut g, 5, vec![vec![Target::Object(source)]], "");
    resolve(&mut g);
    until(&mut g, 1, Part::BeginningOfCombat);
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(bear)]])).unwrap();
    cast(&mut g, 6, vec![vec![Target::Object(source)]], "");
    resolve(&mut g);
    resolve(&mut g);
    cite!(["608.2h","113.7a"]; assert_eq!(g.power(bear),Some(6)));
}

// Rules: 701.23b, 205.4c
// The Basic requirement applies to all three alternatives, not just Island.
#[test]
fn r701_jeskai_monument_excludes_nonbasic_mountain_and_plains() {
    let mut g = setup("Jeskai Monument", 2);
    for subtype in ["Island", "Mountain", "Plains"] {
        let name = format!("Nonbasic {subtype}");
        let d = CardDef::new(&name, "", &format!("Land — {subtype}"), "", None).unwrap();
        let i = g.defs.len();
        std::sync::Arc::make_mut(&mut g.defs).push(d);
        g.put(0, i, Zone::Library);
    }
    let basic = g.put(0, 13, Zone::Library);
    mana(&mut g, 2, &[]);
    cast(&mut g, 0, vec![], "CC");
    resolve(&mut g);
    resolve(&mut g);
    let Some(Decision::Choose { player: 0, choice: Choice::Search { among, .. } }) = g.decision.clone() else {
        panic!("search");
    };
    cite!(["701.23b","205.4c"]; assert_eq!(among,vec![basic]));
}

// Rules: 702.189a, 106.4, 511.3
#[test]
fn r702_rhino_native_firebending_two_waits_for_resolution_and_ends_with_combat() {
    let mut g = setup("Rough Rhino Cavalry", 2);
    let rhino = g.put(0, 0, Zone::Battlefield);
    until(&mut g, 1, Part::DeclareAttackers);
    g.act(0, Move::Attack(vec![(rhino, Target::Player(1))])).unwrap();
    cite!(["702.189a"]; assert_eq!(g.stack.len(),1));
    assert_eq!(red(&g), 0);
    resolve(&mut g);
    cite!(["702.189a"]; assert_eq!(red(&g),2));
    until(&mut g, 1, Part::EndOfCombat);
    cite!(["702.189a"]; assert_eq!(red(&g),2));
    until(&mut g, 1, Part::PostcombatMain);
    cite!(["106.4","511.3"]; assert_eq!(red(&g),0));
}

// Rules: 603.7b, 707.10c, 115.7d
#[test]
fn r707_teach_targeted_copy_can_change_target_without_changing_original() {
    let mut g = setup("Teach by Example", 2);
    let own = g.put(0, 2, Zone::Battlefield);
    let enemy = g.put(1, 3, Zone::Battlefield);
    mana(&mut g, 0, &[(Color::Red, 2)]);
    cast(&mut g, 0, vec![], "RR");
    resolve(&mut g);
    cast(&mut g, 5, vec![vec![Target::Object(own)]], "");
    resolve(&mut g);
    let Some(Decision::Choose { player: 0, choice: Choice::Retarget { .. } }) = g.decision.clone() else {
        panic!("retarget {:?}", g.decision)
    };
    g.act(0, Move::Choose(Answer::Targets(vec![vec![Target::Object(enemy)]]))).unwrap();
    resolve(&mut g);
    cite!(["707.10c"]; assert_eq!(g.power(enemy),Some(4)));
    assert_eq!(g.power(own), Some(2));
    resolve(&mut g);
    cite!(["115.7d"]; assert_eq!(g.power(own),Some(5)));
}

// Rules: 603.7b, 514.2
#[test]
fn r603_teach_unconsumed_registration_expires_at_cleanup() {
    let mut g = setup("Teach by Example", 2);
    mana(&mut g, 0, &[(Color::Blue, 2)]);
    cast(&mut g, 0, vec![], "UU");
    resolve(&mut g);
    until(&mut g, 3, Part::PrecombatMain);
    cast(&mut g, 4, vec![], "");
    cite!(["603.7b","514.2"]; assert_eq!(g.stack.len(),1));
    resolve(&mut g);
    assert_eq!(g.players[0].life, 21);
}

fn red(g: &Game) -> u32 {
    let pool = &g.players[0].pool;
    pool.colors[Color::Red as usize]
        + pool.restricted.iter().filter(|m| m.mana == ir::mana::ManaSymbol::Color(Color::Red)).count() as u32
}

// Rules: 603.2, 107.1b, 608.2h
// CR107.1b's Colossus example makes a negative +X bonus zero. The opponent's
// beginning of combat is a separate occurrence, not the opponent's main phase.
#[test]
fn r107_war_machine_negative_power_bonus_is_zero_and_opponent_combat_does_not_trigger() {
    let mut g = setup("War Machine, Legacy of Iron", 2);
    let source = g.put(0, 0, Zone::Battlefield);
    let bear = g.put(0, 2, Zone::Battlefield);
    let shrink = CardDef::new("Shrink", "{0}", "Instant", "Target creature gets -3/-0 until end of turn.", None).unwrap();
    std::sync::Arc::make_mut(&mut g.defs).push(shrink);
    cast(&mut g, 14, vec![vec![Target::Object(source)]], "");
    resolve(&mut g);
    cite!(["107.1b"]; assert_eq!(g.power(source), Some(-2)));
    until(&mut g, 1, Part::BeginningOfCombat);
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(bear)]])).unwrap();
    resolve(&mut g);
    cite!(["107.1b", "608.2h"]; assert_eq!(g.power(bear), Some(2)));
    until(&mut g, 2, Part::BeginningOfCombat);
    cite!(["603.2"]; assert!(g.decision.is_none()));
    cite!(["603.2"]; assert!(g.stack.is_empty()));
}

// Rules: 701.23b, 602.5d, 111.3, 205.3m, 702.9b
// A qualified hidden-zone search may fail even with qualifying cards present.
// Tokens retain every explicitly stated characteristic, not only their stats.
#[test]
fn r701_jeskai_monument_failed_search_white_bird_tokens_and_combat_timing() {
    let mut g = setup("Jeskai Monument", 2);
    let island = g.put(0, 11, Zone::Library);
    mana(&mut g, 2, &[]);
    cast(&mut g, 0, vec![], "CC");
    resolve(&mut g);
    resolve(&mut g);
    let Some(Decision::Choose { player: 0, choice: Choice::Search { among, .. } }) = g.decision.clone() else {
        panic!("expected own library search");
    };
    cite!(["701.23b"]; assert_eq!(among, vec![island]));
    cite!(["701.23b"]; ok: g.act(0, Move::Choose(Answer::Objects(vec![]))));
    cite!(["701.23b"]; assert!(g.players[0].hand.is_empty()));
    assert_eq!(g.players[0].library.len(), 21);
    let monument = *g.battlefield.last().unwrap();
    let mut combat = g.clone();
    until(&mut combat, 1, Part::BeginningOfCombat);
    mana(&mut combat, 1, &[(Color::Blue, 1), (Color::Red, 1), (Color::White, 1)]);
    cite!(["602.5d"]; refused("sorcery"): ability(&mut combat, monument, 1, vec![], vec![], "CURW"));
    assert!(combat.battlefield.contains(&monument));
    mana(&mut g, 1, &[(Color::Blue, 1), (Color::Red, 1), (Color::White, 1)]);
    ability(&mut g, monument, 1, vec![], vec![], "CURW").unwrap();
    resolve(&mut g);
    let birds: Vec<_> = g.battlefield.iter().copied().filter(|o| g.obj(*o).token()).collect();
    cite!(["111.3"]; assert_eq!(birds.len(), 2));
    for bird in birds {
        cite!(["111.3"]; assert_eq!(g.obj(bird).chars.colors, vec![Color::White]));
        cite!(["205.3m", "111.3"]; assert_eq!(g.obj(bird).chars.subtypes, vec![ir::Subtype::from_name("Bird").unwrap()]));
        cite!(["111.3"]; assert_eq!((g.power(bird), g.toughness(bird)), (Some(1), Some(1))));
        cite!(["702.9b"]; assert!(g.has(bird, "Flying")));
    }
}

// Rules: 702.177a, 400.7, 122.2, 122.1a
// A returned Rhino is a new object: old counters and the old exhaust limit do
// not follow it. Its printed exhaust can be used again in that same turn.
#[test]
fn r702_rhino_reentry_resets_exhaust_and_discards_old_counters() {
    let mut g = setup("Rough Rhino Cavalry", 2);
    let old = g.put(0, 0, Zone::Battlefield);
    mana(&mut g, 8, &[]);
    ability(&mut g, old, 1, vec![], vec![], "CCCCCCCC").unwrap();
    resolve(&mut g);
    cite!(["122.1a"]; assert_eq!(g.power(old), Some(7)));
    cast(&mut g, 6, vec![vec![Target::Object(old)]], "");
    resolve(&mut g);
    let dead = *g.players[0].graveyard.iter().find(|o| g.obj(**o).chars.name == "Rough Rhino Cavalry").unwrap();
    cast(&mut g, 10, vec![vec![Target::Object(dead)]], "");
    resolve(&mut g);
    let new = *g.battlefield.iter().find(|o| g.obj(**o).chars.name == "Rough Rhino Cavalry").unwrap();
    cite!(["400.7"]; assert_ne!(new, old));
    cite!(["122.2"]; assert_eq!((g.power(new), g.toughness(new)), (Some(5), Some(5))));
    mana(&mut g, 8, &[]);
    cite!(["400.7", "702.177a"]; ok: ability(&mut g, new, 1, vec![], vec![], "CCCCCCCC"));
    resolve(&mut g);
    cite!(["122.1a"]; assert_eq!((g.power(new), g.toughness(new)), (Some(7), Some(7))));
    mana(&mut g, 8, &[]);
    cite!(["702.177a"]; refused("only once"): ability(&mut g, new, 1, vec![], vec![], "CCCCCCCC"));
}

// Rules: 601.2c, 608.2b, 602.2b, 701.21a
// Paid sacrifice stays paid when the sole target leaves in response. Damage
// mode excludes a noncreature Vehicle, while destroy mode includes it.
#[test]
fn r608_scrap_compactor_illegal_damage_target_keeps_sacrifice_paid() {
    let mut g = setup("Scrap Compactor", 2);
    let source = g.put(0, 0, Zone::Battlefield);
    let vehicle = g.put(1, 8, Zone::Battlefield);
    let bear = g.put(1, 2, Zone::Battlefield);
    mana(&mut g, 3, &[]);
    cite!(["601.2c"]; refused("target"): ability(&mut g, source, 0, vec![vec![Target::Object(vehicle)]], vec![], "CCC"));
    assert!(g.battlefield.contains(&source));
    ability(&mut g, source, 0, vec![vec![Target::Object(bear)]], vec![], "CCC").unwrap();
    cite!(["602.2b", "701.21a"]; assert!(!g.battlefield.contains(&source)));
    cast(&mut g, 6, vec![vec![Target::Object(bear)]], "");
    resolve(&mut g);
    resolve(&mut g);
    cite!(["608.2b"]; assert!(g.battlefield.contains(&vehicle)));
    cite!(["608.2b"]; assert_eq!(g.players[1].graveyard.len(), 1));
    cite!(["602.2b"]; assert_eq!(g.players[0].graveyard.iter().filter(|o| g.obj(**o).chars.name == "Scrap Compactor").count(), 1));
    assert!(g.stack.is_empty());
}

// Rules: 303.4f, 303.4g, 603.6a, 608.2c
// With no preexisting creature, an Aura cannot enchant a simultaneous entrant.
// In contrast, the newly returned entry watcher sees the other arriving creature.
#[test]
fn r603_return_light_realms_simultaneous_watcher_and_unattachable_aura() {
    let mut g = setup("Return to the Light Realms", 2);
    let watcher = CardDef::new("Watcher", "{0}", "Creature — Cleric",
        "Whenever another creature you control enters, you gain 1 life.", Some((1, 1))).unwrap();
    std::sync::Arc::make_mut(&mut g.defs).extend([watcher, oracle("Pacifism")]);
    for def in [2, 14, 15] { g.put(0, def, Zone::Graveyard); }
    mana(&mut g, 7, &[(Color::White, 2)]);
    cast(&mut g, 0, vec![], "CCCCCCCWW");
    resolve(&mut g);
    let names: Vec<_> = g.battlefield.iter().map(|o| g.obj(*o).chars.name.as_str()).collect();
    cite!(["303.4f", "303.4g"]; assert_eq!(names, vec!["Bear", "Watcher"]));
    cite!(["303.4g"]; assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).chars.name == "Pacifism")));
    cite!(["603.6a"]; assert_eq!(g.stack.len(), 1));
    cite!(["603.3"]; assert_eq!(g.players[0].life, 20));
    resolve(&mut g);
    cite!(["603.6a"]; assert_eq!(g.players[0].life, 21));
}
