//! Entry choices precede the simultaneous move (614.12, 303.4f). The operation retains
//! collected answers and payment commitments; resuming never repeats an earlier choice.
use super::EntryRequest;
use crate::replace::{Asks, EntryPayments};
use crate::state::{Answer, Choice, Decision, Game, ObjId, PlayerId, PublicChoice};

#[derive(Debug, Clone)]
enum Waiting { Defender {index:usize,player:PlayerId,among:Vec<crate::Target>,nth:usize,of:usize}, Attachment { index: usize, among: Vec<crate::Target> }, Own { index: usize, asks: Asks, choice: Choice } }

#[derive(Debug, Clone)]
pub(crate) struct Preparation {
    pub requests: Vec<EntryRequest>,
    pub payments: EntryPayments,
    pub choices: Vec<PublicChoice>,
    order: Vec<usize>,
    cursor: usize,
    attachment_done: bool,
    own_done: bool,
    skipped: Vec<usize>,
    waiting: Option<Waiting>,
}
impl Preparation {
    pub fn new(game: &mut Game, mut requests: Vec<EntryRequest>) -> Self {
        for request in &mut requests {
            if let EntryRequest::Token { controller, values, chosen, prepared, .. } = request {
                let on = game.new_object(None, values.current().chars.clone(), *controller, *controller, crate::Zone::Gone);
                game.obj_mut(on).copy_acquisition = chosen.copy_acquisition.clone();
                game.obj_mut(on).copied = chosen.copied.clone();
                game.obj_mut(on).token_faces = Some(values.faces.clone());
                game.obj_mut(on).transformed = values.transformed;
                *prepared = Some(on);
            }
        }
        let mut order: Vec<_> = (0..requests.len()).collect();
        let player = |r: &EntryRequest| match r { EntryRequest::Card { controller, .. } | EntryRequest::Token { controller, .. } => *controller };
        order.sort_by_key(|i| (player(&requests[*i]) + game.players.len() - game.active) % game.players.len());
        Self { requests, payments: EntryPayments::default(), choices: Vec::new(), order, cursor: 0,
            attachment_done: false, own_done:false, skipped: Vec::new(), waiting: None }
    }
    pub fn context(&self) -> Option<(ObjId, PlayerId)> {
        self.requests.get(*self.order.get(self.cursor)?)?.context()
    }
    pub fn waiting(&self) -> bool { self.waiting.is_some() }
    /// The question the entering permanent's own text asks, while it is the one asked.
    pub fn asks(&self) -> Option<&Asks> {
        match &self.waiting { Some(Waiting::Own { asks, .. }) => Some(asks), _ => None }
    }
    fn decision(&self) -> Decision {
        let (_, mut player) = self.context().expect("a card owns the entry decision");
        let choice = match self.waiting.as_ref().unwrap() {
            Waiting::Attachment { index, among } => {
                let (card, _) = self.requests[*index].context().expect("entry identity");
                Choice::Attach { aura: card, among: among.clone() }
            }
            Waiting::Own { choice, .. } => choice.clone(),
            Waiting::Defender {player:by,among,nth,of,..}=>{player=*by;Choice::Defender {among:among.clone(),nth:*nth,of:*of}},
        };
        Decision::Choose { player, choice }
    }
    fn next(&mut self) { self.cursor += 1; self.attachment_done = false; self.own_done=false; }
    pub fn advance(&mut self, game: &mut Game) -> crate::Res<Option<Decision>> {
        for request in &mut self.requests {
            let (card,controller,chosen,_)=request.parts();
            let mut entry=crate::replace::Entry::default();
            for arrival in &chosen.arrivals {arrival.checked_apply(game,&crate::select::Ctx::new(controller,card),&mut entry)?;}
            crate::operation::EntryInventory::new(&entry.counters)?;
        }
        if self.waiting.is_some() { return Ok(Some(self.decision())); }
        while let Some(&index) = self.order.get(self.cursor) {
            // A prohibited entry has no replacement choices or entry payments (614.17c).
            if self.requests[index].prohibited(game) { self.skipped.push(index); self.next(); continue; }
            let (card, controller, chosen, token) = self.requests[index].parts();
            let o = game.obj(card);
            // what enters face down is a creature whatever card it is (708.3)
            if (!token && matches!(o.zone, crate::Zone::Gone | crate::Zone::Battlefield)) || (!chosen.transformed && chosen.face_down.is_none() && !o.chars.types.iter().any(|t| t.permanent())) {
                self.skipped.push(index); self.next(); continue;
            }
            if !self.own_done && let Some(entry) = game.entry_choice(card, controller, chosen)
                && matches!(entry.asks, Asks::Copy { .. }) {
                let choice = entry.asks.choice(game, controller, &self.payments).expect("copy selection");
                self.waiting = Some(Waiting::Own { index, asks: entry.asks, choice });
                return Ok(Some(self.decision()));
            }
            if !self.attachment_done {
                if chosen.attached_to.is_none() && let Some(among) = game.entry_attachment_choices(card, controller, chosen) {
                    if among.is_empty() { self.skipped.push(index); self.next(); continue; }
                    self.waiting = Some(Waiting::Attachment { index, among });
                    return Ok(Some(self.decision()));
                }
                self.attachment_done = true;
            }
            if !self.own_done && let Some(entry) = game.entry_choice(card, controller, chosen) {
                if let Some(choice) = entry.asks.choice(game, controller, &self.payments) {
                    self.waiting = Some(Waiting::Own { index, asks: entry.asks, choice });
                    return Ok(Some(self.decision()));
                }
                entry.asks.apply(game, controller, None, chosen, &mut self.payments).expect("unpayable entry cost declines");
            }
            self.own_done=true;
            if chosen.choose_defender {
                let (player,among)=game.entry_defenders(card,controller,chosen);
                match among.as_slice() {
                    []=>{},[only] if !game.ask_forced=>chosen.attacking=Some(*only),
                    _=>{self.waiting=Some(Waiting::Defender {index,player,among,nth:self.cursor+1,of:self.order.len()});return Ok(Some(self.decision()));},
                }
                chosen.choose_defender=false;
            }
            self.next();
        }
        Ok(None)
    }
    pub fn answer(&mut self, game: &Game, answer: Answer) {
        let waiting = self.waiting.take().expect("entry owns an outstanding choice");
        let index = match &waiting { Waiting::Attachment { index, .. } | Waiting::Own { index, .. } | Waiting::Defender {index,..} => *index };
        let (card, controller, chosen, _) = self.requests[index].parts();
        let player=match &waiting {Waiting::Defender {player,..}=>*player,_=>controller};
        self.choices.push(PublicChoice::Entry { player, card, answer: answer.clone() });
        match waiting {
            Waiting::Attachment { .. } => {
                let Answer::Attach(to) = answer else { unreachable!("validated attachment answer") };
                chosen.attached_to = Some(to);
                self.attachment_done = true;
            }
            Waiting::Defender {..}=>{
                let Answer::Defender(target)=answer else {unreachable!("validated entry defender")};
                chosen.attacking=Some(target);chosen.choose_defender=false;
            }
            Waiting::Own { asks, .. } => {
                // a copy, or a value its ability follows with another question, keeps the card
                let more = matches!(asks, Asks::Copy { .. }) || asks.more();
                asks.apply(game, controller, Some(answer), chosen, &mut self.payments).expect("validated entry answer");
                if !more { self.own_done=true; }
            }
        }
    }
    pub fn eligible(&self, game: &mut Game) -> Vec<EntryRequest> {
        self.requests.iter().enumerate().filter(|(i,r)| !self.skipped.contains(i) && r.can_enter(game)).map(|(_,r)| r.clone()).collect()
    }
}
