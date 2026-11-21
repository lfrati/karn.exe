//! Player designation and source-free rule subscriptions (725).
use crate::{CardDef, Event, Game, PlayerId};
use std::sync::OnceLock;
#[derive(Debug, Clone, Copy)]
pub(crate) enum Rule { EndStep, CombatDamage }
impl Rule {
    /// Its full text, as 725.2 says it: "At the beginning of the monarch's end step, that player
    /// draws a card" and "Whenever a creature deals combat damage to the monarch, its
    /// controller becomes the monarch." That is its text on the stack (603.3, 405.4). Its
    /// [`Rule::definition`] is compiled from the same ability said of the player who controls
    /// it, who is the monarch as it triggers (725.2), since the grammar does not read "the
    /// monarch" as a player.
    fn text(self) -> &'static str {
        match self {
            Self::EndStep => "At the beginning of the monarch's end step, that player draws a card.",
            Self::CombatDamage => "Whenever a creature deals combat damage to the monarch, its controller becomes the monarch.",
        }
    }
    /// The text 725.2 gives the monarch's ability this definition is, if it is one.
    pub(crate) fn stated(definition: crate::ability::Template) -> Option<&'static str> {
        [Self::EndStep, Self::CombatDamage].into_iter().find(|rule| rule.definition() == definition).map(Self::text)
    }
    pub(crate) fn definition(self) -> crate::ability::Template {
        static END: OnceLock<crate::ability::Template> = OnceLock::new();
        static DAMAGE: OnceLock<crate::ability::Template> = OnceLock::new();
        let (cell,name,text)=match self {
            Self::EndStep => (&END,"Monarch end step","At the beginning of your end step, you draw a card."),
            Self::CombatDamage => (&DAMAGE,"Monarch combat damage","Whenever a creature deals combat damage to you, its controller becomes the monarch."),
        };
        *crate::token::init(cell, || {
            // Only an immutable ability definition. No source card or permanent is created.
            let def=CardDef::new(name,"","",text,None).expect("rule text reads");
            crate::play::check(&def).expect("monarch rule compiles");
            crate::ability::Template::keep(def)
        })
    }
}
impl Game {
    pub(crate) fn become_monarch(&mut self, player:PlayerId) {
        if !self.players[player].lost && self.monarch != Some(player) {
            let previous=self.monarch.replace(player);
            self.log.push(Event::MonarchChanged {player:Some(player),previous});
        }
    }
    pub(crate) fn monarch_leaving(&mut self, leaving:&[PlayerId]) {
        if !self.monarch.is_some_and(|p| leaving.contains(&p)) {return;}
        let next=(0..self.players.len()).map(|offset|(self.active+offset)%self.players.len())
            .find(|p| !self.players[*p].lost && !leaving.contains(p));
        let previous=self.monarch;
        self.monarch=next;
        self.log.push(Event::MonarchChanged {player:next,previous});
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Move,Target,Zone};
    use crate::state::{Answer,Choice,Decision};
    fn game(players:usize) -> Game {
        let defs=vec![
            CardDef::new("Crimson Fleet Commodore","{3}{R}","Creature — Ogre Pirate","Trample\nWhen this creature enters, you become the monarch.",Some((5,2))).unwrap(),
            CardDef::new("Mountain","","Basic Land — Mountain","",None).unwrap(),
            CardDef::new("Dredger","{0}","Creature","Dredge 1",Some((1,1))).unwrap(),
        ];
        for d in &defs {crate::play::check(d).unwrap();}
        let mut g=Game::new(defs,players,20);
        for p in 0..players {for _ in 0..12 {g.put(p,1,Zone::Library);}}
        g
    }
    fn resolve(g:&mut Game) {for _ in 0..g.players.iter().filter(|p|!p.lost).count() {g.act(g.priority,Move::Pass).unwrap();}}
    #[test]
    fn r725_1_real_entry_uses_the_compiled_action_and_designates_one_monarch() {
        let mut g=game(2);
        assert_eq!(g.monarch,None);
        let spell=g.put(0,0,Zone::Hand);g.players[0].pool.colors[3]=4;
        g.act(0,Move::Paying(crate::state::Pool { colors:[0,0,0,4,0], ..Default::default() }, Box::new(Move::Cast{card:spell,targets:vec![]}))).unwrap();resolve(&mut g);
        assert_eq!(g.monarch,None);
        resolve(&mut g);assert_eq!(g.monarch,Some(0));
        g.become_monarch(1);g.become_monarch(1);
        assert_eq!(g.monarch,Some(1));
        assert_eq!(g.log.iter().filter(|e|matches!(e,Event::MonarchChanged {..})).count(),2);
    }
    #[test]
    fn r725_2_end_step_ability_has_no_source_and_keeps_its_original_controller() {
        let mut g=game(2);g.become_monarch(0);
        g.begin_part(ir::Part::End).unwrap();
        let ability=*g.stack.last().unwrap();
        assert_eq!(g.obj(ability).ability,Some((None,0)));
        assert!(g.obj(ability).card.is_none());
        assert!(g.obj(ability).chars.colors.is_empty());
        let audit=g.audit_game_objects();
        assert_eq!(audit["objects"][ability.0.to_string()]["occurrences"][0]["rule_ref"],serde_json::json!({"rule":"725.2","sub":1}));
        assert_eq!(audit["objects"][ability.0.to_string()]["occurrences"][0]["controller"],0);
        g.become_monarch(1);
        resolve(&mut g);
        assert_eq!(g.players[0].hand.len(),1);assert!(g.players[1].hand.is_empty());
    }
    #[test]
    fn r725_2_the_rules_draw_uses_replacements_and_resumes_a_cloned_procedure() {
        let mut g=game(2);g.become_monarch(0);let dredger=g.put(0,2,Zone::Graveyard);
        let library = g.players[0].library.clone();
        let top = *library.last().unwrap();
        g.begin_part(ir::Part::End).unwrap();resolve(&mut g);
        let effect = crate::operation::ReplacementId::from_ability(
            dredger, crate::binding::AbilityKey::printed(0), 0);
        assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::Replacement {
            on: Target::Player(0), kind: "draw".into(), count: 1, counters: None, tokens: None,
            among: vec![effect], optional: true,
        } }));
        let before = format!("{g:?}");
        assert_eq!(g.act(0,Move::Pass), Err("a decision is pending: a choice of the effect being applied (608.2d)".into()));
        assert_eq!(format!("{g:?}"), before);
        let mut copy=g.clone();
        for g in [&mut g,&mut copy] {
            g.act(0,Move::Choose(Answer::No)).unwrap();
            assert_eq!(g.players[0].hand.len(),1);
            assert_eq!(g.obj(g.players[0].hand[0]).card, Some(1));
            assert_eq!(g.players[0].library, library[..library.len()-1]);
            assert_eq!(g.obj(top).zone, Zone::Gone);
            assert_eq!(g.players[0].graveyard, [dredger]);
            assert!(g.stack.is_empty());assert!(g.decision.is_none());
        }
    }
    #[test]
    fn r725_2_combat_damage_transfers_only_on_resolution_using_the_creatures_controller() {
        let mut g=game(3);g.become_monarch(0);
        let source=g.put(1,0,Zone::Battlefield);
        g.damage_as_rule(&[(source,Target::Player(0),1)],true).unwrap();g.settle().unwrap();
        assert_eq!(g.monarch,Some(0));
        assert_eq!(g.stack.len(),1);assert_eq!(g.obj(g.stack[0]).ability,Some((None,0)));
        g.obj_mut(source).controller=2;
        resolve(&mut g);assert_eq!(g.monarch,Some(2));
    }
    #[test]
    fn r725_2_noncombat_and_zero_damage_do_not_transfer_the_designation() {
        let mut g=game(2);g.become_monarch(0);
        let source=g.put(1,0,Zone::Battlefield);
        g.damage(source,Target::Player(0),1);g.damage_as_rule(&[(source,Target::Player(0),0)],true).unwrap();g.settle().unwrap();
        assert!(g.stack.is_empty());assert_eq!(g.monarch,Some(0));
    }
    #[test]
    fn r725_4_simultaneous_departures_choose_a_surviving_active_or_next_player_once() {
        let mut g=game(4);g.become_monarch(0);g.active=1;
        g.players[0].life=0;g.players[1].life=0;g.settle().unwrap();
        assert_eq!(g.monarch,Some(2));
        assert_eq!(g.log.iter().filter(|e|matches!(e,Event::MonarchChanged {..})).count(),2);
    }
    #[test]
    fn r725_2_event_time_monarch_is_preserved_when_a_later_effect_changes_it() {
        let mut g=game(3);g.become_monarch(0);
        let source=g.put(1,0,Zone::Battlefield);
        g.damage_as_rule(&[(source,Target::Player(0),1)],true).unwrap();
        g.become_monarch(2);g.settle().unwrap();
        assert_eq!(g.obj(g.stack[0]).controller,0);
        resolve(&mut g);assert_eq!(g.monarch,Some(1));
    }
    #[test]
    fn r725_2_multiple_combat_triggers_are_ordered_and_use_last_known_controllers() {
        let mut g=game(3);g.become_monarch(0);
        let first=g.put(1,0,Zone::Battlefield);let second=g.put(2,0,Zone::Battlefield);
        g.damage_as_rule(&[(first,Target::Player(0),1),(second,Target::Player(0),1)],true).unwrap();g.settle().unwrap();
        assert!(matches!(&g.decision,Some(Decision::Order{player:0,abilities}) if abilities.len()==2));
        let pending=g.audit_pending_trigger_order();
        for (index,source) in [first,second].into_iter().enumerate() {
            assert_eq!(pending["occurrences"][index]["ability"]["rule_ref"],serde_json::json!({"rule":"725.2","sub":2}));
            assert_eq!(pending["occurrences"][index]["controller"],0);
            assert_eq!(pending["occurrences"][index]["bound"]["inputs"][0]["subjects"],serde_json::json!([{"object":source.0}]));
        }
        for order in [vec![0,1],vec![1,0]] {
            let mut copy=g.clone();copy.act(0,Move::Order(order.clone())).unwrap();
            // The last chosen trigger resolves first; the other determines the final monarch.
            copy.move_object(first,Zone::Graveyard);copy.move_object(second,Zone::Graveyard);
            resolve(&mut copy);resolve(&mut copy);
            assert_eq!(copy.monarch,Some(if order[0]==0 {1} else {2}));
        }
    }
    #[test]
    fn r725_2_a_monarch_trigger_can_be_countered_without_a_source_card() {
        let mut g=game(2);g.become_monarch(0);g.begin_part(ir::Part::End).unwrap();
        let ability=g.stack[0];assert!(g.counter(ability));
        assert!(g.stack.is_empty());assert!(g.players[0].hand.is_empty());
        assert_eq!(g.monarch,Some(0));
    }
    #[test]
    fn r725_2_no_monarch_draws_at_another_players_end_step() {
        let mut g=game(2);g.become_monarch(1);g.begin_part(ir::Part::End).unwrap();
        assert!(g.stack.is_empty());assert!(g.players[1].hand.is_empty());
    }

}
