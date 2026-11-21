//! Flashback (CR 702.34): a card cast from its owner's graveyard for its flashback cost, and
//! exiled when it leaves the stack. Every card is read from its printed text.

use crate::support::{cite,paid};
use game::card::CardDef;
use game::play::check;
use game::{Game, Move, Zone};

const PLAINS: usize = 0;
const SWAMP: usize = 1;
const SOULS: usize = 2;
const HAUNTING: usize = 3;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Plains", "", "Basic Land — Plains", "({T}: Add {W}.)", None),
        card("Swamp", "", "Basic Land — Swamp", "({T}: Add {B}.)", None),
        card(
            "Lingering Souls",
            "{2}{W}",
            "Sorcery",
            "Create two 1/1 white Spirit creature tokens with flying.\nFlashback {1}{B} (You may cast this card from your graveyard for its flashback cost. Then exile it.)",
            None,
        ),
        card("Midnight Haunting", "{2}{W}", "Instant", "Create two 1/1 white Spirit creature tokens with flying.", None),
    ]
}

fn mana(g: &mut Game, player: usize, lands: &[usize]) {
    for land in lands {
        let land = g.put(player, *land, Zone::Battlefield);
        g.act(player, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
}

fn pass_both(g: &mut Game) {
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
}

fn spirits(g: &Game) -> usize {
    g.battlefield.iter().filter(|o| g.obj(**o).chars.name == "Spirit Token").count()
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r702_34a_a_card_with_flashback_is_cast_from_the_graveyard_for_its_flashback_cost_and_then_exiled() {
    let mut g = Game::new(defs(), 2, 20);
    // from the hand, for its mana cost: it goes to the graveyard as any sorcery
    let souls = g.put(0, SOULS, Zone::Hand);
    mana(&mut g, 0, &[PLAINS, PLAINS, PLAINS]);
    g.act(0, paid("WWW",Move::Cast { card: souls, targets: vec![] })).unwrap();
    pass_both(&mut g);
    assert_eq!(spirits(&g), 2);
    let [souls] = g.players[0].graveyard[..] else { panic!("not in the graveyard") };
    // from the graveyard: not for less than the flashback cost, {1}{B}
    mana(&mut g, 0, &[PLAINS]);
    let before = format!("{g:?}");
    cite!(["702.34a", "118.3"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0, paid("W",Move::CastVia(game::announcement::CastRoute::Flashback, Box::new(Move::Cast { card: souls, targets: vec![] })))));
    assert_eq!(format!("{g:?}"),before);
    mana(&mut g, 0, &[SWAMP]);
    cite!(["702.34a"]; ok: g.act(0, paid("WB",Move::CastVia(game::announcement::CastRoute::Flashback, Box::new(Move::Cast { card: souls, targets: vec![] })))));
    assert!(g.players[0].graveyard.is_empty(), "it is on the stack");
    pass_both(&mut g);
    cite!(["702.34a"]; assert_eq!(spirits(&g), 4));
    // exiled instead of being put into the graveyard, and cast no more
    assert!(g.players[0].graveyard.is_empty());
    let [exiled] = g.exile[..] else { panic!("not exiled") };
    cite!(["702.34a"]; assert_eq!(g.obj(exiled).chars.name, "Lingering Souls"));
    mana(&mut g, 0, &[SWAMP, SWAMP]);
    cite!(["601.3"]; refused("it is in exile, and no rule or effect lets you cast it from there (601.3)"): g.act(0, Move::Cast { card: exiled, targets: vec![] }));
}

#[test]
fn r702_34a_flashback_preserves_ownership_and_sorcery_timing() {
    let mut g = Game::new(defs(), 2, 20);
    let souls = g.put(0, SOULS, Zone::Graveyard);
    let theirs = g.put(1, SOULS, Zone::Graveyard);
    // a card without flashback is not cast from a graveyard
    let haunting = g.put(0, HAUNTING, Zone::Graveyard);
    g.put(0, PLAINS, Zone::Battlefield);
    g.put(0, SWAMP, Zone::Battlefield);
    g.put(0, PLAINS, Zone::Battlefield);
    mana(&mut g, 0, &[SWAMP, SWAMP, PLAINS]);
    cite!(["702.34a", "601.3"]; refused("it is in a graveyard, and no rule or effect lets you cast it from there (601.3)"): g.act(0, Move::Cast { card: theirs, targets: vec![] }));
    cite!(["601.3"]; refused("it is in a graveyard, and no rule or effect lets you cast it from there (601.3)"): g.act(0, Move::Cast { card: haunting, targets: vec![] }));
    // a sorcery still: not while another spell is on the stack (117.1a)
    let instant = g.put(0, HAUNTING, Zone::Hand);
    g.act(0, paid("BBW",Move::Cast { card: instant, targets: vec![] })).unwrap();
    cite!(["117.1a"]; refused("that is cast in a main phase of your own turn with the stack empty (117.1a)"): g.act(0, Move::CastVia(game::announcement::CastRoute::Flashback, Box::new(Move::Cast { card: souls, targets: vec![] }))));
}
