//! Alternative targets (601.2c, 702.33g, 702.174m) with the admitted cards: Tear
//! Asunder, Blood Beckoning ("two target"), Divine Resilience ("any number of target"); a
//! promised Into the Flood Maw whose only target is gone does not give its gift (702.174j);
//! a target change (115.7) or a copy's new targets (707.10c) keep to the slot that was chosen.
use crate::support::cite;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const TEAR: usize = 0;
const BECKONING: usize = 1;
const RESILIENCE: usize = 2;
const MAW: usize = 3;
const REDIRECT: usize = 4;
const TWIN: usize = 5;
const BOUNCE: usize = 6;
const BEAR: usize = 7;
const RELIC: usize = 8;
const ISLAND: usize = 9;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_faces(&CardDef::catalog(&faces)[name]).unwrap();
    let c = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).expect(name);
    let defs = vec![real("Tear Asunder"), real("Blood Beckoning"), real("Divine Resilience"), real("Into the Flood Maw"),
        real("Redirect"),
        c("Twin", "Instant", "Copy target spell. You may choose new targets for the copy.", None),
        c("Bounce", "Instant", "Return target nonland permanent to its owner's hand.", None),
        c("Bear", "Creature — Bear", "", Some((2, 2))),
        c("Relic", "Artifact", "", None),
        real("Island")];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    for p in 0..2 { g.players[p].pool.colors = [20; 5]; g.players[p].pool.colorless = 20; }
    g
}
fn o(x: ObjId) -> Vec<Target> { vec![Target::Object(x)] }
fn kicked(kicks: u32, card: ObjId, targets: Vec<Vec<Target>>) -> Move {
    Move::WithKicks(kicks, Box::new(Move::Cast { card, targets }))
}
/// Resolve everything; a target change or a copy's new targets put `to` in each slot that has
/// a target, any "may" is yes.
fn resolve(g: &mut Game, to: Option<ObjId>) {
    for _ in 0..60 {
        match g.decision.clone() {
            None if g.stack.is_empty() => return,
            None => g.act(g.priority, Move::Pass).unwrap(),
            Some(Decision::Choose { player, choice: Choice::May { possible: true } }) => g.act(player, Move::Choose(Answer::Yes)).unwrap(),
            Some(Decision::Choose { player, choice: Choice::ChangeTargets { original, .. } | Choice::Retarget { original, .. } }) => {
                let new = original.iter().map(|s| if s.is_empty() { vec![] } else { o(to.expect("a new target")) }).collect();
                g.act(player, Move::Choose(Answer::Targets(new))).unwrap()
            }
            Some(d) => panic!("{d:?}"),
        }
    }
    panic!("did not resolve");
}

#[test]
fn tear_asunder_kicked_exiles_a_creature_unkicked_cannot_target_one() {
    let mut g = game();
    let tear = g.put(0, TEAR, Zone::Hand);
    let bear = g.put(1, BEAR, Zone::Battlefield);
    let relic = g.put(1, RELIC, Zone::Battlefield);
    cite!(["601.2c", "115.1"]; refused("an illegal target, Bear: it is neither an artifact nor an enchantment (115.1)"): g.clone().act(0, kicked(0, tear, vec![o(bear), vec![]])));
    let mut unkicked = g.clone();
    unkicked.act(0, crate::support::paid("CG",kicked(0, tear, vec![o(relic), vec![]]))).unwrap();
    resolve(&mut unkicked, None);
    assert_eq!(unkicked.obj(relic).zone, Zone::Gone);
    cite!(["601.2c", "702.33g"]; ok: g.act(0, crate::support::paid("CCBG",kicked(1, tear, vec![vec![], o(bear)]))));
    resolve(&mut g, None);
    cite!(["601.2c", "702.33g"]; assert!(!g.battlefield.contains(&bear)));
    assert!(g.battlefield.contains(&relic));
}

#[test]
fn blood_beckoning_kicked_returns_exactly_two() {
    let mut g = game();
    let beckoning = g.put(0, BECKONING, Zone::Hand);
    let a = g.put(0, BEAR, Zone::Graveyard);
    let b = g.put(0, BEAR, Zone::Graveyard);
    let two = vec![Target::Object(a), Target::Object(b)];
    cite!(["601.2c"]; refused("wrong number of targets: 2 chosen, it takes 1 (601.2c)"): g.clone().act(0, kicked(0, beckoning, vec![two.clone(), vec![]])));
    cite!(["601.2c"]; refused("wrong number of targets: 1 chosen, it takes 2 (601.2c)"): g.clone().act(0, kicked(1, beckoning, vec![vec![], o(a)])));
    cite!(["601.2c", "702.33g"]; ok: g.act(0, crate::support::paid("CCCB",kicked(1, beckoning, vec![vec![], two]))));
    resolve(&mut g, None);
    cite!(["601.2c"]; assert_eq!(g.players[0].hand.iter().filter(|x| g.obj(**x).card == Some(BEAR)).count(), 2));
}

/// "any number of target creatures you control": kicked, two of them, or none.
#[test]
fn divine_resilience_kicked_takes_any_number() {
    let mut g = game();
    let a = g.put(0, BEAR, Zone::Battlefield);
    let b = g.put(0, BEAR, Zone::Battlefield);
    let both = vec![Target::Object(a), Target::Object(b)];
    let spell = g.put(0, RESILIENCE, Zone::Hand);
    cite!(["601.2c"]; refused("wrong number of targets: 2 chosen, it takes 1 (601.2c)"): g.clone().act(0, kicked(0, spell, vec![both.clone(), vec![]])));
    let mut none = g.clone();
    cite!(["601.2c"]; ok: none.act(0, crate::support::paid("CCWW",kicked(1, spell, vec![vec![], vec![]]))));
    resolve(&mut none, None);
    cite!(["601.2c", "702.33g"]; ok: g.act(0, crate::support::paid("CCWW",kicked(1, spell, vec![vec![], both]))));
    resolve(&mut g, None);
    let mut murder = g.clone();
    for x in [a, b] { murder.destroy(x); }
    assert!(murder.battlefield.contains(&a) && murder.battlefield.contains(&b), "both indestructible");
}

/// Promised, its only target bounced in response: it does not resolve, so no Fish (702.174j).
#[test]
fn a_promised_maw_whose_target_left_gives_nothing() {
    let mut g = game();
    let maw = g.put(0, MAW, Zone::Hand);
    let relic = g.put(1, RELIC, Zone::Battlefield);
    g.act(0, crate::support::paid("U",Move::GiftTo(1, Box::new(Move::Cast { card: maw, targets: vec![vec![], o(relic)] })))).unwrap();
    let bounce = g.put(1, BOUNCE, Zone::Hand);
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Cast { card: bounce, targets: vec![o(relic)] }).unwrap();
    resolve(&mut g, None);
    cite!(["702.174j", "608.2b"]; assert_eq!(g.battlefield.iter().filter(|x| g.obj(**x).token()).count(), 0, "no Fish"));
}

/// A kicked Tear Asunder redirected keeps its kicked target: a creature may be the new one;
/// unkicked, a creature may not, and the target stays.
#[test]
fn redirect_keeps_to_the_chosen_slot() {
    for kicks in [1, 0] {
        let mut g = game();
        let tear = g.put(0, TEAR, Zone::Hand);
        let relic = g.put(1, RELIC, Zone::Battlefield);
        let bear = g.put(1, BEAR, Zone::Battlefield);
        let targets = if kicks == 1 { vec![vec![], o(relic)] } else { vec![o(relic), vec![]] };
        g.act(0, crate::support::paid(if kicks == 0 { "CG" } else { "CCBG" },kicked(kicks, tear, targets))).unwrap();
        let spell = *g.stack.last().unwrap();
        let redirect = g.put(0, REDIRECT, Zone::Hand);
        g.act(0, crate::support::paid("UU",Move::Cast { card: redirect, targets: vec![o(spell)] })).unwrap();
        while !matches!(g.decision, Some(Decision::Choose { choice: Choice::ChangeTargets { .. }, .. })) { g.act(g.priority, Move::Pass).unwrap(); }
        let Some(Decision::Choose { choice: Choice::ChangeTargets { original, .. }, .. }) = g.decision.clone() else { unreachable!() };
        assert_eq!(original, if kicks == 1 { vec![vec![], o(relic)] } else { vec![o(relic), vec![]] });
        let new: Vec<_> = original.iter().map(|s| if s.is_empty() { vec![] } else { o(bear) }).collect();
        let moved = g.clone().act(0, Move::Choose(Answer::Targets(new.clone())));
        if kicks == 1 {
            cite!(["115.7d"]; ok: g.act(0, Move::Choose(Answer::Targets(new))));
            resolve(&mut g, None);
            cite!(["115.7d", "702.33g"]; assert!(!g.battlefield.contains(&bear) && g.battlefield.contains(&relic), "kicked: the bear instead"));
        } else {
            cite!(["115.7d"]; refused("a changed target must be legal without invalidating an unchanged target (115.7d)"): moved);
            // a target put in the slot the spell did not choose is not taken either
            cite!(["601.2c"]; refused("new targets keep each target group's number of targets, chosen as it was put on the stack, and its division (601.2c, 115.7f)"): g.clone().act(0, Move::Choose(Answer::Targets(vec![vec![], o(bear)]))));
        }
    }
}

/// A copy of a kicked Tear Asunder is kicked (707.10): its new target may be a creature.
#[test]
fn a_copy_of_a_kicked_spell_keeps_its_alternative_target() {
    let mut g = game();
    let tear = g.put(0, TEAR, Zone::Hand);
    let relic = g.put(1, RELIC, Zone::Battlefield);
    let bear = g.put(1, BEAR, Zone::Battlefield);
    g.act(0, crate::support::paid("CCBG",kicked(1, tear, vec![vec![], o(relic)]))).unwrap();
    let spell = *g.stack.last().unwrap();
    let twin = g.put(0, TWIN, Zone::Hand);
    g.act(0, Move::Cast { card: twin, targets: vec![o(spell)] }).unwrap();
    resolve(&mut g, Some(bear));
    cite!(["707.10", "707.10c"]; assert!(!g.battlefield.contains(&bear), "the copy exiled the bear"));
    assert!(!g.battlefield.contains(&relic), "the original exiled the relic");
}
