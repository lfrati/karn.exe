//! "Exile the top three cards of your library. Until end of turn, you may play those cards."
//! (Act on Impulse); "… Until the end of your next turn, you may play that card." (Inspired
//! Tinkering, Chase Stein); "… Until the beginning of your next upkeep, …" (Elkin Bottle);
//! "… Until your next end step, …". A resolved permission lasts as long as it says (611.2a):
//! it is about each exiled card as it is in exile (400.7), is its controller's alone, and
//! playing a card under it follows every other rule of playing it (305.2, 601.3, 117.1a).
use crate::support::{cite, paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const IMPULSE: usize = 0;
const MOUNTAIN: usize = 1;
const BEAR: usize = 2;
const BOLT: usize = 3;
const NEXT_TURN: usize = 4;
const UPKEEP: usize = 5;
const END_STEP: usize = 6;
const CHOSEN: usize = 7;
const ADVENTURER: usize = 8;
const SKIP: usize = 9;
const ABBOT: usize = 10;
const TINKERING: usize = 11;
const MIXED: usize = 12;

fn game() -> Game { game_with(6) }
/// A game with this many Mountains in each library, under what is put on top.
fn game_with(fill: usize) -> Game {
    let card = |name: &str, cost: &str, ty: &str, text: &str| CardDef::new(name, cost, ty, text, None).unwrap();
    let defs = vec![
        card("Impulse", "{R}", "Sorcery", "Exile the top three cards of your library. Until end of turn, you may play those cards."),
        card("Mountain", "", "Basic Land — Mountain", ""),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        card("Lightning Bolt", "{R}", "Instant", "Lightning Bolt deals 3 damage to any target."),
        card("Tinkering", "{R}", "Instant", "Exile the top card of your library. Until the end of your next turn, you may play that card."),
        card("Bottle", "{R}", "Instant", "Exile the top card of your library. Until the beginning of your next upkeep, you may play that card."),
        card("Glimpse", "{R}", "Sorcery", "Exile the top card of your library. Until your next end step, you may play that card."),
        card("Giant", "{R}", "Sorcery", "Exile the top two cards of your library. Choose one of them. Until end of turn, you may play that card."),
        CardDef::adventurer(CardDef::new("Adventurer", "{6}{G}", "Creature — Giant", "", Some((7, 7))).unwrap(),
            card("Journey", "{G}", "Sorcery — Adventure", "You gain 2 life.")).unwrap(),
        card("Stall", "{R}", "Sorcery", "Target player skips their next upkeep step."),
        CardDef::new("Abbot", "{R}", "Creature — Human Monk", "When this creature enters, exile the top card of your library. Until end of turn, you may play that card.", Some((2, 1))).unwrap(),
        card("Inspired Tinkering", "{R}", "Sorcery", "Exile the top three cards of your library. Until the end of your next turn, you may play those cards. Create three Treasure tokens."),
        card("Mixed", "{R}", "Sorcery", "Exile the top card of your library. Until end of turn, you may play that card. Until the end of your next turn, you may cast that card."),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 {
        for _ in 0..fill { g.put(p, MOUNTAIN, Zone::Library); }
    }
    g
}
fn pass_all(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
/// This player casts this card and it resolves.
fn resolve(g: &mut Game, player: usize, effect: usize) {
    assert_eq!(g.priority, player);
    let card = g.put(player, effect, Zone::Hand);
    g.players[player].pool.colors[3] += 1;
    g.act(player, paid("R", Move::Cast { card, targets: vec![] })).unwrap();
    pass_all(g);
    assert!(g.decision.is_some() || g.stack.is_empty(), "it resolved");
}
fn exiled(g: &Game, card: usize) -> ObjId {
    *g.exile.iter().find(|o| g.obj(**o).card == Some(card)).expect("exiled")
}
/// Pass until this player has priority in this part of this turn.
fn advance(g: &mut Game, turn: u32, part: Part, player: usize) {
    for _ in 0..200 {
        if (g.turn, g.part, g.priority) == (turn, part, player) && g.stack.is_empty() { return; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("never reached turn {turn} {part:?} with p{player} priority: at {} {:?} p{}", g.turn, g.part, g.priority);
}
/// Whether player 0 is offered to play this card as a land, and to cast it, with {R} to cast
fn bolt(g: &mut Game, player: usize, card: ObjId) -> Result<(), String> {
    g.players[player].pool.colors[3] += 1;
    g.act(player, paid("R", Move::Cast { card, targets: vec![vec![Target::Player(1 - player)]] })).map(|_| ())
}

#[test]
fn r611_2a_every_card_exiled_may_be_played_until_end_of_turn() {
    let mut g = game();
    for top in [BOLT, BEAR, MOUNTAIN] { g.put(0, top, Zone::Library); }
    advance(&mut g, 1, Part::PrecombatMain, 0);
    resolve(&mut g, 0, IMPULSE);
    let (land, bear, bolt_card) = (exiled(&g, MOUNTAIN), exiled(&g, BEAR), exiled(&g, BOLT));
    assert_eq!(g.exile.len(), 3);
    cite!(["611.2a"]; assert_eq!((g.clone().act(0, Move::PlayLand(land)).is_ok(), bolt(&mut g.clone(), 0, land).is_ok()), (true, false)));
    crate::support::refused(g.act(1, Move::PlayLand(land)), "that player does not have priority");
    // with priority, the other player is still not the one the effect lets play it
    let mut o = g.clone();
    o.act(0, Move::Pass).unwrap();
    cite!(["305.1"]; refused("an effect lets another player play that card, not you (305.1)"): o.act(1, Move::PlayLand(land)));
    g.act(0, Move::PlayLand(land)).unwrap();
    cite!(["305.2"]; assert!(g.players[0].land_played, "it is the land played this turn (305.2)"));
    g.players[0].pool.colors[4] = 2;
    g.act(0, paid("GG", Move::Cast { card: bear, targets: vec![] })).unwrap();
    pass_all(&mut g);
    cite!(["611.2a"]; assert!(g.battlefield.iter().any(|o| g.obj(*o).card == Some(BEAR))));
    // the instant is still permitted, but not after the turn: an opponent's turn is too late
    let mut later = g.clone();
    advance(&mut later, 2, Part::Upkeep, 0);
    cite!(["611.2a"]; assert_eq!((later.clone().act(0, Move::PlayLand(bolt_card)).is_ok(), bolt(&mut later.clone(), 0, bolt_card).is_ok()), (false, false)));
    cite!(["611.2a"]; refused("that card is not in your hand: it is in exile, and no rule or effect lets you cast it from there (601.3)"): bolt(&mut later, 0, bolt_card));
    bolt(&mut g, 0, bolt_card).unwrap();
    pass_all(&mut g);
    assert_eq!(g.players[1].life, 17);
}

#[test]
fn r611_2a_a_second_land_waits_for_a_land_play() {
    let mut g = game();
    for top in [MOUNTAIN, MOUNTAIN, MOUNTAIN] { g.put(0, top, Zone::Library); }
    advance(&mut g, 1, Part::PrecombatMain, 0);
    resolve(&mut g, 0, IMPULSE);
    let lands: Vec<ObjId> = g.exile.clone();
    assert_eq!(lands.len(), 3);
    g.act(0, Move::PlayLand(lands[0])).unwrap();
    cite!(["305.2"]; refused("you have played a land this turn (305.2)"): g.act(0, Move::PlayLand(lands[1])));
    cite!(["305.2"]; assert_eq!((g.clone().act(0, Move::PlayLand(lands[1])).is_ok(), bolt(&mut g.clone(), 0, lands[1]).is_ok()), (false, false)));
}

#[test]
fn r611_2a_until_the_end_of_your_next_turn_spans_the_opponents_turn_and_your_next() {
    // put in force on its controller's own turn: that turn, the opponent's, and their next
    let mut g = game();
    g.put(0, BOLT, Zone::Library);
    advance(&mut g, 1, Part::PrecombatMain, 0);
    resolve(&mut g, 0, NEXT_TURN);
    let card = exiled(&g, BOLT);
    advance(&mut g, 2, Part::Upkeep, 0);
    cite!(["611.2a"]; assert_eq!((g.clone().act(0, Move::PlayLand(card)).is_ok(), bolt(&mut g.clone(), 0, card).is_ok()), (false, true), "on the opponent's turn too"));
    let at_end = {
        let mut own_next = g.clone();
        advance(&mut own_next, 3, Part::End, 0);
        cite!(["611.2a"]; assert_eq!((own_next.clone().act(0, Move::PlayLand(card)).is_ok(), bolt(&mut own_next.clone(), 0, card).is_ok()), (false, true), "through the end of their next turn"));
        own_next
    };
    let mut after = at_end;
    advance(&mut after, 4, Part::Upkeep, 0);
    assert!(after.exile.contains(&card));
    cite!(["611.2a"]; assert_eq!((after.clone().act(0, Move::PlayLand(card)).is_ok(), bolt(&mut after.clone(), 0, card).is_ok()), (false, false), "not after it"));
    cite!(["611.2a"]; refused("that card is not in your hand: it is in exile, and no rule or effect lets you cast it from there (601.3)"): bolt(&mut after, 0, card));

    // put in force on the opponent's turn: the next turn is the controller's following one
    let mut g = game();
    g.put(0, BOLT, Zone::Library);
    advance(&mut g, 2, Part::Upkeep, 0);
    resolve(&mut g, 0, NEXT_TURN);
    let card = exiled(&g, BOLT);
    let mut own = g.clone();
    advance(&mut own, 3, Part::End, 0);
    cite!(["611.2a"]; assert_eq!((own.clone().act(0, Move::PlayLand(card)).is_ok(), bolt(&mut own.clone(), 0, card).is_ok()), (false, true)));
    advance(&mut own, 4, Part::Upkeep, 0);
    cite!(["611.2a"]; assert_eq!((own.clone().act(0, Move::PlayLand(card)).is_ok(), bolt(&mut own.clone(), 0, card).is_ok()), (false, false)));
    // and at once, on the turn it was put in force in
    advance(&mut g, 2, Part::Upkeep, 0);
    cite!(["611.2a"]; ok: bolt(&mut g, 0, card));
}

#[test]
fn r611_2a_until_your_next_upkeep_or_end_step_ends_as_that_step_begins() {
    // the next upkeep is of a later turn: the permission spans the opponent's turn
    let mut g = game();
    g.put(0, BOLT, Zone::Library);
    advance(&mut g, 1, Part::PrecombatMain, 0);
    resolve(&mut g, 0, UPKEEP);
    let card = exiled(&g, BOLT);
    let mut next = g.clone();
    advance(&mut next, 2, Part::End, 0);
    cite!(["611.2a"]; assert_eq!((next.clone().act(0, Move::PlayLand(card)).is_ok(), bolt(&mut next.clone(), 0, card).is_ok()), (false, true)));
    advance(&mut next, 3, Part::Upkeep, 0);
    cite!(["611.2a"]; assert_eq!((next.clone().act(0, Move::PlayLand(card)).is_ok(), bolt(&mut next.clone(), 0, card).is_ok()), (false, false), "over as their upkeep begins"));
    // the opponent's upkeep is not theirs
    let mut theirs = g.clone();
    advance(&mut theirs, 2, Part::Upkeep, 0);
    cite!(["611.2a"]; assert_eq!((theirs.clone().act(0, Move::PlayLand(card)).is_ok(), bolt(&mut theirs.clone(), 0, card).is_ok()), (false, true)));

    // "your next end step" put in force in a main phase is that same turn's
    let mut g = game();
    g.put(0, BOLT, Zone::Library);
    advance(&mut g, 1, Part::PrecombatMain, 0);
    resolve(&mut g, 0, END_STEP);
    let card = exiled(&g, BOLT);
    let mut combat = g.clone();
    advance(&mut combat, 1, Part::BeginningOfCombat, 0);
    cite!(["611.2a"]; assert_eq!((combat.clone().act(0, Move::PlayLand(card)).is_ok(), bolt(&mut combat.clone(), 0, card).is_ok()), (false, true)));
    advance(&mut g, 1, Part::End, 0);
    cite!(["611.2a"]; assert_eq!((g.clone().act(0, Move::PlayLand(card)).is_ok(), bolt(&mut g.clone(), 0, card).is_ok()), (false, false), "over as their end step begins"));
}

#[test]
fn r611_2a_only_the_chosen_card_may_be_played() {
    let mut g = game();
    for top in [BEAR, BOLT] { g.put(0, top, Zone::Library); }
    advance(&mut g, 1, Part::PrecombatMain, 0);
    resolve(&mut g, 0, CHOSEN);
    let (bolt_card, bear) = (exiled(&g, BOLT), exiled(&g, BEAR));
    let Some(Decision::Choose { player: 0, choice: Choice::Objects { among, .. }, .. }) = &g.decision else { panic!("{:?}", g.decision) };
    let mut among = among.clone();
    among.sort();
    let mut both = vec![bolt_card, bear];
    both.sort();
    assert_eq!(among, both);
    g.act(0, Move::Choose(Answer::Objects(vec![bolt_card]))).unwrap();
    cite!(["601.3"]; assert_eq!((g.clone().act(0, Move::PlayLand(bolt_card)).is_ok(), bolt(&mut g.clone(), 0, bolt_card).is_ok()), (false, true)));
    g.players[0].pool.colors[4] = 2;
    cite!(["601.3"]; assert_eq!((g.clone().act(0, Move::PlayLand(bear)).is_ok(), g.clone().act(0, paid("GG", Move::Cast { card: bear, targets: vec![] })).is_ok()), (false, false), "the other card is only exiled"));
    cite!(["601.3"]; refused("that card is not in your hand: it is in exile, and no rule or effect lets you cast it from there (601.3)"): g.act(0, paid("GG", Move::Cast { card: bear, targets: vec![] })));
}

#[test]
fn r611_2a_the_permission_does_not_follow_a_card_that_leaves_exile_or_was_never_exiled() {
    let mut g = game();
    g.put(0, BOLT, Zone::Library);
    advance(&mut g, 1, Part::PrecombatMain, 0);
    resolve(&mut g, 0, NEXT_TURN);
    let card = exiled(&g, BOLT);
    // to the graveyard and back to exile: a new object (400.7)
    g.move_object(card, Zone::Graveyard);
    let back = *g.players[0].graveyard.iter().find(|o| g.obj(**o).card == Some(BOLT)).unwrap();
    g.exile(back);
    let again = exiled(&g, BOLT);
    cite!(["400.7"]; assert_ne!(again, card));
    cite!(["400.7"]; assert_eq!((g.clone().act(0, Move::PlayLand(again)).is_ok(), bolt(&mut g.clone(), 0, again).is_ok()), (false, false)));
    cite!(["400.7"]; refused("that card is not in your hand: it is in exile, and no rule or effect lets you cast it from there (601.3)"): bolt(&mut g, 0, again));

    // an empty library: nothing is exiled, so nothing may be played
    let mut g = game_with(0);
    g.put(1, MOUNTAIN, Zone::Library);
    advance(&mut g, 1, Part::PrecombatMain, 0);
    assert!(g.players[0].library.is_empty());
    resolve(&mut g, 0, IMPULSE);
    assert!(g.exile.is_empty());

}

#[test]
fn r611_2a_permissions_with_other_deadlines_or_for_other_players_are_refused() {
    for (text,reason) in [
        ("Exile the top card of your library. Until end of combat, you may play that card.","program: a permission to play until another deadline"),
        ("Exile the top card of your library. Until your next turn, you may play that card.","program: a permission to play until another deadline"),
        ("Exile the top card of your library. Until end of turn, target opponent may play that card.","program: permission to play given to another player"),
        // a card in a graveyard, which the permission, kept only for exiled cards, would not reach
        ("You may cast target creature card from your graveyard this turn.","program: a permission to play a targeted card where it is"),
    ] {
        let d = CardDef::new("Refused", "{R}", "Sorcery", text, None).unwrap();
        crate::support::refused(game::play::check(&d).map_err(|e|e.0),reason);
    }
}

#[test]
fn r611_2a_a_skipped_upkeep_does_not_end_until_your_next_upkeep() {
    // 614.10: a skipped step never begins, so the permission waits for an upkeep that does
    let mut g = game();
    g.put(0, BOLT, Zone::Library);
    advance(&mut g, 1, Part::PrecombatMain, 0);
    resolve(&mut g, 0, UPKEEP);
    let card = exiled(&g, BOLT);
    let stall = g.put(0, SKIP, Zone::Hand);
    g.players[0].pool.colors[3] += 1;
    g.act(0, paid("R", Move::Cast { card: stall, targets: vec![vec![Target::Player(0)]] })).unwrap();
    pass_all(&mut g);
    advance(&mut g, 3, Part::Draw, 0);
    cite!(["614.10a"]; assert_eq!((g.clone().act(0, Move::PlayLand(card)).is_ok(), bolt(&mut g.clone(), 0, card).is_ok()), (false, true), "its upkeep was skipped"));
    advance(&mut g, 5, Part::Upkeep, 0);
    cite!(["611.2a"]; assert_eq!((g.clone().act(0, Move::PlayLand(card)).is_ok(), bolt(&mut g.clone(), 0, card).is_ok()), (false, false)));
}

#[test]
fn r611_2a_an_adventure_cast_under_it_gets_its_own_lasting_permission() {
    // 715.3d: the adventurer exiled as the Adventure resolves is a new object, with the
    // Adventure's permission, which the impulse deadline does not end
    let mut g = game();
    for top in [MOUNTAIN, MOUNTAIN, ADVENTURER] { g.put(0, top, Zone::Library); }
    advance(&mut g, 1, Part::PrecombatMain, 0);
    resolve(&mut g, 0, IMPULSE);
    let card = exiled(&g, ADVENTURER);
    assert_eq!(g.permission_until(card), Some("end_of_turn"));
    g.players[0].pool.colors[4] = 1;
    g.act(0, paid("G", Move::CastAs(game::card::CastAs::Adventure, Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    pass_all(&mut g);
    assert_eq!(g.players[0].life, 22);
    let again = exiled(&g, ADVENTURER);
    cite!(["400.7"]; assert_ne!(again, card));
    cite!(["715.3d"]; assert_eq!((g.permission(again), g.permission_until(again)), (Some((0, true)), None)));
    advance(&mut g, 3, Part::PrecombatMain, 0);
    g.players[0].pool.colors[4] = 7;
    cite!(["715.3d"]; ok: g.act(0, paid("GGGGGGG", Move::CastAs(game::card::CastAs::Whole, Box::new(Move::Cast { card: again, targets: vec![] })))));
}

#[test]
fn r611_2a_the_grant_outlives_its_source_and_its_other_instructions_still_happen() {
    // Abbot leaving the battlefield does not end what its resolved ability granted
    let mut g = game();
    g.put(0, BOLT, Zone::Library);
    advance(&mut g, 1, Part::PrecombatMain, 0);
    let abbot = g.put(0, ABBOT, Zone::Hand);
    g.players[0].pool.colors[3] = 1;
    g.act(0, paid("R", Move::Cast { card: abbot, targets: vec![] })).unwrap();
    pass_all(&mut g);
    pass_all(&mut g); // its enters trigger
    assert!(g.stack.is_empty());
    let card = exiled(&g, BOLT);
    let abbot = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(ABBOT)).unwrap();
    g.move_object(abbot, Zone::Graveyard);
    cite!(["611.2a"]; assert_eq!((g.clone().act(0, Move::PlayLand(card)).is_ok(), bolt(&mut g.clone(), 0, card).is_ok()), (false, true)));
    bolt(&mut g, 0, card).unwrap();

    // a library too short for three: what is exiled may be played, the Treasures are made
    let mut g = game_with(0);
    g.put(1, MOUNTAIN, Zone::Library);
    g.put(0, BOLT, Zone::Library);
    advance(&mut g, 1, Part::PrecombatMain, 0);
    resolve(&mut g, 0, TINKERING);
    let card = exiled(&g, BOLT);
    assert_eq!(g.permission_until(card), Some("end_of_their_next_turn"));
    cite!(["609.3"]; assert_eq!(g.battlefield.iter().filter(|o| g.obj(**o).token()).count(), 3));
}

#[test]
fn r611_2a_grants_with_different_rights_and_deadlines_are_each_reported() {
    // a land may be played from it this turn; it may be cast through the next: each grant
    // says its own deadline, and none is shown as the card's, since which ends last can
    // depend on what is still to happen (611.2a, 305.2, 614.10)
    let mut g = game();
    g.put(0, MOUNTAIN, Zone::Library);
    advance(&mut g, 1, Part::PrecombatMain, 0);
    resolve(&mut g, 0, MIXED);
    let land = *g.exile.first().unwrap();
    assert_eq!((g.permission(land), g.permission_until(land)), (Some((0, true)), None));
    assert_eq!(g.permission_grants(land), [(0, true, Some("end_of_turn")), (0, false, Some("end_of_their_next_turn"))]);
    advance(&mut g, 3, Part::PrecombatMain, 0);
    cite!(["611.2a"]; assert_eq!(g.permission(land), Some((0, false)), "only the right to cast it remains"));
    cite!(["611.2a"]; refused("an effect lets you cast that card, not play it as a land"): g.act(0, Move::PlayLand(land)));
}
