//! Storm fixes every player's earlier cast count and never counts its copies (702.40a).
use crate::support::{cite, mana_pool, oracle, paid};
use game::{Game, Move, Zone};

fn fixture() -> Game {
    Game::new(vec![oracle("Weather the Storm"), oracle("Nourish")], 2, 20)
}
fn cast(g: &mut Game, player: usize, index: usize) {
    if g.priority != player { g.act(g.priority, Move::Pass).unwrap(); }
    let spell = g.put(player, index, Zone::Hand);
    g.players[player].pool = mana_pool("GG");
    g.act(player, paid("GG", Move::Cast { card: spell, targets: vec![] })).unwrap();
}
fn resolve(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
#[test]
fn r702_40a_storm_counts_all_players_earlier_casts_but_not_later_casts() {
    let mut g = fixture();
    cast(&mut g, 0, 1); resolve(&mut g);
    cast(&mut g, 1, 1); resolve(&mut g);
    cast(&mut g, 0, 0);
    cite!(["702.40a"]; assert_eq!(g.stack.len(), 2));
    cast(&mut g, 1, 1); resolve(&mut g);
    resolve(&mut g);
    cite!(["702.40a", "707.10"]; assert_eq!(g.stack.len(), 3));
    for _ in 0..3 { resolve(&mut g); }
    cite!(["702.40a"]; assert_eq!((g.players[0].life, g.players[1].life), (35, 32)));
}
#[test]
fn r702_40a_the_first_spell_of_a_turn_produces_no_copies() {
    let mut g = fixture();
    cast(&mut g, 0, 0);
    resolve(&mut g);
    cite!(["702.40a"]; assert_eq!(g.stack.len(), 1));
    resolve(&mut g);
    cite!(["702.40a"]; assert_eq!(g.players[0].life, 23));
}

#[test]
fn fixed_cast_count_outside_its_cast_trigger_is_refused() {
    // Uncited (admission): other capture moments need retained values across incarnations,
    // turns and copied spells; the ordered cast receipt alone proves the own cast trigger.
    for (types, text) in [
        ("Creature", "When this creature enters, you gain 1 life for each spell cast this turn as you cast Cast marker."),
        ("Instant", "You gain 1 life for each spell cast this turn as you cast Cast marker."),
    ] {
        let pt = (types == "Creature").then_some((2, 2));
        let def = game::CardDef::new("Cast marker", "{0}", types, text, pt).unwrap();
        crate::support::refused(game::play::check(&def).map_err(|e| e.0), "fixed-at-cast spell count requires its own cast-trigger context");
    }
}
