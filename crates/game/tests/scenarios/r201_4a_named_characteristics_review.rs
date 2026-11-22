//! Named characteristics and spell-qualified cast prohibitions: an adventurer
//! card's two names (201.4f, 715.3), a modal double-faced card's land back face (201.4d), a
//! split card's halves by color (709.3a), Reflector Mage's "same name as that creature until
//! your next turn", Voidstone Gargoyle's activated abilities, and a card cast from exile.
use game::binding::Value;
use game::state::{Answer, Choice, Decision};
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const MAGE: usize = 0;
const GIANT: usize = 1;
const VALAKUT: usize = 2;
const IONA: usize = 3;
const WEAR_TEAR: usize = 4;
const REFLECTOR: usize = 5;
const BEARS: usize = 6;
const GARGOYLE: usize = 7;
const STONE: usize = 8;
const BOLT: usize = 9;
const PEEK: usize = 10;
const ISLAND: usize = 11;
const AURA: usize = 12;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let mut defs: Vec<_> = ["Meddling Mage", "Bonecrusher Giant // Stomp", "Bala Ged Recovery // Bala Ged Sanctuary",
        "Iona, Shield of Emeria", "Wear // Tear", "Reflector Mage", "Grizzly Bears", "Voidstone Gargoyle", "Mind Stone",
        "Lightning Bolt"].map(|n| CardDef::from_faces(&catalog[n]).unwrap_or_else(|e| panic!("{n}: {e:?}"))).to_vec();
    defs.push(CardDef::new("Peek", "{0}", "Sorcery", "Exile the top card of your library. You may cast it this turn.", None).unwrap());
    defs.push(CardDef::from_faces(&catalog["Island"]).unwrap());
    defs.push(CardDef::new("Aura", "{0}", "Enchantment", "", None).unwrap());
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn mana(g: &mut Game) { for p in 0..2 { g.players[p].pool.colors = [9; 5]; g.players[p].pool.colorless = 9; } }
fn settle(g: &mut Game) {
    for _ in 0..40 {
        match g.decision.clone() {
            None if g.stack.is_empty() => return,
            None => g.act(g.priority, Move::Pass).unwrap(),
            Some(Decision::Choose { player, choice: Choice::May { possible: true } }) => g.act(player, Move::Choose(Answer::Yes)).unwrap(),
            Some(d) => panic!("{d:?}"),
        }
    }
}
/// `card` (Meddling Mage, Voidstone Gargoyle or Iona) enters for player 0 and `answer` is chosen.
fn enter(g: &mut Game, card: usize, answer: Answer, units: &str) -> ObjId {
    let c = g.put(0, card, Zone::Hand);
    mana(g);
    g.act(0, paid(units, Move::Cast { card: c, targets: vec![] })).unwrap();
    while g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Choose(answer)).unwrap();
    settle(g);
    c
}
fn named(n: &str) -> Answer { Answer::Value(Value::CardName(n.into())) }
fn cast_as(card: ObjId, half: usize, targets: Vec<Vec<Target>>) -> Move { as_(game::card::CastAs::Half(half), card, targets) }
fn as_(how: game::card::CastAs, card: ObjId, targets: Vec<Vec<Target>>) -> Move {
    Move::CastAs(how, Box::new(Move::Cast { card, targets }))
}
/// Can `player` cast `card` as `half` now (on a cloned game)?
fn can(g: &Game, player: usize, m: Move) -> Result<(), String> { let mut h = g.clone(); mana(&mut h); h.act(player, m) }

/// "Stomp" is a nonland card name (201.4f); naming it prohibits the adventure, not the Giant.
#[test]
fn an_adventure_is_named_apart_from_its_creature() {
    for (name, giant_ok, stomp_ok) in [("Stomp", true, false), ("Bonecrusher Giant", false, true)] {
        let mut g = game();
        enter(&mut g, MAGE, named(name), "WU");
        let giant = g.put(0, GIANT, Zone::Hand);
        let r = can(&g, 0, paid("RRR", as_(game::card::CastAs::Whole, giant, vec![])));
        cite!(["201.4f", "715.4", "601.3"]; case: name; assert_eq!(r.is_ok(), giant_ok, "{name}: the Giant: {r:?}"));
        if !giant_ok { crate::support::refused(r.clone(), "an effect prohibits casting that spell (601.3)"); }
        let r = can(&g, 0, paid("RR", as_(game::card::CastAs::Adventure, giant, vec![vec![Target::Player(1)]])));
        cite!(["201.4f", "715.3a", "601.3"]; case: name; assert_eq!(r.is_ok(), stomp_ok, "{name}: Stomp: {r:?}"));
        if !stomp_ok { crate::support::refused(r, "an effect prohibits casting that spell (601.3)"); }
    }
}

/// A modal double-faced card's land back face is no nonland card name (201.4d, 201.4a); naming
/// its front prohibits casting it (601.3; in hand it has only its front face's characteristics,
/// 712.8a), not playing its back face as a land, which is no casting (305.1, 712.12).
#[test]
fn a_double_faced_cards_land_face_is_not_a_nonland_name() {
    let mut g = game();
    let c = g.put(0, MAGE, Zone::Hand);
    mana(&mut g);
    g.act(0, paid("WU", Move::Cast { card: c, targets: vec![] })).unwrap();
    while g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["201.4a", "201.4d"]; refused("choose the name of a card with the characteristics the choice says (201.4a)"): g.clone().act(0, Move::Choose(named("Bala Ged Sanctuary"))));
    g.act(0, Move::Choose(named("Bala Ged Recovery"))).unwrap();
    settle(&mut g);
    let valakut = g.put(0, VALAKUT, Zone::Hand);
    let dead = g.put(0, BOLT, Zone::Graveyard);
    cite!(["601.3", "712.8a"]; refused("prohibits casting that spell (601.3)"): can(&g, 0, paid("CCG", as_(game::card::CastAs::Face(0), valakut, vec![vec![Target::Object(dead)]]))));
    cite!(["305.1", "712.12"]; ok: g.act(0, Move::PlayLandAs { card: valakut, face: 1 }));
}

/// Iona naming white: an opponent may cast Wear (red), not Tear (white) (709.3a); Iona's
/// controller may cast either.
#[test]
fn iona_judges_a_split_cards_half_by_its_own_color() {
    let mut g = game();
    enter(&mut g, IONA, Answer::Value(Value::Color(ir::Color::White)), "CCCCCCWWW");
    let stone = g.put(0, STONE, Zone::Battlefield);
    let aura = g.put(0, AURA, Zone::Battlefield);
    let mine = g.put(0, WEAR_TEAR, Zone::Hand);
    let r = can(&g, 0, paid("W", cast_as(mine, 1, vec![vec![Target::Object(aura)]])));
    // Iona: "Your opponents can't cast spells of the chosen color"
    cite!(["601.3"]; assert!(r.is_ok(), "its controller is no opponent: {r:?}"));
    let theirs = g.put(1, WEAR_TEAR, Zone::Hand);
    g.act(0, Move::Pass).unwrap();
    cite!(["709.3a", "601.3"]; ok: can(&g, 1, paid("CR", cast_as(theirs, 0, vec![vec![Target::Object(stone)]]))));
    cite!(["709.3a", "601.3"]; refused("prohibits casting that spell (601.3)"): can(&g, 1, paid("W", cast_as(theirs, 1, vec![vec![Target::Object(aura)]]))));
}

/// Reflector Mage: the owner can't cast spells named as the creature until the Mage's
/// controller's next turn; another name, and afterwards that one, they may.
#[test]
fn reflector_mage_until_your_next_turn() {
    let mut g = game();
    let bears = g.put(1, BEARS, Zone::Battlefield);
    let reflector = g.put(0, REFLECTOR, Zone::Hand);
    mana(&mut g);
    g.act(0, paid("CWU", Move::Cast { card: reflector, targets: vec![] })).unwrap();
    while g.stack.len() < 2 && g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
    if let Some(Decision::Targets { player, .. }) = g.decision.clone() {
        g.act(player, Move::ChooseTargets(vec![vec![Target::Object(bears)]])).unwrap();
    }
    settle(&mut g);
    let back = *g.players[1].hand.iter().find(|o| g.obj(**o).card == Some(BEARS)).expect("bounced");
    let other = g.put(1, GIANT, Zone::Hand);
    let step = |g: &mut Game| match g.decision {
        Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
        Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
        _ => g.act(g.priority, Move::Pass).unwrap(),
    };
    while g.active == 0 { step(&mut g); }
    while g.part != ir::Part::PrecombatMain || g.priority != 1 { step(&mut g); }
    // Grizzly Bears, on its owner's turn
    cite!(["601.3", "611.2a"]; refused("prohibits casting that spell (601.3)"): can(&g, 1, paid("CG", Move::Cast { card: back, targets: vec![] })));
    cite!(["601.3"]; ok: can(&g, 1, paid("RRR", as_(game::card::CastAs::Whole, other, vec![]))));
    // through player 0's next turn, to player 1's turn after it
    while g.active == 1 { step(&mut g); }
    while g.active == 0 { step(&mut g); }
    while g.part != ir::Part::PrecombatMain || g.priority != 1 { step(&mut g); }
    // the Reflector's controller's next turn has come
    cite!(["611.2a"]; ok: can(&g, 1, paid("CG", Move::Cast { card: back, targets: vec![] })));
}

/// Voidstone Gargoyle naming Mind Stone: neither of its abilities can be activated (602.5), the
/// mana one included (605.3: activating a mana ability follows the rules for activating).
#[test]
fn voidstone_gargoyle_stops_activated_abilities_of_the_name() {
    let mut g = game();
    let stone = g.put(1, STONE, Zone::Battlefield);
    enter(&mut g, GARGOYLE, named("Mind Stone"), "CCCWW");
    g.act(0, Move::Pass).unwrap();
    let activate = |ability| can(&g, 1, paid(if ability == 0 { "" } else { "C" }, Move::Activate { source: stone, ability, targets: vec![], paying: vec![] }));
    cite!(["602.5", "605.3"]; case: "its mana ability"; refused("prohibits activating that ability (602.5)"): activate(0));
    cite!(["602.5"]; case: "its other ability"; refused("prohibits activating that ability (602.5)"): activate(1));
}

/// A named card exiled with "You may cast it this turn" still can't be cast (601.3): the
/// permission does not override the prohibition (101.2).
#[test]
fn a_permission_to_cast_from_exile_does_not_lift_the_prohibition() {
    let mut g = game();
    enter(&mut g, MAGE, named("Lightning Bolt"), "WU");
    let bolt = g.put(0, BOLT, Zone::Library);
    assert_eq!(g.players[0].library.last(), Some(&bolt), "on top");
    let peek = g.put(0, PEEK, Zone::Hand);
    g.act(0, Move::Cast { card: peek, targets: vec![] }).unwrap();
    settle(&mut g);
    let exiled = *g.exile.iter().find(|o| g.obj(**o).card == Some(BOLT)).expect("exiled");
    cite!(["601.3", "101.2"]; refused("prohibits casting that spell (601.3)"): can(&g, 0, paid("R", Move::Cast { card: exiled, targets: vec![vec![Target::Player(1)]] })));
}
