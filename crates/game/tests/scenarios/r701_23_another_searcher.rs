//! A search the text has another player than its controller make (CR 701.23, 608.2c, 110.2a):
//! "Exile target creature. Its controller may search their library for a basic land card, put
//! that card onto the battlefield tapped, then shuffle." The player the clause is said of
//! (named as it begins, as the creature last was: 608.2h) chooses whether, looks through their
//! own library and no one else does (701.23a), chooses what is found, puts it onto the
//! battlefield under their own control (110.2a) and shuffles their library (701.24b).
use crate::support::cite;
use game::legal::Asked;
use game::state::Answer;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::mana::ManaSymbol;
use ir::Color;

const PATH: usize = 0;
const QUARTER: usize = 1;
const BEARS: usize = 2;
const STEAL: usize = 3;
const PLAINS: usize = 4;
const FOREST: usize = 5;
const ISLAND: usize = 6;
const LLANOWAR: usize = 7;

fn card(name: &str, cost: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    let d = CardDef::new(name, cost, types, text, pt).unwrap_or_else(|e| panic!("{name}: {e}"));
    game::play::check(&d).unwrap_or_else(|u| panic!("{name}: {}", u.0));
    d
}

/// Oracle text as printed (oracle-cards.jsonl.gz).
fn defs() -> Vec<CardDef> {
    vec![
        card("Path to Exile", "{W}", "Instant",
            "Exile target creature. Its controller may search their library for a basic land card, put that card onto the battlefield tapped, then shuffle.", None),
        card("Ghost Quarter", "", "Land",
            "{T}: Add {C}.\n{T}, Sacrifice this land: Destroy target land. Its controller may search their library for a basic land card, put it onto the battlefield, then shuffle.", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Steal", "{0}", "Instant", "Gain control of target creature until end of turn.", None),
        card("Plains", "", "Basic Land — Plains", "({T}: Add {W}.)", None),
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        card("Island", "", "Basic Land — Island", "({T}: Add {U}.)", None),
        card("Llanowar Elves", "{G}", "Creature — Elf Druid", "{T}: Add {G}.", Some((1, 1))),
    ]
}

fn game() -> Game { game_of(defs()) }

fn game_of(defs: Vec<CardDef>) -> Game {
    let mut g = Game::new(defs, 2, 20);
    // the caster's library holds Islands; the opponent's a Forest under an Elf
    for _ in 0..3 { g.put(0, ISLAND, Zone::Library); }
    g.put(1, FOREST, Zone::Library);
    g.put(1, LLANOWAR, Zone::Library);
    g
}

fn pass_until_asked(g: &mut Game) -> Option<(usize, Asked)> {
    for _ in 0..20 {
        match g.asked() {
            Some((_, Asked::Priority)) if !g.stack.is_empty() => { g.act(g.priority, Move::Pass).unwrap(); }
            other => return other,
        }
    }
    panic!("nothing asked")
}

fn path(g: &mut Game, creature: ObjId) {
    g.add_mana(0, ManaSymbol::Color(Color::White));
    let card = g.put(0, PATH, Zone::Hand);
    g.act(0, crate::support::paid("W", Move::Cast { card, targets: vec![vec![Target::Object(creature)]] })).unwrap();
}

fn shuffled(g: &Game) -> Vec<usize> {
    g.log.iter().filter_map(|e| match e { Event::Shuffled { player, .. } => Some(*player), _ => None }).collect()
}

fn lands(g: &Game, controller: usize, name: &str) -> Vec<ObjId> {
    g.battlefield.iter().copied().filter(|o| g.obj(*o).chars.name == name && g.controller(*o) == controller).collect()
}

#[test]
fn r701_23a_the_exiled_creatures_controller_searches_their_own_library_alone() {
    let mut g = game();
    let bears = g.put(1, BEARS, Zone::Battlefield);
    path(&mut g, bears);
    let Some((1, Asked::May)) = pass_until_asked(&mut g) else { panic!("its controller is asked: {:?}", g.asked()) };
    g.act(1, Move::Choose(Answer::Yes)).unwrap();
    let Some((1, Asked::Search { cards, among, min, max })) = g.asked() else { panic!("they search: {:?}", g.asked()) };
    cite!(["701.23a"]; assert_eq!(cards.len(), 2, "their own library, both cards looked through"));
    assert!(cards.iter().all(|c| g.obj(*c).owner == 1));
    cite!(["701.23a"]; assert_eq!(among.iter().map(|c| g.obj(*c).chars.name.as_str()).collect::<Vec<_>>(), ["Forest"]));
    cite!(["701.23b"]; assert_eq!((min, max), (0, 1), "a basic land card: they need not find it (701.23b)"));
    // looked at by the searching player alone (701.23a)
    cite!(["701.23a"]; assert_eq!(g.observations(1).len(), 1));
    assert!(g.observations(0).is_empty());
    g.act(1, Move::Choose(Answer::Objects(among.clone()))).unwrap();
    pass_until_asked(&mut g);
    let [forest] = lands(&g, 1, "Forest")[..] else { panic!("the Forest entered under its owner's control") };
    assert!(g.obj(forest).tapped, "tapped");
    cite!(["110.2a"]; assert!(lands(&g, 0, "Forest").is_empty()));
    assert_eq!(shuffled(&g), vec![1], "their library is shuffled, the caster's is not");
    assert_eq!(g.obj(bears).zone, Zone::Gone);
    assert_eq!(g.players[1].library.len(), 1);
}

#[test]
fn r608_2d_declining_does_none_of_it() {
    let mut g = game();
    let bears = g.put(1, BEARS, Zone::Battlefield);
    path(&mut g, bears);
    let Some((1, Asked::May)) = pass_until_asked(&mut g) else { panic!("asked") };
    g.act(1, Move::Choose(Answer::No)).unwrap();
    pass_until_asked(&mut g);
    cite!(["608.2d"]; assert!(shuffled(&g).is_empty(), "no search, so no shuffle"));
    cite!(["608.2d"]; assert_eq!(g.players[1].library.len(), 2));
    assert!(g.observations(1).is_empty());
    assert!(g.exile.iter().any(|o| g.obj(*o).chars.name == "Grizzly Bears"));
}

#[test]
fn r608_2h_its_controller_is_who_controlled_it_as_it_last_existed() {
    // the caster controls the opponent's creature when it is exiled: the caster is asked,
    // and searches the caster's own library
    let mut g = game();
    let bears = g.put(1, BEARS, Zone::Battlefield);
    let steal = g.put(0, STEAL, Zone::Hand);
    g.act(0, Move::Cast { card: steal, targets: vec![vec![Target::Object(bears)]] }).unwrap();
    pass_until_asked(&mut g);
    assert_eq!(g.controller(bears), 0);
    path(&mut g, bears);
    let Some((0, Asked::May)) = pass_until_asked(&mut g) else { panic!("the caster is asked: {:?}", g.asked()) };
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    let Some((0, Asked::Search { cards, among, .. })) = g.asked() else { panic!("the caster searches: {:?}", g.asked()) };
    cite!(["608.2h"]; assert!(cards.iter().all(|c| g.obj(*c).owner == 0) && cards.len() == 3));
    assert_eq!(among.len(), 3, "Islands are basic lands");
    g.act(0, Move::Choose(Answer::Objects(vec![among[0]]))).unwrap();
    pass_until_asked(&mut g);
    cite!(["110.2a"]; assert_eq!(lands(&g, 0, "Island").len(), 1));
    assert_eq!(shuffled(&g), vec![0]);
    assert!(g.observations(1).is_empty());
}

#[test]
fn r701_23b_a_search_that_finds_nothing_still_shuffles() {
    let mut g = game();
    // no basic land in the library: nothing can be found
    let forest = *g.players[1].library.iter().find(|o| g.obj(**o).chars.name == "Forest").unwrap();
    g.players[1].library.retain(|o| *o != forest);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    path(&mut g, bears);
    let Some((1, Asked::May)) = pass_until_asked(&mut g) else { panic!("asked") };
    cite!(["701.23g"]; ok: g.act(1, Move::Choose(Answer::Yes)));
    if let Some((1, Asked::Search { among, .. })) = g.asked() {
        assert!(among.is_empty());
        g.act(1, Move::Choose(Answer::Objects(vec![]))).unwrap();
    }
    pass_until_asked(&mut g);
    cite!(["701.23g", "608.2c"]; assert_eq!(shuffled(&g), vec![1]));
    assert!(lands(&g, 1, "Forest").is_empty());
}

#[test]
fn r608_2c_the_next_clause_is_the_controllers_again() {
    // Demolition Field: that land's controller, then you, each search their own library
    let mut defs = defs();
    defs.push(card("Demolition Field", "", "Land",
        "{T}: Add {C}.\n{2}, {T}, Sacrifice this land: Destroy target nonbasic land an opponent controls. That land's controller may search their library for a basic land card, put it onto the battlefield, then shuffle. You may search your library for a basic land card, put it onto the battlefield, then shuffle.", None));
    let mut g = game_of(defs);
    let field = g.put(0, 8, Zone::Battlefield);
    let target = g.put(1, QUARTER, Zone::Battlefield);
    for _ in 0..2 { g.add_mana(0, ManaSymbol::Colorless); }
    let ability = g.def(field).unwrap().abilities().iter().position(|a| format!("{a:?}").contains("Destroy")).unwrap();
    g.act(0, crate::support::paid("CC", Move::Activate { source: field, ability, targets: vec![vec![Target::Object(target)]], paying: vec![] })).unwrap();
    for who in [1, 0] {
        let Some((asked, Asked::May)) = pass_until_asked(&mut g) else { panic!("asked: {:?}", g.asked()) };
        cite!(["608.2c"]; case: format!("{who}"); assert_eq!(asked, who));
        g.act(who, Move::Choose(Answer::Yes)).unwrap();
        let Some((searcher, Asked::Search { cards, among, .. })) = g.asked() else { panic!("search: {:?}", g.asked()) };
        assert_eq!(searcher, who);
        assert!(cards.iter().all(|c| g.obj(*c).owner == who));
        g.act(who, Move::Choose(Answer::Objects(vec![among[0]]))).unwrap();
    }
    pass_until_asked(&mut g);
    cite!(["110.2a"]; assert_eq!((lands(&g, 1, "Forest").len(), lands(&g, 0, "Island").len()), (1, 1)));
    cite!(["608.2c"]; assert_eq!(shuffled(&g), vec![1, 0]));
}

#[test]
fn r608_2c_several_players_each_doing_it_is_not_one_player_doing_it() {
    // "each opponent may search their library …" is each of them in turn; not lowered as one
    let d = CardDef::new("Old-Growth Dryads", "{G}", "Creature — Dryad",
        "When this creature enters, each opponent may search their library for a basic land card, put it onto the battlefield tapped, then shuffle.", Some((3, 3))).unwrap();
    let refused = game::play::check(&d).expect_err("not one player");
    assert!(refused.0.contains("another chooser"), "{}", refused.0);
}

#[test]
fn r110_2a_ghost_quarter_has_the_destroyed_lands_controller_put_their_land_untapped() {
    for own in [false, true] {
        let mut g = game();
        let quarter = g.put(0, QUARTER, Zone::Battlefield);
        // the caster's own Plains, or the opponent's
        let target = g.put(if own { 0 } else { 1 }, PLAINS, Zone::Battlefield);
        let sacrifice = g.def(quarter).unwrap().abilities().iter().position(|a| matches!(a, ir::Ability::Activated { .. })
            && format!("{a:?}").contains("Destroy")).expect("the destroying ability");
        g.act(0, Move::Activate { source: quarter, ability: sacrifice, targets: vec![vec![Target::Object(target)]], paying: vec![] }).unwrap();
        let who = if own { 0 } else { 1 };
        let Some((asked, Asked::May)) = pass_until_asked(&mut g) else { panic!("asked: {:?}", g.asked()) };
        cite!(["608.2h"]; case: format!("own {own}"); assert_eq!(asked, who));
        g.act(who, Move::Choose(Answer::Yes)).unwrap();
        let Some((searcher, Asked::Search { among, .. })) = g.asked() else { panic!("search: {:?}", g.asked()) };
        assert_eq!(searcher, who);
        g.act(who, Move::Choose(Answer::Objects(vec![among[0]]))).unwrap();
        pass_until_asked(&mut g);
        let name = if own { "Island" } else { "Forest" };
        let [land] = lands(&g, who, name)[..] else { panic!("{name} entered under {who}'s control") };
        cite!(["110.5b"]; case: format!("own {own}"); assert!(!g.obj(land).tapped, "untapped: Ghost Quarter does not say tapped"));
        cite!(["608.2h"]; case: format!("own {own}"); assert_eq!(shuffled(&g), vec![who]));
        assert_eq!(g.obj(target).zone, Zone::Gone);
        assert_eq!(g.obj(quarter).zone, Zone::Gone, "sacrificed");
    }
}
