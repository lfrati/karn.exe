//! Fabricate is a resolving optional counter action with a token fallback, not an entry replacement.
use crate::support::{cite, mana_pool, oracle, paid};
use game::{CardDef, Game, Move, ObjId, Zone};
use game::state::{Answer, Choice, Decision};

fn setup() -> Game {
    let mut defs: Vec<_> = ["Weaponcraft Enthusiast", "Unsummon", "Hardened Scales", "Cayth, Famed Mechanist"]
        .into_iter().map(oracle).collect();
    defs.push(CardDef::new("Counter ban", "{0}", "Enchantment", "Creatures can't have counters put on them.", None).unwrap());
    Game::new(defs, 2, 20)
}
fn resolve(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
fn cast(g: &mut Game) -> ObjId {
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool = mana_pool("BBB");
    g.act(0, paid("BBB", Move::Cast { card, targets: vec![] })).unwrap();
    resolve(g);
    *g.battlefield.iter().find(|o| g.obj(**o).card == Some(0)).unwrap()
}
fn answer(g: &mut Game, yes: bool) {
    assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May { possible: true } })));
    g.act(0, Move::Choose(if yes { Answer::Yes } else { Answer::No })).unwrap();
}
fn servos(g: &Game) -> Vec<ObjId> {
    g.battlefield.iter().copied().filter(|o| g.obj(*o).chars.name == "Servo Token").collect()
}

#[test]
fn r702_123a_enters_without_counters_then_chooses_counters_or_tokens_on_resolution() {
    for scales in [false, true] {
        let mut g = setup();
        if scales { g.put(0, 2, Zone::Battlefield); }
        let source = cast(&mut g);
        cite!(["702.123a", "603.3"]; assert!(g.counters(source).is_empty() && g.stack.len() == 1));
        assert_eq!((g.power(source), g.toughness(source)), (Some(0), Some(1)));
        resolve(&mut g);
        for (mut branch, yes) in [(g.clone(), false), (g, true)] {
            answer(&mut branch, yes);
            cite!(["702.123a"]; assert_eq!(servos(&branch).len(), if yes { 0 } else { 2 }));
            cite!(["702.123a", "614.1a"]; assert_eq!(branch.counters(source).get("+1/+1").copied().unwrap_or(0), if yes { 2 + u32::from(scales) } else { 0 }));
            for servo in servos(&branch) {
                cite!(["702.123a"]; assert!(branch.obj(servo).token() && branch.is_a(servo, ir::CardType::Artifact)));
                assert_eq!((branch.power(servo), branch.toughness(servo)), (Some(1), Some(1)));
            }
            assert!(branch.stack.is_empty() && branch.decision.is_none());
        }
    }
}

#[test]
fn r702_123a_departed_source_or_prohibited_counters_forces_servos_without_an_impossible_choice() {
    for ban in [false, true] {
        let mut g = setup();
        if ban { g.put(0, 4, Zone::Battlefield); }
        let source = cast(&mut g);
        if !ban {
            let spell = g.put(0, 1, Zone::Hand);
            g.players[0].pool = mana_pool("U");
            g.act(0, paid("U", Move::Cast { card: spell, targets: vec![vec![source.into()]] })).unwrap();
            resolve(&mut g);
            assert_eq!(g.players[0].hand.len(), 1);
        }
        resolve(&mut g);
        // Pinned Enthusiast ruling: when counters can't be placed, create the
        // tokens; the controller cannot choose an impossible counter action.
        cite!(["608.2d", "702.123a"]; assert_eq!(servos(&g).len(), 2));
        assert!(g.stack.is_empty() && g.decision.is_none());
    }
}

#[test]
fn r702_123b_cayth_grants_an_independent_instance_to_nontokens_and_servos_do_not_recurse() {
    let mut g = setup();
    let cayth = g.put(0, 3, Zone::Battlefield);
    let source = cast(&mut g);
    let Some(Decision::Order { player: 0, abilities }) = &g.decision else { panic!("{:?}", g.decision) };
    cite!(["702.123b", "613.1f"]; assert_eq!(abilities.len(), 2));
    g.act(0, Move::Order(vec![0, 1])).unwrap();
    for _ in 0..2 { resolve(&mut g); answer(&mut g, false); }
    cite!(["702.123b"]; assert_eq!(servos(&g).len(), 3));
    assert!(g.counters(source).is_empty() && g.counters(cayth).is_empty());
    // Uncited (Oracle): Cayth gives fabricate to other nontoken creatures, so
    // none of these three Servo entries adds another occurrence.
    assert!(g.stack.is_empty() && g.decision.is_none());
}
