//! A successful counter names its ability's source, and a source-present duration never restarts.
use crate::support::{cite, mana_pool, oracle, paid};
use game::{Game, Move, ObjId, Target, Zone};
use game::state::Decision;

const TIDE: usize = 0;
const SORCERER: usize = 1;
const STAFF: usize = 2;
const JACE: usize = 3;
const ZEALOT: usize = 4;
const OMEN: usize = 5;
const FOREST: usize = 6;
const UNSUMMON: usize = 7;
const DOMINATE: usize = 8;
const FLIGHT: usize = 9;
const COATING: usize = 10;

fn game() -> Game {
    let mut g = Game::new(["Tishana's Tidebinder", "Prodigal Sorcerer", "Staff of Domination", "Jace Beleren",
        "Dusk Legion Zealot", "Omen of the Sea", "Forest", "Unsummon", "Dominate", "Flight", "Liquimetal Coating"]
        .into_iter().map(oracle).collect::<Vec<_>>(), 2, 20);
    for p in 0..2 { for _ in 0..8 { g.put(p, FOREST, Zone::Library); } }
    g.active = 1; g.priority = 1; g.part = ir::Part::PrecombatMain;
    g
}
fn pass(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
fn priority(g: &mut Game, player: usize) { if g.priority != player { g.act(g.priority, Move::Pass).unwrap(); } }
fn cast(g: &mut Game, player: usize, def: usize, targets: Vec<Vec<Target>>, mana: &str) -> ObjId {
    priority(g, player);
    let card = g.put(player, def, Zone::Hand);
    g.players[player].pool = mana_pool(mana);
    g.act(player, paid(mana, Move::Cast { card, targets })).unwrap();
    *g.stack.last().unwrap()
}
fn activate(g: &mut Game, source: ObjId, targets: Vec<Vec<Target>>, mana: &str) -> ObjId {
    let player = g.controller(source);
    priority(g, player);
    g.players[player].pool = mana_pool(mana);
    g.act(player, paid(mana, Move::Activate { source, ability: 0, targets, paying: vec![] })).unwrap();
    *g.stack.last().unwrap()
}
fn announce_tide(g: &mut Game, ability: Option<ObjId>) -> ObjId {
    cast(g, 0, TIDE, vec![], "CCU");
    pass(g);
    let tide = *g.battlefield.iter().find(|id| g.obj(**id).card == Some(TIDE)).unwrap();
    cite!(["603.3d"]; assert!(matches!(g.decision, Some(Decision::Targets { player: 0, groups: 1, .. }))));
    g.act(0, Move::ChooseTargets(vec![ability.into_iter().map(Target::Object).collect()])).unwrap();
    tide
}
fn targets(id: ObjId) -> Vec<Vec<Target>> { vec![vec![Target::Object(id)]] }

#[test]
fn r701_6a_countered_activated_sources_lose_abilities_without_refunding_costs() {
    for def in [SORCERER, STAFF, JACE] {
        let mut g = game();
        let source = g.put(1, def, Zone::Battlefield);
        if def == JACE { g.set_initial_counters(source, "loyalty", 3); }
        let named = if def == SORCERER { vec![vec![Target::Player(0)]] } else { vec![] };
        let ability = activate(&mut g, source, named, if def == STAFF { "C" } else { "" });
        announce_tide(&mut g, Some(ability));
        pass(&mut g);
        cite!(["701.6a", "613.1f"]; case:format!("source={def}"); assert!(g.abilities(source).is_empty()));
        cite!(["701.6a"]; assert_eq!(g.players[0].life, 20));
        cite!(["701.6a"]; assert!(g.stack.is_empty()));
        if def == SORCERER { cite!(["701.6b"]; assert!(g.obj(source).tapped)); }
        if def == JACE { cite!(["701.6b", "606.4"]; assert_eq!(g.counters(source)["loyalty"], 5)); }
    }
}

#[test]
fn r701_6a_triggered_creature_loses_abilities_but_an_enchantment_does_not() {
    for def in [ZEALOT, OMEN] {
        let mut g = game();
        cast(&mut g, 1, def, vec![], if def == ZEALOT { "CB" } else { "CU" });
        pass(&mut g);
        let source = *g.battlefield.iter().find(|id| g.obj(**id).card == Some(def)).unwrap();
        let ability = g.stack[0];
        announce_tide(&mut g, Some(ability));
        pass(&mut g);
        cite!(["701.6a"]; assert_eq!((g.players[1].life, g.players[1].hand.len()), (20, 0)));
        // Uncited (Oracle): an enchantment alone fails Tidebinder's explicit source-type condition.
        assert_eq!(g.abilities(source).is_empty(), def == ZEALOT);
    }
}

#[test]
fn r608_2h_source_type_is_read_at_resolution_after_a_later_artifact_effect() {
    let mut g = game();
    let coating = g.put(0, COATING, Zone::Battlefield);
    cast(&mut g, 1, OMEN, vec![], "CU");
    pass(&mut g);
    let omen = *g.battlefield.iter().find(|id| g.obj(**id).card == Some(OMEN)).unwrap();
    let ability = g.stack[0];
    announce_tide(&mut g, Some(ability));
    activate(&mut g, coating, targets(omen), "");
    pass(&mut g);
    cite!(["613.1d"]; assert!(g.seen(omen, None).types.contains(&ir::CardType::Artifact)));
    pass(&mut g);
    // Pinned Tidebinder ruling 2023-11-10: current source types determine the conditional effect.
    cite!(["608.2h", "613.1f"]; assert!(g.abilities(omen).is_empty()));
}

#[test]
fn r611_2b_leaving_before_resolution_counters_the_ability_without_starting_a_duration() {
    let mut g = game();
    let source = g.put(1, SORCERER, Zone::Battlefield);
    let ability = activate(&mut g, source, vec![vec![Target::Player(0)]], "");
    let tide = announce_tide(&mut g, Some(ability));
    cast(&mut g, 0, UNSUMMON, targets(tide), "U");
    pass(&mut g);
    pass(&mut g);
    cite!(["701.6a"]; assert!(g.stack.is_empty()));
    cite!(["701.6a"]; assert_eq!(g.players[0].life, 20));
    cite!(["611.2b"]; assert_eq!(g.abilities(source).len(), 1));
}

#[test]
fn r611_2b_control_changes_do_not_end_presence_but_leaving_and_reentering_does() {
    let mut g = game();
    let source = g.put(1, SORCERER, Zone::Battlefield);
    let ability = activate(&mut g, source, vec![vec![Target::Player(0)]], "");
    let tide = announce_tide(&mut g, Some(ability));
    pass(&mut g);
    priority(&mut g, 1);
    let dominate = g.put(1, DOMINATE, Zone::Hand);
    g.players[1].pool = mana_pool("CCCUUU");
    g.act(1, paid("CCCUUU", Move::WithX(3, Box::new(Move::Cast { card: dominate, targets: targets(tide) })))).unwrap();
    pass(&mut g);
    cite!(["611.2b"]; assert_eq!(g.controller(tide), 1));
    cite!(["611.2b"]; assert!(g.abilities(source).is_empty()));
    cast(&mut g, 0, UNSUMMON, targets(tide), "U");
    pass(&mut g);
    cite!(["611.2b", "400.7"]; assert_eq!(g.abilities(source).len(), 1));
    // A new Tidebinder with no target cannot restart the departed incarnation's effect.
    announce_tide(&mut g, None);
    pass(&mut g);
    cite!(["611.2b", "400.7"]; assert_eq!(g.abilities(source).len(), 1));
}

#[test]
fn r613_7b_an_ability_granted_after_tidebinder_is_kept() {
    let mut g = game();
    let source = g.put(1, SORCERER, Zone::Battlefield);
    let ability = activate(&mut g, source, vec![vec![Target::Player(0)]], "");
    announce_tide(&mut g, Some(ability));
    pass(&mut g);
    // Flight is cast in its controller's main phase; the prior loss keeps its older timestamp.
    cast(&mut g, 1, FLIGHT, targets(source), "U");
    pass(&mut g);
    cite!(["613.1f", "613.7b"]; assert!(g.has(source, "Flying")));
    cite!(["613.1f"]; assert!(!g.abilities(source).iter().any(|member| matches!(&**member, ir::Ability::Activated { .. }))));
}

#[test]
fn r400_7_counter_source_projection_does_not_follow_a_departed_permanent() {
    let mut g = game();
    let source = g.put(1, SORCERER, Zone::Battlefield);
    let ability = activate(&mut g, source, vec![vec![Target::Player(0)]], "");
    announce_tide(&mut g, Some(ability));
    cast(&mut g, 0, UNSUMMON, targets(source), "U");
    pass(&mut g);
    pass(&mut g);
    priority(&mut g, 1);
    let card = *g.players[1].hand.iter().find(|id| g.obj(**id).card == Some(SORCERER)).unwrap();
    g.players[1].pool = mana_pool("CCU");
    g.act(1, paid("CCU", Move::Cast { card, targets: vec![] })).unwrap();
    pass(&mut g);
    let returned = *g.battlefield.iter().find(|id| g.obj(**id).card == Some(SORCERER)).unwrap();
    cite!(["400.7", "113.7a"]; assert_eq!(g.abilities(returned).len(), 1));
    cite!(["701.6a"]; assert_eq!(g.players[0].life, 20));
}
