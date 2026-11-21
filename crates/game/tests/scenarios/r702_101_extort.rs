//! Extort (702.101): "Whenever you cast a spell, you may pay {W/B}. If you do, each opponent
//! loses 1 life and you gain life equal to the total life lost this way." The keyword is that
//! triggered ability, paid as any effect's payment is (118.12).
use crate::support::cite;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Zone};
use ir::mana::ManaSymbol;
use ir::Color;

const OBEDIENCE: usize = 0;
const BEAR: usize = 1;
const PLAINS: usize = 2;
const SWAMP: usize = 3;
const TWICE: usize = 4;

fn game(players: usize) -> Game {
    let card = |name: &str, cost: &str, types: &str, text: &str, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    let defs = vec![
        card("Blind Obedience", "{1}{W}", "Enchantment", "Extort\nArtifacts and creatures your opponents control enter tapped.", None),
        card("Bear", "{0}", "Creature — Bear", "", Some((2, 2))),
        card("Plains", "", "Basic Land — Plains", "", None),
        card("Swamp", "", "Basic Land — Swamp", "", None),
        card("Twice", "{2}", "Creature — Spirit", "Extort\nExtort", Some((1, 1))),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    Game::new(defs, players, 20)
}
fn pass_all(g: &mut Game) {
    for _ in 0..g.players.iter().filter(|p| !p.lost).count() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn cast_bear(g: &mut Game, player: usize) -> ObjId {
    let bear = g.put(player, BEAR, Zone::Hand);
    g.act(player, Move::Cast { card: bear, targets: vec![] }).unwrap();
    bear
}
fn life(g: &Game) -> Vec<i32> { g.players.iter().map(|p| p.life).collect() }

#[test]
fn r702_101a_extort_is_paid_with_either_half_and_drains_each_opponent() {
    for (land, units) in [(PLAINS, "W"), (SWAMP, "B")] {
        let mut g = game(3);
        g.put(0, OBEDIENCE, Zone::Battlefield);
        let land = g.put(0, land, Zone::Battlefield);
        cast_bear(&mut g, 0);
        cite!(["702.101a"]; case: format!("{land:?}"); assert_eq!(g.stack.len(), 2, "the spell, and above it extort"));
        pass_all(&mut g);
        let Some(Decision::Choose { player: 0, choice: Choice::Pay { cost, life: 0, .. } }) = g.decision.clone() else { panic!("extort asks: {:?}", g.decision) };
        cite!(["702.101a"]; case: format!("{land:?}"); assert_eq!(cost.0, vec![ManaSymbol::Hybrid(Color::White, Color::Black)]));
        let plan = vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source: land, ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None }];
        g.act(0, Move::Choose(Answer::Pay { plan, with: Some(crate::support::mana_pool(units)), tapped: vec![] })).unwrap();
        cite!(["118.12"]; case: format!("{land:?}"); assert!(g.obj(land).tapped));
        cite!(["702.101a"]; case: format!("{land:?}"); assert_eq!(life(&g), vec![22, 19, 19], "two opponents lose 1 each; that total is gained"));
        assert_eq!(g.stack.len(), 1, "the spell is still to resolve");
    }
}

#[test]
fn r702_101a_declined_or_unpayable_it_does_nothing() {
    let mut g = game(2);
    g.put(0, OBEDIENCE, Zone::Battlefield);
    g.put(0, PLAINS, Zone::Battlefield);
    cast_bear(&mut g, 0);
    pass_all(&mut g);
    g.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["702.101a"]; assert!(g.decision.is_none() && g.resolving().is_none()));
    cite!(["702.101a"]; assert_eq!(life(&g), vec![20, 20]));

    // Nothing to pay with: an explicit payment fails; declining leaves the effect unpaid.
    let mut g = game(2);
    g.put(0, OBEDIENCE, Zone::Battlefield);
    cast_bear(&mut g, 0);
    assert_eq!(g.stack.len(), 2);
    pass_all(&mut g);
    cite!(["118.3a"]; refused("your mana pool cannot pay that cost"): unchanged(&mut g, |g| g.act(0, Move::Choose(Answer::Pay { plan: vec![], with: None, tapped: vec![] }))));
    g.act(0, Move::Choose(Answer::No)).unwrap();
    assert!(g.decision.is_none());
    cite!(["702.101a"]; assert_eq!((life(&g), g.stack.len()), (vec![20, 20], 1)));
}

#[test]
fn r702_101a_only_its_controllers_spells_trigger_it() {
    let mut g = game(2);
    g.put(1, OBEDIENCE, Zone::Battlefield);
    g.put(1, PLAINS, Zone::Battlefield);
    cast_bear(&mut g, 0);
    cite!(["702.101a"]; assert_eq!(g.stack.len(), 1, "an opponent's spell is not extorted"));
    pass_all(&mut g);
    assert_eq!(life(&g), vec![20, 20]);
    // and their creature enters tapped
    let entered = g.battlefield.iter().copied().find(|o| g.obj(*o).card == Some(BEAR));
    cite!(["614.1d"]; assert!(entered.is_some_and(|o| g.obj(o).tapped)));
}

#[test]
fn r702_101b_each_instance_triggers_and_is_paid_separately() {
    let mut g = game(2);
    g.put(0, TWICE, Zone::Battlefield);
    g.players[0].pool.colors[0] = 1;
    g.players[0].pool.colors[2] = 1;
    cast_bear(&mut g, 0);
    if g.decision.is_some() { g.act(0, Move::Order(vec![0, 1])).unwrap(); }
    cite!(["702.101b"]; assert_eq!(g.stack.len(), 3, "two extort triggers"));
    pass_all(&mut g);
    g.act(0, Move::Choose(Answer::Pay { plan: vec![], with: Some(crate::support::mana_pool("W")), tapped: vec![] })).unwrap();
    cite!(["702.101b"]; assert_eq!(life(&g), vec![21, 19]));
    pass_all(&mut g);
    g.act(0, Move::Choose(Answer::Pay { plan: vec![], with: Some(crate::support::mana_pool("B")), tapped: vec![] })).unwrap();
    cite!(["702.101b"]; assert_eq!((life(&g), g.players[0].pool.total()), (vec![22, 18], 0)));
}

#[test]
fn r608_2h_life_lost_result_is_bound_once_and_survives_an_unrelated_choice() {
    let d=CardDef::new("Drain", "{0}", "Sorcery", "Each opponent loses 2 life. You may draw a card. You gain life equal to the life lost this way.", None).unwrap();
    game::play::check(&d).unwrap();
    let mut g=Game::new(vec![d],3,20);
    let spell=g.put(0,0,Zone::Hand);
    g.put(0,0,Zone::Library);
    g.act(0,Move::Cast{card:spell,targets:vec![]}).unwrap();
    for _ in 0..3 { g.act(g.priority,Move::Pass).unwrap(); }
    let copy=g.clone();
    for (mut g,answer) in [(g,Answer::Yes),(copy,Answer::No)] {
        g.act(0,Move::Choose(answer)).unwrap();
        cite!(["608.2c"]; assert_eq!(life(&g),vec![24,18,18]));
    }
}

#[test]
fn r608_2h_life_lost_reference_requires_one_identified_producer() {
    for text in ["You gain life equal to the life lost this way.", "You lose 1 life. Each opponent loses 2 life. You gain life equal to the life lost this way."] {
        let d=CardDef::new("Unbound", "{0}", "Sorcery",text,None).unwrap();
        assert!(game::play::check(&d).unwrap_err().0.contains("result producer"));
    }
}

#[track_caller]
fn unchanged(g: &mut Game, action: impl FnOnce(&mut Game) -> Result<(), String>) -> Result<(), String> {
    let before=format!("{g:?}");
    let result=action(g);
    assert_eq!(format!("{g:?}"),before);
    result
}
