//! Resolution choices bind typed local values; linked choices retain their own lifetime.
use crate::support::{cite, paid};
use game::{Game, Move, Zone, Event};
use game::card::CardDef;
use game::binding::{Kind, Value};
use game::state::{Answer, Choice, Decision};
use ir::Color;

const INTUITION: &str = "Choose a card type, then reveal the top four cards of your library. Put all cards of the chosen type revealed this way into your hand and the rest into your graveyard. (Artifact, battle, creature, enchantment, instant, kindred, land, planeswalker, and sorcery are card types.)";
fn def(text: &str) -> CardDef {
    CardDef::new("Vigean Intuition", "{3}{G}{U}", "Instant", text, None).unwrap()
}
fn setup(text: &str, library: &[usize]) -> Game {
    let defs = vec![def(text),
        CardDef::new("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2,2))).unwrap(),
        CardDef::new("Ornithopter", "{0}", "Artifact Creature — Thopter", "Flying", Some((0,2))).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap(); }
    assert_eq!(defs[0].compiled_abilities(), vec![0]);
    let mut g = Game::new(defs, 3, 20);
    for card in library { g.put(0, *card, Zone::Library); }
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[Color::Blue as usize] = 1;
    g.players[0].pool.colors[Color::Green as usize] = 1;
    g.players[0].pool.colorless = 3;
    g.act(0, paid("CCCGU", Move::Cast {card, targets: vec![]})).unwrap();
    for p in 0..3 { g.act(p, Move::Pass).unwrap(); }
    g
}
fn choose(g: &mut Game, value: Value) { g.act(0, Move::Choose(Answer::Value(value))).unwrap(); }
fn names(g: &Game, cards: &[game::ObjId]) -> Vec<String> {
    let mut result: Vec<_> = cards.iter().map(|o| g.obj(*o).chars.name.clone()).collect();
    result.sort(); result
}

#[test]
fn r608_2d_vigean_intuition_chooses_a_card_type_before_revealing_and_preserves_cloned_choices() {
    let mut g = setup(INTUITION, &[1, 2, 3, 0, 1]);
    cite!(["608.2d"]; assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::Value(Kind::CardType) })));
    cite!(["608.2c"]; assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Revealed {..})).count(), 0));
    assert!(g.top_cards(0,4).iter().all(|o| (0..3).all(|p| !g.sees(p,*o))));
    let before = format!("{g:?}");
    for (player, action) in [
        (1, Move::Choose(Answer::Value(Value::CardType("creature".into())))),
        (0, Move::Choose(Answer::Value(Value::Color(Color::Green)))),
        (0, Move::Choose(Answer::Value(Value::CardType("legendary".into())))),
        (0, Move::Pass),
    ] {
        let why = match (player, &action) { (1, _) => "that decision is another player's", (_, Move::Pass) => "a decision is pending: a choice of the effect being applied (608.2d)", _ => "choose a card type (205.2a)" };
        crate::support::refused(g.act(player, action), why);
        assert_eq!(format!("{g:?}"), before);
    }
    let top = g.top_cards(0,4);
    let mut same = g.clone();
    let mut different = g.clone();
    for game in [&mut g, &mut same] {
        choose(game, Value::CardType("creature".into()));
        // CR404.3: explicitly authored owner order, separate from selection.
        assert_eq!(game.decision, Some(game::state::Decision::Choose { player: 0, choice: game::state::Choice::OrderCards { to: game::state::Placement::Zone(Zone::Graveyard), cards: vec![top[1],top[2]] } }));
        game.act(0, Move::Choose(game::state::Answer::Objects(vec![top[2],top[1]]))).unwrap();
    }
    choose(&mut different, Value::CardType("instant".into()));
    // CR404.3: explicitly authored owner order, separate from selection.
    assert_eq!(different.decision, Some(game::state::Decision::Choose { player: 0, choice: game::state::Choice::OrderCards { to: game::state::Placement::Zone(Zone::Graveyard), cards: vec![top[0],top[2],top[3]] } }));
    different.act(0, Move::Choose(game::state::Answer::Objects(vec![top[3],top[2],top[0]]))).unwrap();

    assert_eq!(format!("{g:?}"), format!("{same:?}"));
    cite!(["608.2c"]; assert_eq!(names(&g, &g.players[0].hand), ["Grizzly Bears", "Ornithopter"]));
    cite!(["608.2c"]; assert_eq!(names(&different, &different.players[0].hand), ["Vigean Intuition"]));
    assert_eq!(g.players[0].library.len(),1);
    assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Revealed {..})).count(), 1);
    assert!(g.resolving().is_none());
    assert!(g.players[0].hand.iter().all(|o| g.obj(*o).choices.is_empty()), "local answers never become object memory");
}

#[test]
fn r205_2a_card_type_choices_include_all_rulebook_types_and_multi_type_cards_match_either() {
    let options = game::binding::options(Kind::CardType);
    cite!(["205.2a"]; assert_eq!(options.len(), ir::vocab::list("card types").len()));
    for name in ir::vocab::list("card types") {
        let value = Value::CardType(name.clone());
        cite!(["205.2a"]; case: &name; assert!(options.contains(&value)));
        let mut g = setup(INTUITION, &[2]);
        choose(&mut g, value);
        cite!(["205.2b"]; case: &name; assert_eq!(g.players[0].hand.len(), usize::from(name == "creature" || name == "artifact"), "{name}"));
    }
    for bad in ["Goblin", "legendary", "permanent", ""] {
        cite!(["205.2a"]; case: bad; assert!(!game::binding::accepts(Kind::CardType, &Value::CardType(bad.into()))));
    }
}

#[test]
fn r608_2d_an_empty_library_does_not_skip_a_scalar_choice() {
    let mut g = setup(INTUITION, &[]);
    cite!(["608.2d"]; assert_eq!(g.decision, Some(Decision::Choose {player:0, choice:Choice::Value(Kind::CardType)})));
    choose(&mut g, Value::CardType("land".into()));
    assert!(g.players[0].hand.is_empty());
    assert_eq!(names(&g, &g.players[0].graveyard), ["Vigean Intuition"]);
}

#[test]
fn r608_2c_chosen_type_is_refined_from_the_local_creature_type_producer() {
    let text = "Choose a creature type. Reveal the top four cards of your library. Put all creature cards of the chosen type revealed this way into your hand and the rest into your graveyard.";
    let mut g = setup(text, &[1,2,3]);
    let library = g.players[0].library.clone();
    let top = g.top_cards(0, 3); // Forest, Ornithopter, Grizzly Bears.
    cite!(["608.2d"]; assert_eq!(g.decision, Some(Decision::Choose { player:0, choice:Choice::Value(Kind::CreatureType) })));
    choose(&mut g, Value::CreatureType(ir::Subtype::from_name("Bear").unwrap()));
    cite!(["608.2f"]; assert_eq!(g.players[0].library, library));
    cite!(["608.2f"]; assert!(g.players[0].hand.is_empty()));
    // The owner puts Forest above Ornithopter before either partition moves.
    cite!(["404.3"]; ok: g.act(0, Move::Choose(Answer::Objects(vec![top[0], top[1]]))));
    cite!(["608.2c"]; assert_eq!(names(&g, &g.players[0].hand), ["Grizzly Bears"]));
}

#[test]
fn r608_2c_binding_refuses_missing_wrong_kind_ambiguous_and_non_dominating_producers() {

    for (prefix, phrase) in [
        ("", "type"),
        ("Choose a color. ", "type"),
        ("Choose a creature type. Choose a card type. ", "type"),
        ("Choose a color. Choose a color. ", "color"),
    ] {
        let text = format!("{prefix}Reveal the top four cards of your library. Put all cards of the chosen {phrase} revealed this way into your hand and the rest into your graveyard.");
        crate::support::refused(game::play::check(&def(&text)).map_err(|e|e.0),if prefix.matches("Choose").count()>1 {"choice reference: ambiguous local or linked producers"} else {"linked choice: no Type producer (607.2d)"});
    }
    let text = "Reveal the top four cards of your library. Put all cards of the chosen type revealed this way into your hand and the rest into your graveyard. Choose a card type.";
    crate::support::refused(game::play::check(&def(text)).map_err(|e|e.0),"linked choice: no Type producer (607.2d)");
    let text = "As this land enters, choose a color.\n{T}: Choose a color. Reveal the top four cards of your library. Put all cards of the chosen color revealed this way into your hand and the rest into your graveyard.";
    let d = CardDef::new("Ambiguous links", "", "Land", text, None).unwrap();
    assert!(game::play::check(&d).unwrap_err().0.contains("ambiguous local or linked"));
}

#[test]
fn r608_2d_multiple_values_and_restricted_domains_remain_explicit_refusals() {
    for prefix in ["Choose two colors.", "Choose a color other than red.", "Choose a color at random."] {
        let d = def(prefix);
        let error = game::play::check(&d).unwrap_err();
        assert!(error.0.contains("choice:"), "{error:?}");
    }
}

#[test]
fn r101_4b_a_local_scalar_answer_is_public_during_later_decisions_and_not_repeated_on_resume() {
    let text = "Choose a color. Reveal the top four cards of your library. Put a card of the chosen color from among them into your hand and the rest on the bottom of your library in any order.";
    let mut g = setup(text, &[1,1,2,3]);
    choose(&mut g, Value::Color(Color::Green));
    let Some(Decision::Choose {player:0, choice:Choice::Objects {among, ..}}) = &g.decision else { panic!("selection") };
    let picked = among[0];
    let previous: Vec<_> = g.public_choices().cloned().collect();
    cite!(["608.2d"]; assert!(matches!(previous.as_slice(), [game::state::PublicChoice::Value {player:0, slot:0, value:Value::Color(Color::Green), ..}])));
    let mut cloned = g.clone();
    for game in [&mut g, &mut cloned] {
        game.act(0, Move::Choose(Answer::Objects(vec![picked]))).unwrap();
        assert_eq!(game.public_choices().cloned().collect::<Vec<_>>(), previous);
    }
    assert_eq!(format!("{g:?}"), format!("{cloned:?}"));
}
