//! Native supported fixed-card partition adjacent to proposed variable-X partition.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Zone};
use game::state::{Answer, Choice, Decision};

fn definitions(names: &[&str]) -> Vec<CardDef> {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    names.iter().map(|name| CardDef::from_faces(&catalog[name]).unwrap()).collect()
}

// Rules: 608.2c, 608.2d, 701.20e, 609.3, 400.7
// Sleight's fixed partition moves only the selected looked-at incarnation. Neither
// an older hand card nor a third library card is eligible; looking is private.
#[test]
fn r608_native_fixed_partition_is_private_and_preserves_the_unlooked_tail() {
    let mut g = Game::new(definitions(&["Sleight of Hand", "Island", "Forest", "Mountain"]), 2, 20);
    let tail = g.put(0, 3, Zone::Library);
    let second = g.put(0, 2, Zone::Library);
    let top = g.put(0, 1, Zone::Library);
    let old_hand = g.put(0, 3, Zone::Hand);
    let spell = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[1] = 1;
    g.act(0, paid("U",Move::Cast { card: spell, targets: vec![] })).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    let Some(Decision::Choose { player: 0, choice: Choice::Objects { among, count: 1, .. } }) = &g.decision else {
        panic!("expected controller's mandatory one-card choice: {:?}", g.decision);
    };
    cite!(["608.2d"]; assert_eq!(among, &vec![top, second]));
    cite!(["701.20e"]; assert!(g.sees(0, top) && g.sees(0, second)));
    cite!(["701.20e"]; assert!(!g.sees(1, top) && !g.sees(1, second)));
    cite!(["701.20e"]; assert!(g.observations(1).is_empty()));
    for invalid in [tail, old_hand] {
        cite!(["608.2d"]; refused("a chosen object is not among the offered objects to put (608.2d)"):
            g.act(0, Move::Choose(Answer::Objects(vec![invalid]))));
    }
    cite!(["608.2d"]; refused("choose exactly 1 of them to put (608.2d)"):
        g.act(0, Move::Choose(Answer::Objects(vec![]))));
    cite!(["701.20b"]; assert_eq!(g.players[0].library, vec![tail, second, top]));
    g.act(0, Move::Choose(Answer::Objects(vec![second]))).unwrap();
    cite!(["608.2c"]; assert_eq!(g.players[0].library.iter().rev().map(|id| g.obj(*id).chars.name.as_str()).collect::<Vec<_>>(), vec!["Mountain", "Island"]));
    cite!(["400.7"]; assert!(!g.players[0].hand.contains(&second)));
    cite!(["608.2c"]; assert_eq!(g.players[0].hand.iter().map(|id| g.obj(*id).chars.name.as_str()).collect::<Vec<_>>(), vec!["Mountain", "Forest"]));
    cite!(["608.2c"]; assert!(g.stack.is_empty() && g.decision.is_none()));
}

// Rules: 609.3, 608.2c, 701.20e
// Looking at fewer than two is legal and is not drawing: empty library causes no loss.
#[test]
fn r609_native_fixed_partition_handles_zero_and_one_available_card() {
    for count in 0..=1 {
        let mut g = Game::new(definitions(&["Sleight of Hand", "Island"]), 2, 20);
        for _ in 0..count { g.put(0, 1, Zone::Library); }
        let spell = g.put(0, 0, Zone::Hand);
        g.players[0].pool.colors[1] = 1;
        g.act(0, paid("U",Move::Cast { card: spell, targets: vec![] })).unwrap();
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        cite!(["609.3"]; case: format!("library size {count}"); assert_eq!(g.players[0].hand.len(), count));
        cite!(["608.2c"]; assert!(g.players[0].library.is_empty() && g.stack.is_empty() && g.decision.is_none()));
        cite!(["701.20e"]; assert!(g.observations(1).is_empty()));
        cite!(["609.3"]; assert_eq!(g.players[0].life, 20));
    }
}

#[test]
fn native_stargaze_admits_variable_partition_and_refuses_overflow_before_payment() {
    let defs = definitions(&["Stargaze", "Island", "Forest", "Mountain"]);
    // Uncited (admission): executable composition; semantics exercised separately.
    game::play::check(&defs[0]).unwrap();
    for x in [0, 1, 2_147_483_648] {
        let mut g = Game::new(defs.clone(), 2, 20);
        for index in [3, 2, 1] { g.put(0, index, Zone::Library); }
        let spell = g.put(0, 0, Zone::Hand);
        g.players[0].pool.colors[2] = x + 2;
        let before = format!("{g:?}");
        // Author-selected black units for XBB, including the overflow probe;
        // the selection is independent of the engine's computed cost or pool.
        let spending = match x {
            0 => crate::support::mana_pool("BB"),
            1 => crate::support::mana_pool("BBB"),
            2_147_483_648 => game::state::Pool { colors: [0, 0, 2_147_483_650, 0, 0], ..Default::default() },
            _ => unreachable!("the three authored X cases"),
        };
        let result = g.act(0, Move::Paying(spending, Box::new(Move::WithX(x, Box::new(Move::Cast { card: spell, targets: vec![] })))));
        // Uncited (engine boundary): representation refusal, not a rule prohibition.
        if x == 2_147_483_648 {
            assert!(result.unwrap_err().contains("numerical range"));
            assert_eq!(format!("{g:?}"), before);
            assert!(g.observations(0).is_empty() && g.observations(1).is_empty());
        } else {
            result.unwrap();
            for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
            if x == 1 {
                let Some(Decision::Choose { player: 0, choice: Choice::Objects { count: 1, among, .. } }) = &g.decision else { panic!("{:?}", g.decision) };
                let chosen = among[0];
                g.act(0, Move::Choose(Answer::Objects(vec![chosen]))).unwrap();
            }
            cite!(["107.3a", "119.3"]; assert_eq!(g.players[0].life, 20-x as i32));
            cite!(["608.2c"]; assert_eq!(g.players[0].hand.len(), x as usize));
            cite!(["608.2c"]; assert_eq!(g.players[0].graveyard.len(), x as usize+1));
            cite!(["608.2c"]; assert_eq!(g.players[0].library.len(), 3-2*x as usize));
        }
    }
}
