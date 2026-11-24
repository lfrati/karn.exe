//! Token creation precedes battlefield entry (701.7b). One instruction retains a
//! counted inventory for each creator until its replacement ordering is complete.
use crate::{Game, PlayerId, Target};
use crate::entry::EntryRequest;
use crate::operation::{EventOrigin, Replaced, ReplacementId};

/// The exact pre-entry specifications offered by a creation replacement question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal { pub by: PlayerId, pub inventory: Vec<ProposedToken>, pub instructions: Option<ProposedInstructions> }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposedInstructions { pub tapped:bool, pub attacking:Option<Target>, pub choose_defender:bool, pub attached:Option<Target> }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposedToken { pub specification:serde_json::Value, pub count:u32, pub chars:crate::state::Chars, pub tapped:bool, pub attacking:Option<Target>, pub choose_defender:bool, pub attached:Option<Target> }

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Request {
    pub player: PlayerId,
    pub count: u32,
    pub values: crate::copy::Token,
    pub tapped: bool,
    pub attacking: Option<Target>,
    pub choose_defender: bool,
    /// Of what its controller chooses it attacks, what the effect allows (508.4).
    pub defending: Option<Defending>,
    pub attached: Option<Target>,
}
/// "Attacking that player or a planeswalker they control": what a token put onto the
/// battlefield attacking may attack, as the effect that made it says (508.4). Of a player no
/// longer in the game, or no planeswalker of theirs, it enters attacking nothing (508.4a).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Defending { pub players: Vec<PlayerId>, pub planeswalkers: bool }
impl Defending {
    pub(crate) fn allows(&self, game: &Game, defender: Target) -> bool {
        match defender {
            Target::Player(p) => self.players.contains(&p),
            Target::Object(o) => self.planeswalkers && game.is_a(o, ir::CardType::Planeswalker) && self.players.contains(&game.controller(o)),
        }
    }
}
impl Request {
    pub(crate) fn new(player:PlayerId,count:u32,values:crate::copy::Token,tapped:bool)->Self {
        Self { player,count,values,tapped,attacking:None,choose_defender:false,defending:None,attached:None }
    }
    fn entries(&self)->impl Iterator<Item=EntryRequest> + '_ {
        (0..self.count).map(|_|EntryRequest::token(self.player,self.values.clone(),self.tapped)
            .attacking(self.attacking).choose_defender(self.choose_defender).defending(self.defending.clone()).attached(self.attached))
    }
}

/// Entry instructions belong to the creating effect, separately from token values.
/// A heterogeneous inventory retains each original specification; append-one currently
/// refuses differing instruction sets rather than guessing which applies to the extra token.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Instructions { tapped:bool, attacking:Option<Target>, choose_defender:bool, defending:Option<Defending>, attached:Option<Target> }
impl Request {
    fn instructions(&self)->Instructions { Instructions {tapped:self.tapped,attacking:self.attacking,choose_defender:self.choose_defender,defending:self.defending.clone(),attached:self.attached} }
}
#[derive(Debug, Clone)]
pub(crate) struct Event {
    player: PlayerId,
    inventory: Vec<Request>,
    origin: EventOrigin,
    modified: bool,
    instructions: Option<Instructions>,
}
impl Event {
    pub(crate) fn batch(requests:Vec<Request>,origin:EventOrigin)->Vec<Self> {
        let mut events:Vec<Self>=Vec::new();
        for request in requests.into_iter().filter(|r|r.count>0) {
            if let Some(event)=events.iter_mut().find(|e|e.player==request.player) { event.inventory.push(request); }
            else { events.push(Self { player:request.player,inventory:vec![request],origin,modified:false,instructions:None }); }
        }
        for event in &mut events {
            let mut instructions=event.inventory.iter().map(Request::instructions);
            let first=instructions.next().expect("nonempty creating event");
            event.instructions=instructions.all(|other|other==first).then_some(first);
        }
        events
    }
    pub(crate) fn entries(&self)->Vec<EntryRequest> { self.inventory.iter().flat_map(Request::entries).collect() }
}
impl Replaced for Event {
    type Change=(crate::replace::TokenScope,crate::replace::TokenChange);
    fn affected(&self)->Target { Target::Player(self.player) }
    fn controller(&self,_:&Game)->PlayerId { self.player }
    fn question(&self)->(String,u32) { ("create".into(),0) }
    fn tokens(&self,game:&Game)->Option<Proposal> { Some(Proposal { by:self.player,instructions:self.instructions.as_ref().map(|i|ProposedInstructions {tapped:i.tapped,attacking:i.attacking,choose_defender:i.choose_defender,attached:i.attached}), inventory:self.inventory.iter().map(|r|ProposedToken {specification:game.audit_creation_specification(&r.values),count:r.count,chars:r.values.current().chars.clone(),tapped:r.tapped,attacking:r.attacking,choose_defender:r.choose_defender,attached:r.attached}).collect() }) }
    fn applicable(&mut self,game:&mut Game,applied:&[ReplacementId])->crate::Res<Vec<(ReplacementId,Self::Change)>> {
        Ok(game.token_replacements(self.player,&self.inventory,
            self.origin==EventOrigin::Effect || self.modified,applied))
    }
    fn apply(&mut self,(scope,change):Self::Change)->crate::Res<()> {
        let crate::replace::TokenChange::Multiply(factor)=change else {
            let crate::replace::TokenChange::Append(values)=change else {unreachable!()};
            // Quina/Helm rulings: inherit the creating effect's instructions,
            // not the abilities of the originally specified tokens (111.3).
            let instructions=self.instructions.as_ref().ok_or_else(||crate::Unsupported(
                "additional token: heterogeneous token entry instructions are not implemented".into()))?;
            self.inventory.iter().try_fold(1u32,|total,r|total.checked_add(r.count))
                .ok_or_else(||crate::Unsupported("token creation exceeds the engine numerical range".into()))?;
            let mut request=Request::new(self.player,1,values,instructions.tapped);
            request.choose_defender=instructions.choose_defender;request.attached=instructions.attached;request.attacking=instructions.attacking;request.defending=instructions.defending.clone();
            self.inventory.push(request);self.modified=true;return Ok(());
        };
        let counts=self.inventory.iter().map(|request|if scope.matches(&request.values) {
            request.count.checked_mul(factor).ok_or_else(||crate::Unsupported("token creation exceeds the engine numerical range".into()))
        } else { Ok(request.count) }).collect::<crate::Res<Vec<_>>>()?;
        for (request,count) in self.inventory.iter_mut().zip(counts) { request.count=count; }
        self.modified=true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CardDef,Move,Zone};
    use crate::state::{Answer,Choice,Decision};

    // Uncited (projection contract): full copiable abilities, not ability-row indices.
    #[test]
    fn creation_proposal_retains_printed_ability_text() {
        // Direct owner tests must initialize rule-defined tokens before lowering them.
        crate::token::ready();
        let game=Game::new(Vec::new(),2,20);
        let clue=crate::token::token_values(&ir::token::TokenKind::Predefined(ir::Predefined::Clue)).unwrap();
        let event=Event::batch(vec![Request::new(0,1,clue,false)],EventOrigin::Effect).pop().unwrap();
        let proposal=event.tokens(&game).unwrap();
        let rows=proposal.inventory[0].specification["front"]["rows"].as_array().unwrap();
        assert_eq!(rows.len(),1);
        assert!(rows[0].as_str().unwrap().to_lowercase().contains("draw a card"));
    }

    #[test]
    fn r614_16_creation_origin_and_modified_cost_are_distinct() {
        // An announcement cost is not a resolving effect. A replacement that
        // modifies that creation event makes its result effect-qualified (614.16).
        let defs=vec![
            CardDef::new("Season clause","{0}","Enchantment","If an effect would create one or more tokens under your control, it creates twice that many of those tokens instead.",None).unwrap(),
            CardDef::new("Unqualified","{0}","Enchantment","If one or more tokens would be created under your control, twice that many of those tokens are created instead.",None).unwrap(),
            CardDef::new("Model","{0}","Creature","",Some((1,1))).unwrap(),
        ];
        let mut game=Game::new(defs,2,20);
        let season=game.put(0,0,Zone::Battlefield);
        let model=game.put(0,2,Zone::Battlefield);
        let request=Request::new(0,1,game.token_copy(model),false);
        for (origin,expected) in [(EventOrigin::Effect,1),(EventOrigin::AnnouncementCost,0),(EventOrigin::TurnBased,0)] {
            let mut event=Event::batch(vec![request.clone()],origin).pop().unwrap();
            assert_eq!(event.applicable(&mut game,&[]).unwrap().len(),expected);
        }
        let other=game.put(0,1,Zone::Battlefield);
        let mut event=Event::batch(vec![request],EventOrigin::AnnouncementCost).pop().unwrap();
        let options=event.applicable(&mut game,&[]).unwrap();
        assert_eq!(options.len(),1);assert_eq!(options[0].0.source,other);
        let (id,change)=options.into_iter().next().unwrap();event.apply(change).unwrap();
        let remaining=event.applicable(&mut game,&[id]).unwrap();
        assert_eq!(remaining.len(),1);assert_eq!(remaining[0].0.source,season);
        assert_eq!(event.tokens(&game).unwrap().inventory[0].count,2);
    }

    #[test]
    fn r508_4_noncreature_entry_copy_has_no_defender_choice() {
        // 701.7b/508.4: multiplication precedes each copy's own entry choices.
        // The resulting noncreature must not inherit a premature defender choice.
        let defs=vec![
            CardDef::new("Double","{0}","Enchantment","If one or more tokens would be created under your control, twice that many of those tokens are created instead.",None).unwrap(),
            CardDef::new("Copy","{0}","Creature","You may have this creature enter as a copy of any artifact on the battlefield.",Some((2,2))).unwrap(),
            CardDef::new("Rock","{0}","Artifact","",None).unwrap(),
            CardDef::planeswalker("Walker","{0}","Planeswalker","+1: You gain 1 life.",5).unwrap(),
        ];
        for def in &defs {crate::play::check(def).unwrap();}
        let mut game=Game::new(defs,2,20);
        game.put(0,0,Zone::Battlefield);let model=game.put(0,1,Zone::Battlefield);
        let rock=game.put(0,2,Zone::Battlefield);let walker=game.put(1,3,Zone::Battlefield);
        game.put_counters(walker,"loyalty",5);game.part=ir::Part::DeclareAttackers;
        // Exercise the owning creation/entry continuation under a retained rule action.
        game.procedures.push(crate::procedure::Procedure::RuleAction {complete:false});
        let mut request=Request::new(0,1,game.token_copy(model),true);request.choose_defender=true;
        let Err(question)=game.create_tokens(vec![request],EventOrigin::Effect).unwrap() else {panic!("each token's copy selection")};
        game.decision=Some(question);
        for _ in 0..2 {
            assert!(matches!(game.decision,Some(Decision::Choose {choice:Choice::Select {..},..})));
            game.act(0,Move::Choose(Answer::Objects(vec![rock]))).unwrap();
        }
        let tokens=game.battlefield.iter().copied().filter(|on|game.obj(*on).token()).collect::<Vec<_>>();
        assert_eq!(tokens.len(),2);assert!(game.decision.is_none());
        for token in tokens {assert!(!game.is_a(token,ir::CardType::Creature));assert!(game.combat.attackers.iter().all(|(on,_)|*on!=token));}
    }
}
