//! Piles retain individual permanents and assign separation and pile choice to their stated players.
use crate::support::{cite, oracle};
use game::{Game, Move, Target, Zone};
use game::state::{Answer, Choice, Decision};

#[test]
fn r700_3_liliana_separates_but_the_target_chooses_the_sacrificed_pile() {
    for empty in [false, true] {
        let mut g = Game::new(vec![oracle("Liliana of the Veil"), oracle("Grizzly Bears"), oracle("Forest"), oracle("Sol Ring")], 3, 20);
        let liliana = g.put(0, 0, Zone::Battlefield);
        let current = g.obj(liliana).counters.get("loyalty").copied().unwrap_or(0);
        g.put_counters(liliana, "loyalty", 7 - current);
        let bear = g.put(1, 1, Zone::Battlefield);
        let land = g.put(1, 2, Zone::Battlefield);
        let ring = g.put(1, 3, Zone::Battlefield);
        let token = g.create_token(1, g.obj(bear).chars.clone(), false);
        let bystander = g.put(2, 1, Zone::Battlefield);
        g.act(0, Move::Activate { source: liliana, ability: 2, targets: vec![vec![Target::Player(1)]], paying: vec![] }).unwrap();
        for _ in 0..3 { g.act(g.priority, Move::Pass).unwrap(); }
        let Some(Decision::Choose { player: 0, choice: Choice::Select { among, min: 0, max: 4 } }) = &g.decision else { panic!("{:?}", g.decision) };
        cite!(["700.3a"]; assert!(among.len() == 4 && [bear, land, ring, token].iter().all(|o| among.contains(o))));
        cite!(["700.3c"]; assert!([bear, land, ring, token].iter().all(|o| g.obj(*o).zone == Zone::Battlefield)));
        cite!(["608.2d"]; refused("not among the offered objects"): g.act(0, Move::Choose(Answer::Objects(vec![bystander]))));
        let chosen = if empty { vec![] } else { vec![bear, token] };
        g.act(0, Move::Choose(Answer::Objects(chosen.clone()))).unwrap();
        let Some(Decision::Choose { player: 1, choice: Choice::Pile { piles } }) = &g.decision else { panic!("{:?}", g.decision) };
        cite!(["700.3a", "700.3d"]; assert_eq!(piles[0].len(), chosen.len()));
        cite!(["700.3c"]; assert!([bear, land, ring, token].iter().all(|o| g.obj(*o).zone == Zone::Battlefield)));
        let mut fork = g.clone();
        for g in [&mut g, &mut fork] {
            g.act(1, Move::Choose(Answer::Pile(0))).unwrap();
            cite!(["701.21a", "700.3d"]; assert_eq!(g.obj(bear).zone == Zone::Battlefield, empty));
            cite!(["701.21a", "111.7"]; assert_eq!(g.obj(token).zone == Zone::Battlefield, empty));
            cite!(["700.3a"]; assert!([land, ring, bystander, liliana].iter().all(|o| g.battlefield.contains(o))));
            cite!(["606.4"]; assert_eq!(g.obj(liliana).counters.get("loyalty"), Some(&1)));
            cite!(["608.2c"]; assert!(g.stack.is_empty() && g.decision.is_none()));
        }
    }
}
