//! Omen characteristics and successful completion, derived from CR720 and pinned Oracle.
use crate::support::{cite, paid};
use game::card::CastAs;
use game::state::{Answer, Choice, Decision, DestinationEffect};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use std::sync::LazyLock;
static FACES: LazyLock<Vec<oracle::corpus::Face>> =
    LazyLock::new(|| oracle::corpus::load(&oracle::corpus::default_path()).unwrap());
fn real(name: &str) -> CardDef {
    CardDef::from_faces(&CardDef::catalog(&FACES)[name]).unwrap()
}
fn card(name: &str, kind: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    CardDef::new(name, "{0}", kind, text, pt).unwrap()
}
fn setup(omen: &str, extras: Vec<CardDef>) -> Game {
    let mut defs = vec![real(omen)];
    defs.extend(extras);
    for d in &defs {
        game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0));
    }
    Game::new(defs, 2, 20)
}
fn pay(g: &mut Game, p: usize) {
    g.players[p].pool.colors = [10; 5];
    g.players[p].pool.colorless = 20;
}
fn cast(g: &mut Game, p: usize, card: ObjId, choice: CastAs, targets: Vec<Vec<Target>>, spending: &str) {
    g.act(p, paid(spending, Move::CastAs(choice, Box::new(Move::Cast { card, targets })))).unwrap();
}
fn resolve(g: &mut Game) {
    for _ in 0..2 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn shuffles(g: &Game) -> Vec<(usize, usize)> {
    g.log
        .iter()
        .filter_map(|e| if let Event::Shuffled { player, by } = e { Some((*player, *by)) } else { None })
        .collect()
}

// Given: white Twinmaw, a 4/4 without flying and a flying creature.
// When: cast the red Charring Bite alternative; only a nonflying creature is legal.
// Expect: sorcery/red/value2 on stack, 5 damage; full white creature/value6 in library.
#[test]
fn r720_3a_chosen_cost_colors_types_targets_and_normal_library_characteristics() {
    let mut g = setup(
        "Twinmaw Stormbrood // Charring Bite",
        vec![
            card("Bear", "Creature — Bear", "", Some((4, 4))),
            card("Bird", "Creature — Bird", "Flying", Some((1, 1))),
        ],
    );
    let omen = g.put(0, 0, Zone::Hand);
    let bear = g.put(1, 1, Zone::Battlefield);
    let bird = g.put(1, 2, Zone::Battlefield);
    g.players[0].pool.colors[3] = 1;
    g.players[0].pool.colorless = 1;
    cite!(["720.4","202.2"];assert_eq!(g.seen(omen,None).colors,vec![ir::Color::White]));
    cite!(["720.3a"];refused("legal target"):g.act(0,Move::CastAs(CastAs::Omen,Box::new(Move::Cast{card:omen,targets:vec![vec![Target::Object(bird)]]}))));
    cite!(["720.2","715.2"];refused("no Adventure"):g.act(0,Move::CastAs(CastAs::Adventure,Box::new(Move::Cast{card:omen,targets:vec![vec![Target::Object(bear)]]}))));
    cast(&mut g, 0, omen, CastAs::Omen, vec![vec![Target::Object(bear)]], "CR");
    let spell = g.stack[0];
    cite!(["720.3b","202.2"];assert_eq!(g.seen(spell,None).colors,vec![ir::Color::Red]));
    cite!(["720.3b"];assert_eq!(g.obj(spell).chars.mana_value,2));
    cite!(["720.3b"];assert_eq!(g.def(spell).unwrap().name(),"Charring Bite"));
    cite!(["720.3b"];assert!(g.seen(spell,None).is(ir::CardType::Sorcery)&&!g.seen(spell,None).is(ir::CardType::Creature)));
    resolve(&mut g);
    cite!(["704.5g"];assert_eq!(g.players[1].graveyard.len(),1));
    cite!(["720.3d","701.24e"];assert_eq!(shuffles(&g),vec![(0,0)]));
    let restored = g.players[0].library[0];
    cite!(["720.4"];assert_eq!(g.def(restored).unwrap().name(),"Twinmaw Stormbrood"));
    cite!(["720.4"];assert_eq!(g.obj(restored).chars.mana_value,6));
    cite!(["720.4","202.2"];assert_eq!(g.seen(restored,None).colors,vec![ir::Color::White]));
    cite!(["701.24a"];assert!((0..2).all(|p|!g.sees(p,restored))));
    cite!(["720.3d"];assert!(g.exile.is_empty()&&g.permission(restored).is_none()));
}

// The normal creature resolves its own enters ability, never the Omen instructions.
#[test]
fn r720_4_normal_cast_enters_as_the_dragon_and_never_shuffles() {
    let mut g = setup("Twinmaw Stormbrood // Charring Bite", vec![]);
    let c = g.put(0, 0, Zone::Hand);
    pay(&mut g, 0);
    cast(&mut g, 0, c, CastAs::Whole, vec![], "CCCCCW");
    resolve(&mut g);
    resolve(&mut g);
    cite!(["720.4"];assert_eq!(g.players[0].life,25));
    cite!(["720.4"];assert_eq!(g.seen(g.battlefield[0],None).pt,Some((5,4))));
    cite!(["720.4"];assert!(shuffles(&g).is_empty()&&g.players[0].library.is_empty()));
}

// Petty Revenge is targeted on power<=3; power becomes4 before resolution.
// Its only target is illegal, so it does not resolve and neither destroys nor shuffles.
#[test]
fn r720_3d_all_targets_illegal_and_countering_use_graveyard_without_shuffle() {
    for counter in [false, true] {
        let mut g = setup(
            "Disruptive Stormbrood // Petty Revenge",
            vec![
                card("Bear", "Creature — Bear", "", Some((3, 3))),
                card(
                    "Response",
                    "Instant",
                    if counter { "Counter target spell." } else { "Target creature gets +1/+1 until end of turn." },
                    None,
                ),
            ],
        );
        let c = g.put(0, 0, Zone::Hand);
        let bear = g.put(1, 1, Zone::Battlefield);
        pay(&mut g, 0);
        cast(&mut g, 0, c, CastAs::Omen, vec![vec![Target::Object(bear)]], "CB");
        let spell = g.stack[0];
        g.act(0, Move::Pass).unwrap();
        let response = g.put(1, 2, Zone::Hand);
        let target = if counter { spell } else { bear };
        g.act(1, Move::Cast { card: response, targets: vec![vec![Target::Object(target)]] }).unwrap();
        resolve(&mut g);
        if !counter {
            resolve(&mut g);
        }
        cite!(["608.2b","701.6a","720.3d"];assert_eq!(g.players[0].graveyard.len(),1));
        cite!(["608.2b","701.6a","720.3d"];assert!(shuffles(&g).is_empty()&&g.players[0].library.is_empty()&&g.exile.is_empty()));
        cite!(["608.2b","701.6a"];assert_eq!(g.battlefield,vec![bear]));
    }
}

// A copied Signaling Roar has only the Omen characteristics; its owner is the copier.
// Both spells create their Soldier and shuffle the respective owner's library.
#[test]
fn r720_3c_copied_omen_shuffles_its_owners_empty_library_then_ceases() {
    let mut g =
        setup("Riling Dawnbreaker // Signaling Roar", vec![card("Mirror", "Instant", "Copy target spell.", None)]);
    let c = g.put(0, 0, Zone::Hand);
    pay(&mut g, 0);
    cast(&mut g, 0, c, CastAs::Omen, vec![], "CW");
    let original = g.stack[0];
    g.act(0, Move::Pass).unwrap();
    let copier = g.put(1, 1, Zone::Hand);
    g.act(1, Move::Cast { card: copier, targets: vec![vec![Target::Object(original)]] }).unwrap();
    resolve(&mut g);
    let copy = *g.stack.last().unwrap();
    cite!(["720.3c","707.10"];assert_eq!(g.obj(copy).owner,1));
    cite!(["720.3c"];assert_eq!(g.obj(copy).cast_as,CastAs::Omen));
    cite!(["720.3c"];assert_eq!(g.def(copy).unwrap().name(),"Signaling Roar"));
    cite!(["720.3c"];assert!(g.seen(copy,None).is(ir::CardType::Sorcery)));
    resolve(&mut g);
    cite!(["720.3c","720.3d","701.24e"];assert_eq!(shuffles(&g),vec![(1,1)]));
    cite!(["707.10a","704.5e"];assert!(g.players[1].library.is_empty()));
    cite!(["720.3c"];assert_eq!(g.battlefield.iter().filter(|id|g.controller(**id)==1).count(),1));
    resolve(&mut g);
    cite!(["720.3d"];assert_eq!(shuffles(&g),vec![(1,1),(0,0)]));
    cite!(["720.3d"];assert_eq!(g.players[0].library.len(),1));
    cite!(["720.3c"];assert_eq!(g.battlefield.len(),2));
}

// Generic permission permits another player's Omen. Its caster shuffles the owner's
// library, so 'a player shuffles your library' sees the owner, 'their library' does not.
#[test]
fn r720_3d_controller_shuffles_owner_library_and_trigger_subjects_stay_distinct() {
    let mut g = setup(
        "Riling Dawnbreaker // Signaling Roar",
        vec![
            card("Access", "Sorcery", "Exile target card from a graveyard. You may cast it this turn.", None),
            card("Owner witness", "Artifact", "Whenever a player shuffles your library, you gain 1 life.", None),
            card("Actor witness", "Artifact", "Whenever you shuffle your library, you gain 3 life.", None),
            card("Own library witness", "Artifact", "Whenever a player shuffles their library, you gain 5 life.", None),
        ],
    );
    g.put(1, 2, Zone::Battlefield);
    g.put(0, 3, Zone::Battlefield);
    g.put(1, 4, Zone::Battlefield);
    let c = g.put(1, 0, Zone::Graveyard);
    let access = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card: access, targets: vec![vec![Target::Object(c)]] }).unwrap();
    resolve(&mut g);
    let c = g.exile[0];
    pay(&mut g, 0);
    cast(&mut g, 0, c, CastAs::Omen, vec![], "CW");
    resolve(&mut g);
    cite!(["720.3d","400.3"];assert_eq!(g.players[1].library.len(),1));
    cite!(["720.3d"];assert!(g.players[0].library.is_empty()));
    cite!(["720.3d","603.2"];assert_eq!(shuffles(&g),vec![(1,0)]));
    cite!(["603.2","720.3d"];assert_eq!(g.stack.len(),1));
    resolve(&mut g);
    cite!(["603.2","720.3d"];assert_eq!((g.players[0].life,g.players[1].life),(20,21)));
    let restored = g.players[1].library[0];
    cite!(["720.3d"];assert!(g.permission(restored).is_none()));
}

// Rest in Peace and Omen both replace the successful graveyard move. Applying one
// eliminates the other; the controller chooses, including owner/controller separation.
#[test]
fn r616_1_omen_and_general_graveyard_redirect_are_chosen_before_departure() {
    for choose_omen in [false, true] {
        let mut g = setup("Riling Dawnbreaker // Signaling Roar", vec![real("Rest in Peace")]);
        g.put(1, 1, Zone::Battlefield);
        let c = g.put(0, 0, Zone::Hand);
        pay(&mut g, 0);
        cast(&mut g, 0, c, CastAs::Omen, vec![], "CW");
        let spell = g.stack[0];
        resolve(&mut g);
        cite!(["616.1e","720.3d"];assert!(matches!(&g.decision,Some(Decision::Choose{player:0,choice:Choice::SpellDestination{on,among}})if *on==spell&&among.contains(&DestinationEffect::Omen)&&among.contains(&DestinationEffect::GraveyardReplacement(0)))));
        cite!(["608.2c","616.1"];assert_eq!(g.battlefield.len(),2));
        cite!(["616.1"];assert_eq!(g.obj(spell).zone,Zone::Stack));
        let effect = if choose_omen { DestinationEffect::Omen } else { DestinationEffect::GraveyardReplacement(0) };
        cite!(["616.1e"];refused("another player's"):g.act(1,Move::Choose(Answer::SpellDestination{on:spell,effect})));
        g.act(0, Move::Choose(Answer::SpellDestination { on: spell, effect })).unwrap();
        cite!(["616.1g","720.3d"];assert_eq!(g.players[0].library.len(),usize::from(choose_omen)));
        cite!(["616.1g","720.3d"];assert_eq!(shuffles(&g).len(),usize::from(choose_omen)));
        cite!(["616.1g","614.6"];assert_eq!(g.exile.len(),usize::from(!choose_omen)));
    }
}

// Synthetic mechanism fixture: the pinned Omen cards do not print flashback,
// so this custom alternative explicitly provides it. Both replacement orders exile; only Omen-first shuffles.
#[test]
fn r701_24c_flashback_redirect_preserves_the_selected_omen_shuffle() {
    let main = card("Dragon", "Creature — Dragon", "", Some((3, 3)));
    let alt = CardDef::new("Omen", "{0}", "Sorcery — Omen", "Flashback {0}\nYou gain 1 life.", None).unwrap();
    let def = CardDef::omen_card(main, alt).unwrap();
    game::play::check(&def).unwrap();
    for effect in [DestinationEffect::Omen, DestinationEffect::Flashback] {
        let mut g = Game::new(vec![def.clone()], 2, 20);
        let c = g.put(0, 0, Zone::Graveyard);
        g.act(0, Move::CastVia(game::announcement::CastRoute::Flashback, Box::new(Move::CastAs(CastAs::Omen, Box::new(Move::Cast { card: c, targets: vec![] }))))).unwrap();
        let spell = g.stack[0];
        resolve(&mut g);
        cite!(["616.1e","720.3d","702.34a"];assert!(matches!(&g.decision,Some(Decision::Choose{choice:Choice::SpellDestination{among,..},..})if among==&vec![DestinationEffect::Omen,DestinationEffect::Flashback])));
        g.act(0, Move::Choose(Answer::SpellDestination { on: spell, effect })).unwrap();
        cite!(["702.34a","701.24c"];assert_eq!(g.exile.len(),1));
        cite!(["701.24c","616.1g"];assert_eq!(shuffles(&g).len(),usize::from(effect==DestinationEffect::Omen)));
        cite!(["608.2c"];assert_eq!(g.players[0].life,21));
    }
}

// No selected targets is distinct from a selected target becoming illegal. Skimming Strike
// always draws then shuffles when cast without its optional target.
#[test]
fn r608_2b_zero_optional_targets_resolve_and_shuffle() {
    let mut g = setup("Dirgur Island Dragon // Skimming Strike", vec![real("Forest")]);
    g.put(0, 1, Zone::Library);
    let c = g.put(0, 0, Zone::Hand);
    pay(&mut g, 0);
    cast(&mut g, 0, c, CastAs::Omen, vec![vec![]], "CU");
    resolve(&mut g);
    cite!(["608.2b","720.3d"];assert_eq!(g.players[0].hand.len(),1));
    cite!(["608.2b","720.3d"];assert_eq!(g.players[0].library.len(),1));
    cite!(["720.3d"];assert_eq!(shuffles(&g),vec![(0,0)]));
}

// Normal creature copiable values retain the alternative even though it is not active.
#[test]
fn r720_2b_copying_normal_creature_retains_omen_characteristics() {
    let mut g = setup(
        "Riling Dawnbreaker // Signaling Roar",
        vec![card("Mirror", "Sorcery", "Create a token that's a copy of target creature.", None)],
    );
    let original = g.put(0, 0, Zone::Battlefield);
    let copier = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card: copier, targets: vec![vec![Target::Object(original)]] }).unwrap();
    resolve(&mut g);
    let copy = *g.battlefield.last().unwrap();
    cite!(["720.2b","707.2"];assert_eq!(g.def(copy).unwrap().omen().unwrap().name(),"Signaling Roar"));
    cite!(["720.4","707.2"];assert_eq!(g.seen(copy,None).pt,Some((3,4))));
    cite!(["720.4"];assert!(shuffles(&g).is_empty()));
}

// General completion replacement arbitration also applies to ordinary Buyback spells.
#[test]
fn r616_1_buyback_and_general_graveyard_redirect_share_completion_arbitration() {
    let spell = card("Boon", "Instant", "Buyback {0}\nYou gain 1 life.", None);
    for buyback in [false, true] {
        let mut g = Game::new(vec![spell.clone(), real("Rest in Peace")], 2, 20);
        g.put(1, 1, Zone::Battlefield);
        let c = g.put(0, 0, Zone::Hand);
        g.act(0, Move::WithBuyback(true, Box::new(Move::Cast { card: c, targets: vec![] }))).unwrap();
        let spell = g.stack[0];
        resolve(&mut g);
        cite!(["616.1e","702.27a"];assert!(matches!(&g.decision,Some(Decision::Choose{choice:Choice::SpellDestination{among,..},..})if among.contains(&DestinationEffect::Buyback)&&among.contains(&DestinationEffect::GraveyardReplacement(0)))));
        let effect = if buyback { DestinationEffect::Buyback } else { DestinationEffect::GraveyardReplacement(0) };
        g.act(0, Move::Choose(Answer::SpellDestination { on: spell, effect })).unwrap();
        cite!(["616.1g","702.27a"];assert_eq!(g.players[0].hand.len(),usize::from(buyback)));
        cite!(["616.1g","614.6"];assert_eq!(g.exile.len(),usize::from(!buyback)));
        cite!(["614.6","701.13a"];assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Exiled{object}if *object==spell)).count(),usize::from(!buyback)));
        cite!(["608.2c"];assert_eq!(g.players[0].life,21));
    }
}
