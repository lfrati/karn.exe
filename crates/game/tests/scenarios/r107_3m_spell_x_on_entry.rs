//! X on entry (107.3m): a permanent's enters-the-battlefield triggered abilities and its own
//! entry replacement effects read the X announced for the spell it was, while the permanent's
//! own X is 0 (107.3g). Entering any other way leaves that X 0.
use crate::support::{cite,paid};
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const HYDRA: usize = 0;
const CAPTAIN: usize = 1;
const RECALLER: usize = 2;
const ZENITH: usize = 3;
const FOREST: usize = 4;
const PLAINS: usize = 5;
const SWAMP: usize = 6;
const BEAR: usize = 7;
const GIANT: usize = 8;
const SCALES: usize = 9;
const FORK: usize = 10;

fn defs() -> Vec<CardDef> {
    vec![
        CardDef::new("Ivy Hydra", "{X}{G}", "Creature — Hydra", "This creature enters with X +1/+1 counters on it.", Some((0, 0))).unwrap(),
        CardDef::new("Muster Captain", "{X}{W}", "Creature — Human Soldier", "When this creature enters, create X 1/1 white Soldier creature tokens.", Some((1, 1))).unwrap(),
        CardDef::new("Recaller", "{X}{B}", "Creature — Zombie", "When this creature enters, return target creature card with mana value X or less from your graveyard to the battlefield.", Some((1, 1))).unwrap(),
        CardDef::new("Green Sun's Zenith", "{X}{G}", "Sorcery", "Search your library for a green creature card with mana value X or less, put it onto the battlefield, then shuffle. Shuffle Green Sun's Zenith into its owner's library.", None).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Plains", "", "Basic Land — Plains", "", None).unwrap(),
        CardDef::new("Swamp", "", "Basic Land — Swamp", "", None).unwrap(),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Giant", "{3}{G}{G}", "Creature — Giant", "", Some((5, 5))).unwrap(),
        CardDef::new("Hardened Scales", "{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.", None).unwrap(),
        CardDef::new("Fork", "{0}", "Instant", "Copy target spell.", None).unwrap(),
    ]
}
fn game() -> Game {
    let defs = defs();
    for d in &defs { game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0)); }
    Game::new(defs, 2, 20)
}
fn lands(g: &mut Game, def: usize, n: usize) -> Vec<ObjId> { (0..n).map(|_| g.put(0, def, Zone::Battlefield)).collect() }
fn cast(x: u32, card: ObjId, plan: &[ObjId], units:&str) -> Move {
    let steps = plan.iter().map(|&source| game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source, ability: 0, paying: vec![], adding: vec![] }).collect();
    Move::WithX(x, Box::new(Move::WithMana(steps, Box::new(paid(units,Move::Cast { card, targets: vec![] })))))
}
fn resolve(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
fn named(g: &Game, name: &str) -> Vec<ObjId> { g.battlefield.iter().copied().filter(|o| g.obj(*o).chars.name == name).collect() }
fn counters(g: &Game, o: ObjId) -> u32 { g.obj(o).counters.get("+1/+1").copied().unwrap_or(0) }

#[test]
fn r107_3m_only_a_permanents_own_entry_abilities_read_its_spells_x() {
    game();
    for (name, text) in [
        // the X of a spell is not that of other permanents' entries
        ("Watcher", "Whenever another creature enters, you gain X life."),
        // an ability that triggers on something besides entering is not an enters ability
        ("Raider", "Whenever this creature enters or attacks, you gain X life."),
        // the permanent's own X is 0, and nothing announces one for this ability (107.3g)
        ("Keeper", "{T}: You gain X life."),
        ("Lord", "Other creatures you control get +X/+X."),
        // a granted ability defines its own X or has 0 (107.3j)
        ("Herald", "Other creatures you control have \"When this creature enters, you gain X life.\""),
    ] {
        let def = CardDef::new(name, "{X}{G}", "Creature — Elf", text, Some((1, 1))).unwrap();
        // Uncited (admission): an unbound X is refused, rather than borrowed from another spell.
        crate::support::refused(game::play::check(&def).map_err(|e| e.0), "amount: X");
    }
    // without {X} in the mana cost there is no spell X to read
    let def = CardDef::new("Plain Hydra", "{2}{G}", "Creature — Hydra", "This creature enters with X +1/+1 counters on it.", Some((0, 0))).unwrap();
    // Uncited (admission): this entry replacement has no announced spell X.
    crate::support::refused(game::play::check(&def).map_err(|e|e.0), "amount: X");
}

#[test]
fn r107_3m_an_entry_replacement_reads_the_x_of_the_spell_the_permanent_was() {
    for x in [0, 3] {
        let mut g = game();
        let card = g.put(0, HYDRA, Zone::Hand);
        let forests = lands(&mut g, FOREST, 4);
        g.act(0, cast(x, card, &forests[..x as usize + 1],&"G".repeat(x as usize+1))).unwrap();
        resolve(&mut g);
        let hydra = named(&g, "Ivy Hydra");
        if x == 0 {
            cite!(["704.5f"]; assert!(hydra.is_empty(), "a 0/0 with no counters is put into the graveyard (704.5f)"));
            continue;
        }
        let [hydra] = hydra[..] else { panic!("{hydra:?}") };
        cite!(["107.3m"]; assert_eq!(counters(&g, hydra), 3));
        assert_eq!((g.power(hydra), g.toughness(hydra)), (Some(3), Some(3)));
        cite!(["107.3g", "107.3m"]; assert_eq!((g.obj(hydra).x, g.obj(hydra).chars.mana_value), (None, 1), "the permanent's own X is 0 (107.3m, 107.3g)"));
    }
}

#[test]
fn r107_3m_a_replaced_counter_placement_starts_from_the_spells_x() {
    let mut g = game();
    g.put(0, SCALES, Zone::Battlefield);
    let card = g.put(0, HYDRA, Zone::Hand);
    let forests = lands(&mut g, FOREST, 3);
    g.act(0, cast(2, card, &forests,"GGG")).unwrap();
    resolve(&mut g);
    let [hydra] = named(&g, "Ivy Hydra")[..] else { panic!() };
    cite!(["614.1a", "614.1c"]; assert_eq!(counters(&g, hydra), 3, "two from X, plus one (614.1c, 614.1a)"));
}

#[test]
fn r107_3m_an_enters_trigger_reads_the_x_of_the_spell_the_permanent_was() {
    for x in [0, 2] {
        let mut g = game();
        let card = g.put(0, CAPTAIN, Zone::Hand);
        let plains = lands(&mut g, PLAINS, 3);
        g.act(0, cast(x, card, &plains[..x as usize + 1],&"W".repeat(x as usize+1))).unwrap();
        resolve(&mut g);
        assert_eq!(g.stack.len(), 1, "the enters ability triggers whatever X is");
        resolve(&mut g);
        cite!(["107.3m"]; case: format!("x {x}"); assert_eq!(named(&g, "Soldier Token").len(), x as usize));
        let [captain] = named(&g, "Muster Captain")[..] else { panic!() };
        cite!(["107.3m"]; case: format!("x {x}"); assert_eq!(g.obj(captain).x, None));
    }
}

#[test]
fn r107_3m_the_spells_x_bounds_an_enters_triggers_targets_and_is_rechecked() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Graveyard);
    let giant = g.put(0, GIANT, Zone::Graveyard);
    let card = g.put(0, RECALLER, Zone::Hand);
    let swamps = lands(&mut g, SWAMP, 3);
    g.act(0, cast(2, card, &swamps,"BBB")).unwrap();
    resolve(&mut g);
    assert!(matches!(g.decision, Some(Decision::Targets { player: 0, .. })));
    cite!(["107.3m", "115.1"]; refused("an illegal target, Giant: its mana value is 5, and the target says mana value 2 or less (115.1)"): g.act(0, Move::ChooseTargets(vec![vec![Target::Object(giant)]])));
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(bear)]])).unwrap();
    resolve(&mut g);
    cite!(["608.2b"]; assert_eq!(named(&g, "Bear").len(), 1, "still a legal target as the ability resolves (608.2b)"));
    assert!(g.players[0].graveyard.contains(&giant));
}

#[test]
fn r107_3m_a_permanent_put_onto_the_battlefield_by_another_spell_has_x_0() {
    let mut g = game();
    g.put(0, HYDRA, Zone::Library);
    let card = g.put(0, ZENITH, Zone::Hand);
    let forests = lands(&mut g, FOREST, 2);
    // Zenith's own X is 1; the Hydra it finds was no spell, so its X is 0
    g.act(0, cast(1, card, &forests,"GG")).unwrap();
    resolve(&mut g);
    let found: Vec<_> = g.players[0].library.iter().copied().filter(|o| g.obj(*o).card == Some(HYDRA)).collect();
    g.act(0, Move::Choose(game::state::Answer::Objects(found))).unwrap();
    assert!(named(&g, "Ivy Hydra").is_empty());
    cite!(["107.3m", "704.5f"]; assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).card == Some(HYDRA)), "it entered with no counters and died (704.5f)"));
}

#[test]
fn r107_3m_a_copy_of_the_spell_has_its_x_and_so_does_the_token_it_becomes() {
    let mut g = game();
    let card = g.put(0, HYDRA, Zone::Hand);
    let forests = lands(&mut g, FOREST, 3);
    g.act(0, cast(2, card, &forests,"GGG")).unwrap();
    let original = g.stack[0];
    let fork = g.put(0, FORK, Zone::Hand);
    g.act(0, Move::Cast { card: fork, targets: vec![vec![Target::Object(original)]] }).unwrap();
    resolve(&mut g);
    let copied = *g.stack.last().unwrap();
    cite!(["707.10"]; assert_eq!(g.obj(copied).x, Some(2), "a copy copies the choice of X (707.10)"));
    resolve(&mut g);
    resolve(&mut g);
    let hydras = named(&g, "Ivy Hydra");
    assert_eq!(hydras.len(), 2);
    cite!(["707.10f"]; assert!(hydras.iter().any(|o| g.obj(*o).token())));
    cite!(["107.3m", "707.10"]; assert!(hydras.iter().all(|o| counters(&g, *o) == 2)));
}

#[test]
fn r107_3m_the_meathook_massacre_shrinks_each_creature_by_its_spells_x() {
    let massacre = CardDef::new("The Meathook Massacre", "{X}{B}{B}", "Legendary Enchantment",
        "When The Meathook Massacre enters, each creature gets -X/-X until end of turn.\nWhenever a creature you control dies, each opponent loses 1 life.\nWhenever a creature an opponent controls dies, you gain 1 life.", None).unwrap();
    game::play::check(&massacre).unwrap();
    let mut defs = defs();
    defs.push(massacre);
    let mut g = Game::new(defs, 2, 20);
    let mine = g.put(0, BEAR, Zone::Battlefield);
    let theirs = g.put(1, BEAR, Zone::Battlefield);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    let card = g.put(0, 11, Zone::Hand);
    let swamps = lands(&mut g, SWAMP, 4);
    g.act(0, cast(2, card, &swamps,"BBBB")).unwrap();
    resolve(&mut g);
    resolve(&mut g);
    cite!(["704.5f"]; assert!(!g.battlefield.contains(&mine) && !g.battlefield.contains(&theirs), "-2/-2 kills both 2/2s (704.5f)"));
    assert!((0..2).all(|p| g.players[p].graveyard.iter().any(|o| g.obj(*o).card == Some(BEAR))));
    cite!(["107.3m"]; assert_eq!((g.power(giant), g.toughness(giant)), (Some(3), Some(3))));
    // both death triggers, ordered by their controller: each opponent loses 1, and you gain 1
    g.act(0, Move::Order(vec![0, 1])).unwrap();
    while !g.stack.is_empty() { resolve(&mut g); }
    assert_eq!((g.players[0].life, g.players[1].life), (21, 19));
}
