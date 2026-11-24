//! "Their graveyard", "that player": a zone's owner named by a pronoun is the player the text
//! named (a target, the player each iteration is of, the owner of the graveyard a target was in),
//! never the controller of the ability that the text leaves unsaid (400.3, 608.2c).
use crate::support::{Board, cite, mana_pool};
use game::legal::Asked;
use game::{Move, Target, Zone};

/// Cast a creature whose enters ability targets, and answer its targets.
fn enters_with(t: &mut crate::support::T, name: &str, mana: &str, targets: Vec<Vec<Target>>) {
    t.mana(0, mana);
    cite!(["601.2"]; ok: t.cast(0, name, vec![], mana_pool(mana)));
    t.pass_all();
    let Some((0, Asked::Targets { .. })) = t.asked() else { panic!("no targets asked: {:?}", t.asked()) };
    t.g.act(0, Move::ChooseTargets(targets)).unwrap();
    t.resolve();
}

#[test]
fn r400_3_endurance_puts_the_target_players_graveyard_into_that_players_library() {
    // "up to one target player puts all the cards from their graveyard on the bottom of their
    // library in a random order"
    let mut t = Board::new(2).hand(0, &["Endurance"]).graveyard(0, &["Hill Giant"])
        .graveyard(1, &["Grizzly Bears", "Forest"]).build();
    enters_with(&mut t, "Endurance", "{1}{G}{G}", vec![vec![Target::Player(1)]]);
    t.expect().rules(["400.3", "608.2c"])
        .zone_exact(1, Zone::Graveyard, &[])
        .zone_exact(1, Zone::Library, &["Grizzly Bears", "Forest"])
        .zone_exact(0, Zone::Graveyard, &["Hill Giant"]).check();
}

#[test]
fn r115_1_endurance_with_no_target_moves_no_graveyard() {
    let mut t = Board::new(2).hand(0, &["Endurance"]).graveyard(0, &["Hill Giant"]).graveyard(1, &["Grizzly Bears"]).build();
    enters_with(&mut t, "Endurance", "{1}{G}{G}", vec![vec![]]);
    t.expect().rules(["115.1", "601.2c"])
        .zone_exact(0, Zone::Graveyard, &["Hill Giant"])
        .zone_exact(1, Zone::Graveyard, &["Grizzly Bears"]).check();
}

#[test]
fn r400_3_carrion_locust_takes_life_from_the_owner_of_the_exiled_card() {
    // "exile target card from an opponent's graveyard. If it was a creature card, that player
    // loses 1 life."
    let mut t = Board::new(2).hand(0, &["Carrion Locust"]).graveyard(1, &["Grizzly Bears"]).build();
    let bears = t.find_in(1, Zone::Graveyard, "Grizzly Bears");
    enters_with(&mut t, "Carrion Locust", "{2}{B}", vec![vec![Target::Object(bears)]]);
    t.expect().rules(["400.3", "608.2c"])
        .zone_exact(1, Zone::Exile, &["Grizzly Bears"])
        .life(1, 19).life(0, 20).check();
}

#[test]
fn r608_2c_underworld_cerberus_returns_each_players_creature_cards_to_their_owners_hand() {
    // "When this creature dies, exile it and each player returns all creature cards from their
    // graveyard to their hand": each player's own, to their own hand
    let mut t = Board::new(2).battlefield(0, &["Underworld Cerberus"]).hand(0, &["Murder"])
        .graveyard(0, &["Hill Giant", "Forest"]).graveyard(1, &["Grizzly Bears", "Lightning Bolt"]).build();
    t.mana(0, "{1}{B}{B}");
    cite!(["601.2"]; ok: t.cast(0, "Murder", vec!["Underworld Cerberus".into()], mana_pool("{1}{B}{B}")));
    t.resolve();
    t.expect().rules(["400.3", "608.2c", "603.10a"])
        .zone_exact(0, Zone::Exile, &["Underworld Cerberus"])
        .zone_exact(0, Zone::Hand, &["Hill Giant"]).zone_exact(1, Zone::Hand, &["Grizzly Bears"])
        .zone_exact(0, Zone::Graveyard, &["Forest", "Murder"]).zone_exact(1, Zone::Graveyard, &["Lightning Bolt"]).check();
}

#[test]
fn r608_2c_empty_the_catacombs_returns_each_players_creature_cards_to_their_owners_hand() {
    let mut t = Board::new(3).hand(0, &["Empty the Catacombs"]).graveyard(0, &["Hill Giant"])
        .graveyard(1, &["Grizzly Bears", "Forest"]).graveyard(2, &["Llanowar Elves"]).build();
    t.mana(0, "{3}{B}");
    cite!(["601.2"]; ok: t.cast(0, "Empty the Catacombs", vec![], mana_pool("{3}{B}")));
    t.resolve();
    t.expect().rules(["400.3", "608.2c"])
        .zone_exact(0, Zone::Hand, &["Hill Giant"]).zone_exact(1, Zone::Hand, &["Grizzly Bears"]).zone_exact(2, Zone::Hand, &["Llanowar Elves"])
        .zone_exact(0, Zone::Graveyard, &["Empty the Catacombs"]).zone_exact(1, Zone::Graveyard, &["Forest"]).check();
}

#[test]
fn r601_2c_loaming_shaman_targets_cards_from_the_target_players_graveyard() {
    // "target player shuffles any number of target cards from their graveyard into their
    // library": the cards are in the graveyard of the player targeted before them
    let board = || Board::new(2).hand(0, &["Loaming Shaman"]).graveyard(0, &["Hill Giant"]).graveyard(1, &["Grizzly Bears", "Forest"]);
    let mut t = board().build();
    let (giant, bears) = (t.find_in(0, Zone::Graveyard, "Hill Giant"), t.find_in(1, Zone::Graveyard, "Grizzly Bears"));
    t.mana(0, "{2}{G}");
    cite!(["601.2"]; ok: t.cast(0, "Loaming Shaman", vec![], mana_pool("{2}{G}")));
    t.pass_all();
    let Some((0, Asked::Targets { .. })) = t.asked() else { panic!("no targets asked: {:?}", t.asked()) };
    cite!(["601.2c", "115.1"]; refused("an illegal target, Hill Giant"): t.g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)], vec![Target::Object(giant)]])));
    cite!(["601.2c"]; ok: t.g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)], vec![Target::Object(bears)]])));
    t.resolve();
    t.expect().rules(["601.2c", "701.24a"])
        .zone_exact(1, Zone::Graveyard, &["Forest"]).zone_exact(1, Zone::Library, &["Grizzly Bears"])
        .zone_exact(0, Zone::Graveyard, &["Hill Giant"]).check();
    // its controller may be the player: then their own graveyard's
    let mut t = board().build();
    let giant = t.find_in(0, Zone::Graveyard, "Hill Giant");
    enters_with(&mut t, "Loaming Shaman", "{2}{G}", vec![vec![Target::Player(0)], vec![Target::Object(giant)]]);
    t.expect().rules(["601.2c"]).zone_exact(0, Zone::Library, &["Hill Giant"]).zone_exact(1, Zone::Graveyard, &["Grizzly Bears", "Forest"]).check();
}

#[test]
fn r601_2c_rite_of_renewal_shuffles_cards_from_the_target_players_graveyard() {
    // "Return up to two target permanent cards from your graveyard to your hand. Target player
    // shuffles up to four target cards from their graveyard into their library. Exile Rite of
    // Renewal.": their graveyard is the target player's, not the one the first targets are in
    let mut t = Board::new(2).hand(0, &["Rite of Renewal"]).graveyard(0, &["Hill Giant", "Forest"])
        .graveyard(1, &["Grizzly Bears", "Lightning Bolt"]).build();
    let rite = t.find_in(0, Zone::Hand, "Rite of Renewal");
    let (giant, forest) = (t.find_in(0, Zone::Graveyard, "Hill Giant"), t.find_in(0, Zone::Graveyard, "Forest"));
    let (bears, bolt) = (t.find_in(1, Zone::Graveyard, "Grizzly Bears"), t.find_in(1, Zone::Graveyard, "Lightning Bolt"));
    let cast = |targets: Vec<Vec<Target>>| Move::Paying(mana_pool("{3}{G}"), Box::new(Move::Cast { card: rite, targets }));
    t.mana(0, "{3}{G}");
    cite!(["601.2c", "115.1"]; refused("an illegal target, Forest"):
        t.g.act(0, cast(vec![vec![Target::Object(giant)], vec![Target::Player(1)], vec![Target::Object(forest)]])));
    cite!(["601.2c"]; ok: t.g.act(0, cast(vec![vec![Target::Object(giant)], vec![Target::Player(1)], vec![Target::Object(bears), Target::Object(bolt)]])));
    t.resolve();
    t.expect().rules(["601.2c", "701.24a"])
        .zone_exact(0, Zone::Hand, &["Hill Giant"]).zone_exact(0, Zone::Graveyard, &["Forest"]).zone_exact(0, Zone::Exile, &["Rite of Renewal"])
        .zone_exact(1, Zone::Graveyard, &[]).zone_exact(1, Zone::Library, &["Grizzly Bears", "Lightning Bolt"]).check();
}

#[test]
fn r601_2c_deadeye_trackers_two_targets_are_in_one_opponents_graveyard() {
    // "Exile two target cards from an opponent's graveyard": one opponent's, both of them (the
    // Deadeye Tracker ruling: it can't be activated targeting one card in each of two).
    let mut t = Board::new(3).battlefield(0, &["Deadeye Tracker"]).graveyard(1, &["Grizzly Bears", "Forest"])
        .graveyard(2, &["Hill Giant"]).build();
    let (bears, forest, giant) = (t.find_in(1, Zone::Graveyard, "Grizzly Bears"), t.find_in(1, Zone::Graveyard, "Forest"), t.find_in(2, Zone::Graveyard, "Hill Giant"));
    let tracker = t.find_in_battlefield("Deadeye Tracker");
    let activate = |targets: Vec<Target>| Move::Paying(mana_pool("{1}{B}"), Box::new(Move::Activate { source: tracker, ability: 0, targets: vec![targets], paying: vec![] }));
    t.mana(0, "{1}{B}");
    cite!(["601.2c", "115.1"]; refused("cards in a single graveyard"): t.g.act(0, activate(vec![Target::Object(bears), Target::Object(giant)])));
    cite!(["601.2c"]; ok: t.g.act(0, activate(vec![Target::Object(bears), Target::Object(forest)])));
}

#[test]
fn r601_2c_krosan_reclamation_shuffles_from_the_target_players_graveyard_cast_or_flashed_back() {
    // "Target player shuffles up to two target cards from their graveyard into their library.
    // Flashback {1}{G}": their graveyard is the target player's, whoever casts it, and from
    // wherever it is cast
    let mut t = Board::new(2).hand(0, &["Krosan Reclamation"]).graveyard(0, &["Hill Giant"])
        .graveyard(1, &["Grizzly Bears", "Lightning Bolt", "Forest"]).build();
    let card = t.find_in(0, Zone::Hand, "Krosan Reclamation");
    let giant = t.find_in(0, Zone::Graveyard, "Hill Giant");
    let (bears, bolt) = (t.find_in(1, Zone::Graveyard, "Grizzly Bears"), t.find_in(1, Zone::Graveyard, "Lightning Bolt"));
    let cast = |targets: Vec<Vec<Target>>| Move::Paying(mana_pool("{1}{G}"), Box::new(Move::Cast { card, targets }));
    t.mana(0, "{1}{G}");
    cite!(["601.2c", "115.1"]; refused("an illegal target, Hill Giant"): t.g.act(0, cast(vec![vec![Target::Player(1)], vec![Target::Object(giant)]])));
    cite!(["601.2c", "115.1"]; ok: t.g.act(0, cast(vec![vec![Target::Player(1)], vec![Target::Object(bears), Target::Object(bolt)]])));
    t.resolve();
    t.expect().rules(["601.2c", "701.24a"])
        .zone_exact(1, Zone::Graveyard, &["Forest"]).zone_exact(1, Zone::Library, &["Grizzly Bears", "Lightning Bolt"])
        .zone_exact(0, Zone::Graveyard, &["Hill Giant", "Krosan Reclamation"]).check();
    // cast again from its owner's graveyard, it targets that player and a card there: the
    // card being cast is on the stack, not among them (601.2a)
    let card = t.find_in(0, Zone::Graveyard, "Krosan Reclamation");
    let flashback = |targets: Vec<Vec<Target>>| Move::Paying(mana_pool("{1}{G}"), Box::new(Move::CastVia(game::announcement::CastRoute::Flashback, Box::new(Move::Cast { card, targets }))));
    t.mana(0, "{1}{G}");
    let forest = t.find_in(1, Zone::Graveyard, "Forest");
    cite!(["601.2c", "115.1"]; refused("an illegal target, Forest"): t.g.act(0, flashback(vec![vec![Target::Player(0)], vec![Target::Object(forest)]])));
    cite!(["702.34a", "601.2c"]; ok: t.g.act(0, flashback(vec![vec![Target::Player(0)], vec![Target::Object(giant)]])));
    t.resolve();
    t.expect().rules(["702.34a", "701.24a"])
        .zone_exact(0, Zone::Library, &["Hill Giant"]).zone_exact(0, Zone::Graveyard, &[]).zone_exact(0, Zone::Exile, &["Krosan Reclamation"])
        .zone_exact(1, Zone::Graveyard, &["Forest"]).check();
}

#[test]
fn r700_2_quandrix_commands_fourth_mode_shuffles_from_its_target_players_graveyard() {
    // "Choose two — … • Put two +1/+1 counters on target creature. • Target player shuffles up
    // to three target cards from their graveyard into their library.": the cards of the fourth
    // mode are in its target player's graveyard, not the graveyard of the creature's controller
    // or of the caster
    let board = || Board::new(2).hand(0, &["Quandrix Command"]).battlefield(0, &["Grizzly Bears"])
        .graveyard(0, &["Hill Giant"]).graveyard(1, &["Serra Angel", "Lightning Bolt", "Forest", "Island"]);
    let mut t = board().build();
    let command = t.find_in(0, Zone::Hand, "Quandrix Command");
    let bears = Target::Object(t.find_in_battlefield("Grizzly Bears"));
    let giant = Target::Object(t.find_in(0, Zone::Graveyard, "Hill Giant"));
    let [angel, bolt, forest, island] = ["Serra Angel", "Lightning Bolt", "Forest", "Island"].map(|name| Target::Object(t.find_in(1, Zone::Graveyard, name)));
    let cast = |modes: &[usize], targets: Vec<Vec<Target>>| Move::Paying(mana_pool("{1}{G}{U}"),
        Box::new(Move::Modes(modes.to_vec(), Box::new(Move::Cast { card: command, targets }))));
    t.mana(0, "{1}{G}{U}");
    cite!(["601.2c", "115.1"]; refused("an illegal target, Hill Giant"): t.g.act(0, cast(&[2, 3], vec![vec![bears], vec![Target::Player(1)], vec![angel, giant]])));
    // up to three: a fourth card is one too many (115.1, 601.2c)
    cite!(["601.2c"]; refused("target"): t.g.act(0, cast(&[2, 3], vec![vec![bears], vec![Target::Player(1)], vec![angel, bolt, forest, island]])));
    cite!(["700.2a", "601.2c"]; ok: t.g.act(0, cast(&[2, 3], vec![vec![bears], vec![Target::Player(1)], vec![angel, bolt, forest]])));
    t.resolve();
    t.expect().rules(["700.2a", "701.24a", "122.1a"])
        .counters("Grizzly Bears", "+1/+1", 2)
        .zone_exact(1, Zone::Graveyard, &["Island"]).zone_exact(1, Zone::Library, &["Serra Angel", "Lightning Bolt", "Forest"])
        .zone_exact(0, Zone::Graveyard, &["Hill Giant", "Quandrix Command"]).check();
    // its caster may be the player, with up to three of none: their own graveyard is untouched
    let mut t = board().build();
    let command = t.find_in(0, Zone::Hand, "Quandrix Command");
    let bears = Target::Object(t.find_in_battlefield("Grizzly Bears"));
    let giant = Target::Object(t.find_in(0, Zone::Graveyard, "Hill Giant"));
    let cast = |targets: Vec<Vec<Target>>| Move::Paying(mana_pool("{1}{G}{U}"),
        Box::new(Move::Modes(vec![0, 3], Box::new(Move::Cast { card: command, targets }))));
    t.mana(0, "{1}{G}{U}");
    cite!(["700.2a", "601.2c"]; ok: t.g.act(0, cast(vec![vec![bears], vec![Target::Player(0)], vec![giant]])));
    t.resolve();
    t.expect().rules(["700.2a", "701.24a"])
        .zone_exact(0, Zone::Hand, &["Grizzly Bears"]).zone_exact(0, Zone::Library, &["Hill Giant"])
        .zone_exact(0, Zone::Graveyard, &["Quandrix Command"]).zone_exact(1, Zone::Graveyard, &["Serra Angel", "Lightning Bolt", "Forest", "Island"]).check();
}
