use super::*;

fn card(name: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    CardDef::new(name, "", types, text, pt).expect(name)
}

fn creature(name: &str, text: &str) -> CardDef {
    card(name, "Creature — Bear", text, Some((2, 2)))
}

const RETURNS: &str = "At the beginning of your upkeep, you may return this card from your graveyard to your hand.";
const REASSEMBLES: &str = "{1}{B}: Return this card from your graveyard to the battlefield tapped.";

#[test]
fn r113_6_a_definition_keeps_where_each_ability_functions_or_its_refusal() {
    // one scope for each ability, in the order a game numbers them
    let squee = creature("Squee", &format!("Flying\n{RETURNS}\n{REASSEMBLES}\nWhenever this creature attacks, you gain 1 life."));
    let scopes: Vec<_> = (0..5).map(|i| squee.scope(i)).collect();
    let (field, yard) = (Some(Ok(Scope::In(Zone::Battlefield))), Some(Ok(Scope::In(Zone::Graveyard))));
    assert_eq!(scopes, vec![field.clone(), yard.clone(), yard, field, None]);
    assert_eq!((squee.scope_ok(1), squee.scope_ok(4)), (Some(Ok(())), None));

    // an activated meaning of a keyword follows the printed rows and has a scope of its own
    let sword = card("Sword", "Artifact — Equipment", "Equipped creature gets +1/+1.\nEquip {2}", None);
    assert_eq!(sword.keyword_abilities.len(), 1);
    assert_eq!(sword.functioning.scopes.len(), sword.abilities.len() + 1);
    assert_eq!(sword.scope(sword.abilities.len()), Some(Ok(Scope::In(Zone::Battlefield))));

    // what the engine does not follow is refused once, as the definition is loaded, and the
    // definition keeps the refusal
    let risen = creature("Risen", "When this creature dies, return this card from your graveyard to your hand.");
    let refusal = risen.scope_ok(0).expect("an ability").unwrap_err();
    assert!(refusal.0.contains("113.6m"), "{}", refusal.0);
    assert_eq!(risen.scope(0), Some(Err(refusal)));

    // the same definition keeps the same: nothing of a game is in it
    assert_eq!(squee.functioning, creature("Squee", &format!("Flying\n{RETURNS}\n{REASSEMBLES}\nWhenever this creature attacks, you gain 1 life.")).functioning);
}

#[test]
fn r113_6_what_a_definition_says_of_casting_it_is_kept() {
    let ghoul = creature("Ghoul", "As an additional cost to cast this spell, exile three creature cards from your graveyard.\nFlying\nYou may cast this card from your graveyard.");
    // a cost of casting functions on the stack (113.6d), a permission to cast everywhere (113.6f)
    assert_eq!(ghoul.scope(0), Some(Ok(Scope::In(Zone::Stack))));
    assert_eq!(ghoul.scope(2), Some(Ok(Scope::Everywhere)));
    assert!(ghoul.cast_from(Zone::Graveyard));
    assert!(!ghoul.cast_from(Zone::Exile) && !ghoul.cast_from(Zone::Hand));
    let costs: Vec<_> = ghoul.casting_costs().collect();
    let [(0, cost)] = costs.as_slice() else { panic!("one additional cost of ability 0: {costs:?}") };
    assert_eq!(Some(*cost), casting_cost(&ghoul.abilities[0]));

    // each mandatory additional cost is kept, in the order written
    let twice = card("Twice", "Sorcery", "As an additional cost to cast this spell, sacrifice a creature.\nAs an additional cost to cast this spell, sacrifice a creature.\nYou gain 3 life.", None);
    assert_eq!(twice.casting_costs().map(|(i, _)| i).collect::<Vec<_>>(), vec![0, 1]);

    let bear = creature("Bear", "");
    assert!(!bear.cast_from(Zone::Graveyard));
    assert_eq!(bear.casting_costs().count(), 0);

    // "can't be countered" of its own spell functions on the stack (113.6g)
    let sure = creature("Sure", "This spell can't be countered.");
    assert_eq!(sure.scope(0), Some(Ok(Scope::In(Zone::Stack))));
}

#[test]
fn r113_6m_a_game_asks_the_kept_scope_where_the_object_is_or_last_was() {
    let mut game = Game::new(vec![creature("Squee", &format!("{RETURNS}\nWhenever this creature attacks, you gain 1 life."))], 2, 20);
    let there = game.put(0, 0, Zone::Graveyard);
    let here = game.put(0, 0, Zone::Battlefield);
    assert_eq!((game.functions(there, 0), game.functions(there, 1)), (true, false));
    assert_eq!((game.functions(here, 0), game.functions(here, 1)), (false, true));
    // no such ability functions nowhere
    assert!(!game.functions(here, 2) && game.scope(here, 2).is_none());

    // what has left the battlefield functions there still for what looks back (603.10a)
    let gone = game.put(0, 0, Zone::Battlefield);
    game.obj_mut(gone).zone = Zone::Gone;
    assert!(!game.functions(gone, 1));
    game.obj_mut(gone).left = Some(Zone::Battlefield);
    assert_eq!((game.functions(gone, 0), game.functions(gone, 1)), (false, true));
}

#[test]
fn r305_6_an_intrinsic_ability_follows_the_definitions_and_functions_on_the_battlefield() {
    let defs = vec![
        card("Tundra", "Land — Plains Island", "", None),
        card("Forest", "Basic Land — Forest", "({T}: Add {G}.)", None),
        card("Depot", "Land", "{T}: Add {C}.", None),
    ];
    let mut game = Game::new(defs, 2, 20);
    let tundra = game.put(0, 0, Zone::Battlefield);
    let forest = game.put(0, 1, Zone::Battlefield);
    let depot = game.put(0, 2, Zone::Battlefield);
    let buried = game.put(0, 0, Zone::Graveyard);

    // one for each basic land type it has, after what its definition has
    assert_eq!(game.abilities(tundra).len(), 2);
    assert_eq!((game.scope(tundra, 0), game.scope(tundra, 1), game.scope(tundra, 2)), (Some(Scope::In(Zone::Battlefield)), Some(Scope::In(Zone::Battlefield)), None));
    let printed = game.def(forest).unwrap().functioning.scopes.len();
    assert_eq!(game.abilities(forest).len(), printed + 1);
    assert!(game.functions(forest, printed) && !game.functions(forest, printed + 1));
    // a land with no basic land type has none
    assert!(game.functions(depot, 0) && !game.functions(depot, 1));
    // and none functions off the battlefield
    assert!(!game.functions(buried, 0));

    // the rule's own ability, read as any definition is, functions where the rule says
    let rule = card("Forest", "Land", "{T}: Add {G}.", None);
    assert_eq!(rule.scope(0), Some(Ok(Scope::In(Zone::Battlefield))));
}

#[test]
fn r113_6_keywords_keep_individual_scopes_without_an_executable_zone_union() {
    use crate::keyword::{Gives, Paid, Departure, RouteCost};
    use crate::announcement::CastRoute;
    let def = card("Mixed", "Instant", "You gain 1 life.\nKicker {1}, flashback {2}, rebound", None);
    crate::play::check(&def).unwrap();
    let contributions: Vec<_> = crate::keyword::contributions(&def).collect();
    assert_eq!(contributions.len(), 5);
    assert!(matches!(contributions[0], (Scope::In(Zone::Stack), Gives::Additional { then: Paid::Kicked, .. })));
    assert!(matches!(contributions[1], (Scope::In(Zone::Graveyard), Gives::Route { route: CastRoute::Flashback, cost: RouteCost::Alternative(_) })));
    assert!(matches!(contributions[2], (Scope::In(Zone::Stack), Gives::Destination { when: Departure::Any, to: Zone::Exile, .. })));
    assert!(matches!(contributions[3], (Scope::In(Zone::Stack), Gives::Asked)));
    assert!(matches!(contributions[4], (Scope::In(Zone::Stack), Gives::Destination { when: Departure::Resolution, to: Zone::Exile, .. })));
    assert_eq!(def.scope_ok(1), Some(Ok(())));
    assert_eq!(def.scope(1).unwrap().unwrap_err().0, "keyword row has separate contribution scopes (113.6)");
    assert!(!contributions[1].0.contains(Zone::Stack));
    assert!(!contributions[2].0.contains(Zone::Graveyard));
    assert!(!contributions[0].0.contains(Zone::Battlefield));
}
