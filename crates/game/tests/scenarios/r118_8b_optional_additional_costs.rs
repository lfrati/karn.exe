//! Optional additional costs (118.8b): announced paid or not as the spell is cast (601.2b),
//! paid with its other costs (601.2f-h), and read on resolution by "if this spell's
//! additional cost was paid" and its kind's phrases (607.2); offspring's token copy, except
//! it's 1/1 (702.175a, 707.9b), casualty's copy of the spell (702.153a, 707.10), teamwork
//! and "choose both instead" (702.194, 700.2a); blight, collect evidence and behold as costs
//! (701.68a, 701.59a, 701.4a).
use crate::support::{cite, paid as spend};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const PRINTED: usize = 0;
const BEAR: usize = 1;
const FORK: usize = 2;
const RABBIT: usize = 3;
const BOLT: usize = 4;
const GOBLIN: usize = 5;
const KICKED: usize = 6;
const ISLAND: usize = 7;
const HULK: usize = 8;
const GROWN: usize = 9;
const TEAMWORK: usize = 10;
const INSCRIPTION: usize = 11;

fn game() -> Game {
    let c = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    let defs = vec![
        c("Offering", "{0}", "Instant", "As an additional cost to cast this spell, you may sacrifice a creature.\nDraw a card. If this spell's additional cost was paid, you gain 2 life.", None),
        c("Bear", "{0}", "Creature — Bear", "", Some((2, 2))),
        c("Fork", "{0}", "Instant", "Copy target instant or sorcery spell. You may choose new targets for the copy.", None),
        c("Warren Rabbit", "{1}", "Creature — Rabbit", "Flying\nOffspring {2}", Some((3, 3))),
        c("Grim Bolt", "{0}", "Instant", "Casualty 2\nThis spell deals 1 damage to any target.", None),
        c("Goblin", "{0}", "Creature — Goblin", "", Some((1, 1))),
        c("Kicked Offering", "{0}", "Sorcery", "Kicker {1}\nCasualty 1\nYou gain 1 life. If this spell was kicked, draw a card.", None),
        c("Island", "", "Basic Land — Island", "", None),
        c("Grave Hulk", "{0}", "Creature — Zombie", "Casualty 1", Some((4, 4))),
        c("Grown Rabbit", "{1}", "Creature — Rabbit", "Offspring {1}\nThis creature enters with two +1/+1 counters on it.", Some((2, 2))),
        c("Team Effort", "{0}", "Instant", "Teamwork 3\nChoose one. If this spell was cast using teamwork, choose both instead.\n• You gain 2 life.\n• Draw a card.", None),
        c("Inscription", "{0}", "Instant", "Kicker {1}\nChoose one. If this spell was kicked, choose both instead.\n• You gain 2 life.\n• Draw a card.", None),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..8 { g.put(p, ISLAND, Zone::Library); } }
    g
}
fn paid(card: ObjId, fodder: ObjId, targets: Vec<Vec<Target>>) -> Move {
    Move::WithOptional(true, Box::new(Move::WithCosts(vec![vec![fodder]], Box::new(Move::Cast { card, targets }))))
}

/// Pass until the stack is empty, keeping each copy's targets.
fn resolve(g: &mut Game) {
    for _ in 0..40 {
        match &g.decision {
            Some(Decision::Choose { player, choice: Choice::Retarget { .. } }) => { let p = *player; g.act(p, Move::Choose(Answer::No)).unwrap(); }
            _ if g.stack.is_empty() => return,
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("the stack did not empty");
}
fn hand(g: &Game, p: usize) -> usize { g.players[p].hand.len() }
/// A card of this definition is in that player's graveyard (its object a new one, 400.7).
fn buried(g: &Game, p: usize, def: usize) -> bool { g.players[p].graveyard.iter().any(|o| g.obj(*o).card == Some(def)) }

#[test]
fn r118_8b_a_printed_optional_cost_paid_declined_or_unpayable() {
    // paid: the creature is sacrificed as it is cast, and the condition reads the choice
    let mut g = game();
    let card = g.put(0, PRINTED, Zone::Hand);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    g.act(0, paid(card, bear, vec![])).unwrap();
    cite!(["601.2h"]; assert!(buried(&g, 0, BEAR)));
    resolve(&mut g);
    cite!(["118.8b", "607.2"]; case: "paid"; assert_eq!((g.players[0].life, hand(&g, 0)), (22, 1)));

    // declined: nothing sacrificed, the condition is false
    let mut g = game();
    let card = g.put(0, PRINTED, Zone::Hand);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    g.act(0, Move::WithOptional(false, Box::new(Move::Cast { card, targets: vec![] }))).unwrap();
    resolve(&mut g);
    cite!(["118.8b"]; case: "declined"; assert_eq!(g.obj(bear).zone, Zone::Battlefield));
    cite!(["118.8b", "607.2"]; case: "declined"; assert_eq!((g.players[0].life, hand(&g, 0)), (20, 1)));
    // not announcing it is declining it
    let mut g = game();
    let card = g.put(0, PRINTED, Zone::Hand);
    g.put(0, BEAR, Zone::Battlefield);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    resolve(&mut g);
    cite!(["601.2b"]; case: "unannounced"; assert_eq!(g.players[0].life, 20));

    // unpayable: with no creature it is only cast without it (601.2f-h)
    let mut g = game();
    let card = g.put(0, PRINTED, Zone::Hand);
    cite!(["118.8b", "601.2h"]; refused("which asks for exactly 1"): act_unchanged(&mut g, Move::WithOptional(true, Box::new(Move::Cast { card, targets: vec![] }))));
    assert_eq!(g.obj(card).zone, Zone::Hand);
    // a spell without an optional cost announces none
    let bear = g.put(0, BEAR, Zone::Hand);
    cite!(["118.8b"]; refused("that spell has no optional additional cost (118.8b)"): g.act(0, Move::WithOptional(false, Box::new(Move::Cast { card: bear, targets: vec![] }))));
}

#[test]
fn r707_10_a_copy_of_a_spell_whose_optional_cost_was_paid_reads_it_as_paid() {
    for pay in [true, false] {
        let mut g = game();
        let card = g.put(0, PRINTED, Zone::Hand);
        let bear = g.put(0, BEAR, Zone::Battlefield);
        g.act(0, if pay { paid(card, bear, vec![]) } else { Move::Cast { card, targets: vec![] } }).unwrap();
        let spell = *g.stack.last().unwrap();
        let fork = g.put(0, FORK, Zone::Hand);
        g.act(0, Move::Cast { card: fork, targets: vec![vec![Target::Object(spell)]] }).unwrap();
        resolve(&mut g);
        let gained = if pay { 4 } else { 0 };
        cite!(["707.10", "707.2"]; case: format!("paid={pay}"); assert_eq!((g.players[0].life, hand(&g, 0)), (20 + gained, 2)));
    }
}

#[test]
fn r702_175a_offspring_paid_makes_a_1_1_token_copy() {
    let mut g = game();
    let card = g.put(0, RABBIT, Zone::Hand);
    g.players[0].pool.colorless = 3;
    g.act(0, spend("CCC", Move::WithOptional(true, Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    cite!(["702.175a", "601.2f"]; assert_eq!(g.players[0].pool.colorless, 0, "{{1}} and the offspring cost {{2}}"));
    resolve(&mut g);
    let tokens: Vec<_> = g.battlefield.iter().copied().filter(|o| g.obj(*o).token()).collect();
    cite!(["702.175a"]; assert_eq!(tokens.len(), 1));
    let token = tokens[0];
    cite!(["702.175a", "707.9b"]; assert_eq!((g.power(token), g.toughness(token)), (Some(1), Some(1))));
    cite!(["707.2"]; assert_eq!(g.obj(token).chars.name, "Warren Rabbit"));
    cite!(["707.2"]; assert!(g.has(token, "Flying")));
    // the token was not cast with its offspring cost paid: it makes no token of its own
    cite!(["702.175a"]; assert_eq!(g.battlefield.iter().filter(|o| g.obj(**o).token()).count(), 1));
    // without paying it: no trigger, no token
    let mut g = game();
    let card = g.put(0, RABBIT, Zone::Hand);
    g.players[0].pool.colorless = 3;
    g.act(0, spend("C", Move::Cast { card, targets: vec![] })).unwrap();
    resolve(&mut g);
    cite!(["702.175a"]; case: "declined"; assert_eq!(g.battlefield.iter().filter(|o| g.obj(**o).token()).count(), 0));
    assert_eq!(g.players[0].pool.colorless, 2);
    // The mana cost alone is affordable; the additional offspring cost is not.
    let mut g = game();
    let card = g.put(0, RABBIT, Zone::Hand);
    g.players[0].pool.colorless = 1;
    let before = format!("{g:?}");
    cite!(["702.175a", "118.3", "601.2h"]; refused("that mana is not in your mana pool"): act_unchanged(&mut g, spend("CCC", Move::WithOptional(true, Box::new(Move::Cast { card, targets: vec![] })))));
    cite!(["733.1"]; assert_eq!(format!("{g:?}"), before));
}

#[test]
fn r707_9b_the_offspring_token_is_1_1_before_its_counters() {
    let mut g = game();
    let card = g.put(0, GROWN, Zone::Hand);
    g.players[0].pool.colorless = 2;
    g.act(0, spend("CC", Move::WithOptional(true, Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    resolve(&mut g);
    let token = *g.battlefield.iter().find(|o| g.obj(**o).token()).unwrap();
    // its copiable 1/1 is its base; it enters with the counters its text gives (614.1c)
    cite!(["707.9b", "613.4"]; assert_eq!((g.power(token), g.toughness(token)), (Some(3), Some(3))));
}

#[test]
fn r702_153a_casualty_sacrifices_a_creature_with_enough_power_and_copies_the_spell() {
    let mut g = game();
    let card = g.put(0, BOLT, Zone::Hand);
    let goblin = g.put(0, GOBLIN, Zone::Battlefield);
    // power 1 is not 2 or greater
    cite!(["702.153a", "601.2h"]; refused("Goblin is not what that cost asks for"): act_unchanged(&mut g, paid(card, goblin, vec![vec![Target::Player(1)]])));
    let bear = g.put(0, BEAR, Zone::Battlefield);
    g.act(0, paid(card, bear, vec![vec![Target::Player(1)]])).unwrap();
    cite!(["601.2h"]; assert!(buried(&g, 0, BEAR)));
    // the spell and its cast trigger
    cite!(["702.153a", "603.2"]; assert_eq!(g.stack.len(), 2));
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    // the copy may have new targets (707.10c)
    let copy = match &g.decision {
        Some(Decision::Choose { player: 0, choice: Choice::Retarget { copy, .. } }) => *copy,
        other => panic!("expected the copy's targets: {other:?}"),
    };
    g.act(0, Move::Choose(Answer::Targets(vec![vec![Target::Object(goblin)]]))).unwrap();
    cite!(["707.10"]; assert!(g.stack.contains(&copy)));
    resolve(&mut g);
    cite!(["707.10c"]; assert!(buried(&g, 0, GOBLIN)));
    cite!(["702.153a"]; assert_eq!(g.players[1].life, 19));
    // declined: no copy
    let mut g = game();
    let card = g.put(0, BOLT, Zone::Hand);
    g.put(0, BEAR, Zone::Battlefield);
    g.act(0, Move::Cast { card, targets: vec![vec![Target::Player(1)]] }).unwrap();
    cite!(["702.153a"]; case: "declined"; assert_eq!(g.stack.len(), 1));
    resolve(&mut g);
    assert_eq!(g.players[1].life, 19);
}

#[test]
fn r707_10f_casualty_copy_of_a_permanent_spell_becomes_a_token() {
    let mut g = game();
    let card = g.put(0, HULK, Zone::Hand);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    g.act(0, paid(card, bear, vec![])).unwrap();
    resolve(&mut g);
    let hulks: Vec<_> = g.battlefield.iter().copied().filter(|o| g.obj(*o).chars.name == "Grave Hulk").collect();
    cite!(["702.153a", "707.10f", "608.3f"]; assert_eq!(hulks.len(), 2));
    cite!(["707.10f", "608.3f"]; assert_eq!(hulks.iter().filter(|o| g.obj(**o).token()).count(), 1));
    // the token was not created (608.3f) and is a 4/4: it is not offspring's 1/1
    let token = *hulks.iter().find(|o| g.obj(**o).token()).unwrap();
    assert_eq!((g.power(token), g.toughness(token)), (Some(4), Some(4)));
}

#[test]
fn r707_10_casualty_and_kicker_are_announced_and_copied_together() {
    for kick in [0, 1] {
        let mut g = game();
        let card = g.put(0, KICKED, Zone::Hand);
        let bear = g.put(0, BEAR, Zone::Battlefield);
        g.players[0].pool.colorless = 1;
        g.act(0, spend(&"C".repeat(kick as usize), Move::WithKicks(kick, Box::new(paid(card, bear, vec![]))))).unwrap();
        resolve(&mut g);
        // the copy is kicked as the spell was (707.2): each draws when kicked
        let case = format!("kicks={kick}");
        cite!(["702.153a", "707.10"]; case: case; assert_eq!(g.players[0].life, 22));
        cite!(["702.33d", "707.2"]; case: case; assert_eq!(hand(&g, 0), 2 * kick as usize));
    }
}

/// Intrepid Rabbit: the token copy has its enters trigger too, and is a 1/1 (702.175a).
#[test]
fn r702_175a_intrepid_rabbit_offspring_token_triggers_on_entering() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let rabbit = CardDef::from_faces(catalog.get("Intrepid Rabbit").unwrap()).unwrap();
    game::play::check(&rabbit).unwrap();
    let mut g = Game::new(vec![rabbit, CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap()], 2, 20);
    for p in 0..2 { for _ in 0..4 { g.put(p, 1, Zone::Library); } }
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors = [1, 0, 0, 0, 0];
    g.players[0].pool.colorless = 3;
    g.act(0, spend("CCCW", Move::WithOptional(true, Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    cite!(["601.2f"]; assert_eq!(g.players[0].pool.colorless, 0));
    // the spell resolves: its own enters trigger and the offspring trigger
    for _ in 0..40 {
        if g.stack.is_empty() && g.decision.is_none() { break; }
        match &g.decision {
            Some(Decision::Targets { player, .. }) => {
                let p = *player;
                let first = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(0) && !g.obj(**o).token()).unwrap();
                g.act(p, Move::ChooseTargets(vec![vec![Target::Object(first)]])).unwrap(); }
            Some(Decision::Order { player, .. }) => { let p = *player; g.act(p, Move::Order(vec![0, 1])).unwrap(); }
            None => g.act(g.priority, Move::Pass).unwrap(),
            other => panic!("unexpected {other:?}"),
        }
    }
    let token = *g.battlefield.iter().find(|o| g.obj(**o).token()).expect("the offspring token");
    cite!(["702.175a", "707.9b"]; assert_eq!((g.power(token), g.toughness(token)), (Some(1), Some(1))));
    // the original got +1/+1 twice: from its own trigger and from the token's (603.6a)
    let first = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(0) && !g.obj(**o).token()).unwrap();
    cite!(["603.6a", "707.2"]; assert_eq!(g.power(first), Some(5)));
}

fn teamwork(card: ObjId, tapped: Vec<ObjId>, modes: Vec<usize>) -> Move {
    Move::WithOptional(true, Box::new(Move::WithCosts(vec![tapped], Box::new(Move::Modes(modes, Box::new(Move::Cast { card, targets: vec![] }))))))
}

/// Teamwork N: tap any number of creatures you control with total power N or more (702.194a),
/// summoning sick or not (302.6 limits only a creature's own {T} abilities); "If this spell
/// was cast using teamwork, choose both instead" as it was announced (702.194b, 601.2b).
#[test]
fn r702_194a_teamwork_taps_creatures_with_total_power_n_and_unlocks_both_modes() {
    let setup = || {
        let mut g = game();
        let bear = g.put(0, BEAR, Zone::Battlefield);
        let goblin = g.put(0, GOBLIN, Zone::Hand);
        g.act(0, Move::Cast { card: goblin, targets: vec![] }).unwrap();
        resolve(&mut g);
        let goblin = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(GOBLIN)).unwrap();
        assert!(g.summoning_sick(goblin));
        let card = g.put(0, TEAMWORK, Zone::Hand);
        (g, card, bear, goblin)
    };
    // 2 + 1 power, one of them summoning sick: both modes
    let (mut g, card, bear, goblin) = setup();
    g.act(0, teamwork(card, vec![bear, goblin], vec![0, 1])).unwrap();
    cite!(["702.194a", "302.6"]; assert!(g.obj(bear).tapped && g.obj(goblin).tapped));
    resolve(&mut g);
    cite!(["702.194b", "700.2a"]; assert_eq!((g.players[0].life, hand(&g, 0)), (22, 1)));
    // power 2 alone is not 3
    let (mut g, card, bear, _) = setup();
    cite!(["702.194a"]; refused("chosen creatures have total power less than 3"): act_unchanged(&mut g, teamwork(card, vec![bear], vec![0, 1])));
    assert!(!g.obj(bear).tapped);
    // with teamwork, both: one mode is refused (700.2a)
    let (mut g, card, bear, goblin) = setup();
    cite!(["702.194b", "700.2a"]; refused("choose 2 to 2 of its modes"): act_unchanged(&mut g, teamwork(card, vec![bear, goblin], vec![0])));
    // without it, one: both are refused
    let (mut g, card, _, _) = setup();
    cite!(["700.2a"]; refused("choose 1 to 1 of its modes"): act_unchanged(&mut g, Move::Modes(vec![0, 1], Box::new(Move::Cast { card, targets: vec![] }))));
    g.act(0, Move::Modes(vec![1], Box::new(Move::Cast { card, targets: vec![] }))).unwrap();
    resolve(&mut g);
    cite!(["700.2a"]; assert_eq!((g.players[0].life, hand(&g, 0)), (20, 1)));
}

/// "If this spell was kicked, choose both instead" (702.33d, 601.2b): the modes the listing
/// offers follow the announcement, and a copy has the modes of the spell it copies (707.10).
#[test]
fn r702_33d_kicked_choose_both_instead_and_its_copy() {
    let mut g = game();
    let card = g.put(0, INSCRIPTION, Zone::Hand);
    g.players[0].pool.colorless = 1;
    for (kicks, modes, accepted) in [(0, vec![0], true), (0, vec![1], true), (1, vec![0, 1], true), (1, vec![0], false), (0, vec![0, 1], false)] {
        let mut attempt = g.clone();
        let result = attempt.act(0, spend(&"C".repeat(kicks as usize), Move::WithKicks(kicks, Box::new(Move::Modes(modes.clone(), Box::new(Move::Cast { card, targets: vec![] }))))));
        cite!(["601.2b", "702.33d"]; case: format!("{kicks}/{modes:?}"); assert_eq!(result.is_ok(), accepted));
    }
    g.act(0, spend("C", Move::WithKicks(1, Box::new(Move::Modes(vec![0, 1], Box::new(Move::Cast { card, targets: vec![] })))))).unwrap();
    let spell = *g.stack.last().unwrap();
    let fork = g.put(0, FORK, Zone::Hand);
    g.act(0, Move::Cast { card: fork, targets: vec![vec![Target::Object(spell)]] }).unwrap();
    resolve(&mut g);
    cite!(["707.10", "702.33d"]; assert_eq!((g.players[0].life, hand(&g, 0)), (24, 2)));
}

/// Real cards with these names, then a vanilla 2/2 Bear, a {4} and a {2}{U} card to collect as
/// evidence, a Dragon, Islands for libraries, a {4} 4/4 Giant and a {1} artifact.
fn real(names: &[&str]) -> (Game, [usize; 7]) {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let mut defs: Vec<_> = names.iter().map(|n| {
        let d = CardDef::from_faces(catalog.get(*n).unwrap()).unwrap();
        game::play::check(&d).unwrap_or_else(|e| panic!("{n}: {}", e.0));
        d
    }).collect();
    let n = defs.len();
    let c = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    defs.push(c("Bear", "{0}", "Creature — Bear", "", Some((2, 2))));
    defs.push(c("Four", "{4}", "Sorcery", "Draw a card.", None));
    defs.push(c("Three", "{2}{U}", "Sorcery", "Draw a card.", None));
    defs.push(c("Drake", "{0}", "Creature — Dragon", "", Some((1, 1))));
    defs.push(c("Island", "", "Basic Land — Island", "", None));
    defs.push(c("Giant", "{4}", "Creature — Giant", "", Some((4, 4))));
    defs.push(c("Trinket", "{1}", "Artifact", "", None));
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..6 { g.put(p, n + 4, Zone::Library); } }
    (g, [n, n + 1, n + 2, n + 3, n + 4, n + 5, n + 6])
}
fn optional(paid: bool, paying: Vec<Vec<ObjId>>, card: ObjId, targets: Vec<Vec<Target>>, spending: &str) -> Move {
    let cast = Move::Cast { card, targets };
    spend(spending, Move::WithOptional(paid, Box::new(if paying.is_empty() { cast } else { Move::WithCosts(paying, Box::new(cast)) })))
}

/// Requiting Hex: "you may blight 1": a -1/-1 counter on a creature you control (701.68a), not
/// one an opponent controls; "if this spell's additional cost was paid" reads it (118.8b).
#[test]
fn r701_68a_blight_as_an_optional_additional_cost() {
    let setup = || {
        let (mut g, [bear, ..]) = real(&["Requiting Hex"]);
        let card = g.put(0, 0, Zone::Hand);
        let mine = g.put(0, bear, Zone::Battlefield);
        let theirs = g.put(1, bear, Zone::Battlefield);
        g.players[0].pool.colors = [0, 0, 1, 0, 0];
        (g, card, mine, theirs)
    };
    let (mut g, card, mine, theirs) = setup();
    cite!(["701.68b"]; refused("Bear is not what that cost asks for"): act_unchanged(&mut g, optional(true, vec![vec![theirs]], card, vec![vec![Target::Object(theirs)]], "B")));
    g.act(0, optional(true, vec![vec![mine]], card, vec![vec![Target::Object(theirs)]], "B")).unwrap();
    cite!(["701.68a", "601.2h"]; assert_eq!((g.counters(Target::Object(mine)).get("-1/-1").copied(), g.power(mine)), (Some(1), Some(1))));
    resolve(&mut g);
    cite!(["118.8b"]; assert_eq!((g.players[0].life, buried(&g, 1, 1)), (22, true)));
    // declined: no counter, no life
    let (mut g, card, mine, theirs) = setup();
    g.act(0, optional(false, vec![], card, vec![vec![Target::Object(theirs)]], "B")).unwrap();
    resolve(&mut g);
    cite!(["118.8b"]; assert_eq!((g.players[0].life, g.counters(Target::Object(mine)).get("-1/-1")), (20, None)));
    // no creature of yours: it can't be paid, only declined (701.68b)
    let (mut g, card, mine, theirs) = setup();
    g.move_object(mine, Zone::Graveyard);
    cite!(["701.68b"]; refused("Bear is not what that cost asks for"): act_unchanged(&mut g, optional(true, vec![vec![theirs]], card, vec![vec![Target::Object(theirs)]], "B")));
}

/// Crimestopper Sprite: collect evidence 6 exiles cards from your graveyard with total mana
/// value 6 or more (701.59a); "if evidence was collected" reads it (701.59c).
#[test]
fn r701_59a_collect_evidence_as_an_optional_additional_cost() {
    let setup = |graveyard: &[usize]| {
        let (mut g, [bear, four, three, ..]) = real(&["Crimestopper Sprite"]);
        let card = g.put(0, 0, Zone::Hand);
        let theirs = g.put(1, bear, Zone::Battlefield);
        let cards: Vec<_> = graveyard.iter().map(|d| g.put(0, [four, three][*d], Zone::Graveyard)).collect();
        g.players[0].pool.colors = [0, 1, 0, 0, 0];
        g.players[0].pool.colorless = 2;
        (g, card, theirs, cards)
    };
    let enter = |g: &mut Game, theirs: ObjId| {
        for _ in 0..20 {
            match &g.decision {
                Some(Decision::Targets { player, .. }) => { let p = *player; g.act(p, Move::ChooseTargets(vec![vec![Target::Object(theirs)]])).unwrap(); }
                _ if g.stack.is_empty() && g.decision.is_none() => return,
                _ => g.act(g.priority, Move::Pass).unwrap(),
            }
        }
    };
    // 4 + 3: enough; both exiled, and the creature is tapped and stunned
    let (mut g, card, theirs, cards) = setup(&[0, 1]);
    g.act(0, optional(true, vec![cards.clone()], card, vec![], "CCU")).unwrap();
    cite!(["701.59a", "601.2h"]; assert!(g.players[0].graveyard.is_empty() && g.exile.len() == 2));
    enter(&mut g, theirs);
    cite!(["701.59c", "607.2"]; assert_eq!((g.obj(theirs).tapped, g.counters(Target::Object(theirs)).get("stun").copied()), (true, Some(1))));
    // 4 alone is not 6
    let (mut g, card, _, cards) = setup(&[0, 1]);
    cite!(["701.59a"]; refused("chosen cards have total mana value less than 6"): act_unchanged(&mut g, optional(true, vec![vec![cards[0]]], card, vec![], "CCU")));
    // Even the entire graveyard cannot pay collect evidence 6 when its total is 4.
    let (mut g, card, _, cards) = setup(&[0]);
    let before = format!("{g:?}");
    cite!(["701.59a", "701.59b", "601.2h"]; refused("chosen cards have total mana value less than 6"): act_unchanged(&mut g, optional(true, vec![cards], card, vec![], "CCU")));
    cite!(["733.1"]; assert_eq!(format!("{g:?}"), before));
    // declined: tapped, no stun counter
    let (mut g, card, theirs, _) = setup(&[0, 1]);
    g.act(0, optional(false, vec![], card, vec![], "CCU")).unwrap();
    enter(&mut g, theirs);
    cite!(["701.59c"]; assert_eq!((g.obj(theirs).tapped, g.counters(Target::Object(theirs)).get("stun")), (true, None)));
}

/// Piercing Exhale: behold a Dragon, a Dragon card revealed from your hand or a Dragon you
/// control chosen; nothing happens to it (701.4a), and "if a Dragon was beheld" reads it.
#[test]
fn r701_4a_behold_as_an_optional_additional_cost() {
    let setup = || {
        let (mut g, [bear, _, _, drake, ..]) = real(&["Piercing Exhale"]);
        let card = g.put(0, 0, Zone::Hand);
        let mine = g.put(0, bear, Zone::Battlefield);
        let theirs = g.put(1, bear, Zone::Battlefield);
        g.players[0].pool.colors = [0, 0, 0, 0, 1];
        g.players[0].pool.colorless = 1;
        (g, card, mine, theirs, drake)
    };
    let targets = |mine, theirs| vec![vec![Target::Object(mine)], vec![Target::Object(theirs)]];
    // from the hand: the card stays there
    let (mut g, card, mine, theirs, drake) = setup();
    let dragon = g.put(0, drake, Zone::Hand);
    cite!(["701.4a"]; refused("Bear is not what that cost asks for"): act_unchanged(&mut g, optional(true, vec![vec![mine]], card, targets(mine, theirs), "CG")));
    g.act(0, optional(true, vec![vec![dragon]], card, targets(mine, theirs), "CG")).unwrap();
    cite!(["701.4a"]; assert!(g.players[0].hand.contains(&dragon)));
    for _ in 0..4 { if g.decision.is_some() { break; } g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["701.4b", "701.25a"]; assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::Arrange { verb: ir::Verb::Surveil, .. } }))));
    cite!(["120.3"]; assert_eq!(g.obj(theirs).damage, 2));
    // a Dragon you control: chosen, not tapped or moved
    let (mut g, card, mine, theirs, drake) = setup();
    let dragon = g.put(0, drake, Zone::Battlefield);
    g.act(0, optional(true, vec![vec![dragon]], card, targets(mine, theirs), "CG")).unwrap();
    cite!(["701.4a"]; assert!(g.battlefield.contains(&dragon) && !g.obj(dragon).tapped));
    // an opponent's Dragon is not yours to behold
    let (mut g, card, mine, theirs, drake) = setup();
    let dragon = g.put(1, drake, Zone::Battlefield);
    cite!(["701.4a"]; refused("Drake is not what that cost asks for"): act_unchanged(&mut g, optional(true, vec![vec![dragon]], card, targets(mine, theirs), "CG")));
}

/// Collect evidence and blight in activation costs: Forensic Researcher ({T}, collect evidence
/// 3) and Sting-Slinger ({1}{R}, {T}, blight 1, which may be itself).
#[test]
fn r701_59a_68a_collect_evidence_and_blight_in_activation_costs() {
    let (mut g, [_, four, ..]) = real(&["Forensic Researcher", "Sting-Slinger"]);
    let researcher = g.put(0, 0, Zone::Battlefield);
    let slinger = g.put(0, 1, Zone::Battlefield);
    let theirs = g.put(1, 1, Zone::Battlefield);
    let evidence = g.put(0, four, Zone::Graveyard);
    let tapping = 1; // its second ability
    g.act(0, Move::Activate { source: researcher, ability: tapping, targets: vec![vec![Target::Object(theirs)]], paying: vec![vec![evidence]] }).unwrap();
    cite!(["701.59a", "602.2b"]; assert!(g.exile.iter().any(|o| g.obj(*o).card == Some(four))));
    resolve(&mut g);
    cite!(["701.59a"]; assert!(g.obj(theirs).tapped));
    g.players[0].pool.colors = [0, 0, 0, 1, 0];
    g.players[0].pool.colorless = 1;
    let power = g.power(slinger).unwrap();
    g.act(0, spend("CR", Move::Activate { source: slinger, ability: 0, targets: vec![], paying: vec![vec![slinger]] })).unwrap();
    cite!(["701.68a"]; assert_eq!(g.power(slinger), Some(power - 1)));
    resolve(&mut g);
    cite!(["701.68a"]; assert_eq!(g.players[1].life, 18));
}

/// Cruel Alliance: "If this spell was cast using teamwork, instead exile target creature":
/// which target it has is announced with the cost (601.2b-c): a mana value 4 creature only
/// with teamwork.
#[test]
fn r601_2c_cruel_alliance_targets_as_its_teamwork_announcement_says() {
    let setup = || {
        let (mut g, [bear, .., giant, _]) = real(&["Cruel Alliance"]);
        let card = g.put(0, 0, Zone::Hand);
        let mine = g.put(0, bear, Zone::Battlefield);
        let theirs = g.put(1, giant, Zone::Battlefield);
        g.players[0].pool.colors = [0, 0, 1, 0, 0];
        g.players[0].pool.colorless = 2;
        (g, card, mine, theirs)
    };
    let (mut g, card, _, theirs) = setup();
    // each target slot in order, the one not announced empty
    cite!(["601.2c"]; refused("an illegal target, Giant"): act_unchanged(&mut g, optional(false, vec![], card, vec![vec![Target::Object(theirs)], vec![]], "CCB")));
    let (mut g, card, mine, theirs) = setup();
    g.act(0, optional(true, vec![vec![mine]], card, vec![vec![], vec![Target::Object(theirs)]], "CCB")).unwrap();
    resolve(&mut g);
    cite!(["702.194b"]; assert_eq!((g.exile.len(), g.players[0].life), (1, 23)));
}

/// Pyrrhic Strike: "you may blight 2"; "If this spell's additional cost was paid, choose both
/// instead" (601.2b, 700.2a). Blighting the Bear kills it (704.5f).
#[test]
fn r700_2a_pyrrhic_strike_both_modes_when_blighted() {
    let setup = || {
        let (mut g, [bear, .., giant, trinket]) = real(&["Pyrrhic Strike"]);
        let card = g.put(0, 0, Zone::Hand);
        let mine = g.put(0, bear, Zone::Battlefield);
        let giant = g.put(1, giant, Zone::Battlefield);
        let trinket = g.put(1, trinket, Zone::Battlefield);
        g.players[0].pool.colors = [1, 0, 0, 0, 0];
        g.players[0].pool.colorless = 2;
        (g, card, mine, giant, trinket)
    };
    let both = |paid, paying, card, trinket, giant| Move::WithOptional(paid, Box::new(Move::WithCosts(paying,
        Box::new(Move::Modes(vec![0, 1], Box::new(Move::Cast { card, targets: vec![vec![Target::Object(trinket)], vec![Target::Object(giant)]] }))))));
    let (mut g, card, _, giant, trinket) = setup();
    cite!(["700.2a"]; refused("choose 1 to 1 of its modes"): act_unchanged(&mut g, spend("CCW", both(false, vec![], card, trinket, giant))));
    let (mut g, card, mine, giant, trinket) = setup();
    g.act(0, spend("CCW", both(true, vec![vec![mine]], card, trinket, giant))).unwrap();
    resolve(&mut g);
    cite!(["701.68a", "704.5f"]; assert!(buried(&g, 0, 1)));
    cite!(["700.2a", "118.8b"]; assert!(buried(&g, 1, 6) && buried(&g, 1, 7)));
}

/// A rejected proposal preserves the whole game; the caller asserts its specific reason.
#[track_caller]
fn act_unchanged(g: &mut Game, action: Move) -> Result<(), String> {
    let before = format!("{g:?}");
    let result = g.act(0, action);
    assert_eq!(format!("{g:?}"), before);
    result
}
