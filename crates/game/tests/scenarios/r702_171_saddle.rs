//! Saddle (702.171): "Saddle N" means "Tap any number of other untapped creatures you control
//! with total power N or greater: This permanent becomes saddled until end of turn. Activate
//! only as a sorcery." Saddled is a designation that lasts until end of turn or until the
//! permanent leaves the battlefield, not a copiable value (702.171b). A creature tapped to pay
//! that cost saddles the permanent (702.171c).
use crate::support::cite;
use game::state::Decision;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::Part;

const NAMES: [&str; 10] = ["Plains", "Alacrian Jaguar", "Canyon Vaulter", "Dynamite Diver", "Grizzly Bears", "Smuggler's Copter", "Hotshot Mechanic", "Giant Beaver", "Rambling Possum", "Fortune, Loyal Steed"];
const PLAINS: usize = 0;
const JAGUAR: usize = 1;
const VAULTER: usize = 2;
const DIVER: usize = 3;
const BEARS: usize = 4;
const MECHANIC: usize = 6;
const BEAVER: usize = 7;
const POSSUM: usize = 8;
const FORTUNE: usize = 9;
const BIG: usize = 10;
const BOLT: usize = 11;
const CART: usize = 12;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let mut defs: Vec<_> = NAMES.map(|n| CardDef::from_faces(&catalog[n]).unwrap()).to_vec();
    defs.push(CardDef::new("Big Mount", "{3}", "Creature — Horse Mount", "Saddle 3", Some((3, 3))).unwrap());
    defs.push(CardDef::new("Bolt", "{0}", "Instant", "Bolt deals 3 damage to any target.", None).unwrap());
    defs.push(CardDef::new("Cart", "{1}", "Artifact — Vehicle", "Crew 1", Some((3, 3))).unwrap());
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, PLAINS, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}

/// Printed keyword rows stay present before their derived activation. The caller
/// supplies the index from its declared fixture, rather than searching live abilities.
fn saddle(g: &mut Game, mount: ObjId, ability: usize, creatures: &[ObjId]) -> Result<(), String> {
    g.act(0, Move::Activate { source: mount, ability, targets: vec![], paying: vec![creatures.to_vec()] })
}
fn resolve(g: &mut Game) { while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); } }
/// Player 0 attacks player 1 with these; the triggers resolve.
fn attack(g: &mut Game, attackers: &[ObjId]) {
    while g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Attack(attackers.iter().map(|a| (*a, Target::Player(1))).collect())).unwrap();
    while !g.stack.is_empty() || matches!(g.decision, Some(Decision::Order { .. })) {
        let Some((player, game::legal::Asked::Priority)) = g.asked() else {
            panic!("unexpected attack-trigger question: {:?}", g.asked());
        };
        g.act(player, Move::Pass).unwrap();
    }
    assert_eq!(g.part, Part::DeclareAttackers);
}
fn saddles(g: &Game) -> Vec<(ObjId, Vec<ObjId>)> {
    g.log.iter().filter_map(|e| match e { Event::Saddles { mount, creatures } => Some((*mount, creatures.clone())), _ => None }).collect()
}
fn saddled(g: &Game, o: ObjId) -> bool {
    game::snapshot::Snapshot::of(g, Some(1)).objects.into_iter().find(|s| s.id == o).is_some_and(|s| s.permanent.is_some_and(|p| p.saddled))
}

#[test]
fn r702_171a_saddling_taps_other_creatures_and_the_mount_is_saddled_until_end_of_turn() {
    let mut g = game();
    let jaguar = g.put(0, JAGUAR, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    saddle(&mut g, jaguar, 3, &[bears]).unwrap();
    cite!(["702.171a"]; assert!(g.obj(bears).tapped));
    cite!(["702.171c"]; assert_eq!(saddles(&g), [(jaguar, vec![bears])]));
    cite!(["702.171a"]; assert!(!saddled(&g, jaguar), "the ability uses the stack"));
    resolve(&mut g);
    cite!(["702.171a", "702.171b"]; assert!(saddled(&g, jaguar), "every seat sees it saddled"));
    // until end of turn (514.2)
    while !(g.active == 1 && g.part == Part::PrecombatMain) {
        match g.decision.clone() {
            Some(Decision::Attackers { .. }) => g.act(0, Move::Attack(vec![])).unwrap(),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    cite!(["702.171b", "514.2"]; assert!(!saddled(&g, jaguar)));
}

#[test]
fn r702_171a_saddle_only_as_a_sorcery() {
    // with a spell on the stack
    let mut g = game();
    let jaguar = g.put(0, JAGUAR, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let ability = 3; // vigilance, attack trigger, Saddle; then derived Saddle activation
    let bolt = g.put(0, BOLT, Zone::Hand);
    g.act(0, Move::Cast { card: bolt, targets: vec![vec![Target::Player(1)]] }).unwrap();
    cite!(["702.171a"]; refused("that ability is activated only as a sorcery"): act_unchanged(&mut g, Move::Activate { source: jaguar, ability, targets: vec![], paying: vec![vec![bears]] }));
    // in combat
    let mut g = game();
    let jaguar = g.put(0, JAGUAR, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    assert_eq!(g.part, Part::BeginningOfCombat);
    cite!(["702.171a"]; refused("that ability is activated only as a sorcery"): act_unchanged(&mut g, Move::Activate { source: jaguar, ability: 3, targets: vec![], paying: vec![vec![bears]] }));
}

#[test]
fn r702_171a_attacks_while_saddled_triggers_only_when_saddled() {
    // saddled: Alacrian Jaguar gets +2/+2
    let mut g = game();
    let jaguar = g.put(0, JAGUAR, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    saddle(&mut g, jaguar, 3, &[bears]).unwrap();
    resolve(&mut g);
    attack(&mut g, &[jaguar]);
    cite!(["702.171a", "603.2"]; assert_eq!((g.power(jaguar), g.toughness(jaguar)), (Some(6), Some(6))));
    // not saddled: no trigger
    let mut g = game();
    let jaguar = g.put(0, JAGUAR, Zone::Battlefield);
    g.put(0, BEARS, Zone::Battlefield);
    attack(&mut g, &[jaguar]);
    cite!(["603.2"]; assert_eq!((g.power(jaguar), g.toughness(jaguar)), (Some(4), Some(4))));
}

#[test]
fn r702_171c_saddles_a_mount_or_crews_a_vehicle_during_your_main_phase_triggers() {
    // Canyon Vaulter: "Whenever this creature saddles a Mount or crews a Vehicle during your
    // main phase, that Mount or Vehicle gains flying until end of turn."
    let mut g = game();
    let jaguar = g.put(0, JAGUAR, Zone::Battlefield);
    let vaulter = g.put(0, VAULTER, Zone::Battlefield);
    saddle(&mut g, jaguar, 3, &[vaulter]).unwrap();
    resolve(&mut g);
    cite!(["702.171c", "603.2"]; assert!(g.has(jaguar, "Flying")));
    // crewing in the main phase, too
    let mut g = game();
    let cart = g.put(0, CART, Zone::Battlefield);
    let vaulter = g.put(0, VAULTER, Zone::Battlefield);
    let crew = 1; // printed Crew row0, derived activation1
    g.act(0, Move::Activate { source: cart, ability: crew, targets: vec![], paying: vec![vec![vaulter]] }).unwrap();
    resolve(&mut g);
    cite!(["702.122b", "603.2"]; assert!(g.has(cart, "Flying")));
    // crewing at the beginning of combat: not a main phase (505.1)
    let mut g = game();
    let cart = g.put(0, CART, Zone::Battlefield);
    let vaulter = g.put(0, VAULTER, Zone::Battlefield);
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    assert_eq!(g.part, Part::BeginningOfCombat);
    g.act(0, Move::Activate { source: cart, ability: crew, targets: vec![], paying: vec![vec![vaulter]] }).unwrap();
    resolve(&mut g);
    cite!(["505.1", "603.2"]; assert!(!g.has(cart, "Flying")));
}

#[test]
fn r702_171a_a_creature_saddles_and_crews_as_though_its_power_were_greater() {
    // Dynamite Diver (1/1): "This creature saddles Mounts and crews Vehicles as though its
    // power were 2 greater." It pays Saddle 3 alone.
    let mut g = game();
    let mount = g.put(0, BIG, Zone::Battlefield);
    let diver = g.put(0, DIVER, Zone::Battlefield);
    g.put(0, BEARS, Zone::Battlefield);
    saddle(&mut g, mount, 1, &[diver]).unwrap();
    resolve(&mut g);
    cite!(["702.171a"]; assert!(saddled(&g, mount)));
    // its power is otherwise unchanged
    assert_eq!(g.power(diver), Some(1));
    // the Bears alone do not pay it
    let mut g = game();
    let mount = g.put(0, BIG, Zone::Battlefield);
    g.put(0, DIVER, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    cite!(["702.171a"]; refused("chosen creatures have total power less than 3"): act_unchanged(&mut g, Move::Activate { source: mount, ability: 1, targets: vec![], paying: vec![vec![bears]] }));
    // Crew 3 requires the Diver's extra two power: its printed power 1 cannot pay.
    let mut g = game();
    let def = CardDef::new("Crew three", "{0}", "Artifact — Vehicle", "Crew 3", Some((3, 3))).unwrap();
    let index = g.defs.len();
    std::sync::Arc::make_mut(&mut g.defs).push(def);
    let copter = g.put(0, index, Zone::Battlefield);
    let diver = g.put(0, DIVER, Zone::Battlefield);
    cite!(["702.122a"]; ok: saddle(&mut g, copter, 1, &[diver]));

}

#[test]
fn r702_122a_a_creature_that_crews_as_though_its_power_were_greater_does_not_saddle_so() {
    // Hotshot Mechanic (2/1): "This creature crews Vehicles as though its power were 2
    // greater." Toward a crew cost it counts 4; toward a saddle cost, its power, 2.
    let mut g = game();
    let def = CardDef::new("Crew four", "{0}", "Artifact — Vehicle", "Crew 4", Some((4, 4))).unwrap();
    let index = g.defs.len();
    std::sync::Arc::make_mut(&mut g.defs).push(def);
    let copter = g.put(0, index, Zone::Battlefield);
    let mount = g.put(0, BIG, Zone::Battlefield);
    let mechanic = g.put(0, MECHANIC, Zone::Battlefield);
    g.put(0, BEARS, Zone::Battlefield);
    cite!(["702.171a"]; refused("chosen creatures have total power less than 3"): act_unchanged(&mut g, Move::Activate { source: mount, ability: 1, targets: vec![], paying: vec![vec![mechanic]] }));
    cite!(["702.122a"]; ok: saddle(&mut g, copter, 1, &[mechanic]));
}

#[test]
fn r702_171c_a_creature_that_saddled_it_this_turn() {
    // Giant Beaver: "Whenever this creature attacks while saddled, put a +1/+1 counter on
    // target creature that saddled it this turn." Saddle 3: the Bears and the Diver saddle it;
    // the Jaguar, which did not, is no legal target.
    let mut g = game();
    let beaver = g.put(0, BEAVER, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let diver = g.put(0, DIVER, Zone::Battlefield);
    let jaguar = g.put(0, JAGUAR, Zone::Battlefield);
    saddle(&mut g, beaver, 3, &[bears, diver]).unwrap();
    resolve(&mut g);
    while g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Attack(vec![(beaver, Target::Player(1))])).unwrap();
    cite!(["603.3d"]; assert!(matches!(g.decision, Some(Decision::Targets { player: 0, .. })), "{:?}", g.decision));
    cite!(["702.171c", "115.1"]; refused("an illegal target, Alacrian Jaguar"): act_unchanged(&mut g, Move::ChooseTargets(vec![vec![Target::Object(jaguar)]])));
    cite!(["702.171c", "115.1"]; refused("an illegal target, Giant Beaver"): act_unchanged(&mut g, Move::ChooseTargets(vec![vec![Target::Object(beaver)]])));
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(diver)]])).unwrap();
    resolve(&mut g);
    cite!(["702.171c"]; assert_eq!(g.obj(diver).counters.get("+1/+1").copied(), Some(1)));
    // a creature that crewed a Vehicle did not saddle it
    let mut g = game();
    let beaver = g.put(0, BEAVER, Zone::Battlefield);
    let cart = g.put(0, CART, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let diver = g.put(0, DIVER, Zone::Battlefield);
    let jaguar = g.put(0, JAGUAR, Zone::Battlefield);
    let crew = 1; // printed Crew row0, derived activation1
    g.act(0, Move::Activate { source: cart, ability: crew, targets: vec![], paying: vec![vec![jaguar]] }).unwrap();
    resolve(&mut g);
    saddle(&mut g, beaver, 3, &[bears, diver]).unwrap();
    resolve(&mut g);
    while g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Attack(vec![(beaver, Target::Player(1))])).unwrap();
    cite!(["702.171c", "702.122b"]; refused("an illegal target, Alacrian Jaguar"): act_unchanged(&mut g, Move::ChooseTargets(vec![vec![Target::Object(jaguar)]])));
    cite!(["702.171c"]; ok: g.act(0, Move::ChooseTargets(vec![vec![Target::Object(bears)]])));
}

#[test]
fn r702_171c_return_any_number_of_creatures_that_saddled_it() {
    // Rambling Possum: "Whenever this creature attacks while saddled, it gets +1/+2 until end
    // of turn. Then you may return any number of creatures that saddled it this turn to their
    // owner's hand." The Bears and the Diver saddled it; the Jaguar did not.
    let mut g = game();
    let possum = g.put(0, POSSUM, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let diver = g.put(0, DIVER, Zone::Battlefield);
    let jaguar = g.put(0, JAGUAR, Zone::Battlefield);
    saddle(&mut g, possum, 2, &[bears, diver]).unwrap();
    resolve(&mut g);
    while g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Attack(vec![(possum, Target::Player(1))])).unwrap();
    // Resolve the attack trigger to its optional return. This is the declared
    // choice boundary; no failed answer is used to decide which action to submit.
    while g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
    let Some((0, game::legal::Asked::Select { among, min: 0, max: 2 })) = g.asked() else {
        panic!("the two creatures that saddled it may return: {:?}", g.asked());
    };
    assert_eq!(among.iter().copied().collect::<std::collections::BTreeSet<_>>(), [bears, diver].into_iter().collect());
    cite!(["702.171c", "608.2d"]; refused("a chosen object is not among the offered objects"): act_unchanged(&mut g, Move::Choose(game::state::Answer::Objects(vec![jaguar]))));
    g.act(0, Move::Choose(game::state::Answer::Objects(vec![bears]))).unwrap();
    // Selected objects are bound before the optional movement is consented to.
    assert_eq!(g.asked(), Some((0, game::legal::Asked::May)));
    g.act(0, Move::Choose(game::state::Answer::Yes)).unwrap();
    resolve(&mut g);
    assert!(g.stack.is_empty());
    cite!(["702.171a"]; assert_eq!((g.power(possum), g.toughness(possum)), (Some(4), Some(5))));
    cite!(["702.171c", "608.2d"]; assert!(g.obj(bears).zone != Zone::Battlefield && g.players[0].hand.iter().any(|c| g.obj(*c).card == Some(BEARS))));
    cite!(["608.2d"]; assert!(g.battlefield.contains(&diver) && g.battlefield.contains(&jaguar)));
}

#[test]
fn r603_7a_at_end_of_combat_exile_it_and_a_creature_that_saddled_it_then_return_them() {
    // Fortune, Loyal Steed: "Whenever Fortune attacks while saddled, at end of combat, exile it
    // and up to one creature that saddled it this turn, then return those cards to the
    // battlefield under their owner's control."
    let mut g = game();
    let fortune = g.put(0, FORTUNE, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let jaguar = g.put(0, JAGUAR, Zone::Battlefield);
    saddle(&mut g, fortune, 3, &[bears]).unwrap();
    resolve(&mut g);
    attack(&mut g, &[fortune]);
    // nothing yet: the delayed ability waits for the end of combat step (603.7a)
    cite!(["603.7a"]; assert!(g.battlefield.contains(&fortune)));
    // Keep a declared pair of Plains on top when Fortune's reentry triggers scry2.
    let second = g.put(0, PLAINS, Zone::Library);
    let top = g.put(0, PLAINS, Zone::Library);
    loop {
        match g.asked() {
            Some((player, game::legal::Asked::Blockers { .. })) => g.act(player, Move::Block(vec![])).unwrap(),
            Some((player, game::legal::Asked::Priority)) => g.act(player, Move::Pass).unwrap(),
            Some((0, game::legal::Asked::Select { among, min: 0, max: 1 })) => {
                assert_eq!(among, vec![bears]);
                break;
            }
            other => panic!("expected delayed exile selection: {other:?}"),
        }
    }
    cite!(["702.171c", "608.2d"]; refused("a chosen object is not among the offered objects"): act_unchanged(&mut g, Move::Choose(game::state::Answer::Objects(vec![jaguar]))));
    g.act(0, Move::Choose(game::state::Answer::Objects(vec![bears]))).unwrap();
    while g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
    let Some(Decision::Choose { player: 0, choice: game::state::Choice::Arrange { verb: ir::Verb::Scry, cards } }) = &g.decision else {
        panic!("Fortune's reentry scries two: {:?}", g.decision);
    };
    assert_eq!(cards, &vec![top, second]);
    g.act(0, Move::Choose(game::state::Answer::Arrange { top: vec![top, second], away: vec![] })).unwrap();
    while g.part != Part::PostcombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    // both came back as new objects (400.7), untapped; the Jaguar stayed
    let named = |card| g.battlefield.iter().filter(|o| g.obj(**o).card == Some(card)).count();
    cite!(["603.7a", "610.3"]; assert!(!g.battlefield.contains(&fortune) && !g.battlefield.contains(&bears)));
    cite!(["610.3"]; assert_eq!((named(FORTUNE), named(BEARS)), (1, 1)));
    cite!(["400.7"]; assert!(g.battlefield.iter().filter(|o| g.obj(**o).card == Some(BEARS)).all(|o| !g.obj(*o).tapped)));
    assert!(g.battlefield.contains(&jaguar));
}

#[track_caller]
fn act_unchanged(g: &mut Game, action: Move) -> Result<(), String> {
    let before = format!("{g:?}");
    let result = g.act(0, action);
    assert_eq!(format!("{g:?}"), before);
    result
}
