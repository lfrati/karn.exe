//! A distributive damage source's own
//! "its" names each member, and nothing else changes. Run with `engine::distributive_runtime_review::`.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};

fn creature(name: &str, p: i32, t: i32, text: &str) -> CardDef { CardDef::new(name, "{0}", "Creature — Beast", text, Some((p, t))).unwrap() }
fn spell(text: &str) -> CardDef { CardDef::new("Probe", "{0}", "Sorcery", text, None).unwrap() }
fn admitted(d: &CardDef) -> Result<(), String> { game::play::check(d).map_err(|e| e.0) }
fn damage(g: &Game) -> Vec<(ObjId, Target, u32)> {
    let mut out: Vec<_> = g.log.iter().filter_map(|e| match e {
        Event::DamageDealt { source, to, amount, .. } => Some((*source, *to, *amount)),
        _ => None,
    }).collect();
    out.sort_by_key(|(s, t, _)| (*s, format!("{t:?}")));
    out
}
fn pass(g: &mut Game) { for _ in 0..4 { if g.stack.is_empty() || g.decision.is_some() { return; } g.act(g.priority, Move::Pass).unwrap(); } }

/// Mine: Big 3/5 and Small 2/2. Theirs: Wall 1/20. Two cards in my hand after casting.
fn board(d: CardDef, targets: impl Fn(ObjId) -> Vec<Vec<Target>>) -> (Game, ObjId, ObjId, ObjId) {
    let mut g = Game::new(vec![d, creature("Big", 3, 5, ""), creature("Small", 2, 2, ""), creature("Wall", 1, 20, "")], 2, 20);
    let a = g.put(0, 1, Zone::Battlefield);
    let b = g.put(0, 2, Zone::Battlefield);
    let w = g.put(1, 3, Zone::Battlefield);
    for _ in 0..2 { g.put(0, 1, Zone::Hand); }
    let c = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: c, targets: targets(w) }).unwrap();
    pass(&mut g);
    (g, a, b, w)
}

/// The guard: a member quantity never reaches the player-recipient evaluator.
#[test]
fn player_capable_recipients_refuse_member_quantities() {
    for to in ["any target", "target player", "target player or planeswalker", "target creature and each opponent", "target creature's controller"] {
        let text = format!("Each creature you control deals damage equal to its power to {to}.");
        assert_eq!(admitted(&spell(&text)), Err("per-source object quantity in damage a player may be dealt".into()), "{text}");
    }
    for text in ["Target opponent loses 1 life. Each creature you control deals damage equal to its power to that player.",
                 "This spell deals 1 damage to any target. Each creature you control deals damage equal to its power to that permanent or player."] {
        assert_eq!(admitted(&spell(text)), Err("per-source object quantity in damage a player may be dealt".into()), "{text}");
    }
    let d = creature("Watcher", 1, 1, "Whenever an opponent casts a spell, each creature you control deals damage equal to its power to that player.");
    assert_eq!(admitted(&d), Err("per-source object quantity in damage a player may be dealt".into()));
}

/// Separate hits keep separate quantities: a member amount to the creature, a fixed one to a player.
#[test]
fn mixed_hits_keep_member_and_fixed_quantities_apart() {
    for (to, player, n) in [("1 damage to that creature's controller", 1, 1), ("2 damage to you", 0, 2)] {
        let text = format!("Each creature you control deals damage equal to its power to target creature and {to}.");
        let d = spell(&text);
        admitted(&d).unwrap();
        let (g, a, b, w) = board(d, |w| vec![vec![Target::Object(w)]]);
        let mut expected = vec![(a, Target::Object(w), 3), (a, Target::Player(player), n), (b, Target::Object(w), 2), (b, Target::Player(player), n)];
        expected.sort_by_key(|(s, t, _)| (*s, format!("{t:?}")));
        cite!(["120.1", "608.2h"]; case: &text; assert_eq!(damage(&g), expected, "{text}"));
    }
}

/// Fixed, counted and explicitly owned quantities are unchanged; member stats and the member's
/// controller are per source.
#[test]
fn fixed_counted_and_member_quantities() {
    for (amount, big, small) in [("2 damage", 2, 2), ("damage equal to the number of creatures you control", 2, 2),
                                  ("damage equal to its toughness", 5, 2), ("damage equal to the number of cards in its controller's hand", 2, 2)] {
        let text = format!("Each creature you control deals {amount} to target creature.");
        let d = spell(&text);
        admitted(&d).unwrap();
        let (g, a, b, w) = board(d, |w| vec![vec![Target::Object(w)]]);
        cite!(["120.1", "608.2h"]; case: &text; assert_eq!(damage(&g), vec![(a, Target::Object(w), big), (b, Target::Object(w), small)], "{text}"));
    }
    // a permanent's ability: "its" is each member, "this creature's" stays the source
    for (text, expect_source) in [("{T}: Each creature you control deals damage equal to its power to target creature.", Some(4)),
                                  ("{T}: Each other creature you control deals damage equal to this creature's power to target creature.", None)] {
        let d = creature("Src", 4, 4, text);
        admitted(&d).unwrap();
        let mut g = Game::new(vec![d, creature("Big", 3, 5, ""), creature("Wall", 1, 30, "")], 2, 20);
        let s = g.put(0, 0, Zone::Battlefield);
        let a = g.put(0, 1, Zone::Battlefield);
        let w = g.put(1, 2, Zone::Battlefield);
        let targets = vec![vec![Target::Object(w)]];
        let ability = (0..4).find(|&i| g.clone().act(0, Move::Activate { source: s, ability: i, targets: targets.clone(), paying: vec![] }).is_ok()).unwrap();
        g.act(0, Move::Activate { source: s, ability, targets, paying: vec![] }).unwrap();
        pass(&mut g);
        let expected = match expect_source {
            Some(n) => vec![(s, Target::Object(w), n), (a, Target::Object(w), 3)],
            None => vec![(a, Target::Object(w), 4)],
        };
        cite!(["120.1", "608.2h"]; case: text; assert_eq!(damage(&g), expected, "{text}"));
    }
}

/// Clause-local: a later sentence's "its" is not a member, and a later single actor reads its own power.
#[test]
fn member_binding_ends_with_its_clause() {
    let d = spell("Each creature you control deals damage equal to its power to target creature. Then that creature deals damage equal to its power to each creature you control.");
    admitted(&d).unwrap();
    let (g, a, b, w) = board(d, |w| vec![vec![Target::Object(w)]]);
    // "Then that creature deals damage equal to its power": the target is that clause's source (120.1), its own power read as applied (608.2h); the clause-local "its" is the card text's English
    cite!(["120.1", "608.2h"]; assert_eq!(damage(&g), vec![(a, Target::Object(w), 3), (b, Target::Object(w), 2), (w, Target::Object(a), 1), (w, Target::Object(b), 1)]));
}

/// A per-member "other" set must not include its own dealer. Both the member-quantity
/// form and the fixed form stay refused until per-source recipient selection exists
/// (608.2c). Band's distinct target stays supported.
#[test]
fn per_source_other_recipient_sets_are_refused() {
    for amount in ["damage equal to its power", "1 damage"] {
        let text = format!("Each creature you control deals {amount} to each other creature.");
        assert_eq!(admitted(&spell(&text)), Err(
            "distributive damage: per-source other recipient set not implemented".into()), "{text}");
    }
}

/// Explicit comparison keeps its own referent even under distributive dealers (201.2a).
#[test]
fn explicit_comparison_recipients_stay_supported() {
    for text in [
        "Tap target creature. Each creature you control deals 1 damage to each other creature with the same name as that creature.",
    ] {
        let d = spell(text);
        admitted(&d).unwrap();
        let mut g = Game::new(vec![d, creature("Big", 3, 5, ""), creature("Small", 2, 2, ""), creature("Wall", 1, 20, "")], 2, 20);
        let a = g.put(0, 1, Zone::Battlefield);
        let b = g.put(0, 2, Zone::Battlefield);
        let target = g.put(1, 3, Zone::Battlefield);
        let other = g.put(1, 3, Zone::Battlefield);
        let c = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card: c, targets: vec![vec![Target::Object(target)]] }).unwrap();
        pass(&mut g);
        cite!(["201.2a"]; case: text; assert_eq!(damage(&g), vec![(a, Target::Object(other), 1), (b, Target::Object(other), 1)], "{text}"));
        assert_eq!(g.obj(target).damage, 0);
    }
}

/// An earlier target must not replace each dealer when source and recipient groups can
/// overlap. Assert the bounded refusal.
#[test]
fn proven_target_exclusion_does_not_replace_each_dealer() {
    for dealers in ["Each creature that player controls", "Each creature", "Each creature an opponent controls", "Each creature you control", "Creatures you control", "All creatures you control"] {
        let verb = if dealers.starts_with("Each") { "deals" } else { "deal" };
        let text = format!("Tap target creature an opponent controls. {dealers} {verb} 1 damage to each other creature that player controls.");
        assert_eq!(admitted(&spell(&text)), Err(
            "other: earlier-target exclusion under distributive dealers not implemented".into()), "{text}");
    }
    // Direct current-controller queries do not prove the captured opponent identity:
    // ownership/control may have changed since the target was selected.
    let text = "Tap target creature an opponent controls. Each creature you control deals 1 damage to each other creature that creature's controller controls.";
    assert_eq!(admitted(&spell(text)), Err(
        "other: earlier-target exclusion under distributive dealers not implemented".into()));
}
