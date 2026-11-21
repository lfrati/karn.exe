//! Cards exiled face down (406.3): every player knows the card is there, but only a player who
//! was allowed to look at it may continue to look, even after the grant ends, until it leaves
//! exile (406.3). While it is exiled face down it has no characteristics (406.3a). It is not a
//! face-down permanent (708): it is no 2/2 and nothing that affects creatures affects it.
use crate::support::cite;
use crate::support;
use game::snapshot::Snapshot;
use support::*;

type Text = (&'static str, &'static str, &'static str, &'static str, Option<(i32, i32)>);
/// Exiles the top card of its controller's library face down, which nobody looks at.
const VAULT: Text = ("Vault", "{0}", "Artifact", "{T}: Exile the top card of your library face down.\n{1}: Put all cards exiled with this artifact into their owner's hand.", None);
/// Its controller looks at two cards and exiles one of them face down.
const SEER: Text = ("Seer", "{0}", "Creature — Human", "When this creature enters, look at the top two cards of your library. Exile one of them face down and put the rest on the bottom of your library in a random order.", Some((1, 1)));

/// Oracle cards by name, then test texts, each at its index among the definitions.
fn game(names: &[&str], texts: &[Text]) -> T {
    let mut defs: Vec<CardDef> = names.iter().map(|n| oracle(n)).collect();
    for (name, cost, types, text, pt) in texts {
        let d = CardDef::new(name, cost, types, text, *pt).unwrap();
        game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
        defs.push(d);
    }
    T::new(Game::new(defs, 2, 20))
}
fn put(t: &mut T, p: usize, name: &str, zone: Zone) -> ObjId {
    let index = t.g.defs.iter().position(|d| d.name() == name).unwrap_or_else(|| panic!("no {name}"));
    t.g.put(p, index, zone)
}
fn view(g: &Game, viewer: Option<usize>, o: ObjId) -> Option<game::snapshot::Object> {
    Snapshot::of(g, viewer).objects.into_iter().find(|x| x.id == o)
}
/// The card a viewer's snapshot says a face-down exiled card is, if it says.
fn card_seen(g: &Game, viewer: Option<usize>, o: ObjId) -> Option<String> {
    view(g, viewer, o).and_then(|x| x.hidden).and_then(|h| h.card).map(|c| c.name)
}
/// The exiled card that is new since `before`.
fn exiled_since(g: &Game, before: &[ObjId]) -> ObjId {
    let new: Vec<ObjId> = g.exile.iter().copied().filter(|o| !before.contains(o)).collect();
    assert_eq!(new.len(), 1, "one card was exiled");
    new[0]
}
/// Player 0 activates Vault's first ability and it resolves: the exiled card.
fn vault_exiles(t: &mut T) -> ObjId {
    let before = t.g.exile.clone();
    t.activate(0, "Vault", 0, vec![], crate::support::mana_pool("")).unwrap();
    t.resolve();
    exiled_since(&t.g, &before)
}

#[test]
fn r406_3_exiled_face_down_unseen_is_perceived_by_all_and_seen_by_none() {
    let mut t = game(&["Grizzly Bears"], &[VAULT]);
    put(&mut t, 0, "Vault", Zone::Battlefield);
    put(&mut t, 0, "Grizzly Bears", Zone::Library);
    let card = vault_exiles(&mut t);
    assert_eq!(t.g.hidden(card).map(|h| h.lookers.clone()), Some(vec![]));
    for p in 0..2 {
        cite!(["406.3"]; assert!(t.g.perceives(p, card), "everyone knows a card is there (406.3)"));
        cite!(["406.3"]; assert!(!t.g.sees(p, card), "nobody looked at it, so nobody may look at it (406.3)"));
        let o = view(&t.g, Some(p), card).expect("in every snapshot");
        assert_eq!(o.name, "");
        assert!(o.types.is_empty());
        assert_eq!(o.hidden.as_ref().map(|h| (h.lookers.clone(), h.card.clone())), Some((vec![], None)));
    }
    assert_eq!(card_seen(&t.g, None, card).as_deref(), Some("Grizzly Bears"), "the harness sees all");
}

#[test]
fn r406_3a_a_card_exiled_face_down_has_no_characteristics() {
    let mut t = game(&["Grizzly Bears"], &[VAULT]);
    put(&mut t, 0, "Vault", Zone::Battlefield);
    put(&mut t, 0, "Grizzly Bears", Zone::Library);
    let card = vault_exiles(&mut t);
    let seen = t.g.seen(card, None);
    cite!(["406.3a"]; assert!(seen.types.is_empty() && seen.subtypes.is_empty() && seen.pt.is_none()));
    assert_eq!(t.g.obj(card).chars.name, "");
    assert!(seen.colors.is_empty() && seen.supertypes.is_empty());
    cite!(["406.3a"]; assert!(t.g.def(card).is_some_and(|d| d.cost().is_none() && d.abilities().is_empty()), "no mana cost or text"));
    assert!(t.g.is(Target::Object(card), ir::Sort::Card), "it is still a card");
    cite!(["406.3a"]; assert!(!t.g.is(Target::Object(card), ir::Sort::Permanent) && t.g.power(card).is_none(), "not a face-down 2/2 (708.2)"));
    assert_eq!(t.g.shown_name(card), "Grizzly Bears", "what it is when shown");
}

#[test]
fn r406_3_the_player_who_looked_keeps_looking_until_it_leaves_exile() {
    let mut t = game(&["Grizzly Bears", "Llanowar Elves"], &[SEER]);
    put(&mut t, 0, "Llanowar Elves", Zone::Library);
    put(&mut t, 0, "Grizzly Bears", Zone::Library);
    let before = t.g.exile.clone();
    put(&mut t, 0, "Seer", Zone::Hand);
    t.cast(0, "Seer", vec![], crate::support::mana_pool("")).unwrap();
    // Seer's trigger: player 0 looks at the top two and chooses one to exile face down
    let Boundary::Question(p, Asked::Objects { .. }) = t.settle() else { panic!("which card to exile") };
    assert_eq!(p, 0);
    t.choose(0, &["Grizzly Bears"]).unwrap();
    t.resolve();
    let card = exiled_since(&t.g, &before);
    cite!(["406.3"]; assert_eq!(t.g.hidden(card).map(|h| h.lookers.clone()), Some(vec![0])));
    cite!(["406.3"]; assert!(t.g.sees(0, card) && !t.g.sees(1, card)));
    assert!(t.g.perceives(1, card));
    assert_eq!(card_seen(&t.g, Some(0), card).as_deref(), Some("Grizzly Bears"), "shown with no decision to look");
    assert_eq!(card_seen(&t.g, Some(1), card), None);
    assert_eq!(view(&t.g, Some(1), card).unwrap().hidden.unwrap().lookers, vec![0], "who may look is public");
    // the other card went to the bottom of the library, unseen by the opponent
    assert_eq!(t.g.players[0].library.len(), 1);
}

/// Exiles the top card face down and lets its controller play it this turn.
const IMPULSE: Text = ("Impulse Vault", "{0}", "Artifact", "{T}: Exile the top card of your library face down. You may play that card this turn.", None);
/// Returns the cards it exiled to their owner's graveyard.
const GRAVE_VAULT: Text = ("Grave Vault", "{0}", "Artifact", "{T}: Exile the top card of your library face down.\n{1}: Put all cards exiled with this artifact into their owner's graveyard.", None);

#[test]
fn r406_3_leaving_exile_it_is_a_new_face_up_object_where_it_goes() {
    // to the hand: its owner's alone (402.3); every player saw a card leave exile
    let mut t = game(&["Grizzly Bears"], &[VAULT]);
    put(&mut t, 0, "Vault", Zone::Battlefield);
    put(&mut t, 0, "Grizzly Bears", Zone::Library);
    let card = vault_exiles(&mut t);
    t.mana(0, "{1}");
    t.activate(0, "Vault", 1, vec![], crate::support::mana_pool("C")).unwrap();
    t.resolve();
    let now = t.find_in(0, Zone::Hand, "Grizzly Bears");
    cite!(["400.7"]; assert!(t.g.hidden(now).is_none() && t.g.obj(now).chars.name == "Grizzly Bears"));
    cite!(["402.3"]; assert!(t.g.sees(0, now) && !t.g.sees(1, now)));
    assert!(t.g.perceives(1, card), "the face-down card that left is still known to have been there");
    assert!(!t.g.sees(1, card), "and leaving to a hand does not show it");
    // to the graveyard: public (404.2)
    let mut t = game(&["Grizzly Bears"], &[GRAVE_VAULT]);
    put(&mut t, 0, "Grave Vault", Zone::Battlefield);
    put(&mut t, 0, "Grizzly Bears", Zone::Library);
    let before = t.g.exile.clone();
    t.activate(0, "Grave Vault", 0, vec![], crate::support::mana_pool("")).unwrap();
    t.resolve();
    exiled_since(&t.g, &before);
    t.mana(0, "{1}");
    t.activate(0, "Grave Vault", 1, vec![], crate::support::mana_pool("C")).unwrap();
    t.resolve();
    let now = t.find_in(0, Zone::Graveyard, "Grizzly Bears");
    cite!(["404.2"]; assert!(t.g.sees(1, now) && t.g.hidden(now).is_none()));
}

#[test]
fn r406_3a_a_card_exiled_face_down_is_not_played_while_turning_it_up_is_unsupported() {
    let mut t = game(&["Grizzly Bears"], &[IMPULSE]);
    put(&mut t, 0, "Impulse Vault", Zone::Battlefield);
    put(&mut t, 0, "Grizzly Bears", Zone::Library);
    let before = t.g.exile.clone();
    t.activate(0, "Impulse Vault", 0, vec![], crate::support::mana_pool("")).unwrap();
    t.resolve();
    let card = exiled_since(&t.g, &before);
    t.mana(0, "{1}{G}");
    cite!(["406.3a"]; refused("playing a card exiled face down is not supported"): t.g.act(0, paid("CG", Move::Cast { card, targets: vec![] })));
    cite!(["406.3a"]; assert!(t.g.exile.contains(&card) && t.g.stack.is_empty() && t.g.hidden(card).is_some()));
    // Uncited (contract): refusal consumes no supplied mana.
    assert_eq!(t.g.players[0].pool, mana_pool("CG"));
}

// ------------------------------------------------------------------------- hideaway (702.75)

/// Watcher for Tomorrow enters for player 0 over these library cards, top first, and its
/// hideaway trigger exiles `chosen` face down: the permanent and the exiled card.
fn watcher(t: &mut T, library: &[&str], chosen: &str) -> (ObjId, ObjId) {
    for name in library.iter().rev() { put(t, 0, name, Zone::Library); }
    put(t, 0, "Watcher for Tomorrow", Zone::Hand);
    t.mana(0, "{1}{U}");
    t.cast(0, "Watcher for Tomorrow", vec![], crate::support::mana_pool("CU")).unwrap();
    let before = t.g.exile.clone();
    let Boundary::Question(0, Asked::Objects { among, .. }) = t.settle() else { panic!("which card to hide") };
    assert_eq!(among.len(), 4, "hideaway 4 looks at four cards");
    let pick = among.iter().copied().find(|o| t.g.obj(*o).chars.name == chosen).unwrap();
    t.answer(0, Answer::Objects(vec![pick])).unwrap();
    t.resolve();
    (t.find_in_battlefield("Watcher for Tomorrow"), exiled_since(&t.g, &before))
}
/// On to player 1's precombat main phase, with a card for each player to draw.
fn to_their_turn(t: &mut T) {
    put(t, 1, "Forest", Zone::Library);
    t.to_no_combat(1, Part::PrecombatMain);
}
const LIBRARY: [&str; 5] = ["Grizzly Bears", "Llanowar Elves", "Forest", "Island", "Shivan Dragon"];

#[test]
fn r702_75a_watcher_for_tomorrow_hides_one_of_four_and_returns_it_when_it_leaves() {
    let mut t = game(&["Watcher for Tomorrow", "Grizzly Bears", "Llanowar Elves", "Forest", "Island", "Shivan Dragon", "Unsummon"], &[]);
    let (watcher, card) = watcher(&mut t, &LIBRARY, "Llanowar Elves");
    // Uncited (card text): Watcher for Tomorrow's "This creature enters tapped."
    assert!(t.g.obj(watcher).tapped, "it enters tapped");
    cite!(["702.75a"]; assert_eq!(t.g.hidden(card).map(|h| (h.lookers.clone(), h.watcher)), Some((vec![0], Some(watcher)))));
    cite!(["406.3"]; assert!(t.g.sees(0, card) && !t.g.sees(1, card)));
    assert_eq!(view(&t.g, Some(1), card).unwrap().name, "");
    // the rest went to the bottom; Shivan Dragon, never looked at, is now on top
    assert_eq!(t.g.players[0].library.len(), 4);
    cite!(["702.75a"]; assert_eq!(t.g.obj(*t.g.players[0].library.last().unwrap()).chars.name, "Shivan Dragon", "the fifth card is on top"));
    // it leaves the battlefield: the exiled card goes to its owner's hand
    put(&mut t, 1, "Unsummon", Zone::Hand);
    while t.g.priority != 1 { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.mana(1, "{U}");
    t.cast(1, "Unsummon", vec![on(0, Zone::Battlefield, "Watcher for Tomorrow")], crate::support::mana_pool("U")).unwrap();
    t.resolve();
    let hand: Vec<String> = t.g.players[0].hand.iter().map(|o| t.g.obj(*o).chars.name.clone()).collect();
    // Uncited (card text): Watcher's "put the exiled card into its owner's hand" and Unsummon's "Return target creature to its owner's hand."
    assert!(hand.contains(&"Llanowar Elves".to_owned()) && hand.contains(&"Watcher for Tomorrow".to_owned()), "{hand:?}");
    // Uncited (card text): "put the exiled card into its owner's hand" takes the only exiled card
    assert!(t.g.exile.is_empty());
}

#[test]
fn r702_75a_whoever_controls_the_hideaway_permanent_may_look_at_the_card() {
    let mut t = game(&["Watcher for Tomorrow", "Grizzly Bears", "Llanowar Elves", "Forest", "Island", "Shivan Dragon", "Act of Treason", "Murder"], &[]);
    let (watcher, card) = watcher(&mut t, &LIBRARY, "Grizzly Bears");
    // player 1 gains control of it: they may look, and player 0 still may (406.3)
    put(&mut t, 1, "Act of Treason", Zone::Hand);
    to_their_turn(&mut t);
    t.mana(1, "{2}{R}");
    t.cast(1, "Act of Treason", vec![on(0, Zone::Battlefield, "Watcher for Tomorrow")], crate::support::mana_pool("CCR")).unwrap();
    t.resolve();
    assert_eq!(t.g.controller(watcher), 1);
    cite!(["702.75a", "406.3"]; assert!(t.g.sees(1, card) && t.g.sees(0, card)));
    assert_eq!(card_seen(&t.g, Some(1), card).as_deref(), Some("Grizzly Bears"));
    assert_eq!(view(&t.g, Some(1), card).unwrap().hidden.unwrap().lookers, vec![0, 1], "permission is retained without requiring a look");
}

/// Rules: 406.3, 702.75a, 514.2
/// Given: Watcher hides a card; one cloned branch grants player 1 temporary control.
/// Expect: only that branch retains permission after control returns, without a look.
#[test]
fn r702_75a_permission_survives_control_return_and_clones_stay_independent() {
    let mut original = game(&["Watcher for Tomorrow", "Grizzly Bears", "Llanowar Elves", "Forest", "Island", "Shivan Dragon", "Act of Treason"], &[VAULT]);
    let (watcher, card) = watcher(&mut original, &LIBRARY, "Grizzly Bears");
    let mut changed = T::new(original.g.clone());
    put(&mut changed, 1, "Act of Treason", Zone::Hand);
    to_their_turn(&mut changed);
    changed.mana(1, "{2}{R}");
    changed.cast(1, "Act of Treason", vec![on(0, Zone::Battlefield, "Watcher for Tomorrow")], mana_pool("CCR")).unwrap();
    changed.resolve();
    // No sees/snapshot/LOOK call while player 1 controls Watcher.
    changed.to_no_combat(0, Part::PrecombatMain);
    cite!(["514.2"]; assert_eq!(changed.g.controller(watcher), 0));
    cite!(["406.3", "702.75a"]; assert_eq!(changed.g.hidden(card).unwrap().lookers, vec![0, 1]));
    cite!(["406.3"]; assert_eq!(card_seen(&changed.g, Some(1), card).as_deref(), Some("Grizzly Bears")));
    cite!(["406.3"]; assert_eq!(card_seen(&original.g, Some(1), card), None));
    cite!(["406.3a"]; assert!(view(&changed.g, Some(1), card).unwrap().name.is_empty()));
    let mut native = game::simulation::Simulation::new(changed.g.clone());
    let seat = native.snapshot(Some(1)).unwrap();
    let shown = seat.objects.iter().find(|o| o.id == card).unwrap();
    cite!(["406.3", "406.3a"]; assert!(shown.name.is_empty() && shown.hidden.as_ref().unwrap().card.as_ref().unwrap().name == "Grizzly Bears"));
    // Direct zone-operation boundary: the physical card's new incarnation has no grants.
    changed.g.move_object(card, Zone::Library);
    let library_card = *changed.g.players[0].library.last().unwrap();
    cite!(["400.7"]; assert!(library_card != card && changed.g.hidden(library_card).is_none()));
    put(&mut changed, 0, "Vault", Zone::Battlefield);
    let exiled_again = vault_exiles(&mut changed);
    cite!(["400.7", "406.3"]; assert!(exiled_again != card && changed.g.hidden(exiled_again).unwrap().lookers.is_empty()));
    for player in 0..2 {
        cite!(["406.3"]; assert_eq!(card_seen(&changed.g, Some(player), exiled_again), None));
    }
}

#[test]
fn r702_75a_permission_survives_the_source_leaving_without_an_actual_look() {
    let mut t = game(&["Watcher for Tomorrow", "Grizzly Bears", "Llanowar Elves", "Forest", "Island", "Shivan Dragon", "Act of Treason", "Murder"], &[]);
    let (watcher, card) = watcher(&mut t, &LIBRARY, "Grizzly Bears");
    put(&mut t, 1, "Act of Treason", Zone::Hand);
    put(&mut t, 1, "Murder", Zone::Hand);
    to_their_turn(&mut t);
    t.mana(1, "{2}{R}");
    t.cast(1, "Act of Treason", vec![on(0, Zone::Battlefield, "Watcher for Tomorrow")], crate::support::mana_pool("CCR")).unwrap();
    t.resolve();
    // Do not observe the card after the control change: permission itself persists.
    t.mana(1, "{1}{B}{B}");
    t.cast(1, "Murder", vec![on(1, Zone::Battlefield, "Watcher for Tomorrow")], crate::support::mana_pool("CBB")).unwrap();
    // its leaves trigger returns the card; before it resolves nobody controls the permanent,
    // and both players retain permission (406.3; Watcher ruling 2022-04-29).
    while t.g.battlefield.contains(&watcher) { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    assert!(!t.g.stack.is_empty(), "the leaves trigger waits");
    cite!(["406.3"]; assert!(t.g.sees(0, card) && t.g.sees(1, card)));
    cite!(["406.3"]; assert_eq!(card_seen(&t.g, Some(1), card).as_deref(), Some("Grizzly Bears")));
    let Boundary::Priority(_) = t.settle() else { panic!() };
    let hand: Vec<String> = t.g.players[0].hand.iter().map(|o| t.g.obj(*o).chars.name.clone()).collect();
    // Uncited (card text): Watcher's "put the exiled card into its owner's hand"
    assert!(hand.contains(&"Grizzly Bears".to_owned()), "returned to its owner's hand, not its controller's: {hand:?}");
    let returned = t.find_in(0, Zone::Hand, "Grizzly Bears");
    cite!(["400.7", "402.3"]; assert!(returned != card && t.g.hidden(returned).is_none() && !t.g.sees(1, returned)));
    cite!(["402.3"]; assert!(view(&t.g, Some(1), returned).is_none()));
}

#[test]
fn r702_75a_a_hideaway_card_is_not_a_creature_or_affected_by_creature_effects() {
    let mut t = game(&["Watcher for Tomorrow", "Grizzly Bears", "Llanowar Elves", "Forest", "Island", "Shivan Dragon"], &[]);
    let (_, card) = watcher(&mut t, &LIBRARY, "Llanowar Elves");
    let seen = t.g.seen(card, None);
    cite!(["406.3a"]; assert!(seen.types.is_empty() && seen.subtypes.is_empty() && seen.colors.is_empty() && seen.pt.is_none()));
    cite!(["406.3a"]; assert!(t.g.abilities(card).is_empty(), "no text, so no mana ability (406.3a)"));
}
