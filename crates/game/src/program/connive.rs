//! 701.50: retain each conniver and actor around ordinary draw/discard children.
use super::{Frame,action,choice,expr};
use crate::{Game,ObjId,PlayerId,Target,Event};
use crate::state::{Answer,Choice,Decision,Zone};

#[derive(Debug,Clone,PartialEq,Eq)]
pub(super) struct Pending {
    remaining: Vec<ObjId>,
    order: Option<(PlayerId,Vec<ObjId>)>,
    current: Option<Current>,
}
#[derive(Debug,Clone,PartialEq,Eq)]
struct Current {
    object: ObjId,
    player: PlayerId,
    drawn: bool,
    countered: bool,
    waiting: bool,
    discard: Option<choice::Pending>,
    nonlands: Vec<ObjId>,
}
impl Pending {
    pub(super) fn new(game:&Game,f:&Frame,objects:&action::Operand)->Self {
        let mut remaining=Vec::new();
        for t in action::named(game,f,objects) {if let Target::Object(o)=t {if !remaining.contains(&o){remaining.push(o);}}}
        Self {remaining,order:None,current:None}
    }
    pub(super) fn answer(&mut self,answer:Answer) {
        if let Some((player,among))=self.order.take() {
            let Answer::Objects(chosen)=answer else {unreachable!("validated connive ordering")};
            let object=chosen[0];assert!(among.contains(&object));
            self.remaining.retain(|o|*o!=object);
            // Current control is captured by advance before any part of this connive.
            self.current=Some(Current{object,player,drawn:false,countered:false,waiting:false,discard:None,nonlands:Vec::new()});
        }else{self.current.as_mut().unwrap().discard.as_mut().unwrap().answer(answer);}
    }
    pub(super) fn operation_complete(&mut self,result:crate::operation::Completed) {
        let current=self.current.as_mut().expect("connive child owns a current permanent");
        if current.waiting {current.waiting=false;}else{current.discard.as_mut().unwrap().operation_complete(result);}
    }
    pub(super) fn advance(&mut self,game:&mut Game,f:&mut Frame)->crate::Res<Option<Decision>> {
        loop {
            if self.current.is_none() {
                let Some(&first)=self.remaining.iter().min_by_key(|o|(game.controller(**o)+game.players.len()-game.active)%game.players.len()) else { return Ok(None); };
                let player=game.controller(first);
                let among:Vec<_>=self.remaining.iter().copied().filter(|o|game.controller(*o)==player).collect();
                if among.len()>1 && !game.players[player].lost {
                    self.order=Some((player,among.clone()));return Ok(Some(Decision::Choose{player,choice:Choice::Objects{verb:ir::Verb::Connive,to:None,among,count:1}}));
                }
                self.remaining.retain(|o|*o!=first);
                self.current=Some(Current{object:first,player,drawn:false,countered:false,waiting:false,discard:None,nonlands:Vec::new()});
            }
            let current=self.current.as_mut().unwrap();
            assert!(!current.waiting,"draw completion is delivered before connive resumes");
            if !current.drawn {
                current.drawn=true;
                if let Err(decision)=game.draw_batch(vec![(current.player,1)])? {current.waiting=true;return Ok(Some(decision));}
            }
            if current.discard.is_none() {
                current.nonlands=game.players[current.player].hand.iter().copied().filter(|o|!game.is_a(*o,ir::CardType::Land)).collect();
                let discard=choice::Action::discard(action::Operand::Player(current.player),expr::Cards::any_card(),choice::Count::Exactly(expr::Amount::constant(1)));
                current.discard=Some(choice::Pending::new(game,f,&discard));
            }
            let discard=current.discard.as_mut().unwrap();
            if let Some(decision)=discard.advance(game)? {return Ok(Some(decision));}
            if !current.countered {
                current.countered=true;
                f.choices.extend_from_slice(&discard.public());f.transitions.extend_from_slice(discard.transitions());
                if discard.transitions().iter().any(|(was,now)|current.nonlands.contains(was) && discard.actual().contains(now)) && game.obj(current.object).zone==Zone::Battlefield {
                    let placement=crate::operation::Placement{on:current.object.into(),kind:"+1/+1".into(),count:1,by:current.player};
                    if let Err(decision)=game.put_counter_batch(vec![placement],crate::operation::EventOrigin::Effect)? {current.waiting=true;return Ok(Some(decision));}
                }
            }
            game.log.push(Event::Connived{object:current.object,player:current.player});
            self.current=None;
        }
    }
}
