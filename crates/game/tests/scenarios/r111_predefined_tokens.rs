//! Predefined tokens (CR 111.10): "Some effects instruct a player to create a predefined token.
//! These effects use the definition below to determine the characteristics the token is
//! created with."
//!
//! 111.10a A Treasure token is a colorless Treasure artifact token with “{T}, Sacrifice this
//! token: Add one mana of any color.”
//! 111.10b A Food token is a colorless Food artifact token with “{2}, {T}, Sacrifice this token:
//! You gain 3 life.”
//! 111.10f A Clue token is a colorless Clue artifact token with “{2}, Sacrifice this token: Draw
//! a card.”
//!
//! Each token is created by a real card's Oracle text and its ability is activated through the
//! ordinary activation, cost and resolution of 602 and 605.
use crate::support::{cite, mana_pool, paid};
use game::legal::Step;
use game::state::Chars;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::mana::ManaSymbol;
use ir::{CardType, Color, Sort, Subtype};

const PIRATES_PRIZE: usize = 0;
const BAKE_INTO_A_PIE: usize = 1;
const SOKKAS_SWORD_TRAINING: usize = 2;
const BEARS: usize = 3;
const SHOCK: usize = 4;
const ISLAND: usize = 5;
const SIGNET: usize = 6;
const ZOMBIES: usize = 7;

fn card(name: &str, cost: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    let d = CardDef::new(name, cost, types, text, pt).unwrap_or_else(|e| panic!("{name}: {e}"));
    game::play::check(&d).unwrap_or_else(|u| panic!("{name}: {}", u.0));
    d
}

/// Oracle text as printed (oracle-cards.jsonl.gz), reminder text included.
fn defs() -> Vec<CardDef> {
    vec![
        card(
            "Pirate's Prize",
            "{3}{U}",
            "Sorcery",
            "Draw two cards. Create a Treasure token. (It's an artifact with \"{T}, Sacrifice this token: Add one mana of any color.\")",
            None,
        ),
        card(
            "Bake into a Pie",
            "{2}{B}{B}",
            "Instant",
            "Destroy target creature. Create a Food token. (It's an artifact with \"{2}, {T}, Sacrifice this token: You gain 3 life.\")",
            None,
        ),
        card(
            "Sokka's Sword Training",
            "{1}{W}",
            "Instant — Lesson",
            "Target creature gets +2/+2 until end of turn.\nCreate a Clue token. (It's an artifact with \"{2}, Sacrifice this token: Draw a card.\")",
            None,
        ),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Shock", "{R}", "Instant", "Shock deals 2 damage to any target.", None),
        card("Island", "", "Basic Land — Island", "({T}: Add {U}.)", None),
        card("Dimir Signet", "{2}", "Artifact", "{1}, {T}: Add {U}{B}.", None),
        card("Scathe Zombies", "{2}{B}", "Creature — Zombie", "", Some((2, 2))),
    ]
}

/// Exact player-0 library for the draws this scenario executes; other libraries are empty.
fn game(library: &[usize]) -> Game {
    let mut g = Game::new(defs(), 2, 20);
    for def in library.iter().rev() { g.put(0, *def, Zone::Library); }
    g
}

fn mana(g: &mut Game, player: usize, symbols: &[ManaSymbol]) {
    for s in symbols {
        g.add_mana(player, *s);
    }
}

fn pass(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}

fn created(g: &Game) -> Vec<ObjId> {
    g.log.iter().filter_map(|e| match e { Event::Created { token } => Some(*token), _ => None }).collect()
}

fn activate(source: ObjId) -> Move {
    Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }
}

/// Pirate's Prize resolves: the Treasure it created.
fn treasure(g: &mut Game) -> ObjId {
    let [t] = prize(g)[..] else { panic!("one Treasure") };
    t
}

/// Pirate's Prize resolves: the tokens it created that the game did not have before.
fn prize(g: &mut Game) -> Vec<ObjId> {
    let before = created(g);
    mana(g, 0, &[ManaSymbol::Colorless, ManaSymbol::Colorless, ManaSymbol::Colorless, ManaSymbol::Color(Color::Blue)]);
    let prize = g.put(0, PIRATES_PRIZE, Zone::Hand);
    g.act(0, paid("CCCU", Move::Cast { card: prize, targets: vec![] })).unwrap();
    pass(g);
    created(g).into_iter().filter(|t| !before.contains(t)).collect()
}

/// Bake into a Pie resolves, destroying a Grizzly Bears: the Food it created.
fn food(g: &mut Game) -> ObjId {
    let bears = g.put(1, BEARS, Zone::Battlefield);
    mana(g, 0, &[ManaSymbol::Colorless, ManaSymbol::Colorless, ManaSymbol::Color(Color::Black), ManaSymbol::Color(Color::Black)]);
    let pie = g.put(0, BAKE_INTO_A_PIE, Zone::Hand);
    g.act(0, paid("CCBB", Move::Cast { card: pie, targets: vec![vec![Target::Object(bears)]] })).unwrap();
    pass(g);
    let [t] = created(g)[..] else { panic!("one Food") };
    t
}

/// Sokka's Sword Training resolves: the Clue it created.
fn clue(g: &mut Game) -> ObjId {
    let bears = g.put(0, BEARS, Zone::Battlefield);
    mana(g, 0, &[ManaSymbol::Colorless, ManaSymbol::Color(Color::White)]);
    let training = g.put(0, SOKKAS_SWORD_TRAINING, Zone::Hand);
    g.act(0, paid("CW", Move::Cast { card: training, targets: vec![vec![Target::Object(bears)]] })).unwrap();
    pass(g);
    let [t] = created(g)[..] else { panic!("one Clue") };
    t
}

fn graveyard_incarnation(g: &Game, was: ObjId) -> ObjId {
    g.log
        .iter()
        .find_map(|e| match e {
            Event::ZoneChanged { was: w, now: Some(now), to: Zone::Graveyard, .. } if *w == was => Some(*now),
            _ => None,
        })
        .expect("the sacrificed token went to its owner's graveyard")
}

/// The pinned Comprehensive Rules, the edition resources/manifest.json names.
fn rules() -> String {
    let path = oracle::corpus::rules_path().expect("the manifest names the pinned edition");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Every "A[n] X token is …" definition in CR 111.10 is a predefined token, and nothing else.
#[test]
fn r111_10_the_predefined_tokens_are_the_rules_definitions() {
    let cr = rules();
    let mut defined: Vec<&str> = cr
        .lines()
        .filter(|l| l.starts_with("111.10") && l.as_bytes().get(6).is_some_and(u8::is_ascii_lowercase))
        .filter_map(|l| {
            let rest = l.split_once(' ')?.1;
            let rest = rest.strip_prefix("An ").or_else(|| rest.strip_prefix("A "))?;
            Some(rest.split_once(" token is")?.0)
        })
        .collect();
    defined.sort_unstable();
    let mut ours: Vec<&str> = ir::Predefined::ALL.iter().map(|p| p.name()).collect();
    ours.sort_unstable();
    cite!(["111.10"]; assert_eq!(ours, defined));
}

#[test]
fn r111_10_each_definition_is_the_rule_word_for_word() {
    let cr = rules();
    for d in game::token::DEFINED {
        // a definition may end with a cross-reference: "… sorcery.” See rule 701.44, “Explore.”"
        let s = d.sentence();
        cite!(["111.10"]; case: s; assert!(cr.lines().any(|l| l == s || l.starts_with(&format!("{s} See rule "))), "not in the rules: {s}"));
    }
}

#[test]
fn r111_10a_a_treasure_is_what_its_rule_defines_and_no_card() {
    let mut g = game(&[ISLAND, ISLAND]);
    let t = treasure(&mut g);
    let o = g.obj(t);
    // 111.4: the definition names none, so its name is its subtype plus the word "Token"
    cite!(["111.4"]; assert_eq!(o.chars.name, "Treasure Token"));
    cite!(["111.10a"]; assert_eq!(o.chars.types, vec![CardType::Artifact]));
    cite!(["111.10a"]; assert_eq!(o.chars.subtypes.iter().map(|s| s.name()).collect::<Vec<_>>(), vec!["Treasure"]));
    cite!(["111.10a"]; assert!(o.chars.colors.is_empty(), "colorless"));
    assert_eq!((o.owner, o.controller, o.zone), (0, 0, Zone::Battlefield));
    // 111.6: a token is no card, though its text is found as a card's is
    cite!(["111.6"]; assert!(o.token() && o.card.is_none()));
    assert!(!g.is(Target::Object(t), Sort::Card));
    assert!(g.is(Target::Object(t), Sort::Of(CardType::Artifact)));
    cite!(["111.10a"]; assert_eq!(g.abilities(t).len(), 1));
    assert_eq!(g.players[0].hand.len(), 2, "Pirate's Prize also drew two cards");
}

#[test]
fn r605_3b_a_treasure_adds_mana_of_the_color_chosen_without_using_the_stack() {
    for color in [Color::White, Color::Blue, Color::Black, Color::Red, Color::Green] {
        let mut g = game(&[ISLAND, ISLAND]);
        let t = treasure(&mut g);
        assert_eq!(g.players[0].pool.total(), 0);
        // created this turn, and no creature: 602.5a does not keep it from tapping
        g.act(0, Move::Adding(vec![vec![ManaSymbol::Color(color)]], Box::new(activate(t)))).unwrap();
        cite!(["111.10a", "605.3b"]; case: format!("{color:?}"); assert_eq!(g.players[0].pool.colors[color as usize], 1));
        assert_eq!(g.players[0].pool.total(), 1);
        cite!(["605.3b"]; case: format!("{color:?}"); assert!(g.stack.is_empty(), "a mana ability does not go on the stack (605.3b)"));
        cite!(["111.10a"]; case: format!("{color:?}"); assert!(g.log.iter().any(|e| matches!(e, Event::Sacrificed { player: 0, object } if *object == t))));
        assert_eq!(g.obj(t).zone, Zone::Gone);
        assert!(!g.battlefield.contains(&t));
    }
    // "one mana of any color": colorless is no color (106.1b)
    let mut g = game(&[ISLAND, ISLAND]);
    let t = treasure(&mut g);
    cite!(["106.1a"]; refused("that ability doesn't offer the mana said for it: say one of what it offers (608.2d)"): g.act(0, Move::Adding(vec![vec![ManaSymbol::Colorless]], Box::new(activate(t)))));
    assert_eq!((g.obj(t).zone, g.players[0].pool.total()), (Zone::Battlefield, 0));
}

#[test]
fn r605_3a_a_treasure_is_activated_while_a_spell_is_cast_and_while_one_is_on_the_stack() {
    let mut g = game(&[ISLAND, ISLAND]);
    let t = treasure(&mut g);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    let shock = g.put(0, SHOCK, Zone::Hand);
    cite!(["601.2h", "118.3"]; refused("that mana is not in your mana pool (118.3a)"): g.act(0, paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Object(bears)]] }))); // no mana in the pool
    // during casting, as its cost is paid (601.2g)
    let step = Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source: t, ability: 0, paying: vec![], adding: vec![vec![ManaSymbol::Color(Color::Red)]] };
    cite!(["605.3a", "601.2g"]; ok: g.act(0, Move::WithMana(vec![step], Box::new(paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Object(bears)]] })))));
    assert_eq!(g.stack.len(), 1, "Shock alone is on the stack");
    assert_eq!(g.obj(t).zone, Zone::Gone);
    cite!(["601.2g"]; assert_eq!(g.players[0].pool.total(), 0, "the Treasure's mana paid for it"));
    // with a spell on the stack, holding priority (117.1d), another Treasure adds its mana at once
    let mut g = game(&[ISLAND, ISLAND]);
    let t = treasure(&mut g);
    let shock = g.put(0, SHOCK, Zone::Hand);
    g.add_mana(0, ManaSymbol::Color(Color::Red));
    g.act(0, paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Player(1)]] })).unwrap();
    assert_eq!(g.priority, 0);
    cite!(["605.3a"]; ok: g.act(0, Move::Adding(vec![vec![ManaSymbol::Color(Color::Green)]], Box::new(activate(t)))));
    assert_eq!((g.stack.len(), g.players[0].pool.colors[Color::Green as usize]), (1, 1));
    pass(&mut g);
    assert_eq!(g.players[1].life, 18);
}

#[test]
fn r111_10b_a_food_is_paid_for_tapped_sacrificed_and_gains_3_life_as_it_resolves() {
    let mut g = game(&[]);
    let f = food(&mut g);
    assert_eq!(g.obj(f).chars.name, "Food Token");
    assert!(g.obj(f).token());
    // offered to its controller when the cost can be paid
    mana(&mut g, 0, &[ManaSymbol::Colorless, ManaSymbol::Colorless]);
    g.act(0, paid("CC", activate(f))).unwrap();
    // the cost is paid as it is activated (602.2b, 601.2h): mana, tapping and sacrifice
    cite!(["601.2h", "602.2b"]; assert_eq!(g.players[0].pool.total(), 0));
    assert!(g.log.iter().any(|e| matches!(e, Event::Tapped { object } if *object == f)));
    cite!(["111.10b"]; assert!(g.log.iter().any(|e| matches!(e, Event::Sacrificed { player: 0, object } if *object == f))));
    assert_eq!(g.obj(f).zone, Zone::Gone);
    // not a mana ability: it waits on the stack, its source gone (113.7a)
    cite!(["113.7a"]; assert_eq!(g.stack.len(), 1));
    assert_eq!(g.obj(g.stack[0]).ability, Some((Some(f), 0)));
    assert_eq!(g.players[0].life, 20);
    pass(&mut g);
    cite!(["111.10b"]; assert_eq!(g.players[0].life, 23));
    assert!(g.stack.is_empty());
}

#[test]
fn r602_2_a_food_is_not_activated_without_its_whole_cost_or_by_another_player() {
    let mut g = game(&[]);
    let f = food(&mut g);
    // {2} is not paid with one mana (601.2h)
    g.add_mana(0, ManaSymbol::Colorless);
    cite!(["601.2h", "118.3"]; refused("your mana pool cannot pay that cost (118.3a): {2}"): g.act(0, activate(f)));
    assert_eq!((g.obj(f).zone, g.obj(f).tapped, g.players[0].pool.total()), (Zone::Battlefield, false, 1));
    assert!(g.stack.is_empty());
    // only its controller activates it (602.2)
    g.add_mana(1, ManaSymbol::Colorless);
    g.add_mana(1, ManaSymbol::Colorless);
    g.act(0, Move::Pass).unwrap();
    assert_eq!(g.priority, 1);
    cite!(["602.2"]; refused("only an object's controller can activate its ability (602.2)"): g.act(1, activate(f)));
    assert_eq!(g.obj(f).zone, Zone::Battlefield);
    // {T} cannot be paid by a tapped Food (107.5)
    let mut g = game(&[]);
    let f = food(&mut g);
    mana(&mut g, 0, &[ManaSymbol::Colorless, ManaSymbol::Colorless]);
    g.set_tapped(f, true);
    cite!(["107.5"]; refused("Food Token can't pay {T} as it is (107.5, 118.3)"): g.act(0, activate(f)));
    assert_eq!((g.obj(f).zone, g.players[0].pool.total()), (Zone::Battlefield, 2));
}

#[test]
fn r111_10f_a_clue_is_paid_for_and_sacrificed_and_its_ability_draws_after_it_is_gone() {
    let mut g = game(&[ISLAND]);
    let c = clue(&mut g);
    assert_eq!(g.obj(c).chars.name, "Clue Token");
    // its cost has no {T}: a tapped Clue is activated all the same
    g.set_tapped(c, true);
    mana(&mut g, 0, &[ManaSymbol::Color(Color::Blue), ManaSymbol::Colorless]);
    let hand = g.players[0].hand.len();
    cite!(["111.10f"]; ok: g.act(0, paid("CU", activate(c))));
    let there = graveyard_incarnation(&g, c);
    assert_eq!(g.obj(c).zone, Zone::Gone);
    // the token left the battlefield and ceased to exist at state-based actions (111.7, 704.5d)
    cite!(["111.7", "704.5d"]; assert_eq!(g.obj(there).zone, Zone::Gone));
    assert!(!g.players[0].graveyard.contains(&there));
    assert_eq!(g.stack.len(), 1);
    assert_eq!(g.players[0].hand.len(), hand);
    pass(&mut g);
    cite!(["111.10f", "113.7a"]; assert_eq!(g.players[0].hand.len(), hand + 1, "it resolves without its source (113.7a)"));
}

#[test]
fn r701_21a_a_sacrificed_clue_cannot_be_activated_again() {
    let mut g = game(&[]);
    let c = clue(&mut g);
    mana(&mut g, 0, &[ManaSymbol::Colorless; 4]);
    g.act(0, paid("CC", activate(c))).unwrap();
    let there = graveyard_incarnation(&g, c);
    cite!(["400.7", "113.6"]; refused("an ability of a permanent on the battlefield is activated (113.6)"): g.act(0, activate(c))); // what left is no permanent (400.7)
    cite!(["400.7", "113.6"]; refused("an ability of a permanent on the battlefield is activated (113.6)"): g.act(0, activate(there))); // nor is the token that left the battlefield
    assert_eq!((g.stack.len(), g.players[0].pool.total()), (1, 2));
    // no insufficiently paid Clue either: with one mana left
    let mut g = game(&[]);
    let c = clue(&mut g);
    g.add_mana(0, ManaSymbol::Colorless);
    cite!(["601.2h", "118.3"]; refused("your mana pool cannot pay that cost (118.3a): {2}"): g.act(0, activate(c)));
    assert_eq!(g.obj(c).zone, Zone::Battlefield);
}

#[test]
fn r111_7_a_token_put_into_a_graveyard_another_way_ceases_to_exist() {
    let mut g = game(&[ISLAND, ISLAND]);
    let t = treasure(&mut g);
    assert!(g.destroy(t));
    let there = graveyard_incarnation(&g, t);
    assert!(g.obj(there).token());
    assert_eq!(g.obj(there).chars.name, "Treasure Token");
    cite!(["111.7"]; assert!(g.state_based_actions()));
    cite!(["111.7"]; assert_eq!(g.obj(there).zone, Zone::Gone));
    assert!(!g.players[0].graveyard.contains(&there));
}

#[test]
fn a_game_clone_has_its_own_token_and_the_same_definition() {
    let mut g = game(&[ISLAND, ISLAND]);
    let t = treasure(&mut g);
    let mut other = g.clone();
    other.act(0, Move::Adding(vec![vec![ManaSymbol::Color(Color::Red)]], Box::new(activate(t)))).unwrap();
    assert_eq!(other.obj(t).zone, Zone::Gone);
    assert_eq!((g.obj(t).zone, g.players[0].pool.total()), (Zone::Battlefield, 0), "the original is untouched");
    g.act(0, Move::Adding(vec![vec![ManaSymbol::Color(Color::Green)]], Box::new(activate(t)))).unwrap();
    assert_eq!(g.players[0].pool.colors[Color::Green as usize], 1);
    assert!(std::ptr::eq(g.def(t).unwrap(), other.def(t).unwrap()), "one immutable definition");
}

#[test]
fn r111_10_other_predefined_tokens_are_refused_naming_their_rule() {
    // Gild: "Exile target creature. Create a Gold token." (111.10c)
    let gild = CardDef::new(
        "Gild",
        "{3}{B}",
        "Sorcery",
        "Exile target creature. Create a Gold token. (It's an artifact with \"Sacrifice this token: Add one mana of any color.\")",
        None,
    )
    .unwrap();
    let refused = game::play::check(&gild).unwrap_err().0;
    assert!(refused.contains("token: Gold (111.10)"), "{refused}");
    for kind in ir::Predefined::ALL {
        let supported = game::token::DEFINED.iter().any(|d| d.kind == *kind);
        assert_eq!(game::token::definition(*kind).is_ok(), supported, "{}", kind.name());
    }
}

/// A described token (111.4) with what a Treasure has but its definition: "a colorless Treasure
/// artifact token", no ability, named "Treasure Token" as a token whose text names none is.
///
/// No card in oracle-cards.jsonl.gz creates one: every card text that describes a Treasure
/// token gives it the Treasure's ability (Diabolical Salvation's "a colorless Treasure
/// artifact token with '{T}, Sacrifice this artifact: …'", which the engine refuses as a
/// token with abilities) or other subtypes (Vraska, Soul of Stone's "Sculpture Treasure"
/// token, named "Sculpture Treasure Token"). So it is created with [`Game::create_token`],
/// the creation a described token's resolution performs, with the characteristics that
/// resolution would give it.
fn inert_treasure_named(g: &mut Game) -> ObjId {
    let chars = Chars {
        name: "Treasure Token".into(),
        supertypes: vec![],
        types: vec![CardType::Artifact],
        subtypes: vec![Subtype::from_name("Treasure").unwrap()],
        colors: vec![],
        pt: None,
        mana_value: 0,
        keywords: vec![],
        predefined: None,
    };
    g.create_token(0, chars, false)
}

#[test]
fn r111_10a_a_treasure_pays_beside_a_described_token_of_its_name_that_has_no_ability() {
    for inert_first in [true, false] {
        let mut g = game(&[ISLAND, ISLAND]);
        let island = g.put(0, ISLAND, Zone::Battlefield);
        let signet = g.put(0, SIGNET, Zone::Battlefield);
        let (inert, t) = if inert_first {
            let inert = inert_treasure_named(&mut g);
            (inert, treasure(&mut g))
        } else {
            let t = treasure(&mut g);
            (inert_treasure_named(&mut g), t)
        };
        let order = if inert_first { "the described token first" } else { "the Treasure first" };
        // the same name and characteristics, but only one is the Treasure of 111.10a
        cite!(["111.10a"]; assert_eq!(g.obj(inert).chars, Chars { predefined: None, ..g.obj(t).chars.clone() }, "{order}"));
        assert!(g.obj(inert).token() && g.obj(inert).card.is_none());
        cite!(["111.10a"]; case: order; assert_eq!((g.abilities(t).len(), g.abilities(inert).len()), (1, 0), "{order}"));
        let zombies = g.put(0, ZOMBIES, Zone::Hand);
        let step = |source| game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source, ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None };
        let before=format!("{g:?}");
        cite!(["111.10a"]; refused("that is not an activated ability of that permanent"): g.act(0, Move::Activate { source: inert, ability: 0, targets: vec![], paying: vec![] }));
        assert_eq!(format!("{g:?}"),before);
        let plan = vec![step(island), game::legal::Step { plan: vec![], x: None, life: vec![], with: Some(mana_pool("U")), ..step(signet) }, game::legal::Step { plan: vec![], x: None, life: vec![], with: None, adding: vec![vec![ManaSymbol::Color(Color::Black)]], ..step(t) }];
        g.act(0, paid("UBB", Move::WithMana(plan.clone(), Box::new(Move::Cast { card: zombies, targets: vec![] })))).unwrap();
        assert!(g.log.iter().any(|e| matches!(e, Event::Sacrificed { player: 0, object } if *object == t)), "{order}");
        assert_eq!(g.obj(t).zone, Zone::Gone);
        cite!(["111.10a"]; case: order; assert_eq!((g.obj(inert).zone, g.obj(inert).tapped), (Zone::Battlefield, false), "{order}: the other token is untouched"));
        assert_eq!(g.players[0].pool.total(), 0);
        pass(&mut g);
        assert!(g.stack.is_empty());
        let resolved: Vec<ObjId> = g.battlefield.iter().copied().filter(|o| g.obj(*o).chars.name == "Scathe Zombies").collect();
        assert_eq!(resolved.len(), 1, "{order}: the Zombies resolved");
        assert_eq!(g.obj(resolved[0]).controller, 0);
    }
}

#[test]
fn r601_2g_two_treasures_pay_a_cost_of_two_mana() {
    let mut g = game(&[ISLAND, ISLAND, ISLAND, ISLAND]);
    let first = treasure(&mut g);
    let [second] = prize(&mut g)[..] else { panic!("a second Treasure") };
    assert_ne!(first, second);
    let bears = g.put(0, BEARS, Zone::Hand);
    assert_eq!(g.players[0].pool.total(), 0);
    // no other mana: {1}{G} is paid by sacrificing both Treasures as it is cast (601.2g)
    let plan: Vec<_> = [first, second].into_iter().map(|source| game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source, ability: 0, paying: vec![], adding: vec![vec![ManaSymbol::Color(Color::Green)]], chose: vec![], cost_branches: None }).collect();
    g.act(0, paid("GG", Move::WithMana(plan.clone(), Box::new(Move::Cast { card: bears, targets: vec![] })))).unwrap();
    for t in [first, second] {
        assert!(g.log.iter().any(|e| matches!(e, Event::Sacrificed { player: 0, object } if *object == t)));
        assert_eq!(g.obj(t).zone, Zone::Gone);
    }
    cite!(["601.2g"]; assert_eq!((g.stack.len(), g.players[0].pool.total()), (1, 0)));
    pass(&mut g);
    assert!(g.stack.is_empty());
    assert!(g.battlefield.iter().any(|o| g.obj(*o).chars.name == "Grizzly Bears" && g.obj(*o).controller == 0), "the Bears resolved");
}
