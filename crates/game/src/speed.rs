//! Speed (702.179): a number a player has once a rule or effect sets it, from 1 to 4. A
//! player who controls a permanent with start your engines! and has no speed gets speed 1, a
//! state-based action (704.5aa). A player with speed has the rules' triggered ability
//! [`rule`] (702.179d), which has no source, like the monarch's (725.2).
use crate::{Event, Game, PlayerId};
use std::sync::OnceLock;

/// The most speed a player has: "max speed" (702.179e).
pub(crate) const MAX: u8 = 4;

/// "Whenever one or more opponents lose life during your turn, if your speed is less than 4,
/// your speed increases by 1. This ability triggers only once each turn." (702.179d)
pub(crate) fn rule() -> crate::ability::Template {
    static RULE: OnceLock<crate::ability::Template> = OnceLock::new();
    *crate::token::init(&RULE, || {
        // only an immutable ability definition: no source card or permanent is created
        let text = "Whenever one or more opponents lose life during your turn, if your speed is less than 4, your speed increases by 1. This ability triggers only once each turn.";
        let def = crate::CardDef::new("Speed", "", "", text, None).expect("rule text reads");
        crate::play::check(&def).expect("speed rule compiles");
        crate::ability::Template::keep(def)
    })
}

impl Game {
    /// A player's speed, 0 for a player who has none (702.179f).
    pub fn speed(&self, player: PlayerId) -> u8 { self.players[player].speed.unwrap_or(0) }

    /// "Your speed increases by 1" (702.179c-d): from none, it becomes 1; never above 4.
    pub(crate) fn increase_speed(&mut self, player: PlayerId) {
        let previous = self.players[player].speed;
        let speed = previous.map_or(1, |s| (s + 1).min(MAX));
        if previous != Some(speed) { self.set_speed(player, speed); }
    }

    fn set_speed(&mut self, player: PlayerId, speed: u8) {
        let previous = self.players[player].speed.replace(speed);
        self.log.push(Event::SpeedChanged { player, speed, previous });
    }

    /// 704.5aa: who controls a permanent with start your engines! and has no speed.
    pub(crate) fn starting_engines(&self) -> Vec<PlayerId> {
        let mut starting: Vec<PlayerId> = self.battlefield.iter()
            .filter(|o| self.has(**o, "Start Your Engines!"))
            .map(|o| self.controller(*o))
            .filter(|p| !self.players[*p].lost && self.players[*p].speed.is_none())
            .collect();
        starting.sort_unstable();
        starting.dedup();
        starting
    }

    /// Their speed becomes 1 (704.5aa).
    pub(crate) fn start_engines(&mut self, players: &[PlayerId]) {
        for p in players { if self.players[*p].speed.is_none() { self.set_speed(*p, 1); } }
    }
}

/// "Max speed — [Ability]" means "As long as your speed is 4, this object has '[Ability]'"
/// (702.178a): read as that, where the ability functions on the battlefield. One that
/// functions elsewhere (702.178b, "Exile this card from your graveyard") stays unread.
pub(crate) fn max_speed(a: &ir::Ability) -> Option<ir::Ability> {
    use ir::effect::{Effect, Sentence};
    let ir::Ability::Prefixed { keyword, ability } = a else { return None };
    if keyword.name() != "Max Speed" { return None; }
    if crate::functions::scope(ability, &[]).ok()? != crate::functions::Scope::In(crate::Zone::Battlefield) { return None; }
    // the reading, once, of the same sentence with a stand-in ability
    static GRANT: OnceLock<Option<ir::Ability>> = OnceLock::new();
    let grant = crate::token::init(&GRANT, || {
        let text = "As long as your speed is 4, this permanent has \"{T}: Add {C}.\"";
        crate::CardDef::new("Max speed", "{0}", "Artifact", text, None).ok()?.abilities().first().cloned()
    }).as_ref()?;
    let ir::Ability::Text(Effect(sentences)) = grant else { return None };
    let [Sentence::Led { lead, rest }] = sentences.as_slice() else { return None };
    // a static ability of this permanent's own: each of its sentences, as long as it holds
    if let ir::Ability::Text(Effect(own)) = &**ability && own.iter().all(|s| matches!(s, Sentence::Do(_))) {
        return Some(ir::Ability::Text(Effect(own.iter().map(|s| Sentence::Led { lead: lead.clone(), rest: Box::new(s.clone()) }).collect())));
    }
    // any other: this permanent has it, as long as it holds
    let mut rest = (**rest).clone();
    let Sentence::Do(body) = &mut rest else { return None };
    let ir::Action::Static(act) = &mut body.first.first.action else { return None };
    let ir::statics::StaticAct::Has(granted) = &mut **act else { return None };
    let [ir::action::Granted::Ability(quoted)] = granted.as_mut_slice() else { return None };
    **quoted = (**ability).clone();
    Some(ir::Ability::Text(Effect(vec![Sentence::Led { lead: lead.clone(), rest: Box::new(rest) }])))
}
