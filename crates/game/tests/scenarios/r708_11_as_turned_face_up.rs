//! "As this creature is turned face up, put four +1/+1 counters on it." (708.11): the ability
//! of the face-up permanent is applied while it is being turned face up, not afterward, by
//! whatever turns it: the special action for any of its costs or an effect. What triggers on
//! its turning sees the counters. It does nothing as the permanent enters face up. Other
//! "as … turned face up" texts are refused by name. Bubble Smuggler ({1}{U} 2/1, disguise
//! {5}{U}), Soul Summons and Break Open are Oracle cards.
use crate::support;
use game::face::Way;
use game::announcement::CastCost;
use support::*;

type Text<'a> = (&'a str, &'a str, &'a str, &'a str, Option<(i32, i32)>);

const SCALES: Text = ("Hardened Scales", "{G}", "Enchantment",
    "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.", None);

/// Oracle cards by name, then test texts, with ten Wastes in each library.
fn game(names: &[&str], texts: &[Text]) -> T {
    let mut defs: Vec<CardDef> = names.iter().map(|n| oracle(n)).collect();
    for (name, cost, types, text, pt) in texts {
        let d = CardDef::new(name, cost, types, text, *pt).unwrap();
        game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
        defs.push(d);
    }
    defs.push(CardDef::new("Wastes", "", "Basic Land", "", None).unwrap());
    let mut g = Game::new(defs, 2, 20);
    let filler = g.defs.len() - 1;
    for p in 0..2 { for _ in 0..10 { g.put(p, filler, Zone::Library); } }
    T::new(g)
}
fn put(t: &mut T, p: usize, name: &str, zone: Zone) -> ObjId {
    let index = t.g.defs.iter().position(|d| d.name() == name).unwrap_or_else(|| panic!("no {name}"));
    t.g.put(p, index, zone)
}
/// Player 0 casts this card face down for {3} and it resolves: the face-down permanent.
fn face_down(t: &mut T, name: &str) -> ObjId {
    let card = put(t, 0, name, Zone::Hand);
    t.mana(0, "{3}");
    let row = t.g.def(card).unwrap().abilities().iter().position(|a| matches!(a,
        ir::Ability::Keywords(items) if items.iter().any(|i| matches!(i.keyword.name(), "Morph" | "Megamorph" | "Disguise"))))
        .expect("the fixture's printed face-down cost");
    let cost = CastCost::Alternative(row);
    t.g.act(0, paid("CCC",Move::CastFor(cost, Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    let before = t.g.battlefield.clone();
    t.resolve();
    *t.g.battlefield.iter().find(|o| !before.contains(o)).expect("it entered")
}
fn turn_up(t: &mut T, o: ObjId, way: Way, mana: &str) {
    while t.g.priority != 0 { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.mana(0, mana);
    t.g.act(0, paid(mana,Move::TurnFaceUp(o, way))).unwrap();
    assert!(!t.g.face_down(o));
}
fn counters(t: &T, o: ObjId) -> u32 {
    t.g.obj(o).counters.get("+1/+1").copied().unwrap_or(0)
}

#[test]
fn r708_11_the_counters_are_put_as_it_turns_and_its_watchers_see_them() {
    let mut t = game(&["Bubble Smuggler"], &[(
        "Counter Watcher", "{1}", "Enchantment",
        "Whenever a permanent you control is turned face up, if it has a +1/+1 counter on it, you gain 3 life.", None,
    )]);
    put(&mut t, 0, "Counter Watcher", Zone::Battlefield);
    let o = face_down(&mut t, "Bubble Smuggler");
    let (life, from) = (t.g.players[0].life, t.mark());
    turn_up(&mut t, o, Way::Disguise, "{5}{U}");
    cite!(["708.11"]; assert_eq!(counters(&t, o), 4));
    assert_eq!((t.g.power(o), t.g.toughness(o)), (Some(6), Some(5)));
    let log = t.g.log.since(from);
    let up = log.iter().position(|e| matches!(e, game::Event::TurnedFaceUp { .. })).unwrap();
    let put = log.iter().position(|e| matches!(e, game::Event::CountersPut { count: 4, .. })).unwrap();
    assert!(up < put);
    t.resolve();
    cite!(["708.11", "603.4"]; assert_eq!(t.g.players[0].life, life + 3, "the intervening if saw the counters"));
}

#[test]
fn r708_11_an_effect_turning_it_up_applies_it_too() {
    let mut t = game(&["Bubble Smuggler", "Break Open"], &[]);
    let o = face_down(&mut t, "Bubble Smuggler");
    t.g.act(0, Move::Pass).unwrap();
    let open = put(&mut t, 1, "Break Open", Zone::Hand);
    t.mana(1, "{1}{R}{2}");
    t.g.act(1, paid("CR",Move::Cast { card: open, targets: vec![vec![Target::Object(o)]] })).unwrap();
    assert!(matches!(t.settle(), Boundary::Question(1, Asked::Pay { .. })), "ward {{2}}");
    t.g.act(1, Move::Choose(Answer::Pay { plan: vec![], with: Some(mana_pool("CC")), tapped: vec![] })).unwrap();
    t.resolve();
    assert!(!t.g.face_down(o));
    cite!(["708.11"]; assert_eq!(counters(&t, o), 4));
}

#[test]
fn r708_11_a_manifested_one_turned_up_for_its_mana_cost_applies_it() {
    let mut t = game(&["Bubble Smuggler", "Soul Summons"], &[]);
    put(&mut t, 0, "Bubble Smuggler", Zone::Library);
    let soul = put(&mut t, 0, "Soul Summons", Zone::Hand);
    t.mana(0, "{1}{W}");
    t.g.act(0, paid("CW",Move::Cast { card: soul, targets: vec![] })).unwrap();
    t.resolve();
    let o = *t.g.battlefield.last().unwrap();
    turn_up(&mut t, o, Way::Manifested, "{1}{U}");
    cite!(["708.11", "701.40b"]; assert_eq!(counters(&t, o), 4));
}

#[test]
fn r708_11_it_does_nothing_as_it_enters_face_up() {
    let mut t = game(&["Bubble Smuggler"], &[]);
    let card = put(&mut t, 0, "Bubble Smuggler", Zone::Hand);
    t.mana(0, "{1}{U}");
    t.g.act(0, paid("CU",Move::Cast { card, targets: vec![] })).unwrap();
    t.resolve();
    let o = *t.g.battlefield.last().unwrap();
    assert_eq!(counters(&t, o), 0);
}

#[test]
fn r708_11_with_megamorph_each_ability_puts_its_own_counters() {
    let mut t = game(&[], &[SCALES, (
        "Swelling Beast", "{4}{G}", "Creature — Beast",
        "Megamorph {2}{G}\nAs this creature is turned face up, put two +1/+1 counters on it.", Some((1, 1)),
    )]);
    put(&mut t, 0, "Hardened Scales", Zone::Battlefield);
    let o = face_down(&mut t, "Swelling Beast");
    turn_up(&mut t, o, Way::Morph, "{2}{G}");
    cite!(["702.37b", "708.11", "614.1a"]; assert_eq!(counters(&t, o), 5, "1+1 for megamorph and 2+1 for the other: two placements"));
}

#[test]
fn r708_11_other_texts_are_refused_by_name() {
    let d = CardDef::new("Probe", "{4}", "Creature — Beast", "Morph {2}\nAs this creature is turned face up, draw a card.", Some((3, 3))).unwrap();
    let refused = game::play::check(&d).expect_err("refused").0;
    assert!(refused.starts_with("as this is turned face up: only putting counters on it"), "{refused}");
}

#[test]
fn r708_11_separate_face_up_instructions_retain_orders_without_partial_commit() {
    let times=("Times", "{0}", "Enchantment",
        "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.",None);
    let mut t=game(&[],&[SCALES,times,("Swelling Beast","{4}{G}","Creature — Beast",
        "Megamorph {2}{G}\nAs this creature is turned face up, put two +1/+1 counters on it.",Some((1,1)))]);
    let scales=put(&mut t,0,"Hardened Scales",Zone::Battlefield);let times=put(&mut t,0,"Times",Zone::Battlefield);
    let o=face_down(&mut t,"Swelling Beast");let start=t.g.log.len();turn_up(&mut t,o,Way::Morph,"{2}{G}");
    let answer=|g:&mut Game,source:ObjId| {
        let Some(game::state::Decision::Choose{choice:game::state::Choice::Replacement{among,..},..})=g.decision.clone() else{panic!("replacement question")};
        let id=among.into_iter().find(|id|id.source==source).unwrap();g.act(0,Move::Choose(Answer::Replacement(id))).unwrap();
    };
    assert_eq!(counters(&t,o),0);
    answer(&mut t.g,scales); // Megamorph's1: (1+1)*2=4.
    assert_eq!(counters(&t,o),0,"the other instruction still has a pending order");
    assert!(t.g.log[start..].iter().all(|e|!matches!(e,game::Event::CountersPut{..})));
    let Some(game::state::Decision::Choose{choice:game::state::Choice::Replacement{counters:Some(proposal),..},..})=&t.g.decision else{panic!("second instruction question")};
    assert_eq!(proposal.inventory,vec![("+1/+1".into(),2)]);
    answer(&mut t.g,times); // Printed instruction's2: 2*2+1=5.
    cite!(["702.37b","708.11","614.5"];assert_eq!(counters(&t,o),9));
    let actual:Vec<_>=t.g.log[start..].iter().filter_map(|e|if let game::Event::CountersPut{on:Target::Object(on),count,..}=e{(*on==o).then_some(*count)}else{None}).collect();
    assert_eq!(actual,vec![4,5]);
}
#[test]
fn r708_11_effect_turn_face_up_preserves_each_printed_placement_instruction() {
    let mut t=game(&[],&[SCALES,("Growing Beast","{4}{G}","Creature — Beast",
        "Morph {2}{G}\nAs this creature is turned face up, put two +1/+1 counters on it.\nAs this creature is turned face up, put three +1/+1 counters on it.",Some((1,1))),
        ("Uncover","{0}","Instant","Turn target creature face up.",None)]);
    put(&mut t,0,"Hardened Scales",Zone::Battlefield);let o=face_down(&mut t,"Growing Beast");
    let card=put(&mut t,0,"Uncover",Zone::Hand);t.g.act(0,Move::Cast{card,targets:vec![vec![o.into()]]}).unwrap();t.resolve();
    cite!(["708.11","614.5"];assert_eq!(counters(&t,o),7,"(2+1)+(3+1), no paid megamorph instruction"));
}
