//! 701.44: each permanent told to explore, in APNAP order of their controllers, its
//! controller choosing among their own (701.44d): the top card of their library revealed, a
//! land put into their hand; otherwise a +1/+1 counter on the permanent, and the card into
//! their graveyard if they choose. It has explored once that is done, whatever of it could
//! not be (701.44b).
use super::{Frame,action};
use crate::{Game,ObjId,PlayerId,Target,Event};
use crate::state::{Answer,Choice,Decision,Zone};

#[derive(Debug,Clone,PartialEq,Eq)]
pub(super) struct Pending {
    remaining: Vec<ObjId>,
    order: Option<(PlayerId,Vec<ObjId>)>,
    current: Option<Current>,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
enum Step { Reveal, Counter, Ask, Asked(bool), Move, Done }
#[derive(Debug,Clone,PartialEq,Eq)]
struct Current {
    object: ObjId,
    player: PlayerId,
    step: Step,
    /// The card revealed, when there was one, and whether it is a land card.
    revealed: Option<(ObjId,bool)>,
    /// A child operation (the counter, the card's move) is under way.
    waiting: bool,
}
impl Pending {
    pub(super) fn new(game:&Game,f:&Frame,objects:&action::Operand)->Self {
        let mut remaining=Vec::new();
        for t in action::named(game,f,objects) {if let Target::Object(o)=t {if !remaining.contains(&o){remaining.push(o);}}}
        Self {remaining,order:None,current:None}
    }
    /// The card revealed by the explore under way: it stays revealed until that explore is
    /// done, while its graveyard choice is pending (701.20a).
    pub(super) fn revealed(&self)->Option<ObjId> { self.current.as_ref().and_then(|c|c.revealed).map(|(card,_)|card) }
    fn start(object:ObjId,player:PlayerId)->Current { Current{object,player,step:Step::Reveal,revealed:None,waiting:false} }
    pub(super) fn answer(&mut self,answer:Answer) {
        if let Some((player,among))=self.order.take() {
            let Answer::Objects(chosen)=answer else {unreachable!("validated explore ordering")};
            let object=chosen[0];assert!(among.contains(&object));
            self.remaining.retain(|o|*o!=object);
            self.current=Some(Self::start(object,player));
            return;
        }
        let current=self.current.as_mut().expect("an exploring permanent asks");
        assert_eq!(current.step,Step::Ask);
        current.step=Step::Asked(match answer {Answer::Yes=>true,Answer::No=>false,_=>unreachable!("validated may answer")});
    }
    pub(super) fn operation_complete(&mut self,_result:crate::operation::Completed) {
        let current=self.current.as_mut().expect("explore child owns a current permanent");
        assert!(current.waiting);current.waiting=false;
    }
    pub(super) fn advance(&mut self,game:&mut Game,_f:&mut Frame)->crate::Res<Option<Decision>> {
        loop {
            if self.current.is_none() {
                // its controller as it is told to, or as it last existed (701.44c)
                let Some(&first)=self.remaining.iter().min_by_key(|o|(game.controller(**o)+game.players.len()-game.active)%game.players.len()) else { return Ok(None); };
                let player=game.controller(first);
                let among:Vec<_>=self.remaining.iter().copied().filter(|o|game.controller(*o)==player).collect();
                if among.len()>1 && !game.players[player].lost {
                    self.order=Some((player,among.clone()));return Ok(Some(Decision::Choose{player,choice:Choice::Objects{verb:ir::Verb::Explore,to:None,among,count:1}}));
                }
                self.remaining.retain(|o|*o!=first);
                self.current=Some(Self::start(first,player));
            }
            let current=self.current.as_mut().unwrap();
            assert!(!current.waiting,"a child completes before explore resumes");
            let player=current.player;
            match current.step {
                Step::Reveal => {
                    current.revealed=game.top_cards(player,1).first().copied()
                        .and_then(|top|game.reveal(player,vec![top]).first().copied())
                        .map(|card|(card,game.is_a(card,ir::CardType::Land)));
                    current.step=if matches!(current.revealed,Some((_,true))) {Step::Move} else {Step::Counter};
                }
                Step::Counter => {
                    current.step=if current.revealed.is_some() {Step::Ask} else {Step::Done};
                    // on the permanent only while it is still there (701.44b)
                    if game.obj(current.object).zone==Zone::Battlefield {
                        let placement=crate::operation::Placement{on:current.object.into(),kind:"+1/+1".into(),count:1,by:player};
                        if let Err(decision)=game.put_counter_batch(vec![placement],crate::operation::EventOrigin::Effect)? {current.waiting=true;return Ok(Some(decision));}
                    }
                }
                Step::Ask => {
                    let (card,_)=current.revealed.unwrap();
                    if game.obj(card).zone!=Zone::Library || game.players[player].lost {current.step=Step::Done;continue;}
                    return Ok(Some(Decision::Choose{player,choice:Choice::May}));
                }
                Step::Asked(into_graveyard) => {
                    current.step=if into_graveyard {Step::Move} else {Step::Done};
                }
                Step::Move => {
                    current.step=Step::Done;
                    let (card,land)=current.revealed.unwrap();
                    if game.obj(card).zone==Zone::Library {
                        let to=if land {Zone::Hand} else {Zone::Graveyard};
                        if let Err(decision)=game.departure_batch(vec![(card,crate::departure::Kind::Move(to))])? {current.waiting=true;return Ok(Some(decision));}
                    }
                }
                Step::Done => {
                    game.log.push(Event::Explored{object:current.object,player});
                    self.current=None;
                }
            }
        }
    }
}
