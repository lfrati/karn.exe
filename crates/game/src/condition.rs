//! Conditions: what "if …" asks of the game as it is.
//!
//! An instruction that is followed only if something is true ("If you control a Swamp, …",
//! "draw a card if your life total is greater than your starting life total", 608.2c) asks
//! the game once, as the effect is applied (608.2h). What a condition asks, and which the
//! engine refuses, is [`crate::program::condition`]: the one implementation. Runtime abilities
//! and registrations retain that compiled form. This helper exposes explicit compilation and
//! evaluation for tools and tests; the game driver does not call it.

use crate::program::condition::Condition as Compiled;
use crate::program::expr::{NoLocals, linked};
use crate::select::{Ctx, Support};
use crate::state::Game;
use crate::Res;
use ir::Condition;

/// Does this condition hold, now?
pub fn holds(game: &Game, ctx: &Ctx, c: &Condition) -> Res<bool> {
    let support = Support::runtime(game, ctx);
    Ok(Compiled::compile(c, &support, &mut linked(&support))?.evaluate(game, ctx, &NoLocals))
}
