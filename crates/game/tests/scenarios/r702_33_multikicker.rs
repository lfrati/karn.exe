//! Multikicker (702.33c): "You may pay an additional [cost] any number of times as you cast
//! this spell." How many times is declared as the spell is cast (601.2b), the cost is locked
//! in with the rest (601.2f), and the permanent's linked abilities count them (702.33e).
use crate::support::cite;
use game::{CardDef, Game, Move, ObjId, Zone};

const WARCALLER: usize = 0;
const ELF: usize = 1;
const BEAR: usize = 2;
const FOREST: usize = 3;
const DOUBLER: usize = 4;

const TEXT: &str = "Multikicker {1}{G} (You may pay an additional {1}{G} any number of times as you cast this spell.)\nThis creature enters with a +1/+1 counter on it for each time it was kicked.\nOther Elf creatures you control get +1/+1 for each +1/+1 counter on this creature.";

fn game() -> Game {
    let defs = vec![
        CardDef::new("Joraga Warcaller", "{G}", "Creature — Elf Warrior", TEXT, Some((1, 1))).unwrap(),
        CardDef::new("Llanowar Scout", "{G}", "Creature — Elf Scout", "", Some((1, 3))).unwrap(),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "{T}: Add {G}.", None).unwrap(),
        CardDef::new("Doubler", "{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on it instead.", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    let mut g = Game::new(defs, 2, 20);
    for player in 0..2 { for _ in 0..5 { g.put(player, FOREST, Zone::Library); } }
    g
}
fn green(g: &mut Game, n: u32) { g.players[0].pool.colors = [0, 0, 0, 0, n]; }
fn kick(times: u32, card: ObjId) -> Move { Move::WithKicks(times, Box::new(Move::Cast { card, targets: vec![] })) }
fn resolve(g: &mut Game) { while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); } }
fn counters(g: &Game, o: ObjId) -> u32 { g.obj(o).counters.get("+1/+1").copied().unwrap_or(0) }
fn pt(g: &Game, o: ObjId) -> (Option<i32>, Option<i32>) { (g.power(o), g.toughness(o)) }
fn warcaller(g: &Game) -> ObjId { *g.battlefield.iter().find(|o| g.obj(**o).card == Some(WARCALLER)).expect("it entered") }

#[test]
fn r702_33c_each_kick_is_paid_and_it_enters_with_a_counter_for_each() {
    let mut g = game();
    let elf = g.put(0, ELF, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let theirs = g.put(1, ELF, Zone::Battlefield);
    let card = g.put(0, WARCALLER, Zone::Hand);
    // {G} and {1}{G} three times
    green(&mut g, 6);
    cite!(["601.2f", "601.2h"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0, crate::support::paid("GGGGGG",kick(3, card))));
    assert_eq!(g.obj(card).zone, Zone::Hand);
    green(&mut g, 8);
    g.act(0, crate::support::paid("GGGGGGG", kick(3, card))).unwrap();
    cite!(["601.2f"]; assert_eq!(g.players[0].pool.colors[4], 1, "seven mana was paid"));
    let spell = *g.stack.last().unwrap();
    let audit=g.audit_game_objects();
    cite!(["702.33c", "702.33d"]; assert_eq!(audit["objects"][spell.0.to_string()]["announced"]["costs"]["kicks"], 3));
    cite!(["702.33d", "118.8d"]; assert_eq!((g.obj(spell).kicked, g.obj(spell).chars.mana_value), (3, 1), "an additional cost does not change its mana value (202.3)"));
    resolve(&mut g);
    let caller = warcaller(&g);
    cite!(["702.33e"]; assert_eq!((counters(&g, caller), pt(&g, caller)), (3, (Some(4), Some(4)))));
    assert_eq!(pt(&g, elf), (Some(4), Some(6)), "other Elves you control get +1/+1 for each");
    assert_eq!((pt(&g, bear), pt(&g, theirs)), ((Some(2), Some(2)), (Some(1), Some(3))));
}

#[test]
fn r702_33d_a_spell_that_was_not_kicked_enters_with_none() {
    let mut g = game();
    let elf = g.put(0, ELF, Zone::Battlefield);
    let card = g.put(0, WARCALLER, Zone::Hand);
    green(&mut g, 3);
    g.act(0, crate::support::paid("G", Move::Cast { card, targets: vec![] })).unwrap();
    assert_eq!(g.players[0].pool.colors[4], 2, "only its mana cost");
    resolve(&mut g);
    let caller = warcaller(&g);
    cite!(["702.33d"]; assert_eq!((counters(&g, caller), pt(&g, caller), pt(&g, elf)), (0, (Some(1), Some(1)), (Some(1), Some(3)))));
    // one put onto the battlefield without being cast was not kicked
    let old = g.put(0, WARCALLER, Zone::Graveyard);
    g.move_object(old, Zone::Battlefield);
    let put = *g.battlefield.last().unwrap();
    assert_ne!(put, old);
    assert_eq!(g.obj(put).card, Some(WARCALLER));
    cite!(["702.33d"]; assert_eq!(counters(&g, put), 0));
}

#[test]
fn r702_33c_only_a_spell_with_a_kicker_cost_is_kicked() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Hand);
    green(&mut g, 6);
    cite!(["702.33a"]; refused("that spell has no kicker cost to pay (702.33)"): g.act(0, kick(1, bear)));
    assert_eq!(g.obj(bear).zone, Zone::Hand);
    g.act(0, crate::support::paid("GG", kick(0, bear))).unwrap();
}

#[test]
fn r614_1c_its_entering_counters_are_counters_put_on_it() {
    let mut g = game();
    g.put(0, DOUBLER, Zone::Battlefield);
    let card = g.put(0, WARCALLER, Zone::Hand);
    green(&mut g, 5);
    g.act(0, crate::support::paid("GGGGG", kick(2, card))).unwrap();
    resolve(&mut g);
    cite!(["122.6a", "614.1c"]; assert_eq!(counters(&g, warcaller(&g)), 4, "a replacement of counters applies to those it enters with (614.1c, 122.6a)"));
}

#[test]
fn r400_7_what_it_becomes_after_it_leaves_was_not_kicked() {
    let mut g = game();
    let card = g.put(0, WARCALLER, Zone::Hand);
    green(&mut g, 5);
    g.act(0, crate::support::paid("GGGGG", kick(2, card))).unwrap();
    resolve(&mut g);
    let caller = warcaller(&g);
    assert_eq!(g.obj(caller).kicked, 2);
    g.move_object(caller, Zone::Hand);
    let hand = *g.players[0].hand.last().unwrap();
    cite!(["400.7"]; assert_eq!(g.obj(hand).kicked, 0));
    green(&mut g, 1);
    g.act(0, crate::support::paid("G", Move::Cast { card: hand, targets: vec![] })).unwrap();
    resolve(&mut g);
    cite!(["400.7"]; assert_eq!(counters(&g, warcaller(&g)), 0));
}

#[test]
fn r601_2b_explicit_multikicker_counts_require_the_full_payment() {
    let mut g = game();
    let card = g.put(0, WARCALLER, Zone::Hand);
    for _ in 0..5 { g.put(0, FOREST, Zone::Battlefield); }
    let sources = g.battlefield.clone();
    for kicks in [0, 1, 2, 3, 17] {
        let mut branch = g.clone();
        let plan = sources.iter().map(|source| game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source: *source,
            ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None }).collect();
        let supplied = crate::support::mana_pool(match kicks { 0 => "G", 1 => "GGG", 2 => "GGGGG", _ => "GGGGG" });
        let result = branch.act(0, Move::Paying(supplied, Box::new(Move::WithMana(plan, Box::new(Move::WithKicks(kicks,
            Box::new(Move::Cast { card, targets: vec![] })))))));
        cite!(["702.33c", "601.2h"]; assert_eq!(result.is_ok(), kicks <= 2, "kicks={kicks}: {result:?}"));
    }

}
