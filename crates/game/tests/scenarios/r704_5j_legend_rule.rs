//! 704.5j, the legend rule, and the legendary tokens the grammar's named-token rules let the
//! engine run; and the "Staff of the … Magus" triggers (601.2, 305.1 events, once each).

use crate::support::{cite, paid};
use game::card::CardDef;
use game::play::check;
use game::{Game, Move, ObjId, Zone};
use ir::Supertype;

const FOREST: usize = 0;
const TOLSIMIR: usize = 1;
const JIANG: usize = 2;
const WOLF_MAKER: usize = 3;
const LEGEND: usize = 4;

fn card(name: &str, cost: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    let d = CardDef::new(name, cost, types, text, pt).expect(name);
    check(&d).unwrap_or_else(|u| panic!("{name}: {}", u.0));
    d
}

fn game() -> Game {
    Game::new(
        vec![
            card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
            // the real texts of the cards
            card(
                "Tolsimir Wolfblood",
                "{4}{G}{W}",
                "Legendary Creature — Elf Warrior",
                "Other green creatures you control get +1/+1.\nOther white creatures you control get +1/+1.\n{T}: Create Voja, a legendary 2/2 green and white Wolf creature token.",
                Some((3, 4)),
            ),
            card(
                "Jiang Yanggu, Never Alone",
                "{3}{G}",
                "Legendary Creature — Human Druid",
                "When Jiang Yanggu enters, create Mowu, a legendary 3/3 green Dog creature token.\nAt the beginning of your end step, untap all tokens you control.",
                Some((2, 2)),
            ),
            // another source of the same token, so that one player controls two
            card(
                "Wolf Maker",
                "{0}",
                "Creature — Elf",
                "{T}: Create Voja, a legendary 2/2 green and white Wolf creature token.",
                Some((1, 1)),
            ),
            card("Legend", "{0}", "Legendary Creature — Elf", "", Some((2, 2))),
        ],
        2,
        20,
    )
}

fn pass_both(g: &mut Game) {
    let first = g.priority;
    g.act(first, Move::Pass).unwrap();
    g.act(1 - first, Move::Pass).unwrap();
}

fn named(g: &Game, name: &str) -> Vec<ObjId> {
    g.battlefield.iter().copied().filter(|o| g.obj(*o).chars.name == name).collect()
}

fn activate(g: &mut Game, source: ObjId, ability: usize) {
    g.act(0, Move::Activate { source, ability, targets: vec![], paying: vec![] }).unwrap();
    pass_both(g);
}

/// (c) A created legendary token has the legendary supertype and the name the text gives.
#[test]
fn r111_4_a_created_legendary_token_is_legendary_and_named() {
    let mut g = game();
    let tolsimir = g.put(0, TOLSIMIR, Zone::Battlefield);
    activate(&mut g, tolsimir, 2);
    let [voja] = named(&g, "Voja")[..] else { panic!("one token named Voja: {:?}", named(&g, "Voja")) };
    assert!(g.obj(voja).token());
    cite!(["111.3"]; assert!(g.obj(voja).chars.supertypes.contains(&Supertype::Legendary)));
    cite!(["111.3"]; assert_eq!(g.obj(voja).chars.pt, Some((2, 2))));
}

#[test]
fn r111_4_jiang_yanggus_token_is_legendary_and_named() {
    let mut g = game();
    for _ in 0..4 {
        let f = g.put(0, FOREST, Zone::Battlefield);
        g.act(0, Move::Activate { source: f, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
    let jiang = g.put(0, JIANG, Zone::Hand);
    g.act(0, paid("GGGG", Move::Cast { card: jiang, targets: vec![] })).unwrap();
    pass_both(&mut g);
    pass_both(&mut g);
    let [mowu] = named(&g, "Mowu")[..] else { panic!("one token named Mowu") };
    cite!(["111.3"]; assert!(g.obj(mowu).chars.supertypes.contains(&Supertype::Legendary)));
}

/// (a) 704.5j with ordinary cards: the player keeps one, the other goes to its owner's graveyard.
#[test]
fn r704_5j_two_legendary_cards_with_the_same_name_leave_one() {
    let mut g = game();
    g.put(0, LEGEND, Zone::Battlefield);
    g.put(0, LEGEND, Zone::Battlefield);
    g.state_based_actions();
    let keep = named(&g, "Legend")[1];
    g.act(0, Move::Choose(game::state::Answer::Objects(vec![keep]))).unwrap();
    cite!(["704.5j"]; assert_eq!(named(&g, "Legend"), vec![keep]));
    assert_eq!(named(&g, "Legend").len(), 1, "the rest are put into their owners' graveyards");
    cite!(["704.5j"]; assert_eq!(g.players[0].graveyard.len(), 1));
}

/// 704.5j is per controller: two players may each control one.
#[test]
fn r704_5j_legendary_permanents_of_different_controllers_both_stay() {
    let mut g = game();
    g.put(0, LEGEND, Zone::Battlefield);
    g.put(1, LEGEND, Zone::Battlefield);
    g.state_based_actions();
    cite!(["704.5j"]; assert_eq!(named(&g, "Legend").len(), 2));
    cite!(["704.5j"]; assert!(g.decision.is_none(), "different controllers must not be asked to keep one"));
}

/// (b) 704.5j with two legendary tokens of the same name from real card text.
#[test]
fn r704_5j_two_legendary_tokens_with_the_same_name_leave_one() {
    let mut g = game();
    let tolsimir = g.put(0, TOLSIMIR, Zone::Battlefield);
    let maker = g.put(0, WOLF_MAKER, Zone::Battlefield);
    activate(&mut g, tolsimir, 2);
    activate(&mut g, maker, 0);
    let keep = named(&g, "Voja")[0];
    g.act(0, Move::Choose(game::state::Answer::Objects(vec![keep]))).unwrap();
    cite!(["704.5j"]; assert_eq!(named(&g, "Voja").len(), 1, "the player chooses one, the rest are put into the graveyard"));
}

/// The batch retains both permanents until the controller has made the required choice.
#[test]
fn r704_5j_no_permanent_leaves_before_the_controller_chooses() {
    let mut g = game();
    let tolsimir = g.put(0, TOLSIMIR, Zone::Battlefield);
    let maker = g.put(0, WOLF_MAKER, Zone::Battlefield);
    activate(&mut g, tolsimir, 2);
    activate(&mut g, maker, 0);
    cite!(["704.5j"]; assert_eq!(named(&g, "Voja").len(), 2));
    cite!(["704.5j"]; assert!(matches!(g.decision, Some(game::state::Decision::Choose { player: 0, .. }))));
    let before = format!("{g:?}");
    cite!(["704.3"]; refused("a decision is pending: a choice a state-based action asks for (704.3)"): g.act(0, Move::Pass));
    cite!(["704.5j"]; refused("that decision is another player's"): g.act(1, Move::Choose(game::state::Answer::Objects(vec![named(&g, "Voja")[0]]))));
    cite!(["704.5j"]; refused("choose exactly 1 of them to choose (608.2d)"): g.act(0, Move::Choose(game::state::Answer::Objects(vec![]))));
    assert_eq!(format!("{g:?}"), before);
}

// ---------------------------------------------------------------- Staff of the … Magus

const STAFFS: [(&str, &str, &str, &str, &str); 5] = [
    ("Staff of the Mind Magus", "blue", "Island", "{U}", "U"),
    ("Staff of the Wild Magus", "green", "Forest", "{G}", "G"),
    ("Staff of the Sun Magus", "white", "Plains", "{W}", "W"),
    ("Staff of the Death Magus", "black", "Swamp", "{B}", "B"),
    ("Staff of the Flame Magus", "red", "Mountain", "{R}", "R"),
];

#[test]
fn r603_2_staff_of_the_magus_triggers_once_for_each_of_its_two_events() {
    for (name, color, land, cost, sym) in STAFFS {
        let text = format!("Whenever you cast a {color} spell or a {land} you control enters, you gain 1 life.");
        let defs = vec![
            card(name, "{3}", "Artifact", &text, None),
            card(land, "", &format!("Basic Land — {land}"), &format!("({{T}}: Add {{{sym}}}.)"), None),
            card("Spell", cost, "Creature — Bear", "", Some((1, 1))),
            card("Other Spell", "{0}", "Creature — Bear", "", Some((1, 1))),
            card("Wastes", "", "Basic Land", "{T}: Add {C}.", None),
        ];
        let mut g = Game::new(defs, 2, 20);
        g.put(0, 0, Zone::Battlefield);
        // cast a spell of the color: once
        let l = g.put(0, 1, Zone::Battlefield);
        g.act(0, Move::Activate { source: l, ability: 0, targets: vec![], paying: vec![] }).unwrap();
        let spell = g.put(0, 2, Zone::Hand);
        g.act(0, paid(sym, Move::Cast { card: spell, targets: vec![] })).unwrap();
        while !g.stack.is_empty() {
            pass_both(&mut g);
        }
        cite!(["603.2", "603.2c"]; case: name; assert_eq!(g.players[0].life, 21, "{name}: casting a {color} spell gains 1 life once"));
        // play the land: once
        let l = g.put(0, 1, Zone::Hand);
        g.act(0, Move::PlayLand(l)).unwrap();
        while !g.stack.is_empty() {
            pass_both(&mut g);
        }
        cite!(["603.2", "603.2c"]; case: name; assert_eq!(g.players[0].life, 22, "{name}: a {land} entering gains 1 life once"));
        // neither another spell nor another land
        let spell = g.put(0, 3, Zone::Hand);
        g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
        while !g.stack.is_empty() {
            pass_both(&mut g);
        }
        cite!(["603.2"]; case: name; assert_eq!(g.players[0].life, 22, "{name}: a spell of another color does nothing"));
        let other = g.put(0, 4, Zone::Hand);
        g.move_object(other, Zone::Battlefield);
        pass_both(&mut g);
        assert!(g.stack.is_empty());
        assert!(g.decision.is_none());
        cite!(["603.2"]; case: name; assert_eq!(g.players[0].life, 22, "{name}: a land without the named subtype does nothing"));
    }
}

#[test]
fn r101_4_legend_choices_are_apnap_public_and_committed_together() {
    use game::state::{Answer, Choice, Decision};
    let defs = vec![card("Legend", "{0}", "Legendary Creature", "", Some((2, 2)))];
    let mut g = Game::new(defs, 3, 20);
    g.active = 1;
    let groups: Vec<Vec<_>> = (0..3).map(|p| (0..2).map(|_| g.put(p, 0, Zone::Battlefield)).collect()).collect();
    assert!(g.state_based_actions());
    for (i, p) in [1, 2, 0].into_iter().enumerate() {
        cite!(["101.4", "704.5j"]; assert!(matches!(&g.decision, Some(Decision::Choose { player, choice: Choice::Objects { among, count: 1, .. } })
            if *player == p && *among == groups[p])));
        cite!(["101.4"]; assert_eq!(g.battlefield.len(), 6, "no earlier player's choice commits early"));
        assert_eq!(g.public_choices().count(), i);
        let mut cloned = g.clone();
        let answer = Move::Choose(Answer::Objects(vec![groups[p][1]]));
        g.act(p, answer.clone()).unwrap();
        cloned.act(p, answer).unwrap();
        assert_eq!(format!("{g:?}"), format!("{cloned:?}"));
    }
    cite!(["101.4", "704.5j"]; assert_eq!(g.battlefield.len(), 3));
    for (p, group) in groups.iter().enumerate() {
        cite!(["704.5j"]; assert!(g.battlefield.contains(&group[1])));
        assert_eq!(g.players[p].graveyard.len(), 1);
    }
    assert!(g.decision.is_none());
}

#[test]
fn r101_4c_controller_can_choose_legend_name_groups_in_either_order() {
    use game::state::{Answer, Choice, Decision};
    for reversed in [false, true] {
        let defs = vec![
            card("First", "{0}", "Legendary Creature", "", Some((2, 2))),
            card("Second", "{0}", "Legendary Creature", "", Some((2, 2))),
        ];
        let mut g = Game::new(defs, 2, 20);
        let groups: Vec<Vec<_>> = (0..2).map(|d| (0..2).map(|_| g.put(0, d, Zone::Battlefield)).collect()).collect();
        g.state_based_actions();
        let order = if reversed { [1, 0] } else { [0, 1] };
        for (i, group) in order.into_iter().enumerate() {
            let Some(Decision::Choose { choice: Choice::Objects { among, .. }, .. }) = &g.decision else { panic!("legend choice") };
            assert_eq!(among.len(), 4 - i * 2);
            cite!(["101.4c"]; assert!(among.contains(&groups[group][0])));
            g.act(0, Move::Choose(Answer::Objects(vec![groups[group][0]]))).unwrap();
        }
        cite!(["404.3"]; ok: g.act(0,Move::Choose(Answer::Objects(vec![groups[0][1],groups[1][1]]))));
        cite!(["101.4c", "704.5j"]; assert_eq!(g.battlefield, vec![groups[0][0], groups[1][0]]));
    }
}

#[test]
fn r704_3_legend_and_lethal_damage_use_the_same_retained_snapshot() {
    use game::{Target, state::Answer};
    let defs = vec![
        card("Leader", "{0}", "Legendary Creature", "Other creatures you control get +1/+1.", Some((2, 2))),
        card("Follower", "{0}", "Creature", "", Some((1, 1))),
    ];
    let mut g = Game::new(defs, 2, 20);
    let first = g.put(0, 0, Zone::Battlefield);
    let second = g.put(0, 0, Zone::Battlefield);
    let follower = g.put(0, 1, Zone::Battlefield);
    g.damage(first, Target::Object(follower), 3);
    let before = g.log.len();
    g.state_based_actions();
    assert_eq!(g.log.len(), before);
    assert_eq!(g.obj(follower).zone, Zone::Battlefield);
    g.act(0, Move::Choose(Answer::Objects(vec![second]))).unwrap();
    cite!(["404.3"]; ok: g.act(0,Move::Choose(Answer::Objects(vec![first,follower]))));
    cite!(["704.3", "704.5j", "704.5g"]; assert_eq!(g.battlefield, vec![second]));
    cite!(["704.3"]; assert_eq!(g.power(follower), Some(3), "last-known power includes both simultaneous departing sources"));
    assert_eq!(g.players[0].graveyard.len(), 2);
}

#[test]
fn r704_5j_keeping_a_lethally_damaged_legend_does_not_save_either_permanent() {
    use game::{Target, state::Answer};
    let mut g = game();
    let first = g.put(0, LEGEND, Zone::Battlefield);
    let second = g.put(0, LEGEND, Zone::Battlefield);
    g.damage(first, Target::Object(first), 2);
    g.regenerate(second); // legend movement is not destruction
    g.state_based_actions();
    g.act(0, Move::Choose(Answer::Objects(vec![first]))).unwrap();
    cite!(["404.3"]; ok: g.act(0,Move::Choose(Answer::Objects(vec![first,second]))));
    cite!(["704.5j", "704.5g"]; assert!(g.battlefield.is_empty()));
    assert_eq!(g.players[0].graveyard.len(), 2);
    let deaths = g.log.iter().filter(|event| matches!(event,
        game::Event::ZoneChanged { from: Zone::Battlefield, to: Zone::Graveyard, .. })).count();
    assert_eq!(deaths, 2, "no duplicate movement from overlapping state-based actions");
}
