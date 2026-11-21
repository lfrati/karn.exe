//! Investigate (CR 701.16): "701.16a “Investigate” means “Create a Clue token.” See rule
//! 111.10f." The keyword action is the ordinary creation of the predefined token, by whoever
//! the text says investigates, as many times as it says, under whatever condition it says;
//! the Clue it makes is the one 111.10f defines, activated like any other.
//!
//! Each card is its Oracle text as printed (oracle-cards.jsonl.gz), reminder text included.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::mana::ManaSymbol;
use ir::{CardType, Color, Subtype};

const INSPECTOR: usize = 0;
const SUSPICIONS: usize = 1;
const ABSENCE: usize = 2;
const CANAL: usize = 3;
const TRACKER: usize = 4;
const BEARS: usize = 5;
const SHOCK: usize = 6;
const FOREST: usize = 7;
const ISLAND: usize = 8;

const CLUE: &str = "(Create a Clue token. It's an artifact with \"{2}, Sacrifice this token: Draw a card.\")";

fn card(name: &str, cost: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    let d = CardDef::new(name, cost, types, text, pt).unwrap_or_else(|e| panic!("{name}: {e}"));
    game::play::check(&d).unwrap_or_else(|u| panic!("{name}: {}", u.0));
    d
}

fn defs() -> Vec<CardDef> {
    vec![
        card("Thraben Inspector", "{W}", "Creature — Human Soldier", &format!("When this creature enters, investigate. {CLUE}"), Some((1, 2))),
        card("Confirm Suspicions", "{3}{U}{U}", "Instant",
            "Counter target spell.\nInvestigate three times. (To investigate, create a Clue token. It's an artifact with \"{2}, Sacrifice this token: Draw a card.\")", None),
        card("Fateful Absence", "{1}{W}", "Instant",
            "Destroy target creature or planeswalker. Its controller investigates. (They create a Clue token. It's an artifact with \"{2}, Sacrifice this token: Draw a card.\")", None),
        card("Drag the Canal", "{U}{B}", "Instant",
            &format!("Create a 2/2 white and blue Detective creature token. If a creature died this turn, you gain 2 life, surveil 2, then investigate. {CLUE}"), None),
        card("Tireless Tracker", "{2}{G}", "Creature — Human Scout",
            &format!("Landfall — Whenever a land you control enters, investigate. {CLUE}\nWhenever you sacrifice a Clue, put a +1/+1 counter on this creature."), Some((3, 2))),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Shock", "{R}", "Instant", "Shock deals 2 damage to any target.", None),
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        card("Island", "", "Basic Land — Island", "({T}: Add {U}.)", None),
    ]
}

/// Libraries of `library` Islands each.
fn game(library: usize) -> Game {
    let mut g = Game::new(defs(), 2, 20);
    for p in 0..2 {
        for _ in 0..library {
            g.put(p, ISLAND, Zone::Library);
        }
    }
    g
}

fn mana(g: &mut Game, player: usize, symbols: &[ManaSymbol]) {
    for s in symbols {
        g.add_mana(player, *s);
    }
}

fn generic(n: usize) -> Vec<ManaSymbol> { vec![ManaSymbol::Colorless; n] }

/// Every player passes until the stack is empty.
fn resolve(g: &mut Game) {
    for _ in 0..40 {
        if g.stack.is_empty() { return; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("the stack did not empty");
}

fn created(g: &Game) -> Vec<ObjId> {
    g.log.iter().filter_map(|e| match e { Event::Created { token } => Some(*token), _ => None }).collect()
}

fn clues(g: &Game, controller: usize) -> Vec<ObjId> {
    g.battlefield.iter().copied().filter(|o| g.obj(*o).chars.name == "Clue Token" && g.controller(*o) == controller).collect()
}

fn cast(g: &mut Game, player: usize, def: usize, targets: Vec<Vec<Target>>, paying: &[ManaSymbol]) -> ObjId {
    mana(g, player, paying);
    let card = g.put(player, def, Zone::Hand);
    g.act(player, crate::support::paid(&paying.iter().map(|s| s.text()).collect::<String>(), Move::Cast { card, targets })).unwrap_or_else(|e| panic!("cast: {e}"));
    card
}

#[test]
fn r701_16a_a_triggered_investigate_creates_the_clue_of_111_10f_for_its_controller() {
    let mut g = game(3);
    cast(&mut g, 0, INSPECTOR, vec![], &[ManaSymbol::Color(Color::White)]);
    resolve(&mut g);
    let [clue] = clues(&g, 0)[..] else { panic!("one Clue: {:?}", created(&g)) };
    cite!(["701.16a"]; assert_eq!(created(&g), vec![clue]));
    let o = g.obj(clue);
    cite!(["111.2"]; assert!(o.token() && o.owner == 0));
    let seen = g.seen(clue, None);
    cite!(["111.10f"]; assert_eq!(seen.types, vec![CardType::Artifact]));
    cite!(["111.10f"]; assert_eq!(seen.subtypes, vec![Subtype::from_name("Clue").unwrap()]));
    cite!(["111.10f"]; assert!(seen.colors.is_empty(), "colorless (111.10f)"));
    assert!(clues(&g, 1).is_empty());
    // its ability is the token's own: {2}, sacrifice it, draw a card
    mana(&mut g, 0, &generic(2));
    let hand = g.players[0].hand.len();
    g.act(0, crate::support::paid("CC", Move::Activate { source: clue, ability: 0, targets: vec![], paying: vec![] })).unwrap();
    cite!(["111.10f"]; assert!(clues(&g, 0).is_empty()));
    resolve(&mut g);
    cite!(["111.10f"]; assert_eq!(g.players[0].hand.len(), hand + 1));
}

#[test]
fn r701_16a_investigate_three_times_creates_a_clue_three_times() {
    let mut g = game(3);
    // a spell to counter: the caster holds priority after casting it (117.3c)
    cast(&mut g, 0, SHOCK, vec![vec![Target::Player(1)]], &[ManaSymbol::Color(Color::Red)]);
    // the spell is a new object on the stack (400.7)
    let shock = *g.stack.last().unwrap();
    let blue = ManaSymbol::Color(Color::Blue);
    cast(&mut g, 0, SUSPICIONS, vec![vec![Target::Object(shock)]], &[blue, blue, ManaSymbol::Colorless, ManaSymbol::Colorless, ManaSymbol::Colorless]);
    resolve(&mut g);
    cite!(["701.6a"]; assert_eq!(g.players[1].life, 20, "countered"));
    let made = created(&g);
    cite!(["701.16a"]; assert_eq!(made.len(), 3, "three Clues, each created by an investigate of its own"));
    cite!(["111.2"]; assert_eq!(clues(&g, 0), made));
}

#[test]
fn r701_16a_investigate_zero_times_creates_no_clue_and_the_rest_is_done() {
    // composed with the instructions around it, in order (608.2c): none of the counts is a
    // card's, so the definitions are made here; each reads and is admitted as a card is
    let defs = vec![
        card("Not At All", "{0}", "Sorcery", "Investigate zero times. You gain 1 life.", None),
        card("Twice Then", "{0}", "Sorcery", "Draw a card, then investigate twice. You gain 1 life.", None),
        card("Island", "", "Basic Land — Island", "({T}: Add {U}.)", None),
    ];
    let mut g = Game::new(defs, 2, 20);
    for _ in 0..3 { g.put(0, 2, Zone::Library); }
    let none = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: none, targets: vec![] }).unwrap();
    resolve(&mut g);
    cite!(["701.16a", "608.2c"]; assert_eq!((created(&g).len(), g.players[0].life), (0, 21)));
    let twice = g.put(0, 1, Zone::Hand);
    let hand = g.players[0].hand.len();
    g.act(0, Move::Cast { card: twice, targets: vec![] }).unwrap();
    resolve(&mut g);
    cite!(["701.16a", "608.2c"]; assert_eq!((created(&g).len(), clues(&g, 0).len(), g.players[0].life), (2, 2, 22)));
    cite!(["608.2c"]; assert_eq!(g.players[0].hand.len(), hand, "cast one, drew one"));
}

#[test]
fn r701_16a_what_investigating_created_is_what_later_text_names() {
    // as "create a Clue token. … it": the Clues, all of them, and none when none was created
    let defs = vec![
        card("Once", "{0}", "Sorcery", "Investigate. Tap it.", None),
        card("Twice", "{0}", "Sorcery", "Investigate twice. Tap them.", None),
        card("Never", "{0}", "Sorcery", "Tap target creature. Investigate zero times. Untap it.", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
    ];
    let mut g = Game::new(defs, 2, 20);
    let bears = g.put(1, 3, Zone::Battlefield);
    for (def, total) in [(0, 1), (1, 3)] {
        let card = g.put(0, def, Zone::Hand);
        g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
        resolve(&mut g);
        let made = clues(&g, 0);
        cite!(["701.16a"]; case: def; assert_eq!(made.len(), total));
        cite!(["608.2c"]; case: def; assert!(made.iter().all(|c| g.obj(*c).tapped), "each Clue it created is tapped"));
    }
    // zero times: "it" is what was created, which is nothing; not the creature tapped before
    let never = g.put(0, 2, Zone::Hand);
    g.act(0, Move::Cast { card: never, targets: vec![vec![Target::Object(bears)]] }).unwrap();
    resolve(&mut g);
    cite!(["608.2c"]; assert!(g.obj(bears).tapped, "the creature stays tapped"));
    assert_eq!(clues(&g, 0).len(), 3);
}

#[test]
fn r701_16a_the_player_the_text_names_investigates() {
    let mut g = game(3);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    cast(&mut g, 0, ABSENCE, vec![vec![Target::Object(bears)]], &[ManaSymbol::Color(Color::White), ManaSymbol::Colorless]);
    resolve(&mut g);
    assert_eq!(g.obj(bears).zone, Zone::Gone, "destroyed");
    // "Its controller investigates": the Clue is created under the control of the destroyed
    // creature's controller (111.2), not the caster's
    cite!(["111.2"]; assert_eq!(clues(&g, 1).len(), 1));
    assert!(clues(&g, 0).is_empty());
    cite!(["111.2"]; assert_eq!(g.obj(clues(&g, 1)[0]).owner, 1));
}

#[test]
fn r608_2c_investigate_under_a_condition_is_done_only_when_it_holds() {
    let (u, b) = (ManaSymbol::Color(Color::Blue), ManaSymbol::Color(Color::Black));
    // no creature died this turn: only the Detective (and surveil looks at no card)
    let mut g = game(0);
    cast(&mut g, 0, CANAL, vec![], &[u, b]);
    resolve(&mut g);
    cite!(["608.2c", "608.2i"]; assert_eq!((created(&g).len(), clues(&g, 0).len(), g.players[0].life), (1, 0, 20)));
    // a creature died this turn: the Detective, 2 life, then a Clue
    let mut g = game(0);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    cast(&mut g, 0, SHOCK, vec![vec![Target::Object(bears)]], &[ManaSymbol::Color(Color::Red)]);
    resolve(&mut g);
    assert_ne!(g.obj(bears).zone, Zone::Battlefield);
    cast(&mut g, 0, CANAL, vec![], &[u, b]);
    resolve(&mut g);
    cite!(["608.2c", "608.2i"]; assert_eq!((created(&g).len(), clues(&g, 0).len(), g.players[0].life), (2, 1, 22)));
}

#[test]
fn r603_2_landfall_investigates_and_sacrificing_that_clue_triggers_the_tracker() {
    let mut g = game(3);
    let tracker = g.put(0, TRACKER, Zone::Battlefield);
    let forest = g.put(0, FOREST, Zone::Hand);
    g.act(0, Move::PlayLand(forest)).unwrap();
    resolve(&mut g);
    let [clue] = clues(&g, 0)[..] else { panic!("landfall made one Clue") };
    // sacrificing the Clue to its own ability is sacrificing a Clue (701.21a)
    mana(&mut g, 0, &generic(2));
    let hand = g.players[0].hand.len();
    g.act(0, crate::support::paid("CC", Move::Activate { source: clue, ability: 0, targets: vec![], paying: vec![] })).unwrap();
    resolve(&mut g);
    cite!(["603.2", "701.21a"]; assert_eq!(g.obj(tracker).counters.get("+1/+1").copied(), Some(1)));
    cite!(["122.1a"]; assert_eq!(g.seen(tracker, None).pt, Some((4, 3))));
    cite!(["111.10f"]; assert_eq!(g.players[0].hand.len(), hand + 1));
}

#[test]
fn r608_2h_lowered_creation_results_survive_skipped_branches_and_paused_choices() {
    for creation in ["create a Clue token", "investigate"] {
        for enabled in [false, true] {
            let text = format!("Untap target creature. If you control an Island, {creation}. You may gain 1 life. Tap it.");
            let defs = vec![
                card("Conditional creation", "{0}", "Sorcery", &text, None),
                card("Bear", "{0}", "Creature — Bear", "", Some((2, 2))),
                card("Island", "", "Basic Land — Island", "", None),
            ];
            let mut g = Game::new(defs, 2, 20);
            let bear = g.put(0, 1, Zone::Battlefield);
            g.tap(bear);
            if enabled { g.put(0, 2, Zone::Battlefield); }
            let spell = g.put(0, 0, Zone::Hand);
            g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(bear)]] }).unwrap();
            g.act(0, Move::Pass).unwrap();
            g.act(1, Move::Pass).unwrap();
            assert!(g.decision.is_some());
            assert!(!g.obj(bear).tapped);
            for (mut branch, answer, life) in [(g.clone(), game::state::Answer::Yes, 21), (g, game::state::Answer::No, 20)] {
                branch.act(0, Move::Choose(answer)).unwrap();
                cite!(["608.2d"]; case: format!("{creation}/{enabled}/{life}"); assert_eq!(branch.players[0].life, life));
                cite!(["608.2c"]; case: format!("{creation}/{enabled}/{life}"); assert!(!branch.obj(bear).tapped, "an empty result must not fall back to the earlier target"));
                let made = clues(&branch, 0);
                cite!(["608.2c"]; case: format!("{creation}/{enabled}/{life}"); assert_eq!(made.len(), usize::from(enabled)));
                cite!(["608.2c"]; case: format!("{creation}/{enabled}/{life}"); assert!(made.iter().all(|o| branch.obj(*o).tapped)));
                assert_eq!(created(&branch).len(), usize::from(enabled), "resuming must not replay creation");
            }
        }
    }
}
