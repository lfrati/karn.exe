//! A described token's keyword that is more than a flag survives a copy
//! (707.2, 111.3), and a keyword the engine doesn't follow is still refused.
use crate::support::{cite, paid};
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};

fn real(name: &str) -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let d = CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
    d
}
fn def(name: &str, cost: &str, types: &str, pt: Option<(i32, i32)>) -> CardDef { CardDef::new(name, cost, types, "", pt).unwrap() }
fn resolve(g: &mut Game) {
    for _ in 0..20 {
        if g.stack.is_empty() && g.decision.is_none() { return; }
        match g.decision.clone() {
            None => g.act(g.priority, Move::Pass).unwrap(),
            Some(Decision::Order { player, abilities }) => g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(),
            Some(d) => panic!("{d:?}"),
        }
    }
}
fn tokens(g: &Game, name: &str) -> Vec<ObjId> { g.battlefield.iter().copied().filter(|o| g.obj(*o).card.is_none() && g.obj(*o).chars.name == name).collect() }

/// Cackling Counterpart copies a Monk token with prowess: the copy has the original's
/// copiable values as its creating effect stated them, prowess included (707.2, 111.3), and
/// it triggers on the next noncreature spell.
#[test]
fn r707_2_a_copy_of_a_monk_token_has_prowess_and_it_triggers() {
    let mut g = Game::new(vec![real("Monastery Mentor"), def("Ornament", "{0}", "Artifact", None), real("Cackling Counterpart")], 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, 1, Zone::Library); } }
    g.put(0, 0, Zone::Battlefield);
    let card = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    resolve(&mut g);
    let [monk] = tokens(&g, "Monk Token")[..] else { panic!() };
    g.players[0].pool.colors = [5; 5];
    let counterpart = g.put(0, 2, Zone::Hand);
    g.act(0, paid("UUU", Move::Cast { card: counterpart, targets: vec![vec![Target::Object(monk)]] })).unwrap();
    resolve(&mut g);
    let copy = *tokens(&g, "Monk Token").iter().find(|o| **o != monk && g.power(**o) == Some(1)).expect("the copy");
    cite!(["707.2", "111.3"]; assert!(g.has(copy, "Prowess")));
    let card = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    resolve(&mut g);
    cite!(["702.108a"]; assert_eq!(g.power(copy), Some(2), "the copy's prowess triggered"));
}

/// A token "with" a keyword the engine doesn't follow is refused, not created without it;
/// one it follows on a card (prowess, landwalk) is admitted.
#[test]
fn r111_3_a_token_with_an_unfollowed_keyword_is_still_refused() {
    for keyword in ["prowess", "mountainwalk", "banding", "rampage 1", "bushido 1", "phasing", "flanking", "horsemanship"] {
        let text = format!("When this creature enters, create a 1/1 white Soldier creature token with {keyword}.");
        let Ok(d) = CardDef::new("Maker", "{0}", "Creature — Human", &text, Some((1, 1))) else { continue };
        let checked = game::play::check(&d);
        let follows = CardDef::new("Holder", "{0}", "Creature — Human", keyword, Some((1, 1))).ok().is_some_and(|h| game::play::check(&h).is_ok());
        eprintln!("{keyword}: token {:?}, printed {follows}", checked.as_ref().err().map(|e| &e.0));
        assert_eq!(checked.is_ok(), follows, "{keyword}: a token is admitted with a keyword exactly when a card printed with it is");
    }
}
