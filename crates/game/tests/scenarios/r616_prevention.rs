//! 615.7/616.1: full pinned prevention resolves, retains shields and orders with modifiers.
use crate::support::{cite, paid};
use game::operation::DamageOption;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, Target, Zone};
fn pinned(name: &str) -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let d = CardDef::from_faces(CardDef::catalog(&faces).get(name).unwrap()).unwrap();
    game::play::check(&d).unwrap();
    d
}
fn cast(g: &mut Game, card: usize, targets: Vec<Vec<Target>>, spending: &str) {
    g.players[0].pool.colors = [10; 5];
    let spell = g.put(0, card, Zone::Hand);
    g.act(0, paid(spending, Move::Cast { card: spell, targets })).unwrap();
    for _ in 0..2 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn source() -> CardDef {
    CardDef::new("Dealer", "{R}", "Creature", "", Some((2, 20))).unwrap()
}
#[test]
fn r615_7_mending_hands_retains_next_four_across_hits() {
    let mut g = Game::new(vec![pinned("Mending Hands"), source()], 2, 20);
    let source = g.put(0, 1, Zone::Battlefield);
    cast(&mut g, 0, vec![vec![Target::Player(1)]], "W");
    g.damage(source, Target::Player(1), 3);
    cite!(["615.7"]; assert_eq!(g.players[1].life, 20));
    g.damage(source, Target::Player(1), 3);
    cite!(["615.7"]; assert_eq!(g.players[1].life, 18));
}
#[test]
fn r615_1_fog_and_holy_day_prevent_combat_only() {
    for (name, spending) in [("Fog", "G"), ("Holy Day", "W")] {
        let mut g = Game::new(vec![pinned(name), source()], 2, 20);
        let source = g.put(0, 1, Zone::Battlefield);
        cast(&mut g, 0, vec![], spending);
        // Ordinary damage still happens; combat prevention is exercised through full combat
        // by scripts/tests/test_prevention_review_client.py. Here the scope's negative side is exact.
        g.damage(source, Target::Player(1), 3);
        // Uncited (card text): "Prevent all combat damage that would be dealt this turn.": this damage is not combat damage
        assert_eq!(g.players[1].life, 17);
    }
}
#[test]
fn r616_1_shield_before_or_after_furnace_changes_the_final_amount() {
    for shield_first in [false, true] {
        let mut g = Game::new(vec![pinned("Mending Hands"), pinned("Furnace of Rath"), source()], 2, 20);
        g.put(0, 1, Zone::Battlefield);
        let source = g.put(0, 2, Zone::Battlefield);
        cast(&mut g, 0, vec![vec![Target::Player(1)]], "W");
        g.damage(source, Target::Player(1), 3);
        let Some(Decision::Choose { player: 1, choice: Choice::DamageOrder { among, .. } }) = g.decision.clone() else {
            panic!("{:?}", g.decision)
        };
        let offer = among
            .iter()
            .find(|o| {
                if shield_first { matches!(o, DamageOption::Shield(_)) } else { matches!(o, DamageOption::Modifier(_)) }
            })
            .unwrap().clone();
        g.act(1, Move::Choose(Answer::DamageOrder(offer))).unwrap();
        cite!(["616.1", "616.1f"]; case: format!("shield first: {shield_first}"); assert_eq!(g.players[1].life, if shield_first { 20 } else { 18 }));
    }
}
#[test]
fn r615_1_kami_sacrifice_creates_prevention_after_the_source_has_gone() {
    let mut g = Game::new(vec![pinned("Kami of False Hope"), source()], 2, 20);
    let kami = g.put(0, 0, Zone::Battlefield);
    g.act(0, Move::Activate { source: kami, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    for _ in 0..2 {
        g.act(g.priority, Move::Pass).unwrap();
    }
    assert_eq!(g.obj(kami).zone, Zone::Gone);
    let dealer = g.put(0, 1, Zone::Battlefield);
    g.damage(dealer, Target::Player(1), 2);
    // Uncited (card text): "Sacrifice this creature: Prevent all combat damage that would be dealt this turn.": this damage is not combat damage
    assert_eq!(g.players[1].life, 18, "combat-only prevention leaves noncombat damage alone");
}
#[test]
fn r615_7_healing_salves_modal_branches_keep_their_separate_targets_and_effects() {
    for mode in [0, 1] {
        let mut g = Game::new(vec![pinned("Healing Salve"), source()], 2, 20);
        let source = g.put(0, 1, Zone::Battlefield);
        g.players[0].pool.colors = [10; 5];
        let spell = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Modes(vec![mode], Box::new(paid("W", Move::Cast { card: spell, targets: vec![vec![Target::Player(1)]] }))))
            .unwrap();
        for _ in 0..2 {
            g.act(g.priority, Move::Pass).unwrap();
        }
        cite!(["700.2a"]; case: format!("mode {mode}"); assert_eq!(g.players[1].life, if mode == 0 { 23 } else { 20 }));
        g.damage(source, Target::Player(1), 4);
        cite!(["700.2a"]; case: format!("mode {mode}"); assert_eq!(g.players[1].life, 19, "life-gain branch23-4; shield branch20-(4-3)"));
        g.damage(source, Target::Player(1), 1);
        assert_eq!(g.players[1].life, 18);
    }
}
#[test]
fn r608_2h_lasting_source_predicate_reads_the_departed_creatures_last_known_facts() {
    let prevent = CardDef::new(
        "Source Ward",
        "{0}",
        "Instant",
        "Prevent all damage that would be dealt this turn by creatures you control.",
        None,
    )
    .unwrap();
    game::play::check(&prevent).unwrap();
    let mut g = Game::new(vec![prevent, source(), pinned("Lightning Bolt")], 2, 20);
    let source = g.put(0, 1, Zone::Battlefield);
    cast(&mut g, 0, vec![], "");
    g.destroy(source);
    g.damage(source, Target::Player(1), 2);
    cite!(["609.7b"]; assert_eq!(g.players[1].life, 20));
    let spell = g.put(0, 2, Zone::Stack);
    g.damage(spell, Target::Player(1), 2);
    cite!(["609.7b"]; assert_eq!(g.players[1].life, 18));
}
