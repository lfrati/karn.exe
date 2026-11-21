//! Tokens leave the battlefield before ceasing to exist at state-based actions (111.7–8).
use crate::support::cite;
use game::state::Answer;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};

fn game() -> Game {
    let defs = vec![
        CardDef::new("Maker", "{0}", "Instant", "Create a 2/2 green Bear creature token.", None).unwrap(),
        CardDef::new(
            "Pause",
            "{0}",
            "Instant",
            "Exile target creature. You may gain 1 life. Return it to the battlefield.",
            None,
        )
        .unwrap(),
        CardDef::new("Watcher", "{0}", "Enchantment", "Whenever a creature dies, you gain 2 life.", None).unwrap(),
        CardDef::new(
            "Revive",
            "{0}",
            "Instant",
            "Return target creature card from your graveyard to the battlefield.",
            None,
        )
        .unwrap(),
        CardDef::new("Destroy and pause", "{0}", "Instant", "Destroy target creature. You may gain 1 life.", None)
            .unwrap(),
        CardDef::new(
            "Bounce and count",
            "{0}",
            "Instant",
            "Return target creature to its owner's hand. You gain life equal to the number of cards in your hand.",
            None,
        )
        .unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    Game::new(defs, 3, 20)
}
fn pass(g: &mut Game) {
    for _ in 0..3 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn token(g: &mut Game) -> ObjId {
    let maker = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: maker, targets: vec![] }).unwrap();
    pass(g);
    *g.battlefield.last().unwrap()
}
fn moved(g: &Game, was: ObjId) -> ObjId {
    g.log
        .iter()
        .find_map(|e| match e {
            Event::ZoneChanged { was: w, now: Some(now), .. } if *w == was => Some(*now),
            _ => None,
        })
        .expect("a token first arrives in the destination zone")
}
#[test]
fn r111_7_tokens_exist_in_the_destination_until_state_based_actions() {
    for zone in [Zone::Graveyard, Zone::Hand, Zone::Library, Zone::Exile] {
        let mut g = game();
        let token = token(&mut g);
        g.move_object(token, zone);
        let there = moved(&g, token);
        cite!(["111.7"]; case: format!("{zone:?}"); assert_eq!(g.obj(there).zone, zone));
        assert!(g.obj(there).token());
        cite!(["111.6"]; case: format!("{zone:?}"); assert!(!g.is(Target::Object(there), ir::Sort::Card)));
        let changes = g.log.iter().filter(|e| matches!(e, Event::ZoneChanged { .. })).count();
        assert!(g.state_based_actions());
        cite!(["704.5d"]; case: format!("{zone:?}"); assert_eq!(g.obj(there).zone, Zone::Gone));
        assert!(!g.players[0].graveyard.contains(&there));
        assert!(!g.players[0].hand.contains(&there));
        assert!(!g.players[0].library.contains(&there));
        assert!(!g.exile.contains(&there));
        assert_eq!(
            g.log.iter().filter(|e| matches!(e, Event::ZoneChanged { .. })).count(),
            changes,
            "ceasing to exist is not another zone change"
        );
        assert!(!g.state_based_actions(), "cessation happens only once");
    }
}
#[test]
fn r111_8_a_token_cannot_return_during_the_same_resolution_before_cessation() {
    let mut g = game();
    let token = token(&mut g);
    let spell = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(token)]] }).unwrap();
    pass(&mut g);
    assert!(g.resolving().is_some(), "the optional instruction pauses resolution");
    let exiled = moved(&g, token);
    assert_eq!(g.exile, vec![exiled]);
    g.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["111.8"]; assert!(g.battlefield.is_empty()));
    assert!(g.exile.is_empty());
    cite!(["111.8"]; assert!(
        !g.log.iter().any(|e| matches!(e,Event::ZoneChanged{was,..} if *was==exiled)),
        "the attempted return cannot move the token a second time"
    ));
}
#[test]
fn r111_7_death_triggers_survive_token_cessation_and_do_not_trigger_twice() {
    let mut g = game();
    let token = token(&mut g);
    g.put(0, 2, Zone::Battlefield);
    let spell = g.put(0, 4, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(token)]] }).unwrap();
    pass(&mut g);
    let dead = moved(&g, token);
    assert_eq!(g.obj(dead).zone, Zone::Graveyard);
    assert_eq!(g.stack.len(), 1, "the death trigger waits until resolution ends");
    g.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["111.7"]; assert_eq!(g.obj(dead).zone, Zone::Gone));
    assert_eq!(g.stack.len(), 1);
    pass(&mut g);
    cite!(["111.7"]; assert_eq!(g.players[0].life, 22));
    assert!(g.stack.is_empty());
}
#[test]
fn r111_6_a_token_in_a_graveyard_is_not_a_creature_card_for_card_selectors() {

        let mut g = game();
    let token = token(&mut g);
    g.destroy(token);
    let dead = moved(&g, token);
    let revive = g.put(0, 3, Zone::Hand);
    let before = format!("{g:?}");
    cite!(["111.6"]; refused("an illegal target, Bear Token: it doesn't match the target's description (115.1)"): g.act(0, Move::Cast { card: revive, targets: vec![vec![Target::Object(dead)]] }));
    assert_eq!(format!("{g:?}"), before);
    assert_eq!(g.obj(dead).zone, Zone::Graveyard);
}
#[test]
fn r111_6_a_bounced_token_is_not_counted_as_a_card_during_resolution() {
    let mut g = game();
    let token = token(&mut g);
    let spell = g.put(0, 5, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(token)]] }).unwrap();
    pass(&mut g);
    cite!(["111.6"]; assert_eq!(g.players[0].life, 20));
    assert!(g.players[0].hand.is_empty());
}

#[test]
fn r121_1_drawing_uses_the_top_card_ignoring_a_token_temporarily_in_the_library() {
    // 111.6: a token is not a card. 121.1 draws the top card, not the top object.
    let defs = vec![
        CardDef::new("Maker", "{0}", "Instant", "Create a 2/2 green Bear creature token.", None).unwrap(),
        CardDef::new(
            "Put and draw",
            "{0}",
            "Instant",
            "Put target creature on top of its owner's library. Draw a card.",
            None,
        )
        .unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap();
    }
    for actual_card in [false, true] {
        let mut g = Game::new(defs.clone(), 3, 20);
        let token = token(&mut g);
        if actual_card {
            g.put(0, 2, Zone::Library);
        }
        let spell = g.put(0, 1, Zone::Hand);
        g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(token)]] }).unwrap();
        pass(&mut g);
        let in_library = moved(&g, token);
        assert_eq!(g.obj(in_library).zone, Zone::Gone);
        assert!(g.players[0].library.is_empty());
        cite!(["104.3c", "111.6"]; case: actual_card; assert_eq!(g.players[0].lost, !actual_card, "a token cannot make an empty library drawable"));
        cite!(["121.1"]; case: actual_card; assert_eq!(g.players[0].hand.len(), usize::from(actual_card)));
        if actual_card {
            assert_eq!(g.obj(g.players[0].hand[0]).chars.name, "Forest");
        }
        cite!(["121.1"]; assert!(!g.log.iter().any(|e| matches!(e,Event::Drew{card,..} if *card==in_library))));
    }
}

#[test]
fn r113_1c_a_cardless_stack_ability_is_not_a_token() {
    // Model the object category directly; general token-granted abilities are still unsupported.
    let mut g = game();
    let source = token(&mut g);
    let mut ability = g.obj(source).clone();
    ability.zone = Zone::Stack;
    ability.ability = Some((Some(source), 0));
    cite!(["113.1c"]; assert!(!ability.token(), "an ability from a token is a separate ability object"));
}

#[test]
fn r701_17_milling_and_top_card_queries_ignore_transient_tokens() {
    let mut g = game();
    let token = token(&mut g);
    let card = g.put(0, 0, Zone::Library);
    g.move_object(token, Zone::Library);
    let there = moved(&g, token);
    cite!(["111.6"]; assert_eq!(g.top_cards(0, 2), vec![card]));
    g.mill(0, 1);
    assert_eq!(g.players[0].library, vec![there]);
    cite!(["701.17a"]; assert!(g.log.iter().any(|e| matches!(e,Event::Milled{card:now,..} if g.obj(*now).card==Some(0)))));
    cite!(["701.17a", "111.6"]; assert!(!g.log.iter().any(|e| matches!(e,Event::Milled{card:now,..} if *now==there))));
    assert!(g.state_based_actions());
    assert!(g.players[0].library.is_empty());
}
#[test]
fn r401_7_numbered_library_positions_count_cards_not_transient_tokens() {
    let defs = vec![
        CardDef::new("Maker", "{0}", "Instant", "Create a 2/2 green Bear creature token.", None).unwrap(),
        CardDef::new("Place twice", "{0}", "Instant", "Put target creature on top of its owner's library. Put target creature into its owner's library third from the top.", None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2,2))).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap();
    }
    let mut g = Game::new(defs, 3, 20);
    let token = token(&mut g);
    let card = g.put(0, 2, Zone::Battlefield);
    for _ in 0..3 {
        g.put(0, 3, Zone::Library);
    }
    let spell = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(token)], vec![Target::Object(card)]] })
        .unwrap();
    pass(&mut g);
    let names: Vec<_> = g.top_cards(0, 10).iter().map(|o| g.obj(*o).chars.name.clone()).collect();
    cite!(["111.6"]; assert_eq!(names, ["Forest", "Forest", "Bear", "Forest"]));
}

/// 201.5, 111.4: a token named from its creator's own name (`~`), and a token created in combat,
/// are not run by the engine; a token with a plain name is, wherever the text puts the name.
#[test]
fn r111_4_names_from_the_creators_name_and_combat_placement_are_refused() {

    for text in [
        "Create a 3/3 blue Serpent creature token named ~'s Coil.",
        "Create a 1/1 white Soldier creature token that's tapped and attacking that opponent.",
        "Create a 1/1 white Cat Soldier creature token that's blocking target creature.",
    ] {
        let d=CardDef::new("Maker", "{0}", "Instant", text, None).unwrap();
        crate::support::refused(game::play::check(&d).map_err(|e|e.0),if text.contains("named") {"token: name made from the card's own name"} else {"program: token combat/attachment not lowered"});
    }
    for text in [
        "Create a 1/1 green Insect creature token with flying named Butterfly.",
        "Create a 1/1 green Insect creature token named Butterfly with flying.",
    ] {
        let d = CardDef::new("Maker", "{0}", "Instant", text, None).unwrap();
        game::play::check(&d).unwrap_or_else(|u| panic!("{text}: {}", u.0));
    }
}
