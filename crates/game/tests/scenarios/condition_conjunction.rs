//! Conditional noun-phrase alternatives execute; conjunctions remain an explicit admission gap.
use game::{CardDef, Game, Move, Zone};
use crate::support::cite;

fn definition(condition: &str) -> CardDef {
    CardDef::new("Reward", "{0}", "Sorcery", &format!("If {condition}, you gain 3 life."), None).unwrap()
}
fn gain(condition: &str, artifact: bool) -> i32 {
    let d = definition(condition);
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d, CardDef::new("Relic", "{0}", "Artifact", "", None).unwrap()], 2, 20);
    if artifact { g.put(0, 1, Zone::Battlefield); }
    let c = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: c, targets: vec![] }).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    g.players[0].life - 20
}
#[test]
fn r608_2c_alternatives_run_and_conjunctions_are_refused() {
    assert_eq!(gain("you control no creatures", false), 3);
    cite!(["608.2c"]; assert_eq!(gain("you control an artifact or an enchantment", false), 0));
    cite!(["608.2c"]; assert_eq!(gain("you control an artifact or an enchantment", true), 3));
    assert_eq!(game::play::check(&definition("you control an artifact and an enchantment")).unwrap_err().0,"condition: control of a conjunction of noun phrases");
}
