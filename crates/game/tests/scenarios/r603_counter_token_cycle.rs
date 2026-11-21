//! The Modern deck's engine composes counter triggers, quoted token abilities,
//! death triggers and attached-object references without card-specific execution.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Zone, Target};
use game::legal::Asked;
use game::state::Answer;
use ir::mana::ManaSymbol;

fn settle(g: &mut Game) {
    for _ in 0..50 {
        match g.asked() {
            Some((p, Asked::May)) => g.act(p, Move::Choose(Answer::Yes)).unwrap(),
            _ if g.stack.is_empty() => return,
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("counter/token cycle did not settle: {:?}", g.asked());
}
fn spawn(g: &Game) -> ObjId {
    let tokens: Vec<_> = g.battlefield.iter().copied().filter(|o| g.obj(*o).token()).collect();
    assert_eq!(tokens.len(), 1); tokens[0]
}
#[test]
fn r603_2_counter_token_death_cycle_repeats_through_ordinary_priority_and_mana_abilities() {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog=CardDef::catalog(&faces);
    let defs: Vec<_>=["Basking Broodscale", "Blade of the Bloodchief", "Glaring Fleshraker"]
        .iter().map(|n| CardDef::from_faces(catalog.get(*n).unwrap()).unwrap()).collect();
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}",d.name(),e.0)); }
    let mut g=Game::new(defs,2,20);
    let brood=g.put(0,0,Zone::Battlefield);
    let blade=g.put(0,1,Zone::Battlefield);
    g.put(0,2,Zone::Battlefield);
    assert!(g.attach(blade,Target::Object(brood)));
    g.add_mana(0,ManaSymbol::Color(ir::Color::Green));g.add_mana(0,ManaSymbol::Colorless);
    g.act(0,paid("CG",Move::Activate {source:brood,ability:1,targets:vec![],paying:vec![]})).unwrap();
    settle(&mut g);
    assert_eq!(g.obj(brood).counters.get("+1/+1"),Some(&1));
    assert_eq!(g.players[1].life,19);
    let mut copy=g.clone();
    for branch in [&mut g,&mut copy] {
        for n in 2..=4 {
            let token=spawn(branch);
            branch.act(0,Move::Activate {source:token,ability:0,targets:vec![],paying:vec![]}).unwrap();
            // The mana ability is immediate; death, counter and creation triggers use the stack.
            settle(branch);
            cite!(["603.2"]; case: format!("counter {n}"); assert_eq!(branch.obj(brood).counters.get("+1/+1"),Some(&n)));
            assert_eq!(branch.players[0].pool.total(),n-1);
            cite!(["603.2"]; case: format!("damage {n}"); assert_eq!(branch.players[1].life,20-n as i32));
            assert_ne!(spawn(branch),token);
        }
    }
    assert_eq!(format!("{g:?}"),format!("{copy:?}"));
}

#[test]
fn r603_2c_named_counter_batches_fire_once_or_per_counter_and_ignore_other_kinds() {
    for (phrase, expected) in [("one or more +1/+1 counters are",21),("a +1/+1 counter is",23)] {
        let text=format!("Whenever {phrase} put on this creature, you gain 1 life.");
        let def=CardDef::new("Watcher","{0}","Creature",&text,Some((2,2))).unwrap();
        game::play::check(&def).unwrap();
        let mut g=Game::new(vec![def],2,20);let o=g.put(0,0,Zone::Battlefield);
        g.put_counters(o,"charge",3);g.act(0,Move::Pass).unwrap();settle(&mut g);
        assert_eq!(g.players[0].life,20);
        g.put_counters(o,"+1/+1",3);g.act(g.priority,Move::Pass).unwrap();settle(&mut g);
        cite!(["603.2c"]; case: phrase; assert_eq!(g.players[0].life,expected));
        g.put_counters(o,"+1/+1",0);g.act(g.priority,Move::Pass).unwrap();settle(&mut g);
        cite!(["603.2g"]; case: phrase; assert_eq!(g.players[0].life,expected));
    }
}
