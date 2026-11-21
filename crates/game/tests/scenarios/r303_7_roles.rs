//! Role tokens (111.10j–r, 303.7): predefined Aura Role enchantment tokens created attached to a
//! creature (303.4f). Of the Roles one player controls on a permanent, only the newest stays
//! (303.7a, 704.5z). Monstrous Rage, Witch's Mark, Cursed Courtier, Spiteful Hexmage, Embereth
//! Veteran.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const RAGE: usize = 0;
const MARK: usize = 1;
const COURTIER: usize = 2;
const HEXMAGE: usize = 3;
const VETERAN: usize = 4;
const BEAR: usize = 5;
const CURSE: usize = 6;
const VIRTUE: usize = 7;
const SORCERY: usize = 8;
const ISLAND: usize = 9;

fn real(name: &str) -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let def = CardDef::from_faces(&CardDef::catalog(&faces)[name]).unwrap();
    game::play::check(&def).unwrap_or_else(|e| panic!("{name}: {}", e.0));
    def
}
fn game() -> Game {
    let c = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).expect(name);
    let defs = vec![real("Monstrous Rage"), real("Witch's Mark"), real("Cursed Courtier"), real("Spiteful Hexmage"),
        real("Embereth Veteran"), c("Bear", "Creature — Bear", "", Some((2, 2))),
        c("Curse", "Instant", "Create a Cursed Role token attached to target creature.", None),
        c("Virtue", "Instant", "Create a Virtuous Role token attached to target creature.", None),
        c("Sorcery", "Instant", "Create a Sorcerer Role token attached to target creature.", None),
        real("Island")];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
/// Resolve the stack, keeping each scried card on top; how many times a scry was asked.
fn resolve(g: &mut Game) -> usize {
    let mut scried = 0;
    for _ in 0..40 {
        match g.asked() {
            Some((p, game::legal::Asked::Arrange { cards, .. })) => { scried += 1; g.act(p, Move::Choose(game::state::Answer::Arrange { top: cards, away: vec![] })).unwrap() },
            _ if g.stack.is_empty() => return scried,
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("the stack did not empty");
}
fn cast(g: &mut Game, player: usize, card: usize, targets: Vec<Vec<Target>>, spending: &str) -> ObjId {
    let spell = g.put(player, card, Zone::Hand);
    g.players[player].pool.colors = [9; 5];
    g.players[player].pool.colorless = 9;
    if g.priority != player { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(player, paid(spending, Move::Cast { card: spell, targets })).unwrap();
    resolve(g);
    spell
}
fn roles_on(g: &Game, creature: ObjId) -> Vec<ObjId> {
    g.battlefield.iter().copied().filter(|o| g.obj(*o).attached_to == Some(Target::Object(creature))).collect()
}
fn pt(g: &Game, o: ObjId) -> (i32, i32) { g.seen(o, None).pt.unwrap() }

#[test]
fn r111_10k_monstrous_rage_creates_a_monster_role_attached_to_its_target() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    cast(&mut g, 0, RAGE, vec![vec![Target::Object(bear)]], "R");
    let [role] = roles_on(&g, bear)[..] else { panic!("one Role on the bear") };
    let o = g.obj(role);
    // a colorless Aura Role enchantment token named Monster (111.10k)
    cite!(["111.6"]; assert!(o.token() && o.card.is_none()));
    cite!(["111.10k"]; assert_eq!(o.chars.name, "Monster"));
    assert!(o.chars.colors.is_empty());
    assert_eq!(o.chars.types, vec![ir::CardType::Enchantment]);
    cite!(["111.10k"]; assert_eq!(o.chars.subtypes.iter().map(|s| s.name()).collect::<Vec<_>>(), ["Aura", "Role"]));
    assert_eq!(o.controller, 0);
    // +2/+0 until end of turn, and +1/+1 and trample from the Role
    assert_eq!(pt(&g, bear), (5, 3));
    cite!(["111.10k"]; assert!(g.has(bear, "Trample")));
}

#[test]
fn r303_7a_a_second_role_from_the_same_player_replaces_the_first() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    cast(&mut g, 0, RAGE, vec![vec![Target::Object(bear)]], "R");
    let [first] = roles_on(&g, bear)[..] else { panic!() };
    cast(&mut g, 0, CURSE, vec![vec![Target::Object(bear)]], "");
    // the older Monster Role is put into the graveyard and, a token, ceases to exist (704.5z, 704.5d)
    let [newest] = roles_on(&g, bear)[..] else { panic!("only the newest Role stays: {:?}", roles_on(&g, bear)) };
    cite!(["303.7a"]; assert_ne!(newest, first));
    assert_eq!(g.obj(newest).chars.name, "Cursed");
    cite!(["704.5d"]; assert_eq!(g.obj(first).zone, Zone::Gone));
    // base 1/1, and Monstrous Rage's +2/+0 still applies (613.4b, 613.4c)
    cite!(["613.4b", "613.4c"]; assert_eq!(pt(&g, bear), (3, 1)));
    assert!(!g.has(bear, "Trample"));
}

#[test]
fn r303_7a_roles_of_different_players_on_one_creature_both_stay() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    cast(&mut g, 0, RAGE, vec![vec![Target::Object(bear)]], "R");
    cast(&mut g, 1, CURSE, vec![vec![Target::Object(bear)]], "");
    let roles = roles_on(&g, bear);
    cite!(["303.7a"]; assert_eq!(roles.len(), 2));
    assert_eq!(roles.iter().map(|r| g.obj(*r).controller).collect::<Vec<_>>(), [0, 1]);
    // base 1/1, then +2/+0 and +1/+1
    cite!(["613.4b", "613.4c"]; assert_eq!(pt(&g, bear), (4, 2)));
}

#[test]
fn r111_10q_a_superseded_wicked_role_drains_each_opponent() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    // Witch's Mark: "You may discard a card. If you do, draw two cards." declined
    let spell = g.put(0, MARK, Zone::Hand);
    g.players[0].pool.colors = [9; 5];
    g.act(0, paid("WR", Move::Cast { card: spell, targets: vec![vec![Target::Object(bear)]] })).unwrap();
    for _ in 0..10 {
        if g.stack.is_empty() { break; }
        match g.decision.clone() {
            Some(game::state::Decision::Choose { player, .. }) => g.act(player, Move::Choose(game::state::Answer::No)).unwrap(),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    let [wicked] = roles_on(&g, bear)[..] else { panic!() };
    cite!(["111.10q"]; assert_eq!(g.obj(wicked).chars.name, "Wicked"));
    assert_eq!(pt(&g, bear), (3, 3));
    cast(&mut g, 0, CURSE, vec![vec![Target::Object(bear)]], "");
    cite!(["111.10q", "704.5z"]; assert_eq!(g.players[1].life, 19, "the Wicked Role went to the graveyard"));
    cite!(["111.10q"]; assert_eq!(g.players[0].life, 20));
    cite!(["303.7a"]; assert_eq!(pt(&g, bear), (1, 1)));
}

#[test]
fn r303_4i_up_to_one_target_with_none_chosen_creates_no_role() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let spell = g.put(0, MARK, Zone::Hand);
    g.players[0].pool.colors = [9; 5];
    g.act(0, paid("WR", Move::Cast { card: spell, targets: vec![vec![]] })).unwrap();
    for _ in 0..10 {
        if g.stack.is_empty() { break; }
        match g.decision.clone() {
            Some(game::state::Decision::Choose { player, .. }) => g.act(player, Move::Choose(game::state::Answer::No)).unwrap(),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    assert!(roles_on(&g, bear).is_empty());
    cite!(["303.4i"]; assert!(!g.battlefield.iter().any(|o| g.obj(*o).token()), "the Aura token is not created (303.4i)"));
}

#[test]
fn r111_10j_cursed_courtier_curses_itself_as_it_enters() {
    let mut g = game();
    cast(&mut g, 0, COURTIER, vec![], "CCW");
    let courtier = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(COURTIER)).unwrap();
    let [cursed] = roles_on(&g, courtier)[..] else { panic!() };
    cite!(["111.10j"]; assert_eq!(g.obj(cursed).chars.name, "Cursed"));
    cite!(["111.10j"]; assert_eq!(pt(&g, courtier), (1, 1)));
}

#[test]
fn r111_10j_spiteful_hexmage_curses_a_creature_its_controller_controls() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let theirs = g.put(1, BEAR, Zone::Battlefield);
    let hexmage = g.put(0, HEXMAGE, Zone::Hand);
    g.players[0].pool.colors = [9; 5];
    g.act(0, paid("B", Move::Cast { card: hexmage, targets: vec![] })).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    // its enters trigger targets a creature its controller controls: not the opponent's
    let Some((0, game::legal::Asked::Targets { slots, .. })) = g.asked() else { panic!("{:?}", g.asked()) };
    cite!(["115.1"]; assert!(slots[0].among.contains(&Target::Object(bear)) && !slots[0].among.contains(&Target::Object(theirs))));
    cite!(["115.1"]; refused("an illegal target, Bear: the target is one you control, and another player controls it (115.1)"): g.act(0, Move::ChooseTargets(vec![vec![Target::Object(theirs)]])));
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(bear)]])).unwrap();
    resolve(&mut g);
    cite!(["111.10j"]; assert_eq!(pt(&g, bear), (1, 1)));
    assert_eq!(pt(&g, theirs), (2, 2));
}

#[test]
fn r111_10p_a_virtuous_role_counts_each_enchantment_its_controller_controls() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    cast(&mut g, 0, VIRTUE, vec![vec![Target::Object(bear)]], "");
    // itself
    cite!(["111.10p"]; assert_eq!(pt(&g, bear), (3, 3)));
    let other = g.put(1, BEAR, Zone::Battlefield);
    cast(&mut g, 0, RAGE, vec![vec![Target::Object(other)]], "R");
    // the Monster Role on the other bear is an enchantment the Virtuous Role's controller controls
    cite!(["111.10p"]; assert_eq!(pt(&g, bear), (4, 4)));
}

fn attack_with(g: &mut Game, attacker: ObjId) -> usize {
    while g.part != Part::DeclareAttackers { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Attack(vec![(attacker, Target::Player(1))])).unwrap();
    resolve(g)
}

#[test]
fn r111_10n_a_sorcerer_role_scries_as_its_creature_attacks() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    cast(&mut g, 0, SORCERY, vec![vec![Target::Object(bear)]], "");
    cite!(["111.10n"]; assert_eq!(pt(&g, bear), (3, 3)));
    cite!(["111.10n"]; assert_eq!(attack_with(&mut g, bear), 1, "scry 1"));
}

#[test]
fn r111_10r_embereth_veterans_young_hero_role_grows_a_small_attacker() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let veteran = g.put(0, VETERAN, Zone::Battlefield);
    g.players[0].pool.colorless = 1;
    // "another target creature": not itself
    cite!(["115.1"]; refused("an illegal target, Embereth Veteran: it is the object whose ability this is, and the target says \"another\" (115.1)"): g.act(0, Move::Activate { source: veteran, ability: 0, targets: vec![vec![Target::Object(veteran)]], paying: vec![] }));
    g.act(0, paid("C", Move::Activate { source: veteran, ability: 0, targets: vec![vec![Target::Object(bear)]], paying: vec![] })).unwrap();
    resolve(&mut g);
    assert_eq!(g.obj(veteran).zone, Zone::Gone);
    let [role] = roles_on(&g, bear)[..] else { panic!() };
    cite!(["111.10r"]; assert_eq!(g.obj(role).chars.name, "Young Hero"));
    attack_with(&mut g, bear);
    cite!(["111.10r"]; assert_eq!(g.obj(bear).counters.get("+1/+1"), Some(&1)));
    assert_eq!(pt(&g, bear), (3, 3));
}

#[test]
fn r111_10m_royal_role_is_refused_until_ward_is_run() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let def = CardDef::from_faces(&CardDef::catalog(&faces)["Royal Treatment"]).unwrap();
    let refused = game::play::check(&def).unwrap_err().0;
    assert!(refused.contains("Royal Role"), "{refused}");
}
