//! A crime belongs to a completed announcement, not its eventual effects (700.13).
use crate::support::{cite, paid, mana_pool};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use std::sync::{Arc, OnceLock};

fn native(name: &str) -> CardDef {
    static FACES: OnceLock<Vec<oracle::corpus::Face>> = OnceLock::new();
    let faces = FACES.get_or_init(|| oracle::corpus::load(&oracle::corpus::default_path()).unwrap());
    let catalog = CardDef::catalog(faces);
    CardDef::from_faces(catalog.get(name).unwrap()).unwrap_or_else(|e| panic!("{name}: {e}"))
}
fn add(g: &mut Game, def: CardDef) -> usize {
    game::play::check(&def).unwrap_or_else(|e| panic!("{}: {e:?}", def.name()));
    let index = g.defs.len(); Arc::make_mut(&mut g.defs).push(def); index
}
fn synthetic(g: &mut Game, name: &str, cost: &str, types: &str, text: &str, pt: Option<(i32,i32)>) -> usize {
    add(g, CardDef::new(name, cost, types, text, pt).unwrap())
}
fn setup() -> Game { Game::new(vec![], 2, 20) }
fn cast(g: &mut Game, def: usize, targets: Vec<Vec<Target>>) -> ObjId {
    let card = g.put(0, def, Zone::Hand);
    g.act(0, Move::Cast { card, targets }).unwrap();
    *g.stack.first().unwrap()
}
fn pass(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
fn raven(g: &mut Game) -> ObjId { let d = add(g, native("Raven of Fell Omens")); g.put(0, d, Zone::Battlefield) }

#[test]
fn r700_13_multiple_qualifying_targets_are_one_crime_before_resolution() {
    let mut g = setup(); raven(&mut g);
    let bear = synthetic(&mut g, "Bear", "{0}", "Creature — Bear", "", Some((2,2)));
    let enemy = g.put(1, bear, Zone::Battlefield);
    let spell = synthetic(&mut g, "Two Gifts", "{0}", "Instant", "Target creature gets +1/+1 until end of turn. Target creature gains trample until end of turn.", None);
    cast(&mut g, spell, vec![vec![Target::Object(enemy)], vec![Target::Object(enemy)]]);
    cite!(["700.13", "603.2c"]; assert_eq!(g.stack.len(), 2));
    pass(&mut g);
    cite!(["700.13", "601.2i"]; assert_eq!((g.players[0].life, g.players[1].life, g.power(enemy)), (21,19,Some(2))));
    pass(&mut g); assert_eq!(g.power(enemy), Some(3));
}

#[test]
fn r700_13_own_target_and_untargeted_opponent_effect_are_not_crimes() {
    let mut g = setup(); raven(&mut g);
    let spell = synthetic(&mut g, "Own Gift", "{0}", "Instant", "Target player gains 1 life.", None);
    cast(&mut g, spell, vec![vec![Target::Player(0)]]);
    cite!(["700.13"]; assert_eq!(g.stack.len(), 1)); pass(&mut g);
    let loss = synthetic(&mut g, "Untargeted", "{0}", "Instant", "Each opponent loses 1 life.", None);
    cast(&mut g, loss, vec![]);
    cite!(["115.10a"]; assert_eq!(g.stack.len(), 1)); pass(&mut g);
    assert_eq!((g.players[0].life, g.players[1].life), (21,19));
}

#[test]
fn r700_13_history_is_retained_before_a_consumer_enters() {
    let mut g = setup();
    let spell = synthetic(&mut g, "Gift", "{0}", "Instant", "Target player gains 1 life.", None);
    cast(&mut g, spell, vec![vec![Target::Player(1)]]); pass(&mut g);
    let d = add(&mut g, native("Slickshot Vault-Buster")); let creature = g.put(0, d, Zone::Battlefield);
    cite!(["700.13", "608.2i"]; assert_eq!(g.power(creature), Some(3)));
}

#[test]
fn r700_13_completed_native_activation_and_trigger_quota() {
    let mut g = setup();
    let d = add(&mut g, native("Blood Hustler")); let creature = g.put(0, d, Zone::Battlefield);
    for (mine, theirs) in [(21,19),(22,18)] {
        g.players[0].pool.colorless = 3; g.players[0].pool.colors[ir::Color::Black as usize] = 1;
        let from = g.log.end();
        g.act(0, paid("CCCB",Move::Activate { source: creature, ability: 1, targets: vec![vec![Target::Player(1)]], paying: vec![] })).unwrap();
        cite!(["700.13"]; assert_eq!(g.log.since(from).iter().filter(|e|matches!(e,game::Event::CrimeCommitted{player:0})).count(),1));
        while !g.stack.is_empty() { pass(&mut g); }
        cite!(["603.2", "700.13"]; assert_eq!(g.obj(creature).counters.get("+1/+1"), Some(&1)));
        assert_eq!((g.players[0].life, g.players[1].life), (mine,theirs));
    }
}

#[test]
fn r700_13_true_trigger_placement_waits_for_targets() {
    let mut g = setup(); raven(&mut g);
    let d = synthetic(&mut g, "Messenger", "{0}", "Creature — Human", "When this creature enters, target player gains 1 life.", Some((2,2)));
    cast(&mut g, d, vec![]); pass(&mut g);
    cite!(["603.3d", "700.13"]; assert_eq!((g.players[0].life,g.players[1].life,g.stack.len()), (20,20,0)));
    g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap();
    cite!(["700.13"]; assert_eq!(g.stack.len(), 2));
    pass(&mut g); assert_eq!((g.players[0].life,g.players[1].life), (21,19));
    pass(&mut g); assert_eq!((g.players[0].life,g.players[1].life), (21,20));
}

#[test]
fn r733_1_failed_payment_commits_no_crime_or_history() {
    let mut g = setup(); raven(&mut g);
    let d = add(&mut g, native("Slickshot Vault-Buster")); let creature = g.put(0, d, Zone::Battlefield);
    let d = synthetic(&mut g, "Unpaid Gift", "{G}", "Instant", "Target player gains 1 life.", None);
    let card = g.put(0, d, Zone::Hand);
    let before = format!("{g:?}");
    cite!(["733.1"]; refused("that mana is not in your mana pool (118.3a)"): g.act(0, paid("G",Move::Cast {card, targets:vec![vec![Target::Player(1)]]})));
    assert_eq!(format!("{g:?}"), before);
    assert!(g.stack.is_empty()); assert_eq!(g.power(creature),Some(1));
    assert_eq!((g.players[0].life,g.players[1].life),(20,20));
}

// Synthetic no-target mana side effect supplies an actual control transition during
// 601.2g. The sole selected enemy is yours by 601.2i, so it no longer qualifies.
#[test]
fn r700_13_target_control_is_checked_after_nested_mana_payment() {
    let mut g = setup(); raven(&mut g);
    let bear = synthetic(&mut g, "Bear", "{0}", "Creature — Bear", "", Some((2,2)));
    let enemy = g.put(1, bear, Zone::Battlefield);
    let source_def = synthetic(&mut g, "Mana Takeover", "{0}", "Enchantment", "Sacrifice this enchantment: Add {G}. Gain control of all creatures.", None);
    let source = g.put(0, source_def, Zone::Battlefield);
    let spell_def = synthetic(&mut g, "Paid Gift", "{G}", "Instant", "Target creature gets +1/+1 until end of turn.", None);
    let spell = g.put(0, spell_def, Zone::Hand);
    g.act(0, paid("G", Move::WithMana(vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, source, ability:0, paying:vec![], adding:vec![], chose:vec![] }], Box::new(Move::Cast {card:spell, targets:vec![vec![Target::Object(enemy)]]})))).unwrap();
    cite!(["601.2i", "602.2b", "700.13", "115.9b"]; assert_eq!(g.controller(enemy),0));
    cite!(["700.13"]; assert_eq!(g.stack.len(),1));
    pass(&mut g); assert_eq!((g.players[0].life,g.players[1].life),(20,20));
}

#[test]
fn r700_13_opponent_graveyard_uses_owner_not_former_controller() {
    for (owner, former_controller, expected) in [(1,0,true),(0,1,false)] {
        let mut g = setup();
        let bear = synthetic(&mut g, "Bear", "{0}", "Creature — Bear", "", Some((2,2)));
        let old = g.put(owner, bear, Zone::Battlefield);
        let steal = synthetic(&mut g, "Steal", "{0}", "Instant", "Gain control of target creature.", None);
        if former_controller != g.priority { g.act(g.priority,Move::Pass).unwrap(); }
        let spell = g.put(former_controller,steal,Zone::Hand);
        g.act(former_controller,Move::Cast{card:spell,targets:vec![vec![Target::Object(old)]]}).unwrap(); pass(&mut g);
        assert_eq!(g.controller(old),former_controller);
        g.move_object(old,Zone::Graveyard);
        let card=*g.players[owner].graveyard.last().unwrap();
        while g.priority!=0 {g.act(g.priority,Move::Pass).unwrap();}
        raven(&mut g);
        let def = synthetic(&mut g, "Exile", "{0}", "Instant", "Exile target card from a graveyard.", None);
        cast(&mut g, def, vec![vec![Target::Object(card)]]);
        cite!(["400.3", "700.13"]; assert_eq!(g.stack.len(),if expected {2} else {1}));
    }
}

#[test]
fn r700_13_opponent_spell_and_ability_use_stack_controller() {
    for ability in [false,true] {
        let mut g = setup(); raven(&mut g);
        let creature_def = synthetic(&mut g, "Caller", "{0}", "Creature — Human", "{0}: Target player gains 1 life.", Some((2,2)));
        let creature = g.put(1, creature_def, Zone::Battlefield);
        let gift = synthetic(&mut g, "Gift", "{0}", "Instant", "Target player gains 1 life.", None);
        g.act(0,Move::Pass).unwrap();
        if ability { g.act(1,Move::Activate{source:creature,ability:0,targets:vec![vec![Target::Player(1)]],paying:vec![]}).unwrap(); }
        else { let card=g.put(1,gift,Zone::Hand); g.act(1,Move::Cast{card,targets:vec![vec![Target::Player(1)]]}).unwrap(); }
        let target=*g.stack.last().unwrap(); g.act(1,Move::Pass).unwrap();
        let text=if ability {"Counter target activated ability."} else {"Counter target spell."};
        let counter=synthetic(&mut g,"Answer","{0}","Instant",text,None);
        cast(&mut g,counter,vec![vec![Target::Object(target)]]);
        cite!(["700.13", "602.2a"]; assert_eq!(g.stack.len(),3));
        pass(&mut g); assert_eq!((g.players[0].life,g.players[1].life),(21,19));
    }
}

fn watcher(g: &mut Game) {
    let d = synthetic(g,"Crime Witness","{0}","Enchantment","Whenever you commit a crime, you gain 2 life.",None);
    g.put(0,d,Zone::Battlefield);
}

#[test]
fn r700_13_retargeting_neither_creates_nor_erases_the_completed_crime() {
    for initial in [0,1] {
        let mut g=setup(); watcher(&mut g);
        let gift=synthetic(&mut g,"Gift","{0}","Instant","Target player gains 1 life.",None);
        let original=cast(&mut g,gift,vec![vec![Target::Player(initial)]]);
        if initial==1 {pass(&mut g);}
        let life=if initial==1 {22} else {20}; assert_eq!(g.players[0].life,life);
        let change=synthetic(&mut g,"Change","{0}","Instant","You may choose new targets for target spell.",None);
        let hand=g.put(0,change,Zone::Hand);
        g.act(0,Move::Cast{card:hand,targets:vec![vec![Target::Object(original)]]}).unwrap();pass(&mut g);
        g.act(0,Move::Choose(game::state::Answer::Targets(vec![vec![Target::Player(1-initial)]]))).unwrap();
        cite!(["700.13", "115.7"]; assert_eq!(g.stack.len(),1));
        cite!(["700.13"]; assert_eq!(g.players[0].life,life));
    }
}

#[test]
fn r707_10_retargeted_spell_copy_is_not_a_new_crime() {
    let mut g=setup(); watcher(&mut g);
    let gift=synthetic(&mut g,"Gift","{0}","Instant","Target player gains 1 life.",None);
    let original=cast(&mut g,gift,vec![vec![Target::Player(0)]]);
    let mirror=synthetic(&mut g,"Mirror","{0}","Instant","Copy target instant or sorcery spell. You may choose new targets for the copy.",None);
    let hand=g.put(0,mirror,Zone::Hand);
    g.act(0,Move::Cast{card:hand,targets:vec![vec![Target::Object(original)]]}).unwrap();pass(&mut g);
    g.act(0,Move::Choose(game::state::Answer::Targets(vec![vec![Target::Player(1)]]))).unwrap();
    cite!(["707.10", "700.13"]; assert_eq!(g.stack.len(),2));
    pass(&mut g); assert_eq!((g.players[0].life,g.players[1].life),(20,21));
}

#[test]
fn r700_13_forsaken_miner_observes_sacrifice_cost_before_completed_crime() {
    let mut g=setup();
    let miner_def=add(&mut g,native("Forsaken Miner")); let miner=g.put(0,miner_def,Zone::Battlefield);
    let bear=synthetic(&mut g,"Bear","{0}","Creature — Bear","",Some((2,2)));let enemy=g.put(1,bear,Zone::Battlefield);
    let removal=synthetic(&mut g,"Sacrifice Removal","{0}","Instant","As an additional cost to cast this spell, sacrifice a creature.\nDestroy target creature.",None);
    let hand=g.put(0,removal,Zone::Hand);g.players[0].pool.colors[ir::Color::Black as usize]=1;
    g.act(0,Move::WithCosts(vec![vec![miner]],Box::new(Move::Cast{card:hand,targets:vec![vec![Target::Object(enemy)]]}))).unwrap();
    cite!(["601.2i", "700.13", "113.6m"]; assert_eq!(g.stack.len(),2));
    pass(&mut g);g.act(0,Move::Choose(game::state::Answer::Pay {plan:vec![],with:Some(mana_pool("B")),tapped:vec![]})).unwrap();
    cite!(["608.2h"]; assert_eq!(g.battlefield.iter().filter(|id|g.obj(**id).chars.name=="Forsaken Miner").count(),1));
    assert!(g.battlefield.contains(&enemy));
}

#[test]
fn r700_13_miner_killed_by_the_crime_does_not_trigger_retroactively() {
    let mut g=setup();
    let miner_def=add(&mut g,native("Forsaken Miner"));let miner=g.put(0,miner_def,Zone::Battlefield);
    let bear=synthetic(&mut g,"Bear","{0}","Creature — Bear","",Some((2,2)));let enemy=g.put(1,bear,Zone::Battlefield);
    let removal=synthetic(&mut g,"Two Removals","{0}","Instant","Destroy target creature. Destroy target creature.",None);
    cast(&mut g,removal,vec![vec![Target::Object(enemy)],vec![Target::Object(miner)]]);
    cite!(["700.13", "603.2"]; assert_eq!(g.stack.len(),1)); pass(&mut g);
    assert!(g.stack.is_empty());assert_eq!(g.players[0].graveyard.iter().filter(|id|g.obj(**id).chars.name=="Forsaken Miner").count(),1);
}

#[test]
fn r700_13_native_cost_reduction_uses_only_completed_crime_history() {
    let mut g=setup();
    let land=synthetic(&mut g,"Forest","","Basic Land — Forest","",None);
    for _ in 0..5 {g.put(0,land,Zone::Library);}
    let seize=add(&mut g,native("Seize the Secrets"));let hand=g.put(0,seize,Zone::Hand);
    g.players[0].pool.colorless=1;g.players[0].pool.colors[ir::Color::Blue as usize]=1;
    let before = format!("{g:?}");
    cite!(["601.2f"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"):g.act(0,paid("CU",Move::Cast{card:hand,targets:vec![]})));
    assert_eq!(format!("{g:?}"), before);
    let gift=synthetic(&mut g,"Gift","{0}","Instant","Target player gains 1 life.",None);
    cast(&mut g,gift,vec![vec![Target::Player(1)]]);pass(&mut g);
    cite!(["700.13", "601.2f"]; ok:g.act(0,paid("CU",Move::Cast{card:hand,targets:vec![]})));pass(&mut g);
    assert_eq!(g.players[0].hand.len(),2);assert_eq!(g.players[0].pool.colorless,0);
}

#[test]
fn r700_13_history_and_native_trigger_quota_reset_on_every_turn() {
    let mut g=setup();raven(&mut g);
    let vault=add(&mut g,native("Slickshot Vault-Buster"));let creature=g.put(0,vault,Zone::Battlefield);
    let land=synthetic(&mut g,"Forest","","Basic Land — Forest","",None);
    for player in 0..2 {for _ in 0..8 {g.put(player,land,Zone::Library);}}
    let gift=synthetic(&mut g,"Gift","{0}","Instant","Target player gains 1 life.",None);
    for _ in 0..2 {cast(&mut g,gift,vec![vec![Target::Player(1)]]);while !g.stack.is_empty(){pass(&mut g);}}
    assert_eq!(g.players[0].life,21);assert_eq!(g.power(creature),Some(3));
    let turn=g.turn;
    for _ in 0..100 {
        if g.turn>turn && g.active==1 && g.part==ir::Part::PrecombatMain {break}
        let (player,asked)=g.asked().unwrap();
        let mv=match asked {game::legal::Asked::Priority=>Move::Pass,game::legal::Asked::Attackers(_)=>Move::Attack(vec![]), other=>panic!("{other:?}")};
        g.act(player,mv).unwrap();
    }
    cite!(["700.13", "608.2i"]; assert_eq!(g.power(creature),Some(1)));
    while g.priority!=0 {g.act(g.priority,Move::Pass).unwrap();}
    cast(&mut g,gift,vec![vec![Target::Player(1)]]);pass(&mut g);
    cite!(["603.2", "700.13"]; assert_eq!(g.players[0].life,22));
    assert_eq!(g.power(creature),Some(3));
}

#[test]
fn r733_1_failed_activation_does_not_commit_a_crime() {
    let mut g=setup();watcher(&mut g);
    let d=add(&mut g,native("Blood Hustler"));let source=g.put(0,d,Zone::Battlefield);
    let from=g.log.end();
    let before = format!("{g:?}");
    cite!(["733.1"]; refused("that mana is not in your mana pool (118.3a)"):g.act(0,paid("CCCB",Move::Activate{source,ability:1,targets:vec![vec![Target::Player(1)]],paying:vec![]})));
    assert_eq!(format!("{g:?}"), before);
    assert_eq!(g.log.since(from).iter().filter(|e|matches!(e,game::Event::CrimeCommitted{..})).count(),0);
    assert!(g.stack.is_empty());assert_eq!(g.players[0].life,20);
}

#[test]
fn r700_13_suspended_and_reversed_activation_waits_for_completion() {
    let mut g=setup();watcher(&mut g);
    let mana_def=synthetic(&mut g,"Late Mana","{0}","Creature — Elf","{T}: Put a +1/+1 counter on this creature. Add X mana of any one color, where X is the number of +1/+1 counters on this creature.",Some((1,1)));
    let mana=g.put(0,mana_def,Zone::Battlefield);g.turn=2;
    let scales=synthetic(&mut g,"Scales","{0}","Enchantment","If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.",None);
    g.put(0,scales,Zone::Battlefield);
    let evolution=synthetic(&mut g,"Evolution","{0}","Enchantment","If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.",None);
    let evolution=g.put(0,evolution,Zone::Battlefield);
    let outer_def=synthetic(&mut g,"Consumer","{0}","Artifact","{4}: Target player gains 1 life.",None);
    let source=g.put(0,outer_def,Zone::Battlefield);
    let act=Move::Activate{source,ability:0,targets:vec![vec![Target::Player(1)]],paying:vec![]};
    let step=game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, source:mana,ability:0,paying:vec![],adding:vec![],chose:vec![]};
    let from=g.log.end();
    g.act(0,paid("GGGG",Move::WithMana(vec![step],Box::new(act.clone())))).unwrap();
    assert!(g.decision.is_some());
    assert_eq!(g.log.since(from).iter().filter(|e|matches!(e,game::Event::CrimeCommitted{..})).count(),0);
    let Some(game::state::Decision::Choose{choice:game::state::Choice::Replacement{among,..},..})=&g.decision else {panic!("replacement choice")};
    let first=among.iter().find(|r|r.source==evolution).unwrap().clone();
    g.act(0,Move::Choose(game::state::Answer::Replacement(first))).unwrap();
    g.act(0,Move::Choose(game::state::Answer::Mana(vec![ir::mana::ManaSymbol::Color(ir::Color::Green);3]))).unwrap();
    assert!(matches!(g.decision,Some(game::state::Decision::Payment{..})));
    cite!(["700.13", "602.2b"]; assert_eq!(g.log.since(from).iter().filter(|e|matches!(e,game::Event::CrimeCommitted{..})).count(),0));
    g.act(0,Move::ReversePayment).unwrap();assert_eq!(g.players[0].life,20);
    g.players[0].pool.colorless=4;
    let from=g.log.end();g.act(0,paid("CCCC",act)).unwrap();
    cite!(["700.13"]; assert_eq!(g.log.since(from).iter().filter(|e|matches!(e,game::Event::CrimeCommitted{player:0})).count(),1));
    pass(&mut g);assert_eq!(g.players[0].life,22);
}

#[test]
fn r707_10_retargeted_activated_ability_copy_is_not_new_crime() {
    let mut g=setup();watcher(&mut g);
    let pit=add(&mut g,native("Pit Automaton"));let source=g.put(0,pit,Zone::Battlefield);
    g.players[0].pool.colorless=2;
    g.act(0,paid("CC",Move::Activate{source,ability:2,targets:vec![],paying:vec![]})).unwrap();pass(&mut g);
    let donor=synthetic(&mut g,"Exhaust Witness","{0}","Artifact","Exhaust — {0}: Target player gains 1 life.",None);
    let donor=g.put(0,donor,Zone::Battlefield);
    g.act(0,Move::Activate{source:donor,ability:0,targets:vec![vec![Target::Player(0)]],paying:vec![]}).unwrap();
    pass(&mut g);
    g.act(0,Move::Choose(game::state::Answer::Targets(vec![vec![Target::Player(1)]]))).unwrap();
    cite!(["707.10", "700.13"]; assert_eq!(g.stack.len(),2));
    pass(&mut g);assert_eq!((g.players[0].life,g.players[1].life),(20,21));
}

#[test]
fn r707_12_casting_a_card_copy_can_commit_crime() {
    let reenact=native("Reenact the Crime");game::play::check(&reenact).unwrap();
    let mut g=Game::new(vec![reenact],2,20);watcher(&mut g);
    let gift=synthetic(&mut g,"Gift","{0}","Instant","Target player gains 1 life.",None);
    let old=g.put(0,gift,Zone::Hand);g.move_object(old,Zone::Graveyard);let target=*g.players[0].graveyard.last().unwrap();
    let hand=g.put(0,0,Zone::Hand);
    g.players[0].pool.colorless=1;g.players[0].pool.colors[ir::Color::Blue as usize]=3;
    g.act(0,paid("CUUU",Move::Cast{card:hand,targets:vec![vec![Target::Object(target)]]})).unwrap();pass(&mut g);
    let Some((0,game::legal::Asked::Cast{cards,..}))=g.asked() else {panic!("card copy offer")};
    let copy=cards[0];
    g.act(0,Move::Cast{card:copy,targets:vec![vec![Target::Player(1)]]}).unwrap();
    assert_eq!(g.stack.len(),2);pass(&mut g);
    cite!(["707.12", "700.13"]; assert_eq!(g.players[0].life,22));
    assert_eq!(g.players[1].life,20);
}

#[test]
fn r700_13_control_duration_ends_during_costs_before_crime() {
    let mut g=setup();
    let bear=synthetic(&mut g,"Bear","{0}","Creature — Bear","",Some((2,2)));
    let target=g.put(1,bear,Zone::Battlefield);
    let source_def=synthetic(&mut g,"Control Source","{0}","Enchantment","{0}: Gain control of target creature for as long as you control this enchantment.\nSacrifice this enchantment: Add {G}.\n{G}: Target creature gets +1/+1 until end of turn.",None);
    let source=g.put(0,source_def,Zone::Battlefield);
    g.act(0,Move::Activate{source,ability:0,targets:vec![vec![Target::Object(target)]],paying:vec![]}).unwrap();pass(&mut g);
    assert_eq!(g.controller(target),0);watcher(&mut g);
    let from=g.log.end();
    let step=game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, source,ability:1,paying:vec![],adding:vec![],chose:vec![]};
    g.act(0,paid("G",Move::WithMana(vec![step],Box::new(Move::Activate{source,ability:2,targets:vec![vec![Target::Object(target)]],paying:vec![]})))).unwrap();
    cite!(["115.9b", "700.13"]; assert_eq!(g.controller(target),1));
    cite!(["113.7a", "602.2b", "700.13"]; assert_eq!(g.log.since(from).iter().filter(|e|matches!(e,game::Event::CrimeCommitted{player:0})).count(),1));
    assert_eq!(g.stack.len(),2);pass(&mut g);assert_eq!(g.players[0].life,22);
}

#[test]
fn r700_13_countering_does_not_erase_the_original_actor_history() {
    let mut g=setup();watcher(&mut g);
    let vault=add(&mut g,native("Slickshot Vault-Buster"));let creature=g.put(0,vault,Zone::Battlefield);
    let gift=synthetic(&mut g,"Gift","{0}","Instant","Target player gains 1 life.",None);
    let original=cast(&mut g,gift,vec![vec![Target::Player(1)]]);pass(&mut g);
    assert_eq!(g.players[0].life,22);assert_eq!(g.players[1].life,20);
    let counter=synthetic(&mut g,"Counter","{0}","Instant","Counter target spell.",None);
    g.act(0,Move::Pass).unwrap();let hand=g.put(1,counter,Zone::Hand);
    g.act(1,Move::Cast{card:hand,targets:vec![vec![Target::Object(original)]]}).unwrap();
    assert_eq!(g.stack.len(),2);pass(&mut g);
    cite!(["700.13", "608.2i"]; assert_eq!(g.power(creature),Some(3)));
    cite!(["700.13"]; assert_eq!(g.players[0].life,22));assert_eq!(g.players[1].life,20);assert!(g.stack.is_empty());
}

#[test]
fn r115_9b_target_departure_during_costs_does_not_use_lki_for_crime() {
    let mut g=setup();watcher(&mut g);
    let bear=synthetic(&mut g,"Bear","{0}","Creature — Bear","",Some((2,2)));
    let target=g.put(1,bear,Zone::Battlefield);
    let mana=synthetic(&mut g,"Mana Sweep","{0}","Artifact","Sacrifice this artifact: Add {G}. Destroy all creatures.",None);
    let source=g.put(0,mana,Zone::Battlefield);
    let spell=synthetic(&mut g,"Growth","{G}","Instant","Target creature gets +1/+1 until end of turn.",None);
    let card=g.put(0,spell,Zone::Hand);let from=g.log.end();
    let step=game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, source,ability:0,paying:vec![],adding:vec![],chose:vec![]};
    g.act(0,paid("G",Move::WithMana(vec![step],Box::new(Move::Cast{card,targets:vec![vec![Target::Object(target)]]})))).unwrap();
    cite!(["115.9b", "700.13"]; assert_eq!(g.log.since(from).iter().filter(|e|matches!(e,game::Event::CrimeCommitted{..})).count(),0));
    assert_eq!(g.stack.len(),1);assert_eq!(g.players[0].life,20);
}

#[test]
fn r115_9b_illegal_but_current_target_still_qualifies_for_crime() {
    let mut g=setup();watcher(&mut g);
    let bear=synthetic(&mut g,"Bear","{0}","Creature — Bear","",Some((2,2)));
    let target=g.put(1,bear,Zone::Battlefield);
    let mana=synthetic(&mut g,"Mana Veil","{0}","Artifact","Sacrifice this artifact: Add {G}. All creatures gain shroud until end of turn.",None);
    let source=g.put(0,mana,Zone::Battlefield);
    let spell=synthetic(&mut g,"Growth","{G}","Instant","Target creature gets +1/+1 until end of turn.",None);
    let card=g.put(0,spell,Zone::Hand);let from=g.log.end();
    let step=game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, source,ability:0,paying:vec![],adding:vec![],chose:vec![]};
    g.act(0,paid("G",Move::WithMana(vec![step],Box::new(Move::Cast{card,targets:vec![vec![Target::Object(target)]]})))).unwrap();
    cite!(["115.9b", "700.13"]; assert_eq!(g.log.since(from).iter().filter(|e|matches!(e,game::Event::CrimeCommitted{player:0})).count(),1));
    assert_eq!(g.stack.len(),2);pass(&mut g);assert_eq!(g.players[0].life,22);
    pass(&mut g);cite!(["608.2b"]; assert_eq!(g.power(target),Some(2)));
}
