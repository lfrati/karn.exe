//! A prompt shows each characteristic (109.3) of every object its seat may look at: the inset
//! spell a card has without using it (715.2a, 720.2a, 722.2a), supertypes (205.4a), and power,
//! toughness and loyalty off the battlefield (208.3, 306.5a).
use crate::support::{cite, oracle, paid};
use game::card::CastAs;
use game::prompt::{Inset, InsetKind, Prompt};
use game::simulation::Simulation;
use game::snapshot::Object;
use game::{CardDef, Game, Move, ObjId, Zone};
use ir::{CardType, Color, Subtype, Supertype};

fn checked(def: CardDef) -> CardDef {
    game::play::check(&def).unwrap_or_else(|e| panic!("{}: {}", def.name(), e.0));
    def
}

/// What this seat is prompted with, the game as it is now.
fn prompt(g: &Game, seat: usize) -> Prompt { Simulation::new(g.clone()).prompt(seat).unwrap() }

fn object(p: &Prompt, o: ObjId) -> &Object { p.view.objects.iter().find(|x| x.id == o).expect("the seat sees it") }

fn resolve(g: &mut Game) {
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
}

fn adventurer() -> CardDef {
    let normal = CardDef::new("Giant", "{6}{G}", "Creature — Giant", "", Some((7, 7))).unwrap();
    let journey = CardDef::new("Journey", "{2}{G}", "Sorcery — Adventure", "You gain 3 life.", None).unwrap();
    checked(CardDef::adventurer(normal, journey).unwrap())
}

/// The Adventure of [`adventurer`], as printed: its color is its mana cost's (105.2).
fn journey() -> Inset {
    Inset {
        kind: InsetKind::Adventure, name: "Journey".into(), mana_cost: Some("{2}{G}".into()), supertypes: vec![],
        types: vec![CardType::Sorcery], subtypes: vec![Subtype::adventure()], colors: vec![Color::Green],
        text: vec!["You gain 3 life.".into()],
    }
}

#[test]
fn r715_2a_an_adventurer_card_shows_its_adventure_wherever_it_does_not_use_it() {
    let mut g = Game::new(vec![adventurer()], 2, 20);
    let held = g.put(0, 0, Zone::Hand);
    let theirs = g.put(1, 0, Zone::Hand);
    let fielded = g.put(1, 0, Zone::Battlefield);
    let mine = prompt(&g, 0);
    // a card in a hand or on the battlefield has only its normal characteristics (715.4) and
    // still has an Adventure (715.2a)
    cite!(["715.2a", "715.4"]; assert_eq!(mine.entities[&held].inset, Some(journey())));
    cite!(["715.4"]; assert_eq!(mine.entities[&held].mana_cost.as_deref(), Some("{6}{G}")));
    cite!(["715.2a"]; assert_eq!(mine.entities[&fielded].inset, Some(journey())));
    // another player's hand is not shown, nor what its cards' insets say
    cite!(["402.3"]; assert!(!mine.entities.contains_key(&theirs)));
    // cast as its Adventure, the spell has only the alternative characteristics: no inset
    g.players[0].pool.colors[4] = 1;
    g.players[0].pool.colorless = 2;
    g.act(0, paid("CCG", Move::CastAs(CastAs::Adventure, Box::new(Move::Cast { card: held, targets: vec![] })))).unwrap();
    let spell = g.stack[0];
    let seen = prompt(&g, 1);
    cite!(["715.3b"]; assert_eq!((seen.entities[&spell].inset.as_ref(), seen.entities[&spell].mana_cost.as_deref()), (None, Some("{2}{G}"))));
    cite!(["715.3b"]; assert_eq!(object(&seen, spell).name, "Journey"));
    // exiled as it resolves, the card has its normal characteristics and its Adventure again
    resolve(&mut g);
    let exiled = g.exile[0];
    cite!(["715.3d", "715.4"]; assert_eq!(prompt(&g, 1).entities[&exiled].inset, Some(journey())));
    // cast from exile as the creature: on the stack not as an Adventure, it is the card as normal
    g.players[0].pool.colors[4] = 1;
    g.players[0].pool.colorless = 6;
    g.act(0, paid("CCCCCCG", Move::Cast { card: exiled, targets: vec![] })).unwrap();
    cite!(["715.4", "715.2a"]; assert_eq!(prompt(&g, 1).entities[&g.stack[0]].inset, Some(journey())));
}

#[test]
fn r720_2a_an_omen_card_shows_its_omen_until_it_is_cast_as_one() {
    let normal = CardDef::new("Seer", "{3}{U}", "Creature — Human", "", Some((2, 3))).unwrap();
    let glimpse = CardDef::new("Glimpse", "{1}{U}", "Instant — Omen", "Draw a card.", None).unwrap();
    let mut g = Game::new(vec![checked(CardDef::omen_card(normal, glimpse).unwrap())], 2, 20);
    let held = g.put(0, 0, Zone::Hand);
    let omen = Inset {
        kind: InsetKind::Omen, name: "Glimpse".into(), mana_cost: Some("{1}{U}".into()), supertypes: vec![],
        types: vec![CardType::Instant], subtypes: vec![Subtype::omen()], colors: vec![Color::Blue],
        text: vec!["Draw a card.".into()],
    };
    cite!(["720.2a"]; assert_eq!(prompt(&g, 0).entities[&held].inset, Some(omen)));
    g.players[0].pool.colors[1] = 1;
    g.players[0].pool.colorless = 1;
    g.act(0, paid("CU", Move::CastAs(CastAs::Omen, Box::new(Move::Cast { card: held, targets: vec![] })))).unwrap();
    let spell = g.stack[0];
    let seen = prompt(&g, 1);
    cite!(["720.3b"]; assert_eq!((seen.entities[&spell].inset.as_ref(), object(&seen, spell).name.as_str()), (None, "Glimpse")));
}

#[test]
fn r722_2a_a_preparation_card_shows_its_prepare_spell_and_its_prepared_copy_is_only_that_spell() {
    let scholar = CardDef::new("Scholar", "{0}", "Creature — Bird", "This creature enters prepared.", Some((2, 2))).unwrap();
    let lesson = CardDef::new("Lesson", "{U}", "Instant", "Target player gains 3 life.", None).unwrap();
    let mut g = Game::new(vec![checked(CardDef::preparation(scholar, lesson).unwrap())], 2, 20);
    let shown = Inset {
        kind: InsetKind::Prepare, name: "Lesson".into(), mana_cost: Some("{U}".into()), supertypes: vec![],
        types: vec![CardType::Instant], subtypes: vec![], colors: vec![Color::Blue],
        text: vec!["Target player gains 3 life.".into()],
    };
    let held = g.put(0, 0, Zone::Hand);
    cite!(["722.2a"]; assert_eq!(prompt(&g, 0).entities[&held].inset, Some(shown.clone())));
    let fielded = g.put(0, 0, Zone::Battlefield);
    g.as_entered(fielded);
    let copy = *g.exile.iter().find(|c| g.obj(**c).chars.name == "Lesson").expect("it entered prepared");
    let seen = prompt(&g, 1);
    cite!(["722.2a"]; assert_eq!(seen.entities[&fielded].inset, Some(shown)));
    // the copy has only the prepare spell's characteristics, as its normal ones
    cite!(["722.3c"]; assert_eq!((seen.entities[&copy].inset.as_ref(), seen.entities[&copy].mana_cost.as_deref()), (None, Some("{U}"))));
}

#[test]
fn r205_4a_a_prompt_shows_supertypes() {
    let hero = checked(CardDef::new("Hero", "{0}", "Legendary Creature — Human", "", Some((2, 2))).unwrap());
    let forest = checked(CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap());
    let mut g = Game::new(vec![hero, forest], 2, 20);
    let (hero, forest, held) = (g.put(0, 0, Zone::Battlefield), g.put(0, 1, Zone::Battlefield), g.put(0, 0, Zone::Hand));
    let theirs = prompt(&g, 1);
    cite!(["205.4a"]; assert_eq!(object(&theirs, hero).supertypes, [Supertype::Legendary]));
    cite!(["205.4a"]; assert_eq!(object(&theirs, forest).supertypes, [Supertype::Basic]));
    cite!(["205.4a"]; assert_eq!(object(&prompt(&g, 0), held).supertypes, [Supertype::Legendary]));
}

#[test]
fn r208_3_power_and_toughness_off_the_battlefield_are_those_printed() {
    let bear = checked(CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap());
    let cart = checked(CardDef::new("Cart", "{1}", "Artifact — Vehicle", "Crew 1", Some((3, 3))).unwrap());
    let mut g = Game::new(vec![bear, cart], 2, 20);
    let (held, buried) = (g.put(0, 0, Zone::Hand), g.put(1, 0, Zone::Graveyard));
    let (cart_held, parked) = (g.put(0, 1, Zone::Hand), g.put(0, 1, Zone::Battlefield));
    let mine = prompt(&g, 0);
    cite!(["208.1"]; assert_eq!((object(&mine, held).pt, object(&mine, buried).pt), (Some((2, 2)), Some((2, 2)))));
    // a noncreature card off the battlefield has the power and toughness printed on it (208.3),
    // and a noncreature permanent, a Vehicle that isn't a creature included, has none (208.3,
    // 301.7a)
    cite!(["208.3"]; assert_eq!(object(&mine, cart_held).pt, Some((3, 3))));
    cite!(["208.3", "301.7a"]; assert_eq!(object(&mine, parked).pt, None));
    // a creature spell has them too, as its opponent decides whether to answer it
    g.act(0, paid("", Move::Cast { card: held, targets: vec![] })).unwrap();
    cite!(["208.1"]; assert_eq!(object(&prompt(&g, 1), g.stack[0]).pt, Some((2, 2))));
}

#[test]
fn r306_5a_a_planeswalker_card_off_the_battlefield_has_its_printed_loyalty() {
    let walker = checked(CardDef::planeswalker("Walker", "{0}", "Legendary Planeswalker — Jace", "+1: You gain 1 life.", 4).unwrap());
    let mut g = Game::new(vec![walker], 2, 20);
    let (held, buried) = (g.put(0, 0, Zone::Hand), g.put(0, 0, Zone::Graveyard));
    let mine = prompt(&g, 0);
    cite!(["306.5a", "209.1"]; assert_eq!((object(&mine, held).loyalty, object(&mine, buried).loyalty), (Some(4), Some(4))));
    g.act(0, paid("", Move::Cast { card: held, targets: vec![] })).unwrap();
    cite!(["306.5a"]; assert_eq!(object(&prompt(&g, 1), g.stack[0]).loyalty, Some(4)));
    resolve(&mut g);
    let fielded = *g.battlefield.last().expect("the planeswalker resolved");
    let seen = prompt(&g, 1);
    let permanent = object(&seen, fielded);
    // on the battlefield its loyalty is the loyalty counters it entered with, not a printed number
    cite!(["306.5c", "306.5b"]; assert_eq!((permanent.loyalty, permanent.permanent.as_ref().and_then(|p| p.counters.get("loyalty").copied())), (None, Some(4))));
}

#[test]
fn the_pilot_decks_inset_cards_show_their_inset_spells_as_printed() {
    let names = ["Elusive Otter // Grove's Bounty", "Hearth Elemental // Stoke Genius", "Pompous Battlemage // Improvised Act"];
    let mut g = Game::new(names.iter().map(|n| oracle(n)).collect::<Vec<_>>(), 2, 20);
    let held: Vec<ObjId> = (0..3).map(|i| g.put(0, i, Zone::Hand)).collect();
    let fielded: Vec<ObjId> = (0..3).map(|i| g.put(1, i, Zone::Battlefield)).collect();
    // each inset as its Oracle text prints it, without its reminder text (207.2a), and of the
    // color of its own mana cost (105.2)
    let adventure = || vec![Subtype::adventure()];
    let printed = [
        (InsetKind::Adventure, "Grove's Bounty", "{X}{G}", adventure(), Color::Green,
            "Distribute X +1/+1 counters among any number of target creatures you control."),
        (InsetKind::Adventure, "Stoke Genius", "{1}{R}", adventure(), Color::Red, "Discard your hand, then draw two cards."),
        (InsetKind::Prepare, "Improvised Act", "{R}", vec![], Color::Red, "You may discard a card. If you do, draw a card."),
    ];
    let mine = prompt(&g, 0);
    for (o, (kind, name, cost, subtypes, color, text)) in held.iter().chain(&fielded).zip(printed.iter().chain(&printed)) {
        let inset = Inset {
            kind: *kind, name: name.to_string(), mana_cost: Some(cost.to_string()), supertypes: vec![],
            types: vec![CardType::Sorcery], subtypes: subtypes.clone(), colors: vec![*color], text: vec![text.to_string()],
        };
        cite!(["715.2a", "722.2a"]; assert_eq!(mine.entities[o].inset, Some(inset), "{name}"));
    }
}
