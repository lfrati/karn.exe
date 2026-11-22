//! Cascade uses the shared sequential exile, ordinary free-cast, and random-bottom owners.
use crate::support::{cite, mana_pool, oracle_read, paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, Zone};

fn setup() -> Game {
    let defs = vec![oracle_read("Bloodbraid Elf"), oracle_read("Mountain"),
        oracle_read("Runeclaw Bear"), oracle_read("Hill Giant"), oracle_read("Lotus Bloom"),
        CardDef::new("Counter", "{0}", "Instant", "Counter target spell.", None).unwrap(),
        oracle_read("Rift Bolt")];
    // Uncited (admission): complete pinned cards, with one generic composition fixture.
    for def in &defs { game::play::check(def).unwrap_or_else(|why| panic!("{}: {}", def.name(), why.0)); }
    Game::new(defs, 2, 20)
}
fn library(game: &mut Game, cards: &[usize]) -> Vec<ObjId> {
    cards.iter().map(|card| game.put(0, *card, Zone::Library)).collect()
}
fn passes(game: &mut Game) { for _ in 0..2 { game.act(game.priority, Move::Pass).unwrap(); } }
fn start(game: &mut Game) -> ObjId {
    let card = game.put(0, 0, Zone::Hand);
    game.players[0].pool = mana_pool("CCRG");
    game.act(0, paid("CCRG", Move::Cast { card, targets: vec![] })).unwrap();
    game.stack[0]
}
fn offered(game: &Game) -> ObjId {
    let Some(Decision::Choose { player: 0, choice: Choice::Cast { cards } }) = &game.decision else { panic!("{:?}", game.decision) };
    assert_eq!(cards.len(), 1);
    cards[0]
}

#[test]
fn r702_85a_bloodbraid_exiles_through_lands_and_equal_values_then_casts_before_it_resolves() {
    let mut game = setup();
    let cards = library(&mut game, &[1, 2, 3, 1]);
    let spell = start(&mut game);
    cite!(["702.85a", "603.3"]; assert_eq!(game.stack.len(), 2));
    passes(&mut game);
    let hit = offered(&game);
    cite!(["702.85a"]; assert_eq!(game.obj(hit).chars.name, "Runeclaw Bear"));
    let exiled: Vec<_> = game.log.iter().filter_map(|event| match event { Event::Exiled { object } => Some(*object), _ => None }).collect();
    cite!(["702.85a"]; assert_eq!(exiled, vec![cards[3], cards[2], cards[1]]));
    cite!(["702.85a", "118.9"]; ok: game.act(0, Move::Cast { card: hit, targets: vec![] }));
    cite!(["702.85a", "608.2g"]; assert_eq!(game.stack.len(), 2));
    cite!(["702.85a"]; assert_eq!(game.stack[0], spell));
    cite!(["702.85a"]; assert_eq!(game.obj(game.stack[1]).chars.name, "Runeclaw Bear"));
    cite!(["702.85a"]; assert!(game.exile.is_empty()));
    cite!(["702.85a"]; assert_eq!(game.players[0].library.len(), 3));
    cite!(["702.85a"]; assert_eq!(game.players[0].library.last(), Some(&cards[0])));
    passes(&mut game);
    cite!(["608.3"]; assert!(game.battlefield.iter().any(|card| game.obj(*card).chars.name == "Runeclaw Bear")));
    cite!(["608.2c"]; assert_eq!(game.stack, vec![spell]));
}

#[test]
fn r702_85a_declined_hit_returns_with_the_other_exiled_cards_and_permission_ends() {
    let mut game = setup();
    let cards = library(&mut game, &[1, 2, 3, 1]);
    start(&mut game); passes(&mut game); let hit = offered(&game);
    cite!(["702.85a"]; ok: game.act(0, Move::Choose(Answer::No)));
    cite!(["702.85a"]; assert!(game.players[0].hand.is_empty() && game.exile.is_empty()));
    cite!(["702.85a"]; assert_eq!(game.players[0].library.len(), 4));
    cite!(["702.85a"]; assert_eq!(game.players[0].library.last(), Some(&cards[0])));
    cite!(["400.7", "733.1"]; refused("ceased to exist"): game.act(0, Move::Cast { card: hit, targets: vec![] }));
}

#[test]
fn r702_85a_empty_or_exhausted_without_a_match_returns_every_actual_exile() {
    for cards in [vec![], vec![1, 1], vec![3, 1, 3]] {
        let mut game = setup(); library(&mut game, &cards); let spell = start(&mut game); passes(&mut game);
        cite!(["702.85a"]; assert!(game.decision.is_none()));
        cite!(["702.85a"]; assert!(game.exile.is_empty() && game.players[0].hand.is_empty()));
        cite!(["702.85a"]; assert_eq!(game.players[0].library.len(), cards.len()));
        cite!(["608.2c"]; assert_eq!(game.stack, vec![spell]));
    }
}

#[test]
fn r702_85a_countering_the_source_spell_does_not_counter_its_independent_cascade() {
    let mut game = setup(); library(&mut game, &[2]); let spell = start(&mut game);
    let counter = game.put(0, 5, Zone::Hand);
    game.act(0, Move::Cast { card: counter, targets: vec![vec![spell.into()]] }).unwrap(); passes(&mut game);
    cite!(["113.7a", "701.6"]; assert_eq!(game.obj(spell).zone, Zone::Gone));
    cite!(["113.7a"]; assert_eq!(game.stack.len(), 1));
    passes(&mut game); let hit = offered(&game);
    cite!(["702.85a", "608.2h"]; assert_eq!(game.obj(hit).chars.name, "Runeclaw Bear"));
    cite!(["702.85a"]; ok: game.act(0, Move::Cast { card: hit, targets: vec![] }));
    cite!(["702.85a"]; assert!(game.exile.is_empty()));
}

#[test]
fn r702_85a_zero_value_source_cannot_hit_zero_and_a_no_mana_cost_hit_can_be_cast() {
    let mut game = setup(); library(&mut game, &[4]); start(&mut game); passes(&mut game); let hit = offered(&game);
    cite!(["702.85a", "118.6a"]; ok: game.act(0, Move::Cast { card: hit, targets: vec![] }));
    cite!(["702.85a"]; assert_eq!(game.obj(*game.stack.last().unwrap()).chars.name, "Lotus Bloom"));
    let mut game = setup(); let mut defs = game.defs.to_vec();
    defs[0] = CardDef::new("Zero cascade", "{0}", "Artifact", "Cascade", None).unwrap();
    game = Game::new(defs, 2, 20); library(&mut game, &[4, 1]);
    let card = game.put(0, 0, Zone::Hand); game.act(0, Move::Cast { card, targets: vec![] }).unwrap(); passes(&mut game);
    cite!(["702.85a"]; assert!(game.decision.is_none() && game.exile.is_empty()));
    cite!(["702.85a"]; assert_eq!(game.players[0].library.len(), 2));
}

#[test]
fn r702_85a_resulting_spell_face_must_remain_strictly_below_source_value() {
    use game::card::CastAs;
    // Independent dual-characteristic fixture isolates the rule's resulting-spell check.
    let normal = CardDef::new("Small", "{2}", "Creature — Giant", "", Some((2, 2))).unwrap();
    let alternate = CardDef::new("Equal", "{4}", "Sorcery — Adventure", "You gain 1 life.", None).unwrap();
    let mut game = setup(); let mut defs = game.defs.to_vec(); defs.push(CardDef::adventurer(normal, alternate).unwrap());
    game = Game::new(defs, 2, 20); library(&mut game, &[7]); start(&mut game); passes(&mut game); let hit = offered(&game);
    cite!(["702.85a", "715.3a"]; refused("effect's bound"): game.act(0,
        Move::CastAs(CastAs::Adventure, Box::new(Move::Cast { card: hit, targets: vec![] }))));
    cite!(["733.1"]; assert_eq!(game.obj(hit).zone, Zone::Exile));
    cite!(["702.85a"]; ok: game.act(0, Move::Cast { card: hit, targets: vec![] }));
}

#[test]
fn full_cascade_oracle_family_compiles() {
    for name in ["Bloodbraid Elf", "Shardless Agent", "Violent Outburst", "Bituminous Blast", "Maelstrom Wanderer", "Throes of Chaos"] {
        // Uncited (admission): full pinned Oracle cards, rather than keyword-only stubs.
        game::play::check(&oracle_read(name)).unwrap_or_else(|why| panic!("{name}: {}", why.0));
    }
}

#[test]
fn r702_85c_maelstrom_wanderer_has_two_separate_cascades_in_the_supplied_order() {
    let mut game = setup(); let mut defs = game.defs.to_vec(); defs[0] = oracle_read("Maelstrom Wanderer");
    game = Game::new(defs, 2, 20); library(&mut game, &[2, 2]);
    let card = game.put(0, 0, Zone::Hand); game.players[0].pool = mana_pool("CCCCCURG");
    game.act(0, paid("CCCCCURG", Move::Cast { card, targets: vec![] })).unwrap();
    cite!(["702.85c", "603.3b"]; assert!(matches!(&game.decision, Some(Decision::Order { player: 0, abilities }) if abilities.len() == 2)));
    cite!(["603.3b"]; ok: game.act(0, Move::Order(vec![0, 1])));
    cite!(["702.85c"]; assert_eq!(game.stack.len(), 3));
    passes(&mut game); let first = offered(&game);
    cite!(["702.85a"]; ok: game.act(0, Move::Cast { card: first, targets: vec![] }));
    passes(&mut game);
    cite!(["608.3"]; assert_eq!(game.battlefield.len(), 1));
    passes(&mut game); let second = offered(&game);
    cite!(["702.85c", "400.7"]; assert_ne!(first, second));
    cite!(["702.85a"]; ok: game.act(0, Move::Choose(Answer::No)));
    cite!(["702.85c"]; assert_eq!(game.stack.len(), 1));
    cite!(["702.85a"]; assert_eq!(game.players[0].library.len(), 1));
}

#[test]
fn r202_3e_cascade_uses_its_source_spells_announced_x_including_after_departure() {
    let mut game = setup(); let mut defs = game.defs.to_vec();
    defs[0] = CardDef::new("Variable cascade", "{X}{U}", "Sorcery", "Cascade\nYou gain 1 life.", None).unwrap();
    game = Game::new(defs, 2, 20); library(&mut game, &[3]);
    let card = game.put(0, 0, Zone::Hand); game.players[0].pool = mana_pool("CCCCU");
    cite!(["202.3e", "107.3a"]; ok: game.act(0, paid("CCCCU", Move::WithX(4, Box::new(Move::Cast { card, targets: vec![] })))));
    let spell = game.stack[0]; let counter = game.put(0, 5, Zone::Hand);
    game.act(0, Move::Cast { card: counter, targets: vec![vec![spell.into()]] }).unwrap(); passes(&mut game); passes(&mut game);
    let hit = offered(&game);
    cite!(["202.3e", "608.2h", "702.85a"]; assert_eq!(game.obj(hit).chars.name, "Hill Giant"));
    cite!(["702.85a"]; ok: game.act(0, Move::Cast { card: hit, targets: vec![] }));
}

#[test]
fn r702_85a_cascade_cast_on_a_row_with_suspend_does_not_inherit_suspend_haste() {
    let mut game = setup(); let mut defs = game.defs.to_vec();
    defs[0] = CardDef::new("Two mechanics", "{4}", "Creature — Giant", "Suspend 1—{0}, cascade", Some((4, 4))).unwrap();
    game = Game::new(defs, 2, 20); library(&mut game, &[2]);
    let card = game.put(0, 0, Zone::Hand); game.players[0].pool = mana_pool("CCCC");
    game.act(0, paid("CCCC", Move::Cast { card, targets: vec![] })).unwrap(); passes(&mut game);
    let hit = offered(&game); game.act(0, Move::Cast { card: hit, targets: vec![] }).unwrap();
    cite!(["702.85a", "702.62a"]; assert!(!game.has(*game.stack.last().unwrap(), "Haste")));
    passes(&mut game);
    cite!(["702.85a", "702.62a"]; assert!(!game.has(game.battlefield[0], "Haste")));
}
