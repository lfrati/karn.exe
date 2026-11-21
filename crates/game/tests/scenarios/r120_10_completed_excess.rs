//! 120.10/608.2i: quantities refer to completed actual damage, not live characteristics.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

fn pinned(name: &str) -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let card = CardDef::from_faces(&CardDef::catalog(&faces)[name]).unwrap();
    card
}
fn native(name: &str) -> CardDef {let card=pinned(name);game::play::check(&card).unwrap();card}
fn echo() -> CardDef {native("Violent Echoes")}
fn bear(toughness: i32) -> CardDef {
    CardDef::new("Bear", "{0}", "Creature", "", Some((2,toughness))).unwrap()
}
fn cast(g: &mut Game, target: ObjId, mana: &str) {
    g.players[0].pool.colors = [10;5];
    let card = g.put(0,0,Zone::Hand);
    g.act(0,paid(mana,Move::Cast {card,targets:vec![vec![Target::Object(target)]]})).unwrap();
    for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
}
fn jace(g: &Game) -> Vec<ObjId> {
    g.battlefield.iter().copied().filter(|on|g.obj(*on).chars.name=="Jace Token").collect()
}

#[test]
fn r120_10_native_echoes_uses_actual_excess_and_not_six() {
    for (toughness,expected) in [(2,4),(6,0),(8,0)] {
        let mut g=Game::new(vec![echo(),bear(toughness)],2,20);
        let target=g.put(1,1,Zone::Battlefield);cast(&mut g,target,"RRRR");
        cite!(["120.10","701.71a"];assert_eq!(jace(&g).len(),usize::from(expected>0)));
        if expected>0 {assert_eq!(g.counters(jace(&g)[0]).get("loyalty"),Some(&expected));}
    }
}
#[test]
fn r120_10_marked_damage_reduces_remaining_lethal_threshold() {
    let mut g=Game::new(vec![echo(),bear(5)],2,20);
    let target=g.put(1,1,Zone::Battlefield);g.damage(target,Target::Object(target),3);
    cast(&mut g,target,"RRRR");
    cite!(["120.10","120.6"];assert_eq!(g.counters(jace(&g)[0]).get("loyalty"),Some(&4)));
}
#[test]
fn r120_10_planeswalker_uses_before_damage_loyalty() {
    let walker=CardDef::planeswalker("Walker","{0}","Planeswalker — Jace","+1: You gain 1 life.",3).unwrap();
    let mut g=Game::new(vec![echo(),walker],2,20);
    let target=g.put(1,1,Zone::Battlefield);g.put_counters(target,"loyalty",3);
    cast(&mut g,target,"RRRR");
    cite!(["120.10","608.2i"];assert_eq!(g.counters(jace(&g)[0]).get("loyalty"),Some(&3)));
}

#[test]
fn r120_10_prevention_changes_actual_excess_and_complete_prevention_has_no_result() {
    for (prevented,expected) in [(3,1),(6,0)] {
        let shield=CardDef::new("Shield","{0}","Instant",&format!("Prevent the next {prevented} damage that would be dealt to target creature this turn."),None).unwrap();
        game::play::check(&shield).unwrap();
        let mut g=Game::new(vec![echo(),bear(2),shield],2,20);
        let target=g.put(1,1,Zone::Battlefield);let shield=g.put(0,2,Zone::Hand);
        g.act(0,Move::Cast {card:shield,targets:vec![vec![Target::Object(target)]]}).unwrap();
        for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
        cast(&mut g,target,"RRRR");
        cite!(["120.4b","120.8","120.10"];assert_eq!(jace(&g).len(),usize::from(expected>0)));
        if expected>0 {assert_eq!(g.counters(jace(&g)[0]).get("loyalty"),Some(&expected));}
        assert_eq!(g.log.iter().filter_map(|e|match e {game::Event::DamageDealt {amount,..}=>Some(*amount),_=>None}).collect::<Vec<_>>(),if prevented==6 {vec![]} else {vec![3]});
    }
}
#[test]
fn r608_2i_two_damage_producers_are_not_arbitrarily_disambiguated() {
    let spell=CardDef::new("Repeat","{0}","Instant","Repeat deals 3 damage to target creature. Repeat deals 3 damage to that creature. You gain life equal to the excess damage dealt to that creature this way.",None).unwrap();
    // Two eligible result producers are genuinely ambiguous; do not choose an older
    // or latest producer by convention. A future explicit occurrence binding can extend it.
    let reason=game::play::check(&spell).unwrap_err().0;
    cite!(["608.2c","608.2i"];assert!(reason.contains("ambiguous completed producers"),"{reason}"));
}

#[test]
fn r608_2i_completed_excess_survives_its_recipient_leaving_the_battlefield() {
    let spell=CardDef::new("Memory","{0}","Instant","Memory deals 6 damage to target creature. Exile that creature. You gain life equal to the excess damage dealt to that creature this way.",None).unwrap();
    game::play::check(&spell).unwrap();
    let mut g=Game::new(vec![spell,bear(3)],2,20);let target=g.put(1,1,Zone::Battlefield);cast(&mut g,target,"");
    cite!(["608.2i","120.10"];assert_eq!(g.players[0].life,23));
    assert!(!g.battlefield.contains(&target));
}

#[test]
fn r608_2i_skipped_damage_produces_empty_not_an_earlier_game_event() {
    let spell=CardDef::new("Maybe","{0}","Instant","If you control an artifact, Maybe deals 3 damage to target creature. You gain life equal to the excess damage dealt this way.",None).unwrap();
    game::play::check(&spell).unwrap();
    let wall=CardDef::new("Wall","{0}","Creature","Indestructible",Some((1,2))).unwrap();
    let mut g=Game::new(vec![spell,wall],2,20);let target=g.put(1,1,Zone::Battlefield);
    g.damage(target,target.into(),6);cast(&mut g,target,"");
    cite!(["608.2c","608.2i"];assert_eq!(g.players[0].life,20));
    assert_eq!(g.log.iter().filter(|e|matches!(e,game::Event::DamageDealt {..})).count(),1);
}

#[test]
fn r120_10_full_native_created_groups_use_completed_excess_not_announced_x() {
    for (name,token,damage,expected,tapped,mana) in [
        ("Goblin Negotiation","Goblin Token",6,4,false,"RRRRRRRR"),
        ("Hell to Pay","Treasure Token",6,4,true,"RRRRRRR"),
        ("Torch the Witness","Clue Token",3,1,false,"RRRR"),
    ] {
        let mut g=Game::new(vec![native(name),bear(2)],2,20);g.players[0].pool.colors=[20;5];
        let target=g.put(1,1,Zone::Battlefield);let card=g.put(0,0,Zone::Hand);
        g.act(0,paid(mana,Move::WithX(damage,Box::new(Move::Cast {card,targets:vec![vec![target.into()]]})))).unwrap();
        for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
        let made=g.battlefield.iter().copied().filter(|on|g.obj(*on).chars.name==token).collect::<Vec<_>>();
        cite!(["120.10","701.7a"];assert_eq!(made.len(),expected,"{name}"));
        for on in made {assert_eq!(g.obj(on).tapped,tapped);}
        let amounts=g.log.iter().filter_map(|e|match e {game::Event::DamageDealt {amount,..}=>Some(*amount),_=>None}).collect::<Vec<_>>();
        assert_eq!(amounts,vec![if name=="Torch the Witness" {6} else {damage}]);
    }
}
#[test]
fn r120_10_orbital_plunge_retains_its_independent_lander_dependency() {
    // The completed excess condition is now represented; this unrelated predefined
    // token behavior remains honestly refused, not replaced with a different token.
    let card=pinned("Orbital Plunge");
    cite!(["111.10","120.10"];assert!(game::play::check(&card).unwrap_err().0.contains("token: Lander")));
}
