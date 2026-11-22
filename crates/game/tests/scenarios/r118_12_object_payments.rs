//! "[Do something] unless [a player does something else]" means "[that player] may [do
//! something else]. If they don't, [do something]" (118.12a), whatever the payment is: mana,
//! or objects to sacrifice, discard, exile, return or tap. Mana payments require supplied
//! instructions; an invalid payment is refused (118.3), and declining follows the unpaid
//! outcome (118.12a). Object choices retain their mandatory domains. The pinned cards.

use crate::support::{cite, mana_pool, paid};
use game::card::CardDef;
use game::state::{Answer, Choice, Decision};
use game::{Event, Game, Move, Target, Zone};

fn pinned(names: &[&str]) -> Vec<CardDef> {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    names.iter().map(|name| {
        let def = CardDef::from_faces(catalog.get(name).unwrap_or_else(|| panic!("{name} is pinned"))).unwrap();
        game::play::check(&def).unwrap_or_else(|e| panic!("{name}: {}", e.0));
        def
    }).collect()
}

/// Libraries of the last card for both players.
fn game(names: &[&str]) -> Game {
    let mut g = Game::new(pinned(names), 2, 20);
    for player in 0..2 {
        for _ in 0..10 { g.put(player, names.len() - 1, Zone::Library); }
    }
    g
}

fn pass(g: &mut Game) {
    for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); }
}

/// Plays on to player 0's next upkeep, until their upkeep trigger asks or is done.
fn upkeep(g: &mut Game) {
    let from = g.log.len();
    for _ in 0..300 {
        let began = g.log[from..].iter().any(|e| matches!(e, Event::Began { part: ir::Part::Upkeep })) && g.active == 0;
        if began && g.decision.is_some() { return; }
        if began && g.stack.is_empty() && g.log[from..].iter().any(|e| matches!(e, Event::Began { part: ir::Part::Draw })) { return; }
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(other) => panic!("not asked here: {other:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("no upkeep of player 0 came");
}

fn answer(g: &mut Game, player: usize, answer: Answer) {
    g.act(player, Move::Choose(answer)).unwrap();
}

#[test]
fn r118_12a_phantasmal_forces_stays_only_when_its_controller_pays() {
    for pay in [false, true] {
        let mut g = game(&["Phantasmal Forces", "Island"]);
        let forces = g.put(0, 0, Zone::Battlefield);
        let island=g.put(0, 1, Zone::Battlefield);
        upkeep(&mut g);
        cite!(["118.12a"]; case: format!("pay {pay}"); assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::Pay { .. } })), "{:?}", g.decision));
        cite!(["118.12a"]; refused("that decision is another player's"): act_unchanged(&mut g, 1, Move::Choose(Answer::Yes)));
        answer(&mut g, 0, if pay { Answer::Pay{plan:vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None,source:island,ability:0,paying:vec![],adding:vec![],chose:vec![],cost_branches:None}],with:Some(mana_pool("U")),tapped:vec![]} } else { Answer::No });
        assert!(g.decision.is_none());
        cite!(["118.12a"]; case: format!("pay {pay}"); assert_eq!(g.battlefield.contains(&forces), pay));
    }
    // With no resources, an explicit attempted payment fails; declining sacrifices it.
    let mut g = game(&["Phantasmal Forces", "Island"]);
    let forces = g.put(0, 0, Zone::Battlefield);
    upkeep(&mut g);
    cite!(["118.3"]; refused("that mana is not in your mana pool"): act_unchanged(&mut g, 0, Move::Choose(Answer::Pay{plan:vec![],with:Some(mana_pool("U")),tapped:vec![]})));
    answer(&mut g,0,Answer::No);
    cite!(["118.12a"]; assert!(!g.battlefield.contains(&forces)));
}

#[test]
fn r118_12a_waterspout_djinn_returns_an_untapped_island_its_controller_chooses() {
    let mut g = game(&["Waterspout Djinn", "Island"]);
    let djinn = g.put(0, 0, Zone::Battlefield);
    let islands = [g.put(0, 1, Zone::Battlefield), g.put(0, 1, Zone::Battlefield)];
    let theirs = g.put(1, 1, Zone::Battlefield);
    upkeep(&mut g);
    cite!(["118.12a"]; assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May { possible: true } })), "{:?}", g.decision));
    answer(&mut g, 0, Answer::Yes);
    let Some(Decision::Choose { player: 0, choice: Choice::Objects { among, count: 1, .. } }) = g.decision.clone() else { panic!("{:?}", g.decision) };
    cite!(["608.2d"]; assert_eq!(among, islands.to_vec(), "only untapped Islands its controller controls"));
    answer(&mut g, 0, Answer::Objects(vec![islands[1]]));
    cite!(["118.12a"]; assert!(g.battlefield.contains(&djinn)));
    assert!(g.battlefield.contains(&islands[0]) && g.battlefield.contains(&theirs));
    assert!(!g.battlefield.contains(&islands[1]));
    assert!(g.players[0].hand.iter().any(|&c| g.obj(c).card == Some(1)), "the Island is in its owner's hand");

    // declined: the Djinn goes, the Islands stay
    let mut g = game(&["Waterspout Djinn", "Island"]);
    let djinn = g.put(0, 0, Zone::Battlefield);
    let island = g.put(0, 1, Zone::Battlefield);
    upkeep(&mut g);
    answer(&mut g, 0, Answer::No);
    assert!(g.decision.is_none());
    cite!(["118.12a"]; assert!(!g.battlefield.contains(&djinn) && g.battlefield.contains(&island)));

    // only an opponent's Island: not offered
    let mut g = game(&["Waterspout Djinn", "Island"]);
    let djinn = g.put(0, 0, Zone::Battlefield);
    g.put(1, 1, Zone::Battlefield);
    upkeep(&mut g);
    cite!(["118.3"]; assert!(g.decision.is_none()));
    cite!(["118.12a"]; assert!(!g.battlefield.contains(&djinn)));
}

#[test]
fn r118_12a_command_bridge_taps_another_untapped_permanent_or_is_sacrificed() {
    for other in [false, true] {
        let mut g = game(&["Command Bridge", "Forest"]);
        let forest = other.then(|| g.put(0, 1, Zone::Battlefield));
        let card = g.put(0, 0, Zone::Hand);
        g.act(0, Move::PlayLand(card)).unwrap();
        let bridge = *g.battlefield.last().unwrap();
        assert!(g.obj(bridge).tapped, "it enters tapped, so it can't pay for itself");
        pass(&mut g);
        match forest {
            Some(forest) => {
                cite!(["118.12a"]; assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May { possible: true } })), "{:?}", g.decision));
                answer(&mut g, 0, Answer::Yes);
                assert!(g.obj(forest).tapped);
                cite!(["118.12a"]; assert!(g.battlefield.contains(&bridge)));
            }
            None => {
                cite!(["118.3"]; assert!(g.decision.is_none()));
                cite!(["118.12a"]; assert!(!g.battlefield.contains(&bridge)));
            }
        }
    }
}

#[test]
fn r118_12a_rotting_giant_exiles_a_card_from_its_controllers_graveyard() {
    for (cards, pay) in [(1, true), (1, false), (0, false)] {
        let mut g = game(&["Rotting Giant", "Forest"]);
        let giant = g.put(0, 0, Zone::Battlefield);
        let card = (cards > 0).then(|| g.put(0, 1, Zone::Graveyard));
        g.put(1, 1, Zone::Graveyard);
        pass(&mut g);
        pass(&mut g);
        g.act(0, Move::Attack(vec![(giant, Target::Player(1))])).unwrap();
        pass(&mut g);
        if let Some(card) = card {
            cite!(["118.12a"]; case: format!("cards {cards}, pay {pay}"); assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May { possible: true } })), "{:?}", g.decision));
            answer(&mut g, 0, if pay { Answer::Yes } else { Answer::No });
            assert_eq!(g.players[0].graveyard.contains(&card), !pay);
        }
        assert!(g.decision.is_none());
        cite!(["118.12a"]; case: format!("cards {cards}, pay {pay}"); assert_eq!(g.battlefield.contains(&giant), pay));
        assert_eq!(g.players[1].graveyard.len(), 1, "not an opponent's graveyard");
    }
}

#[test]
fn r118_12a_thirst_for_identity_discards_a_creature_card_or_two_cards() {
    for pay in [false, true] {
        let mut g = game(&["Thirst for Identity", "Grizzly Bears", "Island"]);
        let creature = g.put(0, 1, Zone::Hand);
        let card = g.put(0, 0, Zone::Hand);
        g.players[0].pool.colors[ir::Color::Blue as usize] = 3;
        g.act(0, paid("UUU", Move::Cast { card, targets: vec![] })).unwrap();
        pass(&mut g);
        cite!(["118.12a"]; assert_eq!(g.players[0].hand.len(), 4));
        assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May { possible: true } })), "{:?}", g.decision);
        answer(&mut g, 0, if pay { Answer::Yes } else { Answer::No });
        if pay { answer(&mut g, 0, Answer::Objects(vec![creature])); }
        if !pay {
            let Some(Decision::Choose { choice: Choice::Objects { count: 2, .. }, .. }) = g.decision.clone() else { panic!("{:?}", g.decision) };
            let two = g.players[0].hand[..2].to_vec();
            answer(&mut g, 0, Answer::Objects(two.clone()));
            cite!(["404.3"]; ok: g.act(0, Move::Choose(Answer::Objects(two))));
        }
        assert!(g.decision.is_none(), "{:?}", g.decision);
        cite!(["118.12a"]; case: format!("pay {pay}"); assert_eq!(g.players[0].hand.len(), if pay { 3 } else { 2 }));
        if pay { cite!(["400.7"]; assert!(g.players[0].graveyard.iter().any(|&c| g.obj(c).card == Some(1)), "the creature card, as a new object (400.7)")); }
    }
    // No creature card: the hidden qualification cannot suppress the optional boundary.
    let mut g = game(&["Thirst for Identity", "Island"]);
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[ir::Color::Blue as usize] = 3;
    g.act(0, paid("UUU", Move::Cast { card, targets: vec![] })).unwrap();
    pass(&mut g);
    cite!(["402.3"]; assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May { possible: false } }))));
    answer(&mut g, 0, Answer::No);
    cite!(["118.3", "118.12a"]; assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::Objects { count: 2, .. } })), "{:?}", g.decision));
}

/// "where X is …" between the payment and the act still skips the act when paid.
#[test]
fn r118_12a_esper_sentinel_draws_only_when_the_caster_does_not_pay_x() {
    for pay in [false, true] {
        let mut g = game(&["Esper Sentinel", "Opt", "Island"]);
        g.put(0, 0, Zone::Battlefield);
        let island=g.put(1, 2, Zone::Battlefield);
        g.players[1].pool.colors[ir::Color::Blue as usize] = 1;
        g.act(0, Move::Pass).unwrap();
        let hand = g.players[0].hand.len();
        let opt = g.put(1, 1, Zone::Hand);
        g.act(1, paid("U", Move::Cast { card: opt, targets: vec![] })).unwrap();
        pass(&mut g);
        cite!(["118.12a"]; case: format!("pay {pay}"); assert!(matches!(g.decision, Some(Decision::Choose { player: 1, choice: Choice::Pay { .. } })), "{:?}", g.decision));
        answer(&mut g, 1, if pay { Answer::Pay{plan:vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None,source:island,ability:0,paying:vec![],adding:vec![],chose:vec![],cost_branches:None}],with:Some(mana_pool("U")),tapped:vec![]} } else { Answer::No });
        assert!(g.decision.is_none(), "{:?}", g.decision);
        cite!(["118.12a"]; case: format!("pay {pay}"); assert_eq!(g.players[0].hand.len(), hand + usize::from(!pay)));
    }
}

#[test]
fn r118_12a_the_or_form_and_at_random_payments_are_refused() {
    for (text, reason) in [
        ("Each opponent loses 3 life unless they discard a card or sacrifice a creature.", "unless: alternative payments"),
        ("You lose 3 life unless you discard a card at random.", "choice: at random selection not lowered"),
    ] {
        let d = CardDef::new("Procedure", "{0}", "Sorcery", text, None).unwrap();
        assert_eq!(game::play::check(&d).expect_err("unsupported unless payment").0,reason,"{text}");
    }
}

#[track_caller]
fn act_unchanged(g: &mut Game, player: usize, action: Move) -> Result<(), String> {
    let before = format!("{g:?}");
    let result = g.act(player, action);
    assert_eq!(format!("{g:?}"), before);
    result
}
