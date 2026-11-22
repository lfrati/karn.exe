//! Previously admitted compositions must remain executable through compiled programs.
use crate::support::cite;
use game::{CardDef, Game, Move, Zone, Target};
use game::state::{Answer, Choice, Decision};
fn pass(g: &mut Game) { for p in 0..2 { g.act(p, Move::Pass).unwrap(); } }
fn creature() -> CardDef { CardDef::new("Bear", "{2}{G}", "Creature — Bear", "", Some((2,2))).unwrap() }

#[test]
fn r107_3c_where_x_on_the_first_clause_action_is_kept_through_scry_then_draw() {
    let spell = CardDef::new("Ugin's Insight", "{0}", "Sorcery", "Scry X, where X is the greatest mana value among permanents you control, then draw three cards.", None).unwrap();
    game::play::check(&spell).unwrap();
    let mut g=Game::new(vec![spell, creature()], 2, 20);
    g.put(0,1,Zone::Battlefield);
    for _ in 0..5 { g.put(0,1,Zone::Library); }
    let spell=g.put(0,0,Zone::Hand);
    g.act(0,Move::Cast {card:spell,targets:vec![]}).unwrap(); pass(&mut g);
    let cards=match g.decision.clone().unwrap() {
        Decision::Choose { player:0, choice:Choice::Arrange { cards, .. } } => cards,
        other=>panic!("{other:?}"),
    };
    cite!(["107.3c"]; assert_eq!(cards.len(),3));
    g.act(0,Move::Choose(Answer::Arrange { top:cards, away:vec![] })).unwrap();
    assert_eq!(g.players[0].hand.len(),3);
    assert_eq!(g.players[0].library.len(),2);
}

#[test]
fn r113_7a_sacrificed_aura_retains_the_enchanted_object_for_exile() {
    let aura=CardDef::new("Choking Restraints", "{0}", "Enchantment — Aura", "Enchant creature\nEnchanted creature can't attack or block.\n{0}, Sacrifice this Aura: Exile enchanted creature.", None).unwrap();
    game::play::check(&aura).unwrap();
    let mut g=Game::new(vec![aura,creature()],2,20);
    let bear=g.put(1,1,Zone::Battlefield);
    let aura=g.put(0,0,Zone::Hand);
    g.act(0,Move::Cast {card:aura,targets:vec![vec![Target::Object(bear)]]}).unwrap();pass(&mut g);
    let aura=*g.battlefield.iter().find(|o|g.obj(**o).chars.name=="Choking Restraints").unwrap();
    g.act(0,Move::Activate {source:aura,ability:2,targets:vec![],paying:vec![]}).unwrap(); pass(&mut g);
    assert_eq!(g.players[0].graveyard.len(),1);
    assert_eq!(g.exile.len(),1);
    cite!(["113.7a"]; assert_eq!(g.obj(g.exile[0]).chars.name,"Bear"));
}

#[test]
fn r118_12_optional_draw_outcome_is_consent_and_resumes_at_the_discard() {
    let spell=CardDef::new("Loot", "{0}", "Sorcery", "You may draw two cards. If you do, discard a card.",None).unwrap();
    game::play::check(&spell).unwrap();
    let mut g=Game::new(vec![spell,creature()],2,20);
    for _ in 0..3 {g.put(0,1,Zone::Library);}
    let c=g.put(0,0,Zone::Hand);
    g.act(0,Move::Cast{card:c,targets:vec![]}).unwrap();pass(&mut g);
    let mut declined=g.clone();
    declined.act(0,Move::Choose(Answer::No)).unwrap();
    assert_eq!(declined.players[0].library.len(),3);
    cite!(["118.12"]; assert!(declined.players[0].hand.is_empty()));
    g.act(0,Move::Choose(Answer::Yes)).unwrap();
    assert_eq!(g.players[0].hand.len(),2);
    assert!(matches!(g.decision,Some(Decision::Choose{choice:Choice::Objects{..},..})));
    let mut copy=g.clone();
    for g in [&mut g,&mut copy] {
        let chosen=g.players[0].hand[0];
        g.act(0,Move::Choose(Answer::Objects(vec![chosen]))).unwrap();
        cite!(["118.12"]; assert_eq!(g.players[0].hand.len(),1));
        assert_eq!(g.players[0].library.len(),1);
        assert!(g.resolving().is_none());
    }
}

#[test]
fn r118_12_a_skipped_conditional_payment_does_not_pay_or_supply_a_stale_outcome() {
    for bears in [0,1] {
        let d=CardDef::new("Conditional loot", "{0}", "Sorcery", "If you control a creature, you may discard a card. If you do, draw a card.",None).unwrap();
        game::play::check(&d).unwrap();
        let mut g=Game::new(vec![d,creature()],2,20);
        for _ in 0..bears {g.put(0,1,Zone::Battlefield);}
        let c=g.put(0,0,Zone::Hand);
        g.put(0,1,Zone::Hand); g.put(0,1,Zone::Library);
        g.act(0,Move::Cast{card:c,targets:vec![]}).unwrap(); pass(&mut g);
        if bears==1 {g.act(0,Move::Choose(Answer::Yes)).unwrap();}
        cite!(["118.12"]; assert_eq!(g.players[0].library.len(),1-bears));
        assert_eq!(g.players[0].hand.len(),1);
        assert!(g.resolving().is_none());
    }
}

#[test]
fn r118_12_optional_counter_payment_retains_its_outcome_across_replacement_wait() {
    use game::operation::ReplacementId;
    let defs=vec![
        CardDef::new("Counter payment", "{0}", "Sorcery", "You may put a +1/+1 counter on target creature. If you do, you gain 3 life.",None).unwrap(),
        creature(),
        CardDef::new("Hardened Scales", "{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.",None).unwrap(),
        CardDef::new("Branching Evolution", "{2}{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.",None).unwrap(),
    ];
    for d in &defs {game::play::check(d).unwrap();}
    let mut g=Game::new(defs,2,20);
    let bear=g.put(0,1,Zone::Battlefield);
    let scales=g.put(0,2,Zone::Battlefield);
    let evolution=g.put(0,3,Zone::Battlefield);
    let card=g.put(0,0,Zone::Hand);
    g.act(0,Move::Cast{card,targets:vec![vec![Target::Object(bear)]]}).unwrap(); pass(&mut g);
    let mut declined=g.clone(); declined.act(0,Move::Choose(Answer::No)).unwrap();
    assert!(declined.obj(bear).counters.is_empty()); cite!(["118.12"]; assert_eq!(declined.players[0].life,20));
    g.act(0,Move::Choose(Answer::Yes)).unwrap();
    assert!(matches!(g.decision,Some(Decision::Choose{choice:Choice::Replacement{..},..})));
    assert_eq!(g.players[0].life,20);
    let mut copy=g.clone();
    for (game,first,n) in [(&mut g,scales,4),(&mut copy,evolution,3)] {
        game.act(0,Move::Choose(Answer::Replacement(ReplacementId::from_ability(first, game::binding::AbilityKey::printed(0), 0)))).unwrap();
        cite!(["616.1f"]; assert_eq!(game.obj(bear).counters["+1/+1"],n));
        cite!(["118.12"]; assert_eq!(game.players[0].life,23));
        assert!(game.resolving().is_none());
    }
}

#[test]
fn r601_2c_counted_targets_are_announced_before_sacrificing_the_quantity_source() {
    let d=CardDef::new("Recantation", "{0}", "Enchantment", "Sacrifice this enchantment: Return up to X target permanents to their owners' hands, where X is the number of verse counters on this enchantment.",None).unwrap();
    game::play::check(&d).unwrap();
    let mut g=Game::new(vec![d,creature()],2,20);
    let source=g.put(0,0,Zone::Battlefield);
    g.put_counters(source, "verse", 2);
    let bears:Vec<_>=(0..3).map(|_|g.put(0,1,Zone::Battlefield)).collect();
    let activate=|targets|Move::Activate{source,ability:0,targets:vec![targets],paying:vec![]};
    cite!(["601.2c"]; refused("wrong number of targets: 3 chosen, it takes 0 to 2 (601.2c)"): g.act(0,activate(bears.iter().copied().map(Target::Object).collect())));
    assert_eq!(g.obj(source).zone,Zone::Battlefield);
    cite!(["602.2b", "601.2c"]; ok: g.act(0,activate(bears[..2].iter().copied().map(Target::Object).collect())));
    assert_eq!(g.players[0].graveyard.len(),1);
    pass(&mut g);
    assert_eq!(g.players[0].hand.len(),2);
    assert_eq!(g.battlefield,vec![bears[2]]);
}

#[test]
fn r608_2e_optional_player_actions_collect_consent_before_performing_the_action() {
    for text in ["Each player may draw a card.","Each player may scry 1."] {
        let d=CardDef::new("Shared option", "{0}", "Sorcery",text,None).unwrap();
        game::play::check(&d).unwrap();
        let mut g=Game::new(vec![d,creature()],3,20);
        for p in 0..3 {g.put(p,1,Zone::Library);}
        let c=g.put(0,0,Zone::Hand);
        g.act(0,Move::Cast{card:c,targets:vec![]}).unwrap();
        for p in 0..3 {g.act(p,Move::Pass).unwrap();}
        for p in 0..3 {
            cite!(["101.4"]; assert_eq!(g.decision,Some(Decision::Choose{player:p,choice:Choice::May { possible: true }})));
            cite!(["101.4"]; assert!(g.players.iter().all(|p|p.hand.is_empty())));
            assert!(!g.log.iter().any(|e|matches!(e,game::Event::Arranged{..})));
            if p==1 {crate::support::refused(g.act(0,Move::Choose(Answer::Yes)), "that decision is another player's");}
            g.act(p,Move::Choose(if p==1 {Answer::No}else{Answer::Yes})).unwrap();
        }
        if text.contains("scry") {
            let mut copy=g.clone();
            for g in [&mut g,&mut copy] {
                for p in [0,2] {
                    let cards=match g.decision.clone().unwrap() {
                        Decision::Choose{player,choice:Choice::Arrange{cards,..}}=>{assert_eq!(player,p);cards},
                        other=>panic!("{other:?}"),
                    };
                    g.act(p,Move::Choose(Answer::Arrange{top:cards,away:vec![]})).unwrap();
                }
                let scried:Vec<_>=g.log.iter().filter_map(|e|if let game::Event::Arranged{player,..}=e{Some(*player)}else{None}).collect();
                assert_eq!(scried,vec![0,2]);
            }
        } else {assert_eq!(g.players.iter().map(|p|p.hand.len()).collect::<Vec<_>>(),vec![1,0,1]);}
        assert!(g.resolving().is_none());
    }
}

#[test]
fn r118_12_optional_reveal_binds_the_shown_card_and_declining_does_not_lose_life() {
    for accept in [false,true] {
        let d=CardDef::new("Omen", "{0}", "Sorcery", "You may reveal the top card of your library. If you do, each opponent loses life equal to that card's mana value.",None).unwrap();
        game::play::check(&d).unwrap();
        let mut g=Game::new(vec![d,creature()],3,20);
        let top=g.put(0,1,Zone::Library); let c=g.put(0,0,Zone::Hand);
        g.act(0,Move::Cast{card:c,targets:vec![]}).unwrap(); for p in 0..3 {g.act(p,Move::Pass).unwrap();}
        g.act(0,Move::Choose(if accept{Answer::Yes}else{Answer::No})).unwrap();
        cite!(["118.12"]; assert_eq!(g.players[1].life,if accept{17}else{20})); assert_eq!(g.players[2].life,g.players[1].life);
        assert_eq!(g.players[0].library,vec![top]);
        assert!(g.resolving().is_none());
    }
}

#[test]
fn r118_12_library_placement_cost_uses_the_old_card_information_after_the_move() {
    for accept in [false,true] {
        let d=CardDef::new("Storyteller effect", "{0}", "Sorcery", "You may put target enchantment card from your graveyard on top of your library. If you do, you gain life equal to its mana value.",None).unwrap();
        game::play::check(&d).unwrap();
        let target=CardDef::new("Enchantment", "{3}{W}","Enchantment","",None).unwrap();
        let mut g=Game::new(vec![d,target],2,20);
        let target=g.put(0,1,Zone::Graveyard); let card=g.put(0,0,Zone::Hand);
        g.act(0,Move::Cast{card,targets:vec![vec![Target::Object(target)]]}).unwrap(); pass(&mut g);
        g.act(0,Move::Choose(if accept{Answer::Yes}else{Answer::No})).unwrap();
        cite!(["118.12", "608.2h"]; assert_eq!(g.players[0].life,if accept{24}else{20}));
        assert_eq!(g.players[0].library.len(),usize::from(accept));
        assert_eq!(g.players[0].graveyard.len(),if accept{1}else{2});
    }
}

#[test]
fn r118_12_an_opponent_owns_the_optional_payment_and_its_object_choice() {
    let d=CardDef::new("Opponent payment", "{0}", "Sorcery", "Target opponent may sacrifice a creature. If they don't, you draw a card.",None).unwrap();
    game::play::check(&d).unwrap();
    let mut g=Game::new(vec![d,creature()],2,20);
    let bears:Vec<_>=(0..2).map(|_|g.put(1,1,Zone::Battlefield)).collect();
    g.put(0,1,Zone::Library); let card=g.put(0,0,Zone::Hand);
    g.act(0,Move::Cast{card,targets:vec![vec![Target::Player(1)]]}).unwrap(); pass(&mut g);
    assert_eq!(g.decision,Some(Decision::Choose{player:1,choice:Choice::May { possible: true }}));
    crate::support::refused(g.act(0,Move::Choose(Answer::Yes)), "that decision is another player's");
    let mut declined=g.clone(); declined.act(1,Move::Choose(Answer::No)).unwrap();
    cite!(["118.12"]; assert_eq!(declined.players[0].hand.len(),1)); assert_eq!(declined.battlefield,bears);
    g.act(1,Move::Choose(Answer::Yes)).unwrap();
    assert!(matches!(g.decision,Some(Decision::Choose{player:1,choice:Choice::Objects{..}})));
    assert_eq!(g.battlefield,bears);
    g.act(1,Move::Choose(Answer::Objects(vec![bears[0]]))).unwrap();
    cite!(["118.12"]; assert!(g.players[0].hand.is_empty())); assert_eq!(g.players[1].graveyard.len(),1);
}

#[test]
fn r608_2d_alternative_clauses_are_refused_until_the_action_choice_is_implemented() {
    // The old interpreter flattened body.rest without looking at its Join, so Lava Storm
    // damaged both attacking and blocking creatures. Admission was a false positive.
    let d=CardDef::new("Lava Storm", "{3}{R}{R}","Instant","Lava Storm deals 2 damage to each attacking creature or Lava Storm deals 2 damage to each blocking creature.",None).unwrap();
    assert!(game::play::check(&d).unwrap_err().0.contains("alternative clauses"));
}

#[test]
fn r608_2e_coordinated_sacrifices_collect_both_choices_before_any_permanent_leaves() {
    let d=CardDef::new("Structural Collapse", "{0}","Sorcery","Target player sacrifices an artifact and a land of their choice. Structural Collapse deals 2 damage to that player.",None).unwrap();
    game::play::check(&d).unwrap();
    let artifact=CardDef::new("Artifact", "{0}","Artifact","",None).unwrap();
    let both=CardDef::new("Artifact land", "","Artifact Land","",None).unwrap();
    let land=CardDef::new("Land", "","Land","",None).unwrap();
    let mut g=Game::new(vec![d,artifact,both,land],2,20);
    let artifact=g.put(1,1,Zone::Battlefield); let both=g.put(1,2,Zone::Battlefield); let land=g.put(1,3,Zone::Battlefield);
    let card=g.put(0,0,Zone::Hand);
    g.act(0,Move::Cast{card,targets:vec![vec![Target::Player(1)]]}).unwrap();pass(&mut g);
    g.act(1,Move::Choose(Answer::Objects(vec![artifact]))).unwrap();
    // one sacrifice of both: the player makes both choices first, in order (101.4c); Structural Collapse ruling 0f921cffb58829d4 (an artifact land can't be both) presumes it
    cite!(["101.4c"]; assert_eq!(g.battlefield,vec![artifact,both,land])); assert_eq!(g.players[1].life,20);
    let mut copy=g.clone();
    for (g,chosen) in [(&mut g,both),(&mut copy,land)] {
        g.act(1,Move::Choose(Answer::Objects(vec![chosen]))).unwrap();
        // The owner chooses the graveyard order of the simultaneous pair (404.3).
        g.act(1,Move::Choose(Answer::Objects(vec![artifact,chosen]))).unwrap();
        cite!(["701.21a"]; assert_eq!(g.players[1].graveyard.len(),2)); cite!(["120.3a"]; assert_eq!(g.players[1].life,18));
        cite!(["701.21a"]; assert_eq!(g.battlefield.len(),1));
        // Uncited (implementation): the resolution asks nothing more once both are sacrificed and the damage dealt
        assert!(g.resolving().is_none());
    }
}

#[test]
fn r701_24b_optional_search_can_hold_a_found_card_aside_for_either_conditional_destination() {
    for (accept,assassin) in [(false,false),(true,false),(true,true)] {
        let d=CardDef::new("Inventor effect", "{0}","Sorcery","You may search your library for an artifact card, reveal it, then shuffle. Put that card into your hand if you control an Assassin. Otherwise, put that card on top of your library.",None).unwrap();
        game::play::check(&d).unwrap();
        let artifact=CardDef::new("Artifact","{1}","Artifact","",None).unwrap();
        let assassin_def=CardDef::new("Assassin","{1}","Creature — Assassin","",Some((1,1))).unwrap();
        let mut g=Game::new(vec![d,artifact,assassin_def],2,20);
        let found=g.put(0,1,Zone::Library); g.put(0,2,Zone::Library);
        if assassin {g.put(0,2,Zone::Battlefield);}
        let before=g.players[0].library.clone();
        let card=g.put(0,0,Zone::Hand);
        g.act(0,Move::Cast{card,targets:vec![]}).unwrap();pass(&mut g);
        g.act(0,Move::Choose(if accept{Answer::Yes}else{Answer::No})).unwrap();
        if accept {g.act(0,Move::Choose(Answer::Objects(vec![found]))).unwrap();}
        assert!(g.resolving().is_none());
        if !accept {assert_eq!(g.players[0].library,before);assert!(g.players[0].hand.is_empty());}
        else if assassin {cite!(["701.24b"]; assert_eq!(g.players[0].hand.len(),1)); assert_eq!(g.obj(g.players[0].hand[0]).chars.name,"Artifact");}
        else {assert_eq!(g.players[0].library.len(),2); cite!(["701.24b"]; assert_eq!(g.obj(*g.players[0].library.last().unwrap()).chars.name,"Artifact"));}
    }
}

#[test]
fn r608_2h_each_players_quantity_uses_their_zone_without_rebinding_you() {
    for text in ["Each player draws a card for each creature card in their graveyard.", "Each player draws a card for each creature card in your graveyard."] {
        let d=CardDef::new("Nature's Resurgence", "{0}","Sorcery",text,None).unwrap();
        game::play::check(&d).unwrap();
        let mut g=Game::new(vec![d,creature()],3,20);
        for p in 0..3 {
            for _ in 0..p {g.put(p,1,Zone::Graveyard);}
            for _ in 0..3 {g.put(p,1,Zone::Library);}
        }
        let card=g.put(0,0,Zone::Hand);
        g.act(0,Move::Cast{card,targets:vec![]}).unwrap(); for p in 0..3{g.act(p,Move::Pass).unwrap();}
        cite!(["109.5", "608.2h"]; case: text; assert_eq!(g.players.iter().map(|p|p.hand.len()).collect::<Vec<_>>(),if text.contains("their"){vec![0,1,2]}else{vec![0,0,0]}));
        assert!(g.resolving().is_none());
    }
}

#[test]
fn r608_2c_mill_result_from_a_guarded_action_is_empty_on_the_skipped_path() {
    for accept in [false,true] {
        let d=CardDef::new("Farmer effect", "{0}","Sorcery","You may sacrifice a Food. If you do, mill three cards. You may put a permanent card from among them into your hand.",None).unwrap();
        game::play::check(&d).unwrap();
        let food=CardDef::new("Food", "{0}","Artifact — Food","",None).unwrap();
        let mut g=Game::new(vec![d,creature(),food],2,20);
        let food=g.put(0,2,Zone::Battlefield);
        let milled:Vec<_>=(0..3).map(|_|g.put(0,1,Zone::Library)).collect();
        let old=g.put(0,1,Zone::Graveyard); let card=g.put(0,0,Zone::Hand);
        g.act(0,Move::Cast{card,targets:vec![]}).unwrap();pass(&mut g);
        g.act(0,Move::Choose(if accept{Answer::Yes}else{Answer::No})).unwrap();
        if accept {
            assert!(!g.battlefield.contains(&food));
            // Author top-first order for the simultaneous mill, oldest first in the graveyard.
            g.act(0,Move::Choose(Answer::Objects(vec![milled[2],milled[1],milled[0]]))).unwrap();
            g.act(0,Move::Choose(Answer::Yes)).unwrap();
            let among=match g.decision.clone().unwrap(){Decision::Choose{choice:Choice::Objects{among,..},..}=>among,other=>panic!("{other:?}")};
            assert_eq!(among.len(),3);cite!(["608.2c"]; assert!(!among.contains(&old)));
            g.act(0,Move::Choose(Answer::Objects(vec![among[0]]))).unwrap();
            assert_eq!(g.players[0].hand.len(),1);
        }else{cite!(["118.12"]; assert_eq!(g.players[0].library.len(),3));assert!(g.players[0].hand.is_empty());assert!(g.battlefield.contains(&food));}
        assert!(g.resolving().is_none());
    }
}

#[test]
fn r107_3c_shared_x_for_each_opponent_does_not_require_a_per_player_value() {
    let spell = CardDef::new("Pile effect", "{0}", "Sorcery", "Each opponent loses X life, where X is the number of creatures you control.", None).unwrap();
    game::play::check(&spell).unwrap();
    let mut g = Game::new(vec![spell, creature()], 3, 20);
    for _ in 0..2 { g.put(0, 1, Zone::Battlefield); }
    for _ in 0..3 { g.put(1, 1, Zone::Battlefield); }
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    for p in 0..3 { g.act(p, Move::Pass).unwrap(); }
    cite!(["107.3c"]; assert_eq!(g.players.iter().map(|p| p.life).collect::<Vec<_>>(), vec![20, 18, 18]));
    let unsupported = CardDef::new("Individual X", "{0}", "Sorcery", "Each player draws X cards, where X is the number of creature cards in their graveyard.", None).unwrap();
    assert!(game::play::check(&unsupported).unwrap_err().0.contains("per-player quantity"));
}

#[test]
fn r608_2k_naming_the_source_in_the_body_takes_precedence_over_a_paid_cost() {
    let d = CardDef::new("Trafficker", "{0}", "Creature", "Sacrifice an artifact: Put a +1/+1 counter on this creature. It gains indestructible until end of turn.", Some((1, 1))).unwrap();
    game::play::check(&d).unwrap();
    let artifact = CardDef::new("Rock", "{0}", "Artifact", "", None).unwrap();
    let mut g = Game::new(vec![d, artifact], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let cost = g.put(0, 1, Zone::Battlefield);
    g.act(0, Move::Activate { source, ability: 0, targets: vec![], paying: vec![vec![cost]] }).unwrap();
    pass(&mut g);
    assert_eq!(g.obj(cost).zone, Zone::Gone);
    assert_eq!(g.power(source), Some(2));
    cite!(["608.2c"]; assert!(g.has(source, "Indestructible")));
}

#[test]
fn r113_7a_source_named_before_sacrifice_still_deals_the_following_damage() {
    let d = CardDef::new("Bombers", "{0}", "Creature", "When another creature enters, sacrifice this creature and it deals 3 damage to target player.", Some((1, 1))).unwrap();
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d, creature()], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let other = g.put(0, 1, Zone::Hand);
    for _ in 0..3 { g.add_mana(0, ir::ManaSymbol::Color(ir::Color::Green)); }
    g.act(0, crate::support::paid("GGG", Move::Cast { card: other, targets: vec![] })).unwrap(); pass(&mut g);
    assert!(matches!(g.decision, Some(Decision::Targets { .. })));
    g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap(); pass(&mut g);
    assert_eq!(g.obj(source).zone, Zone::Gone);
    cite!(["113.7a", "608.2h"]; assert_eq!(g.players[1].life, 17));
}
