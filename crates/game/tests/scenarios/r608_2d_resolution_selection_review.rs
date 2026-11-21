//! Choices made on resolution (608.2d, 609.3, 118.3, 118.12, 701.8b).
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Zone};

fn def(name: &str, ty: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    CardDef::new(name, "{0}", ty, text, pt).unwrap()
}
fn resolve(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
fn cast(g: &mut Game, spending: &str) { let c=g.put(0,0,Zone::Hand); g.act(0,paid(spending,Move::Cast{card:c,targets:vec![]})).unwrap(); resolve(g); }
fn checked(ds: Vec<CardDef>) -> Game {
    for d in &ds { game::play::check(d).unwrap_or_else(|e|panic!("{}: {}",d.name(),e.0)); }
    Game::new(ds,2,20)
}

#[test]
fn r608_2d_a_destruction_choice_excludes_an_indestructible_permanent() {
    // Rules: 608.2d, 701.8b, 702.12b
    let mut g=checked(vec![def("Effect","Sorcery","Destroy a creature.",None),
        def("Bear","Creature — Bear","",Some((2,2))),
        def("Myr","Artifact Creature — Myr","Indestructible",Some((1,1)))]);
    let bear=g.put(0,1,Zone::Battlefield);
    let myr=g.put(1,2,Zone::Battlefield);
    cast(&mut g, "");
    cite!(["608.2d"]; assert!(g.decision.is_none(),"only Bear can be destroyed; no choice is needed: {:?}",g.decision));
    cite!(["701.8a"]; assert!(!g.battlefield.contains(&bear)));
    cite!(["702.12b"]; assert!(g.battlefield.contains(&myr)));
}

#[test]
fn r608_2d_a_battlefield_destination_excludes_cards_cage_prohibits_from_entering() {
    // Rules: 608.2d, 101.2, 609.3
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog=CardDef::catalog(&faces);
    let ds=["Extract from Darkness","Grafdigger's Cage","Grizzly Bears","Forest"].iter()
        .map(|n|CardDef::from_faces(catalog.get(*n).unwrap()).unwrap()).collect();
    let mut g=checked(ds);
    g.put(1,1,Zone::Battlefield);
    let a=g.put(0,2,Zone::Graveyard);
    let b=g.put(1,2,Zone::Graveyard);
    let milled: Vec<Vec<_>> = (0..2).map(|p|(0..2).map(|_|g.put(p,3,Zone::Library)).collect()).collect();
    g.players[0].pool.colors[1]=1; g.players[0].pool.colors[2]=1; g.players[0].pool.colorless=3;
    cast(&mut g, "CCCUB");
    for p in 0..2 {
        // CR404.3: owner authors these simultaneous arrivals' top-first order.
        assert_eq!(g.decision,Some(game::state::Decision::Choose{player:p,choice:game::state::Choice::OrderCards{to:game::state::Placement::Zone(Zone::Graveyard),cards:vec![milled[p][1],milled[p][0]]}}));
        g.act(p,Move::Choose(game::state::Answer::Objects(milled[p].clone()))).unwrap();
    }
    cite!(["608.2d", "101.2"]; assert!(g.decision.is_none(),"no creature card can enter, so none can be chosen: {:?}",g.decision));
    cite!(["609.3"]; assert!(g.players[0].graveyard.contains(&a) && g.players[1].graveyard.contains(&b)));
}

#[test]
fn r118_12_a_zero_object_cost_is_not_lowered_as_a_choice() {
    // Rules: 118.3, 118.12, 608.2d. Exiling zero cards is a payable zero cost; the
    // selection path does not model it, so the text is refused rather than run as
    // an impossible action that gains nothing.
    let d=def("Effect","Sorcery","Exile zero cards from your graveyard. If you do, you gain 3 life.",None);
    crate::support::refused(game::play::check(&d).map_err(|e|e.0),"selected permanent action: fixed bounded count required");
}

#[test]
fn r608_2d_multiple_selection_groups_keep_the_declared_refusal_boundary() {
    // Rules: 608.2d, 118.12
    let d=def("Effect","Sorcery","Put a +1/+1 counter on two creatures you control and a -1/-1 counter on a creature an opponent controls. If you do, you gain 3 life.",None);
    crate::support::refused(game::play::check(&d).map_err(|e|e.0),"selected object: one description");
}

#[test]
fn r608_2d_multiple_movement_groups_keep_the_declared_refusal_boundary() {
    // Rules: 608.2d, 118.12
    let d=def("Effect","Sorcery","Return two creature cards from your graveyard to your hand and a land card from your graveyard to the battlefield. If you do, you gain 3 life.",None);
    crate::support::refused(game::play::check(&d).map_err(|e|e.0),"selected object: one description");
}

#[test]
fn r118_12_an_up_to_cost_an_outcome_reads_is_not_lowered_as_a_choice() {
    // Rules: 118.3, 118.12, 608.2d. Choosing none of "up to one" still starts the cost, so
    // "if you do" would be true; the selection path does not model that, and refuses it.
    let d=def("Effect","Sorcery","Exile up to one card from your graveyard. If you do, you gain 3 life.",None);
    crate::support::refused(game::play::check(&d).map_err(|e|e.0),"selected object: an up-to choice an outcome reads");
}


#[test]
fn r608_2d_an_entry_replacement_does_not_make_the_chosen_return_impossible() {
    // Rules: 608.2d, 614.1c, 400.7
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog=CardDef::catalog(&faces);
    let ds=["Extract from Darkness","Orb of Dreams","Grizzly Bears","Forest"].iter()
        .map(|n|CardDef::from_faces(catalog.get(*n).unwrap()).unwrap()).collect();
    let mut g=checked(ds);
    g.put(1,1,Zone::Battlefield);
    let a=g.put(0,2,Zone::Graveyard);
    let b=g.put(1,2,Zone::Graveyard);
    let milled: Vec<Vec<_>> = (0..2).map(|p|(0..2).map(|_|g.put(p,3,Zone::Library)).collect()).collect();
    g.players[0].pool.colors[1]=1; g.players[0].pool.colors[2]=1; g.players[0].pool.colorless=3;
    cast(&mut g, "CCCUB");
    for p in 0..2 {
        // CR404.3: owner authors these simultaneous arrivals' top-first order.
        assert_eq!(g.decision,Some(game::state::Decision::Choose{player:p,choice:game::state::Choice::OrderCards{to:game::state::Placement::Zone(Zone::Graveyard),cards:vec![milled[p][1],milled[p][0]]}}));
        g.act(p,Move::Choose(game::state::Answer::Objects(milled[p].clone()))).unwrap();
    }
    let Some(game::state::Decision::Choose { player:0, choice:game::state::Choice::Objects { among, count:1, .. } })=&g.decision else { panic!("{:?}",g.decision); };
    cite!(["608.2d"]; assert_eq!(among,&vec![a,b]));
    g.act(0,Move::Choose(game::state::Answer::Objects(vec![b]))).unwrap();
    assert!(g.players[0].graveyard.contains(&a));
    assert!(!g.players[1].graveyard.contains(&b));
    let bear=*g.battlefield.iter().find(|o|g.obj(**o).chars.name=="Grizzly Bears").unwrap();
    cite!(["400.7"]; assert_ne!(bear,b));
    cite!(["614.1d"]; assert_eq!((g.obj(bear).controller,g.obj(bear).owner,g.obj(bear).tapped),(0,1,true)));
}
