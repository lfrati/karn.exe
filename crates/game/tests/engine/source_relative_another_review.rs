//! "you control another Knight" stays relative to
//! the source when no target-relative proof exists, and explicit-source "each other" programs
//! keep their behavior. Run with `engine::source_relative_another_review::`.
use game::state::Decision;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};

fn pinned(name: &str) -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    CardDef::from_faces(CardDef::catalog(&faces).get(name).unwrap()).unwrap()
}
fn bear() -> CardDef { CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap() }
fn squire() -> CardDef { CardDef::new("Squire", "{0}", "Creature — Human Knight", "", Some((1, 1))).unwrap() }

/// Cast the Knight (card 0), then target `pick` for its enters trigger. Returns the target's
/// stun counters.
fn knight_enters(knight: CardDef, place: impl FnOnce(&mut Game) -> ObjId, spending: &str) -> u32 {
    let mut g = Game::new(vec![knight, bear(), squire()], 2, 20);
    let t = place(&mut g);
    g.players[0].pool.colors = [10; 5];
    let c = g.put(0, 0, Zone::Hand);
    g.act(0, crate::support::paid(spending, Move::Cast { card: c, targets: vec![] })).unwrap();
    for _ in 0..6 {
        if let Some(Decision::Targets { .. }) = g.decision { g.act(0, Move::ChooseTargets(vec![vec![Target::Object(t)]])).unwrap(); continue; }
        if g.stack.is_empty() { break; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    assert!(g.obj(t).tapped);
    g.obj(t).counters.get("stun").copied().unwrap_or(0)
}

/// "Another Knight" is other than Protocol Knight: alone, it
/// doesn't stun. The opponent's target can't be a Knight you control.
#[test]
fn protocol_knight_counts_knights_other_than_itself() {
    // Uncited (card text): Protocol Knight's "if you control another Knight": a Knight other than this one
    assert_eq!(knight_enters(pinned("Protocol Knight"), |g| g.put(1, 1, Zone::Battlefield), "WWWU"), 0, "alone");
    // Uncited (card text): Protocol Knight's "if you control another Knight": a Knight other than this one
    assert_eq!(knight_enters(pinned("Protocol Knight"), |g| { g.put(0, 2, Zone::Battlefield); g.put(1, 1, Zone::Battlefield) }, "WWWU"), 1, "with a Squire");
}

/// Any-controller target: still source-relative. Targeting my own Bear with no other Knight
/// must not stun (target-relative reading would count the entering Knight); targeting my own
/// Squire stuns, because the Squire is a Knight other than the source.
#[test]
fn any_controller_target_keeps_source_relative_another() {
    let knight = || CardDef::new("Probe Knight", "{0}", "Creature — Human Knight",
        "When this creature enters, tap target creature. Put a stun counter on that creature if you control another Knight.", Some((2, 2))).unwrap();
    game::play::check(&knight()).unwrap();
    // Uncited (card text): "Put a stun counter on that creature if you control another Knight": a Knight other than this one
    assert_eq!(knight_enters(knight(), |g| g.put(0, 1, Zone::Battlefield), ""), 0, "own Bear, no other Knight");
    // Uncited (card text): "Put a stun counter on that creature if you control another Knight": a Knight other than this one
    assert_eq!(knight_enters(knight(), |g| g.put(0, 2, Zone::Battlefield), ""), 1, "own Squire");
}

fn damage(g: &Game) -> Vec<(ObjId, Target, u32)> {
    g.log.iter().filter_map(|e| match e {
        Event::DamageDealt { source, to, amount, .. } => Some((*source, *to, *amount)),
        _ => None,
    }).collect()
}

/// The explicit `other_than: Source` compiled for Fire Ants and Harbinger of the Hunt (also
/// Cinder Giant, Scourge of Kher Ridges) excludes the source, as an unqualified "other" does.
#[test]
fn explicit_source_other_is_unchanged() {
    let flier = CardDef::new("Bird", "{0}", "Creature — Bird", "Flying", Some((1, 3))).unwrap();
    let mut g = Game::new(vec![pinned("Fire Ants"), bear(), flier.clone()], 2, 20);
    let ants = g.put(0, 0, Zone::Battlefield);
    let b = g.put(1, 1, Zone::Battlefield);
    g.put(1, 2, Zone::Battlefield);
    g.act(0, Move::Activate { source: ants, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    for _ in 0..3 { if g.stack.is_empty() { break; } g.act(g.priority, Move::Pass).unwrap(); }
    // Uncited (card text): Fire Ants' "This creature deals 1 damage to each other creature without flying"
    assert_eq!(damage(&g), vec![(ants, Target::Object(b), 1)]);

    let mut g = Game::new(vec![pinned("Harbinger of the Hunt"), bear(), flier], 2, 20);
    let h = g.put(0, 0, Zone::Battlefield);
    g.put(1, 1, Zone::Battlefield);
    let f = g.put(1, 2, Zone::Battlefield);
    g.players[0].pool.colors = [10; 5];
    g.act(0, crate::support::paid("WWG", Move::Activate { source: h, ability: 2, targets: vec![], paying: vec![] })).unwrap();
    for _ in 0..3 { if g.stack.is_empty() { break; } g.act(g.priority, Move::Pass).unwrap(); }
    // Uncited (card text): Harbinger of the Hunt's "This creature deals 1 damage to each other creature with flying"
    assert_eq!(damage(&g), vec![(h, Target::Object(f), 1)]);
}

/// Activate a 1/1 with this text, targeting an opponent's Bear, with `theirs` more creatures
/// for the opponent and `mine` more for me. Returns the life I gained.
fn gained(text: &str, theirs: usize, mine: usize) -> i32 {
    let d = CardDef::new("Probe", "{0}", "Creature — Insect", text, Some((1, 1))).unwrap();
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d, bear()], 2, 20);
    let p = g.put(0, 0, Zone::Battlefield);
    let t = g.put(1, 1, Zone::Battlefield);
    for _ in 0..theirs { g.put(1, 1, Zone::Battlefield); }
    for _ in 0..mine { g.put(0, 1, Zone::Battlefield); }
    g.act(0, Move::Activate { source: p, ability: 0, targets: vec![vec![Target::Object(t)]], paying: vec![] }).unwrap();
    for _ in 0..3 { if g.stack.is_empty() { break; } g.act(g.priority, Move::Pass).unwrap(); }
    g.players[0].life - 20
}

/// "you control no other creatures"
/// is other than the source, whatever creature was targeted.
#[test]
fn you_control_no_other_is_source_relative() {
    for text in ["{T}: This creature deals 1 damage to target creature an opponent controls. You gain 5 life if you control no other creatures.",
                 "{T}: Tap target creature an opponent controls. You gain 5 life if you control no other creatures.",
                 "{T}: Tap target creature. You gain 5 life if you control no other creatures."] {
        // Uncited (card text): "You gain 5 life if you control no other creatures": creatures other than this one
        assert_eq!([gained(text, 0, 0), gained(text, 1, 0), gained(text, 0, 1)], [5, 5, 0], "{text}");
    }
}

/// A captured target controller's
/// "no other creatures" excludes the target and counts that player's others.
#[test]
fn that_player_controls_no_other_is_target_relative() {
    for text in ["{T}: This creature deals 1 damage to target creature an opponent controls. Then if that player controls no other creatures, you gain 5 life.",
                 "{T}: This creature deals 1 damage to target creature an opponent controls. If that creature's controller controls no other creatures, you gain 5 life.",
                 "{T}: Target creature an opponent controls gets -1/-0 until end of turn. You gain 5 life if that opponent controls no other creatures."] {
        // Uncited (card text): "if that player/that creature's controller/that opponent controls no other creatures": that player's creatures other than the target
        assert_eq!([gained(text, 0, 0), gained(text, 1, 0), gained(text, 0, 1)], [5, 0, 5], "{text}");
    }
}
