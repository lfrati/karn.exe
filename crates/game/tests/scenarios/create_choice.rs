//! 608.2d: described-token alternatives remain outside the fixed predefined-token slice;
//! ordinary single and And creation remain executable (701.7).
use game::CardDef;

fn checked(text: &str) -> Result<(), String> {
    let d = CardDef::new("Procedure", "{0}", "Sorcery", text, None).unwrap();
    game::play::check(&d).map_err(|u| u.0)
}

#[test]
fn r608_2d_described_token_alternatives_are_refused_and_creating_both_is_admitted() {
    let refused = checked("Create a 1/1 white Soldier creature token or a 2/2 black Zombie creature token.").expect_err("a choice of tokens");
    assert!(refused.contains("token alternatives require predefined tokens"), "{refused}");
    for text in ["Create a 1/1 white Soldier creature token and a 2/2 black Zombie creature token.", "Create a 1/1 white Soldier creature token."] {
        assert_eq!(checked(text), Ok(()), "{text}");
    }
}
