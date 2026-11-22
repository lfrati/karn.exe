//! The newly readable bounce alternatives share one target, across stack and battlefield.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Target, Zone};

fn cast(card: game::ObjId, targets: Vec<Target>, mode: Option<usize>, mana: &str) -> Move {
    let action = Move::Cast { card, targets: vec![targets] };
    paid(mana, match mode {
        Some(mode) => Move::Modes(vec![mode], Box::new(action)),
        None => action,
    })
}

#[test]
fn r115_1a_real_bounce_spells_accept_either_domain_and_reject_other_objects() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let names = ["Unsubstantiate", "Brutal Expulsion", "Fatehold Charm", "Ornithopter", "Lightning Bolt", "Sol Ring"];
    let defs = names.map(|name| {
        let def = CardDef::from_faces(&catalog[name]).unwrap();
        game::play::check(&def).unwrap_or_else(|error| panic!("{name}: {}", error.0));
        def
    });
    for (bounce, mode, mana) in [(0, None, "WU"), (1, Some(0), "WWUR"), (2, Some(1), "WU")] {
        for domain in ["permanent", "creature spell", "noncreature spell"] {
            let mut g = Game::new(defs.to_vec(), 2, 20);
            for player in &mut g.players { player.pool.colors = [20; 5]; }
            let (target, owner, definition) = match domain {
                "permanent" => (g.put(1, 3, Zone::Battlefield), 1, 3),
                "creature spell" => {
                    let card = g.put(0, 3, Zone::Hand);
                    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
                    (*g.stack.last().unwrap(), 0, 3)
                }
                _ => {
                    g.act(0, Move::Pass).unwrap();
                    let card = g.put(1, 4, Zone::Hand);
                    g.act(1, paid("R", Move::Cast { card, targets: vec![vec![Target::Player(0)]] })).unwrap();
                    g.act(1, Move::Pass).unwrap();
                    (*g.stack.last().unwrap(), 1, 4)
                }
            };
            let noncreature = g.put(1, 5, Zone::Battlefield);
            let graveyard_card = g.put(1, 3, Zone::Graveyard);
            let card = g.put(0, bounce, Zone::Hand);
            for invalid in [Target::Object(noncreature), Target::Object(graveyard_card), Target::Player(1)] {
                cite!(["115.1a", "601.2c"]; case: format!("{}: {domain}", names[bounce]);
                    refused("an illegal target,"): g.act(0, cast(card, vec![invalid], mode, mana)));
            }
            cite!(["601.2c"]; case: names[bounce]; refused("wrong number of targets: 2 chosen, it takes 1"):
                g.act(0, cast(card, vec![Target::Object(target), Target::Object(target)], mode, mana)));
            g.act(0, cast(card, vec![Target::Object(target)], mode, mana)).unwrap();
            for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
            cite!(["400.3", "608.2c"]; case: format!("{}: {domain}", names[bounce]);
                assert!(g.players[owner].hand.iter().any(|o| g.obj(*o).card == Some(definition))));
            assert!(g.stack.is_empty(), "{}: {domain}", names[bounce]);
            assert!(g.players.iter().all(|p| p.life == 20), "{}: {domain}", names[bounce]);
            assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).card == Some(bounce)));
        }
    }
}
