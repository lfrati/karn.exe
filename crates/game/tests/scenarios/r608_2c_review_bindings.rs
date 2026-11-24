//! What a reference names when an earlier sentence made something, or an earlier branch was not
//! taken (608.2c): the text's own nouns, as the instruction is followed.
use crate::support::{Board, Boundary, Tgt, cite, mana_pool};
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
fn r608_2c_increasing_confusion_mills_twice_x_cast_from_a_graveyard() {
    // "Target player mills X cards. If this spell was cast from a graveyard, that player mills
    // twice that many cards instead": that many is X, what the act it replaces says
    for (flashback, milled) in [(false, 2), (true, 4)] {
        let board = Board::new(2).fill(6);
        let mut t = if flashback { board.graveyard(0, &["Increasing Confusion"]) } else { board.hand(0, &["Increasing Confusion"]) }.build();
        let zone = if flashback { Zone::Graveyard } else { Zone::Hand };
        let card = t.find_in(0, zone, "Increasing Confusion");
        t.mana(0, "{2}{U}");
        let cast = Move::WithX(2, Box::new(Move::Cast { card, targets: vec![vec![game::Target::Player(1)]] }));
        let cast = if flashback { Move::CastVia(CastRoute::Flashback, Box::new(cast)) } else { cast };
        cite!(["601.2", "702.34a"]; case: flashback; ok: t.g.act(0, Move::Paying(mana_pool("{2}{U}"), Box::new(cast))));
        // the milled cards go to the graveyard in the order their owner chooses
        while let Boundary::Question(p, _) = t.settle() {
            let Some(Decision::Choose { choice: Choice::OrderCards { cards, .. }, .. }) = t.g.decision.clone() else { panic!("{:?}", t.g.decision) };
            t.answer(p, Answer::Objects(cards)).unwrap();
        }
        cite!(["701.17a", "608.2c"]; case: flashback; assert_eq!(t.g.players[1].graveyard.len(), milled));
        cite!(["701.17a"]; case: flashback; assert_eq!(t.g.players[1].library.len(), 6 - milled));
    }
}

#[test]
fn r608_2c_rally_the_righteous_gives_its_target_and_each_creature_sharing_its_color_two_power() {
    // "Untap target creature and each other creature that shares a color with it. Those
    // creatures get +2/+0 until end of turn": those are the target and the others
    let mut t = Board::new(2).hand(0, &["Rally the Righteous"]).tapped(0, &["Raging Goblin", "Goblin Piker"])
        .tapped(1, &["Grizzly Bears"]).build();
    let (raging, piker, bears) = (t.find_in(0, Zone::Battlefield, "Raging Goblin"), t.find_in(0, Zone::Battlefield, "Goblin Piker"), t.find_in(1, Zone::Battlefield, "Grizzly Bears"));
    t.mana(0, "{1}{R}{W}");
    cite!(["601.2"]; ok: t.cast(0, "Rally the Righteous", vec![Tgt::Object(raging)], mana_pool("{1}{R}{W}")));
    t.resolve();
    cite!(["608.2c", "105.4"]; assert!(!t.g.obj(raging).tapped && !t.g.obj(piker).tapped && t.g.obj(bears).tapped));
    cite!(["608.2c", "613.4c"]; assert_eq!((t.g.power(raging), t.g.power(piker), t.g.power(bears)), (Some(3), Some(4), Some(2))));
}
