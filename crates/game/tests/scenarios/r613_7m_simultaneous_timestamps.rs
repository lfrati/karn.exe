//! 613.7m: "If two or more objects would receive a timestamp simultaneously, such as by
//! entering a zone simultaneously or becoming attached simultaneously, their relative
//! timestamps are determined in APNAP order (see rule 101.4). Objects controlled by the active
//! player (or owned by the active player, if they have no controller) have an earlier relative
//! timestamp in the order of that player's choice, followed by each other player in turn
//! order." The engine applies APNAP order and asks no player for the order of their own.
//! Where that choice would decide a result — two effects whose timestamps one player's
//! objects were given at once apply in one layer or sublayer to a common object, and
//! timestamp order (613.7), not dependency (613.8), decides between them — the action is
//! refused as unsupported and the game is left as it was. Effects given their timestamps at
//! once that commute, that apply in different sublayers, that change nothing in common, that
//! dependency orders, or whose order APNAP order fixes are applied as usual. Each card is a
//! test text read by the compiler: "Return all enchantment cards from your graveyard to the
//! battlefield" puts every enchantment there onto the battlefield at once (613.7d), and so do
//! the returns "from all graveyards", each card under its owner's control. Timestamps are
//! also given at once to Equipment and Auras attached together (613.7e), to permanents
//! transforming together (613.7g) or turned face up or face down together (613.7f), to the
//! counters one instruction puts on (613.7c) and to the emblems one instruction gives (114.2).
use crate::support::cite;
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const BEAR: usize = 0;
const GIANT: usize = 1;
const SHRINK: usize = 2;
const ANTHEM: usize = 3;
const RALLY: usize = 4;
const WINGS: usize = 5;
const AIRBORNE: usize = 6;
const STRIP: usize = 7;
const MINE: usize = 8;
const THEIRS: usize = 9;
const REVIVAL: usize = 10;
/// Seven setters of base power and toughness, 1/1 to 7/7.
const SETTERS: [usize; 7] = [11, 12, 13, 14, 15, 16, 17];
const HOMECOMING: usize = 18;
const REUNION: usize = 19;
const GRANTER: usize = 20;
const GIANT_GEAR: usize = 21;
const SHRINK_GEAR: usize = 22;
const BOOST_GEAR: usize = 23;
const ARMORY: usize = 24;
const COLOSSUS: usize = 25;
const MITE: usize = 26;
const CHEER: usize = 27;
const UNVEIL: usize = 28;
const CONCEAL: usize = 29;
const GIANT_GIFT: usize = 30;
const SHRINK_GIFT: usize = 31;
const CHEER_GIFT: usize = 32;
const FEATHERS: usize = 33;
const FLIER: usize = 34;
const GROUNDED: usize = 35;
const EMBLEMS: usize = 36;
const MONSTER: usize = 37;
const CURSE: usize = 38;
const BOON: usize = 39;
const GATHER: usize = 40;
const ANNEX: usize = 41;
const FLIP: usize = 42;
/// Double-faced enchantments with no front text, whose back faces set base power and
/// toughness 3/3, 1/1 or give +1/+1.
const GIANT_BACK: usize = 43;
const SHRINK_BACK: usize = 44;
const CHEER_BACK: usize = 45;

const REFUSED: &str = "which the players choose in APNAP order (613.7m)";
const TOO_MANY: &str = "too many effects in one layer have timestamps given at once";
const ROLES: &str = "Roles with timestamps given at once: which is the most recent is chosen in APNAP order (613.7m, 303.7a)";

fn game() -> Game {
    let setters = ["Ace", "Duo", "Trio", "Quad", "Penta", "Hexa", "Hepta"].into_iter().zip(1..).map(|(name, n)| {
        CardDef::new(name, "{0}", "Enchantment", &format!("Creatures have base power and toughness {n}/{n}."), None).unwrap()
    });
    let defs: Vec<CardDef> = [
        ("Bear", "Creature", "", Some((2, 2))),
        ("Giant", "Enchantment", "Creatures have base power and toughness 3/3.", None),
        ("Shrink", "Enchantment", "Creatures have base power and toughness 1/1.", None),
        ("Anthem", "Enchantment", "Creatures get +1/+1.", None),
        ("Rally", "Enchantment", "Creatures get +2/+2.", None),
        ("Wings", "Enchantment", "Creatures have flying.", None),
        ("Airborne", "Enchantment", "Creatures with flying have vigilance.", None),
        ("Strip", "Enchantment", "All creatures lose all abilities.", None),
        ("Mine", "Enchantment", "Creatures you control have base power and toughness 3/3.", None),
        ("Theirs", "Enchantment", "Creatures your opponents control have base power and toughness 1/1.", None),
        ("Revival", "Sorcery", "Return all enchantment cards from your graveyard to the battlefield.", None),
    ].into_iter().map(|(name, types, text, pt)| CardDef::new(name, "{0}", types, text, pt).unwrap()).chain(setters).chain([
        ("Homecoming", "Sorcery", "Return all enchantment cards from all graveyards to the battlefield under their owners' control.", None),
        ("Reunion", "Sorcery", "Return all creature and enchantment cards from all graveyards to the battlefield under their owners' control.", None),
        ("Granter", "Enchantment", "Creatures have \"This creature has base power and toughness 4/4.\"", None),
        ("Giant Gear", "Artifact — Equipment", "Equipped creature has base power and toughness 3/3.\nEquip {0}", None),
        ("Shrink Gear", "Artifact — Equipment", "Equipped creature has base power and toughness 1/1.\nEquip {0}", None),
        ("Boost Gear", "Artifact — Equipment", "Equipped creature gets +1/+1.\nEquip {0}", None),
        ("Armory", "Sorcery", "Attach all Equipment you control to target creature you control.", None),
        ("Colossus", "Creature", "Creatures have base power and toughness 3/3.", Some((2, 2))),
        ("Mite", "Creature", "Creatures have base power and toughness 1/1.", Some((2, 2))),
        ("Cheer", "Creature", "Creatures get +1/+1.", Some((2, 2))),
        ("Unveil", "Sorcery", "Turn all face-down creatures you control face up.", None),
        ("Conceal", "Sorcery", "Turn all creatures you control face down.", None),
        ("Giant Gift", "Artifact — Equipment", "Equipped creature has \"Creatures have base power and toughness 3/3.\"\nEquip {0}", None),
        ("Shrink Gift", "Artifact — Equipment", "Equipped creature has \"Creatures have base power and toughness 1/1.\"\nEquip {0}", None),
        ("Cheer Gift", "Artifact — Equipment", "Equipped creature has \"Creatures get +1/+1.\"\nEquip {0}", None),
        ("Feathers", "Sorcery", "Put a flying counter and a lifelink counter on target creature.", None),
        ("Flier", "Creature", "This creature enters with a flying counter on it.", Some((2, 2))),
        ("Grounded", "Enchantment", "Creatures you control lose flying.", None),
        ("Insignia", "Sorcery", "Each player gets an emblem with \"Creatures have base power and toughness 1/1.\"", None),
        ("Rage", "Sorcery", "Create a Monster Role token attached to target creature.", None),
        ("Hex", "Sorcery", "Create a Cursed Role token attached to target creature.", None),
        ("Boon", "Enchantment — Aura", "Enchant creature\nEnchanted creature gets +1/+1.", None),
        ("Gather", "Sorcery", "Attach all Auras you control to target creature.", None),
        ("Annex", "Sorcery", "Return all enchantment cards from all graveyards to the battlefield under your control.", None),
        ("Flip", "Sorcery", "Transform all permanents you control.", None),
    ].into_iter().map(|(name, types, text, pt)| CardDef::new(name, "{0}", types, text, pt).unwrap_or_else(|e| panic!("{name}: {e}")))).chain([
        ("Giant Back", "Creatures have base power and toughness 3/3."),
        ("Shrink Back", "Creatures have base power and toughness 1/1."),
        ("Cheer Back", "Creatures get +1/+1."),
    ].into_iter().map(|(name, back)| CardDef::transforming(
        CardDef::new(&format!("{name} Front"), "{0}", "Enchantment", "", None).unwrap(),
        CardDef::new(name, "", "Enchantment", back, None).unwrap()).unwrap())).collect();
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    Game::new(defs, 2, 20)
}

/// Player 0 casts this spell with these targets; the result of the pass that resolves it, and
/// the game as it was just before that pass.
fn resolved(g: &mut Game, spell: usize, targets: Vec<Vec<Target>>) -> (Result<(), String>, String) {
    let card = g.put(0, spell, Zone::Hand);
    g.act(0, Move::Cast { card, targets }).unwrap();
    g.act(0, Move::Pass).unwrap();
    let before = format!("{g:?}");
    (g.act(1, Move::Pass), before)
}
/// Put these enchantments into player 0's graveyard and cast Revival ([`resolved`]).
fn returned(g: &mut Game, enchantments: &[usize]) -> (Result<(), String>, String) {
    for e in enchantments { g.put(0, *e, Zone::Graveyard); }
    resolved(g, REVIVAL, vec![])
}
fn on(o: ObjId) -> Vec<Vec<Target>> { vec![vec![Target::Object(o)]] }
fn pt(g: &Game, o: ObjId) -> (Option<i32>, Option<i32>) { (g.power(o), g.toughness(o)) }
fn has(g: &Game, o: ObjId, keyword: &str) -> bool { g.seen(o, None).has(ir::Keyword::from_name(keyword).unwrap()) }
fn on_battlefield(g: &Game, card: usize) -> usize { g.battlefield.iter().filter(|o| g.obj(**o).card == Some(card)).count() }

#[test]
fn r613_7m_two_setters_given_timestamps_at_once_are_refused_and_change_nothing() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let (result, before) = returned(&mut g, &[GIANT, SHRINK]);
    // each sets base power and toughness in layer 7b (613.4b); the later timestamp wins
    // (613.7), and which is later is player 0's choice (613.7m)
    cite!(["613.7m", "613.4b"]; refused(REFUSED): result);
    cite!(["613.7m"]; assert_eq!(format!("{g:?}"), before));
    assert_eq!((g.stack.len(), g.players[0].graveyard.len()), (1, 2));
    assert_eq!(pt(&g, bear), (Some(2), Some(2)));
}

#[test]
fn r613_7_the_same_setters_entering_one_after_the_other_apply_in_timestamp_order() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    returned(&mut g, &[SHRINK]).0.unwrap();
    cite!(["613.4b"]; assert_eq!(pt(&g, bear), (Some(1), Some(1))));
    // Giant enters later, with the later timestamp: its 3/3 is applied last (613.7, 613.7d)
    returned(&mut g, &[GIANT]).0.unwrap();
    cite!(["613.7", "613.7d"]; assert_eq!(pt(&g, bear), (Some(3), Some(3))));
}

#[test]
fn r613_7m_with_no_common_object_the_order_decides_nothing_until_one_arrives() {
    let mut g = game();
    // no creature: neither effect applies to anything (613.7m changes no result)
    returned(&mut g, &[GIANT, SHRINK]).0.unwrap();
    assert_eq!((on_battlefield(&g, GIANT), on_battlefield(&g, SHRINK)), (1, 1));
    // a creature arriving later is one both apply to: its power is the players' choice of
    // their order, which the engine does not ask, so its arrival is refused
    let bear = g.put(0, BEAR, Zone::Hand);
    g.act(0, Move::Cast { card: bear, targets: vec![] }).unwrap();
    g.act(0, Move::Pass).unwrap();
    let before = format!("{g:?}");
    cite!(["613.7m", "613.4b"]; refused(REFUSED): g.act(1, Move::Pass));
    assert_eq!(format!("{g:?}"), before);
}

#[test]
fn r613_7m_setters_given_timestamps_at_once_on_different_objects_need_no_order() {
    let mut g = game();
    let mine = g.put(0, BEAR, Zone::Battlefield);
    let theirs = g.put(1, BEAR, Zone::Battlefield);
    returned(&mut g, &[MINE, THEIRS]).0.unwrap();
    // each applies to creatures the other does not (613.4b): no common object
    cite!(["613.4b", "613.7m"]; assert_eq!((pt(&g, mine), pt(&g, theirs)), ((Some(3), Some(3)), (Some(1), Some(1)))));
}

#[test]
fn r613_4c_modifications_given_timestamps_at_once_commute() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    returned(&mut g, &[ANTHEM, RALLY]).0.unwrap();
    // +1/+1 and +2/+2 in layer 7c: 5/5 in either order (613.4c)
    cite!(["613.4c", "613.7m"]; assert_eq!(pt(&g, bear), (Some(5), Some(5))));
}

#[test]
fn r613_4_effects_of_different_sublayers_are_not_ordered_by_their_timestamps() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    returned(&mut g, &[ANTHEM, SHRINK]).0.unwrap();
    // 1/1 in layer 7b, then +1/+1 in layer 7c, whatever their timestamps (613.4b-c)
    cite!(["613.4b", "613.4c"]; assert_eq!(pt(&g, bear), (Some(2), Some(2))));
}

#[test]
fn r613_8_dependency_not_timestamp_orders_effects_given_timestamps_at_once() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    returned(&mut g, &[AIRBORNE, WINGS]).0.unwrap();
    // Airborne depends on Wings, which changes what it applies to (613.8a): Wings applies
    // first in either timestamp order (613.8b)
    cite!(["613.8a", "613.8b"]; assert!(has(&g, bear, "Flying")));
    cite!(["613.8a", "613.8b"]; assert!(has(&g, bear, "Vigilance")));
}

#[test]
fn r613_7m_adding_and_removing_an_ability_at_once_is_refused() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let (result, before) = returned(&mut g, &[WINGS, STRIP]);
    // in layer 6 the later of "have flying" and "lose all abilities" decides whether the
    // Bear has flying (613.1f, 613.7); neither depends on the other (613.8a)
    cite!(["613.7m", "613.1f"]; refused(REFUSED): result);
    assert_eq!(format!("{g:?}"), before);
    assert!(!has(&g, bear, "Flying"));
}

#[test]
fn r613_7m_six_setters_at_once_are_tried_in_every_order_and_refused() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    // the last of the six to get its timestamp sets the Bear's base power and toughness
    // (613.4b, 613.7): player 0 chooses which (613.7m)
    let (result, before) = returned(&mut g, &SETTERS[..6]);
    cite!(["613.7m", "613.4b"]; refused(REFUSED): result);
    assert_eq!(format!("{g:?}"), before);
    assert_eq!(pt(&g, bear), (Some(2), Some(2)));
}

#[test]
fn r613_7m_seven_setters_at_once_are_too_many_orders_to_try_and_are_refused() {
    let mut g = game();
    g.put(0, BEAR, Zone::Battlefield);
    // 7! = 5040 orders: refused without trying them, the game left as it was
    let (result, before) = returned(&mut g, &SETTERS);
    cite!(["613.7m"]; refused(TOO_MANY): result);
    assert_eq!(format!("{g:?}"), before);
}

#[test]
fn r613_4c_seven_modifications_at_once_commute_without_trying_their_orders() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    // seven +1/+1 effects in layer 7c: 9/9 in any of their 5040 orders (613.4c)
    returned(&mut g, &[ANTHEM; 7]).0.unwrap();
    cite!(["613.4c", "613.7m"]; assert_eq!(pt(&g, bear), (Some(9), Some(9))));
}

#[test]
fn r613_7m_equal_setters_at_once_leave_the_same_result_in_either_order() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    // two copies of one 3/3 setter: whichever is later, the Bear is 3/3 (613.4b)
    returned(&mut g, &[GIANT, GIANT]).0.unwrap();
    cite!(["613.4b", "613.7m"]; assert_eq!(pt(&g, bear), (Some(3), Some(3))));
}

/// Put each `(owner, card)` into its owner's graveyard and have the active player cast this
/// spell, which returns cards of every graveyard at once; the result of the pass that resolves
/// it, and the game as it was just before that pass.
fn all_returned(g: &mut Game, spell: usize, cards: &[(usize, usize)]) -> (Result<(), String>, String) {
    for (owner, card) in cards { g.put(*owner, *card, Zone::Graveyard); }
    let (active, other) = (g.active, 1 - g.active);
    let card = g.put(active, spell, Zone::Hand);
    g.act(active, Move::Cast { card, targets: vec![] }).unwrap();
    g.act(active, Move::Pass).unwrap();
    let before = format!("{g:?}");
    (g.act(other, Move::Pass), before)
}
fn with_active(active: usize) -> Game {
    let mut g = game();
    g.active = active;
    g.priority = active;
    g
}
fn bear_of(g: &Game, owner: usize) -> ObjId {
    g.battlefield.iter().copied().find(|o| g.obj(*o).card == Some(BEAR) && g.obj(*o).owner == owner).expect("a Bear")
}

#[test]
fn r613_7m_the_active_players_objects_have_the_earlier_timestamps() {
    // player 0 is active: its Giant (3/3) has the earlier timestamp, player 1's Shrink (1/1)
    // the later, which is applied last in layer 7b (613.7m, 613.7, 613.4b)
    let mut g = with_active(0);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    all_returned(&mut g, HOMECOMING, &[(0, GIANT), (1, SHRINK)]).0.unwrap();
    cite!(["613.7m", "613.4b"]; assert_eq!(pt(&g, bear), (Some(1), Some(1))));
    // player 1 is active: the same two, player 1's Shrink now earlier and player 0's Giant later
    let mut g = with_active(1);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    all_returned(&mut g, HOMECOMING, &[(0, GIANT), (1, SHRINK)]).0.unwrap();
    cite!(["613.7m", "613.4b"]; assert_eq!(pt(&g, bear), (Some(3), Some(3))));
}

#[test]
fn r613_7m_a_players_own_order_is_refused_only_where_it_decides_a_result() {
    for active in [0, 1] {
        let other = 1 - active;
        // the active player orders its Giant and Shrink as it chooses, but the other
        // player's Duo (2/2) is later than both and applied last (613.7m, 613.4b)
        let mut g = with_active(active);
        let bear = g.put(0, BEAR, Zone::Battlefield);
        all_returned(&mut g, HOMECOMING, &[(active, GIANT), (active, SHRINK), (other, SETTERS[1])]).0.unwrap();
        cite!(["613.7m", "613.4b"]; case: active; assert_eq!(pt(&g, bear), (Some(2), Some(2))));
        // the other player's Giant and Shrink are the later two, in the order it chooses:
        // which is applied last is that choice, which the engine does not ask
        let mut g = with_active(active);
        let bear = g.put(0, BEAR, Zone::Battlefield);
        let (result, before) = all_returned(&mut g, HOMECOMING, &[(active, SETTERS[1]), (other, GIANT), (other, SHRINK)]);
        cite!(["613.7m", "613.4b"]; case: active; refused(REFUSED): result);
        assert_eq!(format!("{g:?}"), before);
        assert_eq!(pt(&g, bear), (Some(2), Some(2)));
    }
}

#[test]
fn r613_7a_a_given_static_ability_has_the_later_of_two_timestamps_in_the_players_order() {
    // Granter gives each creature "This creature has base power and toughness 4/4": that
    // effect has the later of the creature's timestamp and Granter's (613.7a). Each case is
    // tried with either player active, so that the engine's own order of entering differs
    // from APNAP order in one of them.
    for active in [0, 1] {
        let other = 1 - active;
        // the other player's Bear is later than the active player's Granter (613.7m), so the
        // Bear's given 4/4 has the Bear's timestamp; the other player orders it and its Duo
        // (2/2) as it chooses, which decides the Bear's base power and toughness (613.4b)
        let mut g = with_active(active);
        let (result, before) = all_returned(&mut g, REUNION, &[(active, GRANTER), (other, BEAR), (other, SETTERS[1])]);
        cite!(["613.7a", "613.7m"]; case: active; refused(REFUSED): result);
        assert_eq!(format!("{g:?}"), before);
        // the active player's Bear and Granter are both earlier than the other player's Duo:
        // whichever is later, the given 4/4 is applied before Duo's 2/2 (613.7a, 613.7m)
        let mut g = with_active(active);
        all_returned(&mut g, REUNION, &[(active, GRANTER), (active, BEAR), (other, SETTERS[1])]).0.unwrap();
        cite!(["613.7a", "613.7m", "613.4b"]; case: active; assert_eq!(pt(&g, bear_of(&g, active)), (Some(2), Some(2))));
        // the other player's Bear and Granter are both later than the active player's Duo:
        // whichever is later, the given 4/4 is applied after Duo's 2/2
        let mut g = with_active(active);
        all_returned(&mut g, REUNION, &[(active, SETTERS[1]), (other, GRANTER), (other, BEAR)]).0.unwrap();
        cite!(["613.7a", "613.7m", "613.4b"]; case: active; assert_eq!(pt(&g, bear_of(&g, other)), (Some(4), Some(4))));
    }
}

#[test]
fn r613_7m_cards_of_both_owners_returned_under_one_controller_are_in_its_order() {
    for owner in [0, 1] {
        // player 0's Annex puts one player's Giant and the other's Shrink onto the battlefield
        // under player 0's control: 613.7m orders objects by their controller, so both are in
        // the order player 0 chooses, and the later sets the Bear's base power and toughness
        let mut g = game();
        let bear = g.put(0, BEAR, Zone::Battlefield);
        g.put(owner, GIANT, Zone::Graveyard);
        g.put(1 - owner, SHRINK, Zone::Graveyard);
        let (result, before) = resolved(&mut g, ANNEX, vec![]);
        cite!(["613.7m", "613.7d", "613.4b"]; case: owner; refused(REFUSED): result);
        assert_eq!(format!("{g:?}"), before);
        assert_eq!(pt(&g, bear), (Some(2), Some(2)));
    }
}

#[test]
fn r613_7e_two_setters_attached_at_once_are_refused_and_change_nothing() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let gear = [g.put(0, GIANT_GEAR, Zone::Battlefield), g.put(0, SHRINK_GEAR, Zone::Battlefield)];
    // Armory attaches both: each Equipment receives a new timestamp as it becomes attached
    // (613.7e, 701.3c), both at once (613.7m), so which of 3/3 and 1/1 is applied last in
    // layer 7b (613.4b, 613.7) is the order player 0 chooses
    let (result, before) = resolved(&mut g, ARMORY, on(bear));
    cite!(["613.7e", "613.7m", "613.4b"]; refused(REFUSED): result);
    assert_eq!(format!("{g:?}"), before);
    assert_eq!(pt(&g, bear), (Some(2), Some(2)));
    assert!(gear.iter().all(|e| g.obj(*e).attached_to.is_none()));
}

#[test]
fn r613_7e_modifications_attached_at_once_commute() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let gear = [g.put(0, BOOST_GEAR, Zone::Battlefield), g.put(0, BOOST_GEAR, Zone::Battlefield)];
    resolved(&mut g, ARMORY, on(bear)).0.unwrap();
    // +1/+1 twice in layer 7c: 4/4 in either order (613.4c)
    cite!(["613.7e", "613.4c"]; assert_eq!(pt(&g, bear), (Some(4), Some(4))));
    cite!(["701.3a"]; assert!(gear.iter().all(|e| g.obj(*e).attached_to == Some(Target::Object(bear)))));
}

#[test]
fn r613_7g_two_setters_transforming_at_once_are_refused_and_change_nothing() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    g.put(0, GIANT_BACK, Zone::Battlefield);
    g.put(0, SHRINK_BACK, Zone::Battlefield);
    // both enter front face up (712.14) with no abilities; Flip transforms them, and each
    // receives a new timestamp as it does (613.7g), both at once (613.7m): the later of their
    // back faces' 3/3 and 1/1 is player 0's choice. The Bear has no other face and does not
    // transform (701.27c).
    let (result, before) = resolved(&mut g, FLIP, vec![]);
    cite!(["613.7g", "613.7m", "613.4b"]; refused(REFUSED): result);
    assert_eq!(format!("{g:?}"), before);
    assert_eq!(pt(&g, bear), (Some(2), Some(2)));
}

#[test]
fn r613_7g_modifications_transforming_at_once_commute() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    g.put(0, CHEER_BACK, Zone::Battlefield);
    g.put(0, CHEER_BACK, Zone::Battlefield);
    resolved(&mut g, FLIP, vec![]).0.unwrap();
    // their back faces each give +1/+1 in layer 7c: 4/4 in either order (613.4c)
    cite!(["613.7g", "613.4c"]; assert_eq!(pt(&g, bear), (Some(4), Some(4))));
}

/// Put these creature cards onto player 0's battlefield face down, as an effect that lists no
/// characteristics would turn them (708.2a).
fn face_down(g: &mut Game, cards: &[usize]) {
    for card in cards {
        let o = g.put(0, *card, Zone::Battlefield);
        assert!(g.turn_face_down(o));
    }
}

#[test]
fn r613_7f_two_setters_turned_face_up_at_once_are_refused_and_change_nothing() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    face_down(&mut g, &[COLOSSUS, MITE]);
    // face down, neither has an ability (708.2a)
    cite!(["708.2a"]; assert_eq!(pt(&g, bear), (Some(2), Some(2))));
    // Unveil turns both face up, and each receives a new timestamp as it does (613.7f), both
    // at once (613.7m): the later of 3/3 and 1/1 is player 0's choice
    let (result, before) = resolved(&mut g, UNVEIL, vec![]);
    cite!(["613.7f", "613.7m", "613.4b"]; refused(REFUSED): result);
    assert_eq!(format!("{g:?}"), before);
    assert_eq!(pt(&g, bear), (Some(2), Some(2)));
}

#[test]
fn r613_7f_modifications_turned_face_up_at_once_commute() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    face_down(&mut g, &[CHEER, CHEER]);
    resolved(&mut g, UNVEIL, vec![]).0.unwrap();
    // each gives +1/+1 in layer 7c: 4/4 in either order (613.4c)
    cite!(["613.7f", "613.4c"]; assert_eq!(pt(&g, bear), (Some(4), Some(4))));
}

/// Two of player 0's Bears, each equipped with one of these Equipment, the second attached
/// later, and player 1's Bear: the two equipped Bears and player 1's.
fn equipped(g: &mut Game, gifts: [usize; 2]) -> ([ObjId; 2], ObjId) {
    let bears = [g.put(0, BEAR, Zone::Battlefield), g.put(0, BEAR, Zone::Battlefield)];
    let theirs = g.put(1, BEAR, Zone::Battlefield);
    for (gift, bear) in gifts.into_iter().zip(bears) {
        let gift = g.put(0, gift, Zone::Battlefield);
        assert!(g.attach(gift, Target::Object(bear)));
    }
    (bears, theirs)
}

#[test]
fn r613_7f_abilities_given_to_permanents_turned_face_down_at_once_are_in_their_order() {
    let mut g = game();
    let (_, theirs) = equipped(&mut g, [GIANT_GIFT, SHRINK_GIFT]);
    // an effect of a static ability that an Equipment gives has the later of the equipped
    // creature's timestamp and the Equipment's (613.7a): the Equipment's, as each became
    // attached after its creature entered (613.7e). Shrink Gift's is the later: 1/1 (613.4b).
    cite!(["613.7a", "613.7e", "613.4b"]; assert_eq!(pt(&g, theirs), (Some(1), Some(1))));
    // Conceal turns player 0's Bears face down; each receives a new timestamp as it does
    // (613.7f), both at once (613.7m), later than either Equipment's. A face-down permanent
    // keeps what other effects give it (708.2): the given abilities' effects now have the
    // Bears' timestamps (613.7a), in the order player 0 chooses, which decides which of 3/3
    // and 1/1 is applied last.
    let (result, before) = resolved(&mut g, CONCEAL, vec![]);
    cite!(["613.7f", "613.7a", "613.7m"]; refused(REFUSED): result);
    assert_eq!(format!("{g:?}"), before);
    assert_eq!(pt(&g, theirs), (Some(1), Some(1)));
}

#[test]
fn r613_7f_modifications_given_to_permanents_turned_face_down_at_once_commute() {
    let mut g = game();
    let (bears, theirs) = equipped(&mut g, [CHEER_GIFT, CHEER_GIFT]);
    resolved(&mut g, CONCEAL, vec![]).0.unwrap();
    cite!(["708.2a"]; assert!(bears.iter().all(|b| g.face_down(*b))));
    // +1/+1 from each given ability in layer 7c: 4/4 in either order (613.4c)
    cite!(["613.7f", "613.7a", "613.4c"]; assert_eq!(pt(&g, theirs), (Some(4), Some(4))));
}

#[test]
fn r613_7c_keyword_counters_put_on_at_once_only_add_to_what_the_creature_has() {
    // Every counter's effect gives a keyword (122.1b) or changes power and toughness in layer
    // 7c (613.4c): what the counters one instruction puts on do is the same in any order of
    // their timestamps (613.7c, 613.7m), so the order of counters alone decides nothing.
    let mut g = game();
    g.put(0, STRIP, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    resolved(&mut g, FEATHERS, on(bear)).0.unwrap();
    // both counters are later than Strip (613.7c): it takes the Bear's abilities first, then
    // they give it flying and lifelink (613.1f)
    cite!(["613.7c", "122.1b", "613.1f"]; assert!(has(&g, bear, "Flying")));
    cite!(["613.7c", "122.1b", "613.1f"]; assert!(has(&g, bear, "Lifelink")));
}

#[test]
fn r613_7c_a_counter_put_on_as_its_creature_enters_with_an_ability_remover_is_refused() {
    // Flier enters with a flying counter (614.1c, 122.6), put on as it and Grounded enter at
    // once (613.7c, 613.7d), all of it player 0's: whether "lose flying" or the counter's
    // flying is applied last in layer 6 (613.1f) is the order player 0 chooses (613.7m)
    let mut g = with_active(0);
    let (result, before) = all_returned(&mut g, REUNION, &[(0, FLIER), (0, GROUNDED)]);
    cite!(["613.7c", "613.7m", "613.1f"]; refused(REFUSED): result);
    assert_eq!(format!("{g:?}"), before);
    assert_eq!(g.players[0].graveyard.len(), 2);
}

#[test]
fn r114_2_emblems_one_instruction_gives_are_ordered_by_apnap_order_alone() {
    // "Each player gets an emblem" gives each player one emblem, with the same ability (114.2):
    // no player has two emblems to order, and each player's is in APNAP order (613.7m). Both
    // set 1/1 in layer 7b: the Bears are 1/1 whichever is applied last (613.4b).
    for active in [0, 1] {
        let mut g = with_active(active);
        let bears = [g.put(0, BEAR, Zone::Battlefield), g.put(1, BEAR, Zone::Battlefield)];
        let card = g.put(active, EMBLEMS, Zone::Hand);
        g.act(active, Move::Cast { card, targets: vec![] }).unwrap();
        g.act(active, Move::Pass).unwrap();
        g.act(1 - active, Move::Pass).unwrap();
        cite!(["114.2"]; case: active; assert_eq!(g.command.iter().filter(|o| g.obj(**o).emblem()).count(), 2));
        cite!(["114.2", "613.7m", "613.4b"]; case: active; assert!(bears.iter().all(|b| pt(&g, *b) == (Some(1), Some(1)))));
    }
}

fn attached(g: &Game, to: ObjId) -> Vec<ObjId> {
    g.battlefield.iter().copied().filter(|o| g.obj(*o).attached_to == Some(Target::Object(to))).collect()
}

#[test]
fn r303_7a_two_roles_of_one_player_attached_at_once_are_refused_and_change_nothing() {
    let mut g = game();
    let [x, y, z] = [0; 3].map(|_| g.put(0, BEAR, Zone::Battlefield));
    resolved(&mut g, MONSTER, on(x)).0.unwrap();
    resolved(&mut g, CURSE, on(y)).0.unwrap();
    let roles = [attached(&g, x), attached(&g, y)].concat();
    assert_eq!(roles.len(), 2);
    // Gather attaches both Roles to z, each with a new timestamp (613.7e), both at once
    // (613.7m). Of player 0's Roles on z all but the most recent go to the graveyard (303.7a,
    // 704.5z): which is the most recent is the order player 0 chooses.
    let (result, before) = resolved(&mut g, GATHER, on(z));
    cite!(["303.7a", "704.5z", "613.7e", "613.7m"]; refused(ROLES): result);
    assert_eq!(format!("{g:?}"), before);
    assert_eq!((attached(&g, x), attached(&g, y), attached(&g, z)), (vec![roles[0]], vec![roles[1]], vec![]));
}

#[test]
fn r303_7a_a_role_attached_at_once_with_an_aura_that_is_not_one_stays() {
    let mut g = game();
    let [x, y, z] = [0; 3].map(|_| g.put(0, BEAR, Zone::Battlefield));
    resolved(&mut g, MONSTER, on(x)).0.unwrap();
    resolved(&mut g, BOON, on(y)).0.unwrap();
    resolved(&mut g, GATHER, on(z)).0.unwrap();
    // one Role of player 0 on z: 303.7a puts none of them into the graveyard. Monster gives
    // +1/+1 and trample (111.10k), Boon +1/+1: 4/4 (613.4c).
    cite!(["303.7a", "613.7e"]; assert_eq!(attached(&g, z).len(), 2));
    cite!(["111.10k", "613.4c"]; assert_eq!(pt(&g, z), (Some(4), Some(4))));
    cite!(["111.10k"]; assert!(has(&g, z, "Trample")));
}
