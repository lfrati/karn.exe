//! Review of afterlife (702.135a): "When this permanent is put into a graveyard from the
//! battlefield, create N 1/1 white and black Spirit creature tokens with flying." A copy has it
//! (707.2), a token is put into the graveyard before it ceases to exist (111.7), one exiled
//! instead is not put there, and the Spirits are its controller's as it last was (603.3a). Every
//! card is read from its Oracle text.
use crate::support::*;
use game::announcement::CastCost;
use game::legal::Asked;
use game::state::Decision;

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Ministrant of Obligation", "Indebted Spirit", "Knight of the Last Breath", "Afterlife Insurance", "Clone", "Cackling Counterpart",
        "Path to Exile", "Act of Treason", "Murder", "Disenchant", "Lightning Bolt", "Grizzly Bears", "Ornithopter", "Plains"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

/// The Spirit tokens this player controls.
fn spirits(t: &T, p: usize) -> usize {
    t.g.battlefield.iter().filter(|o| t.g.obj(**o).chars.name.starts_with("Spirit") && t.g.controller(**o) == p).count()
}

/// Until priority with an empty stack: triggers in the order offered (603.3b), yes to "you
/// may", `copied` chosen as what a Clone copies, and a library searched for its first card.
fn settle(t: &mut T, copied: Option<ObjId>) {
    for _ in 0..40 {
        if let Some(Decision::Order { player, abilities }) = t.g.decision.clone() {
            t.g.act(player, Move::Order((0..abilities.len()).collect())).unwrap();
            continue;
        }
        match t.settle() {
            Boundary::Question(p, Asked::May) => { t.answer(p, Answer::Yes).unwrap(); }
            Boundary::Question(p, Asked::Select { among, .. }) if copied.is_some_and(|c| among.contains(&c)) => { t.answer(p, Answer::Objects(vec![copied.unwrap()])).unwrap(); }
            Boundary::Question(p, Asked::Search { among, .. }) => { t.answer(p, Answer::Objects(among.into_iter().take(1).collect())).unwrap(); }
            Boundary::Priority(_) => return,
            other => panic!("unexpected {other:?}"),
        }
    }
    panic!("the stack did not empty");
}

/// Player 0 casts this card from their hand, paying `mana`, and it resolves.
fn cast(t: &mut T, name: &str, targets: Vec<Tgt>, mana: &str) {
    let units: String = mana.trim_matches(|c| c == '{' || c == '}').split("}{").map(|s| match s.parse::<usize>() { Ok(n) => "C".repeat(n), Err(_) => s.to_string() }).collect();
    t.mana(0, mana).cast(0, name, targets, mana_pool(&units)).unwrap();
    settle(t, None);
}

// Rules: 707.2, 702.135a, 603.3a
// Given: player 1's Ministrant of Obligation ("Afterlife 2").
// When: player 0 casts Clone as a copy of it, then Murders the Clone.
// Expect: the Clone has afterlife as a copy (707.2): player 0, who controlled it, gets two
// Spirits; player 1 none.
#[test]
fn r707_2_a_copy_of_a_creature_with_afterlife_makes_spirits_when_it_dies() {
    let mut t = Board::new(2).fill(8).battlefield(1, &["Ministrant of Obligation"]).hand(0, &["Clone", "Murder"]).build();
    let ministrant = t.find_in_battlefield("Ministrant of Obligation");
    t.mana(0, "{3}{U}").cast(0, "Clone", vec![], mana_pool("CCCU")).unwrap();
    settle(&mut t, Some(ministrant));
    let clone = t.find_in(0, Zone::Battlefield, "Ministrant of Obligation");
    cast(&mut t, "Murder", vec![Tgt::Object(clone)], "{1}{B}{B}");
    cite!(["707.2", "702.135a", "603.3a"]; assert_eq!((spirits(&t, 0), spirits(&t, 1)), (2, 0)));
}

// Rules: 111.7, 702.135a
// Given: player 0's Ministrant of Obligation.
// When: player 0 casts Cackling Counterpart ("Create a token that's a copy of target creature you
// control."), then Murders the token.
// Expect: the token is put into the graveyard before it ceases to exist (111.7), so its
// afterlife makes two Spirits; the Ministrant stays.
#[test]
fn r111_7_a_token_with_afterlife_makes_spirits_when_it_dies() {
    let mut t = Board::new(2).fill(8).battlefield(0, &["Ministrant of Obligation"]).hand(0, &["Cackling Counterpart", "Murder"]).build();
    let ministrant = t.find_in_battlefield("Ministrant of Obligation");
    cast(&mut t, "Cackling Counterpart", vec![Tgt::Object(ministrant)], "{1}{U}{U}");
    let token = t.g.battlefield.iter().copied().find(|o| *o != ministrant && t.g.obj(*o).chars.name == "Ministrant of Obligation").expect("the token copy");
    cast(&mut t, "Murder", vec![Tgt::Object(token)], "{1}{B}{B}");
    cite!(["111.7", "702.135a"]; assert_eq!(spirits(&t, 0), 2));
    cite!(["111.7"]; assert!(t.g.battlefield.contains(&ministrant)));
}

// Rules: 702.135a
// Given: player 0's Ministrant of Obligation.
// When: player 0 casts Path to Exile at it.
// Expect: it is exiled, not put into a graveyard: no Spirit.
#[test]
fn r702_135a_an_exiled_creature_makes_no_spirits() {
    let mut t = Board::new(2).fill(8).battlefield(0, &["Ministrant of Obligation"]).hand(0, &["Path to Exile"]).library(0, &["Plains"]).build();
    let ministrant = t.find_in_battlefield("Ministrant of Obligation");
    cast(&mut t, "Path to Exile", vec![Tgt::Object(ministrant)], "{W}");
    t.expect().rules(["702.135a"]).zone(0, Zone::Exile, &["Ministrant of Obligation"]).check();
    cite!(["702.135a"]; assert_eq!(spirits(&t, 0), 0));
}

// Rules: 603.3a, 702.135a
// Given: player 1's Ministrant of Obligation.
// When: player 0 casts Act of Treason at it, then Murders it.
// Expect: player 0 controlled it as it died: the Spirits are theirs, none player 1's.
#[test]
fn r603_3a_a_creature_that_dies_under_another_players_control_makes_their_spirits() {
    let mut t = Board::new(2).fill(8).battlefield(1, &["Ministrant of Obligation"]).hand(0, &["Act of Treason", "Murder"]).build();
    let ministrant = t.find_in_battlefield("Ministrant of Obligation");
    cast(&mut t, "Act of Treason", vec![Tgt::Object(ministrant)], "{2}{R}");
    cite!(["603.3a"]; assert_eq!(t.g.controller(ministrant), 0));
    cast(&mut t, "Murder", vec![Tgt::Object(ministrant)], "{1}{B}{B}");
    cite!(["603.3a", "702.135a"]; assert_eq!((spirits(&t, 0), spirits(&t, 1)), (2, 0)));
}

/// Player 0 casts Indebted Spirit from their hand bestowed on `on` (702.103a), and it resolves.
fn bestow(t: &mut T, on: ObjId) -> ObjId {
    let card = t.find_in(0, Zone::Hand, "Indebted Spirit");
    let row = t.g.def(card).unwrap().abilities().iter().position(|a| matches!(a, ir::Ability::Keywords(items)
        if items.iter().any(|i| i.keyword.name() == "Bestow"))).expect("a bestow ability");
    let cast = Move::CastFor(CastCost::Alternative(row), Box::new(Move::Cast { card, targets: vec![vec![Target::Object(on)]] }));
    t.g.players[0].pool = mana_pool("CCW");
    t.g.act(0, Move::Paying(mana_pool("CCW"), Box::new(cast))).unwrap();
    settle(t, None);
    let aura = t.find_in_battlefield("Indebted Spirit");
    assert_eq!(t.g.attached_to(aura), Some(Target::Object(on)));
    aura
}

// Rules: 702.135a, 702.103f, 603.10a
// Given: player 0's Grizzly Bears with Indebted Spirit bestowed on it ("Afterlife 1 (When this
// permanent is put into a graveyard from the battlefield, ...)", "Enchanted creature gets
// +1/+1 and has afterlife 1.").
// When: player 0 Murders the Bears; or, again, casts Disenchant at the Aura.
// Expect: the Bears had afterlife 1 as it died (603.10a): one Spirit, and Indebted Spirit stays
// as a creature (702.103f). The Aura put into a graveyard itself makes one Spirit too, the
// Bears staying.
#[test]
fn r702_135a_indebted_spirit_gives_afterlife_and_has_it_as_an_aura() {
    let mut t = Board::new(2).fill(8).battlefield(0, &["Grizzly Bears"]).hand(0, &["Indebted Spirit", "Murder"]).build();
    let bears = t.find_in_battlefield("Grizzly Bears");
    bestow(&mut t, bears);
    cite!(["702.103b"]; assert_eq!((t.g.power(bears), t.g.toughness(bears)), (Some(3), Some(3))));
    cast(&mut t, "Murder", vec![Tgt::Object(bears)], "{1}{B}{B}");
    cite!(["603.10a", "702.135a"]; assert_eq!(spirits(&t, 0), 1));
    t.expect().rules(["702.103f"]).zone(0, Zone::Battlefield, &["Indebted Spirit", "Spirit Token"]).check();

    let mut t = Board::new(2).fill(8).battlefield(0, &["Grizzly Bears"]).hand(0, &["Indebted Spirit", "Disenchant"]).build();
    let bears = t.find_in_battlefield("Grizzly Bears");
    let aura = bestow(&mut t, bears);
    cast(&mut t, "Disenchant", vec![Tgt::Object(aura)], "{1}{W}");
    cite!(["702.135a"]; assert_eq!(spirits(&t, 0), 1));
    t.expect().rules(["702.135a"]).zone(0, Zone::Battlefield, &["Grizzly Bears", "Spirit Token"]).check();
}

// Rules: 702.135a, 118.3
// Given: player 0's Knight of the Last Breath ("{3}, Sacrifice another nontoken creature: Create a
// 1/1 white and black Spirit creature token with flying.") and Ministrant of Obligation.
// When: player 0 sacrifices the Ministrant to it, then offers a Spirit token.
// Expect: three Spirits, two of afterlife and one of the Knight; a Spirit token can't pay the
// cost.
#[test]
fn r702_135a_a_creature_sacrificed_to_knight_of_the_last_breath_makes_spirits_too() {
    let mut t = Board::new(2).fill(8).battlefield(0, &["Knight of the Last Breath", "Ministrant of Obligation"]).build();
    let (knight, ministrant) = (t.find_in_battlefield("Knight of the Last Breath"), t.find_in_battlefield("Ministrant of Obligation"));
    let ability = t.g.abilities(knight).iter().position(|a| matches!(&**a, ir::Ability::Activated { .. })).unwrap();
    let sacrifice = |t: &mut T, o: ObjId| t.g.act(0, paid("CCC", Move::Activate { source: knight, ability, targets: vec![], paying: vec![vec![o]] }));
    t.mana(0, "{3}");
    sacrifice(&mut t, ministrant).unwrap();
    settle(&mut t, None);
    cite!(["702.135a"]; assert_eq!(spirits(&t, 0), 3));
    let spirit = *t.g.battlefield.iter().find(|o| t.g.obj(**o).chars.name == "Spirit Token").unwrap();
    t.mana(0, "{3}");
    cite!(["118.3"]; refused("Spirit Token is not what that cost asks for"): sacrifice(&mut t, spirit));
}

// Rules: 611.2c, 702.135a
// Given: player 0's Grizzly Bears.
// When: player 0 casts Afterlife Insurance ("Creatures you control gain afterlife 1 until end of
// turn."), then Ornithopter, then Murders the Ornithopter and Bolts the Bears.
// Expect: what it affects is fixed as it resolves (611.2c): the Ornithopter dies with no
// Spirit, the Bears with one.
#[test]
fn r611_2c_afterlife_insurance_gives_afterlife_to_the_creatures_there_as_it_resolves() {
    let mut t = Board::new(2).fill(8).battlefield(0, &["Grizzly Bears"]).hand(0, &["Afterlife Insurance", "Ornithopter", "Murder", "Lightning Bolt"]).build();
    let bears = t.find_in_battlefield("Grizzly Bears");
    cast(&mut t, "Afterlife Insurance", vec![], "{1}{W}");
    t.cast(0, "Ornithopter", vec![], mana_pool("")).unwrap();
    settle(&mut t, None);
    let thopter = t.find_in_battlefield("Ornithopter");
    cast(&mut t, "Murder", vec![Tgt::Object(thopter)], "{1}{B}{B}");
    cite!(["611.2c"]; assert_eq!(spirits(&t, 0), 0));
    cast(&mut t, "Lightning Bolt", vec![Tgt::Object(bears)], "{R}");
    cite!(["611.2c", "702.135a"]; assert_eq!(spirits(&t, 0), 1));
}
