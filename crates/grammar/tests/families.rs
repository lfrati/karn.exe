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

/// CR 502.3–4: this turn-based instruction happens with untapping, before priority.
#[test]
fn r502_3_as_permanents_untap() {
    for text in [
        "{T}: Add one mana of any color. During your next untap step, as you untap your permanents, return ~ to its owner's hand.",
        "During that player's next untap step, as that player untaps that player's permanents, draw a card.",
    ] {
        let (value, _) = read(text);
        assert!(serde_json::to_string(&value).unwrap().contains("AsPermanentsUntap"));
    }
    declines("During your next untap step, as target player untaps your permanents, draw a card.");
    declines("During your next upkeep, as you untap your permanents, draw a card.");
    declines("During your next untap step, as you untap your creatures, draw a card.");
}

/// CR 107.1a and 608.2b: evenly rounded damage differs from a chosen distribution (601.2d).
#[test]
fn r107_1a_even_damage_division() {
    for text in [
        "~ deals X damage divided evenly, rounded down, among any number of targets.",
        "~ deals X damage divided evenly, rounded down, among all creatures target opponent controls.",
    ] {
        let (value, _) = read(text);
        let json = serde_json::to_string(&value).unwrap();
        assert!(json.contains("Evenly") && json.contains("false"));
        assert!(!json.contains("Chosen"));
    }
    let (chosen, _) = read("~ deals 3 damage divided as you choose among up to three target creatures.");
    assert!(serde_json::to_string(&chosen).unwrap().contains("Chosen"));
    for text in [
        "You may choose new targets for target spell.",
        "Change the target of target spell with a single target.",
        "Copy target instant or sorcery spell. You may choose new targets for the copy.",
    ] { read(text); }
    declines("Change any number of targets of target spell.");
    declines("~ deals X damage divided evenly, rounded down, among target spell.");
    declines("~ deals X damage divided evenly, rounded down, among any number of target.");
}

/// CR 110.2: passive controller modifiers describe current control, not past control.
#[test]
fn r110_2_passive_controller_relation() {
    let (active, _) = read("Destroy target creature target player controls.");
    let (passive, _) = read("Destroy target creature controlled by target player.");
    let (past, _) = read("Destroy target creature target player controlled.");
    assert_eq!(active, passive);
    assert_ne!(passive, past);
    read("At the beginning of your end step, gain control of target nonland permanent controlled by a player who was dealt combat damage by three or more Pirates this turn.");
}

/// CR 120.2 and 608.2i: the damage source and historical controller stay in the lookback.
#[test]
fn r608_2i_damage_time_before_source() {
    let (past, _) = read("Destroy target creature dealt damage this turn by a source you controlled.");
    let (present, _) = read("Destroy target creature dealt damage this turn by a source you control.");
    assert_ne!(past, present);
    // The old ordering also permits a trailing instruction duration; refuse to pick an AST.
    assert!(matches!(abilities().ability.parse_all(&tokenize("Destroy target creature dealt damage by a red instant spell this turn.")), Outcome::Ambiguous(_)));
    for text in [
        "If a creature dealt damage this turn by a source you controlled would die, exile it instead.",
        "Whenever another creature dealt damage this turn by a Spider you controlled dies, create a token that's a copy of that creature, except it's a Food artifact with \"{2}, {T}, Sacrifice this token: You gain 3 life,\" and it loses all other card types.",
        "Cast ~ only if you were dealt damage this turn by a red instant or sorcery spell.",
    ] { read(text); }
    let (before, _) = read("Cast ~ only if you were dealt damage this turn by a red instant or sorcery spell.");
    let (after, _) = read("Cast ~ only if you were dealt damage by a red instant or sorcery spell this turn.");
    assert_eq!(before, after);
    declines("Destroy target creature dealt damage this turn by target player.");
}

/// CR 508.4: one attacking token's defender alternatives preserve their distinct referents.
#[test]
fn r508_4_token_defender_alternatives() {
    let (value, _) = read("Whenever you attack, for each opponent, create a 1/1 white Human creature token that's tapped and attacking that player or a planeswalker they control.");
    let json = serde_json::to_string(&value).unwrap();
    assert!(json.contains("Attacking") && json.contains("Definite") && json.contains("Indefinite") && json.contains("They"));
    for text in [
        "Create a 1/1 white Soldier creature token that's tapped and attacking that opponent.",
        "Create two 1/1 white Soldier creature tokens that are attacking target player or planeswalker.",
    ] { read(text); }
    declines("Create a 1/1 white Soldier creature token that's attacking target spell.");
    declines("Create a 1/1 white Soldier creature token that's attacking target player and target planeswalker.");
}

/// CR 205.2a and 608.2d: a finite list restricts the existing card-type value domain.
#[test]
fn r205_2a_finite_card_type_choices() {
    for (text, allowed) in [
        ("Choose creature or land. Reveal the top four cards of your library. Put all cards of the chosen type revealed this way into your hand and the rest into your graveyard.", [ir::CardType::Creature, ir::CardType::Land]),
        ("Choose artifact or enchantment. Destroy all permanents of the chosen type.", [ir::CardType::Artifact, ir::CardType::Enchantment]),
    ] {
        let (value, _) = read(text);
        let ir::Ability::Text(effect) = value else { panic!() };
        let ir::Sentence::Do(body) = &effect.0[0] else { panic!() };
        let ir::Action::Choose(ir::Choice::Value { kind: ir::ValueKind::CardType, except, .. }) = &body.first.first.action else { panic!() };
        assert_eq!(except.len(), ir::CardType::ALL.len() - 2);
        for card_type in ir::CardType::ALL {
            assert_eq!(except.contains(&ir::Exclusion::CardType(*card_type)), !allowed.contains(card_type));
        }
    }
    declines("Choose creature or red.");
}

/// CR 608.2c: a definite reference names the number previously chosen or computed.
#[test]
fn r608_2c_definite_recalled_number() {
    let (that, _) = read("Choose a number. Draw cards equal to that number.");
    let (the, _) = read("Choose a number. Draw cards equal to the number.");
    assert_eq!(that, the);
    read("Choose a number. Destroy all artifacts and creatures with mana value equal to that number. Then target player reveals their hand and discards all nonland cards with mana value equal to the number.");
    // Numeric parity is a separate condition construction, still absent for either spelling.
    declines("If the number is odd, draw a card.");
    declines("If that number is odd, draw a card.");
    read("Draw cards equal to the number of creatures you control.");
    declines("Draw cards equal to the number of.");
}

/// CR 608.2c: each damage part keeps its own condition, within one damage action.
#[test]
fn r608_2c_independently_guarded_damage_parts() {
    let (value, _) = read("~ deals 3 damage to each creature without flying if {R} was spent to cast ~ and 3 damage to each creature with flying if {G} was spent to cast ~.");
    let ir::Ability::Text(effect) = value else { panic!() };
    let ir::Sentence::Do(body) = &effect.0[0] else { panic!() };
    let ir::Action::DealDamage(hits) = &body.first.first.action else { panic!() };
    assert_eq!(hits.len(), 2);
    assert!(hits.iter().all(|h| h.condition.is_some()));
    assert_ne!(hits[0].condition, hits[1].condition);
    assert!(body.first.first.mods.is_empty());
    read("~ deals 3 damage to each creature if {R} was spent to cast ~.");
    declines("~ deals 3 damage to target spell if {R} was spent to cast ~ and 3 damage to each creature if {G} was spent to cast ~.");
}

/// CR 702.72c: champion names linked exile by the permanent, rather than arbitrary exile.
#[test]
fn r702_72c_championed_with_source() {
    let (with, _) = read("When a Faerie is championed with ~, tap all lands target player controls.");
    let (by, _) = read("When a Faerie is championed by ~, tap all lands target player controls.");
    assert_eq!(with, by);
    let ir::Ability::Triggered { .. } = &with else { panic!() };
    read("Whenever a creature is championed with another creature, draw a card.");
    declines("When a player is championed with ~, draw a card.");
    declines("When a creature is championed with target spell, draw a card.");
}

/// CR 105.2c and 608.2c: adjective predicates still qualify the noun after numeric modifiers.
#[test]
fn r105_2c_adjective_relative_after_modifiers() {
    let (late, _) = read("−X: Exile each permanent with mana value X or less that's one or more colors.");
    let (early, _) = read("−X: Exile each permanent that's one or more colors with mana value X or less.");
    assert_eq!(late, early);
    let (late, _) = read("Destroy target creature with power 3 that's black.");
    let (early, _) = read("Destroy target creature that's black with power 3.");
    assert_eq!(late, early);
    declines("Destroy target player with power 3 that's black.");
}

/// CR 608.2c: "the other cards" is the complementary group, not source exclusion.
#[test]
fn r608_2c_named_complementary_group() {
    for text in [
        "Search your library for up to three monocolored cards with different names and exile them. An opponent chooses one of those cards. Shuffle that card into your library. You may cast the other cards without paying their mana costs. Exile ~.",
        "Put the other cards exiled this way on the bottom of your library in a random order.",
        "You may cast up to two spells from among the other cards exiled this way without paying their mana costs.",
        "Return the other cards exiled this way to the battlefield under their owners' control attached to that creature.",
        "Whenever one or more other creatures you control enter, if ~ is suspected, you may suspect one of the other creatures.",
    ] { read(text); }
    for noun in ["the other cards", "the other creatures"] {
        let Outcome::Accepted { value, .. } = grammar::selector::selectors().selector.parse_all(&tokenize(noun)) else { panic!("{noun}") };
        assert_eq!(value.quant, ir::Quant::Definite(ir::Ref::Anaphor(ir::Anaphor::Other { plural: true })));
        assert!(!value.set.other);
    }
    // Status removal is another construction; this family only supplies its object group.
    declines("If you do, ~ is no longer suspected.");
    declines("You may cast the other players without paying their mana costs.");
}

/// CR 608.2i: an action's player stays an active relative unless its lexicon declares By.
#[test]
fn r608_2i_passive_agents_need_declared_role() {
    for text in [
        "Each player discards their hand, then draws cards equal to the greatest number of cards a player discarded this way.",
        "Exile an artifact or creature card with mana value 1 you discarded this way.",
        "Return the creature you sacrificed this way to the battlefield.",
    ] { read(text); }
}

/// CR 601.2 and 400.7: caster and permission provenance both survive the passive spelling.
#[test]
fn r601_2_passive_caster_and_permission_provenance() {
    let (value, _) = read("Exile target nonland permanent. For as long as that card remains exiled, its owner may play it. A spell cast by an opponent this way costs {2} more to cast.");
    let json = serde_json::to_string(&value).unwrap();
    assert!(json.contains("ThisWay") && json.contains("By") && json.contains("Opponent"));
    let (passive, _) = read("A spell cast by an opponent this way costs {2} more to cast.");
    let (active, _) = read("A spell an opponent cast this way costs {2} more to cast.");
    assert_eq!(passive, active);
    read("If a spell was cast from exile by an opponent this turn, draw a card.");
    declines("A spell cast by target creature this way costs {2} more to cast.");
}

/// CR 608.2c–i: a shared definite player is repeated without losing the history window.
#[test]
fn r608_2c_shared_condition_subject() {
    let (shared, _) = read("Mill four cards. You may put a permanent card from among the cards milled this way into your hand. If you control a Squirrel or returned a Squirrel card to your hand this way, create a Food token.");
    let (explicit, _) = read("Mill four cards. You may put a permanent card from among the cards milled this way into your hand. If you control a Squirrel or you returned a Squirrel card to your hand this way, create a Food token.");
    assert_eq!(shared, explicit);
    let (shared, _) = read("If that player controls an artifact or was dealt damage this turn, draw a card.");
    let (explicit, _) = read("If that player controls an artifact or that player was dealt damage this turn, draw a card.");
    assert_eq!(shared, explicit);
    read("If you control an artifact and discarded a card this turn, draw a card.");
    declines("If an opponent controls an artifact and discarded a card this turn, draw a card.");
    declines("If target player controls an artifact and discarded a card this turn, draw a card.");
    declines("If you control a Squirrel or died this turn, draw a card.");
}

/// CR 115.1 and 702.62: one target ranges over independently qualified object descriptions.
#[test]
fn r115_1_qualified_target_alternatives() {
    read("When ~ enters, choose one —");
    read("Suspend 4—{R}");
    for text in [
        "• Remove two time counters from target permanent or suspended card.",
        "• Put two time counters on target permanent with a time counter on it or suspended card.",
    ] {
        let mode = &abilities().mode;
        let Outcome::Accepted { value, .. } = mode.parse_all(&tokenize(text)) else { panic!("{text}") };
        let printed = mode.print_text(&value, true).unwrap();
        let Outcome::Accepted { value: again, .. } = mode.parse_all(&tokenize(&printed)) else { panic!("{printed}") };
        assert_eq!(value, again);
    }
    for text in [
        "Put X time counters on target permanent with a time counter on it or suspended card.",
        "Put two charge counters on target permanent with two charge counters on it or suspended card.",
        "{T}: Choose target permanent you control or suspended card you own.",
    ] { read(text); }
    let rule = &grammar::selector::selectors().selector;
    let Outcome::Accepted { value: counter_target, .. } = rule.parse_all(&tokenize("target permanent with a time counter on it or suspended card")) else { panic!() };
    assert_eq!(counter_target.set.filter, ir::Filter::All(Vec::new()));
    let ir::Domain::Union(counter_arms) = counter_target.set.domain else { panic!() };
    assert_eq!(counter_arms[0].filter, ir::Filter::All(vec![ir::Filter::Atom(ir::Atom::Counters {
        kind: Some(ir::CounterKind("time".into())), count: ir::Count::AtLeast(ir::Amount::Const(1)),
    })]));
    assert!(matches!(&counter_arms[1].filter, ir::Filter::All(items) if items.contains(&ir::Filter::Atom(ir::Atom::Status(ir::Status::Suspended)))));
    let Outcome::Accepted { value, .. } = rule.parse_all(&tokenize("target permanent you control or suspended card you own")) else { panic!() };
    assert_eq!(value.quant, ir::Quant::Target(ir::Count::Exactly(ir::Amount::Const(1))));
    let ir::Domain::Union(arms) = value.set.domain else { panic!() };
    assert_eq!(arms.len(), 2);
    assert_eq!(arms[0].domain, ir::Domain::Permanent);
    assert_eq!(arms[1].domain, ir::Domain::Card);
    assert!(arms[0].controller.is_some() && arms[0].owner.is_none());
    assert!(arms[1].owner.is_some() && arms[1].controller.is_none());
    read("Counter target spell that targets a permanent or suspended card.");
    declines("Put two time counters on target permanent with a time counter on it or suspended card or.");
}

/// CR 608.2f: a simultaneous prefix retains the entire actor's action group.
#[test]
fn r608_2f_simultaneous_predicates() {
    let (value, _) = read("That player simultaneously sacrifices the artifact and returns the artifact card to the battlefield.");
    let (leading, _) = read("Simultaneously, that player sacrifices the artifact and returns the artifact card to the battlefield.");
    assert_eq!(value, leading);
    let ir::Ability::Text(effect) = value else { panic!() };
    let ir::Sentence::Led { lead: ir::Lead::Simultaneously, rest } = &effect.0[0] else { panic!() };
    let ir::Sentence::Do(body) = &**rest else { panic!() };
    assert!(body.rest.is_empty());
    assert!(body.first.subject.is_some());
    assert_eq!(body.first.rest.len(), 1);
    assert_eq!(body.first.rest[0].0, ir::Join::And);
    let (optional, _) = read("You may simultaneously sacrifice an artifact and return an artifact card from your graveyard to your hand.");
    let ir::Ability::Text(effect) = optional else { panic!() };
    let ir::Sentence::Led { rest, .. } = &effect.0[0] else { panic!() };
    let ir::Sentence::Do(body) = &**rest else { panic!() };
    assert!(body.first.may);
    for text in [
        "At the beginning of each player's upkeep, that player simultaneously untaps each tapped artifact, creature, and land they control and taps each untapped artifact, creature, and land they control.",
        "Simultaneously untap all tapped creatures and tap all untapped creatures.",
    ] { read(text); }
    declines("That player simultaneously sacrifices an artifact, then draws a card.");
    declines("Target creature simultaneously draws a card and discards a card.");
}

/// CR 608.2b: legality names the existing target set and preserves its required cardinality.
#[test]
fn r608_2b_existing_targets_remain_legal() {
    let (value, _) = read("{T}: Choose target artifact a player controls and target artifact card in that player's graveyard. If both targets are still legal as this ability resolves, that player simultaneously sacrifices the artifact and returns the artifact card to the battlefield.");
    let json = serde_json::to_string(&value).unwrap();
    assert!(json.contains("TargetsRemainLegal") && json.contains("Simultaneously"));
    let rule = &grammar::condition::conditions().condition;
    for (text, expected, domain) in [
        ("both targets are still legal as this ability resolves", Some(2), ir::Domain::Ability),
        ("all targets are still legal as this spell resolves", None, ir::Domain::Spell),
        ("both targets are still legal as that spell resolves", Some(2), ir::Domain::Spell),
    ] {
        let Outcome::Accepted { value, .. } = rule.parse_all(&tokenize(text)) else { panic!("{text}") };
        let ir::Condition::TargetsRemainLegal { source, expected: got } = &value else { panic!() };
        assert_eq!(*got, expected);
        assert_eq!(source.set.domain, domain);
        assert!(matches!(source.quant, ir::Quant::Definite(_)));
        let printed = rule.print_text(&value, true).unwrap();
        let Outcome::Accepted { value: again, .. } = rule.parse_all(&tokenize(&printed)) else { panic!("{printed}") };
        assert_eq!(value, again);
    }
    for text in [
        "If both creatures are still legal as this ability resolves, draw a card.",
        "If both targets are still legal as this creature resolves, draw a card.",
        "If all targets are still legal as target spell resolves, draw a card.",
    ] { declines(text); }
}

/// CR 115.1c and 701.65a: one quantified target includes permanent and spell alternatives.
#[test]
fn r115_1c_bare_type_and_untyped_target_heads() {
    read("When ~ enters, airbend up to one other target creature or spell.");
    read("Waterbend {8}: Transform ~.");
    for text in [
        "Exile up to one other target artifact or card.",
        "Exile target spell or creature.",
        "Exile another target land or spell.",
    ] { read(text); }
    let rule = &grammar::selector::selectors().selector;
    let Outcome::Accepted { value, .. } = rule.parse_all(&tokenize("up to one other target creature or spell")) else { panic!() };
    assert_eq!(value.quant, ir::Quant::Target(ir::Count::UpTo(ir::Amount::Const(1))));
    assert!(value.set.other);
    let ir::Domain::Union(arms) = value.set.domain else { panic!() };
    assert_eq!(arms.len(), 2);
    assert_eq!(arms[0].domain, ir::Domain::Permanent);
    assert_eq!(arms[0].filter, ir::Filter::All(vec![ir::Filter::Atom(ir::Atom::Type(ir::CardType::Creature))]));
    assert_eq!(arms[1], ir::ObjectSet::of(ir::Domain::Spell));
    // A shared typed head remains shared: neither alternative names a permanent.
    for text in ["target instant or sorcery spell", "target creature or artifact card"] {
        let Outcome::Accepted { value, .. } = rule.parse_all(&tokenize(text)) else { panic!("{text}") };
        assert!(matches!(value.set.domain, ir::Domain::Spell | ir::Domain::Card));
    }
    declines("Airbend up to one other target creature or player.");
    declines("Sacrifice target creature or spell.");
}

/// CR 111.3 and 305.6: a token's relative clause retains a whole subtype class.
#[test]
fn r111_3_whole_subtype_class_in_token_description() {
    let (relative, _) = read("Whenever ~ enters or attacks, create a tapped colorless land token named Everywhere that is every basic land type.");
    let (with, _) = read("Whenever ~ enters or attacks, create a tapped colorless land token named Everywhere with all basic land types.");
    assert_eq!(relative, with);
    let json = serde_json::to_string(&relative).unwrap();
    assert!(json.contains("Every") && json.contains("BasicLandTypes") && json.contains("Everywhere"));
    for text in [
        "Create two land tokens that are every nonbasic land type.",
        "Create a 1/1 colorless Shapeshifter creature token that's every creature type.",
        "Create a land token with flying that is every land type.",
    ] { read(text); }
    declines("Create a land token that is every color.");
    declines("Create a creature token that is every card type.");
    declines("Create two land tokens that is every basic land type.");
}

/// CR 608.2c: a count override retains the old count instead of adding a second procedure.
#[test]
fn r608_2c_replaced_instruction_counts() {
    let (value, _) = read("Search your library for up to two basic Forest cards, reveal those cards, and put one onto the battlefield tapped and the rest into your hand. Then shuffle. If there are two or more instant and/or sorcery cards in your graveyard, search your library for up to three basic Forest cards instead of two.");
    let ir::Ability::Text(effect) = value else { panic!() };
    let ir::Sentence::Led { rest, .. } = &effect.0[2] else { panic!() };
    let ir::Sentence::Do(body) = &**rest else { panic!() };
    assert!(body.first.first.mods.contains(&ir::Modifier::InsteadOfCount(ir::Amount::Const(2))));
    for text in [
        "Reveal the top five cards of your library. You may put a creature card from among them into your hand. Put the rest into your graveyard. If there are two or more instant and/or sorcery cards in your graveyard, put up to two creature cards from among the revealed cards into your hand instead of one.",
        "Target opponent reveals the top seven cards of their library. You may cast an instant or sorcery spell from among them without paying its mana cost. Then that player puts the rest into their graveyard. If there are two or more instant and/or sorcery cards in your graveyard, you may cast up to two instant and/or sorcery spells from among the revealed cards instead of one.",
        "Reveal the top eight cards of your library. You may put a creature card from among them onto the battlefield. Put the rest into your graveyard. If you control a creature with power 4 or greater, you may put two creature cards onto the battlefield instead of one.",
        "Search your library for a basic land card, reveal it, put it into your hand, then shuffle. If this spell was kicked, search your library for two basic land cards instead of one.",
        "Search your library for up to four basic land cards instead of three.",
    ] { read(text); }
    let (words, _) = read("Draw three cards instead of two.");
    let (digits, _) = read("Draw three cards instead of 2.");
    assert_eq!(words, digits);
    declines("Draw three cards instead of two lands.");
    declines("Draw three cards instead of a card.");
}

/// CR 702.30a: Echo queries an acquisition since the beginning of the last upkeep.
#[test]
fn r702_30_control_acquisition_history() {
    let (rule, _) = read("At the beginning of your upkeep, if this permanent came under your control since the beginning of your last upkeep, sacrifice it unless you pay {2}{G}.");
    let (short, _) = read("At the beginning of your upkeep, if ~ came under your control since your last upkeep, sacrifice it unless you pay {2}{G}.");
    assert_eq!(rule, short);
    let json = serde_json::to_string(&rule).unwrap();
    assert!(json.contains("CameUnderControl") && json.contains("Since") && json.contains("Upkeep"));
    read("At the beginning of your end step, if ~ didn't attack this turn, it deals 2 damage to you unless it came under your control this turn.");
    let rule = &grammar::condition::conditions().condition;
    let text = "that creature came under that player's control since the beginning of your last upkeep";
    let Outcome::Accepted { value, .. } = rule.parse_all(&tokenize(text)) else { panic!("{text}") };
    let ir::Condition::CameUnderControl { what, player, window } = &value else { panic!() };
    assert_eq!(what.set.domain, ir::Domain::Permanent);
    assert_eq!(player.set.domain, ir::Domain::Player);
    assert_eq!(*window, ir::Window::Since { step: ir::Step::Upkeep, whose: Box::new(ir::Selector::you()) });
    let printed = rule.print_text(&value, true).unwrap();
    let Outcome::Accepted { value: again, .. } = rule.parse_all(&tokenize(&printed)) else { panic!("{printed}") };
    assert_eq!(value, again);
    let (long, _) = read("If it attacked since the beginning of your last upkeep, draw a card.");
    let (short, _) = read("If it attacked since your last upkeep, draw a card.");
    assert_eq!(long, short);
    for text in [
        "If that player came under your control since your last upkeep, draw a card.",
        "If ~ came under a creature's control this turn, draw a card.",
        "If ~ came under your control, draw a card.",
    ] { declines(text); }
}

/// CR 601.3: casting restrictions retain the spell description and actor's timing.
#[test]
fn r601_3_scoped_casting_restrictions_and_caps() {
    let (fires, _) = read("You can cast spells only during your turn and you can cast no more than two spells each turn. You may cast spells with mana value less than or equal to the number of lands you control without paying their mana costs.");
    let json = serde_json::to_string(&fires).unwrap();
    assert!(json.contains("CastOnly") && json.contains("During") && json.contains("EachTurn"));
    read("Players can cast spells only during their own turns.");
    let (teferi, _) = read("Each opponent can cast spells only any time they could cast a sorcery.");
    let (sorcery, _) = read("Each opponent can cast spells only as a sorcery.");
    assert_eq!(teferi, sorcery);
    for (capped, forbidden) in [
        ("You can cast no more than two spells each turn.", "You can't cast more than two spells each turn."),
        ("Each player can draw no more than two cards each turn.", "Each player can't draw more than two cards each turn."),
        ("You can activate no more than one ability each turn.", "You can't activate more than one ability each turn."),
    ] {
        let (a, _) = read(capped);
        let (b, _) = read(forbidden);
        assert_eq!(a, b);
    }
    let (restricted, _) = read("You can cast noncreature spells only during your turn.");
    let ir::Ability::Text(effect) = restricted else { panic!() };
    let ir::Sentence::Do(body) = &effect.0[0] else { panic!() };
    let ir::Action::Static(rule) = &body.first.first.action else { panic!() };
    let ir::StaticAct::CastOnly { what, restrictions } = &**rule else { panic!() };
    assert_eq!(what.domain, ir::Domain::Spell);
    assert!(!matches!(&what.filter, ir::Filter::All(v) if v.is_empty()));
    assert_eq!(*restrictions, vec![ir::Restriction::During(ir::event::During::Turn(Box::new(ir::Selector::you())))]);
    for text in [
        "Creatures can cast spells only during your turn.",
        "You can cast creatures only during your turn.",
        "Target creature can draw no more than two cards each turn.",
        "You can cast only one more spell this turn.",
    ] { declines(text); }
}

/// CR 702.29a and 608.2i: alternative actions share one participant query and time window.
#[test]
fn r702_29_alternative_object_participant_histories() {
    let (value, _) = read("~ costs {2} less to cast for each card you've cycled or discarded this turn.");
    read("Cycling {2}");
    let ir::Ability::Text(effect) = value else { panic!() };
    let ir::Sentence::Do(body) = &effect.0[0] else { panic!() };
    let [ir::Modifier::ForEach(ir::Amount::Participants { pattern, counted, when })] = body.first.first.mods.as_slice() else { panic!() };
    assert_eq!(*counted, ir::amount::Role::Object);
    assert_eq!(*when, ir::amount::When::ThisTurn);
    assert_eq!(pattern.subjects, vec![ir::Selector::you()]);
    assert_eq!(pattern.actions.len(), 2);
    for (action, verb) in pattern.actions.iter().zip([ir::Verb::Cycle, ir::Verb::Discard]) {
        let ir::EventAction::Deed(deed) = action else { panic!() };
        assert_eq!(deed.verb, verb);
        assert!(!deed.passive);
        assert_eq!(deed.what().unwrap().set.domain, ir::Domain::Card);
    }
    for text in [
        "Draw a card for each card you've drawn or discarded this turn.",
        "Draw a card for each card that player has cycled or discarded this game.",
        "Gain life equal to the number of cards cycled or discarded this turn.",
        "Draw a card for each spell you've cast or copied this turn.",
        "Draw a card for each artifact you've sacrificed or exiled this turn.",
        "Gain life equal to the number of opponents you attacked this turn.",
    ] { read(text); }
    let (past, _) = read("Draw a card for each card you cycled or discarded this turn.");
    let (perfect, _) = read("Draw a card for each card you've cycled or discarded this turn.");
    assert_eq!(past, perfect);
    for text in [
        "Draw a card for each opponent you've cycled or discarded this turn.",
        "Draw a card for each spell you've cycled or discarded this turn.",
        "Draw a card for each card you've cycled and discarded this turn.",
        "Draw a card for each card you've cycled or discarded.",
    ] { declines(text); }
}

/// CR 608.2c and 614.1a: a coordinated replacement stays inside its whole sentence's gate.
#[test]
fn r608_2c_instruction_followed_by_replacement() {
    let (value, _) = read("~ deals X damage to any target. If it's a creature, it can't be regenerated this turn, and if it would die this turn, exile it instead.");
    read("~ deals 3 damage to any target. If it's a creature, it can't be regenerated this turn, and if it would die this turn, exile it instead.");
    read("Kicker {R}");
    read("~ deals 2 damage to any target. If this spell was kicked, that creature can't be regenerated this turn and if it would die this turn, exile it instead.");
    read("When you unlock this door, you may cast spells from your graveyard this turn, and if a card would be put into your graveyard from anywhere this turn, exile it instead.");
    let ir::Ability::Text(effect) = value else { panic!() };
    let ir::Sentence::Led { lead: ir::Lead::Modifier(ir::Modifier::If(condition)), rest } = &effect.0[1] else { panic!() };
    assert!(matches!(condition, ir::Condition::Is { negated: false, .. }));
    let ir::Sentence::Static(rule) = &**rest else { panic!() };
    let ir::Static::After { body, join, rest } = &**rule else { panic!() };
    assert_eq!(*join, ir::Join::CommaAnd);
    assert!(matches!(body.first.first.action, ir::Action::Cant(_)));
    assert_eq!(body.first.first.mods, vec![ir::Modifier::Duration(ir::Duration::ThisTurn)]);
    assert!(matches!(&**rest, ir::Static::Replacement(_)));
    read("Target creature gains trample until end of turn, and the next time it would die this turn, exile it instead.");
    read("If a spell or ability an opponent controls causes you to discard a card, you may reveal that card and put it on top of your library instead of putting it anywhere else.");
    declines("It can't be regenerated this turn, and if it would die this turn.");
    declines("Target player can't be regenerated this turn, and if it would die this turn, exile it instead.");
}


/// CR 608.2i: a source-free passive does not require any player to have caused the event.
#[test]
fn r608_2i_passive_participants_keep_actor_unbound() {
    let rule = &grammar::amount::amounts().for_each;
    let text = "for each creature destroyed or exiled this turn";
    let Outcome::Accepted { value, .. } = rule.parse_all(&tokenize(text)) else { panic!("{text}") };
    let ir::Amount::Participants { pattern, counted, .. } = &value else { panic!() };
    assert_eq!(*counted, ir::amount::Role::Subject);
    assert_eq!(pattern.subjects.len(), 1);
    assert_eq!(pattern.subjects[0].set.domain, ir::Domain::Permanent);
    for action in &pattern.actions {
        let ir::EventAction::Deed(deed) = action else { panic!() };
        assert!(deed.passive && deed.args.is_empty());
    }
    let printed = rule.print_text(&value, false).unwrap();
    let Outcome::Accepted { value: again, .. } = rule.parse_all(&tokenize(&printed)) else { panic!("{printed}") };
    assert_eq!(value, again);
    let Outcome::Accepted { value: active, .. } = rule.parse_all(&tokenize("for each creature you've destroyed or exiled this turn")) else { panic!() };
    assert_ne!(value, active);
    let ir::Amount::Participants { pattern, counted, .. } = active else { panic!() };
    assert_eq!(counted, ir::amount::Role::Object);
    assert_eq!(pattern.subjects, vec![ir::Selector::you()]);
}

/// CR 608.2c: a closed excluded reference cannot take the outer object's zone qualifier.
#[test]
fn r608_2c_closed_exclusions_before_zone_qualifiers() {
    read("At the beginning of your upkeep, if ~ is in your graveyard, you may exile a black creature card other than ~ from your graveyard. If you do, return ~ to the battlefield.");
    let (late, _) = read("Exile a black creature card other than ~ from your graveyard.");
    let (early, _) = read("Exile a black creature card from your graveyard other than ~.");
    assert_eq!(late, early);
    let rule = &grammar::selector::selectors().selector_from;
    let text = "a black creature card other than it from your graveyard";
    let Outcome::Accepted { value, .. } = rule.parse_all(&tokenize(text)) else { panic!("{text}") };
    assert_eq!(value.set.zone.as_ref().unwrap().zone(), Some(ir::Zone::Graveyard));
    let ir::Filter::All(filters) = &value.set.filter else { panic!() };
    let excluded = filters.iter().find_map(|filter| match filter {
        ir::Filter::Not(inner) => match &**inner { ir::Filter::Atom(ir::Atom::Among(s)) => Some(s), _ => None },
        _ => None,
    }).unwrap();
    assert_eq!(excluded.quant, ir::Quant::Definite(ir::Ref::Anaphor(ir::Anaphor::It)));
    assert!(excluded.set.zone.is_none());
    read("Destroy target creature other than ~ you control.");
    // A complete noun can take its own following zone; preserve right association.
    let Outcome::Accepted { value, .. } = rule.parse_all(&tokenize("a creature card other than a creature card in your graveyard")) else { panic!() };
    assert!(value.set.zone.is_none());
    declines("Exile a black creature card other than ~ from.");
    declines("Destroy target player other than ~.");
}

/// CR 201.2c: different from a reference group requires a name and no shared name with any member.
#[test]
fn r201_2c_different_names_and_conjoined_properties() {
    read("Whenever an Aura you control enters, if you cast it, you may search your library for an Aura card with mana value less than or equal to that Aura and with a different name than each Aura you control, put that card onto the battlefield attached to ~, then shuffle.");
    // Tibalt's Trickery has the same predicate; its separate numeric-choice sentence is unread.
    read("They exile cards from the top of their library until they exile a nonland card with a different name than that spell.");
    let (joined, _) = read("Exile target creature with power 2 and with flying.");
    let (plain, _) = read("Exile target creature with power 2 with flying.");
    assert_eq!(joined, plain);
    let (elided, _) = read("Exile an Aura card with mana value less than or equal to that Aura.");
    let (explicit, _) = read("Exile an Aura card with mana value less than or equal to that Aura's mana value.");
    assert_eq!(elided, explicit);
    let (than, _) = read("Exile a card with a different name than that spell.");
    let (from, _) = read("Exile a card with a different name from that spell.");
    assert_eq!(than, from);
    let rule = &grammar::selector::selectors().selector;
    let text = "an Aura card with a different name than each Aura you control";
    let Outcome::Accepted { value, .. } = rule.parse_all(&tokenize(text)) else { panic!("{text}") };
    let ir::Filter::All(filters) = &value.set.filter else { panic!() };
    let source = filters.iter().find_map(|f| match f {
        ir::Filter::Atom(ir::Atom::DifferentNameFrom(source)) => Some(source), _ => None,
    }).unwrap();
    assert_eq!(source.quant, ir::Quant::Each);
    assert_eq!(source.set.controller.as_ref().unwrap().who.as_ref(), &ir::Selector::you());
    assert!(!filters.iter().any(|f| matches!(f, ir::Filter::Not(_))));
    read("Exile a creature card with a different name than it from your graveyard.");
    for text in [
        "Exile a card with a different name than target player.",
        "Exile a creature card and with flying.",
        "Exile a card with a different name than.",
        "Exile a card with mana value less than or equal to each Aura you control.",
        "Exile a card with mana value less than or equal to that player.",
    ] { declines(text); }
}


/// CR 400.1 and 603.2c: plural graveyards allow any owner without changing event batching.
#[test]
fn r603_2c_plural_graveyard_event_destinations() {
    read("Whenever one or more creature cards are put into graveyards from anywhere, investigate. This ability triggers only once each turn.");
    read("Destroy all artifacts, then ~ deals damage to each creature equal to the number of artifacts that were put into graveyards from the battlefield this turn.");
    read("Exile up to two creature cards put into graveyards this way.");
    let (plural, _) = read("Whenever one or more creature cards are put into graveyards from anywhere, draw a card.");
    let (singular, _) = read("Whenever one or more creature cards are put into a graveyard from anywhere, draw a card.");
    assert_eq!(plural, singular);
    let ir::Ability::Triggered { trigger: ir::Trigger::Event { pattern, .. }, .. } = plural else { panic!() };
    assert_eq!(pattern.subjects.len(), 1);
    assert_eq!(pattern.subjects[0].quant, ir::Quant::Indefinite(ir::Count::AtLeast(ir::Amount::Const(1))));
    let [ir::EventAction::PutIntoGraveyard { into, from: None }] = pattern.actions.as_slice() else { panic!() };
    assert_eq!(into.quant, ir::Quant::Indefinite(ir::Count::Exactly(ir::Amount::Const(1))));
    assert_eq!(into.set, ir::ObjectSet::of(ir::Domain::Player));
    let (one, _) = read("Whenever a creature card is put into graveyards from anywhere, draw a card.");
    assert_ne!(singular, one);
    for text in [
        "Whenever one or more creature cards are put into players from anywhere, draw a card.",
        "Whenever one or more creature cards are put into your graveyards from anywhere, draw a card.",
        "Whenever one or more players are put into graveyards from anywhere, draw a card.",
    ] { declines(text); }
}


/// CR 614.1a and 700.4: complete alternative events keep their subject/action correlation.
#[test]
fn r614_1a_correlated_replacement_event_alternatives() {
    let (value, _) = read("If a nontoken creature an opponent owns would die or a creature card not on the battlefield would be put into an opponent's graveyard, exile that card instead.");
    let ir::Ability::Text(effect) = value else { panic!() };
    let [ir::Sentence::Static(rule)] = effect.0.as_slice() else { panic!() };
    let ir::Static::Replacement(replacement) = &**rule else { panic!() };
    assert_eq!(replacement.event.conj, ir::Conj::Or);
    let [battlefield, elsewhere] = replacement.event.items.as_slice() else { panic!() };
    assert!(replacement.single_event().is_none());
    assert_eq!(battlefield.subjects.len(), 1);
    assert_eq!(battlefield.subjects[0].set.domain, ir::Domain::Permanent);
    assert!(battlefield.subjects[0].set.owner.is_some());
    let [ir::EventAction::Deed(deed)] = battlefield.actions.as_slice() else { panic!() };
    assert_eq!(deed.verb, ir::Verb::Die);
    assert!(!deed.passive && deed.args.is_empty());
    assert_eq!(elsewhere.subjects.len(), 1);
    assert_eq!(elsewhere.subjects[0].set.domain, ir::Domain::Card);
    assert!(elsewhere.subjects[0].set.owner.is_none());
    let ir::Filter::All(filters) = &elsewhere.subjects[0].set.filter else { panic!() };
    assert!(filters.iter().any(|f| matches!(f, ir::Filter::Not(inner)
        if matches!(&**inner, ir::Filter::Atom(ir::Atom::InZone(zone)) if zone.zone() == Some(ir::Zone::Battlefield)))));
    let [ir::EventAction::PutIntoGraveyard { into, from: None }] = elsewhere.actions.as_slice() else { panic!() };
    assert_ne!(into.as_ref(), &ir::Selector::you());
    // A shared subject with alternative verbs remains the existing single event pattern.
    let (shared, _) = read("If a creature would die or be exiled, exile it instead.");
    let ir::Ability::Text(effect) = shared else { panic!() };
    let ir::Sentence::Static(rule) = &effect.0[0] else { panic!() };
    let ir::Static::Replacement(replacement) = &**rule else { panic!() };
    assert_eq!(replacement.single_event().unwrap().actions.len(), 2);
    read("The first time you would draw a card each turn, instead draw two cards.");
    let (short, _) = read("Exile a creature card not on the battlefield.");
    let (relative, _) = read("Exile a creature card that isn't on the battlefield.");
    assert_eq!(short, relative);
    for text in [
        "If a creature would die and a card would be exiled, draw a card instead.",
        "The first time a creature would die or a card would be exiled each turn, instead draw a card.",
        "If a creature would die or a player would die, draw a card instead.",
    ] { declines(text); }
}


/// CR 608.2c: a passive stop condition preserves the reveal loop and its counted selection.
#[test]
fn r608_2c_passive_reveal_until() {
    read("Destroy X target artifacts and/or creatures. For each permanent destroyed this way, its controller reveals cards from the top of their library until an artifact or creature card is revealed and exiles that card. Those players put the exiled cards onto the battlefield, then shuffle.");
    read("Target opponent reveals cards from the top of their library until four land cards are revealed. That player puts all cards revealed this way into their graveyard.");
    read("Choose a card name. Target player reveals cards from the top of their library until a card with that name is revealed.");
    let (passive, _) = read("Target opponent reveals cards from the top of their library until a creature card is revealed.");
    let (active, _) = read("Target opponent reveals cards from the top of their library until they reveal a creature card.");
    assert_eq!(passive, active);
    let (passive, _) = read("Reveal cards from the top of your library until a nonland card is revealed.");
    let (active, _) = read("Reveal cards from the top of your library until you reveal a nonland card.");
    assert_eq!(passive, active);
    for text in [
        "Target opponent reveals cards from the top of their library until four land cards is revealed.",
        "Target opponent reveals cards from the top of their library until a creature card are revealed.",
        "Reveal cards from the top of your library until a player is revealed.",
        "Reveal cards from the top of their library until a creature card is revealed.",
        "Target opponent reveals cards from the top of their library until an artifact card or X cards are revealed, whichever comes first.",
    ] { declines(text); }
}

/// CR 401.1 and 601.3: the top of a library is where a cast or play permission finds a spell or
/// a land; a bare card there is the slice "the top card".
#[test]
fn r601_3_spell_from_the_top_of_a_library() {
    for text in [
        "Once each turn, you may cast a spell from the top of your library if it shares a card type with a card exiled with ~.",
        "Once each turn, you may cast a creature spell with power 2 or less from the top of your library.",
        "Once each turn, you may cast an instant or sorcery spell from the top of your library.",
    ] {
        let (value, printed) = read(text);
        assert_eq!(printed, text);
        let json = serde_json::to_string(&value).unwrap();
        assert!(json.contains(r#""zones":{"items":["Library"]"#) && json.contains(r#""slice":{"Top":{"Const":1}}"#), "{json}");
    }
    let (card, _) = read("Exile a card from the top of your library.");
    let (top, _) = read("Exile the top card of your library.");
    assert_eq!(card, top);
    for text in [
        "Once each turn, you may cast a spell from the top of your graveyard.",
        "Destroy a creature from the top of your library.",
        "Exile ~ from the top of your library.",
    ] { declines(text); }
}

/// CR 511.1: the end of combat step, named as a step.
#[test]
fn r511_1_end_of_combat_step() {
    let (value, _) = read("{T}: ~ deals 1 damage to target attacking creature. Activate only during the end of combat step.");
    assert!(serde_json::to_string(&value).unwrap().contains("EndOfCombat"));
    for text in ["Activate only during the end of combat steps.", "Activate only during end of combat step."] { declines(text); }
}

/// CR 105.2: a color of an object of a set, as a choice of one value.
#[test]
fn r105_2_choose_a_color_of_an_object() {
    for text in [
        "{T}: Choose a color of a card in your graveyard. Add one mana of that color.",
        "{T}: Choose a color of a permanent you control. Add one mana of that color.",
    ] {
        let (value, _) = read(text);
        assert!(serde_json::to_string(&value).unwrap().contains("ColorAmong"));
    }
    for text in [
        "{T}: Choose a color of target permanent. Add one mana of that color.",
        "{T}: Choose a color of a player. Add one mana of that color.",
        "{T}: Choose two colors of a card in your graveyard.",
    ] { declines(text); }
}

/// CR 122.1 and 614.1a: a player gets counters; "that many plus one of each of those kinds" says
/// the same replacement of a player as of a permanent.
#[test]
fn r122_1_player_gets_each_kind_of_counter() {
    let (value, printed) = read("If you would get one or more counters, you get that many plus one of each of those kinds of counters instead.");
    assert_eq!(printed, "If you would get one or more counters, you get that many plus one of each of those kinds of counters instead.");
    let json = serde_json::to_string(&value).unwrap();
    assert!(json.contains("GetsCounters") && json.contains("EachProposed"), "{json}");
    for text in [
        "If you would get one or more counters, you gets that many plus one of each of those kinds of counters instead.",
        "If one or more counters would be put on a creature you control, it gets that many plus one of each of those kinds of counters instead.",
    ] { declines(text); }
}

/// CR 725.1 and 725.3: "the monarch" names the one player with that designation, wherever a
/// player is named.
#[test]
fn r725_1_the_monarch_names_a_player() {
    for text in [
        "~ can't be blocked by creatures the monarch controls.",
        "At the beginning of the monarch's end step, that player draws a card.",
        "The monarch controls enchanted creature.",
        "Whenever equipped creature attacks the monarch, destroy target tapped nonland permanent that player controls.",
    ] {
        let (value, printed) = read(text);
        assert_eq!(printed, text);
        assert!(serde_json::to_string(&value).unwrap().contains("Monarch"), "{text}");
    }
    // the designation itself is still a condition and an action, not the player
    for text in ["When ~ enters, you become the monarch.", "At the beginning of your upkeep, if that player is the monarch, draw a card."] {
        let (_, printed) = read(text);
        assert_eq!(printed, text);
    }
    for text in ["~ can't be blocked by creatures the monarchs control.", "Destroy target creature the monarch."] { declines(text); }
}

/// CR 401.4: "on the bottom of that library" puts cards in the library the text spoke of, the
/// place an unsaid library names.
#[test]
fn r401_4_that_library_is_the_library_said_before() {
    let (said, _) = read("Look at the top two cards of target opponent's library. Exile one of them face down and put the other on the bottom of that library.");
    let (unsaid, _) = read("Look at the top two cards of target opponent's library. Exile one of them face down and put the other on the bottom.");
    assert_eq!(said, unsaid);
    let (said, _) = read("Look at the top five cards of target player's library. Put any number of them on the bottom of that library in any order and the rest on top of the library in any order.");
    let (unsaid, _) = read("Look at the top five cards of target player's library. Put any number of them on the bottom in any order and the rest on top in any order.");
    assert_eq!(said, unsaid);
    declines("Look at the top two cards of target opponent's library. Put one of them on the bottom of that libraries.");
}

/// CR 400.1 and 401.1: "that library", "that graveyard" name the zone the text spoke of again;
/// its owner is whoever owns that zone, not the player said last.
#[test]
fn r400_1_that_zone_names_the_zone_said_before() {
    for text in [
        "Whenever an opponent shuffles their library, you may look at the top two cards of that library. You may exile one of those cards. Then put the rest on top in any order.",
        "When ~ enters, for each graveyard with an instant or sorcery card in it, exile target instant or sorcery card from that graveyard.",
        "{1}{B}: Exile target card from an opponent's graveyard. If no cards are in that graveyard, draw a card.",
    ] {
        let (value, printed) = read(text);
        assert_eq!(printed, text);
        assert!(serde_json::to_string(&value).unwrap().contains("ThatZone"), "{text}");
    }
    let (value, printed) = read("If an opponent would search a library, that player searches the top four cards of that library instead.");
    assert_eq!(printed, "If an opponent would search a library, that player searches the top four cards of that library instead.");
    let json = serde_json::to_string(&value).unwrap();
    assert!(json.contains("Searches") && json.contains("ThatZone") && json.contains(r#""Top":{"Const":4}"#), "{json}");
    for text in [
        "Exile target card from that graveyards.",
        "Exile all cards from that hand or graveyard.",
        "If an opponent would search a library, that player searches that library instead.",
        "If an opponent would draw a card, that player searches the top four cards of that library instead.",
    ] { declines(text); }
}

/// CR 701.38: what a vote gave the most votes, and how many votes an option got.
#[test]
fn r701_38_most_votes_and_the_number_of_votes() {
    let (value, printed) = read("Starting with you, each player votes for a nonland permanent you don't control. Exile each permanent with the most votes or tied for most votes.");
    assert!(printed.ends_with("Exile each permanent with the most votes or tied for most votes."), "{printed}");
    assert!(serde_json::to_string(&value).unwrap().contains(r#"{"MostVotes":{"tied":true}}"#));
    let (value, _) = read("You draw cards equal to the number of truth votes.");
    assert!(serde_json::to_string(&value).unwrap().contains(r#"{"Votes":"truth"}"#));
    for text in ["Exile each permanent with the most vote.", "You draw cards equal to the number of red votes."] { declines(text); }
}

/// CR 702.29a and 702.29f: "a cycling ability" is the keyword cycling (or a typecycling) said as
/// an ability, the same with a plural noun ("spells that have a cycling ability"); "and it
/// wasn't cycled" says the zone change is not its discard for a cycling cost (702.29c).
#[test]
fn r702_29_a_cycling_ability_and_a_card_not_cycled() {
    let (said, printed) = read("You may cast spells that have a cycling ability from your graveyard.");
    let (plain, _) = read("You may cast spells with cycling from your graveyard.");
    assert_eq!((said, printed.as_str()), (plain, "You may cast spells with cycling from your graveyard."));
    let text = "If a card that has a cycling ability would be put into your graveyard from anywhere and it wasn't cycled, exile it instead.";
    let (value, printed) = read(text);
    assert_eq!(printed, "If a card with cycling would be put into your graveyard from anywhere and it wasn't cycled, exile it instead.");
    let json = serde_json::to_string(&value).unwrap();
    assert!(json.contains(r#""verb":"Cycle","passive":true"#) && json.contains(r#""negated":true"#), "{json}");
    for text in [
        "When ~ dies, if it wasn't cycled, draw a card.",
        "If a card would be put into your graveyard from anywhere and it wasn't cycle, exile it instead.",
    ] { declines(text); }
}

/// CR 614.1a: "you may instead create …" is the optional replacement "instead you may create …"
/// says, "instead" after the subject.
#[test]
fn r614_1a_you_may_instead_is_instead_you_may() {
    let (said, printed) = read("If you would draw a card, you may instead put the top card of your library into your graveyard.");
    let (other, _) = read("If you would draw a card, instead you may put the top card of your library into your graveyard.");
    assert_eq!(said, other);
    assert_eq!(printed, "If you would draw a card, instead you may put the top card of your library into your graveyard.");
    for text in [
        "If you would draw a card, you may instead you put the top card of your library into your graveyard.",
        "Draw a card, you may instead discard a card.",
    ] { declines(text); }
}

/// CR 401.1: "the top three cards" after the text spoke of a library are that library's ("exile
/// the top three cards instead").
#[test]
fn r401_1_the_top_cards_of_the_library_said_before() {
    let (unsaid, printed) = read("Exile the top two cards of your library. If ~'s additional cost was paid, exile the top three cards instead.");
    let (said, _) = read("Exile the top two cards of your library. If ~'s additional cost was paid, exile the top three cards of that library instead.");
    assert_eq!(unsaid, said);
    assert!(printed.ends_with("exile the top three cards of that library instead."), "{printed}");
}

/// CR 119.3: "you gained and lost life this turn" is both events, in the same turn.
#[test]
fn r119_3_gained_and_lost_life_this_turn() {
    let (both, printed) = read("At the beginning of your end step, if you gained and lost life this turn, create a 1/1 black Bat creature token with flying.");
    let (each, _) = read("At the beginning of your end step, if you gained life this turn and you lost life this turn, create a 1/1 black Bat creature token with flying.");
    assert_eq!(both, each);
    assert!(printed.contains("if you gained life this turn and you lost life this turn"), "{printed}");
    declines("At the beginning of your end step, if you gained and lost life, create a 1/1 black Bat creature token with flying.");
}

/// CR 603.7b: "When you cast your next instant or sorcery spell this turn" is the delayed trigger
/// "when you next cast an instant or sorcery spell this turn" says.
#[test]
fn r603_7b_your_next_spell_is_the_next_one_you_cast() {
    let (said, printed) = read("When you cast your next instant or sorcery spell this turn, copy that spell.");
    let (other, _) = read("When you next cast an instant or sorcery spell this turn, copy that spell.");
    assert_eq!(said, other);
    assert_eq!(printed, "When you next cast an instant or sorcery spell this turn, copy that spell.");
}

/// CR 715.3: a card that has an Adventure may be cast as that Adventure; "as an Adventure" says
/// how a cast is made, never how anything else is done.
#[test]
fn r715_3_cast_as_an_adventure() {
    let (adventure, printed) = read("When ~ dies, you may cast it from your graveyard as an Adventure until the end of your next turn.");
    let (plain, _) = read("When ~ dies, you may cast it from your graveyard until the end of your next turn.");
    assert_ne!(adventure, plain);
    assert_eq!(printed, "When ~ dies, you may cast it from your graveyard as an Adventure until the end of your next turn.");
    for text in ["When ~ dies, exile it as an Adventure.", "When ~ dies, return it to your hand as an Adventure."] { declines(text); }
}

/// CR 702.8a: "and as though it had flash" goes with a cast made without paying the mana cost
/// (Aluren ruling of 2004-10-04: the cast is free and at instant speed together, or neither).
#[test]
fn r702_8a_a_free_cast_as_though_it_had_flash() {
    let (_, printed) = read("Any player may cast creature spells with mana value 3 or less without paying their mana costs and as though they had flash.");
    assert_eq!(printed, "Any player may cast creature spells with mana value 3 or less without paying their mana costs and as though they had flash.");
    let (_, printed) = read("If a creature is attacking you and you control a Forest and a Plains, you may cast ~ without paying its mana cost and as though it had flash.");
    assert!(printed.ends_with("you may cast ~ without paying its mana cost and as though it had flash."), "{printed}");
    for text in ["You may cast ~ and as though it had flash.", "Draw a card and as though it had flash."] { declines(text); }
}

/// CR 608.2c: ", or" between two long instructions is the choice "or" offers.
#[test]
fn r608_2c_a_comma_before_or_between_instructions() {
    let (comma, printed) = read("Tap all untapped permanents of the chosen type target player controls, or untap all tapped permanents of that type that player controls.");
    let (bare, _) = read("Tap all untapped permanents of the chosen type target player controls or untap all tapped permanents of that type that player controls.");
    assert_eq!(comma, bare);
    assert!(printed.contains(" or untap all tapped permanents"), "{printed}");
}

/// CR 614.1a: "instead create one of each" of the kinds of token the event names is that many of
/// each (Academy Manufactor rulings of 2021-06-18).
#[test]
fn r614_1a_one_of_each_kind_of_token_instead() {
    let (each, printed) = read("If you would create a Clue, Food, or Treasure token, instead create one of each.");
    let (said, _) = read("If you would create one or more Clue, Food, or Treasure tokens, instead create that many Clue tokens, that many Food tokens, and that many Treasure tokens.");
    assert_eq!(each, said);
    assert_eq!(printed, "If you would create one or more Clue, Food, and/or Treasure tokens, instead create that many Clue tokens, that many Food tokens, and that many Treasure tokens.");
    for text in ["If you would create a Clue token, instead create one of each.", "If you would draw a card, instead create one of each."] { declines(text); }
}

/// CR 115.1: "any target of an opponent's choice" is a target an opponent chooses, as "target
/// creature of an opponent's choice" is (Cuombajj Witches ruling of 2020-11-10).
#[test]
fn r115_1_any_target_of_an_opponents_choice() {
    let (witches, printed) = read("{T}: ~ deals 1 damage to any target and 1 damage to any target of an opponent's choice.");
    assert_eq!(printed, "{T}: ~ deals 1 damage to any target and 1 damage to any target of an opponent's choice.");
    let (plain, _) = read("{T}: ~ deals 1 damage to any target and 1 damage to any other target.");
    assert_ne!(witches, plain);
    declines("{T}: ~ deals 1 damage to any target of an opponent's choosing.");
}

/// CR 601.2a: "from anywhere other than their hands" is each one's own hand, as "their hand"
/// says it of each of them (Drannith Magistrate).
#[test]
fn r601_2a_their_hands_is_each_ones_hand() {
    let (hands, printed) = read("Your opponents can't cast spells from anywhere other than their hands.");
    let (hand, _) = read("Your opponents can't cast spells from anywhere other than their hand.");
    assert_eq!(hands, hand);
    assert_eq!(printed, "Your opponents can't cast spells from anywhere other than their hand.");
    declines("Your opponents can't cast spells from anywhere other than their exiles.");
}

/// CR 400.1 and 406.1: "from graveyards or exile" is from any graveyard or from exile, as
/// "from graveyards or from exile" says it too (Soulless Jailer, Aven Interrupter).
#[test]
fn r400_1_from_graveyards_or_exile() {
    let (exile, printed) = read("Players can't cast noncreature spells from graveyards or exile.");
    let (from_exile, _) = read("Players can't cast noncreature spells from graveyards or from exile.");
    assert_eq!(exile, from_exile);
    assert_eq!(printed, "Players can't cast noncreature spells from graveyards or exile.");
    let (graveyards, _) = read("Players can't cast noncreature spells from graveyards.");
    assert_ne!(exile, graveyards);
}

/// CR 601.2a and 400.1: "spells cast from graveyards or from exile" are cast from any
/// graveyard or from exile (Aven Interrupter).
#[test]
fn r601_2a_cast_from_graveyards_or_from_exile() {
    let text = "Spells your opponents cast from graveyards or from exile cost {2} more to cast.";
    let (both, printed) = read(text);
    assert_eq!(printed, text);
    let (short, _) = read("Spells your opponents cast from graveyards or exile cost {2} more to cast.");
    assert_eq!(both, short);
    let (exile, _) = read("Spells your opponents cast from exile cost {2} more to cast.");
    assert_ne!(both, exile);
}

/// CR 106.12 and 106.12b: "is tapped for two or more mana" says how much mana the tapping
/// produces, which a replacement of that mana production may ask (Damping Sphere).
#[test]
fn r106_12_tapped_for_two_or_more_mana() {
    let text = "If a land is tapped for two or more mana, it produces {C} instead of any other type and amount.";
    let (two, printed) = read(text);
    assert_eq!(printed, text);
    let (any, _) = read("If a land is tapped for mana, it produces {C} instead of any other type and amount.");
    assert_ne!(two, any);
    declines("If a land is tapped for two or more life, it produces {C} instead of any other type and amount.");
}

/// CR 101.2: "Each player who has cast a nonartifact spell this turn can't cast additional
/// nonartifact spells" is "can't cast more than one nonartifact spell each turn", counting the
/// spells cast before the ability applied (Ethersworn Canonist rulings of 2020-08-07).
#[test]
fn r101_2_who_has_cast_a_spell_cant_cast_additional_ones() {
    let (canonist, printed) = read("Each player who has cast a nonartifact spell this turn can't cast additional nonartifact spells.");
    let (cap, _) = read("Each player can't cast more than one nonartifact spell each turn.");
    assert_eq!(canonist, cap);
    assert_eq!(printed, "Each player can't cast more than one nonartifact spell each turn.");
    declines("Each player who has cast a nonartifact spell this turn can't cast additional creature spells.");
}

/// CR 102.1 and 601.2f: "except during its controller's turn" exempts a spell while its
/// controller is the active player, so it is the spells the active player doesn't control
/// (Defense Grid).
#[test]
fn r102_1_except_during_its_controllers_turn() {
    let (grid, printed) = read("Each spell costs {3} more to cast except during its controller's turn.");
    assert_eq!(printed, "Each spell the active player doesn't control costs {3} more to cast.");
    let (all, _) = read("Each spell costs {3} more to cast.");
    assert_ne!(grid, all);
}

/// CR 500.7 and 101.2: "during your first, second, or third turns of the game" is while it's
/// one of those turns of yours (Serra Avenger, Jace Reawakened, Spider-Man 2099; extra turns
/// count, rulings of 2012-07-01).
#[test]
fn r500_7_during_your_first_second_or_third_turns() {
    let (during, printed) = read("You can't cast ~ during your first, second, or third turns of the game.");
    assert_eq!(printed, "You can't cast ~ as long as it's your first, second, or third turn of the game.");
    let (as_long_as, _) = read("You can't cast ~ as long as it's your first, second, or third turn of the game.");
    assert_eq!(during, as_long_as);
    let (your_turn, _) = read("You can't cast ~ during your turn.");
    assert_ne!(during, your_turn);
}

/// CR 603.12 and 700.2: a payment made up to N times, then a reflexive trigger choosing up to
/// as many modes as it was paid (Tranquil Frillback).
#[test]
fn r603_12_pay_up_to_three_times_choose_up_to_that_many() {
    let text = "When ~ enters, you may pay {G} up to three times. When you pay this cost one or more times, choose up to that many —";
    let (up_to, printed) = read(text);
    assert_eq!(printed, text);
    let (any, _) = read("When ~ enters, you may pay {G} any number of times. When you pay this cost one or more times, choose up to that many —");
    assert_ne!(up_to, any);
    let (two, _) = read("When ~ enters, you may pay {G} up to three times. When you pay this cost one or more times, choose up to two —");
    assert_ne!(up_to, two);
}

/// CR 702.33b and 702.33d: a spell with two kicker costs is kicked once for each one paid;
/// "if it was kicked twice" asks for both (Archangel of Wrath rulings of 2022-09-09).
#[test]
fn r702_33d_kicked_twice() {
    let (twice, printed) = read("When ~ enters, if it was kicked twice, it deals 2 damage to any target.");
    assert_eq!(printed, "When ~ enters, if there are two or more times it was kicked, it deals 2 damage to any target.");
    let (once, _) = read("When ~ enters, if it was kicked, it deals 2 damage to any target.");
    assert_ne!(twice, once);
    assert!(serde_json::to_string(&twice).unwrap().contains("Kicked"));
}

/// CR 301.5 and 303.4: "permanents attached to creatures" are those attached to any creature,
/// as "attached to a creature" says (End Hostilities, Auras attached to permanents you control).
#[test]
fn r303_4_attached_to_creatures_is_attached_to_a_creature() {
    let (plural, printed) = read("Destroy all creatures and all permanents attached to creatures.");
    let (one, _) = read("Destroy all creatures and all permanents attached to a creature.");
    assert_eq!(plural, one);
    assert_eq!(printed, "Destroy all creatures and all permanents attached to a creature.");
    let (yours, _) = read("Auras attached to permanents you control have umbra armor.");
    let (your_one, _) = read("Auras attached to a permanent you control have umbra armor.");
    assert_eq!(yours, your_one);
}

/// CR 400.1 and 102.3: "all opponents' hands and graveyards" are your opponents' hands and
/// graveyards, every one of them (Ashiok, Nightmare Weaver).
#[test]
fn r400_1_all_opponents_hands_and_graveyards() {
    let (all, printed) = read("−10: Exile all cards from all opponents' hands and graveyards.");
    let (yours, _) = read("−10: Exile all cards from your opponents' hands and graveyards.");
    assert_eq!(all, yours);
    assert_eq!(printed, "−10: Exile all cards from your opponents' hands and graveyards.");
    let (graveyards, _) = read("−10: Exile all cards from your opponents' graveyards.");
    assert_ne!(all, graveyards);
}

/// CR 404.1 and 401.1: a graveyard keeps its order, so it has a top and a bottom card as a
/// library has (Phyrexian Furnace: "Exile the bottom card of target player's graveyard.").
#[test]
fn r404_1_bottom_card_of_a_graveyard() {
    let text = "{T}: Exile the bottom card of target player's graveyard.";
    let (bottom, printed) = read(text);
    assert_eq!(printed, text);
    let (top, printed) = read("{T}: Exile the top card of target player's graveyard.");
    assert_eq!(printed, "{T}: Exile the top card of target player's graveyard.");
    assert_ne!(bottom, top);
    let (library, _) = read("{T}: Exile the bottom card of target player's library.");
    assert_ne!(bottom, library);
}

/// CR 102.3: "that player's opponents" are the opponents of the player the text named, not of
/// the ability's controller (Standstill).
#[test]
fn r102_3_that_players_opponents() {
    let text = "When a player casts a spell, sacrifice ~. If you do, each of that player's opponents draws three cards.";
    let (theirs, printed) = read(text);
    let (yours, _) = read("When a player casts a spell, sacrifice ~. If you do, each of your opponents draws three cards.");
    assert_ne!(theirs, yours);
    assert!(serde_json::to_string(&theirs).unwrap().contains("OpponentOf"), "{printed}");
}

/// CR 605.1a and 602.1: "cost {2} more to activate unless they're mana abilities" changes the
/// cost of the abilities that aren't mana abilities only (Anointed Peacekeeper).
#[test]
fn r605_1a_cost_more_unless_mana_abilities() {
    let text = "Activated abilities of sources with the chosen name cost {2} more to activate unless they're mana abilities.";
    let (unless, printed) = read(text);
    assert_eq!(printed, text);
    let (all, _) = read("Activated abilities of sources with the chosen name cost {2} more to activate.");
    assert_ne!(unless, all);
}

/// CR 708.2 and 110.5: after a verb the face-down status is written "face down" (Unable to
/// Scream: "As long as enchanted creature is face down, it can't be turned face up.").
#[test]
fn r708_2_is_face_down() {
    let text = "As long as enchanted creature is face down, it can't be turned face up.";
    let (down, printed) = read(text);
    assert_eq!(printed, text);
    let (up, _) = read("As long as enchanted creature is face up, it can't be turned face up.");
    assert_ne!(down, up);
    assert!(serde_json::to_string(&down).unwrap().contains("FaceDown"));
}

/// CR 702.16a: protection from a quality the permanent's choice names: "the chosen card name"
/// (Runed Halo), "the chosen card type" (Serra's Emissary).
#[test]
fn r702_16a_protection_from_the_chosen_card_name_or_type() {
    for text in ["You have protection from the chosen card name.", "You and creatures you control have protection from the chosen card type."] {
        let (_, printed) = read(text);
        assert_eq!(printed, text);
    }
    let (name, _) = read("You have protection from the chosen card name.");
    let (opponents, _) = read("You have protection from each of your opponents.");
    assert_ne!(name, opponents);
}

/// CR 109.1 and 110.1: "all cards that aren't on the battlefield, spells, and permanents" is
/// every object of those three kinds, the arms joined as one union (Mycosynth Lattice,
/// Painter's Servant).
#[test]
fn r109_1_cards_not_on_the_battlefield_spells_and_permanents() {
    let (and, _) = read("All cards that aren't on the battlefield, spells, and permanents are colorless.");
    let (or, _) = read("All cards that aren't on the battlefield, spells, or permanents are colorless.");
    assert_eq!(and, or);
    let (two, _) = read("All spells and permanents are colorless.");
    assert_ne!(and, two);
    read("All cards that aren't on the battlefield, spells, and permanents are the chosen color in addition to their other colors.");
}

/// CR 508.1c: "No more than one creature can attack ~ each combat" limits the attackers of that
/// planeswalker as "… can attack you each combat" does a player's (The Eternal Wanderer,
/// Crawlspace).
#[test]
fn r508_1c_no_more_than_one_creature_can_attack_it() {
    for text in ["No more than one creature can attack ~ each combat.", "No more than two creatures can attack you each combat."] {
        let (_, printed) = read(text);
        assert_eq!(printed, text);
    }
    let (it, _) = read("No more than one creature can attack ~ each combat.");
    let (any, _) = read("No more than one creature can attack each combat.");
    assert_ne!(it, any);
}

/// CR 601.2h and 602.2b: "can't pay life or sacrifice nonland permanents to cast spells or
/// activate abilities" forbids those acts as costs (Yasharn, Angel of Jubilation), not the acts
/// an effect has a player do (Yasharn rulings of 2020-09-25).
#[test]
fn r601_2h_cant_pay_life_or_sacrifice_to_cast_or_activate() {
    let text = "Players can't pay life or sacrifice nonland permanents to cast spells or activate abilities.";
    let (costs, printed) = read(text);
    assert_eq!(printed, text);
    assert!(serde_json::to_string(&costs).unwrap().contains("CantPayCosts"), "{printed}");
    let (creatures, _) = read("Players can't pay life or sacrifice creatures to cast spells or activate abilities.");
    assert_ne!(costs, creatures);
    let (acts, _) = read("Players can't pay life or sacrifice nonland permanents.");
    assert_ne!(costs, acts);
}

/// CR 614.1a and 122.6: "that many -1/-1 counters minus one are put on it instead" puts one
/// fewer (Vizier of Remedies), as "that many plus one" puts one more (Hardened Scales).
#[test]
fn r614_1a_that_many_counters_minus_one() {
    let text = "If one or more -1/-1 counters would be put on a creature you control, that many -1/-1 counters minus one are put on it instead.";
    let (minus, printed) = read(text);
    assert_eq!(printed, text);
    let (plus, _) = read("If one or more -1/-1 counters would be put on a creature you control, that many plus one -1/-1 counters are put on it instead.");
    assert_ne!(minus, plus);
}

/// CR 107.1 and 122.1: "fewer than three +1/+1 counters on it" is "two or fewer" (Runaway
/// Steam-Kin).
#[test]
fn r122_1_fewer_than_three_counters_on_it() {
    let (fewer, printed) = read("Whenever you cast a red spell, if ~ has fewer than three +1/+1 counters on it, put a +1/+1 counter on ~.");
    assert_eq!(printed, "Whenever you cast a red spell, if ~ has two or fewer +1/+1 counters on it, put a +1/+1 counter on ~.");
    let (two, _) = read("Whenever you cast a red spell, if ~ has two or fewer +1/+1 counters on it, put a +1/+1 counter on ~.");
    assert_eq!(fewer, two);
}

/// CR 608.2c: "Then repeat this process for an enchantment and a planeswalker" runs the
/// instructions again for each of those kinds in turn (Invoke Despair); "for artifacts and
/// creatures" does the same with plural kinds (Equipoise).
#[test]
fn r608_2c_repeat_this_process_for_other_kinds() {
    let text = "Target opponent sacrifices a creature of their choice. If they can't, they lose 2 life and you draw a card. Then repeat this process for an enchantment and a planeswalker.";
    let (others, printed) = read(text);
    assert_eq!(printed, text);
    let (again, _) = read("Target opponent sacrifices a creature of their choice. If they can't, they lose 2 life and you draw a card. Then repeat this process.");
    assert_ne!(others, again);
    let (artifacts, printed) = read("Draw a card. Repeat this process for artifacts and creatures.");
    assert_eq!(printed, "Draw a card. Repeat this process for artifacts and creatures.");
    let (one, _) = read("Draw a card. Repeat this process for an artifact.");
    assert_ne!(artifacts, one);
}

/// CR 101.2: "can't cause you to discard cards or sacrifice permanents" forbids each of the
/// events (Tamiyo, Collector of Tales), as "… to sacrifice permanents" forbids one (Tajuru
/// Preserver).
#[test]
fn r101_2_cant_cause_you_to_discard_or_sacrifice() {
    let text = "Spells or abilities your opponents control can't cause you to discard cards or sacrifice permanents.";
    let (both, printed) = read(text);
    assert_eq!(printed, text);
    let (sacrifice, _) = read("Spells or abilities your opponents control can't cause you to sacrifice permanents.");
    assert_ne!(both, sacrifice);
}

/// CR 608.2d: "choose a land of each basic land type" chooses one land for each basic land type
/// (Sundering Titan); "chooses a permanent they control of each permanent type" one permanent
/// for each permanent type (Liliana, Dreadhorde General).
#[test]
fn r608_2d_choose_one_of_each_type() {
    for text in [
        "When ~ enters or leaves the battlefield, choose a land of each basic land type, then destroy those lands.",
        "Each opponent chooses a permanent they control of each permanent type and sacrifices the rest.",
    ] {
        let (_, printed) = read(text);
        assert_eq!(printed, text);
    }
    let (each, _) = read("Choose a land of each basic land type.");
    let (one, _) = read("Choose a land.");
    assert_ne!(each, one);
}

/// CR 608.2c: the controller follows the instructions, so an instruction after ", then" that
/// follows one object's clause is the controller's ("~ gets +1/+0 until end of turn, then scry
/// 1", Burning Prophet), as "then you scry 1" says it. After a player's clause the same words
/// are that player's acts.
#[test]
fn r608_2c_instruction_after_an_objects_clause() {
    for text in [
        "Whenever you cast a noncreature spell, ~ gets +1/+0 until end of turn, then scry 1.",
        "When ~ enters, it deals damage equal to its power to its controller, then destroy that creature.",
    ] {
        let (_, printed) = read(text);
        assert_eq!(printed, text);
    }
    let (then, _) = read("Whenever you cast a noncreature spell, ~ gets +1/+0 until end of turn, then scry 1.");
    let (you, _) = read("Whenever you cast a noncreature spell, ~ gets +1/+0 until end of turn, then you scry 1.");
    assert_ne!(then, you);
    declines("Whenever you cast a noncreature spell, ~ gets +1/+0 until end of turn and scry 1.");
    declines("Whenever you cast a noncreature spell, creatures you control get +1/+0 until end of turn, then scry 1.");
}

/// CR 702.16a: a protection quality can be any characteristic value, so "colorless" is one
/// (105.2c), and "protection from colorless or from the color of your choice" is a choice of
/// one of those qualities, as "from artifacts or from the color of your choice" is (Giver of
/// Runes, Angelic Intervention).
#[test]
fn r702_16a_protection_from_colorless() {
    let text = "{T}: Another target creature you control gains protection from colorless or from the color of your choice until end of turn.";
    let (colorless, printed) = read(text);
    assert_eq!(printed, text);
    let (artifacts, _) = read("{T}: Another target creature you control gains protection from artifacts or from the color of your choice until end of turn.");
    assert_ne!(colorless, artifacts);
}

/// CR 608.2d: "divided as its controller chooses among any number of those Wolves" divides
/// the damage as the effect resolves, by the source's controller, not by the ability's
/// (Master of the Wild Hunt).
#[test]
fn r608_2d_damage_divided_as_its_controller_chooses() {
    let text = "{T}: Tap all untapped Wolf creatures you control. Each Wolf tapped this way deals damage equal to its power to target creature. That creature deals damage equal to its power divided as its controller chooses among any number of those Wolves.";
    let (theirs, printed) = read(text);
    assert_eq!(printed, text);
    let (yours, _) = read("{T}: Tap all untapped Wolf creatures you control. Each Wolf tapped this way deals damage equal to its power to target creature. That creature deals damage equal to its power divided as you choose among any number of those Wolves.");
    assert_ne!(theirs, yours);
}

/// CR 115.4: "any target" may be a creature, player, planeswalker or battle, and "that
/// was dealt damage this turn" is said of any of them alike (Needle Drop), as "that isn't a
/// Dragon" is.
#[test]
fn r115_4_any_target_that_was_dealt_damage() {
    let text = "~ deals 1 damage to any target that was dealt damage this turn.";
    let (dealt, printed) = read(text);
    assert_eq!(printed, text);
    let (any, _) = read("~ deals 1 damage to any target.");
    assert_ne!(dealt, any);
    let (combat, _) = read("~ deals 1 damage to any target that was dealt combat damage this turn.");
    assert_ne!(dealt, combat);
    declines("~ deals 1 damage to any target who lost life this turn.");
}

/// CR 608.2c: "instead only you draw a card" replaces what the sentence before does, as
/// "instead you draw a card" does; "only" adds nothing to "instead" (Kumena's Awakening,
/// Golden Demise).
#[test]
fn r608_2c_instead_only() {
    let (only, printed) = read("At the beginning of your upkeep, each player draws a card. If you have the city's blessing, instead only you draw a card.");
    assert_eq!(printed, "At the beginning of your upkeep, each player draws a card. If you have the city's blessing, instead you draw a card.");
    let (plain, _) = read(&printed);
    assert_eq!(only, plain);
    read("All creatures get -2/-2 until end of turn. If you have the city's blessing, instead only creatures your opponents control get -2/-2 until end of turn.");
}
