//! Mana that remembers how it may be spent (CR 106.6), and the creature type a land has chosen
//! for it as it enters (614.12a), which its mana ability calls "the chosen type" (607.2d):
//! Cavern of Souls. Every card is read from its printed text.

use crate::support::cite;
use game::card::CardDef;
use game::legal::{Asked, Step};
use game::play::check;
use game::state::{Answer, Choice, Decision, Pool};
use game::{Event, Game, Move, ObjId, Target, Zone};
use ir::mana::ManaSymbol;
use ir::{Color, Subtype};

const CAVERN: usize = 0;
const MOUNTAIN: usize = 1;
const FOREST: usize = 2;
const RAIDERS: usize = 3;
const PIKER: usize = 4;
const TRADER: usize = 5;
const ELVES: usize = 6;
const BOLT: usize = 7;
const BRIGADE: usize = 8;
const BEARS: usize = 9;
const RESTORE: usize = 10;
const HALFLING: usize = 11;
const ISAMARU: usize = 12;
const ZIGGURAT: usize = 13;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card(
            "Cavern of Souls",
            "",
            "Land",
            "As this land enters, choose a creature type.\n{T}: Add {C}.\n{T}: Add one mana of any color. Spend this mana only to cast a creature spell of the chosen type, and that spell can't be countered.",
            None,
        ),
        card("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None),
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        card("Mons's Goblin Raiders", "{R}", "Creature — Goblin", "", Some((1, 1))),
        card("Goblin Piker", "{1}{R}", "Creature — Goblin Warrior", "", Some((2, 1))),
        card("Dwarven Trader", "{R}", "Creature — Dwarf", "", Some((1, 1))),
        card("Llanowar Elves", "{G}", "Creature — Elf Druid", "{T}: Add {G}.", Some((1, 1))),
        card("Lightning Bolt", "{R}", "Instant", "Lightning Bolt deals 3 damage to any target.", None),
        card("Goblin Balloon Brigade", "{R}", "Creature — Goblin Warrior", "{R}: This creature gains flying until end of turn.", Some((1, 1))),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Restore", "{1}{G}", "Sorcery", "Put target land card from a graveyard onto the battlefield under your control.", None),
        card(
            "Delighted Halfling",
            "{G}",
            "Creature — Halfling Citizen",
            "{T}: Add {C}.\n{T}: Add one mana of any color. Spend this mana only to cast a legendary spell, and that spell can't be countered.",
            Some((1, 2)),
        ),
        card("Isamaru, Hound of Konda", "{W}", "Legendary Creature — Dog", "", Some((2, 2))),
        card("Ancient Ziggurat", "", "Land", "{T}: Add one mana of any color. Spend this mana only to cast a creature spell.", None),
    ]
}

fn game() -> Game {
    Game::new(defs(), 2, 20)
}

fn creature_type(name: &str) -> game::binding::Value {
    game::binding::Value::CreatureType(Subtype::from_name(name).unwrap_or_else(|| panic!("{name} is a subtype")))
}

/// Player 0 plays Cavern of Souls from their hand and chooses this creature type for it.
fn play_cavern(g: &mut Game, chosen: &str) -> ObjId {
    let card = g.put(0, CAVERN, Zone::Hand);
    g.act(0, Move::PlayLand(card)).unwrap();
    g.act(0, Move::Choose(Answer::Value(creature_type(chosen)))).unwrap();
    *g.battlefield.last().expect("the land entered")
}

/// The mana of this color from the ability that offers one mana of any color.
fn tap_for(g: &mut Game, source: ObjId, ability: usize, color: Color) {
    let activate = Move::Activate { source, ability, targets: vec![], paying: vec![] };
    g.act(0, Move::Adding(vec![vec![ManaSymbol::Color(color)]], Box::new(activate))).unwrap();
}

/// Cavern of Souls' last ability: "{T}: Add one mana of any color. Spend this mana only to
/// cast a creature spell of the chosen type, and that spell can't be countered."
const RESTRICTED: usize = 2;

fn mana(g: &mut Game, lands: &[usize]) {
    for land in lands {
        let land = g.put(0, *land, Zone::Battlefield);
        g.act(0, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
}

fn pass_both(g: &mut Game) {
    let first = g.asked().expect("someone has priority").0;
    g.act(first, Move::Pass).unwrap();
    g.act(1 - first, Move::Pass).unwrap();
}

/// Exact ordinary units and authenticated lot indices supplied by each fixture.
fn selected(g:&Game, units:&str, lots:&[usize]) -> Pool {
    let mut pool = crate::support::mana_pool(units);
    pool.restricted = lots.iter().map(|i|g.players[0].pool.restricted[*i].clone()).collect();
    pool
}

fn cast(g: &mut Game, def: usize, units: &str, lots: &[usize]) -> Result<ObjId, String> {
    let card = g.put(0, def, Zone::Hand);
    g.act(0, Move::Paying(selected(g,units,lots),Box::new(Move::Cast { card, targets: vec![] })))?;
    Ok(*g.stack.last().expect("the spell is on the stack"))
}

/// The cards player 0 is offered to cast now.
#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r614_12a_a_creature_type_is_chosen_before_the_land_enters_by_the_player_who_plays_it() {
    let mut g = game();
    let card = g.put(0, CAVERN, Zone::Hand);
    g.act(0, Move::PlayLand(card)).unwrap();
    assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::Value(game::binding::Kind::CreatureType) }));
    cite!(["614.12a"]; assert!(g.battlefield.is_empty(), "the choice is made before it enters"));
    crate::support::refused(g.act(0, Move::Pass), "a decision is pending: a choice of how the permanent enters the battlefield (614.12)");
    // one of the creature types of 205.3m, and nothing else
    let Some((0, Asked::Value { kind: game::binding::Kind::CreatureType, options })) = g.asked() else { panic!("no creature type asked") };
    cite!(["205.3m"]; assert_eq!(options.len(), 324));
    cite!(["205.3m"]; assert!(options.contains(&creature_type("Goblin")) && options.contains(&creature_type("Time Lord"))));
    for not_one in ["Forest", "Equipment", "Aura"] {
        assert!(!options.contains(&creature_type(not_one)));
        cite!(["205.3m"]; refused("choose a creature type (205.3m)"): g.act(0, Move::Choose(Answer::Value(creature_type(not_one)))));
    }
    crate::support::refused(g.act(0, Move::Choose(Answer::Yes)), "choose a creature type (205.3m)");
    g.act(0, Move::Choose(Answer::Value(creature_type("Goblin")))).unwrap();
    let [cavern] = g.battlefield[..] else { panic!("not on the battlefield") };
    assert_eq!(g.obj(cavern).choices.first().map(|r| r.value.clone()), Some(creature_type("Goblin")));
    assert!(!g.obj(cavern).tapped);
    assert!(g.players[0].land_played);
    assert!(g.log.iter().any(|e| matches!(e, Event::LandPlayed { by: 0, .. })));
}

#[test]
fn r614_12a_a_creature_type_is_chosen_by_the_player_under_whose_control_an_effect_puts_the_land() {
    let mut g = game();
    // another player's card, put onto the battlefield under player 0's control (110.2a)
    let theirs = g.put(1, CAVERN, Zone::Graveyard);
    mana(&mut g, &[FOREST, FOREST]);
    let restore = g.put(0, RESTORE, Zone::Hand);
    g.act(0, crate::support::paid("GG",Move::Cast { card: restore, targets: vec![vec![Target::Object(theirs)]] })).unwrap();
    pass_both(&mut g);
    assert!(matches!(g.asked(), Some((0, Asked::Value { kind: game::binding::Kind::CreatureType, .. }))));
    cite!(["614.12a"]; assert!(g.battlefield.iter().all(|o| g.obj(*o).chars.name != "Cavern of Souls")));
    g.act(0, Move::Choose(Answer::Value(creature_type("Elf")))).unwrap();
    let cavern = *g.battlefield.last().unwrap();
    assert_eq!(g.obj(cavern).chars.name, "Cavern of Souls");
    cite!(["110.2a"]; assert_eq!((g.obj(cavern).controller, g.obj(cavern).owner), (0, 1)));
    // Uncited (ruling 3cd538ba62aca5f4): "As a Siege enters the battlefield, its controller chooses" (read ahead 421f37ab838d3522 likewise); 707.6 gives a copy's "as enters" choices to its controller too
    assert_eq!(g.obj(cavern).choices[0].chooser, 0, "the choice belongs to the instructed player, not the owner");
    assert_eq!(g.obj(cavern).choices.first().map(|r| r.value.clone()), Some(creature_type("Elf")));
    assert!(g.stack.is_empty(), "the sorcery resolved once the choice was made");
}

#[test]
fn r400_7_a_land_that_enters_again_is_a_new_object_and_its_creature_type_is_chosen_again() {
    let mut g = game();
    let cavern = play_cavern(&mut g, "Goblin");
    g.move_object(cavern, Zone::Graveyard);
    let [card] = g.players[0].graveyard[..] else { panic!("not in the graveyard") };
    cite!(["400.7"]; assert_eq!(g.obj(card).choices.first().map(|r| r.value.clone()), None, "the card in the graveyard is a new object"));
    mana(&mut g, &[FOREST, FOREST]);
    let restore = g.put(0, RESTORE, Zone::Hand);
    g.act(0, crate::support::paid("GG",Move::Cast { card: restore, targets: vec![vec![Target::Object(card)]] })).unwrap();
    pass_both(&mut g);
    g.act(0, Move::Choose(Answer::Value(creature_type("Elf")))).unwrap();
    let again = *g.battlefield.last().unwrap();
    assert_ne!(again, cavern);
    cite!(["400.7"]; assert_eq!(g.obj(again).choices.first().map(|r| r.value.clone()), Some(creature_type("Elf"))));
}

#[test]
fn r106_6_mana_restricted_to_a_kind_of_spell_pays_only_for_one() {
    let mut g = game();
    let cavern = play_cavern(&mut g, "Goblin");
    tap_for(&mut g, cavern, RESTRICTED, Color::Red);
    // it keeps its type (106.6) and is in the pool as itself
    let pool = &g.players[0].pool;
    assert_eq!((pool.colors, pool.colorless, pool.restricted.len()), ([0; 5], 0, 1));
    cite!(["106.6"]; assert_eq!(pool.restricted[0].mana, ManaSymbol::Color(Color::Red)));
    // not on a noncreature spell, a creature spell of another type, or an ability
    let bolt = g.put(0, BOLT, Zone::Hand);
    cite!(["106.6"]; refused("that mana can't be spent on that: its text restricts how it can be spent (106.6)"): g.act(0, Move::Paying(selected(&g,"",&[0]),Box::new(Move::Cast { card: bolt, targets: vec![vec![Target::Player(1)]] }))));
    cite!(["106.6"]; refused("that mana can't be spent on that: its text restricts how it can be spent (106.6)"): cast(&mut g, TRADER, "", &[0]));
    let brigade = g.put(0, BRIGADE, Zone::Battlefield);
    cite!(["106.6"]; refused("that mana can't be spent on that: its text restricts how it can be spent (106.6)"): g.act(0, Move::Paying(selected(&g,"",&[0]),Box::new(Move::Activate { source: brigade, ability: 0, targets: vec![], paying: vec![] }))));
    // said to be spent on what it may not be, it is not (107.4b)
    let paying = g.players[0].pool.clone();
    let cast_bolt = Move::Cast { card: bolt, targets: vec![vec![Target::Player(1)]] };
    cite!(["106.6"]; refused("that mana can't be spent on that: its text restricts how it can be spent (106.6)"): g.act(0, Move::Paying(paying, Box::new(cast_bolt))));
    assert_eq!(g.players[0].pool.restricted.len(), 1, "nothing was spent");
    // on a creature spell of the chosen type
    let raiders = cast(&mut g, RAIDERS, "", &[0]).unwrap();
    assert_eq!(g.obj(raiders).chars.name, "Mons's Goblin Raiders");
    cite!(["106.6"]; assert_eq!(g.players[0].pool.total(), 0));
}

#[test]
fn r106_6_restricted_and_unrestricted_mana_pay_one_cost_together() {
    let mut g = game();
    let cavern = play_cavern(&mut g, "Goblin");
    tap_for(&mut g, cavern, RESTRICTED, Color::Red);
    mana(&mut g, &[MOUNTAIN]);
    // {1}{R}: both of them
    cast(&mut g, PIKER, "R", &[0]).unwrap();
    cite!(["106.6"]; assert_eq!(g.players[0].pool.total(), 0));
    pass_both(&mut g);
    // Choose the restricted unit explicitly, leaving ordinary R for Lightning Bolt.
    let mut g = game();
    let cavern = play_cavern(&mut g, "Goblin");
    tap_for(&mut g, cavern, RESTRICTED, Color::Red);
    mana(&mut g, &[MOUNTAIN]);
    cast(&mut g, RAIDERS, "", &[0]).unwrap();
    let pool = &g.players[0].pool;
    assert!(pool.restricted.is_empty());
    assert_eq!(pool.colors[Color::Red as usize], 1);
    let bolt = g.put(0, BOLT, Zone::Hand);
    g.act(0, crate::support::paid("R",Move::Cast { card: bolt, targets: vec![vec![Target::Player(1)]] })).unwrap();
}

#[test]
fn r106_6_mana_spent_on_a_spell_can_have_an_effect_on_that_spell() {
    // "and that spell can't be countered": what it was spent on is marked so
    let mut g = game();
    let cavern = play_cavern(&mut g, "Goblin");
    tap_for(&mut g, cavern, RESTRICTED, Color::Red);
    let raiders = cast(&mut g, RAIDERS, "", &[0]).unwrap();
    cite!(["106.6"]; assert!(g.obj(raiders).uncounterable));
    pass_both(&mut g);
    // not with other mana
    mana(&mut g, &[MOUNTAIN, MOUNTAIN]);
    let piker = cast(&mut g, PIKER, "RR", &[]).unwrap();
    cite!(["106.6"]; assert!(!g.obj(piker).uncounterable));
    pass_both(&mut g);
    // "to cast a legendary spell, and that spell can't be countered"
    let halfling = g.put(0, HALFLING, Zone::Battlefield);
    tap_for(&mut g, halfling, 1, Color::Red);
    cite!(["106.6"]; refused("that mana can't be spent on that: its text restricts how it can be spent (106.6)"): cast(&mut g, RAIDERS, "", &[0]));
    let halfling = g.put(0, HALFLING, Zone::Battlefield);
    tap_for(&mut g, halfling, 1, Color::White);
    let isamaru = cast(&mut g, ISAMARU, "", &[1]).unwrap();
    cite!(["106.6"]; assert!(g.obj(isamaru).uncounterable));
    pass_both(&mut g);
    // a restriction with no effect on the spell: "Spend this mana only to cast a creature
    // spell.", beside the red mana of the Halfling that no spell here may be paid with
    let ziggurat = g.put(0, ZIGGURAT, Zone::Battlefield);
    tap_for(&mut g, ziggurat, 0, Color::Red);
    let from_ziggurat = g.players[0].pool.restricted.iter().find(|m| m.source == ziggurat).expect("its mana");
    assert!(!from_ziggurat.uncounterable);
    let bolt = g.put(0, BOLT, Zone::Hand);
    cite!(["106.6"]; refused("that mana can't be spent on that: its text restricts how it can be spent (106.6)"): g.act(0, Move::Paying(selected(&g,"",&[1]),Box::new(Move::Cast { card: bolt, targets: vec![vec![Target::Player(1)]] }))));
    let trader = cast(&mut g, TRADER, "", &[1]).unwrap();
    cite!(["106.6"]; assert!(!g.obj(trader).uncounterable));
    assert!(g.players[0].pool.restricted.iter().all(|m| m.source != ziggurat), "the Ziggurat's mana paid");
}

#[test]
fn r106_6_authored_mana_payments_respect_spell_restrictions() {
    for mountain in [false,true] {
        for def in [RAIDERS,TRADER,BOLT,PIKER,ELVES] {
            let mut g=game();
            let cavern=play_cavern(&mut g,"Goblin");
            let card=g.put(0,def,Zone::Hand);
            let mut plan=vec![Step { plan: vec![], x: None, life: vec![], with: None, source:cavern, ability:RESTRICTED, paying:vec![], adding:vec![vec![ManaSymbol::Color(Color::Red)]], chose:vec![], cost_branches:None }];
            if mountain {
                let source=g.put(0,MOUNTAIN,Zone::Battlefield);
                plan.push(Step { plan: vec![], x: None, life: vec![], with: None, source, ability:0, paying:vec![], adding:vec![], chose:vec![], cost_branches:None });
            }
            let targets=if def==BOLT {vec![vec![Target::Player(1)]]} else {vec![]};
            // Bind the first produced lot from this exact authored activation. No
            // source, output color or spending choice is discovered by the clone.
            let mut production = g.clone();
            tap_for(&mut production,cavern,RESTRICTED,Color::Red);
            let (plain,lots):(&str,&[usize]) = match (def,mountain) {
                (RAIDERS,_) => ("",&[0]),
                (PIKER,true) => ("R",&[0]),
                (TRADER|BOLT,true) => ("R",&[]),
                _ => ("",&[0]),
            };
            let with = selected(&production,plain,lots);
            cite!(["106.6"]; case:format!("{def} mountain={mountain}"); assert_eq!(g.act(0,Move::Paying(with,Box::new(Move::WithMana(plan,Box::new(Move::Cast{card,targets}))))).is_ok(), def==RAIDERS || mountain && def!=ELVES));
        }
    }
    for def in [RAIDERS,TRADER,BOLT] {
        let mut g=game();let cavern=play_cavern(&mut g,"Goblin");
        tap_for(&mut g,cavern,RESTRICTED,Color::Red);
        let card=g.put(0,def,Zone::Hand);
        let targets=if def==BOLT {vec![vec![Target::Player(1)]]} else {vec![]};
        cite!(["106.6"]; case:def; assert_eq!(g.act(0,Move::Paying(selected(&g,"",&[0]),Box::new(Move::Cast{card,targets}))).is_ok(),def==RAIDERS));
    }
}

#[test]
fn r101_2_cavern_mana_prevents_countering_but_not_targeting_or_later_instructions() {
    let mut definitions = defs();
    let denial = definitions.len();
    definitions.push(CardDef::new("Denial", "{0}", "Instant", "Counter target spell. You gain 2 life.", None).unwrap());
    let mut g = Game::new(definitions, 2, 20);
    let cavern = play_cavern(&mut g, "Goblin");
    tap_for(&mut g, cavern, RESTRICTED, Color::Red);
    let raiders = cast(&mut g, RAIDERS, "", &[0]).unwrap();
    g.act(0, Move::Pass).unwrap();
    let response = g.put(1, denial, Zone::Hand);
    g.act(1, Move::Cast { card: response, targets: vec![vec![Target::Object(raiders)]] }).unwrap();
    g.act(1, Move::Pass).unwrap();
    g.act(0, Move::Pass).unwrap();
    cite!(["101.2"]; assert_eq!(g.stack, [raiders], "can't wins over can (101.2)"));
    cite!(["101.3"]; assert_eq!(g.players[1].life, 22, "the counterspell resolved and followed its other instruction"));
    assert!(!g.log.contains(&Event::Countered { object: raiders }));
    pass_both(&mut g);
    assert!(g.battlefield.iter().any(|o| g.obj(*o).card == Some(RAIDERS)));
}

#[test]
fn r106_6_mana_its_text_does_not_restrict_is_spent_on_anything() {
    // "{T}: Add {C}.": plain colorless mana, for the generic part of any cost
    let mut g = game();
    let cavern = play_cavern(&mut g, "Goblin");
    g.act(0, Move::Activate { source: cavern, ability: 1, targets: vec![], paying: vec![] }).unwrap();
    assert_eq!(g.players[0].pool, Pool { colors: [0; 5], colorless: 1, restricted: vec![] });
    mana(&mut g, &[FOREST]);
    cast(&mut g, BEARS, "CG", &[]).unwrap();
    cite!(["106.4"]; assert_eq!(g.players[0].pool.total(), 0));
}

#[test]
fn r607_2d_the_chosen_type_is_the_one_chosen_for_the_permanent_whose_ability_it_is() {
    let mut g = game();
    let goblins = play_cavern(&mut g, "Goblin");
    let card = g.put(0, CAVERN, Zone::Graveyard);
    mana(&mut g, &[FOREST, FOREST]);
    let restore = g.put(0, RESTORE, Zone::Hand);
    g.act(0, crate::support::paid("GG",Move::Cast { card: restore, targets: vec![vec![Target::Object(card)]] })).unwrap();
    pass_both(&mut g);
    g.act(0, Move::Choose(Answer::Value(creature_type("Elf")))).unwrap();
    let elves = *g.battlefield.last().unwrap();
    tap_for(&mut g, goblins, RESTRICTED, Color::Green);
    tap_for(&mut g, elves, RESTRICTED, Color::Red);
    // a green Goblin's mana and a red Elf's: neither pays for the other's spell
    cite!(["607.2d", "106.6"]; refused("that mana can't be spent on that: its text restricts how it can be spent (106.6)"): cast(&mut g, ELVES, "", &[0]));
    cite!(["607.2d", "106.6"]; refused("that mana can't be spent on that: its text restricts how it can be spent (106.6)"): cast(&mut g, RAIDERS, "", &[1]));
    assert_eq!(g.players[0].pool.restricted.len(), 2);
}

#[test]
fn r106_4_restricted_mana_empties_from_the_pool_like_any_other() {
    let mut g = game();
    let cavern = play_cavern(&mut g, "Goblin");
    tap_for(&mut g, cavern, RESTRICTED, Color::Red);
    assert_eq!(g.players[0].pool.restricted.len(), 1);
    pass_both(&mut g);
    assert_ne!(g.part, ir::Part::PrecombatMain);
    cite!(["106.4"]; assert_eq!(g.players[0].pool, Pool::default()));
}

#[test]
fn r106_6_ability_only_mana_pays_activations_but_not_spells() {
    let hawker = CardDef::new("Omen Hawker", "{U}", "Creature — Octopus Advisor", "{T}: Add {C}{U}. Spend this mana only to activate abilities.", Some((1, 1))).unwrap();
    check(&hawker).unwrap();
    let activated = CardDef::new("Receiver", "{C}{U}", "Artifact", "{C}{U}: You gain 2 life.", None).unwrap();
    let mut g=Game::new(vec![hawker,activated],2,20);
    let source=g.put(0,0,Zone::Battlefield);let receiver=g.put(0,1,Zone::Battlefield);let card=g.put(0,1,Zone::Hand);
    g.act(0,Move::Activate{source,ability:0,targets:vec![],paying:vec![]}).unwrap();
    cite!(["106.6"]; refused("that mana can't be spent on that: its text restricts how it can be spent (106.6)"): g.act(0,Move::Paying(selected(&g,"",&[0,1]),Box::new(Move::Cast{card,targets:vec![]}))));
    g.act(0,Move::Paying(selected(&g,"",&[0,1]),Box::new(Move::Activate{source:receiver,ability:0,targets:vec![],paying:vec![]}))).unwrap();
    pass_both(&mut g);cite!(["106.6"]; assert_eq!(g.players[0].life,22));
}

#[test]
fn r106_6_restricted_mana_uses_the_colorless_eldrazi_spell_or_ability_source() {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let temple=CardDef::from_face(faces.iter().find(|f|f.face_name=="Eldrazi Temple").unwrap()).unwrap();check(&temple).unwrap();
    let creature=|name,cost,kind,text| CardDef::new(name,cost,kind,text,Some((2,2))).unwrap();
    let defs=vec![temple,
        creature("Empty Eldrazi","{2}","Creature — Eldrazi","{2}: You gain 1 life."),
        creature("Green Eldrazi","{1}{G}","Creature — Eldrazi","{2}: You gain 1 life."),
        creature("Artifact","{2}","Artifact Creature","{2}: You gain 1 life."),
        creature("Devoid Eldrazi","{1}{G}","Creature — Eldrazi","Devoid\n{2}: You gain 1 life."),
    ];
    for (index,ability,allowed) in [(1,false,true),(2,false,false),(3,false,false),(1,true,true),(2,true,false),(3,true,false),(4,true,true)] {
        let mut g=Game::new(defs.clone(),2,20);let land=g.put(0,0,Zone::Battlefield);
        let object=g.put(0,index,if ability {Zone::Battlefield}else{Zone::Hand});
        g.act(0,Move::Activate{source:land,ability:1,targets:vec![],paying:vec![]}).unwrap();
        let action=if ability {Move::Activate{source:object,ability:if index==4 {1}else{0},targets:vec![],paying:vec![]}}else{Move::Cast{card:object,targets:vec![]}};
        // Give the colored Eldrazi cast its unrelated G requirement, so only the
        // Temple restriction is invalid. Its ability has a wholly generic cost.
        let with = if index==2 && !ability {
            g.players[0].pool.colors[4]=1;
            selected(&g,"G",&[0])
        } else { selected(&g,"",&[0,1]) };
        let action=Move::Paying(with,Box::new(action));
        if allowed {
            cite!(["106.6"]; case: format!("card {index}, ability={ability}"); ok: g.act(0,action));
            assert_eq!(g.players[0].pool.total(),0);
            assert_eq!(g.stack.len(),1);
            pass_both(&mut g);
            if ability { assert_eq!(g.players[0].life,21); }
            else { assert!(g.battlefield.iter().any(|o|g.obj(*o).card==Some(index))); }
        } else {
            cite!(["106.6"]; case: format!("card {index}, ability={ability}"); refused("that mana can't be spent on that: its text restricts how it can be spent"): act_unchanged(&mut g,action));
        }
    }
    // Explicit Temple mana pays for a colorless Eldrazi, and refuses an artifact.
    let mut g=Game::new(defs,2,20);let temple=g.put(0,0,Zone::Battlefield);let yes=g.put(0,1,Zone::Hand);let no=g.put(0,3,Zone::Hand);
    let plan=vec![Step { plan: vec![], x: None, life: vec![], with: None, source:temple, ability:1, paying:vec![], adding:vec![], chose:vec![], cost_branches:None }];
    let mut production=g.clone();
    production.act(0,Move::Activate{source:temple,ability:1,targets:vec![],paying:vec![]}).unwrap();
    let with=selected(&production,"",&[0,1]);
    cite!(["106.6"]; refused("that mana can't be spent on that: its text restricts how it can be spent"): act_unchanged(&mut g,Move::Paying(with.clone(),Box::new(Move::WithMana(plan.clone(),Box::new(Move::Cast{card:no,targets:vec![]}))))));
    g.act(0,Move::Paying(with,Box::new(Move::WithMana(plan,Box::new(Move::Cast{card:yes,targets:vec![]}))))).unwrap();
    let mut g=Game::new(defs2(),2,20);let temple=g.put(0,0,Zone::Battlefield);let forest=g.put(0,1,Zone::Battlefield);let devoid=g.put(0,2,Zone::Hand);
    let plan=vec![Step { plan: vec![], x: None, life: vec![], with: None, source:temple, ability:1, paying:vec![], adding:vec![], chose:vec![], cost_branches:None },Step { plan: vec![], x: None, life: vec![], with: None, source:forest, ability:0, paying:vec![], adding:vec![], chose:vec![], cost_branches:None }];
    let mut production=g.clone();
    production.act(0,Move::Activate{source:temple,ability:1,targets:vec![],paying:vec![]}).unwrap();
    let with=selected(&production,"G",&[0,1]);
    g.act(0,Move::Paying(with,Box::new(Move::WithMana(plan,Box::new(Move::Cast{card:devoid,targets:vec![]}))))).unwrap();
    assert_eq!(g.obj(*g.stack.last().unwrap()).chars.name,"Devoid Eldrazi");
    cite!(["702.114a"]; assert!(g.seen(*g.stack.last().unwrap(),None).colors.is_empty(),"a devoid spell is colorless on the stack"));
}
/// Eldrazi Temple, a Forest and a {2}{G} devoid Eldrazi: the Temple's {C}{C} pays the generic part.
fn defs2() -> Vec<CardDef> {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let temple=CardDef::from_face(faces.iter().find(|f|f.face_name=="Eldrazi Temple").unwrap()).unwrap();
    vec![temple,
        CardDef::new("Forest","","Basic Land — Forest","",None).unwrap(),
        CardDef::new("Devoid Eldrazi","{2}{G}","Creature — Eldrazi","Devoid",Some((3,3))).unwrap(),
    ]
}

#[test]
fn r106_6_production_evidence_retains_oracle_terms_and_chosen_type_after_source_leaves() {
    // CR 106.6/607.2d: the mana keeps the chosen-type condition and countering
    // rider; CR 400.7: the returning card is a different incarnation.
    let mut g = game();
    let cavern = play_cavern(&mut g, "Goblin");
    tap_for(&mut g, cavern, RESTRICTED, Color::Red);
    let proof = g.players[0].pool.restricted[0].provenance.clone().expect("authenticated production");
    let cold = proof.as_ref().clone();
    // First evidence request happens after the producer leaves (400.7).
    g.move_object(cavern, Zone::Graveyard);
    let projection = proof.audit().unwrap().clone();
    assert_eq!(proof.as_ref(), &cold, "materializing evidence cannot change pool equality");
    assert_eq!(projection["chosen"]["creature_type"], "Goblin");
    assert_eq!(projection["ability"]["paragraph"], 3);
    assert_eq!(projection["ability"]["source"], cavern.0);
    let terms = projection["restriction"].as_str().unwrap();
    assert!(terms.contains("chosen type") && terms.contains("can't be countered"));
    cite!(["106.6", "607.2d"]; assert_eq!(g.players[0].pool.restricted[0].provenance.as_ref(), Some(&proof)));
    let raider = g.put(0, RAIDERS, Zone::Hand);
    g.act(0, Move::Paying(selected(&g,"",&[0]),Box::new(Move::Cast { card: raider, targets: vec![] }))).unwrap();
    assert!(g.players[0].pool.restricted.is_empty());
    assert!(g.obj(*g.stack.last().unwrap()).uncounterable);
}

#[track_caller]
fn act_unchanged(g: &mut Game, action: Move) -> Result<(), String> {
    let before=format!("{g:?}");
    let result=g.act(0,action);
    assert_eq!(format!("{g:?}"),before);
    result
}
