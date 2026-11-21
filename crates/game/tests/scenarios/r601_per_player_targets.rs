//! Per-player library search does not imply per-player announced target groups.
use game::CardDef;
#[test]
fn r601_2c_per_player_target_cardinality_is_refused_until_announcements_represent_it() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let grasp = CardDef::from_faces(catalog.get("Grasp of Fate").unwrap()).unwrap();
    let error = game::play::check(&grasp).unwrap_err();
    assert!(error.0.contains("per-player target announcements"), "{error:?}");
    let jace = CardDef::from_faces(catalog.get("Jace, Architect of Thought").unwrap()).unwrap();
    game::play::check(&jace).unwrap();
}
