//! What an effect's "that creature" and "that player" are said of when more than one thing
//! took part in the event that made its ability trigger (608.2k): what the trigger condition
//! says over what it leaves unsaid, what it introduced ("a creature") over what it names
//! definitely ("equipped creature", "this creature", "you"), and what it called by the same
//! noun. Decided as the text is lowered; every card here passes admission.

use crate::support::cite;
use game::card::CardDef;
use game::play::check;
use game::{Event, Game, Move, ObjId, Target, Zone};

const BEARS: usize = 0;
const WALL: usize = 1;
const VAMPIRE: usize = 2;

/// Grizzly Bears, a 0/20 Wall, a 2/2 Vampire, then the cards under test.
fn game(cards: &[(&str, &str, &str, Option<(i32, i32)>)]) -> Game {
    let card = |name, types, text, pt| {
        let def = CardDef::new(name, "{1}", types, text, pt).expect(name);
        check(&def).unwrap_or_else(|e| panic!("{name}: {}", e.0));
        def
    };
    let mut defs = vec![
        card("Grizzly Bears", "Creature — Bear", "", Some((2, 2))),
        card("Wall", "Creature — Wall", "", Some((0, 20))),
        card("Vampire", "Creature — Vampire", "", Some((2, 2))),
    ];
    defs.extend(cards.iter().map(|(name, types, text, pt)| card(name, types, text, *pt)));
    let mut g = Game::new(defs, 2, 20);
    for player in 0..2 {
        for _ in 0..5 {
            g.put(player, BEARS, Zone::Library);
        }
    }
    g
}

fn pass_until_something(g: &mut Game) {
    for _ in 0..60 {
        if g.decision.is_some() || !g.stack.is_empty() {
            return;
        }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("nothing happened");
}

fn all_pass(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}

/// Player 0's creature attacks player 1, whose creature blocks it; stops with whatever
/// triggered on the stack, or at the next decision.
fn fight(g: &mut Game, attacker: ObjId, blocker: ObjId) {
    pass_until_something(g);
    g.act(0, Move::Attack(vec![(attacker, Target::Player(1))])).unwrap();
    pass_until_something(g);
    g.act(1, Move::Block(vec![(blocker, attacker)])).unwrap();
}

/// Every pass until the stack is empty and combat damage has been dealt.
fn through_damage(g: &mut Game) {
    for _ in 0..60 {
        if g.stack.is_empty() && g.log.iter().any(|e| matches!(e, Event::DamageDealt { combat: true, .. })) {
            return;
        }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("no combat damage");
}

fn noncombat_damage(g: &Game) -> Vec<(ObjId, Target, u32)> {
    g.log
        .iter()
        .filter_map(|e| match e {
            Event::DamageDealt { source, to, amount, combat: false } => Some((*source, *to, *amount)),
            _ => None,
        })
        .collect()
}

#[test]
fn r608_2k_that_creature_is_the_one_the_condition_says_not_what_it_leaves_unsaid() {
    // "Whenever a creature blocks, …": the creature it blocked also took part, unsaid
    let mut g = game(&[("Heat of Battle", "Enchantment", "Whenever a creature blocks, this enchantment deals 1 damage to that creature's controller.", None)]);
    let heat = g.put(0, 3, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let wall = g.put(1, WALL, Zone::Battlefield);
    fight(&mut g, bears, wall);
    assert_eq!(g.stack.len(), 1);
    all_pass(&mut g);
    cite!(["608.2k"]; assert_eq!(noncombat_damage(&g), vec![(heat, Target::Player(1), 1)], "the blocker's controller, not the attacker's"));

    // "Whenever enchanted creature is dealt damage, …": what dealt it also took part, unsaid
    let mut g = game(&[(
        "Binding Agony",
        "Enchantment — Aura",
        "Enchant creature\nWhenever enchanted creature is dealt damage, this Aura deals that much damage to that creature's controller.",
        None,
    )]);
    let agony = g.put(0, 3, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let wall = g.put(1, WALL, Zone::Battlefield);
    assert!(g.attach(agony, Target::Object(wall)));
    fight(&mut g, bears, wall);
    through_damage(&mut g);
    cite!(["608.2k"]; assert_eq!(noncombat_damage(&g), vec![(agony, Target::Player(1), 2)], "the damaged creature's controller, not the controller of what dealt it"));
}

#[test]
fn r608_2k_that_creature_is_the_one_introduced_not_the_one_named_definitely() {
    // "Whenever equipped creature deals damage to a creature, exile that creature."
    let mut g = game(&[(
        "Sword of Kaldra",
        "Legendary Artifact — Equipment",
        "Equipped creature gets +5/+5.\nWhenever equipped creature deals damage to a creature, exile that creature. (Exile it only if it's still on the battlefield.)\nEquip {4}",
        None,
    )]);
    let sword = g.put(0, 3, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let wall = g.put(1, WALL, Zone::Battlefield);
    assert!(g.attach(sword, Target::Object(bears)));
    fight(&mut g, bears, wall);
    through_damage(&mut g);
    cite!(["608.2k"]; assert!(g.battlefield.contains(&bears), "not the equipped creature"));
    cite!(["608.2k"]; assert!(!g.battlefield.contains(&wall) && g.log.iter().any(|e| matches!(e, Event::ZoneChanged { was, to: Zone::Exile, .. } if *was == wall))));

    // "Whenever a creature deals damage to this creature, this creature deals that much
    // damage to that creature.": the text says "this creature" again of its source
    let mut g = game(&[(
        "Tephraderm",
        "Creature — Beast",
        "Whenever a creature deals damage to this creature, this creature deals that much damage to that creature.\nWhenever a spell deals damage to this creature, this creature deals that much damage to that spell's controller.",
        // no power here, so that what damaged it is still there to be dealt damage
        Some((0, 5)),
    )]);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let tephraderm = g.put(1, 3, Zone::Battlefield);
    fight(&mut g, bears, tephraderm);
    through_damage(&mut g);
    cite!(["608.2k"]; assert_eq!(noncombat_damage(&g), vec![(tephraderm, Target::Object(bears), 2)], "not itself"));
}

#[test]
fn r608_2k_that_creature_is_the_one_called_by_the_same_noun() {
    // "Whenever a Sliver deals combat damage to a creature, destroy that creature."
    let mut g = game(&[(
        "Toxin Sliver",
        "Creature — Sliver",
        "Whenever a Sliver deals combat damage to a creature, destroy that creature. It can't be regenerated.",
        Some((3, 3)),
    )]);
    let sliver = g.put(0, 3, Zone::Battlefield);
    let wall = g.put(1, WALL, Zone::Battlefield);
    fight(&mut g, sliver, wall);
    through_damage(&mut g);
    cite!(["608.2k"]; assert!(g.battlefield.contains(&sliver), "not the Sliver"));
    cite!(["608.2k"]; assert!(g.log.contains(&Event::Destroyed { object: wall })));
}

#[test]
fn r608_2k_that_creature_is_the_vampire_whichever_way_the_block_went() {
    let stake = (
        "Wooden Stake",
        "Artifact — Equipment",
        "Equipped creature gets +1/+0.\nWhenever equipped creature blocks or becomes blocked by a Vampire, destroy that creature. It can't be regenerated.\nEquip {1}",
        None,
    );
    // becomes blocked by a Vampire
    let mut g = game(&[stake]);
    let wooden = g.put(0, 3, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let vampire = g.put(1, VAMPIRE, Zone::Battlefield);
    assert!(g.attach(wooden, Target::Object(bears)));
    fight(&mut g, bears, vampire);
    assert_eq!(g.stack.len(), 1);
    all_pass(&mut g);
    cite!(["608.2k"]; assert!(g.log.contains(&Event::Destroyed { object: vampire })));
    cite!(["608.2k"]; assert!(g.battlefield.contains(&bears), "not the equipped creature"));

    // blocks a Vampire
    let mut g = game(&[stake]);
    let wooden = g.put(1, 3, Zone::Battlefield);
    let wall = g.put(1, WALL, Zone::Battlefield);
    let vampire = g.put(0, VAMPIRE, Zone::Battlefield);
    assert!(g.attach(wooden, Target::Object(wall)));
    fight(&mut g, vampire, wall);
    assert_eq!(g.stack.len(), 1);
    all_pass(&mut g);
    cite!(["608.2k"]; assert!(g.log.contains(&Event::Destroyed { object: vampire })));

    // blocks, or becomes blocked by, anything else: no trigger
    let mut g = game(&[stake]);
    let wooden = g.put(1, 3, Zone::Battlefield);
    let wall = g.put(1, WALL, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    assert!(g.attach(wooden, Target::Object(wall)));
    fight(&mut g, bears, wall);
    cite!(["603.2"]; assert!(g.stack.is_empty(), "the creature it blocked is not a Vampire"));
    let mut g = game(&[stake]);
    let wooden = g.put(0, 3, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let wall = g.put(1, WALL, Zone::Battlefield);
    assert!(g.attach(wooden, Target::Object(bears)));
    fight(&mut g, bears, wall);
    cite!(["603.2"]; assert!(g.stack.is_empty(), "the creature that blocked it is not a Vampire"));
}

#[test]
fn r508_3e_a_player_attacking_a_named_defender_is_not_yet_supported() {
    // CR 508.3e defines this trigger. The engine does not yet match a player's
    // attack declaration to the named defender, so admission must refuse it.
    for text in [
        "Enchant opponent\nWhenever you attack enchanted player, that player loses 2 life. You draw a card and gain 2 life.",
        "Enchant opponent\nWhenever a player attacks you, you may attach this Aura to that player.",
    ] {
        let def = CardDef::new("Archnemesis", "{1}", "Enchantment — Aura", text, None).unwrap();
        assert_eq!(check(&def).unwrap_err().0, "trigger: a player attacks whom the pattern names", "{text}");
    }
    // a creature's attack names what it attacks (508.1b)
    let def = CardDef::new("Watcher", "{1}", "Enchantment", "Whenever a creature attacks you, you gain 1 life.", None).unwrap();
    check(&def).unwrap();
    // and a player's attack that names nobody is the declaration
    let def = CardDef::new("Raider", "{1}", "Enchantment", "Whenever you attack, you gain 1 life.", None).unwrap();
    check(&def).unwrap();
}
