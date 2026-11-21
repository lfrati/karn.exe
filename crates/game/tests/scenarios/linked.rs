//! Linked abilities (CR 607): one ability of an object exiles cards, another speaks of "the
//! exiled cards". Every card is read from its printed text.

use crate::support::{cite, paid};
use game::card::CardDef;
use game::legal::Asked;
use game::play::check;
use game::state::Answer;
use game::{Game, Move, ObjId, Target, Zone};

const PLAINS: usize = 0;
const SWAMP: usize = 1;
const FOREST: usize = 2;
const BEARS: usize = 3;
const ANGEL: usize = 4;
const RING: usize = 5;
const MURDER: usize = 6;
const NATURALIZE: usize = 7;
const CREMATE: usize = 8;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Plains", "", "Basic Land — Plains", "({T}: Add {W}.)", None),
        card("Swamp", "", "Basic Land — Swamp", "({T}: Add {B}.)", None),
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card(
            "Angel of Serenity",
            "{4}{W}{W}{W}",
            "Creature — Angel",
            "Flying\nWhen this creature enters, you may exile up to three other target creatures from the battlefield and/or creature cards from graveyards.\nWhen this creature leaves the battlefield, return the exiled cards to their owners' hands.",
            Some((5, 6)),
        ),
        card(
            "Oblivion Ring",
            "{2}{W}",
            "Enchantment",
            "When this enchantment enters, exile another target nonland permanent.\nWhen this enchantment leaves the battlefield, return the exiled card to the battlefield under its owner's control.",
            None,
        ),
        card("Murder", "{1}{B}{B}", "Instant", "Destroy target creature.", None),
        card("Naturalize", "{1}{G}", "Instant", "Destroy target artifact or enchantment.", None),
        card("Cremate", "{B}", "Instant", "Exile target card from a graveyard.\nDraw a card.", None),
    ]
}

fn mana(g: &mut Game, player: usize, lands: &[usize]) {
    for land in lands {
        let land = g.put(player, *land, Zone::Battlefield);
        g.act(player, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
}

fn pass_both(g: &mut Game) {
    let first = g.asked().expect("someone has priority").0;
    g.act(first, Move::Pass).unwrap();
    g.act(1 - first, Move::Pass).unwrap();
}

/// This player casts this card from their hand with the mana of these lands.
fn cast(g: &mut Game, player: usize, def: usize, lands: &[usize], targets: Vec<Vec<Target>>, spending: &str) -> ObjId {
    mana(g, player, lands);
    let card = g.put(player, def, Zone::Hand);
    g.act(player, paid(spending, Move::Cast { card, targets })).unwrap();
    card
}

/// The permanent with this name.
fn named(g: &Game, name: &str) -> ObjId {
    *g.battlefield.iter().find(|o| g.obj(**o).chars.name == name).unwrap_or_else(|| panic!("no {name}"))
}

fn names(g: &Game, cards: &[ObjId]) -> Vec<String> {
    let mut names: Vec<String> = cards.iter().map(|c| g.obj(*c).chars.name.clone()).collect();
    names.sort();
    names
}

/// What the triggered ability waiting for its targets could target.
fn among(g: &mut Game) -> Vec<Target> {
    match g.asked() {
        Some((0, Asked::Targets { slots, .. })) => slots[0].among.clone(),
        other => panic!("no targets asked: {other:?}"),
    }
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r115_2_one_target_word_is_said_of_permanents_and_of_cards_in_graveyards_where_the_text_says_both() {
    let mut g = Game::new(defs(), 2, 20);
    let mine = g.put(0, BEARS, Zone::Battlefield);
    let theirs = g.put(1, BEARS, Zone::Battlefield);
    g.put(1, FOREST, Zone::Battlefield);
    let buried = g.put(0, BEARS, Zone::Graveyard);
    let their_buried = g.put(1, BEARS, Zone::Graveyard);
    g.put(1, MURDER, Zone::Graveyard);
    cast(&mut g, 0, ANGEL, &[PLAINS; 7], vec![], "WWWWWWW");
    pass_both(&mut g);
    // "up to three other target creatures from the battlefield and/or creature cards from
    // graveyards": not the Angel, not a land, not a card that is no creature card
    let legal = among(&mut g);
    let expected = [mine, theirs, buried, their_buried].map(Target::Object);
    assert_eq!(legal.len(), 4);
    cite!(["115.2"]; assert!(expected.iter().all(|t| legal.contains(t)), "{legal:?}"));
    let four = expected.to_vec();
    cite!(["601.2c"]; refused("wrong number of targets: 4 chosen, it takes 0 to 3 (601.2c)"): g.act(0, Move::ChooseTargets(vec![four])));
    cite!(["601.2c"]; ok: g.act(0, Move::ChooseTargets(vec![vec![Target::Object(theirs), Target::Object(buried)]])));
}

#[test]
fn r607_2a_the_exiled_cards_are_those_the_linked_ability_exiled_and_that_are_still_in_exile() {
    let mut g = Game::new(defs(), 2, 20);
    g.put(1, SWAMP, Zone::Library);
    let mine = g.put(0, BEARS, Zone::Battlefield);
    let theirs = g.put(1, BEARS, Zone::Battlefield);
    let buried = g.put(0, BEARS, Zone::Graveyard);
    let other = g.put(0, FOREST, Zone::Graveyard);
    cast(&mut g, 0, ANGEL, &[PLAINS; 7], vec![], "WWWWWWW");
    pass_both(&mut g);
    g.act(0, Move::ChooseTargets(vec![[mine, theirs, buried].map(Target::Object).to_vec()])).unwrap();
    pass_both(&mut g);
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    assert_eq!(names(&g, &g.exile), ["Grizzly Bears", "Grizzly Bears", "Grizzly Bears"]);
    // a card another effect exiled is not one of them
    g.act(0, Move::Pass).unwrap();
    cast(&mut g, 1, CREMATE, &[SWAMP], vec![vec![Target::Object(other)]], "B");
    g.act(1, Move::Pass).unwrap();
    g.act(0, Move::Pass).unwrap();
    assert_eq!(g.exile.len(), 4);
    // the Angel leaves the battlefield: "return the exiled cards to their owners' hands"
    let angel = named(&g, "Angel of Serenity");
    g.act(0, Move::Pass).unwrap();
    cast(&mut g, 1, MURDER, &[SWAMP, SWAMP, SWAMP], vec![vec![Target::Object(angel)]], "BBB");
    g.act(1, Move::Pass).unwrap();
    g.act(0, Move::Pass).unwrap();
    pass_both(&mut g);
    cite!(["607.2a"]; assert_eq!(names(&g, &g.players[0].hand), ["Grizzly Bears", "Grizzly Bears"]));
    assert_eq!(names(&g, &g.players[1].hand), ["Grizzly Bears", "Swamp"], "its owner's hand, with the card Cremate drew");
    cite!(["607.2a"]; assert_eq!(names(&g, &g.exile), ["Forest"]));
}

#[test]
fn r607_2a_an_object_that_left_before_its_ability_exiled_returns_nothing() {
    let mut g = Game::new(defs(), 2, 20);
    let theirs = g.put(1, BEARS, Zone::Battlefield);
    cast(&mut g, 0, RING, &[PLAINS; 3], vec![], "WWW");
    pass_both(&mut g);
    assert_eq!(among(&mut g), [Target::Object(theirs)], "\"another target nonland permanent\"");
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(theirs)]])).unwrap();
    // in response the Ring is destroyed: its second ability resolves first, with no card
    // exiled yet, and the first then exiles for good
    let ring = named(&g, "Oblivion Ring");
    g.act(0, Move::Pass).unwrap();
    cast(&mut g, 1, NATURALIZE, &[FOREST, FOREST], vec![vec![Target::Object(ring)]], "GG");
    g.act(1, Move::Pass).unwrap();
    g.act(0, Move::Pass).unwrap();
    pass_both(&mut g);
    assert!(g.exile.is_empty());
    cite!(["607.2a"]; assert_eq!(g.obj(theirs).zone, Zone::Battlefield));
    pass_both(&mut g);
    assert!(g.stack.is_empty());
    cite!(["607.2a"]; assert_eq!(names(&g, &g.exile), ["Grizzly Bears"]));
}

#[test]
fn r607_2a_the_exiled_card_returns_to_the_battlefield_under_its_owners_control() {
    let mut g = Game::new(defs(), 2, 20);
    let theirs = g.put(1, BEARS, Zone::Battlefield);
    cast(&mut g, 0, RING, &[PLAINS; 3], vec![], "WWW");
    pass_both(&mut g);
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(theirs)]])).unwrap();
    pass_both(&mut g);
    assert_eq!(names(&g, &g.exile), ["Grizzly Bears"]);
    let ring = named(&g, "Oblivion Ring");
    g.act(0, Move::Pass).unwrap();
    cast(&mut g, 1, NATURALIZE, &[FOREST, FOREST], vec![vec![Target::Object(ring)]], "GG");
    g.act(1, Move::Pass).unwrap();
    g.act(0, Move::Pass).unwrap();
    pass_both(&mut g);
    cite!(["607.2a"]; assert!(g.exile.is_empty()));
    let bears = named(&g, "Grizzly Bears");
    cite!(["400.7"]; assert_ne!(bears, theirs, "a new object (400.7)"));
    assert_eq!((g.obj(bears).controller, g.obj(bears).owner), (1, 1));
}

#[test]
fn r608_2c_the_exiled_card_said_by_the_instruction_that_exiles_it_is_the_one_it_exiled() {
    // of what this resolution exiled, not a card earlier ones did (607.1 links abilities only)
    let bloom = CardDef::new(
        "Morbid Bloom",
        "{0}",
        "Sorcery",
        "Exile target creature card from a graveyard, then create X 1/1 green Saproling creature tokens, where X is the exiled card's toughness.",
        None,
    )
    .unwrap();
    check(&bloom).unwrap();
    let giant = CardDef::new("Wall", "{1}", "Creature — Wall", "", Some((0, 4))).unwrap();
    let mut g = Game::new(vec![bloom, giant], 2, 20);
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    let earlier = g.put(0, 1, Zone::Graveyard);
    let cast = |g: &mut Game, target: ObjId| {
        let card = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(target)]] }).unwrap();
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    };
    cast(&mut g, earlier);
    let tokens = |g: &Game| g.battlefield.iter().filter(|o| g.obj(**o).chars.name.starts_with("Saproling")).count();
    assert_eq!((g.obj(earlier).zone, g.stack.len()), (Zone::Gone, 0));
    cite!(["608.2c"]; assert_eq!(tokens(&g), 4, "the exiled card's toughness"));
    let wall = g.put(1, 1, Zone::Graveyard);
    cast(&mut g, wall);
    cite!(["608.2c"]; assert_eq!(tokens(&g), 8, "of this resolution's card, not the earlier one as well"));
}
