//! "Whenever one or more …" (603.2c): once for each event, however many of them it was of,
//! each of them the trigger's subject. Damage dealt at once (510.2), creatures declared as
//! attackers at once (508.1) and what one instruction does (608.2) are one event; a separate
//! damage step or state-based check is another. What the phrase says ("to a player") tells
//! the events apart; what it leaves unsaid (the source of damage) does not.
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;
use crate::support::{cite, paid};

const BEAR: usize = 0;
const PIRATES: usize = 1;
const STRIKER: usize = 2;
const MOURNER: usize = 3;
const BURN: usize = 4;
const WOUNDED: usize = 5;
const ISLAND: usize = 6;
const WARDEN: usize = 7;
const PURGE: usize = 8;
const SCARRED: usize = 9;
const HOLLOW: usize = 10;
const TIDE: usize = 11;

fn game(players: usize) -> Game {
    let c = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    let defs = vec![
        c("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        c("Pirates' Map", "{1}{U}", "Enchantment", "Whenever one or more creatures you control deal combat damage to a player, draw a card.", None),
        c("Striker", "{R}", "Creature — Human", "First strike", Some((2, 2))),
        c("Mourner", "{B}", "Enchantment", "Whenever one or more other creatures you control die, you gain 1 life.", None),
        c("Burn", "{R}", "Sorcery", "Burn deals 2 damage to each creature.", None),
        c("Wounded", "{G}", "Enchantment", "Whenever one or more creatures you control are dealt damage, you gain 1 life.", None),
        c("Island", "", "Basic Land — Island", "", None),
        c("Warden", "{B}", "Enchantment", "Whenever one or more cards leave your graveyard, you gain 1 life.", None),
        c("Purge", "{B}", "Sorcery", "Exile all graveyards.", None),
        c("Scarred", "{G}", "Enchantment", "Whenever one or more creatures you control deal combat damage to a player, put a +1/+1 counter on each of those creatures.", None),
        c("Hollow", "{G}", "Enchantment", "Whenever one or more creatures you control die, if you control no creatures, you gain 1 life.", None),
        c("Tide", "{U}", "Enchantment", "Whenever one or more creatures you control become untapped, you gain 1 life.", None),
    ];
    for d in defs.iter().filter(|d| d.name() != "Hollow") { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, players, 20);
    for p in 0..players { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    g
}

/// Pass, answering combat decisions with what `attack` and `blocks` say, until `part`.
fn until(g: &mut Game, part: Part, attack: &[(ObjId, Target)], blocks: &[(ObjId, ObjId)]) {
    until_ordered(g, part, attack, blocks, &[]);
}
fn until_ordered(g: &mut Game, part: Part, attack: &[(ObjId, Target)], blocks: &[(ObjId, ObjId)], orders: &[(usize, Vec<ObjId>)]) {
    let mut orders=orders.iter();
    for _ in 0..100 {
        if g.part == part && g.stack.is_empty() && g.decision.is_none() { return; }
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(attack.to_vec())).unwrap(),
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(blocks.to_vec())).unwrap(),
            None => g.act(g.priority, Move::Pass).unwrap(),
            Some(Decision::Choose { choice: game::state::Choice::OrderCards {..}, player }) => {
                let (owner, cards)=orders.next().expect("an authored graveyard order"); assert_eq!(player,*owner);
                cite!(["404.3"]; ok: g.act(player,Move::Choose(game::state::Answer::Objects(cards.clone()))));
            }
            other => panic!("{other:?}"),
        }
    }
    panic!("never reached {part:?}");
}
fn hand(g: &Game) -> usize { g.players[0].hand.len() }

#[test]
fn r510_2_combat_damage_to_a_player_is_one_event_for_each_player() {
    let mut g = game(2);
    g.put(0, PIRATES, Zone::Battlefield);
    let (a, b) = (g.put(0, BEAR, Zone::Battlefield), g.put(0, BEAR, Zone::Battlefield));
    let before = hand(&g);
    until(&mut g, Part::EndOfCombat, &[(a, Target::Player(1)), (b, Target::Player(1))], &[]);
    cite!(["510.2", "603.2c"]; assert_eq!((hand(&g), g.players[1].life), (before + 1, 16)));

    // two players dealt damage: once for each
    let mut g = game(3);
    let map=g.put(0, PIRATES, Zone::Battlefield);
    let (a, b) = (g.put(0, BEAR, Zone::Battlefield), g.put(0, BEAR, Zone::Battlefield));
    let before = hand(&g);
    until(&mut g, Part::DeclareBlockers, &[(a, Target::Player(1)), (b, Target::Player(2))], &[]);
    for _ in 0..3 {g.act(g.priority,Move::Pass).unwrap();}
    let Some(Decision::Order{player:0,abilities})=g.decision.clone() else {panic!("expected distinct damaged-player trigger ordering: {:?}",g.decision)};
    cite!(["603.3b"]; assert_eq!(abilities.len(),2));
    assert!(abilities.iter().all(|ability|ability.source==Some(map)));
    // Each trigger draws one; choose their offered order, preserving both.
    g.act(0,Move::Order(vec![0,1])).unwrap();
    until(&mut g, Part::EndOfCombat, &[(a, Target::Player(1)), (b, Target::Player(2))], &[]);
    cite!(["603.2c"]; assert_eq!(hand(&g), before + 2));
}

#[test]
fn r510_4_each_combat_damage_step_is_its_own_event() {
    let mut g = game(2);
    g.put(0, PIRATES, Zone::Battlefield);
    let (striker, bear) = (g.put(0, STRIKER, Zone::Battlefield), g.put(0, BEAR, Zone::Battlefield));
    let before = hand(&g);
    until(&mut g, Part::EndOfCombat, &[(striker, Target::Player(1)), (bear, Target::Player(1))], &[]);
    cite!(["510.4", "603.2c"]; assert_eq!(hand(&g), before + 2));
}

#[test]
fn r608_2k_those_creatures_are_every_one_that_dealt_the_damage() {
    let mut g = game(2);
    g.put(0, SCARRED, Zone::Battlefield);
    let (a, b, home) = (g.put(0, BEAR, Zone::Battlefield), g.put(0, BEAR, Zone::Battlefield), g.put(0, BEAR, Zone::Battlefield));
    until(&mut g, Part::EndOfCombat, &[(a, Target::Player(1)), (b, Target::Player(1))], &[]);
    cite!(["608.2k"]; assert_eq!((g.power(a), g.power(b), g.power(home)), (Some(3), Some(3), Some(2))));
}

#[test]
fn r704_3_creatures_dying_in_one_check_die_together() {
    // two Bears die to one burn: once, not for the burned Mourner's controller's other card
    let mut g = game(2);
    g.put(0, MOURNER, Zone::Battlefield);
    let a=g.put(0, BEAR, Zone::Battlefield);
    let b=g.put(0, BEAR, Zone::Battlefield);
    g.put(1, BEAR, Zone::Battlefield);
    let burn = g.put(0, BURN, Zone::Hand);
    g.players[0].pool.colors[3] = 1;
    until(&mut g, Part::PrecombatMain, &[], &[]);
    g.act(0, paid("R", Move::Cast { card: burn, targets: vec![] })).unwrap();
    for _ in 0..2 {g.act(g.priority, Move::Pass).unwrap();}
    cite!(["404.3"]; ok: g.act(0,Move::Choose(game::state::Answer::Objects(vec![a,b]))));
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["704.3", "603.2c"]; assert_eq!(g.players[0].life, 21));
    assert!(!g.battlefield.iter().any(|o| g.obj(*o).card == Some(BEAR)));
}

#[test]
fn r603_2c_what_is_left_unsaid_does_not_split_the_event() {
    // two of your creatures blocked by two different creatures: dealt damage by different
    // sources at once, which is one event
    let mut g = game(2);
    g.put(0, WOUNDED, Zone::Battlefield);
    let (a, b) = (g.put(0, BEAR, Zone::Battlefield), g.put(0, BEAR, Zone::Battlefield));
    let (x, y) = (g.put(1, BEAR, Zone::Battlefield), g.put(1, BEAR, Zone::Battlefield));
    until_ordered(&mut g, Part::EndOfCombat, &[(a, Target::Player(1)), (b, Target::Player(1))], &[(x, a), (y, b)], &[(0,vec![a,b]),(1,vec![x,y])]);
    cite!(["603.2c"]; assert_eq!(g.players[0].life, 21));
}

#[test]
fn r603_2c_cards_leaving_a_graveyard_at_once() {
    let mut g = game(2);
    g.put(0, WARDEN, Zone::Battlefield);
    for _ in 0..3 { g.put(0, BEAR, Zone::Graveyard); }
    let purge = g.put(0, PURGE, Zone::Hand);
    g.players[0].pool.colors[2] = 1;
    until(&mut g, Part::PrecombatMain, &[], &[]);
    g.act(0, paid("B", Move::Cast { card: purge, targets: vec![] })).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["603.2c"]; assert_eq!(g.players[0].life, 21));
    assert!(g.players[0].graveyard.iter().all(|c| g.obj(*c).card == Some(PURGE)));
}

#[test]
fn r603_4_an_intervening_if_of_one_or_more_of_them_is_refused() {
    // it would be asked of the first of them before the rest are gathered
    let g = game(2);
    // Uncited (admission): gathering the whole subject group before testing the if is not modeled.
    crate::support::refused(game::play::check(&g.defs[HOLLOW]).map_err(|e|e.0), "trigger: an intervening \"if\" of one or more of them");
}

#[test]
fn r502_3_permanents_untap_at_once() {
    let mut g = game(2);
    g.put(1, TIDE, Zone::Battlefield);
    for _ in 0..2 { let bear = g.put(1, BEAR, Zone::Battlefield); g.set_tapped(bear, true); }
    until(&mut g, Part::Upkeep, &[], &[]);
    // the next turn is player 1's: its untap step untaps both Bears together
    while g.active != 1 || g.part != Part::Upkeep || !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["502.3", "603.2c"]; assert_eq!(g.players[1].life, 21));
}
