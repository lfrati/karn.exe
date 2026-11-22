//! The optional reveal belongs to the first actual draw, before the next draw (702.94).
//! Its linked trigger offers only this hand incarnation and this printed alternative cost.
use crate::{CardDef, Game, ObjId, Res};
use ir::{Ability, Cost, CostPart, token::{KeywordInstance, KeywordParam}};

pub(crate) fn cost(item: &KeywordInstance) -> Option<Res<Cost>> {
    if item.keyword.name() != "Miracle" { return None; }
    let mana = match &item.param {
        KeywordParam::Mana(mana) => Some(mana),
        KeywordParam::Cost(Cost(parts)) => match parts.as_slice() { [CostPart::Mana(mana)] => Some(mana), _ => None },
        _ => None,
    };
    Some(match mana {
        Some(mana) if mana.0.iter().all(|symbol| matches!(symbol, ir::mana::ManaSymbol::Var('X')) || symbol.mana_payments().is_some()) => Ok(Cost(vec![CostPart::Mana(mana.clone())])),
        _ => crate::unsupported("miracle: a printed mana cost, optionally including X (702.94a)"),
    })
}

pub(crate) fn instances(def: &CardDef) -> impl Iterator<Item = (usize, Cost, usize)> + '_ {
    crate::keyword::generated_by(def).filter_map(|(made, instance)| {
        cost(&instance.item)?.ok().map(|cost| (instance.row, cost, made))
    })
}

pub(crate) fn trigger(cost: &Cost) -> Ability {
    static RULE: std::sync::OnceLock<Ability> = std::sync::OnceLock::new();
    let mut ability = RULE.get_or_init(|| {
        CardDef::new("Miracle rule", "", "Sorcery",
            "When you draw a card, you may cast this card by paying {0} rather than paying its mana cost.", None)
            .expect("the grammar reads the linked cast of rule 702.94a").abilities[0].clone()
    }).clone();
    let Ability::Triggered { effect: ir::Effect(sentences), .. } = &mut ability else { unreachable!() };
    let Some(ir::Sentence::Do(body)) = sentences.first_mut() else { unreachable!() };
    let Some(ir::effect::Modifier::By(said)) = body.first.first.mods.first_mut() else { unreachable!() };
    said.actions.items = cost.0.iter().map(|part| match part {
        CostPart::Mana(mana) => ir::Action::PayMana(mana.clone()), _ => unreachable!("a supported Miracle price is mana"),
    }).collect();
    ability
}

impl Game {
    /// A completed choice creates one original occurrence. Copies retain its linked cast,
    /// but do not acquire its independent reveal lifetime (702.94b, 707.10).
    pub(crate) fn reveal_miracle(&mut self, card: ObjId, index: usize) {
        let player = self.obj(card).owner;
        let key = crate::binding::AbilityKey::printed(index);
        let ctx = crate::select::Ctx::new(player, card).with_key(key.clone());
        let captured = crate::program::capture(self, ctx);
        let bound = crate::state::Bound { revealed_through_resolution: vec![card], ..Default::default() };
        self.reveal(player, vec![card]);
        self.triggered.push(crate::state::Triggered { source: Some(card), controller: player,
            origin: key.origin, index, bound, delayed: None, captured: Some(captured) });
    }
}
