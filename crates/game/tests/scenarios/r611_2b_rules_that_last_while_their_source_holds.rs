//! A rule a resolving ability puts in force that lasts "for as long as you control this
//! creature" ends once that is no longer true, and is not back if it is true again (611.2b);
//! one that lasts "this combat" ends with the combat phase (506.1, 500.5a); one that lasts "for
//! the rest of the game" never ends (611.2a).
use crate::support;
use support::*;

/// Pass until this player is asked to declare attackers.
fn to_attackers(t: &mut T, player: usize) {
    for _ in 0..40 {
        if matches!(t.g.decision, Some(Decision::Attackers { player: p }) if p == player) { return; }
        match t.g.decision.clone() {
            Some(Decision::Attackers { player }) => t.g.act(player, Move::Attack(vec![])).unwrap(),
            Some(Decision::Blockers { player }) => t.g.act(player, Move::Block(vec![])).unwrap(),
            _ => { let p = t.g.decision_maker(t.g.priority); t.g.act(p, Move::Pass).unwrap() }
        }
    }
    panic!("never reached player {player}'s declare attackers step");
}

// Rules: 611.2b, 502.3
// Given: player 0 holds Dungeon Geists ("When this creature enters, tap target creature an
// opponent controls. That creature doesn't untap during its controller's untap step for as long
// as you control this creature."); player 1 controls Grizzly Bears and holds Unsummon.
// When: the Geists enters tapping the Bears; player 1's untap step passes; then player 1
// returns the Geists to its owner's hand, and their next untap step comes.
// Expect: the Bears stay tapped through the first untap step, and untap in the one after the
// Geists left.
#[test]
fn r611_2b_dungeon_geists_keeps_it_tapped_while_you_control_the_geists() {
    let mut t = Board::new(2).hand(0, &["Dungeon Geists"]).battlefield(1, &["Grizzly Bears"]).hand(1, &["Unsummon"]).fill(10).build();
    let bears = t.find_in_battlefield("Grizzly Bears");
    t.to_no_combat(0, Part::PrecombatMain);
    t.mana(0, "{2}{U}{U}").cast(0, "Dungeon Geists", vec![], mana_pool("CCUU")).unwrap();
    t.settle();
    let Some(Decision::Targets { player, .. }) = t.g.decision.clone() else { panic!("{:?}", t.g.decision) };
    t.g.act(player, Move::ChooseTargets(vec![vec![Target::Object(bears)]])).unwrap();
    t.resolve();
    cite!(["611.2b"]; assert!(t.g.obj(bears).tapped));
    t.to_no_combat(1, Part::Upkeep);
    cite!(["611.2b", "502.3"]; assert!(t.g.obj(bears).tapped, "it doesn't untap while player 0 controls the Geists"));
    let geists = t.find_in_battlefield("Dungeon Geists");
    t.mana(1, "{U}").cast(1, "Unsummon", vec![Tgt::Object(geists)], mana_pool("U")).unwrap();
    t.resolve();
    t.to_no_combat(0, Part::Upkeep);
    t.to_no_combat(1, Part::Upkeep);
    cite!(["611.2b", "502.3"]; assert!(!t.g.obj(bears).tapped, "the rule ended as the Geists left"));
}

// Rules: 506.1, 509.1b
// Given: player 0 controls Yuan Shao's Infantry ("Whenever this creature attacks alone, this
// creature can't be blocked this combat."); player 1 controls Grizzly Bears.
// When: the Infantry attacks alone and its ability resolves.
// Expect: the Bears can't block it in that combat; after the combat phase, the rule is over.
#[test]
fn r506_1_yuan_shaos_infantry_cant_be_blocked_this_combat() {
    let mut t = Board::new(2).battlefield(0, &["Yuan Shao's Infantry"]).battlefield(1, &["Grizzly Bears"]).fill(10).build();
    let (infantry, bears) = (t.find_in_battlefield("Yuan Shao's Infantry"), t.find_in_battlefield("Grizzly Bears"));
    to_attackers(&mut t, 0);
    t.g.act(0, Move::Attack(vec![(infantry, Target::Player(1))])).unwrap();
    t.settle();
    cite!(["506.1", "509.1b"]; assert!(t.g.cant_block(bears, infantry, &[bears])));
    t.to_no_combat(0, Part::PostcombatMain);
    cite!(["506.1"]; assert!(!t.g.cant_block(bears, infantry, &[bears]), "it lasted that combat only"));
}

// Rules: 611.2a, 119.7
// Given: player 0 controls Stigma Lasher ("Whenever this creature deals damage to a player,
// that player can't gain life for the rest of the game."); player 1 holds Reviving Dose ("You
// gain 3 life. Draw a card.").
// When: the Lasher deals combat damage to player 1; on their own turn player 1 casts Reviving
// Dose.
// Expect: player 1 gains no life, a turn after the rule was put in force.
#[test]
fn r611_2a_stigma_lasher_stops_life_gain_for_the_rest_of_the_game() {
    let mut t = Board::new(2).battlefield(0, &["Stigma Lasher"]).hand(1, &["Reviving Dose"]).fill(10).build();
    let lasher = t.find_in_battlefield("Stigma Lasher");
    to_attackers(&mut t, 0);
    t.g.act(0, Move::Attack(vec![(lasher, Target::Player(1))])).unwrap();
    t.to_no_combat(0, Part::PostcombatMain);
    cite!(["510.2"]; assert_eq!(t.g.players[1].life, 18));
    t.to_no_combat(1, Part::PrecombatMain);
    t.mana(1, "{2}{W}").cast(1, "Reviving Dose", vec![], mana_pool("CCW")).unwrap();
    t.resolve();
    cite!(["611.2a", "119.7"]; assert_eq!(t.g.players[1].life, 18));
}
