//! "X mana in any combination of colors" from a mana ability, X more than three (605.3b,
//! 106.1a): the color of each mana is its own choice of the five colors, asked in turn as
//! the ability is activated, and all the mana is added at once. Three or fewer stay one
//! choice among every combination. Callers supply their mana choices and sources.
//! Cards are read from their printed text; the synthetic
//! ones say what no printed card isolates.
use crate::support::{cite,paid};
use game::legal::Step;
use game::operation::ReplacementId;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, Zone};
use ir::Color;
use ir::mana::ManaSymbol;

const GUARDIAN: usize = 0;
const ROCKETS: usize = 1;
const FOREST: usize = 2;
const BAXTER: usize = 3;
const FIVE_COLOR: usize = 4;
const HYBRID: usize = 5;
const COUNTING: usize = 6;
const SCALES: usize = 7;
const EVOLUTION: usize = 8;

fn game() -> Game {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).unwrap();
    let defs = vec![
        card("Axebane Guardian", "{2}{G}", "Creature — Human Druid", "Defender\n{T}: Add X mana in any combination of colors, where X is the number of creatures you control with defender.", Some((0, 3))),
        card("Wizard's Rockets", "{1}", "Artifact", "This artifact enters tapped.\n{X}, {T}, Sacrifice this artifact: Add X mana in any combination of colors.\nWhen this artifact is put into a graveyard from the battlefield, draw a card.", None),
        card("Forest", "", "Basic Land — Forest", "", None),
        card("Baxter Building", "", "Land", "{T}: Add {C}.\n{4}, {T}: Add four mana in any combination of colors.\n{4}, {T}: Draw a card. Activate only if you control a creature with toughness 4 or greater.", None),
        // no printed card: one mana of each color, and four symbols of four hybrid pairs
        card("Prism Giant", "{W}{U}{B}{R}{G}", "Creature — Giant", "", Some((5, 5))),
        card("Hybrid Rite", "{W/U}{B/R}{G/W}{U/B}", "Sorcery", "You gain 1 life.", None),
        card("Counting Druid", "{0}", "Creature — Elf", "{T}: Put a +1/+1 counter on this creature. Add X mana in any combination of colors, where X is the number of +1/+1 counters on this creature.", Some((1, 1))),
        card("Hardened Scales", "{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.", None),
        card("Branching Evolution", "{2}{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.", None),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0));
    }
    Game::new(defs, 2, 20)
}

const COLORS: [Color; 5] = [Color::White, Color::Blue, Color::Black, Color::Red, Color::Green];

fn one(c: Color) -> Vec<ManaSymbol> {
    vec![ManaSymbol::Color(c)]
}

fn each_color() -> Vec<Vec<ManaSymbol>> {
    COLORS.map(one).to_vec()
}

fn step(source: ObjId, ability: usize, adding: Vec<Vec<ManaSymbol>>) -> Step {
    Step { plan: vec![], x: None, life: vec![], with: None, source, ability, adding, paying: vec![], chose: vec![], cost_branches: None }
}

fn activate(source: ObjId, ability: usize, adding: Vec<Vec<ManaSymbol>>) -> Move {
    Move::Adding(adding, Box::new(Move::Activate { source, ability, targets: vec![], paying: vec![] }))
}

/// The mana each `ManaAdded` event of this source says, in order.
fn added(g: &Game, source: ObjId) -> Vec<Vec<ManaSymbol>> {
    g.log.iter().filter_map(|e| match e {
        Event::ManaAdded { source: s, mana, .. } if *s == source => Some(mana.clone()),
        _ => None,
    }).collect()
}

#[test]
fn r605_3b_four_mana_in_any_combination_asks_the_color_of_each_in_turn() {
    // three defenders: one choice among all 35 combinations, as before
    let mut g = game();
    let guardians: Vec<_> = (0..3).map(|_| g.put(0, GUARDIAN, Zone::Battlefield)).collect();
    let mut three = g.clone();
    three.act(0, activate(guardians[0], 1, vec![vec![ManaSymbol::Color(Color::Green); 3]])).unwrap();
    cite!(["106.1a"]; assert_eq!(three.players[0].pool.colors, [0, 0, 0, 0, 3]));
    let guardian = guardians[0];
    g.put(0, GUARDIAN, Zone::Battlefield);

    // anything but one color for each of the four is refused whole (605.3b, 733)
    let before = format!("{g:?}");
    // mana not offered; too few said; too many said; nothing said
    let unoffered = "that ability doesn't offer the mana said for it: say one of what it offers (608.2d)";
    let refused = [
        (vec![vec![ManaSymbol::Color(Color::Green); 4]], unoffered),
        (vec![one(Color::Green); 3], "that ability offers a choice of mana: say which mana it adds (608.2d)"),
        (vec![one(Color::Green); 5], "that ability offers no such choice of mana (608.2d)"),
        (vec![one(Color::Green), one(Color::Green), one(Color::Green), vec![ManaSymbol::Colorless]], unoffered),
        (vec![one(Color::Green), one(Color::Green), one(Color::Green), vec![ManaSymbol::Color(Color::Red), ManaSymbol::Color(Color::Red)]], unoffered),
        (vec![], "that ability offers a choice of mana: say which mana it adds (608.2d)"),
    ];
    for (adding, why) in refused {
        let result = g.act(0, activate(guardian, 1, adding.clone()));
        cite!(["605.3b"]; case: format!("{adding:?}"); refused(why): result);
        assert_eq!(format!("{g:?}"), before, "{adding:?} changes nothing");
    }

    g.act(0, activate(guardian, 1, vec![one(Color::White), one(Color::Blue), one(Color::Red), one(Color::Red)])).unwrap();
    cite!(["605.3b"]; assert!(g.obj(guardian).tapped && g.stack.is_empty() && g.decision.is_none()));
    assert_eq!((g.players[0].pool.colors, g.players[0].pool.colorless), ([1, 1, 0, 2, 0], 0));
    // one event: all the mana is added at once (106.3)
    cite!(["106.3"]; assert_eq!(added(&g, guardian), vec![[Color::White, Color::Blue, Color::Red, Color::Red].map(ManaSymbol::Color).to_vec()]));
}

#[test]
fn r605_3b_many_defenders_produce_each_supplied_mana_color() {
    // twelve: twelve choices of five, not 1,820 combinations or 5^12 ways
    let mut g = game();
    let guardians: Vec<_> = (0..12).map(|_| g.put(0, GUARDIAN, Zone::Battlefield)).collect();
    let adding: Vec<_> = (0..12).map(|i| one(COLORS[i % 5])).collect();
    g.act(0, activate(guardians[0], 1, adding)).unwrap();
    assert_eq!(g.players[0].pool.colors, [3, 3, 2, 2, 2]);
}

#[test]
fn r107_3a_wizards_rockets_adds_the_x_announced_for_its_cost() {
    // X is explicitly supplied with the activation.
    let mut g = game();
    let rockets = g.put(0, ROCKETS, Zone::Battlefield);
    g.untap(rockets);
    let forests: Vec<_> = (0..6).map(|_| g.put(0, FOREST, Zone::Battlefield)).collect();
    // Announced X determines both the paid cost and actual output (107.3a).
    for x in 0..=6u32 {
        let mut branch = g.clone();
        let plan = forests[..x as usize].iter().map(|o|step(*o,0,vec![])).collect();
        let adding = match x { 0=>vec![], 1..=3=>vec![vec![ManaSymbol::Color(Color::Red);x as usize]], _=>vec![one(Color::Red);x as usize] };
        branch.act(0, Move::WithX(x, Box::new(Move::WithMana(plan, Box::new(paid(&"G".repeat(x as usize),activate(rockets, 1, adding))))))).unwrap();
        cite!(["107.3a"]; assert_eq!(branch.players[0].pool.total(), x));
    }

    // X more than the Forests pay is refused whole
    let before = format!("{g:?}");
    let plan: Vec<_> = forests.iter().map(|o|step(*o,0,vec![])).collect();
    let seven = Move::WithX(7, Box::new(Move::WithMana(plan.clone(), Box::new(paid("GGGGGG",activate(rockets, 1, vec![one(Color::Red); 7]))))));
    cite!(["118.3a"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0, seven));
    assert_eq!(format!("{g:?}"), before);

    // Five Forests pay X=5, producing one mana of each color.
    let plan = forests[..5].iter().map(|o|step(*o,0,vec![])).collect();
    let adding = COLORS.map(one).to_vec();
    g.act(0, Move::WithX(5, Box::new(Move::WithMana(plan, Box::new(paid("GGGGG",activate(rockets, 1, adding))))))).unwrap();
    cite!(["107.3a"]; assert_eq!(g.players[0].pool.colors, [1, 1, 1, 1, 1], "the Forests' mana paid {{X}}"));
    assert_eq!(g.players[0].pool.colorless, 0);
    assert_eq!(forests.iter().filter(|f| g.obj(**f).tapped).count(), 5);
    assert_eq!(names(&g, &g.players[0].graveyard), vec!["Wizard's Rockets"], "sacrificed as its cost");
    assert_eq!(added(&g, rockets).len(), 1);
}

#[test]
fn r605_3b_a_printed_four_mana_in_any_combination_is_asked_by_color() {
    // Baxter Building: "{4}, {T}: Add four mana in any combination of colors."
    let mut g = game();
    let baxter = g.put(0, BAXTER, Zone::Battlefield);
    let plan = (0..4).map(|_|step(g.put(0,FOREST,Zone::Battlefield),0,vec![])).collect();
    let adding = vec![one(Color::Blue), one(Color::Blue), one(Color::Black), one(Color::White)];
    g.act(0, Move::WithMana(plan, Box::new(paid("GGGG",activate(baxter, 1, adding))))).unwrap();
    assert_eq!(g.players[0].pool.colors, [1, 2, 1, 0, 0]);
    assert!(g.obj(baxter).tapped);
}

#[test]
fn r601_2g_supplied_colors_pay_colored_and_hybrid_costs() {
    // one untapped Guardian of five pays {W}{U}{B}{R}{G} alone: only one of the 3,125 ways
    // of answering its five choices (up to their order) does
    let mut g = game();
    let guardians: Vec<_> = (0..5).map(|_| g.put(0, GUARDIAN, Zone::Battlefield)).collect();
    guardians[1..].iter().for_each(|o| g.tap(*o));
    let giant = g.put(0, FIVE_COLOR, Zone::Hand);
    let plan = vec![step(guardians[0], 1, COLORS.map(one).to_vec())];
    g.act(0, Move::WithMana(plan, Box::new(paid("WUBRG",Move::Cast { card: giant, targets: vec![] })))).unwrap();
    cite!(["601.2g"]; assert_eq!(names(&g, &g.stack), vec!["Prism Giant"]));
    assert_eq!(g.players[0].pool.total(), 0);

    // hybrid symbols: four Guardians, three tapped. No one color pays {W/U}{B/R}{G/W}{U/B}
    let mut g = game();
    let guardians: Vec<_> = (0..4).map(|_| g.put(0, GUARDIAN, Zone::Battlefield)).collect();
    guardians[1..].iter().for_each(|o| g.tap(*o));
    let rite = g.put(0, HYBRID, Zone::Hand);
    let plan = vec![step(guardians[0], 1, [Color::White,Color::Black,Color::Green,Color::Blue].map(one).to_vec())];
    g.act(0, Move::WithMana(plan, Box::new(paid("WBGU",Move::Cast { card: rite, targets: vec![] })))).unwrap();
    assert_eq!(names(&g, &g.stack), vec!["Hybrid Rite"]);
}

#[test]
fn r601_2g_two_sources_of_many_colors_share_what_a_cost_lacks() {
    // two untapped Guardians of four, each adding four: neither alone has five colors, the
    // second adds the one the first leaves lacking
    let mut g = game();
    let guardians: Vec<_> = (0..4).map(|_| g.put(0, GUARDIAN, Zone::Battlefield)).collect();
    guardians[2..].iter().for_each(|o| g.tap(*o));
    let giant = g.put(0, FIVE_COLOR, Zone::Hand);
    let plan = vec![
        step(guardians[0],1,[Color::White,Color::Blue,Color::Black,Color::Red].map(one).to_vec()),
        step(guardians[1],1,vec![one(Color::Green);4]),
    ];
    g.act(0, Move::WithMana(plan, Box::new(paid("WUBRG",Move::Cast { card: giant, targets: vec![] })))).unwrap();
    assert_eq!(names(&g, &g.stack), vec!["Prism Giant"]);
    cite!(["106.4"]; assert_eq!(g.players[0].pool.total(), 3, "eight mana for five"));
}

#[test]
fn r608_2h_answers_saved_for_three_are_not_used_for_four_and_each_copy_keeps_its_own() {
    // "Put a +1/+1 counter … Add X mana …, where X is the number of +1/+1 counters": the
    // order of the replacements decides whether X is three or four (616.1)
    let mut g = game();
    let druid = g.put(0, COUNTING, Zone::Battlefield);
    g.turn = 2;
    let scales = g.put(0, SCALES, Zone::Battlefield);
    let evolution = g.put(0, EVOLUTION, Zone::Battlefield);
    let three = vec![vec![ManaSymbol::Color(Color::White), ManaSymbol::Color(Color::Blue), ManaSymbol::Color(Color::Black)]];
    g.act(0, activate(druid, 0, three.clone())).unwrap();
    assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::Replacement { .. } })));
    let replacement = |source| Move::Choose(Answer::Replacement(ReplacementId::from_ability(source, game::binding::AbilityKey::printed(0), 0)));

    // Evolution first: 2 + 1 = three, and the saved answer is used
    let mut other = g.clone();
    other.act(0, replacement(evolution)).unwrap();
    cite!(["616.1"]; assert_eq!(other.obj(druid).counters["+1/+1"], 3));
    cite!(["608.2h"]; assert_eq!(other.players[0].pool.colors, [1, 1, 1, 0, 0]));
    assert!(other.decision.is_none() && other.resolving().is_none());

    // Scales first: (1 + 1) × 2 = four, and the saved three-mana answer is no answer to the
    // color of one mana: each is asked of the player, and nothing is added until the last
    g.act(0, replacement(scales)).unwrap();
    cite!(["616.1"]; assert_eq!(g.obj(druid).counters["+1/+1"], 4));
    for color in [Color::Red, Color::Red] {
        let Some(Decision::Choose { player: 0, choice: Choice::Mana { options } }) = &g.decision else { panic!("{:?}", g.decision) };
        assert_eq!(options, &each_color());
        g.act(0, Move::Choose(Answer::Mana(one(color)))).unwrap();
    }
    cite!(["608.2d"]; refused("the choice is which of the mana offered to add (608.2d)"): g.act(0, Move::Choose(Answer::Mana(vec![ManaSymbol::Colorless]))));
    assert_eq!(g.players[0].pool.total(), 0);
    let mut copy = g.clone();
    for (game, color, colors) in [(&mut g, Color::Green, [0, 0, 0, 2, 2]), (&mut copy, Color::White, [2, 0, 0, 2, 0])] {
        for _ in 0..2 {
            game.act(0, Move::Choose(Answer::Mana(one(color)))).unwrap();
        }
        cite!(["608.2h"]; case: format!("{color:?}"); assert_eq!(game.players[0].pool.colors, colors));
        assert!(game.decision.is_none() && game.resolving().is_none() && game.stack.is_empty());
        cite!(["106.3"]; assert_eq!(added(game, druid).len(), 1));
        assert_eq!(game.priority, 0);
    }
}

/// The names of these objects: a card that moves is a new object (400.7).
fn names(g: &Game, objects: &[ObjId]) -> Vec<String> {
    objects.iter().map(|o| g.obj(*o).chars.name.clone()).collect()
}
