//! "Another" spell, said by a permanent of the spell it was (CR 400.7, 400.7d): "An ability of
//! a permanent can reference information about the spell that became that permanent as it
//! resolved." Hotheaded Giant: "This creature enters with two -1/-1 counters on it unless
//! you've cast another red spell this turn." Its own cast is not another red spell; an
//! earlier cast of the same card, a different object, is (rulings of 2008-08-01: the check is
//! made as it enters, and it need not have been cast at all).
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::mana::ManaSymbol;
use ir::Color;

const GIANT: usize = 0;
const SHOCK: usize = 1;
const UNSUMMON: usize = 2;
const ZOMBIFY: usize = 3;
const ISLAND: usize = 4;

fn card(name: &str, cost: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    let d = CardDef::new(name, cost, types, text, pt).unwrap_or_else(|e| panic!("{name}: {e}"));
    game::play::check(&d).unwrap_or_else(|u| panic!("{name}: {}", u.0));
    d
}

fn game() -> Game {
    let defs = vec![
        card("Hotheaded Giant", "{3}{R}", "Creature — Giant Warrior",
            "Haste\nThis creature enters with two -1/-1 counters on it unless you've cast another red spell this turn.", Some((4, 4))),
        card("Shock", "{R}", "Instant", "Shock deals 2 damage to any target.", None),
        card("Unsummon", "{U}", "Instant", "Return target creature to its owner's hand.", None),
        card("Zombify", "{3}{B}", "Sorcery", "Return target creature card from your graveyard to the battlefield.", None),
        card("Island", "", "Basic Land — Island", "({T}: Add {U}.)", None),
    ];
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 {
        for _ in 0..3 { g.put(p, ISLAND, Zone::Library); }
    }
    g
}

fn resolve(g: &mut Game) {
    for _ in 0..40 {
        if g.stack.is_empty() { return; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("the stack did not empty");
}

fn cast_card(g: &mut Game, card: ObjId, targets: Vec<Vec<Target>>, paying: &[ManaSymbol], spending: &str) {
    for s in paying { g.add_mana(0, *s); }
    g.act(0, paid(spending, Move::Cast { card, targets })).unwrap_or_else(|e| panic!("cast {}: {e}", g.obj(card).chars.name));
    resolve(g);
}

const RED: ManaSymbol = ManaSymbol::Color(Color::Red);
const GIANT_COST: [ManaSymbol; 4] = [RED, ManaSymbol::Colorless, ManaSymbol::Colorless, ManaSymbol::Colorless];

/// Cast a Giant from hand: the permanent it became.
fn giant(g: &mut Game) -> ObjId {
    let card = g.put(0, GIANT, Zone::Hand);
    cast_card(g, card, vec![], &GIANT_COST, "CCCR");
    *g.battlefield.last().unwrap()
}

fn state(g: &Game, o: ObjId) -> (Option<(i32, i32)>, Option<u32>) {
    (g.seen(o, None).pt, g.obj(o).counters.get("-1/-1").copied())
}

#[test]
fn r400_7d_the_spell_a_permanent_was_is_not_another_spell() {
    let mut g = game();
    let first = giant(&mut g);
    assert_eq!(g.obj(first).chars.name, "Hotheaded Giant");
    cite!(["400.7d", "614.1c"]; assert_eq!(state(&g, first), (Some((2, 2)), Some(2)), "no red spell but itself was cast this turn"));
}

#[test]
fn r400_7d_the_second_giant_counts_the_first_and_the_first_is_unchanged() {
    let mut g = game();
    let first = giant(&mut g);
    let second = giant(&mut g);
    cite!(["400.7d"]; assert_eq!(state(&g, second), (Some((4, 4)), None), "the first Giant was another red spell"));
    cite!(["614.1c"]; assert_eq!(state(&g, first), (Some((2, 2)), Some(2))));
}

#[test]
fn r400_7_the_same_card_cast_again_counts_its_earlier_cast_as_another_spell() {
    let mut g = game();
    let card = g.put(0, GIANT, Zone::Hand);
    cast_card(&mut g, card, vec![], &GIANT_COST, "CCCR");
    let first = *g.battlefield.last().unwrap();
    assert_eq!(state(&g, first), (Some((2, 2)), Some(2)));
    // back to its owner's hand: a new object, which is cast as a new spell
    let unsummon = g.put(0, UNSUMMON, Zone::Hand);
    cast_card(&mut g, unsummon, vec![vec![Target::Object(first)]], &[ManaSymbol::Color(Color::Blue)], "U");
    let again = *g.players[0].hand.iter().find(|o| g.obj(**o).chars.name == "Hotheaded Giant").expect("returned to hand");
    cast_card(&mut g, again, vec![], &GIANT_COST, "CCCR");
    let now = *g.battlefield.last().unwrap();
    assert_ne!(now, first);
    cite!(["400.7"]; assert_eq!(state(&g, now), (Some((4, 4)), None), "its first cast was a different spell (400.7)"));
}

#[test]
fn r614_12_a_giant_put_onto_the_battlefield_uncast_asks_all_the_same() {
    let black = [ManaSymbol::Color(Color::Black), ManaSymbol::Colorless, ManaSymbol::Colorless, ManaSymbol::Colorless];
    for shocked in [false, true] {
        let mut g = game();
        let card = g.put(0, GIANT, Zone::Graveyard);
        if shocked {
            let shock = g.put(0, SHOCK, Zone::Hand);
            cast_card(&mut g, shock, vec![vec![Target::Player(1)]], &[RED], "R");
        }
        let zombify = g.put(0, ZOMBIFY, Zone::Hand);
        cast_card(&mut g, zombify, vec![vec![Target::Object(card)]], &black, "CCCB");
        let giant = *g.battlefield.last().unwrap();
        assert_eq!(g.obj(giant).chars.name, "Hotheaded Giant");
        let expected = if shocked { (Some((4, 4)), None) } else { (Some((2, 2)), Some(2)) };
        cite!(["614.12"]; case: format!("shocked {shocked}"); assert_eq!(state(&g, giant), expected, "a red spell cast this turn: {shocked}"));
    }
}
