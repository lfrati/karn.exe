//! Oracle phrasing families: typed readings, canonical printing, and nearby refusals.
use grammar::{ability::abilities, kernel::Outcome};
use oracle::token::tokenize;

fn read(text: &str) -> (ir::Ability, String) {
    let rule = &abilities().ability;
    let Outcome::Accepted { value, .. } = rule.parse_all(&tokenize(text)) else {
        panic!("{text}: {:?}", rule.parse_all(&tokenize(text)));
    };
    let printed = rule.print_text(&value, true).unwrap_or_else(|| panic!("{text}: no printing for {value:#?}"));
    let Outcome::Accepted { value: again, .. } = rule.parse_all(&tokenize(&printed)) else {
        panic!("{text}: printing does not read: {printed}");
    };
    assert_eq!(value, again, "{text}: {printed}");
    (value, printed)
}
fn declines(text: &str) {
    assert!(matches!(abilities().ability.parse_all(&tokenize(text)), Outcome::Declined { .. }), "{text}");
}

/// CR 700.3b: membership describes each object; the pile is never a permanent or player.
#[test]
fn r700_3b_pile_membership() {
    for text in [
        "−6: Separate all permanents target player controls into two piles. That player sacrifices all permanents in the pile of their choice.",
        "Destroy all creatures in the pile of your choice.",
        "Exile all cards in that pile.",
        "Put all cards from the pile of your choice onto the battlefield under your control.",
        "Return all cards in the other pile to your hand.",
    ] {
        let (value, _) = read(text);
        assert!(serde_json::to_string(&value).unwrap().contains("InPile"));
    }
    for text in ["Destroy target pile.", "Target player in that pile draws a card.", "Sacrifice all permanents in the pile of choice."] {
        declines(text);
    }
}

/// CR 205.2a and 608.2c: iterate card types, retaining which type each instruction names.
#[test]
fn r205_2a_iterate_value_classes() {
    for text in [
        "When ~ enters, reveal the top ten cards of your library. For each card type, you may put a card of that type from among the revealed cards into your hand. Put the rest on the bottom of your library in a random order.",
        "Reveal the top X cards of your library. For each card type, you may exile a card of that type from among them. Put the rest into your graveyard.",
        "For each color, add one mana of that color.",
        "For each basic land type, choose a land of the chosen type.",
    ] {
        let (value, _) = read(text);
        assert!(serde_json::to_string(&value).unwrap().contains("ForEachValue"));
    }
    for text in ["For each damage, draw a card.", "For each card types, draw a card.", "For each power, draw a card."] {
        declines(text);
    }
}

/// CR 606.3: timing permission remains scoped to the stated planeswalker's loyalty abilities.
#[test]
fn r606_3_scoped_activation_timing() {
    for text in [
        "As long as ~ entered this turn, you may activate her loyalty abilities any time you could cast an instant.",
        "You may activate loyalty abilities of target planeswalker any time you could cast an instant.",
        "You may activate its activated abilities any time you could cast an instant.",
    ] {
        let (value, _) = read(text);
        let json = serde_json::to_string(&value).unwrap();
        assert!(json.contains("ActivateAtInstantSpeed"));
        assert!(!json.contains("LoyaltyAtInstantSpeed"));
    }
    for text in ["You may activate her triggered abilities any time you could cast an instant.", "You may activate loyalty spells of ~ any time you could cast an instant."] {
        declines(text);
    }
}

/// CR 107.4b: generic mana is a cost component, not a color or type of produced mana.
#[test]
fn r107_4b_generic_mana_spending() {
    let (value, _) = read("{T}: Add {W}{U}{B}{R}{G}. This mana can't be spent to pay generic mana costs.");
    assert!(serde_json::to_string(&value).unwrap().contains("Generic"));
    for text in ["This mana can't be spent to pay generic mana creatures.", "This mana can't be spent to cast generic mana costs."] {
        declines(text);
    }
}

/// CR 500.1 and 614.1a: replacing the beginning of a turn is distinct from an extra turn.
#[test]
fn r500_1_turn_begin_replacements() {
    for text in [
        "If you would begin your turn while ~ is tapped, you may skip that turn instead. If you do, untap ~.",
        "If a player would begin a turn, that player skips that turn instead.",
    ] {
        let (value, _) = read(text);
        assert!(serde_json::to_string(&value).unwrap().contains("BeginTurn"));
    }
    declines("If target creature would begin a turn, draw a card.");
}

/// CR 608.2d: an omitted noun still selects an object from the previously named group.
#[test]
fn r608_2d_choice_ellipsis() {
    for text in [
        "Search your library for three cards and reveal them. Target opponent chooses one. Put that card into your hand and the rest into your graveyard. Then shuffle.",
        "Reveal three cards. You choose one.",
        "Target player chooses one.",
    ] {
        let (value, _) = read(text);
        assert!(serde_json::to_string(&value).unwrap().contains("One"));
    }
    for text in ["Target opponent chooses two.", "Target creature chooses one.", "Choose one card player."] {
        declines(text);
    }
}

/// CR 107.1a: one trailing rounding instruction covers each fractional amount separately.
#[test]
fn r107_1a_round_each_amount() {
    for text in [
        "When you cast ~, you gain half X life and draw half X cards. Round down each time.",
        "Target player draws cards equal to half the number of cards in their library and loses half their life. Round up each time.",
        "Each player loses half their life, then discards half the cards in their hand, then sacrifices half the creatures they control of their choice. Round up each time.",
    ] {
        let (value, _) = read(text);
        assert!(serde_json::to_string(&value).unwrap().contains("RoundEach"));
    }
    declines("Draw half X cards. Round sideways each time.");
}

/// CR 608.2i: an ordinal over the game does not become an ordinal over just this turn.
#[test]
fn r608_2i_game_cast_ordinal() {
    for text in [
        "If ~ is the first spell you've cast this game, you may cast it without paying its mana cost.",
        "If that spell is the second creature spell you have cast this game, draw a card.",
    ] {
        let (value, _) = read(text);
        assert!(serde_json::to_string(&value).unwrap().contains("NthCastThisGame"));
    }
    for text in ["If ~ is the first creature you've cast this game, draw a card.", "If target player is the first spell you've cast this game, draw a card."] {
        declines(text);
    }
}

/// CR 113.7a and 608.2i: an ability's source retains its own object description.
#[test]
fn r113_7a_ability_source_of() {
    for text in [
        "When ~ enters, counter up to one target activated or triggered ability. If an ability of an artifact, creature, or planeswalker is countered this way, that permanent loses all abilities for as long as ~ remains on the battlefield.",
        "Counter target activated ability of an artifact.",
        "Counter target triggered ability from a creature.",
    ] {
        let (value, _) = read(text);
        assert!(serde_json::to_string(&value).unwrap().contains("Source"));
    }
    for text in ["Counter target creature of an artifact.", "Counter target activated ability of a player."] {
        declines(text);
    }
}

/// CR 119.1: ranking life totals and its tied case remain explicit.
#[test]
fn r119_1_life_rank_conditions() {
    for text in [
        "Whenever ~ attacks the player with the most life or tied for most life, create a 1/1 white Vampire creature token with lifelink.",
        "Whenever ~ attacks while you have the most life or are tied for most life, you draw a card and you lose 1 life.",
        "If you have the least life or are tied for least life, draw a card.",
    ] {
        let (value, _) = read(text);
        assert!(serde_json::to_string(&value).unwrap().contains("MostOf"));
    }
    declines("If you have the most life or are tied for least life, draw a card.");
}

/// CR 608.2c and 202.3: moving the endpoints does not change the selected objects or constraints.
#[test]
fn r608_2c_endpoints_before_objects() {
    for text in [
        "Return from your graveyard to the battlefield any number of target creature cards that each have a different mana value X or less.",
        "Return from your graveyard to your hand two target creature cards with different names.",
        "Put from your hand onto the battlefield two creature cards.",
    ] {
        read(text);
    }
    for text in [
        "Return from your graveyard to the battlefield target creature card from your hand.",
        "Return from your graveyard to the battlefield target creature card that each have a different mana value X or less.",
    ] {
        declines(text);
    }
}

/// CR 121.1 and 608.2i: a card's draw history is distinct from its current hand membership.
#[test]
fn r608_2i_drawn_card_history() {
    for text in [
        "At the beginning of your draw step, you may draw two additional cards. If you do, choose two cards in your hand drawn this turn. For each of those cards, pay 4 life or put the card on top of your library.",
        "Exile a card drawn this turn.",
        "Reveal two cards in your hand drawn this game.",
    ] {
        let (value, _) = read(text);
        assert!(serde_json::to_string(&value).unwrap().contains("Did"));
    }
    declines("Choose two players drawn this turn.");
}

/// CR 102.3 and 608.2c: a relative opponent and the first instructed player keep distinct references.
#[test]
fn r608_2c_ordered_players_and_relative_opponents() {
    for text in [
        "At the beginning of each player's upkeep, that player chooses target player who controls more creatures than they do and is their opponent. The first player may reveal cards from the top of their library until they reveal a creature card. If the first player does, that player puts that card onto the battlefield and all other cards revealed this way into their graveyard.",
        "The second player draws a card.",
        "Target player who is your opponent draws a card.",
    ] {
        read(text);
    }
    declines("The first creature who is your opponent draws a card.");
}

/// CR 118.9 and 702.122: the substituted keyword cost and its source remain explicit.
#[test]
fn r118_9_keyword_cost_substitution() {
    for text in [
        "You may remove a loyalty counter from a planeswalker you control rather than pay ~'s crew cost.",
        "You may discard a card rather than pay its equip cost.",
    ] {
        let (value, _) = read(text);
        assert!(serde_json::to_string(&value).unwrap().contains("AlternativeKeywordCost"));
    }
    for text in ["You may discard a card rather than pay its flying cost.", "You may discard a card rather than pay target player's crew cost."] {
        declines(text);
    }
}

/// CR 602.2: an activated ability's source does not create a second interpretation of the event.
#[test]
fn r602_2_activation_source_alias() {
    let (of, _) = read("Whenever you activate an ability of an artifact, draw a card.");
    let (from, _) = read("Whenever you activate an ability from an artifact, draw a card.");
    assert_eq!(of, from);
    read("If you haven't activated a loyalty ability of a planeswalker this turn, draw a card.");
}

/// CR 700.2: the bare choice header remains modal, with or without its trailing dash.
#[test]
fn r700_2_choice_header_is_not_object_ellipsis() {
    for text in ["Choose one —", "Choose one."] {
        let (value, _) = read(text);
        assert!(serde_json::to_string(&value).unwrap().contains("ChooseModes"));
    }
    read("An opponent chooses one —");
    read("If you discarded a card this turn, you choose one instead.");
    declines("You choose one —");
}

/// CR 602.2b: source-qualified activation cost rules use one static representation.
#[test]
fn r602_2b_activation_cost_source_alias() {
    let (of, _) = read("Activated abilities of creatures you control cost {2} less to activate.");
    let (from, _) = read("Activated abilities from creatures you control cost {2} less to activate.");
    assert_eq!(of, from);
}

/// CR 608.2c: role-local player pronouns do not duplicate every noun-phrase reading.
#[test]
fn r608_2c_role_local_pronouns_in_permissions() {
    read("{T}, Exile ~: Each player exiles the top seven cards of their library. Until your next turn, players may play cards they exiled this way, and they can't play cards from their hand. Activate only as a sorcery.");
}

/// CR 609.4 and 702.11: targetability can disregard an ability without removing it.
#[test]
fn r609_4_targetability_as_though() {
    for text in [
        "Creatures your opponents control can be the targets of spells and abilities as though they didn't have hexproof. Ward abilities of those creatures don't trigger.",
        "Creatures your opponents control with hexproof can be the targets of spells and abilities you control as though they didn't have hexproof.",
        "Your opponents and permanents your opponents control with hexproof can be the targets of spells and abilities you control as though they didn't have hexproof.",
        "{1}, {T}: Until end of turn, your opponents and creatures your opponents control with hexproof can be the targets of spells and abilities you control as though they didn't have hexproof.",
        "{G}: Until end of turn, ~ can be the target of spells and abilities controlled by target player as though it didn't have shroud.",
    ] {
        read(text);
    }
    declines("Target creature can be the target of creatures as though it didn't have hexproof.");
}

/// CR 101.4 and 608.2c: each repeated domain recomputes its own minimum and choices.
#[test]
fn r608_2c_same_way_procedure() {
    for text in [
        "Each player chooses a number of lands they control equal to the number of lands controlled by the player who controls the fewest, then sacrifices the rest. Players discard cards and sacrifice creatures the same way.",
        "Each player chooses a number of lands they control equal to the number of lands controlled by the player who controls the fewest, then sacrifices the rest. Players sacrifice creatures and discard cards the same way.",
        "{4}{W}, {T}, Sacrifice ~: Each player chooses a number of lands they control equal to the number of lands controlled by the player who controls the fewest, then sacrifices the rest. Players discard cards and sacrifice creatures the same way.",
        "Each player chooses a number of permanents they control equal to the number of permanents controlled by the player who controls the fewest, then sacrifices the rest. Each player discards cards the same way.",
    ] {
        let (value, _) = read(text);
        let json = serde_json::to_string(&value).unwrap();
        assert!(json.contains("LeastCount") && json.contains("SameWay"));
    }
    declines("Each player chooses a number of players equal to the number of players controlled by the player who controls the fewest.");
    declines("Each player sacrifices players the same way.");
}
