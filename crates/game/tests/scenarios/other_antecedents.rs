//! “Other” contrasts with the object or group the text identifies (608.2c), which may
//! be a trigger participant, newly created permanents, or a preceding affected set.
use crate::support::{cite, mana_pool, paid};
use game::{CardDef, Game, Move, Target, Zone};

fn definitions(names: &[&str]) -> Vec<CardDef> {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    names.iter().map(|name| {
        let def = CardDef::from_faces(&catalog[*name]).unwrap();
        game::play::check(&def).unwrap();
        def
    }).collect()
}
fn resolve(g: &mut Game) {
    for _ in 0..12 {
        if g.stack.is_empty() || g.decision.is_some() { return; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("resolution did not settle");
}

#[test]
fn r608_2c_martial_coup_spares_the_complete_created_group() {
    let defs = definitions(&["Martial Coup", "Grizzly Bears", "Doubling Season"]);
    for (x, doubled) in [(4, false), (5, false), (6, true)] {
        let mut g = Game::new(defs.clone(), 2, 20);
        let bear = g.put(1, 1, Zone::Battlefield);
        if doubled { g.put(0, 2, Zone::Battlefield); }
        let card = g.put(0, 0, Zone::Hand);
        let spending = "W".repeat(x as usize + 2);
        g.players[0].pool = mana_pool(&spending);
        g.act(0, paid(&spending, Move::WithX(x, Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
        resolve(&mut g);
        let tokens: Vec<_> = g.battlefield.iter().filter(|o| g.obj(**o).token()).collect();
        // Uncited (Oracle): Martial Coup destroys creatures other than the tokens it created.
        cite!(["608.2c", "614.6"]; case: format!("X={x}, doubled={doubled}");
            assert_eq!(tokens.len(), (x * if doubled { 2 } else { 1 }) as usize));
        cite!(["608.2c"]; assert_eq!(g.obj(bear).zone == Zone::Battlefield, x < 5));
    }
}

#[test]
fn r608_2c_white_suns_twilight_spares_its_mites_and_keeps_its_life_gain() {
    let defs = definitions(&["White Sun's Twilight", "Grizzly Bears"]);
    for x in [4, 5] {
        let mut g = Game::new(defs.clone(), 2, 20);
        let bear = g.put(1, 1, Zone::Battlefield);
        let card = g.put(0, 0, Zone::Hand);
        let spending = "W".repeat(x as usize + 2);
        g.players[0].pool = mana_pool(&spending);
        g.act(0, paid(&spending, Move::WithX(x, Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
        let branch = g.clone();
        for mut g in [g, branch] {
            resolve(&mut g);
            // Uncited (Oracle): X Mites remain, X life is gained, other creatures die at X>=5.
            cite!(["608.2c"]; assert_eq!(g.battlefield.iter().filter(|o| g.obj(**o).token()).count(), x as usize));
            cite!(["119.3"]; assert_eq!(g.players[0].life, 20 + x as i32));
            cite!(["608.2c"]; assert_eq!(g.obj(bear).zone == Zone::Battlefield, x < 5));
        }
    }
}

#[test]
fn r608_2c_in_oketras_name_has_disjoint_zombie_and_other_creature_groups() {
    let mut g = Game::new(definitions(&["In Oketra's Name", "Walking Corpse", "Grizzly Bears"]), 2, 20);
    let zombie = g.put(0, 1, Zone::Battlefield);
    let bear = g.put(0, 2, Zone::Battlefield);
    let theirs = g.put(1, 1, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool = mana_pool("WW");
    g.act(0, paid("WW", Move::Cast { card, targets: vec![] })).unwrap();
    resolve(&mut g);
    // Uncited (Oracle): Zombies get +2/+1; other creatures get +1/+1, both only under your control.
    cite!(["608.2c", "611.2c"]; assert_eq!((g.power(zombie), g.toughness(zombie)), (Some(4), Some(3))));
    cite!(["608.2c", "611.2c"]; assert_eq!((g.power(bear), g.toughness(bear)), (Some(3), Some(3))));
    cite!(["608.2c"]; assert_eq!((g.power(theirs), g.toughness(theirs)), (Some(2), Some(2))));
}

#[test]
fn r603_3d_illicit_masquerade_excludes_the_dying_creatures_graveyard_incarnation() {
    let mut g = Game::new(definitions(&["Illicit Masquerade", "Grizzly Bears", "Elvish Visionary", "Doom Blade"]), 2, 20);
    g.put(0, 0, Zone::Battlefield);
    let bear = g.put(0, 1, Zone::Battlefield);
    g.set_initial_counters(bear, "impostor", 1);
    let other = g.put(0, 2, Zone::Graveyard);
    g.put(0, 1, Zone::Library); // Elvish Visionary's entry trigger will draw this card.
    let blade = g.put(0, 3, Zone::Hand);
    g.players[0].pool = mana_pool("BB");
    g.act(0, paid("BB", Move::Cast { card: blade, targets: vec![vec![Target::Object(bear)]] })).unwrap();
    resolve(&mut g);
    let dead = *g.players[0].graveyard.iter().find(|o| g.obj(**o).chars.name == "Grizzly Bears").unwrap();
    // Uncited (Oracle): “other” excludes the creature whose death triggered this ability.
    cite!(["400.7e"]; assert_ne!(dead, bear));
    cite!(["115.1"]; refused("an illegal target, Grizzly Bears: it is the object the target must be other than (115.1)"):
        g.act(0, Move::ChooseTargets(vec![vec![Target::Object(dead)]])));
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(other)]])).unwrap();
    resolve(&mut g);
    cite!(["608.2c"]; assert!(g.exile.iter().any(|o| g.obj(*o).chars.name == "Grizzly Bears")));
    cite!(["608.2c"]; assert!(g.battlefield.iter().any(|o| g.obj(*o).chars.name == "Elvish Visionary")));
}

#[test]
fn r611_2c_the_complement_keeps_the_first_affected_group_after_its_power_changes() {
    let spell = CardDef::new("Contrast", "{0}", "Instant", "Creatures with power 2 or less get +1/+1 until end of turn. Other creatures get +2/+2 until end of turn.", None).unwrap();
    game::play::check(&spell).unwrap();
    let small = CardDef::new("Small", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap();
    let large = CardDef::new("Large", "{0}", "Creature — Bear", "", Some((4, 4))).unwrap();
    let mut g = Game::new(vec![spell, small, large], 2, 20);
    let small = g.put(0, 1, Zone::Battlefield);
    let large = g.put(0, 2, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    resolve(&mut g);
    // Uncited (test text): “other” contrasts with the first set, whose bonus changes its power.
    cite!(["608.2c", "611.2c"]; assert_eq!((g.power(small), g.toughness(small)), (Some(3), Some(3))));
    cite!(["608.2c", "611.2c"]; assert_eq!((g.power(large), g.toughness(large)), (Some(6), Some(6))));
}

#[test]
fn r109_5_creation_does_not_change_whom_other_players_excludes() {
    let def = CardDef::new("Players", "{0}", "Sorcery", "Create a 1/1 white Soldier creature token. Each other player loses 1 life.", None).unwrap();
    game::play::check(&def).unwrap();
    let mut g = Game::new(vec![def], 3, 20);
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    resolve(&mut g);
    // Uncited (test text): other players excludes the controller, not the created token.
    cite!(["109.5", "608.2c"]; assert_eq!(g.players.iter().map(|p| p.life).collect::<Vec<_>>(), vec![20, 19, 19]));
}

#[test]
fn r611_2c_an_empty_preceding_group_still_binds_the_complement() {
    let mut g = Game::new(definitions(&["In Oketra's Name", "Grizzly Bears"]), 2, 20);
    let bear = g.put(0, 1, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool = mana_pool("WW");
    g.act(0, paid("WW", Move::Cast { card, targets: vec![] })).unwrap();
    resolve(&mut g);
    // Uncited (Oracle): no Zombies does not suppress the bonus to other creatures.
    cite!(["608.2c", "611.2c"]; assert_eq!((g.power(bear), g.toughness(bear)), (Some(3), Some(3))));
}
