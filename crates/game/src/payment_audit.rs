//! Opt-in evidence at the actual locked payment boundary (601.2f–h).
//! Shared across speculative clones only while a host explicitly requests a quote;
//! failed/unfinished attempts remain distinguishable by their absent `left` pool.
use crate::state::{Game, Pool, PlayerId, Spent};
use crate::mana::ReducedCost;
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Scope { pub path: Vec<usize>, pub prior: Option<Pool>, pub triggered: bool, pub ability:Option<usize>, pub x:Option<u32>, pub life:Vec<bool> }
impl Scope { pub fn root()->Self {Self {path:Vec::new(),prior:None,triggered:false,ability:None,x:None,life:Vec::new()}} }

#[derive(Debug, Clone)]
pub(crate) struct Record { value:Value, pool:Pool }

impl Game {
    pub fn begin_payment_audit(&mut self) {
        self.payment_audit = Some(Default::default());
    }
    pub fn payment_mana_answers(&self) -> Vec<Value> {
        self.payment_audit.as_ref().map(|log|log.lock().expect("payment audit mutex").iter().filter(|r|r.value["answer_problem"]==true).map(|r|r.value.clone()).collect()).unwrap_or_default()
    }
    pub(crate) fn audit_mana_answer(&self, source:crate::ObjId, problem:&str) {
        if let Some(log)=&self.payment_audit {
            log.lock().expect("payment audit mutex").push(Record {pool:Pool::default(),value:json!({"answer_problem":true,
                "path":self.payment_scope.as_ref().map(|s|&s.path),"triggered":self.payment_scope.as_ref().map(|s|s.triggered),
                "source":source.0,"problem":problem})});
        }
    }
    pub fn payment_production_audit(&self) -> Vec<Value> {
        self.payment_audit.as_ref().map(|log|log.lock().expect("payment audit mutex").iter().filter(|r|r.value["production"]==true).map(|r|r.value.clone()).collect()).unwrap_or_default()
    }
    pub(crate) fn audit_mana_production(&self, source:crate::ObjId, player:PlayerId, ability:Option<&crate::binding::AbilityKey>, mana:&[ir::mana::ManaSymbol], before:Pool) {
        if let Some(log)=&self.payment_audit {
            let mut log=log.lock().expect("payment audit mutex");
            let sequence=log.len();
            log.push(Record {pool:before.clone(),value:json!({"production":true,"sequence":sequence,
                "path":self.payment_scope.as_ref().map(|s|&s.path),"triggered":self.payment_scope.as_ref().map(|s|s.triggered),
                "source":source.0,"player":player,"activation":self.payment_scope.as_ref().and_then(|s|s.ability),"invocation":ability.map(|key|self.audit_ability(Some(source),key)),
                "mana":mana.iter().map(|m|m.text()).collect::<Vec<_>>(),"before":Self::audit_payment_pool(&before),"after":Self::audit_payment_pool(&self.players[player].pool)})});
        }
    }
    pub fn payment_audit(&self) -> Vec<Value> {
        self.payment_audit.as_ref().map(|log|log.lock().expect("payment audit mutex").iter().filter(|r|r.value["production"]!=true && r.value["answer_problem"]!=true).map(|r|r.value.clone()).collect()).unwrap_or_default()
    }
    /// Pools at each actual locked boundary, in the same order as `payment_audit`.
    /// An unpaid boundary is retained; no allocation is invented to pass it.
    pub fn quoted_payment_pools(&self) -> Vec<Pool> {
        self.payment_audit.as_ref().map(|log| log.lock().expect("payment audit mutex").iter().filter(|r|r.value["production"]!=true && r.value["answer_problem"]!=true).map(|r| r.pool.clone()).collect()).unwrap_or_default()
    }
    pub fn quoted_payment_pool(&self) -> Option<Pool> {
        self.payment_audit.as_ref()?.lock().expect("payment audit mutex").iter().rev().find(|r|r.value["production"]!=true && r.value["answer_problem"]!=true).map(|r|r.pool.clone())
    }
    /// Same native pool vocabulary as the privileged wire snapshot, plus complete
    /// restriction identity for checking quotes. Indices remain local to this pool.
    pub fn audit_payment_pool(pool: &Pool) -> Value {
        let [w,u,b,r,g] = pool.colors;
        json!({"W":w,"U":u,"B":b,"R":r,"G":g,"C":pool.colorless,
            "restricted":pool.restricted.iter().map(|m|json!({"mana":m.mana.text(),"source":m.source.0,
                "provenance":m.provenance.as_deref().and_then(crate::audit::RestrictedManaProvenance::audit),"controller":m.controller,"ability":format!("{:?}",m.ability),
                "restriction":format!("{:?}",m.only),"uncounterable":m.uncounterable,
                "trigger":format!("{:?}",m.trigger),"kept":format!("{:?}",m.kept)})).collect::<Vec<_>>()})
    }
    pub(crate) fn audit_payment_start(&self, player:PlayerId, cost:&ReducedCost, selected:Option<&Pool>, on:Spent) -> Option<usize> {
        let log=self.payment_audit.as_ref()?;
        let mut log=log.lock().expect("payment audit mutex");
        let index=log.len();
        let (kind,source)=match on {Spent::Spell(o,_) => ("spell",o),Spent::Ability{source,..}=>("ability",source),Spent::Effect(o)=>("effect",o)};
        log.push(Record {pool:self.players[player].pool.clone(),value:json!({"path":self.payment_scope.as_ref().map(|s|&s.path),
            "prior_pool":self.payment_scope.as_ref().and_then(|s|s.prior.as_ref()).map(Self::audit_payment_pool),
            "symbol_indices":self.payment_scope.as_ref().and_then(|s|s.ability.and_then(|ability| self.payment_symbol_indices(source,Some(ability),crate::card::CastAs::Whole,&s.life,s.x,&cost.symbols.0.iter().map(|m|m.text()).collect::<Vec<_>>()))),
            "player":player,"kind":kind,"source":source.0,
            "requirement":{"symbols":cost.symbols.0.iter().map(|s|s.text()).collect::<Vec<_>>(),
                "reduction":cost.reduction,"typed_reduction":cost.typed_reduction,"minimum":cost.minimum,
                "substitution":cost.substitution,"convoke":cost.convoke},
            "pool":Self::audit_payment_pool(&self.players[player].pool),
            "selected":selected.map(Self::audit_payment_pool),"left":null})});
        Some(index)
    }
    pub(crate) fn audit_payment_finish(&self, index:Option<usize>, left:&Pool) {
        if let (Some(index),Some(log))=(index,&self.payment_audit) {
            log.lock().expect("payment audit mutex")[index].value["left"]=Self::audit_payment_pool(left);
        }
    }
}

impl Game {
    /// Authenticate printed-symbol indices only when the selected immutable cost
    /// matches the locked requirement after X/Phyrexian announcements. Extra or
    /// alternative cost text needs its own provenance; never guess an offset.
    pub fn payment_symbol_indices(&self, source:crate::state::ObjId, ability:Option<usize>, cast_as:crate::card::CastAs,
        life:&[bool], x:Option<u32>, locked:&[String]) -> Option<Vec<usize>> {
        use ir::mana::ManaSymbol as M;
        let original=if let Some(index)=ability {
            self.ability_cost(source,self.key(source,index).ok()?).ok()?.mana()
        } else {self.def(source)?.spell(cast_as).ok()?.cost()?.clone()};
        let mut choices=life.iter().copied();
        let mut indices=Vec::new();
        let mut symbols=Vec::new();
        for (index,symbol) in original.0.iter().copied().enumerate() {
            let symbol=match symbol {
                M::Phyrexian(_) | M::HybridPhyrexian(..) if choices.next().unwrap_or(false)=>continue,
                M::Phyrexian(c)=>M::Color(c),M::HybridPhyrexian(a,b)=>M::Hybrid(a,b),
                M::Var('X')=>M::Generic(x?),other=>other,
            };
            indices.push(index);symbols.push(symbol.text());
        }
        (symbols==locked).then_some(indices)
    }
}
