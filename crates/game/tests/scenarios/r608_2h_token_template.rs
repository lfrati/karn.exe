//! "Those tokens" reuses the creating text, including when its count is replaced.
use crate::support::{cite, oracle, mana_pool, paid};
use game::{Game, Move, Zone};

#[test]
fn r111_3_gather_the_townsfolk_reuses_its_human_template_at_fateful_hour() {
    for (life, count) in [(20, 2), (5, 5), (4, 5)] {
        let mut g = Game::new(vec![oracle("Gather the Townsfolk")], 2, life);
        let card = g.put(0, 0, Zone::Hand);
        g.players[0].pool = mana_pool("CW");
        g.act(0, paid("CW", Move::Cast { card, targets: vec![] })).unwrap();
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        cite!(["608.2c", "111.3"]; assert_eq!(g.battlefield.len(), count));
        for token in &g.battlefield {
            cite!(["111.3", "111.4"]; assert_eq!(g.obj(*token).chars.name, "Human Token"));
            cite!(["111.3"]; assert_eq!(g.power(*token), Some(1)));
            cite!(["111.3"]; assert_eq!(g.seen(*token, None).colors, vec![ir::Color::White]));
        }
    }
}

#[test]
fn r702_33_conquerors_pledge_replaces_six_with_twelve_of_its_kor_tokens() {
    for (kicks, count, units) in [(0, 6, "CCWWW"), (1, 12, "CCCCCCCCWWW")] {
        let mut g = Game::new(vec![oracle("Conqueror's Pledge")], 2, 20);
        let card = g.put(0, 0, Zone::Hand);
        g.players[0].pool = mana_pool(units);
        g.act(0, paid(units, Move::WithKicks(kicks, Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        cite!(["702.33e", "111.3"]; assert_eq!(g.battlefield.len(), count));
        for token in &g.battlefield {
            cite!(["111.3", "111.4"]; assert_eq!(g.obj(*token).chars.name, "Kor Soldier Token"));
            cite!(["111.3"]; assert_eq!(g.power(*token), Some(1)));
        }
    }
}

#[test]
fn token_anaphors_without_one_explicit_template_remain_refused() {
    for text in ["Create two of those tokens.",
        "Create a 1/1 white Human creature token. Create a 1/1 green Elf creature token. Create two of those tokens."] {
        let def = game::CardDef::new("Token reference", "{0}", "Sorcery", text, None).unwrap();
        // Uncited (admission): an absent or ambiguous template must never be guessed.
        crate::support::refused(game::play::check(&def).map_err(|e| e.0),
            "token reference: absent or ambiguous earlier token template");
    }
}
