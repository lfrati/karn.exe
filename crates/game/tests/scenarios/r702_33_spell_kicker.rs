//! Kicker's announced additional cost and conditional targets (601.2b–c, 702.33a,g).
use crate::support::cite;
use game::{CardDef, Game, Move, ObjId, Target, Zone};

fn setup(text: &str) -> Game {
    let defs = vec![
        CardDef::new("Kicker witness", "{R}", "Instant", text, None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap();
    }
    let mut g = Game::new(defs, 2, 20);
    g.players[0].pool.colors[ir::Color::Red as usize] = 1;
    g.players[0].pool.colorless = 4;
    g
}
fn cast(card: ObjId, kicks: u32, targets: Vec<Vec<Target>>) -> Move {
    Move::WithKicks(kicks, Box::new(Move::Cast { card, targets }))
}
fn resolve(g: &mut Game) {
    for _ in 0..2 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
const BURST: &str =
    "Kicker {4}\nThis spell deals 2 damage to any target. If this spell was kicked, it deals 4 damage instead.";

#[test]
fn r702_33a_burst_lightning_pays_once_and_replaces_rather_than_adds_damage() {
    for (kicks, damage, left) in [(0, 2, 4), (1, 4, 0)] {
        let mut g = setup(BURST);
        let card = g.put(0, 0, Zone::Hand);
        cite!(["702.33a"]; refused("a kicker cost can be paid only once (702.33a)"): g.act(0, cast(card, 2, vec![vec![Target::Player(1)]])));
        g.act(0, crate::support::paid(if kicks == 0 { "R" } else { "CCCCR" }, cast(card, kicks, vec![vec![Target::Player(1)]]))).unwrap();
        let spell = *g.stack.last().unwrap();
        let audit=g.audit_game_objects();
        cite!(["601.2b", "702.33d"]; assert_eq!(audit["objects"][spell.0.to_string()]["announced"]["costs"]["kicks"], kicks));
        cite!(["118.8d"]; assert_eq!(g.obj(spell).chars.mana_value, 1));
        cite!(["601.2f"]; assert_eq!(g.players[0].pool.total(), left));
        let mut copy = g.clone();
        for g in [&mut g, &mut copy] {
            resolve(g);
            cite!(["702.33e"]; assert_eq!(g.players[1].life, 20 - damage));
            assert!(g.stack.is_empty());
        }
    }
}

#[test]
fn r601_2h_an_unpayable_kicker_is_neither_offered_nor_committed() {
    let mut g = setup(BURST);
    g.players[0].pool.colorless = 3;
    let card = g.put(0, 0, Zone::Hand);
    cite!(["601.2h"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0, crate::support::paid("CCCR",cast(card, 1, vec![vec![Target::Player(1)]]))));
    assert_eq!(g.players[0].pool.total(), 4);
    assert_eq!(g.obj(card).zone, Zone::Hand);
    assert!(g.stack.is_empty());
}

#[test]
fn r702_33g_only_kicked_spells_choose_the_conditional_target() {
    let mut g = setup("Kicker {4}\nYou gain 1 life. If this spell was kicked, destroy target creature.");
    let card = g.put(0, 0, Zone::Hand);
    let bear = g.put(1, 1, Zone::Battlefield);
    cite!(["702.33g"]; refused("wrong number of targets: 1 chosen, it takes 0 (601.2c)"): g.act(0, cast(card, 0, vec![vec![Target::Object(bear)]])));
    cite!(["702.33g"]; refused("wrong number of targets: 0 chosen, it takes 1 (601.2c)"): g.act(0, cast(card, 1, vec![vec![]])));
    let mut kicked = g.clone();
    g.act(0, crate::support::paid("R", cast(card, 0, vec![vec![]]))).unwrap();
    resolve(&mut g);
    assert_eq!(g.players[0].life, 21);
    cite!(["702.33e"]; assert!(g.battlefield.contains(&bear)));
    kicked.act(0, crate::support::paid("CCCCR", cast(card, 1, vec![vec![Target::Object(bear)]]))).unwrap();
    resolve(&mut kicked);
    assert_eq!(kicked.players[0].life, 21);
    cite!(["702.33e"]; assert!(!kicked.battlefield.contains(&bear)));
}

#[test]
fn r608_2b_a_lost_conditional_target_still_makes_the_kicked_spell_fail_to_resolve() {
    let mut g = setup("Kicker {4}\nYou gain 1 life. If this spell was kicked, destroy target creature.");
    let bear = g.put(1, 1, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, crate::support::paid("CCCCR", cast(card, 1, vec![vec![Target::Object(bear)]]))).unwrap();
    g.move_object(bear, Zone::Graveyard);
    resolve(&mut g);
    cite!(["608.2b"]; assert_eq!(g.players[0].life, 20));
    assert!(g.stack.is_empty());
}

#[test]
fn r601_2c_ordinary_resolution_conditions_do_not_remove_target_requirements() {
    let mut g = setup("Kicker {4}\nIf you control an artifact, destroy target creature.");
    let card = g.put(0, 0, Zone::Hand);
    let bear = g.put(1, 1, Zone::Battlefield);
    cite!(["601.2c"]; refused("wrong number of targets: 0 chosen, it takes 1 (601.2c)"): g.act(0, cast(card, 0, vec![vec![]])));
    g.act(0, crate::support::paid("R", cast(card, 0, vec![vec![Target::Object(bear)]]))).unwrap();
    resolve(&mut g);
    assert!(g.battlefield.contains(&bear));
}

#[test]
fn r702_33g_otherwise_uses_the_opposite_kicker_target_requirement() {
    let mut g = setup("Kicker {4}\nIf this spell was kicked, destroy target creature. Otherwise, you gain 1 life.");
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, crate::support::paid("R", cast(card, 0, vec![vec![]]))).unwrap();
    resolve(&mut g);
    cite!(["702.33g"]; assert_eq!(g.players[0].life, 21));
}

/// "If this spell was kicked, instead destroy target creature": alternative targets (601.2c).
/// Unkicked, only the first is chosen; kicked, only the second (702.33g).
#[test]
fn r601_2c_kicked_the_instead_sentence_has_the_only_target() {
    let text = "Kicker {4}\nDestroy target creature with mana value 2 or less. If this spell was kicked, instead destroy target creature.";
    let mut g = setup(text);
    let card = g.put(0, 0, Zone::Hand);
    let bear = g.put(1, 1, Zone::Battlefield);
    for (kicks, wrong, why) in [(0, vec![vec![Target::Object(bear)], vec![Target::Object(bear)]], "wrong number of targets: 1 chosen, it takes 0 (601.2c)"), (0, vec![vec![], vec![Target::Object(bear)]], "wrong number of targets: 0 chosen, it takes 1 (601.2c)"),
                                (1, vec![vec![Target::Object(bear)], vec![]], "wrong number of targets: 1 chosen, it takes 0 (601.2c)"), (1, vec![vec![Target::Object(bear)], vec![Target::Object(bear)]], "wrong number of targets: 1 chosen, it takes 0 (601.2c)")] {
        cite!(["702.33g"]; case: format!("{kicks} {wrong:?}"); refused(why): g.clone().act(0, cast(card, kicks, wrong.clone())));
    }
    let mut unkicked = g.clone();
    unkicked.act(0, crate::support::paid("R", cast(card, 0, vec![vec![Target::Object(bear)], vec![]]))).unwrap();
    resolve(&mut unkicked);
    cite!(["702.33g"]; assert!(!unkicked.battlefield.contains(&bear)));
    g.act(0, crate::support::paid("CCCCR", cast(card, 1, vec![vec![], vec![Target::Object(bear)]]))).unwrap();
    resolve(&mut g);
    cite!(["702.33g"]; assert!(!g.battlefield.contains(&bear)));
}

/// A target only inside a quoted ability the instead sentence gives is not the spell's own:
/// the cast-time alternative is not read.
#[test]
fn r601_2c_a_quoted_target_is_not_an_alternative_target() {
    let text = "Kicker {4}\nTap target creature. If this spell was kicked, instead create a 1/1 white Soldier creature token with \"{T}: Tap target creature.\"";
    let d = CardDef::new("Quoted", "{R}", "Instant", text, None).unwrap();
    crate::support::refused(game::play::check(&d).map_err(|e|e.0),"instead: a cast-time alternative whose only target is not the spell's");
}

#[test]
fn r702_33g_a_target_in_the_unkicked_branch_is_not_chosen_when_kicked() {
    let mut g = setup("Kicker {4}\nIf this spell was kicked, you gain 1 life. Otherwise, destroy target creature.");
    let card = g.put(0, 0, Zone::Hand);
    cite!(["601.2c", "702.33g"]; refused("there is no legal target for it (601.2c)"): g.act(0, cast(card, 0, vec![vec![]])));
    g.act(0, crate::support::paid("CCCCR", cast(card, 1, vec![vec![]]))).unwrap();
    resolve(&mut g);
    cite!(["702.33g"]; assert_eq!(g.players[0].life, 21));
}

#[test]
fn r608_2d_kicker_and_announced_targets_survive_two_resolution_choices() {
    use game::state::Answer;
    let mut g = setup(
        "Kicker {4}\nDraw three cards, then discard two cards. If this spell was kicked, target player discards two cards.",
    );
    for _ in 0..3 {
        g.put(0, 1, Zone::Library);
        g.put(1, 1, Zone::Hand);
    }
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, crate::support::paid("CCCCR", cast(card, 1, vec![vec![Target::Player(1)]]))).unwrap();
    resolve(&mut g);
    assert_eq!(g.players[0].hand.len(), 3);
    crate::support::refused(g.act(0, Move::Pass), "a decision is pending: a choice of the effect being applied (608.2d)");
    let selected = g.players[0].hand[..2].to_vec();
    g.act(0, Move::Choose(Answer::Objects(selected.clone()))).unwrap();
    // CR404.3: the owner explicitly orders these simultaneous graveyard arrivals.
    assert_eq!(g.decision, Some(game::state::Decision::Choose { player: 0, choice: game::state::Choice::OrderCards { to: game::state::Placement::Zone(Zone::Graveyard), cards: selected.clone() } }));
    g.act(0, Move::Choose(game::state::Answer::Objects(vec![selected[1],selected[0]]))).unwrap();
    cite!(["608.2d"]; assert_eq!(g.players[0].hand.len(), 1));
    let mut copy = g.clone();
    for g in [&mut g, &mut copy] {
        let selected = g.players[1].hand[..2].to_vec();
        crate::support::refused(g.act(0, Move::Choose(Answer::Objects(selected.clone()))), "that decision is another player's");
        g.act(1, Move::Choose(Answer::Objects(selected.clone()))).unwrap();
        // CR404.3: the owner explicitly orders these simultaneous graveyard arrivals.
        assert_eq!(g.decision, Some(game::state::Decision::Choose { player: 1, choice: game::state::Choice::OrderCards { to: game::state::Placement::Zone(Zone::Graveyard), cards: selected.clone() } }));
        g.act(1, Move::Choose(game::state::Answer::Objects(vec![selected[1],selected[0]]))).unwrap();
        assert_eq!(g.players[0].hand.len(), 1);
        cite!(["702.33e"]; assert_eq!(g.players[1].hand.len(), 1));
        assert_eq!(g.players[0].pool.total(), 0);
        assert!(g.stack.is_empty());
    }
}
