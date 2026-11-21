//! Setup names physical cards. Reading an isolated face for grammar coverage is separate.
use game::CardDef;

#[test]
fn r712_8a_back_faces_and_r710_2_flipped_names_cannot_be_deck_entries() {

    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog=CardDef::catalog(&faces);
    for name in ["Avatar Kyoshi","Azamuki, Treachery Incarnate","Soporific Springs","Brisela, Voice of Nightmares"] {
        assert!(!catalog.contains_key(name),"{name} is not a standalone physical card");
        assert!(faces.iter().any(|f|f.face_name==name),"face coverage keeps {name}");
    }
    for name in ["The Legend of Kyoshi","Cunning Bandit","Sink into Stupor"] {
        let card=&catalog[name];assert_eq!(card.len(),2,"{name}");
        assert_eq!(card[0].face,0);assert_eq!(card[1].face,1);
        assert_eq!(card[0].oracle_id,card[1].oracle_id);
        assert_eq!(catalog[card[0].name.as_str()][0].oracle_id,card[0].oracle_id);
    }
    game::play::check(&CardDef::from_faces(&catalog["Sink into Stupor"]).unwrap()).unwrap();
    for face in &catalog["Sink into Stupor"] {
        game::play::check(&CardDef::from_face(face).unwrap()).unwrap();
        crate::support::refused(CardDef::from_faces(&[*face]),"a modal double-faced card requires both faces (712.3)");
    }
    // Until these whole-card layouts execute, refuse instead of admitting just one side.
    for name in ["Cunning Bandit"] {
        crate::support::refused(CardDef::from_faces(&catalog[name]),"unsupported whole-card layout: flip");
    }
}

#[test]
fn r709_2_and_r715_2_aliases_keep_whole_cards_and_do_not_replace_standalone_names() {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog=CardDef::catalog(&faces);
    for (name,alias) in [("Toil // Trouble","Trouble"),("Bonecrusher Giant // Stomp","Stomp")] {
        assert_eq!(catalog[name].len(),2);
        assert_eq!(catalog[name][0].oracle_id,catalog[alias][0].oracle_id);
        game::play::check(&CardDef::from_faces(&catalog[alias]).unwrap()).unwrap();
    }
    for name in ["Lightning Bolt","Ancestral Recall","Brainstorm","Swords to Plowshares"] {
        assert_eq!(catalog[name].len(),1,"standalone card {name}");
        assert_eq!(catalog[name][0].face,0);
        game::play::check(&CardDef::from_faces(&catalog[name]).unwrap()).unwrap();
    }
    let reversed:Vec<_>=faces.into_iter().rev().collect();
    let reverse=CardDef::catalog(&reversed);
    assert_eq!(reverse["Brainstorm"].len(),1,"corpus order cannot select an unrelated back face");
}
