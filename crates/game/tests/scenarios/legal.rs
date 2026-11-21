//! Explicit actions are accepted or refused according to their rules and supplied costs.

use crate::support::{cite, paid};
use game::card::CardDef;
use game::legal::{Asked, Step};
use game::play::check;
use game::{Game, Move, ObjId, Target, Zone};

const FOREST: usize = 0;
const MOUNTAIN: usize = 1;
const SWAMP: usize = 2;
const ISLAND: usize = 3;
const BEARS: usize = 4;
const BOLT: usize = 5;
const ELVES: usize = 6;
const SKULL: usize = 7;
const BLOOD: usize = 8;
const ZOMBIES: usize = 9;
const GREED: usize = 12;
const RUSALKA: usize = 13;
const SIGNET: usize = 15;
const ALTAR: usize = 16;
const BRUTE: usize = 17;
const BLAST: usize = 18;
const ISAMARU: usize = 19;
const PLAINS: usize = 20;
const OASIS: usize = 21;
const BIRDS: usize = 22;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        card("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None),
        card("Swamp", "", "Basic Land — Swamp", "({T}: Add {B}.)", None),
        card("Island", "", "Basic Land — Island", "({T}: Add {U}.)", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Lightning Bolt", "{R}", "Instant", "Lightning Bolt deals 3 damage to any target.", None),
        card("Llanowar Elves", "{G}", "Creature — Elf Druid", "{T}: Add {G}.", Some((1, 1))),
        card("Skull of Ramos", "{3}", "Artifact", "{T}: Add {B}.\nSacrifice this artifact: Add {B}.", None),
        card("Dragon Blood", "{3}", "Artifact", "{3}, {T}: Put a +1/+1 counter on target creature.", None),
        card("Scathe Zombies", "{2}{B}", "Creature — Zombie", "", Some((2, 2))),
        card("Elvish Visionary", "{1}{G}", "Creature — Elf Shaman", "When this creature enters, draw a card.", Some((1, 1))),
        card(
            "Sun-Crowned Hunters",
            "{4}{R}{R}",
            "Creature — Dinosaur",
            "Enrage — Whenever this creature is dealt damage, it deals 3 damage to target opponent or planeswalker.",
            Some((5, 4)),
        ),
        card("Greed", "{3}{B}", "Enchantment", "{B}, Pay 2 life: Draw a card.", None),
        card(
            "Drowned Rusalka",
            "{U}",
            "Creature — Spirit",
            "{U}, Sacrifice a creature: Discard a card, then draw a card.",
            Some((1, 1)),
        ),
        card("Defiant Elf", "{G}", "Creature — Elf", "Trample", Some((1, 1))),
        card("Dimir Signet", "{2}", "Artifact", "{1}, {T}: Add {U}{B}.", None),
        card("Ashnod's Altar", "{3}", "Artifact", "Sacrifice a creature: Add {C}{C}.", None),
        card(
            "Boggart Brute",
            "{2}{R}",
            "Creature — Goblin Warrior",
            "Menace (This creature can't be blocked except by two or more creatures.)",
            Some((3, 2)),
        ),
        card(
            "Urza's Ruinous Blast",
            "{4}{W}",
            "Legendary Sorcery",
            "(You may cast a legendary sorcery only if you control a legendary creature or planeswalker.)\nExile all nonland permanents that aren't legendary.",
            None,
        ),
        card("Isamaru, Hound of Konda", "{W}", "Legendary Creature — Dog", "", Some((2, 2))),
        card("Plains", "", "Basic Land — Plains", "({T}: Add {W}.)", None),
        card("Shivan Oasis", "", "Land", "This land enters tapped.\n{T}: Add {R} or {G}.", None),
        card("Birds of Paradise", "{G}", "Creature — Bird", "Flying\n{T}: Add one mana of any color.", Some((0, 1))),
        card(
            "Cavern of Souls",
            "",
            "Land",
            "As this land enters, choose a creature type.\n{T}: Add {C}.\n{T}: Add one mana of any color. Spend this mana only to cast a creature spell of the chosen type, and that spell can't be countered.",
            None,
        ),
    ]
}

fn game() -> Game {
    Game::new(defs(), 2, 20)
}

/// The first ability of a permanent, its cost asking for no choice: a step of a plan.
fn tap(source: ObjId) -> Step {
    Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source, ability: 0, paying: vec![], adding: vec![] }
}

/// The move that is one step of a plan: a mana ability activated, saying which mana it adds
/// where its text offers a choice.
fn step(s: &Step) -> Move {
    let activate = Move::Activate { source: s.source, ability: s.ability, targets: vec![], paying: s.paying.clone() };
    let action=if s.adding.is_empty() { activate } else { Move::Adding(s.adding.clone(), Box::new(activate)) };
    if let Some(with)=&s.with { Move::Paying(with.clone(),Box::new(action)) } else {action}
}

fn follow(g: &mut Game, player: usize, plan: &[Step]) {
    for s in plan {
        g.act(player, step(s)).unwrap();
    }
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r117_1_a_player_with_priority_may_play_a_land_cast_a_spell_activate_an_ability_or_pass() {
    let mut g=game();
    let forest=g.put(0,FOREST,Zone::Hand); let bears=g.put(0,BEARS,Zone::Hand);
    let land=g.put(0,FOREST,Zone::Battlefield); let elves=g.put(0,ELVES,Zone::Battlefield);
    cite!(["305.1"]; ok: g.clone().act(0,Move::PlayLand(forest)));
    cite!(["601.2g"]; ok: g.clone().act(0,paid("GG",Move::WithMana(vec![tap(land),tap(elves)],Box::new(Move::Cast{card:bears,targets:vec![]})))));
    cite!(["117.1b"]; ok: g.clone().act(0,step(&tap(land))));
    cite!(["117.3d"]; ok: g.act(0,Move::Pass));
    let other=g.put(1,FOREST,Zone::Hand);
    cite!(["305.1"]; refused("a land is played in a main phase of your own turn with the stack empty"): act_unchanged(&mut g,1,Move::PlayLand(other)));
    g.act(1,Move::Pass).unwrap();
    cite!(["305.1"]; refused("a land is played in a main phase of your own turn with the stack empty"): act_unchanged(&mut g,0,Move::PlayLand(forest)));
}

#[test]
fn r601_2c_an_ability_is_activated_only_with_a_legal_target_for_each_target_it_requires() {
    let mut g=game(); let blood=g.put(0,BLOOD,Zone::Battlefield);
    let plan: Vec<_>=(0..3).map(|_|tap(g.put(0,SWAMP,Zone::Battlefield))).collect();
    let activate=|targets|paid("BBB",Move::WithMana(plan.clone(),Box::new(Move::Activate{source:blood,ability:0,targets,paying:vec![]})));
    let zombies=g.put(1,ZOMBIES,Zone::Battlefield);
    cite!(["602.2b","601.2c"]; refused("wrong number of targets: 0 chosen, it takes 1"): act_unchanged(&mut g,0,activate(vec![vec![]])));
    cite!(["602.2b","601.2c"]; refused("an illegal target, that player"): act_unchanged(&mut g,0,activate(vec![vec![Target::Player(1)]])));
    cite!(["602.2b","601.2g"]; ok: g.act(0,activate(vec![vec![Target::Object(zombies)]])));
    for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
    assert_eq!(g.obj(zombies).counters["+1/+1"],1);
}

#[test]
fn r602_2b_a_creature_chosen_as_a_target_can_then_pay_for_mana() {
    let mut g = game();
    let blood = g.put(0, BLOOD, Zone::Battlefield);
    let swamp = g.put(0, SWAMP, Zone::Battlefield);
    let altar = g.put(0, ALTAR, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let plan=vec![tap(swamp),Step { plan: vec![], x: None, life: vec![], with: None,paying:vec![vec![bears]],..tap(altar)}];
    g.act(0, paid("CCB",Move::WithMana(plan.clone(), Box::new(Move::Activate { source: blood, ability: 0, targets: vec![vec![Target::Object(bears)]], paying: vec![] })))).unwrap();
    cite!(["602.2b", "601.2g"]; assert_eq!(g.obj(bears).zone, Zone::Gone));
    assert!(g.obj(blood).tapped);
    assert_eq!(g.stack.len(), 1);
    let ability = g.stack[0];
    for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); }
    assert!(g.stack.is_empty(), "the ability has no legal target on resolution");
    cite!(["608.2b"]; assert!(g.log.iter().any(|e| matches!(e, game::Event::Fizzled { spell } if *spell == ability))));
}

#[test]
fn r601_2g_a_cost_is_payable_with_the_mana_its_player_could_produce() {
    let mut g=game(); let bolt=g.put(0,BOLT,Zone::Hand); let forest=g.put(0,FOREST,Zone::Battlefield);
    let cast=Move::Cast{card:bolt,targets:vec![vec![Target::Player(1)]]};
    cite!(["601.2h"]; refused("that mana is not in your mana pool"): act_unchanged(&mut g,0,paid("R",Move::WithMana(vec![tap(forest)],Box::new(cast.clone())))));
    let mountain=g.put(0,MOUNTAIN,Zone::Battlefield);
    cite!(["601.2g"]; ok: g.act(0,paid("R",Move::WithMana(vec![tap(mountain)],Box::new(cast)))));
    assert!(!g.obj(forest).tapped);
    let mut g=game(); let zombies=g.put(0,ZOMBIES,Zone::Hand); let skull=g.put(0,SKULL,Zone::Battlefield);
    let cast=Move::Cast{card:zombies,targets:vec![]};
    let sacrifice=Step { plan: vec![], x: None, life: vec![], with: None,ability:1,..tap(skull)};
    cite!(["601.2h"]; refused("the supplied mana does not pay that cost"): act_unchanged(&mut g,0,paid("BB",Move::WithMana(vec![tap(skull),sacrifice.clone()],Box::new(cast.clone())))));
    let swamp=g.put(0,SWAMP,Zone::Battlefield);
    cite!(["601.2g","601.2h"]; ok: g.act(0,paid("BBB",Move::WithMana(vec![tap(skull),tap(swamp),sacrifice],Box::new(cast)))));
    assert_eq!(g.players[0].pool.total(),0); assert!(!g.battlefield.contains(&skull));
}

#[test]
fn r601_2g_a_mana_ability_that_costs_mana_is_paid_with_mana_produced_before_it() {
    let mut g=game(); let zombies=g.put(0,ZOMBIES,Zone::Hand);
    let first=g.put(0,FOREST,Zone::Battlefield); let signet=g.put(0,SIGNET,Zone::Battlefield);
    let cast=Move::Cast{card:zombies,targets:vec![]};
    let signet_step=Step { plan: vec![], x: None, life: vec![],with:Some(crate::support::mana_pool("G")),..tap(signet)};
    cite!(["601.2h"]; refused("the supplied mana does not pay that cost"): act_unchanged(&mut g,0,paid("UB",Move::WithMana(vec![tap(first),signet_step.clone()],Box::new(cast.clone())))));
    let second=g.put(0,FOREST,Zone::Battlefield);
    cite!(["601.2g"]; ok: g.act(0,paid("GUB",Move::WithMana(vec![tap(first),signet_step,tap(second)],Box::new(cast)))));
    assert_eq!(g.players[0].pool.total(),0);
    let mut g=game(); let forest=g.put(0,FOREST,Zone::Battlefield); let signet=g.put(0,SIGNET,Zone::Battlefield);
    cite!(["602.2b","601.2g"]; ok: g.act(0,Move::WithMana(vec![tap(forest)],Box::new(step(&Step { plan: vec![], x: None, life: vec![],with:Some(crate::support::mana_pool("G")),..tap(signet)})))));
    assert_eq!(g.players[0].pool.colors,[0,1,1,0,0]);
}

#[test]
fn r205_4e_a_legendary_sorcery_is_cast_only_by_a_player_who_controls_a_legendary_creature() {
    let mut g=game(); let blast=g.put(0,BLAST,Zone::Hand);
    let plan: Vec<_>=(0..5).map(|_|tap(g.put(0,PLAINS,Zone::Battlefield))).collect();
    g.put(1,ISAMARU,Zone::Battlefield);
    let cast=paid("WWWWW",Move::WithMana(plan,Box::new(Move::Cast{card:blast,targets:vec![]})));
    cite!(["205.4e"]; refused("a legendary instant or sorcery is cast only by a player who controls a legendary creature or planeswalker (205.4e)"): g.act(0,cast.clone()));
    g.put(0,ISAMARU,Zone::Battlefield);
    cite!(["205.4e"]; ok: g.act(0,cast));
}

#[test]
fn r601_2g_a_choice_of_mana_is_made_so_that_the_cost_is_paid() {
    use ir::Color::{Black,Green,Red}; use ir::mana::ManaSymbol::Color;
    let mut g=game(); let bolt=g.put(0,BOLT,Zone::Hand); let oasis=g.put(0,OASIS,Zone::Battlefield);
    let red=Step { plan: vec![], x: None, life: vec![], with: None,ability:1,adding:vec![vec![Color(Red)]],..tap(oasis)};
    assert!(!g.obj(oasis).tapped,"initialized untapped premise, not an entry event");
    follow(&mut g,0,&[red]); g.act(0,paid("R",Move::Cast{card:bolt,targets:vec![vec![Target::Player(1)]]})).unwrap();
    let mut g=game(); let zombies=g.put(0,ZOMBIES,Zone::Hand);
    let forest=g.put(0,FOREST,Zone::Battlefield); let oasis=g.put(0,OASIS,Zone::Battlefield);
    let green=Step { plan: vec![], x: None, life: vec![], with: None,ability:1,adding:vec![vec![Color(Green)]],..tap(oasis)};
    let cast=Move::Cast{card:zombies,targets:vec![]};
    let birds=g.put(0,BIRDS,Zone::Battlefield);
    let bird_green=Step{ability:1,adding:vec![vec![Color(Green)]],..tap(birds)};
    cite!(["601.2h"]; refused("the supplied mana does not pay that cost"): act_unchanged(&mut g,0,paid("GGG",Move::WithMana(vec![tap(forest),green.clone(),bird_green],Box::new(cast.clone())))));
    let black=Step { plan: vec![], x: None, life: vec![], with: None,ability:1,adding:vec![vec![Color(Black)]],..tap(birds)};
    cite!(["601.2g"]; ok: g.act(0,paid("GGB",Move::WithMana(vec![tap(forest),green.clone(),black],Box::new(cast)))));
    assert_eq!(g.players[0].pool.total(),0);
    cite!(["602.2b"]; refused("Shivan Oasis can't pay {T} as it is"): act_unchanged(&mut g,0,step(&green)));
}

#[test]
fn r509_1b_a_creature_that_only_several_can_block_is_listed_with_how_many_it_takes() {
    // "This creature can't be blocked except by two or more creatures." (702.111b): one
    // creature can't block it, two together can
    let mut g = game();
    let brute = g.put(0, BRUTE, Zone::Battlefield);
    let attacker = g.put(0, BEARS, Zone::Battlefield);
    let first = g.put(1, BEARS, Zone::Battlefield);
    let blockers = |g: &mut Game| match g.asked() {
        Some((1, Asked::Blockers { able, together, .. })) => (able, together),
        other => panic!("player 1 declares blockers: {other:?}"),
    };
    let to_blockers = |g: &mut Game| {
        while g.decision.is_none() {
            g.act(g.priority, Move::Pass).unwrap();
        }
        g.act(0, Move::Attack(vec![(brute, Target::Player(1)), (attacker, Target::Player(1))])).unwrap();
        g.act(0, Move::Pass).unwrap();
        g.act(1, Move::Pass).unwrap();
    };
    let mut alone = g.clone();
    to_blockers(&mut alone);
    cite!(["702.111b", "509.1b"]; assert_eq!(blockers(&mut alone), (vec![(first, vec![attacker])], vec![]), "one creature blocks the Bears only"));
    cite!(["702.111b", "509.1b"]; refused("Grizzly Bears can't block Boggart Brute: Boggart Brute has menace (702.111b, 509.1b)"): alone.act(1, Move::Block(vec![(first, brute)])));
    let second = g.put(1, ZOMBIES, Zone::Battlefield);
    to_blockers(&mut g);
    let both = vec![brute, attacker];
    cite!(["702.111b", "509.1b"]; assert_eq!(blockers(&mut g), (vec![(first, both.clone()), (second, both)], vec![(brute, 2)])));
    cite!(["702.111b", "509.1b"]; ok: g.act(1, Move::Block(vec![(first, brute), (second, brute)])));
}

#[test]
fn r602_2_activation_requires_valid_sacrifice_and_life_payment() {
    let mut g=game(); let rusalka=g.put(0,RUSALKA,Zone::Battlefield);
    let bears=g.put(0,BEARS,Zone::Battlefield); let opposing=g.put(1,BEARS,Zone::Battlefield);
    let activate=|creature|Move::Activate{source:rusalka,ability:0,targets:vec![],paying:vec![vec![creature]]};
    cite!(["118.3a"]; refused("that mana is not in your mana pool"): act_unchanged(&mut g,0,paid("U",activate(bears))));
    let island=g.put(0,ISLAND,Zone::Battlefield);
    cite!(["602.2b","601.2h"]; refused("Grizzly Bears is not what that cost asks for or cannot pay it"): act_unchanged(&mut g,0,paid("U",Move::WithMana(vec![tap(island)],Box::new(activate(opposing))))));
    for creature in [rusalka,bears] {
        let mut paid_state=g.clone();
        cite!(["602.2b","601.2h"]; ok: paid_state.act(0,paid("U",Move::WithMana(vec![tap(island)],Box::new(activate(creature))))));
        assert!(!paid_state.battlefield.contains(&creature) && paid_state.obj(island).tapped && paid_state.players[0].pool.total()==0 && paid_state.stack.len()==1);
    }
    let mut g=game(); let greed=g.put(0,GREED,Zone::Battlefield); let swamp=g.put(0,SWAMP,Zone::Battlefield);
    let activate=paid("B",Move::WithMana(vec![tap(swamp)],Box::new(Move::Activate{source:greed,ability:0,targets:vec![],paying:vec![]})));
    let mut paid_state=g.clone();
    cite!(["119.4"]; ok: paid_state.act(0,activate.clone()));
    assert_eq!(paid_state.players[0].life,18); assert!(paid_state.obj(swamp).tapped && paid_state.players[0].pool.total()==0);
    g.players[0].life=1;
    cite!(["119.4"]; refused("you can't pay 2 life with a life total of 1"): act_unchanged(&mut g,0,activate));
}

#[track_caller]
fn act_unchanged(g:&mut Game,player:usize,action:Move)->Result<(),String>{
 let before=format!("{g:?}");let result=g.act(player,action);assert_eq!(format!("{g:?}"),before);result
}
