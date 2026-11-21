//! The Oracle corpus as the grammar sees it.
//!
//! Pipeline, each stage lossless so the grammar's round trip is a proof and not a formality:
//!
//! | Stage | Module | Inverse |
//! |---|---|---|
//! | Scryfall faces → [`corpus::Face`] | [`corpus`] | (none needed: the face is the input) |
//! | Oracle text → ability rows with reminder text, labels and self-references abstracted | [`normalize`] | [`normalize::Normalized::restore`] |
//! | row → tokens (words, numbers, symbols, punctuation, quoted abilities) | [`token`] | [`token::detokenize`] |
//! | rows → ability blocks with a coarse kind (keyword line, activated, triggered, …) | [`classify`] | blocks keep their row ranges |
//!
//! The closed vocabularies the stages need (ability words, keyword abilities, types) come from
//! the Comprehensive Rules via [`cr`], never from the corpus.

#![forbid(unsafe_code)]

pub mod classify;
pub mod corpus;
pub mod cr;
pub mod names;
pub mod normalize;
pub mod token;
