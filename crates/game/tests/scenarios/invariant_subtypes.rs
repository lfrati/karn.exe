//! A subtype named in the plural where English spells it like the singular ("all creatures and
//! Spacecraft", 205.3g) picks out the permanents of that subtype, through real card text.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Zone};

#[test]
fn r205_3g_beyond_the_quiet_exiles_creatures_and_spacecraft_and_nothing_else() {
    let defs = vec![
        CardDef::new("Beyond the Quiet", "{3}{W}{W}", "Sorcery", "Exile all creatures and Spacecraft.", None).unwrap(),
        CardDef::new("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Hulk", "{3}", "Artifact — Spacecraft", "", None).unwrap(),
        CardDef::new("Ornithopter Kit", "{1}", "Artifact", "", None).unwrap(),
        CardDef::new("Plains", "", "Basic Land — Plains", "({T}: Add {W}.)", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap();
    }
    let mut g = Game::new(defs, 3, 20);
    let bears = g.put(1, 1, Zone::Battlefield);
    let hulk = g.put(1, 2, Zone::Battlefield);
    let kit = g.put(1, 3, Zone::Battlefield);
    let land = g.put(0, 4, Zone::Battlefield);
    for _ in 0..5 {
        g.add_mana(0, ir::mana::ManaSymbol::Color(ir::Color::White));
    }
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, paid("WWWWW", Move::Cast { card, targets: vec![] })).unwrap();
    for _ in 0..3 {
        g.act(g.priority, Move::Pass).unwrap();
    }
    assert_eq!(g.obj(bears).zone, Zone::Gone);
    assert_eq!(g.obj(hulk).zone, Zone::Gone);
    cite!(["205.3g"]; assert_eq!(g.battlefield, vec![kit, land]));
    assert_eq!(g.exile.len(), 2);
}
