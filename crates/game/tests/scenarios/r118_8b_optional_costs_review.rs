//! Optional additional costs (118.8b, 601.2b): what was announced as the spell was
//! cast is of that spell and the permanent it became, not of a new object that permanent
//! becomes after a zone change (400.7); teamwork taps untapped creatures, so a creature tapped
//! for mana for the same spell can't also pay it (601.2g-h, 702.194a); and what a spell does
//! not have can't be announced paid (601.2b).
use crate::support::{cite, paid};
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const INSPECTOR: usize = 0;
const RAMPAGER: usize = 1;
const BLINK: usize = 2;
const FOUR: usize = 3;
const THREE: usize = 4;
const ISLAND: usize = 5;
const ELF: usize = 6;
const TEAM: usize = 7;
const BEAR: usize = 8;
const INSCRIPTION: usize = 9;
const EXHALE: usize = 10;
const DRAKE: usize = 11;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let real = |n: &str| CardDef::from_faces(catalog.get(n).unwrap()).unwrap();
    let c = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    let defs = vec![
        real("Vitu-Ghazi Inspector"),
        real("Rust-Shield Rampager"),
        c("Blink", "{0}", "Instant", "Exile target creature you control, then return that card to the battlefield under its owner's control.", None),
        c("Four", "{4}", "Sorcery", "Draw a card.", None),
        c("Three", "{2}{U}", "Sorcery", "Draw a card.", None),
        c("Island", "", "Basic Land — Island", "", None),
        c("Mana Elf", "{0}", "Creature — Elf", "{T}: Add {G}.", Some((2, 2))),
        c("Team Growth", "{G}", "Instant", "Teamwork 2\nChoose one. If this spell was cast using teamwork, choose both instead.\n• You gain 2 life.\n• Draw a card.", None),
        c("Bear", "{0}", "Creature — Bear", "", Some((2, 2))),
        c("Inscription", "{0}", "Instant", "Kicker {1}\nChoose one. If this spell was kicked, choose both instead.\n• You gain 2 life.\n• Draw a card.", None),
        real("Piercing Exhale"),
        c("Drake", "{0}", "Creature — Dragon", "", Some((1, 1))),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..8 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}

/// Pass until the stack is empty, answering a target with `target`.
fn resolve(g: &mut Game, target: ObjId) {
    for _ in 0..40 {
        match &g.decision {
            Some(Decision::Targets { player, .. }) => { let p = *player; g.act(p, Move::ChooseTargets(vec![vec![Target::Object(target)]])).unwrap(); }
            Some(d) => panic!("unexpected {d:?}"),
            None if g.stack.is_empty() => return,
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("the stack did not empty");
}

fn find(g: &Game, def: usize) -> ObjId { *g.battlefield.iter().find(|o| g.obj(**o).card == Some(def) && !g.obj(**o).token()).unwrap() }
fn tokens(g: &Game) -> usize { g.battlefield.iter().filter(|o| g.obj(**o).token()).count() }



/// Vitu-Ghazi Inspector cast with evidence collected: its enters trigger reads it; flickered,
/// the permanent is a new object that was never cast, and evidence was not collected for it.
#[test]
fn r400_7_a_flickered_permanent_was_not_cast_with_its_optional_cost_paid() {
    let mut g = game();
    let card = g.put(0, INSPECTOR, Zone::Hand);
    let evidence = vec![g.put(0, FOUR, Zone::Graveyard), g.put(0, THREE, Zone::Graveyard)];
    let bear = g.put(0, BEAR, Zone::Battlefield);
    g.players[0].pool.colorless = 3;
    g.players[0].pool.colors = [0, 0, 0, 0, 1];
    g.act(0, paid("CG", Move::WithOptional(true, Box::new(Move::WithCosts(vec![evidence], Box::new(Move::Cast { card, targets: vec![] })))))).unwrap();
    resolve(&mut g, bear);
    cite!(["701.59c", "607.2"]; assert_eq!((g.players[0].life, g.counters(Target::Object(bear)).get("+1/+1").copied()), (22, Some(1))));
    let inspector = find(&g, INSPECTOR);
    let blink = g.put(0, BLINK, Zone::Hand);
    g.act(0, Move::Cast { card: blink, targets: vec![vec![Target::Object(inspector)]] }).unwrap();
    resolve(&mut g, bear);
    let again = find(&g, INSPECTOR);
    assert_ne!(again, inspector);
    cite!(["400.7", "603.4"]; assert_eq!((g.players[0].life, g.counters(Target::Object(bear)).get("+1/+1").copied()), (22, Some(1))));
}

/// Offspring paid: one 1/1 token copy. Flickering the creature or the token makes no other.
#[test]
fn r702_175a_offspring_is_of_the_cast_permanent_only() {
    let mut g = game();
    let card = g.put(0, RAMPAGER, Zone::Hand);
    g.players[0].pool.colorless = 5;
    g.players[0].pool.colors = [0, 0, 0, 0, 2];
    g.act(0, paid("CCCCCG", Move::WithOptional(true, Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    resolve(&mut g, card);
    cite!(["702.175a"]; assert_eq!(tokens(&g), 1));
    let token = *g.battlefield.iter().find(|o| g.obj(**o).token()).unwrap();
    cite!(["707.9b"]; assert_eq!((g.power(token), g.toughness(token)), (Some(1), Some(1))));
    let rampager = find(&g, RAMPAGER);
    let blink = g.put(0, BLINK, Zone::Hand);
    g.act(0, Move::Cast { card: blink, targets: vec![vec![Target::Object(rampager)]] }).unwrap();
    resolve(&mut g, card);
    cite!(["400.7", "702.175a"]; assert_eq!(tokens(&g), 1));
}

/// Teamwork taps untapped creatures (702.194a). A creature that taps for the spell's mana is
/// tapped before the costs are paid (601.2g-h): it can't also be tapped for teamwork, and a
/// creature already tapped can't be either.
#[test]
fn r702_194a_teamwork_needs_untapped_creatures_the_mana_did_not_tap() {
    let mut g = game();
    let elf = g.put(0, ELF, Zone::Battlefield);
    let card = g.put(0, TEAM, Zone::Hand);
    let step = game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source: elf, ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None };
    let teamwork = Move::WithOptional(true, Box::new(Move::WithCosts(vec![vec![elf]], Box::new(Move::Modes(vec![0, 1], Box::new(Move::Cast { card, targets: vec![] }))))));
    cite!(["601.2g", "702.194a"]; refused("Mana Elf is not what that cost asks for"): act_unchanged(&mut g, paid("G", Move::WithMana(vec![step], Box::new(teamwork.clone())))));
    // the mana from elsewhere: now the elf can pay it
    g.players[0].pool.colors = [0, 0, 0, 0, 1];
    cite!(["702.194a"]; ok: g.clone().act(0, paid("G", teamwork.clone())));
    g.set_tapped(elf, true);
    let teamwork = Move::WithOptional(true, Box::new(Move::WithCosts(vec![vec![elf]], Box::new(Move::Modes(vec![0, 1], Box::new(Move::Cast { card, targets: vec![] }))))));
    cite!(["702.194a"]; refused("Mana Elf is not what that cost asks for"): act_unchanged(&mut g, paid("G", teamwork)));
}

/// Only an optional cost the spell has is announced (601.2b): a kicker-unlocked spell has no
/// optional additional cost, and announcing one twice is refused.
#[test]
fn r601_2b_an_optional_cost_the_spell_does_not_have_is_refused() {
    let mut g = game();
    let card = g.put(0, INSCRIPTION, Zone::Hand);
    g.players[0].pool.colorless = 1;
    for paid in [true, false] {
        let mv = Move::WithOptional(paid, Box::new(Move::Modes(vec![0], Box::new(Move::Cast { card, targets: vec![] }))));
        cite!(["601.2b"]; case: paid; refused("that spell has no optional additional cost"): act_unchanged(&mut g, mv));
    }
    let team = g.put(0, TEAM, Zone::Hand);
    g.players[0].pool.colors = [0, 0, 0, 0, 1];
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let twice = Move::WithOptional(true, Box::new(Move::WithOptional(true, Box::new(Move::WithCosts(vec![vec![bear]], Box::new(Move::Modes(vec![0, 1], Box::new(Move::Cast { card: team, targets: vec![] }))))))));
    cite!(["601.2b"]; refused("announce an optional additional cost only once"): act_unchanged(&mut g, paid("G", twice)));
    // A spell without a kicker cost cannot be announced kicked.
    let kicked = Move::WithKicks(1, Box::new(Move::Modes(vec![0], Box::new(Move::Cast { card: team, targets: vec![] }))));
    cite!(["702.33a"]; refused("that spell has no kicker cost to pay"): act_unchanged(&mut g, paid("G", kicked)));
    assert_eq!(g.players[0].life, 20);
}

/// Behold a Dragon: "reveal a Dragon card from your hand or choose a Dragon permanent you
/// control" (701.4a). A card beheld from the hand is revealed: shown to every player (701.20a).
#[test]
fn r701_4a_a_card_beheld_from_the_hand_is_revealed() {
    let mut g = game();
    let card = g.put(0, EXHALE, Zone::Hand);
    let dragon = g.put(0, DRAKE, Zone::Hand);
    let mine = g.put(0, BEAR, Zone::Battlefield);
    let theirs = g.put(1, BEAR, Zone::Battlefield);
    g.players[0].pool.colorless = 1;
    g.players[0].pool.colors = [0, 0, 0, 0, 1];
    let from = g.log.end();
    g.act(0, paid("CG", Move::WithOptional(true, Box::new(Move::WithCosts(vec![vec![dragon]],
        Box::new(Move::Cast { card, targets: vec![vec![Target::Object(mine)], vec![Target::Object(theirs)]] })))))).unwrap();
    assert_eq!(g.obj(dragon).zone, Zone::Hand, "nothing happens to it");
    let revealed = g.log.since(from).iter().any(|e| matches!(e, game::Event::Revealed { player: 0, cards } if cards.contains(&dragon)));
    cite!(["701.4a", "701.20a"]; assert!(revealed, "the beheld Dragon card is revealed"));
}

/// A rejected proposal preserves the whole game; the caller asserts its specific reason.
#[track_caller]
fn act_unchanged(g: &mut Game, action: Move) -> Result<(), String> {
    let before = format!("{g:?}");
    let result = g.act(0, action);
    assert_eq!(format!("{g:?}"), before);
    result
}
