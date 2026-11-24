//! A suspended card's own abilities that speak of it while it is exiled: "it" in "while it's
//! exiled" is the card the event happens to, and a condition naming this card makes it the
//! "it" its effect acts on (113.6b, 702.62a, 608.2c).
use crate::support::*;
use game::legal::Asked;
use game::state::{Answer, Decision};
use game::{Move, Zone};
use ir::Part;

/// Pass priority and answer the questions these abilities raise, until `p`'s turn reaches
/// `part` with an empty stack: simultaneous triggers are put on the stack in the order listed,
/// an offered cast is declined, no one attacks.
fn until(t: &mut T, p: usize, part: Part) {
    for _ in 0..2000 {
        if t.g.active == p && t.g.part == part && t.g.stack.is_empty() && matches!(t.g.asked(), Some((_, Asked::Priority))) { return; }
        match t.g.asked() {
            Some((q, Asked::Order(abilities))) => { t.g.act(q, Move::Order((0..abilities.len()).collect())).unwrap(); }
            Some((q, Asked::Cast { .. })) => { t.answer(q, Answer::No).unwrap(); }
            _ => match t.g.decision.clone() {
                Some(Decision::Attackers { player }) => { t.g.act(player, Move::Attack(vec![])).unwrap(); }
                Some(Decision::Blockers { player }) => { t.g.act(player, Move::Block(vec![])).unwrap(); }
                _ => { t.g.act(t.g.decision_maker(t.g.priority), Move::Pass).unwrap(); }
            },
        }
    }
    panic!("never reached player {p}'s {part:?}");
}

fn suspended(t: &mut T, name: &str, time: u32) -> game::ObjId {
    let card = t.find_in(0, Zone::Exile, name);
    t.g.put_counters(card, "time", time);
    assert!(t.g.suspended(card));
    card
}

#[test]
fn r608_2c_deep_sea_kraken_removes_a_time_counter_from_itself_when_an_opponent_casts_a_spell() {
    let mut t = Board::new(2).active(1).exile(0, &["Deep-Sea Kraken"]).hand(1, &["Grizzly Bears"]).build();
    let kraken = suspended(&mut t, "Deep-Sea Kraken", 9);
    t.mana(1, "{1}{G}").cast(1, "Grizzly Bears", vec![], mana_pool("CG")).unwrap();
    cite!(["603.4", "702.62a"]; assert_eq!(t.g.stack.len(), 2));
    t.resolve();
    // "it" is this card the condition names, not the spell the opponent cast
    cite!(["608.2c", "122.1"]; assert_eq!(t.g.counters(kraken).get("time"), Some(&8)));
    let bears = t.find_in(1, Zone::Battlefield, "Grizzly Bears");
    cite!(["608.2c"]; assert!(t.g.counters(bears).is_empty()));
}

#[test]
fn r113_6b_dinosaurs_on_a_spaceship_makes_a_dinosaur_as_a_time_counter_leaves_it_in_exile() {
    let mut t = Board::new(2).active(1).at(Part::End).fill(3).exile(0, &["Dinosaurs on a Spaceship"]).build();
    let card = suspended(&mut t, "Dinosaurs on a Spaceship", 4);
    until(&mut t, 0, Part::Draw);
    cite!(["702.62a", "122.1"]; assert_eq!(t.g.counters(card).get("time"), Some(&3)));
    // the ability functions in exile, where "it" is the card the counter was removed from
    let tokens: Vec<_> = t.g.battlefield.iter().copied().filter(|o| t.g.obj(*o).token()).collect();
    cite!(["113.6b", "603.2"]; assert_eq!(tokens.len(), 1));
    let token = tokens[0];
    cite!(["111.1"]; assert!(t.g.has(token, "Flying") && t.g.has(token, "Haste")));
    cite!(["113.6b"]; assert_eq!(t.g.controller(token), 0));
}

#[test]
fn r113_6b_veiling_oddity_makes_creatures_unblockable_as_its_last_time_counter_leaves_it_in_exile() {
    let mut t = Board::new(2).active(1).at(Part::End).fill(3).exile(0, &["Veiling Oddity"])
        .battlefield(0, &["Grizzly Bears"]).battlefield(1, &["Wall of Stone"]).build();
    let card = suspended(&mut t, "Veiling Oddity", 1);
    until(&mut t, 0, Part::Draw);
    cite!(["702.62a"]; assert!(t.g.counters(card).is_empty()));
    until(&mut t, 0, Part::PrecombatMain);
    let (bears, wall) = (t.find_in(0, Zone::Battlefield, "Grizzly Bears"), t.find_in(1, Zone::Battlefield, "Wall of Stone"));
    // the defending player is asked even with no legal block, so that the Wall's is refused
    t.g.set_ask_forced(true);
    let decision = |t: &mut T, attackers: bool| for _ in 0..8 {
        if matches!(t.g.decision, Some(Decision::Attackers { .. })) == attackers && t.g.decision.is_some() { return; }
        t.g.act(t.g.decision_maker(t.g.priority), Move::Pass).unwrap();
    };
    decision(&mut t, true);
    t.g.act(0, Move::Attack(vec![(bears, game::Target::Player(1))])).unwrap();
    decision(&mut t, false);
    cite!(["509.1"]; assert!(matches!(t.g.decision, Some(Decision::Blockers { player: 1 })), "{:?}", t.g.decision));
    cite!(["113.6b", "509.1b"]; refused("Wall of Stone can't block Grizzly Bears"): t.g.act(1, Move::Block(vec![(wall, bears)])));
}
