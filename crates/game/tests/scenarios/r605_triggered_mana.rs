//! Triggered mana abilities (605.1b): one with no target that triggers from a mana ability
//! and could add mana resolves as soon as the mana ability that triggered it does, without
//! using the stack (605.4a).
use crate::support;
use support::*;

/// Leyline of Abundance: "Whenever you tap a creature for mana, add an additional {G}." The
/// Elves' {G} and the Leyline's are both in the pool at once, with nothing on the stack.
#[test]
fn r605_4a_the_additional_mana_is_added_at_once() {
    let mut t = Board::new(2).battlefield(0, &["Leyline of Abundance", "Llanowar Elves"]).build();
    let elves = t.find_in_battlefield("Llanowar Elves");
    t.g.act(0, Move::Activate { source: elves, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    t.expect().rules(["605.4a"]).stack(&[]).holds(|g| g.players[0].pool.colors[4] == 2, "{G}{G} in the pool").check();
    // nothing waits for the stack either
    t.g.act(0, Move::Pass).unwrap();
    t.expect().stack(&[]).check();
}

/// Tapping a land is not tapping a creature; an opponent's creature is not "you" tapping one.
#[test]
fn r605_1b_it_triggers_only_as_its_pattern_says() {
    let mut t = Board::new(2).battlefield(0, &["Leyline of Abundance", "Forest"]).battlefield(1, &["Llanowar Elves"]).build();
    let forest = t.find_in_battlefield("Forest");
    t.g.act(0, Move::Activate { source: forest, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    cite!(["603.2"]; assert_eq!(t.g.players[0].pool.colors[4], 1));
    let elves = t.find_in_battlefield("Llanowar Elves");
    t.g.act(0, Move::Pass).unwrap();
    t.g.act(1, Move::Activate { source: elves, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    cite!(["603.2"]; assert_eq!((t.g.players[0].pool.colors[4], t.g.players[1].pool.colors[4]), (1, 1)));
}

/// Vernal Bloom: "Whenever a Forest is tapped for mana, its controller adds an additional
/// {G}." Its controller is whoever controls the Forest, not the Bloom's controller.
#[test]
fn r605_4a_its_controller_adds_it() {
    let mut t = Board::new(2).battlefield(0, &["Vernal Bloom"]).battlefield(1, &["Forest"]).build();
    let forest = t.find_in_battlefield("Forest");
    t.g.act(0, Move::Pass).unwrap();
    t.g.act(1, Move::Activate { source: forest, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    assert_eq!((t.g.players[0].pool.colors[4], t.g.players[1].pool.colors[4]), (0, 2));
}

/// Paying a cost (601.2g): one Elf pays for Grizzly Bears' {1}{G} with Badgermole Cub's
/// additional {G}, and not without it.
#[test]
fn r601_2g_a_payment_counts_the_additional_mana() {
    let mut t = Board::new(2).battlefield(0, &["Badgermole Cub", "Llanowar Elves"]).hand(0, &["Grizzly Bears"]).build();
    let bears = t.find_in(0, Zone::Hand, "Grizzly Bears");
    let elf = t.find_in_battlefield("Llanowar Elves");
    let step = |source| game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source, ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None };
    let steps = vec![step(elf)];
    t.g.begin_payment_audit();
    t.g.act(0, Move::WithMana(steps, Box::new(paid("GG",Move::Cast { card: bears, targets: vec![] })))).unwrap();
    t.expect().rules(["601.2g", "605.4a"]).stack(&["Grizzly Bears"]).tapped("Llanowar Elves", true).check();
    let production=t.g.payment_production_audit();
    assert_eq!(production.len(),2);
    assert_eq!(production[0]["path"],serde_json::json!([0]));
    assert_eq!(production[1]["path"],serde_json::json!([0]));
    assert_eq!(production[0]["triggered"],false);
    assert_eq!(production[1]["triggered"],true);
    assert_ne!(production[0]["source"],production[1]["source"]);
    let mut t = Board::new(2).battlefield(0, &["Llanowar Elves"]).hand(0, &["Grizzly Bears"]).build();
    let elf = t.find_in_battlefield("Llanowar Elves");
    let bears = t.find_in(0, Zone::Hand, "Grizzly Bears");
    let before = format!("{:?}", t.g);
    cite!(["601.2g"]; refused("the supplied mana does not pay that cost, all of it and no more"): t.g.act(0, Move::WithMana(vec![step(elf)], Box::new(paid("G",Move::Cast { card: bears, targets: vec![] })))));
    assert_eq!(format!("{:?}", t.g), before);
}

/// A trigger from a mana ability that adds no mana is not a mana ability: it goes on the
/// stack (605.1b).
#[test]
fn r605_1b_a_trigger_that_adds_no_mana_uses_the_stack() {
    let defs = vec![
        game::CardDef::new("Tapper", "{0}", "Enchantment", "Whenever a Forest is tapped for mana, you gain 1 life.", None).unwrap(),
        game::CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, 1, Zone::Library); } }
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g.put(0, 0, Zone::Battlefield);
    let forest = g.put(0, 1, Zone::Battlefield);
    g.act(0, Move::Activate { source: forest, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    cite!(["605.1b"]; assert_eq!(g.players[0].life, 20));
    assert_eq!(g.players[0].pool.colors[4], 1);
    g.act(0, Move::Pass).unwrap();
    cite!(["605.1b", "603.3"]; assert_eq!(g.stack.len(), 1));
}

/// A game at player 0's main phase with these cards, the first a land enchanted by the
/// second when it is an Aura ("Enchant land"), cast with mana from other Forests.
fn enchanted(names: &[&str]) -> (Game, ObjId) {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let defs: Vec<game::CardDef> = names.iter().map(|n| game::CardDef::from_face(faces.iter().find(|f| f.name == *n).unwrap()).unwrap()).collect();
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, 0, Zone::Library); } }
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    let land = g.put(0, 0, Zone::Battlefield);
    for _ in 0..2 { let pay = g.put(0, 0, Zone::Battlefield); g.act(0, Move::Activate { source: pay, ability: 0, targets: vec![], paying: vec![] }).unwrap(); }
    let aura = g.put(0, 1, Zone::Hand);
    g.act(0, paid("GG",Move::Cast { card: aura, targets: vec![vec![Target::Object(land)]] })).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    (g, land)
}

/// Fertile Ground: "Whenever enchanted land is tapped for mana, its controller adds an
/// additional one mana of any color." Tapped by itself, the color is asked as it resolves.
#[test]
fn r605_4a_a_triggered_mana_ability_asks_as_it_resolves() {
    let (mut g, forest) = enchanted(&["Forest", "Fertile Ground"]);
    g.act(0, Move::Activate { source: forest, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    cite!(["608.2d"]; assert!(matches!(g.asked(), Some((0, Asked::Mana { .. }))), "{:?}", g.asked()));
    g.act(0, Move::Choose(Answer::Mana(vec![ir::mana::ManaSymbol::Color(ir::Color::Blue)]))).unwrap();
    cite!(["605.4a"]; assert_eq!((g.players[0].pool.colors[1], g.players[0].pool.colors[4]), (1, 1)));
    cite!(["605.4a"]; assert!(g.stack.is_empty()));
}

/// A payment plan says the color for the trigger after what its mana ability says (605.4a):
/// one enchanted Forest pays for Growth Spiral's {G}{U}.
#[test]
fn r601_2g_a_plan_says_what_the_trigger_adds() {
    let (mut g, forest) = enchanted(&["Forest", "Fertile Ground", "Growth Spiral"]);
    let spiral = g.put(0, 2, Zone::Hand);
    let plan = vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source: forest, ability: 0, paying: vec![], adding: vec![vec![ir::mana::ManaSymbol::Color(ir::Color::Blue)]], chose: vec![], cost_branches: None }];
    g.act(0, Move::WithMana(plan, Box::new(paid("GU",Move::Cast { card: spiral, targets: vec![] })))).unwrap();
    cite!(["601.2g"]; assert!(g.stack.len() == 1 && g.obj(g.stack[0]).chars.name == "Growth Spiral"));
    assert_eq!(g.players[0].pool.total(), 0);
}

/// High Tide: "Until end of turn, whenever a player taps an Island for mana, that player
/// adds an additional {U}." A delayed triggered mana ability resolves at once too.
#[test]
fn r605_4a_a_delayed_triggered_mana_ability_resolves_at_once() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let defs: Vec<game::CardDef> = ["Island", "High Tide"].iter().map(|n| game::CardDef::from_face(faces.iter().find(|f| f.name == *n).unwrap()).unwrap()).collect();
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, 0, Zone::Library); } }
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    let pay = g.put(0, 0, Zone::Battlefield);
    g.act(0, Move::Activate { source: pay, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    let tide = g.put(0, 1, Zone::Hand);
    g.act(0, paid("U",Move::Cast { card: tide, targets: vec![] })).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    let island = g.put(0, 0, Zone::Battlefield);
    g.act(0, Move::Activate { source: island, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    cite!(["605.4a"]; assert_eq!(g.players[0].pool.colors[1], 2));
    cite!(["605.4a"]; assert!(g.stack.is_empty()));
    g.act(0, Move::Pass).unwrap();
    assert!(g.stack.is_empty());
}
