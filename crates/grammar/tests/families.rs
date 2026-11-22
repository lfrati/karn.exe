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
