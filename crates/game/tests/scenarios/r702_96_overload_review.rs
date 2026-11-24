//! Review of overloaded spells whose "each" is said of more than damage or a zone change
//! (702.96a): what each creature becomes, gets and can't have done to it.
use crate::support::*;
use game::announcement::CastCost;

fn overloaded(t: &mut T, name: &str, mana: &str) {
    let card = t.find_in(0, Zone::Hand, name);
    let row = t.g.def(card).unwrap().abilities().iter().position(|a| matches!(a, ir::Ability::Keywords(items)
        if items.iter().any(|i| i.keyword.name() == "Overload"))).expect("an overload ability");
    let cast = Move::CastFor(CastCost::Alternative(row), Box::new(Move::Cast { card, targets: vec![] }));
    t.g.players[0].pool = mana_pool(mana);
    cite!(["702.96a", "601.2b"]; ok: t.g.act(0, Move::Paying(mana_pool(mana), Box::new(cast))));
    t.resolve();
}

fn board(hand: &[&str]) -> T {
    Board::new(2).fill(5).at(Part::PrecombatMain).hand(0, hand)
        .named(0, Zone::Battlefield, "my elves", "Llanowar Elves")
        .named(0, Zone::Battlefield, "my bears", "Grizzly Bears")
        .named(1, Zone::Battlefield, "elves", "Llanowar Elves")
        .named(1, Zone::Battlefield, "bears", "Grizzly Bears")
        .build()
}

#[test]
fn teleportal_overloaded_makes_each_of_your_creatures_unblockable() {
    // "Each creature you control gets +1/+0 until end of turn and can't be blocked this turn."
    let mut t = board(&["Teleportal"]);
    overloaded(&mut t, "Teleportal", "CCCUR");
    let (elves, bears, their) = (t.id("my elves"), t.id("my bears"), t.id("bears"));
    cite!(["702.96a", "613.4c"]; assert_eq!((t.g.power(elves), t.g.power(bears), t.g.power(their)), (Some(2), Some(3), Some(2))));
    let mut asked_to_block = false;
    for _ in 0..50 {
        if t.g.part == Part::PostcombatMain { break; }
        match t.g.decision.clone() {
            Some(Decision::Attackers { player }) => t.g.act(player, Move::Attack(vec![(elves, Target::Player(1)), (bears, Target::Player(1))])).unwrap(),
            Some(Decision::Blockers { player }) => {
                asked_to_block = true;
                t.g.act(player, Move::Block(vec![])).unwrap();
            }
            _ => t.g.act(t.g.decision_maker(t.g.priority), Move::Pass).unwrap(),
        }
    }
    // neither attacker can be blocked: the defending player is not asked (509.1)
    cite!(["509.1", "702.96a"]; assert!(!asked_to_block));
    t.expect().rules(["510.1", "702.96a"]).life(1, 20 - 2 - 3).check();
}

#[test]
fn scale_up_overloaded_makes_each_of_your_creatures_a_6_4_wurm_and_none_of_theirs() {
    // "Until end of turn, each creature you control becomes a green Wurm with base power and
    // toughness 6/4."
    let mut t = board(&["Scale Up"]);
    overloaded(&mut t, "Scale Up", "CCCCGG");
    for label in ["my elves", "my bears"] {
        let seen = t.g.seen(t.id(label), None);
        cite!(["702.96a", "613.4b", "205.1a"]; case: label; assert!(seen.pt == Some((6, 4)) && seen.subtypes.iter().any(|s| s.name() == "Wurm")
            && seen.colors == vec![ir::Color::Green]));
    }
    // its creature types are replaced by Wurm, not added to (205.1a)
    cite!(["205.1a"]; assert!(!t.g.seen(t.id("my bears"), None).subtypes.iter().any(|s| s.name() == "Bear")));
    cite!(["702.96a"]; assert_eq!(t.g.seen(t.id("bears"), None).pt, Some((2, 2))));
}

#[test]
fn mizzium_skin_overloaded_gives_each_of_your_creatures_hexproof() {
    // "Each creature you control gets +0/+1 and gains hexproof until end of turn."
    let mut t = Board::new(2).fill(5).at(Part::PrecombatMain).hand(0, &["Mizzium Skin"]).hand(1, &["Lightning Bolt"])
        .named(0, Zone::Battlefield, "my bears", "Grizzly Bears").named(1, Zone::Battlefield, "bears", "Grizzly Bears").build();
    overloaded(&mut t, "Mizzium Skin", "CU");
    let (mine, theirs) = (t.id("my bears"), t.id("bears"));
    cite!(["702.96a", "613.4c"]; assert_eq!((t.g.toughness(mine), t.g.toughness(theirs)), (Some(3), Some(2))));
    t.g.act(0, Move::Pass).unwrap();
    t.mana(1, "{R}");
    let bolt = t.find_in(1, Zone::Hand, "Lightning Bolt");
    cite!(["702.11b", "702.96a"]; refused("it has hexproof, and an opponent of its controller is targeting it (702.11b)"):
        t.g.act(1, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Object(mine)]] })));
    cite!(["702.96a"]; ok: t.g.act(1, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Object(theirs)]] })));
}
