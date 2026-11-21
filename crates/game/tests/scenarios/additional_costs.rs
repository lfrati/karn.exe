//! Mandatory additional casting costs are paid with the spell's total cost (118.8, 601.2h).
use crate::support::{cite,paid};
use game::{CardDef, Game, Move, Zone};

#[test]
fn r118_8_mandatory_additional_costs_are_casting_instructions_not_resolution_effects() {
    for (name, types, text, pt) in [
        (
            "Village Rites",
            "Instant",
            "As an additional cost to cast this spell, sacrifice a creature.\nDraw two cards.",
            None,
        ),
        (
            "Offering",
            "Creature — Spirit",
            "As an additional cost to cast this spell, exile three creature cards from your graveyard.\nFlying",
            Some((5, 6)),
        ),
    ] {
        let d = CardDef::new(name, "{0}", types, text, pt).unwrap();
        game::play::check(&d).unwrap();
    }
}
#[test]
fn r601_2h_additional_life_cost_is_paid_before_resolution_and_cannot_be_partially_paid() {
    let d = CardDef::new(
        "Bargain",
        "{0}",
        "Instant",
        "As an additional cost to cast this spell, pay 3 life.\nYou gain 5 life.",
        None,
    )
    .unwrap();
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d], 3, 20);
    let spell = g.put(0, 0, Zone::Hand);
    g.players[0].life = 2;
    let before = format!("{g:?}");
    cite!(["601.2h", "119.4"]; refused("you can't pay 3 life with a life total of 2 (119.4)"): g.act(0, Move::Cast { card: spell, targets: vec![] }));
    assert_eq!(format!("{g:?}"), before);
    g.players[0].life = 4;
    g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
    cite!(["601.2h", "118.8"]; assert_eq!(g.players[0].life, 1));
    for _ in 0..3 {
        g.act(g.priority, Move::Pass).unwrap();
    }
    assert_eq!(g.players[0].life, 6);
}

fn game(text: &str, cost: &str) -> Game {
    let defs = vec![
        CardDef::new("Offering", cost, "Instant", text, None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Llanowar Elves", "{G}", "Creature — Elf Druid", "{T}: Add {G}.", Some((1, 1))).unwrap(),
        CardDef::new("Denial", "{0}", "Instant", "Counter target spell.", None).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    Game::new(defs, 3, 20)
}
fn pass(g: &mut Game) {
    for _ in 0..3 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn cast(
    g: &mut Game,
    card: game::ObjId,
    targets: Vec<Vec<game::Target>>,
    paying: Vec<Vec<game::ObjId>>,
) -> Result<(), String> {
    g.act(0, Move::WithCosts(paying, Box::new(Move::Cast { card, targets })))
}
#[test]
fn r601_2g_a_creature_can_produce_mana_then_pay_the_spells_sacrifice_cost() {
        let mut g = game("As an additional cost to cast this spell, sacrifice a creature.\nYou gain 3 life.", "{G}");
    let elf = g.put(0, 2, Zone::Battlefield);
    let spell = g.put(0, 0, Zone::Hand);
    let plan = vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source: elf, ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None }];
    g.act(
        0,
        Move::WithMana(
            plan,
            Box::new(Move::WithCosts(vec![vec![elf]], Box::new(paid("G",Move::Cast { card: spell, targets: vec![] })))),
        ),
    )
    .unwrap();
    cite!(["601.2g", "601.2h"]; assert_eq!(g.obj(elf).zone, Zone::Gone));
    assert_eq!(g.players[0].life, 20);
    pass(&mut g);
    assert_eq!(g.players[0].life, 23);
}
#[test]
fn r608_2b_a_target_can_be_chosen_then_sacrificed_to_pay_the_spell() {
    let mut g =
        game("As an additional cost to cast this spell, sacrifice a creature.\nDestroy target creature.", "{0}");
    let bear = g.put(0, 1, Zone::Battlefield);
    let spell = g.put(0, 0, Zone::Hand);
    cast(&mut g, spell, vec![vec![game::Target::Object(bear)]], vec![vec![bear]]).unwrap();
    assert_eq!(g.stack.len(), 1);
    pass(&mut g);
    assert!(g.stack.is_empty());
    cite!(["608.2b"]; assert_eq!(g.players[0].graveyard.len(), 2));
    cite!(["608.2b"]; assert!(g.log.iter().any(|e| matches!(e, game::Event::Fizzled { .. }))));
}
#[test]
fn r118_10_multiple_additional_costs_require_distinct_object_payments() {
        let mut g = game(
        "As an additional cost to cast this spell, sacrifice a creature.\nAs an additional cost to cast this spell, sacrifice a creature.\nYou gain 3 life.",
        "{0}",
    );
    let one = g.put(0, 1, Zone::Battlefield);
    let spell = g.put(0, 0, Zone::Hand);
    let before = format!("{g:?}");
    cite!(["118.10"]; refused("Bear pays for one thing (118.10)"): cast(&mut g, spell, vec![], vec![vec![one], vec![one]]));
    assert_eq!(format!("{g:?}"), before);
    let two = g.put(0, 1, Zone::Battlefield);
    cite!(["118.10"]; ok: cast(&mut g, spell, vec![], vec![vec![one], vec![two]]));
    assert!(g.battlefield.is_empty());
    pass(&mut g);
    assert_eq!(g.players[0].life, 23);
}
#[test]
fn r701_6b_countering_a_spell_does_not_refund_its_additional_cost() {
    let mut g = game("As an additional cost to cast this spell, sacrifice a creature.\nYou gain 3 life.", "{0}");
    let bear = g.put(0, 1, Zone::Battlefield);
    let spell = g.put(0, 0, Zone::Hand);
    cast(&mut g, spell, vec![], vec![vec![bear]]).unwrap();
    let spell = g.stack[0];
    let denial = g.put(0, 3, Zone::Hand);
    g.act(0, Move::Cast { card: denial, targets: vec![vec![game::Target::Object(spell)]] }).unwrap();
    pass(&mut g);
    cite!(["701.6b"]; assert_eq!(g.players[0].life, 20));
    assert!(g.stack.is_empty());
    cite!(["701.6b"]; assert!(g.battlefield.is_empty()));
}
#[test]
fn r702_34a_flashback_still_requires_additional_costs_and_cannot_pay_with_the_announced_spell() {
    let mut g = game(
        "As an additional cost to cast this spell, exile a card from your graveyard.\nYou gain 3 life.\nFlashback {0}",
        "{0}",
    );
    let spell = g.put(0, 0, Zone::Graveyard);
    let card = g.put(0, 4, Zone::Graveyard);
    let before = format!("{g:?}");
    cite!(["601.2a"]; refused("Offering is not what that cost asks for"): g.act(0, Move::CastVia(game::announcement::CastRoute::Flashback, Box::new(Move::WithCosts(vec![vec![spell]], Box::new(Move::Cast {card:spell,targets:vec![]}))))));
    assert_eq!(format!("{g:?}"), before);
    cite!(["702.34a", "601.2h"]; ok: g.act(0, Move::CastVia(game::announcement::CastRoute::Flashback, Box::new(Move::WithCosts(vec![vec![card]], Box::new(Move::Cast {card:spell,targets:vec![]}))))));
    assert_eq!(g.exile.len(), 1);
    pass(&mut g);
    assert_eq!(g.exile.len(), 2);
    assert_eq!(g.players[0].life, 23);
}
#[test]
fn r608_2h_an_additional_cost_binds_its_objects_last_known_power() {
    let mut g = game(
        "As an additional cost to cast this spell, sacrifice a creature.\nYou gain life equal to its power.",
        "{0}",
    );
    let bear = g.put(0, 1, Zone::Battlefield);
    g.put_counters(bear, "+1/+1", 3);
    let spell = g.put(0, 0, Zone::Hand);
    cast(&mut g, spell, vec![], vec![vec![bear]]).unwrap();
    pass(&mut g);
    cite!(["608.2h"]; assert_eq!(g.players[0].life, 25));
}
#[test]
fn r118_8d_an_additional_cost_does_not_change_the_spells_mana_value() {
    let mut g = game("As an additional cost to cast this spell, pay 3 life.\nYou gain 5 life.", "{1}");
    g.players[0].pool.colorless = 1;
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0,paid("C",Move::Cast {card:spell,targets:vec![]})).unwrap();
    cite!(["118.8d"]; assert_eq!(g.obj(g.stack[0]).chars.mana_value, 1));
    assert_eq!(g.players[0].life, 17);
}
/// Optional additional costs are r118_8b_optional_additional_costs's.
#[test]
fn r601_2f_variable_additional_costs_remain_refused() {
    let text = "As an additional cost to cast this spell, pay life equal to your life total.\nYou gain 3 life.";
    let d = CardDef::new("Unknown", "{0}", "Instant", text, None).unwrap();
    // Uncited (admission): this variable life cost has no supported locking procedure.
    assert_eq!(game::play::check(&d).expect_err("variable life remains unsupported").0,
        "additional cost: a variable amount must be locked before mana production (601.2f)");
}

#[test]
fn r608_2h_participial_cost_reference_is_independent_of_the_target() {
    let d=CardDef::new("Fling","{1}{R}","Instant","As an additional cost to cast this spell, sacrifice a creature.\nFling deals damage equal to the sacrificed creature's power to any target.",None).unwrap();
    game::play::check(&d).unwrap();
    let creature=CardDef::new("Giant","{4}{G}","Creature — Giant","",Some((5,5))).unwrap();
    let mut g=Game::new(vec![d,creature],2,20);
    let sacrificed=g.put(0,1,Zone::Battlefield);let spell=g.put(0,0,Zone::Hand);
    g.players[0].pool.colors[3]=2;
    g.act(0,paid("RR", Move::WithCosts(vec![vec![sacrificed]],Box::new(Move::Cast{card:spell,targets:vec![vec![game::Target::Player(1)]]})))).unwrap();
    assert!(!g.battlefield.contains(&sacrificed));
    let cloned=g.clone();for mut branch in [g,cloned] {pass(&mut branch);cite!(["608.2h"]; assert_eq!(branch.players[1].life,15));}
    let ambiguous=CardDef::new("Unbound","{0}","Artifact","Sacrifice a creature, Sacrifice a creature: You gain life equal to the sacrificed creature's power.",None).unwrap();
    assert_eq!(game::play::check(&ambiguous).expect_err("two payments cannot bind one participle").0, "input reference: ambiguous paid-action producers");
    let absent=CardDef::new("Unbound","{0}","Instant","You gain life equal to the sacrificed creature's power.",None).unwrap();
    assert_eq!(game::play::check(&absent).expect_err("no sacrifice supplies the participant").0, "input reference: nothing before it supplies the required participant/cardinality");
}

#[test]
fn r400_7j_a_spell_finds_every_card_in_a_plural_additional_payment() {
    let mut g = game(
        "As an additional cost to cast this spell, exile two creature cards from your graveyard.\nReturn them to the battlefield.",
        "{0}",
    );
    let cards = vec![g.put(0, 1, Zone::Graveyard), g.put(0, 1, Zone::Graveyard)];
    let spell = g.put(0, 0, Zone::Hand);
    cast(&mut g, spell, vec![], vec![cards]).unwrap();
    assert_eq!(g.exile.len(), 2);
    pass(&mut g);
    assert!(g.exile.is_empty());
    cite!(["400.7j"]; assert_eq!(g.battlefield.len(), 2));
}

#[test]
fn r118_8_mandatory_additional_mana_is_included_in_total_payment() {
    let mut g = game("As an additional cost to cast this spell, pay {2}.\nYou gain 3 life.", "{1}");
    let spell = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colorless = 2;
    // {1} and the additional {2} are one total cost, {3}: exact, so no part of it goes unshown
    cite!(["118.8", "601.2f"]; assert_eq!(cast(&mut g, spell, vec![], vec![]), Err("your mana pool cannot pay that cost (118.3a): {3}".into())));
    assert_eq!(g.players[0].pool.total(), 2);
    cite!(["601.2h"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0,paid("CC",Move::Cast {card:spell,targets:vec![]})));
    g.players[0].pool.colorless = 3;
    g.act(0,paid("CCC",Move::Cast {card:spell,targets:vec![]})).unwrap();
    cite!(["118.8", "601.2f"]; assert_eq!(g.players[0].pool.total(), 0));
    cite!(["118.8d"]; assert_eq!(g.obj(g.stack[0]).chars.mana_value, 1));
}
#[test]
fn r113_6d_a_permanents_additional_cost_is_paid_while_casting_not_on_entry() {
    let defs = vec![
        CardDef::new(
            "Offering",
            "{0}",
            "Creature — Spirit",
            "As an additional cost to cast this spell, exile three creature cards from your graveyard.\nFlying",
            Some((5, 6)),
        )
        .unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap();
    }
    let mut g = Game::new(defs, 3, 20);
    let cards = (0..3).map(|_| g.put(0, 1, Zone::Graveyard)).collect();
    let card = g.put(0, 0, Zone::Hand);
    cast(&mut g, card, vec![], vec![cards]).unwrap();
    cite!(["113.6d", "601.2h"]; assert_eq!(g.exile.len(), 3));
    assert!(g.battlefield.is_empty());
    pass(&mut g);
    assert_eq!(g.exile.len(), 3);
    cite!(["113.6d"]; assert_eq!(g.battlefield.len(), 1));
    assert_eq!(g.power(g.battlefield[0]), Some(5));
    assert!(g.has(g.battlefield[0], "Flying"));
}
