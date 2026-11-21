//! Waterbend (701.67).
//!
//! 701.67a: "“Waterbend [cost]” means “Pay [cost]. For each generic mana in that cost, you may
//! tap an untapped artifact or creature you control rather than pay that mana.”"
//!
//! 701.67b: "If a waterbend cost is part of the total cost to cast a spell or activate an
//! ability (usually because the waterbend cost itself is an additional cost), the alternate
//! method to pay for mana described in rule 701.67a may be used only to pay for the amount of
//! generic mana in the waterbend cost, even if the total cost to cast that spell or activate
//! that ability includes other generic mana components."
//!
//! 118.12a: "Some spells, activated abilities, and triggered abilities read, “[Do something]
//! unless [a player does something else].”" A waterbend cost there is paid as any other cost
//! is, its artifacts and creatures tapped by the player who pays it.
//!
//! The cards are read from their Oracle text, but for the {X} and the mixed costs.
use crate::support::cite;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const NAMES: [&str; 5] = ["Island", "Geyser Leaper", "North Pole Patrol", "Benevolent River Spirit", "Grizzly Bears"];
const ISLAND: usize = 0;
const LEAPER: usize = 1;
const PATROL: usize = 2;
const SPIRIT: usize = 3;
const BEARS: usize = 4;
const TRINKET: usize = 5;
const SPRING: usize = 6;
const SURGE: usize = 7;
const LESSON: usize = 8;
const UNAGI: usize = 9;
const SHOCK: usize = 10;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let mut defs: Vec<_> = NAMES.map(|n| CardDef::from_faces(&catalog[n]).unwrap()).to_vec();
    defs.push(CardDef::new("Trinket", "{1}", "Artifact", "{T}: You gain 1 life.", None).unwrap());
    defs.push(CardDef::new("Spring", "{U}", "Artifact", "Waterbend {X}: You gain X life.", None).unwrap());
    defs.push(CardDef::new("Surge", "{2}{U}", "Sorcery", "As an additional cost to cast this spell, waterbend {2}.\nDraw a card.", None).unwrap());
    defs.extend(["Waterbending Lesson", "The Unagi of Kyoshi Island", "Shock"].map(|n| CardDef::from_faces(&catalog[n]).unwrap()));
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn generic(g: &mut Game, n: u32) { g.players[0].pool.colors = [0; 5]; g.players[0].pool.colorless = n; }
fn activate(g: &mut Game, source: ObjId, ability: usize, targets: Vec<Vec<Target>>, tapped: &[ObjId], spending: &str) -> Result<(), String> {
    g.act(0, crate::support::paid(spending, Move::Activate { source, ability, targets, paying: vec![tapped.to_vec()] }))
}
fn resolve(g: &mut Game) { while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); } }


#[test]
fn r701_67a_each_tapped_artifact_or_creature_pays_one_generic_mana() {
    // Geyser Leaper: "Waterbend {4}: Draw a card, then discard a card."
    let mut g = game();
    let leaper = g.put(0, LEAPER, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let trinket = g.put(0, TRINKET, Zone::Battlefield);
    let index = 1 /* printed Flying, then Waterbend activation */;
    generic(&mut g, 1);
    // one mana and three of its own: the Leaper, the Bears, the Trinket
    let hand = g.players[0].hand.len();
    activate(&mut g, leaper, index, vec![], &[leaper, bears, trinket], "C").unwrap();
    cite!(["701.67a"]; assert!([leaper, bears, trinket].iter().all(|o| g.obj(*o).tapped)));
    assert_eq!(g.players[0].pool.total(), 0, "and the one mana paid");
    resolve(&mut g);
    if let Some(game::state::Decision::Discard { player, .. }) = g.decision.clone() { let c = g.players[player].hand[0]; g.act(player, Move::Discard(vec![c])).unwrap(); }
    assert_eq!(g.players[0].hand.len(), hand);
}

#[test]
fn r701_67a_mana_for_the_rest_and_nothing_tapped_at_all() {
    let mut g = game();
    let leaper = g.put(0, LEAPER, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let index = 1 /* printed Flying, then Waterbend activation */;
    generic(&mut g, 4);
    activate(&mut g, leaper, index, vec![], &[bears], "CCC").unwrap();
    cite!(["701.67a"]; assert_eq!((g.obj(bears).tapped, g.obj(leaper).tapped, g.players[0].pool.total()), (true, false, 1)));
    resolve(&mut g);
    let mut g = game();
    let leaper = g.put(0, LEAPER, Zone::Battlefield);
    generic(&mut g, 4);
    activate(&mut g, leaper, index, vec![], &[], "CCCC").unwrap();
    cite!(["701.67a"]; assert_eq!((g.obj(leaper).tapped, g.players[0].pool.total()), (false, 0), "paid in mana"));
}

#[test]
fn r701_67a_not_enough_to_tap_and_what_cant_be_tapped() {
    let mut g = game();
    let leaper = g.put(0, LEAPER, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let tapped = g.put(0, BEARS, Zone::Battlefield);
    g.tap(tapped);
    let theirs = g.put(1, BEARS, Zone::Battlefield);
    let land = g.put(0, ISLAND, Zone::Battlefield);
    g.tap(land);
    let index = 1 /* printed Flying, then Waterbend activation */;
    generic(&mut g, 1);
    // one mana and two untapped: three of the four
    cite!(["701.67a"]; refused("the supplied mana does not pay that cost, all of it and no more"): unchanged(&mut g, |g| activate(g, leaper, index, vec![], &[leaper, bears], "C")));
    cite!(["701.67a"]; refused("Grizzly Bears is not what that cost asks for or cannot pay it"): unchanged(&mut g, |g| activate(g, leaper, index, vec![], &[leaper, bears, tapped], "C")));
    cite!(["701.67a"]; refused("Grizzly Bears is not what that cost asks for or cannot pay it"): unchanged(&mut g, |g| activate(g, leaper, index, vec![], &[leaper, bears, theirs], "C")));
    g.untap(land);
    cite!(["701.67a"]; refused("Island is not what that cost asks for or cannot pay it"): unchanged(&mut g, |g| activate(g, leaper, index, vec![], &[leaper, bears, land], "C")));
    assert!(!g.obj(bears).tapped && g.stack.is_empty(), "nothing of a refused activation happened");
    // more than the generic mana in it: five for {4}
    let more: Vec<_> = (0..3).map(|_| g.put(0, BEARS, Zone::Battlefield)).collect();
    generic(&mut g, 0);
    cite!(["701.67a"]; refused("a waterbend cost taps at most one artifact or creature for each generic mana in it, 4: 5 chosen"): unchanged(&mut g, |g| activate(g, leaper, index, vec![], &[leaper, bears, more[0], more[1], more[2]], "")));
    // Four selected creatures pay all four generic mana; the Island is unused.
    activate(&mut g, leaper, index, vec![], &[leaper, bears, more[0], more[1]], "").unwrap();
    assert!([leaper, bears, more[0], more[1]].iter().all(|o| g.obj(*o).tapped));
    assert!(!g.obj(land).tapped && !g.obj(more[2]).tapped);
    assert_eq!(g.stack.len(), 1);
}

#[test]
fn r701_67a_with_a_tap_symbol_it_doesnt_tap_its_source_twice() {
    // North Pole Patrol: "Waterbend {3}, {T}: Tap target creature an opponent controls."
    let mut g = game();
    let patrol = g.put(0, PATROL, Zone::Battlefield);
    let bears: Vec<_> = (0..3).map(|_| g.put(0, BEARS, Zone::Battlefield)).collect();
    let theirs = g.put(1, BEARS, Zone::Battlefield);
    let index = 1 /* printed untap activation, then Waterbend activation */;
    generic(&mut g, 0);
    cite!(["118.10"]; refused("North Pole Patrol cannot be tapped twice to pay a cost"): unchanged(&mut g, |g| activate(g, patrol, index, vec![vec![Target::Object(theirs)]], &[patrol, bears[0], bears[1]], "")));
    activate(&mut g, patrol, index, vec![vec![Target::Object(theirs)]], &bears, "").unwrap();
    resolve(&mut g);
    cite!(["701.67a"]; assert!(g.obj(theirs).tapped && g.obj(patrol).tapped));
}

#[test]
fn r701_67b_only_the_generic_mana_of_the_waterbend_cost() {
    // Benevolent River Spirit {U}{U}: "As an additional cost to cast this spell, waterbend {5}."
    let mut g = game();
    let spirit = g.put(0, SPIRIT, Zone::Hand);
    let bears: Vec<_> = (0..7).map(|_| g.put(0, BEARS, Zone::Battlefield)).collect();
    g.players[0].pool.colors[1] = 1;
    let cast = |g: &mut Game, card, tapped: &[ObjId], spending: &str| g.act(0, crate::support::paid(spending, Move::WithCosts(vec![tapped.to_vec()], Box::new(Move::Cast { card, targets: vec![] }))));
    cite!(["701.67b"]; refused("a waterbend cost taps at most one artifact or creature for each generic mana in it, 5: 6 chosen"): unchanged(&mut g, |g| cast(g, spirit, &bears[..6], "U")));
    g.players[0].pool.colors[1] = 2;
    cast(&mut g, spirit, &bears[..5], "UU").unwrap();
    cite!(["701.67a"]; assert!(bears[..5].iter().all(|b| g.obj(*b).tapped) && !g.obj(bears[5]).tapped));
    // Surge {2}{U}: "As an additional cost to cast this spell, waterbend {2}." Four generic
    // mana in all, two of them the waterbend cost's
    let mut g = game();
    let surge = g.put(0, SURGE, Zone::Hand);
    let bears: Vec<_> = (0..4).map(|_| g.put(0, BEARS, Zone::Battlefield)).collect();
    g.players[0].pool.colors[1] = 1;
    g.players[0].pool.colorless = 1;
    cite!(["701.67b"]; refused("a waterbend cost taps at most one artifact or creature for each generic mana in it, 2: 3 chosen"): unchanged(&mut g, |g| cast(g, surge, &bears[..3], "CU")));
    cite!(["701.67b"]; refused("the supplied mana does not pay that cost, all of it and no more"): unchanged(&mut g, |g| cast(g, surge, &bears[..2], "CU")));
    g.players[0].pool.colorless = 2;
    cast(&mut g, surge, &bears[..2], "CCU").unwrap();
    assert_eq!(g.players[0].pool.total(), 0);
}

#[test]
fn r701_67a_waterbend_x_taps_up_to_the_value_announced() {
    // "Waterbend {X}: You gain X life."
    let mut g = game();
    let spring = g.put(0, SPRING, Zone::Battlefield);
    let bears: Vec<_> = (0..3).map(|_| g.put(0, BEARS, Zone::Battlefield)).collect();
    generic(&mut g, 0);
    let with_x = |x, tapped: &[ObjId]| Move::WithX(x, Box::new(Move::Activate { source: spring, ability: 0, targets: vec![], paying: vec![tapped.to_vec()] }));
    cite!(["701.67a", "107.3a"]; refused("a waterbend cost taps at most one artifact or creature for each generic mana in it, 2: 3 chosen"): unchanged(&mut g, |g| g.act(0, with_x(2, &bears))));
    g.act(0, with_x(3, &bears)).unwrap();
    resolve(&mut g);
    cite!(["701.67a"]; assert_eq!(g.players[0].life, 23));
}

/// What the payer of an "unless" is asked: the mana, what it may tap and up to how many, and
/// its generic-mana bound.
fn asked_to_bend(g: &mut Game) -> (game::PlayerId, Vec<ObjId>, u32) {
    match g.asked() {
        Some((player, game::legal::Asked::Pay { waterbend: Some(bend), .. })) => (player, bend.among, bend.most),
        other => panic!("asked {other:?}"),
    }
}
fn lesson(bears: usize) -> (Game, Vec<ObjId>) {
    // Waterbending Lesson {3}{U}: "Draw three cards. Then discard a card unless you waterbend {2}."
    let mut g = game();
    let lesson = g.put(0, LESSON, Zone::Hand);
    let bears: Vec<ObjId> = (0..bears).map(|_| g.put(0, BEARS, Zone::Battlefield)).collect();
    g.players[0].pool.colors[1] = 1;
    g.players[0].pool.colorless = 3;
    g.act(0, crate::support::paid("CCCU", Move::Cast { card: lesson, targets: vec![] })).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    (g, bears)
}

#[test]
fn r701_67a_unless_you_waterbend_with_creatures_tapped() {
    let pay = |tapped: Vec<ObjId>, spending: &str| Move::Choose(game::state::Answer::Pay { plan: vec![], with: Some(crate::support::mana_pool(spending)), tapped });
    // two creatures and no mana: explicitly tap both
    let (mut g, bears) = lesson(2);
    let hand = g.players[0].hand.len();
    let (player, among, most) = asked_to_bend(&mut g);
    cite!(["701.67a", "118.12"]; assert_eq!((player, among.clone(), most), (0, bears.clone(), 2)));
    g.act(0, pay(bears.clone(), "")).unwrap();
    cite!(["701.67a", "118.12"]; assert!(bears.iter().all(|b| g.obj(*b).tapped) && g.decision.is_none() && g.players[0].hand.len() == hand, "no discard"));

    // said: one tapped is too few, three too many; a mana and one creature pays it
    let (mut g, bears) = lesson(3);
    cite!(["701.67a"]; refused("the supplied mana does not pay that cost, all of it and no more"): unchanged(&mut g, |g| g.act(0, pay(vec![bears[0]], ""))));
    cite!(["701.67a"]; refused("a waterbend cost taps at most one artifact or creature for each generic mana in it, 2: 3 chosen"): unchanged(&mut g, |g| g.act(0, pay(bears.clone(), ""))));
    cite!(["701.67a"]; refused("a card is chosen once for that cost"): unchanged(&mut g, |g| g.act(0, pay(vec![bears[0], bears[0]], ""))));
    g.players[0].pool.colorless = 1;
    g.act(0, pay(vec![bears[2]], "C")).unwrap();
    cite!(["701.67a"]; assert_eq!(bears.iter().map(|b| g.obj(*b).tapped).collect::<Vec<_>>(), vec![false, false, true]));
    cite!(["701.67a"]; assert_eq!(g.players[0].pool.total(), 0));
    assert!(g.decision.is_none(), "no discard");

    // declined, or none to tap and no mana: a card is discarded
    for bears in [2, 0] {
        let (mut g, _) = lesson(bears);
        g.act(0, Move::Choose(game::state::Answer::No)).unwrap();
        cite!(["118.12"]; case: bears; assert!(matches!(g.decision, Some(game::state::Decision::Choose { player: 0, choice: game::state::Choice::Objects { .. } })), "{:?}", g.decision));
    }
}

#[test]
fn r702_21a_ward_waterbend_paid_by_the_opponents_creatures() {
    // The Unagi of Kyoshi Island: "Ward—Waterbend {4}."
    for paid in [true, false] {
        let mut g = game();
        let unagi = g.put(0, UNAGI, Zone::Battlefield);
        let shock = g.put(1, SHOCK, Zone::Hand);
        let theirs: Vec<ObjId> = (0..3).map(|_| g.put(1, BEARS, Zone::Battlefield)).collect();
        g.put(0, BEARS, Zone::Battlefield);
        g.act(0, Move::Pass).unwrap();
        g.players[1].pool.colors[3] = 2;
        g.act(1, crate::support::paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Object(unagi)]] })).unwrap();
        while g.asked().is_none_or(|(_, a)| !matches!(a, game::legal::Asked::Pay { .. })) { g.act(g.priority, Move::Pass).unwrap(); }
        let (player, among, most) = asked_to_bend(&mut g);
        // its controller's creatures, not the Unagi's controller's; one red mana and three
        cite!(["702.21a", "701.67a"]; case: paid; assert_eq!((player, among.clone(), most), (1, theirs.clone(), 3)));
        g.act(1, Move::Choose(if paid { game::state::Answer::Pay { plan: vec![], with: Some(crate::support::mana_pool("R")), tapped: theirs.clone() } } else { game::state::Answer::No })).unwrap();
        resolve(&mut g);
        cite!(["702.21a", "701.67a"]; case: paid; assert_eq!(theirs.iter().all(|b| g.obj(*b).tapped), paid));
        cite!(["702.21a"]; case: paid; assert_eq!(g.obj(unagi).damage, if paid { 2 } else { 0 }, "Shock resolves only if ward is paid"));
    }
}

#[track_caller]
fn unchanged(g: &mut Game, action: impl FnOnce(&mut Game) -> Result<(), String>) -> Result<(), String> {
    let before = format!("{g:?}");
    let result = action(g);
    assert_eq!(format!("{g:?}"), before);
    result
}
