//! Conditional self-flash permits beginning a cast (601.3d), without making its
//! granting ability function in the hand (113.6e). Costs follow that check (601.2f-g).
use crate::support::{cite, mana_pool, oracle, paid};
use game::{CardDef, Game, Move, Zone};

const WURM: usize = 0;
const DESERT: usize = 1;
const ALTAR: usize = 2;

fn game(active: usize, desert_controller: Option<usize>) -> (Game, game::ObjId) {
    let altar = CardDef::new("Land altar", "{0}", "Artifact", "Sacrifice a land: Add {G}.", None).unwrap();
    game::play::check(&altar).unwrap();
    let mut g = Game::new(vec![oracle("Colossal Rattlewurm"), oracle("Sunscorched Desert"), altar], 2, 20);
    g.active = active;
    g.priority = 0;
    g.part = ir::Part::Upkeep;
    if let Some(player) = desert_controller { g.put(player, DESERT, Zone::Battlefield); }
    let card = g.put(0, WURM, Zone::Hand);
    g.players[0].pool = mana_pool("CCGG");
    (g, card)
}

#[test]
fn r601_3d_rattlewurm_can_begin_casting_in_its_upkeep_or_an_opponents_turn() {
    for active in [0, 1] {
        let (mut g, card) = game(active, Some(0));
        cite!(["113.6e"]; assert!(!g.has(card, "Flash"), "the hand is not given flash"));
        cite!(["601.3d"]; ok: g.act(0, paid("CCGG", Move::Cast { card, targets: vec![] })));
        cite!(["601.2a"]; assert_eq!(g.stack.len(), 1));
        assert_eq!(g.obj(g.stack[0]).chars.name, "Colossal Rattlewurm");
    }
}

#[test]
fn r601_3d_rattlewurm_requires_a_desert_the_caster_controls() {
    for desert in [None, Some(1)] {
        let (mut g, card) = game(0, desert);
        let before = format!("{g:?}");
        cite!(["601.3d", "117.1a"]; refused("that is cast in a main phase of your own turn with the stack empty (117.1a)"):
            g.act(0, paid("CCGG", Move::Cast { card, targets: vec![] })));
        cite!(["601.2"]; assert_eq!(format!("{g:?}"), before));
    }
}

#[test]
fn r601_3d_losing_the_desert_while_paying_does_not_revoke_the_cast() {
    let (mut g, card) = game(1, Some(0));
    let desert = g.battlefield[0];
    let altar = g.put(0, ALTAR, Zone::Battlefield);
    g.players[0].pool = mana_pool("CCG");
    let plan = vec![game::legal::Step { source: altar, ability: 0, paying: vec![vec![desert]],
        plan: vec![], x: None, life: vec![], with: None, adding: vec![], chose: vec![], cost_branches: None }];
    // Pinned Rattlewurm ruling 8a5bb314846b80e5: losing the Desert while activating
    // mana abilities follows the legality check and does not revoke that permission.
    cite!(["601.3d", "601.2f", "601.2g"]; ok:
        g.act(0, Move::WithMana(plan, Box::new(paid("CCGG", Move::Cast { card, targets: vec![] })))));
    cite!(["601.2h", "400.7"]; assert_eq!(g.obj(desert).zone, Zone::Gone));
    assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).card == Some(DESERT)));
    cite!(["601.2a"]; assert_eq!(g.stack.len(), 1));
    assert_eq!(g.obj(g.stack[0]).chars.name, "Colossal Rattlewurm");
}
