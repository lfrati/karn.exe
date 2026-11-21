//! Where in a library cards are put, and in what order.

use crate::action::{LibraryPosition, Order, Whose};
use serde::Serialize;

/// Cards put into a library: whose library, where in it, in what order.
///
/// "If an effect puts two or more cards in a specific position in a library at the same time,
/// the owner of those cards may arrange them in any order" (CR 401.4); "If an effect causes a
/// player to put a card into a library 'Nth from the top,' and that library has fewer than N
/// cards in it, the player puts that card on the bottom of that library" (401.7).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct LibraryPlace {
    /// "on top of your library", "on the bottom of its owner's library". None: the library the
    /// text last spoke of, which it does not repeat ("put the rest on the bottom in a random
    /// order" after "look at the top six cards of your library"; "search your library …, then
    /// shuffle and put that card on top").
    pub library: Option<Whose>,
    /// "put one of them back on top of your library", "put them back in any order": into the
    /// library they were looked at in, so the library is the one they came from. With
    /// `library` None the position is the top (the cards were looked at there).
    pub back: bool,
    pub position: Position,
    /// "in any order" (the owner chooses) or "in a random order" (401.4).
    pub order: Option<Order>,
}

/// A position in a library.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Position {
    /// "on top of", "on the bottom of", "third from the top of".
    One(LibraryPosition),
    /// "on your choice of the top or bottom of", "on the top or bottom of", "second from the top
    /// or on the bottom of": one of two positions, chosen by `chooser` (None: the player who
    /// puts the card).
    Choice { chooser: Option<Whose>, between: [LibraryPosition; 2] },
    /// "just beneath the top X cards of that library".
    BeneathTopX,
}
