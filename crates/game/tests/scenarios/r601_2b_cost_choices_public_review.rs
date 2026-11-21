//! Cost choices over the wire, derived from the rules: the player chooses
//! among a cost's alternatives as the spell or ability is proposed (601.2b, 602.2b), mana
//! abilities activated during a payment are activated by the player like any other
//! (601.2g, 605.3a), a copy copies the choices made for the spell (707.10), and beholding a
//! permanent is public (701.4a). Each alternative said as the card's text says it.
use crate::support::{cite,paid};
use game::legal::Step;
use game::snapshot::Snapshot;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;
use game::{Branch,BranchSelection,ForagePayment};
use serde_json::json;

fn semantic(g:&Game, source:ObjId, paragraph:usize, branch:Branch) -> BranchSelection {
    let audit=g.audit_game_objects();
    let sites=audit["objects"][source.0.to_string()]["cost_branch_choices"].as_array().unwrap();
    let rows:Vec<BranchSelection>=sites.iter().flat_map(|site|site.as_array().unwrap()).cloned().map(|row|serde_json::from_value(row).unwrap()).collect();
    let row=rows.into_iter().find(|row|row.branch==branch && row.ability["paragraph"]==paragraph).expect("the compiled semantic choice exists");
    assert_eq!(row.ability["source"],json!(source.0));
    assert_eq!(row.ability["paragraph"],json!(paragraph));
    assert_eq!(row.ability["origin"],json!({"kind":"own"}));
    row
}


const POOL: [&str; 10] = ["Thornvault Forager", "Bitter Triumph", "Pumpkin Bombardment", "Louisoix's Sacrifice", "Silence the Echo",
    "Soaring Stoneglider", "Bogslither's Embrace", "Bullseye, Death Dealer", "Mudbutton Cursetosser", "Kinsbaile Aspirant"];
const FORAGER: usize = 0;
const TRIUMPH: usize = 1;
const WASTES: usize = 10;
const BEAR: usize = 11;
const FOOD: usize = 12;
const FORK: usize = 13;
const LEGEND: usize = 14;
const GOBLIN: usize = 15;
const RELIC: usize = 16;
const GROWER: usize = 17;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let mut defs: Vec<CardDef> = POOL.iter().map(|n| CardDef::from_faces(&catalog[*n]).unwrap()).collect();
    let c = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    defs.extend([
        c("Wastes", "", "Basic Land", "{T}: Add {C}.", None),
        c("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        c("Food", "", "Artifact — Food", "{2}, {T}, Sacrifice this artifact: You gain 3 life.", None),
        c("Fork", "{0}", "Instant", "Copy target instant or sorcery spell. You may choose new targets for the copy.", None),
        c("Legend", "{1}", "Legendary Creature — Human", "", Some((1, 1))),
        c("Goblin", "{R}", "Creature — Goblin", "", Some((1, 1))),
        c("Relic", "{1}", "Artifact", "", None),
        c("Grower", "{0}", "Artifact", "Sacrifice an artifact or pay 1 life: Add {G} for each creature you control.", None),
    ]);
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..8 { g.put(p, WASTES, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn on_stack_announced(g: &Game, object: ObjId, viewer: usize) -> Option<Vec<String>> {
    Snapshot::of(g, Some(viewer)).objects.into_iter().find(|o| o.id == object).and_then(|o| o.on_stack).map(|s| s.announced)
}
fn forage_step(g:&Game, forager: ObjId, chose: usize, paying: Vec<ObjId>) -> Step {
    let green = vec![ir::mana::ManaSymbol::Color(ir::Color::Green); 2];
    Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: Some(vec![semantic(g,forager,2,Branch::Forage { payment:if chose==0 {ForagePayment::ExileThree} else {ForagePayment::SacrificeFood} })]), source: forager, ability: 1, paying: vec![paying], adding: vec![green], chose: vec![] }
}

// Rules: 601.2b, 601.2g, 605.3a, 701.61a
// Given: Thornvault Forager, a Food and three cards in the graveyard; a Bear in hand and no
// other mana.
// Expect: a payment of either
// alternative supplied by the player is followed as said: the Food sacrificed and the
// graveyard kept, or the graveyard exiled and the Food kept.
// Why: which alternative is paid is the player's choice as the mana ability is activated
// (602.2b by way of 605.3a).
#[test]
fn r601_2b_both_forage_alternatives_payable_the_player_chooses_which() {
    let fresh = || {
        let mut g = game();
        let forager = g.put(0, FORAGER, Zone::Battlefield);
        let food = g.put(0, FOOD, Zone::Battlefield);
        let cards: Vec<ObjId> = (0..3).map(|_| g.put(0, WASTES, Zone::Graveyard)).collect();
        let bear = g.put(0, BEAR, Zone::Hand);
        (g, forager, food, cards, bear)
    };
    // the Food sacrificed, the graveyard kept
    let (mut g, forager, food, cards, bear) = fresh();
    g.act(0, Move::WithMana(vec![forage_step(&g,forager, 1, vec![food])], Box::new(paid("GG",Move::Cast { card: bear, targets: vec![] })))).unwrap();
    cite!(["601.2b", "701.61a"]; assert!(!g.battlefield.contains(&food) && cards.iter().all(|c| g.obj(*c).zone == Zone::Graveyard)));
    // the graveyard exiled, the Food kept
    let (mut g, forager, food, cards, bear) = fresh();
    g.act(0, Move::WithMana(vec![forage_step(&g,forager, 0, cards.clone())], Box::new(paid("GG",Move::Cast { card: bear, targets: vec![] })))).unwrap();
    cite!(["601.2b", "701.61a"]; assert!(g.battlefield.contains(&food) && cards.iter().all(|c| g.obj(*c).zone != Zone::Graveyard) && g.players[0].graveyard.is_empty() && g.exile.len() == 3));
    cite!(["601.2h"]; assert!(g.stack.iter().any(|o| g.obj(*o).card == Some(BEAR))));
}

// Rules: 602.2b, 605.3a, 701.61a
// Given: the Forager with a Food and three graveyard cards, at priority, nothing to pay for.
// Expect: activated outside a payment its forage is chosen either way; asked without a
// choice, it is refused.
#[test]
fn r602_2b_a_forage_mana_ability_activated_at_priority() {
    for chose in [0, 1] {
        let mut g = game();
        let forager = g.put(0, FORAGER, Zone::Battlefield);
        let food = g.put(0, FOOD, Zone::Battlefield);
        let cards: Vec<ObjId> = (0..3).map(|_| g.put(0, WASTES, Zone::Graveyard)).collect();
        let paid = if chose == 0 { cards.clone() } else { vec![food] };
        let activate = Move::Activate { source: forager, ability: 1, targets: vec![], paying: vec![paid] };
        cite!(["602.2b"]; case: chose; refused("that cost has 1 choice(s) among alternatives"): act_unchanged(&mut g, activate.clone()));
        // the mana it adds: two in any combination of colors
        let green = vec![ir::mana::ManaSymbol::Color(ir::Color::Green); 2];
        g.act(0, Move::Adding(vec![green], Box::new(Move::WithCostChoice(vec![chose], Box::new(activate))))).unwrap();
        cite!(["605.3a", "701.61a"]; case: chose; assert_eq!(g.players[0].pool.total(), 2));
        cite!(["701.61a"]; case: chose; assert_eq!((g.battlefield.contains(&food), cards.iter().all(|c| g.obj(*c).zone == Zone::Graveyard)), (chose == 0, chose == 1)));
    }
}

// Rules: 707.10, 601.2b
// Given: Bitter Triumph cast paying 3 life, then copied by Fork.
// Expect: both seats see "pay 3 life" announced on the original and on the copy; a Bear cast
// with no choice in its cost announces nothing.
#[test]
fn r707_10_a_copy_shows_the_alternative_announced_for_the_spell_it_copies() {
    let mut g = game();
    let triumph = g.put(0, TRIUMPH, Zone::Hand);
    let fork = g.put(0, FORK, Zone::Hand);
    let victim = g.put(1, BEAR, Zone::Battlefield);
    g.players[0].pool.colors[2] = 1;
    g.players[0].pool.colorless = 1;
    g.act(0, Move::WithCostBranches(vec![semantic(&g,triumph,1,Branch::PrintedOr { arm:2,forage:None })], Box::new(paid("CB",Move::Cast { card: triumph, targets: vec![vec![Target::Object(victim)]] })))).unwrap();
    assert_eq!(g.players[0].life, 17);
    let spell = *g.stack.last().unwrap();
    g.act(0, Move::Cast { card: fork, targets: vec![vec![Target::Object(spell)]] }).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    while g.decision.is_some() { let (p, _) = g.asked().unwrap(); g.act(p, Move::Choose(game::state::Answer::No)).unwrap(); }
    let copy = *g.stack.last().unwrap();
    assert_ne!(copy, spell, "the copy is on the stack");
    let audit=g.audit_game_objects();
    for object in [spell,copy] {
        let rows=&audit["objects"][object.0.to_string()]["announced"]["cost_branches"];
        cite!(["707.10", "601.2b"]; assert_eq!(rows[0]["branch"],json!({"kind":"printed_or","arm":2})));
        assert_eq!(rows[0]["ability"]["definition"],"Bitter Triumph");
        assert_eq!(rows[0]["ability"]["paragraph"],1);
        assert_eq!(rows[0]["ability"]["source"],object.0);
    }

    for viewer in [0, 1] {
        cite!(["601.2b"]; case: viewer; assert_eq!(on_stack_announced(&g, spell, viewer), Some(vec!["pay 3 life".to_string()])));
        cite!(["707.10"]; case: viewer; assert_eq!(on_stack_announced(&g, copy, viewer), Some(vec!["pay 3 life".to_string()])));
    }
    // A copied spell's cost declaration survives the original spell's departure;
    // its immutable definition is not recovered through the old source incarnation.
    g.move_object(spell,Zone::Graveyard);
    let departed=g.audit_game_objects();
    let retained=&departed["objects"][copy.0.to_string()]["announced"]["cost_branches"];
    cite!(["707.10","400.7"]; assert_eq!(retained[0]["branch"],json!({"kind":"printed_or","arm":2})));
    assert_eq!(retained[0]["ability"]["source"],copy.0);
    assert_eq!(retained[0]["ability"]["definition"],"Bitter Triumph");
    // no choice: nothing announced
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Hand);
    g.players[0].pool.colors[4] = 2;
    g.act(0, paid("GG",Move::Cast { card: bear, targets: vec![] })).unwrap();
    cite!(["601.2b"]; assert_eq!(on_stack_announced(&g, bear_on_stack(&g), 1), Some(vec![])));
}
fn bear_on_stack(g: &Game) -> ObjId { *g.stack.last().unwrap() }

// Rules: 601.2b, 602.2b
// Given: eight admitted Standard cards (Louisoix's Sacrifice with a spell to counter) whose costs are a choice, with every alternative payable.
// Expect: each alternative said as the Oracle text says it.
#[test]
fn r601_2b_the_alternatives_of_eight_pool_cards_read_as_printed() {
    let expected: [(&str,&[&str]);8]=[
        ("Bitter Triumph",&["discard a card","pay 3 life"]),
        ("Pumpkin Bombardment",&["discard a card","pay {2}"]),
        ("Silence the Echo",&["sacrifice a creature or planeswalker","pay {3}"]),
        ("Soaring Stoneglider",&["exile two cards from your graveyard","pay {1}{W}"]),
        ("Bogslither's Embrace",&["blight 1","pay {3}"]),
        ("Bullseye, Death Dealer",&["sacrifice an artifact","discard a nonland card"]),
        ("Mudbutton Cursetosser",&["behold a Goblin","pay {2}"]),
        ("Louisoix's Sacrifice",&["sacrifice a legendary creature","pay {2}"]),
    ];
    for (name,printed) in expected { for arm in [0,1] {
        let mut g=game(); g.players[0].pool.colorless=20; g.players[0].pool.colors=[4;5];
        let legend=g.put(0,LEGEND,Zone::Battlefield); let bear=g.put(0,BEAR,Zone::Battlefield);
        let goblin=g.put(0,GOBLIN,Zone::Battlefield); let relic=g.put(0,RELIC,Zone::Battlefield);
        let grave=vec![g.put(0,WASTES,Zone::Graveyard),g.put(0,WASTES,Zone::Graveyard)];
        let discard=g.put(0,BEAR,Zone::Hand); let victim=g.put(1,BEAR,Zone::Battlefield);
        let opposing=(name=="Louisoix's Sacrifice").then(||g.put(1,RELIC,Zone::Stack));
        let index=POOL.iter().position(|n|*n==name).unwrap();
        let is_ability=name=="Bullseye, Death Dealer";
        let card=g.put(0,index,if is_ability {Zone::Battlefield} else {Zone::Hand});
        let chosen=if arm==0 {match name {
            "Bitter Triumph"|"Pumpkin Bombardment"=>vec![discard],
            "Silence the Echo"|"Bogslither's Embrace"=>vec![bear],
            "Soaring Stoneglider"=>grave,
            "Bullseye, Death Dealer"=>vec![relic],
            "Mudbutton Cursetosser"=>vec![goblin],
            "Louisoix's Sacrifice"=>vec![legend], _=>unreachable!(),
        }} else if is_ability {vec![discard]} else {vec![]};
        let paying=if chosen.is_empty() {vec![]} else {vec![chosen]};
        let targets=match name {
            "Soaring Stoneglider"|"Mudbutton Cursetosser"=>vec![],
            "Louisoix's Sacrifice"=>vec![vec![Target::Object(opposing.unwrap())]],
            _=>vec![vec![Target::Object(victim)]],
        };
        let units=match name {
            "Bitter Triumph"=>"CB",
            "Pumpkin Bombardment"=>if arm==0{"R"}else{"CCR"},
            "Silence the Echo"|"Bogslither's Embrace"=>if arm==0{"CB"}else{"CCCCB"},
            "Soaring Stoneglider"=>if arm==0{"CCW"}else{"CCCWW"},
            "Bullseye, Death Dealer"=>"CCC",
            "Mudbutton Cursetosser"=>if arm==0{"B"}else{"CCB"},
            "Louisoix's Sacrifice"=>if arm==0{"U"}else{"CCU"}, _=>unreachable!(),
        };
        let action=if is_ability {Move::Activate{source:card,ability:1,targets,paying}} else {Move::WithCosts(paying,Box::new(Move::Cast{card,targets}))};
        cite!(["601.2b","602.2b"]; case: format!("{name} arm {arm}"); ok: g.act(0,Move::WithCostChoice(vec![arm],Box::new(paid(units,action)))));
        let stack=*g.stack.last().unwrap();
        for viewer in [0,1] {
            cite!(["601.2b","602.2b"]; assert_eq!(on_stack_announced(&g,stack,viewer),Some(vec![printed[arm].to_string()])));
        }
    }}
}

// Rules: 601.2b, 605.3a, 605.3b, 608.2h
// Given: a test artifact, "Sacrifice an artifact or pay 1 life: Add {G} for each creature you
// control." (a mana ability counting its mana as it resolves), two creatures, a Relic, and a
// Bear in hand.
// Expect: either supplied cost alternative pays for the Bear in one move.
// Why: a choice cost on such an ability is otherwise untested; no
// admitted card has one.
#[test]
fn r605_3b_a_mana_ability_counting_its_mana_with_a_choice_cost_pays_either_way() {
    for chose in [0usize, 1] {
        let mut g = game();
        let source = g.put(0, GROWER, Zone::Battlefield);
        let relic = g.put(0, RELIC, Zone::Battlefield);
        for _ in 0..2 { g.put(0, LEGEND, Zone::Battlefield); }
        let bear = g.put(0, BEAR, Zone::Hand);
        let step=Step { plan: vec![], x: None, life: vec![], with: None,source,ability:0,cost_branches:None,paying:if chose==0 {vec![vec![relic]]} else {vec![]},chose:vec![chose],adding:vec![]};
        g.act(0, Move::WithMana(vec![step], Box::new(paid("GG",Move::Cast { card: bear, targets: vec![] })))).unwrap();
        cite!(["605.3b", "608.2h"]; case: chose; assert!(g.stack.iter().any(|o| g.obj(*o).card == Some(BEAR))));
        cite!(["601.2b"]; case: chose; assert_eq!((g.battlefield.contains(&relic), g.players[0].life), if chose == 0 { (false, 20) } else { (true, 19) }));
    }
}

// CR601.2b/f/h and733.1: the announced branch must belong to the assembled cost;
// a refused announcement pays no mana/life and moves no object.
#[test]
fn r601_2b_semantic_cost_branches_refuse_missing_duplicate_foreign_and_wrong_arm_atomically() {
    let mut g=game();
    let card=g.put(0,TRIUMPH,Zone::Hand);
    let foreign=g.put(0,TRIUMPH,Zone::Hand);
    let victim=g.put(1,BEAR,Zone::Battlefield);
    g.players[0].pool.colorless=1;g.players[0].pool.colors[2]=1;
    let selected=semantic(&g,card,1,Branch::PrintedOr {arm:2,forage:None});
    let foreign=semantic(&g,foreign,1,Branch::PrintedOr {arm:2,forage:None});
    let mut wrong=selected.clone();wrong.branch=Branch::PrintedOr {arm:3,forage:None};
    let mut stale=selected.clone();stale.ability["definition"]=json!("Stale definition");
    let mut owner=selected.clone();owner.ability["paragraph"]=json!(2);
    let mut nested=selected.clone();nested.branch=Branch::PrintedOr {arm:2,forage:Some(ForagePayment::SacrificeFood)};
    let before=g.audit_game_objects();
    for (rows,reason) in [
        (vec![],"missing or foreign cost branch selection"),
        (vec![selected.clone(),selected.clone()],"missing or foreign cost branch selection"),
        (vec![foreign],"cost branch must match exactly one actual choice site"),
        (vec![wrong],"cost branch must match exactly one actual choice site"),
        (vec![stale],"cost branch must match exactly one actual choice site"),
        (vec![owner],"cost branch must match exactly one actual choice site"),
        (vec![nested],"cost branch must match exactly one actual choice site"),
    ] {
        let action=Move::WithCostBranches(rows,Box::new(paid("CB",Move::Cast {card,targets:vec![vec![Target::Object(victim)]]})));
        cite!(["601.2b","733.1"]; assert_eq!(g.act(0,action).unwrap_err().to_string(),reason));
        assert_eq!(g.audit_game_objects(),before);
        assert_eq!((g.players[0].life,g.players[0].pool.total()),(20,2));
    }
    g.act(0,Move::WithCostBranches(vec![selected],Box::new(paid("CB",Move::Cast {card,targets:vec![vec![Target::Object(victim)]]})))).unwrap();
    cite!(["601.2h"]; assert_eq!(g.players[0].life,17));
    assert_eq!(g.players[0].pool.total(),0);
}

/// A rejected proposal preserves the whole game; the caller asserts its specific reason.
#[track_caller]
fn act_unchanged(g: &mut Game, action: Move) -> Result<(), String> {
    let before = format!("{g:?}");
    let result = g.act(0, action);
    assert_eq!(format!("{g:?}"), before);
    result
}
