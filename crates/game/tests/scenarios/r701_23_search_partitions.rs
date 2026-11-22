//! Fixed searched-card groups and one/other allocation (701.23, 608.2c-d).
use crate::support::cite;
use game::card::CastAs;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, Zone};
use std::sync::LazyLock;

static FACES: LazyLock<Vec<oracle::corpus::Face>> =
    LazyLock::new(|| oracle::corpus::load(&oracle::corpus::default_path()).unwrap());
fn real(name: &str) -> CardDef {
    CardDef::from_faces(&CardDef::catalog(&FACES)[name]).unwrap()
}

fn setup() -> Game {
    let mut defs = vec![real("Bloomvine Regent // Claim Territory"), real("Troop of Ponies"),
                        real("Flourishing Bloom-Kin"), real("Forest")];
    // Synthetic mechanism fixtures exercise a prohibited entry and a suspended entry
    // choice. Their text is not claimed as a Standard Oracle consumer.
    defs.push(CardDef::new("Sapling", "{0}", "Creature — Plant", "", Some((2, 2))).unwrap());
    defs.push(CardDef::new("Creature partition", "{0}", "Sorcery",
        "Search your library for up to two creature cards, reveal them, put one onto the battlefield tapped and the other into your hand, then shuffle.", None).unwrap());
    defs.push(CardDef::new("Entry restriction", "{0}", "Artifact",
        "Creature cards in libraries can't enter the battlefield.", None).unwrap());
    defs.push(CardDef::new("Choosing Forest", "", "Basic Land — Forest",
        "As this land enters, choose a color.\nThis land enters with two +1/+1 counters on it.", None).unwrap());
    defs.push(CardDef::new("Second Forest", "", "Basic Land — Forest", "", None).unwrap());
    defs.push(CardDef::new("Second Sapling", "{0}", "Creature — Plant", "", Some((2, 2))).unwrap());
    for def in &defs { game::play::check(def).unwrap_or_else(|e| panic!("{}: {}", def.name(), e.0)); }
    Game::new(defs, 2, 20)
}
fn resolve(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
fn claim(g: &mut Game) {
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[4] = 3;
    g.act(0, crate::support::paid("GGG", Move::CastAs(CastAs::Omen, Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    resolve(g);
}
fn choose(g: &mut Game, cards: Vec<ObjId>) { g.act(0, Move::Choose(Answer::Objects(cards))).unwrap(); }
fn shuffles(g: &Game) -> usize { g.log.iter().filter(|e| matches!(e, Event::Shuffled { player: 0, by: 0 })).count() }
fn allocation(g: &Game) -> Vec<ObjId> {
    match &g.decision {
        Some(Decision::Choose { player: 0, choice: Choice::Objects { count: 1, among, .. } }) => among.clone(),
        other => panic!("expected one-card allocation: {other:?}"),
    }
}

#[test]
fn r701_23b_zero_found_still_shuffles_the_library_and_resolving_omen_separately() {
    let mut g = setup();
    g.put(0, 3, Zone::Library);
    claim(&mut g);
    choose(&mut g, vec![]);
    cite!(["701.23b", "608.2c"]; assert!(g.battlefield.is_empty() && g.players[0].hand.is_empty()));
    let forest = *g.players[0].library.iter().find(|id| g.obj(**id).chars.name == "Forest").unwrap();
    assert_eq!(g.obj(forest).zone, Zone::Library);
    cite!(["720.3d", "701.24d"]; assert_eq!(shuffles(&g), 2));
    assert_eq!(g.players[0].library.len(), 2, "Forest plus the completed Omen card");
    assert!(!g.sees(1, forest));
}

#[test]
fn r608_2c_unrepresented_partition_cannot_reuse_older_or_later_movement_tallies() {
    let partition = "Search your library for up to two basic land cards, reveal them, put one onto the battlefield tapped and the other into your hand, then shuffle.";
    for condition in ["", "If you control a Forest, "] {
        for later in ["", " Return target creature to its owner's hand."] {
            for tally in ["You gain 1 life for each creature returned this way.", "Draw that many cards."] {
                let partition = if condition.is_empty() { partition.to_owned() } else { format!("{condition}s{}", &partition[1..]) };
                let text = format!("Return target creature to its owner's hand. {partition}{later} {tally}");
                let def = CardDef::new("Incomplete movement tally", "{0}", "Sorcery", &text, None).unwrap_or_else(|e| panic!("{text}: {e}"));
                let error = game::play::check(&def).unwrap_err();
                cite!(["608.2c"]; assert!(error.0.contains("complete search-partition movement result not modeled"), "{text}: {error:?}"));
            }
        }
    }
}

#[test]
fn r608_2c_one_found_must_enter_tapped_and_cannot_be_allocated_to_hand() {
    // Ponies ruling 6bd12dbd772a165c and Bloom-Kin 90adcca9d80dfcfd fix this
    // consequence of the shared one/other instruction independently of engine output.
    let mut g = setup();
    let forest = g.put(0, 3, Zone::Library);
    claim(&mut g);
    choose(&mut g, vec![forest]);
    cite!(["608.2c", "609.3"]; assert!(g.decision.is_none() && g.players[0].hand.is_empty()));
    assert_eq!(g.battlefield.len(), 1);
    let entered = g.battlefield[0];
    assert_eq!(g.obj(entered).chars.name, "Forest");
    assert!(g.obj(entered).tapped);
    cite!(["400.7"]; assert_ne!(entered, forest));
    assert_eq!(shuffles(&g), 2);
}

#[test]
fn r608_2d_two_found_are_public_during_allocation_and_each_fixed_member_moves_once() {
    for reverse in [false, true] {
        let mut g = setup();
        let first = g.put(0, 3, Zone::Library);
        let second = g.put(0, 8, Zone::Library);
        claim(&mut g);
        choose(&mut g, vec![first, second]);
        cite!(["608.2d"]; assert_eq!(allocation(&g), vec![first, second]));
        cite!(["701.20a"]; assert!(g.sees(1, first) && g.sees(1, second)));
        assert_eq!(g.obj(first).zone, Zone::Library);
        assert_eq!(g.obj(second).zone, Zone::Library);
        let before = format!("{g:?}");
        cite!(["608.2d"]; refused("choose each object to put once"): g.act(0, Move::Choose(Answer::Objects(vec![first, first]))));
        assert_eq!(format!("{g:?}"), before);
        let (bf, hand) = if reverse { (second, first) } else { (first, second) };
        choose(&mut g, vec![bf]);
        assert_eq!(g.battlefield.len(), 1);
        assert!(g.obj(g.battlefield[0]).tapped);
        assert_eq!(g.obj(g.battlefield[0]).chars.name, if reverse { "Second Forest" } else { "Forest" });
        assert_ne!(g.battlefield[0], bf);
        assert_eq!(g.players[0].hand.len(), 1);
        assert_ne!(g.players[0].hand[0], hand);
        assert_eq!(g.obj(g.players[0].hand[0]).chars.name, if reverse { "Forest" } else { "Second Forest" });
        assert_eq!(g.players[0].library.len(), 1, "only the resolving Omen remains");
        cite!(["701.20a", "400.7"]; assert!(!g.sees(1, g.players[0].hand[0])));
        assert_eq!(shuffles(&g), 2);
    }
}

#[test]
fn r609_3_prohibited_entry_does_not_reassign_its_allocated_member_to_hand() {
    for count in [1, 2] {
        let mut g = setup();
        g.put(1, 6, Zone::Battlefield);
        let cards: Vec<_> = (0..count).map(|n| g.put(0, if n == 0 { 4 } else { 9 }, Zone::Library)).collect();
        let spell = g.put(0, 5, Zone::Hand);
        g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
        resolve(&mut g);
        choose(&mut g, cards.clone());
        if count == 2 {
            assert_eq!(allocation(&g), cards);
            choose(&mut g, vec![cards[0]]);
        }
        cite!(["609.3", "614.17"]; assert_eq!(g.players[0].library.len(), 1));
        assert_eq!(g.obj(g.players[0].library[0]).chars.name, "Sapling");
        assert_eq!(g.players[0].hand.len(), count - 1);
        if count == 2 { assert_eq!(g.obj(g.players[0].hand[0]).chars.name, "Second Sapling"); }
        assert_eq!(g.battlefield.len(), 1, "only the entry restriction remains");
        cite!(["701.20d"]; assert!(!g.sees(1, g.players[0].library[0])));
        assert_eq!(shuffles(&g), 1);
    }
}

#[test]
fn r614_12_entry_choice_preserves_reveal_and_fixed_hand_member_until_entry_completes() {
    let mut g = setup();
    let choosing = g.put(0, 7, Zone::Library);
    let other = g.put(0, 3, Zone::Library);
    claim(&mut g);
    choose(&mut g, vec![choosing, other]);
    choose(&mut g, vec![choosing]);
    assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::Value(_) })));
    cite!(["614.12", "608.2c"]; assert!(g.players[0].hand.is_empty()));
    assert_eq!(g.obj(other).zone, Zone::Library);
    cite!(["701.20a"]; assert!(g.sees(1, other)));
    g.act(0, Move::Choose(Answer::Value(game::binding::Value::Color(ir::Color::Blue)))).unwrap();
    assert_eq!(g.obj(g.battlefield[0]).chars.name, "Choosing Forest");
    assert!(g.obj(g.battlefield[0]).tapped);
    cite!(["614.1c"]; assert_eq!(g.obj(g.battlefield[0]).counters.get("+1/+1"), Some(&2)));
    assert_eq!(g.obj(g.players[0].hand[0]).chars.name, "Forest");
    cite!(["701.20a"]; assert!(!g.sees(1, g.players[0].hand[0])));
    assert_eq!(shuffles(&g), 2);
}

#[test]
fn r701_23_partition_refuses_unproved_source_groups_and_larger_singular_complements() {
    for text in [
        "Search your library for up to three cards, reveal them, put one onto the battlefield tapped and the other into your hand, then shuffle.",
        "Search your graveyard, hand, and library for up to two cards, reveal them, put one onto the battlefield tapped and the other into your hand, then shuffle.",
    ] {
        let def = CardDef::new("Unsupported partition", "{0}", "Sorcery", text, None).unwrap();
        let error = game::play::check(&def).unwrap_err();
        assert!(error.0.contains("one mandatory library result of at most two cards"), "{error:?}");
    }
    // Entry choices/counters are exercised above; destination redirection remains a
    // separate unmodeled replacement, rather than a claim made by this evidence.
    let def = CardDef::new("Unmodeled entry destination", "{0}", "Enchantment",
        "If a creature would enter the battlefield, exile it instead.", None).unwrap();
    crate::support::refused(game::play::check(&def).map_err(|e|e.0),"ability on a permanent: sentence: Replacement");
}

#[test]
fn r608_2d_revealed_search_group_supplies_only_the_explicitly_chosen_hand_card() {
    let spell = CardDef::new("Selected search", "{0}", "Sorcery",
        "Search your library for up to two cards, reveal them, put one into your hand, then shuffle.", None).unwrap();
    game::play::check(&spell).unwrap();
    for count in [0, 2] {
        let mut g = Game::new(vec![spell.clone(), real("Forest"), real("Island"), real("Plains")], 2, 20);
        let forest = g.put(0, 1, Zone::Library);
        let island = g.put(0, 2, Zone::Library);
        let hidden = g.put(0, 3, Zone::Library);
        let card = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
        resolve(&mut g);
        let found = [forest, island][..count].to_vec();
        cite!(["701.23a"]; assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::Search { min: 0, max: 2, .. }, .. }))));
        choose(&mut g, found.clone());
        if count == 2 {
            cite!(["608.2d"]; assert_eq!(allocation(&g), found));
            cite!(["701.20a", "701.20b"]; assert!(g.sees(1, forest) && g.sees(1, island) && !g.sees(1, hidden)));
            let before = format!("{g:?}");
            cite!(["608.2d"]; refused("not among the offered objects"): g.act(0, Move::Choose(Answer::Objects(vec![hidden]))));
            assert_eq!(format!("{g:?}"), before);
            choose(&mut g, vec![forest]);
            cite!(["608.2c"]; assert_eq!(g.players[0].hand.len(), 1));
            assert_eq!(g.obj(g.players[0].hand[0]).chars.name, "Forest");
            assert_eq!(g.players[0].library.iter().map(|o| g.obj(*o).chars.name.as_str()).collect::<std::collections::BTreeSet<_>>(),
                       ["Island", "Plains"].into_iter().collect());
        } else {
            cite!(["609.3"]; assert!(g.players[0].hand.is_empty() && g.decision.is_none()));
            assert_eq!(g.players[0].library.len(), 3);
        }
        cite!(["701.24d"]; assert_eq!(shuffles(&g), 1));
        cite!(["701.20d"]; assert!(!g.players[0].library.iter().any(|o| g.sees(1, *o))));
        assert!(g.stack.is_empty());
    }
}

#[test]
fn r701_23_ponies_sacrificed_source_retains_its_controller_and_partition_instruction() {
    let mut g = setup();
    let pony = g.put(0, 1, Zone::Battlefield);
    let forest = g.put(0, 3, Zone::Library);
    let other = g.put(0, 8, Zone::Library);
    g.players[0].pool.colorless = 2;
    g.act(0, crate::support::paid("CC", Move::Activate { source: pony, ability: 0, targets: vec![], paying: vec![] })).unwrap();
    resolve(&mut g);
    choose(&mut g, vec![forest, other]);
    choose(&mut g, vec![other]);
    cite!(["602.2b", "113.7a"]; assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).chars.name == "Troop of Ponies")));
    assert_eq!(g.obj(g.battlefield[0]).chars.name, "Second Forest");
    assert!(g.obj(g.battlefield[0]).tapped);
    assert_eq!(g.obj(g.players[0].hand[0]).chars.name, "Forest");
    assert_eq!(shuffles(&g), 1);
}

#[test]
fn r702_168_bloom_kin_turn_up_trigger_uses_the_same_fixed_search_partition() {
    use game::announcement::CastCost;
    use game::face::Way;
    let mut g = setup();
    g.put(0, 3, Zone::Battlefield); // Keeps face-up 0/0 Bloom-Kin alive through its Forest bonus.
    let forest = g.put(0, 3, Zone::Library);
    let other = g.put(0, 8, Zone::Library);
    let kin = g.put(0, 2, Zone::Hand);
    g.players[0].pool.colorless = 3;
    g.act(0, crate::support::paid("CCC", Move::CastFor(CastCost::Alternative(1), Box::new(Move::Cast { card: kin, targets: vec![] })))).unwrap();
    resolve(&mut g);
    let down = *g.battlefield.iter().find(|o| g.face_down(**o)).unwrap();
    g.players[0].pool.colors[4] = 5;
    g.act(0, crate::support::paid("GGGGG", Move::TurnFaceUp(down, Way::Disguise))).unwrap();
    resolve(&mut g);
    choose(&mut g, vec![forest, other]);
    choose(&mut g, vec![forest]);
    assert!(g.battlefield.iter().any(|o| g.obj(*o).chars.name == "Forest" && g.obj(*o).tapped));
    assert_eq!(g.obj(g.players[0].hand[0]).chars.name, "Second Forest");
    cite!(["702.168d", "603.2"]; assert_eq!((g.power(down), g.toughness(down)), (Some(2), Some(2))));
    assert_eq!(shuffles(&g), 1);
}
