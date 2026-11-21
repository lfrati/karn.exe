//! A bare "it" damage recipient
//! after a retained mixed target: what it names, what is refused, and the
//! contrasts that must hold. Run with `r608::r608_2k_retained_target_pronoun_review::`.
use crate::support::{cite, paid};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};

fn spell(text: &str) -> CardDef { CardDef::new("Probe", "{0}", "Sorcery", text, None).unwrap() }
fn admitted(text: &str) -> Result<(), String> { game::play::check(&spell(text)).map_err(|e| e.0) }
fn creature(name: &str, p: i32) -> CardDef { CardDef::new(name, "{0}", "Creature — Bear", "", Some((p, 9))).unwrap() }

/// Mine: Big 3/9 and Small 2/9. The opponent's: Wall 1/9 and Wall 1/9. Returns the damage log,
/// the objects (Big, Small, Wall, Wall, this spell) and the life totals.
fn run(d: CardDef, spending: &str, targets: impl FnOnce([ObjId; 4]) -> Vec<Vec<Target>>) -> (Vec<(ObjId, Target, u32)>, [ObjId; 5], Vec<i32>) {
    let mut g = Game::new(vec![d, creature("Big", 3), creature("Small", 2), creature("Wall", 1)], 2, 20);
    let o = [g.put(0, 1, Zone::Battlefield), g.put(0, 2, Zone::Battlefield), g.put(1, 3, Zone::Battlefield), g.put(1, 3, Zone::Battlefield)];
    g.players[0].pool.colors = [10; 5];
    let c = g.put(0, 0, Zone::Hand);
    g.act(0, paid(spending, Move::Cast { card: c, targets: targets(o) })).unwrap();
    let spell = *g.stack.last().unwrap();
    for _ in 0..3 { if g.stack.is_empty() { break; } g.act(g.priority, Move::Pass).unwrap(); }
    let dealt = g.log.iter().filter_map(|e| match e {
        Event::DamageDealt { source, to, amount, .. } => Some((*source, *to, *amount)),
        _ => None,
    }).collect();
    (dealt, [o[0], o[1], o[2], o[3], spell], g.players.iter().map(|p| p.life).collect())
}

/// "it" is the chosen target, object or player (115.4; Oracle's "any
/// target. If it's a creature …" on Carbonize, Disintegrate and Searing Barb).
#[test]
fn bare_it_recipient_is_the_mixed_target() {
    for text in ["This spell deals 1 damage to any target. Each creature you control deals 2 damage to it.",
                 "This spell deals 1 damage to target player or planeswalker. Each creature you control deals 2 damage to it."] {
        admitted(text).unwrap();
        if text.contains("any target") {
            let (dealt, [a, b, w, _, s], _) = run(spell(text), "", |o| vec![vec![Target::Object(o[2])]]);
            cite!(["608.2c"]; case: text; assert_eq!(dealt, vec![(s, Target::Object(w), 1), (a, Target::Object(w), 2), (b, Target::Object(w), 2)], "{text}"));
        }
        let (dealt, [a, b, _, _, s], life) = run(spell(text), "", |_| vec![vec![Target::Player(1)]]);
        cite!(["608.2c"]; case: text; assert_eq!(dealt, vec![(s, Target::Player(1), 1), (a, Target::Player(1), 2), (b, Target::Player(1), 2)], "{text}"));
        assert_eq!(life, vec![20, 15]);
    }
}

/// The member-amount guard must see the bound target's player possibility, even
/// though bare "it" is Object-typed and this cast could choose a creature.
#[test]
fn member_amount_to_a_player_capable_it_is_refused() {
    for text in ["This spell deals 1 damage to any target. Each creature you control deals damage equal to its power to it.",
                 "This spell deals 1 damage to target player or planeswalker. Each creature you control deals damage equal to its power to it."] {
        crate::support::refused(admitted(text), "per-source object quantity in damage a player may be dealt");
    }
}

/// The clause's own dealer isn't the "it" it deals damage to. Same-sentence narrowing
/// that kept the dealer would have the "acted on" rule turn it into the spell, so nothing
/// would be dealt, even with object-only types. The acting subject is excluded
/// before narrowing; the Garruk "… to him" case still reaches the named source.
#[test]
fn target_dealer_hits_the_earlier_target() {
    for text in ["This spell deals 1 damage to target creature an opponent controls. Target creature you control deals 2 damage to it.",
                 "This spell deals 1 damage to any target. Target creature you control deals 2 damage to it."] {
        admitted(text).unwrap();
        let (dealt, [a, _, w, _, s], _) = run(spell(text), "", |o| vec![vec![Target::Object(o[2])], vec![Target::Object(o[0])]]);
        cite!(["608.2c"]; case: text; assert_eq!(dealt, vec![(s, Target::Object(w), 1), (a, Target::Object(w), 2)], "{text}"));
    }
}

/// There is no nearest-target rule. With the mixed target a candidate, the first form must
/// not silently read the spell nor the second bind the only Object-typed target: both are
/// ambiguous. A singular "it" for a plural target group must not read the spell either.
#[test]
fn ambiguous_or_plural_antecedents_are_refused() {
    let ambiguous = "reference: ambiguous earlier subject producers";
    for (text, why) in [("Tap target creature. This spell deals 1 damage to any target. Each creature you control deals 2 damage to it.", ambiguous),
                 ("This spell deals 1 damage to any target. Tap target creature. Each creature you control deals 2 damage to it.", ambiguous),
                 ("This spell deals 2 damage to each of up to two targets. Each creature you control deals 1 damage to it.", "damage recipient: singular reference to a plural producer")] {
        crate::support::refused(admitted(text), why);
    }
}

/// Contrasts that must hold.
#[test]
fn contrasts_stay() {
    // The typed union reference binds both kinds, and the member-amount guard covers it.
    let text = "This spell deals 1 damage to any target. Each creature you control deals 2 damage to that permanent or player.";
    let (dealt, [a, b, _, _, s], life) = run(spell(text), "", |_| vec![vec![Target::Player(1)]]);
    cite!(["608.2c"]; assert_eq!((dealt, life), (vec![(s, Target::Player(1), 1), (a, Target::Player(1), 2), (b, Target::Player(1), 2)], vec![20, 15])));
    assert_eq!(admitted("This spell deals 1 damage to any target. Each creature you control deals damage equal to its power to that permanent or player."),
               Err("per-source object quantity in damage a player may be dealt".into()));
    // A creature-only producer supports the member amount: the guard must use the bound
    // producer's kinds, not a widened recipient description.
    let text = "This spell deals 1 damage to target creature. Each creature you control deals damage equal to its power to it.";
    let (dealt, [a, b, w, _, s], _) = run(spell(text), "", |o| vec![vec![Target::Object(o[2])]]);
    cite!(["608.2c"]; assert_eq!(dealt, vec![(s, Target::Object(w), 1), (a, Target::Object(w), 3), (b, Target::Object(w), 2)]));
    // A subject "it" stays the spell after a mixed target (Court of Ire: "this enchantment deals
    // 2 damage to any target. If you're the monarch, it deals 7 damage instead"). Widening
    // outside the recipient would make a player-capable target the dealer.
    let text = "This spell deals 1 damage to any target. Then it deals 2 damage to each creature.";
    let (dealt, [_, _, _, _, s], _) = run(spell(text), "", |o| vec![vec![Target::Object(o[2])]]);
    assert_eq!(dealt.len(), 5);
    cite!(["608.2c"]; assert!(dealt.iter().all(|(src, _, _)| *src == s), "{dealt:?}"));
    // Pinned admitted cards whose subject "it" is the spell: Burst Lightning (unkicked: 2).
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let burst = CardDef::from_faces(CardDef::catalog(&faces).get("Burst Lightning").unwrap()).unwrap();
    let (dealt, [_, _, w, _, s], _) = run(burst, "R", |o| vec![vec![Target::Object(o[2])]]);
    assert_eq!(dealt, vec![(s, Target::Object(w), 2)]);
}

/// 608.2b: a retained pronoun does not rescue an illegal sole target or its new incarnation.
#[test]
fn illegal_target_fizzles_without_rebinding() {
    let text = "This spell deals 1 damage to any target. Each creature you control deals 2 damage to it.";
    let mut g = Game::new(vec![spell(text), creature("Big", 3), creature("Wall", 1)], 2, 20);
    let a = g.put(0, 1, Zone::Battlefield);
    let w = g.put(1, 2, Zone::Battlefield);
    let c = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: c, targets: vec![vec![Target::Object(w)]] }).unwrap();
    g.move_object(w, Zone::Graveyard);
    let replacement = g.put(1, 2, Zone::Battlefield);
    for _ in 0..3 { if g.stack.is_empty() { break; } g.act(g.priority, Move::Pass).unwrap(); }
    assert!(g.stack.is_empty());
    cite!(["608.2b"]; assert!(!g.log.iter().any(|e| matches!(e, Event::DamageDealt { .. }))));
    cite!(["608.2b", "400.7"]; assert_eq!((g.obj(a).damage, g.obj(replacement).damage), (0, 0)));
}

/// 608.2c: the target belongs to this body, including its condition branch; frames clone
/// their captured identities, but each branch evaluates its own live set of dealers.
#[test]
fn conditional_recipient_and_cloned_frame_keep_the_target() {
    let text = "This spell deals 1 damage to any target. If you control a creature, each creature you control deals 2 damage to it.";
    admitted(text).unwrap();
    let mut g = Game::new(vec![spell(text), creature("Big", 3), creature("Small", 2)], 2, 20);
    g.put(0, 1, Zone::Battlefield);
    let small = g.put(0, 2, Zone::Battlefield);
    let c = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: c, targets: vec![vec![Target::Player(1)]] }).unwrap();
    let mut fork = g.clone();
    g.move_object(small, Zone::Graveyard);
    for (branch, expected) in [(&mut g, 17), (&mut fork, 15)] {
        for _ in 0..3 { if branch.stack.is_empty() { break; } branch.act(branch.priority, Move::Pass).unwrap(); }
        cite!(["608.2c"]; case: format!("expected={expected}"); assert_eq!(branch.players[1].life, expected));
        assert!(branch.log.iter().filter_map(|e| match e {
            Event::DamageDealt { to, .. } => Some(to), _ => None,
        }).all(|to| *to == Target::Player(1)));
    }
    crate::support::refused(admitted("Choose one —\n• This spell deals 1 damage to any target.\n• Each creature you control deals 2 damage to it."), "input reference: nothing before it supplies the required participant/cardinality");
}


/// Only a mixed target's object possibility
/// licenses bare "it". Player-only targets do not become Object-pronoun candidates.
#[test]
fn player_only_targets_do_not_widen_bare_it() {
    for text in ["Target player loses 2 life. Each creature you control deals 1 damage to it.",
        "Target opponent loses 2 life. Each creature you control deals 1 damage to it."] {
        crate::support::refused(admitted(text), "input reference: nothing before it supplies the required participant/cardinality");
    }
    // An explicit spell still supplies the source fallback; the recipient is not the player
    // target. This is not the non-recipient fallthrough (r608_2c_mixed_reference_refusal_review).
    let text = "This spell deals 1 damage to target player. Each creature you control deals 2 damage to it.";
    let (dealt, [_, _, _, _, source], life) = run(spell(text), "", |_| vec![vec![Target::Player(1)]]);
    assert_eq!(dealt, vec![(source, Target::Player(1), 1)]);
    assert_eq!(life, vec![20, 19]);
    // The independent player slot does not compete with the earlier/later object slot.
    for (text, player_first) in [
        ("Tap target creature. Target player draws a card. Each creature you control deals 2 damage to it.", false),
        ("Target player draws a card. Tap target creature. Each creature you control deals 2 damage to it.", true),
    ] {
        admitted(text).unwrap();
        let (dealt, [a, b, w, _, _], _) = run(spell(text), "", |o| {
            let mut targets = vec![vec![Target::Object(o[2])], vec![Target::Player(1)]];
            if player_first { targets.reverse(); }
            targets
        });
        cite!(["608.2c"]; case: text; assert_eq!(dealt, vec![(a, Target::Object(w), 2), (b, Target::Object(w), 2)], "{text}"));
    }
}
