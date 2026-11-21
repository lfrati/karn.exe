//! Mechanism fixtures below deliberately simplify historical Werewolves; their names
//! are distinct from Oracle cards and establish no complete-card support claim.
//! Transform (701.27) and double-faced cards (712): a permanent's other face comes up, it is
//! the same object with that face's characteristics and abilities, and it is its front face
//! again anywhere else.
use crate::support::cite;
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const RANGER: usize = 0;
const HUNTMASTER: usize = 1;
const SIP: usize = 2;
const LAND: usize = 3;
const TURNER: usize = 4;
const BEAR: usize = 5;
const UNSUMMON: usize = 6;
const TWICE: usize = 7;

fn defs() -> Vec<CardDef> {
    let werewolf = |name: &str, cost: &str, text: &str, night: &str, night_text: &str| {
        let front = CardDef::new(name, cost, "Creature — Human Werewolf", text, Some((2, 2))).unwrap();
        let back = CardDef::new(night, "", "Creature — Werewolf", night_text, Some((4, 4))).unwrap();
        CardDef::transforming(front, back).unwrap()
    };
    vec![
        werewolf("Test Ranger", "{2}{G}",
            "{T}: This creature deals 2 damage to target creature with flying.\nAt the beginning of each upkeep, if no spells were cast last turn, transform this creature.",
            "Test Predator", "Trample\nAt the beginning of each upkeep, if a player cast two or more spells last turn, transform this creature."),
        werewolf("Test Huntmaster", "{2}{R}{G}",
            "Whenever this creature enters or transforms into Test Huntmaster, create a 2/2 green Wolf creature token and you gain 2 life.\nAt the beginning of each upkeep, if no spells were cast last turn, transform this creature.",
            "Test Ravager", "Trample\nWhenever this creature transforms into Test Ravager, you lose 1 life.\nAt the beginning of each upkeep, if a player cast two or more spells last turn, transform this creature."),
        CardDef::new("Sip", "{G}", "Instant", "Target player draws a card.", None).unwrap(),
        CardDef::new("Wastes", "", "Land", "", None).unwrap(),
        CardDef::new("Turner", "{U}", "Instant", "Transform target creature.", None).unwrap(),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Unsummon", "{U}", "Instant", "Return target creature to its owner's hand.", None).unwrap(),
        werewolf("Twice", "{G}", "{0}: Transform this creature.", "Twice Over", "{0}: Transform this creature."),
    ]
}
fn game() -> Game {
    let defs = defs();
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    let mut g = Game::new(defs, 2, 20);
    for player in 0..2 { for _ in 0..12 { g.put(player, LAND, Zone::Library); } }
    g
}
/// This player casts this card with mana given for it, and everything resolves.
fn cast(g: &mut Game, player: usize, card: usize, targets: Vec<Vec<Target>>) {
    let spell = g.put(player, card, Zone::Hand);
    g.players[player].pool.colors = [9, 9, 9, 9, 9];
    while g.priority != player { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(player, crate::support::paid(["WWG", "WWRG", "G", "", "U", "WG", "U", "G"][card], Move::Cast { card: spell, targets })).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    g.players[player].pool = Default::default();
}
/// To the next turn's first main phase with an empty stack.
fn next_turn(g: &mut Game) {
    let turn = g.turn;
    while g.turn == turn || g.part != ir::Part::PrecombatMain || !g.stack.is_empty() {
        match g.asked() {
            Some((player, game::legal::Asked::Priority)) => g.act(player, Move::Pass).unwrap(),
            Some((player, game::legal::Asked::Attackers(_))) => g.act(player, Move::Attack(vec![])).unwrap(),
            other => panic!("unexpected next-turn boundary: {other:?}"),
        }
    }
}
fn wolves(g: &Game) -> usize {
    g.battlefield.iter().filter(|o| g.obj(**o).chars.name == "Wolf Token").count()
}
fn name(g: &Game, o: ObjId) -> &str { &g.obj(o).chars.name }

#[test]
fn r701_27a_the_other_face_comes_up_and_it_is_the_same_object() {
    let mut g = game();
    let ranger = g.put(0, RANGER, Zone::Battlefield);
    g.put_counters(ranger, "+1/+1", 1);
    g.tap(ranger);
    cast(&mut g, 0, TURNER, vec![vec![Target::Object(ranger)]]);
    // the same object, with what applied to it (712.18), and only its back face's
    // characteristics (712.8e)
    cite!(["712.18"]; assert!(g.battlefield.contains(&ranger) && g.obj(ranger).transformed));
    cite!(["701.27a", "712.8e"]; assert_eq!(name(&g, ranger), "Test Predator"));
    cite!(["712.18", "712.8e"]; assert_eq!((g.power(ranger), g.toughness(ranger)), (Some(5), Some(5)), "4/4 with its counter"));
    cite!(["712.18"]; assert!(g.obj(ranger).tapped));
    cite!(["712.8e"]; assert!(g.has(ranger, "Trample")));
    assert_eq!(g.abilities(ranger).len(), g.def(ranger).unwrap().abilities().len());
    assert_eq!(g.def(ranger).unwrap().name(), "Test Predator");
    // its mana value is its front face's (202.3b)
    cite!(["202.3b"]; assert_eq!(g.obj(ranger).chars.mana_value, 3));
    // and back again
    cast(&mut g, 0, TURNER, vec![vec![Target::Object(ranger)]]);
    assert!(!g.obj(ranger).transformed);
    cite!(["712.8d"]; assert_eq!(name(&g, ranger), "Test Ranger"));
    cite!(["712.8d"]; assert_eq!((g.power(ranger), g.toughness(ranger)), (Some(3), Some(3))));
    cite!(["712.8d"]; assert!(!g.has(ranger, "Trample")));
}

#[test]
fn r701_27c_what_is_not_double_faced_does_not_transform() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    cast(&mut g, 0, TURNER, vec![vec![Target::Object(bear)]]);
    cite!(["701.27c"]; assert!(!g.obj(bear).transformed));
    cite!(["701.27c"]; assert_eq!(name(&g, bear), "Bear"));
}

#[test]
fn r712_8a_it_is_its_front_face_anywhere_else() {
    let mut g = game();
    let ranger = g.put(0, RANGER, Zone::Battlefield);
    cast(&mut g, 0, TURNER, vec![vec![Target::Object(ranger)]]);
    cast(&mut g, 0, UNSUMMON, vec![vec![Target::Object(ranger)]]);
    let [card] = g.players[0].hand[..] else { panic!("not in hand") };
    cite!(["712.8a"]; assert_eq!(name(&g, card), "Test Ranger"));
    cite!(["712.8a"]; assert!(!g.obj(card).transformed));
    // cast again, it enters with its front face up (712.13)
    g.players[0].pool.colors = [9, 9, 9, 9, 9];
    while g.priority != 0 { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, crate::support::paid("WWG", Move::Cast { card, targets: vec![] })).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    let [ranger] = g.battlefield[..] else { panic!("not on the battlefield") };
    cite!(["712.8d"]; assert_eq!(name(&g, ranger), "Test Ranger"));
    cite!(["712.8d"]; assert_eq!((g.power(ranger), g.toughness(ranger)), (Some(2), Some(2))));
}

#[test]
fn werewolves_turn_with_the_spells_of_the_turn_before() {
    let mut g = game();
    let ranger = g.put(0, RANGER, Zone::Battlefield);
    cast(&mut g, 0, SIP, vec![vec![Target::Player(0)]]);
    next_turn(&mut g);
    cite!(["603.4"]; assert!(!g.obj(ranger).transformed, "a spell was cast last turn"));
    next_turn(&mut g);
    cite!(["603.4", "701.27a"]; assert!(g.obj(ranger).transformed, "no spells were cast last turn"));
    cast(&mut g, 0, SIP, vec![vec![Target::Player(0)]]);
    cast(&mut g, 1, SIP, vec![vec![Target::Player(1)]]);
    next_turn(&mut g);
    cite!(["603.4"]; assert!(g.obj(ranger).transformed, "the night face asks for two spells by one player"));
    cast(&mut g, 1, SIP, vec![vec![Target::Player(1)]]);
    cast(&mut g, 1, SIP, vec![vec![Target::Player(1)]]);
    next_turn(&mut g);
    cite!(["603.4"]; assert!(!g.obj(ranger).transformed));
    assert_eq!(name(&g, ranger), "Test Ranger");
}

#[test]
fn r701_27e_transforms_into_is_said_by_the_face_that_came_up() {
    let mut g = game();
    cast(&mut g, 0, HUNTMASTER, vec![]);
    let hunt = g.battlefield.iter().copied().find(|o| name(&g, *o) == "Test Huntmaster").unwrap();
    cite!(["603.2"]; assert_eq!((wolves(&g), g.players[0].life), (1, 22), "it entered"));
    cast(&mut g, 0, TURNER, vec![vec![Target::Object(hunt)]]);
    assert_eq!(name(&g, hunt), "Test Ravager");
    cite!(["701.27e"]; assert_eq!((wolves(&g), g.players[0].life), (1, 21), "Ravager's ability, not Huntmaster's"));
    cast(&mut g, 0, TURNER, vec![vec![Target::Object(hunt)]]);
    cite!(["701.27e"]; assert_eq!((wolves(&g), g.players[0].life), (2, 23), "it transformed into Test Huntmaster"));
}

#[test]
fn r701_27f_an_ability_of_the_permanent_does_not_transform_it_twice() {
    let mut g = game();
    let twice = g.put(0, TWICE, Zone::Battlefield);
    let turn = Move::Activate { source: twice, ability: 0, targets: vec![], paying: vec![] };
    // two activations on the stack: the second to resolve finds it has transformed since
    g.act(0, turn.clone()).unwrap();
    g.act(0, turn.clone()).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["701.27f"]; assert!(g.obj(twice).transformed, "once, not there and back"));
    // an ability put on the stack afterwards transforms it again
    g.act(0, turn).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["701.27f"]; assert!(!g.obj(twice).transformed));
}
