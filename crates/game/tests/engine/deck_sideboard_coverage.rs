//! Sideboard cards as the corpus prints them, each cast or activated and run to its end:
//! Firedrinker Satyr's activated ability, Manabarbs, Abrupt Decay, Gainsay, Ray of
//! Revelation and Ancient Grudge with their flashback, Creeping Corrosion and Shrivel.
use crate::support::cite;
use game::legal::Asked;
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const SATYR: usize = 0;
const MANABARBS: usize = 1;
const DECAY: usize = 2;
const GAINSAY: usize = 3;
const RAY: usize = 4;
const GRUDGE: usize = 5;
const CORROSION: usize = 6;
const SHRIVEL: usize = 7;
const COUNTERSPELL: usize = 8;
const DIVINATION: usize = 9;
const BOLT: usize = 10;
const INGOT: usize = 11;
const ORNITHOPTER: usize = 12;
const FOREST: usize = 13;
const NONBASIC: usize = 14;
const ELF: usize = 15;
const BEAR: usize = 16;
const GIANT: usize = 17;
const AURA: usize = 18;
const BIG_AURA: usize = 19;
const ROCK: usize = 20;
const BLUE_BEAR: usize = 21;
const ARTIFACT_LAND: usize = 22;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let real = |name: &str| CardDef::from_faces(catalog.get(name).unwrap_or_else(|| panic!("{name} is in the corpus"))).unwrap();
    let made = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).unwrap();
    let defs = vec![
        real("Firedrinker Satyr"),
        real("Manabarbs"),
        real("Abrupt Decay"),
        real("Gainsay"),
        real("Ray of Revelation"),
        real("Ancient Grudge"),
        real("Creeping Corrosion"),
        real("Shrivel"),
        real("Counterspell"),
        real("Divination"),
        real("Lightning Bolt"),
        real("Darksteel Ingot"),
        real("Ornithopter"),
        made("Forest", "", "Basic Land — Forest", "", None),
        made("Grove", "", "Land", "{T}: Add {G}.", None),
        made("Elf", "{G}", "Creature — Elf Druid", "{T}: Add {G}.", Some((1, 1))),
        made("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        made("Giant", "{3}{R}", "Creature — Giant", "", Some((5, 3))),
        made("Glow", "{2}{W}", "Enchantment", "", None),
        made("Great Glow", "{3}{W}", "Enchantment", "", None),
        made("Rock", "{3}", "Artifact", "", None),
        made("Blue Bear", "{1}{U}", "Creature — Bear", "", Some((2, 2))),
        made("Vault", "", "Artifact Land", "{T}: Add {C}.", None),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    let mut g = Game::new(defs, 2, 20);
    for player in 0..2 { for _ in 0..12 { g.put(player, FOREST, Zone::Library); } }
    g
}
/// This player's mana pool holds exactly this: white, blue, black, red, green.
fn pool(g: &mut Game, player: usize, colors: [u32; 5]) { g.players[player].pool.colors = colors; }
fn cast(g: &mut Game, player: usize, card: ObjId, targets: &[ObjId], spending: &str) -> Result<(), String> {
    let targets = if targets.is_empty() { vec![] } else { vec![targets.iter().map(|t| Target::Object(*t)).collect()] };
    g.act(player, crate::support::paid(spending, Move::Cast { card, targets }))
}
/// All pass while the stack has something and nothing else is asked.
fn resolve(g: &mut Game) {
    while g.asked().is_some_and(|(_, a)| a == Asked::Priority) && !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
}
fn top(g: &Game) -> ObjId { *g.stack.last().expect("something on the stack") }
fn there(g: &Game, o: ObjId) -> bool { g.obj(o).zone == Zone::Battlefield }
/// How many cards of this definition are in the zone, of anyone's.
fn count(g: &Game, def: usize, zone: Zone) -> usize {
    let of = |objects: &[ObjId]| objects.iter().filter(|o| g.obj(**o).card == Some(def)).count();
    match zone {
        Zone::Battlefield => of(&g.battlefield),
        Zone::Exile => of(&g.exile),
        Zone::Graveyard => g.players.iter().map(|p| of(&p.graveyard)).sum(),
        Zone::Hand => g.players.iter().map(|p| of(&p.hand)).sum(),
        other => panic!("not counted: {other:?}"),
    }
}
/// Pass, attacking with nothing, until this player's turn has begun.
fn to_turn_of(g: &mut Game, player: usize) {
    for _ in 0..200 {
        if g.active == player { return; }
        match g.asked().expect("the game goes on") {
            (p, Asked::Priority) => g.act(p, Move::Pass).unwrap(),
            (p, Asked::Attackers(_)) => g.act(p, Move::Attack(vec![])).unwrap(),
            (_, asked) => panic!("unexpected decision while advancing this fixture: {asked:?}"),
        }
    }
    panic!("no turn of player {player}");
}

#[test]
fn firedrinker_satyr_pumps_itself_and_burns_its_controller() {
    let mut g = game();
    let satyr = g.put(0, SATYR, Zone::Battlefield);
    pool(&mut g, 0, [0, 0, 0, 1, 0]);
    cite!(["118.3a"]; refused("that mana is not in your mana pool (118.3a)"): g.act(0, crate::support::paid("RR", Move::Activate { source: satyr, ability: 1, targets: vec![], paying: vec![] })));
    pool(&mut g, 0, [0, 0, 0, 4, 2]);
    let mut other = g.clone();
    other.priority = 1;
    cite!(["602.2"]; refused("only an object's controller can activate its ability (602.2)"): other.act(1, crate::support::paid("RR", Move::Activate { source: satyr, ability: 1, targets: vec![], paying: vec![] })));
    for _ in 0..2 { g.act(0, crate::support::paid("RR", Move::Activate { source: satyr, ability: 1, targets: vec![], paying: vec![] })).unwrap(); }
    assert_eq!((g.stack.len(), g.power(satyr), g.players[0].life), (2, Some(2), 20), "nothing before it resolves");
    assert_eq!(g.players[0].pool.total(), 2);
    resolve(&mut g);
    assert_eq!((g.power(satyr), g.toughness(satyr)), (Some(4), Some(1)));
    // damage to its controller is not damage to it: the other ability does not trigger
    assert_eq!((g.players[0].life, g.players[1].life, g.obj(satyr).damage), (18, 20, 0));
    // until end of turn
    to_turn_of(&mut g, 1);
    cite!(["514.2"]; assert_eq!((g.power(satyr), g.players[0].life), (Some(2), 18)));
}

/// Destroyed in response, there is nothing to pump, and the ability still deals its damage:
/// the source is found as it last was (608.2h, 113.7a).
#[test]
fn firedrinker_satyr_destroyed_in_response_still_burns_its_controller() {
    let mut g = game();
    let satyr = g.put(0, SATYR, Zone::Battlefield);
    pool(&mut g, 0, [0, 0, 0, 2, 0]);
    g.act(0, crate::support::paid("RR", Move::Activate { source: satyr, ability: 1, targets: vec![], paying: vec![] })).unwrap();
    let decay = g.put(1, DECAY, Zone::Hand);
    pool(&mut g, 1, [0, 0, 1, 0, 1]);
    g.act(0, Move::Pass).unwrap();
    cast(&mut g, 1, decay, &[satyr], "BG").unwrap();
    g.act(1, Move::Pass).unwrap();
    g.act(0, Move::Pass).unwrap();
    assert_eq!((g.obj(satyr).zone, g.stack.len(), g.players[0].life), (Zone::Gone, 1, 20));
    resolve(&mut g);
    cite!(["113.7a", "608.2h"]; assert_eq!(g.players[0].life, 19));
}

#[test]
fn manabarbs_burns_whoever_taps_a_land_for_mana() {
    let mut g = game();
    g.put(1, MANABARBS, Zone::Battlefield);
    let (forest, grove, vault, elf) =
        (g.put(0, FOREST, Zone::Battlefield), g.put(0, NONBASIC, Zone::Battlefield), g.put(0, ARTIFACT_LAND, Zone::Battlefield), g.put(0, ELF, Zone::Battlefield));
    let theirs = g.put(1, FOREST, Zone::Battlefield);
    // a basic land: the mana is there at once, the damage when the ability resolves
    g.act(0, Move::Activate { source: forest, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    cite!(["106.12a"]; assert_eq!((g.players[0].pool.total(), g.stack.len(), g.players[0].life), (1, 1, 20)));
    resolve(&mut g);
    assert_eq!((g.players[0].life, g.players[1].life), (19, 20), "to the player who tapped it, not to Manabarbs's controller");
    // any land, basic or not; a creature tapped for mana is no land
    g.act(0, Move::Activate { source: grove, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    g.act(0, Move::Activate { source: vault, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    assert_eq!(g.stack.len(), 2);
    resolve(&mut g);
    assert_eq!(g.players[0].life, 17);
    g.act(0, Move::Activate { source: elf, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    assert!(g.stack.is_empty());
    assert_eq!(g.players[0].life, 17);
    // its controller is a player too; a second Manabarbs is a second ability
    g.put(0, MANABARBS, Zone::Battlefield);
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Activate { source: theirs, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    assert_eq!(g.stack.len(), 2);
    resolve(&mut g);
    assert_eq!((g.players[0].life, g.players[1].life), (17, 18));
}

#[test]
fn abrupt_decay_destroys_a_cheap_nonland_permanent_and_cannot_be_countered() {
    let mut g = game();
    let decay = g.put(0, DECAY, Zone::Hand);
    let (bear, giant, glow, great, rock, land) = (
        g.put(1, BEAR, Zone::Battlefield),
        g.put(1, GIANT, Zone::Battlefield),
        g.put(1, AURA, Zone::Battlefield),
        g.put(1, BIG_AURA, Zone::Battlefield),
        g.put(1, ROCK, Zone::Battlefield),
        g.put(1, ARTIFACT_LAND, Zone::Battlefield),
    );
    let ingot = g.put(1, INGOT, Zone::Battlefield);
    pool(&mut g, 0, [0, 0, 1, 0, 1]);
    // mana value 4 and a land (mana value 0) are no targets; nothing was paid for them
    for (wrong, why) in [(giant, "an illegal target, Giant: its mana value is 4, and the target says mana value 3 or less (115.1)"),
        (great, "an illegal target, Great Glow: its mana value is 4, and the target says mana value 3 or less (115.1)"),
        (land, "an illegal target, Vault: it is a land (115.1)")] {
        cite!(["115.1"]; case: why; refused(why): cast(&mut g, 0, decay, &[wrong], "BG"));
    }
    cite!(["115.1"]; refused("an illegal target, that player: a player isn't a permanent (115.1)"): g.act(0, Move::Cast { card: decay, targets: vec![vec![Target::Player(1)]] }));
    cite!(["601.2c"]; refused("wrong number of targets: 2 chosen, it takes 1 (601.2c)"): cast(&mut g, 0, decay, &[bear, glow], "BG"));
    assert_eq!((g.players[0].pool.total(), g.obj(decay).zone), (2, Zone::Hand));
    // creature, enchantment, artifact of mana value 3 or less, each in a copy of the game
    for target in [bear, glow, rock] {
        let mut copy = g.clone();
        cast(&mut copy, 0, decay, &[target], "BG").unwrap();
        resolve(&mut copy);
        cite!(["701.8a"]; assert!(!there(&copy, target), "{}", g.obj(target).chars.name));
        assert_eq!(copy.battlefield.len(), g.battlefield.len() - 1);
        assert_eq!(count(&copy, DECAY, Zone::Graveyard), 1);
    }
    // an indestructible one is a target and stays (701.8b)
    let mut copy = g.clone();
    cast(&mut copy, 0, decay, &[ingot], "BG").unwrap();
    resolve(&mut copy);
    cite!(["702.12b"]; assert!(there(&copy, ingot)));
    // Counterspell may target it, resolves, and counters nothing (101.2)
    cast(&mut g, 0, decay, &[bear], "BG").unwrap();
    let spell = top(&g);
    g.act(0, Move::Pass).unwrap();
    let counterspell = g.put(1, COUNTERSPELL, Zone::Hand);
    pool(&mut g, 1, [0, 2, 0, 0, 0]);
    cast(&mut g, 1, counterspell, &[spell], "UU").unwrap();
    g.act(1, Move::Pass).unwrap();
    g.act(0, Move::Pass).unwrap();
    cite!(["101.2"]; assert_eq!(g.stack, vec![spell], "Counterspell resolved; Abrupt Decay is still a spell"));
    assert_eq!(count(&g, COUNTERSPELL, Zone::Graveyard), 1);
    assert!(there(&g, bear));
    resolve(&mut g);
    assert!(!there(&g, bear));
    assert_eq!((count(&g, BEAR, Zone::Graveyard), count(&g, DECAY, Zone::Graveyard)), (1, 1));
}

#[test]
fn gainsay_counters_a_blue_spell_and_only_a_blue_one() {
    let mut g = game();
    let (divination, bolt, blue_bear) = (g.put(0, DIVINATION, Zone::Hand), g.put(0, BOLT, Zone::Hand), g.put(0, BLUE_BEAR, Zone::Hand));
    let on_field = g.put(0, BLUE_BEAR, Zone::Battlefield);
    let gainsay = [g.put(1, GAINSAY, Zone::Hand), g.put(1, GAINSAY, Zone::Hand)];
    pool(&mut g, 0, [0, 5, 0, 1, 0]);
    pool(&mut g, 1, [0, 4, 0, 0, 0]);
    // a red spell is no target, nor is a blue permanent, nor Gainsay with nothing to counter
    g.act(0, crate::support::paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Player(1)]] })).unwrap();
    let red = top(&g);
    g.act(0, Move::Pass).unwrap();
    cite!(["115.1"]; refused("an illegal target, Lightning Bolt: it isn't blue (115.1)"): cast(&mut g, 1, gainsay[0], &[red], "UU"));
    cite!(["115.1"]; refused("an illegal target, Blue Bear: it isn't a spell (115.1)"): cast(&mut g, 1, gainsay[0], &[on_field], "UU"));
    cite!(["601.2c"]; refused("that has 1 target group(s), 0 given"): cast(&mut g, 1, gainsay[0], &[], "UU"));
    assert_eq!(g.players[1].pool.total(), 4);
    resolve(&mut g);
    assert_eq!(g.players[1].life, 17);
    // a blue sorcery: countered, in its owner's graveyard, and it did nothing (701.6a)
    cast(&mut g, 0, divination, &[], "UUU").unwrap();
    let blue = top(&g);
    g.act(0, Move::Pass).unwrap();
    // a copy of the game lets it resolve
    let mut let_through = g.clone();
    cast(&mut g, 1, gainsay[0], &[blue], "UU").unwrap();
    resolve(&mut g);
    resolve(&mut let_through);
    cite!(["701.6a"]; assert_eq!((g.players[0].hand.len(), let_through.players[0].hand.len()), (1, 3)));
    assert_eq!((count(&g, DIVINATION, Zone::Graveyard), count(&g, GAINSAY, Zone::Graveyard)), (1, 1));
    assert_eq!(g.players[0].library.len(), 12);
    // a blue creature spell never becomes a permanent
    cast(&mut g, 0, blue_bear, &[], "UU").unwrap();
    let creature = top(&g);
    g.act(0, Move::Pass).unwrap();
    cast(&mut g, 1, gainsay[1], &[creature], "UU").unwrap();
    resolve(&mut g);
    cite!(["701.6a"]; assert_eq!((count(&g, BLUE_BEAR, Zone::Battlefield), count(&g, BLUE_BEAR, Zone::Graveyard)), (1, 1)));
    assert!(g.stack.is_empty());
}

#[test]
fn ray_of_revelation_and_ancient_grudge_destroy_from_hand_and_again_by_flashback() {
    // (the card, its mana from hand, what it destroys, what it cannot target, the type it needs, that target's name)
    for (spell, hand_mana, right, wrong, needs, wrong_name) in [(RAY, [1, 0, 0, 1, 0], AURA, ROCK, "an enchantment", "Rock"), (GRUDGE, [0, 0, 0, 2, 0], ROCK, AURA, "an artifact", "Glow")] {
        let not = |name: &str| format!("an illegal target, {name}: it isn't {needs} (115.1)");
        let mut g = game();
        let card = g.put(0, spell, Zone::Hand);
        let targets = [g.put(1, right, Zone::Battlefield), g.put(1, right, Zone::Battlefield)];
        let other = g.put(1, wrong, Zone::Battlefield);
        let bear = g.put(1, BEAR, Zone::Battlefield);
        pool(&mut g, 0, hand_mana);
        cite!(["115.1"]; case: wrong_name; refused(&not(wrong_name)): cast(&mut g, 0, card, &[other], if spell == RAY { "WR" } else { "RR" }));
        cite!(["115.1"]; case: "Bear"; refused(&not("Bear")): cast(&mut g, 0, card, &[bear], if spell == RAY { "WR" } else { "RR" }));
        cast(&mut g, 0, card, &[targets[0]], if spell == RAY { "WR" } else { "RR" }).unwrap();
        resolve(&mut g);
        assert!(!there(&g, targets[0]) && there(&g, targets[1]));
        assert_eq!(count(&g, spell, Zone::Graveyard), 1);
        let card = g.players[0].graveyard[0];
        // from the graveyard for {G} only: not for its mana cost, not for nothing
        cite!(["702.34a", "118.3a"]; refused("that mana is not in your mana pool (118.3a)"): (g).act(0, crate::support::paid("G", Move::CastVia(game::announcement::CastRoute::Flashback,Box::new(Move::Cast{card,targets:vec![vec![Target::Object(targets[1])]]})))));
        pool(&mut g, 0, [1, 0, 0, 2, 0]);
        cite!(["702.34a", "118.3a"]; refused("that mana is not in your mana pool (118.3a)"): (g).act(0, crate::support::paid("G", Move::CastVia(game::announcement::CastRoute::Flashback,Box::new(Move::Cast{card,targets:vec![vec![Target::Object(targets[1])]]})))));
        let mut stranger = g.clone();
        stranger.priority = 1;
        cite!(["702.34a", "601.3"]; refused("that card cannot be cast using flashback (702.34a)"): (stranger).act(1, crate::support::paid("G", Move::CastVia(game::announcement::CastRoute::Flashback,Box::new(Move::Cast{card,targets:vec![vec![Target::Object(targets[1])]]})))));
        pool(&mut g, 0, [0, 0, 0, 0, 1]);
        // the same target restriction
        cite!(["702.34a", "115.1"]; case: wrong_name; refused(&not(wrong_name)): (g).act(0, crate::support::paid("G", Move::CastVia(game::announcement::CastRoute::Flashback,Box::new(Move::Cast{card,targets:vec![vec![Target::Object(other)]]})))));
        // the target leaves in response: the spell does nothing and is exiled all the same (702.34a)
        let mut fizzled = g.clone();
        (g).act(0, crate::support::paid("G", Move::CastVia(game::announcement::CastRoute::Flashback,Box::new(Move::Cast{card,targets:vec![vec![Target::Object(targets[1])]]})))).unwrap();
        (fizzled).act(0, crate::support::paid("G", Move::CastVia(game::announcement::CastRoute::Flashback,Box::new(Move::Cast{card,targets:vec![vec![Target::Object(targets[1])]]})))).unwrap();
        fizzled.move_object(targets[1], Zone::Exile);
        resolve(&mut g);
        resolve(&mut fizzled);
        assert!(!there(&g, targets[1]) && there(&g, other) && there(&g, bear));
        for game in [&mut g, &mut fizzled] {
            cite!(["702.34a"]; assert_eq!((count(game, spell, Zone::Exile), count(game, spell, Zone::Graveyard)), (1, 0)));
            assert_eq!(game.players[0].pool.total(), 0);
            // exiled, it is cast no more
            let exiled = *game.exile.iter().find(|o| game.obj(**o).card == Some(spell)).unwrap();
            pool(game, 0, [1, 0, 0, 2, 1]);
            cite!(["702.34a", "601.3"]; refused("that card is not in your hand: it is in exile, and no rule or effect lets you cast it from there (601.3)"): cast(game, 0, exiled, &[other], if spell == RAY { "WR" } else { "RR" }));
            cite!(["702.34a", "601.3"]; refused("that card is not in your hand: it is in exile, and no rule or effect lets you cast it from there (601.3)"): cast(game, 0, exiled, &[bear], if spell == RAY { "WR" } else { "RR" }));
        }
    }
}

#[test]
fn creeping_corrosion_destroys_every_artifact_and_shrivel_shrinks_every_creature() {
    let mut g = game();
    let (corrosion, shrivel) = (g.put(0, CORROSION, Zone::Hand), g.put(0, SHRIVEL, Zone::Hand));
    let artifacts = [
        g.put(0, ROCK, Zone::Battlefield),
        g.put(1, ROCK, Zone::Battlefield),
        g.put(0, ORNITHOPTER, Zone::Battlefield),
        g.put(1, ORNITHOPTER, Zone::Battlefield),
        g.put(1, ARTIFACT_LAND, Zone::Battlefield),
    ];
    let ingot = g.put(1, INGOT, Zone::Battlefield);
    let kept = [g.put(0, BEAR, Zone::Battlefield), g.put(1, AURA, Zone::Battlefield), g.put(1, NONBASIC, Zone::Battlefield), g.put(0, ELF, Zone::Battlefield)];
    let mut shrunk = g.clone();
    pool(&mut g, 0, [0, 0, 0, 0, 4]);
    cast(&mut g, 0, corrosion, &[], "GGGG").unwrap();
    resolve(&mut g);
    // Each owner orders their simultaneous graveyard arrivals, in APNAP order (404.3, 101.4).
    g.act(0,Move::Choose(game::state::Answer::Objects(vec![artifacts[0],artifacts[2]]))).unwrap();
    g.act(1,Move::Choose(game::state::Answer::Objects(vec![artifacts[1],artifacts[3],artifacts[4]]))).unwrap();
    cite!(["701.8a"]; assert!(artifacts.iter().all(|a| !there(&g, *a)), "artifact, artifact creature and artifact land, of both players"));
    cite!(["702.12b"]; assert!(there(&g, ingot), "indestructible (702.12b)"));
    assert!(kept.iter().all(|k| there(&g, *k)));
    assert_eq!((g.players[0].graveyard.len(), g.players[1].graveyard.len()), (3, 3), "each to its owner's graveyard, Creeping Corrosion after them");
    // Shrivel: -1/-1 to every creature of both players; toughness 0 dies (704.5f), the rest
    // get it back at end of turn
    let g = &mut shrunk;
    let giant = g.put(1, GIANT, Zone::Battlefield);
    pool(g, 0, [0, 0, 2, 0, 0]);
    cast(g, 0, shrivel, &[], "BB").unwrap();
    resolve(g);
    let (bear, elf) = (kept[0], kept[3]);
    assert_eq!((g.power(bear), g.toughness(bear), g.power(giant), g.toughness(giant)), (Some(1), Some(1), Some(4), Some(2)));
    cite!(["704.5f"]; assert!(!there(g, elf)));
    assert_eq!((count(g, ELF, Zone::Graveyard), count(g, ORNITHOPTER, Zone::Battlefield)), (1, 2), "a 0/2 lives as a -1/1");
    let late = g.put(0, ELF, Zone::Battlefield);
    cite!(["611.2c"]; assert_eq!(g.toughness(late), Some(1), "only the creatures there as it resolved (611.2c)"));
    to_turn_of(g, 1);
    cite!(["514.2"]; assert_eq!((g.power(bear), g.toughness(bear), g.toughness(giant)), (Some(2), Some(2), Some(3))));
}
