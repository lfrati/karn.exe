//! Combined entry costs (614.12b), through resolution and simultaneous one-shot returns.
use crate::support::{cite, paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};

const RECLAMATION: usize = 0;
const TEMPLE: usize = 1;
const WATERY: usize = 2;
const PRISON: usize = 3;
const NATURALIZE: usize = 4;

fn game(players: usize, life: i32) -> Game {
    let defs = vec![
        CardDef::new("Splendid Reclamation", "{3}{G}", "Sorcery", "Return all land cards from your graveyard to the battlefield tapped.", None).unwrap(),
        CardDef::new("Temple Garden", "", "Land — Forest Plains", "As this land enters, you may pay 2 life. If you don't, it enters tapped.", None).unwrap(),
        CardDef::new("Watery Grave", "", "Land — Island Swamp", "As this land enters, you may pay 2 life. If you don't, it enters tapped.", None).unwrap(),
        // Rule-valid fixture isolates 610.3d without introducing an unrelated card mechanic.
        CardDef::new("Entry return fixture", "{0}", "Enchantment", "{0}: Exile target permanent until this enchantment leaves the battlefield.", None).unwrap(),
        CardDef::new("Naturalize", "{1}{G}", "Instant", "Destroy target artifact or enchantment.", None).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap_or_else(|e| panic!("{}: {}", def.name(), e.0)); }
    let mut g = Game::new(defs, players, life);
    g.players[0].pool.colors = [10; 5];
    g
}

fn pass(g: &mut Game) {
    for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); }
}

fn cast(g: &mut Game, def: usize, targets: Vec<Vec<Target>>, spending: &str) {
    let card = g.put(0, def, Zone::Hand);
    g.act(0, paid(spending, Move::Cast { card, targets })).unwrap();
    pass(g);
}

fn offered(g: &Game, player: usize) {
    assert_eq!(g.decision, Some(Decision::Choose { player, choice: Choice::May { possible: true } }));
}

fn answer(g: &mut Game, player: usize, yes: bool) {
    offered(g, player);
    g.act(player, Move::Choose(if yes { Answer::Yes } else { Answer::No })).unwrap();
}

fn payments(g: &Game, who: usize) -> Vec<u32> {
    g.log.iter().filter_map(|e| match e {
        Event::LifeLost { player, amount } if *player == who => Some(*amount),
        _ => None,
    }).collect()
}

fn reclaim(life: i32) -> Game {
    let mut g = game(2, life);
    g.put(0, TEMPLE, Zone::Graveyard);
    g.put(0, WATERY, Zone::Graveyard);
    cast(&mut g, RECLAMATION, vec![], "WWWG");
    g
}

#[test]
fn r614_12b_reclamation_cannot_commit_four_life_when_only_three_is_payable() {
    let mut g = reclaim(3);
    offered(&g, 0);
    assert_eq!(g.players[0].life, 3);
    assert!(g.battlefield.is_empty());
    answer(&mut g, 0, true);
    cite!(["614.12b"]; assert!(g.decision.is_none(), "the second payment is no longer affordable and must not be offered"));
    cite!(["119.4"]; assert_eq!(g.players[0].life, 1));
    assert_eq!(payments(&g, 0), [2]);
    assert_eq!(g.battlefield.len(), 2);
    assert!(g.battlefield.iter().all(|o| g.obj(*o).tapped), "Reclamation itself requires tapped entry");
    crate::support::refused(g.act(0, Move::Choose(Answer::Yes)), "no decision is pending");
}

#[test]
fn r614_12b_declining_does_not_reserve_life_and_cloned_answers_are_independent() {
    let mut decline = reclaim(3);
    let mut accept = decline.clone();
    answer(&mut decline, 0, false);
    offered(&decline, 0);
    // the choices come before entry and are paid as the lands enter (614.12a), and their combined costs must be payable (614.12b)
    cite!(["614.12a", "614.12b"]; assert_eq!(decline.players[0].life, 3, "nothing has been paid while collecting"));
    assert!(decline.battlefield.is_empty());
    answer(&mut accept, 0, true);
    assert!(accept.decision.is_none());
    assert_eq!(decline.players[0].life, 3, "the clone must not share payment commitments");
    answer(&mut decline, 0, true);
    cite!(["614.12b"]; assert_eq!(decline.players[0].life, 1));
    assert_eq!(payments(&decline, 0), [2]);
}

#[test]
fn r119_4_exactly_four_life_can_pay_both_entries_before_state_based_loss() {
    let mut g = reclaim(4);
    answer(&mut g, 0, true);
    offered(&g, 0);
    // mid-collection: the end state (4 -> 0) alone could not tell joint from sequential payment
    cite!(["614.12a", "614.12b"]; assert_eq!(g.players[0].life, 4, "the first Yes is only a commitment"));
    assert!(g.battlefield.is_empty());
    answer(&mut g, 0, true);
    cite!(["119.4","614.12b"]; assert_eq!(payments(&g, 0), [2, 2]));
    assert_eq!(g.players[0].life, 0);
    cite!(["704.5a"]; assert!(g.players[0].lost));
    let lost = g.log.iter().position(|e| matches!(e, Event::Lost { player: 0 })).unwrap();
    let entries = g.log[..lost].iter().filter(|e| matches!(e, Event::ZoneChanged { to: Zone::Battlefield, .. })).count();
    cite!(["704.3"]; assert_eq!(entries, 2, "entry completes before state-based actions"));
}

#[test]
fn r119_4_unpayable_entry_costs_offer_no_payment_and_lose_no_life() {
    let g = reclaim(1);
    cite!(["119.4"]; assert!(g.decision.is_none()));
    assert_eq!(g.players[0].life, 1);
    assert!(payments(&g, 0).is_empty());
    assert_eq!(g.battlefield.len(), 2);
}

fn exile_land(g: &mut Game, prison: ObjId, owner: usize, def: usize) {
    // Fixture placement establishes the pre-exile state; the tested entry is the return.
    let land = g.put(owner, def, Zone::Battlefield);
    g.act(0, Move::Activate { source: prison, ability: 0, targets: vec![vec![Target::Object(land)]], paying: vec![] }).unwrap();
    pass(g);
    assert_eq!(g.obj(land).zone, Zone::Gone);
}

fn release(g: &mut Game, prison: ObjId) {
    cast(g, NATURALIZE, vec![vec![Target::Object(prison)]], "WG");
}

#[test]
fn r614_12b_simultaneous_until_returns_share_the_same_payment_budget() {
    let mut g = game(2, 3);
    let prison = g.put(0, PRISON, Zone::Battlefield);
    exile_land(&mut g, prison, 0, TEMPLE);
    exile_land(&mut g, prison, 0, WATERY);
    release(&mut g, prison);
    answer(&mut g, 0, true);
    cite!(["614.12b","610.3d"]; assert!(g.decision.is_none(), "the return path must also stop offering unaffordable payments"));
    assert_eq!(g.players[0].life, 1);
    assert_eq!(payments(&g, 0), [2]);
    assert!(g.exile.is_empty());
    cite!(["610.3"]; assert_eq!(g.battlefield.len(), 2));
    assert_eq!(g.battlefield.iter().filter(|o| g.obj(**o).tapped).count(), 1);
}

#[test]
fn r614_12b_return_commitments_survive_suspension_and_clone_without_double_payment() {
    let mut g = game(2, 5);
    let prison = g.put(0, PRISON, Zone::Battlefield);
    exile_land(&mut g, prison, 0, TEMPLE);
    exile_land(&mut g, prison, 0, WATERY);
    exile_land(&mut g, prison, 0, TEMPLE);
    release(&mut g, prison);
    answer(&mut g, 0, true);
    offered(&g, 0);
    assert_eq!(g.players[0].life, 5);
    assert_eq!(g.exile.len(), 3);
    cite!(["614.12b"]; assert!(g.exile.contains(&g.entering_card().expect("pending return identifies its card"))));
    assert!(matches!(g.public_choices().next(), Some(game::state::PublicChoice::Entry { player: 0, answer: Answer::Yes, .. })));
    assert_eq!(g.public_choices().count(), 1);
    let mut other = g.clone();
    answer(&mut g, 0, true);
    cite!(["614.12b"]; assert!(g.decision.is_none(), "the third payment exceeds the budget"));
    assert_eq!(payments(&g, 0), [2, 2]);
    assert_eq!(g.players[0].life, 1);
    answer(&mut other, 0, false);
    assert_eq!(other.public_choices().count(), 2, "return observations survive suspension without duplicates");
    answer(&mut other, 0, false);
    assert_eq!(payments(&other, 0), [2]);
    assert_eq!(other.players[0].life, 3);
}

#[test]
fn r614_12b_each_player_has_their_own_budget_for_simultaneous_returns() {
    let mut g = game(3, 3);
    let prison = g.put(0, PRISON, Zone::Battlefield);
    exile_land(&mut g, prison, 1, TEMPLE);
    exile_land(&mut g, prison, 1, WATERY);
    exile_land(&mut g, prison, 2, TEMPLE);
    release(&mut g, prison);
    answer(&mut g, 1, true);
    offered(&g, 2);
    cite!(["101.4"]; assert_eq!(g.players[1].life, 3, "all choices precede payments and entries"));
    assert_eq!(g.exile.len(), 3);
    answer(&mut g, 2, true);
    assert!(g.decision.is_none());
    cite!(["614.12b"]; assert_eq!((g.players[1].life, g.players[2].life), (1, 1)));
    assert_eq!(payments(&g, 1), [2]);
    assert_eq!(payments(&g, 2), [2]);
    assert_eq!(g.battlefield.len(), 3);
}

#[test]
fn r119_4_large_payment_amounts_do_not_wrap_to_negative_costs() {
    let mut g = game(2, i32::MAX);
    cite!(["119.4"]; assert!(g.can_pay_life(0, i32::MAX as u32)));
    cite!(["119.4"]; assert!(!g.can_pay_life(0, i32::MAX as u32 + 1)));
    assert!(!g.pay_life(0, u32::MAX).unwrap());
    assert_eq!(g.players[0].life, i32::MAX);
    assert!(payments(&g, 0).is_empty());
    g.players[0].life = -1;
    cite!(["119.4b"]; assert!(g.pay_life(0, 0).unwrap(), "119.4b: zero life is always payable"));
}
