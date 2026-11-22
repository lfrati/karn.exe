//! A player choosing objects as an instruction is followed (608.2d) is told what the chosen
//! objects are chosen for: discarded, returned, put on top. An answer naming an object that
//! was not offered is refused as such, not as a wrong count.
use crate::support::{cite, paid};
use game::state::{Answer, Choice, Decision, Placement};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::{Part, Verb};

fn real(faces: &[oracle::corpus::Face], name: &str) -> CardDef {
    CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap()
}
fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let defs = vec![real(&faces, "Duress"), real(&faces, "Rakdos Carnarium"), real(&faces, "Chittering Rats"),
        real(&faces, "Shock"), real(&faces, "Swamp")];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, 4, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
/// Pass, choosing targets as given, until an object choice is asked: its chooser, what for
/// and what it offers.
fn asked(g: &mut Game, targets: Vec<Vec<Target>>) -> (usize, (Verb, Option<Placement>), Vec<ObjId>) {
    let mut targets = Some(targets);
    for _ in 0..20 {
        match g.decision.clone() {
            Some(Decision::Choose { player, choice: Choice::Objects { verb, to, among, .. } }) => return (player, (verb, to), among),
            Some(Decision::Targets { player, .. }) => { g.act(player, Move::ChooseTargets(targets.take().unwrap())).unwrap(); }
            Some(d) => panic!("{d:?}"),
            None => { g.act(g.priority, Move::Pass).unwrap(); }
        }
    }
    panic!("no object choice");
}
fn refusal(g: &mut Game, player: usize, chosen: Vec<ObjId>) -> String {
    g.act(player, Move::Choose(Answer::Objects(chosen))).unwrap_err()
}

#[test]
fn r608_2d_duress_asks_for_a_card_to_discard() {
    let mut g = game();
    let duress = g.put(0, 0, Zone::Hand);
    let shocks = [g.put(1, 3, Zone::Hand), g.put(1, 3, Zone::Hand)];
    g.put(1, 4, Zone::Hand);
    g.players[0].pool.colors[2] = 1;
    g.act(0, paid("B", Move::Cast { card: duress, targets: vec![vec![Target::Player(1)]] })).unwrap();
    let (player, verb, among) = asked(&mut g, vec![]);
    cite!(["608.2d"]; assert_eq!((player, verb), (0, (Verb::Discard, None))));
    cite!(["608.2d"]; assert_eq!(among, shocks));
}

#[test]
fn r608_2d_rakdos_carnarium_asks_for_a_land_to_return() {
    let mut g = game();
    let swamp = g.put(0, 4, Zone::Battlefield);
    let carnarium = g.put(0, 1, Zone::Hand);
    g.act(0, Move::PlayLand(carnarium)).unwrap();
    let (player, verb, among) = asked(&mut g, vec![]);
    cite!(["608.2d"]; assert_eq!((player, verb), (0, (Verb::Move, Some(Placement::Zone(Zone::Hand))))));
    assert_eq!(among.len(), 2);
    cite!(["608.2d"]; assert!(among.contains(&swamp)));
    let theirs = g.put(1, 4, Zone::Battlefield);
    let refused = refusal(&mut g, 0, vec![theirs]);
    cite!(["608.2d"]; assert_eq!(refused, "a chosen object is not among the offered objects to return (608.2d)"));
}

#[test]
fn r608_2d_chittering_rats_asks_for_a_card_to_put_on_top() {
    let mut g = game();
    let rats = g.put(0, 2, Zone::Hand);
    let theirs = [g.put(1, 3, Zone::Hand), g.put(1, 4, Zone::Hand)];
    g.players[0].pool.colors[2] = 2;
    g.players[0].pool.colorless = 1;
    g.act(0, paid("CBB", Move::Cast { card: rats, targets: vec![] })).unwrap();
    let (player, verb, among) = asked(&mut g, vec![vec![Target::Player(1)]]);
    cite!(["608.2d"]; assert_eq!((player, verb), (1, (Verb::Move, Some(Placement::Library(ir::action::LibraryPosition::Top))))));
    cite!(["608.2d"]; assert_eq!(among, theirs));
    let refused = refusal(&mut g, 1, vec![rats]);
    cite!(["608.2d"]; assert_eq!(refused, "a chosen object is not among the offered objects to put (608.2d)"));
}

#[test]
fn r608_2d_an_object_not_offered_is_refused_as_not_offered() {
    let mut g = game();
    let duress = g.put(0, 0, Zone::Hand);
    g.put(1, 3, Zone::Hand);
    g.put(1, 3, Zone::Hand);
    let land = g.put(1, 4, Zone::Hand);
    g.players[0].pool.colors[2] = 1;
    g.act(0, paid("B", Move::Cast { card: duress, targets: vec![vec![Target::Player(1)]] })).unwrap();
    let (_, _, among) = asked(&mut g, vec![]);
    let not_offered = refusal(&mut g, 0, vec![land]);
    cite!(["608.2d"]; assert_eq!(not_offered, "a chosen object is not among the offered objects to discard (608.2d)"));
    let too_many = refusal(&mut g, 0, among.clone());
    cite!(["608.2d"]; assert_eq!(too_many, "choose exactly 1 of them to discard (608.2d)"));
    let twice = refusal(&mut g, 0, vec![among[0], among[0]]);
    cite!(["608.2d"]; assert_eq!(twice, "choose each object to discard once (608.2d)"));
}


/// Malevolent Rumble puts a card from the library into the hand; its
/// refusal must not say "to return".
#[test]
fn r608_2d_malevolent_rumble_asks_for_a_card_to_put_into_the_hand() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let defs = vec![real(&faces, "Malevolent Rumble"), real(&faces, "Swamp"),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap()];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, 2, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    let rumble = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[4] = 1;
    g.players[0].pool.colorless = 1;
    g.act(0, paid("CG", Move::Cast { card: rumble, targets: vec![] })).unwrap();
    let mut asked_may = false;
    for _ in 0..10 {
        match g.decision.clone() {
            Some(Decision::Choose { player, choice: Choice::May { possible: true } }) => {
                // what is optional: putting a card into the hand
                assert_eq!(g.optional(), Some((Verb::Move, Some(Placement::Zone(Zone::Hand)))));
                asked_may = true;
                g.act(player, Move::Choose(Answer::Yes)).unwrap()
            }
            Some(_) => break,
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    assert!(asked_may, "\"You may put\" is asked first");
    let Some(Decision::Choose { player: 0, choice: Choice::Objects { verb, to, .. } }) = g.decision.clone() else { panic!("{:?}", g.decision) };
    cite!(["608.2d"]; assert_eq!((verb, to), (Verb::Move, Some(Placement::Zone(Zone::Hand)))));
    let refused = refusal(&mut g, 0, vec![rumble]);
    cite!(["608.2d"]; assert_eq!(refused, "a chosen object is not among the offered objects to put (608.2d)"));
}
