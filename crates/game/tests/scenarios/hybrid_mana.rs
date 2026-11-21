//! Hybrid symbols have two alternative nonhybrid costs (107.4e, 601.2b).
use crate::support::{cite,paid};
use game::state::Pool;
use game::{CardDef, Game, Move, Zone};

fn pool(w: u32, u: u32, b: u32, c: u32) -> Pool {
    Pool { colors: [w, u, b, 0, 0], colorless: c, restricted: vec![] }
}
fn game(cost: &str) -> Game {
    let defs = vec![
        CardDef::new("Prism", cost, "Instant", "You gain 1 life.", None).unwrap(),
        CardDef::new("Plains", "", "Basic Land — Plains", "", None).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    Game::new(defs, 2, 20)
}
fn cast(g: &mut Game, with: Option<Pool>) -> Result<(), String> {
    let card = g.put(0, 0, Zone::Hand);
    let mv = Move::Cast { card, targets: vec![] };
    g.act(0, with.map_or(mv.clone(), |p| Move::Paying(p, Box::new(mv))))
}

#[test]
fn r107_4e_a_hybrid_symbol_accepts_either_half_but_not_an_unrelated_type() {
    for (cost, pools) in [
        ("{W/U}", vec![pool(1, 0, 0, 0), pool(0, 1, 0, 0)]),
        ("{C/W}", vec![pool(0, 0, 0, 1), pool(1, 0, 0, 0)]),
        ("{2/W}", vec![pool(1, 0, 0, 0), pool(0, 1, 1, 0), pool(0, 0, 0, 2)]),
    ] {
        for mana in pools {
            let mut g = game(cost);
            g.players[0].pool = mana.clone();
            cite!(["107.4e"]; case: cost; ok: cast(&mut g, Some(mana)));
            assert_eq!(g.players[0].pool.total(), 0);
        }
        let mut g = game(cost);
        g.players[0].pool = pool(0, 0, 1, 0);
        let card = g.put(0, 0, Zone::Hand);
        let before = format!("{g:?}");
        cite!(["107.4e"]; case: cost; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0, Move::Paying(pool(0,0,1,0),Box::new(Move::Cast { card, targets: vec![] }))));
        assert_eq!(format!("{g:?}"), before);
    }
}

#[test]
fn r601_2h_hybrid_choices_must_leave_mana_for_the_other_symbols() {
    for cost in ["{W/U}{W}", "{U}{W/U}", "{W/U}{W/U}"] {
        let mut g = game(cost);
        g.players[0].pool = pool(1, 1, 0, 0);
        cite!(["107.4e", "601.2h"]; case: cost; ok: cast(&mut g, Some(pool(1,1,0,0))));
        assert_eq!(g.players[0].pool.total(), 0);
    }
}

#[test]
fn r601_2b_explicit_payment_can_choose_the_generic_half_even_when_the_colored_half_is_available() {
    for with in [pool(1, 0, 0, 0), pool(0, 0, 0, 2), pool(1, 0, 0, 1)] {
        let mut g = game("{2/W}");
        g.players[0].pool = pool(1, 0, 0, 2);
        let spent = with.total();
        cast(&mut g, Some(with)).unwrap();
        cite!(["601.2b"]; assert_eq!(g.players[0].pool.total(), 3 - spent));
    }
    let mut g = game("{2/W}");
    g.players[0].pool = pool(1, 0, 0, 2);
    cite!(["601.2h"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): cast(&mut g, Some(pool(1, 0, 0, 2))));
    assert_eq!(g.players[0].pool, pool(1, 0, 0, 2));
}

#[test]
fn r107_4e_exact_hybrid_spending_matches_its_nonhybrid_alternatives() {
    for (case, cost) in ["{W/U}{U/B}", "{W/U}{W/U}", "{2/W}{U/B}{C}", "{C/W}{U/B}"].iter().enumerate() {
        let template = game(cost);
        for w in 0..3 {
            for u in 0..3 {
                for b in 0..3 {
                    for c in 0..3 {
                        let expected = match case {
                            0 => c == 0 && w+u+b == 2 && (w >= 1 && u >= 1 || w >= 1 && b >= 1 || u >= 2 || u >= 1 && b >= 1),
                            1 => w + u == 2 && b == 0 && c == 0,
                            2 => c >= 1 && u+b >= 1 && (w+u+b+c == 4 || w >= 1 && w+u+b+c == 3),
                            _ => w+u+b+c == 2 && (c >= 1 || w >= 1) && u+b >= 1,
                        };
                        let mut g = template.clone();
                        g.players[0].pool = pool(3,3,3,3);
                        cite!(["107.4e"]; case: format!("{cost} with W={w},U={u},B={b},C={c}"); assert_eq!(cast(&mut g, Some(pool(w,u,b,c))).is_ok(), expected, "{cost} with W={w},U={u},B={b},C={c}"));
                    }
                }
            }
        }
    }
}

#[test]
fn r601_2g_hybrid_cast_offers_use_an_executable_mana_plan() {
    let mut g = game("{W/U}{U}");
    let sources = [g.put(0, 1, Zone::Battlefield), g.put(0, 2, Zone::Battlefield)];
    let card = g.put(0, 0, Zone::Hand);
    for source in sources {
        g.act(0, Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
    cite!(["601.2g"]; ok: g.act(0, paid("WU",Move::Cast { card, targets: vec![] })));
    assert_eq!(g.players[0].pool.total(), 0);
}

#[test]
fn r602_2b_activated_hybrid_costs_use_the_same_payment_rules() {
    let d = CardDef::new("Relic", "{0}", "Artifact", "{W/U}: You gain 1 life.", None).unwrap();
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    g.players[0].pool = pool(0, 1, 0, 0);
    cite!(["602.2b", "107.4e"]; ok: g.act(0, Move::Paying(pool(0,1,0,0),Box::new(Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }))));
    assert_eq!(g.players[0].pool.total(), 0);
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    assert_eq!(g.players[0].life, 21);
}

#[test]
fn r106_6_hybrid_payment_preserves_restricted_mana_rules_and_spending_effects() {
    let defs = vec![
        CardDef::new(
            "Spring",
            "{0}",
            "Artifact",
            "{T}: Add {U}. Spend this mana only to cast a creature spell, and that spell can't be countered.",
            None,
        )
        .unwrap(),
        CardDef::new("Prism", "{W/U}", "Creature — Spirit", "", Some((2, 2))).unwrap(),
        CardDef::new("Trial", "{W/U}", "Instant", "You gain 1 life.", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap();
    }
    let mut g = Game::new(defs, 2, 20);
    let spring = g.put(0, 0, Zone::Battlefield);
    g.act(0, Move::Activate { source: spring, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    let spell = g.put(0, 2, Zone::Hand);
    let before = format!("{g:?}");
    let with=Pool { restricted:vec![g.players[0].pool.restricted[0].clone()], ..Pool::default() };
    cite!(["106.6"]; refused("that mana can't be spent on that: its text restricts how it can be spent (106.6)"): g.act(0, Move::Paying(with.clone(),Box::new(Move::Cast { card: spell, targets: vec![] }))));
    assert_eq!(format!("{g:?}"), before);
    let creature = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Paying(with, Box::new(Move::Cast { card: creature, targets: vec![] }))).unwrap();
    assert_eq!(g.players[0].pool.total(), 0);
    let stack = g.stack[0];
    cite!(["106.6"]; assert!(!g.counter(stack), "the spent mana's effect survives hybrid payment"));
}

#[test]
fn r107_4e_hybrid_support_does_not_admit_unmodeled_snow_payments() {
    // {X} is announced (r107_3_announced_x.rs); a second variable is not
    for (cost, reason) in [("{S}", "cost: Snow"), ("{X}{Y}", "cost: Var")] {
        let d = CardDef::new("Boundary", cost, "Instant", "You gain 1 life.", None).unwrap();
        assert_eq!(game::play::check(&d).expect_err("unsupported mana symbol").0, reason, "{cost}");
    }
    // life or mana for Phyrexian symbols is said as the spell is cast (r107_4f_phyrexian_mana.rs)
    for cost in ["{W/P}", "{W/U/P}"] {
        let d = CardDef::new("Boundary", cost, "Instant", "You gain 1 life.", None).unwrap();
        game::play::check(&d).unwrap_or_else(|u| panic!("{cost}: {}", u.0));
    }
    // a hybrid Phyrexian symbol is paid with mana of either color, or 2 life (107.4f)
    let d = CardDef::new("Boundary", "{W/U/P}", "Instant", "You gain 1 life.", None).unwrap();
    for (color, life, left) in [(1, false, 20), (0, false, 20), (4, true, 18)] {
        let mut g = Game::new(vec![d.clone()], 2, 20);
        let card = g.put(0, 0, Zone::Hand);
        g.players[0].pool.colors[color] = 1;
        let cast = Move::Cast { card, targets: vec![] };
        if life {
            let before = format!("{g:?}");
            cite!(["107.4f", "601.2h"]; refused("the supplied mana does not pay that cost, all of it and no more"): g.act(0, Move::Paying(Pool {colors:[0,0,0,0,1],..Pool::default()},Box::new(cast.clone()))));
            assert_eq!(format!("{g:?}"), before);
        }
        let mut with=Pool::default();
        if !life {with.colors[color]=1;}
        g.act(0, Move::Paying(with,Box::new(Move::WithLife(vec![life], Box::new(cast))))).unwrap();
        cite!(["107.4f"]; case: format!("color {color}, life {life}"); assert_eq!(g.players[0].life, left));
    }
}

#[test]
fn r107_4e_boros_recruit_keeps_both_colors_regardless_of_the_mana_paid() {
    for color in [ir::Color::Red, ir::Color::White] {
        let d = CardDef::new(
            "Boros Recruit",
            "{R/W}",
            "Creature — Goblin Soldier",
            "({R/W} can be paid with either {R} or {W}.)\nFirst strike",
            Some((1, 1)),
        )
        .unwrap();
        game::play::check(&d).unwrap();
        let mut g = Game::new(vec![d], 2, 20);
        g.players[0].pool.colors[color as usize] = 1;
        let mut with=Pool::default(); with.colors[color as usize]=1;
        cast(&mut g, Some(with)).unwrap();
        let spell = g.stack[0];
        cite!(["107.4e", "202.2"]; case: format!("{color:?}"); assert_eq!(g.seen(spell, None).colors, vec![ir::Color::Red, ir::Color::White]));
        cite!(["202.3f"]; case: format!("{color:?}"); assert_eq!(g.obj(spell).chars.mana_value, 1));
        g.act(0, Move::Pass).unwrap();
        g.act(1, Move::Pass).unwrap();
        assert!(g.has(g.battlefield[0], "First strike"));
    }
}

#[test]
fn r605_1a_deathrite_shamans_targeted_mana_ability_uses_the_stack_and_all_abilities_exile_their_targets() {
    use game::state::{Answer, Choice, Decision};
    use game::Target;
    use ir::mana::ManaSymbol;
    let defs=vec![CardDef::new("Deathrite Shaman","{B/G}","Creature — Elf Shaman",
        "{T}: Exile target land card from a graveyard. Add one mana of any color.\n{B}, {T}: Exile target instant or sorcery card from a graveyard. Each opponent loses 2 life.\n{G}, {T}: Exile target creature card from a graveyard. You gain 2 life.",Some((1,2))).unwrap(),
        CardDef::new("Forest","","Basic Land — Forest","",None).unwrap(),
        CardDef::new("Trial","{0}","Instant","You gain 1 life.",None).unwrap(),
        CardDef::new("Bear","{0}","Creature — Bear","",Some((2,2))).unwrap()];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    for ability in 0..3 {
        for target_leaves in [false, true] {
            let mut g = Game::new(defs.clone(), 3, 20);
            let source = g.put(0, 0, Zone::Battlefield);
            let target = g.put(1, ability + 1, Zone::Graveyard);
            if ability > 0 {
                g.players[0].pool.colors[if ability == 1 { ir::Color::Black } else { ir::Color::Green } as usize] = 1;
            }
            let mut with=Pool::default();
            if ability>0 {with.colors[if ability==1 {ir::Color::Black} else {ir::Color::Green} as usize]=1;}
            g.act(0, Move::Paying(with,Box::new(Move::Activate { source, ability, targets: vec![vec![Target::Object(target)]], paying: vec![] })))
                .unwrap();
            cite!(["605.5a"]; case: format!("ability {ability}, target leaves {target_leaves}"); assert_eq!(g.stack.len(), 1, "even the mana-producing ability has a target"));
            assert_eq!(g.obj(target).zone, Zone::Graveyard);
            assert_eq!(g.players[0].pool.total(), 0);
            if target_leaves {
                g.move_object(target, Zone::Hand);
            }
            for _ in 0..3 {
                g.act(g.priority, Move::Pass).unwrap();
            }
            if ability == 0 && !target_leaves {
                assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::Mana { .. }, .. })));
                g.act(0, Move::Choose(Answer::Mana(vec![ManaSymbol::Color(ir::Color::Green)]))).unwrap();
            }
            cite!(["608.2b"]; case: format!("ability {ability}, target leaves {target_leaves}"); assert_eq!(g.exile.len(), usize::from(!target_leaves)));
            assert_eq!(g.players[0].pool.total(), u32::from(ability == 0 && !target_leaves));
            assert_eq!(
                g.players.iter().map(|p| p.life).collect::<Vec<_>>(),
                if target_leaves {
                    vec![20, 20, 20]
                } else {
                    match ability {
                        1 => vec![20, 18, 18],
                        2 => vec![22, 20, 20],
                        _ => vec![20, 20, 20],
                    }
                }
            );
        }
    }
}
