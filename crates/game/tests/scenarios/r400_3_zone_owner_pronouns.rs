//! "Their graveyard", "that player": a zone's owner named by a pronoun is the player the text
//! named (a target, the player each iteration is of, the owner of the graveyard a target was in),
//! never the controller of the ability that the text leaves unsaid (400.3, 608.2c).
use crate::support::{Board, cite, mana_pool, oracle_read};
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
fn r608_2c_underworld_cerberus_stays_refused_until_their_graveyard_is_each_players_own() {
    // Uncited (admission): "each player returns all creature cards from their graveyard to their
    // hand" is each one's own graveyard (608.2c); the engine read "their" as its controller's.
    let refused = game::play::check(&oracle_read("Underworld Cerberus")).map_err(|e| e.0);
    crate::support::refused(refused, "noun phrase: a zone owner's pronoun with no player named before it");
}

#[test]
fn r601_2c_loaming_shaman_stays_refused_until_a_target_can_depend_on_the_target_player() {
    // Uncited (admission): "target cards from their graveyard" is in the graveyard of the target
    // player announced before them (601.2c); the engine has no target relation to an earlier
    // target player yet, so it would offer the cards of its controller's graveyard.
    crate::support::refused(game::play::check(&oracle_read("Loaming Shaman")).map_err(|e| e.0), "noun phrase: a zone owner's pronoun with no player named before it");
}

