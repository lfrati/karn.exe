//! Ward wherever its permanent has it (702.21a): printed, or granted by an Equipment, an Aura, a
//! static ability or a resolved effect (613.1f); and ward costs that are actions, paid by the
//! player who controls the targeting spell or ability: discard a card, sacrifice a permanent,
//! pay life equal to a power. Lavaspur Boots, Sheltered by Ghosts, Sunset Saboteur, Vein
//! Ripper, Raubahn and Coppercoat Vanguard are their Oracle texts; the others are test cards.
use crate::support::cite;
use game::card::CardDef;
use game::legal::Asked;
use game::play::check;
use game::state::Answer;
use game::{Game, Move, ObjId, Target, Zone};
use ir::mana::ManaSymbol;

const BEAR: usize = 0;
const SHOCK: usize = 1;
const PINGER: usize = 2;
const GROW: usize = 3;
const BOOTS: usize = 4;
const SHELTERED: usize = 5;
const SABOTEUR: usize = 6;
const RIPPER: usize = 7;
const RAUBAHN: usize = 8;
const VANGUARD: usize = 9;
const HUMAN: usize = 10;
const SHIELD: usize = 11;
const MOUNTAIN: usize = 12;
const CLERIC: usize = 13;

fn defs() -> Vec<CardDef> {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let real = |name: &str| CardDef::from_faces(catalog.get(name).unwrap()).unwrap();
    let card = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).expect(name);
    vec![
        card("Bear", "Creature — Bear", "", Some((2, 2))),
        card("Shock", "Instant", "Shock deals 2 damage to any target.", None),
        card("Pinger", "Artifact", "{T}: This artifact deals 1 damage to any target.", None),
        card("Grow", "Instant", "Target creature gets +1/+1 until end of turn.", None),
        real("Lavaspur Boots"),
        real("Sheltered by Ghosts"),
        real("Sunset Saboteur"),
        real("Vein Ripper"),
        real("Raubahn, Bull of Ala Mhigo"),
        real("Coppercoat Vanguard"),
        card("Human", "Creature — Human Soldier", "", Some((1, 1))),
        card("Shield", "Instant", "Target creature you control gains ward {2} until end of turn.", None),
        CardDef::new("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None).unwrap(),
        // Alpharael, Stonechosen's ward; the card itself is refused for its void ability
        card("Warded Cleric", "Creature — Human Cleric", "Ward—Discard a card at random.", Some((3, 3))),
    ]
}

fn game() -> Game {
    let mut g = Game::new(defs(), 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, MOUNTAIN, Zone::Library); } }
    g
}

/// Player `by` has priority (the other player has passed it on) and casts Shock at `target`.
fn shock(g: &mut Game, by: usize, target: Target) -> ObjId {
    while g.priority != by { g.act(g.priority, Move::Pass).unwrap(); }
    let shock = g.put(by, SHOCK, Zone::Hand);
    g.act(by, Move::Cast { card: shock, targets: vec![vec![target]] }).unwrap();
    shock_on_stack(g).unwrap()
}

fn shock_on_stack(g: &Game) -> Option<ObjId> {
    g.stack.iter().copied().find(|o| g.obj(*o).chars.name == "Shock")
}

/// Pass priority until something is asked other than priority, or the stack is empty.
fn until_asked(g: &mut Game) {
    while g.decision.is_none() && !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
}

fn triggers(g: &Game) -> usize {
    g.stack.iter().filter(|o| g.obj(**o).ability.is_some()).count()
}

fn colorless(g: &mut Game, player: usize, n: usize) {
    for _ in 0..n { g.add_mana(player, ManaSymbol::Colorless); }
}

/// A Bear of player 0's wearing the Boots.
fn booted(g: &mut Game) -> ObjId {
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let boots = g.put(0, BOOTS, Zone::Battlefield);
    assert!(g.attach(boots, Target::Object(bear)));
    bear
}

#[test]
fn r702_21a_the_cards_are_admitted() {
    for d in defs() { check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0)); }
}

#[test]
fn r702_21a_ward_granted_by_equipment_counters_an_opponents_spell_unless_paid() {
    for pays in [true, false] {
        let mut g = game();
        let bear = booted(&mut g);
        colorless(&mut g, 1, 2);
        let spell = shock(&mut g, 1, Target::Object(bear));
        cite!(["702.21a", "613.1f"]; case: pays; assert_eq!(triggers(&g), 1, "the equipped creature's ward triggers"));
        until_asked(&mut g);
        cite!(["702.21a"]; case: pays; assert!(matches!(g.asked(), Some((1, Asked::Pay { .. }))), "{:?}", g.asked()));
        g.act(1, Move::Choose(if pays { Answer::Pay { plan: vec![], with: Some(crate::support::mana_pool("C")), tapped: vec![] } } else { Answer::No })).unwrap();
        if pays {
            cite!(["702.21a"]; assert_eq!((g.stack.clone(), g.players[1].pool.total()), (vec![spell], 1), "{{1}} paid, Shock still on the stack"));
            until_asked(&mut g);
            assert!(!g.battlefield.contains(&bear), "Shock resolved: 2 damage to a 3/2");
        } else {
            cite!(["702.21a", "701.6a"]; assert!(g.stack.is_empty() && g.battlefield.contains(&bear)));
            assert_eq!(g.players[1].pool.total(), 2, "nothing paid");
        }
    }
}

#[test]
fn r702_21a_granted_ward_ignores_its_controllers_spells_and_counts_an_opponents_abilities() {
    let mut g = game();
    let bear = booted(&mut g);
    let grow = g.put(0, GROW, Zone::Hand);
    g.act(0, Move::Cast { card: grow, targets: vec![vec![Target::Object(bear)]] }).unwrap();
    cite!(["702.21a"]; assert_eq!(triggers(&g), 0, "its controller's own spell"));
    until_asked(&mut g);
    // An opponent's activated ability is countered when its controller declines ward.
    let pinger = g.put(1, PINGER, Zone::Battlefield);
    while g.priority != 1 { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(1, Move::Activate { source: pinger, ability: 0, targets: vec![vec![Target::Object(bear)]], paying: vec![] }).unwrap();
    cite!(["702.21a"]; assert_eq!(triggers(&g), 2, "the ability and the ward trigger"));
    until_asked(&mut g);
    g.act(1, Move::Choose(Answer::No)).unwrap();
    cite!(["702.21a", "118.3"]; assert!(g.stack.is_empty() && g.obj(bear).damage == 0));
}

#[test]
fn r702_21a_ward_triggered_from_a_grant_resolves_after_the_grant_ends() {
    // the Boots leave after the ward trigger is put on the stack: it still counters unless {1}
    for leaves in ["boots", "bear"] {
        let mut g = game();
        let bear = booted(&mut g);
        let boots = *g.battlefield.iter().find(|o| g.obj(**o).chars.name == "Lavaspur Boots").unwrap();
        colorless(&mut g, 1, 2);
        shock(&mut g, 1, Target::Object(bear));
        assert_eq!(triggers(&g), 1);
        g.destroy(if leaves == "boots" { boots } else { bear });
        until_asked(&mut g);
        cite!(["113.7a", "702.21a"]; case: leaves; assert!(matches!(g.asked(), Some((1, Asked::Pay { .. }))), "{:?}", g.asked()));
        g.act(1, Move::Choose(Answer::Pay { plan: vec![], with: Some(crate::support::mana_pool("C")), tapped: vec![] })).unwrap();
        cite!(["702.21a"]; case: leaves; assert_eq!(g.players[1].pool.total(), 1, "the cost as stated, {{1}}"));
    }
}

#[test]
fn r702_21a_each_ward_instance_triggers_separately() {
    // Boots (ward {1}) and Sheltered by Ghosts (ward {2}) on one creature: two ward abilities
    let mut g = game();
    let bear = booted(&mut g);
    let aura = g.put(0, SHELTERED, Zone::Battlefield);
    assert!(g.attach(aura, Target::Object(bear)));
    colorless(&mut g, 1, 3);
    let spell = shock(&mut g, 1, Target::Object(bear));
    // two triggers of one controller: they are put on the stack in the order it chooses
    cite!(["702.21a", "603.2", "603.3b"]; assert!(matches!(g.asked(), Some((0, Asked::Order(a))) if a.len() == 2), "{:?}", g.asked()));
    g.act(0, Move::Order(vec![0, 1])).unwrap();
    cite!(["702.21a", "603.2"]; assert_eq!(triggers(&g), 2, "one trigger for each ward"));
    // pays both: {1} and {2}
    for units in ["CC", "C"] {
        until_asked(&mut g);
        assert!(matches!(g.asked(), Some((1, Asked::Pay { .. }))), "{:?}", g.asked());
        g.act(1, Move::Choose(Answer::Pay { plan: vec![], with: Some(crate::support::mana_pool(units)), tapped: vec![] })).unwrap();
    }
    cite!(["702.21a"]; assert_eq!((g.stack.clone(), g.players[1].pool.total()), (vec![spell], 0), "both paid, Shock still there"));
    // a second grant of the same ward by another Boots is a third instance
    let mut g = game();
    let bear = booted(&mut g);
    let more = g.put(0, BOOTS, Zone::Battlefield);
    assert!(g.attach(more, Target::Object(bear)));
    shock(&mut g, 1, Target::Object(bear));
    cite!(["702.21a", "603.2"]; assert!(matches!(g.asked(), Some((0, Asked::Order(a))) if a.len() == 2), "two Boots, two ward {{1}} abilities: {:?}", g.asked()));
}

#[test]
fn r702_21a_ward_granted_by_a_static_ability_and_by_a_resolved_effect() {
    // Coppercoat Vanguard: "Each other Human you control … has ward {1}."
    let mut g = game();
    let _vanguard = g.put(0, VANGUARD, Zone::Battlefield);
    let human = g.put(0, HUMAN, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    shock(&mut g, 1, Target::Object(bear));
    cite!(["702.21a"]; assert_eq!(triggers(&g), 0, "not a Human"));
    until_asked(&mut g);
    shock(&mut g, 1, Target::Object(human));
    cite!(["702.21a", "613.1f"]; assert_eq!(triggers(&g), 1, "the other Human has ward"));
    // "Target creature you control gains ward {2} until end of turn."
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let shield = g.put(0, SHIELD, Zone::Hand);
    g.act(0, Move::Cast { card: shield, targets: vec![vec![Target::Object(bear)]] }).unwrap();
    until_asked(&mut g);
    colorless(&mut g, 1, 1);
    shock(&mut g, 1, Target::Object(bear));
    cite!(["702.21a", "613.1f"]; assert_eq!(triggers(&g), 1, "the gained ward"));
    until_asked(&mut g);
    cite!(["118.3"]; refused("the supplied mana does not pay that cost"): act_unchanged(&mut g,1,Move::Choose(Answer::Pay{plan:vec![],with:Some(crate::support::mana_pool("C")),tapped:vec![]})));
    g.act(1,Move::Choose(Answer::No)).unwrap();
    cite!(["702.21a", "118.3"]; assert!(g.stack.is_empty() && g.battlefield.contains(&bear), "{{1}} can't pay {{2}}: countered"));
}

#[test]
fn r702_21a_ward_discard_a_card() {
    // Sunset Saboteur, "Ward—Discard a card.": the targeting player may discard; declining or
    // having no card counters the spell
    for (hand, pays) in [(2, true), (2, false), (0, false)] {
        let mut g = game();
        let saboteur = g.put(0, SABOTEUR, Zone::Battlefield);
        let cards: Vec<ObjId> = (0..hand).map(|_| g.put(1, BEAR, Zone::Hand)).collect();
        let spell = shock(&mut g, 1, Target::Object(saboteur));
        until_asked(&mut g);
        let case = format!("hand {hand}, pays {pays}");
        if hand == 0 {
            cite!(["702.21a", "118.3"]; case: case; assert!(g.stack.is_empty() && g.battlefield.contains(&saboteur), "{:?}", g.asked()));
            continue;
        }
        cite!(["702.21a"]; case: case.clone(); assert!(matches!(g.asked(), Some((1, Asked::May))), "{:?}", g.asked()));
        if !pays {
            g.act(1, Move::Choose(Answer::No)).unwrap();
            cite!(["702.21a", "701.6a"]; case: case; assert!(g.stack.is_empty() && g.players[1].hand == cards));
            continue;
        }
        g.act(1, Move::Choose(Answer::Yes)).unwrap();
        cite!(["702.21a"]; assert!(matches!(g.asked(), Some((1, Asked::Objects { count: 1, .. }))), "{:?}", g.asked()));
        g.act(1, Move::Choose(Answer::Objects(vec![cards[1]]))).unwrap();
        cite!(["702.21a", "701.9a"]; assert_eq!((g.players[1].hand.clone(), g.stack.clone()), (vec![cards[0]], vec![spell])));
    }
}

#[test]
fn r702_21a_ward_discard_a_card_at_random() {
    // "Ward—Discard a card at random." (Alpharael, Stonechosen): the targeting player chooses
    // whether to pay; if they do, a card picked at random is discarded, and no one picks it
    // (701.9b). Declining or having no card counters the spell.
    for (hand, pays) in [(2, true), (2, false), (0, false)] {
        let mut g = game();
        let cleric = g.put(0, CLERIC, Zone::Battlefield);
        let cards: Vec<ObjId> = (0..hand).map(|_| g.put(1, BEAR, Zone::Hand)).collect();
        let spell = shock(&mut g, 1, Target::Object(cleric));
        until_asked(&mut g);
        let case = format!("hand {hand}, pays {pays}");
        if hand == 0 {
            cite!(["702.21a", "118.3"]; case: case; assert!(g.stack.is_empty() && g.battlefield.contains(&cleric), "{:?}", g.asked()));
            continue;
        }
        cite!(["702.21a", "118.12"]; case: case.clone(); assert!(matches!(g.asked(), Some((1, Asked::May))), "{:?}", g.asked()));
        if !pays {
            g.act(1, Move::Choose(Answer::No)).unwrap();
            cite!(["702.21a", "701.6a"]; case: case; assert!(g.stack.is_empty() && g.players[1].hand == cards));
            continue;
        }
        g.act(1, Move::Choose(Answer::Yes)).unwrap();
        cite!(["701.9b"]; assert!(!matches!(g.asked(), Some((_, Asked::Objects { .. } | Asked::OptionalObjects { .. } | Asked::Select { .. }))), "no one picks the card: {:?}", g.asked()));
        let discarded: Vec<ObjId> = cards.iter().copied().filter(|c| g.obj(*c).zone != Zone::Hand).collect();
        cite!(["702.21a", "701.9b"]; assert_eq!((discarded.len(), g.players[1].hand.len(), g.players[1].graveyard.len(), g.stack.clone()), (1, 1, 1, vec![spell])));
        until_asked(&mut g);
        assert_eq!(g.obj(cleric).damage, 2, "Shock resolved");
    }
}

#[test]
fn r702_21a_ward_sacrifice_a_creature() {
    // Vein Ripper, "Ward—Sacrifice a creature."
    for has in [true, false] {
        let mut g = game();
        let ripper = g.put(0, RIPPER, Zone::Battlefield);
        let fodder = has.then(|| g.put(1, BEAR, Zone::Battlefield));
        let spell = shock(&mut g, 1, Target::Object(ripper));
        until_asked(&mut g);
        match fodder {
            Some(fodder) => {
                cite!(["702.21a"]; assert!(matches!(g.asked(), Some((1, Asked::May))), "{:?}", g.asked()));
                g.act(1, Move::Choose(Answer::Yes)).unwrap();
                cite!(["702.21a", "701.21a"]; assert!(!g.battlefield.contains(&fodder) && g.stack.contains(&spell)));
            }
            None => cite!(["702.21a", "118.3"]; assert!(shock_on_stack(&g).is_none() && g.battlefield.contains(&ripper), "{:?}", g.asked())),
        }
    }
}

#[test]
fn r702_21a_ward_pay_life_equal_to_its_power() {
    // Raubahn, "Ward—Pay life equal to Raubahn's power.": its power as the trigger resolves
    let mut g = game();
    let raubahn = g.put(0, RAUBAHN, Zone::Battlefield);
    let power = g.power(raubahn).unwrap();
    shock(&mut g, 1, Target::Object(raubahn));
    until_asked(&mut g);
    cite!(["702.21a", "119.4"]; assert!(matches!(g.asked(), Some((1, Asked::Pay { life, .. })) if life == power as u32), "{:?}", g.asked()));
    g.act(1, Move::Choose(Answer::Pay { plan: vec![], with: None, tapped: vec![] })).unwrap();
    cite!(["702.21a", "119.4"]; assert_eq!(g.players[1].life, 20 - power));
}

#[track_caller]
fn act_unchanged(g:&mut Game,player:usize,action:Move)->Result<(),String>{
 let before=format!("{g:?}");let result=g.act(player,action);assert_eq!(format!("{g:?}"),before);result
}
