//! A cost selects objects in its stated zone; the same pool is offered and validated (118.3).
use crate::support::{cite,paid,mana_pool};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

fn game(text: &str) -> Game {
    let defs = vec![
        CardDef::new("Source", "{0}", "Creature — Spirit", text, Some((3, 3))).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Reward", "{2}", "Sorcery", "You gain 3 life.", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    Game::new(defs, 3, 20)
}
fn activate(g: &mut Game, source: ObjId, paying: Vec<Vec<ObjId>>) -> Result<(), String> {
    g.act(0, Move::Activate { source, ability: 0, targets: vec![], paying })
}
fn pass(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
#[test]
fn r601_2h_graveyard_exile_costs_offer_and_require_distinct_matching_cards() {
    let mut g = game("Exile three creature cards from your graveyard: You gain 1 life.");
    let source = g.put(0, 0, Zone::Battlefield);
    let cards: Vec<_> = (0..4).map(|_| g.put(0, 1, Zone::Graveyard)).collect();
    let land = g.put(0, 2, Zone::Graveyard);
    let foreign = g.put(1, 1, Zone::Graveyard);
    let hand = g.put(0, 1, Zone::Hand);
    let permanent = g.put(0, 1, Zone::Battlefield);
    // a card twice; two of three; another's graveyard, the hand, a land, the battlefield
    for (chosen, why) in [
        (vec![cards[0], cards[0], cards[1]], "a card is chosen once for that cost (601.2h)"),
        (vec![cards[0], cards[1]], "which asks for exactly 3 card(s): 2 chosen"),
        (vec![cards[0], cards[1], foreign], "Bear is not what that cost asks for"),
        (vec![cards[0], cards[1], hand], "Bear is not what that cost asks for"),
        (vec![cards[0], cards[1], land], "Forest is not what that cost asks for"),
        (vec![cards[0], cards[1], permanent], "Bear is not what that cost asks for"),
    ] {
        let before = format!("{g:?}");
        cite!(["601.2h"]; refused(why): activate(&mut g, source, vec![chosen]));
        assert_eq!(format!("{g:?}"), before);
    }
    cite!(["601.2h"]; ok: activate(&mut g, source, vec![cards[..3].to_vec()]));
    assert_eq!(g.exile.len(), 3);
    assert_eq!(g.players[0].life, 20);
    pass(&mut g);
    assert_eq!(g.players[0].life, 21);
    cite!(["118.3"]; assert_eq!(g.players[1].graveyard, vec![foreign]));
}
#[test]
fn r400_7_a_graveyard_cost_cannot_pay_with_a_stale_card_identity() {
    let mut g = game("Exile a creature card from your graveyard: You gain 1 life.");
    let source = g.put(0, 0, Zone::Battlefield);
    let card = g.put(0, 1, Zone::Graveyard);
    g.move_object(card, Zone::Hand);
    g.move_object(g.players[0].hand[0], Zone::Graveyard);
    let before = format!("{g:?}");
    cite!(["400.7"]; refused("Bear is not what that cost asks for"): activate(&mut g, source, vec![vec![card]]));
    assert_eq!(format!("{g:?}"), before);
}
#[test]
fn r113_6m_self_exile_from_a_graveyard_functions_only_there_and_for_its_owner() {
    let mut g = game("Exile this card from your graveyard: You gain 1 life.");
    let battlefield = g.put(0, 0, Zone::Battlefield);
    let mine = g.put(0, 0, Zone::Graveyard);
    let theirs = g.put(1, 0, Zone::Graveyard);
    cite!(["113.6m"]; refused("that ability functions only in the zone it moves its card out of (113.6m)"): activate(&mut g, battlefield, vec![]));
    cite!(["108.4a", "602.2"]; refused("an ability of a card outside the battlefield is activated by its owner (108.4a, 602.2)"): activate(&mut g, theirs, vec![]));
    cite!(["113.6m"]; ok: activate(&mut g, mine, vec![]));
    assert_eq!(g.obj(mine).zone, Zone::Gone);
    assert_eq!(g.exile.len(), 1);
    pass(&mut g);
    assert_eq!(g.players[0].life, 21);
}
#[test]
fn r400_7j_an_effect_finds_the_card_exiled_from_a_graveyard_by_its_cost() {
    let mut g = game("Exile a creature card from your graveyard: Return it to the battlefield.");
    let source = g.put(0, 0, Zone::Battlefield);
    let card = g.put(0, 1, Zone::Graveyard);
    activate(&mut g, source, vec![vec![card]]).unwrap();
    assert_eq!(g.exile.len(), 1);
    pass(&mut g);
    assert!(g.exile.is_empty());
    cite!(["400.7j"]; assert_eq!(g.battlefield.len(), 2));
}
#[test]
fn r118_3_a_cost_that_names_any_graveyard_can_use_another_players_card() {
    let mut g = game("Exile a creature card from a graveyard: You gain 1 life.");
    let source = g.put(0, 0, Zone::Battlefield);
    let card = g.put(2, 1, Zone::Graveyard);
    cite!(["118.3"]; ok: activate(&mut g, source, vec![vec![card]]));
    assert_eq!(g.obj(g.exile[0]).owner, 2);
    pass(&mut g);
    assert_eq!(g.players.iter().map(|p| p.life).collect::<Vec<_>>(), vec![21, 20, 20]);
}
#[test]
fn r608_2b_a_target_exiled_to_pay_a_cost_is_illegal_on_resolution() {
    let mut g = game(
        "Exile a creature card from your graveyard: Return target creature card from your graveyard to your hand.",
    );
    let source = g.put(0, 0, Zone::Battlefield);
    let card = g.put(0, 1, Zone::Graveyard);
    g.act(
        0,
        Move::Activate { source, ability: 0, targets: vec![vec![Target::Object(card)]], paying: vec![vec![card]] },
    )
    .unwrap();
    pass(&mut g);
    cite!(["608.2b"]; assert!(g.players[0].hand.is_empty()));
    assert_eq!(g.exile.len(), 1);
}

#[test]
fn r605_3a_a_casting_mana_plan_can_exile_a_graveyard_source() {
    let mut g = game("{1}, Exile this card from your graveyard: Add {C}{C}{C}.");
    g.put(0, 0, Zone::Battlefield); // The same printed card here cannot activate this ability.
    let source = g.put(0, 0, Zone::Graveyard);
    let land = g.put(0, 2, Zone::Battlefield);
    let spell = g.put(0, 3, Zone::Hand);
    let step = |source| game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source, ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None };
    let plan = vec![step(land), game::legal::Step { plan: vec![], x: None, life: vec![],with:Some(mana_pool("G")),..step(source)}];
    g.act(0, Move::WithMana(plan, Box::new(paid("CC",Move::Cast { card: spell, targets: vec![] })))).unwrap();
    assert_eq!(g.exile.len(), 1);
    assert_eq!(g.players[0].pool.total(), 1);
    pass(&mut g);
    cite!(["605.3a"]; assert_eq!(g.players[0].life, 23));
}
#[test]
fn r118_3_mana_plans_exhaust_graveyard_payment_cards_without_reusing_them() {
    let mut g = game("Exile a creature card from your graveyard: Add {C}.");
    let source = g.put(0, 0, Zone::Battlefield);
    let spell = g.put(0, 3, Zone::Hand);
    let first = g.put(0, 1, Zone::Graveyard);
    let step = |card| game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source, ability: 0, paying: vec![vec![card]], adding: vec![], chose: vec![], cost_branches: None };
    let before = format!("{g:?}");
    cite!(["118.3", "118.10"]; refused("Bear is not what that cost asks for"): g.act(0, paid("CC", Move::WithMana(vec![step(first), step(first)], Box::new(Move::Cast { card: spell, targets: vec![] })))));
    assert_eq!(format!("{g:?}"), before);
    let second = g.put(0, 1, Zone::Graveyard);
    let plan = vec![step(first), step(second)];
    g.act(0, paid("CC", Move::WithMana(plan, Box::new(Move::Cast { card: spell, targets: vec![] })))).unwrap();
    assert_eq!(g.exile.len(), 2);
    cite!(["118.3"]; assert!(g.players[0].graveyard.is_empty()));
    pass(&mut g);
    assert_eq!(g.players[0].life, 23);
}

#[test]
fn r602_2b_grim_lavamancer_combines_mana_tapping_and_graveyard_exile_costs() {
    let defs = vec![
        CardDef::new(
            "Grim Lavamancer",
            "{R}",
            "Creature — Human Wizard",
            "{R}, {T}, Exile two cards from your graveyard: This creature deals 2 damage to any target.",
            Some((1, 1)),
        )
        .unwrap(),
        CardDef::new("Mountain", "", "Basic Land — Mountain", "", None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap();
    }
    let mut g = Game::new(defs, 3, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let land = g.put(0, 1, Zone::Battlefield);
    let cards = vec![g.put(0, 1, Zone::Graveyard), g.put(0, 1, Zone::Graveyard)];
    let victim = g.put(2, 2, Zone::Battlefield);
    let plan = vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source: land, ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None }];
    g.act(
        0,
        Move::WithMana(
            plan,
            Box::new(paid("R",Move::Activate {
                source,
                ability: 0,
                targets: vec![vec![Target::Object(victim)]],
                paying: vec![cards],
            })),
        ),
    )
    .unwrap();
    cite!(["602.2b"]; assert!(g.obj(land).tapped && g.obj(source).tapped));
    cite!(["602.2b"]; assert_eq!(g.exile.len(), 2));
    assert_eq!(g.obj(victim).damage, 0);
    pass(&mut g);
    assert_eq!(g.obj(victim).zone, Zone::Gone);
}

#[test]
fn r400_7j_a_plural_activation_payment_binds_every_card_it_exiles() {
    let mut g = game("Exile two creature cards from your graveyard: Return them to the battlefield.");
    let source = g.put(0, 0, Zone::Battlefield);
    let cards = vec![g.put(0, 1, Zone::Graveyard), g.put(0, 1, Zone::Graveyard)];
    activate(&mut g, source, vec![cards]).unwrap();
    pass(&mut g);
    assert!(g.exile.is_empty());
    cite!(["400.7j"]; assert_eq!(g.battlefield.len(), 3));
}
