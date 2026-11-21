use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Target, Zone};
const SEVER: &str = "Exile target creature and all other creatures with the same name as that creature.\nFlashback {5}{B}{B}";

fn game() -> Game {
    let defs = vec![
        CardDef::new("Sever the Bloodline", "{3}{B}", "Sorcery", SEVER, None).unwrap(),
        CardDef::new("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Grizzly Bears", "{1}", "Artifact", "", None).unwrap(),
        CardDef::new("Runeclaw Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Grizzly Bears", "{1}{G}", "Creature — Bear", "Shroud", Some((2, 2))).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap(); }
    Game::new(defs, 3, 20)
}
#[test]
fn r201_2a_exile_matches_names_not_types_and_does_not_target_the_other_creatures() {
    let mut g = game();
    let first = g.put(0, 1, Zone::Battlefield);
    let second = g.put(1, 4, Zone::Battlefield);
    let third = g.put(2, 1, Zone::Battlefield);
    let artifact = g.put(1, 2, Zone::Battlefield);
    let other = g.put(2, 3, Zone::Battlefield);
    let hand = g.put(1, 1, Zone::Hand);
    let card = g.put(0, 0, Zone::Hand);
    g.add_mana(0, ir::mana::ManaSymbol::Color(ir::Color::Black));
    for _ in 0..3 { g.add_mana(0, ir::mana::ManaSymbol::Colorless); }
    g.act(0, paid("CCCB", Move::Cast { card, targets: vec![vec![Target::Object(first)]] })).unwrap();
    for _ in 0..3 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["201.2a", "115.10a"]; assert!(g.exile.len()==3 && [first,second,third].iter().all(|o| g.obj(*o).zone==Zone::Gone)));
    cite!(["201.2a"]; assert_eq!(g.battlefield, vec![artifact,other]));
    assert_eq!(g.players[1].hand, vec![hand]);
}

#[test]
fn r201_2a_other_excludes_the_named_comparison_subject_not_the_ability_source() {
    let d = CardDef::new("Bearer", "{0}", "Creature — Bear", "{0}: Exile target creature and all other creatures with the same name as that creature.", Some((2,2))).unwrap();
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let target = g.put(1, 0, Zone::Battlefield);
    g.act(0, Move::Activate { source, ability: 0, targets: vec![vec![Target::Object(target)]], paying: vec![] }).unwrap();
    g.act(0, Move::Pass).unwrap(); g.act(1, Move::Pass).unwrap();
    cite!(["201.2a"]; assert_eq!(g.exile.len(), 2, "the source also shares the targeted creature's name"));
}

#[test]
fn r406_1_exiling_a_target_players_graveyard_uses_only_that_zone() {
    let d = CardDef::new("Graveyard Relic", "{1}", "Artifact", "{T}, Sacrifice this artifact: Exile target player's graveyard.", None).unwrap();
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d], 3, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let own = g.put(0, 0, Zone::Graveyard);
    let other = g.put(2, 0, Zone::Graveyard);
    let victim = g.put(1, 0, Zone::Graveyard);
    g.act(0, Move::Activate { source, ability: 0, targets: vec![vec![Target::Player(1)]], paying: vec![] }).unwrap();
    while !g.stack.is_empty() {
        g.act(g.priority, Move::Pass).unwrap();
    }
    assert_eq!(g.exile.len(), 1);
    cite!(["406.2"]; assert_eq!(g.obj(victim).zone, Zone::Gone));
    assert!(g.players[0].graveyard.contains(&own));
    cite!(["400.1"]; assert_eq!(g.players[2].graveyard, vec![other]));
}

#[test]
fn r603_6c_battlefield_to_graveyard_triggers_use_departed_abilities() {
    let d = CardDef::new("Relic", "{0}", "Artifact", "{0}, Sacrifice this artifact: You gain 1 life.\nWhen this artifact is put into a graveyard from the battlefield, you gain 2 life.", None).unwrap();
    game::play::check(&d).unwrap();
    let discard = CardDef::new("Discard", "{0}", "Sorcery", "Target player discards a card.", None).unwrap();
    let mut g = Game::new(vec![d,discard], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    g.act(0, Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["603.6c", "603.10a"]; assert_eq!(g.players[0].life, 23));
    g.put(0, 0, Zone::Hand);
    let spell = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Player(0)]] }).unwrap();
    g.act(0, Move::Pass).unwrap(); g.act(1, Move::Pass).unwrap();
    cite!(["603.6c"]; assert!(g.stack.is_empty(), "a discard is not departure from the battlefield"));
    assert_eq!(g.players[0].life, 23);
}

#[test]
fn r603_6c_graveyard_owner_is_not_the_departed_permanents_controller() {
    let mut defs: Vec<_> = ["your", "an opponent's"].into_iter().map(|owner| {
        let d = CardDef::new("Relic", "{0}", "Artifact Creature — Golem", &format!("{{0}}, Sacrifice this artifact: You gain 1 life.\nWhen this artifact is put into {owner} graveyard from the battlefield, you gain 2 life."), Some((1,1))).unwrap();
        game::play::check(&d).unwrap(); d
    }).collect();
    let reanimate = CardDef::new("Return", "{0}", "Sorcery", "Put target creature card from a graveyard onto the battlefield under your control.", None).unwrap();
    game::play::check(&reanimate).unwrap();
    defs.push(reanimate);
    let mut g = Game::new(defs, 2, 20);
    for (def, expected) in [(0, 21), (1, 24)] {
        let grave = g.put(1, def, Zone::Graveyard);
        let spell = g.put(0, 2, Zone::Hand);
        g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(grave)]] }).unwrap();
        g.act(0, Move::Pass).unwrap(); g.act(1, Move::Pass).unwrap();
        let source = *g.battlefield.last().unwrap();
        assert_eq!((g.obj(source).owner, g.obj(source).controller), (1,0));
        g.act(0, Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }).unwrap();
        while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
        cite!(["400.3", "109.5"]; case: format!("def {def}"); assert_eq!(g.players[0].life, expected));
    }
}

#[test]
fn r702_34a_sever_flashback_uses_the_same_effect_and_exiles_the_spell() {
    let mut g = game();
    let target = g.put(1, 1, Zone::Battlefield);
    g.put(2, 1, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Graveyard);
    for _ in 0..2 { g.add_mana(0, ir::mana::ManaSymbol::Color(ir::Color::Black)); }
    for _ in 0..5 { g.add_mana(0, ir::mana::ManaSymbol::Colorless); }
    g.act(0,paid("CCCCCBB",Move::CastVia(game::announcement::CastRoute::Flashback,Box::new(Move::Cast { card, targets: vec![vec![Target::Object(target)]] })))).unwrap();
    for _ in 0..3 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["201.2a"]; assert!(g.battlefield.is_empty()));
    cite!(["702.34a"]; assert!(g.players[0].graveyard.is_empty()));
    cite!(["702.34a"]; assert_eq!(g.exile.len(), 3));
}

#[test]
fn r608_2b_no_same_name_exile_when_the_only_target_is_illegal() {
    let mut g = game();
    let target = g.put(1, 1, Zone::Battlefield);
    let other = g.put(2, 1, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand);
    g.add_mana(0, ir::mana::ManaSymbol::Color(ir::Color::Black));
    for _ in 0..3 { g.add_mana(0, ir::mana::ManaSymbol::Colorless); }
    g.act(0, paid("CCCB", Move::Cast { card, targets: vec![vec![Target::Object(target)]] })).unwrap();
    g.destroy(target);
    for _ in 0..3 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["608.2b"]; assert!(g.exile.is_empty()));
    cite!(["608.2b"]; assert_eq!(g.battlefield, vec![other]));
}
