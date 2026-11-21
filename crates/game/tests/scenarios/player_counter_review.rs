//! Player-counter admission boundaries, plural
//! consent, per-actor amounts, typed player replacements and player-counter reads.
use crate::support::cite;
use game::state::{Answer, Decision};
use game::{CardDef, Event, Game, Move, Target, Zone};

fn def(name: &str, ty: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef { CardDef::new(name, "{0}", ty, text, pt).unwrap() }
fn admitted(d: &CardDef) -> Result<(), String> { game::play::check(d).map_err(|e| e.0) }
fn bear() -> CardDef { def("Bear", "Creature — Bear", "", Some((2, 2))) }
/// Resolve the top of the stack, answering any "may" with `yes` for the players in `accept`.
fn resolve(g: &mut Game, accept: &[usize]) {
    for _ in 0..40 {
        match g.decision.clone() {
            Some(Decision::Choose { player, .. }) => {
                println!("  asked player {player}: {:?}", g.decision);
                let a = if accept.contains(&player) { Answer::Yes } else { Answer::No };
                g.act(player, Move::Choose(a)).unwrap();
            }
            Some(other) => panic!("unexpected decision {other:?}"),
            None if g.stack.is_empty() => return,
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("did not resolve");
}
fn cast(g: &mut Game, card: usize, targets: Vec<Vec<Target>>, accept: &[usize]) {
    let c = g.put(0, card, Zone::Hand);
    g.players[0].pool.colors = [10; 5];
    g.act(0, Move::Cast { card: c, targets }).unwrap();
    resolve(g, accept);
}
fn n(g: &Game, p: usize, kind: &str) -> u32 { g.counters(Target::Player(p)).get(kind).copied().unwrap_or(0) }

/// The generic placement verb reaches the same player holder as "gets", so it must keep the
/// same rad boundary (122.1i's upkeep trigger is not implemented): neither text is
/// admitted, so "Put a rad counter on target player." places no rad counter.
#[test]
fn put_rad_counter_on_a_player_is_refused_like_get() {
    for text in ["Put a rad counter on target player.", "Put two rad counters on each opponent."] {
        let d = def("Probe", "Sorcery", text, None);
        crate::support::refused(admitted(&d), "player counter: rad upkeep trigger (122.1i) is not implemented");
    }
}

/// 101.4 / 608.2d: each player's "may" is their own; only those who accept get counters.
#[test]
fn each_player_may_get_counters_is_per_player_consent() {
    let d = def("Probe", "Sorcery", "Each player may get two energy counters.", None);
    println!("admitted={:?}", admitted(&d));
    admitted(&d).expect("reviewed capability must remain admitted");
    let mut g = Game::new(vec![d], 3, 20);
    cast(&mut g, 0, vec![], &[0, 2]);
    cite!(["608.2d", "101.4"]; assert_eq!((n(&g, 0, "energy"), n(&g, 1, "energy"), n(&g, 2, "energy")), (2, 0, 2)));
}

/// "They" is each actor in turn; "you" stays the spell's controller (109.5).
#[test]
fn per_actor_amounts_versus_controller_amounts() {
    for (text, expected) in [
        ("Each player gets an energy counter for each creature they control.", [2, 1, 0]),
        ("Each opponent gets a poison counter for each creature you control.", [0, 2, 2]),
    ] {
        let d = def("Probe", "Sorcery", text, None);
        println!("{text}: admitted={:?}", admitted(&d));
        admitted(&d).expect("per-actor counter amount must remain admitted");
        let mut g = Game::new(vec![d, bear()], 3, 20);
        for (p, k) in [(0, 2), (1, 1)] { for _ in 0..k { g.put(p, 1, Zone::Battlefield); } }
        for _ in 0..3 { g.put(1, 1, Zone::Hand); }
        g.put(2, 1, Zone::Hand);
        let kind = if text.contains("poison") { "poison" } else { "energy" };
        cast(&mut g, 0, vec![], &[]);
        let got = [n(&g, 0, kind), n(&g, 1, kind), n(&g, 2, kind)];
        println!("  got {got:?}");
        cite!(["109.5"]; case: text; assert_eq!(got, expected, "{text}"));
    }
}

/// Player replacements: which typed forms are read and admitted, and an opponent-restricted
/// one applies only to opponents (616.1). A kind-changing substitute is a different event.
#[test]
fn typed_player_replacement_restrictions() {
    let texts = [
        "If one or more poison counters would be put on an opponent, twice that many poison counters are put on that player instead.",
        "If one or more poison counters would be put on a player, that many energy counters are put on that player instead.",
        "If one or more poison counters would be put on a player, that many plus one poison counters are put on it instead.",
        "If one or more rad counters would be put on a player, twice that many rad counters are put on that player instead.",
        "If one or more +1/+1 counters would be put on a player, twice that many +1/+1 counters are put on that player instead.",
        "If one or more poison counters would be put on you, twice that many poison counters are put on you instead.",
    ];
    let opp = def("Opp", "Enchantment", texts[0], None);
    admitted(&opp).expect("opponent player replacement must remain admitted");
    for text in [texts[2], texts[5]] {
        let d = def("Rep", "Enchantment", text, None);
        // Uncited (admission): only the explicitly retained player form is supported.
        crate::support::refused(admitted(&d), "ability on a permanent: sentence: Replacement");
    }
    let rad = def("Rep", "Enchantment", texts[3], None);
    crate::support::refused(admitted(&rad), "player counter: rad upkeep trigger (122.1i) is not implemented");
    admitted(&def("Rep", "Enchantment", texts[4], None)).unwrap();
    crate::support::refused(CardDef::new("Rep", "{0}", "Enchantment", texts[1], None), "text the grammar does not read");
    let spell = def("Probe", "Sorcery", "Each player gets a poison counter.", None);
    let mut g = Game::new(vec![spell, opp], 2, 20);
    g.put(0, 1, Zone::Battlefield);
    cast(&mut g, 0, vec![], &[]);
    cite!(["614.1a"]; assert_eq!((n(&g, 0, "poison"), n(&g, 1, "poison")), (1, 2)));
}

/// "That player" in a single target's amount is the target (per-holder evaluation).
#[test]
fn target_player_amount_counts_that_players_creatures() {
    let d = def("Probe", "Sorcery", "Target player gets X poison counters, where X is the number of creatures that player controls.", None);
    let mut g = Game::new(vec![d, bear()], 2, 20);
    for _ in 0..3 { g.put(0, 1, Zone::Battlefield); }
    g.put(1, 1, Zone::Battlefield);
    cast(&mut g, 0, vec![vec![Target::Player(1)]], &[]);
    cite!(["608.2h"]; assert_eq!((n(&g, 0, "poison"), n(&g, 1, "poison")), (0, 1)));
}

/// A player-counter read in a characteristic follows tracked mutations (613, cache).
#[test]
fn experience_read_follows_player_counter_changes() {
    let d = def("Mentor", "Creature — Human", "This creature gets +1/+1 for each experience counter you have.", Some((1, 1)));
    println!("admitted={:?}", admitted(&d));
    admitted(&d).expect("reviewed capability must remain admitted");
    let gain = def("Probe", "Sorcery", "You get an experience counter.", None);
    let mut g = Game::new(vec![d, gain], 2, 20);
    let m = g.put(0, 0, Zone::Battlefield);
    assert_eq!(g.power(m), Some(1));
    cast(&mut g, 1, vec![], &[]);
    cite!(["611.3a"]; assert_eq!(g.power(m), Some(2)));
    g.put_counters(Target::Player(1), "experience", 3);
    cite!(["109.5"]; assert_eq!(g.power(m), Some(2), "opponent's experience is not yours"));
    let before = g.clone();
    g.remove_counters(Target::Player(0), "experience", 1);
    assert_eq!((g.power(m), before.power(m)), (Some(1), Some(2)), "clone keeps its own cache");
    assert!(g.log.iter().any(|e| matches!(e, Event::CountersRemoved { from: Target::Player(0), .. })));
}

/// Lost players hold no counters (800.4a); remaining opponents still get theirs.
#[test]
fn lost_player_gets_no_counters() {
    let mut g = Game::new(vec![def("Probe", "Sorcery", "Each opponent gets a poison counter.", None)], 3, 20);
    g.put_counters(Target::Player(1), "poison", 9);
    cast(&mut g, 0, vec![], &[]);
    cite!(["704.5c"]; assert!(g.players[1].lost, "10 poison loses (704.5c)"));
    assert!(!g.players[2].lost);
    let before = g.log.len();
    let c = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: c, targets: vec![] }).unwrap();
    resolve(&mut g, &[]);
    assert!(g.log[before..].iter().all(|e| !matches!(e, Event::CountersPut { on: Target::Player(1), .. })), "lost player got counters");
    assert_eq!(n(&g, 2, "poison"), 2);
}

fn pinned(name: &str) -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let d = CardDef::from_faces(CardDef::catalog(&faces).get(name).unwrap()).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
    d
}

/// Entry counters and permanent replacements are unchanged beside a player replacement:
/// Hardened Scales adds one to "enters with two +1/+1 counters" (614.1c, 122.6), and a
/// poison replacement neither applies to it nor is applied by it.
#[test]
fn entry_counters_with_permanent_and_player_replacements() {
    let entering = def("Grower", "Creature — Beast", "This creature enters with two +1/+1 counters on it.", Some((0, 0)));
    let times = def("Times", "Enchantment", "If one or more poison counters would be put on a player, twice that many poison counters are put on that player instead.", None);
    let poison = def("Probe", "Sorcery", "Target player gets a poison counter.", None);
    let mut g = Game::new(vec![entering, pinned("Hardened Scales"), times, poison], 2, 20);
    g.put(0, 1, Zone::Battlefield);
    g.put(0, 2, Zone::Battlefield);
    let c = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: c, targets: vec![] }).unwrap();
    resolve(&mut g, &[]);
    let grower = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(0)).expect("entered");
    cite!(["614.1c", "122.6"]; assert_eq!(g.counters(grower).get("+1/+1"), Some(&3)));
    assert_eq!(g.power(grower), Some(3));
    cast(&mut g, 3, vec![vec![Target::Player(1)]], &[]);
    cite!(["614.1a"]; assert_eq!(n(&g, 1, "poison"), 2));
    assert_eq!(g.counters(grower).get("+1/+1"), Some(&3));
}

/// The rad boundary over references and possessors: a player reached through
/// "its controller" or "that player" keeps the rad refusal; a creature reached through "it"
/// keeps inert permanent rad; other player kinds stay admitted.
#[test]
fn rad_boundary_through_references() {
    for (text, ok) in [
        ("Put a rad counter on target creature. Put a rad counter on its controller.", false),
        ("Target player draws a card. Put a rad counter on that player.", false),
        ("Put a +1/+1 counter on target creature. Put a rad counter on it.", true),
        ("Put a rad counter on each creature.", true),
        ("Put a poison counter on target player.", true),
        ("Put an energy counter on each opponent.", true),
    ] {
        let d = def("Probe", "Sorcery", text, None);
        assert_eq!(admitted(&d).is_ok(), ok, "{text}");
    }
}

/// This mixed-holder phrase is outside the current grammar, so it is not evidence
/// about the compiler's player-kind admission boundary.
#[test]
fn creature_or_player_counter_phrase_is_a_grammar_limit() {
    // Uncited (grammar): the mixed holder phrase is unavailable before rad admission.
    crate::support::refused(CardDef::new("Probe", "{0}", "Sorcery",
        "Put a rad counter on target creature or player.", None), "text the grammar does not read");
}
