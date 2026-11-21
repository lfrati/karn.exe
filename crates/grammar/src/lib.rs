//! Oracle text ⇄ IR. Every rule parses and prints ([`kernel`]); sub-grammars are measured on
//! their own sub-corpora before they are composed into abilities.

#![forbid(unsafe_code)]

pub mod ability;
pub mod action;
pub mod amount;
pub mod becomes;
pub mod choice;
pub mod condition;
pub mod cost;
pub mod effect;
pub mod event;
pub mod except;
pub mod kernel;
pub mod keyword;
pub mod mana;
pub mod procedure;
pub mod selector;
pub mod statics;
pub mod token;
pub mod vote;
pub mod words;
pub mod zone_move;
