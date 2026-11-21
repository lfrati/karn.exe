//! A condition said after its act ("… if it's blue") is of what that act names, as the text
//! says it: it sees what was named before the act and the act's own targets, and among them
//! what its own sentence named first (608.2c); nothing named after it. Whatever kind of act it is.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

fn card(name: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    let d = CardDef::new(name, "{0}", types, text, pt).unwrap_or_else(|e| panic!("{name}: {e}"));
    game::play::check(&d).unwrap_or_else(|u| panic!("{name}: {}", u.0));
    d
}

const BOUNCE: usize = 0;
const GROW: usize = 1;
const TAP_THEN_DESTROY: usize = 2;
const DESTROY_THEN_TAP: usize = 3;
const BLUE_BEAR: usize = 4;
const RED_BEAR: usize = 5;
const GOLEM: usize = 6;
const ROCK: usize = 7;

fn game() -> Game {
    let defs = vec![
        card("Bounce", "Instant", "Return target creature to its owner's hand if it's blue.", None),
        card("Grow", "Instant", "Put a +1/+1 counter on target creature if it's blue.", None),
        card("Tap Then Destroy", "Instant", "Tap target artifact. Destroy target creature if it's an artifact.", None),
        card("Destroy Then Tap", "Instant", "Destroy target creature if it's blue. Tap target artifact.", None),
        card("Blue Bear", "Creature — Bear", "", Some((2, 2))),
        card("Red Bear", "Creature — Bear", "", Some((2, 2))),
        card("Golem", "Artifact Creature — Golem", "", Some((3, 3))),
        card("Rock", "Artifact", "", None),
    ];
    let mut defs = defs;
    // the bears' colors are their mana costs'
    defs[BLUE_BEAR] = CardDef::new("Blue Bear", "{1}{U}", "Creature — Bear", "", Some((2, 2))).unwrap();
    defs[RED_BEAR] = CardDef::new("Red Bear", "{1}{R}", "Creature — Bear", "", Some((2, 2))).unwrap();
    Game::new(defs, 2, 20)
}

fn cast(g: &mut Game, spell: usize, targets: &[ObjId]) {
    let card = g.put(0, spell, Zone::Hand);
    g.act(0, Move::Cast { card, targets: targets.iter().map(|t| vec![Target::Object(*t)]).collect() }).unwrap_or_else(|e| panic!("cast: {e}"));
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
}

#[test]
fn r608_2c_a_condition_after_a_bounce_or_a_counter_is_of_that_acts_target() {
    for (bear, blue) in [(BLUE_BEAR, true), (RED_BEAR, false)] {
        let mut g = game();
        let target = g.put(1, bear, Zone::Battlefield);
        cast(&mut g, BOUNCE, &[target]);
        cite!(["608.2c"]; case: format!("blue={blue}"); assert_eq!(g.obj(target).zone != Zone::Battlefield, blue, "bounced if blue"));
        let mut g = game();
        let target = g.put(1, bear, Zone::Battlefield);
        cast(&mut g, GROW, &[target]);
        cite!(["608.2c"]; case: format!("blue={blue}"); assert_eq!(g.obj(target).counters.get("+1/+1").copied().unwrap_or(0), u32::from(blue), "counter if blue"));
    }
}

#[test]
fn r608_2c_it_is_what_its_own_sentence_named_not_an_earlier_target() {
    // "Tap target artifact. Destroy target creature if it's an artifact.": the creature, not
    // the artifact tapped before — a non-artifact creature survives beside an artifact target
    for (creature, destroyed) in [(GOLEM, true), (RED_BEAR, false)] {
        let mut g = game();
        let rock = g.put(1, ROCK, Zone::Battlefield);
        let target = g.put(1, creature, Zone::Battlefield);
        cast(&mut g, TAP_THEN_DESTROY, &[rock, target]);
        assert!(g.obj(rock).tapped);
        cite!(["608.2c"]; case: &g.obj(target).chars.name; assert_eq!(g.obj(target).zone != Zone::Battlefield, destroyed, "{}", g.obj(target).chars.name));
    }
}

#[test]
fn r608_2c_a_counter_then_exile_rider_is_unchanged() {
    // Dissipate: a condition of what happened, not of a named thing; the countered spell is
    // exiled instead of going to its owner's graveyard
    let mut defs = (0..8).map(|i| game().defs[i].clone()).collect::<Vec<_>>();
    defs.push(card("Dissipate", "Instant", "Counter target spell. If that spell is countered this way, exile it instead of putting it into its owner's graveyard.", None));
    let mut g = Game::new(defs, 2, 20);
    let bear = g.put(0, BLUE_BEAR, Zone::Hand);
    g.add_mana(0, ir::mana::ManaSymbol::Color(ir::Color::Blue));
    g.add_mana(0, ir::mana::ManaSymbol::Colorless);
    g.act(0, paid("CU", Move::Cast { card: bear, targets: vec![] })).unwrap();
    let spell = *g.stack.last().unwrap();
    cast(&mut g, 8, &[spell]);
    cite!(["608.2c"]; assert!(g.exile.iter().any(|o| g.obj(*o).chars.name == "Blue Bear"), "exiled"));
    cite!(["608.2c"]; assert!(g.players[0].graveyard.iter().all(|o| g.obj(*o).chars.name != "Blue Bear"), "not in the graveyard"));
}

#[test]
fn r608_2c_a_target_named_after_the_condition_is_not_what_it_means() {
    // "Destroy target creature if it's blue. Tap target artifact.": the artifact named after
    // is not "it", and the targets are announced in printed order (creature, then artifact)
    for (bear, destroyed) in [(BLUE_BEAR, true), (RED_BEAR, false)] {
        let mut g = game();
        let target = g.put(1, bear, Zone::Battlefield);
        let rock = g.put(1, ROCK, Zone::Battlefield);
        cast(&mut g, DESTROY_THEN_TAP, &[target, rock]);
        assert!(g.obj(rock).tapped, "the second target is the artifact");
        cite!(["608.2c"]; case: format!("destroyed={destroyed}"); assert_eq!(g.obj(target).zone != Zone::Battlefield, destroyed));
    }
}
