//! Alternative targets (601.2c, 702.174m): "Return target creature an opponent controls to its
//! owner's hand. If the gift was promised, instead return target nonland permanent an opponent
//! controls to its owner's hand." The spell requires the second target only if the gift was
//! promised, and then not the first. Into the Flood Maw.
use crate::support::cite;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const MAW: usize = 0;
const BEAR: usize = 1;
const RELIC: usize = 2;
const ISLAND: usize = 3;

fn game() -> (Game, ObjId, ObjId, ObjId) {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_faces(&CardDef::catalog(&faces)[name]).unwrap();
    let defs = vec![real("Into the Flood Maw"),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Relic", "{0}", "Artifact", "", None).unwrap(),
        real("Island")];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    let maw = g.put(0, MAW, Zone::Hand);
    let bear = g.put(1, BEAR, Zone::Battlefield);
    let relic = g.put(1, RELIC, Zone::Battlefield);
    g.add_mana(0, ir::mana::ManaSymbol::Color(ir::Color::Blue));
    (g, maw, bear, relic)
}
fn cast(maw: ObjId, gift: bool, targets: Vec<Vec<Target>>) -> Move {
    let cast = Move::Cast { card: maw, targets };
    crate::support::paid("U",if gift { Move::GiftTo(1, Box::new(cast)) } else { cast })
}
fn resolve(g: &mut Game) { while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); } }

#[test]
fn r601_2c_unpromised_only_the_creature_is_targeted() {
    let (mut g, maw, bear, relic) = game();
    for (wrong, why) in [(vec![vec![], vec![Target::Object(relic)]], "wrong number of targets: 0 chosen, it takes 1 (601.2c)"),
                         (vec![vec![Target::Object(bear)], vec![Target::Object(relic)]], "wrong number of targets: 1 chosen, it takes 0 (601.2c)")] {
        cite!(["702.174m"]; case: format!("{wrong:?}"); refused(why): g.clone().act(0, cast(maw, false, wrong.clone())));
    }
    cite!(["601.2c", "702.174m"]; ok: g.act(0, cast(maw, false, vec![vec![Target::Object(bear)], vec![]])));
    resolve(&mut g);
    assert!(g.players[1].hand.iter().any(|o| g.obj(*o).card == Some(BEAR)));
    assert!(g.battlefield.contains(&relic));
}

#[test]
fn r702_174m_promised_only_the_nonland_permanent_is_targeted() {
    let (mut g, maw, bear, relic) = game();
    for wrong in [vec![vec![Target::Object(bear)], vec![]], vec![vec![Target::Object(bear)], vec![Target::Object(relic)]]] {
        cite!(["702.174m"]; refused("wrong number of targets: 1 chosen, it takes 0 (601.2c)"): g.clone().act(0, cast(maw, true, wrong.clone())));
    }
    cite!(["601.2c", "702.174m"]; ok: g.act(0, cast(maw, true, vec![vec![], vec![Target::Object(relic)]])));
    resolve(&mut g);
    cite!(["608.2c"]; assert!(g.players[1].hand.iter().any(|o| g.obj(*o).card == Some(RELIC))));
    cite!(["608.2c"]; assert!(g.battlefield.contains(&bear), "the creature is not returned"));
    cite!(["702.174f"]; assert_eq!(g.battlefield.iter().filter(|o| g.obj(**o).token()).count(), 1, "the Fish"));
}

/// The listing offers each choice with the targets it has.
#[test]
fn r601_2c_each_gift_announcement_accepts_its_own_targets() {
    let (g, maw, bear, relic) = game();
    cite!(["601.2c"]; ok: g.clone().act(0, cast(maw, false, vec![vec![Target::Object(bear)], vec![]])));
    cite!(["601.2c", "702.174m"]; ok: g.clone().act(0, cast(maw, true, vec![vec![], vec![Target::Object(relic)]])));
}

/// An instead sentence that names the replaced sentence's target as well as its own is not
/// read: what it names would not have been chosen.
#[test]
fn r601_2c_alternative_targets_cannot_name_the_replaced_ones() {
    let text = "Gift a card\nTap target creature. If the gift was promised, instead tap that creature and target artifact.";
    let d=CardDef::new("Both", "{0}", "Instant", text, None).unwrap();
    crate::support::refused(game::play::check(&d).map_err(|e|e.0),"input reference: nothing before it supplies the required participant/cardinality");
}
