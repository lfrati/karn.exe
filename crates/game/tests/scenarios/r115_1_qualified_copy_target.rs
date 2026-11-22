//! Daretti's one artifact target can be a permanent or a card in any graveyard.
use crate::support::cite;
use game::{CardDef, Game, Move, Target, Zone};

#[test]
fn r115_1c_daretti_copies_one_artifact_from_either_zone_without_copying_state() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let defs = ["Daretti, Ingenious Iconoclast", "Ornithopter", "Grizzly Bears"].map(|name| {
        let def = CardDef::from_faces(&catalog[name]).unwrap();
        game::play::check(&def).unwrap_or_else(|error| panic!("{name}: {}", error.0));
        def
    });
    for zone in [Zone::Battlefield, Zone::Graveyard] {
        let mut g = Game::new(defs.to_vec(), 2, 20);
        let daretti = g.put(0, 0, Zone::Battlefield);
        // Trusted setup skips entry actions, including the printed loyalty counters.
        g.put_counters(daretti, "loyalty", 7);
        let artifact = g.put(1, 1, zone);
        let nonartifact = g.put(1, 2, zone);
        if zone == Zone::Battlefield {
            g.set_tapped(artifact, true);
            g.put_counters(artifact, "+1/+1", 2);
        }
        let activate = |target| Move::Activate {
            source: daretti, ability: 2, targets: vec![vec![Target::Object(target)]], paying: vec![],
        };
        cite!(["115.1c", "602.2b"]; case: format!("{zone:?}");
            refused("an illegal target, Grizzly Bears:"): g.act(0, activate(nonartifact)));
        assert_eq!(g.obj(daretti).counters.get("loyalty"), Some(&7));
        g.act(0, activate(artifact)).unwrap();
        cite!(["606.4"]; case: format!("{zone:?}"); assert_eq!(g.obj(daretti).counters.get("loyalty"), Some(&1)));
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        assert!(g.stack.is_empty());
        assert_eq!(g.obj(artifact).zone, zone, "copying does not move the original");
        let copies: Vec<_> = g.battlefield.iter().copied().filter(|o| g.obj(*o).token()).collect();
        cite!(["707.2"]; case: format!("{zone:?}"); assert_eq!(copies.len(), 3));
        for token in copies {
            cite!(["707.2"]; case: format!("{zone:?}"); assert_eq!(g.obj(token).chars.name, "Ornithopter"));
            cite!(["707.2"]; case: format!("{zone:?}"); assert_eq!((g.power(token), g.toughness(token)), (Some(0), Some(2))));
            cite!(["707.2"]; case: format!("{zone:?}"); assert!(!g.obj(token).tapped));
            assert!(g.has(token, "Flying"));
            assert_eq!((g.obj(token).owner, g.obj(token).controller), (0, 0));
        }
    }
}
