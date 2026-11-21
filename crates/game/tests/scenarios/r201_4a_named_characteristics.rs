//! A card name chosen with characteristics is the name of a card with them (201.4a), a split
//! card's half by that half alone (201.4b). "Spells with the chosen name can't be cast." is a
//! prohibition of casting such spells, checked against the spell as it would be cast (601.3e).
//! Meddling Mage, Wear // Tear.
use crate::support::{cite, paid};
use game::binding::{Kind, Value};
use game::legal::Asked;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const MAGE: usize = 0;
const WEAR_TEAR: usize = 1;
const BOLT: usize = 2;
const ISLAND: usize = 3;
const ORDER: usize = 4;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let mut defs: Vec<_> = ["Meddling Mage", "Wear // Tear", "Lightning Bolt", "Island"].map(|n| CardDef::from_faces(&catalog[n]).unwrap()).to_vec();
    defs.push(CardDef::new("Order", "{0}", "Artifact Enchantment", "Players can't cast creature spells.", None).unwrap());
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn pass(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }
fn name(n: &str) -> Move { Move::Choose(Answer::Value(Value::CardName(n.into()))) }
/// Meddling Mage enters for player 0, who is asked for a nonland card name.
fn mage(g: &mut Game) -> ObjId {
    let card = g.put(0, MAGE, Zone::Hand);
    g.players[0].pool.colors[0] += 1;
    g.players[0].pool.colors[1] += 1;
    g.act(0, paid("WU",Move::Cast { card, targets: vec![] })).unwrap();
    pass(g);
    assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::Name(_) })), "{:?}", g.decision);
    card
}

#[test]
fn r201_4a_a_nonland_card_name_is_not_a_land_cards_name() {
    let mut g = game();
    mage(&mut g);
    let Some((0, Asked::Value { kind: Kind::CardName, options })) = g.asked() else { panic!("a name is asked") };
    let has = |n: &str| options.contains(&Value::CardName(n.into()));
    cite!(["201.4a", "201.4b"]; assert!(has("Lightning Bolt") && has("Wear") && has("Tear") && has("Meddling Mage")));
    cite!(["201.4a", "201.4b"]; assert!(!has("Island") && !has("Wear // Tear"), "a land's name, or no single card's name (201.4b)"));
    cite!(["201.4a"]; refused("choose the name of a card with the characteristics the choice says (201.4a)"): g.act(0, name("Island")));
    cite!(["201.4b"]; refused("choose the name of one half of a split card, not both (201.4b)"): g.act(0, name("Wear // Tear")));
    g.act(0, name("Lightning Bolt")).unwrap();
}

#[test]
fn r601_3_spells_with_the_chosen_name_cant_be_cast() {
    let mut g = game();
    mage(&mut g);
    g.act(0, name("Lightning Bolt")).unwrap();
    // its opponent, and its controller too
    for p in [0, 1] {
        let bolt = g.put(p, BOLT, Zone::Hand);
        g.players[p].pool.colors[3] += 1;
        cite!(["601.3"]; refused("an effect prohibits casting that spell (601.3)"): g.act(p, paid("R",Move::Cast { card: bolt, targets: vec![vec![Target::Player(1 - p)]] })));
        g.players[p].pool.colors[3] = 0;
        g.act(p, Move::Pass).unwrap();
    }
}

#[test]
fn r601_3e_a_split_cards_half_is_named_alone() {
    let mut g = game();
    mage(&mut g);
    g.act(0, name("Wear")).unwrap();
    let order = g.put(0, ORDER, Zone::Battlefield);
    let card = g.put(1, WEAR_TEAR, Zone::Hand);
    g.act(0, Move::Pass).unwrap();
    g.players[1].pool.colors[3] = 2;
    g.players[1].pool.colors[0] = 1;
    let wear = paid("RR",Move::CastAs(game::card::CastAs::Half(0), Box::new(Move::Cast { card, targets: vec![vec![Target::Object(order)]] })));
    cite!(["601.3"]; refused("an effect prohibits casting that spell (601.3)"): g.act(1, wear));
    // Tear is another name: casting it is not prohibited
    cite!(["709.3a"]; ok: g.act(1, paid("W",Move::CastAs(game::card::CastAs::Half(1), Box::new(Move::Cast { card, targets: vec![vec![Target::Object(order)]] })))));
}

#[test]
fn r601_3e_players_cant_cast_spells_with_a_characteristic() {
    let mut g = game();
    g.put(1, ORDER, Zone::Battlefield);
    let card = g.put(0, MAGE, Zone::Hand);
    g.players[0].pool.colors[0] += 1;
    g.players[0].pool.colors[1] += 1;
    cite!(["601.3"]; refused("an effect prohibits casting that spell (601.3)"): g.act(0, paid("WU",Move::Cast { card, targets: vec![] })));
    let bolt = g.put(0, BOLT, Zone::Hand);
    g.players[0].pool.colors[3] += 1;
    cite!(["601.3"]; ok: g.act(0, paid("R",Move::Cast { card: bolt, targets: vec![vec![Target::Player(1)]] })));
}
