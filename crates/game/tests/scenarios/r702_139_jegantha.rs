//! Exact printed symbols constrain companion designation; restricted units cannot pay a generic component.
use crate::support::{cite, oracle, mana_pool};
use game::{CardDef, Game, Move, Zone};
use game::state::Pool;

#[test]
fn r702_139_jegantha_compares_whole_symbols_including_both_split_halves() {
    for (name, allowed) in [("Lightning Bolt", true), ("Wear // Tear", true), ("Counterspell", false),
        ("Walking Ballista", false), ("Burning-Tree Emissary", false), ("Fire // Ice", false)] {
        let defs = vec![oracle("Jegantha, the Wellspring"), oracle("Forest"), oracle(name)];
        let mut decks = vec![vec![1;20];2];
        decks[0].push(2);
        let starting = Game::start_with_companions(defs, &decks, &[Some(0),None], 20, 0, 1);
        if allowed {
            cite!(["702.139a", "709.4b"]; case:name; assert!(starting.is_ok()));
        } else {
            cite!(["702.139a", "709.4b"]; case:name; refused(&format!("starting deck does not meet companion condition: {name}")): starting);
        }
    }
}

#[test]
fn r106_6_jegantha_mana_pays_colored_symbols_but_not_generic_or_announced_x() {
    for cost in ["{1}{G}", "{2/G}", "{X}{G}"] {
        let subject = CardDef::new("Payment subject", cost, "Creature", "", Some((1,1))).unwrap();
        let mut g = Game::new(vec![oracle("Jegantha, the Wellspring"), subject], 2, 20);
        let source = g.put(0, 0, Zone::Battlefield);
        g.act(0, Move::Activate { source, ability: 1, targets: vec![], paying: vec![] }).unwrap();
        cite!(["106.6"]; assert_eq!(g.players[0].pool.restricted.len(), 5));
        let card = g.put(0, 1, Zone::Hand);
        let spell = Move::Cast { card, targets: vec![] };
        let spell = if cost == "{X}{G}" { Move::WithX(1, Box::new(spell)) } else { spell };
        let green = g.players[0].pool.restricted[4].clone();
        let white = g.players[0].pool.restricted[0].clone();
        let bad = Pool { restricted: vec![green.clone(),white], ..Pool::default() };
        cite!(["106.6", "107.4b", "601.2h"]; case:cost; refused("the supplied mana does not pay that cost"): g.act(0, Move::Paying(bad, Box::new(spell.clone()))));
        let good = if cost == "{2/G}" { Pool { restricted:vec![green], ..Pool::default() } }
            else { g.players[0].pool.colorless = 1; Pool { restricted:vec![green], ..mana_pool("C") } };
        cite!(["106.6", "601.2h"]; case:cost; ok: g.act(0, Move::Paying(good, Box::new(spell))));
        cite!(["106.6"]; assert_eq!(g.players[0].pool.restricted.len(), 4));
    }
}
