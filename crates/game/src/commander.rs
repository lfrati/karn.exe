//! Persistent commander designation and casting (903.3, 903.8).
//! Physical identity is private game state; targets and the player protocol still use ObjId.
use crate::state::CardId;
use crate::{Game, ObjId, PlayerId, Zone};
use crate::state::{Answer, Choice, Decision};
use crate::start::Refused;

/// Optional destination changes (903.9b), retained by the operation doing the move.
/// Moving within a library is not a zone change and does not offer this replacement.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Returns {
    pending: Vec<(PlayerId, ObjId, bool)>,
    next: usize,
    pub selected: Vec<ObjId>,
    pub choices: Vec<crate::state::PublicChoice>,
}
impl Returns {
    pub(crate) fn new(game: &Game, moves: impl IntoIterator<Item = (ObjId, Zone)>) -> Self {
        if game.commanders.is_empty() { return Self::default(); }
        let mut pending = Vec::new();
        for (object, zone) in moves {
            if matches!(zone, Zone::Hand | Zone::Library) && game.obj(object).zone != zone
                && game.is_commander(object) && !pending.iter().any(|(_, o, _)| *o == object)
            {
                let public = !matches!(game.obj(object).zone, Zone::Hand | Zone::Library);
                pending.push((game.obj(object).owner, object, public));
            }
        }
        pending.sort_by_key(|(p, _, _)| (p + game.players.len() - game.active) % game.players.len());
        Self { pending, ..Self::default() }
    }
    /// It goes elsewhere after all, before it is asked of: nothing to offer.
    pub(crate) fn forget(&mut self, object: ObjId) {
        assert_eq!(self.next, 0, "asked of before its destination was settled");
        self.pending.retain(|(_, o, _)| *o != object);
    }
    pub(crate) fn decision(&self) -> Option<Decision> {
        self.pending.get(self.next).map(|&(player, object, _)| Decision::Choose { player, choice: Choice::CommandZone { object } })
    }
    pub(crate) fn answer(&mut self, answer: Answer) {
        let &(player, object, public) = self.pending.get(self.next).expect("pending commander destination");
        match answer { Answer::Yes => self.selected.push(object), Answer::No => {}, _ => unreachable!("validated commander return") }
        // A decline must not reveal that a hidden draw/search/hand contained the commander.
        // Public-source departures and accepted public destinations remain visible (101.4b).
        if public || answer == Answer::Yes {
            self.choices.push(crate::state::PublicChoice::Objects { player, verb: ir::Verb::Move,
                objects: if answer == Answer::Yes { vec![object] } else { vec![] } });
        }
        self.next += 1;
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Commander {
    pub card: CardId,
    pub owner: PlayerId,
    pub definition: usize,
    pub identity: Vec<ir::Color>,
    /// Successful command-zone casts, distinguished by casting player (903.8).
    pub casts: Vec<u32>,
    /// Combat damage to each player by this physical commander (903.10a).
    pub damage: Vec<u32>,
}

impl Game {
    /// Commander setup with one legendary creature per player. The submitted libraries
    /// contain 99 cards; designated commanders are supplied separately (903.5a, 903.6).
    /// This validates Comprehensive Rules construction, not an external format ban list.
    pub fn start_commander(defs: impl Into<std::sync::Arc<Vec<crate::CardDef>>>, decks: &[Vec<usize>], commanders: &[usize], first: PlayerId, seed: u64) -> Result<Game, Refused> {
        Self::start_commander_from(defs, decks, commanders, first, crate::start::Initial::Seed(seed))
    }

    /// As [`Self::start_commander`], the initial shuffles' outcomes from a seed or a record
    /// ([`crate::start::Initial`]); recorded orders index each deck without its commander.
    pub fn start_commander_from(defs: impl Into<std::sync::Arc<Vec<crate::CardDef>>>, decks: &[Vec<usize>], commanders: &[usize], first: PlayerId, initial: crate::start::Initial<'_>) -> Result<Game, Refused> {
        if decks.len() < 2 || commanders.len() != decks.len() || first >= decks.len() {
            return Err(Refused::malformed("one commander per player and a valid starting player"));
        }
        let mut g = Game::new(defs, decks.len(), 40);
        for (player, (deck, &commander)) in decks.iter().zip(commanders).enumerate() {
            if deck.len() != 99 { return Err(Refused::illegal(format!("player {player}: exactly 100 cards including the commander (903.5a)"))); }
            let leader = g.defs.get(commander).ok_or_else(|| Refused::malformed("unknown commander definition"))?;
            let identity = leader.color_identity();
            let mut names = std::collections::BTreeSet::new();
            for &definition in deck.iter().chain(std::iter::once(&commander)) {
                let def = g.defs.get(definition).ok_or_else(|| Refused::malformed("unknown Commander deck definition"))?;
                crate::play::check(def).map_err(|e| Refused::unsupported(format!("{}: {}", def.name(), e.0)))?;
                if !(def.is(ir::CardType::Land) && def.supertypes().contains(&ir::Supertype::Basic)) && !names.insert(def.name()) {
                    return Err(Refused::illegal(format!("player {player}: duplicate {} (903.5b)", def.name())));
                }
                if !def.color_identity().iter().all(|c| identity.contains(c)) {
                    return Err(Refused::illegal(format!("player {player}: {} outside commander's color identity (903.5c)", def.name())));
                }
                if def.is(ir::CardType::Land) && !crate::activate::basic_land_colors(def.subtypes()).all(|c| identity.contains(&c)) {
                    return Err(Refused::illegal(format!("player {player}: {} has a basic land type outside commander's colors (903.5d)", def.name())));
                }
            }
            let object = g.put(player, commander, Zone::Command);
            g.designate_commander(object)?;
        }
        g.prepare_start(decks, &vec![None; decks.len()], first, initial)
    }

    /// Low-level fixture/setup designation. Managed Commander setup additionally validates
    /// deck construction and supplies all format rules before exposing the game to players.
    pub fn designate_commander(&mut self, object: ObjId) -> Result<(), Refused> {
        let o = self.get_object(object).ok_or_else(|| Refused::malformed("unknown commander"))?;
        if o.zone != Zone::Command { return Err(Refused::illegal("designate a commander in the command zone (903.6)")); }
        let physical = o.card_id.ok_or_else(|| Refused::illegal("a commander is a physical card (903.3)"))?;
        let def = o.card.and_then(|i| self.defs.get(i)).ok_or_else(|| Refused::malformed("a commander has a printed definition"))?;
        // a commander is a legendary card (903.3); of those that may be one, the engine runs
        // creature cards alone, not Vehicle or Spacecraft cards (903.3) nor cards whose ability
        // says they can be one (903.3a)
        if !def.supertypes().contains(&ir::Supertype::Legendary) {
            return Err(Refused::illegal("a commander is a legendary card (903.3)"));
        }
        if !def.is(ir::CardType::Creature) {
            return Err(Refused::unsupported("commander setup currently supports legendary creature cards (903.3)"));
        }
        if self.commanders.iter().any(|c| c.owner == o.owner) {
            return Err(Refused::unsupported("multiple designated commanders need their deck-construction permission (903.3a)"));
        }
        let commander = Commander { card: physical, owner: o.owner, definition: o.card.unwrap(), identity: def.color_identity(),
            casts: vec![0; self.players.len()], damage: vec![0; self.players.len()] };
        self.commanders.push(commander);
        Ok(())
    }

    pub(crate) fn commander_index(&self, object: ObjId) -> Option<usize> {
        let physical = self.get_object(object)?.card_id?;
        self.commanders.iter().position(|c| c.card == physical)
    }

    pub fn is_commander(&self, object: ObjId) -> bool { self.commander_index(object).is_some() }

    pub fn commander_identity(&self, player: PlayerId) -> Vec<ir::Color> {
        ir::Color::ALL.iter().copied().filter(|color|
            self.commanders.iter().any(|c| c.owner == player && c.identity.contains(color))).collect()
    }

    pub fn commander_casts(&self, object: ObjId, player: PlayerId) -> u32 {
        self.commander_index(object).and_then(|i| self.commanders[i].casts.get(player)).copied().unwrap_or(0)
    }

    pub(crate) fn command_cast_allowed(&self, player: PlayerId, object: ObjId) -> bool {
        self.get_object(object).is_some_and(|o| o.zone == Zone::Command && o.owner == player)
            && self.commander_index(object).is_some()
    }

    pub(crate) fn commander_tax(&self, spell: ObjId) -> u32 {
        let o = self.obj(spell);
        if o.cast_from != Some(Zone::Command) { return 0; }
        self.commander_casts(spell, o.controller).saturating_mul(2)
    }

    pub(crate) fn commander_cast_completed(&mut self, spell: ObjId, player: PlayerId) {
        if self.obj(spell).cast_from == Some(Zone::Command) && let Some(i) = self.commander_index(spell) {
            self.commanders[i].casts[player] = self.commanders[i].casts[player].saturating_add(1);
        }
    }

    pub(crate) fn commander_damage(&mut self, source: ObjId, player: PlayerId, amount: u32) {
        if let Some(i) = self.commander_index(source) {
            self.commanders[i].damage[player] = self.commanders[i].damage[player].saturating_add(amount);
        }
    }

    pub(crate) fn lethal_commander_damage(&self, player: PlayerId) -> bool {
        self.commanders.iter().any(|c| c.damage[player] >= 21)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CardDef, Target};

    #[test]
    fn r903_10a_counts_only_actual_combat_damage_by_one_physical_commander_across_zones() {
        let leader = CardDef::new("Leader", "{0}", "Legendary Creature — Human", "", Some((3, 3))).unwrap();
        let mut g = Game::new(vec![leader], 3, 100);
        let commander = g.put(0, 0, Zone::Command);
        g.designate_commander(commander).unwrap();
        let commander = g.move_to(commander, Zone::Battlefield, 0).unwrap();
        let ordinary = g.put(2, 0, Zone::Battlefield);
        g.damage(commander, Target::Player(1), 30);
        g.damage_as_rule(&[(ordinary, Target::Player(1), 20)], true).unwrap();
        g.move_object(ordinary, Zone::Graveyard);
        g.damage_as_rule(&[(commander, Target::Player(1), 10)], true).unwrap();
        g.state_based_actions();
        assert!(!g.players[1].lost);
        let commander = g.move_to(commander, Zone::Exile, 0).unwrap();
        let commander = g.move_to(commander, Zone::Battlefield, 0).unwrap();
        g.obj_mut(commander).controller = 2;
        g.retire_events(g.log.end());
        g.collect_departed();
        let mut clone = g.clone();
        for game in [&mut g, &mut clone] {
            game.damage_as_rule(&[(commander, Target::Player(1), 10)], true).unwrap();
            game.state_based_actions();
            assert!(!game.players[1].lost);
            game.damage_as_rule(&[(commander, Target::Player(1), 1)], true).unwrap();
            game.state_based_actions();
            assert!(game.players[1].lost);
            assert!(game.players[1].life > 0);
            assert!(!game.players[0].lost && !game.players[2].lost);
        }
    }
}
