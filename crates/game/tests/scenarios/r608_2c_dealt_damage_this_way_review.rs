//! Review of "dealt damage this way" (608.2c), for the cards its own tests don't play: a
//! requirement to attack and a tap of only the creatures that were dealt damage, a casting
//! restriction on the players that were, three targets that must be different, damage to
//! fliers and players, threshold, a spell that shuffles itself away, a delayed draw, a
//! sacrifice that destroys everything, and the graveyard clause of Cry of the Carnarium. Every
//! card is read from its Oracle text.
use crate::support::*;
use game::state::Decision;

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Aggravate", "Aurelia's Fury", "Serpentine Spike", "Flamebreak", "Toxic Stench", "Red Sun's Zenith", "Gravebind", "Planar Collapse",
        "Cry of the Carnarium", "Inviolability", "Ambush Viper", "Drudge Skeletons", "Shock", "Lightning Bolt", "Hill Giant", "Craw Wurm",
        "Savannah Lions", "Serra Angel", "Grizzly Bears", "Mountain"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

/// Player 0 casts this card from their hand at these targets, paying `cost`, and it resolves.
fn cast(t: &mut T, name: &str, targets: Vec<Tgt>, cost: &str) {
    let pool: String = cost.split('{').filter(|s| !s.is_empty()).map(|s| s.trim_end_matches('}'))
        .map(|s| s.parse::<usize>().map_or(s.to_string(), |n| "C".repeat(n))).collect();
    t.mana(0, cost).cast(0, name, targets, mana_pool(&pool)).unwrap().resolve();
}

/// Pass, declaring no attackers or blockers, until `done`.
fn pass_until(t: &mut T, done: impl Fn(&T) -> bool) {
    for _ in 0..60 {
        if done(t) { return; }
        if let Some(Decision::Order { player, abilities }) = t.g.decision.clone() {
            t.g.act(player, Move::Order((0..abilities.len()).collect())).unwrap();
            continue;
        }
        // cards put into a graveyard at once, in the order they are offered
        if let Some((p, Asked::OrderCards { cards, .. })) = t.g.asked() {
            t.answer(p, Answer::Objects(cards)).unwrap();
            continue;
        }
        match t.g.decision.clone() {
            Some(Decision::Attackers { player }) => t.g.act(player, Move::Attack(vec![])).unwrap(),
            Some(Decision::Blockers { player }) => t.g.act(player, Move::Block(vec![])).unwrap(),
            _ => { let p = t.g.decision_maker(t.g.priority); t.g.act(p, Move::Pass).unwrap() }
        }
    }
    panic!("never reached: part {:?}, decision {:?}", t.g.part, t.g.decision);
}

// Rules: 508.1d, 608.2c, 615.1a
// Given: player 1's Hill Giant (3/3), Craw Wurm (6/4) enchanted by player 0's Inviolability
// ("Prevent all damage that would be dealt to enchanted creature."), and Savannah Lions (2/1), in
// player 1's first main phase.
// When: player 0 casts Aggravate at player 1.
// Expect: the Lions dies; the Giant was dealt damage and must attack; the Wurm's damage was
// prevented, so it was not dealt damage this way and need not attack.
#[test]
fn r508_1d_aggravate_makes_only_the_creatures_dealt_damage_attack() {
    // Aggravate: "Aggravate deals 1 damage to each creature target player controls. Each
    // creature dealt damage this way attacks this turn if able."
    let mut t = Board::new(2).fill(8).active(1).at(Part::PrecombatMain).hand(0, &["Aggravate"]).battlefield(0, &["Inviolability"])
        .battlefield(1, &["Hill Giant", "Craw Wurm", "Savannah Lions"]).build();
    let (giant, wurm) = (t.find_in_battlefield("Hill Giant"), t.find_in_battlefield("Craw Wurm"));
    let inviolability = t.find_in_battlefield("Inviolability");
    assert!(t.g.attach(inviolability, Target::Object(wurm)));
    t.g.act(1, Move::Pass).unwrap();
    cast(&mut t, "Aggravate", vec![Tgt::Player(1)], "{3}{R}{R}");
    t.expect().rules(["608.2c", "704.5g"]).zone(1, Zone::Graveyard, &["Savannah Lions"]).check();
    cite!(["615.1a"]; assert_eq!((t.g.obj(giant).damage, t.g.obj(wurm).damage), (1, 0)));
    pass_until(&mut t, |t| matches!(t.g.decision, Some(Decision::Attackers { player: 1 })));
    let attack = |t: &T, with: &[ObjId]| t.g.clone().act(1, Move::Attack(with.iter().map(|o| (*o, Target::Player(0))).collect()));
    cite!(["508.1d", "608.2c"]; refused("Hill Giant attacks if able (508.1d)"): attack(&t, &[]));
    cite!(["508.1d", "608.2c"]; refused("Hill Giant attacks if able (508.1d)"): attack(&t, &[wurm]));
    // the Wurm need not
    cite!(["508.1d", "615.1a"]; ok: attack(&t, &[giant]));
    cite!(["508.1d"]; ok: attack(&t, &[giant, wurm]));
}

// Rules: 608.2c, 601.3
// Given: player 1's Hill Giant; Lightning Bolt and Ambush Viper (a creature with flash) in player
// 1's hand.
// When: player 0 casts Aurelia's Fury with X 2, dividing 1 to the Giant and 1 to player 1, and
// then passes priority.
// Expect: the Giant is tapped; player 1 can't cast Lightning Bolt, a noncreature spell, this
// turn, but can cast Ambush Viper.
#[test]
fn r608_2c_aurelias_fury_taps_the_creatures_and_restricts_the_players_dealt_damage() {
    // Aurelia's Fury: "Aurelia's Fury deals X damage divided as you choose among any number of
    // targets. Tap each creature dealt damage this way. Players dealt damage this way can't cast
    // noncreature spells this turn."
    let mut t = Board::new(2).fill(8).hand(0, &["Aurelia's Fury"]).hand(1, &["Lightning Bolt", "Ambush Viper"]).battlefield(1, &["Hill Giant"]).build();
    let giant = t.find_in_battlefield("Hill Giant");
    let fury = t.find_in(0, Zone::Hand, "Aurelia's Fury");
    t.mana(0, "{2}{R}{W}");
    t.g.act(0, Move::WithDivision(vec![vec![1, 1]], Box::new(Move::WithX(2, Box::new(paid("CCRW",
        Move::Cast { card: fury, targets: vec![vec![Target::Object(giant), Target::Player(1)]] })))))).unwrap();
    t.resolve();
    cite!(["608.2c"]; assert_eq!((t.g.obj(giant).tapped, t.g.obj(giant).damage, t.g.players[1].life), (true, 1, 19)));
    t.g.act(0, Move::Pass).unwrap();
    let bolt = t.find_in(1, Zone::Hand, "Lightning Bolt");
    let viper = t.find_in(1, Zone::Hand, "Ambush Viper");
    t.mana(1, "{1}{R}{G}");
    cite!(["608.2c", "601.3"]; refused("an effect prohibits casting that spell (601.3)"):
        t.g.clone().act(1, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Player(0)]] })));
    cite!(["608.2c", "601.3"]; ok: t.g.clone().act(1, paid("CG", Move::Cast { card: viper, targets: vec![] })));
}

// Rules: 115.1, 601.2c, 614.1a, 608.2c
// Given: player 1's Hill Giant (3/3), Grizzly Bears (2/2) and Serra Angel (4/4).
// When: player 0 casts Serpentine Spike with the Giant as two of its targets, then as the first
// target with the Bears and the Angel; then Shocks the Giant.
// Expect: the same creature can't be two of its targets; the Bears and the Angel die of its
// damage and are exiled; the Giant lives with 2 damage, and when Shock kills it later in the
// turn it is exiled too.
#[test]
fn r614_1a_serpentine_spike_exiles_each_of_its_three_creatures_whenever_it_dies() {
    // Serpentine Spike: "Serpentine Spike deals 2 damage to target creature, 3 damage to another
    // target creature, and 4 damage to a third target creature. If a creature dealt damage this
    // way would die this turn, exile it instead."
    let mut t = Board::new(2).fill(8).hand(0, &["Serpentine Spike", "Shock"]).battlefield(1, &["Hill Giant", "Grizzly Bears", "Serra Angel"]).build();
    let (giant, bears, angel) = (t.find_in_battlefield("Hill Giant"), t.find_in_battlefield("Grizzly Bears"), t.find_in_battlefield("Serra Angel"));
    let spike = t.find_in(0, Zone::Hand, "Serpentine Spike");
    t.mana(0, "{5}{R}{R}");
    let targets = |a: ObjId, b: ObjId, c: ObjId| vec![vec![Target::Object(a)], vec![Target::Object(b)], vec![Target::Object(c)]];
    cite!(["115.1", "601.2c"]; refused("another target must differ from the preceding targets"):
        t.g.clone().act(0, paid("CCCCCRR", Move::Cast { card: spike, targets: targets(giant, giant, angel) })));
    t.g.act(0, paid("CCCCCRR", Move::Cast { card: spike, targets: targets(giant, bears, angel) })).unwrap();
    t.resolve();
    t.expect().rules(["614.1a", "608.2c"]).zone(1, Zone::Exile, &["Grizzly Bears", "Serra Angel"]).zone(1, Zone::Graveyard, &[]).zone(1, Zone::Battlefield, &["Hill Giant"]).check();
    cite!(["120.3"]; assert_eq!(t.g.obj(giant).damage, 2));
    cast(&mut t, "Shock", vec![Tgt::Object(giant)], "{R}");
    t.expect().rules(["614.1a", "608.2c"]).zone(1, Zone::Exile, &["Grizzly Bears", "Serra Angel", "Hill Giant"]).zone(1, Zone::Graveyard, &[]).check();
}

// Rules: 701.19c, 608.2c, 120.3
// Given: player 1's Drudge Skeletons (1/1) and Craw Wurm (6/4), each with a regeneration shield,
// and Serra Angel (4/4, flying).
// When: player 0 casts Flamebreak.
// Expect: the Skeletons is dealt damage this way and is not regenerated; the Wurm lives with 3
// damage; the Angel, flying, is dealt none; each player loses 3 life.
#[test]
fn r701_19c_flamebreak_damages_what_has_no_flying_and_leaves_shields_unapplied() {
    // Flamebreak: "Flamebreak deals 3 damage to each creature without flying and each player.
    // Creatures dealt damage this way can't be regenerated this turn."
    let mut t = Board::new(2).fill(8).hand(0, &["Flamebreak"]).battlefield(1, &["Drudge Skeletons", "Serra Angel", "Craw Wurm"]).build();
    let (skeletons, wurm, angel) = (t.find_in_battlefield("Drudge Skeletons"), t.find_in_battlefield("Craw Wurm"), t.find_in_battlefield("Serra Angel"));
    t.g.regenerate(skeletons);
    t.g.regenerate(wurm);
    cast(&mut t, "Flamebreak", vec![], "{R}{R}{R}");
    t.expect().rules(["701.19c", "608.2c"]).zone(1, Zone::Graveyard, &["Drudge Skeletons"]).zone(1, Zone::Battlefield, &["Serra Angel", "Craw Wurm"]).check();
    cite!(["608.2c", "120.3"]; assert_eq!((t.g.obj(wurm).damage, t.g.obj(angel).damage), (3, 0)));
    cite!(["120.3"]; assert_eq!((t.g.players[0].life, t.g.players[1].life), (17, 17)));
}

// Rules: 207.2c, 701.19c
// Given: player 1's Hill Giant (3/3) with a regeneration shield; six or seven Mountains in player
// 0's graveyard.
// When: player 0 casts Toxic Stench at the Giant.
// Expect: with six, the Giant gets -1/-1 and lives; with seven, it is destroyed and the shield
// doesn't save it.
#[test]
fn r701_19c_toxic_stench_with_threshold_destroys_and_does_not_regenerate() {
    // Toxic Stench: "Target nonblack creature gets -1/-1 until end of turn. Threshold — If seven
    // or more cards are in your graveyard, instead destroy that creature. It can't be
    // regenerated."
    for cards in [6, 7] {
        let mut t = Board::new(2).fill(8).hand(0, &["Toxic Stench"]).battlefield(1, &["Hill Giant"]).graveyard(0, &vec!["Mountain"; cards]).build();
        let giant = t.find_in_battlefield("Hill Giant");
        t.g.regenerate(giant);
        cast(&mut t, "Toxic Stench", vec![Tgt::Object(giant)], "{1}{B}");
        if cards == 6 {
            cite!(["207.2c"]; case: format!("{cards} cards"); assert_eq!((t.g.power(giant), t.g.toughness(giant)), (Some(2), Some(2))));
        } else {
            t.expect().rules(["701.19c", "207.2c"]).zone(1, Zone::Graveyard, &["Hill Giant"]).check();
        }
    }
}

// Rules: 614.1a, 608.2c, 701.24a
// Given: player 1's Grizzly Bears.
// When: player 0 casts Red Sun's Zenith with X 2 at the Bears.
// Expect: the Bears dies of its damage and is exiled; Red Sun's Zenith is shuffled into player
// 0's library, not put into their graveyard.
#[test]
fn r614_1a_red_suns_zenith_exiles_the_creature_and_shuffles_itself_away() {
    // Red Sun's Zenith: "Red Sun's Zenith deals X damage to any target. If a creature dealt
    // damage this way would die this turn, exile it instead. Shuffle Red Sun's Zenith into its
    // owner's library."
    let mut t = Board::new(2).fill(8).hand(0, &["Red Sun's Zenith"]).battlefield(1, &["Grizzly Bears"]).build();
    let bears = t.find_in_battlefield("Grizzly Bears");
    let zenith = t.find_in(0, Zone::Hand, "Red Sun's Zenith");
    let library = t.g.players[0].library.len();
    t.mana(0, "{2}{R}");
    t.g.act(0, paid("CCR", Move::WithX(2, Box::new(Move::Cast { card: zenith, targets: vec![vec![Target::Object(bears)]] })))).unwrap();
    t.resolve();
    t.expect().rules(["614.1a", "608.2c"]).zone(1, Zone::Exile, &["Grizzly Bears"]).zone(1, Zone::Graveyard, &[]).zone(0, Zone::Graveyard, &[]).check();
    cite!(["701.24a"]; assert_eq!(t.g.players[0].library.len(), library + 1));
}

// Rules: 701.19c, 603.7a, 504.1
// Given: player 1's Drudge Skeletons (1/1).
// When: player 0 casts Gravebind at it; the Skeletons then gets a regeneration shield and is
// Shocked.
// Expect: it is not regenerated and dies; player 0 draws a card at the beginning of the next
// turn's upkeep, not before.
#[test]
fn r701_19c_gravebind_stops_a_later_shield_and_draws_at_the_next_upkeep() {
    // Gravebind: "Target creature can't be regenerated this turn. Draw a card at the beginning of
    // the next turn's upkeep."
    let mut t = Board::new(2).fill(8).hand(0, &["Gravebind", "Shock"]).battlefield(1, &["Drudge Skeletons"]).build();
    let skeletons = t.find_in_battlefield("Drudge Skeletons");
    cast(&mut t, "Gravebind", vec![Tgt::Object(skeletons)], "{B}");
    t.g.regenerate(skeletons);
    cast(&mut t, "Shock", vec![Tgt::Object(skeletons)], "{R}");
    t.expect().rules(["701.19c"]).zone(1, Zone::Graveyard, &["Drudge Skeletons"]).check();
    let hand = t.g.players[0].hand.len();
    pass_until(&mut t, |t| t.g.active == 1 && t.g.part == Part::Upkeep);
    cite!(["603.7a"]; assert_eq!(t.g.players[0].hand.len(), hand));
    pass_until(&mut t, |t| t.g.active == 1 && t.g.part == Part::Draw);
    cite!(["603.7a", "504.1"]; assert_eq!(t.g.players[0].hand.len(), hand + 1));
}

// Rules: 603.4, 701.19c, 701.8a
// Given: player 0's Planar Collapse; three or four creatures of player 1's, among them Drudge
// Skeletons with a regeneration shield.
// When: player 0's upkeep begins.
// Expect: with three there is no trigger; with four, Planar Collapse is sacrificed and every
// creature is destroyed, the Skeletons not regenerated.
#[test]
fn r603_4_planar_collapse_destroys_all_creatures_only_with_four_of_them() {
    // Planar Collapse: "At the beginning of your upkeep, if there are four or more creatures on
    // the battlefield, sacrifice Planar Collapse and destroy all creatures. They can't be
    // regenerated."
    let creatures = ["Drudge Skeletons", "Grizzly Bears", "Hill Giant", "Savannah Lions"];
    for count in [3, 4] {
        let mut t = Board::new(2).fill(8).active(1).at(Part::End).battlefield(0, &["Planar Collapse"]).battlefield(1, &creatures[..count]).build();
        let skeletons = t.find_in_battlefield("Drudge Skeletons");
        t.g.regenerate(skeletons);
        pass_until(&mut t, |t| t.g.active == 0 && t.g.part == Part::Draw);
        if count == 3 {
            t.expect().rules(["603.4"]).zone(0, Zone::Battlefield, &["Planar Collapse"]).zone(1, Zone::Graveyard, &[]).check();
        } else {
            t.expect().rules(["603.4", "701.19c"]).zone(0, Zone::Graveyard, &["Planar Collapse"]).zone(1, Zone::Battlefield, &[])
                .zone(1, Zone::Graveyard, &creatures).check();
        }
    }
}

// Rules: 400.7, 603.10a, 614.1a
// Given: player 1's Grizzly Bears and Savannah Lions, Hill Giant in player 1's graveyard, and
// player 0's Serra Angel.
// When: player 0 Shocks the Bears, then casts Cry of the Carnarium.
// Expect: the Lions would die of -2/-2 and is exiled instead; the Bears, put into the graveyard
// from the battlefield this turn, is exiled; the Hill Giant was not and stays; the Angel is 2/2.
#[test]
fn r400_7_cry_of_the_carnarium_exiles_the_creature_cards_put_there_from_the_battlefield() {
    // Cry of the Carnarium: "Each creature gets -2/-2 until end of turn. Exile all creature cards
    // in all graveyards that were put there from the battlefield this turn. If a creature would
    // die this turn, exile it instead."
    let mut t = Board::new(2).fill(8).hand(0, &["Shock", "Cry of the Carnarium"]).battlefield(0, &["Serra Angel"])
        .battlefield(1, &["Grizzly Bears", "Savannah Lions"]).graveyard(1, &["Hill Giant"]).build();
    let angel = t.find_in_battlefield("Serra Angel");
    cast(&mut t, "Shock", vec![Tgt::In(1, Zone::Battlefield, "Grizzly Bears")], "{R}");
    cast(&mut t, "Cry of the Carnarium", vec![], "{1}{B}{B}");
    t.expect().rules(["400.7", "603.10a", "614.1a"]).zone(1, Zone::Graveyard, &["Hill Giant"]).zone(1, Zone::Exile, &["Grizzly Bears", "Savannah Lions"])
        .zone(0, Zone::Graveyard, &["Shock", "Cry of the Carnarium"]).check();
    cite!(["613.4c"]; assert_eq!((t.g.power(angel), t.g.toughness(angel)), (Some(2), Some(2))));
}
