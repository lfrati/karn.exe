//! Improvise taps explicit controlled artifacts to pay locked generic mana (702.126).
use crate::support::{cite, mana_pool, oracle, paid};
use game::{CardDef, Game, Move, ObjId, Zone};
use game::state::{Answer, Choice, Decision};
fn cast(card: ObjId, artifacts: &[ObjId]) -> Move {
    Move::WithImprovise(artifacts.len() as u32, Box::new(Move::WithCosts(
        if artifacts.is_empty() { vec![] } else { vec![artifacts.to_vec()] }, Box::new(Move::Cast { card, targets: vec![] }))))
}
fn pass(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }
#[test]
fn r702_126a_fen_hauler_uses_only_controlled_untapped_artifacts_and_cannot_pay_colored_mana() {
    let mut g = Game::new(vec![oracle("Fen Hauler"), oracle("Darksteel Relic"), oracle("Grizzly Bears")], 2, 20);
    let card = g.put(0, 0, Zone::Hand);
    let artifacts: Vec<_> = (0..7).map(|_| g.put(0, 1, Zone::Battlefield)).collect();
    let foreign = g.put(1, 1, Zone::Battlefield);
    let nonartifact = g.put(0, 2, Zone::Battlefield);
    cite!(["702.126a"]; refused("improvise taps at most 6 artifact(s)"): g.act(0, cast(card, &artifacts)));
    cite!(["702.126a"]; refused("your mana pool cannot pay that cost"): g.act(0, cast(card, &artifacts[..6])));
    g.players[0].pool = mana_pool("B");
    for wrong in [foreign, nonartifact] {
        let mut chosen = artifacts[..6].to_vec(); chosen[0] = wrong;
        cite!(["702.126a", "118.3"]; refused("is not what that cost asks for or cannot pay it"): g.act(0, paid("B", cast(card, &chosen))));
    }
    let mut duplicate = artifacts[..6].to_vec(); duplicate[0] = duplicate[1];
    cite!(["702.126a", "118.10"]; refused("a card is chosen once for that cost"): g.act(0, paid("B", cast(card, &duplicate))));
    g.tap(artifacts[6]);
    let mut tapped = artifacts[..6].to_vec(); tapped[0] = artifacts[6];
    cite!(["702.126a"]; refused("is not what that cost asks for or cannot pay it"): g.act(0, paid("B", cast(card, &tapped))));
    g.act(0, paid("B", cast(card, &artifacts[..6]))).unwrap();
    cite!(["702.126a"]; assert!(artifacts[..6].iter().all(|o| g.obj(*o).tapped)));
    assert_eq!(g.players[0].pool.total(), 0);
    pass(&mut g);
    assert!(g.battlefield.iter().any(|o| g.obj(*o).card == Some(0)));
}
#[test]
fn r702_126b_whir_uses_announced_x_without_reducing_the_effects_x() {
    let mut g = Game::new(vec![oracle("Whir of Invention"), oracle("Darksteel Relic"), oracle("Sol Ring")], 2, 20);
    let card = g.put(0, 0, Zone::Hand);
    let artifacts = [g.put(0, 1, Zone::Battlefield), g.put(0, 1, Zone::Battlefield)];
    let found = g.put(0, 2, Zone::Library);
    g.players[0].pool = mana_pool("UUU");
    g.act(0, paid("UUU", Move::WithX(2, Box::new(cast(card, &artifacts))))).unwrap(); pass(&mut g);
    let Some(Decision::Choose { choice: Choice::Search { among, .. }, .. }) = &g.decision else { panic!("{:?}", g.decision) };
    cite!(["702.126b", "107.3"]; assert_eq!(among, &vec![found]));
    g.act(0, Move::Choose(Answer::Objects(vec![found]))).unwrap();
    assert!(artifacts.iter().all(|o| g.obj(*o).tapped));
    assert!(g.battlefield.iter().any(|o| g.obj(*o).card == Some(2)));
}
#[test]
fn r702_126b_cost_reductions_and_the_final_minimum_are_fixed_before_improvise() {
    let mut defs = vec![oracle("Darksteel Relic"), oracle("Trinisphere")];
    let affinity = CardDef::new("Discounted improvisation", "{6}", "Instant", "Affinity for artifacts\nImprovise\nYou gain 1 life.", None).unwrap();
    let minimum = CardDef::new("Minimum improvisation", "{0}", "Instant", "Improvise\nYou gain 1 life.", None).unwrap();
    game::play::check(&affinity).unwrap(); game::play::check(&minimum).unwrap();
    defs.extend([affinity, minimum]);
    for (index, copies, generic) in [(2, 2, 2), (3, 3, 0)] {
        let mut g = Game::new(defs.clone(), 2, 20);
        let card = g.put(0, index, Zone::Hand);
        let artifacts: Vec<_> = (0..copies).map(|_| g.put(0, 0, Zone::Battlefield)).collect();
        if index == 3 { g.put(0, 1, Zone::Battlefield); }
        g.players[0].pool.colorless = generic;
        g.act(0, paid(if generic == 0 { "" } else { "CC" }, cast(card, &artifacts))).unwrap();
        cite!(["702.126b", "601.2f"]; assert!(artifacts.iter().all(|o| g.obj(*o).tapped)));
        assert_eq!(g.players[0].pool.total(), 0);
    }
}
#[test]
fn r702_126a_a_granting_artifact_can_itself_help_pay_for_its_granted_improvise() {
    // Uncited (fixture): the controller-only grant isolates existing stack keyword grants;
    // Inspiring Statuary's distinct "you cast" predicate remains explicitly unsupported below.
    let grant = CardDef::new("Controlled improvisation", "{3}", "Artifact", "Nonartifact spells you control have improvise.", None).unwrap();
    game::play::check(&grant).unwrap();
    let mut g = Game::new(vec![oracle("Hill Giant"), grant, oracle("Darksteel Relic")], 2, 20);
    let card = g.put(0, 0, Zone::Hand);
    let artifacts = [g.put(0, 1, Zone::Battlefield), g.put(0, 2, Zone::Battlefield), g.put(0, 2, Zone::Battlefield)];
    g.players[0].pool = mana_pool("R");
    g.act(0, paid("R", cast(card, &artifacts))).unwrap();
    cite!(["702.126a", "613.1f"]; assert!(artifacts.iter().all(|o| g.obj(*o).tapped)));
}
#[test]
fn r118_10_convoke_and_improvise_use_separate_tap_resources() {
    let spell = CardDef::new("Two substitutions", "{2}{G}", "Instant", "Convoke\nImprovise\nYou gain 1 life.", None).unwrap();
    game::play::check(&spell).unwrap();
    let mut g = Game::new(vec![spell, oracle("Memnite"), oracle("Darksteel Relic")], 2, 20);
    let card = g.put(0, 0, Zone::Hand); let creature = g.put(0, 1, Zone::Battlefield); let artifact = g.put(0, 2, Zone::Battlefield);
    g.players[0].pool = mana_pool("G");
    let both = |improviser| paid("G", Move::WithConvoke(vec![None], Box::new(Move::WithImprovise(1,
        Box::new(Move::WithCosts(vec![vec![creature], vec![improviser]], Box::new(Move::Cast { card, targets: vec![] })))))));
    cite!(["118.10", "702.126a"]; refused("Memnite pays for one thing"): g.act(0, both(creature)));
    g.act(0, both(artifact)).unwrap();
    cite!(["118.10", "702.126a"]; assert!(g.obj(creature).tapped && g.obj(artifact).tapped));
}
#[test]
fn improvise_zone_costs_preserve_the_independent_resource_boundary() {
    // Uncited (admission): the tap/resource owner still refuses mixed zone-changing costs.
    let def = CardDef::new("Unsupported mixed improvisation", "{B}", "Instant", "Improvise\nAs an additional cost to cast this spell, sacrifice an artifact.\nYou gain 1 life.", None).unwrap();
    crate::support::refused(game::play::check(&def).map_err(|e| e.0), "improvise combined with a zone-changing object cost requires independent resource kinds");
}

#[test]
fn inspiring_statuary_keeps_its_distinct_caster_predicate_refusal() {
    // Uncited (admission): controlling a spell is not proof of casting it (707.10).
    let def = crate::support::oracle_read("Inspiring Statuary");
    crate::support::refused(game::play::check(&def).map_err(|e| e.0), "noun phrase: a caster");
}
