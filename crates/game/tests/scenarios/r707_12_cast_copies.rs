//! Casting copies of a card (707.12): "Choose an instant or sorcery card exiled this way and
//! copy it three times. You may cast the copies without paying their mana costs." Each copy is
//! made where the card is, is offered on its own (707.12a), is cast and not merely put on the
//! stack, and ceases to exist anywhere but the stack and the battlefield (707.10a).
use crate::support::cite;
use game::legal::Asked;
use game::state::Answer;
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const CHANDRA: usize = 0;
const MOUNTAIN: usize = 1;
const BOLT: usize = 2;
const WATCHER: usize = 3;
const BEAR: usize = 4;
const SHOCK: usize = 5;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let defs = vec![
        CardDef::from_faces(catalog.get("Chandra, Pyromaster").expect("in the corpus")).unwrap(),
        CardDef::new("Mountain", "", "Basic Land — Mountain", "", None).unwrap(),
        CardDef::new("Lightning Bolt", "{R}", "Instant", "Lightning Bolt deals 3 damage to any target.", None).unwrap(),
        CardDef::new("Watcher", "{W}", "Enchantment", "Whenever you cast a spell, you gain 2 life.", None).unwrap(),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Shock", "{R}", "Sorcery", "Shock deals 2 damage to any target.", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    let mut g = Game::new(defs, 2, 20);
    for player in 0..2 { for _ in 0..12 { g.put(player, MOUNTAIN, Zone::Library); } }
    g
}
/// Chandra's last ability resolved up to its first question, with these cards on top of the
/// library among Mountains.
fn ultimate(g: &mut Game, top: &[usize]) -> ObjId {
    for card in top { g.put(0, *card, Zone::Library); }
    let chandra = g.put(0, CHANDRA, Zone::Battlefield);
    g.put_counters(chandra, "loyalty", 7 - g.obj(chandra).counters.get("loyalty").copied().unwrap_or(0));
    g.act(0, Move::Activate { source: chandra, ability: 2, targets: vec![], paying: vec![] }).unwrap();
    while g.asked().is_some_and(|(_, a)| a == Asked::Priority) && !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    chandra
}
fn copies(g: &Game, zone: Zone) -> Vec<ObjId> {
    let list = match zone { Zone::Exile => &g.exile, Zone::Stack => &g.stack, _ => &g.players[0].graveyard };
    list.iter().copied().filter(|o| g.obj(*o).card.is_none() && g.obj(*o).chars.name == "Lightning Bolt").collect()
}

fn offered(g: &mut Game) -> Vec<ObjId> {
    match g.asked() { Some((0, Asked::Cast { cards, .. })) => cards, other => panic!("not an offer to cast: {other:?}") }
}
fn bolt(g: &mut Game, copy: ObjId, at: usize) {
    g.act(0, Move::Cast { card: copy, targets: vec![vec![Target::Player(at)]] }).unwrap();
}
fn resolve(g: &mut Game) {
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
}

#[test]
fn r707_12_three_copies_are_made_in_exile_and_each_is_cast_for_nothing() {
    let mut g = game();
    let watcher = g.put(0, WATCHER, Zone::Battlefield);
    let chandra = ultimate(&mut g, &[BOLT, BEAR]);
    assert_eq!(g.exile.len(), 13, "ten cards and three copies");
    let made = offered(&mut g);
    cite!(["707.12"]; assert_eq!((made.len(), copies(&g, Zone::Exile)), (3, made.clone())));
    for copy in &made {
        let o = g.obj(*copy);
        assert!(!o.token() && o.card.is_none() && o.is(ir::CardType::Instant) && o.owner == 0);
        cite!(["707.2"]; assert_eq!(o.chars.mana_value, 1, "a copy has the card's mana cost (707.2)"));
    }
    cite!(["608.2g"]; refused("a decision is pending: cast one of the cards offered, or decline (608.2g)"): g.act(0, Move::Pass));
    bolt(&mut g, made[0], 1);
    cite!(["707.12a"]; assert_eq!(offered(&mut g), made[1..], "each is offered on its own (707.12a)"));
    bolt(&mut g, made[1], 1);
    bolt(&mut g, made[2], 0);
    cite!(["707.12", "608.2g"]; assert_eq!((copies(&g, Zone::Stack).len(), copies(&g, Zone::Exile).len()), (3, 0)));
    cite!(["118.9"]; assert!(g.players[0].pool.colors.iter().all(|n| *n == 0), "nothing was paid"));
    // Cast triggers wait while Chandra resolves. At the next priority checkpoint
    // the controller orders all three distinct Watcher occurrences (603.3b).
    let Some((0, Asked::Order(abilities))) = g.asked() else { panic!("expected three cast triggers to order: {:?}", g.asked()) };
    cite!(["603.3b"]; assert_eq!(abilities.len(), 3));
    assert!(abilities.iter().all(|(source, _)| *source == Some(watcher)));
    g.act(0, Move::Order(vec![0, 1, 2])).unwrap();
    resolve(&mut g);
    // each copy was cast (707.12): three triggers of "whenever you cast a spell"
    cite!(["707.12"]; assert_eq!((g.players[1].life, g.players[0].life), (14, 20 - 3 + 6)));
    // a copy that has left the stack ceases to exist (707.10a); the card stays exiled
    cite!(["707.10a"]; assert!(copies(&g, Zone::Graveyard).is_empty() && copies(&g, Zone::Exile).is_empty()));
    assert_eq!(g.exile.iter().filter(|o| g.obj(**o).card == Some(BOLT)).count(), 1);
    assert_eq!(g.exile.len(), 10);
    assert_eq!(g.obj(chandra).zone, Zone::Gone, "no loyalty is left (704.5i)");
}

#[test]
fn r707_10a_a_copy_that_is_not_cast_ceases_to_exist() {
    let mut g = game();
    ultimate(&mut g, &[BOLT]);
    let made = offered(&mut g);
    bolt(&mut g, made[0], 1);
    g.act(0, Move::Choose(Answer::No)).unwrap();
    assert_eq!(g.asked().map(|(_, a)| a), Some(Asked::Priority));
    cite!(["707.10a", "704.5e"]; assert!(copies(&g, Zone::Exile).is_empty(), "state-based actions were checked"));
    assert_eq!(g.exile.len(), 10);
    cite!(["704.5e"]; refused("that object has ceased to exist: there is nothing to cast"): g.act(0, Move::Cast { card: made[1], targets: vec![vec![Target::Player(1)]] }));
    resolve(&mut g);
    assert_eq!(g.players[1].life, 17);
}

#[test]
fn r608_2c_with_no_instant_or_sorcery_exiled_nothing_is_copied() {
    let mut g = game();
    ultimate(&mut g, &[BEAR]);
    cite!(["101.3"]; assert_eq!(g.asked().map(|(_, a)| a), Some(Asked::Priority)));
    // the card's instruction exiles ten; 101.3 only lets the rest still happen
    cite!(["101.3"]; assert_eq!(g.exile.len(), 10));
    cite!(["101.3"]; assert!(g.stack.is_empty()));
}

#[test]
fn r707_12_the_card_chosen_is_the_one_copied() {
    let mut g = game();
    ultimate(&mut g, &[BOLT, SHOCK]);
    // Uncited (card text): "Choose an instant or sorcery card exiled this way" asks for one card.
    let Some((0, Asked::Objects { among, count: 1, .. })) = g.asked() else { panic!("a card is chosen: {:?}", g.asked()) };
    // Uncited (card text): "Choose an instant or sorcery card exiled this way" offers the instant and the sorcery only.
    assert_eq!(among.len(), 2, "the instant and the sorcery, not the lands");
    let shock = *among.iter().find(|o| g.obj(**o).card == Some(SHOCK)).unwrap();
    g.act(0, Move::Choose(Answer::Objects(vec![shock]))).unwrap();
    let made = offered(&mut g);
    assert_eq!(made.len(), 3);
    cite!(["707.12", "707.2"]; assert!(made.iter().all(|o| g.obj(*o).chars.name == "Shock" && g.obj(*o).is(ir::CardType::Sorcery))));
    // a sorcery's copy is cast as the effect is applied, whatever its type's timing (608.2g)
    cite!(["608.2g"]; ok: g.act(0, Move::Cast { card: made[0], targets: vec![vec![Target::Player(1)]] }));
    g.act(0, Move::Choose(Answer::No)).unwrap();
    resolve(&mut g);
    assert_eq!(g.players[1].life, 18);
    assert_eq!(g.exile.len(), 10);
}
