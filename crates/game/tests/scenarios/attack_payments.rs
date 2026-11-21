//! Fixed per-attacker costs share payment continuations (508.1h-j) and never force payment.
use crate::support::{cite,mana_pool};
use game::{CardDef, Game, Move, Target, Zone, Event};
use game::state::{Answer, Choice, Decision};
use game::legal::Asked;
use ir::Part;

const TAX: &str = "Creatures can't attack you unless their controller pays {2} for each creature they control that's attacking you.";
fn game(n: usize) -> Game {
    let defs = vec![
        CardDef::new("Ghostly Prison", "{2}{W}", "Enchantment", TAX, None).unwrap(),
        CardDef::new("Propaganda", "{2}{U}", "Enchantment", TAX, None).unwrap(),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2,2))).unwrap(),
        CardDef::new("Mountain", "", "Basic Land — Mountain", "", None).unwrap(),
        CardDef::planeswalker("Jace Beleren", "{1}{U}{U}", "Legendary Planeswalker — Jace", "+2: Each player draws a card.\n−1: Target player draws a card.\n−10: Target player mills twenty cards.", 3).unwrap(),
        CardDef::new("Required", "{R}", "Creature — Warrior", "This creature attacks each combat if able.", Some((2,2))).unwrap(),
        CardDef::new("Vigilant", "{W}", "Creature — Elf", "Vigilance\n{T}: Add {W}.", Some((2,2))).unwrap(),
        CardDef::new("Mana creature", "{0}", "Creature — Elf", "{T}: You gain 2 life. Put a +1/+1 counter on this creature. Add X mana of any one color, where X is the number of +1/+1 counters on this creature.", Some((1,1))).unwrap(),
        CardDef::new("Hardened Scales", "{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.", None).unwrap(),
        CardDef::new("Branching Evolution", "{2}{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.", None).unwrap(),
        CardDef::new("Tap witness", "{G}", "Creature — Bear", "Whenever this creature becomes tapped, you gain 1 life.", Some((2,2))).unwrap(),
        CardDef::new("Ashnod's Altar", "{3}", "Artifact", "Sacrifice a creature: Add {C}{C}.", None).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap_or_else(|e| panic!("{}: {}", def.name(),e.0)); }
    Game::new(defs,n,20)
}
fn ready(g: &mut Game) {
    for _ in 0..2 { for _ in 0..g.players.len() { g.act(g.priority,Move::Pass).unwrap(); } }
    assert_eq!(g.part,Part::DeclareAttackers);
}
fn generic(g: &Game) -> u32 {
    let Some(Decision::Choose { choice: Choice::Pay { cost, .. }, .. }) = &g.decision else { panic!("{:?}",g.decision) };
    cost.0.iter().map(|s| match s { ir::mana::ManaSymbol::Generic(n)=>*n,_=>panic!() }).sum()
}
#[test]
fn full_cards_stack_per_creature_and_only_on_the_attacked_player() {
    let mut g=game(3);
    g.put(1,0,Zone::Battlefield); g.put(1,1,Zone::Battlefield); g.put(2,1,Zone::Battlefield);
    let a=g.put(0,2,Zone::Battlefield); let b=g.put(0,2,Zone::Battlefield); let c=g.put(0,2,Zone::Battlefield);
    ready(&mut g);
    g.players[0].pool.colorless=10;
    g.act(0,Move::Attack(vec![(a,Target::Player(1)),(b,Target::Player(1)),(c,Target::Player(2))])).unwrap();
    cite!(["508.1h"]; assert_eq!(generic(&g),10)); assert!(g.combat.attackers.is_empty());
    cite!(["508.1f"]; assert!(g.obj(a).tapped)); crate::support::refused(g.act(1,Move::Choose(Answer::Yes)), "that decision is another player's");
    crate::support::refused(g.act(0,Move::Pass), "a decision is pending: a choice of the effect being applied (608.2d)");
    g.act(0,Move::Choose(Answer::Pay { plan: vec![], with: Some(mana_pool("{10}")), tapped: vec![] })).unwrap();
    cite!(["508.1k"]; assert_eq!(g.combat.attackers.len(),3)); cite!(["508.1j"]; assert_eq!(g.players[0].pool.colorless,0));
    assert_eq!(g.log.iter().filter(|e|matches!(e,Event::AttackersDeclared{..})).count(),1);
}
#[test]
fn player_tax_does_not_tax_planeswalker_and_offers_include_both() {
    for tax in [0,1] {
        let mut g=game(2); g.put(1,tax,Zone::Battlefield);
        let walker=g.put(1,4,Zone::Battlefield); g.put_counters(walker,"loyalty",3);
        let bear=g.put(0,2,Zone::Battlefield); ready(&mut g);
        let Some((0,Asked::Attackers(able)))=g.asked() else {panic!()};
        cite!(["508.1b"]; case: format!("tax {tax}"); assert!(able[0].attacks.contains(&Target::Object(walker))));
        assert!(able[0].costs.iter().any(|(t,c)| *t==Target::Object(walker)&&c.0.is_empty()));
        g.act(0,Move::Attack(vec![(bear,Target::Object(walker))])).unwrap();
        assert!(g.decision.is_none()); cite!(["508.1b", "508.1k"]; case: format!("tax {tax}"); assert_eq!(g.combat.attackers,vec![(bear,Target::Object(walker))]));
    }
}
#[test]
fn decline_restores_taps_and_requirements_cannot_force_payment() {
    let mut g=game(2); g.put(1,0,Zone::Battlefield); let a=g.put(0,5,Zone::Battlefield);
    ready(&mut g);
    let Some((_,Asked::Attackers(able)))=g.asked() else {panic!()}; cite!(["508.1d"]; assert!(!able[0].must));
    g.act(0,Move::Attack(vec![(a,Target::Player(1))])).unwrap();
    cite!(["508.1h"]; assert_eq!(generic(&g),2));
    crate::support::refused(g.act(0,Move::Choose(Answer::Yes)), "submit explicit payment instructions or decline (118.12)");
    g.act(0,Move::Choose(Answer::No)).unwrap();
    cite!(["508.1", "733.1"]; assert!(!g.obj(a).tapped)); assert!(g.combat.attackers.is_empty());
    cite!(["508.1d"]; ok: g.act(0,Move::Attack(vec![]))); assert!(g.decision.is_none());
}
#[test]
fn declaration_taps_prevent_using_attacker_mana_but_vigilance_allows_it() {
    let mut g=game(2); g.put(1,0,Zone::Battlefield); let elf=g.put(0,6,Zone::Battlefield);
    let land=g.put(0,3,Zone::Battlefield); ready(&mut g);
    g.act(0,Move::Attack(vec![(elf,Target::Player(1))])).unwrap();
    cite!(["702.20b"]; assert!(!g.obj(elf).tapped));
    let step = |source, ability| game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source, ability, paying: vec![], adding: vec![], chose: vec![], cost_branches: None };
    g.act(0,Move::Choose(Answer::Pay { plan: vec![step(elf, 1), step(land, 0)], with: Some(mana_pool("WR")), tapped: vec![] })).unwrap();
    cite!(["508.1i"]; assert!(g.obj(elf).tapped&&g.obj(land).tapped)); cite!(["508.1k"]; assert!(g.combat.attacking(elf)));
}

#[test]
fn ambiguous_payers_multipliers_and_nonmana_costs_are_refused() {
    for (text, reason) in [
        (TAX.replace("their controller pays", "you pay"), "attack payment: payer is not the attackers' controller"),
        (TAX.replace("{2}", "{X}"), "attack payment: variable, snow or life alternative cost"),
        (TAX.replace("that's attacking you", "that's attacking a player"), "attack payment: different multiplier recipient"),
        (TAX.replace("for each creature they control that's attacking you", "for each land they control"), "attack payment: different multiplier recipient"),
        ("Creatures can't attack you unless their controller pays 2 life for each creature they control that's attacking you.".into(), "attack payment: nonmana or multiple costs"),
    ] {
        let def = CardDef::new("Refused", "{2}{W}", "Enchantment", &text, None).unwrap_or_else(|why| panic!("{text}: {why}"));
        // Uncited (admission): each legal text names its unsupported attack-payment shape.
        assert_eq!(game::play::check(&def).expect_err("unsupported attack-payment shape").0, reason, "{text}");
    }
}

fn suspended(n: usize) -> (Game,Vec<game::ObjId>,game::ObjId,game::ObjId) {
    let mut g=game(2); g.put(1,0,Zone::Battlefield);
    let attackers=(0..n).map(|_|g.put(0,10,Zone::Battlefield)).collect::<Vec<_>>();
    let elf=g.put(0,7,Zone::Battlefield); let scales=g.put(0,8,Zone::Battlefield); g.put(0,9,Zone::Battlefield);
    ready(&mut g);
    g.act(0,Move::Attack(attackers.iter().map(|c|(*c,Target::Player(1))).collect())).unwrap();
    let step=game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![], source:elf,ability:0,paying:vec![],adding:vec![vec![ir::mana::ManaSymbol::Color(ir::Color::Black);3]] };
    g.act(0,Move::Choose(Answer::Pay { plan:vec![step],with:Some(mana_pool(&"B".repeat(n*2))),tapped:vec![]})).unwrap();
    assert_eq!(g.players[0].life,22); assert!(g.combat.attackers.is_empty());
    (g,attackers,elf,scales)
}
fn finish_mana(g: &mut Game, scales:game::ObjId) {
    let Some(Decision::Choose{choice:Choice::Replacement{among,..},..})=&g.decision else {panic!("{:?}",g.decision)};
    let replacement=among.iter().find(|e|e.source==scales).unwrap().clone();
    g.act(0,Move::Choose(Answer::Replacement(replacement))).unwrap();
    g.act(0,Move::Choose(Answer::Mana(vec![ir::mana::ManaSymbol::Color(ir::Color::Black);4]))).unwrap();
}
#[test]
fn suspended_payment_clones_resume_once_and_cost_stays_locked() {
    let (mut g,attackers,elf,scales)=suspended(1);
    // A changed effective tax after locking cannot recalculate this declaration's cost.
    g.put(1,1,Zone::Battlefield);
    let mut copy=g.clone();
    for game in [&mut g,&mut copy] {
        assert_eq!(game.collect_departed(),0);
        game.retire_events(game.log.end());
        finish_mana(game,scales);
        assert_eq!(game.players[0].life,22);
        cite!(["508.1h"]; assert_eq!(game.players[0].pool.colors[2],2,"locked two, not newly effective four"));
        cite!(["508.1k"]; assert!(game.combat.attacking(attackers[0]))); assert!(game.obj(elf).tapped);
        assert_eq!(game.log.iter().filter(|e|matches!(e,Event::TappedForMana{..})).count(),1);
        cite!(["603.3"]; assert_eq!(game.stack.len(),1,"declaration tap trigger waits until payment finishes"));
    }
}
#[test]
fn reversing_failed_payment_restores_declaration_taps_but_keeps_mana_and_its_effects() {
    let (mut g,attackers,elf,scales)=suspended(3);
    finish_mana(&mut g,scales);
    assert!(matches!(g.decision,Some(Decision::Payment{..})),"{:?}",g.decision);
    g.act(0,Move::ReversePayment).unwrap();
    // Uncited (implementation): 733.1 lets the player reverse the mana abilities ("may"); the engine never does, so the mana, its life and the Elf's tap stay
    assert_eq!(g.players[0].life,22); assert_eq!(g.players[0].pool.colors[2],4);
    assert!(g.obj(elf).tapped); cite!(["733.1"]; assert!(attackers.iter().all(|a|!g.obj(*a).tapped)));
    cite!(["733.1"]; assert!(g.combat.attackers.is_empty()&&g.stack.is_empty()));
    cite!(["733.2"]; assert!(matches!(g.decision,Some(Decision::Attackers{..}))));
    g.act(0,Move::Attack(vec![])).unwrap();
    cite!(["733.1"]; assert!(g.stack.is_empty(),"reversal's restoration is not an untap event or tap trigger"));
}
#[test]
fn an_attacker_sacrificed_for_payment_does_not_become_attacking() {
    let mut g=game(2);g.put(1,0,Zone::Battlefield);let a=g.put(0,2,Zone::Battlefield);let altar=g.put(0,11,Zone::Battlefield);
    ready(&mut g);g.act(0,Move::Attack(vec![(a,Target::Player(1))])).unwrap();
    let step=game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![], source:altar,ability:0,paying:vec![vec![a]],adding:vec![]};
    g.act(0,Move::Choose(Answer::Pay{plan:vec![step],with:Some(mana_pool("CC")),tapped:vec![]})).unwrap();
    cite!(["508.1k"]; assert!(!g.combat.attacking(a)));assert!(!g.log.iter().any(|e|matches!(e,Event::Attacked{..})));
}
#[test]
fn a_required_creature_with_a_free_walker_recipient_still_has_to_attack() {
    let mut g=game(2);g.put(1,0,Zone::Battlefield);let walker=g.put(1,4,Zone::Battlefield);g.put_counters(walker,"loyalty",3);g.put(0,5,Zone::Battlefield);
    ready(&mut g);cite!(["508.1d"]; refused("Required attacks if able (508.1d)"): g.act(0,Move::Attack(vec![])));
}

#[test]
fn declaration_tapped_mana_creature_cannot_pay_its_own_attack_cost() {
    let mut g=game(2);g.put(1,0,Zone::Battlefield);let elf=g.put(0,7,Zone::Battlefield);
    ready(&mut g);g.act(0,Move::Attack(vec![(elf,Target::Player(1))])).unwrap();
    let before=format!("{g:?}");
    let step=game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![], source:elf,ability:0,paying:vec![],adding:vec![vec![ir::mana::ManaSymbol::Color(ir::Color::Black)]]};
    cite!(["508.1f"]; refused("Mana creature can't pay {T} as it is (107.5, 118.3)"): g.act(0,Move::Choose(Answer::Pay{plan:vec![step],with:None,tapped:vec![]})));
    assert_eq!(format!("{g:?}"),before);
}
#[test]
fn battles_remain_absent_from_offers_and_refused_by_declaration() {
    let mut g=Game::new(vec![
        CardDef::new("Bear","{G}","Creature — Bear","",Some((2,2))).unwrap(),
        CardDef::new("Battle fixture","{1}","Battle","",None).unwrap(),
    ],2,20);
    let a=g.put(0,0,Zone::Battlefield);let battle=g.put(1,1,Zone::Battlefield);ready(&mut g);
    let Some((_,Asked::Attackers(able)))=g.asked() else {panic!()};
    assert!(!able[0].attacks.contains(&Target::Object(battle)));
    let before=format!("{g:?}");
    crate::support::refused(g.act(0,Move::Attack(vec![(a,Target::Object(battle))])), "attacking a battle is not in the engine yet");
    assert_eq!(format!("{g:?}"),before);
}

#[test]
fn declaration_read_scope_ends_before_taps_and_payment_reversal() {
    let mut g = game(3);
    let guard = CardDef::new("Alert", "{0}", "Creature", "As long as this creature is untapped, it gets +2/+0.", Some((2,2))).unwrap();
    game::play::check(&guard).unwrap();
    let index = g.defs.len();
    std::sync::Arc::make_mut(&mut g.defs).push(guard);
    let attacker = g.put(0,index,Zone::Battlefield);
    g.put(1,0,Zone::Battlefield);
    ready(&mut g);
    assert!(matches!(g.asked(),Some((0,Asked::Attackers(_)))));
    assert_eq!(g.power(attacker),Some(4));
    let duplicate = vec![(attacker,Target::Player(1));2];
    // Uncited (protocol): the declaration lists each chosen creature once with what it attacks (508.1a-b); a repeated entry is a malformed answer, not a rules choice
    crate::support::refused(g.act(0,Move::Attack(duplicate)), "a creature is declared as an attacker once");
    assert_eq!(g.power(attacker),Some(4));
    g.players[0].pool.colorless=2;
    g.act(0,Move::Attack(vec![(attacker,Target::Player(1))])).unwrap();
    cite!(["508.1f"]; assert_eq!(g.power(attacker),Some(2)));
    let mut paid=g.clone();
    g.act(0,Move::Choose(Answer::No)).unwrap();
    assert!(matches!(g.asked(),Some((0,Asked::Attackers(_)))));
    cite!(["733.1"]; assert_eq!(g.power(attacker),Some(4)));
    paid.act(0,Move::Choose(Answer::Pay { plan: vec![], with: Some(mana_pool("CC")), tapped: vec![] })).unwrap();
    assert_eq!(paid.power(attacker),Some(2));
    cite!(["508.1k"]; assert_eq!(paid.combat.attackers,vec![(attacker,Target::Player(1))]));
}
