//! State triggers (603.8): an ability that triggers when the game is a certain way rather
//! than when something happens. It triggers as soon as that is so, and not again until it
//! has left the stack.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const GARRUK: usize = 0;
const BEAR: usize = 1;
const SERPENT: usize = 2;
const ISLAND: usize = 3;
const STONE_RAIN: usize = 4;
const OGRE: usize = 5;
const SENTRY: usize = 6;
const CURSE: usize = 7;

const RELENTLESS: &str = "When Garruk has two or fewer loyalty counters on him, transform him.\n0: Garruk deals 3 damage to target creature. That creature deals damage equal to its power to him.\n0: Create a 2/2 green Wolf creature token.";
const VEIL_CURSED: &str = "+1: Create a 1/1 black Wolf creature token with deathtouch.\n−1: Sacrifice a creature. If you do, search your library for a creature card, reveal it, put it into your hand, then shuffle.\n−3: Creatures you control gain trample and get +X/+X until end of turn, where X is the number of creature cards in your graveyard.";

fn game() -> Game {
    let front = CardDef::planeswalker("Garruk Relentless", "{3}{G}", "Legendary Planeswalker — Garruk", RELENTLESS, 3).unwrap();
    let back = CardDef::new("Garruk, the Veil-Cursed", "", "Legendary Planeswalker — Garruk", VEIL_CURSED, None).unwrap();
    let defs = vec![
        CardDef::transforming(front, back).unwrap(),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Serpent", "{U}", "Creature — Serpent", "When you control no Islands, sacrifice this creature.", Some((5, 5))).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
        CardDef::new("Stone Rain", "{2}{R}", "Sorcery", "Destroy target land.", None).unwrap(),
        CardDef::new("Ogre", "{2}{R}", "Creature — Ogre", "", Some((1, 4))).unwrap(),
        CardDef::new("Sentry", "{W}", "Creature — Soldier", "When this creature is tapped, untap it.", Some((1, 1))).unwrap(),
        CardDef::new("Curse of Tides", "{U}", "Enchantment — Aura", "Enchant creature\nEnchanted creature has \"When you control no Islands, sacrifice this creature.\"", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    let mut g = Game::new(defs, 2, 20);
    for player in 0..2 { for _ in 0..5 { g.put(player, ISLAND, Zone::Library); } }
    g
}
fn resolve(g: &mut Game) {
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
}
fn cast(g: &mut Game, card: usize, targets: Vec<Vec<Target>>, spending: &str) {
    let spell = g.put(0, card, Zone::Hand);
    g.players[0].pool.colors = [9, 9, 9, 9, 9];
    g.act(0, paid(spending, Move::Cast { card: spell, targets })).unwrap();
}
fn loyalty(g: &Game, o: ObjId) -> u32 { g.obj(o).counters.get("loyalty").copied().unwrap_or(0) }
/// Garruk Relentless was cast and has resolved.
fn garruk(g: &mut Game) -> ObjId {
    cast(g, GARRUK, vec![], "GGGG");
    resolve(g);
    *g.battlefield.iter().find(|o| g.obj(**o).card == Some(GARRUK)).expect("Garruk entered")
}

#[test]
fn r603_8_it_triggers_as_soon_as_the_game_is_so() {
    let mut g = game();
    let bear = g.put(1, BEAR, Zone::Battlefield);
    let garruk = garruk(&mut g);
    assert!(g.stack.is_empty() && !g.obj(garruk).transformed, "three loyalty counters are more than two");
    // he deals 3 damage to the Bear, which deals 2 to him: one loyalty counter is left
    g.act(0, Move::Activate { source: garruk, ability: 1, targets: vec![vec![Target::Object(bear)]], paying: vec![] }).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    assert!(!g.battlefield.contains(&bear));
    assert_eq!(loyalty(&g, garruk), 1);
    cite!(["603.8"]; assert_eq!(g.stack.len(), 1, "the state trigger is on the stack"));
    assert!(!g.obj(garruk).transformed);
    resolve(&mut g);
    // the same permanent with his other face up, and the counters he had (712.18)
    cite!(["712.18"]; assert!(g.obj(garruk).transformed));
    cite!(["712.8e"]; assert_eq!(g.obj(garruk).chars.name, "Garruk, the Veil-Cursed"));
    cite!(["712.18"]; assert_eq!(loyalty(&g, garruk), 1));
    assert!(g.battlefield.contains(&garruk));
    // the face that is up has no such ability: nothing triggers again
    assert!(g.stack.is_empty());
    cite!(["202.3b"]; assert_eq!(g.obj(garruk).chars.mana_value, 4, "the mana value of his front face (202.3b)"));
}

#[test]
fn r712_8e_the_back_face_has_its_own_loyalty_abilities() {
    let mut g = game();
    let ogre = g.put(1, OGRE, Zone::Battlefield);
    let garruk = garruk(&mut g);
    g.act(0, Move::Activate { source: garruk, ability: 1, targets: vec![vec![Target::Object(ogre)]], paying: vec![] }).unwrap();
    resolve(&mut g);
    assert!(g.obj(garruk).transformed);
    assert_eq!(loyalty(&g, garruk), 2);
    // a loyalty ability of the permanent was activated this turn (606.3)
    let plus = Move::Activate { source: garruk, ability: 0, targets: vec![], paying: vec![] };
    cite!(["606.3"]; refused("a loyalty ability of that permanent has already been activated this turn (606.3)"): g.act(0, plus.clone()));
    // on his controller's next turn, the Veil-Cursed's first ability
    let mut t = crate::support::T::new(g);
    t.to_no_combat(1, ir::Part::PrecombatMain).to_no_combat(0, ir::Part::PrecombatMain);
    let mut g = t.g;
    g.act(0, plus).unwrap();
    resolve(&mut g);
    cite!(["712.8e", "606.4"]; assert_eq!(loyalty(&g, garruk), 3));
    let wolf = *g.battlefield.iter().find(|o| g.obj(**o).chars.name == "Wolf Token").expect("a Wolf");
    cite!(["712.8e"]; assert!(g.has(wolf, "Deathtouch")));
    assert_eq!((g.power(wolf), g.toughness(wolf)), (Some(1), Some(1)));
}

/// Each state trigger is its own ability: one an object was given and one of its own with the
/// same place in their definitions do not hold each other back.
#[test]
fn r603_8_a_given_state_trigger_on_the_stack_does_not_hold_back_its_own() {
    let mut g = game();
    let island = g.put(0, ISLAND, Zone::Battlefield);
    let sentry = g.put(0, SENTRY, Zone::Battlefield);
    cast(&mut g, CURSE, vec![vec![Target::Object(sentry)]], "U");
    resolve(&mut g);
    cast(&mut g, STONE_RAIN, vec![vec![Target::Object(island)]], "RRR");
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    cite!(["603.8"]; assert_eq!(g.stack.len(), 1, "the ability the Aura gives it"));
    g.tap(sentry);
    g.act(0, Move::Pass).unwrap();
    cite!(["603.8"]; assert_eq!(g.stack.len(), 2, "its own ability triggers while the other is on the stack"));
    while g.stack.len() == 2 { g.act(g.priority, Move::Pass).unwrap(); }
    assert!(!g.obj(sentry).tapped && g.battlefield.contains(&sentry) && g.stack.len() == 1);
    resolve(&mut g);
    assert!(!g.battlefield.contains(&sentry));
}

#[test]
fn r603_8_it_does_not_trigger_again_until_it_has_left_the_stack() {
    let mut g = game();
    let island = g.put(0, ISLAND, Zone::Battlefield);
    let serpent = g.put(0, SERPENT, Zone::Battlefield);
    cast(&mut g, STONE_RAIN, vec![vec![Target::Object(island)]], "RRR");
    cite!(["603.8"]; assert_eq!(g.stack.len(), 1, "only the spell: an Island is controlled"));
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    cite!(["603.8"]; assert_eq!(g.stack.len(), 1, "it triggered once"));
    assert!(g.obj(g.stack[0]).ability.is_some());
    // the state is still so each time a player would receive priority
    g.act(0, Move::Pass).unwrap();
    cite!(["603.8"]; assert_eq!(g.stack.len(), 1, "and not again while it is on the stack"));
    assert!(g.battlefield.contains(&serpent));
    g.act(1, Move::Pass).unwrap();
    assert!(!g.battlefield.contains(&serpent), "sacrificed");
    assert!(g.stack.is_empty());
}
