//! Counter-placement bans govern effects and positive costs, not removal or damage itself.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
fn card(name: &str, kind: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    let d = CardDef::new(name, "{0}", kind, text, pt).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
    d
}
fn pass(g: &mut Game) {
    for _ in 0..2 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn act(g: &mut Game, s: ObjId, a: usize, paying: Vec<Vec<ObjId>>) -> Result<(), String> {
    g.act(0, Move::Activate { source: s, ability: a, targets: vec![], paying })
}
#[test]
fn r101_2_counter_ban_precedes_replacement_choices_and_does_not_prevent_removal() {
    let mut g = Game::new(
        vec![
            card("Bear", "Creature", "{0}: Put two +1/+1 counters on this creature.", Some((2, 2))),
            card("Ban", "Enchantment", "Creatures can't have counters put on them.", None),
            card(
                "Double",
                "Enchantment",
                "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.",
                None,
            ),
            card(
                "Plus",
                "Enchantment",
                "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.",
                None,
            ),
        ],
        2,
        20,
    );
    let s = g.put(0, 0, Zone::Battlefield);
    g.put_counters(s, "+1/+1", 3);
    for d in [1, 2, 3] {
        g.put(0, d, Zone::Battlefield);
    }
    act(&mut g, s, 0, vec![]).unwrap();
    pass(&mut g);
    cite!(["101.2","614.1"];assert!(g.decision.is_none()));
    assert_eq!(g.obj(s).counters.get("+1/+1"), Some(&3));
    cite!(["122.1"];assert_eq!(g.remove_counters(s,"+1/+1",3),3));
}
#[test]
fn r118_3_positive_loyalty_is_forbidden_but_zero_and_removal_are_payable() {
    let walker = CardDef::planeswalker(
        "Walker",
        "{0}",
        "Planeswalker — Jace",
        "+1: You gain 1 life.\n0: You gain 2 life.\n−1: You gain 3 life.",
        3,
    )
    .unwrap();
    let mut g =
        Game::new(vec![walker, card("Ban", "Enchantment", "Permanents can't have counters put on them.", None)], 2, 20);
    let s = g.put(0, 0, Zone::Battlefield);
    g.put_counters(s, "loyalty", 3);
    g.put(0, 1, Zone::Battlefield);
    cite!(["606.4","118.3","101.2"]; refused("you can't put loyalty counters on"): act(&mut g,s,0,vec![]));
    cite!(["118.3"]; assert!(g.stack.is_empty() && g.players[0].life == 20 && g.priority == 0));
    assert_eq!(g.obj(s).counters.get("loyalty"), Some(&3));
    act(&mut g, s, 1, vec![]).unwrap();
    pass(&mut g);
    assert_eq!(g.players[0].life, 22);
    let mut g = Game::new(g.defs.clone(), 2, 20);
    let s = g.put(0, 0, Zone::Battlefield);
    g.put_counters(s, "loyalty", 3);
    g.put(0, 1, Zone::Battlefield);
    act(&mut g, s, 2, vec![]).unwrap();
    pass(&mut g);
    cite!(["606.4"];assert_eq!(g.obj(s).counters.get("loyalty"),Some(&2)));
}
#[test]
fn active_player_counter_placement_ban_remains_refused() {
    let d = CardDef::new("Active ban", "{0}", "Enchantment", "Players can't put counters on permanents.", None);
    // Uncited (grammar): this active-player phrasing has no admitted syntax.
    crate::support::refused(d, "text the grammar does not read");
}
#[test]
fn r701_68_zero_blight_still_chooses_a_creature_but_ignores_placement_ban() {
    for n in [0, 1] {
        let d = card("Blighter", "Creature", &format!("Blight {n}: You gain 1 life."), Some((3, 3)));
        let mut g =
            Game::new(vec![d, card("Ban", "Enchantment", "Creatures can't have counters put on them.", None)], 2, 20);
        let s = g.put(0, 0, Zone::Battlefield);
        g.put(0, 1, Zone::Battlefield);
        let result = act(&mut g, s, 0, vec![vec![s]]);
        cite!(["701.68a","701.68b","118.3"];assert_eq!(result.is_ok(),n==0));
        if n == 0 {
            pass(&mut g);
            assert_eq!(g.players[0].life, 21);
        }
        assert!(g.obj(s).counters.is_empty());
    }
}
#[test]
fn r120_3_forbidden_infect_counters_do_not_prevent_damage_or_lifelink() {
    let mut g = Game::new(
        vec![
            card(
                "Source",
                "Creature",
                "Infect\nLifelink\n{0}: This creature deals 2 damage to target creature.",
                Some((2, 2)),
            ),
            card("Bear", "Creature", "", Some((3, 3))),
            card("Ban", "Enchantment", "Creatures can't have counters put on them.", None),
        ],
        2,
        20,
    );
    let s = g.put(0, 0, Zone::Battlefield);
    let bear = g.put(1, 1, Zone::Battlefield);
    g.put(0, 2, Zone::Battlefield);
    g.act(0, Move::Activate { source: s, ability: 2, targets: vec![vec![Target::Object(bear)]], paying: vec![] })
        .unwrap();
    pass(&mut g);
    cite!(["120.3d","120.3f","101.2"];assert!(g.obj(bear).counters.is_empty()));
    assert_eq!(g.players[0].life, 22);
    assert!(
        g.log
            .iter()
            .any(|e| matches!(e,Event::DamageDealt{source,to:Target::Object(o),amount:2,..}if *source==s&&*o==bear))
    );
}
#[test]
fn r603_2_counter_cost_captures_watcher_before_later_sacrifice_cost() {
    let mut g = Game::new(
        vec![
            card("Blighter", "Creature", "Blight 1, Sacrifice a creature: You gain 1 life.", Some((4, 4))),
            card(
                "Watcher",
                "Creature",
                "Whenever one or more -1/-1 counters are put on this creature, you gain 2 life.",
                Some((4, 4)),
            ),
        ],
        2,
        20,
    );
    let s = g.put(0, 0, Zone::Battlefield);
    let watcher = g.put(0, 1, Zone::Battlefield);
    act(&mut g, s, 0, vec![vec![watcher], vec![watcher]]).unwrap();
    cite!(["603.2","113.7a"];assert_eq!(g.stack.len(),2));
    pass(&mut g);
    pass(&mut g);
    assert_eq!(g.players[0].life, 23);
}
#[test]
fn native_blossombind_full_card_remains_refused_for_untapping_clause() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let d = CardDef::from_faces(&catalog["Blossombind"]).unwrap();
    // This stage owns counter placement, not the separate prohibition on becoming untapped.
    // Uncited (admission): the full card's untap prohibition remains unavailable.
    crate::support::refused(game::play::check(&d).map_err(|e|e.0), "prohibition: Untap");
}
#[test]
fn r101_2_attached_holder_ban_forces_endure_token_and_entry_ban_prevents_counters() {
    let mut g = Game::new(
        vec![
            card("Endurer", "Creature", "{0}: This creature endures 2.", Some((2, 2))),
            card(
                "Aura",
                "Enchantment — Aura",
                "Enchant creature\nEnchanted creature can't have counters put on it.",
                None,
            ),
        ],
        2,
        20,
    );
    let s = g.put(0, 0, Zone::Battlefield);
    let aura = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card: aura, targets: vec![vec![Target::Object(s)]] }).unwrap();
    pass(&mut g);
    act(&mut g, s, 0, vec![]).unwrap();
    pass(&mut g);
    cite!(["101.2","701.63a"];assert!(g.decision.is_none()));
    assert!(g.obj(s).counters.is_empty());
    assert!(g.log.iter().any(|e| matches!(e, Event::Created { .. })));
    let entrant = card(
        "Entrant",
        "Creature",
        "This creature enters with two +1/+1 counters on it.\nThis creature can't have counters put on it.",
        Some((2, 2)),
    );
    let mut g = Game::new(vec![entrant], 2, 20);
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    pass(&mut g);
    let s = g.battlefield[0];
    cite!(["101.2","614.12","122.6a"];assert!(g.obj(s).counters.is_empty()));
}
