//! What a clause of a text says of its own words: whose "its" is (608.2h), and what X is where
//! the sentence says it (107.3c). Every text is read as printed and refused, where it is
//! refused, before anything is cast.

use game::card::CardDef;
use game::play::check;
use game::state::Answer;
use game::{Game, Move, ObjId, Target, Zone};
use crate::support::{cite, paid};

const BEARS: usize = 1;
const GIANT: usize = 2;

fn card(name: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    CardDef::new(name, "{0}", types, text, pt).expect(name)
}

/// A game where the first definition has this text.
fn game(types: &str, text: &str, pt: Option<(i32, i32)>) -> Game {
    let defs = vec![
        card("Witness", types, text, pt),
        card("Grizzly Bears", "Creature — Bear", "", Some((2, 2))),
        card("Hill Giant", "Creature — Giant", "", Some((3, 3))),
    ];
    check(&defs[0]).unwrap_or_else(|u| panic!("{text}: {}", u.0));
    Game::new(defs, 2, 20)
}

fn refusal(text: &str) -> String {
    check(&card("Witness", "Sorcery", text, None)).unwrap_err().0
}

fn all_pass(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}

fn cast(g: &mut Game, targets: Vec<Vec<Target>>) {
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets }).unwrap();
    all_pass(g);
}

fn counters(g: &Game, o: ObjId) -> i32 {
    g.power(o).unwrap() - 2
}

#[test]
fn r608_2h_its_is_of_the_subject_of_its_clause_whatever_was_named_since() {
    // "its power" is the first target's, though a second is named before the number is said
    let mut g = game("Sorcery", "Target creature you control deals damage equal to its power to target creature you don't control.", None);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    cast(&mut g, vec![vec![Target::Object(bears)], vec![Target::Object(giant)]]);
    cite!(["608.2c", "608.2h"]; assert_eq!((g.obj(giant).damage, g.obj(bears).damage), (2, 0)));

    // and the pronoun that is the subject is what an earlier sentence named
    let mut g = game("Sorcery", "Tap target creature you control. It deals damage equal to its power to target creature you don't control.", None);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    cast(&mut g, vec![vec![Target::Object(bears)], vec![Target::Object(giant)]]);
    assert!(g.obj(bears).tapped);
    cite!(["608.2c", "608.2h"]; assert_eq!((g.obj(giant).zone, g.obj(giant).damage), (Zone::Battlefield, 2)));

    // of the source, where the source is the subject: not of the one target named
    let mut g = game("Creature — Archer", "{T}: This creature deals damage equal to its power to target creature.", Some((1, 1)));
    let source = g.put(0, 0, Zone::Battlefield);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    g.act(0, Move::Activate { source, ability: 0, targets: vec![vec![Target::Object(giant)]], paying: vec![] }).unwrap();
    all_pass(&mut g);
    cite!(["608.2c", "608.2h"]; assert_eq!((g.obj(giant).zone, g.obj(giant).damage), (Zone::Battlefield, 1)));
}

#[test]
fn r608_2h_its_of_another_sentence_is_not_guessed_among_what_was_named() {
    // the subject of the last clause is "you": nothing says which target "its" is
    let u = refusal("Tap target creature. Untap target creature. You gain life equal to its power.");
    assert_eq!(u, "reference: ambiguous earlier subject producers");
    // one thing named: that one
    let mut g = game("Sorcery", "Tap target creature. You gain life equal to its power.", None);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    cast(&mut g, vec![vec![Target::Object(giant)]]);
    cite!(["608.2c", "608.2h"]; assert_eq!(g.players[0].life, 23));
    // nothing named: nothing it could be
    assert!(refusal("You gain life equal to its power.").contains("input reference"));
}

#[test]
fn r107_3c_x_said_of_the_acts_own_target_is_of_that_target() {
    let mut g = game("Sorcery", "Put X +1/+1 counters on target creature, where X is that creature's power.", None);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    cast(&mut g, vec![vec![Target::Object(giant)]]);
    cite!(["107.3c"]; assert_eq!(g.power(giant), Some(6)));

    // inside "you may": computed only where the act is done
    for (answer, grown) in [(Answer::Yes, 2), (Answer::No, 0)] {
        let mut g = game("Sorcery", "You may put X +1/+1 counters on target creature, where X is that creature's power.", None);
        let bears = g.put(0, BEARS, Zone::Battlefield);
        cast(&mut g, vec![vec![Target::Object(bears)]]);
        g.act(0, Move::Choose(answer)).unwrap();
        cite!(["107.3c"]; case: format!("{grown}"); assert_eq!(counters(&g, bears), grown));
    }
}

#[test]
fn r107_3c_x_of_a_clause_is_computed_once_for_each_of_its_acts() {
    // two cards in hand as it resolves: two drawn, and two life lost though the hand is
    // then four
    let mut g = game("Sorcery", "Target player draws X cards and loses X life, where X is the number of cards in your hand.", None);
    for _ in 0..6 {
        g.put(0, BEARS, Zone::Library);
    }
    g.put(0, BEARS, Zone::Hand);
    g.put(0, BEARS, Zone::Hand);
    cast(&mut g, vec![vec![Target::Player(0)]]);
    cite!(["107.3c", "608.2h"]; assert_eq!((g.players[0].hand.len(), g.players[0].life), (4, 18)));

    // what follows the clause has no X
    assert_eq!(refusal("Target player draws X cards and loses X life, where X is the number of cards in your hand. You gain X life."), "amount: X");
    // and "where X is" closes its clause: said of an earlier act of it, it is not followed
    assert!(refusal("You gain X life. You draw X cards, where X is the number of cards in your hand.").contains("amount: X"));
}

#[test]
fn r608_2h_coordinated_subjects_share_one_value_but_new_definitions_recompute() {
    // CR 608.2h/107.3i: draw changes the measured hand, but the coordinated
    // gain uses the same X=2. A new sentence independently defines X=4.
    let mut g = game("Sorcery", "Target player draws X cards and you gain X life, where X is the number of cards in your hand. You gain X life, where X is the number of cards in your hand.", None);
    for _ in 0..6 { g.put(0, BEARS, Zone::Library); }
    for _ in 0..2 { g.put(0, BEARS, Zone::Hand); }
    cast(&mut g, vec![vec![Target::Player(0)]]);
    cite!(["107.3c", "107.3i", "608.2c", "608.2h"]; assert_eq!((g.players[0].hand.len(),g.players[0].life),(4,26)));
}

#[test]
fn r608_2d_coordinated_value_survives_a_real_choice_after_its_first_use() {
    let mut g = game("Sorcery", "You draw X cards and choose a color, then target player gains X life, where X is the number of cards in your hand.", None);
    for _ in 0..6 { g.put(0, BEARS, Zone::Library); }
    for _ in 0..2 { g.put(0, BEARS, Zone::Hand); }
    cast(&mut g, vec![vec![Target::Player(0)]]);
    cite!(["608.2c", "608.2d"]; assert_eq!((g.players[0].hand.len(),g.players[0].life),(4,20)));
    cite!(["608.2d"]; assert!(matches!(g.decision,Some(game::state::Decision::Choose {player:0,..}))));
    g.act(0,Move::Choose(Answer::Value(game::program::Value::Color(ir::Color::Blue)))).unwrap();
    cite!(["608.2d", "608.2h"]; assert_eq!((g.players[0].hand.len(),g.players[0].life),(4,22)));
    assert!(g.stack.is_empty() && g.decision.is_none());
}

#[test]
fn r611_2d_coordinated_changes_snapshot_before_the_first_changes_the_count() {
    let mut g = game("Sorcery", "Target creature you control gets +X/+0 until end of turn and target creature an opponent controls gets -0/-X until end of turn, where X is the number of creatures you control with power 2 or less.", None);
    let own = g.put(0,BEARS,Zone::Battlefield); let other = g.put(1,GIANT,Zone::Battlefield);
    cast(&mut g,vec![vec![own.into()],vec![other.into()]]);
    // First +1 power removes our only creature from the counted set. The
    // independently calculated second modifier still uses retained X=1.
    cite!(["608.2h", "611.2d"]; assert_eq!((g.power(own),g.toughness(other)),(Some(3),Some(2))));
}

#[test]
fn r608_2c_native_haunt_counts_preceding_tokens_and_a_response() {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let haunt=CardDef::from_faces(&CardDef::catalog(&faces)["Haunt the Network"]).unwrap();
    check(&haunt).unwrap();
    let mut g=Game::new(vec![haunt,card("Rock","Artifact","",None),card("Response","Instant","Create a Treasure token.",None)],2,20);
    g.put(0,1,Zone::Battlefield);g.players[0].pool.colors=[10;5];
    let spell=g.put(0,0,Zone::Hand);g.act(0,paid("UUUUB",Move::Cast {card:spell,targets:vec![vec![Target::Player(1)]]})).unwrap();
    let response=g.put(0,2,Zone::Hand);g.act(0,Move::Cast {card:response,targets:vec![]}).unwrap();
    all_pass(&mut g);all_pass(&mut g);
    // Oracle creates two artifact Thopters BEFORE either life change. Rock +
    // response Treasure + two Thopters = four, determined on resolution.
    cite!(["107.3c", "608.2c", "608.2h"]; assert_eq!((g.players[0].life,g.players[1].life),(24,16)));
    let thopters:Vec<_>=g.battlefield.iter().filter(|o|g.seen(**o,None).subtypes.iter().any(|s|s.name()=="Thopter")).collect();
    assert_eq!(thopters.len(),2);
    assert!(thopters.iter().all(|o|g.power(**o)==Some(1)&&g.toughness(**o)==Some(1)&&g.has(**o,"Flying")));
}

#[test]
fn r611_2d_native_monoist_uses_live_artifact_count_and_surviving_target() {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let native=|name|CardDef::from_faces(&CardDef::catalog(&faces)[name]).unwrap();
    let monoist=native("Monoist Circuit-Feeder");check(&monoist).unwrap();
    let mut g=Game::new(vec![monoist,card("Rock","Artifact","",None),
        card("Giant","Creature — Giant","",Some((3,3))),
        card("Bounce","Instant","Return target creature to its owner's hand.",None),
        card("Response","Instant","Create a Treasure token.",None)],2,20);
    g.players[0].pool.colors=[20;5];g.put(0,1,Zone::Battlefield);
    let other=g.put(1,2,Zone::Battlefield);
    let spell=g.put(0,0,Zone::Hand);g.act(0,paid("BBBBBB",Move::Cast {card:spell,targets:vec![]})).unwrap();all_pass(&mut g);
    let source=*g.battlefield.iter().find(|o|g.obj(**o).card==Some(0)).unwrap();
    g.act(0,Move::ChooseTargets(vec![vec![source.into()],vec![other.into()]])).unwrap();
    // Announcement has two artifacts. Bounce the source/own target in response;
    // the trigger survives, with only Rock counted as it resolves (ruling eb0319...).
    let bounce=g.put(0,3,Zone::Hand);g.act(0,Move::Cast {card:bounce,targets:vec![vec![source.into()]]}).unwrap();
    all_pass(&mut g);all_pass(&mut g);
    cite!(["608.2b", "608.2h", "611.2d"]; assert_eq!(g.toughness(other),Some(2)));
    // Ruling 0b697a...: losing one target does not suppress the other modifier.
    assert!(g.players[0].hand.iter().any(|o|g.obj(*o).card==Some(0)));
    let response=g.put(0,4,Zone::Hand);g.act(0,Move::Cast {card:response,targets:vec![]}).unwrap();all_pass(&mut g);
    cite!(["611.2d"]; assert_eq!(g.toughness(other),Some(2)));
}

#[test]
fn r608_2d_declining_first_optional_use_preserves_x_for_later_subject() {
    let mut g = game("Sorcery", "Target player may draw X cards and you gain X life, where X is the number of cards in your hand.", None);
    for _ in 0..2 { g.put(0, BEARS, Zone::Hand); }
    for _ in 0..6 { g.put(1, BEARS, Zone::Library); }
    cast(&mut g, vec![vec![Target::Player(1)]]);
    g.act(1, Move::Choose(Answer::No)).unwrap();
    cite!(["107.3c", "608.2d", "608.2h"]; assert_eq!((g.players[0].life, g.players[1].hand.len()), (22, 0)));
}

#[test]
fn r107_3c_cross_branch_first_definitions_are_explicitly_refused() {
    for text in [
        "You may draw X cards and discard a card, then target player gains X life, where X is the number of cards in your hand.",
        "You draw X cards if you control an artifact, then target player gains X life, where X is the number of cards in your hand.",
        "You may add X mana of any one color, then target player gains X life, where X is the number of cards in your hand.",
    ] {
        // The later consumer runs even if the earlier branch does not. The current
        // retained value has no branch-aware initialization, so admission must refuse.
        cite!(["107.3c", "608.2c", "608.2d"]; case: text.to_owned();
            assert!(check(&CardDef::new("Witness", "{0}", "Sorcery", text, None).unwrap_or_else(|e| panic!("{text}: {e:?}"))).is_err_and(|e| e.0.contains("first definition may be skipped")), "{text}"));
    }
}

#[test]
fn r608_2d_first_mana_use_defines_x_before_its_suspended_color_choice() {
    for (lead, creature) in [("", false), ("If you control a creature, ", true)] {
        let text=format!("{lead}you add X mana of any one color and target player gains X life, where X is the number of cards in your hand.");
        let mut g=game("Sorcery", &text, None);
        for _ in 0..2 { g.put(0, BEARS, Zone::Hand); }
        if creature { g.put(0, BEARS, Zone::Battlefield); }
        cast(&mut g, vec![vec![Target::Player(1)]]);
        g.act(0, Move::Choose(Answer::Mana(vec![ir::mana::ManaSymbol::Color(ir::Color::Green); 2]))).unwrap();
        cite!(["107.3c", "608.2d", "608.2h"]; case: lead.to_owned();
            assert_eq!((g.players[0].pool.colors[4], g.players[1].life), (2, 22)));
    }
}

#[test]
fn r608_2d_initialized_x_survives_an_intervening_optional_clause_marker() {
    let mut g=game("Sorcery", "You draw X cards, then you may choose a color and discard a card, then target player gains X life, where X is the number of cards in your hand.", None);
    for _ in 0..2 { g.put(0, BEARS, Zone::Hand); }
    for _ in 0..6 { g.put(0, BEARS, Zone::Library); }
    cast(&mut g, vec![vec![Target::Player(1)]]);
    g.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["107.3c", "608.2c", "608.2d"]; assert_eq!((g.players[0].hand.len(), g.players[1].life), (4, 22)));
}
