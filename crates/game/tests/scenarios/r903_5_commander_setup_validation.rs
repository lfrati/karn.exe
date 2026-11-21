//! Managed Commander setup (903.5–903.7), with real cards.
//! Each deck is the named cards plus Wastes (basic, colorless, no basic land type) up to the
//! 99 cards beside the commander, so only the named cards decide legality.
use crate::support::cite;
use crate::support;
use support::*;

/// Start a Commander game: one (commander, cards) pair per player.
fn start(seats: &[(&str, &[&str])]) -> Result<Game, game::start::Refused> {
    let mut defs: Vec<CardDef> = Vec::new();
    let mut index = std::collections::BTreeMap::<String, usize>::new();
    let mut id = |name: &str, defs: &mut Vec<CardDef>| *index.entry(name.to_owned()).or_insert_with(|| {
        defs.push(oracle(name));
        defs.len() - 1
    });
    let mut decks = Vec::new();
    let mut commanders = Vec::new();
    for (commander, cards) in seats {
        commanders.push(id(commander, &mut defs));
        let mut deck: Vec<usize> = cards.iter().map(|c| id(c, &mut defs)).collect();
        let wastes = id("Wastes", &mut defs);
        deck.resize(99.max(deck.len()), wastes);
        decks.push(deck);
    }
    Game::start_commander(defs, &decks, &commanders, 0, 7)
}

fn accepted(seats: &[(&str, &[&str])]) -> Game {
    start(seats).unwrap_or_else(|e| panic!("refused: {e}"))
}

const BLACK: &str = "Yargle, Glutton of Urborg";
const GREEN: &str = "Rhonas the Indomitable";
const WHITE: &str = "Thalia, Guardian of Thraben";
const BANT: &str = "Aang and Katara";
const IZZET: &str = "Adeliz, the Cinder Wind";
const COLORLESS: &str = "Graaz, Unstoppable Juggernaut";

#[test]
fn r903_4_mana_symbols_in_rules_text_are_part_of_the_color_identity() {
    // colorless lands whose activation costs contain {W}/{G} and {R}/{G}
    cite!(["903.5c"]; refused("player 0: Gavony Township outside commander's color identity (903.5c)"): start(&[(GREEN, &["Gavony Township"]), (BLACK, &[])]));
    cite!(["903.5c"]; refused("player 0: Kessig Wolf Run outside commander's color identity (903.5c)"): start(&[(GREEN, &["Kessig Wolf Run"]), (BLACK, &[])]));
    accepted(&[(BANT, &["Gavony Township"]), (BLACK, &[])]);
}

#[test]
fn r903_4_hybrid_symbols_count_both_colors() {
    cite!(["903.5c"]; refused("player 0: Kitchen Finks outside commander's color identity (903.5c)"): start(&[(GREEN, &["Kitchen Finks"]), (BLACK, &[])]));
    accepted(&[(BANT, &["Kitchen Finks"]), (BLACK, &[])]);
}

#[test]
fn r903_4_a_color_indicator_is_part_of_the_color_identity() {
    // Dryad Arbor has no mana cost; its color indicator is green
    cite!(["903.5c"]; refused("player 0: Dryad Arbor outside commander's color identity (903.5c)"): start(&[(BLACK, &["Dryad Arbor"]), (BLACK, &[])]));
}

#[test]
fn r903_4c_reminder_text_is_ignored() {
    // Syndic of Tithes: "Extort (Whenever you cast a spell, you may pay {W/B}. ...)"
    cite!(["903.4c"]; ok: start(&[(WHITE, &["Syndic of Tithes"]), (BLACK, &[])]));
}

#[test]
fn r903_4d_the_back_face_is_part_of_the_color_identity() {
    // Daybreak Ranger is green; Nightfall Predator's text costs {R}
    cite!(["903.5c"]; refused("player 0: Daybreak Ranger outside commander's color identity (903.5c)"): start(&[(GREEN, &["Daybreak Ranger"]), (BLACK, &[])]));
}

#[test]
fn r903_4e_an_adventure_is_part_of_the_color_identity() {
    // Twining Twins is blue; Swift Spiral costs {W}. Scalding Viper // Steam Clean is red and blue.
    cite!(["903.5c"]; refused("player 0: Twining Twins outside commander's color identity (903.5c)"): start(&[(IZZET, &["Twining Twins"]), (BLACK, &[])]));
    accepted(&[(IZZET, &["Scalding Viper"]), (BLACK, &[])]);
}

#[test]
fn r903_5d_a_basic_land_type_must_produce_only_the_commanders_colors() {
    // Tropical Island's only mana text is reminder text: 903.5c allows it, 903.5d does not
    cite!(["903.5d"]; refused("player 0: Tropical Island has a basic land type outside commander's colors (903.5d)"): start(&[(GREEN, &["Tropical Island"]), (BLACK, &[])]));
    accepted(&[(BANT, &["Tropical Island"]), (BLACK, &[])]);
    cite!(["903.5d"]; refused("player 0: Swamp has a basic land type outside commander's colors (903.5d)"): start(&[(COLORLESS, &["Swamp"]), (BLACK, &[])]));
    // Wastes is basic with no basic land type: any deck may include it
    accepted(&[(COLORLESS, &[]), (BLACK, &[])]);
}

#[test]
fn r903_5b_singleton_except_basic_lands() {
    cite!(["903.5b"]; refused("player 0: duplicate Grizzly Bears (903.5b)"): start(&[(GREEN, &["Grizzly Bears", "Grizzly Bears"]), (BLACK, &[])]));
    accepted(&[(BLACK, &["Snow-Covered Swamp"; 60]), (BLACK, &["Swamp"; 99])]);
    // the commander is one of the deck's 100 cards
    cite!(["903.5b"]; refused("player 0: duplicate Rhonas the Indomitable (903.5b)"): start(&[(GREEN, &[GREEN]), (BLACK, &[])]));
}

#[test]
fn r903_5a_exactly_one_hundred_cards() {
    let mut defs = vec![oracle(BLACK), oracle("Wastes")];
    defs.truncate(2);
    for n in [98, 100] {
        let decks = vec![vec![1; n], vec![1; 99]];
        let e = Game::start_commander(defs.clone(), &decks, &[0, 0], 0, 7).err().unwrap_or_else(|| panic!("{n} accepted"));
        cite!(["903.5a"]; case: n; assert_eq!((e.class, e.why.as_str()), (game::simulation::Class::Illegal, "player 0: exactly 100 cards including the commander (903.5a)")));
    }
}

#[test]
fn r903_3_the_commander_is_legendary() {
    // Grizzly Bears is a nonlegendary "Creature — Bear": "Each deck has a legendary card
    // designated as its commander" (903.3)
    cite!(["903.3"]; refused("a commander is a legendary card (903.3)"): start(&[("Grizzly Bears", &[]), (BLACK, &[])]));
}

/// A physical card outside the battlefield has its front face's characteristics (712.8a); a
/// flip card has its normal ones (710.2). A back or flipped face's name names no card to put
/// in a deck, and cannot make the card a legendary creature with fewer colors.
#[test]
fn r903_3_a_back_or_flipped_face_is_not_a_commander_card() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    // The Legend of Kyoshi is a green Saga; Avatar Kyoshi is its back face.
    // Cunning Bandit is a nonlegendary red creature; Azamuki is its flipped side.
    for face in ["Avatar Kyoshi", "Azamuki, Treachery Incarnate"] {
        assert!(catalog.get(face).is_none(), "{face} must not be a separate physical card");
    }
}

#[test]
fn r903_6_7_four_players_start_at_forty_with_commanders_in_the_command_zone() {
    let g = accepted(&[(BLACK, &[]), (GREEN, &[]), (WHITE, &[]), (BANT, &[])]);
    assert_eq!(g.command.len(), 4);
    for (p, name) in [BLACK, GREEN, WHITE, BANT].iter().enumerate() {
        let player = &g.players[p];
        cite!(["903.7"]; assert_eq!(player.life, 40, "903.7"));
        cite!(["903.7"]; assert_eq!(player.hand.len(), 7, "903.7"));
        cite!(["903.6"]; assert_eq!(player.library.len(), 92, "903.6: the other 99 cards, less the hand"));
        let commander = g.command.iter().copied().find(|o| g.obj(*o).owner == p).expect("in the command zone");
        assert_eq!(g.defs[g.obj(commander).card.unwrap()].name(), *name);
        assert!(g.is_commander(commander));
    }
    assert!(matches!(g.decision, Some(Decision::Mulligan { player: 0 })), "{:?}", g.decision);
}

#[test]
fn r903_designations_are_public_but_hidden_cards_stay_hidden() {
    let g = accepted(&[(BLACK, &[]), (GREEN, &[]), (WHITE, &[])]);
    let seen = game::snapshot::Snapshot::of(&g, Some(1));
    for p in 0..3 {
        cite!(["903.6", "400.2"]; case: p; assert_eq!(seen.players[p].commanders.len(), 1, "every player's commander is public"));
        assert_eq!(seen.players[p].commanders[0].casts, vec![0, 0, 0]);
    }
    cite!(["402.3"]; assert!(seen.players[0].hand.is_none()));
    for o in g.players[0].hand.iter().chain(&g.players[0].library) {
        assert!(!seen.objects.iter().any(|s| s.id == *o), "player 0's hidden card {o:?} is shown to player 1");
    }
}
