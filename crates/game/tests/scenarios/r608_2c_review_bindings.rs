//! What a reference names when an earlier sentence made something, or an earlier branch was not
//! taken (608.2c): the text's own nouns, as the instruction is followed.
use crate::support::{Board, cite, mana_pool, oracle_read, refused};
use game::announcement::CastRoute;
use game::{Move, Zone};

#[test]
fn r111_3_the_final_days_from_a_graveyard_creates_those_tokens_tapped() {
    // "Create two tapped 2/2 black Horror creature tokens. If this spell was cast from a
    // graveyard, instead create X of those tokens": those tokens are the tapped Horrors.
    let mut t = Board::new(2).graveyard(0, &["The Final Days", "Grizzly Bears", "Hill Giant", "Llanowar Elves"]).build();
    let days = t.find_in(0, Zone::Graveyard, "The Final Days");
    t.mana(0, "{4}{B}{B}");
    cite!(["702.34a"]; ok: t.g.act(0, Move::Paying(mana_pool("{4}{B}{B}"), Box::new(Move::CastVia(CastRoute::Flashback, Box::new(Move::Cast { card: days, targets: vec![] }))))));
    t.resolve();
    let horrors: Vec<_> = t.g.battlefield.iter().copied().filter(|o| t.g.obj(*o).chars.name == "Horror Token").collect();
    cite!(["608.2c"]; assert_eq!(horrors.len(), 3, "X is the three creature cards"));
    cite!(["111.3", "110.5b"]; assert!(horrors.iter().all(|o| t.g.obj(*o).tapped), "each enters tapped"));
}

#[test]
fn r608_2c_increasing_confusion_stays_refused_until_that_many_reads_the_replaced_amount() {
    // Uncited (admission): "mills X cards. If cast from a graveyard, that player mills twice that
    // many cards instead": that many is X, the amount of the act it replaces; the act's own
    // result is never made on this path, so it is refused rather than milling none.
    refused(game::play::check(&oracle_read("Increasing Confusion")).map_err(|e| e.0), "that many: no preceding group of objects");
}

#[test]
fn r608_2c_rally_the_righteous_stays_refused_until_those_creatures_include_its_target() {
    // Uncited (admission): "untap target creature and each other creature that shares a color
    // with it. Those creatures get +2/+0": those are both; the engine kept only the others.
    refused(game::play::check(&oracle_read("Rally the Righteous")).map_err(|e| e.0), "reference: a plural of one act's single and several objects");
}
