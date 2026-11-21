//! 704.5c / 704.3: poison losses join the existing simultaneous player-loss batch.
use crate::support::cite;
use game::{Event, Game, Target};
#[test]
fn r704_5c_poison_threshold_and_other_loss_reasons_share_one_loss_per_player() {
    let mut g=Game::new(vec![],4,20);
    g.put_counters(Target::Player(0),"poison",9);
    g.put_counters(Target::Player(1),"poison",10);
    g.put_counters(Target::Player(2),"poison",11);
    g.players[2].life=0;
    g.players[3].life=0;
    assert!(g.state_based_actions());
    cite!(["704.5c"]; assert!(!g.players[0].lost));
    for p in [1,2,3] { cite!(["704.5c", "704.5a"]; case: p; assert!(g.players[p].lost)); }
    cite!(["704.3"]; assert_eq!(g.log.iter().filter_map(|e|if let Event::Lost{player}=e {Some(*player)} else {None}).collect::<Vec<_>>(),vec![1,2,3]));
    assert_eq!(g.winner(),Some(0));
    assert!(!g.state_based_actions());
    g.put_counters(Target::Player(1),"poison",1);
    assert!(g.counters(Target::Player(1)).is_empty());
    assert_eq!(g.remove_counters(Target::Player(1),"poison",1),0);
}
