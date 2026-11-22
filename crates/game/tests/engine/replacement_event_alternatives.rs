//! Correlated replacement-event alternatives remain syntax until every arm can be lowered.
use game::card::CardDef;

#[test]
fn correlated_replacement_events_remain_explicitly_refused() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let face = faces.iter().find(|face| face.face_name == "Anafenza, the Foremost").unwrap();
    let def = CardDef::from_face(face).expect("the complete pinned card reads");
    // Uncited (admission): no replacement consumer may silently select or flatten one arm.
    assert_eq!(game::play::check(&def).unwrap_err().0, "replacement: alternative event patterns not lowered");
}
