//! Generic native counter replacements act on one holder inventory (614.5/616.1–2).
use crate::support::{cite, paid};
use game::{CardDef,Game,Move,ObjId,Target,Zone};
use game::state::{Answer,Choice,Decision};
const DOC:usize=0;
const ZONE:usize=1;
const BEAR:usize=2;
const PUT:usize=3;
const SEASON:usize=4;
const WALKER:usize=5;
const TALENT:usize=6;
const ENDURER:usize=7;
const ENTRANT:usize=8;
const STRIP:usize=9;
const SELF:usize=10;
const GRANT:usize=11;
const INFECTOR:usize=12;
const TOUGH:usize=13;
const TYRANT:usize=14;
const SAGA:usize=15;
const BLIGHTER:usize=16;
const WIDE:usize=17;
fn setup()->Game {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog=CardDef::catalog(&faces);
    let mut defs=vec![CardDef::from_faces(&catalog["Doc Samson, Super Psychiatrist"]).unwrap(),
        CardDef::from_faces(&catalog["Loading Zone"]).unwrap()];
    defs.extend([
        CardDef::new("Bear","{0}","Creature","",Some((2,2))).unwrap(),
        CardDef::new("Inventory","{0}","Instant","Put a charge counter and two stun counters on target creature.",None).unwrap(),
    ]);
    // Only Season's exact printed counter clause is isolated: full token-creation
    // replacement support is a separate matter.
    defs.push(CardDef::new("Season counter clause","{0}","Enchantment",
        "If an effect would put one or more counters on a permanent you control, it puts twice that many of those counters on that permanent instead.",None).unwrap());
    defs.push(CardDef::planeswalker("Walker","{0}","Planeswalker — Jace","+1: You gain 1 life.",3).unwrap());
    defs.push(CardDef::from_faces(&catalog["Innkeeper's Talent"]).unwrap());
    defs.push(CardDef::new("Endurer","{0}","Creature","{0}: This creature endures 2.",Some((2,2))).unwrap());
    defs.push(CardDef::new("Entrant","{0}","Creature",
        "This creature enters with a charge counter on it.\nThis creature enters with a stun counter on it.",Some((2,2))).unwrap());
    defs.push(CardDef::new("Strip","{0}","Instant","Target enchantment loses all abilities until end of turn.",None).unwrap());
    defs.push(CardDef::new("Own general modifier","{0}","Creature",
        "This creature enters with a charge counter on it.\nIf you would put one or more counters on a permanent you control, put that many plus one of each of those kinds of counters on that permanent instead.",Some((2,2))).unwrap());
    defs.push(CardDef::new("Grant","{0}","Enchantment",
        "Creatures you control have \"If you would put one or more counters on a permanent you control, put that many plus one of each of those kinds of counters on that permanent instead.\"",None).unwrap());
    defs.push(CardDef::new("Infector","{0}","Creature","Infect, haste\n{0}: This creature deals 2 damage to target creature.",Some((2,2))).unwrap());
    defs.push(CardDef::new("Tough","{0}","Creature","",Some((10,10))).unwrap());
    defs.push(CardDef::from_faces(&catalog["Twinflame Tyrant"]).unwrap());
    defs.push(CardDef::new("Chronicle","{0}","Enchantment — Saga","I, II, III, IV — You gain 1 life.",None).unwrap());
    defs.push(CardDef::new("Blighter","{0}","Creature","Blight 1: You gain 1 life.",Some((10,10))).unwrap());
    defs.push(CardDef::new("Wide","{0}","Instant","Put a charge counter and two stun counters on each creature you control.",None).unwrap());
    for d in &defs {game::play::check(d).unwrap_or_else(|e|panic!("{}: {}",d.name(),e.0));}
    Game::new(defs,2,20)
}
fn resolve(g:&mut Game) {for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}}
fn choose(g:&mut Game,source:ObjId) {
    let Some(Decision::Choose{player,choice:Choice::Replacement{among,..}})=g.decision.clone() else {panic!("replacement question: {:?}",g.decision)};
    let id=among.into_iter().find(|id|id.source==source).expect("authored source offered");
    g.act(player,Move::Choose(Answer::Replacement(id))).unwrap();
}
fn level_three(g:&mut Game,talent:ObjId) {
    g.players[0].pool.colors[4]=5;
    for (level,spending) in [(2,"G"),(3,"GGGG")] {
        let ability=g.abilities(talent).iter().enumerate().filter(|(_,a)|matches!(&***a,ir::Ability::Activated{..})).nth(level-2).unwrap().0;
        g.act(0,paid(spending,Move::Activate{source:talent,ability,targets:vec![],paying:vec![]})).unwrap();resolve(g);
    }
    assert_eq!(g.obj(talent).level,Some(3));
}
fn cast(g:&mut Game,player:usize,bear:ObjId) {
    let card=g.put(player,PUT,Zone::Hand);
    g.act(player,Move::Cast{card,targets:vec![vec![Target::Object(bear)]]}).unwrap();
    resolve(g);
}
#[test]
fn r614_5_one_simultaneous_instruction_uses_one_order_for_all_kinds() {
    for plus_first in [true,false] {
        let mut g=setup();let doc=g.put(0,DOC,Zone::Battlefield);let zone=g.put(0,ZONE,Zone::Battlefield);let bear=g.put(0,BEAR,Zone::Battlefield);
        cast(&mut g,0,bear);
        choose(&mut g,if plus_first {doc}else{zone});
        let charge=if plus_first {4}else{3};let stun=if plus_first {6}else{5};
        cite!(["614.5","616.1","616.2"];assert_eq!(g.obj(bear).counters.get("charge"),Some(&charge)));
        assert_eq!(g.obj(bear).counters.get("stun"),Some(&stun));
        assert!(g.decision.as_ref().is_none_or(|d|!matches!(d,Decision::Choose{choice:Choice::Replacement{..},..})));
    }
}
#[test]
fn r122_6_active_actor_is_not_the_affected_permanents_controller() {
    let mut g=setup();g.put(0,DOC,Zone::Battlefield);g.put(0,ZONE,Zone::Battlefield);let bear=g.put(0,BEAR,Zone::Battlefield);
    // Seat1 casts during seat0's turn; its placing player remains seat1. Doc's "you"
    // does not match merely because seat0 controls the recipient; passive Loading Zone does.
    g.act(0,Move::Pass).unwrap();cast(&mut g,1,bear);
    cite!(["122.6","616.1"];assert_eq!(g.obj(bear).counters.get("charge"),Some(&2)));
    assert_eq!(g.obj(bear).counters.get("stun"),Some(&4));
}

#[test]
fn r614_16_announcement_cost_is_not_effect_but_counter_replacement_makes_it_eligible() {
    for modified in [false,true] {
        let mut g=setup();g.put(0,SEASON,Zone::Battlefield);
        if modified {g.put(0,DOC,Zone::Battlefield);}
        let walker=g.put(0,WALKER,Zone::Battlefield);let before=g.obj(walker).counters.get("loyalty").copied().unwrap_or(0);
        g.act(0,Move::Activate{source:walker,ability:0,targets:vec![],paying:vec![]}).unwrap();
        let delta=if modified {4}else{1};
        cite!(["614.16","616.2","118.11"];assert_eq!(g.obj(walker).counters.get("loyalty"),Some(&(before+delta))));
        resolve(&mut g);assert_eq!(g.players[0].life,21);
    }
}
#[test]
fn r616_2_native_generic_counter_cost_order_suspends_and_commits_once() {
    let mut g=setup();let doc=g.put(0,DOC,Zone::Battlefield);let season=g.put(0,SEASON,Zone::Battlefield);let talent=g.put(0,TALENT,Zone::Battlefield);
    level_three(&mut g,talent);
    let walker=g.put(0,WALKER,Zone::Battlefield);let before=g.obj(walker).counters.get("loyalty").copied().unwrap_or(0);
    let start=g.log.len();
    g.act(0,Move::Activate{source:walker,ability:0,targets:vec![],paying:vec![]}).unwrap();
    choose(&mut g,doc);choose(&mut g,season);
    cite!(["616.1","616.2","118.11"];assert_eq!(g.obj(walker).counters.get("loyalty"),Some(&(before+8))));
    let actual:Vec<_>=g.log[start..].iter().filter_map(|e|if let game::Event::CountersPut{on:Target::Object(o),kind,count,..}=e {(*o==walker && kind=="loyalty").then_some(*count)}else{None}).collect();
    assert_eq!(actual,vec![8]);resolve(&mut g);assert_eq!(g.players[0].life,21);
}
#[test]
fn r614_16_endure_optional_counter_payment_is_instruction_of_resolving_effect() {
    let mut g=setup();g.put(0,SEASON,Zone::Battlefield);let source=g.put(0,ENDURER,Zone::Battlefield);
    g.act(0,Move::Activate{source,ability:0,targets:vec![],paying:vec![]}).unwrap();resolve(&mut g);
    g.act(0,Move::Choose(Answer::Alternative(0))).unwrap();
    cite!(["701.63a","118.12a","614.16"];assert_eq!(g.obj(source).counters.get("+1/+1"),Some(&4)));
    assert_eq!(g.battlefield.len(),2);
}

fn choose_entry_row(g:&mut Game,row:usize) {
    let Some(Decision::Choose{player,choice:Choice::Replacement{among,..}})=g.decision.clone() else {panic!("entry replacement: {:?}",g.decision)};
    let id=among.into_iter().find(|id|g.obj(id.source).chars.name=="Entrant" && id.ability().unwrap().index==row).expect("authored entry replacement");
    g.act(player,Move::Choose(Answer::Replacement(id))).unwrap();
}
#[test]
fn r614_5_entry_inventory_additions_do_not_reset_used_generic_replacement() {
    for add_first in [false,true] {
        let mut g=setup();let doc=g.put(0,DOC,Zone::Battlefield);let zone=g.put(0,ZONE,Zone::Battlefield);
        let card=g.put(0,ENTRANT,Zone::Hand);g.act(0,Move::Cast{card,targets:vec![]}).unwrap();resolve(&mut g);
        choose_entry_row(&mut g,0);
        if add_first {choose_entry_row(&mut g,1);choose(&mut g,doc);}
        else {choose(&mut g,doc);choose(&mut g,zone);}
        let entrant=*g.battlefield.iter().find(|o|g.obj(**o).card==Some(ENTRANT)).unwrap();
        cite!(["614.5","616.2"];assert_eq!(g.obj(entrant).counters.get("charge"),Some(&4)));
        assert_eq!(g.obj(entrant).counters.get("stun"),Some(&if add_first {4}else{1}));
    }
}

#[test]
fn r613_1f_native_class_counter_grant_acquires_and_loses_effective_membership() {
    let mut g=setup();let talent=g.put(0,TALENT,Zone::Battlefield);let bear=g.put(0,BEAR,Zone::Battlefield);
    cast(&mut g,0,bear);
    cite!(["716.2a"];assert_eq!(g.obj(bear).counters.get("charge"),Some(&1)));
    level_three(&mut g,talent);cast(&mut g,0,bear);
    assert_eq!(g.obj(bear).counters.get("charge"),Some(&3));
    let strip=g.put(0,STRIP,Zone::Hand);g.act(0,Move::Cast{card:strip,targets:vec![vec![talent.into()]]}).unwrap();resolve(&mut g);
    cast(&mut g,0,bear);
    cite!(["613.1f","614.1"];assert_eq!(g.obj(bear).counters.get("charge"),Some(&4)));
    assert_eq!(g.obj(bear).counters.get("stun"),Some(&8));
}
#[test]
fn r614_12_granted_native_counter_modifier_is_seen_in_prospective_entry() {
    let mut g=setup();let talent=g.put(0,TALENT,Zone::Battlefield);level_three(&mut g,talent);
    let card=g.put(0,ENTRANT,Zone::Hand);g.act(0,Move::Cast{card,targets:vec![]}).unwrap();resolve(&mut g);
    choose_entry_row(&mut g,0);
    choose_entry_row(&mut g,1); // Add both kinds before the granted doubling applies.
    let entrant=*g.battlefield.iter().find(|o|g.obj(**o).card==Some(ENTRANT)).unwrap();
    cite!(["614.12","614.5"];assert_eq!(g.obj(entrant).counters.get("charge"),Some(&2)));
    assert_eq!(g.obj(entrant).counters.get("stun"),Some(&2));
}

#[test]
fn r614_12_entering_own_general_subset_modifier_is_not_newly_in_force() {
    let mut g=setup();let card=g.put(0,SELF,Zone::Hand);
    g.act(0,Move::Cast{card,targets:vec![]}).unwrap();resolve(&mut g);
    let own=*g.battlefield.iter().find(|o|g.obj(**o).card==Some(SELF)).unwrap();
    cite!(["614.12"];assert_eq!(g.obj(own).counters.get("charge"),Some(&1)));
    cast(&mut g,0,own);assert_eq!(g.obj(own).counters.get("charge"),Some(&3));
}

#[test]
fn r614_12_external_giver_does_not_make_new_entrant_general_ability_apply() {
    let mut g=setup();g.put(0,GRANT,Zone::Battlefield);
    let card=g.put(0,ENTRANT,Zone::Hand);g.act(0,Move::Cast{card,targets:vec![]}).unwrap();resolve(&mut g);
    choose_entry_row(&mut g,0); // The other arrival then applies automatically.
    let entrant=*g.battlefield.iter().find(|o|g.obj(**o).card==Some(ENTRANT)).unwrap();
    cite!(["614.12"];assert_eq!(g.obj(entrant).counters.get("charge"),Some(&1)));
    assert_eq!(g.obj(entrant).counters.get("stun"),Some(&1));
    cast(&mut g,0,entrant);assert_eq!(g.obj(entrant).counters.get("charge"),Some(&3));
}

#[test]
fn r120_4_damage_stage_modification_is_distinct_from_counter_result_modification() {
    // Synthetic infect source isolates the already-owned damage-result rule; it is not
    // a claim that an Infect card is in the current Standard pool.
    for (combat,damage_modifier,counter_modifier,expected) in [(false,false,false,4),(true,false,false,2),(true,true,false,4),(true,false,true,8)] {
        let mut g=setup();g.put(1,SEASON,Zone::Battlefield);
        if damage_modifier {g.put(0,TYRANT,Zone::Battlefield);}
        if counter_modifier {g.put(1,ZONE,Zone::Battlefield);}
        let source=g.put(0,INFECTOR,Zone::Battlefield);let target=g.put(1,TOUGH,Zone::Battlefield);
        if combat {
            g.part=ir::Part::BeginningOfCombat;
            resolve(&mut g);g.act(0,Move::Attack(vec![(source,Target::Player(1))])).unwrap();
            for _ in 0..20 {
                if let Some(Decision::Blockers{player,..})=g.decision.clone() {
                    g.act(player,Move::Block(vec![(target,source)])).unwrap();break;
                }
                g.act(g.priority,Move::Pass).unwrap();
            }
            for _ in 0..20 {
                if g.part==ir::Part::EndOfCombat {break;}
                g.act(g.priority,Move::Pass).unwrap();
            }
            assert_eq!(g.part,ir::Part::EndOfCombat);
        } else {
            let ability=g.abilities(source).iter().position(|a|matches!(&**a,ir::Ability::Activated{..})).unwrap();
            g.act(0,Move::Activate{source,ability,targets:vec![vec![target.into()]],paying:vec![]}).unwrap();resolve(&mut g);
        }
        cite!(["120.3d","120.4","614.16","616.2"];assert_eq!(g.obj(target).counters.get("-1/-1"),Some(&expected)));
    }
}

#[test]
fn r714_3_entry_lore_replacement_differs_from_turn_based_lore_action() {
    let mut g=setup();g.put(0,SEASON,Zone::Battlefield);
    for player in 0..2 {for _ in 0..3 {g.put(player,BEAR,Zone::Library);}}
    let card=g.put(0,SAGA,Zone::Hand);g.act(0,Move::Cast{card,targets:vec![]}).unwrap();resolve(&mut g);
    let saga=*g.battlefield.iter().find(|o|g.obj(**o).card==Some(SAGA)).unwrap();
    cite!(["714.3a","614.16"];assert_eq!(g.obj(saga).counters.get("lore"),Some(&2)));
    // The two crossed chapters are ordinary pending triggers, not further lore placement.
    for _ in 0..30 {
        match g.decision.clone() {
            Some(Decision::Order{player,abilities})=>g.act(player,Move::Order((0..abilities.len()).collect())).unwrap(),
            None if g.stack.is_empty()=>break,
            None=>g.act(g.priority,Move::Pass).unwrap(),
            other=>panic!("chapter question: {other:?}"),
        }
    }
    assert_eq!(g.players[0].life,22);
    for _ in 0..100 {
        if g.turn==3 && g.part==ir::Part::PrecombatMain {break;}
        match g.decision.clone() {
            Some(Decision::Attackers{player,..})=>g.act(player,Move::Attack(vec![])).unwrap(),
            Some(Decision::Blockers{player,..})=>g.act(player,Move::Block(vec![])).unwrap(),
            Some(Decision::Order{player,abilities})=>g.act(player,Move::Order((0..abilities.len()).collect())).unwrap(),
            None=>g.act(g.priority,Move::Pass).unwrap(),
            other=>panic!("turn question: {other:?}"),
        }
    }
    assert_eq!((g.turn,g.part),(3,ir::Part::PrecombatMain));
    cite!(["714.3c","614.16"];assert_eq!(g.obj(saga).counters.get("lore"),Some(&3)));
}

#[test]
fn r118_11_blight_counter_cost_resumes_once_after_generic_replacement_choice() {
    let mut g=setup();let doc=g.put(0,DOC,Zone::Battlefield);let talent=g.put(0,TALENT,Zone::Battlefield);
    level_three(&mut g,talent);let source=g.put(0,BLIGHTER,Zone::Battlefield);
    let start=g.log.len();
    g.act(0,Move::Activate{source,ability:0,targets:vec![],paying:vec![vec![source]]}).unwrap();
    assert!(g.obj(source).counters.is_empty()); // No partial payment while ordering is pending.
    choose(&mut g,doc); // (1+1)*2: one native modifier order for the whole cost.
    cite!(["701.68a","118.11","616.1"];assert_eq!(g.obj(source).counters.get("-1/-1"),Some(&4)));
    let actual:Vec<_>=g.log[start..].iter().filter_map(|e|if let game::Event::CountersPut{on:Target::Object(o),kind,count,..}=e {(*o==source && kind=="-1/-1").then_some(*count)}else{None}).collect();
    assert_eq!(actual,vec![4]);assert_eq!(g.stack.len(),1);
    resolve(&mut g);assert_eq!(g.players[0].life,21);
}
#[test]
fn r616_1_separate_holders_keep_independent_inventory_ordering_history() {
    let mut g=setup();let doc=g.put(0,DOC,Zone::Battlefield);let zone=g.put(0,ZONE,Zone::Battlefield);
    let a=g.put(0,BEAR,Zone::Battlefield);let b=g.put(0,BEAR,Zone::Battlefield);
    let card=g.put(0,WIDE,Zone::Hand);g.act(0,Move::Cast{card,targets:vec![]}).unwrap();resolve(&mut g);
    // Doc is also a creature: every affected holder gets its own replacement order.
    let mut assigned=vec![];
    while let Some(Decision::Choose{choice:Choice::Replacement{on:Target::Object(holder),..},..})=g.decision.clone() {
        let first=if holder==b {zone}else{doc};choose(&mut g,first);assigned.push(holder);
    }
    assert!(assigned.contains(&a)&&assigned.contains(&b));
    cite!(["616.1","614.5"];assert_eq!(g.obj(a).counters.get("charge"),Some(&4)));
    assert_eq!(g.obj(a).counters.get("stun"),Some(&6));
    assert_eq!(g.obj(b).counters.get("charge"),Some(&3));
    assert_eq!(g.obj(b).counters.get("stun"),Some(&5));
}

#[test]
fn r608_2g_nested_discover_cast_cost_does_not_inherit_outer_effect_cause() {
    for replacement in [false,true] {
        let mut defs=setup().defs.to_vec();let finder=defs.len();
        defs.push(CardDef::new("Finder","{0}","Sorcery","Discover 0.",None).unwrap());let demand=defs.len();
        defs.push(CardDef::new("Demand","{0}","Sorcery","As an additional cost to cast this spell, blight 1.\nYou gain 1 life.",None).unwrap());
        let mut g=Game::new(defs,2,20);g.put(0,SEASON,Zone::Battlefield);
        if replacement {g.put(0,DOC,Zone::Battlefield);}
        let bear=g.put(0,TOUGH,Zone::Battlefield);g.put(0,demand,Zone::Library);
        let card=g.put(0,finder,Zone::Hand);g.act(0,Move::Cast{card,targets:vec![]}).unwrap();resolve(&mut g);
        let Some(Decision::Choose{choice:Choice::Cast{cards},..})=g.decision.clone() else{panic!("discover cast question")};
        g.act(0,Move::WithCosts(vec![vec![bear]],Box::new(Move::Cast{card:cards[0],targets:vec![]}))).unwrap();
        cite!(["608.2g","601.2h","614.16"];assert_eq!(g.obj(bear).counters.get("-1/-1"),Some(&if replacement{4}else{1})));
        assert_eq!(g.stack.len(),1);assert_eq!(g.obj(g.stack[0]).chars.name,"Demand");
        resolve(&mut g);assert_eq!(g.players[0].life,21);
    }
}

#[test]
fn r122_6_native_class_generic_modifier_matches_players_as_well_as_permanents() {
    let mut defs=setup().defs.to_vec();let poison=defs.len();
    defs.push(CardDef::new("Poison","{0}","Sorcery","Target player gets two poison counters.",None).unwrap());
    let mut g=Game::new(defs,2,20);let talent=g.put(0,TALENT,Zone::Battlefield);level_three(&mut g,talent);
    let card=g.put(0,poison,Zone::Hand);g.act(0,Move::Cast{card,targets:vec![vec![Target::Player(1)]]}).unwrap();resolve(&mut g);
    cite!(["122.6","614.1a"];assert_eq!(g.counters(Target::Player(1)).get("poison"),Some(&4)));
    assert!(g.counters(Target::Player(0)).is_empty());
}

