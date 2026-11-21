//! CR 702.81: retrace grants a graveyard casting permission and one additional land discard.
use crate::support::cite;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use game::announcement::{CastCost, CastRoute};
use game::simulation::Simulation;

const SIX: &str = "Reach\nWhenever Six attacks, mill three cards. You may put a land card from among them into your hand.\nDuring your turn, nonland permanent cards in your graveyard have retrace. (You may cast permanent cards from your graveyard by discarding a land card in addition to paying their other costs.)";
fn defs() -> Vec<CardDef> {
    let defs = vec![
        CardDef::new("Six", "{2}{G}", "Legendary Creature — Treefolk", SIX, Some((2,4))).unwrap(),
        CardDef::new("Bear", "{1}", "Creature — Bear", "Vigilance", Some((2,2))).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None).unwrap(),
        CardDef::new("Instant witness", "{0}", "Instant", "You gain 1 life.", None).unwrap(),
        CardDef::new("Raven's Crime", "{B}", "Sorcery", "Target player discards a card.\nRetrace (You may cast this card from your graveyard by discarding a land card in addition to paying its other costs.)", None).unwrap(),
        CardDef::new("Removal witness", "{0}", "Instant", "Destroy target creature.", None).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap_or_else(|e| panic!("{}: {e:?}", def.name())); }
    defs
}
fn retrace(card: ObjId, land: ObjId) -> Move {
    Move::CastVia(CastRoute::Retrace, Box::new(Move::WithCosts(vec![vec![land]], Box::new(Move::Cast { card, targets: vec![] }))))
}
fn respond(sim: &mut Simulation, action: Move) {
    let request = sim.request().unwrap().unwrap().clone();
    sim.respond(request.id, request.player, action).unwrap();
}
fn resolve(sim: &mut Simulation) {
    for _ in 0..sim.game().unwrap().players.len() { respond(sim, Move::Pass); }
}
#[test]
fn r702_81_full_six_definition_compiles() { defs(); }
#[test]
fn r702_81_a_graveyard_grant_keeps_printed_abilities_and_limits_recipients_by_zone_owner_type_and_turn() {
    let mut game = Game::new(defs(), 3, 20);
    game.put(1, 0, Zone::Battlefield);
    let own = game.put(1, 1, Zone::Graveyard);
    let theirs = game.put(2, 1, Zone::Graveyard);
    let hand = game.put(1, 1, Zone::Hand);
    let land = game.put(1, 2, Zone::Graveyard);
    let instant = game.put(1, 3, Zone::Graveyard);
    let retrace = ir::Keyword::from_name("Retrace").unwrap();
    let vigilance = ir::Keyword::from_name("Vigilance").unwrap();
    for active in 0..3 {
        game.active = active;
        // Six's "During your turn, nonland permanent cards in your graveyard have retrace" applies at each moment to what its text indicates (611.3a)
        cite!(["611.3a"]; case: format!("active {active}"); assert_eq!(game.seen(own, None).has(retrace), active == 1));
        // Uncited (card text): the Bear's printed "Vigilance"; Six's grant only adds retrace
        assert!(game.seen(own, None).has(vigilance));
        for other in [theirs, hand, land, instant] { cite!(["611.3a"]; case: format!("active {active}"); assert!(!game.seen(other, None).has(retrace))); }
    }
}
#[test]
fn r702_81_an_explicit_retrace_cast_pays_a_land_and_survives_the_grant_source_leaving() {
    let mut game = Game::new(defs(), 2, 20);
    let six = game.put(0, 0, Zone::Battlefield);
    let bear = game.put(0, 1, Zone::Graveyard);
    let land = game.put(0, 2, Zone::Hand);
    let remove = game.put(0, 5, Zone::Hand);
    game.players[0].pool.colorless = 1;
    let mut sim = Simulation::new(game);
    respond(&mut sim, crate::support::paid("C",retrace(bear, land)));
    cite!(["702.81a"]; assert!(sim.game().unwrap().players[0].hand.iter().all(|c| *c != land)));
    assert_eq!(sim.game().unwrap().players[0].pool.total(), 0);
    respond(&mut sim, Move::Cast { card: remove, targets: vec![vec![Target::Object(six)]] });
    resolve(&mut sim);
    resolve(&mut sim);
    let game = sim.game().unwrap();
    assert_eq!(game.battlefield.len(), 1);
    assert_eq!(game.obj(game.battlefield[0]).card, Some(1));
    assert!(game.seen(game.battlefield[0], None).has(ir::Keyword::from_name("Vigilance").unwrap()));
    assert!(!game.seen(game.battlefield[0], None).has(ir::Keyword::from_name("Retrace").unwrap()));
}
#[test]
fn r601_2h_illegal_retrace_costs_leave_the_card_mana_and_hand_unchanged() {
    let mut game = Game::new(defs(), 3, 20);
    game.put(0, 0, Zone::Battlefield);
    let bear = game.put(0, 1, Zone::Graveyard);
    let land = game.put(0, 2, Zone::Hand);
    let other_land = game.put(1, 2, Zone::Hand);
    let not_land = game.put(0, 3, Zone::Hand);
    game.players[0].pool.colorless = 1;
    let mut sim = Simulation::new(game);
    let request = sim.request().unwrap().unwrap().clone();
    let before = format!("{:?}",sim.game().unwrap());
    for (action, rule, why) in [
        // no land discarded
        (Move::CastVia(CastRoute::Retrace, Box::new(Move::Cast { card: bear, targets: vec![] })), "702.81a", "that selection does not satisfy the cost, which asks for exactly 1 card(s): 0 chosen (601.2h, 701.9a)"),
        (retrace(bear, other_land), "702.81a", "Forest is not what that cost asks for or cannot pay it (701.9a, 118.3)"),
        (retrace(bear, not_land), "702.81a", "Instant witness is not what that cost asks for or cannot pay it (701.9a, 118.3)"),
        (retrace(bear, bear), "702.81a", "Bear is not what that cost asks for or cannot pay it (701.9a, 118.3)"),
        (Move::CastVia(CastRoute::Normal, Box::new(Move::Cast { card: bear, targets: vec![] })), "601.3", "that card is not in your hand: it is in a graveyard, and no rule or effect lets you cast it from there (601.3)"),
    ] {
        cite!([rule]; case: why; refused(why): sim.respond(request.id,0,action));
        assert_eq!(format!("{:?}",sim.game().unwrap()),before);
    }
    respond(&mut sim,crate::support::paid("C",retrace(bear,land)));
}

#[test]
fn r118_9d_retrace_combines_with_an_alternative_cost_while_flashback_uses_its_own_cost_and_destination() {
    let spell = CardDef::new("Route witness", "{3}", "Sorcery",
        "You gain 1 life.\nRetrace\nFlashback {1}\nYou may cast this card from your graveyard.\nYou may pay 2 life rather than pay this spell's mana cost.", None).unwrap();
    game::play::check(&spell).unwrap();
    let mut definitions = defs();
    definitions.push(spell);
    for route in [CastRoute::Normal, CastRoute::Flashback, CastRoute::Retrace] {
        for alternative in [false, true] {
            let mut game = Game::new(definitions.clone(), 2, 20);
            let card = game.put(0,6,Zone::Graveyard);
            let land = game.put(0,2,Zone::Hand);
            game.players[0].pool.colorless = 3;
            let cost = if alternative { CastCost::Alternative(4) } else { CastCost::Mana };
            let mut sim = Simulation::new(game);
            let request = sim.request().unwrap().unwrap().clone();
            let accepted = !(route == CastRoute::Flashback && alternative);
            let action = Move::CastVia(route, Box::new(Move::CastFor(cost, Box::new(Move::WithCosts(
                if route == CastRoute::Retrace { vec![vec![land]] } else { vec![] }, Box::new(Move::Cast { card, targets: vec![] }))))));
            if !accepted {
                let before = format!("{:?}",sim.game().unwrap());
                // flashback is itself an alternative cost (702.34a)
                crate::support::refused(sim.respond(request.id,0,action), "only one alternative cost is paid for a spell (118.9a)");
                assert_eq!(format!("{:?}",sim.game().unwrap()),before);
                continue;
            }
            respond(&mut sim,crate::support::paid(if alternative { "" } else if route == CastRoute::Flashback { "C" } else { "CCC" },action));
            let game = sim.game().unwrap();
            assert_eq!(game.players[0].pool.total(), if alternative {3} else if route == CastRoute::Flashback {2} else {0});
            assert_eq!(game.players[0].life, if alternative {18} else {20});
            cite!(["702.81a"]; case: format!("{route:?} alternative {alternative}"); assert_eq!(game.players[0].hand.contains(&land), route != CastRoute::Retrace));
            resolve(&mut sim);
            let game = sim.game().unwrap();
            cite!(["702.34a"]; case: format!("{route:?} alternative {alternative}"); assert_eq!(game.exile.len(), usize::from(route == CastRoute::Flashback)));
            assert_eq!(game.players[0].graveyard.iter().filter(|o| game.obj(**o).card == Some(6)).count(), usize::from(route != CastRoute::Flashback));
        }
    }
}

#[test]
fn r601_2f_retrace_cost_is_locked_when_its_granting_source_is_sacrificed_to_the_same_spell() {
    let mut definitions = defs();
    definitions.push(CardDef::new("Sacrifice witness", "{0}", "Creature — Bear", "As an additional cost to cast this spell, sacrifice a creature.", Some((3,3))).unwrap());
    game::play::check(definitions.last().unwrap()).unwrap();
    let mut game = Game::new(definitions, 2, 20);
    let six = game.put(0,0,Zone::Battlefield);
    let spell = game.put(0,6,Zone::Graveyard);
    let land = game.put(0,2,Zone::Hand);
    let mut sim = Simulation::new(game);
    respond(&mut sim, Move::CastVia(CastRoute::Retrace,Box::new(Move::WithCosts(vec![vec![six],vec![land]],Box::new(Move::Cast {card:spell,targets:vec![]})))));
    assert!(sim.game().unwrap().battlefield.is_empty());
    assert_eq!(sim.game().unwrap().players[0].graveyard.len(),2);
    resolve(&mut sim);
    cite!(["601.2f"]; assert_eq!(sim.game().unwrap().battlefield.len(),1));
}

#[test]
fn r702_81_no_land_no_permission_or_wrong_turn_is_refused() {
    for (with_six, active, with_land, with_mana) in [(false,0,true,true),(true,1,true,true),(true,0,false,true),(true,0,true,false)] {
        let mut game = Game::new(defs(),2,20);
        game.active=active;
        game.priority=0;
        if with_six { game.put(0,0,Zone::Battlefield); }
        let card=game.put(0,1,Zone::Graveyard);
        let land=game.put(0,2,if with_land {Zone::Hand} else {Zone::Battlefield});
        game.players[0].pool.colorless=u32::from(with_mana);
        let mut sim=Simulation::new(game);
        let request=sim.request().unwrap().unwrap().clone();
        let before=format!("{:?}",sim.game().unwrap());
        // no Six, or not its controller's turn: no retrace; a land on the battlefield is no land
        // card in hand to discard; no mana for {1}
        let why = match (with_six, active, with_land, with_mana) {
            (false, ..) | (_, 1, ..) => "that card cannot be cast using retrace (702.81a)",
            (_, _, false, _) => "Forest is not what that cost asks for or cannot pay it (701.9a, 118.3)",
            _ => "your mana pool cannot pay that cost (118.3a): {1}",
        };
        crate::support::refused(sim.respond(request.id,0,retrace(card,land)), why);
        assert_eq!(format!("{:?}",sim.game().unwrap()),before);
    }
}

#[test]
fn r603_3_attack_mills_three_then_offers_only_lands_from_that_committed_group() {
    for accept in [false,true] {
        let mut game=Game::new(defs(),2,20);
        let six=game.put(0,0,Zone::Battlefield);
        let old_land=game.put(0,2,Zone::Graveyard);
        let a=game.put(0,2,Zone::Library);
        let b=game.put(0,2,Zone::Library);
        let c=game.put(0,1,Zone::Library);
        let mut sim=Simulation::new(game);
        resolve(&mut sim);
        resolve(&mut sim);
        respond(&mut sim,Move::Attack(vec![(six,Target::Player(1))]));
        resolve(&mut sim);
        respond(&mut sim,Move::Choose(game::state::Answer::Objects(vec![a,b,c])));
        assert!(sim.game().unwrap().players[0].library.is_empty());
        cite!(["701.17a"]; case: format!("accept {accept}"); assert_eq!(sim.game().unwrap().players[0].graveyard.len(),4));
        assert!(matches!(sim.request().unwrap().unwrap().question,game::legal::Asked::May));
        respond(&mut sim,Move::Choose(if accept {game::state::Answer::Yes} else {game::state::Answer::No}));
        if accept {
            let request=sim.request().unwrap().unwrap().clone();
            let game::legal::Asked::Objects {among,count:1,..}=request.question else {panic!("land selection: {request:?}")};
            assert_eq!(among.len(),2);
            assert!(!among.contains(&old_land));
            let mut branch=sim.clone();
            crate::support::refused(sim.respond(request.id,0,Move::Choose(game::state::Answer::Objects(vec![old_land]))), "a chosen object is not among the offered objects to return (608.2d)");
            for sim in [&mut sim,&mut branch] {
                respond(sim,Move::Choose(game::state::Answer::Objects(vec![among[0]])));
                assert_eq!(sim.game().unwrap().players[0].hand.len(),1);
                assert_eq!(sim.game().unwrap().players[0].graveyard.len(),3);
            }
        } else { assert!(sim.game().unwrap().players[0].hand.is_empty()); }
    }
}

#[test]
fn r601_3e_retrace_permission_checks_the_selected_adventure_characteristics() {
    let creature=CardDef::new("Adventurer", "{0}", "Creature — Bear", "", Some((2,2))).unwrap();
    let adventure=CardDef::new("Journey", "{0}", "Sorcery — Adventure", "You gain 1 life.", None).unwrap();
    let mut definitions=defs();
    definitions.push(CardDef::adventurer(creature,adventure).unwrap());
    let mut game=Game::new(definitions,2,20);
    game.put(0,0,Zone::Battlefield);
    let card=game.put(0,6,Zone::Graveyard);
    let land=game.put(0,2,Zone::Hand);
    let mut sim=Simulation::new(game);
    let request=sim.request().unwrap().unwrap().clone();
    crate::support::refused(sim.respond(request.id,0,Move::CastAs(game::card::CastAs::Adventure,Box::new(retrace(card,land)))), "that card cannot be cast using retrace (702.81a)");
    respond(&mut sim,retrace(card,land));
    resolve(&mut sim);
    assert_eq!(sim.game().unwrap().battlefield.len(),2);
}

#[test]
fn r613_1f_removing_the_sources_abilities_removes_its_graveyard_grant() {
    let mut definitions=defs();
    definitions.push(CardDef::new("Removal witness", "{0}", "Instant", "Target creature loses all abilities until end of turn.", None).unwrap());
    game::play::check(definitions.last().unwrap()).unwrap();
    let mut game=Game::new(definitions,2,20);
    let six=game.put(0,0,Zone::Battlefield);
    let card=game.put(0,1,Zone::Graveyard);
    game.put(0,2,Zone::Hand);
    let spell=game.put(0,6,Zone::Hand);
    let mut sim=Simulation::new(game);
    assert!(sim.game().unwrap().seen(card,None).has(ir::Keyword::from_name("Retrace").unwrap()));
    respond(&mut sim,Move::Cast {card:spell,targets:vec![vec![Target::Object(six)]]});
    resolve(&mut sim);
    cite!(["613.1f"]; assert!(!sim.game().unwrap().seen(card,None).has(ir::Keyword::from_name("Retrace").unwrap())));
}

#[test]
fn r733_1_reversing_suspended_retrace_restores_the_graveyard_card_and_keeps_the_land_in_hand() {
    let mut definitions=defs();
    definitions.push(CardDef::new("Mana creature", "{0}", "Creature — Elf", "{T}: You gain 2 life. Put a +1/+1 counter on this creature. Add X mana of any one color, where X is the number of +1/+1 counters on this creature.", Some((1,1))).unwrap());
    definitions.push(CardDef::new("Scales", "{0}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.", None).unwrap());
    definitions.push(CardDef::new("Evolution", "{0}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on it instead.", None).unwrap());
    definitions.push(CardDef::new("Payment witness", "{4}", "Creature — Bear", "", Some((4,4))).unwrap());
    for def in &definitions { game::play::check(def).unwrap(); }
    let mut game=Game::new(definitions,2,20);
    game.put(0,0,Zone::Battlefield);
    let source=game.put(0,6,Zone::Battlefield);
    game.put(0,7,Zone::Battlefield);
    let evolution=game.put(0,8,Zone::Battlefield);
    let card=game.put(0,9,Zone::Graveyard);
    let land=game.put(0,2,Zone::Hand);
    let mut sim=Simulation::new(game);
    let step=game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![], source,ability:0,paying:vec![],adding:vec![vec![ir::mana::ManaSymbol::Color(ir::Color::Green);3]]};
    respond(&mut sim,Move::WithMana(vec![step],Box::new(retrace(card,land))));
    let game=sim.game().unwrap();
    let Some(game::state::Decision::Choose {choice:game::state::Choice::Replacement {among,..},..})=&game.decision else {panic!("replacement choice")};
    let effect=among.iter().find(|e| e.source==evolution).unwrap().clone();
    respond(&mut sim,Move::Choose(game::state::Answer::Replacement(effect)));
    assert!(matches!(sim.game().unwrap().decision,Some(game::state::Decision::Payment {..})));
    respond(&mut sim,Move::ReversePayment);
    let game=sim.game().unwrap();
    assert!(game.stack.is_empty());
    cite!(["733.1"]; assert!(game.players[0].graveyard.contains(&card)));
    cite!(["733.1"]; assert!(game.players[0].hand.contains(&land)));
    assert_eq!(game.obj(card).zone,Zone::Graveyard);
    assert_eq!(game.players[0].pool.total(),3);
    assert_eq!(game.players[0].life,22);
    assert!(game.obj(source).tapped);
    cite!(["733.1"]; assert!(!game.log.iter().any(|e| matches!(e,game::Event::Cast {..}))));
}

#[test]
fn r601_3_cage_prohibition_overrides_six_permission_without_spending_retrace_costs() {
    let mut cards = defs();
    cards.push(CardDef::new("Grafdigger's Cage", "{1}", "Artifact",
        "Creature cards in graveyards and libraries can't enter the battlefield.\nPlayers can't cast spells from graveyards or libraries.", None).unwrap());
    game::play::check(cards.last().unwrap()).unwrap();
    let mut g = Game::new(cards, 2, 20);
    g.put(0, 0, Zone::Battlefield);
    g.put(1, 6, Zone::Battlefield);
    let bear = g.put(0, 1, Zone::Graveyard);
    let land = g.put(0, 2, Zone::Hand);
    g.players[0].pool.colorless = 1;
    let mut sim = Simulation::new(g);
    let request = sim.request().unwrap().unwrap().clone();
    let before = format!("{:?}", sim.game().unwrap());
    crate::support::refused(sim.respond(request.id, request.player, retrace(bear, land)), "an effect prohibits casting that spell (601.3)");
    assert_eq!(format!("{:?}", sim.game().unwrap()), before);
}

#[test]
fn r601_2b_omitted_route_preserves_normal_cost_beside_a_flashback_permission() {
    let spell = CardDef::new("Route selection", "{3}", "Sorcery",
        "You gain 1 life.\nFlashback {1}\nYou may cast this card from your graveyard.", None).unwrap();
    game::play::check(&spell).unwrap();
    let mut g = Game::new(vec![spell], 2, 20);
    let card = g.put(0, 0, Zone::Graveyard);
    g.players[0].pool.colorless = 3;
    cite!(["601.2b", "118.9"]; ok: g.act(0, crate::support::paid("CCC",Move::Cast { card, targets: vec![] })));
    cite!(["601.2h"]; assert_eq!(g.players[0].pool.total(), 0));
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["702.34a", "608.2n"]; assert!(g.exile.is_empty()));
    assert_eq!(g.players[0].graveyard.len(), 1);
}

#[test]
fn r601_2b_omitted_route_never_selects_a_printed_flashback_cost() {
    let spell = CardDef::new("Flashback selection", "{3}", "Sorcery",
        "You gain 1 life.\nFlashback {1}", None).unwrap();
    game::play::check(&spell).unwrap();
    let mut g = Game::new(vec![spell], 2, 20);
    let card = g.put(0, 0, Zone::Graveyard);
    g.players[0].pool.colorless = 3;
    let before = format!("{g:?}");
    cite!(["601.2b"]; refused("that card is not in your hand: it is in a graveyard"): unchanged(&mut g, |g| g.act(0, crate::support::paid("CCC", Move::Cast { card, targets: vec![] }))));
    assert_eq!(format!("{g:?}"), before);
    cite!(["702.34a"]; ok: g.act(0, crate::support::paid("C",Move::CastVia(CastRoute::Flashback, Box::new(Move::Cast { card, targets: vec![] })))));
    assert_eq!(g.players[0].pool.total(), 2);
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["702.34a"]; assert_eq!(g.exile.len(), 1));
}

#[track_caller]
fn unchanged(g: &mut Game, action: impl FnOnce(&mut Game) -> Result<(), String>) -> Result<(), String> {
    let before=format!("{g:?}");
    let result=action(g);
    assert_eq!(format!("{g:?}"),before);
    result
}
