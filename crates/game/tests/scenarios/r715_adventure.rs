//! Alternative characteristics and the resolving Adventure's scoped permission (715).
use crate::support::{cite, paid};
use game::card::CastAs;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
fn adventurer(text: &str) -> CardDef {
    let normal = CardDef::new("Adventurer", "{6}{G}", "Creature — Giant", "", Some((7, 7))).unwrap();
    let alternative = CardDef::new("Journey", "{2}{G}", "Sorcery — Adventure", text, None).unwrap();
    let def = CardDef::adventurer(normal, alternative).unwrap();
    game::play::check(&def).unwrap();
    def
}
fn setup(text: &str) -> Game {
    let defs = vec![
        adventurer(text),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Counterspell", "{U}{U}", "Instant", "Counter target spell.", None).unwrap(),
    ];
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 {
        for _ in 0..10 {
            g.put(p, 1, Zone::Library);
        }
    }
    g
}
fn cast(card: ObjId, cast_as: CastAs, targets: Vec<Vec<Target>>, units: &str) -> Move {
    paid(units, Move::CastAs(cast_as, Box::new(Move::Cast { card, targets })))
}
fn resolve(g: &mut Game) {
    for _ in 0..2 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}

#[test]
fn r715_3_only_the_chosen_characteristics_apply_and_resolution_restores_the_card() {

    let mut g = setup("You gain 3 life.");
    let card = g.put(0, 0, Zone::Hand);
    assert_eq!(g.obj(card).chars.mana_value, 7);
    assert_eq!(g.def(card).unwrap().name(), "Adventurer");
    g.players[0].pool.colors[4] = 1;
    g.players[0].pool.colorless = 2;
    cite!(["715.3a", "601.2h"]; refused("the supplied mana does not pay that cost, all of it and no more"): unchanged(&mut g,cast(card, CastAs::Whole, vec![], "CCG")));
    g.act(0, cast(card, CastAs::Adventure, vec![], "CCG")).unwrap();
    let spell = g.stack[0];
    assert_eq!(g.def(spell).unwrap().name(), "Journey");
    cite!(["715.3b"]; assert_eq!(g.obj(spell).chars.mana_value, 3));
    assert!(g.seen(spell, None).is(ir::CardType::Sorcery));
    cite!(["715.3b"]; assert!(!g.seen(spell, None).is(ir::CardType::Creature)));
    resolve(&mut g);
    assert_eq!(g.players[0].life, 23);
    let exiled = g.exile[0];
    cite!(["715.4"]; assert_eq!(g.obj(exiled).chars.mana_value, 7));
    assert_eq!(g.def(exiled).unwrap().name(), "Adventurer");
    cite!(["715.3d"]; assert_eq!(g.permission(exiled), Some((0, true))));
    g.players[0].pool.colors[4] = 1;
    g.players[0].pool.colorless = 6;
    cite!(["715.3d"]; refused("it was exiled on an adventure: it may be cast as its creature or other main card, not as its Adventure (715.3d)"): unchanged(&mut g,cast(exiled, CastAs::Adventure, vec![], "CCG")));
    g.act(0, cast(exiled, CastAs::Whole, vec![], "CCCCCCG")).unwrap();
    resolve(&mut g);
    assert_eq!(g.battlefield.len(), 1);
    assert_eq!(g.seen(g.battlefield[0], None).pt, Some((7, 7)));
    assert!(g.exile.is_empty());
}
#[test]
fn r715_3d_permission_survives_turns_and_clones_but_not_another_incarnation() {
    let mut g = setup("You gain 3 life.");
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[4] = 1;
    g.players[0].pool.colorless = 2;
    g.act(0, cast(card, CastAs::Adventure, vec![], "CCG")).unwrap();
    resolve(&mut g);
    let exiled = g.exile[0];
    let mut copy = g.clone();
    while g.active == 0 {
        g.act(g.priority, Move::Pass).unwrap();
    }
    while g.active == 1 {
        g.act(g.priority, Move::Pass).unwrap();
    }
    cite!(["715.3d"]; assert_eq!(g.permission(exiled), Some((0, true))));
    g.move_object(exiled, Zone::Hand);
    let hand = *g.players[0].hand.last().unwrap();
    g.move_object(hand, Zone::Exile);
    let other = *g.exile.last().unwrap();
    cite!(["400.7"]; assert_ne!(other, exiled));
    cite!(["715.3d"]; assert_eq!(g.permission(other), None));
    copy.players[0].pool.colors[4] = 1;
    copy.players[0].pool.colorless = 6;
    copy.act(0, cast(exiled, CastAs::Whole, vec![], "CCCCCCG")).unwrap();
    resolve(&mut copy);
    assert_eq!(copy.battlefield.len(), 1);
}
#[test]
fn r715_3d_countering_or_losing_all_targets_does_not_grant_permission() {
    {
        let mut g = setup("Target player gains 3 life.");
        let card = g.put(0, 0, Zone::Hand);
        g.players[0].pool.colors[4] = 1;
        g.players[0].pool.colorless = 2;
        g.act(0, cast(card, CastAs::Adventure, vec![vec![Target::Player(0)]], "CCG")).unwrap();
        {
            let spell = g.stack[0];
            let answer = g.put(1, 2, Zone::Hand);
            g.players[1].pool.colors[1] = 2;
            g.act(0, Move::Pass).unwrap();
            g.act(1, paid("UU", Move::Cast { card: answer, targets: vec![vec![Target::Object(spell)]] })).unwrap();
            resolve(&mut g);
        }
        cite!(["701.6a"]; assert!(g.exile.is_empty()));
        cite!(["701.6a"]; assert_eq!(g.players[0].graveyard.len(), 1));
        assert_eq!(g.def(g.players[0].graveyard[0]).unwrap().name(), "Adventurer");
    }
    let normal = CardDef::new("Oak", "{3}{G}", "Creature — Treefolk", "", Some((3, 3))).unwrap();
    let alt = CardDef::new("Boon", "{G}", "Instant — Adventure", "Target creature gets +2/+2 until end of turn.", None)
        .unwrap();
    let def = CardDef::adventurer(normal, alt).unwrap();
    game::play::check(&def).unwrap();
    let mut g = Game::new(vec![def], 2, 20);
    let target = g.put(1, 0, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[4] = 1;
    g.act(0, cast(card, CastAs::Adventure, vec![vec![Target::Object(target)]], "G")).unwrap();
    g.move_object(target, Zone::Graveyard);
    resolve(&mut g);
    cite!(["608.2b"]; assert!(g.exile.is_empty()));
    cite!(["608.2b"]; assert_eq!(g.players[0].graveyard.len(), 1));
}
#[test]
fn r715_3_normal_cast_neither_runs_adventure_nor_exiles_the_permanent() {
    let mut g = setup("You gain 3 life.");
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[4] = 1;
    g.players[0].pool.colorless = 6;
    g.act(0, cast(card, CastAs::Whole, vec![], "CCCCCCG")).unwrap();
    resolve(&mut g);
    cite!(["715.4"]; assert_eq!(g.players[0].life, 20));
    assert_eq!(g.battlefield.len(), 1);
    let permanent = g.battlefield[0];
    g.move_object(permanent, Zone::Graveyard);
    cite!(["715.3d"]; assert!(g.exile.is_empty()));
    assert_eq!(g.players[0].graveyard.len(), 1);
}
#[test]
fn r715_3a_instant_adventure_has_its_own_timing() {
    let normal = CardDef::new("Giant", "{3}{G}", "Creature — Giant", "", Some((3, 3))).unwrap();
    let alt = CardDef::new("Help", "{G}", "Instant — Adventure", "You gain 1 life.", None).unwrap();
    let mut g = Game::new(vec![CardDef::adventurer(normal, alt).unwrap()], 2, 20);
    let first = g.put(0, 0, Zone::Hand);
    let second = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[4] = 2;
    g.players[0].pool.colorless = 6;
    g.act(0, cast(first, CastAs::Whole, vec![], "CCCG")).unwrap();
    g.act(0, cast(second, CastAs::Adventure, vec![], "G")).unwrap();
    resolve(&mut g);
    assert_eq!(g.stack.len(), 1);
    assert_eq!(g.players[0].life, 21);
}
#[test]
fn r715_3d_the_caster_keeps_permission_for_an_opponents_card_and_other_permissions_allow_adventure() {
    let access = CardDef::new(
        "Access",
        "{0}",
        "Sorcery",
        "Exile target card from a graveyard. You may cast it this turn.",
        None,
    )
    .unwrap();
    game::play::check(&access).unwrap();
    let mut g = Game::new(vec![adventurer("You gain 3 life."), access], 2, 20);
    let card = g.put(1, 0, Zone::Graveyard);
    let access = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card: access, targets: vec![vec![Target::Object(card)]] }).unwrap();
    resolve(&mut g);
    let card = g.exile[0];
    g.players[0].pool.colors[4] = 1;
    g.players[0].pool.colorless = 2;

    g.act(0, cast(card, CastAs::Adventure, vec![], "CCG")).unwrap();
    resolve(&mut g);
    let card = g.exile[0];
    assert_eq!(g.obj(card).owner, 1);
    cite!(["715.3d"]; assert_eq!(g.permission(card), Some((0, true))));
    g.players[0].pool.colors[4] = 1;
    g.players[0].pool.colorless = 6;
    g.act(0, cast(card, CastAs::Whole, vec![], "CCCCCCG")).unwrap();
    resolve(&mut g);
    assert_eq!(g.controller(g.battlefield[0]), 0);
    assert_eq!(g.obj(g.battlefield[0]).owner, 1);
}
#[test]
fn r715_2b_copied_permanents_retain_alternative_characteristics_and_the_query_counts_them() {
    let copy =
        CardDef::new("Duplicate", "{0}", "Instant", "Create a token that's a copy of target creature.", None).unwrap();
    let count = CardDef::new(
        "Measure",
        "{0}",
        "Instant",
        "You gain 1 life for each creature you control that has an Adventure.",
        None,
    )
    .unwrap();
    for d in [&copy, &count] {
        game::play::check(d).unwrap();
    }
    let mut g = Game::new(vec![adventurer("You gain 3 life."), copy, count], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let duplicate = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card: duplicate, targets: vec![vec![Target::Object(source)]] }).unwrap();
    resolve(&mut g);
    let token = *g.battlefield.last().unwrap();
    assert!(g.obj(token).token());
    cite!(["715.2b"]; assert!(g.def(token).unwrap().adventure().is_some()));
    let measure = g.put(0, 2, Zone::Hand);
    g.act(0, Move::Cast { card: measure, targets: vec![] }).unwrap();
    resolve(&mut g);
    cite!(["715.2a"]; assert_eq!(g.players[0].life, 22));
}
#[test]
fn r715_3d_resolution_keeps_the_adventure_on_stack_until_its_last_decision() {
    use game::state::Answer;
    let mut g = setup("Scry 2. You gain 3 life.");
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[4] = 1;
    g.players[0].pool.colorless = 2;
    g.act(0, cast(card, CastAs::Adventure, vec![], "CCG")).unwrap();
    resolve(&mut g);
    cite!(["608.2n"]; assert_eq!(g.stack.len(), 1));
    assert!(g.exile.is_empty());
    assert_eq!(g.players[0].life, 20);
    crate::support::refused(g.act(0, Move::Pass), "a decision is pending: a choice of the effect being applied (608.2d)");
    let mut copy = g.clone();
    for g in [&mut g, &mut copy] {
        let choice = g.decision.as_ref().unwrap();
        let game::state::Decision::Choose { choice: game::state::Choice::Arrange { cards, .. }, .. } = choice else {
            panic!("{choice:?}")
        };
        let cards = cards.clone();
        g.act(0, Move::Choose(Answer::Arrange { top: cards, away: vec![] })).unwrap();
        assert!(g.stack.is_empty());
        cite!(["715.3d"]; assert_eq!(g.exile.len(), 1));
        assert_eq!(g.players[0].life, 23);
        assert_eq!(g.permission(g.exile[0]), Some((0, true)));
    }
}
#[test]
fn r720_3d_omen_is_not_an_adventure_despite_sharing_the_corpus_layout() {
    let omen =
        CardDef::new("Coil and Catch", "{3}{G}", "Instant — Omen", "Draw three cards, then discard a card.", None)
            .unwrap();
    assert!(game::play::check(&omen).unwrap_err().0.contains("720.3d"));
    let normal = CardDef::new("Dragon", "{4}{G}{G}", "Creature — Dragon", "Flying", Some((6, 7))).unwrap();
    assert!(CardDef::adventurer(normal, omen).unwrap_err().contains("720.2"));
}

#[test]
fn r715_3d_hearth_elemental_discards_then_draws_and_its_normal_spell_counts_the_graveyard_union_once() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let hearth = CardDef::from_faces(&catalog["Hearth Elemental"]).unwrap();
    game::play::check(&hearth).unwrap();
    assert_eq!(hearth.name(), "Hearth Elemental");
    assert_eq!(hearth.adventure().unwrap().name(), "Stoke Genius");
    assert_eq!(CardDef::from_faces(&catalog["Stoke Genius"]).unwrap().name(), "Hearth Elemental");
    let forest = CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap();
    let instant = CardDef::new("Shock", "{R}", "Instant", "Shock deals 2 damage to any target.", None).unwrap();
    let sorcery = CardDef::new("Gift", "{0}", "Sorcery", "You gain 1 life.", None).unwrap();
    let both = CardDef::adventurer(
        sorcery,
        CardDef::new("Trip", "{0}", "Instant — Adventure", "You gain 1 life.", None).unwrap(),
    )
    .unwrap();
    let mut g = Game::new(vec![hearth, forest, instant, both], 2, 20);
    let hearth = g.put(0, 0, Zone::Hand);
    // Three distinct cards satisfy the union; the sorcery/Adventure counts only once.
    let discarded: Vec<_> = [0, 2, 3].map(|def| g.put(0,def,Zone::Hand)).to_vec();
    for _ in 0..4 {
        g.put(0, 1, Zone::Library);
    }
    g.players[0].pool.colors[3] = 1;
    g.players[0].pool.colorless = 1;
    g.act(0, cast(hearth, CastAs::Adventure, vec![], "CR")).unwrap();
    resolve(&mut g);
    // CR404.3: the owner explicitly orders these simultaneous graveyard arrivals.
    assert_eq!(g.decision, Some(game::state::Decision::Choose { player: 0, choice: game::state::Choice::OrderCards { to: game::state::Placement::Zone(Zone::Graveyard), cards: discarded.clone() } }));
    g.act(0, Move::Choose(game::state::Answer::Objects(vec![discarded[2],discarded[0],discarded[1]]))).unwrap();
    assert_eq!(g.players[0].hand.len(), 2);
    assert_eq!(g.players[0].graveyard.len(), 3);
    assert_eq!(g.players[0].library.len(), 2);
    cite!(["715.3d"]; assert_eq!(g.exile.len(), 1));
    let hearth = g.exile[0];
    g.players[0].pool.colors[3] = 1;
    g.players[0].pool.colorless = 1;
    g.players[0].pool.colorless = 2;
    g.act(0, cast(hearth, CastAs::Whole, vec![], "CCR")).unwrap();
    assert_eq!(g.players[0].pool.total(), 0);
    resolve(&mut g);
    assert_eq!(g.seen(g.battlefield[0], None).pt, Some((4, 5)));
}

#[track_caller]
fn unchanged(g: &mut Game, action: Move) -> Result<(), String> {
    let before=format!("{g:?}"); let result=g.act(0,action);
    assert_eq!(format!("{g:?}"),before); result
}
