//! Planeswalkers and loyalty abilities (306, 606): a planeswalker enters with its printed
//! loyalty as counters (306.5b); a loyalty ability's cost puts counters on it or removes them
//! (606.4), and is activated as a sorcery could be cast, once a turn for the permanent (606.3);
//! damage removes loyalty (306.8) and none left puts it into the graveyard (704.5i).
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const GARRUK: usize = 0;
const FOREST: usize = 1;
const BOLT: usize = 2;
const BEAR: usize = 3;

const WILDSPEAKER: &str = "+1: Untap two target lands.\n−1: Create a 3/3 green Beast creature token.\n−4: Creatures you control get +3/+3 and gain trample until end of turn.";

fn game() -> Game {
    let defs = vec![
        CardDef::planeswalker("Garruk Wildspeaker", "{2}{G}{G}", "Legendary Planeswalker — Garruk", WILDSPEAKER, 3).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Lightning Bolt", "{R}", "Instant", "Lightning Bolt deals 3 damage to any target.", None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    Game::new(defs, 2, 20)
}
fn pass_all(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn loyalty(g: &Game, o: ObjId) -> u32 { g.obj(o).counters.get("loyalty").copied().unwrap_or(0) }
fn activate(g: &mut Game, source: ObjId, ability: usize, targets: Vec<Vec<Target>>) -> Result<(), String> {
    g.act(0, Move::Activate { source, ability, targets, paying: vec![] })
}
/// Garruk was cast and has resolved. Returns him and two tapped Forests.
fn resolved() -> (Game, ObjId, [ObjId; 2]) {
    let mut g = game();
    let lands = [g.put(0, FOREST, Zone::Battlefield), g.put(0, FOREST, Zone::Battlefield)];
    let card = g.put(0, GARRUK, Zone::Hand);
    g.players[0].pool.colors[4] = 4;
    g.act(0, paid("GGGG", Move::Cast { card, targets: vec![] })).unwrap();
    pass_all(&mut g);
    let garruk = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(GARRUK)).expect("Garruk entered");
    for land in lands { g.set_tapped(land, true); }
    (g, garruk, lands)
}

#[test]
fn r306_5b_a_planeswalker_enters_with_its_printed_loyalty() {
    let (g, garruk, _) = resolved();
    cite!(["306.5b"]; assert_eq!(loyalty(&g, garruk), 3));
    // a planeswalker without a printed loyalty is not run
    let blank = CardDef::new("Blank", "{1}", "Legendary Planeswalker — Garruk", "+1: You gain 1 life.", None).unwrap();
    assert!(game::play::check(&blank).unwrap_err().0.contains("306.5b"));
}

#[test]
fn r606_4_the_cost_puts_or_removes_loyalty_counters() {
    let (mut g, garruk, lands) = resolved();
    activate(&mut g, garruk, 0, vec![lands.iter().map(|l| Target::Object(*l)).collect()]).unwrap();
    cite!(["606.4"]; assert_eq!(loyalty(&g, garruk), 4, "the cost is paid as the ability is activated"));
    assert_eq!(g.stack.len(), 1);
    pass_all(&mut g);
    assert!(lands.iter().all(|l| !g.obj(*l).tapped));

    let (mut g, garruk, _) = resolved();
    activate(&mut g, garruk, 1, vec![]).unwrap();
    cite!(["606.4"]; assert_eq!(loyalty(&g, garruk), 2));
    pass_all(&mut g);
    let beast = g.battlefield.iter().copied().find(|o| g.obj(*o).card.is_none()).expect("a Beast token");
    assert_eq!((g.power(beast), g.toughness(beast)), (Some(3), Some(3)));
}

#[test]
fn r606_6_more_counters_than_it_has_cannot_be_removed() {
    let (mut g, garruk, _) = resolved();
    let before = format!("{g:?}");
    cite!(["606.6"]; refused("Garruk Wildspeaker can't have 4 loyalty counters removed to pay this cost (606.6, 118.3, 118.10)"): activate(&mut g, garruk, 2, vec![]));
    assert_eq!(format!("{g:?}"), before);
}

#[test]
fn r606_3_one_loyalty_ability_of_a_permanent_each_turn_at_sorcery_timing() {
    let (mut g, garruk, lands) = resolved();
    activate(&mut g, garruk, 1, vec![]).unwrap();
    // not with the stack occupied, and not a second one this turn once it is empty
    cite!(["606.3"]; refused("a loyalty ability is activated in a main phase of your own turn with the stack empty (606.3)"): activate(&mut g, garruk, 0, vec![lands.iter().map(|l| Target::Object(*l)).collect()]));
    pass_all(&mut g);
    cite!(["606.3"]; refused("a loyalty ability of that permanent has already been activated this turn (606.3)"): activate(&mut g, garruk, 1, vec![]));
    cite!(["606.3"]; refused("a loyalty ability of that permanent has already been activated this turn (606.3)"): activate(&mut g, garruk, 0, vec![lands.iter().map(|l| Target::Object(*l)).collect()]));
    assert_eq!(loyalty(&g, garruk), 2);
    // The opponent has priority; control, not lack of priority, prevents activation.
    g.act(0, Move::Pass).unwrap();
    assert!(g.stack.is_empty());
    cite!(["602.2"]; refused("only an object's controller can activate its ability (602.2)"): g.act(1, Move::Activate { source: garruk, ability: 1, targets: vec![], paying: vec![] }));
    for player in 0..2 { g.put(player, FOREST, Zone::Library); } // One draw each on turns 2 and 3.
    let advance = |g: &mut Game, turn| {
        for _ in 0..200 {
            if g.turn == turn && g.part == ir::Part::PrecombatMain && g.priority == 0 && g.stack.is_empty() { return; }
            let action = match g.decision {
                Some(game::state::Decision::Attackers { .. }) => Move::Attack(vec![]),
                Some(game::state::Decision::Blockers { .. }) => Move::Block(vec![]),
                _ => Move::Pass,
            };
            g.act(g.priority, action).unwrap();
        }
        panic!("did not reach turn {turn}'s main phase with player 0's priority");
    };
    advance(&mut g, 2);
    assert_eq!(g.active, 1);
    cite!(["606.3"]; refused("a loyalty ability is activated in a main phase of your own turn with the stack empty (606.3)"): activate(&mut g, garruk, 1, vec![]));
    advance(&mut g, 3);
    assert_eq!(g.active, 0);
    cite!(["606.3"]; ok: activate(&mut g, garruk, 1, vec![]));
    assert_eq!(loyalty(&g, garruk), 1);
}

#[test]
fn r606_supplied_abilities_obey_available_loyalty_and_once_per_turn_limits() {
    let (mut g, garruk, lands) = resolved();
    for ability in 0..3 {
        let mut attempt = g.clone();
        let targets=if ability==0 {vec![lands.iter().copied().map(Target::Object).collect()]} else {vec![]};
        if ability<2 {
            cite!(["606.4"]; ok: activate(&mut attempt,garruk,ability,targets));
            cite!(["606.4"]; assert_eq!(loyalty(&attempt,garruk),if ability==0 {4} else {2}));
            assert_eq!(attempt.stack.len(),1);
        } else {
            cite!(["606.6"]; refused("Garruk Wildspeaker can't have 4 loyalty counters removed"): activate(&mut attempt,garruk,ability,targets));
            assert_eq!(format!("{attempt:?}"),format!("{g:?}"));
        }
    }
    activate(&mut g, garruk, 1, vec![]).unwrap();
    pass_all(&mut g);
    // Restore enough loyalty so usage this turn is the only failed precondition,
    // including for -4; +1 still receives its two legal land targets.
    g.set_initial_counters(garruk,"loyalty",4);
    for ability in 0..3 {
        let targets=if ability==0 {vec![lands.iter().copied().map(Target::Object).collect()]} else {vec![]};
        let before=format!("{g:?}");
        cite!(["606.3"]; case:ability; refused("a loyalty ability of that permanent has already been activated this turn (606.3)"): activate(&mut g,garruk,ability,targets));
        assert_eq!(format!("{g:?}"),before);
    }
}

#[test]
fn r306_8_damage_removes_loyalty_and_none_left_is_a_state_based_action() {
    let (mut g, garruk, _) = resolved();
    let bolt = g.put(0, BOLT, Zone::Hand);
    g.players[0].pool.colors[3] = 1;
    g.act(0, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Object(garruk)]] })).unwrap();
    pass_all(&mut g);
    cite!(["306.8", "704.5i"]; assert!(!g.battlefield.contains(&garruk)));
    cite!(["704.5i"]; assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).card == Some(GARRUK)), "put into its owner's graveyard (704.5i)"));
}

#[test]
fn r506_a_planeswalker_can_be_attacked() {
    let (mut g, garruk, _) = resolved();
    g.put(1, FOREST, Zone::Library); // The attacker's next draw step.
    let bear = g.put(1, BEAR, Zone::Battlefield);
    while g.active == 0 { g.act(g.priority, Move::Pass).unwrap(); }
    while g.asked().is_some_and(|(_, a)| !matches!(a, game::legal::Asked::Attackers(_))) { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(1, Move::Attack(vec![(bear, Target::Object(garruk))])).unwrap();
    while g.battlefield.contains(&garruk) && loyalty(&g, garruk) == 3 {
        let (player, asked) = g.asked().expect("the game goes on");
        match asked {
            game::legal::Asked::Blockers { .. } => g.act(player, Move::Block(vec![])).unwrap(),
            _ => g.act(player, Move::Pass).unwrap(),
        }
    }
    cite!(["506.3", "120.3c"]; assert_eq!((loyalty(&g, garruk), g.players[0].life), (1, 20), "combat damage to the planeswalker, not its controller"));
}
