//! "If you have less life than an opponent", "if you control fewer creatures than an
//! opponent": a condition asked of the game as the instruction is followed (608.2c), of one
//! player's life total or permanents against another's.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Zone};

const TIMELY: usize = 0;
const BEAR: usize = 1;
const LAND: usize = 2;
const MARGIN: usize = 3;
const EACH: usize = 4;
const START: usize = 5;

fn game(players: usize) -> Game {
    let defs = vec![
        CardDef::new("Timely Reinforcements", "{2}{W}", "Sorcery", "If you have less life than an opponent, you gain 6 life. If you control fewer creatures than an opponent, create three 1/1 white Soldier creature tokens.", None).unwrap(),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Wastes", "", "Land", "", None).unwrap(),
        CardDef::new("Margin", "{W}", "Sorcery", "If an opponent controls at least two more lands than you, you gain 3 life.", None).unwrap(),
        CardDef::new("Each", "{W}", "Sorcery", "If you control more creatures than each opponent, you gain 3 life.", None).unwrap(),
        CardDef::new("Start", "{W}", "Sorcery", "If you have at least 10 life more than your starting life total, you gain 1 life.", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    let mut g = Game::new(defs, players, 20);
    for player in 0..players { for _ in 0..5 { g.put(player, LAND, Zone::Library); } }
    g
}
/// Player 0 casts this sorcery and it resolves.
fn resolve(g: &mut Game, card: usize, spending: &str) {
    let spell = g.put(0, card, Zone::Hand);
    g.players[0].pool.colors[0] = 3;
    g.act(0, paid(spending, Move::Cast { card: spell, targets: vec![] })).unwrap();
    for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); }
    g.players[0].pool = Default::default();
}
fn tokens(g: &Game) -> Vec<ObjId> { g.battlefield.iter().copied().filter(|o| g.obj(*o).card.is_none()).collect() }

#[test]
fn each_half_is_asked_on_its_own_as_it_is_reached() {
    // neither: equal life, equal creatures
    let mut g = game(2);
    resolve(&mut g, TIMELY, "WWW");
    assert_eq!((g.players[0].life, tokens(&g).len()), (20, 0));
    // less life only
    let mut g = game(2);
    g.players[0].life = 19;
    resolve(&mut g, TIMELY, "WWW");
    cite!(["608.2c"]; assert_eq!((g.players[0].life, tokens(&g).len()), (25, 0)));
    // fewer creatures only; more life is not less
    let mut g = game(2);
    g.players[0].life = 21;
    g.put(1, BEAR, Zone::Battlefield);
    resolve(&mut g, TIMELY, "WWW");
    cite!(["608.2c"]; assert_eq!((g.players[0].life, tokens(&g).len()), (21, 3)));
    assert!(tokens(&g).iter().all(|t| g.obj(*t).controller == 0));
    // both
    let mut g = game(2);
    g.players[0].life = 1;
    g.put(0, BEAR, Zone::Battlefield);
    g.put(1, BEAR, Zone::Battlefield);
    g.put(1, BEAR, Zone::Battlefield);
    resolve(&mut g, TIMELY, "WWW");
    cite!(["608.2c"]; assert_eq!((g.players[0].life, tokens(&g).len()), (7, 3)));
}

#[test]
fn one_opponent_is_enough_and_each_opponent_is_all_of_them() {
    // "an opponent": the second opponent alone has more life
    let mut g = game(3);
    g.players[2].life = 30;
    g.players[1].life = 5;
    resolve(&mut g, TIMELY, "WWW");
    cite!(["608.2c"]; assert_eq!(g.players[0].life, 26));
    // "each opponent": more creatures than one of them is not enough
    let mut g = game(3);
    g.put(0, BEAR, Zone::Battlefield);
    g.put(0, BEAR, Zone::Battlefield);
    g.put(1, BEAR, Zone::Battlefield);
    g.put(2, BEAR, Zone::Battlefield);
    let mut tied = g.clone();
    resolve(&mut g, EACH, "W");
    cite!(["608.2c"]; assert_eq!(g.players[0].life, 23));
    tied.put(2, BEAR, Zone::Battlefield);
    resolve(&mut tied, EACH, "W");
    cite!(["608.2c"]; assert_eq!(tied.players[0].life, 20));
}

#[test]
fn a_margin_is_the_least_difference_and_a_number_can_be_measured_against() {
    for (mine, theirs, gained) in [(0,1,0), (0,2,3), (0,3,3), (1,2,0), (1,3,3), (2,3,0), (2,4,3)] {
        let mut g = game(2);
        for _ in 0..mine { g.put(0, LAND, Zone::Battlefield); }
        for _ in 0..theirs { g.put(1, LAND, Zone::Battlefield); }
        resolve(&mut g, MARGIN, "W");
        cite!(["608.2c"]; case: format!("mine={mine}, theirs={theirs}"); assert_eq!(g.players[0].life, 20 + gained, "mine={mine}, theirs={theirs}"));
    }
    for (life, gained) in [(29, 0), (30, 1), (45, 1)] {
        let mut g = game(2);
        g.players[0].life = life;
        resolve(&mut g, START, "W");
        cite!(["608.2c"]; case: format!("life={life}"); assert_eq!(g.players[0].life, life + gained));
    }
}
