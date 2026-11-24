//! Following instructions (CR 608.2), rule by rule: the steps in the order written, the
//! choices a player makes while an effect is applied, and what one instruction leaves for the
//! next. Every card is read from its printed text.

use crate::support::{cite, paid};
use game::card::CardDef;
use game::play::check;
use game::state::{Answer, Choice, Decision};
use game::{Event, Game, Move, ObjId, Target, Zone};
use ir::{Part, Verb};

const MOUNTAIN: usize = 0;
const BEARS: usize = 1;
const RAIDER: usize = 2;
const MIND_ROT: usize = 3;
const SWAMP: usize = 4;
const INNOCENT_BLOOD: usize = 5;
const LOOTER: usize = 6;
const HERETIC: usize = 7;
const RUST: usize = 8;
const SOL_RING: usize = 9;
const EDICT: usize = 10;
const GIANT: usize = 11;
const IRE: usize = 12;
const ELIXIR: usize = 13;
const SOVEREIGN: usize = 14;
const SCHOLAR: usize = 15;
const SURVIVOR: usize = 16;
const ISLAND: usize = 17;
const FANATIC: usize = 18;
const TORCHRUNNER: usize = 19;
const SEIZAN: usize = 20;
const VIGILANTE: usize = 21;
const INFERNO: usize = 22;
const BONDS: usize = 23;
const VIPER: usize = 24;
const UNICORN: usize = 25;
const WOUND: usize = 26;
const FOREST: usize = 27;
const BRAVERY: usize = 28;
const WISDOM: usize = 29;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card(
            "Keldon Raider",
            "{2}{R}{R}",
            "Creature — Human Warrior",
            "When this creature enters, you may discard a card. If you do, draw a card.",
            Some((4, 3)),
        ),
        card("Mind Rot", "{2}{B}", "Sorcery", "Target player discards two cards.", None),
        card("Swamp", "", "Basic Land — Swamp", "({T}: Add {B}.)", None),
        card("Innocent Blood", "{B}", "Sorcery", "Each player sacrifices a creature of their choice.", None),
        card("Merfolk Looter", "{1}{U}", "Creature — Merfolk Rogue", "{T}: Draw a card, then discard a card.", Some((1, 1))),
        card(
            "Vedalken Heretic",
            "{G}{U}",
            "Creature — Vedalken Rogue",
            "Whenever this creature deals damage to an opponent, you may draw a card.",
            Some((1, 1)),
        ),
        card(
            "Rust Elemental",
            "{4}",
            "Artifact Creature — Elemental",
            "Flying\nAt the beginning of your upkeep, sacrifice another artifact. If you can't, tap this creature and you lose 4 life.",
            Some((4, 4)),
        ),
        card("Sol Ring", "{1}", "Artifact", "{T}: Add {C}{C}.", None),
        card("Cruel Edict", "{1}{B}", "Sorcery", "Target opponent sacrifices a creature of their choice.", None),
        card("Hill Giant", "{3}{R}", "Creature — Giant", "", Some((3, 3))),
        card(
            "Giant's Ire",
            "{3}{R}",
            "Kindred Sorcery — Giant",
            "Giant's Ire deals 4 damage to target player or planeswalker. If you control a Giant, draw a card.",
            None,
        ),
        card(
            "Cosmos Elixir",
            "{4}",
            "Artifact",
            "At the beginning of your end step, draw a card if your life total is greater than your starting life total. Otherwise, you gain 2 life.",
            None,
        ),
        card(
            "Sphinx Sovereign",
            "{4}{W}{U}{U}{B}",
            "Artifact Creature — Sphinx",
            "Flying\nAt the beginning of your end step, you gain 3 life if this creature is untapped. Otherwise, each opponent loses 3 life.",
            Some((6, 6)),
        ),
        card(
            "Scholar of Stars",
            "{3}{U}",
            "Creature — Human Artificer",
            "When this creature enters, if you control an artifact, draw a card.",
            Some((3, 2)),
        ),
        card(
            "Cautious Survivor",
            "{3}{G}",
            "Creature — Elf Survivor",
            "Survival — At the beginning of your second main phase, if this creature is tapped, you gain 2 life.",
            Some((4, 4)),
        ),
        card("Island", "", "Basic Land — Island", "({T}: Add {U}.)", None),
        card("Mogg Fanatic", "{R}", "Creature — Goblin", "Sacrifice this creature: It deals 1 damage to any target.", Some((1, 1))),
        card(
            "Mudbutton Torchrunner",
            "{2}{R}",
            "Creature — Goblin Warrior",
            "When this creature dies, it deals 3 damage to any target.",
            Some((1, 1)),
        ),
        card(
            "Seizan, Perverter of Truth",
            "{3}{B}{B}",
            "Legendary Creature — Demon Spirit",
            "At the beginning of each player's upkeep, that player loses 2 life and draws two cards.",
            Some((6, 5)),
        ),
        card(
            "Somberwald Vigilante",
            "{R}",
            "Creature — Human Warrior",
            "Whenever this creature becomes blocked by a creature, this creature deals 1 damage to that creature.",
            Some((1, 1)),
        ),
        card(
            "Inferno Elemental",
            "{4}{R}{R}",
            "Creature — Elemental",
            "Whenever this creature blocks or becomes blocked by a creature, this creature deals 3 damage to that creature.",
            Some((4, 4)),
        ),
        card(
            "Polluted Bonds",
            "{3}{B}{B}",
            "Enchantment",
            "Whenever a land an opponent controls enters, that player loses 2 life and you gain 2 life.",
            None,
        ),
        card(
            "Scalding Viper",
            "{1}{R}",
            "Creature — Elemental Snake",
            "Whenever an opponent casts a spell with mana value 3 or less, this creature deals 1 damage to that player.",
            Some((2, 1)),
        ),
        card(
            "Good-Fortune Unicorn",
            "{1}{G}{W}",
            "Creature — Unicorn",
            "Whenever another creature you control enters, put a +1/+1 counter on that creature.",
            Some((2, 2)),
        ),
        card(
            "Self-Inflicted Wound",
            "{1}{B}",
            "Sorcery",
            "Target opponent sacrifices a green or white creature of their choice. If that player does, they lose 2 life.",
            None,
        ),
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        card(
            "Pippin's Bravery",
            "{G}",
            "Instant",
            "You may sacrifice a Food. If you do, target creature gets +4/+4 until end of turn. Otherwise, that creature gets +2/+2 until end of turn.",
            None,
        ),
        card("Words of Wisdom", "{1}{U}", "Instant", "You draw two cards, then each other player draws a card.", None),
    ]
}

/// Pass priority until something is on the stack or the game waits for a decision.
fn pass_until_something(g: &mut Game) {
    for _ in 0..60 {
        if g.decision.is_some() || !g.stack.is_empty() {
            return;
        }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("nothing happened");
}

fn game() -> Game {
    Game::new(defs(), 2, 20)
}

fn all_pass(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}

/// Put lands onto the battlefield for a player and tap them for mana.
fn mana(g: &mut Game, player: usize, land: usize, n: usize) {
    for _ in 0..n {
        let l = g.put(player, land, Zone::Battlefield);
        g.act(player, Move::Activate { source: l, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
}

/// Player 0 casts this card from hand with this mana and it resolves, or waits for a choice.
fn cast(g: &mut Game, def: usize, land: usize, lands: usize, targets: Vec<Vec<Target>>, spending: &str) -> ObjId {
    let card = g.put(0, def, Zone::Hand);
    mana(g, 0, land, lands);
    g.act(0, paid(spending, Move::Cast { card, targets })).unwrap();
    let spell = *g.stack.last().unwrap();
    all_pass(g);
    spell
}

fn may(player: usize) -> Option<Decision> {
    Some(Decision::Choose { player, choice: Choice::May { possible: true } })
}

fn which(player: usize, verb: Verb, among: Vec<ObjId>, count: usize) -> Option<Decision> {
    Some(Decision::Choose { player, choice: Choice::Objects { verb, to: None, among, count } })
}

fn choose(g: &mut Game, player: usize, objects: &[ObjId]) -> Result<(), String> {
    g.act(player, Move::Choose(Answer::Objects(objects.to_vec())))
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r608_2d_a_choice_an_effect_offers_is_made_while_it_is_applied() {
    let mut g = game();
    g.put(0, BEARS, Zone::Library);
    let heretic = g.put(0, HERETIC, Zone::Battlefield);
    // it attacks unblocked and deals damage: its ability triggers and goes on the stack
    all_pass(&mut g);
    all_pass(&mut g);
    g.act(0, Move::Attack(vec![(heretic, Target::Player(1))])).unwrap();
    all_pass(&mut g);
    all_pass(&mut g);
    cite!(["608.2d"]; assert_eq!((g.part, g.stack.len(), g.decision.clone()), (Part::CombatDamage, 1, None), "not chosen as it goes on the stack"));
    // as it resolves the game waits for the choice, with the ability still on the stack
    all_pass(&mut g);
    cite!(["608.2d"]; assert_eq!(g.decision, may(0)));
    assert_eq!((g.stack.len(), g.players[0].hand.len()), (1, 0), "nothing has happened yet");
    assert_eq!(g.resolving().map(|r| r.object), g.stack.last().copied());
    // only that choice can be made, and only by that player
    cite!(["608.2d"]; refused("a decision is pending: a choice of the effect being applied (608.2d)"): g.act(0, Move::Pass));
    crate::support::refused(g.act(1, Move::Choose(Answer::Yes)), "that decision is another player's");
    cite!(["608.2d"]; refused("the choice is whether to do it: yes or no (608.2d)"): g.act(0, Move::Choose(Answer::Objects(vec![]))));
    let declined = g.clone();
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    assert_eq!((g.players[0].hand.len(), g.stack.len(), g.resolving().cloned()), (1, 0, None));
    cite!(["117.3b"]; assert_eq!(g.priority, 0, "the active player receives priority after it resolves (117.3b)"));
    // a game waiting for a choice is a value like any other: copied, it goes on the other way
    let mut g = declined;
    g.act(0, Move::Choose(Answer::No)).unwrap();
    assert_eq!((g.players[0].hand.len(), g.stack.len(), g.resolving().cloned()), (0, 0, None));
}

#[test]
fn r118_12_if_you_do_asks_what_became_of_the_instruction_before() {
    // "you may discard a card. If you do, draw a card."
    let setup = |hand: usize| {
        let mut g = game();
        g.put(0, BEARS, Zone::Library);
        let cards: Vec<ObjId> = (0..hand).map(|_| g.put(0, BEARS, Zone::Hand)).collect();
        cast(&mut g, RAIDER, MOUNTAIN, 4, vec![], "RRRR");
        // the Raider entered; its ability triggered and resolves
        assert_eq!(g.stack.len(), 1);
        all_pass(&mut g);
        (g, cards)
    };
    // yes: the player chooses which card, discards it, and draws
    let (mut g, cards) = setup(2);
    assert_eq!(g.decision, may(0));
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    cite!(["701.9b"]; assert_eq!(g.decision, which(0, Verb::Discard, cards.clone(), 1)));
    assert_eq!(g.players[0].hand.len(), 2, "nothing is discarded before the choice");
    choose(&mut g, 0, &cards[..1]).unwrap();
    cite!(["118.12"]; assert_eq!((g.players[0].graveyard.len(), g.players[0].hand.len(), g.decision.clone()), (1, 2, None), "discarded one, drew one"));
    // no: nothing is discarded and no card is drawn
    let (mut g, _) = setup(2);
    g.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["118.12"]; assert_eq!((g.players[0].graveyard.len(), g.players[0].hand.len(), g.stack.len()), (0, 2, 0)));
    // with one card there is no choice of which
    let (mut g, _) = setup(1);
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    cite!(["118.12"]; assert_eq!((g.players[0].graveyard.len(), g.players[0].hand.len(), g.decision.clone()), (1, 1, None)));
}

#[test]
fn r608_2d_a_player_cant_choose_an_option_that_is_impossible() {
    // with no card in hand "you may discard a card" is not offered, and "if you do" is false
    let mut g = game();
    g.put(0, BEARS, Zone::Library);
    cast(&mut g, RAIDER, MOUNTAIN, 4, vec![], "RRRR");
    all_pass(&mut g);
    cite!(["608.2d"]; assert_eq!((g.decision.clone(), g.stack.len()), (None, 0), "the ability resolved without asking"));
    cite!(["118.12"]; assert_eq!(g.players[0].hand.len(), 0, "and no card was drawn"));
}

#[test]
fn r701_9b_the_player_who_discards_chooses_the_cards() {
    let mut g = game();
    let cards: Vec<ObjId> = (0..3).map(|_| g.put(1, BEARS, Zone::Hand)).collect();
    let mine = g.put(0, BEARS, Zone::Hand);
    let spell = cast(&mut g, MIND_ROT, SWAMP, 3, vec![vec![Target::Player(1)]], "BBB");
    cite!(["701.9b"]; assert_eq!(g.decision, which(1, Verb::Discard, cards.clone(), 2)));
    assert!(g.stack.contains(&spell), "the spell is on the stack while its instructions wait");
    cite!(["701.9b"]; refused("that decision is another player's"): choose(&mut g, 0, &cards[..2]));
    cite!(["608.2d"]; refused("choose exactly 2 of them to discard (608.2d)"): choose(&mut g, 1, &cards[..1]));
    cite!(["608.2d"]; refused("choose each object to discard once (608.2d)"): choose(&mut g, 1, &[cards[0], cards[0]]));
    cite!(["701.9a"]; refused("a chosen object is not among the offered objects to discard (608.2d)"): choose(&mut g, 1, &[cards[0], mine]));
    choose(&mut g, 1, &cards[1..]).unwrap();
    cite!(["404.3"]; ok: choose(&mut g,1,&[cards[1],cards[2]]));
    cite!(["701.9a"]; assert_eq!(g.players[1].hand, vec![cards[0]]));
    // the spell is put into its owner's graveyard as the last part of resolving (608.2n)
    cite!(["608.2n"]; assert_eq!((g.stack.len(), g.players[0].graveyard.len()), (0, 1)));

    // with no more cards than asked there is nothing to choose: as much as possible (609.3)
    let mut g = game();
    g.put(1, BEARS, Zone::Hand);
    cast(&mut g, MIND_ROT, SWAMP, 3, vec![vec![Target::Player(1)]], "BBB");
    cite!(["609.3"]; assert_eq!((g.decision.clone(), g.players[1].hand.len(), g.players[1].graveyard.len()), (None, 0, 1)));
}

#[test]
fn r608_2c_instructions_are_followed_in_the_order_written() {
    // "Draw a card, then discard a card": the card drawn is one of those to choose from
    let mut g = game();
    let looter = g.put(0, LOOTER, Zone::Battlefield);
    let held = g.put(0, BEARS, Zone::Hand);
    let top = g.put(0, GIANT, Zone::Library);
    g.act(0, Move::Activate { source: looter, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    all_pass(&mut g);
    let Some(Decision::Choose { player: 0, choice: Choice::Objects { among, count: 1, .. } }) = g.decision.clone() else {
        panic!("the discard waits for a choice: {:?}", g.decision)
    };
    cite!(["608.2c"]; assert_eq!(among.len(), 2));
    cite!(["400.7"]; assert!(among.contains(&held) && !among.contains(&top), "the card drawn is a new object in the hand (400.7)"));
    let drawn = *among.iter().find(|c| **c != held).unwrap();
    assert_eq!(g.obj(drawn).chars.name, "Hill Giant");
    choose(&mut g, 0, &[drawn]).unwrap();
    assert_eq!(g.players[0].hand, vec![held]);
}

#[test]
fn r608_2e_each_player_chooses_in_turn_order_and_then_it_happens_at_once() {
    let mut g = game();
    let mine: Vec<ObjId> = vec![g.put(0, BEARS, Zone::Battlefield), g.put(0, GIANT, Zone::Battlefield)];
    let theirs: Vec<ObjId> = vec![g.put(1, BEARS, Zone::Battlefield), g.put(1, GIANT, Zone::Battlefield)];
    cast(&mut g, INNOCENT_BLOOD, SWAMP, 1, vec![], "B");
    // the active player first (101.4); each sacrifices only what they control (701.21a)
    cite!(["101.4", "701.21a"]; assert_eq!(g.decision, which(0, Verb::Sacrifice, mine.clone(), 1)));
    cite!(["701.21a"]; refused("a chosen object is not among the offered objects to sacrifice (608.2d)"): choose(&mut g, 0, &theirs[..1]));
    choose(&mut g, 0, &mine[..1]).unwrap();
    cite!(["101.4"]; assert_eq!(g.decision, which(1, Verb::Sacrifice, theirs.clone(), 1)));
    cite!(["608.2e"]; assert!(g.battlefield.contains(&mine[0]), "nothing is sacrificed until every player has chosen"));
    choose(&mut g, 1, &theirs[1..]).unwrap();
    assert!(!g.battlefield.contains(&mine[0]) && !g.battlefield.contains(&theirs[1]));
    assert!(g.battlefield.contains(&mine[1]) && g.battlefield.contains(&theirs[0]));
    let sacrificed = g.log.iter().filter(|e| matches!(e, Event::Sacrificed { .. })).count();
    assert_eq!(sacrificed, 2);
}

#[test]
fn r701_21a_a_player_told_to_sacrifice_chooses_among_what_they_control() {
    let mut g = game();
    g.put(0, BEARS, Zone::Battlefield);
    let theirs = g.put(1, BEARS, Zone::Battlefield);
    cast(&mut g, EDICT, SWAMP, 2, vec![vec![Target::Player(1)]], "BB");
    // one creature: no choice, and never one of another player's
    assert_eq!(g.decision, None);
    cite!(["701.21a"]; assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Sacrificed { .. })).collect::<Vec<_>>(), vec![&Event::Sacrificed { player: 1, object: theirs }]));
    assert_eq!(g.battlefield.iter().filter(|o| g.obj(**o).chars.name == "Grizzly Bears").count(), 1);
}

#[test]
fn r608_2c_a_sentence_led_by_if_is_followed_only_if_the_condition_holds() {
    // "… deals 4 damage to target player or planeswalker. If you control a Giant, draw a card."
    let mut g = game();
    g.put(0, BEARS, Zone::Library);
    cast(&mut g, IRE, MOUNTAIN, 4, vec![vec![Target::Player(1)]], "RRRR");
    cite!(["608.2c"]; assert_eq!((g.players[1].life, g.players[0].hand.len()), (16, 0), "no Giant: the spell on the stack is not a permanent"));
    let mut g = game();
    g.put(0, BEARS, Zone::Library);
    g.put(0, GIANT, Zone::Battlefield);
    cast(&mut g, IRE, MOUNTAIN, 4, vec![vec![Target::Player(1)]], "RRRR");
    cite!(["608.2c"]; assert_eq!((g.players[1].life, g.players[0].hand.len()), (16, 1)));
    // a Giant another player controls is not one you control
    let mut g = game();
    g.put(0, BEARS, Zone::Library);
    g.put(1, GIANT, Zone::Battlefield);
    cast(&mut g, IRE, MOUNTAIN, 4, vec![vec![Target::Player(1)]], "RRRR");
    assert_eq!(g.players[0].hand.len(), 0);
}

#[test]
fn r608_2c_otherwise_is_followed_when_the_if_before_it_did_not_hold() {
    // "draw a card if your life total is greater than your starting life total. Otherwise,
    // you gain 2 life."
    let end_step = |life_gained: u32| {
        let mut g = game();
        g.put(0, BEARS, Zone::Library);
        g.put(0, ELIXIR, Zone::Battlefield);
        g.gain_life(0, life_gained).unwrap();
        pass_until_something(&mut g);
        assert_eq!((g.part, g.stack.len()), (Part::End, 1));
        all_pass(&mut g);
        (g.players[0].life, g.players[0].hand.len())
    };
    cite!(["608.2c", "103.4"]; assert_eq!(end_step(0), (22, 0), "20 is not greater than the starting 20 (103.4): 2 life, no card"));
    cite!(["608.2c"]; assert_eq!(end_step(1), (21, 1), "21 is: a card, and no life"));

    // "you gain 3 life if this creature is untapped. Otherwise, each opponent loses 3 life."
    let mut g = game();
    let sovereign = g.put(0, SOVEREIGN, Zone::Battlefield);
    pass_until_something(&mut g);
    g.act(0, Move::Attack(vec![])).unwrap();
    pass_until_something(&mut g);
    assert_eq!(g.part, Part::End);
    all_pass(&mut g);
    cite!(["608.2c"]; assert_eq!((g.players[0].life, g.players[1].life), (23, 20)));
    let mut g = game();
    let sovereign2 = g.put(0, SOVEREIGN, Zone::Battlefield);
    pass_until_something(&mut g);
    g.act(0, Move::Attack(vec![(sovereign2, Target::Player(1))])).unwrap();
    pass_until_something(&mut g);
    assert_eq!((g.part, g.obj(sovereign2).tapped), (Part::End, true));
    all_pass(&mut g);
    cite!(["608.2c"]; assert_eq!((g.players[0].life, g.players[1].life), (20, 11), "6 in combat, then 3"));
    let _ = sovereign;
}

#[test]
fn every_sentence_is_checked_whichever_way_its_if_would_go() {
    // "… If you do, create a 7/7 green Giant creature token. Otherwise, create three Food
    // tokens." (Giant Opportunity): Food tokens are made (111.10b), so the card runs. With a
    // predefined token the engine does not make yet in the branch not taken (Gold, 111.10c),
    // it says so before the card is cast
    let text = "You may sacrifice two Foods. If you do, create a 7/7 green Giant creature token. Otherwise, create three Food tokens.";
    let d = CardDef::new("Giant Opportunity", "{2}{G}", "Sorcery", text, None).unwrap();
    assert_eq!(check(&d), Ok(()));
    let d = CardDef::new("Giant Opportunity", "{2}{G}", "Sorcery", &text.replace("Food tokens", "Gold tokens"), None).unwrap();
    assert!(check(&d).unwrap_err().0.contains("token: Gold"));
}

#[test]
fn r608_2c_it_is_what_that_creature_has_just_named() {
    // "Whenever this creature deals damage to a creature, tap that creature and it doesn't
    // untap during its controller's next untap step.": "it" is the creature dealt damage,
    // which "that creature" has just named, not the creature whose ability this is
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    let defs = vec![
        card(
            "Matsu-Tribe Sniper",
            "{1}{G}",
            "Creature — Snake Warrior Archer",
            "{T}: This creature deals 1 damage to target creature with flying.\nWhenever this creature deals damage to a creature, tap that creature and it doesn't untap during its controller's next untap step.",
            Some((1, 1)),
        ),
        card("Serra Angel", "{3}{W}{W}", "Creature — Angel", "Flying\nVigilance (Attacking doesn't cause this creature to tap.)", Some((4, 4))),
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
    ];
    for d in &defs {
        check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    let mut g = Game::new(defs, 2, 20);
    for player in [0, 1] {
        for _ in 0..4 {
            g.put(player, 2, Zone::Library);
        }
    }
    let sniper = g.put(0, 0, Zone::Battlefield);
    let angel = g.put(1, 1, Zone::Battlefield);
    g.act(0, Move::Activate { source: sniper, ability: 0, targets: vec![vec![Target::Object(angel)]], paying: vec![] }).unwrap();
    all_pass(&mut g);
    assert_eq!((g.obj(angel).damage, g.stack.len()), (1, 1), "the damage triggered its second ability");
    all_pass(&mut g);
    cite!(["608.2c"]; assert!(g.obj(angel).tapped));
    let to_turn = |g: &mut Game, turn: u32| {
        while g.turn < turn {
            match g.decision {
                Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
                _ => g.act(g.priority, Move::Pass).unwrap(),
            }
        }
    };
    to_turn(&mut g, 2);
    cite!(["502.3"]; assert!(g.obj(angel).tapped, "its controller's next untap step has passed: it did not untap"));
    to_turn(&mut g, 3);
    cite!(["502.3"]; assert!(!g.obj(sniper).tapped, "the Sniper, tapped for its cost, untaps as usual"));
    to_turn(&mut g, 4);
    assert!(!g.obj(angel).tapped, "and the Angel in the untap step after");
}

#[test]
fn what_only_a_creature_has_is_said_of_the_creature_in_a_list_of_types() {
    // "Destroy target artifact, battle, enchantment, or creature with flying.": any artifact,
    // any enchantment, and a creature that has flying; "with flying" was read of all of
    // them, so that only what flies could be targeted. Likewise "… or creature with power 4
    // or greater."
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    let defs = vec![
        card("Atraxa's Fall", "{1}{G}", "Sorcery", "Destroy target artifact, battle, enchantment, or creature with flying.", None),
        card("Exorcise", "{1}{W}", "Sorcery", "Exile target artifact, enchantment, or creature with power 4 or greater.", None),
        card("Sliver Construct", "{3}", "Artifact Creature — Sliver Construct", "", Some((2, 2))),
        card("Glorious Anthem", "{1}{W}{W}", "Enchantment", "Creatures you control get +1/+1.", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Storm Crow", "{1}{U}", "Creature — Bird", "Flying (This creature can't be blocked except by creatures with flying or reach.)", Some((1, 2))),
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        card("Plains", "", "Basic Land — Plains", "({T}: Add {W}.)", None),
    ];
    for d in &defs {
        check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    let board = |spell: usize, land: usize| {
        let mut g = Game::new(defs.clone(), 2, 20);
        let [construct, anthem, bears, crow] = [2, 3, 4, 5].map(|def| g.put(1, def, Zone::Battlefield));
        for _ in 0..2 {
            let land = g.put(0, land, Zone::Battlefield);
            g.act(0, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
        }
        (g.put(0, spell, Zone::Hand), [construct, anthem, bears, crow], g)
    };
    let at = |o: ObjId| vec![vec![Target::Object(o)]];
    for target in 0..4 {
        // the Fall: not the Bears, which have no flying; the Construct though it has none
        let (card, permanents, mut g) = board(0, 6);
        // Uncited (card text): "target artifact, battle, enchantment, or creature with flying": "with flying" qualifies only "creature"
        assert_eq!(g.act(0, paid("GG", Move::Cast { card, targets: at(permanents[target]) })).is_ok(), target != 2, "Atraxa's Fall at {target}");
        // Exorcise: neither creature of power less than 4; the Construct as an artifact
        let (card, permanents, mut g) = board(1, 7);
        // Uncited (card text): "target artifact, enchantment, or creature with power 4 or greater": the power qualifies only "creature"
        assert_eq!(g.act(0, paid("WW", Move::Cast { card, targets: at(permanents[target]) })).is_ok(), target < 2, "Exorcise at {target}");
    }
}

#[test]
fn what_a_target_player_would_do_is_checked_before_any_player_is_chosen() {
    // "Target player discards any number of cards at random.": the engine does not discard
    // a number of cards at random that is no number, and says so before the card is cast,
    // when there is no target player to walk the instruction for; it was found out as the
    // spell resolved
    let d = CardDef::new("Probe", "{B}{B}", "Sorcery", "Target player discards any number of cards at random.", None).unwrap();
    assert!(check(&d).unwrap_err().0.contains("at random"));
    let d = CardDef::new("Mind Rot", "{2}{B}", "Sorcery", "Target player discards two cards.", None).unwrap();
    check(&d).unwrap();
}

#[test]
fn r603_4_an_ability_with_an_intervening_if_triggers_only_if_it_is_true() {
    // "When this creature enters, if you control an artifact, draw a card."
    let enter = |artifact: bool| {
        let mut g = game();
        g.put(0, BEARS, Zone::Library);
        let ring = artifact.then(|| g.put(0, SOL_RING, Zone::Battlefield));
        cast(&mut g, SCHOLAR, ISLAND, 4, vec![], "UUUU");
        (g, ring)
    };
    let (g, _) = enter(false);
    cite!(["603.4"]; assert_eq!((g.stack.len(), g.players[0].hand.len()), (0, 0), "it did not trigger"));
    let (mut g, _) = enter(true);
    assert_eq!(g.stack.len(), 1);
    all_pass(&mut g);
    assert_eq!(g.players[0].hand.len(), 1);
    // checked again as it resolves: not true any more, it does nothing (608.2a)
    let (mut g, ring) = enter(true);
    g.destroy(ring.unwrap());
    let ability = *g.stack.last().unwrap();
    all_pass(&mut g);
    cite!(["603.4", "608.2a"]; assert_eq!((g.stack.len(), g.players[0].hand.len()), (0, 0)));
    assert!(g.log.contains(&Event::Fizzled { spell: ability }));

    // "At the beginning of your second main phase, if this creature is tapped, you gain 2 life."
    let second_main = |attacks: bool| {
        let mut g = game();
        let survivor = g.put(0, SURVIVOR, Zone::Battlefield);
        pass_until_something(&mut g);
        g.act(0, Move::Attack(if attacks { vec![(survivor, Target::Player(1))] } else { vec![] })).unwrap();
        for _ in 0..12 {
            if g.part == Part::PostcombatMain {
                break;
            }
            g.act(g.priority, Move::Pass).unwrap();
        }
        assert_eq!(g.part, Part::PostcombatMain);
        let triggered = g.stack.len();
        all_pass(&mut g);
        (triggered, g.players[0].life)
    };
    cite!(["603.4"]; assert_eq!(second_main(false), (0, 20), "untapped: no trigger"));
    cite!(["603.4"]; assert_eq!(second_main(true), (1, 22)));
}

/// The source of the last damage dealt, and what it was dealt to.
fn last_damage(g: &Game) -> Option<(ObjId, Target, u32)> {
    g.log.iter().rev().find_map(|e| match e {
        Event::DamageDealt { source, to, amount, .. } => Some((*source, *to, *amount)),
        _ => None,
    })
}

#[test]
fn r113_7a_it_is_the_source_as_it_most_recently_existed() {
    // "Sacrifice this creature: It deals 1 damage to any target.": the cost names it (608.2k)
    let mut g = game();
    let fanatic = g.put(0, FANATIC, Zone::Battlefield);
    g.act(0, Move::Activate { source: fanatic, ability: 0, targets: vec![vec![Target::Player(1)]], paying: vec![] }).unwrap();
    assert!(!g.battlefield.contains(&fanatic));
    all_pass(&mut g);
    assert_eq!(g.players[1].life, 19);
    cite!(["113.7a"]; assert_eq!(last_damage(&g), Some((fanatic, Target::Player(1), 1)), "the creature that is no longer there dealt it"));

    // "When this creature dies, it deals 3 damage to any target.": the trigger condition does
    let mut g = game();
    let torchrunner = g.put(0, TORCHRUNNER, Zone::Battlefield);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    g.destroy(torchrunner);
    // its ability goes on the stack the next time a player would receive priority (603.3)
    g.act(0, Move::Pass).unwrap();
    assert!(matches!(g.decision, Some(Decision::Targets { player: 0, .. })), "{:?}", g.decision);
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(bears)]])).unwrap();
    all_pass(&mut g);
    cite!(["113.7a"]; assert_eq!(last_damage(&g), Some((torchrunner, Target::Object(bears), 3)), "the creature in the graveyard dealt it"));
    assert!(g.log.contains(&Event::Destroyed { object: bears }));
}

#[test]
fn r608_2k_an_effect_refers_to_what_the_trigger_condition_referred_to() {
    // "becomes blocked by a creature, this creature deals 1 damage to that creature"
    let mut g = game();
    let vigilante = g.put(0, VIGILANTE, Zone::Battlefield);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    pass_until_something(&mut g);
    g.act(0, Move::Attack(vec![(vigilante, Target::Player(1))])).unwrap();
    pass_until_something(&mut g);
    g.act(1, Move::Block(vec![(giant, vigilante)])).unwrap();
    assert_eq!(g.stack.len(), 1);
    all_pass(&mut g);
    cite!(["608.2k"]; assert_eq!(last_damage(&g), Some((vigilante, Target::Object(giant), 1)), "the creature that blocked it, not itself"));

    // "blocks or becomes blocked by a creature": when it blocks, the creature it blocks
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let inferno = g.put(1, INFERNO, Zone::Battlefield);
    pass_until_something(&mut g);
    g.act(0, Move::Attack(vec![(bears, Target::Player(1))])).unwrap();
    pass_until_something(&mut g);
    g.act(1, Move::Block(vec![(inferno, bears)])).unwrap();
    all_pass(&mut g);
    cite!(["608.2k"]; assert_eq!(last_damage(&g), Some((inferno, Target::Object(bears), 3))));
    // and when it becomes blocked, the creature that blocks it: the subject of the trigger
    // condition is the one of the two events that happened
    let mut g = game();
    let inferno = g.put(0, INFERNO, Zone::Battlefield);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    pass_until_something(&mut g);
    g.act(0, Move::Attack(vec![(inferno, Target::Player(1))])).unwrap();
    pass_until_something(&mut g);
    g.act(1, Move::Block(vec![(giant, inferno)])).unwrap();
    all_pass(&mut g);
    cite!(["608.2k"]; assert_eq!(last_damage(&g), Some((inferno, Target::Object(giant), 3)), "not itself"));

    // "whenever another creature you control enters, put a +1/+1 counter on that creature"
    let mut g = game();
    let unicorn = g.put(0, UNICORN, Zone::Battlefield);
    cast(&mut g, BEARS, FOREST, 2, vec![], "GG");
    all_pass(&mut g);
    let bears = *g.battlefield.last().unwrap();
    cite!(["608.2k"]; assert_eq!((g.power(bears), g.power(unicorn)), (Some(3), Some(2))));
}

#[test]
fn r601_2c_a_target_is_named_whichever_of_the_instructions_is_followed() {
    // "You may sacrifice a Food. If you do, target creature gets +4/+4 until end of turn.
    // Otherwise, that creature gets +2/+2 until end of turn.": the target was chosen as the
    // spell was cast, and "that creature" is it though the sentence that says "target" is
    // not followed
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    cast(&mut g, BRAVERY, FOREST, 1, vec![vec![Target::Object(bears)]], "G");
    cite!(["601.2c", "118.12"]; assert_eq!((g.power(bears), g.toughness(bears)), (Some(4), Some(4)), "no Food to sacrifice: +2/+2"));
}

#[test]
fn r608_2c_each_other_player_is_every_player_but_the_one_whose_text_it_is() {
    // "You draw two cards, then each other player draws a card."
    let mut g = game();
    for player in 0..2 {
        for _ in 0..3 {
            g.put(player, BEARS, Zone::Library);
        }
    }
    cast(&mut g, WISDOM, ISLAND, 2, vec![], "UU");
    cite!(["608.2c"]; assert_eq!((g.players[0].hand.len(), g.players[1].hand.len()), (2, 1)));
}

#[test]
fn r608_2k_that_player_is_the_player_the_event_was_about() {
    // "At the beginning of each player's upkeep, that player …": the player whose step it is
    let mut g = game();
    g.put(0, SEIZAN, Zone::Battlefield);
    for _ in 0..3 {
        g.put(1, BEARS, Zone::Library);
    }
    pass_until_something(&mut g);
    g.act(0, Move::Attack(vec![])).unwrap();
    pass_until_something(&mut g);
    assert_eq!((g.active, g.part), (1, Part::Upkeep));
    all_pass(&mut g);
    // Uncited (card text): "At the beginning of each player's upkeep, that player loses 2 life and draws two cards": "that player" is the upkeep's player (608.2k is of objects)
    assert_eq!((g.players[1].life, g.players[1].hand.len(), g.players[0].life, g.players[0].hand.len()), (18, 2, 20, 0));

    // "Whenever an opponent casts a spell …, this creature deals 1 damage to that player"
    let mut g = game();
    let viper = g.put(1, VIPER, Zone::Battlefield);
    let card = g.put(0, BEARS, Zone::Hand);
    mana(&mut g, 0, FOREST, 2);
    g.act(0, paid("GG", Move::Cast { card, targets: vec![] })).unwrap();
    assert_eq!(g.stack.len(), 2, "the Viper's ability is on the stack above the spell");
    all_pass(&mut g);
    // Uncited (card text): "Whenever an opponent casts a spell …, this creature deals 1 damage to that player": the opponent who cast it (608.2k is of objects)
    assert_eq!(last_damage(&g), Some((viper, Target::Player(0), 1)));

    // "Whenever a land an opponent controls enters, that player loses 2 life and you gain 2
    // life": the player under whose control it entered
    let mut g = game();
    g.put(1, BONDS, Zone::Battlefield);
    let land = g.put(0, FOREST, Zone::Hand);
    g.act(0, Move::PlayLand(land)).unwrap();
    assert_eq!(g.stack.len(), 1);
    all_pass(&mut g);
    // Uncited (card text): "Whenever a land an opponent controls enters, that player loses 2 life": the land's controller (608.2k is of objects)
    assert_eq!((g.players[0].life, g.players[1].life), (18, 22));
    // its controller's own land is not one an opponent controls
    let mut g = game();
    g.put(0, BONDS, Zone::Battlefield);
    let land = g.put(0, FOREST, Zone::Hand);
    g.act(0, Move::PlayLand(land)).unwrap();
    cite!(["102.2"]; assert_eq!((g.stack.len(), g.players[0].life), (0, 20)));
}

#[test]
fn r608_2c_a_later_instruction_refers_to_what_an_earlier_one_named() {
    // "Target opponent sacrifices a green or white creature of their choice. If that player
    // does, they lose 2 life."
    let mut g = game();
    let bears = g.put(1, BEARS, Zone::Battlefield);
    g.put(1, GIANT, Zone::Battlefield);
    cast(&mut g, WOUND, SWAMP, 2, vec![vec![Target::Player(1)]], "BB");
    assert!(g.log.contains(&Event::Sacrificed { player: 1, object: bears }), "the green one: the red Giant is not what it asks for");
    cite!(["118.12"]; assert_eq!((g.players[1].life, g.players[0].life), (18, 20)));
    // with none to sacrifice, the player does not, and loses nothing
    let mut g = game();
    g.put(1, GIANT, Zone::Battlefield);
    cast(&mut g, WOUND, SWAMP, 2, vec![vec![Target::Player(1)]], "BB");
    cite!(["118.12"]; assert_eq!((g.battlefield.len(), g.players[1].life), (3, 20), "two Swamps and the Giant"));
}

#[test]
fn r118_12_if_you_cant_is_true_when_the_instruction_was_impossible() {
    // "sacrifice another artifact. If you can't, tap this creature and you lose 4 life."
    let upkeep = |g: &mut Game| {
        g.put(1, BEARS, Zone::Library);
        for _ in 0..40 {
            if g.decision.is_some() || !g.stack.is_empty() {
                break;
            }
            g.act(g.priority, Move::Pass).unwrap();
        }
        assert_eq!((g.active, g.part, g.stack.len()), (1, Part::Upkeep, 1), "its ability triggered in its controller's upkeep");
        all_pass(g);
    };
    // with another artifact it is sacrificed, and the rest is not followed
    let mut g = game();
    let rust = g.put(1, RUST, Zone::Battlefield);
    let ring = g.put(1, SOL_RING, Zone::Battlefield);
    upkeep(&mut g);
    assert!(g.log.contains(&Event::Sacrificed { player: 1, object: ring }));
    assert!(g.battlefield.contains(&rust), "another artifact: not itself");
    cite!(["118.12"]; assert_eq!((g.obj(rust).tapped, g.players[1].life), (false, 20)));
    // with none it can't be, and the rest is
    let mut g = game();
    let rust = g.put(1, RUST, Zone::Battlefield);
    upkeep(&mut g);
    assert!(!g.log.iter().any(|e| matches!(e, Event::Sacrificed { .. })));
    cite!(["118.12"]; assert_eq!((g.obj(rust).tapped, g.players[1].life), (true, 16)));
}
