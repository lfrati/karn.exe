//! Inverse Equipment/Aura descriptions: relation, scope, changing source types and lookback.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use std::sync::OnceLock;
fn native(name: &str) -> CardDef {
    static FACES: OnceLock<Vec<oracle::corpus::Face>> = OnceLock::new();
    let faces = FACES.get_or_init(|| oracle::corpus::load(&oracle::corpus::default_path()).unwrap());
    CardDef::from_faces(&CardDef::catalog(faces)[name]).unwrap_or_else(|e| panic!("{name}: {e}"))
}
fn card(name: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    CardDef::new(name, "{0}", types, text, pt).unwrap()
}
fn game(defs: Vec<CardDef>) -> Game {
    for d in &defs {
        game::play::check(d).unwrap_or_else(|e| panic!("{}: {e:?}", d.name()));
    }
    Game::new(defs, 2, 20)
}
fn pass(g: &mut Game) {
    for _ in 0..2 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn settle(g: &mut Game) {
    for _ in 0..20 {
        if g.stack.is_empty() {
            return;
        }
        pass(g);
    }
    panic!("unsettled");
}
fn cast(g: &mut Game, d: usize, targets: Vec<Vec<Target>>) -> ObjId {
    let c = g.put(0, d, Zone::Hand);
    g.act(0, Move::Cast { card: c, targets }).unwrap();
    c
}
fn bear() -> CardDef { card("Bear", "Creature — Bear", "", Some((2,2))) }
fn equipment() -> CardDef { card("Harness", "Artifact — Equipment", "Equip {0}", None) }
fn aura() -> CardDef { card("Aura", "Enchantment — Aura", "Enchant creature", None) }
fn count(kind: &str) -> CardDef { card("Count", "Sorcery", &format!("You gain life equal to the number of {kind} creatures you control."), None) }
fn gain(g: &mut Game, index: usize) { cast(g,index,vec![]); settle(g); }

#[test]
fn r301_5a_inverse_equipment_counts_recipients_once_and_ignores_controller() {
    let mut g=game(vec![count("equipped"),bear(),equipment(),aura()]);
    let own=g.put(0,1,Zone::Battlefield); let enemy=g.put(1,1,Zone::Battlefield);
    gain(&mut g,0); cite!(["301.5a"]; assert_eq!(g.players[0].life,20));
    let a=g.put(1,2,Zone::Battlefield); let b=g.put(0,2,Zone::Battlefield);
    let unused=g.put(0,2,Zone::Battlefield); let enchant=g.put(1,3,Zone::Battlefield);
    assert!(g.attach(a,Target::Object(own))); assert!(g.attach(b,Target::Object(own)));
    assert!(g.attach(enchant,Target::Object(enemy)));
    gain(&mut g,0); cite!(["301.5a","301.5d","109.5"]; assert_eq!(g.players[0].life,21));
    assert_eq!(g.attached_to(unused),None);
    g.attach(a,Target::Object(enemy)); g.attach(b,Target::Object(enemy));
    gain(&mut g,0); cite!(["701.3a","301.5a","109.5"]; assert_eq!(g.players[0].life,21));
}
#[test]
fn r303_4b_native_tale_tracks_reattachment_multiple_auras_and_opponent_control() {
    let mut g=game(vec![native("A Tale for the Ages"),bear(),aura(),equipment()]);
    g.put(0,0,Zone::Battlefield);
    let a=g.put(0,1,Zone::Battlefield); let b=g.put(0,1,Zone::Battlefield); let enemy=g.put(1,1,Zone::Battlefield);
    let x=g.put(1,2,Zone::Battlefield); let y=g.put(0,2,Zone::Battlefield); let eq=g.put(0,3,Zone::Battlefield);
    assert_eq!(g.power(a),Some(2)); g.attach(eq,Target::Object(a));
    cite!(["303.4b","301.5a"]; assert_eq!(g.power(a),Some(2)));
    g.attach(x,Target::Object(a)); g.attach(y,Target::Object(a));
    cite!(["303.4b","303.4e","611.3a"]; assert_eq!(g.seen(a,None).pt,Some((4,4))));
    g.attach(x,Target::Object(b)); g.attach(y,Target::Object(enemy));
    cite!(["701.3a","109.5","611.3a"]; assert_eq!((g.power(a),g.power(b),g.power(enemy)),(Some(2),Some(4),Some(2))));
}
#[test]
fn r301_5c_source_type_loss_changes_inverse_description_before_sba() {
    // The type layer removes Equipment's artifact subtype; PT must already stop applying.
    let reshape=card("Reshape","Enchantment","All artifacts are enchantments.",None);
    let buff=card("Buff","Enchantment","Equipped creatures you control get +3/+3.",None);
    let silence=card("Silence","Enchantment","All artifacts lose all abilities.",None);
    let mut g=game(vec![bear(),equipment(),buff,reshape,silence]);
    let b=g.put(0,0,Zone::Battlefield);let e=g.put(1,1,Zone::Battlefield);g.put(0,2,Zone::Battlefield);g.attach(e,Target::Object(b));
    g.put(0,4,Zone::Battlefield);
    cite!(["301.5a","613.1f"]; assert_eq!(g.power(b),Some(5)));
    g.put(0,3,Zone::Battlefield);
    cite!(["205.1a","301.5c","613.1d","613.1g"]; assert_eq!(g.power(b),Some(2)));
    // A read cannot eagerly detach the edge: the next ordinary SBA owns that mutation.
    assert_eq!(g.attached_to(e),Some(Target::Object(b)));
}
#[test]
fn r303_4b_source_aura_type_loss_changes_pt_in_partial_layer_world() {
    let reshape=card("Reshape","Enchantment","All enchantments are artifacts.",None);
    let buff=card("Buff","Creature","Enchanted creatures you control get +3/+3.",Some((2,2)));
    let mut g=game(vec![bear(),aura(),buff,reshape]);let b=g.put(0,0,Zone::Battlefield);let a=g.put(1,1,Zone::Battlefield);
    g.put(0,2,Zone::Battlefield);g.attach(a,Target::Object(b));assert_eq!(g.power(b),Some(5));
    g.put(0,3,Zone::Battlefield);
    cite!(["205.1a","303.4b","613.1d","613.1g"]; assert_eq!(g.power(b),Some(2)));
    assert_eq!(g.attached_to(a),Some(Target::Object(b)));
}
#[test]
fn r301_5a_new_incarnation_does_not_inherit_equipped_relation() {
    let mut g=game(vec![bear(),equipment(),count("equipped")]);let b=g.put(0,0,Zone::Battlefield);let e=g.put(0,1,Zone::Battlefield);g.attach(e,Target::Object(b));
    g.move_object(b,Zone::Graveyard);let card=*g.players[0].graveyard.last().unwrap();g.move_object(card,Zone::Battlefield);
    gain(&mut g,2);cite!(["400.7","301.5a","301.5c"]; assert_eq!(g.players[0].life,20));
}
#[test]
fn r603_10a_simultaneous_source_recipient_and_watcher_departure_freezes_edges() {
    for kind in ["equipped","enchanted"] {
        let watch=card("Watch","Enchantment",&format!("Whenever an {kind} creature you control dies, you gain 1 life."),None);
        let sweep=card("Sweep","Sorcery","Destroy all nonland permanents.",None);
        let source=if kind=="equipped" {equipment()} else {aura()};
        let mut g=game(vec![watch,bear(),source,sweep]);let watcher=g.put(0,0,Zone::Battlefield);let b=g.put(0,1,Zone::Battlefield);let a=g.put(1,2,Zone::Battlefield);g.attach(a,Target::Object(b));
        let unused=g.put(0,1,Zone::Battlefield);assert_ne!(b,unused);
        cast(&mut g,3,vec![]);pass(&mut g);
        // CR404.3: the owner explicitly orders these simultaneous graveyard arrivals.
        assert_eq!(g.decision, Some(game::state::Decision::Choose { player: 0, choice: game::state::Choice::OrderCards { to: game::state::Placement::Zone(Zone::Graveyard), cards: vec![watcher,b,unused] } }));
        g.act(0, Move::Choose(game::state::Answer::Objects(vec![unused,b,watcher]))).unwrap();
        cite!(["603.10a","301.5a","303.4b","603.2c"]; assert_eq!(g.stack.len(),1));
        settle(&mut g);cite!(["603.10a","608.2h"]; assert_eq!(g.players[0].life,21));
    }
}
#[test]
fn r603_10a_creature_sba_death_precedes_its_auras_later_sba_departure() {
    let watch=card("Watch","Enchantment","Whenever an enchanted creature you control dies, you gain 1 life.",None);
    let sweep=card("Sweep","Sorcery","All creatures get -2/-2 until end of turn.",None);
    let mut g=game(vec![watch,bear(),aura(),sweep]);g.put(0,0,Zone::Battlefield);let b=g.put(0,1,Zone::Battlefield);let a=g.put(1,2,Zone::Battlefield);g.attach(a,Target::Object(b));
    cast(&mut g,3,vec![]);pass(&mut g);settle(&mut g);
    cite!(["704.5f","603.10a","303.4b"]; assert_eq!(g.players[0].life,21));
}
#[test]
fn native_fifteen_consumer_admission_matrix() {
    // Uncited (admission): whole native definitions, including all independent blockers.
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog=CardDef::catalog(&faces);
    for (name, expected) in [
        // Its coordinated X is now bound; counting Equipment attached to the
        // source still needs the unsupported AttachedTo description.
        ("Whiplash, Vengeful Engineer", Some("engine: description: AttachedTo")),
        ("Archon of the Wild Rose", Some("grammar: text the grammar does not read")),
        ("Swordsman, Sharp Scoundrel", None),
        ("Cloud, Midgar Mercenary", Some("grammar: text the grammar does not read")),
        ("Syr Armont, the Redeemer", None),
        ("A Tale for the Ages", None),
        ("Item Shopkeep", None),
        ("Cloud, Planet's Champion", Some("engine: activation modifier: unsupported ability group")),
        ("Barret Wallace", None),
        ("Dáin Ironfoot", None),
        ("Blacksmith's Talent", None),
        ("Graceful Takedown", None),
        ("Lord Skitter's Blessing", Some("engine: program: action not lowered: DrawAdditional")),
        ("Firion, Wild Rose Warrior", Some("engine: copy exception ability")),
        ("Eriette of the Charmed Apple", Some("grammar: text the grammar does not read")),
    ] {
        let result=CardDef::from_faces(&catalog[name]).map_err(|e| format!("grammar: {e}"))
            .and_then(|d|game::play::check(&d).map_err(|e|format!("engine: {}",e.0)));
        assert_eq!(result.as_ref().err().map(String::as_str), expected, "{name}");
        println!("CONSUMER\t{name}\t{result:?}");
    }
}

#[test]
fn r301_5a_negation_complements_relation_within_creature_and_controller_scope() {
    let buff=card("Buff","Enchantment","Creatures you control that aren't equipped get +3/+3.",None);
    let mut g=game(vec![bear(),equipment(),buff]);g.put(0,2,Zone::Battlefield);let a=g.put(0,0,Zone::Battlefield);let b=g.put(0,0,Zone::Battlefield);let enemy=g.put(1,0,Zone::Battlefield);let e=g.put(1,1,Zone::Battlefield);g.attach(e,Target::Object(a));
    cite!(["301.5a","301.5d","109.5"];assert_eq!((g.power(a),g.power(b),g.power(enemy)),(Some(2),Some(5),Some(2))));
}
#[test]
fn r303_4e_creature_control_change_keeps_opponent_aura_relation() {
    let steal=card("Steal","Sorcery","Gain control of target creature.",None);
    let mut g=game(vec![bear(),aura(),native("A Tale for the Ages"),steal]);g.put(0,2,Zone::Battlefield);let b=g.put(1,0,Zone::Battlefield);let a=g.put(1,1,Zone::Battlefield);g.attach(a,Target::Object(b));assert_eq!(g.power(b),Some(2));
    cast(&mut g,3,vec![vec![Target::Object(b)]]);settle(&mut g);
    cite!(["303.4e","611.3a","109.5"];assert_eq!((g.controller(b),g.controller(a),g.power(b)),(0,1,Some(4))));
}

fn declare(g:&mut Game, attackers:&[ObjId]) {
    while g.decision.is_none() {g.act(g.priority,Move::Pass).unwrap();}
    g.act(0,Move::Attack(attackers.iter().map(|o|(*o,Target::Player(1))).collect())).unwrap();
}
#[test]
fn r608_2h_native_barret_counts_current_equipped_recipients_once() {
    let mut g=game(vec![native("Barret Wallace"),bear(),equipment()]);let barret=g.put(0,0,Zone::Battlefield);let b=g.put(0,1,Zone::Battlefield);let _enemy=g.put(1,1,Zone::Battlefield);
    let a=g.put(1,2,Zone::Battlefield);let e=g.put(0,2,Zone::Battlefield);g.attach(a,Target::Object(barret));g.attach(e,Target::Object(barret));
    declare(&mut g,&[barret]);assert_eq!(g.stack.len(),1);
    g.attach(e,Target::Object(b));pass(&mut g);
    cite!(["608.2h","301.5a","301.5d"];assert_eq!(g.players[1].life,18));
}
#[test]
fn r608_2b_native_shopkeep_rechecks_equipped_target_then_retains_temporary_menace() {
    let mut g=game(vec![native("Item Shopkeep"),bear(),equipment()]);g.put(0,0,Zone::Battlefield);let b=g.put(0,1,Zone::Battlefield);let spare=g.put(0,1,Zone::Battlefield);let e=g.put(1,2,Zone::Battlefield);g.attach(e,Target::Object(b));
    declare(&mut g,&[b]);g.act(0,Move::ChooseTargets(vec![vec![Target::Object(b)]])).unwrap();
    let mut illegal=g.clone();illegal.attach(e,Target::Object(spare));pass(&mut illegal);
    let menace=ir::Keyword::from_name("Menace").unwrap();
    cite!(["608.2b","301.5a"];assert!(!illegal.seen(b,None).has(menace)));
    pass(&mut g);assert!(g.seen(b,None).has(menace));g.attach(e,Target::Object(spare));
    cite!(["611.2c","611.2a"];assert!(g.seen(b,None).has(menace)));
}
#[test]
fn native_graceful_takedown_admits_combined_member_quantity() {
    // Uncited (admission): repository limitation, not a Magic legality restriction.
    let def=native("Graceful Takedown");
    game::play::check(&def).unwrap();
}

// Rules: 603.10a, 603.4, 301.5a, 303.4b, 608.2h
// Both endpoints leave together. The intervening condition at resolution uses the source's
// pre-departure relationship, even though neither attachment survives on the battlefield.
#[test]
fn r608_2h_departed_source_inverse_condition_requires_last_known_attachments() {
    for kind in ["equipped", "enchanted"] {
        let source=card("Source","Creature",&format!("When this creature dies, if this creature is {kind}, you gain 1 life."),Some((2,2)));
        let sweep=card("Sweep","Sorcery","Destroy all nonland permanents.",None);
        let attachment=if kind=="equipped" {equipment()} else {aura()};
        let mut g=game(vec![source,attachment,sweep]);let b=g.put(0,0,Zone::Battlefield);let a=g.put(1,1,Zone::Battlefield);g.attach(a,Target::Object(b));cast(&mut g,2,vec![]);pass(&mut g);
        cite!(["603.10a","603.4","301.5a","303.4b"];assert_eq!(g.stack.len(),1));settle(&mut g);
        cite!(["603.4","608.2h","301.5a","303.4b"];assert_eq!(g.players[0].life,21));
    }
}

#[test]
fn r608_2h_departed_trigger_subject_condition_requires_inverse_lki() {
    let watch=card("Watch","Enchantment","Whenever an enchanted creature you control dies, if that creature is enchanted, you gain 1 life.",None);
    let sweep=card("Sweep","Sorcery","Destroy all creatures and all Auras.",None);
    let mut g=game(vec![watch,bear(),aura(),sweep]);g.put(0,0,Zone::Battlefield);let b=g.put(0,1,Zone::Battlefield);let a=g.put(1,2,Zone::Battlefield);g.attach(a,Target::Object(b));cast(&mut g,3,vec![]);pass(&mut g);
    cite!(["603.10a","603.4","303.4b"];assert_eq!(g.stack.len(),1));settle(&mut g);
    cite!(["603.4","608.2h","303.4b"];assert_eq!(g.players[0].life,21));
}

// Rules: 301.5a, 303.4b, 603.10a, 603.4, 608.2h
// An attachment that left before its recipient died is absent from both the trigger
// lookback and the later last-known condition; a past edge must not be resurrected.
#[test]
fn r603_10a_attachment_departed_earlier_does_not_qualify_later_death() {
    for kind in ["equipped", "enchanted"] {
        let watch=card("Watch","Enchantment",&format!("Whenever an {kind} creature you control dies, you gain 1 life."),None);
        let attachment=if kind=="equipped" {equipment()} else {aura()};
        let kill=card("Kill","Sorcery","Destroy target nonland permanent.",None);
        let mut g=game(vec![watch,bear(),attachment,kill]);g.put(0,0,Zone::Battlefield);
        let b=g.put(0,1,Zone::Battlefield);let a=g.put(1,2,Zone::Battlefield);g.attach(a,Target::Object(b));
        cast(&mut g,3,vec![vec![Target::Object(a)]]);settle(&mut g);
        cast(&mut g,3,vec![vec![Target::Object(b)]]);pass(&mut g);
        cite!(["603.10a","301.5a","303.4b"];assert!(g.stack.is_empty()));
        cite!(["603.10a","301.5a","303.4b"];assert_eq!(g.players[0].life,20));
    }
}

// Rules: 608.2h, 301.5a, 303.4b, 113.7a
// A live ability source reads its current edge; a source that departs in response reads
// its own last-known edge. Reattachment after departure cannot rewrite that information.
#[test]
fn r608_2h_activated_source_uses_current_or_departure_inverse_relation() {
    for kind in ["equipped", "enchanted"] {
        let source=card("Source","Creature",&format!("{{0}}: If this creature is {kind}, you gain 1 life."),Some((2,2)));
        let attachment=if kind=="equipped" {equipment()} else {aura()};
        let kill=card("Kill","Instant","Destroy target creature.",None);
        let mut g=game(vec![source,attachment,bear(),kill]);let b=g.put(0,0,Zone::Battlefield);let a=g.put(1,1,Zone::Battlefield);let spare=g.put(0,2,Zone::Battlefield);g.attach(a,Target::Object(b));
        g.act(0,Move::Activate{source:b,ability:0,targets:vec![],paying:vec![]}).unwrap();
        let mut live=g.clone();live.attach(a,Target::Object(spare));pass(&mut live);
        cite!(["608.2h","301.5a","303.4b"];assert_eq!(live.players[0].life,20));
        cast(&mut g,3,vec![vec![Target::Object(b)]]);pass(&mut g);
        // Equipment survives; an Aura is put in the graveyard by the next SBA.
        if kind=="equipped" {g.attach(a,Target::Object(spare));}
        settle(&mut g);
        cite!(["608.2h","113.7a","301.5a","303.4b"];assert_eq!(g.players[0].life,21));
    }
}

// Rules: 303.4b, 303.4e, 611.3a
// Armont's static clause counts enchanted recipients regardless of Aura controller.
#[test]
fn r303_4b_native_armont_static_clause_tracks_opponent_aura() {
    let mut g=game(vec![native("Syr Armont, the Redeemer"),bear(),aura()]);
    g.put(0,0,Zone::Battlefield);let b=g.put(0,1,Zone::Battlefield);let a=g.put(1,2,Zone::Battlefield);
    cite!(["611.3a","303.4b"];assert_eq!(g.power(b),Some(2)));
    g.attach(a,Target::Object(b));
    cite!(["611.3a","303.4b","303.4e"];assert_eq!(g.seen(b,None).pt,Some((3,3))));
}

// Rules: 301.5a, 608.2h, 611.2c, 702.4b
// Dain grants only to creatures that are BOTH attacking and equipped at resolution.
#[test]
fn r608_2h_native_dain_selects_current_equipped_attackers() {
    let mut g=game(vec![native("Dáin Ironfoot"),bear(),equipment()]);
    let d=g.put(0,0,Zone::Battlefield);let b=g.put(0,1,Zone::Battlefield);let idle=g.put(0,1,Zone::Battlefield);
    let a=g.put(1,2,Zone::Battlefield);let other=g.put(0,2,Zone::Battlefield);
    g.attach(a,Target::Object(d));g.attach(other,Target::Object(idle));
    declare(&mut g,&[d,b]);g.attach(a,Target::Object(b));pass(&mut g);
    let double=ir::Keyword::from_name("Double strike").unwrap();
    cite!(["608.2h","301.5a","702.4b"];assert!(g.seen(b,None).has(double)));
    cite!(["608.2h","301.5a"];assert!(!g.seen(d,None).has(double) && !g.seen(idle,None).has(double)));
    g.attach(a,Target::Object(d));
    cite!(["611.2c"];assert!(g.seen(b,None).has(double)));
}

// Rules: 716.2a, 301.5a, 611.3a, 611.3b
// Native level-3 continuous grants follow Equipment immediately after reattachment.
#[test]
fn r301_5a_native_blacksmith_level_three_tracks_inverse_relation() {
    let mut g=game(vec![native("Blacksmith's Talent"),bear(),equipment()]);
    let class=g.put(0,0,Zone::Battlefield);let b=g.put(0,1,Zone::Battlefield);let spare=g.put(0,1,Zone::Battlefield);let e=g.put(1,2,Zone::Battlefield);g.attach(e,Target::Object(b));
    g.players[0].pool.colors=[10;5];g.players[0].pool.colorless=10;
    let bars:Vec<_>=g.abilities(class).iter().enumerate().filter(|(_,a)|matches!(&***a,ir::Ability::Activated{..})).map(|(i,_)|i).collect();
    let [level_two, level_three] = bars[..] else { panic!("two printed Class level bars") };
    for (index, units) in [(level_two, "CCR"), (level_three, "CCCR")] {g.act(0,paid(units,Move::Activate{source:class,ability:index,targets:vec![],paying:vec![]})).unwrap();settle(&mut g);}
    cite!(["716.2a"];assert_eq!(g.obj(class).level,Some(3)));
    for name in ["Double strike","Haste"] {
        let keyword=ir::Keyword::from_name(name).unwrap();
        cite!(["301.5a","611.3a"];assert!(g.seen(b,None).has(keyword) && !g.seen(spare,None).has(keyword)));
        g.attach(e,Target::Object(spare));
        cite!(["301.5a","611.3a","611.3b"];assert!(!g.seen(b,None).has(keyword) && g.seen(spare,None).has(keyword)));
        g.attach(e,Target::Object(b));
    }
}

// Rules: 301.5a, 603.2, 701.50a, 701.50d
// Native Swordsman triggers once for an equipped attacker, even with two Equipment,
// and not for the unenchanted/unequipped attacker. Connive draws, asks, then discards.
#[test]
fn r301_5a_native_swordsman_equipped_attack_connives_once() {
    let drawn=card("Drawn","Sorcery","You gain 1 life.",None);
    let mut g=game(vec![native("Swordsman, Sharp Scoundrel"),bear(),equipment(),drawn]);
    g.put(0,0,Zone::Battlefield);let b=g.put(0,1,Zone::Battlefield);let other=g.put(0,1,Zone::Battlefield);
    for player in [0,1] {let e=g.put(player,2,Zone::Battlefield);g.attach(e,Target::Object(b));}
    g.put(0,2,Zone::Hand);g.put(0,3,Zone::Library);declare(&mut g,&[b,other]);
    cite!(["301.5a","603.2"];assert_eq!(g.stack.len(),1));pass(&mut g);
    cite!(["701.50a"];assert!(matches!(g.decision,Some(game::state::Decision::Choose{player:0,..}))));
    let drawn=*g.players[0].hand.iter().find(|o|g.obj(**o).chars.name=="Drawn").unwrap();
    cite!(["701.50a"];assert_eq!(g.obj(drawn).chars.name,"Drawn"));
    g.act(0,Move::Choose(game::state::Answer::Objects(vec![drawn]))).unwrap();
    cite!(["701.50a"];assert_eq!(g.obj(b).counters.get("+1/+1"),Some(&1)));
    cite!(["701.50a"];assert!(g.obj(other).counters.is_empty()));
    cite!(["701.50a"];assert_eq!(g.players[0].graveyard.len(),1));
}
