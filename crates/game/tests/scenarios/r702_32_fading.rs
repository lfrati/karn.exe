//! Fading sacrifices only when its mandatory upkeep removal cannot be paid, not on the last counter.
use crate::support::{cite, mana_pool, oracle, paid};
use game::{CardDef, Game, Move, ObjId, Zone};
use game::state::Decision;
use ir::Part;

fn resolve(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
fn game() -> Game {
    let mut g = Game::new(["Blastoderm", "Ancient Hydra", "Forest", "Cloudshift"].into_iter().map(oracle).collect::<Vec<_>>(), 2, 20);
    for player in 0..2 { for _ in 0..12 { g.put(player, 2, Zone::Library); } }
    g
}
fn cast(g: &mut Game, index: usize, mana: &str) -> ObjId {
    let card = g.put(0, index, Zone::Hand);
    g.players[0].pool = mana_pool(mana);
    g.act(0, paid(mana, Move::Cast { card, targets: vec![] })).unwrap();
    resolve(g);
    *g.battlefield.iter().find(|o| g.obj(**o).card == Some(index)).unwrap()
}
fn upkeep(g: &mut Game) {
    let turn = g.turn;
    for _ in 0..200 {
        if g.turn > turn && g.active == 0 && g.part == Part::Upkeep { return; }
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            None => g.act(g.priority, Move::Pass).unwrap(),
            other => panic!("unexpected question {other:?}"),
        }
    }
    panic!("no upkeep");
}

#[test]
fn r702_32a_blastoderm_lives_through_its_last_removal_then_dies_on_the_next_upkeep() {
    let mut g = game();
    let source = cast(&mut g, 0, "GGGG");
    cite!(["702.32a"]; assert_eq!(g.counters(source).get("fade"), Some(&3)));
    for left in [2, 1, 0] {
        upkeep(&mut g);
        assert_eq!(g.stack.len(), 1);
        resolve(&mut g);
        cite!(["702.32a"]; assert!(g.battlefield.contains(&source) && g.stack.is_empty()));
        assert_eq!(g.counters(source).get("fade").copied().unwrap_or(0), left);
    }
    upkeep(&mut g);
    resolve(&mut g);
    cite!(["702.32a", "118.12"]; assert!(!g.battlefield.contains(&source)));
    assert!(g.decision.is_none());
}

#[test]
fn r702_32a_hydra_can_spend_the_last_counter_and_an_old_upkeep_cannot_sacrifice_its_blinked_successor() {
    let mut g = game();
    let source = cast(&mut g, 1, "RRRRR");
    for left in [4, 3, 2, 1, 0] {
        g.players[0].pool = mana_pool("C");
        g.act(0, paid("C", Move::Activate { source, ability: 1, targets: vec![vec![game::Target::Player(1)]], paying: vec![] })).unwrap();
        resolve(&mut g);
        cite!(["702.32a"]; assert!(g.battlefield.contains(&source) && g.stack.is_empty()));
        assert_eq!(g.counters(source).get("fade").copied().unwrap_or(0), left);
    }
    // Uncited (Oracle): five paid activations deal five damage; fading has not sacrificed it.
    assert_eq!(g.players[1].life, 15);
    upkeep(&mut g);
    let blink = g.put(0, 3, Zone::Hand);
    g.players[0].pool = mana_pool("W");
    g.act(0, paid("W", Move::Cast { card: blink, targets: vec![vec![source.into()]] })).unwrap();
    resolve(&mut g);
    let later = *g.battlefield.iter().find(|o| g.obj(**o).chars.name == "Ancient Hydra").unwrap();
    cite!(["400.7", "702.32a"]; assert_ne!(source, later));
    resolve(&mut g);
    cite!(["400.7", "113.7a"]; assert!(g.battlefield.contains(&later)));
    assert_eq!(g.counters(later).get("fade"), Some(&5));
}

#[test]
fn r118_12_mandatory_counter_cost_is_full_while_an_unlinked_effect_removes_as_many_as_possible() {
    for payment in [false, true] {
        let mut text = "{0}: Remove two charge counters from this artifact.".to_owned();
        if payment { text.push_str(" If you can't, you lose 1 life."); }
        let def = CardDef::new("Removal outcome", "{0}", "Artifact", &text, None).unwrap();
        game::play::check(&def).unwrap();
        let mut g = Game::new(vec![def], 2, 20);
        let source = g.put(0, 0, Zone::Battlefield);
        g.put_counters(source, "charge", 1);
        g.act(0, Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }).unwrap();
        resolve(&mut g);
        cite!(["118.3", "118.12", "609.3"]; assert_eq!(g.counters(source).get("charge").copied().unwrap_or(0), u32::from(payment)));
        assert_eq!(g.players[0].life, if payment { 19 } else { 20 });
    }
}
