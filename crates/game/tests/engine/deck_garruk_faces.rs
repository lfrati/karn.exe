//! Garruk Relentless and Garruk, the Veil-Cursed as the corpus prints both faces: the
//! abilities the other tests of him do not run. His transformation and the Veil-Cursed's
//! first ability are in `r603_8_state_triggers.rs`.
use crate::support::cite;
use game::legal::Asked;
use game::state::Answer;
use game::{CardDef, Event, Game, Move, ObjId, Zone};

const GARRUK: usize = 0;
const FOREST: usize = 1;
const BEAR: usize = 2;
const GIANT: usize = 3;
const ELF: usize = 4;
const BOLT: usize = 5;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let defs = vec![
        CardDef::from_faces(catalog.get("Garruk Relentless").expect("Garruk Relentless is in the corpus")).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Giant", "{3}{R}", "Creature — Giant", "", Some((5, 3))).unwrap(),
        CardDef::new("Elf", "{G}", "Creature — Elf Druid", "", Some((1, 1))).unwrap(),
        CardDef::new("Lightning Bolt", "{R}", "Instant", "Lightning Bolt deals 3 damage to any target.", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    let mut g = Game::new(defs, 2, 20);
    for player in 0..2 { for _ in 0..8 { g.put(player, FOREST, Zone::Library); } }
    g
}
fn loyalty(g: &Game, o: ObjId) -> u32 { g.obj(o).counters.get("loyalty").copied().unwrap_or(0) }
/// Garruk Relentless on the battlefield under player 0's control with this much loyalty.
fn relentless(g: &mut Game, counters: u32) -> ObjId {
    let garruk = g.put(0, GARRUK, Zone::Battlefield);
    let has = loyalty(g, garruk);
    g.put_counters(garruk, "loyalty", counters - has);
    assert_eq!(g.obj(garruk).chars.name, "Garruk Relentless");
    garruk
}
/// The same permanent with its other face up.
fn veil_cursed(g: &mut Game, counters: u32) -> ObjId {
    let garruk = relentless(g, counters);
    assert!(g.transform(garruk, None));
    assert_eq!(g.obj(garruk).chars.name, "Garruk, the Veil-Cursed");
    garruk
}
fn activate(g: &mut Game, source: ObjId, ability: usize) -> Result<(), String> {
    g.act(0, Move::Activate { source, ability, targets: vec![], paying: vec![] })
}
/// All pass while the stack has something and nothing else is asked.
fn resolve(g: &mut Game) {
    while g.asked().is_some_and(|(_, a)| a == Asked::Priority) && !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
}
fn choose(g: &mut Game, player: usize, objects: Vec<ObjId>) -> Result<(), String> { g.act(player, Move::Choose(Answer::Objects(objects))) }
fn wolves(g: &Game) -> Vec<ObjId> {
    g.battlefield.iter().copied().filter(|o| g.obj(*o).token() && g.obj(*o).chars.name == "Wolf Token").collect()
}
fn count(g: &Game, from: game::event::Seq, of: impl Fn(&Event) -> bool) -> usize { g.log.since(from).iter().filter(|e| of(e)).count() }
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
fn r606_3_relentless_makes_a_green_wolf_and_stays_as_he_is() {
    let mut g = game();
    let garruk = relentless(&mut g, 3);
    activate(&mut g, garruk, 2).unwrap();
    resolve(&mut g);
    let [wolf] = wolves(&g)[..] else { panic!("one Wolf: {:?}", wolves(&g)) };
    let o = g.obj(wolf);
    assert_eq!((g.power(wolf), g.toughness(wolf), o.controller), (Some(2), Some(2), 0));
    assert!(o.is(ir::CardType::Creature) && o.chars.colors == vec![ir::Color::Green] && !g.has(wolf, "Deathtouch"));
    // a cost of 0: three counters still, so nothing transforms him
    assert_eq!((loyalty(&g, garruk), g.obj(garruk).transformed, g.stack.len()), (3, false, 0));
    // one loyalty ability a turn (606.3)
    cite!(["606.3"]; refused("a loyalty ability of that permanent has already been activated this turn (606.3)"): activate(&mut g, garruk, 2));
}

#[test]
fn r608_2c_veil_cursed_sacrifices_a_creature_to_search_for_a_creature_card() {
    let mut g = game();
    let garruk = veil_cursed(&mut g, 3);
    let (bear, elf, theirs) = (g.put(0, BEAR, Zone::Battlefield), g.put(0, ELF, Zone::Battlefield), g.put(1, GIANT, Zone::Battlefield));
    let (giant, bolt) = (g.put(0, GIANT, Zone::Library), g.put(0, BOLT, Zone::Library));
    g.put(1, GIANT, Zone::Library);
    let from = g.log.end();
    activate(&mut g, garruk, 1).unwrap();
    resolve(&mut g);
    cite!(["606.4"]; assert_eq!(loyalty(&g, garruk), 2));
    // which creature: one of his controller's own
    let Some((0, Asked::Objects { among, count: 1, .. })) = g.asked() else { panic!("a sacrifice: {:?}", g.asked()) };
    cite!(["701.21a"]; assert_eq!(among, vec![bear, elf]));
    let question = g.asked();
    cite!(["608.2d"]; refused("a chosen object is not among the offered objects to sacrifice (608.2d)"): choose(&mut g, 0, vec![theirs]));
    cite!(["608.2d"]; refused("choose exactly 1 of them to sacrifice (608.2d)"): choose(&mut g, 0, vec![bear, elf]));
    cite!(["608.2d"]; refused("choose exactly 1 of them to sacrifice (608.2d)"): choose(&mut g, 0, vec![]));
    crate::support::refused(choose(&mut g, 1, vec![bear]), "that decision is another player's");
    assert_eq!(g.asked(), question);
    // a copy of the game sacrifices the other one
    let mut other = g.clone();
    choose(&mut g, 0, vec![elf]).unwrap();
    choose(&mut other, 0, vec![bear]).unwrap();
    assert!(g.battlefield.contains(&bear) && !g.battlefield.contains(&elf));
    assert!(other.battlefield.contains(&elf) && !other.battlefield.contains(&bear));
    // "if you do": the search, for a creature card of his controller's library
    let Some((0, Asked::Search { cards, among, min: 0, max: 1 })) = g.asked() else { panic!("a search: {:?}", g.asked()) };
    assert_eq!((cards.len(), among), (10, vec![giant]));
    assert_eq!(other.asked(), g.asked());
    cite!(["701.23a"]; refused("a chosen card is not one the search can find, in the searched zone and matching its description (701.23a): choose between 0 and 1 distinct matching objects"): choose(&mut g, 0, vec![bolt]));
    crate::support::refused(choose(&mut g, 1, vec![giant]), "that decision is another player's");
    // copies taken while the search waits: one finds it, one does not
    let mut none = g.clone();
    choose(&mut g, 0, vec![giant]).unwrap();
    choose(&mut none, 0, vec![]).unwrap();
    choose(&mut other, 0, vec![giant]).unwrap();
    for (game, found) in [(&mut g, true), (&mut none, false), (&mut other, true)] {
        cite!(["701.23b"]; assert_eq!(game.players[0].hand.iter().filter(|c| game.obj(**c).card == Some(GIANT)).count(), usize::from(found)));
        assert_eq!(game.players[0].library.len(), if found { 9 } else { 10 });
        assert_eq!(count(game, from, |e| matches!(e, Event::Shuffled { player: 0, .. })), 1, "shuffled either way (701.23)");
        assert_eq!(count(game, from, |e| matches!(e, Event::Revealed { player: 0, cards } if cards.len() == 1)), usize::from(found), "revealed if found");
        assert_eq!(count(game, from, |e| matches!(e, Event::Sacrificed { player: 0, .. })), 1);
        assert_eq!(game.asked().map(|(_, a)| a), Some(Asked::Priority));
        assert!(game.battlefield.contains(&theirs));
    }
    assert_eq!((g.players[0].graveyard.len(), g.players[1].library.len()), (1, 9));
}

#[test]
fn r608_2c_veil_cursed_with_nothing_to_sacrifice_does_not_search() {
    let mut g = game();
    let garruk = veil_cursed(&mut g, 1);
    g.put(1, GIANT, Zone::Battlefield);
    g.put(0, GIANT, Zone::Library);
    let (from, library) = (g.log.end(), g.players[0].library.clone());
    activate(&mut g, garruk, 1).unwrap();
    // the cost took his last counter: he is gone and the ability resolves all the same
    cite!(["704.5i"]; assert_eq!(g.obj(garruk).zone, Zone::Gone));
    resolve(&mut g);
    assert_eq!(g.asked().map(|(_, a)| a), Some(Asked::Priority));
    assert!(g.stack.is_empty() && g.players[0].hand.is_empty());
    assert_eq!(g.players[0].library, library, "neither searched nor shuffled");
    assert_eq!(count(&g, from, |e| matches!(e, Event::Shuffled { .. } | Event::Sacrificed { .. } | Event::Revealed { .. })), 0);
    cite!(["701.21a"]; assert_eq!(g.battlefield.len(), 1, "the opponent's creature is not his to sacrifice"));
}

#[test]
fn r611_2c_veil_cursed_overruns_by_the_creature_cards_in_his_controllers_graveyard() {
    let mut g = game();
    let garruk = veil_cursed(&mut g, 2);
    cite!(["606.6"]; refused("Garruk, the Veil-Cursed can't have 3 loyalty counters removed to pay this cost (606.6, 118.3, 118.10)"): activate(&mut g, garruk, 2));
    g.put_counters(garruk, "loyalty", 1);
    let (bear, theirs) = (g.put(0, BEAR, Zone::Battlefield), g.put(1, BEAR, Zone::Battlefield));
    // counted: creature cards in his controller's graveyard. Not counted: another card
    // there, a creature card in the opponent's
    g.put(0, GIANT, Zone::Graveyard);
    g.put(0, BOLT, Zone::Graveyard);
    g.put(0, FOREST, Zone::Graveyard);
    for _ in 0..3 { g.put(1, GIANT, Zone::Graveyard); }
    // with none there: trample and +0/+0, in a copy of the game
    let mut empty = g.clone();
    let cards = empty.players[0].graveyard.clone();
    for card in cards { empty.move_object(card, Zone::Exile); }
    activate(&mut empty, garruk, 2).unwrap();
    resolve(&mut empty);
    assert_eq!((empty.power(bear), empty.toughness(bear), empty.has(bear, "Trample")), (Some(2), Some(2), true));
    activate(&mut g, garruk, 2).unwrap();
    assert_eq!((loyalty(&g, garruk), g.power(bear), g.has(bear, "Trample")), (0, Some(2), false));
    // X is counted as the ability resolves (608.2h): a card that arrives in response counts
    g.put(0, ELF, Zone::Graveyard);
    resolve(&mut g);
    cite!(["608.2h"]; assert_eq!((g.power(bear), g.toughness(bear), g.has(bear, "Trample")), (Some(4), Some(4), true)));
    assert_eq!((g.power(theirs), g.toughness(theirs), g.has(theirs, "Trample")), (Some(2), Some(2), false));
    // and is that number from then on, whatever the graveyard comes to hold
    g.put(0, GIANT, Zone::Graveyard);
    assert_eq!(g.power(bear), Some(4));
    let cards = g.players[0].graveyard.clone();
    for card in cards { g.move_object(card, Zone::Exile); }
    cite!(["608.2h"]; assert_eq!((g.power(bear), g.toughness(bear)), (Some(4), Some(4))));
    // the creatures are those there as it resolved (611.2c): not one that came later
    let late = g.put(0, BEAR, Zone::Battlefield);
    cite!(["611.2c"]; assert_eq!((g.power(late), g.has(late, "Trample")), (Some(2), false)));
    // until end of turn
    to_turn_of(&mut g, 1);
    cite!(["514.2"]; assert_eq!((g.power(bear), g.toughness(bear), g.has(bear, "Trample")), (Some(2), Some(2), false)));
}
