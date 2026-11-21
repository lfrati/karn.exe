//! A fixed selection of ten cards; native rule boundaries, never inferred coverage.
use crate::support::{cite, oracle, paid};
use game::legal::Asked;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

fn setup(name: &str) -> Game {
    let defs = vec![
        oracle(name),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Wall", "{0}", "Creature — Wall", "", Some((1, 7))).unwrap(),
        CardDef::new("Gift", "{0}", "Instant", "You gain 1 life.", None).unwrap(),
        CardDef::new("Bounce", "{0}", "Instant", "Return target creature to its owner's hand.", None).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Ponder", "{0}", "Instant", "Scry 1.", None).unwrap(),
        CardDef::new("Observe", "{0}", "Instant", "Surveil 1.", None).unwrap(),
        CardDef::new(
            "Treasure",
            "{0}",
            "Artifact — Treasure",
            "{T}, Sacrifice this artifact: Add one mana of any color.",
            None,
        )
        .unwrap(),
    ];
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 {
        for _ in 0..30 {
            g.put(p, 5, Zone::Library);
        }
    }
    g
}
fn cast(g: &mut Game, p: usize, d: usize, targets: Vec<Vec<Target>>, spending: &str) -> ObjId {
    while g.priority != p {
        g.act(g.priority, Move::Pass).unwrap();
    }
    let c = g.put(p, d, Zone::Hand);
    g.act(p, paid(spending, Move::Cast { card: c, targets })).unwrap();
    c
}
fn resolve(g: &mut Game) {
    for _ in 0..2 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn settle(g: &mut Game) {
    for _ in 0..20 {
        if g.stack.is_empty() {
            return;
        }
        resolve(g);
    }
    panic!("unsettled");
}
fn advance(g: &mut Game, turn: u32) {
    for _ in 0..200 {
        if g.turn == turn && g.part == Part::PrecombatMain {
            return;
        }
        let (p, q) = g.asked().unwrap();
        g.act(
            p,
            match q {
                Asked::Priority => Move::Pass,
                Asked::Attackers(_) => Move::Attack(vec![]),
                Asked::Blockers { .. } => Move::Block(vec![]),
                x => panic!("{x:?}"),
            },
        )
        .unwrap();
    }
    panic!("advance exhausted");
}
fn pay(g: &mut Game, w: u32, u: u32, b: u32, r: u32, green: u32, c: u32) {
    g.players[0].pool.colors = [w, u, b, r, green];
    g.players[0].pool.colorless = c;
}

// Rules: 608.2c, 608.2h, 611.2d, 608.2b
// Treasure is made first, then counted once. A chosen illegal target prevents all effects;
// choosing zero targets still creates Treasure.
#[test]
fn r608_gold_rush_counts_new_treasure_once_and_revalidates_optional_target() {
    let mut g = setup("Gold Rush");
    let bear = g.put(0, 1, Zone::Battlefield);
    let old = g.put(0, 8, Zone::Battlefield);
    pay(&mut g, 0, 0, 0, 0, 1, 1);
    cast(&mut g, 0, 0, vec![vec![Target::Object(bear)]], "CG");
    settle(&mut g);
    cite!(["608.2c", "608.2h", "611.2d"]; assert_eq!(g.power(bear),Some(6)));
    g.move_object(old, Zone::Graveyard);
    cite!(["611.2d"]; assert_eq!(g.power(bear),Some(6)));
    advance(&mut g, 2);
    cite!(["514.2"]; assert_eq!(g.power(bear),Some(2)));
    advance(&mut g, 3);
    pay(&mut g, 0, 0, 0, 0, 1, 1);
    cast(&mut g, 0, 0, vec![vec![]], "CG");
    settle(&mut g);
    let treasures =
        g.battlefield.iter().filter(|o| g.obj(**o).chars.subtypes.iter().any(|s| s.name() == "Treasure")).count();
    assert_eq!(treasures, 2);
    pay(&mut g, 0, 0, 0, 0, 1, 1);
    cast(&mut g, 0, 0, vec![vec![Target::Object(bear)]], "CG");
    cast(&mut g, 0, 4, vec![vec![Target::Object(bear)]], "");
    resolve(&mut g);
    resolve(&mut g);
    cite!(["608.2b"]; assert_eq!(g.battlefield.iter().filter(|o|g.obj(**o).chars.subtypes.iter().any(|s| s.name()=="Treasure")).count(),treasures));
}

// Rules: 608.2b, 608.2c, 608.2h, 701.6a
// Uncounterable does not prevent target invalidation. Controller information survives exile.
#[test]
fn r608_inevitable_defeat_uses_exiled_controller_and_illegal_target_loses_no_life() {
    let mut g = setup("Inevitable Defeat");
    let bear = g.put(1, 1, Zone::Battlefield);
    pay(&mut g, 1, 0, 1, 1, 0, 1);
    let spell = cast(&mut g, 0, 0, vec![vec![Target::Object(bear)]], "CWBR");
    cite!(["701.6a"]; assert!(!g.counter(spell)));
    settle(&mut g);
    cite!(["608.2c", "608.2h"]; assert_eq!((g.players[0].life,g.players[1].life),(23,17)));
    assert!(g.exile.iter().any(|o| g.obj(*o).chars.name == "Bear"));
    let bear = g.put(1, 1, Zone::Battlefield);
    pay(&mut g, 1, 0, 1, 1, 0, 1);
    cast(&mut g, 0, 0, vec![vec![Target::Object(bear)]], "CWBR");
    cast(&mut g, 0, 4, vec![vec![Target::Object(bear)]], "");
    resolve(&mut g);
    resolve(&mut g);
    cite!(["608.2b"]; assert_eq!((g.players[0].life,g.players[1].life),(23,17)));
}

// Rules: 614.1c, 614.12, 605.3b
// Zero/one existing other lands enter tapped; two enter untapped. Mana choices resolve directly.
#[test]
fn r614_shipwreck_marsh_other_land_threshold_and_both_mana_choices() {
    for others in 0..=2 {
        let mut g = setup("Shipwreck Marsh");
        for _ in 0..others {
            g.put(0, 5, Zone::Battlefield);
        }
        let c = g.put(0, 0, Zone::Hand);
        g.act(0, Move::PlayLand(c)).unwrap();
        let marsh = *g.battlefield.last().unwrap();
        cite!(["614.1c", "614.12"]; case:others; assert_eq!(g.obj(marsh).tapped,others<2));
        if others == 2 {
            for color in [ir::Color::Blue, ir::Color::Black] {
                let mut branch = g.clone();
                branch
                    .act(
                        0,
                        Move::Adding(
                            vec![vec![ir::mana::ManaSymbol::Color(color)]],
                            Box::new(Move::Activate { source: marsh, ability: 1, targets: vec![], paying: vec![] }),
                        ),
                    )
                    .unwrap();
                cite!(["605.3b"]; assert_eq!(branch.players[0].pool.colors[color as usize],1));
                assert!(branch.stack.is_empty());
            }
        }
    }
}

// Rules: 603.2, 603.3, 614.1c, 605.3b
#[test]
fn r603_stark_industries_entry_life_waits_for_resolution() {
    let mut g = setup("Stark Industries");
    let c = g.put(0, 0, Zone::Hand);
    g.act(0, Move::PlayLand(c)).unwrap();
    let land = *g.battlefield.last().unwrap();
    cite!(["614.1c"]; assert!(g.obj(land).tapped));
    cite!(["603.3"]; assert_eq!(g.players[0].life,20));
    assert_eq!(g.stack.len(), 1);
    settle(&mut g);
    cite!(["608.2c"]; assert_eq!(g.players[0].life,21));
}

// Rules: 608.2c, 608.2h, 611.2a, 514.2
// Infusion is checked at resolution, including life gained in response; keywords expire,
// counters remain and gaining life afterward does not retroactively add keywords.
#[test]
fn r608_efflorescence_infusion_resolution_time_and_cleanup() {
    for gained in [false, true] {
        let mut g = setup("Efflorescence");
        let bear = g.put(1, 1, Zone::Battlefield);
        pay(&mut g, 0, 0, 0, 0, 1, 2);
        cast(&mut g, 0, 0, vec![vec![Target::Object(bear)]], "CCG");
        if gained {
            cast(&mut g, 0, 3, vec![], "");
            resolve(&mut g);
        }
        resolve(&mut g);
        cite!(["608.2c", "608.2h"]; case:gained; assert_eq!((g.has(bear,"Trample"),g.has(bear,"Indestructible")),(gained,gained)));
        assert_eq!(g.obj(bear).counters.get("+1/+1"), Some(&2));
        if !gained {
            cast(&mut g, 0, 3, vec![], "");
            settle(&mut g);
            assert!(!g.has(bear, "Trample"));
        }
        advance(&mut g, 2);
        cite!(["514.2", "611.2a"]; assert!(!g.has(bear,"Indestructible")));
        assert_eq!(g.power(bear), Some(4));
    }
}

// Rules: 603.2, 603.3, 701.22d, 701.25d, 109.5
// One action (not each moved card) triggers once; only the source controller's action.
#[test]
fn r603_denzilore_scry_and_surveil_each_put_one_counter_on_each_own_creature() {
    for def in [6, 7] {
        let mut g = setup("Denzilore Fatehold");
        let sphinx = g.put(0, 0, Zone::Battlefield);
        let mine = g.put(0, 1, Zone::Battlefield);
        let theirs = g.put(1, 1, Zone::Battlefield);
        assert!(g.has(sphinx, "Flash") && g.has(sphinx, "Flying"));
        cast(&mut g, 0, def, vec![], "");
        resolve(&mut g);
        let top = *g.players[0].library.last().unwrap();
        g.act(0, Move::Choose(game::state::Answer::Arrange { top: vec![top], away: vec![] })).unwrap();
        cite!(["603.2", "603.3"]; assert_eq!(g.stack.len(),1));
        assert!(g.obj(mine).counters.is_empty());
        settle(&mut g);
        cite!(["608.2c"]; assert_eq!(g.obj(mine).counters.get("+1/+1"),Some(&1)));
        assert_eq!(g.obj(sphinx).counters.get("+1/+1"), Some(&1));
        assert!(g.obj(theirs).counters.is_empty());
        cast(&mut g, 1, def, vec![], "");
        resolve(&mut g);
        let top = *g.players[1].library.last().unwrap();
        g.act(1, Move::Choose(game::state::Answer::Arrange { top: vec![top], away: vec![] })).unwrap();
        cite!(["109.5", "603.2"]; assert!(g.stack.is_empty()));
    }
}

// Rules: 702.193a, 702.193b, 118.7a, 400.7, 602.5b
// Native {G} reduces entry-turn {3}{G} to {3}; a new incarnation has a fresh use.
#[test]
fn r702_serpent_specialist_entry_reduction_once_and_new_incarnation() {
    let mut g = setup("Serpent Specialist");
    let card = g.put(0, 0, Zone::Hand);
    g.move_object(card, Zone::Battlefield);
    let snake = *g.battlefield.last().unwrap();
    pay(&mut g, 0, 0, 0, 0, 0, 3);
    cite!(["702.193b", "118.7a"]; ok:g.act(0,paid("CCC",Move::Activate {source:snake,ability:1,targets:vec![],paying:vec![]})));
    settle(&mut g);
    cite!(["608.2c"]; assert_eq!(g.obj(snake).counters.get("+1/+1"),Some(&2)));
    assert!(g.has(snake, "Deathtouch"));
    pay(&mut g, 0, 0, 0, 0, 1, 3);
    let before = format!("{g:?}");
    cite!(["702.193a", "602.5b"]; refused("only once"):g.act(0,paid("CCC",Move::Activate {source:snake,ability:1,targets:vec![],paying:vec![]})));
    assert_eq!(format!("{g:?}"), before);
    g.move_object(snake, Zone::Hand);
    let returned = *g.players[0].hand.last().unwrap();
    g.move_object(returned, Zone::Battlefield);
    let fresh = *g.battlefield.last().unwrap();
    cite!(["400.7"]; assert_ne!(snake,fresh));
    pay(&mut g, 0, 0, 0, 0, 0, 3);
    g.act(0, paid("CCC", Move::Activate { source: fresh, ability: 1, targets: vec![], paying: vec![] })).unwrap();
    settle(&mut g);
    cite!(["400.7", "702.193a"]; assert_eq!(g.obj(fresh).counters.get("+1/+1"),Some(&2)));
}

// Rules: 608.2c, 611.2c, 611.2a, 514.2, 602.2b
// Sacrifice is paid immediately; recipients are fixed on resolution (any counter kind).
#[test]
fn r611_bulwark_ox_sacrifice_grant_uses_resolution_counter_recipients() {
    let mut g = setup("Bulwark Ox");
    let ox = g.put(0, 0, Zone::Battlefield);
    let mine = g.put(0, 1, Zone::Battlefield);
    let plain = g.put(0, 1, Zone::Battlefield);
    let theirs = g.put(1, 1, Zone::Battlefield);
    g.set_initial_counters(mine, "charge", 1);
    g.set_initial_counters(theirs, "+1/+1", 1);
    g.act(0, Move::Activate { source: ox, ability: 1, targets: vec![], paying: vec![] }).unwrap();
    cite!(["602.2b"]; assert!(!g.battlefield.contains(&ox)));
    settle(&mut g);
    cite!(["608.2c", "611.2c"]; assert!(g.has(mine,"Hexproof")&&g.has(mine,"Indestructible")));
    assert!(!g.has(plain, "Hexproof") && !g.has(theirs, "Hexproof"));
    g.remove_counters(mine, "charge", 1);
    g.put_counters(plain, "charge", 1);
    cite!(["611.2c"]; assert!(g.has(mine,"Hexproof")&&!g.has(plain,"Hexproof")));
    advance(&mut g, 2);
    cite!(["611.2a", "514.2"]; assert!(!g.has(mine,"Hexproof")));
}

// Rules: 608.2b, 608.2c, 601.2c, 120.3e
// Required opponent damage target and optional own counter target revalidate independently.
#[test]
fn r608_awaken_inferno_one_surviving_target_still_gets_its_effect() {
    let mut g = setup("Awaken the Inferno");
    let wall = g.put(1, 2, Zone::Battlefield);
    let bear = g.put(0, 1, Zone::Battlefield);
    pay(&mut g, 0, 0, 0, 1, 0, 4);
    cast(&mut g, 0, 0, vec![vec![Target::Object(wall)], vec![Target::Object(bear)]], "CCCCR");
    cast(&mut g, 0, 4, vec![vec![Target::Object(wall)]], "");
    resolve(&mut g);
    resolve(&mut g);
    cite!(["608.2b", "608.2c"]; assert_eq!(g.obj(bear).counters.get("+1/+1"),Some(&1)));
    let wall = g.put(1, 2, Zone::Battlefield);
    pay(&mut g, 0, 0, 0, 1, 0, 4);
    cast(&mut g, 0, 0, vec![vec![Target::Object(wall)], vec![]], "CCCCR");
    settle(&mut g);
    cite!(["120.3e", "608.2c"]; assert_eq!(g.obj(wall).damage,6));
}

// Rules: 614.12, 608.2c
// Existing one land plus three simultaneously entering lands still means one OTHER land
// at the replacement decision; ruling22990a9a297f9558 explicitly excludes fellow entrants.
#[test]
fn r614_shipwreck_marsh_simultaneous_other_entries_are_not_preexisting_lands() {
    let mut g = setup("Shipwreck Marsh");
    g.put(0, 5, Zone::Battlefield);
    g.put(0, 0, Zone::Hand);
    g.put(0, 5, Zone::Hand);
    g.put(0, 5, Zone::Hand);
    let index = g.defs.len();
    std::sync::Arc::make_mut(&mut g.defs).push(
        CardDef::new("Deploy", "{0}", "Instant", "Put all land cards from your hand onto the battlefield.", None)
            .unwrap(),
    );
    cast(&mut g, 0, index, vec![], "");
    settle(&mut g);
    let marsh = *g.battlefield.iter().find(|o| g.obj(**o).chars.name == "Shipwreck Marsh").unwrap();
    cite!(["614.12"]; assert!(g.obj(marsh).tapped));
    assert_eq!(g.battlefield.len(), 4);
}

// Rules: 702.171a, 702.171b, 702.171c, 603.2, 603.3d
// Native attack trigger is absent unsaddled; saddling by another creature resolves first,
// then an opponent's creature may be chosen for the unqualified targeted counter.
#[test]
fn r603_bulwark_ox_native_saddled_attack_targets_any_creature() {
    for saddle in [false, true] {
        let mut g = setup("Bulwark Ox");
        let ox = g.put(0, 0, Zone::Battlefield);
        let bear = g.put(0, 1, Zone::Battlefield);
        let theirs = g.put(1, 1, Zone::Battlefield);
        if saddle {
            let ability = g
                .abilities(ox)
                .iter()
                .position(|a| matches!(&**a,ir::Ability::Activated{cost,..} if format!("{cost:?}").contains("Total")))
                .unwrap();
            g.act(0, Move::Activate { source: ox, ability, targets: vec![], paying: vec![vec![bear]] }).unwrap();
            settle(&mut g);
            cite!(["702.171c"]; assert!(g.obj(bear).tapped));
        }
        for _ in 0..30 {
            if matches!(g.asked(), Some((0, Asked::Attackers(_)))) {
                break;
            }
            g.act(g.priority, Move::Pass).unwrap();
        }
        g.act(0, Move::Attack(vec![(ox, Target::Player(1))])).unwrap();
        if saddle {
            let Some((0, Asked::Targets { .. })) = g.asked() else {
                panic!("attack trigger target request {:?}", g.asked())
            };
            g.act(0, Move::ChooseTargets(vec![vec![Target::Object(theirs)]])).unwrap();
            settle(&mut g);
            cite!(["603.2", "608.2c"]; assert_eq!(g.obj(theirs).counters.get("+1/+1"),Some(&1)));
        } else {
            cite!(["603.2", "702.171b"]; assert!(g.stack.is_empty()));
        }
    }
}

// Rules: 702.171b, 707.2, 613.1f
// Saddled is a designation, not an ability or copiable characteristic.
#[test]
fn r702_bulwark_saddled_survives_ability_loss_but_is_not_copied() {
    let mut g = setup("Bulwark Ox");
    let ox = g.put(0, 0, Zone::Battlefield);
    let rider = g.put(0, 1, Zone::Battlefield);
    let saddle = g
        .abilities(ox)
        .iter()
        .position(|a| matches!(&**a,ir::Ability::Activated{cost,..} if format!("{cost:?}").contains("Total")))
        .unwrap();
    g.act(0, Move::Activate { source: ox, ability: saddle, targets: vec![], paying: vec![vec![rider]] }).unwrap();
    settle(&mut g);
    let index = g.defs.len();
    std::sync::Arc::make_mut(&mut g.defs).extend([
        CardDef::new("Mirror", "{0}", "Instant", "Create a token that's a copy of target creature.", None).unwrap(),
        CardDef::new("Strip", "{0}", "Instant", "Target creature loses all abilities until end of turn.", None)
            .unwrap(),
    ]);
    cast(&mut g, 0, index, vec![vec![Target::Object(ox)]], "");
    settle(&mut g);
    let copy = *g.battlefield.last().unwrap();
    cite!(["707.2", "702.171b"]; assert!(g.obj(ox).saddled&&!g.obj(copy).saddled));
    cast(&mut g, 0, index + 1, vec![vec![Target::Object(ox)]], "");
    settle(&mut g);
    cite!(["613.1f", "702.171b"]; assert!(g.abilities(ox).is_empty()&&g.obj(ox).saddled));
}

// Rules: 701.22d, 701.25d, 603.2, 608.2c, 701.22b, 701.25c
// Oracle: Denzilore's "Whenever you scry or surveil"; no associated rulings.
// Positive N on an empty library still triggers; N=0 explicitly does not.
#[test]
fn r603_cycle1_denzilore_empty_library_positive_and_zero_actions() {
    for text in ["Scry 1.", "Surveil 1.", "Scry 0.", "Surveil 0."] {
        let defs = setup("Denzilore Fatehold").defs.clone();
        let mut g = Game::new(defs.as_ref().clone(), 2, 20);
        let sphinx = g.put(0, 0, Zone::Battlefield);
        let index = g.defs.len();
        std::sync::Arc::make_mut(&mut g.defs).push(CardDef::new("Arrange", "{0}", "Instant", text, None).unwrap());
        cast(&mut g, 0, index, vec![], "");
        resolve(&mut g);
        let positive = text.contains('1');
        cite!(["701.22d", "701.25d", "701.22b", "701.25c", "603.2"]; case:text; assert_eq!(g.stack.len(), usize::from(positive)));
        settle(&mut g);
        cite!(["608.2c"]; assert_eq!(g.obj(sphinx).counters.get("+1/+1").copied().unwrap_or(0), u32::from(positive)));
    }
}

// Rules: 702.29e, 602.2b, 701.23a, 701.23b, 701.24a
// Oracle: Awaken's basic landcycling {2} searches a BASIC LAND, not any land.
// No associated rulings. Discard is paid before search; finding nothing is allowed.
#[test]
fn r702_cycle1_awaken_basic_landcycling_native_search_and_fail_to_find() {
    for find in [false, true] {
        let mut g = setup("Awaken the Inferno");
        let nonbasic_index = g.defs.len();
        std::sync::Arc::make_mut(&mut g.defs).push(oracle("Shipwreck Marsh"));
        let nonbasic = g.put(0, nonbasic_index, Zone::Library);
        let basics = g.players[0]
            .library
            .iter()
            .copied()
            .filter(|o| *o != nonbasic)
            .collect::<Vec<_>>();
        let card = g.put(0, 0, Zone::Hand);
        let ability = g
            .abilities(card)
            .iter()
            .position(|a| matches!(&**a, ir::Ability::Activated { .. }))
            .unwrap();
        pay(&mut g, 0, 0, 0, 0, 0, 2);
        g.act(
            0,
            paid("CC", Move::Activate {
                source: card,
                ability,
                targets: vec![],
                paying: vec![],
            }),
        )
        .unwrap();
        cite!(["602.2b", "702.29e"]; assert!(g.players[0].hand.is_empty() && g.players[0].graveyard.len() == 1));
        cite!(["702.29e"]; assert_eq!(g.players[0].pool.total(), 0));
        resolve(&mut g);
        let Some((0, Asked::Search { among, min, max, .. })) = g.asked() else {
            panic!("native basic search")
        };
        cite!(["702.29e", "701.23a", "701.23b"]; assert_eq!((among, min, max), (basics.clone(), 0, 1)));
        g.act(
            0,
            Move::Choose(game::state::Answer::Objects(if find {
                vec![basics[0]]
            } else {
                vec![]
            })),
        )
        .unwrap();
        cite!(["702.29e", "701.23b"]; assert_eq!(g.players[0].hand.len(), usize::from(find)));
        cite!(["702.29e"]; assert_eq!(g.players[0].library.len(), 31 - usize::from(find)));
        cite!(["701.23a"]; assert!(g.players[0].library.contains(&nonbasic)));
        cite!(["701.24a"]; assert!(!g.sees(0, nonbasic) && !g.sees(1, nonbasic)));
    }
}

// Rules: 109.5, 614.12, 614.1c
// Oracle: Shipwreck counts "you control"; ruling 22990a9a297f9558 excludes
// simultaneous entrants (already exercised above), and opposing lands cannot count either.
#[test]
fn r614_cycle1_shipwreck_opponent_lands_do_not_meet_threshold() {
    let mut g = setup("Shipwreck Marsh");
    g.put(0, 5, Zone::Battlefield);
    for _ in 0..3 {
        g.put(1, 5, Zone::Battlefield);
    }
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::PlayLand(card)).unwrap();
    let marsh = *g.battlefield.last().unwrap();
    cite!(["109.5", "614.12", "614.1c"]; assert!(g.obj(marsh).tapped));
}

// Rules: 608.2c, 608.2h, 119.3, 122.1a
// Oracle: Efflorescence asks whether you GAINED life this turn, not net life change.
// No rulings. Losing that life again does not erase the gain event.
#[test]
fn r608_cycle1_efflorescence_gain_then_loss_still_enables_infusion() {
    let mut g = setup("Efflorescence");
    let bear = g.put(0, 1, Zone::Battlefield);
    let index = g.defs.len();
    std::sync::Arc::make_mut(&mut g.defs)
        .push(CardDef::new("Lose", "{0}", "Instant", "You lose 1 life.", None).unwrap());
    cast(&mut g, 0, 3, vec![], "");
    settle(&mut g);
    cast(&mut g, 0, index, vec![], "");
    settle(&mut g);
    cite!(["119.3"]; assert_eq!(g.players[0].life, 20));
    pay(&mut g, 0, 0, 0, 0, 1, 2);
    cast(&mut g, 0, 0, vec![vec![Target::Object(bear)]], "CCG");
    settle(&mut g);
    cite!(["608.2c", "608.2h", "119.3"]; assert!(g.has(bear, "Trample") && g.has(bear, "Indestructible")));
    cite!(["122.1a"]; assert_eq!((g.power(bear), g.toughness(bear)), (Some(4), Some(4))));
}

// Rules: 608.2c, 608.2h, 119.3
// Oracle: Inevitable targets any nonland permanent, including your own artifact.
// Ruling 35224ded41ba2e92 concerns invalid targets (exercised above).
// With your own target you lose 3 THEN gain 3, ending at 20, and exile the artifact.
#[test]
fn r608_cycle1_inevitable_own_noncreature_permanent_life_net_zero() {
    let mut g = setup("Inevitable Defeat");
    let treasure = g.put(0, 8, Zone::Battlefield);
    pay(&mut g, 1, 0, 1, 1, 0, 1);
    cast(&mut g, 0, 0, vec![vec![Target::Object(treasure)]], "CWBR");
    settle(&mut g);
    cite!(["608.2c", "608.2h", "119.3"]; assert_eq!((g.players[0].life, g.players[1].life), (20, 20)));
    cite!(["608.2c"]; assert!(!g.battlefield.contains(&treasure) && g.exile.len() == 1));
}

// Rules: 702.171a, 702.171b, 702.171c, 514.2
// Oracle: Bulwark saddle 1; rulings 5df3cfeca888a530 (repeat allowed),
// fe78449efd86e12e (other untapped creatures), 709da4a5f8457ab1 (expires).
#[test]
fn r702_cycle1_bulwark_can_saddle_twice_and_designation_expires() {
    let mut g = setup("Bulwark Ox");
    let ox = g.put(0, 0, Zone::Battlefield);
    let riders = [g.put(0, 1, Zone::Battlefield), g.put(0, 1, Zone::Battlefield)];
    let ability = g
        .abilities(ox)
        .iter()
        .position(|a| matches!(&**a, ir::Ability::Activated { cost, .. } if format!("{cost:?}").contains("Total")))
        .unwrap();
    for rider in riders {
        cite!(["702.171a"]; ok: g.act(0, Move::Activate { source: ox, ability, targets: vec![], paying: vec![vec![rider]] }));
        cite!(["702.171c"]; assert!(g.obj(rider).tapped));
        settle(&mut g);
        cite!(["702.171b"]; assert!(g.obj(ox).saddled));
    }
    advance(&mut g, 2);
    cite!(["702.171b", "514.2"]; assert!(!g.obj(ox).saddled));
}

// Rules: 702.193a, 602.5b, 122.1a
// Oracle: Serpent Power-up is once per incarnation, not once per turn. No rulings.
#[test]
fn r702_cycle1_serpent_once_limit_persists_into_next_turn() {
    let mut g = setup("Serpent Specialist");
    let snake = g.put(0, 0, Zone::Battlefield);
    pay(&mut g, 0, 0, 0, 0, 1, 3);
    g.act(
        0,
        paid("CCCG", Move::Activate {
            source: snake,
            ability: 1,
            targets: vec![],
            paying: vec![],
        }),
    )
    .unwrap();
    settle(&mut g);
    advance(&mut g, 3);
    pay(&mut g, 0, 0, 0, 0, 1, 3);
    cite!(["702.193a", "602.5b"]; refused("only once"): g.act(0, paid("CCCG", Move::Activate { source: snake, ability: 1, targets: vec![], paying: vec![] })));
    cite!(["122.1a"]; assert_eq!((g.power(snake), g.toughness(snake)), (Some(3), Some(3))));
}

// Rules: 109.5, 608.2c, 608.2h, 611.2d
// Oracle: Gold Rush may target any creature; counts only YOUR Treasures.
// Ruling 57cd8403c651c9bc fixes bonus on resolution; de919e60001c8327
// optional/illegal target branches are exercised above.
#[test]
fn r608_cycle1_gold_rush_opponent_target_excludes_opponent_treasures() {
    let mut g = setup("Gold Rush");
    let bear = g.put(1, 1, Zone::Battlefield);
    for _ in 0..3 {
        g.put(1, 8, Zone::Battlefield);
    }
    pay(&mut g, 0, 0, 0, 0, 1, 1);
    cast(&mut g, 0, 0, vec![vec![Target::Object(bear)]], "CG");
    settle(&mut g);
    cite!(["109.5", "608.2c", "608.2h", "611.2d"]; assert_eq!((g.power(bear), g.toughness(bear)), (Some(4), Some(4))));
}

// Rules: 502.3, 605.1a, 605.3b, 107.5
// Oracle: Stark's "{T}: Add {U} or {R}"; no rulings. Both alternatives add
// exactly one mana, tap the land, and use no stack after natural untap.
#[test]
fn r605_cycle1_stark_both_native_mana_choices_after_untap() {
    let mut g = setup("Stark Industries");
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::PlayLand(card)).unwrap();
    let land = *g.battlefield.last().unwrap();
    settle(&mut g);
    advance(&mut g, 3);
    cite!(["502.3"]; assert!(!g.obj(land).tapped));
    for color in [ir::Color::Blue, ir::Color::Red] {
        let mut branch = g.clone();
        let ability = branch
            .abilities(land)
            .iter()
            .position(|a| matches!(&**a, ir::Ability::Activated { .. }))
            .unwrap();
        branch
            .act(
                0,
                Move::Adding(
                    vec![vec![ir::mana::ManaSymbol::Color(color)]],
                    Box::new(Move::Activate {
                        source: land,
                        ability,
                        targets: vec![],
                        paying: vec![],
                    }),
                ),
            )
            .unwrap();
        cite!(["605.1a", "605.3b", "107.5"]; assert!(branch.obj(land).tapped && branch.stack.is_empty()));
        cite!(["605.3b"]; assert_eq!((branch.players[0].pool.total(), branch.players[0].pool.colors[color as usize]), (1, 1)));
    }
}

// Rules: 702.17b, 509.1b, 702.10b, 302.6
// Oracle: Rearing Embermare "Reach, haste", 4/5; no rulings.
// Cast on entry turn and attack; separate native reach branch blocks a real flying attacker.
#[test]
fn r702_cycle1_embermare_native_entry_haste_and_reach_block() {
    let mut g = setup("Rearing Embermare");
    pay(&mut g, 0, 0, 0, 1, 0, 4);
    cast(&mut g, 0, 0, vec![], "CCCCR");
    settle(&mut g);
    let horse = *g.battlefield.last().unwrap();
    for _ in 0..30 {
        if matches!(g.asked(), Some((0, Asked::Attackers(_)))) {
            break;
        }
        g.act(g.priority, Move::Pass).unwrap();
    }
    cite!(["702.10b", "302.6"]; ok: g.act(0, Move::Attack(vec![(horse, Target::Player(1))])));
    let mut g = setup("Rearing Embermare");
    let index = g.defs.len();
    std::sync::Arc::make_mut(&mut g.defs).push(oracle("Denzilore Fatehold"));
    let flier = g.put(0, index, Zone::Battlefield);
    let horse = g.put(1, 0, Zone::Battlefield);
    for _ in 0..30 {
        if matches!(g.asked(), Some((0, Asked::Attackers(_)))) {
            break;
        }
        g.act(g.priority, Move::Pass).unwrap();
    }
    g.act(0, Move::Attack(vec![(flier, Target::Player(1))])).unwrap();
    for _ in 0..30 {
        if matches!(g.asked(), Some((1, Asked::Blockers { .. }))) {
            break;
        }
        g.act(g.priority, Move::Pass).unwrap();
    }
    cite!(["702.17b", "509.1b"]; ok: g.act(1, Move::Block(vec![(horse, flier)])));
}
