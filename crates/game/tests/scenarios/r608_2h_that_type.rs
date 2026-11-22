//! "That type" reads the prior scalar choice, using the same binding as "chosen type".
use crate::support::{cite, oracle, mana_pool, paid};
use game::{Game, Move, Zone};
use game::state::Answer;

#[test]
fn r608_2h_distant_melody_counts_only_permanents_of_the_type_chosen() {
    let mut g = Game::new(vec![oracle("Distant Melody"), oracle("Llanowar Elves"), oracle("Grizzly Bears"), oracle("Island")], 2, 20);
    g.put(0, 1, Zone::Battlefield);
    g.put(0, 1, Zone::Battlefield);
    g.put(0, 2, Zone::Battlefield);
    g.put(1, 1, Zone::Battlefield);
    for _ in 0..4 { g.put(0, 3, Zone::Library); }
    let spell = g.put(0, 0, Zone::Hand);
    g.players[0].pool = mana_pool("CCCU");
    g.act(0, paid("CCCU", Move::Cast { card: spell, targets: vec![] })).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    let mut fork = g.clone();
    for (g, name, count) in [(&mut g, "Elf", 2), (&mut fork, "Bear", 1)] {
        g.act(0, Move::Choose(Answer::Value(game::binding::Value::CreatureType(ir::Subtype::from_name(name).unwrap())))).unwrap();
        cite!(["608.2h"]; assert_eq!(g.players[0].hand.len(), count));
        cite!(["608.2c"]; assert_eq!(g.players[0].library.len(), 4 - count));
    }
}

#[test]
fn that_type_with_two_compatible_choices_remains_refused() {
    let def = game::CardDef::new("Ambiguous type", "{0}", "Sorcery",
        "Choose a creature type. Choose a card type. Draw a card for each permanent of that type.", None).unwrap();
    // Uncited (admission): a type reference cannot choose one of two compatible producers.
    crate::support::refused(game::play::check(&def).map_err(|e| e.0),
        "choice reference: ambiguous local or linked producers");
}
