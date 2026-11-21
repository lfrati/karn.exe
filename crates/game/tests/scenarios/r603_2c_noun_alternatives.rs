//! Complete noun-phrase alternatives denote one union, not separate trigger occurrences.
use crate::support::{cite, mana_pool, paid};
use game::state::Pool;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use game::legal::Asked;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let mut defs: Vec<_> = ["Runescale Stormbrood // Chilling Screech", "Danitha, Sword of Hope", "Maelstrom of the Spirit Dragon"]
        .iter().map(|name| CardDef::from_faces(catalog.get(name).unwrap()).unwrap()).collect();
    defs.extend([
        CardDef::new("Dragon", "{0}", "Creature — Dragon", "", Some((2,2))).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2,2))).unwrap(),
        CardDef::new("Ordinary spell", "{0}", "Instant", "You gain 1 life.", None).unwrap(),
        CardDef::new("Equipment", "{0}", "Artifact — Equipment", "Flash\nEquip {0}", None).unwrap(),
        CardDef::new("Targeted spell", "{0}", "Instant", "Target creature gets +1/+0 until end of turn.", None).unwrap(),
        // Synthetic overlap: a Kindred noncreature spell can also have the Dragon subtype.
        CardDef::new("Both Runescale branches", "{0}", "Kindred Instant — Dragon", "You gain 1 life.", None).unwrap(),
        // Synthetic overlap: one spell is both Equipment and an Aura targeting a creature.
        CardDef::new("Both Danitha branches", "{0}", "Artifact Enchantment — Aura Equipment", "Enchant creature", None).unwrap(),
        CardDef::new("Mirror", "{0}", "Instant", "Copy target instant or sorcery spell.", None).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
    ]);
    for def in &defs { game::play::check(def).unwrap_or_else(|e| panic!("{}: {e:?}", def.name())); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..12 { g.put(p, 11, Zone::Library); } }
    g
}
fn cast(g: &mut Game, player: usize, definition: usize, targets: Vec<Vec<Target>>, with: Pool) {
    while g.priority != player { g.act(g.priority, Move::Pass).unwrap(); }
    let card = g.put(player, definition, Zone::Hand);
    g.act(player, Move::Paying(with, Box::new(Move::Cast { card, targets }))).unwrap();
}
fn settle(g: &mut Game) {
    for _ in 0..20 {
        if g.stack.is_empty() { return; }
        assert!(matches!(g.asked(), Some((_, Asked::Priority))));
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    }
    panic!("unsettled stack");
}
fn object(o: ObjId) -> Vec<Vec<Target>> { vec![vec![Target::Object(o)]] }

// Rules: 603.2c, 603.3a, 112.1
// Given: the full Runescale Oracle watches its controller's cast events.
// Expect: either/both alternatives produce one trigger; neither/opponent produces none.
#[test]
fn r603_2c_runescale_either_both_neither_and_controller_are_one_boolean_match() {
    let original = game();
    for (player, definition, bonus) in [(0,5,2), (0,3,2), (0,8,2), (0,4,0), (1,5,0)] {
        let mut g = original.clone();
        let source = g.put(0, 0, Zone::Battlefield);
        cast(&mut g, player, definition, vec![], mana_pool(""));
        cite!(["603.2c", "603.3a"]; assert_eq!(g.stack.len(), if bonus == 0 { 1 } else { 2 }));
        settle(&mut g);
        cite!(["603.2c"]; assert_eq!(g.power(source), Some(2 + bonus)));
    }
}

// Rules: 603.2c, 707.10
// Expect: casting the copy-producing spell triggers Runescale, but its created stack copy does not.
#[test]
fn r707_10_a_stack_copy_is_not_another_cast_matching_the_union() {
    let mut g = game();
    let source = g.put(0, 0, Zone::Battlefield);
    cast(&mut g, 0, 5, vec![], mana_pool(""));
    let original = g.stack[0];
    // Resolve only Runescale's first trigger, retaining the original spell as a copy target.
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    assert_eq!(g.power(source), Some(4));
    cast(&mut g, 0, 10, object(original), mana_pool(""));
    settle(&mut g);
    cite!(["603.2c", "707.10"]; assert_eq!(g.power(source), Some(6)));
    cite!(["707.10"]; assert_eq!(g.players[0].life, 22));
}

// Rules: 603.2c, 603.3a, 115.9b
// Given: Danitha's full Oracle has Equipment OR targeting a creature you control.
// Expect: each branch and their overlap draw once; opponent-controlled targets/casts do not.
#[test]
fn r603_2c_danitha_qualifiers_stay_with_the_targeting_arm() {
    let original = game();
    for (player, definition, target_owner, draw) in [(0,6,None,true), (0,7,Some(0),true), (0,9,Some(0),true), (0,7,Some(1),false), (0,5,None,false), (1,6,None,false)] {
        let mut g = original.clone();
        g.put(0, 1, Zone::Battlefield);
        let targets = target_owner.map(|owner| { let creature = g.put(owner, 4, Zone::Battlefield); object(creature) }).unwrap_or_default();
        cast(&mut g, player, definition, targets, mana_pool(""));
        cite!(["603.2c", "603.3a", "115.9b"]; assert_eq!(g.stack.len(), if draw { 2 } else { 1 }));
        settle(&mut g);
        cite!(["603.2c"]; assert_eq!(g.players[0].hand.len(), usize::from(draw)));
    }
}

// Rules: 603.2c, 603.3a; Danitha's printed once-each-turn restriction.
// Expect: changing which branch matches doesn't create a separate quota; the next turn resets it.
#[test]
fn r603_2c_danitha_one_quota_covers_both_branches_and_resets_next_turn() {
    let mut g = game();
    g.put(0, 1, Zone::Battlefield);
    let creature = g.put(0, 4, Zone::Battlefield);
    cast(&mut g, 0, 6, vec![], mana_pool("")); settle(&mut g);
    assert_eq!(g.players[0].hand.len(), 1);
    cast(&mut g, 0, 7, object(creature), mana_pool(""));
    cite!(["603.2c"]; assert_eq!(g.stack.len(), 1)); settle(&mut g);
    assert_eq!(g.players[0].hand.len(), 1);
    let turn = g.turn;
    for _ in 0..100 {
        if g.turn != turn { break; }
        let (player, asked) = g.asked().unwrap();
        g.act(player, match asked { Asked::Priority => Move::Pass, Asked::Attackers(_) => Move::Attack(vec![]), other => panic!("{other:?}") }).unwrap();
    }
    assert_ne!(g.turn, turn);
    cast(&mut g, 0, 7, object(creature), mana_pool(""));
    cite!(["603.2c"]; assert_eq!(g.stack.len(), 2)); settle(&mut g);
    assert_eq!(g.players[0].hand.len(), 2);
}

fn restricted_mana(g: &mut Game, color: ir::Color) {
    let land = g.put(0, 2, Zone::Battlefield);
    let activate = Move::Activate { source: land, ability: 1, targets: vec![], paying: vec![] };
    g.act(0, Move::Adding(vec![vec![ir::ManaSymbol::Color(color)]], Box::new(activate))).unwrap();
}

// Rules: 106.6, 601.2f; Maelstrom's full Oracle restriction.
// Expect: colored or generic Dragon costs accept the restricted unit, unrelated spells
// and activated abilities refuse it; its distinct unrestricted C ability remains usable.
#[test]
fn r106_6_maelstrom_spell_union_preserves_colored_generic_and_activation_restrictions() {
    let mut original = game();
    let defs = [
        CardDef::new("Colored Dragon", "{R}", "Creature — Dragon", "", Some((2,2))).unwrap(),
        CardDef::new("Generic Dragon", "{1}", "Creature — Dragon", "", Some((2,2))).unwrap(),
        CardDef::new("Unrelated", "{R}", "Instant", "You gain 1 life.", None).unwrap(),
        CardDef::new("Activation", "{0}", "Creature — Bear", "{1}: You gain 1 life.", Some((2,2))).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap(); }
    let index = original.defs.len(); std::sync::Arc::make_mut(&mut original.defs).extend(defs);
    for offset in [0,1] {
        let mut g = original.clone(); restricted_mana(&mut g, ir::Color::Red);
        let selected = Pool { restricted: vec![g.players[0].pool.restricted[0].clone()], ..Pool::default() };
        cast(&mut g, 0, index + offset, vec![], selected);
        cite!(["106.6"]; assert_eq!(g.players[0].pool.total(), 0));
    }
    let mut g = original.clone(); restricted_mana(&mut g, ir::Color::Red);
    let selected = Pool { restricted: vec![g.players[0].pool.restricted[0].clone()], ..Pool::default() };
    let card = g.put(0, index + 2, Zone::Hand);
    cite!(["106.6"]; refused("restricts how it can be spent"): g.act(0, Move::Paying(selected.clone(), Box::new(Move::Cast { card, targets: vec![] }))));
    let creature = g.put(0, index + 3, Zone::Battlefield);
    cite!(["106.6"]; refused("restricts how it can be spent"): g.act(0, Move::Paying(selected.clone(), Box::new(Move::Activate { source: creature, ability: 0, targets: vec![], paying: vec![] }))));
    let land = g.put(0, 2, Zone::Battlefield);
    g.act(0, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    cite!(["106.6"]; ok: g.act(0, paid("C", Move::Activate { source: creature, ability: 0, targets: vec![], paying: vec![] })));
}

// Rules: 106.6, 720.3a, 715.3a
// Expect: actual Omen characteristics match; a synthetic non-Omen/non-Dragon Adventure
// doesn't become eligible merely because the physical card's normal face is a Dragon.
#[test]
fn r106_6_maelstrom_uses_announced_omen_or_adventure_characteristics() {
    let mut g = game();
    cast(&mut g, 1, 5, vec![], mana_pool(""));
    let target = g.stack[0];
    while g.priority != 0 { g.act(g.priority, Move::Pass).unwrap(); }
    restricted_mana(&mut g, ir::Color::Blue);
    g.players[0].pool.colorless = 1;
    let selected = Pool { colorless: 1, restricted: vec![g.players[0].pool.restricted[0].clone()], ..Pool::default() };
    let omen = g.put(0, 0, Zone::Hand);
    cite!(["106.6", "720.3a"]; ok: g.act(0, Move::Paying(selected, Box::new(Move::CastAs(game::card::CastAs::Omen, Box::new(Move::Cast { card: omen, targets: object(target) }))))));
    cite!(["106.6"]; assert_eq!(g.players[0].pool.total(), 0));

    let normal = CardDef::new("Adventure Dragon", "{U}", "Creature — Dragon", "", Some((2,2))).unwrap();
    let alternative = CardDef::new("Unrelated adventure", "{U}", "Instant — Adventure", "You gain 1 life.", None).unwrap();
    let def = CardDef::adventurer(normal, alternative).unwrap(); game::play::check(&def).unwrap();
    let mut g = game(); let index = g.defs.len(); std::sync::Arc::make_mut(&mut g.defs).push(def);
    restricted_mana(&mut g, ir::Color::Blue);
    let selected = Pool { restricted: vec![g.players[0].pool.restricted[0].clone()], ..Pool::default() };
    let card = g.put(0, index, Zone::Hand);
    cite!(["106.6", "715.3a"]; refused("restricts how it can be spent"): g.act(0, Move::Paying(selected, Box::new(Move::CastAs(game::card::CastAs::Adventure, Box::new(Move::Cast { card, targets: vec![] }))))));
}

// Rules: 115.9b, 115.10a, 601.2h, 601.2i, 603.2c
// Given: actual target groups contain two creatures, or a target sacrificed during payment.
// Expect: multiple matching targets still draw once; departed targets are ignored without LKI;
// a surviving matching target still suffices; an effect affecting creatures without targeting does not.
#[test]
fn r115_9b_target_membership_uses_current_zone_not_departed_target_lki() {
    let mut original = game();
    original.put(0, 1, Zone::Battlefield);
    let first = original.put(0, 4, Zone::Battlefield);
    let second = original.put(0, 4, Zone::Battlefield);
    let defs = [
        CardDef::new("Several targets", "{0}", "Instant", "Up to two target creatures get +1/+0 until end of turn.", None).unwrap(),
        CardDef::new("Departed target", "{0}", "Instant", "As an additional cost to cast this spell, sacrifice a creature.\nTarget creature gets +1/+0 until end of turn.", None).unwrap(),
        CardDef::new("One surviving target", "{0}", "Instant", "As an additional cost to cast this spell, sacrifice a creature.\nUp to two target creatures get +1/+0 until end of turn.", None).unwrap(),
        CardDef::new("No targets", "{0}", "Instant", "Creatures you control get +1/+0 until end of turn.", None).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap(); }
    let index = original.defs.len(); std::sync::Arc::make_mut(&mut original.defs).extend(defs);
    let mut g = original.clone();
    cast(&mut g, 0, index, vec![vec![Target::Object(first), Target::Object(second)]], mana_pool(""));
    cite!(["603.2c", "115.9b"]; assert_eq!(g.stack.len(), 2)); settle(&mut g);
    assert_eq!(g.players[0].hand.len(), 1);
    for (offset, targets, triggers) in [(1, object(first), false),
        (2, vec![vec![Target::Object(first), Target::Object(second)]], true)] {
        let mut g = original.clone();
        let card = g.put(0, index + offset, Zone::Hand);
        cite!(["601.2h", "601.2i"]; ok: g.act(0, Move::WithCosts(vec![vec![first]], Box::new(Move::Cast { card, targets }))));
        cite!(["115.9b", "603.2c"]; assert_eq!(g.stack.len(), if triggers { 2 } else { 1 }));
    }
    let mut g = original.clone(); cast(&mut g, 0, index + 3, vec![], mana_pool(""));
    cite!(["115.10a"]; assert_eq!(g.stack.len(), 1)); settle(&mut g);
    assert_eq!(g.players[0].hand.len(), 0);
}

// Rules: 115.9b, 115.9c
// Bounded existential membership does not approximate exclusive or counted predicates.
#[test]
fn r115_9c_unsupported_exclusive_or_counted_target_predicates_refuse() {
    for text in ["Whenever you cast a spell that targets only a single creature you control, draw a card.",
        "Whenever you cast a spell that targets two creatures you control, draw a card."] {
        let def = CardDef::new("Target limitation", "{0}", "Creature", text, Some((2,2))).unwrap();
        cite!(["115.9b", "115.9c"]; assert!(game::play::check(&def).unwrap_err().0.contains("target description: singular existential alternatives required")));
    }
}
