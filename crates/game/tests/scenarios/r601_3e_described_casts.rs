//! "You may cast a spell with mana value 3 or less from your hand without paying its mana
//! cost", "you may cast an instant or sorcery spell from among them": one card of that hand,
//! or of the group the text named, cast while the effect is applied or not at all (608.2g).
//! What the spell is described as is asked of it as it would be cast: of the half of a split
//! card (709.3a), of an Adventure (715.3a), with X 0 for a spell cast without paying its mana
//! cost (107.3b), all before it is cast (601.3e).
use crate::support::{cite, mana_pool, oracle_read, paid};
use game::card::CastAs;
use game::legal::Asked;
use game::state::Answer;
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const SRAM: usize = 0;
const ELECTRO: usize = 1;
const MARVEL: usize = 2;
const SHAMAN: usize = 3;
const BEARS: usize = 4;
const ANGEL: usize = 5;
const FIRE_ICE: usize = 6;
const GIANT: usize = 7;
const BOLT: usize = 8;
const BLAZE: usize = 9;
const MOUNTAIN: usize = 10;
const BECK_CALL: usize = 11;

fn game() -> Game {
    let defs = vec![oracle_read("Sram's Expertise"), oracle_read("Electrodominance"), oracle_read("Aetherworks Marvel"),
        oracle_read("Mindclaw Shaman"), oracle_read("Grizzly Bears"), oracle_read("Serra Angel"), oracle_read("Fire // Ice"),
        oracle_read("Bonecrusher Giant // Stomp"), oracle_read("Lightning Bolt"),
        CardDef::new("Blaze", "{X}{R}", "Sorcery", "Blaze deals X damage to any target.", None).unwrap(),
        oracle_read("Mountain"), oracle_read("Beck // Call")];
    // Uncited (admission): the pinned cards these tests play run.
    for def in &defs { game::play::check(def).unwrap_or_else(|why| panic!("{}: {}", def.name(), why.0)); }
    Game::new(defs, 2, 20)
}

fn pass_all(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }

fn offered(g: &mut Game, player: usize) -> Vec<ObjId> {
    match g.asked() { Some((p, Asked::Cast { cards })) if p == player => cards, other => panic!("not an offer to cast: {other:?}") }
}

fn names(g: &Game, cards: &[ObjId]) -> Vec<String> {
    let mut names: Vec<String> = cards.iter().map(|c| g.obj(*c).chars.name.clone()).collect();
    names.sort();
    names
}

/// Sram's Expertise resolving, with these cards in its caster's hand.
fn expertise(hand: &[usize]) -> (Game, Vec<ObjId>) {
    let mut g = game();
    let cards = hand.iter().map(|card| g.put(0, *card, Zone::Hand)).collect();
    let sram = g.put(0, SRAM, Zone::Hand);
    g.players[0].pool = mana_pool("WWCC");
    g.act(0, paid("WWCC", Move::Cast { card: sram, targets: vec![] })).unwrap();
    pass_all(&mut g);
    (g, cards)
}

#[test]
fn r608_2g_a_spell_of_the_mana_value_described_is_cast_from_the_hand_for_nothing() {
    let (mut g, cards) = expertise(&[BEARS, ANGEL]);
    let [bears, angel] = cards[..] else { unreachable!() };
    cite!(["111.1"]; assert_eq!(g.battlefield.iter().filter(|o| g.obj(**o).chars.name == "Servo Token").count(), 3));
    cite!(["601.3e"]; assert_eq!(offered(&mut g, 0), vec![bears]));
    cite!(["601.3"]; refused("the effect offers other cards to cast (608.2g)"): g.act(0, Move::Cast { card: angel, targets: vec![] }));
    cite!(["608.2g", "118.9"]; ok: g.act(0, Move::Cast { card: bears, targets: vec![] }));
    cite!(["608.2g"]; assert_eq!(g.obj(g.stack[0]).chars.name, "Grizzly Bears"));
    cite!(["608.2g"]; assert_eq!(g.stack.len(), 1));
    pass_all(&mut g);
    cite!(["608.3"]; assert!(g.battlefield.iter().any(|o| g.obj(*o).chars.name == "Grizzly Bears")));
    cite!(["601.3e"]; assert!(g.players[0].hand.contains(&angel)));
}

#[test]
fn r709_3a_of_a_split_card_only_the_half_cast_is_asked_about() {
    // Fire // Ice has mana value 4 in a hand (709.4); each half is a spell of mana value 2
    let (mut g, cards) = expertise(&[FIRE_ICE]);
    let fire_ice = cards[0];
    cite!(["709.3a", "601.3e"]; assert_eq!(offered(&mut g, 0), vec![fire_ice]));
    let servo = g.battlefield[0];
    cite!(["709.3a"]; ok: g.act(0, Move::CastAs(CastAs::Half(1), Box::new(Move::Cast { card: fire_ice, targets: vec![vec![Target::Object(servo)]] }))));
    cite!(["709.3b"]; assert_eq!(g.obj(g.stack[0]).chars.name, "Ice"));
}

#[test]
fn r709_3a_a_half_or_a_fused_spell_not_described_is_not_cast_from_the_hand_under_the_offer() {
    // Beck has mana value 2, Call 6, and the fused spell 8 (709.4d)
    let (mut g, cards) = expertise(&[BECK_CALL]);
    let beck_call = cards[0];
    cite!(["709.3a", "601.3e"]; assert_eq!(offered(&mut g, 0), vec![beck_call]));
    let not_described = "cast that way, it is not a spell the effect describes (601.3e, 709.3a, 712.11c, 715.3a)";
    cite!(["709.3a", "601.3e"]; refused(not_described): g.act(0, Move::CastAs(CastAs::Half(1), Box::new(Move::Cast { card: beck_call, targets: vec![] }))));
    cite!(["709.4d", "601.3e"]; refused(not_described): g.act(0, Move::CastAs(CastAs::Fused, Box::new(Move::Cast { card: beck_call, targets: vec![] }))));
    cite!(["709.3a"]; ok: g.act(0, Move::CastAs(CastAs::Half(0), Box::new(Move::Cast { card: beck_call, targets: vec![] }))));
    cite!(["709.3b"]; assert_eq!(g.obj(g.stack[0]).chars.name, "Beck"));
}

#[test]
fn r107_3b_the_x_of_a_spell_cast_for_nothing_is_0_as_its_mana_value_is_asked() {
    let mut g = game();
    let blaze = g.put(0, BLAZE, Zone::Hand);
    let bolt = g.put(0, BOLT, Zone::Hand);
    g.put(0, BEARS, Zone::Hand);
    let electro = g.put(0, ELECTRO, Zone::Hand);
    g.players[0].pool = mana_pool("RRC");
    g.act(0, paid("RRC", Move::WithX(1, Box::new(Move::Cast { card: electro, targets: vec![vec![Target::Player(1)]] })))).unwrap();
    pass_all(&mut g);
    cite!(["120.3"]; assert_eq!(g.players[1].life, 19));
    // "mana value X or less" is Electrodominance's X, 1; Blaze's own X is 0 (107.3b)
    let mut cards = offered(&mut g, 0);
    cards.sort();
    let mut expected = vec![blaze, bolt];
    expected.sort();
    cite!(["107.3b", "601.3e"]; assert_eq!(cards, expected));
    cite!(["107.3b"]; refused("X is 0 for a spell cast without paying its mana cost (107.3b)"): g.act(0, Move::WithX(1, Box::new(Move::Cast { card: blaze, targets: vec![vec![Target::Player(1)]] }))));
}

/// Mindclaw Shaman's trigger resolving: player 1 reveals this hand.
fn shaman(theirs: &[usize]) -> (Game, Vec<ObjId>) {
    let mut g = game();
    let cards = theirs.iter().map(|card| g.put(1, *card, Zone::Hand)).collect();
    let shaman = g.put(0, SHAMAN, Zone::Hand);
    g.players[0].pool = mana_pool("RCCCC");
    g.act(0, paid("RCCCC", Move::Cast { card: shaman, targets: vec![] })).unwrap();
    pass_all(&mut g);
    g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap();
    pass_all(&mut g);
    (g, cards)
}

#[test]
fn r715_3a_of_an_adventurer_card_its_adventure_may_be_the_spell_described() {
    let (mut g, cards) = shaman(&[GIANT, ANGEL, BOLT, MOUNTAIN]);
    let [giant, _, bolt, _] = cards[..] else { unreachable!() };
    let cards = offered(&mut g, 0);
    cite!(["701.20a", "715.3a", "601.3e"]; assert_eq!(names(&g, &cards), names(&g, &[giant, bolt])));
    cite!(["715.3a"]; refused("cast that way, it is not a spell the effect describes (601.3e, 709.3a, 712.11c, 715.3a)"):
        g.act(0, Move::CastAs(CastAs::Whole, Box::new(Move::Cast { card: giant, targets: vec![] }))));
    cite!(["715.3a", "608.2g"]; ok: g.act(0, Move::CastAs(CastAs::Adventure, Box::new(Move::Cast { card: giant, targets: vec![vec![Target::Player(1)]] }))));
    let stomp = g.stack[0];
    cite!(["715.3b"]; assert_eq!(g.obj(stomp).chars.name, "Stomp"));
    cite!(["108.4"]; assert_eq!(g.controller(stomp), 0));
    pass_all(&mut g);
    cite!(["120.3"]; assert_eq!(g.players[1].life, 18));
    cite!(["715.3d", "108.3"]; assert!(g.exile.iter().any(|o| g.obj(*o).owner == 1 && g.obj(*o).chars.name.starts_with("Bonecrusher Giant"))));
}

/// Aetherworks Marvel's second ability resolving, the top six cards of the library these.
fn marvel(library: &[usize]) -> (Game, Vec<ObjId>) {
    let mut g = game();
    let cards = library.iter().map(|card| g.put(0, *card, Zone::Library)).collect();
    let marvel = g.put(0, MARVEL, Zone::Battlefield);
    g.put_counters(Target::Player(0), "energy", 6);
    g.act(0, Move::Activate { source: marvel, ability: 1, targets: vec![], paying: vec![] }).unwrap();
    pass_all(&mut g);
    (g, cards)
}

#[test]
fn r608_2g_a_spell_is_cast_from_among_the_cards_looked_at_and_the_rest_go_to_the_bottom() {
    let (mut g, cards) = marvel(&[MOUNTAIN, BEARS, ANGEL, BOLT, MOUNTAIN, FIRE_ICE]);
    cite!(["107.14"]; assert_eq!(g.counters(Target::Player(0)).get("energy").copied().unwrap_or(0), 0));
    // a land is not cast (305.9)
    let castable = offered(&mut g, 0);
    cite!(["305.9", "601.3e"]; assert_eq!(names(&g, &castable), names(&g, &[cards[1], cards[2], cards[3], cards[5]])));
    cite!(["608.2g"]; ok: g.act(0, Move::Cast { card: cards[2], targets: vec![] }));
    cite!(["608.2g"]; assert_eq!(g.obj(g.stack[0]).chars.name, "Serra Angel"));
    cite!(["401.4"]; assert_eq!(g.players[0].library.len(), 5));
    pass_all(&mut g);
    cite!(["608.3"]; assert!(g.battlefield.iter().any(|o| g.obj(*o).chars.name == "Serra Angel")));
}

#[test]
fn r608_2g_declined_the_cards_looked_at_all_go_to_the_bottom() {
    let (mut g, _) = marvel(&[BEARS, ANGEL]);
    cite!(["608.2g"]; ok: g.act(0, Move::Choose(Answer::No)));
    cite!(["401.4"]; assert_eq!(g.players[0].library.len(), 2));
    cite!(["608.2g"]; assert!(g.stack.is_empty() && g.decision.is_none()));
}
