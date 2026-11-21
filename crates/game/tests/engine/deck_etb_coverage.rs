//! The remaining sideboard ETB/search cards use their complete corpus definitions.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use game::state::{Answer, Choice, Decision};

const SLIME: usize = 0;
const MAGE: usize = 1;
const TITAN: usize = 2;
const SPIDER: usize = 3;
const ARCHON: usize = 4;
const FOREST: usize = 5;
const ZERO: usize = 6;
const ONE: usize = 7;
const TWO: usize = 8;
const ENCHANTMENT: usize = 9;
const BEAR: usize = 10;
const FLYER: usize = 11;
const NONBASIC: usize = 12;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let mut defs: Vec<_> = ["Acidic Slime", "Trinket Mage", "Primeval Titan", "Stingerfling Spider", "Archon of Justice"]
        .into_iter().map(|name| CardDef::from_faces(catalog.get(name).unwrap()).unwrap()).collect();
    for (name, cost, kind, text, pt) in [
        ("Forest", "", "Basic Land — Forest", "", None),
        ("Zero", "{0}", "Artifact", "", None),
        ("One", "{1}", "Artifact", "", None),
        ("Two", "{2}", "Artifact", "", None),
        ("Enchantment", "{1}", "Enchantment", "", None),
        ("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        ("Flyer", "{1}{U}", "Creature — Bird", "Flying", Some((2, 2))),
        ("Nonbasic", "", "Land", "{T}: Add {C}.", None),
    ] { defs.push(CardDef::new(name, cost, kind, text, pt).unwrap()); }
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    Game::new(defs, 2, 20)
}
fn pass(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
fn resolve(g: &mut Game) {
    while g.decision.is_none() && !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
}
fn cast(g: &mut Game, def: usize) -> ObjId {
    g.players[0].pool.colors = [10; 5];
    let card = g.put(0, def, Zone::Hand);
    g.act(0, crate::support::paid(["WWWGG", "WWU", "WWWWGG", "WWWWG", "WWWWW"][def], Move::Cast { card, targets: vec![] })).unwrap();
    pass(g);
    *g.battlefield.iter().find(|o| g.obj(**o).card == Some(def)).unwrap()
}
fn targets(g: &mut Game, object: ObjId) { g.act(0, Move::ChooseTargets(vec![vec![Target::Object(object)]])).unwrap(); }
fn answer(g: &mut Game, yes: bool) {
    assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May })));
    g.act(0, Move::Choose(if yes { Answer::Yes } else { Answer::No })).unwrap();
}
fn search(g: &Game) -> (Vec<ObjId>, usize, usize) {
    let Some(Decision::Choose { player: 0, choice: Choice::Search { among, min, max, .. } }) = &g.decision else { panic!("search: {:?}", g.decision) };
    (among.clone(), *min, *max)
}
fn choose(g: &mut Game, cards: Vec<ObjId>) { g.act(0, Move::Choose(Answer::Objects(cards))).unwrap(); }
fn shuffles(g: &Game) -> usize { g.log.iter().filter(|e| matches!(e, Event::Shuffled { player: 0, .. })).count() }

#[test]
fn acidic_slime_destroys_each_allowed_type_after_its_source_leaves() {
    for def in [ONE, ENCHANTMENT, FOREST] {
        let mut g = game();
        let victim = g.put(1, def, Zone::Battlefield);
        let bear = g.put(1, BEAR, Zone::Battlefield);
        let slime = cast(&mut g, SLIME);
        assert!(g.has(slime, "Deathtouch"));
        cite!(["115.1"]; refused("an illegal target, Bear: it is neither an artifact nor an enchantment nor a land (115.1)"): g.act(0, Move::ChooseTargets(vec![vec![Target::Object(bear)]])));
        targets(&mut g, victim);
        // Deathtouch is effective on the real card, beyond merely appearing in its text.
        g.damage(slime, Target::Object(bear), 1);
        g.state_based_actions();
        cite!(["702.2b", "704.5h"]; assert_eq!(g.obj(bear).zone, Zone::Gone));
        g.exile(slime);
        resolve(&mut g);
        cite!(["113.7a"]; assert_eq!(g.obj(victim).zone, Zone::Gone));
        assert!(g.players[1].graveyard.iter().any(|o| g.obj(*o).card == Some(def)));
        assert!(g.stack.is_empty());
    }
}

#[test]
fn trinket_mage_searches_only_cheap_artifacts_and_reveals_the_chosen_card_once() {
    let mut g = game();
    let zero = g.put(0, ZERO, Zone::Library);
    let one = g.put(0, ONE, Zone::Library);
    let two = g.put(0, TWO, Zone::Library);
    let land = g.put(0, FOREST, Zone::Library);
    let nonartifact = g.put(0, ENCHANTMENT, Zone::Library);
    let mage = cast(&mut g, MAGE);
    g.exile(mage);
    resolve(&mut g);
    answer(&mut g, true);
    cite!(["701.23a"]; assert_eq!(search(&g), (vec![zero, one], 0, 1)));
    let unfound = "a chosen card is not one the search can find, in the searched zone and matching its description (701.23a): choose between 0 and 1 distinct matching objects";
    for (cards, rule, why) in [(vec![two], "701.23a", unfound), (vec![land], "701.23a", unfound), (vec![nonartifact], "701.23a", unfound),
        (vec![zero, one], "608.2d", "2 chosen (608.2d): choose between 0 and 1 distinct matching objects")] {
        cite!([rule]; case: format!("{cards:?}"); refused(why): g.act(0, Move::Choose(Answer::Objects(cards))));
    }
    crate::support::refused(g.act(1, Move::Choose(Answer::Objects(vec![one]))), "that decision is another player's");
    for (mut branch, selected, def) in [(g.clone(), zero, ZERO), (g, one, ONE)] {
        choose(&mut branch, vec![selected]);
        assert_eq!(branch.players[0].hand.len(), 1);
        assert_eq!(branch.obj(branch.players[0].hand[0]).card, Some(def));
        assert_eq!(branch.players[0].library.len(), 4);
        let reveals: Vec<_> = branch.log.iter().filter_map(|e| if let Event::Revealed { cards, .. } = e { Some(cards.clone()) } else { None }).collect();
        assert_eq!(reveals, vec![vec![selected]]);
        // Uncited (card text): Trinket Mage's "reveal that card, put it into your hand, then shuffle"
        assert_eq!(shuffles(&branch), 1);
        assert!(branch.stack.is_empty() && branch.decision.is_none());
    }
}

#[test]
fn trinket_mage_declines_search_without_shuffling_but_failed_search_still_shuffles() {
    let mut g = game();
    let artifact = g.put(0, ONE, Zone::Library);
    cast(&mut g, MAGE);
    resolve(&mut g);
    let mut declined = g.clone();
    answer(&mut declined, false);
    // Uncited (card text): "you may search your library ... then shuffle": declining the "may" skips the shuffle with the search
    assert_eq!(shuffles(&declined), 0);
    assert_eq!(declined.players[0].library, vec![artifact]);
    answer(&mut g, true);
    choose(&mut g, vec![]);
    // Uncited (card text): "then shuffle" follows the search whether or not it found a card
    assert_eq!(shuffles(&g), 1);
    cite!(["701.23b"]; assert!(g.players[0].hand.is_empty()));
    assert!(g.log.iter().all(|e| !matches!(e, Event::Revealed { .. })));
}

#[test]
fn primeval_titan_enters_searches_up_to_two_lands_and_puts_them_in_tapped() {
    let mut g = game();
    let lands = [g.put(0, FOREST, Zone::Library), g.put(0, NONBASIC, Zone::Library), g.put(0, FOREST, Zone::Library)];
    let other = g.put(0, BEAR, Zone::Library);
    let titan = cast(&mut g, TITAN);
    assert!(g.has(titan, "Trample"));
    resolve(&mut g);
    answer(&mut g, true);
    cite!(["701.23a", "701.23b"]; assert_eq!(search(&g), (lands.to_vec(), 0, 2)));
    for (invalid, rule, why) in [
        (vec![other], "701.23a", "a chosen card is not one the search can find, in the searched zone and matching its description (701.23a): choose between 0 and 2 distinct matching objects"),
        (lands.to_vec(), "608.2d", "3 chosen (608.2d): choose between 0 and 2 distinct matching objects"),
        (vec![lands[0], lands[0]], "608.2d", "each object is chosen once (608.2d): choose between 0 and 2 distinct matching objects"),
    ] {
        cite!([rule]; case: why; refused(why): g.act(0, Move::Choose(Answer::Objects(invalid))));
    }
    for selected in [vec![], vec![lands[1]], vec![lands[0], lands[1]]] {
        let mut branch = g.clone();
        choose(&mut branch, selected.clone());
        let arrived: Vec<_> = branch.battlefield.iter().copied().filter(|o| *o != titan).collect();
        assert_eq!(arrived.len(), selected.len());
        for permanent in arrived {
            assert!(branch.obj(permanent).tapped);
            assert_eq!(branch.controller(permanent), 0);
        }
        assert_eq!(branch.players[0].library.len(), 4 - selected.len());
        // Uncited (card text): Primeval Titan's "put them onto the battlefield tapped, then shuffle", whatever was found
        assert_eq!(shuffles(&branch), 1);
        assert!(branch.log.iter().all(|e| !matches!(e, Event::Revealed { .. })));
        assert!(branch.stack.is_empty());
    }
}

#[test]
fn primeval_titan_attacks_trigger_the_same_optional_search_and_trample_works() {
    let mut g = game();
    let titan = g.put(0, TITAN, Zone::Battlefield);
    let land = g.put(0, NONBASIC, Zone::Library);
    let blocker = g.put(1, BEAR, Zone::Battlefield);
    while !matches!(g.decision, Some(Decision::Attackers { player: 0 })) { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Attack(vec![(titan, Target::Player(1))])).unwrap();
    resolve(&mut g);
    let mut declined = g.clone();
    answer(&mut declined, false);
    // Uncited (card text): "you may search your library ... then shuffle": declining the "may" skips the shuffle with the search
    assert_eq!(shuffles(&declined), 0);
    answer(&mut g, true);
    assert_eq!(search(&g), (vec![land], 0, 1));
    choose(&mut g, vec![land]);
    let arrived = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(NONBASIC)).unwrap();
    assert!(g.obj(arrived).tapped);
    // Uncited (card text): Primeval Titan's "then shuffle"
    assert_eq!(shuffles(&g), 1);
    while !matches!(g.decision, Some(Decision::Blockers { player: 1 })) { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(1, Move::Block(vec![(blocker, titan)])).unwrap();
    for _ in 0..12 {
        if g.players[1].life == 16 { break; }
        match g.decision {
            Some(Decision::Divide { .. }) => g.act(0, Move::Divide(vec![(Target::Object(blocker), 2), (Target::Player(1), 4)])).unwrap(),
            None => g.act(g.priority, Move::Pass).unwrap(),
            ref other => panic!("unexpected combat question: {other:?}"),
        }
    }
    cite!(["702.19b"]; assert_eq!(g.players[1].life, 16));
    cite!(["704.5g"]; assert_eq!(g.obj(blocker).zone, Zone::Gone));
}

#[test]
fn stingerfling_spider_may_destroy_a_flyer_but_cannot_target_a_ground_creature() {
    let mut g = game();
    let flyer = g.put(1, FLYER, Zone::Battlefield);
    let ground = g.put(1, BEAR, Zone::Battlefield);
    let spider = cast(&mut g, SPIDER);
    assert!(g.has(spider, "Reach"));
    cite!(["115.1"]; refused("an illegal target, Bear: it doesn't have flying (115.1)"): g.act(0, Move::ChooseTargets(vec![vec![Target::Object(ground)]])));
    targets(&mut g, flyer);
    g.exile(spider);
    resolve(&mut g);
    for yes in [false, true] {
        let mut branch = g.clone();
        answer(&mut branch, yes);
        cite!(["603.5"]; assert_eq!(branch.obj(flyer).zone, if yes { Zone::Gone } else { Zone::Battlefield }));
        assert_eq!(branch.obj(ground).zone, Zone::Battlefield);
        assert!(branch.stack.is_empty());
    }
}

#[test]
fn archon_of_justice_exiles_any_permanent_from_its_departed_death_trigger() {
    for def in [ONE, ENCHANTMENT, FOREST, BEAR] {
        let mut g = game();
        let archon = g.put(0, ARCHON, Zone::Battlefield);
        assert!(g.has(archon, "Flying"));
        let victim = g.put(1, def, Zone::Battlefield);
        let hand = g.put(1, BEAR, Zone::Hand);
        assert!(g.destroy(archon));
        g.act(0, Move::Pass).unwrap();
        assert_eq!(g.obj(archon).zone, Zone::Gone);
        for (illegal, why) in [(Target::Player(1), "an illegal target, that player: a player isn't a permanent (115.1)"),
            (Target::Object(hand), "an illegal target, Bear: it isn't a permanent on the battlefield (115.1)")] {
            cite!(["115.1"]; case: why; refused(why): g.act(0, Move::ChooseTargets(vec![vec![illegal]])));
        }
        targets(&mut g, victim);
        resolve(&mut g);
        assert_eq!(g.obj(victim).zone, Zone::Gone);
        cite!(["603.10a"]; assert!(g.exile.iter().any(|o| g.obj(*o).card == Some(def))));
        assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).card == Some(ARCHON)));
    }
}
