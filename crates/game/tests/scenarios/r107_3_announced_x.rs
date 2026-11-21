//! Announced X (107.3a, 601.2b, 601.2f, 602.2b): chosen with the spell or activation, locked
//! into the cost, kept by the object on the stack and read by its own instructions.
use crate::support::{cite, paid};
use game::legal::{Step};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const REVELATION: usize = 0;
const ZENITH: usize = 1;
const WOLF_RUN: usize = 2;
const ISLAND: usize = 3;
const PLAINS: usize = 4;
const FOREST: usize = 5;
const MOUNTAIN: usize = 6;
const BEAR: usize = 7;
const GIANT: usize = 8;
const RAZE: usize = 9;
const GROWTH: usize = 10;

fn defs() -> Vec<CardDef> {
    vec![
        CardDef::new("Sphinx's Revelation", "{X}{W}{U}{U}", "Instant", "You gain X life and draw X cards.", None).unwrap(),
        CardDef::new("Green Sun's Zenith", "{X}{G}", "Sorcery", "Search your library for a green creature card with mana value X or less, put it onto the battlefield, then shuffle. Shuffle Green Sun's Zenith into its owner's library.", None).unwrap(),
        CardDef::new("Kessig Wolf Run", "", "Land", "{T}: Add {C}.\n{X}{R}{G}, {T}: Target creature gets +X/+0 and gains trample until end of turn.", None).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
        CardDef::new("Plains", "", "Basic Land — Plains", "", None).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Mountain", "", "Basic Land — Mountain", "", None).unwrap(),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Giant", "{3}{G}{G}", "Creature — Giant", "", Some((5, 5))).unwrap(),
        CardDef::new("Raze", "{0}", "Instant", "Destroy target land.", None).unwrap(),
        CardDef::new("Growth", "{G}", "Instant", "Target creature gets +3/+3 until end of turn.", None).unwrap(),
    ]
}
fn game() -> Game {
    let defs = defs();
    for d in &defs { game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0)); }
    Game::new(defs, 2, 20)
}
fn step(source: ObjId) -> Step {  Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source, ability: 0, paying: vec![], adding: vec![] } }
fn lands(g: &mut Game, def: usize, n: usize) -> Vec<ObjId> { (0..n).map(|_| g.put(0, def, Zone::Battlefield)).collect() }
fn cast(x: u32, card: ObjId, plan: &[ObjId], spending: &str) -> Move {
    paid(spending, Move::WithX(x, Box::new(Move::WithMana(plan.iter().copied().map(step).collect(), Box::new(Move::Cast { card, targets: vec![] })))))
}
fn resolve(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
/// A Revelation in hand, a library to draw from, and one Plains and `islands` Islands.
fn revelation(islands: usize) -> (Game, ObjId, Vec<ObjId>) {
    let mut g = game();
    for _ in 0..8 { g.put(0, BEAR, Zone::Library); }
    let card = g.put(0, REVELATION, Zone::Hand);
    let mut mana = lands(&mut g, PLAINS, 1);
    mana.extend(lands(&mut g, ISLAND, islands));
    (g, card, mana)
}

#[test]
fn r107_3_the_witnesses_are_admitted_and_other_x_mechanisms_stay_refused() {
    game();
    // Counter-unless-X is covered by r701_5_counter_unless; these mechanisms remain unsupported.
    for (name, cost, kind, text) in [
        ("Two variables", "{X}{Y}{G}", "Sorcery", "You gain X life."),
    ] {
        let d=CardDef::new(name,cost,kind,text,None).unwrap();
        crate::support::refused(game::play::check(&d).map_err(|e|e.0),"cost: Var");
    }
    // text that reads X with no {X} to announce stays refused
    let unannounced = CardDef::new("No X", "{W}", "Instant", "You gain X life and draw X cards.", None).unwrap();
    crate::support::refused(game::play::check(&unannounced).map_err(|e|e.0),"amount: X");
}

#[test]
fn r107_3a_the_announced_value_is_paid_kept_on_the_stack_and_read_on_resolution() {
    let (mut g, card, mana) = revelation(5);
    cite!(["107.3g"]; assert_eq!(g.obj(card).chars.mana_value, 3, "X is 0 off the stack (107.3g)"));
    g.act(0, cast(3, card, &mana, "WUUUUU")).unwrap();
    let spell = g.stack[0];
    cite!(["107.3a"]; assert_eq!(g.obj(spell).x, Some(3)));
    cite!(["202.3e"]; assert_eq!(g.obj(spell).chars.mana_value, 6, "on the stack X is the announced value (202.3e)"));
    assert!(mana.iter().all(|l| g.obj(*l).tapped));
    assert_eq!(g.players[0].pool.total(), 0);
    let copy = g.clone();
    for mut game in [g, copy] {
        resolve(&mut game);
        cite!(["107.3a"]; assert_eq!(game.players[0].life, 23));
        assert_eq!(game.players[0].hand.len(), 3);
        let card = game.players[0].graveyard[0];
        cite!(["107.3g"]; assert_eq!((game.obj(card).x, game.obj(card).chars.mana_value), (None, 3), "X is 0 again off the stack (107.3g)"));
    }
}

#[test]
fn r107_3a_zero_is_a_legal_announcement() {
    let (mut g, card, mana) = revelation(2);
    g.act(0, cast(0, card, &mana, "WUU")).unwrap();
    cite!(["107.3a"]; assert_eq!((g.obj(g.stack[0]).x, g.obj(g.stack[0]).chars.mana_value), (Some(0), 3)));
    resolve(&mut g);
    cite!(["107.3a"]; assert_eq!((g.players[0].life, g.players[0].hand.len()), (20, 0)));
}

#[test]
fn r601_2f_a_refused_announcement_changes_nothing() {
    let (mut g, card, mana) = revelation(5);
    let before = g.clone();
    let plan: Vec<Step> = mana.iter().copied().map(step).collect();
    for (mv, why) in [
        (Move::WithMana(plan.clone(), Box::new(Move::Cast { card, targets: vec![] })), "that cost has {X}: announce the value of X (107.3a, 601.2b)"),
        (cast(4, card, &mana, "WUUUUUU"), "that mana is not in your mana pool (118.3a)"),
        (Move::WithX(1, Box::new(cast(2, card, &mana, "WUUUU"))), "announce the value of X only once (107.3a)"),
    ] {
        cite!(["733.1"]; case: why; refused(why): g.act(0, mv));
        assert!(g.stack.is_empty() && g.players[0].hand == vec![card] && mana.iter().all(|l| !g.obj(*l).tapped));
        assert_eq!(g.log.len(), before.log.len());
    }
    // a cost without {X} takes no value
    let bear = g.put(0, BEAR, Zone::Hand);
    g.players[0].pool.colors[4] = 2;
    cite!(["107.3a"]; refused("that cost has no {X} whose value is announced (107.3a)"): g.act(0, Move::WithX(0, Box::new(paid("GG", Move::Cast { card: bear, targets: vec![] })))));
    g.act(0, paid("GG", Move::Cast { card: bear, targets: vec![] })).unwrap();
}

#[test]
fn r107_3a_x_bounds_what_the_spell_finds() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Library);
    let giant = g.put(0, GIANT, Zone::Library);
    let card = g.put(0, ZENITH, Zone::Hand);
    let forests = lands(&mut g, FOREST, 3);
    g.act(0, cast(2, card, &forests, "GGG")).unwrap();
    resolve(&mut g);
    let (_, asked) = g.asked().expect("the search asks which card");
    let game::legal::Asked::Search { cards, among, .. } = asked else { panic!("{asked:?}") };
    cite!(["701.23a", "107.3a"]; assert_eq!((cards, among), (vec![bear, giant], vec![bear]), "the whole library is seen; mana value 2 or less can be found"));
}

#[test]
fn r601_2c_targets_are_chosen_with_x_already_announced() {
    let defs = vec![
        CardDef::new("Bound", "{X}{B}", "Instant", "Destroy target creature with mana value X or less.", None).unwrap(),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Sweep", "{X}{W}", "Instant", "Tap X target creatures.", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    let (card, sweep) = (g.put(0, 0, Zone::Hand), g.put(0, 2, Zone::Hand));
    let bears = [g.put(1, 1, Zone::Battlefield), g.put(1, 1, Zone::Battlefield)];
    g.players[0].pool.colors = [4, 0, 4, 0, 0];
    let at = |x: u32, card: ObjId, targets: Vec<ObjId>, spending: &str| paid(spending, Move::WithX(x, Box::new(Move::Cast { card, targets: vec![targets.into_iter().map(Target::Object).collect()] })));
    cite!(["601.2c", "115.1"]; refused("an illegal target, Bear: its mana value is 2, and the target says mana value 1 or less (115.1)"): g.act(0, at(1, card, vec![bears[0]], "BB")));
    cite!(["601.2c"]; refused("wrong number of targets: 2 chosen, it takes 1 (601.2c)"): g.act(0, at(1, sweep, vec![bears[0], bears[1]], "WW")));
    for x in 0..=8 {
        cite!(["601.2c", "118.3a"]; case:x; assert_eq!(g.clone().act(0, at(x, card, vec![bears[0]], &format!("{}{}", "B".repeat((x+1).min(4) as usize), "W".repeat(x.saturating_sub(3) as usize)))).is_ok(), (2..=7).contains(&x)));
    }
    g.act(0, at(2, sweep, vec![bears[0], bears[1]], "WWW")).unwrap();
    g.act(0, at(2, card, vec![bears[0]], "BBB")).unwrap();
    resolve(&mut g);
    cite!(["107.3a"]; assert_ne!(g.obj(bears[0]).zone, Zone::Battlefield));
    resolve(&mut g);
    cite!(["608.2b"]; assert!(g.obj(bears[1]).tapped, "the tapping spell keeps its remaining target"));
}

fn wolf_run() -> (Game, ObjId, ObjId, Vec<ObjId>) {
    let mut g = game();
    let run = g.put(0, WOLF_RUN, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let mut mana = lands(&mut g, MOUNTAIN, 1);
    mana.extend(lands(&mut g, FOREST, 3));
    (g, run, bear, mana)
}
fn pump(x: u32, run: ObjId, bear: ObjId, plan: &[ObjId], spending: &str) -> Move {
    paid(spending, Move::WithX(x, Box::new(Move::WithMana(plan.iter().copied().map(step).collect(),
        Box::new(Move::Activate { source: run, ability: 1, targets: vec![vec![Target::Object(bear)]], paying: vec![] })))))
}

#[test]
fn r602_2b_an_activation_announces_x_and_keeps_it_when_its_source_is_gone() {
    let (mut g, run, bear, mana) = wolf_run();
    cite!(["118.3a"]; refused("that mana is not in your mana pool (118.3a)"): g.act(0, pump(3, run, bear, &mana, "RGGGG")));
    cite!(["602.2b"]; assert!(!g.obj(run).tapped && g.stack.is_empty()));
    g.act(0, pump(2, run, bear, &mana, "RGGG")).unwrap();
    cite!(["107.3a"]; assert_eq!(g.obj(g.stack[0]).x, Some(2)));
    cite!(["107.3k"]; assert_eq!(g.obj(run).x, None, "the value belongs to the activation, not to its source"));
    // the source is destroyed in response; the ability exists independently of it (113.7a)
    let raze = g.put(0, RAZE, Zone::Hand);
    g.act(0, Move::Cast { card: raze, targets: vec![vec![Target::Object(run)]] }).unwrap();
    resolve(&mut g);
    assert_ne!(g.obj(run).zone, Zone::Battlefield);
    resolve(&mut g);
    cite!(["113.7a"]; assert_eq!(g.power(bear), Some(4)));
}

#[test]
fn r107_3a_each_activation_has_its_own_x() {
    let (mut g, run, bear, mut mana) = wolf_run();
    let second = g.put(0, WOLF_RUN, Zone::Battlefield);
    mana.extend(lands(&mut g, MOUNTAIN, 1));
    mana.extend(lands(&mut g, FOREST, 2));
    // {1}{R}{G} from the first three lands, {2}{R}{G} from the other four
    g.act(0, pump(1, run, bear, &[mana[0], mana[1], mana[2]], "RGG")).unwrap();
    g.act(0, pump(2, second, bear, &[mana[4], mana[3], mana[5], mana[6]], "RGGG")).unwrap();
    cite!(["107.3k"]; assert_eq!(g.stack.iter().map(|o| g.obj(*o).x).collect::<Vec<_>>(), vec![Some(1), Some(2)]));
    resolve(&mut g);
    assert_eq!(g.power(bear), Some(4));
    resolve(&mut g);
    cite!(["107.3k"]; assert_eq!(g.power(bear), Some(5)));
}

#[test]
fn r107_3a_authored_x_values_pay_their_announced_costs() {
    let (mut g, card, mana) = revelation(4);
    for x in 0..=2 {
        let mut tried = g.clone();
        tried.act(0, cast(x, card, &mana[..3+x as usize], &format!("W{}", "U".repeat(2+x as usize)))).unwrap();
        cite!(["107.3a"]; assert_eq!(tried.obj(tried.stack[0]).x, Some(x)));
    }
    let before=format!("{g:?}");
    cite!(["107.3a","118.3a"]; refused("that mana is not in your mana pool"): g.act(0, cast(3, card, &mana, "WUUUUU")));
    assert_eq!(format!("{g:?}"),before);
    g.players[0].pool.colors[1] = 12;
    g.act(0, cast(10, card, &mana[..1], "WUUUUUUUUUUUU")).unwrap();
    cite!(["107.3a"]; assert_eq!(g.obj(g.stack[0]).x, Some(10)));
    let (g, run, bear, mana) = wolf_run();
    for x in 0..=3 {
        let mut attempt=g.clone();
        let action=pump(x,run,bear,&mana,&format!("R{}", "G".repeat(1+x as usize)));
        if x<=2 {
            cite!(["602.2b","107.3a"]; case:x; ok: attempt.act(0,action));
            cite!(["107.3a"]; assert_eq!(attempt.obj(attempt.stack[0]).x,Some(x)));
        } else {
            cite!(["602.2b","107.3a","118.3a"]; refused("that mana is not in your mana pool"): attempt.act(0,action));
            assert_eq!(format!("{attempt:?}"),format!("{g:?}"));
        }
    }
}

#[test]
fn r601_2f_x_stays_locked_while_the_payment_waits_and_when_it_resumes() {
    use game::state::{Answer, Choice, Decision};
    use ir::mana::ManaSymbol;
    let green = |n: usize| vec![ManaSymbol::Color(ir::Color::Green); n];
    let defs = vec![
        CardDef::new("Mana creature", "{0}", "Creature — Elf", "{T}: You gain 2 life. Put a +1/+1 counter on this creature. Add X mana of any one color, where X is the number of +1/+1 counters on this creature.", Some((1, 1))).unwrap(),
        CardDef::new("Hardened Scales", "{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.", None).unwrap(),
        CardDef::new("Branching Evolution", "{2}{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.", None).unwrap(),
        CardDef::new("Reward", "{X}{G}", "Sorcery", "You gain X life.", None).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let scales = g.put(0, 1, Zone::Battlefield);
    g.put(0, 2, Zone::Battlefield);
    let card = g.put(0, 3, Zone::Hand);
    // the creature's "X" is its own text's (107.3c); the spell's is announced as 3
    let plan = vec![Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source, ability: 0, paying: vec![], adding: vec![green(3)] }];
    g.act(0, paid("GGGG", Move::WithX(3, Box::new(Move::WithMana(plan, Box::new(Move::Cast { card, targets: vec![] })))))).unwrap();
    // the mana ability waits on the order of two replacement effects; the spell is announced
    let spell = g.stack[0];
    cite!(["601.2b"]; assert_eq!((g.obj(spell).x, g.obj(spell).chars.mana_value), (Some(3), 4)));
    crate::support::refused(g.act(0, Move::WithX(1, Box::new(Move::Cast { card: spell, targets: vec![] }))), "a decision is pending: a choice of the effect being applied (608.2d)");
    let Some(Decision::Choose { choice: Choice::Replacement { among, .. }, .. }) = &g.decision else { panic!("{:?}", g.decision) };
    let effect = among.iter().find(|e| e.source == scales).unwrap().clone();
    g.act(0, Move::Choose(Answer::Replacement(effect))).unwrap();
    // scales first makes four counters: four mana pays {3}{G} exactly
    g.act(0, Move::Choose(Answer::Mana(green(4)))).unwrap();
    assert!(g.decision.is_none());
    cite!(["601.2f"]; assert_eq!((g.players[0].pool.total(), g.obj(spell).x), (0, Some(3))));
    resolve(&mut g);
    cite!(["107.3a"]; assert_eq!(g.players[0].life, 25, "2 from the mana ability, 3 from X"));
}

#[test]
fn r601_2h_mana_from_the_pool_pays_the_locked_cost_and_no_more() {
    let (mut g, card, _) = revelation(0);
    g.players[0].pool.colors = [1, 6, 0, 0, 0];
    g.act(0, paid("WUUUU", Move::WithX(2, Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    cite!(["118.3a"]; assert_eq!(g.players[0].pool.total(), 2, "{{2}}{{W}}{{U}}{{U}} takes five of seven"));
    // X in the cost and a later effect on the same creature do not disturb one another
    let (mut g, run, bear, mana) = wolf_run();
    g.act(0, pump(2, run, bear, &mana, "RGGG")).unwrap();
    let growth = g.put(0, GROWTH, Zone::Hand);
    g.players[0].pool.colors[4] = 1;
    g.act(0, paid("G", Move::Cast { card: growth, targets: vec![vec![Target::Object(bear)]] })).unwrap();
    resolve(&mut g);
    resolve(&mut g);
    assert_eq!((g.power(bear), g.toughness(bear)), (Some(7), Some(5)));
}

#[test]
fn r608_2m_self_shuffle_waits_for_search_then_moves_the_resolving_spell_once() {
    for find in [false, true] {
        let mut g = game();
        let bear = g.put(0, BEAR, Zone::Library);
        let giant = g.put(0, GIANT, Zone::Library);
        let card = g.put(0, ZENITH, Zone::Hand);
        let forests = lands(&mut g, FOREST, 3);
        g.act(0, cast(2, card, &forests, "GGG")).unwrap();
        resolve(&mut g);
        cite!(["608.2n"]; case: format!("find={find}"); assert_eq!(g.stack.len(), 1, "the spell remains on the stack while searching"));
        let copy = g.clone();
        for mut g in [g, copy] {
            let found = if find { vec![bear] } else { vec![] };
            g.act(0, Move::Choose(game::state::Answer::Objects(found))).unwrap();
            // Uncited (card text): "Shuffle Green Sun's Zenith into its owner's library" moves the resolving spell off the stack.
            assert!(g.stack.is_empty());
            // the card shuffled into the library is a new object (400.7); the spell finishes resolving (608.2m), and the spell 608.2n would move no longer exists
            cite!(["400.7", "608.2m"]; assert!(g.players[0].graveyard.is_empty()));
            // Uncited (card text): "put it onto the battlefield, then shuffle. Shuffle Green Sun's Zenith into its owner's library."
            assert_eq!(g.players[0].library.len(), if find { 2 } else { 3 });
            // Uncited (card text): "Shuffle Green Sun's Zenith into its owner's library" is one move of the card.
            assert_eq!(g.players[0].library.iter().filter(|o| g.obj(**o).card == Some(ZENITH)).count(), 1);
            // Uncited (card text): only the chosen card is "put onto the battlefield"; the rest of the library stays.
            assert!(g.players[0].library.contains(&giant));
            // Uncited (card text): "put it onto the battlefield"; finding nothing is allowed by 701.23b.
            assert_eq!(g.battlefield.iter().filter(|o| g.obj(**o).card == Some(BEAR)).count(), usize::from(find));
        }
    }
}
