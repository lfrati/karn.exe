//! Triggers that count the creatures declared as attackers: "attacks alone" (506.5, 702.83b),
//! "whenever you attack with two or more creatures" (508.3c-d) and battalion ("this creature
//! and at least two other creatures attack"). Counted from the declaration, never from a
//! creature put onto the battlefield attacking (508.3a), and not rechecked on resolution (603.4).
use crate::support::{cite, mana_pool};
use game::state::Answer;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const BEAR: usize = 0;
const KAVU: usize = 1;
const INTELLIGENCE: usize = 2;
const ELITE: usize = 3;
const EXALTATION: usize = 5;
const GIDEON: usize = 6;
const ELF: usize = 7;
const ISLAND: usize = 8;
const PRISON: usize = 9;
const ALTAR: usize = 10;
const SIGIL: usize = 11;
const FULL_PARTY: usize = 12;

fn game(players: usize) -> Game {
    let c = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    let defs = vec![
        c("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        c("Rogue Kavu", "{1}{R}", "Creature — Kavu Rogue", "Whenever this creature attacks alone, it gets +2/+0 until end of turn.", Some((1, 1))),
        c("Military Intelligence", "{1}{U}", "Enchantment", "Whenever you attack with two or more creatures, draw a card.", None),
        c("Boros Elite", "{W}", "Creature — Human Soldier", "Battalion — Whenever this creature and at least two other creatures attack, this creature gets +2/+2 until end of turn.", Some((1, 1))),
        c("Garrison", "{2}{R}", "Creature — Human Soldier", "", Some((2, 3))),
        c("Exaltation", "{W}", "Enchantment", "Whenever a creature you control attacks alone, it gets +1/+1 until end of turn.", None),
        c("Gideon's Rally", "{W}", "Enchantment", "Whenever you attack with two or more non-Bear creatures, put a +1/+1 counter on each of those creatures.", None),
        c("Elf", "{G}", "Creature — Elf", "", Some((1, 1))),
        c("Island", "", "Basic Land — Island", "", None),
        c("Ghostly Prison", "{2}{W}", "Enchantment", "Creatures can't attack you unless their controller pays {2} for each creature they control that's attacking you.", None),
        c("Ashnod's Altar", "{3}", "Artifact", "Sacrifice a creature: Add {C}{C}.", None),
        c("Sigil of Valor", "{2}", "Artifact — Equipment", "Whenever equipped creature attacks alone, it gets +1/+1 until end of turn for each other creature you control.\nEquip {1}", None),
        c("Full Party", "{W}", "Enchantment", "Whenever you attack with three or more creatures, those creatures get +4/+4 until end of turn.", None),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, players, 20);
    for p in 0..players { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    g
}
fn to_attack(g: &mut Game) {
    for _ in 0..20 {
        if g.part == Part::DeclareAttackers && g.decision.is_some() { return; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("never reached declare attackers");
}
fn attack(g: &mut Game, attackers: &[ObjId]) {
    to_attack(g);
    g.act(0, Move::Attack(attackers.iter().map(|a| (*a, Target::Player(1))).collect())).unwrap();
}
/// Resolve whatever triggered and is on the stack.
fn resolve(g: &mut Game) {
    for _ in 0..20 {
        if g.stack.is_empty() { return; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("the stack did not empty");
}
fn power(g: &Game, o: ObjId) -> i32 { g.power(o).unwrap() }

#[test]
fn r506_5_attacks_alone_is_the_only_creature_declared() {
    let mut g = game(2);
    let kavu = g.put(0, KAVU, Zone::Battlefield);
    attack(&mut g, &[kavu]);
    resolve(&mut g);
    cite!(["506.5"]; assert_eq!(power(&g, kavu), 3));

    let mut g = game(2);
    let kavu = g.put(0, KAVU, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    attack(&mut g, &[kavu, bear]);
    resolve(&mut g);
    cite!(["506.5"]; assert_eq!(power(&g, kavu), 1, "not alone"));

    // "a creature you control attacks alone": that creature, whichever it is; one left home
    // does not keep the attacker from being alone
    let mut g = game(2);
    g.put(0, EXALTATION, Zone::Battlefield);
    let (bear, home) = (g.put(0, BEAR, Zone::Battlefield), g.put(0, BEAR, Zone::Battlefield));
    attack(&mut g, &[bear]);
    resolve(&mut g);
    cite!(["506.5"]; assert_eq!((power(&g, bear), power(&g, home)), (3, 2)));
    // an opponent's creature attacking alone is not one you control
    let mut g = game(2);
    g.put(1, EXALTATION, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    attack(&mut g, &[bear]);
    resolve(&mut g);
    cite!(["506.5"]; assert_eq!(power(&g, bear), 2));
}

#[test]
fn r508_3c_only_at_least_a_number_of_attackers_is_counted() {
    for text in [
        "Whenever you attack with your commander, draw a card.",
        "Whenever you attack with exactly two creatures, draw a card.",
    ] {
        let d = CardDef::new("Refused", "{1}", "Enchantment", text, None).unwrap();
        crate::support::refused(game::play::check(&d).map_err(|e|e.0),"trigger: attacking with other than at least a number of creatures");
    }
}

#[test]
fn r508_3d_you_attack_with_two_or_more_triggers_once_per_declaration() {
    // none of three declared, one, two, three
    for (n, drawn) in [(0, 0), (1, 0), (2, 1), (3, 1)] {
        let mut g = game(2);
        g.put(0, INTELLIGENCE, Zone::Battlefield);
        let bears: Vec<ObjId> = (0..3).map(|_| g.put(0, BEAR, Zone::Battlefield)).collect();
        let hand = g.players[0].hand.len();
        attack(&mut g, &bears[..n]);
        resolve(&mut g);
        cite!(["508.3d"]; assert_eq!(g.players[0].hand.len(), hand + drawn, "{n} attackers"));
    }
    // attacking different players counts together
    let mut g = game(3);
    g.put(0, INTELLIGENCE, Zone::Battlefield);
    let (a, b) = (g.put(0, BEAR, Zone::Battlefield), g.put(0, BEAR, Zone::Battlefield));
    let hand = g.players[0].hand.len();
    to_attack(&mut g);
    g.act(0, Move::Attack(vec![(a, Target::Player(1)), (b, Target::Player(2))])).unwrap();
    resolve(&mut g);
    cite!(["508.3d"]; assert_eq!(g.players[0].hand.len(), hand + 1));
}

#[test]
fn battalion_counts_the_others_with_the_subject_declared() {
    let mut g = game(2);
    let elite = g.put(0, ELITE, Zone::Battlefield);
    let (a, b) = (g.put(0, BEAR, Zone::Battlefield), g.put(0, BEAR, Zone::Battlefield));
    attack(&mut g, &[elite, a]);
    resolve(&mut g);
    // Uncited (card text): "Whenever this creature and at least two other creatures attack": one other is not two (battalion is an ability word, 207.2c)
    assert_eq!(power(&g, elite), 1, "one other");

    let mut g = game(2);
    let elite = g.put(0, ELITE, Zone::Battlefield);
    let others: Vec<ObjId> = (0..3).map(|_| g.put(0, BEAR, Zone::Battlefield)).collect();
    attack(&mut g, &others);
    resolve(&mut g);
    cite!(["508.3a"]; assert_eq!(power(&g, elite), 1, "it stayed home"));

    // the count is the trigger event, not a condition checked again (603.4)
    let mut g = game(2);
    let elite = g.put(0, ELITE, Zone::Battlefield);
    let (a, b2) = (g.put(0, BEAR, Zone::Battlefield), g.put(0, BEAR, Zone::Battlefield));
    let _ = b;
    attack(&mut g, &[elite, a, b2]);
    g.move_object(a, Zone::Graveyard);
    resolve(&mut g);
    cite!(["603.4"]; assert_eq!(power(&g, elite), 3));
}

#[test]
fn r608_2k_those_creatures_are_the_attackers_counted() {
    // "two or more non-Bear creatures": the Elves get counters, the Bear does not
    let mut g = game(2);
    g.put(0, GIDEON, Zone::Battlefield);
    let (e1, e2, bear) = (g.put(0, ELF, Zone::Battlefield), g.put(0, ELF, Zone::Battlefield), g.put(0, BEAR, Zone::Battlefield));
    attack(&mut g, &[e1, bear, e2]);
    resolve(&mut g);
    cite!(["608.2k"]; assert_eq!((power(&g, e1), power(&g, e2), power(&g, bear)), (2, 2, 2)));
    // one Elf and the Bear: not two non-Bears
    let mut g = game(2);
    g.put(0, GIDEON, Zone::Battlefield);
    let (e1, bear) = (g.put(0, ELF, Zone::Battlefield), g.put(0, BEAR, Zone::Battlefield));
    attack(&mut g, &[e1, bear]);
    resolve(&mut g);
    cite!(["608.2k"]; assert_eq!(power(&g, e1), 1));
}

#[test]
fn every_admitted_attack_count_form_has_a_fresh_count_each_combat() {
    // a second turn's declaration counts again
    let mut g = game(2);
    g.put(0, INTELLIGENCE, Zone::Battlefield);
    let (a, b) = (g.put(0, BEAR, Zone::Battlefield), g.put(0, BEAR, Zone::Battlefield));
    let hand = g.players[0].hand.len();
    attack(&mut g, &[a, b]);
    resolve(&mut g);
    cite!(["508.3d"]; assert_eq!(g.players[0].hand.len(), hand + 1));
    for _ in 0..200 {
        if g.turn == 3 && g.part == Part::DeclareAttackers && g.decision.is_some() { break; }
        match g.decision.clone() {
            Some(game::state::Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(game::state::Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            Some(game::state::Decision::Discard { player, count }) => { let h = g.players[player].hand[..count].to_vec(); g.act(player, Move::Discard(h)).unwrap() }
            None => g.act(g.priority, Move::Pass).unwrap(),
            other => panic!("{other:?}"),
        }
    }
    let hand = g.players[0].hand.len();
    g.act(0, Move::Attack(vec![(a, Target::Player(1)), (b, Target::Player(1))])).unwrap();
    resolve(&mut g);
    cite!(["508.3d", "603.2"]; assert_eq!(g.players[0].hand.len(), hand + 1));
}

#[test]
fn r508_1k_only_the_declaration_completed_after_its_costs_is_counted() {
    // a proposed attacker sacrificed to pay the attack cost is not declared (508.1j-k): Boros
    // Elite and one other creature attack, which is not battalion
    let mut g = game(2);
    g.put(1, PRISON, Zone::Battlefield);
    let altar = g.put(0, ALTAR, Zone::Battlefield);
    let elite = g.put(0, ELITE, Zone::Battlefield);
    let (a, b) = (g.put(0, BEAR, Zone::Battlefield), g.put(0, BEAR, Zone::Battlefield));
    to_attack(&mut g);
    g.players[0].pool.colorless = 4;
    attack(&mut g, &[elite, a, b]);
    let step = game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![], source: altar, ability: 0, paying: vec![vec![b]], adding: vec![] };
    g.act(0, Move::Choose(Answer::Pay { plan: vec![step], with: Some(mana_pool("CCCCCC")), tapped: vec![] })).unwrap();
    cite!(["508.1k"]; assert_eq!(g.combat.attackers.len(), 2));
    resolve(&mut g);
    cite!(["508.1k"]; assert_eq!(power(&g, elite), 1));

    // the cost declined, nothing was declared, and nothing triggers from it
    let mut g = game(2);
    g.put(1, PRISON, Zone::Battlefield);
    g.put(0, INTELLIGENCE, Zone::Battlefield);
    let (a, b) = (g.put(0, BEAR, Zone::Battlefield), g.put(0, BEAR, Zone::Battlefield));
    let hand = g.players[0].hand.len();
    attack(&mut g, &[a, b]);
    g.act(0, Move::Choose(Answer::No)).unwrap();
    g.act(0, Move::Attack(vec![])).unwrap();
    cite!(["508.1k"]; assert!(g.stack.is_empty()));
    cite!(["508.1k"]; assert_eq!(g.players[0].hand.len(), hand));
}

#[test]
fn r608_2k_other_than_the_creature_that_attacked_alone() {
    // "it gets +1/+1 for each other creature you control": other than the equipped attacker
    let mut g = game(2);
    let sigil = g.put(0, SIGIL, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let _home = (g.put(0, ELF, Zone::Battlefield), g.put(0, ELF, Zone::Battlefield));
    assert!(g.attach(sigil, Target::Object(bear)));
    attack(&mut g, &[bear]);
    resolve(&mut g);
    cite!(["608.2k"]; assert_eq!(power(&g, bear), 4));
}

#[test]
fn r608_2k_those_creatures_are_every_attacker_counted() {
    let mut g = game(2);
    g.put(0, FULL_PARTY, Zone::Battlefield);
    let attackers: Vec<ObjId> = (0..3).map(|_| g.put(0, ELF, Zone::Battlefield)).collect();
    let home = g.put(0, ELF, Zone::Battlefield);
    attack(&mut g, &attackers);
    resolve(&mut g);
    cite!(["608.2k"]; assert!(attackers.iter().all(|a| power(&g, *a) == 5)));
    cite!(["608.2k"]; assert_eq!(power(&g, home), 1));
}
