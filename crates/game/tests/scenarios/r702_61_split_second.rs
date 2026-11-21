//! Split second contributes stack-scoped casting and non-mana restrictions (702.61a-c).
use crate::support::cite;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use game::legal::Asked;
use game::state::Answer;
use std::sync::OnceLock;
fn pinned(name: &str) -> CardDef {
    static FACES: OnceLock<Vec<oracle::corpus::Face>> = OnceLock::new();
    let faces = FACES.get_or_init(|| oracle::corpus::load(&oracle::corpus::default_path()).unwrap());
    let d = CardDef::from_faces(CardDef::catalog(faces).get(name).unwrap()).unwrap();
    game::play::check(&d).unwrap(); d
}
fn def(name: &str, kind: &str, text: &str) -> CardDef {
    let d = CardDef::new(name, "{0}", kind, text, None).unwrap_or_else(|e| panic!("{name}: {text}: {e}"));
    game::play::check(&d).unwrap_or_else(|e| panic!("{text}: {}", e.0)); d
}
fn pass(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }
fn cast(card: ObjId, targets: Vec<Vec<Target>>) -> Move { Move::Cast { card, targets } }
fn activate(source: ObjId, ability: usize, targets: Vec<Vec<Target>>) -> Move { Move::Activate { source, ability, targets, paying: vec![] } }


#[test]
fn full_sudden_shock_blocks_both_players_casts_and_off_battlefield_activations() {
    let defs = vec![pinned("Sudden Shock"),
        def("Spell", "Instant", "You gain 1 life."),
        def("Resource", "Artifact", "{T}: Add {C}. You gain 1 life.\n{0}: You gain 1 life.\n{0}: Add {C}. Target player gains 1 life."),
        def("Cycler", "Instant", "Cycling {1}\nYou gain 1 life."),
        def("Grave spell", "Instant", "You gain 1 life.\nFlashback {0}")];
    let mut g = Game::new(defs, 2, 20);
    let shock = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[3] = 1; g.players[0].pool.colorless = 1;
    g.act(0, crate::support::paid("CR",cast(shock, vec![vec![Target::Player(1)]]))).unwrap();
    for player in 0..2 {
        let mut g = g.clone(); g.priority = player;
        g.players[player].pool.colorless = 10;
        let spell = g.put(player, 1, Zone::Hand);
        let resource = g.put(player, 2, Zone::Battlefield);
        let cycler = g.put(player, 3, Zone::Hand);
        let grave = g.put(player, 4, Zone::Graveyard);
        for card in [spell, grave] {
            cite!(["702.61a"]; assert!(!g.clone().act(g.priority, cast(card, vec![])).is_ok()));
            let before = format!("{g:?}");
            cite!(["702.61a"]; refused("an effect prohibits casting that spell (601.3)"): g.act(player, cast(card, vec![])));
            assert_eq!(format!("{g:?}"), before);
        }
        // cycling is the card's activated ability, wherever its keyword is listed
        let cycling = g.abilities(cycler).iter().position(|a| matches!(&**a, ir::Ability::Activated { .. })).unwrap();
        for (source, ability, targets) in [(resource, 1, vec![]), (cycler, cycling, vec![]), (resource, 2, vec![vec![Target::Player(player)]])] {
            cite!(["702.61a"]; assert!(!g.clone().act(g.priority, activate(source, ability, vec![])).is_ok()));
            let before = format!("{g:?}");
            cite!(["702.61a"]; refused("an effect prohibits activating that ability (602.5)"): g.act(player, activate(source, ability, targets)));
            assert_eq!(format!("{g:?}"), before);
        }
        cite!(["702.61b"]; assert!(g.clone().act(g.priority, activate(resource, 0, vec![])).is_ok(), "a mana ability with a side effect stays legal"));
        cite!(["702.61b"]; ok: g.act(player, activate(resource, 0, vec![])));
        assert_eq!(g.stack.len(), 1);
        assert_eq!(g.players[player].life, 21);
    }
    pass(&mut g);
    assert_eq!(g.players[1].life, 18);
    let spell = g.put(0, 1, Zone::Hand);
    cite!(["702.61a"]; assert!(g.clone().act(g.priority, cast(spell, vec![])).is_ok(), "the restriction ends with the spell"));
}
#[test]
fn full_krosan_grip_resolves_its_destroy_instruction() {
    let mut g = Game::new(vec![pinned("Krosan Grip"), def("Artifact", "Artifact", "")], 2, 20);
    let artifact = g.put(1, 1, Zone::Battlefield);
    let grip = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[4] = 1; g.players[0].pool.colorless = 2;
    g.act(0, crate::support::paid("CCG",cast(grip, vec![vec![Target::Object(artifact)]]))).unwrap();
    pass(&mut g);
    assert_eq!(g.obj(artifact).zone, Zone::Gone);
    cite!(["701.8a"]; assert_eq!(g.players[1].graveyard.len(), 1));
    assert!(g.stack.is_empty());
}
#[test]
fn casting_trigger_stacks_and_resolves_normally_above_split_second() {
    let mut g = Game::new(vec![pinned("Sudden Shock"), pinned("Guttersnipe")], 2, 20);
    g.put(0, 1, Zone::Battlefield);
    let shock = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[3] = 1; g.players[0].pool.colorless = 1;
    g.act(0, crate::support::paid("CR",cast(shock, vec![vec![Target::Player(1)]]))).unwrap();
    cite!(["702.61b"]; assert_eq!(g.stack.len(), 2));
    pass(&mut g);
    assert_eq!(g.stack.len(), 1);
    cite!(["702.61b"]; assert_eq!(g.players[1].life, 18));
    pass(&mut g);
    assert_eq!(g.players[1].life, 16);
}
#[test]
fn resolution_cast_offer_cannot_override_the_stack_restriction() {
    for split in [false, true] {
        let text = if split { "Split second\nYou gain 1 life." } else { "You gain 1 life." };
        let mut g = Game::new(vec![def("Spell", "Instant", text),
            def("Watcher", "Enchantment", "Whenever you cast a spell, exile the top card of your library. You may cast that card without paying its mana cost."),
            def("Free", "Instant", "You gain 2 life.")], 2, 20);
        g.put(0, 1, Zone::Battlefield);
        let top=g.put(0, 2, Zone::Library);
        let spell = g.put(0, 0, Zone::Hand);
        g.act(0, cast(spell, vec![])).unwrap();
        pass(&mut g);
        assert_eq!(g.exile.len(),1);
        let exiled=g.exile[0];
        assert_ne!(exiled,top);
        assert_eq!(g.obj(exiled).card,Some(2));
        // Native Cast questions expose the retained permission domain; they do
        // not claim every card can legally be cast under current restrictions.
        assert_eq!(g.asked(),Some((0,Asked::Cast{cards:vec![exiled]})));
        if split {
            let before=format!("{g:?}");
            cite!(["702.61a","101.2"]; refused("an effect prohibits casting that spell (601.3)"): g.act(0,cast(exiled,vec![])));
            assert_eq!(format!("{g:?}"),before);
            g.act(0,Move::Choose(Answer::No)).unwrap();
            assert_eq!(g.stack.len(),1);
        } else {
            cite!(["608.2g"]; ok: g.act(0,cast(exiled,vec![])));
        }
        while !g.stack.is_empty() {
            assert!(g.decision.is_none(),"no further card exists for the cast trigger: {:?}",g.decision);
            g.act(g.priority,Move::Pass).unwrap();
        }
        assert_eq!(g.players[0].life,if split {21}else{23});
        assert_eq!(g.exile.contains(&exiled),split);
    }
}
#[test]
fn replicated_copy_and_clones_restrict_until_the_last_source_leaves() {
    let mut g = Game::new(vec![def("Echo", "Instant", "Split second\nReplicate {1}\nYou gain 1 life."),
        def("Other", "Instant", "You gain 1 life.")], 2, 20);
    let echo = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colorless = 1;
    g.act(0, crate::support::paid("C",Move::WithReplicate(1, Box::new(cast(echo, vec![]))))).unwrap();
    let original = g.stack[0];
    pass(&mut g);
    assert_eq!(g.stack.len(), 2);
    cite!(["707.10"]; assert!(g.stack.iter().all(|o| g.has(*o, "Split Second"))));
    let other = g.put(0, 1, Zone::Hand);
    cite!(["702.61a"]; assert!(!g.clone().act(g.priority, cast(other, vec![])).is_ok()));
    assert!(g.counter(original), "split second does not forbid being countered");
    cite!(["702.61a"]; assert!(!g.clone().act(g.priority, cast(other, vec![])).is_ok(), "copy remains an independent source"));
    for mut g in [g.clone(), g] { pass(&mut g); cite!(["702.61a"]; assert!(g.clone().act(g.priority, cast(other, vec![])).is_ok())); }
}
#[test]
fn layered_grant_uses_the_same_scoped_rules_and_ends_with_its_source() {
    let mut g = Game::new(vec![def("Grant", "Enchantment", "Instant and sorcery spells you control have split second."),
        def("Spell", "Instant", "You gain 1 life.")], 2, 20);
    let grant = g.put(0, 0, Zone::Battlefield);
    let first = g.put(0, 1, Zone::Hand);
    let other = g.put(0, 1, Zone::Hand);
    g.act(0, cast(first, vec![])).unwrap();
    assert!(g.has(g.stack[0], "Split Second"));
    cite!(["702.61a"]; assert!(!g.clone().act(g.priority, cast(other, vec![])).is_ok()));
    let clone = g.clone();
    g.destroy(grant);
    cite!(["702.61a"]; assert!(g.clone().act(g.priority, cast(other, vec![])).is_ok()));
    assert!(!clone.clone().act(clone.priority, cast(other, vec![])).is_ok());
}
#[test]
fn permanent_spell_restricts_only_on_stack_and_plot_keeps_its_own_timing() {
    let mut g = Game::new(vec![def("Permanent", "Artifact", "Split second"),
        def("Spell", "Instant", "You gain 1 life."), pinned("Slickshot Show-Off")], 2, 20);
    let source = g.put(0, 0, Zone::Hand);
    let other = g.put(0, 1, Zone::Hand);
    let plot = g.put(0, 2, Zone::Hand);
    g.players[0].pool.colors[3] = 1; g.players[0].pool.colorless = 1;
    g.act(0, cast(source, vec![])).unwrap();
    cite!(["702.61a"]; assert!(!g.clone().act(g.priority, cast(other, vec![])).is_ok()));
    cite!(["702.170a"]; refused("plot only in your main phase with an empty stack (702.170a)"): g.act(0, Move::Plot(plot)));
    pass(&mut g);
    assert!(g.has(source, "Split Second"));
    cite!(["702.61a"]; assert!(g.clone().act(g.priority, cast(other, vec![])).is_ok(), "a battlefield keyword does not impose a stack rule"));
}

#[test]
fn pinned_loyalty_mana_production_is_not_legal_at_split_second_priority() {
    let mut g = Game::new(vec![def("Spell", "Instant", "Split second\nYou gain 1 life."),
        pinned("Chandra, Novice Pyromancer")], 2, 20);
    let chandra = g.put(0, 1, Zone::Battlefield);
    g.put_counters(chandra, "loyalty", 5);
    assert!(g.clone().act(g.priority, activate(chandra, 1, vec![])).is_ok());
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, cast(spell, vec![])).unwrap();
    cite!(["702.61a", "605.1a"]; assert!(!g.clone().act(g.priority, activate(chandra, 1, vec![])).is_ok()));
    let before = format!("{g:?}");
    cite!(["606.3"]; refused("a loyalty ability is activated in a main phase of your own turn with the stack empty (606.3)"): g.act(0, activate(chandra, 1, vec![])));
    assert_eq!(format!("{g:?}"), before);
}

#[test]
fn all_seven_full_player_targeting_cards_keep_their_singleton_rules() {
    for name in ["Leyline of Sanctity", "Aegis of the Gods", "Teyo, the Shieldmage",
        "Metropolis Reformer", "Spirit of the Hearth", "Ivory Mask", "True Believer"] {
        let mut g = Game::new(vec![pinned(name), pinned("Lightning Bolt")], 2, 20);
        g.put(1, 0, Zone::Battlefield);
        g.players[0].pool.colors[3] = 1;
        let bolt = g.put(0, 1, Zone::Hand);
        let why = if matches!(name, "Ivory Mask" | "True Believer") { "an illegal target, that player: that player has shroud (702.18a)" } else { "an illegal target, that player: that player has hexproof (702.11c)" };
        cite!(["601.2c", "115.1"]; case: name; refused(why): g.act(0, cast(bolt, vec![vec![Target::Player(1)]])));
        let mut self_cast = g.clone(); self_cast.priority = 1;
        self_cast.players[1].pool.colors[3] = 1;
        let own_bolt = self_cast.put(1, 1, Zone::Hand);
        cite!(["702.11c", "702.18a"]; case: name; assert_eq!(self_cast.act(1, crate::support::paid("R",cast(own_bolt, vec![vec![Target::Player(1)]]))).is_ok(),
            !matches!(name, "Ivory Mask" | "True Believer"), "{name}"));
    }
}
