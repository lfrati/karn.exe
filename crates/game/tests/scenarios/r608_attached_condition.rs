use crate::support::cite;
use game::{CardDef,Game,Move,Zone,Target};
#[test]
fn r608_2h_equipped_creature_condition_uses_the_current_attachment_for_the_instead_branch() {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let blade=CardDef::from_face(faces.iter().find(|f|f.face_name=="Blade of the Bloodchief").unwrap()).unwrap();
    game::play::check(&blade).unwrap();
    let bear=CardDef::new("Bear","{0}","Creature — Bear","",Some((2,2))).unwrap();
    let vampire=CardDef::new("Vampire","{0}","Creature — Vampire","",Some((2,2))).unwrap();
    for (recipient,expected) in [(1,1),(2,2)] {
        let mut g=Game::new(vec![blade.clone(),bear.clone(),vampire.clone()],2,20);
        let equipment=g.put(0,0,Zone::Battlefield);
        let creature=g.put(0,recipient,Zone::Battlefield);
        let victim=g.put(1,1,Zone::Battlefield);
        assert!(g.attach(equipment,Target::Object(creature)));
        g.destroy(victim);
        // The ordinary priority driver discovers the death and stacks its trigger.
        for _ in 0..3 {g.act(g.priority,Move::Pass).unwrap();}
        cite!(["608.2h"];case:format!("recipient={recipient}");assert_eq!(g.obj(creature).counters.get("+1/+1"),Some(&expected)));
    }
}
