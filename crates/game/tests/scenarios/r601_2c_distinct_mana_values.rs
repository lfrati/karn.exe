//! Agadeem's explicit graveyard origin and collective target restriction share one target owner.
use crate::support::{cite, mana_pool, oracle, paid};
use game::{Event, Game, Move, ObjId, Target, Zone};
use game::state::{Answer, Choice, Decision};

const AGADEEM: usize = 0;
const MEMNITE: usize = 1;
const BALLISTA: usize = 2;
const ELF: usize = 3;
const BEARS: usize = 4;
const ZEALOT: usize = 5;
const COURSER: usize = 6;
const FOREST: usize = 7;
const BOLT: usize = 8;
const REDIRECT: usize = 9;

fn game() -> Game {
    let mut g = Game::new(["Agadeem's Awakening", "Memnite", "Walking Ballista", "Llanowar Elves",
        "Grizzly Bears", "Dusk Legion Zealot", "Centaur Courser", "Forest", "Lightning Bolt", "Redirect"]
        .into_iter().map(oracle).collect::<Vec<_>>(), 2, 20);
    g.part = ir::Part::PrecombatMain;
    for p in 0..2 { for _ in 0..8 { g.put(p, FOREST, Zone::Library); } }
    g
}
fn pass(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
fn resolve(g: &mut Game) { while !g.stack.is_empty() { pass(g); } }
fn payment(x: u32) -> String { format!("{}BBB", "C".repeat(x as usize)) }
fn action(card: ObjId, x: u32, targets: Vec<ObjId>) -> Move {
    paid(&payment(x), Move::WithX(x, Box::new(Move::Cast { card, targets: vec![targets.into_iter().map(Target::Object).collect()] })))
}
fn cast(g: &mut Game, x: u32, targets: Vec<ObjId>) -> ObjId {
    let card = g.put(0, AGADEEM, Zone::Hand);
    g.players[0].pool = mana_pool(&payment(x));
    g.act(0, action(card, x, targets)).unwrap();
    *g.stack.last().unwrap()
}
fn battlefield(g: &Game) -> Vec<usize> {
    let mut cards: Vec<_> = g.battlefield.iter().map(|id| g.obj(*id).card.unwrap()).collect();
    cards.sort(); cards
}

#[test]
fn r601_2c_x_three_returns_one_of_each_value_zero_through_three_before_entry_triggers_resolve() {
    let mut g = game();
    let chosen: Vec<_> = [MEMNITE,ELF,ZEALOT,COURSER].into_iter().map(|def|g.put(0,def,Zone::Graveyard)).collect();
    let other_two = g.put(0, BEARS, Zone::Graveyard);
    let other_zero = g.put(0, BALLISTA, Zone::Graveyard);
    cast(&mut g, 3, chosen.clone());
    pass(&mut g);
    // Pinned Agadeem ruling 2020-09-25: X=3 can return four distinct values, including zero.
    cite!(["202.3", "601.2c", "608.2f"]; assert_eq!(battlefield(&g), vec![MEMNITE,ELF,ZEALOT,COURSER]));
    cite!(["400.7"]; assert!(chosen.iter().all(|old| g.obj(*old).zone == Zone::Gone)));
    cite!(["608.2b"]; assert!(g.players[0].graveyard.contains(&other_two) && g.players[0].graveyard.contains(&other_zero)));
    cite!(["603.3", "608.2f"]; assert_eq!(g.stack.len(), 1));
    cite!(["603.3"]; assert_eq!((g.players[0].life,g.players[0].hand.len()), (20,0)));
    pass(&mut g);
    cite!(["608.2c"]; assert_eq!((g.players[0].life,g.players[0].hand.len()), (19,1)));
}

#[test]
fn r601_2c_x_zero_allows_no_targets_or_one_zero_value_card_but_never_both() {
    for def in [None, Some(MEMNITE), Some(BALLISTA)] {
        let mut g = game();
        let memnite = g.put(0, MEMNITE, Zone::Graveyard);
        let ballista = g.put(0, BALLISTA, Zone::Graveyard);
        let card = g.put(0, AGADEEM, Zone::Hand);
        g.players[0].pool = mana_pool("BBB");
        cite!(["601.2c"]; refused("different mana values"): g.act(0, action(card,0,vec![memnite,ballista])));
        let chosen = match def { Some(MEMNITE) => vec![memnite], Some(BALLISTA) => vec![ballista], _ => vec![] };
        g.act(0, action(card,0,chosen)).unwrap();
        resolve(&mut g);
        cite!(["107.3g", "202.3e"]; assert_eq!(battlefield(&g), if def == Some(MEMNITE) {vec![MEMNITE]} else {vec![]}));
        // Ballista is a legal zero-value card; it enters with zero counters and dies as a 0/0.
        if def == Some(BALLISTA) {
            cite!(["704.5f"]; assert!(g.players[0].graveyard.iter().any(|id|g.obj(*id).card == Some(BALLISTA) && *id != ballista)));
        }
        cite!(["608.2b"]; assert!(g.stack.is_empty() && g.decision.is_none()));
    }
}

#[test]
fn r601_2c_invalid_values_owners_zones_and_types_leave_the_entire_proposal_unchanged() {
    let mut g = game();
    let bear = g.put(0, BEARS, Zone::Graveyard);
    let zealot = g.put(0, ZEALOT, Zone::Graveyard);
    let large = g.put(0, COURSER, Zone::Graveyard);
    let other = g.put(1, BEARS, Zone::Graveyard);
    let land = g.put(0, FOREST, Zone::Graveyard);
    let instant = g.put(0, BOLT, Zone::Graveyard);
    let live = g.put(0, BEARS, Zone::Battlefield);
    let card = g.put(0, AGADEEM, Zone::Hand);
    g.players[0].pool = mana_pool("CCBBB");
    let before = format!("{g:?}");
    for (chosen, reason) in [(vec![bear,zealot], "different mana values"), (vec![bear,bear], "same target twice"),
        (vec![large], "an illegal target"), (vec![other], "an illegal target"), (vec![land], "an illegal target"),
        (vec![instant], "an illegal target"), (vec![live], "an illegal target")] {
        cite!(["601.2c", "115.3"]; case:reason; refused(reason): g.act(0, action(card,2,chosen)));
        cite!(["733.1"]; assert_eq!(format!("{g:?}"), before));
    }
    g.act(0, action(card,2,vec![bear])).unwrap();
    resolve(&mut g);
    cite!(["608.2c"]; assert_eq!(battlefield(&g), vec![BEARS,BEARS]));
}

#[test]
fn r608_2b_departed_targets_do_not_invalidate_survivors_or_follow_a_returned_incarnation() {
    for mode in ["one", "all", "returned"] {
        let mut g = game();
        let memnite = g.put(0, MEMNITE, Zone::Graveyard);
        let elf = g.put(0, ELF, Zone::Graveyard);
        let spell = cast(&mut g, 1, vec![memnite,elf]);
        g.move_object(memnite, Zone::Hand);
        if mode == "all" { g.move_object(elf, Zone::Hand); }
        if mode == "returned" { let card = g.players[0].hand[0]; g.move_object(card, Zone::Graveyard); }
        resolve(&mut g);
        cite!(["608.2b", "400.7"]; case:mode; assert_eq!(battlefield(&g), if mode == "all" {vec![]} else {vec![ELF]}));
        cite!(["608.2b"]; assert_eq!(g.log.iter().any(|e|matches!(e,Event::Fizzled { spell: id } if *id == spell)), mode == "all"));
        if mode == "returned" {
            cite!(["400.7"]; assert!(g.players[0].graveyard.iter().any(|id|g.obj(*id).card == Some(MEMNITE) && *id != memnite)));
        }
    }
}

fn redirect(g: &mut Game, spell: ObjId) {
    let card = g.put(0, REDIRECT, Zone::Hand);
    g.players[0].pool = mana_pool("UU");
    g.act(0, paid("UU", Move::Cast { card, targets: vec![vec![Target::Object(spell)]] })).unwrap();
    pass(g);
    cite!(["115.7d"]; assert!(matches!(g.decision,Some(Decision::Choose { player:0, choice:Choice::ChangeTargets { .. } }))));
}

#[test]
fn r115_7e_retargeted_final_group_keeps_distinct_values_and_exposes_the_same_constraint() {
    let mut g = game();
    let memnite = g.put(0, MEMNITE, Zone::Graveyard);
    let bear = g.put(0, BEARS, Zone::Graveyard);
    let zealot = g.put(0, ZEALOT, Zone::Graveyard);
    let spell = cast(&mut g, 2, vec![memnite,bear]);
    redirect(&mut g, spell);
    let Some(Decision::Choose { choice: Choice::ChangeTargets { slots, .. }, .. }) = &g.decision else { unreachable!() };
    // Uncited (managed boundary): the offered domain represents the same collective rule as execution.
    assert!(slots[0].different_mana_values.is_some());
    assert!(!slots[0].allows_group(&[bear.into(),zealot.into()]));
    cite!(["115.7d", "115.7e"]; refused("changed target must be legal"): g.act(0, Move::Choose(Answer::Targets(vec![vec![bear.into(),zealot.into()]]))));
    g.act(0, Move::Choose(Answer::Targets(vec![vec![memnite.into(),zealot.into()]]))).unwrap();
    resolve(&mut g);
    cite!(["115.7e", "608.2c"]; assert_eq!(battlefield(&g), vec![MEMNITE,ZEALOT]));
}

#[test]
fn illegal_retained_value_corner_is_unsupported_while_decline_and_replacing_all_targets_work() {
    let mut g = game();
    let memnite = g.put(0, MEMNITE, Zone::Graveyard);
    let bear = g.put(0, BEARS, Zone::Graveyard);
    let elf = g.put(0, ELF, Zone::Graveyard);
    let zealot = g.put(0, ZEALOT, Zone::Graveyard);
    let spell = cast(&mut g, 2, vec![memnite,bear]);
    g.move_object(memnite, Zone::Hand);
    redirect(&mut g, spell);
    let mut replace_all = g.clone();
    let before = format!("{g:?}");
    let error = g.attempt(0, Move::Choose(Answer::Targets(vec![vec![memnite.into(),zealot.into()]]))).unwrap_err();
    // Uncited (capability boundary): a missing cross-target LKI proof is Unsupported, not a claim of illegality.
    assert_eq!(error.class(), game::simulation::Class::Unsupported);
    assert!(error.to_string().contains("retained illegal target"));
    assert_eq!(format!("{g:?}"), before);
    g.act(0, Move::Choose(Answer::No)).unwrap();
    resolve(&mut g);
    cite!(["115.7d", "608.2b"]; assert_eq!(battlefield(&g), vec![BEARS]));
    replace_all.act(0, Move::Choose(Answer::Targets(vec![vec![elf.into(),zealot.into()]]))).unwrap();
    resolve(&mut replace_all);
    cite!(["115.7d", "115.7e"]; assert_eq!(battlefield(&replace_all), vec![ELF,ZEALOT]));
}

#[test]
fn r712_12_the_same_full_agadeem_card_still_plays_its_land_face_and_pays_life_explicitly() {
    for pay in [false,true] {
        let mut g = game();
        let card = g.put(0, AGADEEM, Zone::Hand);
        g.act(0, Move::PlayLandAs { card, face: 1 }).unwrap();
        cite!(["614.1c"]; assert!(matches!(g.decision, Some(Decision::Choose { player:0, choice:Choice::May { possible:true } }))));
        g.act(0, Move::Choose(if pay {Answer::Yes} else {Answer::No})).unwrap();
        let land = g.battlefield[0];
        cite!(["712.12", "614.1c"]; assert_eq!(g.def(land).unwrap().name(), "Agadeem, the Undercrypt"));
        cite!(["614.1c"]; assert_eq!(g.obj(land).tapped, !pay));
        cite!(["119.4"]; assert_eq!(g.players[0].life, if pay {17} else {20}));
        cite!(["305.1", "601.2i"]; assert_eq!(g.players[0].game_casts, Some(0)));
    }
}
