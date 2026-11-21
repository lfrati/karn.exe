//! Card backing and zone roles overlap (108.2); action preconditions still constrain zones.
use game::{CardDef, Event, Game, Move, Target, Zone};
use ir::{Sort, Verb};
use crate::support::cite;
fn game() -> Game {
    let defs = vec![
        CardDef::new("Bear", "{0}", "Creature — Bear", "{0}: You gain 1 life.", Some((2,2))).unwrap(),
        CardDef::new("Maker", "{0}", "Sorcery", "Create a 1/1 white Soldier creature token.", None).unwrap(),
        CardDef::new("Stay", "{0}", "Creature — Spirit", "{0}: Put target creature onto the battlefield.", Some((2,2))).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap(); }
    Game::new(defs, 2, 20)
}
fn resolve(g: &mut Game) { g.act(0, Move::Pass).unwrap(); g.act(1, Move::Pass).unwrap(); }
#[test]
fn r108_2_a_card_remains_a_card_when_it_becomes_a_spell_and_a_permanent() {
    let mut g = game();
    let card = g.put(0, 0, Zone::Hand);
    assert!(g.is(Target::Object(card), Sort::Card));
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    let spell = g.stack[0];
    cite!(["108.2", "112.1"]; assert!(g.is(Target::Object(spell), Sort::Card)));
    cite!(["112.1"]; assert!(g.is(Target::Object(spell), Sort::Spell)));
    cite!(["400.7"]; assert!(!g.is(Target::Object(card), Sort::Card), "the stale identity is gone"));
    resolve(&mut g);
    let permanent = g.battlefield[0];
    cite!(["110.1"]; assert!(g.is(Target::Object(permanent), Sort::Card)));
    cite!(["110.1"]; assert!(g.is(Target::Object(permanent), Sort::Permanent)));
    assert!(!g.is(Target::Object(spell), Sort::Card));
    for zone in [Zone::Library, Zone::Graveyard, Zone::Exile] {
        let card = g.put(1, 0, zone);
        cite!(["108.2"]; case: format!("{zone:?}"); assert!(g.is(Target::Object(card), Sort::Card)));
        cite!(["110.1"]; case: format!("{zone:?}"); assert!(!g.is(Target::Object(card), Sort::Permanent)));
    }
}
#[test]
fn r108_2b_tokens_and_stack_abilities_are_not_cards() {
    let mut g = game();
    let source = g.put(0, 0, Zone::Battlefield);
    g.act(0, Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    cite!(["108.2"]; assert!(!g.is(Target::Object(g.stack[0]), Sort::Card)));
    resolve(&mut g);
    let maker = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card: maker, targets: vec![] }).unwrap(); resolve(&mut g);
    let token = *g.battlefield.last().unwrap();
    assert!(g.obj(token).token());
    cite!(["110.1"]; assert!(g.is(Target::Object(token), Sort::Permanent)));
    cite!(["108.2b", "111.6"]; assert!(!g.is(Target::Object(token), Sort::Card)));
}
#[test]
fn r701_9a_card_classification_does_not_allow_discarding_outside_your_hand() {
    let mut g = game();
    for zone in [Zone::Battlefield, Zone::Stack, Zone::Library, Zone::Graveyard, Zone::Exile] {
        let card = g.put(0, 0, zone);
        assert!(g.is(Target::Object(card), Sort::Card));
        cite!(["701.9a"]; case: format!("{zone:?}"); assert!(!g.can(Verb::Discard, 0, Target::Object(card))));
        let before = format!("{g:?}"); g.discard(0, card); assert_eq!(format!("{g:?}"), before);
    }
    let other = g.put(1, 0, Zone::Hand);
    cite!(["701.9a"]; assert!(!g.can(Verb::Discard, 0, Target::Object(other))));
    let before = format!("{g:?}");
    g.discard(0, other);
    assert_eq!(format!("{g:?}"), before);
    let own = g.put(0, 0, Zone::Hand);
    cite!(["701.9a"]; assert!(g.can(Verb::Discard, 0, Target::Object(own))));
    g.discard(0, own); cite!(["701.9a"]; assert_eq!(g.obj(own).zone, Zone::Gone));
}
#[test]
fn r400_7_putting_a_permanent_onto_its_current_battlefield_does_not_reenter_it() {
    let mut g = game();
    let source = g.put(0, 2, Zone::Battlefield);
    g.tap(source);
    g.act(0, Move::Activate { source, ability: 0, targets: vec![vec![Target::Object(source)]], paying: vec![] }).unwrap();
    let before = g.log.len();
    resolve(&mut g);
    // "target creature" is a permanent already on the battlefield (109.2); putting it there is no move
    // "from one zone to another" (400.7), so that instruction is impossible and ignored (101.3)
    cite!(["101.3"]; assert_eq!(g.battlefield, [source]));
    cite!(["101.3", "400.7"]; assert!(g.obj(source).tapped, "the same object"));
    cite!(["101.3", "400.7"]; assert!(!g.log[before..].iter().any(|e| matches!(e, Event::ZoneChanged { was, .. } if *was == source))));
}
