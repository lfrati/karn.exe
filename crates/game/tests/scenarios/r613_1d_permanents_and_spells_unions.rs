//! A static ability said of a union of permanents and spells, each arm qualified on its own
//! (109.1): it applies to each object of either arm where that object is. Secret Arcade's
//! type change (205.1b, 613.1d) reaches permanent spells on the stack as well as nonland
//! permanents, and Dusty Parlor sees such a creature spell as an enchantment spell; Lo and
//! Li's lifelink reaches a Lesson spell as it deals damage (702.15b) as well as Noble
//! creatures.
use crate::support::{cite, paid};
use game::card::CastAs;
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::{CardType, Part};

/// Oracle cards by name, with ten Wastes in each library, in player 0's precombat main phase.
fn game(names: &[&str]) -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let defs: Vec<_> = names.iter().chain(["Wastes"].iter()).map(|n| {
        let d = CardDef::from_faces(&catalog[*n]).unwrap_or_else(|e| panic!("{n}: {e:?}"));
        game::play::check(&d).unwrap_or_else(|e| panic!("{n}: {}", e.0));
        d
    }).collect();
    let wastes = defs.len() - 1;
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, wastes, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn mana(g: &mut Game, p: usize) { g.players[p].pool.colors = [9; 5]; g.players[p].pool.colorless = 9; }
/// Resolve the stack, aiming any trigger at `at`.
fn resolve(g: &mut Game, at: Option<ObjId>) {
    for _ in 0..30 {
        match g.decision.clone() {
            None if g.stack.is_empty() => return,
            None => g.act(g.priority, Move::Pass).unwrap(),
            Some(Decision::Targets { player, .. }) => g.act(player, Move::ChooseTargets(vec![at.map(Target::Object).into_iter().collect()])).unwrap(),
            Some(d) => panic!("{d:?}"),
        }
    }
}

const ROOM: usize = 0;
const BEARS: usize = 1;
const FOREST: usize = 2;
const BOLT: usize = 3;

/// Secret Arcade: "Nonland permanents you control and permanent spells you control are
/// enchantments in addition to their other types." Its controller's creature is an
/// enchantment creature, their land and an opponent's creature are not; a creature spell they
/// cast is an enchantment spell on the stack, an instant spell is not. With Dusty Parlor
/// unlocked too, casting that creature spell is casting an enchantment spell.
#[test]
fn r613_1d_secret_arcade_makes_nonland_permanents_and_permanent_spells_enchantments() {
    let mut g = game(&["Secret Arcade // Dusty Parlor", "Grizzly Bears", "Forest", "Lightning Bolt"]);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let forest = g.put(0, FOREST, Zone::Battlefield);
    let theirs = g.put(1, BEARS, Zone::Battlefield);
    cite!(["613.1d"]; assert!(!g.is_a(bears, CardType::Enchantment), "not before the door is unlocked"));
    let card = g.put(0, ROOM, Zone::Hand);
    mana(&mut g, 0);
    g.act(0, paid("CCCCW", Move::CastAs(CastAs::Half(0), Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    resolve(&mut g, None);
    let room = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(ROOM)).unwrap();
    cite!(["205.1b", "613.1d"]; assert!(g.is_a(bears, CardType::Enchantment) && g.is_a(bears, CardType::Creature)));
    cite!(["613.1d"]; assert!(!g.is_a(forest, CardType::Enchantment), "a land is no nonland permanent"));
    cite!(["613.1d"]; assert!(!g.is_a(theirs, CardType::Enchantment), "an opponent's creature is not theirs to change"));

    // a permanent spell on the stack is an enchantment spell; an instant spell is not
    let spell = g.put(0, BEARS, Zone::Hand);
    mana(&mut g, 0);
    g.act(0, paid("CG", Move::Cast { card: spell, targets: vec![] })).unwrap();
    let on_stack = *g.stack.last().unwrap();
    cite!(["205.1b", "613.1d"]; assert!(g.is_a(on_stack, CardType::Enchantment) && g.is_a(on_stack, CardType::Creature)));
    let bolt = g.put(0, BOLT, Zone::Hand);
    g.act(0, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Object(theirs)]] })).unwrap();
    let bolt_on_stack = *g.stack.last().unwrap();
    cite!(["613.1d"]; assert!(!g.is_a(bolt_on_stack, CardType::Enchantment), "an instant spell is no permanent spell"));
    resolve(&mut g, None);

    // Dusty Parlor: "Whenever you cast an enchantment spell, put a number of +1/+1 counters
    // equal to that spell's mana value on up to one target creature."
    mana(&mut g, 0);
    g.act(0, paid("CCW", Move::Unlock { room, half: 1 })).unwrap();
    let another = g.put(0, BEARS, Zone::Hand);
    mana(&mut g, 0);
    g.act(0, paid("CG", Move::Cast { card: another, targets: vec![] })).unwrap();
    resolve(&mut g, Some(bears));
    cite!(["205.1b", "603.2"]; assert_eq!(g.obj(bears).counters.get("+1/+1").copied().unwrap_or(0), 2));
}

const LO_AND_LI: usize = 0;
const NOBLE: usize = 1;
const LESSON: usize = 2;
const BEAR: usize = 3;
const SHOCK: usize = 4;

/// Lo and Li: "Noble creatures you control and Lesson spells you control have lifelink." A
/// Noble creature its controller controls has lifelink, another creature and an opponent's
/// Noble don't; a Lesson spell its controller casts gains them life as it deals damage, an
/// instant that is no Lesson and an opponent's Lesson don't.
#[test]
fn r702_15b_lo_and_li_give_lifelink_to_noble_creatures_and_lesson_spells() {
    let mut g = game(&["Lo and Li, Twin Tutors", "Vampire Noble", "Firebending Lesson", "Grizzly Bears", "Shock"]);
    g.put(0, LO_AND_LI, Zone::Battlefield);
    let noble = g.put(0, NOBLE, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let their_noble = g.put(1, NOBLE, Zone::Battlefield);
    let (a, b) = (g.put(1, BEAR, Zone::Battlefield), g.put(1, BEAR, Zone::Battlefield));
    cite!(["702.15a"]; assert!(g.has(noble, "Lifelink") && !g.has(bear, "Lifelink") && !g.has(their_noble, "Lifelink")));

    let lesson = g.put(0, LESSON, Zone::Hand);
    mana(&mut g, 0);
    g.act(0, paid("R", Move::Cast { card: lesson, targets: vec![vec![Target::Object(a)]] })).unwrap();
    resolve(&mut g, None);
    cite!(["702.15b"]; assert_eq!(g.players[0].life, 22));
    let shock = g.put(0, SHOCK, Zone::Hand);
    g.act(0, paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Object(b)]] })).unwrap();
    resolve(&mut g, None);
    cite!(["702.15b"]; assert_eq!(g.players[0].life, 22, "Shock is no Lesson"));

    // an opponent's Lesson, cast in response to player 0's Shock, is not "you control"
    let shock = g.put(0, SHOCK, Zone::Hand);
    mana(&mut g, 0);
    g.act(0, paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Player(1)]] })).unwrap();
    g.act(0, Move::Pass).unwrap();
    let theirs = g.put(1, LESSON, Zone::Hand);
    mana(&mut g, 1);
    g.act(1, paid("R", Move::Cast { card: theirs, targets: vec![vec![Target::Object(bear)]] })).unwrap();
    resolve(&mut g, None);
    cite!(["702.15b"]; assert_eq!((g.players[0].life, g.players[1].life), (22, 18)));
}
