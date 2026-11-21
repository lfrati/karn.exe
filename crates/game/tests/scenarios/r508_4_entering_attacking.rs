//! Tokens put onto the battlefield attacking (508.4): their controller chooses what each
//! attacks as it enters, a defending player or a planeswalker one controls; such a creature
//! is attacking but never "attacked". Only an attacking player's creature, during combat,
//! enters attacking (506.3b).
use crate::support::cite;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const HERO: usize = 0;
const WALKER: usize = 1;
const RALLY: usize = 2;
const BEAR: usize = 3;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let hero = CardDef::from_face(faces.iter().find(|f| f.name == "Hero of Bladehold").unwrap()).unwrap();
    let defs = vec![
        hero,
        CardDef::planeswalker("Walker", "{0}", "Legendary Planeswalker — Garruk", "+1: You gain 1 life.", 5).unwrap(),
        CardDef::new("Rally", "{0}", "Instant", "Create a 1/1 white Soldier creature token that's tapped and attacking.", None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, 4, Zone::Library); } }
    g
}
fn soldiers(g: &Game) -> Vec<ObjId> { g.battlefield.iter().copied().filter(|o| g.obj(*o).token()).collect() }
fn to_attackers(g: &mut Game) {
    while g.decision != Some(Decision::Attackers { player: 0 }) { g.act(g.priority, Move::Pass).unwrap(); }
}
/// Pass until `part`, answering trigger order as listed and each defender choice with `pick`.
fn until(g: &mut Game, part: Part, pick: &mut dyn FnMut(&[Target]) -> Target) -> usize {
    let mut asked = 0;
    while g.part != part {
        match g.decision.clone() {
            Some(Decision::Order { player, abilities }) => g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(),
            Some(Decision::Choose { player: 0, choice: Choice::Defender { among, nth, of } }) => {
                asked += 1;
                assert_eq!((nth, of), (asked, 2), "which of the two soldiers this is for");
                cite!(["508.4"]; refused("that decision is another player's"): g.act(1, Move::Choose(Answer::Defender(among[0]))));
                cite!(["508.4"]; refused("choose one of the offered players or planeswalkers for it to attack (508.4)"): g.act(0, Move::Choose(Answer::Defender(Target::Player(0)))));
                let chosen = pick(&among);
                g.act(0, Move::Choose(Answer::Defender(chosen))).unwrap();
            }
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            Some(d) => panic!("{d:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    asked
}

#[test]
fn r508_4_the_controller_chooses_what_each_token_attacks() {
    let mut g = game();
    let hero = g.put(0, HERO, Zone::Battlefield);
    let walker = g.put(1, WALKER, Zone::Battlefield);
    g.put_counters(walker, "loyalty", 5);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(hero, Target::Player(1))])).unwrap();
    // one soldier at the planeswalker, the other at the player
    let mut n = 0;
    let asked = until(&mut g, Part::DeclareBlockers, &mut |among| {
        cite!(["508.4"]; assert_eq!(among, [Target::Player(1), Target::Object(walker)]));
        n += 1;
        if n == 1 { Target::Object(walker) } else { Target::Player(1) }
    });
    cite!(["508.4"]; assert_eq!(asked, 2, "each token its own choice"));
    let attacks: Vec<_> = soldiers(&g).into_iter().map(|s| g.combat.attackers.iter().find(|(a, _)| *a == s).map(|(_, t)| *t)).collect();
    cite!(["508.4"]; assert_eq!(attacks.len(), 2));
    cite!(["508.4"]; assert!(attacks.contains(&Some(Target::Object(walker))) && attacks.contains(&Some(Target::Player(1))), "{attacks:?}"));
    assert!(soldiers(&g).iter().all(|s| g.obj(*s).tapped));
    until(&mut g, Part::EndOfCombat, &mut |_| unreachable!());
    // whether battle cry resolved after the tokens were created decides their power
    let soldier_power = g.power(soldiers(&g)[0]).unwrap();
    cite!(["508.4"]; assert_eq!(g.players[1].life, 20 - 3 - soldier_power));
    cite!(["508.4"]; assert_eq!(g.obj(walker).counters.get("loyalty").copied(), Some(5 - soldier_power as u32)));
}

#[test]
fn r508_4_with_one_defender_there_is_nothing_to_ask() {
    let mut g = game();
    let hero = g.put(0, HERO, Zone::Battlefield);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(hero, Target::Player(1))])).unwrap();
    cite!(["508.4"]; assert_eq!(until(&mut g, Part::DeclareBlockers, &mut |_| unreachable!()), 0));
    cite!(["508.4"]; assert!(soldiers(&g).iter().all(|s| g.combat.attackers.contains(&(*s, Target::Player(1))))));
}

#[test]
fn r506_3b_outside_combat_a_token_enters_but_is_not_attacking() {
    let mut g = game();
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    let card = g.put(0, RALLY, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    let [soldier] = soldiers(&g)[..] else { panic!() };
    assert!(g.obj(soldier).tapped);
    cite!(["506.3b"]; assert!(g.combat.attackers.is_empty()));
}

#[test]
fn r506_3b_the_defending_players_token_is_not_attacking() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(bear, Target::Player(1))])).unwrap();
    while g.part != Part::DeclareBlockers { g.act(g.priority, Move::Pass).unwrap(); }
    if g.decision.is_some() { g.act(1, Move::Block(vec![])).unwrap(); }
    // player 1 casts it in the declare blockers step
    let card = g.put(1, RALLY, Zone::Hand);
    while g.priority != 1 { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(1, Move::Cast { card, targets: vec![] }).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    let [soldier] = soldiers(&g)[..] else { panic!() };
    assert_eq!(g.controller(soldier), 1);
    cite!(["506.3b"]; assert_eq!(g.combat.attackers, vec![(bear, Target::Player(1))]));
}
