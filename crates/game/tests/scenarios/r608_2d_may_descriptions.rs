//! A "may" asked as an instruction is followed is a choice of the effect (608.2d); the
//! prompt says what answering yes does: the action the text makes optional, and for a zone
//! change where to. Each expectation is the action the card text says the player may take.
use crate::support::cite;
use game::simulation::Simulation;
use game::state::{Choice, Decision, Placement};
use game::{CardDef, Game, Move, PlayerId, Target, Zone};
use ir::{Part, Verb};

const PROBE: usize = 0;
const BEAR: usize = 1;
const FOREST: usize = 2;

/// A game in its first main phase with a {0} sorcery of this text, Bears in each library and
/// a Forest under them.
fn game(text: &str) -> Game {
    let defs = vec![CardDef::new("Probe", "{0}", "Sorcery", text, None).unwrap(),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None).unwrap()];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 {
        g.put(p, FOREST, Zone::Library);
        for _ in 0..5 { g.put(p, BEAR, Zone::Library); }
    }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}

/// The "may" the resolving sorcery asks, and what the asked player's prompt says yes does.
fn asked(mut g: Game, targets: Vec<Vec<Target>>) -> (PlayerId, Option<(Verb, Option<Placement>)>) {
    let card = g.put(0, PROBE, Zone::Hand);
    g.act(0, Move::Cast { card, targets }).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    let Some(Decision::Choose { player, choice: Choice::May }) = g.decision.clone() else { panic!("a \"may\" is asked: {:?}", g.decision) };
    let mut sim = Simulation::new(g);
    let prompt = sim.prompt(player).unwrap();
    (player, prompt.decision.expect("the asked player is shown the question").optional)
}

fn of(text: &str) -> (PlayerId, Option<(Verb, Option<Placement>)>) { asked(game(text), vec![]) }

#[test]
fn r608_2d_a_may_of_a_player_action_says_that_action() {
    for (text, verb) in [
        ("You may draw a card.", Verb::Draw),
        ("You may gain 1 life.", Verb::GainLife),
        ("You may mill two cards.", Verb::Mill),
        ("You may create a 1/1 white Soldier creature token.", Verb::Create),
        // 701.22a, 701.25a: the optional keyword action itself
        ("You may scry 2.", Verb::Scry),
        ("You may surveil 2.", Verb::Surveil),
        // 701.23a: what the governed instructions begin with
        ("You may search your library for a basic land card, reveal it, put it into your hand, then shuffle.", Verb::Search),
    ] {
        cite!(["608.2d"]; case: text; assert_eq!(of(text), (0, Some((verb, None)))));
    }
}

#[test]
fn r608_2d_a_may_of_another_player_is_asked_of_them_and_says_their_action() {
    // "each opponent may": the opponent answers, and yes is their draw
    cite!(["608.2d"]; assert_eq!(of("Each opponent may draw a card."), (1, Some((Verb::Draw, None)))));
}

#[test]
fn r608_2d_a_may_of_a_targeted_action_says_that_action() {
    for (text, said) in [
        ("You may destroy target creature.", (Verb::Destroy, None)),
        // to exile an object is to move it to the exile zone (701.13a): said as that move
        ("You may exile target creature.", (Verb::Move, Some(Placement::Zone(Zone::Exile)))),
        ("You may put a +1/+1 counter on target creature.", (Verb::PutCounters, None)),
    ] {
        let mut g = game(text);
        let bear = g.put(1, BEAR, Zone::Battlefield);
        cite!(["608.2d"]; case: text; assert_eq!(asked(g, vec![vec![Target::Object(bear)]]), (0, Some(said))));
    }
}

#[test]
fn r608_2d_a_may_of_choosing_objects_for_an_action_says_that_action() {
    // the objects are chosen once yes is answered; yes is that the chosen are sacrificed or
    // discarded
    let mut g = game("You may sacrifice a creature. If you do, you gain 3 life.");
    g.put(0, BEAR, Zone::Battlefield);
    cite!(["608.2d", "701.21a"]; assert_eq!(asked(g, vec![]), (0, Some((Verb::Sacrifice, None)))));
    let mut g = game("You may discard a card. If you do, draw a card.");
    for _ in 0..2 { g.put(0, BEAR, Zone::Hand); }
    cite!(["608.2d", "701.9a"]; assert_eq!(asked(g, vec![]), (0, Some((Verb::Discard, None)))));
}

#[test]
fn r608_2d_an_unless_says_the_action_that_avoids_the_rest() {
    // "unless you discard a creature card": yes is that discard, not the other two
    cite!(["608.2d", "701.9a"]; assert_eq!(of("Draw three cards. Then discard two cards unless you discard a creature card."), (0, Some((Verb::Discard, None)))));
    // "unless you sacrifice a creature": yes is the sacrifice, not the damage
    let mut g = game("Target creature deals 2 damage to you unless you sacrifice a creature.");
    let bear = g.put(1, BEAR, Zone::Battlefield);
    g.put(0, BEAR, Zone::Battlefield);
    cite!(["608.2d", "701.21a"]; assert_eq!(asked(g, vec![vec![Target::Object(bear)]]), (0, Some((Verb::Sacrifice, None)))));
}

#[test]
fn r608_2d_a_may_of_a_zone_change_says_where_to() {
    // "You may put that card into your graveyard": yes moves it there (400.7)
    cite!(["608.2d", "400.7"]; assert_eq!(of("Look at the top card of your library. You may put that card into your graveyard."),
        (0, Some((Verb::Move, Some(Placement::Zone(Zone::Graveyard)))))));
}

#[test]
fn r614_12a_a_may_as_a_land_enters_says_yes_pays_the_life() {
    // "As this land enters, you may pay 2 life. If you don't, it enters tapped.": the choice
    // is made before it enters (614.12a), and yes pays 2 life (119.4)
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let tomb = CardDef::from_face(faces.iter().find(|f| f.name == "Overgrown Tomb").unwrap()).unwrap();
    game::play::check(&tomb).unwrap();
    let mut g = Game::new(vec![tomb], 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, 0, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    let land = g.put(0, 0, Zone::Hand);
    g.act(0, Move::PlayLand(land)).unwrap();
    assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May })), "{:?}", g.decision);
    let mut sim = Simulation::new(g);
    let decision = sim.prompt(0).unwrap().decision.unwrap();
    cite!(["614.12a", "119.4"]; assert_eq!(decision.optional, Some((Verb::PayLife, None))));
}

/// Each player's library of five of `library`, in the first main phase, with what `on` puts in
/// play; the "may" a trigger asks once the spell `on` gives is cast and what is on the stack
/// resolves, and what the asked player's prompt says yes does.
fn triggered(defs: Vec<CardDef>, library: usize, mut on: impl FnMut(&mut Game) -> Move) -> (Game, Option<(Verb, Option<Placement>)>) {
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, library, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    let cast = on(&mut g);
    g.act(0, cast).unwrap();
    while g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
    assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May })), "{:?}", g.decision);
    let optional = Simulation::new(g.clone()).prompt(0).unwrap().decision.expect("the controller is shown the question").optional;
    (g, optional)
}

#[test]
fn r608_2d_a_may_to_attach_equipment_says_attach() {
    // Doc Ock's Tentacles: "Whenever a creature you control with mana value 5 or greater
    // enters, you may attach this Equipment to it.": yes attaches it to that creature (701.3a)
    let giant = CardDef::new("Giant", "{5}", "Creature — Giant", "", Some((5, 5))).unwrap();
    let mut tentacles = None;
    let mut entering = None;
    let (mut g, optional) = triggered(vec![crate::support::oracle("Doc Ock's Tentacles"), giant], 1, |g| {
        tentacles = Some(g.put(0, 0, Zone::Battlefield));
        let card = g.put(0, 1, Zone::Hand);
        entering = Some(card);
        g.players[0].pool.colorless = 5;
        crate::support::paid("CCCCC", Move::Cast { card, targets: vec![] })
    });
    cite!(["608.2d", "701.3a"]; assert_eq!(optional, Some((Verb::Attach, None))));
    g.act(0, Move::Choose(game::state::Answer::Yes)).unwrap();
    let giant = *g.battlefield.iter().find(|o| g.obj(**o).card == g.obj(entering.unwrap()).card).unwrap();
    cite!(["701.3a"]; assert_eq!(g.obj(tentacles.unwrap()).attached_to, Some(Target::Object(giant))));
}

#[test]
fn r608_2d_a_may_of_an_action_on_what_triggered_it_says_that_action() {
    // "you may transform it": what "it" is is found before the transform it governs; yes is
    // that transform (701.27a)
    let watch = CardDef::new("Watch", "{0}", "Enchantment", "Whenever a creature you control enters, you may transform it.", None).unwrap();
    game::play::check(&watch).unwrap();
    let giant = CardDef::new("Giant", "{0}", "Creature — Giant", "", Some((5, 5))).unwrap();
    let (_, optional) = triggered(vec![watch, giant], 1, |g| {
        g.put(0, 0, Zone::Battlefield);
        Move::Cast { card: g.put(0, 1, Zone::Hand), targets: vec![] }
    });
    cite!(["608.2d", "701.27a"]; assert_eq!(optional, Some((Verb::Transform, None))));
}

#[test]
fn r608_2d_a_may_of_the_top_of_a_library_says_exile_or_reveal() {
    // to exile a card is to move it to exile (701.13a), said as a targeted exile is; to reveal
    // one is to show it to all players (701.20a)
    for (text, said) in [
        ("You may exile the top card of your library.", (Verb::Move, Some(Placement::Zone(Zone::Exile)))),
        ("You may exile the top two cards of your library. If you do, you gain 2 life.", (Verb::Move, Some(Placement::Zone(Zone::Exile)))),
        ("You may reveal the top card of your library.", (Verb::Reveal, None)),
    ] {
        cite!(["608.2d"]; case: text; assert_eq!(of(text), (0, Some(said))));
    }
    // Harnesser of Storms: "Whenever you cast a noncreature or Otter spell, you may exile the
    // top card of your library. Until end of turn, you may play that card.": yes exiles it
    let spell = CardDef::new("Spell", "{0}", "Sorcery", "You gain 1 life.", None).unwrap();
    let mut top = None;
    let (mut g, optional) = triggered(vec![crate::support::oracle("Harnesser of Storms"), spell], 1, |g| {
        g.put(0, 0, Zone::Battlefield);
        top = g.top_cards(0, 1).first().copied();
        Move::Cast { card: g.put(0, 1, Zone::Hand), targets: vec![] }
    });
    cite!(["608.2d", "701.13a"]; assert_eq!(optional, Some((Verb::Move, Some(Placement::Zone(Zone::Exile))))));
    g.act(0, Move::Choose(game::state::Answer::Yes)).unwrap();
    // the exiled card is a new object (400.7): the top card is now that one in exile
    let card = g.obj(top.unwrap()).card;
    cite!(["701.13a", "400.7"]; assert!(g.exile.iter().any(|o| g.obj(*o).card == card), "{:?}", g.exile));
}
