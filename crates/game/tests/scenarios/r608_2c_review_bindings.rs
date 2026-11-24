//! What a reference names when an earlier sentence made something, or an earlier branch was not
//! taken (608.2c): the text's own nouns, as the instruction is followed.
use crate::support::{Board, Tgt, cite, mana_pool, oracle_read, refused};
use game::announcement::CastRoute;
use game::state::{Answer, Choice, Decision};
use game::{Move, Zone};

#[test]
fn r608_2c_protect_the_negotiators_asks_the_target_spells_controller_to_pay() {
    // "If this spell was kicked, create a 1/1 white Soldier creature token. Counter target spell
    // unless its controller pays {1} for each creature you control." Unkicked, no token: "its
    // controller" is the target spell's, who may pay {1} for the one creature.
    let mut t = Board::new(2).active(1).battlefield(0, &["Grizzly Bears"]).hand(0, &["Protect the Negotiators"])
        .hand(1, &["Hill Giant"]).build();
    t.mana(1, "{3}{R}");
    cite!(["601.2"]; ok: t.cast(1, "Hill Giant", vec![], mana_pool("{3}{R}")));
    t.g.act(1, Move::Pass).unwrap();
    t.mana(0, "{1}{U}");
    cite!(["601.2"]; ok: t.cast(0, "Protect the Negotiators", vec![Tgt::In(1, Zone::Stack, "Hill Giant")], mana_pool("{1}{U}")));
    t.mana(1, "{1}");
    while !matches!(t.g.decision, Some(Decision::Choose { .. })) && !t.g.stack.is_empty() { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    cite!(["608.2c", "118.12"]; assert!(matches!(&t.g.decision, Some(Decision::Choose { player: 1, choice: Choice::Pay { .. } })),
        "the Giant's controller is asked to pay: {:?}", t.g.decision));
    cite!(["118.12"]; ok: t.g.act(1, Move::Choose(Answer::Pay { plan: vec![], with: Some(mana_pool("C")), tapped: vec![] })));
    t.resolve();
    t.expect().rules(["118.12", "608.2c"]).zone_exact(1, Zone::Battlefield, &["Hill Giant"])
        .zone_exact(0, Zone::Graveyard, &["Protect the Negotiators"]).check();
}

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
