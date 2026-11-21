//! A named player group and its permanents are one simultaneous damage instruction.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Zone};
fn setup() -> Game {
    let hazard = CardDef::new(
        "Tectonic Hazard",
        "{R}",
        "Sorcery",
        "Tectonic Hazard deals 1 damage to each opponent and each creature they control.",
        None,
    )
    .unwrap();
    game::play::check(&hazard).unwrap();
    let bear = CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap();
    let witness = CardDef::new(
        "Witness",
        "{0}",
        "Creature — Human",
        "Whenever this creature is dealt damage, you gain 1 life.",
        Some((1, 1)),
    )
    .unwrap();
    game::play::check(&witness).unwrap();
    Game::new(vec![hazard, bear, witness], 3, 20)
}
fn resolve(g: &mut Game) {
    for _ in 0..3 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
#[test]
fn r120_4_their_names_all_previously_named_opponents_and_damage_is_not_targeted() {
    let mut g = setup();
    let ours = g.put(0, 1, Zone::Battlefield);
    let a = g.put(1, 1, Zone::Battlefield);
    let b = g.put(2, 1, Zone::Battlefield);
    let spell = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[3] = 1;
    g.act(0, paid("R", Move::Cast { card: spell, targets: vec![] })).unwrap();
    resolve(&mut g);
    cite!(["120.3a"]; assert_eq!(g.players.iter().map(|p| p.life).collect::<Vec<_>>(), [20, 19, 19]));
    assert_eq!(g.obj(ours).damage, 0);
    cite!(["120.3e"]; assert_eq!(g.obj(a).damage, 1));
    assert_eq!(g.obj(b).damage, 1);
}
#[test]
fn r603_2_damage_triggers_survive_simultaneous_lethal_damage_to_the_group() {
    let mut g = setup();
    g.put(1, 2, Zone::Battlefield);
    g.put(2, 2, Zone::Battlefield);
    let spell = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[3] = 1;
    g.act(0, paid("R", Move::Cast { card: spell, targets: vec![] })).unwrap();
    resolve(&mut g);
    cite!(["704.5g"]; assert!(g.battlefield.is_empty()));
    cite!(["120.4b"]; assert_eq!(g.stack.len(), 2));
    assert_eq!(g.players[1].life, 19);
    assert_eq!(g.players[2].life, 19);
    resolve(&mut g);
    resolve(&mut g);
    assert_eq!(g.players[1].life, 20);
    assert_eq!(g.players[2].life, 20);
}
#[test]
fn r608_2h_player_groups_are_captured_when_the_instruction_runs_not_when_cast() {
    let mut g = setup();
    let spell = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[3] = 1;
    g.act(0, paid("R", Move::Cast { card: spell, targets: vec![] })).unwrap();
    let later = g.put(2, 1, Zone::Battlefield);
    let mut copy = g.clone();
    for g in [&mut g, &mut copy] {
        resolve(g);
        cite!(["608.2h"]; assert_eq!(g.obj(later).damage, 1));
    }
}
