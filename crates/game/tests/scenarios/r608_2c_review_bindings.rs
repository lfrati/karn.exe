//! What a reference names when an earlier sentence made something, or an earlier branch was not
//! taken (608.2c): the text's own nouns, as the instruction is followed.
use crate::support::{Board, cite, mana_pool};
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

