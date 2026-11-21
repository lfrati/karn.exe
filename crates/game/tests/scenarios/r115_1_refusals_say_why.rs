//! A refused target says what of the target's description it is not, and the rule (115.1):
//! never only "an illegal target".
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Target, Zone};
use ir::Part;

fn game(names: &[&str], custom: Vec<CardDef>) -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let mut defs: Vec<CardDef> = names.iter().map(|n| CardDef::from_face(faces.iter().find(|f| f.name == *n).unwrap()).unwrap()).collect();
    defs.extend(custom);
    defs.push(CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap());
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let island = defs.len() - 1;
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..8 { g.put(p, island, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn c(name: &str, cost: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef { CardDef::new(name, cost, types, text, pt).expect(name) }
fn refusal(g: &mut Game, card: game::ObjId, target: Target, mana: &str) -> String {
    g.act(0, paid(mana, Move::Cast { card, targets: vec![vec![target]] })).expect_err("refused")
}

#[test]
fn ephemerate_at_an_opponents_creature_says_it_must_be_yours() {
    let mut g = game(&["Ephemerate"], vec![c("Bear", "{0}", "Creature — Bear", "", Some((2, 2)))]);
    let bear = g.put(1, 1, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[0] = 1;
    cite!(["115.1"]; assert_eq!(refusal(&mut g, card, Target::Object(bear), "W"),
        "an illegal target, Bear: the target is one you control, and another player controls it (115.1)"));
}

#[test]
fn a_graveyard_target_says_the_card_is_in_an_opponents_graveyard() {
    let mut g = game(&[], vec![
        c("Recall", "{0}", "Sorcery", "Return target instant card from your graveyard to your hand.", None),
        c("Shock", "{R}", "Instant", "Shock deals 2 damage to any target.", None),
    ]);
    let theirs = g.put(1, 1, Zone::Graveyard);
    let card = g.put(0, 0, Zone::Hand);
    assert_eq!(refusal(&mut g, card, Target::Object(theirs), ""),
        "an illegal target, Shock: it is in an opponent's graveyard, not yours (115.1)");
    let in_hand = g.put(0, 1, Zone::Hand);
    cite!(["115.1"]; assert_eq!(refusal(&mut g, card, Target::Object(in_hand), ""),
        "an illegal target, Shock: it isn't in your graveyard (115.1)"));
}

#[test]
fn spell_snare_at_a_bigger_spell_names_its_mana_value() {
    let mut g = game(&["Spell Snare"], vec![c("Giant", "{4}{U}", "Creature — Giant", "", Some((5, 5)))]);
    let giant = g.put(0, 1, Zone::Hand);
    g.players[0].pool.colors[1] = 5;
    g.act(0, paid("UUUUU", Move::Cast { card: giant, targets: vec![] })).unwrap();
    let giant = g.stack[0];
    let snare = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[1] = 1;
    cite!(["115.1"]; assert_eq!(refusal(&mut g, snare, Target::Object(giant), "U"),
        "an illegal target, Giant: its mana value is 5, and the target says mana value 2 (115.1)"));
}

#[test]
fn hexproof_and_a_filter_keyword_are_named() {
    let mut g = game(&[], vec![
        c("Bolt", "{0}", "Instant", "Bolt deals 3 damage to target creature with flying.", None),
        c("Bear", "{0}", "Creature — Bear", "", Some((2, 2))),
        c("Wisp", "{0}", "Creature — Spirit", "Flying, hexproof", Some((1, 1))),
    ]);
    let bear = g.put(1, 1, Zone::Battlefield);
    let wisp = g.put(1, 2, Zone::Battlefield);
    let bolt = g.put(0, 0, Zone::Hand);
    cite!(["115.1"]; assert_eq!(refusal(&mut g, bolt, Target::Object(bear), ""), "an illegal target, Bear: it doesn't have flying (115.1)"));
    cite!(["702.11b"]; assert_eq!(refusal(&mut g, bolt, Target::Object(wisp), ""),
        "an illegal target, Wisp: it has hexproof, and an opponent of its controller is targeting it (702.11b)"));
}

/// An Aura cast with nothing to enchant must say why, not "wrong number of targets".
#[test]
fn an_aura_with_nothing_to_enchant_says_so() {
    let mut g = game(&[], vec![
        c("Might", "{0}", "Enchantment — Aura", "Enchant creature\nEnchanted creature gets +2/+2.", None),
        c("Bolt", "{0}", "Instant", "Bolt deals 3 damage to target creature.", None),
        c("Bear", "{0}", "Creature — Bear", "", Some((2, 2))),
    ]);
    let aura = g.put(0, 0, Zone::Hand);
    cite!(["303.4a", "601.2c"]; assert_eq!(g.act(0, Move::Cast { card: aura, targets: vec![vec![]] }), Err("there is nothing it could enchant to target (303.4a, 601.2c)".into())));
    let bolt = g.put(0, 1, Zone::Hand);
    assert_eq!(g.act(0, Move::Cast { card: bolt, targets: vec![vec![]] }), Err("there is no legal target for it (601.2c)".into()));
    g.put(1, 2, Zone::Battlefield);
    cite!(["601.2c"]; assert_eq!(g.act(0, Move::Cast { card: bolt, targets: vec![vec![]] }), Err("wrong number of targets: 0 chosen, it takes 1 (601.2c)".into())));
}
