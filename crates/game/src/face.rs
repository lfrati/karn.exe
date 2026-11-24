//! Face-down spells and permanents (708): a status of an object on the stack or the
//! battlefield, the characteristics it then has, and turning it face up again.
//!
//! A face-down permanent is the same object as the card it represents, with an immutable
//! face-down definition in front of everything else that defines it ([`Game::def`]). Three
//! views of what it is stay apart:
//!
//! - [`Game::physical_card`]: the printed card, ignoring copy and type effects. What a
//!   manifest's special action asks (701.40b), what an instant or sorcery shows (701.40g),
//!   and what a reveal or look names.
//! - [`Game::underlying`]: its copiable values without the face-down overlay, so including
//!   copy effects (708.10). What it is once turned face up, before continuous effects; with
//!   them, what it would be face up is asked of a view in which it is turned (702.37e,
//!   702.168d).
//! - [`Game::def`] and the layered values: what it is now, a nameless 2/2 (708.2a).
//!
//! A card exiled face down (406.3) is another thing: no status, but a card with no
//! characteristics at all ([`Hidden`], 406.3a), which only players granted permission may
//! look at.
//!
//! A permanent becomes face down by being manifested or cloaked (701.40a, 701.58a), or by
//! being cast face down with morph, megamorph or disguise (702.37c, 702.168b) and resolving,
//! or by an effect turning a face-up permanent face down (708.2a). Of face-down exile,
//! only exiling the top cards of a library, one of the cards a player just looked at, or a
//! foretold card ([`crate::foretell`]).
use std::sync::{Arc, OnceLock};

use crate::cost::Plan;
use crate::observation::Kind;
use crate::state::{Chars, Zone};
use crate::{CardDef, Event, Game, ObjId, PlayerId};

/// Which face of a permanent is up (110.5e, 708.2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Face {
    #[default]
    Up,
    Down(Down),
}

/// A face-down permanent, and the public way it came to be face down (708.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Down {
    pub by: Concealment,
}

/// How a spell or permanent came to be face down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Concealment {
    /// 701.40a.
    Manifested,
    /// 701.58a.
    Cloaked,
    /// 702.37.
    Morph,
    /// 702.168.
    Disguised,
    /// A face-up permanent an effect turned face down (708.2a).
    TurnedDown,
}
impl Concealment {
    pub fn name(self) -> &'static str {
        match self {
            Self::Manifested => "manifested",
            Self::Cloaked => "cloaked",
            Self::Morph => "morph",
            Self::Disguised => "disguised",
            Self::TurnedDown => "turned face down",
        }
    }
}

/// A card exiled face down (406.3): not a status (110.5d) and never a 2/2, but a card with no
/// characteristics (406.3a), which only its lookers may look at. It is the exiled object's
/// alone: once the card leaves exile it is a new object (400.7), face up wherever it is.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hidden {
    /// Players allowed to look (406.3), in player order. Permission persists even if its
    /// granting instruction ends, without requiring an actual look. Public information.
    pub lookers: Vec<PlayerId>,
    /// The permanent whose controllers gain permission to look (702.75a). Grants are
    /// retained in `lookers` for this exile incarnation (406.3).
    pub watcher: Option<ObjId>,
}

/// The special action that turns a face-down permanent face up, by the rule that permits it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    /// Pay the mana cost of the creature card it represents (701.40b).
    Manifested,
    /// Pay the mana cost of the creature card it represents (701.58b).
    Cloaked,
    /// Pay its morph cost (702.37e).
    Morph,
    /// Pay its disguise cost (702.168d).
    Disguise,
}
impl Way {
    pub const ALL: [Way; 4] = [Way::Manifested, Way::Cloaked, Way::Morph, Way::Disguise];
    pub fn name(self) -> &'static str {
        match self {
            Self::Manifested => "manifested",
            Self::Cloaked => "cloaked",
            Self::Morph => "morph",
            Self::Disguise => "disguise",
        }
    }
}

/// What an instruction to turn a permanent face up did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnedUp {
    /// It is face up now.
    Turned,
    /// It represents an instant or sorcery card: that card was revealed, and it stays face
    /// down (701.40g).
    Revealed,
    /// It is not a face-down permanent.
    Nothing,
}

/// The face-down characteristics (708.2a, 701.40a, 702.37a): a 2/2 creature with no text,
/// no name, no subtypes and no mana cost; with ward {2} when it is disguised or cloaked
/// (702.168a, 701.58a). Immutable definitions, shared by every game.
pub(crate) fn definition(by: Concealment) -> &'static Arc<CardDef> {
    static PLAIN: OnceLock<Arc<CardDef>> = OnceLock::new();
    static WARDED: OnceLock<Arc<CardDef>> = OnceLock::new();
    match by {
        Concealment::Manifested | Concealment::Morph | Concealment::TurnedDown =>
            crate::token::init(&PLAIN, || Arc::new(CardDef::new("", "", "Creature", "", Some((2, 2))).expect("face-down characteristics read"))),
        Concealment::Disguised | Concealment::Cloaked =>
            crate::token::init(&WARDED, || Arc::new(CardDef::new("", "", "Creature", "Ward {2}", Some((2, 2))).expect("face-down characteristics read"))),
    }
}

/// What a card exiled face down is (406.3a): no characteristics at all, so no name, types,
/// cost, color, text or power, whatever the card is.
pub(crate) fn hidden_definition() -> &'static Arc<CardDef> {
    static DEF: OnceLock<Arc<CardDef>> = OnceLock::new();
    crate::token::init(&DEF, || Arc::new(CardDef::new("", "", "", "", None).expect("no characteristics read")))
}

impl Game {
    /// Which face of this object is up. Only a spell or a permanent is ever face down here.
    pub fn face(&self, o: ObjId) -> Face {
        self.obj(o).face
    }

    /// A face-down spell or permanent, or what was one as it last existed.
    pub fn face_down(&self, o: ObjId) -> bool {
        matches!(self.obj(o).face, Face::Down(_))
    }

    /// A card exiled face down (406.3), or what was one as it last existed.
    pub fn hidden(&self, o: ObjId) -> Option<&Hidden> {
        self.obj(o).hidden.as_ref()
    }

    /// Exile a card face down (406.3) as the effect that says so, the players in `lookers`
    /// allowed to look at it. The card it becomes in exile has no
    /// characteristics (406.3a). A replacement that puts it anywhere else leaves it face up
    /// there. Nothing else is changed: it is exiled as any card is ([`Game::exile_result`]).
    pub(crate) fn exile_hidden(&mut self, object: ObjId, lookers: &[PlayerId], watcher: Option<ObjId>) -> Option<ObjId> {
        let now = self.exile_result(object)?;
        if self.obj(now).zone == Zone::Exile {
            let mut lookers = lookers.to_vec();
            if let Some(watcher) = watcher.filter(|w| self.obj(*w).zone == Zone::Battlefield) {
                lookers.push(self.controller(watcher));
            }
            lookers.sort_unstable();
            lookers.dedup();
            let o = self.obj_mut(now);
            o.hidden = Some(Hidden { lookers, watcher });
            o.chars = hidden_definition().chars();
        }
        Some(now)
    }

    /// Control changing grants permission immediately, independently of observation.
    /// CR 406.3 retains it after control changes again or the hideaway source leaves.
    pub(crate) fn grant_hideaway_look(&mut self, watcher: ObjId, player: PlayerId) {
        let cards: Vec<_> = self.exile.iter().copied().filter(|o|
            self.hidden(*o).is_some_and(|h| h.watcher == Some(watcher) && !h.lookers.contains(&player))
        ).collect();
        for card in cards {
            let hidden = self.obj_mut(card).hidden.as_mut().expect("hidden exile");
            hidden.lookers.push(player);
            hidden.lookers.sort_unstable();
        }
    }

    /// The physical card the object is, with its front face's printed characteristics:
    /// not what a copy or type-changing effect makes it (701.40b, 701.40g). None for what no
    /// card represents.
    pub fn physical_card(&self, o: ObjId) -> Option<(usize, &CardDef)> {
        self.obj(o).card.map(|i| (i, &self.defs[i]))
    }

    /// How this card is cast face down for the alternative cost at `row`, and that cost: a
    /// morph or disguise ability its card prints (702.37c, 702.168b). None for another row.
    pub(crate) fn face_down_cast(&self, card: ObjId, row: usize) -> Option<(Concealment, &crate::Res<Plan>)> {
        let (_, def) = self.physical_card(card)?;
        def.costs.face_down.iter().find(|(at, _, _)| *at == row).map(|(_, by, plan)| (*by, plan))
    }

    /// The name a player is shown when this object is revealed or looked at. A face-down
    /// permanent or a card exiled face down is shown as the card it is, not as the nameless
    /// 2/2, what a copy effect makes it or a card with no characteristics (708.9, 701.40b,
    /// 701.40g, 406.3); anything else by its name. An object stays face down as it last
    /// existed, so this does not change once the object has left.
    pub fn shown_name(&self, o: ObjId) -> &str {
        let obj = self.obj(o);
        match (matches!(obj.face, Face::Down(_)) || obj.hidden.is_some(), self.physical_card(o)) {
            (true, Some((_, card))) => card.name(),
            _ => &obj.chars.name,
        }
    }

    /// What defines the object apart from being face down: its copy effects (708.10), its
    /// token baseline or its card's face that is up.
    pub(crate) fn underlying(&self, id: ObjId) -> Option<&CardDef> {
        let o = self.obj(id);
        // a Room permanent is what its unlocked halves make it (709.5)
        if o.zone == Zone::Battlefield { return self.underlying_def(id).map(|def| def.room(o.unlocked)); }
        self.underlying_def(id)
    }

    /// The definition before the face-down overlay and Room lock abilities; in
    /// particular, both Room halves remain available as copiable values (709.5).
    pub(crate) fn underlying_def(&self, id: ObjId) -> Option<&CardDef> {
        let o = self.obj(id);
        if let Some(values) = &o.copied { return values.definition(self); }
        if let Some(faces) = &o.token_faces {
            let def = faces.face(o.transformed || o.cast_as == crate::card::CastAs::Face(1)).definition(self)?;
            // A cast split-card copy has only its chosen half on the stack (709.3c).
            return match o.cast_as { crate::card::CastAs::Half(i) if o.zone == Zone::Stack => def.halves.get(i), crate::card::CastAs::Adventure if o.zone == Zone::Stack => def.adventure().or_else(|| def.subtypes.contains(&ir::Subtype::adventure()).then_some(def)), crate::card::CastAs::Omen if o.zone == Zone::Stack => def.omen().or_else(|| def.subtypes.contains(&ir::Subtype::omen()).then_some(def)), crate::card::CastAs::Overloaded if o.zone == Zone::Stack => def.overloaded().or(Some(def)), _ => Some(def) };
        }
        let Some(card) = o.card else {
            return o.chars.predefined.and_then(|kind| crate::token::definition(kind).ok());
        };
        let def = &self.defs[card];
        // a double-faced permanent with its back face up is only that face (712.8e)
        if o.transformed { return def.back.as_deref(); }
        match o.cast_as { crate::card::CastAs::Half(i) => def.halves.get(i), crate::card::CastAs::Face(i) => def.modal_face(i).ok(), crate::card::CastAs::Adventure => def.adventure(), crate::card::CastAs::Omen => def.omen(), crate::card::CastAs::Overloaded => def.overloaded(), _ => Some(def) }
    }

    /// The characteristics the object has face up: its copy effects' values (708.10), or
    /// its token's or card's face that is up.
    pub(crate) fn face_up_chars(&self, id: ObjId) -> Chars {
        let o = self.obj(id);
        if let Some(values) = &o.copied {
            return if o.zone == Zone::Battlefield { values.permanent_chars(self, o.unlocked) } else { values.chars.clone() };
        }
        if let Some(faces) = &o.token_faces {
            let values = faces.face(o.transformed);
            return if o.zone == Zone::Battlefield { values.permanent_chars(self, o.unlocked) } else { values.chars.clone() };
        }
        match o.card {
            Some(i) if o.transformed => self.defs[i].back().map_or_else(|| o.chars.clone(), CardDef::chars),
            Some(i) if o.zone == Zone::Battlefield => self.defs[i].room(o.unlocked).chars(),
            Some(i) => self.defs[i].chars(),
            None => o.chars.clone(),
        }
    }

    /// The one writer of a permanent's characteristics after a change of face or of copy
    /// effects: the face-down values while it is face down, otherwise what it is face up.
    pub(crate) fn refresh_face(&mut self, id: ObjId) {
        let chars = match self.obj(id).face { Face::Down(down) => definition(down.by).chars(), Face::Up => self.face_up_chars(id) };
        self.obj_mut(id).chars = chars;
    }

    /// Write down that these face-down objects' cards were shown to every player, as the
    /// cards they are (708.9, 701.40b, 701.40g), and log it as a reveal when the rule says
    /// they are revealed.
    pub(crate) fn show_face_down(&mut self, by: PlayerId, objects: &[ObjId], revealed: bool) {
        if objects.is_empty() { return; }
        if revealed { self.event_log_mut().push(Event::Revealed { player: by, cards: objects.to_vec() }); }
        self.disclose(Kind::Revealed, by, objects);
    }

    /// Turn a face-down permanent face up by an effect (701.40e, 708.8). It is the same
    /// object (708.8) with a new timestamp (613.7f); it does not enter the battlefield, so
    /// nothing that triggers on entering does. A manifested or cloaked instant or sorcery card
    /// is revealed instead and stays face down (701.40g, 701.58g).
    pub fn turn_face_up(&mut self, o: ObjId) -> TurnedUp {
        self.turn_up(o, false)
    }

    /// Turn a face-up permanent face down by an effect that lists no characteristics (708.2a):
    /// it is the same object, a 2/2 with no text, no name, no subtypes and no mana cost, with
    /// a new timestamp (613.7f). Its counters, attachments, damage and tapped status stay. A
    /// face-down permanent (708.2b) or a double-faced one (712.16) is not turned, and nothing
    /// happens. Whether it can be turned face up again is what it is face up: a morph or
    /// disguise ability (702.37e, 702.168d), nothing otherwise.
    pub fn turn_face_down(&mut self, o: ObjId) -> bool {
        let object = self.obj(o);
        if object.zone != Zone::Battlefield || matches!(object.face, Face::Down(_)) { return false; }
        let double_faced = match &object.token_faces {
            Some(faces) => faces.back.is_some(),
            None => self.physical_card(o).is_some_and(|(_, d)| d.back().is_some()),
        };
        if double_faced { return false; }
        let stamp = self.tick();
        let object = self.obj_mut(o);
        object.face = Face::Down(Down { by: Concealment::TurnedDown });
        object.stamp = stamp;
        self.refresh_face(o);
        self.event_log_mut().push(Event::TurnedFaceDown { object: o });
        true
    }

    /// `announced`: the special action already showed the card to every player (701.40b).
    pub(crate) fn turn_up(&mut self, o: ObjId, announced: bool) -> TurnedUp {
        let object = self.obj(o);
        if object.zone != Zone::Battlefield { return TurnedUp::Nothing; }
        let Face::Down(down) = object.face else { return TurnedUp::Nothing };
        let controller = self.controller(o);
        if matches!(down.by, Concealment::Manifested | Concealment::Cloaked)
            && self.physical_card(o).is_some_and(|(_, d)| d.is(ir::CardType::Instant) || d.is(ir::CardType::Sorcery))
        {
            self.show_face_down(controller, &[o], true);
            return TurnedUp::Revealed;
        }
        if !announced { self.show_face_down(controller, &[o], false); }
        self.flip_up(o);
        self.event_log_mut().push(Event::TurnedFaceUp { object: o });
        TurnedUp::Turned
    }

    /// The permanent is face up: the same object with a new timestamp (613.7f) and its normal
    /// characteristics (708.8). What is shown and what triggers is the caller's.
    fn flip_up(&mut self, o: ObjId) {
        let stamp = self.tick();
        let object = self.obj_mut(o);
        object.face = Face::Up;
        object.stamp = stamp;
        self.refresh_face(o);
    }

    /// What this face-down permanent would be if it were face up (702.37e, 702.168d): its
    /// normal copiable values (708.8, 708.10) with every continuous effect that would then
    /// apply to it, those applied to it face down among them (708.8), through the layers
    /// (613.1). Asked of a private view in which it is turned; nothing here is.
    fn face_up_seen(&self, o: ObjId) -> crate::layers::Seen {
        let mut view = self.clone();
        view.flip_up(o);
        view.seen_for(o, crate::layers::Aspect::Abilities)
    }

    /// What turning this face-down permanent face up this way costs (116.2b), and whether
    /// that cost is a megamorph cost (702.37b).
    fn turn_face_up_terms(&self, player: PlayerId, o: ObjId, way: Way) -> crate::play::ActionResult<(Plan, bool)> {
        let object = self.get_object(o).ok_or("no such object")?;
        if object.zone != Zone::Battlefield { return Err("only a permanent is turned face up (116.2b)".into()); }
        let Face::Down(down) = object.face else { return Err("that permanent is face up (116.2b)".into()) };
        if self.controller(o) != player { return Err("only its controller turns a face-down permanent face up (116.2b)".into()); }
        let (by, rule) = match way {
            Way::Manifested if down.by == Concealment::Manifested => (None, ""),
            Way::Manifested => return Err("only a manifested permanent is turned face up for its card's mana cost (701.40b)".into()),
            Way::Cloaked if down.by == Concealment::Cloaked => (None, ""),
            Way::Cloaked => return Err("only a cloaked permanent is turned face up for its card's mana cost this way (701.58b)".into()),
            Way::Morph => (Some(Concealment::Morph), "702.37e"),
            Way::Disguise => (Some(Concealment::Disguised), "702.168d"),
        };
        // Whether it has the ability, and its cost, is what it would have face up: an ability
        // an effect would remove from it then is no cost (702.37e, 702.168d). The manifest and
        // cloak ways below ask only the card (701.40b, 701.58b).
        if let Some(by) = by {
            let up = self.face_up_seen(o);
            let (cost, megamorph) = crate::keyword::face_down_costs(&up.keywords).find(|(kind, _, _)| *kind == by)
                .map(|(_, cost, megamorph)| (cost.clone(), megamorph))
                .ok_or_else(|| format!("that permanent would have no such cost if it were face up ({rule})"))?;
            let underlying = self.underlying(o).ok_or("nothing defines that permanent face up")?;
            let plan = Plan::compile(&ir::Cost(vec![ir::CostPart::Mana(cost)]), &crate::select::Support::definition(underlying, None))?;
            return Ok((plan, megamorph));
        }
        let (_, card) = self.physical_card(o).ok_or("no card represents that permanent")?;
        if !card.is(ir::CardType::Creature) { return Err("only a creature card is turned face up for its mana cost (701.40b, 701.58b)".into()); }
        let cost = card.cost().ok_or("a card with no mana cost can't be turned face up for it (701.40b, 701.58b)")?;
        let plan = Plan::compile(&ir::Cost(vec![ir::CostPart::Mana(cost.clone())]), &crate::select::Support::definition(card, None))?;
        if !plan.phyrexian().is_empty() { return Err(crate::Unsupported("turning face up for a Phyrexian mana cost is not supported".into()).into()); }
        let plan = if plan.x() > 0 { plan.announced(Some(0))? } else { plan };
        Ok((plan, false))
    }

    /// The special action (116.2b, 701.40b): show every player the creature card and its
    /// mana cost, pay that cost, then turn the permanent face up. The showing is part of the
    /// announcement: it is recorded once, survives a payment that waits for mana, and is
    /// not withdrawn when the payment is reversed.
    pub(crate) fn turn_face_up_action(
        &mut self,
        player: PlayerId,
        o: ObjId,
        way: Way,
        steps: &[crate::legal::Step],
        with: Option<&crate::state::Pool>,
    ) -> crate::play::ActionResult<()> {
        let mut proposed = self.clone();
        let (cost, megamorph) = proposed.turn_face_up_terms(player, o, way)?;
        proposed.show_face_down(player, &[o], false);
        let library_tops = proposed.library_tops();
        proposed.procedures.push(crate::procedure::Procedure::Payment(
            crate::payment::Payment::turn_face_up(player, o, cost, megamorph, steps, with).library_tops(library_tops),
        ));
        proposed.continue_execution()?;
        proposed.expose_payments();
        self.adopt(proposed);
        Ok(())
    }

    /// The special action's cost is paid: turn the permanent face up, and if that was a
    /// megamorph cost, put a +1/+1 counter on it as part of turning it, through the ordinary
    /// counter operation, before anything that triggers on its turning looks (702.37b). A
    /// permanent no longer face down on the battlefield is not turned (701.40b).
    pub(crate) fn turn_up_paid(&mut self, o: ObjId, megamorph: bool) -> crate::Res<()> {
        if self.turn_up(o, true) != TurnedUp::Turned { return Ok(()); }
        let mut instructions = self.as_turned_face_up(o)?;
        if megamorph {
            instructions.insert(0, vec![crate::operation::Placement { on: o.into(), kind: "+1/+1".to_string(), count: 1, by: self.controller(o) }]);
        }
        if instructions.is_empty() { return Ok(()); }
        let parent = self.procedures.len();
        if let Err(decision) = self.put_counter_instructions(instructions,crate::operation::EventOrigin::Effect)? {
            self.procedures.insert(parent, crate::procedure::Procedure::RuleAction { complete: false });
            self.decision = Some(decision);
        }
        Ok(())
    }

    /// The counters its "as this is turned face up" abilities put on a permanent just turned
    /// face up, as part of its turning (708.11): each its own placement.
    pub(crate) fn as_turned_face_up(&self, o:ObjId)->crate::Res<Vec<Vec<crate::operation::Placement>>> {
        let by=self.controller(o);let ctx=crate::select::Ctx::new(by,o);let mut instructions=Vec::new();
        for (key,replacement) in self.replacements(o) {
            if let crate::replace::Replacement::AsTurnedFaceUp(counters)=replacement {
                let mut instruction=Vec::new();
                for (kind,n) in counters {let count=n.checked(self,&ctx.clone().with_key(key.clone()),&crate::program::expr::NoLocals)?;
                    if count>0 {instruction.push(crate::operation::Placement{on:o.into(),kind:kind.clone(),count,by});}
                }
                if !instruction.is_empty(){instructions.push(instruction);}
            }
        }
        Ok(instructions)
    }

    /// 708.9, 702.143f: a player who leaves the game reveals the face-down permanents and spells
    /// and the face-down foretold cards they own; at the end of the game all of them are revealed.
    pub(crate) fn reveal_face_down_of(&mut self, owner: Option<PlayerId>) {
        let foretold = self.exile.iter().copied().filter(|o| self.obj(*o).hidden.is_some() && self.foretold(*o).is_some());
        let down: Vec<ObjId> = self.battlefield.iter().chain(&self.stack).copied().filter(|o| self.face_down(*o)).chain(foretold)
            .filter(|o| owner.is_none_or(|p| self.obj(*o).owner == p)).collect();
        let mut by_owner: Vec<(PlayerId, Vec<ObjId>)> = Vec::new();
        for o in down {
            let owner = self.obj(o).owner;
            match by_owner.iter_mut().find(|(p, _)| *p == owner) {
                Some((_, list)) => list.push(o),
                None => by_owner.push((owner, vec![o])),
            }
        }
        for (owner, objects) in by_owner { self.show_face_down(owner, &objects, true); }
    }
}
