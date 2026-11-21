//! CR construction and format setup, independent of any external tournament ban list.
use crate::support::cite;
use game::{CardDef, Game, Move};
use game::state::Decision;
use game::simulation::Class;
use game::start::Refused;

fn defs() -> Vec<CardDef> {
    vec![
        CardDef::new("Leader", "{U}", "Legendary Creature — Human", "", Some((2,2))).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Dual", "", "Land — Island Mountain", "", None).unwrap(),
        CardDef::new("Rock", "{0}", "Artifact", "", None).unwrap(),
        CardDef::new("Rock", "{1}", "Artifact", "", None).unwrap(),
        CardDef::new("Red", "{R}", "Creature", "", Some((1,1))).unwrap(),
        CardDef::new("Mana", "{0}", "Artifact", "{T}: Add {G}.", None).unwrap(),
        CardDef::new("Courier", "{0}", "Artifact Creature — Golem", "This creature is all colors.", Some((3,3))).unwrap(),
        CardDef::new("Ordinary", "{U}", "Creature — Human", "", Some((2,2))).unwrap(),
        CardDef::new("Empty", "{0}", "Legendary Creature — Human", "", Some((2,2))).unwrap(),
        CardDef::new("Wastes", "", "Basic Land", "{T}: Add {C}.", None).unwrap(),
        CardDef::new("Wagon", "{2}", "Legendary Artifact — Vehicle", "", Some((3,3))).unwrap(),
    ]
}
fn start(n: usize, first: usize) -> Game {
    Game::start_commander(defs(), &vec![vec![1;99];n], &vec![0;n], first, 17).unwrap()
}
/// Why the setup is refused; a deck the rules do not allow is an illegal one.
fn rejected(deck: Vec<usize>, leader: usize) -> Refused {
    Game::start_commander(defs(), &[deck,vec![1;99]], &[leader,0], 0, 17).map(|_| ()).unwrap_err()
}
fn malformed(why: &str) -> Refused { Refused { class: Class::Malformed, why: why.into() } }
fn illegal(why: &str) -> Refused { Refused { class: Class::Illegal, why: why.into() } }

#[test]
fn r903_5a_and_6_and_7_start_three_and_four_players_with_commanders_outside_shuffled_libraries() {
    for n in [3,4] {
        let g = start(n,n-1);
        assert_eq!(g.command.len(),n);
        assert_eq!(g.decision,Some(Decision::Mulligan{player:n-1}));
        for p in 0..n {
            cite!(["903.7"]; case: format!("{n}p seat {p}"); assert_eq!(g.players[p].life,40));
            cite!(["903.7"]; case: format!("{n}p seat {p}"); assert_eq!(g.players[p].hand.len(),7));
            cite!(["903.6", "903.7"]; assert_eq!(g.players[p].library.len(),92));
            assert_eq!(g.obj(g.command[p]).owner,p);
            cite!(["903.6"]; case: format!("{n}p seat {p}"); assert!(g.is_commander(g.command[p])));
            assert!(g.players[p].hand.iter().chain(&g.players[p].library).all(|c|!g.is_commander(*c)));
        }
        assert_eq!(g.players[0].library,start(n,n-1).players[0].library);
    }
}

#[test]
fn r903_5_construction_checks_count_singletons_identity_and_basic_land_types() {
    cite!(["903.5a"]; assert_eq!(rejected(vec![1;98],0), illegal("player 0: exactly 100 cards including the commander (903.5a)")));
    cite!(["903.5a"]; assert_eq!(rejected(vec![1;100],0), illegal("player 0: exactly 100 cards including the commander (903.5a)")));
    let mut deck=vec![1;99];deck[0]=4;deck[1]=5;
    cite!(["903.5b"]; assert_eq!(rejected(deck,0), illegal("player 0: duplicate Rock (903.5b)"), "names, not definition indices"));
    // implied basic land abilities count separately
    for (card,why) in [(2,"player 0: Forest has a basic land type outside commander's colors (903.5d)"),(3,"player 0: Dual has a basic land type outside commander's colors (903.5d)")] {
        let mut deck=vec![1;99];deck[0]=card;
        cite!(["903.5d"]; case: why; assert_eq!(rejected(deck,0), illegal(why)));
    }
    // mana cost, rules text and color CDA
    for (card,why) in [(6,"player 0: Red outside commander's color identity (903.5c)"),(7,"player 0: Mana outside commander's color identity (903.5c)"),(8,"player 0: Courier outside commander's color identity (903.5c)")] {
        let mut deck=vec![1;99];deck[0]=card;
        cite!(["903.5c"]; case: why; assert_eq!(rejected(deck,0), illegal(why)));
    }
    let mut deck=vec![1;99];deck[0]=0;
    cite!(["903.5b"]; assert_eq!(rejected(deck,0), illegal("player 0: duplicate Leader (903.5b)")));
}

#[test]
fn r903_5_colorless_commanders_allow_wastes_but_not_a_basic_colored_land() {
    cite!(["903.5d"]; assert!(Game::start_commander(defs(),&[vec![11;99],vec![11;99]],&[10,10],0,0).is_ok()));
    cite!(["903.5d"]; assert_eq!(rejected(vec![1;99],10), illegal("player 0: Island has a basic land type outside commander's colors (903.5d)")));
}

#[test]
fn r903_3_unsupported_commander_designations_and_invalid_setup_are_rejected() {
    // "Each deck has a legendary card designated as its commander" (903.3): Ordinary is a
    // nonlegendary creature, so the rules forbid it
    cite!(["903.3"]; refused("a commander is a legendary card (903.3)"): Game::start_commander(defs(), &[vec![1;99],vec![1;99]], &[9,0], 0, 17));
    // a legendary Vehicle card may be a commander ("(b) a Vehicle card", 903.3); the engine
    // does not run one. Wagon is colorless, so its deck is of Wastes, no basic land type (903.5d)
    cite!(["903.3"]; assert_eq!(rejected(vec![11;99],12), Refused { class: Class::Unsupported, why: "commander setup currently supports legendary creature cards (903.3)".into() }));
    // what names no definition or seat describes no game
    assert_eq!(rejected(vec![1;99],500), malformed("unknown commander definition"));
    let mut deck=vec![1;99];deck[0]=500;
    assert_eq!(rejected(deck,0), malformed("unknown Commander deck definition"));
    for (decks, commanders, first) in [(vec![vec![1;99]], vec![0], 0), (vec![vec![1;99],vec![1;99]], vec![0], 0), (vec![vec![1;99],vec![1;99]], vec![0,0], 2)] {
        assert_eq!(Game::start_commander(defs(),&decks,&commanders,first,0).map(|_| ()).unwrap_err(), malformed("one commander per player and a valid starting player"));
    }
}

#[test]
fn r103_5c_multiplayer_free_mulligan_and_starting_players_draw_use_shared_start_procedure() {
    let mut g=start(3,0);
    g.act(0,Move::Mulligan).unwrap();g.act(1,Move::Keep).unwrap();g.act(2,Move::Keep).unwrap();
    cite!(["103.5c"]; assert_eq!(g.decision,Some(Decision::Mulligan{player:0}),"first mulligan has no bottom choice"));
    cite!(["103.5c"]; assert_eq!(g.players[0].hand.len(),7));
    g.act(0,Move::Keep).unwrap();
    while g.part!=ir::Part::PrecombatMain { g.act(g.priority,Move::Pass).unwrap(); }
    cite!(["103.8c"]; assert_eq!(g.players[0].hand.len(),8,"starting player draws in multiplayer"));
    assert_eq!(g.command.len(),3);
}

#[test]
fn r903_3_public_summaries_never_expose_the_commanders_hidden_zone_handle() {
    let mut g=start(3,0);
    let old=g.command[0];g.move_object(old,game::Zone::Library);
    let hidden=*g.players[0].library.last().unwrap();
    let mut sim=game::simulation::Simulation::new(g);
    let shown=sim.snapshot(Some(1)).unwrap();
    cite!(["401.2"]; assert!(shown.objects.iter().all(|o|o.id!=hidden)));
    cite!(["903.3"]; assert_eq!(shown.players[0].commanders[0].name,"Leader"));
    assert_eq!(shown.players[0].commanders[0].casts,vec![0;3]);
    assert_eq!(shown.players[0].commanders[0].damage,vec![0;3]);
    assert!(shown.objects.iter().filter(|o|o.commander).all(|o|o.owner!=0));
}
