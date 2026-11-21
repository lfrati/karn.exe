//! Zone changes until an event (CR 610.3), composed with Auras and resolution choices.
use crate::support::{cite, paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

fn game() -> Game {
    let defs = vec![
        CardDef::new("Chained to the Rocks", "{W}", "Enchantment — Aura", "Enchant Mountain you control\nWhen this Aura enters, exile target creature an opponent controls until this Aura leaves the battlefield.", None).unwrap(),
        CardDef::new("Mountain", "", "Basic Land — Mountain", "", None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Release", "{0}", "Instant", "Destroy target enchantment. You gain 2 life.", None).unwrap(),
        CardDef::new("Prison", "{0}", "Enchantment", "{0}: Exile target permanent until this enchantment leaves the battlefield.", None).unwrap(),
        CardDef::new("Aura", "{0}", "Enchantment — Aura", "Enchant creature\nEnchanted creature gets +1/+1.", None).unwrap(),
        CardDef::new("Release and kill", "{0}", "Instant", "Destroy target enchantment. Destroy all creatures.", None).unwrap(),
        CardDef::new("Soul Warden", "{W}", "Creature — Human Cleric", "Whenever another creature enters, you gain 1 life.", Some((1, 1))).unwrap(),
        CardDef::new("Cleanse", "{0}", "Instant", "Destroy all enchantments.", None).unwrap(),
        CardDef::new("Overgrown Tomb", "", "Land — Swamp Forest", "As this land enters, you may pay 2 life. If you don't, it enters tapped.", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0));
    }
    Game::new(defs, 2, 20)
}
fn pass(g: &mut Game) {
    for _ in 0..2 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn cast(g: &mut Game, def: usize, to: ObjId, spending: &str) {
    let card = g.put(0, def, Zone::Hand);
    g.act(0, paid(spending, Move::Cast { card, targets: vec![vec![Target::Object(to)]] })).unwrap();
}
fn chain(g: &mut Game, bear: ObjId) -> ObjId {
    let mountain = g.put(0, 1, Zone::Battlefield);
    g.add_mana(0, ir::mana::ManaSymbol::Color(ir::Color::White));
    cast(g, 0, mountain, "W");
    pass(g);
    let aura = *g.battlefield.last().unwrap();
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(bear)]])).unwrap();
    aura
}
fn prison(g: &mut Game, target: ObjId) -> ObjId {
    let source = g.put(0, 4, Zone::Battlefield);
    g.act(0, Move::Activate { source, ability: 0, targets: vec![vec![Target::Object(target)]], paying: vec![] })
        .unwrap();
    pass(g);
    source
}

#[test]
fn r610_3_a_return_happens_before_the_next_instruction_without_using_the_stack() {
    let mut g = game();
    let bear = g.put(1, 2, Zone::Battlefield);
    let source = chain(&mut g, bear);
    pass(&mut g);
    assert_eq!(g.exile.len(), 1);
    cast(&mut g, 6, source, "");
    pass(&mut g);
    cite!(["610.3"]; assert!(g.exile.is_empty() && g.stack.is_empty()));
    cite!(["610.3"]; assert!(
        g.players[1].graveyard.iter().any(|o| g.obj(*o).card == Some(2)),
        "the returned creature was there for the next instruction to destroy"
    ));
}

#[test]
fn r610_3b_a_source_that_left_before_its_trigger_resolves_exiles_nothing() {
    let mut g = game();
    let bear = g.put(1, 2, Zone::Battlefield);
    let source = chain(&mut g, bear);
    cast(&mut g, 3, source, "");
    pass(&mut g);
    pass(&mut g);
    cite!(["610.3b"]; assert_eq!(g.obj(bear).zone, Zone::Battlefield));
    cite!(["610.3b"]; assert!(g.exile.is_empty()));
}

#[test]
fn r610_3a_a_source_that_left_before_its_activated_ability_resolves_exiles_nothing() {
    let mut g = game();
    let bear = g.put(1, 2, Zone::Battlefield);
    let source = g.put(0, 4, Zone::Battlefield);
    g.act(0, Move::Activate { source, ability: 0, targets: vec![vec![Target::Object(bear)]], paying: vec![] }).unwrap();
    cast(&mut g, 3, source, "");
    pass(&mut g);
    pass(&mut g);
    cite!(["610.3a"]; assert_eq!(g.obj(bear).zone, Zone::Battlefield));
}

#[test]
fn r400_7_the_return_does_not_find_a_card_that_left_exile_and_came_back() {
    let mut g = game();
    let bear = g.put(1, 2, Zone::Battlefield);
    let source = prison(&mut g, bear);
    let exiled = g.exile[0];
    g.move_object(exiled, Zone::Hand);
    let card = g.players[1].hand[0];
    g.exile(card);
    let other = g.exile[0];
    cast(&mut g, 3, source, "");
    pass(&mut g);
    cite!(["400.7"]; assert_eq!(g.exile, vec![other]));
    assert_eq!(g.obj(other).zone, Zone::Exile);
}

#[test]
fn r610_3c_the_object_returns_under_its_owners_control() {
    let mut g = game();
    let bear = g.put(1, 2, Zone::Battlefield);
    let source = prison(&mut g, bear);
    cast(&mut g, 3, source, "");
    pass(&mut g);
    let returned = *g.battlefield.last().unwrap();
    assert_ne!(returned, bear);
    cite!(["610.3c"]; assert_eq!((g.obj(returned).owner, g.obj(returned).controller), (1, 1)));
}

#[test]
fn r610_3d_simultaneous_returns_see_each_other_enter() {
    let mut g = game();
    let bear = g.put(1, 2, Zone::Battlefield);
    let first_prison=prison(&mut g, bear);
    let warden = g.put(1, 7, Zone::Battlefield);
    let second_prison=prison(&mut g, warden);
    let spell = g.put(0, 8, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
    pass(&mut g);
    // CR404.3: explicitly authored owner order, separate from selection.
    assert_eq!(g.decision, Some(game::state::Decision::Choose { player: 0, choice: game::state::Choice::OrderCards { to: game::state::Placement::Zone(Zone::Graveyard), cards: vec![first_prison,second_prison] } }));
    g.act(0, Move::Choose(game::state::Answer::Objects(vec![second_prison,first_prison]))).unwrap();
    cite!(["610.3d", "603.6a"]; assert_eq!(g.stack.len(), 1, "the Warden saw the Bear return with it"));
    pass(&mut g);
    assert_eq!(g.players[1].life, 21);
}

#[test]
fn r610_3_a_returning_aura_chooses_its_attachment_before_the_spell_continues() {
    let mut g = game();
    let bear = g.put(1, 2, Zone::Battlefield);
    cast(&mut g, 5, bear, "");
    pass(&mut g);
    let aura = *g.battlefield.last().unwrap();
    let source = prison(&mut g, aura);
    assert_eq!(g.power(bear), Some(2));
    cast(&mut g, 3, source, "");
    pass(&mut g);
    cite!(["303.4f"]; assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::Attach { .. }, .. }))));
    cite!(["610.3"]; assert_eq!(g.players[0].life, 20, "the later instruction has not happened"));
    g.act(0, Move::Choose(Answer::Attach(Target::Object(bear)))).unwrap();
    cite!(["303.4f"]; assert_eq!(g.power(bear), Some(3)));
    cite!(["608.2c"]; assert_eq!(g.players[0].life, 22));
    assert!(g.stack.is_empty());
}

#[test]
fn r610_3_a_source_exiled_by_its_own_effect_returns_immediately() {
    let mut g = game();
    let source = g.put(0, 4, Zone::Battlefield);
    g.act(0, Move::Activate { source, ability: 0, targets: vec![vec![Target::Object(source)]], paying: vec![] })
        .unwrap();
    pass(&mut g);
    assert_eq!(g.obj(source).zone, Zone::Gone);
    cite!(["610.3"]; assert_eq!(g.battlefield.len(), 1));
    cite!(["610.3"]; assert!(g.exile.is_empty()));
}


#[test]
fn r101_4b_an_until_return_exposes_its_attachment_answer_before_the_next_entry_choice() {
    let mut g = game();
    let bear = g.put(0, 2, Zone::Battlefield);
    cast(&mut g, 5, bear, "");
    pass(&mut g);
    let aura = *g.battlefield.last().unwrap();
    let source = prison(&mut g, aura);
    let land = g.put(0, 9, Zone::Battlefield);
    g.act(0, Move::Activate { source, ability: 0, targets: vec![vec![Target::Object(land)]], paying: vec![] }).unwrap();
    pass(&mut g);
    cast(&mut g, 3, source, "");
    pass(&mut g);
    let entering_aura = g.entering_card().unwrap();
    g.act(0, Move::Choose(Answer::Attach(Target::Object(bear)))).unwrap();
    cite!(["614.12a"]; assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May })));
    cite!(["614.12a"]; assert_eq!(g.obj(entering_aura).zone, Zone::Exile));
    cite!(["101.4b"]; assert_eq!(g.public_choices().cloned().collect::<Vec<_>>(), vec![game::state::PublicChoice::Entry {
        player: 0, card: entering_aura, answer: Answer::Attach(Target::Object(bear)),
    }]));
    g.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["610.3d", "303.4f"]; assert_eq!(g.power(bear), Some(3)));
    cite!(["610.3"]; assert_eq!(g.players[0].life, 22, "the later gain happens after the entire return"));
    assert_eq!(g.public_choices().count(), 0);
}
