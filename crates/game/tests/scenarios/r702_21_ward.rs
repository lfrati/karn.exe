//! Ward (702.21a): "Whenever this permanent becomes the target of a spell or ability an
//! opponent controls, counter that spell or ability unless that player pays [cost]." Sire of
//! Seven Deaths is its Oracle text; the others are test cards.
use crate::support::cite;
use game::card::CardDef;
use game::legal::Asked;
use game::play::check;
use game::state::Answer;
use game::{Game, Move, ObjId, Target, Zone};
use ir::mana::ManaSymbol;

const SIRE: usize = 0;
const BEAR: usize = 1;
const SHOCK: usize = 2;
const PINGER: usize = 3;
const GROW: usize = 4;
const DENY: usize = 5;
const MOUNTAIN: usize = 6;

fn defs() -> Vec<CardDef> {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let sire = CardDef::from_faces(CardDef::catalog(&faces).get("Sire of Seven Deaths").unwrap()).unwrap();
    let card = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).expect(name);
    vec![
        sire,
        card("Warded Bear", "Creature — Bear", "Ward {2}", Some((2, 2))),
        card("Shock", "Instant", "Shock deals 2 damage to any target.", None),
        card("Pinger", "Artifact", "{T}: This artifact deals 1 damage to any target.", None),
        card("Grow", "Instant", "Target creature gets +1/+1 until end of turn.", None),
        card("Deny", "Instant", "Counter target spell.", None),
        CardDef::new("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None).unwrap(),
    ]
}

fn game(players: usize) -> Game {
    let mut g = Game::new(defs(), players, 20);
    for p in 0..players { for _ in 0..10 { g.put(p, MOUNTAIN, Zone::Library); } }
    g
}

/// Player `by` has priority (player 0 has passed it on to them) and casts Shock at `target`.
fn shock(g: &mut Game, by: usize, target: Target) -> ObjId {
    while g.priority != by { g.act(g.priority, Move::Pass).unwrap(); }
    let shock = g.put(by, SHOCK, Zone::Hand);
    g.act(by, Move::Cast { card: shock, targets: vec![vec![target]] }).unwrap();
    *g.stack.iter().find(|o| g.obj(**o).chars.name == "Shock").unwrap()
}

/// Pass priority until something is asked other than priority, or the stack is empty.
fn until_asked(g: &mut Game) {
    while g.decision.is_none() && !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
}

fn ward_on_stack(g: &Game) -> bool {
    g.stack.iter().any(|o| g.obj(*o).ability.is_some())
}

#[test]
fn r702_21_sire_is_admitted_and_unsupported_ward_costs_are_refused() {
    for d in defs() { check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0)); }
    // costs whose payment the engine doesn't perform; ward {X} chooses no X (702.21b)
    for (text, why) in [
        ("Ward {X}", "keyword: Ward"),
        ("Ward—Discard a card at random.", "choice: at random selection not lowered"),
        ("Ward—Get five poison counters.", "unless: a payment of GetCounters"),
        ("Ward—Collect evidence 4.", "unless: a payment of Numbered"),
    ] {
        let d = CardDef::new("Probe", "{0}", "Creature — Bear", text, Some((1, 1))).unwrap();
        assert_eq!(check(&d).map_err(|u| u.0), Err(why.to_string()), "{text}");
    }
    // Ward—Waterbend {4}: r701_67_waterbend
    for text in ["Ward—Waterbend {4}.", "Ward—Discard a card.", "Ward—Sacrifice a creature.", "Ward—Sacrifice three permanents.", "Ward—Pay life equal to this creature's power."] {
        let d = CardDef::new("Probe", "{0}", "Creature — Bear", text, Some((1, 1))).unwrap();
        cite!(["702.21a"]; case: text; assert!(check(&d).is_ok()));
    }
    let enchantment = CardDef::new("Probe", "{0}", "Enchantment", "Ward {1}", None).unwrap();
    check(&enchantment).unwrap();
}

#[test]
fn r702_21a_an_opponents_spell_is_countered_unless_they_pay() {
    // pays: the spell resolves
    let mut g = game(2);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    g.add_mana(1, ManaSymbol::Colorless);
    g.add_mana(1, ManaSymbol::Colorless);
    let spell = shock(&mut g, 1, Target::Object(bear));
    // the ward trigger is its controller's, above the spell
    cite!(["702.21a"]; assert!(ward_on_stack(&g)));
    cite!(["603.3a"]; assert_eq!(g.controller(*g.stack.last().unwrap()), 0));
    until_asked(&mut g);
    cite!(["702.21a"]; assert!(matches!(g.asked(), Some((1, Asked::Pay { life: 0, .. }))), "{:?}", g.asked()));
    cite!(["702.21a"]; refused("that decision is another player's"): act_unchanged(&mut g,0, Move::Choose(Answer::Pay { plan: vec![], with: None, tapped: vec![] })));
    g.act(1, Move::Choose(Answer::Pay { plan: vec![], with: Some(crate::support::mana_pool("CC")), tapped: vec![] })).unwrap();
    assert_eq!(g.players[1].pool.total(), 0);
    assert_eq!(g.stack, vec![spell]);
    until_asked(&mut g);
    cite!(["702.21a"]; assert!(!g.battlefield.contains(&bear), "Shock resolved"));
    // declines: countered
    let mut g = game(2);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    g.add_mana(1, ManaSymbol::Colorless);
    g.add_mana(1, ManaSymbol::Colorless);
    shock(&mut g, 1, Target::Object(bear));
    until_asked(&mut g);
    g.act(1, Move::Choose(Answer::No)).unwrap();
    assert!(g.stack.is_empty());
    assert!(g.battlefield.contains(&bear));
    assert_eq!(g.players[1].pool.total(), 2, "nothing paid");
    cite!(["702.21a", "701.6a"]; assert!(g.players[1].graveyard.iter().any(|o| g.obj(*o).chars.name == "Shock")));
    // An unaffordable supplied payment is refused; declining counters the spell.
    let mut g = game(2);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    shock(&mut g, 1, Target::Object(bear));
    until_asked(&mut g);
    cite!(["118.3"]; refused("that mana is not in your mana pool"): act_unchanged(&mut g,1,Move::Choose(Answer::Pay { plan: vec![], with: Some(crate::support::mana_pool("CC")), tapped: vec![] })));
    g.act(1, Move::Choose(Answer::No)).unwrap();
    cite!(["702.21a", "118.3"]; assert!(g.stack.is_empty() && g.battlefield.contains(&bear)));
}

#[test]
fn r702_21a_sire_ward_pay_7_life() {
    for (life, pays) in [(20, true), (7, true), (6, false)] {
        let mut g = game(2);
        let sire = g.put(0, SIRE, Zone::Battlefield);
        g.players[1].life = life;
        shock(&mut g, 1, Target::Object(sire));
        until_asked(&mut g);
        if pays {
            cite!(["702.21a"]; assert!(matches!(g.asked(), Some((1, Asked::Pay { life: 7, .. }))), "{:?}", g.asked()));
            // A life-only payment does not open a mana-ability window (605.3a).
            let land = g.put(1, MOUNTAIN, Zone::Battlefield);
            let step = game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source: land, ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None };
            let before = format!("{g:?}");
            cite!(["605.3a"]; refused("there is no mana payment during which to activate mana abilities (605.3a)"): g.act(1, Move::Choose(Answer::Pay { plan: vec![step], with: None, tapped: vec![] })));
            assert_eq!(format!("{g:?}"), before);
            // A copy at the question continues the same, nothing replayed.
            let mut copy = g.clone();
            for g in [&mut g, &mut copy] { g.act(1, Move::Choose(Answer::Pay { plan: vec![], with: None, tapped: vec![] })).unwrap(); }
            assert_eq!(format!("{g:?}"), format!("{copy:?}"));
            cite!(["119.4"]; case: format!("life {life}"); assert_eq!(g.players[1].life, life - 7));
            if life == 7 { cite!(["704.5a"]; assert!(g.players[1].lost, "paid to 0 life: lost (704.5a)")); continue; }
            until_asked(&mut g);
            assert_eq!(g.obj(sire).damage, 2, "Shock resolved");
        } else {
            // 119.4: not offered
            cite!(["119.4"]; assert!(g.stack.is_empty(), "{:?}", g.asked()));
            assert_eq!((g.players[1].life, g.obj(sire).damage), (6, 0));
        }
    }
}

#[test]
fn r702_21a_its_controllers_own_spells_do_not_trigger_it_an_opponents_abilities_do() {
    let mut g = game(2);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let grow = g.put(0, GROW, Zone::Hand);
    g.act(0, Move::Cast { card: grow, targets: vec![vec![Target::Object(bear)]] }).unwrap();
    cite!(["702.21a"]; assert!(!ward_on_stack(&g)));
    until_asked(&mut g);
    assert_eq!(g.power(bear), Some(3));
    // an opponent's activated ability: countered unless paid
    let pinger = g.put(1, PINGER, Zone::Battlefield);
    while g.priority != 1 { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(1, Move::Activate { source: pinger, ability: 0, targets: vec![vec![Target::Object(bear)]], paying: vec![] }).unwrap();
    cite!(["702.21a"]; assert!(g.stack.iter().filter(|o| g.obj(**o).ability.is_some()).count() == 2, "the ability and the ward trigger"));
    until_asked(&mut g);
    g.act(1, Move::Choose(Answer::No)).unwrap();
    cite!(["702.21a"]; assert!(g.stack.is_empty(), "the declined ward payment counters the ability"));
    assert_eq!(g.obj(bear).damage, 0);
}

#[test]
fn r702_21a_multiplayer_the_targeting_player_pays_and_the_warded_controller_controls() {
    let mut g = game(3);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    g.add_mana(2, ManaSymbol::Colorless);
    g.add_mana(2, ManaSymbol::Colorless);
    g.add_mana(1, ManaSymbol::Colorless);
    g.add_mana(1, ManaSymbol::Colorless);
    shock(&mut g, 2, Target::Object(bear));
    cite!(["603.3a"]; assert_eq!(g.controller(*g.stack.last().unwrap()), 0));
    until_asked(&mut g);
    cite!(["702.21a"]; assert!(matches!(g.asked(), Some((2, Asked::Pay { .. }))), "{:?}", g.asked()));
    cite!(["702.21a"]; refused("that decision is another player's"): g.act(1, Move::Choose(Answer::Pay { plan: vec![], with: None, tapped: vec![] })));
    g.act(2, Move::Choose(Answer::No)).unwrap();
    assert!(g.battlefield.contains(&bear));
    assert_eq!((g.players[1].pool.total(), g.players[2].pool.total()), (2, 2));
}

#[test]
fn r702_21a_the_spell_gone_or_the_permanent_gone_before_ward_resolves() {
    // the spell is countered first: ward has nothing to counter
    let mut g = game(2);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    g.add_mana(1, ManaSymbol::Colorless);
    g.add_mana(1, ManaSymbol::Colorless);
    let spell = shock(&mut g, 1, Target::Object(bear));
    while g.priority != 0 { g.act(g.priority, Move::Pass).unwrap(); }
    let deny = g.put(0, DENY, Zone::Hand);
    g.act(0, Move::Cast { card: deny, targets: vec![vec![Target::Object(spell)]] }).unwrap();
    for _ in 0..10 {
        if g.stack.is_empty() { break; }
        match g.asked() {
            Some((1, Asked::Pay { .. })) => g.act(1, Move::Choose(Answer::No)).unwrap(),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    cite!(["701.6a"]; assert!(g.stack.is_empty() && g.battlefield.contains(&bear)));
    assert_eq!(g.players[1].graveyard.iter().filter(|o| g.obj(**o).chars.name == "Shock").count(), 1);
    assert_eq!(g.players[1].pool.total(), 2);
    // the warded permanent leaves first: ward still counters the spell unless paid
    let mut g = game(2);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    g.add_mana(1, ManaSymbol::Colorless);
    g.add_mana(1, ManaSymbol::Colorless);
    shock(&mut g, 1, Target::Player(0));
    let bear_spell = shock(&mut g, 1, Target::Object(bear));
    let _ = bear_spell;
    g.destroy(bear);
    until_asked(&mut g);
    cite!(["113.7a"]; assert!(matches!(g.asked(), Some((1, Asked::Pay { .. }))), "{:?}", g.asked()));
    g.act(1, Move::Choose(Answer::No)).unwrap();
    until_asked(&mut g);
    assert_eq!(g.players[0].life, 18, "the other Shock resolved");
}

#[track_caller]
fn act_unchanged(g:&mut Game,player:usize,action:Move)->Result<(),String>{
 let before=format!("{g:?}");let result=g.act(player,action);assert_eq!(format!("{g:?}"),before);result
}
