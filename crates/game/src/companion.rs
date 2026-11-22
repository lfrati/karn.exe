//! Starting-deck companion conditions and the once-per-game special action (702.139).
//! An outside-game designation is not an object in any game zone. Payment reserves a
//! source for the ordinary cost procedure; only success brings a card into the game.
use crate::program::expr::{self, Filter};
use crate::select::{Ctx, Support};
use crate::{CardDef, Game, ObjId, PlayerId, Res, Target, Zone, unsupported};
#[derive(Debug, Clone)]
pub(crate) struct Condition {
    deck: DeckCondition,
    cost: crate::cost::Plan,
}
#[derive(Debug, Clone)]
enum DeckCondition {
    Each { each: Filter, has: Filter },
    NoRepeatedManaSymbol,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chosen {
    pub definition: usize,
    pub taken: bool,
}
fn characteristic(f: &ir::Filter) -> bool {
    match f {
        ir::Filter::All(v) | ir::Filter::Any(v) => v.iter().all(characteristic),
        ir::Filter::Not(x) => characteristic(x),
        ir::Filter::Atom(
            ir::Atom::Type(_)
            | ir::Atom::PermanentType
            | ir::Atom::Subtype(_)
            | ir::Atom::Supertype(_)
            | ir::Atom::Color(_),
        ) => true,
        ir::Filter::Atom(ir::Atom::Stat { stat: ir::Stat::ManaValue, value: ir::Amount::Const(_), .. }) => true,
        _ => false,
    }
}
pub(crate) fn compile(def: &CardDef) -> Option<Res<Condition>> {
    let all = def.abilities.iter().filter(|a| matches!(a, ir::Ability::Companion { .. } | ir::Ability::CompanionRule(_))).collect::<Vec<_>>();
    if all.is_empty() { return None; }
    Some((|| {
        let [rule] = all.as_slice() else { return unsupported("one companion condition per card"); };
        let support = Support::definition(def, None);
        let deck = match rule {
            ir::Ability::Companion { each, has } => {
                let mut bare = each.clone();
                bare.filter = ir::Filter::any_object();
                if bare != ir::ObjectSet::of(ir::Domain::Card) || !characteristic(&each.filter) || !characteristic(has) {
                    return unsupported("companion: starting-deck printed characteristic predicate");
                }
                DeckCondition::Each {
                    each: Filter::compile(&each.filter, &support, &mut expr::linked(&support))?,
                    has: Filter::compile(has, &support, &mut expr::linked(&support))?,
                }
            }
            ir::Ability::CompanionRule(ir::ability::CompanionRule::NoRepeatedManaSymbol) => DeckCondition::NoRepeatedManaSymbol,
            _ => return unsupported("companion: a starting-deck condition the engine does not check (702.139)"),
        };
        let cost = crate::cost::Plan::compile(
            &ir::Cost(vec![ir::CostPart::Mana(ir::mana::ManaCost(vec![ir::mana::ManaSymbol::Generic(3)]))]),
            &support,
        )?;
        Ok(Condition { cost, deck })
    })())
}
impl Game {
    pub fn companion(&self, player: PlayerId) -> Option<&Chosen> {
        self.companions.get(player)?.as_ref()
    }
    /// A card revealed as this player's companion: one with a companion ability whose
    /// condition their starting deck fulfills (103.2b, 702.139a), or the setup is refused.
    pub(crate) fn designate_companion(&mut self, player: PlayerId, definition: usize) -> Result<(), crate::start::Refused> {
        use crate::start::Refused;
        let def = self.defs.get(definition).ok_or_else(|| Refused::malformed("unknown companion definition"))?;
        crate::play::check(def)?;
        let condition = def.companion.as_ref().ok_or_else(|| Refused::illegal("that card has no companion condition"))?.as_ref().map_err(|e| Refused::unsupported(e.0.clone()))?;
        for card in &self.players[player].library {
            let ctx = Ctx::new(player, *card);
            let target = Target::Object(*card);
            let allowed = match &condition.deck {
                DeckCondition::Each { each, has } => !each.matches(self, &ctx, &expr::NoLocals, target) || has.matches(self, &ctx, &expr::NoLocals, target),
                DeckCondition::NoRepeatedManaSymbol => self.def(*card).expect("starting-deck card").cost().is_none_or(|cost| cost.0.iter().enumerate().all(|(index, symbol)| !cost.0[..index].contains(symbol))),
            };
            if !allowed {
                return Err(Refused::illegal(format!("starting deck does not meet companion condition: {}", self.obj(*card).chars.name)));
            }
        }
        self.companions[player] = Some(Chosen { definition, taken: false });
        Ok(())
    }
    pub(crate) fn companion_cost(&self, player: PlayerId) -> crate::play::ActionResult<crate::cost::Plan> {
        if !self.sorcery_timing(player) {
            return Err("companion action requires your main phase and an empty stack (116.2g)".into());
        }
        let chosen = self.companion(player).filter(|c| !c.taken).ok_or("no untaken chosen companion (702.139a)")?;
        Ok(self.defs[chosen.definition]
            .companion
            .as_ref()
            .expect("chosen companion")
            .as_ref()
            .map_err(Clone::clone)?
            .cost
            .clone())
    }
    pub(crate) fn reserve_companion(&mut self, player: PlayerId) -> ObjId {
        let index = self.companions[player].as_ref().unwrap().definition;
        self.new_object(Some(index), self.defs[index].chars(), player, player, Zone::Gone)
    }
    pub(crate) fn take_companion(
        &mut self,
        player: PlayerId,
        steps: &[crate::legal::Step],
        with: Option<&crate::state::Pool>,
    ) -> crate::play::ActionResult<()> {
        let mut proposed = self.clone();
        let cost = proposed.companion_cost(player)?;
        let card = proposed.reserve_companion(player);
        let library_tops = proposed.library_tops();
        proposed.procedures.push(crate::procedure::Procedure::Payment(crate::payment::Payment::companion(
            player, card, cost, steps, with,
        ).library_tops(library_tops)));
        proposed.continue_execution()?;
        proposed.expose_payments();
        self.adopt(proposed);
        Ok(())
    }
    pub(crate) fn complete_companion(&mut self, player: PlayerId, card: ObjId) {
        self.companions[player].as_mut().unwrap().taken = true;
        self.move_to(card, Zone::Hand, player);
    }
}
