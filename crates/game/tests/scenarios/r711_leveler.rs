//! Leveler cards (711) and level up (702.87): a level symbol stands for a static ability that
//! gives the creature a base power and toughness and abilities while its level counters are in
//! the symbol's range.
use crate::support::{cite,paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::{Ability, CostPart};

const TREESPEAKER: usize = 0;
const ELF: usize = 1;
const BEAR: usize = 2;
const SIP: usize = 3;
const FOREST: usize = 4;

const TEXT: &str = "Level up {1}{G} ({1}{G}: Put a level counter on this. Level up only as a sorcery.)\nLEVEL 1-4\n1/2\n{T}: Add {G}{G}.\nLEVEL 5+\n1/4\nElves you control have \"{T}: Add {G}{G}.\"";

fn game() -> Game {
    let defs = vec![
        CardDef::new("Joraga Treespeaker", "{G}", "Creature — Elf Druid", TEXT, Some((1, 1))).unwrap(),
        CardDef::new("Llanowar Scout", "{G}", "Creature — Elf Scout", "", Some((1, 3))).unwrap(),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Sip", "{G}", "Instant", "Target player draws a card.", None).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    let mut g = Game::new(defs, 2, 20);
    for player in 0..2 { for _ in 0..5 { g.put(player, FOREST, Zone::Library); } }
    g
}
/// Which of its abilities, as it is now, is the activated one that costs {T} (or does not).
fn activated(g: &Game, o: ObjId, taps: bool) -> Option<usize> {
    g.abilities(o).iter().position(|a| matches!(&**a, Ability::Activated { cost, .. } if cost.0.contains(&CostPart::Tap) == taps))
}
fn level_up(g: &mut Game, o: ObjId) -> Result<(), String> {
    let ability = activated(g, o, false).expect("level up");
    g.players[0].pool.colors = [0, 0, 0, 0, 2];
    let done = g.act(0, paid("GG",Move::Activate { source: o, ability, targets: vec![], paying: vec![] }));
    while done.is_ok() && !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    done
}
fn levels(g: &Game, o: ObjId) -> u32 { g.obj(o).counters.get("level").copied().unwrap_or(0) }
fn pt(g: &Game, o: ObjId) -> (Option<i32>, Option<i32>) { (g.power(o), g.toughness(o)) }
/// Tap it for mana with the ability it has now; the green mana that added.
fn tap_for_mana(g: &mut Game, o: ObjId) -> Option<u32> {
    let ability = activated(g, o, true)?;
    g.players[0].pool = Default::default();
    g.act(0, Move::Activate { source: o, ability, targets: vec![], paying: vec![] }).ok()?;
    Some(std::mem::take(&mut g.players[0].pool).colors[4])
}

#[test]
fn r702_87a_level_up_puts_a_level_counter_on_it_as_a_sorcery() {
    let mut g = game();
    let tree = g.put(0, TREESPEAKER, Zone::Battlefield);
    cite!(["711.5"]; assert_eq!((levels(&g, tree), pt(&g, tree)), (0, (Some(1), Some(1))), "its uppermost power and toughness (711.5)"));
    assert!(activated(&g, tree, true).is_none(), "no ability of a level it has not reached");
    level_up(&mut g, tree).unwrap();
    cite!(["702.87a"]; assert_eq!(levels(&g, tree), 1));
    cite!(["702.87a"]; assert!(g.players[0].pool.colors.iter().all(|n| *n == 0), "{{1}}{{G}} was paid"));
    // only as a sorcery: not with a spell on the stack, not in another player's turn
    let sip = g.put(0, SIP, Zone::Hand);
    g.players[0].pool.colors = [0, 0, 0, 0, 1];
    g.act(0, paid("G",Move::Cast { card: sip, targets: vec![vec![Target::Player(0)]] })).unwrap();
    cite!(["702.87a", "602.5d"]; refused("that ability is activated only as a sorcery: in a main phase of your own turn with the stack empty (602.5d)"): level_up(&mut g, tree));
    assert_eq!(levels(&g, tree), 1);
}

#[test]
fn r711_2a_a_band_gives_its_power_and_toughness_and_abilities_while_the_counters_are_in_it() {
    let mut g = game();
    let tree = g.put(0, TREESPEAKER, Zone::Battlefield);
    let elf = g.put(0, ELF, Zone::Battlefield);
    for level in 1..=4 {
        level_up(&mut g, tree).unwrap();
        cite!(["711.2a"]; case: format!("level {level}"); assert_eq!((levels(&g, tree), pt(&g, tree)), (level, (Some(1), Some(2)))));
        assert!(activated(&g, elf, true).is_none(), "LEVEL 1-4 gives the ability to it alone");
    }
    cite!(["711.2a"]; assert_eq!(tap_for_mana(&mut g, tree), Some(2)));
    assert!(g.obj(tree).tapped);
}

#[test]
fn r711_2b_the_last_band_has_no_upper_bound_and_the_one_before_it_ends() {
    let mut g = game();
    let tree = g.put(0, TREESPEAKER, Zone::Battlefield);
    let elf = g.put(0, ELF, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let theirs = g.put(1, ELF, Zone::Battlefield);
    g.put_counters(tree, "level", 4);
    level_up(&mut g, tree).unwrap();
    cite!(["711.2b"]; assert_eq!((levels(&g, tree), pt(&g, tree)), (5, (Some(1), Some(4)))));
    // Elves you control have the ability: the Treespeaker as one of them, once
    assert_eq!(g.abilities(tree).iter().filter(|a| matches!(&***a, Ability::Activated { cost, .. } if cost.0.contains(&CostPart::Tap))).count(), 1);
    assert!(activated(&g, bear, true).is_none() && activated(&g, theirs, true).is_none());
    cite!(["711.2b"]; assert_eq!(tap_for_mana(&mut g, elf), Some(2)));
    assert_eq!(tap_for_mana(&mut g, tree), Some(2));
    // it has its level up ability at every level (711.4)
    level_up(&mut g, tree).unwrap();
    cite!(["711.4", "711.2b"]; assert_eq!((levels(&g, tree), pt(&g, tree)), (6, (Some(1), Some(4)))));
    // +1/+1 counters and the like apply over the base it sets (613.4b, 613.4c)
    g.put_counters(tree, "+1/+1", 1);
    cite!(["613.4b", "613.4c"]; assert_eq!(pt(&g, tree), (Some(2), Some(5))));
}

/// A level symbol is one static ability (711.2a). An effect that removes all abilities is
/// applied in the layer the symbol gives its abilities in, and the symbol's effect depends on
/// it (613.8a): it would end its existence. So the removal is applied first whatever the
/// timestamps (613.8b), the symbol's effect never begins (613.6), and the creature has its
/// printed power and toughness, whether or not the band gives an ability.
#[test]
fn r613_8_a_leveler_that_loses_its_abilities_has_its_printed_power_and_toughness() {
    let defs = vec![
        CardDef::new("Skywatcher", "{U}", "Creature — Merfolk", "Level up {3}\nLEVEL 1-2\n2/2\nFlying\nLEVEL 3+\n4/2\nFlying", Some((1, 1))).unwrap(),
        CardDef::new("Plain Leveler", "{U}", "Creature — Merfolk", "Level up {3}\nLEVEL 1+\n3/3", Some((1, 1))).unwrap(),
        CardDef::new("Silence", "{0}", "Enchantment", "All creatures lose all abilities.", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0)); }
    let mut g = Game::new(defs, 2, 20);
    let sky = g.put(0, 0, Zone::Battlefield);
    let plain = g.put(0, 1, Zone::Battlefield);
    g.put_counters(sky, "level", 1);
    g.put_counters(plain, "level", 1);
    assert!(g.has(sky, "Flying"));
    assert_eq!((pt(&g, sky), pt(&g, plain)), ((Some(2), Some(2)), (Some(3), Some(3))));
    g.put(1, 2, Zone::Battlefield);
    cite!(["613.1f"]; assert!(!g.has(sky, "Flying") && activated(&g, sky, false).is_none()));
    cite!(["613.8a", "613.8b"]; assert_eq!(pt(&g, sky), (Some(1), Some(1)), "the removal it depends on was applied first"));
    cite!(["613.1f", "613.4b"]; assert_eq!(pt(&g, plain), (Some(1), Some(1)), "its ability was gone before layer 7b"));
}

/// The power and toughness and the abilities of a band are one continuous effect: one static
/// ability of the card, not one for each.
#[test]
fn r711_2a_a_level_symbol_is_one_static_ability() {
    let def = CardDef::new("Skywatcher", "{U}", "Creature — Merfolk", "Level up {3}\nLEVEL 1-2\n2/2\nFlying\nLEVEL 3+\n4/2\nFlying, lifelink\nWhenever this creature attacks, draw a card.", Some((1, 1))).unwrap();
    game::play::check(&def).unwrap_or_else(|u| panic!("{}", u.0));
    cite!(["711.2"]; assert_eq!(def.abilities().len(), 3, "level up, and one for each symbol: {:?}", def.abilities()));
    let mut g = Game::new(vec![def], 2, 20);
    let sky = g.put(0, 0, Zone::Battlefield);
    g.put_counters(sky, "level", 3);
    cite!(["711.2b"]; assert!(g.has(sky, "Flying") && g.has(sky, "Lifelink")));
    cite!(["711.2b"]; assert_eq!(pt(&g, sky), (Some(4), Some(2))));
    cite!(["711.2b"]; assert_eq!(g.abilities(sky).iter().filter(|a| matches!(&***a, Ability::Triggered { .. })).count(), 1));
}

#[test]
fn r711_6_it_has_its_uppermost_power_and_toughness_in_every_other_zone() {
    let mut g = game();
    let card = g.put(0, TREESPEAKER, Zone::Hand);
    cite!(["711.6"]; assert_eq!(pt(&g, card), (Some(1), Some(1))));
    // the whole leveler corpus entry reads the same
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let def = CardDef::from_faces(catalog.get("Joraga Treespeaker").expect("in the corpus")).unwrap();
    game::play::check(&def).unwrap_or_else(|u| panic!("{}", u.0));
}
