//! Remaining main-deck compositions use complete corpus definitions, including every ability.
use crate::support::cite;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::CardType;

const COPPERLINE: usize = 0;
const RAZORVERGE: usize = 1;
const SEACHROME: usize = 2;
const FAUNA: usize = 3;
const WURMCOIL: usize = 4;
const DIMIR: usize = 5;
const FOREST: usize = 6;
const BEAR: usize = 7;
const ARTIFACT: usize = 8;
const SORCERY: usize = 9;
const INSTANT: usize = 10;
const RETURN: usize = 11;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let mut defs: Vec<_> = ["Copperline Gorge", "Razorverge Thicket", "Seachrome Coast", "Fauna Shaman", "Wurmcoil Engine", "Dimir Charm"]
        .into_iter().map(|name| CardDef::from_faces(&catalog[name]).unwrap()).collect();
    for (name, cost, types, text, pt) in [
        ("Forest", "", "Basic Land — Forest", "", None),
        ("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        ("Artifact", "{0}", "Artifact", "", None),
        ("Sorcery", "{0}", "Sorcery", "You gain 3 life.", None),
        ("Instant", "{0}", "Instant", "You gain 3 life.", None),
        ("Simultaneous land return", "{0}", "Sorcery", "Return all land cards from your graveyard to the battlefield.", None),
    ] { defs.push(CardDef::new(name, cost, types, text, pt).unwrap()); }
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    Game::new(defs, 2, 20)
}
fn pass(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
fn resolve(g: &mut Game) {
    while g.decision.is_none() && !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
}
fn cast(g: &mut Game, def: usize) {
    let card = g.put(0, def, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    pass(g);
}
fn choose(g: &mut Game, objects: Vec<ObjId>) { g.act(0, Move::Choose(Answer::Objects(objects))).unwrap(); }

#[test]
fn r614_12_fastlands_count_only_other_lands_their_controller_already_controls() {
    for def in [COPPERLINE, RAZORVERGE, SEACHROME] {
        for others in [0, 2, 3] {
            let mut g = game();
            for _ in 0..others { g.put(0, FOREST, Zone::Battlefield); }
            for _ in 0..5 { g.put(1, FOREST, Zone::Battlefield); }
            g.put(0, BEAR, Zone::Battlefield);
            let land = g.put(0, def, Zone::Hand);
            g.act(0, Move::PlayLand(land)).unwrap();
            let arrived = *g.battlefield.last().unwrap();
            assert_eq!(g.obj(arrived).card, Some(def));
            cite!(["614.1d"]; assert_eq!(g.obj(arrived).tapped, others > 2, "definition {def}, {others} other lands"));
            assert!(!g.log.iter().any(|e| matches!(e, Event::Tapped { object } if *object == arrived)), "entering tapped is not tapping");
        }
    }
}

#[test]
fn r614_12_simultaneous_fastlands_do_not_count_one_another_before_entry() {
    for others in [0, 2, 3] {
        let mut g = game();
        for _ in 0..others { g.put(0, FOREST, Zone::Battlefield); }
        for def in [COPPERLINE, RAZORVERGE, SEACHROME] { g.put(0, def, Zone::Graveyard); }
        cast(&mut g, RETURN);
        let arrivals: Vec<_> = g.battlefield.iter().filter(|o| g.obj(**o).card.is_some_and(|d| d <= SEACHROME)).collect();
        assert_eq!(arrivals.len(), 3);
        // Uncited (ruling): fastland ruling 3c59066198a82025, "If one of these lands enters the battlefield at the same time as one or more other lands ..., it doesn't take those lands into consideration"; 614.1d only makes "enters tapped unless" a replacement effect
        assert!(arrivals.iter().all(|o| g.obj(**o).tapped == (others > 2)), "{others} existing lands: all three use the world before entry");
        assert!(g.stack.is_empty() && g.decision.is_none());
    }
}

#[test]
fn r118_3_fauna_discards_only_a_creature_and_search_resumes_without_repaying() {
    let mut g = game();
    let fauna = g.put(0, FAUNA, Zone::Battlefield);
    let discarded = g.put(0, BEAR, Zone::Hand);
    let noncreature = g.put(0, ARTIFACT, Zone::Hand);
    let opponents = g.put(1, BEAR, Zone::Hand);
    let found = g.put(0, WURMCOIL, Zone::Library);
    let land = g.put(0, FOREST, Zone::Library);
    g.players[0].pool.colors[4] = 1;
    for (invalid, name) in [(noncreature, "Artifact"), (opponents, "Bear"), (fauna, "Fauna Shaman")] {
        let why = format!("{name} is not what that cost asks for or cannot pay it (701.9a, 118.3)");
        cite!(["118.3"]; case: name; refused(&why): g.act(0, crate::support::paid("G", Move::Activate { source: fauna, ability: 0, targets: vec![], paying: vec![vec![invalid]] })));
        assert!(!g.obj(fauna).tapped);
        assert_eq!(g.players[0].pool.total(), 1);
        assert!(g.players[0].graveyard.is_empty() && g.stack.is_empty());
    }
    g.act(0, crate::support::paid("G", Move::Activate { source: fauna, ability: 0, targets: vec![], paying: vec![vec![discarded]] })).unwrap();
    assert!(g.obj(fauna).tapped);
    assert_eq!(g.players[0].pool.total(), 0);
    assert_eq!(g.obj(discarded).zone, Zone::Gone);
    cite!(["701.9a"]; assert_eq!(g.players[0].graveyard.len(), 1));
    g.exile(fauna);
    resolve(&mut g);
    assert!(matches!(&g.decision, Some(Decision::Choose { player: 0, choice: Choice::Search { among, min: 0, max: 1, .. } }) if *among == vec![found]));
    cite!(["701.23a"]; refused("a chosen card is not one the search can find, in the searched zone and matching its description (701.23a): choose between 0 and 1 distinct matching objects"): g.act(0, Move::Choose(Answer::Objects(vec![land]))));
    crate::support::refused(g.act(1, Move::Choose(Answer::Objects(vec![found]))), "that decision is another player's");
    for take in [false, true] {
        let mut branch = g.clone();
        choose(&mut branch, if take { vec![found] } else { vec![] });
        cite!(["701.23b"]; assert_eq!(branch.players[0].hand.len(), if take { 2 } else { 1 }));
        assert_eq!(branch.players[0].graveyard.len(), 1);
        assert_eq!(branch.players[0].pool.total(), 0);
        assert_eq!(branch.log.iter().filter(|e| matches!(e, Event::Shuffled { player: 0, .. })).count(), 1);
        let reveals: Vec<_> = branch.log.iter().filter_map(|e| if let Event::Revealed { cards, .. } = e { Some(cards.clone()) } else { None }).collect();
        assert_eq!(reveals, if take { vec![vec![found]] } else { vec![] });
        assert!(branch.stack.is_empty() && branch.decision.is_none());
        assert_eq!(branch.players[0].hand.iter().filter(|o| branch.obj(**o).card == Some(WURMCOIL)).count(), usize::from(take));
    }
}

#[test]
fn r603_6_wurmcoil_death_creates_two_distinct_keyword_baselines() {
    let mut g = game();
    let wurm = g.put(0, WURMCOIL, Zone::Battlefield);
    let bear = g.put(1, BEAR, Zone::Battlefield);
    g.damage(wurm, Target::Object(bear), 1);
    g.state_based_actions();
    cite!(["702.2b", "704.5h"]; assert_eq!(g.obj(bear).zone, Zone::Gone));
    cite!(["702.15b"]; assert_eq!(g.players[0].life, 21));
    assert!(g.destroy(wurm));
    g.act(0, Move::Pass).unwrap();
    assert_eq!(g.stack.len(), 1);
    for mut branch in [g.clone(), g] {
        resolve(&mut branch);
        let tokens: Vec<_> = branch.battlefield.iter().copied().filter(|o| branch.obj(*o).token()).collect();
        cite!(["603.10a"]; assert_eq!(tokens.len(), 2));
        assert_eq!(branch.obj(tokens[0]).chars.name, branch.obj(tokens[1]).chars.name);
        for token in &tokens {
            assert!(branch.is_a(*token, CardType::Artifact) && branch.is_a(*token, CardType::Creature));
            assert!(branch.obj(*token).chars.colors.is_empty());
            assert_eq!((branch.power(*token), branch.toughness(*token)), (Some(3), Some(3)));
            assert_eq!(branch.controller(*token), 0);
            let subtypes: Vec<_> = branch.obj(*token).chars.subtypes.iter().map(|s| s.name()).collect();
            assert_eq!(subtypes, ["Phyrexian", "Wurm"]);
            assert_ne!(branch.has(*token, "Deathtouch"), branch.has(*token, "Lifelink"));
        }
        let death = *tokens.iter().find(|o| branch.has(**o, "Deathtouch")).unwrap();
        let life = *tokens.iter().find(|o| branch.has(**o, "Lifelink")).unwrap();
        let first = branch.put(1, BEAR, Zone::Battlefield);
        let second = branch.put(1, BEAR, Zone::Battlefield);
        branch.damage(death, Target::Object(first), 1);
        branch.damage(life, Target::Object(second), 1);
        branch.state_based_actions();
        cite!(["702.2b", "704.5h"]; assert_eq!(branch.obj(first).zone, Zone::Gone));
        assert_eq!(branch.obj(second).zone, Zone::Battlefield);
        cite!(["702.15b"]; assert_eq!(branch.players[0].life, 22));
        assert!(branch.stack.is_empty());
    }
}

#[test]
fn r700_2_dimir_counter_mode_accepts_only_sorcery_spells() {
    for (def, name) in [(SORCERY, "Sorcery"), (INSTANT, "Instant"), (BEAR, "Bear"), (ARTIFACT, "Artifact")] {
        let mut g = game();
        let target = g.put(0, def, Zone::Hand);
        g.players[0].pool.colors = [9; 5];
        g.act(0, crate::support::paid(if def == BEAR { "WG" } else { "" }, Move::Cast { card: target, targets: vec![] })).unwrap();
        let target = g.stack[0];
        g.act(0, Move::Pass).unwrap();
        let charm = g.put(1, DIMIR, Zone::Hand);
        g.players[1].pool.colors = [0, 1, 1, 0, 0];
        let result = g.act(1, crate::support::paid("UB", Move::Modes(vec![0], Box::new(Move::Cast { card: charm, targets: vec![vec![Target::Object(target)]] }))));
        if def == SORCERY {
            result.unwrap();
            pass(&mut g);
            assert!(g.stack.is_empty());
            cite!(["701.6a"]; assert_eq!(g.players[0].life, 20, "the sorcery never resolves"));
            cite!(["701.6a"]; assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).card == Some(SORCERY))));
            assert!(g.log.iter().any(|e| matches!(e, Event::Countered { object } if *object == target)));
        } else {
            cite!(["115.1"]; case: name; refused(&format!("an illegal target, {name}: it isn't a sorcery (115.1)")): result);
            assert_eq!(g.stack, vec![target]);
            assert_eq!(g.players[1].pool.total(), 2);
            assert_eq!(g.obj(charm).zone, Zone::Hand);
        }
    }
}
