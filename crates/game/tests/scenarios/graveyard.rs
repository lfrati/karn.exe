//! Cards in graveyards as what a text names and targets (CR 115.2), and cards put onto the
//! battlefield by an effect (110.2a, 400.7j, 614.12). Every card is read from its printed text.

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
const VISIONARY: usize = 4;
const RITES: usize = 5;
const ANGEL: usize = 6;
const CREMATE: usize = 7;
const RESTORE: usize = 8;
const CRYPT: usize = 9;
const HAUNTING: usize = 10;
const ISLAND: usize = 11;
const FLOOD: usize = 12;
const DIVER: usize = 13;
const ORNITHOPTER: usize = 14;
const SHOCK: usize = 15;
const MOUNTAIN: usize = 16;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Plains", "", "Basic Land — Plains", "({T}: Add {W}.)", None),
        card("Swamp", "", "Basic Land — Swamp", "({T}: Add {B}.)", None),
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Elvish Visionary", "{1}{G}", "Creature — Elf Shaman", "When this creature enters, draw a card.", Some((1, 1))),
        card(
            "Unburial Rites",
            "{4}{B}",
            "Sorcery",
            "Return target creature card from your graveyard to the battlefield.\nFlashback {3}{W} (You may cast this card from your graveyard for its flashback cost. Then exile it.)",
            None,
        ),
        card(
            "Restoration Angel",
            "{3}{W}",
            "Creature — Angel",
            "Flash\nFlying\nWhen this creature enters, you may exile target non-Angel creature you control, then return that card to the battlefield under your control.",
            Some((3, 4)),
        ),
        card("Cremate", "{B}", "Instant", "Exile target card from a graveyard.\nDraw a card.", None),
        card("Restore", "{1}{G}", "Sorcery", "Put target land card from a graveyard onto the battlefield under your control.", None),
        card(
            "Blood Crypt",
            "",
            "Land — Swamp Mountain",
            "({T}: Add {B} or {R}.)\nAs this land enters, you may pay 2 life. If you don't, it enters tapped.",
            None,
        ),
        card("Midnight Haunting", "{2}{W}", "Instant", "Create two 1/1 white Spirit creature tokens with flying.", None),
        card("Island", "", "Basic Land — Island", "({T}: Add {U}.)", None),
        card(
            "Flood of Recollection",
            "{U}{U}",
            "Sorcery",
            "Return target instant or sorcery card from your graveyard to your hand. Exile Flood of Recollection.",
            None,
        ),
        card(
            "Junk Diver",
            "{3}",
            "Artifact Creature — Bird",
            "Flying\nWhen this creature dies, return another target artifact card from your graveyard to your hand.",
            Some((1, 1)),
        ),
        card("Ornithopter", "{0}", "Artifact Creature — Thopter", "Flying", Some((0, 2))),
        card("Shock", "{R}", "Instant", "Shock deals 2 damage to any target.", None),
        card("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None),
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

/// The permanents with this name.
fn named(g: &Game, name: &str) -> Vec<ObjId> {
    g.battlefield.iter().copied().filter(|o| g.obj(*o).chars.name == name).collect()
}

/// Player 0 casts Restoration Angel; it enters and its ability triggers. What the ability
/// could target.
fn angel_enters(g: &mut Game) -> Vec<Target> {
    cast(g, 0, ANGEL, &[PLAINS, PLAINS, PLAINS, PLAINS], vec![], "WWWW");
    pass_both(g);
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
fn r115_2_a_card_in_a_graveyard_is_targeted_by_a_text_that_says_so_and_by_what_it_describes() {
    let mut g = Game::new(defs(), 2, 20);
    let mine = g.put(0, BEARS, Zone::Graveyard);
    let land = g.put(0, FOREST, Zone::Graveyard);
    let theirs = g.put(1, BEARS, Zone::Graveyard);
    let permanent = g.put(0, BEARS, Zone::Battlefield);
    let rites = g.put(0, RITES, Zone::Hand);
    for _ in 0..5 {
        g.put(0, SWAMP, Zone::Battlefield);
    }
    mana(&mut g, 0, &[SWAMP, SWAMP, SWAMP, SWAMP, SWAMP]);
    for (illegal, why) in [(land, "an illegal target, Forest: it isn't a creature (115.1)"),
        (theirs, "an illegal target, Grizzly Bears: it is in an opponent's graveyard, not yours (115.1)"),
        (permanent, "an illegal target, Grizzly Bears: it isn't in your graveyard (115.1)")] {
        cite!(["601.2c", "115.1"]; case: why; refused(why): g.act(0, Move::Cast { card: rites, targets: vec![vec![Target::Object(illegal)]] }));
    }
    g.act(0, paid("BBBBB", Move::Cast { card: rites, targets: vec![vec![Target::Object(mine)]] })).unwrap();
    // "target card from a graveyard": any card in any graveyard, and no permanent
    let cremate = g.put(1, CREMATE, Zone::Hand);
    g.act(0, Move::Pass).unwrap();
    mana(&mut g, 1, &[SWAMP]);
    cite!(["601.2c", "115.1"]; refused("an illegal target, Grizzly Bears: it isn't in a graveyard (115.1)"): g.act(1, Move::Cast { card: cremate, targets: vec![vec![Target::Object(permanent)]] }));
    cite!(["115.2"]; ok: g.act(1, paid("B", Move::Cast { card: cremate, targets: vec![vec![Target::Object(land)]] })));
}

#[test]
fn r110_2a_a_card_returned_to_the_battlefield_enters_under_the_control_of_the_player_who_returns_it() {
    let mut g = Game::new(defs(), 2, 20);
    g.put(0, FOREST, Zone::Library);
    let visionary = g.put(0, VISIONARY, Zone::Graveyard);
    cast(&mut g, 0, RITES, &[SWAMP, SWAMP, SWAMP, SWAMP, SWAMP], vec![vec![Target::Object(visionary)]], "BBBBB");
    pass_both(&mut g);
    // a new object (400.7), a permanent its owner controls; it entered, so its ability
    // triggered (603.6a)
    let [returned] = named(&g, "Elvish Visionary")[..] else { panic!("not on the battlefield") };
    cite!(["400.7"]; assert_ne!(returned, visionary));
    cite!(["110.2a"]; assert_eq!(g.obj(returned).controller, 0));
    assert_eq!(g.players[0].graveyard.len(), 1, "the sorcery, and not the creature card");
    assert!(g.players[0].hand.is_empty());
    pass_both(&mut g);
    cite!(["603.6a"]; assert_eq!(g.players[0].hand.len(), 1, "drawn as the Visionary entered"));
    // with flashback, from the graveyard (702.34a): the same instruction, and then exiled
    let bears = g.put(0, BEARS, Zone::Graveyard);
    let [rites] = g.players[0].graveyard.iter().copied().filter(|c| g.obj(*c).chars.name == "Unburial Rites").collect::<Vec<_>>()[..] else { panic!() };
    mana(&mut g, 0, &[PLAINS, PLAINS, PLAINS, PLAINS]);
    g.act(0, paid("WWWW", Move::CastVia(game::announcement::CastRoute::Flashback, Box::new(Move::Cast { card: rites, targets: vec![vec![Target::Object(bears)]] })))).unwrap();
    pass_both(&mut g);
    assert_eq!(named(&g, "Grizzly Bears").len(), 1);
    assert!(g.players[0].graveyard.is_empty());
    cite!(["702.34a"]; assert_eq!(g.exile.len(), 1));
    // "onto the battlefield under your control": a card another player owns (108.3) is
    // controlled by the player the text says, and is still its owner's
    let forest = g.put(1, FOREST, Zone::Graveyard);
    cast(&mut g, 0, RESTORE, &[FOREST, FOREST], vec![vec![Target::Object(forest)]], "GG");
    pass_both(&mut g);
    let land = *g.battlefield.last().unwrap();
    cite!(["110.2a"]; assert_eq!((g.obj(land).controller, g.obj(land).owner), (0, 1)));
    assert!(g.players[1].graveyard.is_empty());
}

#[test]
fn r115_2_another_card_is_not_the_card_whose_ability_it_is_where_that_is_in_the_graveyard() {
    let mut g = Game::new(defs(), 2, 20);
    let thopter = g.put(0, ORNITHOPTER, Zone::Graveyard);
    g.put(0, BEARS, Zone::Graveyard);
    let diver = g.put(0, DIVER, Zone::Battlefield);
    cast(&mut g, 0, SHOCK, &[MOUNTAIN], vec![vec![Target::Object(diver)]], "R");
    pass_both(&mut g);
    // it died and is an artifact card in that graveyard: "another target artifact card"
    assert_eq!(g.players[0].graveyard.len(), 4);
    let Some((0, Asked::Targets { slots, .. })) = g.asked() else { panic!("no targets asked") };
    cite!(["115.2"]; assert_eq!(slots[0].among, [Target::Object(thopter)]));
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(thopter)]])).unwrap();
    pass_both(&mut g);
    let [returned] = g.players[0].hand[..] else { panic!("not returned") };
    assert_eq!(g.obj(returned).chars.name, "Ornithopter");
}

#[test]
fn r608_2m_a_spell_that_exiles_itself_as_it_resolves_is_not_put_into_the_graveyard() {
    let mut g = Game::new(defs(), 2, 20);
    let shock = g.put(0, SHOCK, Zone::Graveyard);
    cast(&mut g, 0, FLOOD, &[ISLAND, ISLAND], vec![vec![Target::Object(shock)]], "UU");
    pass_both(&mut g);
    let [returned] = g.players[0].hand[..] else { panic!("not returned") };
    assert_eq!(g.obj(returned).chars.name, "Shock");
    assert!(g.players[0].graveyard.is_empty());
    let [flood] = g.exile[..] else { panic!("not exiled, or more than once") };
    assert_eq!(g.obj(flood).chars.name, "Flood of Recollection");
    assert!(g.stack.is_empty());
}

#[test]
fn r608_2b_a_target_that_has_left_the_graveyard_it_was_in_is_illegal() {
    let mut g = Game::new(defs(), 2, 20);
    g.put(1, SWAMP, Zone::Library);
    let bears = g.put(0, BEARS, Zone::Graveyard);
    cast(&mut g, 0, RITES, &[SWAMP, SWAMP, SWAMP, SWAMP, SWAMP], vec![vec![Target::Object(bears)]], "BBBBB");
    g.act(0, Move::Pass).unwrap();
    // in response the card is exiled
    cast(&mut g, 1, CREMATE, &[SWAMP], vec![vec![Target::Object(bears)]], "B");
    g.act(1, Move::Pass).unwrap();
    g.act(0, Move::Pass).unwrap();
    let [exiled] = g.exile[..] else { panic!("not exiled") };
    assert_eq!(g.obj(exiled).chars.name, "Grizzly Bears");
    assert_eq!(g.players[1].hand.len(), 1);
    // the spell's only target is illegal: it doesn't resolve, and is put into its owner's
    // graveyard
    pass_both(&mut g);
    assert!(g.stack.is_empty());
    cite!(["608.2b"]; assert!(named(&g, "Grizzly Bears").is_empty()));
    assert_eq!(g.exile, [exiled]);
    let [rites] = g.players[0].graveyard[..] else { panic!("not in the graveyard") };
    cite!(["608.2b"]; assert_eq!(g.obj(rites).chars.name, "Unburial Rites"));
}

#[test]
fn r608_2b_a_spell_no_target_was_chosen_for_has_no_illegal_target_and_resolves() {
    // "Exile up to one target artifact, creature, or enchantment you control, then return it
    // to the battlefield under its owner's control.\nDraw a card.": cast with no target
    let mut defs = defs();
    defs.push(
        CardDef::new(
            "Scrollshift",
            "{2}{W}",
            "Instant",
            "Exile up to one target artifact, creature, or enchantment you control, then return it to the battlefield under its owner's control.\nDraw a card.",
            None,
        )
        .unwrap(),
    );
    check(defs.last().unwrap()).unwrap();
    let scrollshift = defs.len() - 1;
    let mut g = Game::new(defs, 2, 20);
    g.put(0, FOREST, Zone::Library);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    cast(&mut g, 0, scrollshift, &[PLAINS, PLAINS, PLAINS], vec![vec![]], "WWW");
    pass_both(&mut g);
    cite!(["608.2b", "601.2c"]; assert_eq!(g.players[0].hand.len(), 1, "it resolved: a card was drawn"));
    assert_eq!(named(&g, "Grizzly Bears"), [bears]);
}

#[test]
fn r702_34a_flashback_exiles_unburial_rites_when_its_target_becomes_illegal() {
    let mut g = Game::new(defs(), 2, 20);
    let bears = g.put(0, BEARS, Zone::Graveyard);
    let rites = g.put(0, RITES, Zone::Graveyard);
    mana(&mut g, 0, &[PLAINS; 4]);
    g.act(0, paid("WWWW", Move::CastVia(game::announcement::CastRoute::Flashback, Box::new(Move::Cast { card: rites, targets: vec![vec![Target::Object(bears)]] })))).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.put(1, SWAMP, Zone::Library);
    cast(&mut g, 1, CREMATE, &[SWAMP], vec![vec![Target::Object(bears)]], "B");
    pass_both(&mut g);
    pass_both(&mut g);
    assert!(g.stack.is_empty());
    assert!(g.players[0].graveyard.is_empty());
    assert!(named(&g, "Grizzly Bears").is_empty());
    let exiled: Vec<_> = g.exile.iter().map(|o| g.obj(*o).chars.name.as_str()).collect();
    cite!(["702.34a", "608.2b"]; assert_eq!(exiled, ["Grizzly Bears", "Unburial Rites"]));
}

#[test]
fn r400_7_reanimation_does_not_follow_a_target_that_leaves_and_returns_to_the_graveyard() {
    let mut g = Game::new(defs(), 2, 20);
    let bears = g.put(0, BEARS, Zone::Graveyard);
    cast(&mut g, 0, RITES, &[SWAMP; 5], vec![vec![Target::Object(bears)]], "BBBBB");
    // Model an intervening zone-change sequence: the same physical card returns,
    // but the spell still targets its old incarnation (400.7, 608.2b).
    g.move_object(bears, Zone::Hand);
    let returned = g.players[0].hand[0];
    g.move_object(returned, Zone::Graveyard);
    let new_bears = g.players[0].graveyard[0];
    assert_ne!(new_bears, bears);
    pass_both(&mut g);
    cite!(["400.7", "608.2b"]; assert!(named(&g, "Grizzly Bears").is_empty()));
    assert!(g.players[0].graveyard.contains(&new_bears));
    assert_eq!(g.players[0].graveyard.len(), 2);
}

#[test]
fn r400_7j_what_an_effect_moved_to_a_public_zone_the_rest_of_the_effect_finds_there() {
    let mut g = Game::new(defs(), 2, 20);
    g.put(0, FOREST, Zone::Library);
    let visionary = g.put(0, VISIONARY, Zone::Battlefield);
    let other_angel = g.put(0, ANGEL, Zone::Battlefield);
    let theirs = g.put(1, BEARS, Zone::Battlefield);
    g.put_counters(visionary, "+1/+1", 2);
    // "target non-Angel creature you control"
    let among = angel_enters(&mut g);
    assert_eq!(among, [Target::Object(visionary)]);
    assert!(!among.contains(&Target::Object(other_angel)) && !among.contains(&Target::Object(theirs)));
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(visionary)]])).unwrap();
    pass_both(&mut g);
    // "you may exile …, then return that card to the battlefield": the card it became in
    // exile is the one returned, a new object again (400.7)
    assert!(matches!(g.asked(), Some((0, Asked::May))));
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    cite!(["400.7j"]; assert!(g.exile.is_empty()));
    let [returned] = named(&g, "Elvish Visionary")[..] else { panic!("not returned") };
    assert_ne!(returned, visionary);
    cite!(["400.7", "122.2"]; assert!(g.obj(returned).counters.is_empty()));
    assert_eq!(g.power(returned), Some(1));
    // it entered: its ability triggers again (603.6a)
    pass_both(&mut g);
    cite!(["603.6a"]; assert_eq!(g.players[0].hand.len(), 1));
}

#[test]
fn r608_2d_a_player_who_may_exile_and_return_and_chooses_not_to_does_neither() {
    let mut g = Game::new(defs(), 2, 20);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    assert_eq!(angel_enters(&mut g), [Target::Object(bears)]);
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(bears)]])).unwrap();
    pass_both(&mut g);
    g.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["608.2d"]; assert_eq!(named(&g, "Grizzly Bears"), [bears]));
    assert!(g.stack.is_empty());
}

#[test]
fn r111_8_a_token_that_has_left_the_battlefield_does_not_come_back_onto_it() {
    let mut g = Game::new(defs(), 2, 20);
    cast(&mut g, 0, HAUNTING, &[PLAINS, PLAINS, PLAINS], vec![], "WWW");
    pass_both(&mut g);
    let spirits = named(&g, "Spirit Token");
    assert_eq!(spirits.len(), 2);
    assert_eq!(angel_enters(&mut g).len(), 2);
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(spirits[0])]])).unwrap();
    pass_both(&mut g);
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    cite!(["111.8"]; assert_eq!(named(&g, "Spirit Token"), [spirits[1]]));
    cite!(["111.7"]; assert!(g.exile.is_empty(), "it has ceased to exist (111.7)"));
}

#[test]
fn r614_12a_a_card_put_onto_the_battlefield_by_an_effect_enters_as_its_own_text_has_it() {
    for pays in [false, true] {
        let mut g = Game::new(defs(), 2, 20);
        let crypt = g.put(1, CRYPT, Zone::Graveyard);
        cast(&mut g, 0, RESTORE, &[FOREST, FOREST], vec![vec![Target::Object(crypt)]], "GG");
        pass_both(&mut g);
        // "As this land enters, you may pay 2 life.": asked of the player under whose
        // control it enters, before it does
        assert!(matches!(g.asked(), Some((0, Asked::May))));
        cite!(["614.12a"]; case: format!("pays={pays}"); assert!(named(&g, "Blood Crypt").is_empty()));
        g.act(0, Move::Choose(if pays { Answer::Yes } else { Answer::No })).unwrap();
        let [crypt] = named(&g, "Blood Crypt")[..] else { panic!("not on the battlefield") };
        cite!(["110.2a"]; case: format!("pays={pays}"); assert_eq!(g.obj(crypt).controller, 0));
        cite!(["614.1c"]; case: format!("pays={pays}"); assert_eq!(g.obj(crypt).tapped, !pays, "\"If you don't, it enters tapped.\""));
        assert_eq!(g.players[0].life, if pays { 18 } else { 20 });
        assert_eq!(g.players[1].life, 20);
    }
}
