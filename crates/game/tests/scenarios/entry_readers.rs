//! Entry choices (614.12a) composed with library entry, costs, mana, triggers and restrictions.
//! Every card is the printed Oracle text of a real card the engine admits.
//!
//! 614.12a: "If a replacement effect that modifies how a permanent enters the battlefield
//! requires a choice, that choice is made before the permanent enters the battlefield."
//! 110.2: "A permanent's controller is, by default, the player under whose control it entered
//! the battlefield."
//! 603.10a: "Some zone-change triggers look back in time. These are leaves-the-battlefield
//! abilities, ..."
//! 704.5f: "If a creature has toughness 0 or less, it's put into its owner's graveyard."
//! 106.6: "Some spells or abilities that produce mana restrict how that mana can be spent"
//! 508.1c: "If any restrictions are being disobeyed, the declaration of attackers is illegal."
//! 702.18a: "Shroud" means "This permanent or player can't be the target of spells or abilities."
//! 502.3: "Normally, all of a player's permanents untap, but effects can keep one or more of a
//! player's permanents from untapping."
//! 601.2h: "Unpayable costs can't be paid."

use crate::support::{cite, mana_pool, paid};
use game::binding::{Kind, Value};
use game::legal::Asked;
use game::state::{Answer, Choice, Decision, PublicChoice};
use game::{CardDef, Event, Game, Move, ObjId, PlayerId, Target, Zone};
use ir::mana::ManaSymbol;
use ir::{Color, Subtype};

const TINKER: usize = 0;
const MEMNITE: usize = 1;
const DOOM_CANNON: usize = 2;
const REANIMATE: usize = 3;
const PLAGUE_ENGINEER: usize = 4;
const RAIDERS: usize = 5;
const BEARS: usize = 6;
const LIGHT_REALMS: usize = 7;
const SPECIALIST: usize = 8;
const DIAMOND_KNIGHT: usize = 9;
const PLAINS: usize = 10;
const ETCHINGS: usize = 11;
const PILLAR: usize = 12;
const BOLT: usize = 13;
const WIZARD: usize = 14;
const DOOR: usize = 15;
const CHRONICLE: usize = 16;
const WRATH: usize = 17;
const ENGINEERED_PLAGUE: usize = 18;
const PIKER: usize = 19;
const MOAT: usize = 20;
const OPEN_THE_VAULTS: usize = 21;
const SHIVAN: usize = 22;
const STEELY: usize = 23;
const RUINS: usize = 24;
const RINGHART: usize = 25;
const TRIUMPHANT: usize = 26;
const RECTOR: usize = 27;
const KINDRED: usize = 28;
const TIGRA: usize = 29;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Tinker", "{2}{U}", "Sorcery", "As an additional cost to cast this spell, sacrifice an artifact.\nSearch your library for an artifact card, put that card onto the battlefield, then shuffle.", None),
        card("Memnite", "{0}", "Artifact Creature — Construct", "", Some((1, 1))),
        card("Doom Cannon", "{6}", "Artifact", "As this artifact enters, choose a creature type.\n{3}, {T}, Sacrifice a creature of the chosen type: This artifact deals 3 damage to any target.", None),
        card("Reanimate", "{B}", "Sorcery", "Put target creature card from a graveyard onto the battlefield under your control. You lose life equal to that card's mana value.", None),
        card("Plague Engineer", "{2}{B}", "Creature — Phyrexian Carrier", "Deathtouch\nAs this creature enters, choose a creature type.\nCreatures of the chosen type your opponents control get -1/-1.", Some((2, 2))),
        card("Mons's Goblin Raiders", "{R}", "Creature — Goblin", "", Some((1, 1))),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Return to the Light Realms", "{7}{W}{W}", "Sorcery", "Return all nonland permanent cards from your graveyard to the battlefield.", None),
        card("Species Specialist", "{2}{B}{B}", "Creature — Human Warrior", "As this creature enters, choose a creature type.\nWhenever a creature of the chosen type dies, you may draw a card.", Some((2, 3))),
        card("Diamond Knight", "{3}", "Artifact Creature — Knight", "Vigilance (Attacking doesn't cause this creature to tap.)\nAs this creature enters, choose a color.\nWhenever you cast a spell of the chosen color, put a +1/+1 counter on this creature.", Some((1, 1))),
        card("Plains", "", "Basic Land — Plains", "({T}: Add {W}.)", None),
        card("Etchings of the Chosen", "{1}{W}{B}", "Enchantment", "As this enchantment enters, choose a creature type.\nCreatures you control of the chosen type get +1/+1.\n{1}, Sacrifice a creature of the chosen type: Target creature you control gains indestructible until end of turn. (Damage and effects that say \"destroy\" don't destroy it.)", None),
        card("Pillar of Origins", "{2}", "Artifact", "As this artifact enters, choose a creature type.\n{T}: Add one mana of any color. Spend this mana only to cast a creature spell of the chosen type.", None),
        card("Lightning Bolt", "{R}", "Instant", "Lightning Bolt deals 3 damage to any target.", None),
        card("Fugitive Wizard", "{U}", "Creature — Human Wizard", "", Some((1, 1))),
        card("Door of Destinies", "{4}", "Artifact", "As this artifact enters, choose a creature type.\nWhenever you cast a spell of the chosen type, put a charge counter on this artifact.\nCreatures you control of the chosen type get +1/+1 for each charge counter on this artifact.", None),
        card("Chronicle of Victory", "{6}", "Legendary Artifact", "As Chronicle of Victory enters, choose a creature type.\nCreatures you control of the chosen type get +2/+2 and have first strike and trample.\nWhenever you cast a spell of the chosen type, draw a card.", None),
        card("Wrath of God", "{2}{W}{W}", "Sorcery", "Destroy all creatures. They can't be regenerated.", None),
        card("Engineered Plague", "{2}{B}", "Enchantment", "As this enchantment enters, choose a creature type.\nAll creatures of the chosen type get -1/-1.", None),
        card("Goblin Piker", "{1}{R}", "Creature — Goblin Warrior", "", Some((2, 1))),
        card("Teferi's Moat", "{3}{W}{U}", "Enchantment", "As this enchantment enters, choose a color.\nCreatures of the chosen color without flying can't attack you.", None),
        card("Open the Vaults", "{4}{W}{W}", "Sorcery", "Return all artifact and enchantment cards from all graveyards to the battlefield under their owners' control. (Auras with nothing to enchant remain in graveyards.)", None),
        card("Shivan Dragon", "{4}{R}{R}", "Creature — Dragon", "Flying\n{R}: This creature gets +1/+0 until end of turn.", Some((5, 5))),
        card("Steely Resolve", "{1}{G}", "Enchantment", "As this enchantment enters, choose a creature type.\nCreatures of the chosen type have shroud. (They can't be the targets of spells or abilities.)", None),
        card("An-Zerrin Ruins", "{2}{R}{R}", "Enchantment", "As this enchantment enters, choose a creature type.\nCreatures of the chosen type don't untap during their controllers' untap steps.", None),
        card("The Ringhart Crest", "{1}{G}", "Legendary Artifact", "As The Ringhart Crest enters, choose a creature type.\n{T}: Add {G}. Spend this mana only to cast a creature spell of the chosen type or a legendary creature spell.", None),
        card("Triumphant Reckoning", "{6}{W}{W}{W}", "Sorcery", "Return all artifact, enchantment, and planeswalker cards from your graveyard to the battlefield.", None),
        card("Academy Rector", "{3}{W}", "Creature — Human Cleric", "When this creature dies, you may exile it. If you do, search your library for an enchantment card, put that card onto the battlefield, then shuffle.", Some((1, 2))),
        card("Kindred Discovery", "{3}{U}{U}", "Enchantment", "As this enchantment enters, choose a creature type.\nWhenever a creature you control of the chosen type enters or attacks, draw a card.", None),
        card("Tigra, Feline Fury", "{1}{G}", "Legendary Creature — Cat Human Hero", "Flash\nTrample\nWhenever you gain life, put a +1/+1 counter on Tigra.", Some((2, 1))),
    ]
}

fn admitted(defs: &[CardDef], cards: &[usize]) {
    for &c in cards {
        game::play::check(&defs[c]).unwrap_or_else(|u| panic!("{} is not admitted: {}", defs[c].name(), u.0));
    }
}

fn game(players: usize, cards: &[usize]) -> Game {
    let ds = defs();
    admitted(&ds, cards);
    Game::new(ds, players, 20)
}

fn ty(name: &str) -> Value {
    Value::CreatureType(Subtype::from_name(name).unwrap_or_else(|| panic!("{name} is a subtype")))
}

fn color(c: Color) -> Value {
    Value::Color(c)
}

const W: ManaSymbol = ManaSymbol::Color(Color::White);
const U: ManaSymbol = ManaSymbol::Color(Color::Blue);
const R: ManaSymbol = ManaSymbol::Color(Color::Red);
const G: ManaSymbol = ManaSymbol::Color(Color::Green);
const B: ManaSymbol = ManaSymbol::Color(Color::Black);
const C: ManaSymbol = ManaSymbol::Colorless;

fn mana(g: &mut Game, player: PlayerId, symbols: &[ManaSymbol]) {
    for s in symbols {
        g.add_mana(player, s.clone());
    }
}

fn last_cast(g: &Game, player: PlayerId) -> ObjId {
    g.log.iter().rev().find_map(|event| match event {
        Event::Cast { spell, by } if *by == player => Some(*spell),
        _ => None,
    }).expect("cast event")
}

fn cast(g: &mut Game, player: PlayerId, def: usize, targets: Vec<Vec<Target>>, spending: &str) -> ObjId {
    let card = g.put(player, def, Zone::Hand);
    g.act(player, paid(spending, Move::Cast { card, targets })).unwrap_or_else(|e| panic!("cast {}: {e}", g.defs[def].name()));
    last_cast(g, player)
}

fn pass_all(g: &mut Game) {
    for _ in 0..g.players.len() {
        let p = g.priority;
        g.act(p, Move::Pass).unwrap_or_else(|e| panic!("player {p} passes: {e}"));
    }
}

fn named_all(g: &Game, name: &str) -> Vec<ObjId> {
    g.battlefield.iter().copied().filter(|o| g.obj(*o).chars.name == name).collect()
}

fn named(g: &Game, name: &str) -> Option<ObjId> {
    match named_all(g, name)[..] {
        [one] => Some(one),
        [] => None,
        _ => panic!("more than one {name}"),
    }
}

fn chosen(g: &Game, permanent: ObjId) -> (PlayerId, Value) {
    let [record] = &g.obj(permanent).choices[..] else {
        panic!("{} has {} choice records", g.obj(permanent).chars.name, g.obj(permanent).choices.len())
    };
    (record.chooser, record.value.clone())
}

/// The pending decision is an entry choice of this kind for this player, offered to no one
/// else, and `card` (not yet on the battlefield) is the object it belongs to.
fn pending(g: &mut Game, chooser: PlayerId, kind: Kind, card: ObjId) {
    assert_eq!(g.decision, Some(Decision::Choose { player: chooser, choice: Choice::Value(kind) }));
    let Some((who, Asked::Value { kind: asked, options })) = g.asked() else { panic!("no value asked: {:?}", g.asked()) };
    assert_eq!((who, asked), (chooser, kind));
    assert_eq!(options, game::binding::options(kind));
    assert_eq!(g.entering_card(), Some(card), "entering_card names the card about to enter");
    cite!(["614.12a"]; assert_ne!(g.obj(card).zone, Zone::Battlefield));
}

/// The objects that moved onto the battlefield from these cards.
fn arrived_from(g: &Game, card: ObjId) -> Option<ObjId> {
    g.log.iter().find_map(|e| match e {
        Event::ZoneChanged { was, now: Some(now), to: Zone::Battlefield, .. } if *was == card => Some(*now),
        _ => None,
    })
}

// ---------------------------------------------------------------------------------------------
// Admission

#[test]
fn every_entry_reader_card_is_admitted() {
    admitted(&defs(), &[
        TINKER, MEMNITE, DOOM_CANNON, REANIMATE, PLAGUE_ENGINEER, RAIDERS, BEARS, LIGHT_REALMS, SPECIALIST,
        DIAMOND_KNIGHT, PLAINS, ETCHINGS, PILLAR, BOLT, WIZARD, DOOR, CHRONICLE, WRATH, ENGINEERED_PLAGUE, PIKER,
        MOAT, OPEN_THE_VAULTS, SHIVAN, STEELY, RUINS, TRIUMPHANT, RINGHART, TIGRA, RECTOR, KINDRED,
    ]);
}

// ---------------------------------------------------------------------------------------------
// A. Other ways onto the battlefield

/// Tinker finds Doom Cannon and puts it onto the battlefield: its controller chooses once,
/// before it enters, while it is still in the library. Also C: the pending state.
#[test]
fn r614_12a_tinker_search_to_battlefield_asks_once_before_entry_and_rejects_bad_answers() {
    let mut g = game(2, &[TINKER, MEMNITE, DOOM_CANNON, PLAINS, RAIDERS]);
    for _ in 0..5 {
        g.put(0, PLAINS, Zone::Library);
    }
    let cannon = g.put(0, DOOM_CANNON, Zone::Library);
    let memnite = g.put(0, MEMNITE, Zone::Battlefield);
    let tinker = g.put(0, TINKER, Zone::Hand);
    mana(&mut g, 0, &[U, C, C]);
    g.act(0, paid("UCC", Move::WithCosts(vec![vec![memnite]], Box::new(Move::Cast { card: tinker, targets: vec![] })))).unwrap();
    let spell = last_cast(&g, 0);
    pass_all(&mut g);
    g.act(0, Move::Choose(Answer::Objects(vec![cannon]))).unwrap();
    pending(&mut g, 0, Kind::CreatureType, cannon);
    assert_eq!(g.obj(cannon).zone, Zone::Library);
    assert_eq!(g.stack, vec![spell]);
    assert_eq!(g.public_choices().count(), 0, "a single entry has no earlier public answers");
    let before = format!("{g:?}");
    crate::support::refused(g.act(1, Move::Choose(Answer::Value(ty("Goblin")))), "that decision is another player's");
    assert_eq!(format!("{g:?}"), before, "a rejected answer by player 1 changed the game");
    for bad in [
        Answer::Value(color(Color::Red)),
        Answer::Value(ty("Forest")),
        Answer::Objects(vec![cannon]),
        Answer::Yes,
    ] {
        cite!(["205.3m"]; case: format!("{bad:?}"); refused("choose a creature type (205.3m)"): g.act(0, Move::Choose(bad.clone())));
        assert_eq!(format!("{g:?}"), before, "a rejected {bad:?} changed the game");
    }
    for p in 0..2 {
        cite!(["614.12"]; refused("a decision is pending: a choice of how the permanent enters the battlefield (614.12)"): g.act(p, Move::Pass));
    }
    g.act(0, Move::Choose(Answer::Value(ty("Goblin")))).unwrap();
    let on = named(&g, "Doom Cannon").expect("Doom Cannon entered");
    assert_eq!(arrived_from(&g, cannon), Some(on));
    cite!(["614.12a"]; assert_eq!(chosen(&g, on), (0, ty("Goblin"))));
    assert_eq!(g.obj(on).controller, 0);
    assert_eq!(g.decision, None, "asked exactly once");
    assert!(g.log.contains(&Event::Resolved { spell }));
}

/// Reanimate, cast by player 0, takes player 1's Plague Engineer from player 1's graveyard:
/// player 0, its controller as it enters, chooses (614.12a, 110.2), and "your opponents" is
/// player 0's opponents.
#[test]
fn r614_12a_reanimate_from_an_opponents_graveyard_the_new_controller_chooses() {
    let mut g = game(3, &[REANIMATE, PLAGUE_ENGINEER, RAIDERS]);
    let engineer = g.put(1, PLAGUE_ENGINEER, Zone::Graveyard);
    let mine = g.put(0, RAIDERS, Zone::Battlefield);
    let owners = g.put(1, RAIDERS, Zone::Battlefield);
    let third = g.put(2, RAIDERS, Zone::Battlefield);
    mana(&mut g, 0, &[B]);
    let spell = cast(&mut g, 0, REANIMATE, vec![vec![Target::Object(engineer)]], "B");
    pass_all(&mut g);
    pending(&mut g, 0, Kind::CreatureType, engineer);
    assert_eq!(g.obj(engineer).zone, Zone::Graveyard);
    // the owner does not choose
    crate::support::refused(g.act(1, Move::Choose(Answer::Value(ty("Goblin")))), "that decision is another player's");
    g.act(0, Move::Choose(Answer::Value(ty("Goblin")))).unwrap();
    let on = named(&g, "Plague Engineer").expect("entered");
    cite!(["110.2a"]; assert_eq!(g.obj(on).controller, 0));
    assert_eq!(g.obj(on).owner, 1);
    cite!(["614.12a", "110.2"]; assert_eq!(chosen(&g, on), (0, ty("Goblin"))));
    assert!(g.log.contains(&Event::Resolved { spell }));
    assert_eq!(g.players[0].life, 17, "mana value 3");
    // the -1/-1 applies to player 0's opponents' Goblins: 1/1s die (704.5f); player 0's lives
    assert_eq!(g.obj(mine).zone, Zone::Battlefield);
    assert_eq!(g.power(mine), Some(1));
    cite!(["704.5f"]; assert_ne!(g.obj(owners).zone, Zone::Battlefield, "the owner's Goblin is an opponent's"));
    assert_ne!(g.obj(third).zone, Zone::Battlefield);
    assert_eq!(g.decision, None);
}

/// A2 via a triggered ability: Academy Rector dies, its controller exiles it and searches for
/// Engineered Plague, which asks its type once before entering, and SBA follow (704.5f).
#[test]
fn r614_12a_academy_rector_trigger_search_puts_engineered_plague_after_its_choice() {
    let mut g = game(2, &[RECTOR, ENGINEERED_PLAGUE, BOLT, PLAINS, RAIDERS]);
    for _ in 0..3 {
        g.put(0, PLAINS, Zone::Library);
    }
    let plague = g.put(0, ENGINEERED_PLAGUE, Zone::Library);
    let rector = g.put(0, RECTOR, Zone::Battlefield);
    let goblin = g.put(1, RAIDERS, Zone::Battlefield);
    mana(&mut g, 0, &[R]);
    cast(&mut g, 0, BOLT, vec![vec![Target::Object(rector)]], "R");
    pass_all(&mut g);
    cite!(["603.10a"]; assert_eq!(g.stack.len(), 1, "the Rector's dies trigger"));
    pass_all(&mut g);
    cite!(["603.5"]; assert!(matches!(g.asked(), Some((0, Asked::May))), "{:?}", g.asked()));
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    g.act(0, Move::Choose(Answer::Objects(vec![plague]))).unwrap();
    pending(&mut g, 0, Kind::CreatureType, plague);
    assert_eq!(g.obj(plague).zone, Zone::Library);
    assert_eq!(g.obj(goblin).zone, Zone::Battlefield);
    crate::support::refused(g.act(1, Move::Choose(Answer::Value(ty("Goblin")))), "that decision is another player's");
    g.act(0, Move::Choose(Answer::Value(ty("Goblin")))).unwrap();
    let on = arrived_from(&g, plague).expect("entered");
    assert_eq!(chosen(&g, on), (0, ty("Goblin")));
    assert_eq!(g.decision, None, "asked once");
    cite!(["704.5f"]; assert_ne!(g.obj(goblin).zone, Zone::Battlefield, "the 1/1 Goblin died to SBA"));
}

/// 603.6a with 614.12a: Kindred Discovery and a Goblin enter together (Return to the Light
/// Realms); the choice was made before entering, so the Goblin's entry triggers it.
/// 603.6a: "Each time an event puts one or more permanents onto the battlefield, all permanents
/// on the battlefield (including the newcomers) are checked for any enters-the-battlefield
/// triggers that match the event."
#[test]
fn r603_6a_kindred_discovery_sees_a_goblin_entering_with_it() {
    let mut g = game(2, &[LIGHT_REALMS, KINDRED, RAIDERS, BEARS, PLAINS]);
    for _ in 0..5 {
        g.put(0, PLAINS, Zone::Library);
    }
    g.put(0, KINDRED, Zone::Graveyard);
    g.put(0, RAIDERS, Zone::Graveyard);
    g.put(0, BEARS, Zone::Graveyard);
    mana(&mut g, 0, &[W, W, C, C, C, C, C, C, C]);
    cast(&mut g, 0, LIGHT_REALMS, vec![], "CCCCCCCWW");
    pass_all(&mut g);
    let card = g.entering_card().unwrap();
    pending(&mut g, 0, Kind::CreatureType, card);
    g.act(0, Move::Choose(Answer::Value(ty("Goblin")))).unwrap();
    assert_eq!(g.decision, None);
    cite!(["603.6a", "614.12a"]; assert_eq!(g.stack.len(), 1, "one trigger: the Goblin, not the Bear"));
    let hand = g.players[0].hand.len();
    pass_all(&mut g);
    assert_eq!(g.players[0].hand.len(), hand + 1);
}

/// one mass return by one player: two Species Specialists, a Diamond Knight and a Plague
/// Engineer; each is asked once, each keeps its own answer; public_choices shows the earlier
/// answers (101.4b); nothing enters before the last answer.
#[test]
fn r101_4b_return_to_the_light_realms_each_object_keeps_its_own_answer() {
    let mut g = game(2, &[LIGHT_REALMS, SPECIALIST, DIAMOND_KNIGHT, PLAGUE_ENGINEER, PLAINS]);
    let s1 = g.put(0, SPECIALIST, Zone::Graveyard);
    let s2 = g.put(0, SPECIALIST, Zone::Graveyard);
    let knight = g.put(0, DIAMOND_KNIGHT, Zone::Graveyard);
    let engineer = g.put(0, PLAGUE_ENGINEER, Zone::Graveyard);
    let plains = g.put(0, PLAINS, Zone::Graveyard);
    mana(&mut g, 0, &[W, W, C, C, C, C, C, C, C]);
    let spell = cast(&mut g, 0, LIGHT_REALMS, vec![], "CCCCCCCWW");
    pass_all(&mut g);
    let answer = |card: ObjId| -> Value {
        if card == s1 { ty("Goblin") } else if card == s2 { ty("Bear") } else if card == knight { color(Color::Red) } else if card == engineer { ty("Elf") } else { panic!("{card:?} asked") }
    };
    let mut asked = Vec::new();
    let mut observed = Vec::new();
    while let Some(Decision::Choose { player, choice: Choice::Value(kind) }) = g.decision.clone() {
        assert_eq!(player, 0);
        let card = g.entering_card().expect("entering card");
        pending(&mut g, 0, kind, card);
        for c in [s1, s2, knight, engineer] {
            assert_eq!(g.obj(c).zone, Zone::Graveyard, "nothing entered yet");
        }
        cite!(["101.4b"]; case: format!("{card:?}"); assert_eq!(g.public_choices().cloned().collect::<Vec<_>>(), observed));
        assert!(!asked.contains(&card), "{card:?} asked twice");
        asked.push(card);
        let reply = Answer::Value(answer(card));
        observed.push(PublicChoice::Entry { player: 0, card, answer: reply.clone() });
        g.act(0, Move::Choose(reply)).unwrap();
    }
    asked.sort();
    let mut expected = vec![s1, s2, knight, engineer];
    expected.sort();
    assert_eq!(asked, expected, "each entering object asked exactly once");
    assert!(g.log.contains(&Event::Resolved { spell }));
    for c in [s1, s2, knight, engineer] {
        let on = arrived_from(&g, c).expect("entered");
        cite!(["614.12a"]; case: format!("{c:?}"); assert_eq!(chosen(&g, on), (0, answer(c))));
    }
    assert_eq!(g.obj(plains).zone, Zone::Graveyard, "a land card is not returned");
}

/// A5 variant: Triumphant Reckoning returns two Door of Destinies; each keeps its own type.
#[test]
fn r607_2d_triumphant_reckoning_two_doors_with_different_types() {
    let mut g = game(2, &[TRIUMPHANT, DOOR, RAIDERS, BEARS]);
    let d1 = g.put(0, DOOR, Zone::Graveyard);
    let d2 = g.put(0, DOOR, Zone::Graveyard);
    mana(&mut g, 0, &[W, W, W, C, C, C, C, C, C]);
    cast(&mut g, 0, TRIUMPHANT, vec![], "CCCCCCWWW");
    pass_all(&mut g);
    let mut n = 0;
    while let Some(Decision::Choose { .. }) = g.decision.clone() {
        let card = g.entering_card().unwrap();
        pending(&mut g, 0, Kind::CreatureType, card);
        g.act(0, Move::Choose(Answer::Value(if card == d1 { ty("Goblin") } else { ty("Bear") }))).unwrap();
        n += 1;
    }
    assert_eq!(n, 2);
    let (n1, n2) = (arrived_from(&g, d1).unwrap(), arrived_from(&g, d2).unwrap());
    assert_eq!(chosen(&g, n1).1, ty("Goblin"));
    assert_eq!(chosen(&g, n2).1, ty("Bear"));
    // each Door counts its own spells: a Goblin spell puts a counter on the Goblin Door only
    let raiders_spell = { mana(&mut g, 0, &[R]); cast(&mut g, 0, RAIDERS, vec![], "R") };
    cite!(["607.2d"]; assert_eq!(g.stack.len(), 2, "one Door triggered"));
    pass_all(&mut g);
    assert_eq!(g.obj(n1).counters.get("charge").copied().unwrap_or(0), 1);
    cite!(["607.2d"]; assert_eq!(g.obj(n2).counters.get("charge").copied().unwrap_or(0), 0));
    pass_all(&mut g);
    let raiders = arrived_from_stack(&g, raiders_spell);
    assert_eq!(g.power(raiders), Some(2));
}

fn arrived_from_stack(g: &Game, spell: ObjId) -> ObjId {
    g.log.iter().find_map(|e| match e {
        Event::ZoneChanged { was, now: Some(now), from: Zone::Stack, to: Zone::Battlefield } if *was == spell => Some(*now),
        _ => None,
    }).expect("the spell became a permanent")
}

// ---------------------------------------------------------------------------------------------
// B. Readers of the chosen value

/// Cast a permanent and answer its entry choice; the permanent.
fn cast_choose(g: &mut Game, player: PlayerId, def: usize, cost: &[ManaSymbol], answer: Value, spending: &str) -> ObjId {
    mana(g, player, cost);
    let spell = cast(g, player, def, vec![], spending);
    pass_all(g);
    let kind = match answer { Value::Color(_) => Kind::Color, _ => Kind::CreatureType };
    let card = g.entering_card().expect("asked");
    assert_eq!(card, spell);
    assert_eq!(g.decision, Some(Decision::Choose { player, choice: Choice::Value(kind) }));
    g.act(player, Move::Choose(Answer::Value(answer))).unwrap();
    arrived_from_stack(g, spell)
}

/// Doom Cannon's "Sacrifice a creature of the chosen type": the listing offers only the
/// Goblin; a Bear is refused; the Goblin pays.
#[test]
fn r601_2h_doom_cannon_sacrifice_only_the_chosen_type_and_listing_agrees() {
    let mut g = game(2, &[DOOM_CANNON, RAIDERS, BEARS]);
    let goblin = g.put(0, RAIDERS, Zone::Battlefield);
    let bear = g.put(0, BEARS, Zone::Battlefield);
    let _theirs = g.put(1, RAIDERS, Zone::Battlefield);
    let cannon = cast_choose(&mut g, 0, DOOM_CANNON, &[C, C, C, C, C, C], ty("Goblin"), "CCCCCC");
    mana(&mut g, 0, &[C, C, C]);
    let before = format!("{g:?}");
    let act = |paying: Vec<Vec<ObjId>>| paid("CCC", Move::Activate { source: cannon, ability: 1, targets: vec![vec![Target::Player(1)]], paying });
    cite!(["118.3"]; refused("Grizzly Bears is not what that cost asks for or cannot pay it (701.21a, 118.3)"): g.act(0, act(vec![vec![bear]])));
    cite!(["701.21a"]; refused("Mons's Goblin Raiders is not what that cost asks for or cannot pay it (701.21a, 118.3)"): g.act(0, act(vec![vec![_theirs]])));
    assert_eq!(format!("{g:?}"), before);
    g.act(0, act(vec![vec![goblin]])).unwrap();
    assert_ne!(g.obj(goblin).zone, Zone::Battlefield);
    pass_all(&mut g);
    assert_eq!(g.players[1].life, 17);
}

/// Etchings of the Chosen, same reader on an enchantment, plus its +1/+1 for my Goblins only.
#[test]
fn r607_2d_etchings_of_the_chosen_sacrifice_and_anthem_read_the_chosen_type() {
    let mut g = game(2, &[ETCHINGS, RAIDERS, BEARS]);
    let goblin = g.put(0, RAIDERS, Zone::Battlefield);
    let bear = g.put(0, BEARS, Zone::Battlefield);
    let theirs = g.put(1, RAIDERS, Zone::Battlefield);
    let etchings = cast_choose(&mut g, 0, ETCHINGS, &[W, B, C], ty("Goblin"), "WBC");
    cite!(["607.2d"]; assert_eq!((g.power(goblin), g.power(theirs), g.power(bear)), (Some(2), Some(1), Some(2))));
    mana(&mut g, 0, &[C]);
    let act = |paying: Vec<Vec<ObjId>>| paid("C", Move::Activate { source: etchings, ability: 2, targets: vec![vec![Target::Object(bear)]], paying });
    cite!(["118.3"]; refused("Grizzly Bears is not what that cost asks for or cannot pay it (701.21a, 118.3)"): g.act(0, act(vec![vec![bear]])));
    g.act(0, act(vec![vec![goblin]])).unwrap();
    assert_ne!(g.obj(goblin).zone, Zone::Battlefield);
}

/// Pillar of Origins' mana casts a creature spell of the chosen type, not another spell
/// (106.6); the listing agrees.
#[test]
fn r106_6_pillar_of_origins_mana_only_for_a_creature_spell_of_the_chosen_type() {
    let mut g = game(2, &[PILLAR, RAIDERS, BOLT, WIZARD]);
    let pillar = cast_choose(&mut g, 0, PILLAR, &[C, C], ty("Goblin"), "CC");
    let raiders = g.put(0, RAIDERS, Zone::Hand);
    let bolt = g.put(0, BOLT, Zone::Hand);
    g.put(0, WIZARD, Zone::Hand);
    g.act(0, Move::Adding(vec![vec![R]], Box::new(Move::Activate { source: pillar, ability: 1, targets: vec![], paying: vec![] }))).unwrap();
    let before = format!("{g:?}");
    cite!(["106.6"]; refused("that mana can't be spent on that: its text restricts how it can be spent (106.6)"): g.act(0, Move::Paying(game::state::Pool { restricted: vec![g.players[0].pool.restricted[0].clone()], ..mana_pool("") }, Box::new(Move::Cast { card: bolt, targets: vec![vec![Target::Player(1)]] }))));
    assert_eq!(format!("{g:?}"), before);
    g.act(0, Move::Paying(game::state::Pool { restricted: vec![g.players[0].pool.restricted[0].clone()], ..mana_pool("") }, Box::new(Move::Cast { card: raiders, targets: vec![] }))).unwrap();
}

/// the Wizard with blue Pillar mana is refused too.
#[test]
fn r106_6_pillar_mana_refused_for_a_creature_of_another_type() {
    let mut g = game(2, &[PILLAR, WIZARD]);
    let pillar = cast_choose(&mut g, 0, PILLAR, &[C, C], ty("Goblin"), "CC");
    let wizard = g.put(0, WIZARD, Zone::Hand);
    g.act(0, Move::Adding(vec![vec![U]], Box::new(Move::Activate { source: pillar, ability: 1, targets: vec![], paying: vec![] }))).unwrap();
    cite!(["106.6"]; refused("that mana can't be spent on that: its text restricts how it can be spent (106.6)"): g.act(0, Move::Paying(game::state::Pool { restricted: vec![g.players[0].pool.restricted[0].clone()], ..mana_pool("") }, Box::new(Move::Cast { card: wizard, targets: vec![] }))));
}

/// The Ringhart Crest: its {G} pays for a creature spell of the chosen type or a legendary
/// creature spell, not for another creature spell (106.6).
#[test]
fn r106_6_the_ringhart_crest_mana_for_chosen_type_or_legendary_creature_only() {
    let mut g = game(2, &[RINGHART, BEARS, PIKER, TIGRA]);
    let crest = cast_choose(&mut g, 0, RINGHART, &[G, C], ty("Goblin"), "GC");
    let bears = g.put(0, BEARS, Zone::Hand);
    let tigra = g.put(0, TIGRA, Zone::Hand);
    let piker = g.put(0, PIKER, Zone::Hand);
    g.act(0, Move::Activate { source: crest, ability: 1, targets: vec![], paying: vec![] }).unwrap();
    let tapped = g.clone();
    // Bears: {1}{G}, the {1} from the pool, the {G} only from the Crest
    mana(&mut g, 0, &[C]);
    cite!(["106.6"]; refused("that mana can't be spent on that: its text restricts how it can be spent (106.6)"): g.act(0, Move::Paying(game::state::Pool { restricted: vec![g.players[0].pool.restricted[0].clone()], ..mana_pool("C") }, Box::new(Move::Cast { card: bears, targets: vec![] }))));
    cite!(["106.6"]; ok: g.act(0, Move::Paying(game::state::Pool { restricted: vec![g.players[0].pool.restricted[0].clone()], ..mana_pool("C") }, Box::new(Move::Cast { card: tigra, targets: vec![] }))));
    let mut g = tapped;
    mana(&mut g, 0, &[R]);
    cite!(["106.6"]; ok: g.act(0, Move::Paying(game::state::Pool { restricted: vec![g.players[0].pool.restricted[0].clone()], ..mana_pool("R") }, Box::new(Move::Cast { card: piker, targets: vec![] }))));
}

/// Door of Destinies and Chronicle of Victory trigger for a spell of the chosen type only.
#[test]
fn r603_2_door_and_chronicle_trigger_on_a_spell_of_the_chosen_type_only() {
    let mut g = game(2, &[DOOR, CHRONICLE, RAIDERS, BEARS, BOLT, PLAINS]);
    for _ in 0..5 {
        g.put(0, PLAINS, Zone::Library);
    }
    let door = cast_choose(&mut g, 0, DOOR, &[C, C, C, C], ty("Goblin"), "CCCC");
    let _chronicle = cast_choose(&mut g, 0, CHRONICLE, &[C, C, C, C, C, C], ty("Goblin"), "CCCCCC");
    let hand = g.players[0].hand.len();
    mana(&mut g, 0, &[G, G]);
    let bears = cast(&mut g, 0, BEARS, vec![], "GG");
    cite!(["603.2"]; assert_eq!(g.stack, vec![bears], "a Bear spell triggers neither"));
    pass_all(&mut g);
    mana(&mut g, 0, &[R]);
    cast(&mut g, 0, BOLT, vec![vec![Target::Player(1)]], "R");
    cite!(["603.2"]; assert_eq!(g.stack.len(), 1, "an instant triggers neither"));
    pass_all(&mut g);
    mana(&mut g, 0, &[R]);
    let raiders = cast(&mut g, 0, RAIDERS, vec![], "R");
    // two abilities of one player triggered: they are ordered (603.3b)
    let Some((0, Asked::Order(triggered))) = g.asked() else { panic!("{:?}", g.asked()) };
    cite!(["603.2", "603.3b"]; assert_eq!(triggered.len(), 2, "a Goblin spell triggers both"));
    g.act(0, Move::Order(vec![0, 1])).unwrap();
    assert_eq!(g.stack.len(), 3, "{:?}", g.stack);
    while g.stack.len() > 1 {
        pass_all(&mut g);
    }
    assert_eq!(g.obj(door).counters.get("charge").copied().unwrap_or(0), 1);
    assert_eq!(g.players[0].hand.len(), hand + 1, "Chronicle drew one card");
    pass_all(&mut g);
    let goblin = arrived_from_stack(&g, raiders);
    assert_eq!(g.power(goblin), Some(1 + 1 + 2), "Door +1 for one counter, Chronicle +2");
}

/// Species Specialist: a Goblin dying triggers it, a Bear does not; with Wrath of God,
/// the Specialist dying with a Goblin still sees the Goblin die (603.10a).
#[test]
fn r603_10a_species_specialist_triggers_on_the_chosen_type_including_simultaneous_wrath() {
    let mut g = game(2, &[SPECIALIST, RAIDERS, BEARS, BOLT, WRATH, PLAINS, PIKER]);
    for _ in 0..5 {
        g.put(0, PLAINS, Zone::Library);
    }
    let goblin = g.put(1, RAIDERS, Zone::Battlefield);
    let bear = g.put(1, BEARS, Zone::Battlefield);
    let piker = g.put(1, PIKER, Zone::Battlefield);
    let specialist = cast_choose(&mut g, 0, SPECIALIST, &[B, B, C, C], ty("Goblin"), "BBCC");
    let hand = g.players[0].hand.len();
    mana(&mut g, 0, &[R]);
    cast(&mut g, 0, BOLT, vec![vec![Target::Object(bear)]], "R");
    pass_all(&mut g);
    assert_ne!(g.obj(bear).zone, Zone::Battlefield);
    cite!(["603.2"]; assert!(g.stack.is_empty(), "a Bear dying triggers nothing"));
    mana(&mut g, 0, &[R]);
    cast(&mut g, 0, BOLT, vec![vec![Target::Object(goblin)]], "R");
    pass_all(&mut g);
    cite!(["603.2"]; assert_eq!(g.stack.len(), 1, "the opponent's Goblin died: triggered"));
    pass_all(&mut g);
    assert!(matches!(g.asked(), Some((0, Asked::May))), "{:?}", g.asked());
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    assert_eq!(g.players[0].hand.len(), hand + 1);
    // Wrath: the Specialist and the Piker die together
    mana(&mut g, 0, &[W, W, C, C]);
    cast(&mut g, 0, WRATH, vec![], "CCWW");
    pass_all(&mut g);
    assert_ne!(g.obj(specialist).zone, Zone::Battlefield);
    assert_ne!(g.obj(piker).zone, Zone::Battlefield);
    cite!(["603.10a"]; assert_eq!(g.stack.len(), 1, "603.10a: the Specialist looks back and sees the Goblin die"));
    pass_all(&mut g);
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    assert_eq!(g.players[0].hand.len(), hand + 2);
}

/// Species Specialist naming its own type (Human) sees itself die with another Human in
/// one Wrath: two triggers (603.10a, the leaves-the-battlefield look back).
#[test]
fn r603_10a_species_specialist_sees_itself_die_when_it_named_its_own_type() {
    let mut g = game(2, &[SPECIALIST, WIZARD, WRATH, PLAINS]);
    for _ in 0..5 {
        g.put(0, PLAINS, Zone::Library);
    }
    g.put(1, WIZARD, Zone::Battlefield);
    cast_choose(&mut g, 0, SPECIALIST, &[B, B, C, C], ty("Human"), "BBCC");
    let hand = g.players[0].hand.len();
    mana(&mut g, 0, &[W, W, C, C]);
    cast(&mut g, 0, WRATH, vec![], "CCWW");
    pass_all(&mut g);
    if let Some((0, Asked::Order(t))) = g.asked() {
        assert_eq!(t.len(), 2);
        g.act(0, Move::Order(vec![0, 1])).unwrap();
    }
    cite!(["603.10a"]; assert_eq!(g.stack.len(), 2, "the Specialist and the Wizard, both Humans, died: {:?}", g.asked()));
    for _ in 0..2 {
        pass_all(&mut g);
        g.act(0, Move::Choose(Answer::Yes)).unwrap();
    }
    assert_eq!(g.players[0].hand.len(), hand + 2);
}

/// Engineered Plague: 1/1 Goblins and a 2/1 Goblin die by state-based actions right after
/// it enters (704.5f), on both sides; a Bear is untouched.
#[test]
fn r704_5f_engineered_plague_kills_x_1_goblins_by_state_based_actions() {
    let mut g = game(2, &[ENGINEERED_PLAGUE, RAIDERS, PIKER, BEARS]);
    let mine = g.put(0, RAIDERS, Zone::Battlefield);
    let theirs = g.put(1, PIKER, Zone::Battlefield);
    let bear = g.put(1, BEARS, Zone::Battlefield);
    let plague = cast_choose(&mut g, 0, ENGINEERED_PLAGUE, &[B, C, C], ty("Goblin"), "BCC");
    assert_eq!(g.obj(plague).zone, Zone::Battlefield);
    cite!(["704.5f"]; assert_ne!(g.obj(mine).zone, Zone::Battlefield));
    cite!(["704.5f"]; assert_ne!(g.obj(theirs).zone, Zone::Battlefield));
    assert_eq!(g.players[0].graveyard.len() + g.players[1].graveyard.len(), 2);
    assert_eq!((g.power(bear), g.toughness(bear)), (Some(2), Some(2)));
}

/// Plague Engineer cast normally, three players: both opponents' Goblins shrink, mine not.
#[test]
fn r611_3a_plague_engineer_affects_only_opponents_creatures() {
    let mut g = game(3, &[PLAGUE_ENGINEER, PIKER, RAIDERS]);
    let mine = g.put(0, PIKER, Zone::Battlefield);
    let p1 = g.put(1, PIKER, Zone::Battlefield);
    let p2 = g.put(2, PIKER, Zone::Battlefield);
    let p2small = g.put(2, RAIDERS, Zone::Battlefield);
    let engineer = cast_choose(&mut g, 0, PLAGUE_ENGINEER, &[B, C, C], ty("Goblin"), "BCC");
    cite!(["404.3"]; ok: g.act(2, Move::Choose(Answer::Objects(vec![p2, p2small]))));
    assert_eq!((g.power(mine), g.toughness(mine)), (Some(2), Some(1)));
    // 2/1 Pikers at 1/0 die
    assert_ne!(g.obj(p1).zone, Zone::Battlefield);
    cite!(["704.5f"]; assert_ne!(g.obj(p2).zone, Zone::Battlefield));
    assert_ne!(g.obj(p2small).zone, Zone::Battlefield);
    assert_eq!((g.power(engineer), g.toughness(engineer)), (Some(2), Some(2)));
}

/// Pass to the declare-attackers step of the current turn.
fn to_attackers(g: &mut Game) {
    pass_all(g);
    assert_eq!(g.part, ir::Part::BeginningOfCombat);
    pass_all(g);
    assert_eq!(g.part, ir::Part::DeclareAttackers);
}

/// Teferi's Moat controlled by player 1 (chosen red, by player 1 as it returns with Open
/// the Vaults): player 0's red creature without flying can't attack player 1 but can attack
/// player 2; a red flier and a green creature can attack player 1 (508.1c).
#[test]
fn r508_1c_teferis_moat_restricts_only_nonflying_creatures_of_the_color_attacking_its_controller() {
    let mut g = game(3, &[MOAT, OPEN_THE_VAULTS, RAIDERS, SHIVAN, BEARS]);
    let moat_card = g.put(1, MOAT, Zone::Graveyard);
    let goblin = g.put(0, RAIDERS, Zone::Battlefield);
    let dragon = g.put(0, SHIVAN, Zone::Battlefield);
    let bear = g.put(0, BEARS, Zone::Battlefield);
    mana(&mut g, 0, &[W, W, C, C, C, C]);
    cast(&mut g, 0, OPEN_THE_VAULTS, vec![], "CCCCWW");
    pass_all(&mut g);
    pending(&mut g, 1, Kind::Color, moat_card);
    // the caster is not its controller
    crate::support::refused(g.act(0, Move::Choose(Answer::Value(color(Color::Red)))), "that decision is another player's");
    g.act(1, Move::Choose(Answer::Value(color(Color::Red)))).unwrap();
    let moat = named(&g, "Teferi's Moat").unwrap();
    cite!(["614.12a", "110.2"]; assert_eq!(chosen(&g, moat), (1, color(Color::Red))));
    to_attackers(&mut g);
    let Some((0, Asked::Attackers(able))) = g.asked() else { panic!("{:?}", g.asked()) };
    let attacks = |c: ObjId| able.iter().find(|a| a.creature == c).map(|a| a.attacks.clone()).unwrap_or_default();
    cite!(["508.1c"]; assert!(!attacks(goblin).contains(&Target::Player(1)), "listing: the Goblin can't attack player 1"));
    assert!(attacks(goblin).contains(&Target::Player(2)));
    assert!(attacks(dragon).contains(&Target::Player(1)));
    assert!(attacks(bear).contains(&Target::Player(1)));
    let before = format!("{g:?}");
    cite!(["508.1c"]; refused("Mons's Goblin Raiders can't attack that (508.1c)"): g.act(0, Move::Attack(vec![(goblin, Target::Player(1))])));
    assert_eq!(format!("{g:?}"), before);
    cite!(["508.1c"]; ok: g.act(0, Move::Attack(vec![(goblin, Target::Player(2)), (dragon, Target::Player(1)), (bear, Target::Player(1))])));
}

/// Steely Resolve: a Goblin has shroud, a Bear does not; the listing agrees.
#[test]
fn r702_18a_steely_resolve_shroud_only_for_the_chosen_type() {
    let mut g = game(2, &[STEELY, RAIDERS, BEARS, BOLT]);
    let goblin = g.put(1, RAIDERS, Zone::Battlefield);
    let bear = g.put(1, BEARS, Zone::Battlefield);
    cast_choose(&mut g, 0, STEELY, &[G, C], ty("Goblin"), "GC");
    mana(&mut g, 0, &[R]);
    let bolt = g.put(0, BOLT, Zone::Hand);
    let before = format!("{g:?}");
    cite!(["702.18a"]; refused("an illegal target, Mons's Goblin Raiders: it has shroud (702.18a)"): g.act(0, Move::Cast { card: bolt, targets: vec![vec![Target::Object(goblin)]] }));
    assert_eq!(format!("{g:?}"), before);
    g.act(0, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Object(bear)]] })).unwrap();
}

/// An-Zerrin Ruins: a tapped Goblin stays tapped in its controller's untap step; a tapped
/// Bear untaps (502.3).
#[test]
fn r502_3_an_zerrin_ruins_keeps_only_the_chosen_type_tapped() {
    let mut g = game(2, &[RUINS, RAIDERS, BEARS, PLAINS]);
    for p in 0..2 {
        for _ in 0..5 {
            g.put(p, PLAINS, Zone::Library);
        }
    }
    let goblin = g.put(1, RAIDERS, Zone::Battlefield);
    let bear = g.put(1, BEARS, Zone::Battlefield);
    g.set_tapped(goblin, true);
    g.set_tapped(bear, true);
    cast_choose(&mut g, 0, RUINS, &[R, R, C, C], ty("Goblin"), "RRCC");
    for _ in 0..100 {
        if g.active == 1 && g.part == ir::Part::PrecombatMain {
            break;
        }
        assert_eq!(g.decision, None);
        g.act(g.priority, Move::Pass).unwrap();
    }
    assert_eq!((g.active, g.part), (1, ir::Part::PrecombatMain));
    cite!(["502.3"]; assert!(g.obj(goblin).tapped, "the Goblin does not untap"));
    cite!(["502.3"]; assert!(!g.obj(bear).tapped, "the Bear untaps"));
}

// ---------------------------------------------------------------------------------------------
// C. Pending state for a cast permanent spell, with a trigger-free choice of colour

/// Diamond Knight cast: entering_card is the spell, no public choices, only the caster is
/// offered the colour, wrong-kind answers refused; then its reader works for that colour only.
#[test]
fn r614_12a_diamond_knight_pending_state_and_reader() {
    let mut g = game(2, &[DIAMOND_KNIGHT, BOLT, BEARS]);
    mana(&mut g, 0, &[C, C, C]);
    let spell = cast(&mut g, 0, DIAMOND_KNIGHT, vec![], "CCC");
    pass_all(&mut g);
    pending(&mut g, 0, Kind::Color, spell);
    assert_eq!(g.public_choices().count(), 0);
    let before = format!("{g:?}");
    crate::support::refused(g.act(1, Move::Choose(Answer::Value(color(Color::Red)))), "that decision is another player's");
    assert_eq!(format!("{g:?}"), before);
    for a in [Answer::Value(ty("Goblin")), Answer::No, Answer::Mana(vec![R])] {
        cite!(["105.4"]; case: format!("{a:?}"); refused("choose one of the five colors (105.4)"): g.act(0, Move::Choose(a.clone())));
        assert_eq!(format!("{g:?}"), before);
    }
    g.act(0, Move::Choose(Answer::Value(color(Color::Red)))).unwrap();
    let knight = arrived_from_stack(&g, spell);
    assert_eq!(g.entering_card(), None);
    mana(&mut g, 0, &[G, G]);
    cast(&mut g, 0, BEARS, vec![], "GG");
    cite!(["607.2d"]; assert_eq!(g.stack.len(), 1, "a green spell does not trigger"));
    pass_all(&mut g);
    mana(&mut g, 0, &[R]);
    cast(&mut g, 0, BOLT, vec![vec![Target::Player(1)]], "R");
    cite!(["607.2d"]; assert_eq!(g.stack.len(), 2, "a red spell triggers"));
    pass_all(&mut g);
    assert_eq!(g.power(knight), Some(2));
}
