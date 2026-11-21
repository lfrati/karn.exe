//! 701.63: a frozen amount and exact enduring incarnation; the optional counter cost
//! and its mandatory token fallback share ordinary replacement-aware operations.
use super::{Frame, action};
use crate::state::{ActionOption, ActionOptionBody, Answer, Choice, Decision, Zone};
use crate::{Game, ObjId, PlayerId, Res, Target};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Pending {
    object: ObjId,
    player: PlayerId,
    amount: u32,
    paid: Option<bool>,
}
impl Pending {
    pub(super) fn new(game: &Game, frame: &Frame, permanent: &action::Operand, amount: u32) -> Res<Option<Self>> {
        if amount == 0 {
            return Ok(None);
        } // 701.63b, even when the permanent has departed.
        let named = action::named(game, frame, permanent);
        if named.is_empty() {
            return Ok(None);
        } // An absent singular producer supplies no permanent (609.3).
        let [Target::Object(object)] = named.as_slice() else {
            return crate::unsupported("endure: expected one exact permanent incarnation");
        };
        let object = *object;
        // controller() explicitly uses the departed incarnation's LKI, not the
        // controller of the triggered ability and not any movement successor (608.2h).
        let player = game.controller(object);
        if game.players[player].lost {
            return Ok(None);
        }
        let possible =
            game.obj(object).zone == Zone::Battlefield && game.may_place(Target::Object(object), "+1/+1", player);
        Ok(Some(Self { object, player, amount, paid: (!possible).then_some(false) }))
    }
    pub(super) fn answer(&mut self, answer: Answer) {
        self.paid = Some(match answer {
            Answer::Alternative(0) => true,
            Answer::No => false,
            _ => unreachable!("validated optional Endure cost"),
        });
    }
    pub(super) fn decision(&self) -> Option<Decision> {
        self.paid.is_none().then(|| Decision::Choose {
            player: self.player,
            choice: Choice::Alternative {
                optional: true,
                among: vec![ActionOption {
                    index: 0,
                    body: ActionOptionBody::Counters { on: self.object, kind: "+1/+1".into(), count: self.amount },
                }],
            },
        })
    }
    pub(super) fn payment(&self) -> Option<(PlayerId, bool)> {
        self.paid.map(|paid| (self.player, paid))
    }
    pub(super) fn request(&self) -> Res<crate::perform::Action> {
        if self.paid == Some(true) {
            // 118.11: after this legally chosen cost is modified by replacements, it
            // remains paid; never infer payment success from the resulting count.
            Ok(crate::perform::Action::PutCounters(vec![crate::operation::Placement {
                on: Target::Object(self.object),
                kind: "+1/+1".into(),
                count: self.amount,
                by: self.player,
            }]))
        } else {
            Ok(crate::perform::Action::Create(vec![crate::creation::Request::new(
                self.player,1,
                crate::token::endure_spirit(self.amount)?,
                false,
            )]))
        }
    }
}
