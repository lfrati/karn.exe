//! Mana abilities that add X mana, "where X is …" (605.3b, 608.2h): X is counted as the
//! ability resolves, once its cost is paid and what its text does first is done, and the
//! caller supplies its choice of mana with the activation. Every card is read from its printed text.
use crate::support::{cite,paid};
use game::legal::Step;
use game::{CardDef, Game, Move, ObjId, Zone};
use ir::Color;
use ir::mana::ManaSymbol;

const WEAVER: usize = 0;
const GUARDIAN: usize = 1;
const BLOSSOM: usize = 2;
const AUTOGENERATOR: usize = 3;
const STOCKPILE: usize = 4;
const BEAR: usize = 5;
const BALDURS_GATE: usize = 6;
const FOREST: usize = 7;
const GUILDGATE: usize = 8;
const COLLAPSING_GATE: usize = 9;

fn game() -> Game {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).unwrap();
    let defs = vec![
        card("Sanctum Weaver", "{1}{G}", "Enchantment Creature — Dryad", "{T}: Add X mana of any one color, where X is the number of enchantments you control.", Some((0, 2))),
        card("Axebane Guardian", "{2}{G}", "Creature — Human Druid", "Defender\n{T}: Add X mana in any combination of colors, where X is the number of creatures you control with defender.", Some((0, 3))),
        card("Lotus Blossom", "{2}", "Artifact", "At the beginning of your upkeep, you may put a petal counter on this artifact.\n{T}, Sacrifice this artifact: Add X mana of any one color, where X is the number of petal counters on this artifact.", None),
        card("Empowered Autogenerator", "{4}", "Artifact", "This artifact enters tapped.\n{T}: Put a charge counter on this artifact. Add X mana of any one color, where X is the number of charge counters on this artifact.", None),
        card("Glittering Stockpile", "{2}{R}", "Artifact — Treasure", "{T}: Add {R}. Put a stash counter on this artifact.\n{T}, Sacrifice this artifact: Add X mana of any one color, where X is the number of stash counters on this artifact.", None),
        card("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Baldur's Gate", "", "Legendary Land — Gate", "{T}: Add {C}.\n{2}, {T}: Add X mana of any one color, where X is the number of other Gates you control.", None),
        card("Forest", "", "Basic Land — Forest", "", None),
        card("Azorius Guildgate", "", "Land — Gate", "This land enters tapped.\n{T}: Add {W} or {U}.", None),
        // no printed card: Crystal Vein's second ability on a Gate, so that paying for
        // Baldur's Gate changes what it counts
        card("Collapsing Gate", "", "Land — Gate", "{T}, Sacrifice this land: Add {C}{C}.", None),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0));
    }
    Game::new(defs, 2, 20)
}

fn of(color: Color, n: usize) -> Vec<ManaSymbol> {
    vec![ManaSymbol::Color(color); n]
}

/// Activate this mana ability, saying it adds this.
fn add(g: &mut Game, source: ObjId, ability: usize, adding: Vec<Vec<ManaSymbol>>) -> Result<(), String> {
    let activate = Move::Activate { source, ability, targets: vec![], paying: vec![] };
    g.act(0, if adding.is_empty() { activate } else { Move::Adding(adding, Box::new(activate)) })
}

fn tap(source: ObjId) -> Step {
    Step { plan: vec![], x: None, life: vec![], with: None, source, ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None }
}

#[test]
fn r605_3b_x_mana_of_any_one_color_is_added_as_x_is_counted() {
    // two enchantments: two mana of one color
    let mut g = game();
    let weaver = g.put(0, WEAVER, Zone::Battlefield);
    g.put(0, WEAVER, Zone::Battlefield);
    cite!(["608.2d"]; refused("that ability offers a choice of mana: say which mana it adds (608.2d)"): add(&mut g, weaver, 0, vec![]));
    cite!(["608.2d"]; refused("that ability doesn't offer the mana said for it: say one of what it offers (608.2d)"): add(&mut g, weaver, 0, vec![of(Color::Green, 1)]));
    cite!(["608.2d"]; refused("that ability doesn't offer the mana said for it: say one of what it offers (608.2d)"): add(&mut g, weaver, 0, vec![vec![ManaSymbol::Color(Color::Green), ManaSymbol::Color(Color::Red)]]));
    cite!(["608.2d"]; refused("that ability offers no such choice of mana (608.2d)"): add(&mut g, weaver, 0, vec![of(Color::Green, 2), of(Color::Green, 2)]));
    assert!(!g.obj(weaver).tapped && g.players[0].pool.total() == 0, "a refused activation pays and adds nothing");
    add(&mut g, weaver, 0, vec![of(Color::Green, 2)]).unwrap();
    cite!(["605.3b"]; assert!(g.obj(weaver).tapped && g.stack.is_empty()));
    cite!(["608.2h"]; assert_eq!(g.players[0].pool.total(), 2));
}

#[test]
fn r605_3b_x_mana_in_any_combination_of_colors_takes_each_combination() {
    let mut g = game();
    let guardian = g.put(0, GUARDIAN, Zone::Battlefield);
    g.put(0, GUARDIAN, Zone::Battlefield);
    add(&mut g, guardian, 1, vec![vec![ManaSymbol::Color(Color::White), ManaSymbol::Color(Color::Blue)]]).unwrap();
    assert_eq!(g.players[0].pool.total(), 2);
}

#[test]
fn r608_2h_x_is_counted_after_what_the_ability_does_first() {
    // "{T}: Put a charge counter on this artifact. Add X mana of any one color, where X is
    // the number of charge counters on this artifact.": the counter it puts is counted
    let mut g = game();
    let autogenerator = g.put(0, AUTOGENERATOR, Zone::Battlefield);
    cite!(["608.2d"]; refused("that ability offers a choice of mana: say which mana it adds (608.2d)"): add(&mut g, autogenerator, 1, vec![]));
    assert!(g.obj(autogenerator).counters.is_empty() && !g.obj(autogenerator).tapped, "refused whole: no counter is put");
    add(&mut g, autogenerator, 1, vec![of(Color::Red, 1)]).unwrap();
    cite!(["608.2h"]; assert_eq!((g.obj(autogenerator).counters.get("charge"), g.players[0].pool.total()), (Some(&1), 1)));

    g.untap(autogenerator);
    cite!(["608.2d"]; refused("that ability doesn't offer the mana said for it: say one of what it offers (608.2d)"): add(&mut g, autogenerator, 1, vec![of(Color::Red, 1)]));
    assert_eq!(g.obj(autogenerator).counters.get("charge"), Some(&1));
    add(&mut g, autogenerator, 1, vec![of(Color::Blue, 2)]).unwrap();
    cite!(["608.2h"]; assert_eq!((g.obj(autogenerator).counters.get("charge"), g.players[0].pool.total()), (Some(&2), 3)));
}

#[test]
fn r608_2h_x_of_a_permanent_sacrificed_as_the_cost_is_what_it_had() {
    // "{T}, Sacrifice this artifact: Add X mana of any one color, where X is the number of
    // stash counters on this artifact.": as it last was on the battlefield (113.7a)
    let mut g = game();
    let stockpile = g.put(0, STOCKPILE, Zone::Battlefield);
    g.put_counters(stockpile, "stash", 3);
    add(&mut g, stockpile, 1, vec![of(Color::Blue, 3)]).unwrap();
    cite!(["608.2h", "113.7a"]; assert_eq!(g.players[0].pool.total(), 3));
    assert_eq!(g.players[0].graveyard.len(), 1);
}

#[test]
fn r605_3b_no_mana_offers_no_choice() {
    // no petal counter: X is 0, and there is no color to choose
    let mut g = game();
    let blossom = g.put(0, BLOSSOM, Zone::Battlefield);
    cite!(["608.2d"]; refused("that ability offers no such choice of mana (608.2d)"): add(&mut g, blossom, 1, vec![of(Color::Red, 1)]));
    add(&mut g, blossom, 1, vec![]).unwrap();
    cite!(["605.2"]; assert_eq!((g.players[0].pool.total(), g.players[0].graveyard.len()), (0, 1)));
}

#[test]
fn r605_3b_x_mana_is_counted_after_its_cost_is_paid() {
    // "{2}, {T}: Add X mana …" with an empty pool: paid by two Forests, and what is listed
    // is what the ability offers then
    let mut g = game();
    let gate = g.put(0, BALDURS_GATE, Zone::Battlefield);
    let forests = [g.put(0, FOREST, Zone::Battlefield), g.put(0, FOREST, Zone::Battlefield)];
    let other = g.put(0, GUILDGATE, Zone::Battlefield);
    g.tap(other);
    let plan = forests.map(tap).to_vec();
    let action = Move::Adding(vec![of(Color::Green, 1)], Box::new(Move::Activate { source: gate, ability: 1, targets: vec![], paying: vec![] }));
    cite!(["608.2h"]; ok: g.act(0, Move::WithMana(plan, Box::new(paid("GG",action)))));
    assert_eq!(g.players[0].pool.total(), 1);
}

#[test]
fn r608_2h_x_mana_counts_what_the_supplied_payment_leaves() {
    // two other Gates, but the only way to pay {2} sacrifices one of them: one mana is
    // offered, not two
    let mut g = game();
    let gate = g.put(0, BALDURS_GATE, Zone::Battlefield);
    let collapsing = g.put(0, COLLAPSING_GATE, Zone::Battlefield);
    let other = g.put(0, GUILDGATE, Zone::Battlefield);
    g.tap(other);
    let mut before = g.clone();
    let plan = vec![tap(collapsing)];
    let action = Move::Adding(vec![of(Color::Green, 1)], Box::new(Move::Activate { source: gate, ability: 1, targets: vec![], paying: vec![] }));
    cite!(["608.2h"]; ok: g.act(0, Move::WithMana(plan.clone(), Box::new(paid("CC",action)))));
    cite!(["608.2h"]; assert_eq!((g.players[0].pool.total(), g.players[0].graveyard.len()), (1, 1)));
    // and two mana, what it would offer before the plan, is refused whole
    let activate = Move::Activate { source: gate, ability: 1, targets: vec![], paying: vec![] };
    cite!(["608.2d"]; refused("that ability doesn't offer the mana said for it: say one of what it offers (608.2d)"): before.act(0, Move::WithMana(plan, Box::new(paid("CC",Move::Adding(vec![of(Color::Green, 2)], Box::new(activate)))))));
    assert_eq!((before.players[0].pool.total(), before.players[0].graveyard.len()), (0, 0));
    assert!(!before.obj(gate).tapped);
}

#[test]
fn r601_2g_a_payment_plan_taps_for_x_mana() {
    // {1}{G} from one Sanctum Weaver beside another: its two green mana are the plan
    let mut g = game();
    let weavers = [g.put(0, WEAVER, Zone::Battlefield), g.put(0, WEAVER, Zone::Battlefield)];
    let bear = g.put(0, BEAR, Zone::Hand);
    let plan = vec![Step { plan: vec![], x: None, life: vec![], with: None, adding: vec![of(Color::Green, 2)], ..tap(weavers[0]) }];
    g.act(0, Move::WithMana(plan, Box::new(paid("GG",Move::Cast { card: bear, targets: vec![] })))).unwrap();
    assert_eq!(g.stack.len(), 1);
    cite!(["601.2g"]; assert_eq!(weavers.iter().filter(|w| g.obj(**w).tapped).count(), 1, "one Weaver pays for it"));
}
