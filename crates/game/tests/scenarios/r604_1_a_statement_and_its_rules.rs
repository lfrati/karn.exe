//! "As long as …, this creature gets +2/+2, has flying, and attacks each combat if able": one
//! statement says both what its subject is (613) and a rule for it (508.1d). Each owner reads
//! its own acts, under the condition they share (604.1, 207.2c).
use crate::support;
use support::*;

/// Dragon's Rage Channeler on player 0's battlefield, an opposing Grizzly Bears, and player 0's
/// graveyard: brought to player 0's declare attackers step.
fn channeler(graveyard: &[&str]) -> (T, ObjId, ObjId) {
    let mut t = Board::new(2).battlefield(0, &["Dragon's Rage Channeler"]).battlefield(1, &["Grizzly Bears"])
        .graveyard(0, graveyard).build();
    let drc = t.find_in_battlefield("Dragon's Rage Channeler");
    let bears = t.find_in_battlefield("Grizzly Bears");
    for _ in 0..20 {
        if matches!(t.g.decision, Some(Decision::Attackers { .. })) { break; }
        let p = t.g.decision_maker(t.g.priority);
        t.g.act(p, Move::Pass).unwrap();
    }
    assert!(matches!(t.g.decision, Some(Decision::Attackers { player: 0 })), "reached the declare attackers step");
    (t, drc, bears)
}

// Rules: 207.2c, 604.1, 613.4c, 508.1d, 702.9b
// Given: Dragon's Rage Channeler; its controller's graveyard holds a land, a creature, an
// instant and a sorcery card (four card types).
// When: its controller declares attackers, and then its opponent declares blockers.
// Expect: it is 3/3; declaring no attackers is refused, as it attacks if able; it attacks, the
// opponent's Grizzly Bears can't block it, as it has flying, and it deals that player 3.
#[test]
fn r604_1_dragons_rage_channeler_with_delirium_gets_bigger_flies_and_must_attack() {
    let (mut t, drc, bears) = channeler(&["Forest", "Grizzly Bears", "Lightning Bolt", "Divination"]);
    t.expect().rules(["613.4c", "207.2c"]).pt("Dragon's Rage Channeler", 3, 3).check();
    cite!(["508.1d", "604.1"]; refused("Dragon's Rage Channeler attacks if able (508.1d)"): t.g.act(0, Move::Attack(vec![])));
    t.g.act(0, Move::Attack(vec![(drc, Target::Player(1))])).unwrap();
    for _ in 0..20 {
        if t.g.part == Part::PostcombatMain { break; }
        if matches!(t.g.decision, Some(Decision::Blockers { .. })) {
            cite!(["702.9b", "509.1b"]; refused("Grizzly Bears can't block Dragon's Rage Channeler: Dragon's Rage Channeler has flying (702.9b, 509.1b)"): t.g.act(1, Move::Block(vec![(bears, drc)])));
            t.g.act(1, Move::Block(vec![])).unwrap();
            continue;
        }
        let p = t.g.decision_maker(t.g.priority);
        t.g.act(p, Move::Pass).unwrap();
    }
    cite!(["702.9b", "510.1a"]; assert_eq!(t.g.players[1].life, 17));
}

// Rules: 207.2c, 604.1, 508.1d
// Given: Dragon's Rage Channeler; its controller's graveyard holds a land, a creature and an
// instant card (three card types).
// When: its controller declares no attackers.
// Expect: it is 1/1, and declaring no attackers is legal.
#[test]
fn r604_1_dragons_rage_channeler_without_delirium_is_neither_bigger_nor_made_to_attack() {
    let (mut t, _, _) = channeler(&["Forest", "Grizzly Bears", "Lightning Bolt"]);
    t.expect().rules(["207.2c"]).pt("Dragon's Rage Channeler", 1, 1).check();
    cite!(["508.1d", "604.1"]; ok: t.g.act(0, Move::Attack(vec![])));
}

/// Pass until the stack is empty.
fn resolve(t: &mut T) {
    for _ in 0..8 {
        if t.g.stack.is_empty() { return; }
        t.pass_all();
    }
}

/// Player 0 casts Enchanted River's Grasp on `creature`, and it and its enters trigger resolve.
fn grasp(t: &mut T, creature: ObjId) {
    t.mana(0, "{2}{U}").cast(0, "Enchanted River's Grasp", vec![Tgt::Object(creature)], mana_pool("CCU")).unwrap();
    resolve(t);
}

// Rules: 604.1, 613.1f, 302.6, 122.1, 610.3
// Given: player 1's Benalish Marshal ("Other creatures you control get +1/+1.") with a +1/+1
// counter, beside their Grizzly Bears; player 0 holds Enchanted River's Grasp, player 1
// Disenchant.
// When: Grasp enchants the Marshal; player 1's turn comes; player 1 destroys Grasp; player 1's
// next turn comes.
// Expect: the Marshal is tapped, without its counter (3/3), and has no abilities: the Bears are
// 2/2. It stays tapped through player 1's untap step. Once Grasp is gone the Bears are 3/3
// again, and at player 1's next untap step the Marshal untaps.
#[test]
fn r604_1_enchanted_rivers_grasp_taps_strips_and_holds_the_creature_while_it_is_attached() {
    let mut t = Board::new(2).hand(0, &["Enchanted River's Grasp"]).battlefield(1, &["Benalish Marshal", "Grizzly Bears"])
        .hand(1, &["Disenchant"]).counters("Benalish Marshal", "+1/+1", 1).fill(5).build();
    let marshal = t.find_in_battlefield("Benalish Marshal");
    t.expect().rules(["613.4c"]).pt("Benalish Marshal", 4, 4).pt("Grizzly Bears", 3, 3).check();
    grasp(&mut t, marshal);
    let aura = t.find_in_battlefield("Enchanted River's Grasp");
    cite!(["701.26a", "122.1"]; assert_eq!((t.g.obj(marshal).tapped, t.g.obj(marshal).counters.values().sum::<u32>()), (true, 0)));
    t.expect().rules(["613.1f", "604.1"]).pt("Benalish Marshal", 3, 3).pt("Grizzly Bears", 2, 2).check();

    t.to_no_combat(1, Part::PrecombatMain);
    cite!(["302.6", "604.1"]; assert!(t.g.obj(marshal).tapped));
    let disenchant = t.find_in(1, Zone::Hand, "Disenchant");
    t.mana(1, "{1}{W}");
    t.g.act(1, Move::Paying(mana_pool("CW"), Box::new(Move::Cast { card: disenchant, targets: vec![vec![Target::Object(aura)]] }))).unwrap();
    resolve(&mut t);
    assert!(!t.g.battlefield.contains(&aura));
    t.expect().rules(["611.3a"]).pt("Benalish Marshal", 3, 3).pt("Grizzly Bears", 3, 3).check();

    t.to_no_combat(0, Part::PrecombatMain);
    t.to_no_combat(1, Part::PrecombatMain);
    cite!(["502.3", "611.3a"]; assert!(!t.g.obj(marshal).tapped));
}

// Rules: 613.7, 613.1f
// Given: player 1's Serra Angel (flying, vigilance); player 0 holds Enchanted River's Grasp and
// Jump ("Target creature gains flying until end of turn.").
// When: Grasp enchants the Angel; then player 0 casts Jump on it.
// Expect: the Angel loses flying; Jump's effect, with the later timestamp, gives it flying again.
#[test]
fn r613_7_an_ability_granted_after_enchanted_rivers_grasp_still_applies() {
    let mut t = Board::new(2).hand(0, &["Enchanted River's Grasp", "Jump"]).battlefield(1, &["Serra Angel"]).build();
    let angel = t.find_in_battlefield("Serra Angel");
    assert!(t.g.has(angel, "Flying"));
    grasp(&mut t, angel);
    cite!(["613.1f"]; assert!(!t.g.has(angel, "Flying") && !t.g.has(angel, "Vigilance")));
    t.mana(0, "{U}").cast(0, "Jump", vec![Tgt::Object(angel)], mana_pool("U")).unwrap();
    resolve(&mut t);
    cite!(["613.7"]; assert!(t.g.has(angel, "Flying")));
}

// Rules: 604.1, 613.4c, 506.4
// Given: three players; player 0 enchants player 1's Grizzly Bears with Vow of Flight
// ("Enchanted creature gets +2/+2, has flying, and can't attack you or planeswalkers you
// control.").
// When: player 1 declares the Bears as an attacker on their turn.
// Expect: the Bears are 4/4 with flying; attacking player 0, the Aura's controller, is refused,
// and attacking player 2 is legal.
#[test]
fn r604_1_vow_of_flight_grows_the_creature_and_keeps_it_from_attacking_its_controller() {
    let mut t = Board::new(3).hand(0, &["Vow of Flight"]).battlefield(1, &["Grizzly Bears"]).fill(5).build();
    let bears = t.find_in_battlefield("Grizzly Bears");
    t.mana(0, "{2}{U}").cast(0, "Vow of Flight", vec![Tgt::Object(bears)], mana_pool("CCU")).unwrap();
    resolve(&mut t);
    t.expect().rules(["613.4c"]).pt("Grizzly Bears", 4, 4).check();
    cite!(["613.1f"]; assert!(t.g.has(bears, "Flying")));
    t.to_no_combat(1, Part::PrecombatMain);
    to_attackers(&mut t);
    assert!(matches!(t.g.decision, Some(Decision::Attackers { player: 1 })), "reached player 1's declare attackers step");
    cite!(["506.4", "604.1"]; refused("can't attack"): t.g.act(1, Move::Attack(vec![(bears, Target::Player(0))])));
    cite!(["506.4"]; ok: t.g.act(1, Move::Attack(vec![(bears, Target::Player(2))])));
}

/// Pass until attackers are to be declared.
fn to_attackers(t: &mut T) {
    for _ in 0..20 {
        if matches!(t.g.decision, Some(Decision::Attackers { .. })) { return; }
        let p = t.g.decision_maker(t.g.priority);
        t.g.act(p, Move::Pass).unwrap();
    }
}

// Rules: 604.1, 613.1f, 508.1d
// Given: Avatar of Slaughter ("All creatures have double strike and attack each combat if
// able.") and Grizzly Bears on player 0's battlefield.
// When: player 0 declares attackers with only the Avatar.
// Expect: both creatures have double strike; declaring only the Avatar is refused, as the Bears
// attack if able; declaring both is legal.
#[test]
fn r604_1_avatar_of_slaughter_gives_every_creature_double_strike_and_makes_it_attack() {
    let mut t = Board::new(2).battlefield(0, &["Avatar of Slaughter", "Grizzly Bears"]).build();
    to_attackers(&mut t);
    let avatar = t.find_in_battlefield("Avatar of Slaughter");
    let bears = t.find_in_battlefield("Grizzly Bears");
    cite!(["613.1f"]; assert!(t.g.has(avatar, "Double Strike") && t.g.has(bears, "Double Strike")));
    cite!(["508.1d", "604.1"]; refused("Grizzly Bears attacks if able (508.1d)"): t.g.act(0, Move::Attack(vec![(avatar, Target::Player(1))])));
    cite!(["508.1d"]; ok: t.g.act(0, Move::Attack(vec![(avatar, Target::Player(1)), (bears, Target::Player(1))])));
}
