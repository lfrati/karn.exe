//! Commander color identity is immutable designation, not the visible command-zone contents.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Zone};
use ir::{Color, mana::ManaSymbol};

fn game(commander: Option<&str>) -> Game {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let mut names=vec!["Command Tower","Arcane Signet","Counterspell"];
    if let Some(commander)=commander { names.push(commander); }
    let defs=names.iter().map(|name| {
        let face=faces.iter().find(|f|f.name==*name).unwrap();
        let d=CardDef::from_face(face).unwrap();game::play::check(&d).unwrap();d
    }).collect::<Vec<_>>();
    let mut g=Game::new(defs,3,40);
    if commander.is_some() { let c=g.put(0,3,Zone::Command);g.designate_commander(c).unwrap(); }
    g
}
fn activate(g:&mut Game,source:ObjId,adding:Vec<Vec<ManaSymbol>>) -> Result<(),String> {
    let action=Move::Activate{source,ability:0,targets:vec![],paying:vec![]};
    g.act(0,if adding.is_empty(){action}else{Move::Adding(adding,Box::new(action))})
}


#[test]
fn r903_4f_tower_and_signet_offer_exactly_the_commanders_identity() {
    for def in [0,1] {
        let mut g=game(Some("Y'shtola, Night's Blessed"));
        let source=g.put(0,def,Zone::Battlefield);
        let colors=vec![vec![ManaSymbol::Color(Color::White)],vec![ManaSymbol::Color(Color::Blue)],vec![ManaSymbol::Color(Color::Black)]];
        for color in [Color::Red,Color::Green] {
            cite!(["903.4"]; case: format!("{color:?}"); refused("that ability doesn't offer the mana said for it: say one of what it offers (608.2d)"): activate(&mut g,source,vec![vec![ManaSymbol::Color(color)]]));
            assert!(!g.obj(source).tapped);
        }
        cite!(["903.4"]; refused("that ability doesn't offer the mana said for it: say one of what it offers (608.2d)"): activate(&mut g,source,vec![vec![ManaSymbol::Colorless]]));
        for mana in colors {
            let mut branch=g.clone();activate(&mut branch,source,vec![mana]).unwrap();
            assert_eq!(branch.players[0].pool.total(),1);
            assert!(branch.obj(source).tapped);
        }
    }
}

#[test]
fn r903_4a_identity_survives_hidden_zone_changes_and_uses_the_mana_ability_controller() {
    for zone in [Zone::Hand,Zone::Library,Zone::Graveyard,Zone::Exile,Zone::Battlefield] {
        let mut g=game(Some("Yargle, Glutton of Urborg"));
        g.move_object(g.command[0],zone);
        if matches!(zone,Zone::Graveyard|Zone::Exile) {
            g.state_based_actions();g.act(0,Move::Choose(game::state::Answer::No)).unwrap();
        }
        let source=g.put(0,0,Zone::Battlefield);
        activate(&mut g,source,vec![]).unwrap(); // one color: no question
        cite!(["903.4a"]; case: format!("{zone:?}"); assert_eq!(g.players[0].pool.colors[2],1));
        let theirs=g.put(1,0,Zone::Battlefield);
        g.act(0,Move::Pass).unwrap();
        // player 1 has no commander; player 0's designation cannot supply their colors.
        g.act(1,Move::Activate{source:theirs,ability:0,targets:vec![],paying:vec![]}).unwrap();
        cite!(["903.4f"]; case: format!("{zone:?}"); assert_eq!(g.players[1].pool.total(),0));
        assert!(g.obj(theirs).tapped);
    }
}

#[test]
fn r903_4f_no_commander_or_colorless_identity_produces_no_mana_and_no_impossible_choice() {
    for commander in [None,Some("Ultron, Machine Overlord")] {
        let mut g=game(commander);
        let source=g.put(0,0,Zone::Battlefield);
        activate(&mut g,source,vec![]).unwrap();
        cite!(["903.4f"]; case: format!("{commander:?}"); assert_eq!(g.players[0].pool.total(),0));
        assert!(g.obj(source).tapped);
        assert!(g.decision.is_none());
    }
}

#[test]
fn r601_2g_explicit_payment_validates_commander_colors() {
    for (commander,payable) in [("Y'shtola, Night's Blessed",true),("Yargle, Glutton of Urborg",false)] {
        let mut g=game(Some(commander));
        let tower=g.put(0,0,Zone::Battlefield);let signet=g.put(0,1,Zone::Battlefield);
        let card=g.put(0,2,Zone::Hand);let target=g.put(1,1,Zone::Stack);
        let plan = [tower, signet].into_iter().map(|source| game::legal::Step { plan: vec![], x: None, life: vec![], with: None,
            source, ability: 0, paying: vec![], adding: vec![vec![ManaSymbol::Color(Color::Blue)]], chose: vec![], cost_branches: None,
        }).collect();
        let before = format!("{g:?}");
        let result = g.act(0, paid("UU", Move::WithMana(plan, Box::new(Move::Cast { card, targets: vec![vec![game::Target::Object(target)]] }))));
        if payable {
            cite!(["601.2g", "903.4"]; case: commander; ok: result);
            assert_eq!(g.players[0].pool.total(), 0);
            assert!(g.obj(tower).tapped && g.obj(signet).tapped);
        } else {
            cite!(["601.2g", "903.4"]; case: commander; refused("that mana ability offers no such choice of mana (608.2d)"): result);
            assert_eq!(format!("{g:?}"), before);
            assert!(!g.obj(tower).tapped && !g.obj(signet).tapped);
        }

    }
}
