//! A fixed selection of cards the engine runs. Fixture put never performs entry; native cards use pinned Oracle.
use crate::support::{cite, mana_pool, oracle, paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::{Color, Part};

fn setup(name: &str, players: usize) -> Game {
    let mut g = Game::new(
        vec![
            oracle(name),
            CardDef::new("Land", "", "Basic Land — Forest", "", None).unwrap(),
            CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
            CardDef::new("Wall", "{0}", "Creature — Wall", "", Some((1, 4))).unwrap(),
            CardDef::new("Gift", "{0}", "Instant", "You gain 1 life.", None).unwrap(),
            CardDef::new("Boost", "{0}", "Instant", "Target creature gets +3/+0 until end of turn.", None).unwrap(),
            CardDef::new("Destroy", "{0}", "Instant", "Destroy target creature.", None).unwrap(),
            CardDef::new("Shield", "{0}", "Instant", "Target creature gains indestructible until end of turn.", None)
                .unwrap(),
            CardDef::new("Vehicle", "{0}", "Artifact — Vehicle", "Crew 1", Some((3, 3))).unwrap(),
            CardDef::new("Rock", "{0}", "Artifact", "", None).unwrap(),
            CardDef::new(
                "Raise",
                "{0}",
                "Instant",
                "Return target creature card from your graveyard to the battlefield.",
                None,
            )
            .unwrap(),
            oracle("Island"),
            oracle("Mountain"),
            oracle("Plains"),
        ],
        players,
        20,
    );
    for p in 0..players {
        for _ in 0..20 {
            g.put(p, 1, Zone::Library);
        }
    }
    g
}
fn resolve(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn cast(g: &mut Game, def: usize, targets: Vec<Vec<Target>>, spending: &str) -> ObjId {
    assert_eq!(g.priority, 0);
    let card = g.put(0, def, Zone::Hand);
    g.act(0, paid(spending, Move::Cast { card, targets })).unwrap();
    card
}
fn step(g: &mut Game) {
    let mv = match &g.decision {
        Some(Decision::Attackers { .. }) => Move::Attack(vec![]),
        Some(Decision::Blockers { .. }) => Move::Block(vec![]),
        Some(other) => panic!("unexpected {other:?}"),
        None => Move::Pass,
    };
    let p = g.asked().unwrap().0;
    g.act(p, mv).unwrap();
}
fn until(g: &mut Game, turn: u32, part: Part) {
    for _ in 0..300 {
        if g.turn == turn && g.part == part {
            return;
        }
        step(g);
    }
    panic!("boundary not reached");
}
fn ability(
    g: &mut Game,
    source: ObjId,
    index: usize,
    targets: Vec<Vec<Target>>,
    paying: Vec<Vec<ObjId>>,
    spending: &str,
) -> Result<(), String> {
    g.act(0, paid(spending, Move::Activate { source, ability: index, targets, paying }))
}
fn mana(g: &mut Game, generic: u32, colors: &[(Color, u32)]) {
    g.players[0].pool.colorless = generic;
    for (c, n) in colors {
        g.players[0].pool.colors[*c as usize] = *n;
    }
}

// Rules: 107.3a, 601.2b, 121.2, 121.3, 704.5b
// Chosen X controls the number drawn; zero does not attempt an empty-library draw.
#[test]
fn r121_mind_spring_zero_three_and_overdraw() {
    for (x, count) in [(0, 0), (3, 3), (3, 2)] {
        let mut g = setup("Mind Spring", 2);
        g.players[0].library.clear();
        for _ in 0..count {
            g.put(0, 1, Zone::Library);
        }
        mana(&mut g, x, &[(Color::Blue, 2)]);
        let card = g.put(0, 0, Zone::Hand);
        g.act(0, paid(&format!("{}UU", "C".repeat(x as usize)), Move::WithX(x, Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
        resolve(&mut g);
        cite!(["121.2", "121.3"]; assert_eq!(g.players[0].hand.len(), count));
        cite!(["704.5b"]; assert_eq!(g.players[0].lost, x as usize > count));
        cite!(["601.2f", "601.2h"]; assert_eq!(g.players[0].pool.colors[Color::Blue as usize], 0));
    }
}

// Rules: 109.5, 113.6, 613.4c, 611.3a
// Each Imperiosaur excludes itself but buffs the other; neither enemy nor Bear qualifies.
#[test]
fn r613_imperiosaur_controller_other_and_source_departure() {
    let mut g = setup("Regal Imperiosaur", 2);
    let a = g.put(0, 0, Zone::Battlefield);
    let b = g.put(0, 0, Zone::Battlefield);
    let enemy = g.put(1, 0, Zone::Battlefield);
    let bear = g.put(0, 2, Zone::Battlefield);
    cite!(["613.4c"]; assert_eq!((g.power(a), g.toughness(a), g.power(b)), (Some(6), Some(5), Some(6))));
    cite!(["109.5"]; assert_eq!((g.power(enemy), g.power(bear)), (Some(5), Some(2))));
    cast(&mut g, 6, vec![vec![Target::Object(b)]], "");
    resolve(&mut g);
    cite!(["611.3a", "113.6"]; assert_eq!((g.power(a), g.toughness(a)), (Some(5), Some(4))));
}

// Rules: 603.2, 109.5, 701.26b, 608.2h
// End-step trigger waits on stack and uses the controller's current nonland set.
#[test]
fn r603_plan_end_step_scope_and_resolution_set() {
    let mut g = setup("Unstoppable Plan", 2);
    let plan = g.put(0, 0, Zone::Battlefield);
    let own = g.put(0, 2, Zone::Battlefield);
    let land = g.put(0, 1, Zone::Battlefield);
    let enemy = g.put(1, 2, Zone::Battlefield);
    for o in [plan, own, land, enemy] {
        g.set_tapped(o, true);
    }
    until(&mut g, 1, Part::End);
    cite!(["603.2"]; assert_eq!(g.stack.len(), 1));
    cite!(["603.2"]; assert!(g.obj(own).tapped));
    // Explicit fixture arriving before resolution, not evidence of an entry instruction.
    let late = g.put(0, 9, Zone::Battlefield);
    g.set_tapped(late, true);
    resolve(&mut g);
    cite!(["701.26b", "608.2h"]; assert!([plan, own, late].iter().all(|o| !g.obj(*o).tapped)));
    cite!(["109.5"]; assert!(g.obj(land).tapped && g.obj(enemy).tapped));
    until(&mut g, 2, Part::End);
    cite!(["603.2"]; assert!(g.stack.is_empty()));
}

// Rules: 702.6a, 702.6b, 613.4c, 702.17b, 702.20b, 301.5d
// The native equip transfers all bonuses, and only one's own creature is a legal target.
#[test]
fn r702_short_bow_transfer_and_target_scope() {
    let mut g = setup("Short Bow", 2);
    let bow = g.put(0, 0, Zone::Battlefield);
    let a = g.put(0, 2, Zone::Battlefield);
    let b = g.put(0, 3, Zone::Battlefield);
    let enemy = g.put(1, 2, Zone::Battlefield);
    mana(&mut g, 1, &[]);
    let equip = 2; // Printed rows precede the derived equip ability.
    cite!(["702.6a"]; refused("target"): ability(&mut g, bow, equip, vec![vec![Target::Object(enemy)]], vec![], "C"));
    ability(&mut g, bow, equip, vec![vec![Target::Object(a)]], vec![], "C").unwrap();
    resolve(&mut g);
    cite!(["613.4c"]; assert_eq!((g.power(a), g.toughness(a)), (Some(3), Some(3))));
    cite!(["702.17b", "702.20b"]; assert!(g.has(a,"Reach") && g.has(a,"Vigilance")));
    mana(&mut g, 1, &[]);
    ability(&mut g, bow, equip, vec![vec![Target::Object(b)]], vec![], "C").unwrap();
    resolve(&mut g);
    cite!(["301.5d"]; assert_eq!((g.power(a), g.toughness(a), g.power(b), g.toughness(b)), (Some(2),Some(2),Some(2),Some(5))));
    cite!(["301.5d"]; assert!(!g.has(a,"Reach") && !g.has(a,"Vigilance") && g.has(b,"Reach") && g.has(b,"Vigilance")));
}

// Rules: 602.2b, 701.21a, 603.6c, 118.12, 111.3
// Sacrifice is paid before resolution; the death trigger resolves before life gain.
#[test]
fn r603_haunch_sacrifice_optional_payment_and_dogs() {
    for pay in [false, true] {
        let mut g = setup("Krovod Haunch", 2);
        let food = g.put(0, 0, Zone::Battlefield);
        cite!(["205.3g"]; assert!(g.obj(food).chars.subtypes.contains(&ir::Subtype::from_name("Food").unwrap())));
        mana(&mut g, 3, &[(Color::White, 1)]);
        ability(&mut g, food, 1, vec![], vec![], "CC").unwrap();
        cite!(["602.2b", "701.21a"]; assert!(!g.battlefield.contains(&food)));
        cite!(["603.6c"]; assert_eq!(g.stack.len(), 2));
        resolve(&mut g);
        cite!(["118.12"]; assert!(matches!(g.decision, Some(Decision::Choose { player:0, choice:Choice::Pay{..} }))));
        g.act(0, Move::Choose(if pay { Answer::Pay { plan: vec![], with: Some(mana_pool("CW")), tapped: vec![] } } else { Answer::No })).unwrap();
        cite!(["118.3a", "118.12"]; assert_eq!(g.players[0].pool.total(), if pay { 0 } else { 2 }));
        let dogs: Vec<_> = g.battlefield.iter().copied().filter(|o| g.obj(*o).token()).collect();
        cite!(["118.12", "111.3"]; assert_eq!(dogs.len(), if pay {2} else {0}));
        for o in dogs {
            cite!(["111.3"]; assert_eq!((g.power(o),g.toughness(o),g.obj(o).chars.colors.clone(),g.obj(o).chars.subtypes.clone()),(Some(1),Some(1),vec![Color::White],vec![ir::Subtype::from_name("Dog").unwrap()])));
        }
        cite!(["603.3"]; assert_eq!(g.players[0].life,20));
        resolve(&mut g);
        cite!(["119.3"]; assert_eq!(g.players[0].life,23));
    }
}

// Rules: 603.6c, 701.62a, 701.40a, 708.2a, 701.40b
// Death, unlike fixture entry, produces the native trigger; two-card choice is private.
#[test]
fn r701_beastie_native_death_manifest_choice_and_face_up() {
    let mut g = setup("Bashful Beastie", 2);
    g.players[0].library.clear();
    let tail = g.put(0, 1, Zone::Library);
    let other = g.put(0, 4, Zone::Library);
    let top = g.put(0, 2, Zone::Library);
    let beast = g.put(0, 0, Zone::Battlefield);
    cast(&mut g, 6, vec![vec![Target::Object(beast)]], "");
    resolve(&mut g);
    resolve(&mut g);
    let Some(Decision::Choose { player: 0, choice: Choice::Objects { among, count: 1, .. } }) = g.decision.clone()
    else {
        panic!("{:?}", g.decision)
    };
    cite!(["701.62a"]; assert_eq!(among,vec![top,other]));
    cite!(["701.20e"]; assert!(g.sees(0,top) && !g.sees(1,top) && !g.sees(1,other)));
    g.act(0, Move::Choose(Answer::Objects(vec![top]))).unwrap();
    let down = *g.battlefield.last().unwrap();
    cite!(["701.40a", "708.2a"]; assert_eq!((g.power(down),g.toughness(down),g.obj(down).chars.name.as_str()),(Some(2),Some(2),"")));
    cite!(["701.62a"]; assert_eq!(g.players[0].library,vec![tail]));
    cite!(["701.62a"]; assert!(g.players[0].graveyard.iter().any(|o|g.obj(*o).chars.name=="Gift")));
    g.act(0, Move::TurnFaceUp(down, game::face::Way::Manifested)).unwrap();
    cite!(["701.40b", "400.7"]; assert_eq!(g.obj(down).chars.name,"Bear"));
}

// Rules: 701.62a, 609.3
// One card must be manifested; zero cards does nothing and does not draw/lose.
#[test]
fn r701_beastie_short_library() {
    for count in 0..=1 {
        let mut g = setup("Bashful Beastie", 2);
        g.players[0].library.clear();
        for _ in 0..count {
            g.put(0, 2, Zone::Library);
        }
        let beast = g.put(0, 0, Zone::Battlefield);
        cast(&mut g, 6, vec![vec![Target::Object(beast)]], "");
        resolve(&mut g);
        resolve(&mut g);
        if let Some(Decision::Choose { player: 0, choice: Choice::Objects { among, count: 1, .. } }) =
            g.decision.clone()
        {
            cite!(["701.62a"]; assert_eq!(among.len(),1));
            g.act(0, Move::Choose(Answer::Objects(among))).unwrap();
        }
        cite!(["701.62a"]; assert_eq!(g.battlefield.iter().filter(|o|g.face_down(**o)).count(),count));
        cite!(["609.3"]; assert!(!g.players[0].lost));
    }
}

// Rules: 702.179a, 704.5aa, 702.178a, 605.3b, 302.6
// Speed's SBA is separate from mana activation; max-speed gate and tap are consequential.
#[test]
fn r702_catalyzer_speed_gate_and_immediate_mana() {
    let mut g = setup("Endrider Catalyzer", 2);
    mana(&mut g, 1, &[(Color::Red, 1)]);
    cast(&mut g, 0, vec![], "CR");
    resolve(&mut g);
    let source = *g.battlefield.last().unwrap();
    cite!(["704.5aa"]; assert_eq!((g.players[0].speed,g.players[1].speed),(Some(1),None)));
    until(&mut g, 3, Part::PrecombatMain);
    g.players[0].speed = Some(4); // explicit speed-4 fixture, not evidence of increasing speed
    let count = g.stack.len();
    let index = 2; // Derived activation of the second printed Max speed row.
    ability(&mut g, source, index, vec![], vec![], "").unwrap();
    cite!(["605.3b"]; assert_eq!((g.players[0].pool.colors[Color::Red as usize],g.stack.len()),(2,count)));
    cite!(["602.2b"]; assert!(g.obj(source).tapped));
}

// Rules: 603.6a, 700.2, 701.25a, 701.20e
// Mode zero gains life before the private surveil choice; all three cards are partitioned.
#[test]
fn r701_familiar_life_then_private_surveil() {
    let mut g = setup("Fangkeeper's Familiar", 2);
    g.players[0].library.clear();
    let tail = g.put(0, 1, Zone::Library);
    let c = g.put(0, 2, Zone::Library);
    let b = g.put(0, 3, Zone::Library);
    let a = g.put(0, 4, Zone::Library);
    mana(&mut g, 1, &[(Color::Black, 1), (Color::Green, 1), (Color::Blue, 1)]);
    cast(&mut g, 0, vec![], "CBGU");
    resolve(&mut g);
    cite!(["700.2"]; assert!(matches!(g.decision,Some(Decision::Modes{player:0,..}))));
    g.act(0, Move::Modes(vec![0], Box::new(Move::ChooseTargets(vec![])))).unwrap();
    resolve(&mut g);
    cite!(["119.3"]; assert_eq!(g.players[0].life,23));
    let Some(Decision::Choose { player: 0, choice: Choice::Arrange { verb: ir::Verb::Surveil, cards } }) =
        g.decision.clone()
    else {
        panic!("{:?}", g.decision)
    };
    cite!(["701.25a"]; assert_eq!(cards,vec![a,b,c]));
    cite!(["701.20e"]; assert!(cards.iter().all(|o|g.sees(0,*o)&&!g.sees(1,*o))));
    g.act(0, Move::Choose(Answer::Arrange { top: vec![c, a], away: vec![b] })).unwrap();
    cite!(["701.25a"]; assert_eq!(g.players[0].library.iter().rev().map(|o|g.obj(*o).chars.name.as_str()).collect::<Vec<_>>(),vec!["Bear","Gift","Land"]));
    cite!(["701.25a"]; assert!(g.players[0].graveyard.iter().any(|o|g.obj(*o).chars.name=="Wall")));
    let _ = tail;
}

// Rules: 603.3d, 700.2, 701.8a, 608.2b
// Enchantment mode permits an opponent's enchantment but excludes a creature.
#[test]
fn r700_familiar_enchantment_mode_target_scope() {
    let mut g = setup("Fangkeeper's Familiar", 2);
    std::sync::Arc::make_mut(&mut g.defs).push(oracle("Unstoppable Plan"));
    let enchant = g.put(1, 14, Zone::Battlefield);
    let bear = g.put(1, 2, Zone::Battlefield);
    mana(&mut g, 1, &[(Color::Black, 1), (Color::Green, 1), (Color::Blue, 1)]);
    cast(&mut g, 0, vec![], "CBGU");
    resolve(&mut g);
    cite!(["603.3d", "700.2"]; refused("target"): g.act(0,Move::Modes(vec![1],Box::new(Move::ChooseTargets(vec![vec![Target::Object(bear)]])))));
    g.act(0, Move::Modes(vec![1], Box::new(Move::ChooseTargets(vec![vec![Target::Object(enchant)]])))).unwrap();
    resolve(&mut g);
    cite!(["701.8a"]; assert!(!g.battlefield.contains(&enchant) && g.battlefield.contains(&bear)));
}

// Rules: 714.3a, 714.2b, 111.3
// Casting, rather than fixture placement, adds lore and triggers the first chapter.
#[test]
fn r714_mice_entry_and_first_mouse() {
    let mut g = setup("Three Blind Mice", 2);
    mana(&mut g, 2, &[(Color::White, 1)]);
    cast(&mut g, 0, vec![], "CCW");
    resolve(&mut g);
    let saga = *g.battlefield.last().unwrap();
    cite!(["714.3a"]; assert_eq!(g.obj(saga).counters.get("lore"),Some(&1)));
    cite!(["714.2b"]; assert_eq!(g.stack.len(),1));
    resolve(&mut g);
    let mouse = *g.battlefield.last().unwrap();
    cite!(["111.3"]; assert_eq!((g.power(mouse),g.toughness(mouse),g.obj(mouse).chars.colors.clone(),g.obj(mouse).chars.subtypes.clone()),(Some(1),Some(1),vec![Color::White],vec![ir::Subtype::from_name("Mouse").unwrap()])));
}

// Rules: 701.20e, 608.2d, 609.3, 400.7
// Hybrid cost paid blue; only top four eligible identities may be chosen, optionally.
#[test]
fn r608_flamekin_private_selection_filter_and_short_library() {
    for decline in [false, true] {
        let mut g = setup("Eclipsed Flamekin", 2);
        g.players[0].library.clear();
        let tail = g.put(0, 11, Zone::Library);
        let excluded = g.put(0, 2, Zone::Library);
        let mountain = g.put(0, 12, Zone::Library);
        let island = g.put(0, 11, Zone::Library);
        let land = g.put(0, 1, Zone::Library);
        mana(&mut g, 1, &[(Color::Blue, 2)]);
        cast(&mut g, 0, vec![], "CUU");
        resolve(&mut g);
        resolve(&mut g);
        cite!(["701.20e"]; assert!(g.sees(0,island)&&!g.sees(1,island)&&!g.sees(0,tail)));
        // Optional reveal is a may decision, followed by a constrained object choice.
        if matches!(g.decision, Some(Decision::Choose { choice: Choice::May, .. })) {
            g.act(0, Move::Choose(if decline { Answer::No } else { Answer::Yes })).unwrap();
        }
        if !decline {
            let Some(Decision::Choose { player: 0, choice: Choice::Objects { among, .. } }) = g.decision.clone() else {
                panic!("{:?}", g.decision)
            };
            cite!(["608.2d"]; assert_eq!(among,vec![island,mountain]));
            cite!(["608.2d"]; refused("among"): g.act(0,Move::Choose(Answer::Objects(vec![excluded]))));
            g.act(0, Move::Choose(Answer::Objects(vec![island]))).unwrap();
        }
        cite!(["608.2c"]; assert_eq!(g.players[0].hand.len(),usize::from(!decline)));
        cite!(["608.2c"]; assert_eq!(g.players[0].library.len(),if decline {5}else{4}));
        cite!(["608.2c"]; assert_eq!(*g.players[0].library.last().unwrap(),tail));
        let _ = land;
    }
}
// Rules: 702.8a, 601.2b, 700.2, 701.5a
// Flash lets the native Familiar be cast over a creature spell; only that spell is countered.
#[test]
fn r702_familiar_flash_and_creature_counter_mode() {
    let mut g = setup("Fangkeeper's Familiar", 2);
    cast(&mut g, 2, vec![], "");
    let bear = g.stack[0]; // Spell incarnation, not the departed hand card.
    mana(&mut g, 1, &[(Color::Black, 1), (Color::Green, 1), (Color::Blue, 1)]);
    cite!(["702.8a"]; ok: {let card=g.put(0,0,Zone::Hand);g.act(0,paid("CBGU", Move::Cast{card,targets:vec![]}))});
    resolve(&mut g);
    g.act(0, Move::Modes(vec![2], Box::new(Move::ChooseTargets(vec![vec![Target::Object(bear)]])))).unwrap();
    resolve(&mut g);
    cite!(["701.5a"]; assert!(g.stack.is_empty()));
    cite!(["701.5a"]; assert_eq!(g.players[0].graveyard.iter().map(|o|g.obj(*o).chars.name.as_str()).collect::<Vec<_>>(),vec!["Bear"]));
    cite!(["608.3"]; assert_eq!(g.battlefield.iter().map(|o|g.obj(*o).chars.name.as_str()).collect::<Vec<_>>(),vec!["Fangkeeper's Familiar"]));
}

// Rules: 714.3c, 714.2c, 707.2, 714.4, 611.2c, 514.2
// Chapters II/III copy base token characteristics, not a temporary buff; IV freezes recipients.
#[test]
fn r714_mice_chapters_copy_and_final_snapshot() {
    let mut g = setup("Three Blind Mice", 2);
    mana(&mut g, 2, &[(Color::White, 1)]);
    cast(&mut g, 0, vec![], "CCW");
    resolve(&mut g);
    resolve(&mut g);
    let saga = g.battlefield[0];
    let mouse = g.battlefield[1];
    until(&mut g, 3, Part::PrecombatMain);
    cite!(["714.3c"]; assert_eq!(g.obj(saga).counters.get("lore"),Some(&2)));
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(mouse)]])).unwrap();
    cast(&mut g, 5, vec![vec![Target::Object(mouse)]], "");
    resolve(&mut g);
    resolve(&mut g);
    let copy = *g.battlefield.last().unwrap();
    cite!(["707.2"]; assert_eq!((g.power(mouse),g.power(copy),g.toughness(copy)),(Some(4),Some(1),Some(1))));
    until(&mut g, 5, Part::PrecombatMain);
    let enemy = g.put(1, 2, Zone::Battlefield);
    cite!(["603.3d"]; refused("target"): g.act(0,Move::ChooseTargets(vec![vec![Target::Object(enemy)]])));
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(copy)]])).unwrap();
    resolve(&mut g);
    cite!(["714.2c"]; assert_eq!(g.battlefield.iter().filter(|o|g.obj(**o).token()).count(),3));
    until(&mut g, 7, Part::PrecombatMain);
    cite!(["714.4"]; assert!(g.battlefield.contains(&saga)));
    resolve(&mut g);
    cite!(["714.4"]; assert!(!g.battlefield.contains(&saga)));
    let late = g.put(0, 2, Zone::Battlefield);
    cite!(["611.2c"]; assert_eq!(g.power(late),Some(2)));
    cite!(["611.2c"]; assert!(!g.has(late,"Vigilance") && !g.has(enemy,"Vigilance")));
    for o in [mouse, copy] {
        cite!(["613.4c"]; assert_eq!((g.power(o),g.toughness(o)),(Some(2),Some(2))));
        cite!(["702.20b"]; assert!(g.has(o,"Vigilance")));
    }
    until(&mut g, 8, Part::PrecombatMain);
    cite!(["514.2"]; assert_eq!(g.power(mouse),Some(1)));
    cite!(["514.2"]; assert!(!g.has(mouse,"Vigilance")));
}

// Rules: 609.3, 701.20e, 608.2c
// Empty looking does not draw; one eligible Elemental can be selected with red hybrid payment.
#[test]
fn r609_flamekin_empty_and_single_elemental_red_payment() {
    for count in 0..=1 {
        let mut g = setup("Eclipsed Flamekin", 2);
        g.players[0].library.clear();
        for _ in 0..count {
            g.put(0, 0, Zone::Library);
        }
        mana(&mut g, 1, &[(Color::Red, 2)]);
        cast(&mut g, 0, vec![], "CRR");
        resolve(&mut g);
        resolve(&mut g);
        if let Some(Decision::Choose { player: 0, choice: Choice::May }) = g.decision {
            g.act(0, Move::Choose(Answer::Yes)).unwrap();
        }
        if let Some(Decision::Choose { player: 0, choice: Choice::Objects { among, .. } }) = g.decision.clone() {
            cite!(["608.2d"]; assert_eq!(among.len(),1));
            g.act(0, Move::Choose(Answer::Objects(among))).unwrap();
        }
        cite!(["609.3"]; assert_eq!(g.players[0].hand.len(),count));
        cite!(["609.3"]; assert!(!g.players[0].lost && g.players[0].library.is_empty()));
        cite!(["701.20e"]; assert!(!g.observations(1).iter().any(|o|o.kind==game::observation::Kind::Looked)));
    }
}
// Rules: 702.6b, 702.17b, 702.9b, 702.20b, 508.1f
// Native Bow supplies reach to one ground creature, and equip is unavailable in combat.
#[test]
fn r702_bow_reach_vigilance_and_sorcery_window() {
    let mut g = setup("Short Bow", 2);
    std::sync::Arc::make_mut(&mut g.defs).push(oracle("Spectral Sailor"));
    let bow = g.put(0, 0, Zone::Battlefield);
    let bear = g.put(0, 2, Zone::Battlefield);
    let other = g.put(0, 3, Zone::Battlefield);
    let flying = g.put(1, 14, Zone::Battlefield);
    mana(&mut g, 1, &[]);
    let equip = 2; // Printed rows precede the derived equip ability.
    ability(&mut g, bow, equip, vec![vec![Target::Object(bear)]], vec![], "C").unwrap();
    resolve(&mut g);
    cite!(["702.9b", "702.17b"]; assert!(!g.cant_block(bear,flying,&[bear]) && g.cant_block(other,flying,&[other])));
    until(&mut g, 3, Part::BeginningOfCombat);
    mana(&mut g, 1, &[]);
    cite!(["702.6b"]; refused("sorcery"): ability(&mut g,bow,equip,vec![vec![Target::Object(other)]],vec![], "C"));
    until(&mut g, 3, Part::DeclareAttackers);
    g.act(0, Move::Attack(vec![(bear, Target::Player(1)), (other, Target::Player(1))])).unwrap();
    cite!(["508.1f", "702.20b"]; assert!(!g.obj(bear).tapped && g.obj(other).tapped));
}

// Rules: 601.2f, 601.2h, 733.1
// Announcing X=3 with only two generic plus UU cannot be paid; the hand card stays put.
#[test]
fn r601_spring_insufficient_x_payment_refuses() {
    let mut g = setup("Mind Spring", 2);
    let card = g.put(0, 0, Zone::Hand);
    mana(&mut g, 2, &[(Color::Blue, 2)]);
    cite!(["601.2f", "601.2h"]; refused("mana"): g.act(0,paid("CCCUU", Move::WithX(3,Box::new(Move::Cast{card,targets:vec![]})))));
    cite!(["733.1"]; assert!(g.players[0].hand.contains(&card) && g.stack.is_empty()));
    cite!(["733.1"]; assert_eq!((g.players[0].pool.colorless,g.players[0].pool.colors[Color::Blue as usize]),(2,2)));
}
// Rules: 702.6a, 613.4c, 301.5d, 602.2b, 603.6c
// Haunch's native equip increases power only; sacrificing it removes the attached bonus.
#[test]
fn r613_haunch_equipped_power_and_sacrificed_departure() {
    let mut g = setup("Krovod Haunch", 2);
    let food = g.put(0, 0, Zone::Battlefield);
    let bear = g.put(0, 2, Zone::Battlefield);
    mana(&mut g, 2, &[]);
    let equip = 4; // Printed rows precede the derived equip ability.
    ability(&mut g, food, equip, vec![vec![Target::Object(bear)]], vec![], "CC").unwrap();
    resolve(&mut g);
    cite!(["613.4c"]; assert_eq!((g.power(bear),g.toughness(bear)),(Some(4),Some(2))));
    mana(&mut g, 3, &[(Color::White, 1)]);
    ability(&mut g, food, 1, vec![], vec![], "CC").unwrap();
    cite!(["301.5d", "602.2b"]; assert_eq!((g.power(bear),g.toughness(bear)),(Some(2),Some(2))));
    resolve(&mut g);
    cite!(["118.12"]; assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::Pay{..}}))));
    g.act(0, Move::Choose(Answer::No)).unwrap();
    resolve(&mut g);
    cite!(["119.3"]; assert_eq!(g.players[0].life,23));
}
