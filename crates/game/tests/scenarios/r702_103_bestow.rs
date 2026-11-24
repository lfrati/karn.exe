//! Bestow (702.103): a creature card cast for its bestow cost is an Aura spell with enchant
//! creature, the permanent it becomes a bestowed Aura; it becomes a creature again if its
//! target is illegal as it resolves, or once it is not attached, as the same object.
use crate::support::{cite, mana_pool, Board, T};
use game::announcement::CastCost;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::{CardType, Part};

/// The row of the card's bestow ability: the alternative cost it is cast for (702.103a).
fn bestow_row(g: &Game, card: ObjId) -> usize {
    g.def(card).unwrap().abilities().iter().position(|a| matches!(a, ir::Ability::Keywords(items)
        if items.iter().any(|i| i.keyword.name() == "Bestow"))).expect("a bestow ability")
}

/// Cast this card from its owner's hand bestowed, enchanting `on`, paying `mana`.
fn cast_bestowed(t: &mut T, p: usize, name: &str, on: Target, mana: &str) -> Result<(), String> {
    let card = t.find_in(p, Zone::Hand, name);
    let row = bestow_row(&t.g, card);
    let cast = Move::CastFor(CastCost::Alternative(row), Box::new(Move::Cast { card, targets: vec![vec![on]] }));
    t.g.players[p].pool = mana_pool(mana);
    t.g.act(p, Move::Paying(mana_pool(mana), Box::new(cast))).map(|_| ()).map_err(|e| e.to_string())
}

fn on_stack(t: &T, name: &str) -> ObjId {
    *t.g.stack.iter().find(|o| t.g.def(**o).is_some_and(|d| d.name() == name)).unwrap_or_else(|| panic!("no {name} on the stack"))
}

fn satyr_and_bears() -> T {
    Board::new(2).fill(5).at(Part::PrecombatMain)
        .hand(0, &["Boon Satyr", "Murder", "Unsummon"])
        .named(0, Zone::Battlefield, "bears", "Grizzly Bears")
        .build()
}

fn is_aura(g: &Game, o: ObjId) -> bool {
    let seen = g.seen(o, None);
    seen.is(CardType::Enchantment) && seen.subtypes.contains(&ir::Subtype::aura())
}

#[test]
fn r702_103b_a_spell_cast_bestowed_is_an_aura_spell_and_enchants_its_target() {
    let mut t = satyr_and_bears();
    let bears = Target::Object(t.id("bears"));
    cite!(["702.103a", "118.9", "601.2b"]; ok: cast_bestowed(&mut t, 0, "Boon Satyr", bears, "GGGGG"));
    let spell = on_stack(&t, "Boon Satyr");
    // an Aura enchantment, not a creature, with no creature type (702.103b, 205.1a)
    cite!(["702.103b", "205.1a"]; assert!(is_aura(&t.g, spell) && !t.g.is_a(spell, CardType::Creature)));
    cite!(["702.103b", "205.1a"]; assert!(!t.g.seen(spell, None).subtypes.iter().any(|s| s.name() == "Satyr")));
    t.resolve();
    let satyr = t.find_in_battlefield("Boon Satyr");
    cite!(["702.103b", "303.4a", "608.3a"]; assert_eq!(t.g.attached_to(satyr), Some(bears)));
    cite!(["702.103b"]; assert!(is_aura(&t.g, satyr) && !t.g.is_a(satyr, CardType::Creature)));
    // a noncreature permanent has no power or toughness (208.3)
    cite!(["208.3"]; assert_eq!(t.g.power(satyr), None));
    t.expect().rules(["702.103b", "611.3a"]).pt("Grizzly Bears", 6, 4).check();
}

#[test]
fn r702_103a_cast_for_its_mana_cost_it_is_a_creature_spell() {
    let mut t = satyr_and_bears();
    t.g.players[0].pool = mana_pool("GGG");
    t.cast(0, "Boon Satyr", vec![], mana_pool("GGG")).unwrap();
    let spell = on_stack(&t, "Boon Satyr");
    cite!(["702.103a"]; assert!(t.g.is_a(spell, CardType::Creature) && !is_aura(&t.g, spell)));
    t.resolve();
    let satyr = t.find_in_battlefield("Boon Satyr");
    cite!(["702.103a"]; assert_eq!(t.g.attached_to(satyr), None));
    t.expect().rules(["702.103a"]).pt("Boon Satyr", 4, 2).pt("Grizzly Bears", 2, 2).check();
}

#[test]
fn r702_103e_a_bestowed_aura_spell_with_an_illegal_target_resolves_as_a_creature() {
    let mut t = satyr_and_bears();
    let bears = Target::Object(t.id("bears"));
    cast_bestowed(&mut t, 0, "Boon Satyr", bears, "GGGGG").unwrap();
    let spell = on_stack(&t, "Boon Satyr");
    t.g.players[0].pool = mana_pool("U");
    t.cast(0, "Unsummon", vec![bears.into_tgt()], mana_pool("U")).unwrap();
    // Unsummon resolves first; then Boon Satyr's target is gone
    t.pass_all();
    cite!(["702.103e"]; assert!(t.g.stack.contains(&spell)));
    t.resolve();
    let satyr = t.find_in_battlefield("Boon Satyr");
    cite!(["702.103e", "608.3b"]; assert!(t.g.is_a(satyr, CardType::Creature) && !is_aura(&t.g, satyr)));
    cite!(["702.103e"]; assert_eq!(t.g.attached_to(satyr), None));
    t.expect().rules(["702.103e", "608.3b"]).pt("Boon Satyr", 4, 2).contains(0, Zone::Hand, "Grizzly Bears").check();
}

#[test]
fn r702_103f_an_unattached_bestowed_aura_is_the_same_object_as_a_creature() {
    // cast with flash on the opponent's turn, so it is under its controller's control since
    // their turn began
    let mut t = Board::new(2).fill(5).active(1).at(Part::PrecombatMain)
        .hand(0, &["Boon Satyr", "Murder"])
        .named(0, Zone::Battlefield, "bears", "Grizzly Bears")
        .build();
    let bears = Target::Object(t.id("bears"));
    t.g.priority = 1;
    t.g.act(1, Move::Pass).unwrap();
    cite!(["702.8a", "702.103b"]; ok: cast_bestowed(&mut t, 0, "Boon Satyr", bears, "GGGGG"));
    t.resolve();
    let satyr = t.find_in_battlefield("Boon Satyr");
    t.to_no_combat(0, Part::PrecombatMain);
    t.g.players[0].pool = mana_pool("BBB");
    t.cast(0, "Murder", vec![bears.into_tgt()], mana_pool("BBB")).unwrap();
    t.resolve();
    cite!(["702.103f", "400.7"]; assert!(t.g.battlefield.contains(&satyr)));
    cite!(["702.103f"]; assert!(t.g.is_a(satyr, CardType::Creature) && !is_aura(&t.g, satyr)));
    cite!(["702.103f"]; assert_eq!(t.g.attached_to(satyr), None));
    t.expect().rules(["702.103f", "704.5m"]).pt("Boon Satyr", 4, 2).contains(0, Zone::Graveyard, "Grizzly Bears").contains(0, Zone::Graveyard, "Murder").check();
    // it has been under its controller's control continuously since their turn began (302.6)
    while t.g.decision != Some(game::state::Decision::Attackers { player: 0 }) { let p = t.g.priority; t.g.act(p, Move::Pass).unwrap(); }
    cite!(["702.103f", "302.6"]; ok: t.g.act(0, Move::Attack(vec![(satyr, Target::Player(1))])));
}

#[test]
fn r702_103b_its_x_and_counters_are_of_the_bestowed_spell() {
    let mut t = Board::new(2).fill(5).at(Part::PrecombatMain)
        .hand(0, &["Nyxborn Hydra", "Murder"])
        .named(0, Zone::Battlefield, "bears", "Grizzly Bears")
        .build();
    let bears = Target::Object(t.id("bears"));
    let card = t.find_in(0, Zone::Hand, "Nyxborn Hydra");
    let row = bestow_row(&t.g, card);
    let cast = Move::WithX(2, Box::new(Move::CastFor(CastCost::Alternative(row), Box::new(Move::Cast { card, targets: vec![vec![bears]] }))));
    t.g.players[0].pool = mana_pool("GGGG");
    cite!(["702.103a", "107.3a"]; ok: t.g.act(0, Move::Paying(mana_pool("GGGG"), Box::new(cast))));
    t.resolve();
    let hydra = t.find_in_battlefield("Nyxborn Hydra");
    cite!(["107.3m", "702.103b"]; assert_eq!(t.g.attached_to(hydra), Some(bears)));
    t.expect().rules(["107.3m", "702.103b"]).counters("Nyxborn Hydra", "+1/+1", 2).pt("Grizzly Bears", 4, 4)
        .holds(|g| g.has(bears.object(), "Reach") && g.has(bears.object(), "Trample"), "the enchanted creature has reach and trample").check();
    t.g.players[0].pool = mana_pool("BBB");
    t.cast(0, "Murder", vec![bears.into_tgt()], mana_pool("BBB")).unwrap();
    t.resolve();
    // a 0/1 with two +1/+1 counters
    t.expect().rules(["702.103f", "122.1a"]).pt("Nyxborn Hydra", 2, 3).check();
}

#[test]
fn r702_103b_a_bestowed_spell_is_a_noncreature_spell_for_what_targets_spells() {
    let mut t = Board::new(2).fill(5).at(Part::PrecombatMain)
        .hand(0, &["Boon Satyr"])
        .hand(1, &["Essence Scatter", "Negate"])
        .named(0, Zone::Battlefield, "bears", "Grizzly Bears")
        .build();
    let bears = Target::Object(t.id("bears"));
    cast_bestowed(&mut t, 0, "Boon Satyr", bears, "GGGGG").unwrap();
    let spell = Target::Object(on_stack(&t, "Boon Satyr"));
    t.g.act(0, Move::Pass).unwrap();
    let scatter = t.find_in(1, Zone::Hand, "Essence Scatter");
    t.g.players[1].pool = mana_pool("UU");
    cite!(["702.103b", "115.1"]; refused("an illegal target, Boon Satyr: it isn't a creature (115.1)"):
        t.g.act(1, Move::Paying(mana_pool("UU"), Box::new(Move::Cast { card: scatter, targets: vec![vec![spell]] }))));
    t.g.players[1].pool = mana_pool("UU");
    cite!(["702.103b", "115.1"]; ok: t.cast(1, "Negate", vec![spell.into_tgt()], mana_pool("UU")));
    t.resolve();
    t.expect().rules(["702.103b", "701.6a"]).contains(0, Zone::Graveyard, "Boon Satyr").pt("Grizzly Bears", 2, 2).check();
}

#[test]
fn r702_103b_casting_it_bestowed_is_casting_an_aura_spell() {
    let mut t = Board::new(2).fill(5).at(Part::PrecombatMain)
        .hand(0, &["Boon Satyr", "Hopeful Eidolon"])
        .battlefield(0, &["Sram, Senior Edificer"])
        .named(0, Zone::Battlefield, "bears", "Grizzly Bears")
        .build();
    let bears = Target::Object(t.id("bears"));
    let hand = t.g.players[0].hand.len();
    cast_bestowed(&mut t, 0, "Boon Satyr", bears, "GGGGG").unwrap();
    t.resolve();
    cite!(["702.103b", "603.2"]; assert_eq!(t.g.players[0].hand.len(), hand));
    // the Eidolon cast for its mana cost is a creature spell: Sram does not trigger
    t.g.players[0].pool = mana_pool("W");
    t.cast(0, "Hopeful Eidolon", vec![], mana_pool("W")).unwrap();
    t.resolve();
    cite!(["702.103a"]; assert_eq!(t.g.players[0].hand.len(), hand - 1));
}

#[test]
fn r702_103d_only_its_bestowed_characteristics_are_asked_whether_it_can_be_cast() {
    // 702.103d's example: "Creature spells can't be cast" doesn't stop it cast bestowed
    let prohibition = CardDef::new("Creature Ban", "{0}", "Enchantment", "Creature spells can't be cast.", None).unwrap();
    game::play::check(&prohibition).unwrap();
    let mut g = Game::new(vec![crate::support::oracle("Boon Satyr"), crate::support::oracle("Grizzly Bears"), prohibition], 2, 20);
    g.part = Part::PrecombatMain;
    let satyr = g.put(0, 0, Zone::Hand);
    let bears = g.put(0, 1, Zone::Battlefield);
    g.put(1, 2, Zone::Battlefield);
    let mut t = T::new(g);
    t.g.players[0].pool = mana_pool("GGG");
    cite!(["702.103d", "601.3"]; refused("an effect prohibits casting that spell (601.3)"):
        t.g.act(0, Move::Paying(mana_pool("GGG"), Box::new(Move::Cast { card: satyr, targets: vec![] }))));
    cite!(["702.103d"]; ok: cast_bestowed(&mut t, 0, "Boon Satyr", Target::Object(bears), "GGGGG"));
    t.resolve();
    t.expect().rules(["702.103d"]).pt("Grizzly Bears", 6, 4).check();
}

#[test]
fn r702_103c_a_copy_of_a_bestowed_aura_spell_is_bestowed() {
    let mut t = Board::new(2).fill(5).at(Part::PrecombatMain)
        .hand(0, &["Boon Satyr"])
        .battlefield(0, &["Lithoform Engine"])
        .named(0, Zone::Battlefield, "bears", "Grizzly Bears")
        .build();
    let bears = Target::Object(t.id("bears"));
    cast_bestowed(&mut t, 0, "Boon Satyr", bears, "GGGGG").unwrap();
    let spell = Target::Object(on_stack(&t, "Boon Satyr"));
    // "{4}, {T}: Copy target permanent spell you control."
    t.g.players[0].pool = mana_pool("CCCC");
    t.activate(0, "Lithoform Engine", 2, vec![spell.into_tgt()], mana_pool("CCCC")).unwrap();
    t.pass_all();
    let copy = *t.g.stack.last().unwrap();
    cite!(["702.103c", "707.10"]; assert!(copy != spell.object() && is_aura(&t.g, copy) && !t.g.is_a(copy, CardType::Creature)));
    t.resolve();
    let auras: Vec<_> = t.g.battlefield.iter().copied().filter(|o| t.g.attached_to(*o) == Some(bears)).collect();
    cite!(["702.103c"]; assert_eq!(auras.len(), 2));
    t.expect().rules(["702.103c"]).pt("Grizzly Bears", 10, 6).check();
}

#[test]
fn r702_103b_what_it_says_of_itself_as_an_aura_follows_whether_it_is_bestowed() {
    // "Discard a creature card: This creature gets +2/+2 until end of turn. If this permanent
    // is an Aura, enchanted creature gets +2/+2 until end of turn instead."
    for bestowed in [true, false] {
        let mut t = Board::new(2).fill(5).at(Part::PrecombatMain)
            .hand(0, &["Erebos's Emissary"])
            .named(0, Zone::Hand, "fodder", "Grizzly Bears")
            .named(0, Zone::Battlefield, "bears", "Grizzly Bears")
            .build();
        let bears = Target::Object(t.id("bears"));
        if bestowed { cast_bestowed(&mut t, 0, "Erebos's Emissary", bears, "BBBBBB").unwrap(); }
        else { t.g.players[0].pool = mana_pool("BBBB"); t.cast(0, "Erebos's Emissary", vec![], mana_pool("BBBB")).unwrap(); }
        t.resolve();
        let emissary = t.find_in_battlefield("Erebos's Emissary");
        let fodder = t.id("fodder");
        t.g.act(0, Move::Activate { source: emissary, ability: 1, targets: vec![], paying: vec![vec![fodder]] }).unwrap();
        t.resolve();
        if bestowed {
            t.expect().rules(["702.103b", "611.2a"]).pt("Grizzly Bears", 7, 7).check();
        } else {
            t.expect().rules(["702.103a", "611.2a"]).pt("Erebos's Emissary", 5, 5).pt("Grizzly Bears", 2, 2).check();
        }
    }
}

trait AsTgt { fn into_tgt(self) -> crate::support::Tgt<'static>; fn object(self) -> ObjId; }
impl AsTgt for Target {
    fn into_tgt(self) -> crate::support::Tgt<'static> { match self { Target::Object(o) => crate::support::Tgt::Object(o), Target::Player(p) => crate::support::Tgt::Player(p) } }
    fn object(self) -> ObjId { match self { Target::Object(o) => o, Target::Player(_) => panic!("a player") } }
}
