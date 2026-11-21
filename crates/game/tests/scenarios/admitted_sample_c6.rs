//! A fixed selection of cards the engine runs. Oracle compositions with rule-derived, finite assertions.
use crate::support::{cite, oracle, paid};
use game::state::{Decision, Choice, Answer};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::{Color, ManaSymbol, Part};

fn setup(name: &str) -> Game {
    let mut g = Game::new(
        vec![
            oracle(name),
            oracle("Island"),
            CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
            CardDef::new("Goblin", "{0}", "Creature — Goblin", "", Some((1, 1))).unwrap(),
            CardDef::new("Enchant", "{0}", "Enchantment", "", None).unwrap(),
            CardDef::new("Legend", "{0}", "Legendary Creature — Human", "", Some((2, 2))).unwrap(),
            CardDef::new("Outlet", "{0}", "Artifact", "Discard a card: You gain 1 life.", None).unwrap(),
            CardDef::new("Ninja", "{0}", "Creature — Ninja", "", Some((1, 1))).unwrap(),
            CardDef::new("Shield", "{0}", "Creature — Spirit", "Indestructible", Some((2, 5))).unwrap(),
            CardDef::new("Artifact Bear", "{0}", "Artifact Creature — Bear", "", Some((2, 2))).unwrap(),
            CardDef::new("Uncounterable", "{0}", "Creature — Bear", "This spell can't be countered.", Some((2, 2)))
                .unwrap(),
            CardDef::new("Enchant Bear", "{0}", "Enchantment Creature — Bear", "", Some((2, 2))).unwrap(),
        ],
        2,
        20,
    );
    for p in 0..2 {
        for _ in 0..20 {
            g.put(p, 1, Zone::Library);
        }
    }
    g
}
fn pass(g: &mut Game) {
    for _ in 0..2 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn settle(g: &mut Game) {
    for _ in 0..30 {
        if g.stack.is_empty() {
            return;
        }
        assert!(g.decision.is_none(), "unexpected {:?}", g.decision);
        pass(g);
    }
    panic!("unsettled");
}
fn cast(g: &mut Game, p: usize, def: usize, targets: Vec<Vec<Target>>, spending: &str) -> ObjId {
    let card = g.put(p, def, Zone::Hand);
    g.act(p, paid(spending, Move::Cast { card, targets })).unwrap();
    *g.stack.last().unwrap()
}
fn activate(source: ObjId, ability: usize) -> Move {
    Move::Activate { source, ability, targets: vec![], paying: vec![] }
}
fn mana(g: &mut Game, generic: u32, color: Color, n: u32) {
    g.players[0].pool.colorless = generic;
    g.players[0].pool.colors[color as usize] = n;
}
fn until(g: &mut Game, turn: u32, part: Part) {
    for _ in 0..300 {
        if g.turn == turn && g.part == part {
            return;
        }
        let mv = match g.decision {
            Some(Decision::Attackers { .. }) => Move::Attack(vec![]),
            Some(Decision::Blockers { .. }) => Move::Block(vec![]),
            Some(ref d) => panic!("unexpected {d:?}"),
            None => Move::Pass,
        };
        g.act(g.decision_maker(g.priority), mv).unwrap();
    }
    panic!("boundary not reached");
}
fn attack(g: &mut Game, attackers: &[ObjId]) {
    pass(g);
    pass(g);
    g.act(0, Move::Attack(attackers.iter().map(|a| (*a, Target::Player(1))).collect())).unwrap();
}
// Rules: 603.6a, 701.11a, 614.6, 700.4
// Entry exiles both graveyards. Later destruction is replaced, including the enchantment itself.
#[test]
fn r614_rest_in_peace_entry_and_self_departure() {
    let mut g = setup("Rest in Peace");
    g.put(0,2,Zone::Graveyard); g.put(1,3,Zone::Graveyard);
    mana(&mut g,1,Color::White,1);
    cast(&mut g,0,0,vec![], "CW"); settle(&mut g);
    cite!(["603.6a","701.11a"]; assert!(g.players.iter().all(|p|p.graveyard.is_empty()) && g.exile.len()==2));
    let bear=g.put(1,2,Zone::Battlefield);
    g.destroy(bear);
    cite!(["614.6","700.4"]; assert!(g.players[1].graveyard.is_empty() && g.exile.len()==3));
    let rip=*g.battlefield.iter().find(|o|g.obj(**o).card==Some(0)).unwrap();
    g.destroy(rip);
    cite!(["614.6"]; assert_eq!(g.exile.len(),4));
    let next=g.put(1,2,Zone::Battlefield);g.destroy(next);
    cite!(["614.6"]; assert_eq!(g.players[1].graveyard.len(),1));
}

// Rules: 701.67a, 602.5b, 122.1a
// Five generic paid by the source and four fresh creatures; own-turn restriction remains.
#[test]
fn r701_vinebender_waterbend_fresh_creatures_and_turn() {
    let mut g=setup("Foggy Swamp Vinebender");
    let v=g.put(0,0,Zone::Battlefield);
    let mut tapped=vec![v];for _ in 0..4 {tapped.push(g.put(0,2,Zone::Battlefield));}
    g.act(0,Move::Activate{source:v,ability:1,targets:vec![],paying:vec![tapped.clone()]}).unwrap();
    cite!(["701.67a"]; assert!(tapped.iter().all(|o|g.obj(*o).tapped) && g.players[0].pool.total()==0));
    settle(&mut g);
    cite!(["122.1a"]; assert_eq!((g.power(v),g.toughness(v)),(Some(5),Some(4))));
    until(&mut g,2,Part::PrecombatMain);g.act(1,Move::Pass).unwrap();g.players[0].pool.colorless=5;
    cite!(["602.5b"]; refused("the activation condition does not hold"): act_unchanged(&mut g,0,paid("CCCCC",activate(v,1))));
}

// Rules: 614.1d, 107.5, 605.3b, 106.1a
// Three legal choices each add one mana; red is not a choice.
#[test]
fn r605_citadel_entry_and_three_colors() {
    for color in [Color::White,Color::Black,Color::Green] {
        let mut g=setup("Sandsteppe Citadel");let card=g.put(0,0,Zone::Hand);
        g.act(0,Move::PlayLand(card)).unwrap();let land=*g.battlefield.last().unwrap();
        let add=|c|Move::Adding(vec![vec![ManaSymbol::Color(c)]],Box::new(activate(land,1)));
        cite!(["614.1d"]; assert!(g.obj(land).tapped));
        cite!(["107.5"]; refused("can't pay {T}"): g.act(0,add(color)));
        until(&mut g,3,Part::PrecombatMain);
        cite!(["106.1a"]; refused("mana"): g.act(0,add(Color::Red)));
        g.act(0,add(color)).unwrap();
        cite!(["605.3b"]; assert_eq!((g.players[0].pool.total(),g.players[0].pool.colors[color as usize],g.stack.len()),(1,1,0)));
    }
}

// Rules: 702.108a, 611.2a, 702.4a
// Prowess from an actual noncreature cast resolves before it, and expires at cleanup.
#[test]
fn r702_tetsuko_prowess_and_expiration() {
    let mut g=setup("Tetsuko Umezawa, Pursuer");let t=g.put(0,0,Zone::Battlefield);
    cite!(["702.4a"]; assert!(g.has(t,"Double strike")));
    cast(&mut g,0,4,vec![], "");
    cite!(["702.108a"]; assert_eq!(g.stack.len(),2));
    pass(&mut g);
    cite!(["702.108a"]; assert_eq!((g.power(t),g.toughness(t),g.stack.len()),(Some(3),Some(5),1)));
    settle(&mut g);until(&mut g,2,Part::PrecombatMain);
    cite!(["611.2a"]; assert_eq!((g.power(t),g.toughness(t)),(Some(2),Some(4))));
}

// Rules: 702.193a, 122.1a, 611.2a
// Entry-turn discount removes 2G from 4G; quota consumed, counter persists after granted abilities expire.
#[test]
fn r702_hercules_entry_discount_once_and_duration() {
    let mut g=setup("Hercules, Prince of Power");mana(&mut g,2,Color::Green,1);
    cast(&mut g,0,0,vec![], "CCG");settle(&mut g);let h=*g.battlefield.last().unwrap();
    g.players[0].pool.colorless=2;g.act(0,paid("CC",activate(h,0))).unwrap();
    assert_eq!(g.players[0].pool.total(),0);
    g.players[0].pool.colorless=5;g.players[0].pool.colors[4]=1;
    cite!(["702.193a"]; refused("that ability is activated only once for this object"): act_unchanged(&mut g,0,paid("CC",activate(h,0))));settle(&mut g);
    cite!(["122.1a","611.2a"]; assert!(g.power(h)==Some(4) && g.has(h,"Vigilance") && g.has(h,"Indestructible") && g.has(h,"Haste")));
    until(&mut g,2,Part::PrecombatMain);
    cite!(["611.2a","122.1a"]; assert!(g.power(h)==Some(4) && !g.has(h,"Vigilance") && !g.has(h,"Indestructible") && !g.has(h,"Haste")));
}

// Rules: 121.2, 608.2c, 701.8a
// Three actual draws precede one required discard; identities and order are declared.
#[test]
fn r121_reminiscence_draw_then_discard() {
    let mut g=setup("Ancestral Reminiscence");g.players[0].library.clear();
    for d in [2,3,4] {g.put(0,d,Zone::Library);}
    mana(&mut g,3,Color::Blue,1);cast(&mut g,0,0,vec![], "CCCU");pass(&mut g);
    cite!(["121.2","608.2c"]; assert!(g.players[0].library.is_empty() && g.players[0].hand.len()==3));
    cite!(["701.8a"]; assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::Objects{verb:ir::Verb::Discard,count:1,..}})), "{:?}",g.decision));
    let discarded=g.players[0].hand[0];g.act(0,Move::Choose(Answer::Objects(vec![discarded]))).unwrap();
    cite!(["701.8a"]; assert_eq!((g.players[0].hand.len(),g.players[0].graveyard.len(),g.stack.len()),(2,2,0)));
}

// Rules: 700.2, 120.3a, 202.3, 202.3e, 701.11a
// Separate modes: each opponent loses three; artifact exile includes own, opponent, X=0 and MV3, excludes MV4/nonartifact.
#[test]
fn r700_rampage_both_modes_and_mana_value_boundary() {
    for mode in [0,1] {
        let mut g=setup("Ruinous Rampage");
        let defs=std::sync::Arc::make_mut(&mut g.defs);
        for (name,cost) in [("Three","{3}"),("Four","{4}"),("Variable","{X}")] {
            defs.push(CardDef::new(name,cost,"Artifact","",None).unwrap());
        }
        let three=g.put(0,12,Zone::Battlefield);let four=g.put(1,13,Zone::Battlefield);let x=g.put(1,14,Zone::Battlefield);let bear=g.put(1,2,Zone::Battlefield);
        mana(&mut g,1,Color::Red,2);let c=g.put(0,0,Zone::Hand);
        g.act(0,Move::Modes(vec![mode],Box::new(paid("CRR",Move::Cast{card:c,targets:vec![]})))).unwrap();settle(&mut g);
        cite!(["120.3a","700.2"]; assert_eq!((g.players[0].life,g.players[1].life),if mode==0 {(20,17)}else{(20,20)}));
        cite!(["202.3","202.3e","701.11a"]; assert_eq!((g.battlefield.contains(&three),g.battlefield.contains(&x)),(mode==0,mode==0)));
        cite!(["202.3"]; assert!(g.battlefield.contains(&four) && g.battlefield.contains(&bear)));
    }
}

// Rules: 701.21a, 603.2, 120.3a
// Own artifact, opposing creature and destruction do not trigger. The positive case follows.
#[test]
fn r603_tracker_nonqualifying_events_do_not_trigger() {
    let mut g=setup("Vengeful Tracker");g.put(0,0,Zone::Battlefield);
    let own=g.put(0,9,Zone::Battlefield);g.sacrifice(0,own);
    let bear=g.put(1,2,Zone::Battlefield);g.sacrifice(1,bear);
    let destroyed=g.put(1,9,Zone::Battlefield);g.destroy(destroyed);
    cite!(["603.2","701.21a"]; assert!(g.stack.is_empty()));
    g.act(0,Move::Pass).unwrap();
    cite!(["603.2","701.21a"]; assert!(g.stack.is_empty()));
    cite!(["120.3a"]; assert_eq!((g.players[0].life,g.players[1].life),(20,20)));
    // The positive sacrifice-to-damage case is r603_tracker_sacrifice_controller_and_type, below.

}

// Rules: 701.21a, 603.2, 120.3a
// The positive case: the sacrificing opponent is the damage recipient.
#[test]
fn r603_tracker_sacrifice_controller_and_type() {
    let mut g = setup("Vengeful Tracker");
    g.put(0, 0, Zone::Battlefield);
    let sacrificed = g.put(1, 9, Zone::Battlefield);
    g.sacrifice(1, sacrificed);
    g.act(0, Move::Pass).unwrap();
    cite!(["603.2", "701.21a"]; assert_eq!(g.stack.len(), 1));
    settle(&mut g);
    cite!(["120.3a"]; assert_eq!((g.players[0].life, g.players[1].life), (20, 18)));
}

// Rules: 506.5, 603.2, 611.2a, 702.110b
// One attacking ally gains both grants; two attackers produce no trigger.
#[test]
fn r506_widow_alone_threshold_and_grants() {
    for n in [1,2] {
        let mut g=setup("Black Widow, Double Agent");g.put(0,0,Zone::Battlefield);
        let a=g.put(0,2,Zone::Battlefield);let b=g.put(0,2,Zone::Battlefield);
        attack(&mut g,&[a,b][..n]);
        cite!(["506.5","603.2"]; assert_eq!(g.stack.len(),usize::from(n==1)));settle(&mut g);
        cite!(["611.2a","702.110b"]; assert_eq!((g.has(a,"First strike"),g.has(a,"Menace")),(n==1,n==1)));
    }
}

// Rules: 105.4, 603.6a, 121.2, 702.9a
// Source contributes blue; a black/red permanent contributes two more, colorless lands none.
#[test]
fn r105_shinestriker_counts_distinct_colors_on_entry() {
    for multicolor in [false,true] {
        let mut g=setup("Shinestriker");g.put(0,1,Zone::Battlefield);
        if multicolor {let d=g.defs.len();std::sync::Arc::make_mut(&mut g.defs).push(oracle("Bolg's Company"));g.put(0,d,Zone::Battlefield);}
        mana(&mut g,4,Color::Blue,2);cast(&mut g,0,0,vec![], "CCCCUU");settle(&mut g);
        let source=*g.battlefield.iter().find(|o|g.obj(**o).card==Some(0)).unwrap();
        cite!(["702.9a"]; assert!(g.has(source,"Flying")));
        cite!(["105.4","121.2"]; assert_eq!(g.players[0].hand.len(),if multicolor {3}else{1}));
    }
}

// Rules: 601.2c, 701.6a, 304.1
// Opponent enchantment-creature spell can be countered during its controller's turn.
#[test]
fn r701_scatter_opponent_enchantment_creature_instant() {
    let mut g=setup("Essence Scatter");until(&mut g,2,Part::PrecombatMain);
    let spell=cast(&mut g,1,11,vec![], "");g.act(1,Move::Pass).unwrap();
    mana(&mut g,1,Color::Blue,1);let c=g.put(0,0,Zone::Hand);
    g.act(0,paid("CU",Move::Cast{card:c,targets:vec![vec![Target::Object(spell)]]})).unwrap();pass(&mut g);
    cite!(["701.6a","304.1"]; assert!(g.stack.is_empty() && g.players[1].graveyard.len()==1 && g.players[0].graveyard.len()==1));
}

// Rules: 602.2b, 302.6, 702.10c, 605.3b
// Actual entry this turn cannot tap without another own Goblin; self, enemy, non-Goblin costs rejected transactionally.
#[test]
fn r605_bolg_entry_haste_and_cost_rejections() {
    let mut g=setup("Bolg's Company");mana(&mut g,0,Color::Black,1);g.players[0].pool.colors[3]=1;
    cast(&mut g,0,0,vec![], "BR");settle(&mut g);let b=*g.battlefield.last().unwrap();
    let enemy=g.put(1,3,Zone::Battlefield);let non=g.put(0,2,Zone::Battlefield);
    let pay=|o|Move::Activate{source:b,ability:1,targets:vec![],paying:vec![vec![o]]};
    cite!(["302.6","611.3a"]; assert!(g.summoning_sick(b) && !g.has(b,"Haste")));
    // Every valid sacrifice is another own Goblin, which also supplies haste.
    // Add that prerequisite before isolating the three invalid cost selections.
    let gob=g.put(0,3,Zone::Battlefield);
    for (invalid,name) in [(b,"Bolg's Company"),(enemy,"Goblin"),(non,"Bear")] {
        cite!(["602.2b"]; refused(&format!("{name} is not what that cost asks for or cannot pay it")): act_unchanged(&mut g,0,pay(invalid)));
        cite!(["602.2b"]; assert!(!g.obj(b).tapped && g.battlefield.contains(&invalid) && g.players[0].pool.total()==0));
    }
    g.act(0,pay(gob)).unwrap();
    cite!(["702.10c","605.3b"]; assert!(g.obj(b).tapped && g.stack.is_empty() && !g.battlefield.contains(&gob)));
    cite!(["605.3b"]; assert_eq!((g.players[0].pool.colors[2],g.players[0].pool.colors[3]),(1,1)));
}

// Rules: 702.10b, 302.6, 508.1a, 611.3a, 605.3b
// Entry-turn attacker allowed with an own Goblin, denied with only enemy Goblins.
#[test]
fn r508_bolg_entry_turn_attack_and_remaining_goblin() {
    for owner in [0,1] {
        let mut g=setup("Bolg's Company");mana(&mut g,0,Color::Black,1);g.players[0].pool.colors[3]=1;
        cast(&mut g,0,0,vec![], "BR");settle(&mut g);let b=*g.battlefield.last().unwrap();
        let gob=g.put(owner,3,Zone::Battlefield);let extra=g.put(owner,3,Zone::Battlefield);
        g.put(0,2,Zone::Battlefield); // An older non-Goblin keeps the attacker question open.
        pass(&mut g);pass(&mut g);
        let mv=Move::Attack(vec![(b,Target::Player(1))]);
        if owner==1 {
            cite!(["302.6","508.1a"]; refused("that creature has not been under your control since your turn began"): act_unchanged(&mut g,0,mv));
        } else {
            cite!(["702.10b"]; ok: g.act(0,mv));
            // Uncited (fixture): untap to isolate conditional grant while sacrificing one of two.
            g.untap(b);
            g.act(0,Move::Activate{source:b,ability:1,targets:vec![],paying:vec![vec![gob]]}).unwrap();
            cite!(["611.3a","605.3b"]; assert!(g.has(b,"Haste") && g.battlefield.contains(&extra) && g.players[0].pool.total()==2));
        }
    }
}

// Rules: 611.3a, 302.6, 702.10c, 602.2b, 605.3b
// Changing controller changes which other Goblins qualify; newly controlled source taps once it regains haste.
#[test]
fn r611_bolg_controller_change_recomputes_haste() {
    let mut g=setup("Bolg's Company");let b=g.put(0,0,Zone::Battlefield);let old_gob=g.put(0,3,Zone::Battlefield);
    cite!(["611.3a"]; assert!(g.has(b,"Haste")));
    let d=g.defs.len();std::sync::Arc::make_mut(&mut g.defs).push(CardDef::new("Steal","{0}","Instant","Gain control of target creature until end of turn.",None).unwrap());
    until(&mut g,2,Part::PrecombatMain);cast(&mut g,1,d,vec![vec![Target::Object(b)]], "");settle(&mut g);
    cite!(["611.3a"]; assert_eq!((g.controller(b),g.has(b,"Haste")),(1,false)));
    let pay=|o|Move::Activate{source:b,ability:1,targets:vec![],paying:vec![vec![o]]};
    cite!(["302.6"]; assert!(g.summoning_sick(b)));
    let new_gob=g.put(1,3,Zone::Battlefield);
    cite!(["611.3a"]; assert!(g.has(b,"Haste")));
    cite!(["602.2b","701.21a"]; refused("Goblin is not what that cost asks for or cannot pay it"): act_unchanged(&mut g,1,pay(old_gob)));
    g.act(1,pay(new_gob)).unwrap();
    cite!(["702.10c","605.3b"]; assert_eq!((g.players[1].pool.colors[2],g.players[1].pool.colors[3]),(1,1)));
    cite!(["611.3a"]; assert!(!g.has(b,"Haste") && g.battlefield.contains(&old_gob)));
}

#[track_caller]
fn act_unchanged(g:&mut Game,player:usize,action:Move)->Result<(),String>{let before=format!("{g:?}");let result=g.act(player,action);assert_eq!(format!("{g:?}"),before);result}
