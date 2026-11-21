//! Linked exiled cards return through normal entry, counters and continuous effects (607.2a).
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Target, Zone};
const VACUUM: &str = "{T}: Exile target card from a graveyard.\n{6}, {T}, Sacrifice this artifact: Put each creature card exiled with this artifact onto the battlefield under your control with a flying counter on it. Each of them is a 1/1 Spirit in addition to its other types. Activate only as a sorcery.";
fn setup() -> Game {
    let defs=vec![
        CardDef::new("Ghost Vacuum","{1}","Artifact",VACUUM,None).unwrap(),
        CardDef::new("Bear","{0}","Creature — Bear","",Some((2,2))).unwrap(),
        CardDef::new("Island","","Basic Land — Island","",None).unwrap(),
        CardDef::new("Diamond Knight","{3}","Artifact Creature — Knight","Vigilance\nAs this creature enters, choose a color.\nWhenever you cast a spell of the chosen color, put a +1/+1 counter on this creature.",Some((1,1))).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap();
    }
    Game::new(defs, 3, 20)
}
fn resolve(g: &mut Game) {
    for _ in 0..3 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
#[test]
fn r607_2a_the_link_survives_the_source_sacrifice_and_returns_only_creatures() {
    let mut g = setup();
    let vacuum = g.put(0, 0, Zone::Battlefield);
    let bear = g.put(1, 1, Zone::Graveyard);
    let land = g.put(2, 2, Zone::Graveyard);
    for card in [bear, land] {
        g.untap(vacuum);
        g.act(
            0,
            Move::Activate { source: vacuum, ability: 0, targets: vec![vec![Target::Object(card)]], paying: vec![] },
        )
        .unwrap();
        resolve(&mut g);
    }
    g.untap(vacuum);
    g.players[0].pool.colorless = 6;
    g.act(0, paid("CCCCCC", Move::Activate { source: vacuum, ability: 1, targets: vec![], paying: vec![] })).unwrap();
    assert!(!g.battlefield.contains(&vacuum));
    let mut copy = g.clone();
    for g in [&mut g, &mut copy] {
        resolve(g);
        cite!(["607.2a"]; assert_eq!(g.battlefield.len(), 1));
        cite!(["607.2a"]; assert_eq!(g.exile.len(), 1));
        let returned = g.battlefield[0];
        assert_eq!(g.controller(returned), 0);
        assert_eq!(g.obj(returned).owner, 1);
        let seen = g.seen(returned, None);
        cite!(["613.4b"]; assert_eq!(seen.pt, Some((1, 1))));
        assert!(seen.subtypes.iter().any(|s| s.name() == "Spirit"));
        cite!(["613.1d"]; assert!(seen.subtypes.iter().any(|s| s.name() == "Bear")));
        assert_eq!(g.obj(returned).counters.get("flying"), Some(&1));
    }
}

#[test]
fn r614_12_entry_choices_finish_before_changes_apply_to_the_returned_group() {
    use game::binding::Value;
    use game::state::Answer;
    let mut g = setup();
    let vacuum = g.put(0, 0, Zone::Battlefield);
    let knight = g.put(1, 3, Zone::Graveyard);
    g.act(
        0,
        Move::Activate { source: vacuum, ability: 0, targets: vec![vec![Target::Object(knight)]], paying: vec![] },
    )
    .unwrap();
    resolve(&mut g);
    g.untap(vacuum);
    g.players[0].pool.colorless = 6;
    g.act(0, paid("CCCCCC", Move::Activate { source: vacuum, ability: 1, targets: vec![], paying: vec![] })).unwrap();
    resolve(&mut g);
    cite!(["614.12a"]; assert!(g.battlefield.is_empty()));
    cite!(["614.12"]; refused("a decision is pending: a choice of how the permanent enters the battlefield (614.12)"): g.act(0, Move::Pass));
    let mut copy = g.clone();
    for (g, color) in [(&mut g, ir::Color::Red), (&mut copy, ir::Color::Blue)] {
        crate::support::refused(g.act(1, Move::Choose(Answer::Value(Value::Color(color)))), "that decision is another player's");
        g.act(0, Move::Choose(Answer::Value(Value::Color(color)))).unwrap();
        assert_eq!(g.battlefield.len(), 1);
        assert!(g.stack.is_empty());
        assert_eq!(g.players[0].pool.total(), 0);
        let returned = g.battlefield[0];
        let choices = &g.obj(returned).choices;
        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].chooser, 0);
        assert_eq!(choices[0].value, Value::Color(color));
        let seen = g.seen(returned, None);
        cite!(["611.2c"]; case: format!("{color:?}"); assert_eq!(seen.pt, Some((1, 1))));
        cite!(["611.2c"]; case: format!("{color:?}"); assert!(seen.subtypes.iter().any(|s| s.name() == "Spirit")));
        assert_eq!(g.obj(returned).counters.get("flying"), Some(&1));
    }
}
#[test]
fn r607_2a_two_sources_keep_distinct_linked_groups() {
    let mut g = setup();
    let first = g.put(0, 0, Zone::Battlefield);
    let second = g.put(0, 0, Zone::Battlefield);
    let a = g.put(1, 1, Zone::Graveyard);
    let b = g.put(2, 1, Zone::Graveyard);
    for (source, card) in [(first, a), (second, b)] {
        g.act(0, Move::Activate { source, ability: 0, targets: vec![vec![Target::Object(card)]], paying: vec![] })
            .unwrap();
        resolve(&mut g);
    }
    g.untap(first);
    g.players[0].pool.colorless = 6;
    g.act(0, paid("CCCCCC", Move::Activate { source: first, ability: 1, targets: vec![], paying: vec![] })).unwrap();
    resolve(&mut g);
    assert_eq!(g.exile.len(), 1);
    cite!(["607.2a"]; assert_eq!(g.obj(g.exile[0]).owner, 2));
    let returned = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(1)).unwrap();
    cite!(["607.2a"]; assert_eq!(g.obj(returned).owner, 1));
    assert!(g.battlefield.contains(&second));
}
#[test]
fn r611_2c_an_empty_linked_group_produces_no_lasting_recipients() {
    let mut g = setup();
    let vacuum = g.put(0, 0, Zone::Battlefield);
    g.players[0].pool.colorless = 6;
    g.act(0, paid("CCCCCC", Move::Activate { source: vacuum, ability: 1, targets: vec![], paying: vec![] })).unwrap();
    resolve(&mut g);
    assert!(g.battlefield.is_empty());
    assert!(g.stack.is_empty());
    let later = g.put(0, 1, Zone::Battlefield);
    let seen = g.seen(later, None);
    cite!(["611.2c"]; assert_eq!(seen.pt, Some((2, 2))));
    cite!(["611.2c"]; assert!(!seen.subtypes.iter().any(|s| s.name() == "Spirit")));
}

#[test]
fn r608_2c_a_skipped_entry_exports_an_empty_result_group() {
    for perform in [false, true] {
        let mut g = setup();
        let def=CardDef::new("Conditional return","{0}","Sorcery","If you control an artifact, return target creature card from your graveyard to the battlefield. Each of them is a 1/1 Spirit in addition to its other types.",None).unwrap();
        game::play::check(&def).unwrap();
        let index = g.defs.len();
        std::sync::Arc::make_mut(&mut g.defs).push(def);
        if perform {
            g.put(0, 0, Zone::Battlefield);
        }
        let bystander = g.put(1, 1, Zone::Battlefield);
        let bear = g.put(0, 1, Zone::Graveyard);
        let spell = g.put(0, index, Zone::Hand);
        g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(bear)]] }).unwrap();
        resolve(&mut g);
        let returned = g.battlefield.iter().find(|o| **o != bystander && g.obj(**o).card == Some(1)).copied();
        assert_eq!(returned.is_some(), perform);
        let seen = g.seen(bystander, None);
        assert_eq!(seen.pt, Some((2, 2)));
        cite!(["608.2c"]; case: format!("perform {perform}"); assert!(!seen.subtypes.iter().any(|s| s.name() == "Spirit")));
        if let Some(returned) = returned {
            cite!(["608.2c"]; assert_eq!(g.seen(returned, None).pt, Some((1, 1))));
        }
    }
}
