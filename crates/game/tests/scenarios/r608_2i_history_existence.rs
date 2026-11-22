//! Boolean history conditions read pending and retained events (608.2i).
use crate::support::cite;
use game::{CardDef, Game, Move, Zone};

fn fixture() -> (Game, game::ObjId) {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let def = CardDef::from_faces(&catalog["Ulna Alley Shopkeep"]).unwrap();
    game::play::check(&def).unwrap();
    let mut g = Game::new(vec![def], 2, 20);
    let creature = g.put(0,0,Zone::Battlefield);
    (g,creature)
}

#[test]
fn r608_2i_own_life_gain_is_visible_before_delivery_and_after_the_raw_log_is_cleared() {
    let (mut g,creature) = fixture();
    g.gain_life(1,1).unwrap();
    cite!(["608.2i", "613.1g"]; assert_eq!(g.power(creature),Some(2),"an opponent's gain is not yours"));
    for _ in 0..3 {g.gain_life(0,1).unwrap();}
    cite!(["608.2i", "613.1g"]; assert_eq!(g.power(creature),Some(4),"pending gains satisfy the condition once"));
    g.act(0,Move::Pass).unwrap();
    // All events are delivered. This documented fixture operation clears only
    // raw events; conditions must continue to read the retained current turn.
    g.log.clear();
    cite!(["608.2i", "613.1g"]; assert_eq!(g.power(creature),Some(4)));
}

#[test]
fn r608_2i_history_queries_keep_empty_and_changed_branches_independent() {
    let (g,creature) = fixture();
    let mut branch = g.clone();
    branch.gain_life(0,1).unwrap();
    cite!(["608.2i", "613.1g"]; assert_eq!(branch.power(creature),Some(4)));
    cite!(["608.2i", "613.1g"]; assert_eq!(g.power(creature),Some(2)));
    let mut other = g.clone();
    other.gain_life(1,1).unwrap();
    cite!(["608.2i", "613.1g"]; assert_eq!(other.power(creature),Some(2)));
}
