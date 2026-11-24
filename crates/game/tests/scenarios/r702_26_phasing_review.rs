//! Review of phasing (702.26) through the cards it touches: a phased-out permanent's abilities
//! don't apply and don't trigger, "each creature" and the legend rule don't see it, it phases
//! in with no enter or leave triggers (702.26d), what is attached to it phases with it
//! (702.26g-h), and an Aura phased out alone phases in attached when it can be (702.26i).
//! Every card is read from its Oracle text.
use crate::support;
use game::state::{Choice, Decision};
use ir::Ability;
use support::*;

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Glorious Anthem", "Soul Warden", "Elvish Visionary", "Fiend Hunter", "Kird Ape", "Wrath of God", "Isamaru, Hound of Konda",
        "Raise the Alarm", "Bonesplitter", "Pacifism", "Blink Dog", "Haystack", "Teferi's Curse", "Clever Concealment", "Shock", "Grizzly Bears"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

/// The index of its one activated ability.
fn activated(t: &T, o: ObjId) -> usize {
    t.g.abilities(o).iter().position(|a| matches!(&**a, Ability::Activated { .. })).expect("an activated ability")
}

fn phased_out(t: &T, o: ObjId) -> bool {
    t.g.obj(o).zone == Zone::PhasedOut && t.g.phased_out.contains(&o) && !t.g.battlefield.contains(&o)
}

/// A card of this name is in this player's graveyard.
fn in_graveyard(t: &T, player: usize, name: &str) -> bool {
    t.g.players[player].graveyard.iter().any(|o| t.g.obj(*o).chars.name == name)
}

/// Clever Concealment: "Convoke / Any number of target nonland permanents you control phase
/// out." Player 0 casts it on `objects`, one target group of any number.
fn conceal(t: &mut T, objects: Vec<ObjId>) {
    t.mana(0, "{2}{W}{W}");
    let card = t.find_in(0, Zone::Hand, "Clever Concealment");
    t.g.act(0, Move::Paying(mana_pool("{2}{W}{W}"), Box::new(Move::Cast { card, targets: vec![objects.into_iter().map(Target::Object).collect()] }))).unwrap();
    t.resolve();
}

/// Blink Dog's "{3}{W}: Blink Dog phases out."
fn teleport(t: &mut T, player: usize, dog: ObjId) {
    let ability = activated(t, dog);
    t.mana(player, "{3}{W}");
    t.g.act(player, paid("{3}{W}", Move::Activate { source: dog, ability, targets: vec![], paying: vec![] })).unwrap();
    t.resolve();
}

/// Haystack's "{2}: Target creature you control phases out."
fn haystack(t: &mut T, hay: ObjId, target: ObjId) {
    t.mana(0, "{2}");
    t.g.act(0, paid("{2}", Move::Activate { source: hay, ability: 0, targets: vec![vec![Target::Object(target)]], paying: vec![] })).unwrap();
    t.resolve();
}

// Rules: 702.26b, 702.26d, 613.1g, 603.2
// Given: Glorious Anthem and Soul Warden; Clever Concealment phases both out.
// When: player 0 casts Grizzly Bears; then their next untap step comes.
// Expect: the anthem stops applying and Soul Warden doesn't trigger for the Bears while they
// are phased out; once both phase in, the anthem applies again, and no life was gained.
#[test]
fn r702_26b_a_phased_out_anthem_does_not_apply_and_a_phased_out_soul_warden_does_not_trigger() {
    // Glorious Anthem: "Creatures you control get +1/+1."
    // Soul Warden: "Whenever another creature you control enters, you gain 1 life."
    let mut t = Board::new(2).fill(6).named(0, Zone::Battlefield, "anthem", "Glorious Anthem").named(0, Zone::Battlefield, "warden", "Soul Warden")
        .named(0, Zone::Battlefield, "bears", "Grizzly Bears").hand(0, &["Clever Concealment", "Grizzly Bears"]).build();
    let (anthem, warden, bears) = (t.id("anthem"), t.id("warden"), t.id("bears"));
    cite!(["613.4c"]; assert_eq!((t.g.power(bears), t.g.toughness(bears)), (Some(3), Some(3))));
    conceal(&mut t, vec![anthem, warden]);
    cite!(["702.26b"]; assert!(phased_out(&t, anthem) && phased_out(&t, warden)));
    cite!(["702.26b", "613.4c"]; assert_eq!((t.g.power(bears), t.g.toughness(bears)), (Some(2), Some(2))));
    t.mana(0, "{1}{G}").cast(0, "Grizzly Bears", vec![], mana_pool("{1}{G}")).unwrap();
    t.resolve();
    cite!(["702.26b", "603.2"]; assert!(t.g.players[0].life == 20 && t.g.stack.is_empty()));
    t.to_no_combat(1, Part::PrecombatMain);
    t.to_no_combat(0, Part::Upkeep);
    cite!(["702.26a", "502.1"]; assert!(t.g.battlefield.contains(&anthem) && t.g.battlefield.contains(&warden)));
    cite!(["613.4c"]; assert_eq!((t.g.power(bears), t.g.toughness(bears)), (Some(3), Some(3))));
    cite!(["702.26d"]; assert_eq!(t.g.players[0].life, 20));
}

// Rules: 702.26d, 610.3
// Given: Fiend Hunter has exiled Kird Ape; Clever Concealment phases out Fiend Hunter and
// Elvish Visionary.
// When: they phase out, and phase in at player 0's next untap step.
// Expect: the Ape stays exiled, as Fiend Hunter didn't leave the battlefield; Elvish
// Visionary's "enters" doesn't trigger as it phases in: no card is drawn.
#[test]
fn r702_26d_phasing_triggers_no_leaves_or_enters_ability() {
    // Fiend Hunter: "When this creature enters, you may exile another target creature. / When
    // this creature leaves the battlefield, return the exiled card to the battlefield under its
    // owner's control." Elvish Visionary: "When this creature enters, draw a card."
    let mut t = Board::new(2).fill(6).named(0, Zone::Battlefield, "visionary", "Elvish Visionary")
        .named(1, Zone::Battlefield, "ape", "Kird Ape").hand(0, &["Clever Concealment", "Fiend Hunter"]).build();
    let (visionary, ape) = (t.id("visionary"), t.id("ape"));
    t.mana(0, "{1}{W}{W}").cast(0, "Fiend Hunter", vec![], mana_pool("{1}{W}{W}")).unwrap();
    t.pass_all();
    let Some((0, Asked::Targets { .. })) = t.g.asked() else { panic!("Fiend Hunter's trigger asks for its target: {:?}", t.g.decision) };
    t.g.act(0, Move::ChooseTargets(vec![vec![Target::Object(ape)]])).unwrap();
    let Boundary::Question(0, Asked::May) = t.settle() else { panic!("Fiend Hunter's exile is optional: {:?}", t.g.decision) };
    t.answer(0, Answer::Yes).unwrap();
    t.resolve();
    let hunter = t.find_in_battlefield("Fiend Hunter");
    let exiled = |t: &T| t.g.exile.iter().any(|o| t.g.obj(*o).chars.name == "Kird Ape") && !t.g.battlefield.contains(&ape);
    cite!(["610.3"]; assert!(exiled(&t)));
    let hand = t.g.players[0].hand.len();
    conceal(&mut t, vec![visionary, hunter]);
    cite!(["702.26d", "610.3"]; assert!(phased_out(&t, hunter) && exiled(&t) && t.g.stack.is_empty()));
    t.to_no_combat(1, Part::PrecombatMain);
    t.to_no_combat(0, Part::Upkeep);
    cite!(["702.26d"]; assert!(t.g.battlefield.contains(&visionary) && t.g.battlefield.contains(&hunter)));
    cite!(["702.26d"]; assert!(t.g.stack.is_empty() && exiled(&t)));
    cite!(["702.26d"]; assert_eq!(t.g.players[0].hand.len(), hand - 1, "Clever Concealment was cast; nothing was drawn"));
}

// Rules: 702.26b, 704.5j
// Given: Haystack phases out Isamaru, Hound of Konda; Grizzly Bears is on the battlefield.
// When: Wrath of God resolves; then player 0 casts a second Isamaru; then the first phases in.
// Expect: the Wrath doesn't destroy the phased-out Isamaru; the second Isamaru meets no legend
// rule while the first is phased out; when the first phases in, the legend rule asks player 0
// which of the two to keep.
#[test]
fn r702_26b_a_phased_out_legend_escapes_a_wrath_and_meets_the_legend_rule_as_it_phases_in() {
    // Wrath of God: "Destroy all creatures. They can't be regenerated."
    // Haystack: "{2}: Target creature you control phases out."
    let mut t = Board::new(2).fill(6).named(0, Zone::Battlefield, "hay", "Haystack").named(0, Zone::Battlefield, "dog", "Isamaru, Hound of Konda")
        .named(0, Zone::Battlefield, "bears", "Grizzly Bears").hand(0, &["Wrath of God", "Isamaru, Hound of Konda"]).build();
    let (hay, dog, bears) = (t.id("hay"), t.id("dog"), t.id("bears"));
    haystack(&mut t, hay, dog);
    cite!(["702.26b"]; assert!(phased_out(&t, dog)));
    t.mana(0, "{2}{W}{W}").cast(0, "Wrath of God", vec![], mana_pool("{2}{W}{W}")).unwrap();
    t.resolve();
    cite!(["702.26b"]; assert!(phased_out(&t, dog) && !t.g.battlefield.contains(&bears) && in_graveyard(&t, 0, "Grizzly Bears")));
    t.mana(0, "{W}").cast(0, "Isamaru, Hound of Konda", vec![], mana_pool("{W}")).unwrap();
    t.resolve();
    let second = t.find_in_battlefield("Isamaru, Hound of Konda");
    cite!(["702.26b", "704.5j"]; assert!(t.g.decision.is_none() && t.g.battlefield.contains(&second)));
    t.to_no_combat(1, Part::PrecombatMain);
    // to player 0's upkeep, where the legend rule's choice waits before priority (704.3)
    while !(t.g.active == 0 && t.g.part == Part::Upkeep) {
        match t.g.decision.clone() {
            Some(Decision::Attackers { player }) => t.g.act(player, Move::Attack(vec![])).unwrap(),
            Some(Decision::Blockers { player }) => t.g.act(player, Move::Block(vec![])).unwrap(),
            _ => { let p = t.g.decision_maker(t.g.priority); t.g.act(p, Move::Pass).unwrap() }
        }
    }
    cite!(["702.26a"]; assert!(t.g.battlefield.contains(&dog)));
    let Some(Decision::Choose { player: 0, choice: Choice::Objects { among, count: 1, .. } }) = t.g.decision.clone() else {
        panic!("the legend rule asks which Isamaru to keep: {:?}", t.g.decision)
    };
    cite!(["704.5j"]; assert_eq!(among.len(), 2));
    cite!(["704.5j"]; assert!(among.contains(&dog) && among.contains(&second)));
}

// Rules: 702.26d, 111.8, 302.6
// Given: Raise the Alarm's Soldier token; Haystack phases it out.
// When: player 1's turn passes; then player 0's untap step comes.
// Expect: the phased-out token isn't a token that left the battlefield and ceases to exist;
// it phases in, and as it has been under player 0's control since their most recent turn
// began, it can attack.
#[test]
fn r702_26d_a_phased_out_token_does_not_cease_to_exist_and_returns_able_to_attack() {
    let mut t = Board::new(2).fill(6).named(0, Zone::Battlefield, "hay", "Haystack").hand(0, &["Raise the Alarm"]).build();
    let hay = t.id("hay");
    t.mana(0, "{1}{W}").cast(0, "Raise the Alarm", vec![], mana_pool("{1}{W}")).unwrap();
    t.resolve();
    let token = *t.g.battlefield.iter().find(|o| t.g.obj(**o).token()).expect("a Soldier");
    haystack(&mut t, hay, token);
    cite!(["702.26b"]; assert!(phased_out(&t, token)));
    t.to_no_combat(1, Part::PrecombatMain);
    cite!(["702.26d", "111.8"]; assert!(phased_out(&t, token)));
    t.to_no_combat(0, Part::PrecombatMain);
    cite!(["702.26a"]; assert!(t.g.battlefield.contains(&token)));
    cite!(["702.26d", "302.6"]; assert!(!t.g.summoning_sick(token)));
}

// Rules: 702.26g, 702.26h
// Given: Bonesplitter on Blink Dog, and another on Grizzly Bears.
// When: Blink Dog phases itself out; Clever Concealment phases out the Bears and the
// Bonesplitter attached to it; both phase in at player 0's next untap step.
// Expect: the Dog's Bonesplitter phases out indirectly with it; the Bears' Bonesplitter, also
// phasing out directly, phases out indirectly (702.26h); each phases in attached.
#[test]
fn r702_26h_equipment_phases_out_indirectly_with_its_creature_even_when_it_would_phase_out_itself() {
    // Bonesplitter: "Equipped creature gets +2/+0. / Equip {1}"
    let mut t = Board::new(2).fill(6).named(0, Zone::Battlefield, "dog", "Blink Dog").named(0, Zone::Battlefield, "split", "Bonesplitter")
        .named(0, Zone::Battlefield, "bears", "Grizzly Bears").named(0, Zone::Battlefield, "split2", "Bonesplitter").hand(0, &["Clever Concealment"]).build();
    let (dog, split, bears, split2) = (t.id("dog"), t.id("split"), t.id("bears"), t.id("split2"));
    let equip = activated(&t, split);
    for (equipment, creature) in [(split, dog), (split2, bears)] {
        t.mana(0, "{1}");
        t.g.act(0, paid("C", Move::Activate { source: equipment, ability: equip, targets: vec![vec![Target::Object(creature)]], paying: vec![] })).unwrap();
        t.resolve();
    }
    cite!(["301.5"]; assert_eq!((t.g.power(dog), t.g.power(bears)), (Some(3), Some(4))));
    teleport(&mut t, 0, dog);
    cite!(["702.26g"]; assert!(phased_out(&t, dog) && phased_out(&t, split) && t.g.obj(split).phased.is_some_and(|p| p.indirect)));
    conceal(&mut t, vec![bears, split2]);
    cite!(["702.26h"]; assert!(phased_out(&t, bears) && phased_out(&t, split2) && t.g.obj(split2).phased.is_some_and(|p| p.indirect)));
    t.to_no_combat(1, Part::PrecombatMain);
    t.to_no_combat(0, Part::PrecombatMain);
    cite!(["702.26g"]; assert!(t.g.battlefield.contains(&split) && t.g.attached_to(split) == Some(Target::Object(dog))));
    cite!(["702.26g", "702.26h"]; assert!(t.g.battlefield.contains(&split2) && t.g.attached_to(split2) == Some(Target::Object(bears))));
    cite!(["301.5"]; assert_eq!((t.g.power(dog), t.g.power(bears)), (Some(3), Some(4))));
}

// Rules: 702.26i, 704.5m
// Given: player 0's Pacifism on player 1's Grizzly Bears; Clever Concealment phases out the
// Pacifism alone.
// When: player 1's turn passes, with or without a Shock killing the Bears first; then
// player 0's untap step comes.
// Expect: with the Bears still there, Pacifism phases in attached to it; with the Bears gone,
// it phases in attached to nothing and is put into its owner's graveyard.
#[test]
fn r702_26i_an_aura_phased_out_directly_phases_in_attached_if_it_can() {
    // Pacifism: "Enchant creature / Enchanted creature can't attack or block."
    for host_dies in [false, true] {
        let mut t = Board::new(2).fill(8).named(1, Zone::Battlefield, "bears", "Grizzly Bears").hand(0, &["Pacifism", "Clever Concealment", "Shock"]).build();
        let bears = t.id("bears");
        t.mana(0, "{1}{W}").cast(0, "Pacifism", vec![Tgt::Object(bears)], mana_pool("{1}{W}")).unwrap();
        t.resolve();
        let pacifism = t.find_in_battlefield("Pacifism");
        conceal(&mut t, vec![pacifism]);
        cite!(["702.26b"]; assert!(phased_out(&t, pacifism) && t.g.obj(pacifism).phased.is_some_and(|p| !p.indirect) && t.g.battlefield.contains(&bears)));
        if host_dies {
            t.mana(0, "{R}").cast(0, "Shock", vec![Tgt::Object(bears)], mana_pool("{R}")).unwrap();
            t.resolve();
            cite!(["704.5g"]; assert!(!t.g.battlefield.contains(&bears) && in_graveyard(&t, 1, "Grizzly Bears")));
        }
        t.to_no_combat(1, Part::PrecombatMain);
        t.to_no_combat(0, Part::PrecombatMain);
        if host_dies {
            cite!(["702.26i", "704.5m"]; assert!(!t.g.battlefield.contains(&pacifism) && !t.g.phased_out.contains(&pacifism) && in_graveyard(&t, 0, "Pacifism")));
        } else {
            cite!(["702.26i"]; assert!(t.g.battlefield.contains(&pacifism) && t.g.attached_to(pacifism) == Some(Target::Object(bears))));
        }
    }
}

// Rules: 702.26g, 502.1, 702.26f
// Given: player 0's Teferi's Curse on player 1's Grizzly Bears.
// When: player 1's untap step comes; then player 0's turn; then player 1's next untap step.
// Expect: the Bears phase out in their controller's untap step, and the Curse, player 0's,
// with them; the Curse stays out through player 0's turn and phases in with the Bears at
// player 1's next untap step, still attached.
#[test]
fn r702_26g_teferis_curse_on_another_players_creature_phases_with_it_in_that_players_untap_step() {
    // Teferi's Curse: "Enchant artifact or creature / Enchanted permanent has phasing."
    let mut t = Board::new(2).fill(8).named(1, Zone::Battlefield, "bears", "Grizzly Bears").hand(0, &["Teferi's Curse"]).build();
    let bears = t.id("bears");
    t.mana(0, "{1}{U}").cast(0, "Teferi's Curse", vec![Tgt::Object(bears)], mana_pool("{1}{U}")).unwrap();
    t.resolve();
    let curse = t.find_in_battlefield("Teferi's Curse");
    t.to_no_combat(1, Part::PrecombatMain);
    cite!(["702.26a", "702.26g"]; assert!(phased_out(&t, bears) && phased_out(&t, curse) && t.g.obj(curse).phased.is_some_and(|p| p.indirect)));
    t.to_no_combat(0, Part::PrecombatMain);
    cite!(["502.1"]; assert!(phased_out(&t, bears) && phased_out(&t, curse)));
    t.to_no_combat(1, Part::PrecombatMain);
    cite!(["502.1", "702.26g"]; assert!(t.g.battlefield.contains(&bears) && t.g.battlefield.contains(&curse)));
    cite!(["702.26g"]; assert_eq!(t.g.attached_to(curse), Some(Target::Object(bears))));
}

// Rules: 506.4, 509.1h, 510.1c
// Given: player 1's Grizzly Bears attacks player 0 and is blocked by Blink Dog.
// When: Blink Dog phases out before combat damage.
// Expect: the Dog is removed from combat; the Bears stay blocked, so with no creature blocking
// them they assign no combat damage: player 0 stays at 20 life.
#[test]
fn r506_4_a_blocker_that_phases_out_leaves_its_attacker_blocked() {
    let mut t = Board::new(2).fill(8).active(1).named(1, Zone::Battlefield, "bears", "Grizzly Bears").named(0, Zone::Battlefield, "dog", "Blink Dog").build();
    let (bears, dog) = (t.id("bears"), t.id("dog"));
    while !matches!(t.g.decision, Some(Decision::Attackers { .. })) { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.g.act(1, Move::Attack(vec![(bears, Target::Player(0))])).unwrap();
    while !matches!(t.g.decision, Some(Decision::Blockers { .. })) { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.g.act(0, Move::Block(vec![(dog, bears)])).unwrap();
    t.g.act(1, Move::Pass).unwrap();
    teleport(&mut t, 0, dog);
    cite!(["506.4", "702.26b"]; assert!(phased_out(&t, dog)));
    cite!(["506.4"]; assert!(t.g.combat.attackers.iter().any(|(a, _)| *a == bears)));
    while t.g.part != Part::PostcombatMain { let p = t.g.decision_maker(t.g.priority); t.g.act(p, Move::Pass).unwrap(); }
    cite!(["509.1h", "510.1c"]; assert_eq!(t.g.players[0].life, 20));
    cite!(["702.26b"]; assert_eq!(t.g.obj(dog).damage, 0));
}
