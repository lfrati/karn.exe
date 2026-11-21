//! Bargain (702.166): a spell that costs less bargained is offered bargained
//! when only that cost can be paid; a bargained permanent's "if it was bargained" (High Fae
//! Negotiator); a target only a bargained spell has, on a real card (Brave the Wilds).
use crate::support::cite;
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const GLUTTON: usize = 0;
const ICE: usize = 1;
const FAE: usize = 2;
const WILDS: usize = 3;
const RELIC: usize = 4;
const BEAR: usize = 5;
const FOREST: usize = 6;
const TORCH: usize = 7;

fn real(name: &str) -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let def = CardDef::from_faces(&CardDef::catalog(&faces)[name]).unwrap();
    game::play::check(&def).unwrap_or_else(|e| panic!("{name}: {}", e.0));
    def
}
fn game() -> Game {
    let c = |name, types, pt| CardDef::new(name, "{0}", types, "", pt).expect(name);
    let mut g = Game::new(vec![real("Hamlet Glutton"), real("Ice Out"), real("High Fae Negotiator"), real("Brave the Wilds"),
        c("Relic", "Artifact", None), c("Bear", "Creature — Bear", Some((2, 2))), real("Forest"), real("Torch the Tower")], 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, FOREST, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn pool(g: &mut Game, colors: [u32; 5], colorless: u32) { g.players[0].pool.colors = colors; g.players[0].pool.colorless = colorless; }
fn resolve(g: &mut Game) {
    for _ in 0..30 {
        match g.decision.clone() {
            None if g.stack.is_empty() => return,
            None => g.act(g.priority, Move::Pass).unwrap(),
            Some(d) => panic!("{d:?}"),
        }
    }
}
fn bargained(card: ObjId, fodder: ObjId, targets: Vec<Vec<Target>>) -> Move {
    Move::WithOptional(true, Box::new(Move::WithCosts(vec![vec![fodder]], Box::new(Move::Cast { card, targets }))))
}


/// Hamlet Glutton {5}{G}{G} costs {3}{G}{G} bargained: with exactly that and a Relic, it is
/// offered bargained (601.2f: the cost is what the announcement makes it).
#[test]
fn a_spell_affordable_only_bargained_requires_the_sacrifice() {
    let mut g = game();
    let card = g.put(0, GLUTTON, Zone::Hand);
    let relic = g.put(0, RELIC, Zone::Battlefield);
    pool(&mut g, [0, 0, 0, 0, 2], 3);
    cite!(["702.166a", "601.2f"]; refused("the supplied mana does not pay that cost, all of it and no more"): unchanged(&mut g, |g| g.act(0, crate::support::paid("CCCGG", Move::Cast { card, targets: vec![] }))));
    cite!(["702.166a", "601.2f"]; ok: g.act(0, crate::support::paid("CCCGG",bargained(card, relic, vec![]))));
}

/// Ice Out {1}{U}{U} costs {U}{U} bargained.
#[test]
fn a_counterspell_affordable_only_bargained_requires_the_sacrifice() {
    let mut g = game();
    let card = g.put(0, ICE, Zone::Hand);
    let relic = g.put(0, RELIC, Zone::Battlefield);
    let torch = g.put(0, TORCH, Zone::Hand);
    let bear = g.put(1, BEAR, Zone::Battlefield);
    pool(&mut g, [0, 2, 0, 1, 0], 0);
    g.act(0, crate::support::paid("R",Move::Cast { card: torch, targets: vec![vec![Target::Object(bear)]] })).unwrap();
    let target = *g.stack.last().unwrap();
    cite!(["702.166a", "601.2f"]; refused("the supplied mana does not pay that cost, all of it and no more"): unchanged(&mut g, |g| g.act(0, crate::support::paid("UU", Move::Cast { card, targets: vec![vec![Target::Object(target)]] }))));
    cite!(["702.166a", "601.2f"]; ok: g.act(0, crate::support::paid("UU",bargained(card, relic, vec![vec![Target::Object(target)]]))));
}

/// "When this creature enters, if it was bargained, each opponent loses 3 life and you gain
/// 3 life.": the permanent was the bargained spell (702.166b).
#[test]
fn high_fae_negotiator_bargained_drains_and_unbargained_does_not() {
    for bargain in [false, true] {
        let mut g = game();
        let card = g.put(0, FAE, Zone::Hand);
        let relic = g.put(0, RELIC, Zone::Battlefield);
        let before=g.audit_game_objects();
        let available=&before["objects"][card.0.to_string()]["casting_costs"]["optional"];
        assert_eq!(available.as_array().unwrap().len(),1);
        assert_eq!(available[0]["kind"],"bargain");
        assert_eq!(available[0]["ability"]["source"],card.0);
        assert_eq!(available[0]["ability"]["paragraph"],1);
        pool(&mut g, [0, 0, 2, 0, 0], 3);
        g.act(0, crate::support::paid("CCCBB",if bargain { bargained(card, relic, vec![]) } else { Move::Cast { card, targets: vec![] } })).unwrap();
        let spell=*g.stack.last().unwrap();
        {
            let audit=g.audit_game_objects();
            let optional=&audit["objects"][spell.0.to_string()]["announced"]["costs"]["optional"];
            if bargain {
                cite!(["601.2b", "702.166b"]; assert_eq!(optional["kind"], "bargain"));
                cite!(["601.2b", "702.166b"]; assert_eq!(optional["ability"]["source"], spell.0));
                cite!(["601.2b", "702.166b"]; assert_eq!(optional["ability"]["definition"], "High Fae Negotiator"));
                cite!(["601.2b", "702.166b"]; assert_eq!(optional["ability"]["paragraph"], 1));
                cite!(["601.2b", "702.166b"]; assert!(optional["ability"]["keyword"].is_null()));
            } else { cite!(["601.2b", "702.166b"]; assert!(optional.is_null())); }
        }
        resolve(&mut g);
        let permanent=*g.battlefield.iter().find(|id|g.obj(**id).card==Some(FAE)).unwrap();
        assert_ne!(spell, permanent);
        {
            let audit=g.audit_game_objects();
            let optional=&audit["objects"][permanent.0.to_string()]["announced"]["costs"]["optional"];
            if bargain {
                cite!(["400.7d", "702.166b"]; assert_eq!(optional["kind"], "bargain"));
                cite!(["400.7d", "702.166b"]; assert_eq!(optional["ability"]["source"], permanent.0));
                cite!(["400.7d", "702.166b"]; assert_eq!(optional["ability"]["definition"], "High Fae Negotiator"));
                cite!(["400.7d", "702.166b"]; assert_eq!(optional["ability"]["paragraph"], 1));
                cite!(["400.7d", "702.166b"]; assert!(optional["ability"]["keyword"].is_null()));
            } else { cite!(["400.7d", "702.166b"]; assert!(optional.is_null())); }
        }
        let drained = if bargain { 3 } else { 0 };
        cite!(["702.166b"]; assert_eq!((g.players[0].life, g.players[1].life), (20 + drained, 20 - drained), "bargained: {bargain}"));
    }
}

/// "If this spell was bargained, target land you control becomes a 3/3 Elemental creature
/// with haste that's still a land." (702.166d)
#[test]
fn brave_the_wilds_targets_a_land_only_bargained() {
    for bargain in [false, true] {
        let mut g = game();
        let card = g.put(0, WILDS, Zone::Hand);
        let relic = g.put(0, RELIC, Zone::Battlefield);
        let land = g.put(0, FOREST, Zone::Battlefield);
        pool(&mut g, [0, 0, 0, 0, 1], 0);
        let targets = vec![if bargain { vec![Target::Object(land)] } else { vec![] }];
        g.act(0, crate::support::paid("G",if bargain { bargained(card, relic, targets) } else { Move::Cast { card, targets } })).unwrap();
        for _ in 0..30 {
            match g.decision.clone() {
                None if g.stack.is_empty() => break,
                None => g.act(g.priority, Move::Pass).unwrap(),
                // the searched basic land: the first offered
                Some(Decision::Choose { player, .. }) => {
                    let (_, asked) = g.asked().unwrap();
                    let answer = match asked {
                        game::legal::Asked::Search { cards, .. } | game::legal::Asked::Objects { among: cards, .. } => game::state::Answer::Objects(cards.into_iter().take(1).collect()),
                        a => panic!("{a:?}"),
                    };
                    g.act(player, Move::Choose(answer)).unwrap();
                }
                Some(d) => panic!("{d:?}"),
            }
        }
        cite!(["702.166d"]; assert_eq!(g.seen(land, None).pt, if bargain { Some((3, 3)) } else { None }, "bargained: {bargain}"));
    }
}

#[track_caller]
fn unchanged(g: &mut Game, action: impl FnOnce(&mut Game) -> Result<(), String>) -> Result<(), String> {
    let before=format!("{g:?}");
    let result=action(g);
    assert_eq!(format!("{g:?}"),before);
    result
}
