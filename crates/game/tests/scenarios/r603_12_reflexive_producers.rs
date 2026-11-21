//! Any action can produce a reflexive triggered ability ("When you do", 603.12) or an
//! outcome ("If you do", "If you can't", 118.12), not only a payment of mana or one
//! sacrifice: a discard, a mill, creating tokens, putting or removing a counter, tapping,
//! sacrificing the source or several permanents. The ability triggers once for each time the
//! action is done as written, after the resolution that did it, with targets chosen then, and
//! it can be responded to; an action declined, impossible or done only in part produces none
//! (Wick's Patrol ruling 2024-07-26: milling fewer than three doesn't trigger it). Riveteers
//! Overlook and Faebloom Trick are Oracle cards.
use crate::scenarios::r708_8_turned_face_up::{game, put};
use crate::support::cite;
use crate::support::*;

type Text<'a> = (&'a str, &'a str, &'a str, &'a str, Option<(i32, i32)>);

fn spell(text: &str) -> Text<'_> { ("Parent", "{0}", "Instant", text, None) }

/// Settle the stack, answering a "may" with `yes`, any object choice with its first
/// objects and every target question with `targets`: the questions asked, in order.
fn run(t: &mut T, yes: bool, targets: &[Target]) -> Vec<&'static str> {
    let mut asked = Vec::new();
    for _ in 0..200 {
        let Some((p, question)) = t.g.asked() else { break };
        match question {
            Asked::Priority if t.g.stack.is_empty() => break,
            Asked::Priority => { t.g.act(p, Move::Pass).unwrap(); }
            Asked::May => { asked.push("may"); t.g.act(p, Move::Choose(if yes { Answer::Yes } else { Answer::No })).unwrap(); }
            Asked::Targets { .. } => { asked.push("targets"); t.g.act(p, Move::ChooseTargets(targets.iter().map(|t| vec![*t]).collect())).unwrap(); }
            Asked::Objects { among, count, .. } | Asked::Discard { among, count } => {
                asked.push("objects");
                t.g.act(p, Move::Choose(Answer::Objects(among[..count].to_vec()))).unwrap();
            }
            Asked::Search { among, min, .. } | Asked::Select { among, min, .. } => {
                asked.push("search");
                t.g.act(p, Move::Choose(Answer::Objects(among[..min.max(1).min(among.len())].to_vec()))).unwrap();
            }
            Asked::Attackers(_) => { t.g.act(p, Move::Attack(vec![])).unwrap(); }
            Asked::Blockers { .. } => { t.g.act(p, Move::Block(vec![])).unwrap(); }
            other => panic!("unexpected question {other:?}"),
        }
    }
    asked
}

/// Player 0 says yes to "you may discard a card" and discards the first card offered, if asked which.
fn discard(t: &mut T) {
    t.g.act(0, Move::Choose(Answer::Yes)).unwrap();
    if let Some((_, Asked::Objects { among, .. } | Asked::Discard { among, .. })) = t.g.asked() {
        t.g.act(0, Move::Choose(Answer::Objects(vec![among[0]]))).unwrap();
    }
}

fn main_phase(t: &mut T) { while t.g.part != Part::PrecombatMain { t.g.act(t.g.priority, Move::Pass).unwrap(); } }

/// Player 0 casts the test spell, which resolves; the questions asked until the stack is empty.
fn cast(t: &mut T, yes: bool, targets: &[Target]) -> Vec<&'static str> {
    let card = put(t, 0, "Parent", Zone::Hand);
    t.g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    run(t, yes, targets)
}

#[test]
fn an_optional_discard_triggers_when_done_and_not_when_declined_or_impossible() {
    let text = "You may discard a card. When you do, target player gains 3 life.";
    for (cards, yes) in [(1, true), (1, false), (0, true)] {
        let mut t = game(&["Wastes"], &[spell(text)]);
        main_phase(&mut t);
        for _ in 0..cards { put(&mut t, 0, "Wastes", Zone::Hand); }
        let asked = cast(&mut t, yes, &[Target::Player(0)]);
        let did = cards > 0 && yes;
        cite!(["603.12"]; case: format!("{cards} {yes}"); assert_eq!(t.g.players[0].life, if did { 23 } else { 20 }));
        assert_eq!(asked.contains(&"targets"), did, "{asked:?}");
        assert_eq!(t.g.players[0].graveyard.len(), usize::from(did) + 1, "the card discarded, and the spell");
    }
}

#[test]
fn the_reflexive_ability_waits_for_the_resolution_and_can_be_responded_to() {
    // what follows "when you do" is the reflexive ability's (603.12): the parent goes on in another mode
    let text = "Choose one or both —\n• You may discard a card. When you do, target player gains 3 life.\n• Draw a card.";
    let mut t = game(&["Wastes"], &[spell(text)]);
    main_phase(&mut t);
    put(&mut t, 0, "Wastes", Zone::Hand);
    let library = t.g.players[0].library.len();
    let card = put(&mut t, 0, "Parent", Zone::Hand);
    t.g.act(0, Move::Modes(vec![0, 1], Box::new(Move::Cast { card, targets: vec![] }))).unwrap();
    t.g.act(0, Move::Pass).unwrap();
    t.g.act(1, Move::Pass).unwrap();
    discard(&mut t);
    assert!(matches!(t.g.asked(), Some((0, Asked::Targets { .. }))), "{:?}", t.g.asked());
    // the resolution finished first: its card is drawn, then the ability is put on the stack (603.3)
    cite!(["603.12", "603.3"]; assert_eq!(t.g.players[0].library.len(), library - 1));
    t.g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap();
    assert_eq!(t.g.stack.len(), 1);
    assert!(matches!(t.g.asked(), Some((0, Asked::Priority))), "its controller gets priority with it on the stack");
    run(&mut t, true, &[]);
    assert_eq!(t.g.players[1].life, 23);
}

#[test]
fn milling_fewer_cards_than_it_says_does_not_trigger_it() {
    for library in [10, 2] {
        let mut t = game(&[], &[spell("Mill three cards. When you do, you gain 3 life.")]);
        main_phase(&mut t);
        while t.g.players[0].library.len() > library { let top = t.g.players[0].library[0]; t.g.move_object(top, Zone::Exile); }
        let milled: Vec<_> = t.g.players[0].library.iter().rev().take(library.min(3)).copied().collect();
        let parent=put(&mut t,0,"Parent",Zone::Hand);
        t.g.act(0,Move::Cast{card:parent,targets:vec![]}).unwrap();
        t.g.act(0,Move::Pass).unwrap();t.g.act(1,Move::Pass).unwrap();
        cite!(["404.3"]; ok: t.g.act(0,Move::Choose(Answer::Objects(milled))));
        run(&mut t,true,&[]);
        cite!(["603.12"]; case: library; assert_eq!(t.g.players[0].life, if library >= 3 { 23 } else { 20 }));
        assert_eq!(t.g.players[0].graveyard.len(), library.min(3) + 1, "it mills what it can");
    }
}

#[test]
fn creating_tokens_triggers_faebloom_trick() {
    let mut t = game(&["Faebloom Trick", "Grizzly Bears"], &[]);
    main_phase(&mut t);
    let bears = put(&mut t, 1, "Grizzly Bears", Zone::Battlefield);
    let trick = put(&mut t, 0, "Faebloom Trick", Zone::Hand);
    t.mana(0, "{2}{U}");
    t.g.act(0, paid("CCU", Move::Cast { card: trick, targets: vec![] })).unwrap();
    let asked = run(&mut t, true, &[Target::Object(bears)]);
    cite!(["603.12"]; assert!(t.g.obj(bears).tapped, "{asked:?}"));
    assert_eq!(t.g.battlefield.iter().filter(|o| t.g.controller(**o) == 0).count(), 2, "two Faeries");
}

#[test]
fn putting_and_removing_a_counter_trigger_it_and_no_counter_to_remove_does_not() {
    let texts = [("Quester", "{0}", "Enchantment", "{0}: Put a quest counter on this enchantment. When you do, you gain 1 life.", None),
                 ("Remover", "{0}", "Creature — Ninja", "{0}: You may remove a +1/+1 counter from this creature. When you do, you gain 2 life.", Some((1, 1)))];
    let mut t = game(&[], &texts);
    main_phase(&mut t);
    let quester = put(&mut t, 0, "Quester", Zone::Battlefield);
    t.g.act(0, Move::Activate { source: quester, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    run(&mut t, true, &[]);
    cite!(["603.12"]; assert_eq!(t.g.players[0].life, 21));
    assert_eq!(t.g.obj(quester).counters.get("quest"), Some(&1));
    for counters in [0, 1] {
        let remover = put(&mut t, 0, "Remover", Zone::Battlefield);
        if counters > 0 { t.g.set_initial_counters(remover, "+1/+1", counters); }
        let life = t.g.players[0].life;
        t.g.act(0, Move::Activate { source: remover, ability: 0, targets: vec![], paying: vec![] }).unwrap();
        run(&mut t, true, &[]);
        cite!(["603.12", "118.12"]; case: counters; assert_eq!(t.g.players[0].life, life + if counters > 0 { 2 } else { 0 }));
        assert_eq!(t.g.obj(remover).counters.get("+1/+1").copied().unwrap_or(0), 0);
    }
}

#[test]
fn sacrificing_two_creatures_is_done_wholly_or_not_at_all() {
    for creatures in [1, 2] {
        let mut t = game(&["Grizzly Bears"], &[spell("You may sacrifice two creatures. When you do, you gain 1 life.")]);
        main_phase(&mut t);
        let bodies:Vec<_>=(0..creatures).map(|_|put(&mut t,0,"Grizzly Bears",Zone::Battlefield)).collect();
        let parent=put(&mut t,0,"Parent",Zone::Hand);
        t.g.act(0,Move::Cast{card:parent,targets:vec![]}).unwrap();
        t.g.act(0,Move::Pass).unwrap();t.g.act(1,Move::Pass).unwrap();
        if creatures == 2 {
            t.g.act(0,Move::Choose(Answer::Yes)).unwrap();
            cite!(["404.3"]; ok: t.g.act(0,Move::Choose(Answer::Objects(bodies))));
        }
        run(&mut t,true,&[]);
        cite!(["603.12", "118.12"]; case: creatures; assert_eq!(t.g.players[0].life, if creatures == 2 { 21 } else { 20 }));
        assert_eq!(t.g.battlefield.len(), if creatures == 2 { 0 } else { 1 }, "one creature isn't sacrificed for a cost of two");
    }
}

#[test]
fn tapping_the_source_triggers_it_unless_it_is_tapped() {
    let texts = [("Tapper", "{0}", "Creature — Soldier", "{0}: You may tap this creature. When you do, you gain 1 life.", Some((1, 1)))];
    for tapped in [false, true] {
        let mut t = game(&[], &texts);
        main_phase(&mut t);
        let tapper = put(&mut t, 0, "Tapper", Zone::Battlefield);
        if tapped { t.g.tap(tapper); }
        t.g.act(0, Move::Activate { source: tapper, ability: 0, targets: vec![], paying: vec![] }).unwrap();
        run(&mut t, true, &[]);
        cite!(["603.12", "118.12"]; case: tapped; assert_eq!(t.g.players[0].life, if tapped { 20 } else { 21 }));
    }
}

#[test]
fn riveteers_overlook_sacrifices_itself_and_then_searches() {
    let mut t = game(&["Riveteers Overlook", "Mountain"], &[]);
    main_phase(&mut t);
    put(&mut t, 0, "Mountain", Zone::Library);
    let overlook = put(&mut t, 0, "Riveteers Overlook", Zone::Hand);
    t.g.act(0, Move::PlayLand(overlook)).unwrap();
    let asked = run(&mut t, true, &[]);
    let names: Vec<String> = t.g.battlefield.iter().map(|o| t.g.obj(*o).chars.name.clone()).collect();
    cite!(["603.12"]; assert_eq!(names, ["Mountain"], "{asked:?}"));
    assert!(t.g.obj(t.g.battlefield[0]).tapped);
    assert_eq!(t.g.players[0].life, 21);
}

#[test]
fn two_reflexive_abilities_of_one_resolution_each_trigger_after_it() {
    let text = "Choose one or both —\n• You may discard a card. When you do, you gain 2 life.\n• You may discard a card. When you do, you gain 3 life.";
    let mut t = game(&["Wastes"], &[spell(text)]);
    main_phase(&mut t);
    for _ in 0..2 { put(&mut t, 0, "Wastes", Zone::Hand); }
    let card = put(&mut t, 0, "Parent", Zone::Hand);
    t.g.act(0, Move::Modes(vec![0, 1], Box::new(Move::Cast { card, targets: vec![] }))).unwrap();
    t.g.act(0, Move::Pass).unwrap();
    t.g.act(1, Move::Pass).unwrap();
    for _ in 0..2 {
        discard(&mut t);
    }
    if matches!(t.g.asked(), Some((_, Asked::Order(_)))) { t.g.act(0, Move::Order(vec![0, 1])).unwrap(); }
    cite!(["603.12"]; assert_eq!(t.g.stack.len(), 2));
    run(&mut t, true, &[]);
    assert_eq!(t.g.players[0].life, 25);
}

#[test]
fn its_source_gone_it_still_resolves_as_the_source_last_was() {
    let texts = [("Spark", "{0}", "Creature — Elemental", "{0}: You may discard a card. When you do, this creature deals 2 damage to target player.", Some((1, 1)))];
    let mut t = game(&["Wastes"], &texts);
    main_phase(&mut t);
    let spark = put(&mut t, 0, "Spark", Zone::Battlefield);
    put(&mut t, 0, "Wastes", Zone::Hand);
    t.g.act(0, Move::Activate { source: spark, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    t.g.act(0, Move::Pass).unwrap();
    t.g.act(1, Move::Pass).unwrap();
    discard(&mut t);
    t.g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap();
    assert_eq!(t.g.stack.len(), 1);
    t.g.move_object(spark, Zone::Graveyard);
    run(&mut t, true, &[]);
    cite!(["603.10", "113.7a"]; assert_eq!(t.g.players[1].life, 18));
}

#[test]
fn if_you_cannot_discard_the_other_instruction_is_followed() {
    let text = "Discard a card. If you can't, you lose 2 life.";
    for cards in [0, 1] {
        let mut t = game(&["Wastes"], &[spell(text)]);
        main_phase(&mut t);
        for _ in 0..cards { put(&mut t, 0, "Wastes", Zone::Hand); }
        cast(&mut t, true, &[]);
        cite!(["118.12"]; case: cards; assert_eq!(t.g.players[0].life, if cards == 0 { 18 } else { 20 }));
    }
}

/// Done once for each opponent, an optional discard would be a producer repeated in one
/// resolution (603.12a); the engine does it only once, so it is refused, not run once.
#[test]
fn an_optional_act_for_each_opponent_is_refused() {
    for text in ["For each opponent, you may discard a card. When you do, you gain 1 life.",
                 "For each opponent, you may discard a card. If you do, you gain 1 life.",
                 "For each opponent, sacrifice a creature."] {
        let d = CardDef::new("Parent", "{0}", "Instant", text, None).unwrap();
        let why = game::play::check(&d).unwrap_err().0;
        cite!(["603.12a"]; assert!(why.contains("for each player"), "{text}: {why}"));
    }
}

fn energy(t: &T, p: usize) -> u32 { t.g.counters(Target::Player(p)).get("energy").copied().unwrap_or(0) }

/// Greenbelt Rampager: "When this creature enters, pay {E}{E} (two energy counters). If you
/// can't, return this creature to its owner's hand and you get {E}." Paid in full or not at all
/// (107.14, 118.3): with one energy, nothing is paid.
#[test]
fn greenbelt_rampager_pays_two_energy_or_returns_and_gets_one() {
    for had in [0, 1, 2] {
        let mut t = game(&["Greenbelt Rampager"], &[]);
        main_phase(&mut t);
        if had > 0 { t.g.put_counters(Target::Player(0), "energy", had); }
        let rampager = put(&mut t, 0, "Greenbelt Rampager", Zone::Hand);
        t.mana(0, "{G}");
        t.g.act(0, paid("G", Move::Cast { card: rampager, targets: vec![] })).unwrap();
        let asked = run(&mut t, true, &[]);
        let stayed = t.g.battlefield.iter().any(|o| t.g.obj(*o).chars.name == "Greenbelt Rampager");
        cite!(["107.14", "118.3", "118.12"]; case: had; assert_eq!(stayed, had >= 2, "{asked:?}"));
        assert_eq!(energy(&t, 0), if had >= 2 { 0 } else { had + 1 });
        assert!(asked.is_empty(), "a payment required, not offered: {asked:?}");
    }
}

/// Glint-Sleeve Siphoner: "At the beginning of your upkeep, you may pay {E}{E}. If you do, you
/// draw a card and you lose 1 life." Not offered with too little energy (118.3).
#[test]
fn glint_sleeve_siphoner_may_pay_energy_in_its_upkeep() {
    for (had, yes) in [(2, true), (2, false), (1, true)] {
        let mut t = game(&["Glint-Sleeve Siphoner"], &[]);
        put(&mut t, 0, "Glint-Sleeve Siphoner", Zone::Battlefield);
        t.g.put_counters(Target::Player(0), "energy", had);
        let library = t.g.players[0].library.len();
        let mut asked = Vec::new();
        // to its controller's second turn: the first player skips their first draw (103.8a)
        while !(t.g.turn == 3 && t.g.part == Part::Draw) {
            asked.extend(run(&mut t, yes, &[]));
            if !(t.g.turn == 3 && t.g.part == Part::Draw) {
                match t.g.decision { Some(Decision::Attackers { player }) => { t.g.act(player, Move::Attack(vec![])).unwrap(); } _ => { t.g.act(t.g.priority, Move::Pass).unwrap(); } }
            }
        }
        let paid = had >= 2 && yes;
        cite!(["107.14", "118.12"]; case: format!("{had} {yes}"); assert_eq!(energy(&t, 0), if paid { 0 } else { had }));
        assert_eq!(t.g.players[0].life, if paid { 19 } else { 20 });
        assert_eq!(t.g.players[0].library.len(), library - 1 - usize::from(paid), "the upkeep's draw, and the draw step's (504.1)");
        assert_eq!(asked.contains(&"may"), had >= 2, "{asked:?}");
    }
}

/// Voltaic Brawler: "Whenever this creature attacks, you may pay {E}. If you do, it gets +1/+1
/// and gains trample until end of turn."
#[test]
fn voltaic_brawler_pays_one_energy_as_it_attacks() {
    let mut t = game(&["Voltaic Brawler"], &[]);
    main_phase(&mut t);
    let brawler = put(&mut t, 0, "Voltaic Brawler", Zone::Battlefield);
    t.g.put_counters(Target::Player(0), "energy", 1);
    while t.g.part != Part::DeclareAttackers { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.g.act(0, Move::Attack(vec![(brawler, Target::Player(1))])).unwrap();
    run(&mut t, true, &[]);
    cite!(["107.14", "118.12"]; assert_eq!(t.g.power(brawler), Some(4)));
    assert_eq!(energy(&t, 0), 0);
}

/// Glacial Dragonhunt: "Draw a card, then you may discard a card. When you discard a nonland
/// card this way, Glacial Dragonhunt deals 3 damage to target creature." What was discarded
/// this way decides (603.12, 608.2c): a land, or nothing, triggers nothing.
#[test]
fn glacial_dragonhunt_triggers_on_a_nonland_card_discarded_this_way() {
    for (discard, yes) in [("Grizzly Bears", true), ("Wastes", true), ("Grizzly Bears", false)] {
        let mut t = game(&["Glacial Dragonhunt", "Grizzly Bears"], &[]);
        main_phase(&mut t);
        let theirs = put(&mut t, 1, "Grizzly Bears", Zone::Battlefield);
        let card = put(&mut t, 0, discard, Zone::Hand);
        let hunt = put(&mut t, 0, "Glacial Dragonhunt", Zone::Hand);
        t.mana(0, "{U}{R}");
        t.g.act(0, paid("UR", Move::Cast { card: hunt, targets: vec![] })).unwrap();
        t.g.act(0, Move::Pass).unwrap();
        t.g.act(1, Move::Pass).unwrap();
        t.g.act(0, Move::Choose(if yes { Answer::Yes } else { Answer::No })).unwrap();
        if yes { t.g.act(0, Move::Choose(Answer::Objects(vec![card]))).unwrap(); }
        let asked = run(&mut t, true, &[Target::Object(theirs)]);
        let hit = yes && discard == "Grizzly Bears";
        cite!(["603.12", "608.2c"]; case: format!("{discard} {yes}"); assert_eq!(t.g.obj(theirs).zone == Zone::Battlefield, !hit, "{asked:?}"));
        assert_eq!(asked.contains(&"targets"), hit);
    }
}

/// Obscura Interceptor: "When this creature enters, it connives. When it connives this way,
/// return up to one target spell to its owner's hand." It connives whatever it discards, and
/// even gone (701.50; ruling 2022-04-29), so the reflexive ability always triggers.
#[test]
fn obscura_interceptor_returns_a_spell_when_it_connives() {
    for gone in [false, true] {
        let mut t = game(&["Obscura Interceptor", "Grizzly Bears"], &[]);
        main_phase(&mut t);
        // a spell of theirs to return: its owner's, Grizzly Bears, with the Interceptor flashed
        // in on top of it
        let bears = put(&mut t, 0, "Grizzly Bears", Zone::Hand);
        t.mana(0, "{1}{G}");
        t.g.act(0, paid("CG", Move::Cast { card: bears, targets: vec![] })).unwrap();
        let spell = *t.g.stack.last().unwrap();
        let interceptor = put(&mut t, 0, "Obscura Interceptor", Zone::Hand);
        t.mana(0, "{1}{W}{U}{B}");
        t.g.act(0, paid("CWUB", Move::Cast { card: interceptor, targets: vec![] })).unwrap();
        t.g.act(0, Move::Pass).unwrap();
        t.g.act(1, Move::Pass).unwrap();
        // its enters trigger is on the stack, above the spell
        assert_eq!(t.g.stack.len(), 2, "{:?}", t.g.asked());
        if gone { let it = t.find_in_battlefield("Obscura Interceptor"); t.g.move_object(it, Zone::Graveyard); }
        let hand = t.g.players[0].hand.len();
        let asked = run(&mut t, true, &[Target::Object(spell)]);
        cite!(["603.12", "701.50"]; case: gone; assert!(asked.contains(&"targets"), "{asked:?}"));
        // it drew a card and discarded one; the spell came back
        assert_eq!(t.g.players[0].hand.len(), hand + 1, "the Bears back in hand: {asked:?}");
        assert!(t.g.stack.is_empty());
    }
}

/// Prishe's Wanderings: "Search your library for a basic land card or Town card, put it onto
/// the battlefield tapped, then shuffle. When you search your library this way, put a +1/+1
/// counter on target creature you control." It searched whether or not it found one (118.12b).
#[test]
fn prishes_wanderings_triggers_on_the_search_found_or_not() {
    for basic in [true, false] {
        let mut t = game(&["Prishe's Wanderings", "Grizzly Bears", "Forest"], &[]);
        main_phase(&mut t);
        let ours = put(&mut t, 0, "Grizzly Bears", Zone::Battlefield);
        // no basic land at all: the library's Wastes go, and a Forest comes in or doesn't
        while let Some(top) = t.g.players[0].library.first().copied() { t.g.move_object(top, Zone::Exile); }
        if basic { put(&mut t, 0, "Forest", Zone::Library); }
        put(&mut t, 0, "Grizzly Bears", Zone::Library);
        let spell = put(&mut t, 0, "Prishe's Wanderings", Zone::Hand);
        t.mana(0, "{2}{G}");
        t.g.act(0, paid("CCG", Move::Cast { card: spell, targets: vec![] })).unwrap();
        let asked = run(&mut t, true, &[Target::Object(ours)]);
        cite!(["603.12", "118.12b"]; case: basic; assert_eq!(t.g.power(ours), Some(3), "{asked:?}"));
        assert_eq!(t.g.battlefield.len(), if basic { 2 } else { 1 });
    }
}

/// "When you discard a nonland card this way, you gain life equal to that card's mana value":
/// it triggers once for each nonland card discarded, and "that card" is the one it triggered
/// for (603.12a, 603.2c). Grizzly Bears and Hill Giant discarded with Wastes: 2 and 4.
#[test]
fn each_nonland_card_discarded_this_way_is_that_card_of_its_own_trigger() {
    let text = "Discard three cards. When you discard a nonland card this way, you gain life equal to that card's mana value.";
    let mut t = game(&["Wastes", "Grizzly Bears", "Hill Giant"], &[spell(text)]);
    main_phase(&mut t);
    let discarded:Vec<_>=["Wastes", "Grizzly Bears", "Hill Giant"].into_iter().map(|name|put(&mut t,0,name,Zone::Hand)).collect();
    let parent=put(&mut t,0,"Parent",Zone::Hand);
    t.g.act(0,Move::Cast{card:parent,targets:vec![]}).unwrap();
    t.g.act(0,Move::Pass).unwrap();t.g.act(1,Move::Pass).unwrap();
    cite!(["404.3"]; ok: t.g.act(0,Move::Choose(Answer::Objects(discarded))));
    // Exactly three cards remain in hand: discarding all three needs no selection.
    assert!(t.g.players[0].hand.is_empty());
    let Some((0,Asked::Order(abilities)))=t.g.asked() else {panic!("expected reflexive trigger ordering: {:?}",t.g.asked())};
    cite!(["603.3b","603.12a"]; assert_eq!(abilities.len(),2));
    // Both independent discard subjects are ordered by their controller; either
    // order gains 2+4 life, and neither occurrence is merged into the other.
    t.g.act(0,Move::Order(vec![0,1])).unwrap();
    run(&mut t,true,&[]);
    cite!(["603.12a"]; assert_eq!(t.g.players[0].life, 26, "2 for the Bears, 4 for the Giant, none for the Wastes"));
}
