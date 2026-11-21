//! CR 106.7: query compiled production, without activating or paying for anything.
//!
//! Each dependency starts with no mana types. Repeatedly propagating actual output finds
//! the least fixed point: a circle cannot invent a color, but a Forest can ground it.
//! The six-bit result only grows, so this terminates even with mutually dependent lands.
//! Mana replacements are currently refused by the replacement compiler. When that family
//! is implemented, its possible output transformations must also participate here.

use std::collections::HashMap;
use std::sync::Arc;

use super::{Production, COLORS};
use crate::event::Target;
use crate::program::{expr, Program};
use crate::select::Ctx;
use crate::{Game, ObjId, Res, Zone};
use ir::mana::ManaSymbol;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Types(u8);
impl Types {
    fn of(symbol: ManaSymbol) -> Self {
        Self(match symbol {
            ManaSymbol::Color(c) => 1 << c as u8,
            ManaSymbol::Colorless => 1 << 5,
            _ => unreachable!("compiled production supplies mana types"),
        })
    }
    pub(super) fn contains(self, symbol: ManaSymbol) -> bool { self.0 & Self::of(symbol).0 != 0 }
    fn include(&mut self, other: Self) { self.0 |= other.0; }
}

fn members(game: &Game, set: &expr::Set, ctx: &Ctx, locals: &dyn expr::Locals) -> Vec<ObjId> {
    set.members(game, ctx, locals).into_iter().filter_map(|target| match target {
        Target::Object(o) if game.obj(o).zone == Zone::Battlefield => Some(o),
        _ => None,
    }).collect()
}

/// The query retains only programs, contexts and six-bit results, never a speculative Game.
pub(super) fn query(game: &Game, set: &expr::Set, ctx: &Ctx, locals: &dyn expr::Locals) -> Res<Types> {
    let roots = members(game, set, ctx, locals);
    let mut values: HashMap<ObjId, Types> = HashMap::new();
    let mut bodies: Vec<(ObjId, Vec<(Ctx<'static>, Arc<Program>)>)> = Vec::new();
    let mut pending = roots.clone();
    loop {
        for object in std::mem::take(&mut pending) {
            if values.contains_key(&object) { continue; }
            values.insert(object, Types::default());
            let mut programs = Vec::new();
            for member in game.members(object) {
                let def = member.def();
                let index = member.key.index;
                if !def.scope(index).is_some_and(|s| s.as_ref().is_ok_and(|s| s.contains(Zone::Battlefield))) { continue; }
                let Some(body) = def.programs.iter().find(|body| body.ability == index) else { continue; };
                let Ok(compiled) = &body.result else { continue; };
                for program in compiled.programs() {
                    if program.announcement.adds_mana {
                        // Cost/timing restrictions are not queried (106.7); the same exact
                        // member owner supplies inherited production in borrower context.
                        let ctx = Ctx::new(game.controller(object), object).with_key(member.key.clone());
                        programs.push((ctx, program.clone()));
                    }
                }
            }
            bodies.push((object, programs));
        }
        let mut changed = false;
        for (object, programs) in &bodies {
            let mut found = Types::default();
            for (ctx, program) in programs {
                found.include(program_types(game, ctx.clone(), program, &mut |dependency| {
                    match values.get(&dependency) {
                        Some(types) => *types,
                        None => { pending.push(dependency); Types::default() }
                    }
                })?);
            }
            let old = values[object];
            found.include(old);
            if old != found { values.insert(*object, found); changed = true; }
        }
        if !changed && pending.is_empty() { break; }
    }
    let mut result = Types::default();
    for root in roots { result.include(values[&root]); }
    Ok(result)
}

/// Use the compiled branch conditions and quantities; don't flatten conditional mana into
/// unconditional alternatives. Instructions after the last production cannot change it.
fn program_types(game: &Game, ctx: Ctx<'static>, program: &Program,
    dependency: &mut dyn FnMut(ObjId) -> Types) -> Res<Types> {
    let mut types = Types::default();
    super::visit_productions(game, ctx, program, &mut |ctx, locals, production| {
        types.include(production_types(game, ctx, locals, production, dependency)?);
        Ok(())
    })?;
    Ok(types)
}

/// Types don't require enumerating amounts/combinations, and spending restrictions don't
/// affect a mana type (106.6). Reuse the same compiled numeric expressions as execution.
fn production_types(game: &Game, ctx: &Ctx, locals: &dyn expr::Locals, production: &Production,
    dependency: &mut dyn FnMut(ObjId) -> Types) -> Res<Types> {
    let mut result = Types::default();
    match production {
        Production::Fixed(mana) => for m in mana { result.include(Types::of(m.symbol())); },
        Production::OneOf(options) => for m in options.iter().flatten() { result.include(Types::of(m.symbol())); },
        Production::Times { mana, amount } => if amount.evaluate(game, ctx, locals) > 0 {
            for m in mana { result.include(Types::of(m.symbol())); }
        },
        Production::AnyColors(amount) | Production::AnyOneColor(amount) => if amount.evaluate(game, ctx, locals) > 0 {
            for color in COLORS { result.include(Types::of(ManaSymbol::Color(color))); }
        },
        Production::CombinationOf { amount, of } => if amount.evaluate(game, ctx, locals) > 0 {
            for symbol in of { result.include(Types::of(*symbol)); }
        },
        Production::CommanderColors(amount) => if amount.evaluate(game, ctx, locals) > 0 {
            for color in game.commander_identity(ctx.controller) { result.include(Types::of(ManaSymbol::Color(color))); }
        },
        Production::CouldProduce { amount, by, types } => if amount.evaluate(game, ctx, locals) > 0 {
            for object in members(game, by, ctx, locals) { result.include(dependency(object)); }
            if !types { result.0 &= 0b11111; }
        },
        Production::Restricted { production, .. } => return production_types(game, ctx, locals, production, dependency),
    }
    Ok(result)
}
