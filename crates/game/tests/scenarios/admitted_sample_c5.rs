//! A fixed selection of cards the engine runs. Oracle compositions with rule-derived, finite assertions.
use crate::support::{cite, oracle, paid};
use game::announcement::CastCost;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::{Color, ManaSymbol, Part};

fn setup(name: &str) -> Game {
    let mut g = Game::new(
        vec![
            oracle(name),
            oracle("Island"),
            CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
            CardDef::new("Goblin", "{0}", "Creature — Goblin", "", Some((1, 1))).unwrap(),
            CardDef::new("Enchant", "{0}", "Enchantment", "", None).unwrap(),
            CardDef::new("Legend", "{0}", "Legendary Creature — Human", "", Some((2, 2))).unwrap(),
            CardDef::new("Outlet", "{0}", "Artifact", "Discard a card: You gain 1 life.", None).unwrap(),
            CardDef::new("Ninja", "{0}", "Creature — Ninja", "", Some((1, 1))).unwrap(),
            CardDef::new("Shield", "{0}", "Creature — Spirit", "Indestructible", Some((2, 5))).unwrap(),
            CardDef::new("Artifact Bear", "{0}", "Artifact Creature — Bear", "", Some((2, 2))).unwrap(),
            CardDef::new("Uncounterable", "{0}", "Creature — Bear", "This spell can't be countered.", Some((2, 2)))
                .unwrap(),
            CardDef::new("Enchant Bear", "{0}", "Enchantment Creature — Bear", "", Some((2, 2))).unwrap(),
        ],
        2,
        20,
    );
    for p in 0..2 {
        for _ in 0..20 {
            g.put(p, 1, Zone::Library);
        }
    }
    g
}
fn pass(g: &mut Game) {
    for _ in 0..2 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn settle(g: &mut Game) {
    for _ in 0..30 {
        if g.stack.is_empty() {
            return;
        }
        assert!(g.decision.is_none(), "unexpected {:?}", g.decision);
        pass(g);
    }
    panic!("unsettled");
}
fn cast(g: &mut Game, p: usize, def: usize, targets: Vec<Vec<Target>>, spending: &str) -> ObjId {
    let card = g.put(p, def, Zone::Hand);
    g.act(p, paid(spending, Move::Cast { card, targets })).unwrap();
    *g.stack.last().unwrap()
}
fn activate(source: ObjId, ability: usize) -> Move {
    Move::Activate { source, ability, targets: vec![], paying: vec![] }
}
fn mana(g: &mut Game, generic: u32, color: Color, n: u32) {
    g.players[0].pool.colorless = generic;
    g.players[0].pool.colors[color as usize] = n;
}
fn until(g: &mut Game, turn: u32, part: Part) {
    for _ in 0..300 {
        if g.turn == turn && g.part == part {
            return;
        }
        let mv = match g.decision {
            Some(Decision::Attackers { .. }) => Move::Attack(vec![]),
            Some(Decision::Blockers { .. }) => Move::Block(vec![]),
            Some(ref d) => panic!("unexpected {d:?}"),
            None => Move::Pass,
        };
        g.act(g.decision_maker(g.priority), mv).unwrap();
    }
    panic!("boundary not reached");
}
fn attack(g: &mut Game, attackers: &[ObjId]) {
    pass(g);
    pass(g);
    g.act(0, Move::Attack(attackers.iter().map(|a| (*a, Target::Player(1))).collect())).unwrap();
}
fn unblocked(g: &mut Game) {
    settle(g);
    pass(g);
    assert!(matches!(g.decision,Some(Decision::Blockers{player:1})));
    g.act(1,Move::Block(vec![])).unwrap();
}

// Rules: 702.10b, 508.3d, 603.2, 111.3
// Zero attackers gives no trigger; one or two gives exactly one, even if Hordecaller stays home.
#[test]
fn r508_hordecaller_counts_attack_events_not_creatures() {
    for n in 0..=2 {
        let mut g = setup("Razorkin Hordecaller");
        let h = g.put(0, 0, Zone::Battlefield);
        let a = g.put(0, 2, Zone::Battlefield);
        let b = g.put(0, 2, Zone::Battlefield);
        cite!(["702.10b"]; assert!(g.has(h,"Haste")));
        attack(&mut g, &[a, b][..n]);
        cite!(["508.3d","603.2"]; assert_eq!(g.stack.len(),usize::from(n>0)));
        settle(&mut g);
        let tokens: Vec<_> = g.battlefield.iter().copied().filter(|o| g.obj(*o).card.is_none()).collect();
        cite!(["111.3"]; assert_eq!(tokens.len(),usize::from(n>0)));
        for t in tokens {
            cite!(["111.3"]; assert_eq!((g.power(t),g.toughness(t),g.controller(t)),(Some(1),Some(1),0)));
            cite!(["111.3"]; assert!(g.seen(t,None).subtypes.iter().any(|s|s.name()=="Gremlin") && g.seen(t,None).colors==vec![Color::Red]));
        }
    }
}

// Rules: 702.190a, 702.190b, 506.3a, 702.2b, 611.3a
// Sneak returns an unblocked attacker as cost; deathtouch applies only while attacking.
#[test]
fn r702_dark_leo_sneak_and_attacking_deathtouch() {
    let mut g = setup("Dark Leo & Shredder");
    let bear = g.put(0, 2, Zone::Battlefield);
    let home = g.put(0, 7, Zone::Battlefield);
    g.put(1,2,Zone::Battlefield); // Require an authored blockers declaration.
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[0] = 1;
    g.players[0].pool.colors[2] = 1;
    let sneaking = |card, returned| {
        paid("WB",Move::CastFor(
            CastCost::Alternative(0),
            Box::new(Move::WithCosts(vec![vec![returned]], Box::new(Move::Cast { card, targets: vec![] }))),
        ))
    };
    cite!(["702.190a"]; refused("a sneak cost is paid only during your declare blockers step"): act_unchanged(&mut g,0,sneaking(card,bear)));
    attack(&mut g, &[bear]);
    unblocked(&mut g);
    g.players[0].pool.colors[0] = 1;
    g.players[0].pool.colors[2] = 1;
    g.act(0, sneaking(card, bear)).unwrap();
    assert_eq!(g.players[0].pool.total(),0);
    assert_eq!(g.players[0].hand.len(),1);assert_eq!(g.obj(g.players[0].hand[0]).card,Some(2));
    cite!(["702.190a","400.7"]; assert_eq!(g.obj(bear).zone,Zone::Gone));
    settle(&mut g);
    let leo = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(0)).unwrap();
    cite!(["702.190b","506.3a"]; assert!(g.obj(leo).tapped && g.combat.attackers.contains(&(leo,Target::Player(1)))));
    cite!(["611.3a","702.2b"]; assert!(g.has(leo,"Deathtouch") && !g.has(home,"Deathtouch")));
    until(&mut g, 1, Part::PostcombatMain);
    cite!(["611.3a"]; assert!(!g.has(leo,"Deathtouch")));
}

// Rules: 603.2, 608.2c, 119.3, 107.1a, 111.3
// Dark Leo itself and the new token both count toward five; half of 19 rounded up is 10, after 1 combat damage.
#[test]
fn r603_dark_leo_token_precedes_five_ninja_life_check() {
    for ninjas in [2, 3] {
        let mut g = setup("Dark Leo & Shredder");
        let leo = g.put(0, 0, Zone::Battlefield);
        g.put(1,2,Zone::Battlefield); // Require an authored blockers declaration.
        for _ in 0..ninjas {
            g.put(0, 7, Zone::Battlefield);
        }
        attack(&mut g, &[leo]);
        unblocked(&mut g);
        pass(&mut g);
        cite!(["603.2","120.3a"]; assert_eq!((g.players[1].life,g.stack.len()),(19,1)));
        settle(&mut g);
        cite!(["608.2c","119.3","107.1a"]; assert_eq!(g.players[1].life,if ninjas==3 {9} else {19}));
        let token = *g.battlefield.iter().find(|o| g.obj(**o).card.is_none()).unwrap();
        cite!(["111.3"]; assert!(g.seen(token,None).subtypes.iter().any(|s|s.name()=="Ninja") && g.seen(token,None).colors==vec![Color::Black] && g.power(token)==Some(1) && g.toughness(token)==Some(1)));
    }
}

// Rules: 701.7a, 608.2c, 702.12b
// Destroying an enchantment (also a creature) loses 2; an indestructible enchantment does not.
#[test]
fn r608_technique_life_loss_requires_destroyed_enchantment() {
    for kind in [2, 4, 8, 11] {
        let mut g = setup("Shredder's Technique");
        if kind == 8 {
            std::sync::Arc::make_mut(&mut g.defs)[8] = CardDef::new(
                "Shield Enchantment",
                "{0}",
                "Enchantment Creature — Spirit",
                "Indestructible",
                Some((2, 5)),
            )
            .unwrap();
        }
        let target = g.put(1, kind, Zone::Battlefield);
        mana(&mut g, 2, Color::Black, 1);
        cast(&mut g, 0, 0, vec![vec![Target::Object(target)]], "CCB");
        settle(&mut g);
        cite!(["701.7a","608.2c"]; assert_eq!(g.players[0].life,if kind==4 || kind==11 {18} else {20}));
        cite!(["701.7a"]; assert_eq!(g.battlefield.contains(&target),kind==8));
    }
}

// Rules: 611.3a, 702.10b, 602.2b, 605.1a, 605.3b
// Another own Goblin grants haste and is paid as sacrifice, removing haste and adding BR immediately.
#[test]
fn r605_bolg_another_goblin_haste_and_mana_cost() {
    let mut g = setup("Bolg's Company");
    let b = g.put(0, 0, Zone::Battlefield);
    g.put(1, 3, Zone::Battlefield);
    cite!(["611.3a","702.10b"]; assert!(!g.has(b,"Haste")));
    let goblin = g.put(0, 3, Zone::Battlefield);
    cite!(["611.3a","702.10b"]; assert!(g.has(b,"Haste")));
    g.act(0, Move::Activate { source: b, ability: 1, targets: vec![], paying: vec![vec![goblin]] }).unwrap();
    cite!(["602.2b","605.3b"]; assert!(g.obj(b).tapped && !g.battlefield.contains(&goblin) && g.stack.is_empty()));
    cite!(["605.3b"]; assert_eq!((g.players[0].pool.colors[2],g.players[0].pool.colors[3]),(1,1)));
    cite!(["611.3a"]; assert!(!g.has(b,"Haste")));
}

// Rules: 601.2c, 701.6a, 608.2b
// Artifact creatures qualify; enchantments without creature type don't. Uncounterable spells remain.
#[test]
fn r701_scatter_creature_types_and_uncounterable_boundary() {
    for def in [2, 9, 10] {
        let mut g = setup("Essence Scatter");
        let target = cast(&mut g, 0, def, vec![], "");
        let scatter = g.put(0, 0, Zone::Hand);
        mana(&mut g, 1, Color::Blue, 1);
        g.act(0, paid("CU", Move::Cast { card: scatter, targets: vec![vec![Target::Object(target)]] })).unwrap();
        pass(&mut g);
        cite!(["701.6a"]; assert_eq!(g.stack.contains(&target),def==10));
        cite!(["701.6a","608.2b"]; assert_eq!(g.players[0].graveyard.len(),if def==10 {1} else {2}));
    }
    let mut g = setup("Essence Scatter");
    let target = cast(&mut g, 0, 4, vec![], "");
    let scatter = g.put(0, 0, Zone::Hand);
    mana(&mut g, 1, Color::Blue, 1);
    cite!(["601.2c"]; refused("target"): g.act(0,paid("CU",Move::Cast{card:scatter,targets:vec![vec![Target::Object(target)]]})));
    cite!(["601.2c"]; assert_eq!((g.stack.len(),g.players[0].pool.total()),(1,2)));
}

// Rules: 608.2b, 701.6a, 400.7
// Once the sole target leaves the stack, Scatter does not resolve.
#[test]
fn r608_scatter_departed_target_does_not_resolve() {
    let mut g = setup("Essence Scatter");
    let target = cast(&mut g, 0, 2, vec![], "");
    let scatter = g.put(0, 0, Zone::Hand);
    mana(&mut g, 1, Color::Blue, 1);
    g.act(0, paid("CU", Move::Cast { card: scatter, targets: vec![vec![Target::Object(target)]] })).unwrap();
    // Uncited (fixture): remove target directly to isolate resolution legality.
    g.counter(target);
    pass(&mut g);
    cite!(["608.2b"]; assert!(g.stack.is_empty() && g.players[0].graveyard.len()==2));
}

// Rules: 614.1d, 603.6a, 119.3, 305.2, 107.5, 605.3b
// Entry is tapped; one trigger gives one life. Tapped source can't pay; each choice adds only that color.
#[test]
fn r614_pym_entry_trigger_and_both_mana_choices() {
    for color in [Color::Green, Color::Blue] {
        let mut g = setup("Pym Technologies");
        let card = g.put(0, 0, Zone::Hand);
        g.act(0, Move::PlayLand(card)).unwrap();
        let land = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(0)).unwrap();
        let add = |c| Move::Adding(vec![vec![ManaSymbol::Color(c)]], Box::new(activate(land, 2)));
        cite!(["614.1d","603.6a"]; assert!(g.obj(land).tapped && g.stack.len()==1 && g.players[0].life==20));
        cite!(["107.5"]; refused("can't pay {T}"): g.act(0,add(color)));
        settle(&mut g);
        cite!(["119.3"]; assert_eq!((g.players[0].life,g.players[1].life),(21,20)));
        until(&mut g, 3, Part::PrecombatMain);
        cite!(["502.3"]; assert!(!g.obj(land).tapped));
        cite!(["106.1a","605.3b"]; refused("mana"): g.act(0,add(Color::Red)));
        g.act(0, add(color)).unwrap();
        cite!(["605.3b"]; assert!(g.decision.is_none() && g.stack.is_empty()));
        cite!(["605.3b"]; assert_eq!((g.players[0].pool.total(),g.players[0].pool.colors[color as usize]),(1,1)));
        cite!(["107.5"]; refused("can't pay {T}"): g.act(0,add(color)));
    }
}

// Rules: 113.7a, 603.6a, 119.3
// The entry trigger survives the land leaving. Fixture placement creates no entry event.
#[test]
fn r603_pym_entry_trigger_survives_source_departure() {
    let mut g = setup("Pym Technologies");
    let old = g.put(0, 0, Zone::Battlefield);
    cite!(["603.6a"]; assert!(g.stack.is_empty() && g.players[0].life==20));
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::PlayLand(card)).unwrap();
    let land = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(0) && **o != old).unwrap();
    // Uncited (fixture): departure while its entry trigger is pending.
    g.destroy(land);
    settle(&mut g);
    cite!(["113.7a","119.3"]; assert_eq!(g.players[0].life,21));
}

// Rules: 601.2f, 601.2h, 120.3e, 704.5g
// Own legendary creature saves exactly one generic; enemy legend doesn't. Damage is four.
#[test]
fn r601_wrath_legendary_discount_scope_and_damage() {
    for owner in [None, Some(0), Some(1)] {
        let mut g = setup("Wrath of the Bloodmane");
        if let Some(p) = owner {
            g.put(p, 5, Zone::Battlefield);
        }
        let target = g.put(1, 8, Zone::Battlefield);
        let card = g.put(0, 0, Zone::Hand);
        mana(&mut g, 1, Color::Red, 1);
        let mv = Move::Cast { card, targets: vec![vec![Target::Object(target)]] };
        if owner != Some(0) {
            let before = format!("{g:?}");
            cite!(["601.2f","601.2h"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0,paid("CR",mv.clone())));
            assert_eq!(format!("{g:?}"), before);
            mana(&mut g, 2, Color::Red, 1);
        }
        g.act(0, paid(if owner == Some(0) { "CR" } else { "CCR" }, mv)).unwrap();
        settle(&mut g);
        cite!(["120.3e"]; assert_eq!(g.obj(target).damage,4));
        cite!(["601.2f","601.2h"]; assert_eq!(g.players[0].pool.total(),0));
    }
}

// Rules: 702.187b, 307.1, 400.7, 120.3e
// Actual discard unlocks the alternative cost; resolving creates a new graveyard object without permission.
#[test]
fn r702_electro_actual_discard_mayhem_and_no_recast() {
    let mut g = setup("Electro's Bolt");
    let target = g.put(1, 8, Zone::Battlefield);
    let outlet = g.put(0, 6, Zone::Battlefield);
    let held = g.put(0, 0, Zone::Hand);
    let placed = g.put(0, 0, Zone::Graveyard);
    mana(&mut g, 1, Color::Red, 1);
    let mv = |card| {
        paid("CR",Move::CastFor(
            CastCost::Alternative(1),
            Box::new(Move::Cast { card, targets: vec![vec![Target::Object(target)]] }),
        ))
    };
    cite!(["702.187b"]; refused("that card is not in your hand: it is in a graveyard, and no rule or effect lets you cast it from there (601.3)"): act_unchanged(&mut g,0,mv(placed)));
    g.act(0, Move::Activate { source: outlet, ability: 0, targets: vec![], paying: vec![vec![held]] }).unwrap();
    settle(&mut g);
    let discarded = *g.players[0].graveyard.last().unwrap();
    mana(&mut g, 1, Color::Red, 1);
    g.act(0, mv(discarded)).unwrap();
    settle(&mut g);
    cite!(["120.3e","702.187b"]; assert_eq!((g.obj(target).damage,g.players[0].pool.total()),(4,0)));
    let resolved = *g.players[0].graveyard.last().unwrap();
    mana(&mut g, 1, Color::Red, 1);
    assert_ne!(resolved,discarded);
    cite!(["400.7","702.187b"]; refused("that card is not in your hand: it is in a graveyard, and no rule or effect lets you cast it from there (601.3)"): act_unchanged(&mut g,0,mv(resolved)));
}

// Rules: 702.177a, 602.2b, 122.1a, 111.3, 113.7a
// Exhaust is consumed at activation. Departure prevents its counter, but not the untargeted token.
#[test]
fn r702_camera_exhaust_once_and_source_departure() {
    for depart in [false, true] {
        let mut g = setup("Camera Launcher");
        let source = g.put(0, 0, Zone::Battlefield);
        g.players[0].pool.colorless = 3;
        g.act(0, paid("CCC",activate(source, 0))).unwrap();
        assert_eq!(g.players[0].pool.total(),0);
        g.players[0].pool.colorless = 3;
        cite!(["702.177a"]; refused("that ability is activated only once for this object"): act_unchanged(&mut g,0,paid("CCC",activate(source,0))));
        if depart {
            g.destroy(source);
        }
        settle(&mut g);
        if !depart {
            cite!(["122.1a"]; assert_eq!((g.power(source),g.toughness(source)),(Some(3),Some(3))));
        }
        let token = *g.battlefield.iter().find(|o| g.obj(**o).card.is_none()).unwrap();
        cite!(["111.3","113.7a"]; assert!(g.seen(token,None).subtypes.iter().any(|s|s.name()=="Thopter") && g.seen(token,None).types.contains(&ir::CardType::Artifact) && g.seen(token,None).colors.is_empty() && g.has(token,"Flying") && g.power(token)==Some(1) && g.toughness(token)==Some(1)));
    }
}

// Rules: 701.17b, 701.17a, 603.6a
// Optional mill two is indivisible: fewer than two cannot be chosen. With two, accept/decline are distinct.
#[test]
fn r701_daggerfang_optional_mill_requires_two_cards() {
    for (n, accept) in [(0, false), (1, false), (2, false), (2, true)] {
        let mut g = setup("Daggerfang Duo");
        g.players[0].library.clear();
        let milled: Vec<_> = (0..n).map(|_| g.put(0, 1, Zone::Library)).collect();
        mana(&mut g, 2, Color::Black, 1);
        cast(&mut g, 0, 0, vec![], "CCB");
        pass(&mut g);
        pass(&mut g);
        if n >= 2 {
            cite!(["701.17b"]; assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::May}))));
            g.act(0, Move::Choose(if accept { Answer::Yes } else { Answer::No })).unwrap();
            if accept { cite!(["404.3"]; ok: g.act(0, Move::Choose(Answer::Objects(milled.clone())))); }
        } else {
            cite!(["701.17b"]; assert!(g.decision.is_none()));
        }
        cite!(["701.17a","701.17b"]; assert_eq!((g.players[0].library.len(),g.players[0].graveyard.len()),if accept {(0,2)}else{(n,0)}));
    }
}

// Rules: 702.2b, 702.2c, 704.5h, 510.1c
// Three deathtouch damage destroys a 5-toughness blocker; blocked Duo deals no player damage.
#[test]
fn r702_daggerfang_deathtouch_destroys_larger_blocker() {
    let mut g = setup("Daggerfang Duo");
    let duo = g.put(0, 0, Zone::Battlefield);
    std::sync::Arc::make_mut(&mut g.defs)[8] =
        CardDef::new("Large", "{0}", "Creature — Beast", "", Some((1, 5))).unwrap();
    let large = g.put(1, 8, Zone::Battlefield);
    attack(&mut g, &[duo]);
    pass(&mut g);
    g.act(1, Move::Block(vec![(large, duo)])).unwrap();
    pass(&mut g);
    cite!(["702.2b","704.5h"]; assert!(!g.battlefield.contains(&large) && g.battlefield.contains(&duo)));
    cite!(["510.1c"]; assert_eq!((g.players[1].life,g.obj(duo).damage),(20,1)));
}

#[track_caller]
fn act_unchanged(g:&mut Game,player:usize,action:Move)->Result<(),String>{let before=format!("{g:?}");let result=g.act(player,action);assert_eq!(format!("{g:?}"),before);result}
