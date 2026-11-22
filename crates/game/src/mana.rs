//! Paying mana (CR 601.2h, 602.2b), and mana that remembers how it may be spent (106.6).
//!
//! "Spend this mana only to cast a creature spell of the chosen type, and that spell can't be
//! countered.": a sentence of the text that adds the mana, said of all the mana that text adds
//! (106.6a). Such mana keeps its type (106.6) and goes into the pool as itself
//! ([`Restricted`]), with what it may be spent on and what it does to the spell it is spent
//! on, as the text said them of the permanent whose text it is ("the chosen type" is the one
//! chosen for that permanent, 607.2d, also once it has left the battlefield).
//!
//! The caller supplies the exact mana spent with [`crate::Move::Paying`]. Omitting
//! that selection supplies no mana; the engine never chooses units from a live pool.
//! Requirement equivalence validates only the supplied units (including hybrid costs).
//!
//! Compiled restrictions query the announced spell or an activated ability's source.
//! Other unsupported spending relationships remain refused at definition time.

use crate::select::Ctx;
use crate::state::{Game, Pool, Restricted, Spent};
use ir::effect::Rider;
use ir::mana::{ManaCost, ManaSymbol};
use ir::{Effect, Sentence};

/// A compiled payment requirement can retain a generic reduction until hybrid symbols
/// choose their nonhybrid equivalent (601.2b, 601.2f, 118.7a). Printed mana costs have none.
/// The pool cannot pay the mana of a cost, whatever mana were chosen from it (118.3a). The
/// caller that knows the total says it.
pub(crate) const UNPAYABLE: &str = "your mana pool cannot pay that cost (118.3a)";
/// No mana was chosen from a pool that could pay the cost: the player says which mana pays
/// (118.3a), and the engine never chooses it. The caller that knows the total says it.
pub(crate) const UNCHOSEN: &str = "no mana was chosen from your mana pool to pay that cost (118.3a)";

pub(crate) trait Requirement {
    fn symbols(&self) -> &[ManaSymbol];
    fn reduction(&self) -> u32 { 0 }
    fn typed_reduction(&self) -> [u32; 6] { [0; 6] }
    fn minimum(&self) -> u32 { 0 }
    fn substitution(&self) -> u32 { 0 }
    fn convoke(&self) -> [u32;5] { [0;5] }
}
impl Requirement for ManaCost {
    fn symbols(&self) -> &[ManaSymbol] { &self.0 }
}
#[derive(Debug, Clone)]
pub(crate) struct ReducedCost {
    pub(crate) symbols: ManaCost,
    pub(crate) reduction: u32,
    pub(crate) typed_reduction: [u32; 6],
    pub(crate) minimum: u32,
    pub(crate) substitution: u32,
    pub(crate) convoke: [u32;5],
}
impl Requirement for ReducedCost {
    fn symbols(&self) -> &[ManaSymbol] { &self.symbols.0 }
    fn reduction(&self) -> u32 { self.reduction }
    fn typed_reduction(&self) -> [u32; 6] { self.typed_reduction }
    fn minimum(&self) -> u32 { self.minimum }
    fn substitution(&self) -> u32 { self.substitution }
    fn convoke(&self) -> [u32;5] { self.convoke }
}

/// "Spend this mana only to …", when that is a sentence of this text.
pub(crate) fn spend_only(effect: &Effect) -> Option<&Rider> {
    effect.0.iter().find_map(|s| match s {
        Sentence::Rider(r @ Rider::SpendOnly { .. }) => Some(r),
        _ => None,
    })
}

/// "Until end of combat, you don't lose this mana as steps and phases end." (702.189a), "Until
/// end of turn, you don't lose this mana as steps and phases end.": the mana this text adds is
/// kept in its pool until then (106.4), when that is a sentence of this text.
pub(crate) fn kept(effect: &Effect) -> Option<crate::state::Kept> {
    effect.0.iter().find_map(kept_by)
}
/// Is this sentence the one [`kept`] reads, and until when?
pub(crate) fn kept_by(sentence: &Sentence) -> Option<crate::state::Kept> {
    use ir::effect::{Duration, Lead, Modifier};
    use ir::event::{Step, StepOwner, Time};
    let Sentence::Led { lead: Lead::Modifier(Modifier::Duration(Duration::Until(Time { end: true, step, whose: StepOwner::This }))), rest } = sentence else { return None };
    let Sentence::Do(body) = rest.as_ref() else { return None };
    let clause = &body.first;
    let you = clause.subject.as_ref().is_none_or(|s| matches!(s, ir::Subject::Selector(s) if s.quant == ir::Quant::Definite(ir::Ref::You)));
    let keeps = matches!(clause.first.action, ir::Action::KeepMana { mana: ir::mana::Pooled::This, phases: true });
    if !body.rest.is_empty() || !clause.rest.is_empty() || !clause.first.mods.is_empty() || clause.may || clause.first.may || !you || !keeps { return None; }
    match step {
        Step::Combat => Some(crate::state::Kept::Combat),
        Step::Turn => Some(crate::state::Kept::Turn),
        _ => None,
    }
}

/// Validate that all supplied units pay one equivalent of the requirement (601.2h).
/// Backtracking checks hybrid equivalence; it never chooses units from the live pool.
fn equivalent(pool: &Pool, cost: &dyn Requirement) -> Option<()> { pays(pool, cost, false) }

/// Could some of this pool's mana pay the requirement (118.3a)? `spendable` is the pool's
/// restricted mana that may be spent on this (106.6). It tells a pool that is short whatever
/// is chosen from one the player did not choose from; it pays nothing and names no payment:
/// the player still says which mana pays (601.2h).
fn suffices<'a>(pool: &Pool, spendable: impl Iterator<Item = &'a Restricted>, cost: &dyn Requirement) -> bool {
    let mut all = Pool { colors: pool.colors, colorless: pool.colorless, restricted: Vec::new() };
    for unit in spendable {
        let Some(count) = plain(&mut all, unit.mana) else { continue };
        *count = count.saturating_add(1);
    }
    pays(&all, cost, true).is_some()
}

/// Whether the units pay one equivalent of the requirement: all of them, or with `over`, some.
fn pays(pool: &Pool, cost: &dyn Requirement, over: bool) -> Option<()> {
    type State = (usize, [u32; 5], u32, u32, [u32;5], [u32;6]);
    #[allow(clippy::too_many_arguments)]
    fn search(pool: &Pool, symbols: &[ManaSymbol], at: usize, generic: u32, reduction: u32, typed_reduction: [u32;6], minimum: u32, substitution: u32, convoke: [u32;5],
        over: bool, chosen: &mut Vec<ManaSymbol>, failed: &mut std::collections::HashSet<State>) -> Option<()> {
        let available = pool.colors.iter().sum::<u32>() + pool.colorless;
        if available.saturating_add(substitution) < generic.saturating_sub(reduction + typed_reduction.iter().sum::<u32>()) { return None; }
        let state = (at, pool.colors, pool.colorless, generic, convoke, typed_reduction);
        if failed.contains(&state) { return None; }
        let Some(symbol) = symbols.get(at) else {
            if convoke.iter().any(|n|*n!=0) {return None;}
            let typed = chosen.iter().filter(|s| !matches!(s, ManaSymbol::Generic(_))).count() as u32;
            let generic = generic.saturating_sub(reduction + typed_reduction.iter().sum::<u32>()).max(minimum.saturating_sub(typed)).checked_sub(substitution)?;
            return (available == generic || (over && available > generic)).then_some(());
        };
        let (first, second) = symbol.mana_payments()?;
        for payment in std::iter::once(first).chain(second) {
            let mut left = pool.clone();
            let mut generic = generic;
            let mut convoke=convoke;
            let mut typed_reduction = typed_reduction;
            let typed = match payment { ManaSymbol::Color(c) => Some(c as usize), ManaSymbol::Colorless => Some(5), _ => None };
            let reduced = typed.is_some_and(|c| typed_reduction[c] > 0);
            if reduced { typed_reduction[typed.unwrap()] -= 1; }
            match payment {
                _ if reduced => {},
                ManaSymbol::Color(c) if convoke[c as usize]>0 => convoke[c as usize]-=1,
                ManaSymbol::Generic(n) => generic = generic.checked_add(n)?,
                _ => {
                    let have = plain(&mut left, payment)?;
                    if *have == 0 { continue; }
                    *have -= 1;
                }
            }
            if !reduced { chosen.push(payment); }
            if let Some(found) = search(&left, symbols, at + 1, generic, reduction, typed_reduction, minimum, substitution, convoke, over, chosen, failed) { return Some(found); }
            if !reduced { chosen.pop(); }
        }
        failed.insert(state);
        None
    }
    search(pool, cost.symbols(), 0, 0, cost.reduction(), cost.typed_reduction(), cost.minimum(), cost.substitution(), cost.convoke(), over, &mut Vec::new(), &mut std::collections::HashSet::new())
}

/// One mana of a type from the plain counts of a pool.
fn plain(pool: &mut Pool, of: ManaSymbol) -> Option<&mut u32> {
    match of {
        ManaSymbol::Color(c) => Some(&mut pool.colors[c as usize]),
        ManaSymbol::Colorless => Some(&mut pool.colorless),
        _ => None,
    }
}

impl Game {
    /// May this restricted mana be spent on this (106.6)? "To cast a creature spell of the
    /// chosen type": on a card being cast that is such a spell, as its text describes it, of
    /// the permanent whose text it is.
    pub(crate) fn may_spend(&self, mana: &Restricted, on: Spent) -> bool {
        let ctx = Ctx { ability: mana.ability.clone(), ..Ctx::new(mana.controller, mana.source) };
        mana.only.allows(self, &ctx, on)
    }

    /// Take a cost out of a pool with the mana the player chooses to pay it with (107.4b,
    /// 118.3a): mana that is in the pool, may be spent on this, and pays the cost with nothing
    /// over. With none stated, no mana is supplied. Returns the pool left and the
    /// restricted mana spent; it never allocates units from the live pool.
    pub(crate) fn spend_with(&self, pool: &Pool, cost: &dyn Requirement, with: Option<&Pool>, on: Spent) -> crate::play::ActionResult<(Pool, Vec<Restricted>)> {
        let empty = Pool::default();
        let supplied = with;
        let with = with.unwrap_or(&empty);
        let mut left = pool.clone();
        for (have, spent) in left.colors.iter_mut().chain(std::iter::once(&mut left.colorless)).zip(with.colors.iter().chain(std::iter::once(&with.colorless))) {
            *have = have.checked_sub(*spent).ok_or("that mana is not in your mana pool (118.3a)")?;
        }
        for r in &with.restricted {
            let at = left.restricted.iter().position(|x| x == r).ok_or("that mana is not in your mana pool (118.3a)")?;
            if !self.may_spend(r, on) {
                return Err("that mana can't be spent on that: its text restricts how it can be spent (106.6)".into());
            }
            left.restricted.remove(at);
        }
        let mut selected = Pool { colors: with.colors, colorless: with.colorless, restricted: Vec::new() };
        for unit in &with.restricted {
            let count = plain(&mut selected, unit.mana).ok_or("a supplied mana unit must have a mana type (106.1)")?;
            *count = count.checked_add(1).ok_or_else(|| crate::Unsupported("supplied mana exceeds the engine numerical range".into()))?;
        }
        selected.colors.iter().chain(std::iter::once(&selected.colorless))
            .try_fold(0u32, |total, count| total.checked_add(*count))
            .ok_or_else(|| crate::Unsupported("supplied mana exceeds the engine numerical range".into()))?;
        match equivalent(&selected, cost) {
            Some(()) => Ok((left, with.restricted.clone())),
            // a pool too short whatever is chosen, or one nothing was chosen from
            _ if supplied.is_none() && !suffices(pool, pool.restricted.iter().filter(|r| self.may_spend(r, on)), cost) => Err(UNPAYABLE.into()),
            _ if supplied.is_none() => Err(UNCHOSEN.into()),
            _ => Err("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)".into()),
        }
    }
}

/// The same mana, said in any order: a pool's mana has no order (106.4).
pub(crate) fn same_mana(a: &[ManaSymbol], b: &[ManaSymbol]) -> bool {
    a.len() == b.len() && a.iter().all(|m| a.iter().filter(|n| *n == m).count() == b.iter().filter(|n| *n == m).count())
}

#[cfg(test)]
mod explicit_pool_tests {
    use super::*;
    use crate::{CardDef, Move, Zone};

    #[test]
    fn r118_3a_a_short_pool_is_told_from_one_nothing_was_chosen_from() {
        // a pool of R and G: {U} needs blue mana it lacks and {3} three mana where it has two,
        // whatever is chosen (118.3a); {1}{G} and {R/U} it could pay, had mana been chosen (601.2h)
        let costs = ["{U}", "{3}", "{1}{G}", "{R/U}"];
        let defs: Vec<CardDef> = costs.iter().map(|cost| CardDef::new("Gift", cost, "Sorcery", "You gain 1 life.", None).unwrap()).collect();
        let mut g = Game::new(defs, 2, 20);
        g.players[0].pool.colors[3] = 1;
        g.players[0].pool.colors[4] = 1;
        let refusal = |g: &mut Game, index: usize| {
            let card = g.put(0, index, Zone::Hand);
            g.act(0, Move::Cast { card, targets: vec![] }).unwrap_err()
        };
        assert_eq!(refusal(&mut g, 0), "your mana pool cannot pay that cost (118.3a): {U}");
        assert_eq!(refusal(&mut g, 1), "your mana pool cannot pay that cost (118.3a): {3}");
        assert_eq!(refusal(&mut g, 2), "no mana was chosen from your mana pool to pay that cost (118.3a): {1}{G}");
        assert_eq!(refusal(&mut g, 3), "no mana was chosen from your mana pool to pay that cost (118.3a): {R/U}");
        assert_eq!(g.players[0].pool.total(), 2, "neither refusal spends any of it");
    }

    #[test]
    fn r601_2h_no_pool_selection_spends_nothing_and_each_exact_generic_selection_is_authored() {
        let defs = vec![CardDef::new("Gift", "{1}", "Sorcery", "You gain 1 life.", None).unwrap(),
                        CardDef::new("Free gift", "{0}", "Sorcery", "You gain 1 life.", None).unwrap()];
        let mut g = Game::new(defs, 2, 20);
        let card = g.put(0, 0, Zone::Hand);
        g.players[0].pool.colors[3] = 1;
        g.players[0].pool.colors[4] = 1;
        let cast = Move::Cast { card, targets: vec![] };
        // an empty pool cannot pay {1} whatever is chosen (118.3a): the shortfall says so, with
        // the total cost determined (601.2f)
        let mut empty = g.clone();
        empty.players[0].pool = Pool::default();
        assert_eq!(empty.act(0, cast.clone()).unwrap_err(), "your mana pool cannot pay that cost (118.3a): {1}");
        // R and G could pay {1}; the player did not say which (601.2h): an omission, not a
        // shortfall, and the engine does not choose for them
        let before = format!("{g:?}");
        assert_eq!(g.act(0, cast.clone()).unwrap_err(), "no mana was chosen from your mana pool to pay that cost (118.3a): {1}");
        assert_eq!(format!("{g:?}"), before, "omission refuses atomically (733.1)");
        let both = g.players[0].pool.clone();
        assert_eq!(g.act(0, Move::Paying(both, Box::new(cast.clone()))).unwrap_err(),
            "the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)");
        assert_eq!(format!("{g:?}"), before, "overspending changes nothing");
        for color in [3, 4] {
            let mut branch = g.clone();
            let mut selected = Pool::default(); selected.colors[color] = 1;
            branch.act(0, Move::Paying(selected, Box::new(cast.clone()))).unwrap();
            assert_eq!(branch.players[0].pool.colors[color], 0);
            assert_eq!(branch.players[0].pool.colors[7 - color], 1, "the other color was not chosen");
        }
        let free = g.put(0, 1, Zone::Hand);
        let pool = g.players[0].pool.clone();
        g.act(0, Move::Cast { card: free, targets: vec![] }).unwrap();
        assert_eq!(g.players[0].pool, pool, "a zero cost needs no supplied mana");
    }

    #[test]
    fn r106_6_exact_plain_or_restricted_units_preserve_unchosen_kept_mana() {
        let defs = vec![CardDef::new("Body", "{G}", "Creature", "", Some((1, 1))).unwrap(),
            CardDef::new("Gift", "{G}", "Sorcery", "You gain 1 life.", None).unwrap(),
            CardDef::new("Source", "", "Land", "{T}: Add {G}. Spend this mana only to cast a creature spell.", None).unwrap()];
        let mut g = Game::new(defs, 2, 20);
        let source = g.put(0, 2, Zone::Battlefield);
        g.act(0, Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }).unwrap();
        g.players[0].pool.restricted[0].kept = Some(crate::state::Kept::Turn);
        g.players[0].pool.colors[4] = 1;
        let body = g.put(0, 0, Zone::Hand);
        let gift = g.put(0, 1, Zone::Hand);
        let unit = g.players[0].pool.restricted[0].clone();
        let selected = Pool { restricted: vec![unit.clone()], ..Pool::default() };
        let before = format!("{g:?}");
        assert_eq!(g.act(0, Move::Paying(selected.clone(), Box::new(Move::Cast { card: gift, targets: vec![] }))).unwrap_err(),
            "that mana can't be spent on that: its text restricts how it can be spent (106.6)");
        assert_eq!(format!("{g:?}"), before);
        let mut plain = Pool::default(); plain.colors[4] = 1;
        let mut plain_branch = g.clone();
        plain_branch.act(0, Move::Paying(plain, Box::new(Move::Cast { card: body, targets: vec![] }))).unwrap();
        assert_eq!(plain_branch.players[0].pool.restricted, vec![unit], "the unchosen lot keeps all terms (106.6, 106.4)");
        g.act(0, Move::Paying(selected, Box::new(Move::Cast { card: body, targets: vec![] }))).unwrap();
        assert!(g.players[0].pool.restricted.is_empty());
        assert_eq!(g.players[0].pool.colors[4], 1, "plain mana was not selected");
    }

    #[test]
    fn r106_6_mana_its_text_keeps_from_a_spell_leaves_the_pool_short_for_it() {
        // a pool of one {G} spendable only to cast a creature spell: it cannot pay a sorcery's
        // {G}, whatever is chosen (106.6, 118.3a); a creature's {G} it could, had it been
        // chosen (601.2h)
        let defs = vec![CardDef::new("Body", "{G}", "Creature", "", Some((1, 1))).unwrap(),
            CardDef::new("Gift", "{G}", "Sorcery", "You gain 1 life.", None).unwrap(),
            CardDef::new("Source", "", "Land", "{T}: Add {G}. Spend this mana only to cast a creature spell.", None).unwrap()];
        let mut g = Game::new(defs, 2, 20);
        let source = g.put(0, 2, Zone::Battlefield);
        g.act(0, Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }).unwrap();
        let body = g.put(0, 0, Zone::Hand);
        let gift = g.put(0, 1, Zone::Hand);
        assert_eq!(g.act(0, Move::Cast { card: gift, targets: vec![] }).unwrap_err(), "your mana pool cannot pay that cost (118.3a): {G}");
        assert_eq!(g.act(0, Move::Cast { card: body, targets: vec![] }).unwrap_err(), "no mana was chosen from your mana pool to pay that cost (118.3a): {G}");
        assert_eq!(g.players[0].pool.restricted.len(), 1, "neither refusal spends it");
    }
}
