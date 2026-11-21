//! A spell's instructions said in several paragraphs are followed in order as one text
//! (608.2c): a later paragraph can name what an earlier one did ("that creature"), and a
//! reference a paragraph makes to a group it produced itself is still its own. Every card is
//! read from its Oracle text.
use crate::support::{cite, paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const URGE: usize = 0;
const SPOILS: usize = 1;
const BEARS: usize = 2;
const SWAMP: usize = 3;
const BOLT: usize = 4;
const DIVINATION: usize = 5;
const MAGUS: usize = 6;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let defs: Vec<_> = ["Violent Urge", "Liliana's Spoils", "Grizzly Bears", "Swamp", "Lightning Bolt", "Divination", "Vampire Nighthawk"]
        .map(|n| CardDef::from_faces(&catalog[n]).unwrap()).to_vec();
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}

fn cast(g: &mut Game, card: usize, target: Target, spending: &str) {
    let c = g.put(0, card, Zone::Hand);
    g.players[0].pool.colors = [0, 0, 5, 5, 0];
    g.act(0, paid(spending, Move::Cast { card: c, targets: vec![vec![target]] })).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
}

#[test]
fn r608_2c_violent_urge_the_second_paragraph_names_the_first_ones_target() {
    // "Target creature gets +1/+0 and gains first strike until end of turn.
    // Delirium — If there are four or more card types among cards in your graveyard, that
    // creature gains double strike until end of turn."
    for delirium in [false, true] {
        let mut g = game();
        let bears = g.put(0, BEARS, Zone::Battlefield);
        let other = g.put(0, BEARS, Zone::Battlefield);
        if delirium { for card in [BEARS, SWAMP, BOLT, DIVINATION] { g.put(0, card, Zone::Graveyard); } }
        cast(&mut g, URGE, Target::Object(bears), "R");
        cite!(["608.2c"]; case: format!("{delirium}"); assert_eq!((g.power(bears), g.has(bears, "First strike")), (Some(3), true)));
        cite!(["608.2c"]; case: format!("{delirium}"); assert_eq!(g.has(bears, "Double strike"), delirium, "that creature: the first paragraph's target"));
        assert!(!g.has(other, "Double strike"));
    }
}

#[test]
fn r608_2c_liliana_s_spoils_them_is_what_its_own_paragraph_looked_at() {
    // "Target opponent discards a card.
    // Look at the top five cards of your library. You may reveal a black card from among them
    // and put it into your hand. Put the rest on the bottom of your library in a random order."
    let mut g = game();
    let discarded = [g.put(1, MAGUS, Zone::Hand), g.put(1, SWAMP, Zone::Hand)];
    for _ in 0..2 { g.put(0, SWAMP, Zone::Library); }
    let top: Vec<ObjId> = [SPOILS, MAGUS, SWAMP, BOLT, DIVINATION].map(|c| g.put(0, c, Zone::Library)).to_vec();
    cast(&mut g, SPOILS, Target::Player(1), "BBBB");
    let Some(Decision::Choose { player: 1, .. }) = g.decision.clone() else { panic!("the opponent discards: {:?}", g.decision) };
    g.act(1, Move::Choose(Answer::Objects(vec![discarded[0]]))).unwrap();
    let Some(Decision::Choose { player: 0, choice: Choice::May }) = g.decision.clone() else { panic!("you may reveal: {:?}", g.decision) };
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    // the black card among the five looked at, not the discarded one
    let mut among = match g.decision.clone() {
        Some(Decision::Choose { player: 0, choice: Choice::Objects { among, .. } | Choice::Select { among, .. } }) => among,
        other => panic!("the controller chooses among the five: {other:?}"),
    };
    among.sort();
    cite!(["608.2c"]; assert_eq!(among, vec![top[0], top[1]], "the black cards of the five"));
    g.act(0, Move::Choose(Answer::Objects(vec![top[1]]))).unwrap();
    let names: Vec<_> = g.players[0].hand.iter().map(|c| g.obj(*c).chars.name.as_str()).collect();
    cite!(["608.2c"]; assert_eq!(names, ["Vampire Nighthawk"]));
    let library = &g.players[0].library;
    cite!(["608.2c"]; assert_eq!(library.len(), 6, "the rest stay in the library"));
    let mut bottom: Vec<_> = library[..4].iter().map(|c| g.obj(*c).chars.name.as_str()).collect();
    bottom.sort();
    cite!(["608.2c"]; assert_eq!(bottom, ["Divination", "Lightning Bolt", "Liliana's Spoils", "Swamp"], "the rest, under the cards not looked at"));
    assert_eq!(g.players[1].hand, vec![discarded[1]]);
}

#[test]
fn r709_3_a_split_cards_halves_are_not_one_text() {
    // Consecrate // Consume: "Exile target card from a graveyard. / Draw a card." and "Target
    // player sacrifices a creature …": each half is its own spell (709.3), so the whole card's
    // definition (a fused spell's, 702.102) does not read Consume as Consecrate's third
    // paragraph. For every split card: each half's instructions begin their own program in the
    // whole definition, compiled there when they are in the half.
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let whole = CardDef::from_faces(&catalog["Consecrate // Consume"]).unwrap();
    cite!(["709.3", "608.2c"]; assert_eq!(whole.compiled_abilities(), vec![0, 2], "Consecrate's two paragraphs, then Consume"));
    let mut checked = 0;
    for faces in catalog.values() {
        // Select the split-card corpus before parsing unrelated card text.
        if !faces.iter().any(|face| face.layout == "split") { continue; }
        let Ok(whole) = CardDef::from_faces(faces) else { continue };
        let [left, right] = whole.halves() else { continue };
        let compiled = whole.compiled_abilities();
        for (half, start) in [(left, 0), (right, left.abilities().len())] {
            if half.compiled_abilities().first() == Some(&0) {
                cite!(["709.3"]; assert!(compiled.contains(&start), "{}: {} begins its own program", whole.name(), half.name()));
            }
        }
        checked += 1;
    }
    assert!(checked > 100, "{checked} split cards");
}
