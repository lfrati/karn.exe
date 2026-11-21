//! Combined object dealers retain target groups and bind amounts to individual sources.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use std::sync::OnceLock;
fn native(name: &str) -> CardDef {
    static FACES: OnceLock<Vec<oracle::corpus::Face>> = OnceLock::new();
    let faces = FACES.get_or_init(|| oracle::corpus::load(&oracle::corpus::default_path()).unwrap());
    CardDef::from_faces(&CardDef::catalog(faces)[name]).unwrap_or_else(|e| panic!("{name}: {e}"))
}
fn card(name: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    CardDef::new(name, "{0}", types, text, pt).unwrap_or_else(|e| panic!("{name}: {e}"))
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
fn aura() -> CardDef { card("Aura", "Enchantment — Aura", "Enchant creature", None) }
// Rules: 601.2c, 115.3, 608.2b, 608.2h, 120.1
// Graceful Takedown: losing the
// power-three dealer's Aura excludes only that dealer, so the remaining 2+4 is six.
#[test]
fn r608_2b_native_graceful_takedown_preserves_target_groups_and_each_dealers_power() {
    let mut g=game(vec![native("Graceful Takedown"),aura(),card("Two","Creature","",Some((2,3))),card("Three","Creature","",Some((3,4))),card("Four","Creature","",Some((4,5))),card("Wall","Creature","",Some((1,20)))]);
    let a=g.put(0,2,Zone::Battlefield);let b=g.put(0,3,Zone::Battlefield);let c=g.put(0,4,Zone::Battlefield);let wall=g.put(1,5,Zone::Battlefield);
    let x=g.put(1,1,Zone::Battlefield);let y=g.put(0,1,Zone::Battlefield);g.attach(x,Target::Object(a));g.attach(y,Target::Object(b));
    g.players[0].pool.colors[ir::Color::Green as usize]=2;let spell=g.put(0,0,Zone::Hand);
    let before=format!("{g:?}");
    cite!(["601.2c","115.3"]; refused("another target must differ from the preceding targets"): g.act(0,paid("GG",Move::Cast{card:spell,targets:vec![vec![Target::Object(a),Target::Object(b)],vec![Target::Object(a)],vec![Target::Object(wall)]]})));
    assert_eq!(format!("{g:?}"),before);
    g.act(0,paid("GG",Move::Cast{card:spell,targets:vec![vec![Target::Object(a),Target::Object(b)],vec![Target::Object(c)],vec![Target::Object(wall)]]})).unwrap();
    let mut partial=g.clone();partial.move_object(y,Zone::Graveyard);settle(&mut partial);
    cite!(["608.2b","608.2h","120.1"];assert_eq!(partial.obj(wall).damage,6));
    settle(&mut g);
    let dealt:Vec<_>=g.log.iter().filter_map(|e|match e {game::Event::DamageDealt{source,to,amount,..}=>Some((*source,*to,*amount)),_=>None}).collect();
    cite!(["608.2h","120.1"];assert_eq!(dealt,vec![(a,Target::Object(wall),2),(b,Target::Object(wall),3),(c,Target::Object(wall),4)]));
}

fn position() -> (Game, ObjId, ObjId, ObjId, ObjId, ObjId, ObjId) {
    let mut g = game(vec![native("Graceful Takedown"), aura(),
        card("Two", "Creature", "", Some((2, 3))),
        card("Three", "Creature", "", Some((3, 4))),
        card("Four", "Creature", "", Some((4, 5))),
        card("Wall", "Creature", "", Some((1, 20))),
        card("Steal", "Instant", "Gain control of target creature.", None)]);
    let a = g.put(0, 2, Zone::Battlefield);
    let b = g.put(0, 3, Zone::Battlefield);
    let c = g.put(0, 4, Zone::Battlefield);
    let wall = g.put(1, 5, Zone::Battlefield);
    let x = g.put(1, 1, Zone::Battlefield);
    let y = g.put(0, 1, Zone::Battlefield);
    g.attach(x, a.into()); g.attach(y, b.into());
    g.players[0].pool.colors[ir::Color::Green as usize] = 2;
    (g, a, b, c, wall, x, y)
}
fn takedown(g: &mut Game, dealers: &[ObjId], other: Option<ObjId>, wall: ObjId) {
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, paid("GG", Move::Cast { card: spell, targets: vec![
        dealers.iter().map(|o| (*o).into()).collect(),
        other.into_iter().map(Target::Object).collect(), vec![wall.into()]] })).unwrap();
}
fn dealt(g: &Game) -> Vec<(ObjId, Target, u32)> {
    g.log.iter().filter_map(|e| match e {
        game::Event::DamageDealt { source, to, amount, .. } => Some((*source, *to, *amount)),
        _ => None,
    }).collect()
}
// Rules: 601.2c, 608.2b, 120.1
// Empty first/optional groups are legal; surviving sources alone deal damage.
#[test]
fn r608_2b_zero_one_and_multiple_surviving_dealers_and_optional_omission() {
    for (count, optional, expected) in [(0,false,0),(0,true,4),(1,false,2),(2,false,5),(2,true,9)] {
        let (mut g,a,b,c,wall,_,_) = position();
        takedown(&mut g, &[a,b][..count], optional.then_some(c), wall);
        settle(&mut g);
        cite!(["601.2c","608.2b","120.1"]; assert_eq!(g.obj(wall).damage,expected));
        cite!(["120.1"]; assert_eq!(dealt(&g).len(), count + usize::from(optional)));
    }
    for removed in [1,2] {
        let (mut g,a,b,_,wall,x,y) = position();
        takedown(&mut g,&[a,b],None,wall);
        g.move_object(y,Zone::Graveyard);
        if removed==2 { g.move_object(x,Zone::Graveyard); }
        settle(&mut g);
        cite!(["608.2b","120.1"]; assert_eq!(g.obj(wall).damage,if removed==1 {2}else{0}));
    }
}
// Rules: 608.2h, 122.1a, 120.3e
// Current powers are independently 4 and 3, not the group's final member power.
#[test]
fn r608_2h_changed_power_is_read_individually_at_resolution() {
    let (mut g,a,b,_,wall,_,_) = position();
    takedown(&mut g,&[a,b],None,wall);
    g.put_counters(a,"+1/+1",2);
    settle(&mut g);
    cite!(["608.2h","122.1a","120.3e"]; assert_eq!(dealt(&g),vec![(a,wall.into(),4),(b,wall.into(),3)]));
}
// Rules: 608.2b, 400.7, 303.4b
// Lost control and a new incarnation both invalidate the original dealer target.
#[test]
fn r608_2b_control_loss_and_departure_reentry_exclude_only_illegal_dealer() {
    for reentry in [false,true] {
        let (mut g,a,b,_,wall,_,_) = position();
        takedown(&mut g,&[a,b],None,wall);
        if reentry {
            g.move_object(b,Zone::Graveyard);
            let new = *g.players[0].graveyard.last().unwrap();
            g.move_object(new,Zone::Battlefield);
        } else {
            g.act(0,Move::Pass).unwrap();
            let steal=g.put(1,6,Zone::Hand);
            g.act(1,Move::Cast {card:steal,targets:vec![vec![b.into()]]}).unwrap();
            pass(&mut g);
        }
        settle(&mut g);
        cite!(["608.2b","400.7"]; assert_eq!(dealt(&g),vec![(a,wall.into(),2)]));
    }
}
// Rules: 608.2b, 120.1
// Legal dealers cannot deal damage to an illegal recipient.
#[test]
fn r608_2b_illegal_recipient_prevents_all_hits() {
    let (mut g,a,b,c,wall,_,_) = position();
    takedown(&mut g,&[a,b],Some(c),wall);
    g.move_object(wall,Zone::Graveyard);
    settle(&mut g);
    cite!(["608.2b","120.1"]; assert!(dealt(&g).is_empty()));
}
// Rules: 608.2f, 120.4, 120.10, 603.2c, 700.1
// Two three-power sources simultaneously exceed toughness five. Serial hits
// cannot produce the Galleon's one excess occurrence. History still has two hits.
#[test]
fn r120_4_simultaneous_combined_damage_has_one_excess_occurrence() {
    let mut g=game(vec![native("Graceful Takedown"),aura(),native("Magmatic Galleon"),
        card("Three","Creature","",Some((3,4))),
        card("Wall","Creature","Indestructible",Some((1,5)))]);
    g.put(0,2,Zone::Battlefield);
    let a=g.put(0,3,Zone::Battlefield);let b=g.put(0,3,Zone::Battlefield);
    let wall=g.put(1,4,Zone::Battlefield);let aura=g.put(0,1,Zone::Battlefield);g.attach(aura,a.into());
    g.players[0].pool.colors[ir::Color::Green as usize]=2;
    takedown(&mut g,&[a],Some(b),wall);pass(&mut g);
    cite!(["608.2f","120.4","120.10","603.2c"]; assert_eq!(g.stack.len(),1));
    cite!(["120.1","700.1"]; assert_eq!(dealt(&g),vec![(a,wall.into(),3),(b,wall.into(),3)]));
    settle(&mut g);
    cite!(["603.2c"]; assert_eq!(g.battlefield.iter().filter(|o|g.obj(**o).chars.name=="Treasure Token").count(),1));
}
// Rules: 615.6, 702.2b, 120.3f, 603.2
// Prevention applies only to the power-three source, preserving the other source's identity,
// deathtouch and lifelink. An ordinary source damage trigger sees one actual hit.
#[test]
fn r615_6_per_source_prevention_deathtouch_lifelink_and_damage_trigger() {
    let mut g=game(vec![native("Graceful Takedown"),aura(),
        card("Poisoner","Creature","Deathtouch\nLifelink\nWhenever this creature deals damage to a creature, you gain 1 life.",Some((2,3))),
        card("Three","Creature","",Some((3,4))),
        card("Wall","Creature","",Some((1,20))),
        card("Source Ward","Instant","Prevent all damage that would be dealt this turn by creatures with power 3 or greater.",None)]);
    let a=g.put(0,2,Zone::Battlefield);let b=g.put(0,3,Zone::Battlefield);let wall=g.put(1,4,Zone::Battlefield);
    let aura=g.put(0,1,Zone::Battlefield);g.attach(aura,a.into());
    g.players[0].pool.colors[ir::Color::Green as usize]=2;
    takedown(&mut g,&[a],Some(b),wall);
    let prevent=g.put(0,5,Zone::Hand);
    g.act(0,Move::Cast{card:prevent,targets:vec![]}).unwrap();pass(&mut g);pass(&mut g);
    cite!(["615.6","120.1"]; assert_eq!(dealt(&g),vec![(a,wall.into(),2)]));
    cite!(["702.2b","704.5h"]; assert!(!g.battlefield.contains(&wall)));
    cite!(["120.3f","603.2"]; assert_eq!((g.players[0].life,g.stack.len()),(22,1)));
    settle(&mut g);
    cite!(["603.2"]; assert_eq!(g.players[0].life,23));
}

// Rules: 601.2c, 608.2h, 120.1, 608.2f
// Friendly Rivalry has the same object union but restricts its optional dealer to
// another legendary creature. This neighbouring native shape uses the same binding.
#[test]
fn r608_2h_native_friendly_rivalry_uses_each_legends_own_power() {
    let mut g=game(vec![native("Friendly Rivalry"),
        card("Small","Creature","",Some((2,3))),
        card("Legend","Legendary Creature","",Some((3,4))),
        card("Wall","Creature","",Some((1,20)))]);
    let a=g.put(0,1,Zone::Battlefield);let b=g.put(0,2,Zone::Battlefield);let wall=g.put(1,3,Zone::Battlefield);
    g.players[0].pool.colors=[2;5];
    let spell=g.put(0,0,Zone::Hand);
    g.act(0,paid("RG",Move::Cast{card:spell,targets:vec![vec![a.into()],vec![b.into()],vec![wall.into()]]})).unwrap();
    settle(&mut g);
    cite!(["608.2h","120.1","608.2f"]; assert_eq!(dealt(&g),vec![(a,wall.into(),2),(b,wall.into(),3)]));
}
#[test]
fn combined_source_shapes_outside_demonstrated_contract_remain_refused() {
    // Uncited (admission): engine boundaries, not game legality claims.
    for (recipient,expected) in [
        ("each other creature","distributive damage: per-source other recipient set not implemented"),
        ("target player","per-source object quantity in damage a player may be dealt"),
    ] {
        let text=format!("Any number of target enchanted creatures you control and up to one other target creature you control each deal damage equal to their power to {recipient}.");
        let d=card("Probe","Sorcery",&text,None);
        assert_eq!(game::play::check(&d).unwrap_err().0,expected);
    }
}

// Rules: 608.2f, 120.1, 603.2, 603.2c
// Each actual dealer has its own ordinary damage trigger. Simultaneous damage
// retains two source occurrences, with no additional occurrence for their sum.
#[test]
fn r603_2_simultaneous_dealers_each_trigger_once_without_duplicate_history() {
    let trigger="Whenever this creature deals damage to a creature, you gain 1 life.";
    let mut g=game(vec![native("Graceful Takedown"),aura(),
        card("Two","Creature",trigger,Some((2,3))),
        card("Three","Creature",trigger,Some((3,4))),
        card("Wall","Creature","",Some((1,20)))]);
    let a=g.put(0,2,Zone::Battlefield);let b=g.put(0,3,Zone::Battlefield);let wall=g.put(1,4,Zone::Battlefield);
    let aura=g.put(0,1,Zone::Battlefield);g.attach(aura,a.into());
    g.players[0].pool.colors[ir::Color::Green as usize]=2;
    takedown(&mut g,&[a],Some(b),wall);pass(&mut g);
    // Both triggers belong to p0: CR603.3b requires ordering before they stack.
    if matches!(g.decision,Some(game::state::Decision::Order {..})) {
        g.act(0,Move::Order(vec![0,1])).unwrap();
    }
    cite!(["120.1","608.2f"]; assert_eq!(dealt(&g),vec![(a,wall.into(),2),(b,wall.into(),3)]));
    cite!(["603.2","603.2c"]; assert_eq!(g.stack.len(),2));
    settle(&mut g);
    cite!(["603.2","603.2c"]; assert_eq!(g.players[0].life,22));
}
