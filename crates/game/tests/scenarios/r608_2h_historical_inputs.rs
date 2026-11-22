//! Historical predicates read the object/time named by the completed instruction
//! (608.2h/i, 400.7), rather than a new incarnation that happens to share its card.
use crate::support::{Board, T, cite, mana_pool, oracle, paid};
use game::{Move, ObjId, Target, Zone};

fn setup(target: &str) -> T {
    Board::new(2).battlefield(0, &["Scavenging Ooze"])
        .graveyard(1, &[target]).hand(0, &["Forest", "Cloudshift"]).build()
}
fn activate(t: &mut T, target: ObjId) {
    let source = t.find_in(0, Zone::Battlefield, "Scavenging Ooze");
    t.g.act(0, paid("G", Move::Activate { source, ability: 0,
        targets: vec![vec![Target::Object(target)]], paying: vec![] })).unwrap();
}

#[test]
fn r608_2h_scavenging_ooze_reads_whether_its_input_was_a_creature_card() {
    for (name, creature) in [("Grizzly Bears", true), ("Island", false)] {
        let mut t = setup(name);
        let source = t.find_in(0, Zone::Battlefield, "Scavenging Ooze");
        let target = t.find_in(1, Zone::Graveyard, name);
        t.g.players[0].pool = mana_pool("G");
        activate(&mut t, target);
        let mut fork = T::new(t.g.clone());
        for t in [&mut t, &mut fork] {
            t.pass_all();
            cite!(["400.7"]; assert_eq!(t.g.obj(target).zone, Zone::Gone));
            assert_eq!(t.g.obj(t.g.exile[0]).chars.name, name);
            // Oracle: only a creature card yields the counter and 1 life.
            cite!(["608.2h", "122.1a"]; assert_eq!(t.g.obj(source).counters.get("+1/+1").copied().unwrap_or(0), u32::from(creature)));
            cite!(["608.2h", "119.3"]; assert_eq!(t.g.players[0].life, 20 + i32::from(creature)));
        }
    }
}

#[test]
fn r608_2b_only_the_first_ooze_activation_to_resolve_has_an_effect_on_the_same_card() {
    let mut t = setup("Grizzly Bears");
    let target = t.find_in(1, Zone::Graveyard, "Grizzly Bears");
    let source = t.find_in(0, Zone::Battlefield, "Scavenging Ooze");
    t.g.players[0].pool = mana_pool("GG");
    activate(&mut t, target);
    activate(&mut t, target);
    t.pass_all();
    t.pass_all();
    // Pinned Ooze ruling 6583d0a2930ccdf0: the remaining activation has an illegal target.
    cite!(["608.2b"]; assert!(t.g.stack.is_empty() && t.g.exile.len() == 1));
    cite!(["608.2b"]; assert_eq!(t.g.obj(source).counters.get("+1/+1").copied(), Some(1)));
    cite!(["608.2b"]; assert_eq!(t.g.players[0].life, 21));
}

#[test]
fn r400_7_a_returned_ooze_is_not_the_source_of_the_pending_ability() {
    let mut t = setup("Grizzly Bears");
    let source = t.find_in(0, Zone::Battlefield, "Scavenging Ooze");
    let target = t.find_in(1, Zone::Graveyard, "Grizzly Bears");
    let blink = t.find_in(0, Zone::Hand, "Cloudshift");
    t.g.players[0].pool = mana_pool("GW");
    activate(&mut t, target);
    t.g.act(0, paid("W", Move::Cast { card: blink, targets: vec![vec![Target::Object(source)]] })).unwrap();
    t.pass_all();
    let returned = t.find_in(0, Zone::Battlefield, "Scavenging Ooze");
    cite!(["400.7"]; assert_ne!(returned, source));
    t.pass_all();
    cite!(["400.7", "609.3"]; assert!(t.g.obj(returned).counters.is_empty()));
    cite!(["113.7a", "608.2h"]; assert_eq!(t.g.players[0].life, 21));
    assert_eq!(t.g.obj(t.g.exile[0]).chars.name, "Grizzly Bears");
}

#[test]
fn r115_1_an_ooze_activation_cannot_target_a_card_in_the_hand() {
    let mut t = setup("Grizzly Bears");
    let target = t.find_in(0, Zone::Hand, "Forest");
    let source = t.find_in(0, Zone::Battlefield, "Scavenging Ooze");
    t.g.players[0].pool = mana_pool("G");
    let before = format!("{:?}", t.g);
    cite!(["115.1"]; refused("an illegal target, Forest: it isn't in a graveyard (115.1)"):
        t.g.act(0, paid("G", Move::Activate { source, ability: 0,
            targets: vec![vec![Target::Object(target)]], paying: vec![] })));
    assert_eq!(format!("{:?}", t.g), before);
}

#[test]
fn r608_2b_reanimating_the_target_before_ooze_resolves_does_not_retarget_it() {
    let mut t = Board::new(2).battlefield(0, &["Scavenging Ooze", "Vedalken Orrery"])
        .graveyard(1, &["Grizzly Bears"]).hand(0, &["Reanimate"]).build();
    let target = t.find_in(1, Zone::Graveyard, "Grizzly Bears");
    let spell = t.find_in(0, Zone::Hand, "Reanimate");
    t.g.players[0].pool = mana_pool("GB");
    activate(&mut t, target);
    t.g.act(0, paid("B", Move::Cast { card: spell, targets: vec![vec![target.into()]] })).unwrap();
    t.pass_all();
    let bear = t.find_in(0, Zone::Battlefield, "Grizzly Bears");
    cite!(["400.7"]; assert_ne!(bear, target));
    t.pass_all();
    cite!(["608.2b"]; assert!(t.g.exile.is_empty()));
    cite!(["608.2b"]; assert_eq!(t.g.players[0].life, 18)); // Reanimate alone lost 2 life.
}

#[test]
fn r608_2h_cling_to_dust_uses_the_exiled_inputs_type_for_its_two_branches() {
    for (target, creature) in [("Grizzly Bears", true), ("Island", false)] {
        let mut t = Board::new(2).hand(0, &["Cling to Dust"])
            .graveyard(1, &[target]).library(0, &["Forest"]).build();
        let spell = t.find_in(0, Zone::Hand, "Cling to Dust");
        let target = t.find_in(1, Zone::Graveyard, target);
        t.g.players[0].pool = mana_pool("B");
        t.g.act(0, paid("B", Move::Cast { card: spell, targets: vec![vec![target.into()]] })).unwrap();
        t.pass_all();
        cite!(["608.2h", "119.3"]; assert_eq!(t.g.players[0].life, if creature {23} else {20}));
        cite!(["608.2h", "121.1"]; assert_eq!(t.g.players[0].hand.len(), usize::from(!creature)));
    }
}

#[test]
fn r608_2h_generals_enforcer_creates_a_soldier_only_for_a_creature_card() {
    for (name, creature) in [("Grizzly Bears", true), ("Island", false)] {
        let mut t = Board::new(2).battlefield(0, &["General's Enforcer"])
            .graveyard(1, &[name]).build();
        let source = t.find_in(0, Zone::Battlefield, "General's Enforcer");
        let target = t.find_in(1, Zone::Graveyard, name);
        t.g.players[0].pool = mana_pool("WWBB");
        t.g.act(0, paid("WWBB", Move::Activate {source, ability: 1,
            targets: vec![vec![target.into()]], paying: vec![]})).unwrap();
        t.pass_all();
        let tokens: Vec<_> = t.g.battlefield.iter().copied().filter(|&o| t.g.obj(o).token()).collect();
        cite!(["608.2h"]; assert_eq!(tokens.len(), usize::from(creature)));
        if let Some(&token) = tokens.first() {
            cite!(["111.3"]; assert_eq!((t.g.power(token), t.g.toughness(token)), (Some(1), Some(1))));
            cite!(["111.3"]; assert_eq!(t.g.obj(token).chars.colors, vec![ir::Color::White]));
            let subtypes = &t.g.obj(token).chars.subtypes;
            cite!(["111.3"]; assert!(subtypes.contains(&ir::Subtype::from_name("Human").unwrap())
                && subtypes.contains(&ir::Subtype::from_name("Soldier").unwrap())));
        }
    }
}

#[test]
fn r608_2i_a_target_without_a_preceding_movement_does_not_prove_a_past_time() {
    // Synthetic boundary: announcing a target supplies an identity, not an earlier state.
    let def = game::CardDef::new("Unproved history", "{0}", "Instant",
        "Tap target creature. If it was a creature card, you gain 1 life.", None).unwrap();
    cite!(["608.2i"]; assert_eq!(game::play::check(&def).unwrap_err().0, "past condition: no retained movement input"));
    // The complete pinned witnesses, rather than only extracted effects, must admit.
    for name in ["Scavenging Ooze", "Cling to Dust"] {
        game::play::check(&oracle(name)).unwrap();
    }
}

#[test]
fn r608_2h_a_suspended_entry_keeps_the_original_cards_color_in_a_cloned_frame() {
    // Authored family boundary: a blue Clone enters as a green Grizzly Bears. The past
    // question is of the graveyard card, independently of the chosen entry copy (707.2).
    for (tail, exiled) in [("If it was a blue card, you gain 1 life.", false),
        ("Exile it. If it was a green card, you gain 1 life.", true)] {
        let spell = game::CardDef::new("Return and remember", "{0}", "Instant",
            &format!("Return target creature card from your graveyard to the battlefield. {tail}"), None).unwrap();
        let defs = vec![spell, oracle("Clone"), oracle("Grizzly Bears")];
        for def in &defs { game::play::check(def).unwrap(); }
        let mut g = game::Game::new(defs, 2, 20);
        let spell = g.put(0, 0, Zone::Hand);
        let original = g.put(0, 1, Zone::Graveyard);
        let model = g.put(1, 2, Zone::Battlefield);
        g.act(0, Move::Cast {card: spell, targets: vec![vec![original.into()]]}).unwrap();
        let mut t = T::new(g);
        t.pass_all();
        cite!(["614.12"]; assert!(matches!(t.g.decision, Some(game::state::Decision::Choose {..}))));
        assert_eq!(t.g.players[0].life, 20);
        let mut fork = t.g.clone();
        for g in [&mut t.g, &mut fork] {
            g.act(0, Move::Choose(game::state::Answer::Objects(vec![model]))).unwrap();
            cite!(["608.2h", "608.2i"]; assert_eq!(g.players[0].life, 21));
            if exiled {
                // The second movement's input was a green copy, although the physical
                // card now in exile is blue again. Neither other incarnation is the past.
                cite!(["400.7", "707.2"]; assert_eq!(g.obj(g.exile[0]).chars.name, "Clone"));
            } else {
                let returned = *g.battlefield.iter().find(|&&o| o != model).unwrap();
                cite!(["400.7", "707.2"]; assert_eq!(g.obj(returned).chars.name, "Grizzly Bears"));
                assert_ne!(returned, original);
            }
        }
    }
}
