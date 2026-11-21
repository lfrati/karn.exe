//! Linked exile is by source incarnation and producing ability (607.2a).
use crate::support::{cite, paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
fn pass(g: &mut Game) {
    for _ in 0..g.players.iter().filter(|p| !p.lost).count() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn defs() -> Vec<CardDef> {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let vat = CardDef::from_face(faces.iter().find(|f| f.name == "Mimic Vat").unwrap()).unwrap();
    game::play::check(&vat).unwrap();
    vec![
        vat,
        CardDef::new("Witness", "{0}", "Creature — Bear", "{0}: You gain 1 life.", Some((2, 2))).unwrap(),
        CardDef::new("Kill", "{0}", "Instant", "Destroy target creature.", None).unwrap(),
    ]
}
fn kill(g: &mut Game, object: ObjId) {
    let spell = g.put(0, 2, Zone::Hand);
    g.priority = 0;
    g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(object)]] }).unwrap();
    pass(g);
}
fn accept(g: &mut Game, player: usize, yes: bool) {
    pass(g);
    assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::May, .. })));
    g.act(player, Move::Choose(if yes { Answer::Yes } else { Answer::No })).unwrap();
}
fn imprint(g: &mut Game, owner: usize) -> ObjId {
    let c = g.put(owner, 1, Zone::Battlefield);
    kill(g, c);
    accept(g, 0, true);
    *g.exile.last().unwrap()
}
fn activate(g: &mut Game, vat: ObjId) {
    let p = g.controller(vat);
    g.priority = p;
    g.untap(vat);
    g.players[p].pool.colorless = 3;
    g.act(p, paid("CCC", Move::Activate { source: vat, ability: 1, targets: vec![], paying: vec![] })).unwrap();
    pass(g);
}
fn select(g: &mut Game, p: usize, card: ObjId) {
    if g.decision.is_some() {
        g.act(p, Move::Choose(Answer::Objects(vec![card]))).unwrap();
    }
}
#[test]
fn r607_2a_full_vat_keeps_the_new_exile_returns_only_the_old_and_honors_declining() {
    let mut g = Game::new(defs(), 3, 20);
    g.put(0, 0, Zone::Battlefield);
    let first = imprint(&mut g, 1);
    assert_eq!(g.exile, vec![first]);
    let second = imprint(&mut g, 2);
    cite!(["607.2a"]; assert_eq!(g.exile, vec![second]));
    assert_eq!(g.players[1].graveyard.len(), 1);
    assert_eq!(g.players[2].graveyard.len(), 0);
    let decline = g.put(1, 1, Zone::Battlefield);
    kill(&mut g, decline);
    accept(&mut g, 0, false);
    cite!(["118.12"]; assert_eq!(g.exile, vec![second]));
    assert_eq!(g.players[1].graveyard.len(), 2);
}
#[test]
fn r607_2a_token_copy_reads_linked_definition_gains_haste_and_is_exiled_at_next_end_step() {
    let mut g = Game::new(defs(), 2, 20);
    let vat = g.put(0, 0, Zone::Battlefield);
    let card = imprint(&mut g, 1);
    activate(&mut g, vat);
    select(&mut g, 0, card);
    let token = *g.battlefield.iter().find(|o| g.obj(**o).token()).unwrap();
    cite!(["707.2"]; assert_eq!(g.obj(token).chars.name, "Witness"));
    assert_eq!(g.power(token), Some(2));
    assert!(g.has(token, "Haste"));
    g.act(0, Move::Activate { source: token, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    pass(&mut g);
    assert_eq!(g.players[0].life, 21);
    g.exile(vat); // the delayed occurrence owns its program and token reference
    while g.part != ir::Part::End {
        if matches!(g.decision, Some(Decision::Attackers { .. })) {
            g.act(g.active, Move::Attack(vec![])).unwrap();
        } else {
            g.act(g.priority, Move::Pass).unwrap();
        }
    }
    pass(&mut g);
    cite!(["603.7"]; assert!(!g.battlefield.contains(&token)));
    assert!(g.exile.contains(&card));
    assert!(!g.exile.iter().any(|o| g.obj(*o).token()));
}
#[test]
fn r607_2a_empty_or_departed_link_creates_no_token_but_activation_cost_is_paid() {
    let mut g = Game::new(defs(), 2, 20);
    let vat = g.put(0, 0, Zone::Battlefield);
    activate(&mut g, vat);
    assert!(g.obj(vat).tapped);
    assert_eq!(g.players[0].pool.total(), 0);
    assert_eq!(g.battlefield.len(), 1);
    let card = imprint(&mut g, 1); // exile the linked card by fixture movement out of exile
    // A different physical vat cannot inherit the old object's link.
    g.exile(vat);
    let fresh = g.put(0, 0, Zone::Battlefield);
    activate(&mut g, fresh);
    cite!(["400.7", "607.2a"]; assert_eq!(g.battlefield.len(), 1));
    assert!(g.exile.contains(&card));
}
#[test]
fn r607_2a_linked_card_choice_is_owned_clonable_and_cannot_take_unrelated_exile() {
    let mut definitions = defs();
    definitions[0]=CardDef::new("Archive","{3}","Artifact","Whenever a nontoken creature dies, you may exile that card.\n{3}, {T}: Create a token that's a copy of a card exiled with this artifact.",None).unwrap();
    game::play::check(&definitions[0]).unwrap();
    definitions.push(CardDef::new("Second witness", "{0}", "Creature — Giant", "", Some((4, 4))).unwrap());
    definitions.push(CardDef::new("Unrelated witness", "{0}", "Creature — Elf", "", Some((1, 1))).unwrap());
    let mut g = Game::new(definitions, 3, 20);
    let vat = g.put(0, 0, Zone::Battlefield);
    let first = imprint(&mut g, 1);
    let second = g.put(2, 3, Zone::Battlefield);
    kill(&mut g, second);
    accept(&mut g, 0, true);
    let linked = *g.exile.last().unwrap();
    let unrelated = g.put(1, 4, Zone::Exile);
    activate(&mut g, vat);
    let Some(Decision::Choose { player: 0, choice: Choice::Objects { among, .. } }) = &g.decision else { panic!("linked-card choice") };
    assert_eq!(among.len(), 2);
    cite!(["607.2a"]; assert!(among.contains(&first) && among.contains(&linked)));
    crate::support::refused(g.act(1, Move::Choose(Answer::Objects(vec![linked]))), "that decision is another player's");
    cite!(["607.2a"]; refused("a chosen object is not among the offered objects to choose (608.2d)"): g.act(0, Move::Choose(Answer::Objects(vec![unrelated]))));
    cite!(["608.2d"]; refused("a decision is pending: a choice of the effect being applied (608.2d)"): g.act(0, Move::Pass));
    let clone = g.clone();
    for mut g in [g, clone] {
        select(&mut g, 0, linked);
        let tokens: Vec<_> = g.battlefield.iter().copied().filter(|o| g.obj(*o).token()).collect();
        assert_eq!(tokens.len(), 1);
        assert_eq!(g.obj(tokens[0]).chars.name, "Second witness");
        assert_eq!(g.seen(tokens[0], None).pt, Some((4, 4)));
        assert!(g.exile.contains(&linked));
        assert!(g.exile.contains(&unrelated));
    }
}
#[test]
fn r607_2a_source_departure_during_activation_does_not_lose_the_retained_link() {
    let mut g = Game::new(defs(), 2, 20);
    let vat = g.put(0, 0, Zone::Battlefield);
    let linked = imprint(&mut g, 1);
    g.players[0].pool.colorless = 3;
    g.act(0, paid("CCC", Move::Activate { source: vat, ability: 1, targets: vec![], paying: vec![] })).unwrap();
    g.exile(vat);
    pass(&mut g);
    select(&mut g, 0, linked);
    cite!(["113.7a"]; assert_eq!(g.battlefield.iter().filter(|o| g.obj(**o).token()).count(), 1));
}

#[test]
fn r607_2a_different_sources_do_not_share_links_and_cannot_exile_the_same_card_twice() {
    let mut g = Game::new(defs(), 2, 20);
    let first = g.put(0, 0, Zone::Battlefield);
    let second = g.put(1, 0, Zone::Battlefield);
    let victim = g.put(0, 1, Zone::Battlefield);
    kill(&mut g, victim);
    accept(&mut g, 1, true);
    pass(&mut g); // the first vat cannot act on the departed graveyard incarnation
    cite!(["607.2a"]; assert!(g.decision.is_none()));
    let linked = g.exile[0];
    activate(&mut g, first);
    cite!(["607.2a"]; assert_eq!(g.battlefield.iter().filter(|o| g.obj(**o).token()).count(), 0));
    activate(&mut g, second);
    select(&mut g, 1, linked);
    let token = *g.battlefield.iter().find(|o| g.obj(**o).token()).unwrap();
    cite!(["111.2"]; assert_eq!(g.controller(token), 1));
}
#[test]
fn return_to_graveyard_parses_the_move_verb_and_refuses_wrong_prepositions() {
    let text = "Return each card exiled with this artifact to its owner's graveyard.";
    assert!(
        CardDef::new("Archive", "{0}", "Artifact", text, None).is_ok(),
        "{:?}",
        CardDef::new("Archive", "{0}", "Artifact", text, None)
    );
    crate::support::refused(CardDef::new("Archive", "{0}", "Artifact", &text.replace(" to ", " into "), None),"text the grammar does not read");
}
